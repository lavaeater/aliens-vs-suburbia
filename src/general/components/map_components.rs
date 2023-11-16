use bevy::prelude::Component;
use bevy::reflect::Reflect;

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct AlienGoal {
    pub x: usize,
    pub y: usize
}

impl AlienGoal {
    pub fn new(x: usize, y: usize) -> Self {
        Self {
            x,
            y
        }
    }
}

#[derive(Component, Debug, Reflect, Default)]
pub struct CurrentTile {
    pub tile: (usize, usize)
}

#[derive(Component, Debug, Reflect)]
pub struct AlienSpawnPoint {
    pub spawn_rate_per_minute: f32,
    pub spawn_cool_down: f32
}

impl AlienSpawnPoint {
    pub fn new(spawn_rate_per_minute: f32) -> Self {
        Self {
            spawn_rate_per_minute,
            spawn_cool_down: 0.0
        }
    }
}

pub trait CoolDown {
    /// Returns true if the cool down is finished
    fn cool_down(&mut self, delta: f32) -> bool;
}

impl CoolDown for AlienSpawnPoint {
    fn cool_down(&mut self, delta: f32) -> bool {
        self.spawn_cool_down -= delta;
        if self.spawn_cool_down <= 0.0 {
            self.spawn_cool_down = 60.0 / self.spawn_rate_per_minute;
            return true;
        }
        false
    }
}


#[derive(Component)]
pub struct Floor {}
