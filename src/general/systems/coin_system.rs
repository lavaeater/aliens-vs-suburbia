use bevy::prelude::*;
use avian3d::prelude::Position;
use crate::alien::components::general::Alien;
use crate::general::components::Health;
use crate::player::components::Player;

/// Shared team wallet — all players draw from and deposit into the same pool.
#[derive(Resource, Default)]
pub struct TeamWallet {
    pub coins: u32,
}

/// Component on coin pickup entities.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct Coin {
    pub value: u32,
}

/// How close (world units) a player must be to auto-collect coins.
#[derive(Component, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct PickupRange(pub f32);

impl Default for PickupRange {
    fn default() -> Self { Self(1.8) }
}

/// Spawn a coin entity at each alien's position when it dies.
pub fn spawn_coins_on_alien_death(
    dying: Query<(&Health, &Position), With<Alien>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (health, pos) in &dying {
        if health.health <= 0 {
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.85, 0.1),
                emissive: LinearRgba::new(1.5, 1.2, 0.0, 1.0),
                unlit: true,
                ..default()
            });
            commands.spawn((
                Coin { value: 5 },
                Mesh3d(meshes.add(Sphere::new(0.15))),
                MeshMaterial3d(mat),
                Transform::from_translation(pos.0 + Vec3::Y * 0.3),
            ));
        }
    }
}

/// Players automatically collect coins within their PickupRange.
pub fn coin_pickup_system(
    mut wallet: ResMut<TeamWallet>,
    players: Query<(&Transform, &PickupRange), With<Player>>,
    coins: Query<(Entity, &Transform, &Coin)>,
    mut commands: Commands,
) {
    for (player_transform, range) in &players {
        for (coin_entity, coin_transform, coin) in &coins {
            let dist = player_transform.translation.distance(coin_transform.translation);
            if dist <= range.0 {
                wallet.coins += coin.value;
                commands.entity(coin_entity).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{coin_pickup_system, Coin, PickupRange, TeamWallet};
    use bevy::prelude::*;
    use crate::player::components::Player;

    fn app_with_player_at(x: f32, range: f32) -> App {
        let mut app = App::new();
        app.init_resource::<TeamWallet>();
        app.add_systems(Update, coin_pickup_system);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(x, 0.0, 0.0), PickupRange(range)));
        app
    }

    #[test]
    fn a_coin_in_range_is_collected_into_the_wallet_and_despawned() {
        let mut app = app_with_player_at(0.0, 1.8);
        let coin = app
            .world_mut()
            .spawn((Coin { value: 5 }, Transform::from_xyz(1.0, 0.0, 0.0)))
            .id();

        app.update();

        assert_eq!(app.world().resource::<TeamWallet>().coins, 5);
        assert!(app.world().get::<Coin>(coin).is_none(), "the coin should be picked up");
    }

    #[test]
    fn a_coin_out_of_range_is_left_alone() {
        let mut app = app_with_player_at(0.0, 1.8);
        let coin = app
            .world_mut()
            .spawn((Coin { value: 5 }, Transform::from_xyz(10.0, 0.0, 0.0)))
            .id();

        app.update();

        assert_eq!(app.world().resource::<TeamWallet>().coins, 0);
        assert!(app.world().get::<Coin>(coin).is_some(), "a distant coin stays on the ground");
    }
}
