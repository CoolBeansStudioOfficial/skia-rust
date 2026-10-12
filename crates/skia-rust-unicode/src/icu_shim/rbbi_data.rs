// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `RBBIDataWrapper` (`rbbidata.h`, `rbbidata.cpp`): the sections of a compiled break-rule file
//! (`brkitr/*.brk`), read in place.

use super::ucptrie::UcpTrie;
use super::utypes::{U_INVALID_FORMAT_ERROR, UErrorCode};

// Port of: icu4c/source/common/rbbidata.h#L59-L61 (chromium/deps/icu d578f2e8)
/// `RBBI_DATA_FORMAT_VERSION[0]`.
const RBBI_DATA_FORMAT_VERSION_MAJOR: u8 = 6;

// Port of: icu4c/source/common/rbbidata.h#L157-L159 (chromium/deps/icu d578f2e8)
// (`RBBI_LOOKAHEAD_HARD_BREAK` = 1 is not tested at run time.)
/// `RBBI_BOF_REQUIRED`.
pub(crate) const RBBI_BOF_REQUIRED: u32 = 2;
/// `RBBI_8BITS_ROWS`.
pub(crate) const RBBI_8BITS_ROWS: u32 = 4;

fn u32_at(bytes: &[u8], i: usize) -> Result<u32, UErrorCode> {
    bytes
        .get(i..i + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(U_INVALID_FORMAT_ERROR)
}

/// `RBBIStateTable`: a forward or reverse state table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StateTable {
    /// `fTableData` onwards.
    rows: &'static [u8],
    /// `fRowLen`, in bytes.
    row_len: usize,
    /// `fDictCategoriesStart`.
    pub(crate) dict_categories_start: u32,
    /// `fLookAheadResultsSize`.
    pub(crate) look_ahead_results_size: u32,
    /// `fFlags`.
    pub(crate) flags: u32,
}

/// The fields of an `RBBIStateTableRow`, in order; `fNextState[category]` follows.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RowField {
    /// `fAccepting`.
    Accepting = 0,
    /// `fLookAhead`.
    LookAhead = 1,
    /// `fTagsIdx`.
    TagsIdx = 2,
}

impl StateTable {
    // Port of: icu4c/source/common/rbbidata.h#L141-L154 (chromium/deps/icu d578f2e8)
    fn parse(bytes: &'static [u8], offset: usize, len: usize) -> Result<Self, UErrorCode> {
        let table = bytes
            .get(offset..offset + len)
            .ok_or(U_INVALID_FORMAT_ERROR)?;
        let num_states = u32_at(table, 0)? as usize;
        let row_len = u32_at(table, 4)? as usize;
        let rows = table.get(20..).ok_or(U_INVALID_FORMAT_ERROR)?;
        if rows.len() < num_states * row_len {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        Ok(StateTable {
            rows,
            row_len,
            dict_categories_start: u32_at(table, 8)?,
            look_ahead_results_size: u32_at(table, 12)?,
            flags: u32_at(table, 16)?,
        })
    }

    /// Entry `k` of row `state` (`RBBIStateTableRow8` or `RBBIStateTableRow16`): a field, or
    /// with `k = 3 + category` the next state.
    fn entry(&self, state: u16, k: usize) -> u16 {
        let row = self.row_len * usize::from(state);
        if self.flags & RBBI_8BITS_ROWS != 0 {
            u16::from(self.rows[row + k])
        } else {
            u16::from_le_bytes([self.rows[row + 2 * k], self.rows[row + 2 * k + 1]])
        }
    }

    /// A field of row `state`.
    pub(crate) fn field(&self, state: u16, field: RowField) -> u16 {
        self.entry(state, field as usize)
    }

    /// `row->fNextState[category]`.
    pub(crate) fn next_state(&self, state: u16, category: u16) -> u16 {
        self.entry(state, 3 + usize::from(category))
    }
}

/// `RBBIDataWrapper`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RbbiData {
    /// `fForwardTable`.
    pub(crate) forward: StateTable,
    /// `fReverseTable`.
    pub(crate) reverse: StateTable,
    /// `fTrie`.
    pub(crate) trie: UcpTrie,
    /// `fRuleStatusTable` (little-endian `int32_t`s).
    status_table: &'static [u8],
    /// `fHeader->fCatCount`.
    pub(crate) cat_count: u32,
}

impl RbbiData {
    // Port of: icu4c/source/common/rbbidata.cpp#L44-L127 (chromium/deps/icu d578f2e8)
    /// `RBBIDataWrapper(UDataMemory*, status)` over a `.brk` file of ICU's data.
    ///
    /// # Errors
    /// `U_INVALID_FORMAT_ERROR` if the file is not RBBI data of format 6 in this byte order.
    pub(crate) fn open(file: &'static [u8]) -> Result<Self, UErrorCode> {
        // `DataHeader`: `MappedData { headerSize, magic1, magic2 }` and `UDataInfo`.
        let header_size = usize::from(u16::from_le_bytes([
            *file.first().ok_or(U_INVALID_FORMAT_ERROR)?,
            *file.get(1).ok_or(U_INVALID_FORMAT_ERROR)?,
        ]));
        let info = file.get(4..20).ok_or(U_INVALID_FORMAT_ERROR)?;
        // isBigEndian == U_IS_BIG_ENDIAN (the data is little-endian; the shim reads it as such
        // on every host), charsetFamily == ASCII, dataFormat == "Brk ".
        if header_size < 20
            || info[4] != 0
            || info[5] != 0
            || info[8..12] != *b"Brk "
            || info[12] != RBBI_DATA_FORMAT_VERSION_MAJOR
        {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let data = file.get(header_size..).ok_or(U_INVALID_FORMAT_ERROR)?;
        // `RBBIDataHeader`.
        if u32_at(data, 0)? != 0xb1a0 || data.get(4) != Some(&RBBI_DATA_FORMAT_VERSION_MAJOR) {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let field = |i: usize| u32_at(data, 12 + 4 * i).map(|v| v as usize);
        let cat_count = u32_at(data, 12)?;
        let (f_table, f_table_len) = (field(1)?, field(2)?);
        let (r_table, r_table_len) = (field(3)?, field(4)?);
        let (trie, trie_len) = (field(5)?, field(6)?);
        let (status, status_len) = (field(9)?, field(10)?);
        if f_table_len == 0 || r_table_len == 0 {
            return Err(U_INVALID_FORMAT_ERROR);
        }
        let trie = UcpTrie::open_from_binary(
            data.get(trie..trie + trie_len)
                .ok_or(U_INVALID_FORMAT_ERROR)?,
        )?;
        Ok(RbbiData {
            forward: StateTable::parse(data, f_table, f_table_len)?,
            reverse: StateTable::parse(data, r_table, r_table_len)?,
            trie,
            status_table: data
                .get(status..status + status_len)
                .ok_or(U_INVALID_FORMAT_ERROR)?,
            cat_count,
        })
    }

    /// `fRuleStatusTable[i]`.
    pub(crate) fn rule_status(&self, i: usize) -> i32 {
        let b = &self.status_table[4 * i..4 * i + 4];
        i32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }
}
