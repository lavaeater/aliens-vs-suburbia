//! Skeleton and hardpoint overlays, shared by the asset browser and the playground.
//!
//! The browser shows one model in an empty scene, so it can collect every skinned mesh in
//! the world and be right. The playground cannot: the player, the target dummies and any
//! aliens are all skinned, so it has to scope the overlay to one character's subtree. That
//! difference is the reason these take their joints as an argument instead of querying for
//! them — the *drawing* is identical, the *selection* is not.

use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::assets::asset_definition::Hardpoint;
use crate::assets::hardpoint::frame_from_euler;

/// Every joint of every skinned mesh in the world.
///
/// Only correct where exactly one character exists — i.e. the asset browser. Use
/// [`joints_under`] anywhere a scene can hold more than one skinned model.
pub fn all_joints(skinned_q: &Query<&SkinnedMesh>) -> HashSet<Entity> {
    let mut joints = HashSet::new();
    for skinned in skinned_q.iter() {
        joints.extend(skinned.joints.iter().copied());
    }
    joints
}

/// Joints belonging to the skinned meshes beneath `root`, and nothing else.
///
/// Walks down rather than up: a joint's `ChildOf` chain leads through the armature, but the
/// `SkinnedMesh` component sits on the mesh entity, which is a sibling of the armature
/// rather than an ancestor of the joints.
pub fn joints_under(
    root: Entity,
    children_q: &Query<&Children>,
    skinned_q: &Query<&SkinnedMesh>,
) -> HashSet<Entity> {
    let mut joints = HashSet::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok(skinned) = skinned_q.get(entity) {
            joints.extend(skinned.joints.iter().copied());
        }
        if let Ok(children) = children_q.get(entity) {
            stack.extend(children.iter());
        }
    }
    joints
}

/// Map bone name -> entity for a set of joints. First one wins, matching the browser's
/// behaviour for rigs that reuse a name.
pub fn bone_map(joints: &HashSet<Entity>, names: &Query<&Name>) -> HashMap<String, Entity> {
    let mut map = HashMap::new();
    for &joint in joints {
        if let Ok(name) = names.get(joint) {
            map.entry(name.as_str().to_string()).or_insert(joint);
        }
    }
    map
}

/// Draw the skeleton: a small cross at each joint and a segment to its parent joint.
/// `highlight` names a bone to draw larger and in a different colour (the browser's
/// attachment socket, the playground's selected hardpoint anchor).
pub fn draw_skeleton(
    gizmos: &mut Gizmos,
    joints: &HashSet<Entity>,
    highlight: Option<&str>,
    transforms: &Query<&GlobalTransform>,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
) {
    let bone_color = Color::srgb(0.2, 1.0, 0.5);
    let joint_color = Color::srgb(1.0, 0.85, 0.2);
    let socket_color = Color::srgb(1.0, 0.3, 0.9);

    for &joint in joints {
        let Ok(joint_transform) = transforms.get(joint) else { continue };
        let position = joint_transform.translation();

        let is_socket = highlight.is_some()
            && names.get(joint).map(|n| Some(n.as_str()) == highlight).unwrap_or(false);
        let (mark_color, size) = if is_socket { (socket_color, 0.03) } else { (joint_color, 0.012) };

        gizmos.line(position - Vec3::X * size, position + Vec3::X * size, mark_color);
        gizmos.line(position - Vec3::Y * size, position + Vec3::Y * size, mark_color);
        gizmos.line(position - Vec3::Z * size, position + Vec3::Z * size, mark_color);

        // Skip the skeleton root, whose parent is a non-joint scene node.
        if let Ok(child_of) = parents.get(joint)
            && joints.contains(&child_of.parent())
            && let Ok(parent_transform) = transforms.get(child_of.parent())
        {
            gizmos.line(parent_transform.translation(), position, bone_color);
        }
    }
}

/// Draw each hardpoint as an RGB axis cross at its world frame, so you can see where it
/// sits and which way it points.
///
/// `resolve_anchor` turns a hardpoint's anchor (a bone name, or `None` for the model
/// origin) into the entity its frame is relative to. Callers differ in what "the model
/// origin" means — the browser's viewer entity, the playground's player root — so they
/// supply it.
pub fn draw_hardpoints(
    gizmos: &mut Gizmos,
    hardpoints: &HashMap<String, Hardpoint>,
    active: Option<&str>,
    transforms: &Query<&GlobalTransform>,
    resolve_anchor: impl Fn(&Option<String>) -> Option<Entity>,
) {
    for (role, hardpoint) in hardpoints {
        let Some(anchor) = resolve_anchor(&hardpoint.anchor) else { continue };
        let Ok(anchor_transform) = transforms.get(anchor) else { continue };

        let frame = frame_from_euler(hardpoint.translation, hardpoint.rotation_euler_deg);
        let position = anchor_transform.transform_point(Vec3::from(frame.translation));
        let rotation = anchor_transform.rotation() * frame.rotation;
        let size = if Some(role.as_str()) == active { 0.09 } else { 0.055 };

        gizmos.line(position, position + rotation * Vec3::X * size, Color::srgb(1.0, 0.25, 0.25));
        gizmos.line(position, position + rotation * Vec3::Y * size, Color::srgb(0.25, 1.0, 0.25));
        gizmos.line(position, position + rotation * Vec3::Z * size, Color::srgb(0.35, 0.55, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build `root -> mesh(SkinnedMesh over `joints`)`, returning the root.
    fn character(world: &mut World, joint_count: usize) -> (Entity, Vec<Entity>) {
        let joints: Vec<Entity> = (0..joint_count).map(|_| world.spawn_empty().id()).collect();
        let mesh = world
            .spawn(SkinnedMesh { inverse_bindposes: Default::default(), joints: joints.clone() })
            .id();
        let root = world.spawn_empty().add_child(mesh).id();
        (root, joints)
    }

    /// The whole reason this takes a root: the playground has several skinned characters
    /// in the scene at once, and an overlay drawn over all of them is unreadable.
    #[test]
    fn only_the_asked_for_characters_joints_come_back() {
        let mut world = World::new();
        let (player, player_joints) = character(&mut world, 3);
        let (_dummy, dummy_joints) = character(&mut world, 4);

        let mut children_q = world.query::<&Children>();
        let mut skinned_q = world.query::<&SkinnedMesh>();
        let children_q = children_q.query(&world);
        let skinned_q = skinned_q.query(&world);

        let found = joints_under(player, &children_q, &skinned_q);
        assert_eq!(found.len(), 3);
        assert!(player_joints.iter().all(|j| found.contains(j)));
        assert!(dummy_joints.iter().all(|j| !found.contains(j)), "no other character's bones");
    }

    /// A character whose scene has not spawned yet has no skinned mesh below it; the
    /// overlay has to treat that as "nothing to draw", not panic or draw the world.
    #[test]
    fn a_character_with_no_skinned_mesh_yields_nothing() {
        let mut world = World::new();
        let bare = world.spawn_empty().id();
        let (_other, _) = character(&mut world, 3);

        let mut children_q = world.query::<&Children>();
        let mut skinned_q = world.query::<&SkinnedMesh>();
        let children_q = children_q.query(&world);
        let skinned_q = skinned_q.query(&world);

        assert!(joints_under(bare, &children_q, &skinned_q).is_empty());
    }

    #[test]
    fn all_joints_collects_every_character_in_the_world() {
        let mut world = World::new();
        character(&mut world, 3);
        character(&mut world, 4);

        let mut skinned_q = world.query::<&SkinnedMesh>();
        let skinned_q = skinned_q.query(&world);

        assert_eq!(all_joints(&skinned_q).len(), 7);
    }

    #[test]
    fn the_bone_map_prefers_the_first_entity_for_a_repeated_name() {
        let mut world = World::new();
        let first = world.spawn(Name::new("Spine")).id();
        let second = world.spawn(Name::new("Spine")).id();
        let joints: HashSet<Entity> = [first, second].into_iter().collect();

        let mut names_q = world.query::<&Name>();
        let names_q = names_q.query(&world);

        let map = bone_map(&joints, &names_q);
        assert_eq!(map.len(), 1, "one entry per name");
        assert!(map["Spine"] == first || map["Spine"] == second);
    }
}
