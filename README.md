# geojsonvt

**A Rust library for slicing GeoJSON data into vector tile on the fly.**

## Features

- Slices large GeoJSON data into vector tiles
- High performance Rust implementation

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
geojsonvt = "0.1.0"
```

## Usage

```rust
use geojson::GeoJson;
use geojsonvt::{GeoJSONVT, Options};
use std::fs;
use std::str::FromStr;

fn main() {
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
    let contents = fs::read_to_string(file_path).expect("Something went wrong reading the file");
    let geo_json = GeoJson::from_str(&contents).unwrap();
    let geojsonvt = GeoJSONVT::from_geojson(geo_json, &options);
    let tile = geojsonvt.get_tile(0, 0, 0);
}
```

## Run Example

```bash
cargo run --example main
```

## benchmark

| [geojson-vt](https://github.com/mapbox/geojson-vt) (JS) | [geojson-vt](https://github.com/mapbox/geojson-vt) (JS --jitless) | [geojson2vt](https://github.com/geometalab/geojson2vt) (Py) | [geojson-vt-rs](https://github.com/maxammann/geojson-vt-rs) (rs) | [geojsonvt](https://github.com/GeoPanic/geojsonvt) (rs,this repo) |
| ------------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------- |
| 2.563s                                                  | 15.147s                                                           | 29.943s                                                     | 5.286                                                            | **1.617s**                                                        |
