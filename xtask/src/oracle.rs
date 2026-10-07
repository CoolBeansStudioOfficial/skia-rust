//! The oracle: real Skia, built from the pinned checkout, rendering goldens with DM.
//!
//! See `oracle/README.md`. Builds and tiers are described by `oracle/tiers.toml`; Skia is
//! patched with `oracle/patches/skia-oracle.patch` plus the files in `oracle/dm/`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::skia;

/// Runtime CPU levels in increasing order; the values of `SKIA_ORACLE_CPU_CAP`.
const LEVELS: [&str; 4] = ["baseline", "ssse3", "ml3", "ml4"];

/// Attempts at `git-sync-deps` before giving up (see [`deps`]).
const SYNC_ATTEMPTS: u32 = 5;

#[derive(Debug, Deserialize)]
struct Config {
    gn: GnConfig,
    build: Vec<Build>,
    #[serde(default)]
    gpu_tier: Vec<GpuTier>,
}

#[derive(Debug, Deserialize)]
struct GnConfig {
    args: Vec<String>,
    /// `extra_cflags` for every build, before each build's own.
    #[serde(default)]
    extra_cflags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Build {
    name: String,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    gpu: bool,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    extra_cflags: Vec<String>,
}

/// A GPU tier as written in `oracle/tiers.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct GpuTier {
    name: String,
    build: String,
    /// DM configs, e.g. `grdawn_d3d12`.
    configs: Vec<String>,
    /// Substring of the Dawn adapter's device name or description.
    adapter: String,
    /// Runtime CPU level: Graphite still does some work (e.g. software masks) on the CPU.
    cpu_level: String,
    #[serde(default)]
    report_only: bool,
}

/// A tier: one build run with one runtime CPU cap, and for GPU tiers one Dawn adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub name: String,
    pub build: String,
    pub level: String,
    pub gpu: Option<GpuRun>,
}

/// The GPU-specific part of a tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuRun {
    pub configs: Vec<String>,
    pub adapter: String,
    pub report_only: bool,
}

fn oracle_dir(root: &Path) -> PathBuf {
    root.join("oracle")
}

fn read_config(root: &Path) -> Result<Config> {
    let path = oracle_dir(root).join("tiers.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn level_index(level: &str) -> Result<usize> {
    LEVELS
        .iter()
        .position(|l| *l == level)
        .with_context(|| format!("unknown CPU level `{level}` (expected one of {LEVELS:?})"))
}

/// Every CPU tier: each CPU build at its own level and at every runtime level above it.
fn derive_tiers(config: &Config) -> Result<Vec<Tier>> {
    let mut tiers = Vec::new();
    for build in config.build.iter().filter(|b| !b.gpu) {
        let level = build
            .level
            .as_deref()
            .with_context(|| format!("CPU build `{}` has no `level`", build.name))?;
        let own = level_index(level)?;
        for (i, rt) in LEVELS.iter().enumerate().skip(own) {
            let name = if i == own {
                format!("cpu-{}", build.name)
            } else {
                format!("cpu-{}-rt-{rt}", build.name)
            };
            tiers.push(Tier {
                name,
                build: build.name.clone(),
                level: (*rt).to_owned(),
                gpu: None,
            });
        }
    }
    for gpu in &config.gpu_tier {
        level_index(&gpu.cpu_level)?;
        ensure!(
            config.build.iter().any(|b| b.gpu && b.name == gpu.build),
            "GPU tier `{}` names unknown GPU build `{}`",
            gpu.name,
            gpu.build
        );
        tiers.push(Tier {
            name: gpu.name.clone(),
            build: gpu.build.clone(),
            level: gpu.cpu_level.clone(),
            gpu: Some(GpuRun {
                configs: gpu.configs.clone(),
                adapter: gpu.adapter.clone(),
                report_only: gpu.report_only,
            }),
        });
    }
    Ok(tiers)
}

fn find_tier(config: &Config, name: &str) -> Result<Tier> {
    derive_tiers(config)?
        .into_iter()
        .find(|t| t.name == name)
        .with_context(|| format!("no tier named `{name}`; see `cargo xtask oracle tiers`"))
}

pub fn list_tiers(root: &Path) -> Result<()> {
    let config = read_config(root)?;
    for tier in derive_tiers(&config)? {
        print!(
            "{:<28} build={:<10} cpu={:<8}",
            tier.name, tier.build, tier.level
        );
        if let Some(gpu) = &tier.gpu {
            print!(
                " adapter=\"{}\" configs={}",
                gpu.adapter,
                gpu.configs.join(",")
            );
            if gpu.report_only {
                print!(" (report only)");
            }
        }
        println!();
    }
    Ok(())
}

fn python() -> Result<Command> {
    for (exe, args) in [
        ("python3", &["--version"][..]),
        ("python", &["--version"]),
        ("py", &["-3", "--version"]),
    ] {
        let ok = Command::new(exe)
            .args(args)
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            let mut cmd = Command::new(exe);
            if exe == "py" {
                cmd.arg("-3");
            }
            return Ok(cmd);
        }
    }
    bail!("Python 3 not found (tried python3, python, py -3); Skia's build scripts need it")
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("running {cmd:?}"))?;
    ensure!(status.success(), "{cmd:?} failed with {status}");
    Ok(())
}

/// Syncs Skia's third-party dependencies (Dawn, libjpeg-turbo, …) plus `gn` and `ninja`.
pub fn deps(root: &Path) -> Result<()> {
    let skia_dir = skia::checkout_path(root);
    ensure!(
        skia_dir.exists(),
        "third_party/skia missing; run `cargo xtask skia fetch`"
    );
    // git-sync-deps clones every dependency at once, which googlesource answers with
    // HTTP 429 rate limits. Re-running is cheap (finished deps are skipped), so retry.
    for attempt in 1..=SYNC_ATTEMPTS {
        let result = run(python()?
            .arg("tools/git-sync-deps")
            .current_dir(&skia_dir)
            .env("GIT_SYNC_DEPS_SKIP_EMSDK", "1"));
        match result {
            Ok(()) => break,
            Err(e) if attempt < SYNC_ATTEMPTS => {
                let wait = 60 * u64::from(attempt);
                eprintln!("git-sync-deps failed ({e}); retrying in {wait}s");
                std::thread::sleep(std::time::Duration::from_secs(wait));
            }
            Err(e) => return Err(e),
        }
    }
    run(python()?.arg("bin/fetch-ninja").current_dir(&skia_dir))?;
    println!("Skia dependencies synced");
    Ok(())
}

/// Copies `oracle/dm/*` into Skia and applies `oracle/patches/*.patch` (idempotent).
pub fn patch(root: &Path) -> Result<()> {
    let skia_dir = skia::checkout_path(root);
    ensure!(
        skia_dir.exists(),
        "third_party/skia missing; run `cargo xtask skia fetch`"
    );
    for entry in std::fs::read_dir(oracle_dir(root).join("dm"))? {
        let entry = entry?;
        let dest = skia_dir.join("dm").join(entry.file_name());
        std::fs::copy(entry.path(), &dest)
            .with_context(|| format!("copying to {}", dest.display()))?;
    }
    let mut patches: Vec<PathBuf> = std::fs::read_dir(oracle_dir(root).join("patches"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    patches.sort();
    for patch in patches
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "patch"))
    {
        let patch_arg = patch.to_str().context("non-UTF-8 patch path")?;
        if skia::git(&skia_dir, &["apply", "--reverse", "--check", patch_arg]).is_ok() {
            println!("already applied: {}", patch.display());
            continue;
        }
        skia::git(&skia_dir, &["apply", patch_arg])
            .with_context(|| format!("applying {} (did the Skia pin change?)", patch.display()))?;
        println!("applied: {}", patch.display());
    }
    Ok(())
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn build_dir(root: &Path, build: &str) -> PathBuf {
    skia::checkout_path(root)
        .join("out")
        .join("oracle")
        .join(build)
}

/// `clang_win` for GN: `SKIA_ORACLE_CLANG_WIN`, else the default LLVM install location.
fn clang_win() -> Result<String> {
    let dir = std::env::var("SKIA_ORACLE_CLANG_WIN")
        .unwrap_or_else(|_| "C:/Program Files/LLVM".to_owned());
    ensure!(
        Path::new(&dir).join("bin").join("clang-cl.exe").exists(),
        "clang-cl not found under {dir}; install LLVM or set SKIA_ORACLE_CLANG_WIN"
    );
    Ok(dir.replace('\\', "/"))
}

/// `win_vc` for GN: `SKIA_ORACLE_WIN_VC`, else the newest Visual Studio install that has the
/// x64 C++ tools. Skia's own detection takes the first install it finds, which may lack them.
fn win_vc() -> Result<String> {
    if let Ok(dir) = std::env::var("SKIA_ORACLE_WIN_VC") {
        return Ok(dir.replace('\\', "/"));
    }
    Ok(format!("{}/VC", vs_install()?))
}

/// The newest Visual Studio install with the x64 C++ tools, with `/` separators.
fn vs_install() -> Result<String> {
    let vswhere =
        Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    let out = Command::new(vswhere)
        .args(["-latest", "-products", "*", "-requires"])
        .arg("Microsoft.VisualStudio.Component.VC.Tools.x86.x64")
        .args(["-property", "installationPath"])
        .output()
        .context("running vswhere")?;
    let install = String::from_utf8(out.stdout)?.trim().to_owned();
    ensure!(
        !install.is_empty(),
        "no Visual Studio install with the x64 C++ tools; install them or set SKIA_ORACLE_WIN_VC"
    );
    Ok(install.replace('\\', "/"))
}

/// `PATH` for the build: Skia's `ninja`, and on Windows Visual Studio's bundled `CMake` when
/// none is installed. Skia builds Dawn and Tint by running `CMake` + `ninja` from GN, and its
/// script looks both up on `PATH`.
fn build_path(ninja: &Path) -> Result<std::ffi::OsString> {
    let mut dirs = vec![
        ninja
            .parent()
            .context("ninja has no parent dir")?
            .to_path_buf(),
    ];
    let has_cmake = Command::new("cmake")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if cfg!(windows) && !has_cmake {
        let cmake = PathBuf::from(vs_install()?)
            .join("Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin");
        ensure!(
            cmake.join("cmake.exe").exists(),
            "CMake not found on PATH or in Visual Studio; install it (Dawn's build needs it)"
        );
        dirs.push(cmake);
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    Ok(std::env::join_paths(dirs)?)
}

fn tool(root: &Path, candidates: &[&str], name: &str) -> Result<PathBuf> {
    let skia_dir = skia::checkout_path(root);
    candidates
        .iter()
        .map(|c| skia_dir.join(c).join(exe(name)))
        .find(|p| p.exists())
        .with_context(|| format!("{name} not found in Skia; run `cargo xtask oracle deps`"))
}

/// Configures and builds DM for one oracle build.
pub fn build(root: &Path, name: &str) -> Result<()> {
    let config = read_config(root)?;
    let build = config
        .build
        .iter()
        .find(|b| b.name == name)
        .with_context(|| format!("no build named `{name}` in oracle/tiers.toml"))?;
    patch(root)?;

    let out = build_dir(root, name);
    std::fs::create_dir_all(&out)?;
    let mut args = String::new();
    for arg in config.gn.args.iter().chain(&build.args) {
        writeln!(args, "{arg}")?;
    }
    // GN allows one `extra_cflags` assignment, so merge the common and per-build lists.
    let cflags: Vec<String> = config
        .gn
        .extra_cflags
        .iter()
        .chain(&build.extra_cflags)
        .map(|f| format!("{f:?}"))
        .collect();
    if !cflags.is_empty() {
        writeln!(args, "extra_cflags = [{}]", cflags.join(", "))?;
    }
    if cfg!(windows) {
        writeln!(args, "clang_win = \"{}\"", clang_win()?)?;
        writeln!(args, "win_vc = \"{}\"", win_vc()?)?;
    }
    std::fs::write(out.join("args.gn"), args)?;

    let gn = tool(root, &["bin"], "gn")?;
    let ninja = tool(root, &["third_party/ninja", "bin"], "ninja")?;
    let skia_dir = skia::checkout_path(root);
    let path = build_path(&ninja)?;
    run(Command::new(gn)
        .arg("gen")
        .arg(&out)
        .current_dir(&skia_dir)
        .env("PATH", &path))?;
    run(Command::new(ninja)
        .arg("-C")
        .arg(&out)
        .arg("dm")
        .env("PATH", &path))?;
    println!("built DM for {name}: {}", out.join(exe("dm")).display());
    Ok(())
}

/// Goldens for the current pin: `goldens/<commit>/`.
///
/// Layout:
/// - `objects/<sha[..2]>/<sha>.zst`: every distinct output once, zstd-compressed;
/// - `<tier>/hashes.json`: result id -> SHA-256 of its raw bytes;
/// - `<tier>/meta.json`: result id -> `OracleDump` metadata (size, color type, ...);
/// - `<tier>/toolchain.txt`: compiler and GN args the tier was rendered with.
///
/// Many outputs are identical across tiers, so storing them by hash keeps the full
/// tier matrix within a few GB.
fn pin_golden_dir(root: &Path) -> Result<PathBuf> {
    let pin = skia::read_pin(root)?;
    Ok(root.join("goldens").join(&pin.commit))
}

/// Golden directory for one tier at the current pin.
fn golden_dir(root: &Path, tier: &str) -> Result<PathBuf> {
    Ok(pin_golden_dir(root)?.join(tier))
}

fn object_path(pin_dir: &Path, sha: &str) -> PathBuf {
    pin_dir
        .join("objects")
        .join(&sha[..2])
        .join(format!("{sha}.zst"))
}

fn read_json_map<T: serde::de::DeserializeOwned>(path: &Path) -> Result<BTreeMap<String, T>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("writing {}", path.display()))
}

/// Options for one DM run.
#[derive(Debug)]
pub struct RunOptions {
    pub tier: String,
    pub configs: Vec<String>,
    pub srcs: Vec<String>,
    pub matches: Vec<String>,
    pub threads: Option<usize>,
    /// Drop the tier's existing results first instead of merging into them.
    pub fresh: bool,
}

/// Renders goldens for one tier with DM, then checks every result ran at that CPU tier.
/// CPU tiers default to the `8888` config; GPU tiers always use their own configs.
pub fn run_tier(root: &Path, opts: &RunOptions) -> Result<()> {
    let config = read_config(root)?;
    let tier = find_tier(&config, &opts.tier)?;
    let pin = skia::read_pin(root)?;
    let head = skia::checkout_commit(root)?;
    ensure!(
        head == pin.commit,
        "third_party/skia is at {head}, pin is {}",
        pin.commit
    );

    let dm = build_dir(root, &tier.build).join(exe("dm"));
    ensure!(
        dm.exists(),
        "{} missing; run `cargo xtask oracle build {}`",
        dm.display(),
        tier.build
    );
    let out = pin_golden_dir(root)?.join(".staging").join(&tier.name);
    if out.exists() {
        std::fs::remove_dir_all(&out).with_context(|| format!("clearing {}", out.display()))?;
    }
    std::fs::create_dir_all(&out)?;

    let skia_dir = skia::checkout_path(root);
    let mut cmd = Command::new(&dm);
    cmd.current_dir(&skia_dir)
        .env("SKIA_ORACLE_CPU_CAP", &tier.level)
        .arg("--oracleRawPath")
        .arg(&out)
        .args(["--resourcePath", "resources"])
        .args(["--nativeFonts", "false"]);
    let configs = if let Some(gpu) = &tier.gpu {
        ensure!(
            opts.configs.is_empty(),
            "GPU tier `{}` uses its configs from oracle/tiers.toml; drop --config",
            tier.name
        );
        cmd.env("SKIA_ORACLE_DAWN_ADAPTER", &gpu.adapter).args([
            "--cpu",
            "false",
            "--gpu",
            "false",
            "--graphite",
            "true",
        ]);
        gpu.configs.clone()
    } else {
        cmd.args(["--cpu", "true", "--gpu", "false", "--graphite", "false"]);
        if opts.configs.is_empty() {
            vec!["8888".to_owned()]
        } else {
            opts.configs.clone()
        }
    };
    cmd.arg("--config")
        .args(&configs)
        .arg("--src")
        .args(&opts.srcs);
    if !opts.matches.is_empty() {
        cmd.arg("--match").args(&opts.matches);
    }
    if let Some(threads) = opts.threads {
        cmd.args(["--threads", &threads.to_string()]);
    }
    // Keep whatever DM rendered even if some sources fail; report the failure after.
    let dm_status = cmd.status().with_context(|| format!("running {cmd:?}"))?;

    let count = check_tier(&out, &tier.level)?;
    let tier_dir = golden_dir(root, &tier.name)?;
    if opts.fresh && tier_dir.exists() {
        std::fs::remove_dir_all(&tier_dir)
            .with_context(|| format!("clearing {}", tier_dir.display()))?;
    }
    std::fs::create_dir_all(&tier_dir)?;
    let stored = ingest(root, &tier_dir, &out)?;
    write_toolchain(root, &tier_dir, &tier)?;
    std::fs::remove_dir_all(&out).with_context(|| format!("clearing {}", out.display()))?;
    println!(
        "{count} results for {} ({stored} new objects) -> {}",
        tier.name,
        tier_dir.display()
    );
    ensure!(
        dm_status.success(),
        "DM exited with {dm_status}; results that rendered were kept (see its log above)"
    );
    Ok(())
}

/// Moves a DM run's outputs from `staging` into the object store and merges their
/// hashes and metadata into the tier's `hashes.json` / `meta.json`.
/// Returns how many objects were new.
fn ingest(root: &Path, tier_dir: &Path, staging: &Path) -> Result<usize> {
    let pin_dir = pin_golden_dir(root)?;
    let mut hashes: BTreeMap<String, String> = read_json_map(&tier_dir.join("hashes.json"))?;
    let mut metas: BTreeMap<String, serde_json::Value> =
        read_json_map(&tier_dir.join("meta.json"))?;
    let mut stored = 0;
    for (id, path) in outputs(staging)? {
        let bytes = std::fs::read(&path)?;
        let sha = sha256_hex(&bytes);
        let object = object_path(&pin_dir, &sha);
        if !object.exists() {
            std::fs::create_dir_all(object.parent().context("object has no parent")?)?;
            let compressed = zstd::encode_all(bytes.as_slice(), 19)?;
            // Write then rename, so an interrupted run never leaves a truncated object.
            let tmp = object.with_extension("tmp");
            std::fs::write(&tmp, compressed)?;
            std::fs::rename(&tmp, &object)?;
            stored += 1;
        }
        let meta_text = std::fs::read_to_string(path.with_extension("json"))
            .with_context(|| format!("metadata for {id}"))?;
        let mut meta: serde_json::Value = serde_json::from_str(&meta_text)?;
        if let Some(obj) = meta.as_object_mut() {
            // Tier facts live in toolchain.txt; keep meta.json about the output only.
            for key in ["cpu_x64_level", "cpu_cap", "cpu_tier"] {
                obj.remove(key);
            }
        }
        hashes.insert(id.clone(), sha);
        metas.insert(id, meta);
    }
    write_json(&tier_dir.join("hashes.json"), &hashes)?;
    write_json(&tier_dir.join("meta.json"), &metas)?;
    Ok(stored)
}

/// Writes the raw bytes of one golden (`<config>/<src>/[<options>/]<name>`) to `dest`.
pub fn extract(root: &Path, tier: &str, id: &str, dest: &Path) -> Result<()> {
    let hashes: BTreeMap<String, String> =
        read_json_map(&golden_dir(root, tier)?.join("hashes.json"))?;
    let sha = hashes
        .get(id)
        .with_context(|| format!("no golden `{id}` in tier {tier}"))?;
    let object = object_path(&pin_golden_dir(root)?, sha);
    let compressed =
        std::fs::read(&object).with_context(|| format!("reading {}", object.display()))?;
    let bytes = zstd::decode_all(compressed.as_slice())?;
    ensure!(
        sha256_hex(&bytes) == *sha,
        "object {} is corrupt",
        object.display()
    );
    std::fs::write(dest, bytes)?;
    println!("{id} ({tier}) -> {}", dest.display());
    Ok(())
}

/// Verifies every result's metadata reports the expected runtime CPU tier.
fn check_tier(dir: &Path, expected: &str) -> Result<usize> {
    let mut count = 0;
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        if entry.path().extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text = std::fs::read_to_string(entry.path())?;
        let meta: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", entry.path().display()))?;
        let Some(ran) = meta.get("cpu_tier").and_then(serde_json::Value::as_str) else {
            continue; // hashes.json and other non-result files
        };
        ensure!(
            ran == expected,
            "{} ran at CPU tier `{ran}`, expected `{expected}`; this host can't run the tier",
            entry.path().display()
        );
        count += 1;
    }
    Ok(count)
}

fn write_toolchain(root: &Path, out: &Path, tier: &Tier) -> Result<()> {
    let mut text = String::new();
    writeln!(text, "tier = {}", tier.name)?;
    writeln!(text, "build = {}", tier.build)?;
    writeln!(text, "cpu_cap = {}", tier.level)?;
    if let Some(gpu) = &tier.gpu {
        writeln!(text, "dawn_adapter = {}", gpu.adapter)?;
    }
    if cfg!(windows) {
        let clang = Path::new(&clang_win()?).join("bin").join("clang-cl.exe");
        let version = Command::new(clang).arg("--version").output()?;
        writeln!(
            text,
            "compiler = {}",
            String::from_utf8_lossy(&version.stdout)
                .lines()
                .next()
                .unwrap_or("")
        )?;
    }
    let args = std::fs::read_to_string(build_dir(root, &tier.build).join("args.gn"))?;
    writeln!(text, "\n[args.gn]\n{args}")?;
    std::fs::write(out.join("toolchain.txt"), text)?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Every `.raw`/`.bin` output under `dir`, keyed by its path relative to `dir` with
/// `/` separators and no extension (the result id).
fn outputs(dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut found = BTreeMap::new();
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "raw" || e == "bin") {
            continue;
        }
        let rel = path
            .strip_prefix(dir)?
            .with_extension("")
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        found.insert(rel, path.to_path_buf());
    }
    Ok(found)
}

/// Hashes every output under `dir` (see [`outputs`]).
pub fn hash_dir(dir: &Path) -> Result<BTreeMap<String, String>> {
    outputs(dir)?
        .into_iter()
        .map(|(id, path)| Ok((id, sha256_hex(&std::fs::read(path)?))))
        .collect()
}

/// Compares a directory of skia-rust outputs (same layout as the goldens) against a tier.
/// Returns an error listing every mismatch; outputs with no golden are reported too.
pub fn compare(root: &Path, tier: &str, ours: &Path) -> Result<()> {
    let golden_path = golden_dir(root, tier)?.join("hashes.json");
    ensure!(
        golden_path.exists(),
        "no goldens at {}",
        golden_path.display()
    );
    let goldens: BTreeMap<String, String> = read_json_map(&golden_path)?;
    let ours = hash_dir(ours)?;
    ensure!(!ours.is_empty(), "no .raw/.bin outputs found");

    let mut failures = Vec::new();
    for (id, hash) in &ours {
        match goldens.get(id) {
            Some(golden) if golden == hash => {}
            Some(golden) => failures.push(format!("MISMATCH {id}: ours {hash}, oracle {golden}")),
            None => failures.push(format!("NO GOLDEN {id}")),
        }
    }
    let matched = ours.len() - failures.len();
    println!("{matched}/{} outputs match {tier}", ours.len());
    if failures.is_empty() {
        return Ok(());
    }
    for f in &failures {
        println!("  {f}");
    }
    bail!(
        "{} of {} outputs differ from the oracle",
        failures.len(),
        ours.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(text: &str) -> Config {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn tiers_cover_every_runtime_level_from_the_build_level_up() {
        let c = config(
            r#"
            [gn]
            args = []
            [[build]]
            name = "x64-sse2"
            level = "baseline"
            args = []
            [[build]]
            name = "x64-v3"
            level = "ml3"
            args = []
            [[build]]
            name = "gpu"
            gpu = true
            args = []
            [[gpu_tier]]
            name = "gpu-warp"
            build = "gpu"
            configs = ["grdawn_d3d12"]
            adapter = "Microsoft Basic Render Driver"
            cpu_level = "ml3"
            "#,
        );
        let names: Vec<_> = derive_tiers(&c)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(
            names,
            [
                "cpu-x64-sse2",
                "cpu-x64-sse2-rt-ssse3",
                "cpu-x64-sse2-rt-ml3",
                "cpu-x64-sse2-rt-ml4",
                "cpu-x64-v3",
                "cpu-x64-v3-rt-ml4",
                "gpu-warp",
            ]
        );
    }

    #[test]
    fn checked_in_config_parses() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let tiers = derive_tiers(&read_config(root).unwrap()).unwrap();
        assert!(tiers.iter().any(|t| t.name == "cpu-x64-sse2-rt-ml4"));
    }

    #[test]
    fn hash_dir_keys_are_relative_without_extension() {
        let dir = std::env::temp_dir().join(format!("xtask-hash-{}", std::process::id()));
        let sub = dir.join("8888").join("gm");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("aarectmodes.raw"), b"abc").unwrap();
        std::fs::write(sub.join("aarectmodes.json"), b"{}").unwrap();
        let hashes = hash_dir(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            hashes.get("8888/gm/aarectmodes").map(String::as_str),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(hashes.len(), 1);
    }
}
