//! What is actually inside a `.glb` / `.gltf`, read straight off the file.
//!
//! The character editor needs one answer before it can do anything sensible with a file the user
//! clicks: does it contain geometry, or is it an animation library? A GLB with no meshes
//! can be imported as a player model, spawned, and walked around as — and nothing appears
//! on screen, because there is nothing to draw. That is not a bug you can see; it looks
//! like the model failed to load.
//!
//! This reads the glTF JSON directly rather than going through `AssetServer`, because the
//! answer is needed while building a list, synchronously, for files that are not (and
//! should not be) loaded.

use std::collections::HashMap;
use std::path::Path;

use bevy::math::{Mat4, Quat, Vec3};
use bevy::transform::components::Transform;

/// The `assets/` folder every model path is relative to.
const ASSET_ROOT: &str = "assets";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GltfInfo {
    pub mesh_count: usize,
    /// Clip names, in file order — the order `GltfAssetLabel::Animation(i)` uses.
    pub animations: Vec<String>,
    /// Names of the nodes that draw a mesh, sorted: what `hidden_nodes` can hide.
    pub mesh_nodes: Vec<String>,
    /// How many nodes are joints of a skin.
    pub joint_count: usize,
}

impl GltfInfo {
    /// Nothing to draw. Such a file is an animation library, not a model.
    pub const fn is_animation_only(&self) -> bool {
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
    parse_info(&read_json(path)?)
}

/// The skeleton's bind pose as local transforms (bone name -> `Transform`): the pose the
/// mesh was skinned in, a T-pose on most rigs (an A-pose on some).
///
/// Taken from the skins' inverse bind matrices, *not* the nodes' own transforms. Exporters
/// write whatever pose the armature was left in onto the nodes -- `amy.glb` has a frame of
/// animation baked there, one arm out front and one hanging -- while the inverse bind
/// matrices are the bind pose by definition. Nodes no skin covers fall back to their
/// authored transform.
///
/// Read off the file rather than captured from the spawned scene, because by the time
/// anything could look at the spawned joints the animation has already posed them.
pub fn rest_pose(model_path: &str) -> Option<HashMap<String, Transform>> {
    let (doc, buffers) = read_document(model_path)?;
    Some(rest_pose_of(&doc, &buffers))
}

/// Every buffer the document declares, in order: the GLB's own `BIN` chunk for one with no
/// `uri`, a sidecar file for a relative one. Embedded `data:` URIs come back empty -- the
/// skins in them are skipped and those bones fall back to their node transforms.
fn load_buffers(doc: &serde_json::Value, mut bin: Option<Vec<u8>>, dir: &Path) -> Vec<Vec<u8>> {
    doc.get("buffers")
        .and_then(|b| b.as_array())
        .into_iter()
        .flatten()
        .map(|buffer| match buffer.get("uri").and_then(|u| u.as_str()) {
            None => bin.take().unwrap_or_default(),
            Some(uri) if uri.starts_with("data:") => Vec::new(),
            Some(uri) => std::fs::read(dir.join(uri)).unwrap_or_default(),
        })
        .collect()
}

/// The `BIN` chunk of a binary glTF: the one after the JSON chunk, if there is one.
fn glb_bin(bytes: &[u8]) -> Option<&[u8]> {
    let json_len = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?) as usize;
    let header = 20usize.checked_add(json_len)?;
    let len_end = header.checked_add(4)?;
    let len = u32::from_le_bytes(bytes.get(header..len_end)?.try_into().ok()?) as usize;
    let payload = len_end.checked_add(4)?;
    if bytes.get(len_end..payload)? != b"BIN\0" {
        return None;
    }
    bytes.get(payload..payload.checked_add(len)?)
}

fn read_json(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("gltf") => String::from_utf8(bytes).ok(),
        _ => glb_json(&bytes),
    }
}

/// Pull the JSON chunk out of a binary glTF.
///
/// Layout: a 12-byte header (`glTF`, version, total length) then chunks, each a 4-byte
/// length, a 4-byte type, and the payload. The JSON chunk is required to be first.
fn glb_json(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 20 || bytes.get(0..4)? != b"glTF" {
        return None;
    }
    let chunk_len = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?) as usize;
    if bytes.get(16..20)? != b"JSON" {
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
    let mesh_count = value
        .get("meshes")
        .and_then(|m| m.as_array())
        .map_or(0, std::vec::Vec::len);
    let animations = value
        .get("animations")
        .and_then(|a| a.as_array())
        .map_or_default(|clips| {
            clips
                .iter()
                .enumerate()
                .map(|(index, clip)| {
                    clip.get("name")
                        .and_then(|n| n.as_str())
                        .map_or_else(|| format!("Animation{index}"), str::to_string)
                })
                .collect()
        });
    let nodes = value.get("nodes").and_then(|n| n.as_array());
    let mut mesh_nodes: Vec<String> = nodes.map_or_default(|nodes| {
        nodes
            .iter()
            .filter(|node| node.get("mesh").is_some())
            .filter_map(|node| node.get("name").and_then(|n| n.as_str()))
            .map(str::to_string)
            .collect()
    });
    mesh_nodes.sort();
    mesh_nodes.dedup();
    let joint_count = value
        .get("skins")
        .and_then(|s| s.as_array())
        .map_or(0, |skins| {
            skins
                .iter()
                .filter_map(|skin| skin.get("joints").and_then(|j| j.as_array()))
                .map(Vec::len)
                .sum()
        });
    Some(GltfInfo {
        mesh_count,
        animations,
        mesh_nodes,
        joint_count,
    })
}

/// Node name -> local rest transform, see [`rest_pose`].
///
/// Per the glTF spec a joint's world matrix in the bind pose is `mesh_world * IBM^-1`,
/// where `mesh_world` is the world matrix of the node the skin is used on. A joint's local
/// is then its parent's bind world (or, for a parent outside the skin, its authored world)
/// inverted, times its own. Only ratios of the two appear, so whatever the game later does
/// to the armature node above them -- the model scale fix -- does not matter.
///
/// Unnamed nodes are skipped (nothing could look them up), and the first of a duplicated
/// name wins, matching `assets::gizmos::bone_map`.
fn rest_pose_of(doc: &serde_json::Value, buffers: &[Vec<u8>]) -> HashMap<String, Transform> {
    let nodes = nodes_of(doc);
    let parent = parents_of(nodes);
    let bind_world = bind_worlds(doc, buffers, nodes, &parent);

    let mut pose = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(name) = node.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let local = bind_world.get(&index).map_or_else(
            || node_transform(node),
            |world| {
                let parent_world = parent.get(&index).map_or(Mat4::IDENTITY, |&up| {
                    bind_world
                        .get(&up)
                        .copied()
                        .unwrap_or_else(|| authored_world(nodes, &parent, up))
                });
                Transform::from_matrix(parent_world.inverse() * *world)
            },
        );
        pose.entry(name.to_string()).or_insert(local);
    }
    pose
}

/// The bind pose as *model-space* matrices (joint name -> world matrix): where each bone
/// sits and how it is turned with the whole skeleton in its rest pose, which is what a
/// suggestion has to reason about ("forward", "down") and a local pose cannot answer
/// without its parents. Joints only; same first-name-wins rule as [`rest_pose`].
pub fn bind_pose_world(model_path: &str) -> Option<HashMap<String, Mat4>> {
    let (doc, buffers) = read_document(model_path)?;
    let nodes = nodes_of(&doc);
    let parent = parents_of(nodes);
    let mut out = HashMap::new();
    for (index, world) in bind_worlds(&doc, &buffers, nodes, &parent) {
        if let Some(name) = nodes
            .get(index)
            .and_then(|node| node.get("name"))
            .and_then(|n| n.as_str())
        {
            out.entry(name.to_string()).or_insert(world);
        }
    }
    Some(out)
}

/// Every vertex of every mesh, in model space (node transforms applied, skinning not):
/// the shape of a static model such as a gun, for guessing where its parts are.
pub fn mesh_points(model_path: &str) -> Option<Vec<Vec3>> {
    let (doc, buffers) = read_document(model_path)?;
    let nodes = nodes_of(&doc);
    let parent = parents_of(nodes);
    let meshes = doc.get("meshes").and_then(|m| m.as_array())?;
    let mut points = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(mesh) = node
            .get("mesh")
            .and_then(serde_json::Value::as_u64)
            .and_then(|m| meshes.get(m as usize))
        else {
            continue;
        };
        let world = authored_world(nodes, &parent, index);
        let primitives = mesh
            .get("primitives")
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten();
        for primitive in primitives {
            let Some(positions) = primitive
                .get("attributes")
                .and_then(|a| a.get("POSITION"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|accessor| read_floats::<3>(&doc, &buffers, accessor as usize, "VEC3"))
            else {
                continue;
            };
            points.extend(
                positions
                    .into_iter()
                    .map(|p| world.transform_point3(Vec3::from(p))),
            );
        }
    }
    Some(points)
}

/// How far the model must rise for its lowest vertex to touch the floor, in model units:
/// the value for `AssetDefinition::lift`. Negative for a model that starts above its
/// origin (it is lowered onto the floor).
pub fn floor_lift(model_path: &str) -> Option<f32> {
    let lowest = mesh_points(model_path)?
        .into_iter()
        .map(|p| p.y)
        .reduce(f32::min)?;
    Some(-lowest)
}

/// The parsed document and its buffers, for a model path as stored in a def.
fn read_document(model_path: &str) -> Option<(serde_json::Value, Vec<Vec<u8>>)> {
    let path = Path::new(ASSET_ROOT).join(model_path);
    let bytes = std::fs::read(&path).ok()?;
    let (json, bin) = match path.extension().and_then(|e| e.to_str()) {
        Some("gltf") => (String::from_utf8(bytes).ok()?, None),
        _ => (glb_json(&bytes)?, glb_bin(&bytes).map(<[u8]>::to_vec)),
    };
    let doc: serde_json::Value = serde_json::from_str(&json).ok()?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let buffers = load_buffers(&doc, bin, dir);
    Some((doc, buffers))
}

fn nodes_of(doc: &serde_json::Value) -> &[serde_json::Value] {
    doc.get("nodes")
        .and_then(|n| n.as_array())
        .map_or(&[], Vec::as_slice)
}

/// Child node index -> parent node index.
fn parents_of(nodes: &[serde_json::Value]) -> HashMap<usize, usize> {
    let mut parent = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        for child in node
            .get("children")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            if let Some(child) = child.as_u64() {
                parent.insert(child as usize, index);
            }
        }
    }
    parent
}

/// A node's world matrix from the authored node transforms.
fn authored_world(
    nodes: &[serde_json::Value],
    parent: &HashMap<usize, usize>,
    mut index: usize,
) -> Mat4 {
    let mut world = Mat4::IDENTITY;
    // Bounded by the node count, so a malformed file with a cycle cannot hang.
    for _ in 0..=nodes.len() {
        let Some(node) = nodes.get(index) else { break };
        world = node_transform(node).to_matrix() * world;
        match parent.get(&index) {
            Some(&up) => index = up,
            None => break,
        }
    }
    world
}

/// Bind-pose world matrix per joint node, across every skin we can read.
fn bind_worlds(
    doc: &serde_json::Value,
    buffers: &[Vec<u8>],
    nodes: &[serde_json::Value],
    parent: &HashMap<usize, usize>,
) -> HashMap<usize, Mat4> {
    let mut bind_world: HashMap<usize, Mat4> = HashMap::new();
    let skins = doc
        .get("skins")
        .and_then(|s| s.as_array())
        .into_iter()
        .flatten();
    for (skin_index, skin) in skins.enumerate() {
        let Some(ibms) = skin
            .get("inverseBindMatrices")
            .and_then(serde_json::Value::as_u64)
            .and_then(|accessor| read_mat4s(doc, buffers, accessor as usize))
        else {
            continue;
        };
        let mesh_world = nodes
            .iter()
            .position(|n| {
                n.get("skin").and_then(serde_json::Value::as_u64) == Some(skin_index as u64)
            })
            .map_or(Mat4::IDENTITY, |index| authored_world(nodes, parent, index));
        let joints = skin
            .get("joints")
            .and_then(|j| j.as_array())
            .into_iter()
            .flatten();
        for (joint, ibm) in joints.zip(ibms) {
            if let Some(joint) = joint.as_u64() {
                bind_world
                    .entry(joint as usize)
                    .or_insert(mesh_world * ibm.inverse());
            }
        }
    }
    bind_world
}

/// A `MAT4` float accessor's matrices, honouring the buffer view's stride.
fn read_mat4s(doc: &serde_json::Value, buffers: &[Vec<u8>], accessor: usize) -> Option<Vec<Mat4>> {
    let matrices = read_floats::<16>(doc, buffers, accessor, "MAT4")?;
    Some(matrices.iter().map(Mat4::from_cols_array).collect())
}

/// A float accessor of `N` components per element (`kind` is its glTF `type`, `"VEC3"`
/// or `"MAT4"`), honouring the buffer view's stride. Anything else -- quantized or
/// sparse data -- is `None`.
fn read_floats<const N: usize>(
    doc: &serde_json::Value,
    buffers: &[Vec<u8>],
    accessor: usize,
    kind: &str,
) -> Option<Vec<[f32; N]>> {
    const FLOAT: u64 = 5126;
    let element_bytes = N.checked_mul(4)?;
    let accessor = doc.get("accessors")?.get(accessor)?;
    if accessor.get("type")?.as_str()? != kind || accessor.get("componentType")?.as_u64()? != FLOAT
    {
        return None;
    }
    let count = accessor.get("count")?.as_u64()? as usize;
    let view = doc
        .get("bufferViews")?
        .get(accessor.get("bufferView")?.as_u64()? as usize)?;
    let buffer = buffers.get(view.get("buffer")?.as_u64()? as usize)?;
    let offset = |v: &serde_json::Value| {
        v.get("byteOffset")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize
    };
    let start = offset(view).checked_add(offset(accessor))?;
    let stride = view
        .get("byteStride")
        .and_then(serde_json::Value::as_u64)
        .map_or(element_bytes, |s| s as usize);

    (0..count)
        .map(|i| {
            let at = start.checked_add(i.checked_mul(stride)?)?;
            let bytes = buffer.get(at..at.checked_add(element_bytes)?)?;
            let mut element = [0.0_f32; N];
            for (slot, chunk) in element.iter_mut().zip(bytes.as_chunks::<4>().0) {
                *slot = f32::from_le_bytes(*chunk);
            }
            Some(element)
        })
        .collect()
}

/// A node's own local transform: a column-major `matrix`, or any of `translation` /
/// `rotation` (xyzw) / `scale`, each defaulting to identity.
fn node_transform(node: &serde_json::Value) -> Transform {
    if let Some(m) = floats::<16>(node.get("matrix")) {
        return Transform::from_matrix(Mat4::from_cols_array(&m));
    }
    Transform {
        translation: floats::<3>(node.get("translation")).map_or(Vec3::ZERO, Vec3::from),
        rotation: floats::<4>(node.get("rotation")).map_or(Quat::IDENTITY, Quat::from_array),
        scale: floats::<3>(node.get("scale")).map_or(Vec3::ONE, Vec3::from),
    }
}

/// A JSON array of exactly `N` numbers, or `None`.
fn floats<const N: usize>(value: Option<&serde_json::Value>) -> Option<[f32; N]> {
    let array = value?.as_array()?;
    if array.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, v) in out.iter_mut().zip(array) {
        *slot = v.as_f64()? as f32;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

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

    /// What `hidden_nodes` can hide: named nodes carrying a mesh, not bones or empties.
    #[test]
    fn mesh_nodes_are_the_named_nodes_with_a_mesh() {
        let info = parse_info(
            r#"{"meshes":[{},{}],
                "nodes":[{"name":"Pistol","mesh":0},{"name":"Hips"},{"name":"Body","mesh":1},{"mesh":0}],
                "skins":[{"joints":[1]}]}"#,
        )
        .unwrap();
        assert_eq!(info.mesh_nodes, vec!["Body", "Pistol"]);
        assert_eq!(info.joint_count, 1);
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

    #[test]
    fn rest_pose_reads_trs_and_defaults_the_missing_parts() {
        let pose = pose_of(
            r#"{"nodes":[
                {"name":"Hips","translation":[0,1,0]},
                {"name":"Spine","rotation":[0,0.70710677,0,0.70710677],"scale":[2,2,2]},
                {"name":"Root"}
            ]}"#,
        );
        assert_eq!(pose["Hips"].translation, Vec3::Y);
        assert_eq!(pose["Hips"].rotation, Quat::IDENTITY);
        assert!(
            pose["Spine"]
                .rotation
                .dot(Quat::from_rotation_y(FRAC_PI_2))
                .abs()
                > 1.0 - 1e-6
        );
        assert_eq!(pose["Spine"].scale, Vec3::splat(2.0));
        assert_eq!(pose["Root"], Transform::IDENTITY);
    }

    /// glTF lets a node give a column-major matrix instead of TRS.
    #[test]
    fn rest_pose_reads_a_matrix_node() {
        let pose =
            pose_of(r#"{"nodes":[{"name":"Hand","matrix":[1,0,0,0, 0,1,0,0, 0,0,1,0, 3,4,5,1]}]}"#);
        assert!(pose["Hand"].translation.distance(Vec3::new(3.0, 4.0, 5.0)) < 1e-5);
    }

    #[test]
    fn rest_pose_skips_unnamed_nodes_and_keeps_the_first_duplicate() {
        let pose = pose_of(
            r#"{"nodes":[{"translation":[9,9,9]},{"name":"A","translation":[1,0,0]},{"name":"A","translation":[2,0,0]}]}"#,
        );
        assert_eq!(pose.len(), 1);
        assert_eq!(pose["A"].translation, Vec3::X);
    }

    /// Node transforms only: no skins, so every bone falls back to what the node says.
    fn pose_of(json: &str) -> HashMap<String, Transform> {
        rest_pose_of(&serde_json::from_str(json).unwrap(), &[])
    }

    fn ibm_bytes(worlds: &[Mat4]) -> Vec<u8> {
        worlds
            .iter()
            .flat_map(|w| w.inverse().to_cols_array())
            .flat_map(f32::to_le_bytes)
            .collect()
    }

    /// The case that made the skin the source of truth: the nodes hold an animated pose,
    /// the inverse bind matrices the real one, and the skin has to win.
    #[test]
    fn the_bind_pose_comes_from_the_skin_not_the_node_transforms() {
        // Armature scaled 0.01 (the Mixamo way), a mesh and two bones under it.
        let doc = r#"{
            "nodes":[
                {"name":"Armature","scale":[0.01,0.01,0.01],"children":[1,2]},
                {"name":"Body","skin":0},
                {"name":"Hips","translation":[5,5,5],"children":[3]},
                {"name":"Arm","rotation":[0,0,0.70710677,0.70710677],"translation":[1,1,1]}
            ],
            "skins":[{"joints":[2,3],"inverseBindMatrices":0}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":2,"type":"MAT4"}],
            "bufferViews":[{"buffer":0,"byteLength":128}],
            "buffers":[{"byteLength":128}]
        }"#;
        // Bind pose in mesh space: hips 100 up, the arm 50 to their side.
        let hips = Mat4::from_translation(Vec3::new(0.0, 100.0, 0.0));
        let arm = Mat4::from_translation(Vec3::new(50.0, 100.0, 0.0));
        let buffers = vec![ibm_bytes(&[hips, arm])];
        let pose = rest_pose_of(&serde_json::from_str(doc).unwrap(), &buffers);

        assert!(
            pose["Hips"]
                .translation
                .distance(Vec3::new(0.0, 100.0, 0.0))
                < 1e-3
        );
        assert!(pose["Arm"].translation.distance(Vec3::new(50.0, 0.0, 0.0)) < 1e-3);
        assert!(pose["Arm"].rotation.dot(Quat::IDENTITY).abs() > 1.0 - 1e-6);
        // Not a joint: keeps its authored transform.
        assert_eq!(pose["Armature"].scale, Vec3::splat(0.01));
    }

    #[test]
    fn the_bin_chunk_follows_the_json_chunk() {
        let mut bytes = glb("{}");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(b"BIN\0");
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(glb_bin(&bytes), Some(&[1u8, 2, 3, 4][..]));
        assert_eq!(glb_bin(&glb("{}")), None);
    }

    /// The real file that motivated reading the skin: its nodes are posed (one arm out
    /// front, one hanging), its bind pose is a T. Chaining the locals back up must put both
    /// hands out to the sides at shoulder height.
    #[test]
    fn amy_rests_in_a_t_pose() {
        let pose = rest_pose("packs/mesh2motion/amy.glb").expect("amy.glb is tracked");
        let chain = |bone: &str| {
            let mut path = vec![
                "mixamorigHips",
                "mixamorigSpine",
                "mixamorigSpine1",
                "mixamorigSpine2",
            ];
            path.extend(match bone {
                "left" => [
                    "mixamorigLeftShoulder",
                    "mixamorigLeftArm",
                    "mixamorigLeftForeArm",
                    "mixamorigLeftHand",
                ],
                _ => [
                    "mixamorigRightShoulder",
                    "mixamorigRightArm",
                    "mixamorigRightForeArm",
                    "mixamorigRightHand",
                ],
            });
            path.iter()
                .fold(Mat4::IDENTITY, |world, b| world * pose[*b].to_matrix())
                .transform_point3(Vec3::ZERO)
        };
        let shoulder_height = (Mat4::IDENTITY
            * pose["mixamorigHips"].to_matrix()
            * pose["mixamorigSpine"].to_matrix()
            * pose["mixamorigSpine1"].to_matrix()
            * pose["mixamorigSpine2"].to_matrix())
        .transform_point3(Vec3::ZERO)
        .y;
        let (left, right) = (chain("left"), chain("right"));
        // Arms out: hands far apart sideways, level with the chest, not in front.
        assert!((left.x - right.x).abs() > 80.0, "{left} / {right}");
        assert!(
            left.y > shoulder_height && right.y > shoulder_height,
            "{left} / {right}"
        );
        assert!(
            left.z.abs() < 10.0 && right.z.abs() < 10.0,
            "{left} / {right}"
        );
    }

    /// The bag that motivated `lift`: its origin is in its middle, so it needs raising
    /// by about half its height to sit on the floor.
    #[test]
    fn the_trash_bag_is_lifted_onto_the_floor() {
        let lift = floor_lift("packs/post-apocalypse/Trash Bag.glb").expect("the bag is tracked");
        let top = mesh_points("packs/post-apocalypse/Trash Bag.glb")
            .unwrap()
            .into_iter()
            .map(|p| p.y)
            .reduce(f32::max)
            .unwrap();
        assert!(lift > 0.0, "hangs below its origin: {lift}");
        assert!(top + lift > 0.0, "and is above the floor once lifted");
        println!("trash bag lift {lift}, height {}", top + lift);
    }
}
