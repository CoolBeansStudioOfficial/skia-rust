// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Third-party inputs of the text modules, fetched on demand and cached outside the repository
//! (`docs/design/modules.md` §9, tasks M0b and M8).
//!
//! * **HarfBuzz** and **Chromium ICU** at Skia's `DEPS` pins (`inventory/text-deps.lock`), for the
//!   differential tools (`oracle/shaper-diff`, `oracle/unicode-diff`). They are only needed
//!   to *regenerate* expected files; `cargo test` replays the committed ones.
//! * The **UAX conformance files** of Unicode 17.0.0 (`[ucd]` of the same lock file), which `oracle/unicode-diff`
//!   replays like the fonts: fetched from unicode.org, verified and cached.
//! * The **paragraph test fonts** (`inventory/test-fonts.lock`): the release asset
//!   `test-fonts-m156`, fetched the way `tests/gm` fetches the goldens (curl, then `gh release
//!   download`), verified against the lock and cached.
//!
//! The cache directory is, in order: `$SKIA_RUST_TEXT_DEPS`, `$XDG_CACHE_HOME/skia-rust/text-deps`,
//! `%LOCALAPPDATA%\skia-rust\text-deps`, `$HOME/.cache/skia-rust/text-deps`.

// `HarfBuzz`, `HarfRust` and `FreeType` are proper names, not code.
#![allow(clippy::doc_markdown)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

/// The workspace root.
///
/// # Panics
/// Never: the crate is always two levels below the root.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("oracle/text-deps is two levels below the workspace root")
        .to_path_buf()
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

/// SHA-256 of a file, streamed.
///
/// # Errors
/// If the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut out = String::with_capacity(64);
    for b in hasher.finalize() {
        let _ = write!(out, "{b:02x}");
    }
    Ok(out)
}

/// The cache directory (see the module docs). Not created.
#[must_use]
pub fn cache_root() -> PathBuf {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = var("SKIA_RUST_TEXT_DEPS") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = var("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("skia-rust").join("text-deps");
    }
    if let Some(dir) = var("LOCALAPPDATA") {
        return PathBuf::from(dir).join("skia-rust").join("text-deps");
    }
    if let Some(home) = var("HOME").or_else(|| var("USERPROFILE")) {
        return PathBuf::from(home)
            .join(".cache")
            .join("skia-rust")
            .join("text-deps");
    }
    std::env::temp_dir().join("skia-rust-text-deps")
}

// ---------------------------------------------------------------------------------------------
// Lock files (flat TOML, written by hand in exactly this form).

/// A parsed lock file: `section -> key -> value`. Keys before the first `[section]` are in `""`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lock(pub BTreeMap<String, BTreeMap<String, String>>);

impl Lock {
    /// Parses `key = "value"` lines grouped under `[section]` headers; `#` lines are comments.
    #[must_use]
    pub fn parse(text: &str) -> Lock {
        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut current = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                name.trim().clone_into(&mut current);
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(unquote(k), unquote(v));
            }
        }
        Lock(sections)
    }

    /// Reads `inventory/<name>`.
    ///
    /// # Errors
    /// If the file cannot be read.
    pub fn read(name: &str) -> Result<Lock, String> {
        let path = workspace_root().join("inventory").join(name);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Lock::parse(&text))
    }

    /// The value of `key` in `section`.
    ///
    /// # Errors
    /// If it is missing.
    pub fn get(&self, section: &str, key: &str) -> Result<&str, String> {
        self.0
            .get(section)
            .and_then(|s| s.get(key))
            .map(String::as_str)
            .ok_or_else(|| format!("lock file has no `{key}` in [{section}]"))
    }

    /// All entries of `section`.
    #[must_use]
    pub fn section(&self, section: &str) -> BTreeMap<String, String> {
        self.0.get(section).cloned().unwrap_or_default()
    }
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').to_owned()
}

// ---------------------------------------------------------------------------------------------
// Downloading and extracting.

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

fn curl(url: &str, dest: &Path) -> Result<(), String> {
    run(Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(dest)
        .arg(url))
}

/// A fresh, empty sibling directory of `final_dir` to build into before an atomic rename.
fn staging_dir(final_dir: &Path) -> Result<PathBuf, String> {
    let name = final_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad cache path")?;
    let staging = final_dir.with_file_name(format!("{name}.{}.part", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    Ok(staging)
}

fn publish_dir(staging: &Path, final_dir: &Path) -> Result<(), String> {
    match std::fs::rename(staging, final_dir) {
        Ok(()) => Ok(()),
        // Another process finished first: keep its (equally verified) directory.
        Err(_) if final_dir.is_dir() => {
            let _ = std::fs::remove_dir_all(staging);
            Ok(())
        }
        Err(e) => Err(format!("{}: {e}", final_dir.display())),
    }
}

/// Decodes standard base64 (padding and ASCII whitespace allowed): googlesource serves raw files
/// as `?format=TEXT`, which is base64.
///
/// # Errors
/// On a character outside the alphabet.
pub fn base64_decode(text: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in text {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return Err(format!("bad base64 byte {c:#x}")),
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            // Masked to 8 bits: the cast cannot truncate.
            #[allow(clippy::cast_possible_truncation)]
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

/// The tree digest of `lock/text-deps.lock` (see that file): SHA-256 of the sorted
/// `<sha256>  <path>` listing of every regular file under `root`.
///
/// # Errors
/// On I/O errors.
pub fn tree_digest(root: &Path) -> Result<String, String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "non-UTF-8 file name".to_owned())?;
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let ty = entry.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                walk(&entry.path(), &rel, out)?;
            } else if ty.is_file() {
                out.push((rel, entry.path()));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, "", &mut files)?;
    files.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut listing = String::new();
    for (rel, path) in &files {
        let _ = writeln!(listing, "{}  {rel}", sha256_file(path)?);
    }
    Ok(sha256_hex(listing.as_bytes()))
}

fn stamp_ok(dir: &Path, digest: &str) -> bool {
    std::fs::read_to_string(dir.join(".verified")).is_ok_and(|s| s.trim() == digest)
}

// ---------------------------------------------------------------------------------------------
// HarfBuzz and ICU.

/// The extracted HarfBuzz `src/` tree at Skia's pin (`<cache>/harfbuzz-<rev8>/`), downloading and
/// verifying it on first use.
///
/// # Errors
/// If the download or the digest check fails.
pub fn harfbuzz() -> Result<PathBuf, String> {
    let lock = Lock::read("text-deps.lock")?;
    let rev = lock.get("harfbuzz", "rev")?;
    let digest = lock.get("harfbuzz", "tree_sha256")?;
    let dir = cache_root().join(format!("harfbuzz-{}", &rev[..8]));
    if stamp_ok(&dir, digest) {
        return Ok(dir);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(cache_root()).map_err(|e| e.to_string())?;
    let staging = staging_dir(&dir)?;
    let result = (|| {
        let tgz = staging.join("download.tar.gz");
        curl(lock.get("harfbuzz", "url")?, &tgz)?;
        let tree = staging.join("src");
        std::fs::create_dir(&tree).map_err(|e| e.to_string())?;
        run(Command::new("tar")
            .arg("-xzf")
            .arg(&tgz)
            .arg("-C")
            .arg(&tree))?;
        std::fs::remove_file(&tgz).map_err(|e| e.to_string())?;
        let got = tree_digest(&tree)?;
        if got != digest {
            return Err(format!(
                "HarfBuzz tree digest is {got}, inventory/text-deps.lock says {digest}"
            ));
        }
        std::fs::write(staging.join(".verified"), digest).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    publish_dir(&staging, &dir)?;
    Ok(dir)
}

/// The HarfBuzz source directory (what HarfBuzz calls `src/`) inside [`harfbuzz`]'s result.
#[must_use]
pub fn harfbuzz_src(root: &Path) -> PathBuf {
    root.join("src")
}

/// Chromium ICU's `source/common` and `common/icudtl.dat`, extracted to
/// `<cache>/icu-<rev8>/{common,icudtl.dat}`.
#[derive(Clone, Debug)]
pub struct Icu {
    /// `source/common` (the `.cpp` files, to compile with `-DU_COMMON_IMPLEMENTATION`).
    pub common: PathBuf,
    /// `common/icudtl.dat`.
    pub data: PathBuf,
}

/// Fetches Chromium ICU at Skia's pin.
///
/// # Errors
/// If a download or a digest check fails.
pub fn icu() -> Result<Icu, String> {
    let lock = Lock::read("text-deps.lock")?;
    let rev = lock.get("icu", "rev")?;
    let tree_digest_lock = lock.get("icu", "tree_sha256")?;
    let data_digest = lock.get("icu", "data_sha256")?;
    let dir = cache_root().join(format!("icu-{}", &rev[..8]));
    let stamp = format!("{tree_digest_lock}+{data_digest}");
    let result = Icu {
        common: dir.join("common"),
        data: dir.join("icudtl.dat"),
    };
    if stamp_ok(&dir, &stamp) {
        return Ok(result);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(cache_root()).map_err(|e| e.to_string())?;
    let staging = staging_dir(&dir)?;
    let built = (|| {
        let tgz = staging.join("download.tar.gz");
        curl(lock.get("icu", "url")?, &tgz)?;
        let tree = staging.join("common");
        std::fs::create_dir(&tree).map_err(|e| e.to_string())?;
        run(Command::new("tar")
            .arg("-xzf")
            .arg(&tgz)
            .arg("-C")
            .arg(&tree))?;
        std::fs::remove_file(&tgz).map_err(|e| e.to_string())?;
        let got = tree_digest(&tree)?;
        if got != tree_digest_lock {
            return Err(format!(
                "ICU tree digest is {got}, inventory/text-deps.lock says {tree_digest_lock}"
            ));
        }
        let b64 = staging.join("icudtl.b64");
        curl(lock.get("icu", "data_url")?, &b64)?;
        let raw = base64_decode(&std::fs::read(&b64).map_err(|e| e.to_string())?)?;
        std::fs::remove_file(&b64).map_err(|e| e.to_string())?;
        let got = sha256_hex(&raw);
        if got != data_digest {
            return Err(format!(
                "icudtl.dat has SHA-256 {got}, inventory/text-deps.lock says {data_digest}"
            ));
        }
        std::fs::write(staging.join("icudtl.dat"), raw).map_err(|e| e.to_string())?;
        std::fs::write(staging.join(".verified"), &stamp).map_err(|e| e.to_string())
    })();
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    publish_dir(&staging, &dir)?;
    Ok(result)
}

// ---------------------------------------------------------------------------------------------
// The UAX conformance files.

/// The Unicode conformance files of `inventory/text-deps.lock` `[ucd]` (`GraphemeBreakTest.txt`,
/// `WordBreakTest.txt`, `SentenceBreakTest.txt`, `LineBreakTest.txt`, `BidiCharacterTest.txt`),
/// downloaded from unicode.org, verified and cached in `<cache>/ucd-<version>/`.
///
/// `$SKIA_RUST_UCD_DIR` names a directory that already holds them (verified all the same).
///
/// # Errors
/// If a download fails or a file does not match the lock.
pub fn ucd() -> Result<PathBuf, String> {
    let lock = Lock::read("text-deps.lock")?;
    let version = lock.get("ucd", "version")?.to_owned();
    let base = lock.get("ucd", "base_url")?.to_owned();
    let files: Vec<(String, String, String)> = lock
        .section("ucd")
        .into_iter()
        .filter(|(_, v)| v.contains('|'))
        .map(|(name, v)| {
            let (path, sha) = v.split_once('|').unwrap_or((&v, ""));
            (name, path.to_owned(), sha.to_owned())
        })
        .collect();
    let stamp = {
        let mut s = String::new();
        for (name, _, sha) in &files {
            let _ = writeln!(s, "{name} {sha}");
        }
        sha256_hex(s.as_bytes())
    };
    let dir = cache_root().join(format!("ucd-{version}"));
    if stamp_ok(&dir, &stamp) {
        return Ok(dir);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(cache_root()).map_err(|e| e.to_string())?;
    let staging = staging_dir(&dir)?;
    let local = std::env::var_os("SKIA_RUST_UCD_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let built = (|| {
        for (name, path, sha) in &files {
            let dest = staging.join(name);
            if let Some(local) = &local {
                std::fs::copy(local.join(name), &dest)
                    .map_err(|e| format!("{}: {e}", local.join(name).display()))?;
            } else {
                curl(&format!("{base}/{path}"), &dest)?;
            }
            let got = sha256_file(&dest)?;
            if &got != sha {
                return Err(format!(
                    "{name} has SHA-256 {got}, inventory/text-deps.lock says {sha}"
                ));
            }
        }
        std::fs::write(staging.join(".verified"), &stamp).map_err(|e| e.to_string())
    })();
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    publish_dir(&staging, &dir)?;
    Ok(dir)
}

// ---------------------------------------------------------------------------------------------
// The paragraph test fonts.

/// What `SKIA_RUST_PARAGRAPH_FONTS` selects (the counterpart of Skia's `--paragraph_fonts`,
/// `modules/skparagraph/tests/SkParagraphTest.cpp#L74-L76`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParagraphFonts {
    /// Unset or empty: `FLAGS_paragraph_fonts.size() == 0`. Tests that need the fonts return
    /// early (`SKIP_IF_FONTS_NOT_FOUND`).
    None,
    /// `release`: the fonts of `inventory/test-fonts.lock`, fetched and cached.
    Release,
    /// Any other value: a directory (absolute, or relative to Skia's `resources` directory).
    Dir(PathBuf),
}

impl ParagraphFonts {
    /// Reads `SKIA_RUST_PARAGRAPH_FONTS`.
    #[must_use]
    pub fn from_env() -> ParagraphFonts {
        match std::env::var("SKIA_RUST_PARAGRAPH_FONTS") {
            Ok(v) if v == "release" => ParagraphFonts::Release,
            Ok(v) if !v.is_empty() => ParagraphFonts::Dir(PathBuf::from(v)),
            _ => ParagraphFonts::None,
        }
    }
}

/// `FLAGS_run_paragraph_tests_needing_system_fonts` (default true; Skia's bots pass
/// `--norun_paragraph_tests_needing_system_fonts`): `SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS`
/// set to `0`, `false` or `no` turns it off.
#[must_use]
pub fn run_paragraph_tests_needing_system_fonts() -> bool {
    std::env::var("SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS")
        .ok()
        .is_none_or(|v| !matches!(v.as_str(), "0" | "false" | "no"))
}

/// The release asset's name and SHA-256, and the SHA-256 of every font, from
/// `inventory/test-fonts.lock`.
#[derive(Clone, Debug)]
pub struct TestFontsLock {
    /// The GitHub release tag.
    pub release: String,
    /// The tar archive's file name.
    pub asset: String,
    /// The archive's SHA-256.
    pub asset_sha256: String,
    /// Font file name -> SHA-256.
    pub fonts: BTreeMap<String, String>,
}

impl TestFontsLock {
    /// Reads `inventory/test-fonts.lock`.
    ///
    /// # Errors
    /// If it is missing or malformed.
    pub fn read() -> Result<TestFontsLock, String> {
        let lock = Lock::read("test-fonts.lock")?;
        let release = lock.get("", "release")?.to_owned();
        let assets = lock.section("assets");
        let (asset, asset_sha256) = assets
            .into_iter()
            .next()
            .ok_or("test-fonts.lock lists no asset")?;
        Ok(TestFontsLock {
            release,
            asset,
            asset_sha256,
            fonts: lock.section("fonts"),
        })
    }

    fn url(&self) -> String {
        format!(
            "{}/releases/download/{}/{}",
            env!("CARGO_PKG_REPOSITORY"),
            self.release,
            self.asset
        )
    }
}

/// The directory of the paragraph test fonts: the release asset of `inventory/test-fonts.lock`,
/// downloaded, verified and extracted into the cache on first use.
///
/// `$SKIA_RUST_TEST_FONTS_TAR` names a local copy of the archive to use instead of downloading
/// (it is verified against the lock all the same).
///
/// # Errors
/// If the archive cannot be fetched, or it or a font does not match the lock.
pub fn test_fonts() -> Result<PathBuf, String> {
    let lock = TestFontsLock::read()?;
    let dir = cache_root().join(&lock.release);
    let fonts = dir.join("fonts");
    if stamp_ok(&dir, &lock.asset_sha256) {
        return Ok(fonts);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(cache_root()).map_err(|e| e.to_string())?;
    let staging = staging_dir(&dir)?;
    let built = (|| {
        let tar = staging.join(&lock.asset);
        if let Some(local) = std::env::var_os("SKIA_RUST_TEST_FONTS_TAR").filter(|v| !v.is_empty())
        {
            std::fs::copy(&local, &tar)
                .map_err(|e| format!("{}: {e}", PathBuf::from(&local).display()))?;
        } else {
            download_release_asset(&lock, &tar)?;
        }
        let got = sha256_file(&tar)?;
        if got != lock.asset_sha256 {
            return Err(format!(
                "{} has SHA-256 {got}, inventory/test-fonts.lock says {}",
                lock.asset, lock.asset_sha256
            ));
        }
        let out = staging.join("fonts");
        std::fs::create_dir(&out).map_err(|e| e.to_string())?;
        run(Command::new("tar").arg("-xf").arg(&tar).arg("-C").arg(&out))?;
        std::fs::remove_file(&tar).map_err(|e| e.to_string())?;
        verify_fonts(&lock, &out)?;
        std::fs::write(staging.join(".verified"), &lock.asset_sha256).map_err(|e| e.to_string())
    })();
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    publish_dir(&staging, &dir)?;
    Ok(fonts)
}

fn download_release_asset(lock: &TestFontsLock, dest: &Path) -> Result<(), String> {
    let url = lock.url();
    let Err(curl_err) = curl(&url, dest) else {
        return Ok(());
    };
    let repo = env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/");
    let gh = run(Command::new("gh")
        .args([
            "release",
            "download",
            &lock.release,
            "-R",
            repo,
            "-p",
            &lock.asset,
            "-O",
        ])
        .arg(dest)
        .arg("--clobber"));
    gh.map_err(|gh_err| {
        format!(
            "downloading {url} failed (curl: {curl_err}; gh: {gh_err}); set \
             SKIA_RUST_TEST_FONTS_TAR to a local copy of the archive, or SKIA_RUST_PARAGRAPH_FONTS \
             to a directory of fonts"
        )
    })
}

fn verify_fonts(lock: &TestFontsLock, dir: &Path) -> Result<(), String> {
    for (name, sha) in &lock.fonts {
        let got = sha256_file(&dir.join(name))?;
        if &got != sha {
            return Err(format!(
                "font {name} has SHA-256 {got}, inventory/test-fonts.lock says {sha}"
            ));
        }
    }
    Ok(())
}

/// The font directory selected by `SKIA_RUST_PARAGRAPH_FONTS` (`GetResourcePath(FLAGS_paragraph_fonts[0])`
/// in `SkParagraphTest.cpp#L90-L95`): `None` when no fonts are selected.
///
/// A relative `Dir` value is resolved against `resources_dir` (Skia's `resources` directory) when
/// one is given.
///
/// # Errors
/// If `release` is selected and the fonts cannot be fetched.
pub fn paragraph_fonts_dir(resources_dir: Option<&Path>) -> Result<Option<PathBuf>, String> {
    match ParagraphFonts::from_env() {
        ParagraphFonts::None => Ok(None),
        ParagraphFonts::Release => test_fonts().map(Some),
        ParagraphFonts::Dir(dir) => {
            if dir.is_absolute() {
                Ok(Some(dir))
            } else {
                Ok(Some(resources_dir.map_or(dir.clone(), |r| r.join(&dir))))
            }
        }
    }
}

/// The font files of `dir` in the order `ResourceFontCollection` registers them: file names
/// sorted by `strcmp` (`SkParagraphTest.cpp#L96-L116`), subdirectories skipped
/// (`SkOSFile::Iter::next` without `getDir`).
///
/// # Errors
/// If the directory cannot be read.
pub fn font_files_sorted(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_file() {
            paths.push(entry.path());
        }
    }
    paths.sort_by(|a, b| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_locks_parse() {
        let fonts = TestFontsLock::read().unwrap();
        assert_eq!(fonts.release, "test-fonts-m156");
        assert_eq!(fonts.asset_sha256.len(), 64);
        assert!(fonts.fonts.contains_key("Roboto-Italic.ttf"));
        assert!(fonts.fonts.values().all(|s| s.len() == 64));
        let deps = Lock::read("text-deps.lock").unwrap();
        for (section, key) in [
            ("harfbuzz", "rev"),
            ("harfbuzz", "tree_sha256"),
            ("icu", "rev"),
            ("icu", "tree_sha256"),
            ("icu", "data_sha256"),
        ] {
            assert!(deps.get(section, key).unwrap().len() >= 40);
        }
    }

    #[test]
    fn base64_round_trips_known_vectors() {
        assert_eq!(base64_decode(b"").unwrap(), b"");
        assert_eq!(base64_decode(b"Zg==").unwrap(), b"f");
        assert_eq!(base64_decode(b"Zm8=\n").unwrap(), b"fo");
        assert_eq!(base64_decode(b"Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode(b"Zm9vYg==").unwrap(), b"foob");
        assert!(base64_decode(b"Zm9v!").is_err());
    }

    #[test]
    fn tree_digest_is_the_sha256sum_of_the_listing() {
        let dir =
            std::env::temp_dir().join(format!("skia-rust-tree-digest-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("b")).unwrap();
        std::fs::write(dir.join("a.txt"), b"a").unwrap();
        std::fs::write(dir.join("b").join("c.txt"), b"c").unwrap();
        let listing = format!(
            "{}  a.txt\n{}  b/c.txt\n",
            sha256_hex(b"a"),
            sha256_hex(b"c")
        );
        assert_eq!(tree_digest(&dir).unwrap(), sha256_hex(listing.as_bytes()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn paragraph_fonts_flag_semantics() {
        // Not set in the environment of the unit tests unless a developer exports it, in which
        // case the assertion below is skipped.
        if std::env::var_os("SKIA_RUST_PARAGRAPH_FONTS").is_none() {
            assert_eq!(ParagraphFonts::from_env(), ParagraphFonts::None);
            assert_eq!(paragraph_fonts_dir(None), Ok(None));
        }
    }
}
