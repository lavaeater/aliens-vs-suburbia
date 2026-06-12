//! Built-in game stories, ported from Kotlin's `StoryHelper`. These define the basic
//! level-flow (start / complete / failed) and a couple of win-condition variants. They are
//! authored in code via the [`builder`](super::builder) DSL; designers can add more via RON
//! story files (see [`super::persistence`]).
//!
//! Consequence side effects are surfaced as named [`StoryEffect`](super::StoryEffect)s for
//! game plugins to handle: `"level_starting"`, `"level_complete"`, `"level_failed"`.

use super::builder::story;
use super::keys;
use super::story::Story;

/// The level has started once `LevelStarted` flips true; emits `level_starting`. Its
/// `init_facts` reset the flow + derived-condition facts on (re)activation, so replaying a
/// level starts from a clean slate.
pub fn level_start_story() -> Story {
    story("Level Start")
        .init_bool(keys::LEVEL_STARTED, false)
        .init_bool(keys::LEVEL_COMPLETE, false)
        .init_bool(keys::LEVEL_FAILED, false)
        .init_bool(keys::GOTO_NEXT_LEVEL, false)
        .init_bool(keys::ALL_ALIENS_DEAD, false)
        .init_bool(keys::ALL_PLAYERS_DEAD, false)
        .init_bool(keys::TOO_MANY_ALIENS_ESCAPED, false)
        .rule("not yet started", |r| {
            r.is_false(keys::LEVEL_STARTED);
        })
        .set_true(keys::LEVEL_STARTED)
        .set_false(keys::LEVEL_COMPLETE)
        .set_false(keys::LEVEL_FAILED)
        .emit("level_starting")
        .build()
}

/// Win condition: the level is running and every spawned alien is dead with no waves left.
/// Sets `LevelComplete`, which the `level_complete_story` then turns into `GotoNextLevel`.
pub fn aliens_cleared_story() -> Story {
    story("Aliens Cleared")
        .rule("all aliens dead", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .is_true(keys::ALL_ALIENS_DEAD);
        })
        .set_true(keys::LEVEL_COMPLETE)
        .build()
}

/// When the level is complete, move on; emits `level_complete`.
pub fn level_complete_story() -> Story {
    story("Level Complete")
        .rule("complete", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .is_true(keys::LEVEL_COMPLETE);
        })
        .set_true(keys::GOTO_NEXT_LEVEL)
        .emit("level_complete")
        .build()
}

/// All players dead while a level is running -> failed; emits `level_failed`. Reads the
/// derived `ALL_PLAYERS_DEAD` bool (which already guards against "no players yet") rather
/// than `LIVING_PLAYER_COUNT == 0`, so the level can't fail before anyone has spawned.
pub fn level_failed_story() -> Story {
    story("Level Failed (players)")
        .rule("all players dead", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .is_true(keys::ALL_PLAYERS_DEAD);
        })
        .set_true(keys::LEVEL_FAILED)
        .emit("level_failed")
        .build()
}

/// Too many aliens reached the goal while a level is running -> failed; emits `level_failed`.
/// A separate story from the all-players-dead loss because a story ANDs its rules — distinct
/// lose conditions are OR'd by being distinct stories.
pub fn level_failed_escaped_story() -> Story {
    story("Level Failed (escaped)")
        .rule("too many escaped", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .is_true(keys::TOO_MANY_ALIENS_ESCAPED);
        })
        .set_true(keys::LEVEL_FAILED)
        .emit("level_failed")
        .build()
}

/// Classic win: boss dead and all objectives touched. Seeds the level-start facts.
pub fn boss_and_objectives_story() -> Story {
    story("Touch All Objectives and Kill the Boss")
        .init_bool(keys::BOSS_IS_DEAD, false)
        .init_bool(keys::ALL_OBJECTIVES_TOUCHED, false)
        .init_bool(keys::LEVEL_COMPLETE, false)
        .init_bool(keys::LEVEL_FAILED, false)
        .rule("done", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .is_true(keys::BOSS_IS_DEAD)
                .is_true(keys::ALL_OBJECTIVES_TOUCHED);
        })
        .set_true(keys::LEVEL_COMPLETE)
        .build()
}

/// Kill-count win: complete the level once enough enemies are dead.
pub fn enemy_kill_count_story() -> Story {
    story("Reach the Kill Count")
        .init_int(keys::ENEMY_KILL_COUNT, 0)
        .init_int(keys::TARGET_ENEMY_KILL_COUNT, 3)
        .init_bool(keys::LEVEL_COMPLETE, false)
        .rule("enough kills", |r| {
            r.is_true(keys::LEVEL_STARTED)
                .int_more_than_fact(keys::ENEMY_KILL_COUNT, keys::TARGET_ENEMY_KILL_COUNT);
        })
        .set_true(keys::LEVEL_COMPLETE)
        .build()
}

/// The base level-flow stories that drive the live game's win/lose verdict. Mirrors Kotlin's
/// `StoryHelper.baseStories`. The `aliens_cleared_story` is the default win condition (kill
/// everything); maps can swap it out via RON stories for survival/objective variants.
pub fn base_stories() -> Vec<Story> {
    vec![
        level_start_story(),
        aliens_cleared_story(),
        level_complete_story(),
        level_failed_story(),
        level_failed_escaped_story(),
    ]
}
