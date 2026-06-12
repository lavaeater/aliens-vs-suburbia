use bevy::app::{App, Plugin, Update};
use bevy::prelude::IntoScheduleConfigs;

use super::facts_resource::Facts;
use super::messages::{FactChanged, SetFact, StoryEffect, StoryFired};
use super::story::StoryStore;
use super::systems::{apply_set_fact, arm_story_check, check_stories, emit_fact_changes};

/// Registers the facts subsystem: the [`Facts`] store and [`StoryStore`] resources, the
/// fact/story messages, and the chained systems that drive story reactions.
pub struct FactsPlugin;

impl Plugin for FactsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Facts>()
            .init_resource::<StoryStore>()
            .add_message::<FactChanged>()
            .add_message::<SetFact>()
            .add_message::<StoryEffect>()
            .add_message::<StoryFired>()
            // Order matters (a shared dirty-flag chain, see docs/turbofacts.md):
            //   apply_set_fact   drain SetFact -> Facts
            //   emit_fact_changes drain Facts dirty -> FactChanged
            //   arm_story_check  FactChanged -> StoryStore.needs_checking
            //   check_stories    evaluate + apply consequences (re-dirties Facts)
            .add_systems(
                Update,
                (
                    apply_set_fact,
                    emit_fact_changes,
                    arm_story_check,
                    check_stories,
                )
                    .chain(),
            );
    }
}
