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
//! Not ported: external storage (`SkWriter32(void*, size_t)`, `usingInitialStorage`), the
//! `SkSerialProcs`, the factory and typeface sets (so a flattenable is always written by name),
//! and every write that needs a type that is not ported yet (images, paints, regions, ...).

use crate::mask_filter::MaskFilter;
use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_effect::PathEffect;

/// Rounds `x` up to a multiple of 4 (`SkAlign4`).
fn align4(x: usize) -> usize {
    (x + 3) & !3
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

    /// Writes a scalar as its bit pattern in one word (`writeScalar`).
    // Port of: src/core/SkWriter32.h#L122-L124 (chrome/m156)
    pub fn write_scalar(&mut self, value: f32) {
        self.reserve(size_of::<f32>())
            .copy_from_slice(&value.to_ne_bytes());
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
    /// (`writeString`).
    // Port of: src/core/SkWriter32.cpp#L38-L55 (chrome/m156)
    pub fn write_string(&mut self, value: &str) {
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

    /// Overwrites the word at `offset` (`overwriteTAt`).
    // Port of: src/core/SkWriter32.h#L94-L98 (chrome/m156)
    pub fn overwrite32(&mut self, offset: usize, value: u32) {
        self.data[offset..offset + size_of::<u32>()].copy_from_slice(&value.to_ne_bytes());
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
}

impl BinaryWriteBuffer {
    /// An empty buffer (with no serial procs).
    #[must_use]
    pub fn new() -> BinaryWriteBuffer {
        BinaryWriteBuffer::default()
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

    /// The shared body of `writeFlattenable`: the name (or its dictionary index), the size, and
    /// what `flatten` writes.
    // Port of: src/core/SkWriteBuffer.cpp#L283-L313 (chrome/m156), the name arm
    fn write_flattenable(&mut self, name: &str, flatten: impl FnOnce(&mut Self)) {
        if let Some(position) = self.flattenable_dict.iter().position(|known| known == name) {
            // The index is shifted left by 8, so its first byte is zero: this marks an index.
            let index = i32::try_from(position + 1).unwrap_or(i32::MAX);
            self.writer.write32(index << 8);
        } else {
            self.writer.write_string(name);
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

    /// Writes a scalar (`writeScalar`).
    // Port of: src/core/SkWriteBuffer.cpp#L57-L59 (chrome/m156)
    #[doc(alias = "writeScalar")]
    pub fn write_scalar(&mut self, value: f32) {
        self.writer.write_scalar(value);
    }

    /// Writes a typeface reference (`writeTypeface`). Only the empty case is ported: without a
    /// typeface set or serial procs, C++ writes `0` for every typeface, null or not. The index
    /// and custom (serial proc) arms arrive with picture serialization (T16).
    // Port of: src/core/SkWriteBuffer.cpp#L226-L251 (chrome/m156), the `fTFSet == nullptr`
    // and `fProcs.fTypefaceProc == nullptr` path
    #[doc(alias = "writeTypeface")]
    pub fn write_typeface(&mut self, _typeface: Option<&crate::typeface::Typeface>) {
        self.writer.write32(0);
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
