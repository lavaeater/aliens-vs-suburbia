use belly::build::{eml, StyleSheet};
use bevy::prelude::Commands;
use belly::prelude::*;
use bevy::prelude::*;
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
    commands.add(StyleSheet::load("belly/color-picker.ess"));

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
                <follow target=erp>
                    <label value=name />
                    <progressbar s:width="50px" maximum=100.0 minimum=0.0 bind:value=from!(erp, Health:as_f32()) s:color="#00ff00" />
                </follow>
            });
    }
}
