// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGImage.h, modules/sksg/src/SkSGImage.cpp (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::image::Image as SkImage;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;

use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::render_node::{Hit, RenderContext, RenderNode, ScopedRenderContext};

/// Draws an image at the origin of the local coordinates (`Image`).
// Port of: modules/sksg/include/SkSGImage.h#L15-L40 (chrome/m156) (`class Image`)
#[doc(alias = "sksg::Image")]
#[derive(Debug)]
#[allow(clippy::struct_field_names)] // mirrors the Skia header, where the field is prefixed
pub struct Image {
    core: NodeCore,
    image: RefCell<Option<SkImage>>,
    sampling_options: Cell<SamplingOptions>,
    anti_alias: Cell<bool>,
}

impl Image {
    /// `Image::Make(image)`.
    // Port of: modules/sksg/include/SkSGImage.h#L18-L20 (chrome/m156) (`Image::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(image: Option<SkImage>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            image: RefCell::new(image),
            sampling_options: Cell::new(SamplingOptions::default()),
            anti_alias: Cell::new(true),
        })
    }

    /// The image (`getImage`).
    #[must_use]
    pub fn image(&self) -> Option<SkImage> {
        self.image.borrow().clone()
    }

    /// Sets the image, invalidating the node if it changed (`setImage`).
    pub fn set_image(&self, image: Option<SkImage>) {
        if *self.image.borrow() != image {
            *self.image.borrow_mut() = image;
            self.invalidate();
        }
    }

    /// The sampling options (`getSamplingOptions`).
    #[must_use]
    pub fn sampling_options(&self) -> SamplingOptions {
        self.sampling_options.get()
    }

    /// Sets the sampling options, invalidating the node if they changed (`setSamplingOptions`).
    pub fn set_sampling_options(&self, options: SamplingOptions) {
        if self.sampling_options.get() != options {
            self.sampling_options.set(options);
            self.invalidate();
        }
    }

    /// Whether the image is anti-aliased (`getAntiAlias`).
    #[must_use]
    pub fn anti_alias(&self) -> bool {
        self.anti_alias.get()
    }

    /// Sets anti-aliasing, invalidating the node if it changed (`setAntiAlias`).
    pub fn set_anti_alias(&self, anti_alias: bool) {
        if self.anti_alias.get() != anti_alias {
            self.anti_alias.set(anti_alias);
            self.invalidate();
        }
    }
}

impl Node for Image {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGImage.cpp#L46-L48 (chrome/m156) (`Image::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        match self.image.borrow().as_ref() {
            Some(image) => Rect::from_irect(image.bounds()),
            None => Rect::new_empty(),
        }
    }
}

impl RenderNode for Image {
    // Port of: modules/sksg/src/SkSGImage.cpp#L20-L39 (chrome/m156) (`Image::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let Some(image) = self.image.borrow().clone() else {
            return;
        };
        let mut paint = skia_rust_core::paint::Paint::default();
        paint.set_anti_alias(self.anti_alias.get());

        let mut local_ctx = ScopedRenderContext::new(canvas, ctx);
        if let Some(ctx) = ctx {
            if ctx.mask_shader.is_some() {
                // Mask shaders cannot be applied via drawImage - we need layer isolation.
                local_ctx =
                    local_ctx.set_isolation(&self.core.bounds(), &canvas.total_matrix(), true);
            }
            local_ctx
                .context()
                .modulate_paint(&canvas.total_matrix(), &mut paint, false);
        }
        canvas.draw_image_with_sampling_options(
            &image,
            Point { x: 0.0, y: 0.0 },
            self.sampling_options.get(),
            Some(&paint),
        );
    }

    // Port of: modules/sksg/src/SkSGImage.cpp#L41-L44 (chrome/m156) (`Image::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        debug_assert!(crate::util::rect_contains(&self.core.bounds(), p));
        Some(Hit::This)
    }
}
