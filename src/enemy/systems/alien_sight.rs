use bevy::core::Name;
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Query, With};
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::{Collider, SpatialQuery, SpatialQueryFilter};
use crate::enemy::components::general::Alien;
use crate::general::components::Layer;

pub fn alien_sight(
    spatial_query: SpatialQuery,
    alien_query: Query<(&Position, &Rotation), With<Alien>>,
    name_query: Query<&Name>
) {
// Cast ray and get up to 20 hits
    for (position, rotation) in alien_query.iter() {

        let shape_rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -90.0, 0.0);
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
        let hits = spatial_query.shape_hits(
            &Collider::cone(5.0, 0.5), // Shape (cone
            position.0,                    // Origin
            shape_rotation,
            direction,// Direction
            5.0,                         // Maximum time of impact (travel distance)
            20,                            // Maximum number of hits
            true,                          // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Player]), // Query filter
        );
        // Print hits
        for hit in hits.iter() {
            if name_query.contains(hit.entity) {
                let name = name_query.get(hit.entity).unwrap();
                println!("Hit: {:?}", name);
            } else {
                println!("Hit: {:?}", hit.entity);
            }
        }
    }



}