use bevy::math::{Quat, Vec3};
use bevy::prelude::*;
use bevy::asset::AssetServer;
use bevy::gltf::GltfAssetLabel;
use bevy::world_serialization::WorldAssetRoot;
use avian3d::prelude::Collider;
use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::assets::assets_plugin::GameAssets;
use crate::control::gamepad_input::WantsGamepad;
use crate::player::systems::equip::PendingEquip;
use crate::player_setup::state::InputDevice;
pub use crate::player::components::WeaponsHidden;
use crate::character_creator::config::{CharacterConfig, ComposedSpriteSheet};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
use crate::general::events::map_events::SpawnPlayer;
use crate::model_settings::plugin::PlayerAssetDef;
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

#[allow(clippy::too_many_arguments)]
pub fn spawn_players(
    mut spawn_player_event_reader: MessageReader<SpawnPlayer>,
    mut commands: Commands,
    mut game_assets: ResMut<GameAssets>,
    mut player_asset_def: ResMut<PlayerAssetDef>,
    model_settings: Res<ModelSettings>,
    asset_server: Res<AssetServer>,
    roster: Option<Res<crate::player_setup::state::PlayerRoster>>,
    existing_players: Query<(), With<crate::player::components::Player>>,
    config: Option<Res<CharacterConfig>>,
    sheet: Option<Res<ComposedSpriteSheet>>,
    billboard_mesh: Option<Res<BillboardMeshHandle>>,
    mut sprite_materials: ResMut<Assets<SpriteBillboardMaterial>>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut player_added_mw: MessageWriter<GameTrackingEvent>,
) {
    let max_players = roster.as_ref()
        .map(|r| r.def_paths.len().max(1))
        .unwrap_or(1);
    let current_count = existing_players.iter().count();
    let spawn_events: Vec<_> = spawn_player_event_reader.read()
        .take(max_players.saturating_sub(current_count))
        .collect();

    for (slot, spawn_player) in (current_count..).zip(spawn_events.iter()) {
        let pos = Transform::from_xyz(
            spawn_player.position.x,
            spawn_player.position.y,
            spawn_player.position.z,
        );

        // The roster def for this slot drives ability, throw rate, model and weapon.
        let roster_def = roster.as_ref()
            .and_then(|r| r.def_paths.get(slot))
            .and_then(|def_path| AssetDefinition::load_from_def_path(def_path));
        let player_props = roster_def.as_ref().and_then(|def| match &def.model_type {
            ModelType::Player(props) => Some(props.clone()),
            _ => None,
        });

        let (roster_ability, roster_throw_rate) = player_props.as_ref()
            .map(|props| {
                use crate::assets::asset_definition::PlayerAbility::*;
                use crate::player::systems::abilities::SpecialAbility;
                let ability = match props.ability {
                    Bombardment => SpecialAbility::Bombardment,
                    Healing     => SpecialAbility::Healing,
                    Whirlwind   => SpecialAbility::Whirlwind,
                    GoldDigger  => SpecialAbility::GoldDigger,
                    Molotov     => SpecialAbility::Molotov,
                };
                (ability, props.throw_rate_per_minute)
            })
            .unwrap_or_else(|| (ability_for_slot(slot), 60.0));

        // Weapon to snap onto this character's `grip` hardpoint, if any.
        let pending_equip = player_props.as_ref()
            .and_then(|props| props.weapon.as_ref())
            .zip(roster_def.as_ref())
            .and_then(|(weapon_def_path, def)| PendingEquip::resolve(def, weapon_def_path));

        // Decide: use sprite billboard or 3D model?
        let use_billboard = config.as_ref()
            .map(|c| !c.body_type.is_empty())
            .unwrap_or(false);
        let billboard_sheet = use_billboard
            .then(|| sheet.as_ref().and_then(|s| s.billboard_handle.clone()))
            .flatten();

        let player = if use_billboard && let Some(billboard_sheet) = billboard_sheet && let Some(billboard_mesh) = billboard_mesh.as_ref() {
            let sheet_handle = billboard_sheet;
            let mesh_handle = billboard_mesh.0.clone();

            let mat = sprite_materials.add(SpriteBillboardMaterial {
                sprite_sheet: sheet_handle,
                uv_rect: bevy::prelude::Vec4::new(0.0, 0.5, 64.0 / 704.0, 64.0 / 256.0),
            });

            let entity_cmds = commands.spawn((
                pos,
                Visibility::default(),
                Collider::cuboid(0.5, 0.5, 0.45),
                PlayerBundle::with_throw_rate(
                    "player",
                    [CollisionLayer::Player],
                    [
                        CollisionLayer::Ball,
                        CollisionLayer::ImpassableAll,
                        CollisionLayer::ImpassablePlayer,
                        CollisionLayer::Floor,
                        CollisionLayer::Alien,
                        CollisionLayer::Player,
                        CollisionLayer::AlienSpawnPoint,
                        CollisionLayer::AlienGoal,
                    ],
                    roster_throw_rate,
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
            // 3D model path — use roster def if available for this slot, else default.
            let s = &*model_settings;
            // Load scene from roster def if available; also sync game_assets and
            // player_asset_def so build_player_anim_graph uses the right GLTF.
            let scene = roster_def.clone()
                .map(|def| {
                    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(def.model_path.clone()));
                    // Slot 0 drives the shared animation graph — keep game_assets in sync.
                    if slot == 0 {
                        game_assets.player_scene = scene.clone();
                        game_assets.player_gltf = asset_server.load(def.model_path.clone());
                        if matches!(def.model_type, ModelType::Player(_)) {
                            player_asset_def.0 = Some(def);
                        }
                    }
                    scene
                })
                .unwrap_or_else(|| game_assets.player_scene.clone());
            commands.spawn((
                FixSceneTransform::new(
                    Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                    Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                    Vec3::splat(s.scale),
                ),
                WorldAssetRoot(scene),
                pos,
                Collider::cuboid(0.5, 0.5, 0.45),
                PlayerBundle::with_throw_rate(
                    "player",
                    [CollisionLayer::Player],
                    [
                        CollisionLayer::Ball,
                        CollisionLayer::ImpassableAll,
                        CollisionLayer::ImpassablePlayer,
                        CollisionLayer::Floor,
                        CollisionLayer::Alien,
                        CollisionLayer::Player,
                        CollisionLayer::AlienSpawnPoint,
                        CollisionLayer::AlienGoal,
                    ],
                    roster_throw_rate,
                ),
                )).id()
        };

        // Override ability from def / slot default.
        commands.entity(player).insert(roster_ability);
        // Players who joined on a gamepad drop the keyboard component; `assign_gamepads`
        // resolves the pad index to the actual gamepad entity once it sees this.
        if let Some(InputDevice::Gamepad(pad_index)) =
            roster.as_ref().and_then(|r| r.devices.get(slot)).copied()
        {
            commands
                .entity(player)
                .remove::<crate::control::components::InputKeyboard>()
                .insert(WantsGamepad(pad_index));
        }
        // The weapon is spawned later, once the skeleton exists (see `equip`).
        if let Some(equip) = pending_equip {
            commands.entity(player).insert(equip);
        }
        add_health_bar_mw.write(AddHealthBar { entity: player, name: "PLAYER" });
        player_added_mw.write(GameTrackingEvent::PlayerAdded(player));
    }
}

/// Cycles through abilities by slot so each player starts with a different one.
fn ability_for_slot(slot: usize) -> crate::player::systems::abilities::SpecialAbility {
    use crate::player::systems::abilities::SpecialAbility::*;
    match slot % 4 {
        0 => Bombardment,
        1 => Healing,
        2 => Whirlwind,
        _ => GoldDigger,
    }
}

/// Marker placed on the direct scene-root child of the player so we can retarget it later.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct PlayerModelRoot;

pub fn fix_scene_transform(
    mut commands: Commands,
    mut scene_instance_query: Query<(Entity, &FixSceneTransform, &Children)>,
    mut child_query: Query<&mut Transform, With<Visibility>>,
) {
    for (parent, fix_scene_transform, children) in scene_instance_query.iter_mut() {
        for child in children.iter() {
            if let Ok(mut transform) = child_query.get_mut(child) {
                transform.translation = fix_scene_transform.translation;
                transform.rotation = fix_scene_transform.rotation;
                transform.scale = fix_scene_transform.scale;
                commands.entity(child).insert(PlayerModelRoot);
                commands.entity(parent).remove::<FixSceneTransform>();
            }
        }
    }
}

pub fn apply_model_settings_live(
    model_settings: Res<ModelSettings>,
    mut root_query: Query<&mut Transform, With<PlayerModelRoot>>,
    mut last: Local<(f32, f32, f32, f32, f32)>,
) {
    if !model_settings.is_changed() { return; }
    let s = &*model_settings;
    let sig = (s.scale, s.translation_x, s.translation_y, s.translation_z, s.rotation_y_degrees);
    if *last == sig { return; }
    *last = sig;
    for mut transform in root_query.iter_mut() {
        transform.translation = Vec3::new(s.translation_x, s.translation_y, s.translation_z);
        transform.rotation = Quat::from_rotation_y(s.rotation_y_degrees.to_radians());
        transform.scale = Vec3::splat(s.scale);
    }
}
