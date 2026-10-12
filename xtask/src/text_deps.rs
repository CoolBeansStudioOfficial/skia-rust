//! `cargo xtask text-deps fetch`: the third-party inputs of the text modules (design:
//! `docs/design/modules.md` §9, M0b). The work is in `oracle/text-deps`; this prints where
//! everything ended up.

// `HarfBuzz` and `HarfRust` are proper names, not code.
#![allow(clippy::doc_markdown)]

use anyhow::{Result, anyhow};
use clap::ValueEnum;

/// An input `text-deps fetch` can fetch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Dep {
    /// HarfBuzz `9cb1fee5` (`src/`).
    Harfbuzz,
    /// Chromium ICU `d578f2e8`: `source/common` and `common/icudtl.dat`.
    Icu,
    /// The `skparagraph` test fonts (release asset `test-fonts-m156`).
    Fonts,
}

pub fn fetch(only: &[Dep]) -> Result<()> {
    let wanted = |dep: Dep| only.is_empty() || only.contains(&dep);
    println!("cache: {}", skia_rust_text_deps::cache_root().display());
    if wanted(Dep::Harfbuzz) {
        let dir = skia_rust_text_deps::harfbuzz().map_err(|e| anyhow!(e))?;
        println!(
            "harfbuzz: {}",
            skia_rust_text_deps::harfbuzz_src(&dir).display()
        );
    }
    if wanted(Dep::Icu) {
        let icu = skia_rust_text_deps::icu().map_err(|e| anyhow!(e))?;
        println!("icu common: {}", icu.common.display());
        println!("icu data: {}", icu.data.display());
    }
    if wanted(Dep::Fonts) {
        let dir = skia_rust_text_deps::test_fonts().map_err(|e| anyhow!(e))?;
        println!("test fonts: {}", dir.display());
    }
    Ok(())
}
