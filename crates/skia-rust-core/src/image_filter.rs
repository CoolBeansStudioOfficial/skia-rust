// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImageFilter.h, src/core/SkImageFilter.cpp,
// src/core/SkImageFilter_Base.h

//! `SkImageFilter`: filters applied to the rendered result of a draw.
//!
//! [`ImageFilter`] is the shared handle (`sk_sp<SkImageFilter>`). [`ImageFilterBase`] is
//! `SkImageFilter_Base`: a filter implements the `on_*` hooks, and the default methods of the
//! trait are the non-virtual parts of `SkImageFilter_Base` (the DAG recursion over inputs,
//! `filterImage`, the bounds queries). The filters themselves live in `skia-rust-effects`.
//!
//! skia-rust: the image filter cache (`SkImageFilterCache`) is not ported, so `filterImage` always
//! evaluates the filter (the cache only saves work; results are the same). Serialization
//! (`flatten`/`Unflatten`) is not ported.

use core::any::Any;
use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::color_filter::ColorFilter;
use crate::image::Image;
use crate::image_filter_result::FilterResult;
use crate::image_filter_types::{Backend, Context, Mapping, MatrixCapability, Stats, round_out};
use crate::local_matrix_image_filter::make_local_matrix_image_filter;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::point::{IPoint, Point};
use crate::rect::{Contains, IRect, Rect, rect_priv};

/// `SkImageFilter_Base::Direction`'s `SkImageFilter::MapDirection`.
// Port of: include/core/SkImageFilter.h#L120-L125 (chrome/m156)
#[doc(alias = "SkImageFilter::MapDirection")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MapDirection {
    /// `kForward_MapDirection`.
    Forward,
    /// `kReverse_MapDirection`.
    Reverse,
}

static NEXT_IMAGE_FILTER_UNIQUE_ID: AtomicU32 = AtomicU32::new(1);

/// The state every `SkImageFilter_Base` carries (`fInputs`, `fUsesSrcInput`, `fUniqueID`).
#[derive(Debug)]
pub struct ImageFilterCommon {
    inputs: Vec<Option<ImageFilter>>,
    uses_src_input: bool,
    unique_id: u32,
}

impl ImageFilterCommon {
    /// `SkImageFilter_Base(inputs, inputCount, usesSrc)`: when `uses_src` is `None`, the filter
    /// uses the source if any input is null or uses it.
    // Port of: src/core/SkImageFilter.cpp#L143-L157 (chrome/m156)
    #[must_use]
    pub fn new(inputs: Vec<Option<ImageFilter>>, uses_src: Option<bool>) -> ImageFilterCommon {
        let mut uses_src_input = uses_src.unwrap_or(false);
        if uses_src.is_none() {
            for input in &inputs {
                match input {
                    None => uses_src_input = true,
                    Some(f) if f.uses_source() => uses_src_input = true,
                    _ => {}
                }
            }
        }
        let unique_id = loop {
            let id = NEXT_IMAGE_FILTER_UNIQUE_ID.fetch_add(1, Ordering::Relaxed);
            if id != 0 {
                break id;
            }
        };
        ImageFilterCommon {
            inputs,
            uses_src_input,
            unique_id,
        }
    }

    /// `countInputs`.
    #[must_use]
    pub fn count_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// `getInput(i)`: `None` if the input is not connected.
    #[must_use]
    pub fn input(&self, i: usize) -> Option<&ImageFilter> {
        self.inputs[i].as_ref()
    }
}

/// The virtual interface of an image filter (`SkImageFilter_Base`).
///
/// Implementors provide [`common`](Self::common) and the `on_*` hooks; the default methods are
/// the DAG algorithms of `SkImageFilter_Base` and must not be overridden.
// Port of: src/core/SkImageFilter_Base.h#L23-L308 (chrome/m156)
#[doc(alias = "SkImageFilter_Base")]
pub trait ImageFilterBase: Any + fmt::Debug + Send + Sync {
    /// The inputs, usage flags and unique ID of this filter.
    fn common(&self) -> &ImageFilterCommon;

    /// Return the color filter if this node is just a color filter without crop constraints
    /// (`onIsColorFilterNode`).
    #[doc(alias = "onIsColorFilterNode")]
    fn on_is_color_filter_node(&self) -> Option<ColorFilter> {
        None
    }

    /// The most complex matrix type this filter can support (`onGetCTMCapability`).
    #[doc(alias = "onGetCTMCapability")]
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::ScaleTranslate
    }

    /// True if this filter maps transparent black to something else (`onAffectsTransparentBlack`).
    #[doc(alias = "onAffectsTransparentBlack")]
    fn on_affects_transparent_black(&self) -> bool {
        false
    }

    /// True if `affectsTransparentBlack` should ignore the inputs (`ignoreInputsAffectsTransparentBlack`).
    #[doc(alias = "ignoreInputsAffectsTransparentBlack")]
    fn ignore_inputs_affects_transparent_black(&self) -> bool {
        false
    }

    /// Filters the source image (`onFilterImage`). Subclasses recurse into their inputs with
    /// [`ImageFilterBase::get_child_output`].
    #[doc(alias = "onFilterImage")]
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult;

    /// The layer-space input bounds that cover `desired_output` (`onGetInputLayerBounds`).
    #[doc(alias = "onGetInputLayerBounds")]
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect;

    /// The layer-space output bounds for content of `content_bounds`; `None` if unbounded
    /// (`onGetOutputLayerBounds`).
    #[doc(alias = "onGetOutputLayerBounds")]
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect>;

    /// `countInputs`.
    #[doc(alias = "countInputs")]
    fn count_inputs(&self) -> usize {
        self.common().count_inputs()
    }

    /// `getInput(index)`.
    #[doc(alias = "getInput")]
    fn get_input(&self, index: usize) -> Option<&ImageFilter> {
        self.common().input(index)
    }

    /// `usesSource()`: the graph references the context's source image.
    fn uses_source(&self) -> bool {
        self.common().uses_src_input
    }

    /// `uniqueID()`.
    fn unique_id(&self) -> u32 {
        self.common().unique_id
    }

    /// `affectsTransparentBlack()`: whether a transparent black source can come out non-transparent.
    // Port of: src/core/SkImageFilter.cpp#L102-L116 (chrome/m156)
    #[doc(alias = "affectsTransparentBlack")]
    fn affects_transparent_black(&self) -> bool {
        if self.on_affects_transparent_black() {
            return true;
        } else if self.ignore_inputs_affects_transparent_black() {
            // TODO(skbug.com/40045513): Automatically infer this from output bounds being finite
            return false;
        }
        for i in 0..self.count_inputs() {
            if self
                .get_input(i)
                .is_some_and(|input| input.as_base().affects_transparent_black())
            {
                return true;
            }
        }
        false
    }

    /// `getCTMCapability()`: the most complex matrix the filter DAG can handle.
    // Port of: src/core/SkImageFilter_Base.cpp (getCTMCapability) (chrome/m156)
    #[doc(alias = "getCTMCapability")]
    fn get_ctm_capability(&self) -> MatrixCapability {
        let mut result = self.on_get_ctm_capability();
        for i in 0..self.count_inputs() {
            if let Some(input) = self.get_input(i) {
                result = result.min(input.as_base().get_ctm_capability());
            }
        }
        result
    }

    /// `SkImageFilter::computeFastBounds`: the device bounds of geometry with bounds `src`.
    // Port of: src/core/SkImageFilter.cpp#L82-L96 (chrome/m156)
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        if self.count_inputs() == 0 {
            return *src;
        }
        let mut combined = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        for i in 1..self.count_inputs() {
            match self.get_input(i) {
                Some(input) => combined.join(input.compute_fast_bounds(src)),
                None => combined.join(src),
            }
        }
        combined
    }

    /// `getInputBounds`: the layer-space bounds of the device to allocate for the source.
    // Port of: src/core/SkImageFilter_Base.cpp (getInputBounds) (chrome/m156)
    #[doc(alias = "getInputBounds")]
    fn get_input_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        known_content_bounds: Option<Rect>,
    ) -> IRect {
        // Map both the device-space desired coverage area and the known content bounds to layer
        // space
        let desired_bounds = mapping.device_to_layer(&desired_output);
        // If we have no known content bounds, leave 'contentBounds' uninstantiated to represent
        // infinite possible content.
        let content_bounds = known_content_bounds
            .map(|content| round_out_layer(&mapping.param_to_layer_rect(&content)));
        // Process the layer-space desired output with the filter DAG to determine required input
        self.on_get_input_layer_bounds(mapping, desired_bounds, content_bounds)
    }

    /// `getOutputBounds`: the device bounds of the filter output for content of `content_bounds`;
    /// `None` if the output is unbounded.
    // Port of: src/core/SkImageFilter_Base.cpp (getOutputBounds) (chrome/m156)
    #[doc(alias = "getOutputBounds")]
    fn get_output_bounds(&self, mapping: &Mapping, content_bounds: &Rect) -> Option<IRect> {
        // Map the input content into the layer space where filtering will occur
        let layer_content = mapping.param_to_layer_rect(content_bounds);
        // Determine the filter DAGs output bounds in layer space
        let filter_output =
            self.on_get_output_layer_bounds(mapping, Some(round_out_layer(&layer_content)))?;
        // Map all the way to device space
        Some(mapping.layer_to_device_irect(&filter_output))
    }

    /// `getChildInputLayerBounds`: the input bounds of child `index`, or of the source when the
    /// child is null.
    // Port of: src/core/SkImageFilter_Base.cpp (getChildInputLayerBounds) (chrome/m156)
    #[doc(alias = "getChildInputLayerBounds")]
    fn get_child_input_layer_bounds(
        &self,
        index: usize,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // The required input for childFilter filter, or 'contentBounds' intersected with
        // 'desiredOutput' if the filter is null and the source image is used (i.e. the identity
        // filter)
        if let Some(child) = self.get_input(index) {
            child
                .as_base()
                .on_get_input_layer_bounds(mapping, desired_output, content_bounds)
        } else {
            // NOTE: We don't calculate the intersection between content and root desired output
            // because the desired output can expand or contract as it propagates through the
            // filter graph to the leaves that would actually sample from the source content.
            let mut visible_content = desired_output;
            if content_bounds.is_some_and(|content| {
                !crate::image_filter_types::irect_intersect_in_place(&mut visible_content, &content)
            }) {
                return IRect::new_empty();
            }
            // This will be equal to 'desiredOutput' if the contentBounds are unknown.
            visible_content
        }
    }

    /// `getChildOutputLayerBounds`: the output bounds of child `index`, or `content_bounds` when
    /// the child is null.
    // Port of: src/core/SkImageFilter_Base.cpp (getChildOutputLayerBounds) (chrome/m156)
    #[doc(alias = "getChildOutputLayerBounds")]
    fn get_child_output_layer_bounds(
        &self,
        index: usize,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        match self.get_input(index) {
            Some(child) => child
                .as_base()
                .on_get_output_layer_bounds(mapping, content_bounds),
            None => content_bounds,
        }
    }

    /// `getChildOutput`: the filtered result of input `index`, or the context's source when the
    /// input is null.
    // Port of: src/core/SkImageFilter_Base.cpp (getChildOutput) (chrome/m156)
    #[doc(alias = "getChildOutput")]
    fn get_child_output(&self, index: usize, ctx: &Context<'_>) -> FilterResult {
        match self.get_input(index) {
            Some(input) => input.as_base().filter_image(ctx),
            None => ctx.source().clone(),
        }
    }

    /// `SkImageFilter_Base::filterImage`: the filtered result of the DAG rooted here.
    // Port of: src/core/SkImageFilter.cpp#L244-L271 (chrome/m156), without the cache
    #[doc(alias = "filterImage")]
    fn filter_image(&self, context: &Context<'_>) -> FilterResult {
        context.mark_visited_image_filter();
        if context.desired_output().is_empty() || !context.mapping().layer_matrix().is_finite() {
            return FilterResult::default();
        }
        self.on_filter_image(context)
    }
}

/// `LayerSpace<SkRect>::roundOut()`, which is `skif::RoundOut`.
// Port of: src/core/SkImageFilterTypes.h#L274-L278 (chrome/m156)
#[must_use]
pub fn round_out_layer(r: &Rect) -> IRect {
    round_out(r)
}

/// A shared image filter (`sk_sp<SkImageFilter>`): a cheaply clonable handle to an
/// [`ImageFilterBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`ImageFilter::ptr_eq`]).
// Port of: include/core/SkImageFilter.h#L35-L117 (chrome/m156)
#[doc(alias = "SkImageFilter")]
#[derive(Clone)]
pub struct ImageFilter(Arc<dyn ImageFilterBase>);

impl ImageFilter {
    /// Wraps an image filter implementation.
    #[must_use]
    pub fn from_base(base: impl ImageFilterBase) -> ImageFilter {
        ImageFilter(Arc::new(base))
    }

    /// The implementation (`as_IFB`).
    #[doc(alias = "as_IFB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn ImageFilterBase {
        &*self.0
    }

    /// True if `self` and `other` are the same filter (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &ImageFilter) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// `SkImageFilter::makeWithLocalMatrix(matrix)`: this filter, applied with `matrix` as a local
    /// matrix. `None` if the matrix cannot be inverted.
    // Port of: src/core/SkImageFilter.cpp#L130-L132 (chrome/m156)
    #[doc(alias = "makeWithLocalMatrix")]
    #[must_use]
    pub fn with_local_matrix(&self, matrix: &Matrix) -> Option<ImageFilter> {
        make_local_matrix_image_filter(matrix, Some(self.clone()))
    }

    /// `SkImageFilter_Base::makeImageWithFilter`: filters `src` (its `subset`) with this filter,
    /// with `backend` doing the work, and returns the result, the subset of it that was produced,
    /// and its offset within `clip_bounds`. `None` if the filter produces nothing.
    ///
    /// Reached through `SkImages::MakeWithFilter` (see `skia_rust_raster::image_filter_backend`).
    // Port of: src/core/SkImageFilter.cpp#L265-L300 (chrome/m156)
    #[doc(alias = "makeImageWithFilter")]
    #[must_use]
    pub fn make_image_with_filter(
        &self,
        backend: Arc<dyn Backend>,
        src: &Image,
        subset: IRect,
        clip_bounds: IRect,
    ) -> Option<(Image, IRect, IPoint)> {
        if !src.bounds().contains(subset) {
            return None;
        }

        let src_special_image = backend.make_image(&subset, src).map(Arc::new)?;

        let stats = Stats::default();
        let context = Context::new(
            backend,
            Mapping::from_layer_matrix(&M44::new_identity()),
            clip_bounds,
            FilterResult::new(
                Some(src_special_image),
                IPoint::new(subset.left, subset.top),
            ),
            src.image_info().color_space(),
            Some(&stats),
        );

        let result = self.as_base().filter_image(&context);
        let (special, offset) = result.image_and_offset(&context);
        let special = special?;

        let subset = special.subset();
        let image = special.as_image()?;
        Some((image, subset, offset))
    }

    /// `countInputs`.
    #[must_use]
    pub fn count_inputs(&self) -> usize {
        self.0.count_inputs()
    }

    /// `getInput(i)`.
    #[must_use]
    pub fn get_input(&self, i: usize) -> Option<&ImageFilter> {
        self.0.get_input(i)
    }

    /// The bounds of the filtered result of geometry with bounds `bounds`
    /// (`computeFastBounds`).
    #[doc(alias = "computeFastBounds")]
    #[must_use]
    pub fn compute_fast_bounds(&self, bounds: impl AsRef<Rect>) -> Rect {
        self.0.compute_fast_bounds(bounds.as_ref())
    }

    /// Can this filter DAG compute the resulting bounds of an object-space rectangle
    /// (`canComputeFastBounds`)?
    // Port of: src/core/SkImageFilter.cpp#L98-L100 (chrome/m156)
    #[doc(alias = "canComputeFastBounds")]
    #[must_use]
    pub fn can_compute_fast_bounds(&self) -> bool {
        !self.0.affects_transparent_black()
    }

    /// `affectsTransparentBlack()`.
    #[must_use]
    pub fn affects_transparent_black(&self) -> bool {
        self.0.affects_transparent_black()
    }

    /// `usesSource()`.
    #[must_use]
    pub fn uses_source(&self) -> bool {
        self.0.uses_source()
    }

    /// `uniqueID()`.
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.0.unique_id()
    }

    /// `getCTMCapability()`.
    #[must_use]
    pub fn ctm_capability(&self) -> MatrixCapability {
        self.0.get_ctm_capability()
    }

    /// `asAColorFilter`'s `isColorFilterNode`: the color filter of a node that is just a color
    /// filter, if it has no inputs and does not affect transparent black.
    // Port of: src/core/SkImageFilter.cpp#L118-L128 (chrome/m156)
    #[must_use]
    pub fn as_a_color_filter(&self) -> Option<ColorFilter> {
        let filter = self.0.on_is_color_filter_node()?;
        let has_input0 = self.0.count_inputs() > 0 && self.0.get_input(0).is_some();
        if has_input0 || filter.as_base().affects_transparent_black() {
            return None;
        }
        Some(filter)
    }

    /// `SkImageFilter::filterBounds`: maps a rectangle through the filter in either direction.
    /// `input_rect` is the known content for `MapDirection::Reverse`.
    // Port of: src/core/SkImageFilter.cpp#L61-L80 (chrome/m156)
    #[doc(alias = "filterBounds")]
    #[must_use]
    pub fn filter_bounds(
        &self,
        src: &IRect,
        ctm: &Matrix,
        direction: MapDirection,
        input_rect: Option<&IRect>,
    ) -> IRect {
        // The old filterBounds() function uses SkIRects that are defined in layer space so, while
        // we still are supporting it, bypass SkIF_B's new public filter bounds functions and go
        // right to the internal layer-space calculations.
        let mapping = Mapping::from_layer_matrix(&M44::from(ctm));
        if direction == MapDirection::Reverse {
            let content = input_rect.copied();
            self.0.on_get_input_layer_bounds(&mapping, *src, content)
        } else {
            debug_assert!(input_rect.is_none());
            match self.0.on_get_output_layer_bounds(&mapping, Some(*src)) {
                Some(output) => output,
                None => rect_priv::make_i_large(),
            }
        }
    }
}

impl PartialEq for ImageFilter {
    /// Identity, as Skia's `sk_sp<SkImageFilter>` `operator==`.
    fn eq(&self, other: &ImageFilter) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for ImageFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ImageFilter").field(&self.0).finish()
    }
}

impl Mapping {
    /// `Mapping::decomposeCTM(ctm, filter, pt)`: decomposes with the filter's CTM capability
    /// (`kComplex` for no filter).
    // Port of: src/core/SkImageFilterTypes.cpp#L300-L307 (chrome/m156)
    #[doc(alias = "decomposeCTM")]
    #[must_use]
    pub fn decompose_ctm_for_filter(
        &mut self,
        ctm: &M44,
        filter: Option<&ImageFilter>,
        representative_pt: Point,
    ) -> bool {
        let capability = filter.map_or(MatrixCapability::Complex, ImageFilter::ctm_capability);
        self.decompose_ctm(ctm, capability, representative_pt)
    }
}
