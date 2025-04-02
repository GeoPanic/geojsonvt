use std::rc::Rc;

use geojson::{
    Feature, FeatureCollection, Geometry, JsonObject, JsonValue, PointType, Position, Value,
    feature::Id,
};
use serde_json::Number;

use crate::types::{
    BBox, VtFeature, VtGeometry, VtLineString, VtLinearRing, VtMultiLineString, VtMultiPoint,
    VtMultiPolygon, VtPoint, VtPolygon,
};

pub static EMPTY_TILE: Tile = {
    Tile {
        feature_collection: FeatureCollection {
            bbox: None,
            foreign_members: None,
            features: vec![],
        },
        point_count: 0,
        simplified_count: 0,
    }
};
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Tile {
    pub feature_collection: FeatureCollection,
    pub point_count: u32,
    pub simplified_count: u32,
}

#[derive(Debug)]
pub struct InternalTile {
    pub x: u32,
    pub y: u32,
    pub z: u8,
    extent: u16,
    z2: f64,
    tolerance: f64,
    sq_tolerance: f64,
    line_metrics: bool,
    pub source_feature: Vec<Rc<VtFeature>>,
    pub bbox: BBox,
    pub tile: Tile,
}
impl PartialEq for InternalTile {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}
impl InternalTile {
    pub fn new(
        source_feature: &[Rc<VtFeature>],
        z: u8,
        x: u32,
        y: u32,
        extent: u16,
        tolerance: f64,
        line_metrics: bool,
    ) -> InternalTile {
        let mut tile = Self {
            x,
            y,
            z,
            extent,
            z2: (1u32 << z) as f64,
            tolerance,
            sq_tolerance: tolerance * tolerance,
            line_metrics,
            source_feature: vec![],
            bbox: Default::default(),
            tile: Tile {
                feature_collection: FeatureCollection::default(),
                point_count: 0,
                simplified_count: 0,
            },
        };
        for feature in source_feature {
            let geometry = &feature.geometry;
            let properties = &feature.properties;
            let id = &feature.id;
            tile.tile.point_count += &feature.point_count;
            tile.add_feature(geometry, properties, id);
            if let Some(bbox) = &feature.bbox {
                tile.bbox.merge(bbox);
            }
        }
        tile
    }

    pub fn add_feature(
        &mut self,
        geometry: &VtGeometry,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        match geometry {
            VtGeometry::Point(value) => {
                self.add_point(value, properties, id);
            }
            VtGeometry::MultiPoint(value) => {
                self.add_multi_point(value, properties, id);
            }
            VtGeometry::LineString(value) => self.add_line_string(value, properties, id),
            VtGeometry::MultiLineString(value) => self.add_multi_line_string(value, properties, id),
            VtGeometry::Polygon(value) => self.add_polygon(value, properties, id),
            VtGeometry::MultiPolygon(value) => self.add_multi_polygon(value, properties, id),
            VtGeometry::GeometryCollection(value) => {
                self.add_geometry_collection(value, properties, id)
            }
        }
    }
    fn add_point(&mut self, value: &VtPoint, properties: &Option<JsonObject>, id: &Option<Id>) {
        let coords = self.transform_point(value);
        let geometry = Some(Geometry::new(Value::Point(coords)));
        self.tile.feature_collection.features.push(Feature {
            bbox: None,
            geometry,
            id: id.clone(),
            properties: properties.as_ref().map(|p| p.clone()),
            // TODO: Avoid clone
            // properties: None,
            foreign_members: None,
        });
    }
    fn add_multi_point(
        &mut self,
        points: &VtMultiPoint,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        let multi_coords: Vec<Position> = points.iter().map(|p| self.transform_point(p)).collect();

        match multi_coords.len() {
            0 => (),
            1 => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::Point(multi_coords[0].clone()))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
            _ => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::MultiPoint(multi_coords))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
        }
    }
    fn add_line_string(
        &mut self,
        line: &VtLineString,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        let coords = self.transform_line_string(line);
        if coords.is_empty() {
            return;
        }
        if self.line_metrics {
            let mut new_properties = properties.as_ref().map(|p| p.clone()).unwrap_or_default();
            let start = line.seg_start / line.dist;
            new_properties.insert(
                "mapbox_clip_start".to_string(),
                if start.fract() == 0.0 {
                    JsonValue::Number(Number::from(start as i64))
                } else {
                    JsonValue::Number(Number::from_f64(start).unwrap())
                },
            );
            let end = line.seg_end / line.dist;
            new_properties.insert(
                "mapbox_clip_end".to_string(),
                if end.fract() == 0.0 {
                    JsonValue::Number(Number::from(end as i64))
                } else {
                    JsonValue::Number(Number::from_f64(end).unwrap())
                },
            );
            self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::LineString(coords))),
                id: id.clone(),
                // properties: None,
                properties: Some(new_properties),
                foreign_members: None,
            });
        } else {
            self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::LineString(coords))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            });
        }
    }
    fn add_multi_line_string(
        &mut self,
        multi_lines: &VtMultiLineString,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        let tolerance = self.tolerance;
        let multi_coords: Vec<_> = multi_lines
            .iter()
            .filter(|line| line.dist > tolerance)
            .map(|line| self.transform_line_string(line))
            .collect();
        match multi_coords.len() {
            0 => (),
            1 => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::LineString(multi_coords[0].clone()))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
            _ => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::MultiLineString(multi_coords))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
        }
    }
    fn add_polygon(
        &mut self,
        polygon: &VtPolygon,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        let coords = self.transform_polygon(polygon);
        if !coords.is_empty() {
            self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::Polygon(coords))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            })
        }
    }
    fn add_multi_polygon(
        &mut self,
        polygons: &VtMultiPolygon,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        let multi_coords: Vec<_> = polygons
            .iter()
            .filter_map(|polygon| {
                let coords = self.transform_polygon(polygon);
                if !coords.is_empty() {
                    Some(coords)
                } else {
                    None
                }
            })
            .collect();
        match multi_coords.len() {
            0 => (),
            1 => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::Polygon(multi_coords[0].clone()))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
            _ => self.tile.feature_collection.features.push(Feature {
                bbox: None,
                geometry: Some(Geometry::new(Value::MultiPolygon(multi_coords))),
                id: id.clone(),
                // properties: None,
                properties: properties.as_ref().map(|p| p.clone()),
                foreign_members: None,
            }),
        }
    }
    fn add_geometry_collection(
        &mut self,
        geometries: &Vec<VtGeometry>,
        properties: &Option<JsonObject>,
        id: &Option<Id>,
    ) {
        for geometry in geometries {
            self.add_feature(geometry, properties, id);
        }
    }

    fn transform_point(&mut self, p: &VtPoint) -> PointType {
        self.tile.simplified_count += 1;
        let x = ((p.x * self.z2 - self.x as f64) * self.extent as f64).round();
        let y = ((p.y * self.z2 - self.y as f64) * self.extent as f64).round();
        vec![x, y]
    }
    fn transform_line_string(&mut self, line: &VtLineString) -> Vec<Position> {
        if line.dist < self.tolerance {
            return vec![];
        }
        let tolerance = self.tolerance;
        line.elements
            .iter()
            .filter(|p| p.z > tolerance)
            .map(|p| self.transform_point(p))
            .collect()
    }
    fn transform_line_ring(&mut self, ring: &VtLinearRing) -> Vec<Position> {
        if ring.area < self.sq_tolerance {
            return vec![];
        }
        let sq_tolerance = self.sq_tolerance;
        ring.elements
            .iter()
            .filter(|p| p.z > sq_tolerance)
            .map(|p| self.transform_point(p))
            .collect()
    }
    fn transform_polygon(&mut self, rings: &VtPolygon) -> Vec<Vec<Position>> {
        let sq_tolerance = self.sq_tolerance;
        rings
            .iter()
            .filter(|ring| ring.area > sq_tolerance)
            .map(|ring| self.transform_line_ring(ring))
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileCoord {
    x: u32,
    y: u32,
    z: u8,
}

impl TileCoord {
    pub fn new(x: u32, y: u32, z: u8) -> TileCoord {
        TileCoord { x, y, z }
    }
}
