// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The per-tier loop (design §4.4, §4.6): render every GM once per [`Tier`] under
//! `force_tier`, for every config, and compare the SHA-256 of the bytes with every oracle tier
//! that records that tier's behaviour (`Tier::oracle_tiers()`).
//!
//! # How each tier runs on this host ([`plan`])
//! - `Scalar`: natively everywhere.
//! - x86 tiers: `Native` when the host runs the tier and its `rcp`/`rsqrt` estimates match the
//!   oracle host's (`Fingerprints::matches_for(&AMD_ZEN4, tier)`); otherwise the tier's model
//!   with the oracle host's estimate tables (`Model(AmdZen4)`, §4.6 policies 3–4; for `Ml4` that
//!   is the `vrcp14ps`/`vrsqrt14ps` model, exact on all inputs, so hosts without AVX-512 check
//!   `Ml4` too).
//! - `Neon`: natively on arm64, else `Model(Arm)`.
//!
//! An oracle tier without goldens (`wasm-simd128`, `arm64-neon` today) is not compared. Proxy
//! oracle tiers ([`PROXY_ORACLE_TIERS`]: `cpu-x64-scalar` stands in for the wasm oracle, §4.5)
//! are compared and reported, but never make a GM pass or fail.
//!
//! # Configs
//! `8888` is `kN32_SkColorType`: BGRA on Windows (where the default goldens were made), RGBA
//! elsewhere. Bytes are never swizzled: on a host whose N32 is RGBA, `8888` is compared with the
//! RGBA oracle variants (`Tier::oracle_tiers_rgba()`, built with `SK_R32_SHIFT=0`;
//! [`uses_rgba_goldens`]). A tier with no such golden is **not checkable** for `8888` there.
//! `565` and `f16` do not depend on byte order and always use the default tiers.
//!
//! Their *bytes* do not, but which blitter draws an offscreen surface Skia makes with an explicit
//! `kRGBA_8888` (the picture shader's tile) does: it is N32 only in an RGBA build. So each render
//! also forces the N32 of the oracle build it is compared with ([`oracle_n32`],
//! `skia_rust_raster::oracle_n32`), as it forces the CPU tier.
//!
//! # Verdicts
//! A GM is [`Verdict::Passing`] only when every config matches on every non-proxy oracle tier
//! with goldens and every such tier was checkable here. Any mismatch (or a draw failure, a panic,
//! a skip the oracle did not make, an output the oracle does not have) is
//! [`Verdict::Failing`]. Otherwise — some tier could not be checked here — it is
//! [`Verdict::NotCheckable`], which never counts as passing.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Serialize;
use skia_rust_raster::oracle_n32::testing::force_oracle_n32;
use skia_rust_simd::estimates::{AMD_ZEN4, Fingerprints};
use skia_rust_simd::testing::force_tier;
use skia_rust_simd::{Estimates, Selection, Tier};

use crate::diff;
use crate::goldens::{GoldenStore, sha256_hex};
use crate::registry::GmRegistration;
use crate::sink::{Config, GmSrc, RasterSink, Status, oracle_n32, packed_bytes, uses_rgba_goldens};
use skia_rust_core::color_type::ColorType;

/// Oracle tiers that only approximate the tier they stand for (design §4.5).
pub const PROXY_ORACLE_TIERS: &[&str] = &["cpu-x64-scalar", "cpu-x64-scalar-rgba"];

/// How one [`Tier`] is checked on this host.
#[derive(Clone, Debug)]
pub struct TierPlan {
    pub tier: Tier,
    /// The selection to render with, or why the tier cannot be checked here.
    pub run: Result<Selection, String>,
    /// Oracle tiers mapped to `tier` that have goldens (authoritative ones first, proxies last).
    pub oracle_tiers: Vec<&'static str>,
    /// The same for the RGBA-order oracle variants, which `8888` uses on hosts whose N32 is RGBA.
    pub rgba_oracle_tiers: Vec<&'static str>,
}

impl TierPlan {
    /// Whether any non-proxy oracle tier of this tier has goldens.
    #[must_use]
    pub fn is_authoritative(&self) -> bool {
        self.oracle_tiers.iter().any(|t| !is_proxy(t))
    }

    /// The oracle tiers `config` is compared with on a host whose N32 is `host_n32`.
    #[must_use]
    pub fn oracle_tiers_for(&self, config: Config, host_n32: ColorType) -> &[&'static str] {
        if uses_rgba_goldens(config, host_n32) {
            &self.rgba_oracle_tiers
        } else {
            &self.oracle_tiers
        }
    }
}

fn is_proxy(oracle_tier: &str) -> bool {
    PROXY_ORACLE_TIERS.contains(&oracle_tier)
}

/// The host's estimate fingerprints, measured once per process.
#[must_use]
pub fn host_fingerprints() -> &'static Fingerprints {
    static HOST: OnceLock<Fingerprints> = OnceLock::new();
    HOST.get_or_init(Fingerprints::host)
}

/// The selection policy of the module docs, for a host with `fingerprints`.
///
/// # Errors
/// Why `tier` cannot be checked on this host.
pub fn selection_for(tier: Tier, fingerprints: &Fingerprints) -> Result<Selection, String> {
    let sel = match tier {
        Tier::Neon if !tier.is_native() => Selection::model(tier, Estimates::Arm),
        t if t.is_x86() && !(t.is_native() && fingerprints.matches_for(&AMD_ZEN4, t)) => {
            Selection::model(t, Estimates::AmdZen4)
        }
        t => Selection::native(t),
    };
    sel.check().map_err(|e| e.to_string())
}

/// The plan for every tier against `store`, for a host with `fingerprints`.
#[must_use]
pub fn plan_for(store: &GoldenStore, fingerprints: &Fingerprints) -> Vec<TierPlan> {
    plan_with(store, |tier| selection_for(tier, fingerprints))
}

/// The plan for every tier against `store`, running each tier as `select` says.
pub fn plan_with(
    store: &GoldenStore,
    select: impl Fn(Tier) -> Result<Selection, String>,
) -> Vec<TierPlan> {
    Tier::ALL
        .into_iter()
        .map(|tier| {
            let with_goldens = |names: &'static [&'static str]| {
                let mut v: Vec<&'static str> = names
                    .iter()
                    .copied()
                    .filter(|t| store.has_tier(t))
                    .collect();
                v.sort_by_key(|t| is_proxy(t));
                v
            };
            TierPlan {
                tier,
                run: select(tier),
                oracle_tiers: with_goldens(tier.oracle_tiers()),
                rgba_oracle_tiers: with_goldens(tier.oracle_tiers_rgba()),
            }
        })
        .collect()
}

/// The plan for this host against `store` (measures the host's fingerprints on first use).
#[must_use]
pub fn plan(store: &GoldenStore) -> Vec<TierPlan> {
    plan_for(store, host_fingerprints())
}

/// The outcome of one (config, tier, oracle tier) comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// Our bytes hash to the golden (or we and the oracle both skipped).
    Match,
    /// The hashes differ. `diff` is the diff image, or why none was written.
    Mismatch {
        ours: String,
        golden: String,
        diff: String,
    },
    /// We skipped but the oracle wrote a result.
    SkippedButGolden { msg: String },
    /// We rendered but the oracle has no such result (it skipped, or the GM name differs).
    NoGolden { ours: String },
    /// The GM returned `DrawResult::Fail` (DM: fatal).
    DrawFailed { msg: String },
    /// Rendering panicked.
    Panicked { msg: String },
    /// The tier cannot be checked on this host.
    NotCheckable { reason: String },
}

impl Outcome {
    fn is_failure(&self) -> bool {
        !matches!(self, Outcome::Match | Outcome::NotCheckable { .. })
    }
}

/// One comparison.
#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub config: &'static str,
    pub tier: &'static str,
    /// How the tier ran (`None` if it did not).
    pub selection: Option<String>,
    /// The oracle tier compared against (`None` for outcomes that precede any comparison).
    pub oracle_tier: Option<&'static str>,
    /// Compared against a proxy oracle tier: reported, never decisive.
    pub proxy: bool,
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// A GM's overall result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Passing,
    Failing,
    NotCheckable,
}

/// Everything checked for one GM.
#[derive(Clone, Debug, Serialize)]
pub struct GmReport {
    /// The registry key (`gm::<file>::<registration name>`).
    pub key: String,
    /// The GM's name (`<config>/gm/<name>`).
    pub name: String,
    pub verdict: Verdict,
    pub checks: Vec<Check>,
}

impl fmt::Display for GmReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} ({}): {:?}", self.key, self.name, self.verdict)?;
        for c in &self.checks {
            let against = c.oracle_tier.unwrap_or("-");
            let proxy = if c.proxy { " (proxy)" } else { "" };
            let how = c.selection.as_deref().unwrap_or("not run");
            write!(
                f,
                "  {}/{}: tier {} [{how}] vs {against}{proxy}: ",
                c.config, self.name, c.tier
            )?;
            match &c.outcome {
                Outcome::Match => writeln!(f, "match")?,
                Outcome::Mismatch { ours, golden, diff } => {
                    writeln!(f, "MISMATCH ours {ours}, golden {golden}; diff: {diff}")?;
                }
                Outcome::SkippedButGolden { msg } => {
                    writeln!(f, "SKIPPED ({msg}) but the oracle has a result")?;
                }
                Outcome::NoGolden { ours } => {
                    writeln!(f, "NO GOLDEN (ours {ours}); does the oracle skip it?")?;
                }
                Outcome::DrawFailed { msg } => writeln!(f, "DRAW FAILED: {msg}")?,
                Outcome::Panicked { msg } => writeln!(f, "PANICKED: {msg}")?,
                Outcome::NotCheckable { reason } => writeln!(f, "not checkable: {reason}")?,
            }
        }
        Ok(())
    }
}

/// What [`check_gm`] does besides comparing.
#[derive(Clone, Debug)]
pub struct Options {
    pub configs: Vec<Config>,
    /// Where to write diff images for mismatches (fetching the golden objects if needed), or
    /// `None` for no diffs.
    pub diffs: Option<PathBuf>,
    /// The host's `kN32_SkColorType`: `8888` is compared with the oracle variant of that byte
    /// order ([`uses_rgba_goldens`]); injectable so tests do not depend on the real host.
    pub host_n32: ColorType,
}

impl Default for Options {
    /// All configs, diffs in [`diff::diff_root`].
    fn default() -> Self {
        Self {
            configs: Config::ALL.to_vec(),
            diffs: Some(diff::diff_root()),
            host_n32: ColorType::N32,
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

/// What one render produced.
enum Render {
    Skipped(String),
    Failed(String),
    Panicked(String),
    Pixels { bytes: Vec<u8>, size: (i32, i32) },
}

/// Renders `src` for `config` as the oracle build whose goldens it is compared with: on the CPU
/// tier `sel`, with that build's `kN32_SkColorType` (`oracle_n32`).
fn render(src: GmSrc, config: Config, sel: Selection, oracle_n32: ColorType) -> Render {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _guard = force_tier(sel).map_err(|e| e.to_string())?;
        let _n32 = force_oracle_n32(oracle_n32);
        Ok::<_, String>(RasterSink::new(config).draw(&src))
    }));
    match result {
        Err(payload) => Render::Panicked(panic_message(payload.as_ref())),
        Ok(Err(unsupported)) => Render::Panicked(unsupported),
        Ok(Ok(rendered)) => match rendered.result.status {
            Status::Skip => Render::Skipped(rendered.result.msg),
            Status::Fatal => Render::Failed(rendered.result.msg),
            Status::Ok => {
                let bitmap = rendered.bitmap.expect("an Ok result has pixels");
                Render::Pixels {
                    bytes: packed_bytes(&bitmap),
                    size: (bitmap.width(), bitmap.height()),
                }
            }
        },
    }
}

/// `8888` on a host whose N32 is `host_n32`, for a tier the goldens have no variant of that byte
/// order for.
fn no_variant_check(config: Config, tp: &TierPlan, host_n32: ColorType) -> Check {
    Check {
        config: config.tag(),
        tier: tp.tier.name(),
        selection: None,
        oracle_tier: None,
        proxy: !tp.is_authoritative(),
        outcome: Outcome::NotCheckable {
            reason: format!(
                "this host's 8888 is {host_n32:?} and the goldens have no tier of that byte                  order for {} (needs an RGBA oracle variant, SK_R32_SHIFT=0)",
                tp.tier
            ),
        },
    }
}

/// Renders `src` for every config in `opts` and every tier in `plan`, and compares against
/// `store`.
#[must_use]
pub fn check_gm(
    key: &str,
    src: &GmSrc,
    store: &GoldenStore,
    plan: &[TierPlan],
    opts: &Options,
) -> GmReport {
    let name = src.name();
    let mut checks = Vec::new();
    for &config in &opts.configs {
        let id = config.result_id(&name);
        for tp in plan.iter().filter(|tp| !tp.oracle_tiers.is_empty()) {
            let oracle_tiers = tp.oracle_tiers_for(config, opts.host_n32);
            if oracle_tiers.is_empty() {
                checks.push(no_variant_check(config, tp, opts.host_n32));
                continue;
            }
            let mut push = |selection: Option<Selection>,
                            oracle_tier: Option<&'static str>,
                            outcome: Outcome| {
                checks.push(Check {
                    config: config.tag(),
                    tier: tp.tier.name(),
                    selection: selection.map(|s| s.to_string()),
                    oracle_tier,
                    proxy: oracle_tier.map_or(!tp.is_authoritative(), is_proxy),
                    outcome,
                });
            };
            let sel = match &tp.run {
                Ok(sel) => *sel,
                Err(reason) => {
                    let reason = reason.clone();
                    push(None, None, Outcome::NotCheckable { reason });
                    continue;
                }
            };
            match render(*src, config, sel, oracle_n32(config, opts.host_n32)) {
                Render::Panicked(msg) => push(Some(sel), None, Outcome::Panicked { msg }),
                Render::Failed(msg) => push(Some(sel), None, Outcome::DrawFailed { msg }),
                Render::Skipped(msg) => {
                    for &ot in oracle_tiers {
                        let outcome = if store.golden(ot, &id).is_some() {
                            Outcome::SkippedButGolden { msg: msg.clone() }
                        } else {
                            Outcome::Match
                        };
                        push(Some(sel), Some(ot), outcome);
                    }
                }
                Render::Pixels { bytes, size } => {
                    let ours = sha256_hex(&bytes);
                    for &ot in oracle_tiers {
                        let outcome = match store.golden(ot, &id) {
                            None => Outcome::NoGolden { ours: ours.clone() },
                            Some(golden) if golden == ours => Outcome::Match,
                            Some(golden) => Outcome::Mismatch {
                                ours: ours.clone(),
                                golden: golden.to_owned(),
                                diff: match &opts.diffs {
                                    None => "not requested".to_owned(),
                                    Some(root) => store
                                        .object(golden)
                                        .and_then(|g| {
                                            diff::write_diff(
                                                root, ot, config, &name, size, &bytes, &g,
                                            )
                                        })
                                        .map_or_else(
                                            |e| format!("not written: {e}"),
                                            |p| p.display().to_string(),
                                        ),
                                },
                            },
                        };
                        push(Some(sel), Some(ot), outcome);
                    }
                }
            }
        }
    }
    let decisive = || checks.iter().filter(|c| !c.proxy);
    let verdict = if decisive().any(|c| c.outcome.is_failure()) {
        Verdict::Failing
    } else if decisive().any(|c| c.outcome != Outcome::Match) || decisive().next().is_none() {
        Verdict::NotCheckable
    } else {
        Verdict::Passing
    };
    GmReport {
        key: key.to_owned(),
        name,
        verdict,
        checks,
    }
}

/// The body of every `def_gm!` test: checks the GM against the goldens on every tier.
///
/// # Panics
/// If the goldens cannot be loaded or the GM's verdict is [`Verdict::Failing`]. A
/// [`Verdict::NotCheckable`] GM passes the test (with a warning); `cargo xtask inventory verify`
/// still does not count it as passing.
pub fn run_gm_test(reg: &GmRegistration) {
    let store = GoldenStore::shared().unwrap_or_else(|e| panic!("loading goldens: {e}"));
    let plan = plan(store);
    let report = check_gm(
        &reg.key(),
        &GmSrc::from_registration(reg),
        store,
        &plan,
        &Options::default(),
    );
    match report.verdict {
        Verdict::Passing => {}
        Verdict::NotCheckable => eprintln!("warning: not every tier checkable here\n{report}"),
        Verdict::Failing => panic!("{report}"),
    }
}

#[cfg(test)]
mod tests;
