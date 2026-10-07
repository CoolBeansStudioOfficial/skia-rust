// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/Test.cpp (skiatest::GetTmpDir) (chrome/m156)

//! A scratch directory for tests that write files.

use std::path::PathBuf;

/// Port of `skiatest::GetTmpDir`: a directory the tests may write files into, or `None` if
/// there is none. (Skia's is the `--tmpDir` flag, empty by default and then file tests are
/// skipped; here it is always `<system temp dir>/skia-rust-tests`.)
#[must_use]
pub fn get_tmp_dir() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("skia-rust-tests");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
