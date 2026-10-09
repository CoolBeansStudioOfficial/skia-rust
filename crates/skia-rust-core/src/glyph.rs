// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGlyph.{h,cpp}, src/core/SkDrawable.cpp (SkPictureBackedGlyphDrawable)

//! The glyph record of a strike (`SkGlyph`): metrics, the mask image, the path or drawable of a
//! glyph, and the compact [`GlyphDigest`] that a strike keeps per glyph.
//!
//! Not in this module yet: the parts that create glyph data from a scaler context
//! (`SkGlyph::setImage(alloc, scalerContext)` and `setPath`/`setDrawable` with a scaler context;
//! they arrive with T6), the arena-based setters (Rust owns the buffers instead), and the
//! buffer flattening (`flattenMetrics`, `flattenImage`, `flattenPath`, `flattenDrawable`, the
//! `addXFromBuffer` family), which need `SkWriteBuffer`/`SkReadBuffer`.

use crate::canvas::Canvas;
use crate::checksum::cheap_mix;
use crate::drawable::{Drawable, DrawableBase};
use crate::font_types::GlyphId;
use crate::mask::{Mask, MaskFormat};
use crate::packed_glyph_id::PackedGlyphId;
use crate::path::Path;
use crate::picture::Picture;
use crate::point::{IPoint, Point};
use crate::read_buffer::ReadBuffer;
use crate::rect::IRect;
use crate::rect::Rect;
use crate::scalar::{scalar, scalar_floor_to_int};
use crate::scaler_context::AxisAlignment;
use crate::write_buffer::BinaryWriteBuffer;

/// `SkGlyph::kMaxGlyphWidth`: glyphs at least this wide have no image in the atlas.
// Port of: src/core/SkGlyph.h#L573 (chrome/m156)
const MAX_GLYPH_WIDTH: u16 = 1 << 13;

/// `sizeof(SkPictureBackedGlyphDrawable)` on 64-bit targets: the vtable pointer, the generation
/// id and the picture handle. Used only for memory accounting, as in C++.
// Port of: src/core/SkGlyph.h#L409-L421 (sizeof, chrome/m156)
const PICTURE_BACKED_GLYPH_DRAWABLE_SIZE: usize = 24;

/// `SkGlyphRect`'s helper namespace `skglyph`.
// Port of: src/core/SkGlyph.h#L243-L250 (chrome/m156)
pub mod skglyph {
    use super::GlyphRect;

    /// `skglyph::rect_union`: the smallest rect holding both.
    // Port of: src/core/SkGlyph.h#L297-L299 (chrome/m156)
    #[must_use]
    pub fn rect_union(a: GlyphRect, b: GlyphRect) -> GlyphRect {
        GlyphRect {
            rect: vmax(a.rect, b.rect),
        }
    }

    /// `skglyph::rect_intersection`: the overlap of both.
    // Port of: src/core/SkGlyph.h#L300-L302 (chrome/m156)
    #[must_use]
    pub fn rect_intersection(a: GlyphRect, b: GlyphRect) -> GlyphRect {
        GlyphRect {
            rect: vmin(a.rect, b.rect),
        }
    }

    /// `skglyph::empty_rect`: the identity of [`rect_union`].
    // Port of: src/core/SkGlyph.h#L289-L292 (chrome/m156)
    #[must_use]
    pub fn empty_rect() -> GlyphRect {
        let max = f32::MAX;
        GlyphRect::new(max, max, -max, -max)
    }

    /// `skglyph::full_rect`: the identity of [`rect_intersection`].
    // Port of: src/core/SkGlyph.h#L293-L296 (chrome/m156)
    #[must_use]
    pub fn full_rect() -> GlyphRect {
        let max = f32::MAX;
        GlyphRect::new(-max, -max, max, max)
    }

    /// Lane-wise maximum, as `skvx::max` (the `a > b ? a : b` of `_mm_max_ps`).
    // Port of: src/core/SkGlyph.h#L297-L299 (skvx::max, chrome/m156)
    pub(super) fn vmax(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
        [0, 1, 2, 3].map(|i| if a[i] > b[i] { a[i] } else { b[i] })
    }

    /// Lane-wise minimum, as `skvx::min` (the `a < b ? a : b` of `_mm_min_ps`).
    // Port of: src/core/SkGlyph.h#L300-L302 (skvx::min, chrome/m156)
    pub(super) fn vmin(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
        [0, 1, 2, 3].map(|i| if a[i] < b[i] { a[i] } else { b[i] })
    }
}

/// A rectangle stored as `(-left, -top, right, bottom)`, so that union and intersection are lane-
/// wise max and min (`SkGlyphRect`).
// Port of: src/core/SkGlyph.h#L251-L287 (chrome/m156)
#[doc(alias = "SkGlyphRect")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlyphRect {
    rect: [scalar; 4],
}

impl GlyphRect {
    /// `SkGlyphRect(left, top, right, bottom)`.
    // Port of: src/core/SkGlyph.h#L254-L255 (chrome/m156)
    #[must_use]
    pub fn new(left: scalar, top: scalar, right: scalar, bottom: scalar) -> Self {
        Self {
            rect: [-left, -top, right, bottom],
        }
    }

    /// `SkGlyphRect::empty`: true if the width or the height is not positive.
    // Port of: src/core/SkGlyph.h#L256-L258 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        -self.rect[0] >= self.rect[2] || -self.rect[1] >= self.rect[3]
    }

    /// `SkGlyphRect::rect`: the rectangle as an `SkRect`.
    // Port of: src/core/SkGlyph.h#L259-L261 (chrome/m156)
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_ltrb(-self.rect[0], -self.rect[1], self.rect[2], self.rect[3])
    }

    /// `SkGlyphRect::offset(x, y)`.
    // Port of: src/core/SkGlyph.h#L262-L264 (chrome/m156)
    #[must_use]
    pub fn offset(&self, x: scalar, y: scalar) -> Self {
        let d = [-x, -y, x, y];
        Self {
            rect: [0, 1, 2, 3].map(|i| self.rect[i] + d[i]),
        }
    }

    /// `SkGlyphRect::offset(pt)`.
    // Port of: src/core/SkGlyph.h#L265-L267 (chrome/m156)
    #[must_use]
    pub fn offset_point(&self, pt: Point) -> Self {
        self.offset(pt.x, pt.y)
    }

    /// `SkGlyphRect::scaleAndOffset`: `rect * scale + (-x, -y, x, y)`.
    // Port of: src/core/SkGlyph.h#L268-L271 (chrome/m156)
    #[must_use]
    pub fn scale_and_offset(&self, scale: scalar, offset: Point) -> Self {
        let (x, y) = (offset.x, offset.y);
        let d = [-x, -y, x, y];
        Self {
            rect: [0, 1, 2, 3].map(|i| self.rect[i] * scale + d[i]),
        }
    }

    /// `SkGlyphRect::inset(dx, dy)`: `rect - (dx, dy, dx, dy)`.
    // Port of: src/core/SkGlyph.h#L272-L274 (chrome/m156)
    #[must_use]
    pub fn inset(&self, dx: scalar, dy: scalar) -> Self {
        let d = [dx, dy, dx, dy];
        Self {
            rect: [0, 1, 2, 3].map(|i| self.rect[i] - d[i]),
        }
    }

    /// `SkGlyphRect::leftTop`.
    // Port of: src/core/SkGlyph.h#L275 (chrome/m156)
    #[must_use]
    pub fn left_top(&self) -> Point {
        Point::new(-self.rect[0], -self.rect[1])
    }

    /// `SkGlyphRect::rightBottom`.
    // Port of: src/core/SkGlyph.h#L276 (chrome/m156)
    #[must_use]
    pub fn right_bottom(&self) -> Point {
        Point::new(self.rect[2], self.rect[3])
    }

    /// `SkGlyphRect::widthHeight`: `rightBottom + negLeftTop`.
    // Port of: src/core/SkGlyph.h#L277 (chrome/m156)
    #[must_use]
    pub fn width_height(&self) -> Point {
        Point::new(self.rect[2] + self.rect[0], self.rect[3] + self.rect[1])
    }
}

/// What a strike should do with a glyph for one kind of drawing (`skglyph::GlyphAction`).
// Port of: src/core/SkGlyph.h#L304-L311 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GlyphAction {
    /// Not decided yet.
    Unset = 0,
    /// Use the glyph for this drawing.
    Accept = 1,
    /// Do not use the glyph for this drawing; try the next.
    Reject = 2,
    /// The glyph draws nothing; drop it.
    Drop = 3,
}

/// The kinds of drawing a glyph can be prepared for, as the shift of their 2-bit action
/// (`skglyph::ActionType`).
// Port of: src/core/SkGlyph.h#L312-L320 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ActionType {
    /// Direct mask (GPU).
    DirectMask = 0,
    /// Direct mask on the CPU.
    DirectMaskCpu = 2,
    /// Mask (GPU).
    Mask = 4,
    /// Signed distance field text (GPU).
    Sdft = 6,
    /// Path.
    Path = 8,
    /// Drawable.
    Drawable = 10,
}

/// `skglyph::ActionTypeSize::kTotalBits`: the bits used by all the actions together.
// Port of: src/core/SkGlyph.h#L321-L324 (chrome/m156)
const ACTION_TOTAL_BITS: u32 = 12;

/// The per-glyph record a strike keeps: the packed id, the glyph's index in the strike's list,
/// the mask format, whether it is empty, the action for each kind of drawing, and the bounds
/// (`SkGlyphDigest`).
///
/// The C++ bit-fields are packed into one `u64` in the same order (least significant first):
/// `packedID` (20 bits), `index` (20), `isEmpty` (1), `format` (3), `actions` (12).
// Port of: src/core/SkGlyph.h#L329-L407 (chrome/m156)
#[doc(alias = "SkGlyphDigest")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphDigest {
    bits: u64,
    left: i16,
    top: i16,
    width: u16,
    height: u16,
}

impl GlyphDigest {
    /// `kSkSideTooBigForAtlas`: the largest side a glyph may have to go in the atlas.
    // Port of: src/core/SkGlyph.h#L333 (chrome/m156)
    pub const SIDE_TOO_BIG_FOR_ATLAS: u16 = 256;

    const PACKED_ID_BITS: u32 = 20;
    const INDEX_SHIFT: u32 = 20;
    const EMPTY_SHIFT: u32 = 40;
    const FORMAT_SHIFT: u32 = 41;
    const ACTIONS_SHIFT: u32 = 44;
    const FORMAT_MASK: u64 = 0b111;
    const ACTION_MASK: u64 = 0b11;

    /// `SkGlyphDigest(size_t index, const SkGlyph& glyph)`. Empty glyphs start with every action
    /// set to `Drop`.
    // Port of: src/core/SkGlyph.cpp#L614-L638 (chrome/m156)
    #[must_use]
    pub fn new(index: usize, glyph: &Glyph) -> Self {
        let packed = u64::from(glyph.packed_id().value());
        let index = index as u64;
        let is_empty = glyph.is_empty();
        let format = glyph.mask_format() as u64;
        // init_actions: every action is Drop for an empty glyph, Unset otherwise.
        let actions: u64 = if is_empty {
            [0_u32, 2, 4, 6, 8, 10]
                .iter()
                .map(|shift| (GlyphAction::Drop as u64) << shift)
                .sum()
        } else {
            0
        };
        let bits = packed
            | (index << Self::INDEX_SHIFT)
            | (u64::from(is_empty) << Self::EMPTY_SHIFT)
            | (format << Self::FORMAT_SHIFT)
            | (actions << Self::ACTIONS_SHIFT);
        Self {
            bits,
            left: i16::try_from(glyph.left()).unwrap_or(0),
            top: i16::try_from(glyph.top()).unwrap_or(0),
            width: glyph.width(),
            height: glyph.height(),
        }
    }

    /// `SkGlyphDigest::index`: the glyph's position in the strike's glyph list.
    // Port of: src/core/SkGlyph.h#L338 (chrome/m156)
    #[must_use]
    pub fn index(&self) -> usize {
        ((self.bits >> Self::INDEX_SHIFT) & ((1 << Self::PACKED_ID_BITS) - 1)) as usize
    }

    /// `SkGlyphDigest::isEmpty`.
    // Port of: src/core/SkGlyph.h#L339 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        (self.bits >> Self::EMPTY_SHIFT) & 1 != 0
    }

    /// `SkGlyphDigest::isColor`: the format is ARGB32.
    // Port of: src/core/SkGlyph.h#L340 (chrome/m156)
    #[must_use]
    pub fn is_color(&self) -> bool {
        self.mask_format() == MaskFormat::Argb32
    }

    /// `SkGlyphDigest::maskFormat`.
    // Port of: src/core/SkGlyph.h#L341 (chrome/m156)
    #[must_use]
    pub fn mask_format(&self) -> MaskFormat {
        match (self.bits >> Self::FORMAT_SHIFT) & Self::FORMAT_MASK {
            0 => MaskFormat::BW,
            1 => MaskFormat::A8,
            2 => MaskFormat::ThreeD,
            3 => MaskFormat::Argb32,
            4 => MaskFormat::Lcd16,
            _ => MaskFormat::Sdf,
        }
    }

    /// `SkGlyphDigest::actionFor`: the action for `action_type`.
    // Port of: src/core/SkGlyph.h#L343-L345 (chrome/m156)
    #[must_use]
    pub fn action_for(&self, action_type: ActionType) -> GlyphAction {
        let actions = (self.bits >> Self::ACTIONS_SHIFT) & ((1 << ACTION_TOTAL_BITS) - 1);
        match (actions >> action_type as u32) & Self::ACTION_MASK {
            0 => GlyphAction::Unset,
            1 => GlyphAction::Accept,
            2 => GlyphAction::Reject,
            _ => GlyphAction::Drop,
        }
    }

    /// `SkGlyphDigest::setAction` (private in C++): records the action for `action_type`, which
    /// must still be `Unset`. `setActionFor` (which also sets the strike's pending work) arrives
    /// with the strike.
    // Port of: src/core/SkGlyph.h#L386-L393 (chrome/m156)
    pub(crate) fn set_action(&mut self, action_type: ActionType, action: GlyphAction) {
        assert_ne!(action, GlyphAction::Unset);
        assert_eq!(self.action_for(action_type), GlyphAction::Unset);
        let shift = Self::ACTIONS_SHIFT + action_type as u32;
        let mask = Self::ACTION_MASK << shift;
        self.bits &= !mask;
        self.bits |= (action as u64) << shift;
    }

    /// `SkGlyphDigest::maxDimension`.
    // Port of: src/core/SkGlyph.h#L349-L351 (chrome/m156)
    #[must_use]
    pub fn max_dimension(&self) -> u16 {
        self.width.max(self.height)
    }

    /// `SkGlyphDigest::fitsInAtlasDirect`.
    // Port of: src/core/SkGlyph.h#L353-L355 (chrome/m156)
    #[must_use]
    pub fn fits_in_atlas_direct(&self) -> bool {
        self.max_dimension() <= Self::SIDE_TOO_BIG_FOR_ATLAS
    }

    /// `SkGlyphDigest::fitsInAtlasInterpolated`: two pixels smaller, for the filter border.
    // Port of: src/core/SkGlyph.h#L357-L360 (chrome/m156)
    #[must_use]
    pub fn fits_in_atlas_interpolated(&self) -> bool {
        self.max_dimension() <= Self::SIDE_TOO_BIG_FOR_ATLAS - 2
    }

    /// `SkGlyphDigest::bounds`.
    // Port of: src/core/SkGlyph.h#L362-L364 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> GlyphRect {
        GlyphRect::new(
            f32::from(self.left),
            f32::from(self.top),
            f32::from(self.left) + f32::from(self.width),
            f32::from(self.top) + f32::from(self.height),
        )
    }

    /// `SkGlyphDigest::FitsInAtlas`.
    // Port of: src/core/SkGlyph.cpp#L690-L692 (chrome/m156)
    #[must_use]
    pub fn fits_in_atlas(glyph: &Glyph) -> bool {
        glyph.max_dimension() <= Self::SIDE_TOO_BIG_FOR_ATLAS
    }

    /// `SkGlyphDigest::GetKey`: the packed id of a digest.
    // Port of: src/core/SkGlyph.h#L369-L371 (chrome/m156)
    #[must_use]
    pub fn key(&self) -> PackedGlyphId {
        PackedGlyphId::from_raw((self.bits & ((1 << Self::PACKED_ID_BITS) - 1)) as u32)
    }

    /// `SkGlyphDigest::Hash`.
    // Port of: src/core/SkGlyph.h#L372-L374 (chrome/m156)
    #[must_use]
    pub fn hash(packed_id: PackedGlyphId) -> u32 {
        cheap_mix(packed_id.value())
    }

    /// `SkGlyphDigest::ShouldGrow`.
    // Port of: src/core/SkGlyph.h#L375-L379 (chrome/m156)
    #[must_use]
    pub fn should_grow(count: i32, capacity: i32) -> bool {
        2 * count >= capacity
    }

    /// `SkGlyphDigest::ShouldShrink`.
    // Port of: src/core/SkGlyph.h#L380-L383 (chrome/m156)
    #[must_use]
    pub fn should_shrink(count: i32, capacity: i32) -> bool {
        6 * count <= capacity
    }
}

/// The path of a glyph, once it has been set (`SkGlyph::PathData`).
#[derive(Clone, Debug)]
struct PathData {
    path: Option<Path>,
    hairline: bool,
    modified: bool,
}

/// The drawable of a glyph, once it has been set (`SkGlyph::DrawableData`).
#[derive(Clone, Debug)]
struct DrawableData {
    drawable: Option<Drawable>,
}

/// The metrics, image, path and drawable of one glyph in a strike (`SkGlyph`).
///
/// The metric fields are `pub(crate)`: the scaler context fills them, as in C++. Images, paths
/// and drawables are owned here instead of living in an arena; "has been called" is tracked the
/// way C++ tracks it (`setImageHasBeenCalled` and friends).
// Port of: src/core/SkGlyph.h#L422-L654 (chrome/m156)
#[doc(alias = "SkGlyph")]
#[derive(Clone, Debug)]
pub struct Glyph {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) left: i16,
    pub(crate) top: i16,
    image: Option<Box<[u8]>>,
    path_data: Option<PathData>,
    drawable_data: Option<DrawableData>,
    advance_x: scalar,
    advance_y: scalar,
    mask_format: MaskFormat,
    scaler_context_bits: u16,
    id: PackedGlyphId,
}

impl Default for Glyph {
    /// `SkGlyph()`: the glyph with the impossible id and no metrics.
    // Port of: src/core/SkGlyph.h#L426 (chrome/m156)
    fn default() -> Self {
        Self::new(PackedGlyphId::default())
    }
}

impl Glyph {
    /// `explicit SkGlyph(SkPackedGlyphID id)`: no metrics, no image, BW format.
    // Port of: src/core/SkGlyph.h#L432 (chrome/m156)
    #[must_use]
    pub fn new(id: PackedGlyphId) -> Self {
        Self {
            width: 0,
            height: 0,
            left: 0,
            top: 0,
            image: None,
            path_data: None,
            drawable_data: None,
            advance_x: 0.0,
            advance_y: 0.0,
            mask_format: MaskFormat::BW,
            scaler_context_bits: 0,
            id,
        }
    }

    /// `SkGlyph::advanceVector`: the advance as a vector.
    // Port of: src/core/SkGlyph.h#L436-L437 (chrome/m156)
    #[must_use]
    pub fn advance_vector(&self) -> Point {
        Point::new(self.advance_x, self.advance_y)
    }

    /// `SkGlyph::advanceX`.
    // Port of: src/core/SkGlyph.h#L435 (chrome/m156)
    #[must_use]
    pub fn advance_x(&self) -> scalar {
        self.advance_x
    }

    /// `SkGlyph::advanceY`.
    // Port of: src/core/SkGlyph.h#L436 (chrome/m156)
    #[must_use]
    pub fn advance_y(&self) -> scalar {
        self.advance_y
    }

    /// Sets the advances. The scaler context does this when it makes metrics.
    // Port of: src/core/SkGlyph.h#L640-L641 (fAdvanceX, fAdvanceY, chrome/m156)
    #[allow(dead_code)] // consumed by the scaler context, which arrives with T6
    pub(crate) fn set_advances(&mut self, x: scalar, y: scalar) {
        self.advance_x = x;
        self.advance_y = y;
    }

    /// `SkGlyph::getGlyphID`.
    // Port of: src/core/SkGlyph.h#L438 (chrome/m156)
    #[must_use]
    pub fn glyph_id(&self) -> GlyphId {
        self.id.glyph_id()
    }

    /// `SkGlyph::getPackedID`.
    // Port of: src/core/SkGlyph.h#L439 (chrome/m156)
    #[must_use]
    pub fn packed_id(&self) -> PackedGlyphId {
        self.id
    }

    /// `SkGlyph::getSubXFixed`.
    // Port of: src/core/SkGlyph.h#L440 (chrome/m156)
    #[must_use]
    pub fn sub_x_fixed(&self) -> crate::fixed::Fixed {
        self.id.sub_x_fixed()
    }

    /// `SkGlyph::getSubYFixed`.
    // Port of: src/core/SkGlyph.h#L441 (chrome/m156)
    #[must_use]
    pub fn sub_y_fixed(&self) -> crate::fixed::Fixed {
        self.id.sub_y_fixed()
    }

    /// `SkGlyph::zeroMetrics`: zero advances and bounds, keeping the id and format.
    // Port of: src/core/SkGlyph.cpp#L139-L146 (chrome/m156)
    #[allow(dead_code)] // consumed by the scaler context, which arrives with T6
    pub(crate) fn zero_metrics(&mut self) {
        self.advance_x = 0.0;
        self.advance_y = 0.0;
        self.width = 0;
        self.height = 0;
        self.top = 0;
        self.left = 0;
    }

    /// `SkGlyph::rowBytes`: the bytes in one row of the mask.
    // Port of: src/core/SkGlyph.cpp#L232-L234 (chrome/m156)
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        format_rowbytes(self.width, self.mask_format)
    }

    /// `SkGlyph::rowBytesUsingFormat`.
    // Port of: src/core/SkGlyph.cpp#L236-L238 (chrome/m156)
    #[must_use]
    pub fn row_bytes_using_format(&self, format: MaskFormat) -> usize {
        format_rowbytes(self.width, format)
    }

    /// `SkGlyph::imageSize`: the bytes the image occupies, or 0 if there is none.
    // Port of: src/core/SkGlyph.cpp#L240-L250 (chrome/m156)
    #[must_use]
    pub fn image_size(&self) -> usize {
        if self.is_empty() || self.image_too_large() {
            return 0;
        }
        let mut size = self.row_bytes() * usize::from(self.height);
        if self.mask_format == MaskFormat::ThreeD {
            size *= 3;
        }
        size
    }

    /// `SkGlyph::formatAlignment`: the alignment of the image bytes for the format.
    // Port of: src/core/SkGlyph.cpp#L152-L164 (chrome/m156)
    #[must_use]
    pub fn format_alignment(&self) -> usize {
        format_alignment(self.mask_format)
    }

    /// `SkGlyph::setImageHasBeenCalled`: an empty glyph, a glyph with an image, or one too large
    /// for an image counts as done.
    // Port of: src/core/SkGlyph.h#L468-L471 (chrome/m156)
    #[must_use]
    pub fn set_image_has_been_called(&self) -> bool {
        self.is_empty() || self.image.is_some() || self.image_too_large()
    }

    /// `SkGlyph::image`: the mask bytes, or `None` for an empty or too-large glyph.
    // Port of: src/core/SkGlyph.h#L474 (chrome/m156)
    #[must_use]
    pub fn image(&self) -> Option<&[u8]> {
        self.image.as_deref()
    }

    /// `SkGlyph::mask()`: the mask of the glyph at its own bounds (no image gives an empty image).
    // Port of: src/core/SkGlyph.cpp#L127-L130 (chrome/m156)
    #[must_use]
    pub fn mask(&self) -> Mask<'_> {
        Mask::new(
            self.image().unwrap_or(&[]),
            self.i_rect(),
            row_bytes_u32(self.row_bytes()),
            self.mask_format,
        )
    }

    /// `SkGlyph::mask(position)`: the mask with its bounds moved by the floor of `position`. The
    /// position must be integral in the painter, which is the only caller.
    // Port of: src/core/SkGlyph.cpp#L132-L137 (chrome/m156)
    #[must_use]
    pub fn mask_at(&self, position: Point) -> Mask<'_> {
        let mut bounds = self.i_rect();
        bounds.offset((
            scalar_floor_to_int(position.x),
            scalar_floor_to_int(position.y),
        ));
        Mask::new(
            self.image().unwrap_or(&[]),
            bounds,
            row_bytes_u32(self.row_bytes()),
            self.mask_format,
        )
    }

    /// `SkGlyph::setImage(void*)`: installs an image the caller has produced.
    // Port of: src/core/SkGlyph.h#L541 (chrome/m156)
    pub fn set_image(&mut self, image: Box<[u8]>) {
        self.image = Some(image);
    }

    /// `SkGlyph::setPathHasBeenCalled`.
    // Port of: src/core/SkGlyph.h#L495 (chrome/m156)
    #[must_use]
    pub fn set_path_has_been_called(&self) -> bool {
        self.path_data.is_some()
    }

    /// `SkGlyph::setPath(alloc, path, hairline, modified)`: records the path (or that there is
    /// none) once. Returns true if the glyph has a path.
    // Port of: src/core/SkGlyph.cpp#L274-L280 (chrome/m156)
    pub fn set_path(&mut self, path: Option<Path>, hairline: bool, modified: bool) -> bool {
        if self.set_path_has_been_called() {
            return false;
        }
        self.path_data = Some(PathData {
            path,
            hairline,
            modified,
        });
        self.path().is_some()
    }

    /// `SkGlyph::path`: the path, or `None` when the glyph has none (or `setPath` was not
    /// called).
    // Port of: src/core/SkGlyph.cpp#L282-L289 (chrome/m156)
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path_data.as_ref()?.path.as_ref()
    }

    /// `SkGlyph::pathIsHairline`.
    // Port of: src/core/SkGlyph.cpp#L291-L294 (chrome/m156)
    #[must_use]
    pub fn path_is_hairline(&self) -> bool {
        self.path_data.as_ref().is_some_and(|data| data.hairline)
    }

    /// `SkGlyph::pathIsModified`.
    // Port of: src/core/SkGlyph.cpp#L296-L299 (chrome/m156)
    #[must_use]
    pub fn path_is_modified(&self) -> bool {
        self.path_data.as_ref().is_some_and(|data| data.modified)
    }

    /// `SkGlyph::setDrawableHasBeenCalled`.
    // Port of: src/core/SkGlyph.h#L505 (chrome/m156)
    #[must_use]
    pub fn set_drawable_has_been_called(&self) -> bool {
        self.drawable_data.is_some()
    }

    /// `SkGlyph::setDrawable(alloc, drawable)`: records the drawable (or that there is none)
    /// once. Returns true if the glyph has a drawable.
    // Port of: src/core/SkGlyph.cpp#L323-L329 (chrome/m156)
    pub fn set_drawable(&mut self, drawable: Option<Drawable>) -> bool {
        if self.set_drawable_has_been_called() {
            return false;
        }
        self.drawable_data = Some(DrawableData { drawable });
        self.drawable().is_some()
    }

    /// `SkGlyph::drawable`.
    // Port of: src/core/SkGlyph.cpp#L331-L338 (chrome/m156)
    #[must_use]
    pub fn drawable(&self) -> Option<&Drawable> {
        self.drawable_data.as_ref()?.drawable.as_ref()
    }

    /// `SkGlyph::isColor`: the format is ARGB32.
    // Port of: src/core/SkGlyph.h#L509 (chrome/m156)
    #[must_use]
    pub fn is_color(&self) -> bool {
        self.mask_format == MaskFormat::Argb32
    }

    /// `SkGlyph::maskFormat`.
    // Port of: src/core/SkGlyph.h#L510 (chrome/m156)
    #[must_use]
    pub fn mask_format(&self) -> MaskFormat {
        self.mask_format
    }

    /// Sets the mask format. The scaler context chooses it.
    // Port of: src/core/SkGlyph.h#L643 (fMaskFormat, chrome/m156)
    #[allow(dead_code)] // consumed by the scaler context, which arrives with T6
    pub(crate) fn set_mask_format(&mut self, format: MaskFormat) {
        self.mask_format = format;
    }

    /// `SkGlyph::fScalerContextBits`, set from the metrics.
    pub(crate) fn set_scaler_context_bits(&mut self, bits: u16) {
        self.scaler_context_bits = bits;
    }

    /// Takes the image out of the glyph, leaving none (the scaler context fills it and gives it
    /// back).
    pub(crate) fn take_image(&mut self) -> Option<Box<[u8]>> {
        self.image.take()
    }

    /// `SkGlyph::maxDimension`.
    // Port of: src/core/SkGlyph.h#L514 (chrome/m156)
    #[must_use]
    pub fn max_dimension(&self) -> u16 {
        self.width.max(self.height)
    }

    /// `SkGlyph::iRect`.
    // Port of: src/core/SkGlyph.h#L515 (chrome/m156)
    #[must_use]
    pub fn i_rect(&self) -> IRect {
        IRect::from_xywh(
            i32::from(self.left),
            i32::from(self.top),
            i32::from(self.width),
            i32::from(self.height),
        )
    }

    /// `SkGlyph::rect`: the glyph's bounds as a rectangle, from its left, top, width and height.
    // Port of: src/core/SkGlyph.h#L516 (chrome/m156)
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_ltrb(
            f32::from(self.left),
            f32::from(self.top),
            f32::from(self.left) + f32::from(self.width),
            f32::from(self.top) + f32::from(self.height),
        )
    }

    /// `SkGlyph::glyphRect`.
    // Port of: src/core/SkGlyph.h#L517-L519 (chrome/m156)
    #[must_use]
    pub fn glyph_rect(&self) -> GlyphRect {
        GlyphRect::new(
            f32::from(self.left),
            f32::from(self.top),
            f32::from(self.left) + f32::from(self.width),
            f32::from(self.top) + f32::from(self.height),
        )
    }

    /// `SkGlyph::left`.
    // Port of: src/core/SkGlyph.h#L520 (chrome/m156)
    #[must_use]
    pub fn left(&self) -> i32 {
        i32::from(self.left)
    }

    /// `SkGlyph::top`.
    // Port of: src/core/SkGlyph.h#L521 (chrome/m156)
    #[must_use]
    pub fn top(&self) -> i32 {
        i32::from(self.top)
    }

    /// `SkGlyph::width`.
    // Port of: src/core/SkGlyph.h#L522 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// `SkGlyph::height`.
    // Port of: src/core/SkGlyph.h#L523 (chrome/m156)
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// `SkGlyph::isEmpty`: no area.
    // Port of: src/core/SkGlyph.h#L524-L526 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// `SkGlyph::imageTooLarge`.
    // Port of: src/core/SkGlyph.h#L527 (chrome/m156)
    #[must_use]
    pub fn image_too_large(&self) -> bool {
        self.width >= MAX_GLYPH_WIDTH
    }

    /// `SkGlyph::extraBits`: the bits the scaler context stores in the glyph.
    // Port of: src/core/SkGlyph.h#L529 (chrome/m156)
    #[must_use]
    pub fn extra_bits(&self) -> u16 {
        self.scaler_context_bits
    }

    /// Sets the metrics the way `SkGlyphTestPeer::SetGlyph1`/`SetGlyph2` do in
    /// `tests/SkGlyphTest.cpp`, which write the private fields directly. Test-only.
    // Port of: tests/SkGlyphTest.cpp#L65-L92 (SkGlyphTestPeer, chrome/m156)
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ peer, which sets each field
    pub fn set_metrics_for_testing(
        &mut self,
        advance: Point,
        left: i16,
        top: i16,
        width: u16,
        height: u16,
        format: MaskFormat,
    ) {
        self.advance_x = advance.x;
        self.advance_y = advance.y;
        self.left = left;
        self.top = top;
        self.width = width;
        self.height = height;
        self.mask_format = format;
    }
}

/// The mask format with the given `uint8_t` value (`SkMask::Format`), or `None` past the last
/// one.
// Port of: src/core/SkMask.h#L26-L37 (chrome/m156)
fn mask_format_from_u8(format: u8) -> Option<MaskFormat> {
    match format {
        0 => Some(MaskFormat::BW),
        1 => Some(MaskFormat::A8),
        2 => Some(MaskFormat::ThreeD),
        3 => Some(MaskFormat::Argb32),
        4 => Some(MaskFormat::Lcd16),
        5 => Some(MaskFormat::Sdf),
        _ => None,
    }
}

// Port of: src/core/SkGlyph.cpp#L103-L125 (buffer flattening, chrome/m156)
impl Glyph {
    /// `SkGlyph::MakeFromBuffer`: reads the metrics that [`flatten_metrics`](Self::flatten_metrics)
    /// wrote. Returns `None`, and leaves the buffer invalid, if the mask format is not valid.
    #[doc(alias = "MakeFromBuffer")]
    #[must_use]
    pub fn make_from_buffer(buffer: &mut ReadBuffer<'_>) -> Option<Glyph> {
        debug_assert!(buffer.is_valid());
        let packed_id = PackedGlyphId::from_raw(buffer.read_uint());
        let advance = buffer.read_point();
        let dimensions = buffer.read_uint();
        let left_top = buffer.read_uint();
        // `SkTo<SkMask::Format>`: the enum has a `uint8_t` underlying type, so the word is cut to
        // its low byte before the validity check.
        #[allow(clippy::cast_possible_truncation)] // mirrors the C++ conversion to uint8_t
        let format_byte = buffer.read_uint() as u8;
        let format = mask_format_from_u8(format_byte);
        if !buffer.validate(MaskFormat::is_valid_format(format_byte)) {
            return None;
        }
        let format = format?;

        let mut glyph = Glyph::new(packed_id);
        glyph.advance_x = advance.x;
        glyph.advance_y = advance.y;
        // The 16-bit fields are taken from the words as the C++ does (the casts keep the low
        // bits, and a negative `left` or `top` comes back through two's complement).
        #[allow(clippy::cast_possible_truncation)] // the words hold 16-bit fields
        {
            glyph.width = (dimensions >> 16) as u16;
            glyph.height = (dimensions & 0xffff) as u16;
            glyph.left = (left_top >> 16) as i16;
            glyph.top = (left_top & 0xffff) as i16;
        }
        glyph.mask_format = format;
        Some(glyph)
    }

    /// `SkGlyph::flattenMetrics`: writes the id, advances, bounds and format of the glyph.
    // Port of: src/core/SkGlyph.cpp#L340-L351 (chrome/m156)
    #[doc(alias = "flattenMetrics")]
    pub fn flatten_metrics(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_uint(self.id.value());
        buffer.write_point(Point::new(self.advance_x, self.advance_y));
        buffer.write_uint((u32::from(self.width) << 16) | u32::from(self.height));
        // Negative `left` and `top` are written as their 16-bit two's complement, so they do not
        // sign-extend into the other half of the word.
        #[allow(clippy::cast_sign_loss)] // the 16-bit pattern of a signed field, as in C++
        let left = u32::from(self.left as u16);
        #[allow(clippy::cast_sign_loss)] // the 16-bit pattern of a signed field, as in C++
        let top = u32::from(self.top as u16);
        buffer.write_uint((left << 16) | top);
        buffer.write_uint(self.mask_format as u32);
    }

    /// `SkGlyph::flattenImage`: writes the mask bytes, unless the glyph is empty or too big for
    /// an atlas.
    // Port of: src/core/SkGlyph.cpp#L353-L360 (chrome/m156)
    #[doc(alias = "flattenImage")]
    pub fn flatten_image(&self, buffer: &mut BinaryWriteBuffer) {
        debug_assert!(self.set_image_has_been_called());
        // If the glyph is empty or too big, then no image data is sent.
        if let Some(image) = self
            .image()
            .filter(|_| !self.is_empty() && GlyphDigest::fits_in_atlas(self))
        {
            buffer.write_byte_array(image);
        }
    }

    /// `SkGlyph::addImageFromBuffer`: reads the mask bytes that
    /// [`flatten_image`](Self::flatten_image) wrote and installs them. Returns the number of
    /// bytes of memory the image adds (0 if none was read).
    // Port of: src/core/SkGlyph.cpp#L362-L380 (chrome/m156)
    #[doc(alias = "addImageFromBuffer")]
    pub fn add_image_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> usize {
        debug_assert!(buffer.is_valid());
        // If the glyph is empty or too big, then no image data is received.
        if self.is_empty() || !GlyphDigest::fits_in_atlas(self) {
            return 0;
        }
        let image_size = self.image_size();
        let mut image = vec![0u8; image_size].into_boxed_slice();
        buffer.read_byte_array(&mut image);
        if buffer.is_valid() {
            debug_assert!(!self.set_image_has_been_called());
            self.set_image(image);
            return image_size;
        }
        0
    }

    /// `SkGlyph::flattenDrawable`: writes the drawable as a picture, or an empty array for a glyph
    /// with no area or no drawable.
    // Port of: src/core/SkGlyph.cpp#L424-L433 (chrome/m156)
    #[doc(alias = "flattenDrawable")]
    pub fn flatten_drawable(&self, buffer: &mut BinaryWriteBuffer) {
        debug_assert!(self.set_drawable_has_been_called());
        let drawable = if self.is_empty() {
            None
        } else {
            self.drawable()
        };
        PictureBackedGlyphDrawable::flatten_drawable(buffer, drawable);
    }

    /// `SkGlyph::addDrawableFromBuffer`: reads the drawable that
    /// [`flatten_drawable`](Self::flatten_drawable) wrote and records it. Returns the number of
    /// bytes of memory the drawable adds.
    // Port of: src/core/SkGlyph.cpp#L435-L448 (chrome/m156)
    #[doc(alias = "addDrawableFromBuffer")]
    pub fn add_drawable_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> usize {
        debug_assert!(buffer.is_valid());
        let drawable = PictureBackedGlyphDrawable::from_buffer(buffer);
        if !buffer.is_valid() {
            return 0;
        }
        if self.set_drawable(drawable) {
            self.drawable().map_or(0, Drawable::approximate_bytes_used)
        } else {
            0
        }
    }

    /// `SkGlyph::flattenPath`: writes whether there is a path and, if so, its flags and the path.
    // Port of: src/core/SkGlyph.cpp#L382-L392 (chrome/m156)
    #[doc(alias = "flattenPath")]
    pub fn flatten_path(&self, buffer: &mut BinaryWriteBuffer) {
        debug_assert!(self.set_path_has_been_called());
        let path = self.path();
        buffer.write_bool(path.is_some());
        if let Some(path) = path {
            buffer.write_bool(self.path_is_hairline());
            buffer.write_bool(self.path_is_modified());
            buffer.write_path(path);
        }
    }

    /// `SkGlyph::addPathFromBuffer`: reads the path that [`flatten_path`](Self::flatten_path)
    /// wrote and records it. Returns the number of bytes of memory the path adds.
    // Port of: src/core/SkGlyph.cpp#L394-L421 (chrome/m156)
    #[doc(alias = "addPathFromBuffer")]
    pub fn add_path_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> usize {
        debug_assert!(buffer.is_valid());
        let mut memory_increase = 0;
        let has_path = buffer.read_bool();
        // Check if the buffer is invalid, so as to not make a logical decision on invalid data.
        if !buffer.is_valid() {
            return 0;
        }
        if has_path {
            let path_is_hairline = buffer.read_bool();
            let path_is_modified = buffer.read_bool();
            if let Some(path) = buffer.read_path() {
                if !matches!(
                    self.mask_format,
                    MaskFormat::BW | MaskFormat::A8 | MaskFormat::Lcd16
                ) {
                    buffer.validate(false);
                    return 0;
                }
                let bytes = path.approximate_bytes_used();
                if self.set_path(Some(path), path_is_hairline, path_is_modified) {
                    memory_increase += bytes;
                }
            }
        } else {
            self.set_path(None, false, false);
        }
        memory_increase
    }
}

/// `format_rowbytes`: bytes per row for `width` pixels in `format`.
// Port of: src/core/SkGlyph.cpp#L167-L170 (chrome/m156)
fn format_rowbytes(width: u16, format: MaskFormat) -> usize {
    if format == MaskFormat::BW {
        // bits_to_bytes
        (usize::from(width) + 7) >> 3
    } else {
        usize::from(width) * format_alignment(format)
    }
}

/// `format_alignment`: the alignment, and the bytes per pixel, of the format.
// Port of: src/core/SkGlyph.cpp#L152-L165 (chrome/m156)
fn format_alignment(format: MaskFormat) -> usize {
    match format {
        MaskFormat::BW | MaskFormat::A8 | MaskFormat::ThreeD | MaskFormat::Sdf => 1,
        MaskFormat::Argb32 => 4,
        MaskFormat::Lcd16 => 2,
    }
}

/// `SkPictureBackedGlyphDrawable`: a drawable that replays a picture (the form color glyphs take
/// when they are too large for an image).
// Port of: src/core/SkGlyph.h#L409-L420 (chrome/m156)
#[doc(alias = "SkPictureBackedGlyphDrawable")]
#[derive(Debug)]
pub struct PictureBackedGlyphDrawable {
    picture: Picture,
}

impl PictureBackedGlyphDrawable {
    /// `SkPictureBackedGlyphDrawable(sk_sp<SkPicture>)`, as a drawable handle.
    // Port of: src/core/SkGlyph.cpp#L87-L88 (chrome/m156)
    #[must_use]
    pub fn into_drawable(picture: Picture) -> Drawable {
        Drawable::new(std::sync::Arc::new(Self { picture }))
    }

    /// `SkPictureBackedGlyphDrawable::MakeFromBuffer`: the drawable of the picture that the buffer
    /// holds. `None` for an empty drawable (which is written as an empty byte array), and for an
    /// invalid buffer or picture, which also invalidates the buffer.
    // Port of: src/core/SkGlyph.cpp#L38-L65 (chrome/m156), MakeFromBuffer
    #[must_use]
    pub fn from_buffer(buffer: &mut ReadBuffer<'_>) -> Option<Drawable> {
        let (data, size) = buffer.skip_byte_array();
        // Return nullptr if invalid or there an empty drawable, which is represented by nullptr.
        if !buffer.is_valid() || size == 0 {
            return None;
        }
        let Some(data) = data else {
            buffer.validate(false);
            return None;
        };
        let picture = Picture::from_data(data, None);
        if !buffer.validate(picture.is_some()) {
            return None;
        }
        picture.map(Self::into_drawable)
    }

    /// `SkPictureBackedGlyphDrawable::FlattenDrawable`: writes the picture snapshot of `drawable`
    /// as a byte array. An empty array is written when there is no drawable, when its picture is
    /// empty, or when the picture is too big for 32 bits.
    // Port of: src/core/SkGlyph.cpp#L67-L84 (chrome/m156), FlattenDrawable
    pub fn flatten_drawable(buffer: &mut BinaryWriteBuffer, drawable: Option<&Drawable>) {
        // The drawable's picture is serialized with the default procs, as the glyph has no
        // images, typefaces or pictures of its own.
        let data = drawable
            .and_then(Drawable::make_picture_snapshot)
            .and_then(|picture| picture.serialize(None));
        match data {
            Some(data) if !data.as_bytes().is_empty() && u32::try_from(data.size()).is_ok() => {
                buffer.write_byte_array(data.as_bytes());
            }
            // If the picture is too big, or there is no picture, then drop by sending an empty
            // byte array.
            _ => buffer.write_byte_array(&[]),
        }
    }
}

impl DrawableBase for PictureBackedGlyphDrawable {
    /// `onGetBounds`: the cull rect of the picture.
    // Port of: src/core/SkGlyph.cpp#L90-L92 (chrome/m156)
    fn on_get_bounds(&self) -> Rect {
        self.picture.cull_rect()
    }

    /// `onApproximateBytesUsed`: the object plus the picture.
    // Port of: src/core/SkGlyph.cpp#L94-L96 (chrome/m156)
    fn on_approximate_bytes_used(&self) -> usize {
        PICTURE_BACKED_GLYPH_DRAWABLE_SIZE + self.picture.approximate_bytes_used()
    }

    /// `onDraw`: draws the picture.
    // Port of: src/core/SkGlyph.cpp#L98-L100 (chrome/m156)
    fn on_draw(&self, canvas: &Canvas) {
        canvas.draw_picture(&self.picture, None, None);
    }
}

/// How a glyph's position is rounded for a strike (`SkGlyphPositionRoundingSpec`): the half
/// sample frequency that the painter adds before flooring, and the masks that drop the
/// position field the strike ignores.
// Port of: src/core/SkGlyph.h#L231-L240 (chrome/m156)
#[doc(alias = "SkGlyphPositionRoundingSpec")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GlyphPositionRoundingSpec {
    /// `halfAxisSampleFreq`.
    pub half_axis_sample_freq: Point,
    /// `ignorePositionMask`.
    pub ignore_position_mask: IPoint,
    /// `ignorePositionFieldMask`.
    pub ignore_position_field_mask: IPoint,
}

impl GlyphPositionRoundingSpec {
    /// `SkGlyphPositionRoundingSpec(bool isSubpixel, SkAxisAlignment axisAlignment)`.
    // Port of: src/core/SkGlyph.cpp#L728-L732 (chrome/m156)
    #[must_use]
    pub fn new(is_subpixel: bool, axis_alignment: AxisAlignment) -> Self {
        Self {
            half_axis_sample_freq: Self::half_axis_sample_freq(is_subpixel, axis_alignment),
            ignore_position_mask: Self::ignore_position_mask(is_subpixel, axis_alignment),
            ignore_position_field_mask: Self::ignore_position_field_mask(
                is_subpixel,
                axis_alignment,
            ),
        }
    }

    /// `SkGlyphPositionRoundingSpec::HalfAxisSampleFreq`.
    // Port of: src/core/SkGlyph.cpp#L695-L712 (chrome/m156)
    #[must_use]
    pub fn half_axis_sample_freq(is_subpixel: bool, axis_alignment: AxisAlignment) -> Point {
        if !is_subpixel {
            return Point::new(0.5, 0.5);
        }
        match axis_alignment {
            AxisAlignment::X => Point::new(PackedGlyphId::SUBPIXEL_ROUND, 0.5),
            AxisAlignment::Y => Point::new(0.5, PackedGlyphId::SUBPIXEL_ROUND),
            AxisAlignment::None => {
                Point::new(PackedGlyphId::SUBPIXEL_ROUND, PackedGlyphId::SUBPIXEL_ROUND)
            }
        }
    }

    /// `SkGlyphPositionRoundingSpec::IgnorePositionMask`: all bits set on the ignored axes.
    // Port of: src/core/SkGlyph.cpp#L714-L718 (chrome/m156)
    #[must_use]
    pub fn ignore_position_mask(is_subpixel: bool, axis_alignment: AxisAlignment) -> IPoint {
        let x = if !is_subpixel || axis_alignment == AxisAlignment::Y {
            0
        } else {
            !0
        };
        let y = if !is_subpixel || axis_alignment == AxisAlignment::X {
            0
        } else {
            !0
        };
        IPoint::new(x, y)
    }

    /// `SkGlyphPositionRoundingSpec::IgnorePositionFieldMask`: the ignore mask restricted to the
    /// packed id's x and y fields.
    // Port of: src/core/SkGlyph.cpp#L720-L726 (chrome/m156)
    #[must_use]
    pub fn ignore_position_field_mask(is_subpixel: bool, axis_alignment: AxisAlignment) -> IPoint {
        let ignore_mask = Self::ignore_position_mask(is_subpixel, axis_alignment);
        IPoint::new(
            ignore_mask.x & PackedGlyphId::XY_FIELD_MASK.x,
            ignore_mask.y & PackedGlyphId::XY_FIELD_MASK.y,
        )
    }
}

/// `SkMask::fRowBytes` is 32-bit. A glyph row is a few hundred bytes at most, far below that.
#[allow(clippy::cast_possible_truncation)] // see above
fn row_bytes_u32(row_bytes: usize) -> u32 {
    row_bytes as u32
}
