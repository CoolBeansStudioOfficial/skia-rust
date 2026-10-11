// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/Fuzz.h (DEF_FUZZ, Fuzzable)

//! 1:1 ports of Skia's fuzzers (`fuzz/*.cpp`), and the replay that checks them on stable Rust
//! (design: `docs/design/fuzz.md`).
//!
//! # Writing a fuzzer
//! A file `fuzz/<File>.cpp` is ported to `src/fuzz/<file_snake>.rs` (`fuzz/FuzzCubicRoots.cpp` →
//! `fuzz::fuzz_cubic_roots`), its `Fuzz` calls one for one through [`Fuzz`]. Each
//! `DEF_FUZZ(Name, fuzz) { … }` becomes
//!
//! ```ignore
//! def_fuzz!(Name, fuzz_Name, |fuzz| { … });
//! ```
//!
//! which defines `pub fn fuzz_Name(fuzz: &mut Fuzz<'_>)` (the C++ `fuzz_##name`) and a `#[test]`
//! named `Name` that replays the committed seeds `fuzz/corpus/Name/` and the crash reproducers
//! `fuzz/regressions/Name/` through it ([`replay`]). Register the fuzzer in [`fuzz::FUZZERS`] and
//! give it seeds in [`seeds`]. `cargo xtask inventory verify` maps the test
//! `fuzz::<file_snake>::Name` back to the manifest entry `fuzz/<File>.cpp::Name`.

pub mod canvas_helpers;
pub mod common;
pub mod fuzz;
mod reader;
pub mod replay;
pub mod seeds;

pub use reader::{Fuzz, FuzzValue};

/// A registered fuzzer (`struct Fuzzable`): its Skia name and entry point.
// Port of: fuzz/Fuzz.h#L153-L156 (chrome/m156)
#[derive(Debug, Clone, Copy)]
pub struct Fuzzable {
    /// The `DEF_FUZZ` name, verbatim.
    pub name: &'static str,
    /// The fuzzer (`fuzz_<name>`).
    pub f: fn(&mut Fuzz<'_>),
}

/// Runs the registered fuzzer `name` on `data`, as Skia's `fuzz -n <name>` does. `None` if no
/// fuzzer has that name.
#[must_use]
pub fn run(name: &str, data: &[u8]) -> Option<()> {
    let fuzzable = fuzz::FUZZERS.iter().find(|f| f.name == name)?;
    (fuzzable.f)(&mut Fuzz::new(data));
    Some(())
}

/// `DEF_FUZZ(name, f) { … }`: defines `fuzz_<name>` (spelled out as the second argument) and the
/// corpus replay test `<name>`.
// Port of: fuzz/Fuzz.h#L158-L162 (chrome/m156)
#[macro_export]
macro_rules! def_fuzz {
    ($name:ident, $fn_name:ident, |$fuzz:ident| $body:block) => {
        #[doc = concat!("`DEF_FUZZ(", stringify!($name), ", fuzz)`.")]
        #[allow(non_snake_case)] // the C++ `fuzz_##name`
        pub fn $fn_name($fuzz: &mut $crate::Fuzz<'_>) $body

        #[cfg(test)]
        #[test]
        #[allow(non_snake_case)] // the manifest name, verbatim
        fn $name() {
            $crate::replay::replay(stringify!($name), $fn_name);
        }
    };
}
