use bevy::prelude::Component;
use crate::general::components::map_components::CoolDown;

#[derive(Component)]
pub struct TowerSensor {}

#[derive(Component)]
pub struct TowerShooter {
    pub cool_down: f32,
    pub rate_of_fire_per_minute: f32,
}

impl TowerShooter {
    pub fn new(rate_of_fire_per_minute: f32) -> Self {
        Self {
            cool_down: 0.0,
            rate_of_fire_per_minute,
        }
    }
}

impl CoolDown for TowerShooter {
    fn cool_down(&mut self, delta_seconds: f32) -> bool {
        self.cool_down -= delta_seconds;
        if self.cool_down <= 0.0 {
            self.cool_down = 60.0 / self.rate_of_fire_per_minute;
            true
        } else {
            false
        }
    }
}

/// Slows aliens in sensor range by scaling their velocity each frame.
#[derive(Component)]
pub struct TowerSlow {
    /// Velocity multiplier while in range (e.g. 0.3 = 30% of normal speed).
    pub factor: f32,
}

/// Deals continuous area damage to aliens in sensor range.
#[derive(Component)]
pub struct TowerArea {
    pub damage_per_second: f32,
    pub cool_down: f32,
    pub tick_interval: f32,
}

impl TowerArea {
    pub fn new(damage_per_second: f32, tick_hz: f32) -> Self {
        let interval = 1.0 / tick_hz;
        Self { damage_per_second, cool_down: interval, tick_interval: interval }
    }
}

impl CoolDown for TowerArea {
    fn cool_down(&mut self, delta_seconds: f32) -> bool {
        self.cool_down -= delta_seconds;
        if self.cool_down <= 0.0 {
            self.cool_down = self.tick_interval;
            true
        } else {
            false
        }
    }
}

/// Applied to an alien while it is in range of a slow tower.
/// Removed when the alien leaves all slow zones.
#[derive(Component)]
pub struct Slowed {
    pub factor: f32,
    /// Refreshed each frame the alien is in range; removal when it expires.
    pub ttl: f32,
}
