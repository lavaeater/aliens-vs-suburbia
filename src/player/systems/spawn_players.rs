use bevy::math::{Quat, Vec3};
use bevy::prelude::{Children, Commands, Component, DetectChanges, Entity, MessageReader, MessageWriter,
                    Assets, Query, Res, ResMut, Transform, Visibility, With};
use bevy::scene::SceneRoot;
use avian3d::prelude::Collider;
use bevy_wind_waker_shader::WindWakerShaderBuilder;
use crate::assets::assets_plugin::GameAssets;
use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
use crate::general::events::map_events::SpawnPlayer;
use crate::model_settings::resources::ModelSettings;
use crate::player::bundle::PlayerBundle;
use crate::sprite_billboard::components::{BillboardMeshHandle, SpriteBillboard};
use crate::sprite_billboard::material::SpriteBillboardMaterial;
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
    config: Option<Res<CharacterConfig>>,
    sheet: Option<Res<ComposedSpriteSheet>>,
    billboard_mesh: Option<Res<BillboardMeshHandle>>,
    mut sprite_materials: ResMut<Assets<SpriteBillboardMaterial>>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut player_added_mw: MessageWriter<GameTrackingEvent>,
) {
    for spawn_player in spawn_player_event_reader.read() {
        let pos = Transform::from_xyz(
            spawn_player.position.x,
            spawn_player.position.y,
            spawn_player.position.z,
        );

        // Decide: use sprite billboard or 3D model?
        let use_billboard = config.as_ref()
            .map(|c| !c.body_type.is_empty())
            .unwrap_or(false);
        let billboard_sheet = use_billboard
            .then(|| sheet.as_ref().and_then(|s| s.billboard_handle.clone()))
            .flatten();

        let player = if use_billboard && billboard_sheet.is_some() && billboard_mesh.is_some() {
            let sheet_handle = billboard_sheet.unwrap();
            let mesh_handle = billboard_mesh.as_ref().unwrap().0.clone();

            let mat = sprite_materials.add(SpriteBillboardMaterial {
                sprite_sheet: sheet_handle,
                uv_rect: bevy::prelude::Vec4::new(0.0, 0.5, 64.0 / 704.0, 64.0 / 256.0),
            });

            let mut entity_cmds = commands.spawn((
                pos,
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
                        CollisionLayer::AlienGoal,
                    ],
                ),
            ));
            // Spawn billboard as a child.
            let parent_id = entity_cmds.id();
            commands.entity(parent_id).with_children(|parent| {
                parent.spawn((
                    SpriteBillboard::default(),
                    bevy::prelude::Mesh3d(mesh_handle),
                    bevy::prelude::MeshMaterial3d(mat),
                    bevy::prelude::Transform::from_xyz(0.0, 0.25, 0.0),
                ));
            });
            parent_id
        } else {
            // 3D model path (original behaviour).
            let s = &*model_settings;
            commands.spawn((
                FixSceneTransform::new(
                    Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                    Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                    Vec3::splat(s.scale),
                ),
                SceneRoot(game_assets.player_scene.clone()),
                pos,
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
                        CollisionLayer::AlienGoal,
                    ],
                ),
                WindWakerShaderBuilder::default().build(),
            )).id()
        };

        add_health_bar_mw.write(AddHealthBar { entity: player, name: "PLAYER" });
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
