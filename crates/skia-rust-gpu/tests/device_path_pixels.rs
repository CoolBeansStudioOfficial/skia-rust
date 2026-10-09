// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! G10c: path draws recorded on a Graphite `Device`, inserted into a context on a real adapter,
//! run by the wgpu command buffer and read back with `Context::read_pixels`: fills, inverse
//! fills, strokes, stroke-and-fills and hairlines (the noop-adapter checks of the same draws are
//! in `device_paths.rs`).
//!
//! Like the other real-adapter tests these are `#[ignore]`d, so CI counts them as not run. Run
//! them with `cargo test -p skia-rust-gpu --test device_path_pixels -- --ignored`. A machine with
//! no adapter that renders (lavapipe on Linux, WARP on Windows) reports that and passes, unless
//! `SKIA_RUST_REQUIRE_ADAPTER` is set.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::color::Color4f;
use skia_rust_core::mesh::{Attribute, AttributeType, Mesh, MeshSpecification, Mode, meshes};
use skia_rust_core::rect::Rect;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_gpu::gpu::backing_fit::BackingFit;
use skia_rust_gpu::gpu::gpu_types::{Budgeted, Mipmapped};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::device::Device;
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::resource_types::LoadOp;
use skia_rust_gpu::graphite::wgpu::{WgpuContext, adapter_backend_context, make_context};

const SIZE: i32 = 128;

/// A context on a real adapter, or `None` (after saying so) if the machine has none.
fn real_context() -> Option<WgpuContext> {
    let Some((backend_context, info)) = adapter_backend_context(wgpu::Backends::all()) else {
        assert!(
            std::env::var_os("SKIA_RUST_REQUIRE_ADAPTER").is_none(),
            "SKIA_RUST_REQUIRE_ADAPTER is set but there is no adapter"
        );
        eprintln!("no adapter that renders: skipping");
        return None;
    };
    eprintln!(
        "adapter: {} ({:?}, {:?})",
        info.name, info.backend, info.device_type
    );
    Some(make_context(&backend_context, &ContextOptions::default()).expect("a context"))
}

/// A paint of an opaque color, without antialiasing so the pixels are exact.
fn paint(style: Style, width: f32) -> Paint {
    let mut paint = Paint::new(Color4f::new(1.0, 0.0, 0.0, 1.0), None);
    paint.set_anti_alias(false);
    paint.set_style(style);
    paint.set_stroke_width(width);
    paint
}

/// A convex hexagon around the centre of the target.
fn hexagon(fill_type: PathFillType) -> Path {
    let points = [
        Point::new(64.0, 20.0),
        Point::new(104.0, 42.0),
        Point::new(104.0, 86.0),
        Point::new(64.0, 108.0),
        Point::new(24.0, 86.0),
        Point::new(24.0, 42.0),
    ];
    let mut builder = PathBuilder::new_with_fill_type(fill_type);
    builder.move_to(points[0]);
    for point in &points[1..] {
        builder.line_to(*point);
    }
    builder.close();
    builder.detach()
}

/// A five-pointed star, which is not convex.
fn star() -> Path {
    let mut builder = PathBuilder::new();
    for i in 0..10 {
        let radius = if i % 2 == 0 { 50.0_f32 } else { 20.0 };
        let angle = std::f32::consts::PI / 5.0 * i as f32 - std::f32::consts::FRAC_PI_2;
        let p = Point::new(64.0 + radius * angle.cos(), 64.0 + radius * angle.sin());
        if i == 0 {
            builder.move_to(p);
        } else {
            builder.line_to(p);
        }
    }
    builder.close();
    builder.detach()
}

/// Draws `draws` into a cleared `SIZE` x `SIZE` N32 target and returns its pixels as `(RGBA
/// bytes, row bytes)`.
fn render(context: &mut WgpuContext, draws: &[(Path, Paint)]) -> (Vec<u8>, usize) {
    render_with(context, |device| {
        for (path, paint) in draws {
            device.draw_path(path, paint);
        }
    })
}

/// Records what `draw` draws into a cleared target, runs it and reads the target back.
fn render_with(
    context: &mut WgpuContext,
    draw: impl FnOnce(&mut Device),
) -> (Vec<u8>, usize) {
    let recorder = context.make_recorder(None);
    let image_info = ImageInfo::new_n32_premul((SIZE, SIZE), None);
    let mut device = Device::make_with_info(
        Some(&recorder),
        &image_info,
        Budgeted::Yes,
        Mipmapped::No,
        BackingFit::Exact,
        &SurfaceProps::default(),
        LoadOp::Clear,
        "DevicePathPixels",
        true,
        false,
    )
    .expect("a device");
    draw(&mut device);
    let target = device.target();
    device.flush_pending_work();

    let mut recorder = recorder;
    let mut recording = recorder.snap().expect("a recording");
    let status = context.insert_recording(InsertRecordingInfo::new(&mut recording));
    assert_eq!(status, InsertStatus::Success);
    assert!(context.submit(SubmitInfo::new(SyncToCpu::Yes)));
    drop(device);

    context
        .read_pixels(
            &target,
            image_info.color_info(),
            IRect::from_size((SIZE, SIZE)),
            &image_info,
        )
        .expect("the pixels")
}

fn pixel(pixels: &(Vec<u8>, usize), x: usize, y: usize) -> [u8; 4] {
    let (pixels, row_bytes) = pixels;
    let i = y * row_bytes + x * 4;
    pixels[i..i + 4].try_into().unwrap()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

// Checks the convex wedges that chooseMSAARenderer picks (Device.cpp#L2304-L2341) by pixels.
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_convex_fill_covers_its_inside_only() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(
        &mut context,
        &[(hexagon(PathFillType::Winding), paint(Style::Fill, 0.0))],
    );
    assert_eq!(pixel(&pixels, 64, 64), RED);
    assert_eq!(pixel(&pixels, 30, 64), RED);
    assert_eq!(pixel(&pixels, 100, 64), RED);
    assert_eq!(pixel(&pixels, 10, 64), CLEAR);
    assert_eq!(pixel(&pixels, 64, 10), CLEAR);
    assert_eq!(pixel(&pixels, 2, 2), CLEAR);
}

// A concave fill leaves the notches between the spikes clear.
// Covers: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `StencilTessellatedWedges`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_concave_fill_leaves_its_notches_clear() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(&mut context, &[(star(), paint(Style::Fill, 0.0))]);
    assert_eq!(pixel(&pixels, 64, 64), RED);
    // Radius 40 along the direction of an inner vertex (radius 20): outside the star.
    assert_eq!(pixel(&pixels, 87, 32), CLEAR);
    assert_eq!(pixel(&pixels, 2, 2), CLEAR);
}

// An inverse fill covers everything outside the path.
// Covers: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `shape.inverted()`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn an_inverse_fill_covers_the_outside() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(
        &mut context,
        &[(
            hexagon(PathFillType::InverseWinding),
            paint(Style::Fill, 0.0),
        )],
    );
    assert_eq!(pixel(&pixels, 64, 64), CLEAR);
    assert_eq!(pixel(&pixels, 2, 2), RED);
    assert_eq!(pixel(&pixels, 126, 126), RED);
}

// A stroke is centred on the outline: the vertex at (64, 20) with width 8 covers y in [16, 24).
// Covers: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kStroke_Style`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_stroke_covers_its_outline_only() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(
        &mut context,
        &[(hexagon(PathFillType::Winding), paint(Style::Stroke, 8.0))],
    );
    assert_eq!(pixel(&pixels, 64, 21), RED);
    assert_eq!(pixel(&pixels, 64, 64), CLEAR);
    assert_eq!(pixel(&pixels, 64, 10), CLEAR);
}

// A stroke-and-fill covers the inside and the outline.
// Covers: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kStrokeAndFill_Style`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_stroke_and_fill_covers_the_inside_and_the_outline() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(
        &mut context,
        &[(
            hexagon(PathFillType::Winding),
            paint(Style::StrokeAndFill, 8.0),
        )],
    );
    assert_eq!(pixel(&pixels, 64, 64), RED);
    assert_eq!(pixel(&pixels, 64, 21), RED);
    assert_eq!(pixel(&pixels, 64, 10), CLEAR);
}

// A hairline is one device pixel wide along the outline, and covers nothing inside.
// Covers: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156), `kHairline_Style`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_hairline_draws_a_thin_line_on_the_outline() {
    let Some(mut context) = real_context() else {
        return;
    };
    let pixels = render(
        &mut context,
        &[(hexagon(PathFillType::Winding), paint(Style::Stroke, 0.0))],
    );
    // The top-left edge runs from (64, 20) to (24, 42); its midpoint is (44, 31). A hairline is
    // antialiased (its coverage is analytic in the stroke shaders), so the pixels on the line are
    // partly covered red rather than RED.
    let drawn = (43..=45)
        .flat_map(|x| (30..=32).map(move |y| (x, y)))
        .any(|(x, y)| {
            let [r, _, _, a] = pixel(&pixels, x, y);
            a > 0 && r == a
        });
    assert!(drawn, "the hairline is on the edge");
    assert_eq!(pixel(&pixels, 64, 64), CLEAR);
}

/// A mesh specification whose fragment shader returns the local coordinates and no color, so the
/// paint's color is drawn.
fn mesh_spec() -> Arc<MeshSpecification> {
    let attributes = [Attribute {
        ty: AttributeType::Float2,
        offset: 0,
        name: String::from("pos"),
    }];
    let result = MeshSpecification::make(
        &attributes,
        8,
        &[],
        "Varyings main(const Attributes a) { Varyings v; v.position = a.pos; return v; }",
        "float2 main(const Varyings v) { return v.position; }",
    );
    result.specification.expect(&result.error)
}

/// A mesh of `points`, drawn as triangles, through `indices` if there are some.
fn mesh_of(points: &[(f32, f32)], indices: Option<&[u16]>) -> Mesh {
    let bytes: Vec<u8> = points
        .iter()
        .flat_map(|(x, y)| x.to_ne_bytes().into_iter().chain(y.to_ne_bytes()))
        .collect();
    let vb = meshes::make_vertex_buffer(Some(&bytes), bytes.len());
    let bounds = Rect::from_ltrb(0.0, 0.0, SIZE as f32, SIZE as f32);
    let result = match indices {
        None => Mesh::make(
            Some(mesh_spec()),
            Mode::Triangles,
            Some(vb),
            points.len(),
            0,
            None,
            &[],
            bounds,
        ),
        Some(indices) => {
            let index_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_ne_bytes()).collect();
            let ib = meshes::make_index_buffer(Some(&index_bytes), index_bytes.len());
            Mesh::make_indexed(
                Some(mesh_spec()),
                Mode::Triangles,
                Some(vb),
                points.len(),
                0,
                Some(ib),
                indices.len(),
                0,
                None,
                &[],
                bounds,
            )
        }
    };
    assert!(result.error.is_empty(), "{}", result.error);
    result.mesh
}

// A mesh fills its triangles with the paint's color.
// Covers: src/gpu/graphite/Device.cpp#L1007-L1070 (chrome/m156), `drawMesh`
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn a_mesh_fills_its_triangle() {
    let Some(mut context) = real_context() else {
        return;
    };
    let mesh = mesh_of(&[(10.0, 10.0), (118.0, 10.0), (10.0, 118.0)], None);
    let pixels = render_with(&mut context, |device| {
        device.draw_mesh(
            &mesh,
            Blender::mode(BlendMode::SrcOver),
            &paint(Style::Fill, 0.0),
        );
    });
    assert_eq!(pixel(&pixels, 20, 20), RED);
    assert_eq!(pixel(&pixels, 100, 100), CLEAR);
}

// An indexed mesh draws the triangles its index buffer names.
// Covers: src/gpu/graphite/Device.cpp#L1007-L1070 (chrome/m156), `drawMesh` with an index buffer
#[test]
#[ignore = "needs a real adapter in CI (lavapipe job)"]
fn an_indexed_mesh_fills_the_triangles_of_its_indices() {
    let Some(mut context) = real_context() else {
        return;
    };
    // Two triangles sharing the edge from (60, 10) to (10, 118).
    let mesh = mesh_of(
        &[(10.0, 10.0), (60.0, 10.0), (10.0, 118.0), (118.0, 118.0)],
        Some(&[0, 1, 2, 1, 2, 3]),
    );
    let pixels = render_with(&mut context, |device| {
        device.draw_mesh(
            &mesh,
            Blender::mode(BlendMode::SrcOver),
            &paint(Style::Fill, 0.0),
        );
    });
    assert_eq!(pixel(&pixels, 30, 30), RED);
    assert_eq!(pixel(&pixels, 100, 100), RED);
    assert_eq!(pixel(&pixels, 120, 20), CLEAR);
}
