// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/Rect.h

//! SIMD rect used by Graphite's geometry code (`skgpu::graphite::Rect`).
//!
//! Values are stored internally as `[left, top, -right, -bot]`, so that intersection tests and
//! unions are lane-wise `min`/`max` and comparisons.

use skia_rust_core::floating_point::{FLOAT_INFINITY, FLOAT_NEGATIVE_INFINITY};
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_simd::vx::{self, Float2, Float4};

/// `NegateBotRight(vals)`: returns `[vals.xy, -vals.zw]` by flipping the sign bits of z and w.
// Port of: src/gpu/graphite/geom/Rect.h#L183-L186 (chrome/m156)
fn negate_bot_right(vals: Float4) -> Float4 {
    Float4::new(
        vals[0],
        vals[1],
        f32::from_bits(vals[2].to_bits() ^ (1u32 << 31)),
        f32::from_bits(vals[3].to_bits() ^ (1u32 << 31)),
    )
}

/// `float2` negation (lane-wise `-x`).
fn neg2(v: Float2) -> Float2 {
    -v
}

/// `float4(xy.xyxy)`: `[x, y, x, y]`.
fn xyxy(v: Float2) -> Float4 {
    Float4::new(v.x(), v.y(), v.x(), v.y())
}

/// `all(a == b)` for `float4` lanes.
fn all_eq4(a: Float4, b: Float4) -> bool {
    vx::all(a.eq_mask(b))
}

/// `all(a < b)` for `float4` lanes.
fn all_lt4(a: Float4, b: Float4) -> bool {
    vx::all(a.lt_mask(b))
}

/// `all(a <= b)` for `float4` lanes.
fn all_le4(a: Float4, b: Float4) -> bool {
    vx::all(a.le_mask(b))
}

// Port of: src/gpu/graphite/geom/Rect.h#L28-L186 (chrome/m156)
/// SIMD rect implementation. Values are stored internally in the form: `[left, top, -right, -bot]`.
///
/// Some operations (e.g., intersect, inset) may return a negative or empty rect (negative meaning,
/// left >= right or top >= bot).
///
/// Operations on a rect that is either negative or empty, while well-defined, might not give the
/// intended result. It is the caller's responsibility to check `is_empty_negative_or_nan()` if
/// needed.
#[doc(alias = "skgpu::graphite::Rect")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    vals: Float4, // [left, top, -right, -bottom]
}

impl Rect {
    /// `Rect(l, t, r, b)`.
    #[must_use]
    pub fn new(l: f32, t: f32, r: f32, b: f32) -> Self {
        Self {
            vals: negate_bot_right(Float4::new(l, t, r, b)),
        }
    }

    /// `Rect(float2 topLeft, float2 botRight)`.
    #[must_use]
    pub fn from_corners(top_left: Float2, bot_right: Float2) -> Self {
        Self {
            vals: Float4::from_xy_zw(top_left, neg2(bot_right)),
        }
    }

    /// `Rect(SkRect)`.
    #[must_use]
    pub fn from_sk_rect(r: &SkRect) -> Self {
        let [l, t, r, b] = r.as_scalars();
        Self {
            vals: negate_bot_right(Float4::new(l, t, r, b)),
        }
    }

    /// `Rect(SkIRect)`.
    #[must_use]
    pub fn from_sk_irect(r: &IRect) -> Self {
        let v = Float4::new(
            r.left() as f32,
            r.top() as f32,
            r.right() as f32,
            r.bottom() as f32,
        );
        Self {
            vals: negate_bot_right(v),
        }
    }

    /// `Rect::LTRB(float4 ltrb)`.
    #[must_use]
    pub fn ltrb_vals(ltrb: Float4) -> Self {
        Self {
            vals: negate_bot_right(ltrb),
        }
    }

    /// `Rect::XYWH(x, y, w, h)`.
    #[must_use]
    pub fn xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new(x, y, x + w, y + h)
    }

    /// `Rect::XYWH(float2 topLeft, float2 size)`.
    #[must_use]
    pub fn xywh_vec(top_left: Float2, size: Float2) -> Self {
        Self::from_corners(top_left, top_left + size)
    }

    /// `Rect::WH(w, h)`.
    #[must_use]
    pub fn wh(w: f32, h: f32) -> Self {
        Self::new(0.0, 0.0, w, h)
    }

    /// `Rect::WH(float2 size)`.
    #[must_use]
    pub fn wh_vec(size: Float2) -> Self {
        Self::from_corners(Float2::from(0.0), size)
    }

    /// `Rect::Point(float2 p)`.
    #[must_use]
    pub fn point(p: Float2) -> Self {
        Self::from_corners(p, p)
    }

    /// `Rect::FromVals(float4 vals)`: `vals.zw` must already be negated.
    #[must_use]
    pub fn from_vals(vals: Float4) -> Self {
        Self { vals }
    }

    /// Constructs a Rect with ltrb = `[-inf, -inf, inf, inf]`, useful for accumulating
    /// intersections.
    #[must_use]
    pub fn infinite() -> Self {
        Self::from_vals(Float4::from(FLOAT_NEGATIVE_INFINITY))
    }

    /// Constructs a negative Rect with ltrb = `[inf, inf, -inf, -inf]`, useful for accumulating
    /// unions.
    #[must_use]
    pub fn infinite_inverted() -> Self {
        Self::from_vals(Float4::from(FLOAT_INFINITY))
    }

    /// `nearlyEquals(r, epsilon)`.
    #[must_use]
    pub fn nearly_equals(&self, r: &Rect, epsilon: f32) -> bool {
        // Port of: src/gpu/graphite/geom/Rect.h#L53-L55 (chrome/m156)
        let d = self.vals - r.vals;
        let d = vx::abs(d);
        vx::all(d.le_mask(Float4::from(epsilon)))
    }

    /// `vals()`: `[left, top, -right, -bot]`.
    #[must_use]
    pub fn vals(&self) -> Float4 {
        self.vals
    }

    /// Mutable `vals()`.
    pub fn vals_mut(&mut self) -> &mut Float4 {
        &mut self.vals
    }

    /// `x()`.
    #[must_use]
    pub fn x(&self) -> f32 {
        self.vals.x()
    }

    /// `y()`.
    #[must_use]
    pub fn y(&self) -> f32 {
        self.vals.y()
    }

    /// `left()`.
    #[must_use]
    pub fn left(&self) -> f32 {
        self.vals.x()
    }

    /// `top()`.
    #[must_use]
    pub fn top(&self) -> f32 {
        self.vals.y()
    }

    /// `right()`.
    #[must_use]
    pub fn right(&self) -> f32 {
        -self.vals.z()
    }

    /// `bot()`.
    #[must_use]
    pub fn bot(&self) -> f32 {
        -self.vals.w()
    }

    /// `topLeft()`.
    #[must_use]
    pub fn top_left(&self) -> Float2 {
        self.vals.xy()
    }

    /// `botRight()`.
    #[must_use]
    pub fn bot_right(&self) -> Float2 {
        neg2(self.vals.zw())
    }

    /// `ltrb()`.
    #[must_use]
    pub fn ltrb(&self) -> Float4 {
        negate_bot_right(self.vals)
    }

    /// `setLeft(left)`.
    pub fn set_left(&mut self, left: f32) {
        *self.vals.x_mut() = left;
    }

    /// `setTop(top)`.
    pub fn set_top(&mut self, top: f32) {
        *self.vals.y_mut() = top;
    }

    /// `setRight(right)`.
    pub fn set_right(&mut self, right: f32) {
        *self.vals.z_mut() = -right;
    }

    /// `setBot(bot)`.
    pub fn set_bot(&mut self, bot: f32) {
        *self.vals.w_mut() = -bot;
    }

    /// `setTopLeft(topLeft)`.
    pub fn set_top_left(&mut self, top_left: Float2) {
        self.vals.set_xy(top_left);
    }

    /// `setBotRight(botRight)`.
    pub fn set_bot_right(&mut self, bot_right: Float2) {
        self.vals.set_zw(neg2(bot_right));
    }

    /// `asSkRect()`.
    #[must_use]
    pub fn as_sk_rect(&self) -> SkRect {
        let v = self.ltrb();
        SkRect::new(v[0], v[1], v[2], v[3])
    }

    /// `asSkIRect()`.
    #[must_use]
    pub fn as_sk_irect(&self) -> IRect {
        let v = self.ltrb();
        // `skvx::cast<int>`: truncating conversion of each lane.
        IRect::new(v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32)
    }

    /// `isEmptyNegativeOrNaN()`.
    #[must_use]
    pub fn is_empty_negative_or_nan(&self) -> bool {
        // !([l-r, r-b] < 0) == ([w, h] <= 0)
        // Use "!(-size < 0)" in order to detect NaN.
        let s = self.vals.xy() + self.vals.zw();
        !vx::all(s.lt_mask(Float2::from(0.0)))
    }

    /// `size()`: `[w, h]`.
    #[must_use]
    pub fn size(&self) -> Float2 {
        neg2(self.vals.xy() + self.vals.zw())
    }

    /// `center()`.
    #[must_use]
    pub fn center(&self) -> Float2 {
        let p = self.vals * Float4::new(0.5, 0.5, -0.5, -0.5); // == [l, t, r, b] * .5
        p.xy() + p.zw() // == [(l + r)/2, (t + b)/2]
    }

    /// `area()`.
    #[must_use]
    pub fn area(&self) -> f32 {
        let negative_size = self.vals.xy() + self.vals.zw(); // == [l-r, t-b] == [-w, -h]
        negative_size.x() * negative_size.y()
    }

    /// `intersects(ComplementRect)`.
    #[must_use]
    pub fn intersects_complement(&self, comp: ComplementRect) -> bool {
        all_lt4(self.vals, comp.vals)
    }

    /// `intersects(Rect)` through a `ComplementRect` built from `rect`.
    #[must_use]
    pub fn intersects(&self, rect: Rect) -> bool {
        self.intersects_complement(ComplementRect::new(rect))
    }

    /// `contains(Rect)`.
    #[must_use]
    pub fn contains(&self, rect: Rect) -> bool {
        all_le4(self.vals, rect.vals)
    }

    /// `makeRoundIn()`.
    #[must_use]
    pub fn make_round_in(&self) -> Rect {
        Rect::from_vals(vx::ceil(self.vals))
    }

    /// `makeRoundOut()`.
    #[must_use]
    pub fn make_round_out(&self) -> Rect {
        Rect::from_vals(vx::floor(self.vals))
    }

    /// `makeRound()`.
    #[must_use]
    pub fn make_round(&self) -> Rect {
        // To match SkRect::round(), which is implemented as floor(x+.5), we don't use std::round.
        // But this means we have to undo the negative R and B components before flooring.
        Rect::ltrb_vals(vx::floor(self.ltrb() + Float4::from(0.5)))
    }

    /// `makeInset(float)`.
    #[must_use]
    pub fn make_inset(&self, inset: f32) -> Rect {
        Rect::from_vals(self.vals + Float4::from(inset))
    }

    /// `makeInset(float2)`.
    #[must_use]
    pub fn make_inset_vec(&self, inset: Float2) -> Rect {
        Rect::from_vals(self.vals + xyxy(inset))
    }

    /// `makeOutset(float)`.
    #[must_use]
    pub fn make_outset(&self, outset: f32) -> Rect {
        Rect::from_vals(self.vals - Float4::from(outset))
    }

    /// `makeOutset(float2)`.
    #[must_use]
    pub fn make_outset_vec(&self, outset: Float2) -> Rect {
        Rect::from_vals(self.vals - xyxy(outset))
    }

    /// `makeOffset(float2)`.
    #[must_use]
    pub fn make_offset(&self, offset: Float2) -> Rect {
        Rect::from_vals(self.vals + Float4::from_xy_zw(offset, neg2(offset)))
    }

    /// `makeJoin(Rect)`.
    #[must_use]
    pub fn make_join(&self, rect: Rect) -> Rect {
        Rect::from_vals(self.vals.min(rect.vals))
    }

    /// `makeIntersect(Rect)`.
    #[must_use]
    pub fn make_intersect(&self, rect: Rect) -> Rect {
        Rect::from_vals(self.vals.max(rect.vals))
    }

    /// `makeSorted()`.
    #[must_use]
    pub fn make_sorted(&self) -> Rect {
        Rect::from_vals(self.vals.min(-self.vals.zwxy()))
    }

    /// `roundIn()`.
    pub fn round_in(&mut self) -> &mut Rect {
        *self = self.make_round_in();
        self
    }

    /// `roundOut()`.
    pub fn round_out(&mut self) -> &mut Rect {
        *self = self.make_round_out();
        self
    }

    /// `round()`.
    pub fn round(&mut self) -> &mut Rect {
        *self = self.make_round();
        self
    }

    /// `inset(float)`.
    pub fn inset(&mut self, inset: f32) -> &mut Rect {
        *self = self.make_inset(inset);
        self
    }

    /// `inset(float2)`.
    pub fn inset_vec(&mut self, inset: Float2) -> &mut Rect {
        *self = self.make_inset_vec(inset);
        self
    }

    /// `outset(float)`.
    pub fn outset(&mut self, outset: f32) -> &mut Rect {
        *self = self.make_outset(outset);
        self
    }

    /// `outset(float2)`.
    pub fn outset_vec(&mut self, outset: Float2) -> &mut Rect {
        *self = self.make_outset_vec(outset);
        self
    }

    /// `offset(float2)`.
    pub fn offset(&mut self, offset: Float2) -> &mut Rect {
        *self = self.make_offset(offset);
        self
    }

    /// `join(Rect)`.
    pub fn join(&mut self, rect: Rect) -> &mut Rect {
        *self = self.make_join(rect);
        self
    }

    /// `intersect(Rect)`.
    pub fn intersect(&mut self, rect: Rect) -> &mut Rect {
        *self = self.make_intersect(rect);
        self
    }

    /// `sort()`.
    pub fn sort(&mut self) -> &mut Rect {
        *self = self.make_sorted();
        self
    }
}

impl PartialEq for Rect {
    // Port of: src/gpu/graphite/geom/Rect.h#L43 (chrome/m156)
    fn eq(&self, other: &Rect) -> bool {
        all_eq4(self.vals, other.vals)
    }
}

impl PartialEq<SkRect> for Rect {
    // `rect == skRect` promotes `skRect` to a Rect.
    fn eq(&self, other: &SkRect) -> bool {
        *self == Rect::from_sk_rect(other)
    }
}

impl From<SkRect> for Rect {
    // Port of: src/gpu/graphite/geom/Rect.h#L35 (chrome/m156)
    fn from(r: SkRect) -> Self {
        Rect::from_sk_rect(&r)
    }
}

impl From<IRect> for Rect {
    // Port of: src/gpu/graphite/geom/Rect.h#L36 (chrome/m156)
    fn from(r: IRect) -> Self {
        Rect::from_sk_irect(&r)
    }
}

/// A rect stored in a complementary form of `[right, bottom, -left, -top]`. Store a local
/// `ComplementRect` object if `intersects()` will be called many times.
#[doc(alias = "skgpu::graphite::Rect::ComplementRect")]
#[derive(Clone, Copy, Debug)]
pub struct ComplementRect {
    vals: Float4, // [right, bottom, -left, -top]
}

impl ComplementRect {
    /// `ComplementRect(Rect rect)`.
    #[must_use]
    pub fn new(rect: Rect) -> Self {
        Self {
            vals: -rect.vals.zwxy(),
        }
    }
}
