// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkCapabilities.h, src/core/SkCapabilities.cpp

//! `SkCapabilities`: what a backend can run. For now, only the `SkSL` version
//! ([`Capabilities::raster_backend`]).

use skia_rust_sksl::program_settings::Version;

/// Describes what a backend supports (`SkCapabilities`).
// Port of: include/core/SkCapabilities.h#L20-L40 (chrome/m156)
#[doc(alias = "SkCapabilities")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    sksl_version: Version,
}

impl Capabilities {
    /// The capabilities of the raster backend: `SkSL` version 100
    /// (`SkCapabilities::RasterBackend`).
    // Port of: src/core/SkCapabilities.cpp#L12-L20 (chrome/m156)
    #[doc(alias = "RasterBackend")]
    #[must_use]
    pub fn raster_backend() -> &'static Capabilities {
        static RASTER: Capabilities = Capabilities {
            sksl_version: Version::K100,
        };
        &RASTER
    }

    /// The highest `SkSL` version the backend runs (`skslVersion`).
    #[doc(alias = "skslVersion")]
    #[must_use]
    pub fn sksl_version(&self) -> Version {
        self.sksl_version
    }
}
