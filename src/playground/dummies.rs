//! Target dummies: things to shoot at in the playground.
//!
//! A dummy is a real [`Alien`] — same collision layers, same health, same gore and coin
//! drops — with two changes: its rigid body is `Static` so it stands still instead of
//! pathing toward the goal, and it deals no touch damage so you can walk through the
//! middle of them while tweaking without dying.
//!
//! Being a genuine `Alien` is what makes them useful: `auto_aim` targets `With<Alien>`,
//! so aiming, the torso twist and the whole shooting chain behave exactly as they do in a
//! real match.
//!
//! Each dummy is owned by a [`DummyPost`] that respawns it a few seconds after it dies,
//! so the arena never runs out of targets.

use avian3d::prelude::{Position, RigidBody};
use bevy::prelude::*;

use crate::ai::components::avoid_wall_components::AvoidWallsData;
use crate::ai::components::move_towards_goal_components::MoveTowardsGoalData;
use crate::alien::components::general::{Alien, AlienCounter};
use crate::assets::assets_plugin::GameAssets;
use crate::building::systems::ToWorldCoordinates;
use crate::general::components::TouchDamage;
use crate::general::systems::map_systems::TileDefinitions;
use crate::ui::spawn_ui::{AddHealthBar, StateMarker};

/// Seconds between a dummy dying and its replacement appearing.
const RESPAWN_DELAY: f32 = 3.0;

/// Tile coordinates of the dummy posts, in *padded* map space — `map_loader` wraps the
/// authored grid in a one-tile border, so these are the authored coordinates plus one.
/// The player spawns at the centre of the 16x16 arena, padded (9, 9).
const POST_TILES: [(usize, usize); 3] = [(5, 5), (9, 4), (13, 5)];

/// A fixed spot in the arena that keeps a dummy standing on it.
#[derive(Component)]
pub struct DummyPost {
    pub position: Vec3,
    /// The dummy currently standing here, if it is still alive.
    pub occupant: Option<Entity>,
    /// Counts down while the post is empty.
    pub respawn_in: f32,
}

/// Marks the dummy itself, so it can be told apart from a real alien.
#[derive(Component)]
pub struct TargetDummy;

/// Place the posts. The dummies themselves are spawned by [`respawn_dummies`] on the next
/// frame, which keeps the spawn logic in exactly one place.
pub fn spawn_dummy_posts(mut commands: Commands, tile_defs: Res<TileDefinitions>) {
    for (col, row) in POST_TILES {
        let position = (col, row).to_world_coords(&tile_defs) + Vec3::new(0.0, 1.0, 0.0);
        commands.spawn((
            Name::from("Dummy Post"),
            DummyPost { position, occupant: None, respawn_in: 0.0 },
            StateMarker,
        ));
    }
}

/// Keep every post occupied. A dummy that has been despawned by `health_monitor_system`
/// leaves its post empty; after [`RESPAWN_DELAY`] a fresh one takes its place.
pub fn respawn_dummies(
    mut commands: Commands,
    time: Res<Time>,
    game_assets: Res<GameAssets>,
    mut alien_counter: ResMut<AlienCounter>,
    mut posts: Query<&mut DummyPost>,
    alive: Query<(), With<TargetDummy>>,
    mut add_health_bar_mw: MessageWriter<AddHealthBar>,
) {
    for mut post in posts.iter_mut() {
        if post.occupant.is_some_and(|e| alive.contains(e)) {
            continue;
        }
        if post.occupant.take().is_some() {
            // Just died — start the countdown rather than popping back instantly.
            post.respawn_in = RESPAWN_DELAY;
        }
        post.respawn_in -= time.delta_secs();
        if post.respawn_in > 0.0 {
            continue;
        }

        let dummy = commands
            .spawn((
                Name::from("Target Dummy"),
                Alien,
                TargetDummy,
                // Overrides `Alien`'s required components: stand still, hurt no one.
                RigidBody::Static,
                TouchDamage { dps: 0.0 },
                Position::from(post.position),
                Transform::from_translation(post.position).with_scale(Vec3::splat(0.25)),
                bevy::world_serialization::WorldAssetRoot(game_assets.alien_scene.clone()),
                StateMarker,
            ))
            // `Alien` pulls in the AI data components. A static dummy has no use for
            // them, and leaving them on makes `move_towards_goal_system` re-run A* every
            // frame against a map with no goal, logging "Has no goal" forever. Dropping
            // the data is what actually takes the dummy out of those queries.
            .remove::<(MoveTowardsGoalData, AvoidWallsData)>()
            .id();

        // `collision_handling_system` decrements this on every alien death, so a dummy
        // has to be counted on the way in or the counter underflows.
        alien_counter.count += 1;

        add_health_bar_mw.write(AddHealthBar { entity: dummy, name: "DUMMY" });
        post.occupant = Some(dummy);
    }
}
