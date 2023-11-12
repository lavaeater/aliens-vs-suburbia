use bevy::prelude::Component;
use bonsai_bt::BT;

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
    CanISeePlayer,
    ApproachPlayer,
    AttackPlayer,
}

#[derive(Component)]
pub struct Loiter {}
#[derive(Component)]
pub struct CanISeePlayer {}

#[derive(Component)]
pub struct ApproachPlayer {}

#[derive(Component)]
pub struct AttackPlayer {}

#[derive(Component)]
pub struct BonsaiTree {
    pub tree: BT<AlienBehavior, String, serde_json::Value>,
}

#[derive(Component)]
pub struct BonsaiTreeStatus {
    pub current_action_status: bonsai_bt::Status,
}