use belly::build::{eml, FromWorldAndParams, widget, WidgetContext};
use belly::core::eml::Params;
use bevy::prelude::Commands;
use belly::prelude::*;
use bevy::prelude::*;
use crate::camera::components::camera::GameCamera;
use crate::general::components::Health;


pub fn spawn_ui(mut commands: Commands) {
    commands.add(eml! {
        <body>
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
        for (entity, follow, mut style, _node) in fellows.iter_mut() {
            let Ok(tr) = transforms.get(follow.target) else {
                commands.entity(entity).despawn_recursive();
                continue;
            };
            if let Some(pos) = camera.world_to_viewport(camera_global_transform, tr.translation()) {
                style.left = Val::Px(pos.x.round());
                style.top = Val::Px(pos.y.round());
            }
        }
    }
}