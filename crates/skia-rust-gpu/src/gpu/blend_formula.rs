// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/BlendFormula.h, src/gpu/BlendFormula.cpp

//! `skgpu::BlendFormula`: how a draw's blend mode is split between the shader's outputs and
//! the fixed-function blend state, plus the table that picks one for every Porter-Duff mode.

use bitflags::bitflags;
use skia_rust_core::blend_mode::BlendMode;

use crate::gpu::blend::{
    BlendCoeff, BlendEquation, blend_allows_coverage_as_alpha, blend_coeff_refs_src2,
    blend_coeffs_use_dst_color, blend_coeffs_use_src_color, blend_modifies_dst,
};

/// Values the shader can write to primary and secondary outputs. These are all modulated by
/// coverage. We will ignore the multiplies when not using coverage.
// Port of: src/gpu/BlendFormula.h#L30-L40 (chrome/m156)
#[doc(alias = "skgpu::BlendFormula::OutputType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum OutputType {
    /// 0
    None,
    /// inputCoverage
    Coverage,
    /// inputColor * inputCoverage
    Modulate,
    /// inputColor.a * inputCoverage
    SAModulate,
    /// (1 - inputColor.a) * inputCoverage
    ISAModulate,
    /// (1 - inputColor) * inputCoverage
    ISCModulate,
}

bitflags! {
    /// Deduced properties of a [`BlendFormula`].
    // Port of: src/gpu/BlendFormula.h#L58-L66 (chrome/m156)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Properties: u8 {
        const MODIFIES_DST = 1 << 0;
        const UNAFFECTED_BY_DST = 1 << 1;
        const UNAFFECTED_BY_DST_IF_OPAQUE = 1 << 2;
        const USES_INPUT_COLOR = 1 << 3;
        const CAN_TWEAK_ALPHA_FOR_COVERAGE = 1 << 4;
    }
}

/// A blend formula: the outputs the shader writes and the blend state that combines them with
/// the destination.
// Port of: src/gpu/BlendFormula.h#L23-L175 (chrome/m156)
#[doc(alias = "skgpu::BlendFormula")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendFormula {
    primary_output_type: OutputType,
    secondary_output_type: OutputType,
    blend_equation: BlendEquation,
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
    props: Properties,
}

impl BlendFormula {
    // Port of: src/gpu/BlendFormula.h#L42-L53 (chrome/m156)
    /// Builds a formula, deducing its properties. The formula must already be optimized.
    #[must_use]
    pub const fn new(
        primary_out: OutputType,
        secondary_out: OutputType,
        equation: BlendEquation,
        src_coeff: BlendCoeff,
        dst_coeff: BlendCoeff,
    ) -> BlendFormula {
        BlendFormula {
            primary_output_type: primary_out,
            secondary_output_type: secondary_out,
            blend_equation: equation,
            src_coeff,
            dst_coeff,
            props: get_properties(primary_out, secondary_out, equation, src_coeff, dst_coeff),
        }
    }

    // Port of: src/gpu/BlendFormula.h#L72-L74 (chrome/m156)
    /// Is there a secondary (dual-source) output?
    #[must_use]
    pub const fn has_secondary_output(&self) -> bool {
        !matches!(self.secondary_output_type, OutputType::None)
    }

    // Port of: src/gpu/BlendFormula.h#L76-L78 (chrome/m156)
    /// Does the formula write the destination?
    #[must_use]
    pub const fn modifies_dst(&self) -> bool {
        self.props.contains(Properties::MODIFIES_DST)
    }

    // Port of: src/gpu/BlendFormula.h#L80-L82 (chrome/m156)
    /// Is the result independent of the destination?
    #[must_use]
    pub const fn unaffected_by_dst(&self) -> bool {
        self.props.contains(Properties::UNAFFECTED_BY_DST)
    }

    // Port of: src/gpu/BlendFormula.h#L86-L88 (chrome/m156)
    /// Like [`Self::unaffected_by_dst`], for the case where the source is opaque.
    #[must_use]
    pub const fn unaffected_by_dst_if_opaque(&self) -> bool {
        self.props.contains(Properties::UNAFFECTED_BY_DST_IF_OPAQUE)
    }

    // Port of: src/gpu/BlendFormula.h#L90-L92 (chrome/m156)
    /// Does the formula read the input color?
    #[must_use]
    pub const fn uses_input_color(&self) -> bool {
        self.props.contains(Properties::USES_INPUT_COLOR)
    }

    // Port of: src/gpu/BlendFormula.h#L94-L96 (chrome/m156)
    /// May coverage be folded into the source alpha?
    #[must_use]
    pub const fn can_tweak_alpha_for_coverage(&self) -> bool {
        self.props
            .contains(Properties::CAN_TWEAK_ALPHA_FOR_COVERAGE)
    }

    // Port of: src/gpu/BlendFormula.h#L98-L100 (chrome/m156)
    /// The blend equation.
    #[must_use]
    pub const fn equation(&self) -> BlendEquation {
        self.blend_equation
    }

    // Port of: src/gpu/BlendFormula.h#L102-L104 (chrome/m156)
    /// The source coefficient.
    #[must_use]
    pub const fn src_coeff(&self) -> BlendCoeff {
        self.src_coeff
    }

    // Port of: src/gpu/BlendFormula.h#L106-L108 (chrome/m156)
    /// The destination coefficient.
    #[must_use]
    pub const fn dst_coeff(&self) -> BlendCoeff {
        self.dst_coeff
    }

    // Port of: src/gpu/BlendFormula.h#L110-L112 (chrome/m156)
    /// The primary output.
    #[must_use]
    pub const fn primary_output(&self) -> OutputType {
        self.primary_output_type
    }

    // Port of: src/gpu/BlendFormula.h#L114-L116 (chrome/m156)
    /// The secondary output.
    #[must_use]
    pub const fn secondary_output(&self) -> OutputType {
        self.secondary_output_type
    }
}

// Port of: src/gpu/BlendFormula.h#L118-L172 (chrome/m156)
/// Deduces the properties of a [`BlendFormula`].
const fn get_properties(
    primary_out: OutputType,
    secondary_out: OutputType,
    equation: BlendEquation,
    src_coeff: BlendCoeff,
    dst_coeff: BlendCoeff,
) -> Properties {
    // The provided formula should already be optimized before a BlendFormula is constructed.
    // Assert that here while setting up the properties in the constexpr constructor.
    debug_assert!(
        matches!(primary_out, OutputType::None)
            == !blend_coeffs_use_src_color(src_coeff, dst_coeff)
    );
    debug_assert!(!blend_coeff_refs_src2(src_coeff));
    debug_assert!(matches!(secondary_out, OutputType::None) == !blend_coeff_refs_src2(dst_coeff));
    debug_assert!(
        primary_out as u8 != secondary_out as u8 || matches!(primary_out, OutputType::None)
    );
    debug_assert!(
        !matches!(primary_out, OutputType::None) || matches!(secondary_out, OutputType::None)
    );

    let mut props = Properties::empty();
    if blend_modifies_dst(equation, src_coeff, dst_coeff) {
        props = props.union(Properties::MODIFIES_DST);
    }
    if !blend_coeffs_use_dst_color(src_coeff, dst_coeff, false) {
        props = props.union(Properties::UNAFFECTED_BY_DST);
    }
    if !blend_coeffs_use_dst_color(src_coeff, dst_coeff, true) {
        props = props.union(Properties::UNAFFECTED_BY_DST_IF_OPAQUE);
    }
    // We assert later that SrcCoeff doesn't ref src2.
    if (primary_out as u8 >= OutputType::Modulate as u8
        && blend_coeffs_use_src_color(src_coeff, dst_coeff))
        || (secondary_out as u8 >= OutputType::Modulate as u8 && blend_coeff_refs_src2(dst_coeff))
    {
        props = props.union(Properties::USES_INPUT_COLOR);
    }
    if (matches!(primary_out, OutputType::Modulate) || matches!(primary_out, OutputType::None))
        && matches!(secondary_out, OutputType::None)
        && blend_allows_coverage_as_alpha(equation, src_coeff, dst_coeff)
    {
        props = props.union(Properties::CAN_TWEAK_ALPHA_FOR_COVERAGE);
    }
    props
}

// Port of: src/gpu/BlendFormula.cpp#L22-L30 (chrome/m156)
// When the coeffs are (Zero, Zero) or (Zero, One) we set the primary output to none.
const fn make_coeff_formula(src_coeff: BlendCoeff, dst_coeff: BlendCoeff) -> BlendFormula {
    if matches!(src_coeff, BlendCoeff::Zero)
        && (matches!(dst_coeff, BlendCoeff::Zero) || matches!(dst_coeff, BlendCoeff::One))
    {
        BlendFormula::new(
            OutputType::None,
            OutputType::None,
            BlendEquation::Add,
            BlendCoeff::Zero,
            dst_coeff,
        )
    } else {
        BlendFormula::new(
            OutputType::Modulate,
            OutputType::None,
            BlendEquation::Add,
            src_coeff,
            dst_coeff,
        )
    }
}

// Port of: src/gpu/BlendFormula.cpp#L32-L40 (chrome/m156)
const fn make_sa_modulate_formula(src_coeff: BlendCoeff, dst_coeff: BlendCoeff) -> BlendFormula {
    BlendFormula::new(
        OutputType::SAModulate,
        OutputType::None,
        BlendEquation::Add,
        src_coeff,
        dst_coeff,
    )
}

// Port of: src/gpu/BlendFormula.cpp#L42-L60 (chrome/m156)
const fn make_coverage_formula(
    one_minus_dst_coeff_modulate_output: OutputType,
    src_coeff: BlendCoeff,
) -> BlendFormula {
    BlendFormula::new(
        OutputType::Modulate,
        one_minus_dst_coeff_modulate_output,
        BlendEquation::Add,
        src_coeff,
        BlendCoeff::IS2C,
    )
}

// Port of: src/gpu/BlendFormula.cpp#L62-L81 (chrome/m156)
const fn make_coverage_src_coeff_zero_formula(
    one_minus_dst_coeff_modulate_output: OutputType,
) -> BlendFormula {
    BlendFormula::new(
        one_minus_dst_coeff_modulate_output,
        OutputType::None,
        BlendEquation::ReverseSubtract,
        BlendCoeff::DC,
        BlendCoeff::One,
    )
}

// Port of: src/gpu/BlendFormula.cpp#L83-L96 (chrome/m156)
const fn make_coverage_dst_coeff_zero_formula(src_coeff: BlendCoeff) -> BlendFormula {
    BlendFormula::new(
        OutputType::Modulate,
        OutputType::Coverage,
        BlendEquation::Add,
        src_coeff,
        BlendCoeff::IS2A,
    )
}

// Port of: src/gpu/BlendFormula.cpp#L103-L177 (chrome/m156)
// Indexed by [isOpaque][hasCoverage][mode]: the first index is "input color is opaque", the
// second "has coverage", and the innermost one the Porter-Duff mode (up to `Screen`).
const K_BLEND_TABLE: [[[BlendFormula; 15]; 2]; 2] = [
    [
        // No coverage, input color unknown
        [
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::Zero), // clear
            make_coeff_formula(BlendCoeff::One, BlendCoeff::Zero),  // src
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),  // dst
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISA),   // src-over
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),   // dst-over
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::Zero),   // src-in
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::SA),   // dst-in
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::Zero),  // src-out
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::ISA),  // dst-out
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::ISA),    // src-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::SA),    // dst-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::ISA),   // xor
            make_coeff_formula(BlendCoeff::One, BlendCoeff::One),   // plus
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::SC),   // modulate
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISC),   // screen
        ],
        // Has coverage, input color unknown
        [
            make_coverage_src_coeff_zero_formula(OutputType::Coverage), // clear
            make_coverage_dst_coeff_zero_formula(BlendCoeff::One),      // src
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),      // dst
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISA),       // src-over
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),       // dst-over
            make_coverage_dst_coeff_zero_formula(BlendCoeff::DA),       // src-in
            make_coverage_src_coeff_zero_formula(OutputType::ISAModulate), // dst-in
            make_coverage_dst_coeff_zero_formula(BlendCoeff::IDA),      // src-out
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::ISA),      // dst-out
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::ISA),        // src-atop
            make_coverage_formula(OutputType::ISAModulate, BlendCoeff::IDA), // dst-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::ISA),       // xor
            make_coeff_formula(BlendCoeff::One, BlendCoeff::One),       // plus
            make_coverage_src_coeff_zero_formula(OutputType::ISCModulate), // modulate
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISC),       // screen
        ],
    ],
    [
        // No coverage, input color opaque
        [
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::Zero), // clear
            make_coeff_formula(BlendCoeff::One, BlendCoeff::Zero),  // src
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),  // dst
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISA),   // src-over, see comment below
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),   // dst-over
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::Zero),   // src-in
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),  // dst-in
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::Zero),  // src-out
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::Zero), // dst-out
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::Zero),   // src-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),   // dst-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::Zero),  // xor
            make_coeff_formula(BlendCoeff::One, BlendCoeff::One),   // plus
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::SC),   // modulate
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISC),   // screen
        ],
        // Has coverage, input color opaque
        [
            make_coverage_src_coeff_zero_formula(OutputType::Coverage), // clear
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISA),       // src
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),      // dst
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISA),       // src-over
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),       // dst-over
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::ISA),        // src-in
            make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),      // dst-in
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::ISA),       // src-out
            make_coverage_src_coeff_zero_formula(OutputType::Coverage), // dst-out
            make_coeff_formula(BlendCoeff::DA, BlendCoeff::ISA),        // src-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),       // dst-atop
            make_coeff_formula(BlendCoeff::IDA, BlendCoeff::ISA),       // xor
            make_coeff_formula(BlendCoeff::One, BlendCoeff::One),       // plus
            make_coverage_src_coeff_zero_formula(OutputType::ISCModulate), // modulate
            make_coeff_formula(BlendCoeff::One, BlendCoeff::ISC),       // screen
        ],
    ],
];

// Port of: src/gpu/BlendFormula.cpp#L184-L212 (chrome/m156)
// Indexed by mode (up to `Screen`, `kLastCoeffMode`).
const K_LCD_BLEND_TABLE: [BlendFormula; 15] = [
    make_coverage_src_coeff_zero_formula(OutputType::Coverage), // clear
    make_coverage_formula(OutputType::Coverage, BlendCoeff::One), // src
    make_coeff_formula(BlendCoeff::Zero, BlendCoeff::One),      // dst
    make_coverage_formula(OutputType::SAModulate, BlendCoeff::One), // src-over
    make_coeff_formula(BlendCoeff::IDA, BlendCoeff::One),       // dst-over
    make_coverage_formula(OutputType::Coverage, BlendCoeff::DA), // src-in
    make_coverage_src_coeff_zero_formula(OutputType::ISAModulate), // dst-in
    make_coverage_formula(OutputType::Coverage, BlendCoeff::IDA), // src-out
    make_sa_modulate_formula(BlendCoeff::Zero, BlendCoeff::ISC), // dst-out
    make_coverage_formula(OutputType::SAModulate, BlendCoeff::DA), // src-atop
    make_coverage_formula(OutputType::ISAModulate, BlendCoeff::IDA), // dst-atop
    make_coverage_formula(OutputType::SAModulate, BlendCoeff::IDA), // xor
    make_coeff_formula(BlendCoeff::One, BlendCoeff::One),       // plus
    make_coverage_src_coeff_zero_formula(OutputType::ISCModulate), // modulate
    make_coeff_formula(BlendCoeff::One, BlendCoeff::ISC),       // screen
];

// Port of: src/gpu/BlendFormula.cpp#L218-L221 (chrome/m156)
/// `GetBlendFormula(isOpaque, hasCoverage, xfermode)`: the formula for a Porter-Duff mode.
#[must_use]
pub fn get_blend_formula(is_opaque: bool, has_coverage: bool, xfermode: BlendMode) -> BlendFormula {
    debug_assert!(xfermode as i32 <= BlendMode::LAST_COEFF_MODE as i32);
    K_BLEND_TABLE[usize::from(is_opaque)][usize::from(has_coverage)][xfermode as usize]
}

// Port of: src/gpu/BlendFormula.cpp#L223-L226 (chrome/m156)
/// `GetLCDBlendFormula(xfermode)`: the formula for LCD text, where coverage is per channel.
#[must_use]
pub fn get_lcd_blend_formula(xfermode: BlendMode) -> BlendFormula {
    debug_assert!(xfermode as i32 <= BlendMode::LAST_COEFF_MODE as i32);
    K_LCD_BLEND_TABLE[xfermode as usize]
}
