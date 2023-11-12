use bevy::prelude::Component;
use bonsai_bt::BT;
use std::{collections::HashMap, thread::sleep, time::Duration};

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
pub struct Loiter {
    pub status: bonsai_bt::Status,
}

#[derive(Component)]
pub struct CanISeePlayer {
    pub status: bonsai_bt::Status,
}

#[derive(Component)]
pub struct ApproachPlayer {
    pub status: bonsai_bt::Status,
}

#[derive(Component)]
pub struct AttackPlayer {
    pub status: bonsai_bt::Status,
}

trait ReturnStatus {
    fn update(&mut self, status: bonsai_bt::Status);
    fn get_status(&self) -> bonsai_bt::Status;
}

#[derive(Component)]
pub struct BonsaiTree {
    pub tree: BT<AlienBehavior, String, serde_json::Value>
}
