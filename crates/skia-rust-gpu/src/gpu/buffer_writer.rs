// Copyright 2010 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/BufferWriter.h

//! `skgpu::BufferWriter`, `VertexWriter`, `IndexWriter` and `TextureUploadWriter`: sequential
//! writers into a mapped GPU buffer.
//!
//! Skia's writers hold a raw pointer and an optional debug end mark. Here a writer owns the
//! remaining `&mut [u8]` of its buffer: every write splits off the bytes it covers, and
//! [`BufferWriter::make_offset`] splits the buffer in two, which is what Skia's `makeOffset`
//! does to the pointer range. Each byte can therefore be written by one writer only, and a
//! write past the end of the buffer panics in every build (Skia only checks in debug builds).
//!
//! `operator<<` becomes [`VertexWriter::put`] (and the matching methods on the other writers).
//! Skia's `VertexWriter` `Conditional`, `Skip`, `ArrayDesc`, `RepeatDesc`, `VertexColor` and
//! quad helpers are kept, implemented through the [`BufferWrite`] trait.
//!
//! The `convert` method of `TextureUploadWriter` needs `graphite::TextureFormatXferFn` and is
//! not ported yet (G8).

use std::marker::PhantomData;

use skia_rust_core::color::PMColor4f;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::Rect;

/// A position in a buffer, as Skia's `BufferWriter::Mark` (an address). Marks from writers over
/// the same buffer compare and subtract by position; the difference is in bytes.
// Port of: src/gpu/BufferWriter.h#L36-L63 (chrome/m156)
#[doc(alias = "skgpu::BufferWriter::Mark")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mark(usize);

impl std::ops::Sub for Mark {
    type Output = isize;

    // Port of: src/gpu/BufferWriter.h#L52 (chrome/m156)
    /// The distance in bytes from `rhs` to `self`.
    fn sub(self, rhs: Mark) -> isize {
        self.0.cast_signed() - rhs.0.cast_signed()
    }
}

/// A sequential writer into the bytes of one buffer (`BufferWriter`).
// Port of: src/gpu/BufferWriter.h#L69-L162 (chrome/m156)
#[doc(alias = "skgpu::BufferWriter")]
#[derive(Debug, Default)]
pub struct BufferWriter<'a> {
    // The bytes not yet written: [fPtr, fEnd) in Skia.
    buf: &'a mut [u8],
    // The position of `buf[0]` within the buffer's address space, for marks.
    pos: usize,
}

impl<'a> BufferWriter<'a> {
    /// Creates a writer over all of `buf`, positioned at 0.
    // Port of: src/gpu/BufferWriter.h#L80-L82 (chrome/m156), BufferWriter(void*, size_t)
    #[must_use]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Marks a read-only position `offset` bytes ahead of the writer (`mark`).
    // Port of: src/gpu/BufferWriter.h#L101-L104 (chrome/m156)
    #[must_use]
    pub fn mark(&self, offset: usize) -> Mark {
        self.validate(offset);
        Mark(self.pos + offset)
    }

    /// Writes `bytes` zeros (`zeroBytes`).
    // Port of: src/gpu/BufferWriter.h#L106-L109 (chrome/m156)
    pub fn zero_bytes(&mut self, bytes: usize) {
        self.slice(bytes).fill(0);
    }

    /// Writes the raw bytes `src` (`write(const void*, size_t)`).
    // Port of: src/gpu/BufferWriter.h#L111-L114 (chrome/m156)
    pub fn write_bytes(&mut self, src: &[u8]) {
        self.slice(src.len()).copy_from_slice(src);
    }

    /// Splits the writer: `self` keeps the first `offset` bytes and the returned writer covers
    /// the rest (`makeOffset`). `std::exchange(w, w.make_offset(n))` hands out the first `n`
    /// bytes and leaves `w` after them.
    // Port of: src/gpu/BufferWriter.h#L140-L153 (chrome/m156)
    #[must_use]
    pub fn make_offset(&mut self, offset: usize) -> BufferWriter<'a> {
        self.validate(offset);
        let buf = std::mem::take(&mut self.buf);
        let (head, tail) = buf.split_at_mut(offset);
        self.buf = head;
        BufferWriter {
            buf: tail,
            pos: self.pos + offset,
        }
    }

    /// Returns the next `bytes` bytes to write, and advances past them (`slice`).
    // Port of: src/gpu/BufferWriter.h#L155-L162 (chrome/m156)
    pub(crate) fn slice(&mut self, bytes: usize) -> &'a mut [u8] {
        self.validate(bytes);
        let buf = std::mem::take(&mut self.buf);
        let (head, tail) = buf.split_at_mut(bytes);
        self.buf = tail;
        self.pos += bytes;
        head
    }

    /// The writable bytes `offset..offset + len` past the writer, without advancing it. Used
    /// by the writers that write at an offset (`TextureUploadWriter::write`).
    // Port of: the `dst = SkTAddOffset<void>(fPtr, offset)` pattern in BufferWriter.h#L372-L376
    pub(crate) fn region_mut(&mut self, offset: usize, len: usize) -> &mut [u8] {
        self.validate(offset + len);
        &mut self.buf[offset..offset + len]
    }

    // Port of: src/gpu/BufferWriter.h#L164-L170 (chrome/m156)
    // The bounds check Skia does in debug builds; here it is always on.
    fn validate(&self, bytes_to_write: usize) {
        assert!(
            bytes_to_write <= self.buf.len(),
            "BufferWriter: writing {bytes_to_write} bytes past the end of the buffer ({} left)",
            self.buf.len()
        );
    }
}

/// A value that can be written into a buffer: a POD value (its native-endian bytes), or one of
/// the helper types of this module. Replaces the `operator<<` overloads of `BufferWriter.h`.
#[doc(alias = "operator<<")]
pub trait BufferWrite {
    /// Writes this value at the writer's position, advancing it.
    fn write_to(&self, w: &mut BufferWriter<'_>);
}

macro_rules! impl_buffer_write_for_pod {
    ($($t:ty),* $(,)?) => {$(
        impl BufferWrite for $t {
            fn write_to(&self, w: &mut BufferWriter<'_>) {
                w.write_bytes(&self.to_ne_bytes());
            }
        }
    )*};
}
impl_buffer_write_for_pod!(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64);

impl BufferWrite for bool {
    // `sizeof(bool)` bytes holding 0 or 1.
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        w.write_bytes(&[u8::from(*self)]);
    }
}

impl<T: BufferWrite> BufferWrite for [T] {
    // The `ArrayDesc` / `write(SkSpan<const T>)` path: each element in order. For POD element
    // types this is the same bytes as a single memcpy of the array.
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        for v in self {
            v.write_to(w);
        }
    }
}

impl<T: BufferWrite, const N: usize> BufferWrite for [T; N] {
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        self.as_slice().write_to(w);
    }
}

/// `VertexWriter::Conditional`: writes `value` only if `condition` is true.
// Port of: src/gpu/BufferWriter.h#L210-L213 (chrome/m156)
#[doc(alias = "VertexWriter::Conditional")]
#[derive(Clone, Copy, Debug)]
pub struct Conditional<T> {
    /// Whether the value is written.
    pub condition: bool,
    /// The value.
    pub value: T,
}

impl<T: BufferWrite> BufferWrite for Conditional<T> {
    // Port of: src/gpu/BufferWriter.h#L334-L338 (chrome/m156)
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        if self.condition {
            self.value.write_to(w);
        }
    }
}

/// `VertexWriter::If`.
// Port of: src/gpu/BufferWriter.h#L216-L220 (chrome/m156)
#[must_use]
pub fn conditional<T>(condition: bool, value: T) -> Conditional<T> {
    Conditional { condition, value }
}

/// `VertexWriter::Skip<T>`: advances the writer by `sizeof(T)` bytes without writing.
// Port of: src/gpu/BufferWriter.h#L222-L223 (chrome/m156)
#[doc(alias = "VertexWriter::Skip")]
#[derive(Clone, Copy, Debug, Default)]
pub struct Skip<T>(PhantomData<T>);

impl<T> Skip<T> {
    /// `VertexWriter::Skip<T>{}`.
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> BufferWrite for Skip<T> {
    // Port of: src/gpu/BufferWriter.h#L373-L376 (chrome/m156)
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        // w = w.makeOffset(sizeof(T)): the skipped bytes are dropped with the head.
        *w = w.make_offset(std::mem::size_of::<T>());
    }
}

/// `VertexWriter::RepeatDesc<kCount, T>`: writes `value` `COUNT` times.
// Port of: src/gpu/BufferWriter.h#L232-L240 (chrome/m156)
#[doc(alias = "VertexWriter::RepeatDesc")]
#[derive(Clone, Copy, Debug)]
pub struct Repeat<'b, const COUNT: usize, T> {
    /// The repeated value.
    pub value: &'b T,
}

impl<const COUNT: usize, T: BufferWrite> BufferWrite for Repeat<'_, COUNT, T> {
    // Port of: src/gpu/BufferWriter.h#L387-L392 (chrome/m156)
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        for _ in 0..COUNT {
            self.value.write_to(w);
        }
    }
}

/// The corners of a quad written as a triangle strip: `(l,t)`, `(l,b)`, `(r,t)`, `(r,b)`.
// Port of: src/gpu/BufferWriter.h#L255-L275 (chrome/m156)
#[doc(alias = "VertexWriter::TriStrip")]
#[derive(Clone, Copy, Debug)]
pub struct TriStrip<T> {
    /// Left.
    pub l: T,
    /// Top.
    pub t: T,
    /// Right.
    pub r: T,
    /// Bottom.
    pub b: T,
}

/// The corners of a quad written as a triangle fan: `(l,t)`, `(l,b)`, `(r,b)`, `(r,t)`.
// Port of: src/gpu/BufferWriter.h#L286-L304 (chrome/m156)
#[doc(alias = "VertexWriter::TriFan")]
#[derive(Clone, Copy, Debug)]
pub struct TriFan<T> {
    /// Left.
    pub l: T,
    /// Top.
    pub t: T,
    /// Right.
    pub r: T,
    /// Bottom.
    pub b: T,
}

/// A quad whose vertices differ per corner: `write_vertex` writes corner `corner` (0..4).
// Port of: src/gpu/BufferWriter.h#L232-L240 (chrome/m156), the `is_quad` protocol
pub trait QuadVertex {
    /// Writes the vertex for `corner`.
    fn write_vertex(&self, corner: usize, w: &mut VertexWriter<'_>);
}

impl<T: BufferWrite> QuadVertex for TriStrip<T> {
    // Port of: src/gpu/BufferWriter.h#L257-L266 (chrome/m156)
    fn write_vertex(&self, corner: usize, w: &mut VertexWriter<'_>) {
        match corner {
            0 => {
                w.put(&self.l).put(&self.t);
            }
            1 => {
                w.put(&self.l).put(&self.b);
            }
            2 => {
                w.put(&self.r).put(&self.t);
            }
            3 => {
                w.put(&self.r).put(&self.b);
            }
            _ => unreachable!("a quad has four corners"),
        }
    }
}

impl<T: BufferWrite> QuadVertex for TriFan<T> {
    // Port of: src/gpu/BufferWriter.h#L288-L297 (chrome/m156)
    fn write_vertex(&self, corner: usize, w: &mut VertexWriter<'_>) {
        match corner {
            0 => {
                w.put(&self.l).put(&self.t);
            }
            1 => {
                w.put(&self.l).put(&self.b);
            }
            2 => {
                w.put(&self.r).put(&self.b);
            }
            3 => {
                w.put(&self.r).put(&self.t);
            }
            _ => unreachable!("a quad has four corners"),
        }
    }
}

/// `TriStripFromRect`.
// Port of: src/gpu/BufferWriter.h#L277-L279 (chrome/m156)
#[must_use]
pub fn tri_strip_from_rect(r: &Rect) -> TriStrip<f32> {
    TriStrip {
        l: r.left,
        t: r.top,
        r: r.right,
        b: r.bottom,
    }
}

/// `TriStripFromUVs`.
// Port of: src/gpu/BufferWriter.h#L281-L283 (chrome/m156)
#[must_use]
pub fn tri_strip_from_uvs(rect: [u16; 4]) -> TriStrip<u16> {
    TriStrip {
        l: rect[0],
        t: rect[1],
        r: rect[2],
        b: rect[3],
    }
}

/// `TriFanFromRect`.
// Port of: src/gpu/BufferWriter.h#L306-L308 (chrome/m156)
#[must_use]
pub fn tri_fan_from_rect(r: &Rect) -> TriFan<f32> {
    TriFan {
        l: r.left,
        t: r.top,
        r: r.right,
        b: r.bottom,
    }
}

/// One argument of [`VertexWriter::write_quad`]: a value written at every corner, or a quad
/// whose vertex differs per corner.
#[derive(Clone, Copy)]
pub enum QuadArg<'b> {
    /// Written again at each of the four corners.
    Value(&'b dyn BufferWrite),
    /// Writes a different vertex at each corner.
    Quad(&'b dyn QuadVertex),
}

impl std::fmt::Debug for QuadArg<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuadArg::Value(_) => f.write_str("QuadArg::Value"),
            QuadArg::Quad(_) => f.write_str("QuadArg::Quad"),
        }
    }
}

/// Writes vertex data (`VertexWriter`).
///
/// Usage, as in Skia: `vertices.put(a).put(b)`; `put` is `operator<<`.
// Port of: src/gpu/BufferWriter.h#L178-L320 (chrome/m156)
#[doc(alias = "skgpu::VertexWriter")]
#[derive(Debug, Default)]
pub struct VertexWriter<'a>(BufferWriter<'a>);

impl<'a> VertexWriter<'a> {
    /// Creates a writer over all of `buf`.
    #[must_use]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self(BufferWriter::new(buf))
    }

    /// `operator<<`: writes `value` and returns the writer for chaining.
    pub fn put<T: BufferWrite + ?Sized>(&mut self, value: &T) -> &mut Self {
        value.write_to(&mut self.0);
        self
    }

    /// See [`BufferWriter::mark`].
    #[must_use]
    pub fn mark(&self, offset: usize) -> Mark {
        self.0.mark(offset)
    }

    /// See [`BufferWriter::zero_bytes`].
    pub fn zero_bytes(&mut self, bytes: usize) {
        self.0.zero_bytes(bytes);
    }

    /// See [`BufferWriter::make_offset`].
    #[must_use]
    pub fn make_offset(&mut self, offset_in_bytes: usize) -> VertexWriter<'a> {
        VertexWriter(self.0.make_offset(offset_in_bytes))
    }

    /// `writeQuad`: writes the four corners of a quad. For each corner, every argument in order
    /// writes its part: a [`QuadArg::Value`] repeats, a [`QuadArg::Quad`] writes its corner.
    // Port of: src/gpu/BufferWriter.h#L320-L355 (chrome/m156)
    pub fn write_quad(&mut self, args: &[QuadArg<'_>]) {
        for corner in 0..4 {
            for arg in args {
                match arg {
                    QuadArg::Value(v) => v.write_to(&mut self.0),
                    QuadArg::Quad(q) => q.write_vertex(corner, self),
                }
            }
        }
    }
}

/// `VertexColor`: a premultiplied color written as four bytes, or as four `f32`s when wide.
// Port of: src/gpu/BufferWriter.h#L413-L461 (chrome/m156)
#[doc(alias = "skgpu::VertexColor")]
#[derive(Clone, Copy, Debug, Default)]
pub struct VertexColor {
    // The bits of the four words written: four `f32`s when wide, else the packed color in [0].
    color: [u32; 4],
    wide_color: bool,
}

impl VertexColor {
    /// `VertexColor(color, wideColor)`.
    // Port of: src/gpu/BufferWriter.h#L416-L418 (chrome/m156)
    #[must_use]
    pub fn new(color: &PMColor4f, wide_color: bool) -> Self {
        let mut vc = Self::default();
        vc.set(color, wide_color);
        vc
    }

    // Port of: src/gpu/BufferWriter.h#L419-L428 (chrome/m156)
    /// `set(color, wideColor)`.
    pub fn set(&mut self, color: &PMColor4f, wide_color: bool) {
        if wide_color {
            // memcpy(fColor, color.vec(), 16 bytes)
            self.color = color.as_array().map(f32::to_bits);
        } else {
            self.color[0] = color.to_bytes();
        }
        self.wide_color = wide_color;
    }

    /// `size()`: 16 when wide, else 4.
    #[must_use]
    pub fn size(&self) -> usize {
        if self.wide_color { 16 } else { 4 }
    }
}

impl BufferWrite for VertexColor {
    // Port of: src/gpu/BufferWriter.h#L442-L450 (chrome/m156)
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        self.color[0].write_to(w);
        if self.wide_color {
            self.color[1].write_to(w);
            self.color[2].write_to(w);
            self.color[3].write_to(w);
        }
    }
}

/// Writes 16-bit indices (`IndexWriter`).
// Port of: src/gpu/BufferWriter.h#L464-L478 (chrome/m156)
#[doc(alias = "skgpu::IndexWriter")]
#[derive(Debug, Default)]
pub struct IndexWriter<'a>(BufferWriter<'a>);

impl<'a> IndexWriter<'a> {
    /// Creates a writer over all of `buf`.
    #[must_use]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self(BufferWriter::new(buf))
    }

    /// `operator<<(IndexWriter&, uint16_t)`.
    pub fn put(&mut self, val: u16) -> &mut Self {
        self.0.write_bytes(&val.to_ne_bytes());
        self
    }

    /// `operator<<(IndexWriter&, int)`: the value must fit in 16 bits (`SkTo<uint16_t>`).
    ///
    /// # Panics
    /// If `val` does not fit in a `u16`.
    pub fn put_int(&mut self, val: i32) -> &mut Self {
        let v = u16::try_from(val).expect("index does not fit in 16 bits");
        self.put(v)
    }

    /// `writeArray`.
    pub fn write_array(&mut self, indices: &[u16]) {
        for i in indices {
            self.put(*i);
        }
    }

    /// `makeOffset(int numIndices)`: the writer after `num_indices` indices.
    #[must_use]
    pub fn make_offset(&mut self, num_indices: usize) -> IndexWriter<'a> {
        IndexWriter(self.0.make_offset(num_indices * std::mem::size_of::<u16>()))
    }

    /// See [`BufferWriter::mark`].
    #[must_use]
    pub fn mark(&self, offset: usize) -> Mark {
        self.0.mark(offset)
    }
}

/// Writes texture upload data (`TextureUploadWriter`). The Graphite-only `convert` is not
/// ported yet (it needs `TextureFormatXferFn`).
// Port of: src/gpu/BufferWriter.h#L480-L530 (chrome/m156)
#[doc(alias = "skgpu::TextureUploadWriter")]
#[derive(Debug, Default)]
pub struct TextureUploadWriter<'a>(BufferWriter<'a>);

impl<'a> TextureUploadWriter<'a> {
    /// Creates a writer over all of `buf`.
    #[must_use]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self(BufferWriter::new(buf))
    }

    /// See [`BufferWriter::mark`].
    #[must_use]
    pub fn mark(&self, offset: usize) -> Mark {
        self.0.mark(offset)
    }

    /// Copies `row_count` rows of `trim_row_bytes` from `src` (rows `src_row_bytes` apart) to
    /// the buffer at `offset` (rows `dst_row_bytes` apart). Does not advance the writer.
    // Port of: src/gpu/BufferWriter.h#L496-L501 (chrome/m156), with SkRectMemcpy from
    // src/core/SkRectMemcpy.h#L16-L30 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn write(
        &mut self,
        offset: usize,
        src: &[u8],
        src_row_bytes: usize,
        dst_row_bytes: usize,
        trim_row_bytes: usize,
        row_count: usize,
    ) {
        let dst = self.0.region_mut(offset, dst_row_bytes * row_count);
        rect_memcpy(
            dst,
            dst_row_bytes,
            src,
            src_row_bytes,
            trim_row_bytes,
            row_count,
        );
    }

    /// Converts `src` (described by `src_info`) into the buffer at `offset`, as described by
    /// `dst_info`. Both infos must have the same dimensions. Does not advance the writer.
    // Port of: src/gpu/BufferWriter.h#L503-L510 (chrome/m156)
    pub fn convert_and_write(
        &mut self,
        offset: usize,
        src_info: &ImageInfo,
        src: &[u8],
        src_row_bytes: usize,
        dst_info: &ImageInfo,
        dst_row_bytes: usize,
    ) {
        debug_assert!(
            src_info.width() == dst_info.width() && src_info.height() == dst_info.height()
        );
        let height = usize::try_from(dst_info.height()).unwrap_or(0);
        let dst = self.0.region_mut(offset, dst_row_bytes * height);
        // SkAssertResult: the conversion must succeed.
        let converted = convert_pixels(dst_info, dst, dst_row_bytes, src_info, src, src_row_bytes);
        debug_assert!(converted);
    }

    /// Writes RGB rows from `RGBx` source rows (4 bytes per pixel, 3 kept).
    // Port of: src/gpu/BufferWriter.h#L512-L522 (chrome/m156)
    pub fn write_rgb_from_rgbx(
        &mut self,
        offset: usize,
        src: &[u8],
        src_row_bytes: usize,
        dst_row_bytes: usize,
        row_pixels: usize,
        row_count: usize,
    ) {
        let dst = self.0.region_mut(offset, dst_row_bytes * row_count);
        for y in 0..row_count {
            let s_row = y * src_row_bytes;
            let d_row = y * dst_row_bytes;
            for x in 0..row_pixels {
                dst[d_row + 3 * x..d_row + 3 * x + 3]
                    .copy_from_slice(&src[s_row + 4 * x..s_row + 4 * x + 3]);
            }
        }
    }
}

/// `SkRectMemcpy`: copies `row_count` rows of `trim_row_bytes` between rows `dst_row_bytes` and
/// `src_row_bytes` apart. Core keeps its copy private, so this is the same loop.
// Port of: src/core/SkRectMemcpy.h#L16-L30 (chrome/m156)
fn rect_memcpy(
    dst: &mut [u8],
    dst_row_bytes: usize,
    src: &[u8],
    src_row_bytes: usize,
    trim_row_bytes: usize,
    row_count: usize,
) {
    debug_assert!(trim_row_bytes <= dst_row_bytes);
    debug_assert!(trim_row_bytes <= src_row_bytes);
    for y in 0..row_count {
        let d = y * dst_row_bytes;
        let s = y * src_row_bytes;
        dst[d..d + trim_row_bytes].copy_from_slice(&src[s..s + trim_row_bytes]);
    }
}
