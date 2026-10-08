// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/codec/SkCodecImageGenerator.{h,cpp}, src/codec/SkPixmapUtils.cpp,
// src/codec/SkPixmapUtilsPriv.h

//! `SkCodecImageGenerator`: an image generator that decodes an encoded image with a [`Codec`].
//!
//! The generator applies the image's [`EncodedOrigin`] (`SkPixmapUtils::Orient`) and prefers
//! premultiplied alpha, as Skia does. Scaled decodes through `SkAndroidCodec` are not part of
//! this generator (Skia's `SkCodecImageGenerator` does not use them either). YUVA planes are not
//! ported.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use skia_rust_core::image_generator::{ImageGenerator, generator_unique_id};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::stream::MemoryStream;
use skia_rust_raster::surfaces::wrap_pixels;

use crate::codec::{Codec, Result as CodecResult};
use crate::codecs;

/// The info of the generator's pixels: the codec's info, with the requested alpha type (or
/// premultiplied, for unpremultiplied images), and width and height swapped for an origin that
/// rotates by 90 degrees (`adjust_info`).
// Port of: src/codec/SkCodecImageGenerator.cpp#L37-L51 (chrome/m156)
fn adjust_info(codec: &Codec<'_>, at: Option<AlphaType>) -> ImageInfo {
    let mut info = codec.info();
    if let Some(at) = at {
        // If a specific alpha type was requested, use that.
        info = info.with_alpha_type(at);
    } else if AlphaType::Unpremul == info.alpha_type() {
        // Otherwise, prefer premul over unpremul (this produces better filtering in general).
        info = info.with_alpha_type(AlphaType::Premul);
    }
    if codec.origin().swaps_width_height() {
        info = swap_width_height(&info);
    }
    info
}

/// `SkPixmapUtils::SwapWidthHeight`.
// Port of: src/codec/SkPixmapUtils.cpp#L69-L71 (chrome/m156)
fn swap_width_height(info: &ImageInfo) -> ImageInfo {
    info.with_wh(info.height(), info.width())
}

/// Whether a codec result means the pixels were (at least partly) written (`getPixels` in
/// `SkCodecImageGenerator`).
fn decoded(result: CodecResult) -> bool {
    matches!(
        result,
        CodecResult::Success | CodecResult::IncompleteInput | CodecResult::ErrorInInput
    )
}

/// `SkPixmapUtils::Orient(dst, src, origin)`: draws `src`, which holds the image in its encoded
/// orientation, into `dst` upright. Returns false if the sizes or color types do not match.
// Port of: src/codec/SkPixmapUtils.cpp#L14-L58 (chrome/m156)
fn orient(dst: &mut Pixmap<'_>, src: &Pixmap<'_>, origin: EncodedOrigin) -> bool {
    if src.info().color_type() != dst.info().color_type() {
        return false;
    }
    // note: we just ignore alphaType and colorSpace for this transformation

    let mut w = src.info().width();
    let mut h = src.info().height();
    if origin.swaps_width_height() {
        std::mem::swap(&mut w, &mut h);
    }
    if dst.info().width() != w || dst.info().height() != h {
        return false;
    }
    if w == 0 || h == 0 {
        return true;
    }

    // The C++ checks for `src` aliasing `dst` here. Every caller in this crate decodes into a
    // separate allocation, so the aliasing case cannot arise.
    draw_orientation(dst, src, origin)
}

/// `draw_orientation` in `SkPixmapUtils.cpp`: draws `src` through the origin's matrix onto a
/// surface that wraps `dst`, with `kSrc` blending.
// Port of: src/codec/SkPixmapUtils.cpp#L20-L35 (chrome/m156)
fn draw_orientation(dst: &mut Pixmap<'_>, src: &Pixmap<'_>, origin: EncodedOrigin) -> bool {
    let dst_info = dst.info().clone();
    let dst_row_bytes = dst.row_bytes();
    let (dst_w, dst_h) = (dst_info.width(), dst_info.height());
    let Some(dst_pixels) = dst.writable_addr() else {
        return false;
    };
    let Some(mut surface) = wrap_pixels(&dst_info, dst_pixels, dst_row_bytes, None) else {
        return false;
    };

    // SkBitmap::installPixels(src): the bitmap borrows the pixels, which is a copy here.
    let mut bitmap = Bitmap::new();
    let Some(src_pixels) = src.addr() else {
        return false;
    };
    if !bitmap.install_pixels(src.info(), src_pixels.to_vec(), src.row_bytes()) {
        return false;
    }
    let Some(image) = images::raster_from_bitmap(&bitmap) else {
        return false;
    };

    let matrix = origin.to_matrix(dst_w, dst_h);
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    let canvas = surface.canvas();
    canvas.concat(&matrix);
    canvas.draw_image_with_sampling_options(
        &image,
        (0, 0),
        SamplingOptions::default(),
        Some(&paint),
    );
    true
}

/// `SkPixmapUtils::Orient(dst, origin, decode)`, the template of `SkPixmapUtilsPriv.h`: decodes
/// into `dst` directly for the default origin, else into a temporary of the unrotated size, and
/// then orients the result into `dst`.
// Port of: src/codec/SkPixmapUtilsPriv.h#L17-L44 (chrome/m156)
pub(crate) fn orient_decode(
    dst: &mut Pixmap<'_>,
    origin: EncodedOrigin,
    decode: impl FnOnce(&mut Pixmap<'_>) -> bool,
) -> bool {
    if origin == EncodedOrigin::TopLeft {
        return decode(dst);
    }

    let mut info = dst.info().clone();
    if origin.swaps_width_height() {
        info = swap_width_height(&info);
    }
    if info.color_type() == skia_rust_core::color_type::ColorType::Unknown {
        return false;
    }
    let row_bytes = info.min_row_bytes();
    let mut storage = vec![0u8; info.compute_byte_size(row_bytes)];
    let Some(mut tmp) = Pixmap::new(&info, &mut storage, row_bytes) else {
        return false;
    };
    if !decode(&mut tmp) {
        return false;
    }
    orient(dst, &tmp, origin)
}

/// A generator that decodes its pixels with a codec (`SkCodecImageGenerator`).
// Port of: src/codec/SkCodecImageGenerator.h#L14-L40 (chrome/m156)
#[doc(alias = "SkCodecImageGenerator")]
pub struct CodecImageGenerator {
    info: ImageInfo,
    unique_id: u32,
    codec: Codec<'static>,
    cached_data: Option<Data>,
}

impl CodecImageGenerator {
    /// A generator over `codec`, with the alpha type `at` if given (the constructor).
    // Port of: src/codec/SkCodecImageGenerator.cpp#L60-L63 (chrome/m156)
    #[must_use]
    pub fn new(codec: Codec<'static>, at: Option<AlphaType>) -> Self {
        let info = adjust_info(&codec, at);
        CodecImageGenerator {
            info,
            unique_id: generator_unique_id(NEED_NEW_IMAGE_UNIQUE_ID),
            codec,
            cached_data: None,
        }
    }

    /// A boxed generator over `codec`, or `None` if there is no codec (`MakeFromCodec`).
    // Port of: src/codec/SkCodecImageGenerator.cpp#L23-L28 (chrome/m156)
    #[doc(alias = "MakeFromCodec")]
    #[must_use]
    pub fn make_from_codec(
        codec: Option<Codec<'static>>,
        at: Option<AlphaType>,
    ) -> Option<Box<dyn ImageGenerator>> {
        codec.map(|codec| Box::new(CodecImageGenerator::new(codec, at)) as Box<dyn ImageGenerator>)
    }

    /// A boxed generator over the codec of `data`, or `None` if no decoder recognises it
    /// (`MakeFromEncodedCodec`).
    // Port of: src/codec/SkCodecImageGenerator.cpp#L14-L21 (chrome/m156)
    #[doc(alias = "MakeFromEncodedCodec")]
    #[must_use]
    pub fn make_from_encoded_codec(
        data: Data,
        at: Option<AlphaType>,
    ) -> Option<Box<dyn ImageGenerator>> {
        // SkCodec::MakeFromData: the codec reads the data through a memory stream.
        let stream = MemoryStream::from_data(Some(data));
        let codec = Codec::make_from_stream(Box::new(stream), codecs::decoders()).ok();
        Self::make_from_codec(codec, at)
    }

    /// `SkCodecImageGenerator::getPixels`: decodes into `pixels`, applying the origin.
    // Port of: src/codec/SkCodecImageGenerator.cpp#L65-L78 (chrome/m156)
    fn decode_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        let Some(mut dst) = Pixmap::new(info, pixels, row_bytes) else {
            return false;
        };
        let origin = self.codec.origin();
        let codec = &mut self.codec;
        orient_decode(&mut dst, origin, |pm| {
            let pm_info = pm.info().clone();
            let pm_row_bytes = pm.row_bytes();
            let Some(pm_pixels) = pm.writable_addr() else {
                return false;
            };
            decoded(codec.get_pixels(&pm_info, pm_pixels, pm_row_bytes, None))
        })
    }
}

impl ImageGenerator for CodecImageGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: src/codec/SkCodecImageGenerator.cpp#L30-L36 (chrome/m156)
    fn ref_encoded_data(&mut self) -> Option<Data> {
        if self.cached_data.is_none() {
            self.cached_data = self.codec.encoded_data();
        }
        self.cached_data.clone()
    }

    // Port of: src/codec/SkCodecImageGenerator.cpp#L80-L84 (chrome/m156)
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        self.decode_pixels(info, pixels, row_bytes)
    }
}
