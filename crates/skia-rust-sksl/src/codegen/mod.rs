// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen (the Raster Pipeline back end so far).

//! Code generators: the Raster Pipeline back end (`src/sksl/codegen/SkSLRasterPipeline*`) and the
//! pipeline-stage generator (`src/sksl/codegen/SkSLPipelineStageCodeGenerator.cpp`).

pub mod pipeline_stage;
pub mod rp;
