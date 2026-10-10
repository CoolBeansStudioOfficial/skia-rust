// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/utils/SkOSPath.h and src/utils/SkOSPath.cpp (chrome/m156)

//! Path string helpers (`SkOSPath`): joining, and splitting a path into its directory and its
//! base name.

/// The path separator of the platform (`SkOSPath::SEPARATOR`).
// Port of: src/utils/SkOSPath.h#L18-L22 (chrome/m156)
#[cfg(windows)]
pub const SEPARATOR: char = '\\';
/// The path separator of the platform (`SkOSPath::SEPARATOR`).
// Port of: src/utils/SkOSPath.h#L18-L22 (chrome/m156)
#[cfg(not(windows))]
pub const SEPARATOR: char = '/';

/// Joins `relative_path` onto `root_path` with a separator between them, unless `root_path`
/// already ends with one or is empty. `None` is an empty string (`SkOSPath::Join`).
// Port of: src/utils/SkOSPath.cpp#L14-L21 (chrome/m156)
#[doc(alias = "Join")]
#[must_use]
pub fn join(root_path: Option<&str>, relative_path: Option<&str>) -> String {
    let mut result = root_path.unwrap_or("").to_owned();
    if !result.ends_with(SEPARATOR) && !result.is_empty() {
        result.push(SEPARATOR);
    }
    result.push_str(relative_path.unwrap_or(""));
    result
}

/// The part of `full_path` after its last separator, or all of it if there is none. `None` is
/// an empty string (`SkOSPath::Basename`).
// Port of: src/utils/SkOSPath.cpp#L23-L34 (chrome/m156)
#[doc(alias = "Basename")]
#[must_use]
pub fn basename(full_path: Option<&str>) -> String {
    let Some(full_path) = full_path else {
        return String::new();
    };
    match full_path.rfind(SEPARATOR) {
        None => full_path.to_owned(),
        Some(index) => full_path[index + SEPARATOR.len_utf8()..].to_owned(),
    }
}

/// The part of `full_path` before its last separator. A separator at the start gives `"/"`, and
/// a path without a separator gives an empty string. `None` is an empty string
/// (`SkOSPath::Dirname`).
// Port of: src/utils/SkOSPath.cpp#L36-L49 (chrome/m156)
#[doc(alias = "Dirname")]
#[must_use]
pub fn dirname(full_path: Option<&str>) -> String {
    let Some(full_path) = full_path else {
        return String::new();
    };
    match full_path.rfind(SEPARATOR) {
        None => String::new(),
        // The root: keep the separator itself.
        Some(0) => full_path[..SEPARATOR.len_utf8()].to_owned(),
        Some(index) => full_path[..index].to_owned(),
    }
}
