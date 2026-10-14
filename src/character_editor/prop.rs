//! A model opened on its own — an enemy, a tower, a barrel — standing in the arena.
//!
//! The character stays in the world but is hidden and held still (Studio is forced on for
//! a prop, see `doc::Docs::open`), and the prop is spawned where its feet were. Keeping
//! the character means coming back to it is instant and keeps its position, and it is the
//! same entity every system already knows how to handle.
//!
//! The prop is a bare scene: no physics, no AI, no animation graph. It is there to be
//! looked at, sized and given hardpoints. Respawned whenever its model or scale changes.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

use crate::character_editor::debug::EditorOverlays;
use crate::character_editor::doc::{EditorFocus, PropDoc, Subject};
use crate::player::components::Player;

/// The spawned prop. Carries what it was spawned from, so a change can be spotted.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct EditorProp {
    pub model_path: String,
    pub scale: f32,
}

/// Half the player's collider height: from its body centre down to its feet.
const PLAYER_HALF_HEIGHT: f32 = 0.5;

/// What should be standing in the arena: the prop doc's model, while a prop is the subject.
pub fn wanted_prop(focus: &EditorFocus, doc: &PropDoc) -> Option<EditorProp> {
    if focus.subject != Subject::Prop {
        return None;
    }
    let def = doc.def.as_ref()?;
    Some(EditorProp {
        model_path: def.model_path.clone(),
        scale: def.scale,
    })
}

/// Spawn, replace or remove the prop so it matches the doc.
pub fn sync_prop(
    focus: Res<EditorFocus>,
    doc: Res<PropDoc>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    props: Query<(Entity, &EditorProp)>,
    players: Query<&GlobalTransform, With<Player>>,
) {
    let wanted = wanted_prop(&focus, &doc);
    let current = props.iter().next();
    if current.map(|(_, p)| p) == wanted.as_ref() {
        return;
    }
    if let Some((entity, _)) = current {
        commands.entity(entity).despawn();
    }
    let Some(prop) = wanted else {
        return;
    };

    let feet = players.iter().next().map_or(Vec3::ZERO, |player| {
        player.translation() - Vec3::Y * PLAYER_HALF_HEIGHT
    });
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(prop.model_path.clone()));
    commands.spawn((
        WorldAssetRoot(scene),
        Transform::from_translation(feet).with_scale(Vec3::splat(prop.scale)),
        prop,
    ));
}

/// Hide the character while a prop stands in for it, and show it again after.
pub fn hide_player_for_prop(
    focus: Res<EditorFocus>,
    mut players: Query<&mut Visibility, With<Player>>,
) {
    let wanted = if focus.subject == Subject::Prop {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut players {
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// A prop has no Live, so keep Studio on while one is the subject, whichever way it was
/// switched off (the F11 key does not know about subjects).
pub fn keep_studio_for_props(focus: Res<EditorFocus>, mut overlays: ResMut<EditorOverlays>) {
    if !focus.subject.has_live() && !overlays.orbit_camera {
        overlays.toggle_orbit_camera();
    }
}

/// Remove the prop when the session ends; it is not a `StateMarker` entity.
pub fn despawn_prop(mut commands: Commands, props: Query<Entity, With<EditorProp>>) {
    for entity in &props {
        commands.entity(entity).despawn();
    }
}

/// World-space centre and radius of everything drawn under `root`, from the meshes'
/// bounding boxes. Works for a prop with no skeleton (a barrel, a tower), which the joint
/// based bounds the character uses cannot.
pub fn mesh_bounds(
    root: Entity,
    children: &Query<&Children>,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, f32)> {
    let (min, max) = mesh_extents(root, children, aabbs)?;
    Some(((min + max) * 0.5, ((max - min).length() * 0.5).max(0.01)))
}

/// World-space box round everything drawn under `root`: (min corner, max corner).
pub fn mesh_extents(
    root: Entity,
    children: &Query<&Children>,
    aabbs: &Query<(&Aabb, &GlobalTransform)>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Ok((aabb, transform)) = aabbs.get(entity) {
            let matrix = transform.to_matrix();
            let center = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let point = matrix.transform_point3(center + half * Vec3::new(x, y, z));
                        min = min.min(point);
                        max = max.max(point);
                    }
                }
            }
            found = true;
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    found.then_some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::{AssetDefinition, EnemyProps, ModelType};
    use crate::character_editor::doc::DefDoc;

    fn enemy_doc() -> PropDoc {
        let def = AssetDefinition {
            model_path: "packs/x/Alien.glb".to_string(),
            scale: 0.5,
            model_type: ModelType::Enemy(EnemyProps::default()),
            ..Default::default()
        };
        PropDoc(DefDoc {
            def_path: Some("assets/defs/Alien.ron".to_string()),
            saved: Some(def.clone()),
            def: Some(def),
        })
    }

    #[test]
    fn a_prop_stands_in_the_arena_only_while_it_is_the_subject() {
        let doc = enemy_doc();
        let mut focus = EditorFocus {
            subject: Subject::Prop,
            ..Default::default()
        };
        assert_eq!(
            wanted_prop(&focus, &doc),
            Some(EditorProp {
                model_path: "packs/x/Alien.glb".to_string(),
                scale: 0.5,
            })
        );
        focus.subject = Subject::Character;
        assert_eq!(wanted_prop(&focus, &doc), None);
    }

    /// The prop carries its scale, so a scale edit reads as a different prop and
    /// respawns it at the new size.
    #[test]
    fn a_scale_edit_asks_for_a_new_prop() {
        let mut doc = enemy_doc();
        let focus = EditorFocus {
            subject: Subject::Prop,
            ..Default::default()
        };
        let before = wanted_prop(&focus, &doc);
        doc.def.as_mut().unwrap().scale = 1.0;
        assert_ne!(wanted_prop(&focus, &doc), before);
    }
}
