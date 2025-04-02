use crate::{
    simplify,
    types::{VtFeature, VtGeometry, VtLineString, VtLinearRing, VtPoint},
};
use geojson::{Feature, FeatureCollection, Geometry, Value, feature::Id};
use std::rc::Rc;

/// Converts a GeoJSON FeatureCollection into a vector of VtFeature objects.
///
/// This function processes each feature in the provided `FeatureCollection`
/// and converts them into `VtFeature` objects according to the given parameters.
///
/// The main purpose of this conversion is to transform GeoJSON features into a
/// more suitable internal representation (`VtFeature`). It includes coordinate
/// conversion from GeoJSON coordinates (longitude and latitude) to Mercator coordinates
/// within the range of 0 to 1, adds additional parameters such as feature IDs and bounding boxes,
/// and performs line simplification. The line simplification is controlled by the `tolerance`
/// parameter.
///
/// # Arguments
///
/// * `fc` - A `FeatureCollection` containing GeoJSON features to be converted
/// * `tolerance` - Simplification tolerance (higher means simpler)
/// * `generate_id` - Whether to auto-generate feature IDs
///
/// # Returns
///
/// A vector of `VtFeature` objects representing the converted GeoJSON features.
///
/// # Examples
///
/// ```ignore
/// let feature_collection = FeatureCollection { ... };
/// let vt_features = convert(feature_collection, 3.0, false);
/// ```
pub fn convert(fc: FeatureCollection, tolerance: f64, generate_id: bool) -> Vec<VtFeature> {
    let mut vt_features: Vec<VtFeature> = Vec::with_capacity(fc.features.len());
    let mut gen_id: u64 = 0;
    for feature in fc.features {
        if feature.geometry.is_none() {
            continue;
        }
        let mut id = feature.id.clone();
        if generate_id {
            id = Some(Id::Number(gen_id.into()));
            gen_id += 1;
        }
        let vt_feature = convert_feature(feature, tolerance, id);
        if let Some(vt_feature) = vt_feature {
            vt_features.push(vt_feature);
        }
    }
    vt_features
}

pub fn convert_feature(feature: Feature, tolerance: f64, id: Option<Id>) -> Option<VtFeature> {
    let geometry = feature.geometry.as_ref()?;
    let vt_geometry = convert_geometry(geometry, tolerance)?;
    Some(VtFeature::new(vt_geometry, Rc::new(feature.properties), id))
}

fn convert_geometry(geometry: &Geometry, tolerance: f64) -> Option<VtGeometry> {
    match &geometry.value {
        Value::Point(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::Point(convert_coords(coords)))
            }
        }
        Value::MultiPoint(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::MultiPoint(
                    coords.iter().map(|p| convert_coords(p)).collect(),
                ))
            }
        }
        Value::LineString(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::LineString(convert_line_string(
                    coords, tolerance,
                )))
            }
        }
        Value::MultiLineString(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::MultiLineString(
                    coords
                        .iter()
                        .map(|coords| convert_line_string(coords, tolerance))
                        .collect(),
                ))
            }
        }

        Value::Polygon(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::Polygon(
                    coords
                        .iter()
                        .map(|coords| convert_line_ring(coords, tolerance))
                        .collect(),
                ))
            }
        }
        Value::MultiPolygon(coords) => {
            if coords.is_empty() {
                None
            } else {
                Some(VtGeometry::MultiPolygon(
                    coords
                        .iter()
                        .map(|coords| {
                            coords
                                .iter()
                                .map(|coords| convert_line_ring(coords, tolerance))
                                .collect()
                        })
                        .collect(),
                ))
            }
        }

        Value::GeometryCollection(geometries) => {
            let geometries = geometries
                .iter()
                .filter_map(|geometry| convert_geometry(geometry, tolerance))
                .collect::<Vec<_>>();
            if geometries.is_empty() {
                None
            } else {
                Some(VtGeometry::GeometryCollection(geometries))
            }
        }
    }
}

fn convert_line_string(coords: &Vec<Vec<f64>>, tolerance: f64) -> VtLineString {
    let mut dist = 0.;
    let mut elements = coords
        .iter()
        .map(|coord| convert_coords(coord))
        .collect::<Vec<_>>();
    for w in elements.windows(2) {
        let a = w[0];
        let b = w[1];
        dist += (b.x - a.x).hypot(b.y - a.y);
    }
    simplify::simplify(&mut elements, tolerance);
    VtLineString {
        elements,
        dist,
        seg_start: 0.,
        seg_end: 0.,
    }
}

fn convert_line_ring(coords: &Vec<Vec<f64>>, tolerance: f64) -> VtLinearRing {
    let mut area = 0.;
    let mut elements = coords
        .iter()
        .map(|coord| convert_coords(coord))
        .collect::<Vec<_>>();
    for w in elements.windows(2) {
        let a = w[0];
        let b = w[1];
        area += a.x * b.y - b.x * a.y;
    }

    simplify::simplify(&mut elements, tolerance);
    area = (area / 2.).abs();
    VtLinearRing { elements, area }
}

fn convert_coords(coords: &[f64]) -> VtPoint {
    let x = lng_to_mercator_x(coords[0]);
    let y = lat_to_mercator_y(coords[1]);
    VtPoint::from_xy(x, y)
}

#[inline]
fn lng_to_mercator_x(lng: f64) -> f64 {
    lng / 360. + 0.5
}
#[inline]
fn lat_to_mercator_y(lat: f64) -> f64 {
    let sin = lat.to_radians().sin();
    let y = 0.5 - 0.25 * ((1. + sin) / (1. - sin)).ln() / std::f64::consts::PI;
    y.clamp(0., 1.)
}
