//! The guns a player carries and switching between them.
//!
//! [`Weapons`] is data only: def paths plus each gun's remembered magazine. The held gun
//! is a live entity built by `equip` from a [`PendingEquip`]; switching despawns it and
//! queues a fresh `PendingEquip` for the next one, carrying the magazine count across.
//! That is a two-frame swap rather than hiding/showing entities, because the equip path
//! also resolves arm IK, sight alignment and the aimed-weapon parenting per gun, and
//! re-running it is far simpler than keeping all of that consistent for hidden weapons.

use bevy::prelude::*;

use crate::assets::asset_definition::AssetDefinition;
use crate::player::components::{Player, PlayerDead};
use crate::player::systems::arm_ik::{SightAlign, WeaponArms};
use crate::player::systems::equip::{EquippedWeapon, PendingEquip};
use crate::player::systems::shoot::Weapon;

#[derive(Debug, Clone, PartialEq)]
pub struct WeaponSlot {
    /// Path to the weapon def, e.g. `assets/defs/Pistol.ron`.
    pub def_path: String,
    /// Rounds left in this gun's magazine when it was last holstered. `None` = never
    /// equipped yet, so it spawns full.
    pub rounds_in_mag: Option<u32>,
}

/// Everything a player carries, whether or not it is in their hands.
#[derive(Component, Debug, Clone, Default)]
pub struct Weapons {
    pub slots: Vec<WeaponSlot>,
    pub active: usize,
    /// The character's own def, needed to resolve grip hardpoints on every equip.
    pub character_def_path: String,
}

impl Weapons {
    pub fn new(character_def_path: impl Into<String>, def_paths: impl IntoIterator<Item = String>) -> Self {
        Self {
            slots: def_paths
                .into_iter()
                .map(|def_path| WeaponSlot { def_path, rounds_in_mag: None })
                .collect(),
            active: 0,
            character_def_path: character_def_path.into(),
        }
    }

    pub fn active_slot(&self) -> Option<&WeaponSlot> {
        self.slots.get(self.active)
    }

    pub fn index_of(&self, def_path: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.def_path == def_path)
    }

    /// Add a gun if it is not already carried. Returns its slot index either way.
    pub fn add(&mut self, def_path: &str) -> (usize, bool) {
        if let Some(i) = self.index_of(def_path) {
            return (i, false);
        }
        self.slots.push(WeaponSlot { def_path: def_path.to_string(), rounds_in_mag: None });
        (self.slots.len() - 1, true)
    }

    /// Resolve a selection to a slot index, wrapping for Next/Prev. `None` when there is
    /// nothing to switch to or the selection is out of range.
    pub fn resolve(&self, select: WeaponSelect) -> Option<usize> {
        let n = self.slots.len();
        if n == 0 {
            return None;
        }
        match select {
            WeaponSelect::Slot(i) => (i < n).then_some(i),
            WeaponSelect::Next => Some((self.active + 1) % n),
            WeaponSelect::Prev => Some((self.active + n - 1) % n),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSelect {
    Slot(usize),
    Next,
    Prev,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct SwitchWeapon {
    pub player: Entity,
    pub select: WeaponSelect,
}

/// Holster the held gun (remembering its magazine) and queue the selected one.
#[allow(clippy::type_complexity)]
pub fn switch_weapons(
    mut commands: Commands,
    mut requests: MessageReader<SwitchWeapon>,
    mut players: Query<(&mut Weapons, Option<&EquippedWeapon>, Has<PendingEquip>), (With<Player>, Without<PlayerDead>)>,
    weapons: Query<&Weapon>,
) {
    for req in requests.read() {
        let Ok((mut loadout, equipped, mid_equip)) = players.get_mut(req.player) else { continue };
        // A swap already in flight: let it land before starting another.
        if mid_equip {
            continue;
        }
        let Some(next) = loadout.resolve(req.select) else { continue };
        if next == loadout.active && equipped.is_some() {
            continue;
        }

        // Holster: remember the magazine and tear the live gun down.
        if let Some(EquippedWeapon(weapon_entity)) = equipped {
            let active = loadout.active;
            if let Ok(weapon) = weapons.get(*weapon_entity)
                && let Some(slot) = loadout.slots.get_mut(active)
            {
                slot.rounds_in_mag = Some(weapon.rounds_in_mag);
            }
            commands.entity(*weapon_entity).despawn();
            commands
                .entity(req.player)
                .remove::<EquippedWeapon>()
                .remove::<WeaponArms>()
                .remove::<SightAlign>();
        }

        loadout.active = next;
        queue_equip(&mut commands, req.player, &loadout);
    }
}

/// Insert a `PendingEquip` for the loadout's active slot, if the defs resolve.
pub fn queue_equip(commands: &mut Commands, player: Entity, loadout: &Weapons) {
    let Some(slot) = loadout.active_slot() else { return };
    let Some(character) = AssetDefinition::load_from_def_path(&loadout.character_def_path) else {
        warn!("cannot equip {}: character def {} failed to load", slot.def_path, loadout.character_def_path);
        return;
    };
    match PendingEquip::resolve(&character, &slot.def_path) {
        Some(mut equip) => {
            equip.rounds_in_mag = slot.rounds_in_mag;
            commands.entity(player).insert(equip);
        }
        None => warn!("cannot equip {}: no grip pairing with {}", slot.def_path, loadout.character_def_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loadout() -> Weapons {
        Weapons::new("c.ron", ["a.ron".to_string(), "b.ron".to_string(), "c.ron".to_string()])
    }

    #[test]
    fn next_and_prev_wrap() {
        let mut w = loadout();
        assert_eq!(w.resolve(WeaponSelect::Next), Some(1));
        w.active = 2;
        assert_eq!(w.resolve(WeaponSelect::Next), Some(0));
        w.active = 0;
        assert_eq!(w.resolve(WeaponSelect::Prev), Some(2));
    }

    #[test]
    fn slot_selection_is_bounds_checked() {
        let w = loadout();
        assert_eq!(w.resolve(WeaponSelect::Slot(1)), Some(1));
        assert_eq!(w.resolve(WeaponSelect::Slot(7)), None);
        assert_eq!(Weapons::default().resolve(WeaponSelect::Next), None);
    }

    #[test]
    fn adding_is_idempotent() {
        let mut w = loadout();
        assert_eq!(w.add("b.ron"), (1, false));
        assert_eq!(w.add("d.ron"), (3, true));
        assert_eq!(w.slots.len(), 4);
    }
}
