use core::panic;
use geojson::{Feature, FeatureCollection, GeoJson};

use std::{
    collections::{HashMap, hash_map::Entry},
    rc::Rc,
};

use crate::{
    clip::clip,
    convert::convert,
    tile::{EMPTY_TILE, InternalTile, Tile, TileCoord},
    types::VtFeature,
    wrap::wrap,
};

#[derive(Debug, Copy, Clone)]
pub struct Options {
    pub max_zoom: u8,
    pub index_max_zoom: u8,
    pub index_max_points: u32,
    pub tolerance: f64,
    pub extent: u16,
    pub buffer: u16,
    pub line_metrics: bool,
    pub generate_id: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            max_zoom: 18,
            index_max_zoom: 5,
            index_max_points: 100000,
            tolerance: 3.,
            extent: 4096,
            buffer: 64,
            line_metrics: false,
            generate_id: false,
        }
    }
}
#[derive(Debug)]
pub struct GeoJSONVT {
    options: Options,
    tiles: HashMap<u64, InternalTile>,
    tile_coords: Vec<TileCoord>,
    total: u32,
    stats: HashMap<u8, u32>,
}

impl GeoJSONVT {
    pub fn from_geojson(geojson: &GeoJson, options: &Options) -> Self {
        let collection = geojson_to_feature_collection(geojson);
        Self::new(collection, *options)
    }
    pub fn new(features: FeatureCollection, options: Options) -> Self {
        assert!(options.max_zoom > 0 && options.max_zoom <= 24);
        let buffer = options.buffer as f64 / options.extent as f64;
        let tolerance =
            (options.tolerance / options.extent as f64) / (1u32 << options.max_zoom as u32) as f64;
        let vt_features = convert(features, tolerance, options.generate_id);
        let vt_features = wrap(vt_features, buffer, options.line_metrics);

        let mut geojsonvt: Self = Self {
            options,
            tiles: HashMap::new(),
            tile_coords: Vec::new(),
            total: 0,
            stats: HashMap::new(),
        };
        geojsonvt.split_tile(&vt_features, 0, 0, 0, 0, 0, 0);
        geojsonvt
    }
    pub fn tile(&mut self, z: u8, x: u32, y: u32) -> &Tile {
        if z > self.options.max_zoom {
            panic!("Requested zoom higher than maxZoom: {}", z);
        }
        let z2 = 1u32 << z;
        let x = ((x % z2) + z2) % z2;
        let id = to_id(z, x, y);
        if self.tiles.contains_key(&id) {
            return &self.tiles[&id].tile;
        }
        let parent = self.find_parent(z, x, y).unwrap();
        self.split_tile(
            &parent.source_feature.clone(),
            parent.z,
            parent.x,
            parent.y,
            z,
            x,
            y,
        );
        if self.tiles.contains_key(&id) {
            return &self.tiles[&id].tile;
        }
        &EMPTY_TILE
    }

    fn find_parent(&self, z: u8, x: u32, y: u32) -> Option<&InternalTile> {
        let mut z0 = z;
        let mut x0 = x;
        let mut y0 = y;
        let end = None;
        let mut parent = end;
        while (parent == end) && (z0 != 0) {
            z0 -= 1;
            x0 /= 2;
            y0 /= 2;
            parent = self.tiles.get(&to_id(z0, x0, y0));
        }
        parent
    }
    fn split_tile(
        &mut self,
        vt_features: &[Rc<VtFeature>],
        z: u8,
        x: u32,
        y: u32,
        cz: u8,
        cx: u32,
        cy: u32,
    ) {
        let z2 = (1u32 << z) as f64;
        let id = to_id(z, x, y);
        if let Entry::Vacant(entry) = self.tiles.entry(id) {
            let tolerance = if z == self.options.max_zoom {
                0.
            } else {
                self.options.tolerance / (z2 * self.options.extent as f64)
            };
            self.tile_coords.push(TileCoord::new(x, y, z));
            entry.insert(InternalTile::new(
                vt_features,
                z,
                x,
                y,
                self.options.extent,
                tolerance,
                self.options.line_metrics,
            ));
            self.stats.insert(
                z,
                if self.stats.contains_key(&z) {
                    self.stats[&z] + 1
                } else {
                    1
                },
            );
            self.total += 1;
        }

        let internal_tile = self.tiles.get_mut(&id).unwrap();
        if cz == 0u8 {
            if z == self.options.index_max_zoom
                || internal_tile.tile.point_count <= self.options.index_max_points
            {
                internal_tile.source_feature = vt_features.to_vec();
                return;
            }
        } else {
            if z == self.options.max_zoom {
                return;
            }
            if z == cz {
                internal_tile.source_feature = vt_features.to_vec();
                return;
            }
            let m = (1u32 << (cz - z)) as f64;
            let a = (cx as f64 / m).floor() as u32;
            let b = (cy as f64 / m).floor() as u32;
            if x != a || y != b {
                internal_tile.source_feature = vt_features.to_vec();
                return;
            }
        }
        internal_tile.source_feature.clear();
        if vt_features.is_empty() {
            return;
        }

        let p = 0.5 * self.options.buffer as f64 / self.options.extent as f64;
        let bbox = internal_tile.bbox;

        let left = clip::<0>(
            vt_features,
            (x as f64 - p) / z2,
            (x as f64 + 0.5 + p) / z2,
            bbox.min_x,
            bbox.max_x,
            self.options.line_metrics,
        );

        let left_top = clip::<1>(
            &left,
            (y as f64 - p) / z2,
            (y as f64 + 0.5 + p) / z2,
            bbox.min_y,
            bbox.max_y,
            self.options.line_metrics,
        );

        self.split_tile(&left_top, z + 1, x * 2, y * 2, cz, cx, cy);
        let left_bottom = clip::<1>(
            &left,
            (y as f64 + 0.5 - p) / z2,
            (y as f64 + 1. + p) / z2,
            bbox.min_y,
            bbox.max_y,
            self.options.line_metrics,
        );
        self.split_tile(&left_bottom, z + 1, x * 2, y * 2 + 1, cz, cx, cy);
        let right = clip::<0>(
            vt_features,
            (x as f64 + 0.5 - p) / z2,
            (x as f64 + 1. + p) / z2,
            bbox.min_x,
            bbox.max_x,
            self.options.line_metrics,
        );
        let right_top = clip::<1>(
            &right,
            (y as f64 - p) / z2,
            (y as f64 + 0.5 + p) / z2,
            bbox.min_y,
            bbox.max_y,
            self.options.line_metrics,
        );
        self.split_tile(&right_top, z + 1, x * 2 + 1, y * 2, cz, cx, cy);
        let right_bottom = clip::<1>(
            &right,
            (y as f64 + 0.5 - p) / z2,
            (y as f64 + 1. + p) / z2,
            bbox.min_y,
            bbox.max_y,
            self.options.line_metrics,
        );
        self.split_tile(&right_bottom, z + 1, x * 2 + 1, y * 2 + 1, cz, cx, cy);
    }

    pub fn internal_tiles(&self) -> &HashMap<u64, InternalTile> {
        &self.tiles
    }
    pub fn tile_coords(&self) -> &Vec<TileCoord> {
        &self.tile_coords
    }
    pub fn total(&self) -> u32 {
        self.total
    }
    pub fn stats(&self) -> &HashMap<u8, u32> {
        &self.stats
    }
}

#[inline]
fn to_id(z: u8, x: u32, y: u32) -> u64 {
    ((1u64 << z) * y as u64 + x as u64) * 32 + z as u64
}

fn geojson_to_feature_collection(geojson: &GeoJson) -> FeatureCollection {
    match geojson {
        GeoJson::Geometry(geom) => FeatureCollection {
            bbox: None,
            features: vec![Feature {
                bbox: None,
                geometry: Some(geom.clone()),
                id: None,
                properties: None,
                foreign_members: None,
            }],
            foreign_members: None,
        },
        GeoJson::Feature(feature) => FeatureCollection {
            bbox: None,
            features: vec![feature.clone()],
            foreign_members: None,
        },
        GeoJson::FeatureCollection(features) => features.clone(),
    }
}
