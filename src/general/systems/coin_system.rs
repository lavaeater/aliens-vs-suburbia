//! The team's shared money. Coins on the ground are ordinary `Item(ItemKind::Coins)`
//! pickups (see `items`); the [`Coin`] marker only exists so GoldDigger can find them.

use bevy::prelude::*;

/// Shared team wallet — all players draw from and deposit into the same pool.
#[derive(Resource, Default)]
pub struct TeamWallet {
    pub coins: u32,
}

/// Marker on coin pickups, carrying the value for abilities that collect them remotely.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct Coin {
    pub value: u32,
}

/// How close (world units) a player must be to auto-collect pickups.
#[derive(Component, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct PickupRange(pub f32);

impl Default for PickupRange {
    fn default() -> Self { Self(1.8) }
}
