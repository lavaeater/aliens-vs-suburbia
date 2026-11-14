//! The copter, the ropes and the team on them.
//!
//! Until a model is given (`CopterLook::model` in the campaign's `intro`), the copter is
//! a stand-in made of primitives: fuselage, tail boom, fin, skids and two rotors, nose
//! along -Z. A model gets its rotors from `CopterLook::rotors`: node names spun about an
//! axis. Either way it carries a searchlight and a blinking beacon.
//!
//! The team goes down as stand-ins: each player's own model, without physics, eased down
//! a rope (`FlightPlan::rappeller_at`); the real players spawn where they land.

use bevy::gltf::{Gltf, GltfAssetLabel};
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;
use serde::{Deserialize, Serialize};

use super::plan::FlightPlan;

/// What the copter looks like (the campaign file's `intro.copter`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CopterLook {
    /// A `.glb` inside `assets/`, nose along -Z (or turned by `turn_degrees`). Empty = the
    /// built-in stand-in.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    pub scale: f32,
    /// Turns the model about up, for one whose nose is not along -Z.
    pub turn_degrees: f32,
    /// Nodes of the model to spin.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rotors: Vec<RotorSpec>,
}

impl Default for CopterLook {
    fn default() -> Self {
        Self {
            model: String::new(),
            scale: 1.0,
            turn_degrees: 0.0,
            rotors: Vec::new(),
        }
    }
}

/// A node of the copter model that spins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RotorSpec {
    pub node: String,
    /// The node's own axis to spin about.
    #[serde(default = "up")]
    pub axis: Vec3,
    #[serde(default = "rotor_rpm")]
    pub rpm: f32,
}

fn up() -> Vec3 {
    Vec3::Y
}

const fn rotor_rpm() -> f32 {
    380.0
}

/// The copter's root; its transform follows the flight plan.
#[derive(Component)]
pub struct Copter {
    /// Model rotors not yet found in the spawned scene.
    pending_rotors: Vec<RotorSpec>,
}

/// Spins about its own `axis` at `rpm`.
#[derive(Component)]
pub struct Rotor {
    axis: Vec3,
    rpm: f32,
}

/// Blinks on and off.
#[derive(Component)]
pub struct Beacon {
    period: f32,
    on: f32,
}

/// Player `index` of `count` going down the rope, as the model the player wears.
#[derive(Component)]
pub struct Rappeller {
    pub index: usize,
    pub count: usize,
    /// The model's file, for its animations.
    pub gltf: Handle<Gltf>,
}

/// The rope player `index` goes down.
#[derive(Component)]
pub struct Rope {
    pub index: usize,
    pub count: usize,
}

/// Lumens: the beam lights the ground from the hover, 13 m up, and more from the low pass.
const SEARCHLIGHT: f32 = 4_500_000.0;
const BEACON: f32 = 9_000.0;
/// How long a rope takes to wind back up once its player is down.
const REEL_IN: f32 = 1.6;

#[allow(clippy::too_many_arguments)]
pub fn spawn_copter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    look: &CopterLook,
    at: Transform,
) -> Entity {
    let copter = commands
        .spawn((
            Name::new("Intro copter"),
            Copter {
                pending_rotors: if look.model.is_empty() {
                    Vec::new()
                } else {
                    look.rotors.clone()
                },
            },
            at,
            Visibility::default(),
        ))
        .id();

    if look.model.is_empty() {
        stand_in(commands, meshes, materials, copter);
    } else {
        let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(look.model.clone()));
        commands.spawn((
            Name::new("Copter model"),
            WorldAssetRoot(scene),
            Transform::from_rotation(Quat::from_rotation_y(look.turn_degrees.to_radians()))
                .with_scale(Vec3::splat(look.scale.max(0.001))),
            ChildOf(copter),
        ));
    }

    // Searchlight under the nose, aimed ahead and down.
    commands.spawn((
        Name::new("Copter searchlight"),
        SpotLight {
            color: Color::srgb(1.0, 0.96, 0.85),
            intensity: SEARCHLIGHT,
            range: 160.0,
            outer_angle: 0.28,
            inner_angle: 0.18,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, -1.0, -1.8).looking_to(Vec3::new(0.0, -1.0, -0.9), Vec3::Y),
        ChildOf(copter),
    ));
    // A red beacon on the tail.
    commands.spawn((
        Name::new("Copter beacon"),
        PointLight {
            color: Color::srgb(1.0, 0.1, 0.05),
            intensity: BEACON,
            range: 12.0,
            ..default()
        },
        Beacon {
            period: 1.1,
            on: 0.15,
        },
        Transform::from_xyz(0.0, 1.5, 5.6),
        ChildOf(copter),
    ));
    copter
}

/// The built-in copter, nose along -Z.
fn stand_in(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    copter: Entity,
) {
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.26, 0.17),
        perceptual_roughness: 0.75,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.07, 0.07, 0.08),
        perceptual_roughness: 0.6,
        ..default()
    });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.16, 0.22),
        emissive: LinearRgba::rgb(0.05, 0.12, 0.1),
        perceptual_roughness: 0.15,
        ..default()
    });
    let along_z = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let mut part_of = |parent: Entity,
                       name: &str,
                       mesh: Mesh,
                       material: &Handle<StandardMaterial>,
                       at: Transform| {
        commands
            .spawn((
                Name::new(name.to_string()),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                at,
                ChildOf(parent),
            ))
            .id()
    };
    let mut part = |name: &str, mesh: Mesh, material: &Handle<StandardMaterial>, at: Transform| {
        part_of(copter, name, mesh, material, at)
    };
    part(
        "Fuselage",
        Capsule3d::new(0.95, 2.4).into(),
        &body,
        Transform::from_rotation(along_z),
    );
    part(
        "Canopy",
        Sphere::new(0.8).into(),
        &glass,
        Transform::from_xyz(0.0, 0.2, -1.45).with_scale(Vec3::new(1.0, 0.85, 1.1)),
    );
    part(
        "Tail boom",
        Cylinder::new(0.2, 4.6).into(),
        &body,
        Transform::from_xyz(0.0, 0.35, 3.6).with_rotation(along_z),
    );
    part(
        "Fin",
        Cuboid::new(0.1, 1.2, 0.8).into(),
        &body,
        Transform::from_xyz(0.0, 0.9, 5.7),
    );
    part(
        "Mast",
        Cylinder::new(0.09, 0.5).into(),
        &dark,
        Transform::from_xyz(0.0, 1.15, 0.0),
    );
    for side in [-1.0, 1.0] {
        part(
            "Skid",
            Cuboid::new(0.09, 0.09, 3.6).into(),
            &dark,
            Transform::from_xyz(0.85 * side, -1.3, 0.1),
        );
        for z in [-0.8, 0.9] {
            part(
                "Strut",
                Cuboid::new(0.07, 0.6, 0.07).into(),
                &dark,
                Transform::from_xyz(0.7 * side, -1.0, z).with_rotation(Quat::from_rotation_z(0.3 * side)),
            );
        }
    }
    let main = part(
        "Main rotor",
        Cuboid::new(10.0, 0.04, 0.28).into(),
        &dark,
        Transform::from_xyz(0.0, 1.42, 0.0),
    );
    let tail = part(
        "Tail rotor",
        Cuboid::new(0.05, 1.5, 0.14).into(),
        &dark,
        Transform::from_xyz(0.16, 0.95, 5.8),
    );
    part_of(
        main,
        "Main rotor, other blade",
        Cuboid::new(0.28, 0.04, 10.0).into(),
        &dark,
        Transform::default(),
    );
    commands.entity(main).insert(Rotor {
        axis: Vec3::Y,
        rpm: 300.0,
    });
    commands.entity(tail).insert(Rotor {
        axis: Vec3::X,
        rpm: 1400.0,
    });
}

/// Hand a model's named rotor nodes their [`Rotor`] once its scene is in.
pub fn find_model_rotors(
    mut commands: Commands,
    mut copters: Query<(Entity, &mut Copter)>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (copter, mut spec) in &mut copters {
        if spec.pending_rotors.is_empty() {
            continue;
        }
        let mut found = Vec::new();
        for node in children.iter_descendants(copter) {
            let Ok(name) = names.get(node) else {
                continue;
            };
            if let Some(rotor) = spec.pending_rotors.iter().find(|r| r.node == name.as_str()) {
                commands.entity(node).insert(Rotor {
                    axis: rotor.axis.normalize_or(Vec3::Y),
                    rpm: rotor.rpm,
                });
                found.push(rotor.node.clone());
            }
        }
        spec.pending_rotors.retain(|r| !found.contains(&r.node));
    }
}

pub fn spin_rotors(time: Res<Time>, mut rotors: Query<(&Rotor, &mut Transform)>) {
    for (rotor, mut transform) in &mut rotors {
        let turn = rotor.rpm / 60.0 * std::f32::consts::TAU * time.delta_secs();
        transform.rotate_local(Quat::from_axis_angle(rotor.axis, turn));
    }
}

pub fn blink_beacons(time: Res<Time>, mut beacons: Query<(&Beacon, &mut PointLight)>) {
    for (beacon, mut light) in &mut beacons {
        let phase = time.elapsed_secs() % beacon.period.max(0.01);
        light.intensity = if phase < beacon.on { BEACON } else { 0.0 };
    }
}

/// Where rope `index` hangs from at `t`, and where its lower end is: in its player's
/// hands (`held`, their body on the rope) until they are down, then hanging straight and
/// winding up.
pub fn rope_ends(plan: &FlightPlan, index: usize, t: f32, held: Vec3, down_at: Option<f32>) -> (Vec3, Vec3) {
    let top = plan.hatch(index, t);
    let Some(down_at) = down_at else {
        return (top, held + Vec3::Y * 0.6);
    };
    let since = (t - down_at).max(0.0);
    let full = (top.y - held.y).max(0.0);
    let length = full * (1.0 - since / REEL_IN).max(0.0);
    (top, top - Vec3::Y * length)
}

/// A rope from `top` to `bottom`, as the transform of a unit cylinder.
pub fn rope_transform(top: Vec3, bottom: Vec3) -> Transform {
    let span = top - bottom;
    let length = span.length().max(0.001);
    Transform {
        translation: (top + bottom) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, span / length),
        scale: Vec3::new(1.0, length, 1.0),
    }
}

/// Give each stand-in a looping clip from its own model once its scene is in: a fall or
/// a jump if it has one, else its idle.
pub fn animate_rappellers(
    mut commands: Commands,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    rappellers: Query<&Rappeller>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    for (entity, mut player) in &mut players {
        let Some(rappeller) = parents
            .iter_ancestors(entity)
            .find_map(|a| rappellers.get(a).ok())
        else {
            continue;
        };
        let Some(gltf) = gltfs.get(&rappeller.gltf) else {
            continue;
        };
        let pick = ["fall", "jump", "idle"].iter().find_map(|want| {
            gltf.named_animations
                .iter()
                .find(|(name, _)| name.to_lowercase().contains(want))
                .map(|(_, clip)| clip.clone())
        });
        let Some(clip) = pick.or_else(|| gltf.animations.first().cloned()) else {
            continue;
        };
        let (graph, node) = AnimationGraph::from_clip(clip);
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(graphs.add(graph)));
        player.play(node).repeat();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intro::plan::{CloudDeck, IntroTiming};

    fn plan() -> FlightPlan {
        FlightPlan::new(
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::ZERO,
            30.0,
            IntroTiming::default(),
            CloudDeck::default(),
            |_, _| 0.0,
        )
    }

    #[test]
    fn a_rope_runs_from_the_hatch_to_its_player_then_winds_up() {
        let p = plan();
        let t = p.timing.hover_at() + 1.0;
        let (held, _) = p.rappeller_at(0, 2, t, p.landing);
        let (top, bottom) = rope_ends(&p, 0, t, held, None);
        assert!(top.y > bottom.y);
        assert!((bottom - (held + Vec3::Y * 0.6)).length() < 1e-4);
        let landed = p.timing.landed_at();
        let (top, bottom) = rope_ends(&p, 0, landed + REEL_IN + 0.1, p.landing, Some(landed));
        assert!(top.distance(bottom) < 1e-3);
    }

    #[test]
    fn the_rope_transform_spans_its_ends() {
        let (top, bottom) = (Vec3::new(1.0, 10.0, 2.0), Vec3::new(1.5, 2.0, 2.0));
        let t = rope_transform(top, bottom);
        let up = t.transform_point(Vec3::Y * 0.5);
        let down = t.transform_point(-Vec3::Y * 0.5);
        assert!(up.distance(top) < 1e-4 && down.distance(bottom) < 1e-4);
    }
}
