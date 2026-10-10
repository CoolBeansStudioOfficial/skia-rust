// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp (chrome/m156)
//
// Shapes (paths) are encoded as a vector of floats: see [`crate::skottie_value::ShapeValue`].

use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie_json::{Parse, ValueExt, parse_default};
use crate::skottie_value::{
    FLOATS_PER_VERTEX, IN_X_INDEX, IN_Y_INDEX, OUT_X_INDEX, OUT_Y_INDEX, X_INDEX, Y_INDEX,
};

// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L46-L48 (chrome/m156) (`shape_encoding_len`)
fn shape_encoding_len(vertex_count: usize) -> usize {
    vertex_count * FLOATS_PER_VERTEX + 1
}

/// Some versions wrap shape values as single-element arrays.
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L50-L58 (chrome/m156) (`shape_root`)
fn shape_root(jv: &Value) -> Option<&ObjectValue> {
    if let Some(av) = jv.as_array()
        && av.size() == 1
    {
        return av[0].as_object();
    }

    jv.as_object()
}

/// Parses the number of floats of the encoding of the shape `jv`.
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L60-L69 (chrome/m156) (`parse_encoding_len`)
pub(crate) fn parse_encoding_len(jv: &Value, len: &mut usize) -> bool {
    if let Some(jshape) = shape_root(jv)
        && let Some(jvs) = jshape.get("v").as_array()
    {
        *len = shape_encoding_len(jvs.size());
        return true;
    }
    false
}

/// Parses a point of an array of points.
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L89-L99 (chrome/m156) (`parse_point`)
fn parse_point(ja: &ArrayValue, i: usize, x: &mut f32, y: &mut f32) -> bool {
    let Some(jpt) = ja[i].as_array() else {
        return false;
    };

    if jpt.size() != 2 {
        return false;
    }

    f32::parse(&jpt[0], x) && f32::parse(&jpt[1], y)
}

/// Parses a point of an optional array of points: absent points are the default control point.
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L101-L111 (chrome/m156) (`parse_optional_point`)
fn parse_optional_point(ja: Option<&ArrayValue>, i: usize, x: &mut f32, y: &mut f32) -> bool {
    match ja {
        Some(ja) if i < ja.size() => parse_point(ja, i, x, y),
        _ => {
            // default control point
            *x = 0.0;
            *y = 0.0;
            true
        }
    }
}

/// Parses the encoding of the shape `jv` into `data`.
// Port of: modules/skottie/src/animator/ShapeKeyframeAnimator.cpp#L71-L135 (chrome/m156) (`parse_encoding_data`)
pub(crate) fn parse_encoding_data(jv: &Value, data_len: usize, data: &mut [f32]) -> bool {
    let Some(jshape) = shape_root(jv) else {
        return false;
    };

    // vertices are required, in/out tangents are optional
    let jvs = jshape.get("v").as_array(); // vertex points
    let jis = jshape.get("i").as_array(); // in-tangent points
    let jos = jshape.get("o").as_array(); // out-tangent points

    let Some(jvs) = jvs else {
        return false;
    };
    if data_len != shape_encoding_len(jvs.size()) {
        return false;
    }

    for i in 0..jvs.size() {
        let dst = &mut data[i * FLOATS_PER_VERTEX..(i + 1) * FLOATS_PER_VERTEX];
        debug_assert!((i + 1) * FLOATS_PER_VERTEX <= data_len);

        let (mut x, mut y) = (0.0, 0.0);
        let mut ok = parse_point(jvs, i, &mut x, &mut y);
        dst[X_INDEX] = x;
        dst[Y_INDEX] = y;
        if !ok {
            return false;
        }

        ok = parse_optional_point(jis, i, &mut x, &mut y);
        dst[IN_X_INDEX] = x;
        dst[IN_Y_INDEX] = y;
        if !ok {
            return false;
        }

        ok = parse_optional_point(jos, i, &mut x, &mut y);
        dst[OUT_X_INDEX] = x;
        dst[OUT_Y_INDEX] = y;
        if !ok {
            return false;
        }
    }

    // "closed" flag
    data[data_len - 1] = f32::from(u8::from(parse_default::<bool>(jshape.get("c"), false)));

    true
}
