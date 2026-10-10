// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGRenderEffect.h (the image filter part),
// modules/sksg/src/SkSGRenderEffect.cpp (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color as SkColor;
use skia_rust_core::color::Color4f;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur_filter;
use skia_rust_effects::image_filters::drop_shadow_filter;

use crate::effect_node::{effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore, inval_traits};
use crate::render_node::{Hit, RenderContext, RenderNode, ScopedRenderContext};
use crate::util::rect_contains;

/// The state of an image filter node (the `ImageFilter` base class): the revalidated filter and
/// its crop rect.
// Port of: modules/sksg/include/SkSGRenderEffect.h#L79-L101 (chrome/m156) (`ImageFilter` fields)
#[derive(Debug)]
pub struct ImageFilterBase {
    core: NodeCore,
    filter: RefCell<Option<ImageFilter>>,
    crop_rect: Cell<Option<Rect>>,
}

impl ImageFilterBase {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L162-L162 (chrome/m156) (`ImageFilter::ImageFilter`)
    fn new(weak: Weak<dyn Node>) -> Self {
        Self {
            core: NodeCore::new(inval_traits::BUBBLE_DAMAGE, weak),
            filter: RefCell::new(None),
            crop_rect: Cell::new(None),
        }
    }

    /// The revalidated filter (`getFilter`).
    #[must_use]
    pub fn filter(&self) -> Option<ImageFilter> {
        debug_assert!(!self.core.has_inval());
        self.filter.borrow().clone()
    }

    /// The crop rect (`getCropRect`).
    #[must_use]
    pub fn crop_rect(&self) -> Option<Rect> {
        self.crop_rect.get()
    }

    /// Sets the crop rect, invalidating the node if it changed (`setCropRect`).
    fn set_crop_rect(&self, node: &dyn Node, crop: Option<Rect>) {
        if self.crop_rect.get() == crop {
            return;
        }
        self.crop_rect.set(crop);
        node.invalidate();
    }
}

/// Generates the revalidation of an image filter node: the filter is rebuilt by
/// `on_revalidate_filter`, and the node has no bounds of its own.
// Port of: modules/sksg/src/SkSGRenderEffect.cpp#L165-L170 (chrome/m156) (`ImageFilter::onRevalidate`)
fn image_filter_revalidate(base: &ImageFilterBase, filter: Option<ImageFilter>) -> Rect {
    debug_assert!(base.core.has_inval());
    *base.filter.borrow_mut() = filter;
    Rect::new_empty()
}

/// A drop shadow image filter (`DropShadowImageFilter`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L213-L240 (chrome/m156)
#[doc(alias = "sksg::DropShadowImageFilter")]
#[derive(Debug)]
pub struct DropShadowImageFilter {
    base: ImageFilterBase,
    offset: Cell<(f32, f32)>,
    sigma: Cell<(f32, f32)>,
    color: Cell<SkColor>,
    mode: Cell<DropShadowMode>,
}

/// What the drop shadow draws (`DropShadowImageFilter::Mode`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L217-L217 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropShadowMode {
    /// The shadow and the input (`kShadowAndForeground`).
    #[default]
    ShadowAndForeground,
    /// The shadow alone (`kShadowOnly`).
    ShadowOnly,
}

impl DropShadowImageFilter {
    /// `DropShadowImageFilter::Make()`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            base: ImageFilterBase::new(weak.clone()),
            offset: Cell::new((0.0, 0.0)),
            sigma: Cell::new((0.0, 0.0)),
            color: Cell::new(SkColor::BLACK),
            mode: Cell::new(DropShadowMode::default()),
        })
    }

    /// The offset.
    #[must_use]
    pub fn offset(&self) -> (f32, f32) {
        self.offset.get()
    }

    /// Sets the offset, invalidating the node if it changed.
    pub fn set_offset(&self, v: (f32, f32)) {
        if self.offset.get() != v {
            self.offset.set(v);
            self.invalidate();
        }
    }

    /// The blur sigmas.
    #[must_use]
    pub fn sigma(&self) -> (f32, f32) {
        self.sigma.get()
    }

    /// Sets the blur sigmas, invalidating the node if they changed.
    pub fn set_sigma(&self, v: (f32, f32)) {
        if self.sigma.get() != v {
            self.sigma.set(v);
            self.invalidate();
        }
    }

    /// The shadow color.
    #[must_use]
    pub fn color(&self) -> SkColor {
        self.color.get()
    }

    /// Sets the shadow color, invalidating the node if it changed.
    pub fn set_color(&self, v: SkColor) {
        if self.color.get() != v {
            self.color.set(v);
            self.invalidate();
        }
    }

    /// The mode.
    #[must_use]
    pub fn mode(&self) -> DropShadowMode {
        self.mode.get()
    }

    /// Sets the mode, invalidating the node if it changed.
    pub fn set_mode(&self, v: DropShadowMode) {
        if self.mode.get() != v {
            self.mode.set(v);
            self.invalidate();
        }
    }

    /// Sets the crop rect (the `ImageFilter` attribute `CropRect`).
    pub fn set_crop_rect(&self, crop: Option<Rect>) {
        self.base.set_crop_rect(self, crop);
    }

    /// The revalidated filter.
    #[must_use]
    pub fn filter(&self) -> Option<ImageFilter> {
        self.base.filter()
    }

    /// The crop rect.
    #[must_use]
    pub fn crop_rect(&self) -> Option<Rect> {
        self.base.crop_rect()
    }
}

impl Node for DropShadowImageFilter {
    fn core(&self) -> &NodeCore {
        &self.base.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L165-L170 (chrome/m156) (`ImageFilter::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        let filter = self.on_revalidate_filter();
        image_filter_revalidate(&self.base, filter)
    }
}

impl DropShadowImageFilter {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L226-L234 (chrome/m156) (`DropShadowImageFilter::onRevalidateFilter`)
    fn on_revalidate_filter(&self) -> Option<ImageFilter> {
        let color = Color4f::from(self.color.get());
        let (offset, sigma) = (self.offset.get(), self.sigma.get());
        let crop = self.base.crop_rect();
        match self.mode.get() {
            DropShadowMode::ShadowOnly => {
                drop_shadow_filter::drop_shadow_only(offset, sigma, color, None, None, crop)
            }
            DropShadowMode::ShadowAndForeground => {
                drop_shadow_filter::drop_shadow(offset, sigma, color, None, None, crop)
            }
        }
    }
}

/// A blur image filter (`BlurImageFilter`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L242-L262 (chrome/m156)
#[doc(alias = "sksg::BlurImageFilter")]
#[derive(Debug)]
pub struct BlurImageFilter {
    base: ImageFilterBase,
    sigma: Cell<(f32, f32)>,
    tile_mode: Cell<TileMode>,
}

impl BlurImageFilter {
    /// `BlurImageFilter::Make()`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            base: ImageFilterBase::new(weak.clone()),
            sigma: Cell::new((0.0, 0.0)),
            tile_mode: Cell::new(TileMode::Decal),
        })
    }

    /// The blur sigmas.
    #[must_use]
    pub fn sigma(&self) -> (f32, f32) {
        self.sigma.get()
    }

    /// Sets the blur sigmas, invalidating the node if they changed.
    pub fn set_sigma(&self, v: (f32, f32)) {
        if self.sigma.get() != v {
            self.sigma.set(v);
            self.invalidate();
        }
    }

    /// The tile mode.
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.tile_mode.get()
    }

    /// Sets the tile mode, invalidating the node if it changed.
    pub fn set_tile_mode(&self, v: TileMode) {
        if self.tile_mode.get() != v {
            self.tile_mode.set(v);
            self.invalidate();
        }
    }

    /// The revalidated filter.
    #[must_use]
    pub fn filter(&self) -> Option<ImageFilter> {
        self.base.filter()
    }

    /// The crop rect.
    #[must_use]
    pub fn crop_rect(&self) -> Option<Rect> {
        self.base.crop_rect()
    }

    /// Sets the crop rect.
    pub fn set_crop_rect(&self, crop: Option<Rect>) {
        self.base.set_crop_rect(self, crop);
    }
}

impl Node for BlurImageFilter {
    fn core(&self) -> &NodeCore {
        &self.base.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L165-L170 (chrome/m156)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        // Tile modes other than kDecal require an explicit crop rect.
        debug_assert!(self.tile_mode.get() == TileMode::Decal || self.base.crop_rect().is_some());
        let (sx, sy) = self.sigma.get();
        let filter = blur_filter::blur(sx, sy, self.tile_mode.get(), None, self.base.crop_rect());
        image_filter_revalidate(&self.base, filter)
    }
}

/// An image filter supplied by the caller (`ExternalImageFilter`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L192-L210 (chrome/m156)
#[doc(alias = "sksg::ExternalImageFilter")]
#[derive(Debug)]
pub struct ExternalImageFilter {
    base: ImageFilterBase,
    image_filter: RefCell<Option<ImageFilter>>,
}

impl ExternalImageFilter {
    /// `ExternalImageFilter::Make()`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| Self {
            base: ImageFilterBase::new(weak.clone()),
            image_filter: RefCell::new(None),
        })
    }

    /// The caller's filter.
    #[must_use]
    pub fn image_filter(&self) -> Option<ImageFilter> {
        self.image_filter.borrow().clone()
    }

    /// Sets the caller's filter, invalidating the node if it changed.
    pub fn set_image_filter(&self, f: Option<ImageFilter>) {
        if *self.image_filter.borrow() == f {
            return;
        }
        *self.image_filter.borrow_mut() = f;
        self.invalidate();
    }

    /// The revalidated filter.
    #[must_use]
    pub fn filter(&self) -> Option<ImageFilter> {
        self.base.filter()
    }

    /// The crop rect.
    #[must_use]
    pub fn crop_rect(&self) -> Option<Rect> {
        self.base.crop_rect()
    }

    /// Sets the crop rect.
    pub fn set_crop_rect(&self, crop: Option<Rect>) {
        self.base.set_crop_rect(self, crop);
    }
}

impl Node for ExternalImageFilter {
    fn core(&self) -> &NodeCore {
        &self.base.core
    }

    // Port of: modules/sksg/include/SkSGRenderEffect.h#L199-L199 (chrome/m156) (`onRevalidateFilter`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        let filter = self.image_filter();
        image_filter_revalidate(&self.base, filter)
    }
}

/// Filters the rendering of a child (`ImageFilterEffect`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L263-L283 (chrome/m156) (`class ImageFilterEffect`)
#[doc(alias = "sksg::ImageFilterEffect")]
#[derive(Debug)]
pub struct ImageFilterEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    image_filter: Rc<dyn ImageFilterNode>,
    cropping: Cell<Cropping>,
}

/// Whether a filter crops to the content bounds (`ImageFilterEffect::Cropping`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L268-L271 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cropping {
    /// Doesn't use a crop rect.
    #[default]
    None,
    /// Uses the content bounding box as a crop rect.
    Content,
}

/// The image filter nodes an [`ImageFilterEffect`] can hold.
pub trait ImageFilterNode: Node {
    /// The revalidated filter.
    fn filter(&self) -> Option<ImageFilter>;
    /// Sets the crop rect of the filter.
    fn set_crop_rect(&self, crop: Option<Rect>);
}

impl ImageFilterNode for DropShadowImageFilter {
    fn filter(&self) -> Option<ImageFilter> {
        DropShadowImageFilter::filter(self)
    }

    fn set_crop_rect(&self, crop: Option<Rect>) {
        DropShadowImageFilter::set_crop_rect(self, crop);
    }
}

impl ImageFilterNode for BlurImageFilter {
    fn filter(&self) -> Option<ImageFilter> {
        BlurImageFilter::filter(self)
    }

    fn set_crop_rect(&self, crop: Option<Rect>) {
        BlurImageFilter::set_crop_rect(self, crop);
    }
}

impl ImageFilterNode for ExternalImageFilter {
    fn filter(&self) -> Option<ImageFilter> {
        ExternalImageFilter::filter(self)
    }

    fn set_crop_rect(&self, crop: Option<Rect>) {
        ExternalImageFilter::set_crop_rect(self, crop);
    }
}

impl ImageFilterEffect {
    /// `ImageFilterEffect::Make(child, filter)`: the child itself if there is no filter.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Rc<dyn RenderNode>,
        filter: Option<Rc<dyn ImageFilterNode>>,
    ) -> Rc<dyn RenderNode> {
        let Some(filter) = filter else {
            return child;
        };
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            // filters always override descendent damage
            core: NodeCore::new(inval_traits::OVERRIDE_DAMAGE, weak.clone()),
            child: Rc::clone(&child),
            image_filter: Rc::clone(&filter),
            cropping: Cell::new(Cropping::None),
        });
        // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L184-L188 (chrome/m156) (`ImageFilterEffect::ImageFilterEffect`)
        effect.observe_inval(effect.child.as_ref());
        effect.observe_inval(effect.image_filter.as_ref());
        effect
    }

    /// The cropping mode.
    #[must_use]
    pub fn cropping(&self) -> Cropping {
        self.cropping.get()
    }

    /// Sets the cropping mode, invalidating the node if it changed.
    pub fn set_cropping(&self, cropping: Cropping) {
        if self.cropping.get() != cropping {
            self.cropping.set(cropping);
            self.invalidate();
        }
    }
}

impl Drop for ImageFilterEffect {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L190-L192 (chrome/m156) (`~ImageFilterEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
        self.unobserve_inval(self.image_filter.as_ref());
    }
}

impl Node for ImageFilterEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L194-L213 (chrome/m156) (`ImageFilterEffect::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        let content_bounds = effect_on_revalidate(&self.child, ic.as_deref_mut(), ctm);
        if self.cropping.get() == Cropping::Content {
            self.image_filter.set_crop_rect(Some(content_bounds));
        } else {
            self.image_filter.set_crop_rect(None);
        }
        // FIXME: image filter effects should replace the descendents' damage!
        self.image_filter.revalidate(ic, ctm);
        match self.image_filter.filter() {
            // Would be nice for this to stick, but canComputeFastBounds() is conservative.
            Some(filter) => filter.compute_fast_bounds(content_bounds),
            None => content_bounds,
        }
    }
}

impl RenderNode for ImageFilterEffect {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L215-L225 (chrome/m156) (`ImageFilterEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        // Note: we use the source content bounds for saveLayer, not our local/filtered bounds.
        let scope = ScopedRenderContext::new(canvas, ctx).set_filter_isolation(
            &self.child.core().bounds(),
            &canvas.total_matrix(),
            self.image_filter.filter(),
        );
        effect_on_render(&self.child, canvas, Some(scope.context()));
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L227-L233 (chrome/m156) (`ImageFilterEffect::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        // TODO: map p through the filter DAG and dispatch to descendants?
        // For now, image filters occlude hit-testing.
        debug_assert!(rect_contains(&self.core.bounds(), p));
        let _ = p;
        Some(Hit::This)
    }
}
