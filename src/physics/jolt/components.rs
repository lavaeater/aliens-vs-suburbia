//! The components game code puts on entities, with Avian's names, fields and constructors.

use std::sync::Arc;

use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use oxijolt::Shape;

/// How a body moves. A `Collider` without one is static, as in Avian.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[reflect(Component)]
#[require(Position, Rotation, LinearVelocity, AngularVelocity)]
pub enum RigidBody {
    #[default]
    Dynamic,
    Static,
    Kinematic,
}

impl RigidBody {
    pub fn is_static(self) -> bool {
        self == Self::Static
    }
    pub fn is_dynamic(self) -> bool {
        self == Self::Dynamic
    }
    pub fn is_kinematic(self) -> bool {
        self == Self::Kinematic
    }
}

/// What a collider is, kept for debug drawing and for building the Jolt shape again.
#[derive(Clone, Debug, PartialEq)]
pub enum ColliderKind {
    /// Half extents.
    Cuboid(Vec3),
    Sphere(f32),
    /// Along Y; `half_length` is the half length of the straight part.
    Capsule { radius: f32, half_length: f32 },
    Cylinder { radius: f32, half_height: f32 },
    /// Tip up, as Avian's.
    Cone { radius: f32, half_height: f32 },
    ConvexHull(Arc<Vec<Vec3>>),
    /// A static triangle mesh; what `Collider::heightfield` becomes.
    TriangleMesh {
        vertices: Arc<Vec<Vec3>>,
        triangles: Arc<Vec<[u32; 3]>>,
    },
}

/// A collision shape. Sizes follow Avian's constructors (full extents, `capsule(radius,
/// length)`); the Jolt shape is built once, here, and shared by every body made from it.
#[derive(Component, Clone)]
#[require(Position, Rotation)]
pub struct Collider {
    kind: ColliderKind,
    shape: Arc<Shape>,
}

impl std::fmt::Debug for Collider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)
    }
}

/// Jolt refuses zero and negative sizes; the game never asks for them, so keep a body rather
/// than panic if it ever does.
const MIN_EXTENT: f32 = 1e-3;

/// Boxes and cylinders keep Jolt's default rounded edges (0.05 m) only when they are big
/// enough; Avian's are sharp, and a small box with a big radius is a pebble.
fn convex_radius(smallest_half_extent: f32) -> f32 {
    (smallest_half_extent * 0.1).min(0.05)
}

impl Collider {
    fn new(kind: ColliderKind) -> Self {
        // A constant 10 cm sphere is always within Jolt's limits.
        #[allow(clippy::expect_used)]
        let shape = build_shape(&kind).unwrap_or_else(|e| {
            warn!("collider {kind:?} refused by Jolt ({e}); using a small sphere");
            Shape::new_sphere(0.1).expect("a 10 cm sphere")
        });
        Self { kind, shape: Arc::new(shape) }
    }

    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        let half = (Vec3::new(x, y, z) / 2.0).max(Vec3::splat(MIN_EXTENT));
        Self::new(ColliderKind::Cuboid(half))
    }

    pub fn sphere(radius: f32) -> Self {
        Self::new(ColliderKind::Sphere(radius.max(MIN_EXTENT)))
    }

    /// `length` is the straight part between the two caps, as Avian's.
    pub fn capsule(radius: f32, length: f32) -> Self {
        Self::new(ColliderKind::Capsule {
            radius: radius.max(MIN_EXTENT),
            half_length: (length / 2.0).max(MIN_EXTENT),
        })
    }

    pub fn cylinder(radius: f32, height: f32) -> Self {
        Self::new(ColliderKind::Cylinder {
            radius: radius.max(MIN_EXTENT),
            half_height: (height / 2.0).max(MIN_EXTENT),
        })
    }

    pub fn cone(radius: f32, height: f32) -> Self {
        Self::new(ColliderKind::Cone {
            radius: radius.max(MIN_EXTENT),
            half_height: (height / 2.0).max(MIN_EXTENT),
        })
    }

    /// `None` when the points span no volume, as Avian's.
    pub fn convex_hull(points: Vec<Vec3>) -> Option<Self> {
        let kind = ColliderKind::ConvexHull(Arc::new(points));
        let shape = build_shape(&kind).ok()?;
        Some(Self { kind, shape: Arc::new(shape) })
    }

    /// Avian's (parry's) heightfield, as a static triangle mesh: Jolt's own heightfield is
    /// square and quantises the heights. Same layout as parry: `heights` is read flattened
    /// and column-major as `rows` x `columns` (`heights.len()` x `heights[0].len()`), rows
    /// along Z and columns along X, centred on the origin and spanning `scale.x` by
    /// `scale.z`; heights are multiplied by `scale.y`. Each cell is split along the same
    /// diagonal as parry's, so both backends have the same ground.
    pub fn heightfield(heights: Vec<Vec<f32>>, scale: Vec3) -> Self {
        let rows = heights.len();
        let columns = heights.first().map_or(0, Vec::len);
        let data: Vec<f32> = heights.into_iter().flatten().collect();
        if rows < 2 || columns < 2 || data.len() != rows * columns {
            warn!("heightfield of {rows} x {columns} with {} heights; using a small sphere", data.len());
            return Self::sphere(0.1);
        }
        let step = Vec3::new(
            scale.x / (columns - 1) as f32,
            scale.y,
            scale.z / (rows - 1) as f32,
        );
        // Vertex `i + j * rows` is row `i` (z), column `j` (x), as parry stores them.
        let vertices: Vec<Vec3> = (0..columns)
            .flat_map(|j| {
                let data = &data;
                (0..rows).map(move |i| {
                    Vec3::new(
                        -scale.x / 2.0 + j as f32 * step.x,
                        data[i + j * rows] * step.y,
                        -scale.z / 2.0 + i as f32 * step.z,
                    )
                })
            })
            .collect();
        let id = |i: usize, j: usize| (i + j * rows) as u32;
        let mut triangles = Vec::with_capacity((rows - 1) * (columns - 1) * 2);
        for j in 0..columns - 1 {
            for i in 0..rows - 1 {
                let (p00, p10, p01, p11) = (id(i, j), id(i + 1, j), id(i, j + 1), id(i + 1, j + 1));
                // Parry's split (no zigzag), wound to face up.
                triangles.push(upward(&vertices, [p00, p10, p01]));
                triangles.push(upward(&vertices, [p10, p11, p01]));
            }
        }
        Self::new(ColliderKind::TriangleMesh {
            vertices: Arc::new(vertices),
            triangles: Arc::new(triangles),
        })
    }

    /// The size of the collider's own bounding box, unscaled and unrotated.
    pub fn local_size(&self) -> Vec3 {
        match &self.kind {
            ColliderKind::Cuboid(half) => *half * 2.0,
            ColliderKind::Sphere(r) => Vec3::splat(r * 2.0),
            ColliderKind::Capsule { radius, half_length } => {
                Vec3::new(radius * 2.0, (half_length + radius) * 2.0, radius * 2.0)
            }
            ColliderKind::Cylinder { radius, half_height }
            | ColliderKind::Cone { radius, half_height } => {
                Vec3::new(radius * 2.0, half_height * 2.0, radius * 2.0)
            }
            ColliderKind::ConvexHull(points) => bounds(points).1 - bounds(points).0,
            ColliderKind::TriangleMesh { vertices, .. } => bounds(vertices).1 - bounds(vertices).0,
        }
    }

    pub fn kind(&self) -> &ColliderKind {
        &self.kind
    }

    pub(super) fn shape(&self) -> &Shape {
        &self.shape
    }
}

/// The smallest and largest corner of `points`; zero for none.
pub(super) fn bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    if points.is_empty() {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    points.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
}

/// `triangle` in the order that makes it face up (counter-clockwise seen from above).
fn upward(vertices: &[Vec3], triangle: [u32; 3]) -> [u32; 3] {
    let [a, b, c] = triangle.map(|i| vertices[i as usize]);
    if (b - a).cross(c - a).y >= 0.0 {
        triangle
    } else {
        [triangle[0], triangle[2], triangle[1]]
    }
}

fn build_shape(kind: &ColliderKind) -> Result<Shape, oxijolt::ShapeError> {
    match kind {
        ColliderKind::Cuboid(half) => {
            Shape::new_box_with_convex_radius((*half).into(), convex_radius(half.min_element()))
        }
        ColliderKind::Sphere(r) => Shape::new_sphere(*r),
        ColliderKind::Capsule { radius, half_length } => Shape::new_capsule(*half_length, *radius),
        ColliderKind::Cylinder { radius, half_height } => Shape::new_cylinder_with_convex_radius(
            *half_height,
            *radius,
            convex_radius(radius.min(*half_height)),
        ),
        ColliderKind::Cone { radius, half_height } => {
            Shape::new_tapered_cylinder(*half_height, 0.0, *radius)
        }
        ColliderKind::ConvexHull(points) => {
            let points: Vec<oxijolt::Vec3> = points.iter().map(|&p| p.into()).collect();
            Shape::new_convex_hull(&points)
        }
        ColliderKind::TriangleMesh { vertices, triangles } => {
            let vertices: Vec<oxijolt::Vec3> = vertices.iter().map(|&p| p.into()).collect();
            let (shape, dropped) = Shape::new_mesh(&vertices, triangles)?;
            if !dropped.indices().is_empty() {
                debug!("triangle mesh: Jolt dropped {} degenerate triangles", dropped.indices().len());
            }
            Ok(shape)
        }
    }
}

/// World-space position, authoritative over `Transform` for bodies (as Avian's): the game writes
/// it to teleport, the backend writes it back after every step.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Deref, DerefMut)]
#[reflect(Component)]
pub struct Position(pub Vec3);

impl Position {
    pub const fn new(v: Vec3) -> Self {
        Self(v)
    }
}

impl From<Vec3> for Position {
    fn from(v: Vec3) -> Self {
        Self(v)
    }
}

/// World-space rotation, as `Position`.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Deref, DerefMut)]
#[reflect(Component)]
pub struct Rotation(pub Quat);

impl From<Quat> for Rotation {
    fn from(q: Quat) -> Self {
        Self(q)
    }
}

#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Deref, DerefMut)]
#[reflect(Component)]
pub struct LinearVelocity(pub Vec3);

impl LinearVelocity {
    pub const ZERO: Self = Self(Vec3::ZERO);
}

#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Deref, DerefMut)]
#[reflect(Component)]
pub struct AngularVelocity(pub Vec3);

impl AngularVelocity {
    pub const ZERO: Self = Self(Vec3::ZERO);
}

/// Degrees of freedom a body may not use, as Avian's builder.
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Component)]
pub struct LockedAxes(u8);

impl LockedAxes {
    pub const ROTATION_LOCKED: Self = Self(0b111_000);
    pub const TRANSLATION_LOCKED: Self = Self(0b000_111);
    pub const ALL_LOCKED: Self = Self(0b111_111);

    pub const fn new() -> Self {
        Self(0)
    }
    pub const fn lock_translation_x(self) -> Self {
        Self(self.0 | 0b000_001)
    }
    pub const fn lock_translation_y(self) -> Self {
        Self(self.0 | 0b000_010)
    }
    pub const fn lock_translation_z(self) -> Self {
        Self(self.0 | 0b000_100)
    }
    pub const fn lock_rotation_x(self) -> Self {
        Self(self.0 | 0b001_000)
    }
    pub const fn lock_rotation_y(self) -> Self {
        Self(self.0 | 0b010_000)
    }
    pub const fn lock_rotation_z(self) -> Self {
        Self(self.0 | 0b100_000)
    }

    /// Jolt's allowed degrees of freedom: the complement. Its bit order is the same (translation
    /// X, Y, Z, then rotation X, Y, Z).
    pub(super) fn allowed(self) -> oxijolt::AllowedDofs {
        let all = [
            oxijolt::AllowedDofs::TRANSLATION_X,
            oxijolt::AllowedDofs::TRANSLATION_Y,
            oxijolt::AllowedDofs::TRANSLATION_Z,
            oxijolt::AllowedDofs::ROTATION_X,
            oxijolt::AllowedDofs::ROTATION_Y,
            oxijolt::AllowedDofs::ROTATION_Z,
        ];
        all.into_iter()
            .enumerate()
            .filter(|&(i, _)| self.0 & (1 << i) == 0)
            .map(|(_, d)| d)
            .reduce(|a, d| a | d)
            // Everything locked: Jolt needs at least one degree of freedom.
            .unwrap_or(oxijolt::AllowedDofs::TRANSLATION_Y)
    }
}

#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component)]
pub struct LinearDamping(pub f32);

#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component)]
pub struct AngularDamping(pub f32);

#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct GravityScale(pub f32);

impl Default for GravityScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Avian's default friction is 0.5 (Jolt's 0.2); a body without the component gets that.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct Friction {
    pub dynamic_coefficient: f32,
    pub static_coefficient: f32,
}

impl Friction {
    pub const DEFAULT: Self = Self::new(0.5);

    pub const fn new(coefficient: f32) -> Self {
        Self { dynamic_coefficient: coefficient, static_coefficient: coefficient }
    }
}

impl Default for Friction {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Detects overlaps without pushing anything.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct Sensor;

/// Send `CollisionStart` for this collider's new contacts.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct CollisionEventsEnabled;

/// Ease `Transform` between physics ticks (`interpolation.rs`).
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
#[require(super::interpolation::TransformEasing)]
pub struct TransformInterpolation;

/// The colliders touching this one, kept up to date on entities that have the component (it is
/// never added for you, as in Avian).
#[derive(Component, Clone, Debug, Default, PartialEq, Deref, DerefMut)]
pub struct CollidingEntities(pub EntityHashSet);

/// Two colliders started touching; sent when either has `CollisionEventsEnabled`.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct CollisionStart {
    pub collider1: Entity,
    pub collider2: Entity,
    pub body1: Option<Entity>,
    pub body2: Option<Entity>,
}
