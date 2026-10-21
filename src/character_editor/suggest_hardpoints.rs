//! First guesses for every hardpoint, read off the model files: a character's from its
//! bind pose, a weapon's from the shape of its mesh. Good enough to start nudging from,
//! not a substitute for looking.
//!
//! ## Character frames
//!
//! The rule Mesh2Motion uses to put a prop in a hand (`PropsManager.ts`): in the rest
//! pose the character faces model +Z and the palms face down, so the thumbs point forward.
//! A hand holds a gun with the barrel carrying on the line of the hand and the top of the
//! gun on the thumb side; in the rest pose that is straight out to the side, top forward. The
//! grip and foregrip frames say that in the hand bone's own frame, and sit 9 cm along the
//! hand and 3.5 cm below the palm (Mesh2Motion's numbers, scaled to the rig's height). The
//! stock (the shoulder pocket) and the sight (the right eye) are oriented the way the gun
//! points when it is aimed: barrel forward, top up. On `male-simple` the grip lands within
//! a few degrees and a centimetre of where Mesh2Motion exported its pistol, and of the grip
//! authored by hand before it.
//!
//! ## Weapon frames
//!
//! Weapon frames are relative to the model origin and assume the convention every gun in
//! the packs (and Mesh2Motion's props) is modelled in: barrel along +X, up +Y. They are
//! left unrotated, which is what that convention means; a weapon modelled another way
//! gets its points in the wrong places, and the nudge rows fix it. The points come from
//! the mesh's profile along the barrel: the muzzle at the front on the barrel's line, the
//! grip in the first part hanging below the barrel (behind it, for a two-handed gun, the
//! stock is not the grip), the foregrip under the barrel ahead of the grip, the stock at
//! the back, the sight on top above the grip.

use std::collections::HashMap;

use bevy::math::{EulerRot, Mat3, Mat4, Quat, Vec3};

use crate::assets::asset_definition::{AssetDefinition, Hardpoint, ModelType, WeaponHands};
use crate::assets::gltf_info;
use crate::character_editor::hardpoints::HardpointSide;

/// What a suggestion is worked out from, read off the open def's model file once per
/// click: a skeleton for a character (or a model opened on its own, when it has one), a
/// mesh for a weapon.
pub enum ModelShape {
    Rig(HashMap<String, Mat4>),
    Gun(Vec<Vec3>, WeaponHands),
}

impl ModelShape {
    /// `None` when the file cannot be read, or there is nothing to reason about (a model
    /// opened on its own with no skeleton).
    pub fn read(def: &AssetDefinition, side: HardpointSide) -> Option<Self> {
        match side {
            HardpointSide::Weapon => {
                let hands = match &def.model_type {
                    ModelType::Weapon(props) => props.hands,
                    _ => WeaponHands::default(),
                };
                let points = gltf_info::mesh_points(&def.model_path)?;
                (!points.is_empty()).then_some(Self::Gun(points, hands))
            }
            HardpointSide::Character | HardpointSide::Prop => {
                let bones = gltf_info::bind_pose_world(&def.model_path)?;
                (!bones.is_empty()).then_some(Self::Rig(bones))
            }
        }
    }

    pub fn frame(&self, role: &str) -> Option<Hardpoint> {
        match self {
            Self::Rig(bones) => character_frame(bones, role),
            Self::Gun(points, hands) => weapon_frame(points, *hands, role),
        }
    }
}

/// Height the rest-pose offsets are measured for, metres. A rig drawn in other units, or
/// a child, gets them scaled by its own height over this.
const REFERENCE_HEIGHT: f32 = 1.75;

/// Model-space directions of a character in its rest pose.
const FORWARD: Vec3 = Vec3::Z;
const UP: Vec3 = Vec3::Y;

/// A character's suggestion for `role`, from its bind pose (`gltf_info::bind_pose_world`),
/// or `None` when the rig has no bone to put it on.
pub fn character_frame(bones: &HashMap<String, Mat4>, role: &str) -> Option<Hardpoint> {
    let scale = rig_height(bones) / REFERENCE_HEIGHT;
    let names: Vec<&str> = bones.keys().map(String::as_str).collect();
    let (anchor, offset) = match role {
        "grip" | "foregrip" => {
            let side = if role == "grip" {
                Side::Right
            } else {
                Side::Left
            };
            let (hand, palm) = palm(bones, &names, side)?;
            let offset = palm - UP * 0.035 * scale - bones[hand].w_axis.truncate();
            // Barrel out to the side the hand is on (the right of a character facing +Z is
            // -X), top toward the thumb: forward, with the palm down. Straight out rather
            // than along the bone, as Mesh2Motion does, so an A-pose rig's drooping hand
            // does not tilt the gun.
            let outward = match side {
                Side::Right => -Vec3::X,
                Side::Left => Vec3::X,
            };
            return Some(frame_on_bone(hand, &bones[hand], offset, outward, FORWARD));
        }
        "stock" => {
            let shoulder = find_bone(&names, Limb::UpperArm, Some(Side::Right))?;
            let at = bones[shoulder].w_axis.truncate();
            // Into the chest from the shoulder joint, a little down and in front of it.
            let inward = Vec3::X * -at.x.signum();
            let pocket = at + (inward * 0.06 - UP * 0.03 + FORWARD * 0.06) * scale;
            // On the collarbone when there is one: it carries the shoulder without swinging
            // with the arm, as the upper arm does.
            let anchor = find_bone(&names, Limb::Clavicle, Some(Side::Right)).unwrap_or(shoulder);
            (anchor, pocket - bones[anchor].w_axis.truncate())
        }
        "sight" => {
            let head = find_bone(&names, Limb::Head, None)?;
            // The right eye; the right of a character facing +Z is -X.
            (head, (FORWARD * 0.09 + UP * 0.08 - Vec3::X * 0.03) * scale)
        }
        _ => return None,
    };
    Some(frame_on_bone(anchor, &bones[anchor], offset, FORWARD, UP))
}

/// A weapon's suggestion for `role`, from its vertices (`gltf_info::mesh_points`), or
/// `None` when the role does not fit the weapon (a stock on a pistol) or there is no mesh.
pub fn weapon_frame(points: &[Vec3], hands: WeaponHands, role: &str) -> Option<Hardpoint> {
    let gun = GunShape::measure(points)?;
    let point = match role {
        "muzzle" => gun.muzzle(),
        "grip" => gun.grip(hands),
        "foregrip" => gun.foregrip(hands),
        "stock" => gun.stock(hands)?,
        "sight" => gun.sight(hands),
        _ => return None,
    };
    Some(Hardpoint {
        anchor: None,
        translation: round_translation(point),
        rotation_euler_deg: [0.0; 3],
    })
}

/// The bone a hand's frames hang on and the middle of its palm (model space). Usually the
/// hand bone, 9 cm along it. Some rigs (the quaternius monsters) have none, the fingers
/// growing straight out of the forearm: then the forearm, and the palm a little short of
/// the knuckles, where the fingers start.
fn palm<'a>(
    bones: &'a HashMap<String, Mat4>,
    names: &[&'a str],
    side: Side,
) -> Option<(&'a str, Vec3)> {
    let scale = rig_height(bones) / REFERENCE_HEIGHT;
    if let Some(hand) = find_bone(names, Limb::Hand, Some(side)) {
        let along = bones[hand].transform_vector3(Vec3::Y).normalize_or_zero();
        return Some((hand, bones[hand].w_axis.truncate() + along * 0.09 * scale));
    }
    let forearm = find_bone(names, Limb::Forearm, Some(side))?;
    let knuckles: Vec<Vec3> = names
        .iter()
        .filter(|name| {
            let lower = name.to_lowercase();
            FINGERS.iter().any(|f| lower.contains(f))
                && lower.contains('1')
                && side_of(&lower) == Some(side)
        })
        .map(|name| bones[*name].w_axis.truncate())
        .collect();
    let elbow = bones[forearm].w_axis.truncate();
    let along = bones[forearm]
        .transform_vector3(Vec3::Y)
        .normalize_or_zero();
    let palm = if knuckles.is_empty() {
        elbow + along * 0.3 * scale
    } else {
        knuckles.iter().sum::<Vec3>() / knuckles.len() as f32 - along * 0.03 * scale
    };
    Some((forearm, palm))
}

/// A frame at `offset` (model space) from `bone`, in the bone's own frame, whose X runs
/// along `barrel` and Y along `top` (model space, perpendicular): the axes a weapon's own
/// frames are written in, so a weapon frame lands on it the right way up. A bone's +Y is
/// along it toward its child (the fingers, for a hand) on every rig Mesh2Motion supports.
fn frame_on_bone(anchor: &str, bone: &Mat4, offset: Vec3, barrel: Vec3, top: Vec3) -> Hardpoint {
    let local_offset = bone.inverse().transform_vector3(offset);
    // The bone's rotation without its scale: rigs bake scale into the chain.
    let (_, bone_rotation, _) = bone.to_scale_rotation_translation();
    let held = Quat::from_mat3(&Mat3::from_cols(barrel, top, barrel.cross(top)));
    let local = bone_rotation.inverse() * held;
    let (x, y, z) = local.to_euler(EulerRot::XYZ);
    Hardpoint {
        anchor: Some(anchor.to_string()),
        translation: round_translation(local_offset),
        rotation_euler_deg: [x, y, z].map(|r| (r.to_degrees() * 10.0).round() / 10.0),
    }
}

fn round_translation(v: Vec3) -> [f32; 3] {
    v.to_array().map(|c| (c * 10_000.0).round() / 10_000.0)
}

/// Top to bottom of the skeleton in the rest pose.
fn rig_height(bones: &HashMap<String, Mat4>) -> f32 {
    let heights = bones.values().map(|m| m.w_axis.y);
    let (low, high) = heights.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
    if high > low {
        high - low
    } else {
        REFERENCE_HEIGHT
    }
}

const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "pinky"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum Limb {
    Hand,
    Forearm,
    UpperArm,
    Clavicle,
    Head,
}

/// The bone for `limb` on `side`, across naming schemes (`hand_r`, `mixamorig:RightHand`,
/// `Hand.R`, `RightArm`, `clavicle_l`, ...). Shortest name wins, skipping leaf helpers
/// like `RightHandEnd`.
fn find_bone<'a>(names: &[&'a str], limb: Limb, side: Option<Side>) -> Option<&'a str> {
    const HELPERS: [&str; 5] = ["end", "leaf", "nub", "top", "twist"];
    let is_limb = |lower: &str| match limb {
        Limb::Hand => lower.contains("hand") && !FINGERS.iter().any(|f| lower.contains(f)),
        Limb::Forearm => ["forearm", "lowerarm", "lower_arm"]
            .iter()
            .any(|w| lower.contains(w)),
        Limb::UpperArm => {
            (lower.contains("upperarm") || lower.contains("upper_arm") || lower.contains("arm"))
                && !["fore", "lower", "hand"].iter().any(|w| lower.contains(w))
        }
        Limb::Clavicle => lower.contains("clavicle") || lower.contains("shoulder"),
        Limb::Head => lower.contains("head"),
    };
    names
        .iter()
        .copied()
        .filter(|name| {
            let lower = name.to_lowercase();
            is_limb(&lower)
                && !HELPERS.iter().any(|h| lower.contains(h))
                && side.is_none_or(|side| side_of(&lower) == Some(side))
        })
        .min_by_key(|name| name.len())
}

fn side_of(lower: &str) -> Option<Side> {
    let marked = |word: &str, letter: char| {
        lower.contains(word)
            || [".", "_", " "]
                .iter()
                .any(|sep| lower.ends_with(&format!("{sep}{letter}")))
            || lower.starts_with(&format!("{letter}_"))
    };
    if marked("right", 'r') {
        Some(Side::Right)
    } else if marked("left", 'l') {
        Some(Side::Left)
    } else {
        None
    }
}

/// Slices along the barrel this many to the gun's length.
const SLICES: usize = 24;

/// A gun's mesh as a profile along its barrel (+X).
struct GunShape {
    min: Vec3,
    max: Vec3,
    /// Per slice, rear to front: lowest and highest Y of the vertices in it.
    profile: Vec<Option<(f32, f32)>>,
    points: Vec<Vec3>,
}

impl GunShape {
    fn measure(points: &[Vec3]) -> Option<Self> {
        let min = points.iter().copied().reduce(Vec3::min)?;
        let max = points.iter().copied().reduce(Vec3::max)?;
        if max.x - min.x <= f32::EPSILON || max.y - min.y <= f32::EPSILON {
            return None;
        }
        let mut profile: Vec<Option<(f32, f32)>> = vec![None; SLICES];
        for p in points {
            if let Some(slot) = profile.get_mut(Self::slice_of(min.x, max.x, p.x)) {
                *slot = Some(slot.map_or((p.y, p.y), |(lo, hi)| (lo.min(p.y), hi.max(p.y))));
            }
        }
        Some(Self {
            min,
            max,
            profile,
            points: points.to_vec(),
        })
    }

    fn slice_of(min_x: f32, max_x: f32, x: f32) -> usize {
        let t = (x - min_x) / (max_x - min_x);
        // `t` is in [0, 1]; the cast saturates and the `min` keeps the front edge in.
        ((t * SLICES as f32) as usize).min(SLICES - 1)
    }

    fn length(&self) -> f32 {
        self.max.x - self.min.x
    }

    fn slice_x(&self, slice: usize) -> f32 {
        self.min.x + self.length() * (slice as f32 + 0.5) / SLICES as f32
    }

    /// Vertices between `from` and `to` along the barrel.
    fn between(&self, from: f32, to: f32) -> impl Iterator<Item = Vec3> + '_ {
        self.points
            .iter()
            .copied()
            .filter(move |p| p.x >= from && p.x <= to)
    }

    fn centre(points: impl Iterator<Item = Vec3>) -> Option<Vec3> {
        let (sum, count) = points.fold((Vec3::ZERO, 0_u32), |(s, n), p| (s + p, n + 1));
        (count > 0).then(|| sum / count as f32)
    }

    /// The front of the gun, where the barrel's line runs: the middle of the front end.
    fn front(&self) -> Vec3 {
        let from = self.max.x - self.length() * 0.1;
        let (lo, hi) = self
            .between(from, self.max.x)
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                (lo.min(p.y), hi.max(p.y))
            });
        let z = Self::centre(self.between(from, self.max.x)).map_or(0.0, |c| c.z);
        Vec3::new(self.max.x, f32::midpoint(lo, hi), z)
    }

    /// Bottom of the barrel: the median bottom of the front fifth's slices. The median,
    /// because a magazine or a pump reaching into that fifth is one slice, not the barrel.
    fn barrel_bottom(&self) -> f32 {
        let front = SLICES - SLICES / 5;
        let mut bottoms: Vec<f32> = self
            .profile
            .iter()
            .skip(front)
            .flatten()
            .map(|(lo, _)| *lo)
            .collect();
        bottoms.sort_by(f32::total_cmp);
        bottoms
            .get(bottoms.len() / 2)
            .copied()
            .unwrap_or(self.min.y)
    }

    /// Runs of slices hanging below the barrel, rear to front, as (first, last) slice.
    /// `depth` is how far below, as a fraction of how far the lowest part of the gun hangs.
    fn hanging_runs(&self, depth: f32) -> Vec<(usize, usize)> {
        let barrel = self.barrel_bottom();
        let below = barrel - (barrel - self.min.y) * depth;
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for (slice, span) in self.profile.iter().enumerate() {
            if !span.is_some_and(|(lo, _)| lo < below) {
                continue;
            }
            match runs.last_mut() {
                // A slice with no vertices of its own (a long flat face) does not break a run.
                Some((_, last))
                    if self
                        .profile
                        .iter()
                        .take(slice)
                        .skip(*last + 1)
                        .all(Option::is_none) =>
                {
                    *last = slice;
                }
                _ => runs.push((slice, slice)),
            }
        }
        runs
    }

    /// Where the trigger hand closes: the first part hanging below the barrel. One-handed
    /// that is the handle, and the grip is its middle. Two-handed, the back eighth is
    /// skipped (the butt of a stock hangs too), the part has to hang at least 40% as deep
    /// as the gun's lowest point (a receiver's underside does not), and the grip is the
    /// part's rear, because the magazine in front of it often runs together with it.
    fn grip(&self, hands: WeaponHands) -> Vec3 {
        let runs = match hands {
            WeaponHands::OneHanded => self.hanging_runs(0.1),
            WeaponHands::TwoHanded => self.hanging_runs(0.4),
        };
        let run = match hands {
            WeaponHands::OneHanded => runs.first(),
            WeaponHands::TwoHanded => runs
                .iter()
                .find(|(first, _)| *first >= SLICES / 8)
                .or(runs.first()),
        };
        let Some(&(first, last)) = run else {
            // Nothing hangs below: hold it a quarter of the way along, under the barrel.
            let x = self.min.x + self.length() * 0.25;
            return Vec3::new(x, self.barrel_bottom(), self.front().z);
        };
        let slice_width = self.length() / SLICES as f32;
        let (from, to) = match hands {
            WeaponHands::OneHanded => (self.slice_x(first), self.slice_x(last)),
            WeaponHands::TwoHanded => {
                let start = self.slice_x(first) - slice_width / 2.0;
                (
                    start,
                    start + (self.length() * 0.08).min(slice_width * (last - first + 1) as f32),
                )
            }
        };
        let handle = Self::centre(self.between(from - slice_width / 2.0, to + slice_width / 2.0))
            .unwrap_or_else(|| self.front());
        let bottom = self
            .between(from, to)
            .map(|p| p.y)
            .fold(self.barrel_bottom(), f32::min);
        // A little below the middle of the handle, where a fist closed around it centres.
        let y = bottom + (self.barrel_bottom() - bottom) * 0.45;
        Vec3::new(f32::midpoint(from, to), y, handle.z)
    }

    fn muzzle(&self) -> Vec3 {
        self.front()
    }

    /// Two-handed: under the barrel, halfway from the grip to the muzzle. One-handed: the
    /// support hand cupping the trigger hand, against the left face of the handle (the
    /// left of a gun pointing +X with +Y up is -Z).
    fn foregrip(&self, hands: WeaponHands) -> Vec3 {
        let grip = self.grip(hands);
        match hands {
            WeaponHands::OneHanded => Vec3::new(grip.x, grip.y, self.min.z),
            WeaponHands::TwoHanded => {
                let x = grip.x + (self.max.x - grip.x) * 0.55;
                let half = self.length() / SLICES as f32;
                let (lo, hi) = self
                    .between(x - half, x + half)
                    .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                        (lo.min(p.y), hi.max(p.y))
                    });
                let y = if lo <= hi {
                    f32::midpoint(lo, hi)
                } else {
                    self.front().y
                };
                Vec3::new(x, y, self.front().z)
            }
        }
    }

    /// The back end, on the barrel's line. A one-handed gun has none.
    fn stock(&self, hands: WeaponHands) -> Option<Vec3> {
        (hands == WeaponHands::TwoHanded).then(|| {
            let to = self.min.x + self.length() * 0.08;
            let back = Self::centre(self.between(self.min.x, to)).unwrap_or_else(|| self.front());
            Vec3::new(self.min.x, back.y, back.z)
        })
    }

    /// The top of the gun above the grip, where a rear sight sits.
    fn sight(&self, hands: WeaponHands) -> Vec3 {
        let grip = self.grip(hands);
        let half = self.length() * 0.06;
        let top = self
            .between(grip.x - half, grip.x + half)
            .map(|p| p.y)
            .fold(f32::MIN, f32::max);
        let y = if top > f32::MIN { top } else { self.max.y };
        Vec3::new(grip.x, y, self.front().z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::gltf_info;
    use crate::assets::hardpoint::hardpoint_frame;

    fn close(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
        (Vec3::from(a) - Vec3::from(b)).length() < tolerance
    }

    /// Mesh2Motion exported `male-simple` holding its pistol with the pistol's origin at
    /// (-0.035, 0.09, 0) under `hand_r`, turned by this quaternion. Our suggestion for the
    /// character's grip should be that same frame, give or take the few degrees between
    /// Mesh2Motion's live skeleton and the bind pose the file stores.
    #[test]
    fn the_grip_lands_where_mesh2motion_puts_a_prop() {
        let bones = gltf_info::bind_pose_world("new-models/male-simple.glb").expect("model");
        let grip = character_frame(&bones, "grip").expect("grip");
        assert_eq!(grip.anchor.as_deref(), Some("hand_r"));
        assert!(
            close(grip.translation, [-0.035, 0.09, 0.0], 0.01),
            "got {:?}",
            grip.translation
        );
        let theirs = Quat::from_xyzw(0.4939, 0.506, 0.4939, 0.506);
        let angle = hardpoint_frame(&grip)
            .rotation
            .angle_between(theirs)
            .to_degrees();
        assert!(angle < 5.0, "{angle} degrees off");
    }

    #[test]
    fn every_character_role_finds_its_bone_on_a_mesh2motion_rig() {
        let bones = gltf_info::bind_pose_world("new-models/male-simple.glb").expect("model");
        let anchor = |role| character_frame(&bones, role).and_then(|h| h.anchor);
        assert_eq!(anchor("foregrip").as_deref(), Some("hand_l"));
        assert_eq!(anchor("stock").as_deref(), Some("clavicle_r"));
        assert_eq!(anchor("sight").as_deref(), Some("head"));
        assert_eq!(anchor("muzzle"), None);
    }

    /// The quaternius monsters have no hand bone; the grip goes on the forearm, out at
    /// the fingers.
    #[test]
    fn a_rig_without_hands_holds_on_its_forearm() {
        let bones = gltf_info::bind_pose_world("packs/monsters/Blue Demon.glb").expect("model");
        let grip = character_frame(&bones, "grip").expect("grip");
        assert_eq!(grip.anchor.as_deref(), Some("LowerArm.R"));
        let world = bones["LowerArm.R"].transform_point3(Vec3::from(grip.translation));
        let knuckle = bones["Middle1.R"].w_axis.truncate();
        assert!(
            world.distance(knuckle) < 0.2,
            "{world} vs knuckle {knuckle}"
        );
    }

    #[test]
    fn bones_are_found_across_naming_schemes() {
        let mixamo = [
            "mixamorig:RightHand",
            "mixamorig:RightHandIndex1",
            "mixamorig:LeftHand",
            "mixamorig:RightArm",
            "mixamorig:RightForeArm",
            "mixamorig:RightShoulder",
            "mixamorig:Head",
            "mixamorig:HeadTop_End",
        ];
        assert_eq!(
            find_bone(&mixamo, Limb::Hand, Some(Side::Right)),
            Some("mixamorig:RightHand")
        );
        assert_eq!(
            find_bone(&mixamo, Limb::Hand, Some(Side::Left)),
            Some("mixamorig:LeftHand")
        );
        assert_eq!(
            find_bone(&mixamo, Limb::UpperArm, Some(Side::Right)),
            Some("mixamorig:RightArm")
        );
        assert_eq!(
            find_bone(&mixamo, Limb::Clavicle, Some(Side::Right)),
            Some("mixamorig:RightShoulder")
        );
        assert_eq!(find_bone(&mixamo, Limb::Head, None), Some("mixamorig:Head"));
        let blender = ["Hand.R", "Hand.L", "UpperArm.R"];
        assert_eq!(
            find_bone(&blender, Limb::Hand, Some(Side::Left)),
            Some("Hand.L")
        );
        assert_eq!(
            find_bone(&blender, Limb::UpperArm, Some(Side::Right)),
            Some("UpperArm.R")
        );
    }

    /// A box gun: barrel along +X from x=0 to 10 at y 4..5, a handle hanging at x 2..3.
    fn boxy_pistol() -> Vec<Vec3> {
        let mut points = Vec::new();
        for x in 0..=10 {
            for y in [4.0, 5.0] {
                points.push(Vec3::new(x as f32, y, 0.0));
            }
        }
        for y in 0..4 {
            for x in [2.0, 3.0] {
                points.push(Vec3::new(x, y as f32, 0.0));
            }
        }
        points
    }

    #[test]
    fn a_pistol_is_held_by_its_handle_and_fires_from_its_front() {
        let points = boxy_pistol();
        let frame = |role| weapon_frame(&points, WeaponHands::OneHanded, role).expect(role);
        let grip = frame("grip").translation;
        assert!((2.0..=3.0).contains(&grip[0]), "grip x {}", grip[0]);
        assert!(grip[1] < 4.0, "grip y {} is not on the handle", grip[1]);
        assert!(close(frame("muzzle").translation, [10.0, 4.5, 0.0], 0.01));
        assert!(weapon_frame(&points, WeaponHands::OneHanded, "stock").is_none());
        assert_eq!(frame("sight").translation[1], 5.0);
    }

    /// The case the two-handed rule is for: a stock at the very back hangs lower than the
    /// barrel too, and holding the rifle by it would be wrong.
    #[test]
    fn a_rifle_is_held_by_its_grip_not_its_stock() {
        let mut points = boxy_pistol();
        for y in 0..4 {
            points.push(Vec3::new(0.0, y as f32, 0.0));
        }
        let frame = |role| weapon_frame(&points, WeaponHands::TwoHanded, role).expect(role);
        let grip = frame("grip").translation;
        assert!((1.5..=3.5).contains(&grip[0]), "grip x {}", grip[0]);
        let foregrip = frame("foregrip").translation;
        assert!(foregrip[0] > grip[0] && foregrip[0] < 10.0);
        assert_eq!(frame("stock").translation[0], 0.0);
    }

    /// Every weapon in the game gets a full set, inside its own bounds.
    #[test]
    fn every_weapon_def_gets_frames_inside_its_model() {
        for def in [
            "Pistol",
            "SMG",
            "Shotgun",
            "Assault Rifle",
            "Grenade Launcher",
        ] {
            let text = std::fs::read_to_string(format!("assets/defs/{def}.ron")).expect("def");
            let def: crate::assets::asset_definition::AssetDefinition =
                ron::from_str(&text).expect("parses");
            let points = gltf_info::mesh_points(&def.model_path).expect("mesh");
            let (min, max) = (
                points.iter().copied().reduce(Vec3::min).expect("points"),
                points.iter().copied().reduce(Vec3::max).expect("points"),
            );
            for role in ["grip", "muzzle", "foregrip", "sight"] {
                let p = Vec3::from(
                    weapon_frame(&points, WeaponHands::TwoHanded, role)
                        .expect(role)
                        .translation,
                );
                let slack = (max - min) * 0.01;
                assert!(
                    p.cmpge(min - slack).all() && p.cmple(max + slack).all(),
                    "{} {role} {p} outside {min}..{max}",
                    def.model_path
                );
            }
        }
    }
}
