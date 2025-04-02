use std::rc::Rc;

use crate::types::{
    VtFeature, VtGeometry, VtGeometryCollection, VtLineString, VtLinearRing, VtMultiPolygon,
    VtPoint, VtPolygon,
};
use crate::types::{calc_progress, get_bbox_range, get_coordinate, intersect};

/// Clips a set of geographical features (`VtFeature`) to a specified range.
///
/// This function clips the input collection of geographical features based on the given clipping range defined by `k1` and `k2`.
/// If the entire feature collection lies within the clipping range, it returns the original collection. If the entire feature collection is outside the clipping range, it returns `None`.
/// For features that are partially within the clipping range, the function performs the clipping operation and returns the clipped feature collection.
///
/// Additionally, this function supports line metric calculation, which is controlled by the `line_metric` parameter.
///
/// # Type Parameters
/// - `I`: Specifies the dimension for clipping. Typically, `0` represents the x-axis and `1` represents the y-axis.
///
/// # Arguments
/// - `features`: A reference to a vector of `VtFeature` objects representing the geographical features to be clipped.
/// - `k1`: The starting value of the clipping range.
/// - `k2`: The ending value of the clipping range.
/// - `min_all`: The minimum boundary value of all features.
/// - `max_all`: The maximum boundary value of all features.
/// - `line_metric`: A boolean flag indicating whether to calculate line metrics during the clipping process.
///
/// # Returns
/// - An `Option` containing a vector of `VtFeature` objects representing the clipped features. Returns `None` if no features are within the clipping range.
pub fn clip<const I: usize>(
    features: &[Rc<VtFeature>],
    k1: f64,
    k2: f64,
    min_all: f64,
    max_all: f64,
    line_metric: bool,
) -> Vec<Rc<VtFeature>> {
    if min_all >= k1 && max_all <= k2 {
        return features.to_vec();
    } else if max_all < k1 || min_all > k2 {
        return vec![];
    }
    let mut clipped_features: Vec<Rc<VtFeature>> = Vec::with_capacity(features.len());
    for feature in features {
        let bbox = feature.bbox.as_ref().unwrap();
        let (min, max) = get_bbox_range::<I>(bbox);
        if min >= k1 && max <= k2 {
            clipped_features.push(feature.clone());
        } else if max < k1 || min > k2 {
            continue;
        } else {
            let clipper = Clipper::<I>::new(k1, k2, line_metric);
            let clipped_geometry = clipper.clip_geometry(&feature.geometry);
            if clipped_geometry.is_none() {
                continue;
            }
            let clipped_geometry = clipped_geometry.unwrap();
            if line_metric {
                if let VtGeometry::MultiLineString(lines) = &clipped_geometry {
                    for segment in lines {
                        let feature = VtFeature::new(
                            VtGeometry::LineString(segment.clone()),
                            feature.properties.clone(),
                            feature.id.clone(),
                        );
                        clipped_features.push(Rc::new(feature));
                    }
                    continue;
                };
            }
            let feature = VtFeature::new(
                clipped_geometry,
                feature.properties.clone(),
                feature.id.clone(),
            );
            clipped_features.push(Rc::new(feature));
        }
    }
    clipped_features
}

struct Clipper<const I: usize> {
    k1: f64,
    k2: f64,
    line_metrics: bool,
}
impl<const I: usize> Clipper<I> {
    pub fn new(k1: f64, k2: f64, line_metrics: bool) -> Self {
        Self {
            k1,
            k2,
            line_metrics,
        }
    }
    pub fn clip_geometry(&self, geometry: &VtGeometry) -> Option<VtGeometry> {
        match geometry {
            VtGeometry::Point(point) => self.clip_point(point),
            VtGeometry::MultiPoint(points) => self.clip_points(points),
            VtGeometry::LineString(line_string) => self.clip_line_string(line_string),
            VtGeometry::MultiLineString(multi_line_string) => {
                self.clip_multi_line_string(multi_line_string)
            }
            VtGeometry::Polygon(polygon) => self.clip_polygon(polygon),
            VtGeometry::MultiPolygon(multi_polygon) => self.clip_multi_polygon(multi_polygon),

            VtGeometry::GeometryCollection(geometries) => self.clip_geometry_collection(geometries),
        }
    }
    fn clip_point(&self, point: &VtPoint) -> Option<VtGeometry> {
        let v = get_coordinate::<I>(point);
        if v < self.k1 || v > self.k2 {
            None
        } else {
            Some(VtGeometry::Point(*point))
        }
    }

    fn clip_points(&self, points: &[VtPoint]) -> Option<VtGeometry> {
        let multi_points = points
            .iter()
            .filter_map(|point| {
                let v = get_coordinate::<I>(point);
                if v < self.k1 || v > self.k2 {
                    None
                } else {
                    Some(*point)
                }
            })
            .collect::<Vec<_>>();
        if multi_points.is_empty() {
            None
        } else {
            Some(VtGeometry::MultiPoint(multi_points))
        }
    }
    fn clip_line_string(&self, line: &VtLineString) -> Option<VtGeometry> {
        let mut parts: Vec<VtLineString> = Vec::new();
        self.clip_line(line, &mut parts);
        match parts.len() {
            0 => None,
            1 => Some(VtGeometry::LineString(parts.pop().unwrap())),
            _ => Some(VtGeometry::MultiLineString(parts)),
        }
    }
    fn clip_line(&self, line: &VtLineString, slices: &mut Vec<VtLineString>) {
        let len = line.elements.len();
        if len < 2 {
            return;
        }
        let mut line_len = line.seg_start;
        let (k1, k2) = (self.k1, self.k2);
        let mut slice = self.new_slice(line);
        for (i, w) in line.elements.windows(2).enumerate() {
            let a = w[0];
            let b = w[1];
            let seg_len = if self.line_metrics {
                (b.x - a.x).hypot(b.y - a.y)
            } else {
                0.0
            };
            let ak = get_coordinate::<I>(&a);
            let bk = get_coordinate::<I>(&b);
            let is_last_seg = i == (len - 2);

            match (ak < k1, ak > k2, bk < k1, bk > k2) {
                (true, _, true, _) | (_, true, _, true) => (),
                (false, false, false, false) => {
                    slice.elements.push(a);
                    if self.line_metrics && is_last_seg {
                        slice.seg_end = line_len + seg_len;
                    }
                    if is_last_seg {
                        slice.elements.push(b);
                        slices.push(slice);
                        break;
                    }
                }
                _ => {
                    let enter = match ak {
                        ak if ak < k1 => k1,
                        ak if ak > k2 => k2,
                        _ => ak,
                    };
                    let exit = match bk {
                        bk if bk > k2 => k2,
                        bk if bk < k1 => k1,
                        _ => bk,
                    };
                    let t_enter = calc_progress::<I>(&a, &b, enter);
                    let t_exit = calc_progress::<I>(&a, &b, exit);
                    let p1 = intersect::<I>(&a, &b, enter, t_enter);
                    let p2 = intersect::<I>(&a, &b, exit, t_exit);
                    slice.elements.push(p1);
                    if enter != ak && self.line_metrics {
                        slice.seg_start = line_len + seg_len * t_enter;
                    }
                    if exit == bk {
                        if self.line_metrics {
                            slice.seg_end = line_len + seg_len * t_exit;
                        }
                        if is_last_seg {
                            slice.elements.push(b);
                            slices.push(slice);
                            slice = self.new_slice(line);
                        }
                    } else {
                        if self.line_metrics {
                            slice.seg_end = line_len + seg_len * t_exit;
                        }

                        slice.elements.push(p2);
                        slices.push(slice);
                        slice = self.new_slice(line);
                    }
                }
            }
            if self.line_metrics {
                line_len += seg_len;
            }
        }
    }

    fn clip_multi_line_string(&self, multi_line_string: &Vec<VtLineString>) -> Option<VtGeometry> {
        let mut parts: Vec<VtLineString> = Vec::new();
        for line in multi_line_string {
            self.clip_line(line, &mut parts);
        }
        match parts.len() {
            0 => None,
            1 => Some(VtGeometry::LineString(parts.pop().unwrap())),
            _ => Some(VtGeometry::MultiLineString(parts)),
        }
    }
    fn clip_polygon(&self, polygon: &Vec<VtLinearRing>) -> Option<VtGeometry> {
        let mut parts: VtPolygon = Vec::new();
        for ring in polygon {
            let new_ring = self.clip_ring(ring);
            if let Some(r) = new_ring {
                parts.push(r);
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(VtGeometry::Polygon(parts))
        }
    }
    fn clip_multi_polygon(&self, polygons: &VtMultiPolygon) -> Option<VtGeometry> {
        let mut parts: VtMultiPolygon = Vec::new();
        for polygon in polygons {
            let mut part: VtPolygon = Vec::new();
            for ring in polygon {
                let new_ring = self.clip_ring(ring);
                if let Some(r) = new_ring {
                    part.push(r);
                }
            }
            if !part.is_empty() {
                parts.push(part);
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(VtGeometry::MultiPolygon(parts))
        }
    }
    fn clip_geometry_collection(&self, geometries: &VtGeometryCollection) -> Option<VtGeometry> {
        let parts = geometries
            .iter()
            .filter_map(|g| self.clip_geometry(g))
            .collect::<Vec<_>>();
        if parts.is_empty() {
            None
        } else {
            Some(VtGeometry::GeometryCollection(parts))
        }
    }
    fn clip_ring(&self, ring: &VtLinearRing) -> Option<VtLinearRing> {
        let len = ring.elements.len();
        let mut slice = VtLinearRing {
            area: ring.area,
            ..Default::default()
        };
        if len < 2 {
            return None;
        }
        let k1 = self.k1;
        let k2 = self.k2;
        for (i, w) in ring.elements.windows(2).enumerate() {
            let a = w[0];
            let b = w[1];
            let ak = get_coordinate::<I>(&a);
            let bk = get_coordinate::<I>(&b);
            let is_last_seg = i == (len - 1);
            if ak < k1 {
                if bk > k1 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k1, calc_progress::<I>(&a, &b, k1)));
                }
                if bk > k2 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k2, calc_progress::<I>(&a, &b, k2)));
                } else if is_last_seg {
                    slice.elements.push(b);
                }
            } else if ak > k2 {
                if bk < k2 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k2, calc_progress::<I>(&a, &b, k2)));
                }
                if bk < k1 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k1, calc_progress::<I>(&a, &b, k1)));
                } else if is_last_seg {
                    slice.elements.push(b);
                }
            } else {
                slice.elements.push(a);
                if bk < k1 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k1, calc_progress::<I>(&a, &b, k1)));
                } else if bk > k2 {
                    slice
                        .elements
                        .push(intersect::<I>(&a, &b, k2, calc_progress::<I>(&a, &b, k2)));
                }
            }
        }
        if !slice.elements.is_empty() {
            let first = slice.elements.first();
            let last = slice.elements.last();
            if first != last {
                slice.elements.push(*first.unwrap());
            }
        }
        if slice.elements.len() < 3 {
            None
        } else {
            Some(slice)
        }
    }
    fn new_slice(&self, line: &VtLineString) -> VtLineString {
        let mut slice = VtLineString {
            dist: line.dist,
            ..Default::default()
        };
        if self.line_metrics {
            slice.seg_start = line.seg_start;
            slice.seg_end = line.seg_end;
        }
        slice
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{VtLineString, VtLinearRing, VtMultiPoint, VtPoint};

    const GEOM1: [i32; 42] = [
        0, 0, 0, 50, 0, 0, 50, 10, 0, 20, 10, 0, 20, 20, 0, 30, 20, 0, 30, 30, 0, 50, 30, 0, 50,
        40, 0, 25, 40, 0, 25, 50, 0, 0, 50, 0, 0, 60, 0, 25, 60, 0,
    ];
    const GEOM2: [i32; 12] = [0, 0, 0, 50, 0, 0, 50, 10, 0, 0, 10, 0];

    fn create_multi_point(points: &[i32]) -> VtMultiPoint {
        points
            .chunks(3)
            .map(|chunk| VtPoint::new(chunk[0] as f64, chunk[1] as f64, chunk[2] as f64))
            .collect()
    }
    fn create_line_string(points: &[i32]) -> VtLineString {
        let points = points
            .chunks(3)
            .map(|chunk| VtPoint::new(chunk[0] as f64, chunk[1] as f64, chunk[2] as f64))
            .collect();
        VtLineString {
            elements: points,
            ..Default::default()
        }
    }
    fn create_line_ring(points: &[i32]) -> VtLinearRing {
        let mut closed_points = points
            .chunks(3)
            .map(|chunk| VtPoint::new(chunk[0] as f64, chunk[1] as f64, chunk[2] as f64))
            .collect::<Vec<_>>();
        let p = closed_points[0];
        closed_points.push(p);
        VtLinearRing {
            elements: closed_points,
            ..Default::default()
        }
    }
    #[test]
    fn test_clip_points() {
        let clip = Clipper::<0>::new(10., 40., false);
        let multi_points1 = create_multi_point(&GEOM1);
        let clipped1 = clip.clip_points(&multi_points1).unwrap();
        let expected1 = create_multi_point(&[
            20, 10, 0, 20, 20, 0, 30, 20, 0, 30, 30, 0, 25, 40, 0, 25, 50, 0, 25, 60, 0,
        ]);
        assert_eq!(clipped1, VtGeometry::MultiPoint(expected1));
        let multi_points2 = create_multi_point(&GEOM2);
        let clipped2 = clip.clip_points(&multi_points2);
        assert_eq!(clipped2, None);
    }
    #[test]
    fn test_clip_line_string() {
        let line1 = create_line_string(&GEOM1);
        let line2 = create_line_string(&GEOM2);
        let clip = Clipper::<0>::new(10., 40., false);
        let clipped1 = clip.clip_line_string(&line1).unwrap();
        let clipped2 = clip.clip_line_string(&line2).unwrap();

        let expected1 = vec![
            create_line_string(&[10, 0, 1, 40, 0, 1]),
            create_line_string(&[
                40, 10, 1, 20, 10, 0, 20, 20, 0, 30, 20, 0, 30, 30, 1, 40, 30, 1,
            ]),
            create_line_string(&[40, 40, 1, 25, 40, 0, 25, 50, 1, 10, 50, 1]),
            create_line_string(&[10, 60, 1, 25, 60, 0]),
        ];
        let expected2 = vec![
            create_line_string(&[10, 0, 1, 40, 0, 1]),
            create_line_string(&[40, 10, 1, 10, 10, 1]),
        ];
        assert_eq!(clipped1, VtGeometry::MultiLineString(expected1));
        assert_eq!(clipped2, VtGeometry::MultiLineString(expected2));
    }
    #[test]
    fn test_clip_line_string_metric() {
        let line = create_line_string(&GEOM1);
        let clip = Clipper::<0>::new(10., 40., true);
        let clipped = clip.clip_line_string(&line).unwrap();
        match clipped {
            VtGeometry::MultiLineString(lines) => {
                let result = lines
                    .iter()
                    .map(|f| (f.seg_start, f.seg_end))
                    .collect::<Vec<_>>();
                assert_eq!(
                    result,
                    vec![(10., 40.), (70., 130.), (160., 200.), (230., 245.)]
                )
            }
            _ => {
                assert!(false, "Expected VtGeometry::MultiLineString")
            }
        }
    }
    #[test]
    fn clip_polygons() {
        let ring1 = create_line_ring(&GEOM1);
        let ring2 = create_line_ring(&GEOM2);
        let clip = Clipper::<0>::new(10., 40., false);

        let polygon1 = vec![ring1];
        let polygon2 = vec![ring2];
        let clipped1 = clip.clip_polygon(&polygon1).unwrap();
        let clipped2 = clip.clip_polygon(&polygon2).unwrap();
        let expected1 = VtGeometry::Polygon(vec![create_line_ring(&[
            10, 0, 1, 40, 0, 1, 40, 10, 1, 20, 10, 0, 20, 20, 0, 30, 20, 0, 30, 30, 0, 40, 30, 1,
            40, 40, 1, 25, 40, 0, 25, 50, 0, 10, 50, 1, 10, 60, 1, 25, 60, 0, 10, 24, 1,
        ])]);
        let expected2 = VtGeometry::Polygon(vec![create_line_ring(&[
            10, 0, 1, 40, 0, 1, 40, 10, 1, 10, 10, 1,
        ])]);
        assert_eq!(clipped1, expected1);
        assert_eq!(clipped2, expected2);
    }
}
