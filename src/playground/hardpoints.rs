//! Editing hardpoints on the live character.
//!
//! The asset browser authors hardpoints against a static model in an empty scene. This
//! does it against the character you are actually walking around as, mid-animation, with
//! the weapon in hand — which is the only way to tell whether a grip looks right while the
//! arm is moving.
//!
//! The edited value lives in `PlayerAssetDef`, the def resource `spawn_players` already
//! populates. Two things then follow for free:
//!
//! - the hardpoint overlay (`playground::debug`) reads that resource, so the gizmo moves
//!   as you nudge;
//! - `keep_weapons_snapped` re-derives the weapon's transform every frame from
//!   `WeaponModel::char_grip`, so pushing the edited grip into that component is all it
//!   takes for the held weapon to follow.
//!
//! Nothing is written to disk until you press Save.

use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, Hardpoint};
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::Player;
use crate::player::systems::equip::{EquippedWeapon, WeaponModel, GRIP_ROLE};

/// Roles offered in the panel. Characters really only use `grip`; the rest are here
/// because a def may carry them and they should be visible and editable if so.
pub const ROLES: [&str; 4] = ["grip", "foregrip", "stock", "sight"];

/// Metres per nudge. Fine is what you use once the grip is roughly right.
pub const COARSE_TRANSLATION: f32 = 0.02;
pub const FINE_TRANSLATION: f32 = 0.002;

/// Degrees per nudge.
pub const COARSE_ROTATION: f32 = 15.0;
pub const FINE_ROTATION: f32 = 1.0;

#[derive(Resource, Default)]
pub struct HardpointEditor {
    /// Role being edited, if any.
    pub active_role: Option<String>,
    /// Set when the def has unsaved edits, so the Save button can say so.
    pub dirty: bool,
    pub ui_dirty: bool,
    pub status: String,
}

impl HardpointEditor {
    pub fn select_role(&mut self, role: &str) {
        self.active_role = Some(role.to_string());
        self.ui_dirty = true;
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.ui_dirty = true;
    }
}

/// Fetch (creating if absent) the hardpoint for `role`.
///
/// A new role inherits the anchor of an existing one, because on a character every
/// hardpoint is anchored to some bone and guessing the model origin instead would put the
/// gizmo at the character's feet, far from anything.
pub fn ensure_role<'a>(def: &'a mut AssetDefinition, role: &str) -> &'a mut Hardpoint {
    if !def.hardpoints.contains_key(role) {
        let inherited = def.hardpoints.values().find_map(|hp| hp.anchor.clone());
        def.hardpoints
            .insert(role.to_string(), Hardpoint { anchor: inherited, ..Default::default() });
    }
    def.hardpoints.get_mut(role).expect("just inserted")
}

/// Nudge one translation axis. `axis` is 0/1/2 for x/y/z.
pub fn nudge_translation(hardpoint: &mut Hardpoint, axis: usize, delta: f32) {
    if let Some(value) = hardpoint.translation.get_mut(axis) {
        *value += delta;
    }
}

/// Nudge one rotation axis, keeping the stored degrees in (-180, 180].
///
/// Without the wrap the numbers grow without bound as you spin a hardpoint round, which
/// looks identical on screen but reads as nonsense in the saved `.ron`.
pub fn nudge_rotation(hardpoint: &mut Hardpoint, axis: usize, delta_degrees: f32) {
    if let Some(value) = hardpoint.rotation_euler_deg.get_mut(axis) {
        *value = wrap_degrees(*value + delta_degrees);
    }
}

pub fn wrap_degrees(degrees: f32) -> f32 {
    let wrapped = (degrees + 180.0).rem_euclid(360.0) - 180.0;
    // rem_euclid maps exactly -180 onto -180; prefer +180 so a half-turn reads naturally.
    if wrapped == -180.0 { 180.0 } else { wrapped }
}

/// Push the edited `grip` into the live weapon so it moves with the gizmo.
///
/// Only runs when the def actually changed. `keep_weapons_snapped` does the rest every
/// frame from the component this writes.
pub fn apply_grip_to_equipped_weapon(
    player_def: Res<PlayerAssetDef>,
    players: Query<&EquippedWeapon, With<Player>>,
    mut weapons: Query<&mut WeaponModel>,
) {
    if !player_def.is_changed() {
        return;
    }
    let Some(def) = player_def.0.as_ref() else { return };
    let Some(grip) = def.hardpoints.get(GRIP_ROLE) else { return };

    for equipped in players.iter() {
        if let Ok(mut weapon) = weapons.get_mut(equipped.0) {
            weapon.set_char_grip(grip.clone());
        }
    }
}

/// Write the edited def back to `assets/defs/<stem>.ron`.
pub fn save_player_def(def: &AssetDefinition) -> String {
    def.save();
    let path = AssetDefinition::def_path(&def.model_path);
    if path.exists() {
        format!("saved {}", path.display())
    } else {
        format!("FAILED to write {}", path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::AssetDefinition;

    fn def_with_grip_on(bone: &str) -> AssetDefinition {
        let mut def = AssetDefinition::default();
        def.hardpoints.insert(
            "grip".to_string(),
            Hardpoint { anchor: Some(bone.to_string()), ..Default::default() },
        );
        def
    }

    /// A new role anchored to the model origin would put the gizmo at the character's
    /// feet, which reads as a bug rather than a starting point.
    #[test]
    fn a_new_role_inherits_an_existing_anchor_bone() {
        let mut def = def_with_grip_on("mixamorigRightHand");
        let created = ensure_role(&mut def, "sight");
        assert_eq!(created.anchor.as_deref(), Some("mixamorigRightHand"));
    }

    #[test]
    fn selecting_an_existing_role_does_not_reset_it() {
        let mut def = def_with_grip_on("Hand");
        def.hardpoints.get_mut("grip").unwrap().translation = [1.0, 2.0, 3.0];
        let grip = ensure_role(&mut def, "grip");
        assert_eq!(grip.translation, [1.0, 2.0, 3.0], "existing values survive");
    }

    #[test]
    fn nudging_moves_only_the_asked_for_axis() {
        let mut hardpoint = Hardpoint::default();
        nudge_translation(&mut hardpoint, 1, 0.02);
        assert_eq!(hardpoint.translation, [0.0, 0.02, 0.0]);
    }

    #[test]
    fn an_out_of_range_axis_is_ignored_rather_than_panicking() {
        let mut hardpoint = Hardpoint::default();
        nudge_translation(&mut hardpoint, 7, 0.02);
        nudge_rotation(&mut hardpoint, 7, 15.0);
        assert_eq!(hardpoint.translation, [0.0; 3]);
        assert_eq!(hardpoint.rotation_euler_deg, [0.0; 3]);
    }

    /// Spinning a hardpoint all the way round should read as a sane angle in the saved
    /// file, not as an ever-growing number.
    #[test]
    fn rotation_stays_within_half_a_turn_either_way() {
        let mut hardpoint = Hardpoint::default();
        for _ in 0..30 {
            nudge_rotation(&mut hardpoint, 0, 15.0);
        }
        assert!(
            hardpoint.rotation_euler_deg[0].abs() <= 180.0,
            "got {}",
            hardpoint.rotation_euler_deg[0]
        );
    }

    #[test]
    fn wrapping_keeps_the_angle_it_is_given_where_it_can() {
        assert_eq!(wrap_degrees(0.0), 0.0);
        assert_eq!(wrap_degrees(90.0), 90.0);
        assert_eq!(wrap_degrees(-90.0), -90.0);
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(wrap_degrees(180.0), 180.0, "a half turn reads as +180, not -180");
    }
}
