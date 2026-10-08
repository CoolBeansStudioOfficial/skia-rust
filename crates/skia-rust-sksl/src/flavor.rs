// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLModuleDataDefault.cpp and tools/skslc (the two build flavours).

//! The two build flavours of the compiler, chosen by an explicit value (`docs/design/sksl.md`
//! decision 6), never by a global.

/// Which of Skia's two `SkSL` module texts a compiler loads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleSource {
    /// The minified texts (`src/sksl/generated/*.minified.sksl`), which Skia's release build
    /// compiles into every GM and unit test.
    Minified,
    /// The original sources (`src/sksl/sksl_*.sksl`), which `skslc` read when it generated the
    /// `tests/sksl` goldens. They give observably different output (parameter names).
    Original,
}

/// Which build of Skia the compiler imitates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Flavor {
    /// Skia built as a library: `SkSLModuleDataDefault.cpp` with minified modules. This is the
    /// default, and what every GM and unit test was rendered with.
    #[default]
    Library,
    /// `skslc` (`SKSL_STANDALONE`): the original module sources. Only the golden harness uses it.
    Standalone,
}

impl Flavor {
    /// The module text this flavour loads (`docs/design/sksl.md` §1.3).
    #[must_use]
    #[doc(alias = "SKSL_STANDALONE")]
    pub fn module_source(self) -> ModuleSource {
        match self {
            Self::Library => ModuleSource::Minified,
            Self::Standalone => ModuleSource::Original,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Flavor, ModuleSource};

    #[test]
    fn library_is_the_default_and_loads_minified_modules() {
        assert_eq!(Flavor::default(), Flavor::Library);
        assert_eq!(Flavor::Library.module_source(), ModuleSource::Minified);
        assert_eq!(Flavor::Standalone.module_source(), ModuleSource::Original);
    }
}
