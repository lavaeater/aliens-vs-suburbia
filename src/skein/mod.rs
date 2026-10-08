use bevy::prelude::{ Component, Debug }

#[derive(Component, Debug, Reflection)]
pub struct RightHand {
  pub offset: Vec3
}
