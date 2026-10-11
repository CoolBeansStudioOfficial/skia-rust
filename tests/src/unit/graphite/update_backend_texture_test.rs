// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/UpdateBackendTextureTest.cpp (chrome/m156)
//
// `UpdateImageBackendTextureTest` reads the mip levels back, so it runs on a real adapter
// (`def_graphite_adapter_test!`). `UpdateBackendTextureFinishedProcTest` runs on every context.

#![cfg(test)]
// Mirrors the C++ helpers, which take the reporter, the context and the recorder separately.
#![allow(clippy::too_many_arguments)]

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_max_bits_per_channel;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::gpu::ref_cnted_callback::CallbackProc;
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::graphite_types::InsertRecordingInfo;
use skia_rust_gpu::graphite::image_factories::wrap_texture_for_alpha_type;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::{WgpuCaps, WgpuContext};

use crate::tools::compare_pixels::check_solid_pixels;
use crate::tools::graphite_test_context::synced_submit;
use crate::tools::tool_utils::{PixmapLevel, colortype_name, make_pixmaps};
use crate::{
    Reporter, def_graphite_adapter_test, def_graphite_test_for_all_contexts, errorf,
    reporter_assert,
};

const K_SIZE: ISize = ISize {
    width: 32,
    height: 32,
};
const K_NUM_MIP_LEVELS: usize = 6;

const K_COLORS: [Color4f; 6] = [
    Color4f::new(1.0, 0.0, 0.0, 1.0), // R
    Color4f::new(0.0, 1.0, 0.0, 0.9), // G
    Color4f::new(0.0, 0.0, 1.0, 0.7), // B
    Color4f::new(0.0, 1.0, 1.0, 0.5), // C
    Color4f::new(1.0, 0.0, 1.0, 0.3), // M
    Color4f::new(1.0, 1.0, 0.0, 0.2), // Y
];

const K_COLORS_NEW: [Color4f; 6] = [
    Color4f::new(1.0, 1.0, 0.0, 0.2), // Y
    Color4f::new(1.0, 0.0, 0.0, 1.0), // R
    Color4f::new(0.0, 1.0, 0.0, 0.9), // G
    Color4f::new(0.0, 0.0, 1.0, 0.7), // B
    Color4f::new(0.0, 1.0, 1.0, 0.5), // C
    Color4f::new(1.0, 0.0, 1.0, 0.3), // M
];

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L19-L36 (chrome/m156)
fn check_solid_pixmap(
    reporter: &mut Reporter,
    expected: &Color4f,
    actual: &Pixmap<'_>,
    ct: ColorType,
    label: &str,
) {
    let expected = *expected;
    let mut error = |x: i32, y: i32, diffs: &[f32; 4]| {
        errorf!(
            reporter,
            "{} {} - mismatch at {}, {} expected: ({:.2}, {:.2}, {:.2}, {:.2}) - diffs: ({:.2}, \
             {:.2}, {:.2}, {:.2})",
            colortype_name(ct),
            label,
            x,
            y,
            expected.r,
            expected.g,
            expected.b,
            expected.a,
            diffs[0],
            diffs[1],
            diffs[2],
            diffs[3]
        );
    };
    // 0.5 / (2^bits) is theoretically the limit but with floating point precision 1/2^N is safer.
    let tol = 1.0f32 / (1u32 << color_type_max_bits_per_channel(ct)) as f32;
    let tols = [tol; 4];
    check_solid_pixels(&expected, actual, &tols, &mut error);
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L38-L51 (chrome/m156)
fn update_backend_texture(
    recorder: &mut Recorder,
    backend_tex: &BackendTexture,
    ct: ColorType,
    with_mips: bool,
    colors: &[Color4f; 6],
    finished_proc: Option<CallbackProc>,
) {
    let levels: Vec<PixmapLevel> = make_pixmaps(ct, AlphaType::Premul, with_mips, colors);
    let pixmaps: Vec<Pixmap<'_>> = levels.iter().map(PixmapLevel::pixmap).collect();
    assert_eq!(pixmaps[0].dimensions(), K_SIZE);
    let _ = recorder.update_backend_texture(backend_tex, &pixmaps, finished_proc);
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L53-L74 (chrome/m156)
fn create_backend_texture(
    caps: &WgpuCaps,
    recorder: &mut Recorder,
    ct: ColorType,
    with_mips: bool,
    renderable: Renderable,
    is_protected: Protected,
    colors: &[Color4f; 6],
    finished_proc: Option<CallbackProc>,
) -> BackendTexture {
    let mipmapped = if with_mips {
        Mipmapped::Yes
    } else {
        Mipmapped::No
    };
    let info = caps.get_default_sampled_texture_info(ct, mipmapped, is_protected, renderable);
    let backend_tex = recorder.create_backend_texture(K_SIZE, &info);
    if backend_tex.is_valid() {
        update_backend_texture(
            recorder,
            &backend_tex,
            ct,
            with_mips,
            colors,
            finished_proc,
        );
    } // else the color type mapped to a texture format not supported by the GPU backend
    backend_tex
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L76-L91 (chrome/m156)
fn wrap_backend_texture(
    reporter: &mut Reporter,
    recorder: &Recorder,
    backend_tex: &BackendTexture,
    force_opaque: bool,
    with_mips: bool,
) -> Option<Image> {
    // `SkImages::WrapTexture(recorder, backendTex, alphaType, nullptr)`: the colour type comes from
    // the texture, the origin is top-left and the image has no release proc.
    let alpha_type = if force_opaque {
        AlphaType::Unknown
    } else {
        AlphaType::Premul
    };
    let image = wrap_texture_for_alpha_type(
        recorder,
        backend_tex,
        alpha_type,
        None,
        Origin::TopLeft,
        None,
    );
    reporter_assert!(reporter, image.is_some());
    reporter_assert!(
        reporter,
        image.as_ref().is_some_and(|i| i.has_mipmaps() == with_mips)
    );
    image
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L93-L146 (chrome/m156)
fn check_levels(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    recorder: &Recorder,
    image: &Image,
    force_opaque: bool,
    with_mips: bool,
    colors: &[Color4f; 6],
) {
    let num_levels = if with_mips { K_NUM_MIP_LEVELS } else { 1 };
    let sampling = if with_mips {
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Nearest)
    } else {
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None)
    };
    let surface_ii = ImageInfo::new(K_SIZE, ColorType::RGBA8888, AlphaType::Premul, None);
    let surf = Surface::render_target(recorder, &surface_ii, Mipmapped::No, None, "")
        .expect("a render target surface");
    let canvas = surf.canvas();

    for i in 0..num_levels {
        let draw_size = K_SIZE.width >> i;
        if i == 5 {
            // TODO: Metal currently never draws the top-most mip-level (skbug.com/40044877)
            continue;
        }
        let readback_ii = ImageInfo::new(
            (draw_size, draw_size),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        let mut actual = Bitmap::new();
        actual.alloc_pixels_info(&readback_ii, None);
        actual.erase_color(Color::TRANSPARENT);

        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        canvas.clear(Color::TRANSPARENT);
        let dst = Rect::from_ltrb(0.0, 0.0, draw_size as f32, draw_size as f32);
        canvas.draw_image_rect_with_sampling_options(image, None, dst, sampling, &paint);

        {
            let mut pixmap = actual.peek_pixels_mut().expect("pixels");
            if !context.read_surface_pixels(&surf, &mut pixmap, 0, 0) {
                errorf!(reporter, "readPixels failed");
                return;
            }
        }

        // The mipmaps of the backend texture were defined in the test as an unpremul colour with
        // an alpha value, and converted to premul RGBA before uploading to the GPU. It is *after*
        // that premultiplication that SkImages::WrapTexture's forceOpaque applies so we expect
        // RGB to match the premul value but have A=1.
        let expected = if force_opaque {
            colors[i].premul().to_opaque().unpremul()
        } else {
            colors[i]
        };
        let label = format!("mip-level {i}");
        let pixmap = actual.peek_pixels().expect("pixels");
        check_solid_pixmap(reporter, &expected, &pixmap, image.color_type(), &label);
    }
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L195-L250 (chrome/m156)
def_graphite_adapter_test!(UpdateImageBackendTextureTest, |reporter, context| {
    let is_protected = if context.wgpu_caps().protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };
    let mut recorder = context.make_recorder(None);
    // TODO: test more than just RGBA8
    for ct in [ColorType::RGBA8888, ColorType::ARGB4444] {
        for with_mips in [true, false] {
            for force_opaque in [false, true] {
                for renderable in [Renderable::Yes, Renderable::No] {
                    let backend_tex = create_backend_texture(
                        context.wgpu_caps(),
                        &mut recorder,
                        ct,
                        with_mips,
                        renderable,
                        is_protected,
                        &K_COLORS,
                        None,
                    );
                    if !backend_tex.is_valid() {
                        continue;
                    }
                    {
                        let Some(image) = wrap_backend_texture(
                            reporter,
                            &recorder,
                            &backend_tex,
                            force_opaque,
                            with_mips,
                        ) else {
                            continue;
                        };
                        if is_protected == Protected::No {
                            check_levels(
                                reporter,
                                context,
                                &recorder,
                                &image,
                                force_opaque,
                                with_mips,
                                &K_COLORS,
                            );
                        }
                    }
                    update_backend_texture(
                        &mut recorder,
                        &backend_tex,
                        ct,
                        with_mips,
                        &K_COLORS_NEW,
                        None,
                    );
                    {
                        let Some(image) = wrap_backend_texture(
                            reporter,
                            &recorder,
                            &backend_tex,
                            force_opaque,
                            with_mips,
                        ) else {
                            continue;
                        };
                        if is_protected == Protected::No {
                            check_levels(
                                reporter,
                                context,
                                &recorder,
                                &image,
                                force_opaque,
                                with_mips,
                                &K_COLORS_NEW,
                            );
                        }
                    }
                    recorder.delete_backend_texture(&backend_tex);
                }
            }
        }
    }
});

/// The state the finished proc writes (`FinishContext` in the C++ test). The C++ proc asserts on
/// `reporter` as it runs; the port records a second call and asserts on it afterwards.
#[derive(Debug, Default)]
struct FinishContext {
    finished_update: bool,
    finished_twice: bool,
}

// Port of: tests/graphite/UpdateBackendTextureTest.cpp#L252-L310 (chrome/m156)
def_graphite_test_for_all_contexts!(UpdateBackendTextureFinishedProcTest, |reporter, context| {
    let is_protected = if context.wgpu_caps().protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };
    let mut recorder = context.make_recorder(None);

    let finish_ctx = Arc::new(Mutex::new(FinishContext::default()));
    let finish_for_proc = Arc::clone(&finish_ctx);
    let finished_proc = CallbackProc::Plain(Box::new(move || {
        let mut state = finish_for_proc
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // REPORTER_ASSERT(finishedCtx->fReporter, !(finishedCtx->fFinishedUpdate))
        if state.finished_update {
            state.finished_twice = true;
        }
        state.finished_update = true;
    }));

    let backend_tex = create_backend_texture(
        context.wgpu_caps(),
        &mut recorder,
        ColorType::RGBA8888,
        false,
        Renderable::No,
        is_protected,
        &K_COLORS,
        Some(finished_proc),
    );
    // Vulkan may use VK_EXT_host_image_copy to upload the texture data right away, so finished may
    // already be called.
    let supports_host_image_copy = ContextPriv::caps(context).supports_host_image_copy();
    if !supports_host_image_copy {
        reporter_assert!(
            reporter,
            !finish_ctx
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .finished_update
        );
    }

    let Some(mut recording) = recorder.snap() else {
        errorf!(reporter, "Failed to make recording");
        return;
    };
    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));

    if !supports_host_image_copy {
        reporter_assert!(
            reporter,
            !finish_ctx
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .finished_update
        );
    }
    synced_submit(context);
    let state = finish_ctx
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    reporter_assert!(reporter, state.finished_update);
    reporter_assert!(reporter, !state.finished_twice);
    drop(state);
    recorder.delete_backend_texture(&backend_tex);
});
