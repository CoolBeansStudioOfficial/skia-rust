// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/F16StagesTest.cpp

// Port of: tests/F16StagesTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline`; the `MemoryCtx { pixels, stride }`
// of each buffer is a `MemoryCtx` bound to a `MemSlot`, with the bytes bound per run through
// `MemoryBindings` (the buffers are read by one pipeline and written by the other).

use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};

use crate::{def_test, reporter_assert};

// Port of: tests/F16StagesTest.cpp#L15-L57 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp, clippy::identity_op, clippy::verbose_bit_mask)]
    // the C++ compares floats with == and spells out every shift and mask
    F16Stages,
    |r| {
        // Make sure SkRasterPipelineOp::load_f16 and store_f16 can handle a range of
        // ordinary (0<=x<=1) and interesting (x<0, x>1) values.
        let mut floats: [f32; 16] = [
            0.0, 0.25, 0.5, 1.0, //
            -1.25, -0.5, 1.25, 2.0, //
            0.0, 0.0, 0.0,
            0.0, // pad a bit to make sure we qualify for platform-specific code
            0.0, 0.0, 0.0, 0.0,
        ];
        let mut halfs: [u64; 4] = [0, 0, 0, 0];

        let f32_ctx = MemoryCtx::new(MemSlot(0));
        let f16_ctx = MemoryCtx::new(MemSlot(1));

        let float_bytes =
            |f: &[f32; 16]| -> Vec<u8> { f.iter().flat_map(|v| v.to_ne_bytes()).collect() };
        let half_bytes =
            |h: &[u64; 4]| -> Vec<u8> { h.iter().flat_map(|v| v.to_ne_bytes()).collect() };

        {
            let mut p = RasterPipeline::new();
            p.append(Stage::LoadF32(f32_ctx));
            p.append(Stage::StoreF16(f16_ctx));
            let src = float_bytes(&floats);
            let mut dst = half_bytes(&halfs);
            p.run(
                0,
                0,
                16 / 4,
                1,
                &mut MemoryBindings::new()
                    .with(MemSlot(0), MemView::read(&src))
                    .with(MemSlot(1), MemView::write(&mut dst)),
            );
            for (h, bytes) in halfs.iter_mut().zip(dst.as_chunks::<8>().0.iter()) {
                *h = u64::from_ne_bytes(*bytes);
            }
        }
        reporter_assert!(r, ((halfs[0] >> 0) & 0xffff) == 0x0000);
        reporter_assert!(r, ((halfs[0] >> 16) & 0xffff) == 0x3400);
        reporter_assert!(r, ((halfs[0] >> 32) & 0xffff) == 0x3800);
        reporter_assert!(r, ((halfs[0] >> 48) & 0xffff) == 0x3c00);
        reporter_assert!(r, ((halfs[1] >> 0) & 0xffff) == 0xbd00);
        reporter_assert!(r, ((halfs[1] >> 16) & 0xffff) == 0xb800);
        reporter_assert!(r, ((halfs[1] >> 32) & 0xffff) == 0x3d00);
        reporter_assert!(r, ((halfs[1] >> 48) & 0xffff) == 0x4000);

        {
            let mut p = RasterPipeline::new();
            p.append(Stage::LoadF16(f16_ctx));
            p.append(Stage::StoreF32(f32_ctx));
            let src = half_bytes(&halfs);
            let mut dst = float_bytes(&floats);
            p.run(
                0,
                0,
                16 / 4,
                1,
                &mut MemoryBindings::new()
                    .with(MemSlot(1), MemView::read(&src))
                    .with(MemSlot(0), MemView::write(&mut dst)),
            );
            for (f, bytes) in floats.iter_mut().zip(dst.as_chunks::<4>().0.iter()) {
                *f = f32::from_ne_bytes(*bytes);
            }
        }
        reporter_assert!(r, floats[0] == 0.00);
        reporter_assert!(r, floats[1] == 0.25);
        reporter_assert!(r, floats[2] == 0.50);
        reporter_assert!(r, floats[3] == 1.00);
        reporter_assert!(r, floats[4] == -1.25);
        reporter_assert!(r, floats[5] == -0.50);
        reporter_assert!(r, floats[6] == 1.25);
        reporter_assert!(r, floats[7] == 2.00);
    }
);
