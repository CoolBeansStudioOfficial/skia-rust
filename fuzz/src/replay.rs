// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The deterministic corpus replay behind every `def_fuzz!` test (docs/design/fuzz.md §1).
//!
//! A fuzzer passes when it runs every input of its replay set to completion: no panic (which is
//! what a crash, an `SkASSERT_RELEASE` or an arithmetic overflow in a debug build becomes) and no
//! input taking longer than [`TIMEOUT`]. The replay set is the empty input, the committed seeds
//! in `fuzz/corpus/<Name>/` and the crash reproducers in `fuzz/regressions/<Name>/`, in file-name
//! order. A fuzzer without seeds fails: an empty corpus checks nothing.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crate::Fuzz;

/// The longest one input may run: OSS-Fuzz's per-input timeout.
pub const TIMEOUT: Duration = Duration::from_secs(25);

/// The committed seed corpus of fuzzer `name`.
#[must_use]
pub fn corpus_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(name)
}

/// The committed crash reproducers of fuzzer `name` (docs/design/fuzz.md §5).
#[must_use]
pub fn regressions_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("regressions")
        .join(name)
}

/// The files in `dir` (none if it does not exist), sorted by name, as (label, bytes). Hidden
/// files (`.gitkeep`) are skipped.
///
/// # Panics
/// If `dir` exists but a file in it cannot be read.
#[must_use]
pub fn read_dir_sorted(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, Vec<u8>)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!name.starts_with('.')).then_some((name, e.path()))
        })
        .map(|(name, path)| {
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
            (name, bytes)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// The replay set of fuzzer `name`: the empty input, then its seeds, then its reproducers.
#[must_use]
pub fn inputs(name: &str) -> Vec<(String, Vec<u8>)> {
    let mut inputs = vec![("<empty>".to_owned(), Vec::new())];
    inputs.extend(
        read_dir_sorted(&corpus_dir(name))
            .into_iter()
            .map(|(file, bytes)| (format!("corpus/{name}/{file}"), bytes)),
    );
    inputs.extend(
        read_dir_sorted(&regressions_dir(name))
            .into_iter()
            .map(|(file, bytes)| (format!("regressions/{name}/{file}"), bytes)),
    );
    inputs
}

/// Runs `f` on `data` on its own thread. `Err` describes a panic or an input that ran longer
/// than `timeout` (the thread is then left running; the test process ends it).
///
/// # Errors
/// A message naming `label` and the failure.
pub fn run_one(
    label: &str,
    data: Vec<u8>,
    f: fn(&mut Fuzz<'_>),
    timeout: Duration,
) -> Result<(), String> {
    let (done, finished) = mpsc::channel();
    let handle = std::thread::Builder::new()
        .name(label.to_owned())
        .spawn(move || {
            f(&mut Fuzz::new(&data));
            // The receiver is gone only after a timeout was already reported.
            let _ = done.send(());
        })
        .map_err(|err| format!("{label}: cannot spawn the replay thread: {err}"))?;
    match finished.recv_timeout(timeout) {
        Ok(()) => handle
            .join()
            .map_err(|_| format!("{label}: panicked after finishing")),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("{label}: hang (still running after {timeout:?})"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // The thread panicked; the panic hook has printed its message.
            let _ = handle.join();
            Err(format!("{label}: panicked"))
        }
    }
}

/// Replays the corpus of fuzzer `name` through `f` (the body of every `def_fuzz!` test).
///
/// # Panics
/// If `name` has no seeds, or any input panics or hangs; the message lists every failing input.
pub fn replay(name: &str, f: fn(&mut Fuzz<'_>)) {
    let inputs = inputs(name);
    let corpus_prefix = format!("corpus/{name}/");
    assert!(
        inputs
            .iter()
            .any(|(label, _)| label.starts_with(&corpus_prefix)),
        "fuzzer {name} has no seeds in {}; run `cargo run -p skia-rust-fuzz --bin fuzz-seeds`",
        corpus_dir(name).display()
    );
    let failures: Vec<String> = inputs
        .into_iter()
        .filter_map(|(label, data)| run_one(&label, data, f, TIMEOUT).err())
        .collect();
    assert!(
        failures.is_empty(),
        "fuzzer {name} failed on {} input(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fine(fuzz: &mut Fuzz<'_>) {
        let _: u32 = fuzz.next();
    }

    fn crashes(fuzz: &mut Fuzz<'_>) {
        if fuzz.next::<u8>() == 7 {
            fuzz.signal_bug();
        }
    }

    fn spins(fuzz: &mut Fuzz<'_>) {
        if fuzz.next::<u8>() == 7 {
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    #[test]
    fn run_one_reports_panics_and_hangs() {
        let short = Duration::from_millis(200);
        assert_eq!(run_one("a", vec![7], fine, short), Ok(()));
        assert_eq!(run_one("b", vec![1], crashes, short), Ok(()));
        assert_eq!(
            run_one("c", vec![7], crashes, short),
            Err("c: panicked".to_owned())
        );
        assert_eq!(
            run_one("d", vec![7], spins, short),
            Err(format!("d: hang (still running after {short:?})"))
        );
    }

    #[test]
    fn inputs_start_with_the_empty_input() {
        let inputs = inputs("NoSuchFuzzer");
        assert_eq!(inputs, vec![("<empty>".to_owned(), Vec::new())]);
    }
}
