//! Session marker for the character editor, plus the run conditions that tell a sandbox
//! session apart from a real match.

use bevy::prelude::*;

/// Present exactly while a character editor session is live. Inserted by the menu button just
/// before it switches to `GameState::InGame`, removed on the way out.
///
/// Its presence is the *only* thing that distinguishes the two kinds of `InGame` session,
/// so anything that must not happen in a sandbox checks for it.
#[derive(Resource, Debug, Default)]
pub struct EditorSession {
    /// Def path (`assets/defs/foo.ron`) of the model the player is currently wearing.
    /// `None` until stage 2 lets you pick one; the roster/default is used meanwhile.
    #[allow(dead_code)]
    pub selected_def: Option<String>,
}

/// True while a character editor session is live.
pub const fn in_editor(session: Option<Res<EditorSession>>) -> bool {
    session.is_some()
}

/// True while a *real* match is running — i.e. `InGame` without an editor session.
///
/// Note this is not simply `!in_editor`: it is false outside `InGame` entirely, so it
/// can be used on its own without an extra `in_state` guard.
pub fn in_normal_game(
    session: Option<Res<EditorSession>>,
    state: Res<State<crate::game_state::GameState>>,
) -> bool {
    session.is_none() && *state.get() == crate::game_state::GameState::InGame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::GameState;

    fn app_in(state: GameState, editor: bool) -> App {
        let mut app = App::new();
        app.insert_resource(State::new(state));
        if editor {
            app.init_resource::<EditorSession>();
        }
        app
    }

    #[test]
    fn a_sandbox_session_is_the_editor_but_not_a_normal_game() {
        let mut app = app_in(GameState::InGame, true);
        assert!(app.world_mut().run_system_cached(in_editor).unwrap());
        assert!(!app.world_mut().run_system_cached(in_normal_game).unwrap());
    }

    #[test]
    fn a_real_match_is_a_normal_game_but_not_the_editor() {
        let mut app = app_in(GameState::InGame, false);
        assert!(!app.world_mut().run_system_cached(in_editor).unwrap());
        assert!(app.world_mut().run_system_cached(in_normal_game).unwrap());
    }

    /// `in_normal_game` guards `OnEnter`-style work that must not fire in other states,
    /// so "not the editor" alone is not enough — the state has to match too.
    #[test]
    fn the_menu_is_neither() {
        let mut app = app_in(GameState::Menu, false);
        assert!(!app.world_mut().run_system_cached(in_editor).unwrap());
        assert!(!app.world_mut().run_system_cached(in_normal_game).unwrap());
    }
}
