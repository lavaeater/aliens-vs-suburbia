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
use bevy::world_serialization::WorldAssetRoot;

use crate::assets::asset_definition::{AssetDefinition, Hardpoint, ModelType, WeaponProps};
use crate::assets::hardpoint::{snap_transform, weapon_local_scale};
use crate::player::systems::shoot::Weapon;
use crate::player::systems::spawn_players::PlayerModelRoot;

/// The role both sides use to attach a one-handed weapon to a hand.
pub const GRIP_ROLE: &str = "grip";

/// Weapon-only role: the frame bullets leave from.
pub const MUZZLE_ROLE: &str = "muzzle";

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
    /// The weapon's `muzzle` hardpoint (where bullets leave), if authored.
    pub muzzle: Option<Hardpoint>,
    /// Combat stats resolved from the weapon def.
    pub weapon_props: WeaponProps,
    tries: u32,
}

impl PendingEquip {
    /// Build from a character def and a weapon def path. Returns `None` unless both
    /// sides actually have a `grip` hardpoint and the weapon def is a `Weapon`.
    pub fn resolve(character: &AssetDefinition, weapon_def_path: &str) -> Option<Self> {
        let char_grip = character.hardpoints.get(GRIP_ROLE)?.clone();
        let weapon = AssetDefinition::load_from_def_path(weapon_def_path)?;
        let ModelType::Weapon(props) = &weapon.model_type else {
            warn!("{weapon_def_path} is not a Weapon def; not equipping");
            return None;
        };
        let weapon_props = props.clone();
        let weapon_grip = weapon.hardpoints.get(GRIP_ROLE)?.clone();
        let muzzle = weapon.hardpoints.get(MUZZLE_ROLE).cloned();
        Some(Self {
            bone: char_grip.anchor.clone(),
            char_grip,
            weapon_grip,
            weapon_model_path: weapon.model_path,
            weapon_scale: weapon.scale,
            muzzle,
            weapon_props,
            tries: 0,
        })
    }
}

/// The weapon entity currently held by this character. Also the "already equipped"
/// marker; the entity is for swapping/dropping later.
#[derive(Component)]
pub struct EquippedWeapon(#[allow(dead_code)] pub Entity);

/// Marker on the spawned weapon scene root, carrying everything needed to keep it
/// snapped every frame. We re-derive the scale continuously (rather than once at
/// spawn) because a rig's bone `GlobalTransform` may not be propagated on the frame
/// the skeleton first appears — a single stale read there would collapse the weapon
/// permanently. Cheap: one query lookup per equipped weapon.
#[derive(Component)]
pub struct WeaponModel {
    char_grip: Hardpoint,
    weapon_grip: Hardpoint,
    weapon_def_scale: f32,
    /// The grip bone the weapon is parented to (its world scale folds in the rig's
    /// baked scale, which we cancel out).
    bone: Entity,
    /// The character's model root (its world scale is how big the character renders).
    root: Entity,
}

impl WeaponModel {
    /// Replace the character-side grip frame. The playground's hardpoint editor calls this
    /// so a nudge shows up on the held weapon immediately — `keep_weapons_snapped` rebuilds
    /// the transform from this field every frame, so nothing else has to be touched.
    pub fn set_char_grip(&mut self, grip: Hardpoint) {
        self.char_grip = grip;
    }

    /// Replace the weapon-side grip frame — the counterpart of [`Self::set_char_grip`],
    /// used when the playground edits the *weapon's* def rather than the character's.
    /// Moving this frame slides the gun through the hand; moving the character grip moves
    /// the hand's attachment point. Either can get a gun sitting right, and which one you
    /// want depends on whether the fault is the rig or the model.
    pub fn set_weapon_grip(&mut self, grip: Hardpoint) {
        self.weapon_grip = grip;
    }
}

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

/// This character's own `PlayerModelRoot` entity (the scaled scene child), found by
/// walking descendants so multi-player scenes don't cross wires.
fn find_descendant_root(
    character: Entity,
    children: &Query<&Children>,
    root_q: &Query<(), With<PlayerModelRoot>>,
) -> Option<Entity> {
    let mut queue = vec![character];
    while let Some(entity) = queue.pop() {
        if root_q.get(entity).is_ok() {
            return Some(entity);
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
    root_q: Query<(), With<PlayerModelRoot>>,
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

        // Wait for this character's own PlayerModelRoot (set by fix_scene_transform) so
        // the scale-tracking below has a root to read. The actual scale is derived every
        // frame in keep_weapons_snapped, not here — see WeaponModel.
        let Some(root) = find_descendant_root(character, &children, &root_q) else {
            equip.tries += 1;
            continue;
        };

        let scene = asset_server
            .load(GltfAssetLabel::Scene(0).from_asset(equip.weapon_model_path.clone()));
        // Spawn with an identity-ish transform; keep_weapons_snapped sets the real one
        // next frame from settled GlobalTransforms.
        let weapon = commands
            .spawn((
                WorldAssetRoot(scene),
                Transform::default(),
                WeaponModel {
                    char_grip: equip.char_grip.clone(),
                    weapon_grip: equip.weapon_grip.clone(),
                    weapon_def_scale: equip.weapon_scale,
                    bone: anchor,
                    root,
                },
                Weapon::from_props(&equip.weapon_props, equip.muzzle.clone()),
            ))
            .id();
        commands.entity(anchor).add_child(weapon);
        commands
            .entity(character)
            .insert(EquippedWeapon(weapon))
            .remove::<PendingEquip>();
    }
}

/// Keep each equipped weapon snapped onto its grip bone, re-deriving the scale from
/// the current (settled) bone and character-root world scales every frame. Constant
/// in practice — bone world scale barely changes across animation — but immune to the
/// spawn-frame propagation race that would otherwise collapse the weapon.
pub fn keep_weapons_snapped(
    time: Res<Time>,
    weapons: Query<(Entity, &WeaponModel)>,
    global_transforms: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform>,
    mut recoil_q: Query<&mut Weapon>,
) {
    let dt = time.delta_secs();
    for (weapon, snap) in weapons.iter() {
        let bone_scale = global_transforms.get(snap.bone).map(|gt| gt.scale().x).unwrap_or(1.0);
        let root_scale = global_transforms.get(snap.root).map(|gt| gt.scale().x).unwrap_or(1.0);
        let effective = weapon_local_scale(snap.weapon_def_scale, root_scale, bone_scale);

        if let Ok(mut t) = transforms.get_mut(weapon) {
            *t = snap_transform(&snap.char_grip, &snap.weapon_grip, effective);

            // Recoil: kick the muzzle up around the weapon's local X, decaying fast.
            if let Ok(mut wpn) = recoil_q.get_mut(weapon) {
                if wpn.recoil > 0.0001 {
                    t.rotation *= Quat::from_rotation_x(wpn.recoil);
                    wpn.recoil = (wpn.recoil - wpn.recoil * 14.0 * dt).max(0.0);
                } else {
                    wpn.recoil = 0.0;
                }
            }
        }
    }
}
