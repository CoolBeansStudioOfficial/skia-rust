// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/runtimeintrinsics.cpp (chrome/m156)
// The GM bodies mirror the long C++ `DEF_SIMPLE_GM` bodies, so they are kept whole.
#![allow(clippy::too_many_lines)]
//
// The `_es3` variants are GPU-only (Ganesh) and are excluded in the manifest.

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::scalar::SCALAR_PI;
use skia_rust_core::shader::Shader;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/runtimeintrinsics.cpp#L31-L33 (chrome/m156)
const K_BOX_SIZE: i32 = 100;
const K_PADDING: i32 = 5;
const K_LABEL_HEIGHT: i32 = 15;

// Port of: gm/runtimeintrinsics.cpp#L35-L37 (chrome/m156), next_column
fn next_column(canvas: &Canvas) {
    canvas.translate((int_to_scalar(K_BOX_SIZE + K_PADDING), 0.0));
}

// Port of: gm/runtimeintrinsics.cpp#L39-L43 (chrome/m156), next_row
fn next_row(canvas: &Canvas) {
    canvas.restore();
    canvas.translate((0.0, int_to_scalar(K_BOX_SIZE + K_PADDING + K_LABEL_HEIGHT)));
    canvas.save();
}

// Port of: gm/runtimeintrinsics.cpp#L45-L51 (chrome/m156), columns_to_width and rows_to_height
const fn columns_to_width(columns: i32) -> i32 {
    (K_PADDING + K_BOX_SIZE) * columns + K_PADDING
}

const fn rows_to_height(rows: i32) -> i32 {
    (K_PADDING + K_LABEL_HEIGHT + K_BOX_SIZE) * rows + K_PADDING
}

// Port of: gm/runtimeintrinsics.cpp#L53-L63 (chrome/m156), draw_label
// `(kLabelHeight + bounds.height()) * 0.5f` is a multiplication by 0.5 (exact), not a midpoint.
#[allow(clippy::manual_midpoint)]
fn draw_label(canvas: &Canvas, label: &str) {
    let font: Font = default_portable_font();
    let mut p = Paint::default();
    p.set_color(Color::BLACK);
    let (_, bounds) = font.measure_text(label.as_bytes(), TextEncoding::UTF8, None);
    canvas.draw_simple_text(
        label.as_bytes(),
        TextEncoding::UTF8,
        (
            (int_to_scalar(K_BOX_SIZE) - bounds.width()) * 0.5,
            // C++: (kLabelHeight + bounds.height()) * 0.5f, a multiplication by 0.5 (exact).
            (int_to_scalar(K_LABEL_HEIGHT) + bounds.height()) * 0.5,
        ),
        &font,
        &p,
    );
    canvas.translate((0.0, int_to_scalar(K_LABEL_HEIGHT)));
}

// Port of: gm/runtimeintrinsics.cpp#L65-L84 (chrome/m156), draw_shader (raster sink: the surface
// is a raster surface, so the raster fallback is always taken)
fn draw_shader(canvas: &Canvas, shader: Option<Shader>) -> Bitmap {
    let mut paint = Paint::default();
    paint.set_shader(shader);
    let mut bitmap = Bitmap::new();
    let info = ImageInfo::new_n32_premul((K_BOX_SIZE, K_BOX_SIZE), None);
    if let Some(mut surface) = surfaces::raster(&info, None, None) {
        surface.canvas().clear(Color::WHITE);
        surface
            .canvas()
            .scale((int_to_scalar(K_BOX_SIZE), int_to_scalar(K_BOX_SIZE)));
        surface.canvas().draw_rect(Rect::from_wh(1.0, 1.0), &paint);
        bitmap.alloc_pixels_flags(&info);
        surface.read_pixels_to_bitmap(&mut bitmap, (0, 0));
        if let Some(image) = bitmap.as_image() {
            canvas.draw_image(&image, (0.0, 0.0), None);
        }
    }
    bitmap
}

// Port of: gm/runtimeintrinsics.cpp#L86-L100 (chrome/m156), make_unary_sksl_1d
fn make_unary_sksl_1d(func: &str, require_es3: bool) -> String {
    format!(
        "#version {}\n\
         uniform float xScale; uniform float xBias;\
         uniform float yScale; uniform float yBias;\
         half4 main(float2 p) {{\
             const float2 v1 = float2(1);\
             const float2 v2 = float2(2);\
             p = float2(p.x, 1 - p.x) * xScale + xBias;\
             float x = p.x;\
             int2  pi = int2(floor(p));\
             int   xi = pi.x;\
             float y = float({}) * yScale + yBias;\
             return y.xxx1;\
         }}",
        if require_es3 { "300" } else { "100" },
        func
    )
}

// Port of: gm/runtimeintrinsics.cpp#L121-L163 (chrome/m156), plot
fn plot(
    canvas: &Canvas,
    func: &str,
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    label: Option<&str>,
) {
    canvas.save();
    draw_label(canvas, label.unwrap_or(func));
    let Some(effect) = RuntimeEffect::make_for_shader(make_unary_sksl_1d(func, false), None).ok()
    else {
        return;
    };
    let mut builder = RuntimeShaderBuilder::new(effect);
    builder.uniform("xScale").set_f32(&[x_max - x_min]);
    builder.uniform("xBias").set_f32(&[x_min]);
    builder.uniform("yScale").set_f32(&[1.0 / (y_max - y_min)]);
    builder
        .uniform("yBias")
        .set_f32(&[-y_min / (y_max - y_min)]);
    let bitmap = draw_shader(canvas, builder.make_shader(None));
    if !bitmap.is_empty() {
        // Plot.
        let mut plot_paint = Paint::default();
        plot_paint.set_color4f(Color4f::new(0.0, 0.5, 0.0, 1.0), None);
        let mut pts = Vec::with_capacity(K_BOX_SIZE as usize);
        for x in 0..K_BOX_SIZE {
            let c = bitmap.get_color((x, 0));
            let y = (1.0 - (f32::from(c.r()) / 255.0)) * int_to_scalar(K_BOX_SIZE);
            pts.push(Point::new(int_to_scalar(x) + 0.5, y));
        }
        plot_paint.set_anti_alias(true);
        canvas.draw_points(PointMode::Polygon, &pts, &plot_paint);
    }
    canvas.restore();
    next_column(canvas);
}

// Port of: gm/runtimeintrinsics.cpp#L165-L196 (chrome/m156), runtime_intrinsics_trig
crate::def_simple_gm!(
    runtime_intrinsics_trig,
    canvas,
    columns_to_width(3),
    rows_to_height(5),
    {
        let pi = SCALAR_PI;
        let two_pi = 2.0 * SCALAR_PI;
        let pi_over_two = SCALAR_PI / 2.0;
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        plot(canvas, "radians(x)", 0.0, 360.0, 0.0, two_pi, None);
        plot(canvas, "degrees(x)", 0.0, two_pi, 0.0, 360.0, None);
        next_row(canvas);
        plot(canvas, "sin(x)", 0.0, two_pi, -1.0, 1.0, None);
        plot(canvas, "cos(x)", 0.0, two_pi, -1.0, 1.0, None);
        plot(canvas, "tan(x)", 0.0, pi, -10.0, 10.0, None);
        next_row(canvas);
        plot(
            canvas,
            "asin(x)",
            -1.0,
            1.0,
            -pi_over_two,
            pi_over_two,
            None,
        );
        plot(canvas, "acos(x)", -1.0, 1.0, 0.0, pi, None);
        plot(
            canvas,
            "atan(x)",
            -10.0,
            10.0,
            -pi_over_two,
            pi_over_two,
            None,
        );
        next_row(canvas);
        plot(canvas, "atan(0.1,  x)", -1.0, 1.0, 0.0, pi, None);
        plot(canvas, "atan(-0.1, x)", -1.0, 1.0, -pi, 0.0, None);
        next_row(canvas);
        plot(
            canvas,
            "atan(x,  0.1)",
            -1.0,
            1.0,
            -pi_over_two,
            pi_over_two,
            None,
        );
        plot(canvas, "atan(x, -0.1)", -1.0, 1.0, -pi, pi, None);
        next_row(canvas);
    }
);

// Port of: gm/runtimeintrinsics.cpp#L238-L265 (chrome/m156), runtime_intrinsics_exponential
crate::def_simple_gm!(
    runtime_intrinsics_exponential,
    canvas,
    columns_to_width(2),
    rows_to_height(5),
    {
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        plot(canvas, "pow(x, 3)", 0.0, 8.0, 0.0, 500.0, None);
        plot(canvas, "pow(x, -3)", 0.0, 4.0, 0.0, 10.0, None);
        next_row(canvas);
        plot(canvas, "pow(0.9, x)", -10.0, 10.0, 0.0, 3.0, None);
        plot(canvas, "pow(1.1, x)", -10.0, 10.0, 0.0, 3.0, None);
        next_row(canvas);
        plot(canvas, "exp(x)", -1.0, 7.0, 0.0, 1000.0, None);
        plot(canvas, "log(x)", 0.0, 2.5, -4.0, 1.0, None);
        next_row(canvas);
        plot(canvas, "exp2(x)", -1.0, 7.0, 0.0, 130.0, None);
        plot(canvas, "log2(x)", 0.0, 4.0, -4.0, 2.0, None);
        next_row(canvas);
        plot(canvas, "sqrt(x)", 0.0, 25.0, 0.0, 5.0, None);
        plot(canvas, "inversesqrt(x)", 0.0, 25.0, 0.2, 4.0, None);
        next_row(canvas);
    }
);

// Port of: gm/runtimeintrinsics.cpp#L267-L320 (chrome/m156), runtime_intrinsics_common
crate::def_simple_gm!(
    runtime_intrinsics_common,
    canvas,
    columns_to_width(6),
    rows_to_height(7),
    {
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        plot(canvas, "abs(x)", -10.0, 10.0, 0.0, 10.0, None);
        plot(canvas, "sign(x)", -1.0, 1.0, -1.5, 1.5, None);
        next_row(canvas);
        plot(canvas, "floor(x)", -3.0, 3.0, -4.0, 4.0, None);
        plot(canvas, "ceil(x)", -3.0, 3.0, -4.0, 4.0, None);
        plot(canvas, "fract(x)", -3.0, 3.0, 0.0, 1.0, None);
        plot(
            canvas,
            "mod(x, 2)",
            -4.0,
            4.0,
            -2.0,
            2.0,
            Some("mod(scalar)"),
        );
        plot(
            canvas,
            "mod(p, -2).x",
            -4.0,
            4.0,
            -2.0,
            2.0,
            Some("mod(mixed)"),
        );
        plot(
            canvas,
            "mod(p, v2).x",
            -4.0,
            4.0,
            -2.0,
            2.0,
            Some("mod(vector)"),
        );
        next_row(canvas);
        plot(canvas, "min(x, 1)", 0.0, 2.0, 0.0, 2.0, Some("min(scalar)"));
        plot(
            canvas,
            "min(p, 1).x",
            0.0,
            2.0,
            0.0,
            2.0,
            Some("min(mixed)"),
        );
        plot(
            canvas,
            "min(p, v1).x",
            0.0,
            2.0,
            0.0,
            2.0,
            Some("min(vector)"),
        );
        plot(canvas, "max(x, 1)", 0.0, 2.0, 0.0, 2.0, Some("max(scalar)"));
        plot(
            canvas,
            "max(p, 1).x",
            0.0,
            2.0,
            0.0,
            2.0,
            Some("max(mixed)"),
        );
        plot(
            canvas,
            "max(p, v1).x",
            0.0,
            2.0,
            0.0,
            2.0,
            Some("max(vector)"),
        );
        next_row(canvas);
        plot(
            canvas,
            "clamp(x, 1, 2)",
            0.0,
            3.0,
            0.0,
            3.0,
            Some("clamp(scalar)"),
        );
        plot(
            canvas,
            "clamp(p, 1, 2).x",
            0.0,
            3.0,
            0.0,
            3.0,
            Some("clamp(mixed)"),
        );
        plot(
            canvas,
            "clamp(p, v1, v2).x",
            0.0,
            3.0,
            0.0,
            3.0,
            Some("clamp(vector)"),
        );
        plot(canvas, "saturate(x)", -1.0, 2.0, -0.5, 1.5, None);
        next_row(canvas);
        plot(
            canvas,
            "mix(1, 2, x)",
            -1.0,
            2.0,
            0.0,
            3.0,
            Some("mix(scalar)"),
        );
        plot(
            canvas,
            "mix(v1, v2, x).x",
            -1.0,
            2.0,
            0.0,
            3.0,
            Some("mix(mixed)"),
        );
        plot(
            canvas,
            "mix(v1, v2, p).x",
            -1.0,
            2.0,
            0.0,
            3.0,
            Some("mix(vector)"),
        );
        next_row(canvas);
        plot(
            canvas,
            "step(1, x)",
            0.0,
            2.0,
            -0.5,
            1.5,
            Some("step(scalar)"),
        );
        plot(
            canvas,
            "step(1, p).x",
            0.0,
            2.0,
            -0.5,
            1.5,
            Some("step(mixed)"),
        );
        plot(
            canvas,
            "step(v1, p).x",
            0.0,
            2.0,
            -0.5,
            1.5,
            Some("step(vector)"),
        );
        plot(
            canvas,
            "smoothstep(1, 2, x)",
            0.5,
            2.5,
            -0.5,
            1.5,
            Some("smooth(scalar)"),
        );
        plot(
            canvas,
            "smoothstep(1, 2, p).x",
            0.5,
            2.5,
            -0.5,
            1.5,
            Some("smooth(mixed)"),
        );
        plot(
            canvas,
            "smoothstep(v1, v2, p).x",
            0.5,
            2.5,
            -0.5,
            1.5,
            Some("smooth(vector)"),
        );
        next_row(canvas);
        plot(canvas, "floor(p).x", -3.0, 3.0, -4.0, 4.0, None);
        plot(canvas, "ceil(p).x", -3.0, 3.0, -4.0, 4.0, None);
        plot(canvas, "floor(p).y", -3.0, 3.0, -4.0, 4.0, None);
        plot(canvas, "ceil(p).y", -3.0, 3.0, -4.0, 4.0, None);
        next_row(canvas);
    }
);

// Port of: gm/runtimeintrinsics.cpp#L382-L436 (chrome/m156), runtime_intrinsics_geometric
crate::def_simple_gm!(
    runtime_intrinsics_geometric,
    canvas,
    columns_to_width(4),
    rows_to_height(5),
    {
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        plot(canvas, "length(x)", -1.0, 1.0, -0.5, 1.5, None);
        plot(canvas, "length(p)", 0.0, 1.0, 0.5, 1.5, None);
        plot(canvas, "distance(x, 0)", -1.0, 1.0, -0.5, 1.5, None);
        plot(canvas, "distance(p, v1)", 0.0, 1.0, 0.5, 1.5, None);
        next_row(canvas);
        plot(canvas, "dot(x, 2)", -1.0, 1.0, -2.5, 2.5, None);
        plot(canvas, "dot(p, p.y1)", -1.0, 1.0, -2.5, 0.5, None);
        next_row(canvas);
        plot(canvas, "cross(p.xy1, p.y1x).x", 0.0, 1.0, -1.0, 1.0, None);
        plot(canvas, "cross(p.xy1, p.y1x).y", 0.0, 1.0, -1.0, 1.0, None);
        plot(canvas, "cross(p.xy1, p.y1x).z", 0.0, 1.0, -1.0, 1.0, None);
        next_row(canvas);
        plot(canvas, "normalize(x)", -2.0, 2.0, -1.5, 1.5, None);
        plot(canvas, "normalize(p).x", 0.0, 2.0, 0.0, 1.0, None);
        plot(canvas, "normalize(p).y", 0.0, 2.0, 0.0, 1.0, None);
        plot(
            canvas,
            "faceforward(v1, p.x0, v1.x0).x",
            -1.0,
            1.0,
            -1.5,
            1.5,
            Some("faceforward"),
        );
        next_row(canvas);
        plot(
            canvas,
            "reflect(p.x1, v1.0x).x",
            -1.0,
            1.0,
            -1.0,
            1.0,
            Some("reflect(horiz)"),
        );
        plot(
            canvas,
            "reflect(p.x1, normalize(v1)).y",
            -1.0,
            1.0,
            -1.0,
            1.0,
            Some("reflect(diag)"),
        );
        plot(
            canvas,
            "refract(v1.x0, v1.0x, x).x",
            0.0,
            1.0,
            -1.0,
            1.0,
            Some("refract().x"),
        );
        plot(
            canvas,
            "refract(v1.x0, v1.0x, x).y",
            0.0,
            1.0,
            -1.0,
            1.0,
            Some("refract().y"),
        );
        next_row(canvas);
    }
);

// Port of: gm/runtimeintrinsics.cpp#L438-L451 (chrome/m156), make_matrix_comp_mult_sksl
fn make_matrix_comp_mult_sksl(dim: usize) -> String {
    format!(
        "uniform float{dim}x{dim} m1;\
         uniform float{dim}x{dim} m2;\
         {SKSL_MATRIX_SELECTORS}\
         half4 main(float2 p) {{\
             float{dim} colSel = sel{dim}(p.x);\
             float{dim} rowSel = sel{dim}(p.y);\
             float{dim} col = matrixCompMult(m1, m2) * colSel;\
             float  v = dot(col, rowSel);\
             return v.xxx1;\
         }}",
    )
}

// Port of: gm/runtimeintrinsics.cpp#L23-L29 (chrome/m156), SKSL_MATRIX_SELECTORS
const SKSL_MATRIX_SELECTORS: &str = "inline float2 sel2(float x) {\
    return float2(\
      x <  0.5 ? 1 : 0,\
      x >= 0.5 ? 1 : 0);\
}\
inline float3 sel3(float x) {\
    return float3(\
      x <  0.33             ? 1 : 0,\
      x >= 0.33 && x < 0.66 ? 1 : 0,\
      x >= 0.66             ? 1 : 0);\
}\
inline float4 sel4(float x) {\
    return float4(\
      x <  0.25             ? 1 : 0,\
      x >= 0.25 && x < 0.5  ? 1 : 0,\
      x >= 0.5  && x < 0.75 ? 1 : 0,\
      x >= 0.75             ? 1 : 0);\
}";

// Port of: gm/runtimeintrinsics.cpp#L453-L476 (chrome/m156), plot_matrix_comp_mult
fn plot_matrix_comp_mult(canvas: &Canvas, dim: usize, mtx1: &[f32], mtx2: &[f32], label: &str) {
    canvas.save();
    draw_label(canvas, label);
    let Some(effect) = RuntimeEffect::make_for_shader(make_matrix_comp_mult_sksl(dim), None).ok()
    else {
        return;
    };
    let mut builder = RuntimeShaderBuilder::new(effect);
    builder.uniform("m1").set_f32(mtx1);
    builder.uniform("m2").set_f32(mtx2);
    draw_shader(canvas, builder.make_shader(None));
    canvas.restore();
    next_column(canvas);
}

// Port of: gm/runtimeintrinsics.cpp#L478-L491 (chrome/m156), make_matrix_inverse_sksl
fn make_matrix_inverse_sksl(dim: usize) -> String {
    format!(
        "uniform float scale; uniform float bias;\
         uniform float{dim}x{dim} m;\
         {SKSL_MATRIX_SELECTORS}\
         half4 main(float2 p) {{\
             float{dim} colSel = sel{dim}(p.x);\
             float{dim} rowSel = sel{dim}(p.y);\
             float{dim} col = inverse(m) * colSel;\
             float  v = dot(col, rowSel) * scale + bias;\
             return v.xxx1;\
         }}",
    )
}

// Port of: gm/runtimeintrinsics.cpp#L493-L514 (chrome/m156), plot_matrix_inverse
fn plot_matrix_inverse(canvas: &Canvas, dim: usize, mtx: &[f32], label: &str) {
    canvas.save();
    draw_label(canvas, label);
    let Some(effect) = RuntimeEffect::make_for_shader(make_matrix_inverse_sksl(dim), None).ok()
    else {
        return;
    };
    let mut builder = RuntimeShaderBuilder::new(effect);
    builder.uniform("scale").set_f32(&[0.5]);
    builder.uniform("bias").set_f32(&[0.5]);
    builder.uniform("m").set_f32(mtx);
    draw_shader(canvas, builder.make_shader(None));
    canvas.restore();
    next_column(canvas);
}

// Port of: gm/runtimeintrinsics.cpp#L516-L564 (chrome/m156), runtime_intrinsics_matrix
crate::def_simple_gm!(
    runtime_intrinsics_matrix,
    canvas,
    columns_to_width(3),
    rows_to_height(2),
    {
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        // Random pairs of matrices where the elements of matrixCompMult(m1, m2) lie in [0, 1]
        plot_matrix_comp_mult(
            canvas,
            2,
            &[1.00, 0.0, 2.0, 0.5],
            &[0.75, 2.0, 0.2, 1.2],
            "compMult(2x2)",
        );
        plot_matrix_comp_mult(
            canvas,
            3,
            &[1.00, 0.0, 2.0, 0.5, -1.0, -2.0, -0.5, 4.00, 0.25],
            &[0.75, 2.0, 0.2, 1.2, -0.8, -0.1, -1.8, 0.25, 2.00],
            "compMult(3x3)",
        );
        plot_matrix_comp_mult(
            canvas,
            4,
            &[
                1.00, 0.0, 2.0, 0.5, -1.0, -2.0, -0.5, 4.00, 0.25, 0.05, 10.00, -0.66, -1.0, -0.5,
                0.5, 0.66,
            ],
            &[
                0.75, 2.0, 0.2, 1.2, -0.8, -0.1, -1.8, 0.25, 2.00, 2.00, 0.03, -1.00, -1.0, -0.5,
                1.7, 0.66,
            ],
            "compMult(4x4)",
        );
        next_row(canvas);
        // Random, invertible matrices where the elements of inverse(m) lie in [-1, 1]
        plot_matrix_inverse(canvas, 2, &[1.20, 0.68, -0.27, -1.55], "inverse(2x2)");
        plot_matrix_inverse(
            canvas,
            3,
            &[-1.13, -2.96, -0.14, 1.45, -1.88, -1.02, -2.54, -2.58, -1.17],
            "inverse(3x3)",
        );
        plot_matrix_inverse(
            canvas,
            4,
            &[
                -1.51, -3.95, -0.19, 1.93, -2.51, -1.35, -3.39, -3.45, -1.56, 1.61, -0.22, -1.08,
                -2.81, -2.14, -0.09, 3.00,
            ],
            "inverse(4x4)",
        );
        next_row(canvas);
    }
);

// Port of: gm/runtimeintrinsics.cpp#L566-L579 (chrome/m156), make_bvec_sksl
fn make_bvec_sksl(ty: &str, func: &str) -> String {
    // We use negative floats, to ensure that the integer variants are working with the correct
    // interpretation of the data.
    format!(
        "uniform {ty}2 v1;\
         half4 main(float2 p) {{\
             p.x = p.x < 0.33 ? -3.0 : (p.x < 0.66 ? -2.0 : -1.0);\
             p.y = p.y < 0.33 ? -3.0 : (p.y < 0.66 ? -2.0 : -1.0);\
             bool2 cmp = {func};\
             return half4(cmp.x ? 1.0 : 0.0, cmp.y ? 1.0 : 0.0, 0, 1);\
         }}"
    )
}

// Port of: gm/runtimeintrinsics.cpp#L581-L601 (chrome/m156), plot_bvec
fn plot_bvec(canvas: &Canvas, is_int: bool, func: &str, label: &str) {
    canvas.save();
    draw_label(canvas, label);
    let ty = if is_int { "int" } else { "float" };
    let Some(effect) = RuntimeEffect::make_for_shader(make_bvec_sksl(ty, func), None).ok() else {
        return;
    };
    // T uniformData[2] = { -2, -2 };
    let uniform_data: Vec<u8> = if is_int {
        [-2_i32, -2_i32]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect()
    } else {
        [-2.0_f32, -2.0_f32]
            .iter()
            .flat_map(|v| v.to_ne_bytes())
            .collect()
    };
    let uniforms = Data::new_from_vec(uniform_data);
    draw_shader(canvas, effect.make_shader(uniforms, &[], None));
    canvas.restore();
    next_column(canvas);
}

// Port of: gm/runtimeintrinsics.cpp#L603-L639 (chrome/m156), runtime_intrinsics_relational
crate::def_simple_gm!(
    runtime_intrinsics_relational,
    canvas,
    columns_to_width(4),
    rows_to_height(6),
    {
        canvas.translate((int_to_scalar(K_PADDING), int_to_scalar(K_PADDING)));
        canvas.save();
        plot_bvec(canvas, false, "lessThan(p, v1)", "lessThan");
        plot_bvec(canvas, true, "lessThan(int2(p), v1)", "lessThan(int)");
        plot_bvec(canvas, false, "lessThanEqual(p, v1)", "lessThanEqual");
        plot_bvec(
            canvas,
            true,
            "lessThanEqual(int2(p), v1)",
            "lessThanEqual(int)",
        );
        next_row(canvas);
        plot_bvec(canvas, false, "greaterThan(p, v1)", "greaterThan");
        plot_bvec(canvas, true, "greaterThan(int2(p), v1)", "greaterThan(int)");
        plot_bvec(canvas, false, "greaterThanEqual(p, v1)", "greaterThanEqual");
        plot_bvec(
            canvas,
            true,
            "greaterThanEqual(int2(p), v1)",
            "greaterThanEqual(int)",
        );
        next_row(canvas);
        plot_bvec(canvas, false, "equal(p, v1)", "equal");
        plot_bvec(canvas, true, "equal(int2(p), v1)", "equal(int)");
        plot_bvec(canvas, false, "notEqual(p, v1)", "notEqual");
        plot_bvec(canvas, true, "notEqual(int2(p), v1)", "notEqual(int)");
        next_row(canvas);
        plot_bvec(
            canvas,
            false,
            "equal(   lessThanEqual(p, v1), greaterThanEqual(p, v1))",
            "equal(bvec)",
        );
        plot_bvec(
            canvas,
            false,
            "notEqual(lessThanEqual(p, v1), greaterThanEqual(p, v1))",
            "notequal(bvec)",
        );
        next_row(canvas);
        plot_bvec(canvas, false, "not(notEqual(p, v1))", "not(notEqual)");
        plot_bvec(canvas, false, "not(equal(p, v1))", "not(equal)");
        next_row(canvas);
        plot_bvec(canvas, false, "bool2(any(equal(p, v1)))", "any(equal)");
        plot_bvec(canvas, false, "bool2(all(equal(p, v1)))", "all(equal)");
        next_row(canvas);
    }
);
