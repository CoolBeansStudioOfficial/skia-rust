// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! `SkSL` arithmetic: n-way and immediate add/sub/mul/div/min/max/mod/mix/compare, casts, bitwise
//! ops, `abs`/`floor`/`ceil`, `dot`, `matrix_multiply_*`, `smoothstep`, `refract`.
//!
//! Owner: task B6b (`docs/design/raster-pipeline.md` §5).
//!
//! `SkSL` slots are `N` lanes of 32 bits (`SLOT` bytes), addressed from the base pointer. Skia's
//! templates (`apply_adjacent_unary`/`_binary`/`_ternary`, `apply_binary_immediate`) become the
//! `unary!`/`binary!`/`ternary!`/`binary_imm!` macros below, which operate on the slots' bytes
//! (`load_bytes`/`store_bytes` are Skia's `sk_unaligned_load`/`store`); the per-family stage
//! functions are stamped by `*_family!` macros because Rust cannot paste identifiers.
//! Comparisons write their all-ones/all-zeros masks into the slots as `I32`s (Skia's `memcpy`).

#[allow(clippy::wildcard_imports)]
use super::*;

/// Bytes of one slot (`N` 32-bit lanes).
const SLOT: usize = 4 * N;

// Port of: src/opts/SkRasterPipeline_opts.h#L4726-L4732 (chrome/m156) (`apply_adjacent_unary`)
/// Applies `$f` to the first `$n` adjacent slots at `$ctx`, in place.
macro_rules! unary {
    ($e:ident, $ctx:ident, $n:literal, $T:ty, $f:expr) => {{
        let bytes = &mut $e.ptr_mut($ctx)[..$n * SLOT];
        for slot in bytes.chunks_exact_mut(SLOT) {
            let v = <$T>::load_bytes(slot);
            ($f)(v).store_bytes(slot);
        }
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4897-L4906 (chrome/m156) (`apply_adjacent_binary`)
/// `$f(dst, src)` over `$n` slots at `$ctx` (`dst`) and the `$n` slots after them (`src`),
/// writing `dst`.
macro_rules! binary {
    ($e:ident, $ctx:ident, $n:literal, $T:ty, $f:expr) => {{
        let bytes = &mut $e.ptr_mut($ctx)[..2 * $n * SLOT];
        let (d, s) = bytes.split_at_mut($n * SLOT);
        for (d, s) in d.chunks_exact_mut(SLOT).zip(s.chunks_exact(SLOT)) {
            ($f)(<$T>::load_bytes(d), <$T>::load_bytes(s)).store_bytes(d);
        }
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4908-L4915 (chrome/m156)
// (`apply_adjacent_binary_packed`)
/// The n-way binary form: `dst` and `src` are `base`-relative byte offsets, and the slot count is
/// their distance.
macro_rules! binary_packed {
    ($e:ident, $ctx:ident, $T:ty, $f:expr) => {{
        let dst = $e.base_ptr($ctx.dst);
        let delta = ($ctx.src - $ctx.dst) as usize;
        let bytes = &mut $e.ptr_mut(dst)[..2 * delta];
        let (d, s) = bytes.split_at_mut(delta);
        for (d, s) in d.chunks_exact_mut(SLOT).zip(s.chunks_exact(SLOT)) {
            ($f)(<$T>::load_bytes(d), <$T>::load_bytes(s)).store_bytes(d);
        }
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4917-L4927 (chrome/m156) (`apply_binary_immediate`)
/// `$f(dst, src)` over `$n` slots at `base + ctx.dst`, with `$src` the broadcast constant.
macro_rules! binary_imm {
    ($e:ident, $ctx:ident, $n:literal, $T:ty, $src:expr, $f:expr) => {{
        let src: $T = $src;
        let dst = $e.base_ptr($ctx.dst);
        let bytes = &mut $e.ptr_mut(dst)[..$n * SLOT];
        for d in bytes.chunks_exact_mut(SLOT) {
            ($f)(<$T>::load_bytes(d), src).store_bytes(d);
        }
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L5249-L5262 (chrome/m156) (`apply_adjacent_ternary`)
/// `$f(a, x, y)` over `$n` slots each at `$ctx` (`a`), then `x`, then `y`, writing `a`.
macro_rules! ternary {
    ($e:ident, $ctx:ident, $n:literal, $T:ty, $f:expr) => {{
        let bytes = &mut $e.ptr_mut($ctx)[..3 * $n * SLOT];
        let (a, rest) = bytes.split_at_mut($n * SLOT);
        let (x, y) = rest.split_at($n * SLOT);
        for ((a, x), y) in a
            .chunks_exact_mut(SLOT)
            .zip(x.chunks_exact(SLOT))
            .zip(y.chunks_exact(SLOT))
        {
            ($f)(
                <$T>::load_bytes(a),
                <$T>::load_bytes(x),
                <$T>::load_bytes(y),
            )
            .store_bytes(a);
        }
    }};
}

// Port of: src/opts/SkRasterPipeline_opts.h#L5264-L5272 (chrome/m156)
// (`apply_adjacent_ternary_packed`)
/// The n-way ternary form: `x` and `y` follow `a` at `delta` byte intervals.
macro_rules! ternary_packed {
    ($e:ident, $ctx:ident, $T:ty, $f:expr) => {{
        let dst = $e.base_ptr($ctx.dst);
        let delta = $ctx.delta as usize;
        let bytes = &mut $e.ptr_mut(dst)[..3 * delta];
        let (a, rest) = bytes.split_at_mut(delta);
        let (x, y) = rest.split_at(delta);
        for ((a, x), y) in a
            .chunks_exact_mut(SLOT)
            .zip(x.chunks_exact(SLOT))
            .zip(y.chunks_exact(SLOT))
        {
            ($f)(<$T>::load_bytes(a), <$T>::load_bytes(x), <$T>::load_bytes(y)).store_bytes(a);
        }
    }};
}

/// The 1-, 2-, 3- and 4-slot unary stages (`DECLARE_UNARY_*`).
macro_rules! unary_family {
    ($T:ty, $f:expr, $n1:ident, $n2:ident, $n3:ident, $n4:ident) => {
        si! {
            pub(super) fn $n1(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                unary!(e, ctx, 1, $T, $f);
            }
        }
        si! {
            pub(super) fn $n2(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                unary!(e, ctx, 2, $T, $f);
            }
        }
        si! {
            pub(super) fn $n3(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                unary!(e, ctx, 3, $T, $f);
            }
        }
        si! {
            pub(super) fn $n4(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                unary!(e, ctx, 4, $T, $f);
            }
        }
    };
}

/// The 1- to 4-slot and n-way binary stages (`DECLARE_BINARY_*`).
macro_rules! binary_family {
    ($T:ty, $f:expr, $n1:ident, $n2:ident, $n3:ident, $n4:ident, $nn:ident) => {
        si! {
            pub(super) fn $n1(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary!(e, ctx, 1, $T, $f);
            }
        }
        si! {
            pub(super) fn $n2(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary!(e, ctx, 2, $T, $f);
            }
        }
        si! {
            pub(super) fn $n3(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary!(e, ctx, 3, $T, $f);
            }
        }
        si! {
            pub(super) fn $n4(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary!(e, ctx, 4, $T, $f);
            }
        }
        si! {
            pub(super) fn $nn(ctx: BinaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary_packed!(e, ctx, $T, $f);
            }
        }
    };
}

/// The 1- to 4-slot and n-way ternary stages (`DECLARE_TERNARY_*`); `a`, `x`, `y` are `k` slots
/// apart (`p, p+k, p+2k`).
macro_rules! ternary_family {
    ($T:ty, $f:expr, $n1:ident, $n2:ident, $n3:ident, $n4:ident, $nn:ident) => {
        si! {
            pub(super) fn $n1(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                ternary!(e, ctx, 1, $T, $f);
            }
        }
        si! {
            pub(super) fn $n2(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                ternary!(e, ctx, 2, $T, $f);
            }
        }
        si! {
            pub(super) fn $n3(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                ternary!(e, ctx, 3, $T, $f);
            }
        }
        si! {
            pub(super) fn $n4(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
                ternary!(e, ctx, 4, $T, $f);
            }
        }
        si! {
            pub(super) fn $nn(ctx: TernaryOpCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
                ternary_packed!(e, ctx, $T, $f);
            }
        }
    };
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L2638-L2638 (chrome/m156)
    /// `clamp_01_`: `min(max(0.0f, v), 1.0f)`.
    fn clamp_01_(v: F) -> F {
        min_f(max_f(F::splat(0.0), v), F::splat(1.0))
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L4997-L5023 (chrome/m156) (cmp*_fn for U32)
    /// `cond_to_mask` of an unsigned comparison, as the `I32` that Skia `memcpy`s into the slot.
    fn u32_mask(m: U32) -> I32 {
        cond_to_mask(m.bit_cast())
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L2922-L2924 (chrome/m156)
    /// `lerp(from, to, t)`: `mad(to - from, t, from)`.
    fn lerp(from: F, to: F, t: F) -> F {
        mad(to - from, t, from)
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L4736-L4755 (chrome/m156) (cast_to_*_fn)
// The casts keep the destination's slot bits: int → float converts, float → int truncates (the
// tier's `to_i32`/`trunc_`).
unary_family!(
    I32,
    |v: I32| v.cast::<f32>(),
    cast_to_float_from_int,
    cast_to_float_from_2_ints,
    cast_to_float_from_3_ints,
    cast_to_float_from_4_ints
);
unary_family!(
    U32,
    |v: U32| v.cast::<f32>(),
    cast_to_float_from_uint,
    cast_to_float_from_2_uints,
    cast_to_float_from_3_uints,
    cast_to_float_from_4_uints
);
unary_family!(
    F,
    |v: F| to_i32(v),
    cast_to_int_from_float,
    cast_to_int_from_2_floats,
    cast_to_int_from_3_floats,
    cast_to_int_from_4_floats
);
unary_family!(
    F,
    |v: F| trunc_(v),
    cast_to_uint_from_float,
    cast_to_uint_from_2_floats,
    cast_to_uint_from_3_floats,
    cast_to_uint_from_4_floats
);
// Port of: src/opts/SkRasterPipeline_opts.h#L4758-L4768 (chrome/m156) (abs_fn, floor_fn, ceil_fn)
unary_family!(
    I32,
    |v: I32| abs_i(v),
    abs_int,
    abs_2_ints,
    abs_3_ints,
    abs_4_ints
);
unary_family!(
    F,
    |v: F| floor_(v),
    floor_float,
    floor_2_floats,
    floor_3_floats,
    floor_4_floats
);
unary_family!(
    F,
    |v: F| ceil_(v),
    ceil_float,
    ceil_2_floats,
    ceil_3_floats,
    ceil_4_floats
);

// Port of: src/opts/SkRasterPipeline_opts.h#L4929-L4985 (chrome/m156) (add/sub/mul/div/bitwise)
binary_family!(
    F,
    |d: F, s: F| d + s,
    add_float,
    add_2_floats,
    add_3_floats,
    add_4_floats,
    add_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| d + s,
    add_int,
    add_2_ints,
    add_3_ints,
    add_4_ints,
    add_n_ints
);
binary_family!(
    F,
    |d: F, s: F| d - s,
    sub_float,
    sub_2_floats,
    sub_3_floats,
    sub_4_floats,
    sub_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| d - s,
    sub_int,
    sub_2_ints,
    sub_3_ints,
    sub_4_ints,
    sub_n_ints
);
binary_family!(
    F,
    |d: F, s: F| d * s,
    mul_float,
    mul_2_floats,
    mul_3_floats,
    mul_4_floats,
    mul_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| d * s,
    mul_int,
    mul_2_ints,
    mul_3_ints,
    mul_4_ints,
    mul_n_ints
);
binary_family!(
    F,
    |d: F, s: F| d / s,
    div_float,
    div_2_floats,
    div_3_floats,
    div_4_floats,
    div_n_floats
);
// Port of: src/opts/SkRasterPipeline_opts.h#L4950-L4973 (chrome/m156) (div_fn for I32 and U32)
binary_family!(
    I32,
    |d: I32, s: I32| div_i32(d, s),
    div_int,
    div_2_ints,
    div_3_ints,
    div_4_ints,
    div_n_ints
);
binary_family!(
    U32,
    |d: U32, s: U32| div_u32(d, s),
    div_uint,
    div_2_uints,
    div_3_uints,
    div_4_uints,
    div_n_uints
);
binary_family!(
    I32,
    |d: I32, s: I32| d & s,
    bitwise_and_int,
    bitwise_and_2_ints,
    bitwise_and_3_ints,
    bitwise_and_4_ints,
    bitwise_and_n_ints
);
binary_family!(
    I32,
    |d: I32, s: I32| d | s,
    bitwise_or_int,
    bitwise_or_2_ints,
    bitwise_or_3_ints,
    bitwise_or_4_ints,
    bitwise_or_n_ints
);
binary_family!(
    I32,
    |d: I32, s: I32| d ^ s,
    bitwise_xor_int,
    bitwise_xor_2_ints,
    bitwise_xor_3_ints,
    bitwise_xor_4_ints,
    bitwise_xor_n_ints
);

// Port of: src/opts/SkRasterPipeline_opts.h#L4987-L4995 (chrome/m156) (max_fn, min_fn)
binary_family!(
    F,
    |d: F, s: F| max_f(d, s),
    max_float,
    max_2_floats,
    max_3_floats,
    max_4_floats,
    max_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| max_i(d, s),
    max_int,
    max_2_ints,
    max_3_ints,
    max_4_ints,
    max_n_ints
);
binary_family!(
    U32,
    |d: U32, s: U32| max_u(d, s),
    max_uint,
    max_2_uints,
    max_3_uints,
    max_4_uints,
    max_n_uints
);
binary_family!(
    F,
    |d: F, s: F| min_f(d, s),
    min_float,
    min_2_floats,
    min_3_floats,
    min_4_floats,
    min_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| min_i(d, s),
    min_int,
    min_2_ints,
    min_3_ints,
    min_4_ints,
    min_n_ints
);
binary_family!(
    U32,
    |d: U32, s: U32| min_u(d, s),
    min_uint,
    min_2_uints,
    min_3_uints,
    min_4_uints,
    min_n_uints
);

// Port of: src/opts/SkRasterPipeline_opts.h#L5033-L5035 (chrome/m156) (mod_fn)
binary_family!(
    F,
    |d: F, s: F| nmad(s, floor_(d / s), d),
    mod_float,
    mod_2_floats,
    mod_3_floats,
    mod_4_floats,
    mod_n_floats
);

// Port of: src/opts/SkRasterPipeline_opts.h#L4997-L5023 (chrome/m156) (cmp*_fn)
// `cond_to_mask(*dst < *src)`, stored as the `I32` mask. Unsigned masks are bit-cast to `I32`
// first (`cond_to_mask` is the identity on SIMD tiers and 0/1 → 0/-1 on Scalar).
binary_family!(
    F,
    |d: F, s: F| cond_to_mask(d.lt_mask(s)),
    cmplt_float,
    cmplt_2_floats,
    cmplt_3_floats,
    cmplt_4_floats,
    cmplt_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| cond_to_mask(d.lt_mask(s)),
    cmplt_int,
    cmplt_2_ints,
    cmplt_3_ints,
    cmplt_4_ints,
    cmplt_n_ints
);
binary_family!(
    U32,
    |d: U32, s: U32| u32_mask(d.lt_mask(s)),
    cmplt_uint,
    cmplt_2_uints,
    cmplt_3_uints,
    cmplt_4_uints,
    cmplt_n_uints
);
binary_family!(
    F,
    |d: F, s: F| cond_to_mask(d.le_mask(s)),
    cmple_float,
    cmple_2_floats,
    cmple_3_floats,
    cmple_4_floats,
    cmple_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| cond_to_mask(d.le_mask(s)),
    cmple_int,
    cmple_2_ints,
    cmple_3_ints,
    cmple_4_ints,
    cmple_n_ints
);
binary_family!(
    U32,
    |d: U32, s: U32| u32_mask(d.le_mask(s)),
    cmple_uint,
    cmple_2_uints,
    cmple_3_uints,
    cmple_4_uints,
    cmple_n_uints
);
binary_family!(
    F,
    |d: F, s: F| cond_to_mask(d.eq_mask(s)),
    cmpeq_float,
    cmpeq_2_floats,
    cmpeq_3_floats,
    cmpeq_4_floats,
    cmpeq_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| cond_to_mask(d.eq_mask(s)),
    cmpeq_int,
    cmpeq_2_ints,
    cmpeq_3_ints,
    cmpeq_4_ints,
    cmpeq_n_ints
);
binary_family!(
    F,
    |d: F, s: F| cond_to_mask(d.ne_mask(s)),
    cmpne_float,
    cmpne_2_floats,
    cmpne_3_floats,
    cmpne_4_floats,
    cmpne_n_floats
);
binary_family!(
    I32,
    |d: I32, s: I32| cond_to_mask(d.ne_mask(s)),
    cmpne_int,
    cmpne_2_ints,
    cmpne_3_ints,
    cmpne_4_ints,
    cmpne_n_ints
);

// Port of: src/opts/SkRasterPipeline_opts.h#L5095-L5131 (chrome/m156) (immediate forms)
/// A float immediate stage (`DECLARE_IMM_BINARY_FLOAT`): the constant is `sk_bit_cast<float>`.
macro_rules! imm_float {
    ($name:ident, $f:expr) => {
        si! {
            pub(super) fn $name(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
                #[allow(clippy::cast_sign_loss)] // sk_bit_cast<float>(int32_t)
                let value = ctx.value as u32;
                binary_imm!(e, ctx, 1, F, F::splat(f32::from_bits(value)), $f);
            }
        }
    };
}
/// An `int` immediate stage over `$n` slots (`DECLARE_IMM_BINARY_INT`).
macro_rules! imm_int {
    ($name:ident, $n:literal, $f:expr) => {
        si! {
            pub(super) fn $name(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
                binary_imm!(e, ctx, $n, I32, I32::splat(ctx.value), $f);
            }
        }
    };
}
/// A `uint` immediate stage (`DECLARE_IMM_BINARY_UINT`).
macro_rules! imm_uint {
    ($name:ident, $f:expr) => {
        si! {
            pub(super) fn $name(ctx: ConstantCtx, _p: &mut Regs, e: &mut Params<'_, '_>) {
                #[allow(clippy::cast_sign_loss)] // sk_bit_cast<uint32_t>(int32_t)
                let value = ctx.value as u32;
                binary_imm!(e, ctx, 1, U32, U32::splat(value), $f);
            }
        }
    };
}

imm_float!(add_imm_float, |d: F, s: F| d + s);
imm_int!(add_imm_int, 1, |d: I32, s: I32| d + s);
imm_float!(mul_imm_float, |d: F, s: F| d * s);
imm_int!(mul_imm_int, 1, |d: I32, s: I32| d * s);
imm_int!(bitwise_and_imm_int, 1, |d: I32, s: I32| d & s);
imm_int!(bitwise_and_imm_2_ints, 2, |d: I32, s: I32| d & s);
imm_int!(bitwise_and_imm_3_ints, 3, |d: I32, s: I32| d & s);
imm_int!(bitwise_and_imm_4_ints, 4, |d: I32, s: I32| d & s);
imm_float!(max_imm_float, |d: F, s: F| max_f(d, s));
imm_float!(min_imm_float, |d: F, s: F| min_f(d, s));
imm_int!(bitwise_xor_imm_int, 1, |d: I32, s: I32| d ^ s);
imm_float!(cmplt_imm_float, |d: F, s: F| cond_to_mask(d.lt_mask(s)));
imm_int!(cmplt_imm_int, 1, |d: I32, s: I32| cond_to_mask(
    d.lt_mask(s)
));
imm_uint!(cmplt_imm_uint, |d: U32, s: U32| u32_mask(d.lt_mask(s)));
imm_float!(cmple_imm_float, |d: F, s: F| cond_to_mask(d.le_mask(s)));
imm_int!(cmple_imm_int, 1, |d: I32, s: I32| cond_to_mask(
    d.le_mask(s)
));
imm_uint!(cmple_imm_uint, |d: U32, s: U32| u32_mask(d.le_mask(s)));
imm_float!(cmpeq_imm_float, |d: F, s: F| cond_to_mask(d.eq_mask(s)));
imm_int!(cmpeq_imm_int, 1, |d: I32, s: I32| cond_to_mask(
    d.eq_mask(s)
));
imm_float!(cmpne_imm_float, |d: F, s: F| cond_to_mask(d.ne_mask(s)));
imm_int!(cmpne_imm_int, 1, |d: I32, s: I32| cond_to_mask(
    d.ne_mask(s)
));

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5146-L5149 (chrome/m156)
    pub(super) fn dot_2_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let m = &mut e.ptr_mut(ctx)[..4 * SLOT];
        let d = |i: usize| F::load_bytes(&m[i * SLOT..]);
        let r = mad(d(0), d(2), d(1) * d(3));
        r.store_bytes(m);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5151-L5155 (chrome/m156)
    pub(super) fn dot_3_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let m = &mut e.ptr_mut(ctx)[..6 * SLOT];
        let d = |i: usize| F::load_bytes(&m[i * SLOT..]);
        let r = mad(d(0), d(3), mad(d(1), d(4), d(2) * d(5)));
        r.store_bytes(m);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5157-L5162 (chrome/m156)
    pub(super) fn dot_4_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        let m = &mut e.ptr_mut(ctx)[..8 * SLOT];
        let d = |i: usize| F::load_bytes(&m[i * SLOT..]);
        let r = mad(d(0), d(4), mad(d(1), d(5), mad(d(2), d(6), d(3) * d(7))));
        r.store_bytes(m);
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L5166-L5211 (chrome/m156) (`matrix_multiply<N>`)
/// `matrix_multiply<$n>`: the result matrix at `base + ctx.dst` is followed by the left and the
/// right matrix (column-major, one `F` per element).
macro_rules! matrix_multiply {
    ($e:ident, $ctx:ident, $n:literal) => {{
        let out_columns = usize::from($ctx.right_columns);
        let out_rows = usize::from($ctx.left_rows);

        debug_assert!(out_columns >= 1);
        debug_assert!(out_rows >= 1);
        debug_assert!(out_columns <= 4);
        debug_assert!(out_rows <= 4);

        debug_assert_eq!($ctx.left_columns, $ctx.right_rows);
        debug_assert_eq!($n, usize::from($ctx.left_columns)); // N should match the result width

        // Get the adjacent result, left- and right-matrices.
        let dst = $e.base_ptr($ctx.dst);
        let left_start = out_columns * out_rows;
        let right_start = left_start + $n * out_rows;
        let m = &mut $e.ptr_mut(dst)[..(right_start + $n * out_columns) * SLOT];
        let (result, inputs) = m.split_at_mut(left_start * SLOT);
        let left = |i: usize| F::load_bytes(&inputs[i * SLOT..]);
        let right = |i: usize| F::load_bytes(&inputs[(right_start - left_start + i) * SLOT..]);

        // Emit each matrix element.
        let mut out = 0;
        for c in 0..out_columns {
            for r in 0..out_rows {
                // Dot a vector from leftMtx[*][r] with rightMtx[c][*].
                let mut left_row = r;
                let mut right_column = c * $n;

                let mut element = left(left_row) * right(right_column);
                for _ in 1..$n {
                    left_row += out_rows;
                    right_column += 1;
                    element = mad(left(left_row), right(right_column), element);
                }

                element.store_bytes(&mut result[out * SLOT..]);
                out += 1;
            }
        }
    }};
}

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5213-L5215 (chrome/m156)
    pub(super) fn matrix_multiply_2(
        ctx: MatrixMultiplyCtx,
        _p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) {
        matrix_multiply!(e, ctx, 2);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5217-L5219 (chrome/m156)
    pub(super) fn matrix_multiply_3(
        ctx: MatrixMultiplyCtx,
        _p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) {
        matrix_multiply!(e, ctx, 3);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5221-L5223 (chrome/m156)
    pub(super) fn matrix_multiply_4(
        ctx: MatrixMultiplyCtx,
        _p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) {
        matrix_multiply!(e, ctx, 4);
    }

    // Port of: src/opts/SkRasterPipeline_opts.h#L5225-L5246 (chrome/m156)
    pub(super) fn refract_4_floats(ctx: MemPtr, _p: &mut Regs, e: &mut Params<'_, '_>) {
        // Algorithm adapted from https://registry.khronos.org/OpenGL-Refpages/gl4/html/refract.xhtml
        let m = &mut e.ptr_mut(ctx)[..9 * SLOT];
        // incident = dst[0..4], normal = dst[4..8], eta = dst[8].
        let d = |i: usize| F::load_bytes(&m[i * SLOT..]);
        let eta = d(8);

        let dot_ni = mad(d(4), d(0), mad(d(5), d(1), mad(d(6), d(2), d(7) * d(3))));

        let one = F::splat(1.0);
        let k = one - eta * eta * (one - dot_ni * dot_ni);
        let sqrt_k = sqrt_(k);

        let mut out = [F::splat(0.0); 4];
        for (idx, out) in out.iter_mut().enumerate() {
            *out = if_then_else_f(
                k.ge_mask(F::splat(0.0)),
                eta * d(idx) - (eta * dot_ni + sqrt_k) * d(4 + idx),
                F::splat(0.0),
            );
        }
        for (idx, out) in out.iter().enumerate() {
            out.store_bytes(&mut m[idx * SLOT..]);
        }
    }
}

// Port of: src/opts/SkRasterPipeline_opts.h#L5274-L5277 (chrome/m156) (mix_fn)
// We reorder the arguments to match lerp's GLSL-style order (interpolation point last).
ternary_family!(
    F,
    |a: F, x: F, y: F| lerp(x, y, a),
    mix_float,
    mix_2_floats,
    mix_3_floats,
    mix_4_floats,
    mix_n_floats
);
// Port of: src/opts/SkRasterPipeline_opts.h#L5279-L5282 (chrome/m156) (mix_fn for I32)
// We reorder the arguments to match if_then_else's expected order (y before x).
ternary_family!(
    I32,
    |a: I32, x: I32, y: I32| if_then_else_i(a, y, x),
    mix_int,
    mix_2_ints,
    mix_3_ints,
    mix_4_ints,
    mix_n_ints
);

si! {
    // Port of: src/opts/SkRasterPipeline_opts.h#L5284-L5292 (chrome/m156)
    // (`smoothstep_fn` and the n-way ternary stage)
    pub(super) fn smoothstep_n_floats(
        ctx: TernaryOpCtx,
        _p: &mut Regs,
        e: &mut Params<'_, '_>,
    ) {
        ternary_packed!(e, ctx, F, |edge0: F, edge1: F, x: F| {
            let t = clamp_01_((x - edge0) / (edge1 - edge0));
            t * t * (F::splat(3.0) - F::splat(2.0) * t)
        });
    }
}
