//! Ammo carried by a player, outside whatever is loaded in the gun.
//!
//! One pool per [`AmmoKind`], each capped by `AmmoKind::cap`. Reloads pull from here
//! (see `shoot::tick_reloads`), pickups add to it, and `Infinite` is never consulted.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::assets::asset_definition::AmmoKind;

#[derive(Component, Debug, Clone, Default)]
pub struct AmmoPouch(pub HashMap<AmmoKind, u32>);

impl AmmoPouch {
    pub fn from_loadout(loadout: &[(AmmoKind, u32)]) -> Self {
        let mut pouch = Self::default();
        for (kind, rounds) in loadout {
            pouch.add(*kind, *rounds);
        }
        pouch
    }

    pub fn rounds(&self, kind: AmmoKind) -> u32 {
        if kind == AmmoKind::Infinite {
            return u32::MAX;
        }
        self.0.get(&kind).copied().unwrap_or(0)
    }

    /// Add rounds up to the kind's cap; returns how many actually fit.
    pub fn add(&mut self, kind: AmmoKind, rounds: u32) -> u32 {
        if kind == AmmoKind::Infinite {
            return rounds;
        }
        let slot = self.0.entry(kind).or_insert(0);
        let room = kind.cap().saturating_sub(*slot);
        let added = rounds.min(room);
        *slot += added;
        added
    }

    /// Take up to `wanted` rounds; returns how many came out.
    pub fn take(&mut self, kind: AmmoKind, wanted: u32) -> u32 {
        if kind == AmmoKind::Infinite {
            return wanted;
        }
        let slot = self.0.entry(kind).or_insert(0);
        let taken = wanted.min(*slot);
        *slot -= taken;
        taken
    }

    pub fn is_full(&self, kind: AmmoKind) -> bool {
        kind != AmmoKind::Infinite && self.rounds(kind) >= kind.cap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_respects_the_cap_and_reports_what_fit() {
        let mut pouch = AmmoPouch::default();
        assert_eq!(pouch.add(AmmoKind::Grenade, 4), 4);
        assert_eq!(pouch.add(AmmoKind::Grenade, 10), 2, "cap is 6");
        assert_eq!(pouch.rounds(AmmoKind::Grenade), 6);
        assert!(pouch.is_full(AmmoKind::Grenade));
    }

    #[test]
    fn taking_never_goes_negative() {
        let mut pouch = AmmoPouch::from_loadout(&[(AmmoKind::Pistol, 5)]);
        assert_eq!(pouch.take(AmmoKind::Pistol, 12), 5);
        assert_eq!(pouch.rounds(AmmoKind::Pistol), 0);
        assert_eq!(pouch.take(AmmoKind::Pistol, 1), 0);
    }

    #[test]
    fn infinite_is_bottomless_and_never_full() {
        let mut pouch = AmmoPouch::default();
        assert_eq!(pouch.take(AmmoKind::Infinite, 30), 30);
        assert!(!pouch.is_full(AmmoKind::Infinite));
        assert!(pouch.0.is_empty(), "infinite never touches the map");
    }
}
