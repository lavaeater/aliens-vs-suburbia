use bevy::math::Vec3;
use bevy::prelude::{MessageWriter, Query, Res, Time, With, Without};
use avian3d::prelude::{CollidingEntities, Position};
use crate::general::components::{Health, TouchDamage};
use crate::gore::components::{DamageDealt, DamageKind};
use crate::player::components::Player;
use crate::player::components::PlayerDead;

#[allow(clippy::type_complexity)]
pub fn touch_damage_system(
    time: Res<Time>,
    damagers: Query<(&CollidingEntities, &TouchDamage, Option<&Position>)>,
    mut players: Query<(&mut Health, Option<&Position>), (With<Player>, Without<PlayerDead>)>,
    mut damage_mw: MessageWriter<DamageDealt>,
) {
    let dt = time.delta_secs();
    for (colliding, touch, damager_pos) in damagers.iter() {
        for &hit in colliding.iter() {
            if let Ok((mut health, player_pos)) = players.get_mut(hit) {
                let amount = (touch.dps * dt) as i32;
                if amount <= 0 {
                    continue;
                }
                health.health -= amount;
                // Blood sprays off the player, away from the thing mauling them.
                let ppos = player_pos.map(|p| p.0).unwrap_or(Vec3::ZERO);
                let apos = damager_pos.map(|p| p.0).unwrap_or(ppos);
                damage_mw.write(DamageDealt {
                    target: hit,
                    position: ppos + Vec3::Y * 0.4,
                    normal: (ppos - apos).normalize_or(Vec3::Y),
                    amount,
                    kind: DamageKind::Blunt,
                    lethal: health.health <= 0,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::touch_damage_system;
    use avian3d::prelude::{CollidingEntities, Position};
    use bevy::ecs::entity::EntityHashSet;
    use bevy::prelude::*;
    use std::time::Duration;
    use crate::general::components::{Health, TouchDamage};
    use crate::gore::components::{DamageDealt, DamageKind};
    use crate::player::components::Player;

    #[derive(Resource, Default)]
    struct Caught(Vec<DamageDealt>);

    fn catch(mut r: MessageReader<DamageDealt>, mut c: ResMut<Caught>) {
        for d in r.read() {
            c.0.push(*d);
        }
    }

    fn overlapping(targets: impl IntoIterator<Item = Entity>) -> CollidingEntities {
        let mut set = EntityHashSet::default();
        set.extend(targets);
        CollidingEntities(set)
    }

    #[test]
    fn a_toucher_damages_the_player_it_overlaps() {
        let mut app = App::new();
        app.add_message::<DamageDealt>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.add_systems(Update, (touch_damage_system, catch).chain());

        let player = app
            .world_mut()
            .spawn((Player, Health { health: 100, max_health: 100 }, Position(Vec3::ZERO)))
            .id();
        app.world_mut().spawn((
            TouchDamage { dps: 100.0 },
            overlapping([player]),
            Position(Vec3::new(1.0, 0.0, 0.0)),
        ));

        // 0.3s at 100 dps -> 30 damage.
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(300));
        app.update();

        assert_eq!(app.world().get::<Health>(player).unwrap().health, 70);
        let caught = &app.world().resource::<Caught>().0;
        assert_eq!(caught.len(), 1);
        assert_eq!(caught[0].kind, DamageKind::Blunt);
        assert_eq!(caught[0].target, player);
    }

    #[test]
    fn no_damage_without_overlap() {
        let mut app = App::new();
        app.add_message::<DamageDealt>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.add_systems(Update, (touch_damage_system, catch).chain());

        let player = app
            .world_mut()
            .spawn((Player, Health { health: 100, max_health: 100 }, Position(Vec3::ZERO)))
            .id();
        // Toucher overlaps nobody.
        app.world_mut().spawn((TouchDamage { dps: 100.0 }, overlapping([]), Position(Vec3::ZERO)));

        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(300));
        app.update();

        assert_eq!(app.world().get::<Health>(player).unwrap().health, 100);
        assert!(app.world().resource::<Caught>().0.is_empty());
    }
}
