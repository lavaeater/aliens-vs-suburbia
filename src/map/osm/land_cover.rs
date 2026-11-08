//! Lantmäteriet's land cover (Marktäcke Direkt, `docs/gis-levels.md`): GeoJSON polygons with
//! an `objekttyp`. Only the forests are used. They come out as ordinary features tagged
//! `natural=wood`, so they paint exactly as OSM's own forests do.

use serde_json::Value;

use super::data::{Feature, LatLon, Shape, Tags};

/// `true` for the `objekttyp` of a forest: Barr- och blandskog, Lövskog, Fjällbjörkskog.
fn is_forest(kind: &str) -> bool {
    kind.to_lowercase().ends_with("skog")
}

fn ring(value: &Value) -> Option<Vec<LatLon>> {
    value
        .as_array()?
        .iter()
        .map(|point| {
            let point = point.as_array()?;
            Some(LatLon {
                lon: point.first()?.as_f64()?,
                lat: point.get(1)?.as_f64()?,
            })
        })
        .collect()
}

fn rings(polygon: &Value) -> Option<Vec<Vec<LatLon>>> {
    polygon.as_array()?.iter().map(ring).collect()
}

/// A polygon's or a multipolygon's rings, each with its holes. Filled even-odd, so a hole
/// stays a hole.
fn area(geometry: &Value) -> Option<Vec<Vec<LatLon>>> {
    let coordinates = geometry.get("coordinates")?;
    match geometry.get("type")?.as_str()? {
        "Polygon" => rings(coordinates),
        "MultiPolygon" => {
            let polygons: Option<Vec<_>> = coordinates.as_array()?.iter().map(rings).collect();
            Some(polygons?.into_iter().flatten().collect())
        }
        _ => None,
    }
}

/// The forests in a land cover export.
pub fn forests(text: &str) -> Result<Vec<Feature>, String> {
    let export: Value =
        serde_json::from_str(text).map_err(|e| format!("not a land cover GeoJSON export: {e}"))?;
    let features = export
        .get("features")
        .and_then(Value::as_array)
        .ok_or("not a land cover GeoJSON export: no `features`")?;
    let tags: Tags = [("natural".to_string(), "wood".to_string())].into();
    Ok(features
        .iter()
        .filter(|f| {
            f.pointer("/properties/objekttyp")
                .and_then(Value::as_str)
                .is_some_and(is_forest)
        })
        .filter_map(|f| area(f.get("geometry")?))
        .enumerate()
        .map(|(id, rings)| Feature {
            id: -(id as i64) - 1,
            tags: tags.clone(),
            shape: Shape::Area(rings),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORT: &str = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"objekttyp":"Barr- och blandskog"},
         "geometry":{"type":"Polygon","coordinates":[
            [[18.0,59.0],[18.1,59.0],[18.1,59.1],[18.0,59.0]],
            [[18.01,59.01],[18.02,59.01],[18.02,59.02],[18.01,59.01]]]}},
        {"type":"Feature","properties":{"objekttyp":"Lövskog"},
         "geometry":{"type":"MultiPolygon","coordinates":[
            [[[17.0,59.0],[17.1,59.0],[17.1,59.1],[17.0,59.0]]],
            [[[17.5,59.0],[17.6,59.0],[17.6,59.1],[17.5,59.0]]]]}},
        {"type":"Feature","properties":{"objekttyp":"Öppen mark"},
         "geometry":{"type":"Polygon","coordinates":[
            [[16.0,59.0],[16.1,59.0],[16.1,59.1],[16.0,59.0]]]}}
    ]}"#;

    #[test]
    fn only_forests_come_through_with_their_rings() {
        let found = forests(EXPORT).unwrap();
        assert_eq!(found.len(), 2);
        let ring_count = |f: &Feature| match &f.shape {
            Shape::Area(rings) => rings.len(),
            _ => 0,
        };
        assert_eq!(ring_count(&found[0]), 2, "the hole stays");
        assert_eq!(ring_count(&found[1]), 2, "both parts of the multipolygon");
    }

    #[test]
    fn coordinates_are_lon_then_lat() {
        let found = forests(EXPORT).unwrap();
        let Shape::Area(rings) = &found[0].shape else {
            panic!("an area");
        };
        let first = rings[0][0];
        assert!((first.lat - 59.0).abs() < 1e-9 && (first.lon - 18.0).abs() < 1e-9);
    }

    #[test]
    fn a_file_that_is_not_an_export_is_an_error() {
        assert!(forests("[]").is_err());
        assert!(forests("nope").is_err());
    }
}
