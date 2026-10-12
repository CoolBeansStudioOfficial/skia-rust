// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The oracle's goldens (`oracle/README.md`): result id → SHA-256 per oracle tier, and the raw
//! bytes behind each hash (for diff images).
//!
//! Where they come from, in order:
//! 1. `$SKIA_RUST_GOLDENS` (a `goldens/` directory, or directly its `<skia-commit>/` directory;
//!    the value `release` skips 1–3);
//! 2. `goldens/<skia-commit>/` in the workspace;
//! 3. `goldens/<skia-commit>/` in the main checkout, when the workspace is a git worktree;
//! 4. the release pinned in `inventory/goldens.lock`: `hashes-<mNNN>.json` is downloaded once,
//!    verified against the lock's SHA-256 and cached in `target/goldens/<release>/`. Golden
//!    objects come from `objects-<mNNN>.tar` (also verified and cached), fetched only when a
//!    mismatch needs a diff image.
//!
//! # DM sources with their own goldens
//! The goldens of DM's other sources (`--src image`, later `--src lottie`) are separate release
//! assets rendered by the CI-hosted oracle (`oracle/README.md`, "CI-hosted oracle"), in the same
//! format as `hashes-<mNNN>.json` and without objects: [`DmSource`], [`GoldenStore::load_dm`].
//! They come from `$<DmSource::env>` (a local file; used until the asset is published), else
//! from the lock's `dm-<src>-hashes-<mNNN>.json` (downloaded and verified like the others). When
//! neither works the result is [`DmGoldens::Unavailable`] with the reason, and the caller decides
//! whether that is an error (`cargo xtask inventory verify` does: see `xtask/src/verify_dm.rs`).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Result id → SHA-256 (hex) of its raw bytes.
pub type Hashes = BTreeMap<String, String>;

/// The workspace root.
///
/// # Panics
/// Never: the crate is always two levels below the root.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tests/gm is two levels below the workspace root")
        .to_path_buf()
}

/// Cargo's target directory.
#[must_use]
pub fn target_dir() -> PathBuf {
    let root = workspace_root();
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => root.join(dir),
        None => root.join("target"),
    }
}

/// SHA-256 of `bytes`, lowercase hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The value of `key = "value"` in a flat TOML file (the pin and lock files are written by
/// xtask in exactly this form).
fn toml_string(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim().trim_matches('"') == key).then(|| v.trim().trim_matches('"').to_owned())
    })
}

/// The Skia commit the goldens must have been rendered from (`inventory/skia-pin.toml`).
///
/// # Errors
/// If the pin file is missing or has no `commit`.
pub fn pinned_commit(root: &Path) -> Result<String, String> {
    let path = root.join("inventory").join("skia-pin.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml_string(&text, "commit").ok_or_else(|| format!("no commit in {}", path.display()))
}

/// `inventory/goldens.lock`: the release and its assets' SHA-256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    pub release: String,
    /// Asset name → SHA-256.
    pub assets: BTreeMap<String, String>,
}

impl Lock {
    /// Parses the lock file text.
    ///
    /// # Errors
    /// If there is no `release`.
    pub fn parse(text: &str) -> Result<Lock, String> {
        let release = toml_string(text, "release").ok_or("goldens.lock has no release")?;
        let mut assets = BTreeMap::new();
        let mut in_assets = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                in_assets = line == "[assets]";
                continue;
            }
            if let (true, Some((k, v))) = (in_assets, line.split_once('=')) {
                assets.insert(
                    k.trim().trim_matches('"').to_owned(),
                    v.trim().trim_matches('"').to_owned(),
                );
            }
        }
        Ok(Lock { release, assets })
    }

    /// Reads `inventory/goldens.lock`.
    ///
    /// # Errors
    /// If it cannot be read or parsed.
    pub fn read(root: &Path) -> Result<Lock, String> {
        let path = root.join("inventory").join("goldens.lock");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Lock::parse(&text)
    }

    /// The asset whose name starts with `prefix` (`hashes-`, `objects-`): (name, SHA-256).
    #[must_use]
    pub fn asset(&self, prefix: &str) -> Option<(&str, &str)> {
        self.assets
            .iter()
            .find(|(name, _)| name.starts_with(prefix))
            .map(|(n, s)| (n.as_str(), s.as_str()))
    }

    fn url(&self, asset: &str) -> String {
        format!(
            "{}/releases/download/{}/{asset}",
            env!("CARGO_PKG_REPOSITORY"),
            self.release
        )
    }
}

/// A DM source (`--src <name>`) whose goldens are their own release asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmSource {
    /// DM's `--src` name; the second component of the result ids.
    pub name: &'static str,
    /// The lock asset's name up to the Skia milestone: `dm-image-hashes-` for
    /// `dm-image-hashes-m156.json`.
    pub asset_prefix: &'static str,
    /// The environment variable that names a local hashes file instead of the release asset.
    pub env: &'static str,
}

/// DM's `--src image` (`dm-image-hashes-m156.json`).
pub const DM_IMAGE: DmSource = DmSource {
    name: "image",
    asset_prefix: "dm-image-hashes-",
    env: "SKIA_RUST_DM_IMAGE_HASHES",
};

/// DM's `--src lottie` (`dm-lottie-hashes-m156.json`; rendered with `--config 8888` only).
pub const DM_LOTTIE: DmSource = DmSource {
    name: "lottie",
    asset_prefix: "dm-lottie-hashes-",
    env: "SKIA_RUST_DM_LOTTIE_HASHES",
};

/// The goldens of a [`DmSource`], or why there are none.
#[derive(Debug)]
pub enum DmGoldens {
    Available(GoldenStore),
    /// Neither the environment override nor the lock's asset could be used.
    Unavailable(String),
}

/// Where golden objects (the raw bytes) come from.
#[derive(Clone, Debug)]
pub enum Objects {
    /// A local `objects/` directory (`<aa>/<sha>.zst`).
    Dir(PathBuf),
    /// The release's `objects-<mNNN>.tar`, downloaded into `cache` on first use.
    Release { lock: Lock, cache: PathBuf },
    /// No objects (fixtures).
    None,
}

/// The release bundle's `hashes-<mNNN>.json` (written by `cargo xtask oracle publish`).
#[derive(Debug, Deserialize)]
struct ReleaseHashes {
    skia_commit: String,
    tiers: BTreeMap<String, ReleaseTier>,
}

#[derive(Debug, Deserialize)]
struct ReleaseTier {
    results: Hashes,
}

/// Golden hashes for every oracle tier, and access to the objects behind them.
#[derive(Debug)]
pub struct GoldenStore {
    /// Where the hashes were loaded from (for messages).
    pub source: String,
    tiers: BTreeMap<String, Hashes>,
    objects: Objects,
}

impl GoldenStore {
    /// A store over in-memory hashes.
    #[must_use]
    pub fn new(source: String, tiers: BTreeMap<String, Hashes>, objects: Objects) -> Self {
        Self {
            source,
            tiers,
            objects,
        }
    }

    /// Loads `<pin_dir>/<tier>/hashes.json` for every tier directory, with `<pin_dir>/objects`.
    ///
    /// # Errors
    /// If a hashes file cannot be read or parsed.
    pub fn from_pin_dir(pin_dir: &Path) -> Result<Self, String> {
        let mut tiers = BTreeMap::new();
        let entries =
            std::fs::read_dir(pin_dir).map_err(|e| format!("{}: {e}", pin_dir.display()))?;
        for entry in entries {
            let dir = entry.map_err(|e| e.to_string())?.path();
            let path = dir.join("hashes.json");
            if !path.is_file() {
                continue;
            }
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("non-UTF-8 tier directory")?
                .to_owned();
            tiers.insert(name, read_json::<Hashes>(&path)?);
        }
        Ok(Self::new(
            pin_dir.display().to_string(),
            tiers,
            Objects::Dir(pin_dir.join("objects")),
        ))
    }

    /// Loads a release bundle's `hashes-<mNNN>.json` and checks it was rendered from `commit`.
    ///
    /// # Errors
    /// If it cannot be read or parsed, or is for another Skia commit.
    pub fn from_release_json(path: &Path, commit: &str, objects: Objects) -> Result<Self, String> {
        let hashes: ReleaseHashes = read_json(path)?;
        if hashes.skia_commit != commit {
            return Err(format!(
                "{} is for Skia {}, the pin is {commit}",
                path.display(),
                hashes.skia_commit
            ));
        }
        let tiers = hashes
            .tiers
            .into_iter()
            .map(|(name, t)| (name, t.results))
            .collect();
        Ok(Self::new(path.display().to_string(), tiers, objects))
    }

    /// Loads the goldens for the pinned Skia commit (see the module docs for the search order).
    ///
    /// # Errors
    /// If no local goldens exist and the release cannot be downloaded or verified.
    pub fn load() -> Result<Self, String> {
        let root = workspace_root();
        let commit = pinned_commit(&root)?;
        let release_only = std::env::var_os("SKIA_RUST_GOLDENS").is_some_and(|v| v == "release");
        if let Some(pin_dir) = local_pin_dirs(&root, &commit)
            .into_iter()
            .find(|d| !release_only && has_tier_hashes(d))
        {
            return Self::from_pin_dir(&pin_dir);
        }
        let lock = Lock::read(&root)?;
        let cache = target_dir().join("goldens").join(&lock.release);
        let (name, sha) = lock
            .asset("hashes-")
            .ok_or("goldens.lock lists no hashes-*.json asset")?;
        let path = fetch_verified(&lock, name, sha, &cache).map_err(|e| e.to_string())?;
        Self::from_release_json(&path, &commit, Objects::Release { lock, cache })
    }

    /// Loads the goldens of a DM source: `$<source.env>` if set (a hashes file in the format of
    /// `hashes-<mNNN>.json`), else the lock's asset (downloaded and verified against the lock).
    ///
    /// # Errors
    /// Only for goldens that exist and are wrong: the override file cannot be read or is for
    /// another Skia commit, or the downloaded asset does not match the lock's SHA-256. A source
    /// with no override and no reachable asset is [`DmGoldens::Unavailable`].
    pub fn load_dm(source: &DmSource) -> Result<DmGoldens, String> {
        let file = std::env::var_os(source.env).filter(|v| !v.is_empty());
        Self::load_dm_with(source, file.as_deref().map(Path::new))
    }

    /// [`GoldenStore::load_dm`] with the override file given instead of read from the environment.
    ///
    /// # Errors
    /// As [`GoldenStore::load_dm`].
    pub fn load_dm_with(source: &DmSource, file: Option<&Path>) -> Result<DmGoldens, String> {
        let root = workspace_root();
        let commit = pinned_commit(&root)?;
        if let Some(file) = file {
            let store = Self::from_release_json(file, &commit, Objects::None)?;
            return Ok(DmGoldens::Available(store));
        }
        let lock = Lock::read(&root)?;
        let Some((name, sha)) = lock.asset(source.asset_prefix) else {
            return Ok(DmGoldens::Unavailable(format!(
                "inventory/goldens.lock lists no {}*.json asset and ${} is not set",
                source.asset_prefix, source.env
            )));
        };
        let cache = target_dir().join("goldens").join(&lock.release);
        match fetch_verified(&lock, name, sha, &cache) {
            Ok(path) => {
                Self::from_release_json(&path, &commit, Objects::None).map(DmGoldens::Available)
            }
            Err(FetchError::Download(e)) => Ok(DmGoldens::Unavailable(format!(
                "{name} is not downloadable (not published to {} yet?) and ${} is not set: {e}",
                lock.release, source.env
            ))),
            Err(e @ FetchError::Corrupt(_)) => Err(e.to_string()),
        }
    }

    /// The goldens of a DM source in this process, loaded once per source.
    ///
    /// # Errors
    /// The load error (see [`GoldenStore::load_dm`]), repeated on every call.
    pub fn shared_dm(source: &DmSource) -> Result<&'static DmGoldens, String> {
        type Loaded = Result<&'static DmGoldens, String>;
        static STORES: Mutex<BTreeMap<&'static str, Loaded>> = Mutex::new(BTreeMap::new());
        STORES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(source.name)
            .or_insert_with(|| GoldenStore::load_dm(source).map(|g| &*Box::leak(Box::new(g))))
            .clone()
    }

    /// The goldens of this process, loaded once.
    ///
    /// # Errors
    /// The load error (see [`GoldenStore::load`]), repeated on every call.
    pub fn shared() -> Result<&'static GoldenStore, String> {
        static STORE: OnceLock<Result<GoldenStore, String>> = OnceLock::new();
        STORE
            .get_or_init(GoldenStore::load)
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Whether the store has hashes for `tier`.
    #[must_use]
    pub fn has_tier(&self, tier: &str) -> bool {
        self.tiers.contains_key(tier)
    }

    /// The tiers with hashes.
    pub fn tiers(&self) -> impl Iterator<Item = &str> {
        self.tiers.keys().map(String::as_str)
    }

    /// Whether oracle tier `tier` has any result of config `config` (`8888`, `565`, `f16`). The
    /// goldens of DM's file sources do not record every config on every tier (the byte-order
    /// variants of the CI-hosted oracle hold `8888` only).
    #[must_use]
    pub fn has_config(&self, tier: &str, config: &str) -> bool {
        let prefix = format!("{config}/");
        self.tiers.get(tier).is_some_and(|t| {
            t.range(prefix.clone()..)
                .next()
                .is_some_and(|(id, _)| id.starts_with(&prefix))
        })
    }

    /// The result ids with a golden on oracle tier `tier`.
    pub fn ids(&self, tier: &str) -> impl Iterator<Item = &str> {
        self.tiers
            .get(tier)
            .into_iter()
            .flat_map(|t| t.keys().map(String::as_str))
    }

    /// The golden hash of result `id` on oracle tier `tier`.
    #[must_use]
    pub fn golden(&self, tier: &str, id: &str) -> Option<&str> {
        self.tiers.get(tier)?.get(id).map(String::as_str)
    }

    /// The raw bytes of the golden with hash `sha`, verified against it.
    ///
    /// # Errors
    /// If the objects are unavailable, or the object is missing or corrupt.
    pub fn object(&self, sha: &str) -> Result<Vec<u8>, String> {
        if sha.len() < 2 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("bad object hash `{sha}`"));
        }
        let member = format!("objects/{}/{sha}.zst", &sha[..2]);
        let path = match &self.objects {
            Objects::None => return Err("this golden store has no objects".to_owned()),
            Objects::Dir(dir) => dir.join(&sha[..2]).join(format!("{sha}.zst")),
            Objects::Release { lock, cache } => {
                let path = cache.join(&member);
                if !path.is_file() {
                    let (name, tar_sha) = lock
                        .asset("objects-")
                        .ok_or("goldens.lock lists no objects-*.tar asset")?;
                    fetch_verified(lock, name, tar_sha, cache).map_err(|e| e.to_string())?;
                    run(Command::new("tar")
                        .current_dir(cache)
                        .args(["-xf", name, &member]))?;
                }
                path
            }
        };
        let compressed = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let bytes = zstd::decode_all(compressed.as_slice())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256_hex(&bytes) != sha {
            return Err(format!("object {} is corrupt", path.display()));
        }
        Ok(bytes)
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))
}

fn has_tier_hashes(pin_dir: &Path) -> bool {
    std::fs::read_dir(pin_dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|e| e.path().join("hashes.json").is_file())
    })
}

/// Candidate local `goldens/<commit>/` directories, in search order.
fn local_pin_dirs(root: &Path, commit: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("SKIA_RUST_GOLDENS") {
        let dir = PathBuf::from(dir);
        // `$SKIA_RUST_GOLDENS` may name the commit directory itself.
        if dir.ends_with(commit) {
            dirs.push(dir);
        } else {
            dirs.push(dir.join(commit));
        }
    }
    dirs.push(root.join("goldens").join(commit));
    if let Some(main) = main_checkout(root) {
        dirs.push(main.join("goldens").join(commit));
    }
    dirs
}

/// The main checkout when `root` is a git worktree (its `.git` is a file
/// `gitdir: <main>/.git/worktrees/<name>`).
fn main_checkout(root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(root.join(".git")).ok()?;
    let gitdir = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
    let dot_git = gitdir.parent()?.parent()?;
    (dot_git.file_name()? == ".git").then(|| dot_git.parent().map(Path::to_path_buf))?
}

fn run(cmd: &mut Command) -> Result<(), String> {
    let out = cmd.output().map_err(|e| format!("running {cmd:?}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{cmd:?} failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Why [`fetch_verified`] failed.
#[derive(Debug)]
enum FetchError {
    /// The asset could not be downloaded (not published, no network, ...).
    Download(String),
    /// The asset was downloaded but is not the one the lock pins, or the cache cannot be used.
    Corrupt(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Download(e) | FetchError::Corrupt(e) => f.write_str(e),
        }
    }
}

/// `cache/<asset>`, downloaded from the lock's release if needed, with its SHA-256 checked
/// against the lock (once per download; a `.sha256` stamp records a verified file).
fn fetch_verified(
    lock: &Lock,
    asset: &str,
    sha: &str,
    cache: &Path,
) -> Result<PathBuf, FetchError> {
    let path = cache.join(asset);
    let stamp = cache.join(format!("{asset}.sha256"));
    if path.is_file() && std::fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == sha) {
        return Ok(path);
    }
    std::fs::create_dir_all(cache)
        .map_err(|e| FetchError::Corrupt(format!("{}: {e}", cache.display())))?;
    // Download under a unique name, then rename: concurrent test processes never see a partial
    // file.
    let tmp_name = format!("{asset}.{}.part", std::process::id());
    let url = lock.url(asset);
    let curl = run(Command::new("curl")
        .current_dir(cache)
        .args(["-fsSL", "--retry", "3", "-o", &tmp_name, &url]));
    if let Err(curl_err) = curl {
        let repo = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
        let gh = run(Command::new("gh").current_dir(cache).args([
            "release",
            "download",
            &lock.release,
            "-R",
            repo,
            "-p",
            asset,
            "-O",
            &tmp_name,
            "--clobber",
        ]));
        if let Err(gh_err) = gh {
            let _ = std::fs::remove_file(cache.join(&tmp_name));
            return Err(FetchError::Download(format!(
                "downloading {url} failed (curl: {curl_err}; gh: {gh_err}); set \
                 SKIA_RUST_GOLDENS to a local goldens directory instead"
            )));
        }
    }
    let tmp = cache.join(&tmp_name);
    let bytes =
        std::fs::read(&tmp).map_err(|e| FetchError::Corrupt(format!("{}: {e}", tmp.display())))?;
    let got = sha256_hex(&bytes);
    drop(bytes);
    if got != sha {
        let _ = std::fs::remove_file(&tmp);
        return Err(FetchError::Corrupt(format!(
            "{url} has SHA-256 {got}, inventory/goldens.lock says {sha}"
        )));
    }
    std::fs::rename(&tmp, &path)
        .map_err(|e| FetchError::Corrupt(format!("{}: {e}", path.display())))?;
    std::fs::write(&stamp, sha)
        .map_err(|e| FetchError::Corrupt(format!("{}: {e}", stamp.display())))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_checked_in_lock() {
        let lock = Lock::read(&workspace_root()).unwrap();
        assert!(lock.release.starts_with("goldens-m"));
        let (name, sha) = lock.asset("hashes-").unwrap();
        assert!(Path::new(name).extension().is_some_and(|e| e == "json"));
        assert_eq!(sha.len(), 64);
        assert!(lock.asset("objects-").is_some());
        assert!(
            lock.url(name)
                .ends_with(&format!("/releases/download/{}/{name}", lock.release))
        );
    }

    #[test]
    fn lock_comments_are_not_assets_and_dm_assets_are_found_by_prefix() {
        let lock = Lock::parse(
            "# comment with = in it\nrelease = \"goldens-m1\"\n\n[assets]\n\
             \"hashes-m1.json\" = \"aa\"\n# another = one\n\
             \"dm-image-hashes-m1.json\" = \"bb\"\n",
        )
        .unwrap();
        assert_eq!(lock.assets.len(), 2);
        assert_eq!(lock.asset("hashes-"), Some(("hashes-m1.json", "aa")));
        assert_eq!(
            lock.asset(DM_IMAGE.asset_prefix),
            Some(("dm-image-hashes-m1.json", "bb"))
        );
    }

    #[test]
    fn the_checked_in_lock_pins_the_dm_image_hashes() {
        let lock = Lock::read(&workspace_root()).unwrap();
        let (name, sha) = lock.asset(DM_IMAGE.asset_prefix).unwrap();
        assert!(
            Path::new(name).extension().is_some_and(|e| e == "json"),
            "{name}"
        );
        assert_eq!(sha.len(), 64);
        assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()));
        let (name, sha) = lock.asset(DM_LOTTIE.asset_prefix).unwrap();
        assert_eq!(name, "dm-lottie-hashes-m156.json");
        assert_eq!(sha.len(), 64);
        // The plain hashes asset is still the GM goldens.
        assert_eq!(lock.asset("hashes-").unwrap().0, "hashes-m156.json");
    }

    #[test]
    fn dm_goldens_come_from_a_hashes_file_of_the_pinned_commit() {
        let dir = std::env::temp_dir().join(format!("skia-rust-gm-dm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("dm-image-hashes.json");
        let json = |commit: &str| {
            format!(
                "{{\"skia_branch\":\"b\",\"skia_commit\":\"{commit}\",\"tiers\":\
                 {{\"cpu-x64-sse2\":{{\"results\":{{\"565/image/codec_premul/a.png\":\"ff\"}}}}}}}}"
            )
        };
        std::fs::write(&file, json("c0ffee")).unwrap();
        let store = GoldenStore::from_release_json(&file, "c0ffee", Objects::None).unwrap();
        assert_eq!(
            store.golden("cpu-x64-sse2", "565/image/codec_premul/a.png"),
            Some("ff")
        );
        assert_eq!(
            store.ids("cpu-x64-sse2").collect::<Vec<_>>(),
            ["565/image/codec_premul/a.png"]
        );
        assert_eq!(store.ids("cpu-x64-sse41").count(), 0);
        // Hashes rendered from another Skia are refused.
        let err = GoldenStore::from_release_json(&file, "other", Objects::None).unwrap_err();
        assert!(err.contains("c0ffee") && err.contains("other"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dm_goldens_are_unavailable_without_a_file_or_an_asset() {
        let source = DmSource {
            name: "none",
            asset_prefix: "dm-none-hashes-",
            env: "SKIA_RUST_DM_NONE_HASHES",
        };
        match GoldenStore::load_dm_with(&source, None).unwrap() {
            DmGoldens::Unavailable(reason) => {
                assert!(reason.contains("dm-none-hashes-") && reason.contains(source.env));
            }
            DmGoldens::Available(_) => panic!("there is no such asset"),
        }
        // A named file that is wrong is an error, not an unavailable source.
        let missing = Path::new("/nonexistent/dm-hashes.json");
        assert!(GoldenStore::load_dm_with(&source, Some(missing)).is_err());
    }

    #[test]
    fn pinned_commit_is_a_sha() {
        let commit = pinned_commit(&workspace_root()).unwrap();
        assert_eq!(commit.len(), 40);
    }

    #[test]
    fn local_objects_round_trip() {
        let dir = std::env::temp_dir().join(format!("skia-rust-gm-objects-{}", std::process::id()));
        let bytes = b"golden bytes".to_vec();
        let sha = sha256_hex(&bytes);
        let sub = dir.join(&sha[..2]);
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join(format!("{sha}.zst")),
            zstd::encode_all(bytes.as_slice(), 3).unwrap(),
        )
        .unwrap();
        let store = GoldenStore::new(
            "test".to_owned(),
            BTreeMap::new(),
            Objects::Dir(dir.clone()),
        );
        assert_eq!(store.object(&sha).unwrap(), bytes);
        assert!(store.object(&sha256_hex(b"other")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
