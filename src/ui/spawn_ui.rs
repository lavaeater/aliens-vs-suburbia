use belly::build::{eml, FromWorldAndParams, StyleSheet, widget, WidgetContext};
use belly::core::eml::Params;
use bevy::prelude::Commands;
use belly::prelude::*;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use crate::camera::components::camera::GameCamera;
use crate::general::components::Health;
//
// const COLORS: &[&'static str] = &[
//     // from https://colorswall.com/palette/105557
//     // Red     Pink       Purple     Deep Purple
//     "#f44336", "#e81e63", "#9c27b0", "#673ab7",
//     // Indigo  Blue       Light Blue Cyan
//     "#3f51b5", "#2196f3", "#03a9f4", "#00bcd4",
//     // Teal    Green      Light      Green Lime
//     "#009688", "#4caf50", "#8bc34a", "#cddc39",
//     // Yellow  Amber      Orange     Deep Orange
//     "#ffeb3b", "#ffc107", "#ff9800", "#ff5722",
// ];

pub fn spawn_ui(mut commands: Commands) {
    // commands.add(StyleSheet::load("belly/color-picker.ess"));
    //
    commands.add(eml! {
        <body>
        </body>
    });
    //         <span c:controls>
    //             <slider c:red
    //                 bind:value=to!(colorbox, BackgroundColor:0|r)
    //                 bind:value=from!(colorbox, BackgroundColor:0.r())
    //             />
    //             <slider c:green
    //                 bind:value=to!(colorbox, BackgroundColor:0|g)
    //                 bind:value=from!(colorbox, BackgroundColor:0.g())
    //             />
    //             <slider c:blue
    //                 bind:value=to!(colorbox, BackgroundColor:0|b)
    //                 bind:value=from!(colorbox, BackgroundColor:0.b())
    //             />
    //             <slider c:alpha
    //                 bind:value=to!(colorbox, BackgroundColor:0|a)
    //                 bind:value=from!(colorbox, BackgroundColor:0.a())
    //             />
    //         </span>
    //         <img c:colorbox-holder src="belly/trbg.png">
    //             <span {colorbox} c:colorbox s:background-color=managed()
    //                 on:ready=run!(|c: &mut BackgroundColor| c.0 = Color::WHITE)/>
    //         </img>
    //         <span c:colors>
    //         <for color in = COLORS>
    //             <button on:press=run!(for colorbox |c: &mut BackgroundColor| { c.0 = Color::from_hex(color) })>
    //                 <span s:background-color=*color s:width="100%" s:height="100%"/>
    //             </button>
    //         </for>
    //         </span>
    //     </body>
    // });
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
        let name = add_health_bar.name;
        elements.select("body").add_child(eml! {
                <fellow target=erp>
                    <span><label value=name /></span>
                    <span><progressbar s:width="50px" maximum=100.0 minimum=0.0 bind:value=from!(erp, Health:as_f32()) s:color="#00ff00" /></span>
                </fellow>
        });
    }
}

#[derive(Component)]
pub struct Fellow {
    pub target: Entity
}


#[widget]
#[param(target:Entity => Fellow:target)]
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
    camera_q: Query<(&Camera, &GlobalTransform),With<GameCamera>>,
) {
    if let Ok((camera, camera_global_transform)) =camera_q.get_single() {
        for (entity, follow, mut style, node) in fellows.iter_mut() {
            let Ok(tr) = transforms.get(follow.target) else {
                commands.entity(entity).despawn_recursive();
                continue;
            };
            let pos = camera.world_to_viewport(camera_global_transform, tr.translation()).unwrap();
            style.left = Val::Px(pos.x.round());
            style.top = Val::Px(pos.y.round());
        }
    }
}