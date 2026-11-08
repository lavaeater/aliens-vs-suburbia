//! The pure half of dismemberment: pose a skinned mesh on the CPU, group its skeleton
//! into body parts, and split the triangles into one piece per part, each with a cap
//! over the hole the cut left.
//!
//! No ECS here; `super` gathers the meshes and joints off the corpse and spawns what
//! comes back.

use bevy::math::{Mat3, Mat4, Vec3, Vec4};
use std::collections::HashMap;

/// A part smaller than this share of the body (by vertex count) stays with its parent:
/// fingers stay on the hand, a ponytail on the head.
pub const MIN_SHARE: f32 = 0.025;

/// One joint of the skeleton, for [`regions`].
#[derive(Clone, Copy, Debug)]
pub struct JointNode {
    /// The nearest ancestor that is also a joint of the skin.
    pub parent: Option<usize>,
    /// How much of the mesh follows this joint (vertices it dominates).
    pub weight: f32,
}

/// Group a skeleton into body parts: the region index of each joint.
///
/// Two cuts, rig-agnostic so it works on spiders and snakes as well as people:
/// - **forks**: where a joint has two or more children that each carry at least
///   `min_share` of the body, every such child starts a part (neck and shoulders off
///   the chest, the legs off the hips). Small branches (fingers) don't count.
/// - **chains**: each part is then cut once more at the joint that halves it best
///   (elbows, knees, a snake's middle), if both halves are still `min_share`.
pub fn regions(nodes: &[JointNode], min_share: f32) -> Vec<usize> {
    let n = nodes.len();
    let mut children = vec![Vec::new(); n];
    for (j, node) in nodes.iter().enumerate() {
        if let Some(list) = node.parent.and_then(|p| children.get_mut(p)) {
            list.push(j);
        }
    }

    // Parents before children.
    let mut order: Vec<usize> = (0..n)
        .filter(|&j| nodes.get(j).is_some_and(|node| node.parent.is_none()))
        .collect();
    let mut head = 0;
    while let Some(&j) = order.get(head) {
        head += 1;
        order.extend(children.get(j).into_iter().flatten().copied());
    }

    let mut subtree: Vec<f32> = nodes.iter().map(|node| node.weight).collect();
    for &j in order.iter().rev() {
        let below: f32 = children
            .get(j)
            .into_iter()
            .flatten()
            .filter_map(|&c| subtree.get(c))
            .sum();
        if let Some(w) = subtree.get_mut(j) {
            *w += below;
        }
    }
    let total: f32 = nodes.iter().map(|node| node.weight).sum();
    let min = total * min_share;
    let carries = |j: usize| subtree.get(j).is_some_and(|&w| w >= min && w > 0.0);

    let mut region = vec![0usize; n];
    let mut next = 0;
    for &j in &order {
        let parent = nodes.get(j).and_then(|node| node.parent);
        let own = match parent {
            None => true,
            Some(p) => {
                let forks = children
                    .get(p)
                    .into_iter()
                    .flatten()
                    .filter(|&&c| carries(c))
                    .count()
                    >= 2;
                forks && carries(j)
            }
        };
        let r = if own {
            next += 1;
            next - 1
        } else {
            parent.and_then(|p| region.get(p).copied()).unwrap_or(0)
        };
        if let Some(slot) = region.get_mut(j) {
            *slot = r;
        }
    }

    // Halve each part once along its chain (only the parts found above, not the halves).
    let found = next;
    for r in 0..found {
        // The part's own weight below each of its joints, children first.
        let mut inside = vec![0.0f32; n];
        for &j in order.iter().rev() {
            if region.get(j) != Some(&r) {
                continue;
            }
            let below: f32 = children
                .get(j)
                .into_iter()
                .flatten()
                .filter(|&&c| region.get(c) == Some(&r))
                .filter_map(|&c| inside.get(c))
                .sum();
            if let Some(w) = inside.get_mut(j) {
                *w = nodes.get(j).map_or(0.0, |node| node.weight) + below;
            }
        }
        let part_root = |j: usize| {
            nodes
                .get(j)
                .and_then(|node| node.parent)
                .is_none_or(|p| region.get(p) != Some(&r))
        };
        let whole = order
            .iter()
            .filter(|&&j| region.get(j) == Some(&r) && part_root(j))
            .filter_map(|&j| inside.get(j))
            .sum::<f32>();
        let best = order
            .iter()
            .filter(|&&j| region.get(j) == Some(&r) && !part_root(j))
            .map(|&j| {
                let below = inside.get(j).copied().unwrap_or(0.0);
                (j, below.min(whole - below))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((cut, smaller)) = best else { continue };
        if smaller < min || smaller <= 0.0 {
            continue;
        }
        let fresh = next;
        next += 1;
        for &j in &order {
            let parent_moved = nodes
                .get(j)
                .and_then(|node| node.parent)
                .is_some_and(|p| region.get(p) == Some(&fresh));
            if region.get(j) == Some(&r)
                && (j == cut || parent_moved)
                && let Some(slot) = region.get_mut(j)
            {
                *slot = fresh;
            }
        }
    }
    region
}

/// The joint a vertex follows most.
pub fn dominant_joint(joints: [u16; 4], weights: [f32; 4]) -> u16 {
    joints
        .into_iter()
        .zip(weights)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(j, _)| j)
}

/// A skinned mesh primitive, as read off its `Mesh` asset.
pub struct Skin<'a> {
    pub positions: &'a [[f32; 3]],
    pub normals: Option<&'a [[f32; 3]]>,
    pub uvs: Option<&'a [[f32; 2]]>,
    pub tangents: Option<&'a [[f32; 4]]>,
    pub colors: Option<&'a [[f32; 4]]>,
    pub joints: &'a [[u16; 4]],
    pub weights: &'a [[f32; 4]],
    /// Triangle list.
    pub indices: Vec<u32>,
}

/// The mesh frozen in one pose, in world space.
pub struct Posed {
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub tangents: Option<Vec<Vec4>>,
}

/// Skin every vertex on the CPU: the weighted sum of its joints' `poses` (joint world
/// matrix times inverse bind matrix, one per skin joint), as the GPU would draw it.
pub fn pose(skin: &Skin, poses: &[Mat4]) -> Posed {
    let matrix = |i: usize| {
        let joints = skin.joints.get(i).copied().unwrap_or_default();
        let weights = skin.weights.get(i).copied().unwrap_or_default();
        let sum: Mat4 = joints
            .into_iter()
            .zip(weights)
            .filter(|(_, w)| *w > 0.0)
            .filter_map(|(j, w)| poses.get(usize::from(j)).map(|m| *m * w))
            .fold(Mat4::ZERO, |acc, m| acc + m);
        if sum == Mat4::ZERO {
            Mat4::IDENTITY
        } else {
            sum
        }
    };
    let matrices: Vec<Mat4> = (0..skin.positions.len()).map(matrix).collect();
    let rotate = |m: &Mat4| Mat3::from_mat4(*m);

    let positions = skin
        .positions
        .iter()
        .zip(&matrices)
        .map(|(p, m)| m.transform_point3(Vec3::from(*p)))
        .collect();
    let normals = skin.normals.map(|normals| {
        normals
            .iter()
            .zip(&matrices)
            .map(|(n, m)| (rotate(m) * Vec3::from(*n)).normalize_or(Vec3::Y))
            .collect()
    });
    let tangents = skin.tangents.map(|tangents| {
        tangents
            .iter()
            .zip(&matrices)
            .map(|(t, m)| {
                let t = Vec4::from(*t);
                (rotate(m) * t.truncate()).normalize_or(Vec3::X).extend(t.w)
            })
            .collect()
    });
    Posed {
        positions,
        normals,
        tangents,
    }
}

/// One body part's share of a mesh primitive, in world space.
#[derive(Default)]
pub struct Piece {
    pub region: usize,
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
    pub uvs: Option<Vec<[f32; 2]>>,
    pub tangents: Option<Vec<Vec4>>,
    pub colors: Option<Vec<[f32; 4]>>,
    pub indices: Vec<u32>,
    /// Triangles covering the holes the cut left, fanned from each hole's centre.
    pub cap: Vec<[Vec3; 3]>,
}

/// Which part a triangle belongs to: the one its three vertices' weights favour.
fn triangle_region(skin: &Skin, tri: [u32; 3], region_of: &[usize]) -> usize {
    let mut votes: Vec<(usize, f32)> = Vec::with_capacity(12);
    for v in tri {
        let v = v as usize;
        let joints = skin.joints.get(v).copied().unwrap_or_default();
        let weights = skin.weights.get(v).copied().unwrap_or_default();
        for (j, w) in joints.into_iter().zip(weights) {
            let r = region_of.get(usize::from(j)).copied().unwrap_or(0);
            match votes.iter_mut().find(|(seen, _)| *seen == r) {
                Some((_, total)) => *total += w,
                None => votes.push((r, w)),
            }
        }
    }
    votes
        .into_iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(0, |(r, _)| r)
}

/// Split a posed primitive into one [`Piece`] per part (`region_of`: skin joint index
/// -> part). Parts with no triangles are left out.
pub fn split(skin: &Skin, posed: &Posed, region_of: &[usize]) -> Vec<Piece> {
    let triangles: Vec<[u32; 3]> = skin
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .copied()
        .filter(|t| t.iter().all(|&v| (v as usize) < skin.positions.len()))
        .collect();
    let tri_region: Vec<usize> = triangles
        .iter()
        .map(|&t| triangle_region(skin, t, region_of))
        .collect();

    // Weld vertices by position (in the bind pose), so a UV seam isn't mistaken for a cut.
    let mut weld: HashMap<[i32; 3], u32> = HashMap::new();
    let welded: Vec<u32> = skin
        .positions
        .iter()
        .map(|p| {
            let key = p.map(|c| (c * 10_000.0).round() as i32);
            let next = weld.len() as u32;
            *weld.entry(key).or_insert(next)
        })
        .collect();
    let canon = |v: u32| welded.get(v as usize).copied().unwrap_or(v);

    // Which parts share each welded edge.
    let mut edge_regions: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (t, &r) in triangles.iter().zip(&tri_region) {
        for (a, b) in edges(*t) {
            let (a, b) = (canon(a), canon(b));
            let list = edge_regions.entry((a.min(b), a.max(b))).or_default();
            if !list.contains(&r) {
                list.push(r);
            }
        }
    }

    let mut pieces: Vec<Piece> = Vec::new();
    let mut remap: HashMap<(usize, u32), u32> = HashMap::new();
    let mut boundary: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
    for (t, &r) in triangles.iter().zip(&tri_region) {
        let index = if let Some(i) = pieces.iter().position(|p| p.region == r) {
            i
        } else {
            pieces.push(Piece {
                region: r,
                normals: posed.normals.as_ref().map(|_| Vec::new()),
                uvs: skin.uvs.map(|_| Vec::new()),
                tangents: posed.tangents.as_ref().map(|_| Vec::new()),
                colors: skin.colors.map(|_| Vec::new()),
                ..Piece::default()
            });
            pieces.len() - 1
        };
        let Some(piece) = pieces.get_mut(index) else {
            continue;
        };
        for v in *t {
            let local = *remap.entry((r, v)).or_insert_with(|| {
                let i = v as usize;
                piece
                    .positions
                    .push(posed.positions.get(i).copied().unwrap_or_default());
                push_from(&mut piece.normals, posed.normals.as_deref(), i);
                push_from(&mut piece.uvs, skin.uvs, i);
                push_from(&mut piece.tangents, posed.tangents.as_deref(), i);
                push_from(&mut piece.colors, skin.colors, i);
                (piece.positions.len() - 1) as u32
            });
            piece.indices.push(local);
        }
        for (a, b) in edges(*t) {
            let key = (canon(a).min(canon(b)), canon(a).max(canon(b)));
            if edge_regions.get(&key).is_some_and(|rs| rs.len() > 1) {
                boundary.entry(r).or_default().push((a, b));
            }
        }
    }

    let at = |v: u32| posed.positions.get(v as usize).copied().unwrap_or_default();
    for piece in &mut pieces {
        let cut = boundary.remove(&piece.region).unwrap_or_default();
        piece.cap = cap(&cut, canon, at);
    }
    pieces
}

fn edges([a, b, c]: [u32; 3]) -> [(u32, u32); 3] {
    [(a, b), (b, c), (c, a)]
}

fn push_from<T: Copy>(out: &mut Option<Vec<T>>, from: Option<&[T]>, i: usize) {
    if let (Some(out), Some(value)) = (out.as_mut(), from.and_then(|f| f.get(i))) {
        out.push(*value);
    }
}

/// Fan a cap over each hole: trace the cut edges into loops (by welded vertex) and join
/// each loop of three or more to its centre. The edges run the way their triangles wind,
/// so the cap winds back the other way, facing out of the piece.
fn cap(cut: &[(u32, u32)], canon: impl Fn(u32) -> u32, at: impl Fn(u32) -> Vec3) -> Vec<[Vec3; 3]> {
    let mut next: HashMap<u32, (u32, Vec3)> = HashMap::new();
    let mut start_at: HashMap<u32, Vec3> = HashMap::new();
    for &(a, b) in cut {
        next.entry(canon(a)).or_insert((canon(b), at(b)));
        start_at.entry(canon(a)).or_insert(at(a));
    }

    let mut tris = Vec::new();
    let mut keys: Vec<u32> = next.keys().copied().collect();
    keys.sort_unstable();
    for start in keys {
        let Some(first) = start_at.get(&start).copied() else {
            continue;
        };
        if !next.contains_key(&start) {
            continue;
        }
        let mut ring = vec![first];
        let mut current = start;
        while let Some((to, position)) = next.remove(&current) {
            if to == start {
                break;
            }
            ring.push(position);
            current = to;
            if ring.len() > cut.len() {
                break;
            }
        }
        if ring.len() < 3 {
            continue;
        }
        let centre = ring.iter().copied().sum::<Vec3>() / ring.len() as f32;
        for (i, &a) in ring.iter().enumerate() {
            let b = ring
                .get(i + 1)
                .or_else(|| ring.first())
                .copied()
                .unwrap_or(a);
            tris.push([centre, b, a]);
        }
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(parent: Option<usize>, weight: f32) -> JointNode {
        JointNode { parent, weight }
    }

    /// hips, spine, chest, neck, head, then per arm shoulder / forearm / hand + 5
    /// fingers, per leg thigh / shin / foot.
    fn humanoid() -> (Vec<JointNode>, HashMap<&'static str, usize>) {
        let mut nodes = Vec::new();
        let mut names = HashMap::new();
        let mut add = |name: &'static str,
                       parent: Option<&str>,
                       weight: f32,
                       names: &mut HashMap<&'static str, usize>| {
            let parent = parent.map(|p| names[p]);
            nodes.push(node(parent, weight));
            names.insert(name, nodes.len() - 1);
        };
        add("hips", None, 10.0, &mut names);
        add("spine", Some("hips"), 10.0, &mut names);
        add("chest", Some("spine"), 12.0, &mut names);
        add("neck", Some("chest"), 2.0, &mut names);
        add("head", Some("neck"), 10.0, &mut names);
        for side in ["l", "r"] {
            let (upper, fore, hand): (&'static str, &'static str, &'static str) = if side == "l" {
                ("l_upper", "l_fore", "l_hand")
            } else {
                ("r_upper", "r_fore", "r_hand")
            };
            add(upper, Some("chest"), 5.0, &mut names);
            add(fore, Some(upper), 4.0, &mut names);
            add(hand, Some(fore), 2.0, &mut names);
            for finger in 0..5 {
                let name: &'static str =
                    Box::leak(format!("{side}_finger{finger}").into_boxed_str());
                add(name, Some(hand), 0.3, &mut names);
            }
        }
        for (thigh, shin, foot) in [
            ("l_thigh", "l_shin", "l_foot"),
            ("r_thigh", "r_shin", "r_foot"),
        ] {
            add(thigh, Some("hips"), 7.0, &mut names);
            add(shin, Some(thigh), 5.0, &mut names);
            add(foot, Some(shin), 2.0, &mut names);
        }
        (nodes, names)
    }

    #[test]
    fn a_humanoid_comes_apart_at_the_joints() {
        let (nodes, j) = humanoid();
        let r = regions(&nodes, MIN_SHARE);
        let of = |name: &str| r[j[name]];

        assert_ne!(of("head"), of("chest"), "head off");
        assert_ne!(of("l_upper"), of("chest"), "arm off");
        assert_ne!(of("l_upper"), of("l_hand"), "cut at the elbow");
        assert_eq!(of("l_hand"), of("l_finger3"), "fingers stay on the hand");
        assert_ne!(of("l_thigh"), of("l_foot"), "cut at the knee");
        assert_ne!(of("l_thigh"), of("r_thigh"), "two legs");

        let parts = r.iter().collect::<std::collections::HashSet<_>>().len();
        assert!((8..=16).contains(&parts), "{parts} parts");
    }

    #[test]
    fn a_snake_is_cut_in_half() {
        let nodes: Vec<JointNode> = (0..10)
            .map(|i| node(if i == 0 { None } else { Some(i - 1) }, 1.0))
            .collect();
        let r = regions(&nodes, MIN_SHARE);
        assert_eq!(r[0], r[4]);
        assert_eq!(r[5], r[9]);
        assert_ne!(r[0], r[9]);
    }

    #[test]
    fn a_weightless_skeleton_stays_whole() {
        let nodes = vec![node(None, 0.0), node(Some(0), 0.0), node(Some(0), 0.0)];
        assert_eq!(regions(&nodes, MIN_SHARE), vec![0, 0, 0]);
    }

    #[test]
    fn the_strongest_weight_wins() {
        assert_eq!(dominant_joint([3, 7, 0, 0], [0.4, 0.6, 0.0, 0.0]), 7);
    }

    #[test]
    fn posing_moves_vertices_by_their_joints() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        let joints = [[0, 0, 0, 0], [1, 0, 0, 0]];
        let weights = [[1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0]];
        let skin = Skin {
            positions: &positions,
            normals: None,
            uvs: None,
            tangents: None,
            colors: None,
            joints: &joints,
            weights: &weights,
            indices: vec![],
        };
        let posed = pose(&skin, &[Mat4::IDENTITY, Mat4::from_translation(Vec3::Y)]);
        assert_eq!(posed.positions, vec![Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0)]);
    }

    /// An octahedron, top half on joint 0 and bottom half on joint 1: two pieces, each
    /// capped with a fan over the square equator.
    #[test]
    fn a_closed_mesh_splits_into_capped_halves() {
        let positions = [
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.0, -1.0, 0.0],
        ];
        let top = [1.0, 0.0, 0.0, 0.0];
        let joints = [
            [0, 0, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [0, 1, 0, 0],
            [1, 0, 0, 0],
        ];
        // The equator leans to whichever half each triangle mostly sits in.
        let weights = [
            top,
            [0.5, 0.5, 0.0, 0.0],
            [0.5, 0.5, 0.0, 0.0],
            [0.5, 0.5, 0.0, 0.0],
            [0.5, 0.5, 0.0, 0.0],
            top,
        ];
        let indices = vec![
            0, 2, 1, 0, 3, 2, 0, 4, 3, 0, 1, 4, // top
            5, 1, 2, 5, 2, 3, 5, 3, 4, 5, 4, 1, // bottom
        ];
        let skin = Skin {
            positions: &positions,
            normals: None,
            uvs: None,
            tangents: None,
            colors: None,
            joints: &joints,
            weights: &weights,
            indices,
        };
        let posed = pose(&skin, &[Mat4::IDENTITY, Mat4::IDENTITY]);
        let pieces = split(&skin, &posed, &[0, 1]);

        assert_eq!(pieces.len(), 2);
        for piece in &pieces {
            assert_eq!(piece.indices.len(), 12, "four triangles each");
            assert_eq!(piece.positions.len(), 5, "apex and the equator");
            assert_eq!(piece.cap.len(), 4, "the equator capped with four triangles");
            for tri in &piece.cap {
                assert!(tri.iter().all(|p| p.y.abs() < 1e-6), "cap lies in the cut");
            }
        }
    }

    #[test]
    fn a_uv_seam_is_not_a_cut() {
        // Two triangles of one part, the shared edge's vertices duplicated (a seam).
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ];
        let joints = [[0; 4]; 6];
        let weights = [[1.0, 0.0, 0.0, 0.0]; 6];
        let skin = Skin {
            positions: &positions,
            normals: None,
            uvs: None,
            tangents: None,
            colors: None,
            joints: &joints,
            weights: &weights,
            indices: vec![0, 1, 2, 3, 5, 4],
        };
        let posed = pose(&skin, &[Mat4::IDENTITY]);
        let pieces = split(&skin, &posed, &[0]);
        assert_eq!(pieces.len(), 1);
        assert!(pieces[0].cap.is_empty());
    }
}
