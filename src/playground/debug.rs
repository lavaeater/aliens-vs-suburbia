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
use crate::player::systems::equip::EquippedWeapon;
use crate::playground::hardpoints::PlaygroundWeaponDef;

#[derive(Resource, Default)]
pub struct PlaygroundDebug {
    pub skeleton: bool,
    pub hardpoints: bool,
    /// Mirror of avian's `PhysicsGizmos::enabled`, so the panel can show its state. Kept
    /// current by [`sync_physics_toggle`] — `F3` can flip it behind our back.
    pub physics: bool,
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
