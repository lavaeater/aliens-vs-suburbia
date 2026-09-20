//! Things lying on the map that players can walk over: med-kits, ammo, guns, coins, keys.
//!
//! An item is `Item(ItemKind)` + `Pickup` on any entity with a `Transform`. They come from
//! three places: map placements (`map_systems` tags `Item`-typed defs), loot drops and
//! dropped inventory (both via [`SpawnItem`], which builds a visual for the kind). Players
//! collect within their `PickupRange`; what a pickup *does* is one `match` in
//! [`pickup_items`], and [`ItemPickedUp`] lets the HUD/SFX react.

use bevy::prelude::*;
use avian3d::prelude::Position;

use crate::assets::asset_definition::{AssetDefinition, ItemKind, ModelType};
use crate::game_state::GameState;
use crate::general::components::Health;
use crate::general::systems::coin_system::{Coin, PickupRange, TeamWallet};
use crate::player::ammo::AmmoPouch;
use crate::player::components::{Player, PlayerDead};
use crate::player::systems::loadout::{SwitchWeapon, WeaponSelect, Weapons};

#[derive(Component, Debug, Clone)]
pub struct Item(pub ItemKind);

/// Marks an [`Item`] as collectable. Carries the bob phase so a pile does not move in
/// lock-step.
#[derive(Component, Debug, Clone, Default)]
pub struct Pickup {
    pub phase: f32,
}

/// Request to drop an item into the world with a visual for its kind.
#[derive(Message, Debug, Clone)]
pub struct SpawnItem {
    pub kind: ItemKind,
    pub position: Vec3,
}

/// A player collected something.
#[derive(Message, Debug, Clone)]
pub struct ItemPickedUp {
    pub player: Entity,
    pub kind: ItemKind,
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnItem>()
            .add_message::<ItemPickedUp>()
            .add_systems(
                Update,
                (spawn_items, pickup_items, bob_pickups).run_if(in_state(GameState::InGame)),
            );
    }
}

const BOB_HEIGHT: f32 = 0.08;
const BOB_HZ: f32 = 1.2;
const SPIN_RADS_PER_SEC: f32 = 1.5;

/// Build the entity for a [`SpawnItem`]: a gun uses its own def's model, everything else
/// gets a small coloured primitive so it still reads without art.
pub fn spawn_items(
    mut requests: MessageReader<SpawnItem>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut seed: Local<u32>,
) {
    for req in requests.read() {
        *seed = seed.wrapping_add(7919);
        let phase = (*seed % 628) as f32 / 100.0;
        let base = (
            Name::new(format!("Item {}", req.kind.label())),
            Item(req.kind.clone()),
            Pickup { phase },
            Transform::from_translation(req.position + Vec3::Y * 0.25),
        );

        let weapon_def = match &req.kind {
            ItemKind::WeaponPickup { def } => AssetDefinition::load_from_def_path(def)
                .filter(|d| matches!(d.model_type, ModelType::Weapon(_))),
            _ => None,
        };
        if let Some(def) = weapon_def {
            let scene = asset_server.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(def.model_path.clone()));
            commands.spawn((
                base,
                bevy::world_serialization::WorldAssetRoot(scene),
            )).insert(Transform::from_translation(req.position + Vec3::Y * 0.25).with_scale(Vec3::splat(def.scale)));
            continue;
        }

        let (color, mesh) = primitive_for(&req.kind, &mut meshes);
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 0.6,
            ..default()
        });
        let mut ec = commands.spawn((base, Mesh3d(mesh), MeshMaterial3d(mat)));
        // GoldDigger vacuums coins by this marker.
        if let ItemKind::Coins { value } = &req.kind {
            ec.insert(Coin { value: *value });
        }
    }
}

fn primitive_for(kind: &ItemKind, meshes: &mut Assets<Mesh>) -> (Color, Handle<Mesh>) {
    match kind {
        ItemKind::HealthPickup { .. } => (Color::srgb(0.95, 0.2, 0.25), meshes.add(Cuboid::new(0.3, 0.18, 0.3))),
        ItemKind::AmmoPickup { .. } => (Color::srgb(0.75, 0.6, 0.2), meshes.add(Cuboid::new(0.25, 0.15, 0.18))),
        ItemKind::WeaponPickup { .. } => (Color::srgb(0.5, 0.5, 0.55), meshes.add(Cuboid::new(0.5, 0.1, 0.12))),
        ItemKind::Coins { .. } => (Color::srgb(1.0, 0.85, 0.1), meshes.add(Sphere::new(0.15))),
        ItemKind::Key { .. } => (Color::srgb(0.3, 0.8, 1.0), meshes.add(Cuboid::new(0.12, 0.25, 0.05))),
        ItemKind::Decorative => (Color::srgb(0.6, 0.6, 0.6), meshes.add(Cuboid::new(0.2, 0.2, 0.2))),
    }
}

/// Decide what happens when `player` touches `kind`. Pure so it is testable: returns
/// whether the item is consumed (and so should despawn), after applying the effect.
pub fn apply_pickup(
    kind: &ItemKind,
    health: &mut Health,
    pouch: &mut AmmoPouch,
    loadout: &mut Weapons,
    wallet: &mut TeamWallet,
) -> PickupOutcome {
    match kind {
        ItemKind::Decorative => PickupOutcome::Ignored,
        ItemKind::HealthPickup { amount } => {
            if health.health >= health.max_health {
                return PickupOutcome::Ignored;
            }
            health.heal(*amount as i32);
            PickupOutcome::Consumed
        }
        ItemKind::AmmoPickup { kind, rounds } => {
            if pouch.is_full(*kind) {
                return PickupOutcome::Ignored;
            }
            pouch.add(*kind, *rounds);
            PickupOutcome::Consumed
        }
        ItemKind::WeaponPickup { def } => {
            let (slot, is_new) = loadout.add(def);
            if is_new {
                PickupOutcome::NewWeapon(slot)
            } else {
                // Already carried: worth one magazine of its ammo instead.
                match AssetDefinition::load_from_def_path(def).map(|d| d.model_type) {
                    Some(ModelType::Weapon(props)) if !pouch.is_full(props.ammo) => {
                        pouch.add(props.ammo, props.magazine);
                        PickupOutcome::Consumed
                    }
                    _ => PickupOutcome::Ignored,
                }
            }
        }
        ItemKind::Coins { value } => {
            wallet.coins += value;
            PickupOutcome::Consumed
        }
        ItemKind::Key { .. } => PickupOutcome::Consumed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupOutcome {
    /// Left on the ground (full health, full pouch, set dressing).
    Ignored,
    Consumed,
    /// Consumed and the gun went into this loadout slot; switch to it.
    NewWeapon(usize),
}

#[allow(clippy::type_complexity)]
pub fn pickup_items(
    mut commands: Commands,
    mut wallet: ResMut<TeamWallet>,
    mut players: Query<
        (Entity, &Position, &PickupRange, &mut Health, &mut AmmoPouch, &mut Weapons),
        (With<Player>, Without<PlayerDead>),
    >,
    items: Query<(Entity, &GlobalTransform, &Item), With<Pickup>>,
    mut picked_mw: MessageWriter<ItemPickedUp>,
    mut switch_mw: MessageWriter<SwitchWeapon>,
    mut taken: Local<Vec<Entity>>,
) {
    taken.clear();
    for (player, pos, range, mut health, mut pouch, mut loadout) in players.iter_mut() {
        for (item_entity, item_gt, item) in items.iter() {
            if taken.contains(&item_entity) {
                continue;
            }
            if pos.0.distance(item_gt.translation()) > range.0 {
                continue;
            }
            let outcome = apply_pickup(&item.0, &mut health, &mut pouch, &mut loadout, &mut wallet);
            if outcome == PickupOutcome::Ignored {
                continue;
            }
            if let PickupOutcome::NewWeapon(slot) = outcome {
                switch_mw.write(SwitchWeapon { player, select: WeaponSelect::Slot(slot) });
            }
            taken.push(item_entity);
            commands.entity(item_entity).despawn();
            picked_mw.write(ItemPickedUp { player, kind: item.0.clone() });
        }
    }
}

/// Spin and bob so pickups read as collectable.
pub fn bob_pickups(time: Res<Time>, mut pickups: Query<(&Pickup, &mut Transform), With<Item>>) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (pickup, mut transform) in pickups.iter_mut() {
        transform.rotate_y(SPIN_RADS_PER_SEC * dt);
        let target_offset = ((t * BOB_HZ * std::f32::consts::TAU) + pickup.phase).sin() * BOB_HEIGHT;
        let prev_offset = (((t - dt) * BOB_HZ * std::f32::consts::TAU) + pickup.phase).sin() * BOB_HEIGHT;
        transform.translation.y += target_offset - prev_offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::AmmoKind;

    fn player() -> (Health, AmmoPouch, Weapons, TeamWallet) {
        (Health { health: 40, max_health: 100 }, AmmoPouch::default(), Weapons::default(), TeamWallet::default())
    }

    #[test]
    fn health_heals_and_is_capped_and_ignored_when_full() {
        let (mut h, mut p, mut w, mut t) = player();
        assert_eq!(apply_pickup(&ItemKind::HealthPickup { amount: 80.0 }, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Consumed);
        assert_eq!(h.health, 100);
        assert_eq!(apply_pickup(&ItemKind::HealthPickup { amount: 10.0 }, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Ignored);
    }

    #[test]
    fn ammo_fills_the_pouch_until_full() {
        let (mut h, mut p, mut w, mut t) = player();
        let kind = ItemKind::AmmoPickup { kind: AmmoKind::Grenade, rounds: 4 };
        assert_eq!(apply_pickup(&kind, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Consumed);
        assert_eq!(apply_pickup(&kind, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Consumed);
        assert_eq!(p.rounds(AmmoKind::Grenade), 6, "capped");
        assert_eq!(apply_pickup(&kind, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Ignored);
    }

    #[test]
    fn a_new_gun_joins_the_loadout_and_asks_to_be_drawn() {
        let (mut h, mut p, mut w, mut t) = player();
        let kind = ItemKind::WeaponPickup { def: "assets/defs/Nope.ron".into() };
        assert_eq!(apply_pickup(&kind, &mut h, &mut p, &mut w, &mut t), PickupOutcome::NewWeapon(0));
        assert_eq!(w.slots.len(), 1);
        // Carried already and the def does not exist: nothing to convert into ammo.
        assert_eq!(apply_pickup(&kind, &mut h, &mut p, &mut w, &mut t), PickupOutcome::Ignored);
    }

    fn pickup_app(player_x: f32, item_x: f32) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<TeamWallet>();
        app.add_message::<ItemPickedUp>();
        app.add_message::<SwitchWeapon>();
        app.add_systems(Update, pickup_items);
        app.world_mut().spawn((
            Player,
            Position(Vec3::new(player_x, 0.0, 0.0)),
            PickupRange(1.8),
            Health::default(),
            AmmoPouch::default(),
            Weapons::default(),
        ));
        let item = app
            .world_mut()
            .spawn((
                Item(ItemKind::Coins { value: 5 }),
                Pickup::default(),
                GlobalTransform::from_translation(Vec3::new(item_x, 0.0, 0.0)),
            ))
            .id();
        (app, item)
    }

    #[test]
    fn an_item_in_range_is_collected_and_despawned() {
        let (mut app, item) = pickup_app(0.0, 1.0);
        app.update();
        assert_eq!(app.world().resource::<TeamWallet>().coins, 5);
        assert!(app.world().get::<Item>(item).is_none(), "picked up");
    }

    #[test]
    fn an_item_out_of_range_stays_put() {
        let (mut app, item) = pickup_app(0.0, 10.0);
        app.update();
        assert_eq!(app.world().resource::<TeamWallet>().coins, 0);
        assert!(app.world().get::<Item>(item).is_some());
    }

    #[test]
    fn coins_go_to_the_team_wallet() {
        let (mut h, mut p, mut w, mut t) = player();
        apply_pickup(&ItemKind::Coins { value: 7 }, &mut h, &mut p, &mut w, &mut t);
        assert_eq!(t.coins, 7);
    }
}
