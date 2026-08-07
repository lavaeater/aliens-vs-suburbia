//! What is actually inside a `.glb` / `.gltf`, read straight off the file.
//!
//! The playground needs one answer before it can do anything sensible with a file the user
//! clicks: does it contain geometry, or is it an animation library? A GLB with no meshes
//! can be imported as a player model, spawned, and walked around as — and nothing appears
//! on screen, because there is nothing to draw. That is not a bug you can see; it looks
//! like the model failed to load.
//!
//! This reads the glTF JSON directly rather than going through `AssetServer`, because the
//! answer is needed while building a list, synchronously, for files that are not (and
//! should not be) loaded.

use std::path::Path;

/// The `assets/` folder every model path is relative to.
const ASSET_ROOT: &str = "assets";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GltfInfo {
    pub mesh_count: usize,
    /// Clip names, in file order — the order `GltfAssetLabel::Animation(i)` uses.
    pub animations: Vec<String>,
}

impl GltfInfo {
    /// Nothing to draw. Such a file is an animation library, not a model.
    pub fn is_animation_only(&self) -> bool {
        self.mesh_count == 0 && !self.animations.is_empty()
    }
}

/// Inspect a model path as stored in a def (relative to `assets/`, no prefix).
///
/// `None` when the file is missing or not glTF we can read — callers treat that as "no
/// opinion" and carry on, since being unable to inspect a file is not grounds for
/// refusing to use it.
pub fn inspect(model_path: &str) -> Option<GltfInfo> {
    inspect_file(&Path::new(ASSET_ROOT).join(model_path))
}

pub fn inspect_file(path: &Path) -> Option<GltfInfo> {
    let bytes = std::fs::read(path).ok()?;
    let json = match path.extension().and_then(|e| e.to_str()) {
        Some("gltf") => String::from_utf8(bytes).ok()?,
        _ => glb_json(&bytes)?,
    };
    parse_info(&json)
}

/// Pull the JSON chunk out of a binary glTF.
///
/// Layout: a 12-byte header (`glTF`, version, total length) then chunks, each a 4-byte
/// length, a 4-byte type, and the payload. The JSON chunk is required to be first.
fn glb_json(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return None;
    }
    let chunk_len = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    if &bytes[16..20] != b"JSON" {
        return None;
    }
    let end = 20usize.checked_add(chunk_len)?;
    let chunk = bytes.get(20..end)?;
    String::from_utf8(chunk.to_vec()).ok()
}

/// Count meshes and name animations, without a glTF crate.
///
/// Deliberately shallow: `ron`-adjacent JSON parsing is not worth a dependency for two
/// fields, and both live at the top level of the document.
fn parse_info(json: &str) -> Option<GltfInfo> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let mesh_count = value.get("meshes").and_then(|m| m.as_array()).map_or(0, |m| m.len());
    let animations = value
        .get("animations")
        .and_then(|a| a.as_array())
        .map(|clips| {
            clips
                .iter()
                .enumerate()
                .map(|(index, clip)| {
                    clip.get("name")
                        .and_then(|n| n.as_str())
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("Animation{index}"))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(GltfInfo { mesh_count, animations })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glb(json: &str) -> Vec<u8> {
        let payload = json.as_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&((20 + payload.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"JSON");
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn a_model_reports_its_meshes() {
        let info = parse_info(r#"{"meshes":[{},{}],"animations":[{"name":"Idle"}]}"#).unwrap();
        assert_eq!(info.mesh_count, 2);
        assert_eq!(info.animations, vec!["Idle"]);
        assert!(!info.is_animation_only());
    }

    /// The case that started this: 162 clips, 66 bones, no geometry. Worn as a player
    /// model it spawns an invisible character.
    #[test]
    fn a_file_with_clips_but_no_geometry_is_an_animation_library() {
        let info = parse_info(r#"{"nodes":[{},{}],"animations":[{"name":"Backflip"}]}"#).unwrap();
        assert_eq!(info.mesh_count, 0);
        assert!(info.is_animation_only());
    }

    /// An empty file is not an animation library — it is nothing, and offering to add it
    /// as an animation source would be worse than saying nothing.
    #[test]
    fn a_file_with_neither_meshes_nor_clips_is_not_an_animation_library() {
        let info = parse_info("{}").unwrap();
        assert!(!info.is_animation_only());
        assert_eq!(info.mesh_count, 0);
    }

    /// Unnamed clips still have to be listable; glTF does not require `name`.
    #[test]
    fn an_unnamed_clip_gets_its_index_as_a_name() {
        let info = parse_info(r#"{"animations":[{},{"name":"Wave"}]}"#).unwrap();
        assert_eq!(info.animations, vec!["Animation0", "Wave"]);
    }

    #[test]
    fn the_json_chunk_is_read_out_of_a_binary_gltf() {
        let bytes = glb(r#"{"meshes":[{}],"animations":[]}"#);
        let info = parse_info(&glb_json(&bytes).expect("json chunk")).unwrap();
        assert_eq!(info.mesh_count, 1);
    }

    /// Being unable to read a file is "no opinion", not a crash — the browser lists
    /// whatever is on disk, including things that are not glTF at all.
    #[test]
    fn something_that_is_not_a_glb_is_refused_quietly() {
        assert!(glb_json(b"not a gltf file at all").is_none());
        assert!(glb_json(&[]).is_none());
        assert!(parse_info("<html>").is_none());
    }
}
