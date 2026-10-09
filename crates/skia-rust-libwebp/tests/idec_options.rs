//! Replays the incremental differential with decoder options of `oracle/codec-diff/libwebp`
//! (`webpidec.c -v`): each corpus file is fed to `IDecoder::update` as a growing prefix in steps of
//! 1, 7, 64 and 4096 bytes, in every RGB output mode, with the cropping and scaling that
//! `SkWebpCodec` sets for a subset and a scaled decode. The status, the rows written and their
//! pixels are compared with the C reference after each call (`expected_idec_options.txt`).

mod common;

use common::{CHUNKS, MODES, idec_lines_with, oracle_dir};

/// The option variants, in the order of `kVariants` in `webpidec.c`.
const VARIANTS: [&str; 5] = [
    "crop=10,20,150,120",
    "scale=37,41",
    "scale=800,600",
    "crop=10,20,150,120+scale=75,60",
    "crop=3,5,100,77+nofancy",
];

#[test]
fn idec_options_replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected_idec_options.txt"))
        .expect("read expected_idec_options.txt");
    // The files, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let base = l.split(' ').next().expect("basename");
        if !files.contains(&base) {
            files.push(base);
        }
    }
    // `webpidec.c` loops over the modes and chunks outside the variants.
    let mut actual = String::new();
    for base in files {
        let data = std::fs::read(dir.join("corpus").join(base)).expect("read corpus file");
        for (name, mode, bpp) in MODES {
            for chunk in CHUNKS {
                for variant in VARIANTS {
                    actual.push_str(&idec_lines_with(
                        base,
                        Some(variant),
                        &data,
                        name,
                        mode,
                        bpp,
                        chunk,
                    ));
                }
            }
        }
    }
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let mismatches: Vec<String> = expected_lines
        .iter()
        .zip(actual_lines.iter())
        .filter(|(e, a)| e != a)
        .take(20)
        .map(|(e, a)| format!("expected: {e}\n  actual: {a}"))
        .collect();
    assert!(
        mismatches.is_empty() && expected_lines.len() == actual_lines.len(),
        "{} of {} lines differ (first 20 shown):\n{}",
        expected_lines.len().max(actual_lines.len()) - mismatches.len(),
        expected_lines.len(),
        mismatches.join("\n")
    );
}
