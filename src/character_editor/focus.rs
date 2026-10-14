//! Who gets the keyboard and the pointer: the editor's panels or the character.
//!
//! The editor is the live game with a UI laid over it, so every click on a panel is also
//! a mouse movement the game would happily aim at, and every key typed into a field is a
//! key the game would walk on. This decides, once per frame, which side gets them, and
//! tells the game through `control::components::GameInputBlock`.
//!
//! - **Pointer over a panel** (the left pane, or the floating F1/F2/F10 panels): the aim
//!   stays where it was, so the character and its gun stop swinging toward the buttons.
//!   The keyboard still walks — the panels have no keyboard shortcuts of their own yet, and
//!   losing WASD because the cursor happens to rest on the panel would be a surprise.
//! - **A text field has focus** ([`EditorInputFocus::text_capture`]): the keyboard is the
//!   field's. Nothing sets it yet; the Library search and the clip-tag editor will.
//! - **Inspecting** (orbit camera or rest pose): the keyboard and mouse belong to the
//!   camera and the editor, and the character holds still (`inspect::hold_player_still`).
//!
//! A gamepad is never blocked: it cannot click a panel or type into a field.

use bevy::picking::hover::HoverMap;
use bevy::prelude::*;

use crate::character_editor::debug::EditorOverlays;
use crate::character_editor::ui::EditorViewportPane;
use crate::control::components::GameInputBlock;
use crate::ui::spawn_ui::StateMarker;

/// Editor-side input state that is not visible from the hover map.
#[derive(Resource, Debug, Default)]
pub struct EditorInputFocus {
    /// A text field is being typed into; every key belongs to it until Enter or Escape.
    pub text_capture: bool,
}

/// What the game may have this frame, from where the pointer is and what the editor is
/// doing. Pure, so the rules can be tested without a window.
pub const fn decide_block(
    pointer_over_panel: bool,
    text_capture: bool,
    inspecting: bool,
) -> GameInputBlock {
    GameInputBlock {
        keyboard: text_capture || inspecting,
        mouse_aim: pointer_over_panel || text_capture || inspecting,
    }
}

/// Is the pointer over a panel, as opposed to the game viewport or nothing at all?
///
/// A panel is any UI node under a screen root (`StateMarker`): the editor's own tree and
/// the floating settings panels. The viewport pane is part of that tree but is the game,
/// so it is excluded; health bars and other world-following UI have no such root, so the
/// cursor passing over the player's health bar does not freeze the aim.
pub fn pointer_over_panel(
    hover_map: &HoverMap,
    is_viewport: impl Fn(Entity) -> bool,
    root_of: impl Fn(Entity) -> Entity,
    is_screen_root: impl Fn(Entity) -> bool,
) -> bool {
    hover_map.values().any(|hits| {
        hits.keys()
            .any(|&entity| !is_viewport(entity) && is_screen_root(root_of(entity)))
    })
}

pub fn update_game_input_block(
    hover_map: Res<HoverMap>,
    focus: Res<EditorInputFocus>,
    overlays: Res<EditorOverlays>,
    panes: Query<(), With<EditorViewportPane>>,
    roots: Query<(), With<StateMarker>>,
    parents: Query<&ChildOf>,
    mut block: ResMut<GameInputBlock>,
) {
    let over_panel = pointer_over_panel(
        &hover_map,
        |entity| panes.contains(entity),
        |entity| parents.root_ancestor(entity),
        |entity| roots.contains(entity),
    );
    let wanted = decide_block(over_panel, focus.text_capture, overlays.inspecting());
    // Compared first so the resource only reads as changed when the answer does.
    if *block != wanted {
        *block = wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_the_viewport_the_game_gets_everything() {
        assert_eq!(decide_block(false, false, false), GameInputBlock::default());
    }

    /// The point of the whole module: clicking a button must not swing the gun at it.
    #[test]
    fn over_a_panel_the_aim_holds_but_the_keys_still_walk() {
        let block = decide_block(true, false, false);
        assert!(block.mouse_aim);
        assert!(!block.keyboard);
    }

    #[test]
    fn typing_into_a_field_takes_the_keyboard_wherever_the_pointer_is() {
        assert!(decide_block(false, true, false).keyboard);
        assert!(decide_block(true, true, false).keyboard);
    }

    #[test]
    fn inspecting_takes_both() {
        let block = decide_block(false, false, true);
        assert!(block.keyboard && block.mouse_aim);
    }
}
