use bevy::prelude::Component;
use bonsai_bt::BT;

#[derive(Component)]
pub struct BonsaiTree<A,K,V>{
    pub tree: BT<A, K, V>
}

/*
This should, in reality, be a list of all the small behaviors we can
perform, like really granular. They should be applicable to all enemy
entities, the tree itself is defined later. This means that all these behaviors
should have some kind of Component related to them that will be set when
the behavior is active.
This will allow us to construct systems that use these components to perform
the behaviors. Maybe.

We will not tie it together to strictly now
 */
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub enum AlienBehavior {
    Loiter,
    Chase,
    Melee
}