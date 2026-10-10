// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkWriter32.h, src/core/SkWriteBuffer.h, src/core/SkWriteBuffer.cpp
// (the subset below)

//! `SkWriter32` and `SkBinaryWriteBuffer`: serialization of primitives into a flat binary blob
//! of 4-byte words.
//!
//! Ported: [`Writer32`] (`reserve`, `write32`, `writeScalar`, `writePad`, `writeString`,
//! `overwriteTAt`, `bytesWritten`, `flatten`, `writeMatrix`, `writePath`) and
//! [`BinaryWriteBuffer`] (`writeInt`, `writeUInt`, `writeScalar`, `writeScalarArray`,
//! `writeByteArray`, `writePad32`, `writeMatrix`, `writePath`, `writeTypeface` (the empty case),
//! `writeFlattenable` for path effects and mask filters, `bytesWritten`, `writeToMemory`).
//! The factory set (`setFactoryRecorder`) and the typeface set (`setTypefaceRecorder`) are
//! ported: with a factory set, a flattenable is written as its index. Without one, it is written
//! by name. Not ported: external storage (`SkWriter32(void*, size_t)`, `usingInitialStorage`),
//! and every write that needs a type that is not ported yet (regions, ...). Images are written
//! with their bytes from the image procedure (no encoded data is kept by an image yet), and an
//! image with mipmap levels is not written.

use crate::alpha_type::AlphaType;
use crate::blender::Blender;
use crate::color::Color4f;
use crate::color_filter::ColorFilter;
use crate::data::Data;
use crate::image::Image;
use crate::mask_filter::MaskFilter;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::path::Path;
use crate::path_effect::PathEffect;
use crate::point::Point;
use crate::rect::Rect;
use crate::rrect::RRect;
use crate::sampling_options::SamplingOptions;
use crate::serial_procs::SerialProcs;
use crate::shader::Shader;
use crate::typeface::Typeface;

/// The image flags of `SkWriteBufferImageFlags`: unpremultiplied, and the mipmaps follow.
// Port of: src/core/SkWriteBuffer.h#L167-L174 (chrome/m156)
const IMAGE_FLAG_UNPREMUL: u32 = 1 << 10;

/// Rounds `x` up to a multiple of 4 (`SkAlign4`).
fn align4(x: usize) -> usize {
    (x + 3) & !3
}

/// The flat flag saying a paint has effects that follow the packed word (`kHasEffects_FlatFlag`).
// Port of: src/core/SkPaintPriv.cpp#L190 (chrome/m156)
pub(crate) const FLAT_HAS_EFFECTS: u8 = 0x2;

/// The blend mode value that stands for a custom blender in the packed word
/// (`CUSTOM_BLEND_MODE_SENTINEL`).
// Port of: src/core/SkPaintPriv.cpp#L205 (chrome/m156)
pub(crate) const CUSTOM_BLEND_MODE_SENTINEL: u8 = 0xFF;

/// Packs the anti-alias, dither, blend mode, caps, joins, style and the flat flags into one word
/// (`pack_v68`). The bits of the old filter quality are zero.
// Port of: src/core/SkPaintPriv.cpp#L217-L232 (chrome/m156)
fn pack_v68(paint: &Paint, flat_flags: u8) -> u32 {
    let mode = paint
        .as_blend_mode()
        .map_or(u32::from(CUSTOM_BLEND_MODE_SENTINEL), |bm| bm as u32);
    let mut packed = 0u32;
    packed |= ((u32::from(paint.is_dither()) << 1) | u32::from(paint.is_anti_alias())) & 0xFF;
    packed |= (mode & 0xFF) << 8;
    packed |= (paint.stroke_cap() as u32 & 0x3) << 16;
    packed |= (paint.stroke_join() as u32 & 0x3) << 18;
    packed |= (paint.style() as u32 & 0x3) << 20;
    // The old filter quality bits (22..24) are zero.
    packed |= u32::from(flat_flags) << 24;
    packed
}

/// A growing buffer of 4-byte words (`SkWriter32`).
// Port of: src/core/SkWriter32.h#L35-L262 (chrome/m156)
#[doc(alias = "SkWriter32")]
#[derive(Clone, Debug, Default)]
pub struct Writer32 {
    data: Vec<u8>,
}

impl Writer32 {
    /// An empty writer.
    #[must_use]
    pub fn new() -> Writer32 {
        Writer32::default()
    }

    /// An empty writer that has room for `capacity` bytes before it grows. The storage that
    /// `SkWriter32(void* storage, size_t)` writes into is not ported: the bytes always live in
    /// this writer, so only the allocation is hinted here.
    // Port of: src/core/SkWriter32.h#L43-L53 (chrome/m156), the initial storage as a capacity
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Writer32 {
        Writer32 {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Discards everything written (`reset` with no external storage).
    // Port of: src/core/SkWriter32.h#L55-L65 (chrome/m156)
    pub fn reset(&mut self) {
        self.data.clear();
    }

    /// Moves the cursor back to `offset` bytes from the beginning. `offset` must be a multiple
    /// of 4 no greater than [`bytes_written`](Self::bytes_written) (`rewindToOffset`).
    // Port of: src/core/SkWriter32.h#L229-L233 (chrome/m156)
    #[doc(alias = "rewindToOffset")]
    pub fn rewind_to_offset(&mut self, offset: usize) {
        debug_assert_eq!(align4(offset), offset);
        debug_assert!(offset <= self.bytes_written());
        self.data.truncate(offset);
    }

    /// The current offset, always a multiple of 4 (`bytesWritten`).
    // Port of: src/core/SkWriter32.h#L49 (chrome/m156)
    #[doc(alias = "bytesWritten")]
    #[must_use]
    pub fn bytes_written(&self) -> usize {
        self.data.len()
    }

    /// Reserves `size` bytes, which must be a multiple of 4, and returns them zeroed
    /// (`reserve`).
    // Port of: src/core/SkWriter32.h#L68-L77 (chrome/m156)
    pub fn reserve(&mut self, size: usize) -> &mut [u8] {
        debug_assert_eq!(align4(size), size);
        let offset = self.data.len();
        self.data.resize(offset + size, 0);
        &mut self.data[offset..]
    }

    /// Writes a 32 bit integer (`write32`).
    // Port of: src/core/SkWriter32.h#L118-L120 (chrome/m156)
    pub fn write32(&mut self, value: i32) {
        self.reserve(size_of::<i32>())
            .copy_from_slice(&value.to_ne_bytes());
    }

    /// Writes an unsigned 32 bit integer (`write32` of a `uint32_t`).
    // Port of: src/core/SkWriter32.h#L118-L120 (chrome/m156), the unsigned use
    pub fn write_u32(&mut self, value: u32) {
        self.reserve(size_of::<u32>())
            .copy_from_slice(&value.to_ne_bytes());
    }

    /// Writes a scalar as its bit pattern in one word (`writeScalar`).
    // Port of: src/core/SkWriter32.h#L122-L124 (chrome/m156)
    pub fn write_scalar(&mut self, value: f32) {
        self.reserve(size_of::<f32>())
            .copy_from_slice(&value.to_ne_bytes());
    }

    /// Writes the four scalars of `rect`, left, top, right, bottom (`writeRect`).
    // Port of: src/core/SkWriter32.h#L134-L136 (chrome/m156)
    #[doc(alias = "writeRect")]
    pub fn write_rect(&mut self, rect: &Rect) {
        self.write_scalar(rect.left);
        self.write_scalar(rect.top);
        self.write_scalar(rect.right);
        self.write_scalar(rect.bottom);
    }

    /// Writes `data` as its length, then its bytes padded to 4 bytes. `None` writes a zero
    /// length (`writeData`).
    // Port of: src/core/SkWriter32.h#L213-L219 (chrome/m156)
    #[doc(alias = "writeData")]
    pub fn write_data(&mut self, data: Option<&Data>) {
        let len = data.map_or(0, |data| u32::try_from(data.size()).unwrap_or(u32::MAX));
        self.write_u32(len);
        if let Some(data) = data {
            self.write_pad(data.as_bytes());
        }
    }

    /// The number of bytes [`write_data`](Self::write_data) writes for `data`
    /// (`WriteDataSize`).
    // Port of: src/core/SkWriter32.h#L221-L223 (chrome/m156)
    #[doc(alias = "WriteDataSize")]
    #[must_use]
    pub fn write_data_size(data: Option<&Data>) -> usize {
        4 + align4(data.map_or(0, Data::size))
    }

    /// A copy of the bytes written, as data (`snapshotAsData`).
    // Port of: src/core/SkWriter32.cpp#L78-L80 (chrome/m156)
    #[doc(alias = "snapshotAsData")]
    #[must_use]
    pub fn snapshot_as_data(&self) -> Data {
        Data::new_copy(&self.data)
    }

    /// Reserves `size` bytes, which need not be a multiple of 4: the remaining space (if any) is
    /// filled in with zeroes (`reservePad`).
    // Port of: src/core/SkWriter32.h#L180-L188 (chrome/m156)
    pub fn reserve_pad(&mut self, size: usize) -> &mut [u8] {
        let aligned_size = align4(size);
        let p = self.reserve(aligned_size);
        &mut p[..size]
    }

    /// Writes the bytes of `src`, and pads to 4 byte alignment with zeroes (`writePad`).
    // Port of: src/core/SkWriter32.h#L193-L195 (chrome/m156)
    pub fn write_pad(&mut self, src: &[u8]) {
        self.reserve_pad(src.len()).copy_from_slice(src);
    }

    /// Writes a string as its length, its bytes, a terminating `\0`, and the padding to 4 bytes
    /// (`writeString`). `None` is written as `""`.
    // Port of: src/core/SkWriter32.cpp#L38-L53 (chrome/m156)
    pub fn write_string(&mut self, value: Option<&str>) {
        let value = value.unwrap_or("");
        let len = value.len();
        let ptr = self.reserve_pad(size_of::<u32>() + len + 1);
        ptr[..4].copy_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_ne_bytes());
        ptr[4..4 + len].copy_from_slice(value.as_bytes());
    }

    /// Writes the 9 entries of a matrix (`writeMatrix`).
    // Port of: src/core/SkWriter32.cpp#L18-L22 (chrome/m156)
    pub fn write_matrix(&mut self, matrix: &Matrix) {
        let mut values = [0.0; 9];
        matrix.get_9(&mut values);
        for value in values {
            self.write_scalar(value);
        }
    }

    /// Writes `path` (`writePath`): its serialized size is not padded, as the path format is
    /// 4-byte aligned.
    // Port of: src/core/SkWriter32.h#L146-L150 (chrome/m156)
    pub fn write_path(&mut self, path: &Path) {
        let size = path.write_to_memory(None);
        let reserved = self.reserve(size);
        let _ = path.write_to_memory(Some(reserved));
    }

    /// `SkWriter32::writeSampling`: the anisotropy; then, unless it is set, whether the filter is
    /// cubic, and the cubic coefficients or the filter and mipmap modes.
    // Port of: src/core/SkWriter32.cpp#L24-L36 (chrome/m156)
    #[doc(alias = "writeSampling")]
    pub fn write_sampling(&mut self, sampling: &SamplingOptions) {
        self.write32(sampling.max_aniso);
        if !sampling.is_aniso() {
            self.write32(i32::from(sampling.use_cubic));
            if sampling.use_cubic {
                self.write_scalar(sampling.cubic.b);
                self.write_scalar(sampling.cubic.c);
            } else {
                self.write32(sampling.filter as i32);
                self.write32(sampling.mipmap as i32);
            }
        }
    }

    /// Overwrites the word at `offset` (`overwriteTAt`).
    // Port of: src/core/SkWriter32.h#L94-L98 (chrome/m156)
    pub fn overwrite32(&mut self, offset: usize, value: u32) {
        self.data[offset..offset + size_of::<u32>()].copy_from_slice(&value.to_ne_bytes());
    }

    /// Overwrites the scalar at `offset` (`overwriteTAt<SkScalar>`).
    // Port of: src/core/SkWriter32.h#L94-L98 (chrome/m156), with SkScalar
    #[doc(alias = "overwriteTAt")]
    pub fn overwrite_scalar_at(&mut self, offset: usize, value: f32) {
        self.data[offset..offset + size_of::<f32>()].copy_from_slice(&value.to_ne_bytes());
    }

    /// Overwrites the four scalars of a rectangle at `offset` (`overwriteTAt<SkRect>`).
    // Port of: src/core/SkWriter32.h#L94-L98 (chrome/m156), with SkRect
    #[doc(alias = "overwriteTAt")]
    pub fn overwrite_rect_at(&mut self, offset: usize, rect: &Rect) {
        self.overwrite_scalar_at(offset, rect.left);
        self.overwrite_scalar_at(offset + 4, rect.top);
        self.overwrite_scalar_at(offset + 8, rect.right);
        self.overwrite_scalar_at(offset + 12, rect.bottom);
    }

    /// Reads the scalar at `offset` (`readTAt<SkScalar>`).
    ///
    /// # Panics
    /// If the scalar is not within the bytes written.
    // Port of: src/core/SkWriter32.h#L84-L88 (chrome/m156), with SkScalar
    #[doc(alias = "readTAt")]
    #[must_use]
    pub fn read_scalar_at(&self, offset: usize) -> f32 {
        f32::from_bits(self.read32_at(offset))
    }

    /// Reads the four scalars of a rectangle at `offset` (`readTAt<SkRect>`).
    ///
    /// # Panics
    /// If the rectangle is not within the bytes written.
    // Port of: src/core/SkWriter32.h#L84-L88 (chrome/m156), with SkRect
    #[doc(alias = "readTAt")]
    #[must_use]
    pub fn read_rect_at(&self, offset: usize) -> Rect {
        Rect {
            left: self.read_scalar_at(offset),
            top: self.read_scalar_at(offset + 4),
            right: self.read_scalar_at(offset + 8),
            bottom: self.read_scalar_at(offset + 12),
        }
    }

    /// Reads the word at `offset` (`readTAt<uint32_t>`).
    ///
    /// # Panics
    /// If the word is not within the bytes written.
    // Port of: src/core/SkWriter32.h#L84-L88 (chrome/m156)
    #[must_use]
    pub fn read32_at(&self, offset: usize) -> u32 {
        let bytes: [u8; 4] = self.data[offset..offset + size_of::<u32>()]
            .try_into()
            .expect("four bytes");
        u32::from_ne_bytes(bytes)
    }

    /// The bytes written, taking the writer (`snapshotAsData`, without the copy).
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }

    /// Copies the bytes written into `dst`, which must be at least as large (`flatten`).
    ///
    /// # Panics
    /// If `dst` is smaller than [`bytes_written`](Self::bytes_written).
    // Port of: src/core/SkWriter32.h#L236-L238 (chrome/m156)
    pub fn flatten(&self, dst: &mut [u8]) {
        dst[..self.data.len()].copy_from_slice(&self.data);
    }
}

/// A [`Writer32`] that starts with room for `N` bytes (`SkSWriter32<N>`). The room is a capacity
/// hint: see [`Writer32::with_capacity`].
// Port of: src/core/SkWriter32.h#L264-L284 (chrome/m156)
#[doc(alias = "SkSWriter32")]
#[derive(Clone, Debug)]
pub struct SWriter32<const N: usize>(Writer32);

impl<const N: usize> Default for SWriter32<N> {
    fn default() -> Self {
        SWriter32(Writer32::with_capacity(N))
    }
}

impl<const N: usize> std::ops::Deref for SWriter32<N> {
    type Target = Writer32;

    fn deref(&self) -> &Writer32 {
        &self.0
    }
}

impl<const N: usize> std::ops::DerefMut for SWriter32<N> {
    fn deref_mut(&mut self) -> &mut Writer32 {
        &mut self.0
    }
}

/// Serializes to a flat binary blob (`SkBinaryWriteBuffer`; see the module docs for what is
/// ported).
// Port of: src/core/SkWriteBuffer.h#L98-L165 (chrome/m156)
#[doc(alias = "SkBinaryWriteBuffer")]
#[derive(Clone, Debug, Default)]
pub struct BinaryWriteBuffer {
    writer: Writer32,
    /// `fFlattenableDict`: the names already written, in order; the index of a name is its
    /// position plus one.
    flattenable_dict: Vec<String>,
    /// `fProcs`: how the typefaces (and later the images) are written.
    serial_procs: SerialProcs,
    /// `fTFSet`: the typefaces written by index, in order of first use (`setTypefaceRecorder`).
    typeface_recorder: Option<Vec<Typeface>>,
    /// `fFactorySet`: the factories written by index, in order of first use
    /// (`setFactoryRecorder`). A factory is identified by its name.
    factory_recorder: Option<Vec<String>>,
    /// Set when a flattenable without a name was written (its type is not flattenable, or a
    /// flattenable nested in it is not). Such a buffer cannot be read back, so the picture writer
    /// refuses the paint or picture that holds it.
    flatten_failed: bool,
}

impl BinaryWriteBuffer {
    /// An empty buffer (with no serial procs).
    #[must_use]
    pub fn new() -> BinaryWriteBuffer {
        BinaryWriteBuffer::default()
    }

    /// `SkBinaryWriteBuffer(const SkSerialProcs& procs)`: a buffer that writes with `procs`.
    // Port of: include/core/SkSerialProcs.h and src/core/SkWriteBuffer.h#L44 (chrome/m156)
    #[must_use]
    pub fn with_serial_procs(serial_procs: SerialProcs) -> BinaryWriteBuffer {
        BinaryWriteBuffer {
            serial_procs,
            ..BinaryWriteBuffer::default()
        }
    }

    /// `setTypefaceRecorder`: from now on, typefaces are written as indices into a set that
    /// [`typeface_recorder`](Self::typeface_recorder) returns. The set starts empty.
    // Port of: src/core/SkWriteBuffer.h (setTypefaceRecorder, chrome/m156), with the set as a Vec
    #[doc(alias = "setTypefaceRecorder")]
    pub fn set_typeface_recorder(&mut self) {
        self.typeface_recorder = Some(Vec::new());
    }

    /// The typefaces of the recorder, in index order (index 1 is the first). `None` if no
    /// recorder is set.
    #[must_use]
    #[doc(alias = "typefaceRecorder")]
    pub fn typeface_recorder(&self) -> Option<&[Typeface]> {
        self.typeface_recorder.as_deref()
    }

    /// `setFactoryRecorder`: from now on, a flattenable is written as its index in a set of
    /// factories, which [`factory_recorder`](Self::factory_recorder) returns. The set starts empty.
    // Port of: src/core/SkWriteBuffer.h (setFactoryRecorder, chrome/m156), with the set as a Vec
    #[doc(alias = "setFactoryRecorder")]
    pub fn set_factory_recorder(&mut self) {
        self.factory_recorder = Some(Vec::new());
    }

    /// The factories of the recorder, by name, in index order (index 1 is the first). `None` if
    /// no recorder is set.
    #[must_use]
    #[doc(alias = "factoryRecorder")]
    pub fn factory_recorder(&self) -> Option<&[String]> {
        self.factory_recorder.as_deref()
    }

    /// `writePoint`: the two scalars of a point (`SkBinaryWriteBuffer::writePoint`).
    // Port of: src/core/SkWriteBuffer.cpp#L101-L104 (chrome/m156)
    #[doc(alias = "writePoint")]
    pub fn write_point(&mut self, point: Point) {
        self.write_scalar(point.x);
        self.write_scalar(point.y);
    }

    /// `writeRect`: the four scalars of a rectangle (`SkBinaryWriteBuffer::writeRect`).
    // Port of: src/core/SkWriteBuffer.cpp#L127-L129 and src/core/SkWriter32.cpp (writeRect)
    #[doc(alias = "writeRect")]
    pub fn write_rect(&mut self, rect: &Rect) {
        self.write_scalar(rect.left);
        self.write_scalar(rect.top);
        self.write_scalar(rect.right);
        self.write_scalar(rect.bottom);
    }

    /// Writes a 4-byte-aligned scalar array: its count, then the scalars (`writeScalarArray`).
    // Port of: src/core/SkWriteBuffer.cpp#L61-L64 (chrome/m156)
    #[doc(alias = "writeScalarArray")]
    pub fn write_scalar_array(&mut self, values: &[f32]) {
        self.writer
            .write32(i32::try_from(values.len()).unwrap_or(i32::MAX));
        for &value in values {
            self.writer.write_scalar(value);
        }
    }

    /// Writes the 9 entries of `matrix` (`writeMatrix`).
    // Port of: src/core/SkWriteBuffer.cpp#L119-L121 (chrome/m156)
    #[doc(alias = "writeMatrix")]
    pub fn write_matrix(&mut self, matrix: &Matrix) {
        self.writer.write_matrix(matrix);
    }

    /// Writes `path` (`writePath`).
    // Port of: src/core/SkWriteBuffer.cpp#L139-L141 (chrome/m156)
    #[doc(alias = "writePath")]
    pub fn write_path(&mut self, path: &Path) {
        self.writer.write_path(path);
    }

    /// `writeSampling`: the sampling options (`SkBinaryWriteBuffer::writeSampling`).
    // Port of: src/core/SkWriteBuffer.cpp#L135-L137 (chrome/m156), writeSampling
    #[doc(alias = "writeSampling")]
    pub fn write_sampling(&mut self, sampling: &SamplingOptions) {
        self.writer.write_sampling(sampling);
    }

    /// Writes an image (`SkBinaryWriteBuffer::writeImage`): the flags, then the bytes that the
    /// image procedure makes of it, or an empty byte array when there is none (the image keeps no
    /// encoded data here, so `refEncodedData` never supplies them). The mipmap levels are not
    /// written yet: an image that has them is refused, and nothing is written.
    // Port of: src/core/SkWriteBuffer.cpp#L204-L221 (chrome/m156), writeImage, without the
    // mipmap levels (`serialize_mipmap`)
    #[doc(alias = "writeImage")]
    pub fn write_image(&mut self, image: &Image) -> bool {
        if image.has_mipmaps() {
            return false;
        }
        let flags = if image.alpha_type() == AlphaType::Unpremul {
            IMAGE_FLAG_UNPREMUL
        } else {
            0
        };
        self.write_uint(flags);
        // `serialize_image`: the procedure's data, or none (`writeDataAsByteArray(nullptr)`).
        let data = self
            .serial_procs
            .image
            .as_ref()
            .and_then(|serialize| serialize(image));
        match data {
            Some(data) => self.write_byte_array(data.as_bytes()),
            None => self.writer.write32(0),
        }
        true
    }

    /// Writes a path effect (`writeFlattenable(effect)`): `0` for none, otherwise its name (or
    /// the index of an earlier name), its size, and its flattened body.
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156), with the name arm only
    #[doc(alias = "writeFlattenable")]
    pub fn write_path_effect(&mut self, effect: Option<&PathEffect>) {
        match effect {
            None => self.writer.write32(0),
            Some(effect) => {
                let base = effect.as_base();
                self.write_flattenable(base.type_name(), |buffer| base.flatten(buffer));
            }
        }
    }

    /// Writes a mask filter (`writeFlattenable(filter)`), as [`write_path_effect`](Self::write_path_effect).
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156), with the name arm only
    #[doc(alias = "writeFlattenable")]
    pub fn write_mask_filter(&mut self, filter: Option<&MaskFilter>) {
        match filter {
            None => self.writer.write32(0),
            Some(filter) => {
                let base = filter.as_base();
                self.write_flattenable(base.type_name(), |buffer| base.flatten(buffer));
            }
        }
    }

    /// Writes a shader (`writeFlattenable(shader)`), or the null marker. The shader's
    /// `flatten` writes its parameters; a shader that has no type name marks the buffer as failed
    /// (see [`flatten_failed`](Self::flatten_failed)).
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156), `writeFlattenable` of a shader
    #[doc(alias = "writeFlattenable")]
    pub fn write_shader(&mut self, shader: Option<&Shader>) {
        match shader {
            None => self.writer.write32(0),
            Some(shader) => {
                let base = shader.as_base();
                self.write_flattenable(base.type_name(), |buffer| base.flatten(buffer));
            }
        }
    }

    /// Writes a color filter (`writeFlattenable(filter)`), or the null marker.
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156), `writeFlattenable` of a filter
    #[doc(alias = "writeFlattenable")]
    pub fn write_color_filter(&mut self, filter: Option<&ColorFilter>) {
        match filter {
            None => self.writer.write32(0),
            Some(filter) => {
                let base = filter.as_base();
                self.write_flattenable(base.type_name(), |buffer| base.flatten(buffer));
            }
        }
    }

    /// Writes a blender (`writeFlattenable(blender)`), or the null marker.
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156), `writeFlattenable` of a blender
    #[doc(alias = "writeFlattenable")]
    pub fn write_blender(&mut self, blender: Option<&Blender>) {
        match blender {
            None => self.writer.write32(0),
            Some(blender) => {
                let base = blender.as_base();
                self.write_flattenable(base.type_name(), |buffer| base.flatten(buffer));
            }
        }
    }

    /// True if a flattenable without a name was written, so the buffer cannot be read back.
    #[must_use]
    pub fn flatten_failed(&self) -> bool {
        self.flatten_failed
    }

    /// Writes a paint (`writePaint`): the stroke width and miter, the color, and the packed flags,
    /// then the effects if the paint has any (`SkPaintPriv::Flatten`).
    ///
    /// Returns false, if the paint has an image filter (not written yet) or an effect that cannot
    /// be flattened. The buffer is then not usable.
    // Port of: src/core/SkPaintPriv.cpp#L261-L287 (chrome/m156), with the arms of the
    // flattenables that are ported; the image filter is written as null
    #[doc(alias = "writePaint")]
    pub fn write_paint(&mut self, paint: &Paint) -> bool {
        if paint.image_filter().is_some() {
            return false;
        }
        let path_effect = paint.path_effect();
        let shader = paint.shader();
        let mask_filter = paint.mask_filter();
        let color_filter = paint.color_filter();
        let blender = paint.blender();
        // The paint takes the simple form when it has no effects. A blend mode is not an effect
        // (a paint with one has a blender, which `asBlendMode` sees through).
        let has_effects = path_effect.is_some()
            || shader.is_some()
            || mask_filter.is_some()
            || color_filter.is_some()
            || paint.as_blend_mode().is_none();
        let flat_flags = if has_effects { FLAT_HAS_EFFECTS } else { 0 };

        self.write_scalar(paint.stroke_width());
        self.write_scalar(paint.stroke_miter());
        self.write_color4f(paint.color4f());
        self.write_uint(pack_v68(paint, flat_flags));

        if has_effects {
            self.write_path_effect(path_effect.as_ref());
            self.write_shader(shader.as_ref());
            self.write_mask_filter(mask_filter.as_ref());
            self.write_color_filter(color_filter.as_ref());
            // The image filter is null.
            self.writer.write32(0);
            self.write_blender(blender.as_ref());
        }
        !self.flatten_failed
    }

    /// Writes a color as its four scalars (`writeColor4f`).
    // Port of: src/core/src/core/SkWriteBuffer.cpp#L92-L94 (chrome/m156), writeColor4f
    #[doc(alias = "writeColor4f")]
    pub fn write_color4f(&mut self, color: Color4f) {
        self.write_scalar(color.r);
        self.write_scalar(color.g);
        self.write_scalar(color.b);
        self.write_scalar(color.a);
    }

    /// Writes the bounds and radii of a round rectangle, without its type (`writeRRect`).
    // Port of: src/core/SkWriter32.h#L142-L144 (chrome/m156)
    #[doc(alias = "writeRRect")]
    pub fn write_rrect(&mut self, rrect: &RRect) {
        let mut bytes = Vec::new();
        rrect.write_to_memory(&mut bytes);
        self.writer
            .reserve(RRect::SIZE_IN_MEMORY)
            .copy_from_slice(&bytes);
    }

    /// The shared body of `writeFlattenable`: the factory index (when a factory recorder is set),
    /// else the name (or its dictionary index), then the size, and what `flatten` writes.
    // Port of: src/core/SkWriteBuffer.cpp#L265-L313 (chrome/m156)
    fn write_flattenable(&mut self, name: &str, flatten: impl FnOnce(&mut Self)) {
        if name.is_empty() {
            // The object has no name, so it cannot be read back (`getTypeName` is null in C++).
            self.flatten_failed = true;
        }
        if let Some(factories) = self.factory_recorder.as_mut() {
            // `fFactorySet->add(factory)`: the index is the position plus one, and a new factory
            // is appended. The index is written as it is (it is not shifted).
            let position = factories
                .iter()
                .position(|known| known == name)
                .unwrap_or_else(|| {
                    factories.push(name.to_owned());
                    factories.len() - 1
                });
            let index = i32::try_from(position + 1).unwrap_or(i32::MAX);
            self.writer.write32(index);
        } else if let Some(position) = self.flattenable_dict.iter().position(|known| known == name)
        {
            // The index is shifted left by 8, so its first byte is zero: this marks an index.
            let index = i32::try_from(position + 1).unwrap_or(i32::MAX);
            self.writer.write32(index << 8);
        } else {
            self.writer.write_string(Some(name));
            self.flattenable_dict.push(name.to_owned());
        }
        // Make room for the size of the flattened object, then record it afterwards.
        self.writer.reserve(size_of::<u32>());
        let offset = self.writer.bytes_written();
        flatten(self);
        let obj_size = self.writer.bytes_written() - offset;
        self.writer.overwrite32(
            offset - size_of::<u32>(),
            u32::try_from(obj_size).unwrap_or(u32::MAX),
        );
    }

    /// Writes the bytes of `buffer`, padded to 4 bytes (`writePad32`).
    // Port of: src/core/SkWriteBuffer.h#L107-L109 (chrome/m156)
    #[doc(alias = "writePad32")]
    pub fn write_pad32(&mut self, buffer: &[u8]) {
        self.writer.write_pad(buffer);
    }

    /// The number of bytes written (`bytesWritten`).
    #[doc(alias = "bytesWritten")]
    #[must_use]
    pub fn bytes_written(&self) -> usize {
        self.writer.bytes_written()
    }

    /// `SkBinaryWriteBuffer::snapshotAsData`: the bytes written so far, as data.
    // Port of: src/core/SkWriteBuffer.cpp (snapshotAsData, chrome/m156)
    #[doc(alias = "snapshotAsData")]
    #[must_use]
    pub fn snapshot_as_data(&self) -> crate::data::Data {
        let mut bytes = vec![0u8; self.bytes_written()];
        self.writer.flatten(&mut bytes);
        crate::data::Data::new_from_vec(bytes)
    }

    /// Writes the size of `data` and the bytes padded to 4 (`writeByteArray`).
    ///
    /// # Panics
    /// If `data` is larger than `u32::MAX` bytes (`SkToU32`).
    // Port of: src/core/SkWriteBuffer.cpp#L48-L51 (chrome/m156)
    #[doc(alias = "writeByteArray")]
    pub fn write_byte_array(&mut self, data: &[u8]) {
        self.writer.write32(i32::from_ne_bytes(
            u32::try_from(data.len())
                .expect("SkToU32(size)")
                .to_ne_bytes(),
        ));
        self.writer.write_pad(data);
    }

    /// Writes a 32 bit integer (`writeInt`).
    // Port of: src/core/SkWriteBuffer.cpp#L66-L68 (chrome/m156)
    #[doc(alias = "writeInt")]
    pub fn write_int(&mut self, value: i32) {
        self.writer.write32(value);
    }

    /// Writes a 32 bit unsigned integer (`writeUInt`).
    // Port of: src/core/SkWriteBuffer.cpp#L75-L77 (chrome/m156)
    #[doc(alias = "writeUInt")]
    pub fn write_uint(&mut self, value: u32) {
        self.writer.write32(i32::from_ne_bytes(value.to_ne_bytes()));
    }

    /// Writes a boolean as a 32 bit word, 0 or 1 (`writeBool`).
    // Port of: src/core/SkWriteBuffer.cpp#L53-L55 (chrome/m156)
    #[doc(alias = "writeBool")]
    pub fn write_bool(&mut self, value: bool) {
        self.writer.write32(i32::from(value));
    }

    /// Writes a scalar (`writeScalar`).
    // Port of: src/core/SkWriteBuffer.cpp#L57-L59 (chrome/m156)
    #[doc(alias = "writeScalar")]
    pub fn write_scalar(&mut self, value: f32) {
        self.writer.write_scalar(value);
    }

    /// Writes a typeface reference (`writeTypeface`). A null typeface, or one the serial proc
    /// declines, is written as the default: `0`, the empty font (the typeface set arm, the index,
    /// arrives with picture serialization, T16). A typeface the proc writes is a negative size and
    /// the bytes, padded to 4.
    // Port of: src/core/SkWriteBuffer.cpp#L226-L251 (chrome/m156), the custom arm and the
    // `fTFSet == nullptr` fallback
    #[doc(alias = "writeTypeface")]
    pub fn write_typeface(&mut self, typeface: Option<&Typeface>) {
        // Write 32 bits (signed): 0 is the empty font, >0 an index, <0 custom (serial procs).
        let Some(typeface) = typeface else {
            self.writer.write32(0);
            return;
        };
        let custom = self
            .serial_procs
            .typeface
            .as_ref()
            .and_then(|serialize| serialize(typeface));
        if let Some(data) = custom {
            // C++ falls back to the default font when the size does not fit in an `int32_t`.
            let size = i32::try_from(data.size()).unwrap_or(0);
            // Negative to signal custom.
            self.writer.write32(-size);
            if size != 0 {
                self.write_pad32(data.as_bytes());
            }
            return;
        }
        // No data means fall through for the standard behaviour: the index in the set, which
        // `SkRefCntSet::add` makes if the typeface is new.
        let index = match self.typeface_recorder.as_mut() {
            Some(set) => {
                let position = set
                    .iter()
                    .position(|t| t.ptr_eq(typeface))
                    .unwrap_or_else(|| {
                        set.push(typeface.clone());
                        set.len() - 1
                    });
                i32::try_from(position + 1).unwrap_or(i32::MAX)
            }
            None => 0,
        };
        self.writer.write32(index);
    }

    /// Copies the bytes written into `dst` (`writeToMemory`).
    ///
    /// # Panics
    /// If `dst` is smaller than [`bytes_written`](Self::bytes_written).
    // Port of: src/core/SkWriteBuffer.h#L151 (chrome/m156)
    #[doc(alias = "writeToMemory")]
    pub fn write_to_memory(&self, dst: &mut [u8]) {
        self.writer.flatten(dst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_padding() {
        let mut w = Writer32::new();
        w.write32(-2);
        w.write_pad(&[1, 2, 3, 4, 5]);
        assert_eq!(w.bytes_written(), 4 + 8);
        let mut out = [0xAA; 12];
        w.flatten(&mut out);
        assert_eq!(out[..4], (-2i32).to_ne_bytes());
        assert_eq!(out[4..], [1, 2, 3, 4, 5, 0, 0, 0]);
        // `reservePad` of a multiple of 4 adds no padding.
        assert_eq!(w.reserve_pad(4).len(), 4);
        assert_eq!(w.bytes_written(), 16);
    }

    #[test]
    fn byte_arrays_have_their_size_first() {
        let mut b = BinaryWriteBuffer::new();
        b.write_uint(7);
        b.write_int(-1);
        b.write_byte_array(&[9, 8, 7]);
        b.write_byte_array(&[]);
        assert_eq!(b.bytes_written(), 4 + 4 + (4 + 4) + 4);
        let mut out = vec![0; b.bytes_written()];
        b.write_to_memory(&mut out);
        assert_eq!(out[..4], 7u32.to_ne_bytes());
        assert_eq!(out[4..8], (-1i32).to_ne_bytes());
        assert_eq!(out[8..12], 3u32.to_ne_bytes());
        assert_eq!(out[12..16], [9, 8, 7, 0]);
        assert_eq!(out[16..], 0u32.to_ne_bytes());
    }
}
