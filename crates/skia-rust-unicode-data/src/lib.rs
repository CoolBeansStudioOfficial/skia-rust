// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// The data files in `src/data` are ICU's: see `src/data/LICENSE`.

//! The break data of Chromium's ICU at Skia's pin (`chromium/deps/icu`, [`ICU_REVISION`]): the
//! compiled rule tables (`brkitr/*.brk`) and dictionaries (`brkitr/*.dict`) of `common/icudtl.dat`,
//! byte for byte (milestone M4, `docs/design/modules.md` §3.6 and §9).
//!
//! They are what Chromium's ICU loads at run time. Chromium builds them from
//! `source/data/brkitr` with its patches applied: `wordbrk.patch` (`.` and U+FF0E move from
//! `MidNumLet` to `MidNum`, `:` leaves `MidLetter`), `line_normal.patch` (the default line rules are
//! `line_normal`), `lstm.patch` (no LSTM models: Thai and Burmese use the dictionaries) and
//! `cjdict.patch` (more Chinese words). `khmer-dictbe.patch` changes code, not data;
//! `skia_rust_unicode::icu_shim` ports it.
//!
//! The files are kept in ICU's binary form (behind the data header ICU's `udata` reads), which is
//! the compact form: about 3.3 MB, against several times that as Rust source. The shim reads them
//! in place. `cargo xtask text-deps break-data` regenerates the crate after an ICU pin bump
//! (`--check` verifies it); the digests are in [`FILES`].

mod generated;

pub use generated::{FILES, ICU_REVISION, dictionary, rule_file};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_is_reachable() {
        for (file, _) in FILES {
            let found = if let Some(stem) = file.strip_suffix(".brk") {
                rule_file(stem)
            } else if let Some(stem) = file.strip_suffix(".dict") {
                dictionary(stem)
            } else {
                None
            };
            let bytes = found.unwrap_or_else(|| panic!("{file} is not reachable"));
            // ICU's `MappedData` header: `headerSize`, magic 0xda 0x27, then `UDataInfo` with
            // `isBigEndian` at byte 8.
            assert_eq!(&bytes[2..4], &[0xda, 0x27], "{file}");
            assert_eq!(bytes[8], 0, "{file} is not little-endian");
        }
        assert!(rule_file("line_cj").is_none());
        assert!(rule_file("title").is_none());
    }
}
