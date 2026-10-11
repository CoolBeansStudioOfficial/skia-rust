// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ImageWrapTextureMipmapsTest.cpp (chrome/m156)
//
// The test reads the surface back, so it runs on a real adapter (`def_graphite_adapter_test!`).

#![cfg(test)]

use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{RescaleGamma, RescaleMode};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::IRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::graphite_types::{
    InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use skia_rust_gpu::graphite::image_factories::{
    GenerateMipmapsFromBase, wrap_texture_for_alpha_type_with_mipmaps,
};
use skia_rust_gpu::graphite::recording::Recording;
use skia_rust_gpu::graphite::surface_graphite::Surface;

use crate::tools::managed_graphite_texture::ManagedGraphiteTexture;
use crate::{def_graphite_adapter_test, errorf, reporter_assert};

// The state the read-back callback writes (`struct ReadContext` in the C++ test).
#[derive(Default)]
struct ReadContext {
    called: bool,
    success: bool,
    color: u32,
}

// `struct TestCase` in the C++ test.
struct TestCase {
    gen_mipmaps: GenerateMipmapsFromBase,
    expected_color: u32,
}

def_graphite_adapter_test!(ImageWrapTextureMipmapsTest, |reporter, context| {
    let mut recorder = context.make_recorder(None);

    let is_protected = if context.shared_context().caps().protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };

    let info = ImageInfo::new((2, 1), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut base_bitmap = Bitmap::new();
    base_bitmap.alloc_pixels_info(&info, None);
    base_bitmap.erase_color(Color::GREEN);
    let mut top_bitmap = Bitmap::new();
    top_bitmap.alloc_pixels_info(&info.with_dimensions((1, 1)), None);
    top_bitmap.erase_color(Color::BLUE);

    let (Some(base_pm), Some(top_pm)) = (base_bitmap.peek_pixels(), top_bitmap.peek_pixels())
    else {
        errorf!(reporter, "Could not make pixmaps");
        return;
    };
    let level_pms = [base_pm, top_pm];
    let Some(mbet) = ManagedGraphiteTexture::make_mipmapped_from_pixmaps(
        context,
        &mut recorder,
        &level_pms,
        Renderable::No,
        is_protected,
    ) else {
        errorf!(reporter, "Could not make backend texture");
        return;
    };
    let mbet = Arc::new(mbet);

    // The recordings are kept until the test ends: Skia hands each one to a finished proc that
    // deletes it once the GPU is done with it.
    let mut recordings: Vec<Recording> = Vec::new();

    let Some(recording) = recorder.snap() else {
        errorf!(reporter, "Could not snap recording");
        return;
    };
    recordings.push(recording);
    let inserted = context.insert_recording(InsertRecordingInfo::new(
        recordings.last_mut().expect("the recording just pushed"),
    ));
    if inserted != InsertStatus::Success {
        errorf!(reporter, "Could not insert recording");
        return;
    }

    let test_cases = [
        TestCase {
            gen_mipmaps: GenerateMipmapsFromBase::No,
            expected_color: 0xFFFF_0000,
        },
        TestCase {
            gen_mipmaps: GenerateMipmapsFromBase::Yes,
            expected_color: 0xFF00_FF00,
        },
    ];

    for test_case in &test_cases {
        recorder = context.make_recorder(None);

        let Some(image) = wrap_texture_for_alpha_type_with_mipmaps(
            &recorder,
            mbet.texture(),
            info.alpha_type(),
            info.color_space(),
            Origin::TopLeft,
            test_case.gen_mipmaps,
            Some(mbet.release_context()),
        ) else {
            errorf!(reporter, "Could not make image");
            return;
        };

        // We determine the contents of the image's top level by doing a downsampling draw to a
        // surface and then reading the surface's contents.
        let Some(surface) = Surface::render_target(
            &recorder,
            &info.with_dimensions((1, 1)),
            Mipmapped::No,
            None,
            "",
        ) else {
            errorf!(reporter, "Could not make surface");
            return;
        };

        let Some(shader) = image.to_shader(
            Some((TileMode::Repeat, TileMode::Repeat)),
            SamplingOptions::new(FilterMode::Nearest, MipmapMode::Nearest),
            None,
        ) else {
            errorf!(reporter, "Could not make shader");
            return;
        };

        surface.canvas().scale((0.05, 0.05));
        let mut paint = Paint::default();
        paint.set_shader(shader);
        surface.canvas().draw_paint(&paint);

        let Some(recording) = recorder.snap() else {
            errorf!(reporter, "Could not snap recording");
            return;
        };
        recordings.push(recording);
        let inserted = context.insert_recording(InsertRecordingInfo::new(
            recordings.last_mut().expect("the recording just pushed"),
        ));
        if inserted != InsertStatus::Success {
            errorf!(reporter, "Could not insert recording");
            return;
        }

        let read_context = Arc::new(Mutex::new(ReadContext::default()));
        let callback_context = Arc::clone(&read_context);
        let rect = IRect::from_size(surface.image_info().dimensions());
        context.async_rescale_and_read_pixels_surface(
            &surface,
            surface.image_info(),
            rect,
            RescaleGamma::Src,
            RescaleMode::Nearest,
            Box::new(move |result| {
                let mut read = callback_context
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                read.called = true;
                if let Some(result) = result {
                    read.success = true;
                    // `*static_cast<const uint32_t*>(result->data(0))`: the pixel in native order.
                    let bytes: [u8; 4] = result.data(0)[..4]
                        .try_into()
                        .expect("a 1x1 RGBA8888 pixel is four bytes");
                    read.color = u32::from_ne_bytes(bytes);
                }
            }),
        );
        let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
        loop {
            let called = read_context
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .called;
            if called {
                break;
            }
            context.check_async_work_completion();
        }

        let read = read_context
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !read.success {
            errorf!(reporter, "Read pixels failed");
            return;
        }

        reporter_assert!(reporter, read.color == test_case.expected_color);
    }
});
