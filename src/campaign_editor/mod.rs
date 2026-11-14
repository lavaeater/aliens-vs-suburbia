//! The campaign editor (`GameState::CampaignEditor`, `cargo run -- --campaign-editor
//! [name]`): one base map (an imported suburb, `map::compose`) cut into the campaign's
//! levels. Draw, move and resize each level's region on the map, put its player starts,
//! alien spawns and goal, see whether every spawn reaches the goal, and set its title and
//! rules. Plan: `docs/playtest.md`, *GIS campaign*.
//!
//! - `doc.rs`: the campaign being edited ([`doc::CampaignDoc`]), pure and tested.
//! - `view.rs`: the world view, its camera and the mouse tools.
//! - `analysis.rs`: the selected level composed and walked from its goal.
//! - `panel.rs`: the side panel and the keys.
//! - `play.rs`: F5 plays the selected level and comes back here afterwards.

pub mod analysis;
pub mod catalog;
pub mod doc;
pub mod intro;
pub mod panel;
pub mod play;
pub mod view;

use bevy::prelude::*;

use crate::campaign::CampaignChoice;
use crate::game_state::GameState;
use crate::ui::spawn_ui::cleanup_state;

pub struct CampaignEditorPlugin;

impl Plugin for CampaignEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<doc::CampaignDoc>()
            .init_resource::<view::Tool>()
            .init_resource::<view::PlaceTurn>()
            .init_resource::<catalog::Catalog>()
            .init_resource::<view::Drag>()
            .init_resource::<view::Hover>()
            .init_resource::<analysis::Analysis>()
            .init_resource::<panel::TextFocus>()
            .init_resource::<panel::FrameOn>()
            .init_resource::<panel::Confirm>()
            .add_systems(
                OnEnter(GameState::CampaignEditor),
                (
                    open_campaign,
                    play::reselect_played,
                    view::spawn_world,
                    panel::spawn_panel,
                )
                    .chain(),
            )
            .add_systems(OnEnter(GameState::Menu), play::back_to_editor)
            .add_systems(OnEnter(GameState::Intermission), play::back_to_editor)
            .add_systems(OnExit(GameState::CampaignEditor), cleanup_state)
            .add_systems(
                Update,
                (
                    view::track_hover,
                    panel::handle_keys,
                    play::play_key,
                    view::pan_and_zoom,
                    view::use_tool,
                    analysis::update_analysis,
                    panel::frame_region,
                    view::draw,
                    view::draw_labels,
                    panel::rebuild_panel,
                )
                    .chain()
                    .run_if(in_state(GameState::CampaignEditor)),
            );
    }
}

/// `--campaign-editor <name> --level N`: open with level N (1-based) selected.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OpenAtLevel(pub usize);

/// Read the campaign being played (`--campaign`, else the default) into the editor.
#[allow(clippy::too_many_arguments)]
fn open_campaign(
    choice: Res<CampaignChoice>,
    mut doc: ResMut<doc::CampaignDoc>,
    mut tool: ResMut<view::Tool>,
    mut drag: ResMut<view::Drag>,
    mut focus: ResMut<panel::TextFocus>,
    mut confirm: ResMut<panel::Confirm>,
    mut frame: ResMut<panel::FrameOn>,
    open_at: Option<Res<OpenAtLevel>>,
    mut catalog: ResMut<catalog::Catalog>,
) {
    *catalog = catalog::Catalog::load();
    *doc = doc::CampaignDoc::load(&choice.0)
        .unwrap_or_else(|e| doc::CampaignDoc::failed(&choice.0, e));
    if let Some(at) = open_at {
        let index = at.0.saturating_sub(1);
        doc.select(Some(index));
        frame.0 = doc.region_of(index);
    }
    *tool = view::Tool::Select;
    *drag = view::Drag::None;
    *focus = panel::TextFocus::default();
    *confirm = panel::Confirm::default();
}
