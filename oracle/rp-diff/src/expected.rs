// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Stored Skia results (`oracle/rp-diff/expected/<tier>.txt`) and the replay check against them.
//!
//! One file per Skia code path ([`Tier::name`]: `scalar`, `sse2`, `sse41`, `ml3`, `ml4`),
//! written by `cargo xtask oracle rp-diff --update` on the oracle host and committed:
//!
//! ```text
//! # comment lines: tier, Skia commit, compiler, host and its estimate fingerprints
//! <case name> <case hash> <output hash>       (both 16 hex digits, FNV-1a 64)
//! ```
//!
//! The case hash ([`Case::hash`]) ties a result to the exact case it came from: a case edited
//! without regenerating the results is reported as stale rather than compared.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use skia_rust_simd::estimates::{AMD_ZEN4, Fingerprints};
use skia_rust_simd::tier::{Backend, Estimates, Selection, Tier};

use crate::case::{Case, fnv1a};
use crate::replay::{build_stages, run_case};

/// The tiers with an oracle (x86 Skia paths); `Neon` has none yet.
pub const ORACLE_TIERS: [Tier; 5] = [Tier::Scalar, Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4];

/// `oracle/rp-diff/expected`.
#[must_use]
pub fn expected_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("expected")
}

/// `expected/<tier>.txt`.
#[must_use]
pub fn expected_path(tier: Tier) -> PathBuf {
    expected_dir().join(format!("{}.txt", tier.name()))
}

/// One stored result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// [`Case::hash`] of the case that produced it.
    pub case_hash: u64,
    /// [`fnv1a`] of Skia's output.
    pub output_hash: u64,
}

/// A tier's stored results.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expected {
    /// The comment lines (without `# `).
    pub header: Vec<String>,
    /// Case name → result.
    pub entries: BTreeMap<String, Entry>,
}

impl Expected {
    /// Parses a results file.
    ///
    /// # Errors
    /// On a malformed line.
    pub fn parse(text: &str) -> Result<Expected, String> {
        let mut e = Expected::default();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim_end();
            if let Some(c) = line.strip_prefix('#') {
                e.header.push(c.trim_start().to_owned());
                continue;
            }
            if line.is_empty() {
                continue;
            }
            let t: Vec<&str> = line.split_whitespace().collect();
            let bad = || {
                format!(
                    "line {}: expected `<name> <case hash> <output hash>`",
                    i + 1
                )
            };
            if t.len() != 3 {
                return Err(bad());
            }
            let case_hash = u64::from_str_radix(t[1], 16).map_err(|_| bad())?;
            let output_hash = u64::from_str_radix(t[2], 16).map_err(|_| bad())?;
            e.entries.insert(
                t[0].to_owned(),
                Entry {
                    case_hash,
                    output_hash,
                },
            );
        }
        Ok(e)
    }

    /// Reads `expected/<tier>.txt`; `None` if it does not exist.
    ///
    /// # Errors
    /// If it exists but cannot be read or parsed.
    pub fn load(tier: Tier) -> Result<Option<Expected>, String> {
        let path = expected_path(tier);
        match std::fs::read_to_string(&path) {
            Ok(text) => Expected::parse(&text)
                .map(Some)
                .map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The file text.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for h in &self.header {
            let _ = writeln!(s, "# {h}");
        }
        for (name, e) in &self.entries {
            let _ = writeln!(s, "{name} {:016x} {:016x}", e.case_hash, e.output_hash);
        }
        s
    }
}

/// The host's estimate fingerprints, computed once.
fn host_fingerprints() -> &'static Fingerprints {
    static FP: OnceLock<Fingerprints> = OnceLock::new();
    FP.get_or_init(Fingerprints::host)
}

/// The selections that replay `tier` against Skia's results on this host (design §4.6):
///
/// - `Scalar`: native (it is its own model);
/// - x86 tiers: `Native` and `Model(Host)` when the host can run them and its estimate
///   fingerprints match the oracle host's (`AMD_ZEN4`) for the tier, and always
///   `Model(AmdZen4)`, which is exact on any host;
/// - `Neon`: `Native` where possible and `Model(Arm)` (no oracle results yet).
#[must_use]
pub fn selections(tier: Tier) -> Vec<Selection> {
    match tier {
        Tier::Scalar => vec![Selection::native(Tier::Scalar)],
        Tier::Neon => [
            Selection::native(Tier::Neon),
            Selection::model(Tier::Neon, Estimates::Arm),
        ]
        .into_iter()
        .filter(|s| s.check().is_ok())
        .collect(),
        t => {
            let mut v = Vec::new();
            let same_estimates = || host_fingerprints().matches_for(&AMD_ZEN4, t);
            for s in [Selection::native(t), Selection::model(t, Estimates::Host)] {
                if s.check().is_ok() && same_estimates() {
                    v.push(s);
                }
            }
            v.push(Selection::model(t, Estimates::AmdZen4));
            v
        }
    }
}

/// A short name for a selection, e.g. `ml3/native`, `sse2/model-amd-zen4`.
#[must_use]
pub fn selection_name(sel: Selection) -> String {
    let backend = match sel.backend {
        Backend::Native => "native",
        Backend::Model(Estimates::Host) => "model-host",
        Backend::Model(Estimates::AmdZen4) => "model-amd-zen4",
        Backend::Model(Estimates::Arm) => "model-arm",
    };
    format!("{}/{backend}", sel.tier.name())
}

/// Whether `sel` runs host arithmetic whose invalid operations (`inf - inf`, `0 * inf`, `sqrt` of
/// a negative) produce a different default NaN than the x86 oracle's. On a host that is not x86
/// that is `0x7FC00000` where x86 gives `0xFFC00000`: `Scalar` is plain Rust arithmetic (on
/// wasm32 the sign is up to the engine), and so are the plain operators (`+ - * /`, `sqrt`) in
/// the x86 models' stage code, which spell out the indefinite only in the modelled instructions
/// (`mad`, `min`, conversions, estimates). Native x86 tiers only run on x86 hosts.
fn host_nan_sign_differs(sel: Selection) -> bool {
    let is_arithmetic_model = sel.tier == Tier::Scalar || matches!(sel.backend, Backend::Model(_));
    is_arithmetic_model && !cfg!(any(target_arch = "x86", target_arch = "x86_64"))
}

/// `out` with every word that is exactly the positive default NaN `0x7FC00000` replaced by x86's
/// indefinite `0xFFC00000`. A lane whose NaN was an input propagated unchanged is changed too, so
/// this is a second chance after an exact comparison failed, never a replacement for it.
fn with_x86_default_nan(out: &[u8]) -> Vec<u8> {
    let mut v = out.to_vec();
    for w in v.as_chunks_mut::<4>().0 {
        if *w == 0x7FC0_0000u32.to_le_bytes() {
            *w = 0xFFC0_0000u32.to_le_bytes();
        }
    }
    v
}

/// Replays `cases` for `tier` on every selection of [`selections`] and checks them against
/// `expected`. Returns one line per problem (empty = every case matched on every selection):
/// a case without a stored result, a stale result (case edited since), a stored result for a
/// case that no longer exists, an output mismatch, or a Rust panic.
#[must_use]
pub fn check(tier: Tier, cases: &[Case], expected: &Expected) -> Vec<String> {
    let mut problems = Vec::new();
    let sels = selections(tier);
    for c in cases {
        let Some(entry) = expected.entries.get(&c.name) else {
            problems.push(format!(
                "{}: no stored Skia result for {} (run `cargo xtask oracle rp-diff --update` on \
                 the oracle host)",
                c.name,
                tier.name()
            ));
            continue;
        };
        if entry.case_hash != c.hash() {
            problems.push(format!(
                "{}: stored result is stale (the case changed); rerun `cargo xtask oracle \
                 rp-diff --update` on the oracle host",
                c.name
            ));
            continue;
        }
        let stages = match build_stages(&c.stages, tier) {
            Ok(s) => s,
            Err(e) => {
                problems.push(format!("{}: {e}", c.name));
                continue;
            }
        };
        if tier == Tier::Scalar && c.scalar_proxy_differs() {
            continue;
        }
        for &sel in &sels {
            match run_case(c, &stages, sel) {
                Ok(out) if fnv1a(&out) == entry.output_hash => {}
                Ok(out)
                    if host_nan_sign_differs(sel)
                        && fnv1a(&with_x86_default_nan(&out)) == entry.output_hash => {}
                Ok(out) => problems.push(format!(
                    "{} on {}: output {:016x}, Skia {:016x}",
                    c.name,
                    selection_name(sel),
                    fnv1a(&out),
                    entry.output_hash
                )),
                Err(e) => problems.push(format!("{} on {}: {e}", c.name, selection_name(sel))),
            }
        }
    }
    let names: std::collections::BTreeSet<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    for name in expected.entries.keys() {
        if !names.contains(name.as_str()) {
            problems.push(format!(
                "{name}: stored for {} but no longer a case (rerun `cargo xtask oracle rp-diff \
                 --update`)",
                tier.name()
            ));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = "# tier sse2\n# skia x\na/b 00000000000000ff 0123456789abcdef\n";
        let e = Expected::parse(text).unwrap();
        assert_eq!(e.header, ["tier sse2", "skia x"]);
        assert_eq!(e.entries["a/b"].case_hash, 0xff);
        assert_eq!(e.to_text(), text);
        assert!(Expected::parse("a 1").is_err());
    }
}
