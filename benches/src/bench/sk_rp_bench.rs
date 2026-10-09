// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/SkRPBench.cpp

//! Raster pipeline micro-benches (non-rendering): `SkRPDivBench` runs the integer and unsigned
//! division stages over 128 pixels, `SkRPGatherBench` gathers 128 pixels from a 128 x 128 image
//! in 8888, 565 or RG88.

use skia_rust_core::raster_pipeline::contexts::{GatherCtx, GatherPixels};
use skia_rust_core::raster_pipeline::{
    MemPtr, MemSlot, MemView, MemoryBindings, RasterPipeline, Stage,
};

use crate::def_bench;
use crate::prelude::*;

/// `SkRPDivBench::Type`.
// Port of: bench/SkRPBench.cpp#L27-L28 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DivType {
    Int,
    Uint,
}

/// `class SkRPDivBench`.
// Port of: bench/SkRPBench.cpp#L27-L77 (chrome/m156)
struct SkRpDivBench {
    ty: DivType,
    slots: u32,
    /// `fName`.
    name: String,
    /// `fData`: 1024 divisors `1..=1024`, as the native-order `int32_t`s the stages read.
    data: Vec<u8>,
}

impl SkRpDivBench {
    // Port of: bench/SkRPBench.cpp#L29-L37 (chrome/m156)
    fn new(ty: DivType, slots: u32) -> Self {
        // fName.printf("skrp_div_%s_%d", fType == Type::kInt ? "int" : "uint", fSlots);
        let name = format!(
            "skrp_div_{}_{slots}",
            if ty == DivType::Int { "int" } else { "uint" }
        );
        // Initialize the data we are dividing by so it's non-zero (avoiding any special casing).
        // for (int i = 0; i < 1024; i++) fData[i] = i + 1;
        let data = (0..1024_i32).flat_map(|i| (i + 1).to_ne_bytes()).collect();
        Self {
            ty,
            slots,
            name,
            data,
        }
    }
}

impl Benchmark for SkRpDivBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/SkRPBench.cpp#L39-L65 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // SkRasterPipelineOp op; (chosen by type and slot count)
        let data_ptr = MemPtr::new(MemSlot(0), 0);
        let op = match (self.ty, self.slots) {
            (DivType::Int, 1) => Stage::DivInt(data_ptr),
            (DivType::Int, 4) => Stage::Div4Ints(data_ptr),
            (DivType::Uint, 1) => Stage::DivUint(data_ptr),
            (DivType::Uint, 4) => Stage::Div4Uints(data_ptr),
            _ => unreachable!("SkUNREACHABLE"),
        };
        // SkRasterPipeline p(&alloc); p.append(op, fData);
        let mut p = RasterPipeline::new();
        p.append(op);
        // The divisors are bound to the pipeline's memory slot for each run.
        let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut self.data));
        for _ in 0..loops {
            // p.run(0, 0, 128, 1);
            p.run(0, 0, 128, 1, &mut mem);
        }
    }
}

/// `SkRPGatherBench::Format`.
// Port of: bench/SkRPBench.cpp#L79-L81 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GatherFormat {
    K8888,
    K565,
    KRG88,
}

/// `class SkRPGatherBench`.
// Port of: bench/SkRPBench.cpp#L79-L134 (chrome/m156)
struct SkRpGatherBench {
    format: GatherFormat,
    /// `fName`.
    name: String,
    /// `fPixels`: 128 x 128 pixels, all `0xFFFFFFFF`, as native-order bytes.
    pixels: Vec<u8>,
}

impl SkRpGatherBench {
    // Port of: bench/SkRPBench.cpp#L82-L97 (chrome/m156)
    fn new(format: GatherFormat) -> Self {
        // switch (fFormat) { ... fName.printf("skrp_gather_...") }
        let name = match format {
            GatherFormat::K8888 => "skrp_gather_8888",
            GatherFormat::K565 => "skrp_gather_565",
            GatherFormat::KRG88 => "skrp_gather_rg88",
        }
        .to_owned();
        // for (int i = 0; i < 128 * 128; i++) fPixels[i] = 0xFFFFFFFF;
        let pixels = vec![0xFF; 128 * 128 * 4];
        Self {
            format,
            name,
            pixels,
        }
    }
}

impl Benchmark for SkRpGatherBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/SkRPBench.cpp#L110-L131 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // The context borrows the pixels, so it is built here; `fCtx` in Skia is built once in
        // the constructor, but building the struct is O(1) and does not touch the pixels.
        let ctx = GatherCtx {
            pixels: GatherPixels::from(&self.pixels[..]),
            stride: 128,
            width: 128.0,
            height: 128.0,
            weights: [0.0; 16],
            round_down_at_integer: false,
        };
        // SkRasterPipeline p(&alloc);
        let mut p = RasterPipeline::new();
        // p.append(SkRasterPipelineOp::seed_shader);
        p.append(Stage::SeedShader);
        // p.append(op, fCtx);
        p.append(match self.format {
            GatherFormat::K8888 => Stage::Gather8888(&ctx),
            GatherFormat::K565 => Stage::Gather565(&ctx),
            GatherFormat::KRG88 => Stage::GatherRg88(&ctx),
        });
        let mut mem = MemoryBindings::new();
        for _ in 0..loops {
            // p.run(0, 0, 128, 1);
            p.run(0, 0, 128, 1, &mut mem);
        }
    }
}

// Port of: bench/SkRPBench.cpp#L136 (chrome/m156)
def_bench!(
    sk_rp_div_int_1 = "SkRPDivBench(SkRPDivBench::Type::kInt, 1)",
    SkRpDivBench::new(DivType::Int, 1)
);
// Port of: bench/SkRPBench.cpp#L137 (chrome/m156)
def_bench!(
    sk_rp_div_int_4 = "SkRPDivBench(SkRPDivBench::Type::kInt, 4)",
    SkRpDivBench::new(DivType::Int, 4)
);
// Port of: bench/SkRPBench.cpp#L138 (chrome/m156)
def_bench!(
    sk_rp_div_uint_1 = "SkRPDivBench(SkRPDivBench::Type::kUint, 1)",
    SkRpDivBench::new(DivType::Uint, 1)
);
// Port of: bench/SkRPBench.cpp#L139 (chrome/m156)
def_bench!(
    sk_rp_div_uint_4 = "SkRPDivBench(SkRPDivBench::Type::kUint, 4)",
    SkRpDivBench::new(DivType::Uint, 4)
);
// Port of: bench/SkRPBench.cpp#L141 (chrome/m156)
def_bench!(
    sk_rp_gather_8888 = "SkRPGatherBench(SkRPGatherBench::Format::k8888)",
    SkRpGatherBench::new(GatherFormat::K8888)
);
// Port of: bench/SkRPBench.cpp#L142 (chrome/m156)
def_bench!(
    sk_rp_gather_565 = "SkRPGatherBench(SkRPGatherBench::Format::k565)",
    SkRpGatherBench::new(GatherFormat::K565)
);
// Port of: bench/SkRPBench.cpp#L143 (chrome/m156)
def_bench!(
    sk_rp_gather_rg88 = "SkRPGatherBench(SkRPGatherBench::Format::kRG88)",
    SkRpGatherBench::new(GatherFormat::KRG88)
);
