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

use std::fmt;

pub mod resources;
pub mod tmp_dir;
pub mod tools;
pub mod unit;

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
            let mut reporter = $crate::Reporter::new(stringify!($name));
            {
                let $reporter: &mut $crate::Reporter = &mut reporter;
                $body
            }
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
            let mut reporter = $crate::Reporter::new(stringify!($name));
            for sel in $crate::tier_selections() {
                let _guard = ::skia_rust_simd::testing::force_tier(sel)
                    .expect("tier_selections() returns checked selections");
                reporter.set_context(Some(sel.to_string()));
                {
                    let $reporter: &mut $crate::Reporter = &mut reporter;
                    $body
                }
            }
            reporter.set_context(None);
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
