// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkClipStack.{h,cpp}

//! `SkClipStack`: the stack of clip elements a canvas keeps, with the bounds and generation ids
//! derived from them.
//!
//! Because a single save/restore state can have multiple clips, [`ClipStack`] stores the stack
//! depth (`fSaveCount`) and the clips separately. Each clip stores the stack state to which it
//! belongs (the save count in force when it was added). Restores are thus implemented by removing
//! clips that have a save count larger than the freshly decremented count.
//!
//! skia-rust deviations:
//! - Skia's `SkDeque` (blocks of `Element`s, with a placement-new constructor over client
//!   storage) is a `Vec<Element>`; the clip stack only ever pushes and pops at the back. The
//!   `SkClipStack(void* storage, size_t size)` constructor is dropped. [`Iter`] reproduces
//!   `SkDeque::Iter`'s cursor semantics (`next`/`prev` return the element at the cursor, then
//!   move it).
//! - `Element::dump`/`SkClipStack::dump` (`SK_DEBUG` only, printing with `SkDebugf`) are not
//!   ported.
//! - `getBounds`/`getConservativeBounds` return the optional `isIntersectionOfRects` out
//!   parameter as part of the result.
//! - `SkShaders::Blend` of two clip shaders makes a [`BlendShader`](crate::shaders::BlendShader)
//!   whose stages are not ported yet (see there).

use std::sync::atomic::{AtomicU32, Ordering};

use crate::blend_mode::BlendMode;
use crate::clip_op::ClipOp;
use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::rect::{Contains, IRect, Rect, rect_priv};
use crate::rrect::{RRect, Type as RRectType};
use crate::scalar::int_to_scalar;
use crate::shader::Shader;
use crate::shaders;

/// What a bound means (`SkClipStack::BoundsType`).
// Port of: src/core/SkClipStack.h#L35-L47 (chrome/m156)
#[doc(alias = "SkClipStack::BoundsType")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BoundsType {
    /// The bounding box contains all the pixels that can be written to.
    #[doc(alias = "kNormal_BoundsType")]
    Normal,
    /// The bounding box contains all the pixels that cannot be written to. The real bound extends
    /// out to infinity and all the pixels outside of the bound can be written to. Note that some
    /// of the pixels inside the bound may also be writeable but all pixels that cannot be
    /// written to are guaranteed to be inside.
    #[doc(alias = "kInsideOut_BoundsType")]
    InsideOut,
}

/// The shape type of a clip element in device space (`SkClipStack::Element::DeviceSpaceType`).
// Port of: src/core/SkClipStack.h#L60-L74 (chrome/m156)
#[doc(alias = "SkClipStack::Element::DeviceSpaceType")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeviceSpaceType {
    /// This element makes the clip empty (regardless of previous elements).
    #[doc(alias = "kEmpty")]
    Empty,
    /// This element combines a device space rect with the current clip.
    #[doc(alias = "kRect")]
    Rect,
    /// This element combines a device space round-rect with the current clip.
    #[doc(alias = "kRRect")]
    RRect,
    /// This element combines a device space path with the current clip.
    #[doc(alias = "kPath")]
    Path,
    /// This element does not have geometry, but applies a shader to the clip.
    #[doc(alias = "kShader")]
    Shader,
}

impl DeviceSpaceType {
    /// `kLastType`.
    #[doc(alias = "kLastType")]
    pub const LAST_TYPE: DeviceSpaceType = DeviceSpaceType::Shader;
    /// `kTypeCnt`.
    #[doc(alias = "kTypeCnt")]
    pub const TYPE_COUNT: usize = DeviceSpaceType::LAST_TYPE as usize + 1;
}

/// The invalid generation id, never returned by a [`ClipStack`] (`kInvalidGenID`). Useful when
/// caching clips based on the generation id.
// Port of: src/core/SkClipStack.h#L296-L299 (chrome/m156)
#[doc(alias = "kInvalidGenID")]
pub const INVALID_GEN_ID: u32 = 0;
/// The generation id of a clip that writes no pixels (`kEmptyGenID`).
// Port of: src/core/SkClipStack.h#L300 (chrome/m156)
#[doc(alias = "kEmptyGenID")]
pub const EMPTY_GEN_ID: u32 = 1;
/// The generation id of a clip that writes all pixels (`kWideOpenGenID`).
// Port of: src/core/SkClipStack.h#L301 (chrome/m156)
#[doc(alias = "kWideOpenGenID")]
pub const WIDE_OPEN_GEN_ID: u32 = 2;

/// The different combinations of fill and inverse fill when combining bounding boxes
/// (`SkClipStack::Element::FillCombo`); the discriminants are the bit combinations Skia builds.
// Port of: src/core/SkClipStack.h#L205-L210 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // mirrors Skia's kPrev_Cur_FillCombo etc.
enum FillCombo {
    PrevCur = 0,
    PrevInvCur = 1,
    InvPrevCur = 2,
    InvPrevInvCur = 3,
}

impl FillCombo {
    // `(FillCombo)(combination | 0x01 | 0x02)`
    fn from_bits(bits: u32) -> FillCombo {
        match bits & 3 {
            0 => FillCombo::PrevCur,
            1 => FillCombo::PrevInvCur,
            2 => FillCombo::InvPrevCur,
            _ => FillCombo::InvPrevInvCur,
        }
    }
}

/// An element of the clip stack (`SkClipStack::Element`). It represents a shape combined with the
/// previous clip using a set operator. Each element can be antialiased or not.
// Port of: src/core/SkClipStack.h#L54-L257 (chrome/m156)
#[doc(alias = "SkClipStack::Element")]
#[derive(Clone, Debug)]
pub struct Element {
    /// `fDeviceSpacePath`.
    device_space_path: Option<Path>,
    /// `fDeviceSpaceRRect`.
    device_space_rrect: RRect,
    /// `fShader`.
    shader: Option<Shader>,
    /// `fSaveCount`: save count of the stack when this element was added.
    save_count: i32,
    /// `fOp`.
    op: ClipOp,
    /// `fDeviceSpaceType`.
    device_space_type: DeviceSpaceType,
    /// `fDoAA`.
    do_aa: bool,
    /// `fIsReplace`.
    is_replace: bool,
    /// `fFiniteBoundType` and `fFiniteBound` are used to incrementally update the clip stack's
    /// bound. When the type is `Normal`, `fFiniteBound` represents the conservative bounding box
    /// of the pixels that aren't clipped (i.e., any pixels that can be drawn to are inside the
    /// bound). When it is `InsideOut` (which occurs when a clip is inverse filled),
    /// `fFiniteBound` represents the conservative bounding box of the pixels that _are_ clipped
    /// (i.e., any pixels that cannot be drawn to are inside the bound) and the actual bound is
    /// the infinite plane. This is required so that we can capture the cancelling out of the
    /// extensions to infinity when two inverse filled clips are Booleaned together.
    finite_bound_type: BoundsType,
    /// `fFiniteBound`.
    finite_bound: Rect,
    /// `fIsIntersectionOfRects`: when this element is applied to the previous elements in the
    /// stack, is the result known to be equivalent to a single rect intersection? IOW, is the
    /// clip effectively a rectangle.
    is_intersection_of_rects: bool,
    /// `fGenID`.
    gen_id: u32,
}

impl Default for Element {
    /// An empty element (`Element()`).
    // Port of: src/core/SkClipStack.h#L76-L79 (chrome/m156)
    fn default() -> Element {
        Element::with_save_count_empty(0)
    }
}

impl PartialEq for Element {
    // Port of: src/core/SkClipStack.cpp#L57-L81 (chrome/m156)
    fn eq(&self, element: &Element) -> bool {
        if std::ptr::eq(self, element) {
            return true;
        }
        if self.op != element.op
            || self.device_space_type != element.device_space_type
            || self.do_aa != element.do_aa
            || self.is_replace != element.is_replace
            || self.save_count != element.save_count
        {
            return false;
        }
        match self.device_space_type {
            DeviceSpaceType::Shader => self.shader == element.shader,
            DeviceSpaceType::Path => self.device_space_path() == element.device_space_path(),
            DeviceSpaceType::RRect => self.device_space_rrect == element.device_space_rrect,
            DeviceSpaceType::Rect => self.device_space_rect() == element.device_space_rect(),
            DeviceSpaceType::Empty => true,
        }
    }
}

impl Element {
    /// An element with every field at the value `initCommon` and `setEmpty` would leave before
    /// the shape specific initializer runs.
    fn blank() -> Element {
        Element {
            device_space_path: None,
            device_space_rrect: RRect::new(),
            shader: None,
            save_count: 0,
            op: ClipOp::Intersect,
            device_space_type: DeviceSpaceType::Empty,
            do_aa: false,
            is_replace: false,
            finite_bound_type: BoundsType::InsideOut,
            finite_bound: Rect::new_empty(),
            is_intersection_of_rects: false,
            gen_id: INVALID_GEN_ID,
        }
    }

    /// `Element(int saveCount)`.
    // Port of: src/core/SkClipStack.h#L226-L229 (chrome/m156)
    fn with_save_count_empty(save_count: i32) -> Element {
        let mut e = Element::blank();
        e.init_common(save_count, ClipOp::Intersect, false);
        e.set_empty();
        e
    }

    /// `Element(int saveCount, const SkRect&, const SkMatrix&, SkClipOp, bool)`.
    // Port of: src/core/SkClipStack.h#L235-L237 (chrome/m156)
    fn with_save_count_rect(
        save_count: i32,
        rect: &Rect,
        m: &Matrix,
        op: ClipOp,
        do_aa: bool,
    ) -> Element {
        let mut e = Element::blank();
        e.init_rect(save_count, rect, m, op, do_aa);
        e
    }

    /// `Element(int saveCount, const SkRRect&, const SkMatrix&, SkClipOp, bool)`.
    // Port of: src/core/SkClipStack.h#L231-L233 (chrome/m156)
    fn with_save_count_rrect(
        save_count: i32,
        rrect: &RRect,
        m: &Matrix,
        op: ClipOp,
        do_aa: bool,
    ) -> Element {
        let mut e = Element::blank();
        e.init_rrect(save_count, rrect, m, op, do_aa);
        e
    }

    /// `Element(int saveCount, const SkPath&, const SkMatrix&, SkClipOp, bool)`.
    // Port of: src/core/SkClipStack.h#L239-L241 (chrome/m156)
    fn with_save_count_path(
        save_count: i32,
        path: &Path,
        m: &Matrix,
        op: ClipOp,
        do_aa: bool,
    ) -> Element {
        let mut e = Element::blank();
        e.init_path(save_count, path, m, op, do_aa);
        e
    }

    /// `Element(int saveCount, sk_sp<SkShader>)`.
    // Port of: src/core/SkClipStack.h#L243-L245 (chrome/m156)
    fn with_save_count_shader(save_count: i32, shader: Shader) -> Element {
        let mut e = Element::blank();
        e.init_shader(save_count, shader);
        e
    }

    /// `Element(int saveCount, const SkRect&, bool)`.
    // Port of: src/core/SkClipStack.h#L247-L249 (chrome/m156)
    fn with_save_count_replace_rect(save_count: i32, rect: &Rect, do_aa: bool) -> Element {
        let mut e = Element::blank();
        e.init_replace_rect(save_count, rect, do_aa);
        e
    }

    /// An element combining `rect` mapped by `m` with `op` (`Element(const SkRect&, const
    /// SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkClipStack.h#L84-L86 (chrome/m156)
    #[must_use]
    pub fn new_rect(rect: &Rect, m: &Matrix, op: ClipOp, do_aa: bool) -> Element {
        Element::with_save_count_rect(0, rect, m, op, do_aa)
    }

    /// An element combining `rrect` mapped by `m` with `op` (`Element(const SkRRect&, const
    /// SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkClipStack.h#L88-L90 (chrome/m156)
    #[must_use]
    pub fn new_rrect(rrect: &RRect, m: &Matrix, op: ClipOp, do_aa: bool) -> Element {
        Element::with_save_count_rrect(0, rrect, m, op, do_aa)
    }

    /// An element combining `path` mapped by `m` with `op` (`Element(const SkPath&, const
    /// SkMatrix&, SkClipOp, bool)`).
    // Port of: src/core/SkClipStack.h#L92-L94 (chrome/m156)
    #[must_use]
    pub fn new_path(path: &Path, m: &Matrix, op: ClipOp, do_aa: bool) -> Element {
        Element::with_save_count_path(0, path, m, op, do_aa)
    }

    /// An element that applies `shader` to the clip (`Element(sk_sp<SkShader>)`).
    // Port of: src/core/SkClipStack.h#L96 (chrome/m156)
    #[must_use]
    pub fn new_shader(shader: Shader) -> Element {
        Element::with_save_count_shader(0, shader)
    }

    /// An element that replaces the clip with `rect` (`Element(const SkRect&, bool)`).
    // Port of: src/core/SkClipStack.h#L98-L100 (chrome/m156)
    #[must_use]
    pub fn new_replace_rect(rect: &Rect, do_aa: bool) -> Element {
        Element::with_save_count_replace_rect(0, rect, do_aa)
    }

    /// The shape type of the clip element in device space (`getDeviceSpaceType`).
    #[doc(alias = "getDeviceSpaceType")]
    #[must_use]
    pub fn device_space_type(&self) -> DeviceSpaceType {
        self.device_space_type
    }

    /// The save count associated with this clip element (`getSaveCount`).
    #[doc(alias = "getSaveCount")]
    #[must_use]
    pub fn save_count(&self) -> i32 {
        self.save_count
    }

    /// The path, if [`device_space_type`](Self::device_space_type) is [`DeviceSpaceType::Path`]
    /// (`getDeviceSpacePath`).
    ///
    /// # Panics
    /// If the element is not a path.
    // Port of: src/core/SkClipStack.h#L116-L119 (chrome/m156)
    #[doc(alias = "getDeviceSpacePath")]
    #[must_use]
    pub fn device_space_path(&self) -> &Path {
        debug_assert_eq!(DeviceSpaceType::Path, self.device_space_type);
        self.device_space_path
            .as_ref()
            .expect("a path element has a path")
    }

    /// The round-rect, if [`device_space_type`](Self::device_space_type) is
    /// [`DeviceSpaceType::RRect`] (`getDeviceSpaceRRect`).
    // Port of: src/core/SkClipStack.h#L122-L125 (chrome/m156)
    #[doc(alias = "getDeviceSpaceRRect")]
    #[must_use]
    pub fn device_space_rrect(&self) -> &RRect {
        debug_assert_eq!(DeviceSpaceType::RRect, self.device_space_type);
        &self.device_space_rrect
    }

    /// The rect, if [`device_space_type`](Self::device_space_type) is [`DeviceSpaceType::Rect`]
    /// (`getDeviceSpaceRect`).
    // Port of: src/core/SkClipStack.h#L128-L132 (chrome/m156)
    #[doc(alias = "getDeviceSpaceRect")]
    #[must_use]
    pub fn device_space_rect(&self) -> &Rect {
        debug_assert!(
            DeviceSpaceType::Rect == self.device_space_type
                && (self.device_space_rrect.is_rect() || self.device_space_rrect.is_empty())
        );
        self.device_space_rrect.bounds()
    }

    /// The clip shader, if [`device_space_type`](Self::device_space_type) is
    /// [`DeviceSpaceType::Shader`] (`refShader`/`getShader`).
    // Port of: src/core/SkClipStack.h#L135-L140 (chrome/m156)
    #[doc(alias = "refShader")]
    #[doc(alias = "getShader")]
    #[must_use]
    pub fn shader(&self) -> Option<&Shader> {
        self.shader.as_ref()
    }

    /// The set operation used to combine this element, if the type is not
    /// [`DeviceSpaceType::Empty`] (`getOp`).
    // Port of: src/core/SkClipStack.h#L144 (chrome/m156)
    #[doc(alias = "getOp")]
    #[must_use]
    pub fn op(&self) -> ClipOp {
        self.op
    }

    /// Augments [`op`](Self::op)'s behavior by requiring a clip reset before the op is applied
    /// (`isReplaceOp`).
    // Port of: src/core/SkClipStack.h#L146 (chrome/m156)
    #[doc(alias = "isReplaceOp")]
    #[must_use]
    pub fn is_replace_op(&self) -> bool {
        self.is_replace
    }

    /// The element as a path, regardless of its type (`asDeviceSpacePath`).
    // Port of: src/core/SkClipStack.cpp#L226-L244 (chrome/m156)
    #[doc(alias = "asDeviceSpacePath")]
    #[must_use]
    pub fn as_device_space_path(&self) -> Path {
        let mut builder = PathBuilder::new();
        match self.device_space_type {
            DeviceSpaceType::Empty => {}
            DeviceSpaceType::Rect => {
                builder.add_rect(*self.device_space_rect(), None, None);
            }
            DeviceSpaceType::RRect => {
                builder.add_rrect(self.device_space_rrect, None, None);
            }
            DeviceSpaceType::Path => return self.device_space_path().clone(),
            DeviceSpaceType::Shader => {
                builder.add_rect(rect_priv::make_large_s32(), None, None);
            }
        }
        builder.detach()
    }

    /// The element as a round rect, if the type is not [`DeviceSpaceType::Path`]
    /// (`asDeviceSpaceRRect`).
    // Port of: src/core/SkClipStack.h#L151-L154 (chrome/m156)
    #[doc(alias = "asDeviceSpaceRRect")]
    #[must_use]
    pub fn as_device_space_rrect(&self) -> &RRect {
        debug_assert_ne!(DeviceSpaceType::Path, self.device_space_type);
        &self.device_space_rrect
    }

    /// If the type is not [`DeviceSpaceType::Empty`] this indicates whether the clip shape should
    /// be anti-aliased when it is rasterized (`isAA`).
    #[doc(alias = "isAA")]
    #[must_use]
    pub fn is_aa(&self) -> bool {
        self.do_aa
    }

    /// The generation id, which clip stack clients can use to cache representations of the clip.
    /// The id corresponds to the set of clip elements up to and including this element within the
    /// stack, not to the element itself. That is, the same clip path in different stacks will
    /// have a different id since the elements produce different clip results in the context of
    /// their stacks (`getGenID`).
    // Port of: src/core/SkClipStack.h#L168-L171 (chrome/m156)
    #[doc(alias = "getGenID")]
    #[must_use]
    pub fn gen_id(&self) -> u32 {
        debug_assert!(INVALID_GEN_ID != self.gen_id);
        self.gen_id
    }

    /// The bounds of the clip element, either the rect or path bounds. (Whether the shape is
    /// inverse filled is not considered.)
    // Port of: src/core/SkClipStack.cpp#L83-L103 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match self.device_space_type {
            DeviceSpaceType::Rect | DeviceSpaceType::RRect => *self.device_space_rrect.bounds(),
            DeviceSpaceType::Path => *self.device_space_path().bounds(),
            // Shaders have infinite bounds since any pixel could have clipped or full coverage
            // (which is different from wide-open, where every pixel has 1.0 coverage, or empty
            // where every pixel has 0.0 coverage).
            DeviceSpaceType::Shader => rect_priv::make_large_s32(),
            DeviceSpaceType::Empty => Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Conservatively checks whether the clip shape contains the rect. (Whether the shape is
    /// inverse filled is not considered.)
    // Port of: src/core/SkClipStack.cpp#L105-L120 (chrome/m156)
    #[must_use]
    pub fn contains_rect(&self, rect: &Rect) -> bool {
        match self.device_space_type {
            DeviceSpaceType::Rect => self.device_space_rect().contains(rect),
            DeviceSpaceType::RRect => self.device_space_rrect.contains(rect),
            DeviceSpaceType::Path => self.device_space_path().conservatively_contains_rect(rect),
            DeviceSpaceType::Empty | DeviceSpaceType::Shader => false,
        }
    }

    /// Conservatively checks whether the clip shape contains the rrect. (Whether the shape is
    /// inverse filled is not considered.)
    // Port of: src/core/SkClipStack.cpp#L122-L138 (chrome/m156)
    #[must_use]
    pub fn contains_rrect(&self, rrect: &RRect) -> bool {
        match self.device_space_type {
            DeviceSpaceType::Rect => self.device_space_rect().contains(rrect.bounds()),
            DeviceSpaceType::RRect => {
                // We don't currently have a generalized rrect-rrect containment.
                self.device_space_rrect.contains(rrect.bounds())
                    || *rrect == self.device_space_rrect
            }
            DeviceSpaceType::Path => self
                .device_space_path()
                .conservatively_contains_rect(rrect.bounds()),
            DeviceSpaceType::Empty | DeviceSpaceType::Shader => false,
        }
    }

    /// Is the clip shape inverse filled (`isInverseFilled`).
    // Port of: src/core/SkClipStack.h#L190-L193 (chrome/m156)
    #[doc(alias = "isInverseFilled")]
    #[must_use]
    pub fn is_inverse_filled(&self) -> bool {
        DeviceSpaceType::Path == self.device_space_type
            && self.device_space_path().is_inverse_fill_type()
    }

    // Port of: src/core/SkClipStack.cpp#L140-L151 (chrome/m156)
    fn init_common(&mut self, save_count: i32, op: ClipOp, do_aa: bool) {
        self.save_count = save_count;
        self.op = op;
        self.do_aa = do_aa;
        self.is_replace = false;
        // A default of inside-out and empty bounds means the bounds are effectively void as it
        // indicates that nothing is known to be outside the clip.
        self.finite_bound_type = BoundsType::InsideOut;
        self.finite_bound.set_empty();
        self.is_intersection_of_rects = false;
        self.gen_id = INVALID_GEN_ID;
    }

    // Port of: src/core/SkClipStack.cpp#L153-L164 (chrome/m156)
    fn init_rect(&mut self, save_count: i32, rect: &Rect, m: &Matrix, op: ClipOp, do_aa: bool) {
        if m.rect_stays_rect() {
            let (dev_rect, _) = m.map_rect(rect);
            self.device_space_rrect.set_rect(dev_rect);
            self.device_space_type = DeviceSpaceType::Rect;
            self.init_common(save_count, op, do_aa);
            return;
        }
        self.init_as_path(save_count, &Path::rect(rect, None), m, op, do_aa);
    }

    // Port of: src/core/SkClipStack.cpp#L166-L180 (chrome/m156)
    fn init_rrect(&mut self, save_count: i32, rrect: &RRect, m: &Matrix, op: ClipOp, do_aa: bool) {
        if let Some(rr) = rrect.transform(m) {
            self.device_space_rrect = rr;
            let type_ = self.device_space_rrect.get_type();
            if RRectType::Rect == type_ || RRectType::Empty == type_ {
                self.device_space_type = DeviceSpaceType::Rect;
            } else {
                self.device_space_type = DeviceSpaceType::RRect;
            }
            self.init_common(save_count, op, do_aa);
            return;
        }
        self.init_as_path(save_count, &Path::rrect(rrect, None), m, op, do_aa);
    }

    // Port of: src/core/SkClipStack.cpp#L182-L199 (chrome/m156)
    fn init_path(&mut self, save_count: i32, path: &Path, m: &Matrix, op: ClipOp, do_aa: bool) {
        if !path.is_inverse_fill_type() {
            if let Some((r, _, _)) = path.is_rect() {
                self.init_rect(save_count, &r, m, op, do_aa);
                return;
            }
            if let Some(oval_rect) = path.is_oval() {
                let mut rrect = RRect::new();
                rrect.set_oval(oval_rect);
                self.init_rrect(save_count, &rrect, m, op, do_aa);
                return;
            }
        }
        self.init_as_path(save_count, path, m, op, do_aa);
    }

    // Port of: src/core/SkClipStack.cpp#L201-L210 (chrome/m156)
    fn init_as_path(&mut self, save_count: i32, path: &Path, m: &Matrix, op: ClipOp, do_aa: bool) {
        let mut builder = PathBuilder::new_path(path);
        builder.transform(m);
        builder.set_is_volatile(true);
        self.device_space_path = Some(builder.detach());

        self.device_space_type = DeviceSpaceType::Path;
        self.init_common(save_count, op, do_aa);
    }

    // Port of: src/core/SkClipStack.cpp#L212-L217 (chrome/m156)
    fn init_shader(&mut self, save_count: i32, shader: Shader) {
        self.device_space_type = DeviceSpaceType::Shader;
        self.shader = Some(shader);
        self.init_common(save_count, ClipOp::Intersect, false);
    }

    // Port of: src/core/SkClipStack.cpp#L219-L224 (chrome/m156)
    fn init_replace_rect(&mut self, save_count: i32, rect: &Rect, do_aa: bool) {
        self.device_space_rrect.set_rect(rect);
        self.device_space_type = DeviceSpaceType::Rect;
        self.init_common(save_count, ClipOp::Intersect, do_aa);
        self.is_replace = true;
    }

    // Port of: src/core/SkClipStack.cpp#L246-L256 (chrome/m156)
    fn set_empty(&mut self) {
        self.device_space_type = DeviceSpaceType::Empty;
        self.finite_bound.set_empty();
        self.finite_bound_type = BoundsType::Normal;
        self.is_intersection_of_rects = false;
        self.device_space_rrect.set_empty();
        self.device_space_path = None;
        self.shader = None;
        self.gen_id = EMPTY_GEN_ID;
        self.check_empty();
    }

    // Port of: src/core/SkClipStack.cpp#L258-L266 (chrome/m156)
    fn check_empty(&self) {
        debug_assert!(self.finite_bound.is_empty());
        debug_assert_eq!(BoundsType::Normal, self.finite_bound_type);
        debug_assert!(!self.is_intersection_of_rects);
        debug_assert_eq!(EMPTY_GEN_ID, self.gen_id);
        debug_assert!(self.device_space_rrect.is_empty());
        debug_assert!(self.device_space_path.is_none());
        debug_assert!(self.shader.is_none());
    }

    // Port of: src/core/SkClipStack.cpp#L268-L278 (chrome/m156)
    fn can_be_intersected_in_place(&self, save_count: i32, op: ClipOp) -> bool {
        if DeviceSpaceType::Empty == self.device_space_type
            && (ClipOp::Difference == op || ClipOp::Intersect == op)
        {
            return true;
        }
        // Only clips within the same save/restore frame (as captured by the save count) can be
        // merged
        self.save_count == save_count
            && ClipOp::Intersect == op
            && (ClipOp::Intersect == self.op || self.is_replace_op())
    }

    /// This method checks to see if two rect clips can be safely merged into one. The issue here
    /// is that to be strictly correct all the edges of the resulting rect must have the same
    /// anti-aliasing.
    // Port of: src/core/SkClipStack.cpp#L280-L305 (chrome/m156)
    fn rect_rect_intersect_allowed(&self, new_r: &Rect, new_aa: bool) -> bool {
        debug_assert_eq!(DeviceSpaceType::Rect, self.device_space_type);

        if self.do_aa == new_aa {
            // if the AA setting is the same there is no issue
            return true;
        }

        if !Rect::intersects2(self.device_space_rect(), new_r) {
            // The calling code will correctly set the result to the empty clip
            return true;
        }

        if self.device_space_rect().contains(new_r) {
            // if the new rect carves out a portion of the old one there is no issue
            return true;
        }

        // So either the two overlap in some complex manner or newR contains oldR. In the first
        // case the edges will require different AA. In the second, the AA setting that would be
        // carried forward is incorrect (e.g., oldR is AA while newR is BW but since newR
        // contains oldR, oldR will be drawn BW) since the new AA setting will predominate.
        false
    }

    // a mirror of combineBoundsRevDiff
    // Port of: src/core/SkClipStack.cpp#L307-L348 (chrome/m156)
    fn combine_bounds_diff(&mut self, combination: FillCombo, prev_finite: &Rect) {
        match combination {
            FillCombo::InvPrevInvCur => {
                // In this case the only pixels that can remain set are inside the current clip
                // rect since the extensions to infinity of both clips cancel out and whatever is
                // outside of the current clip is removed
                self.finite_bound_type = BoundsType::Normal;
            }
            FillCombo::InvPrevCur => {
                // In this case the current op is finite so the only pixels that aren't set are
                // whatever isn't set in the previous clip and whatever this clip carves out
                self.finite_bound.join(prev_finite);
                self.finite_bound_type = BoundsType::InsideOut;
            }
            FillCombo::PrevInvCur => {
                // In this case everything outside of this clip's bound is erased, so the only
                // pixels that can remain set occur w/in the intersection of the two finite
                // bounds
                if !self.finite_bound.intersect(prev_finite) {
                    self.finite_bound.set_empty();
                    self.gen_id = EMPTY_GEN_ID;
                }
                self.finite_bound_type = BoundsType::Normal;
            }
            FillCombo::PrevCur => {
                // The most conservative result bound is that of the prior clip. This could be
                // wildly incorrect if the second clip either exactly matches the first clip
                // (which should yield the empty set) or reduces the size of the prior bound
                // (e.g., if the second clip exactly matched the bottom half of the prior clip).
                // We ignore these two possibilities.
                self.finite_bound = *prev_finite;
            }
        }
    }

    // a mirror of combineBoundsUnion
    // Port of: src/core/SkClipStack.cpp#L350-L379 (chrome/m156)
    fn combine_bounds_intersection(&mut self, combination: FillCombo, prev_finite: &Rect) {
        match combination {
            FillCombo::InvPrevInvCur => {
                // The only pixels that aren't writable in this case occur in the union of the
                // two finite bounds
                self.finite_bound.join(prev_finite);
                self.finite_bound_type = BoundsType::InsideOut;
            }
            FillCombo::InvPrevCur => {
                // In this case the only pixels that will remain writeable are within the current
                // clip
            }
            FillCombo::PrevInvCur => {
                // In this case the only pixels that will remain writeable are with the previous
                // clip
                self.finite_bound = *prev_finite;
                self.finite_bound_type = BoundsType::Normal;
            }
            FillCombo::PrevCur => {
                if !self.finite_bound.intersect(prev_finite) {
                    self.set_empty();
                }
            }
        }
    }

    /// Determines possible finite bounds for the element given the previous element of the stack.
    // Port of: src/core/SkClipStack.cpp#L381-L469 (chrome/m156)
    fn update_bound_and_gen_id(&mut self, prior: Option<&Element>) {
        // We set this first here but we may overwrite it later if we determine that the clip is
        // either wide-open or empty.
        self.gen_id = get_next_gen_id();

        // First, optimistically update the current Element's bound information with the current
        // clip's bound
        self.is_intersection_of_rects = false;
        match self.device_space_type {
            DeviceSpaceType::Rect => {
                self.finite_bound = *self.device_space_rect();
                self.finite_bound_type = BoundsType::Normal;

                if self.is_replace_op()
                    || (ClipOp::Intersect == self.op && prior.is_none())
                    || prior.is_some_and(|prior| {
                        ClipOp::Intersect == self.op
                            && prior.is_intersection_of_rects
                            && prior
                                .rect_rect_intersect_allowed(self.device_space_rect(), self.do_aa)
                    })
                {
                    self.is_intersection_of_rects = true;
                }
            }
            DeviceSpaceType::RRect => {
                self.finite_bound = *self.device_space_rrect.bounds();
                self.finite_bound_type = BoundsType::Normal;
            }
            DeviceSpaceType::Path => {
                self.finite_bound = *self.device_space_path().bounds();

                if self.device_space_path().is_inverse_fill_type() {
                    self.finite_bound_type = BoundsType::InsideOut;
                } else {
                    self.finite_bound_type = BoundsType::Normal;
                }
            }
            DeviceSpaceType::Shader => {
                // A shader is infinite. We don't act as wide-open here (which is an empty bounds
                // with the inside out type). This is because when the bounds is empty and
                // inside-out, we know there's full coverage everywhere. With a shader, there's
                // *unknown* coverage everywhere.
                self.finite_bound = rect_priv::make_large_s32();
                self.finite_bound_type = BoundsType::Normal;
            }
            DeviceSpaceType::Empty => {
                debug_assert!(false, "We shouldn't get here with an empty element.");
            }
        }

        // Now determine the previous Element's bound information taking into account that there
        // may be no previous clip
        let (prev_finite, prev_type) = match prior {
            // no prior clip means the entire plane is writable
            // (there are no pixels that cannot be drawn to)
            None => (Rect::new_empty(), BoundsType::InsideOut),
            Some(prior) => (prior.finite_bound, prior.finite_bound_type),
        };

        let mut bits = FillCombo::PrevCur as u32;
        if BoundsType::InsideOut == self.finite_bound_type {
            bits |= 0x01;
        }
        if BoundsType::InsideOut == prev_type {
            bits |= 0x02;
        }
        let combination = FillCombo::from_bits(bits);

        // Now integrate with clip with the prior clips
        if !self.is_replace_op() {
            match self.op {
                ClipOp::Difference => self.combine_bounds_diff(combination, &prev_finite),
                ClipOp::Intersect => self.combine_bounds_intersection(combination, &prev_finite),
            }
        } // else Replace just ignores everything prior and should already have filled in bounds.
    }
}

/// Return the next unique generation id.
// Port of: src/core/SkClipStack.cpp#L896-L906 (chrome/m156)
fn get_next_gen_id() -> u32 {
    // 0-2 are reserved for invalid, empty & wide-open
    const FIRST_UNRESERVED_GEN_ID: u32 = 3;
    static NEXT_ID: AtomicU32 = AtomicU32::new(FIRST_UNRESERVED_GEN_ID);

    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id >= FIRST_UNRESERVED_GEN_ID {
            return id;
        }
    }
}

/// The stack of clip elements (`SkClipStack`).
// Port of: src/core/SkClipStack.h#L32-L494 (chrome/m156)
#[doc(alias = "SkClipStack")]
#[derive(Clone, Debug, Default)]
pub struct ClipStack {
    /// `fDeque`, bottom of the stack first.
    deque: Vec<Element>,
    /// `fSaveCount`.
    save_count: i32,
}

impl PartialEq for ClipStack {
    // Port of: src/core/SkClipStack.cpp#L513-L534 (chrome/m156)
    fn eq(&self, b: &ClipStack) -> bool {
        if self.topmost_gen_id() == b.topmost_gen_id() {
            return true;
        }
        if self.save_count != b.save_count || self.deque.len() != b.deque.len() {
            return false;
        }
        self.deque.iter().zip(b.deque.iter()).all(|(a, b)| a == b)
    }
}

impl ClipStack {
    /// An empty (wide open) stack with a save count of 0.
    // Port of: src/core/SkClipStack.cpp#L477-L480 (chrome/m156)
    #[must_use]
    pub fn new() -> ClipStack {
        ClipStack {
            deque: Vec::new(),
            save_count: 0,
        }
    }

    /// Removes every element and resets the save count to 0.
    // Port of: src/core/SkClipStack.cpp#L536-L546 (chrome/m156)
    pub fn reset(&mut self) {
        self.deque.clear();
        self.save_count = 0;
    }

    /// The current save count (`getSaveCount`).
    #[doc(alias = "getSaveCount")]
    #[must_use]
    pub fn save_count(&self) -> i32 {
        self.save_count
    }

    /// Saves the clip state.
    // Port of: src/core/SkClipStack.cpp#L548-L550 (chrome/m156)
    pub fn save(&mut self) {
        self.save_count += 1;
    }

    /// Restores the clip state, removing the elements added since the matching
    /// [`save`](Self::save).
    // Port of: src/core/SkClipStack.cpp#L552-L555 (chrome/m156)
    pub fn restore(&mut self) {
        self.save_count -= 1;
        self.restore_to(self.save_count);
    }

    /// Restore the stack back to the specified save count.
    // Port of: src/core/SkClipStack.cpp#L557-L566 (chrome/m156)
    fn restore_to(&mut self, save_count: i32) {
        while let Some(element) = self.deque.last() {
            if element.save_count <= save_count {
                break;
            }
            self.deque.pop();
        }
    }

    /// The current finite bound, which kind of bound it is, and whether the bound resulted from
    /// an intersection of rects (`getBounds`). If the bound is [`BoundsType::Normal`] it encloses
    /// all writeable pixels. If it is [`BoundsType::InsideOut`] it encloses all the un-writeable
    /// pixels and the true/normal bound is the infinite plane.
    // Port of: src/core/SkClipStack.cpp#L582-L604 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn get_bounds(&self) -> (Rect, BoundsType, bool) {
        let Some(element) = self.deque.last() else {
            // the clip is wide open - the infinite plane w/ no pixels un-writeable
            return (Rect::new_empty(), BoundsType::InsideOut, false);
        };

        (
            element.finite_bound,
            element.finite_bound_type,
            element.is_intersection_of_rects,
        )
    }

    /// The bounds of the clip within `device_bounds` (`bounds`).
    // Port of: src/core/SkClipStack.cpp#L568-L577 (chrome/m156)
    #[must_use]
    pub fn bounds(&self, device_bounds: &IRect) -> Rect {
        // TODO: optimize this.
        let (mut r, bounds, _) = self.get_bounds();
        if bounds == BoundsType::InsideOut {
            return Rect::from_irect(device_bounds);
        }
        if r.intersect(Rect::from_irect(device_bounds)) {
            r
        } else {
            Rect::new_empty()
        }
    }

    /// True if the clip lets no pixel of `device_bounds` through (`isEmpty`).
    // Port of: src/core/SkClipStack.cpp#L579-L580 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self, device_bounds: &IRect) -> bool {
        // TODO: optimize this.
        self.bounds(device_bounds).is_empty()
    }

    /// Returns true if the input rect in device space is entirely contained by the clip. A return
    /// value of false does not guarantee that the rect is not contained by the clip
    /// (`quickContains(const SkRect&)`).
    // Port of: src/core/SkClipStack.h#L276-L278 (chrome/m156)
    #[doc(alias = "quickContains")]
    #[must_use]
    pub fn quick_contains_rect(&self, dev_rect: &Rect) -> bool {
        self.is_wide_open() || self.internal_quick_contains_rect(dev_rect)
    }

    /// Returns true if the input rrect in device space is entirely contained by the clip. A
    /// return value of false does not guarantee that the rrect is not contained by the clip
    /// (`quickContains(const SkRRect&)`).
    // Port of: src/core/SkClipStack.h#L280-L282 (chrome/m156)
    #[doc(alias = "quickContains")]
    #[must_use]
    pub fn quick_contains_rrect(&self, dev_rrect: &RRect) -> bool {
        self.is_wide_open() || self.internal_quick_contains_rrect(dev_rrect)
    }

    // Port of: src/core/SkClipStack.cpp#L606-L630 (chrome/m156)
    fn internal_quick_contains_rect(&self, rect: &Rect) -> bool {
        let mut iter = Iter::new(self, IterStart::Top);
        while let Some(element) = iter.prev() {
            // TODO: Once expanding ops are removed, this condition is equiv. to op == kDifference.
            if ClipOp::Intersect != element.op() && !element.is_replace_op() {
                return false;
            }
            if element.is_inverse_filled() {
                // Part of 'rect' could be trimmed off by the inverse-filled clip element
                if Rect::intersects2(element.bounds(), rect) {
                    return false;
                }
            } else if !element.contains_rect(rect) {
                return false;
            }
            if element.is_replace_op() {
                break;
            }
        }
        true
    }

    // Port of: src/core/SkClipStack.cpp#L632-L656 (chrome/m156)
    fn internal_quick_contains_rrect(&self, rrect: &RRect) -> bool {
        let mut iter = Iter::new(self, IterStart::Top);
        while let Some(element) = iter.prev() {
            // TODO: Once expanding ops are removed, this condition is equiv. to op == kDifference.
            if ClipOp::Intersect != element.op() && !element.is_replace_op() {
                return false;
            }
            if element.is_inverse_filled() {
                // Part of 'rrect' could be trimmed off by the inverse-filled clip element
                if Rect::intersects2(element.bounds(), rrect.bounds()) {
                    return false;
                }
            } else if !element.contains_rrect(rrect) {
                return false;
            }
            if element.is_replace_op() {
                break;
            }
        }
        true
    }

    /// Clips to the integer rect `ir` (`clipDevRect`).
    // Port of: src/core/SkClipStack.h#L284-L288 (chrome/m156)
    #[doc(alias = "clipDevRect")]
    pub fn clip_dev_rect(&mut self, ir: &IRect, op: ClipOp) {
        let mut r = Rect::new_empty();
        r.set_irect(ir);
        self.clip_rect(&r, Matrix::i(), op, false);
    }

    /// Helper for `clip_rect`, etc.
    // Port of: src/core/SkClipStack.cpp#L658-L712 (chrome/m156)
    fn push_element(&mut self, element: &Element) {
        // `prior` is the back of the stack and `prior_prior` the element below it (Skia walks a
        // reverse iterator for them).
        if !self.deque.is_empty() {
            if element.is_replace_op() {
                self.restore_to(self.save_count - 1);
            } else if self.deque[self.deque.len() - 1]
                .can_be_intersected_in_place(self.save_count, element.op())
            {
                let last = self.deque.len() - 1;
                let (head, tail) = self.deque.split_at_mut(last);
                let prior = &mut tail[0];
                let prior_prior = head.last();
                match prior.device_space_type {
                    DeviceSpaceType::Empty => {
                        prior.check_empty();
                        return;
                    }
                    DeviceSpaceType::Shader => {
                        if DeviceSpaceType::Shader == element.device_space_type {
                            let src = element
                                .shader
                                .clone()
                                .expect("a shader element has a shader");
                            let dst = prior.shader.take().expect("a shader element has a shader");
                            prior.shader = Some(shaders::blend(BlendMode::SrcIn, src, dst));
                            prior.update_bound_and_gen_id(prior_prior);
                            return;
                        }
                    }
                    DeviceSpaceType::Rect => {
                        if DeviceSpaceType::Rect == element.device_space_type {
                            if prior.rect_rect_intersect_allowed(
                                element.device_space_rect(),
                                element.is_aa(),
                            ) {
                                let mut isect_rect = Rect::new_empty();
                                if !isect_rect.intersect2(
                                    prior.device_space_rect(),
                                    element.device_space_rect(),
                                ) {
                                    prior.set_empty();
                                    return;
                                }

                                prior.device_space_rrect.set_rect(isect_rect);
                                prior.do_aa = element.is_aa();
                                prior.update_bound_and_gen_id(prior_prior);
                                return;
                            }
                        } else if !Rect::intersects2(prior.bounds(), element.bounds()) {
                            // [[fallthrough]] to the default case
                            prior.set_empty();
                            return;
                        }
                    }
                    DeviceSpaceType::RRect | DeviceSpaceType::Path => {
                        if !Rect::intersects2(prior.bounds(), element.bounds()) {
                            prior.set_empty();
                            return;
                        }
                    }
                }
            }
        }
        let mut new_element = element.clone();
        new_element.update_bound_and_gen_id(self.deque.last());
        self.deque.push(new_element);
    }

    /// Clips to `rect` mapped by `matrix` (`clipRect`).
    // Port of: src/core/SkClipStack.cpp#L719-L722 (chrome/m156)
    #[doc(alias = "clipRect")]
    pub fn clip_rect(&mut self, rect: &Rect, matrix: &Matrix, op: ClipOp, do_aa: bool) {
        let element = Element::with_save_count_rect(self.save_count, rect, matrix, op, do_aa);
        self.push_element(&element);
    }

    /// Clips to `rrect` mapped by `matrix` (`clipRRect`).
    // Port of: src/core/SkClipStack.cpp#L714-L717 (chrome/m156)
    #[doc(alias = "clipRRect")]
    pub fn clip_rrect(&mut self, rrect: &RRect, matrix: &Matrix, op: ClipOp, do_aa: bool) {
        let element = Element::with_save_count_rrect(self.save_count, rrect, matrix, op, do_aa);
        self.push_element(&element);
    }

    /// Clips to `path` mapped by `matrix` (`clipPath`).
    // Port of: src/core/SkClipStack.cpp#L724-L728 (chrome/m156)
    #[doc(alias = "clipPath")]
    pub fn clip_path(&mut self, path: &Path, matrix: &Matrix, op: ClipOp, do_aa: bool) {
        let element = Element::with_save_count_path(self.save_count, path, matrix, op, do_aa);
        self.push_element(&element);
    }

    /// Clips to `shader`'s coverage (`clipShader`).
    // Port of: src/core/SkClipStack.cpp#L730-L733 (chrome/m156)
    #[doc(alias = "clipShader")]
    pub fn clip_shader(&mut self, shader: Shader) {
        let element = Element::with_save_count_shader(self.save_count, shader);
        self.push_element(&element);
    }

    /// An optimized version of `clip_dev_rect(emptyRect, Intersect)` (`clipEmpty`).
    // Port of: src/core/SkClipStack.cpp#L740-L749 (chrome/m156)
    #[doc(alias = "clipEmpty")]
    pub fn clip_empty(&mut self) {
        if let Some(element) = self.deque.last_mut()
            && element.can_be_intersected_in_place(self.save_count, ClipOp::Intersect)
        {
            element.set_empty();
        }
        let mut element = Element::with_save_count_empty(self.save_count);
        element.gen_id = EMPTY_GEN_ID;
        self.deque.push(element);
    }

    /// Replaces the clip with `dev_rect` (`replaceClip`).
    // Port of: src/core/SkClipStack.cpp#L735-L738 (chrome/m156)
    #[doc(alias = "replaceClip")]
    pub fn replace_clip(&mut self, dev_rect: &Rect, do_aa: bool) {
        let element = Element::with_save_count_replace_rect(self.save_count, dev_rect, do_aa);
        self.push_element(&element);
    }

    /// Returns true if the clip state corresponds to the infinite plane (i.e., draws are not
    /// limited at all) (`isWideOpen`).
    // Port of: src/core/SkClipStack.h#L317 (chrome/m156)
    #[doc(alias = "isWideOpen")]
    #[must_use]
    pub fn is_wide_open(&self) -> bool {
        self.topmost_gen_id() == WIDE_OPEN_GEN_ID
    }

    /// Quickly and conservatively determines whether the entire stack is equivalent to
    /// intersection with a rrect given a bounds, where the rrect must not contain the entire
    /// bounds (`isRRect`).
    ///
    /// `bounds` is a bounds on what will be drawn through the clip. The clip only need be
    /// equivalent to an intersection with a rrect for draws within the bounds. The returned rrect
    /// must intersect the bounds but need not be contained by the bounds. Returns the rrect
    /// equivalent to the stack and whether it is antialiased, or `None` if the stack is not
    /// equivalent to a single rrect intersect clip.
    // Port of: src/core/SkClipStack.cpp#L841-L894 (chrome/m156)
    #[doc(alias = "isRRect")]
    #[must_use]
    pub fn is_rrect(&self, bounds: &Rect) -> Option<(RRect, bool)> {
        // TODO: return bounds when there is no back?
        let back = self.deque.last()?;
        // First check if the entire stack is known to be a rect by the top element.
        if back.is_intersection_of_rects && back.finite_bound_type == BoundsType::Normal {
            let mut rrect = RRect::new();
            rrect.set_rect(back.finite_bound);
            return Some((rrect, back.is_aa()));
        }

        if back.device_space_type() != DeviceSpaceType::Rect
            && back.device_space_type() != DeviceSpaceType::RRect
        {
            return None;
        }
        if back.is_replace_op() {
            return Some((*back.as_device_space_rrect(), back.is_aa()));
        }

        if back.op() == ClipOp::Intersect {
            let mut back_bounds = Rect::new_empty();
            if !back_bounds.intersect2(bounds, back.as_device_space_rrect().rect()) {
                return None;
            }
            // We limit to 17 elements. This means the back element will be bounds checked at
            // most 16 times if it is an rrect.
            let cnt = self.deque.len();
            if cnt > 17 {
                return None;
            }
            if cnt > 1 {
                for prior in self.deque[..cnt - 1].iter().rev() {
                    // TODO: Once expanding clip ops are removed, this is equiv. to op ==
                    // kDifference
                    if (prior.op() != ClipOp::Intersect && !prior.is_replace_op())
                        || !prior.contains_rect(&back_bounds)
                    {
                        return None;
                    }
                    if prior.is_replace_op() {
                        break;
                    }
                }
            }
            return Some((*back.as_device_space_rrect(), back.is_aa()));
        }
        None
    }

    /// The generation id of the whole stack: [`WIDE_OPEN_GEN_ID`] if it lets everything through,
    /// otherwise the topmost element's id (`getTopmostGenID`).
    // Port of: src/core/SkClipStack.cpp#L908-L920 (chrome/m156)
    #[doc(alias = "getTopmostGenID")]
    #[must_use]
    pub fn topmost_gen_id(&self) -> u32 {
        let Some(back) = self.deque.last() else {
            return WIDE_OPEN_GEN_ID;
        };
        if BoundsType::InsideOut == back.finite_bound_type
            && back.finite_bound.is_empty()
            && DeviceSpaceType::Shader != back.device_space_type
        {
            return WIDE_OPEN_GEN_ID;
        }

        back.gen_id()
    }

    /// A conservative bound of the current clip, as a rect in `[0, max_width) x [0, max_height)`
    /// after translating finite bounds by (`offset_x`, `offset_y`), and whether it is the result
    /// of an intersection of rects, in which case the bound is the exact answer
    /// (`getConservativeBounds`). Since the clip could be the infinite plane (if inverse fills
    /// were involved) the max width and height limit the returned bound to the expected drawing
    /// area. The offsets allow the caller to account for translated drawing areas (i.e., those
    /// resulting from a saveLayer).
    // Port of: src/core/SkClipStack.cpp#L813-L839 (chrome/m156)
    #[doc(alias = "getConservativeBounds")]
    #[must_use]
    pub fn get_conservative_bounds(
        &self,
        offset_x: i32,
        offset_y: i32,
        max_width: i32,
        max_height: i32,
    ) -> (Rect, bool) {
        let mut dev_bounds = Rect::new(
            0.0,
            0.0,
            int_to_scalar(max_width),
            int_to_scalar(max_height),
        );

        // temp starts off in canvas space here
        let (mut temp, bound_type, is_intersection_of_rects) = self.get_bounds();
        if BoundsType::InsideOut == bound_type {
            return (dev_bounds, is_intersection_of_rects);
        }

        // but is converted to device space here
        temp.offset((int_to_scalar(offset_x), int_to_scalar(offset_y)));

        if !dev_bounds.intersect(temp) {
            dev_bounds.set_empty();
        }
        (dev_bounds, is_intersection_of_rects)
    }
}

/// Where an [`Iter`] starts (`SkClipStack::Iter::IterStart`).
// Port of: src/core/SkClipStack.h#L340-L343 (chrome/m156)
#[doc(alias = "SkClipStack::Iter::IterStart")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IterStart {
    /// `kBottom_IterStart` (`SkDeque::Iter::kFront_IterStart`).
    #[doc(alias = "kBottom_IterStart")]
    Bottom,
    /// `kTop_IterStart` (`SkDeque::Iter::kBack_IterStart`).
    #[doc(alias = "kTop_IterStart")]
    Top,
}

/// A bidirectional iterator over the elements of a [`ClipStack`] (`SkClipStack::Iter`), with the
/// semantics of `SkDeque::Iter`: the iterator holds a cursor at an element (or nowhere);
/// [`next`](Self::next) and [`prev`](Self::prev) both return the element at the cursor and then
/// move it forward or backward, so after running off an end the iterator stays done.
// Port of: src/core/SkClipStack.h#L336-L384 (chrome/m156)
#[doc(alias = "SkClipStack::Iter")]
#[derive(Clone, Debug, Default)]
pub struct Iter<'a> {
    /// `fStack`.
    stack: Option<&'a ClipStack>,
    /// `fIter`: the index of the element at the cursor.
    pos: Option<usize>,
}

impl<'a> Iter<'a> {
    /// An uninitialized iterator, which must be [`reset`](Self::reset) (`Iter()`).
    // Port of: src/core/SkClipStack.cpp#L753-L754 (chrome/m156)
    #[must_use]
    pub fn new_uninit() -> Iter<'a> {
        Iter {
            stack: None,
            pos: None,
        }
    }

    /// An iterator over `stack` starting at `start_loc` (`Iter(stack, startLoc)`).
    // Port of: src/core/SkClipStack.cpp#L756-L759 (chrome/m156)
    #[must_use]
    pub fn new(stack: &'a ClipStack, start_loc: IterStart) -> Iter<'a> {
        let mut iter = Iter::new_uninit();
        iter.reset(stack, start_loc);
        iter
    }

    /// The clip element at the cursor, moving the cursor towards the top; `None` when the
    /// iterator is done (`next`).
    // Port of: src/core/SkClipStack.cpp#L761-L763 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors the C++ API: `prev` shares the cursor
    pub fn next(&mut self) -> Option<&'a Element> {
        let stack = self.stack?;
        let pos = self.pos?;
        // SkDeque::Iter::next
        self.pos = if pos + 1 < stack.deque.len() {
            Some(pos + 1)
        } else {
            None
        };
        Some(&stack.deque[pos])
    }

    /// The clip element at the cursor, moving the cursor towards the bottom; `None` when the
    /// iterator is done (`prev`).
    // Port of: src/core/SkClipStack.cpp#L765-L767 (chrome/m156)
    pub fn prev(&mut self) -> Option<&'a Element> {
        let stack = self.stack?;
        let pos = self.pos?;
        // SkDeque::Iter::prev
        self.pos = pos.checked_sub(1);
        Some(&stack.deque[pos])
    }

    /// Moves the iterator to the topmost element with the specified op and returns that element.
    /// If no clip element with that op is found, the first element is returned
    /// (`skipToTopmost`).
    // Port of: src/core/SkClipStack.cpp#L769-L805 (chrome/m156)
    #[doc(alias = "skipToTopmost")]
    pub fn skip_to_topmost(&mut self, op: ClipOp) -> Option<&'a Element> {
        let stack = self.stack?;

        self.reset(stack, IterStart::Top);

        let mut found = false;
        while let Some(element) = self.prev() {
            if op == element.op {
                // The Deque's iterator is actually one pace ahead of the returned value. So while
                // "element" is the element we want to return, the iterator is actually pointing
                // at (and will return on the next "next" or "prev" call) the element in front of
                // it in the deque. Bump the iterator forward a step so we get the expected
                // result.
                if self.next().is_none() {
                    // The reverse iterator has run off the front of the deque (i.e., the "op"
                    // clip is the first clip) and can't recover. Reset the iterator to start at
                    // the front.
                    self.reset(stack, IterStart::Bottom);
                }
                found = true;
                break;
            }
        }

        if !found {
            // There were no "op" clips
            self.reset(stack, IterStart::Bottom);
        }

        self.next()
    }

    /// Restarts the iterator on a clip stack (`reset`).
    // Port of: src/core/SkClipStack.cpp#L807-L810 (chrome/m156)
    pub fn reset(&mut self, stack: &'a ClipStack, start_loc: IterStart) {
        self.stack = Some(stack);
        // SkDeque::Iter::reset
        self.pos = match start_loc {
            IterStart::Bottom => {
                if stack.deque.is_empty() {
                    None
                } else {
                    Some(0)
                }
            }
            IterStart::Top => stack.deque.len().checked_sub(1),
        };
    }
}

/// Iterates from the bottom of the stack to the top (`SkClipStack::B2TIter`). It wraps [`Iter`]
/// privately to prevent access to reverse iteration.
// Port of: src/core/SkClipStack.h#L386-L416 (chrome/m156)
#[doc(alias = "SkClipStack::B2TIter")]
#[derive(Clone, Debug, Default)]
pub struct B2TIter<'a>(Iter<'a>);

impl<'a> B2TIter<'a> {
    /// An uninitialized iterator, which must be [`reset`](Self::reset) (`B2TIter()`).
    #[must_use]
    pub fn new_uninit() -> B2TIter<'a> {
        B2TIter(Iter::new_uninit())
    }

    /// An iterator initialized to the bottom of the stack (`B2TIter(stack)`).
    #[must_use]
    pub fn new(stack: &'a ClipStack) -> B2TIter<'a> {
        B2TIter(Iter::new(stack, IterStart::Bottom))
    }

    /// Initializes the iterator to the bottom of the stack (`reset`).
    pub fn reset(&mut self, stack: &'a ClipStack) {
        self.0.reset(stack, IterStart::Bottom);
    }
}

impl<'a> Iterator for B2TIter<'a> {
    type Item = &'a Element;

    fn next(&mut self) -> Option<&'a Element> {
        self.0.next()
    }
}

/// Restores a [`ClipStack`] to its save count at construction when dropped
/// (`SkClipStack::AutoRestore`). It dereferences to the stack.
// Port of: src/core/SkClipStack.h#L130-L152 (chrome/m156)
#[doc(alias = "SkClipStack::AutoRestore")]
#[derive(Debug)]
pub struct AutoRestore<'a> {
    cs: &'a mut ClipStack,
    save_count: i32,
}

impl<'a> AutoRestore<'a> {
    /// Remembers `cs`'s save count, and saves it first if `do_save`.
    #[must_use]
    pub fn new(cs: &'a mut ClipStack, do_save: bool) -> AutoRestore<'a> {
        let save_count = cs.save_count();
        if do_save {
            cs.save();
        }
        AutoRestore { cs, save_count }
    }
}

impl std::ops::Deref for AutoRestore<'_> {
    type Target = ClipStack;

    fn deref(&self) -> &ClipStack {
        self.cs
    }
}

impl std::ops::DerefMut for AutoRestore<'_> {
    fn deref_mut(&mut self) -> &mut ClipStack {
        self.cs
    }
}

impl Drop for AutoRestore<'_> {
    fn drop(&mut self) {
        debug_assert!(self.cs.save_count() >= self.save_count); // no underflow
        while self.cs.save_count() > self.save_count {
            self.cs.restore();
        }
    }
}
