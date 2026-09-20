//! Physics projectiles: grenades, molotovs and whatever a launcher-type gun fires.
//!
//! One [`Projectile`] component covers all of them. What happens on arrival is the
//! [`Impact`]: plain damage to the thing hit, an [`Explode`], or a `SpawnFire`. A
//! projectile with a fuse ignores contacts and detonates when the fuse runs out
//! (grenades bounce); one without detonates on its first contact (molotovs shatter,
//! rockets burst). Throwables are consumed from the player's `AmmoPouch` by
//! [`throw_special`]; launcher guns come through `shoot_weapons` with
//! `WeaponProps.projectile` set.

use avian3d::prelude::{
    Collider, CollisionEventsEnabled, CollisionLayers, CollisionStart, GravityScale, LinearVelocity, Position, RigidBody,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::animation::animation_plugin::{AnimationEvent, AnimationEventType, AnimationKey};
use crate::assets::asset_definition::AmmoKind;
use crate::control::components::{CharacterControl, ControlCommand};
use crate::game_state::score_keeper::GameTrackingEvent;
use crate::general::components::{CollisionLayer, Health};
use crate::general::damage::ApplyDamage;
use crate::general::explosion::{Explode, ExplosionProps};
use crate::gore::components::DamageKind;
use crate::gore::fire::SpawnFire;
use crate::player::ammo::AmmoPouch;
use crate::player::components::{AutoAim, Player, PlayerDead};

/// Gravity magnitude used for lob arcs; matches avian's default.
const GRAVITY: f32 = 9.81;
/// How far a lobbed throwable lands from the thrower on flat ground.
const THROW_RANGE: f32 = 7.0;
const GRENADE_FUSE_SECS: f32 = 2.5;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FireProps {
    pub radius: f32,
    pub duration: f32,
    pub dps: f32,
}

impl Default for FireProps {
    fn default() -> Self {
        Self { radius: 2.2, duration: 6.0, dps: 35.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Impact {
    /// Hurt whatever was hit by this much.
    Damage(i32),
    Explode(ExplosionProps),
    Fire(FireProps),
}

/// Authoring-side description, embedded in `WeaponProps.projectile`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileProps {
    /// Launch speed (world units/s). Ignored for lobbed throwables, whose speed comes
    /// from the throw range.
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_true")]
    pub gravity: bool,
    pub impact: Impact,
    /// Seconds before detonation. `None` = detonate on first contact.
    #[serde(default)]
    pub fuse_secs: Option<f32>,
}

fn default_speed() -> f32 { 18.0 }
fn default_true() -> bool { true }

impl Default for ProjectileProps {
    fn default() -> Self {
        Self { speed: default_speed(), gravity: true, impact: Impact::Explode(ExplosionProps::default()), fuse_secs: None }
    }
}

#[derive(Component, Debug, Clone)]
pub struct Projectile {
    pub source: Entity,
    pub impact: Impact,
    pub fuse: Option<Timer>,
    /// Hard cap so a projectile that never lands does not live forever.
    pub lifetime: Timer,
}

/// Which pocket a thrown special comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrowableKind {
    Grenade,
    Molotov,
}

impl ThrowableKind {
    pub fn ammo(self) -> AmmoKind {
        match self {
            ThrowableKind::Grenade => AmmoKind::Grenade,
            ThrowableKind::Molotov => AmmoKind::Molotov,
        }
    }

    pub fn props(self) -> ProjectileProps {
        match self {
            ThrowableKind::Grenade => ProjectileProps {
                speed: 0.0,
                gravity: true,
                impact: Impact::Explode(ExplosionProps { radius: 2.5, damage: 90, impulse: 9.0, fire: false }),
                fuse_secs: Some(GRENADE_FUSE_SECS),
            },
            ThrowableKind::Molotov => ProjectileProps {
                speed: 0.0,
                gravity: true,
                impact: Impact::Fire(FireProps::default()),
                fuse_secs: None,
            },
        }
    }

    /// Grenades first, molotovs when those run out. `None` = nothing to throw.
    pub fn pick(pouch: &AmmoPouch) -> Option<Self> {
        [ThrowableKind::Grenade, ThrowableKind::Molotov]
            .into_iter()
            .find(|k| pouch.rounds(k.ammo()) > 0)
    }
}

/// Velocity for a 45-degree lob that lands `range` away on flat ground, aimed along the
/// horizontal part of `aim`.
pub fn lob_velocity(aim: Vec3, range: f32) -> Vec3 {
    let flat = Vec3::new(aim.x, 0.0, aim.z).normalize_or(Vec3::X);
    let speed = (range.max(0.5) * GRAVITY).sqrt();
    let c = std::f32::consts::FRAC_1_SQRT_2;
    flat * speed * c + Vec3::Y * speed * c
}

/// Spawn a projectile entity. `velocity` is the launch velocity, already resolved.
pub fn launch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    source: Entity,
    origin: Vec3,
    velocity: Vec3,
    props: &ProjectileProps,
) -> Entity {
    let color = match props.impact {
        Impact::Damage(_) => Color::srgb(0.9, 0.9, 0.6),
        Impact::Explode(_) => Color::srgb(0.25, 0.3, 0.25),
        Impact::Fire(_) => Color::srgb(0.8, 0.4, 0.1),
    };
    let mat = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 0.8,
        ..default()
    });
    commands
        .spawn((
            Name::new("Projectile"),
            Projectile {
                source,
                impact: props.impact.clone(),
                fuse: props.fuse_secs.map(|s| Timer::from_seconds(s, TimerMode::Once)),
                lifetime: Timer::from_seconds(8.0, TimerMode::Once),
            },
            Mesh3d(meshes.add(Sphere::new(0.09))),
            MeshMaterial3d(mat),
            Transform::from_translation(origin),
            RigidBody::Dynamic,
            Collider::sphere(0.09),
            CollisionEventsEnabled,
            GravityScale(if props.gravity { 1.0 } else { 0.0 }),
            LinearVelocity(velocity),
            CollisionLayers::new(
                [CollisionLayer::Ball],
                [
                    CollisionLayer::ImpassableAll,
                    CollisionLayer::Floor,
                    CollisionLayer::Alien,
                    CollisionLayer::Player,
                    CollisionLayer::AlienSpawnPoint,
                    CollisionLayer::AlienGoal,
                ],
            ),
        ))
        .id()
}

/// Carry out a projectile's impact at `position` (optionally against `hit`) and remove it.
#[allow(clippy::too_many_arguments)]
fn detonate(
    commands: &mut Commands,
    entity: Entity,
    projectile: &Projectile,
    position: Vec3,
    hit: Option<Entity>,
    damage_mw: &mut MessageWriter<ApplyDamage>,
    explode_mw: &mut MessageWriter<Explode>,
    fire_mw: &mut MessageWriter<SpawnFire>,
) {
    match &projectile.impact {
        Impact::Damage(amount) => {
            if let Some(target) = hit {
                damage_mw.write(ApplyDamage::at(target, *amount, DamageKind::Ballistic, position).from(projectile.source));
            }
        }
        Impact::Explode(props) => {
            explode_mw.write(Explode { position, props: props.clone(), source: Some(projectile.source) });
        }
        Impact::Fire(props) => {
            fire_mw.write(SpawnFire { position, radius: props.radius, duration: props.duration, dps: props.dps });
        }
    }
    commands.entity(entity).despawn();
}

/// Fuses and lifetimes.
pub fn tick_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Projectile, &Position)>,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut explode_mw: MessageWriter<Explode>,
    mut fire_mw: MessageWriter<SpawnFire>,
) {
    for (entity, mut projectile, pos) in projectiles.iter_mut() {
        projectile.lifetime.tick(time.delta());
        let fused = match projectile.fuse.as_mut() {
            Some(fuse) => {
                fuse.tick(time.delta());
                fuse.is_finished()
            }
            None => false,
        };
        if fused {
            detonate(&mut commands, entity, &projectile, pos.0, None, &mut damage_mw, &mut explode_mw, &mut fire_mw);
        } else if projectile.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Contact handling: fuseless projectiles detonate on whatever they touch first.
#[allow(clippy::too_many_arguments)]
pub fn projectile_impacts(
    mut collisions: MessageReader<CollisionStart>,
    mut commands: Commands,
    projectiles: Query<(&Projectile, &Position)>,
    targets: Query<(), With<Health>>,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut explode_mw: MessageWriter<Explode>,
    mut fire_mw: MessageWriter<SpawnFire>,
    mut done: Local<Vec<Entity>>,
) {
    done.clear();
    for collision in collisions.read() {
        let (entity, other) = if projectiles.contains(collision.collider1) {
            (collision.collider1, collision.collider2)
        } else if projectiles.contains(collision.collider2) {
            (collision.collider2, collision.collider1)
        } else {
            continue;
        };
        if done.contains(&entity) {
            continue;
        }
        let Ok((projectile, pos)) = projectiles.get(entity) else { continue };
        // Grenades bounce; only fuseless projectiles go off on contact. Never on the
        // thrower's own body either.
        if projectile.fuse.is_some() || other == projectile.source {
            continue;
        }
        let hit = targets.contains(other).then_some(other);
        detonate(&mut commands, entity, projectile, pos.0, hit, &mut damage_mw, &mut explode_mw, &mut fire_mw);
        done.push(entity);
    }
}

/// G / L1: lob a grenade (or a molotov once those are gone) from the pouch.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn throw_special(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut players: Query<(Entity, &Position, &AutoAim, &mut CharacterControl, &mut AmmoPouch), (With<Player>, Without<PlayerDead>)>,
    mut anim_ew: MessageWriter<AnimationEvent>,
    mut game_mw: MessageWriter<GameTrackingEvent>,
) {
    for (player, pos, aim, mut control, mut pouch) in players.iter_mut() {
        if !control.triggers.remove(&ControlCommand::ThrowSpecial) {
            continue;
        }
        let Some(kind) = ThrowableKind::pick(&pouch) else { continue };
        pouch.take(kind.ammo(), 1);
        let origin = pos.0 + aim.0.normalize_or(Vec3::X) * 0.5 + Vec3::Y * 0.6;
        launch(&mut commands, &mut meshes, &mut materials, player, origin, lob_velocity(aim.0, THROW_RANGE), &kind.props());
        game_mw.write(GameTrackingEvent::ShotFired(player));
        anim_ew.write(AnimationEvent(AnimationEventType::GotoAnimState, player, AnimationKey::Throwing));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lob_goes_forward_and_up_at_forty_five_degrees() {
        let v = lob_velocity(Vec3::new(1.0, 0.3, 0.0), 7.0);
        assert!((v.x - v.y).abs() < 1e-4, "equal horizontal and vertical parts: {v:?}");
        assert_eq!(v.z, 0.0);
        // Range r at 45 deg needs v^2 = r*g.
        assert!((v.length_squared() - 7.0 * GRAVITY).abs() < 1e-3);
    }

    #[test]
    fn grenades_are_preferred_and_the_pouch_decides() {
        let both = AmmoPouch::from_loadout(&[(AmmoKind::Grenade, 1), (AmmoKind::Molotov, 1)]);
        assert_eq!(ThrowableKind::pick(&both), Some(ThrowableKind::Grenade));
        let molotov_only = AmmoPouch::from_loadout(&[(AmmoKind::Molotov, 2)]);
        assert_eq!(ThrowableKind::pick(&molotov_only), Some(ThrowableKind::Molotov));
        assert_eq!(ThrowableKind::pick(&AmmoPouch::default()), None);
    }

    #[test]
    fn grenades_have_a_fuse_and_molotovs_do_not() {
        assert!(ThrowableKind::Grenade.props().fuse_secs.is_some());
        assert!(ThrowableKind::Molotov.props().fuse_secs.is_none());
        assert!(matches!(ThrowableKind::Molotov.props().impact, Impact::Fire(_)));
    }
}
