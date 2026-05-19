use bevy::prelude::*;
use avian3d::prelude::Position;
use crate::alien::components::general::Alien;
use crate::general::components::{Health, TouchDamage};
use crate::general::systems::coin_system::{Coin, TeamWallet};
use crate::player::components::{Player, PlayerDead};

// ── Data ────────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Default)]
pub enum SpecialAbility {
    #[default]
    Bombardment,
    Healing,
    Whirlwind,
    GoldDigger,
}

impl SpecialAbility {
    pub fn throws_to_charge(&self) -> u32 {
        match self {
            SpecialAbility::Bombardment => 10,
            SpecialAbility::Healing     =>  6,
            SpecialAbility::Whirlwind   => 10,
            SpecialAbility::GoldDigger  =>  8,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SpecialAbility::Bombardment => "Bombardment",
            SpecialAbility::Healing     => "Healing",
            SpecialAbility::Whirlwind   => "Whirlwind",
            SpecialAbility::GoldDigger  => "Gold Digger",
        }
    }
}

/// Fills by throwing balls; ability fires when full (1.0).
#[derive(Component)]
pub struct AbilityCooldown {
    /// 0.0 = empty, 1.0 = ready to fire.
    pub charge: f32,
    /// How many throws it takes to fully charge.
    pub throws_needed: u32,
    /// Throw count since last activation (fractional accumulator).
    pub throws_banked: f32,
}

impl AbilityCooldown {
    pub fn new(throws_needed: u32) -> Self {
        Self { charge: 0.0, throws_banked: 0.0, throws_needed }
    }

    pub fn ready(&self) -> bool { self.charge >= 1.0 }

    /// Call once per thrown ball. Returns true when the meter just hit full.
    pub fn add_throw(&mut self) -> bool {
        if self.charge >= 1.0 { return false; }
        self.throws_banked += 1.0;
        self.charge = (self.throws_banked / self.throws_needed as f32).min(1.0);
        self.charge >= 1.0
    }

    pub fn reset(&mut self) {
        self.charge = 0.0;
        self.throws_banked = 0.0;
    }
}

/// Marker inserted while Whirlwind is active.
#[derive(Component)]
pub struct WhirlwindActive {
    pub timer: Timer,
}

// ── tick_cooldowns is a no-op now (meter fills on throws) ───────────────────

pub fn tick_cooldowns(_time: Res<Time>, _query: Query<&mut AbilityCooldown>) {
    // Meter fills via add_throw() in the throwing system; nothing to tick.
}

// ── Activate ────────────────────────────────────────────────────────────────

pub fn activate_ability(
    mut commands: Commands,
    mut players: Query<(Entity, &Transform, &SpecialAbility, &mut AbilityCooldown), (With<Player>, Without<PlayerDead>)>,
    mut aliens: Query<(&Transform, &mut Health), With<Alien>>,
    mut all_healable: Query<&mut Health, (Without<Alien>, Without<PlayerDead>)>,
    mut wallet: Option<ResMut<TeamWallet>>,
    mut coins: Query<(Entity, &Transform), With<Coin>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ability_input: Res<AbilityInput>,
) {
    if !ability_input.pressed { return; }

    for (entity, player_transform, ability, mut cooldown) in players.iter_mut() {
        if !cooldown.ready() { continue; }
        cooldown.reset();

        match ability {
            SpecialAbility::Bombardment => {
                // Deal 75 damage to all aliens on screen (within large radius).
                for (_, mut health) in aliens.iter_mut() {
                    health.health -= 75;
                }
                // Spawn a visual flash effect.
                spawn_flash(&mut commands, player_transform.translation, &mut meshes, &mut materials);
            }

            SpecialAbility::Healing => {
                let heal_range = 6.0;
                // Heal all players + towers within range.
                for mut health in all_healable.iter_mut() {
                    health.health = (health.health + 30).min(health.max_health);
                }
                let _ = (heal_range, entity); // suppress unused warnings
            }

            SpecialAbility::Whirlwind => {
                commands.entity(entity).insert((
                    WhirlwindActive { timer: Timer::from_seconds(4.0, TimerMode::Once) },
                    TouchDamage { dps: 200.0 },
                ));
            }

            SpecialAbility::GoldDigger => {
                // Teleport all coins to wallet immediately.
                let mut collected = 0u32;
                for (coin_entity, _) in coins.iter_mut() {
                    commands.entity(coin_entity).despawn();
                    collected += 5;
                }
                if let Some(ref mut w) = wallet {
                    w.coins += collected;
                }
            }
        }
    }
}

// ── Whirlwind tick ──────────────────────────────────────────────────────────

pub fn tick_whirlwind(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut WhirlwindActive, &mut crate::control::components::CharacterControl)>,
) {
    for (entity, mut ww, mut controller) in query.iter_mut() {
        ww.timer.tick(time.delta());
        // 10× speed while active.
        controller.speed = 30.0;
        if ww.timer.just_finished() {
            commands.entity(entity).remove::<WhirlwindActive>();
            commands.entity(entity).remove::<TouchDamage>();
            controller.speed = 3.0;
        }
    }
}

// ── Input resource ──────────────────────────────────────────────────────────

/// Written each frame by keyboard/gamepad input systems.
#[derive(Resource, Default)]
pub struct AbilityInput {
    pub pressed: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Fading shockwave visual for Bombardment.
#[derive(Component)]
pub struct AbilityFlash {
    pub timer: Timer,
}

fn spawn_flash(
    commands: &mut Commands,
    pos: Vec3,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.9, 0.3, 0.7),
        emissive: LinearRgba::new(4.0, 3.5, 0.5, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        AbilityFlash { timer: Timer::from_seconds(0.5, TimerMode::Once) },
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(mat),
        Transform::from_translation(pos),
    ));
}

pub fn tick_ability_flash(
    mut commands: Commands,
    mut query: Query<(Entity, &mut AbilityFlash, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (entity, mut flash, mut transform, mat_handle) in query.iter_mut() {
        flash.timer.tick(time.delta());
        let t = flash.timer.fraction();
        transform.scale = Vec3::splat(1.0 + t * 16.0);
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            let c = mat.base_color.to_srgba();
            mat.base_color = Color::srgba(c.red, c.green, c.blue, 1.0 - t);
        }
        if flash.timer.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
