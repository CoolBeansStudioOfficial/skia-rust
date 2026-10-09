//! Replays the incremental differential of `oracle/codec-diff/libwebp` (`webpidec.c`): each corpus
//! file is fed to `IDecoder::update` as a growing prefix in steps of 1, 7, 64 and 4096 bytes, in
//! every RGB output mode, and the status, the rows written and the pixels of those rows are
//! compared with the C reference after each call (`expected_idec.txt`). The truncated and
//! corrupted variants are replayed in `mutated.rs`.

mod common;

use common::{CHUNKS, MODES, idec_lines, oracle_dir};

#[test]
fn idec_replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected =
        std::fs::read_to_string(dir.join("expected_idec.txt")).expect("read expected_idec.txt");
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
            for chunk in CHUNKS {
                actual.push_str(&idec_lines(base, &data, name, mode, bpp, chunk));
            }
        }
    }
    assert_eq!(actual, expected);
}
