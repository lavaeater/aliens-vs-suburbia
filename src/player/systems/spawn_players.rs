use bevy::math::{Quat, Vec3};
use bevy::prelude::*;
use bevy::asset::AssetServer;
use bevy::gltf::GltfAssetLabel;
use bevy::world_serialization::WorldAssetRoot;
use avian3d::prelude::Collider;
use crate::assets::asset_definition::{AssetDefinition, ModelType};
use crate::assets::assets_plugin::GameAssets;
use crate::camera::components::CameraTarget;
use crate::general::damage::Faction;
use crate::player::components::{Lives, PlayerSlot};
use crate::player::systems::death_revive::RespawnQueue;
use crate::settings::resources::GameSettings;
use crate::control::gamepad_input::WantsGamepad;
use crate::player::ammo::AmmoPouch;
use crate::player::systems::equip::PendingEquip;
use crate::player::systems::loadout::Weapons;
use crate::player::systems::leg_ik::PendingLegs;
use crate::player::systems::torso_twist::PendingTorsoTwist;
use crate::player_setup::state::InputDevice;
pub use crate::player::components::WeaponsHidden;
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::CollisionLayer;
use crate::general::events::map_events::SpawnPlayer;
use crate::model_settings::plugin::PlayerAssetDef;
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

#[allow(clippy::too_many_arguments)]
pub fn spawn_players(
    mut spawn_player_event_reader: MessageReader<SpawnPlayer>,
    mut commands: Commands,
    mut game_assets: ResMut<GameAssets>,
    mut player_asset_def: ResMut<PlayerAssetDef>,
    model_settings: Res<ModelSettings>,
    asset_server: Res<AssetServer>,
    roster: Option<Res<crate::player_setup::state::PlayerRoster>>,
    existing_players: Query<&PlayerSlot, With<crate::player::components::Player>>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
    mut player_added_mw: MessageWriter<GameTrackingEvent>,
    settings: Res<GameSettings>,
    mut respawn_queue: ResMut<RespawnQueue>,
) {
    let max_players = roster.as_ref()
        .map(|r| r.def_paths.len().max(1))
        .unwrap_or(1);
    let occupied: Vec<usize> = existing_players.iter().map(|s| s.0).collect();
    let requests: Vec<SpawnPlayer> = spawn_player_event_reader.read().cloned().collect();
    let assignments = assign_spawns(&requests, &occupied, max_players);

    for (slot, spawn_player) in assignments {
        respawn_queue.started = true;
        let pos = Transform::from_xyz(
            spawn_player.position.x,
            spawn_player.position.y,
            spawn_player.position.z,
        );
        let lives = Lives(spawn_player.lives.unwrap_or(settings.lives_per_player));

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

        // Everything this character carries; the first is snapped onto the `grip`
        // hardpoint now, the rest wait in the loadout for a switch.
        let roster_def_path = roster.as_ref().and_then(|r| r.def_paths.get(slot)).cloned();
        let loadout = match (player_props.as_ref(), roster_def_path.as_ref()) {
            (Some(props), Some(def_path)) => Weapons::new(
                def_path.clone(),
                props.weapon.iter().chain(props.extra_weapons.iter()).cloned(),
            ),
            _ => Weapons::default(),
        };
        let pending_equip = loadout.active_slot()
            .zip(roster_def.as_ref())
            .and_then(|(weapon_slot, def)| PendingEquip::resolve(def, &weapon_slot.def_path));
        let pouch = AmmoPouch::from_loadout(
            player_props.as_ref().map(|p| p.starting_ammo.as_slice()).unwrap_or(&[]),
        );

        let player = {
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
                // TransformInterpolation,
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
        commands.entity(player).insert((roster_ability, PlayerSlot(slot), CameraTarget::default(), Faction::Player, loadout, pouch, lives));
        // Torso twist: resolved to bone entities once the skeleton spawns. Defs that
        // don't list `aim_bones` fall back to the default mixamo spine chain.
        commands.entity(player).insert(PendingTorsoTwist::new(
            roster_def.as_ref().map(|def| def.aim_bones.clone()).unwrap_or_default(),
        ));
        // Procedural legs: the chains are found from the skeleton's own bone names once it
        // spawns, so there is nothing per-def to carry here.
        commands.entity(player).insert(PendingLegs::default());
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

/// Pair spawn requests with roster slots. Explicit slots (respawns) are honoured; the
/// rest fill free slots in order. When the map has fewer spawn points than players, the
/// points are reused so everyone still gets on the field.
pub fn assign_spawns(requests: &[SpawnPlayer], occupied: &[usize], max_players: usize) -> Vec<(usize, SpawnPlayer)> {
    let mut taken: Vec<usize> = occupied.to_vec();
    let mut out = Vec::new();

    for req in requests.iter().filter(|r| r.slot.is_some()) {
        let slot = req.slot.unwrap();
        if slot < max_players && !taken.contains(&slot) {
            taken.push(slot);
            out.push((slot, req.clone()));
        }
    }

    let anonymous: Vec<&SpawnPlayer> = requests.iter().filter(|r| r.slot.is_none()).collect();
    if anonymous.is_empty() {
        return out;
    }
    let mut i = 0;
    for slot in 0..max_players {
        if taken.contains(&slot) {
            continue;
        }
        let req = anonymous[i % anonymous.len()];
        out.push((slot, req.clone()));
        taken.push(slot);
        i += 1;
    }
    out
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
                // `try_insert`/`try_remove`: the player and its scene can be despawned
                // between this queueing and the buffers applying -- the playground's model
                // swap does exactly that -- and a plain `insert` on a despawned entity is
                // a hard error that takes the app down.
                commands.entity(child).try_insert(PlayerModelRoot);
                commands.entity(parent).try_remove::<FixSceneTransform>();
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

#[cfg(test)]
mod tests {
    use super::assign_spawns;
    use crate::general::events::map_events::SpawnPlayer;
    use bevy::math::Vec3;

    fn at(x: f32) -> SpawnPlayer {
        SpawnPlayer { position: Vec3::new(x, 0.0, 0.0), slot: None, lives: None }
    }

    #[test]
    fn one_spawn_point_still_seats_every_player() {
        let out = assign_spawns(&[at(1.0)], &[], 3);
        assert_eq!(out.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn free_slots_are_filled_around_the_living() {
        let out = assign_spawns(&[at(1.0), at(2.0)], &[1], 3);
        assert_eq!(out.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!(out[1].1.position.x, 2.0, "second point goes to the second free slot");
    }

    #[test]
    fn explicit_slots_win_and_never_double_seat() {
        let respawn = SpawnPlayer { position: Vec3::ZERO, slot: Some(2), lives: Some(1) };
        let out = assign_spawns(&[respawn.clone(), respawn], &[0], 4);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 2);
        assert_eq!(out[0].1.lives, Some(1));
    }

    #[test]
    fn nothing_spawns_past_the_roster() {
        let out = assign_spawns(&[at(1.0)], &[0], 1);
        assert!(out.is_empty());
    }
}
