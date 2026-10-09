// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/Benchmark.h, bench/Benchmark.cpp

//! 1:1 ports of Skia's benchmarks (`bench/*.cpp`) and the harness that runs them: a port of
//! nanobench's CPU path (design `docs/design/bench.md`).
//!
//! # Writing a bench
//! A bench file `bench/<File>.cpp` is ported to `src/bench/<file_snake>.rs`
//! (`bench/MathBench.cpp` → `bench::math_bench`). A class `class FooBench : public Benchmark`
//! implements the [`Benchmark`] trait, and each registration is a [`def_bench!`]:
//!
//! | Skia | skia-rust |
//! |---|---|
//! | `DEF_BENCH(return new FooBench();)` | [`def_bench!`]`(foo_bench = "FooBench()", FooBench::new());` |
//! | `DEF_BENCH(return new FooBench(1, true);)` | [`def_bench!`]`(foo_bench_1_true = "FooBench(1, true)", FooBench::new(1, true));` |
//! | a registration that creates several benchmarks | [`def_bench_set!`] |
//!
//! The name string is copied verbatim from the manifest id after `::`. Every registration both
//! adds the benchmark to the [registry](registry) (what `cargo xtask inventory verify`
//! enumerates) and defines a `#[test]` that runs the smoke sequence of design §3.1 criterion C2
//! ([`smoke::run_smoke_test`]).
//!
//! # Layout
//! - [`nanobench`]: the runner, a port of the CPU path of `bench/nanobench.cpp`.
//! - [`smoke`]: criterion C2.
//! - [`bench`](mod@bench): the ported benchmarks.

pub mod bench;
pub mod nanobench;
pub mod smoke;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::paint::Paint;
use skia_rust_core::size::ISize;

/// What bench ports import: `use crate::prelude::*;`.
pub mod prelude {
    pub use crate::{Backend, Benchmark};
    pub use skia_rust_core::canvas::Canvas;
    pub use skia_rust_core::paint::Paint;
    pub use skia_rust_core::size::ISize;
}

/// `Benchmark::Backend`. Ganesh and HWUI are dropped (docs/design/bench.md §2.2).
// Port of: bench/Benchmark.h#L51-L58 (chrome/m156)
#[doc(alias = "Benchmark::Backend")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    #[doc(alias = "kNonRendering")]
    NonRendering,
    #[doc(alias = "kRaster")]
    Raster,
    #[doc(alias = "kGraphite")]
    Graphite,
    #[doc(alias = "kPDF")]
    Pdf,
}

/// `Benchmark`: what a benchmark overrides. The non-virtual driver (`getName`, `delayedSetup`,
/// `perCanvasPreDraw`, `preDraw`, `postDraw`, `perCanvasPostDraw`, `draw`) is the methods of the
/// same names in [`nanobench`]; the trait carries the virtual interface.
///
/// The canvas is `None` for [`Backend::NonRendering`] (nanobench passes null), so a bench that
/// dereferences it unconditionally in C++ uses `.expect()` with the C++ precondition.
///
/// GPU-only hooks (`modifyGrContextOptions`, `getGpuStats`, `getDMSAAStats`) and the
/// internal-frames pair (`submitsInternalFrames`, `onDrawFrame`, MSKP only) are not ported.
// Port of: bench/Benchmark.h#L42-L135 (chrome/m156)
pub trait Benchmark {
    /// `onGetName()`.
    #[doc(alias = "onGetName")]
    fn name(&self) -> String;

    /// `onGetUniqueName()`: the result name; defaults to [`name`](Benchmark::name).
    #[doc(alias = "onGetUniqueName")]
    fn unique_name(&self) -> String {
        self.name()
    }

    /// `onGetSize()`: the canvas size.
    // Port of: bench/Benchmark.cpp#L70-L72 (chrome/m156)
    #[doc(alias = "onGetSize")]
    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    /// `isSuitableFor()`: default is every rendering backend.
    // Port of: bench/Benchmark.h#L60-L62 (chrome/m156)
    #[doc(alias = "isSuitableFor")]
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend != Backend::NonRendering
    }

    /// `shouldLoop()`: `false` runs the bench with one loop, whatever the calibration says.
    #[doc(alias = "shouldLoop")]
    fn should_loop(&self) -> bool {
        true
    }

    /// `getUnits()` (`setUnits(n)` sets the field this returns): samples are divided by it.
    #[doc(alias = "getUnits")]
    fn units(&self) -> i32 {
        1
    }

    /// `onDelayedSetup()`: once per benchmark, before any config, outside the timer.
    #[doc(alias = "onDelayedSetup")]
    fn on_delayed_setup(&mut self) {}

    /// `onPerCanvasPreDraw()`: once per (benchmark, config), outside the timer.
    #[doc(alias = "onPerCanvasPreDraw")]
    fn on_per_canvas_pre_draw(&mut self, _canvas: Option<&Canvas>) {}

    /// `onPerCanvasPostDraw()`.
    #[doc(alias = "onPerCanvasPostDraw")]
    fn on_per_canvas_post_draw(&mut self, _canvas: Option<&Canvas>) {}

    /// `onPreDraw()`: before each timed `draw`, outside the timer.
    #[doc(alias = "onPreDraw")]
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {}

    /// `onPostDraw()`: after each timed `draw`, outside the timer.
    #[doc(alias = "onPostDraw")]
    fn on_post_draw(&mut self, _canvas: Option<&Canvas>) {}

    /// `onDraw(int loops, SkCanvas*)`: the timed body. It loops `loops` times itself; a negative
    /// `loops` means "forever" (`SK_MaxS32`, see `detect_forever_loops`).
    #[doc(alias = "onDraw")]
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>);

    /// `setupPaint()`: the default turns anti-aliasing on.
    // Port of: bench/Benchmark.cpp#L66-L68 (chrome/m156)
    #[doc(alias = "setupPaint")]
    fn setup_paint(&self, paint: &mut Paint) {
        paint.set_anti_alias(true);
    }
}

impl std::fmt::Debug for dyn Benchmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Benchmark({})", self.unique_name())
    }
}

/// `Benchmark::draw()`: `onDraw` inside an `SkAutoCanvasRestore` that saves now. A `None`
/// canvas (nonrendering) is fine.
// Port of: bench/Benchmark.cpp#L50-L60 (chrome/m156)
pub fn draw(bench: &mut dyn Benchmark, loops: i32, canvas: Option<&Canvas>) {
    // SkAutoCanvasRestore ar(canvas, true /*save now*/);
    let save_count = canvas.map(|c| {
        let count = c.save_count();
        c.save();
        count
    });
    bench.on_draw(loops, canvas);
    if let (Some(c), Some(count)) = (canvas, save_count) {
        c.restore_to_count(count);
    }
}

/// The benchmark registry (`BenchRegistry`).
pub mod registry {
    use super::Benchmark;

    /// One registered site: what a `def_bench!` / `def_bench_set!` registers.
    #[derive(Debug)]
    pub struct BenchRegistration {
        /// `module_path!()` of the registration's own (macro-generated) module:
        /// `skia_rust_bench::bench::<file>::<test name>`.
        pub module_path: &'static str,
        /// The manifest registration name: the text after `::` in the manifest id, verbatim
        /// (`NoOpMathBench()`, `Floor2IntBench(true)`, …).
        pub name: &'static str,
        /// The benchmarks this site creates: one for `DEF_BENCH`, several for wrapper macros
        /// (`COMPILER_BENCH`, `ADD_BENCH_FAMILY`). A fresh set per call.
        pub factory: fn() -> Vec<Box<dyn Benchmark>>,
        /// Runtime benchmarks of this site that the port omits on purpose (they need an
        /// out-of-scope backend), with the reason in a comment at the registration.
        pub omitted: &'static [&'static str],
    }

    inventory::collect!(BenchRegistration);

    impl BenchRegistration {
        /// The key `cargo xtask inventory verify` maps manifest ids to:
        /// `bench/MathBench.cpp::Floor2IntBench(true)` ↔
        /// `bench::math_bench::Floor2IntBench(true)`.
        #[must_use]
        pub fn key(&self) -> String {
            let path = self
                .module_path
                .strip_prefix("skia_rust_bench::")
                .unwrap_or(self.module_path);
            let file = path.rsplit_once("::").map_or(path, |(file, _)| file);
            format!("{file}::{}", self.name)
        }
    }

    /// Every registered site, sorted by key.
    #[must_use]
    pub fn all() -> Vec<&'static BenchRegistration> {
        let mut all: Vec<_> = inventory::iter::<BenchRegistration>.into_iter().collect();
        all.sort_by_key(|r| r.key());
        all
    }
}

/// Port of `DEF_BENCH(return new FooBench(...);)`.
///
/// `def_bench!(foo_bench = "FooBench(1)", FooBench::new(1))` registers the benchmark under the
/// manifest name `FooBench(1)` (copied verbatim from the manifest id) and defines the smoke
/// test `foo_bench`. Attributes (e.g. `#[ignore = "see notes/…"]`) go first and apply to the
/// test.
// Port of: bench/Benchmark.h#L23-L26 (chrome/m156)
#[macro_export]
macro_rules! def_bench {
    ($(#[$attr:meta])* $test:ident = $name:expr, $make:expr $(,)?) => {
        $crate::def_bench_set!(
            $(#[$attr])* $test = $name,
            ::std::vec![::std::boxed::Box::new($make) as ::std::boxed::Box<dyn $crate::Benchmark>]
        );
    };
}

/// A registration that creates several benchmarks (`COMPILER_BENCH`, `ADD_BENCH_FAMILY`):
/// `def_bench_set!(test = "name", vec![Box::new(…), …])`, optionally followed by
/// `; omitted = ["unique_name", …]` for runtime benchmarks the port leaves out.
#[macro_export]
macro_rules! def_bench_set {
    ($(#[$attr:meta])* $test:ident = $name:expr, $make:expr $(; omitted = $omitted:expr)? $(,)?) => {
        #[allow(non_snake_case)]
        #[doc(hidden)]
        mod $test {
            #[allow(unused_imports)]
            use super::*;
            pub(super) const REGISTRATION: $crate::registry::BenchRegistration =
                $crate::registry::BenchRegistration {
                    module_path: module_path!(),
                    name: $name,
                    factory: {
                        fn factory() -> ::std::vec::Vec<::std::boxed::Box<dyn $crate::Benchmark>> {
                            $make
                        }
                        factory
                    },
                    omitted: {
                        #[allow(unused_mut, unused_assignments)]
                        let mut omitted: &'static [&'static str] = &[];
                        $(omitted = &$omitted;)?
                        omitted
                    },
                };
        }
        $crate::__inventory::submit!($test::REGISTRATION);
        #[test]
        $(#[$attr])*
        fn $test() {
            $crate::smoke::run_smoke_test(&$test::REGISTRATION);
        }
    };
}

#[doc(hidden)]
pub use inventory as __inventory;
