// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp (chrome/m156)
//
// The async read-back tests need a real adapter (`def_graphite_adapter_test!`).

#![cfg(test)]
// The driver mirrors the C++ integer and float arithmetic of the test, including its exact float
// comparisons and int/float casts on small integer values.
// The color type tables mirror the C++ switches, one case per color type, even where two types
// share a value.
#![allow(
    clippy::match_same_arms,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names
)]

use std::sync::{Arc, Mutex};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_channel_flags;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::async_read::{AsyncReadResult, ReadPixelsCallback};
use skia_rust_gpu::graphite::graphite_types::{InsertRecordingInfo, SubmitInfo, SyncToCpu};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;
use skia_rust_raster::surfaces;

use crate::tools::backend_texture_image_factory::make_backend_texture_image;
use crate::tools::compare_pixels::compare_pixels;
use crate::tools::tool_utils::{alphatype_name, colortype_name};
use crate::{Reporter, def_graphite_adapter_test, errorf, reporter_assert};

const K_W: i32 = 16;
const K_H: i32 = 16;

// The byte the destination is filled with before a read (`kInitialByte`).
const K_INITIAL_BYTE: u8 = 0x1B;

const K_GREEN: Color4f = Color4f::new(0.0, 1.0, 0.0, 1.0);
const K_RED: Color4f = Color4f::new(1.0, 0.0, 0.0, 1.0);
const K_BLUE: Color4f = Color4f::new(0.0, 0.0, 1.0, 1.0);
const K_BLACK: Color4f = Color4f::new(0.0, 0.0, 0.0, 1.0);
const K_WHITE: Color4f = Color4f::new(1.0, 1.0, 1.0, 1.0);

// The alpha types, in the order of `kLastEnum_SkAlphaType`.
const ALPHA_TYPES: [AlphaType; 4] = [
    AlphaType::Unknown,
    AlphaType::Opaque,
    AlphaType::Premul,
    AlphaType::Unpremul,
];

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L39-L72 (chrome/m156)
fn min_rgb_channel_bits(ct: ColorType) -> i32 {
    match ct {
        ColorType::Unknown | ColorType::Alpha8 | ColorType::A16UNorm | ColorType::A16Float => 0,
        ColorType::RGB565 => 5,
        ColorType::ARGB4444 => 4,
        ColorType::R8G8UNorm => 8,
        ColorType::R16UNorm => 16,
        ColorType::R16G16UNorm => 16,
        ColorType::R16G16Float => 16,
        ColorType::RGBA8888 => 8,
        ColorType::SRGBA8888 => 8,
        ColorType::RGB888x => 8,
        ColorType::BGRA8888 => 8,
        ColorType::RGBA1010102 => 10,
        ColorType::RGB101010x => 10,
        ColorType::BGRA1010102 => 10,
        ColorType::BGR101010x => 10,
        ColorType::BGR101010xXR => 10,
        ColorType::RGBA10x6 => 10,
        ColorType::BGRA10101010XR => 10,
        // counting gray as "rgb"
        ColorType::Gray8 => 8,
        // just counting the mantissa
        ColorType::RGBAF16Norm => 10,
        ColorType::RGBAF16 => 10,
        ColorType::RGBF16F16F16x => 10,
        ColorType::R16Float => 10,
        // just counting the mantissa
        ColorType::RGBAF32 => 23,
        ColorType::R16G16B16A16UNorm => 16,
        ColorType::R8UNorm => 8,
    }
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L74-L107 (chrome/m156)
fn alpha_channel_bits(ct: ColorType) -> i32 {
    match ct {
        ColorType::Unknown => 0,
        ColorType::Alpha8 => 8,
        ColorType::A16UNorm => 16,
        ColorType::A16Float => 16,
        ColorType::RGB565 => 0,
        ColorType::ARGB4444 => 4,
        ColorType::R8G8UNorm => 0,
        ColorType::R16UNorm => 0,
        ColorType::R16G16UNorm => 0,
        ColorType::R16G16Float => 0,
        ColorType::RGBA8888 => 8,
        ColorType::SRGBA8888 => 8,
        ColorType::RGB888x => 0,
        ColorType::BGRA8888 => 8,
        ColorType::RGBA1010102 => 2,
        ColorType::RGB101010x => 0,
        ColorType::BGRA1010102 => 2,
        ColorType::BGR101010x => 0,
        ColorType::BGR101010xXR => 0,
        ColorType::RGBA10x6 => 10,
        ColorType::BGRA10101010XR => 10,
        ColorType::Gray8 => 0,
        // just counting the mantissa
        ColorType::RGBAF16Norm => 10,
        ColorType::RGBAF16 => 10,
        ColorType::RGBF16F16F16x => 0,
        ColorType::R16Float => 0,
        // just counting the mantissa
        ColorType::RGBAF32 => 23,
        ColorType::R16G16B16A16UNorm => 16,
        ColorType::R8UNorm => 0,
    }
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L110-L138 (chrome/m156)
fn make_long_rect_array(w: i32, h: i32) -> Vec<IRect> {
    vec![
        // entire thing
        IRect::new(0, 0, w, h),
        // larger on all sides
        IRect::new(-10, -10, w + 10, h + 10),
        // fully contained
        IRect::new(w / 4, h / 4, 3 * w / 4, 3 * h / 4),
        // outside top left
        IRect::new(-10, -10, -1, -1),
        // touching top left corner
        IRect::new(-10, -10, 0, 0),
        // overlapping top left corner
        IRect::new(-10, -10, w / 4, h / 4),
        // overlapping top left and top right corners
        IRect::new(-10, -10, w + 10, h / 4),
        // touching entire top edge
        IRect::new(-10, -10, w + 10, 0),
        // overlapping top right corner
        IRect::new(3 * w / 4, -10, w + 10, h / 4),
        // contained in x, overlapping top edge
        IRect::new(w / 4, -10, 3 * w / 4, h / 4),
        // outside top right corner
        IRect::new(w + 1, -10, w + 10, -1),
        // touching top right corner
        IRect::new(w, -10, w + 10, 0),
        // overlapping top left and bottom left corners
        IRect::new(-10, -10, w / 4, h + 10),
        // touching entire left edge
        IRect::new(-10, -10, 0, h + 10),
        // overlapping bottom left corner
        IRect::new(-10, 3 * h / 4, w / 4, h + 10),
        // contained in y, overlapping left edge
        IRect::new(-10, h / 4, w / 4, 3 * h / 4),
        // outside bottom left corner
        IRect::new(-10, h + 1, -1, h + 10),
        // touching bottom left corner
        IRect::new(-10, h, 0, h + 10),
        // overlapping bottom left and bottom right corners
        IRect::new(-10, 3 * h / 4, w + 10, h + 10),
        // touching entire left edge
        IRect::new(0, h, w, h + 10),
        // overlapping bottom right corner
        IRect::new(3 * w / 4, 3 * h / 4, w + 10, h + 10),
        // overlapping top right and bottom right corners
        IRect::new(3 * w / 4, -10, w + 10, h + 10),
    ]
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L159-L168 (chrome/m156)
fn make_short_rect_array(w: i32, h: i32) -> Vec<IRect> {
    vec![
        // entire thing
        IRect::new(0, 0, w, h),
        // fully contained
        IRect::new(w / 4, h / 4, 3 * w / 4, 3 * h / 4),
        // overlapping top right corner
        IRect::new(3 * w / 4, -10, w + 10, h / 4),
    ]
}

// An owned, tightly packed or loosely packed pixel buffer (`SkAutoPixmapStorage`).
struct Buffer {
    info: ImageInfo,
    row_bytes: usize,
    data: Vec<u8>,
}

impl Buffer {
    // `alloc(info)`: a tightly packed buffer of `info`.
    fn new_tight(info: ImageInfo) -> Self {
        let row_bytes = info.min_row_bytes();
        let data = vec![0; info.compute_byte_size(row_bytes)];
        Self {
            info,
            row_bytes,
            data,
        }
    }

    // The pixels of the buffer, or `None` if its info is not valid.
    fn pixmap(&self) -> Option<Pixmap<'_>> {
        Pixmap::new_readonly(&self.info, &self.data, self.row_bytes)
    }
}

// The source of a read-back, which has the image info it was made with.
trait SourceInfo {
    fn source_info(&self) -> &ImageInfo;
}

impl SourceInfo for CoreImage {
    fn source_info(&self) -> &ImageInfo {
        self.image_info()
    }
}

impl SourceInfo for Surface {
    fn source_info(&self) -> &ImageInfo {
        self.image_info()
    }
}

// `enum class Result`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadResult {
    Fail,
    Success,
    ExcusedFailure,
}

// `struct GraphiteReadPixelTestRules`.
#[derive(Clone, Copy, Debug)]
struct ReadPixelTestRules {
    // Test unpremul sources? We could omit this and detect that creating the source of the read
    // failed but having it lets us skip generating reference color data.
    allow_unpremul_src: bool,
    // Are reads that are overlapping but not contained by the src bounds expected to succeed?
    uncontained_rect_succeeds: bool,
    origin: Origin,
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L193-L232 (chrome/m156), `make_ref_data`
fn make_ref_data(info: &ImageInfo, force_opaque: bool) -> Buffer {
    let result_info = if info.alpha_type() == AlphaType::Unknown {
        info.with_alpha_type(AlphaType::Unpremul)
    } else {
        info.clone()
    };
    let mut result = Buffer::new_tight(result_info);
    let w = info.width();
    let h = info.height();
    let row_bytes = result.row_bytes;
    let result_info = result.info.clone();
    let Some(mut surface) = surfaces::wrap_pixels(&result_info, &mut result.data, row_bytes, None)
    else {
        return result;
    };
    let canvas = surface.canvas();

    let pts1 = (Point::new(0.0, 0.0), Point::new(w as f32, h as f32));
    let colors1 = [K_GREEN, K_RED];
    let gradient1 = Gradient::new(
        Colors::new(&colors1, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    let mut paint = Paint::default();
    paint.set_shader(gradient_shaders::linear_gradient(pts1, &gradient1, None));
    canvas.draw_paint(&paint);

    let pts2 = (Point::new(w as f32, 0.0), Point::new(0.0, h as f32));
    let colors2 = [K_BLUE, K_BLACK];
    let gradient2 = Gradient::new(
        Colors::new(&colors2, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(gradient_shaders::linear_gradient(pts2, &gradient2, None));
    paint.set_blend_mode(skia_rust_core::blend_mode::BlendMode::Plus);
    canvas.draw_paint(&paint);

    // If not opaque add some fractional alpha.
    if info.alpha_type() != AlphaType::Opaque && !force_opaque {
        let colors3 = [
            K_WHITE,
            K_WHITE,
            Color4f::from(Color::new(0x60FF_FFFF)),
            K_WHITE,
            K_WHITE,
        ];
        let pos3 = [0.0, 0.15, 0.5, 0.85, 1.0];
        let gradient3 = Gradient::new(
            Colors::new(&colors3, Some(&pos3), TileMode::Mirror, None),
            Interpolation::default(),
        );
        paint.set_shader(gradient_shaders::radial_gradient(
            (
                Point::new(w as f32 / 2.0, h as f32 / 2.0),
                (w + h) as f32 / 10.0,
            ),
            &gradient3,
            None,
        ));
        paint.set_blend_mode(skia_rust_core::blend_mode::BlendMode::DstIn);
        canvas.draw_paint(&paint);
    }
    drop(surface);
    result
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L235-L244 (chrome/m156)
// `SkRectMemcpy` for `rows` rows of `bytes_per_row` bytes.
fn rect_memcpy(
    dst: &mut [u8],
    dst_row_bytes: usize,
    src: &[u8],
    src_row_bytes: usize,
    bytes_per_row: usize,
    rows: usize,
) {
    for y in 0..rows {
        let d = y * dst_row_bytes;
        let s = y * src_row_bytes;
        dst[d..d + bytes_per_row].copy_from_slice(&src[s..s + bytes_per_row]);
    }
}

// Whether every byte of `data[start..end]` is `K_INITIAL_BYTE`, clamped to the buffer.
fn all_initial(data: &[u8], start: usize, end: usize) -> bool {
    let end = end.min(data.len());
    let start = start.min(end);
    data[start..end].iter().all(|&x| x == K_INITIAL_BYTE)
}

// The read of an async rescale-and-read: the callback's state.
#[derive(Default)]
struct AsyncState {
    called: bool,
    result: Option<AsyncReadResult>,
}

// Runs `start` (which makes the asynchronous read) and waits for its callback, then copies the
// result into `dst` (`pixels.writable_addr()`, `pixels.rowBytes()`).
// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L554-L557 (chrome/m156), `async_callback`
fn async_read_into(
    context: &mut WgpuContext,
    dst_info: &ImageInfo,
    dst: &mut [u8],
    dst_row_bytes: usize,
    start: impl FnOnce(&mut WgpuContext, ReadPixelsCallback),
) -> ReadResult {
    let state = Arc::new(Mutex::new(AsyncState::default()));
    let callback_state = Arc::clone(&state);
    start(
        context,
        Box::new(move |result| {
            let mut state = callback_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.called = true;
            state.result = result;
        }),
    );
    if !state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .called
    {
        let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
    }
    loop {
        if state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .called
        {
            break;
        }
        context.shared_context().tick();
        context.check_async_work_completion();
    }

    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(result) = state.result.take() else {
        return ReadResult::Fail;
    };
    rect_memcpy(
        dst,
        dst_row_bytes,
        result.data(0),
        result.row_bytes(0),
        dst_info.min_row_bytes(),
        dst_info.height() as usize,
    );
    ReadResult::Success
}

// `GraphiteReadSrcFn<SkImage>`: reads `image` at `offset` into `dst`.
// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L559-L600 (chrome/m156), `reader`
fn image_read(
    context: &mut WgpuContext,
    image: &CoreImage,
    offset: IPoint,
    dst_info: &ImageInfo,
    dst: &mut [u8],
    dst_row_bytes: usize,
) -> ReadResult {
    let rect = IRect::from_size(dst_info.dimensions()).with_offset(offset);
    // The GPU implementation is based on rendering and will fail for non-renderable color types.
    let tex_info = context
        .shared_context()
        .caps()
        .get_default_sampled_texture_info(
            image.color_type(),
            Mipmapped::No,
            Protected::No,
            Renderable::Yes,
        );
    if !context.shared_context().caps().is_renderable(&tex_info) {
        return ReadResult::ExcusedFailure;
    }

    async_read_into(
        context,
        dst_info,
        dst,
        dst_row_bytes,
        |context, callback| {
            context.async_rescale_and_read_pixels(
                image,
                dst_info,
                rect,
                RescaleGamma::Src,
                RescaleMode::RepeatedLinear,
                callback,
            );
        },
    )
}

// `GraphiteReadSrcFn<SkSurface>`: reads `surface` at `offset` into `dst`.
// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L648-L677 (chrome/m156), `reader`
fn surface_read(
    context: &mut WgpuContext,
    surface: &Surface,
    offset: IPoint,
    dst_info: &ImageInfo,
    dst: &mut [u8],
    dst_row_bytes: usize,
) -> ReadResult {
    let rect = IRect::from_size(dst_info.dimensions()).with_offset(offset);
    async_read_into(
        context,
        dst_info,
        dst,
        dst_row_bytes,
        |context, callback| {
            context.async_rescale_and_read_pixels_surface(
                surface,
                dst_info,
                rect,
                RescaleGamma::Src,
                RescaleMode::RepeatedLinear,
                callback,
            );
        },
    )
}

// Reads `src` at `offset` into `read_info`-shaped pixels with the checks of `runTest`, and returns
// the read's result.
// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L268-L387 (chrome/m156), `runTest`
fn run_test<T: SourceInfo>(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    rules: ReadPixelTestRules,
    label: &str,
    src: &T,
    src_pixels: &Pixmap<'_>,
    read_info: &ImageInfo,
    offset: IPoint,
    read: &impl Fn(&mut WgpuContext, &T, IPoint, &ImageInfo, &mut [u8], usize) -> ReadResult,
) -> ReadResult {
    let src_info = src_pixels.info();
    let read_ct = read_info.color_type();
    let read_at = read_info.alpha_type();
    let src_ct = src_info.color_type();
    let src_at = src_info.alpha_type();
    let cs_conversion = !ColorSpace::equals(
        read_info.color_info().color_space_ref(),
        src_info.color_info().color_space_ref(),
    );
    let rect = IRect::from_size(read_info.dimensions()).with_offset(offset);
    let surf_bounds = IRect::from_size(src_info.dimensions());
    let read_bpp = read_ct.bytes_per_pixel();

    // Make the row bytes in the dst be loose for extra stress.
    let dst_row_bytes = read_bpp * read_info.width() as usize + 10 * read_bpp;
    // This will make the last row tight.
    let dst_size = read_info.compute_byte_size(dst_row_bytes);
    // Initialize with an arbitrary value for each byte. Later we will check that only the correct
    // part of the destination gets overwritten by 'read'.
    let mut dst_data = vec![K_INITIAL_BYTE; dst_size];

    let result = read(
        context,
        src,
        offset,
        read_info,
        &mut dst_data,
        dst_row_bytes,
    );

    if !IRect::intersects(&rect, &surf_bounds) {
        reporter_assert!(reporter, result != ReadResult::Success);
    } else if read_ct == ColorType::Unknown {
        reporter_assert!(reporter, result != ReadResult::Success);
    } else if read_at == AlphaType::Unknown {
        reporter_assert!(reporter, result != ReadResult::Success);
    } else if !rules.uncontained_rect_succeeds && !surf_bounds.contains(rect) {
        reporter_assert!(reporter, result != ReadResult::Success);
    } else if result == ReadResult::Fail {
        // TODO in Skia: Support BGR 101010x, BGRA 1010102, on the GPU.
        errorf!(
            reporter,
            "Read failed. {label}Src CT: {}, Src AT: {} Read CT: {}, Read AT: {}, Rect [{}, {}, {}, {}], CS conversion: {}",
            colortype_name(src_ct),
            alphatype_name(src_at),
            colortype_name(read_ct),
            alphatype_name(read_at),
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
            cs_conversion,
        );
        return result;
    }

    let guard_ok = if result == ReadResult::Success
        && let Some(src_read_rect) = IRect::intersect(&surf_bounds, &rect)
    {
        // The pixels of `dst` that should have been written.
        let dst_write_rect = src_read_rect.with_offset((-rect.left(), -rect.top()));

        let src_flags = color_type_channel_flags(src_ct);
        let read_flags = color_type_channel_flags(read_ct);
        let gray = skia_rust_core::color::ColorChannelFlag::GRAY;
        let lum_conversion =
            src_flags.bits() & gray.bits() == 0 && read_flags.bits() & gray.bits() != 0;
        let srgb_ct_conversion = src_ct == ColorType::SRGBA8888 || read_ct == ColorType::SRGBA8888;
        // A CS or luminance conversion allows a 3 value difference and otherwise a 2 value
        // difference. Note that sometimes read back on GPU can be lossy even when there no
        // conversion at all because GPU->CPU read may go to a lower bit depth format and then be
        // promoted back to the original type. For example, GL ES cannot read to 1010102, so we go
        // through 8888.
        let mut numer: f32 = if lum_conversion || cs_conversion || srgb_ct_conversion {
            3.0
        } else {
            2.0
        };
        // Allow some extra tolerance if unpremuling.
        if src_at == AlphaType::Premul && read_at == AlphaType::Unpremul {
            numer += 1.0;
        }
        let rgb_bits = min_rgb_channel_bits(read_ct)
            .min(min_rgb_channel_bits(src_ct))
            .min(8);
        let tol = numer / ((1i32 << rgb_bits) as f32);
        // ReadPixels operations may perform format, alpha, or color space conversions on the GPU,
        // which can introduce small floating-point differences. Expect a small diff in both RGB
        // and alpha.
        let alpha_bits = alpha_channel_bits(read_ct)
            .min(alpha_channel_bits(src_ct))
            .min(8);
        let alpha_tol = if alpha_bits == 0 {
            tol
        } else {
            numer / ((1i32 << alpha_bits) as f32)
        };
        let tols = [tol, tol, tol, alpha_tol];

        let ref_info = read_info.with_dimensions(dst_write_rect.size());
        let mut ref_buf = Buffer::new_tight(ref_info.clone());
        let ref_row_bytes = ref_buf.row_bytes;
        // Since the texture is uploaded as if it's top left, we flip the ref pixels if the origin
        // is bottom left so the following comparison works identically regardless of the origin.
        let flip_y = rules.origin == Origin::BottomLeft;
        let src_read_y = if flip_y {
            src_info.height() - src_read_rect.bottom()
        } else {
            src_read_rect.y()
        };
        let src_bytes = src_pixels.addr().unwrap_or(&[]);
        if read_at == AlphaType::Unknown {
            // Do a spoofed read where src and dst alpha type are both kUnpremul. This will allow
            // the read to succeed and won't do any alpha type conversion.
            let unpremul_ref_info = ref_info.with_alpha_type(AlphaType::Unpremul);
            let unpremul_src_info = src_info.with_alpha_type(AlphaType::Unpremul);
            if let Some(unpremul_src) =
                Pixmap::new_readonly(&unpremul_src_info, src_bytes, src_pixels.row_bytes())
            {
                let _ = unpremul_src.read_pixels(
                    &unpremul_ref_info,
                    &mut ref_buf.data,
                    ref_row_bytes,
                    (src_read_rect.x(), src_read_y),
                );
            }
        } else {
            let _ = src_pixels.read_pixels(
                &ref_info,
                &mut ref_buf.data,
                ref_row_bytes,
                (src_read_rect.x(), src_read_y),
            );
        }
        if flip_y {
            let h = ref_info.height() as usize;
            for y in 0..h / 2 {
                let (top, bottom) = ref_buf.data.split_at_mut((h - 1 - y) * ref_row_bytes);
                top[y * ref_row_bytes..(y + 1) * ref_row_bytes]
                    .swap_with_slice(&mut bottom[..ref_row_bytes]);
            }
        }

        // This is the part of dst_pixels that should have been updated.
        let actual_info = read_info.with_dimensions(dst_write_rect.size());
        let actual_offset = dst_write_rect.top() as usize * dst_row_bytes
            + dst_write_rect.left() as usize * read_bpp;
        let actual = Pixmap::new_readonly(&actual_info, &dst_data[actual_offset..], dst_row_bytes);
        let ref_pixmap = ref_buf.pixmap();
        if let (Some(ref_pixmap), Some(actual)) = (ref_pixmap, actual) {
            let mut error = |x: i32, y: i32, diffs: &[f32; 4]| {
                errorf!(
                    reporter,
                    "{label}Src CT: {}, Src AT: {}, Read CT: {}, Read AT: {}, Rect [{}, {}, {}, {}], CS conversion: {}\nError at {}, {}. Diff in floats: ({:.6}, {:.6}, {:.6}, {:.6})",
                    colortype_name(src_ct),
                    alphatype_name(src_at),
                    colortype_name(read_ct),
                    alphatype_name(read_at),
                    rect.left(),
                    rect.top(),
                    rect.right(),
                    rect.bottom(),
                    cs_conversion,
                    x,
                    y,
                    diffs[0],
                    diffs[1],
                    diffs[2],
                    diffs[3],
                );
            };
            compare_pixels(&ref_pixmap, &actual, &tols, &mut error);
        } else {
            errorf!(reporter, "{label}Could not make the comparison pixmaps");
        }

        // The bytes outside the written rect must still be the initial byte.
        let row = dst_row_bytes;
        let top = dst_write_rect.top() as usize;
        let bottom = dst_write_rect.bottom() as usize;
        let left_bytes = dst_write_rect.left() as usize * read_bpp;
        let right_bytes = dst_write_rect.right() as usize * read_bpp;
        let total = dst_size;
        let mut guard_ok = all_initial(&dst_data, 0, top * row);
        let mut v = top * row;
        for _ in top..bottom {
            guard_ok |= all_initial(&dst_data, v, v + left_bytes);
            let pad = v + right_bytes;
            let row_end = total.min(v + row);
            guard_ok |= all_initial(&dst_data, pad, row_end);
            v = row_end;
        }
        guard_ok |= all_initial(&dst_data, v, total);
        guard_ok
    } else {
        all_initial(&dst_data, 0, dst_size)
    };
    if !guard_ok {
        errorf!(
            reporter,
            "Result pixels modified result outside read rect [{}, {}, {}, {}]. {label}Src CT: {}, Read CT: {}, CS conversion: {}",
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
            colortype_name(src_ct),
            colortype_name(read_ct),
            cs_conversion,
        );
    }
    result
}

// The color types of `kLastEnum_SkColorType`, in order.
fn color_types() -> impl Iterator<Item = ColorType> {
    (0..=ColorType::LAST_ENUM as i32).filter_map(ColorType::from_i32)
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L235-L541 (chrome/m156),
// `graphite_read_pixels_test_driver`
fn graphite_read_pixels_test_driver<T: SourceInfo>(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    rules: ReadPixelTestRules,
    src_factory: impl Fn(&mut WgpuContext, &mut Recorder, &Pixmap<'_>) -> Option<T>,
    read: impl Fn(&mut WgpuContext, &T, IPoint, &ImageInfo, &mut [u8], usize) -> ReadResult,
    label: &str,
) {
    let label = if label.is_empty() {
        String::new()
    } else {
        // Add space for printing.
        format!("{label} ")
    };

    let long_rect_array = make_long_rect_array(K_W, K_H);
    let short_rect_array = make_short_rect_array(K_W, K_H);

    // We ensure we use the long array once per src and read color type and otherwise use the short
    // array to improve test run time. Also, some color types have no alpha values and thus Opaque
    // Premul and Unpremul are equivalent. Just ensure each redundant AT is tested once with each
    // CT (src and read). Similarly, alpha-only color types behave the same for all alpha types so
    // just test premul after one iter. We consider a src or read CT thoroughly tested once it has
    // run through the long rect array and full complement of alpha types with one successful read
    // in the loop.
    let mut src_ct_tested_thoroughly = [false; ColorType::COUNT];
    let mut read_ct_tested_thoroughly = [false; ColorType::COUNT];

    let srgb = || Some(ColorSpace::new_srgb());
    for &src_at in &ALPHA_TYPES {
        if src_at == AlphaType::Unpremul && !rules.allow_unpremul_src {
            continue;
        }
        for src_ct in color_types() {
            // We always make our ref data as F32.
            let ref_info = ImageInfo::new((K_W, K_H), ColorType::RGBAF32, src_at, srgb());
            // 1010102 formats have an issue where it's easy to make a resulting color where r, g,
            // or b is greater than a. CPU/GPU differ in whether the stored color channels are
            // clipped to the alpha value. CPU clips but GPU does not. Note that we only currently
            // use srcCT for the 1010102 workaround.
            let force_opaque = src_at == AlphaType::Premul
                && (src_ct == ColorType::RGBA1010102 || src_ct == ColorType::BGRA1010102);

            let mut ref_pixels = make_ref_data(&ref_info, force_opaque);
            // Convert the ref data to our desired src color type.
            let mut src_info = ImageInfo::new((K_W, K_H), src_ct, src_at, srgb());
            let mut src_storage = Buffer::new_tight(src_info.clone());
            {
                // Spoof the alpha type to kUnpremul so the read will succeed without doing any
                // conversion (because we made our surface also use kUnpremul).
                let read_info = if src_at == AlphaType::Unknown {
                    src_storage.info.with_alpha_type(AlphaType::Unpremul)
                } else {
                    src_storage.info.clone()
                };
                let src_row_bytes = src_storage.row_bytes;
                if let Some(ref_pm) = ref_pixels.pixmap() {
                    let _ = ref_pm.read_pixels(
                        &read_info,
                        &mut src_storage.data,
                        src_row_bytes,
                        (0, 0),
                    );
                }
            }

            let mut recorder = context.make_recorder(None);

            let Some(src_pm) = src_storage.pixmap() else {
                continue;
            };
            let Some(src) = src_factory(context, &mut recorder, &src_pm) else {
                continue;
            };
            drop(src_pm);

            let src_ct_is_alpha_only =
                skia_rust_core::image_info_priv::color_type_is_alpha_only(src_ct);
            let image_ct = src.source_info().color_type();
            let image_info = src.source_info().clone();
            if src_ct_is_alpha_only
                && !skia_rust_core::image_info_priv::color_type_is_alpha_only(image_ct)
            {
                // Alpha type changed texture's interpretation to red, so we don't need to regen
                // the expected source data, we'll just "cast" it to the red version.
                src_info = image_info;
            } else if !src_ct_is_alpha_only
                && skia_rust_core::image_info_priv::color_type_is_alpha_only(image_ct)
            {
                // Mirror of the above, red type to alpha-only based on the alpha type.
                src_info = image_info;
            } else if src_at == AlphaType::Unknown {
                // Graphite will be forcing this to opaque on readback, so regenerate the expected
                // data to be opaque.
                src_info = src_info.with_alpha_type(AlphaType::Opaque);
                src_storage = Buffer::new_tight(src_info.clone());
                ref_pixels = make_ref_data(&ref_info, true);
                let src_row_bytes = src_storage.row_bytes;
                if let Some(ref_pm) = ref_pixels.pixmap() {
                    let _ =
                        ref_pm.read_pixels(&src_info, &mut src_storage.data, src_row_bytes, (0, 0));
                }
            }

            let Some(src_pixels) =
                Pixmap::new_readonly(&src_info, &src_storage.data, src_storage.row_bytes)
            else {
                continue;
            };
            let src_ct_index = src_ct as usize;
            if src_ct.is_always_opaque()
                && src_ct_tested_thoroughly[src_ct_index]
                && (src_at == AlphaType::Premul || src_at == AlphaType::Unpremul)
            {
                continue;
            }
            if src_ct_is_alpha_only
                && src_ct_tested_thoroughly[src_ct_index]
                && (src_at == AlphaType::Unpremul
                    || src_at == AlphaType::Opaque
                    || src_at == AlphaType::Unknown)
            {
                continue;
            }

            for read_ct in color_types() {
                // ComparePixels will end up converting these types to kUnknown because there's no
                // corresponding GrColorType, and hence it will fail.
                if read_ct == ColorType::BGR101010xXR
                    || read_ct == ColorType::BGRA10101010XR
                    || read_ct == ColorType::BGR101010x
                {
                    continue;
                }
                let read_ct_index = read_ct as usize;
                for read_cs in [ColorSpace::new_srgb(), ColorSpace::new_srgb_linear()] {
                    for &read_at in &ALPHA_TYPES {
                        if src_at != AlphaType::Opaque && read_at == AlphaType::Opaque {
                            // This doesn't make sense.
                            continue;
                        }
                        if read_ct.is_always_opaque()
                            && read_ct_tested_thoroughly[read_ct_index]
                            && (read_at == AlphaType::Premul || read_at == AlphaType::Unpremul)
                        {
                            continue;
                        }
                        if skia_rust_core::image_info_priv::color_type_is_alpha_only(read_ct)
                            && read_ct_tested_thoroughly[read_ct_index]
                            && (read_at == AlphaType::Unpremul
                                || read_at == AlphaType::Opaque
                                || read_at == AlphaType::Unknown)
                        {
                            continue;
                        }

                        let rects = if src_ct_tested_thoroughly[src_ct_index]
                            && read_ct_tested_thoroughly[read_ct_index]
                        {
                            &short_rect_array
                        } else {
                            &long_rect_array
                        };
                        for rect in rects {
                            let read_info = ImageInfo::new(
                                (rect.width(), rect.height()),
                                read_ct,
                                read_at,
                                Some(read_cs.clone()),
                            );
                            let offset = IPoint::new(rect.left(), rect.top());
                            let result = run_test(
                                reporter,
                                context,
                                rules,
                                &label,
                                &src,
                                &src_pixels,
                                &read_info,
                                offset,
                                &read,
                            );
                            if result == ReadResult::Success {
                                src_ct_tested_thoroughly[src_ct_index] = true;
                                read_ct_tested_thoroughly[read_ct_index] = true;
                            }
                        }
                    }
                }
            }
        }
    }
}

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L559-L646 (chrome/m156)
def_graphite_adapter_test!(ImageAsyncReadPixelsGraphite, |reporter, context| {
    let mut rules = ReadPixelTestRules {
        allow_unpremul_src: true,
        uncontained_rect_succeeds: false,
        origin: Origin::TopLeft,
    };

    for origin in [Origin::TopLeft, Origin::BottomLeft] {
        rules.origin = origin;
        for renderable in [Renderable::No, Renderable::Yes] {
            let src_factory = move |context: &mut WgpuContext,
                                    recorder: &mut Recorder,
                                    src: &Pixmap<'_>|
                  -> Option<CoreImage> {
                let image = make_backend_texture_image(
                    context,
                    recorder,
                    src,
                    Mipmapped::No,
                    renderable,
                    origin,
                    Protected::No,
                );

                if let Some(mut recording) = recorder.snap() {
                    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
                }
                image
            };
            let label = format!(
                "Renderable: {}, Origin: {}",
                i32::from(renderable == Renderable::Yes),
                i32::from(origin == Origin::BottomLeft),
            );
            graphite_read_pixels_test_driver(
                reporter,
                context,
                rules,
                src_factory,
                image_read,
                &label,
            );
        }
    }

    // It's possible that we've created an Image using the factory, but then don't try to do
    // readPixels on it, leaving a hanging command buffer. So we submit here to clean up.
    let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
});

// Port of: tests/graphite/ReadWritePixelsGraphiteTest.cpp#L648-L712 (chrome/m156)
def_graphite_adapter_test!(SurfaceAsyncReadPixelsGraphite, |reporter, context| {
    let rules = ReadPixelTestRules {
        allow_unpremul_src: true,
        uncontained_rect_succeeds: false,
        origin: Origin::TopLeft,
    };

    let src_factory =
        |context: &mut WgpuContext, recorder: &mut Recorder, src: &Pixmap<'_>| -> Option<Surface> {
            let surface = Surface::render_target(recorder, src.info(), Mipmapped::No, None, "");
            if let Some(surface) = &surface {
                let _ = surface.canvas().write_pixels(
                    src.info(),
                    src.addr().unwrap_or(&[]),
                    src.row_bytes(),
                    (0, 0),
                );

                if let Some(mut recording) = recorder.snap() {
                    let _ = context.insert_recording(InsertRecordingInfo::new(&mut recording));
                }
            }
            surface
        };
    graphite_read_pixels_test_driver(reporter, context, rules, src_factory, surface_read, "");

    // It's possible that we've created an Image using the factory, but then don't try to do
    // readPixels on it, leaving a hanging command buffer. So we submit here to clean up.
    let _ = context.submit(SubmitInfo::new(SyncToCpu::No));
});
