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

use crate::assets::asset_definition::{
    AssetDefinition, Hardpoint, ModelType, WeaponHands, WeaponProps,
};
use crate::assets::hardpoint::{self, snap_transform, weapon_local_scale};
use crate::player::systems::shoot::Weapon;
use crate::player::systems::spawn_players::PlayerModelRoot;
use crate::player::systems::arm_ik::{self, SightAlign, WeaponArm, WeaponArms};
use crate::player::systems::weapon_aim::{self, AimedWeapon};

/// The role both sides use to attach a one-handed weapon to a hand.
pub const GRIP_ROLE: &str = "grip";

/// Weapon-only role: the frame bullets leave from.
pub const MUZZLE_ROLE: &str = "muzzle";

/// The aim-down-sights frame: on the weapon its rear sight, on the character the eye or
/// head bone that should line up behind it.
pub const SIGHT_ROLE: &str = "sight";

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
    /// Set when this pairing is flown from the aim instead of parented to a hand.
    pub aimed: Option<AimedPlan>,
    tries: u32,
}

/// Roles whose hardpoints an arm is solved onto, and which hand each belongs to.
///
/// Both are optional: a weapon with only a `grip` gets one solved arm, and a character
/// missing a `foregrip` simply keeps its animated support arm.
pub const ARM_ROLES: [&str; 2] = [GRIP_ROLE, "foregrip"];

/// Everything the aim-driven path needs, resolved from the two defs at spawn time.
///
/// Present only for weapons that can actually be flown from the aim: two-handed, with a
/// muzzle to point, and a hardpoint role both sides carry to pin them together. Anything
/// else keeps the hand-parented snap, which is right for a pistol anyway.
#[derive(Clone)]
pub struct AimedPlan {
    /// Role pinning weapon to character — `stock` for a shouldered rifle, else `grip`.
    pub role: String,
    /// The character's frame for that role (its `anchor` names the bone).
    pub char_anchor: Hardpoint,
    /// The weapon-local point pinned to it.
    pub weapon_anchor: Vec3,
    /// Weapon-local anchor-to-muzzle direction.
    pub weapon_axis: Vec3,
    /// Character hardpoint and matching weapon hardpoint per arm role, for the IK. Both
    /// full frames now, not just points: the rotations are what orient the hands.
    pub arm_targets: Vec<(Hardpoint, Hardpoint)>,
    /// Character and weapon `sight` frames, when both sides carry one.
    pub sight: Option<(Hardpoint, Hardpoint)>,
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
        let aimed = plan_aimed(character, &weapon, &weapon_props, muzzle.as_ref());
        Some(Self {
            bone: char_grip.anchor.clone(),
            char_grip,
            weapon_grip,
            weapon_model_path: weapon.model_path,
            weapon_scale: weapon.scale,
            muzzle,
            weapon_props,
            aimed,
            tries: 0,
        })
    }
}

/// Decide whether this pairing can be flown from the aim, and with what.
fn plan_aimed(
    character: &AssetDefinition,
    weapon: &AssetDefinition,
    props: &WeaponProps,
    muzzle: Option<&Hardpoint>,
) -> Option<AimedPlan> {
    if props.hands != WeaponHands::TwoHanded {
        return None;
    }
    // No muzzle, no axis to point: there is nothing to aim along.
    let muzzle = muzzle?;
    let role = weapon_aim::anchor_role(
        |role| character.hardpoints.contains_key(role),
        |role| weapon.hardpoints.contains_key(role),
        &weapon_aim::ANCHOR_ROLES,
    )?;

    let char_anchor = character.hardpoints.get(role)?.clone();
    let weapon_anchor = weapon_aim::hardpoint_point(weapon.hardpoints.get(role)?);

    // Only roles both sides carry can be solved for: a hand has nowhere to go without a
    // point on the weapon, and a point on the weapon has no hand without an anchor bone.
    let arm_targets = ARM_ROLES
        .iter()
        .filter_map(|role| {
            let char_point = character.hardpoints.get(*role)?;
            char_point.anchor.as_ref()?;
            let weapon_point = weapon.hardpoints.get(*role)?;
            Some((char_point.clone(), weapon_point.clone()))
        })
        .collect();

    Some(AimedPlan {
        role: role.to_string(),
        char_anchor,
        weapon_anchor,
        weapon_axis: weapon_aim::weapon_axis(
            weapon_anchor,
            weapon_aim::hardpoint_point(muzzle),
        ),
        arm_targets,
        sight: character
            .hardpoints
            .get(SIGHT_ROLE)
            .filter(|frame| frame.anchor.is_some())
            .cloned()
            .zip(weapon.hardpoints.get(SIGHT_ROLE).cloned()),
    })
}

/// Turn the plan's authored roles into live bone entities for the arm solver.
///
/// Anything that does not resolve — a bone the rig does not have, a hardpoint anchored
/// outside an arm — is dropped rather than guessed at, leaving that arm on its animation.
fn resolve_arms(
    character: Entity,
    weapon: Entity,
    plan: &AimedPlan,
    children: &Query<&Children>,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
) -> WeaponArms {
    let arms = plan
        .arm_targets
        .iter()
        .filter_map(|(char_point, weapon_point)| {
            let bone_name = char_point.anchor.as_ref()?;
            let effector = find_descendant_named(character, bone_name, children, names)?;

            // The chain is found by name from the effector upward, because how many bones
            // sit between a hardpoint and the arm depends on where it was anchored.
            let ancestors = ancestor_names(effector, parents, names);
            let names_ref: Vec<&str> = ancestors.iter().map(String::as_str).collect();
            let (upper_index, lower_index) = arm_ik::arm_chain(&names_ref)?;
            let chain = ancestor_entities(effector, parents);
            // The wrist is the bone just above the forearm; when the hardpoint is anchored
            // to the hand itself that is the effector.
            let hand = if lower_index == 0 { effector } else { *chain.get(lower_index - 1)? };

            Some(WeaponArm {
                upper: *chain.get(upper_index)?,
                lower: *chain.get(lower_index)?,
                hand,
                effector,
                effector_frame: hardpoint::transform_from_frame(hardpoint::hardpoint_frame(char_point)),
                weapon_frame: hardpoint::transform_from_frame(hardpoint::hardpoint_frame(weapon_point)),
                pole: arm_ik::DEFAULT_POLE,
            })
        })
        .collect();
    WeaponArms { weapon, arms }
}

/// Wire up the head-to-sights alignment, when both defs carry a `sight` frame.
fn resolve_sight(
    character: Entity,
    weapon: Entity,
    plan: &AimedPlan,
    children: &Query<&Children>,
    names: &Query<&Name>,
) -> Option<SightAlign> {
    let (char_sight, weapon_sight) = plan.sight.as_ref()?;
    let bone_name = char_sight.anchor.as_ref()?;
    let bone = find_descendant_named(character, bone_name, children, names)?;
    Some(SightAlign {
        weapon,
        bone,
        bone_frame: hardpoint::transform_from_frame(hardpoint::hardpoint_frame(char_sight)),
        weapon_frame: hardpoint::transform_from_frame(hardpoint::hardpoint_frame(weapon_sight)),
    })
}

/// The entity's ancestors, nearest first. Bounded because a cycle in the hierarchy would
/// otherwise hang the frame.
fn ancestor_entities(entity: Entity, parents: &Query<&ChildOf>) -> Vec<Entity> {
    let mut chain = Vec::new();
    let mut current = entity;
    while let Ok(parent) = parents.get(current) {
        chain.push(parent.parent());
        current = parent.parent();
        if chain.len() >= 32 {
            break;
        }
    }
    chain
}

fn ancestor_names(entity: Entity, parents: &Query<&ChildOf>, names: &Query<&Name>) -> Vec<String> {
    ancestor_entities(entity, parents)
        .into_iter()
        .map(|e| names.get(e).map(|n| n.to_string()).unwrap_or_default())
        .collect()
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
    parents: Query<&ChildOf>,
) {
    for (character, mut equip) in pending.iter_mut() {
        // The aim-driven path wants a different bone (the anchor role's, which may be a
        // shoulder rather than a hand) and a different parent (the character root, so the
        // arm IK cannot move the very thing it is reaching for).
        let aim_bone = match equip.aimed.as_ref().map(|plan| plan.char_anchor.anchor.clone()) {
            None => None,
            Some(None) => Some(character),
            Some(Some(bone)) => match find_descendant_named(character, &bone, &children, &names) {
                Some(entity) => Some(entity),
                None => {
                    equip.tries += 1;
                    if equip.tries >= EQUIP_MAX_TRIES {
                        warn!(
                            "anchor bone '{bone}' never appeared under the character; \
                             not equipping {}",
                            equip.weapon_model_path
                        );
                        commands.entity(character).remove::<PendingEquip>();
                    }
                    continue;
                }
            },
        };

        if let (Some(plan), Some(anchor_bone)) = (equip.aimed.clone(), aim_bone) {
            let Some(root) = find_descendant_root(character, &children, &root_q) else {
                equip.tries += 1;
                continue;
            };
            let scene = asset_server
                .load(GltfAssetLabel::Scene(0).from_asset(equip.weapon_model_path.clone()));
            let weapon = commands
                .spawn((
                    WorldAssetRoot(scene),
                    Transform::default(),
                    AimedWeapon {
                        owner: character,
                        anchor_bone,
                        anchor_offset: Vec3::from(plan.char_anchor.translation),
                        weapon_anchor: plan.weapon_anchor,
                        weapon_axis: plan.weapon_axis,
                        model_root: root,
                        def_scale: equip.weapon_scale,
                    },
                    Weapon::from_props(&equip.weapon_props, equip.muzzle.clone()),
                ))
                .id();
            commands.entity(character).add_child(weapon);
            let arms = resolve_arms(character, weapon, &plan, &children, &names, &parents);
            let solved = arms.arms.len();
            let sight = resolve_sight(character, weapon, &plan, &children, &names);
            commands
                .entity(character)
                .insert(EquippedWeapon(weapon))
                .insert(arms)
                .remove::<PendingEquip>();
            if let Some(sight) = sight {
                commands.entity(character).insert(sight);
            }
            // Which role won says a lot about how it will look: `stock` is shouldered,
            // `grip` pivots about the trigger hand because the weapon has no stock frame.
            info!(
                "aim-driven weapon {} anchored at '{}', {solved} arm(s) solved",
                equip.weapon_model_path, plan.role
            );
            continue;
        }

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
