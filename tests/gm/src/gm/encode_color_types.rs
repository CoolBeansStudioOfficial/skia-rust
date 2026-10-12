// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/encode_color_types.cpp (chrome/m156)

use skia_rust_codec::encode::webp_encoder::{self, Compression, Options};
use skia_rust_codec::images::deferred_from_encoded_data;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_raster::surfaces;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;

// Port of: gm/encode_color_types.cpp#L21-L22 (chrome/m156)
const IMAGE_WIDTH: i32 = 128;
const IMAGE_HEIGHT: i32 = 128;
/// `IMAGE_WIDTH` as the `float` the GM translates by.
const IMAGE_WIDTH_F: f32 = 128.0;

/// Port of `make_image(SkColorType, SkAlphaType)` (encode_color_types.cpp#L24-L56): a resource
/// drawn into a surface of the given colour and alpha type, or `None` where the pair has no
/// image (grayscale and the opaque-only types with a non-opaque alpha type).
// Port of: gm/encode_color_types.cpp#L24-L56 (chrome/m156)
fn make_image(color_type: ColorType, alpha_type: AlphaType) -> Option<Image> {
    let resource = match color_type {
        ColorType::Gray8 => {
            if alpha_type != AlphaType::Opaque {
                return None;
            }
            "images/grayscale.jpg"
        }
        ColorType::RGB565
        | ColorType::RGB888x
        | ColorType::RGB101010x
        | ColorType::BGR101010x
        | ColorType::RGBF16F16F16x => {
            if alpha_type != AlphaType::Opaque {
                return None;
            }
            "images/color_wheel.jpg"
        }
        _ => {
            if alpha_type == AlphaType::Opaque {
                "images/color_wheel.jpg"
            } else {
                "images/rainbow-gradient.png"
            }
        }
    };

    let image = get_resource_as_image(resource)?;
    let info = ImageInfo::new(
        ISize::new(image.width(), image.height()),
        color_type,
        alpha_type,
        image.color_space(),
    );
    let mut surface = surfaces::raster(&info, None, None)?;
    surface.canvas().draw_image(&image, (0.0, 0.0), None);
    surface.image_snapshot()
}

/// Port of `enum class Variant` (encode_color_types.cpp#L58-L68). Each variant draws image pairs
/// (the source, then its WebP round trip) for its own set of colour types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    /// One pair, using an opaque image.
    Opaque,
    /// One pair, using a grayscale image.
    Gray,
    /// An opaque pair followed by two more for premul and unpremul.
    Normal,
}

/// Port of `class EncodeColorTypesGM` (encode_color_types.cpp#L70-L178). The format is always
/// WebP: the C++ `SkASSERT_RELEASE(fFormat == SkEncodedImageFormat::kWEBP)` is the only use of it,
/// so it is not a field.
struct EncodeColorTypesGm {
    quality: i32,
    variant: Variant,
    name: &'static str,
}

impl EncodeColorTypesGm {
    // Port of: EncodeColorTypesGM(SkEncodedImageFormat, int, Variant, const char*)
    // (encode_color_types.cpp#L71-L76)
    const fn new(quality: i32, variant: Variant, name: &'static str) -> Self {
        Self {
            quality,
            variant,
            name,
        }
    }
}

impl GM for EncodeColorTypesGm {
    // Port of: EncodeColorTypesGM::getName (encode_color_types.cpp#L79-L86)
    fn name(&self) -> String {
        let variant = match self.variant {
            Variant::Opaque => "opaque-",
            Variant::Gray => "gray-",
            Variant::Normal => "",
        };
        format!("encode-{variant}color-types-{}", self.name)
    }

    // Port of: EncodeColorTypesGM::getISize (encode_color_types.cpp#L88-L92)
    fn size(&mut self) -> ISize {
        let width = if self.variant == Variant::Normal {
            IMAGE_WIDTH * 7
        } else {
            IMAGE_WIDTH * 2
        };
        ISize::new(width, IMAGE_HEIGHT)
    }

    // Port of: EncodeColorTypesGM::onDraw (encode_color_types.cpp#L94-L162)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let color_type = canvas.image_info().color_type();
        match self.variant {
            Variant::Gray => {
                if color_type != ColorType::Gray8 {
                    return DrawResult::Skip;
                }
            }
            Variant::Opaque => {
                if !matches!(
                    color_type,
                    ColorType::RGB565
                        | ColorType::RGB888x
                        | ColorType::RGB101010x
                        | ColorType::RGBF16F16F16x
                        | ColorType::BGR101010x
                ) {
                    return DrawResult::Skip;
                }
            }
            Variant::Normal => {
                if !matches!(
                    color_type,
                    ColorType::ARGB4444
                        | ColorType::RGBA8888
                        | ColorType::BGRA8888
                        | ColorType::RGBA1010102
                        | ColorType::BGRA1010102
                        | ColorType::RGBAF16Norm
                        | ColorType::RGBAF16
                        | ColorType::RGBAF32
                ) {
                    return DrawResult::Skip;
                }
            }
        }

        // Port of: const SkAlphaType alphaTypes[] (encode_color_types.cpp#L120-L122)
        let alpha_types = [AlphaType::Opaque, AlphaType::Premul, AlphaType::Unpremul];
        for alpha_type in alpha_types {
            let Some(src) = make_image(color_type, alpha_type) else {
                break;
            };
            let options = if self.quality < 100 {
                Options {
                    compression: Compression::Lossy,
                    // The quality is an integer in 0..100, exact in f32.
                    #[allow(clippy::cast_precision_loss)]
                    quality: self.quality as f32,
                }
            } else {
                Options {
                    compression: Compression::Lossless,
                    // In lossless mode, this is effort. 70 is the default effort in
                    // SkImageEncoder, which follows Blink and WebPConfigInit.
                    quality: 70.0,
                }
            };
            let data = webp_encoder::encode_image(&src, &options);
            let Some(decoded) = deferred_from_encoded_data(data, None) else {
                break;
            };

            canvas.draw_image(&src, (0.0, 0.0), None);
            canvas.translate((IMAGE_WIDTH_F, 0.0));

            canvas.draw_image(&decoded, (0.0, 0.0), None);
            canvas.translate((IMAGE_WIDTH_F * 1.5, 0.0));
        }
        DrawResult::Ok
    }
}

// Port of: gm/encode_color_types.cpp#L188 (DEF_VARIANTS, chrome/m156): the three variants of
// `webp-lossless` (quality 100) under one manifest id, in the macro's order.
crate::def_variants_gm!(
    #[ignore = "see notes/gm_encode_color_types_cpp_SkEncodedImageFormat_kWEBP.md"]
    SkEncodedImageFormat_kWEBP = "SkEncodedImageFormat::kWEBP",
    [
        EncodeColorTypesGm::new(100, Variant::Normal, "webp-lossless"),
        EncodeColorTypesGm::new(100, Variant::Opaque, "webp-lossless"),
        EncodeColorTypesGm::new(100, Variant::Gray, "webp-lossless"),
    ]
);

// Port of: gm/encode_color_types.cpp#L189 (DEF_VARIANTS, chrome/m156): `webp-lossy` (quality 80).
crate::def_variants_gm!(
    #[ignore = "see notes/gm_encode_color_types_cpp_SkEncodedImageFormat_kWEBP_2.md"]
    SkEncodedImageFormat_kWEBP_2 = "SkEncodedImageFormat::kWEBP#2",
    [
        EncodeColorTypesGm::new(80, Variant::Normal, "webp-lossy"),
        EncodeColorTypesGm::new(80, Variant::Opaque, "webp-lossy"),
        EncodeColorTypesGm::new(80, Variant::Gray, "webp-lossy"),
    ]
);
