use bevy::math::{Quat, Vec3};
use bevy::prelude::{Children, Commands, Component, DetectChanges, Entity, MessageReader, MessageWriter, Query,
                    Res, ResMut, Transform, Visibility, With};
use bevy::scene::SceneRoot;
use bevy_mod_outline::OutlineVolume;
use avian3d::prelude::Collider;
use bevy_wind_waker_shader::{TimeOfDay, Weather, WindWakerShaderBuilder};
use crate::assets::assets_plugin::GameAssets;
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
use crate::general::events::map_events::SpawnPlayer;
use crate::model_settings::resources::ModelSettings;
use crate::player::bundle::PlayerBundle;
use crate::ui::spawn_ui::AddHealthBar;

#[derive(Component)]
pub struct FixSceneTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl FixSceneTransform {
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }
}

pub fn spawn_players(
    mut spawn_player_event_reader: MessageReader<SpawnPlayer>,
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    model_settings: Res<ModelSettings>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut player_added_mw: MessageWriter<GameTrackingEvent>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        let s = &*model_settings;
        let player = commands.spawn((
            FixSceneTransform::new(
                Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                Vec3::splat(s.scale),
            ),
            SceneRoot(game_assets.player_scene.clone()),
            Transform::from_xyz(spawn_player.position.x, spawn_player.position.y, spawn_player.position.z),
            Collider::cuboid(0.5, 0.5, 0.45),
            PlayerBundle::new(
                "player",
                "Player One",
                [CollisionLayer::Player],
                [
                    CollisionLayer::Ball,
                    CollisionLayer::Impassable,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal
                ],
            ),
            WindWakerShaderBuilder::default()
              .build(),
            // OutlineVolume {
            //     visible: true,
            //     width: 1.0,
            //     colour: bevy::prelude::Color::BLACK,
            // },
        )).id();
        add_health_bar_mw.write(AddHealthBar {
            entity: player,
            name: "PLAYER",
        });
        player_added_mw.write(GameTrackingEvent::PlayerAdded(player));
    }
}

/// Marker placed on the direct scene-root child of the player so we can retarget it later.
#[derive(Component)]
pub struct PlayerModelRoot;

pub fn fix_scene_transform(
    mut commands: Commands,
    mut scene_instance_query: Query<(Entity, &FixSceneTransform, &Children)>,
    mut child_query: Query<&mut Transform, With<Visibility>>,
) {
    for (parent, fix_scene_transform, children) in scene_instance_query.iter_mut() {
        for child in children.iter() {
            if let Ok(mut transform) = child_query.get_mut(*child) {
                transform.translation = fix_scene_transform.translation;
                transform.rotation = fix_scene_transform.rotation;
                transform.scale = fix_scene_transform.scale;
                commands.entity(*child).insert(PlayerModelRoot);
                commands.entity(parent).remove::<FixSceneTransform>();
            }
        }
    }
}

pub fn apply_model_settings_live(
    model_settings: Res<ModelSettings>,
    mut root_query: Query<&mut Transform, With<PlayerModelRoot>>,
) {
    if !model_settings.is_changed() { return; }
    let s = &*model_settings;
    for mut transform in root_query.iter_mut() {
        transform.translation = Vec3::new(s.translation_x, s.translation_y, s.translation_z);
        transform.rotation = Quat::from_rotation_y(s.rotation_y_degrees.to_radians());
        transform.scale = Vec3::splat(s.scale);
    }
}
