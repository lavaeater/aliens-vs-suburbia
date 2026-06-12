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
