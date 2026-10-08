// Copyright 2017 The Wuffs Authors (the Wuffs sources this file ports).
// Modifications (the Rust port) Copyright (C) 2025 The skia-rust Authors.
// Licensed under the Apache License, Version 2.0; see the LICENSE file of this crate. This file
// is modified from the Wuffs sources.
//! Replays the differential dump of the GIF decoder against the committed output of the C driver
//! (`oracle/codec-diff/wuffs/wuffsdump.c`, built against Wuffs v0.3 at `e3f919cc`), which is
//! stored in `oracle/codec-diff/wuffs/expected/gif.txt`.
//!
//! The inputs are Skia's GIF resources. They are not in CI's checkout, so the test is skipped
//! there, as the other resource tests are. To regenerate the expected output from this port (only
//! after the C driver has been rebuilt and the two outputs agree), run
//! `WUFFS_DUMP_WRITE=<file> cargo test -p skia-rust-wuffs --test differential`.

mod dump;

use std::fs;
use std::path::Path;

/// The inputs, in the order the expected output was generated.
const FILES: [&str; 19] = [
    "empty_images/zero-dims.gif",
    "images/alphabetAnim.gif",
    "images/box.gif",
    "images/colorTables.gif",
    "images/color_wheel.gif",
    "images/flightAnim.gif",
    "images/gif-transparent-index.gif",
    "images/out-of-palette.gif",
    "images/randPixels.gif",
    "images/randPixelsAnim.gif",
    "images/randPixelsAnim2.gif",
    "images/randPixelsOffset.gif",
    "images/required.gif",
    "images/test640x479.gif",
    "images/xOffsetTooBig.gif",
    "invalid_images/ossfuzz6274.gif",
    "invalid_images/skbug5883.gif",
    "invalid_images/skbug5887.gif",
    "invalid_images/skbug6046.gif",
];

#[test]
fn gif_dump_matches_the_wuffs_v0_3_c_driver() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let resources = manifest.join("../../third_party/skia/resources");
    if !resources.join("images").is_dir() {
        eprintln!(
            "skipping: Skia resources are not checked out ({})",
            resources.display()
        );
        return;
    }
    let files: Vec<(&str, Vec<u8>)> = FILES
        .iter()
        .map(|rel| {
            let base = rel.rsplit('/').next().unwrap_or(rel);
            let data = fs::read(resources.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
            (base, data)
        })
        .collect();
    let got = dump::dump(&files);
    if let Some(path) = std::env::var_os("WUFFS_DUMP_WRITE") {
        fs::write(path, &got).expect("writing WUFFS_DUMP_WRITE");
    }

    let expected_path = manifest.join("../../oracle/codec-diff/wuffs/expected/gif.txt");
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
    if got != expected {
        let line = got
            .lines()
            .zip(expected.lines())
            .position(|(g, e)| g != e)
            .unwrap_or_else(|| got.lines().count().min(expected.lines().count()));
        let g = got.lines().nth(line).unwrap_or("<end of output>");
        let e = expected.lines().nth(line).unwrap_or("<end of output>");
        panic!(
            "first difference at line {}:\n  port:   {g}\n  oracle: {e}",
            line + 1
        );
    }
}
