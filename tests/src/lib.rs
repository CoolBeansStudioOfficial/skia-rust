//! 1:1 ports of Skia's unit tests, plus the harness they run on.
//!
//! # Layout
//! A Skia test file maps to a module path by snake-casing each path component
//! (`cargo xtask inventory module-path <id>` prints it):
//!
//! | Skia | Rust test path |
//! |---|---|
//! | `tests/PointTest.cpp::Point` | `unit::point_test::Point` |
//! | `tests/graphite/RectTest.cpp::X` | `unit::graphite::rect_test::X` |
//! | `modules/svg/tests/Text.cpp::X` | `modules::svg::text::X` |
//!
//! Test functions keep Skia's name verbatim, so `cargo xtask inventory verify` can map
//! every Rust test back to its manifest entry.
//!
//! # Harness
//! [`def_test!`] mirrors `DEF_TEST`, [`reporter_assert!`] mirrors `REPORTER_ASSERT`,
//! [`errorf!`] mirrors `ERRORF` and [`infof!`] mirrors `INFOF`. Like Skia's `Reporter`,
//! failures are collected and the test keeps running; it fails at the end if any
//! assertion failed.
//!
//! # Serial tests
//! Skia's `DEF_SERIAL_TEST` tests run alone: DM runs them after all the parallel tests. Every
//! test macro here takes a read guard on one process-wide lock ([`test_read_guard`]), and
//! [`def_serial_test!`] takes the write guard ([`test_write_guard`]), so a serial test runs while
//! no other test runs. See `docs/PORTING.md` section 6.

use std::fmt;
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub mod codec_priv;
pub mod gainmap_test_common;
#[cfg(test)]
pub mod modules;
pub mod resources;
pub mod tmp_dir;
pub mod tools;
#[cfg(test)]
pub mod unit;

/// The process-wide test lock. Parallel tests hold a read guard, serial tests a write guard.
static TEST_LOCK: RwLock<()> = RwLock::new(());

/// The read guard that every test macro (all but [`def_serial_test!`]) holds for its whole body.
///
/// A poisoned lock is recovered: a failed test panics while holding its guard, and that must not
/// fail the tests after it. The lock guards no data, so there is no inconsistent state to fear.
#[doc(hidden)]
pub fn test_read_guard() -> RwLockReadGuard<'static, ()> {
    TEST_LOCK.read().unwrap_or_else(PoisonError::into_inner)
}

/// The write guard that [`def_serial_test!`] holds: no other test runs while it is held.
/// Poisoning is recovered as in [`test_read_guard`].
#[doc(hidden)]
pub fn test_write_guard() -> RwLockWriteGuard<'static, ()> {
    TEST_LOCK.write().unwrap_or_else(PoisonError::into_inner)
}

/// One recorded failure: where it happened, the failed condition and the message.
#[derive(Debug, Clone)]
pub struct Failure {
    pub file: &'static str,
    pub line: u32,
    pub condition: &'static str,
    pub message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)?;
        if !self.condition.is_empty() {
            write!(f, " {}", self.condition)?;
        }
        if !self.message.is_empty() {
            write!(f, ": {}", self.message)?;
        }
        Ok(())
    }
}

/// Port of `skiatest::Reporter`: collects failures for one test.
#[doc(alias = "skiatest::Reporter")]
#[derive(Debug)]
pub struct Reporter {
    name: &'static str,
    failures: Vec<Failure>,
    test_count: usize,
    /// Prefixed to the message of every failure (the CPU tier of [`def_tier_test!`]).
    context: Option<String>,
}

impl Reporter {
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            failures: Vec::new(),
            test_count: 0,
            context: None,
        }
    }

    /// Port of `Reporter::reportFailed`. Use [`reporter_assert!`] or [`errorf!`].
    pub fn report_failed(&mut self, mut failure: Failure) {
        if let Some(context) = &self.context {
            failure.message = format!("[{context}] {}", failure.message);
        }
        self.failures.push(failure);
    }

    /// Sets the text prefixed to later failures' messages (`None` for none).
    pub fn set_context(&mut self, context: Option<String>) {
        self.context = context;
    }

    /// Port of `Reporter::bumpTestCount`.
    pub fn bump_test_count(&mut self) {
        self.test_count += 1;
    }

    /// Port of `Reporter::allowExtendedTest`: false, as in Skia's default (no `--extendedTest`).
    #[must_use]
    pub fn allow_extended_test(&self) -> bool {
        false
    }

    /// Port of `Reporter::verbose`.
    #[must_use]
    pub fn verbose(&self) -> bool {
        std::env::var_os("SKIA_RUST_TEST_VERBOSE").is_some()
    }

    /// The context prefixed to every failure (`skiatest::ReporterContext`).
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Failures recorded so far.
    #[must_use]
    pub fn failures(&self) -> &[Failure] {
        &self.failures
    }

    /// Ends the test: panics with every recorded failure, if there were any.
    ///
    /// # Panics
    /// If any assertion failed.
    pub fn finish(self) {
        if self.failures.is_empty() {
            return;
        }
        let mut msg = format!("{}: {} failure(s)", self.name, self.failures.len());
        for f in &self.failures {
            msg.push_str("\n  ");
            msg.push_str(&f.to_string());
        }
        panic!("{msg}");
    }
}

/// Port of `DEF_TEST(name, reporter)`.
///
/// The test runs in parallel with the other parallel tests: it holds a read guard on the test lock
/// (see the module docs), so it never overlaps a [`def_serial_test!`] test. Use
/// [`def_serial_test!`] for a `DEF_SERIAL_TEST`.
///
/// ```ignore
/// def_test!(Point, |reporter| {
///     reporter_assert!(reporter, 1 + 1 == 2);
/// });
///
/// // A test that doesn't pass yet: manifest status `failing`, plus a note.
/// def_test!(#[ignore = "see notes/PathTest-Path_arcTo.md"] Path_arcTo, |reporter| { ... });
/// ```
#[macro_export]
macro_rules! def_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            // The body runs as a closure, so an early `return` (after a failed assertion) ends only
            // the body. Without it the failures recorded so far would never reach `finish()`.
            let run = |$reporter: &mut $crate::Reporter| $body;
            run(&mut reporter);
            reporter.finish();
        }
    };
}

/// Port of `DEF_SERIAL_TEST(name, reporter)`: a [`def_test!`] that runs alone. It holds the write
/// guard on the test lock, so no parallel test runs while it does, and every parallel test runs
/// before or after it. Use it for a test that changes process-wide state that other tests read,
/// such as the codec registry (`SkCodecs::Register`).
///
/// The name, the body and the failure reporting are the same as [`def_test!`]'s.
///
/// ```ignore
/// def_serial_test!(Ico_usesRegisteredPngDecoder, |reporter| {
///     /* registers a decoder, then checks what the codec made */
/// });
/// ```
#[macro_export]
macro_rules! def_serial_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_write_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            // A closure, as in `def_test!`: an early `return` ends only the body.
            let run = |$reporter: &mut $crate::Reporter| $body;
            run(&mut reporter);
            reporter.finish();
        }
    };
}

/// Port of `DEF_GRAPHITE_TEST_FOR_ALL_CONTEXTS` / `DEF_GRAPHITE_TEST_FOR_RENDERING_CONTEXTS` for
/// the tests that read pixels back, so need a real adapter (lavapipe, WARP or Metal;
/// `docs/design/gpu.md` section 7). The body runs once, on
/// [`tools::graphite_test_context::real_context`].
///
/// Without the `skia_rust_adapter_tests` cfg the test is `#[ignore]`d, because a machine without
/// an adapter cannot run it, and an ignored test cannot be mistaken for a passing one by
/// `cargo xtask inventory verify`. The Linux x64 CI jobs set the cfg (and
/// `SKIA_RUST_REQUIRE_ADAPTER`) so lavapipe runs them. Elsewhere, run them with
/// `RUSTFLAGS="--cfg skia_rust_adapter_tests" cargo test -p skia-rust-tests --lib` on a machine
/// with an adapter. Without one the test says so and returns, unless `SKIA_RUST_REQUIRE_ADAPTER`
/// is set.
///
/// ```ignore
/// def_graphite_adapter_test!(ImageShaderTest, |reporter, context| {
///     /* … */
/// });
/// ```
#[macro_export]
macro_rules! def_graphite_adapter_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident, $context:ident| $body:block) => {
        #[test]
        #[cfg_attr(
            not(skia_rust_adapter_tests),
            ignore = "needs a real adapter in CI (lavapipe job)"
        )]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            if let Some((context_name, mut context)) =
                $crate::tools::graphite_test_context::real_context()
            {
                reporter.set_context(Some(context_name));
                let run = |$reporter: &mut $crate::Reporter,
                           $context: &mut ::skia_rust_gpu::graphite::wgpu::WgpuContext| $body;
                run(&mut reporter, &mut *context);
                reporter.set_context(None);
            }
            reporter.finish();
        }
    };
}

/// [`def_graphite_adapter_test!`] for a test whose context is made with options
/// (`DEF_CONDITIONAL_GRAPHITE_TEST_FOR_CONTEXTS`'s options-setting function): `|options| { ... }`
/// sets them on the defaults, and the body runs once on a real adapter made with them.
///
/// ```ignore
/// def_graphite_adapter_test_with_options!(
///     DirectMaskLimitTest_Graphite,
///     |options| { options.min_distance_field_font_size = 384.0; },
///     |reporter, context| { /* ... */ }
/// );
/// ```
#[macro_export]
macro_rules! def_graphite_adapter_test_with_options {
    ($(#[$attr:meta])* $name:ident, |$options:ident| $set_options:block,
     |$reporter:ident, $context:ident| $body:block) => {
        #[test]
        #[cfg_attr(
            not(skia_rust_adapter_tests),
            ignore = "needs a real adapter in CI (lavapipe job)"
        )]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut context_options =
                ::skia_rust_gpu::graphite::context_options::ContextOptions::default();
            {
                let $options = &mut context_options;
                $set_options
            }
            let mut reporter = $crate::Reporter::new(stringify!($name));
            if let Some((context_name, mut context)) =
                $crate::tools::graphite_test_context::real_context_with_options(&context_options)
            {
                reporter.set_context(Some(context_name));
                let run = |$reporter: &mut $crate::Reporter,
                           $context: &mut ::skia_rust_gpu::graphite::wgpu::WgpuContext| $body;
                run(&mut reporter, &mut *context);
                reporter.set_context(None);
            }
            reporter.finish();
        }
    };
}

/// Port of `DEF_GRAPHITE_TEST_FOR_ALL_CONTEXTS(name, reporter, context, ...)`: runs the body
/// once per context of [`tools::graphite_test_context::all_contexts`], with the context's name
/// prefixed to each failure.
///
/// Port of `DEF_GRAPHITE_TEST_FOR_DAWN_CONTEXT(name, reporter, context, testContext)`: the body
/// runs once per Dawn context of [`all_contexts`](tools::graphite_test_context::all_contexts)
/// (`IsDawnContextType`); the noop device is skipped.
///
/// ```ignore
/// def_graphite_test_for_dawn_context!(DawnOnlyTest, |reporter, context| { /* ... */ });
/// ```
#[macro_export]
macro_rules! def_graphite_test_for_dawn_context {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident, $context:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for (context_name, mut context) in $crate::tools::graphite_test_context::all_contexts()
                .into_iter()
                .filter(|(context_name, _)| {
                    $crate::tools::graphite_test_context::is_dawn_context(context_name)
                })
            {
                reporter.set_context(Some(context_name));
                // A closure, as in `def_test!`: an early `return` ends this context only.
                let run = |$reporter: &mut $crate::Reporter,
                           $context: &mut ::skia_rust_gpu::graphite::wgpu::WgpuContext| $body;
                run(&mut reporter, &mut context);
            }
            reporter.set_context(None);
            reporter.finish();
        }
    };
}

/// ```ignore
/// def_graphite_test_for_all_contexts!(BackendTextureTest, |reporter, context| {
///     reporter_assert!(reporter, context.backend() == BackendApi::Dawn);
/// });
/// ```
#[macro_export]
macro_rules! def_graphite_test_for_all_contexts {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident, $context:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for (context_name, mut context) in $crate::tools::graphite_test_context::all_contexts() {
                reporter.set_context(Some(context_name));
                // A closure, as in `def_test!`: an early `return` ends this context only.
                let run = |$reporter: &mut $crate::Reporter,
                           $context: &mut ::skia_rust_gpu::graphite::wgpu::WgpuContext| $body;
                run(&mut reporter, &mut context);
            }
            reporter.set_context(None);
            reporter.finish();
        }
    };
}

/// Port of `DEF_CONDITIONAL_GRAPHITE_TEST_FOR_CONTEXTS(name, filter, reporter, context,
/// testContext, options, fn, condition, ...)` for the tests whose contexts are made with
/// options: `|options| { … }` is the options-setting function (`fn`), the body runs once per
/// context made with those options. The context filter and the condition are always true here.
///
/// ```ignore
/// def_graphite_test_for_contexts_with_options!(
///     CacheBudgetTest,
///     |options| { options.gpu_budget_in_bytes = 1234567; },
///     |reporter, context| { /* … */ }
/// );
/// ```
#[macro_export]
macro_rules! def_graphite_test_for_contexts_with_options {
    ($(#[$attr:meta])* $name:ident, |$options:ident| $set_options:block,
     |$reporter:ident, $context:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut context_options = ::skia_rust_gpu::graphite::context_options::ContextOptions::default();
            {
                let $options = &mut context_options;
                $set_options
            }
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for (context_name, mut context) in
                $crate::tools::graphite_test_context::all_contexts_with_options(&context_options)
            {
                reporter.set_context(Some(context_name));
                // A closure, as in `def_test!`: an early `return` ends this context only.
                let run = |$reporter: &mut $crate::Reporter,
                           $context: &mut ::skia_rust_gpu::graphite::wgpu::WgpuContext| $body;
                run(&mut reporter, &mut context);
            }
            reporter.set_context(None);
            reporter.finish();
        }
    };
}

/// Every CPU tier (`Tier::ALL` order), as `skia_rust_simd::testing::oracle_selection` picks it:
/// natively where the CPU has the tier's instructions and the oracle host's estimates, else by
/// the tier's model (with the oracle host's `AmdZen4` estimates for the x86 tiers, the
/// architectural `Arm` estimates for `Neon`).
#[must_use]
pub fn tier_selections() -> Vec<skia_rust_simd::Selection> {
    use skia_rust_simd::Tier;
    Tier::ALL
        .into_iter()
        .map(skia_rust_simd::testing::oracle_selection)
        .collect()
}

/// [`def_test!`] for a test whose results depend on the CPU tier (it runs raster pipelines or
/// `SkOpts` kernels): runs the body once per [`tier_selections`] entry under
/// `skia_rust_simd::testing::force_tier`, with the tier prefixed to each failure.
#[macro_export]
macro_rules! def_tier_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for sel in $crate::tier_selections() {
                let _guard = ::skia_rust_simd::testing::force_tier(sel)
                    .expect("tier_selections() returns checked selections");
                reporter.set_context(Some(sel.to_string()));
                // A closure, as in `def_test!`: an early `return` ends this tier only.
                let run = |$reporter: &mut $crate::Reporter| $body;
                run(&mut reporter);
            }
            reporter.set_context(None);
            reporter.finish();
        }
    };
}

/// [`def_test!`] for a test that makes typefaces from `ToolUtils::TestFontMgr()` (directly, or
/// through `CreateTypefaceFromResource`): runs the body under each [`FontConfig`], the portable
/// configuration and the native Fontations one, as Skia's default and `NativeFonts_Fontations`
/// bots do. A test passes only if both runs pass, so a skipped branch cannot pass hollowly
/// (docs/design/text.md §8). Each failure is prefixed with its configuration.
#[macro_export]
macro_rules! def_font_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for config in [
                ::skia_rust_tools::font_tool_utils::FontConfig::Portable,
                ::skia_rust_tools::font_tool_utils::FontConfig::NativeFontations,
            ] {
                reporter.set_context(Some(format!("{config:?}")));
                ::skia_rust_tools::font_tool_utils::with_font_config(config, || {
                    let $reporter: &mut $crate::Reporter = &mut reporter;
                    $body
                });
            }
            reporter.set_context(None);
            reporter.finish();
        }
    };
}

/// [`def_test!`] for a paragraph test (`UNIX_ONLY_TEST` in `SkParagraphTest.cpp`): runs the body
/// under [`FontConfig::NativeFontations`](::skia_rust_tools::font_tool_utils::FontConfig), the
/// configuration that loads the paragraph fonts (`ResourceFontCollection` makes its typefaces
/// with `ToolUtils::TestFontMgr()->makeFromStream`, which the portable manager cannot do, so
/// Skia's bots run these tests with `--nativeFonts`; docs/design/modules.md §4). Under the
/// portable configuration the C++ tests return early, and there is nothing to run here.
///
/// `UNIX_ONLY_TEST` is `DEF_TEST_DISABLED` unless `SK_BUILD_FOR_UNIX` (neither Windows, macOS,
/// iOS nor Android), and so is this: on those hosts the body does not run.
#[macro_export]
macro_rules! def_paragraph_test {
    ($(#[$attr:meta])* $name:ident, |$reporter:ident| $body:block) => {
        #[test]
        $(#[$attr])*
        #[allow(non_snake_case)]
        fn $name() {
            let _test_guard = $crate::test_read_guard();
            let mut reporter = $crate::Reporter::new(stringify!($name));
            if cfg!(all(
                unix,
                not(target_vendor = "apple"),
                not(target_os = "android")
            )) {
                reporter.set_context(Some(format!(
                    "{:?}",
                    ::skia_rust_tools::font_tool_utils::FontConfig::NativeFontations
                )));
                ::skia_rust_tools::font_tool_utils::with_font_config(
                    ::skia_rust_tools::font_tool_utils::FontConfig::NativeFontations,
                    || {
                        // A closure, as in `def_test!`: an early `return` ends only the body.
                        let run = |$reporter: &mut $crate::Reporter| $body;
                        run(&mut reporter);
                    },
                );
                reporter.set_context(None);
            }
            reporter.finish();
        }
    };
}

/// Port of `REPORTER_ASSERT(r, cond, ...)`. Records a failure and keeps going.
#[macro_export]
macro_rules! reporter_assert {
    ($r:expr, $cond:expr $(,)?) => {
        if !($cond) {
            $r.report_failed($crate::Failure {
                file: file!(),
                line: line!(),
                condition: stringify!($cond),
                message: String::new(),
            });
        }
    };
    ($r:expr, $cond:expr, $($fmt:tt)+) => {
        if !($cond) {
            $r.report_failed($crate::Failure {
                file: file!(),
                line: line!(),
                condition: stringify!($cond),
                message: format!($($fmt)+),
            });
        }
    };
}

/// Port of `ERRORF(r, ...)`. Records a failure and keeps going.
#[macro_export]
macro_rules! errorf {
    ($r:expr, $($fmt:tt)+) => {
        $r.report_failed($crate::Failure {
            file: file!(),
            line: line!(),
            condition: "",
            message: format!($($fmt)+),
        })
    };
}

/// Port of `INFOF(r, ...)`: prints only when the reporter is verbose.
#[macro_export]
macro_rules! infof {
    ($r:expr, $($fmt:tt)+) => {
        if $r.verbose() {
            eprint!($($fmt)+);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    def_test!(
        #[ignore = "demonstrates attributes on def_test!"]
        HarnessIgnoredExample,
        |reporter| {
            errorf!(reporter, "never runs");
        }
    );

    #[test]
    fn reporter_collects_failures_and_keeps_going() {
        let mut r = Reporter::new("t");
        let r = &mut r;
        reporter_assert!(r, 1 + 1 == 3);
        reporter_assert!(r, true);
        reporter_assert!(r, 2 < 1, "value {}", 7);
        errorf!(r, "boom {}", 1);
        assert_eq!(r.failures().len(), 3);
        assert_eq!(r.failures()[0].condition, "1 + 1 == 3");
        assert_eq!(r.failures()[1].message, "value 7");
        assert_eq!(r.failures()[2].message, "boom 1");
    }

    #[test]
    #[should_panic(expected = "t: 1 failure(s)")]
    fn finish_panics_on_failure() {
        let mut r = Reporter::new("t");
        errorf!(r, "x");
        r.finish();
    }
}
