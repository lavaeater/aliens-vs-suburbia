use crate::control::components::CharacterState;
use crate::game_state::GameState;
use bevy::animation::AnimationClip;
use bevy::app::{App, Plugin, Update};
use bevy::asset::{AssetServer, Handle};
use bevy::ecs::component::Mutable;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum AnimationEventType {
    GotoAnimState,
    LeaveAnimState,
}

#[derive(Message, Clone)]
pub struct AnimationEvent(pub AnimationEventType, pub Entity, pub AnimationKey);

/// Play `key`'s clip on `character` outright, bypassing `CharacterState` — the character
/// editor's preview. `CharacterState` only lets a key play when it changes what the
/// intent stack resolves to, so most keys (Run, Wave, Jump...) could never be previewed
/// through it. `None` ends the preview: back to whatever the stack resolves to.
///
/// The previewed key becomes the `CurrentAnimationKey`, so a graph rebuild after a
/// mapping edit replays it, and the next gameplay transition stops it like any other.
#[derive(Message, Clone)]
pub struct PreviewAnimation {
    pub character: Entity,
    pub key: Option<AnimationKey>,
}

#[derive(Component, Debug, Reflect, Default)]
#[reflect(Component, Default)]
#[type_path = "avs"]
pub struct CurrentAnimationKey {
    pub group: String,
    pub key: AnimationKey,
}

impl CurrentAnimationKey {
    pub const fn new(group: String, key: AnimationKey) -> Self {
        Self { group, key }
    }
}

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AnimationEvent>()
            .add_message::<PreviewAnimation>()
            .add_systems(OnEnter(GameState::InGame), load_animations)
            .add_systems(
                Update,
                (
                    start_some_animations,
                    goto_animation_state_handler,
                    leave_animation_state_handler,
                    preview_animation_handler,
                    end_finished_throws,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

/// The graph nodes a key plays together: one clip, or a lower-body walk with an
/// upper-body aim (`model_settings::plugin::build_graph`). The first is the key's main
/// clip, the one that says when a one-shot is finished.
pub type KeyNodes = Vec<AnimationNodeIndex>;

#[derive(Resource)]
pub struct AnimationStore {
    pub anims: HashMap<String, HashMap<AnimationKey, KeyNodes>>,
    pub graphs: HashMap<String, Handle<AnimationGraph>>,
}

impl AnimationStore {
    pub fn nodes(&self, group: &str, key: AnimationKey) -> &[AnimationNodeIndex] {
        self.anims
            .get(group)
            .and_then(|anims| anims.get(&key))
            .map_or(&[], Vec::as_slice)
    }

    /// Start every node of `key`, from `seek` seconds in.
    pub fn play(&self, group: &str, key: AnimationKey, player: &mut AnimationPlayer, seek: f32) {
        for &idx in self.nodes(group, key) {
            let active = player.play(idx);
            active.set_seek_time(seek);
            if key.loops() {
                active.repeat();
            }
        }
    }

    pub fn stop(&self, group: &str, key: AnimationKey, player: &mut AnimationPlayer) {
        for &idx in self.nodes(group, key) {
            player.stop(idx);
        }
    }
}

/// Animation keys: the game's animation vocabulary. A def maps each key to one of its
/// model's clips (`AssetDefinition::animations`); a key it leaves out is resolved by
/// `animation::clips::resolve` (rig preset, then name search, then `fallback`).
///
/// They double as the intents pushed onto `CharacterState`'s stack, which resolves the
/// stack into the one key whose clip plays (Walk + IdleShoot -> WalkShoot, say).
#[derive(
    Eq,
    Hash,
    PartialEq,
    PartialOrd,
    Ord,
    Copy,
    Clone,
    Debug,
    Reflect,
    Default,
    Serialize,
    Deserialize,
)]
#[reflect(Default)]
#[type_path = "avs"]
pub enum AnimationKey {
    // ── Locomotion ─────────────────────────────────────────────────────────
    #[default]
    Idle,
    IdleShoot,
    Walk,
    WalkShoot,
    Run,
    RunShoot,
    RunGun,
    Duck,
    // ── Airborne ───────────────────────────────────────────────────────────
    Jump,
    JumpIdle,
    JumpLand,
    // ── Actions ────────────────────────────────────────────────────────────
    Punch,
    Wave,
    Yes,
    No,
    // ── Reactions ──────────────────────────────────────────────────────────
    Death,
    HitReact,
    // ── Game-intent keys (resolved to composite clips by CharacterState) ───
    Throwing, // one-shot throw (ball, grenade); left again when the clip ends
    Building, // direct clip ("interact" / "wave") or falls back to Idle
    Reload,   // one-shot while a magazine is swapped; freezes if the rig has no clip
}

impl AnimationKey {
    /// Whether this animation should loop.  One-shot animations (Death,
    /// reactions, landing, a throw) hold their last frame instead of restarting.
    pub const fn loops(self) -> bool {
        !matches!(
            self,
            Self::Death | Self::HitReact | Self::JumpLand | Self::Reload | Self::Throwing
        )
    }

    /// The key to borrow a clip from when the model has none for this one. Walking
    /// while shooting on a rig without a walk-and-aim clip walks (torso twist still
    /// aims), rather than freezing.
    ///
    /// A borrowed clip plays with the *borrowing* key's `loops`, so `Throwing` can take
    /// Punch's clip and still end (it is left when its clip finishes).
    pub const fn fallback(self) -> Option<Self> {
        match self {
            Self::IdleShoot => Some(Self::Idle),
            Self::WalkShoot | Self::Run => Some(Self::Walk),
            Self::RunShoot | Self::RunGun => Some(Self::Run),
            Self::JumpIdle => Some(Self::Jump),
            Self::Throwing => Some(Self::Punch),
            _ => None,
        }
    }

    /// Clip-name fragment searched for when neither the def nor a rig preset names a
    /// clip.  Matched against the last `|`-delimited segment of the GLTF
    /// clip name (case-insensitive exact), then as a full-name suffix, then
    /// as a substring.  See `clip_matches()`.
    pub const fn default_search(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::IdleShoot => "idle_shoot",
            Self::Walk => "walk",
            Self::WalkShoot => "walk_shoot",
            Self::Run => "run",
            Self::RunShoot => "run_shoot",
            Self::RunGun => "run_gun",
            Self::Duck => "duck",
            Self::Jump => "jump",
            Self::JumpIdle => "jump_idle",
            Self::JumpLand => "jump_land",
            Self::Punch => "punch",
            Self::Throwing => "throw",
            Self::Wave => "wave",
            Self::Yes => "yes",
            Self::No => "no",
            Self::Death => "death",
            Self::HitReact => "hitreact",
            Self::Building => "interact",
            Self::Reload => "reload",
        }
    }
}

/// Match a GLTF clip name against a search fragment.
///
/// Priority:
/// 1. Exact match on the segment after the last `|` (handles `"Armature|Walk"`
///    vs `"Armature|Walk_Shoot"` correctly).
/// 2. Full-name suffix match (handles models without an armature prefix).
/// 3. Substring fallback (for user-configured partial names).
pub fn clip_matches(clip_name: &str, search: &str) -> bool {
    if search.is_empty() {
        return false;
    }
    let lower = clip_name.to_lowercase();
    let s = search.to_lowercase();
    let anim_part = lower.rsplit('|').next().unwrap_or(&lower);
    anim_part == s || lower.ends_with(&s) || lower.contains(&s)
}

pub const ANIM_KEYS: &[AnimationKey] = &[
    AnimationKey::Idle,
    AnimationKey::IdleShoot,
    AnimationKey::Walk,
    AnimationKey::WalkShoot,
    AnimationKey::Run,
    AnimationKey::RunShoot,
    AnimationKey::RunGun,
    AnimationKey::Duck,
    AnimationKey::Jump,
    AnimationKey::JumpIdle,
    AnimationKey::JumpLand,
    AnimationKey::Punch,
    AnimationKey::Wave,
    AnimationKey::Yes,
    AnimationKey::No,
    AnimationKey::Death,
    AnimationKey::HitReact,
    AnimationKey::Throwing,
    AnimationKey::Building,
    AnimationKey::Reload,
];

/*
[0] AlienArmature|Alien_Clapping
  [1] AlienArmature|Alien_Death
  [2] AlienArmature|Alien_Idle
  [3] AlienArmature|Alien_IdleHold
  [4] AlienArmature|Alien_Jump
  [5] AlienArmature|Alien_Punch
  [6] AlienArmature|Alien_Run
  [7] AlienArmature|Alien_RunHold
  [8] AlienArmature|Alien_RunningJump
  [9] AlienArmature|Alien_Sitting
  [10] AlienArmature|Alien_Standing
  [11] AlienArmature|Alien_Swimming
  [12] AlienArmature|Alien_SwordSlash
  [13] AlienArmature|Alien_Walk
 */

/* Adventurer
[0] CharacterArmature|Death
 [1] CharacterArmature|Gun_Shoot
 [2] CharacterArmature|HitRecieve
 [3] CharacterArmature|HitRecieve_2
 [4] CharacterArmature|Idle
 [5] CharacterArmature|Idle_Gun
 [6] CharacterArmature|Idle_Gun_Pointing
 [7] CharacterArmature|Idle_Gun_Shoot
 [8] CharacterArmature|Idle_Neutral
 [9] CharacterArmature|Idle_Sword
 [10] CharacterArmature|Interact
 [11] CharacterArmature|Kick_Left
 [12] CharacterArmature|Kick_Right
 [13] CharacterArmature|Punch_Left
 [14] CharacterArmature|Punch_Right
 [15] CharacterArmature|Roll
 [16] CharacterArmature|Run
 [17] CharacterArmature|Run_Back
 [18] CharacterArmature|Run_Left
 [19] CharacterArmature|Run_Right
 [20] CharacterArmature|Run_Shoot
 [21] CharacterArmature|Sword_Slash
 [22] CharacterArmature|Walk
 [23] CharacterArmature|Wave
*/

pub fn leave_animation_state_handler(
    anim_store: Res<AnimationStore>,
    mut update_er: MessageReader<AnimationEvent>,
    mut anim_key_query: Query<(&mut CurrentAnimationKey, &mut CharacterState)>,
    mut player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    for AnimationEvent(event_type, entity, anim_key) in update_er.read() {
        if event_type == &AnimationEventType::LeaveAnimState
            && let Ok((mut current_key, mut character_state)) = anim_key_query.get_mut(*entity)
            && let Some(resolved) = character_state.leave_state(*anim_key)
            && let Some(player_entity) =
                get_child_with_component_recursive(*entity, &child_query, &player_query)
            && let Ok(mut player) = player_query.get_mut(player_entity)
        {
            let old = current_key.key;
            current_key.key = resolved;
            anim_thingie(
                &anim_store,
                &current_key.group,
                resolved,
                &mut player,
                Some(old),
            );
        }
    }
}

/// Play `key`'s clip, stopping `old_key`'s clip first to clear any held pose
/// (important after one-shot animations like Death).
///
/// Between a key and the one it borrows from (Walk and WalkShoot) the new clip picks up
/// where the old one was, so starting to aim mid-stride doesn't restart the legs.
fn anim_thingie(
    anim_store: &Res<AnimationStore>,
    group: &str,
    key: AnimationKey,
    player: &mut Mut<AnimationPlayer>,
    old_key: Option<AnimationKey>,
) {
    let mut seek = 0.0;
    if let Some(old) = old_key {
        if key.fallback() == Some(old) || old.fallback() == Some(key) {
            seek = anim_store
                .nodes(group, old)
                .first()
                .and_then(|&idx| player.animation(idx))
                .map_or(0.0, bevy::animation::ActiveAnimation::seek_time);
        }
        anim_store.stop(group, old, player);
    }
    anim_store.play(group, key, player, seek);
}

pub fn goto_animation_state_handler(
    anim_store: Res<AnimationStore>,
    mut update_er: MessageReader<AnimationEvent>,
    mut anim_key_query: Query<(&mut CurrentAnimationKey, &mut CharacterState)>,
    mut player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    for AnimationEvent(event_type, entity, anim_key) in update_er.read() {
        if event_type == &AnimationEventType::GotoAnimState
            && let Ok((mut current_key, mut character_state)) = anim_key_query.get_mut(*entity)
            && let Some(resolved) = character_state.enter_state(*anim_key)
            && let Some(player_entity) =
                get_child_with_component_recursive(*entity, &child_query, &player_query)
            && let Ok(mut player) = player_query.get_mut(player_entity)
        {
            let old = current_key.key;
            current_key.key = resolved;
            anim_thingie(
                &anim_store,
                &current_key.group,
                resolved,
                &mut player,
                Some(old),
            );
        }
    }
}

/// A throw is a one-shot: once its clip has played out (or the model has none, so
/// nothing would ever finish), leave `Throwing` so the stack falls back to whatever
/// is underneath. Nothing else leaves it after a grenade lob.
pub fn end_finished_throws(
    anim_store: Res<AnimationStore>,
    characters: Query<(Entity, &CurrentAnimationKey), With<CharacterState>>,
    player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
    mut anim_mw: MessageWriter<AnimationEvent>,
) {
    for (character, current_key) in &characters {
        if current_key.key != AnimationKey::Throwing {
            continue;
        }
        let Some(player_entity) =
            get_child_with_component_recursive(character, &child_query, &player_query)
        else {
            continue;
        };
        let Ok(player) = player_query.get(player_entity) else {
            continue;
        };
        let finished = anim_store
            .nodes(&current_key.group, AnimationKey::Throwing)
            .first()
            .and_then(|&idx| player.animation(idx))
            .is_none_or(bevy::animation::ActiveAnimation::is_finished);
        if finished {
            anim_mw.write(AnimationEvent(
                AnimationEventType::LeaveAnimState,
                character,
                AnimationKey::Throwing,
            ));
        }
    }
}

pub fn preview_animation_handler(
    anim_store: Res<AnimationStore>,
    mut previews: MessageReader<PreviewAnimation>,
    mut characters: Query<(&mut CurrentAnimationKey, &CharacterState)>,
    mut player_query: Query<&mut AnimationPlayer>,
    child_query: Query<&Children>,
) {
    for preview in previews.read() {
        let Ok((mut current_key, character_state)) = characters.get_mut(preview.character) else {
            continue;
        };
        let Some(player_entity) =
            get_child_with_component_recursive(preview.character, &child_query, &player_query)
        else {
            continue;
        };
        let Ok(mut player) = player_query.get_mut(player_entity) else {
            continue;
        };
        let key = preview.key.unwrap_or_else(|| character_state.resolve());
        current_key.key = key;
        // Everything, not just the current key's clip: a preview must not blend with
        // whatever was playing before it.
        player.stop_all();
        anim_thingie(&anim_store, &current_key.group, key, &mut player, None);
    }
}

pub fn load_animations(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
) {
    // --- Aliens ---
    let mut alien_graph = AnimationGraph::new();
    let mut alien_anims: HashMap<AnimationKey, KeyNodes> = HashMap::new();

    let alien_walking_clip: Handle<AnimationClip> =
        asset_server.load("quaternius/alien.glb#Animation13"); // AlienArmature|Alien_Walk
    let alien_idle_clip: Handle<AnimationClip> =
        asset_server.load("quaternius/alien.glb#Animation2"); // AlienArmature|Alien_Idle

    alien_anims.insert(
        AnimationKey::Walk,
        vec![alien_graph.add_clip(alien_walking_clip, 1.0, alien_graph.root)],
    );
    alien_anims.insert(
        AnimationKey::Idle,
        vec![alien_graph.add_clip(alien_idle_clip, 1.0, alien_graph.root)],
    );

    let alien_graph_handle = animation_graphs.add(alien_graph);

    // Player animations are built dynamically from the character GLTF by
    // build_player_anim_graph in model_settings/plugin.rs once the asset loads.
    let mut anims = HashMap::new();
    anims.insert("aliens".to_string(), alien_anims);
    anims.insert("players".to_string(), HashMap::new());

    let mut graphs = HashMap::new();
    graphs.insert("aliens".to_string(), alien_graph_handle);

    commands.insert_resource(AnimationStore { anims, graphs });
}

pub fn start_some_animations(
    anim_store: Res<AnimationStore>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    anim_key_query: Query<&CurrentAnimationKey>,
    parent_query: Query<&ChildOf>,
    mut commands: Commands,
) {
    for (entity, mut anim_player) in players.iter_mut() {
        if let Some(super_ent) = get_parent_recursive(entity, &parent_query)
            && let Ok(anim_key) = anim_key_query.get(super_ent)
            && let Some(graph_handle) = anim_store.graphs.get(&anim_key.group)
        {
            // `try_insert`: the character owning this animation player may be despawned
            // between here and the buffers being applied (a character editor model swap does
            // this), and a plain `insert` on a dead entity aborts the app.
            commands
                .entity(entity)
                .try_insert(AnimationGraphHandle(graph_handle.clone()));

            anim_thingie(
                &anim_store,
                &anim_key.group,
                anim_key.key,
                &mut anim_player,
                None,
            );
        }
    }
}

pub fn get_parent_recursive(entity: Entity, parent_query: &Query<&ChildOf>) -> Option<Entity> {
    match parent_query.get(entity) {
        Ok(child_of) => get_parent_recursive(child_of.parent(), parent_query),
        Err(_) => Some(entity),
    }
}

pub fn get_child_with_component_recursive<T: Component<Mutability = Mutable>>(
    entity: Entity,
    child_query: &Query<&Children>,
    component_query: &Query<&mut T>,
) -> Option<Entity> {
    if component_query.contains(entity) {
        Some(entity)
    } else {
        match child_query.get(entity) {
            Ok(children) => {
                for child in children {
                    if let Some(ent) =
                        get_child_with_component_recursive(*child, child_query, component_query)
                    {
                        return Some(ent);
                    }
                }
                None
            }
            Err(_) => None,
        }
    }
}
