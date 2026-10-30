//! `PhysicsGizmos`: collider wireframes, off until F3 (or the editor) enables the config.

use bevy::prelude::*;

use super::components::{Collider, ColliderKind, Position, Rotation, Sensor};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct PhysicsGizmos;

const SOLID: Color = Color::srgb(0.2, 0.9, 1.0);
const SENSOR: Color = Color::srgb(1.0, 0.8, 0.2);

pub(super) fn draw_colliders(
    mut gizmos: Gizmos<PhysicsGizmos>,
    colliders: Query<(&Collider, &Position, &Rotation, Has<Sensor>)>,
) {
    for (collider, position, rotation, sensor) in &colliders {
        let at = Isometry3d::new(position.0, rotation.0);
        let color = if sensor { SENSOR } else { SOLID };
        match collider.kind() {
            ColliderKind::Cuboid(half) => {
                gizmos.primitive_3d(&Cuboid { half_size: *half }, at, color);
            }
            ColliderKind::Sphere(r) => {
                gizmos.primitive_3d(&Sphere::new(*r), at, color);
            }
            ColliderKind::Capsule { radius, half_length } => {
                gizmos.primitive_3d(&Capsule3d::new(*radius, half_length * 2.0), at, color);
            }
            ColliderKind::Cylinder { radius, half_height } => {
                gizmos.primitive_3d(&Cylinder::new(*radius, half_height * 2.0), at, color);
            }
            ColliderKind::Cone { radius, half_height } => {
                gizmos.primitive_3d(&Cone { radius: *radius, height: half_height * 2.0 }, at, color);
            }
            ColliderKind::ConvexHull(points) => {
                for pair in points.windows(2) {
                    if let &[a, b] = pair {
                        gizmos.line(at * a, at * b, color);
                    }
                }
            }
        }
    }
}
