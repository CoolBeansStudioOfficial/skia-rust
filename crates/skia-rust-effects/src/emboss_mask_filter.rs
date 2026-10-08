// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/SkEmbossMaskFilter.h, src/effects/SkEmbossMaskFilter.cpp

//! `SkEmbossMaskFilter`: a mask filter that creates a 3D emboss look, by specifying a light and
//! blur amount.
//!
//! skia-rust: flattening and `asImageFilter` (which needs the lighting and blur image filters)
//! are not ported yet.

use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{MaskFilter, MaskFilterBase, MaskFilterType};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::point3::Point3;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::scalar::{scalar, scalar_ceil_to_int};
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::emboss_mask::emboss;

/// The light of an emboss (`SkEmbossMaskFilter::Light`).
// Port of: src/effects/SkEmbossMaskFilter.h#L36-L41 (chrome/m156)
#[doc(alias = "SkEmbossMaskFilter::Light")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Light {
    /// `fDirection`: x, y, z.
    pub direction: [scalar; 3],
    /// `fPad`.
    pub pad: u16,
    /// `fAmbient`.
    pub ambient: u8,
    /// `fSpecular`: exponent, 4.4 right now.
    pub specular: u8,
}

// Port of: src/effects/SkEmbossMaskFilter.h#L33-L86 (chrome/m156)
#[derive(Clone, Debug)]
struct EmbossMaskFilter {
    light: Light,
    blur_sigma: scalar,
}

/// Creates an emboss mask filter with the given blur sigma and light (`SkEmbossMaskFilter::Make`).
/// Returns `None` if the sigma is not finite and positive or the light direction cannot be
/// normalized.
// Port of: src/effects/SkEmbossMaskFilter.cpp#L33-L48 (chrome/m156)
#[doc(alias = "SkEmbossMaskFilter::Make")]
#[must_use]
pub fn new(blur_sigma: scalar, light: &Light) -> Option<MaskFilter> {
    if !blur_sigma.is_finite() || blur_sigma <= 0.0 {
        return None;
    }

    let mut light_dir = Point3::new(light.direction[0], light.direction[1], light.direction[2]);
    if !light_dir.normalize() {
        return None;
    }
    let mut new_light = *light;
    new_light.direction[0] = light_dir.x;
    new_light.direction[1] = light_dir.y;
    new_light.direction[2] = light_dir.z;

    Some(MaskFilter::from_base(EmbossMaskFilter::new(
        blur_sigma, new_light,
    )))
}

impl EmbossMaskFilter {
    // Port of: src/effects/SkEmbossMaskFilter.cpp#L71-L76 (chrome/m156)
    fn new(blur_sigma: scalar, light: Light) -> Self {
        debug_assert!(blur_sigma > 0.0);
        debug_assert!(light.direction.iter().all(|d| d.is_finite()));
        EmbossMaskFilter { light, blur_sigma }
    }
}

/// `SkEmbossMaskFilter::CreateProc`: the light as its 16 bytes, then the sigma.
// Port of: src/effects/SkEmbossMaskFilter.cpp#L139-L147 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<MaskFilter> {
    let mut bytes = [0u8; LIGHT_SIZE];
    if !buffer.read_byte_array(&mut bytes) {
        return None;
    }
    let sigma = buffer.read_scalar();
    let light = Light {
        direction: [
            f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            f32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            f32::from_ne_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        ],
        // `light.fPad = 0`: the font-cache lookup needs it clean.
        pad: 0,
        ambient: bytes[14],
        specular: bytes[15],
    };
    new(sigma, &light)
}

/// `sizeof(SkEmbossMaskFilter::Light)`: three scalars, a `u16` pad, and two bytes.
const LIGHT_SIZE: usize = 16;

impl MaskFilterBase for EmbossMaskFilter {
    // Port of: src/effects/SkEmbossMaskFilter.h#L60 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkEmbossMaskFilter"
    }

    // Port of: src/effects/SkEmbossMaskFilter.cpp#L149-L154 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        // The light as `sizeof(Light)` bytes, with `fPad = 0` for the font-cache lookup.
        let mut bytes = [0u8; LIGHT_SIZE];
        for (i, direction) in self.light.direction.iter().enumerate() {
            bytes[4 * i..4 * i + 4].copy_from_slice(&direction.to_ne_bytes());
        }
        bytes[14] = self.light.ambient;
        bytes[15] = self.light.specular;
        buffer.write_byte_array(&bytes);
        buffer.write_scalar(self.blur_sigma);
    }

    // Port of: src/effects/SkEmbossMaskFilter.cpp#L78-L80 (chrome/m156)
    fn format(&self) -> MaskFormat {
        MaskFormat::ThreeD
    }

    // Port of: src/effects/SkEmbossMaskFilter.cpp#L82-L146 (chrome/m156)
    fn filter_mask(
        &self,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        matrix: &Matrix,
        margin: Option<&mut IPoint>,
    ) -> bool {
        if src.format != MaskFormat::A8 {
            return false;
        }

        let sigma = matrix.map_radius(self.blur_sigma);

        if !BlurMask::box_blur(dst, src, sigma, BlurStyle::Inner, None) {
            return false;
        }

        dst.format = MaskFormat::ThreeD;
        if let Some(margin) = margin {
            margin.set(
                scalar_ceil_to_int(3.0 * sigma),
                scalar_ceil_to_int(3.0 * sigma),
            );
        }

        if src.image.is_empty() {
            return true;
        }

        // create a larger buffer for the other two channels (should force fBlur to do this for us)

        {
            let total_size = dst.compute_total_image_size();
            if total_size == 0 {
                return false; // too big to allocate, abort
            }
            let plane_size = dst.compute_image_size();
            debug_assert!(plane_size != 0); // if totalSize didn't overflow, this can't either
            let mut image = MaskBuilder::alloc_image(total_size, AllocType::Uninit);
            image[..plane_size].copy_from_slice(&dst.image[..plane_size]);
            dst.image = image;
        }

        // run the light direction through the matrix...
        let mut light = self.light;
        let mut mapped = [Point::new(self.light.direction[0], self.light.direction[1])];
        matrix.map_vectors(
            &mut mapped,
            &[Point::new(self.light.direction[0], self.light.direction[1])],
        );
        light.direction[0] = mapped[0].x;
        light.direction[1] = mapped[0].y;

        // now restore the length of the XY component
        let mut vec = Point::new(light.direction[0], light.direction[1]);
        let _ = vec.set_length_xy(
            light.direction[0],
            light.direction[1],
            Point::length_xy(self.light.direction[0], self.light.direction[1]),
        );
        light.direction[0] = vec.x;
        light.direction[1] = vec.y;

        emboss(dst, &light);

        // restore original alpha
        let size = src.compute_image_size();
        dst.image[..size].copy_from_slice(&src.image[..size]);

        true
    }

    fn filter_type(&self) -> MaskFilterType {
        MaskFilterType::Emboss
    }
}
