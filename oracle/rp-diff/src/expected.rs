// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Stored Skia results (`oracle/rp-diff/expected/<tier>.txt`) and the replay check against them.
//!
//! One file per Skia code path ([`Tier::name`]: `scalar`, `sse2`, `sse41`, `ml3`, `ml4`),
//! written by `cargo xtask oracle rp-diff --update` on the oracle host and committed:
//!
//! ```text
//! # comment lines: tier, Skia commit, compiler, host and its estimate fingerprints
//! <case name> <case hash> <output hash> [<canonical output hash>]     (16 hex digits, FNV-1a 64)
//! ```
//!
//! The case hash ([`Case::hash`]) ties a result to the exact case it came from: a case edited
//! without regenerating the results is reported as stale rather than compared.
//!
//! The optional last column is the hash of the output with its default NaNs canonicalized
//! ([`canonical_nans`]); see [`host_nan_is_x86`].

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
    /// [`fnv1a`] of Skia's output after [`canonical_nans`], if stored.
    pub canonical_hash: Option<u64>,
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
                    "line {}: expected `<name> <case hash> <output hash> [<canonical hash>]`",
                    i + 1
                )
            };
            if !(3..=4).contains(&t.len()) {
                return Err(bad());
            }
            let case_hash = u64::from_str_radix(t[1], 16).map_err(|_| bad())?;
            let output_hash = u64::from_str_radix(t[2], 16).map_err(|_| bad())?;
            let canonical_hash = t
                .get(3)
                .map(|h| u64::from_str_radix(h, 16).map_err(|_| bad()))
                .transpose()?;
            e.entries.insert(
                t[0].to_owned(),
                Entry {
                    case_hash,
                    output_hash,
                    canonical_hash,
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
            let _ = write!(s, "{name} {:016x} {:016x}", e.case_hash, e.output_hash);
            if let Some(h) = e.canonical_hash {
                let _ = write!(s, " {h:016x}");
            }
            s.push('\n');
        }
        s
    }
}

/// `bytes` with every aligned 32-bit word `0x7FC00000` read as `0xFFC00000`: the two default
/// NaNs (the `QNaN` that an invalid operation such as `inf * 0` or `sqrt(-1)` produces) made
/// equal. x86 produces `0xFFC00000` (the oracle's), Arm `0x7FC00000`.
#[must_use]
pub fn canonical_nans(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for w in out.as_chunks_mut::<4>().0 {
        if *w == 0x7fc0_0000u32.to_le_bytes() {
            *w = 0xffc0_0000u32.to_le_bytes();
        }
    }
    out
}

/// [`fnv1a`] of [`canonical_nans`]`(bytes)`: the last column of a stored result.
#[must_use]
pub fn canonical_hash(bytes: &[u8]) -> u64 {
    fnv1a(&canonical_nans(bytes))
}

/// Whether this host's arithmetic produces the oracle's default NaN (`0xFFC00000`): x86.
///
/// On other hosts (Arm, wasm) the `Scalar` tier, which runs on the host FPU, and the models
/// (Rust `f32` operators in the generic stage code) produce the host's own default NaN for an
/// invalid operation (`0x7FC00000` on Arm, unspecified on wasm), so there a result also matches
/// if only the default NaNs differ ([`canonical_nans`]). On x86 hosts every result is exact.
#[must_use]
pub const fn host_nan_is_x86() -> bool {
    cfg!(any(target_arch = "x86", target_arch = "x86_64"))
}

/// Whether `out` matches the stored result `entry`: bit for bit, or, off x86
/// ([`host_nan_is_x86`]), up to the default NaN.
#[must_use]
pub fn output_matches(entry: &Entry, out: &[u8]) -> bool {
    fnv1a(out) == entry.output_hash
        || (!host_nan_is_x86() && entry.canonical_hash == Some(canonical_hash(out)))
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
                Ok(out) if output_matches(entry, &out) => {}
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
        let text = "# tier sse2\n# skia x\na/b 00000000000000ff 0123456789abcdef\n\
                    c/d 00000000000000fe 0123456789abcdee 0000000000000011\n";
        let e = Expected::parse(text).unwrap();
        assert_eq!(e.header, ["tier sse2", "skia x"]);
        assert_eq!(e.entries["a/b"].case_hash, 0xff);
        assert_eq!(e.entries["a/b"].canonical_hash, None);
        assert_eq!(e.entries["c/d"].canonical_hash, Some(0x11));
        assert_eq!(e.to_text(), text);
        assert!(Expected::parse("a 1").is_err());
    }

    #[test]
    fn default_nans_are_canonicalized_off_x86_only() {
        let skia: Vec<u8> = [0xffc0_0000u32, 7, 0x7fc0_0001]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let arm: Vec<u8> = [0x7fc0_0000u32, 7, 0x7fc0_0001]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        let entry = Entry {
            case_hash: 0,
            output_hash: fnv1a(&skia),
            canonical_hash: Some(canonical_hash(&skia)),
        };
        assert!(output_matches(&entry, &skia));
        // An Arm default NaN matches only on a host that is not x86.
        assert_eq!(output_matches(&entry, &arm), !host_nan_is_x86());
        // Other payloads are never equal.
        let other: Vec<u8> = [0x7fc0_0000u32, 7, 0x7fc0_0002]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert!(!output_matches(&entry, &other));
    }
}
