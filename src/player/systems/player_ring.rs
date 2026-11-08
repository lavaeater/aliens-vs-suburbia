//! A translucent compass ring on the floor under every player, in the player's colour, so
//! it is plain who is who. Its arrow points the way the player is walking and stays on the
//! last heading while they stand still.
//!
//! The colours follow a PlayStation pad's default light bar order by slot (blue, red,
//! green, pink). A pad's light bar can't be read back through Bevy/gilrs, so the slot is
//! the source of truth.
//!
//! Like the flashlight, the ring is its own entity rather than a child of the player (the
//! body turns, the ring keeps to the walk direction), placed after transform propagation
//! from where the player really is this frame.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::control::components::CharacterControl;
use crate::general::systems::map_systems::TileDefinitions;
use crate::player::components::{Player, PlayerDead, PlayerSlot};

const INNER: f32 = 0.42;
const OUTER: f32 = 0.5;
/// The arrow: from just outside the ring out to its tip, this wide at the base.
const ARROW_BASE: f32 = 0.53;
const ARROW_TIP: f32 = 0.78;
const ARROW_HALF_WIDTH: f32 = 0.15;
/// The three short ticks pointing in from the ring, so it reads as a compass.
const TICK_LENGTH: f32 = 0.08;
const TICK_HALF_WIDTH: f32 = 0.025;
const SEGMENTS: usize = 48;
/// Over the floor, and over the blood decals (`gore::blood`, about 0.01).
const LIFT: f32 = 0.025;
const ALPHA: f32 = 0.55;
/// The ring of a downed player fades further.
const DOWNED_ALPHA: f32 = 0.2;

/// Blue, red, green, pink, then round again.
pub fn slot_color(slot: usize) -> Color {
    const COLORS: [(f32, f32, f32); 4] = [
        (0.25, 0.5, 1.0),
        (1.0, 0.25, 0.2),
        (0.3, 0.95, 0.35),
        (1.0, 0.45, 0.85),
    ];
    let (r, g, b) = COLORS.iter().cycle().nth(slot).copied().unwrap_or((1.0, 1.0, 1.0));
    Color::srgb(r, g, b)
}

#[derive(Component)]
pub struct PlayerRing {
    pub owner: Entity,
}

/// On a player who has been given a ring.
#[derive(Component)]
pub struct HasRing;

/// The ring with its arrow at -Z (forward) and the ticks at the other three quarters, flat
/// on the XZ plane facing up.
pub fn ring_geometry() -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut points: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut quad = |points: &mut Vec<[f32; 3]>, corners: [Vec2; 4]| {
        let base = points.len() as u32;
        points.extend(corners.map(|c| [c.x, 0.0, c.y]));
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    let at = |angle: f32, radius: f32| Vec2::new(angle.sin(), -angle.cos()) * radius;

    for i in 0..SEGMENTS {
        let a0 = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        quad(&mut points, [at(a0, INNER), at(a0, OUTER), at(a1, OUTER), at(a1, INNER)]);
    }
    for quarter in 1..4 {
        let angle = quarter as f32 * std::f32::consts::FRAC_PI_2;
        let along = at(angle, 1.0);
        let across = Vec2::new(-along.y, along.x) * TICK_HALF_WIDTH;
        let (from, to) = (along * INNER, along * (INNER - TICK_LENGTH));
        quad(&mut points, [from - across, from + across, to + across, to - across]);
    }
    let base = points.len() as u32;
    points.extend([
        [-ARROW_HALF_WIDTH, 0.0, -ARROW_BASE],
        [ARROW_HALF_WIDTH, 0.0, -ARROW_BASE],
        [0.0, 0.0, -ARROW_TIP],
    ]);
    indices.extend([base, base + 1, base + 2]);
    (points, indices)
}

fn ring_mesh() -> Mesh {
    let (points, indices) = ring_geometry();
    let normals = vec![[0.0, 1.0, 0.0]; points.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

#[allow(clippy::type_complexity)]
pub fn give_rings(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    players: Query<(Entity, &PlayerSlot), (With<Player>, Without<HasRing>)>,
) {
    for (owner, slot) in &players {
        let mesh = mesh.get_or_insert_with(|| meshes.add(ring_mesh())).clone();
        commands.spawn((
            Name::from("Player ring"),
            PlayerRing { owner },
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: slot_color(slot.0).with_alpha(ALPHA),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                depth_bias: 2.0,
                ..default()
            })),
            Transform::default(),
        ));
        // `try_`: the editor swaps the player model by despawning it, possibly this frame.
        commands.entity(owner).try_insert(HasRing);
    }
}

#[allow(clippy::type_complexity)]
pub fn update_rings(
    mut commands: Commands,
    tiles: Option<Res<TileDefinitions>>,
    players: Query<(&GlobalTransform, &CharacterControl, Has<PlayerDead>), With<Player>>,
    mut rings: Query<
        (
            Entity,
            &PlayerRing,
            &MeshMaterial3d<StandardMaterial>,
            &mut Transform,
            &mut GlobalTransform,
        ),
        Without<Player>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor = tiles.map_or(0.0, |t| t.floor_level);
    for (entity, ring, material, mut transform, mut global) in &mut rings {
        let Ok((body, control, dead)) = players.get(ring.owner) else {
            // The player is gone (bled out, or a model swap); a new one gets a new ring.
            commands.entity(entity).despawn();
            continue;
        };
        let at = body.translation();
        transform.translation = Vec3::new(at.x, floor + LIFT, at.z);
        let walk = Vec3::new(control.walk_direction.x, 0.0, control.walk_direction.z);
        if walk.length_squared() > 1e-4 {
            transform.look_to(walk, Vec3::Y);
        }
        *global = GlobalTransform::from(*transform);

        let alpha = if dead { DOWNED_ALPHA } else { ALPHA };
        if let Some(mut material) = materials.get_mut(&material.0)
            && (material.base_color.alpha() - alpha).abs() > 1e-3
        {
            material.base_color.set_alpha(alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrow_points_forward_and_everything_lies_flat_inside_its_tip() {
        let (points, indices) = ring_geometry();
        assert!(points.iter().all(|p| p[1] == 0.0));
        assert!(indices.iter().all(|&i| (i as usize) < points.len()));
        assert_eq!(indices.len() % 3, 0);
        let tip = points
            .iter()
            .max_by(|a, b| Vec2::new(a[0], a[2]).length().total_cmp(&Vec2::new(b[0], b[2]).length()))
            .unwrap();
        assert_eq!(*tip, [0.0, 0.0, -ARROW_TIP], "the farthest point is the arrow's tip, at -Z");
    }

    #[test]
    fn slots_get_distinct_colours_that_wrap() {
        assert_ne!(slot_color(0), slot_color(1));
        assert_ne!(slot_color(1), slot_color(2));
        assert_ne!(slot_color(2), slot_color(3));
        assert_eq!(slot_color(4), slot_color(0));
    }
}
