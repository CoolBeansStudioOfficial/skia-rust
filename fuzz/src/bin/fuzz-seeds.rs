// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Rewrites the committed seed corpora `fuzz/corpus/<Name>/` from `skia_rust_fuzz::seeds`
//! (docs/design/fuzz.md §2). Needs Skia's resources (`third_party/skia/resources` or
//! `$SKIA_RESOURCES`) for the encoder seeds.

use std::process::ExitCode;

use skia_rust_fuzz::fuzz::FUZZERS;
use skia_rust_fuzz::replay::corpus_dir;
use skia_rust_fuzz::seeds::generate;

fn main() -> ExitCode {
    if skia_rust_tools::resources::resource_dir().is_none() {
        eprintln!("Skia's resources are missing (run `cargo xtask skia fetch`); not writing seeds");
        return ExitCode::FAILURE;
    }
    for fuzzable in FUZZERS {
        let dir = corpus_dir(fuzzable.name);
        let seeds = generate(fuzzable.name).expect("every registered fuzzer has seeds");
        if dir.exists()
            && let Err(err) = std::fs::remove_dir_all(&dir)
        {
            eprintln!("removing {}: {err}", dir.display());
            return ExitCode::FAILURE;
        }
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("creating {}: {err}", dir.display());
            return ExitCode::FAILURE;
        }
        let bytes: usize = seeds.iter().map(|(_, b)| b.len()).sum();
        for (file, data) in &seeds {
            let path = dir.join(file);
            if let Err(err) = std::fs::write(&path, data) {
                eprintln!("writing {}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
        println!(
            "{:<22} {:>3} seeds, {bytes:>8} bytes",
            fuzzable.name,
            seeds.len()
        );
    }
    ExitCode::SUCCESS
}
