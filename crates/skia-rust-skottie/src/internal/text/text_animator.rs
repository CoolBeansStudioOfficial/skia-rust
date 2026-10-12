// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/TextAnimator.h, TextAnimator.cpp (chrome/m156)

use std::rc::Rc;

use skia_rust_core::color::Color;
use skia_rust_core::m44::{V2, V3};

use crate::internal::animator::{Prop, PropertyContainer};
use crate::internal::skottie_priv::AnimationBuilder;
use crate::internal::text::range_selector::RangeSelector;
use crate::json::ObjectValue;
use crate::skottie_json::ValueExt;
use crate::skottie_value::{ColorValue, VectorValue};

/// Direct mapping of AE properties.
// Port of: modules/skottie/src/text/TextAnimator.h#L35-L47 (chrome/m156) (`TextAnimator::AnimatedProps`)
struct AnimatedProps {
    position: Prop<VectorValue>,
    scale: Prop<VectorValue>,
    fill_color: Prop<ColorValue>,
    stroke_color: Prop<ColorValue>,
    // unlike pos/scale which are animated vectors, rotation is separated in each dimension.
    rotation_x: Prop<f32>,
    rotation_y: Prop<f32>,
    rotation_z: Prop<f32>,
    blur: Prop<V2>,
    line_spacing: Prop<V2>,
    opacity: Prop<f32>,
    fill_opacity: Prop<f32>,
    stroke_opacity: Prop<f32>,
    tracking: Prop<f32>,
    stroke_width: Prop<f32>,
}

impl AnimatedProps {
    fn new() -> Self {
        Self {
            position: Prop::new(VectorValue::new()),
            scale: Prop::new(VectorValue::from_slice(&[100.0, 100.0, 100.0])),
            fill_color: Prop::new(ColorValue::new()),
            stroke_color: Prop::new(ColorValue::new()),
            rotation_x: Prop::new(0.0),
            rotation_y: Prop::new(0.0),
            rotation_z: Prop::new(0.0),
            blur: Prop::new(V2 { x: 0.0, y: 0.0 }),
            line_spacing: Prop::new(V2 { x: 0.0, y: 0.0 }),
            opacity: Prop::new(100.0),
            fill_opacity: Prop::new(100.0),
            stroke_opacity: Prop::new(100.0),
            tracking: Prop::new(0.0),
            stroke_width: Prop::new(0.0),
        }
    }
}

/// The properties of a fragment, after the animators modulated them.
// Port of: modules/skottie/src/text/TextAnimator.h#L49-L58 (chrome/m156) (`TextAnimator::ResolvedProps`)
#[doc(alias = "skottie::internal::TextAnimator::ResolvedProps")]
#[derive(Debug, Clone, Copy)]
pub struct ResolvedProps {
    /// The position (`position`).
    pub position: V3,
    /// The scale (`scale`).
    pub scale: V3,
    /// The rotation (`rotation`).
    pub rotation: V3,
    /// The opacity (`opacity`).
    pub opacity: f32,
    /// The tracking (`tracking`).
    pub tracking: f32,
    /// The stroke width (`stroke_width`).
    pub stroke_width: f32,
    /// The fill color (`fill_color`).
    pub fill_color: Color,
    /// The stroke color (`stroke_color`).
    pub stroke_color: Color,
    /// The blur (`blur`).
    pub blur: V2,
    /// The line spacing (`line_spacing`).
    pub line_spacing: V2,
}

impl Default for ResolvedProps {
    // Port of: modules/skottie/src/text/TextAnimator.h#L49-L58 (chrome/m156) (member initializers)
    fn default() -> Self {
        Self {
            position: V3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: V3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            rotation: V3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            opacity: 1.0,
            tracking: 0.0,
            stroke_width: 0.0,
            fill_color: Color::TRANSPARENT,
            stroke_color: Color::TRANSPARENT,
            blur: V2 { x: 0.0, y: 0.0 },
            line_spacing: V2 { x: 0.0, y: 0.0 },
        }
    }
}

/// The properties of a fragment, and the coverage of the current animator.
// Port of: modules/skottie/src/text/TextAnimator.h#L60-L63 (chrome/m156) (`TextAnimator::AnimatedPropsModulator`)
#[doc(alias = "skottie::internal::TextAnimator::AnimatedPropsModulator")]
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimatedPropsModulator {
    /// Accumulates properties across *all* animators (`props`).
    pub props: ResolvedProps,
    /// Accumulates range selector coverage for a given animator (`coverage`).
    pub coverage: f32,
}

/// The modulators of all the fragments (`TextAnimator::ModulatorBuffer`).
// Port of: modules/skottie/src/text/TextAnimator.h#L64 (chrome/m156)
pub type ModulatorBuffer = Vec<AnimatedPropsModulator>;

/// Describes how an index domain (words, lines, etc) relates to the full fragment index range:
/// each `domain[i]` represents a `[domain[i].offset .. domain[i].offset+domain[i].count-1]`
/// fragment subset.
// Port of: modules/skottie/src/text/TextAnimator.h#L66-L77 (chrome/m156) (`TextAnimator::DomainSpan`)
#[doc(alias = "skottie::internal::TextAnimator::DomainSpan")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainSpan {
    /// The first fragment (`fOffset`).
    pub offset: usize,
    /// The number of fragments (`fCount`).
    pub count: usize,
    /// Cumulative advance for all fragments in the span (`fAdvance`).
    pub advance: f32,
    /// Max ascent for all fragments in the span (`fAscent`).
    pub ascent: f32,
}

/// The spans of a domain (`TextAnimator::DomainMap`).
// Port of: modules/skottie/src/text/TextAnimator.h#L78 (chrome/m156)
pub type DomainMap = Vec<DomainSpan>;

/// The domain maps of an adapter (`TextAnimator::DomainMaps`).
// Port of: modules/skottie/src/text/TextAnimator.h#L80-L84 (chrome/m156) (`TextAnimator::DomainMaps`)
#[doc(alias = "skottie::internal::TextAnimator::DomainMaps")]
#[derive(Debug, Clone, Default)]
pub struct DomainMaps {
    /// The non-whitespace fragments (`fNonWhitespaceMap`).
    pub non_whitespace_map: Rc<DomainMap>,
    /// The words (`fWordsMap`).
    pub words_map: Rc<DomainMap>,
    /// The lines (`fLinesMap`).
    pub lines_map: Rc<DomainMap>,
}

/// A text property animator: animated properties, and the range selectors that determine the
/// text subset they apply to.
// Port of: modules/skottie/src/text/TextAnimator.h#L30-L112 (chrome/m156) (`class TextAnimator`)
#[doc(alias = "skottie::internal::TextAnimator")]
#[allow(clippy::struct_excessive_bools)] // mirrors the bit fields of the C++ class
pub struct TextAnimator {
    selectors: Vec<Rc<RangeSelector>>,

    text_props: AnimatedProps,
    has_fill_color: bool,
    has_stroke_color: bool,
    has_fill_opacity: bool,
    has_stroke_opacity: bool,
    has_opacity: bool,
    has_blur: bool,
    /// Animator sensitive to transform origin?
    requires_anchor_point: bool,
    /// Animator effects line-wide fragment adjustments.
    requires_line_adjustments: bool,
}

// Port of: src/core/SkSwizzlePriv.h#L49-L51 (chrome/m156) (`Sk4f_fromL32`). The lanes are
// independent IEEE operations: a byte converted to float, then scaled.
fn sk4f_from_l32(px: Color) -> [f32; 4] {
    let bytes = px.argb().to_le_bytes();
    let scale = 1.0_f32 / 255.0_f32;
    [
        f32::from(bytes[0]) * scale,
        f32::from(bytes[1]) * scale,
        f32::from(bytes[2]) * scale,
        f32::from(bytes[3]) * scale,
    ]
}

// Port of: src/core/SkSwizzlePriv.h#L53-L60 (chrome/m156) (`Sk4f_toL32`). For the expected
// positive color values, the +0.5 before the pin and cast effectively rounds to the nearest int
// without having to call round() or lrint().
fn sk4f_to_l32(px: [f32; 4]) -> Color {
    let mut bytes = [0_u8; 4];
    for (byte, lane) in bytes.iter_mut().zip(px) {
        // skvx::pin(x, 0, 255) is max(0, min(255, x)); the float to byte cast of the in-range
        // value truncates.
        let x = lane * 255.0_f32 + 0.5_f32;
        let pinned = {
            let min = if 255.0_f32 < x { 255.0_f32 } else { x };
            if min < 0.0_f32 { 0.0_f32 } else { min }
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // pinned to [0, 255] above
        {
            *byte = pinned as u8;
        }
    }
    Color::new(u32::from_le_bytes(bytes))
}

// Port of: modules/skottie/src/text/TextAnimator.cpp#L97-L100 (chrome/m156) (`lerp`)
fn lerp(v0: f32, v1: f32, t: f32) -> f32 {
    v0 + (v1 - v0) * t
}

// Port of: modules/skottie/src/text/TextAnimator.cpp#L101-L107 (chrome/m156) (`lerp_color`)
fn lerp_color(c0: Color, c1: Color, t: f32) -> Color {
    let c0_4f = sk4f_from_l32(c0);
    let c1_4f = sk4f_from_l32(c1);
    let mut lerped = [0.0_f32; 4];
    for i in 0..4 {
        lerped[i] = c0_4f[i] + (c1_4f[i] - c0_4f[i]) * t;
    }

    sk4f_to_l32(lerped)
}

/// `std::round` to an unsigned 8-bit value (`static_cast<U8CPU>(std::round(alpha))`).
fn round_to_u8(alpha: f32) -> u8 {
    // The C++ cast of an out-of-range float is undefined; this saturates.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        alpha.round() as u8
    }
}

impl TextAnimator {
    /*
     * Text layers can have optional text property animators.
     *
     * Each animator consists of
     *
     *   1) a list of animated properties (e.g. position, fill color, etc)
     *
     *   2) a list of range selectors
     *
     * Animated properties yield new values to be applied to the text, while range selectors
     * determine the text subset these new values are applied to.
     *
     * The best way to think of range selectors is in terms of coverage: they combine to generate
     * a coverage value [0..1] for each text fragment/glyph.  This coverage is then used to
     * modulate how the new property value is applied to a given fragment (interpolation weight).
     *
     * Note: Bodymovin currently only supports a single selector.
     *
     * JSON structure:
     *
     * "t": {              // text node
     *   "a": [            // animators list
     *     {               // animator node
     *       "s": {...},   // selector node
     *       "a": {        // animator properties node
     *         "a":  {}    // optional anchor point value
     *         "p":  {},   // optional position value
     *         "s":  {},   // optional scale value
     *         "o":  {},   // optional opacity
     *         "fc": {},   // optional fill color value
     *         "sc": {},   // optional stroke color value
     *
     *         // TODO: more props?
     *       }
     *     },
     *     ...
     *   ],
     *   ...
     * }
     */
    /// Makes an animator from `janimator`, binding its properties to `acontainer`.
    // Port of: modules/skottie/src/text/TextAnimator.cpp#L56-L96 (chrome/m156)
    #[must_use]
    pub fn make(
        janimator: Option<&ObjectValue>,
        abuilder: &AnimationBuilder<'_>,
        acontainer: &PropertyContainer,
    ) -> Option<Rc<Self>> {
        let janimator = janimator?;

        let jprops = janimator.get("a").as_object()?;

        let mut selectors: Vec<Rc<RangeSelector>> = Vec::new();

        // Depending on compat mode and whether more than one selector is present,
        // BM exports either an array or a single object.
        if let Some(jselectors) = janimator.get("s").as_array() {
            selectors.reserve(jselectors.size());
            for i in 0..jselectors.size() {
                let Some(jselector) = jselectors[i].as_object() else {
                    continue;
                };
                if let Some(sel) = RangeSelector::make(Some(jselector), abuilder, acontainer) {
                    selectors.push(sel);
                }
            }
        } else if let Some(sel) =
            RangeSelector::make(janimator.get("s").as_object(), abuilder, acontainer)
        {
            selectors.reserve(1);
            selectors.push(sel);
        }

        Some(Rc::new(Self::new(selectors, jprops, abuilder, acontainer)))
    }

    // Port of: modules/skottie/src/text/TextAnimator.cpp#L186-L224 (chrome/m156)
    fn new(
        selectors: Vec<Rc<RangeSelector>>,
        jprops: &ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        acontainer: &PropertyContainer,
    ) -> Self {
        let text_props = AnimatedProps::new();
        let mut requires_anchor_point = false;
        let mut requires_line_adjustments = false;

        acontainer.bind(abuilder, jprops.get("p"), &text_props.position);

        // Tracking and line spacing affect all line fragments.
        requires_line_adjustments |=
            acontainer.bind(abuilder, jprops.get("t"), &text_props.tracking);
        requires_line_adjustments |=
            acontainer.bind(abuilder, jprops.get("ls"), &text_props.line_spacing);

        // Scale and rotation are anchor-point-dependent.
        requires_anchor_point |= acontainer.bind(abuilder, jprops.get("s"), &text_props.scale);

        // Depending on whether we're in 2D/3D mode, some of these will stick and some will not.
        // It's fine either way.
        requires_anchor_point |=
            acontainer.bind(abuilder, jprops.get("rx"), &text_props.rotation_x);
        requires_anchor_point |=
            acontainer.bind(abuilder, jprops.get("ry"), &text_props.rotation_y);
        requires_anchor_point |= acontainer.bind(abuilder, jprops.get("r"), &text_props.rotation_z);

        let has_fill_color = acontainer.bind(abuilder, jprops.get("fc"), &text_props.fill_color);
        let has_stroke_color =
            acontainer.bind(abuilder, jprops.get("sc"), &text_props.stroke_color);
        let has_fill_opacity =
            acontainer.bind(abuilder, jprops.get("fo"), &text_props.fill_opacity);
        let has_stroke_opacity =
            acontainer.bind(abuilder, jprops.get("so"), &text_props.stroke_opacity);
        let has_opacity = acontainer.bind(abuilder, jprops.get("o"), &text_props.opacity);
        let has_blur = acontainer.bind(abuilder, jprops.get("bl"), &text_props.blur);

        acontainer.bind(abuilder, jprops.get("sw"), &text_props.stroke_width);

        Self {
            selectors,
            text_props,
            has_fill_color,
            has_stroke_color,
            has_fill_opacity,
            has_stroke_opacity,
            has_opacity,
            has_blur,
            requires_anchor_point,
            requires_line_adjustments,
        }
    }

    /// Updates the domain maps of the selectors (`updateDomainMaps`).
    // Port of: modules/skottie/src/text/TextAnimator.cpp#L98-L102 (chrome/m156)
    #[doc(alias = "updateDomainMaps")]
    pub fn update_domain_maps(&self, maps: &DomainMaps, fragment_count: usize) {
        for selector in &self.selectors {
            selector.update_domain_map(maps, fragment_count);
        }
    }

    /// Modulates the properties of the fragments in `buf` (`modulateProps`).
    // Port of: modules/skottie/src/text/TextAnimator.cpp#L104-L124 (chrome/m156)
    #[doc(alias = "modulateProps")]
    pub fn modulate_props(&self, buf: &mut ModulatorBuffer) {
        // No selectors -> full coverage.
        let initial_coverage = if self.selectors.is_empty() { 1.0 } else { 0.0 };

        // Coverage is scoped per animator.
        for mod_ in buf.iter_mut() {
            mod_.coverage = initial_coverage;
        }

        // Accumulate selector coverage.
        for selector in &self.selectors {
            selector.modulate_coverage(buf);
        }

        // Modulate animated props.
        for mod_ in buf.iter_mut() {
            mod_.props = self.modulate_props_amount(&mod_.props, mod_.coverage);
        }
    }

    // Port of: modules/skottie/src/text/TextAnimator.cpp#L126-L184 (chrome/m156)
    fn modulate_props_amount(&self, props: &ResolvedProps, amount: f32) -> ResolvedProps {
        let mut modulated_props = *props;

        let text_position = self.text_props.position.borrow().to_v3();
        let text_scale = self.text_props.scale.borrow().to_v3();
        let text_rotation = V3 {
            x: self.text_props.rotation_x.get(),
            y: self.text_props.rotation_y.get(),
            z: self.text_props.rotation_z.get(),
        };

        // Transform props compose.
        modulated_props.position += text_position * amount;
        modulated_props.rotation += text_rotation * amount;
        modulated_props.tracking += self.text_props.tracking.get() * amount;
        let one = V3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        modulated_props.scale *= one + (text_scale * 0.01_f32 - one) * amount;

        // ... as do blur, line spacing, and stroke width.
        modulated_props.blur += self.text_props.blur.get() * amount;
        modulated_props.line_spacing += self.text_props.line_spacing.get() * amount;
        modulated_props.stroke_width += self.text_props.stroke_width.get() * amount;

        // Colors and opacity are interpolated, and use a clamped amount value.
        // std::max(amount, 0.0f)
        let clamped_amount = if amount < 0.0 { 0.0 } else { amount };
        if self.has_fill_color {
            modulated_props.fill_color = lerp_color(
                props.fill_color,
                self.text_props.fill_color.borrow().to_color(),
                clamped_amount,
            );
        }
        if self.has_stroke_color {
            modulated_props.stroke_color = lerp_color(
                props.stroke_color,
                self.text_props.stroke_color.borrow().to_color(),
                clamped_amount,
            );
        }
        if self.has_fill_opacity {
            // 255-based
            let alpha = lerp(
                f32::from(props.fill_color.a()),
                self.text_props.fill_opacity.get() * 2.55_f32,
                clamped_amount,
            );
            modulated_props.fill_color = modulated_props.fill_color.with_a(round_to_u8(alpha));
        }
        if self.has_stroke_opacity {
            // 255-based
            let alpha = lerp(
                f32::from(props.stroke_color.a()),
                self.text_props.stroke_opacity.get() * 2.55_f32,
                clamped_amount,
            );
            modulated_props.stroke_color = modulated_props.stroke_color.with_a(round_to_u8(alpha));
        }
        if self.has_opacity {
            modulated_props.opacity = lerp(
                props.opacity,
                self.text_props.opacity.get() * 0.01_f32,
                clamped_amount,
            );
        }

        modulated_props
    }

    /// True if the animator has a blur (`hasBlur`).
    #[doc(alias = "hasBlur")]
    #[must_use]
    pub fn has_blur(&self) -> bool {
        self.has_blur
    }

    /// True if the animator is sensitive to the transform origin (`requiresAnchorPoint`).
    #[doc(alias = "requiresAnchorPoint")]
    #[must_use]
    pub fn requires_anchor_point(&self) -> bool {
        self.requires_anchor_point
    }

    /// True if the animator affects line-wide fragment adjustments
    /// (`requiresLineAdjustments`).
    #[doc(alias = "requiresLineAdjustments")]
    #[must_use]
    pub fn requires_line_adjustments(&self) -> bool {
        self.requires_line_adjustments
    }
}

opaque_debug!(TextAnimator);
