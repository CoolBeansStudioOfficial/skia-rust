// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRRect.h, src/core/SkRRect.cpp, src/core/SkRRectPriv.h,
// src/core/SkScaleToSides.h, src/utils/SkFloatUtils.h

//! Rounded rectangles (`SkRRect.h`).
//!
//! [`RRect`] describes a rounded rectangle with a bounds and a pair of radii for each corner. The
//! bounds and radii can be set so that [`RRect`] describes: a rectangle with sharp corners; a
//! circle; an oval; or a rectangle with one or more rounded corners.
//!
//! [`RRect`] allows implementing CSS properties that describe rounded corners. [`RRect`] may have
//! up to eight different radii, one for each axis on each of its four corners.
//!
//! [`RRect`] may modify the provided parameters when initializing bounds and radii. If either axis
//! radii is zero or less: radii are stored as zero; corner is square. If corner curves overlap,
//! radii are proportionally reduced to fit within bounds.
//!
//! `SkRRect::transform`, `SkRRect::dump*` and `SkRRectPriv::{Read,Write}*Buffer` are not ported
//! yet (they need `SkMatrix`, `SkString` and `SkRBuffer`/`SkWBuffer`).

use crate::floating_point::{float_midpoint, ieee_float_divide, is_finite_all, is_finite_array};
use crate::point::{Point, Vector};
use crate::rect::{Contains, Rect, rect_priv};
use crate::scalar::{SCALAR_1, Scalar, scalar};

/// `std::min(a, b)` is `(b < a) ? b : a`; it differs from `f32::min` when an operand is NaN.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

/// `std::max(a, b)` is `(a < b) ? b : a`; it differs from `f32::max` when an operand is NaN.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// Describes possible specializations of [`RRect`]. Each type is exclusive; an [`RRect`] may only
/// have one type.
///
/// Type members become progressively less restrictive; larger values of type have more degrees of
/// freedom than smaller values.
// Port of: include/core/SkRRect.h#L70-L77 (chrome/m156)
#[doc(alias = "SkRRect_Type")]
#[doc(alias = "SkRRect::Type")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[repr(i32)]
pub enum Type {
    /// Zero width or height.
    #[default]
    Empty = 0,
    /// Non-zero width and height, and zeroed radii.
    Rect = 1,
    /// Non-zero width and height filled with radii.
    Oval = 2,
    /// Non-zero width and height with equal radii.
    Simple = 3,
    /// Non-zero width and height with axis-aligned radii.
    NinePatch = 4,
    /// Non-zero width and height with arbitrary radii.
    Complex = 5,
}

impl Type {
    /// Largest [`Type`] value (`SkRRect::kLastType`).
    #[doc(alias = "kLastType")]
    pub const LAST: Type = Type::Complex;
}

/// The radii are stored: top-left, top-right, bottom-right, bottom-left.
// Port of: include/core/SkRRect.h#L276-L281 (chrome/m156)
#[doc(alias = "SkRRect_Corner")]
#[doc(alias = "SkRRect::Corner")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(usize)]
pub enum Corner {
    /// Index of top-left corner radii.
    UpperLeft = 0,
    /// Index of top-right corner radii.
    UpperRight = 1,
    /// Index of bottom-right corner radii.
    LowerRight = 2,
    /// Index of bottom-left corner radii.
    LowerLeft = 3,
}

const UPPER_LEFT: usize = Corner::UpperLeft as usize;
const UPPER_RIGHT: usize = Corner::UpperRight as usize;
const LOWER_RIGHT: usize = Corner::LowerRight as usize;
const LOWER_LEFT: usize = Corner::LowerLeft as usize;

/// A rounded rectangle: bounds plus a pair of radii for each corner.
// Port of: include/core/SkRRect.h#L50-L545 (chrome/m156)
#[doc(alias = "SkRRect")]
#[derive(Copy, Clone, Debug)]
pub struct RRect {
    rect: Rect,
    radii: [Vector; 4],
    type_: Type,
}

impl Default for RRect {
    /// Initializes corner radii to (0, 0), and sets type of [`Type::Empty`].
    fn default() -> Self {
        Self::new()
    }
}

impl AsRef<RRect> for RRect {
    fn as_ref(&self) -> &RRect {
        self
    }
}

/// `a` and `b` are not equal if either contain NaN. `a` and `b` are equal if members contain
/// zeroes with different signs.
impl PartialEq for RRect {
    // Port of: include/core/SkRRect.h#L319-L321 (chrome/m156)
    fn eq(&self, rhs: &Self) -> bool {
        self.rect == rhs.rect && self.radii == rhs.radii
    }
}

impl RRect {
    /// Number of bytes written by [`RRect::write_to_memory`]: `12 * size_of::<scalar>()`.
    // Port of: include/core/SkRRect.h#L459 (chrome/m156)
    #[doc(alias = "kSizeInMemory")]
    pub const SIZE_IN_MEMORY: usize = 12 * 4;

    /// Initializes bounds at (0, 0), the origin, with zero width and height; radii to (0, 0) and
    /// type to [`Type::Empty`].
    // Port of: include/core/SkRRect.h#L50-L55 (chrome/m156)
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rect: Rect::new_empty(),
            radii: [Point::new(0.0, 0.0); 4],
            type_: Type::Empty,
        }
    }

    /// Returns the empty [`RRect`].
    // Port of: include/core/SkRRect.h#L146 (chrome/m156)
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub const fn new_empty() -> Self {
        Self::new()
    }

    /// Returns the [`Type`] of this rounded rectangle.
    // Port of: include/core/SkRRect.h#L80-L83 (chrome/m156)
    #[doc(alias = "getType")]
    #[doc(alias = "type")]
    #[must_use]
    pub fn get_type(&self) -> Type {
        debug_assert!(self.is_valid());
        self.type_
    }

    /// Returns true if the type is [`Type::Empty`].
    // Port of: include/core/SkRRect.h#L97 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.get_type() == Type::Empty
    }

    /// Returns true if the type is [`Type::Rect`].
    // Port of: include/core/SkRRect.h#L103 (chrome/m156)
    #[doc(alias = "isRect")]
    #[must_use]
    pub fn is_rect(&self) -> bool {
        self.get_type() == Type::Rect
    }

    /// Returns true if the type is [`Type::Oval`].
    // Port of: include/core/SkRRect.h#L109 (chrome/m156)
    #[doc(alias = "isOval")]
    #[must_use]
    pub fn is_oval(&self) -> bool {
        self.get_type() == Type::Oval
    }

    /// Returns true if the type is [`Type::Simple`].
    // Port of: include/core/SkRRect.h#L115 (chrome/m156)
    #[doc(alias = "isSimple")]
    #[must_use]
    pub fn is_simple(&self) -> bool {
        self.get_type() == Type::Simple
    }

    /// Returns true if the type is [`Type::NinePatch`].
    // Port of: include/core/SkRRect.h#L121 (chrome/m156)
    #[doc(alias = "isNinePatch")]
    #[must_use]
    pub fn is_nine_patch(&self) -> bool {
        self.get_type() == Type::NinePatch
    }

    /// Returns true if the type is [`Type::Complex`].
    // Port of: include/core/SkRRect.h#L127 (chrome/m156)
    #[doc(alias = "isComplex")]
    #[must_use]
    pub fn is_complex(&self) -> bool {
        self.get_type() == Type::Complex
    }

    /// Returns span on the x-axis. This does not check if result fits in 32-bit float; result may
    /// be infinity.
    // Port of: include/core/SkRRect.h#L133 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> scalar {
        self.rect.width()
    }

    /// Returns span on the y-axis. This does not check if result fits in 32-bit float; result may
    /// be infinity.
    // Port of: include/core/SkRRect.h#L139 (chrome/m156)
    #[must_use]
    pub fn height(&self) -> scalar {
        self.rect.height()
    }

    /// Returns the radii of the upper-left corner, which is representative of all corners for
    /// [`Type::Empty`], [`Type::Rect`], [`Type::Oval`] and [`Type::Simple`].
    // Port of: include/core/SkRRect.h#L115-L117 (chrome/m156)
    #[doc(alias = "getSimpleRadii")]
    #[must_use]
    pub fn simple_radii(&self) -> Vector {
        self.radii[0]
    }

    /// Sets bounds to zero width and height at (0, 0), the origin; corner radii to zero and type
    /// to [`Type::Empty`].
    // Port of: include/core/SkRRect.h#L122 (chrome/m156)
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) {
        *self = Self::new();
    }

    /// Sets bounds to `rect`, with all corner radii zero. If `rect` has non-zero width and
    /// height, sets type to [`Type::Rect`]; otherwise, sets type to [`Type::Empty`].
    // Port of: include/core/SkRRect.h#L130-L139 (chrome/m156)
    #[doc(alias = "setRect")]
    pub fn set_rect(&mut self, rect: impl AsRef<Rect>) {
        let rect = rect.as_ref();
        if !self.initialize_rect(rect) {
            return;
        }

        self.radii = [Point::new(0.0, 0.0); 4];
        self.type_ = Type::Rect;

        debug_assert!(self.is_valid());
    }

    /// Returns a new [`RRect`] with bounds `rect`; see [`RRect::set_rect`].
    // Port of: include/core/SkRRect.h#L153-L157 (chrome/m156)
    #[doc(alias = "MakeRect")]
    #[must_use]
    pub fn new_rect(rect: impl AsRef<Rect>) -> Self {
        let mut rr = Self::new();
        rr.set_rect(rect);
        rr
    }

    /// Returns a new oval; see [`RRect::set_oval`].
    // Port of: include/core/SkRRect.h#L166-L170 (chrome/m156)
    #[doc(alias = "MakeOval")]
    #[must_use]
    pub fn new_oval(oval: impl AsRef<Rect>) -> Self {
        let mut rr = Self::new();
        rr.set_oval(oval);
        rr
    }

    /// Returns a new rounded rectangle; see [`RRect::set_rect_xy`].
    // Port of: include/core/SkRRect.h#L184-L188 (chrome/m156)
    #[doc(alias = "MakeRectXY")]
    #[must_use]
    pub fn new_rect_xy(rect: impl AsRef<Rect>, x_rad: scalar, y_rad: scalar) -> Self {
        let mut rr = Self::new();
        rr.set_rect_xy(rect, x_rad, y_rad);
        rr
    }

    /// Returns a new rounded rectangle; see [`RRect::set_rect_radii`].
    // Port of: include/core/SkRRect.h#L205-L209 (chrome/m156)
    #[doc(alias = "MakeRectRadii")]
    #[must_use]
    pub fn new_rect_radii(rect: impl AsRef<Rect>, radii: &[Vector; 4]) -> Self {
        let mut rr = Self::new();
        rr.set_rect_radii(rect, radii);
        rr
    }

    /// Returns a new rounded rectangle; see [`RRect::set_nine_patch`].
    #[must_use]
    pub fn new_nine_patch(
        rect: impl AsRef<Rect>,
        left_rad: scalar,
        top_rad: scalar,
        right_rad: scalar,
        bottom_rad: scalar,
    ) -> Self {
        let mut rr = Self::new();
        rr.set_nine_patch(rect, left_rad, top_rad, right_rad, bottom_rad);
        rr
    }

    /// Sets bounds to oval, x-axis radii to half `oval.width()`, and all y-axis radii to half
    /// `oval.height()`. If oval bounds is empty, sets to [`Type::Empty`]. Otherwise, sets to
    /// [`Type::Oval`].
    // Port of: src/core/SkRRect.cpp#L33-L53 (chrome/m156)
    #[doc(alias = "setOval")]
    pub fn set_oval(&mut self, oval: impl AsRef<Rect>) {
        let oval = oval.as_ref();
        if !self.initialize_rect(oval) {
            return;
        }

        let x_rad = rect_priv::half_width(&self.rect);
        let y_rad = rect_priv::half_height(&self.rect);

        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        if x_rad == 0.0 || y_rad == 0.0 {
            // All the corners will be square
            self.radii = [Point::new(0.0, 0.0); 4];
            self.type_ = Type::Rect;
        } else {
            for radius in &mut self.radii {
                radius.set(x_rad, y_rad);
            }
            self.type_ = Type::Oval;
        }

        debug_assert!(self.is_valid());
    }

    /// Sets to rounded rectangle with the same radii for all four corners.
    ///
    /// If `rect` is empty, sets to [`Type::Empty`]. Otherwise, if `x_rad` or `y_rad` is zero, sets
    /// to [`Type::Rect`]. Otherwise, if `x_rad` is at least half `rect.width()` and `y_rad` is at
    /// least half `rect.height()`, sets to [`Type::Oval`]. Otherwise, sets to [`Type::Simple`].
    // Port of: src/core/SkRRect.cpp#L55-L93 (chrome/m156)
    #[doc(alias = "setRectXY")]
    pub fn set_rect_xy(&mut self, rect: impl AsRef<Rect>, x_rad: scalar, y_rad: scalar) {
        let rect = rect.as_ref();
        let (mut x_rad, mut y_rad) = (x_rad, y_rad);
        if !self.initialize_rect(rect) {
            return;
        }

        if !is_finite_all(x_rad, &[y_rad]) {
            // devolve into a simple rect
            x_rad = 0.0;
            y_rad = 0.0;
        }

        if self.rect.width() < x_rad + x_rad || self.rect.height() < y_rad + y_rad {
            // At most one of these two divides will be by zero, and neither numerator is zero.
            let scale = std_min(
                ieee_float_divide(self.rect.width(), x_rad + x_rad),
                ieee_float_divide(self.rect.height(), y_rad + y_rad),
            );
            debug_assert!(scale < SCALAR_1);
            x_rad *= scale;
            y_rad *= scale;
        }

        if x_rad <= 0.0 || y_rad <= 0.0 {
            // all corners are square in this case
            self.set_rect(rect);
            return;
        }

        for radius in &mut self.radii {
            radius.set(x_rad, y_rad);
        }
        self.type_ = Type::Simple;
        if x_rad >= (self.rect.width() / 2.0) && y_rad >= (self.rect.height() / 2.0) {
            self.type_ = Type::Oval;
            x_rad = rect_priv::half_width(&self.rect);
            y_rad = rect_priv::half_height(&self.rect);
            for radius in &mut self.radii {
                radius.set(x_rad, y_rad);
            }
        }

        debug_assert!(self.is_valid());
    }

    /// Sets bounds to `rect`. Sets radii to `(left_rad, top_rad)`, `(right_rad, top_rad)`,
    /// `(right_rad, bottom_rad)`, `(left_rad, bottom_rad)`.
    ///
    /// If `rect` is empty, sets to [`Type::Empty`]. Otherwise, if `left_rad` and `right_rad` are
    /// zero, sets to [`Type::Rect`]. Otherwise, if `top_rad` and `bottom_rad` are zero, sets to
    /// [`Type::Rect`]. Otherwise, if `left_rad` and `right_rad` are equal and at least half
    /// `rect.width()`, and `top_rad` and `bottom_rad` are equal at least half `rect.height()`,
    /// sets to [`Type::Oval`]. Otherwise, if `left_rad` and `right_rad` are equal, and `top_rad`
    /// and `bottom_rad` are equal, sets to [`Type::Simple`]. Otherwise, sets to
    /// [`Type::NinePatch`].
    // Port of: src/core/SkRRect.cpp#L122-L186 (chrome/m156)
    #[doc(alias = "setNinePatch")]
    #[allow(clippy::float_cmp)] // exact comparisons, as in Skia
    pub fn set_nine_patch(
        &mut self,
        rect: impl AsRef<Rect>,
        left_rad: scalar,
        top_rad: scalar,
        right_rad: scalar,
        bottom_rad: scalar,
    ) {
        let rect = rect.as_ref();
        let (mut left_rad, mut top_rad, mut right_rad, mut bottom_rad) =
            (left_rad, top_rad, right_rad, bottom_rad);
        if !self.initialize_rect(rect) {
            return;
        }

        if !is_finite_all(left_rad, &[top_rad, right_rad, bottom_rad]) {
            self.set_rect(rect); // devolve into a simple rect
            return;
        }

        left_rad = std_max(left_rad, 0.0);
        top_rad = std_max(top_rad, 0.0);
        right_rad = std_max(right_rad, 0.0);
        bottom_rad = std_max(bottom_rad, 0.0);

        let mut scale: scalar = 1.0;
        if left_rad + right_rad > self.rect.width() {
            scale = self.rect.width() / (left_rad + right_rad);
        }
        if top_rad + bottom_rad > self.rect.height() {
            scale = std_min(scale, self.rect.height() / (top_rad + bottom_rad));
        }

        if scale < 1.0 {
            left_rad *= scale;
            top_rad *= scale;
            right_rad *= scale;
            bottom_rad *= scale;
        }

        if left_rad == right_rad && top_rad == bottom_rad {
            if left_rad >= (self.rect.width() / 2.0) && top_rad >= (self.rect.height() / 2.0) {
                self.type_ = Type::Oval;
                left_rad = rect_priv::half_width(&self.rect);
                right_rad = left_rad;
                top_rad = rect_priv::half_height(&self.rect);
                bottom_rad = top_rad;
            } else if 0.0 == left_rad || 0.0 == top_rad {
                // If the left and (by equality check above) right radii are zero then it is a
                // rect. Same goes for top/bottom.
                self.type_ = Type::Rect;
                left_rad = 0.0;
                top_rad = 0.0;
                right_rad = 0.0;
                bottom_rad = 0.0;
            } else {
                self.type_ = Type::Simple;
            }
        } else {
            self.type_ = Type::NinePatch;
        }

        self.radii[UPPER_LEFT].set(left_rad, top_rad);
        self.radii[UPPER_RIGHT].set(right_rad, top_rad);
        self.radii[LOWER_RIGHT].set(right_rad, bottom_rad);
        self.radii[LOWER_LEFT].set(left_rad, bottom_rad);
        if clamp_to_zero(&mut self.radii) {
            self.set_rect(rect); // devolve into a simple rect
            return;
        }
        if self.type_ == Type::NinePatch && !radii_are_nine_patch(&self.radii) {
            self.type_ = Type::Complex;
        }

        debug_assert!(self.is_valid());
    }

    /// Sets bounds to `rect`. Sets radii array for individual control of all for corners.
    ///
    /// If `rect` is empty, sets to [`Type::Empty`]. Otherwise, if one of each corner radii are
    /// zero, sets to [`Type::Rect`]. Otherwise, if all x-axis radii are equal and at least half
    /// `rect.width()`, and all y-axis radii are equal at least half `rect.height()`, sets to
    /// [`Type::Oval`]. Otherwise, if all x-axis radii are equal, and all y-axis radii are equal,
    /// sets to [`Type::Simple`]. Otherwise, sets to [`Type::NinePatch`].
    // Port of: src/core/SkRRect.cpp#L198-L221 (chrome/m156)
    #[doc(alias = "setRectRadii")]
    pub fn set_rect_radii(&mut self, rect: impl AsRef<Rect>, radii: &[Vector; 4]) {
        let rect = rect.as_ref();
        if !self.initialize_rect(rect) {
            return;
        }

        let scalars = [
            radii[0].x, radii[0].y, radii[1].x, radii[1].y, radii[2].x, radii[2].y, radii[3].x,
            radii[3].y,
        ];
        if !is_finite_array(&scalars) {
            self.set_rect(rect); // devolve into a simple rect
            return;
        }

        self.radii = *radii;

        if clamp_to_zero(&mut self.radii) {
            self.set_rect(rect);
            return;
        }

        self.scale_radii();

        if !self.is_valid() {
            self.set_rect(rect);
        }
    }

    /// Returns bounds. Bounds may have zero width or zero height. Bounds right is greater than or
    /// equal to left; bounds bottom is greater than or equal to top. Result is identical to
    /// [`RRect::bounds`].
    // Port of: include/core/SkRRect.h#L291 (chrome/m156)
    #[must_use]
    pub fn rect(&self) -> &Rect {
        &self.rect
    }

    /// Returns scalar pair for radius of curve on x-axis and y-axis for one corner. Both radii may
    /// be zero. If not zero, both are positive and finite.
    // Port of: include/core/SkRRect.h#L299 (chrome/m156)
    #[must_use]
    pub fn radii(&self, corner: Corner) -> Vector {
        self.radii[corner as usize]
    }

    /// Returns the four corner radii, in [`Corner`] order (`SkRRect::radii()` span overload).
    // Port of: include/core/SkRRect.h#L300 (chrome/m156)
    #[must_use]
    pub fn radii_ref(&self) -> &[Vector; 4] {
        &self.radii
    }

    /// Returns bounds. Result is identical to [`RRect::rect`].
    // Port of: include/core/SkRRect.h#L308 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn bounds(&self) -> &Rect {
        &self.rect
    }

    /// Insets bounds by `delta`, and adjusts radii by `delta`. If either corner radius is zero,
    /// the corner has no curvature and is unchanged. Otherwise, if adjusted radius becomes
    /// negative, pins radius to zero. If `delta.x` exceeds half bounds width, bounds left and
    /// right are set to bounds x-axis center. If `delta.y` exceeds half bounds height, bounds top
    /// and bottom are set to bounds y-axis center.
    ///
    /// If `delta` causes the bounds to become infinite, bounds is zeroed.
    // Port of: include/core/SkRRect.h#L370-L372 (chrome/m156)
    pub fn inset(&mut self, delta: impl Into<Vector>) {
        *self = self.with_inset(delta);
    }

    /// Returns a copy with bounds insets by `delta`; see [`RRect::inset`].
    // Port of: src/core/SkRRect.cpp#L667-L700 (chrome/m156)
    #[doc(alias = "inset")]
    #[must_use]
    pub fn with_inset(&self, delta: impl Into<Vector>) -> Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        let mut dst = Self::new();

        let mut r = self.rect.with_inset(delta);
        let mut degenerate = false;
        if r.right <= r.left {
            degenerate = true;
            let mid = float_midpoint(r.left, r.right);
            r.left = mid;
            r.right = mid;
        }
        if r.bottom <= r.top {
            degenerate = true;
            let mid = float_midpoint(r.top, r.bottom);
            r.top = mid;
            r.bottom = mid;
        }
        if degenerate {
            dst.rect = r;
            dst.radii = [Point::new(0.0, 0.0); 4];
            dst.type_ = Type::Empty;
            return dst;
        }
        if !r.is_finite() {
            return Self::new();
        }

        let mut radii = self.radii;
        for radius in &mut radii {
            // `if (radii[i].fX)`: any non-zero value, including NaN
            #[allow(clippy::float_cmp)]
            if radius.x != 0.0 {
                radius.x -= dx;
            }
            #[allow(clippy::float_cmp)]
            if radius.y != 0.0 {
                radius.y -= dy;
            }
        }
        dst.set_rect_radii(r, &radii);
        dst
    }

    /// Outsets bounds by `delta`, and adjusts radii by `delta`; see [`RRect::inset`].
    // Port of: include/core/SkRRect.h#L405-L407 (chrome/m156)
    pub fn outset(&mut self, delta: impl Into<Vector>) {
        *self = self.with_outset(delta);
    }

    /// Returns a copy outset by `delta`; see [`RRect::outset`].
    // Port of: include/core/SkRRect.h#L382-L384 (chrome/m156)
    #[doc(alias = "outset")]
    #[must_use]
    pub fn with_outset(&self, delta: impl Into<Vector>) -> Self {
        self.with_inset(-delta.into())
    }

    /// Translates [`RRect`] by `delta`.
    // Port of: include/core/SkRRect.h#L416-L418 (chrome/m156)
    pub fn offset(&mut self, delta: impl Into<Vector>) {
        self.rect.offset(delta);
    }

    /// Returns [`RRect`] translated by `delta`.
    // Port of: include/core/SkRRect.h#L426-L428 (chrome/m156)
    #[doc(alias = "makeOffset")]
    #[must_use]
    pub fn with_offset(&self, delta: impl Into<Vector>) -> Self {
        Self {
            rect: self.rect.with_offset(delta),
            radii: self.radii,
            type_: self.type_,
        }
    }

    /// Returns true if `point` is inside the bounds and corner radii, and if [`RRect`] is not
    /// empty.
    // Port of: src/core/SkRRect.cpp#L389-L404 (chrome/m156)
    #[doc(alias = "contains")]
    #[must_use]
    pub fn contains_point(&self, point: impl Into<Point>) -> bool {
        let point = point.into();
        if !self.bounds().contains(&point) {
            // If 'point' isn't contained by the RR's bounds then the RR definitely
            // doesn't contain it.
            return false;
        }

        if self.is_rect() {
            // The prior test was sufficient.
            return true;
        }

        // At this point we know `point` is inside the bounds of this RR. Check to
        // see it is inside all the curves.
        self.check_corner_containment(point.x, point.y)
    }

    /// Returns true if `rect` is inside the bounds and corner radii, and if [`RRect`] and `rect`
    /// are not empty.
    // Port of: src/core/SkRRect.cpp#L406-L425 (chrome/m156)
    #[must_use]
    pub fn contains(&self, rect: impl AsRef<Rect>) -> bool {
        let rect = rect.as_ref();
        if !self.bounds().contains(rect) {
            // If 'rect' isn't contained by the RR's bounds then the
            // RR definitely doesn't contain it
            return false;
        }

        if self.is_rect() {
            // the prior test was sufficient
            return true;
        }

        // At this point we know all four corners of 'rect' are inside the
        // bounds of of this RR. Check to make sure all the corners are inside
        // all the curves
        self.check_corner_containment(rect.left, rect.top)
            && self.check_corner_containment(rect.right, rect.top)
            && self.check_corner_containment(rect.right, rect.bottom)
            && self.check_corner_containment(rect.left, rect.bottom)
    }

    /// Returns true if bounds and radii values are finite and describe a [`RRect`] that matches
    /// the type. All [`RRect`] methods construct valid types, even if the input values are not
    /// valid.
    // Port of: src/core/SkRRect.cpp#L792-L873 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact comparisons, as in Skia
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn is_valid(&self) -> bool {
        let fradii = &self.radii;
        if !rrect_priv::are_rect_and_radii_valid(&self.rect, fradii) {
            return false;
        }

        let mut all_radii_zero = 0.0 == fradii[0].x && 0.0 == fradii[0].y;
        let mut all_corners_square = 0.0 == fradii[0].x || 0.0 == fradii[0].y;
        let mut all_radii_same = true;

        for i in 1..4 {
            if 0.0 != fradii[i].x || 0.0 != fradii[i].y {
                all_radii_zero = false;
            }

            if fradii[i].x != fradii[i - 1].x || fradii[i].y != fradii[i - 1].y {
                all_radii_same = false;
            }

            if 0.0 != fradii[i].x && 0.0 != fradii[i].y {
                all_corners_square = false;
            }
        }
        let patches_of_nine = radii_are_nine_patch(fradii);

        // `fType < 0 || fType > kLastType` cannot happen: `Type` is an enum.

        match self.type_ {
            Type::Empty => {
                if !self.rect.is_empty()
                    || !all_radii_zero
                    || !all_radii_same
                    || !all_corners_square
                {
                    return false;
                }
            }
            Type::Rect => {
                if self.rect.is_empty() || !all_radii_zero || !all_radii_same || !all_corners_square
                {
                    return false;
                }
            }
            Type::Oval => {
                if self.rect.is_empty() || all_radii_zero || !all_radii_same || all_corners_square {
                    return false;
                }

                let desired_width_val = rect_priv::half_width(&self.rect);
                let desired_height_val = rect_priv::half_height(&self.rect);

                for radius in fradii {
                    let x_matches =
                        <scalar as Scalar>::nearly_equal(radius.x, desired_width_val, None)
                            || almost_equals_4_ulps(radius.x, desired_width_val);
                    let y_matches =
                        <scalar as Scalar>::nearly_equal(radius.y, desired_height_val, None)
                            || almost_equals_4_ulps(radius.y, desired_height_val);
                    if !x_matches || !y_matches {
                        return false;
                    }
                }
            }
            Type::Simple => {
                if self.rect.is_empty() || all_radii_zero || !all_radii_same || all_corners_square {
                    return false;
                }
            }
            Type::NinePatch => {
                if self.rect.is_empty()
                    || all_radii_zero
                    || all_radii_same
                    || all_corners_square
                    || !patches_of_nine
                {
                    return false;
                }
            }
            Type::Complex => {
                if self.rect.is_empty()
                    || all_radii_zero
                    || all_radii_same
                    || all_corners_square
                    || patches_of_nine
                {
                    return false;
                }
            }
        }

        true
    }

    /// Writes [`RRect`] to `buffer`, replacing its contents with [`RRect::SIZE_IN_MEMORY`] bytes
    /// (the bounds and radii as native-endian floats; the derived type is not stored).
    // Port of: src/core/SkRRect.cpp#L704-L708 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    pub fn write_to_memory(&self, buffer: &mut Vec<u8>) {
        // Serialize only the rect and corners, but not the derived type tag.
        buffer.clear();
        let values = [
            self.rect.left,
            self.rect.top,
            self.rect.right,
            self.rect.bottom,
            self.radii[0].x,
            self.radii[0].y,
            self.radii[1].x,
            self.radii[1].y,
            self.radii[2].x,
            self.radii[2].y,
            self.radii[3].x,
            self.radii[3].y,
        ];
        for v in values {
            buffer.extend_from_slice(&v.to_ne_bytes());
        }
        debug_assert_eq!(buffer.len(), Self::SIZE_IN_MEMORY);
    }

    /// Reads [`RRect`] from `buffer`, validating the contents (via [`RRect::set_rect_radii`]).
    /// Returns [`RRect::SIZE_IN_MEMORY`], the bytes read, if `buffer` is at least that long.
    /// Otherwise, returns zero.
    // Port of: src/core/SkRRect.cpp#L715-L726 (chrome/m156)
    #[doc(alias = "readFromMemory")]
    pub fn read_from_memory(&mut self, buffer: &[u8]) -> usize {
        if buffer.len() < Self::SIZE_IN_MEMORY {
            return 0;
        }

        let mut v = [0.0_f32; 12];
        let (chunks, _) = buffer.as_chunks::<4>();
        for (value, chunk) in v.iter_mut().zip(chunks) {
            *value = scalar::from_ne_bytes(*chunk);
        }
        let rect = Rect::new(v[0], v[1], v[2], v[3]);
        let radii = [
            Point::new(v[4], v[5]),
            Point::new(v[6], v[7]),
            Point::new(v[8], v[9]),
            Point::new(v[10], v[11]),
        ];
        self.set_rect_radii(rect, &radii);
        Self::SIZE_IN_MEMORY
    }

    // Port of: src/core/SkRRect.cpp#L223-L236 (chrome/m156)
    fn initialize_rect(&mut self, rect: &Rect) -> bool {
        // Check this before sorting because sorting can hide nans.
        if !rect.is_finite() {
            *self = Self::new();
            return false;
        }
        self.rect = rect.sorted();
        if self.rect.is_empty() {
            self.radii = [Point::new(0.0, 0.0); 4];
            self.type_ = Type::Empty;
            return false;
        }
        true
    }

    // Port of: src/core/SkRRect.cpp#L251-L294 (chrome/m156)
    fn scale_radii(&mut self) -> bool {
        // Proportionally scale down all radii to fit. Find the minimum ratio
        // of a side and the radii on that side (for all four sides) and use
        // that to scale down _all_ the radii. This algorithm is from the
        // W3 spec (http://www.w3.org/TR/css3-background/) section 5.5 - Overlapping
        // Curves:
        // "Let f = min(Li/Si), where i is one of { top, right, bottom, left },
        //   Si is the sum of the two corresponding radii of the corners on side i,
        //   and Ltop = Lbottom = the width of the box,
        //   and Lleft = Lright = the height of the box.
        // If f < 1, then all corner radii are reduced by multiplying them by f."
        let mut scale: f64 = 1.0;

        // The sides of the rectangle may be larger than a float.
        let width = f64::from(self.rect.right) - f64::from(self.rect.left);
        let height = f64::from(self.rect.bottom) - f64::from(self.rect.top);
        let r = &mut self.radii;
        scale = compute_min_scale(f64::from(r[0].x), f64::from(r[1].x), width, scale);
        scale = compute_min_scale(f64::from(r[1].y), f64::from(r[2].y), height, scale);
        scale = compute_min_scale(f64::from(r[2].x), f64::from(r[3].x), width, scale);
        scale = compute_min_scale(f64::from(r[3].y), f64::from(r[0].y), height, scale);

        with_pair(r, 0, 1, true, flush_to_zero);
        with_pair(r, 1, 2, false, flush_to_zero);
        with_pair(r, 2, 3, true, flush_to_zero);
        with_pair(r, 3, 0, false, flush_to_zero);

        if scale < 1.0 {
            with_pair(r, 0, 1, true, |a, b| adjust_radii(width, scale, a, b));
            with_pair(r, 1, 2, false, |a, b| adjust_radii(height, scale, a, b));
            with_pair(r, 2, 3, true, |a, b| adjust_radii(width, scale, a, b));
            with_pair(r, 3, 0, false, |a, b| adjust_radii(height, scale, a, b));
        }

        // adjust radii may set x or y to zero; set companion to zero as well
        clamp_to_zero(&mut self.radii);

        // May be simple, oval, or complex, or become a rect/empty if the radii adjustment made
        // them 0
        self.compute_type();

        // TODO:  Why can't we assert this here?
        // SkASSERT(this->isValid());

        scale < 1.0
    }

    // This method determines if a point known to be inside the RRect's bounds is
    // inside all the corners.
    // Port of: src/core/SkRRect.cpp#L296-L349 (chrome/m156)
    fn check_corner_containment(&self, x: scalar, y: scalar) -> bool {
        let fradii = &self.radii;
        let frect = &self.rect;
        let canonical_pt: Point; // (x,y) translated to one of the quadrants
        let index: usize;

        if Type::Oval == self.get_type() {
            canonical_pt = Point::new(x - frect.center_x(), y - frect.center_y());
            index = UPPER_LEFT; // any corner will do in this case
        } else if x < frect.left + fradii[UPPER_LEFT].x && y < frect.top + fradii[UPPER_LEFT].y {
            // UL corner
            index = UPPER_LEFT;
            canonical_pt = Point::new(
                x - (frect.left + fradii[UPPER_LEFT].x),
                y - (frect.top + fradii[UPPER_LEFT].y),
            );
            debug_assert!(canonical_pt.x < 0.0 && canonical_pt.y < 0.0);
        } else if x < frect.left + fradii[LOWER_LEFT].x && y > frect.bottom - fradii[LOWER_LEFT].y {
            // LL corner
            index = LOWER_LEFT;
            canonical_pt = Point::new(
                x - (frect.left + fradii[LOWER_LEFT].x),
                y - (frect.bottom - fradii[LOWER_LEFT].y),
            );
            debug_assert!(canonical_pt.x < 0.0 && canonical_pt.y > 0.0);
        } else if x > frect.right - fradii[UPPER_RIGHT].x && y < frect.top + fradii[UPPER_RIGHT].y {
            // UR corner
            index = UPPER_RIGHT;
            canonical_pt = Point::new(
                x - (frect.right - fradii[UPPER_RIGHT].x),
                y - (frect.top + fradii[UPPER_RIGHT].y),
            );
            debug_assert!(canonical_pt.x > 0.0 && canonical_pt.y < 0.0);
        } else if x > frect.right - fradii[LOWER_RIGHT].x
            && y > frect.bottom - fradii[LOWER_RIGHT].y
        {
            // LR corner
            index = LOWER_RIGHT;
            canonical_pt = Point::new(
                x - (frect.right - fradii[LOWER_RIGHT].x),
                y - (frect.bottom - fradii[LOWER_RIGHT].y),
            );
            debug_assert!(canonical_pt.x > 0.0 && canonical_pt.y > 0.0);
        } else {
            // not in any of the corners
            return true;
        }

        // A point is in an ellipse (in standard position) if:
        //      x^2     y^2
        //     ----- + ----- <= 1
        //      a^2     b^2
        // or :
        //     b^2*x^2 + a^2*y^2 <= (ab)^2
        let dist = square(canonical_pt.x) * square(fradii[index].y)
            + square(canonical_pt.y) * square(fradii[index].x);
        dist <= square(fradii[index].x * fradii[index].y)
    }

    // There is a simplified version of this method in setRectXY
    // Port of: src/core/SkRRect.cpp#L428-L482 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparisons, as in Skia
    fn compute_type(&mut self) {
        if self.rect.is_empty() {
            debug_assert!(self.rect.is_sorted());
            for radius in &self.radii {
                debug_assert_eq!(*radius, Point::new(0.0, 0.0));
            }
            self.type_ = Type::Empty;
            debug_assert!(self.is_valid());
            return;
        }

        let mut all_radii_equal = true; // are all x radii equal and all y radii?
        let mut all_corners_square = 0.0 == self.radii[0].x || 0.0 == self.radii[0].y;

        for i in 1..4 {
            if 0.0 != self.radii[i].x && 0.0 != self.radii[i].y {
                // if either radius is zero the corner is square so both have to
                // be non-zero to have a rounded corner
                all_corners_square = false;
            }
            if self.radii[i].x != self.radii[i - 1].x || self.radii[i].y != self.radii[i - 1].y {
                all_radii_equal = false;
            }
        }

        if all_corners_square {
            self.type_ = Type::Rect;
            debug_assert!(self.is_valid());
            return;
        }

        if all_radii_equal {
            if self.radii[0].x >= (self.rect.width() / 2.0)
                && self.radii[0].y >= (self.rect.height() / 2.0)
            {
                self.type_ = Type::Oval;
                for i in 0..4 {
                    self.radii[i].set(
                        rect_priv::half_width(&self.rect),
                        rect_priv::half_height(&self.rect),
                    );
                }
            } else {
                self.type_ = Type::Simple;
            }
            debug_assert!(self.is_valid());
            return;
        }

        if radii_are_nine_patch(&self.radii) {
            self.type_ = Type::NinePatch;
        } else {
            self.type_ = Type::Complex;
        }

        if !self.is_valid() {
            let r = self.rect;
            self.set_rect(r);
            debug_assert!(self.is_valid());
        }
    }
}

// Port of: src/core/SkRRect.cpp#L95-L113 (chrome/m156)
fn clamp_to_zero(radii: &mut [Vector; 4]) -> bool {
    let mut all_corners_square = true;

    // Clamp negative radii to zero
    for radius in radii.iter_mut() {
        if radius.x <= 0.0 || radius.y <= 0.0 {
            // In this case we are being a little fast & loose. Since one of
            // the radii is 0 the corner is square. However, the other radii
            // could still be non-zero and play in the global scale factor
            // computation.
            radius.x = 0.0;
            radius.y = 0.0;
        } else {
            all_corners_square = false;
        }
    }

    all_corners_square
}

// Port of: src/core/SkRRect.cpp#L115-L120 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
fn radii_are_nine_patch(radii: &[Vector; 4]) -> bool {
    radii[UPPER_LEFT].x == radii[LOWER_LEFT].x
        && radii[UPPER_LEFT].y == radii[UPPER_RIGHT].y
        && radii[UPPER_RIGHT].x == radii[LOWER_RIGHT].x
        && radii[LOWER_LEFT].y == radii[LOWER_RIGHT].y
}

// These parameters intentionally double. Apropos crbug.com/463920, if one of the
// radii is huge while the other is small, single precision math can completely
// miss the fact that a scale is required.
// Port of: src/core/SkRRect.cpp#L188-L196 (chrome/m156)
fn compute_min_scale(rad1: f64, rad2: f64, limit: f64, cur_min: f64) -> f64 {
    if (rad1 + rad2) > limit {
        let ratio = limit / (rad1 + rad2);
        // std::min(curMin, ratio)
        return if ratio < cur_min { ratio } else { cur_min };
    }
    cur_min
}

// If we can't distinguish one of the radii relative to the other, force it to zero so it
// doesn't confuse us later. See crbug.com/850350
// Port of: src/core/SkRRect.cpp#L238-L249 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
fn flush_to_zero(a: &mut scalar, b: &mut scalar) {
    debug_assert!(*a >= 0.0);
    debug_assert!(*b >= 0.0);
    if *a + *b == *a {
        *b = 0.0;
    } else if *a + *b == *b {
        *a = 0.0;
    }
}

/// Applies `f` to the `x` (or `y`) radii of corners `i` and `j` (which must differ) and stores
/// the results back. Stands in for the C++ pointers to two members of `fRadii`.
#[allow(clippy::many_single_char_names)] // a, b: the C++ pointer pair; i, j: corner indices
fn with_pair(
    radii: &mut [Vector; 4],
    i: usize,
    j: usize,
    is_x: bool,
    f: impl FnOnce(&mut scalar, &mut scalar),
) {
    debug_assert_ne!(i, j);
    let (mut a, mut b) = if is_x {
        (radii[i].x, radii[j].x)
    } else {
        (radii[i].y, radii[j].y)
    };
    f(&mut a, &mut b);
    if is_x {
        radii[i].x = a;
        radii[j].x = b;
    } else {
        radii[i].y = a;
        radii[j].y = b;
    }
}

// This code assumes that a and b fit in a float, and therefore the resulting smaller value
// of a and b will fit in a float. The side of the rectangle may be larger than a float.
// Scale must be less than or equal to the ratio limit / (*a + *b).
// This code assumes that NaN and Inf are never passed in.
// Port of: src/core/SkScaleToSides.h#L20-L63 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the (float) casts in SkScaleToSides
fn adjust_radii(limit: f64, scale: f64, a: &mut scalar, b: &mut scalar) {
    debug_assert!(scale < 1.0 && scale > 0.0);

    *a = (f64::from(*a) * scale) as f32;
    *b = (f64::from(*b) * scale) as f32;

    if f64::from(*a + *b) > limit {
        // Force min_radius to be the smaller of the two.
        let a_is_max = *a > *b;
        let (min_radius, max_radius) = if a_is_max {
            (&mut *b, &mut *a)
        } else {
            (&mut *a, &mut *b)
        };

        // newMinRadius must be float in order to give the actual value of the radius.
        // The newMinRadius will always be smaller than limit. The largest that minRadius can be
        // is 1/2 the ratio of minRadius : (minRadius + maxRadius), therefore in the resulting
        // division, minRadius can be no larger than 1/2 limit + ULP.
        let new_min_radius: f32 = *min_radius;

        let mut new_max_radius = (limit - f64::from(new_min_radius)) as f32;

        // Reduce newMaxRadius an ulp at a time until it fits. This usually never happens,
        // but if it does it could be 1 or 2 times. In certain pathological cases it could be
        // more. Max iterations seen so far is 17.
        while f64::from(new_max_radius + new_min_radius) > limit {
            new_max_radius = next_after_toward_zero(new_max_radius);
        }
        *max_radius = new_max_radius;
    }

    debug_assert!(*a >= 0.0 && *b >= 0.0);
    debug_assert!(f64::from(*a + *b) <= limit);
}

/// `nextafterf(x, 0.0f)`.
#[allow(clippy::float_cmp)] // exact zero test, as in nextafterf
fn next_after_toward_zero(x: f32) -> f32 {
    if x.is_nan() || x == 0.0 {
        return x;
    }
    // Moving toward zero decreases the magnitude, i.e. the raw bits, for either sign.
    f32::from_bits(x.to_bits() - 1)
}

/// `SkFloatingPoint<float, 4>(a).AlmostEquals(SkFloatingPoint<float, 4>(b))`: true if `a` and `b`
/// are at most 4 ULPs apart (false if either is NaN).
// Port of: src/utils/SkFloatUtils.h#L58-L170 (chrome/m156)
fn almost_equals_4_ulps(a: f32, b: f32) -> bool {
    const SIGN_BIT_MASK: u32 = 1 << 31;
    const FRACTION_BIT_MASK: u32 = !0u32 >> 9;
    const EXPONENT_BIT_MASK: u32 = !(SIGN_BIT_MASK | FRACTION_BIT_MASK);
    const MAX_ULPS: u32 = 4;

    fn is_nan_bits(bits: u32) -> bool {
        (EXPONENT_BIT_MASK & bits) == EXPONENT_BIT_MASK && (FRACTION_BIT_MASK & bits) != 0
    }
    fn sign_and_magnitude_to_biased(sam: u32) -> u32 {
        if SIGN_BIT_MASK & sam != 0 {
            (!sam).wrapping_add(1)
        } else {
            SIGN_BIT_MASK | sam
        }
    }

    let (a_bits, b_bits) = (a.to_bits(), b.to_bits());
    if is_nan_bits(a_bits) || is_nan_bits(b_bits) {
        return false;
    }
    let biased1 = sign_and_magnitude_to_biased(a_bits);
    let biased2 = sign_and_magnitude_to_biased(b_bits);
    let dist = biased1.abs_diff(biased2);
    dist <= MAX_ULPS
}

fn square(x: scalar) -> scalar {
    x * x
}

/// Private helpers of `SkRRect` (`SkRRectPriv.h`), used by Skia's own code and tests.
#[doc(hidden)]
pub mod rrect_priv {
    use super::{
        Corner, LOWER_LEFT, LOWER_RIGHT, RRect, UPPER_LEFT, UPPER_RIGHT, almost_equals_4_ulps,
        std_max,
    };
    use crate::point::{Point, Vector};
    use crate::rect::Rect;
    use crate::scalar::{SCALAR_NEARLY_ZERO, SCALAR_ROOT_2_OVER_2, Scalar, scalar};

    /// True if `rr` is an oval whose radii are equal.
    // Port of: src/core/SkRRectPriv.h#L19-L21 (chrome/m156)
    #[doc(alias = "IsCircle")]
    #[must_use]
    pub fn is_circle(rr: &RRect) -> bool {
        rr.is_oval() && <scalar as Scalar>::nearly_equal(rr.radii[0].x, rr.radii[0].y, None)
    }

    /// The radii of the upper-left corner; `rr` must not be complex.
    // Port of: src/core/SkRRectPriv.h#L23-L26 (chrome/m156)
    #[doc(alias = "GetSimpleRadii")]
    #[must_use]
    pub fn get_simple_radii(rr: &RRect) -> Vector {
        debug_assert!(!rr.is_complex());
        rr.radii[0]
    }

    /// True if `rr` is simple and its radii are equal.
    // Port of: src/core/SkRRectPriv.h#L28-L30 (chrome/m156)
    #[doc(alias = "IsSimpleCircular")]
    #[must_use]
    pub fn is_simple_circular(rr: &RRect) -> bool {
        rr.is_simple() && <scalar as Scalar>::nearly_equal(rr.radii[0].x, rr.radii[0].y, None)
    }

    /// Looser version of [`is_simple_circular`], where the x & y values of the radii only have to
    /// be nearly equal instead of strictly equal. `tolerance` defaults to
    /// `SK_ScalarNearlyZero`.
    // Port of: src/core/SkRRect.cpp#L351-L360 (chrome/m156)
    #[doc(alias = "IsNearlySimpleCircular")]
    #[must_use]
    pub fn is_nearly_simple_circular(rr: &RRect, tolerance: impl Into<Option<scalar>>) -> bool {
        let tolerance = tolerance.into().unwrap_or(SCALAR_NEARLY_ZERO);
        let simple_radius = rr.radii[0].x;
        let ne = |v: scalar| <scalar as Scalar>::nearly_equal(simple_radius, v, tolerance);
        ne(rr.radii[0].y)
            && ne(rr.radii[1].x)
            && ne(rr.radii[1].y)
            && ne(rr.radii[2].x)
            && ne(rr.radii[2].y)
            && ne(rr.radii[3].x)
            && ne(rr.radii[3].y)
    }

    /// True if all corners share the same radii (rect, circle or simple circular).
    // Port of: src/core/SkRRectPriv.h#L40-L42 (chrome/m156)
    #[doc(alias = "EqualRadii")]
    #[must_use]
    pub fn equal_radii(rr: &RRect) -> bool {
        rr.is_rect() || is_circle(rr) || is_simple_circular(rr)
    }

    /// The four corner radii, in [`Corner`] order.
    // Port of: src/core/SkRRectPriv.h#L44 (chrome/m156)
    #[doc(alias = "GetRadiiArray")]
    #[must_use]
    pub fn get_radii_array(rr: &RRect) -> &[Vector; 4] {
        &rr.radii
    }

    /// True if every corner's x and y radii are within `tolerance` (default
    /// `SK_ScalarNearlyZero`) of each other.
    // Port of: src/core/SkRRect.cpp#L362-L367 (chrome/m156)
    #[doc(alias = "AllCornersCircular")]
    #[must_use]
    pub fn all_corners_circular(rr: &RRect, tolerance: impl Into<Option<scalar>>) -> bool {
        let tolerance = tolerance.into().unwrap_or(SCALAR_NEARLY_ZERO);
        let ne = |p: Vector| <scalar as Scalar>::nearly_equal(p.x, p.y, tolerance);
        ne(rr.radii[0]) && ne(rr.radii[1]) && ne(rr.radii[2]) && ne(rr.radii[3])
    }

    /// Prefer this over [`all_corners_circular`], which compares radii by absolute difference,
    /// which is a less stable decision as scale changes.
    // Port of: src/core/SkRRect.cpp#L382-L387 (chrome/m156)
    #[doc(alias = "AllCornersRelativelyCircular")]
    #[must_use]
    pub fn all_corners_relatively_circular(
        rr: &RRect,
        tolerance: impl Into<Option<scalar>>,
    ) -> bool {
        let tolerance = tolerance.into().unwrap_or(SCALAR_NEARLY_ZERO);
        is_relatively_circular(rr.radii[0].x, rr.radii[0].y, tolerance)
            && is_relatively_circular(rr.radii[1].x, rr.radii[1].y, tolerance)
            && is_relatively_circular(rr.radii[2].x, rr.radii[2].y, tolerance)
            && is_relatively_circular(rr.radii[3].x, rr.radii[3].y, tolerance)
    }

    /// The same test used in [`all_corners_relatively_circular`], but for provided radii.
    // Port of: src/core/SkRRect.cpp#L369-L380 (chrome/m156)
    #[doc(alias = "IsRelativelyCircular")]
    #[must_use]
    pub fn is_relatively_circular(
        rx: scalar,
        ry: scalar,
        tolerance: impl Into<Option<scalar>>,
    ) -> bool {
        let tolerance = tolerance.into().unwrap_or(SCALAR_NEARLY_ZERO);
        // The ellipse is considered relatively circular if either `rx/ry` or `ry/rx` is within
        // `tolerance` of 1.0, but this is equivalent to comparing the absolute difference between
        // `rx` and `ry` to `tolerance` multiplied by the largest radii. We also consider the case
        // where both `rx` and `ry` are less than tolerance to be "circular" with a radius of 0.
        // (The `SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER` variant is not built.)
        (rx <= tolerance && ry <= tolerance) || (rx - ry).abs() <= tolerance * std_max(rx, ry)
    }

    // Port of: src/core/SkRRect.cpp#L760-L790 (chrome/m156)
    fn are_radii_predicates_valid(
        radius_from_min: scalar,
        radius_from_max: scalar,
        min_coord: scalar,
        max_coord: scalar,
    ) -> bool {
        if min_coord > max_coord || radius_from_min < 0.0 || radius_from_max < 0.0 {
            return false;
        }

        let limit = max_coord - min_coord;
        let sum = radius_from_min + radius_from_max;
        let pt_from_min = min_coord + radius_from_min;
        let pt_from_max = max_coord - radius_from_max;

        // Accept either floats that are within an absolute tolerance of each other
        // (for small numbers) or within a few ULPs (Units in the Last Place, i.e.
        // the step between adjacent representable floats, for large numbers) to be
        // robust against floating-point imprecision when translated.
        let sum_valid = sum <= limit
            || <scalar as Scalar>::nearly_equal(sum, limit, None)
            || almost_equals_4_ulps(sum, limit);
        let pts_valid = pt_from_min <= pt_from_max
            || <scalar as Scalar>::nearly_equal(pt_from_min, pt_from_max, None)
            || almost_equals_4_ulps(pt_from_min, pt_from_max);

        if !sum_valid || !pts_valid {
            return false;
        }

        min_coord <= pt_from_max && pt_from_min <= max_coord
    }

    /// True if `rect` is finite and sorted, and `radii` are non-negative and fit within `rect`.
    // Port of: src/core/SkRRect.cpp#L875-L895 (chrome/m156)
    #[doc(alias = "AreRectAndRadiiValid")]
    #[must_use]
    pub fn are_rect_and_radii_valid(rect: &Rect, radii: &[Vector; 4]) -> bool {
        if !rect.is_finite() || !rect.is_sorted() {
            return false;
        }
        are_radii_predicates_valid(
            radii[UPPER_LEFT].x,
            radii[UPPER_RIGHT].x,
            rect.left,
            rect.right,
        ) && are_radii_predicates_valid(
            radii[LOWER_LEFT].x,
            radii[LOWER_RIGHT].x,
            rect.left,
            rect.right,
        ) && are_radii_predicates_valid(
            radii[UPPER_LEFT].y,
            radii[LOWER_LEFT].y,
            rect.top,
            rect.bottom,
        ) && are_radii_predicates_valid(
            radii[UPPER_RIGHT].y,
            radii[LOWER_RIGHT].y,
            rect.top,
            rect.bottom,
        )
    }

    /// Compute an approximate largest inscribed bounding box of the rounded rect. For empty,
    /// rect, oval, and simple types this will be the largest inscribed rectangle. Otherwise it may
    /// not be the global maximum, but will be non-empty, touch at least one edge and be contained
    /// in the round rect.
    // Port of: src/core/SkRRect.cpp#L899-L959 (chrome/m156)
    #[doc(alias = "InnerBounds")]
    #[must_use]
    pub fn inner_bounds(rr: &RRect) -> Rect {
        if rr.is_empty() || rr.is_rect() {
            return *rr.rect();
        }

        // We start with the outer bounds of the round rect and consider three subsets and take the
        // one with maximum area. The first two are the horizontal and vertical rects inset from
        // the corners, the third is the rect inscribed at the corner curves' maximal point. This
        // forms the exact solution when all corners have the same radii (the radii do not have to
        // be circular).
        let mut inner_bounds = *rr.bounds();
        let tl = rr.radii(Corner::UpperLeft);
        let tr = rr.radii(Corner::UpperRight);
        let bl = rr.radii(Corner::LowerLeft);
        let br = rr.radii(Corner::LowerRight);

        // Select maximum inset per edge, which may move an adjacent corner of the inscribed
        // rectangle off of the rounded-rect path, but that is acceptable given that the general
        // equation for inscribed area is non-trivial to evaluate.
        let left_shift = std_max(tl.x, bl.x);
        let top_shift = std_max(tl.y, tr.y);
        let right_shift = std_max(tr.x, br.x);
        let bottom_shift = std_max(bl.y, br.y);

        let dw = left_shift + right_shift;
        let dh = top_shift + bottom_shift;

        // Area removed by shifting left/right
        let horiz_area = (inner_bounds.width() - dw) * inner_bounds.height();
        // And by shifting top/bottom
        let vert_area = (inner_bounds.height() - dh) * inner_bounds.width();
        // And by shifting all edges: just considering a corner ellipse, the maximum inscribed rect
        // has a corner at sqrt(2)/2 * (rX, rY), so scale all corner shifts by (1 - sqrt(2)/2) to
        // get the safe shift per edge (since the shifts already are the max radius for that edge).
        // - We actually scale by a value slightly increased to make it so that the shifted corners
        //   are safely inside the curves, otherwise numerical stability can cause it to fail
        //   contains().
        let k_scale: scalar = (1.0 - SCALAR_ROOT_2_OVER_2) + 1e-5_f32;
        let inner_area =
            (inner_bounds.width() - k_scale * dw) * (inner_bounds.height() - k_scale * dh);

        if horiz_area > vert_area && horiz_area > inner_area {
            // Cut off corners by insetting left and right
            inner_bounds.left += left_shift;
            inner_bounds.right -= right_shift;
        } else if vert_area > inner_area {
            // Cut off corners by insetting top and bottom
            inner_bounds.top += top_shift;
            inner_bounds.bottom -= bottom_shift;
        } else if inner_area > 0.0 {
            // Inset on all sides, scaled to touch
            inner_bounds.left += k_scale * left_shift;
            inner_bounds.right -= k_scale * right_shift;
            inner_bounds.top += k_scale * top_shift;
            inner_bounds.bottom -= k_scale * bottom_shift;
        } else {
            // Inner region would collapse to empty
            return Rect::new_empty();
        }

        debug_assert!(inner_bounds.is_sorted() && !inner_bounds.is_empty());
        inner_bounds
    }

    /// Attempt to compute the intersection of two round rects. The intersection is not
    /// necessarily a round rect. This returns intersections only when the shape is representable
    /// as a new round rect (or rect). Empty is returned if `a` and `b` do not intersect or if the
    /// intersection is too complicated. This is conservative, it may not always detect that an
    /// intersection could be represented as a round rect. However, when it does return a round
    /// rect that intersection will be exact (i.e. it is NOT just a subset of the actual
    /// intersection).
    // Port of: src/core/SkRRect.cpp#L961-L1072 (chrome/m156)
    #[doc(alias = "ConservativeIntersect")]
    #[must_use]
    pub fn conservative_intersect(a: &RRect, b: &RRect) -> RRect {
        const CORNERS: [Corner; 4] = [
            Corner::UpperLeft,
            Corner::UpperRight,
            Corner::LowerRight,
            Corner::LowerLeft,
        ];
        // Returns the coordinate of the rect matching the corner enum.
        let get_corner = |r: &Rect, corner: Corner| -> Point {
            match corner {
                Corner::UpperLeft => Point::new(r.left, r.top),
                Corner::UpperRight => Point::new(r.right, r.top),
                Corner::LowerLeft => Point::new(r.left, r.bottom),
                Corner::LowerRight => Point::new(r.right, r.bottom),
            }
        };
        // Returns true if shape A's extreme point is contained within shape B's extreme point,
        // relative to the 'corner' location. If the two shapes' corners have the same ellipse
        // radii, this is sufficient for A's ellipse arc to be contained by B's ellipse arc.
        let inside_corner = |corner: Corner, a: &Point, b: &Point| -> bool {
            match corner {
                Corner::UpperLeft => a.x >= b.x && a.y >= b.y,
                Corner::UpperRight => a.x <= b.x && a.y >= b.y,
                Corner::LowerRight => a.x <= b.x && a.y <= b.y,
                Corner::LowerLeft => a.x >= b.x && a.y <= b.y,
            }
        };

        // Returns the intersection corner radii, or `None` if the corner cannot be represented.
        let get_intersection_radii = |r: &Rect, corner: Corner| -> Option<Vector> {
            let test = get_corner(r, corner);
            let a_corner = get_corner(a.rect(), corner);
            let b_corner = get_corner(b.rect(), corner);

            if test == a_corner && test == b_corner {
                // The round rects share a corner anchor, so pick A or B such that its X and Y
                // radii are both larger than the other rrect's, or return false if neither A or B
                // has the max corner radii (this is more permissive than the single corner tests
                // below).
                let a_radii = a.radii(corner);
                let b_radii = b.radii(corner);
                if a_radii.x >= b_radii.x && a_radii.y >= b_radii.y {
                    Some(a_radii)
                } else if b_radii.x >= a_radii.x && b_radii.y >= a_radii.y {
                    Some(b_radii)
                } else {
                    None
                }
            } else if test == a_corner {
                // Test that A's ellipse is contained by B. This is a non-trivial function to
                // evaluate so we resrict it to when the corners have the same radii. If not, we
                // use the more conservative test that the extreme point of A's bounding box is
                // contained in B.
                let radii = a.radii(corner);
                let ok = if radii == b.radii(corner) {
                    inside_corner(corner, &a_corner, &b_corner) // A inside B
                } else {
                    b.check_corner_containment(a_corner.x, a_corner.y)
                };
                ok.then_some(radii)
            } else if test == b_corner {
                // Mirror of the above
                let radii = b.radii(corner);
                let ok = if radii == a.radii(corner) {
                    inside_corner(corner, &b_corner, &a_corner) // B inside A
                } else {
                    a.check_corner_containment(b_corner.x, b_corner.y)
                };
                ok.then_some(radii)
            } else {
                // This is a corner formed by two straight edges of A and B, so confirm that it is
                // contained in both (if not, then the intersection can't be a round rect).
                (a.check_corner_containment(test.x, test.y)
                    && b.check_corner_containment(test.x, test.y))
                .then_some(Point::new(0.0, 0.0))
            }
        };

        // We fill in the SkRRect directly. Since the rect and radii are either 0s or determined by
        // valid existing SkRRects, we know we are finite.
        let mut intersection = RRect::new();
        if !intersection.rect.intersect2(a.rect(), b.rect()) {
            // Definitely no intersection
            return RRect::new_empty();
        }

        // By definition, edges is contained in the bounds of 'a' and 'b', but now we need to
        // consider the corners. If the bound's corner point is in both rrects, the corner radii
        // will be 0s. If the bound's corner point matches a's edges and is inside 'b', we use a's
        // radii. Same for b's radii. If any corner fails these conditions, we reject the
        // intersection as an rrect. If after determining radii for all 4 corners, they would
        // overlap, we also reject the intersection shape.
        for c in CORNERS {
            match get_intersection_radii(&intersection.rect, c) {
                Some(radii) => intersection.radii[c as usize] = radii,
                None => return RRect::new_empty(), // Resulting intersection is not a rrect
            }
        }

        // Check for radius overlap along the four edges, since the earlier evaluation was only a
        // one-sided corner check. If they aren't valid, a corner's radii doesn't fit within the
        // rect. If the radii are scaled, the combination of radii from two adjacent corners
        // doesn't fit. Normally for a regularly constructed SkRRect, we want this scaling, but in
        // this case it means the intersection shape is definitively not a round rect.
        if !are_rect_and_radii_valid(&intersection.rect, &intersection.radii)
            || intersection.scale_radii()
        {
            return RRect::new_empty();
        }

        // The intersection is an rrect of the given radii. Potentially all 4 corners could have
        // been simplified to (0,0) radii, making the intersection a rectangle.
        intersection.compute_type();
        intersection
    }
}
