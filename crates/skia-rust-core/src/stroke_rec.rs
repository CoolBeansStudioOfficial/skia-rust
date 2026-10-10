// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkStrokeRec.h, src/core/SkStrokeRec.cpp

//! `SkStrokeRec`: the stroke parameters (width, caps, joins, miter limit, resolution scale) that
//! turn a path into its stroked outline.

use crate::paint::{Cap, DEFAULT_MITER_LIMIT, Join, Paint, Style as PaintStyle};
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::scalar::{SCALAR_1, SCALAR_SQRT2, scalar};
use crate::stroke::Stroke;

/// The style a [`StrokeRec`] starts with (`SkStrokeRec::InitStyle`).
// Port of: include/core/SkStrokeRec.h#L23-L26 (chrome/m156)
#[doc(alias = "SkStrokeRec::InitStyle")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InitStyle {
    /// `kHairline_InitStyle`.
    Hairline,
    /// `kFill_InitStyle`.
    Fill,
}

/// The effective style of a [`StrokeRec`] (`SkStrokeRec::Style`).
// Port of: include/core/SkStrokeRec.h#L32-L37 (chrome/m156)
#[doc(alias = "SkStrokeRec::Style")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Style {
    /// `kHairline_Style`.
    Hairline,
    /// `kFill_Style`.
    Fill,
    /// `kStroke_Style`.
    Stroke,
    /// `kStrokeAndFill_Style`.
    StrokeAndFill,
}

// must be < 0, since ==0 means hairline, and >0 means normal stroke
// Port of: src/core/SkStrokeRec.cpp#L16 (chrome/m156)
const STROKE_REC_FILL_STYLE_WIDTH: scalar = -SCALAR_1;

/// The parameters of a stroke (`SkStrokeRec`).
// Port of: include/core/SkStrokeRec.h#L21-L160 (chrome/m156)
#[doc(alias = "SkStrokeRec")]
#[derive(Copy, Clone, Debug)]
pub struct StrokeRec {
    res_scale: scalar,
    width: scalar,
    miter_limit: scalar,
    cap: Cap,
    join: Join,
    stroke_and_fill: bool,
}

impl StrokeRec {
    /// The number of [`Style`]s (`SkStrokeRec::kStyleCount`).
    #[doc(alias = "kStyleCount")]
    pub const STYLE_COUNT: usize = 4;

    /// A rec for a fill or a hairline (`SkStrokeRec(InitStyle)`).
    // Port of: src/core/SkStrokeRec.cpp#L18-L25 (chrome/m156)
    #[must_use]
    pub fn new(init_style: InitStyle) -> Self {
        Self {
            res_scale: 1.0,
            width: if init_style == InitStyle::Fill {
                STROKE_REC_FILL_STYLE_WIDTH
            } else {
                0.0
            },
            miter_limit: DEFAULT_MITER_LIMIT,
            cap: Cap::DEFAULT,
            join: Join::DEFAULT,
            stroke_and_fill: false,
        }
    }

    /// A hairline rec.
    #[must_use]
    pub fn new_hairline() -> Self {
        Self::new(InitStyle::Hairline)
    }

    /// A fill rec.
    #[must_use]
    pub fn new_fill() -> Self {
        Self::new(InitStyle::Fill)
    }

    /// The stroke of `paint`, with `style` overriding the paint's style if given, and a
    /// resolution scale of `res_scale` (1 if `None`) (`SkStrokeRec(const SkPaint&,
    /// [SkPaint::Style,] SkScalar resScale)`).
    // Port of: src/core/SkStrokeRec.cpp#L27-L69 (chrome/m156)
    #[doc(alias = "SkStrokeRec")]
    #[must_use]
    pub fn from_paint(
        paint: &Paint,
        style: impl Into<Option<PaintStyle>>,
        res_scale: impl Into<Option<scalar>>,
    ) -> Self {
        let style = style.into().unwrap_or(paint.style());
        let res_scale = res_scale.into().unwrap_or(1.0);
        let stroke_width = paint.stroke_width();
        let (width, stroke_and_fill) = match style {
            PaintStyle::Fill => (STROKE_REC_FILL_STYLE_WIDTH, false),
            PaintStyle::Stroke => (stroke_width, false),
            PaintStyle::StrokeAndFill => {
                #[allow(clippy::float_cmp)] // exact comparison, as in Skia
                if 0.0 == stroke_width {
                    // hairline+fill == fill
                    (STROKE_REC_FILL_STYLE_WIDTH, false)
                } else {
                    (stroke_width, true)
                }
            }
        };
        Self {
            res_scale,
            width,
            // copy these from the paint, regardless of our "style"
            miter_limit: paint.stroke_miter(),
            cap: paint.stroke_cap(),
            join: paint.stroke_join(),
            stroke_and_fill,
        }
    }

    /// The effective style.
    // Port of: src/core/SkStrokeRec.cpp#L71-L79 (chrome/m156)
    #[doc(alias = "getStyle")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    pub fn style(&self) -> Style {
        if self.width < 0.0 {
            Style::Fill
        } else if 0.0 == self.width {
            Style::Hairline
        } else if self.stroke_and_fill {
            Style::StrokeAndFill
        } else {
            Style::Stroke
        }
    }

    /// `applyToPaint(paint)`: sets the paint's style and stroke parameters from this record.
    // Port of: src/core/SkStrokeRec.cpp#L127-L138 (chrome/m156)
    #[doc(alias = "applyToPaint")]
    pub fn apply_to_paint(&self, paint: &mut Paint) {
        if self.width < 0.0 {
            // fill
            paint.set_style(PaintStyle::Fill);
            return;
        }

        paint.set_style(if self.stroke_and_fill {
            PaintStyle::StrokeAndFill
        } else {
            PaintStyle::Stroke
        });
        paint.set_stroke_width(self.width);
        paint.set_stroke_miter(self.miter_limit);
        paint.set_stroke_cap(self.cap);
        paint.set_stroke_join(self.join);
    }

    /// The stroke width.
    #[doc(alias = "getWidth")]
    #[must_use]
    pub fn width(&self) -> scalar {
        self.width
    }

    /// The miter limit.
    #[doc(alias = "getMiter")]
    #[must_use]
    pub fn miter(&self) -> scalar {
        self.miter_limit
    }

    /// The cap.
    #[doc(alias = "getCap")]
    #[must_use]
    pub fn cap(&self) -> Cap {
        self.cap
    }

    /// The join.
    #[doc(alias = "getJoin")]
    #[must_use]
    pub fn join(&self) -> Join {
        self.join
    }

    /// True if the style is [`Style::Hairline`].
    #[doc(alias = "isHairlineStyle")]
    #[must_use]
    pub fn is_hairline_style(&self) -> bool {
        Style::Hairline == self.style()
    }

    /// True if the style is [`Style::Fill`].
    #[doc(alias = "isFillStyle")]
    #[must_use]
    pub fn is_fill_style(&self) -> bool {
        Style::Fill == self.style()
    }

    /// Makes this a fill.
    // Port of: src/core/SkStrokeRec.cpp#L81-L84 (chrome/m156)
    #[doc(alias = "setFillStyle")]
    pub fn set_fill_style(&mut self) -> &mut Self {
        self.width = STROKE_REC_FILL_STYLE_WIDTH;
        self.stroke_and_fill = false;
        self
    }

    /// Makes this a hairline.
    // Port of: src/core/SkStrokeRec.cpp#L86-L89 (chrome/m156)
    #[doc(alias = "setHairlineStyle")]
    pub fn set_hairline_style(&mut self) -> &mut Self {
        self.width = 0.0;
        self.stroke_and_fill = false;
        self
    }

    /// Specifies the stroke width, and optionally if you want stroke + fill. Note, if
    /// `width == 0`, then this request is taken to mean: `stroke_and_fill == true` -> new style
    /// will be fill, `stroke_and_fill == false` -> new style will be hairline.
    // Port of: src/core/SkStrokeRec.cpp#L91-L99 (chrome/m156)
    #[doc(alias = "setStrokeStyle")]
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    pub fn set_stroke_style(
        &mut self,
        width: scalar,
        stroke_and_fill: impl Into<Option<bool>>,
    ) -> &mut Self {
        let stroke_and_fill = stroke_and_fill.into().unwrap_or(false);
        if stroke_and_fill && (0.0 == width) {
            // hairline+fill == fill
            self.set_fill_style();
        } else {
            self.width = width;
            self.stroke_and_fill = stroke_and_fill;
        }
        self
    }

    /// Sets the cap, join and miter limit.
    // Port of: include/core/SkStrokeRec.h#L74-L78 (chrome/m156)
    #[doc(alias = "setStrokeParams")]
    pub fn set_stroke_params(&mut self, cap: Cap, join: Join, miter_limit: scalar) -> &mut Self {
        self.cap = cap;
        self.join = join;
        self.miter_limit = miter_limit;
        self
    }

    /// The resolution scale.
    #[doc(alias = "getResScale")]
    #[must_use]
    pub fn res_scale(&self) -> scalar {
        self.res_scale
    }

    /// Sets the resolution scale; `rs` must be positive and finite.
    // Port of: include/core/SkStrokeRec.h#L84-L87 (chrome/m156)
    #[doc(alias = "setResScale")]
    pub fn set_res_scale(&mut self, rs: scalar) {
        debug_assert!(rs > 0.0 && rs.is_finite());
        self.res_scale = rs;
    }

    /// True if this specifies any thick stroking, i.e. [`Self::apply_to_path`] will return true.
    // Port of: include/core/SkStrokeRec.h#L93-L96 (chrome/m156)
    #[doc(alias = "needToApply")]
    #[must_use]
    pub fn need_to_apply(&self) -> bool {
        let style = self.style();
        Style::Stroke == style || Style::StrokeAndFill == style
    }

    /// Applies these stroke parameters to `src`, returning the result in `dst`.
    ///
    /// If there was no change (i.e. style == hairline or fill) this returns false and `dst` is
    /// unchanged. Otherwise returns true and the result is stored in `dst`.
    // Port of: src/core/SkStrokeRec.cpp#L107-L125 (chrome/m156)
    #[doc(alias = "applyToPath")]
    pub fn apply_to_path(&self, dst: &mut PathBuilder, src: &Path) -> bool {
        if self.width <= 0.0 {
            // hairline or fill
            return false;
        }

        let mut stroker = Stroke::new();
        stroker.set_cap(self.cap);
        stroker.set_join(self.join);
        stroker.set_miter_limit(self.miter_limit);
        stroker.set_width(self.width);
        stroker.set_do_fill(self.stroke_and_fill);
        stroker.set_res_scale(self.res_scale);
        stroker.stroke_path(src, dst);
        true
    }

    /// Gives a conservative value for the outset that should be applied to a geometry's bounds
    /// to account for any inflation due to applying this rec to the geometry.
    // Port of: src/core/SkStrokeRec.cpp#L140-L142 (chrome/m156)
    #[doc(alias = "getInflationRadius")]
    #[must_use]
    pub fn inflation_radius(&self) -> scalar {
        Self::inflation_radius_from_params(self.join, self.miter_limit, self.cap, self.width)
    }

    /// The inflation radius for the given stroke parameters.
    // Port of: src/core/SkStrokeRec.cpp#L151-L171 (chrome/m156)
    #[doc(alias = "GetInflationRadius")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    pub fn inflation_radius_from_params(
        join: Join,
        miter_limit: scalar,
        cap: Cap,
        stroke_width: scalar,
    ) -> scalar {
        if stroke_width < 0.0 {
            // fill
            return 0.0;
        } else if 0.0 == stroke_width {
            // FIXME: We need a "matrixScale" parameter here in order to properly handle hairlines.
            // Their with is determined in device space, unlike other strokes.
            // skbug.com/40039419
            return SCALAR_1;
        }

        // since we're stroked, outset the rect by the radius (and join type, caps)
        let mut multiplier = SCALAR_1;
        if Join::Miter == join {
            multiplier = std_max(multiplier, miter_limit);
        }
        if Cap::Square == cap {
            multiplier = std_max(multiplier, SCALAR_SQRT2);
        }
        stroke_width / 2.0 * multiplier
    }

    /// The inflation radius of `paint`'s stroke, with `style` overriding the paint's style
    /// (`GetInflationRadius(const SkPaint&, SkPaint::Style)`).
    // Port of: src/core/SkStrokeRec.cpp#L144-L149 (chrome/m156)
    #[doc(alias = "GetInflationRadius")]
    #[must_use]
    pub fn inflation_radius_from_paint_and_style(paint: &Paint, style: PaintStyle) -> scalar {
        let width = if PaintStyle::Fill == style {
            -SCALAR_1
        } else {
            paint.stroke_width()
        };
        Self::inflation_radius_from_params(
            paint.stroke_join(),
            paint.stroke_miter(),
            paint.stroke_cap(),
            width,
        )
    }

    /// True if two recs have an equal effect on a path. Equal recs produce equal paths. Equality
    /// of produced paths does not take the res scale into account.
    // Port of: include/core/SkStrokeRec.h#L144-L153 (chrome/m156)
    #[doc(alias = "hasEqualEffect")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact comparisons, as in Skia
    pub fn has_equal_effect(&self, other: &StrokeRec) -> bool {
        if !self.need_to_apply() {
            return self.style() == other.style();
        }
        self.width == other.width
            && (self.join != Join::Miter || self.miter_limit == other.miter_limit)
            && self.cap == other.cap
            && self.join == other.join
            && self.stroke_and_fill == other.stroke_and_fill
    }
}

// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}
