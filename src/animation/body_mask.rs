//! Splitting a skeleton into upper and lower body, for playing two clips at once: the
//! legs walk while the torso holds the aim pose (`ClipSet::upper_body_layer`).
//!
//! Bevy masks animation targets by *mask group*; `build_graph` puts every bone under the
//! split bone in `UPPER` and every other node in `LOWER`, then masks each half of the
//! pair out of the other's clip. Bones are named by their path from the scene root, the
//! same names Bevy's `AnimationTargetId::from_names` hashes.

/// Mask group of the bones from the split bone up (spine, arms, head).
pub const UPPER: u32 = 0;
/// Mask group of everything else (root, pelvis, legs).
pub const LOWER: u32 = 1;

/// The bone the upper body hangs from: the def's first aim bone (the lowest bone the
/// torso twist turns) if the skeleton has it, else the first bone named like a spine.
/// `paths` lists every node's path from the scene root, parents before children.
pub fn split_bone<'a>(paths: &'a [Vec<String>], aim_bone: Option<&str>) -> Option<&'a str> {
    let last = |path: &'a Vec<String>| path.last().map(String::as_str);
    aim_bone
        .and_then(|aim| paths.iter().filter_map(last).find(|name| *name == aim))
        .or_else(|| {
            paths
                .iter()
                .filter_map(last)
                .find(|name| name.to_lowercase().contains("spine"))
        })
}

/// Whether the node at `path` is the split bone or below it.
pub fn is_upper(path: &[String], split: &str) -> bool {
    path.iter().any(|name| name == split)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(spec: &[&str]) -> Vec<Vec<String>> {
        spec.iter()
            .map(|p| p.split('/').map(ToString::to_string).collect())
            .collect()
    }

    /// The Mesh2Motion human skeleton, as in `assets/new-models/male-simple.glb`.
    fn mesh2motion() -> Vec<Vec<String>> {
        paths(&[
            "Character_15",
            "Character_15/root",
            "Character_15/root/pelvis",
            "Character_15/root/pelvis/thigh_l",
            "Character_15/root/pelvis/spine_01",
            "Character_15/root/pelvis/spine_01/spine_02",
            "Character_15/root/pelvis/spine_01/spine_02/upperarm_r",
        ])
    }

    #[test]
    fn the_upper_body_starts_at_the_first_spine_bone() {
        let paths = mesh2motion();
        let split = split_bone(&paths, None);
        assert_eq!(split, Some("spine_01"));
        let upper: Vec<bool> = paths.iter().map(|p| is_upper(p, "spine_01")).collect();
        assert_eq!(upper, [false, false, false, false, true, true, true]);
    }

    #[test]
    fn the_first_aim_bone_wins_when_the_skeleton_has_it() {
        let paths = mesh2motion();
        assert_eq!(split_bone(&paths, Some("spine_02")), Some("spine_02"));
        assert_eq!(
            split_bone(&paths, Some("mixamorig:Spine")),
            Some("spine_01")
        );
    }

    #[test]
    fn a_skeleton_without_a_spine_has_no_split() {
        assert_eq!(
            split_bone(&paths(&["Root", "Root/Body", "Root/Body/Leg"]), None),
            None
        );
    }
}
