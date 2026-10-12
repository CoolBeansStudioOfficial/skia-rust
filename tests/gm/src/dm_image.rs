// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: dm/DMSrcSink.{h,cpp} (CodecSrc, AndroidCodecSrc, ImageGenSrc, BRDSrc),
// dm/DM.cpp (push_codec_srcs and friends), client_utils/android/BitmapRegionDecoder.cpp

//! DM's `--src image`: the sources that decode the files of `resources/images` in every codec
//! mode, colour type, alpha type and scale, and draw the result into a raster sink.
//!
//! [`gather`] is `push_codec_srcs`: the list of sources DM makes for one file. Each source is a
//! [`Src`], so the raster sink and the per-tier checker ([`crate::check::check_gm`]) draw it as
//! they draw a GM. Its result id is `<config>/image/<options>/<name>`
//! ([`Src::id_dir`], [`Src::name`]), where `<options>` is the `folder` DM's `push_*_src` builds
//! (`codec_premul`, `scaled_codec_kGray8_unpremul_0.500`, `brd_android_codec_divisor`, …).
//!
//! DM at the pin instantiates, for `--src image`, `CodecSrc`, `AndroidCodecSrc`, `BRDSrc` (the
//! oracle build defines `SK_ENABLE_ANDROID_UTILS`, as its `brd_android_codec*` results show) and
//! `ImageGenSrc`. The GPU image generator sources and the platform generators (WIC, CG, NDK) are
//! not raster sources on the Linux oracle, so they produce no result and are not made.

// The C++ converts between int, float and unsigned as written (sizes, scales, sample sizes): the
// values are small image dimensions, so the conversions are exact.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::path::{Path, PathBuf};

use skia_rust_codec::android_codec::{AndroidCodec, AndroidOptions};
use skia_rust_codec::codec::{NO_FRAME, Options, Result as CodecResult, ZeroInitialized};
use skia_rust_codec::codec_image_generator::CodecImageGenerator;
use skia_rust_codec::{Codec, codecs};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

use crate::canvas::Canvas;
use crate::sink::{DmResult, Src, Status};

fn ok() -> DmResult {
    DmResult::new(Status::Ok, String::new())
}

fn skip(msg: impl Into<String>) -> DmResult {
    DmResult::new(Status::Skip, msg.into())
}

fn fatal(msg: impl Into<String>) -> DmResult {
    DmResult::new(Status::Fatal, msg.into())
}

/// `SkOSPath::Basename`.
fn basename(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `SkData::MakeFromFileName`.
fn read_data(path: &Path) -> Option<Data> {
    std::fs::read(path).ok().map(Data::new_from_vec)
}

/// `SkCodec::MakeFromData`.
fn make_codec(data: &Data) -> Option<Codec<'static>> {
    codecs::make_codec_from_stream(MemoryStream::make(Some(data.clone()))).ok()
}

/// `SkAndroidCodec::MakeFromData`.
fn make_android_codec(data: &Data) -> Option<AndroidCodec<'static>> {
    AndroidCodec::make_from_stream(MemoryStream::make(Some(data.clone())))
}

/// `get_scaled_name`: `SkStringPrintf("%s_%.3f", name, scale)`.
// Port of: dm/DMSrcSink.cpp#L225-L227 (chrome/m156)
fn get_scaled_name(name: &str, scale: f32) -> String {
    format!("{name}_{:.3}", f64::from(scale))
}

/// `1 / fScale` or `1.0f / (float) sampleSize` as the folder and name suffix: `%.3f`.
fn scale_suffix(scale: f32) -> String {
    format!("_{:.3}", f64::from(scale))
}

/// `SkMin`/`SkAlign2`: rounds up to an even number.
fn align2(x: i32) -> i32 {
    (x + 1) & !1
}

/// `sk_float_ceil2int`.
fn float_ceil2int(x: f32) -> i32 {
    skia_rust_core::floating_point::float_ceil2int(x)
}

/// `CodecSrc::DstColorType`.
// Port of: dm/DMSrcSink.h#L256-L260 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DstColorType {
    GetFromCanvas,
    Grayscale,
    NonNative8888,
}

/// `CodecSrc::Mode`.
// Port of: dm/DMSrcSink.h#L240-L249 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecMode {
    /// Decode to a pixel buffer with `getPixels`.
    Codec,
    /// The same into a zeroed buffer, telling the codec so.
    CodecZeroInit,
    /// `startScanlineDecode`/`getScanlines` (or incremental decode).
    Scanline,
    /// Skips and reads stripes of 37 rows.
    Stripe,
    /// Decodes 36-wide column subsets with the scanline decoder.
    CroppedScanline,
    /// Decodes quarter subsets with `getPixels`.
    Subset,
    /// Every frame of an animation, drawn in a grid.
    Animated,
}

/// `swap_rb_if_necessary`: tests decodes to non-native 8888.
// Port of: dm/DMSrcSink.cpp#L437-L446 (chrome/m156)
fn swap_rb_if_necessary(
    pixels: &mut [u8],
    info: &ImageInfo,
    row_bytes: usize,
    dst_color_type: DstColorType,
) {
    if DstColorType::NonNative8888 != dst_color_type {
        return;
    }
    // SkOpts::RGBA_to_BGRA on each row: swap the red and blue byte of every pixel.
    for y in 0..info.height() as usize {
        let row = &mut pixels[y * row_bytes..y * row_bytes + info.width() as usize * 4];
        for px in row.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
    }
}

/// `get_decode_info`: the info to decode to for a canvas color type, or `None` to skip the test.
// Port of: dm/DMSrcSink.cpp#L448-L480 (chrome/m156)
fn get_decode_info(
    decode_info: &ImageInfo,
    canvas_color_type: ColorType,
    dst_color_type: DstColorType,
    dst_alpha_type: AlphaType,
) -> Option<ImageInfo> {
    let info = match dst_color_type {
        DstColorType::Grayscale => {
            if ColorType::RGB565 == canvas_color_type {
                return None;
            }
            decode_info.with_color_type(ColorType::Gray8)
        }
        DstColorType::NonNative8888 => {
            if ColorType::RGB565 == canvas_color_type || ColorType::RGBAF16 == canvas_color_type {
                return None;
            }
            // #ifdef SK_PMCOLOR_IS_RGBA: the oracle goldens of 8888 are the RGBA build's.
            if ColorType::N32 == ColorType::RGBA8888 {
                decode_info.with_color_type(ColorType::BGRA8888)
            } else {
                decode_info.with_color_type(ColorType::RGBA8888)
            }
        }
        DstColorType::GetFromCanvas => {
            if ColorType::RGB565 == canvas_color_type
                && AlphaType::Opaque != decode_info.alpha_type()
            {
                return None;
            }
            decode_info.with_color_type(canvas_color_type)
        }
    };
    Some(info.with_alpha_type(dst_alpha_type))
}

/// `draw_to_canvas`: installs the pixels in a bitmap and draws it at `(left, top)`.
///
/// skia-rust: the C++ bitmap borrows `pixels` and the swap for non-native 8888 changes them in
/// place; here the swap is done in `pixels` too (a later frame may be decoded over them), then
/// the bitmap gets a copy.
// Port of: dm/DMSrcSink.cpp#L482-L491 (chrome/m156)
fn draw_to_canvas(
    canvas: &Canvas,
    info: &ImageInfo,
    pixels: &mut [u8],
    row_bytes: usize,
    dst_color_type: DstColorType,
    left: f32,
    top: f32,
) {
    swap_rb_if_necessary(pixels, info, row_bytes, dst_color_type);
    let size = info.compute_byte_size(row_bytes);
    let mut bitmap = Bitmap::new();
    let installed = bitmap.install_pixels(info, pixels[..size].to_vec(), row_bytes);
    assert!(installed, "installPixels failed for {info:?}");
    if let Some(image) = bitmap.as_image() {
        canvas.draw_image(image, (left, top), None);
    }
}

/// `set_bitmap_color_space`: the bitmap pretends to be sRGB, so drawing it does not convert.
// Port of: dm/DMSrcSink.cpp#L496-L498 (chrome/m156)
fn set_bitmap_color_space(info: &ImageInfo) -> ImageInfo {
    info.with_color_space(ColorSpace::new_srgb())
}

/// The bitmap info of a decode: sRGB, and `kN32` for the two 8888 orders.
fn bitmap_info_for(decode_info: &ImageInfo) -> ImageInfo {
    let info = set_bitmap_color_space(decode_info);
    if ColorType::RGBA8888 == decode_info.color_type()
        || ColorType::BGRA8888 == decode_info.color_type()
    {
        info.with_color_type(ColorType::N32)
    } else {
        info
    }
}

/// `CodecSrc`: decodes with `SkCodec` in one of the [`CodecMode`]s.
// Port of: dm/DMSrcSink.h#L236-L278 (chrome/m156)
#[derive(Clone, Debug)]
pub struct CodecSrc {
    path: PathBuf,
    options: String,
    mode: CodecMode,
    dst_color_type: DstColorType,
    dst_alpha_type: AlphaType,
    scale: f32,
}

impl CodecSrc {
    /// `push_codec_src`: the source and its result folder.
    // Port of: dm/DM.cpp#L533-L595 (chrome/m156)
    #[must_use]
    pub fn new(
        path: &Path,
        mode: CodecMode,
        dst_color_type: DstColorType,
        dst_alpha_type: AlphaType,
        scale: f32,
    ) -> Self {
        let mut folder = String::new();
        folder.push_str(match mode {
            CodecMode::Codec => "codec",
            CodecMode::CodecZeroInit => "codec_zero_init",
            CodecMode::Scanline => "scanline",
            CodecMode::Stripe => "stripe",
            CodecMode::CroppedScanline => "crop",
            CodecMode::Subset => "codec_subset",
            CodecMode::Animated => "codec_animated",
        });
        match dst_color_type {
            DstColorType::Grayscale => folder.push_str("_kGray8"),
            DstColorType::NonNative8888 => folder.push_str("_kNonNative"),
            DstColorType::GetFromCanvas => {}
        }
        match dst_alpha_type {
            AlphaType::Premul => folder.push_str("_premul"),
            AlphaType::Unpremul => folder.push_str("_unpremul"),
            _ => {}
        }
        // 1.0f != scale
        #[allow(clippy::float_cmp)] // the scales are the literals of push_codec_srcs
        if 1.0f32 != scale {
            folder.push_str(&scale_suffix(scale));
        }
        Self {
            path: path.to_path_buf(),
            options: folder,
            mode,
            dst_color_type,
            dst_alpha_type,
            scale,
        }
    }

    #[allow(clippy::float_cmp)] // the scales are the literals of push_codec_srcs
    fn is_scaled(&self) -> bool {
        1.0f32 != self.scale
    }
}

impl Src for CodecSrc {
    // Port of: dm/DMSrcSink.cpp#L500-L851 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in C++ too
    fn draw(&self, canvas: &Canvas) -> DmResult {
        let path = self.path.display();
        let Some(encoded) = read_data(&self.path) else {
            return fatal(format!("Couldn't read {path}."));
        };
        let Some(mut codec) = make_codec(&encoded) else {
            return fatal(format!("Couldn't create codec for {path}."));
        };

        let Some(mut decode_info) = get_decode_info(
            &codec.info(),
            canvas.image_info().color_type(),
            self.dst_color_type,
            self.dst_alpha_type,
        ) else {
            return skip("Skipping uninteresting test.");
        };

        // Try to scale the image if it is desired
        let mut size = codec.get_scaled_dimensions(self.scale);

        let mut android_codec: Option<AndroidCodec<'static>> = None;
        if self.is_scaled() && self.mode == CodecMode::Animated {
            android_codec = make_android_codec(&encoded);
            let Some(android) = android_codec.as_ref() else {
                return fatal(format!("Couldn't create android codec for {path}."));
            };
            // getSampledDimensions(1 / fScale): the float converts to int
            size = android.get_sampled_dimensions((1.0 / self.scale) as i32);
        }

        if size == decode_info.dimensions() && self.is_scaled() {
            return skip("Test without scaling is uninteresting.");
        }

        // Visually inspecting very small output images is not necessary.  We will
        // cover these cases in unit testing.
        if (size.width <= 10 || size.height <= 10) && self.is_scaled() {
            return skip("Scaling very small images is uninteresting.");
        }
        decode_info = decode_info.with_dimensions(size);

        let bpp = decode_info.bytes_per_pixel();
        let row_bytes = size.width as usize * bpp;
        let safe_size = decode_info.compute_byte_size(row_bytes);
        let mut pixels = vec![0u8; safe_size];

        let mut options = Options::default();
        if CodecMode::CodecZeroInit == self.mode {
            options.zero_initialized = ZeroInitialized::Yes;
        }

        let bitmap_info = bitmap_info_for(&decode_info);
        let dst = self.dst_color_type;

        match self.mode {
            CodecMode::Animated => {
                let mut android_options = AndroidOptions::default();
                if self.is_scaled() {
                    assert!(android_codec.is_some());
                    android_options.sample_size = (1.0 / self.scale) as i32;
                    let dims = android_codec
                        .as_ref()
                        .map(|a| a.get_sampled_dimensions(android_options.sample_size))
                        .unwrap_or_default();
                    decode_info = decode_info.with_dimensions(dims);
                }

                let frame_infos = match android_codec.as_mut() {
                    Some(android) => android.codec_mut().frame_infos(),
                    None => codec.frame_infos(),
                };
                if frame_infos.len() <= 1 {
                    return fatal(format!("{path} is not an animated image."));
                }

                // As in CodecSrc::size(), compute a roughly square grid to draw the frames
                // into. "factor" is the number of frames to draw on one row. There will be
                // up to "factor" rows as well.
                let root = (frame_infos.len() as f32).sqrt();
                let factor = float_ceil2int(root);

                // Used to cache a frame that future frames will depend on.
                let mut prior_frame_pixels: Vec<u8> = Vec::new();
                let mut cached_frame = NO_FRAME;
                for (i, frame_info) in frame_infos.iter().enumerate() {
                    let i = i as i32;
                    android_options.base.frame_index = i;
                    // Check for a prior frame
                    let req_frame = frame_info.required_frame;
                    if req_frame != NO_FRAME
                        && req_frame == cached_frame
                        && !prior_frame_pixels.is_empty()
                    {
                        // Copy into pixels
                        pixels[..safe_size].copy_from_slice(&prior_frame_pixels[..safe_size]);
                        android_options.base.prior_frame = req_frame;
                    } else {
                        android_options.base.prior_frame = NO_FRAME;
                    }
                    let mut result = match android_codec.as_mut() {
                        Some(android) => android.get_android_pixels(
                            &decode_info,
                            &mut pixels,
                            row_bytes,
                            Some(&android_options),
                        ),
                        None => codec.get_pixels(
                            &decode_info,
                            &mut pixels,
                            row_bytes,
                            Some(&android_options.base),
                        ),
                    };
                    if CodecResult::InvalidInput == result && i > 0 {
                        // Some of our test images have truncated later frames. Treat that
                        // the same as incomplete.
                        result = CodecResult::IncompleteInput;
                    }
                    match result {
                        CodecResult::Success
                        | CodecResult::ErrorInInput
                        | CodecResult::IncompleteInput => {
                            // If the next frame depends on this one, store it in priorFrame.
                            // It is possible that we may discard a frame that future frames depend on,
                            // but the codec will simply redecode the discarded frame.
                            // Do this before calling draw_to_canvas, which premultiplies in place. If
                            // we're decoding to unpremul, we want to pass the unmodified frame to the
                            // codec for decoding the next frame.
                            if ((i + 1) as usize) < frame_infos.len()
                                && frame_infos[(i + 1) as usize].required_frame == i
                            {
                                prior_frame_pixels.clear();
                                prior_frame_pixels.extend_from_slice(&pixels[..safe_size]);
                                cached_frame = i;
                            }

                            // SkAutoCanvasRestore acr(canvas, true)
                            let save_count = canvas.save();
                            let x_translate = (i % factor) * decode_info.width();
                            let y_translate = (i / factor) * decode_info.height();
                            canvas.translate((x_translate as f32, y_translate as f32));
                            draw_to_canvas(
                                canvas,
                                &bitmap_info,
                                &mut pixels,
                                row_bytes,
                                dst,
                                0.0,
                                0.0,
                            );
                            canvas.restore_to_count(save_count);
                            if result != CodecResult::Success {
                                return ok();
                            }
                        }
                        CodecResult::InvalidConversion
                            if i > 0 && decode_info.color_type() == ColorType::RGB565 =>
                        {
                            return skip(format!("Cannot decode frame {i} to 565 ({path})."));
                        }
                        _ => {
                            return fatal(format!("Couldn't getPixels for frame {i} in {path}."));
                        }
                    }
                }
            }
            CodecMode::CodecZeroInit | CodecMode::Codec => {
                match codec.get_pixels(&decode_info, &mut pixels, row_bytes, Some(&options)) {
                    // We consider these to be valid, since we should still decode what is
                    // available.
                    CodecResult::Success
                    | CodecResult::ErrorInInput
                    | CodecResult::IncompleteInput => {}
                    // Everything else is considered a failure.
                    _ => return fatal(format!("Couldn't getPixels {path}.")),
                }

                draw_to_canvas(canvas, &bitmap_info, &mut pixels, row_bytes, dst, 0.0, 0.0);
            }
            CodecMode::Scanline => {
                let height = decode_info.height();
                let path_str = self.path.to_string_lossy();
                let use_incremental = ["png", "PNG", "gif", "GIF"]
                    .iter()
                    .any(|ext| path_str.ends_with(ext));
                // ico may use the old scanline method or the new one, depending on whether it
                // internally holds a bmp or a png.
                let ico = path_str.ends_with("ico");
                let mut use_old_scanline_method = !use_incremental && !ico;
                if use_incremental || ico {
                    if CodecResult::Success
                        == codec.start_incremental_decode(
                            &decode_info,
                            &mut pixels,
                            row_bytes,
                            Some(&options),
                        )
                    {
                        let (result, rows_decoded) = codec.incremental_decode_rows(&mut pixels);
                        if CodecResult::IncompleteInput == result
                            || CodecResult::ErrorInInput == result
                        {
                            codec.fill_incomplete_image(
                                &decode_info,
                                &mut pixels,
                                row_bytes,
                                ZeroInitialized::No,
                                height,
                                rows_decoded,
                            );
                        }
                    } else {
                        if use_incremental {
                            // Error: These should support incremental decode.
                            return fatal("Could not start incremental decode");
                        }
                        // Otherwise, this is an ICO. Since incremental failed, it must contain a BMP,
                        // which should work via startScanlineDecode
                        use_old_scanline_method = true;
                    }
                }

                if use_old_scanline_method {
                    if CodecResult::Success != codec.start_scanline_decode(&decode_info, None) {
                        return fatal("Could not start scanline decoder");
                    }

                    // We do not need to check the return value.  On an incomplete
                    // image, memory will be filled with a default value.
                    codec.get_scanlines(&mut pixels, height, row_bytes);
                }

                draw_to_canvas(canvas, &bitmap_info, &mut pixels, row_bytes, dst, 0.0, 0.0);
            }
            CodecMode::Stripe => {
                let height = decode_info.height();
                // This value is chosen arbitrarily.  We exercise more cases by choosing a value that
                // does not align with image blocks.
                let stripe_height = 37;
                let num_stripes = (height + stripe_height - 1) / stripe_height;

                // Decode odd stripes
                if CodecResult::Success != codec.start_scanline_decode(&decode_info, Some(&options))
                {
                    return fatal("Could not start scanline decoder");
                }

                // This mode was designed to test the new skip scanlines API in libjpeg-turbo.
                // Jpegs have kTopDown_SkScanlineOrder, and at this time, it is not interesting
                // to run this test for image types that do not have this scanline ordering.
                // We only run this on Jpeg, which is always kTopDown.
                assert_eq!(
                    skia_rust_codec::codec::ScanlineOrder::TopDown,
                    codec.scanline_order()
                );

                let mut i = 0;
                while i < num_stripes {
                    // Skip a stripe
                    let lines_to_skip = stripe_height.min(height - i * stripe_height);
                    codec.skip_scanlines(lines_to_skip);

                    // Read a stripe
                    let start_y = (i + 1) * stripe_height;
                    let lines_to_read = stripe_height.min(height - start_y);
                    if lines_to_read > 0 {
                        codec.get_scanlines(
                            &mut pixels[row_bytes * start_y as usize..],
                            lines_to_read,
                            row_bytes,
                        );
                    }
                    i += 2;
                }

                // Decode even stripes
                let start_result = codec.start_scanline_decode(&decode_info, None);
                if CodecResult::Success != start_result {
                    return fatal("Failed to restart scanline decoder with same parameters.");
                }
                let mut i = 0;
                while i < num_stripes {
                    // Read a stripe
                    let start_y = i * stripe_height;
                    let lines_to_read = stripe_height.min(height - start_y);
                    codec.get_scanlines(
                        &mut pixels[row_bytes * start_y as usize..],
                        lines_to_read,
                        row_bytes,
                    );

                    // Skip a stripe
                    let lines_to_skip = stripe_height.min(height - (i + 1) * stripe_height);
                    if lines_to_skip > 0 {
                        codec.skip_scanlines(lines_to_skip);
                    }
                    i += 2;
                }

                draw_to_canvas(canvas, &bitmap_info, &mut pixels, row_bytes, dst, 0.0, 0.0);
            }
            CodecMode::CroppedScanline => {
                let width = decode_info.width();
                let height = decode_info.height();
                // This value is chosen because, as we move across the image, it will sometimes
                // align with the jpeg block sizes and it will sometimes not.  This allows us
                // to test interestingly different code paths in the implementation.
                let tile_size = 36;
                let mut x = 0;
                while x < width {
                    let subset = IRect::from_xywh(x, 0, tile_size.min(width - x), height);
                    options.subset = Some(subset);
                    if CodecResult::Success
                        != codec.start_scanline_decode(&decode_info, Some(&options))
                    {
                        return fatal("Could not start scanline decoder.");
                    }

                    codec.get_scanlines(&mut pixels[x as usize * bpp..], height, row_bytes);
                    x += tile_size;
                }

                draw_to_canvas(canvas, &bitmap_info, &mut pixels, row_bytes, dst, 0.0, 0.0);
            }
            CodecMode::Subset => {
                // Arbitrarily choose a divisor.
                let divisor = 2;
                // Total width/height of the image.
                let w_total = codec.info().width();
                let h_total = codec.info().height();
                if divisor > w_total || divisor > h_total {
                    return skip(format!(
                        "Cannot codec subset: divisor {divisor} is too big for {path} with \
                         dimensions ({w_total} x {h_total})"
                    ));
                }
                // subset dimensions
                // SkWebpCodec, the only one that supports subsets, requires even top/left boundaries.
                let w = align2(w_total / divisor);
                let h = align2(h_total / divisor);
                // Keep track of left and top (for drawing subsetBm into canvas). We could use
                // fScale * x and fScale * y, but we want integers such that the next subset will start
                // where the last one ended. So we'll add decodeInfo.width() and height().
                let mut left = 0;
                let mut x = 0;
                while x < w_total {
                    let mut top = 0;
                    let mut y = 0;
                    while y < h_total {
                        // Do not make the subset go off the edge of the image.
                        let pre_scale_w = w.min(w_total - x);
                        let pre_scale_h = h.min(h_total - y);
                        options.subset = Some(IRect::from_xywh(x, y, pre_scale_w, pre_scale_h));
                        // And scale
                        // FIXME: Should we have a version of getScaledDimensions that takes a subset
                        // into account?
                        let scaled_w = 1.max(skia_rust_core::scalar::scalar_round_to_int(
                            pre_scale_w as f32 * self.scale,
                        ));
                        let scaled_h = 1.max(skia_rust_core::scalar::scalar_round_to_int(
                            pre_scale_h as f32 * self.scale,
                        ));
                        decode_info = decode_info.with_wh(scaled_w, scaled_h);
                        let subset_bitmap_info = bitmap_info.with_wh(scaled_w, scaled_h);
                        let subset_row_bytes = subset_bitmap_info.min_row_bytes();
                        let result = codec.get_pixels(
                            &decode_info,
                            &mut pixels,
                            subset_row_bytes,
                            Some(&options),
                        );
                        match result {
                            CodecResult::Success
                            | CodecResult::ErrorInInput
                            | CodecResult::IncompleteInput => {}
                            _ => {
                                return fatal(format!(
                                    "subset codec failed to decode ({x}, {y}, {}, {}) from {path} \
                                     with dimensions ({w_total} x {h_total})\t error {}",
                                    decode_info.width(),
                                    decode_info.height(),
                                    result.as_str()
                                ));
                            }
                        }
                        draw_to_canvas(
                            canvas,
                            &subset_bitmap_info,
                            &mut pixels,
                            subset_row_bytes,
                            dst,
                            left as f32,
                            top as f32,
                        );

                        // translate by the scaled height.
                        top += decode_info.height();
                        y += h;
                    }
                    // translate by the scaled width.
                    left += decode_info.width();
                    x += w;
                }
                return ok();
            }
        }
        ok()
    }

    // Port of: dm/DMSrcSink.cpp#L853-L876 (chrome/m156)
    fn size(&self) -> ISize {
        let Some(encoded) = read_data(&self.path) else {
            return ISize::new(0, 0);
        };
        let Some(mut codec) = make_codec(&encoded) else {
            return ISize::new(0, 0);
        };

        if self.mode != CodecMode::Animated {
            return codec.get_scaled_dimensions(self.scale);
        }

        // We'll draw one of each frame, so make it big enough to hold them all
        // in a grid. The grid will be roughly square, with "factor" frames per
        // row and up to "factor" rows.
        let count = codec.frame_infos().len();
        let root = (count as f32).sqrt();
        let factor = float_ceil2int(root);

        let Some(android_codec) = AndroidCodec::make_from_codec(codec) else {
            return ISize::new(0, 0);
        };
        let image_size = android_codec.get_sampled_dimensions((1.0 / self.scale) as i32);
        ISize::new(
            image_size.width * factor,
            image_size.height * float_ceil2int(count as f32 / factor as f32),
        )
    }

    // Port of: dm/DMSrcSink.cpp#L878-L887 (chrome/m156)
    fn name(&self) -> String {
        let mut name = basename(&self.path);
        if self.mode == CodecMode::Animated {
            name.push_str("_animated");
        }
        if !self.is_scaled() {
            return name;
        }
        get_scaled_name(&name, self.scale)
    }

    fn id_dir(&self) -> String {
        format!("image/{}", self.options)
    }

    fn fatal_pixels_are_results(&self) -> bool {
        true
    }
}

/// `AndroidCodecSrc`: decodes with `SkAndroidCodec` at a sample size.
// Port of: dm/DMSrcSink.h#L280-L304 (chrome/m156)
#[derive(Clone, Debug)]
pub struct AndroidCodecSrc {
    path: PathBuf,
    options: String,
    dst_color_type: DstColorType,
    dst_alpha_type: AlphaType,
    sample_size: i32,
}

impl AndroidCodecSrc {
    /// `push_android_codec_src`: the source and its result folder.
    // Port of: dm/DM.cpp#L597-L630 (chrome/m156)
    #[must_use]
    pub fn new(
        path: &Path,
        dst_color_type: DstColorType,
        dst_alpha_type: AlphaType,
        sample_size: i32,
    ) -> Self {
        let mut folder = String::from("scaled_codec");
        match dst_color_type {
            DstColorType::Grayscale => folder.push_str("_kGray8"),
            DstColorType::NonNative8888 => folder.push_str("_kNonNative"),
            DstColorType::GetFromCanvas => {}
        }
        match dst_alpha_type {
            AlphaType::Premul => folder.push_str("_premul"),
            AlphaType::Unpremul => folder.push_str("_unpremul"),
            _ => {}
        }
        if 1 != sample_size {
            folder.push_str(&scale_suffix(1.0 / sample_size as f32));
        }
        Self {
            path: path.to_path_buf(),
            options: folder,
            dst_color_type,
            dst_alpha_type,
            sample_size,
        }
    }
}

impl Src for AndroidCodecSrc {
    // Port of: dm/DMSrcSink.cpp#L906-L958 (chrome/m156)
    fn draw(&self, canvas: &Canvas) -> DmResult {
        let path = self.path.display();
        let Some(encoded) = read_data(&self.path) else {
            return fatal(format!("Couldn't read {path}."));
        };
        let Some(mut codec) = make_android_codec(&encoded) else {
            return fatal(format!("Couldn't create android codec for {path}."));
        };

        let Some(mut decode_info) = get_decode_info(
            &codec.info(),
            canvas.image_info().color_type(),
            self.dst_color_type,
            self.dst_alpha_type,
        ) else {
            return skip("Skipping uninteresting test.");
        };

        // Scale the image if it is desired.
        let size = codec.get_sampled_dimensions(self.sample_size);

        // Visually inspecting very small output images is not necessary.  We will
        // cover these cases in unit testing.
        if (size.width <= 10 || size.height <= 10) && 1 != self.sample_size {
            return skip("Scaling very small images is uninteresting.");
        }
        decode_info = decode_info.with_dimensions(size);

        let bpp = decode_info.bytes_per_pixel();
        let row_bytes = size.width as usize * bpp;
        let mut pixels = vec![0u8; size.height as usize * row_bytes];

        let bitmap_info = bitmap_info_for(&decode_info);

        // Create options for the codec.
        let options = AndroidOptions {
            sample_size: self.sample_size,
            ..AndroidOptions::default()
        };

        match codec.get_android_pixels(&decode_info, &mut pixels, row_bytes, Some(&options)) {
            CodecResult::Success | CodecResult::ErrorInInput | CodecResult::IncompleteInput => {}
            _ => return fatal(format!("Couldn't getPixels {path}.")),
        }
        draw_to_canvas(
            canvas,
            &bitmap_info,
            &mut pixels,
            row_bytes,
            self.dst_color_type,
            0.0,
            0.0,
        );
        ok()
    }

    // Port of: dm/DMSrcSink.cpp#L960-L967 (chrome/m156)
    fn size(&self) -> ISize {
        let Some(encoded) = read_data(&self.path) else {
            return ISize::new(0, 0);
        };
        let Some(codec) = make_android_codec(&encoded) else {
            return ISize::new(0, 0);
        };
        codec.get_sampled_dimensions(self.sample_size)
    }

    // Port of: dm/DMSrcSink.cpp#L969-L976 (chrome/m156)
    fn name(&self) -> String {
        // We will replicate the names used by CodecSrc so that images can
        // be compared in Gold.
        if 1 == self.sample_size {
            return basename(&self.path);
        }
        get_scaled_name(&basename(&self.path), 1.0 / self.sample_size as f32)
    }

    fn id_dir(&self) -> String {
        format!("image/{}", self.options)
    }

    fn fatal_pixels_are_results(&self) -> bool {
        true
    }
}

/// `ImageGenSrc::Mode`.
// Port of: dm/DMSrcSink.h#L306-L312 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageGenMode {
    /// `SkCodecImageGenerator`.
    Codec,
}

/// `ImageGenSrc`: decodes through an `SkImageGenerator` to the CPU.
// Port of: dm/DMSrcSink.h#L306-L333 (chrome/m156)
#[derive(Clone, Debug)]
pub struct ImageGenSrc {
    path: PathBuf,
    options: String,
    mode: ImageGenMode,
    dst_alpha_type: AlphaType,
}

impl ImageGenSrc {
    /// `push_image_gen_src` for a CPU test: the source and its result folder.
    // Port of: dm/DM.cpp#L632-L664 (chrome/m156)
    #[must_use]
    pub fn new(path: &Path, mode: ImageGenMode, alpha_type: AlphaType) -> Self {
        let mut folder = String::new();
        match mode {
            ImageGenMode::Codec => folder.push_str("gen_codec"),
        }
        match alpha_type {
            AlphaType::Opaque => folder.push_str("_opaque"),
            AlphaType::Premul => folder.push_str("_premul"),
            AlphaType::Unpremul => folder.push_str("_unpremul"),
            AlphaType::Unknown => {}
        }
        Self {
            path: path.to_path_buf(),
            options: folder,
            mode,
            dst_alpha_type: alpha_type,
        }
    }
}

impl Src for ImageGenSrc {
    // Port of: dm/DMSrcSink.cpp#L998-L1075 (chrome/m156)
    fn draw(&self, canvas: &Canvas) -> DmResult {
        if ColorType::RGB565 == canvas.image_info().color_type() {
            return skip("Uninteresting to test image generator to 565.");
        }

        let path = self.path.display();
        let Some(encoded) = read_data(&self.path) else {
            return fatal(format!("Couldn't read {path}."));
        };

        let ImageGenMode::Codec = self.mode;
        let Some(mut generator) = CodecImageGenerator::make_from_encoded_codec(encoded, None)
        else {
            return fatal("Could not create codec image generator.");
        };

        // Test various color and alpha types on CPU
        let mut decode_info = generator.info().with_alpha_type(self.dst_alpha_type);

        let bpp = decode_info.bytes_per_pixel();
        let row_bytes = decode_info.width() as usize * bpp;
        let mut pixels = vec![0u8; decode_info.height() as usize * row_bytes];
        if !generator.get_pixels(&decode_info, &mut pixels, row_bytes) {
            return fatal(format!(
                "Image generator could not getPixels() for {path}\n"
            ));
        }

        decode_info = set_bitmap_color_space(&decode_info);
        draw_to_canvas(
            canvas,
            &decode_info,
            &mut pixels,
            row_bytes,
            DstColorType::GetFromCanvas,
            0.0,
            0.0,
        );
        ok()
    }

    // Port of: dm/DMSrcSink.cpp#L1077-L1084 (chrome/m156)
    fn size(&self) -> ISize {
        let Some(encoded) = read_data(&self.path) else {
            return ISize::new(0, 0);
        };
        let Some(codec) = make_codec(&encoded) else {
            return ISize::new(0, 0);
        };
        codec.info().dimensions()
    }

    // Port of: dm/DMSrcSink.cpp#L1086-L1088 (chrome/m156)
    fn name(&self) -> String {
        basename(&self.path)
    }

    fn id_dir(&self) -> String {
        format!("image/{}", self.options)
    }

    fn fatal_pixels_are_results(&self) -> bool {
        true
    }
}

/// `BRDSrc::Mode`.
// Port of: dm/DMSrcSink.h#L206-L216 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrdMode {
    /// Decode the whole image as one region.
    FullImage,
    /// Decode four regions, with borders, and draw them together.
    Divisor,
}

/// `BRDSrc`: decodes with the Android `BitmapRegionDecoder`.
// Port of: dm/DMSrcSink.h#L196-L225 (chrome/m156)
#[derive(Clone, Debug)]
pub struct BrdSrc {
    path: PathBuf,
    options: String,
    mode: BrdMode,
    dst_color_type: DstColorType,
    sample_size: u32,
}

impl BrdSrc {
    /// `push_brd_src`: the source and its result folder.
    // Port of: dm/DM.cpp#L667-L698 (chrome/m156)
    #[must_use]
    pub fn new(path: &Path, dst_color_type: DstColorType, mode: BrdMode, sample_size: u32) -> Self {
        let mut folder = String::from("brd_android_codec");
        match mode {
            BrdMode::FullImage => {}
            BrdMode::Divisor => folder.push_str("_divisor"),
        }
        match dst_color_type {
            DstColorType::GetFromCanvas => {}
            DstColorType::Grayscale => folder.push_str("_kGray"),
            DstColorType::NonNative8888 => unreachable!("push_brd_src asserts on it"),
        }
        if 1 != sample_size {
            folder.push_str(&scale_suffix(1.0 / sample_size as f32));
        }
        Self {
            path: path.to_path_buf(),
            options: folder,
            mode,
            dst_color_type,
            sample_size,
        }
    }
}

/// `android::skia::BitmapRegionDecoder`.
// Port of: client_utils/android/BitmapRegionDecoder.h (chrome/m156)
struct BitmapRegionDecoder {
    codec: AndroidCodec<'static>,
}

/// `SubsetType`.
// Port of: client_utils/android/BitmapRegionDecoderPriv.h#L14-L18 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubsetType {
    FullyInside,
    PartiallyInside,
    Outside,
}

/// `adjust_subset_rect`: clips `subset` to the image, and says where in the output the decoded
/// pixels go (`out_x`, `out_y`).
// Port of: client_utils/android/BitmapRegionDecoderPriv.h#L20-L49 (chrome/m156)
fn adjust_subset_rect(
    image_dims: ISize,
    subset: &mut IRect,
    out_x: &mut i32,
    out_y: &mut i32,
) -> SubsetType {
    // These must be at least zero, we can't start decoding the image at a negative coordinate.
    let left = 0.max(subset.left());
    let top = 0.max(subset.top());

    // If input offsets are less than zero, we decode to an offset location in the output bitmap.
    *out_x = left - subset.left();
    *out_y = top - subset.top();

    // Make sure we don't decode pixels past the edge of the image or past the edge of the subset.
    let width = (image_dims.width - left).min(subset.width() - *out_x);
    let height = (image_dims.height - top).min(subset.height() - *out_y);
    if width <= 0 || height <= 0 {
        return SubsetType::Outside;
    }

    // skia-rust: the C++ reads the old width and height after setXYWH, which makes the
    // comparison below always false; kept as written.
    subset.set_xywh(left, top, width, height);
    if (*out_x != 0) || (*out_y != 0) || (width != subset.width()) || (height != subset.height()) {
        return SubsetType::PartiallyInside;
    }

    SubsetType::FullyInside
}

impl BitmapRegionDecoder {
    /// `BitmapRegionDecoder::Make`.
    // Port of: client_utils/android/BitmapRegionDecoder.cpp#L20-L40 (chrome/m156)
    fn make(data: &Data) -> Option<Self> {
        let codec = make_android_codec(data)?;
        match codec.encoded_format() {
            EncodedImageFormat::JPEG
            | EncodedImageFormat::PNG
            | EncodedImageFormat::WEBP
            | EncodedImageFormat::HEIF
            | EncodedImageFormat::AVIF => {}
            _ => return None,
        }
        Some(Self { codec })
    }

    fn width(&self) -> i32 {
        self.codec.info().width()
    }

    fn height(&self) -> i32 {
        self.codec.info().height()
    }

    /// `BitmapRegionDecoder::decodeRegion` with no allocator (the heap).
    // Port of: client_utils/android/BitmapRegionDecoder.cpp#L50-L151 (chrome/m156)
    fn decode_region(
        &mut self,
        desired_subset: IRect,
        sample_size: i32,
        dst_color_type: ColorType,
        require_unpremul: bool,
        dst_color_space: Option<ColorSpace>,
    ) -> Option<Bitmap> {
        // Fix the input sampleSize if necessary.
        let sample_size = sample_size.max(1);

        // The size of the output bitmap is determined by the size of the
        // requested subset, not by the size of the intersection of the subset
        // and the image dimensions.
        // If inputX is negative, we will need to place decoded pixels into the
        // output bitmap starting at a left offset.  Call this outX.
        // If outX is non-zero, subsetX must be zero.
        // If inputY is negative, we will need to place decoded pixels into the
        // output bitmap starting at a top offset.  Call this outY.
        // If outY is non-zero, subsetY must be zero.
        let (mut out_x, mut out_y) = (0, 0);
        let mut subset = desired_subset;
        let ty = adjust_subset_rect(
            self.codec.info().dimensions(),
            &mut subset,
            &mut out_x,
            &mut out_y,
        );
        if SubsetType::Outside == ty {
            return None;
        }

        // Ask the codec for a scaled subset
        if !self.codec.get_supported_subset(&mut subset) {
            return None;
        }
        let scaled_size = self
            .codec
            .get_sampled_subset_dimensions(sample_size, subset);

        // Create the image info for the decode
        let dst_alpha_type = self.codec.compute_output_alpha_type(require_unpremul);
        let decode_info =
            ImageInfo::new(scaled_size, dst_color_type, dst_alpha_type, dst_color_space);

        // Initialize the destination bitmap
        let mut scaled_out_x = 0;
        let mut scaled_out_y = 0;
        let mut scaled_out_width = scaled_size.width;
        let mut scaled_out_height = scaled_size.height;
        if SubsetType::PartiallyInside == ty {
            scaled_out_x = out_x / sample_size;
            scaled_out_y = out_y / sample_size;
            // We need to be safe here because getSupportedSubset() may have modified the subset.
            let extra_x = 0.max(desired_subset.width() - out_x - subset.width());
            let extra_y = 0.max(desired_subset.height() - out_y - subset.height());
            let scaled_extra_x = extra_x / sample_size;
            let scaled_extra_y = extra_y / sample_size;
            scaled_out_width += scaled_out_x + scaled_extra_x;
            scaled_out_height += scaled_out_y + scaled_extra_y;
        }
        let mut out_info = decode_info.with_wh(scaled_out_width, scaled_out_height);
        if ColorType::Gray8 == dst_color_type {
            // The legacy implementations of BitmapFactory and BitmapRegionDecoder
            // used kAlpha8 for grayscale images (before kGray8 existed).  While
            // the codec recognizes kGray8, we need to decode into a kAlpha8
            // bitmap in order to avoid a behavior change.
            out_info = out_info
                .with_color_type(ColorType::Alpha8)
                .with_alpha_type(AlphaType::Premul);
        }
        // bitmap->setInfo(outInfo); bitmap->tryAllocPixels(nullptr): zeroed heap pixels, so the
        // "zero the bitmap if the region is not completely within the image" memset is a no-op.
        let row_bytes = out_info.min_row_bytes();
        let mut pixels = vec![0u8; out_info.compute_byte_size(row_bytes)];

        // Decode into the destination bitmap
        let options = AndroidOptions {
            base: Options {
                subset: Some(subset),
                zero_initialized: ZeroInitialized::No,
                ..Options::default()
            },
            sample_size,
        };
        let offset = out_info.compute_offset((scaled_out_x, scaled_out_y), row_bytes);

        let result = self.codec.get_android_pixels(
            &decode_info,
            &mut pixels[offset..],
            row_bytes,
            Some(&options),
        );
        match result {
            CodecResult::Success | CodecResult::IncompleteInput | CodecResult::ErrorInInput => {}
            _ => return None,
        }
        let mut bitmap = Bitmap::new();
        bitmap
            .install_pixels(&out_info, pixels, row_bytes)
            .then_some(bitmap)
    }
}

/// `alpha8_to_gray8`: Android requires kGray8 bitmaps to be tagged as kAlpha8. Here we convert
/// them back to kGray8 so our test framework can draw them correctly.
// Port of: dm/DMSrcSink.cpp#L248-L256 (chrome/m156)
fn alpha8_to_gray8(bitmap: &mut Bitmap) {
    if ColorType::Alpha8 == bitmap.color_type() {
        let new_info = bitmap
            .info()
            .with_color_type(ColorType::Gray8)
            .with_alpha_type(AlphaType::Opaque);
        let row_bytes = bitmap.row_bytes();
        let pixels = bitmap.pixmap().bytes().unwrap_or(&[]).to_vec();
        let mut gray = Bitmap::new();
        assert!(gray.install_pixels(&new_info, pixels, row_bytes));
        *bitmap = gray;
    }
}

impl Src for BrdSrc {
    // Port of: dm/DMSrcSink.cpp#L258-L378 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in C++ too
    fn draw(&self, canvas: &Canvas) -> DmResult {
        let mut color_type = canvas.image_info().color_type();
        if ColorType::RGB565 == color_type && DstColorType::GetFromCanvas != self.dst_color_type {
            return skip("Testing non-565 to 565 is uninteresting.");
        }
        match self.dst_color_type {
            DstColorType::GetFromCanvas => {}
            DstColorType::Grayscale => color_type = ColorType::Gray8,
            DstColorType::NonNative8888 => unreachable!("BRDSrc is never made with it"),
        }

        let path = self.path.display();
        let Some(encoded) = read_data(&self.path) else {
            return skip(format!("Could not create brd for {path}."));
        };
        let Some(mut brd) = BitmapRegionDecoder::make(&encoded) else {
            return skip(format!("Could not create brd for {path}."));
        };

        let recommended_ct = brd.codec.compute_output_color_type(color_type);
        if ColorType::RGB565 == color_type && recommended_ct != color_type {
            return skip("Skip decoding non-opaque to 565.");
        }
        color_type = recommended_ct;

        let color_space = brd.codec.compute_output_color_space(color_type, None);

        let width = brd.width() as u32;
        let height = brd.height() as u32;
        let sample_size = self.sample_size;
        // Visually inspecting very small output images is not necessary.
        if (width / sample_size <= 10 || height / sample_size <= 10) && 1 != sample_size {
            return skip("Scaling very small images is uninteresting.");
        }
        match self.mode {
            BrdMode::FullImage => {
                let Some(mut bitmap) = brd.decode_region(
                    IRect::from_xywh(0, 0, width as i32, height as i32),
                    sample_size as i32,
                    color_type,
                    false,
                    color_space,
                ) else {
                    return fatal("Cannot decode (full) region.");
                };
                alpha8_to_gray8(&mut bitmap);

                if let Some(image) = bitmap.as_image() {
                    canvas.draw_image(image, (0.0, 0.0), None);
                }
                ok()
            }
            BrdMode::Divisor => {
                let divisor: u32 = 2;
                if width < divisor || height < divisor {
                    return skip("Divisor is larger than image dimension.");
                }

                // Use a border to test subsets that extend outside the image.
                // We will not allow the border to be larger than the image dimensions.  Allowing
                // these large borders causes off by one errors that indicate a problem with the
                // test suite, not a problem with the implementation.
                let max_border = width.min(height) / (sample_size * divisor);
                let scaled_border = 5u32.min(max_border);
                let unscaled_border = scaled_border * sample_size;

                // We may need to clear the canvas to avoid uninitialized memory.
                // Assume we are scaling a 780x780 image with sampleSize = 8.
                // The output image should be 97x97.
                // Each subset will be 390x390.
                // Each scaled subset be 48x48.
                // Four scaled subsets will only fill a 96x96 image.
                // The bottom row and last column will not be touched.
                // This is an unfortunate result of our rounding rules when scaling.
                // Maybe we need to consider testing scaled subsets without trying to
                // combine them to match the full scaled image?  Or maybe this is the
                // best we can do?
                canvas.clear(skia_rust_core::color::Color::from(0u32));

                for x in 0..divisor {
                    for y in 0..divisor {
                        // Calculate the subset dimensions
                        let mut subset_width = width / divisor;
                        let mut subset_height = height / divisor;
                        let left = (x * subset_width) as i32;
                        let top = (y * subset_height) as i32;

                        // Increase the size of the last subset in each row or column, when the
                        // divisor does not divide evenly into the image dimensions
                        subset_width += if x + 1 == divisor { width % divisor } else { 0 };
                        subset_height += if y + 1 == divisor {
                            height % divisor
                        } else {
                            0
                        };

                        // Increase the size of the subset in order to have a border on each side
                        let decode_left = left - unscaled_border as i32;
                        let decode_top = top - unscaled_border as i32;
                        let decode_width = subset_width + unscaled_border * 2;
                        let decode_height = subset_height + unscaled_border * 2;
                        let Some(mut bitmap) = brd.decode_region(
                            IRect::from_xywh(
                                decode_left,
                                decode_top,
                                decode_width as i32,
                                decode_height as i32,
                            ),
                            sample_size as i32,
                            color_type,
                            false,
                            color_space.clone(),
                        ) else {
                            return fatal("Cannot decode region.");
                        };

                        alpha8_to_gray8(&mut bitmap);
                        let scaled_w = (subset_width / sample_size) as f32;
                        let scaled_h = (subset_height / sample_size) as f32;
                        if let Some(image) = bitmap.as_image() {
                            canvas.draw_image_rect(
                                image,
                                Some((
                                    &Rect::from_xywh(
                                        scaled_border as f32,
                                        scaled_border as f32,
                                        scaled_w,
                                        scaled_h,
                                    ),
                                    crate::canvas::SrcRectConstraint::Strict,
                                )),
                                Rect::from_xywh(
                                    (left as u32 / sample_size) as f32,
                                    (top as u32 / sample_size) as f32,
                                    scaled_w,
                                    scaled_h,
                                ),
                                &Paint::default(),
                            );
                        }
                    }
                }
                ok()
            }
        }
    }

    // Port of: dm/DMSrcSink.cpp#L380-L387 (chrome/m156)
    fn size(&self) -> ISize {
        let Some(encoded) = read_data(&self.path) else {
            return ISize::new(0, 0);
        };
        match BitmapRegionDecoder::make(&encoded) {
            Some(brd) => ISize::new(
                1.max(brd.width() / self.sample_size as i32),
                1.max(brd.height() / self.sample_size as i32),
            ),
            None => ISize::new(0, 0),
        }
    }

    // Port of: dm/DMSrcSink.cpp#L389-L397 (chrome/m156)
    fn name(&self) -> String {
        // We will replicate the names used by CodecSrc so that images can
        // be compared in Gold.
        if 1 == self.sample_size {
            return basename(&self.path);
        }
        get_scaled_name(&basename(&self.path), 1.0 / self.sample_size as f32)
    }

    fn id_dir(&self) -> String {
        format!("image/{}", self.options)
    }

    fn fatal_pixels_are_results(&self) -> bool {
        true
    }
}

// Test on a variety of sampleSizes, making sure to include:
// - 2, 4, and 8, which are natively supported by jpeg
// - multiples of 2 which are not divisible by 4 (analogous for 4)
// - larger powers of two, since BRD clients generally use powers of 2
// We will only produce output for the larger sizes on large images.
const BRD_SAMPLE_SIZES: [u32; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 24, 32, 64];
const BRD_MODES: [BrdMode; 2] = [BrdMode::FullImage, BrdMode::Divisor];

/// The extensions `push_codec_srcs` makes BRD sources for (`SK_ENABLE_ANDROID_UTILS`).
const BRD_EXTS: [&str; 8] = ["jpg", "jpeg", "png", "webp", "JPG", "JPEG", "PNG", "WEBP"];

/// `push_brd_srcs`.
// Port of: dm/DM.cpp#L700-L725 (chrome/m156)
fn push_brd_srcs(srcs: &mut Vec<Box<dyn Src>>, path: &Path, gray: bool) {
    if gray {
        // Only run grayscale to one sampleSize and Mode. Though interesting
        // to test grayscale, it should not reveal anything across various
        // sampleSizes and Modes
        // Arbitrarily choose Mode and sampleSize.
        srcs.push(Box::new(BrdSrc::new(
            path,
            DstColorType::Grayscale,
            BrdMode::FullImage,
            2,
        )));
    }

    for sample_size in BRD_SAMPLE_SIZES {
        for mode in BRD_MODES {
            srcs.push(Box::new(BrdSrc::new(
                path,
                DstColorType::GetFromCanvas,
                mode,
                sample_size,
            )));
        }
    }
}

/// `push_codec_srcs`: every source DM makes for the image file `path` with `--src image`
/// (`--simpleCodec` off), or none if the file cannot be read or decoded.
///
/// The sources DM makes but no raster sink draws (the GPU image generator, the platform image
/// generators) are left out.
// Port of: dm/DM.cpp#L727-L898 (chrome/m156)
#[must_use]
#[allow(clippy::too_many_lines)] // one function in C++ too
pub fn gather(path: &Path) -> Vec<Box<dyn Src>> {
    let mut srcs: Vec<Box<dyn Src>> = Vec::new();
    let Some(encoded) = read_data(path) else {
        return srcs;
    };
    let Some(mut codec) = make_codec(&encoded) else {
        return srcs;
    };

    // native scaling is only supported by WEBP and JPEG
    let mut supports_native_scaling = false;

    let mut native_modes = vec![CodecMode::Codec, CodecMode::CodecZeroInit];
    match codec.encoded_format() {
        EncodedImageFormat::JPEG => {
            native_modes.push(CodecMode::Scanline);
            native_modes.push(CodecMode::Stripe);
            native_modes.push(CodecMode::CroppedScanline);
            supports_native_scaling = true;
        }
        EncodedImageFormat::WEBP => {
            native_modes.push(CodecMode::Subset);
            supports_native_scaling = true;
        }
        EncodedImageFormat::DNG => {}
        _ => native_modes.push(CodecMode::Scanline),
    }

    let mut color_types = vec![DstColorType::GetFromCanvas, DstColorType::NonNative8888];
    if codec.info().color_type() == ColorType::Gray8 {
        color_types.push(DstColorType::Grayscale);
    }

    let mut alpha_modes = vec![AlphaType::Premul];
    if codec.info().alpha_type() != AlphaType::Opaque {
        alpha_modes.push(AlphaType::Unpremul);
    }

    for &mode in &native_modes {
        for &color_type in &color_types {
            for &alpha_type in &alpha_modes {
                // Only test kCroppedScanline_Mode when the alpha type is premul.  The test is
                // slow and won't be interestingly different with different alpha types.
                if CodecMode::CroppedScanline == mode && AlphaType::Premul != alpha_type {
                    continue;
                }

                srcs.push(Box::new(CodecSrc::new(
                    path, mode, color_type, alpha_type, 1.0,
                )));

                // Skip kNonNative on different native scales.  It won't be interestingly
                // different.
                if supports_native_scaling && DstColorType::NonNative8888 == color_type {
                    // Native Scales
                    // SkJpegCodec natively supports scaling to the following:
                    for scale in [0.125f32, 0.25, 0.375, 0.5, 0.625, 0.750, 0.875] {
                        srcs.push(Box::new(CodecSrc::new(
                            path, mode, color_type, alpha_type, scale,
                        )));
                    }
                }
            }
        }
    }

    if codec.frame_infos().len() > 1 {
        for dst_ct in [DstColorType::NonNative8888, DstColorType::GetFromCanvas] {
            for at in [AlphaType::Unpremul, AlphaType::Premul] {
                srcs.push(Box::new(CodecSrc::new(
                    path,
                    CodecMode::Animated,
                    dst_ct,
                    at,
                    1.0,
                )));
            }
        }
        for scale in [0.5f32, 0.33] {
            srcs.push(Box::new(CodecSrc::new(
                path,
                CodecMode::Animated,
                DstColorType::GetFromCanvas,
                AlphaType::Premul,
                scale,
            )));
        }
    }

    for sample_size in 1..=8 {
        for &color_type in &color_types {
            for &alpha_type in &alpha_modes {
                // We can exercise all of the kNonNative support code in the swizzler with just a
                // few sample sizes.  Skip the rest.
                if DstColorType::NonNative8888 == color_type && sample_size > 3 {
                    continue;
                }

                srcs.push(Box::new(AndroidCodecSrc::new(
                    path,
                    color_type,
                    alpha_type,
                    sample_size,
                )));
            }
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        const RAW_EXTS: [&str; 20] = [
            "arw", "cr2", "dng", "nef", "nrw", "orf", "raf", "rw2", "pef", "srw", "ARW", "CR2",
            "DNG", "NEF", "NRW", "ORF", "RAF", "RW2", "PEF", "SRW",
        ];
        if RAW_EXTS.contains(&ext) {
            // RAW is not supported by image generator (skbug.com/40036243) or BRD.
            return srcs;
        }

        if BRD_EXTS.contains(&ext) {
            let gray = codec.info().color_type() == ColorType::Gray8;
            push_brd_srcs(&mut srcs, path, gray);
        }
    }

    // Push image generator GPU test: not a raster source.

    // Push image generator CPU tests.
    for &alpha_type in &alpha_modes {
        srcs.push(Box::new(ImageGenSrc::new(
            path,
            ImageGenMode::Codec,
            alpha_type,
        )));
        // The platform generators exist for Mac, Windows and the NDK; the oracle is Linux.
    }
    srcs
}

/// The files `--images resources/images` makes DM try (`CommonFlags::CollectImages`): the
/// files of `dir` with an image extension, by name. A subdirectory is not searched.
// Port of: tools/flags/CommonFlagsImages.cpp#L11-L83 (chrome/m156)
#[must_use]
pub fn collect_images(dir: &Path) -> Vec<PathBuf> {
    const EXTS: [&str; 20] = [
        "bmp", "gif", "jpg", "jpeg", "png", "webp", "ktx", "astc", "wbmp", "ico", "BMP", "GIF",
        "JPG", "JPEG", "PNG", "WEBP", "KTX", "ASTC", "WBMP", "ICO",
    ];
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| EXTS.contains(&e))
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_utils::resource_dir;

    #[test]
    fn scaled_names_have_three_decimals() {
        assert_eq!(get_scaled_name("a.png", 0.5), "a.png_0.500");
        assert_eq!(get_scaled_name("a.png", 1.0 / 3.0), "a.png_0.333");
        assert_eq!(get_scaled_name("a.png", 0.33), "a.png_0.330");
        assert_eq!(get_scaled_name("a.png", 1.0 / 64.0), "a.png_0.016");
        assert_eq!(scale_suffix(0.125), "_0.125");
    }

    #[test]
    fn folders_follow_push_src() {
        let path = Path::new("images/a.jpg");
        let codec = |mode, ct, at, scale| CodecSrc::new(path, mode, ct, at, scale).id_dir();
        assert_eq!(
            codec(
                CodecMode::Codec,
                DstColorType::GetFromCanvas,
                AlphaType::Premul,
                1.0
            ),
            "image/codec_premul"
        );
        assert_eq!(
            codec(
                CodecMode::CroppedScanline,
                DstColorType::NonNative8888,
                AlphaType::Premul,
                0.375
            ),
            "image/crop_kNonNative_premul_0.375"
        );
        assert_eq!(
            codec(
                CodecMode::Animated,
                DstColorType::GetFromCanvas,
                AlphaType::Premul,
                0.33
            ),
            "image/codec_animated_premul_0.330"
        );
        assert_eq!(
            codec(
                CodecMode::Subset,
                DstColorType::Grayscale,
                AlphaType::Unpremul,
                1.0
            ),
            "image/codec_subset_kGray8_unpremul"
        );
        assert_eq!(
            AndroidCodecSrc::new(path, DstColorType::GetFromCanvas, AlphaType::Unpremul, 3)
                .id_dir(),
            "image/scaled_codec_unpremul_0.333"
        );
        assert_eq!(
            ImageGenSrc::new(path, ImageGenMode::Codec, AlphaType::Premul).id_dir(),
            "image/gen_codec_premul"
        );
        let brd = BrdSrc::new(path, DstColorType::Grayscale, BrdMode::FullImage, 2);
        assert_eq!(brd.id_dir(), "image/brd_android_codec_kGray_0.500");
        assert_eq!(brd.name(), "a.jpg_0.500");
        let brd = BrdSrc::new(path, DstColorType::GetFromCanvas, BrdMode::Divisor, 1);
        assert_eq!(brd.id_dir(), "image/brd_android_codec_divisor");
        assert_eq!(brd.name(), "a.jpg");
        assert_eq!(
            CodecSrc::new(
                path,
                CodecMode::Animated,
                DstColorType::GetFromCanvas,
                AlphaType::Premul,
                0.5
            )
            .name(),
            "a.jpg_animated_0.500"
        );
    }

    #[test]
    fn adjust_subset_rect_clips_to_the_image() {
        let dims = ISize::new(100, 80);
        let (mut out_x, mut out_y) = (0, 0);

        let mut subset = IRect::from_xywh(10, 20, 30, 40);
        assert_eq!(
            adjust_subset_rect(dims, &mut subset, &mut out_x, &mut out_y),
            SubsetType::FullyInside
        );
        assert_eq!(subset, IRect::from_xywh(10, 20, 30, 40));

        // A border off the top left: decoded pixels go to an offset in the output.
        let mut subset = IRect::from_xywh(-5, -3, 30, 40);
        assert_eq!(
            adjust_subset_rect(dims, &mut subset, &mut out_x, &mut out_y),
            SubsetType::PartiallyInside
        );
        assert_eq!((out_x, out_y), (5, 3));
        assert_eq!(subset, IRect::from_xywh(0, 0, 25, 37));

        // Past the bottom right edge: clipped, and (as in the C++, which compares the clipped
        // width with itself) still "fully inside" because nothing is offset in the output.
        let mut subset = IRect::from_xywh(90, 70, 30, 30);
        assert_eq!(
            adjust_subset_rect(dims, &mut subset, &mut out_x, &mut out_y),
            SubsetType::FullyInside
        );
        assert_eq!(subset, IRect::from_xywh(90, 70, 10, 10));

        let mut subset = IRect::from_xywh(100, 0, 10, 10);
        assert_eq!(
            adjust_subset_rect(dims, &mut subset, &mut out_x, &mut out_y),
            SubsetType::Outside
        );
    }

    #[test]
    fn non_native_swap_exchanges_red_and_blue_in_place() {
        let info = ImageInfo::new(
            ISize::new(2, 2),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        // Rows are 12 bytes apart: 8 bytes of pixels and 4 of padding that must stay.
        let mut px = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 9, 9, //
            11, 12, 13, 14, 15, 16, 17, 18, 9, 9, 9, 9,
        ];
        swap_rb_if_necessary(&mut px, &info, 12, DstColorType::GetFromCanvas);
        assert_eq!(px[0], 1);
        swap_rb_if_necessary(&mut px, &info, 12, DstColorType::NonNative8888);
        assert_eq!(
            px,
            [
                3, 2, 1, 4, 7, 6, 5, 8, 9, 9, 9, 9, //
                13, 12, 11, 14, 17, 16, 15, 18, 9, 9, 9, 9,
            ]
        );
    }

    /// `1x1.png`: a PNG gets every `CodecSrc` mode that applies to it, `AndroidCodecSrc`s at
    /// eight sample sizes, the `BRDSrc`s and the generators; the golden ids of the oracle for
    /// it (`codecs.md` §9.4) are all among them.
    #[test]
    fn png_sources_are_dm_s() {
        let Some(dir) = resource_dir() else {
            eprintln!("skipped: Skia's resources are not available");
            return;
        };
        let path = dir.join("images").join("1x1.png");
        let srcs = gather(&path);
        let dirs: Vec<String> = srcs.iter().map(|s| s.id_dir()).collect();
        for expected in [
            "image/codec_premul",
            "image/codec_kNonNative_premul",
            "image/codec_zero_init_premul",
            "image/scanline_premul",
            "image/scaled_codec_premul",
            "image/scaled_codec_kNonNative_premul_0.333",
            "image/scaled_codec_premul_0.125",
            "image/brd_android_codec",
            "image/brd_android_codec_divisor_0.016",
            "image/gen_codec_premul",
        ] {
            assert!(dirs.iter().any(|d| d == expected), "{expected} in {dirs:?}");
        }
        // An opaque PNG has no unpremul variants, and a PNG is not a JPEG or WebP.
        assert!(
            !dirs
                .iter()
                .any(|d| d.contains("unpremul") || d.starts_with("image/stripe"))
        );
        assert!(srcs.iter().all(|s| s.name().starts_with("1x1.png")));
        // A file that is not an image has no sources.
        assert!(gather(&dir.join("images").join("does-not-exist.png")).is_empty());
    }

    #[test]
    fn collect_images_takes_the_extensions_of_dm() {
        let dir = std::env::temp_dir().join(format!("skia-rust-dm-images-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub.png")).unwrap();
        for name in ["b.png", "a.JPG", "c.avif", "d.dng", "e.txt", "f.webp"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let names: Vec<String> = collect_images(&dir).iter().map(|p| basename(p)).collect();
        assert_eq!(names, ["a.JPG", "b.png", "f.webp"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
