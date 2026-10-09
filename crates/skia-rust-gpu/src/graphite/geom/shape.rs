// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/Shape.h, src/gpu/graphite/geom/Shape.cpp

//! `skgpu::graphite::Shape`: a tagged union over the geometric shapes Graphite draws.
//!
//! The C++ class stores a `union` with a `fType` tag and an `fInverted` flag. Here the union is an
//! enum that carries its payload, and `inverted` stays a separate flag as in the C++.
//!
//! `keySize()` and `writeKey()` encode the shape's state and geometry into `u32` words, as in
//! `Shape.cpp#L143-L310`.

use skia_rust_core::arc::{self, Arc};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rrect::RRect;
use skia_rust_simd::vx::Float2;

use crate::graphite::geom::rect::Rect;

/// The kind of geometry stored in a [`Shape`], as in `Shape::Type`.
// Port of: src/gpu/graphite/geom/Shape.h#L35-L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::Shape::Type")]
// The discriminants match `enum class Type : uint8_t` and feed the key's state word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Type {
    /// No geometry (the default).
    Empty = 0,
    /// A line segment, stored as its two corners.
    Line = 1,
    /// An axis-aligned rect.
    Rect = 2,
    /// A round rect.
    RRect = 3,
    /// An arc or wedge of an oval.
    Arc = 4,
    /// An arbitrary path.
    Path = 5,
}

/// The geometry itself. Each variant carries the data for its `Type`.
#[derive(Clone, Debug)]
enum Kind {
    Empty,
    // p0 = top-left, p1 = bot-right (may be unsorted)
    Line(Rect),
    Rect(Rect),
    RRect(RRect),
    Arc(Arc),
    Path(Path),
}

/// `Shape` is effectively a tagged union over different geometric shapes, with the most complex
/// being a `Path`. It provides a consistent way to query geometric properties, such as convexity,
/// point containment, or iteration.
#[doc(alias = "skgpu::graphite::Shape")]
#[derive(Clone, Debug)]
pub struct Shape {
    kind: Kind,
    inverted: bool,
}

impl Default for Shape {
    // Port of: src/gpu/graphite/geom/Shape.h#L46 (chrome/m156)
    fn default() -> Self {
        Self {
            kind: Kind::Empty,
            inverted: false,
        }
    }
}

impl Shape {
    /// `kDefaultPixelTolerance`: any difference (in pixels) under this is perceptibly equivalent.
    /// (1.f - 0.001f) / 255.f, rounded down.
    // Port of: src/gpu/graphite/geom/Shape.h#L44 (chrome/m156)
    pub const DEFAULT_PIXEL_TOLERANCE: f32 = 0.0039;

    /// `Shape::Shape(p0, p1)`: a line segment.
    // Port of: src/gpu/graphite/geom/Shape.h#L51 (chrome/m156)
    #[must_use]
    pub fn new_line(p0: Float2, p1: Float2) -> Self {
        Self {
            kind: Kind::Line(Rect::from_corners(p0, p1)),
            inverted: false,
        }
    }

    /// `Shape::Shape(const Rect&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L53 (chrome/m156)
    #[must_use]
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            kind: Kind::Rect(rect),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkRRect&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L55 (chrome/m156)
    #[must_use]
    pub fn from_rrect(rrect: RRect) -> Self {
        Self {
            kind: Kind::RRect(rrect),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkArc&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L56 (chrome/m156)
    #[must_use]
    pub fn from_arc(arc: Arc) -> Self {
        Self {
            kind: Kind::Arc(arc),
            inverted: false,
        }
    }

    /// `Shape::Shape(const SkPath&)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L57 (chrome/m156)
    #[must_use]
    pub fn from_path(path: Path) -> Self {
        let inverted = path.is_inverse_fill_type();
        Self {
            kind: Kind::Path(path),
            inverted,
        }
    }

    /// `type()`: the type of the data last stored in the `Shape`. This does not incorporate any
    /// possible simplifications (a degenerate round rect with 0 radius corners is `RRect`).
    // Port of: src/gpu/graphite/geom/Shape.h#L69 (chrome/m156)
    #[must_use]
    pub fn type_(&self) -> Type {
        match self.kind {
            Kind::Empty => Type::Empty,
            Kind::Line(_) => Type::Line,
            Kind::Rect(_) => Type::Rect,
            Kind::RRect(_) => Type::RRect,
            Kind::Arc(_) => Type::Arc,
            Kind::Path(_) => Type::Path,
        }
    }

    /// `isEmpty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.type_() == Type::Empty
    }

    /// `isLine()`.
    #[must_use]
    pub fn is_line(&self) -> bool {
        self.type_() == Type::Line
    }

    /// `isRect()`.
    #[must_use]
    pub fn is_rect(&self) -> bool {
        self.type_() == Type::Rect
    }

    /// `isRRect()`.
    #[must_use]
    pub fn is_rrect(&self) -> bool {
        self.type_() == Type::RRect
    }

    /// `isArc()`.
    #[must_use]
    pub fn is_arc(&self) -> bool {
        self.type_() == Type::Arc
    }

    /// `isPath()`.
    #[must_use]
    pub fn is_path(&self) -> bool {
        self.type_() == Type::Path
    }

    /// `isFloodFill()`.
    #[must_use]
    pub fn is_flood_fill(&self) -> bool {
        self.is_empty() && self.inverted()
    }

    /// `isVolatilePath()`.
    #[must_use]
    pub fn is_volatile_path(&self) -> bool {
        matches!(&self.kind, Kind::Path(p) if p.is_volatile())
    }

    /// `inverted()`.
    #[must_use]
    pub fn inverted(&self) -> bool {
        debug_assert!(
            !matches!(&self.kind, Kind::Path(p) if self.inverted != p.is_inverse_fill_type())
        );
        self.inverted
    }

    /// `setInverted(inverted)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L89-L94 (chrome/m156)
    pub fn set_inverted(&mut self, inverted: bool) {
        if let Kind::Path(p) = &mut self.kind
            && inverted != p.is_inverse_fill_type()
        {
            p.toggle_inverse_fill_type();
        }
        self.inverted = inverted;
    }

    /// `fillType()`.
    // Port of: src/gpu/graphite/geom/Shape.h#L96-L102 (chrome/m156)
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        match &self.kind {
            // already incorporates invertedness
            Kind::Path(p) => p.fill_type(),
            _ => {
                if self.inverted {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                }
            }
        }
    }

    /// `conservativeContains(const Rect&)`: true if the given bounding box is completely inside
    /// the shape, if it's conservatively treated as a filled, closed shape.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L43-L68 (chrome/m156)
    #[must_use]
    pub fn conservative_contains_rect(&self, rect: Rect) -> bool {
        match &self.kind {
            Kind::Empty | Kind::Line(_) => false,
            Kind::Rect(r) => r.contains(rect),
            Kind::RRect(rr) => rr.contains(rect.as_sk_rect()),
            // We need to ensure the path is non-inverted.
            Kind::Path(p) => {
                if self.inverted() {
                    let mut non_inverted = p.clone();
                    non_inverted.toggle_inverse_fill_type();
                    non_inverted.conservatively_contains_rect(rect.as_sk_rect())
                } else {
                    p.conservatively_contains_rect(rect.as_sk_rect())
                }
            }
            Kind::Arc(a) => {
                if a.is_wedge() {
                    let mut arc_path = self.as_path();
                    if self.inverted() {
                        arc_path.toggle_inverse_fill_type();
                    }
                    arc_path.conservatively_contains_rect(rect.as_sk_rect())
                } else {
                    false
                }
            }
        }
    }

    /// `conservativeContains(skvx::float2 point)`.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L70-L87 (chrome/m156)
    #[must_use]
    pub fn conservative_contains_point(&self, point: Float2) -> bool {
        match &self.kind {
            Kind::Empty | Kind::Line(_) | Kind::Arc(_) => false,
            Kind::Rect(r) => r.contains(Rect::point(point)),
            Kind::RRect(rr) => rr.contains_point(Point::new(point.x(), point.y())),
            // We need to ensure the path is non-inverted.
            Kind::Path(p) => {
                if self.inverted() {
                    let mut non_inverted = p.clone();
                    non_inverted.toggle_inverse_fill_type();
                    non_inverted.contains((point.x(), point.y()))
                } else {
                    p.contains((point.x(), point.y()))
                }
            }
        }
    }

    /// `convex(simpleFill)`: true if the underlying shape is known to be convex, assuming no other
    /// styles. If `simple_fill` is true, it is assumed the contours will be implicitly closed when
    /// drawn or used.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L89-L99 (chrome/m156)
    #[must_use]
    pub fn convex(&self, simple_fill: bool) -> bool {
        match &self.kind {
            Kind::Path(p) => (simple_fill || p.is_last_contour_closed()) && p.is_convex(),
            Kind::Arc(a) => arc::draw_arc_is_convex(a.sweep_angle, a.kind, simple_fill),
            // Every other shape type is convex by construction.
            _ => true,
        }
    }

    /// `bounds()`: the bounding box of the shape.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L101-L111 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        match &self.kind {
            Kind::Empty => Rect::new(0.0, 0.0, 0.0, 0.0),
            // sorting corners computes bbox of segment
            Kind::Line(r) => r.make_sorted(),
            // assuming it's sorted
            Kind::Rect(r) => *r,
            Kind::RRect(rr) => Rect::from(*rr.bounds()),
            Kind::Arc(a) => Rect::from(a.oval),
            Kind::Path(p) => Rect::from(*p.bounds()),
        }
    }

    /// `asPath()`: converts the shape into a path that describes the same geometry.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L113-L140 (chrome/m156)
    #[must_use]
    pub fn as_path(&self) -> Path {
        match &self.kind {
            Kind::Path(p) => p.clone(),
            Kind::Arc(a) => {
                // Filled ovals are already culled out so we assume no simple fills
                let mut out = arc::create_draw_arc_path(a, false);
                // CreateDrawArcPath resets the output path and configures its fill type, so we
                // just have to ensure invertedness is correct.
                if self.inverted {
                    out.toggle_inverse_fill_type();
                }
                out
            }
            _ => {
                let mut builder = PathBuilder::new_with_fill_type(self.fill_type());
                match &self.kind {
                    Kind::Empty | Kind::Path(_) | Kind::Arc(_) => {}
                    Kind::Line(r) => {
                        builder
                            .move_to((r.left(), r.top()))
                            .line_to((r.right(), r.bot()));
                    }
                    Kind::Rect(r) => {
                        builder.add_rect(r.as_sk_rect(), Some(PathDirection::CW), Some(0));
                    }
                    Kind::RRect(rr) => {
                        builder.add_rrect(*rr, Some(PathDirection::CW), Some(0));
                    }
                }
                builder.detach()
            }
        }
    }

    /// `p0()`: the first corner of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line (`SkASSERT(isLine())` in the C++).
    #[must_use]
    pub fn p0(&self) -> Float2 {
        self.line_rect().top_left()
    }

    /// `p1()`: the second corner of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line.
    #[must_use]
    pub fn p1(&self) -> Float2 {
        self.line_rect().bot_right()
    }

    /// `line()`: `[left, top, right, bot]` of a line.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a line.
    #[must_use]
    pub fn line(&self) -> skia_rust_simd::vx::Float4 {
        self.line_rect().ltrb()
    }

    /// `rect()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a rect.
    #[must_use]
    pub fn rect(&self) -> &Rect {
        let Kind::Rect(r) = &self.kind else {
            panic!("Shape::rect() called on a {:?}", self.type_());
        };
        r
    }

    /// `rrect()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a round rect.
    #[must_use]
    pub fn rrect(&self) -> &RRect {
        let Kind::RRect(rr) = &self.kind else {
            panic!("Shape::rrect() called on a {:?}", self.type_());
        };
        rr
    }

    /// `arc()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not an arc.
    #[must_use]
    pub fn arc(&self) -> &Arc {
        let Kind::Arc(a) = &self.kind else {
            panic!("Shape::arc() called on a {:?}", self.type_());
        };
        a
    }

    /// `path()`.
    ///
    /// # Panics
    ///
    /// Panics if the shape is not a path.
    #[must_use]
    pub fn path(&self) -> &Path {
        let Kind::Path(p) = &self.kind else {
            panic!("Shape::path() called on a {:?}", self.type_());
        };
        p
    }

    // The line's corners, for `p0()`, `p1()` and `line()`.
    fn line_rect(&self) -> &Rect {
        let Kind::Line(r) = &self.kind else {
            panic!("Shape line accessor called on a {:?}", self.type_());
        };
        r
    }

    /// `setLine(p0, p1)`: resets inversion to the default for lines.
    // Port of: src/gpu/graphite/geom/Shape.h#L146-L150 (chrome/m156)
    pub fn set_line(&mut self, p0: Float2, p1: Float2) {
        self.kind = Kind::Line(Rect::from_corners(p0, p1));
        self.inverted = false;
    }

    /// `setRect(rect)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L152-L156 (chrome/m156)
    pub fn set_rect(&mut self, rect: Rect) {
        self.kind = Kind::Rect(rect);
        self.inverted = false;
    }

    /// `setRRect(rrect)`. Performs no simplification: a round rect with 0 radii is still an
    /// `RRect`.
    // Port of: src/gpu/graphite/geom/Shape.h#L157-L161 (chrome/m156)
    pub fn set_rrect(&mut self, rrect: RRect) {
        self.kind = Kind::RRect(rrect);
        self.inverted = false;
    }

    /// `setArc(arc)`.
    // Port of: src/gpu/graphite/geom/Shape.h#L162-L166 (chrome/m156)
    pub fn set_arc(&mut self, arc: Arc) {
        self.kind = Kind::Arc(arc);
        self.inverted = false;
    }

    /// `setPath(path)`: the inversion follows the path's own fill type.
    // Port of: src/gpu/graphite/geom/Shape.h#L167-L177 (chrome/m156)
    pub fn set_path(&mut self, path: Path) {
        self.inverted = path.is_inverse_fill_type();
        self.kind = Kind::Path(path);
    }

    /// `reset()`: back to the empty, non-inverted shape.
    // Port of: src/gpu/graphite/geom/Shape.h#L179-L182 (chrome/m156)
    pub fn reset(&mut self) {
        self.kind = Kind::Empty;
        self.inverted = false;
    }
}

// Keys. Every `u32` word of a key is written with the bit pattern of the source value, exactly as
// the C++ `memcpy`s do.

/// `kMaxKeyFromDataVerbCnt`: paths with more verbs than this are keyed by their generation ID.
// Port of: src/gpu/graphite/geom/Shape.cpp#L24 (chrome/m156)
const MAX_KEY_FROM_DATA_VERB_CNT: usize = 10;

/// `SkAlign4(x)`.
fn align4(x: usize) -> usize {
    (x + 3) & !3
}

/// `path_key_from_data_size(path)`: the number of key words for a path keyed by its data, or
/// `None` when the path has too many verbs (C++ returns -1 and the key uses the generation ID).
// Port of: src/gpu/graphite/geom/Shape.cpp#L143-L158 (chrome/m156)
fn path_key_from_data_size(path: &Path) -> Option<usize> {
    let verb_cnt = path.count_verbs();
    if verb_cnt > MAX_KEY_FROM_DATA_VERB_CNT {
        return None;
    }
    let point_cnt = path.points().len();
    let conic_weight_cnt = path.conic_weights().len();

    // 1 is for the verb count. Each verb is a byte but we'll pad the verb data out to
    // a uint32_t length.
    Some(1 + (align4(verb_cnt) >> 2) + 2 * point_cnt + conic_weight_cnt)
}

/// `noninverted_fill_type(fillType)`.
// Port of: src/gpu/graphite/geom/Shape.cpp#L282-L293 (chrome/m156)
fn noninverted_fill_type(fill_type: PathFillType) -> PathFillType {
    match fill_type {
        PathFillType::Winding | PathFillType::InverseWinding => PathFillType::Winding,
        PathFillType::EvenOdd | PathFillType::InverseEvenOdd => PathFillType::EvenOdd,
    }
}

/// The write cursor over the `u32` words of a key (the C++ `uint32_t* key` argument).
struct KeyCursor<'a> {
    words: &'a mut [u32],
    pos: usize,
}

impl KeyCursor<'_> {
    fn push(&mut self, word: u32) {
        self.words[self.pos] = word;
        self.pos += 1;
    }

    fn push_f32(&mut self, value: f32) {
        self.push(value.to_bits());
    }

    /// `memcpy(key, &genID, sizeof(uint64_t))`: two native-endian words.
    fn push_u64(&mut self, value: u64) {
        let b = value.to_ne_bytes();
        self.push(u32::from_ne_bytes([b[0], b[1], b[2], b[3]]));
        self.push(u32::from_ne_bytes([b[4], b[5], b[6], b[7]]));
    }
}

/// `write_path_key_from_data(path, key)`.
// Port of: src/gpu/graphite/geom/Shape.cpp#L159-L182 (chrome/m156)
fn write_path_key_from_data(path: &Path, key: &mut KeyCursor<'_>) {
    let verbs = path.verbs();
    let points = path.points();
    let conics = path.conic_weights();
    debug_assert!(verbs.len() <= MAX_KEY_FROM_DATA_VERB_CNT);
    debug_assert!(!points.is_empty() && !verbs.is_empty());

    key.push(u32::try_from(verbs.len()).expect("verb count fits in a key word"));
    // Pad the verb data out to uint32_t alignment using a value that will stand out when
    // debugging (0xDE).
    let verb_key_size = align4(verbs.len());
    let mut verb_bytes: Vec<u8> = verbs.iter().map(|v| *v as u8).collect();
    verb_bytes.resize(verb_key_size, 0xDE);
    for c in verb_bytes.as_chunks::<4>().0 {
        key.push(u32::from_ne_bytes(*c));
    }

    for p in points {
        key.push_f32(p.x);
        key.push_f32(p.y);
    }
    for &w in conics {
        key.push_f32(w);
    }
}

impl Shape {
    /// `keySize()`: the number of `u32` words [`Shape::write_key`] writes.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L185-L221 (chrome/m156)
    #[doc(alias = "keySize")]
    #[must_use]
    // The expects cannot fire: every key size is a small constant or a bounded path key.
    #[allow(clippy::missing_panics_doc)]
    pub fn key_size(&self) -> u16 {
        // Every key has the state flags from the Shape.
        let mut count: u16 = 1;
        match self.type_() {
            // sizeof(skvx::float4) / sizeof(uint32_t) and sizeof(Rect) / sizeof(uint32_t): both
            // are 16 bytes.
            Type::Line | Type::Rect => count += 4,
            // SkRRect::kSizeInMemory / sizeof(uint32_t)
            Type::RRect => {
                count += u16::try_from(RRect::SIZE_IN_MEMORY / 4).expect("rrect key fits");
            }
            // sizeof(SkArc) / sizeof(uint32_t)
            Type::Arc => count += 7,
            Type::Path => {
                // An empty path is the same as an empty shape -- only needs the state flags.
                if !self.path().is_empty() {
                    match path_key_from_data_size(self.path()) {
                        Some(data_key_size) => {
                            count += u16::try_from(data_key_size).expect("path key fits");
                        }
                        // Just adds the gen ID: sizeof(uint64_t) / sizeof(uint32_t).
                        None => count += 2,
                    }
                }
            }
            // Else it's empty, which just needs the state flags for its key.
            Type::Empty => {}
        }
        count
    }

    /// `writeKey(key, includeInverted)`: writes [`Shape::key_size`] words into `key`.
    ///
    /// # Panics
    ///
    /// Panics if `key` is shorter than [`Shape::key_size`].
    // Port of: src/gpu/graphite/geom/Shape.cpp#L223-L280 (chrome/m156)
    #[doc(alias = "writeKey")]
    pub fn write_key(&self, key: &mut [u32], include_inverted: bool) {
        let size = usize::from(self.key_size());
        let mut out = KeyCursor {
            words: &mut key[..size],
            pos: 0,
        };

        // Every key starts with the state from the Shape (this includes path fill type,
        // and any tracked inversion, as well as the class of geometry).
        out.push(self.state_key(include_inverted));

        match self.type_() {
            Type::Path => {
                // An empty path is the same as an empty shape -- only needs the state flags.
                if !self.path().is_empty() {
                    // The path's inversion must match our state in order for the path's key to
                    // suffice.
                    debug_assert_eq!(self.inverted, self.path().is_inverse_fill_type());

                    if path_key_from_data_size(self.path()).is_some() {
                        write_path_key_from_data(self.path(), &mut out);
                        debug_assert_eq!(out.pos, size);
                        return;
                    }
                    out.push_u64(self.path().generation_id());
                }
            }
            Type::Rect => {
                // memcpy(key, &this->rect(), sizeof(Rect))
                let vals = self.rect().vals();
                for i in 0..4 {
                    out.push_f32(vals[i]);
                }
            }
            Type::RRect => {
                // rrect().writeToMemory(key)
                let mut bytes = Vec::with_capacity(RRect::SIZE_IN_MEMORY);
                self.rrect().write_to_memory(&mut bytes);
                for c in bytes.as_chunks::<4>().0 {
                    out.push(u32::from_ne_bytes(*c));
                }
            }
            Type::Arc => {
                let arc = self.arc();
                // Write the dense floats first: the oval, then the start and sweep angles.
                out.push_f32(arc.oval.left);
                out.push_f32(arc.oval.top);
                out.push_f32(arc.oval.right);
                out.push_f32(arc.oval.bottom);
                out.push_f32(arc.start_angle);
                out.push_f32(arc.sweep_angle);
                // Then the final bool as an int, to make sure upper bits are set.
                out.push(u32::from(arc.is_wedge()));
            }
            Type::Line => {
                // memcpy(key, &line, sizeof(skvx::float4))
                let line = self.line();
                for i in 0..4 {
                    out.push_f32(line[i]);
                }
            }
            // Nothing other than the flag state is needed in the key for an empty shape.
            Type::Empty => {}
        }
        debug_assert_eq!(out.pos, size);
    }

    /// `stateKey(includeInverted)`: the state word every key starts with. It carries the path's
    /// fill type (or its inversion), and the geometry class.
    // Port of: src/gpu/graphite/geom/Shape.cpp#L295-L311 (chrome/m156)
    fn state_key(&self, include_inverted: bool) -> u32 {
        let mut key: u32 = if include_inverted {
            // Use the path's full fill type instead of just whether or not it's inverted.
            if self.is_path() {
                self.path().fill_type() as u32
            } else {
                u32::from(self.inverted)
            }
        } else if self.is_path() {
            // Use the path's noninverted fill type.
            noninverted_fill_type(self.path().fill_type()) as u32
        } else {
            0
        };
        // The fill type was 2 bits.
        key |= (self.type_() as u32) << 2;
        key
    }
}

#[cfg(test)]
mod key_tests {
    use super::{Shape, Type};
    use crate::graphite::geom::rect::Rect;
    use skia_rust_simd::vx::Float2;

    #[test]
    fn empty_shape_key_is_only_state() {
        let s = Shape::default();
        assert_eq!(s.type_(), Type::Empty);
        assert_eq!(s.key_size(), 1);
        let mut key = [0xFFFF_FFFFu32; 1];
        s.write_key(&mut key, true);
        assert_eq!(key, [0]);
    }

    #[test]
    fn rect_key_is_state_then_rect_bits() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        let s = Shape::from_rect(r);
        assert_eq!(s.key_size(), 5);
        let mut key = [0u32; 5];
        s.write_key(&mut key, false);
        // The state word is the geometry class in bits 2.. (Rect = 2).
        assert_eq!(key[0], 2 << 2);
        let vals = r.vals();
        for i in 0..4 {
            assert_eq!(key[i + 1], vals[i].to_bits());
        }
    }

    #[test]
    fn line_key_is_state_then_ltrb_bits() {
        let s = Shape::new_line(Float2::new(1.0, 2.0), Float2::new(3.0, 4.0));
        assert_eq!(s.key_size(), 5);
        let mut key = [0u32; 5];
        s.write_key(&mut key, false);
        assert_eq!(key[0], 1 << 2);
        assert_eq!(key[1], 1.0f32.to_bits());
        assert_eq!(key[2], 2.0f32.to_bits());
        assert_eq!(key[3], 3.0f32.to_bits());
        assert_eq!(key[4], 4.0f32.to_bits());
    }
}
