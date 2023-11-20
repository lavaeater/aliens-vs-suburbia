use bevy::asset::AssetServer;
use bevy::core::Name;
use bevy::hierarchy::BuildChildren;
use bevy::prelude::{Commands, EventReader, Res};
use bevy::scene::SceneBundle;
use bevy::utils::hashbrown::HashSet;
use bevy_xpbd_3d::components::{Collider, Position};
use bevy_xpbd_3d::prelude::CollisionLayers;
use big_brain::actions::Steps;
use big_brain::pickers::Highest;
use big_brain::thinker::{Thinker, ThinkerBuilder};
use crate::general::components::{CollisionLayer, Health};
use crate::general::components::map_components::{CurrentTile, ModelDefinitions};
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::general::IsObstacle;
use crate::towers::components::{AlienInRangeScore, ShootAlienAction, ShootAlienData, TowerShootyBit};
use crate::towers::events::BuildTower;

pub fn build_tower_system(
    mut build_tower_er: EventReader<BuildTower>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    model_defs: Res<ModelDefinitions>,
    tile_defs: Res<TileDefinitions>,
) {
    for build_tower in build_tower_er.read() {
        let model_def = model_defs.definitions.get(build_tower.model_definition_key).unwrap();
        let mut ec = commands.spawn((
            Name::from(model_def.name),
            IsObstacle {}, // let this be, for now!
            SceneBundle {
                scene: asset_server.load(model_def.file),
                ..Default::default()
            },
            model_def.rigid_body,
            tile_defs.create_collider(model_def.width, model_def.height, model_def.depth),
            Position::from(build_tower.position),
            model_def.create_collision_layers(),
            CurrentTile::default(),
            Health::default(),
        ));
        if build_tower.model_definition_key == "tower" {
            ec.with_children(|parent| {
                parent.spawn((
                    Name::from("Sensor"),
                    Collider::cylinder(0.5, 5.0),
                    CollisionLayers::new([CollisionLayer::Sensor], [CollisionLayer::Alien]),
                    Position::from(build_tower.position),
                    TowerShootyBit {},
                    ShootAlienData {
                        aliens_in_range: HashSet::new()
                    },
                    //create_thinker()
                ));
            });
        }
    }
}

pub fn alien_sensor_collisions() {

}

pub fn create_thinker() -> ThinkerBuilder {
    Thinker::build()
        .label("Tower Thinker")
        .picker(Highest {})
        .when(
            AlienInRangeScore,
            Steps::build()
                .label("Shoot Closest Alien")
                .step(ShootAlienAction),
        )
}