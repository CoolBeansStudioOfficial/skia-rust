// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkShaderMaskFilter.h, src/effects/SkShaderMaskFilterImpl.h,
// src/effects/SkShaderMaskFilterImpl.cpp

//! `SkShaderMaskFilter`: a (deprecated) mask filter that draws a shader into the mask, so that the
//! shader's alpha replaces the mask's coverage.
//!
//! skia-rust: flattening (`writeFlattenable(shader)`) and `asImageFilter` (which needs
//! `SkImageFilters::Shader` and `Blend`) are not ported yet.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{AllocType, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{MaskFilter, MaskFilterBase, MaskFilterType};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_raster::surfaces;

/// `SkShaderMaskFilterImpl`.
// Port of: src/effects/SkShaderMaskFilterImpl.h#L19-L50 (chrome/m156)
#[derive(Clone, Debug)]
struct ShaderMaskFilterImpl {
    /// `fShader`.
    shader: Shader,
}

impl MaskFilterBase for ShaderMaskFilterImpl {
    // Port of: src/effects/SkShaderMaskFilterImpl.h#L26 (chrome/m156), getFormat
    fn format(&self) -> MaskFormat {
        MaskFormat::A8
    }

    // Port of: src/effects/SkShaderMaskFilterImpl.h#L27 (chrome/m156), type
    fn filter_type(&self) -> MaskFilterType {
        MaskFilterType::Shader
    }

    // Port of: src/effects/SkShaderMaskFilterImpl.h#L36-L38 (chrome/m156), computeFastBounds
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        *src
    }

    // Port of: src/effects/SkShaderMaskFilterImpl.cpp#L58-L100 (chrome/m156), filterMask
    fn filter_mask(
        &self,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        ctm: &Matrix,
        margin: Option<&mut IPoint>,
    ) -> bool {
        if src.format != MaskFormat::A8 {
            return false;
        }

        if let Some(margin) = margin {
            margin.set(0, 0);
        }
        dst.bounds = src.bounds;
        #[allow(clippy::cast_sign_loss)] // width is positive, mirrors the C++ uint32 rowBytes
        {
            dst.row_bytes = dst.bounds.width() as u32; // need alignment?
        }
        dst.format = MaskFormat::A8;

        if src.image.is_empty() {
            dst.image = Vec::new();
            return true;
        }
        let size = dst.compute_image_size();
        if size == 0 {
            return false; // too big to allocate, abort
        }

        // Allocate and initialize dst image with a copy of the src image (`rect_memcpy`).
        dst.image = MaskBuilder::alloc_image(size, AllocType::Uninit);
        let width = usize::try_from(dst.bounds.width()).expect("positive");
        let height = usize::try_from(dst.bounds.height()).expect("positive");
        let dst_rb = dst.row_bytes as usize;
        let src_rb = src.row_bytes as usize;
        for y in 0..height {
            dst.image[y * dst_rb..y * dst_rb + width]
                .copy_from_slice(&src.image[y * src_rb..y * src_rb + width]);
        }

        // Now we have a dst-mask, just need to setup a canvas and draw into it.
        let info = ImageInfo::new_a8((dst.bounds.width(), dst.bounds.height()));
        let row_bytes = dst.row_bytes as usize;
        let left = dst.bounds.left;
        let top = dst.bounds.top;
        let Some(mut surface) =
            surfaces::wrap_pixels(&info, dst.image.as_mut_slice(), row_bytes, None)
        else {
            return false;
        };

        let mut paint = Paint::default();
        paint.set_shader(Some(self.shader.clone()));
        // this blendmode is the trick: we only draw the shader where the mask is
        paint.set_blend_mode(BlendMode::SrcIn);

        let canvas = surface.canvas();
        // mirrors SkIntToScalar
        #[allow(clippy::cast_precision_loss)]
        canvas.translate((-(left as f32), -(top as f32)));
        canvas.concat(ctm);
        canvas.draw_paint(&paint);
        true
    }
}

/// `SkShaderMaskFilter::Make(shader)`: a mask filter that fills the mask with `shader`
/// (deprecated upstream).
// Port of: src/effects/SkShaderMaskFilterImpl.cpp#L103-L105 (chrome/m156)
#[doc(alias = "SkShaderMaskFilter::Make")]
#[must_use]
pub fn new(shader: impl Into<Shader>) -> MaskFilter {
    MaskFilter::from_base(ShaderMaskFilterImpl {
        shader: shader.into(),
    })
}
