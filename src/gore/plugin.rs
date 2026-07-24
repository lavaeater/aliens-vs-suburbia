//! The gore/FX layer. Registers the shared damage/death messages, the budget, and
//! the subscriber systems (blood now; gibs, scorch, SFX later).

use bevy::prelude::*;

use crate::game_state::GameState;
use crate::gore::blood::{setup_blood_assets, spawn_blood_on_damage};
use crate::gore::components::{DamageDealt, EntityDied, GoreBudget};
use crate::gore::gibs::{setup_gib_assets, spawn_gibs_on_death};
use crate::gore::systems::{record_last_hit, tick_ephemeral};

pub struct GorePlugin;

impl Plugin for GorePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageDealt>()
            .add_message::<EntityDied>()
            .init_resource::<GoreBudget>()
            .add_systems(Startup, (setup_blood_assets, setup_gib_assets))
            .add_systems(
                Update,
                (
                    record_last_hit,
                    spawn_blood_on_damage,
                    spawn_gibs_on_death,
                    tick_ephemeral,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
