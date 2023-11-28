use belly::build::{eml, FromWorldAndParams, widget, WidgetContext};
use belly::core::eml::Params;
use bevy::prelude::Commands;
use belly::prelude::*;
use bevy::prelude::*;
use crate::camera::components::GameCamera;
use crate::game_state::GameState;
use crate::general::components::Health;

#[derive(Event)]
pub struct GotoState {
    pub state: GameState,
}

pub fn spawn_menu(
    mut commands: Commands,
) {
    commands.add(eml! {
        <body>
          <button on:press=|ctx| ctx.send_event(GotoState { state: GameState::InGame })>
                    "Start Game"
                </button>
        </body>
    });
}

pub fn goto_state_system(
    mut state: ResMut<NextState<GameState>>,
    mut goto_state_er: EventReader<GotoState>,
) {
    for goto_state in &mut goto_state_er.read() {
        state.set(goto_state.state.clone());
    }
}

pub fn cleanup_menu(
    mut commands: Commands,
    entities: Query<Entity, Without<Window>>,
) {
    for entity in entities.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

pub fn spawn_ui(mut commands: Commands) {
    commands.add(ess! {
        body {
            // Use the CSS Grid algorithm for laying out this node
            display: grid;
            // Set the grid to have 2 columns with sizes [min-content, minmax(0, 1fr)]
            // - The first column will size to the size of it's contents
            // - The second column will take up the remaining available space
            grid-template-columns: 100%;//min-content; // flex(1.0)
            // Set the grid to have 3 rows with sizes [auto, minmax(0, 1fr), 20px]
            // - The first row will size to the size of it's contents
            // - The second row take up remaining available space (after rows 1 and 3 have both been sized)
            // - The third row will be exactly 20px high
            grid-template-rows: 20% 60% 20%;
            // background-color: white;
        }
        .header {
            // Make this node span two grid columns so that it takes up the entire top tow
            // grid-column: span 2;
            height: 100%;
            font: bold;
            font-size: 8px;
            color: black;
            display: grid;
            padding: 6px;
        }
        .main {
            // Use grid layout for this node
            display: grid;
            height: 100%;
            width: 100%;
            padding: 24px;
            // grid-template-columns: repeat(4, flex(1.0));
            // grid-template-rows: repeat(4, flex(1.0));
            // row-gap: 12px;
            // column-gap: 12px;
            // background-color: #2f2f2f;
        }
        // Note there is no need to specify the position for each grid item. Grid items that are
        // not given an explicit position will be automatically positioned into the next available
        // grid cell. The order in which this is performed can be controlled using the grid_auto_flow
        // style property.
        .cell {
            display: grid;
        }
        // .sidebar {
        //     display: grid;
        //     background-color: black;
        //     // Align content towards the start (top) in the vertical axis
        //     align-items: start;
        //     // Align content towards the center in the horizontal axis
        //     justify-items: center;
        //     padding: 10px;
        //     // Add an fr track to take up all the available space at the bottom of the column so
        //     // that the text nodes can be top-aligned. Normally you'd use flexbox for this, but
        //     // this is the CSS Grid example so we're using grid.
        //     grid-template-rows: auto auto 1fr;
        //     row-gap: 10px;
        //     height: 5%;
        // }
        .text-header {
            font: bold;
            font-size: 24px;
        }
        .footer {
            font: bold;
            font-size: 24px;
            display: grid;
            height: 100%;
            width: 100%;
            padding: 24px;
            grid-template-columns: repeat(4, flex(1.0));
            grid-template-rows: repeat(4, flex(1.0));
            row-gap: 12px;
            column-gap: 12px;
            background-color: #2f2f2faa;
        }
    });
    commands.add(eml! {
        <body>
            <span c:header></span>
            <span c:main>
            </span>
            <span c:footer id="ui-footer">
                // <for color in=COLORS>
                //     <span c:cell s:background-color=color/>
                // </for>
            </span>
        </body>
    });
}

#[derive(Event)]
pub struct AddHealthBar {
    pub entity: Entity,
    pub name: &'static str,
}

pub fn add_health_bar(
    mut elements: Elements,
    mut add_health_bar_er: EventReader<AddHealthBar>,
) {
    for add_health_bar in &mut add_health_bar_er.read() {
        let erp = add_health_bar.entity;
        elements.select("body").add_child(eml! {
                <fellow target=erp>
                    <span><progressbar s:width="50px" maximum=100.0 minimum=0.0 bind:value=from!(erp, Health:as_f32()) s:color="#00ff00" /></span>
                </fellow>
        });
    }
}

#[derive(Component)]
pub struct Fellow {
    pub target: Entity,
}


#[widget]
#[param(target: Entity => Fellow: target)]
fn fellow(ctx: &mut WidgetContext) {
    let content = ctx.content();
    ctx.render(eml! {
        <span s:left=managed() s:top=managed() s:position-type="absolute">
            {content}
        </span>
    })
}

impl FromWorldAndParams for Fellow {
    fn from_world_and_params(_: &mut World, params: &mut Params) -> Self {
        Fellow {
            target: params.try_get("target").expect("Missing required `target` param")
        }
    }
}

pub fn fellow_system(
    mut fellows: Query<(Entity, &Fellow, &mut Style, &Node)>,
    transforms: Query<&GlobalTransform>,
    mut commands: Commands,
    camera_q: Query<(&Camera, &GlobalTransform), With<GameCamera>>,
) {
    if let Ok((camera, camera_global_transform)) = camera_q.get_single() {
        for (entity, follow, mut style, node) in fellows.iter_mut() {
            let Ok(tr) = transforms.get(follow.target) else {
                commands.entity(entity).despawn_recursive();
                continue;
            };
            if let Some(pos) = camera.world_to_viewport(camera_global_transform, tr.translation()) {
                style.left = Val::Px((pos.x - 0.5 * node.size().x).round());
                style.top = Val::Px((pos.y - 0.5 * node.size().y).round());
            }
        }
    }
}