//! Fire fields: burning ground that damages creatures standing in it over time and sets
//! them alight (`status::burning`), throws up flames (a looping particle effect,
//! `assets/fx/fire.ron`) and smoke, lights its surroundings with a flickering glow, and
//! leaves a scorch mark when it burns out.
//!
//! Driven by the [`SpawnFire`] message so any source can start one: molotovs, fire
//! grenades, the Molotov ability, a flamethrower's dribble. A `spill` (a molotov
//! breaking) splashes: a pool where it broke and splashes thrown round it
//! ([`spill_pattern`]), each its own puddle of flames, all spreading out over the first
//! moments. Damage is [`ApplyDamage`] with [`DamageKind::Fire`], which the blood system
//! skips (burns don't spray) while still feeding death/score.

use crate::physics::prelude::Position;
use bevy::prelude::*;
use bevy_sprinkles::prelude::Particles3d;

use crate::alien::components::general::Alien;
use crate::fx::{FxAssets, FxKind, SpawnFx, StopFx};
use crate::general::components::Health;
use crate::general::damage::ApplyDamage;
use crate::general::resources::map_resources::MapGraph;
use crate::general::systems::lights_systems::{LightAnimation, LightStyle};
use crate::general::systems::map_systems::TileDefinitions;
use crate::gore::components::{DECAL_LIFT, DamageKind, GoreBudget};
use crate::infestation::hunters::tile_of;
use crate::infestation::neighbours::Neighbour;
use crate::map::ground::GroundHeight;
use crate::player::components::Player;
use crate::status::Afflict;

/// Start a fire field. Position is on the ground; radius/duration/dps shape it.
#[derive(Message, Clone, Copy, Debug)]
pub struct SpawnFire {
    pub position: Vec3,
    pub radius: f32,
    pub duration: f32,
    pub dps: f32,
    /// Splash into puddles round the spot (a bottle breaking) instead of one round patch.
    pub spill: bool,
}

/// One patch of burning liquid, relative to its field's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puddle {
    pub offset: Vec2,
    pub radius: f32,
}

/// Standing in the fire sets you alight: this hot (dps)...
const IGNITE_DPS: f32 = 8.0;
/// ...for this long, after you step out.
const IGNITE_SECS: f32 = 3.5;
/// How long a spill takes to spread to its full size.
const SPREAD_SECS: f32 = 0.45;
/// A spill starts at this share of its size.
const SPREAD_FROM: f32 = 0.3;
/// Fire light per metre of radius, in lumens (a lantern is 14 000).
const LUMENS_PER_METRE: f32 = 16_000.0;

/// A live patch of fire.
#[derive(Component)]
pub struct FireField {
    life: Timer,
    /// Applies damage on each fire of this timer (repeating).
    damage_tick: Timer,
    dps: f32,
    radius: f32,
    puddles: Vec<Puddle>,
    /// Seconds since it started, for the spread.
    age: f32,
    /// The flames' particle systems, their own entities so they can outlive the field by
    /// the time their last flames take to die out.
    flames: Vec<Entity>,
    /// The smoke over it (`assets/fx/smoke.ron`), likewise.
    smoke: Option<Entity>,
    /// Spreads (a spill) or is there at once.
    spreads: bool,
}

impl FireField {
    /// How often the field applies damage (also the window each tick's damage covers).
    const DAMAGE_INTERVAL: f32 = 0.25;
    /// How long the flames keep going after the field burns out: the longest particle
    /// lifetime in `fire.ron` (embers).
    const FLAME_LINGER: f32 = 1.2;
    /// How long the smoke goes on after: its longest puff in `smoke.ron`.
    const SMOKE_LINGER: f32 = 4.4;

    /// One round patch.
    #[cfg(test)]
    pub fn new(duration: f32, dps: f32, radius: f32) -> Self {
        Self::with_puddles(
            duration,
            dps,
            vec![Puddle {
                offset: Vec2::ZERO,
                radius,
            }],
            false,
        )
    }

    pub fn with_puddles(duration: f32, dps: f32, puddles: Vec<Puddle>, spreads: bool) -> Self {
        let radius = puddles
            .iter()
            .map(|p| p.offset.length() + p.radius)
            .fold(0.0, f32::max);
        Self {
            life: Timer::from_seconds(duration, TimerMode::Once),
            damage_tick: Timer::from_seconds(Self::DAMAGE_INTERVAL, TimerMode::Repeating),
            dps,
            radius,
            puddles,
            age: 0.0,
            flames: Vec::new(),
            smoke: None,
            spreads,
        }
    }

    /// How far it has spread, 0..=1 of its full size.
    fn spread(&self) -> f32 {
        if self.spreads {
            spread_at(self.age)
        } else {
            1.0
        }
    }

    /// Whether `at` (on the floor plane, relative to the centre) is in the fire now.
    fn covers(&self, at: Vec2) -> bool {
        let s = self.spread();
        self.puddles
            .iter()
            .any(|p| at.distance_squared(p.offset * s) <= (p.radius * s).powi(2))
    }
}

/// The share of its size a spill has reached `age` seconds in.
pub fn spread_at(age: f32) -> f32 {
    let t = (age / SPREAD_SECS).clamp(0.0, 1.0);
    // Fast at first, easing out: liquid splashing.
    let eased = 1.0 - (1.0 - t).powi(2);
    SPREAD_FROM + (1.0 - SPREAD_FROM) * eased
}

/// The puddles a bottle of `radius` breaking makes: a pool where it broke and four to six
/// splashes round it, at odd distances and sizes, the same for the same `seed`.
pub fn spill_pattern(seed: u32, radius: f32) -> Vec<Puddle> {
    let mut x = seed.wrapping_mul(0x9E37_79B9) | 1;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut puddles = vec![Puddle {
        offset: Vec2::ZERO,
        radius: radius * 0.6,
    }];
    let splashes = 4 + (next() * 3.0) as usize;
    let turn = next() * std::f32::consts::TAU;
    for i in 0..splashes {
        let angle = turn
            + i as f32 / splashes as f32 * std::f32::consts::TAU
            + (next() - 0.5) * 0.9;
        let distance = radius * (0.55 + next() * 0.75);
        puddles.push(Puddle {
            offset: Vec2::from_angle(angle) * distance,
            radius: radius * (0.22 + next() * 0.23),
        });
    }
    puddles
}

/// Shared fire art.
#[derive(Resource)]
pub struct FireAssets {
    scorch_mesh: Handle<Mesh>,
    scorch_material: Handle<StandardMaterial>,
    pool_mesh: Handle<Mesh>,
    pool_material: Handle<StandardMaterial>,
}

pub fn setup_fire_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let scorch_mesh = meshes.add(Mesh::from(Plane3d::new(Vec3::Y, Vec2::splat(0.5))));
    let scorch_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.05, 0.04, 0.03, 0.9),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        depth_bias: 1.0,
        ..default()
    });
    let pool_mesh = meshes.add(Mesh::from(Sphere::new(0.5)));
    let pool_material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.35, 0.05, 0.5),
        emissive: LinearRgba::new(5.0, 1.6, 0.2, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(FireAssets {
        scorch_mesh,
        scorch_material,
        pool_mesh,
        pool_material,
    });
}

/// Spawn a `FireField` entity for each `SpawnFire`: a glowing pool per puddle (children,
/// so the spread scales them out), the flames over each, smoke over the middle and the
/// firelight.
#[allow(clippy::too_many_arguments)]
pub fn spawn_fire_fields(
    mut msgs: MessageReader<SpawnFire>,
    mut commands: Commands,
    fx: Option<Res<FxAssets>>,
    fire_assets: Option<Res<FireAssets>>,
    map_graph: Option<Res<MapGraph>>,
    tile_defs: Option<Res<TileDefinitions>>,
    ground: Option<Res<GroundHeight>>,
    mut fx_mw: MessageWriter<SpawnFx>,
) {
    for fire in msgs.read() {
        // On the ground under where it landed (a bottle can break on a monster's chest).
        let mut center = fire.position;
        if let (Some(tiles), Some(ground)) = (tile_defs.as_ref(), ground.as_ref()) {
            center.y = tiles.floor_level + ground.height(center.x, center.z);
        }
        let puddles: Vec<Puddle> = if fire.spill {
            let seed = (center.x * 73.0) as i32 as u32 ^ ((center.z * 151.0) as i32 as u32) << 7;
            spill_pattern(seed, fire.radius)
                .into_iter()
                .enumerate()
                // Liquid does not splash through walls: keep the pool, and the splashes
                // that land on open floor.
                .filter(|(i, p)| {
                    let at = center + Vec3::new(p.offset.x, 0.0, p.offset.y);
                    *i == 0
                        || map_graph.as_ref().is_none_or(|g| {
                            at.x >= 0.0 && at.z >= 0.0 && g.path_finding_grid.has_vertex(tile_of(at))
                        })
                })
                .map(|(_, p)| p)
                .collect()
        } else {
            vec![Puddle {
                offset: Vec2::ZERO,
                radius: fire.radius,
            }]
        };
        if fire.spill {
            fx_mw.write(SpawnFx::at(FxKind::FireSplash, center + Vec3::Y * 0.1));
        }

        let mut field = FireField::with_puddles(fire.duration, fire.dps, puddles.clone(), fire.spill);
        if let Some(fx) = fx.as_ref() {
            field.flames = puddles
                .iter()
                .map(|p| {
                    commands
                        .spawn((
                            Name::new("Flames"),
                            Particles3d(fx.fire.clone()),
                            Transform::from_translation(center + Vec3::new(p.offset.x, 0.0, p.offset.y))
                                .with_scale(Vec3::new(p.radius, 1.0, p.radius)),
                        ))
                        .id()
                })
                .collect();
            let smoke_r = fire.radius.max(0.6);
            field.smoke = Some(
                commands
                    .spawn((
                        Name::new("Smoke"),
                        Particles3d(fx.smoke.clone()),
                        Transform::from_translation(center + Vec3::Y * 0.4)
                            .with_scale(Vec3::new(smoke_r, 1.0, smoke_r)),
                    ))
                    .id(),
            );
        }
        let lumens = LUMENS_PER_METRE * field.radius.max(0.5);
        let start = if field.spreads { SPREAD_FROM } else { 1.0 };
        commands
            .spawn((
                Name::new("Fire"),
                field,
                Transform::from_translation(center).with_scale(Vec3::new(start, 1.0, start)),
                Visibility::default(),
            ))
            .with_children(|parent| {
                if let Some(art) = fire_assets.as_ref() {
                    for p in &puddles {
                        parent.spawn((
                            Mesh3d(art.pool_mesh.clone()),
                            MeshMaterial3d(art.pool_material.clone()),
                            Transform::from_xyz(p.offset.x, 0.12, p.offset.y).with_scale(Vec3::new(
                                p.radius * 1.4,
                                0.25,
                                p.radius * 1.4,
                            )),
                        ));
                    }
                }
                parent.spawn((
                    Name::new("Firelight"),
                    PointLight {
                        color: Color::srgb(1.0, 0.5, 0.15),
                        intensity: lumens,
                        range: fire.radius * 2.0 + 4.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    LightAnimation {
                        base: lumens,
                        style: LightStyle::Flicker {
                            seed: center.x * 0.13 + center.z * 0.71,
                        },
                    },
                    Transform::from_xyz(0.0, 0.8, 0.0),
                ));
            });
    }
}

/// Burn things in the fire, and on burnout scorch the ground and let the flames die.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_fire_fields(
    time: Res<Time>,
    mut commands: Commands,
    mut budget: ResMut<GoreBudget>,
    fire_assets: Option<Res<FireAssets>>,
    mut fields: Query<(Entity, &mut FireField, &mut Transform)>,
    targets: Query<
        (Entity, &Position),
        (With<Health>, Or<(With<Alien>, With<Player>, With<Neighbour>)>),
    >,
    mut damage_mw: MessageWriter<ApplyDamage>,
    mut afflict_mw: MessageWriter<Afflict>,
    tile_defs: Res<TileDefinitions>,
    ground: Res<GroundHeight>,
) {
    let dt = time.delta();
    for (entity, mut field, mut transform) in fields.iter_mut() {
        field.life.tick(dt);
        field.damage_tick.tick(dt);
        field.age += dt.as_secs_f32();
        if field.spreads {
            let s = field.spread();
            transform.scale = Vec3::new(s, 1.0, s);
        }

        let center = transform.translation;

        // ── Burn creatures in the fire on each damage tick, and set them alight. ──
        if field.damage_tick.just_finished() {
            let amount = (field.dps * field.damage_tick.duration().as_secs_f32()) as i32;
            let reach = field.radius;
            for (target, pos) in targets.iter() {
                let at = pos.0.xz() - center.xz();
                if at.length_squared() > reach * reach
                    || (pos.0.y - center.y).abs() > 2.0
                    || !field.covers(at)
                {
                    continue;
                }
                if amount > 0 {
                    damage_mw.write(ApplyDamage::at(
                        target,
                        amount,
                        DamageKind::Fire,
                        pos.0 + Vec3::Y * 0.4,
                    ));
                }
                afflict_mw.write(Afflict::burning(target, IGNITE_DPS, IGNITE_SECS));
            }
        }

        // ── Burn out: scorch the ground, then despawn. ─────────────────────────
        if field.life.is_finished() {
            for &flames in &field.flames {
                commands
                    .entity(flames)
                    .insert(StopFx::after(FireField::FLAME_LINGER));
            }
            if let Some(smoke) = field.smoke {
                commands
                    .entity(smoke)
                    .insert(StopFx::after(FireField::SMOKE_LINGER));
            }
            if let Some(fa) = fire_assets.as_ref() {
                for p in &field.puddles {
                    let x = center.x + p.offset.x;
                    let z = center.z + p.offset.y;
                    let scorch = commands
                        .spawn((
                            Mesh3d(fa.scorch_mesh.clone()),
                            MeshMaterial3d(fa.scorch_material.clone()),
                            Transform::from_xyz(
                                x,
                                tile_defs.floor_level + ground.height(x, z) + DECAL_LIFT,
                                z,
                            )
                            .with_rotation(ground.lie(x, z))
                            .with_scale(Vec3::splat(p.radius * 2.2)),
                        ))
                        .id();
                    if let Some(evicted) = budget.push_decal(scorch) {
                        commands.entity(evicted).despawn();
                    }
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FireField, tick_fire_fields};
    use crate::alien::components::general::Alien;
    use crate::game_state::score_keeper::GameTrackingEvent;
    use crate::general::components::Health;
    use crate::general::damage::{ApplyDamage, DamageRules, apply_damage};
    use crate::general::systems::map_systems::TileDefinitions;
    use crate::map::ground::GroundHeight;
    use crate::gore::components::{DamageDealt, DamageKind, GoreBudget};
    use crate::physics::prelude::Position;
    use bevy::prelude::*;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Caught(Vec<DamageDealt>);

    fn catch(mut r: MessageReader<DamageDealt>, mut c: ResMut<Caught>) {
        for d in r.read() {
            c.0.push(*d);
        }
    }

    #[test]
    fn fire_burns_creatures_inside_the_radius_only() {
        let mut app = App::new();
        app.add_message::<DamageDealt>();
        app.add_message::<ApplyDamage>();
        app.add_message::<crate::status::Afflict>();
        app.add_message::<GameTrackingEvent>();
        app.init_resource::<DamageRules>();
        app.init_resource::<Caught>();
        app.init_resource::<Time>();
        app.init_resource::<GoreBudget>();
        // Scorch marks are placed on the floor plane, so the tile metrics have to exist.
        app.insert_resource(TileDefinitions::new(1.0, 32.0, 9.5, 1.0));
        app.init_resource::<GroundHeight>();
        // No FireAssets / FxAssets -> no scorch, no flames; the burning is what's tested.
        app.add_systems(Update, (tick_fire_fields, apply_damage, catch).chain());

        // Fire at the origin: radius 2, 40 dps.
        app.world_mut().spawn((
            FireField::new(5.0, 40.0, 2.0),
            Transform::from_translation(Vec3::ZERO),
        ));
        let inside = app
            .world_mut()
            .spawn((
                Alien,
                Health {
                    health: 100,
                    max_health: 100,
                },
                Position(Vec3::new(1.0, 0.0, 0.0)),
            ))
            .id();
        let outside = app
            .world_mut()
            .spawn((
                Alien,
                Health {
                    health: 100,
                    max_health: 100,
                },
                Position(Vec3::new(9.0, 0.0, 0.0)),
            ))
            .id();

        // 0.3s -> one 0.25s damage tick -> 40 * 0.25 = 10 damage.
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(300));
        app.update();

        assert_eq!(
            app.world().get::<Health>(inside).unwrap().health,
            90,
            "creature in the fire burns"
        );
        assert_eq!(
            app.world().get::<Health>(outside).unwrap().health,
            100,
            "creature outside is safe"
        );

        let caught = &app.world().resource::<Caught>().0;
        assert_eq!(caught.len(), 1);
        assert_eq!(caught[0].target, inside);
        assert_eq!(caught[0].kind, DamageKind::Fire);
    }

    #[test]
    fn a_spill_is_a_pool_and_splashes_round_it() {
        for seed in 0..50 {
            let puddles = super::spill_pattern(seed, 2.0);
            assert_eq!(puddles[0].offset, Vec2::ZERO, "the pool is where it broke");
            assert!((5..=7).contains(&puddles.len()), "{} puddles", puddles.len());
            for p in &puddles[1..] {
                let d = p.offset.length();
                assert!((1.0..=2.7).contains(&d), "a splash {d} m out");
                assert!(p.radius < 1.0);
            }
        }
        assert_eq!(super::spill_pattern(7, 2.0), super::spill_pattern(7, 2.0));
    }

    #[test]
    fn a_spill_spreads_out_and_stops() {
        assert!((super::spread_at(0.0) - 0.3).abs() < 1e-5);
        assert!(super::spread_at(0.2) > super::spread_at(0.1));
        assert!((super::spread_at(5.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn only_the_puddles_burn_not_the_gaps_between() {
        let field = FireField::with_puddles(
            5.0,
            10.0,
            vec![
                super::Puddle { offset: Vec2::ZERO, radius: 0.5 },
                super::Puddle { offset: Vec2::new(2.0, 0.0), radius: 0.4 },
            ],
            false,
        );
        assert!(field.covers(Vec2::new(0.3, 0.0)));
        assert!(field.covers(Vec2::new(2.2, 0.0)));
        assert!(!field.covers(Vec2::new(1.2, 0.0)), "the gap is dry");
    }
}
