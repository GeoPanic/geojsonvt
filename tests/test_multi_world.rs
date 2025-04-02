use geojson::{Feature, FeatureCollection, GeoJson, Geometry, Value};
use geojson_vt::{GeoJSONVT, Options, VtGeometry, VtPoint};
#[test]
fn test_multi_world() {
    let left_point = GeoJson::Feature(Feature {
        id: None,
        geometry: Some(Geometry {
            bbox: None,
            foreign_members: None,
            value: Value::Point(vec![-540.0, 0.0]),
        }),
        properties: None,
        bbox: None,
        foreign_members: None,
    });
    let right_point = GeoJson::Feature(Feature {
        id: None,
        geometry: Some(Geometry {
            bbox: None,
            foreign_members: None,
            value: Value::Point(vec![540.0, 0.0]),
        }),
        properties: None,
        bbox: None,
        foreign_members: None,
    });
    let vt = &mut GeoJSONVT::from_geojson(&right_point, &Options::default());
    let g = &vt.internal_tiles().get(&0).unwrap().source_feature[0].geometry;

    match g {
        VtGeometry::Point(p) => {
            assert_eq!(
                p,
                &VtPoint {
                    x: 1.,
                    y: 0.5,
                    z: 0.
                }
            );
        }
        _ => {
            panic!("not a point");
        }
    }
    let vt = &mut GeoJSONVT::from_geojson(&left_point, &Options::default());
    let g = &vt.internal_tiles().get(&0).unwrap().source_feature[0].geometry;

    match g {
        VtGeometry::Point(p) => {
            assert_eq!(
                p,
                &VtPoint {
                    x: 0.,
                    y: 0.5,
                    z: 0.
                }
            );
        }
        _ => {
            panic!("not a point");
        }
    }

    let GeoJson::Feature(f1) = left_point else {
        panic!("not a feature");
    };
    let GeoJson::Feature(f2) = right_point else {
        panic!("not a feature");
    };
    let fc = GeoJson::FeatureCollection(FeatureCollection::from_iter(vec![f1, f2].into_iter()));
    let vt = GeoJSONVT::from_geojson(&fc, &Options::default());
    let tile = vt.internal_tiles().get(&0).unwrap();
    match &tile.source_feature[0].geometry {
        VtGeometry::Point(p) => {
            assert_eq!(
                p,
                &VtPoint {
                    x: 0.,
                    y: 0.5,
                    z: 0.
                }
            );
        }
        _ => {
            panic!("not a point");
        }
    };
    match &tile.source_feature[1].geometry {
        VtGeometry::Point(p) => {
            assert_eq!(
                p,
                &VtPoint {
                    x: 1.,
                    y: 0.5,
                    z: 0.
                }
            );
        }
        _ => {
            panic!("not a point");
        }
    };
}
