use bevy::prelude::*;
use crate::general::components::map_components::CoolDown;

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct TowerSensor {}

#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
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
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct TowerSlow {
    /// Velocity multiplier while in range (e.g. 0.3 = 30% of normal speed).
    pub factor: f32,
}

/// Deals continuous area damage to aliens in sensor range.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
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
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
 #[type_path = "avs"]
pub struct Slowed {
    pub factor: f32,
    /// Refreshed each frame the alien is in range; removal when it expires.
    pub ttl: f32,
}

#[cfg(test)]
mod tests {
    use super::{TowerArea, TowerShooter};
    use crate::general::components::map_components::{AlienSpawnPoint, CoolDown};

    #[test]
    fn shooter_fires_only_after_its_interval_elapses() {
        // 60 rpm -> one shot per second.
        let mut shooter = TowerShooter::new(60.0);
        assert!(!shooter.cool_down(0.4), "0.4s in: not ready");
        assert!(!shooter.cool_down(0.4), "0.8s in: still not ready");
        assert!(shooter.cool_down(0.4), "past 1.0s: fires");
        // And it re-arms rather than firing every subsequent call.
        assert!(!shooter.cool_down(0.4));
    }

    #[test]
    fn area_tower_ticks_at_its_configured_rate() {
        // 4 Hz -> a tick every 0.25s.
        let mut area = TowerArea::new(10.0, 4.0);
        assert!(!area.cool_down(0.2));
        assert!(area.cool_down(0.2), "0.4s in crosses the 0.25s interval");
    }

    #[test]
    fn spawn_point_cooldown_matches_its_rate() {
        // 120 spawns/min -> one every 0.5s.
        let mut sp = AlienSpawnPoint::new(120.0);
        assert!(!sp.cool_down(0.3));
        assert!(sp.cool_down(0.3), "0.6s in: spawn");
        assert!(!sp.cool_down(0.3), "re-armed for the next 0.5s");
    }
}
