use bevy::prelude::States;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    Menu,
    InGame,
}

pub(crate) mod menu {
    use belly::prelude::*;
    use bevy::prelude::*;
    use bevy::prelude::{Commands, ResMut};
    use crate::general::game_state::AppState;


    fn setup_menu(mut commands: Commands) {
        commands.add(StyleSheet::load("party-editor/styles.ess"));
        commands.add(eml! {
        <body>
        </body>
    });
    }

    // fn menu(
    //     mut state: ResMut<State<AppState>>,
    //     mut interaction_query: Query<
    //         (&Interaction, &mut BackgroundColor),
    //         (Changed<Interaction>, With<Button>),
    //     >,
    // ) {
    //     for (interaction, mut color) in &mut interaction_query {
    //         match *interaction {
    //             Interaction::Clicked => {
    //                 *color = PRESSED_BUTTON.into();
    //                 state.set(AppState::InGame).unwrap();
    //             }
    //             Interaction::Hovered => {
    //                 *color = HOVERED_BUTTON.into();
    //             }
    //             Interaction::None => {
    //                 *color = NORMAL_BUTTON.into();
    //             }
    //         }
    //     }
    // }

    fn cleanup_menu(mut commands: Commands, menu_data: Res<MenuData>) {
        commands.entity(menu_data.button_entity).despawn_recursive();
    }
}