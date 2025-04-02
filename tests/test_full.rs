use approx::AbsDiffEq;
use geojson::feature::Id;
use geojson::{
    Feature, FeatureCollection, GeoJson, Geometry, JsonValue, PointType, PolygonType, Position,
};
use geojsonvt::{GeoJSONVT, Options};
use serde_json::{Number, Value};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::fs::{self, File};
use std::io::BufReader;
use std::str::FromStr;
#[test]
fn test_get_tile_us_state() {
    let geojson = GeoJson::from_reader(BufReader::new(
        File::open("tests/fixtures/us-states.json").unwrap(),
    ))
    .unwrap();
    let mut geojsonvt = GeoJSONVT::from_geojson(&geojson, &Options::default());
    let features = &geojsonvt.tile(7, 37, 48).feature_collection;
    let expected = parse_json_tile(
        serde_json::from_reader(File::open("tests/fixtures/us-states-z7-37-48.json").unwrap())
            .unwrap(),
    );
    assert_eq!(features, &expected);
}
#[test]
fn test_get_tile_generated_ids() {
    let geojson = GeoJson::from_reader(BufReader::new(
        File::open("tests/fixtures/us-states.json").unwrap(),
    ))
    .unwrap();
    let mut geojsonvt = GeoJSONVT::from_geojson(
        &geojson,
        &Options {
            max_zoom: 20,
            generate_id: true,
            ..Options::default()
        },
    );
    let features = &geojsonvt.tile(7, 37, 48).feature_collection;
    let expected = parse_json_tile(
        serde_json::from_reader(
            File::open("tests/fixtures/us-states-z7-37-48-gen-ids.json").unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        features.features.first().unwrap().id,
        Some(Id::Number(Number::from(6)))
    );
    assert_eq!(
        features.features.first().unwrap().id,
        Some(Id::Number(Number::from(6)))
    );
    assert_eq!(features, &expected);
}

#[test]
fn test_get_tile_antimerdian_triangle() {
    let geojson = GeoJson::from_reader(BufReader::new(
        File::open("tests/fixtures/dateline-triangle.json").unwrap(),
    ))
    .unwrap();
    let mut geojsonvt = GeoJSONVT::from_geojson(&geojson, &Options::default());
    let coords = vec![
        (1, 0, 0), // , (1, 0, 1), (1, 1, 0), (1, 1, 1)
    ];
    for (z, x, y) in coords {
        let tile = geojsonvt.tile(z, x, y);
        assert_eq!(tile.point_count, tile.simplified_count);
        assert_eq!(tile.feature_collection.features.len(), 1);
    }
}

#[test]
fn test_get_tile_polygon_clipping_bug() {
    let geojson = GeoJson::from_reader(BufReader::new(
        File::open("tests/fixtures/polygon-bug.json").unwrap(),
    ))
    .unwrap();
    let mut geojsonvt = GeoJSONVT::from_geojson(
        &geojson,
        &Options {
            buffer: 1024,
            ..Options::default()
        },
    );
    let tile = geojsonvt.tile(5, 19, 9);
    assert_eq!(tile.feature_collection.features.len(), 1);
    assert_eq!(tile.point_count, 5);
    let expected = Geometry::new(geojson::Value::Polygon(PolygonType::from(&[vec![
        PointType::from(&[3072., 3072.]),
        PointType::from(&[5120., 3072.]),
        PointType::from(&[5120., 5120.]),
        PointType::from(&[3072., 5120.]),
        PointType::from(&[3072., 3072.]),
    ]])));
    let actual = tile.feature_collection.features[0]
        .geometry
        .as_ref()
        .unwrap();
    assert_eq!(actual, &expected);
}

#[test]
fn test_get_tile_projection() {
    let geojson = GeoJson::from_reader(BufReader::new(
        File::open("tests/fixtures/linestring.json").unwrap(),
    ))
    .unwrap();

    let mut geojsonvt = GeoJSONVT::from_geojson(
        &geojson,
        &Options {
            max_zoom: 20,
            extent: 8192,
            tolerance: 0.,
            ..Options::default()
        },
    );
    let coords = vec![
        (0, 0, 0),
        (1, 0, 0),
        (2, 0, 1),
        (3, 1, 3),
        (4, 2, 6),
        (5, 5, 12),
        (6, 10, 24),
        (7, 20, 49),
        (8, 40, 98),
        (9, 81, 197),
        (10, 163, 395),
        (11, 327, 791),
        (12, 655, 1583),
        (13, 1310, 3166),
        (14, 2620, 6332),
        (15, 5241, 12664),
        (16, 10482, 25329),
        (17, 20964, 50660),
        (18, 41929, 101320),
        (19, 83859, 202640),
        (20, 167719, 405281),
    ];
    for (z, x, y) in coords {
        let tile = geojsonvt.tile(z, x, y);
        assert_eq!(tile.point_count, tile.simplified_count);
        assert_eq!(tile.feature_collection.features.len(), 1);
        let geometry = &tile.feature_collection.features[0]
            .geometry
            .as_ref()
            .unwrap()
            .value;
        let line_string = match geometry {
            geojson::Value::LineString(ls) => ls,
            _ => panic!("geometry not a line string"),
        };

        assert_eq!(line_string.len(), 2);
        let total_features = (1u32 << z) as f64 * 8192.;
        let to_web_mercator_lon = |point: &Position| {
            let x0 = 8192.0 * x as f64;
            return (x0 + point[0]) * 360.0 / total_features - 180.0;
        };

        let to_web_mercator_lat = |point: &Position| {
            let y0 = 8192.0 * y as f64;
            let y2 = 180.0 - (y0 + point[1]) * 360.0 / total_features;
            return 360.0 / PI * (y2 * PI / 180.0).exp().atan() - 90.0;
        };
        let tolerance = 0.1 / (1. + z as f64);
        assert!(
            (-122.41822421550751f64).abs_diff_eq(&to_web_mercator_lon(&line_string[0]), tolerance)
        );
        assert!(37.77852514599172f64.abs_diff_eq(&to_web_mercator_lat(&line_string[0]), tolerance));

        assert!(
            (-122.41707086563109f64).abs_diff_eq(&to_web_mercator_lon(&line_string[1]), tolerance)
        );
        assert!(
            37.780424620898664f64.abs_diff_eq(&to_web_mercator_lat(&line_string[1]), tolerance)
        );
    }
}

#[test]
fn test_tiles() {
    let cases = [
        (
            "tests/fixtures/us-states.json",
            "tests/fixtures/us-states-tiles.json",
            7,
            200,
            false,
        ),
        (
            "tests/fixtures/dateline.json",
            "tests/fixtures/dateline-tiles.json",
            7,
            200,
            false,
        ),
        (
            "tests/fixtures/dateline.json",
            "tests/fixtures/dateline-metrics-tiles.json",
            0,
            10000,
            true,
        ),
        (
            "tests/fixtures/feature.json",
            "tests/fixtures/feature-tiles.json",
            0,
            10000,
            false,
        ),
        (
            "tests/fixtures/collection.json",
            "tests/fixtures/collection-tiles.json",
            0,
            10000,
            false,
        ),
        (
            "tests/fixtures/single-geom.json",
            "tests/fixtures/single-geom-tiles.json",
            0,
            10000,
            false,
        ),
    ];
    for (input_file, expected_file, max_zoom, max_points, line_metrics) in cases {
        let data = fs::read_to_string(input_file).unwrap();
        let mut actual = gen_tiles(&data, max_zoom, max_points, line_metrics);
        let expected =
            parse_json_tiles(serde_json::from_reader(File::open(expected_file).unwrap()).unwrap());
        for (_key, value) in &mut actual {
            value.features = value
                .features
                .iter()
                .map(|feature| Feature {
                    bbox: feature.bbox.clone(),
                    geometry: feature.geometry.clone().map(|geom| {
                        Geometry::new(match geom.value {
                            geojson::Value::MultiPolygon(multi) => geojson::Value::Polygon(
                                multi.iter().flatten().cloned().collect::<Vec<_>>(),
                            ),
                            v => v,
                        })
                    }),
                    id: feature.id.clone(),
                    properties: feature.properties.clone(),
                    foreign_members: feature.foreign_members.clone(),
                })
                .collect();
        }

        assert_eq!(actual, expected);
    }
}

fn gen_tiles(
    data: &str,
    max_zoom: u8,
    max_points: u32,
    line_metrics: bool,
) -> HashMap<String, FeatureCollection> {
    let geojson = GeoJson::from_str(data).unwrap();
    let mut geojsonvt = GeoJSONVT::from_geojson(
        &geojson,
        &Options {
            max_zoom: 14,
            index_max_points: max_points,
            index_max_zoom: max_zoom,
            line_metrics,
            ..Options::default()
        },
    );

    let mut output = HashMap::new();
    let tile_coords: Vec<_> = geojsonvt
        .internal_tiles()
        .iter()
        .map(|(_key, tile)| (tile.z, tile.x, tile.y))
        .collect();

    for (z, x, y) in tile_coords {
        let key = format!("z{}-{}-{}", z, x, y);
        output.insert(key, geojsonvt.tile(z, x, y).feature_collection.clone());
    }
    output
}
fn parse_json_tiles(tiles: JsonValue) -> HashMap<String, FeatureCollection> {
    let Value::Object(tiles) = tiles else {
        panic!("not a valid tiles file");
    };
    tiles
        .into_iter()
        .map(|(key, value)| (key, parse_json_tile(value)))
        .collect()
}
fn parse_json_tile(tile: JsonValue) -> FeatureCollection {
    let mut features = Vec::new();
    assert!(matches!(tile, JsonValue::Array(_)));

    let JsonValue::Array(tile_features) = tile else {
        panic!("tile not an array")
    };
    for feature in tile_features {
        let mut feat = Feature::default();
        if let Some(JsonValue::Object(properties)) = &feature.get("tags") {
            // feat.properties = if properties.is_empty() {
            //     None
            // } else {
            //     Some(properties.clone())
            // }
            feat.properties = Some(properties.clone());
        };
        if let Some(JsonValue::String(tile_id)) = feature.get("id") {
            feat.id = Some(Id::String(tile_id.clone()));
        }
        if let Some(JsonValue::Number(tile_id)) = feature.get("id") {
            feat.id = Some(Id::Number(tile_id.clone()));
        }
        if let (Some(JsonValue::Number(tile_type)), Some(JsonValue::Array(tile_geom))) =
            (feature.get("type"), feature.get("geometry"))
        {
            let geom_type = tile_type.as_u64().unwrap();
            if geom_type == 1 {
                if tile_geom.len() == 1 {
                    let pt = tile_geom.first().unwrap();
                    assert_eq!(pt.as_array().unwrap().len(), 2);
                    feat.geometry = Some(Geometry::new(geojson::Value::Point(PointType::from(&[
                        pt.get(0).unwrap().as_f64().unwrap(),
                        pt.get(1).unwrap().as_f64().unwrap(),
                    ]))))
                } else {
                    let mut points = vec![];
                    for pt in tile_geom {
                        points.push(PointType::from(&[
                            pt.get(0).unwrap().as_f64().unwrap(),
                            pt.get(1).unwrap().as_f64().unwrap(),
                        ]))
                    }
                    feat.geometry = Some(Geometry::new(geojson::Value::MultiPoint(points)))
                }
            } else if geom_type == 2 {
                // linestring geometry
                let mut multi_line: Vec<Vec<Position>> = Vec::new();
                let is_multi = tile_geom.len() > 1;
                for part in tile_geom {
                    let mut line_string: Vec<Position> = Vec::new();
                    for pt in part.as_array().unwrap() {
                        assert_eq!(pt.as_array().unwrap().len(), 2);
                        line_string.push(PointType::from(&[
                            pt.get(0).unwrap().as_f64().unwrap(),
                            pt.get(1).unwrap().as_f64().unwrap(),
                        ]));
                    }
                    if !is_multi {
                        feat.geometry =
                            Some(Geometry::new(geojson::Value::LineString(line_string)));
                        break;
                    } else {
                        multi_line.push(line_string);
                    }
                }

                if is_multi {
                    feat.geometry =
                        Some(Geometry::new(geojson::Value::MultiLineString(multi_line)));
                }
            } else if geom_type == 3 {
                // polygon geometry
                let mut poly: PolygonType = Vec::new();
                for ring in tile_geom {
                    let mut linear_ring: Vec<PointType> = Vec::new();
                    for pt in ring.as_array().unwrap() {
                        assert_eq!(pt.as_array().unwrap().len(), 2);
                        linear_ring.push(PointType::from(&[
                            pt.get(0).unwrap().as_f64().unwrap(),
                            pt.get(1).unwrap().as_f64().unwrap(),
                        ]));
                    }
                    poly.push(linear_ring);
                }
                feat.geometry = Some(Geometry::new(geojson::Value::Polygon(poly)));
            } else {
                panic!("unknown geometry type")
            }
        }
        features.push(feat);
    }
    return FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };
}
