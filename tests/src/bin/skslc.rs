// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/skslc/Main.cpp (the command line of the part `tools/skslc.rs` ports).

//! `skslc <input> <output> [--settings|--nosettings]`: the command line of the `skslc` port, to
//! regenerate one golden and diff it by hand.

use std::process::ExitCode;

use skia_rust_tests::tools::skslc::skslc;

fn main() -> ExitCode {
    let mut paths = Vec::new();
    let mut honor_settings = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--settings" => honor_settings = true,
            "--nosettings" => honor_settings = false,
            _ => paths.push(arg),
        }
    }
    let [input, output] = paths.as_slice() else {
        eprintln!("usage: skslc <input> <output> [--settings|--nosettings]");
        return ExitCode::FAILURE;
    };
    let text = match std::fs::read(input) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("error reading '{input}': {e}");
            return ExitCode::FAILURE;
        }
    };
    match skslc(input, &text, output, honor_settings) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(output, bytes) {
                eprintln!("error writing '{output}': {e}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
