//! Well-known fact keys. The Rust analog of Kotlin's `Factoids` object — a central place
//! for the string keys used across systems so they don't drift.

pub const LEVEL_STARTED: &str = "LevelStarted";
pub const LEVEL_COMPLETE: &str = "LevelComplete";
pub const LEVEL_FAILED: &str = "LevelFailed";
pub const GOTO_NEXT_LEVEL: &str = "GotoNextLevel";

pub const BOSS_IS_DEAD: &str = "BossIsDead";
pub const ALL_OBJECTIVES_TOUCHED: &str = "AllObjectivesAreTouched";

pub const ENEMY_KILL_COUNT: &str = "EnemyKillCount";
pub const TARGET_ENEMY_KILL_COUNT: &str = "TargetEnemyKillCount";
pub const ENEMY_COUNT: &str = "EnemyCount";
pub const ALIENS_TO_SPAWN: &str = "AliensToSpawn";

/// Derived win/lose condition bools that stories read (set by `derive_world_facts`).
pub const ALL_ALIENS_DEAD: &str = "AllAliensDead";
pub const ALL_PLAYERS_DEAD: &str = "AllPlayersDead";
pub const TOO_MANY_ALIENS_ESCAPED: &str = "TooManyAliensEscaped";

pub const ALIENS_ESCAPED: &str = "AliensEscaped";
pub const ALIENS_ESCAPED_CUTOFF: &str = "AliensEscapedCutoff";

pub const SHOTS_FIRED: &str = "ShotsFired";
pub const SHOTS_HIT: &str = "ShotsHit";

pub const WAVE_SIZE: &str = "WaveSize";
pub const CURRENT_WAVE: &str = "CurrentWave";
pub const WAVE_COUNT: &str = "WaveCount";
pub const ALL_WAVES_DONE: &str = "AllWavesDone";

pub const LIVING_PLAYER_COUNT: &str = "LivingPlayerCount";
pub const COINS: &str = "Coins";

pub const MAP_START_MESSAGE: &str = "MapStartMessage";
pub const MAP_SUCCESS_MESSAGE: &str = "MapSuccessMessage";
pub const MAP_FAIL_MESSAGE: &str = "MapFailMessage";

/// Prefix of a [`StoryEffect`](crate::messages::StoryEffect) that shows a story crawl:
/// `Emit(effect: "crawl:level_01_intro")` shows `assets/crawls/level_01_intro.ron`.
pub const CRAWL_EFFECT_PREFIX: &str = "crawl:";
/// True while a crawl is on screen or queued.
pub const CRAWL_SHOWING: &str = "CrawlShowing";

/// Set true once crawl `id` has run to its end (or been skipped), so a story can follow
/// on from it.
pub fn crawl_finished(id: &str) -> String {
    format!("CrawlFinished.{id}")
}

/// Set true when a player picks up a story item (`ItemKind::Key { id }`).
pub fn item_found(id: &str) -> String {
    format!("ItemFound.{id}")
}

/// The crawl a story effect asks for, if it is a crawl effect: `"crawl:intro"` -> `"intro"`.
pub fn crawl_effect(effect: &str) -> Option<&str> {
    effect
        .strip_prefix(CRAWL_EFFECT_PREFIX)
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crawl_effects_name_their_crawl() {
        assert_eq!(crawl_effect("crawl:intro"), Some("intro"));
        assert_eq!(crawl_effect("crawl: intro "), Some("intro"));
        assert_eq!(crawl_effect("crawl:"), None);
        assert_eq!(crawl_effect("level_complete"), None);
    }
}
