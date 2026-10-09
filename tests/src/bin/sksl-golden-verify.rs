// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `sksl-golden-verify`: runs every in-scope `tests/sksl` golden through the `skslc` port and
//! prints one libtest-style line per golden, `test <manifest id> ... ok|FAILED|ignored`, which
//! `cargo xtask inventory verify` reads (`docs/design/sksl.md` §9).
//!
//! It needs the Skia checkout (`third_party/skia`). Without it, it prints a skip note and exits 0.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use skia_rust_tests::tools::sksl_goldens::{Verdict, parse_gni, plan, run_job};

fn skia_root() -> PathBuf {
    // `tests/` sits one level below the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("third_party")
        .join("skia")
}

fn main() -> ExitCode {
    let skia = skia_root();
    let Ok(gni) = std::fs::read_to_string(skia.join("gn/sksl_tests.gni")) else {
        println!(
            "todo: skipping, missing Skia checkout at {}",
            skia.display()
        );
        return ExitCode::SUCCESS;
    };
    let jobs = match parse_gni(&gni).and_then(|lists| plan(&lists)) {
        Ok(jobs) => jobs,
        Err(e) => {
            eprintln!("sksl-golden-verify: {e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut ok, mut failed, mut ignored) = (0_usize, 0_usize, 0_usize);
    for job in &jobs {
        match run_job(job, &skia) {
            Verdict::Ok => {
                ok += 1;
                println!("test {} ... ok", job.id);
            }
            Verdict::Failed(why) => {
                failed += 1;
                println!("test {} ... FAILED", job.id);
                eprintln!("{}: {why}", job.id);
            }
            Verdict::Ignored(why) => {
                ignored += 1;
                println!("test {} ... ignored, {why}", job.id);
            }
        }
    }
    println!("sksl goldens: {ok} ok, {failed} failed, {ignored} ignored");
    ExitCode::SUCCESS
}
