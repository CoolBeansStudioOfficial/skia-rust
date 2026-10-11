// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The ported fuzzers, one module per `fuzz/<File>.cpp`.

pub mod fuzz_cubic_roots;
pub mod fuzz_draw_functions;
pub mod fuzz_encoders;
pub mod fuzz_gradients;
pub mod fuzz_parse_path;
pub mod fuzz_path_measure;
pub mod fuzz_pathop;
pub mod fuzz_poly_utils;
pub mod fuzz_quad_roots;
pub mod fuzz_region_op;

use crate::Fuzzable;

/// Every ported fuzzer (the `sk_tools::Registry<Fuzzable>` that `DEF_FUZZ` fills), sorted by
/// name. The cargo-fuzz targets in `fuzz/libfuzzer` and the seed generator look fuzzers up here.
pub const FUZZERS: &[Fuzzable] = &[
    Fuzzable {
        name: "CubicRoots",
        f: fuzz_cubic_roots::fuzz_CubicRoots,
    },
    Fuzzable {
        name: "DrawFunctions",
        f: fuzz_draw_functions::fuzz_DrawFunctions,
    },
    Fuzzable {
        name: "Gradients",
        f: fuzz_gradients::fuzz_Gradients,
    },
    Fuzzable {
        name: "JPEGEncoder",
        f: fuzz_encoders::fuzz_JPEGEncoder,
    },
    Fuzzable {
        name: "LegacyChromiumPathop",
        f: fuzz_pathop::fuzz_LegacyChromiumPathop,
    },
    Fuzzable {
        name: "PNGEncoder",
        f: fuzz_encoders::fuzz_PNGEncoder,
    },
    Fuzzable {
        name: "ParsePath",
        f: fuzz_parse_path::fuzz_ParsePath,
    },
    Fuzzable {
        name: "PathMeasure",
        f: fuzz_path_measure::fuzz_PathMeasure,
    },
    Fuzzable {
        name: "Pathop",
        f: fuzz_pathop::fuzz_Pathop,
    },
    Fuzzable {
        name: "PolyUtils",
        f: fuzz_poly_utils::fuzz_PolyUtils,
    },
    Fuzzable {
        name: "QuadRoots",
        f: fuzz_quad_roots::fuzz_QuadRoots,
    },
    Fuzzable {
        name: "RegionOp",
        f: fuzz_region_op::fuzz_RegionOp,
    },
    Fuzzable {
        name: "WEBPEncoder",
        f: fuzz_encoders::fuzz_WEBPEncoder,
    },
];
