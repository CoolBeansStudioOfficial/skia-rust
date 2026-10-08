// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkReadBuffer.h, src/core/SkReadBuffer.cpp (the subset below)

//! `SkReadBuffer`: deserialization of what `SkBinaryWriteBuffer` wrote.
//!
//! Ported: the cursor (`skip`, `available`, `isValid`, `validate`), the version check,
//! `readInt`/`readUInt`/`read32`/`read32LE`/`readScalar`, `readPad32`, `readByteArray`,
//! `skipByteArray`, `readScalarArray`, `readString`, `readMatrix`, `readPath`, the flattenable
//! readers for path effects and mask filters, and the empty case of `readTypeface`. Not ported:
//! the deserial procs, the factory array, typeface-table and image readers, the recursion limit,
//! and every read of a type that is not ported yet (paints, ...).
//!
//! skia-rust: `SkReadBuffer` requires its memory to be 4-byte aligned because it reads words in
//! place; here the words are read from the bytes of a slice, so only the offsets are checked.

use crate::flattenable::FlattenableRegistry;
use crate::mask_filter::MaskFilter;
use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_effect::PathEffect;
use crate::typeface::Typeface;

/// Rounds `x` up to a multiple of 4 (`SkAlign4`), wrapping like the unsigned arithmetic of C++.
fn align4(x: usize) -> usize {
    x.wrapping_add(3) & !3
}

/// Reads primitives from a memory block of 4-byte words (`SkReadBuffer`). Reading past the end,
/// or any [`validate`](Self::validate) with a false condition, makes the buffer invalid for good.
// Port of: src/core/SkReadBuffer.h#L55-L286 (chrome/m156)
#[doc(alias = "SkReadBuffer")]
#[derive(Clone, Debug, Default)]
pub struct ReadBuffer<'a> {
    data: &'a [u8],
    /// `fCurr - fBase`: the current position.
    curr: usize,
    version: u32,
    error: bool,
    /// The names read so far (`fFlattenableDict`); the index of a name is its position plus one.
    flattenable_names: Vec<String>,
}

impl<'a> ReadBuffer<'a> {
    /// A buffer reading `data` (`SkReadBuffer(data, size)`). The size must be a multiple of 4,
    /// or the buffer is invalid.
    #[must_use]
    pub fn new(data: &'a [u8]) -> ReadBuffer<'a> {
        let mut buffer = ReadBuffer::default();
        buffer.set_memory(data);
        buffer
    }

    /// Makes the buffer read `data` (`setMemory`).
    // Port of: src/core/SkReadBuffer.cpp#L53-L59 (chrome/m156)
    #[doc(alias = "setMemory")]
    pub fn set_memory(&mut self, data: &'a [u8]) {
        self.validate(align4(data.len()) == data.len());
        if !self.error {
            self.data = data;
            self.curr = 0;
        }
    }

    /// True if the version is older than `target_version` (`isVersionLT`).
    // Port of: src/core/SkReadBuffer.h#L64-L69 (chrome/m156)
    #[doc(alias = "isVersionLT")]
    #[must_use]
    pub fn is_version_lt(&self, target_version: u32) -> bool {
        debug_assert!(target_version > 0);
        self.version > 0 && self.version < target_version
    }

    /// The version (`getVersion`).
    #[doc(alias = "getVersion")]
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Sets the version; at most once (`setVersion`).
    // Port of: src/core/SkReadBuffer.h#L74-L77 (chrome/m156)
    #[doc(alias = "setVersion")]
    pub fn set_version(&mut self, version: u32) {
        debug_assert!(self.version == 0 || version == self.version);
        self.version = version;
    }

    /// The number of bytes left to read (`available`).
    // Port of: src/core/SkReadBuffer.h#L85 (chrome/m156)
    #[must_use]
    pub fn available(&self) -> usize {
        self.data.len() - self.curr
    }

    /// `isAvailable(size)`.
    // Port of: src/core/SkReadBuffer.h#L258 (chrome/m156)
    fn is_available(&self, size: usize) -> bool {
        size <= self.available()
    }

    /// Marks the buffer invalid and sends the read cursor to the end (`setInvalid`).
    // Port of: src/core/SkReadBuffer.cpp#L61-L67 (chrome/m156)
    fn set_invalid(&mut self) {
        if !self.error {
            // When an error is found, send the read cursor to the end of the stream
            self.curr = self.data.len();
            self.error = true;
        }
    }

    /// If `is_valid` is false, makes the buffer invalid. Returns whether the buffer is still
    /// valid (`validate`).
    // Port of: src/core/SkReadBuffer.h#L201-L206 (chrome/m156)
    pub fn validate(&mut self, is_valid: bool) -> bool {
        if !is_valid {
            self.set_invalid();
        }
        !self.error
    }

    /// Whether nothing has gone wrong (`isValid`).
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.error
    }

    /// Skips `size` bytes (rounded up to a multiple of 4) and returns them, or `None` (and an
    /// invalid buffer) if there are not as many (`skip`).
    // Port of: src/core/SkReadBuffer.cpp#L69-L82 (chrome/m156)
    pub fn skip(&mut self, size: usize) -> Option<&'a [u8]> {
        let inc = align4(size);
        self.validate(inc >= size);
        let addr = self.curr;
        self.validate(self.curr.is_multiple_of(4) && self.is_available(inc));
        if self.error {
            return None;
        }

        self.curr += inc;
        Some(&self.data[addr..addr + inc])
    }

    /// Reads an `i32` (`readInt`).
    // Port of: src/core/SkReadBuffer.cpp#L102-L110 (chrome/m156)
    #[doc(alias = "readInt")]
    pub fn read_int(&mut self) -> i32 {
        const INC: usize = size_of::<i32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0;
        }
        let bytes = &self.data[self.curr..self.curr + INC];
        let value = i32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.curr += INC;
        value
    }

    /// Reads a `u32` (`readUInt`).
    // Port of: src/core/SkReadBuffer.cpp#L122-L124 (chrome/m156)
    #[doc(alias = "readUInt")]
    pub fn read_uint(&mut self) -> u32 {
        u32::from_ne_bytes(self.read_int().to_ne_bytes())
    }

    /// Reads an `i32` (`read32`).
    // Port of: src/core/SkReadBuffer.cpp#L126-L128 (chrome/m156)
    pub fn read32(&mut self) -> i32 {
        self.read_int()
    }

    /// Reads a scalar stored as its bit pattern (`readScalar`). Returns 0 if the buffer is
    /// invalid.
    // Port of: src/core/SkReadBuffer.cpp#L112-L120 (chrome/m156)
    #[doc(alias = "readScalar")]
    pub fn read_scalar(&mut self) -> f32 {
        const INC: usize = size_of::<f32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0.0;
        }
        let bytes = &self.data[self.curr..self.curr + INC];
        let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        self.curr += INC;
        value
    }

    /// Reads a typeface reference (`readTypeface`). Only the empty case is ported: the buffer
    /// has no typeface table and no deserial procs, so a non-zero index or custom size is
    /// invalid, as it is in C++ with no table and no `fTypefaceStreamProc`. Arrives with T16.
    // Port of: src/core/SkReadBuffer.cpp (readTypeface, chrome/m156), the `fTFCount == 0` and
    // no-proc path
    #[doc(alias = "readTypeface")]
    pub fn read_typeface(&mut self) -> Option<Typeface> {
        let index = self.read_int();
        if index == 0 {
            return None;
        }
        // Index arm: no typeface table. Custom arm: no `fTypefaceStreamProc`. Either way the
        // buffer becomes invalid (after skipping the custom bytes, as C++ does).
        if index < 0 {
            let size = usize::try_from(index.unsigned_abs()).unwrap_or(usize::MAX);
            let _ = self.skip(size);
        }
        self.validate(false);
        None
    }

    /// Reads `buffer.len()` bytes, skipping the padding up to a multiple of 4 (`readPad32`).
    // Port of: src/core/SkReadBuffer.cpp#L138-L146 (chrome/m156)
    #[doc(alias = "readPad32")]
    pub fn read_pad32(&mut self, buffer: &mut [u8]) -> bool {
        if let Some(src) = self.skip(buffer.len()) {
            buffer.copy_from_slice(&src[..buffer.len()]);
            return true;
        }
        false
    }

    /// `readArray`: reads the count of the elements, which must be `size`, then the bytes.
    // Port of: src/core/SkReadBuffer.cpp#L286-L290 (chrome/m156)
    fn read_array(&mut self, value: &mut [u8], size: usize) -> bool {
        let count = self.read_uint();
        self.validate(usize::try_from(count).ok() == Some(size)) && self.read_pad32(value)
    }

    /// Reads `value.len()` bytes written by `writeByteArray` (`readByteArray`).
    // Port of: src/core/SkReadBuffer.cpp#L292-L294 (chrome/m156)
    #[doc(alias = "readByteArray")]
    pub fn read_byte_array(&mut self, value: &mut [u8]) -> bool {
        let size = value.len();
        self.read_array(value, size)
    }

    /// Skips a byte array written by `writeByteArray` and returns its bytes (with the padding)
    /// and its size, which is 0 if the buffer is invalid (`skipByteArray`).
    // Port of: src/core/SkReadBuffer.cpp#L316-L323 (chrome/m156)
    #[doc(alias = "skipByteArray")]
    pub fn skip_byte_array(&mut self) -> (Option<&'a [u8]>, usize) {
        let count = self.read_uint();
        let buf = usize::try_from(count).ok().and_then(|c| self.skip(c));
        let size = if self.is_valid() {
            usize::try_from(count).unwrap_or(0)
        } else {
            0
        };
        (buf, size)
    }
}

impl ReadBuffer<'_> {
    /// `getArrayCount`: the count at the current position, which is not consumed. Returns 0 (and
    /// makes the buffer invalid) if there is no word left.
    // Port of: src/core/SkReadBuffer.cpp#L338-L344 (chrome/m156)
    #[doc(alias = "getArrayCount")]
    pub fn get_array_count(&mut self) -> u32 {
        const INC: usize = size_of::<u32>();
        if !self.validate(self.curr.is_multiple_of(4) && self.is_available(INC)) {
            return 0;
        }
        u32::from_ne_bytes([
            self.data[self.curr],
            self.data[self.curr + 1],
            self.data[self.curr + 2],
            self.data[self.curr + 3],
        ])
    }

    /// `validateCanReadN<T>`: whether `n` elements of `element_size` bytes are left to read.
    // Port of: src/core/SkReadBuffer.h#L214-L216 (chrome/m156)
    #[doc(alias = "validateCanReadN")]
    pub fn validate_can_read_n(&mut self, n: usize, element_size: usize) -> bool {
        self.validate(n <= self.available() / element_size)
    }

    /// Reads a 32-bit value that must not exceed `max`, or 0 (`read32LE`).
    // Port of: src/core/SkReadBuffer.h#L102-L108 (chrome/m156)
    #[doc(alias = "read32LE")]
    pub fn read32_le(&mut self, max: u32) -> u32 {
        let value = self.read_uint();
        if !self.validate(value <= max) {
            return 0;
        }
        value
    }

    /// Reads the scalars of an array whose count must be `values.len()` (`readScalarArray`).
    // Port of: src/core/SkReadBuffer.cpp#L312-L314 (chrome/m156)
    #[doc(alias = "readScalarArray")]
    pub fn read_scalar_array(&mut self, values: &mut [f32]) -> bool {
        let count = self.read_uint();
        if !self.validate(usize::try_from(count).ok() == Some(values.len())) {
            return false;
        }
        let Some(src) = self.skip(size_of_val(values)) else {
            return false;
        };
        let (chunks, _) = src.as_chunks::<{ size_of::<f32>() }>();
        for (value, chunk) in values.iter_mut().zip(chunks) {
            *value = f32::from_ne_bytes(*chunk);
        }
        true
    }

    /// Reads a string written by `writeString`: its length, its bytes and a terminating zero
    /// (`readString`). Returns `None` (and makes the buffer invalid) if it is malformed.
    // Port of: src/core/SkReadBuffer.cpp#L148-L158 (chrome/m156)
    #[doc(alias = "readString")]
    pub fn read_string(&mut self) -> Option<String> {
        let len = usize::try_from(self.read_uint()).ok()?;
        let bytes = self.skip(len.checked_add(1)?)?;
        if !self.validate(bytes[len] == 0) {
            return None;
        }
        let Ok(value) = std::str::from_utf8(&bytes[..len]) else {
            self.validate(false);
            return None;
        };
        Some(value.to_owned())
    }

    /// Reads a matrix written by `writeMatrix`: 9 scalars. The matrix is the identity if the
    /// buffer is invalid (`readMatrix`).
    // Port of: src/core/SkReadBuffer.cpp#L195-L205 (chrome/m156)
    #[doc(alias = "readMatrix")]
    pub fn read_matrix(&mut self) -> Matrix {
        const SIZE: usize = 9 * size_of::<f32>();
        let mut matrix = Matrix::new_identity();
        let mut size = 0;
        if self.is_valid() {
            // `SkMatrix::readFromMemory` reads nothing if fewer than 36 bytes are left.
            if self.is_available(SIZE) {
                let mut values = [0.0; 9];
                for (i, value) in values.iter_mut().enumerate() {
                    let at = self.curr + i * size_of::<f32>();
                    *value = f32::from_ne_bytes([
                        self.data[at],
                        self.data[at + 1],
                        self.data[at + 2],
                        self.data[at + 3],
                    ]);
                }
                matrix.set_9(&values);
                size = SIZE;
            }
            self.validate(size != 0);
        }
        if !self.is_valid() {
            matrix = Matrix::new_identity();
        }
        let _ = self.skip(size);
        matrix
    }

    /// Reads a path written by `writePath`, and moves past it whether or not it is valid
    /// (`readPath`).
    // Port of: src/core/SkReadBuffer.cpp#L267-L283 (chrome/m156)
    #[doc(alias = "readPath")]
    pub fn read_path(&mut self) -> Option<Path> {
        if !self.is_valid() {
            return None;
        }
        let (path, size) = Path::read_from_memory(&self.data[self.curr..]);
        // The path format is 4-byte aligned, and the path must have been read.
        self.validate(align4(size) == size && path.is_some());
        let _ = self.skip(size);
        path
    }

    /// Reads a path effect written by `writeFlattenable` (`readPathEffect`). `registry` maps the
    /// names to their factories.
    // Port of: src/core/SkReadBuffer.cpp#L538-L546 (chrome/m156), with the factory table passed
    // in (see `flattenable`)
    #[doc(alias = "readPathEffect")]
    pub fn read_path_effect(&mut self, registry: &FlattenableRegistry) -> Option<PathEffect> {
        let name = self.read_flattenable_name()?;
        let Some(factory) = registry.path_effect_factory(&name) else {
            self.validate(false);
            return None;
        };
        self.read_flattenable_body(|buffer| factory(buffer, registry))
    }

    /// Reads a mask filter written by `writeFlattenable` (`readMaskFilter`).
    // Port of: src/core/SkReadBuffer.cpp#L538-L546 (chrome/m156), with the factory table passed
    // in (see `flattenable`)
    #[doc(alias = "readMaskFilter")]
    pub fn read_mask_filter(&mut self, registry: &FlattenableRegistry) -> Option<MaskFilter> {
        let name = self.read_flattenable_name()?;
        let Some(factory) = registry.mask_filter_factory(&name) else {
            self.validate(false);
            return None;
        };
        self.read_flattenable_body(|buffer| factory(buffer, registry))
    }

    /// The name part of `readRawFlattenable`: a string (which is added to the dictionary) or the
    /// dictionary index of an earlier one. `None` if the writer wrote nothing, or on an error.
    // Port of: src/core/SkReadBuffer.cpp#L489-L512 (chrome/m156), the no-factory-array arm
    fn read_flattenable_name(&mut self) -> Option<String> {
        if !self.is_valid() {
            return None;
        }
        // If the first byte is non-zero, the flattenable is specified by a string. Otherwise it
        // is the index, shifted left by 8.
        if self.peek_byte() != 0 {
            let name = self.read_string()?;
            self.flattenable_names.push(name.clone());
            Some(name)
        } else {
            let index = self.read_uint() >> 8;
            if index == 0 {
                return None; // writer failed to give us the flattenable
            }
            let name = usize::try_from(index)
                .ok()
                .and_then(|index| self.flattenable_names.get(index - 1).cloned());
            if !self.validate(name.is_some()) {
                return None;
            }
            name
        }
    }

    /// The body part of `readRawFlattenable`: the recorded size, then what the factory reads,
    /// which must be exactly that size.
    // Port of: src/core/SkReadBuffer.cpp#L515-L536 (chrome/m156)
    fn read_flattenable_body<T>(
        &mut self,
        factory: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let size_recorded = self.read_uint();
        let offset = self.curr;
        let obj = factory(self);
        let size_read = self.curr - offset;
        if usize::try_from(size_recorded).ok() != Some(size_read) {
            self.validate(false);
            return None;
        }
        if !self.is_valid() {
            return None;
        }
        obj
    }

    /// The first byte at the current position, without consuming it (`peekByte`). Returns 0,
    /// and makes the buffer invalid, if there is no byte left.
    // Port of: src/core/SkReadBuffer.cpp#L130-L136 (chrome/m156)
    #[doc(alias = "peekByte")]
    fn peek_byte(&mut self) -> u8 {
        if self.available() == 0 {
            self.validate(false);
            return 0;
        }
        self.data[self.curr]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_buffer::BinaryWriteBuffer;

    fn bytes(b: &BinaryWriteBuffer) -> Vec<u8> {
        let mut out = vec![0; b.bytes_written()];
        b.write_to_memory(&mut out);
        out
    }

    #[test]
    fn reads_what_was_written() {
        let mut w = BinaryWriteBuffer::new();
        w.write_uint(0xDEAD_BEEF);
        w.write_int(-5);
        w.write_byte_array(&[1, 2, 3, 4, 5]);
        w.write_byte_array(&[6, 7]);
        let data = bytes(&w);

        let mut r = ReadBuffer::new(&data);
        assert!(r.is_valid());
        assert_eq!(r.read_uint(), 0xDEAD_BEEF);
        assert_eq!(r.read_int(), -5);
        let mut a = [0; 5];
        assert!(r.read_byte_array(&mut a));
        assert_eq!(a, [1, 2, 3, 4, 5]);
        let (skipped, size) = r.skip_byte_array();
        assert_eq!((skipped.map(<[u8]>::len), size), (Some(4), 2));
        assert_eq!(r.available(), 0);
        assert!(r.is_valid());
    }

    #[test]
    fn errors_are_sticky_and_end_the_buffer() {
        let data = [0u8; 8];
        let mut r = ReadBuffer::new(&data);
        assert_eq!(r.read_int(), 0);
        assert_eq!(r.read_int(), 0);
        assert!(r.is_valid());
        // Past the end.
        assert_eq!(r.read_int(), 0);
        assert!(!r.is_valid());
        assert_eq!(r.available(), 0);
        assert!(!r.validate(true));

        // The wrong array size.
        let mut w = BinaryWriteBuffer::new();
        w.write_byte_array(&[1, 2, 3, 4]);
        let data = bytes(&w);
        let mut r = ReadBuffer::new(&data);
        let mut a = [0; 3];
        assert!(!r.read_byte_array(&mut a));
        assert!(!r.is_valid());

        // A size that is not a multiple of 4.
        assert!(!ReadBuffer::new(&[0, 0, 0]).is_valid());
    }

    #[test]
    fn scalars_and_empty_typefaces_round_trip() {
        let mut w = BinaryWriteBuffer::new();
        w.write_scalar(1.5);
        w.write_typeface(None);
        w.write_scalar(-0.25);
        let data = bytes(&w);
        assert_eq!(data.len(), 12);

        let mut r = ReadBuffer::new(&data);
        assert_eq!(r.read_scalar(), 1.5);
        assert!(r.read_typeface().is_none());
        assert_eq!(r.read_scalar(), -0.25);
        assert!(r.is_valid());

        // A non-zero typeface index has no table to refer to.
        let mut w = BinaryWriteBuffer::new();
        w.write_int(3);
        let data = bytes(&w);
        let mut r = ReadBuffer::new(&data);
        assert!(r.read_typeface().is_none());
        assert!(!r.is_valid());
    }

    #[test]
    fn versions() {
        let mut r = ReadBuffer::new(&[]);
        assert!(!r.is_version_lt(86)); // no version: the latest
        r.set_version(85);
        assert!(r.is_version_lt(86));
        assert!(!r.is_version_lt(85));
        assert_eq!(r.version(), 85);
    }
}
