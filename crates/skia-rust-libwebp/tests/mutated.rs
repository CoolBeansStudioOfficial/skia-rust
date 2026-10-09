//! Replays the truncated and corrupted variants of the corpus through the Rust decoder and
//! compares the result line by line with `expected_mutated.txt`, the output of the C reference on
//! the same bytes.
//!
//! The variants are generated here in-process with the rules of `oracle/codec-diff/libwebp/
//! mutate.py` (`variants`), so no mutated files are committed.

// clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss: the
// corpus lengths and the corruption positions are small non-negative values, converted between
// `usize` and `i64` as in `mutate.py`.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

mod common;

use std::fmt::Write as _;
use std::path::Path;

use common::{CHUNKS, MODES, idec_lines, line, oracle_dir};

/// Corrupted variants per corpus file (`CORRUPTIONS` in `mutate.py`).
const CORRUPTIONS: usize = 2;

/// One step of the 64-bit xorshift used for the corruption positions.
fn xorshift(state: u64) -> u64 {
    let mut s = state;
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    s
}

/// The variants of one corpus file, in the generation order of `mutate.py`.
fn variants(name: &str, data: &[u8], index: usize) -> Vec<(String, Vec<u8>)> {
    let n = data.len() as i64;
    let mut out = Vec::new();
    let mut lengths: Vec<i64> = Vec::new();
    for cut in [0, 1, 8, 12, 20, 30, n / 4, n / 2, (3 * n) / 4, n - 1] {
        if (0..n).contains(&cut) && !lengths.contains(&cut) {
            lengths.push(cut);
        }
    }
    for cut in lengths {
        out.push((format!("{name}.t{cut}.webp"), data[..cut as usize].to_vec()));
    }
    let mut state = (index as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for k in 0..CORRUPTIONS {
        let mut buf = data.to_vec();
        for _ in 0..4 {
            state = xorshift(state);
            let pos = (state % data.len() as u64) as usize;
            buf[pos] ^= (((state >> 8) & 0xFF) as u8) | 1;
        }
        out.push((format!("{name}.c{k}.webp"), buf));
    }
    out
}

/// Every variant of the corpus, in the order of `expected_mutated.txt`: the corpus files sorted by
/// name, each with its own variants. The `usize` is the file's index, which seeds its corruptions.
fn corpus_variants(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("corpus"))
        .expect("read corpus dir")
        .map(|e| {
            e.expect("corpus entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "webp"))
        .collect();
    names.sort();
    let mut out = Vec::new();
    for (index, file) in names.iter().enumerate() {
        let data = std::fs::read(dir.join("corpus").join(file)).expect("read corpus file");
        let base = file.strip_suffix(".webp").expect("corpus file is a .webp");
        out.extend(variants(base, &data, index));
    }
    out
}

#[test]
fn mutated_variants_match_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected_mutated.txt"))
        .expect("read expected_mutated.txt");
    let mut actual = String::new();
    for (vname, vdata) in corpus_variants(&dir) {
        for (name, mode, bpp) in MODES {
            writeln!(actual, "{}", line(&vname, name, mode, bpp, &vdata)).expect("write to String");
        }
    }
    assert_eq!(actual, expected);
}

/// The incremental replay of the variants (`expected_mutated_idec.txt`, from `webpidec` on the
/// files `mutate.py` writes): every variant, every RGB mode, every chunk size.
#[test]
fn mutated_variants_incremental_match_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected_mutated_idec.txt"))
        .expect("read expected_mutated_idec.txt");
    let mut actual = String::new();
    for (vname, vdata) in corpus_variants(&dir) {
        for (name, mode, bpp) in MODES {
            for chunk in CHUNKS {
                actual.push_str(&idec_lines(&vname, &vdata, name, mode, bpp, chunk));
            }
        }
    }
    assert_eq!(actual, expected);
}
