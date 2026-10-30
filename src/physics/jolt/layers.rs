//! Avian's collision layers on Jolt's object layers.
//!
//! Avian lets a body be in several layers and filter on several (`CollisionLayers`: memberships
//! and filters, two bit masks); Jolt puts each body in exactly one object layer and takes a fixed
//! table of which object layers collide, made with the world. `LayerTable` gives every distinct
//! `CollisionLayers` the game uses an object layer of its own and enables a pair by Avian's rule.
//! The table can't grow once the world exists, so a body with a combination the table hasn't
//! seen makes the backend rebuild the world (`world::JoltWorld::rebuild`); that happens a handful
//! of times per session, the first time each kind of thing is spawned.

use bevy::prelude::*;
use oxijolt::{BroadPhaseLayer, ObjectLayer};

/// A layer enum usable in `CollisionLayers`. Same shape as Avian's trait, so one hand-written
/// impl serves both backends.
pub trait PhysicsLayer: Sized + Default {
    fn to_bits(&self) -> u32;
    fn all_bits() -> u32;
}

/// A set of layers as bits.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerMask(pub u32);

impl LayerMask {
    pub const ALL: Self = Self(u32::MAX);
    pub const NONE: Self = Self(0);
}

impl Default for LayerMask {
    fn default() -> Self {
        Self::ALL
    }
}

impl From<u32> for LayerMask {
    fn from(bits: u32) -> Self {
        Self(bits)
    }
}

impl<L: PhysicsLayer> From<L> for LayerMask {
    fn from(layer: L) -> Self {
        Self(layer.to_bits())
    }
}

impl<L: PhysicsLayer, const N: usize> From<[L; N]> for LayerMask {
    fn from(layers: [L; N]) -> Self {
        Self(layers.iter().fold(0, |bits, l| bits | l.to_bits()))
    }
}

/// What a collider is (`memberships`) and what it collides with (`filters`).
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[reflect(Component)]
pub struct CollisionLayers {
    pub memberships: LayerMask,
    pub filters: LayerMask,
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self { memberships: LayerMask(1), filters: LayerMask::ALL }
    }
}

impl CollisionLayers {
    pub fn new(memberships: impl Into<LayerMask>, filters: impl Into<LayerMask>) -> Self {
        Self { memberships: memberships.into(), filters: filters.into() }
    }

    /// Avian's rule: each side's memberships meet the other side's filters.
    pub fn interacts_with(self, other: Self) -> bool {
        self.memberships.0 & other.filters.0 != 0 && other.memberships.0 & self.filters.0 != 0
    }
}

/// Static and moving bodies get a broad-phase tree each, as Jolt recommends.
pub const NON_MOVING: BroadPhaseLayer = BroadPhaseLayer::NON_MOVING;
pub const MOVING: BroadPhaseLayer = BroadPhaseLayer::MOVING;

/// The object layers handed out so far: one per (`CollisionLayers`, moving or not).
#[derive(Default, Clone, Debug)]
pub struct LayerTable {
    entries: Vec<(CollisionLayers, bool)>,
}

impl LayerTable {
    /// The object layer of a combination, if the table has it.
    pub fn get(&self, layers: CollisionLayers, moving: bool) -> Option<ObjectLayer> {
        self.entries
            .iter()
            .position(|&e| e == (layers, moving))
            .map(|i| ObjectLayer::new(i as u32))
    }

    /// Adds a combination; true when it is new (and the world must be rebuilt to know it).
    pub fn learn(&mut self, layers: CollisionLayers, moving: bool) -> bool {
        if self.get(layers, moving).is_some() {
            return false;
        }
        self.entries.push((layers, moving));
        true
    }

    /// Jolt's table for this set. Never empty: Jolt refuses a world without object layers.
    pub fn to_jolt(&self) -> oxijolt::CollisionLayers {
        let mut table = oxijolt::CollisionLayers::new(2);
        if self.entries.is_empty() {
            table.add_object_layer(NON_MOVING);
            return table;
        }
        let layers: Vec<(ObjectLayer, CollisionLayers, bool)> = self
            .entries
            .iter()
            .map(|&(l, moving)| {
                (table.add_object_layer(if moving { MOVING } else { NON_MOVING }), l, moving)
            })
            .collect();
        for (i, &(id_a, a, a_moving)) in layers.iter().enumerate() {
            for &(id_b, b, b_moving) in layers.iter().skip(i) {
                // Two things that never move have nothing to say to each other.
                if (a_moving || b_moving) && a.interacts_with(b) {
                    table.enable_collision(id_a, id_b);
                }
            }
        }
        table
    }

    /// The object layers a query with `mask` sees: those whose memberships meet the mask (Avian's
    /// `SpatialQueryFilter` rule).
    pub fn seen_by(&self, mask: LayerMask) -> Vec<ObjectLayer> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, (l, _))| l.memberships.0 & mask.0 != 0)
            .map(|(i, _)| ObjectLayer::new(i as u32))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default, Clone, Copy)]
    enum Layer {
        #[default]
        Floor,
        Ball,
        Alien,
    }

    impl PhysicsLayer for Layer {
        fn to_bits(&self) -> u32 {
            1 << *self as u32
        }
        fn all_bits() -> u32 {
            0b111
        }
    }

    #[test]
    fn masks_combine_their_layers() {
        assert_eq!(LayerMask::from([Layer::Floor, Layer::Alien]).0, 0b101);
        assert_eq!(LayerMask::from([Layer::Floor; 0]).0, 0);
        assert_eq!(LayerMask::from(Layer::Ball).0, 0b010);
    }

    #[test]
    fn a_pair_collides_only_when_both_sides_agree() {
        let ball = CollisionLayers::new([Layer::Ball], [Layer::Floor, Layer::Alien]);
        let alien = CollisionLayers::new([Layer::Alien], [Layer::Floor]);
        let floor = CollisionLayers::new([Layer::Floor], [Layer::Ball, Layer::Alien]);
        assert!(ball.interacts_with(floor));
        assert!(alien.interacts_with(floor));
        // The ball wants the alien, but the alien doesn't filter on balls.
        assert!(!ball.interacts_with(alien));
    }

    #[test]
    fn each_combination_gets_one_object_layer() {
        let mut table = LayerTable::default();
        let a = CollisionLayers::new([Layer::Ball], [Layer::Floor]);
        assert!(table.learn(a, true));
        assert!(!table.learn(a, true));
        assert!(table.learn(a, false));
        assert_eq!(table.get(a, true), Some(ObjectLayer::new(0)));
        assert_eq!(table.get(a, false), Some(ObjectLayer::new(1)));
        assert_eq!(table.to_jolt().object_layer_count(), 2);
    }

    #[test]
    fn queries_see_the_layers_their_mask_names() {
        let mut table = LayerTable::default();
        table.learn(CollisionLayers::new([Layer::Floor], LayerMask::ALL), false);
        table.learn(CollisionLayers::new([Layer::Alien], LayerMask::ALL), true);
        assert_eq!(table.seen_by(LayerMask::from([Layer::Alien])), [ObjectLayer::new(1)]);
        assert!(table.seen_by(LayerMask::NONE).is_empty());
    }
}
