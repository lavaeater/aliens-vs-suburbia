//! Reading OpenStreetMap data from an Overpass API JSON export.
//!
//! Two shapes of export both work (`docs/gis-levels.md` has a ready-made query):
//! - `out geom;`: ways and relation members carry their own `geometry` (lat/lon list).
//! - `out body; >; out skel qt;`: ways list node ids and the nodes come as their own
//!   elements (what overpass-turbo's "raw OSM data" export of a plain query gives).
//!
//! The result is a flat list of [`Feature`]s: tags plus a point, a line or an area. Areas
//! are a set of rings filled by the even-odd rule, so a multipolygon's inner rings (a
//! courtyard, an island) come out as holes without tracking which ring is which.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

pub type Tags = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Point(LatLon),
    Line(Vec<LatLon>),
    /// Closed rings, filled even-odd.
    Area(Vec<Vec<LatLon>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    /// The OSM id (node, way or relation), used to vary things deterministically.
    pub id: i64,
    pub tags: Tags,
    pub shape: Shape,
}

#[derive(Debug, Default)]
pub struct OsmData {
    pub features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Export {
    elements: Vec<Element>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Element {
    Node {
        id: i64,
        lat: f64,
        lon: f64,
        #[serde(default)]
        tags: Tags,
    },
    Way {
        id: i64,
        #[serde(default)]
        nodes: Vec<i64>,
        #[serde(default)]
        geometry: Option<Vec<Option<LatLon>>>,
        #[serde(default)]
        tags: Tags,
    },
    Relation {
        id: i64,
        #[serde(default)]
        members: Vec<Member>,
        #[serde(default)]
        tags: Tags,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct Member {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "ref")]
    id: i64,
    #[serde(default)]
    role: String,
    #[serde(default)]
    geometry: Option<Vec<Option<LatLon>>>,
}

/// Parse an Overpass JSON export.
pub fn parse(json: &str) -> Result<OsmData, String> {
    let export: Export =
        serde_json::from_str(json).map_err(|e| format!("not an Overpass JSON export: {e}"))?;

    let mut nodes: HashMap<i64, LatLon> = HashMap::new();
    for element in &export.elements {
        if let Element::Node { id, lat, lon, .. } = element {
            nodes.insert(*id, LatLon { lat: *lat, lon: *lon });
        }
    }
    let resolve = |ids: &[i64], geometry: &Option<Vec<Option<LatLon>>>| -> Vec<LatLon> {
        match geometry {
            // Points outside a clipped `out geom(bbox)` come back as null: skip them.
            Some(points) => points.iter().flatten().copied().collect(),
            None => ids.iter().filter_map(|id| nodes.get(id).copied()).collect(),
        }
    };
    let mut way_points: HashMap<i64, Vec<LatLon>> = HashMap::new();
    for element in &export.elements {
        if let Element::Way {
            id,
            nodes: ids,
            geometry,
            ..
        } = element
        {
            way_points.insert(*id, resolve(ids, geometry));
        }
    }

    let mut features = Vec::new();
    for element in export.elements {
        match element {
            Element::Node { id, lat, lon, tags } if !tags.is_empty() => features.push(Feature {
                id,
                tags,
                shape: Shape::Point(LatLon { lat, lon }),
            }),
            Element::Way { id, tags, .. } if !tags.is_empty() => {
                let points = way_points.remove(&id).unwrap_or_default();
                if points.len() < 2 {
                    continue;
                }
                let shape = if is_closed(&points) && is_area(&tags) {
                    Shape::Area(vec![points])
                } else {
                    Shape::Line(points)
                };
                features.push(Feature { id, tags, shape });
            }
            Element::Relation { id, members, tags } if is_area_relation(&tags) => {
                let pieces: Vec<Vec<LatLon>> = members
                    .iter()
                    .filter(|m| m.kind == "way" && (m.role == "outer" || m.role == "inner"))
                    .map(|m| match &m.geometry {
                        Some(g) => g.iter().flatten().copied().collect(),
                        None => way_points.get(&m.id).cloned().unwrap_or_default(),
                    })
                    .filter(|p: &Vec<LatLon>| p.len() >= 2)
                    .collect();
                let rings = assemble_rings(pieces);
                if !rings.is_empty() {
                    features.push(Feature {
                        id,
                        tags,
                        shape: Shape::Area(rings),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(OsmData { features })
}

fn is_closed(points: &[LatLon]) -> bool {
    points.len() >= 4 && points.first() == points.last()
}

/// Whether a closed way is an area rather than a loop of line (a roundabout, a hedge
/// around a garden). OSM leaves this to the tags.
pub fn is_area(tags: &Tags) -> bool {
    match tags.get("area").map(String::as_str) {
        Some("yes") => return true,
        Some("no") => return false,
        _ => {}
    }
    if tags.contains_key("highway") || tags.contains_key("barrier") {
        return false;
    }
    if tags.get("natural").is_some_and(|v| v == "tree_row" || v == "coastline") {
        return false;
    }
    [
        "building", "landuse", "natural", "leisure", "amenity", "place", "water", "landcover",
    ]
    .iter()
    .any(|key| tags.contains_key(*key))
        || tags.get("waterway").is_some_and(|v| v == "riverbank")
}

fn is_area_relation(tags: &Tags) -> bool {
    matches!(
        tags.get("type").map(String::as_str),
        Some("multipolygon" | "building")
    )
}

/// Join a multipolygon's member ways end to end into closed rings. Ways that never close
/// (a member cut off by the export's bounding box) are closed by force: better a slightly
/// wrong polygon than a missing lake.
fn assemble_rings(mut pieces: Vec<Vec<LatLon>>) -> Vec<Vec<LatLon>> {
    let mut rings = Vec::new();
    while let Some(mut ring) = pieces.pop() {
        loop {
            if is_closed(&ring) {
                break;
            }
            let Some(end) = ring.last().copied() else {
                break;
            };
            let next = pieces.iter().position(|p| {
                p.first() == Some(&end) || p.last() == Some(&end)
            });
            let Some(index) = next else {
                if let Some(first) = ring.first().copied() {
                    ring.push(first);
                }
                break;
            };
            let mut piece = pieces.swap_remove(index);
            if piece.first() != Some(&end) {
                piece.reverse();
            }
            ring.extend(piece.into_iter().skip(1));
        }
        if ring.len() >= 4 {
            rings.push(ring);
        }
    }
    rings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon { lat, lon }
    }

    #[test]
    fn reads_out_geom_exports() {
        let json = r#"{"elements":[
            {"type":"node","id":1,"lat":59.0,"lon":18.0,"tags":{"natural":"tree"}},
            {"type":"way","id":2,"tags":{"building":"house"},
             "geometry":[{"lat":59.0,"lon":18.0},{"lat":59.0,"lon":18.1},
                         {"lat":59.1,"lon":18.1},{"lat":59.0,"lon":18.0}]},
            {"type":"way","id":3,"tags":{"highway":"residential"},
             "geometry":[{"lat":59.0,"lon":18.0},null,{"lat":59.1,"lon":18.0}]}
        ]}"#;
        let data = parse(json).unwrap();
        assert_eq!(data.features.len(), 3);
        assert!(matches!(data.features[0].shape, Shape::Point(_)));
        assert!(matches!(&data.features[1].shape, Shape::Area(r) if r.len() == 1));
        assert!(matches!(&data.features[2].shape, Shape::Line(p) if p.len() == 2));
    }

    #[test]
    fn reads_node_reference_exports_and_skips_untagged() {
        let json = r#"{"elements":[
            {"type":"node","id":1,"lat":59.0,"lon":18.0},
            {"type":"node","id":2,"lat":59.0,"lon":18.1},
            {"type":"way","id":10,"nodes":[1,2],"tags":{"barrier":"fence"}},
            {"type":"way","id":11,"nodes":[1,2]}
        ]}"#;
        let data = parse(json).unwrap();
        assert_eq!(data.features.len(), 1);
        assert_eq!(
            data.features[0].shape,
            Shape::Line(vec![ll(59.0, 18.0), ll(59.0, 18.1)])
        );
    }

    #[test]
    fn closed_hedges_and_roundabouts_stay_lines() {
        let mut tags = Tags::new();
        tags.insert("barrier".into(), "hedge".into());
        assert!(!is_area(&tags));
        tags.insert("area".into(), "yes".into());
        assert!(is_area(&tags));
        let mut park = Tags::new();
        park.insert("leisure".into(), "park".into());
        assert!(is_area(&park));
    }

    #[test]
    fn multipolygon_members_join_into_rings() {
        let json = r#"{"elements":[
            {"type":"relation","id":5,"tags":{"type":"multipolygon","natural":"water"},
             "members":[
               {"type":"way","ref":1,"role":"outer",
                "geometry":[{"lat":0.0,"lon":0.0},{"lat":0.0,"lon":1.0},{"lat":1.0,"lon":1.0}]},
               {"type":"way","ref":2,"role":"outer",
                "geometry":[{"lat":0.0,"lon":0.0},{"lat":1.0,"lon":0.0},{"lat":1.0,"lon":1.0}]},
               {"type":"way","ref":3,"role":"inner",
                "geometry":[{"lat":0.4,"lon":0.4},{"lat":0.4,"lon":0.6},
                            {"lat":0.6,"lon":0.6},{"lat":0.4,"lon":0.4}]}
             ]}
        ]}"#;
        let data = parse(json).unwrap();
        let Shape::Area(rings) = &data.features[0].shape else {
            panic!("expected an area");
        };
        assert_eq!(rings.len(), 2);
        assert!(rings.iter().all(|r| is_closed(r)));
    }
}
