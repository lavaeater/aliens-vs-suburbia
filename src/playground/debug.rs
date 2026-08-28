//! Debug overlays for the live character: physics colliders, skeleton, hardpoint frames.
//!
//! The skeleton and hardpoint drawing is shared with the asset browser (see
//! `assets::gizmos`); what differs here is *which* joints to draw. The browser has one
//! model in an empty scene and can take every skinned mesh in the world. The playground has
//! the player plus three skinned dummies, so the overlay is scoped to the player's subtree.
//!
//! Physics debug is avian's own `PhysicsGizmos` config, the same one `F3` toggles globally.
//! The panel button and the key stay in sync because the button reads and writes that
//! config rather than keeping a copy of the answer.

use avian3d::prelude::PhysicsGizmos;
use bevy::gizmos::config::{DefaultGizmoConfigGroup, GizmoConfigStore};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;

use crate::assets::gizmos::{bone_map, draw_hardpoints, draw_skeleton, joints_under};
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::Player;
use crate::player::systems::gait::Foot;
use crate::player::systems::leg_ik::Legs;
use crate::player::systems::equip::EquippedWeapon;
use crate::playground::hardpoints::PlaygroundWeaponDef;

#[derive(Resource, Default)]
pub struct PlaygroundDebug {
    pub skeleton: bool,
    pub hardpoints: bool,
    /// Mirror of avian's `PhysicsGizmos::enabled`, so the panel can show its state. Kept
    /// current by [`sync_physics_toggle`] — `F3` can flip it behind our back.
    pub physics: bool,
    /// The walk's footfalls: where each foot is going, where it is nailed, and how far the
    /// legs can reach.
    pub gait: bool,
    pub ui_dirty: bool,
}

impl PlaygroundDebug {
    pub fn toggle_skeleton(&mut self) {
        self.skeleton = !self.skeleton;
        self.ui_dirty = true;
    }

    pub fn toggle_hardpoints(&mut self) {
        self.hardpoints = !self.hardpoints;
        self.ui_dirty = true;
    }

    pub fn toggle_gait(&mut self) {
        self.gait = !self.gait;
        self.ui_dirty = true;
    }
}

/// Left is orange, right is blue, everywhere and always.
///
/// The same two colours the whole way through, because the first thing this overlay is
/// asked is "which foot is that?" — and a character walking with its legs crossed looks
/// exactly like one walking normally until you can answer it.
const LEFT_COLOR: Color = Color::srgb(1.0, 0.55, 0.15);
const RIGHT_COLOR: Color = Color::srgb(0.25, 0.7, 1.0);

fn foot_color(foot: Foot) -> Color {
    match foot {
        Foot::Left => LEFT_COLOR,
        Foot::Right => RIGHT_COLOR,
    }
}

/// Draw the gait: the reach the legs have, the line each foot walks along, where it is
/// supposed to touch down and leave the ground, and where it is right now.
pub fn draw_gait_gizmos(debug: Res<PlaygroundDebug>, mut gizmos: Gizmos, players: Query<&Legs>) {
    if !debug.gait {
        return;
    }
    let flat = Quat::from_rotation_arc(Vec3::Z, Vec3::Y);

    for legs in players.iter() {
        let f = &legs.frame;
        if f.forward.length_squared() < 0.5 {
            continue; // never driven yet
        }

        // How far a foot can get from under the hips and still touch the floor. Every
        // marker outside this ring is a foot the legs cannot actually reach.
        gizmos.circle(
            Isometry3d::new(f.hip_ground, flat),
            f.reach,
            Color::srgb(0.55, 0.5, 0.2),
        );
        // The hips themselves, and the centreline the feet are placed either side of.
        gizmos.line(f.hip_ground, f.hip_ground + Vec3::Y * 0.25, Color::srgb(0.6, 0.6, 0.6));

        for foot in Foot::BOTH {
            let i = foot.index();
            let color = foot_color(foot);
            let side = f.right * (foot.lateral_sign() * f.stance_width * 0.5);
            let touchdown = f.hip_ground + side + f.forward * f.touchdown;
            let liftoff = f.hip_ground + side + f.forward * f.liftoff;

            // The stance: the foot lands at one end and leaves the ground at the other,
            // having stood still while the body travelled the length of it.
            gizmos.line(liftoff, touchdown, color.with_alpha(0.5));
            cross(&mut gizmos, touchdown, 0.04, color);
            cross(&mut gizmos, liftoff, 0.025, color.with_alpha(0.4));

            // Where the foot actually is, and what it is standing on.
            let target = f.targets[i];
            gizmos.sphere(Isometry3d::from_translation(target), 0.02, color);
            if f.swinging[i] {
                // In the air: show the footprint it left and the one it is heading for.
                gizmos.line(f.plants[i], target, color.with_alpha(0.35));
                cross(&mut gizmos, f.plants[i], 0.02, color.with_alpha(0.3));
            } else {
                gizmos.line(target, target + Vec3::Y * 0.06, color);
            }
        }
    }
}

/// A flat cross on the ground, which reads at any camera angle where a sphere does not.
fn cross(gizmos: &mut Gizmos, at: Vec3, size: f32, color: Color) {
    gizmos.line(at - Vec3::X * size, at + Vec3::X * size, color);
    gizmos.line(at - Vec3::Z * size, at + Vec3::Z * size, color);
}

/// Flip avian's collider gizmos. Writes the config directly rather than a local flag, so
/// this and `F3` cannot disagree.
pub fn toggle_physics_gizmos(store: &mut GizmoConfigStore) {
    let (config, _) = store.config_mut::<PhysicsGizmos>();
    config.enabled = !config.enabled;
}

pub fn physics_gizmos_enabled(store: &GizmoConfigStore) -> bool {
    store.config::<PhysicsGizmos>().0.enabled
}

/// Pick up `F3` presses (or anything else touching the config) so the panel label matches
/// what is actually on screen.
pub fn sync_physics_toggle(mut debug: ResMut<PlaygroundDebug>, store: Res<GizmoConfigStore>) {
    let enabled = physics_gizmos_enabled(&store);
    if debug.physics != enabled {
        debug.physics = enabled;
        debug.ui_dirty = true;
    }
}

/// Draw the skeleton over the mesh rather than inside it. The asset browser does the same
/// on entry; both restore it on the way out so other states' gizmos are unaffected.
pub fn bias_gizmos_over_mesh(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;
    config.line.width = 2.0;
}

pub fn reset_gizmo_bias(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = 0.0;
}

/// Skeleton and hardpoint overlays for every player in the arena.
#[allow(clippy::too_many_arguments)]
pub fn draw_player_overlays(
    debug: Res<PlaygroundDebug>,
    player_def: Res<PlayerAssetDef>,
    weapon_def: Res<PlaygroundWeaponDef>,
    mut gizmos: Gizmos,
    players: Query<(Entity, Option<&EquippedWeapon>), With<Player>>,
    children_q: Query<&Children>,
    skinned_q: Query<&SkinnedMesh>,
    transforms: Query<&GlobalTransform>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
) {
    if !debug.skeleton && !debug.hardpoints {
        return;
    }

    for (player, equipped) in players.iter() {
        // The weapon hangs off a bone, so its frames are drawable whether or not the
        // character's own skeleton resolved.
        if debug.hardpoints
            && let Some(equipped) = equipped
            && let Some(def) = weapon_def.def.as_ref()
        {
            // Weapon hardpoints are authored against the model origin (`anchor: None`),
            // which for a spawned weapon is the weapon entity itself.
            draw_hardpoints(&mut gizmos, &def.hardpoints, None, &transforms, |anchor| {
                anchor.is_none().then_some(equipped.0)
            });
        }

        let joints = joints_under(player, &children_q, &skinned_q);
        if joints.is_empty() {
            continue;
        }

        if debug.skeleton {
            draw_skeleton(&mut gizmos, &joints, None, &transforms, &names, &parents);
        }

        if debug.hardpoints
            && let Some(def) = player_def.0.as_ref()
            && !def.hardpoints.is_empty()
        {
            let bones = bone_map(&joints, &names);
            draw_hardpoints(&mut gizmos, &def.hardpoints, None, &transforms, |anchor| {
                match anchor {
                    Some(bone) => bones.get(bone).copied(),
                    // A hardpoint with no anchor is relative to the model origin, which
                    // for a spawned character is the player entity itself.
                    None => Some(player),
                }
            });
        }
    }
}
