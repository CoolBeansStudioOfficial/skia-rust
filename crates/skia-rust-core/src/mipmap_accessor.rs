// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMipmapAccessor.{h,cpp}

//! `SkMipmapAccessor`: picks the mipmap level (or the two levels to interpolate between) an
//! image shader samples, given the inverse of the matrix that maps it to device space.

use std::sync::Arc;

use crate::arena_alloc::ArenaAlloc;
use crate::bitmap::Bitmap;
use crate::floating_point::{float_floor2int, float_round2int};
use crate::image::Image;
use crate::matrix::Matrix;
use crate::mipmap::Mipmap;
use crate::pixmap::Pixmap;
use crate::sampling_options::MipmapMode;
use crate::size::Size;

/// Where a level's pixels are.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Source {
    /// The image's own pixels (`fBaseStorage`).
    Base,
    /// Level `index` of the mipmap (`fCurrMip`).
    Mip(i32),
}

/// The chosen mipmap level(s) of an image for a matrix (`SkMipmapAccessor`).
// Port of: src/core/SkMipmapAccessor.h#L24-L60 (chrome/m156)
#[doc(alias = "SkMipmapAccessor")]
#[derive(Debug)]
pub struct MipmapAccessor {
    upper: Option<Source>,
    /// Only valid for `mip_linear`.
    lower: Option<Source>,
    /// lower * weight + upper * (1 - weight)
    lower_weight: f32,
    upper_inv: Matrix,
    lower_inv: Matrix,

    // these manage lifetime for the buffers
    base_storage: Option<Bitmap>,
    curr_mip: Option<Arc<Mipmap>>,
}

impl MipmapAccessor {
    /// Picks the levels for sampling `image` through the matrix whose inverse is `inv`
    /// (`SkMipmapAccessor::SkMipmapAccessor`).
    // Port of: src/core/SkMipmapAccessor.cpp#L33-L106 (chrome/m156)
    #[must_use]
    pub fn new(image: &Image, inv: &Matrix, requested_mode: MipmapMode) -> MipmapAccessor {
        let mut this = MipmapAccessor {
            upper: None,
            lower: None,
            lower_weight: 0.0,
            upper_inv: Matrix::new_identity(),
            lower_inv: Matrix::new_identity(),
            base_storage: None,
            curr_mip: None,
        };
        let mut resolved_mode = requested_mode;

        // `load_upper_from_base`: only do this once.
        let load_upper_from_base = |this: &mut MipmapAccessor| {
            if this.base_storage.is_none() {
                this.base_storage = image.as_base().get_ro_pixels();
                this.upper = this
                    .base_storage
                    .as_ref()
                    .and_then(super::bitmap::Bitmap::peek_pixels)
                    .map(|_| Source::Base);
            }
        };

        let mut level = 0.0f32;
        if requested_mode != MipmapMode::None {
            let mut scale = None;
            if let Some(s) = inv.decompose_scale(None) {
                scale = Some(s);
            }
            match scale {
                None => resolved_mode = MipmapMode::None,
                Some(scale) => {
                    level = Mipmap::compute_level(Size::new(1.0 / scale.width, 1.0 / scale.height));
                    if level <= 0.0 {
                        resolved_mode = MipmapMode::None;
                        level = 0.0;
                    }
                }
            }
        }

        // Nearest mode uses this level, so we round to pick the nearest. In linear mode we use
        // this level as the lower of the two to interpolate between, so we take the floor.
        let level_num = if resolved_mode == MipmapMode::Nearest {
            float_round2int(level)
        } else {
            float_floor2int(level)
        };
        #[allow(clippy::cast_precision_loss)] // mirrors `level - levelNum`
        let lower_weight = level - level_num as f32; // fract(level)
        debug_assert!(level_num >= 0);

        if level_num == 0 {
            load_upper_from_base(&mut this);
        }
        // load fCurrMip if needed
        if level_num > 0 || (resolved_mode == MipmapMode::Linear && lower_weight > 0.0) {
            this.curr_mip = image.as_base().try_load_mips();
            if let Some(curr_mip) = this.curr_mip.clone() {
                debug_assert_ne!(resolved_mode, MipmapMode::None);
                if level_num > 0 {
                    if curr_mip.get_level(level_num - 1).is_some() {
                        this.upper = Some(Source::Mip(level_num - 1));
                    } else {
                        load_upper_from_base(&mut this);
                        resolved_mode = MipmapMode::None;
                    }
                }

                if resolved_mode == MipmapMode::Linear {
                    if curr_mip.get_level(level_num).is_some() {
                        this.lower = Some(Source::Mip(level_num));
                        this.lower_weight = lower_weight;
                        this.lower_inv = this.scale(image, Source::Mip(level_num));
                    } else {
                        resolved_mode = MipmapMode::Nearest;
                    }
                }
            } else {
                load_upper_from_base(&mut this);
            }
        }
        if let Some(upper) = this.upper {
            this.upper_inv = this.scale(image, upper);
        }
        let _ = resolved_mode;
        this
    }

    /// `scale`: the matrix from the base level's coordinates to `source`'s.
    fn scale(&self, image: &Image, source: Source) -> Matrix {
        let (w, h) = self
            .pixmap(source)
            .map_or((0, 0), |pm| (pm.width(), pm.height()));
        #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
        Matrix::scale((
            w as f32 / image.width() as f32,
            h as f32 / image.height() as f32,
        ))
    }

    fn pixmap(&self, source: Source) -> Option<Pixmap<'_>> {
        match source {
            Source::Base => self.base_storage.as_ref()?.peek_pixels(),
            Source::Mip(index) => Some(self.curr_mip.as_ref()?.get_level(index)?.pixmap),
        }
    }

    /// Picks the levels in `alloc`; `None` if there are no pixels to sample (`Make`).
    // Port of: src/core/SkMipmapAccessor.cpp#L108-L114 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make<'a>(
        alloc: &'a ArenaAlloc,
        image: &Image,
        inv: &Matrix,
        mipmap: MipmapMode,
    ) -> Option<&'a MipmapAccessor> {
        let access = alloc.make(MipmapAccessor::new(image, inv, mipmap));
        // return null if we failed to get the level (so the caller won't try to use it)
        access.upper.is_some().then_some(access)
    }

    /// The upper level and the matrix from the base level to it (`level`).
    /// # Panics
    /// If the accessor has no upper level pixels.
    #[must_use]
    pub fn level(&self) -> (Pixmap<'_>, Matrix) {
        let pm = self
            .upper
            .and_then(|s| self.pixmap(s))
            .expect("the upper level has pixels");
        (pm, self.upper_inv.clone())
    }

    /// The lower level (for linear mipmap filtering) and the matrix from the base level to it
    /// (`lowerLevel`).
    #[doc(alias = "lowerLevel")]
    /// # Panics
    /// If the accessor has no lower level pixels.
    #[must_use]
    pub fn lower_level(&self) -> (Pixmap<'_>, Matrix) {
        let pm = self
            .lower
            .and_then(|s| self.pixmap(s))
            .expect("the lower level has pixels");
        (pm, self.lower_inv.clone())
    }

    /// `0..1`; 0 if there is no lower level (`lowerWeight`).
    #[doc(alias = "lowerWeight")]
    #[must_use]
    pub fn lower_weight(&self) -> f32 {
        self.lower_weight
    }

    /// The owner of the pixels of the upper level, for the gather context: the shared bytes
    /// and the info of the level.
    pub(crate) fn upper_bytes(&self) -> Arc<dyn skia_rust_simd::rp::contexts::PixelBytes> {
        self.bytes(self.upper.expect("the upper level has pixels"))
    }

    /// The owner of the pixels of the lower level.
    pub(crate) fn lower_bytes(&self) -> Arc<dyn skia_rust_simd::rp::contexts::PixelBytes> {
        self.bytes(self.lower.expect("the lower level has pixels"))
    }

    fn bytes(&self, source: Source) -> Arc<dyn skia_rust_simd::rp::contexts::PixelBytes> {
        match source {
            Source::Base => self
                .base_storage
                .as_ref()
                .and_then(Bitmap::shared_pixel_bytes)
                .expect("the base level has pixels"),
            Source::Mip(index) => Arc::new(crate::mipmap::MipLevelBytes::new(
                Arc::clone(self.curr_mip.as_ref().expect("a mipmap")),
                usize::try_from(index).unwrap_or(0),
            )),
        }
    }
}
