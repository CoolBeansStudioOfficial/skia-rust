// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/OSPathTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::os_path::{self, SEPARATOR};

use crate::{Reporter, def_test, reporter_assert};

// Test SkOSPath::Join, SkOSPath::Basename, and SkOSPath::Dirname.
// Will use SkOSPath::Join to append filename to dir, test that it works correctly,
// and tests using SkOSPath::Basename on the result.
// Port of: tests/OSPathTest.cpp#L26-L67 (chrome/m156)
fn test_dir_with_file(reporter: &mut Reporter, dir: &str, filename: &str) {
    // If filename contains SkOSPath::SEPARATOR, the tests will fail.
    assert!(!filename.contains(SEPARATOR));

    // Tests for SkOSPath::Join and SkOSPath::Basename

    // fullName should be "dir<SkOSPath::SEPARATOR>file"
    let full_name = os_path::join(Some(dir), Some(filename));

    // fullName should be the combined size of dir and file, plus one if
    // dir did not include the final path separator.
    let mut expected_size = dir.len() + filename.len();
    if !dir.ends_with(SEPARATOR) && !dir.is_empty() {
        expected_size += 1;
    }
    reporter_assert!(reporter, full_name.len() == expected_size);

    let base_name = os_path::basename(Some(&full_name));
    let dir_name = os_path::dirname(Some(&full_name));

    // basename should be the same as filename
    reporter_assert!(reporter, base_name == filename);

    // dirname should be the same as dir with any trailing seperators removed.
    // Except when the the string is just "/".
    let mut stripped_dir = dir.to_owned();
    while stripped_dir.len() > 2 && stripped_dir.ends_with(SEPARATOR) {
        stripped_dir.pop();
    }
    if dir_name != stripped_dir {
        // SkDebugf("OOUCH %s %s %s\n", ...)
        eprintln!("OOUCH {dir} {stripped_dir} {dir_name}");
    }
    reporter_assert!(reporter, dir_name == stripped_dir);

    // basename will not contain a path separator
    reporter_assert!(reporter, !base_name.contains(SEPARATOR));

    // Now take the basename of filename, which should be the same as filename.
    let base_name = os_path::basename(Some(filename));
    reporter_assert!(reporter, base_name == filename);
}

// Port of: tests/OSPathTest.cpp#L69-L109 (chrome/m156)
def_test!(OSPath, |reporter| {
    let mut dir = "dir".to_owned();
    let mut filename = "file".to_owned();
    test_dir_with_file(reporter, &dir, &filename);

    // Now make sure this works with a path separator at the end of dir.
    dir.push(SEPARATOR);
    test_dir_with_file(reporter, &dir, &filename);

    // Test using no filename.
    test_dir_with_file(reporter, &dir, "");

    // Testing using no directory.
    test_dir_with_file(reporter, "", &filename);

    // Test with a sub directory.
    dir.push_str("subDir");
    test_dir_with_file(reporter, &dir, &filename);

    // Basename of a directory with a path separator at the end is empty.
    dir.push(SEPARATOR);
    let base_of_dir = os_path::basename(Some(&dir));
    reporter_assert!(reporter, base_of_dir.is_empty());

    // Basename of nullptr is an empty string.
    let empty = os_path::basename(None);
    reporter_assert!(reporter, empty.is_empty());

    // File in root dir
    dir = SEPARATOR.to_string();
    filename = "file".to_owned();
    test_dir_with_file(reporter, &dir, &filename);

    // Just the root dir
    filename.clear();
    test_dir_with_file(reporter, &dir, &filename);

    // Test that nullptr can be used for the directory and filename.
    let empty_path = os_path::join(None, None);
    reporter_assert!(reporter, empty_path.is_empty());
});
