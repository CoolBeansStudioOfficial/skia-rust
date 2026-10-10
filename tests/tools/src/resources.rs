// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/Resources.h (GetResourcePath, GetResourceAsStream) (chrome/m156)

//! Access to Skia's `resources/` directory for the test tools (the SVG glyphs of the test
//! typefaces).
//!
//! The directory is looked up, in order, at the path in the `SKIA_RESOURCES` environment variable
//! and at `third_party/skia/resources` of this workspace (`cargo xtask skia fetch`).

use std::path::PathBuf;

use skia_rust_core::stream::{MemoryStream, StreamAsset};

/// `GetResourcePath`: the directory holding Skia's resources, if it exists.
#[must_use]
pub fn resource_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("SKIA_RESOURCES") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("third_party")
            .join("skia")
            .join("resources"),
    );
    candidates.into_iter().find(|dir| dir.is_dir())
}

/// `GetResourceAsStream`: a stream over the resource at `path` (relative to Skia's `resources`
/// directory, using `/` separators), or `None` if it cannot be read.
#[must_use]
pub fn get_resource_as_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let mut full = resource_dir()?;
    for component in path.split('/') {
        full.push(component);
    }
    let bytes = std::fs::read(full).ok()?;
    let stream: Box<dyn StreamAsset> = MemoryStream::make_copy(&bytes);
    Some(stream)
}
