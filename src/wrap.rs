use std::rc::Rc;

use crate::{clip::clip, types::VtFeature};

fn into_rc_features(features: Vec<VtFeature>) -> Vec<Rc<VtFeature>> {
    features.into_iter().map(Rc::new).collect::<Vec<_>>()
}
fn into_owned_features(features: Vec<Rc<VtFeature>>) -> Vec<VtFeature> {
    features
        .into_iter()
        .map(|f| (*f).clone())
        .collect::<Vec<_>>()
}
pub fn wrap(features: Vec<VtFeature>, buffer: f64, line_metrics: bool) -> Vec<Rc<VtFeature>> {
    let features = into_rc_features(features);
    let left = clip::<0>(&features, -1. - buffer, buffer, -1., 2., line_metrics);
    let right = clip::<0>(&features, 1. - buffer, 2. + buffer, -1., 2., line_metrics);
    let mut left = into_owned_features(left);
    let mut right = into_owned_features(right);

    if left.is_empty() && right.is_empty() {
        return features;
    };

    let mut merged = clip::<0>(&features, -buffer, 1. + buffer, 1., 2., line_metrics);

    if !left.is_empty() {
        shift_coords(&mut left, 1.0);
        let left = into_rc_features(left);
        // merged.extend(left);
        merged.splice(0..0, left);
    }
    if !right.is_empty() {
        shift_coords(&mut right, -1.0);
        let right = into_rc_features(right);
        merged.extend(right);
    }
    merged
}

pub fn shift_coords(features: &mut [VtFeature], offset: f64) {
    features.iter_mut().for_each(|f| {
        // f.bbox
        if let Some(bbox) = &mut f.bbox {
            bbox.min_x += offset;
            bbox.max_x += offset;
        }
        f.geometry.iter_each_point(|p| p.x += offset);
    });
    //
}
