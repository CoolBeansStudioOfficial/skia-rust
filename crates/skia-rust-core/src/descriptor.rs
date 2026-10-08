// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDescriptor.h, src/core/SkDescriptor.cpp

//! [`Descriptor`] and [`AutoDescriptor`]: the flat, checksummed key that identifies a scaler
//! context (`SkDescriptor`).
//!
//! A descriptor is a byte image with a 12-byte header followed by entries. The layout is the
//! C++ one, so lengths, counts, checksums and entry offsets are the same numbers Skia uses:
//!
//! ```text
//! offset 0  u32 fChecksum   hash of bytes [4, fLength)
//! offset 4  u32 fLength     total size in bytes (the header is 12 bytes)
//! offset 8  u32 fCount      number of entries
//! then fCount entries: { u32 fTag, u32 fLen } followed by fLen bytes of data (4-byte aligned)
//! ```
//!
//! Integers are stored in native byte order, as C++ stores them. Only the bytes covered by
//! `fLength` are meaningful.

use std::fmt;

use crate::checksum::hash32;
use crate::font_types::set_four_byte_tag;
use crate::scaler_context::SCALER_CONTEXT_REC_SIZE;

/// `kRec_SkDescriptorTag`: the entry holding a [`ScalerContextRec`](crate::scaler_context::ScalerContextRec).
// Port of: src/core/SkScalerContext.h#L483 (chrome/m156)
#[doc(alias = "kRec_SkDescriptorTag")]
pub const REC_TAG: u32 = set_four_byte_tag(b's', b'r', b'e', b'c');

/// `kEffects_SkDescriptorTag`: the entry holding the flattened path and mask effects.
// Port of: src/core/SkScalerContext.h#L484 (chrome/m156)
#[doc(alias = "kEffects_SkDescriptorTag")]
pub const EFFECTS_TAG: u32 = set_four_byte_tag(b'e', b'f', b'c', b't');

/// `sizeof(SkDescriptor)`: the header is `fChecksum`, `fLength`, `fCount`.
const HEADER_SIZE: usize = 12;
/// `sizeof(SkDescriptor::Entry)`: `fTag` and `fLen`.
const ENTRY_SIZE: usize = 8;
const HEADER_SIZE_U32: u32 = 12;
const ENTRY_SIZE_U32: u32 = 8;

const fn is_align4(x: usize) -> bool {
    x.is_multiple_of(4)
}

/// A byte image of a scaler-context key: header, then tagged, length-prefixed entries
/// (`SkDescriptor`).
///
/// The `Vec` is the allocation. Its length is the capacity (`SkDescriptor::Alloc`), and the
/// header's `fLength` is the number of bytes in use. Entries are appended with
/// [`Descriptor::add_entry`]; the header is then checksummed with
/// [`Descriptor::compute_checksum`].
#[doc(alias = "SkDescriptor")]
#[derive(Clone)]
pub struct Descriptor {
    bytes: Vec<u8>,
}

impl Descriptor {
    /// `SkDescriptor::ComputeOverhead`: the size of a descriptor with `entry_count` entries and no
    /// data.
    // Port of: src/core/SkDescriptor.h#L27-L30 (chrome/m156)
    #[doc(alias = "ComputeOverhead")]
    #[must_use]
    pub const fn compute_overhead(entry_count: usize) -> usize {
        HEADER_SIZE + entry_count * ENTRY_SIZE
    }

    /// `SkDescriptor::Alloc(length)`: an empty descriptor with `length` bytes of capacity.
    ///
    /// # Panics
    ///
    /// Panics if `length` is smaller than the header or not a multiple of 4.
    // Port of: src/core/SkDescriptor.cpp#L20-L24 (chrome/m156)
    #[doc(alias = "Alloc")]
    #[must_use]
    pub fn alloc(length: usize) -> Self {
        assert!(length >= HEADER_SIZE && is_align4(length));
        let mut bytes = vec![0u8; length];
        // SkDescriptor{}: fChecksum = 0, fLength = sizeof(SkDescriptor), fCount = 0.
        bytes[4..8].copy_from_slice(&HEADER_SIZE_U32.to_ne_bytes());
        Self { bytes }
    }

    /// `SkDescriptor::getLength`: bytes in use, including the header.
    // Port of: src/core/SkDescriptor.h#L42 (chrome/m156)
    #[doc(alias = "getLength")]
    #[must_use]
    pub fn length(&self) -> u32 {
        self.read_u32(4)
    }

    /// `SkDescriptor::getCount`: the number of entries.
    // Port of: src/core/SkDescriptor.h#L71 (chrome/m156)
    #[doc(alias = "getCount")]
    #[must_use]
    pub fn count(&self) -> u32 {
        self.read_u32(8)
    }

    /// `SkDescriptor::getChecksum`: the checksum stored in the header.
    // Port of: src/core/SkDescriptor.h#L64 (chrome/m156)
    #[doc(alias = "getChecksum")]
    #[must_use]
    pub fn checksum(&self) -> u32 {
        self.read_u32(0)
    }

    /// `SkDescriptor::addEntry`: appends an entry with `length` bytes of data.
    ///
    /// When `data` is `Some`, its first `length` bytes are copied in. When it is `None` only the
    /// entry header and length are written; fill the data later with
    /// [`Descriptor::find_entry_mut`].
    ///
    /// # Panics
    ///
    /// Panics where C++ asserts (`tag != 0`, a 4-byte aligned length, no duplicate tag), and when
    /// the entry would not fit the allocation.
    // Port of: src/core/SkDescriptor.cpp#L35-L50 (chrome/m156)
    #[doc(alias = "addEntry")]
    pub fn add_entry(&mut self, tag: u32, length: usize, data: Option<&[u8]>) {
        assert!(tag != 0);
        assert!(is_align4(length));
        assert!(self.find_entry(tag).is_none());
        let entry = self.length() as usize;
        assert!(
            entry + ENTRY_SIZE <= self.bytes.len(),
            "descriptor capacity exceeded"
        );
        let len_u32 = u32::try_from(length).expect("SkToU32");
        self.bytes[entry..entry + 4].copy_from_slice(&tag.to_ne_bytes());
        self.bytes[entry + 4..entry + 8].copy_from_slice(&len_u32.to_ne_bytes());
        if let Some(data) = data {
            let end = entry + ENTRY_SIZE + length;
            assert!(end <= self.bytes.len(), "descriptor capacity exceeded");
            self.bytes[entry + ENTRY_SIZE..end].copy_from_slice(&data[..length]);
        }
        let count = self.count() + 1;
        self.write_u32(8, count);
        let new_length = self.length() + len_u32 + ENTRY_SIZE_U32;
        self.write_u32(4, new_length);
    }

    /// `SkDescriptor::computeChecksum`: stores the checksum of bytes `[4, fLength)` in the
    /// header.
    // Port of: src/core/SkDescriptor.cpp#L52-L54 (chrome/m156)
    #[doc(alias = "computeChecksum")]
    pub fn compute_checksum(&mut self) {
        let checksum = self.compute_checksum_value();
        self.write_u32(0, checksum);
    }

    /// `SkDescriptor::ComputeChecksum`: the checksum of bytes `[4, fLength)`, skipping the
    /// checksum field itself.
    // Port of: src/core/SkDescriptor.cpp#L106-L110 (chrome/m156)
    #[doc(alias = "ComputeChecksum")]
    fn compute_checksum_value(&self) -> u32 {
        let end = self.length() as usize;
        hash32(&self.bytes[4..end], 0)
    }

    /// `SkDescriptor::findEntry`: the data of the first entry with `tag`, or `None`.
    ///
    /// The slice is exactly the entry's `fLen` bytes, so its length is the C++ out-parameter.
    // Port of: src/core/SkDescriptor.cpp#L56-L70 (chrome/m156)
    #[doc(alias = "findEntry")]
    #[must_use]
    pub fn find_entry(&self, tag: u32) -> Option<&[u8]> {
        let range = self.entry_data_range(tag)?;
        self.bytes.get(range)
    }

    /// Mutable form of [`Descriptor::find_entry`], for filling an entry after
    /// [`Descriptor::add_entry`] with `data: None`.
    #[must_use]
    pub fn find_entry_mut(&mut self, tag: u32) -> Option<&mut [u8]> {
        let range = self.entry_data_range(tag)?;
        self.bytes.get_mut(range)
    }

    /// `SkDescriptor::isValid`: checks the header and every entry against `fLength`, and that the
    /// rec entry has the size of [`ScalerContextRec`](crate::scaler_context::ScalerContextRec).
    ///
    /// Entry headers are read only where they lie inside the allocation; a `fLength` larger than
    /// the allocation (which C++ leaves undefined) makes the descriptor invalid here.
    // Port of: src/core/SkDescriptor.cpp#L112-L143 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let mut count = self.count();
        let mut length_remaining = self.length() as usize;
        if length_remaining < HEADER_SIZE || !is_align4(length_remaining) {
            return false;
        }
        length_remaining -= HEADER_SIZE;
        let mut offset = HEADER_SIZE;
        while length_remaining > 0 && count > 0 {
            if length_remaining < ENTRY_SIZE {
                return false;
            }
            length_remaining -= ENTRY_SIZE;
            let (Some(tag), Some(entry_len)) =
                (self.read_u32_at(offset), self.read_u32_at(offset + 4))
            else {
                return false;
            };
            let entry_len = entry_len as usize;
            if length_remaining < entry_len || !is_align4(entry_len) {
                return false;
            }
            length_remaining -= entry_len;
            if tag == REC_TAG && entry_len != SCALER_CONTEXT_REC_SIZE {
                return false;
            }
            offset += ENTRY_SIZE + entry_len;
            count -= 1;
        }
        length_remaining == 0 && count == 0
    }

    /// Set the `fLength` header field, as the C++ `DescriptorTest` helper `SetLength` does.
    /// Test-only: it makes the header lie, which is what the validity tests check.
    // Port of: tests/DescriptorTest.cpp#L25 (SkDescriptorTestHelper::SetLength, chrome/m156)
    #[doc(hidden)]
    pub fn set_length_for_testing(&mut self, length: u32) {
        self.write_u32(4, length);
    }

    /// Set the `fCount` header field, as the C++ `DescriptorTest` helper `SetCount` does.
    /// Test-only.
    // Port of: tests/DescriptorTest.cpp#L26 (SkDescriptorTestHelper::SetCount, chrome/m156)
    #[doc(hidden)]
    pub fn set_count_for_testing(&mut self, count: u32) {
        self.write_u32(8, count);
    }

    /// `SkDescriptor::copy`: a new descriptor holding exactly the bytes in use. `AutoDescriptor`
    /// resets through this too (`Alloc(desc.getLength())` plus `memcpy`).
    // Port of: src/core/SkDescriptor.cpp#L72-L76 (chrome/m156)
    #[must_use]
    pub fn copy(&self) -> Self {
        let end = self.length() as usize;
        Self {
            bytes: self.bytes[..end].to_vec(),
        }
    }

    /// Byte range of the data of the first entry with `tag`.
    // Port of: src/core/SkDescriptor.cpp#L56-L70 (chrome/m156), the walk of findEntry
    fn entry_data_range(&self, tag: u32) -> Option<std::ops::Range<usize>> {
        let mut offset = HEADER_SIZE;
        for _ in 0..self.count() {
            let entry_tag = self.read_u32_at(offset)?;
            let entry_len = self.read_u32_at(offset + 4)? as usize;
            if entry_tag == tag {
                let start = offset + ENTRY_SIZE;
                return Some(start..start + entry_len);
            }
            offset += ENTRY_SIZE + entry_len;
        }
        None
    }

    fn read_u32(&self, offset: usize) -> u32 {
        self.read_u32_at(offset)
            .expect("header lies inside the allocation")
    }

    fn read_u32_at(&self, offset: usize) -> Option<u32> {
        let word: [u8; 4] = self.bytes.get(offset..offset + 4)?.try_into().ok()?;
        Some(u32::from_ne_bytes(word))
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
}

impl PartialEq for Descriptor {
    /// `SkDescriptor::operator==`: the bytes in use are equal (header included).
    // Port of: src/core/SkDescriptor.cpp#L78-L92 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        let end = self.length() as usize;
        match (self.bytes.get(..end), other.bytes.get(..end)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Descriptor {}

impl fmt::Debug for Descriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Descriptor")
            .field("checksum", &self.checksum())
            .field("length", &self.length())
            .field("count", &self.count())
            .finish()
    }
}

/// A descriptor that can be empty and can be reset to another size or copy, the owning form of
/// `SkAutoDescriptor`.
///
/// C++ keeps small descriptors in inline storage. Rust has no observable difference from that,
/// so the storage is always the heap allocation of [`Descriptor`].
#[doc(alias = "SkAutoDescriptor")]
#[derive(Clone, Default, Debug)]
pub struct AutoDescriptor {
    desc: Option<Descriptor>,
}

impl AutoDescriptor {
    /// `SkAutoDescriptor()`: holds no descriptor until it is reset.
    // Port of: src/core/SkDescriptor.cpp#L145 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `SkAutoDescriptor(size_t size)`: a fresh descriptor of `size` bytes of capacity.
    // Port of: src/core/SkDescriptor.cpp#L146 (chrome/m156)
    #[must_use]
    pub fn with_size(size: usize) -> Self {
        Self {
            desc: Some(Descriptor::alloc(size)),
        }
    }

    /// `SkAutoDescriptor(const SkDescriptor&)`: a copy of `desc`.
    // Port of: src/core/SkDescriptor.cpp#L147 (chrome/m156)
    #[must_use]
    pub fn from_descriptor(desc: &Descriptor) -> Self {
        Self {
            desc: Some(desc.copy()),
        }
    }

    /// `SkAutoDescriptor::reset(size_t size)`: replaces the descriptor with a fresh one.
    // Port of: src/core/SkDescriptor.cpp#L212-L219 (chrome/m156)
    pub fn reset(&mut self, size: usize) {
        self.desc = Some(Descriptor::alloc(size));
    }

    /// `SkAutoDescriptor::reset(const SkDescriptor&)`: replaces the descriptor with a copy.
    // Port of: src/core/SkDescriptor.cpp#L221-L225 (chrome/m156)
    pub fn reset_from(&mut self, desc: &Descriptor) {
        self.desc = Some(desc.copy());
    }

    /// `SkAutoDescriptor::getDesc`. Panics if no descriptor is held, as the C++ assert does.
    // Port of: src/core/SkDescriptor.h#L103 (chrome/m156)
    ///
    /// # Panics
    ///
    /// Panics if no descriptor is held.
    #[must_use]
    pub fn get_desc(&self) -> &Descriptor {
        self.desc
            .as_ref()
            .expect("SkAutoDescriptor holds no descriptor")
    }

    /// Mutable access to the held descriptor, for adding entries.
    ///
    /// # Panics
    ///
    /// Panics if no descriptor is held.
    pub fn get_desc_mut(&mut self) -> &mut Descriptor {
        self.desc
            .as_mut()
            .expect("SkAutoDescriptor holds no descriptor")
    }
}
