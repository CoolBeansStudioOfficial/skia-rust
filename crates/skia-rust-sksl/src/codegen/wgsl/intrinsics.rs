// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (the intrinsic assemblers and the
// polyfill generators).

//! Intrinsic calls, and the polyfilled functions WGSL lacks (`inverse`, `outerProduct`, `mod`).

use super::expressions::{CallInfo, binary_op_is_ambiguous_in_wgsl};
use super::types::{
    SAMPLER_SUFFIX, TEXTURE_SUFFIX, operator_name, to_wgsl_type, to_wgsl_type_simple,
    type_is_low_precision,
};
use super::{AssembleMode, WgslCodeGenerator, WrittenPolyfills};
use crate::constant_folder::get_constant_value_or_null;
use crate::intrinsic_list::IntrinsicKind;
use crate::ir::{ExprId, ModifierFlags, TypeId, TypeKind};
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::skstd::to_string_f32;

type Precedence = OperatorPrecedence;

/// `kSharpenTexturesBias` (`SkSLCodeGenerator.h`).
const SHARPEN_TEXTURES_BIAS: f32 = -0.475;

// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3578-L3591 (chrome/m156)
fn generate_inverse_2x2(scalar_name: &str, suffix: &str) -> String {
    format!(
        "fn inverse_2x2{suffix}(m: mat2x2<{scalar_name}>) -> mat2x2<{scalar_name}> {{\n\
         return mat2x2<{scalar_name}>(m[1].y, -m[0].y, -m[1].x, m[0].x) * (1/determinant(m));\n\
         }}\n"
    )
}

// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3593-L3610 (chrome/m156)
fn generate_inverse_3x3(scalar_name: &str, suffix: &str) -> String {
    format!(
        "fn inverse_3x3{suffix}(m: mat3x3<{scalar_name}>) -> mat3x3<{scalar_name}> {{\n\
         let a00 = m[0].x; let a01 = m[0].y; let a02 = m[0].z;\n\
         let a10 = m[1].x; let a11 = m[1].y; let a12 = m[1].z;\n\
         let a20 = m[2].x; let a21 = m[2].y; let a22 = m[2].z;\n\
         let b01 =  a22*a11 - a12*a21;\n\
         let b11 = -a22*a10 + a12*a20;\n\
         let b21 =  a21*a10 - a11*a20;\n\
         let det = a00*b01 + a01*b11 + a02*b21;\n\
         return mat3x3<{scalar_name}>(b01, (-a22*a01 + a02*a21), ( a12*a01 - a02*a11),\n\
         b11, ( a22*a00 - a02*a20), (-a12*a00 + a02*a10),\n\
         b21, (-a21*a00 + a01*a20), ( a11*a00 - a01*a10)) * (1/det);\n\
         }}\n"
    )
}

// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3612-L3650 (chrome/m156)
fn generate_inverse_4x4(scalar_name: &str, suffix: &str) -> String {
    format!(
        "fn inverse_4x4{suffix}(m: mat4x4<{scalar_name}>) -> mat4x4<{scalar_name}>{{\n\
         let a00 = m[0].x; let a01 = m[0].y; let a02 = m[0].z; let a03 = m[0].w;\n\
         let a10 = m[1].x; let a11 = m[1].y; let a12 = m[1].z; let a13 = m[1].w;\n\
         let a20 = m[2].x; let a21 = m[2].y; let a22 = m[2].z; let a23 = m[2].w;\n\
         let a30 = m[3].x; let a31 = m[3].y; let a32 = m[3].z; let a33 = m[3].w;\n\
         let b00 = a00*a11 - a01*a10;\n\
         let b01 = a00*a12 - a02*a10;\n\
         let b02 = a00*a13 - a03*a10;\n\
         let b03 = a01*a12 - a02*a11;\n\
         let b04 = a01*a13 - a03*a11;\n\
         let b05 = a02*a13 - a03*a12;\n\
         let b06 = a20*a31 - a21*a30;\n\
         let b07 = a20*a32 - a22*a30;\n\
         let b08 = a20*a33 - a23*a30;\n\
         let b09 = a21*a32 - a22*a31;\n\
         let b10 = a21*a33 - a23*a31;\n\
         let b11 = a22*a33 - a23*a32;\n\
         let det = b00*b11 - b01*b10 + b02*b09 + b03*b08 - b04*b07 + b05*b06;\n\
         return mat4x4<{scalar_name}>(a11*b11 - a12*b10 + a13*b09,\n\
         a02*b10 - a01*b11 - a03*b09,\n\
         a31*b05 - a32*b04 + a33*b03,\n\
         a22*b04 - a21*b05 - a23*b03,\n\
         a12*b08 - a10*b11 - a13*b07,\n\
         a00*b11 - a02*b08 + a03*b07,\n\
         a32*b02 - a30*b05 - a33*b01,\n\
         a20*b05 - a22*b02 + a23*b01,\n\
         a10*b10 - a11*b08 + a13*b06,\n\
         a01*b08 - a00*b10 - a03*b06,\n\
         a30*b04 - a31*b02 + a33*b00,\n\
         a21*b02 - a20*b04 - a23*b00,\n\
         a11*b07 - a10*b09 - a12*b06,\n\
         a00*b09 - a01*b07 + a02*b06,\n\
         a31*b01 - a30*b03 - a32*b00,\n\
         a20*b03 - a21*b01 + a22*b00) * (1/det);\n\
         }}\n"
    )
}

/// `gen_outer_product_fn`: generates `outer_product_$Cx$R(vec$R<f32}>, vec$C<f32>) ->
/// mat$Cx$R<f32>` or its f16 variant.
// Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3652-L3669 (chrome/m156)
fn gen_outer_product_fn(scalar_name: &str, suffix: &str, c: usize, r: usize) -> String {
    format!(
        "fn outer_product_{c}x{r}{suffix}(a: vec{r}<{scalar_name}>, b: vec{c}<{scalar_name}>) -> mat{c}x{r}<{scalar_name}> {{\n\
         var m : mat{c}x{r}<{scalar_name}>;\n\
         for (var c = 0; c < {c}; c++) {{ m[c] = a * b[c]; }}\n\
         return m;\n\
         }}\n"
    )
}

impl WgslCodeGenerator<'_> {
    /// `assembleSimpleIntrinsic(intrinsicName, call, wgslOnlySupportsHighPrecision)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3072-L3115 (chrome/m156)
    fn assemble_simple_intrinsic(
        &mut self,
        intrinsic_name: &str,
        call: &CallInfo,
        wgsl_only_supports_high_precision: bool,
    ) -> String {
        // Invoke the function, passing each function argument.
        let mut expr = intrinsic_name.to_owned();
        expr.push('(');
        let mut separator = crate::string::Separator::new();
        for &arg in &call.args {
            expr += separator.next_str();

            let argument = self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
            let arg_type = self.expr_type(arg);
            if wgsl_only_supports_high_precision && type_is_low_precision(self.ctx, arg_type) {
                // Add a cast to high precision
                debug_assert!(!self.ctx.pool.ty(arg_type).is_atomic());
                let high_p_type = to_wgsl_type(self.ctx, arg_type, None, true);
                expr += &format!("{high_p_type}({argument})");
            } else if self.ctx.pool.ty(arg_type).is_atomic() {
                // WGSL passes atomic values to intrinsics as pointers.
                expr.push('&');
                expr += &argument;
            } else {
                expr += &argument;
            }
        }
        expr.push(')');

        // Wrap the call expression in a cast back to its low precision type if necessary.
        if wgsl_only_supports_high_precision && type_is_low_precision(self.ctx, call.ty) {
            let low_p_type = to_wgsl_type_simple(self.ctx, call.ty);
            expr = format!("{low_p_type}({expr})");
        }

        expr
    }

    /// `assembleVectorizedIntrinsic(intrinsicName, call)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3117-L3145 (chrome/m156)
    fn assemble_vectorized_intrinsic(&mut self, intrinsic_name: &str, call: &CallInfo) -> String {
        debug_assert!(!self.ctx.pool.ty(call.ty).is_void());

        // Invoke the function, passing each function argument.
        let mut expr = intrinsic_name.to_owned();
        expr.push('(');

        let mut separator = crate::string::Separator::new();
        let returns_vector = self.ctx.pool.ty(call.ty).is_vector();
        for &arg in &call.args {
            expr += separator.next_str();

            let arg_type = self.expr_type(arg);
            let vectorize = returns_vector && self.ctx.pool.ty(arg_type).is_scalar();
            if vectorize {
                expr += &to_wgsl_type_simple(self.ctx, call.ty);
                expr.push('(');
            }

            expr += &self.assemble_expression(arg, Precedence::Sequence, AssembleMode::Auto);
            if vectorize {
                expr.push(')');
            }
        }
        expr.push(')');

        expr
    }

    /// `assembleUnaryOpIntrinsic(op, call, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3147-L3167 (chrome/m156)
    fn assemble_unary_op_intrinsic(
        &mut self,
        op: Operator,
        call: &CallInfo,
        parent_precedence: Precedence,
    ) -> String {
        debug_assert!(!self.ctx.pool.ty(call.ty).is_void());

        let need_parens = Precedence::Prefix >= parent_precedence;

        let mut expr = String::new();
        if need_parens {
            expr.push('(');
        }

        expr += operator_name(op);
        expr += &self.assemble_expression(call.args[0], Precedence::Prefix, AssembleMode::Auto);

        if need_parens {
            expr.push(')');
        }

        expr
    }

    /// `assembleBinaryOpIntrinsic(op, call, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3169-L3192 (chrome/m156)
    fn assemble_binary_op_intrinsic(
        &mut self,
        op: Operator,
        call: &CallInfo,
        parent_precedence: Precedence,
    ) -> String {
        debug_assert!(!self.ctx.pool.ty(call.ty).is_void());

        let precedence = op.binary_precedence();
        let need_parens = precedence >= parent_precedence || binary_op_is_ambiguous_in_wgsl(op);
        let mut expr = String::new();
        if need_parens {
            expr.push('(');
        }

        expr += &self.assemble_expression(call.args[0], precedence, AssembleMode::Auto);
        expr += operator_name(op);
        expr += &self.assemble_expression(call.args[1], precedence, AssembleMode::Auto);

        if need_parens {
            expr.push(')');
        }

        expr
    }

    /// `assembleOutAssignedIntrinsic(intrinsicName, returnField, outField, call)`: rewrites a
    /// WGSL intrinsic of the form "intrinsicName(in) -> struct" to the `SkSL`'s
    /// "intrinsicName(in, outField) -> returnField", where outField and returnField are the names
    /// of the fields in the struct returned by the WGSL intrinsic.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3194-L3226 (chrome/m156)
    fn assemble_out_assigned_intrinsic(
        &mut self,
        intrinsic_name: &str,
        return_field: &str,
        out_field: &str,
        call: &CallInfo,
    ) -> String {
        debug_assert!(self.ctx.pool.ty(call.ty).component_type().is_number());
        debug_assert!(call.args.len() == 2);
        debug_assert!({
            let params = &self.ctx.pool.function(call.function).parameters;
            self.ctx
                .pool
                .variable(params[1])
                .modifier_flags
                .contains(ModifierFlags::OUT)
        });

        // Invoke the intrinsic with the first parameter.
        let mut expr = intrinsic_name.to_owned();
        expr += "(";
        expr += &self.assemble_expression(call.args[0], Precedence::Sequence, AssembleMode::Auto);
        expr += ")";
        // In WGSL the intrinsic returns a struct; assign it to a local so that its fields can be
        // accessed multiple times.
        expr = self.write_scratch_let(&expr, false);
        expr += ".";

        // Store the outField of `expr` to the intended "out" argument
        let Some(lvalue) = self.make_lvalue(call.args[1]) else {
            return String::new();
        };
        let mut out_value = expr.clone();
        out_value += out_field;
        let store = lvalue.store(self.ctx, &out_value);
        self.write_line(&store);

        // And return the expression accessing the returnField.
        expr += return_field;
        expr
    }

    /// `assemblePartialSampleCall(functionName, type, sampler, coords)`: returns the beginning of
    /// the expression and the closing parentheses string to append once any additional arguments
    /// are added.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3228-L3293 (chrome/m156)
    fn assemble_partial_sample_call(
        &mut self,
        function_name: &str,
        ty: TypeId,
        sampler: ExprId,
        coords: ExprId,
    ) -> (String, &'static str) {
        // This function returns `functionName(inSampler_texture, inSampler_sampler, coords`
        // without a terminating comma or close-parenthesis. This allows the caller to add more
        // arguments as needed.
        let mut expr = format!("{function_name}(");
        let sampler_kind = self.ctx.pool.ty(self.expr_type(sampler)).type_kind;
        if sampler_kind == TypeKind::Sampler {
            // Split combined texture+sampler into two args with suffixes.
            expr += &self.assemble_expression(sampler, Precedence::Sequence, AssembleMode::Auto);
            expr += TEXTURE_SUFFIX;
            expr += ", ";
            expr += &self.assemble_expression(sampler, Precedence::Sequence, AssembleMode::Auto);
            expr += SAMPLER_SUFFIX;
            expr += ", ";
        } else {
            debug_assert!(sampler_kind == TypeKind::Texture);
            // Pass through the texture expression without any extra suffix
            expr += &self.assemble_expression(sampler, Precedence::Sequence, AssembleMode::Auto);
            expr += ", ";
        }

        // Compute the sample coordinates, dividing out the Z if a vec3 was provided.
        let coords_columns = {
            let t = self.ctx.pool.ty(self.expr_type(coords));
            debug_assert!(t.is_vector());
            t.columns()
        };
        if coords_columns == 3 {
            // The coordinates were passed as a vec3, so we need to emit `coords.xy / coords.z`.
            // This is a binary op applied to `coords` twice, so explicitly reapply the const eval
            // workaround with just it in mind.
            let coords_const = get_constant_value_or_null(&self.ctx.pool, coords).is_some();
            let vec3_coords = self.assemble_expression(
                coords,
                Precedence::Multiplicative,
                if coords_const {
                    AssembleMode::ForceLet
                } else {
                    AssembleMode::UsedMultipleTimes
                },
            );
            expr += &format!("{vec3_coords}.xy / {vec3_coords}.z");
        } else {
            // The coordinates should be a plain vec2; emit the expression as-is.
            debug_assert!(coords_columns == 2);
            expr += &self.assemble_expression(coords, Precedence::Sequence, AssembleMode::Auto);
        }

        // WGSL does not support f16 textures while all of SkSL's sample functions return
        // "half4", so add a cast back to f16 if necessary.
        if type_is_low_precision(self.ctx, ty) {
            let low_p_type = to_wgsl_type_simple(self.ctx, ty);
            (format!("{low_p_type}({expr}"), "))")
        } else {
            (expr, ")")
        }
    }

    /// `assembleIntrinsicCall(call, kind, parentPrecedence)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3254-L3586 (chrome/m156)
    #[allow(clippy::too_many_lines)] // One arm per intrinsic, as the C++ switch.
    pub(super) fn assemble_intrinsic_call(
        &mut self,
        call: &CallInfo,
        kind: IntrinsicKind,
        parent_precedence: Precedence,
    ) -> String {
        // Be careful: WGSL 1.0 will reject any intrinsic calls which can be constant-evaluated to
        // infinity or nan with a compile error. If all arguments to an intrinsic are compile-time
        // constants (`all_arguments_constant`), it is safest to copy one argument into a
        // scratch-let so that the call will be seen as runtime-evaluated, which defuses the
        // overflow checks. Don't worry; a competent driver should still optimize it away. This
        // was automatically handled in appendExpression()'s AutoConstEvalWorkaround before this
        // was reached.

        let arguments = &call.args;
        match kind {
            IntrinsicKind::WorkgroupUniformLoad => {
                // WGSL 17.11.4: workgroupUniformLoad(p : ptr<workgroup, T>) -> T
                // The argument must be a pointer-addressable reference expression (variable
                // reference, struct field, or array element) rooted in a variable declared in the
                // 'workgroup' address space. Vector components (swizzles or indexed vector
                // elements) and rvalues are not addressable.
                let argument = self.assemble_expression(
                    arguments[0],
                    Precedence::Sequence,
                    AssembleMode::Auto,
                );
                format!("workgroupUniformLoad(&{argument})")
            }

            IntrinsicKind::Atan => {
                let name = if arguments.len() == 1 {
                    "atan"
                } else {
                    "atan2"
                };
                self.assemble_simple_intrinsic(name, call, false)
            }

            // Derivative functions in WGSL are only defined for full precision types
            IntrinsicKind::DFdx => self.assemble_simple_intrinsic("dpdx", call, true),

            IntrinsicKind::DFdy => {
                // WSGL is only generated for Graphite, which does not use RTFlip so there's no
                // need to flip the Y derivatives either.
                debug_assert!(self.ctx.config().settings.force_no_rt_flip);
                self.assemble_simple_intrinsic("dpdy", call, true)
            }

            IntrinsicKind::Dot => {
                let arg_type = self.expr_type(arguments[0]);
                if self.ctx.pool.ty(arg_type).is_scalar() {
                    return self.assemble_binary_op_intrinsic(
                        Operator::from(OperatorKind::Star),
                        call,
                        parent_precedence,
                    );
                }
                self.assemble_simple_intrinsic("dot", call, false)
            }
            IntrinsicKind::Equal => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::EqEq),
                call,
                parent_precedence,
            ),

            IntrinsicKind::Faceforward => {
                let arg_type = self.expr_type(arguments[0]);
                if self.ctx.pool.ty(arg_type).is_scalar() {
                    // select(-N, N, (I * Nref) < 0)
                    self.needs_const_eval_workaround = false; // Disable for N, we'll handle it in I*Nref
                    let n = self.assemble_expression(
                        arguments[0],
                        Precedence::Prefix,
                        AssembleMode::UsedMultipleTimes,
                    );
                    let i_nref_const = get_constant_value_or_null(&self.ctx.pool, arguments[1])
                        .is_some()
                        && get_constant_value_or_null(&self.ctx.pool, arguments[2]).is_some();
                    let i = self.assemble_expression(
                        arguments[1],
                        Precedence::Multiplicative,
                        if i_nref_const {
                            AssembleMode::ForceLet
                        } else {
                            AssembleMode::Auto
                        },
                    );
                    let nref = self.assemble_expression(
                        arguments[2],
                        Precedence::Multiplicative,
                        AssembleMode::Auto,
                    );
                    return format!("select(-{n}, {n}, {i} * {nref} < 0)");
                }
                self.assemble_simple_intrinsic("faceForward", call, false)
            }
            // SkSL frexp is "$genType fract = frexp($genType, out $genIType exp)" whereas WGSL
            // returns a struct with no out param: "let [fract, exp] = frexp($genType)".
            IntrinsicKind::Frexp => {
                self.assemble_out_assigned_intrinsic("frexp", "fract", "exp", call)
            }

            IntrinsicKind::GreaterThan => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::Gt),
                call,
                parent_precedence,
            ),

            IntrinsicKind::GreaterThanEqual => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::GtEq),
                call,
                parent_precedence,
            ),

            IntrinsicKind::Inverse => self.assemble_inverse_polyfill(call),

            IntrinsicKind::OuterProduct => self.assemble_outer_product_polyfill(call),

            IntrinsicKind::Inversesqrt => {
                self.assemble_simple_intrinsic("inverseSqrt", call, false)
            }

            IntrinsicKind::LessThan => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::Lt),
                call,
                parent_precedence,
            ),

            IntrinsicKind::LessThanEqual => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::LtEq),
                call,
                parent_precedence,
            ),

            IntrinsicKind::MatrixCompMult => {
                let arg0 = self.assemble_expression(
                    arguments[0],
                    Precedence::Postfix,
                    AssembleMode::UsedMultipleTimes,
                );
                let arg1 = self.assemble_expression(
                    arguments[1],
                    Precedence::Postfix,
                    AssembleMode::UsedMultipleTimes,
                );
                let (t0, t1) = (self.expr_type(arguments[0]), self.expr_type(arguments[1]));
                self.assemble_componentwise_matrix_binary(
                    t0,
                    t1,
                    &arg0,
                    &arg1,
                    Operator::from(OperatorKind::Star),
                )
            }
            IntrinsicKind::Mix => {
                let arg_type = self.expr_type(arguments[2]);
                let name = if self.ctx.pool.ty(arg_type).component_type().is_boolean() {
                    "select"
                } else {
                    "mix"
                };
                self.assemble_vectorized_intrinsic(name, call)
            }
            IntrinsicKind::Mod => {
                // WGSL has no intrinsic equivalent to `mod`. Synthesize `x - y * floor(x / y)`.
                // NOTE: While we are introducing a new binary op between two possible constant
                // expressions, it is consistent with the const-ness already handled by
                // AutoConstEvalWorkaround.
                let arg0 = self.assemble_expression(
                    arguments[0],
                    Precedence::Additive,
                    AssembleMode::UsedMultipleTimes,
                );
                let arg1 = self.assemble_expression(
                    arguments[1],
                    Precedence::Additive,
                    AssembleMode::UsedMultipleTimes,
                );
                format!("({arg0} - {arg1} * floor({arg0} / {arg1}))")
            }

            // SkSL modf is "$genType fract = modf($genType, out $genType whole)" whereas WGSL
            // returns a struct with no out param: "let [fract, whole] = modf($genType)".
            IntrinsicKind::Modf => {
                self.assemble_out_assigned_intrinsic("modf", "fract", "whole", call)
            }

            IntrinsicKind::Normalize => {
                let arg_type = self.expr_type(arguments[0]);
                let name = if self.ctx.pool.ty(arg_type).is_scalar() {
                    "sign"
                } else {
                    "normalize"
                };
                self.assemble_simple_intrinsic(name, call, false)
            }
            IntrinsicKind::Not => self.assemble_unary_op_intrinsic(
                Operator::from(OperatorKind::LogicalNot),
                call,
                parent_precedence,
            ),

            IntrinsicKind::NotEqual => self.assemble_binary_op_intrinsic(
                Operator::from(OperatorKind::Neq),
                call,
                parent_precedence,
            ),

            IntrinsicKind::PackHalf2x16 => {
                self.assemble_simple_intrinsic("pack2x16float", call, false)
            }

            IntrinsicKind::PackSnorm2x16 => {
                self.assemble_simple_intrinsic("pack2x16snorm", call, false)
            }

            IntrinsicKind::PackSnorm4x8 => {
                self.assemble_simple_intrinsic("pack4x8snorm", call, false)
            }

            IntrinsicKind::PackUnorm2x16 => {
                self.assemble_simple_intrinsic("pack2x16unorm", call, false)
            }

            IntrinsicKind::PackUnorm4x8 => {
                self.assemble_simple_intrinsic("pack4x8unorm", call, false)
            }

            IntrinsicKind::FindLsb => {
                // firstTrailingBit (and firstLeadingBit) return a type matching their input, but
                // findLSB and findMSB in SkSL return signed types. Add a cast if needed.
                let lsb = self.assemble_simple_intrinsic("firstTrailingBit", call, false);
                self.cast_to_signed_if_needed(arguments[0], call.ty, &lsb)
            }

            IntrinsicKind::FindMsb => {
                let msb = self.assemble_simple_intrinsic("firstLeadingBit", call, false);
                self.cast_to_signed_if_needed(arguments[0], call.ty, &msb)
            }

            IntrinsicKind::BitCount => {
                // countOneBits returns a type matching its input, but bitCount in SkSL returns a
                // signed type. Add a cast if needed.
                let bit_count = self.assemble_simple_intrinsic("countOneBits", call, false);
                self.cast_to_signed_if_needed(arguments[0], call.ty, &bit_count)
            }

            IntrinsicKind::FloatBitsToInt
            | IntrinsicKind::FloatBitsToUint
            | IntrinsicKind::IntBitsToFloat
            | IntrinsicKind::UintBitsToFloat => {
                let output_type = to_wgsl_type_simple(self.ctx, call.ty);
                self.assemble_simple_intrinsic(&format!("bitcast<{output_type}>"), call, false)
            }

            IntrinsicKind::RoundEven | IntrinsicKind::Round => {
                // WGSL has no built-in roundEven(), but its round() is defined as:
                //     "When e lies halfway between integers k and k + 1, the result is k when k is
                //      even, and k + 1 when k is odd."
                // This is equivalent to GLSL's roundEven(). GLSL's round() is allowed to just
                // match roundEven().
                self.assemble_simple_intrinsic("round", call, false)
            }

            IntrinsicKind::Reflect => {
                let arg_type = self.expr_type(arguments[0]);
                if self.ctx.pool.ty(arg_type).is_scalar() {
                    // I - 2 * N * I * N
                    // Manually override the auto const-eval workaround since both I and N are used
                    // multiple times in new binary expressions.
                    self.needs_const_eval_workaround = false;
                    let i = self.assemble_expression(
                        arguments[0],
                        Precedence::Multiplicative,
                        AssembleMode::UsedMultipleTimes,
                    );
                    // We force N into a let when it is constant, ensuring (2*N), (N*I), and (I*N)
                    // avoid issues with const eval compilation failures. Technically if we were
                    // already lifting I to a let, we could skip lifting N if we knew (2*N) would
                    // not overflow, but this keeps logic simpler.
                    let mode = if get_constant_value_or_null(&self.ctx.pool, arguments[1]).is_some()
                    {
                        AssembleMode::ForceLet
                    } else {
                        AssembleMode::UsedMultipleTimes
                    };
                    let n =
                        self.assemble_expression(arguments[1], Precedence::Multiplicative, mode);
                    return format!("({i} - 2 * {n} * {i} * {n})");
                }
                self.assemble_simple_intrinsic("reflect", call, false)
            }

            IntrinsicKind::Refract => {
                let arg_type = self.expr_type(arguments[0]);
                if self.ctx.pool.ty(arg_type).is_scalar() {
                    // WGSL only implements refract for vectors; rather than reimplementing
                    // refract from scratch, we can replace the call with
                    // `refract(float2(I,0), float2(N,0), eta).x`.
                    let i = self.assemble_expression(
                        arguments[0],
                        Precedence::Sequence,
                        AssembleMode::Auto,
                    );
                    let n = self.assemble_expression(
                        arguments[1],
                        Precedence::Sequence,
                        AssembleMode::Auto,
                    );
                    let eta = self.assemble_expression(
                        arguments[2],
                        Precedence::Sequence,
                        AssembleMode::Auto,
                    );
                    let n_type = self.expr_type(arguments[1]);
                    return format!(
                        "refract(vec2<{}>({i}, 0), vec2<{}>({n}, 0), {eta}).x",
                        to_wgsl_type_simple(self.ctx, arg_type),
                        to_wgsl_type_simple(self.ctx, n_type),
                    );
                }
                self.assemble_simple_intrinsic("refract", call, false)
            }

            IntrinsicKind::Sample => {
                // Determine if a bias argument was passed in.
                debug_assert!(arguments.len() == 2 || arguments.len() == 3);
                let call_includes_bias = arguments.len() == 3;
                let sharpen_textures = self.ctx.config().settings.sharpen_textures;
                let (mut expr, close);
                if sharpen_textures || call_includes_bias {
                    // We need to supply a bias argument; this is a separate intrinsic in WGSL.
                    (expr, close) = self.assemble_partial_sample_call(
                        "textureSampleBias",
                        call.ty,
                        arguments[0],
                        arguments[1],
                    );
                    expr += ", ";
                    if call_includes_bias {
                        expr += &self.assemble_expression(
                            arguments[2],
                            Precedence::Additive,
                            AssembleMode::Auto,
                        );
                        expr += " + ";
                    }
                    expr += &to_string_f32(if sharpen_textures {
                        SHARPEN_TEXTURES_BIAS
                    } else {
                        0.0
                    });
                } else {
                    // No bias is necessary, so we can call `textureSample` directly.
                    (expr, close) = self.assemble_partial_sample_call(
                        "textureSample",
                        call.ty,
                        arguments[0],
                        arguments[1],
                    );
                }

                expr + close
            }
            IntrinsicKind::SampleLod => {
                let (mut expr, close) = self.assemble_partial_sample_call(
                    "textureSampleLevel",
                    call.ty,
                    arguments[0],
                    arguments[1],
                );
                expr += ", ";
                expr += &self.assemble_expression(
                    arguments[2],
                    Precedence::Sequence,
                    AssembleMode::Auto,
                );

                expr + close
            }
            IntrinsicKind::SampleGrad => {
                let (mut expr, close) = self.assemble_partial_sample_call(
                    "textureSampleGrad",
                    call.ty,
                    arguments[0],
                    arguments[1],
                );

                expr += ", ";
                expr += &self.assemble_expression(
                    arguments[2],
                    Precedence::Sequence,
                    AssembleMode::Auto,
                );
                expr += ", ";
                expr += &self.assemble_expression(
                    arguments[3],
                    Precedence::Sequence,
                    AssembleMode::Auto,
                );
                expr + close
            }

            IntrinsicKind::TextureRead => {
                // textureLoad takes a texture and coordinates. It does NOT take a sampler.
                // If we are passed a combined sampler, we must extract just the texture.
                let mut expr = String::from("textureLoad(");

                expr += &self.assemble_texture_from_image_or_sampler(arguments[0]);

                expr += ", ";
                expr += &self.assemble_expression(
                    arguments[1],
                    Precedence::Sequence,
                    AssembleMode::Auto,
                );

                // We need to inject an extra argument for the mip-level. We don't plan on using
                // mipmaps in our storage textures, so we can just pass zero.
                expr += ", 0)";

                // WGSL does not support f16 textures while all of SkSL's sample functions return
                // "half4", so add a cast back to f16 if necessary.
                if type_is_low_precision(self.ctx, call.ty) {
                    let low_p_type = to_wgsl_type_simple(self.ctx, call.ty);
                    return format!("{low_p_type}({expr})");
                }

                expr
            }
            IntrinsicKind::TextureSize
            | IntrinsicKind::TextureWidth
            | IntrinsicKind::TextureHeight => {
                let mut expr = String::from("textureDimensions(");

                // Use our helper to safely extract the texture
                expr += &self.assemble_texture_from_image_or_sampler(arguments[0]);

                expr += ")";
                if kind == IntrinsicKind::TextureWidth {
                    expr += ".x";
                } else if kind == IntrinsicKind::TextureHeight {
                    expr += ".y";
                }
                expr
            }
            IntrinsicKind::TextureWrite => {
                self.assemble_simple_intrinsic("textureStore", call, true)
            }

            IntrinsicKind::UnpackHalf2x16 => {
                self.assemble_simple_intrinsic("unpack2x16float", call, false)
            }

            IntrinsicKind::UnpackSnorm2x16 => {
                self.assemble_simple_intrinsic("unpack2x16snorm", call, false)
            }

            IntrinsicKind::UnpackSnorm4x8 => {
                self.assemble_simple_intrinsic("unpack4x8snorm", call, false)
            }

            IntrinsicKind::UnpackUnorm2x16 => {
                self.assemble_simple_intrinsic("unpack2x16unorm", call, false)
            }

            IntrinsicKind::UnpackUnorm4x8 => {
                self.assemble_simple_intrinsic("unpack4x8unorm", call, false)
            }

            IntrinsicKind::Clamp
            | IntrinsicKind::Max
            | IntrinsicKind::Min
            | IntrinsicKind::Smoothstep
            | IntrinsicKind::Step => {
                let name = self.ctx.pool.function(call.function).name.clone();
                self.assemble_vectorized_intrinsic(&name, call)
            }

            // abs, acos, all, any, asin, atomic*, ceil, cos, cross, degrees, distance, exp, exp2,
            // floor, fract, length, log, log2, radians, pow, saturate, sign, sin, sqrt,
            // storageBarrier, tan, workgroupBarrier, and everything else.
            _ => {
                let name = self.ctx.pool.function(call.function).name.clone();
                self.assemble_simple_intrinsic(&name, call, false)
            }
        }
    }

    /// Casts `expr` to the call's (signed) type unless the first argument's component type is
    /// already signed.
    fn cast_to_signed_if_needed(&self, argument: ExprId, call_type: TypeId, expr: &str) -> String {
        let arg_type = self.expr_type(argument);
        if self.ctx.pool.ty(arg_type).component_type().is_signed() {
            expr.to_owned()
        } else {
            format!("{}({expr})", to_wgsl_type_simple(self.ctx, call_type))
        }
    }

    /// `assembleInversePolyfill(call)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3671-L3703 (chrome/m156)
    fn assemble_inverse_polyfill(&mut self, call: &CallInfo) -> String {
        let arguments = &call.args;
        let ty = self.expr_type(arguments[0]);

        // The `inverse` intrinsic should only accept a single-argument square matrix.
        // Once we implement f16 support, these polyfills will need to be updated to support
        // `hmat`; for the time being, all floats in WGSL are f32, so we don't need to worry about
        // precision.
        debug_assert!(arguments.len() == 1);
        let (is_matrix, rows, columns) = {
            let t = self.ctx.pool.ty(ty);
            (
                t.is_matrix(),
                if t.is_matrix() { t.rows() } else { 0 },
                if t.is_matrix() { t.columns() } else { 0 },
            )
        };
        debug_assert!(is_matrix);
        debug_assert!(rows == columns);

        let use_f16 = type_is_low_precision(self.ctx, ty);
        let polyfill: &mut WrittenPolyfills = if use_f16 {
            &mut self.f16_polyfills
        } else {
            &mut self.f32_polyfills
        };
        let type_name = if use_f16 { "f16" } else { "f32" };
        let suffix = if use_f16 { "h" } else { "f" };

        let idx = usize::try_from(rows - 2).expect("a matrix has at least two rows");
        if !polyfill.inverse[idx] {
            const TEMPLATES: [fn(&str, &str) -> String; 3] = [
                generate_inverse_2x2,
                generate_inverse_3x3,
                generate_inverse_4x4,
            ];
            polyfill.inverse[idx] = true;
            let text = TEMPLATES[idx](type_name, suffix);
            self.header.push_str(&text);
        }
        self.assemble_simple_intrinsic(&format!("inverse_{rows}x{columns}{suffix}"), call, false)
    }

    /// `assembleOuterProductPolyfill(call)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L3705-L3736 (chrome/m156)
    fn assemble_outer_product_polyfill(&mut self, call: &CallInfo) -> String {
        // The outer product should take two vector arguments, with the first type's component
        // count defining R and the second type's component count defining C.
        let arguments = &call.args;
        debug_assert!(arguments.len() == 2);
        let (t0, t1) = (self.expr_type(arguments[0]), self.expr_type(arguments[1]));
        debug_assert!(self.ctx.pool.ty(t0).is_vector() && self.ctx.pool.ty(t1).is_vector());
        debug_assert!(self.ctx.pool.ty(call.ty).is_matrix());

        let r = self.ctx.pool.ty(t0).columns();
        let c = self.ctx.pool.ty(t1).columns();
        debug_assert!(
            r == self.ctx.pool.ty(call.ty).rows() && c == self.ctx.pool.ty(call.ty).columns()
        );

        let use_f16 = type_is_low_precision(self.ctx, call.ty);
        let polyfill: &mut WrittenPolyfills = if use_f16 {
            &mut self.f16_polyfills
        } else {
            &mut self.f32_polyfills
        };
        let type_name = if use_f16 { "f16" } else { "f32" };
        let suffix = if use_f16 { "h" } else { "f" };

        let c_idx = usize::try_from(c - 2).expect("a vector has at least two components");
        let r_idx = usize::try_from(r - 2).expect("a vector has at least two components");
        if !polyfill.outer_product[c_idx][r_idx] {
            polyfill.outer_product[c_idx][r_idx] = true;
            let text = gen_outer_product_fn(
                type_name,
                suffix,
                usize::try_from(c).expect("positive"),
                usize::try_from(r).expect("positive"),
            );
            self.header.push_str(&text);
        }

        self.assemble_simple_intrinsic(&format!("outer_product_{c}x{r}{suffix}"), call, false)
    }
}
