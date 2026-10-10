// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RasterPathUtils.h, src/gpu/graphite/RasterPathUtils.cpp
// (chrome/m156)

//! CPU rasterization of path coverage masks for the raster path atlases: an A8 mask is allocated
//! with padding, and the shape is drawn into it with the raster backend's `SkDraw` (the
//! `SkA8Blitter_Choose` blitter), so the mask matches what Skia's CPU raster produces.

// The mask offsets are small non-negative `int`s in the C++, converted to indices and to floats
// exactly as the C++ converts them.
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)]

use std::sync::OnceLock;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::fixed::scalar_to_fixed;
use skia_rust_core::float_bits::float_to_bits;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::scalar::scalar_fraction;
use skia_rust_core::size::ISize;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_raster::draw::Draw;
use skia_rust_raster::glyph_image::choose_a8;
use skia_rust_raster::raster_clip::RasterClip;

use crate::gpu::resource_key::{UniqueKey, UniqueKeyBuilder, UniqueKeyDomain};
use crate::graphite::clip_stack::ClipElement;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::Transform;

/// The `Half2` position or size of an atlas entry, as `skvx::half2` holds it.
pub type Half2 = (u16, u16);

/// `RasterMaskHelper`: draws shapes into an A8 pixmap, translated by `translate`.
// Port of: src/gpu/graphite/RasterPathUtils.h (chrome/m156)
#[derive(Debug)]
pub struct RasterMaskHelper<'a> {
    info: ImageInfo,
    pixels: &'a mut [u8],
    row_bytes: usize,
    raster_clip: RasterClip,
    translate: IPoint,
}

/// The owned, padded pixels of a mask (`SkBitmap` in `RasterMaskHelper::Allocate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskBuffer {
    pixels: Vec<u8>,
    row_bytes: usize,
    size: ISize,
    padding: i32,
}

impl MaskBuffer {
    /// The pixels of the whole padded buffer.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// The row stride, in bytes.
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// The size of the mask, without its padding.
    #[must_use]
    pub fn size(&self) -> ISize {
        self.size
    }

    /// The padding on each side of the mask.
    #[must_use]
    pub fn padding(&self) -> i32 {
        self.padding
    }

    /// The padded buffer as an immutable A8 bitmap (`bm.setImmutable()` in the C++ callers of
    /// `RasterMaskHelper::Allocate`).
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L19-L33 (chrome/m156), the SkBitmap returned
    #[must_use]
    pub fn to_bitmap(&self) -> Bitmap {
        let padded = ISize::new(
            self.size.width + 2 * self.padding,
            self.size.height + 2 * self.padding,
        );
        let info = ImageInfo::new(padded, ColorType::Alpha8, AlphaType::Premul, None);
        let mut bitmap = Bitmap::new();
        let set = bitmap.set_info(&info, self.row_bytes);
        debug_assert!(set);
        bitmap.set_pixels(self.pixels.clone());
        bitmap.set_immutable();
        bitmap
    }
}

impl RasterMaskHelper<'_> {
    /// `RasterMaskHelper::Allocate(size, translation, padding, initialAlpha)`: the padded mask
    /// buffer. The helper of the mask is [`RasterMaskHelper::over`].
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L19-L33 (chrome/m156)
    #[doc(alias = "Allocate")]
    #[must_use]
    pub fn allocate(size: ISize, padding: i32, initial_alpha: u8) -> MaskBuffer {
        debug_assert!(padding >= 0);
        debug_assert!(!size.is_empty());
        let padded_w = size.width + 2 * padding;
        let padded_h = size.height + 2 * padding;
        // `SkBitmap::allocPixels` uses the minimum row bytes of an A8 image.
        let row_bytes = padded_w as usize;
        MaskBuffer {
            pixels: vec![initial_alpha; row_bytes * padded_h as usize],
            row_bytes,
            size,
            padding,
        }
    }

    /// The helper that draws into the inner (unpadded) region of `buffer`.
    /// `translation` is the device offset of the mask.
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L35-L44 (chrome/m156)
    #[must_use]
    pub fn over(buffer: &mut MaskBuffer, translation: IPoint) -> RasterMaskHelper<'_> {
        let offset = buffer.padding as usize * buffer.row_bytes + buffer.padding as usize;
        let info = ImageInfo::new(buffer.size, ColorType::Alpha8, AlphaType::Premul, None);
        let size = buffer.size;
        let row_bytes = buffer.row_bytes;
        RasterMaskHelper {
            info,
            pixels: &mut buffer.pixels[offset..],
            row_bytes,
            raster_clip: RasterClip::from_rect(&IRect::from_size(size)),
            translate: translation,
        }
    }

    /// `RasterMaskHelper(pixmap, translation)`: the helper of an A8 pixmap, such as the entry of a
    /// `DrawAtlas` (`prepForRender`). The pixmap's pixels are written through the helper.
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L46-L56 (chrome/m156)
    ///
    /// # Panics
    /// If the pixmap has no writable pixels.
    #[must_use]
    pub fn over_pixmap<'a>(
        pixmap: &'a mut Pixmap<'_>,
        translation: IPoint,
    ) -> RasterMaskHelper<'a> {
        let info = pixmap.info().clone();
        let row_bytes = pixmap.row_bytes();
        let raster_clip = RasterClip::from_rect(&IRect::from_size(info.dimensions()));
        let pixels = pixmap
            .writable_addr()
            .expect("the atlas entry has writable pixels");
        RasterMaskHelper {
            info,
            pixels,
            row_bytes,
            raster_clip,
            translate: translation,
        }
    }

    /// `drawShape(shape, localToDevice, strokeRec)`: draws the shape's coverage (anti-aliased,
    /// replacing the destination).
    ///
    /// # Panics
    /// Never: the pixmap is always at least the mask's image size.
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L58-L72 (chrome/m156)
    pub fn draw_shape(
        &mut self,
        shape: &Shape,
        local_to_device: &Transform,
        stroke_rec: &StrokeRec,
    ) {
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src); // "Replace" mode
        paint.set_anti_alias(true);
        paint.set_color(Color::new(0xFFFF_FFFF)); // SK_ColorWHITE
        stroke_rec.apply_to_paint(&mut paint);

        let translated = self.translated_matrix(local_to_device);
        let mut path = shape.as_path();
        if path.is_inverse_fill_type() {
            path.toggle_inverse_fill_type();
        }
        let info = self.info.clone();
        let row_bytes = self.row_bytes;
        let pixmap = Pixmap::new(&info, &mut *self.pixels, row_bytes)
            .expect("the mask is at least its image size");
        let mut draw = Draw {
            dst: pixmap,
            blitter_chooser: choose_a8,
            ctm: &translated,
            rc: &self.raster_clip,
            props: None,
        };
        draw.draw_path_coverage(&path, &paint, None);
    }

    /// `drawClip(shape, localToDevice, alpha)`: draws a clip element's coverage at `alpha`.
    ///
    /// # Panics
    /// Never: the pixmap is always at least the mask's image size.
    // Port of: src/gpu/graphite/RasterPathUtils.cpp#L74-L92 (chrome/m156)
    pub fn draw_clip(&mut self, shape: &Shape, local_to_device: &Transform, alpha: u8) {
        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src); // "Replace" mode
        paint.set_anti_alias(true);
        // SkColorSetARGB(alpha, 0xFF, 0xFF, 0xFF)
        paint.set_color(Color::new((u32::from(alpha) << 24) | 0x00FF_FFFF));

        let translated = self.translated_matrix(local_to_device);
        let path = shape.as_path();
        let info = self.info.clone();
        let row_bytes = self.row_bytes;
        let pixmap = Pixmap::new(&info, &mut *self.pixels, row_bytes)
            .expect("the mask is at least its image size");
        let mut draw = Draw {
            dst: pixmap,
            blitter_chooser: choose_a8,
            ctm: &translated,
            rc: &self.raster_clip,
            props: None,
        };
        if 0xFF == alpha {
            draw.draw_path_coverage(&path, &paint, None);
        } else {
            draw.draw_path(&path, &paint, None);
        }
    }

    /// `SkMatrix(localToDevice)` followed by `postTranslate(fTranslate)`.
    fn translated_matrix(&self, local_to_device: &Transform) -> Matrix {
        let mut m = local_to_device.matrix().to_m33();
        m.post_translate((self.translate.x as f32, self.translate.y as f32));
        m
    }
}

/// `add_transform_key()`: writes the linear part of `transform` at `start_index` and returns the
/// sub-pixel bits of its translation.
// Port of: src/gpu/graphite/RasterPathUtils.cpp#L94-L115 (chrome/m156)
fn add_transform_key(
    builder: &mut UniqueKeyBuilder<'_>,
    start_index: usize,
    transform: &Transform,
) -> u32 {
    let mat = transform.matrix().to_m33();
    let sx = mat.scale_x();
    let sy = mat.scale_y();
    let kx = mat.skew_x();
    let ky = mat.skew_y();

    let tx = mat.translate_x();
    let ty = mat.translate_y();
    let frac_x = scalar_to_fixed(scalar_fraction(tx)) & 0x0000_FF00;
    let frac_y = scalar_to_fixed(scalar_fraction(ty)) & 0x0000_FF00;

    builder[start_index] = float_to_bits(sx);
    builder[start_index + 1] = float_to_bits(sy);
    builder[start_index + 2] = float_to_bits(kx);
    builder[start_index + 3] = float_to_bits(ky);

    (frac_x | (frac_y >> 8)) as u32
}

/// The domain of the raster path masks' keys (`static const Domain kDomain`).
fn path_mask_domain() -> UniqueKeyDomain {
    static DOMAIN: OnceLock<UniqueKeyDomain> = OnceLock::new();
    *DOMAIN.get_or_init(UniqueKey::generate_domain)
}

/// `GeneratePathMaskKey(shape, transform, strokeRec, maskOrigin, maskSize)`: the key of a
/// rasterized path mask, which depends on the shape, the transform's linear part and sub-pixel
/// translation, the style and the mask's placement.
// Port of: src/gpu/graphite/RasterPathUtils.cpp#L117-L152 (chrome/m156)
#[doc(alias = "GeneratePathMaskKey")]
#[must_use]
pub fn generate_path_mask_key(
    shape: &Shape,
    transform: &Transform,
    stroke_rec: &StrokeRec,
    mask_origin: Half2,
    mask_size: Half2,
) -> UniqueKey {
    let mut mask_key = UniqueKey::new();
    let mut style_key_size: u16 = 7;
    if !stroke_rec.is_hairline_style() && !stroke_rec.is_fill_style() {
        style_key_size += 2;
    }
    {
        let mut builder = UniqueKeyBuilder::new(
            &mut mask_key,
            path_mask_domain(),
            style_key_size + shape.key_size(),
            Some("Raster Path Mask"),
        );
        builder[0] = u32::from(mask_origin.0) | (u32::from(mask_origin.1) << 16);
        builder[1] = u32::from(mask_size.0) | (u32::from(mask_size.1) << 16);
        let frac_bits = add_transform_key(&mut builder, 2, transform);

        let mut style_bits = stroke_rec.style() as u32;
        if !stroke_rec.is_fill_style() {
            style_bits |= cap_bits(stroke_rec.cap()) << 2;
        }
        if !stroke_rec.is_hairline_style() && !stroke_rec.is_fill_style() {
            style_bits |= join_bits(stroke_rec.join()) << 4;
            builder[6] = float_to_bits(stroke_rec.width());
            builder[7] = float_to_bits(stroke_rec.miter());
        }
        builder[usize::from(style_key_size) - 1] = frac_bits | (style_bits << 16);

        // `shape.writeKey(&builder[styleKeySize], false)`: written through a scratch buffer.
        let shape_key_start = usize::from(style_key_size);
        let mut shape_words = vec![0u32; usize::from(shape.key_size())];
        shape.write_key(&mut shape_words, /*include_inverted=*/ false);
        for (i, word) in shape_words.into_iter().enumerate() {
            builder[shape_key_start + i] = word;
        }
        builder.finish();
    }
    mask_key
}

/// `SkPaint::Cap` as the key stores it (`kButt_Cap` = 0, `kRound_Cap`, `kSquare_Cap`).
fn cap_bits(cap: Cap) -> u32 {
    cap as u32
}

/// `SkPaint::Join` as the key stores it (`kMiter_Join` = 0, `kRound_Join`, `kBevel_Join`).
fn join_bits(join: Join) -> u32 {
    join as u32
}

/// The domain of the clip masks' keys (`static const Domain kDomain` of `GenerateClipMaskKey`).
fn clip_mask_domain() -> UniqueKeyDomain {
    static DOMAIN: OnceLock<UniqueKeyDomain> = OnceLock::new();
    *DOMAIN.get_or_init(UniqueKey::generate_domain)
}

/// `GenerateClipMaskKey(stackRecordID, elementsForMask, maskDeviceBounds, includeBounds, keyBounds,
/// usesPathKey)`: the key of the mask of a list of clip elements. With at most two elements the
/// key is made of their transforms, operations and shapes (`usesPathKey`); otherwise it is the
/// save record's ID. `keyBounds` are the mask bounds relative to the full transformed mask (used
/// to tell apart masks with equal bounds but different sub-pixel translations).
// Port of: src/gpu/graphite/RasterPathUtils.cpp#L176-L251 (chrome/m156)
#[doc(alias = "GenerateClipMaskKey")]
#[must_use]
pub fn generate_clip_mask_key(
    stack_record_id: u32,
    elements_for_mask: &[&ClipElement],
    mask_device_bounds: IRect,
    include_bounds: bool,
) -> ClipMaskKey {
    const K_MAX_SHAPE_COUNT_FOR_KEY: usize = 2;
    const K_XFORM_KEY_SIZE: u16 = 5;

    let mut mask_key = UniqueKey::new();
    // if the element list is too large we just use the stackRecordID
    if elements_for_mask.len() <= K_MAX_SHAPE_COUNT_FOR_KEY {
        let mut key_size: u16 = if include_bounds { 2 } else { 0 };
        // Iterate through to get key size; given kMaxShapeCountForKey and Shape's own key size
        // limitations, this should always fit safely within a 16-bit number.
        for element in elements_for_mask {
            key_size += K_XFORM_KEY_SIZE + element.shape.key_size();
        }

        let mut element_key_index: usize = 0;
        let mut unclipped_bounds = Rect::infinite_inverted();
        let key_bounds = {
            let mut builder = UniqueKeyBuilder::new(
                &mut mask_key,
                clip_mask_domain(),
                key_size,
                Some("Clip Path Mask"),
            );
            for element in elements_for_mask {
                // Add transform key and get packed fractional translation bits
                let frac_bits =
                    add_transform_key(&mut builder, element_key_index, &element.local_to_device);
                let op_bits = element.op as u32;
                builder[element_key_index + 4] = frac_bits | (op_bits << 16);

                let shape = &element.shape;
                let mut shape_words = vec![0u32; usize::from(shape.key_size())];
                shape.write_key(&mut shape_words, /*include_inverted=*/ true);
                for (i, word) in shape_words.into_iter().enumerate() {
                    builder[element_key_index + usize::from(K_XFORM_KEY_SIZE) + i] = word;
                }

                element_key_index += usize::from(K_XFORM_KEY_SIZE) + usize::from(shape.key_size());

                let transformed_bounds = element.local_to_device.map_rect(&shape.bounds());
                unclipped_bounds.join(transformed_bounds);
            }

            // The keyBounds are the maskDeviceBounds relative to the full transformed mask. We use
            // this to ensure we capture the situation where the maskDeviceBounds are equal in two
            // cases but actually enclose different regions of the full mask due to an integer
            // translation (which is not captured in the key) in the element transforms.
            let ul = unclipped_bounds.left();
            let ut = unclipped_bounds.top();
            let mut key_bounds = mask_device_bounds;
            key_bounds.offset((-(ul as i32), -(ut as i32)));

            if include_bounds {
                builder[element_key_index] =
                    (key_bounds.left as u32) | ((key_bounds.top as u32) << 16);
                builder[element_key_index + 1] =
                    (key_bounds.right as u32) | ((key_bounds.bottom as u32) << 16);
            }
            builder.finish();
            key_bounds
        };
        return ClipMaskKey {
            key: mask_key,
            key_bounds,
            uses_path_key: true,
        };
    }

    // Either we have too many elements or at least one shape can't create a key
    {
        let mut builder = UniqueKeyBuilder::new(
            &mut mask_key,
            clip_mask_domain(),
            1,
            Some("Clip SaveRecord Mask"),
        );
        builder[0] = stack_record_id;
        builder.finish();
    }

    // It doesn't matter what the keyBounds are in this case -- the stackRecordID is enough to
    // distinguish between clips.
    ClipMaskKey {
        key: mask_key,
        key_bounds: IRect::default(),
        uses_path_key: false,
    }
}

/// The result of [`generate_clip_mask_key`]: the key, its bounds (`keyBounds`) and whether the
/// key is made of the elements (`usesPathKey`) or of the save record ID.
#[derive(Clone, Debug)]
pub struct ClipMaskKey {
    /// The mask's key.
    pub key: UniqueKey,
    /// The mask bounds relative to the full transformed mask.
    pub key_bounds: IRect,
    /// Whether the key is made of the elements (otherwise of the save record's ID).
    pub uses_path_key: bool,
}
