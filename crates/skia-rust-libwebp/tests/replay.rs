//! Replays the differential corpus of `oracle/codec-diff/libwebp` through the Rust decoder and
//! compares the result line by line with `expected.txt`, which is the output of the C reference
//! (`webpdump.c`, built from the pinned libwebp 1.4.0 by `build.sh`).

mod common;

use std::fmt::Write as _;

use common::{MODES, line, oracle_dir};

#[test]
fn replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected.txt")).expect("read expected.txt");
    // The files, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let base = l.split(' ').next().expect("basename");
        if !files.contains(&base) {
            files.push(base);
        }
    }
    let mut actual = String::new();
    for base in files {
        let data = std::fs::read(dir.join("corpus").join(base)).expect("read corpus file");
        for (name, mode, bpp) in MODES {
            writeln!(actual, "{}", line(base, name, mode, bpp, &data)).expect("write to String");
        }
    }
    assert_eq!(actual, expected);
}
