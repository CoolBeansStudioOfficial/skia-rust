// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skunicode/src/SkUnicode_icu_bidi.cpp,
// modules/skunicode/src/SkBidiFactory_icu_full.cpp, and ICU's common/ubidiln.cpp
// (`ubidi_reorderVisual`) of Chromium's ICU (chromium/deps/icu d578f2e8).

//! The bidi engine of the ICU path (milestone M3, `docs/design/modules.md` §3.2).
//!
//! Skia reaches ICU's `ubidi_*` through `SkBidiFactory`. The embedding levels come from
//! `unicode-bidi` 0.3.18 over ICU4X's `BidiClass` data (`icu_properties`, feature
//! `unicode_bidi`). `docs/design/modules.md` §3.3 measured these levels against Chromium's ICU
//! and found no difference, and the probe behind this port confirmed the level semantics of
//! `ubidi_getLevelAt`: resolved levels with Rule L1 applied, the paragraph level for
//! trailing whitespace and paragraph separators.
//!
//! Known gaps, from the M0c results (`docs/design/modules.md` §3.4 on branch
//! `port/text-m0c-unicode-diff`). They concern texts with explicit embedding or isolate controls
//! (U+202A-U+202E, U+2066-U+2069), and are not reproduced yet:
//! - `bidi-uniform-direction-shortcut`: ICU does not resolve a text whose direction is uniform
//!   (`ubidi.cpp` `directionFromFlags`), so the embeddings and isolates in it have no effect.
//!   This port resolves them.
//! - `bidi-x9-removed-levels`: ICU gives an embedding opener the level it opens, and a closer
//!   the outer level. This port uses the levels of `unicode-bidi`.
//! - `bidi-direction`: the paragraph level is part of the flags (done here), but the direction
//!   of texts with embeddings is taken from the first direction pass only (see [`extract_bidi`]),
//!   not the second pass of `ubidi.cpp`.
//! - `bidi-default-level-number-after-separator` concerns `UBIDI_DEFAULT_LTR/RTL` only. Skia
//!   passes explicit levels, so it does not arise here.
//!
//! Texts without those controls follow the same flag and level rules as ICU, as the probes of
//! `docs/design/modules.md` §3.3 and the M0c results show.

use icu_properties::CodePointMapData;
use icu_properties::props::BidiClass as IcuBidiClass;
use unicode_bidi::{BidiClass, BidiInfo, Level};

use skia_rust_core::utf;

use crate::unicode::{
    BidiDirection, BidiIterator, BidiLevel, BidiPosition, BidiRegion, Position, TextDirection,
    convert_utf8_to_utf16, for_each_codepoint_utf8,
};

// `UBIDI_MAX_EXPLICIT_LEVEL` of ICU's common/unicode/ubidi.h (chromium/deps/icu d578f2e8).
const UBIDI_MAX_EXPLICIT_LEVEL: u8 = 125;

// Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L36-L37 (chrome/m156): the paragraph
// level passed to `ubidi_setPara`, from the direction.
fn para_level(dir: BidiDirection) -> Level {
    match dir {
        BidiDirection::Ltr => Level::ltr(),
        BidiDirection::Rtl => Level::rtl(),
    }
}

// Unpaired surrogates are one UTF-16 unit each and have bidi class L in ICU. `unicode-bidi`
// works on Rust strings, which cannot hold them, so each is replaced by U+E000 (PRIVATE USE,
// bidi class L, not a bracket): the resolved levels of the text are the same.
const LONE_SURROGATE_REPLACEMENT: char = '\u{e000}';

// Port of: ICU common/ubidi.cpp `directionFromFlags` (chromium/deps/icu d578f2e8) with the
// masks of common/ubidiimp.h#L85-L105. `paragraph_level` is the level of the paragraphs, whose
// L or R flag `getDirProps` adds to the flags of the text (ubidi.cpp#L629-L633).
fn direction_from_classes(
    paragraph_level: Level,
    classes: impl Iterator<Item = BidiClass>,
) -> Option<bool> {
    // Returns `Some(true)` for LTR, `Some(false)` for RTL, `None` for mixed.
    let mut flags: u32 = if paragraph_level.is_rtl() {
        flag(BidiClass::R)
    } else {
        flag(BidiClass::L)
    };
    for class in classes {
        flags |= 1 << class_index(class);
    }
    let mask_ltr = flag(BidiClass::L)
        | flag(BidiClass::EN)
        | flag(BidiClass::AN)
        | flag(BidiClass::LRE)
        | flag(BidiClass::LRO)
        | flag(BidiClass::LRI);
    let mask_rtl = flag(BidiClass::R)
        | flag(BidiClass::AL)
        | flag(BidiClass::RLE)
        | flag(BidiClass::RLO)
        | flag(BidiClass::RLI);
    let mask_possible_n = flag(BidiClass::ON)
        | flag(BidiClass::CS)
        | flag(BidiClass::ES)
        | flag(BidiClass::ET)
        | flag(BidiClass::B)
        | flag(BidiClass::S)
        | flag(BidiClass::WS)
        | flag(BidiClass::BN)
        | flag(BidiClass::LRE)
        | flag(BidiClass::LRO)
        | flag(BidiClass::RLE)
        | flag(BidiClass::RLO)
        | flag(BidiClass::PDF)
        | flag(BidiClass::LRI)
        | flag(BidiClass::RLI)
        | flag(BidiClass::FSI)
        | flag(BidiClass::PDI);
    // ICU: `if(!(flags&MASK_RTL || ((flags&DIRPROP_FLAG(AN)) && (flags&MASK_POSSIBLE_N))))`
    let an_and_neutral = (flags & flag(BidiClass::AN)) != 0 && (flags & mask_possible_n) != 0;
    if (flags & mask_rtl) == 0 && !an_and_neutral {
        Some(true)
    } else if (flags & mask_ltr) == 0 {
        Some(false)
    } else {
        None
    }
}

const fn flag(class: BidiClass) -> u32 {
    1 << class_index(class)
}

// One bit per class, for the flag masks. The bit numbers differ from ICU's `DirProp` values
// (they only need to be distinct), and the masks themselves are ICU's.
const fn class_index(class: BidiClass) -> u32 {
    match class {
        BidiClass::L => 0,
        BidiClass::R => 1,
        BidiClass::EN => 2,
        BidiClass::ES => 3,
        BidiClass::ET => 4,
        BidiClass::AN => 5,
        BidiClass::CS => 6,
        BidiClass::B => 7,
        BidiClass::S => 8,
        BidiClass::WS => 9,
        BidiClass::ON => 10,
        BidiClass::BN => 11,
        BidiClass::NSM => 12,
        BidiClass::AL => 13,
        BidiClass::LRO => 14,
        BidiClass::RLO => 15,
        BidiClass::LRE => 16,
        BidiClass::RLE => 17,
        BidiClass::PDF => 18,
        BidiClass::LRI => 19,
        BidiClass::RLI => 20,
        BidiClass::FSI => 21,
        BidiClass::PDI => 22,
    }
}

/// The result of `ubidi_setPara` on a UTF-16 paragraph, as Skia reads it: the level of each
/// UTF-16 unit (`ubidi_getLevelAt`) and whether the text is mixed (`ubidi_getDirection`).
struct Analysis {
    levels: Vec<BidiLevel>,
    mixed: bool,
}

// Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L31-L49 (chrome/m156) and
// modules/skunicode/src/SkUnicode_icu_bidi.cpp#L101-L124 (chrome/m156), the `ubidi_openSized`
// and `ubidi_setPara` steps they perform, and the level and direction queries they make.
fn analyze_utf16(utf16: &[u16], dir: BidiDirection) -> Analysis {
    let text: String = char::decode_utf16(utf16.iter().copied())
        .map(|unit| unit.unwrap_or(LONE_SURROGATE_REPLACEMENT))
        .collect();
    let data = CodePointMapData::<IcuBidiClass>::new();
    let info = BidiInfo::new_with_data_source(&data, &text, Some(para_level(dir)));

    // The resolved levels of each paragraph, with Rule L1 applied, one per byte of the text.
    let mut byte_levels: Vec<Level> = Vec::with_capacity(text.len());
    for para in &info.paragraphs {
        let levels = info.reordered_levels(para, para.range.clone());
        byte_levels.extend_from_slice(&levels[para.range.clone()]);
    }

    // One level per UTF-16 unit: a character's level applies to each of its units.
    let mut levels: Vec<BidiLevel> = Vec::with_capacity(utf16.len());
    for (byte, c) in text.char_indices() {
        let level = byte_levels[byte].number();
        for _ in 0..c.len_utf16() {
            levels.push(level);
        }
    }

    let classes = text.chars().map(|c| data.get(c).into());
    let mixed = direction_from_classes(para_level(dir), classes).is_none();
    Analysis { levels, mixed }
}

// Port of: ICU common/ubidi.cpp `ubidi_getLevelAt` (chromium/deps/icu d578f2e8): out of range
// positions have level 0.
fn level_at(levels: &[BidiLevel], pos: BidiPosition) -> BidiLevel {
    match usize::try_from(pos) {
        Ok(index) if index < levels.len() => levels[index],
        _ => 0,
    }
}

/// `SkBidiIterator_icu`: the embedding levels of a UTF-16 text, one per code unit.
#[derive(Debug)]
pub struct IcuBidiIterator {
    levels: Vec<BidiLevel>,
}

impl BidiIterator for IcuBidiIterator {
    // Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L23-L24 (chrome/m156)
    fn get_length(&mut self) -> BidiPosition {
        // `ubidi_getLength` is an int32; the text length is checked by Skia's callers.
        BidiPosition::try_from(self.levels.len()).expect("text length fits in int32")
    }

    // Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L26-L27 (chrome/m156)
    fn get_level_at(&mut self, pos: BidiPosition) -> BidiLevel {
        level_at(&self.levels, pos)
    }
}

// Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L51-L70 (chrome/m156), `MakeIterator`
// for UTF-16 text.
#[must_use]
pub fn make_bidi_iterator_utf16(text: &[u16], dir: BidiDirection) -> Option<Box<dyn BidiIterator>> {
    let analysis = analyze_utf16(text, dir);
    Some(Box::new(IcuBidiIterator {
        levels: analysis.levels,
    }))
}

// Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L72-L94 (chrome/m156), `MakeIterator`
// for UTF-8 text: invalid UTF-8 gives no iterator.
#[must_use]
pub fn make_bidi_iterator_utf8(text: &[u8], dir: BidiDirection) -> Option<Box<dyn BidiIterator>> {
    // `SkTFitsIn<int32_t>(utf8Units)`.
    if i32::try_from(text.len()).is_err() {
        return None;
    }
    // `SkUTF::UTF8ToUTF16(nullptr, 0, ...)` < 0 is invalid input.
    let utf16_units = utf::utf8_to_utf16(None, text);
    if utf16_units < 0 {
        return None;
    }
    let utf16 = convert_utf8_to_utf16(text);
    make_bidi_iterator_utf16(&utf16, dir)
}

// Port of: modules/skunicode/src/SkUnicode_icu_bidi.cpp#L96-L164 (chrome/m156),
// `SkBidiFactory::ExtractBidi`.
//
// Not ported: texts whose embedding controls leave them mixed on the first direction pass
// (`ubidi.cpp`'s `directionFromFlags` on the original classes). ICU then runs a second
// direction pass after explicit levels are resolved, which this port does not do. The regions
// of such text are the same whenever the second pass also reports mixed.
pub fn extract_bidi(utf8: &[u8], dir: TextDirection, results: &mut Vec<BidiRegion>) -> bool {
    // Convert to UTF-16, since the bidi iterator only operates on UTF-16.
    let utf16 = convert_utf8_to_utf16(utf8);
    let bidi_dir = match dir {
        TextDirection::Ltr => BidiDirection::Ltr,
        TextDirection::Rtl => BidiDirection::Rtl,
    };
    let analysis = analyze_utf16(&utf16, bidi_dir);
    let levels = &analysis.levels;

    let end16 = levels.len();
    if end16 == 0 {
        return true;
    }
    if !analysis.mixed {
        // The entire paragraph is unidirectional.
        results.push(BidiRegion::new(0, utf8.len(), level_at(levels, 0)));
        return true;
    }

    // Port of the loop of `ExtractBidi`: `start8` is the byte offset of the code point being
    // read, and `pos16` the UTF-16 position of its first unit.
    let mut current_level: BidiLevel = 0;
    let mut pos8: Position = 0;
    let mut pos16: usize = 0;
    let mut end8: Position = 0;
    let mut reading = true;
    for_each_codepoint_utf8(utf8, |_unichar, before, after, count| {
        if !reading {
            return;
        }
        if pos16 >= end16 {
            reading = false;
            return;
        }
        // `pos16 < end16` was checked above, so the index is in range.
        let level = levels[pos16];
        if pos16 == 0 {
            current_level = level;
        } else if level != current_level {
            let end = before;
            results.push(BidiRegion::new(pos8, end, current_level));
            current_level = level;
            pos8 = end;
        }
        end8 = after;
        pos16 += count;
    });
    // `end = start8 - utf8`: the byte offset after the last code point read.
    let end = end8;
    if end != pos8 {
        results.push(BidiRegion::new(pos8, end, current_level));
    }
    true
}

// Port of: modules/skunicode/src/SkBidiFactory_icu_full.cpp#L50-L59 (chrome/m156),
// `SkBidiICUFactory::bidi_reorderVisual`, and ICU common/ubidiln.cpp#L812-L866
// (`ubidi_reorderVisual`) with `prepareReorder` (ubidiln.cpp#L708-L747).
//
// The `logical_from_visual` slice must hold at least `run_levels.len()` entries, as ICU writes
// that many.
///
/// # Panics
///
/// Panics if `logical_from_visual` is shorter than `run_levels`.
pub fn reorder_visual(run_levels: &[BidiLevel], logical_from_visual: &mut [i32]) {
    // To avoid an assert in ICU.
    if run_levels.is_empty() {
        return;
    }
    let length = run_levels.len();
    let index_map = &mut logical_from_visual[..length];

    // Port of: ubidiln.cpp `prepareReorder`: the minimum and maximum levels, and the identity
    // index map. A level above UBIDI_MAX_EXPLICIT_LEVEL+1 makes ICU do nothing.
    let mut min_level: u8 = UBIDI_MAX_EXPLICIT_LEVEL + 1;
    let mut max_level: u8 = 0;
    for &level in run_levels.iter().rev() {
        if level > UBIDI_MAX_EXPLICIT_LEVEL + 1 {
            return;
        }
        if level < min_level {
            min_level = level;
        }
        if level > max_level {
            max_level = level;
        }
    }
    for (start, entry) in index_map.iter_mut().enumerate() {
        *entry = i32::try_from(start).expect("in range");
    }

    // Nothing to do? (ICU: `if(minLevel==maxLevel && (minLevel&1)==0)`.)
    if min_level == max_level && (min_level & 1) == 0 {
        return;
    }

    // Reorder only down to the lowest odd level.
    min_level |= 1;

    // Loop maxLevel..minLevel. The C loop is `do { ... } while(--maxLevel>=minLevel)`; the
    // levels are never negative, so i32 comparisons give the same iterations.
    let mut max = i32::from(max_level);
    let min = i32::from(min_level);
    loop {
        let mut start: usize = 0;

        // Loop for all sequences of levels to reorder at the current maxLevel.
        loop {
            // Look for the first index of a sequence of levels that are all at >= maxLevel.
            while start < length && i32::from(run_levels[start]) < max {
                start += 1;
            }
            if start >= length {
                break; // No more such runs.
            }

            // Look for the limit of such a sequence (the index behind it).
            let mut limit = start;
            loop {
                limit += 1;
                if !(limit < length && i32::from(run_levels[limit]) >= max) {
                    break;
                }
            }

            // Swap the entire interval of indexes from start to limit-1. The levels do not
            // move, so they are not swapped.
            let mut end = limit - 1;
            while start < end {
                index_map.swap(start, end);
                start += 1;
                end -= 1;
            }

            if limit == length {
                break; // No more such sequences.
            }
            start = limit + 1;
        }

        max -= 1;
        if max < min {
            break;
        }
    }
}
