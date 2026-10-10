// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SRGBTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied verbatim from the C++ test.
#![allow(clippy::float_cmp)]
// exact float comparisons, as in the C++ test
// The small counts (at most 384) are exact in f32, as in the C++ int-to-float conversions.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline,
};

use crate::{def_test, errorf, reporter_assert};

// Port of: tests/SRGBTest.cpp#L23-L51 (chrome/m156)
// The C++ loads and stores through the same `reds` buffer. A pipeline run reads each pixel before
// it writes that pixel, so separate source and destination buffers give the same result.
def_test!(srgb_roundtrip, |r| {
    let mut reds: [u32; 256] = [0; 256];
    for (i, red) in reds.iter_mut().enumerate() {
        *red = u32::try_from(i).expect("small value");
    }
    let src_bytes: Vec<u8> = reds.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let mut dst_bytes = vec![0u8; src_bytes.len()];

    let srgb = ColorSpace::new_srgb();
    let linear = srgb.with_linear_gamma();
    let upm = AlphaType::Unpremul;
    let linearize = ColorSpaceXformSteps::new(Some(&srgb), upm, Some(&linear), upm);
    let reencode = ColorSpaceXformSteps::new(Some(&linear), upm, Some(&srgb), upm);

    let alloc = ArenaAlloc::new();
    let mut p = RasterPipeline::new();
    p.append_load(ColorType::N32, MemoryCtx::new(MemSlot(0)));
    linearize.apply_to_pipeline(&mut p, &alloc);
    reencode.apply_to_pipeline(&mut p, &alloc);
    p.append_store(ColorType::N32, MemoryCtx::new(MemSlot(1)));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&src_bytes))
        .with(MemSlot(1), MemView::write(&mut dst_bytes));
    p.run(0, 0, 256, 1, &mut mem);

    for i in 0..256 {
        let out = u32::from_ne_bytes(dst_bytes[4 * i..4 * i + 4].try_into().expect("4 bytes"));
        if out != u32::try_from(i).expect("small value") {
            errorf!(r, "{} doesn't round trip, {}", i, out);
        }
    }
});

// Port of: tests/SRGBTest.cpp#L53-L82 (chrome/m156)
def_test!(srgb_edge_cases, |r| {
    // We need to run at least 4 pixels to make sure we hit all specializations.
    let color: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
    // The C++ stores 4 pixels into `colors[0]`, which is contiguous with the rows after it.
    let mut colors_bytes = vec![0u8; 4 * 4 * 4];

    let srgb = ColorSpace::new_srgb();
    let linear = srgb.with_linear_gamma();
    let upm = AlphaType::Unpremul;
    let steps = ColorSpaceXformSteps::new(Some(&linear), upm, Some(&srgb), upm);

    let alloc = ArenaAlloc::new();
    let mut p = RasterPipeline::new();
    p.append_constant_color(&alloc, &color);
    steps.apply_to_pipeline(&mut p, &alloc);
    p.append_store(ColorType::RGBAF32, MemoryCtx::new(MemSlot(0)));
    let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut colors_bytes));
    p.run(0, 0, 4, 1, &mut mem);

    let first = f32::from_ne_bytes(colors_bytes[0..4].try_into().expect("4 bytes"));
    let second = f32::from_ne_bytes(colors_bytes[4..8].try_into().expect("4 bytes"));
    if first != 0.0 {
        errorf!(r, "expected to_srgb() to map 0.0f to 0.0f, got {}", first);
    }
    if second != 1.0 {
        errorf!(
            r,
            "expected to_srgb() to map 1.0f to 1.0f, got {} ({:08x})",
            second,
            second.to_bits()
        );
    }
});

// Port of: tests/SRGBTest.cpp#L85-L135 (chrome/m156)
// Linearize and then re-encode pixel values, testing that the output is close to the input.
def_test!(srgb_roundtrip_extended, |r| {
    const K_STEPS: usize = 128;
    let expected = |i: usize| -> [f32; 4] {
        let scale: f32 = 10000.0 / (3 * K_STEPS) as f32;
        [
            (3 * i) as f32 * scale,
            (3 * i + 1) as f32 * scale,
            (3 * i + 2) as f32 * scale,
            1.0,
        ]
    };
    let mut rgba: Vec<[f32; 4]> = (0..K_STEPS).map(expected).collect();
    let src_bytes: Vec<u8> = rgba
        .iter()
        .flatten()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    let mut dst_bytes = vec![0u8; src_bytes.len()];

    let cs = ColorSpace::new_srgb();
    let linear = cs.with_linear_gamma();
    let upm = AlphaType::Unpremul;
    let linearize = ColorSpaceXformSteps::new(Some(&cs), upm, Some(&linear), upm);
    let reencode = ColorSpaceXformSteps::new(Some(&linear), upm, Some(&cs), upm);

    let alloc = ArenaAlloc::new();
    let mut p = RasterPipeline::new();
    p.append_load(ColorType::RGBAF32, MemoryCtx::new(MemSlot(0)));
    linearize.apply_to_pipeline(&mut p, &alloc);
    reencode.apply_to_pipeline(&mut p, &alloc);
    p.append_store(ColorType::RGBAF32, MemoryCtx::new(MemSlot(1)));
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&src_bytes))
        .with(MemSlot(1), MemView::write(&mut dst_bytes));
    p.run(0, 0, K_STEPS, 1, &mut mem);

    for (i, out) in rgba.iter_mut().enumerate() {
        for (c, v) in out.iter_mut().enumerate() {
            let at = 4 * (4 * i + c);
            *v = f32::from_ne_bytes(dst_bytes[at..at + 4].try_into().expect("4 bytes"));
        }
    }

    let close = |x: f32, y: f32| x == y || (x / y < 1.001 && y / x < 1.001);
    for (i, got) in rgba.iter().enumerate() {
        let want = expected(i);
        reporter_assert!(r, close(got[0], want[0]));
        reporter_assert!(r, close(got[1], want[1]));
        reporter_assert!(r, close(got[2], want[2]));
    }
});
