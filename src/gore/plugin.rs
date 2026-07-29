//! The gore/FX layer. Registers the shared damage/death messages, the budget, and
//! the subscriber systems: blood, gibs, destructible-terrain rubble, fire, plus the
//! juice layer (SFX + barks).

use bevy::prelude::*;

use crate::game_state::GameState;
use crate::gore::barks::{
    bark_on_events, reset_atrocity, setup_bark_caption, tick_bark_caption, AtrocityMeter, BarkState,
};
use crate::gore::blood::{setup_blood_assets, spawn_blood_on_damage};
use crate::gore::components::{DamageDealt, EntityDied, GoreBudget};
use crate::gore::despair::{apply_despair, despair_heartbeat, DespairSettings, Heartbeat};
use crate::gore::fire::{setup_fire_assets, spawn_fire_fields, tick_fire_fields, SpawnFire};
use crate::gore::gibs::{setup_gib_assets, spawn_gibs_on_death};
use crate::gore::sfx::{emit_combat_sfx, play_sfx, setup_sfx_bank, PlaySfx};
use crate::gore::systems::{record_last_hit, tick_ephemeral};
use crate::gore::terrain::{destroy_damaged_terrain, setup_debris_assets};

pub struct GorePlugin;

impl Plugin for GorePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageDealt>()
            .add_message::<EntityDied>()
            .add_message::<SpawnFire>()
            .add_message::<PlaySfx>()
            .init_resource::<GoreBudget>()
            .init_resource::<AtrocityMeter>()
            .init_resource::<BarkState>()
            .init_resource::<DespairSettings>()
            .init_resource::<Heartbeat>()
            .add_systems(
                Startup,
                (
                    setup_blood_assets,
                    setup_gib_assets,
                    setup_debris_assets,
                    setup_fire_assets,
                    setup_sfx_bank,
                ),
            )
            .add_systems(
                OnEnter(GameState::InGame),
                (setup_bark_caption, reset_atrocity),
            )
            .add_systems(
                Update,
                (
                    record_last_hit,
                    spawn_blood_on_damage,
                    spawn_gibs_on_death,
                    destroy_damaged_terrain,
                    spawn_fire_fields,
                    tick_fire_fields,
                    tick_ephemeral,
                    emit_combat_sfx,
                    play_sfx,
                    bark_on_events,
                    tick_bark_caption,
                    apply_despair,
                    despair_heartbeat,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
