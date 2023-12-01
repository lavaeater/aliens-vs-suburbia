use bevy::app::{App, Plugin, Update};
use bevy::prelude::{Added, Commands, Component, Entity, Event, EventReader, in_state, IntoSystemConfigs, OnEnter, Query, Reflect, Res, Resource};
use bevy::animation::{AnimationClip, AnimationPlayer};
use bevy::hierarchy::Parent;
use bevy::utils::HashMap;
use bevy::asset::{AssetServer, Handle};
use crate::game_state::GameState;

#[derive(Event)]
pub struct AnimationKeyUpdated(pub Entity, pub AnimationKey);

impl AnimationKeyUpdated {
    pub fn new(entity: Entity, anim_key: AnimationKey) -> Self {
        AnimationKeyUpdated(entity, anim_key)
    }
}

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
            .add_event::<AnimationKeyUpdated>()
            .add_systems(
                OnEnter(GameState::InGame),
                load_animations,
            )
            .add_systems(
                Update,
                (
                    start_some_animations,
                    update_animation_key_handler,
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
    Throwing
}

pub fn update_animation_key_handler(
    anim_store: Res<AnimationStore<String>>,
    mut update_er: EventReader<AnimationKeyUpdated>,
    mut anim_key_query: Query<(&mut CurrentAnimationKey, &mut AnimationPlayer)>,
) {
    for AnimationKeyUpdated(entity, anim_key) in update_er.read() {
        if let Ok((mut current_key, mut animation_player)) = anim_key_query.get_mut(*entity) {
            if current_key.key == *anim_key {
                continue;
            }
            current_key.key = *anim_key;
            if let Some(anim) = anim_store.anims.get(&current_key.group).unwrap().get(&current_key.key) {
                animation_player.play(anim.clone_weak()).repeat();
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
    player_anims.insert(AnimationKey::Walking, asset_server.load("girl/walking.glb#Animation0"));
    player_anims.insert(AnimationKey::Idle, asset_server.load("girl/ninja-idle.glb#Animation0"));
    player_anims.insert(AnimationKey::Building, asset_server.load("girl/victory-idle.glb#Animation0"));
    player_anims.insert(AnimationKey::Throwing, asset_server.load("girl/throwing.glb#Animation0"));

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
