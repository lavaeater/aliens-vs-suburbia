//! Dismemberment: a skinned corpse comes apart into its own body parts instead of (or
//! as well as) the box gibs of [`crate::gore::gibs`].
//!
//! The frame a creature's health hits zero, before `health_monitor_system` despawns
//! it, [`dismember_the_dead`] freezes each of its skinned meshes in the pose it died in
//! (CPU skinning, [`cut::pose`]), groups the skeleton into parts at its forks and
//! halfway down its chains ([`cut::regions`]: head, upper/lower arms and legs,
//! torso...), and splits the triangles by the part they follow ([`cut::split`]). Each
//! part flies off as one physics body with the creature's own materials, its holes
//! capped in [`DismemberAssets::flesh`].
//!
//! [`GibStyle`] picks limbs, boxes or both (**Ctrl+K** cycles it). With limbs only, a
//! corpse that has no skinned mesh still bursts into boxes.

pub mod cut;

use std::collections::{HashMap, HashSet};

use avian3d::prelude::{AngularVelocity, Collider, CollisionLayers, LinearVelocity, RigidBody};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::general::components::{CollisionLayer, Health, Indestructible};
use crate::gore::components::{DamageKind, Ephemeral, GoreBudget, LastHit};
use crate::player::components::{IsObstacle, Player};
use cut::{JointNode, Piece, Skin};

/// How long body parts lie around before they vanish.
const PART_LIFETIME: f32 = 8.0;
/// Most points a part's convex-hull collider is built from.
const HULL_POINTS: usize = 128;

/// What a kill bursts into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GibStyle {
    /// The creature's own body parts (boxes for a corpse with no skinned mesh).
    #[default]
    Limbs,
    /// Dark-red cuboids, the original gibs.
    Boxes,
    /// Body parts and boxes.
    Both,
}

impl GibStyle {
    pub const fn next(self) -> Self {
        match self {
            Self::Limbs => Self::Boxes,
            Self::Boxes => Self::Both,
            Self::Both => Self::Limbs,
        }
    }

    pub const fn limbs(self) -> bool {
        matches!(self, Self::Limbs | Self::Both)
    }

    /// Boxes for this corpse, given whether it came apart into limbs.
    pub const fn boxes(self, dismembered: bool) -> bool {
        match self {
            Self::Limbs => !dismembered,
            Self::Boxes | Self::Both => true,
        }
    }
}

#[derive(Resource, Default)]
pub struct GibSettings {
    pub style: GibStyle,
}

/// Corpses that came apart into limbs, so the box gibs (reading `EntityDied` later)
/// know to stay out of it. Each entry is taken out again by `spawn_gibs_on_death`.
#[derive(Resource, Default)]
pub struct Dismembered(pub HashSet<Entity>);

#[derive(Resource)]
pub struct DismemberAssets {
    /// The cut faces: raw meat, both sides drawn so a cap wound the wrong way still shows.
    flesh: Handle<StandardMaterial>,
}

pub fn setup_dismember_assets(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let flesh = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.04, 0.05),
        perceptual_roughness: 0.4,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(DismemberAssets { flesh });
}

/// `Ctrl+K`: next [`GibStyle`].
pub fn cycle_gib_style(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<GibSettings>) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if ctrl && keys.just_pressed(KeyCode::KeyK) {
        settings.style = settings.style.next();
        info!("gibs: {:?}", settings.style);
    }
}

/// One skinned primitive of the corpse, with everything [`cut`] needs.
struct Primitive<'a> {
    mesh: &'a Mesh,
    joints: &'a [Entity],
    bindposes: &'a [Mat4],
    material: Option<Handle<StandardMaterial>>,
}

/// The same filter as `health_monitor_system`: what dies there, comes apart here.
type Mortal = (
    Without<Player>,
    Without<Indestructible>,
    Without<IsObstacle>,
);

#[allow(clippy::too_many_arguments)]
pub fn dismember_the_dead(
    settings: Res<GibSettings>,
    dying: Query<(Entity, &Health, Option<&LastHit>), Mortal>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    skinned: Query<(
        &Mesh3d,
        &SkinnedMesh,
        &InheritedVisibility,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    globals: Query<&GlobalTransform>,
    bindposes: Res<Assets<SkinnedMeshInverseBindposes>>,
    assets: Option<Res<DismemberAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut dismembered: ResMut<Dismembered>,
    mut budget: ResMut<GoreBudget>,
    mut commands: Commands,
) {
    if !settings.style.limbs() {
        return;
    }
    let Some(assets) = assets else { return };

    for (corpse, health, last_hit) in &dying {
        if !health.is_dead() {
            continue;
        }
        let primitives: Vec<Primitive> = children
            .iter_descendants(corpse)
            .filter_map(|e| skinned.get(e).ok())
            .filter(|(_, _, visible, _)| visible.get())
            .filter_map(|(mesh, skin, _, material)| {
                Some(Primitive {
                    mesh: meshes.get(&mesh.0)?,
                    joints: &skin.joints,
                    bindposes: bindposes.get(&skin.inverse_bindposes)?,
                    material: material.map(|m| m.0.clone()),
                })
            })
            .collect();

        let parts = cut_up(&primitives, &parents, &globals);
        if parts.is_empty() {
            continue;
        }
        dismembered.0.insert(corpse);

        let hit = last_hit.copied().unwrap_or_default();
        let centre = parts.iter().map(|p| p.centre).sum::<Vec3>() / parts.len() as f32;
        let mut rng = crate::gore::gibs::Rng(corpse.to_bits() as u32 ^ 0x6d65_6174);
        let away = (hit.normal.normalize_or(Vec3::Y) + Vec3::Y * 0.6).normalize_or(Vec3::Y);
        let force = if hit.kind == DamageKind::Explosive {
            2.0
        } else {
            1.0
        };

        for part in parts {
            let outward = (part.centre - centre).normalize_or(Vec3::Y);
            let jitter = Vec3::new(rng.signed(), rng.unit(), rng.signed());
            let dir = (away * 0.7 + outward * 0.6 + jitter * 0.3).normalize_or(Vec3::Y);
            let speed = (2.5 + rng.unit() * 3.0) * force;
            let spin = Vec3::new(rng.signed(), rng.signed(), rng.signed()) * 8.0 * force;

            let body = commands
                .spawn((
                    Name::new("Body part"),
                    Transform::from_translation(part.centre),
                    Visibility::default(),
                    RigidBody::Dynamic,
                    hull(&part.points),
                    CollisionLayers::new(
                        [CollisionLayer::Ball],
                        [CollisionLayer::Floor, CollisionLayer::ImpassableAll],
                    ),
                    LinearVelocity(dir * speed),
                    AngularVelocity(spin),
                    Ephemeral::new(PART_LIFETIME).no_fade(),
                ))
                .id();
            for (mesh, material) in part.meshes {
                let mut child = commands.spawn((ChildOf(body), Mesh3d(meshes.add(mesh))));
                if let Some(material) = material {
                    child.insert(MeshMaterial3d(material));
                }
            }
            if let Some(cap) = part.cap {
                commands.spawn((
                    ChildOf(body),
                    Mesh3d(meshes.add(cap)),
                    MeshMaterial3d(assets.flesh.clone()),
                ));
            }
            if let Some(evicted) = budget.push_gib(body) {
                commands.entity(evicted).try_despawn();
            }
        }
    }
}

type Material = Option<Handle<StandardMaterial>>;

/// One body part, ready to spawn: meshes relative to its centre.
struct Part {
    centre: Vec3,
    meshes: Vec<(Mesh, Option<Handle<StandardMaterial>>)>,
    cap: Option<Mesh>,
    /// Hull points, relative to the centre.
    points: Vec<Vec3>,
}

/// Pose every primitive, group the joints they share into body parts and split them.
fn cut_up(
    primitives: &[Primitive],
    parents: &Query<&ChildOf>,
    globals: &Query<&GlobalTransform>,
) -> Vec<Part> {
    // Every joint of every primitive, numbered once.
    let mut index: HashMap<Entity, usize> = HashMap::new();
    let mut joints: Vec<Entity> = Vec::new();
    for joint in primitives.iter().flat_map(|p| p.joints) {
        index.entry(*joint).or_insert_with(|| {
            joints.push(*joint);
            joints.len() - 1
        });
    }

    let skins: Vec<(Skin, &Primitive)> = primitives
        .iter()
        .filter_map(|p| Some((read_skin(p.mesh)?, p)))
        .collect();
    if skins.is_empty() {
        return Vec::new();
    }

    let mut weight = vec![0.0f32; joints.len()];
    for (skin, primitive) in &skins {
        for (j, w) in skin.joints.iter().zip(skin.weights) {
            let joint = primitive
                .joints
                .get(usize::from(cut::dominant_joint(*j, *w)));
            if let Some(slot) = joint
                .and_then(|e| index.get(e))
                .and_then(|&i| weight.get_mut(i))
            {
                *slot += 1.0;
            }
        }
    }
    let nodes: Vec<JointNode> = joints
        .iter()
        .zip(&weight)
        .map(|(&joint, &weight)| JointNode {
            parent: parents
                .iter_ancestors(joint)
                .find_map(|ancestor| index.get(&ancestor).copied()),
            weight,
        })
        .collect();
    let region = cut::regions(&nodes, cut::MIN_SHARE);

    let mut by_region: HashMap<usize, Vec<(Piece, Material)>> = HashMap::new();
    for (skin, primitive) in &skins {
        let poses: Vec<Mat4> = primitive
            .joints
            .iter()
            .zip(primitive.bindposes)
            .map(|(joint, bind)| {
                globals
                    .get(*joint)
                    .map_or(Mat4::IDENTITY, |g| Mat4::from(g.affine()))
                    * *bind
            })
            .collect();
        let region_of: Vec<usize> = primitive
            .joints
            .iter()
            .map(|joint| {
                index
                    .get(joint)
                    .and_then(|&i| region.get(i))
                    .copied()
                    .unwrap_or(0)
            })
            .collect();
        let posed = cut::pose(skin, &poses);
        for piece in cut::split(skin, &posed, &region_of) {
            by_region
                .entry(piece.region)
                .or_default()
                .push((piece, primitive.material.clone()));
        }
    }

    by_region
        .into_values()
        .filter_map(|pieces| {
            let all: Vec<Vec3> = pieces
                .iter()
                .flat_map(|(p, _)| p.positions.iter().copied())
                .collect();
            if all.is_empty() {
                return None;
            }
            let centre = all.iter().copied().sum::<Vec3>() / all.len() as f32;
            let stride = all.len().div_ceil(HULL_POINTS).max(1);
            let points = all.iter().step_by(stride).map(|p| *p - centre).collect();
            let cap: Vec<[Vec3; 3]> = pieces
                .iter()
                .flat_map(|(p, _)| p.cap.iter().copied())
                .collect();
            Some(Part {
                centre,
                meshes: pieces
                    .into_iter()
                    .map(|(piece, material)| (piece_mesh(piece, centre), material))
                    .collect(),
                cap: (!cap.is_empty()).then(|| cap_mesh(&cap, centre)),
                points,
            })
        })
        .collect()
}

/// The attributes [`cut`] works on, if the mesh has them in the layout glTF gives.
fn read_skin(mesh: &Mesh) -> Option<Skin<'_>> {
    let attribute = |id| mesh.try_attribute_option(id).ok().flatten();
    let VertexAttributeValues::Float32x3(positions) = attribute(Mesh::ATTRIBUTE_POSITION)? else {
        return None;
    };
    let VertexAttributeValues::Uint16x4(joints) = attribute(Mesh::ATTRIBUTE_JOINT_INDEX)? else {
        return None;
    };
    let VertexAttributeValues::Float32x4(weights) = attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)? else {
        return None;
    };
    let normals = match attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => Some(v.as_slice()),
        _ => None,
    };
    let uvs = match attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(v)) => Some(v.as_slice()),
        _ => None,
    };
    let tangents = match attribute(Mesh::ATTRIBUTE_TANGENT) {
        Some(VertexAttributeValues::Float32x4(v)) => Some(v.as_slice()),
        _ => None,
    };
    let colors = match attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(v)) => Some(v.as_slice()),
        _ => None,
    };
    let indices = match mesh.try_indices_option().ok().flatten() {
        Some(indices) => indices.iter().map(|i| i as u32).collect(),
        None => (0..positions.len() as u32).collect(),
    };
    Some(Skin {
        positions,
        normals,
        uvs,
        tangents,
        colors,
        joints,
        weights,
        indices,
    })
}

fn new_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn piece_mesh(piece: Piece, centre: Vec3) -> Mesh {
    let positions: Vec<[f32; 3]> = piece
        .positions
        .iter()
        .map(|p| (*p - centre).to_array())
        .collect();
    let mut mesh = new_mesh().with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if let Some(normals) = piece.normals {
        let normals: Vec<[f32; 3]> = normals.iter().map(Vec3::to_array).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    }
    if let Some(uvs) = piece.uvs {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
    if let Some(tangents) = piece.tangents {
        let tangents: Vec<[f32; 4]> = tangents.iter().map(Vec4::to_array).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    }
    if let Some(colors) = piece.colors {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    }
    mesh.insert_indices(Indices::U32(piece.indices));
    if mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_none() {
        mesh.compute_normals();
    }
    mesh
}

fn cap_mesh(cap: &[[Vec3; 3]], centre: Vec3) -> Mesh {
    let positions: Vec<[f32; 3]> = cap
        .iter()
        .flatten()
        .map(|p| (*p - centre).to_array())
        .collect();
    let uvs = vec![[0.0f32, 0.0]; positions.len()];
    let mut mesh = new_mesh()
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.compute_flat_normals();
    mesh
}

/// A convex hull around the part, or a small box if the points are degenerate.
fn hull(points: &[Vec3]) -> Collider {
    Collider::convex_hull(points.to_vec()).unwrap_or_else(|| {
        let (min, max) = points.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let size = (max - min).max(Vec3::splat(0.05));
        Collider::cuboid(size.x, size.y, size.z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limbs_fall_back_to_boxes_for_a_corpse_with_no_skin() {
        assert!(GibStyle::Limbs.boxes(false));
        assert!(!GibStyle::Limbs.boxes(true));
        assert!(GibStyle::Both.boxes(true));
        assert!(!GibStyle::Boxes.limbs());
    }

    #[test]
    fn the_style_cycles_through_all_three() {
        let s = GibStyle::default();
        assert_eq!(s.next().next().next(), s);
    }
}
