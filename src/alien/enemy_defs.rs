//! Def-driven enemies: the model, stats and animation graph of an alien come from an
//! `Enemy`-typed `assets/defs/*.ron` named by the wave.
//!
//! `spawn_aliens` asks [`EnemyDefCache`] for the def and its asset handles, applies
//! `EnemyProps` to the components the AI already uses, and tags the entity with
//! [`EnemyDef`]. Animation graphs are per def: [`build_enemy_anim_graphs`] builds one into
//! `AnimationStore` under the def path as the group name once the GLTF is loaded, the same
//! way `build_player_anim_graph` does for the player. [`RangedAttack`] is the one new
//! behaviour: shoot the nearest player in range and line of sight while walking.

use std::collections::HashMap;

use avian3d::prelude::{Position, SpatialQuery, SpatialQueryFilter};
use bevy::gltf::{Gltf, GltfAssetLabel, GltfNode};
use bevy::prelude::*;
use bevy::world_serialization::WorldAsset;

use crate::alien::components::general::Alien;
use crate::animation::animation_plugin::{
    AnimationStore, CurrentAnimationKey, get_child_with_component_recursive,
};
use crate::assets::asset_definition::{AssetDefinition, EnemyAttack, EnemyProps, ModelType};
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::ApplyDamage;
use crate::gore::components::{DamageKind, Ephemeral};
use crate::model_settings::plugin::build_graph;
use crate::player::components::{Player, PlayerDead};

/// A loaded enemy def with the handles spawning needs.
pub struct LoadedEnemyDef {
    pub def: AssetDefinition,
    /// The `ModelType::Enemy` payload of `def`, extracted at load time so that holding a
    /// `LoadedEnemyDef` is itself the proof that the def is an enemy def.
    pub props: EnemyProps,
    pub scene: Handle<WorldAsset>,
    pub gltf: Handle<Gltf>,
}

/// A loaded def of any kind that is spawned as an animated creature (the infestation's
/// neighbours, from character defs).
pub struct LoadedCreatureDef {
    pub def: AssetDefinition,
    pub scene: Handle<WorldAsset>,
    pub gltf: Handle<Gltf>,
}

/// Enemy defs by path, loaded on first use; `creatures` holds other defs spawned the
/// same way (animation graph under the def path).
#[derive(Resource, Default)]
pub struct EnemyDefCache(
    pub HashMap<String, LoadedEnemyDef>,
    pub HashMap<String, LoadedCreatureDef>,
);

impl EnemyDefCache {
    pub fn get_or_load(
        &mut self,
        path: &str,
        asset_server: &AssetServer,
    ) -> Option<&LoadedEnemyDef> {
        if !self.0.contains_key(path) {
            let def = AssetDefinition::load_from_def_path(path)?;
            let ModelType::Enemy(props) = &def.model_type else {
                warn!("{path} is not an Enemy def");
                return None;
            };
            let props = props.clone();
            let scene =
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(def.model_path.clone()));
            let gltf = asset_server.load(def.model_path.clone());
            self.0.insert(
                path.to_string(),
                LoadedEnemyDef {
                    def,
                    props,
                    scene,
                    gltf,
                },
            );
        }
        self.0.get(path)
    }

    /// Any def, for a creature that is not an enemy. `None` if it cannot be read.
    pub fn get_or_load_creature(
        &mut self,
        path: &str,
        asset_server: &AssetServer,
    ) -> Option<&LoadedCreatureDef> {
        if !self.1.contains_key(path) {
            let def = AssetDefinition::load_from_def_path(path)?;
            let scene =
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(def.model_path.clone()));
            let gltf = asset_server.load(def.model_path.clone());
            self.1
                .insert(path.to_string(), LoadedCreatureDef { def, scene, gltf });
        }
        self.1.get(path)
    }

    /// Every loaded def with its model: path, def, GLTF.
    fn models(&self) -> impl Iterator<Item = (&String, &AssetDefinition, &Handle<Gltf>)> {
        self.0
            .iter()
            .map(|(path, l)| (path, &l.def, &l.gltf))
            .chain(self.1.iter().map(|(path, l)| (path, &l.def, &l.gltf)))
    }

    fn contains(&self, path: &str) -> bool {
        self.0.contains_key(path) || self.1.contains_key(path)
    }
}

/// An alien spawned from a def; its animation group (`CurrentAnimationKey::group`) is the
/// def path.
#[derive(Component, Debug, Clone)]
pub struct EnemyDef;

/// Fires at the nearest player in range while walking.
#[derive(Component, Debug, Clone)]
pub struct RangedAttack {
    pub damage: i32,
    pub range: f32,
    pub shot_interval: f32,
    pub cooldown: f32,
}

impl RangedAttack {
    pub fn from_attack(attack: &EnemyAttack) -> Option<Self> {
        match attack {
            EnemyAttack::Melee => None,
            EnemyAttack::Ranged {
                damage,
                range,
                fire_rate_per_minute,
            } => Some(Self {
                damage: *damage,
                range: *range,
                shot_interval: if *fire_rate_per_minute > 0.0 {
                    60.0 / fire_rate_per_minute
                } else {
                    2.0
                },
                cooldown: 1.0,
            }),
        }
    }
}

/// Build (once) the animation graph for every enemy (and creature) def in play and hand
/// it to whatever is animated from it (its `CurrentAnimationKey` group is the def path).
/// Waits for the GLTF; catches models whose `AnimationPlayer` appeared before the graph
/// existed.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn build_enemy_anim_graphs(
    cache: Res<EnemyDefCache>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    asset_server: Res<AssetServer>,
    anim_store: Option<ResMut<AnimationStore>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    animated: Query<(Entity, &CurrentAnimationKey)>,
    child_query: Query<&Children>,
    mut anim_player_query: Query<&mut AnimationPlayer>,
    graph_handles: Query<(), With<AnimationGraphHandle>>,
    mut commands: Commands,
) {
    let Some(mut store) = anim_store else { return };

    for (path, def, gltf) in cache.models() {
        if store.graphs.contains_key(path) {
            continue;
        }
        let Some(gltf) = gltf_assets.get(gltf) else {
            continue;
        };
        let mut extra = Vec::new();
        let mut waiting = false;
        for source in &def.animation_sources {
            let handle: Handle<Gltf> = asset_server.load(source.clone());
            if let Some(g) = gltf_assets.get(&handle) {
                extra.push((source, g));
            } else {
                waiting = true;
                break;
            }
        }
        if waiting {
            continue;
        }
        let (graph, anims) = build_graph(Some(def), None, gltf, &gltf_nodes, &extra);
        let handle = animation_graphs.add(graph);
        store.anims.insert(path.clone(), anims);
        store.graphs.insert(path.clone(), handle);
        info!("animation graph built for {path}");
    }

    // Late binding: models already animated but without a graph (spawned before it existed).
    for (entity, key) in animated.iter() {
        if !cache.contains(&key.group) {
            continue;
        }
        let Some(graph_handle) = store.graphs.get(&key.group) else {
            continue;
        };
        let Some(anim_entity) =
            get_child_with_component_recursive(entity, &child_query, &anim_player_query)
        else {
            continue;
        };
        if graph_handles.contains(anim_entity) {
            continue;
        }
        let Ok(mut player) = anim_player_query.get_mut(anim_entity) else {
            continue;
        };
        commands
            .entity(anim_entity)
            .try_insert(AnimationGraphHandle(graph_handle.clone()));
        store.play(&key.group, key.key, &mut player, 0.0);
    }
}

/// Ranged aliens shoot the nearest living player within range and line of sight.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn ranged_attacks(
    time: Res<Time>,
    spatial: SpatialQuery,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shooters: Query<(Entity, &Position, &mut RangedAttack), With<Alien>>,
    players: Query<(Entity, &Position), (With<Player>, With<Health>, Without<PlayerDead>)>,
    mut damage_mw: MessageWriter<ApplyDamage>,
) {
    let walls = SpatialQueryFilter::from_mask([CollisionLayer::ImpassableAll]);
    let dt = time.delta_secs();
    for (alien, alien_pos, mut attack) in shooters.iter_mut() {
        attack.cooldown -= dt;
        if attack.cooldown > 0.0 {
            continue;
        }
        let Some((player, player_pos)) = players
            .iter()
            .filter(|(_, p)| p.0.distance(alien_pos.0) <= attack.range)
            .min_by(|(_, a), (_, b)| {
                a.0.distance_squared(alien_pos.0)
                    .total_cmp(&b.0.distance_squared(alien_pos.0))
            })
        else {
            continue;
        };
        let from = alien_pos.0 + Vec3::Y * 0.5;
        let to = player_pos.0 + Vec3::Y * 0.4;
        let delta = to - from;
        let Ok(dir) = Dir3::new(delta) else { continue };
        if spatial
            .cast_ray(from, dir, delta.length(), true, &walls)
            .is_some()
        {
            continue; // wall in the way
        }
        attack.cooldown = attack.shot_interval;
        damage_mw.write(
            ApplyDamage::at(player, attack.damage, DamageKind::Ballistic, to)
                .from(alien)
                .along(delta),
        );

        // A thin green streak so the player can see where it came from.
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 1.0, 0.4),
            emissive: LinearRgba::new(1.0, 4.0, 1.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(mat),
            Transform::from_translation(from + delta * 0.5)
                .looking_to(delta.normalize(), Vec3::Y)
                .with_scale(Vec3::new(0.03, 0.03, delta.length())),
            Ephemeral::new(0.08),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn melee_enemies_have_no_ranged_component() {
        assert!(RangedAttack::from_attack(&EnemyAttack::Melee).is_none());
    }

    #[test]
    fn ranged_interval_comes_from_fire_rate() {
        let r = RangedAttack::from_attack(&EnemyAttack::Ranged {
            damage: 8,
            range: 9.0,
            fire_rate_per_minute: 30.0,
        })
        .unwrap();
        assert!((r.shot_interval - 2.0).abs() < 1e-6);
        assert_eq!(r.damage, 8);
    }
}
