// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Sanity tests of the path effects that are not ports of Skia tests.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::dash_path_effect::new;

fn stroke_paint(width: scalar) -> Paint {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(width);
    paint
}

fn stroke_rec(width: scalar) -> StrokeRec {
    StrokeRec::from_paint(&stroke_paint(width), None, None)
}

// The path-effect part of what `DashPathEffectTest_asPoints_limit` does through
// `Canvas::drawLine` (which needs D6): a huge stroke width makes the cull rect outset by a
// large amount, and the dash count must stay bounded.
#[test]
fn long_line_with_many_dashes_is_bounded() {
    let dash = new(&[1.0, 1.0], 0.0).unwrap();
    let path = Path::line((1.0, 1.0), (1.0, 5.0e10));
    let cull = Rect::from_wh(256.0, 256.0);
    let mut builder = PathBuilder::new();
    let mut paint = stroke_paint(5.0e10);
    paint.set_path_effect(dash);
    let _ = fill_path_with_paint(&path, &paint, &mut builder, Some(&cull), None);
}

#[test]
fn dashes_a_horizontal_line() {
    let dash = new(&[2.0, 2.0], 0.0).unwrap();
    let path = Path::line((0.0, 0.0), (10.0, 0.0));
    let (builder, rec) = dash
        .filter_path_with_matrix(&path, &stroke_rec(1.0), None, Matrix::i())
        .unwrap();
    // The special line fast path strokes the dashes itself.
    assert!(rec.is_fill_style());
    let dashed = builder.snapshot();
    // dashes [0,2], [4,6] and [8,10]: three quads of four points
    assert_eq!(dashed.count_points(), 12);
    assert_eq!(dashed.bounds(), &Rect::new(0.0, -0.5, 10.0, 0.5));
}

#[test]
fn invalid_dashes_are_rejected() {
    assert!(new(&[], 0.0).is_none());
    assert!(new(&[1.0], 0.0).is_none());
    assert!(new(&[1.0, -1.0], 0.0).is_none());
    assert!(new(&[0.0, 0.0], 0.0).is_none());
    assert!(new(&[1.0, 1.0], scalar::NAN).is_none());
}

#[test]
fn sum_and_compose_filter() {
    let dash = new(&[2.0, 2.0], 0.0).unwrap();
    let sum = PathEffect::sum(dash.clone(), dash.clone());
    let path = Path::line((0.0, 0.0), (10.0, 0.0));
    let (builder, _) = sum
        .filter_path_with_matrix(&path, &stroke_rec(1.0), None, Matrix::i())
        .unwrap();
    // The first dash turns the shared rec into a fill, so the second one does not apply.
    assert_eq!(builder.snapshot().count_points(), 12);
    assert!(PathEffect::compose(dash.clone(), dash).compute_fast_bounds(None));
}

// Trimming returns the requested span of a line: the first half, and (inverted) the complement
// of the second half, which is the first half again.
#[test]
fn trim_keeps_the_requested_span() {
    let path = Path::line((0.0, 0.0), (100.0, 0.0));
    let first_half = crate::trim_path_effect::new(0.0, 0.5, None).unwrap();
    let (builder, _) = first_half
        .filter_path(&path, &stroke_rec(1.0), None)
        .unwrap();
    assert_eq!(builder.snapshot().bounds(), &Rect::new(0.0, 0.0, 50.0, 0.0));

    let inverted =
        crate::trim_path_effect::new(0.5, 1.0, crate::trim_path_effect::Mode::Inverted).unwrap();
    let (builder, _) = inverted.filter_path(&path, &stroke_rec(1.0), None).unwrap();
    assert_eq!(builder.snapshot().bounds(), &Rect::new(0.0, 0.0, 50.0, 0.0));
}

// Without deviation, the discrete effect moves no point off the line.
#[test]
fn discrete_without_deviation_stays_on_the_path() {
    let path = Path::line((0.0, 0.0), (100.0, 0.0));
    let effect = crate::discrete_path_effect::new(10.0, 0.0, None).unwrap();
    let (builder, _) = effect.filter_path(&path, &stroke_rec(1.0), None).unwrap();
    assert_eq!(
        builder.snapshot().bounds(),
        &Rect::new(0.0, 0.0, 100.0, 0.0)
    );
}

// Translate stamps the path once per advance: stamps at 0, 10, ..., 90 on a 100-long line.
#[test]
fn path_1d_translate_stamps_at_each_advance() {
    let stamp = Path::line((0.0, 0.0), (2.0, 0.0));
    let effect = crate::path_1d_path_effect::new(
        &stamp,
        10.0,
        0.0,
        crate::path_1d_path_effect::Style::Translate,
    )
    .unwrap();
    let path = Path::line((0.0, 0.0), (100.0, 0.0));
    let (builder, rec) = effect.filter_path(&path, &stroke_rec(1.0), None).unwrap();
    assert!(rec.is_fill_style());
    assert_eq!(builder.snapshot().bounds(), &Rect::new(0.0, 0.0, 92.0, 0.0));
}

#[test]
fn invalid_discrete_and_trim_are_rejected() {
    assert!(crate::discrete_path_effect::new(0.0, 1.0, None).is_none());
    assert!(crate::discrete_path_effect::new(scalar::NAN, 1.0, None).is_none());
    assert!(crate::trim_path_effect::new(0.0, 1.0, None).is_none());
    assert!(
        crate::trim_path_effect::new(0.5, 0.5, crate::trim_path_effect::Mode::Inverted).is_none()
    );
    assert!(crate::trim_path_effect::new(scalar::NAN, 0.5, None).is_none());
}

// Flattening: every effect that has a `flatten` reads back to an effect that flattens to the same
// bytes, through the registry of the effects.
mod flatten_round_trip {
    use skia_rust_core::blur_types::BlurStyle;
    use skia_rust_core::mask_filter::MaskFilter;
    use skia_rust_core::matrix::Matrix;
    use skia_rust_core::path_builder::PathBuilder;
    use skia_rust_core::path_effect::PathEffect;

    use crate::flattenable::REGISTRY;
    use crate::{
        corner_path_effect, dash_path_effect, discrete_path_effect, emboss_mask_filter,
        line_2d_path_effect, path_1d_path_effect, path_2d_path_effect, table_mask_filter,
        trim_path_effect,
    };

    fn assert_path_effect_round_trips(effect: &PathEffect) {
        let bytes = effect.serialize();
        let back = PathEffect::deserialize(bytes.as_bytes(), &REGISTRY)
            .expect("a registered effect reads back");
        assert_eq!(back.serialize().as_bytes(), bytes.as_bytes());
    }

    fn assert_mask_filter_round_trips(filter: &MaskFilter) {
        let bytes = filter.serialize();
        let back = MaskFilter::deserialize(bytes.as_bytes(), &REGISTRY)
            .expect("a registered filter reads back");
        assert_eq!(back.serialize().as_bytes(), bytes.as_bytes());
    }

    #[test]
    fn path_effects_round_trip() {
        let dash = dash_path_effect::new(&[1.0, 2.0], 0.5).expect("valid dash");
        assert_path_effect_round_trips(&dash);
        assert_path_effect_round_trips(&corner_path_effect::new(4.0).expect("valid corner"));
        assert_path_effect_round_trips(&discrete_path_effect::new(3.0, 2.0, 7).expect("discrete"));
        assert_path_effect_round_trips(&trim_path_effect::new(0.25, 0.75, None).expect("trim"));
        assert_path_effect_round_trips(&PathEffect::sum(dash.clone(), dash.clone()));
        assert_path_effect_round_trips(&PathEffect::compose(dash.clone(), dash));
    }

    #[test]
    fn mask_filters_round_trip() {
        assert_mask_filter_round_trips(
            &MaskFilter::blur(BlurStyle::Solid, 2.5, false).expect("valid blur"),
        );
        let table = table_mask_filter::new_gamma_table(0.5);
        assert_mask_filter_round_trips(&table_mask_filter::new(&table));
        let light = emboss_mask_filter::Light {
            direction: [1.0, 2.0, 3.0],
            pad: 0,
            ambient: 40,
            specular: 0x12,
        };
        assert_mask_filter_round_trips(&emboss_mask_filter::new(3.0, &light).expect("emboss"));
    }

    #[test]
    fn effects_registered_under_another_name_do_not_read_back() {
        // C++ registers the 1D, line 2D and path 2D effects as `...Impl` names, but writes them
        // under their `getTypeName`s (`SkPath1DPathEffect`, `SkLine2DPathEffect` and
        // `SkPath2DPathEffect`), so none of them reads back there either.
        let mut builder = PathBuilder::new();
        builder.move_to((0.0, 0.0)).line_to((1.0, 0.0));
        let path = builder.detach();
        let one_d =
            path_1d_path_effect::new(&path, 2.0, 0.0, path_1d_path_effect::Style::Translate)
                .expect("valid 1D effect");
        assert!(PathEffect::deserialize(one_d.serialize().as_bytes(), &REGISTRY).is_none());
        let matrix = Matrix::scale((2.0, 3.0));
        let line_2d = line_2d_path_effect::new(1.5, &matrix).expect("valid line 2D effect");
        assert!(PathEffect::deserialize(line_2d.serialize().as_bytes(), &REGISTRY).is_none());
        let path_2d = path_2d_path_effect::new(&matrix, &path);
        assert!(PathEffect::deserialize(path_2d.serialize().as_bytes(), &REGISTRY).is_none());
        assert!(PathEffect::deserialize(&[], &REGISTRY).is_none());
    }
}
