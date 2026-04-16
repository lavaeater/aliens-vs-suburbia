use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Mutable;
use bevy::prelude::{Added, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer,
                    Assets, Children, Commands, Component, Entity, Message, MessageReader,
                    in_state, IntoScheduleConfigs, OnEnter, Parent, Query, Reflect, Res, ResMut, Resource};
use bevy::animation::AnimationClip;
use bevy::asset::{AssetServer, Handle};
use std::collections::HashMap;
use crate::control::components::CharacterState;
use crate::game_state::GameState;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AnimationEventType {
    GotoAnimState,
    LeaveAnimState,
}

#[derive(Message, Clone)]
pub struct AnimationEvent(pub AnimationEventType, pub Entity, pub AnimationKey);

#[derive(Component, Debug, Reflect)]
pub struct CurrentAnimationKey {
    pub group: String,
    pub key: AnimationKey,
}

impl CurrentAnimationKey {
    pub fn new(group: String, key: AnimationKey) -> Self {
        CurrentAnimationKey { group, key }
    }
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_message::<AnimationEvent>()
            .add_systems(
                OnEnter(GameState::InGame),
                load_animations,
            )
            .add_systems(
                Update,
                (
                    start_some_animations,
                    goto_animation_state_handler,
                    leave_animation_state_handler,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

#[derive(Resource)]
pub struct AnimationStore {
    pub anims: HashMap<String, HashMap<AnimationKey, AnimationNodeIndex>>,
    pub graphs: HashMap<String, Handle<AnimationGraph>>,
}

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug, Reflect)]
pub enum AnimationKey {
    Building,
    Idle,
    Walking,
    Throwing,
    Crawling,
}

pub fn leave_animation_state_handler(
    anim_store: Res<AnimationStore>,
    mut update_er: MessageReader<AnimationEvent>,
    mut anim_key_query: Query<(&mut CurrentAnimationKey, &mut CharacterState)>,
    mut player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    for AnimationEvent(event_type, entity, anim_key) in update_er.read() {
        if event_type == &AnimationEventType::LeaveAnimState {
            if let Ok((mut current_key, mut character_state)) = anim_key_query.get_mut(*entity) {
                let (changed, new_key) = character_state.leave_state(*anim_key);
                if changed {
                    if let Some(player_entity) = get_child_with_component_recursive(*entity, &child_query, &player_query) {
                        if let Ok(mut player) = player_query.get_mut(player_entity) {
                            current_key.key = new_key;
                            if let Some(idx) = anim_store.anims.get(&current_key.group).and_then(|m| m.get(&current_key.key)) {
                                player.play(*idx).repeat();
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn goto_animation_state_handler(
    anim_store: Res<AnimationStore>,
    mut update_er: MessageReader<AnimationEvent>,
    mut anim_key_query: Query<(&mut CurrentAnimationKey, &mut CharacterState)>,
    mut player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    for AnimationEvent(event_type, entity, anim_key) in update_er.read() {
        if event_type == &AnimationEventType::GotoAnimState {
            if let Ok((mut current_key, mut character_state)) = anim_key_query.get_mut(*entity) {
                if character_state.enter_state(*anim_key) {
                    if let Some(player_entity) = get_child_with_component_recursive(*entity, &child_query, &player_query) {
                        if let Ok(mut player) = player_query.get_mut(player_entity) {
                            current_key.key = *anim_key;
                            if let Some(idx) = anim_store.anims.get(&current_key.group).and_then(|m| m.get(&current_key.key)) {
                                player.play(*idx).repeat();
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn load_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    // --- Aliens ---
    let mut alien_graph = AnimationGraph::new();
    let mut alien_anims: HashMap<AnimationKey, AnimationNodeIndex> = HashMap::new();

    let alien_walking_clip: Handle<AnimationClip> = asset_server.load("quaternius/alien.glb#Animation13");
    let alien_idle_clip: Handle<AnimationClip> = asset_server.load("quaternius/alien.glb#Animation2");

    alien_anims.insert(AnimationKey::Walking, alien_graph.add_clip(alien_walking_clip, 1.0, alien_graph.root));
    alien_anims.insert(AnimationKey::Idle, alien_graph.add_clip(alien_idle_clip, 1.0, alien_graph.root));

    let alien_graph_handle = animation_graphs.add(alien_graph);

    // --- Players ---
    let mut player_graph = AnimationGraph::new();
    let mut player_anims: HashMap<AnimationKey, AnimationNodeIndex> = HashMap::new();

    let player_idle_clip: Handle<AnimationClip> = asset_server.load("girl/girl.glb#Animation0");
    let player_walking_clip: Handle<AnimationClip> = asset_server.load("girl/girl.glb#Animation1");
    let player_throwing_clip: Handle<AnimationClip> = asset_server.load("girl/girl.glb#Animation2");
    let player_crawling_clip: Handle<AnimationClip> = asset_server.load("girl/girl.glb#Animation3");
    let player_building_clip: Handle<AnimationClip> = asset_server.load("girl/girl.glb#Animation4");

    player_anims.insert(AnimationKey::Idle, player_graph.add_clip(player_idle_clip, 1.0, player_graph.root));
    player_anims.insert(AnimationKey::Walking, player_graph.add_clip(player_walking_clip, 1.0, player_graph.root));
    player_anims.insert(AnimationKey::Throwing, player_graph.add_clip(player_throwing_clip, 1.0, player_graph.root));
    player_anims.insert(AnimationKey::Crawling, player_graph.add_clip(player_crawling_clip, 1.0, player_graph.root));
    player_anims.insert(AnimationKey::Building, player_graph.add_clip(player_building_clip, 1.0, player_graph.root));

    let player_graph_handle = animation_graphs.add(player_graph);

    let mut anims = HashMap::new();
    anims.insert("aliens".to_string(), alien_anims);
    anims.insert("players".to_string(), player_anims);

    let mut graphs = HashMap::new();
    graphs.insert("aliens".to_string(), alien_graph_handle);
    graphs.insert("players".to_string(), player_graph_handle);

    commands.insert_resource(AnimationStore { anims, graphs });
}

pub fn start_some_animations(
    anim_store: Res<AnimationStore>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    anim_key_query: Query<&CurrentAnimationKey>,
    parent_query: Query<&Parent>,
    mut commands: Commands,
) {
    for (entity, mut anim_player) in players.iter_mut() {
        if let Some(super_ent) = get_parent_recursive(entity, &parent_query) {
            if let Ok(anim_key) = anim_key_query.get(super_ent) {
                if let Some(graph_handle) = anim_store.graphs.get(&anim_key.group) {
                    commands.entity(entity).insert(AnimationGraphHandle(graph_handle.clone()));
                    if let Some(idx) = anim_store.anims.get(&anim_key.group).and_then(|m| m.get(&anim_key.key)) {
                        anim_player.play(*idx).repeat();
                    }
                }
            }
        }
    }
}

pub fn get_parent_recursive(entity: Entity, parent_query: &Query<&Parent>) -> Option<Entity> {
    match parent_query.get(entity) {
        Ok(parent) => {
            get_parent_recursive(parent.get(), parent_query)
        }
        Err(_) => {
            Some(entity)
        }
    }
}

pub fn get_child_with_component_recursive<T: Component<Mutability = Mutable>>(entity: Entity, child_query: &Query<&Children>, component_query: &Query<&mut T>) -> Option<Entity> {
    if component_query.contains(entity) {
        Some(entity)
    } else {
        match child_query.get(entity) {
            Ok(children) => {
                for child in children.into_iter() {
                    if let Some(ent) = get_child_with_component_recursive(*child, child_query, component_query) {
                        return Some(ent);
                    }
                }
                None
            }
            Err(_) => {
                None
            }
        }
    }
}
