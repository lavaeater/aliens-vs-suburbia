use bevy::prelude::*;

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct RightHand {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct LeftHand {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct LeftShoulder {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct RightShoulder {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunBarrel {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunGrip {
    pub offset: Vec3,
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct GunHandGuard {
    pub offset: Vec3,
}
