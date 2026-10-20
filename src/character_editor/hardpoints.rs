//! Editing hardpoints on the live character.
//!
//! The asset browser authors hardpoints against a static model in an empty scene. This
//! does it against the character you are actually walking around as, mid-animation, with
//! the weapon in hand — which is the only way to tell whether a grip looks right while the
//! arm is moving.
//!
//! The panel edits the docs (`EditorDoc` for the character, `WeaponDoc` for its gun);
//! `doc::sync_doc_to_live` copies the character's into `PlayerAssetDef`. Two things then
//! follow for free:
//!
//! - the hardpoint overlay (`character_editor::debug`) reads those, so the gizmo moves as
//!   you nudge;
//! - `keep_weapons_snapped` re-derives the weapon's transform every frame from
//!   `WeaponModel::char_grip`, so pushing the edited grip into that component is all it
//!   takes for the held weapon to follow.
//!
//! Nothing is written to disk until you Save, from the doc bar.

use bevy::prelude::*;

use crate::assets::asset_definition::{AssetDefinition, Hardpoint};
use crate::character_editor::doc::{Subject, WeaponDoc};
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::Player;
use crate::player::systems::equip::{EquippedWeapon, GRIP_ROLE, MUZZLE_ROLE, WeaponModel};
use crate::player::systems::shoot::Weapon;

/// Roles offered in the panel. Characters really only use `grip`; the rest are here
/// because a def may carry them and they should be visible and editable if so.
pub const ROLES: [&str; 4] = ["grip", "foregrip", "stock", "sight"];

/// Roles offered on the weapon. `muzzle` is weapon-only — it is where bullets leave, and
/// has no meaning on a character.
pub const WEAPON_ROLES: [&str; 5] = ["grip", "muzzle", "foregrip", "stock", "sight"];

/// Which def the panel is editing.
///
/// A gun sitting wrong in the hand can be fixed from either end, and the two are not
/// interchangeable: the character's `grip` is where the hand holds things (shared by every
/// weapon it picks up), the weapon's `grip` is where *this* gun wants to be held. Editing
/// the wrong one fixes the symptom for one pairing and breaks the rest.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HardpointSide {
    #[default]
    Character,
    Weapon,
    /// A model opened on its own (an enemy, a tower): its hardpoints, anchored to its own
    /// bones. Every role is offered — an enemy with a gun has a grip and a muzzle.
    Prop,
}

impl HardpointSide {
    pub const fn roles(self) -> &'static [&'static str] {
        match self {
            Self::Character => &ROLES,
            Self::Weapon | Self::Prop => &WEAPON_ROLES,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Weapon => "weapon",
            Self::Prop => "model",
        }
    }

    /// Frames anchored to bones (and so offered the bone picker), as opposed to the
    /// weapon's, which are relative to its model origin.
    pub const fn has_bones(self) -> bool {
        !matches!(self, Self::Weapon)
    }

    /// The sides worth offering for what is open. A prop has only itself; a character
    /// or a weapon in its hand have both ends of the grip.
    pub const fn for_subject(subject: Subject) -> &'static [Self] {
        match subject {
            Subject::Character | Subject::Weapon => &[Self::Character, Self::Weapon],
            Subject::Prop => &[Self::Prop],
        }
    }

    /// The side the panel opens on for what is open.
    pub const fn default_for(subject: Subject) -> Self {
        match subject {
            Subject::Character => Self::Character,
            Subject::Weapon => Self::Weapon,
            Subject::Prop => Self::Prop,
        }
    }
}

/// Metres per nudge. Fine is what you use once the grip is roughly right.
pub const COARSE_TRANSLATION: f32 = 0.02;
pub const FINE_TRANSLATION: f32 = 0.002;

/// Degrees per nudge.
pub const COARSE_ROTATION: f32 = 15.0;
pub const FINE_ROTATION: f32 = 1.0;

#[derive(Resource, Default)]
pub struct HardpointEditor {
    /// Which def is being edited: the character's or the weapon's.
    pub side: HardpointSide,
    /// Role being edited, if any.
    pub active_role: Option<String>,
    pub ui_dirty: bool,
}

impl HardpointEditor {
    pub fn select_role(&mut self, role: &str) {
        self.active_role = Some(role.to_string());
        self.ui_dirty = true;
    }

    /// Switch sides, dropping the role selection — role names overlap between sides
    /// (`grip` exists on both) and carrying one over would silently edit the other def.
    pub fn select_side(&mut self, side: HardpointSide) {
        if self.side != side {
            self.side = side;
            self.active_role = None;
        }
        self.ui_dirty = true;
    }

    /// Redraw the panel after an edit.
    pub const fn touch(&mut self) {
        self.ui_dirty = true;
    }
}

/// Fetch (creating if absent) the hardpoint for `role`.
///
/// A new role inherits the anchor of an existing one, because on a character every
/// hardpoint is anchored to some bone and guessing the model origin instead would put the
/// gizmo at the character's feet, far from anything.
pub fn ensure_role<'a>(def: &'a mut AssetDefinition, role: &str) -> &'a mut Hardpoint {
    let inherited = def.hardpoints.values().find_map(|hp| hp.anchor.clone());
    def.hardpoints
        .entry(role.to_string())
        .or_insert_with(|| Hardpoint {
            anchor: inherited,
            ..Default::default()
        })
}

/// Add `role` if the def does not have it yet; a role it already has is left alone.
///
/// The anchor comes from an existing hardpoint when there is one (see [`ensure_role`]),
/// otherwise from `fallback_anchor` — on a character with no hardpoints yet, the caller's
/// best guess at the hand, so the new frame appears in the palm rather than at the feet.
pub fn add_role(def: &mut AssetDefinition, role: &str, fallback_anchor: Option<String>) {
    if def.hardpoints.contains_key(role) {
        return;
    }
    let anchor = def
        .hardpoints
        .values()
        .find_map(|hp| hp.anchor.clone())
        .or(fallback_anchor);
    def.hardpoints.insert(
        role.to_string(),
        Hardpoint {
            anchor,
            ..Default::default()
        },
    );
}

/// The bone a weapon grip most likely belongs on: a right hand, not a finger.
///
/// Rigs name it every which way (`mixamorigRightHand`, `Hand.R`, `hand_r`,
/// `RightHand`), so this looks for "hand" plus a right-side marker and skips the finger
/// bones that also contain "hand". Only a starting point — the anchor picker is right
/// there to correct it.
pub fn guess_hand_bone(bones: &[String]) -> Option<String> {
    const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];
    let is_hand = |name: &str| {
        let lower = name.to_lowercase();
        lower.contains("hand") && !FINGERS.iter().any(|f| lower.contains(f))
    };
    let is_right = |name: &str| {
        let lower = name.to_lowercase();
        lower.contains("right")
            || lower.ends_with(".r")
            || lower.ends_with("_r")
            || lower.ends_with(" r")
            || lower.starts_with("r_")
    };
    bones
        .iter()
        .filter(|bone| is_hand(bone))
        // Shortest wins: `RightHand` over `RightHandEnd` and similar leaf helpers.
        .min_by_key(|bone| (!is_right(bone), bone.len()))
        .cloned()
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
    // rem_euclid lands exactly on -180.0 here; this is an identity test, not a
    // tolerance test.
    #[allow(clippy::float_cmp)]
    let half_turn = wrapped == -180.0;
    if half_turn { 180.0 } else { wrapped }
}

/// Push the edited `grip` into the live weapon so it moves with the gizmo.
///
/// Runs when the def changed, and when a weapon has just been equipped: the equip
/// resolves its frames from the files on disk, so without the second case a respawn would
/// show the saved grip instead of the one being edited. `keep_weapons_snapped` does the
/// rest every frame from the component this writes.
pub fn apply_grip_to_equipped_weapon(
    player_def: Res<PlayerAssetDef>,
    players: Query<&EquippedWeapon, With<Player>>,
    just_equipped: Query<(), (With<Player>, Added<EquippedWeapon>)>,
    mut weapons: Query<&mut WeaponModel>,
) {
    if !player_def.is_changed() && just_equipped.is_empty() {
        return;
    }
    let Some(def) = player_def.0.as_ref() else {
        return;
    };
    let Some(grip) = def.hardpoints.get(GRIP_ROLE) else {
        return;
    };

    for equipped in players.iter() {
        if let Ok(mut weapon) = weapons.get_mut(equipped.0) {
            weapon.set_char_grip(grip.clone());
        }
    }
}

/// Push the edited weapon def into the live weapon: the grip so it re-snaps in the hand,
/// the muzzle so bullets leave from the new spot.
///
/// The muzzle write is what makes this worth having — the tracer origin comes off
/// `Weapon::muzzle`, so with this in place you can fire while nudging and watch the
/// streak line up with the barrel.
pub fn apply_weapon_def_to_equipped_weapon(
    weapon_def: Res<WeaponDoc>,
    players: Query<&EquippedWeapon, With<Player>>,
    just_equipped: Query<(), (With<Player>, Added<EquippedWeapon>)>,
    mut weapons: Query<(&mut WeaponModel, &mut Weapon)>,
) {
    if !weapon_def.is_changed() && just_equipped.is_empty() {
        return;
    }
    let Some(def) = weapon_def.def.as_ref() else {
        return;
    };

    for equipped in players.iter() {
        let Ok((mut model, mut weapon)) = weapons.get_mut(equipped.0) else {
            continue;
        };
        if let Some(grip) = def.hardpoints.get(GRIP_ROLE) {
            model.set_weapon_grip(grip.clone());
        }
        weapon.muzzle = def.hardpoints.get(MUZZLE_ROLE).cloned();
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
            Hardpoint {
                anchor: Some(bone.to_string()),
                ..Default::default()
            },
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
    fn adding_a_role_to_a_bare_character_uses_the_guessed_hand() {
        let mut def = AssetDefinition::default();
        add_role(&mut def, "grip", Some("mixamorigRightHand".to_string()));
        assert_eq!(
            def.hardpoints["grip"].anchor.as_deref(),
            Some("mixamorigRightHand")
        );
    }

    #[test]
    fn adding_a_role_that_exists_keeps_its_values() {
        let mut def = def_with_grip_on("Hand");
        def.hardpoints.get_mut("grip").unwrap().translation = [1.0, 2.0, 3.0];
        add_role(&mut def, "grip", Some("Elsewhere".to_string()));
        assert_eq!(def.hardpoints["grip"].translation, [1.0, 2.0, 3.0]);
        assert_eq!(def.hardpoints["grip"].anchor.as_deref(), Some("Hand"));
    }

    #[test]
    fn the_hand_guess_prefers_the_right_hand_over_fingers_and_the_left() {
        let bones: Vec<String> = [
            "mixamorigLeftHand",
            "mixamorigRightHandIndex1",
            "mixamorigRightHand",
            "mixamorigSpine",
        ]
        .map(str::to_string)
        .to_vec();
        assert_eq!(
            guess_hand_bone(&bones).as_deref(),
            Some("mixamorigRightHand")
        );

        let blender: Vec<String> = ["Hand.L", "Hand.R", "Thumb.R"].map(str::to_string).to_vec();
        assert_eq!(guess_hand_bone(&blender).as_deref(), Some("Hand.R"));

        let none: Vec<String> = ["Spine", "Head"].map(str::to_string).to_vec();
        assert_eq!(guess_hand_bone(&none), None);
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

    /// `muzzle` on a character would be meaningless, and hiding it on the weapon would
    /// make the thing this panel exists for unreachable.
    #[test]
    fn only_the_weapon_side_offers_a_muzzle() {
        assert!(HardpointSide::Weapon.roles().contains(&"muzzle"));
        assert!(!HardpointSide::Character.roles().contains(&"muzzle"));
    }

    /// `grip` exists on both sides; a role carried across the switch would edit the
    /// other def without the panel looking any different.
    #[test]
    fn switching_sides_drops_the_selected_role() {
        let mut editor = HardpointEditor::default();
        editor.select_role("grip");
        editor.select_side(HardpointSide::Weapon);
        assert_eq!(editor.active_role, None);
    }

    #[test]
    fn reselecting_the_same_side_keeps_the_role() {
        let mut editor = HardpointEditor::default();
        editor.select_role("grip");
        editor.select_side(HardpointSide::Character);
        assert_eq!(editor.active_role.as_deref(), Some("grip"));
    }

    #[test]
    fn wrapping_keeps_the_angle_it_is_given_where_it_can() {
        assert_eq!(wrap_degrees(0.0), 0.0);
        assert_eq!(wrap_degrees(90.0), 90.0);
        assert_eq!(wrap_degrees(-90.0), -90.0);
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(
            wrap_degrees(180.0),
            180.0,
            "a half turn reads as +180, not -180"
        );
    }
}
