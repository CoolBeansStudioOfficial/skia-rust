// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/Resources.h (GetResourceAsData, GetResourcePath) (chrome/m156)

//! Access to Skia's `resources/` directory (ICC profiles, images, ...) for the ported tests.
//!
//! The directory is looked up, in order, at:
//! 1. the path in the `SKIA_RESOURCES` environment variable,
//! 2. `third_party/skia/resources` of this workspace (see `cargo xtask skia fetch`),
//! 3. `C:\Users\Levi\Documents\GitHub\skia-rust\skia-rust\third_party\skia\resources`.
//!
//! Tests whose resources are missing should skip themselves with [`skip_missing_resource!`]
//! (the manifest keeps them `todo` on machines without the resources).

use std::path::PathBuf;

/// The absolute fallback location of Skia's `resources` directory.
const FALLBACK_RESOURCES_DIR: &str =
    r"C:\Users\Levi\Documents\GitHub\skia-rust\skia-rust\third_party\skia\resources";

/// Port of `GetResourcePath`: the directory holding Skia's resources, if it exists.
#[must_use]
pub fn resource_dir() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("SKIA_RESOURCES") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("third_party")
            .join("skia")
            .join("resources"),
    );
    candidates.push(PathBuf::from(FALLBACK_RESOURCES_DIR));
    candidates.into_iter().find(|dir| dir.is_dir())
}

/// Port of `GetResourceAsData`: the contents of the resource at `path` (relative to Skia's
/// `resources` directory, using `/` separators), or `None` if it cannot be read.
#[must_use]
pub fn get_resource_as_data(path: &str) -> Option<Vec<u8>> {
    let mut full = resource_dir()?;
    for component in path.split('/') {
        full.push(component);
    }
    std::fs::read(full).ok()
}

/// Port of `ToolUtils::GetResourceAsImage`: a lazy image of the encoded resource at `path`, or
/// `None` if the resource is missing or no codec decodes it
/// (`SkImages::DeferredFromEncodedData(GetResourceAsData(path))`).
// Port of: tools/DecodeUtils.h#L31-L33 (chrome/m156)
#[must_use]
pub fn get_resource_as_image(path: &str) -> Option<skia_rust_core::image::Image> {
    let data = get_resource_as_data(path)?;
    skia_rust_codec::images::deferred_from_encoded_data(
        Some(skia_rust_core::data::Data::new_from_vec(data)),
        None,
    )
}

/// Skips the rest of a test (with a note on stderr) when a resource is missing: use as
/// `let data = skip_missing_resource!(get_resource_as_data("icc_profiles/x.icc"), "icc_profiles/x.icc");`.
/// (An optional third argument is the value to return, for closures that return a value.)
#[macro_export]
macro_rules! skip_missing_resource {
    ($data:expr, $path:expr $(, $ret:expr)?) => {
        match $data {
            Some(data) => data,
            None => {
                eprintln!("todo: skipping, missing Skia resource {}", $path);
                return $($ret)?;
            }
        }
    };
}
