// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The dictionaries of ICU's break engines: `DictionaryMatcher` and its two kinds
//! (`dictionarydata.h`, `dictionarydata.cpp`), over the read side of `UCharsTrie`
//! (`ucharstrie.h`, `ucharstrie.cpp`) and `BytesTrie` (`bytestrie.h`, `bytestrie.cpp`).

use super::utext::UText;
use super::utypes::{U_INVALID_FORMAT_ERROR, UErrorCode};

/// `UStringTrieResult` (`ustringtrie.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UStringTrieResult {
    /// `USTRINGTRIE_NO_MATCH`.
    NoMatch = 0,
    /// `USTRINGTRIE_NO_VALUE`.
    NoValue = 1,
    /// `USTRINGTRIE_FINAL_VALUE`.
    FinalValue = 2,
    /// `USTRINGTRIE_INTERMEDIATE_VALUE`.
    IntermediateValue = 3,
}

impl UStringTrieResult {
    /// `USTRINGTRIE_HAS_VALUE`.
    fn has_value(self) -> bool {
        self as i32 >= UStringTrieResult::FinalValue as i32
    }
}

// --------------------------------------------------------------------------------------------
// UCharsTrie

// Port of: icu4c/source/common/unicode/ucharstrie.h#L561-L597 (chromium/deps/icu d578f2e8)
const UC_MAX_BRANCH_LINEAR_SUB_NODE_LENGTH: i32 = 5;
const UC_MIN_LINEAR_MATCH: i32 = 0x30;
const UC_MAX_LINEAR_MATCH_LENGTH: i32 = 0x10;
const UC_MIN_VALUE_LEAD: i32 = UC_MIN_LINEAR_MATCH + UC_MAX_LINEAR_MATCH_LENGTH;
const UC_NODE_TYPE_MASK: i32 = UC_MIN_VALUE_LEAD - 1;
const UC_VALUE_IS_FINAL: i32 = 0x8000;
const UC_MAX_ONE_UNIT_VALUE: i32 = 0x3fff;
const UC_MIN_TWO_UNIT_VALUE_LEAD: i32 = UC_MAX_ONE_UNIT_VALUE + 1;
const UC_THREE_UNIT_VALUE_LEAD: i32 = 0x7fff;
const UC_MAX_ONE_UNIT_NODE_VALUE: i32 = 0xff;
const UC_MIN_TWO_UNIT_NODE_VALUE_LEAD: i32 =
    UC_MIN_VALUE_LEAD + ((UC_MAX_ONE_UNIT_NODE_VALUE + 1) << 6);
const UC_THREE_UNIT_NODE_VALUE_LEAD: i32 = 0x7fc0;
const UC_MAX_ONE_UNIT_DELTA: i32 = 0xfbff;
const UC_MIN_TWO_UNIT_DELTA_LEAD: i32 = UC_MAX_ONE_UNIT_DELTA + 1;
const UC_THREE_UNIT_DELTA_LEAD: i32 = 0xffff;

/// `UCharsTrie` over little-endian UTF-16 units, read in place. Positions are unit indexes.
#[derive(Debug)]
struct UCharsTrie {
    units: &'static [u8],
    /// `pos_` (`None` is `nullptr`, after a mismatch).
    pos: Option<usize>,
    /// `remainingMatchLength_`.
    remaining_match_length: i32,
}

impl UCharsTrie {
    fn new(units: &'static [u8]) -> Self {
        UCharsTrie {
            units,
            pos: Some(0),
            remaining_match_length: -1,
        }
    }

    fn u(&self, i: usize) -> i32 {
        i32::from(u16::from_le_bytes([
            self.units[2 * i],
            self.units[2 * i + 1],
        ]))
    }

    // Port of: icu4c/source/common/unicode/ucharstrie.h#L414-L497 (chromium/deps/icu d578f2e8)
    fn stop(&mut self) {
        self.pos = None;
    }

    fn read_value(&self, pos: usize, lead_unit: i32) -> i32 {
        if lead_unit < UC_MIN_TWO_UNIT_VALUE_LEAD {
            lead_unit
        } else if lead_unit < UC_THREE_UNIT_VALUE_LEAD {
            ((lead_unit - UC_MIN_TWO_UNIT_VALUE_LEAD) << 16) | self.u(pos)
        } else {
            (self.u(pos) << 16) | self.u(pos + 1)
        }
    }

    fn skip_value_lead(pos: usize, lead_unit: i32) -> usize {
        if lead_unit >= UC_MIN_TWO_UNIT_VALUE_LEAD {
            if lead_unit < UC_THREE_UNIT_VALUE_LEAD {
                pos + 1
            } else {
                pos + 2
            }
        } else {
            pos
        }
    }

    fn skip_value(&self, pos: usize) -> usize {
        let lead_unit = self.u(pos);
        Self::skip_value_lead(pos + 1, lead_unit & 0x7fff)
    }

    fn read_node_value(&self, pos: usize, lead_unit: i32) -> i32 {
        if lead_unit < UC_MIN_TWO_UNIT_NODE_VALUE_LEAD {
            (lead_unit >> 6) - 1
        } else if lead_unit < UC_THREE_UNIT_NODE_VALUE_LEAD {
            (((lead_unit & 0x7fc0) - UC_MIN_TWO_UNIT_NODE_VALUE_LEAD) << 10) | self.u(pos)
        } else {
            (self.u(pos) << 16) | self.u(pos + 1)
        }
    }

    fn skip_node_value(pos: usize, lead_unit: i32) -> usize {
        if lead_unit >= UC_MIN_TWO_UNIT_NODE_VALUE_LEAD {
            if lead_unit < UC_THREE_UNIT_NODE_VALUE_LEAD {
                pos + 1
            } else {
                pos + 2
            }
        } else {
            pos
        }
    }

    fn jump_by_delta(&self, mut pos: usize) -> usize {
        let mut delta = self.u(pos);
        pos += 1;
        if delta >= UC_MIN_TWO_UNIT_DELTA_LEAD {
            if delta == UC_THREE_UNIT_DELTA_LEAD {
                delta = (self.u(pos) << 16) | self.u(pos + 1);
                pos += 2;
            } else {
                delta = ((delta - UC_MIN_TWO_UNIT_DELTA_LEAD) << 16) | self.u(pos);
                pos += 1;
            }
        }
        offset(pos, delta)
    }

    fn skip_delta(&self, mut pos: usize) -> usize {
        let delta = self.u(pos);
        pos += 1;
        if delta >= UC_MIN_TWO_UNIT_DELTA_LEAD {
            if delta == UC_THREE_UNIT_DELTA_LEAD {
                pos += 2;
            } else {
                pos += 1;
            }
        }
        pos
    }

    fn value_result(node: i32) -> UStringTrieResult {
        // USTRINGTRIE_INTERMEDIATE_VALUE - (node >> 15)
        if node >> 15 == 0 {
            UStringTrieResult::IntermediateValue
        } else {
            UStringTrieResult::FinalValue
        }
    }

    // Port of: icu4c/source/common/ucharstrie.cpp#L61-L131 (chromium/deps/icu d578f2e8)
    fn branch_next(&mut self, mut pos: usize, mut length: i32, uchar: i32) -> UStringTrieResult {
        // Branch according to the current unit.
        if length == 0 {
            length = self.u(pos);
            pos += 1;
        }
        length += 1;
        // The length of the branch is the number of units to select from.
        // The data structure encodes a binary search.
        while length > UC_MAX_BRANCH_LINEAR_SUB_NODE_LENGTH {
            let unit = self.u(pos);
            pos += 1;
            if uchar < unit {
                length >>= 1;
                pos = self.jump_by_delta(pos);
            } else {
                length -= length >> 1;
                pos = self.skip_delta(pos);
            }
        }
        // Drop down to linear search for the last few units.
        loop {
            let unit = self.u(pos);
            pos += 1;
            if uchar == unit {
                let mut node = self.u(pos);
                let result = if node & UC_VALUE_IS_FINAL != 0 {
                    // Leave the final value for getValue() to read.
                    UStringTrieResult::FinalValue
                } else {
                    // Use the non-final value as the jump delta.
                    pos += 1;
                    let delta;
                    if node < UC_MIN_TWO_UNIT_VALUE_LEAD {
                        delta = node;
                    } else if node < UC_THREE_UNIT_VALUE_LEAD {
                        delta = ((node - UC_MIN_TWO_UNIT_VALUE_LEAD) << 16) | self.u(pos);
                        pos += 1;
                    } else {
                        delta = (self.u(pos) << 16) | self.u(pos + 1);
                        pos += 2;
                    }
                    pos = offset(pos, delta);
                    node = self.u(pos);
                    if node >= UC_MIN_VALUE_LEAD {
                        Self::value_result(node)
                    } else {
                        UStringTrieResult::NoValue
                    }
                };
                self.pos = Some(pos);
                return result;
            }
            length -= 1;
            pos = self.skip_value(pos);
            if length <= 1 {
                break;
            }
        }
        let unit = self.u(pos);
        pos += 1;
        if uchar == unit {
            self.pos = Some(pos);
            let node = self.u(pos);
            if node >= UC_MIN_VALUE_LEAD {
                Self::value_result(node)
            } else {
                UStringTrieResult::NoValue
            }
        } else {
            self.stop();
            UStringTrieResult::NoMatch
        }
    }

    // Port of: icu4c/source/common/ucharstrie.cpp#L133-L162 (chromium/deps/icu d578f2e8)
    fn next_impl(&mut self, mut pos: usize, uchar: i32) -> UStringTrieResult {
        let mut node = self.u(pos);
        pos += 1;
        loop {
            if node < UC_MIN_LINEAR_MATCH {
                return self.branch_next(pos, node, uchar);
            } else if node < UC_MIN_VALUE_LEAD {
                // Match the first of length+1 units.
                let length = node - UC_MIN_LINEAR_MATCH; // Actual match length minus 1.
                let unit = self.u(pos);
                pos += 1;
                if uchar == unit {
                    let length = length - 1;
                    self.remaining_match_length = length;
                    self.pos = Some(pos);
                    let node = self.u(pos);
                    return if length < 0 && node >= UC_MIN_VALUE_LEAD {
                        Self::value_result(node)
                    } else {
                        UStringTrieResult::NoValue
                    };
                }
                // No match.
                break;
            } else if node & UC_VALUE_IS_FINAL != 0 {
                // No further matching units.
                break;
            }
            // Skip intermediate value.
            pos = Self::skip_node_value(pos, node);
            node &= UC_NODE_TYPE_MASK;
        }
        self.stop();
        UStringTrieResult::NoMatch
    }

    // Port of: icu4c/source/common/unicode/ucharstrie.h#L200-L203 (chromium/deps/icu d578f2e8)
    /// `first(uchar)`: values below 0 and above 0xffff never match.
    fn first(&mut self, uchar: i32) -> UStringTrieResult {
        self.remaining_match_length = -1;
        self.next_impl(0, uchar)
    }

    // Port of: icu4c/source/common/ucharstrie.cpp#L164-L185 (chromium/deps/icu d578f2e8)
    fn next(&mut self, uchar: i32) -> UStringTrieResult {
        let Some(mut pos) = self.pos else {
            return UStringTrieResult::NoMatch;
        };
        let length = self.remaining_match_length; // Actual remaining match length minus 1.
        if length >= 0 {
            // Remaining part of a linear-match node.
            let unit = self.u(pos);
            pos += 1;
            if uchar == unit {
                let length = length - 1;
                self.remaining_match_length = length;
                self.pos = Some(pos);
                let node = self.u(pos);
                return if length < 0 && node >= UC_MIN_VALUE_LEAD {
                    Self::value_result(node)
                } else {
                    UStringTrieResult::NoValue
                };
            }
            self.stop();
            return UStringTrieResult::NoMatch;
        }
        self.next_impl(pos, uchar)
    }

    // Port of: icu4c/source/common/unicode/ucharstrie.h#L258-L264 (chromium/deps/icu d578f2e8)
    fn get_value(&self) -> i32 {
        let pos = self.pos.expect("getValue after a value result");
        let lead_unit = self.u(pos);
        if lead_unit & UC_VALUE_IS_FINAL != 0 {
            self.read_value(pos + 1, lead_unit & 0x7fff)
        } else {
            self.read_node_value(pos + 1, lead_unit)
        }
    }
}

/// `pos + delta` for a trie delta (non-negative in well-formed data).
fn offset(pos: usize, delta: i32) -> usize {
    pos + usize::try_from(delta).expect("trie deltas are non-negative")
}

// --------------------------------------------------------------------------------------------
// BytesTrie

// Port of: icu4c/source/common/unicode/bytestrie.h#L504-L539 (chromium/deps/icu d578f2e8)
const B_MAX_BRANCH_LINEAR_SUB_NODE_LENGTH: i32 = 5;
const B_MIN_LINEAR_MATCH: i32 = 0x10;
const B_MAX_LINEAR_MATCH_LENGTH: i32 = 0x10;
const B_MIN_VALUE_LEAD: i32 = B_MIN_LINEAR_MATCH + B_MAX_LINEAR_MATCH_LENGTH;
const B_VALUE_IS_FINAL: i32 = 1;
const B_MIN_ONE_BYTE_VALUE_LEAD: i32 = B_MIN_VALUE_LEAD / 2;
const B_MAX_ONE_BYTE_VALUE: i32 = 0x40;
const B_MIN_TWO_BYTE_VALUE_LEAD: i32 = B_MIN_ONE_BYTE_VALUE_LEAD + B_MAX_ONE_BYTE_VALUE + 1;
const B_MAX_TWO_BYTE_VALUE: i32 = 0x1aff;
const B_MIN_THREE_BYTE_VALUE_LEAD: i32 =
    B_MIN_TWO_BYTE_VALUE_LEAD + (B_MAX_TWO_BYTE_VALUE >> 8) + 1;
const B_FOUR_BYTE_VALUE_LEAD: i32 = 0x7e;
const B_MAX_ONE_BYTE_DELTA: i32 = 0xbf;
const B_MIN_TWO_BYTE_DELTA_LEAD: i32 = B_MAX_ONE_BYTE_DELTA + 1;
const B_MIN_THREE_BYTE_DELTA_LEAD: i32 = 0xf0;
const B_FOUR_BYTE_DELTA_LEAD: i32 = 0xfe;

/// `BytesTrie`, read in place. Positions are byte indexes.
#[derive(Debug)]
struct BytesTrie {
    bytes: &'static [u8],
    /// `pos_`.
    pos: Option<usize>,
    /// `remainingMatchLength_`.
    remaining_match_length: i32,
}

impl BytesTrie {
    fn new(bytes: &'static [u8]) -> Self {
        BytesTrie {
            bytes,
            pos: Some(0),
            remaining_match_length: -1,
        }
    }

    fn b(&self, i: usize) -> i32 {
        i32::from(self.bytes[i])
    }

    fn stop(&mut self) {
        self.pos = None;
    }

    // Port of: icu4c/source/common/bytestrie.cpp#L29-L45 (chromium/deps/icu d578f2e8)
    /// `readValue`: `lead_byte` is already shifted right by 1.
    fn read_value(&self, pos: usize, lead_byte: i32) -> i32 {
        if lead_byte < B_MIN_TWO_BYTE_VALUE_LEAD {
            lead_byte - B_MIN_ONE_BYTE_VALUE_LEAD
        } else if lead_byte < B_MIN_THREE_BYTE_VALUE_LEAD {
            ((lead_byte - B_MIN_TWO_BYTE_VALUE_LEAD) << 8) | self.b(pos)
        } else if lead_byte < B_FOUR_BYTE_VALUE_LEAD {
            ((lead_byte - B_MIN_THREE_BYTE_VALUE_LEAD) << 16) | (self.b(pos) << 8) | self.b(pos + 1)
        } else if lead_byte == B_FOUR_BYTE_VALUE_LEAD {
            (self.b(pos) << 16) | (self.b(pos + 1) << 8) | self.b(pos + 2)
        } else {
            (self.b(pos) << 24) | (self.b(pos + 1) << 16) | (self.b(pos + 2) << 8) | self.b(pos + 3)
        }
    }

    // Port of: icu4c/source/common/bytestrie.cpp#L47-L66 (chromium/deps/icu d578f2e8)
    fn jump_by_delta(&self, mut pos: usize) -> usize {
        let mut delta = self.b(pos);
        pos += 1;
        if delta < B_MIN_TWO_BYTE_DELTA_LEAD {
            // nothing to do
        } else if delta < B_MIN_THREE_BYTE_DELTA_LEAD {
            delta = ((delta - B_MIN_TWO_BYTE_DELTA_LEAD) << 8) | self.b(pos);
            pos += 1;
        } else if delta < B_FOUR_BYTE_DELTA_LEAD {
            delta = ((delta - B_MIN_THREE_BYTE_DELTA_LEAD) << 16)
                | (self.b(pos) << 8)
                | self.b(pos + 1);
            pos += 2;
        } else if delta == B_FOUR_BYTE_DELTA_LEAD {
            delta = (self.b(pos) << 16) | (self.b(pos + 1) << 8) | self.b(pos + 2);
            pos += 3;
        } else {
            delta = (self.b(pos) << 24)
                | (self.b(pos + 1) << 16)
                | (self.b(pos + 2) << 8)
                | self.b(pos + 3);
            pos += 4;
        }
        offset(pos, delta)
    }

    // Port of: icu4c/source/common/unicode/bytestrie.h#L406-L443 (chromium/deps/icu d578f2e8)
    fn skip_value_lead(pos: usize, lead_byte: i32) -> usize {
        if lead_byte >= (B_MIN_TWO_BYTE_VALUE_LEAD << 1) {
            if lead_byte < (B_MIN_THREE_BYTE_VALUE_LEAD << 1) {
                pos + 1
            } else if lead_byte < (B_FOUR_BYTE_VALUE_LEAD << 1) {
                pos + 2
            } else {
                pos + 3 + usize::from((lead_byte >> 1) & 1 != 0)
            }
        } else {
            pos
        }
    }

    fn skip_value(&self, pos: usize) -> usize {
        let lead_byte = self.b(pos);
        Self::skip_value_lead(pos + 1, lead_byte)
    }

    fn skip_delta(&self, mut pos: usize) -> usize {
        let delta = self.b(pos);
        pos += 1;
        if delta >= B_MIN_TWO_BYTE_DELTA_LEAD {
            if delta < B_MIN_THREE_BYTE_DELTA_LEAD {
                pos += 1;
            } else if delta < B_FOUR_BYTE_DELTA_LEAD {
                pos += 2;
            } else {
                pos += 3 + usize::from(delta & 1 != 0);
            }
        }
        pos
    }

    fn value_result(node: i32) -> UStringTrieResult {
        // USTRINGTRIE_INTERMEDIATE_VALUE - (node & kValueIsFinal)
        if node & B_VALUE_IS_FINAL == 0 {
            UStringTrieResult::IntermediateValue
        } else {
            UStringTrieResult::FinalValue
        }
    }

    // Port of: icu4c/source/common/bytestrie.cpp#L79-L149 (chromium/deps/icu d578f2e8)
    fn branch_next(&mut self, mut pos: usize, mut length: i32, in_byte: i32) -> UStringTrieResult {
        // Branch according to the current byte.
        if length == 0 {
            length = self.b(pos);
            pos += 1;
        }
        length += 1;
        // The length of the branch is the number of bytes to select from.
        // The data structure encodes a binary search.
        while length > B_MAX_BRANCH_LINEAR_SUB_NODE_LENGTH {
            let byte = self.b(pos);
            pos += 1;
            if in_byte < byte {
                length >>= 1;
                pos = self.jump_by_delta(pos);
            } else {
                length -= length >> 1;
                pos = self.skip_delta(pos);
            }
        }
        // Drop down to linear search for the last few bytes.
        loop {
            let byte = self.b(pos);
            pos += 1;
            if in_byte == byte {
                let mut node = self.b(pos);
                let result = if node & B_VALUE_IS_FINAL != 0 {
                    // Leave the final value for getValue() to read.
                    UStringTrieResult::FinalValue
                } else {
                    // Use the non-final value as the jump delta.
                    pos += 1;
                    node >>= 1;
                    let delta;
                    if node < B_MIN_TWO_BYTE_VALUE_LEAD {
                        delta = node - B_MIN_ONE_BYTE_VALUE_LEAD;
                    } else if node < B_MIN_THREE_BYTE_VALUE_LEAD {
                        delta = ((node - B_MIN_TWO_BYTE_VALUE_LEAD) << 8) | self.b(pos);
                        pos += 1;
                    } else if node < B_FOUR_BYTE_VALUE_LEAD {
                        delta = ((node - B_MIN_THREE_BYTE_VALUE_LEAD) << 16)
                            | (self.b(pos) << 8)
                            | self.b(pos + 1);
                        pos += 2;
                    } else if node == B_FOUR_BYTE_VALUE_LEAD {
                        delta = (self.b(pos) << 16) | (self.b(pos + 1) << 8) | self.b(pos + 2);
                        pos += 3;
                    } else {
                        delta = (self.b(pos) << 24)
                            | (self.b(pos + 1) << 16)
                            | (self.b(pos + 2) << 8)
                            | self.b(pos + 3);
                        pos += 4;
                    }
                    pos = offset(pos, delta);
                    node = self.b(pos);
                    if node >= B_MIN_VALUE_LEAD {
                        Self::value_result(node)
                    } else {
                        UStringTrieResult::NoValue
                    }
                };
                self.pos = Some(pos);
                return result;
            }
            length -= 1;
            pos = self.skip_value(pos);
            if length <= 1 {
                break;
            }
        }
        let byte = self.b(pos);
        pos += 1;
        if in_byte == byte {
            self.pos = Some(pos);
            let node = self.b(pos);
            if node >= B_MIN_VALUE_LEAD {
                Self::value_result(node)
            } else {
                UStringTrieResult::NoValue
            }
        } else {
            self.stop();
            UStringTrieResult::NoMatch
        }
    }

    // Port of: icu4c/source/common/bytestrie.cpp#L151-L180 (chromium/deps/icu d578f2e8)
    fn next_impl(&mut self, mut pos: usize, in_byte: i32) -> UStringTrieResult {
        loop {
            let node = self.b(pos);
            pos += 1;
            if node < B_MIN_LINEAR_MATCH {
                return self.branch_next(pos, node, in_byte);
            } else if node < B_MIN_VALUE_LEAD {
                // Match the first of length+1 bytes.
                let length = node - B_MIN_LINEAR_MATCH; // Actual match length minus 1.
                let byte = self.b(pos);
                pos += 1;
                if in_byte == byte {
                    let length = length - 1;
                    self.remaining_match_length = length;
                    self.pos = Some(pos);
                    let node = self.b(pos);
                    return if length < 0 && node >= B_MIN_VALUE_LEAD {
                        Self::value_result(node)
                    } else {
                        UStringTrieResult::NoValue
                    };
                }
                // No match.
                break;
            } else if node & B_VALUE_IS_FINAL != 0 {
                // No further matching bytes.
                break;
            }
            // Skip intermediate value.
            pos = Self::skip_value_lead(pos, node);
        }
        self.stop();
        UStringTrieResult::NoMatch
    }

    // Port of: icu4c/source/common/unicode/bytestrie.h#L203-L209 (chromium/deps/icu d578f2e8)
    fn first(&mut self, mut in_byte: i32) -> UStringTrieResult {
        self.remaining_match_length = -1;
        if in_byte < 0 {
            in_byte += 0x100;
        }
        self.next_impl(0, in_byte)
    }

    // Port of: icu4c/source/common/bytestrie.cpp#L182-L205 (chromium/deps/icu d578f2e8)
    fn next(&mut self, mut in_byte: i32) -> UStringTrieResult {
        let Some(mut pos) = self.pos else {
            return UStringTrieResult::NoMatch;
        };
        if in_byte < 0 {
            in_byte += 0x100;
        }
        let length = self.remaining_match_length; // Actual remaining match length minus 1.
        if length >= 0 {
            // Remaining part of a linear-match node.
            let byte = self.b(pos);
            pos += 1;
            if in_byte == byte {
                let length = length - 1;
                self.remaining_match_length = length;
                self.pos = Some(pos);
                let node = self.b(pos);
                return if length < 0 && node >= B_MIN_VALUE_LEAD {
                    Self::value_result(node)
                } else {
                    UStringTrieResult::NoValue
                };
            }
            self.stop();
            return UStringTrieResult::NoMatch;
        }
        self.next_impl(pos, in_byte)
    }

    // Port of: icu4c/source/common/unicode/bytestrie.h#L246-L251 (chromium/deps/icu d578f2e8)
    fn get_value(&self) -> i32 {
        let pos = self.pos.expect("getValue after a value result");
        let lead_byte = self.b(pos);
        self.read_value(pos + 1, lead_byte >> 1)
    }
}

// --------------------------------------------------------------------------------------------
// DictionaryMatcher

// Port of: icu4c/source/common/dictionarydata.cpp#L24-L31 and dictionarydata.h#L44-L59
// (chromium/deps/icu d578f2e8)
const TRIE_TYPE_BYTES: i32 = 0;
const TRIE_TYPE_UCHARS: i32 = 1;
const TRIE_TYPE_MASK: i32 = 7;
const TRANSFORM_TYPE_OFFSET: i32 = 0x0100_0000;
const TRANSFORM_TYPE_MASK: i32 = 0x7f00_0000;
const TRANSFORM_OFFSET_MASK: i32 = 0x001f_ffff;
const IX_STRING_TRIE_OFFSET: usize = 0;
const IX_TRIE_TYPE: usize = 4;
const IX_TRANSFORM: usize = 5;

/// The output arrays of `DictionaryMatcher::matches` (`nullptr` for an absent one).
#[derive(Debug, Default)]
pub(crate) struct MatchOutput<'a> {
    /// `lengths`: match lengths in native units.
    pub(crate) lengths: Option<&'a mut [i32]>,
    /// `cpLengths`: match lengths in code points.
    pub(crate) cp_lengths: Option<&'a mut [i32]>,
    /// `values`.
    pub(crate) values: Option<&'a mut [i32]>,
    /// `prefix`: the number of code points matched.
    pub(crate) prefix: Option<&'a mut i32>,
}

/// `DictionaryMatcher`: `UCharsDictionaryMatcher` or `BytesDictionaryMatcher`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum DictionaryMatcher {
    /// `UCharsDictionaryMatcher`.
    UChars {
        /// `characters`.
        trie: &'static [u8],
    },
    /// `BytesDictionaryMatcher`.
    Bytes {
        /// `characters`.
        trie: &'static [u8],
        /// `transformConstant`.
        transform: i32,
    },
}

impl DictionaryMatcher {
    // Port of: icu4c/source/common/brkeng.cpp#L268-L297 (ICULanguageBreakFactory::
    // loadDictionaryMatcherFor, after udata_open) (chromium/deps/icu d578f2e8)
    /// The matcher of a `.dict` file of ICU's data.
    ///
    /// # Errors
    /// `U_INVALID_FORMAT_ERROR` if the file is not dictionary data (format "Dict" 1) or names no
    /// known trie type.
    pub(crate) fn open(file: &'static [u8]) -> Result<Self, UErrorCode> {
        let header_size = usize::from(u16::from_le_bytes([
            *file.first().ok_or(U_INVALID_FORMAT_ERROR)?,
            *file.get(1).ok_or(U_INVALID_FORMAT_ERROR)?,
        ]));
        let info = file.get(4..20).ok_or(U_INVALID_FORMAT_ERROR)?;
        if info[4] != 0 || info[8..12] != *b"Dict" || info[12] != 1 {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let data = file.get(header_size..).ok_or(U_INVALID_FORMAT_ERROR)?;
        let index = |i: usize| -> Result<i32, UErrorCode> {
            data.get(4 * i..4 * i + 4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(U_INVALID_FORMAT_ERROR)
        };
        let offset =
            usize::try_from(index(IX_STRING_TRIE_OFFSET)?).map_err(|_| U_INVALID_FORMAT_ERROR)?;
        let trie_type = index(IX_TRIE_TYPE)? & TRIE_TYPE_MASK;
        let trie = data.get(offset..).ok_or(U_INVALID_FORMAT_ERROR)?;
        match trie_type {
            TRIE_TYPE_BYTES => Ok(DictionaryMatcher::Bytes {
                trie,
                transform: index(IX_TRANSFORM)?,
            }),
            TRIE_TYPE_UCHARS => Ok(DictionaryMatcher::UChars { trie }),
            _ => Err(U_INVALID_FORMAT_ERROR),
        }
    }

    // Port of: icu4c/source/common/dictionarydata.cpp#L101-L116 (chromium/deps/icu d578f2e8)
    /// `BytesDictionaryMatcher::transform`.
    fn transform(transform_constant: i32, c: i32) -> i32 {
        if (transform_constant & TRANSFORM_TYPE_MASK) == TRANSFORM_TYPE_OFFSET {
            if c == 0x200D {
                return 0xFF;
            } else if c == 0x200C {
                return 0xFE;
            }
            let delta = c - (transform_constant & TRANSFORM_OFFSET_MASK);
            if !(0..=0xFD).contains(&delta) {
                return -1; // U_SENTINEL
            }
            return delta;
        }
        c
    }

    // Port of: icu4c/source/common/dictionarydata.cpp#L46-L91 and #L122-L167
    // (chromium/deps/icu d578f2e8)
    /// `matches(text, maxLength, limit, lengths, cpLengths, values, prefix)`: finds the words of
    /// the dictionary that start at the text's position (up to `limit` of them, of at most
    /// `max_length` native units), leaving the text after the longest prefix that matched.
    pub(crate) fn matches(
        &self,
        text: &mut UText,
        max_length: i32,
        limit: i32,
        mut out: MatchOutput<'_>,
    ) -> i32 {
        enum Trie {
            U(UCharsTrie),
            B(BytesTrie, i32),
        }
        let mut trie = match *self {
            DictionaryMatcher::UChars { trie } => Trie::U(UCharsTrie::new(trie)),
            DictionaryMatcher::Bytes { trie, transform } => {
                Trie::B(BytesTrie::new(trie), transform)
            }
        };
        let starting_text_index = text.get_native_index();
        let mut word_count = 0;
        let mut code_points_matched = 0;

        let mut c = text.next32();
        while c >= 0 {
            let result = match &mut trie {
                Trie::U(t) => {
                    if code_points_matched == 0 {
                        t.first(c)
                    } else {
                        t.next(c)
                    }
                }
                Trie::B(t, transform) => {
                    let byte = Self::transform(*transform, c);
                    if code_points_matched == 0 {
                        t.first(byte)
                    } else {
                        t.next(byte)
                    }
                }
            };
            let length_matched = text.get_native_index() - starting_text_index;
            code_points_matched += 1;
            if result.has_value() {
                if word_count < limit {
                    let k = usize::try_from(word_count).expect("non-negative");
                    if let Some(values) = out.values.as_deref_mut() {
                        values[k] = match &trie {
                            Trie::U(t) => t.get_value(),
                            Trie::B(t, _) => t.get_value(),
                        };
                    }
                    if let Some(lengths) = out.lengths.as_deref_mut() {
                        lengths[k] = length_matched;
                    }
                    if let Some(cp_lengths) = out.cp_lengths.as_deref_mut() {
                        cp_lengths[k] = code_points_matched;
                    }
                    word_count += 1;
                }
                if result == UStringTrieResult::FinalValue {
                    break;
                }
            } else if result == UStringTrieResult::NoMatch {
                break;
            }
            if length_matched >= max_length {
                break;
            }
            c = text.next32();
        }

        if let Some(prefix) = out.prefix {
            *prefix = code_points_matched;
        }
        word_count
    }
}
