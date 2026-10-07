//! The pinned Skia checkout used as test source and oracle source.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Contents of `inventory/skia-pin.toml`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pin {
    /// Skia release branch, e.g. `chrome/m156`.
    pub branch: String,
    /// Full commit hash the project is tested against.
    pub commit: String,
}

pub const SKIA_URL: &str = "https://skia.googlesource.com/skia";

pub fn pin_path(root: &Path) -> PathBuf {
    root.join("inventory").join("skia-pin.toml")
}

pub fn checkout_path(root: &Path) -> PathBuf {
    root.join("third_party").join("skia")
}

pub fn read_pin(root: &Path) -> Result<Pin> {
    let path = pin_path(root);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Commit currently checked out in `third_party/skia`.
pub fn checkout_commit(root: &Path) -> Result<String> {
    git(&checkout_path(root), &["rev-parse", "HEAD"])
}

pub fn fetch(root: &Path) -> Result<()> {
    let pin = read_pin(root)?;
    let dir = checkout_path(root);
    if dir.exists() {
        let head = checkout_commit(root)?;
        if head == pin.commit {
            println!(
                "third_party/skia already at {} ({})",
                pin.branch, pin.commit
            );
            return fetch_api_reference(root);
        }
        bail!(
            "third_party/skia is at {head}, pin is {}; delete the directory and re-run",
            pin.commit
        );
    }
    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--single-branch",
            "--branch",
            &pin.branch,
            SKIA_URL,
        ])
        .arg(&dir)
        .status()
        .context("running git clone")?;
    if !status.success() {
        bail!("git clone failed");
    }
    let head = checkout_commit(root)?;
    if head != pin.commit {
        // The branch moved past the pin; fetch the pinned commit explicitly.
        git(&dir, &["fetch", "--depth", "1", "origin", &pin.commit])?;
        git(&dir, &["checkout", "--detach", &pin.commit])?;
    }
    println!("fetched Skia {} ({})", pin.branch, pin.commit);
    fetch_api_reference(root)
}

/// Contents of `inventory/api-reference.toml`: the rust-skia commit whose `skia-safe`
/// API skia-rust mirrors.
#[derive(Debug, Deserialize)]
struct ApiReference {
    repo: String,
    commit: String,
}

/// Fetches rust-skia at the pinned commit into `third_party/rust-skia` (reference only;
/// never built or depended on).
fn fetch_api_reference(root: &Path) -> Result<()> {
    let path = root.join("inventory").join("api-reference.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let api: ApiReference =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    let dir = root.join("third_party").join("rust-skia");
    if !dir.join(".git").exists() {
        std::fs::create_dir_all(&dir)?;
        git(&dir, &["init", "-q"])?;
        git(&dir, &["remote", "add", "origin", &api.repo])?;
    }
    if git(&dir, &["rev-parse", "HEAD"]).ok().as_deref() != Some(api.commit.as_str()) {
        git(
            &dir,
            &["fetch", "-q", "--depth", "1", "origin", &api.commit],
        )?;
        git(&dir, &["checkout", "-q", "--detach", &api.commit])?;
    }
    println!(
        "API reference: rust-skia {} in third_party/rust-skia",
        &api.commit[..12]
    );
    Ok(())
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}
