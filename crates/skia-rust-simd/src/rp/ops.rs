// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipelineOpList.h

//! The raster pipeline's op list ([`Op`]) and the typed program ([`Stage`]), both generated from
//! one table, [`rp_ops!`], transcribed from `SkRasterPipelineOpList.h` in Skia's order.
//!
//! # The table
//! One line per op, in `SkRasterPipelineOp` order:
//!
//! ```text
//! name  Variant  [context type]  highp-kind  lowp-kind  task;
//! ```
//!
//! - `name`: Skia's op name (also the name of the stage function in every tier, and
//!   [`Op::name`]).
//! - `Variant`: the [`Op`] and [`Stage`] variant.
//! - `[context type]`: the Rust type of the op's context, held by value in the [`Stage`] variant
//!   (empty for Skia's `NoCtx`). Every context type is `Copy`: a small value, a reference
//!   (`&'a T`) to read-only data, or a [`MemoryCtx`]/[`MemPtr`] naming writable memory bound per
//!   run (see [`contexts`](super::contexts)).
//! - highp kind: `n` (a normal stage) or `br` (a branch: the stage returns the program-counter
//!   offset, Skia's `HIGHP_BRANCH_STAGE`).
//! - lowp kind: `pp`, `gg`, `gp` (Skia's `LOWP_STAGE_PP`/`_GG`/`_GP`), or `hi` for the ops in
//!   `SK_RASTER_PIPELINE_OPS_HIGHP_ONLY`.
//! - `task`: the design task (`docs/design/raster-pipeline.md` §5) that ports the stage; `A3`
//!   stages are implemented, the others are stubs that panic until their task lands, `P3` is
//!   Phase 3 (shaders).
//!
//! `macro_rules!` callbacks consume the table (`rp_ops!(callback)` expands to
//! `callback! { <table> }`): this module defines [`Op`] and [`Stage`] with it, and each tier's
//! interpreters generate their dispatch `match` with it (`rp/tiers/{highp,lowp}/mod.rs`).
//!
//! The table is also exported, hidden, as `skia_rust_simd::rp_op_table!` for test tooling that
//! needs one generated `match` arm per op (the `oracle/rp-diff` replayer builds a [`Stage`] from an
//! op name and a serialized context with it). A callback outside this crate must have the
//! context types in scope (`use skia_rust_simd::rp::contexts::*; use core::cell::Cell;`).

#[allow(clippy::wildcard_imports)] // every context type appears in the generated `Stage`
use super::contexts::*;
use core::cell::Cell;

/// Invokes `$cb! { <the op table> }` (see the [module docs](self)).
#[doc(hidden)]
#[macro_export]
macro_rules! rp_op_table {
    ($cb:ident) => {
        $cb! {
            move_src_dst                        MoveSrcDst                      []                                  n  pp A3;
            move_dst_src                        MoveDstSrc                      []                                  n  pp A3;
            swap_src_dst                        SwapSrcDst                      []                                  n  pp A3;
            clamp_01                            Clamp01                         []                                  n  pp B4;
            clamp_a_01                          ClampA01                        []                                  n  pp B4;
            clamp_gamut                         ClampGamut                      []                                  n  pp B4;
            premul                              Premul                          []                                  n  pp B4;
            premul_dst                          PremulDst                       []                                  n  pp B4;
            force_opaque                        ForceOpaque                     []                                  n  pp B4;
            force_opaque_dst                    ForceOpaqueDst                  []                                  n  pp B4;
            set_rgb                             SetRgb                          [&'a [f32; 3]]                      n  pp B4;
            swap_rb                             SwapRb                          []                                  n  pp B1;
            swap_rb_dst                         SwapRbDst                       []                                  n  pp B1;
            black_color                         BlackColor                      []                                  n  pp B4;
            white_color                         WhiteColor                      []                                  n  pp B4;
            uniform_color                       UniformColor                    [&'a UniformColorCtx]               n  pp B4;
            uniform_color_dst                   UniformColorDst                 [&'a UniformColorCtx]               n  pp B4;
            seed_shader                         SeedShader                      []                                  n  gg A3;
            load_a8                             LoadA8                          [MemoryCtx]                         n  pp B1;
            load_a8_dst                         LoadA8Dst                       [MemoryCtx]                         n  pp B1;
            store_a8                            StoreA8                         [MemoryCtx]                         n  pp B1;
            gather_a8                           GatherA8                        [&'a GatherCtx<'a>]                 n  gp B1;
            load_565                            Load565                         [MemoryCtx]                         n  pp B1;
            load_565_dst                        Load565Dst                      [MemoryCtx]                         n  pp B1;
            store_565                           Store565                        [MemoryCtx]                         n  pp B1;
            gather_565                          Gather565                       [&'a GatherCtx<'a>]                 n  gp B1;
            load_4444                           Load4444                        [MemoryCtx]                         n  pp B1;
            load_4444_dst                       Load4444Dst                     [MemoryCtx]                         n  pp B1;
            store_4444                          Store4444                       [MemoryCtx]                         n  pp B1;
            gather_4444                         Gather4444                      [&'a GatherCtx<'a>]                 n  gp B1;
            load_8888                           Load8888                        [MemoryCtx]                         n  pp B1;
            load_8888_dst                       Load8888Dst                     [MemoryCtx]                         n  pp B1;
            store_8888                          Store8888                       [MemoryCtx]                         n  pp B1;
            gather_8888                         Gather8888                      [&'a GatherCtx<'a>]                 n  gp B1;
            load_rg88                           LoadRg88                        [MemoryCtx]                         n  pp B1;
            load_rg88_dst                       LoadRg88Dst                     [MemoryCtx]                         n  pp B1;
            store_rg88                          StoreRg88                       [MemoryCtx]                         n  pp B1;
            gather_rg88                         GatherRg88                      [&'a GatherCtx<'a>]                 n  gp B1;
            store_r8                            StoreR8                         [MemoryCtx]                         n  pp B1;
            alpha_to_gray                       AlphaToGray                     []                                  n  pp B1;
            alpha_to_gray_dst                   AlphaToGrayDst                  []                                  n  pp B1;
            alpha_to_red                        AlphaToRed                      []                                  n  pp B1;
            alpha_to_red_dst                    AlphaToRedDst                   []                                  n  pp B1;
            bt709_luminance_or_luma_to_alpha    Bt709LuminanceOrLumaToAlpha     []                                  n  pp B4;
            bt709_luminance_or_luma_to_rgb      Bt709LuminanceOrLumaToRgb       []                                  n  pp B4;
            bilerp_clamp_8888                   BilerpClamp8888                 [&'a GatherCtx<'a>]                 n  gp P3;
            load_src                            LoadSrc                         [MemPtr]                            n  pp A3;
            store_src                           StoreSrc                        [MemPtr]                            n  pp A3;
            store_src_a                         StoreSrcA                       [MemPtr]                            n  pp A3;
            load_dst                            LoadDst                         [MemPtr]                            n  pp A3;
            store_dst                           StoreDst                        [MemPtr]                            n  pp A3;
            scale_u8                            ScaleU8                         [MemoryCtx]                         n  pp B3;
            scale_565                           Scale565                        [MemoryCtx]                         n  pp B3;
            scale_1_float                       Scale1Float                     [&'a Cell<f32>]                     n  pp B3;
            scale_native                        ScaleNative                     [MemPtr]                            n  pp B3;
            lerp_u8                             LerpU8                          [MemoryCtx]                         n  pp B3;
            lerp_565                            Lerp565                         [MemoryCtx]                         n  pp B3;
            lerp_1_float                        Lerp1Float                      [&'a Cell<f32>]                     n  pp B3;
            lerp_native                         LerpNative                      [MemPtr]                            n  pp B3;
            dstatop                             Dstatop                         []                                  n  pp B3;
            dstin                               Dstin                           []                                  n  pp B3;
            dstout                              Dstout                          []                                  n  pp B3;
            dstover                             Dstover                         []                                  n  pp B3;
            srcatop                             Srcatop                         []                                  n  pp B3;
            srcin                               Srcin                           []                                  n  pp B3;
            srcout                              Srcout                          []                                  n  pp B3;
            srcover                             Srcover                         []                                  n  pp A3;
            clear                               Clear                           []                                  n  pp B3;
            modulate                            Modulate                        []                                  n  pp B3;
            multiply                            Multiply                        []                                  n  pp B3;
            plus_                               Plus                            []                                  n  pp B3;
            screen                              Screen                          []                                  n  pp B3;
            xor_                                Xor                             []                                  n  pp B3;
            darken                              Darken                          []                                  n  pp B3;
            difference                          Difference                      []                                  n  pp B3;
            exclusion                           Exclusion                       []                                  n  pp B3;
            hardlight                           Hardlight                       []                                  n  pp B3;
            lighten                             Lighten                         []                                  n  pp B3;
            overlay                             Overlay                         []                                  n  pp B3;
            srcover_rgba_8888                   SrcoverRgba8888                 [MemoryCtx]                         n  pp B1;
            matrix_translate                    MatrixTranslate                 [[f32; 2]]                          n  gg B5;
            matrix_scale_translate              MatrixScaleTranslate            [&'a [f32; 4]]                      n  gg B5;
            matrix_2x3                          Matrix2x3                       [&'a [f32; 6]]                      n  gg B5;
            matrix_perspective                  MatrixPerspective               [&'a [f32; 9]]                      n  gg B5;
            decal_x                             DecalX                          [&'a DecalTileCtx]                  n  gg B5;
            decal_y                             DecalY                          [&'a DecalTileCtx]                  n  gg B5;
            decal_x_and_y                       DecalXAndY                      [&'a DecalTileCtx]                  n  gg B5;
            check_decal_mask                    CheckDecalMask                  [&'a DecalTileCtx]                  n  pp B5;
            clamp_x_1                           ClampX1                         []                                  n  gg B5;
            mirror_x_1                          MirrorX1                        []                                  n  gg B5;
            repeat_x_1                          RepeatX1                        []                                  n  gg B5;
            clamp_x_and_y                       ClampXAndY                      [&'a CoordClampCtx]                 n  gg B5;
            evenly_spaced_gradient              EvenlySpacedGradient            [&'a GradientCtx]                   n  gp P3;
            gradient                            Gradient                        [&'a GradientCtx]                   n  gp P3;
            evenly_spaced_2_stop_gradient       EvenlySpaced2StopGradient       [&'a EvenlySpaced2StopGradientCtx]  n  gp P3;
            xy_to_unit_angle                    XyToUnitAngle                   []                                  n  gg P3;
            xy_to_radius                        XyToRadius                      []                                  n  gg P3;
            emboss                              Emboss                          [EmbossCtx]                         n  pp B4;
            swizzle                             Swizzle                         [[u8; 4]]                           n  pp B4;
            debug_x                             DebugX                          [MemoryCtx]                         n  gg B1;
            debug_y                             DebugY                          [MemoryCtx]                         n  gg B1;
            debug_r                             DebugR                          [MemoryCtx]                         n  pp B1;
            debug_g                             DebugG                          [MemoryCtx]                         n  pp B1;
            debug_b                             DebugB                          [MemoryCtx]                         n  pp B1;
            debug_a                             DebugA                          [MemoryCtx]                         n  pp B1;
            debug_r_255                         DebugR255                       [MemoryCtx]                         n  pp B1;
            debug_g_255                         DebugG255                       [MemoryCtx]                         n  pp B1;
            debug_b_255                         DebugB255                       [MemoryCtx]                         n  pp B1;
            debug_a_255                         DebugA255                       [MemoryCtx]                         n  pp B1;
            callback                            Callback                        [&'a CallbackCtx<'a>]               n  hi B6d;
            stack_checkpoint                    StackCheckpoint                 []                                  n  hi A3;
            stack_rewind                        StackRewind                     []                                  n  hi A3;
            unbounded_set_rgb                   UnboundedSetRgb                 [&'a [f32; 3]]                      n  hi B4;
            unbounded_uniform_color             UnboundedUniformColor           [&'a UniformColorCtx]               n  hi B4;
            unpremul                            Unpremul                        []                                  n  hi B4;
            unpremul_polar                      UnpremulPolar                   []                                  n  hi B4;
            dither                              Dither                          [f32]                               n  hi B4;
            load_16161616                       Load16161616                    [MemoryCtx]                         n  hi B2;
            load_16161616_dst                   Load16161616Dst                 [MemoryCtx]                         n  hi B2;
            store_16161616                      Store16161616                   [MemoryCtx]                         n  hi B2;
            gather_16161616                     Gather16161616                  [&'a GatherCtx<'a>]                 n  hi B2;
            load_a16                            LoadA16                         [MemoryCtx]                         n  hi B2;
            load_a16_dst                        LoadA16Dst                      [MemoryCtx]                         n  hi B2;
            store_a16                           StoreA16                        [MemoryCtx]                         n  hi B2;
            gather_a16                          GatherA16                       [&'a GatherCtx<'a>]                 n  hi B2;
            load_r16                            LoadR16                         [MemoryCtx]                         n  hi B2;
            load_r16_dst                        LoadR16Dst                      [MemoryCtx]                         n  hi B2;
            store_r16                           StoreR16                        [MemoryCtx]                         n  hi B2;
            gather_r16                          GatherR16                       [&'a GatherCtx<'a>]                 n  hi B2;
            load_rg1616                         LoadRg1616                      [MemoryCtx]                         n  hi B2;
            load_rg1616_dst                     LoadRg1616Dst                   [MemoryCtx]                         n  hi B2;
            store_rg1616                        StoreRg1616                     [MemoryCtx]                         n  hi B2;
            gather_rg1616                       GatherRg1616                    [&'a GatherCtx<'a>]                 n  hi B2;
            load_f16                            LoadF16                         [MemoryCtx]                         n  hi B2;
            load_f16_dst                        LoadF16Dst                      [MemoryCtx]                         n  hi B2;
            store_f16                           StoreF16                        [MemoryCtx]                         n  hi B2;
            gather_f16                          GatherF16                       [&'a GatherCtx<'a>]                 n  hi B2;
            load_af16                           LoadAf16                        [MemoryCtx]                         n  hi B2;
            load_af16_dst                       LoadAf16Dst                     [MemoryCtx]                         n  hi B2;
            store_af16                          StoreAf16                       [MemoryCtx]                         n  hi B2;
            gather_af16                         GatherAf16                      [&'a GatherCtx<'a>]                 n  hi B2;
            load_rf16                           LoadRf16                        [MemoryCtx]                         n  hi B2;
            load_rf16_dst                       LoadRf16Dst                     [MemoryCtx]                         n  hi B2;
            store_rf16                          StoreRf16                       [MemoryCtx]                         n  hi B2;
            gather_rf16                         GatherRf16                      [&'a GatherCtx<'a>]                 n  hi B2;
            load_rgf16                          LoadRgf16                       [MemoryCtx]                         n  hi B2;
            load_rgf16_dst                      LoadRgf16Dst                    [MemoryCtx]                         n  hi B2;
            store_rgf16                         StoreRgf16                      [MemoryCtx]                         n  hi B2;
            gather_rgf16                        GatherRgf16                     [&'a GatherCtx<'a>]                 n  hi B2;
            load_f32                            LoadF32                         [MemoryCtx]                         n  hi B2;
            load_f32_dst                        LoadF32Dst                      [MemoryCtx]                         n  hi B2;
            store_f32                           StoreF32                        [MemoryCtx]                         n  hi B2;
            gather_f32                          GatherF32                       [&'a GatherCtx<'a>]                 n  hi B2;
            load_1010102                        Load1010102                     [MemoryCtx]                         n  hi B2;
            load_1010102_dst                    Load1010102Dst                  [MemoryCtx]                         n  hi B2;
            store_1010102                       Store1010102                    [MemoryCtx]                         n  hi B2;
            gather_1010102                      Gather1010102                   [&'a GatherCtx<'a>]                 n  hi B2;
            load_1010102_xr                     Load1010102Xr                   [MemoryCtx]                         n  hi B2;
            load_1010102_xr_dst                 Load1010102XrDst                [MemoryCtx]                         n  hi B2;
            store_1010102_xr                    Store1010102Xr                  [MemoryCtx]                         n  hi B2;
            gather_1010102_xr                   Gather1010102Xr                 [&'a GatherCtx<'a>]                 n  hi B2;
            load_10x6                           Load10x6                        [MemoryCtx]                         n  hi B2;
            load_10x6_dst                       Load10x6Dst                     [MemoryCtx]                         n  hi B2;
            store_10x6                          Store10x6                       [MemoryCtx]                         n  hi B2;
            gather_10x6                         Gather10x6                      [&'a GatherCtx<'a>]                 n  hi B2;
            gather_10101010_xr                  Gather10101010Xr                [&'a GatherCtx<'a>]                 n  hi B2;
            load_10101010_xr                    Load10101010Xr                  [MemoryCtx]                         n  hi B2;
            load_10101010_xr_dst                Load10101010XrDst               [MemoryCtx]                         n  hi B2;
            store_10101010_xr                   Store10101010Xr                 [MemoryCtx]                         n  hi B2;
            store_src_rg                        StoreSrcRg                      [MemPtr]                            n  hi B2;
            load_src_rg                         LoadSrcRg                       [MemPtr]                            n  hi B2;
            byte_tables                         ByteTables                      [&'a TablesCtx]                     n  hi B4;
            colorburn                           Colorburn                       []                                  n  hi B3;
            colordodge                          Colordodge                      []                                  n  hi B3;
            softlight                           Softlight                       []                                  n  hi B3;
            hue                                 Hue                             []                                  n  hi B3;
            saturation                          Saturation                      []                                  n  hi B3;
            color                               Color                           []                                  n  hi B3;
            luminosity                          Luminosity                      []                                  n  hi B3;
            matrix_3x3                          Matrix3x3                       [&'a [f32; 9]]                      n  hi B4;
            matrix_3x4                          Matrix3x4                       [&'a [f32; 12]]                     n  hi B4;
            matrix_4x5                          Matrix4x5                       [&'a [f32; 20]]                     n  hi B4;
            matrix_4x3                          Matrix4x3                       [&'a [f32; 12]]                     n  hi B4;
            parametric                          Parametric                      [&'a TransferFunction]              n  hi B4;
            gamma_                              Gamma                           [f32]                               n  hi B4;
            PQish                               PQish                           [&'a TransferFunction]              n  hi B4;
            HLGish                              HLGish                          [&'a TransferFunction]              n  hi B4;
            HLGinvish                           HLGinvish                       [&'a TransferFunction]              n  hi B4;
            ootf                                Ootf                            [&'a [f32; 4]]                      n  hi B4;
            rgb_to_hsl                          RgbToHsl                        []                                  n  hi B4;
            hsl_to_rgb                          HslToRgb                        []                                  n  hi B4;
            css_lab_to_xyz                      CssLabToXyz                     []                                  n  hi B4;
            css_oklab_to_linear_srgb            CssOklabToLinearSrgb            []                                  n  hi B4;
            css_oklab_gamut_map_to_linear_srgb  CssOklabGamutMapToLinearSrgb    []                                  n  hi B4;
            css_hcl_to_lab                      CssHclToLab                     []                                  n  hi B4;
            css_hsl_to_srgb                     CssHslToSrgb                    []                                  n  hi B4;
            css_hwb_to_srgb                     CssHwbToSrgb                    []                                  n  hi B4;
            gauss_a_to_rgba                     GaussAToRgba                    []                                  n  hi B4;
            mirror_x                            MirrorX                         [&'a TileCtx]                       n  hi B5;
            repeat_x                            RepeatX                         [&'a TileCtx]                       n  hi B5;
            mirror_y                            MirrorY                         [&'a TileCtx]                       n  hi B5;
            repeat_y                            RepeatY                         [&'a TileCtx]                       n  hi B5;
            negate_x                            NegateX                         []                                  n  hi P3;
            bilerp_clamp_8888_force_highp       BilerpClamp8888ForceHighp       [&'a GatherCtx<'a>]                 n  hi P3;
            bicubic_clamp_8888                  BicubicClamp8888                [&'a GatherCtx<'a>]                 n  hi P3;
            bilinear_setup                      BilinearSetup                   [&'a SamplerCtx]                    n  hi P3;
            bilinear_nx                         BilinearNx                      [&'a SamplerCtx]                    n  hi P3;
            bilinear_px                         BilinearPx                      [&'a SamplerCtx]                    n  hi P3;
            bilinear_ny                         BilinearNy                      [&'a SamplerCtx]                    n  hi P3;
            bilinear_py                         BilinearPy                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_setup                       BicubicSetup                    [&'a SamplerCtx]                    n  hi P3;
            bicubic_n3x                         BicubicN3x                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_n1x                         BicubicN1x                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_p1x                         BicubicP1x                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_p3x                         BicubicP3x                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_n3y                         BicubicN3y                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_n1y                         BicubicN1y                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_p1y                         BicubicP1y                      [&'a SamplerCtx]                    n  hi P3;
            bicubic_p3y                         BicubicP3y                      [&'a SamplerCtx]                    n  hi P3;
            accumulate                          Accumulate                      [&'a SamplerCtx]                    n  hi P3;
            perlin_noise                        PerlinNoise                     [&'a PerlinNoiseCtx<'a>]            n  hi P3;
            mipmap_linear_init                  MipmapLinearInit                [&'a MipmapCtx]                     n  hi P3;
            mipmap_linear_update                MipmapLinearUpdate              [&'a MipmapCtx]                     n  hi P3;
            mipmap_linear_finish                MipmapLinearFinish              [&'a MipmapCtx]                     n  hi P3;
            xy_to_2pt_conical_strip             XyTo2ptConicalStrip             [&'a Conical2PtCtx]                 n  hi P3;
            xy_to_2pt_conical_focal_on_circle   XyTo2ptConicalFocalOnCircle     []                                  n  hi P3;
            xy_to_2pt_conical_well_behaved      XyTo2ptConicalWellBehaved       [&'a Conical2PtCtx]                 n  hi P3;
            xy_to_2pt_conical_smaller           XyTo2ptConicalSmaller           [&'a Conical2PtCtx]                 n  hi P3;
            xy_to_2pt_conical_greater           XyTo2ptConicalGreater           [&'a Conical2PtCtx]                 n  hi P3;
            alter_2pt_conical_compensate_focal  Alter2ptConicalCompensateFocal  [&'a Conical2PtCtx]                 n  hi P3;
            alter_2pt_conical_unswap            Alter2ptConicalUnswap           []                                  n  hi P3;
            mask_2pt_conical_nan                Mask2ptConicalNan               [&'a Conical2PtCtx]                 n  hi P3;
            mask_2pt_conical_degenerates        Mask2ptConicalDegenerates       [&'a Conical2PtCtx]                 n  hi P3;
            apply_vector_mask                   ApplyVectorMask                 [&'a Cell<[u32; MAX_STRIDE_HIGHP]>] n  hi P3;
            set_base_pointer                    SetBasePointer                  [MemPtr]                            n  hi A3;
            init_lane_masks                     InitLaneMasks                   []                                  n  hi B6a;
            store_device_xy01                   StoreDeviceXy01                 [MemPtr]                            n  hi B6a;
            exchange_src                        ExchangeSrc                     [MemPtr]                            n  hi B6a;
            load_condition_mask                 LoadConditionMask               [MemPtr]                            n  hi B6a;
            store_condition_mask                StoreConditionMask              [MemPtr]                            n  hi B6a;
            merge_condition_mask                MergeConditionMask              [MemPtr]                            n  hi B6a;
            merge_inv_condition_mask            MergeInvConditionMask           [MemPtr]                            n  hi B6a;
            load_loop_mask                      LoadLoopMask                    [MemPtr]                            n  hi B6a;
            store_loop_mask                     StoreLoopMask                   [MemPtr]                            n  hi B6a;
            mask_off_loop_mask                  MaskOffLoopMask                 []                                  n  hi B6a;
            reenable_loop_mask                  ReenableLoopMask                [MemPtr]                            n  hi B6a;
            merge_loop_mask                     MergeLoopMask                   [MemPtr]                            n  hi B6a;
            case_op                             CaseOp                          [CaseOpCtx]                         n  hi B6a;
            continue_op                         ContinueOp                      [MemPtr]                            n  hi B6a;
            load_return_mask                    LoadReturnMask                  [MemPtr]                            n  hi B6a;
            store_return_mask                   StoreReturnMask                 [MemPtr]                            n  hi B6a;
            mask_off_return_mask                MaskOffReturnMask               []                                  n  hi B6a;
            branch_if_all_lanes_active          BranchIfAllLanesActive          [BranchCtx]                         br hi A3;
            branch_if_any_lanes_active          BranchIfAnyLanesActive          [BranchCtx]                         br hi A3;
            branch_if_no_lanes_active           BranchIfNoLanesActive           [BranchCtx]                         br hi A3;
            branch_if_no_active_lanes_eq        BranchIfNoActiveLanesEq         [&'a BranchIfEqualCtx]              br hi A3;
            jump                                Jump                            [BranchCtx]                         br hi A3;
            bitwise_and_imm_4_ints              BitwiseAndImm4Ints              [ConstantCtx]                       n  hi B6b;
            bitwise_and_imm_3_ints              BitwiseAndImm3Ints              [ConstantCtx]                       n  hi B6b;
            bitwise_and_imm_2_ints              BitwiseAndImm2Ints              [ConstantCtx]                       n  hi B6b;
            bitwise_and_imm_int                 BitwiseAndImmInt                [ConstantCtx]                       n  hi B6b;
            bitwise_and_n_ints                  BitwiseAndNInts                 [BinaryOpCtx]                       n  hi B6b;
            bitwise_and_int                     BitwiseAndInt                   [MemPtr]                            n  hi B6b;
            bitwise_and_2_ints                  BitwiseAnd2Ints                 [MemPtr]                            n  hi B6b;
            bitwise_and_3_ints                  BitwiseAnd3Ints                 [MemPtr]                            n  hi B6b;
            bitwise_and_4_ints                  BitwiseAnd4Ints                 [MemPtr]                            n  hi B6b;
            bitwise_or_n_ints                   BitwiseOrNInts                  [BinaryOpCtx]                       n  hi B6b;
            bitwise_or_int                      BitwiseOrInt                    [MemPtr]                            n  hi B6b;
            bitwise_or_2_ints                   BitwiseOr2Ints                  [MemPtr]                            n  hi B6b;
            bitwise_or_3_ints                   BitwiseOr3Ints                  [MemPtr]                            n  hi B6b;
            bitwise_or_4_ints                   BitwiseOr4Ints                  [MemPtr]                            n  hi B6b;
            bitwise_xor_imm_int                 BitwiseXorImmInt                [ConstantCtx]                       n  hi B6b;
            bitwise_xor_n_ints                  BitwiseXorNInts                 [BinaryOpCtx]                       n  hi B6b;
            bitwise_xor_int                     BitwiseXorInt                   [MemPtr]                            n  hi B6b;
            bitwise_xor_2_ints                  BitwiseXor2Ints                 [MemPtr]                            n  hi B6b;
            bitwise_xor_3_ints                  BitwiseXor3Ints                 [MemPtr]                            n  hi B6b;
            bitwise_xor_4_ints                  BitwiseXor4Ints                 [MemPtr]                            n  hi B6b;
            cast_to_float_from_int              CastToFloatFromInt              [MemPtr]                            n  hi B6b;
            cast_to_float_from_2_ints           CastToFloatFrom2Ints            [MemPtr]                            n  hi B6b;
            cast_to_float_from_3_ints           CastToFloatFrom3Ints            [MemPtr]                            n  hi B6b;
            cast_to_float_from_4_ints           CastToFloatFrom4Ints            [MemPtr]                            n  hi B6b;
            cast_to_float_from_uint             CastToFloatFromUint             [MemPtr]                            n  hi B6b;
            cast_to_float_from_2_uints          CastToFloatFrom2Uints           [MemPtr]                            n  hi B6b;
            cast_to_float_from_3_uints          CastToFloatFrom3Uints           [MemPtr]                            n  hi B6b;
            cast_to_float_from_4_uints          CastToFloatFrom4Uints           [MemPtr]                            n  hi B6b;
            cast_to_int_from_float              CastToIntFromFloat              [MemPtr]                            n  hi B6b;
            cast_to_int_from_2_floats           CastToIntFrom2Floats            [MemPtr]                            n  hi B6b;
            cast_to_int_from_3_floats           CastToIntFrom3Floats            [MemPtr]                            n  hi B6b;
            cast_to_int_from_4_floats           CastToIntFrom4Floats            [MemPtr]                            n  hi B6b;
            cast_to_uint_from_float             CastToUintFromFloat             [MemPtr]                            n  hi B6b;
            cast_to_uint_from_2_floats          CastToUintFrom2Floats           [MemPtr]                            n  hi B6b;
            cast_to_uint_from_3_floats          CastToUintFrom3Floats           [MemPtr]                            n  hi B6b;
            cast_to_uint_from_4_floats          CastToUintFrom4Floats           [MemPtr]                            n  hi B6b;
            abs_int                             AbsInt                          [MemPtr]                            n  hi B6b;
            abs_2_ints                          Abs2Ints                        [MemPtr]                            n  hi B6b;
            abs_3_ints                          Abs3Ints                        [MemPtr]                            n  hi B6b;
            abs_4_ints                          Abs4Ints                        [MemPtr]                            n  hi B6b;
            floor_float                         FloorFloat                      [MemPtr]                            n  hi B6b;
            floor_2_floats                      Floor2Floats                    [MemPtr]                            n  hi B6b;
            floor_3_floats                      Floor3Floats                    [MemPtr]                            n  hi B6b;
            floor_4_floats                      Floor4Floats                    [MemPtr]                            n  hi B6b;
            ceil_float                          CeilFloat                       [MemPtr]                            n  hi B6b;
            ceil_2_floats                       Ceil2Floats                     [MemPtr]                            n  hi B6b;
            ceil_3_floats                       Ceil3Floats                     [MemPtr]                            n  hi B6b;
            ceil_4_floats                       Ceil4Floats                     [MemPtr]                            n  hi B6b;
            invsqrt_float                       InvsqrtFloat                    [MemPtr]                            n  hi B6c;
            invsqrt_2_floats                    Invsqrt2Floats                  [MemPtr]                            n  hi B6c;
            invsqrt_3_floats                    Invsqrt3Floats                  [MemPtr]                            n  hi B6c;
            invsqrt_4_floats                    Invsqrt4Floats                  [MemPtr]                            n  hi B6c;
            inverse_mat2                        InverseMat2                     [MemPtr]                            n  hi B6c;
            inverse_mat3                        InverseMat3                     [MemPtr]                            n  hi B6c;
            inverse_mat4                        InverseMat4                     [MemPtr]                            n  hi B6c;
            sin_float                           SinFloat                        [MemPtr]                            n  hi B6c;
            cos_float                           CosFloat                        [MemPtr]                            n  hi B6c;
            tan_float                           TanFloat                        [MemPtr]                            n  hi B6c;
            asin_float                          AsinFloat                       [MemPtr]                            n  hi B6c;
            acos_float                          AcosFloat                       [MemPtr]                            n  hi B6c;
            atan_float                          AtanFloat                       [MemPtr]                            n  hi B6c;
            atan2_n_floats                      Atan2NFloats                    [BinaryOpCtx]                       n  hi B6c;
            sqrt_float                          SqrtFloat                       [MemPtr]                            n  hi B6c;
            pow_n_floats                        PowNFloats                      [BinaryOpCtx]                       n  hi B6c;
            exp_float                           ExpFloat                        [MemPtr]                            n  hi B6c;
            exp2_float                          Exp2Float                       [MemPtr]                            n  hi B6c;
            log_float                           LogFloat                        [MemPtr]                            n  hi B6c;
            log2_float                          Log2Float                       [MemPtr]                            n  hi B6c;
            refract_4_floats                    Refract4Floats                  [MemPtr]                            n  hi B6b;
            copy_uniform                        CopyUniform                     [&'a UniformCtx]                n  hi B6a;
            copy_2_uniforms                     Copy2Uniforms                   [&'a UniformCtx]                n  hi B6a;
            copy_3_uniforms                     Copy3Uniforms                   [&'a UniformCtx]                n  hi B6a;
            copy_4_uniforms                     Copy4Uniforms                   [&'a UniformCtx]                n  hi B6a;
            copy_constant                       CopyConstant                    [ConstantCtx]                       n  hi B6a;
            splat_2_constants                   Splat2Constants                 [ConstantCtx]                       n  hi B6a;
            splat_3_constants                   Splat3Constants                 [ConstantCtx]                       n  hi B6a;
            splat_4_constants                   Splat4Constants                 [ConstantCtx]                       n  hi B6a;
            copy_slot_masked                    CopySlotMasked                  [BinaryOpCtx]                       n  hi B6a;
            copy_2_slots_masked                 Copy2SlotsMasked                [BinaryOpCtx]                       n  hi B6a;
            copy_3_slots_masked                 Copy3SlotsMasked                [BinaryOpCtx]                       n  hi B6a;
            copy_4_slots_masked                 Copy4SlotsMasked                [BinaryOpCtx]                       n  hi B6a;
            copy_from_indirect_unmasked         CopyFromIndirectUnmasked        [&'a CopyIndirectCtx]               n  hi B6a;
            copy_from_indirect_uniform_unmasked CopyFromIndirectUniformUnmasked [&'a CopyIndirectUniformCtx]    n  hi B6a;
            copy_to_indirect_masked             CopyToIndirectMasked            [&'a CopyIndirectCtx]               n  hi B6a;
            swizzle_copy_to_indirect_masked     SwizzleCopyToIndirectMasked     [&'a SwizzleCopyIndirectCtx]        n  hi B6a;
            copy_slot_unmasked                  CopySlotUnmasked                [BinaryOpCtx]                       n  hi B6a;
            copy_2_slots_unmasked               Copy2SlotsUnmasked              [BinaryOpCtx]                       n  hi B6a;
            copy_3_slots_unmasked               Copy3SlotsUnmasked              [BinaryOpCtx]                       n  hi B6a;
            copy_4_slots_unmasked               Copy4SlotsUnmasked              [BinaryOpCtx]                       n  hi B6a;
            copy_immutable_unmasked             CopyImmutableUnmasked           [BinaryOpCtx]                       n  hi B6a;
            copy_2_immutables_unmasked          Copy2ImmutablesUnmasked         [BinaryOpCtx]                       n  hi B6a;
            copy_3_immutables_unmasked          Copy3ImmutablesUnmasked         [BinaryOpCtx]                       n  hi B6a;
            copy_4_immutables_unmasked          Copy4ImmutablesUnmasked         [BinaryOpCtx]                       n  hi B6a;
            swizzle_copy_slot_masked            SwizzleCopySlotMasked           [&'a SwizzleCopyCtx]                n  hi B6a;
            swizzle_copy_2_slots_masked         SwizzleCopy2SlotsMasked         [&'a SwizzleCopyCtx]                n  hi B6a;
            swizzle_copy_3_slots_masked         SwizzleCopy3SlotsMasked         [&'a SwizzleCopyCtx]                n  hi B6a;
            swizzle_copy_4_slots_masked         SwizzleCopy4SlotsMasked         [&'a SwizzleCopyCtx]                n  hi B6a;
            swizzle_1                           Swizzle1                        [SwizzleCtx]                        n  hi B6a;
            swizzle_2                           Swizzle2                        [SwizzleCtx]                        n  hi B6a;
            swizzle_3                           Swizzle3                        [SwizzleCtx]                        n  hi B6a;
            swizzle_4                           Swizzle4                        [SwizzleCtx]                        n  hi B6a;
            shuffle                             Shuffle                         [&'a ShuffleCtx]                    n  hi B6a;
            matrix_multiply_2                   MatrixMultiply2                 [MatrixMultiplyCtx]                 n  hi B6b;
            matrix_multiply_3                   MatrixMultiply3                 [MatrixMultiplyCtx]                 n  hi B6b;
            matrix_multiply_4                   MatrixMultiply4                 [MatrixMultiplyCtx]                 n  hi B6b;
            smoothstep_n_floats                 SmoothstepNFloats               [TernaryOpCtx]                      n  hi B6b;
            dot_2_floats                        Dot2Floats                      [MemPtr]                            n  hi B6b;
            dot_3_floats                        Dot3Floats                      [MemPtr]                            n  hi B6b;
            dot_4_floats                        Dot4Floats                      [MemPtr]                            n  hi B6b;
            add_imm_float                       AddImmFloat                     [ConstantCtx]                       n  hi B6b;
            add_n_floats                        AddNFloats                      [BinaryOpCtx]                       n  hi B6b;
            add_float                           AddFloat                        [MemPtr]                            n  hi B6b;
            add_2_floats                        Add2Floats                      [MemPtr]                            n  hi B6b;
            add_3_floats                        Add3Floats                      [MemPtr]                            n  hi B6b;
            add_4_floats                        Add4Floats                      [MemPtr]                            n  hi B6b;
            add_imm_int                         AddImmInt                       [ConstantCtx]                       n  hi B6b;
            add_n_ints                          AddNInts                        [BinaryOpCtx]                       n  hi B6b;
            add_int                             AddInt                          [MemPtr]                            n  hi B6b;
            add_2_ints                          Add2Ints                        [MemPtr]                            n  hi B6b;
            add_3_ints                          Add3Ints                        [MemPtr]                            n  hi B6b;
            add_4_ints                          Add4Ints                        [MemPtr]                            n  hi B6b;
            sub_n_floats                        SubNFloats                      [BinaryOpCtx]                       n  hi B6b;
            sub_float                           SubFloat                        [MemPtr]                            n  hi B6b;
            sub_2_floats                        Sub2Floats                      [MemPtr]                            n  hi B6b;
            sub_3_floats                        Sub3Floats                      [MemPtr]                            n  hi B6b;
            sub_4_floats                        Sub4Floats                      [MemPtr]                            n  hi B6b;
            sub_n_ints                          SubNInts                        [BinaryOpCtx]                       n  hi B6b;
            sub_int                             SubInt                          [MemPtr]                            n  hi B6b;
            sub_2_ints                          Sub2Ints                        [MemPtr]                            n  hi B6b;
            sub_3_ints                          Sub3Ints                        [MemPtr]                            n  hi B6b;
            sub_4_ints                          Sub4Ints                        [MemPtr]                            n  hi B6b;
            mul_imm_float                       MulImmFloat                     [ConstantCtx]                       n  hi B6b;
            mul_n_floats                        MulNFloats                      [BinaryOpCtx]                       n  hi B6b;
            mul_float                           MulFloat                        [MemPtr]                            n  hi B6b;
            mul_2_floats                        Mul2Floats                      [MemPtr]                            n  hi B6b;
            mul_3_floats                        Mul3Floats                      [MemPtr]                            n  hi B6b;
            mul_4_floats                        Mul4Floats                      [MemPtr]                            n  hi B6b;
            mul_imm_int                         MulImmInt                       [ConstantCtx]                       n  hi B6b;
            mul_n_ints                          MulNInts                        [BinaryOpCtx]                       n  hi B6b;
            mul_int                             MulInt                          [MemPtr]                            n  hi B6b;
            mul_2_ints                          Mul2Ints                        [MemPtr]                            n  hi B6b;
            mul_3_ints                          Mul3Ints                        [MemPtr]                            n  hi B6b;
            mul_4_ints                          Mul4Ints                        [MemPtr]                            n  hi B6b;
            div_n_floats                        DivNFloats                      [BinaryOpCtx]                       n  hi B6b;
            div_float                           DivFloat                        [MemPtr]                            n  hi B6b;
            div_2_floats                        Div2Floats                      [MemPtr]                            n  hi B6b;
            div_3_floats                        Div3Floats                      [MemPtr]                            n  hi B6b;
            div_4_floats                        Div4Floats                      [MemPtr]                            n  hi B6b;
            div_n_ints                          DivNInts                        [BinaryOpCtx]                       n  hi B6b;
            div_int                             DivInt                          [MemPtr]                            n  hi B6b;
            div_2_ints                          Div2Ints                        [MemPtr]                            n  hi B6b;
            div_3_ints                          Div3Ints                        [MemPtr]                            n  hi B6b;
            div_4_ints                          Div4Ints                        [MemPtr]                            n  hi B6b;
            div_n_uints                         DivNUints                       [BinaryOpCtx]                       n  hi B6b;
            div_uint                            DivUint                         [MemPtr]                            n  hi B6b;
            div_2_uints                         Div2Uints                       [MemPtr]                            n  hi B6b;
            div_3_uints                         Div3Uints                       [MemPtr]                            n  hi B6b;
            div_4_uints                         Div4Uints                       [MemPtr]                            n  hi B6b;
            max_imm_float                       MaxImmFloat                     [ConstantCtx]                       n  hi B6b;
            max_n_floats                        MaxNFloats                      [BinaryOpCtx]                       n  hi B6b;
            max_float                           MaxFloat                        [MemPtr]                            n  hi B6b;
            max_2_floats                        Max2Floats                      [MemPtr]                            n  hi B6b;
            max_3_floats                        Max3Floats                      [MemPtr]                            n  hi B6b;
            max_4_floats                        Max4Floats                      [MemPtr]                            n  hi B6b;
            max_n_ints                          MaxNInts                        [BinaryOpCtx]                       n  hi B6b;
            max_int                             MaxInt                          [MemPtr]                            n  hi B6b;
            max_2_ints                          Max2Ints                        [MemPtr]                            n  hi B6b;
            max_3_ints                          Max3Ints                        [MemPtr]                            n  hi B6b;
            max_4_ints                          Max4Ints                        [MemPtr]                            n  hi B6b;
            max_n_uints                         MaxNUints                       [BinaryOpCtx]                       n  hi B6b;
            max_uint                            MaxUint                         [MemPtr]                            n  hi B6b;
            max_2_uints                         Max2Uints                       [MemPtr]                            n  hi B6b;
            max_3_uints                         Max3Uints                       [MemPtr]                            n  hi B6b;
            max_4_uints                         Max4Uints                       [MemPtr]                            n  hi B6b;
            min_imm_float                       MinImmFloat                     [ConstantCtx]                       n  hi B6b;
            min_n_floats                        MinNFloats                      [BinaryOpCtx]                       n  hi B6b;
            min_float                           MinFloat                        [MemPtr]                            n  hi B6b;
            min_2_floats                        Min2Floats                      [MemPtr]                            n  hi B6b;
            min_3_floats                        Min3Floats                      [MemPtr]                            n  hi B6b;
            min_4_floats                        Min4Floats                      [MemPtr]                            n  hi B6b;
            min_n_ints                          MinNInts                        [BinaryOpCtx]                       n  hi B6b;
            min_int                             MinInt                          [MemPtr]                            n  hi B6b;
            min_2_ints                          Min2Ints                        [MemPtr]                            n  hi B6b;
            min_3_ints                          Min3Ints                        [MemPtr]                            n  hi B6b;
            min_4_ints                          Min4Ints                        [MemPtr]                            n  hi B6b;
            min_n_uints                         MinNUints                       [BinaryOpCtx]                       n  hi B6b;
            min_uint                            MinUint                         [MemPtr]                            n  hi B6b;
            min_2_uints                         Min2Uints                       [MemPtr]                            n  hi B6b;
            min_3_uints                         Min3Uints                       [MemPtr]                            n  hi B6b;
            min_4_uints                         Min4Uints                       [MemPtr]                            n  hi B6b;
            mod_n_floats                        ModNFloats                      [BinaryOpCtx]                       n  hi B6b;
            mod_float                           ModFloat                        [MemPtr]                            n  hi B6b;
            mod_2_floats                        Mod2Floats                      [MemPtr]                            n  hi B6b;
            mod_3_floats                        Mod3Floats                      [MemPtr]                            n  hi B6b;
            mod_4_floats                        Mod4Floats                      [MemPtr]                            n  hi B6b;
            mix_n_floats                        MixNFloats                      [TernaryOpCtx]                      n  hi B6b;
            mix_float                           MixFloat                        [MemPtr]                            n  hi B6b;
            mix_2_floats                        Mix2Floats                      [MemPtr]                            n  hi B6b;
            mix_3_floats                        Mix3Floats                      [MemPtr]                            n  hi B6b;
            mix_4_floats                        Mix4Floats                      [MemPtr]                            n  hi B6b;
            mix_n_ints                          MixNInts                        [TernaryOpCtx]                      n  hi B6b;
            mix_int                             MixInt                          [MemPtr]                            n  hi B6b;
            mix_2_ints                          Mix2Ints                        [MemPtr]                            n  hi B6b;
            mix_3_ints                          Mix3Ints                        [MemPtr]                            n  hi B6b;
            mix_4_ints                          Mix4Ints                        [MemPtr]                            n  hi B6b;
            cmplt_imm_float                     CmpltImmFloat                   [ConstantCtx]                       n  hi B6b;
            cmplt_n_floats                      CmpltNFloats                    [BinaryOpCtx]                       n  hi B6b;
            cmplt_float                         CmpltFloat                      [MemPtr]                            n  hi B6b;
            cmplt_2_floats                      Cmplt2Floats                    [MemPtr]                            n  hi B6b;
            cmplt_3_floats                      Cmplt3Floats                    [MemPtr]                            n  hi B6b;
            cmplt_4_floats                      Cmplt4Floats                    [MemPtr]                            n  hi B6b;
            cmplt_imm_int                       CmpltImmInt                     [ConstantCtx]                       n  hi B6b;
            cmplt_n_ints                        CmpltNInts                      [BinaryOpCtx]                       n  hi B6b;
            cmplt_int                           CmpltInt                        [MemPtr]                            n  hi B6b;
            cmplt_2_ints                        Cmplt2Ints                      [MemPtr]                            n  hi B6b;
            cmplt_3_ints                        Cmplt3Ints                      [MemPtr]                            n  hi B6b;
            cmplt_4_ints                        Cmplt4Ints                      [MemPtr]                            n  hi B6b;
            cmplt_imm_uint                      CmpltImmUint                    [ConstantCtx]                       n  hi B6b;
            cmplt_n_uints                       CmpltNUints                     [BinaryOpCtx]                       n  hi B6b;
            cmplt_uint                          CmpltUint                       [MemPtr]                            n  hi B6b;
            cmplt_2_uints                       Cmplt2Uints                     [MemPtr]                            n  hi B6b;
            cmplt_3_uints                       Cmplt3Uints                     [MemPtr]                            n  hi B6b;
            cmplt_4_uints                       Cmplt4Uints                     [MemPtr]                            n  hi B6b;
            cmple_imm_float                     CmpleImmFloat                   [ConstantCtx]                       n  hi B6b;
            cmple_n_floats                      CmpleNFloats                    [BinaryOpCtx]                       n  hi B6b;
            cmple_float                         CmpleFloat                      [MemPtr]                            n  hi B6b;
            cmple_2_floats                      Cmple2Floats                    [MemPtr]                            n  hi B6b;
            cmple_3_floats                      Cmple3Floats                    [MemPtr]                            n  hi B6b;
            cmple_4_floats                      Cmple4Floats                    [MemPtr]                            n  hi B6b;
            cmple_imm_int                       CmpleImmInt                     [ConstantCtx]                       n  hi B6b;
            cmple_n_ints                        CmpleNInts                      [BinaryOpCtx]                       n  hi B6b;
            cmple_int                           CmpleInt                        [MemPtr]                            n  hi B6b;
            cmple_2_ints                        Cmple2Ints                      [MemPtr]                            n  hi B6b;
            cmple_3_ints                        Cmple3Ints                      [MemPtr]                            n  hi B6b;
            cmple_4_ints                        Cmple4Ints                      [MemPtr]                            n  hi B6b;
            cmple_imm_uint                      CmpleImmUint                    [ConstantCtx]                       n  hi B6b;
            cmple_n_uints                       CmpleNUints                     [BinaryOpCtx]                       n  hi B6b;
            cmple_uint                          CmpleUint                       [MemPtr]                            n  hi B6b;
            cmple_2_uints                       Cmple2Uints                     [MemPtr]                            n  hi B6b;
            cmple_3_uints                       Cmple3Uints                     [MemPtr]                            n  hi B6b;
            cmple_4_uints                       Cmple4Uints                     [MemPtr]                            n  hi B6b;
            cmpeq_imm_float                     CmpeqImmFloat                   [ConstantCtx]                       n  hi B6b;
            cmpeq_n_floats                      CmpeqNFloats                    [BinaryOpCtx]                       n  hi B6b;
            cmpeq_float                         CmpeqFloat                      [MemPtr]                            n  hi B6b;
            cmpeq_2_floats                      Cmpeq2Floats                    [MemPtr]                            n  hi B6b;
            cmpeq_3_floats                      Cmpeq3Floats                    [MemPtr]                            n  hi B6b;
            cmpeq_4_floats                      Cmpeq4Floats                    [MemPtr]                            n  hi B6b;
            cmpeq_imm_int                       CmpeqImmInt                     [ConstantCtx]                       n  hi B6b;
            cmpeq_n_ints                        CmpeqNInts                      [BinaryOpCtx]                       n  hi B6b;
            cmpeq_int                           CmpeqInt                        [MemPtr]                            n  hi B6b;
            cmpeq_2_ints                        Cmpeq2Ints                      [MemPtr]                            n  hi B6b;
            cmpeq_3_ints                        Cmpeq3Ints                      [MemPtr]                            n  hi B6b;
            cmpeq_4_ints                        Cmpeq4Ints                      [MemPtr]                            n  hi B6b;
            cmpne_imm_float                     CmpneImmFloat                   [ConstantCtx]                       n  hi B6b;
            cmpne_n_floats                      CmpneNFloats                    [BinaryOpCtx]                       n  hi B6b;
            cmpne_float                         CmpneFloat                      [MemPtr]                            n  hi B6b;
            cmpne_2_floats                      Cmpne2Floats                    [MemPtr]                            n  hi B6b;
            cmpne_3_floats                      Cmpne3Floats                    [MemPtr]                            n  hi B6b;
            cmpne_4_floats                      Cmpne4Floats                    [MemPtr]                            n  hi B6b;
            cmpne_imm_int                       CmpneImmInt                     [ConstantCtx]                       n  hi B6b;
            cmpne_n_ints                        CmpneNInts                      [BinaryOpCtx]                       n  hi B6b;
            cmpne_int                           CmpneInt                        [MemPtr]                            n  hi B6b;
            cmpne_2_ints                        Cmpne2Ints                      [MemPtr]                            n  hi B6b;
            cmpne_3_ints                        Cmpne3Ints                      [MemPtr]                            n  hi B6b;
            cmpne_4_ints                        Cmpne4Ints                      [MemPtr]                            n  hi B6b;
            trace_line                          TraceLine                       [&'a TraceLineCtx<'a>]              n  hi B6d;
            trace_var                           TraceVar                        [&'a TraceVarCtx<'a>]               n  hi B6d;
            trace_enter                         TraceEnter                      [&'a TraceFuncCtx<'a>]              n  hi B6d;
            trace_exit                          TraceExit                       [&'a TraceFuncCtx<'a>]              n  hi B6d;
            trace_scope                         TraceScope                      [&'a TraceScopeCtx<'a>]             n  hi B6d;
        }
    };
}
pub(crate) use crate::rp_op_table as rp_ops;

/// `1` for a lowp op, `0` for a highp-only op (by lowp kind).
macro_rules! lowp_count {
    (hi) => {
        0
    };
    ($kind:ident) => {
        1
    };
}

/// Defines [`Op`], [`Stage`] and their tables from the op table.
macro_rules! define_ops {
    ($($name:ident $variant:ident [$($ctx:ty)?] $hk:ident $lk:ident $task:ident;)*) => {
        /// `SkRasterPipelineOp`: every raster pipeline op, in Skia's order (the discriminant is
        /// Skia's enum value).
        ///
        /// The ops in `SK_RASTER_PIPELINE_OPS_LOWP` come first ([`NUM_LOWP_OPS`] of them); they
        /// have lowp implementations on every tier with a lowp pipeline.
        #[doc(alias = "SkRasterPipelineOp")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum Op {
            $(
                #[doc = concat!("`SkRasterPipelineOp::", stringify!($name), "`")]
                $variant,
            )*
        }

        /// `kNumRasterPipelineHighpOps`: the number of ops.
        #[doc(alias = "kNumRasterPipelineHighpOps")]
        pub const NUM_HIGHP_OPS: usize = [$(Op::$variant),*].len();

        /// `kNumRasterPipelineLowpOps`: the number of ops with lowp implementations (the first
        /// ops of [`Op`]).
        #[doc(alias = "kNumRasterPipelineLowpOps")]
        pub const NUM_LOWP_OPS: usize = 0 $(+ lowp_count!($lk))*;

        impl Op {
            /// Every op, in order.
            pub const ALL: [Op; NUM_HIGHP_OPS] = [$(Op::$variant),*];

            /// Skia's name for the op (`SkRasterPipeline::GetOpName`), e.g. `"load_8888"`.
            #[doc(alias = "GetOpName")]
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Op::$variant => stringify!($name),)*
                }
            }

            /// Whether the op has a lowp implementation (it is in
            /// `SK_RASTER_PIPELINE_OPS_LOWP`). Tiers without a lowp pipeline (`Scalar`) run
            /// every op in highp.
            #[must_use]
            pub const fn has_lowp(self) -> bool {
                (self as usize) < NUM_LOWP_OPS
            }

            /// The design task that ports this op's stages (`"A3"`, `"B1"`…`"B6d"`, `"P3"` for
            /// Phase 3); stages of unported ops panic naming it.
            #[must_use]
            pub const fn task(self) -> &'static str {
                match self {
                    $(Op::$variant => stringify!($task),)*
                }
            }
        }

        /// One stage of a raster pipeline program: an [`Op`] with its context (Skia's
        /// `SkRasterPipelineStage` = stage function + `void* ctx`, typed).
        ///
        /// Contexts that Skia passes as pointers to constant data are borrowed (`&'a T`) or
        /// held by value; memory written by the pipeline (pixels, `load_src`/`store_src`
        /// buffers, `SkSL` slots) is named by a [`MemoryCtx`] or [`MemPtr`] and bound per run with
        /// [`MemoryBindings`](super::MemoryBindings).
        #[doc(alias = "SkRasterPipelineStage")]
        #[derive(Clone, Copy, Debug)]
        pub enum Stage<'a> {
            $(
                #[doc = concat!("`", stringify!($name), "`")]
                $variant $(($ctx))?,
            )*
        }

        impl Stage<'_> {
            /// The stage's op.
            #[must_use]
            pub const fn op(&self) -> Op {
                match self {
                    $(Stage::$variant { .. } => Op::$variant,)*
                }
            }
        }
    };
}

rp_ops!(define_ops);
