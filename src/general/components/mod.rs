use bevy::prelude::Component;
use bevy_xpbd_3d::prelude::PhysicsLayer;

#[derive(Component)]
pub struct Ball {}

#[derive(Component)]
pub struct Wall {}

#[derive(Component)]
pub struct Floor {}

#[derive(Component)]
pub struct HittableTarget {}

#[derive(PhysicsLayer)]
pub enum Layer {
    Floor,
    Ball,
    Wall,
    Alien,
    Player,
}
