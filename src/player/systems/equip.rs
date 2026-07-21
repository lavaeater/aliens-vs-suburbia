//! Runtime weapon equipping driven by hardpoint data.
//!
//! The character def carries a `grip` hardpoint anchored to a hand bone; the weapon
//! def carries its own `grip` hardpoint relative to the weapon origin. Snapping is
//! `weapon_local(char_grip, weapon_grip)` — see `src/assets/hardpoint.rs` and
//! `docs/inverse-kinematics-hardpoints.md`.
//!
//! Authoring happens in the asset browser; this module just replays the same math
//! in-game, so what you see in the browser preview is what you get here.

use bevy::prelude::*;
use bevy::gltf::GltfAssetLabel;
use bevy::scene::SceneRoot;

use crate::assets::asset_definition::{AssetDefinition, Hardpoint, ModelType};
use crate::assets::hardpoint::{snap_transform, weapon_local_scale};
use crate::player::systems::spawn_players::PlayerModelRoot;

/// The role both sides use to attach a one-handed weapon to a hand.
pub const GRIP_ROLE: &str = "grip";

/// How many frames we wait for the skeleton to spawn before giving up (and saying so).
const EQUIP_MAX_TRIES: u32 = 600;

/// Everything needed to place a weapon, resolved from defs at spawn time so the
/// equip system never touches the filesystem.
#[derive(Component, Clone)]
pub struct PendingEquip {
    /// Bone to parent the weapon to. `None` = the character's own entity (root).
    pub bone: Option<String>,
    /// The character's grip frame, local to `bone`.
    pub char_grip: Hardpoint,
    /// The weapon's grip frame, local to the weapon origin.
    pub weapon_grip: Hardpoint,
    /// Weapon GLB path, relative to `assets/`.
    pub weapon_model_path: String,
    /// Weapon def scale, applied as the weapon's local scale under the bone.
    pub weapon_scale: f32,
    tries: u32,
}

impl PendingEquip {
    /// Build from a character def and a weapon def path. Returns `None` unless both
    /// sides actually have a `grip` hardpoint and the weapon def is a `Weapon`.
    pub fn resolve(character: &AssetDefinition, weapon_def_path: &str) -> Option<Self> {
        let char_grip = character.hardpoints.get(GRIP_ROLE)?.clone();
        let weapon = AssetDefinition::load_from_def_path(weapon_def_path)?;
        if !matches!(weapon.model_type, ModelType::Weapon(_)) {
            warn!("{weapon_def_path} is not a Weapon def; not equipping");
            return None;
        }
        let weapon_grip = weapon.hardpoints.get(GRIP_ROLE)?.clone();
        Some(Self {
            bone: char_grip.anchor.clone(),
            char_grip,
            weapon_grip,
            weapon_model_path: weapon.model_path,
            weapon_scale: weapon.scale,
            tries: 0,
        })
    }
}

/// The weapon entity currently held by this character. Also the "already equipped"
/// marker; the entity is for swapping/dropping later.
#[derive(Component)]
pub struct EquippedWeapon(#[allow(dead_code)] pub Entity);

/// Marker on the spawned weapon scene root.
#[derive(Component)]
pub struct WeaponModel;

/// Breadth-first search for a named entity under `root`, so we only ever match bones
/// belonging to *this* character (several players may share a skeleton's bone names).
fn find_descendant_named(
    root: Entity,
    name: &str,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<Entity> {
    let mut queue = vec![root];
    while let Some(entity) = queue.pop() {
        if names.get(entity).is_ok_and(|n| n.as_str() == name) {
            return Some(entity);
        }
        if let Ok(kids) = children.get(entity) {
            queue.extend(kids.iter());
        }
    }
    None
}

/// World scale of this character's own `PlayerModelRoot` (the scaled scene child),
/// found by walking descendants so multi-player scenes don't cross wires.
fn find_descendant_root_scale(
    character: Entity,
    children: &Query<&Children>,
    root_q: &Query<&GlobalTransform, With<PlayerModelRoot>>,
) -> Option<f32> {
    let mut queue = vec![character];
    while let Some(entity) = queue.pop() {
        if let Ok(gt) = root_q.get(entity) {
            return Some(gt.scale().x);
        }
        if let Ok(kids) = children.get(entity) {
            queue.extend(kids.iter());
        }
    }
    None
}

/// Spawns the weapon once the character's skeleton exists, snapped onto its grip.
/// Retries until the bone shows up, since the scene loads asynchronously.
pub fn equip_pending_weapons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut pending: Query<(Entity, &mut PendingEquip), Without<EquippedWeapon>>,
    children: Query<&Children>,
    names: Query<&Name>,
    global_transforms: Query<&GlobalTransform>,
    root_q: Query<&GlobalTransform, With<PlayerModelRoot>>,
) {
    for (character, mut equip) in pending.iter_mut() {
        let anchor = match equip.bone.clone() {
            None => character,
            Some(bone) => match find_descendant_named(character, &bone, &children, &names) {
                Some(e) => e,
                None => {
                    equip.tries += 1;
                    if equip.tries >= EQUIP_MAX_TRIES {
                        warn!(
                            "bone '{bone}' never appeared under the character; \
                             not equipping {}",
                            equip.weapon_model_path
                        );
                        commands.entity(character).remove::<PendingEquip>();
                    }
                    continue;
                }
            },
        };

        // Cancel the rig's baked bone scale and track the character's rendered scale,
        // so a tiny bone world scale (mesh2motion bakes ~0.0136) doesn't collapse the
        // weapon. Wait until this character's own PlayerModelRoot exists — that only
        // happens after fix_scene_transform has run and transforms have propagated, so
        // both scales below are settled rather than first-frame identity garbage.
        let Some(root_scale) = find_descendant_root_scale(character, &children, &root_q) else {
            equip.tries += 1;
            continue; // skeleton is up but the scaled root isn't settled yet — retry
        };
        let bone_scale = global_transforms.get(anchor).map(|gt| gt.scale().x).unwrap_or(1.0);
        let effective = weapon_local_scale(equip.weapon_scale, root_scale, bone_scale);
        let local = snap_transform(&equip.char_grip, &equip.weapon_grip, effective);

        let scene = asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(equip.weapon_model_path.clone()));
        let weapon = commands
            .spawn((
                SceneRoot(scene),
                local,
                WeaponModel,
            ))
            .id();
        commands.entity(anchor).add_child(weapon);
        commands
            .entity(character)
            .insert(EquippedWeapon(weapon))
            .remove::<PendingEquip>();
    }
}
