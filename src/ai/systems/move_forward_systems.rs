use bevy::prelude::{Query, With};
use crate::alien::components::general::Alien;
use crate::control::components::{ControlDirection, CharacterControl};

pub fn move_forward_system(
    mut controller_query: Query<&mut CharacterControl, With<Alien>>,
) {
    for mut controller in controller_query.iter_mut() {
        controller.rotations.clear();
        controller.speed = controller.max_speed;
        controller.directions.insert(ControlDirection::Forward);
    }
}
