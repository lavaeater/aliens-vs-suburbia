use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Added, Children, Commands, Component, Entity, Event, EventReader, in_state, IntoSystemConfigs, OnEnter, Query, Reflect, Res, Resource};
use bevy::animation::{AnimationClip, AnimationPlayer};
use bevy::hierarchy::Parent;
use bevy::utils::HashMap;
use bevy::asset::{AssetServer, Handle};
use crate::control::systems::CharacterState;
use crate::game_state::GameState;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AnimationEventType {
    GotoAnimState,
    LeaveAnimState,
}

#[derive(Event)]
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
            .add_event::<AnimationEvent>()
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
pub struct AnimationStore<S: Into<String>> {
    pub anims: HashMap<S, HashMap<AnimationKey, Handle<AnimationClip>>>,
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
    anim_store: Res<AnimationStore<String>>,
    mut update_er: EventReader<AnimationEvent>,
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
                            if let Some(anim) = anim_store.anims.get(&current_key.group).unwrap().get(&current_key.key) {
                                player.play(anim.clone_weak()).repeat();
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn goto_animation_state_handler(
    anim_store: Res<AnimationStore<String>>,
    mut update_er: EventReader<AnimationEvent>,
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
                            if let Some(anim) = anim_store.anims.get(&current_key.group).unwrap().get(&current_key.key) {
                                player.play(anim.clone_weak()).repeat();
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
) {
    let mut store = AnimationStore::<String> {
        anims: HashMap::new()
    };
    store.anims.insert("aliens".into(),
                       HashMap::new());
    let alien_anims = store
        .anims
        .get_mut("aliens")
        .unwrap();
    alien_anims.insert(AnimationKey::Walking, asset_server.load("quaternius/alien.glb#Animation13"));
    alien_anims.insert(AnimationKey::Idle, asset_server.load("quaternius/alien.glb#Animation2"));


    store
        .anims
        .insert("players".into(),
                HashMap::new());
    let player_anims = store
        .anims
        .get_mut("players")
        .unwrap();
    player_anims.insert(AnimationKey::Idle, asset_server.load("girl/girl.glb#Animation0"));
    player_anims.insert(AnimationKey::Walking, asset_server.load("girl/girl.glb#Animation1"));
    player_anims.insert(AnimationKey::Throwing, asset_server.load("girl/girl.glb#Animation2"));
    player_anims.insert(AnimationKey::Crawling, asset_server.load("girl/girl.glb#Animation3"));
    player_anims.insert(AnimationKey::Building, asset_server.load("girl/girl.glb#Animation4"));

    commands.insert_resource(store);
}

pub fn start_some_animations(
    anim_store: Res<AnimationStore<String>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    anim_key_query: Query<&CurrentAnimationKey>,
    parent_query: Query<&Parent>,
) {
    for (entity, mut anim_player) in players.iter_mut() {
        if let Some(super_ent) = get_parent_recursive(entity, &parent_query) {
            if let Ok(anim_key) = anim_key_query.get(super_ent) {
                if let Some(anim) = anim_store.anims.get(&anim_key.group).unwrap().get(&anim_key.key) {
                    anim_player.play(anim.clone_weak()).repeat();
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

pub fn get_child_with_component_recursive<T: Component>(entity: Entity, child_query: &Query<&Children>, component_query: &Query<&mut T>) -> Option<Entity> {
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


