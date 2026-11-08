//! What an OSM feature becomes in a level, from its tags alone. Pure and table-like, so a
//! rule is easy to find and change.

use crate::general::components::map_components::RoadKind;
use super::data::Tags;

/// For turning a mapped `height` into storeys.
const METRES_PER_STOREY: f64 = 3.0;

/// Ground a tile can have. Painted largest area first, so a park inside a residential
/// area, or a car park inside a shopping area, ends up on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    Grass,
    Floor,
    Mud,
    Rock,
    Water,
}

/// How a fence or hedge affects movement. Walls (`barrier=wall`) always block everyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum FenceRule {
    /// Aliens walk around, players hop over: the suburb's fences are the maze.
    #[default]
    BlocksEnemies,
    BlocksEveryone,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
    /// An area of ground.
    Ground(Ground),
    /// A strip of ground along a line, `width` metres wide (railways, streams).
    Strip { ground: Ground, width: f64 },
    /// A road or path: floor, kept open through fences, and where it leaves the level the
    /// aliens come in.
    Road { width: f64, kind: RoadKind },
    /// A building: solid wall tiles for now, as tall as [`storeys`] says (`docs/gis-levels.md`,
    /// phase 4 makes houses).
    Building,
    /// A wall line that blocks everyone.
    Wall,
    /// A fence or hedge line, handled by the level's [`FenceRule`].
    Fence,
    /// A single tree.
    Tree,
    /// An area of trees: grass, with pines scattered over it.
    Forest,
}

/// What a feature becomes. `area` is whether the shape is an area (closed and tagged as
/// one) or a line / point.
pub fn classify(tags: &Tags, area: bool) -> Option<Paint> {
    let get = |key: &str| tags.get(key).map(String::as_str);
    let underground = get("tunnel").is_some_and(|v| v != "no")
        || get("location") == Some("underground")
        || get("layer").and_then(|l| l.parse::<i32>().ok()).is_some_and(|l| l < 0);

    if let Some(building) = get("building") {
        // Roofs (carports, petrol station canopies) are walkable underneath.
        if area && !underground && !matches!(building, "no" | "roof" | "construction") {
            return Some(Paint::Building);
        }
        return None;
    }

    if let Some(highway) = get("highway") {
        if underground || matches!(highway, "proposed" | "construction" | "abandoned" | "razed") {
            return None;
        }
        if area {
            return Some(Paint::Ground(Ground::Floor));
        }
        let (default_width, kind) = road_width(highway)?;
        let width = get("width")
            .and_then(|w| w.trim_end_matches(" m").trim().parse::<f64>().ok())
            .unwrap_or(default_width);
        return Some(Paint::Road { width, kind });
    }

    if !area {
        if let Some(railway) = get("railway") {
            if !underground && matches!(railway, "rail" | "light_rail" | "tram" | "subway" | "narrow_gauge") {
                return Some(Paint::Strip { ground: Ground::Rock, width: 3.0 });
            }
            return None;
        }
        if let Some(waterway) = get("waterway") {
            if underground {
                return None;
            }
            let width = match waterway {
                "river" | "canal" => 8.0,
                "stream" => 2.0,
                "ditch" | "drain" => 1.0,
                _ => return None,
            };
            return Some(Paint::Strip { ground: Ground::Water, width });
        }
        if let Some(barrier) = get("barrier") {
            return match barrier {
                "wall" | "retaining_wall" | "city_wall" => Some(Paint::Wall),
                "fence" | "hedge" | "guard_rail" | "railing" | "wire_fence" | "wood_fence" => {
                    Some(Paint::Fence)
                }
                _ => None,
            };
        }
        if get("natural") == Some("tree") {
            return Some(Paint::Tree);
        }
        return None;
    }

    if get("landuse") == Some("forest") || get("natural") == Some("wood") {
        return Some(Paint::Forest);
    }
    area_ground(tags).map(Paint::Ground)
}

/// How many storeys a building has: `building:levels` if it is mapped, then `height` (metres),
/// then a guess from its type. A `.map` holds one digit per tile, so at most 9.
pub fn storeys(tags: &Tags) -> u8 {
    let get = |key: &str| tags.get(key).map(String::as_str);
    let number = |value: &str| value.trim_end_matches('m').trim().parse::<f64>().ok();
    let mapped = get("building:levels")
        .and_then(number)
        .or_else(|| get("height").and_then(number).map(|m| m / METRES_PER_STOREY));
    match mapped {
        Some(levels) if levels >= 0.5 => levels.round().min(9.0) as u8,
        _ => match get("building").unwrap_or_default() {
            "apartments" | "hotel" | "hospital" => 4,
            "church" | "cathedral" => 3,
            "shed" | "garage" | "garages" | "hut" | "cabin" | "kiosk" | "greenhouse"
            | "service" | "kindergarten" => 1,
            _ => 2,
        },
    }
}

/// Default carriageway width (metres) and look, by `highway` class.
fn road_width(highway: &str) -> Option<(f64, RoadKind)> {
    Some(match highway {
        "motorway" | "trunk" | "primary" | "motorway_link" | "trunk_link" | "primary_link" => {
            (10.0, RoadKind::Major)
        }
        "secondary" | "tertiary" | "secondary_link" | "tertiary_link" => (8.0, RoadKind::Major),
        "residential" | "unclassified" | "living_street" | "road" | "busway" => {
            (6.0, RoadKind::Street)
        }
        "service" | "pedestrian" => (4.0, RoadKind::Service),
        "track" => (3.0, RoadKind::Path),
        "footway" | "cycleway" | "path" | "bridleway" | "steps" | "corridor" => {
            (2.0, RoadKind::Path)
        }
        _ => return None,
    })
}

fn area_ground(tags: &Tags) -> Option<Ground> {
    let get = |key: &str| tags.get(key).map(String::as_str);
    if get("natural") == Some("water")
        || get("waterway") == Some("riverbank")
        || matches!(get("landuse"), Some("reservoir" | "basin"))
        || get("leisure") == Some("swimming_pool")
        || get("water").is_some()
    {
        return Some(Ground::Water);
    }
    if matches!(get("natural"), Some("bare_rock" | "scree" | "shingle" | "rock" | "cliff")) {
        return Some(Ground::Rock);
    }
    if matches!(get("natural"), Some("wetland" | "mud" | "sand" | "beach"))
        || matches!(get("landuse"), Some("construction" | "brownfield" | "landfill" | "quarry"))
    {
        return Some(Ground::Mud);
    }
    if matches!(
        get("amenity"),
        Some("parking" | "school" | "marketplace" | "fuel" | "bicycle_parking")
    ) || matches!(
        get("landuse"),
        Some("retail" | "commercial" | "industrial" | "railway" | "garages")
    ) || matches!(get("place"), Some("square"))
        || (matches!(get("leisure"), Some("pitch" | "track" | "sports_centre"))
            && get("surface").is_some_and(|s| !matches!(s, "grass" | "artificial_turf")))
    {
        return Some(Ground::Floor);
    }
    if matches!(
        get("landuse"),
        Some(
            "grass" | "meadow" | "recreation_ground" | "village_green" | "allotments" | "farmland"
                | "forest" | "residential" | "cemetery" | "orchard" | "plant_nursery"
        )
    ) || matches!(
        get("leisure"),
        Some("park" | "garden" | "playground" | "pitch" | "dog_park" | "golf_course")
    ) || matches!(get("natural"), Some("wood" | "scrub" | "grassland" | "heath"))
    {
        return Some(Ground::Grass);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect()
    }

    #[test]
    fn buildings() {
        assert_eq!(classify(&tags(&[("building", "house")]), true), Some(Paint::Building));
        assert_eq!(classify(&tags(&[("building", "yes")]), true), Some(Paint::Building));
        assert_eq!(classify(&tags(&[("building", "roof")]), true), None);
        assert_eq!(
            classify(&tags(&[("building", "yes"), ("location", "underground")]), true),
            None
        );
    }

    #[test]
    fn roads_have_widths() {
        assert_eq!(
            classify(&tags(&[("highway", "residential")]), false),
            Some(Paint::Road {
                width: 6.0,
                kind: RoadKind::Street
            })
        );
        assert_eq!(
            classify(&tags(&[("highway", "footway"), ("width", "3.5")]), false),
            Some(Paint::Road {
                width: 3.5,
                kind: RoadKind::Path
            })
        );
        assert_eq!(classify(&tags(&[("highway", "service"), ("tunnel", "yes")]), false), None);
        assert_eq!(
            classify(&tags(&[("highway", "pedestrian"), ("area", "yes")]), true),
            Some(Paint::Ground(Ground::Floor))
        );
        assert_eq!(classify(&tags(&[("highway", "bus_stop")]), false), None);
    }

    #[test]
    fn ground() {
        let ground = |pairs: &[(&str, &str)]| classify(&tags(pairs), true);
        assert_eq!(ground(&[("natural", "water")]), Some(Paint::Ground(Ground::Water)));
        assert_eq!(ground(&[("leisure", "park")]), Some(Paint::Ground(Ground::Grass)));
        assert_eq!(ground(&[("amenity", "parking")]), Some(Paint::Ground(Ground::Floor)));
        assert_eq!(ground(&[("natural", "bare_rock")]), Some(Paint::Ground(Ground::Rock)));
        assert_eq!(ground(&[("landuse", "construction")]), Some(Paint::Ground(Ground::Mud)));
        assert_eq!(
            ground(&[("leisure", "pitch"), ("surface", "asphalt")]),
            Some(Paint::Ground(Ground::Floor))
        );
        assert_eq!(ground(&[("leisure", "pitch")]), Some(Paint::Ground(Ground::Grass)));
        assert_eq!(ground(&[("shop", "bakery")]), None);
    }

    #[test]
    fn lines_and_points() {
        assert_eq!(classify(&tags(&[("barrier", "hedge")]), false), Some(Paint::Fence));
        assert_eq!(classify(&tags(&[("barrier", "wall")]), false), Some(Paint::Wall));
        assert_eq!(classify(&tags(&[("barrier", "gate")]), false), None);
        assert_eq!(classify(&tags(&[("natural", "tree")]), false), Some(Paint::Tree));
        assert_eq!(
            classify(&tags(&[("waterway", "stream")]), false),
            Some(Paint::Strip { ground: Ground::Water, width: 2.0 })
        );
        assert_eq!(
            classify(&tags(&[("railway", "rail")]), false),
            Some(Paint::Strip { ground: Ground::Rock, width: 3.0 })
        );
    }
}
