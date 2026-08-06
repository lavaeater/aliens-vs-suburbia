use crate::game_state::GameState;
use crate::general::components::map_components::Floor;
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::{WeaponsHidden, WEAPON_NODES};
use crate::player::systems::auto_aim::{auto_aim, debug_gizmos};
use crate::player::systems::death_revive::{detect_player_death, player_revive_system};
use crate::player::systems::spawn_players::{fix_scene_transform, spawn_players};
use crate::player::systems::abilities::{AbilityInput, activate_ability, tick_ability_flash, tick_cooldowns, tick_whirlwind};
use crate::player::systems::equip::{equip_pending_weapons, keep_weapons_snapped};
use crate::player::systems::shoot::shoot_weapons;
use crate::player::systems::torso_twist::{
    apply_torso_twist, resolve_twist_bones, toggle_torso_twist, TorsoTwistEnabled,
};
use bevy::transform::TransformSystems;
use bevy::prelude::*;
use bevy::world_serialization::{WorldInstance, WorldAssetRoot};
use bevy_mod_outline::{AsyncWorldInheritOutline, AutoGenerateOutlineNormalsPlugin, InheritOutline, OutlinePlugin, OutlineVolume};

#[derive(Default)]
pub struct PlayerPlugin {
    pub with_debug: bool,
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        if self.with_debug {
            app.add_systems(Update, debug_gizmos.run_if(in_state(GameState::InGame)));
        }
        app.init_resource::<AbilityInput>()
            .init_resource::<TorsoTwistEnabled>()
            // The twist must land after the animation has posed the skeleton and before
            // the pose is propagated -- see torso_twist.rs.
            .add_systems(
                PostUpdate,
                apply_torso_twist
                    .after(bevy::app::AnimationSystems)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_plugins((OutlinePlugin::EXTRUDE_VERTEX, AutoGenerateOutlineNormalsPlugin::default()))
            .add_systems(Update, (auto_outline_scenes, sync_outline_with_visibility))
            .add_systems(
                Update,
                (
                    spawn_players,
                    equip_pending_weapons,
                    keep_weapons_snapped,
                    shoot_weapons,
                    fix_scene_transform,
                    auto_aim,
                    hide_player_weapon_nodes,
                    detect_player_death,
                    player_revive_system,
                    tick_cooldowns,
                    activate_ability,
                    tick_whirlwind,
                    tick_ability_flash,
                    reset_ability_input,
                    resolve_twist_bones,
                    toggle_torso_twist,
                )
                .run_if(in_state(GameState::InGame)),
            );
    }
}

fn reset_ability_input(mut input: ResMut<AbilityInput>) {
    input.pressed = false;
}

#[allow(clippy::type_complexity)]
fn auto_outline_scenes(
    mut commands: Commands,
    query: Query<Entity, (With<WorldAssetRoot>, Without<AsyncWorldInheritOutline>, Without<Floor>)>,
) {
    for entity in query.iter() {
        // `try_insert`, not `insert`: a scene root can be despawned between this system
        // queueing the command and the buffers being applied — the playground's model swap
        // does exactly that on the frame it changes character — and a plain `insert` on a
        // despawned entity is a hard error that takes the app down.
        commands.entity(entity).try_insert((
            OutlineVolume {
                visible: true,
                width: 2.0,
                colour: Color::BLACK,
            },
            AsyncWorldInheritOutline::default(),
        ));
    }
}

/// Keeps outline rendering in sync with Visibility.
///
/// Two races to handle:
/// 1. Visibility::Hidden set first, InheritOutline added later by AsyncWorldInheritOutline.
/// 2. InheritOutline already present, Visibility::Hidden set later.
///
/// When a weapon is later made visible again, re-insert InheritOutline alongside Visibility::Visible.
#[allow(clippy::type_complexity)]
fn sync_outline_with_visibility(
    mut commands: Commands,
    mut volume_query: Query<
        (&Visibility, &mut OutlineVolume),
        Or<(Changed<Visibility>, Added<OutlineVolume>)>,
    >,
    // Race 1: InheritOutline just added to an already-hidden entity.
    added_query: Query<(Entity, &Visibility), Added<InheritOutline>>,
    // Race 2: Visibility changed on an entity that already has InheritOutline.
    changed_query: Query<(Entity, &Visibility), (With<InheritOutline>, Changed<Visibility>)>,
) {
    for (vis, mut outline) in volume_query.iter_mut() {
        let should_be_visible = !matches!(vis, Visibility::Hidden);
        if outline.visible != should_be_visible {
            outline.visible = should_be_visible;
        }
    }
    for (entity, vis) in added_query.iter().chain(changed_query.iter()) {
        if matches!(vis, Visibility::Hidden) {
            commands.entity(entity).remove::<InheritOutline>();
        }
    }
}

/// After the player scene loads, hide all nodes listed in the AssetDefinition
/// for this model (falling back to the hardcoded WEAPON_NODES if no def exists).
#[allow(clippy::type_complexity)]
fn hide_player_weapon_nodes(
    mut commands: Commands,
    player_query: Query<(Entity, &WorldInstance), (With<crate::player::components::Player>, Without<WeaponsHidden>)>,
    scene_spawner: Res<WorldInstanceSpawner>,
    named_query: Query<(Entity, &Name)>,
    player_asset_def: Option<Res<PlayerAssetDef>>,
) {
    // Build the effective hide list: prefer AssetDefinition, fall back to WEAPON_NODES.
    let def_nodes: Vec<&str>;
    let hidden: &[&str] = if let Some(def_res) = &player_asset_def
        && let Some(def) = &def_res.0
        && !def.hidden_nodes.is_empty()
    {
        def_nodes = def.hidden_nodes.iter().map(|s| s.as_str()).collect();
        &def_nodes
    } else {
        WEAPON_NODES
    };

    for (player_entity, scene_instance) in player_query.iter() {
        if !scene_spawner.instance_is_ready(**scene_instance) { continue; }
        commands.entity(player_entity).insert(WeaponsHidden);
        for entity in scene_spawner.iter_instance_entities(**scene_instance) {
            if let Ok((_, name)) = named_query.get(entity)
                && hidden.contains(&name.as_str())
            {
                commands.entity(entity).insert(Visibility::Hidden);
            }
        }
    }
}
