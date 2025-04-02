use geojson::{JsonObject, feature::Id};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct VtFeature {
    pub id: Option<Id>,
    pub geometry: VtGeometry,
    pub properties: Rc<Option<JsonObject>>,
    pub bbox: Option<BBox>,
    pub point_count: u32,
}

impl VtFeature {
    pub fn new(
        mut geometry: VtGeometry,
        properties: Rc<Option<JsonObject>>,
        id: Option<Id>,
    ) -> Self {
        let mut bbox = BBox::default();
        let mut point_count = 0;
        geometry.iter_each_point(|point| {
            bbox.min_x = bbox.min_x.min(point.x);
            bbox.max_x = bbox.max_x.max(point.x);
            bbox.min_y = bbox.min_y.min(point.y);
            bbox.max_y = bbox.max_y.max(point.y);
            point_count += 1;
        });
        VtFeature {
            id,
            geometry,
            properties,
            bbox: if bbox.is_empty() { None } else { Some(bbox) },
            point_count,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BBox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}
impl BBox {
    pub fn merge(&mut self, other: &Self) {
        self.min_x = self.min_x.min(other.min_x);
        self.min_y = self.min_y.min(other.min_y);
        self.max_x = self.max_x.max(other.max_x);
        self.max_y = self.max_y.max(other.max_y);
    }
    pub fn is_empty(&self) -> bool {
        self.min_x.is_infinite()
            && self.min_y.is_infinite()
            && self.max_x.is_infinite()
            && self.max_y.is_infinite()
    }
}
impl Default for BBox {
    fn default() -> Self {
        Self {
            min_x: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            min_y: f64::INFINITY,
            max_y: f64::NEG_INFINITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VtGeometry {
    // Empty,
    Point(VtPoint),
    MultiPoint(VtMultiPoint),
    LineString(VtLineString),
    MultiLineString(VtMultiLineString),
    Polygon(VtPolygon),
    MultiPolygon(VtMultiPolygon),
    GeometryCollection(VtGeometryCollection),
}
impl VtGeometry {
    pub fn iter_each_point(&mut self, mut f: impl FnMut(&mut VtPoint)) {
        let mut f = &mut f as &mut dyn FnMut(&mut VtPoint);
        match self {
            VtGeometry::Point(p) => f(p),
            VtGeometry::MultiPoint(ps) => ps.iter_mut().for_each(&mut f),
            VtGeometry::LineString(ls) => ls.elements.iter_mut().for_each(&mut f),
            VtGeometry::MultiLineString(mls) => mls
                .iter_mut()
                .flat_map(|ls| ls.elements.iter_mut())
                .for_each(&mut f),
            VtGeometry::Polygon(poly) => poly
                .iter_mut()
                .flat_map(|ring| ring.elements.iter_mut())
                .for_each(&mut f),
            VtGeometry::MultiPolygon(mploy) => mploy
                .iter_mut()
                .flat_map(|poly| poly.iter_mut().flat_map(|ring| ring.elements.iter_mut()))
                .for_each(&mut f),
            VtGeometry::GeometryCollection(gc) => {
                gc.iter_mut().for_each(|g| g.iter_each_point(&mut f))
            }
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct VtPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl VtPoint {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn from_xy(x: f64, y: f64) -> Self {
        Self { x, y, z: 0. }
    }
}
#[derive(Default, Debug, Clone, PartialEq)]
pub struct VtLineString {
    pub elements: Vec<VtPoint>,
    pub dist: f64,
    pub seg_start: f64,
    pub seg_end: f64,
}
impl VtLineString {
    #[cfg(test)]
    pub fn from_slice(slice: &[VtPoint]) -> Self {
        Self {
            elements: Vec::from(slice),
            dist: 0.0, // line length
            seg_start: 0.0,
            seg_end: 0.0, // seg_start and seg_end are distance along a line in tile units, when lineMetrics = true
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq)]
pub struct VtLinearRing {
    pub elements: Vec<VtPoint>,
    pub area: f64,
}
#[cfg(test)]
impl VtLinearRing {
    pub fn from_slice(points: &[VtPoint]) -> Self {
        Self {
            elements: Vec::from(points),
            area: 0.0,
        }
    }
}

pub type VtPolygon = Vec<VtLinearRing>;
pub type VtMultiPoint = Vec<VtPoint>;
pub type VtMultiLineString = Vec<VtLineString>;
pub type VtMultiPolygon = Vec<VtPolygon>;
pub type VtGeometryCollection = Vec<VtGeometry>;

pub fn get_bbox_range<const I: usize>(bbox: &BBox) -> (f64, f64) {
    match I {
        0 => (bbox.min_x, bbox.max_x),
        1 => (bbox.min_y, bbox.max_y),
        _ => panic!("get_bbox_range is only implemented for I = 0 and I = 1"),
    }
}

pub fn get_coordinate<const I: usize>(p: &VtPoint) -> f64 {
    match I {
        0 => p.x,
        1 => p.y,
        _ => panic!("get_coordinate is only implemented for I = 0 and I = 1"),
    }
}

pub fn calc_progress<const I: usize>(a: &VtPoint, b: &VtPoint, v: f64) -> f64 {
    match I {
        0 => (v - a.x) / (b.x - a.x),
        1 => (v - a.y) / (b.y - a.y),
        _ => panic!("calc_progress is only implemented for I = 0 and I = 1"),
    }
}

pub fn intersect<const I: usize>(a: &VtPoint, b: &VtPoint, v: f64, t: f64) -> VtPoint {
    match I {
        0 => VtPoint::new(v, a.y + t * (b.y - a.y), 1.),
        1 => VtPoint::new(a.x + t * (b.x - a.x), v, 1.),
        _ => panic!("intersect is only implemented for I = 0 and I = 1"),
    }
}
