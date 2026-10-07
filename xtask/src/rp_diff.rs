//! `cargo xtask oracle rp-diff`: the per-stage raster pipeline oracle
//! (`docs/design/raster-pipeline.md` §4.2, `oracle/rp-diff`).
//!
//! On the oracle host: builds the C++ driver (`oracle/rp-diff/cpp`) against an oracle build's
//! static libraries with the oracle's compiler flags, one `SkRasterPipeline_opts.h`
//! instantiation per x86 code path; writes the case list; runs it through Skia for each tier;
//! replays it through skia-rust on every selection that stands in for the tier; reports every
//! mismatch with its first differing bytes; and with `--update` stores Skia's results in
//! `oracle/rp-diff/expected/<tier>.txt`. `--replay` skips the C++ side and checks the stored
//! results (what `cargo test -p skia-rust-rp-diff` does on every host).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result, bail, ensure};
use skia_rust_rp_diff::case::{Case, cases_to_text, fnv1a, glob_match};
use skia_rust_rp_diff::cases;
use skia_rust_rp_diff::expected::{
    self, Entry, Expected, ORACLE_TIERS, canonical_hash, selection_name, selections,
};
use skia_rust_rp_diff::replay::{build_stages, run_case};
use skia_rust_simd::Tier;
use skia_rust_simd::estimates::{AMD_ZEN4, Fingerprints};

use crate::{oracle, skia};

/// Options of `cargo xtask oracle rp-diff`.
#[derive(Debug)]
pub struct Options {
    /// Tier names (`scalar`, `sse2`, `sse41`, `ml3`, `ml4`); empty = all.
    pub tiers: Vec<String>,
    /// Only cases whose names match this glob (`*` = any characters).
    pub case_glob: Option<String>,
    /// Rewrite `expected/<tier>.txt` with Skia's results.
    pub update: bool,
    /// Only replay against the stored results (no C++).
    pub replay: bool,
    /// The oracle build whose static libraries the driver links against.
    pub build: String,
    /// Rebuild the driver even if it looks up to date.
    pub rebuild: bool,
    /// Mismatch details to print per tier.
    pub details: usize,
}

pub fn run(root: &Path, opts: &Options) -> Result<()> {
    let tiers: Vec<Tier> = if opts.tiers.is_empty() {
        ORACLE_TIERS.to_vec()
    } else {
        opts.tiers
            .iter()
            .flat_map(|t| t.split(','))
            .map(|t| {
                Tier::from_name(t)
                    .filter(|t| ORACLE_TIERS.contains(t))
                    .with_context(|| {
                        format!("unknown rp-diff tier `{t}` (scalar sse2 sse41 ml3 ml4)")
                    })
            })
            .collect::<Result<_>>()?
    };
    let all = cases::all();
    let selected: Vec<Case> = all
        .iter()
        .filter(|c| {
            opts.case_glob
                .as_deref()
                .is_none_or(|g| glob_match(g, &c.name))
        })
        .cloned()
        .collect();
    ensure!(!selected.is_empty(), "no case matches the glob");
    println!("{} of {} cases", selected.len(), all.len());

    if opts.replay {
        ensure!(!opts.update, "--update needs the C++ side (drop --replay)");
        return replay(&tiers, &selected);
    }

    let exe = build_driver(root, &opts.build, opts.rebuild)?;
    let work = root.join("target").join("rp-diff");
    let cases_file = work.join("cases.txt");
    std::fs::write(&cases_file, cases_to_text(&selected))?;

    let mut failures = 0;
    for tier in tiers {
        if tier != Tier::Scalar && !tier.is_native() {
            println!(
                "{}: skipped (this CPU cannot run Skia's {} code)",
                tier.name(),
                tier.name()
            );
            continue;
        }
        let out_file = work.join(format!("skia-{}.txt", tier.name()));
        oracle_run(
            Command::new(&exe)
                .arg(tier.name())
                .arg(&cases_file)
                .arg(&out_file),
        )?;
        let skia = read_outputs(&out_file)?;
        failures += compare(tier, &selected, &skia, opts.details)?;
        if opts.update {
            update_expected(root, tier, &all, &selected, &skia)?;
        }
    }
    ensure!(failures == 0, "{failures} rp-diff mismatches");
    Ok(())
}

/// Runs a command, failing on a nonzero exit status.
fn oracle_run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("running {cmd:?}"))?;
    ensure!(status.success(), "{cmd:?} failed: {status}");
    Ok(())
}

fn replay(tiers: &[Tier], cases: &[Case]) -> Result<()> {
    let mut problems = 0;
    for &tier in tiers {
        let mut stored = Expected::load(tier)
            .map_err(anyhow::Error::msg)?
            .with_context(|| format!("no {}", expected::expected_path(tier).display()))?;
        stored
            .entries
            .retain(|name, _| cases.iter().any(|c| &c.name == name));
        let found = expected::check(tier, cases, &stored);
        let sels: Vec<String> = selections(tier).into_iter().map(selection_name).collect();
        println!(
            "{}: {} cases on {}: {} problems",
            tier.name(),
            cases.len(),
            sels.join(", "),
            found.len()
        );
        for p in found.iter().take(40) {
            println!("  {p}");
        }
        problems += found.len();
    }
    ensure!(problems == 0, "{problems} rp-diff problems");
    Ok(())
}

/// Parses the driver's output: `<name> <hex>` per case.
fn read_outputs(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let (name, hex) = line
            .split_once(' ')
            .with_context(|| format!("bad driver output line: {line}"))?;
        ensure!(hex.len() % 2 == 0, "odd hex length for {name}");
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()?;
        out.insert(name.to_owned(), bytes);
    }
    Ok(out)
}

/// Compares skia-rust with Skia's bytes for one tier; prints a summary and up to `details`
/// mismatch descriptions. Returns the number of mismatches.
fn compare(
    tier: Tier,
    cases: &[Case],
    skia: &BTreeMap<String, Vec<u8>>,
    details: usize,
) -> Result<usize> {
    let sels = selections(tier);
    let mut mismatches = 0;
    let mut shown = 0;
    let mut per_sel = vec![0usize; sels.len()];
    for c in cases {
        let want = skia
            .get(&c.name)
            .with_context(|| format!("the driver produced no output for {}", c.name))?;
        ensure!(
            want.len() == c.output_len(),
            "{}: driver output has the wrong length",
            c.name
        );
        let stages = build_stages(&c.stages, tier).map_err(anyhow::Error::msg)?;
        // Skia's x64 scalar proxy differs from wasm's libc in these (design R5).
        if tier == Tier::Scalar && c.scalar_proxy_differs() {
            continue;
        }
        for (i, &sel) in sels.iter().enumerate() {
            let got = run_case(c, &stages, sel);
            if got.as_deref().ok() == Some(want.as_slice()) {
                continue;
            }
            mismatches += 1;
            per_sel[i] += 1;
            if shown < details {
                shown += 1;
                println!("MISMATCH {} on {}", c.name, selection_name(sel));
                match &got {
                    Ok(got) => print!("{}", describe_diff(c, want, got)),
                    Err(e) => println!("  skia-rust panicked: {e}"),
                }
            }
        }
    }
    let summary: Vec<String> = sels
        .iter()
        .zip(&per_sel)
        .map(|(&s, &n)| format!("{} {}/{}", selection_name(s), cases.len() - n, cases.len()))
        .collect();
    println!("{}: {} (matching/cases)", tier.name(), summary.join(", "));
    Ok(mismatches)
}

/// The first differing bytes of each differing buffer, as 32-bit words around them.
fn describe_diff(case: &Case, want: &[u8], got: &[u8]) -> String {
    let mut s = String::new();
    let mut start = 0;
    for (slot, b) in case.buffers.iter().enumerate() {
        let end = start + b.bytes.len();
        let (w, g) = (&want[start..end], &got[start..end]);
        if let Some(first) = w.iter().zip(g).position(|(a, b)| a != b) {
            let count = w.iter().zip(g).filter(|(a, b)| a != b).count();
            let at = first / 4 * 4;
            let hi = (at + 16).min(w.len());
            let words = |bytes: &[u8]| {
                bytes
                    .chunks(4)
                    .map(|c| {
                        c.iter()
                            .rev()
                            .fold(String::new(), |s, b| s + &format!("{b:02x}"))
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let _ = writeln!(
                s,
                "  slot {slot}: {count} bytes differ, first at byte {first}; from byte {at} \
                 (LE words) skia {} | ours {}",
                words(&w[at..hi]),
                words(&g[at..hi])
            );
        }
        start = end;
    }
    s
}

/// Merges Skia's results for `ran` into `expected/<tier>.txt`, dropping entries of cases that
/// no longer exist.
fn update_expected(
    root: &Path,
    tier: Tier,
    all: &[Case],
    ran: &[Case],
    skia: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let mut e = Expected::load(tier)
        .map_err(anyhow::Error::msg)?
        .unwrap_or_default();
    e.entries
        .retain(|name, _| all.iter().any(|c| &c.name == name));
    for c in ran {
        e.entries.insert(
            c.name.clone(),
            Entry {
                case_hash: c.hash(),
                output_hash: fnv1a(&skia[&c.name]),
                canonical_hash: Some(canonical_hash(&skia[&c.name])),
            },
        );
    }
    let pin = skia::read_pin(root)?;
    let clang = Path::new(&oracle::clang_win()?)
        .join("bin")
        .join("clang-cl.exe");
    let version = Command::new(clang)
        .arg("--version")
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_owned()
        })
        .unwrap_or_default();
    let fp = Fingerprints::host();
    let brand = skia_rust_simd::cpu::cpu_brand().unwrap_or_else(|| "(unknown)".into());
    e.header = vec![
        format!(
            "rp-diff: real Skia's results for the {} code path; written by `cargo xtask oracle \
             rp-diff --update`, do not edit",
            tier.name()
        ),
        format!("skia {} ({})", pin.commit, pin.branch),
        format!("compiler {version}"),
        format!(
            "host {} (estimates {} the AMD Zen 4 oracle host's for this tier)",
            brand.trim(),
            if fp.matches_for(&AMD_ZEN4, tier) {
                "match"
            } else {
                "DIFFER from"
            }
        ),
        "<case> <case hash> <output hash> (FNV-1a 64; see oracle/rp-diff/src/expected.rs)".into(),
    ];
    let path = expected::expected_path(tier);
    std::fs::write(&path, e.to_text())?;
    println!("wrote {} ({} cases)", path.display(), e.entries.len());
    Ok(())
}

/// The Skia checkout: `third_party/skia`, or the main checkout's when this is a git worktree.
fn skia_checkout(root: &Path) -> Result<PathBuf> {
    let own = skia::checkout_path(root);
    if own.join("src").is_dir() {
        return Ok(own);
    }
    let text = std::fs::read_to_string(root.join(".git")).ok();
    let main = text
        .as_deref()
        .and_then(|t| t.strip_prefix("gitdir:"))
        .map(|g| PathBuf::from(g.trim()))
        .and_then(|g| Some(g.parent()?.parent()?.parent()?.to_path_buf()));
    match main {
        Some(main) if skia::checkout_path(&main).join("src").is_dir() => {
            Ok(skia::checkout_path(&main))
        }
        _ => bail!("no Skia checkout; run `cargo xtask skia fetch`"),
    }
}

/// The value of the first `<name> = ...` line of a ninja file (top level or in a build block),
/// split into arguments with ninja's `$` escapes and double quotes resolved.
fn ninja_var(text: &str, name: &str) -> Option<Vec<String>> {
    let line = text.lines().find_map(|l| {
        l.trim_start()
            .strip_prefix(name)
            .and_then(|r| r.trim_start().strip_prefix('='))
    })?;
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.trim().chars();
    while let Some(c) = chars.next() {
        match c {
            '$' => match chars.next() {
                Some(n @ (' ' | ':' | '$')) => cur.push(n),
                Some(n) => {
                    cur.push('$');
                    cur.push(n);
                }
                None => {}
            },
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        args.push(cur);
    }
    Some(args)
}

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// The per-tier flags added to the oracle's (which come from its ml3 target minus `/arch`).
fn tier_flags(tier: Tier) -> &'static [&'static str] {
    match tier {
        Tier::Scalar => &["-DSKRP_CPU_SCALAR"],
        Tier::Sse41 => &["/clang:-msse4.1"],
        Tier::Ml3 => &["/arch:AVX2"],
        Tier::Ml4 => &["/arch:AVX512"],
        _ => &[],
    }
}

/// Builds `target/rp-diff/<build>/rp_diff.exe` (if out of date) and returns its path.
fn build_driver(root: &Path, build: &str, rebuild: bool) -> Result<PathBuf> {
    ensure!(
        cfg!(windows),
        "the rp-diff C++ driver builds on the Windows oracle host only; elsewhere use --replay"
    );
    let skia_dir = skia_checkout(root)?;
    let build_dir = skia_dir.join("out").join("oracle").join(build);
    let skia_lib = build_dir.join("skia.lib");
    ensure!(
        skia_lib.exists(),
        "{} not built; run `cargo xtask oracle build {build}`",
        build_dir.display()
    );
    let src = root.join("oracle").join("rp-diff").join("cpp");
    let out = root.join("target").join("rp-diff").join(build);
    std::fs::create_dir_all(&out)?;
    let exe = out.join("rp_diff.exe");
    let newest_input = ["driver.cpp", "tier.cpp"]
        .iter()
        .map(|f| src.join(f))
        .chain([skia_lib])
        .filter_map(|p| mtime(&p))
        .max();
    if !rebuild && mtime(&exe).is_some_and(|t| Some(t) >= newest_input) {
        return Ok(exe);
    }

    // The oracle's own compile flags, from its ml3 target (the same for every opts target but
    // for `/arch`), without debug info.
    let ninja = std::fs::read_to_string(build_dir.join("obj").join("ml3.ninja"))
        .context("reading the oracle's obj/ml3.ninja")?;
    let mut flags = Vec::new();
    for var in ["defines", "include_dirs", "cflags", "cflags_cc"] {
        flags.extend(
            ninja_var(&ninja, var)
                .with_context(|| format!("no `{var}` in obj/ml3.ninja"))?
                .into_iter()
                .filter(|f| !matches!(f.as_str(), "/arch:AVX2" | "/Z7" | "-gcodeview-ghash")),
        );
    }
    let llvm = PathBuf::from(oracle::clang_win()?).join("bin");
    let clang = llvm.join("clang-cl.exe");

    let mut jobs = vec![(src.join("driver.cpp"), out.join("driver.obj"), Vec::new())];
    for tier in ORACLE_TIERS {
        let mut extra: Vec<String> = tier_flags(tier).iter().map(|s| (*s).to_owned()).collect();
        extra.push(format!("-DSK_OPTS_NS=rpdiff_{}", tier.name()));
        extra.push(format!("-DRPDIFF_INSTALL=rpdiff_install_{}", tier.name()));
        jobs.push((
            src.join("tier.cpp"),
            out.join(format!("tier_{}.obj", tier.name())),
            extra,
        ));
    }
    println!(
        "building the rp-diff driver against {}",
        build_dir.display()
    );
    let results: Vec<Result<()>> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(cpp, obj, extra)| {
                let (clang, flags, build_dir) = (&clang, &flags, &build_dir);
                s.spawn(move || -> Result<()> {
                    let o = Command::new(clang)
                        .arg("/nologo")
                        .args(flags)
                        .args(extra)
                        .arg("/c")
                        .arg(cpp)
                        .arg(format!("/Fo{}", obj.display()))
                        .current_dir(build_dir)
                        .output()
                        .context("running clang-cl")?;
                    ensure!(
                        o.status.success(),
                        "compiling {} {extra:?} failed:\n{}{}",
                        cpp.display(),
                        String::from_utf8_lossy(&o.stdout),
                        String::from_utf8_lossy(&o.stderr)
                    );
                    Ok(())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("compile thread"))
            .collect()
    });
    for r in results {
        r?;
    }
    link_driver(&build_dir, &llvm, &out, &exe)?;
    println!("built {}", exe.display());
    Ok(exe)
}

/// Links the driver's objects in `out` with the oracle build's static libraries and DM's
/// system libraries and link flags.
fn link_driver(build_dir: &Path, llvm: &Path, out: &Path, exe: &Path) -> Result<()> {
    let dm = std::fs::read_to_string(build_dir.join("obj").join("dm.ninja"))
        .context("reading the oracle's obj/dm.ninja")?;
    let ldflags: Vec<String> = ninja_var(&dm, "ldflags")
        .context("no ldflags in obj/dm.ninja")?
        .into_iter()
        .filter(|f| !f.starts_with("/DEBUG"))
        .collect();
    let libs = ninja_var(&dm, "libs").context("no libs in obj/dm.ninja")?;
    let mut static_libs = Vec::new();
    for entry in std::fs::read_dir(build_dir)? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "lib") && p.file_stem().is_some_and(|s| s != "dm") {
            static_libs.push(p);
        }
    }
    static_libs.sort();
    let mut link = Command::new(llvm.join("lld-link.exe"));
    link.arg("/nologo")
        .arg(format!("/OUT:{}", exe.display()))
        .arg(out.join("driver.obj"));
    for tier in ORACLE_TIERS {
        link.arg(out.join(format!("tier_{}.obj", tier.name())));
    }
    let o = link
        .args(&static_libs)
        .args(&libs)
        .args(&ldflags)
        .output()
        .context("running lld-link")?;
    ensure!(
        o.status.success(),
        "linking the rp-diff driver failed:\n{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ninja_vars_unescape_and_split() {
        let text = "defines = -DA -DB=1\ncflags = /arch$:AVX2 -imsvc \"C$:/Program$ Files/x\" /O2\n  ldflags = /LIBPATH:\"C$:/a$ b\"\n";
        assert_eq!(ninja_var(text, "defines").unwrap(), ["-DA", "-DB=1"]);
        assert_eq!(
            ninja_var(text, "cflags").unwrap(),
            ["/arch:AVX2", "-imsvc", "C:/Program Files/x", "/O2"]
        );
        assert_eq!(ninja_var(text, "ldflags").unwrap(), ["/LIBPATH:C:/a b"]);
        assert_eq!(ninja_var(text, "libs"), None);
    }
}
