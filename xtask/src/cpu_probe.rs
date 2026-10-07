//! `cargo xtask cpu-probe`: what this host's CPU means for skia-rust's tiers.
//!
//! Prints Skia's view of the CPU (`SkCpu` features, the tier `SkOpts::Init` would pick, with and
//! without the oracle's CPU caps), which tiers run natively, and the estimate fingerprints of
//! `docs/design/raster-pipeline.md` §1.4 compared with the oracle host's. With `--dump-tables`
//! it also writes the host's `rcpps`/`rsqrtps` tables (design §2.8) and checks that they
//! reproduce the host exactly over `[1,2)` and `[2,4)`.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result, bail};
use skia_rust_simd::Tier;
use skia_rust_simd::cpu::{self, CpuCap, CpuFeatures, X64Level};
use skia_rust_simd::estimates::{self, EstimateTables, Fingerprints};

pub fn probe(dump_tables: Option<&Path>) -> Result<()> {
    let mut out = String::new();
    let vendor = cpu::cpu_vendor().unwrap_or_else(|| "(not x86)".into());
    let brand = cpu::cpu_brand().unwrap_or_else(|| "(unknown)".into());
    writeln!(out, "cpu:               {vendor} / {brand}")?;
    writeln!(out, "arch:              {}", std::env::consts::ARCH)?;
    let features = CpuFeatures::read();
    writeln!(out, "SkCpu features:    {}", flags(features))?;
    match X64Level::compiled() {
        Some(level) => writeln!(out, "compile baseline:  SK_CPU_X64_LEVEL = {level:?}")?,
        None => writeln!(out, "compile baseline:  (not x86)")?,
    }
    writeln!(out, "Tier::detect():    {}", Tier::detect())?;
    if X64Level::compiled().is_some() {
        let capped: Vec<String> = CpuCap::ALL
            .iter()
            .map(|c| format!("{}={}", c.name(), Tier::detect_with_cap(*c)))
            .collect();
        writeln!(out, "with CPU cap:      {}", capped.join(" "))?;
    }
    let native: Vec<&str> = Tier::ALL
        .iter()
        .filter(|t| t.is_native())
        .map(|t| t.name())
        .collect();
    writeln!(out, "native tiers:      {}", native.join(" "))?;
    print!("{out}");

    println!("estimate fingerprints (FNV-1a, design §1.4; oracle host = AMD Zen 4):");
    let host = Fingerprints::host();
    for ((name, value), (_, reference)) in host
        .entries()
        .into_iter()
        .zip(estimates::AMD_ZEN4.entries())
    {
        let verdict = match (value, reference) {
            (None, _) => "not available on this host".to_owned(),
            (Some(v), Some(r)) if v == r => "matches the oracle host".to_owned(),
            (Some(_), Some(r)) => format!("DIFFERS from the oracle host ({r:016x})"),
            (Some(_), None) => "no reference".to_owned(),
        };
        let value = value.map_or_else(|| "-".repeat(16), |v| format!("{v:016x}"));
        println!("  {name:<14} {value}  {verdict}");
    }
    let per_tier: Vec<String> = Tier::ALL
        .iter()
        .filter(|t| t.is_x86())
        .map(|t| {
            let ok = host.matches_for(&estimates::AMD_ZEN4, *t);
            format!("{t}={}", if ok { "yes" } else { "no" })
        })
        .collect();
    println!("estimates match the goldens' host: {}", per_tier.join(" "));

    if let Some(dir) = dump_tables {
        dump(dir, &host)?;
    }
    Ok(())
}

fn flags(f: CpuFeatures) -> String {
    let names: Vec<&str> = f.iter_names().map(|(n, _)| n).collect();
    if names.is_empty() {
        "(none)".into()
    } else {
        names.join(" ")
    }
}

fn dump(dir: &Path, host: &Fingerprints) -> Result<()> {
    let Ok(tables) = EstimateTables::host() else {
        bail!("this host has no rcpps/rsqrtps; nothing to dump");
    };
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let (rcp, rsqrt) = tables.to_le_bytes();
    let write = |name: &str, bytes: &[u8]| -> Result<()> {
        let path = dir.join(name);
        std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))
    };
    write("rcpps.bin", &rcp)?;
    write("rsqrtps.bin", &rsqrt)?;

    let (rcp_bad, rsqrt_bad) = tables.mismatches_vs_host()?;
    let mut info = String::new();
    writeln!(
        info,
        "# Host estimate tables written by `cargo xtask cpu-probe --dump-tables`."
    )?;
    writeln!(
        info,
        "# rcpps.bin: 4096 x u32 LE, entry i = rcpps(bits 0x3f800000 | i << 11)."
    )?;
    writeln!(
        info,
        "# rsqrtps.bin: 8192 x u32 LE, [1,2) (0x3f800000 | i << 11) then [2,4) (0x40000000 | i << 11)."
    )?;
    writeln!(
        info,
        "cpu = {:?}",
        cpu::cpu_brand().unwrap_or_else(|| "(unknown)".into())
    )?;
    writeln!(
        info,
        "vendor = {:?}",
        cpu::cpu_vendor().unwrap_or_else(|| "(unknown)".into())
    )?;
    for (name, value) in host.entries() {
        if let Some(v) = value {
            writeln!(info, "{name} = {v:016x}")?;
        }
    }
    writeln!(info, "rcpps_table_mismatches_1_2 = {rcp_bad}")?;
    writeln!(info, "rsqrtps_table_mismatches_1_4 = {rsqrt_bad}")?;
    write("estimates.txt", info.as_bytes())?;

    println!(
        "wrote rcpps.bin, rsqrtps.bin, estimates.txt to {}",
        dir.display()
    );
    println!(
        "tables vs host over all 2^23 mantissas: rcpps[1,2) {rcp_bad} mismatches, \
         rsqrtps[1,4) {rsqrt_bad} mismatches{}",
        if rcp_bad == 0 && rsqrt_bad == 0 {
            " (exact: estimates depend only on the top 12 mantissa bits)"
        } else {
            ""
        }
    );
    Ok(())
}
