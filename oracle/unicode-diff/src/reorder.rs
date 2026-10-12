// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `reorder` section: `ubidi_reorderVisual`, which no ICU4X or `unicode-bidi` API offers with
//! ICU's contract, so the twin is a function-by-function port of ICU's `ubidiln.cpp`
//! (`prepareReorder`, `ubidi_reorderVisual`) from the pinned tree. It is the seed of M3's port.

use std::fmt::Write as _;

use crate::hash16;

/// Port of: `source/common/ubidiln.cpp`, `prepareReorder` and `ubidi_reorderVisual`
/// (chromium/deps/icu d578f2e8). `None` is ICU's early return that leaves the index map
/// untouched: no levels, or a level above `UBIDI_MAX_EXPLICIT_LEVEL + 1` (126).
#[must_use]
pub fn reorder_visual(levels: &[u8]) -> Option<Vec<i32>> {
    const MAX_EXPLICIT_LEVEL: u8 = 125;
    if levels.is_empty() {
        return None;
    }
    let mut min_level = MAX_EXPLICIT_LEVEL + 1;
    let mut max_level = 0u8;
    for &level in levels.iter().rev() {
        if level > MAX_EXPLICIT_LEVEL + 1 {
            return None;
        }
        min_level = min_level.min(level);
        max_level = max_level.max(level);
    }
    let length = levels.len();
    // Indices are below `length`, which is far below `i32::MAX` here.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    let mut index_map: Vec<i32> = (0..length).map(|i| i as i32).collect();

    if min_level == max_level && min_level & 1 == 0 {
        return Some(index_map);
    }
    min_level |= 1;
    loop {
        let mut start = 0;
        loop {
            while start < length && levels[start] < max_level {
                start += 1;
            }
            if start >= length {
                break;
            }
            let mut limit = start + 1;
            while limit < length && levels[limit] >= max_level {
                limit += 1;
            }
            let mut end = limit - 1;
            while start < end {
                index_map.swap(start, end);
                start += 1;
                end -= 1;
            }
            if limit == length {
                break;
            }
            start = limit + 1;
        }
        // `while(--maxLevel>=minLevel)`
        max_level -= 1;
        if max_level < min_level {
            break;
        }
    }
    Some(index_map)
}

fn group(name: &str, alphabet: &[u8], max_len: usize, dump: bool, out: &mut String) {
    for len in 1..=max_len {
        let total = alphabet.len().pow(u32::try_from(len).expect("small"));
        let mut all = String::new();
        for n in 0..total {
            let mut lv = vec![0u8; len];
            let mut m = n;
            for k in (0..len).rev() {
                lv[k] = alphabet[m % alphabet.len()];
                m /= alphabet.len();
            }
            // Every sequence here is valid, so ICU always fills the map.
            let map = reorder_visual(&lv).expect("levels are valid");
            for (k, l) in lv.iter().enumerate() {
                let _ = write!(all, "{l}{}", if k + 1 < len { "," } else { ">" });
            }
            for (k, m) in map.iter().enumerate() {
                let _ = write!(all, "{m}{}", if k + 1 < len { "," } else { "\n" });
            }
        }
        if dump {
            let _ = write!(out, "reorder {name}{len} 0 {all}");
        } else {
            let _ = writeln!(out, "reorder {name}{len} 0 {total} {}", hash16(&all));
        }
    }
}

/// Prints the section.
#[must_use]
pub fn run(dump: bool) -> String {
    let mut out = String::new();
    group("small", &[0, 1, 2, 3], 7, dump, &mut out);
    group("big", &[0, 1, 2, 125, 126], 5, dump, &mut out);
    out
}
