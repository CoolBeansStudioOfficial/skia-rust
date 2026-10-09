// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (Config, CPU_CONFIG)

//! The CPU configs of nanobench.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;

use crate::Backend;

/// `Config`: the CPU part (no GPU context fields).
// Port of: bench/nanobench.cpp#L190-L205 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub name: &'static str,
    pub backend: Backend,
    pub color: ColorType,
    pub alpha: AlphaType,
}

const fn cfg(name: &'static str, backend: Backend, color: ColorType, alpha: AlphaType) -> Config {
    Config {
        name,
        backend,
        color,
        alpha,
    }
}

/// Every CPU config this port knows, in nanobench's order. `srgba` is not here: it needs the
/// sRGB color space of the `SkCommandLineConfig` "via" part, which the raster harness leaves out.
// Port of: bench/nanobench.cpp#L691-L720 (chrome/m156)
pub const CPU_CONFIGS: [Config; 9] = [
    cfg(
        "nonrendering",
        Backend::NonRendering,
        ColorType::Unknown,
        AlphaType::Unpremul,
    ),
    cfg("a8", Backend::Raster, ColorType::Alpha8, AlphaType::Premul),
    cfg(
        "gray8",
        Backend::Raster,
        ColorType::Gray8,
        AlphaType::Opaque,
    ),
    cfg("r8", Backend::Raster, ColorType::R8UNorm, AlphaType::Opaque),
    cfg("565", Backend::Raster, ColorType::RGB565, AlphaType::Opaque),
    cfg("8888", Backend::Raster, ColorType::N32, AlphaType::Premul),
    cfg(
        "rgba",
        Backend::Raster,
        ColorType::RGBA8888,
        AlphaType::Premul,
    ),
    cfg(
        "bgra",
        Backend::Raster,
        ColorType::BGRA8888,
        AlphaType::Premul,
    ),
    cfg(
        "f16",
        Backend::Raster,
        ColorType::RGBAF16,
        AlphaType::Premul,
    ),
];

/// The configs the smoke run (design §3.1, C2) uses.
pub const SMOKE_CONFIGS: [&str; 4] = ["nonrendering", "8888", "565", "f16"];

/// The default `--config` minus `gl` (`"8888 gl nonrendering"`).
pub const DEFAULT_CONFIGS: [&str; 2] = ["8888", "nonrendering"];

/// Looks a config up by its tag.
#[must_use]
pub fn find(name: &str) -> Option<Config> {
    CPU_CONFIGS.into_iter().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_resolve() {
        for name in SMOKE_CONFIGS.into_iter().chain(DEFAULT_CONFIGS) {
            assert!(find(name).is_some(), "{name}");
        }
        assert!(find("gl").is_none());
        assert_eq!(find("nonrendering").unwrap().backend, Backend::NonRendering);
    }
}
