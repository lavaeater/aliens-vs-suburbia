use bevy::math::Vec3;
use bevy::prelude::{Entity, MessageWriter, Or, Query, Res, Time, With, Without};
use avian3d::prelude::{CollidingEntities, Position};
use crate::alien::components::general::Alien;
use crate::general::components::{Health, TouchDamage};
use crate::general::damage::ApplyDamage;
use crate::gore::components::DamageKind;
use crate::player::components::Player;
use crate::player::components::PlayerDead;

/// Anything with [`TouchDamage`] grinds down the creatures it overlaps. Which creatures
/// it may hurt (an alien mauling a player, a whirlwinding player shredding aliens, never a
/// teammate) is the damage rules' decision, not this system's.
#[allow(clippy::type_complexity)]
pub fn touch_damage_system(
    time: Res<Time>,
    damagers: Query<(Entity, &CollidingEntities, &TouchDamage, Option<&Position>)>,
    creatures: Query<Option<&Position>, (With<Health>, Or<(With<Player>, With<Alien>)>, Without<PlayerDead>)>,
    mut damage_mw: MessageWriter<ApplyDamage>,
) {
    let dt = time.delta_secs();
    for (damager, colliding, touch, damager_pos) in damagers.iter() {
        let amount = (touch.dps * dt) as i32;
        if amount <= 0 {
            continue;
        }
        for &hit in colliding.iter() {
            if hit == damager {
                continue;
            }
            let Ok(target_pos) = creatures.get(hit) else { continue };
            // Blood sprays off the victim, away from the thing mauling them.
            let vpos = target_pos.map_or(Vec3::ZERO, |p| p.0);
            let apos = damager_pos.map_or(vpos, |p| p.0);
            damage_mw.write(
                ApplyDamage::at(hit, amount, DamageKind::Blunt, vpos + Vec3::Y * 0.4)
                    .from(damager)
                    .along(vpos - apos),
            );
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
    use crate::game_state::score_keeper::GameTrackingEvent;
    use crate::general::components::{Health, TouchDamage};
    use crate::general::damage::{apply_damage, ApplyDamage, DamageRules};
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
        app.add_message::<ApplyDamage>();
        app.add_message::<GameTrackingEvent>();
        app.init_resource::<DamageRules>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.add_systems(Update, (touch_damage_system, apply_damage, catch).chain());

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
        app.add_message::<ApplyDamage>();
        app.add_message::<GameTrackingEvent>();
        app.init_resource::<DamageRules>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.add_systems(Update, (touch_damage_system, apply_damage, catch).chain());

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
