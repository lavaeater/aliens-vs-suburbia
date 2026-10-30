//! The campaign: the levels in order (`assets/campaigns/main/campaign.ron`; the folder
//! layout is `assets::content`), what happens between them,
//! and the end.
//!
//! Start Game begins at the first level. When a level's stories call it (`level_complete`
//! / `level_failed`, see `score_keeper::level_state_system`), the match ends in
//! [`GameState::Intermission`]: a card announcing the next level, offering a retry of a
//! lost one, or, after the last level, the victory screen. Continuing re-enters
//! `InGame`, which loads whatever level is current ([`start_level`]); leaving `InGame`
//! sweeps the old level away, so every level starts clean.
//!
//! A level is a map like any other; its rules (`escape_limit`, `weapons`, `coins`) and its
//! stories (`stories:`) come from the map file. This module applies the rules when the
//! level starts and shows the stories' `ObjectiveText` on the HUD.

use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use lava_ui_builder::{LavaTheme, TextStyle, UIBuilder};
use serde::Deserialize;
use turbofacts::facts_resource::Facts;
use turbofacts::keys;

use crate::alien::components::general::AlienCounter;
use crate::alien::wave_manager::WaveManager;
use crate::character_editor::state::in_normal_game;
use crate::game_state::GameState;
use crate::game_state::score_keeper::LevelTracker;
use crate::general::events::map_events::LoadMap;
use crate::general::systems::coin_system::TeamWallet;
use crate::ui::spawn_ui::{StateMarker, cleanup_state};
use crate::ui::ui_plugin::spawn_ui_camera;

/// The campaign played unless `--campaign <name>` says otherwise: a folder under
/// `assets/campaigns`.
pub const DEFAULT_CAMPAIGN: &str = "neighbourhood";

/// A campaign's file: `assets/campaigns/<name>/campaign.ron`.
pub fn campaign_file(name: &str) -> String {
    format!("{}/{name}/campaign.ron", crate::assets::content::CAMPAIGNS)
}

/// Which campaign is played (`--campaign <name>`; [`DEFAULT_CAMPAIGN`]).
#[derive(Resource, Debug, Clone)]
pub struct CampaignChoice(pub String);

impl Default for CampaignChoice {
    fn default() -> Self {
        Self(DEFAULT_CAMPAIGN.into())
    }
}

/// Seconds an intermission card ignores input, so a held fire button does not skip it.
const INPUT_GRACE: f32 = 0.8;

/// One level of the campaign.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CampaignLevel {
    /// The map, inside `assets/` (`"campaigns/main/level_01/level_01.map"`; `assets::paths`).
    /// Given a `.ron`, a `.map` next to it wins (`text_format::load_level`).
    pub map: String,
    /// Shown on the card before the level.
    pub title: String,
    /// One line under the title.
    #[serde(default)]
    pub tagline: String,
}

/// A campaign's `campaign.ron` ([`campaign_file`]).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CampaignFile {
    pub levels: Vec<CampaignLevel>,
    /// The victory screen's heading and lines.
    #[serde(default = "default_victory_title")]
    pub victory_title: String,
    #[serde(default)]
    pub victory_lines: Vec<String>,
}

fn default_victory_title() -> String {
    "VICTORY".into()
}

impl Default for CampaignFile {
    fn default() -> Self {
        Self {
            levels: vec![CampaignLevel {
                map: "campaigns/main/level_01/level_01.map".into(),
                title: "Level 1".into(),
                tagline: String::new(),
            }],
            victory_title: default_victory_title(),
            victory_lines: Vec::new(),
        }
    }
}

/// The campaign being played and where in it the team is.
#[derive(Resource, Debug, Clone, Default)]
pub struct Campaign {
    pub file: CampaignFile,
    /// Index into `file.levels` of the level being played (or about to be).
    pub current: usize,
    /// Kills on the level just finished, for the card.
    pub last_kills: i32,
}

/// What the card between levels says.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intermission {
    /// The level was won; the next one (now `current`) comes up.
    NextLevel,
    /// The level was lost; it can be played again.
    Retry,
    /// The last level was won.
    Victory,
}

impl Campaign {
    /// Read campaign `name` ([`campaign_file`]), or a one-level campaign of level 1 if it
    /// cannot be read.
    pub fn load(name: &str) -> Self {
        let path = campaign_file(name);
        let file = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| ron::from_str::<CampaignFile>(&text).map_err(|e| e.to_string()));
        match file {
            Ok(file) if !file.levels.is_empty() => Self {
                file,
                ..Self::default()
            },
            Ok(_) => {
                error!("{path} lists no levels; playing level 1 only");
                Self::default()
            }
            Err(e) => {
                error!("{path}: {e}; playing level 1 only");
                Self::default()
            }
        }
    }

    pub fn level(&self) -> Option<&CampaignLevel> {
        self.file.levels.get(self.current)
    }

    /// "Level 2 of 5".
    pub fn progress(&self) -> String {
        format!(
            "Level {} of {}",
            self.current.saturating_add(1),
            self.file.levels.len()
        )
    }

    /// Jump to level `number` (1-based), clamped to the campaign.
    pub fn go_to(&mut self, number: usize) {
        self.current = number
            .saturating_sub(1)
            .min(self.file.levels.len().saturating_sub(1));
    }

    /// The current level ended: move on if it was won and there is more, and say what the
    /// card shows.
    pub fn finish(&mut self, won: bool) -> Intermission {
        if !won {
            Intermission::Retry
        } else if self.current.saturating_add(1) < self.file.levels.len() {
            self.current = self.current.saturating_add(1);
            Intermission::NextLevel
        } else {
            Intermission::Victory
        }
    }
}

/// The weapons the current level hands every player instead of their own (the map's
/// `weapons`). Empty = their own. Read by `spawn_players`.
#[derive(Resource, Debug, Clone, Default)]
pub struct LevelLoadout(pub Vec<String>);

/// The HUD line showing the stories' `ObjectiveText`.
#[derive(Component)]
pub struct HudObjective;

/// The intermission card's body, which waits out [`INPUT_GRACE`] before taking input.
#[derive(Component)]
pub struct IntermissionCard {
    shown_for: f32,
}

pub struct CampaignPlugin;

impl Plugin for CampaignPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CampaignChoice>()
            .init_resource::<Campaign>()
            .init_resource::<LevelLoadout>()
            // Before `Startup`, where `--play --level N` picks the level.
            .add_systems(PreStartup, load_campaign)
            .add_systems(OnEnter(GameState::PlayerSetup), restart_campaign)
            .add_systems(
                OnEnter(GameState::InGame),
                (start_level, spawn_objective_hud).run_if(in_normal_game),
            )
            .add_systems(
                Update,
                (update_objective_hud, win_level_key)
                    .run_if(in_state(GameState::InGame))
                    .run_if(in_normal_game),
            )
            .add_systems(
                OnEnter(GameState::Intermission),
                (spawn_ui_camera, spawn_intermission),
            )
            .add_systems(OnExit(GameState::Intermission), cleanup_state)
            .add_systems(
                Update,
                intermission_input.run_if(in_state(GameState::Intermission)),
            );
    }
}

fn load_campaign(choice: Res<CampaignChoice>, mut campaign: ResMut<Campaign>) {
    *campaign = Campaign::load(&choice.0);
}

/// A new game starts at the first level, with an empty wallet. The campaign file is read
/// again, so edits show up without a restart.
fn restart_campaign(
    choice: Res<CampaignChoice>,
    mut campaign: ResMut<Campaign>,
    mut wallet: ResMut<TeamWallet>,
) {
    *campaign = Campaign::load(&choice.0);
    wallet.coins = 0;
}

/// Load the current level and apply its rules, from a clean slate: nothing the previous
/// level left in the trackers carries over (the coins do, topped up to the level's own).
#[allow(clippy::too_many_arguments)]
pub fn start_level(
    campaign: Res<Campaign>,
    mut tracker: ResMut<LevelTracker>,
    mut waves: ResMut<WaveManager>,
    mut alien_counter: ResMut<AlienCounter>,
    mut wallet: ResMut<TeamWallet>,
    mut loadout: ResMut<LevelLoadout>,
    mut load_map: MessageWriter<LoadMap>,
) {
    let Some(level) = campaign.level() else {
        error!("campaign: no level {}", campaign.current);
        return;
    };
    let map = match crate::map::text_format::load_level(crate::assets::paths::disk_path(&level.map))
    {
        Ok((map, path)) => {
            info!(
                "{}: {} ({})",
                campaign.progress(),
                level.title,
                path.display()
            );
            map
        }
        Err(e) => {
            error!("{e}");
            return;
        }
    };
    let defaults = LevelTracker::default();
    *tracker = LevelTracker {
        level_name: level.title.clone(),
        aliens_win_cut_off: map
            .escape_limit
            .map_or(defaults.aliens_win_cut_off, |limit| {
                i32::try_from(limit).unwrap_or(i32::MAX)
            }),
        ..defaults
    };
    // A map without waves of its own keeps the built-in ones; the map loader replaces
    // these with the map's.
    *waves = WaveManager::default();
    alien_counter.count = 0;
    wallet.coins = wallet.coins.max(map.coins.unwrap_or(0));
    loadout.0.clone_from(&map.weapons);
    load_map.write(LoadMap { map });
}

fn spawn_objective_hud(mut commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands.reborrow(), Some(theme.clone()));
    ui.insert(StateMarker)
        .absolute_position()
        .top(px(46.0))
        .width(percent(100.0))
        .display_flex()
        .justify_center();
    ui.with_child(|c| {
        c.with_text(
            "",
            Some(TextStyle::size_color(16.0, Color::srgb(1.0, 0.95, 0.55))),
        )
        .insert(HudObjective);
    });
    ui.build();
}

fn update_objective_hud(facts: Res<Facts>, mut label: Query<&mut Text, With<HudObjective>>) {
    if !facts.is_changed() {
        return;
    }
    let objective = facts.try_text(keys::OBJECTIVE_TEXT).unwrap_or_default();
    for mut text in &mut label {
        if text.0 != objective {
            objective.clone_into(&mut text.0);
        }
    }
}

/// End wins the level on the spot, for testing the campaign (and for a playtest that has
/// seen enough of a level): the stories take it from there, outro and all.
fn win_level_key(input: Res<ButtonInput<KeyCode>>, mut facts: ResMut<Facts>) {
    if input.just_pressed(KeyCode::End) && facts.bool(keys::LEVEL_STARTED) {
        info!("End: level won");
        facts.set_bool(keys::OBJECTIVES_DONE, true);
        facts.set_bool(keys::LEVEL_COMPLETE, true);
    }
}

/// The card's heading, lines and prompt.
pub fn card_text(campaign: &Campaign, kind: Intermission) -> (String, Vec<String>, String) {
    let level = campaign.level();
    let title = level.map_or_else(String::new, |l| l.title.clone());
    let tagline = level.map_or_else(String::new, |l| l.tagline.clone());
    let kills = format!("Aliens squashed: {}", campaign.last_kills);
    match kind {
        Intermission::NextLevel => (
            campaign.progress().to_uppercase(),
            vec![title, tagline, String::new(), kills],
            "Enter / Fire / Cross: continue      Esc / Circle: menu".into(),
        ),
        Intermission::Retry => (
            "THE ALIENS WIN THIS ONE".into(),
            vec![
                format!("{} - {}", campaign.progress(), title),
                "The neighbourhood has fallen. For now.".into(),
                String::new(),
                kills,
            ],
            "Enter / Fire / Cross: try again      Esc / Circle: menu".into(),
        ),
        Intermission::Victory => {
            let mut lines = campaign.file.victory_lines.clone();
            lines.extend([String::new(), kills]);
            (
                campaign.file.victory_title.clone(),
                lines,
                "Enter / Fire / Cross: back to the menu".into(),
            )
        }
    }
}

fn spawn_intermission(
    commands: Commands,
    theme: Res<LavaTheme>,
    campaign: Res<Campaign>,
    intermission: Option<Res<Intermission>>,
) {
    let kind = intermission.map_or(Intermission::NextLevel, |i| *i);
    let (heading, lines, prompt) = card_text(&campaign, kind);
    let heading_color = match kind {
        Intermission::NextLevel => Color::srgb(0.6, 1.0, 0.7),
        Intermission::Retry => Color::srgb(1.0, 0.45, 0.35),
        Intermission::Victory => Color::srgb(1.0, 0.85, 0.2),
    };

    let mut ui = UIBuilder::new(commands, Some(theme.clone()));
    ui.component::<StateMarker>()
        .insert(IntermissionCard { shown_for: 0.0 })
        .size_percent(100.0, 100.0)
        .display_flex()
        .flex_column()
        .align_items_center()
        .justify_center()
        .row_gap_px(14.0)
        .bg_color(Color::srgb(0.02, 0.04, 0.03));
    ui.with_child(|c| {
        c.with_text(heading, Some(TextStyle::size_color(54.0, heading_color)));
    });
    for (i, line) in lines.into_iter().enumerate() {
        let size = if i == 0 { 30.0 } else { 20.0 };
        ui.with_child(|c| {
            c.with_text(
                line,
                Some(TextStyle::size_color(size, Color::srgb(0.85, 0.95, 0.88))),
            );
        });
    }
    ui.with_child(|c| {
        c.with_text(
            prompt,
            Some(TextStyle::size_color(16.0, Color::srgb(0.55, 0.75, 0.6))),
        )
        .modify_node(|mut n| n.margin.top = Val::Px(30.0));
    });
    ui.build();
}

/// Continue (the next level, a retry) or leave for the menu.
fn intermission_input(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<&Gamepad>,
    intermission: Option<Res<Intermission>>,
    mut cards: Query<&mut IntermissionCard>,
    mut next: ResMut<NextState<GameState>>,
) {
    let Ok(mut card) = cards.single_mut() else {
        return;
    };
    card.shown_for += time.delta_secs();
    if card.shown_for < INPUT_GRACE {
        return;
    }
    let pad = |button| pads.iter().any(|p| p.just_pressed(button));
    let go_on = keys.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || mouse.just_pressed(MouseButton::Left)
        || pad(GamepadButton::South)
        || pad(GamepadButton::Start);
    let leave = keys.just_pressed(KeyCode::Escape) || pad(GamepadButton::East);
    let victory = intermission.is_some_and(|i| *i == Intermission::Victory);
    if leave || (go_on && victory) {
        next.set(GameState::Menu);
    } else if go_on {
        next.set(GameState::InGame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign(levels: usize) -> Campaign {
        Campaign {
            file: CampaignFile {
                levels: (1..=levels)
                    .map(|n| CampaignLevel {
                        map: format!("campaigns/main/level_{n:02}/level_{n:02}.map"),
                        title: format!("Level {n}"),
                        tagline: String::new(),
                    })
                    .collect(),
                ..CampaignFile::default()
            },
            ..Campaign::default()
        }
    }

    #[test]
    fn winning_moves_on_losing_retries_and_the_last_win_is_victory() {
        let mut c = campaign(3);
        assert_eq!(c.finish(false), Intermission::Retry);
        assert_eq!(c.current, 0, "a loss stays on the level");
        assert_eq!(c.finish(true), Intermission::NextLevel);
        assert_eq!(c.finish(true), Intermission::NextLevel);
        assert_eq!(c.current, 2);
        assert_eq!(c.finish(true), Intermission::Victory);
        assert_eq!(c.current, 2);
        c.go_to(99);
        assert_eq!(c.current, 2, "clamped");
        c.go_to(1);
        assert_eq!(c.progress(), "Level 1 of 3");
    }

    /// Every enemy spawn has a walkable way to the goal (four-way steps over tiles that
    /// are neither void nor blocked to enemies).
    fn goal_reachable(tiles: &[Vec<u64>]) -> bool {
        use crate::map::MapFeatures;
        let at = |x: usize, y: usize| tiles.get(y).and_then(|r| r.get(x)).copied().unwrap_or(0);
        let walkable = |t: u64| t != 0 && t & MapFeatures::ImpassableForEnemies as u64 == 0;
        let find = |flag: MapFeatures| -> Vec<(usize, usize)> {
            tiles
                .iter()
                .enumerate()
                .flat_map(|(y, row)| {
                    row.iter()
                        .enumerate()
                        .filter(move |(_, t)| *t & flag as u64 != 0)
                        .map(move |(x, _)| (x, y))
                })
                .collect()
        };
        let goals = find(MapFeatures::EnemyExit);
        find(MapFeatures::EnemySpawn).into_iter().all(|start| {
            let mut seen = std::collections::HashSet::from([start]);
            let mut queue = std::collections::VecDeque::from([start]);
            while let Some((x, y)) = queue.pop_front() {
                if goals.contains(&(x, y)) {
                    return true;
                }
                let steps = [
                    (x.wrapping_sub(1), y),
                    (x.saturating_add(1), y),
                    (x, y.wrapping_sub(1)),
                    (x, y.saturating_add(1)),
                ];
                for next in steps {
                    if walkable(at(next.0, next.1)) && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
            false
        })
    }

    #[test]
    fn the_shipped_campaigns_read_and_every_level_loads() {
        use crate::assets::content::{ContentKind, campaign_files};
        let files = campaign_files(ContentKind::Campaign);
        assert!(
            files
                .iter()
                .any(|f| *f == std::path::Path::new(&campaign_file(DEFAULT_CAMPAIGN))),
            "the default campaign is there"
        );
        for path in files {
            let text = std::fs::read_to_string(&path).expect("campaign file");
            let file: CampaignFile = ron::from_str(&text).expect("campaign parses");
            assert!(!file.levels.is_empty(), "{}: no levels", path.display());
            for level in &file.levels {
                check_level(level);
            }
        }
    }

    /// A level loads as the game plays it and holds together: a classic level has a
    /// spawn, a goal the spawns can reach; an infestation level needs neither.
    fn check_level(level: &CampaignLevel) {
        use crate::assets::paths::disk_path;
        {
            let (map, _) = crate::map::text_format::load_level(disk_path(&level.map))
                .unwrap_or_else(|e| panic!("{}: {e}", level.map));
            let flat: Vec<u64> = map.tiles.iter().flatten().copied().collect();
            let has = |flag: crate::map::MapFeatures| flat.iter().any(|t| t & flag as u64 != 0);
            assert!(
                has(crate::map::MapFeatures::PlayerSpawn),
                "{}: player spawn",
                level.map
            );
            let classic = map.infestation.is_none();
            assert!(
                !classic || has(crate::map::MapFeatures::EnemySpawn),
                "{}: enemy spawn",
                level.map
            );
            assert!(
                !classic || has(crate::map::MapFeatures::EnemyExit),
                "{}: goal",
                level.map
            );
            assert!(!map.waves.is_empty(), "{}: waves", level.map);
            let holes = crate::map::walls::enclosed_voids(&map);
            assert!(
                holes.is_empty(),
                "{}: void inside the map at {holes:?}",
                level.map
            );
            assert!(
                !classic || goal_reachable(&map.tiles),
                "{}: an enemy spawn cannot reach the goal",
                level.map
            );
            if !map.stories.is_empty() {
                let stories = turbofacts::persistence::load_stories(disk_path(&map.stories))
                    .unwrap_or_else(|e| panic!("{}: {e}", map.stories));
                assert!(!stories.is_empty(), "{}: no stories", map.stories);
            }
            for def in map
                .weapons
                .iter()
                .chain(map.waves.iter().map(|w| &w.enemy_def))
            {
                assert!(
                    def.is_empty() || disk_path(def).exists(),
                    "{}: {def} does not exist",
                    level.map
                );
            }
            for p in &map.placements {
                assert!(
                    disk_path(&p.def_path).exists(),
                    "{}: {} does not exist",
                    level.map,
                    p.def_path
                );
            }
        }
    }
}
