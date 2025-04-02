use geojson::GeoJson;
use geojson_vt::{GeoJSONVT, Options};
use std::fs;
use std::str::FromStr;
use std::time::Instant;
fn read_geo_json(file_path: &str) -> GeoJson {
    let contents = fs::read_to_string(file_path).expect("Something went wrong reading the file");
    let now = Instant::now();
    let geo_json = GeoJson::from_str(&contents).unwrap();
    let end = now.elapsed().as_millis();
    println!("parse: {:?}mm", end);
    geo_json
}

fn process_geojson(geo_json: &GeoJson) {
    let options = Options {
        max_zoom: 16,
        index_max_zoom: 16,
        index_max_points: 1000,
        generate_id: false,
        tolerance: 16.0,
        extent: 4096,
        buffer: 64,
        line_metrics: false,
    };
    let now = Instant::now();
    let geojsonvt = GeoJSONVT::from_geojson(geo_json, &options);
    let end = now.elapsed().as_millis();
    println!("cost: {:?}mm", end);
    println!("total: {}", geojsonvt.total());
    println! {"stat: {:?}", geojsonvt.stats()};
}
fn main() {
    let geo_json = read_geo_json(r#"examples/usa_zip_codes_geo_100m.json"#);
    process_geojson(&geo_json);
}
