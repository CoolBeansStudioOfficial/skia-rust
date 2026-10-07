// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` arithmetic: n-way and immediate add/sub/mul/div/min/max/mod/mix/compare, casts, bitwise
//! ops, `abs`/`floor`/`ceil`, `dot`, `matrix_multiply_*`, `smoothstep`, `refract`.
//!
//! Owner: task B6b (`docs/design/raster-pipeline.md` §5). Stages not ported yet are stubs
//! that panic naming the task; replace a stub's body with the port (keeping the signature,
//! which the op table fixes) and add a `// Port of:` line.

#[allow(clippy::wildcard_imports)]
use super::*;

si! {
    pub(super) fn bitwise_and_imm_4_ints(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_imm_4_ints", "B6b")
    }

    pub(super) fn bitwise_and_imm_3_ints(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_imm_3_ints", "B6b")
    }

    pub(super) fn bitwise_and_imm_2_ints(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_imm_2_ints", "B6b")
    }

    pub(super) fn bitwise_and_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_imm_int", "B6b")
    }

    pub(super) fn bitwise_and_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_n_ints", "B6b")
    }

    pub(super) fn bitwise_and_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_int", "B6b")
    }

    pub(super) fn bitwise_and_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_2_ints", "B6b")
    }

    pub(super) fn bitwise_and_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_3_ints", "B6b")
    }

    pub(super) fn bitwise_and_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_and_4_ints", "B6b")
    }

    pub(super) fn bitwise_or_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_or_n_ints", "B6b")
    }

    pub(super) fn bitwise_or_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_or_int", "B6b")
    }

    pub(super) fn bitwise_or_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_or_2_ints", "B6b")
    }

    pub(super) fn bitwise_or_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_or_3_ints", "B6b")
    }

    pub(super) fn bitwise_or_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_or_4_ints", "B6b")
    }

    pub(super) fn bitwise_xor_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_imm_int", "B6b")
    }

    pub(super) fn bitwise_xor_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_n_ints", "B6b")
    }

    pub(super) fn bitwise_xor_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_int", "B6b")
    }

    pub(super) fn bitwise_xor_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_2_ints", "B6b")
    }

    pub(super) fn bitwise_xor_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_3_ints", "B6b")
    }

    pub(super) fn bitwise_xor_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("bitwise_xor_4_ints", "B6b")
    }

    pub(super) fn cast_to_float_from_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_int", "B6b")
    }

    pub(super) fn cast_to_float_from_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_2_ints", "B6b")
    }

    pub(super) fn cast_to_float_from_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_3_ints", "B6b")
    }

    pub(super) fn cast_to_float_from_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_4_ints", "B6b")
    }

    pub(super) fn cast_to_float_from_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_uint", "B6b")
    }

    pub(super) fn cast_to_float_from_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_2_uints", "B6b")
    }

    pub(super) fn cast_to_float_from_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_3_uints", "B6b")
    }

    pub(super) fn cast_to_float_from_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_float_from_4_uints", "B6b")
    }

    pub(super) fn cast_to_int_from_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_int_from_float", "B6b")
    }

    pub(super) fn cast_to_int_from_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_int_from_2_floats", "B6b")
    }

    pub(super) fn cast_to_int_from_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_int_from_3_floats", "B6b")
    }

    pub(super) fn cast_to_int_from_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_int_from_4_floats", "B6b")
    }

    pub(super) fn cast_to_uint_from_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_uint_from_float", "B6b")
    }

    pub(super) fn cast_to_uint_from_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_uint_from_2_floats", "B6b")
    }

    pub(super) fn cast_to_uint_from_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_uint_from_3_floats", "B6b")
    }

    pub(super) fn cast_to_uint_from_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cast_to_uint_from_4_floats", "B6b")
    }

    pub(super) fn abs_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("abs_int", "B6b")
    }

    pub(super) fn abs_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("abs_2_ints", "B6b")
    }

    pub(super) fn abs_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("abs_3_ints", "B6b")
    }

    pub(super) fn abs_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("abs_4_ints", "B6b")
    }

    pub(super) fn floor_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("floor_float", "B6b")
    }

    pub(super) fn floor_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("floor_2_floats", "B6b")
    }

    pub(super) fn floor_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("floor_3_floats", "B6b")
    }

    pub(super) fn floor_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("floor_4_floats", "B6b")
    }

    pub(super) fn ceil_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("ceil_float", "B6b")
    }

    pub(super) fn ceil_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("ceil_2_floats", "B6b")
    }

    pub(super) fn ceil_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("ceil_3_floats", "B6b")
    }

    pub(super) fn ceil_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("ceil_4_floats", "B6b")
    }

    pub(super) fn refract_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("refract_4_floats", "B6b")
    }

    pub(super) fn matrix_multiply_2(_ctx: MatrixMultiplyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_multiply_2", "B6b")
    }

    pub(super) fn matrix_multiply_3(_ctx: MatrixMultiplyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_multiply_3", "B6b")
    }

    pub(super) fn matrix_multiply_4(_ctx: MatrixMultiplyCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("matrix_multiply_4", "B6b")
    }

    pub(super) fn smoothstep_n_floats(_ctx: TernaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("smoothstep_n_floats", "B6b")
    }

    pub(super) fn dot_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dot_2_floats", "B6b")
    }

    pub(super) fn dot_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dot_3_floats", "B6b")
    }

    pub(super) fn dot_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("dot_4_floats", "B6b")
    }

    pub(super) fn add_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_imm_float", "B6b")
    }

    pub(super) fn add_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_n_floats", "B6b")
    }

    pub(super) fn add_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_float", "B6b")
    }

    pub(super) fn add_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_2_floats", "B6b")
    }

    pub(super) fn add_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_3_floats", "B6b")
    }

    pub(super) fn add_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_4_floats", "B6b")
    }

    pub(super) fn add_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_imm_int", "B6b")
    }

    pub(super) fn add_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_n_ints", "B6b")
    }

    pub(super) fn add_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_int", "B6b")
    }

    pub(super) fn add_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_2_ints", "B6b")
    }

    pub(super) fn add_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_3_ints", "B6b")
    }

    pub(super) fn add_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("add_4_ints", "B6b")
    }

    pub(super) fn sub_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_n_floats", "B6b")
    }

    pub(super) fn sub_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_float", "B6b")
    }

    pub(super) fn sub_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_2_floats", "B6b")
    }

    pub(super) fn sub_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_3_floats", "B6b")
    }

    pub(super) fn sub_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_4_floats", "B6b")
    }

    pub(super) fn sub_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_n_ints", "B6b")
    }

    pub(super) fn sub_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_int", "B6b")
    }

    pub(super) fn sub_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_2_ints", "B6b")
    }

    pub(super) fn sub_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_3_ints", "B6b")
    }

    pub(super) fn sub_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("sub_4_ints", "B6b")
    }

    pub(super) fn mul_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_imm_float", "B6b")
    }

    pub(super) fn mul_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_n_floats", "B6b")
    }

    pub(super) fn mul_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_float", "B6b")
    }

    pub(super) fn mul_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_2_floats", "B6b")
    }

    pub(super) fn mul_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_3_floats", "B6b")
    }

    pub(super) fn mul_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_4_floats", "B6b")
    }

    pub(super) fn mul_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_imm_int", "B6b")
    }

    pub(super) fn mul_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_n_ints", "B6b")
    }

    pub(super) fn mul_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_int", "B6b")
    }

    pub(super) fn mul_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_2_ints", "B6b")
    }

    pub(super) fn mul_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_3_ints", "B6b")
    }

    pub(super) fn mul_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mul_4_ints", "B6b")
    }

    pub(super) fn div_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_n_floats", "B6b")
    }

    pub(super) fn div_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_float", "B6b")
    }

    pub(super) fn div_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_2_floats", "B6b")
    }

    pub(super) fn div_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_3_floats", "B6b")
    }

    pub(super) fn div_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_4_floats", "B6b")
    }

    pub(super) fn div_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_n_ints", "B6b")
    }

    pub(super) fn div_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_int", "B6b")
    }

    pub(super) fn div_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_2_ints", "B6b")
    }

    pub(super) fn div_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_3_ints", "B6b")
    }

    pub(super) fn div_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_4_ints", "B6b")
    }

    pub(super) fn div_n_uints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_n_uints", "B6b")
    }

    pub(super) fn div_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_uint", "B6b")
    }

    pub(super) fn div_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_2_uints", "B6b")
    }

    pub(super) fn div_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_3_uints", "B6b")
    }

    pub(super) fn div_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("div_4_uints", "B6b")
    }

    pub(super) fn max_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_imm_float", "B6b")
    }

    pub(super) fn max_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_n_floats", "B6b")
    }

    pub(super) fn max_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_float", "B6b")
    }

    pub(super) fn max_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_2_floats", "B6b")
    }

    pub(super) fn max_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_3_floats", "B6b")
    }

    pub(super) fn max_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_4_floats", "B6b")
    }

    pub(super) fn max_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_n_ints", "B6b")
    }

    pub(super) fn max_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_int", "B6b")
    }

    pub(super) fn max_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_2_ints", "B6b")
    }

    pub(super) fn max_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_3_ints", "B6b")
    }

    pub(super) fn max_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_4_ints", "B6b")
    }

    pub(super) fn max_n_uints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_n_uints", "B6b")
    }

    pub(super) fn max_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_uint", "B6b")
    }

    pub(super) fn max_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_2_uints", "B6b")
    }

    pub(super) fn max_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_3_uints", "B6b")
    }

    pub(super) fn max_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("max_4_uints", "B6b")
    }

    pub(super) fn min_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_imm_float", "B6b")
    }

    pub(super) fn min_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_n_floats", "B6b")
    }

    pub(super) fn min_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_float", "B6b")
    }

    pub(super) fn min_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_2_floats", "B6b")
    }

    pub(super) fn min_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_3_floats", "B6b")
    }

    pub(super) fn min_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_4_floats", "B6b")
    }

    pub(super) fn min_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_n_ints", "B6b")
    }

    pub(super) fn min_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_int", "B6b")
    }

    pub(super) fn min_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_2_ints", "B6b")
    }

    pub(super) fn min_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_3_ints", "B6b")
    }

    pub(super) fn min_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_4_ints", "B6b")
    }

    pub(super) fn min_n_uints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_n_uints", "B6b")
    }

    pub(super) fn min_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_uint", "B6b")
    }

    pub(super) fn min_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_2_uints", "B6b")
    }

    pub(super) fn min_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_3_uints", "B6b")
    }

    pub(super) fn min_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("min_4_uints", "B6b")
    }

    pub(super) fn mod_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mod_n_floats", "B6b")
    }

    pub(super) fn mod_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mod_float", "B6b")
    }

    pub(super) fn mod_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mod_2_floats", "B6b")
    }

    pub(super) fn mod_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mod_3_floats", "B6b")
    }

    pub(super) fn mod_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mod_4_floats", "B6b")
    }

    pub(super) fn mix_n_floats(_ctx: TernaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_n_floats", "B6b")
    }

    pub(super) fn mix_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_float", "B6b")
    }

    pub(super) fn mix_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_2_floats", "B6b")
    }

    pub(super) fn mix_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_3_floats", "B6b")
    }

    pub(super) fn mix_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_4_floats", "B6b")
    }

    pub(super) fn mix_n_ints(_ctx: TernaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_n_ints", "B6b")
    }

    pub(super) fn mix_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_int", "B6b")
    }

    pub(super) fn mix_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_2_ints", "B6b")
    }

    pub(super) fn mix_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_3_ints", "B6b")
    }

    pub(super) fn mix_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("mix_4_ints", "B6b")
    }

    pub(super) fn cmplt_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_imm_float", "B6b")
    }

    pub(super) fn cmplt_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_n_floats", "B6b")
    }

    pub(super) fn cmplt_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_float", "B6b")
    }

    pub(super) fn cmplt_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_2_floats", "B6b")
    }

    pub(super) fn cmplt_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_3_floats", "B6b")
    }

    pub(super) fn cmplt_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_4_floats", "B6b")
    }

    pub(super) fn cmplt_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_imm_int", "B6b")
    }

    pub(super) fn cmplt_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_n_ints", "B6b")
    }

    pub(super) fn cmplt_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_int", "B6b")
    }

    pub(super) fn cmplt_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_2_ints", "B6b")
    }

    pub(super) fn cmplt_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_3_ints", "B6b")
    }

    pub(super) fn cmplt_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_4_ints", "B6b")
    }

    pub(super) fn cmplt_imm_uint(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_imm_uint", "B6b")
    }

    pub(super) fn cmplt_n_uints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_n_uints", "B6b")
    }

    pub(super) fn cmplt_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_uint", "B6b")
    }

    pub(super) fn cmplt_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_2_uints", "B6b")
    }

    pub(super) fn cmplt_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_3_uints", "B6b")
    }

    pub(super) fn cmplt_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmplt_4_uints", "B6b")
    }

    pub(super) fn cmple_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_imm_float", "B6b")
    }

    pub(super) fn cmple_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_n_floats", "B6b")
    }

    pub(super) fn cmple_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_float", "B6b")
    }

    pub(super) fn cmple_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_2_floats", "B6b")
    }

    pub(super) fn cmple_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_3_floats", "B6b")
    }

    pub(super) fn cmple_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_4_floats", "B6b")
    }

    pub(super) fn cmple_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_imm_int", "B6b")
    }

    pub(super) fn cmple_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_n_ints", "B6b")
    }

    pub(super) fn cmple_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_int", "B6b")
    }

    pub(super) fn cmple_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_2_ints", "B6b")
    }

    pub(super) fn cmple_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_3_ints", "B6b")
    }

    pub(super) fn cmple_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_4_ints", "B6b")
    }

    pub(super) fn cmple_imm_uint(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_imm_uint", "B6b")
    }

    pub(super) fn cmple_n_uints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_n_uints", "B6b")
    }

    pub(super) fn cmple_uint(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_uint", "B6b")
    }

    pub(super) fn cmple_2_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_2_uints", "B6b")
    }

    pub(super) fn cmple_3_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_3_uints", "B6b")
    }

    pub(super) fn cmple_4_uints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmple_4_uints", "B6b")
    }

    pub(super) fn cmpeq_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_imm_float", "B6b")
    }

    pub(super) fn cmpeq_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_n_floats", "B6b")
    }

    pub(super) fn cmpeq_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_float", "B6b")
    }

    pub(super) fn cmpeq_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_2_floats", "B6b")
    }

    pub(super) fn cmpeq_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_3_floats", "B6b")
    }

    pub(super) fn cmpeq_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_4_floats", "B6b")
    }

    pub(super) fn cmpeq_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_imm_int", "B6b")
    }

    pub(super) fn cmpeq_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_n_ints", "B6b")
    }

    pub(super) fn cmpeq_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_int", "B6b")
    }

    pub(super) fn cmpeq_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_2_ints", "B6b")
    }

    pub(super) fn cmpeq_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_3_ints", "B6b")
    }

    pub(super) fn cmpeq_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpeq_4_ints", "B6b")
    }

    pub(super) fn cmpne_imm_float(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_imm_float", "B6b")
    }

    pub(super) fn cmpne_n_floats(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_n_floats", "B6b")
    }

    pub(super) fn cmpne_float(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_float", "B6b")
    }

    pub(super) fn cmpne_2_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_2_floats", "B6b")
    }

    pub(super) fn cmpne_3_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_3_floats", "B6b")
    }

    pub(super) fn cmpne_4_floats(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_4_floats", "B6b")
    }

    pub(super) fn cmpne_imm_int(_ctx: ConstantCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_imm_int", "B6b")
    }

    pub(super) fn cmpne_n_ints(_ctx: BinaryOpCtx, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_n_ints", "B6b")
    }

    pub(super) fn cmpne_int(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_int", "B6b")
    }

    pub(super) fn cmpne_2_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_2_ints", "B6b")
    }

    pub(super) fn cmpne_3_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_3_ints", "B6b")
    }

    pub(super) fn cmpne_4_ints(_ctx: MemPtr, _p: &mut Regs, _e: &mut Params<'_, '_>) {
        not_ported!("cmpne_4_ints", "B6b")
    }
}
