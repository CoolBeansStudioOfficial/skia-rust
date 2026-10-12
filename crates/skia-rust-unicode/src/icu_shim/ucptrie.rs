// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The read side of `UCPTrie` (`ucptrie.h`, `ucptrie.cpp`, `ucptrie_impl.h`) for the serialized
//! "fast" tries with 8- or 16-bit values that break-rule files hold (character categories).

use super::utypes::{U_INVALID_FORMAT_ERROR, UErrorCode};

// Port of: icu4c/source/common/ucptrie_impl.h#L17-L125 and unicode/ucptrie.h#L551-L578
// (chromium/deps/icu d578f2e8)
const UCPTRIE_SIG: u32 = 0x5472_6933;
const UCPTRIE_OPTIONS_DATA_LENGTH_MASK: u32 = 0xf000;
const UCPTRIE_OPTIONS_RESERVED_MASK: u32 = 0x38;
const UCPTRIE_OPTIONS_VALUE_BITS_MASK: u32 = 7;
const UCPTRIE_FAST_SHIFT: u32 = 6;
const UCPTRIE_FAST_DATA_MASK: u32 = (1 << UCPTRIE_FAST_SHIFT) - 1;
const UCPTRIE_ERROR_VALUE_NEG_DATA_OFFSET: u32 = 1;
const UCPTRIE_HIGH_VALUE_NEG_DATA_OFFSET: u32 = 2;
const UCPTRIE_BMP_INDEX_LENGTH: u32 = 0x10000 >> UCPTRIE_FAST_SHIFT;
const UCPTRIE_SHIFT_3: u32 = 4;
const UCPTRIE_SHIFT_2: u32 = 5 + UCPTRIE_SHIFT_3;
const UCPTRIE_SHIFT_1: u32 = 5 + UCPTRIE_SHIFT_2;
const UCPTRIE_SHIFT_2_3: u32 = UCPTRIE_SHIFT_2 - UCPTRIE_SHIFT_3;
const UCPTRIE_SHIFT_1_2: u32 = UCPTRIE_SHIFT_1 - UCPTRIE_SHIFT_2;
const UCPTRIE_OMITTED_BMP_INDEX_1_LENGTH: u32 = 0x10000 >> UCPTRIE_SHIFT_1;
const UCPTRIE_INDEX_2_MASK: u32 = (1 << UCPTRIE_SHIFT_1_2) - 1;
const UCPTRIE_INDEX_3_MASK: u32 = (1 << UCPTRIE_SHIFT_2_3) - 1;
const UCPTRIE_SMALL_DATA_MASK: u32 = (1 << UCPTRIE_SHIFT_3) - 1;
/// `UCPTRIE_TYPE_FAST`.
const UCPTRIE_TYPE_FAST: u32 = 0;
/// `UCPTRIE_VALUE_BITS_16`, `UCPTRIE_VALUE_BITS_8`.
const UCPTRIE_VALUE_BITS_16: u32 = 0;
const UCPTRIE_VALUE_BITS_8: u32 = 2;
/// `sizeof(UCPTrieHeader)`.
const HEADER_SIZE: usize = 16;

/// The value width of a trie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueWidth {
    /// `UCPTRIE_VALUE_BITS_8`.
    Bits8,
    /// `UCPTRIE_VALUE_BITS_16`.
    Bits16,
}

/// A serialized fast `UCPTrie` with 8- or 16-bit values, read in place.
#[derive(Clone, Copy, Debug)]
pub(crate) struct UcpTrie {
    index: &'static [u8],
    data: &'static [u8],
    data_length: u32,
    high_start: u32,
    width: ValueWidth,
}

fn u16_at(bytes: &[u8], i: usize) -> u32 {
    u32::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]))
}

impl UcpTrie {
    // Port of: icu4c/source/common/ucptrie.cpp#L22-L139 (chromium/deps/icu d578f2e8)
    /// `ucptrie_openFromBinary(UCPTRIE_TYPE_FAST, UCPTRIE_VALUE_BITS_ANY, data, length, ...)` for
    /// the 8- and 16-bit widths `RBBIDataWrapper::init` accepts.
    ///
    /// # Errors
    /// `U_INVALID_FORMAT_ERROR` for anything else.
    pub(crate) fn open_from_binary(bytes: &'static [u8]) -> Result<Self, UErrorCode> {
        if bytes.len() < HEADER_SIZE {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let signature = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if signature != UCPTRIE_SIG {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let options = u16_at(bytes, 4);
        let index_length = u16_at(bytes, 6);
        let type_int = (options >> 6) & 3;
        let value_width_int = options & UCPTRIE_OPTIONS_VALUE_BITS_MASK;
        if type_int != UCPTRIE_TYPE_FAST || (options & UCPTRIE_OPTIONS_RESERVED_MASK) != 0 {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let width = match value_width_int {
            UCPTRIE_VALUE_BITS_16 => ValueWidth::Bits16,
            UCPTRIE_VALUE_BITS_8 => ValueWidth::Bits8,
            _ => return Err(U_INVALID_FORMAT_ERROR),
        };
        let data_length = ((options & UCPTRIE_OPTIONS_DATA_LENGTH_MASK) << 4) | u16_at(bytes, 8);
        // (`dataNullOffset`, from `UCPTRIE_OPTIONS_DATA_NULL_OFFSET_MASK` and bytes 12..14, only
        // gives `nullValue`, which lookups do not use.)
        let high_start = u16_at(bytes, 14) << UCPTRIE_SHIFT_2;
        let index_end = HEADER_SIZE + 2 * index_length as usize;
        let data_end = index_end
            + match width {
                ValueWidth::Bits16 => 2 * data_length as usize,
                ValueWidth::Bits8 => data_length as usize,
            };
        if bytes.len() < data_end {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        Ok(UcpTrie {
            index: &bytes[HEADER_SIZE..index_end],
            data: &bytes[index_end..data_end],
            data_length,
            high_start,
            width,
        })
    }

    fn index(&self, i: u32) -> u32 {
        u16_at(self.index, 2 * i as usize)
    }

    // Port of: icu4c/source/common/ucptrie.cpp#L161-L184 (chromium/deps/icu d578f2e8)
    /// `ucptrie_internalSmallIndex` for a fast trie.
    fn internal_small_index(&self, c: u32) -> u32 {
        let i1 =
            (c >> UCPTRIE_SHIFT_1) + UCPTRIE_BMP_INDEX_LENGTH - UCPTRIE_OMITTED_BMP_INDEX_1_LENGTH;
        let mut i3_block =
            self.index(self.index(i1) + ((c >> UCPTRIE_SHIFT_2) & UCPTRIE_INDEX_2_MASK));
        let mut i3 = (c >> UCPTRIE_SHIFT_3) & UCPTRIE_INDEX_3_MASK;
        let data_block = if (i3_block & 0x8000) == 0 {
            // 16-bit indexes
            self.index(i3_block + i3)
        } else {
            // 18-bit indexes stored in groups of 9 entries per 8 indexes.
            i3_block = (i3_block & 0x7fff) + (i3 & !7) + (i3 >> 3);
            i3 &= 7;
            let high = (self.index(i3_block) << (2 + (2 * i3))) & 0x30000;
            high | self.index(i3_block + 1 + i3)
        };
        data_block + (c & UCPTRIE_SMALL_DATA_MASK)
    }

    // Port of: icu4c/source/common/unicode/ucptrie.h#L601-L620 (_UCPTRIE_CP_INDEX with
    // fastMax 0xffff) (chromium/deps/icu d578f2e8)
    fn cp_index(&self, c: u32) -> u32 {
        if c <= 0xffff {
            self.index(c >> UCPTRIE_FAST_SHIFT) + (c & UCPTRIE_FAST_DATA_MASK)
        } else if c <= 0x10_FFFF {
            if c >= self.high_start {
                self.data_length - UCPTRIE_HIGH_VALUE_NEG_DATA_OFFSET
            } else {
                self.internal_small_index(c)
            }
        } else {
            self.data_length - UCPTRIE_ERROR_VALUE_NEG_DATA_OFFSET
        }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L723-L729 (TrieFunc8, TrieFunc16: UCPTRIE_FAST_GET)
    // and ucptrie.cpp ucptrie_get (chromium/deps/icu d578f2e8)
    /// `UCPTRIE_FAST_GET(trie, UCPTRIE_8 / UCPTRIE_16, c)`, which is also `ucptrie_get` for a fast
    /// trie. `c` is a code point or `U_SENTINEL` (which reads the error value).
    pub(crate) fn get(&self, c: i32) -> u16 {
        // A negative `c` is above 0x10_FFFF as `uint32_t`, as in the macro.
        #[allow(clippy::cast_sign_loss)] // mirrors the macro's `(uint32_t)(c)`
        let i = self.cp_index(c as u32) as usize;
        match self.width {
            ValueWidth::Bits8 => u16::from(self.data[i]),
            ValueWidth::Bits16 => u16::from_le_bytes([self.data[2 * i], self.data[2 * i + 1]]),
        }
    }
}
