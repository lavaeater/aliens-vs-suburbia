//! How content files name each other.
//!
//! Every path stored in content (a map's placements, waves, weapons and stories, a def's
//! weapons, a loot table's weapon pickups, the campaign's maps) is inside `assets/`,
//! written without the prefix: `"defs/Pistol.ron"`, like a def's `model_path`. Files from
//! before that wrote `"assets/defs/Pistol.ron"`; both read the same, and anything saved
//! is written in the short form.
//!
//! The editors still work with paths from the project root (`read_dir("assets/defs")`),
//! so compare a content path with one of theirs through [`same`].

use std::path::PathBuf;

pub const ROOT: &str = "assets";

/// The form content stores: inside `assets/`, no prefix. Takes either form.
pub fn content_ref(path: &str) -> String {
    let path = path.trim().replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    path.strip_prefix("assets/").unwrap_or(path).to_string()
}

/// Strip the prefix in place (for the normalizers on loaded content).
pub fn normalize(path: &mut String) {
    if !path.is_empty() {
        *path = content_ref(path);
    }
}

/// Where a content path is on disk, from the project root (where the game runs).
pub fn disk_path(path: &str) -> PathBuf {
    PathBuf::from(ROOT).join(content_ref(path))
}

/// Whether two paths name the same file, either form.
pub fn same(a: &str, b: &str) -> bool {
    content_ref(a) == content_ref(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_forms_name_the_same_file() {
        assert_eq!(content_ref("assets/defs/Pistol.ron"), "defs/Pistol.ron");
        assert_eq!(content_ref("defs/Pistol.ron"), "defs/Pistol.ron");
        assert_eq!(content_ref("./assets/maps/a.map"), "maps/a.map");
        assert_eq!(
            disk_path("defs/Pistol.ron"),
            PathBuf::from("assets/defs/Pistol.ron")
        );
        assert_eq!(
            disk_path("assets/defs/Pistol.ron"),
            PathBuf::from("assets/defs/Pistol.ron")
        );
        assert!(same("assets/defs/SMG.ron", "defs/SMG.ron"));
        assert!(!same("defs/SMG.ron", "defs/Shotgun.ron"));
    }

    #[test]
    fn an_empty_path_stays_empty() {
        let mut path = String::new();
        normalize(&mut path);
        assert!(path.is_empty());
    }
}
