// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/TextShaper.h, modules/skottie/src/text/TextShaper.cpp
// (chrome/m156)
//
// Text shaping with After Effects semantics, on top of `SkShaper`: line breaking, alignment,
// auto-sizing and the glyph fragments that the text animators move around.
//
// `Shaper` is a class with only static members in Skia: its nested types are items of this
// module here, and `Shaper::Shape` is [`Shaper::shape_at_point`] and [`Shaper::shape_in_box`].

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::font_priv::get_font_bounds;
use skia_rust_core::font_types::{FontHinting, GlyphId, set_four_byte_tag};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_MAX;
use skia_rust_core::size::Size;
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utf;
use skia_rust_core::utils::text_utils::Align;
use skia_rust_shaper::factory::Factory;
use skia_rust_shaper::run_iterator::{
    BiDiRunIterator, ScriptRunIterator, TrivialLanguageRunIterator,
    make_font_mgr_run_iterator_with_request, make_std_language_run_iterator,
};
use skia_rust_shaper::shaper::Shaper as SkShaper;
use skia_rust_shaper::{Buffer, Options, RunHandler, RunInfo};

pub use crate::internal::text::unicode::make_strict_linebreak_unicode;

// Port of: modules/skottie/src/text/TextShaper.cpp#L43-L47 (chrome/m156)
fn is_whitespace(c: u8) -> bool {
    // TODO: we've been getting away with this simple heuristic,
    // but ideally we should use SkUicode::isWhiteSpace().
    c == b' ' || c == b'\t' || c == b'\r' || c == b'\n'
}

/// `std::min(a, b)`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// `std::max(a, b)`.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// A run of glyphs that share a font (`Shaper::RunRec`).
// Port of: modules/skottie/include/TextShaper.h#L40-L47 (chrome/m156)
#[doc(alias = "skottie::Shaper::RunRec")]
#[derive(Debug, Clone)]
pub struct RunRec {
    /// The font (`fFont`).
    pub font: Font,
    /// The number of glyphs (`fSize`).
    pub size: usize,
}

/// How tight the bounds of shaped glyphs are (`ShapedGlyphs::BoundsType`).
// Port of: modules/skottie/include/TextShaper.h#L63 (chrome/m156)
#[doc(alias = "skottie::Shaper::ShapedGlyphs::BoundsType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsType {
    /// The glyph positions grown by the bounds of the font (`kConservative`).
    Conservative,
    /// The union of the bounds of the glyphs (`kTight`).
    Tight,
}

/// Positioned glyphs (`Shaper::ShapedGlyphs`).
// Port of: modules/skottie/include/TextShaper.h#L49-L68 (chrome/m156)
#[doc(alias = "skottie::Shaper::ShapedGlyphs")]
#[derive(Debug, Clone, Default)]
pub struct ShapedGlyphs {
    /// The runs (`fRuns`).
    pub runs: Vec<RunRec>,
    /// Consolidated storage for all the runs (`fGlyphIDs`).
    pub glyph_ids: Vec<GlyphId>,
    /// The position of each glyph (`fGlyphPos`).
    pub glyph_pos: Vec<Point>,
    /// `clusters[i]` is an input string index, pointing to the start of the UTF sequence
    /// associated with `glyph_ids[i]`. The number of entries matches the number of glyphs. Only
    /// available with [`Flags::CLUSTERS`] (`fClusters`).
    pub clusters: Vec<usize>,
}

impl ShapedGlyphs {
    /// The bounds of the glyphs (`computeBounds`).
    // Port of: modules/skottie/src/text/TextShaper.cpp#L679-L721 (chrome/m156)
    #[doc(alias = "computeBounds")]
    #[must_use]
    pub fn compute_bounds(&self, btype: BoundsType) -> Rect {
        let mut btype = btype;
        let mut bounds = Rect::new_empty();

        let mut offset = 0;
        for run in &self.runs {
            let mut font_bounds = Rect::new_empty();
            if btype == BoundsType::Conservative {
                font_bounds = get_font_bounds(&run.font);

                // Empty font bounds is likely a font bug -- fall back to tight bounds.
                if font_bounds.is_empty() {
                    btype = BoundsType::Tight;
                }
            }

            match btype {
                BoundsType::Conservative => {
                    let mut run_bounds =
                        Rect::bounds_or_empty(&self.glyph_pos[offset..offset + run.size]);
                    run_bounds.left += font_bounds.left();
                    run_bounds.top += font_bounds.top();
                    run_bounds.right += font_bounds.right();
                    run_bounds.bottom += font_bounds.bottom();

                    bounds.join(run_bounds);
                }
                BoundsType::Tight => {
                    let mut glyph_bounds = vec![Rect::new_empty(); run.size];
                    run.font.get_widths_bounds(
                        &self.glyph_ids[offset..offset + run.size],
                        &mut [],
                        &mut glyph_bounds,
                        None,
                    );

                    for (i, glyph_bound) in glyph_bounds.iter().enumerate() {
                        bounds.join(glyph_bound.with_offset(self.glyph_pos[offset + i]));
                    }
                }
            }

            offset += run.size;
        }

        bounds
    }

    /// Draws the glyphs (`draw`).
    // Port of: modules/skottie/src/text/TextShaper.cpp#L723-L734 (chrome/m156)
    pub fn draw(&self, canvas: &Canvas, origin: Point, paint: &Paint) {
        let mut offset = 0;
        for run in &self.runs {
            canvas.draw_glyphs_at(
                &self.glyph_ids[offset..offset + run.size],
                &self.glyph_pos[offset..offset + run.size],
                origin,
                &run.font,
                paint,
            );
            offset += run.size;
        }
    }
}

/// A piece of shaped text: a line, or a glyph (`Shaper::Fragment`).
// Port of: modules/skottie/include/TextShaper.h#L70-L82 (chrome/m156)
#[doc(alias = "skottie::Shaper::Fragment")]
#[derive(Debug, Clone)]
pub struct Fragment {
    /// The glyphs (`fGlyphs`).
    pub glyphs: ShapedGlyphs,
    /// The origin (`fOrigin`).
    pub origin: Point,
    /// The advance. Only valid with [`Flags::FRAGMENT_GLYPHS`] (`fAdvance`).
    pub advance: f32,
    /// The ascent. Only valid with [`Flags::FRAGMENT_GLYPHS`] (`fAscent`).
    pub ascent: f32,
    /// The 0-based index of the line this fragment belongs to (`fLineIndex`).
    pub line_index: u32,
    /// True if the first code point in the corresponding cluster is whitespace
    /// (`fIsWhitespace`).
    pub is_whitespace: bool,
}

/// The result of shaping (`Shaper::Result`).
// Port of: modules/skottie/include/TextShaper.h#L84-L94 (chrome/m156)
#[doc(alias = "skottie::Shaper::Result")]
#[derive(Debug, Clone)]
pub struct ShapeResult {
    /// The fragments (`fFragments`).
    pub fragments: Vec<Fragment>,
    /// The number of glyphs the fonts lack (`fMissingGlyphCount`).
    pub missing_glyph_count: usize,
    /// Relative text size scale, when using an auto-scaling [`ResizePolicy`] (otherwise 1.0).
    /// This is informative of the final text size, and is not required to render the result
    /// (`fScale`).
    pub scale: f32,
}

impl Default for ShapeResult {
    fn default() -> Self {
        Self {
            fragments: Vec::new(),
            missing_glyph_count: 0,
            scale: 1.0,
        }
    }
}

impl ShapeResult {
    /// The visual bounds of all the fragments (`computeVisualBounds`).
    // Port of: modules/skottie/src/text/TextShaper.cpp#L736-L745 (chrome/m156)
    #[doc(alias = "computeVisualBounds")]
    #[must_use]
    pub fn compute_visual_bounds(&self) -> Rect {
        let mut bounds = Rect::new_empty();

        for frag in &self.fragments {
            bounds.join(
                frag.glyphs
                    .compute_bounds(BoundsType::Tight)
                    .with_offset(frag.origin),
            );
        }

        bounds
    }
}

/// Vertical alignment (`Shaper::VAlign`).
// Port of: modules/skottie/include/TextShaper.h#L96-L121 (chrome/m156)
#[doc(alias = "skottie::Shaper::VAlign")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    /// Align the first line typographical top with the text box top (AE box text).
    #[default]
    Top,
    /// Align the first line typographical baseline with the text box top (AE point text).
    TopBaseline,

    // Skottie vertical alignment extensions.
    //
    // These are based on a hybrid extent box defined (in Y) as
    //
    //   ------------------------------------------------------
    //   MIN(visual_top_extent   , typographical_top_extent   )
    //
    //                         ...
    //
    //   MAX(visual_bottom_extent, typographical_bottom_extent)
    //   ------------------------------------------------------
    /// Extent box top to text box top.
    HybridTop,
    /// Extent box center to text box center.
    HybridCenter,
    /// Extent box bottom to text box bottom.
    HybridBottom,

    // Visual alignement modes -- these are using tight visual bounds for the paragraph.
    /// Visual top to text box top.
    VisualTop,
    /// Visual center to text box center.
    VisualCenter,
    /// Visual bottom to text box bottom.
    VisualBottom,
}

/// How text is resized to fit its box (`Shaper::ResizePolicy`).
// Port of: modules/skottie/include/TextShaper.h#L123-L133 (chrome/m156)
#[doc(alias = "skottie::Shaper::ResizePolicy")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResizePolicy {
    /// Use the specified text size.
    #[default]
    None,
    /// Resize the text such that the extent box fits (snuggly) in the text box, both horizontally
    /// and vertically.
    ScaleToFit,
    /// Same as `ScaleToFit` if the text doesn't fit at the specified font size. Otherwise, same
    /// as `None`.
    DownscaleToFit,
}

/// Where lines are broken (`Shaper::LinebreakPolicy`).
// Port of: modules/skottie/include/TextShaper.h#L135-L140 (chrome/m156)
#[doc(alias = "skottie::Shaper::LinebreakPolicy")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinebreakPolicy {
    /// Break lines such that they fit in a non-empty paragraph box, horizontally.
    Paragraph,
    /// Only break lines when requested explicitly (`\r`), regardless of paragraph box
    /// dimensions.
    #[default]
    Explicit,
}

/// Initial text direction (`Shaper::Direction`).
// Port of: modules/skottie/include/TextShaper.h#L142-L143 (chrome/m156)
#[doc(alias = "skottie::Shaper::Direction")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left to right.
    #[default]
    LTR,
    /// Right to left.
    RTL,
}

/// Capitalization (`Shaper::Capitalization`).
// Port of: modules/skottie/include/TextShaper.h#L145-L148 (chrome/m156)
#[doc(alias = "skottie::Shaper::Capitalization")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Capitalization {
    /// As typed.
    #[default]
    None,
    /// Upper case.
    UpperCase,
}

bitflags::bitflags! {
    /// What the shaper returns (`Shaper::Flags`).
    // Port of: modules/skottie/include/TextShaper.h#L150-L163 (chrome/m156)
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Flags: u32 {
        /// Split out individual glyphs into separate fragments (useful when the caller intends
        /// to manipulate glyphs independently) (`kFragmentGlyphs`).
        const FRAGMENT_GLYPHS = 0x01;
        /// Compute the advance and ascent for each fragment (`kTrackFragmentAdvanceAscent`).
        const TRACK_FRAGMENT_ADVANCE_ASCENT = 0x02;
        /// Return cluster information (`kClusters`).
        const CLUSTERS = 0x04;
    }
}

/// Everything the shaper needs to know about the text (`Shaper::TextDesc`).
// Port of: modules/skottie/include/TextShaper.h#L165-L184 (chrome/m156)
#[doc(alias = "skottie::Shaper::TextDesc")]
#[derive(Debug, Clone, Default)]
pub struct TextDesc {
    /// The typeface (`fTypeface`); `None` is a null typeface, which shapes with the empty one.
    pub typeface: Option<Typeface>,
    /// The text size (`fTextSize`).
    pub text_size: f32,
    /// The minimum text size, when auto-sizing (`fMinTextSize`).
    pub min_text_size: f32,
    /// The maximum text size, when auto-sizing (`fMaxTextSize`).
    pub max_text_size: f32,
    /// The distance between baselines (`fLineHeight`).
    pub line_height: f32,
    /// The vertical shift of the text (`fLineShift`).
    pub line_shift: f32,
    /// The ascent, if not the font's (`fAscent`).
    pub ascent: f32,
    /// The horizontal alignment (`fHAlign`).
    pub h_align: Align,
    /// The vertical alignment (`fVAlign`).
    pub v_align: VAlign,
    /// The resize policy (`fResize`).
    pub resize: ResizePolicy,
    /// The line break policy (`fLinebreak`).
    pub linebreak: LinebreakPolicy,
    /// The initial text direction (`fDirection`).
    pub direction: Direction,
    /// The capitalization (`fCapitalization`).
    pub capitalization: Capitalization,
    /// The maximum number of lines, when auto-sizing; 0 is no maximum (`fMaxLines`).
    pub max_lines: usize,
    /// The flags (`fFlags`).
    pub flags: Flags,
    /// The locale (`fLocale`).
    pub locale: Option<String>,
    /// The font family, passed to the font manager for fallback (`fFontFamily`).
    pub font_family: Option<String>,
    /// The letter tracking, in 1/1000 em (`fTextTracking`).
    pub text_tracking: f32,
}

/// `Shaper::Shape`: After Effects text shaping semantics on top of `SkShaper`.
// Port of: modules/skottie/include/TextShaper.h#L38-L196 (chrome/m156) (`class Shaper`)
#[doc(alias = "skottie::Shaper")]
#[derive(Debug)]
pub struct Shaper;

impl Shaper {
    /// Performs text layout along an infinite horizontal line, starting at `point`. Only
    /// explicit line breaks (`\r`) are observed.
    // Port of: modules/skottie/src/text/TextShaper.cpp#L648-L657 (chrome/m156)
    #[doc(alias = "Shape")]
    #[must_use]
    pub fn shape_at_point(
        text: &str,
        desc: &TextDesc,
        point: Point,
        fontmgr: Option<&FontMgr>,
        shaping_factory: &Arc<dyn Factory>,
    ) -> ShapeResult {
        let adj_text = adjusted_text(text, desc, shaping_factory.unicode());

        if desc.resize == ResizePolicy::ScaleToFit || desc.resize == ResizePolicy::DownscaleToFit {
            // makes no sense in point mode
            ShapeResult::default()
        } else {
            shape_impl(
                &adj_text,
                desc,
                &Rect::new_empty().with_offset(point),
                fontmgr,
                shaping_factory,
                None,
            )
        }
    }

    /// Performs text layout within `box_`, injecting line breaks as needed to ensure horizontal
    /// fitting. The result is *not* guaranteed to fit vertically (it may extend below the box
    /// bottom).
    // Port of: modules/skottie/src/text/TextShaper.cpp#L659-L677 (chrome/m156)
    #[doc(alias = "Shape")]
    #[must_use]
    pub fn shape_in_box(
        text: &str,
        desc: &TextDesc,
        box_: &Rect,
        fontmgr: Option<&FontMgr>,
        shaping_factory: &Arc<dyn Factory>,
    ) -> ShapeResult {
        let adj_text = adjusted_text(text, desc, shaping_factory.unicode());

        match desc.resize {
            ResizePolicy::None => shape_impl(&adj_text, desc, box_, fontmgr, shaping_factory, None),
            ResizePolicy::ScaleToFit => {
                shape_to_fit(&adj_text, desc, box_, fontmgr, shaping_factory)
            }
            ResizePolicy::DownscaleToFit => {
                let mut size = Size::new(0.0, 0.0);
                let result = shape_impl(
                    &adj_text,
                    desc,
                    box_,
                    fontmgr,
                    shaping_factory,
                    Some(&mut size),
                );

                if result_fits(&result, size, box_, desc) {
                    result
                } else {
                    shape_to_fit(&adj_text, desc, box_, fontmgr, shaping_factory)
                }
            }
        }
    }
}

// Helper for interfacing with SkShaper: buffers shaper-fed runs and performs per-line position
// adjustments (for external line breaking, horizontal alignment, etc).
// Port of: modules/skottie/src/text/TextShaper.cpp#L49-L398 (chrome/m156) (`ResultBuilder`)
struct ResultBuilder<'a> {
    desc: &'a TextDesc,
    box_: &'a Rect,
    h_align_factor: f32,

    font: Font,
    font_mgr: Option<FontMgr>,
    shaper: Option<SkShaper>,
    shaping_factory: Arc<dyn Factory>,

    line_glyphs: Vec<GlyphId>,
    line_pos: Vec<Point>,
    line_clusters: Vec<u32>,
    line_runs: Vec<RunRec>,
    line_glyph_count: usize,

    advance_buffer: Vec<f32>,

    current_position: Point,
    offset: Point,
    pending_line_advance: Vector,
    line_count: u32,
    first_line_ascent: f32,
    last_line_descent: f32,

    /// `fUTF8`: the line being shaped. Only valid during `shape_line` calls.
    utf8: Vec<u8>,
    /// `fUTF8Offset`: the offset of the current line within the original string.
    utf8_offset: usize,

    result: ShapeResult,
}

/// `kMissingGlyphID`.
const MISSING_GLYPH_ID: GlyphId = 0;

// Port of: modules/skottie/src/text/TextShaper.cpp#L159-L160 (chrome/m156)
const BIDI_LEVEL_LTR: u8 = 0;
const BIDI_LEVEL_RTL: u8 = 1;

impl<'a> ResultBuilder<'a> {
    // Port of: modules/skottie/src/text/TextShaper.cpp#L58-L77 (chrome/m156)
    fn new(
        desc: &'a TextDesc,
        box_: &'a Rect,
        fontmgr: Option<&FontMgr>,
        shaping_factory: &Arc<dyn Factory>,
    ) -> Self {
        let mut font = Font::from_typeface(desc.typeface.clone());
        font.set_size(desc.text_size);
        // If the shaper callback returns null, fallback to the primitive shaper. Factories here
        // never return null: this is the fallback of a missing Unicode implementation, which the
        // HarfBuzz factory handles at its construction.
        let shaper = shaping_factory.make_shaper(fontmgr.cloned());
        font.set_hinting(FontHinting::None);
        font.set_subpixel(true);
        font.set_linear_metrics(true);
        font.set_baseline_snap(false);
        font.set_edging(Edging::AntiAlias);

        Self {
            desc,
            box_,
            h_align_factor: h_align_factor(desc.h_align),
            font,
            font_mgr: fontmgr.cloned(),
            shaper: Some(shaper),
            shaping_factory: Arc::clone(shaping_factory),
            line_glyphs: Vec::new(),
            line_pos: Vec::new(),
            line_clusters: Vec::new(),
            line_runs: Vec::new(),
            line_glyph_count: 0,
            advance_buffer: Vec::new(),
            current_position: Point::new(0.0, 0.0),
            offset: Point::new(0.0, 0.0),
            pending_line_advance: Vector::new(0.0, 0.0),
            line_count: 0,
            first_line_ascent: 0.0,
            last_line_descent: 0.0,
            utf8: Vec::new(),
            utf8_offset: 0,
            result: ShapeResult::default(),
        }
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L252-L260 (chrome/m156)
    fn ascent(&self) -> f32 {
        // Use the explicit ascent, when specified.
        // Note: ascent values are negative (relative to the baseline).
        #[allow(clippy::float_cmp)] // C++ `fDesc.fAscent ? ... : ...`
        if self.desc.ascent == 0.0 {
            self.first_line_ascent
        } else {
            self.desc.ascent
        }
    }

    // Tracking expressed as a fraction of the font size (the unit SkShaper expects).
    // Port of: modules/skottie/src/text/TextShaper.cpp#L262-L266 (chrome/m156)
    fn tracking_em(&self) -> f32 {
        const TRACKING_UNITS_PER_EM: f32 = 1000.0;
        self.desc.text_tracking / TRACKING_UNITS_PER_EM
    }

    // Tracking in absolute (text-space) units, applied by the shaper after each glyph.
    // Mirrors the backends' formula (trackingEm * size * scaleX) so the discount matches.
    // Port of: modules/skottie/src/text/TextShaper.cpp#L268-L272 (chrome/m156)
    fn tracking_advance(&self) -> f32 {
        self.tracking_em() * self.font.size() * self.font.scale_x()
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L186-L246 (chrome/m156)
    fn commit_fragmented_run(
        &mut self,
        run: &RunRec,
        glyphs: &[GlyphId],
        pos: &[Point],
        clusters: &[u32],
        line_index: u32,
    ) {
        let mut ascent = 0.0;

        if self
            .desc
            .flags
            .contains(Flags::TRACK_FRAGMENT_ADVANCE_ASCENT)
        {
            let (_, metrics) = run.font.metrics();
            ascent = metrics.ascent;

            // Note: we use per-glyph advances for anchoring, but it's unclear whether this
            // is exactly the same as AE.  E.g. are 'acute' glyphs anchored separately for fonts
            // in which they're distinct?
            self.advance_buffer.resize(run.size, 0.0);
            run.font
                .get_widths(&glyphs[..run.size], &mut self.advance_buffer[..run.size]);
        }

        // In fragmented mode we immediately push the glyphs to fResult,
        // one fragment per glyph.  Glyph positioning is externalized
        // (positions returned in Fragment::fPos).
        for i in 0..run.size {
            let advance = if self
                .desc
                .flags
                .contains(Flags::TRACK_FRAGMENT_ADVANCE_ASCENT)
            {
                self.advance_buffer[i]
            } else {
                0.0
            };

            self.result.fragments.push(Fragment {
                glyphs: ShapedGlyphs {
                    runs: vec![RunRec {
                        font: run.font.clone(),
                        size: 1,
                    }],
                    glyph_ids: vec![glyphs[i]],
                    glyph_pos: vec![Point::new(0.0, 0.0)],
                    clusters: if self.desc.flags.contains(Flags::CLUSTERS) {
                        vec![self.utf8_offset + clusters[i] as usize]
                    } else {
                        Vec::new()
                    },
                },
                origin: Point::new(self.box_.x() + pos[i].x, self.box_.y() + pos[i].y),
                advance,
                ascent,
                line_index,
                is_whitespace: is_whitespace(self.utf8[clusters[i] as usize]),
            });

            // Note: we only check the first code point in the cluster for whitespace.
            // It's unclear whether thers's a saner approach.
            self.result.missing_glyph_count += usize::from(glyphs[i] == MISSING_GLYPH_ID);
        }
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L248-L282 (chrome/m156)
    fn commit_consolidated_run(
        &mut self,
        run: &RunRec,
        glyphs: &[GlyphId],
        pos: &[Point],
        clusters: &[u32],
        _line_index: u32,
    ) {
        // In consolidated mode we just accumulate glyphs to a single fragment in ResultBuilder.
        // Glyph positions are baked in the fragment runs (Fragment::fPos only reflects the
        // box origin).

        if self.result.fragments.is_empty() {
            self.result.fragments.push(Fragment {
                glyphs: ShapedGlyphs::default(),
                origin: Point::new(self.box_.x(), self.box_.y()),
                advance: 0.0,
                ascent: 0.0,
                line_index: 0,
                is_whitespace: false,
            });
        }

        let current_glyphs = &mut self
            .result
            .fragments
            .last_mut()
            .expect("a fragment was just pushed")
            .glyphs;
        current_glyphs.runs.push(run.clone());
        current_glyphs
            .glyph_ids
            .extend_from_slice(&glyphs[..run.size]);
        current_glyphs.glyph_pos.extend_from_slice(&pos[..run.size]);

        for &glyph in &glyphs[..run.size] {
            self.result.missing_glyph_count += usize::from(glyph == MISSING_GLYPH_ID);
        }

        if self.desc.flags.contains(Flags::CLUSTERS) {
            let current_glyphs = &mut self
                .result
                .fragments
                .last_mut()
                .expect("a fragment was just pushed")
                .glyphs;
            current_glyphs.clusters.reserve(run.size);
            for &cluster in &clusters[..run.size] {
                current_glyphs
                    .clusters
                    .push(self.utf8_offset + cluster as usize);
            }
        }
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L142-L145 (chrome/m156), the empty
    // line.
    fn shape_empty_line(&mut self) {
        // SkShaper doesn't care for empty lines.
        self.begin_line();
        self.commit_line();

        // The calls above perform bookkeeping, but they do not add any fragments (since there
        // are no runs to commit).
        //
        // Certain Skottie features (line-based range selectors) do require accurate indexing
        // information even for empty lines though -- so we inject empty fragments solely for
        // line index tracking.
        //
        // Note: we don't add empty fragments in consolidated mode because 1) consolidated mode
        // assumes there is a single result fragment and 2) kFragmentGlyphs is always enabled
        // for cases where line index tracking is relevant.
        //
        // TODO(fmalita): investigate whether it makes sense to move this special case down
        // to commitFragmentedRun().
        if self.desc.flags.contains(Flags::FRAGMENT_GLYPHS) {
            self.result.fragments.push(Fragment {
                glyphs: ShapedGlyphs::default(),
                origin: Point::new(self.box_.x(), self.box_.y()),
                advance: 0.0,
                ascent: 0.0,
                line_index: self.line_count.wrapping_sub(1),
                is_whitespace: false,
            });
        }
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L124-L224 (chrome/m156)
    fn shape_line(&mut self, line: &[u8], utf8_offset: usize) {
        if self.shaper.is_none() {
            return;
        }

        if line.is_empty() {
            self.shape_empty_line();
            return;
        }

        // In default paragraph mode (VAlign::kTop), AE clips out lines when the baseline
        // goes below the box lower edge.
        if self.desc.v_align == VAlign::Top {
            // fOffset is relative to the first line baseline.
            let max_offset = self.box_.height() + self.ascent(); // NB: ascent is negative
            if self.offset.y > max_offset {
                return;
            }
        }

        let shape_width = if self.desc.linebreak == LinebreakPolicy::Explicit {
            SCALAR_MAX
        } else {
            self.box_.width()
        };
        let shape_ltr = self.desc.direction == Direction::LTR;
        let shape_track = self.tracking_em();
        let utf8_bytes = line.len();

        let mut lang_iter = match &self.desc.locale {
            Some(locale) => TrivialLanguageRunIterator::new(locale, utf8_bytes),
            None => make_std_language_run_iterator(utf8_bytes),
        };
        let fallback = self.font_mgr.clone().unwrap_or_else(FontMgr::empty);
        let request_style = self.font.typeface().font_style();
        let language =
            skia_rust_shaper::run_iterator::LanguageRunIterator::current_language(&lang_iter)
                .to_owned();
        let mut font_iter = make_font_mgr_run_iterator_with_request(
            line,
            &self.font,
            Some(fallback),
            self.desc.font_family.as_deref(),
            request_style,
            Some(&language),
        );

        let level = if shape_ltr {
            BIDI_LEVEL_LTR
        } else {
            BIDI_LEVEL_RTL
        };
        // The factory always makes a bidi iterator (a trivial one when the Unicode
        // implementation cannot).
        let mut bidi_iter: Box<dyn BiDiRunIterator> =
            self.shaping_factory.make_bidi_run_iterator(line, level);

        let unknown_script = set_four_byte_tag(b'Z', b'z', b'z', b'z');
        let mut scpt_iter: Box<dyn ScriptRunIterator> = self
            .shaping_factory
            .make_script_run_iterator(line, unknown_script);

        self.utf8.clear();
        self.utf8.extend_from_slice(line);
        self.utf8_offset = utf8_offset;
        let shaper = self.shaper.take().expect("checked above");
        shaper.shape(
            line,
            &mut font_iter,
            bidi_iter.as_mut(),
            scpt_iter.as_mut(),
            &mut lang_iter,
            &[],
            &Options {
                width: shape_width,
                tracking: shape_track,
            },
            self,
        );
        self.shaper = Some(shaper);
        self.utf8.clear();
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L36-L41 and #L70-L76 (chrome/m156)
    // (`finalize`)
    fn finalize(mut self, shaped_size: Option<&mut Size>) -> ShapeResult {
        if !self.desc.flags.contains(Flags::FRAGMENT_GLYPHS) {
            // All glyphs (if any) are pending in a single fragment.
            debug_assert!(self.result.fragments.len() <= 1);
        }

        let ascent = self.ascent();

        // For visual VAlign modes, we use a hybrid extent box computed as the union of
        // actual visual bounds and the vertical typographical extent.
        //
        // This ensures that
        //
        //   a) text doesn't visually overflow the alignment boundaries
        //
        //   b) leading/trailing empty lines are still taken into account for alignment purposes
        let extent_box = |this: &Self, include_typographical_extent: bool| {
            let mut box_ = this.result.compute_visual_bounds();

            if include_typographical_extent {
                // Hybrid visual alignment mode, based on typographical extent.

                // By default, first line is vertically-aligned on a baseline of 0.
                // The typographical height considered for vertical alignment is the distance
                // between the first line top (ascent) to the last line bottom (descent).
                let typographical_top = this.box_.top + ascent;
                #[allow(clippy::cast_precision_loss)] // mirrors the size_t to float conversion
                let typographical_bottom = this.box_.top
                    + this.last_line_descent
                    + this.desc.line_height
                        * (if this.line_count > 0 {
                            (this.line_count - 1) as usize
                        } else {
                            0
                        }) as f32;

                box_.top = std_min(box_.top, typographical_top);
                box_.bottom = std_max(box_.bottom, typographical_bottom);
            }

            box_
        };

        // Only compute the extent box when needed.
        let mut ebox: Option<Rect> = None;

        // Vertical adjustments.
        let mut v_offset = -self.desc.line_shift;

        match self.desc.v_align {
            VAlign::Top => {
                v_offset -= ascent;
            }
            VAlign::TopBaseline => {
                // Default behavior.
            }
            VAlign::HybridTop | VAlign::VisualTop => {
                let e = extent_box(&self, self.desc.v_align == VAlign::HybridTop);
                v_offset += self.box_.top - e.top;
                ebox = Some(e);
            }
            VAlign::HybridCenter | VAlign::VisualCenter => {
                let e = extent_box(&self, self.desc.v_align == VAlign::HybridCenter);
                v_offset += self.box_.center_y() - e.center_y();
                ebox = Some(e);
            }
            VAlign::HybridBottom | VAlign::VisualBottom => {
                let e = extent_box(&self, self.desc.v_align == VAlign::HybridBottom);
                v_offset += self.box_.bottom - e.bottom;
                ebox = Some(e);
            }
        }

        if let Some(shaped_size) = shaped_size {
            let e = ebox.unwrap_or_else(|| extent_box(&self, true));
            *shaped_size = Size::new(e.width(), e.height());
        }

        #[allow(clippy::float_cmp)] // C++ `if (v_offset)`
        if v_offset != 0.0 {
            for fragment in &mut self.result.fragments {
                fragment.origin.y += v_offset;
            }
        }

        self.result
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L111-L123 (chrome/m156)
    fn commit_runs(&mut self) {
        let fragmented = self.desc.flags.contains(Flags::FRAGMENT_GLYPHS);

        let glyphs = std::mem::take(&mut self.line_glyphs);
        let pos = std::mem::take(&mut self.line_pos);
        let clusters = std::mem::take(&mut self.line_clusters);
        let runs = std::mem::take(&mut self.line_runs);

        let mut run_offset = 0;
        for rec in &runs {
            debug_assert!(run_offset < self.line_glyph_count);
            let (g, p, c) = (
                &glyphs[run_offset..],
                &pos[run_offset..],
                &clusters[run_offset..],
            );
            if fragmented {
                self.commit_fragmented_run(rec, g, p, c, self.line_count);
            } else {
                self.commit_consolidated_run(rec, g, p, c, self.line_count);
            }
            run_offset += rec.size;
        }

        self.line_glyphs = glyphs;
        self.line_pos = pos;
        self.line_clusters = clusters;
        self.line_runs = runs;
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L106-L141 (chrome/m156), the
    // `adjust_trailing_whitespace` lambda of `commitLine`.
    fn adjust_trailing_whitespace(&mut self) {
        // For left-alignment, trailing WS doesn't make any difference.
        if self.line_runs.is_empty() || self.desc.h_align == Align::Left {
            return;
        }

        // Technically, trailing whitespace could span multiple runs, but realistically,
        // SkShaper has no reason to split it.  Hence we're only checking the last run.
        let last_run = self.line_runs.last().expect("checked above").clone();
        let mut ws_count = 0_usize;
        for i in 0..last_run.size {
            let cluster = self.line_clusters[self.line_glyph_count - i - 1];
            if is_whitespace(self.utf8[cluster as usize]) {
                ws_count += 1;
            } else {
                break;
            }
        }

        // No trailing whitespace.
        if ws_count == 0 {
            return;
        }

        // Compute the cumulative whitespace advance.  The shaper also appends tracking after
        // each of these glyphs, so include it to fully compensate for trailing whitespace.
        self.advance_buffer.resize(ws_count, 0.0);
        last_run.font.get_widths(
            &self.line_glyphs[self.line_glyph_count - ws_count..self.line_glyph_count],
            &mut self.advance_buffer[..ws_count],
        );

        // std::accumulate(begin, end, 0.0f)
        let mut sum = 0.0_f32;
        for &advance in &self.advance_buffer {
            sum += advance;
        }
        #[allow(clippy::cast_precision_loss)] // mirrors the size_t to float conversion
        let ws_advance = sum + ws_count as f32 * self.tracking_advance();

        // Offset needed to compensate for whitespace.
        let offset = ws_advance * -self.h_align_factor;

        // Shift the whole line horizontally by the computed offset.
        for pos in &mut self.line_pos[..self.line_glyph_count] {
            *pos = Point::new(pos.x + offset, pos.y);
        }
    }
}

// Port of: modules/skottie/src/text/TextShaper.cpp#L236-L247 (chrome/m156) (`HAlignFactor`)
fn h_align_factor(align: Align) -> f32 {
    match align {
        Align::Left => 0.0,
        Align::Center => -0.5,
        Align::Right => -1.0,
    }
}

impl RunHandler for ResultBuilder<'_> {
    // Port of: modules/skottie/src/text/TextShaper.cpp#L79-L90 (chrome/m156)
    fn begin_line(&mut self) {
        self.line_glyphs.clear();
        self.line_pos.clear();
        self.line_clusters.clear();
        self.line_runs.clear();
        self.line_glyph_count = 0;

        self.current_position = self.offset;
        self.pending_line_advance = Vector::new(0.0, 0.0);

        self.last_line_descent = 0.0;
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L92-L100 (chrome/m156)
    fn run_info(&mut self, info: &RunInfo<'_>) {
        self.pending_line_advance += info.advance;

        let (_, metrics) = info.font.metrics();
        if self.line_count == 0 {
            self.first_line_ascent = std_min(self.first_line_ascent, metrics.ascent);
        }
        self.last_line_descent = std_max(self.last_line_descent, metrics.descent);
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L102 (chrome/m156)
    fn commit_run_info(&mut self) {}

    // Port of: modules/skottie/src/text/TextShaper.cpp#L104-L132 (chrome/m156)
    fn run_buffer(&mut self, info: &RunInfo<'_>) -> Buffer<'_> {
        let run_start_index = self.line_glyph_count;
        self.line_glyph_count += info.glyph_count;

        self.line_glyphs.resize(self.line_glyph_count, 0);
        self.line_pos
            .resize(self.line_glyph_count, Point::new(0.0, 0.0));
        self.line_clusters.resize(self.line_glyph_count, 0);
        self.line_runs.push(RunRec {
            font: info.font.clone(),
            size: info.glyph_count,
        });

        // Tracking is inter-glyph: the shaper appends it after every glyph, including the last
        // one on the line, but that trailing slack should not count toward the line width (else
        // it would e.g. push right-aligned text off the right edge).  runBuffer() is only called
        // for non-empty runs, so the line has at least one glyph -> discount one trailing unit.
        let line_advance = self.pending_line_advance.x - self.tracking_advance();

        let alignment_offset = Vector::new(
            self.h_align_factor * (line_advance - self.box_.width()),
            0.0,
        );

        let point = Point::new(
            self.current_position.x + alignment_offset.x,
            self.current_position.y + alignment_offset.y,
        );
        let range = run_start_index..self.line_glyph_count;
        Buffer {
            glyphs: &mut self.line_glyphs[range.clone()],
            positions: &mut self.line_pos[range.clone()],
            offsets: None,
            clusters: Some(&mut self.line_clusters[range]),
            point,
        }
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L134-L136 (chrome/m156)
    fn commit_run_buffer(&mut self, info: &RunInfo<'_>) {
        self.current_position += info.advance;
    }

    // Port of: modules/skottie/src/text/TextShaper.cpp#L138-L184 (chrome/m156)
    fn commit_line(&mut self) {
        self.offset.y += self.desc.line_height;

        // Observed AE handling of whitespace, for alignment purposes:
        //
        //   - leading whitespace contributes to alignment
        //   - trailing whitespace is ignored
        //   - auto line breaking retains all separating whitespace on the first line (no artificial
        //     leading WS is created).
        self.adjust_trailing_whitespace();

        self.commit_runs();

        self.line_count += 1;
    }
}

// Port of: modules/skottie/src/text/TextShaper.cpp#L400-L425 (chrome/m156) (`ShapeImpl`)
fn shape_impl(
    txt: &str,
    desc: &TextDesc,
    box_: &Rect,
    fontmgr: Option<&FontMgr>,
    shaping_factory: &Arc<dyn Factory>,
    shaped_size: Option<&mut Size>,
) -> ShapeResult {
    // TODO: other explicit breaks?
    let is_line_break = |uch: utf::Unichar| uch == utf::Unichar::from(b'\r');

    let bytes = txt.as_bytes();
    let mut rbuilder = ResultBuilder::new(desc, box_, fontmgr, shaping_factory);

    let mut ptr = bytes;
    let mut line_start = 0;
    while !ptr.is_empty() {
        let uch = utf::next_utf8(&mut ptr);
        let consumed = bytes.len() - ptr.len();
        if is_line_break(uch) {
            rbuilder.shape_line(&bytes[line_start..consumed - 1], line_start);
            line_start = consumed;
        }
    }
    rbuilder.shape_line(&bytes[line_start..bytes.len()], line_start);

    rbuilder.finalize(shaped_size)
}

// Port of: modules/skottie/src/text/TextShaper.cpp#L427-L442 (chrome/m156) (`result_fits`)
fn result_fits(res: &ShapeResult, res_size: Size, box_: &Rect, desc: &TextDesc) -> bool {
    // optional max line count constraint
    if desc.max_lines != 0 {
        let line_count = match res.fragments.last() {
            None => 0,
            Some(fragment) => fragment.line_index as usize + 1,
        };
        if line_count > desc.max_lines {
            return false;
        }
    }

    // geometric constraint
    res_size.width <= box_.width() && res_size.height <= box_.height()
}

// Port of: modules/skottie/src/text/TextShaper.cpp#L463 (chrome/m156) (`kMaxIter`)
const MAX_ITER: usize = 16;

// Port of: modules/skottie/src/text/TextShaper.cpp#L444-L513 (chrome/m156) (`ShapeToFit`)
#[allow(clippy::manual_midpoint)] // the arithmetic of C++: `(in_scale + out_scale) * 0.5f`
fn shape_to_fit(
    txt: &str,
    orig_desc: &TextDesc,
    box_: &Rect,
    fontmgr: Option<&FontMgr>,
    shaping_factory: &Arc<dyn Factory>,
) -> ShapeResult {
    let mut best_result = ShapeResult::default();

    if box_.is_empty() || orig_desc.text_size <= 0.0 {
        return best_result;
    }

    let mut desc = orig_desc.clone();

    let min_scale = std_max(desc.min_text_size / desc.text_size, 0.0);
    let max_scale = std_max(desc.max_text_size / desc.text_size, min_scale);

    let mut in_scale = min_scale; // maximum scale that fits inside
    let mut out_scale = max_scale; // minimum scale that doesn't fit
    let mut try_scale = t_pin(1.0_f32, min_scale, max_scale); // current probe

    // Perform a binary search for the best vertical fit (SkShaper already handles
    // horizontal fitting), starting with the specified text size.
    //
    // This hybrid loop handles both the binary search (when in/out extremes are known), and an
    // exponential search for the extremes.
    for _ in 0..MAX_ITER {
        debug_assert!(try_scale >= in_scale && try_scale <= out_scale);
        desc.text_size = try_scale * orig_desc.text_size;
        desc.line_height = try_scale * orig_desc.line_height;
        desc.line_shift = try_scale * orig_desc.line_shift;
        desc.ascent = try_scale * orig_desc.ascent;

        let mut res_size = Size::new(0.0, 0.0);
        let res = shape_impl(
            txt,
            &desc,
            box_,
            fontmgr,
            shaping_factory,
            Some(&mut res_size),
        );

        let prev_scale = try_scale;
        if result_fits(&res, res_size, box_, &desc) {
            // It fits - so it's a candidate.
            best_result = res;
            best_result.scale = try_scale;

            in_scale = try_scale;
            #[allow(clippy::float_cmp)] // C++ `out_scale == max_scale`
            let out_scale_unknown = out_scale == max_scale;
            try_scale = if out_scale_unknown {
                // initial out_scale not found yet - search exponentially
                std_min(max_scale, try_scale * 2.0)
            } else {
                // out_scale found - binary search
                (in_scale + out_scale) * 0.5
            };
        } else {
            out_scale = try_scale;
            #[allow(clippy::float_cmp)] // C++ `in_scale == min_scale`
            let in_scale_unknown = in_scale == min_scale;
            try_scale = if in_scale_unknown {
                // initial in_scale not found yet - search exponentially
                std_max(min_scale, try_scale * 0.5)
            } else {
                // in_scale found - binary search
                (in_scale + out_scale) * 0.5
            };
        }

        #[allow(clippy::float_cmp)] // C++ `try_scale == prev_scale`
        if try_scale == prev_scale {
            // no more progress
            break;
        }
    }

    best_result
}

// Applies capitalization rules.
// Port of: modules/skottie/src/text/TextShaper.cpp#L516-L540 (chrome/m156) (`AdjustedText`)
fn adjusted_text(
    txt: &str,
    desc: &TextDesc,
    unicode: Option<&dyn skia_rust_unicode::unicode::Unicode>,
) -> String {
    match desc.capitalization {
        Capitalization::None => txt.to_owned(),
        Capitalization::UpperCase => match unicode {
            Some(unicode) => unicode.to_upper(txt),
            None => txt.to_owned(),
        },
    }
}
