use crate::game_state::GameState;
use crate::physics::prelude::{Collider, RigidBody};
use bevy::app::{App, Plugin};
use bevy::ecs::observer::Observer;
use bevy::ecs::resource::IsResource;
use bevy::ecs::system::SystemIdMarker;
use bevy::input::gamepad::Gamepad;
use bevy::picking::pointer::PointerId;
use bevy::prelude::{Commands, Component, Entity, OnExit, Query, Window, With, Without};
use bevy::window::Monitor;
use bevy_seedling::prelude::FirewheelNode;

pub struct ClearGameEntitiesPlugin;

impl Plugin for ClearGameEntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameState::InGame), clear_game_entities)
            .add_systems(OnExit(GameState::ModelShowcase), clear_game_entities);
    }
}

/// An entity that lives for the whole app, spawned once at startup: the sweep leaves it
/// (and so its children) alone.
#[derive(Component)]
pub struct Persistent;

/// Entities that belong to the engine and plugins rather than to a match, and that must
/// survive the sweep. Since Bevy 0.19 resources are entities too (`IsResource`):
/// despawning one runs its discard hook, which queues a command on the entity it was just
/// removed from and panics. Observers, registered systems, gamepads, monitors and picking
/// pointers are entities the app needs for the rest of its life (no pointers means no
/// clicks in the menu, no `Gamepad` means no pads). The audio graph is too: seedling and
/// rusty_music build their buses and pools once at startup, and despawning a node takes it
/// out of the graph for good (no music or sound in the next match). A match's own audio
/// nodes hang off its entities as children, so they still go with their parent.
type EngineOwned = (
    Without<Window>,
    Without<IsResource>,
    Without<Observer>,
    Without<SystemIdMarker>,
    Without<Gamepad>,
    Without<Monitor>,
    Without<PointerId>,
    Without<FirewheelNode>,
    Without<Persistent>,
);

pub fn clear_game_entities(
    mut commands: Commands,
    query: Query<Entity, EngineOwned>,
    physics_query: Query<Entity, (EngineOwned, With<RigidBody>)>,
) {
    // Strip colliders from physics bodies first so avian3d can clear contacts
    // before the entity is despawned — otherwise a debug_assert fires inside avian3d.
    for entity in physics_query.iter() {
        commands.entity(entity).remove::<(Collider, RigidBody)>();
    }
    for entity in query.iter() {
        commands.entity(entity).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Component, Resource, World};

    #[derive(Resource, Default)]
    struct Score(u32);

    #[derive(Component)]
    struct Alien;

    /// The crash this guards: leaving a match despawned resource entities, and the
    /// resource's own discard hook then panicked on the dead entity.
    #[test]
    fn the_sweep_keeps_resources_and_drops_game_entities() {
        let mut world = World::new();
        world.insert_resource(Score(7));
        let alien = world.spawn(Alien).id();

        world
            .run_system_cached(clear_game_entities)
            .expect("sweep runs without panicking");

        assert!(world.get_entity(alien).is_err(), "game entities go");
        assert_eq!(world.resource::<Score>().0, 7, "resources stay");
    }
}
