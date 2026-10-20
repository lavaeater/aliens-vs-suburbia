use bevy::prelude::*;

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct RightHand {
  pub transform: Transform
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct LeftHand {
    pub transform: Transform,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct LeftShoulder {
    pub transform: Transform,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct RightShoulder {
    pub transform: Transform,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunBarrel {
    pub transform: Transform,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunGrip {
    pub transform: Transform,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunHandGuard {
    pub transform: Transform,
}
