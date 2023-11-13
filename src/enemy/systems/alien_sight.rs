use bevy::core::Name;
use bevy::math::{Vec3};
use bevy::prelude::{Query, With};
use bevy_xpbd_3d::components::{Position, Rotation};
use bevy_xpbd_3d::prelude::{SpatialQuery, SpatialQueryFilter};
use crate::enemy::components::general::{Alien, AlienSightShape};
use crate::general::components::Layer;

pub fn alien_sight(
    spatial_query: SpatialQuery,
    alien_query: Query<(&Position, &Rotation,&AlienSightShape), With<Alien>>,
    name_query: Query<&Name>
) {
// Cast ray and get up to 20 hits
    for (position, rotation, sight_shape) in alien_query.iter() {
        let direction = rotation.0.mul_vec3(Vec3::new(0.0, 0.0, -1.0));
        let hits = spatial_query.shape_hits(
            &sight_shape.shape, // Shape to cast
            position.0, // Origin
            sight_shape.rotation, // Rotation of shape
            direction,// Direction
            sight_shape.range, // Maximum time of impact (travel distance)
            20, // Maximum number of hits
            true, // Does the ray treat colliders as "solid"
            SpatialQueryFilter::new().with_masks([Layer::Player]), // Query for players
        );

        /*
        What do we do know?
        We create some kind of "brain" for this alien, this brain will contain facts about
        the world around it, like types of creates it wants to see and of course features
        of the environment like walls etc. Perhaps it can build a mental model of the world in the
        form of a graph? Yes. It can.
         */
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