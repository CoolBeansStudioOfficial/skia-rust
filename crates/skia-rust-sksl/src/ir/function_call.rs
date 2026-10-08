// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLFunctionCall.{h,cpp}: the data, `description`, overload
// resolution (`FindBestFunctionForCall`), `Convert` and `Make`.
//
// Not yet ported (see `pending`): the argument coercion of `Convert` (`Type::coerceExpression`,
// task S7a), the `Constructor::Convert` branch (S7a), `ChildCall::Make` for `eval` (S7b), the
// out-parameter `UpdateVariableRefKind` (S7b), and the intrinsic constant folding of `Make` (needs
// the S8 constant folder). Each of those paths panics with a message naming its task rather than
// producing a call that differs from Skia's.

//! [`FunctionCall`]: `function(args…)`.

use super::{
    CoercionCost, Expression, ExpressionKind, FnId, IrPool, LayoutFlags, ModifierFlags,
    ids::{ExprId, TypeId, VarId},
};
use crate::context::Context;
use crate::intrinsic_list::IntrinsicKind;
use crate::operator::OperatorPrecedence;
use crate::position::Position;
use crate::string::Separator;

/// `SkSL::FunctionCall`.
// Port of: src/sksl/ir/SkSLFunctionCall.h#L29-L98 (chrome/m156)
#[doc(alias = "SkSL::FunctionCall")]
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionCall {
    /// `function()`.
    pub function: FnId,
    /// `arguments()`.
    pub arguments: Vec<ExprId>,
    /// `stablePointer()`: identifies the call across `clone()`s (Specialization keys calls by
    /// it). It is the id the original call was allocated at
    /// ([`IrPool::next_expression_id`] before `add_expression`), and clones copy it.
    pub stable_pointer: ExprId,
}

/// Stops with the name of a Skia path that is not ported yet, so that no call is produced that
/// would differ from Skia's.
fn pending(what: &str) -> ! {
    panic!("SkSL port: {what} is not ported yet")
}

impl FunctionCall {
    /// `description()`: `name(a, b)`.
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1012-L1021 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        let mut result = format!("{}(", pool.function(self.function).name);
        let mut separator = Separator::new();
        for &arg in &self.arguments {
            result.push_str(separator.next_str());
            result.push_str(&pool.expression_description_with(arg, OperatorPrecedence::Sequence));
        }
        result.push(')');
        result
    }

    /// `FunctionCall::FindBestFunctionForCall`: the overload of `overload_chain` whose parameters
    /// the arguments coerce to at the lowest cost, or `None` when none can. A chain with a single
    /// function is returned as it is, without checking the arguments.
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1088-L1105 (chrome/m156)
    #[must_use]
    pub fn find_best_function_for_call(
        ctx: &Context,
        overload_chain: FnId,
        arguments: &[ExprId],
    ) -> Option<FnId> {
        if ctx.pool.function(overload_chain).next_overload.is_none() {
            return Some(overload_chain);
        }
        let mut best_cost = CoercionCost::impossible();
        let mut best = None;
        let mut candidate = Some(overload_chain);
        while let Some(f) = candidate {
            let cost = call_cost(ctx, f, arguments);
            if cost <= best_cost {
                best_cost = cost;
                best = Some(f);
            }
            candidate = ctx.pool.function(f).next_overload;
        }
        if best_cost.impossible { None } else { best }
    }

    /// `FunctionCall::Convert(context, pos, functionValue, arguments)`: resolves the callee
    /// (a type, a function name or a method) and converts the call. Returns `None` after
    /// reporting an error.
    ///
    /// # Panics
    ///
    /// On the paths that are not ported yet (see the module docs): a constructor call, and a call
    /// whose arguments need coercion.
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1117-L1162 (chrome/m156)
    #[must_use]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        function_value: ExprId,
        mut arguments: Vec<ExprId>,
    ) -> Option<ExprId> {
        enum Callee {
            Type,
            Function(FnId),
            Method { self_: ExprId, overload_chain: FnId },
            Poison,
            Other,
        }
        let callee = match &ctx.pool.expression(function_value).kind {
            ExpressionKind::TypeReference(_) => Callee::Type,
            ExpressionKind::FunctionReference(r) => Callee::Function(r.overload_chain),
            ExpressionKind::MethodReference(m) => Callee::Method {
                self_: m.self_,
                overload_chain: m.overload_chain,
            },
            ExpressionKind::Poison(_) => Callee::Poison,
            _ => Callee::Other,
        };
        match callee {
            Callee::Type => {
                // Port of: Constructor::Convert (src/sksl/ir/SkSLConstructor.cpp), task S7a.
                pending("Constructor::Convert (task S7a)")
            }
            Callee::Function(overload_chain) => {
                if let Some(best) =
                    Self::find_best_function_for_call(ctx, overload_chain, &arguments)
                {
                    return Self::convert_function(ctx, pos, best, arguments);
                }
                let msg = format!(
                    "no match for {}{}",
                    ctx.pool.function(overload_chain).name,
                    build_argument_type_list(&ctx.pool, &arguments)
                );
                ctx.errors.error(pos, &msg);
                None
            }
            Callee::Method {
                self_,
                overload_chain,
            } => {
                arguments.push(self_);
                if let Some(best) =
                    Self::find_best_function_for_call(ctx, overload_chain, &arguments)
                {
                    return Self::convert_function(ctx, pos, best, arguments);
                }
                let self_arg = *arguments
                    .last()
                    .expect("a method call has its self argument");
                let msg = format!(
                    "no match for {}::{}{}",
                    ctx.pool.ty(ctx.pool.expression(self_arg).ty).display_name(),
                    &ctx.pool.function(overload_chain).name[1..],
                    build_argument_type_list(&ctx.pool, &arguments[..arguments.len() - 1])
                );
                ctx.errors.error(pos, &msg);
                None
            }
            Callee::Poison => {
                ctx.pool.expression_mut(function_value).position = pos;
                Some(function_value)
            }
            Callee::Other => {
                ctx.errors.error(pos, "not a function");
                None
            }
        }
    }

    /// `FunctionCall::Convert(context, pos, function, arguments)`: checks a call to one resolved
    /// function, coerces its arguments and makes the call.
    ///
    /// # Panics
    ///
    /// On the paths that are not ported yet (see the module docs).
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1164-L1273 (chrome/m156)
    #[must_use]
    pub fn convert_function(
        ctx: &mut Context,
        pos: Position,
        function: FnId,
        arguments: Vec<ExprId>,
    ) -> Option<ExprId> {
        // Reject ES3 function calls in strict ES2 mode.
        if ctx.config().strict_es2_mode() && ctx.pool.function(function).modifier_flags.is_es3() {
            let msg = format!(
                "call to '{}' is not supported",
                ctx.pool.function(function).description(&ctx.pool)
            );
            ctx.errors.error(pos, &msg);
            return None;
        }

        // Reject function calls with the wrong number of arguments.
        let parameter_count = ctx.pool.function(function).parameters.len();
        if parameter_count != arguments.len() {
            let mut msg = format!(
                "call to '{}' expected {} argument",
                ctx.pool.function(function).name,
                parameter_count
            );
            if parameter_count != 1 {
                msg.push('s');
            }
            msg.push_str(", but found ");
            msg.push_str(&arguments.len().to_string());
            ctx.errors.error(pos, &msg);
            return None;
        }

        // If the arguments do not match the parameter types due to mismatched modifiers, reject
        // the function call.
        for (i, &arg) in arguments.iter().enumerate() {
            let param = ctx.pool.function(function).parameters[i];
            if !argument_and_parameter_flags_match(&ctx.pool, arg, param) {
                let (msg, arg_pos) = {
                    let p = ctx.pool.variable(param);
                    let msg = format!(
                        "expected argument of type '{}{}{}'",
                        p.layout.padded_description(),
                        p.modifier_flags.padded_description(),
                        ctx.pool.ty(p.ty).description()
                    );
                    (msg, ctx.pool.expression(arg).position)
                };
                ctx.errors.error(arg_pos, &msg);
                return None;
            }
        }

        // Resolve generic types.
        let Some((_parameter_types, return_type)) = ctx
            .pool
            .function(function)
            .determine_final_types(&ctx.pool, &arguments)
        else {
            let msg = format!(
                "no match for {}{}",
                ctx.pool.function(function).name,
                build_argument_type_list(&ctx.pool, &arguments)
            );
            ctx.errors.error(pos, &msg);
            return None;
        };

        if !arguments.is_empty() {
            // Skia coerces each argument to its parameter type (`Type::coerceExpression`, which
            // needs `_parameter_types`) and updates the reference kind of out-parameters
            // (`Analysis::UpdateVariableRefKind`).
            pending("argument coercion and out-parameter reference kinds (tasks S7a and S7b)");
        }

        if ctx.pool.function(function).is_main {
            ctx.errors.error(pos, "call to 'main' is not allowed");
            return None;
        }

        let intrinsic = ctx.pool.function(function).intrinsic_kind;
        if intrinsic == Some(IntrinsicKind::WorkgroupUniformLoad) {
            let root = workgroup_uniform_load_root(&ctx.pool, arguments[0]);
            if !root.is_some_and(|v| ctx.pool.variable(v).modifier_flags.is_workgroup()) {
                ctx.errors.error(
                    pos,
                    "workgroupUniformLoad must be called with a variable expression in workgroup \
                     address space",
                );
                return None;
            }
        }

        if intrinsic == Some(IntrinsicKind::Eval) {
            // A method call on an effect child becomes a ChildCall (`ChildCall::Make`, task S7b).
            pending("ChildCall::Make for eval() (task S7b)");
        }

        Some(Self::make(ctx, pos, return_type, function, arguments))
    }

    /// `FunctionCall::Make`: allocates a call to `function`, with the arguments it has been
    /// given.
    ///
    /// # Panics
    ///
    /// If `function` is an intrinsic that may be called with compile-time constant arguments:
    /// Skia folds those calls (`optimize_intrinsic_call`), which is not ported yet.
    // Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1275-L1298 (chrome/m156)
    #[must_use]
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        return_type: TypeId,
        function: FnId,
        arguments: Vec<ExprId>,
    ) -> ExprId {
        debug_assert_eq!(
            ctx.pool.function(function).parameters.len(),
            arguments.len()
        );
        if ctx.pool.function(function).is_intrinsic()
            && !arguments
                .iter()
                .all(|&arg| is_definitely_not_constant(&ctx.pool, arg))
        {
            pending(
                "intrinsic constant folding (FunctionCall::Make, needs the S8 constant folder)",
            );
        }
        // The call's stable pointer is the id it is about to be allocated at.
        let stable_pointer = ctx.pool.next_expression_id();
        ctx.pool.add_expression(Expression::new(
            pos,
            return_type,
            ExpressionKind::FunctionCall(Self {
                function,
                arguments,
                stable_pointer,
            }),
        ))
    }
}

/// `CallCost`: the cost of calling `function` with `arguments`, or impossible when the call is
/// not valid. Lower costs are preferred. This is never called for functions with one definition.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1054-L1086 (chrome/m156)
fn call_cost(ctx: &Context, function: FnId, arguments: &[ExprId]) -> CoercionCost {
    let pool = &ctx.pool;
    let decl = pool.function(function);
    // Strict-ES2 programs can never call an `$es3` function.
    if ctx.config().strict_es2_mode() && decl.modifier_flags.is_es3() {
        return CoercionCost::impossible();
    }
    // Functions with the wrong number of parameters are never a match.
    if decl.parameters.len() != arguments.len() {
        return CoercionCost::impossible();
    }
    // If the arguments cannot be coerced to the parameter types, the function is never a match.
    let Some((types, _)) = decl.determine_final_types(pool, arguments) else {
        return CoercionCost::impossible();
    };
    // If the arguments do not match the parameter types due to mismatched modifiers, the function
    // is never a match.
    for (&arg, &param) in arguments.iter().zip(&decl.parameters) {
        if !argument_and_parameter_flags_match(pool, arg, param) {
            return CoercionCost::impossible();
        }
    }
    // Return the sum of coercion costs of each argument.
    arguments
        .iter()
        .zip(&types)
        .fold(CoercionCost::free(), |total, (&arg, &ty)| {
            total + pool.ty(pool.expression(arg).ty).coercion_cost(ty)
        })
}

/// `argument_and_parameter_flags_match`: a storage or read-only texture parameter needs an
/// argument with the same pixel format. Other flags do not separate overloads.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1023-L1052 (chrome/m156)
fn argument_and_parameter_flags_match(pool: &IrPool, argument: ExprId, parameter: VarId) -> bool {
    let param = pool.variable(parameter);
    // If the function parameter is a storage texture or readonly texture, the pixel format of the
    // argument must match the parameter's pixel format (including when neither has a format).
    let param_ty = pool.ty(param.ty);
    if param_ty.is_storage_texture() || param_ty.is_read_only_texture() {
        // Currently, we do not support texture arrays, so GetRootVariable isn't completely
        // necessary (we only need to trace back the variable reference), but this may be added in
        // the future.
        let Some(var) = get_root_variable(pool, argument) else {
            return false;
        };
        let arg_pixel_format = pool.variable(var).layout.flags & LayoutFlags::ALL_PIXEL_FORMATS;
        let param_pixel_format = param.layout.flags & LayoutFlags::ALL_PIXEL_FORMATS;
        if arg_pixel_format != param_pixel_format {
            return false;
        }
    }
    true
}

/// `Analysis::GetRootVariable`: the variable an lvalue-like expression is rooted in, through
/// indexing, field access and swizzles.
// Port of: src/sksl/SkSLAnalysis.cpp#L450-L471 (chrome/m156)
fn get_root_variable(pool: &IrPool, expr: ExprId) -> Option<VarId> {
    let mut e = expr;
    loop {
        match &pool.expression(e).kind {
            ExpressionKind::VariableReference(v) => return Some(v.variable),
            ExpressionKind::Index(i) => e = i.base,
            ExpressionKind::FieldAccess(f) => e = f.base,
            ExpressionKind::Swizzle(s) => e = s.base,
            _ => return None,
        }
    }
}

/// The root variable that a `workgroupUniformLoad` argument names, stopping at a vector index.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1213-L1238 (chrome/m156)
fn workgroup_uniform_load_root(pool: &IrPool, argument: ExprId) -> Option<VarId> {
    let mut expr = argument;
    loop {
        match &pool.expression(expr).kind {
            ExpressionKind::VariableReference(v) => return Some(v.variable),
            ExpressionKind::FieldAccess(f) => expr = f.base,
            ExpressionKind::Index(i) => {
                if pool.ty(pool.expression(i.base).ty).is_vector() {
                    return None;
                }
                expr = i.base;
            }
            _ => return None,
        }
    }
}

/// Whether `expr` is certainly not a compile-time constant. A variable that is not `const` holds
/// no constant value, so a call with only such arguments is never folded. Any other expression
/// may be a constant, and is not decided here.
fn is_definitely_not_constant(pool: &IrPool, expr: ExprId) -> bool {
    match &pool.expression(expr).kind {
        ExpressionKind::VariableReference(v) => !pool
            .variable(v.variable)
            .modifier_flags
            .contains(ModifierFlags::CONST),
        _ => false,
    }
}

/// `buildArgumentTypeList`: `(float, int4)`.
// Port of: src/sksl/ir/SkSLFunctionCall.cpp#L1107-L1115 (chrome/m156)
fn build_argument_type_list(pool: &IrPool, arguments: &[ExprId]) -> String {
    let mut result = String::from("(");
    let mut separator = Separator::new();
    for &arg in arguments {
        result.push_str(separator.next_str());
        result.push_str(pool.ty(pool.expression(arg).ty).display_name());
    }
    result.push(')');
    result
}

#[cfg(test)]
mod tests {
    use super::FunctionCall;
    use crate::context::Context;
    use crate::error_reporter::{ErrorReporter, ErrorSink};
    use crate::ir::{
        ExprId, Expression, ExpressionKind, FnId, FunctionReference, ModifierFlags, Modifiers,
        SymbolTable, TypeId, VarId, Variable, VariableRefKind, VariableReference, VariableStorage,
    };
    use crate::modules::ModuleType;
    use crate::position::Position;
    use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

    fn context(kind: ProgramKind) -> Context {
        let mut ctx = Context::new(ErrorReporter::forwarding());
        ctx.config = Some(ProgramConfig::new(
            ModuleType::Program,
            kind,
            ProgramSettings::default(),
        ));
        ctx.symbol_table = Some(ctx.pool.add_symbol_table(SymbolTable::new(None, false)));
        ctx
    }

    fn errors(ctx: &Context) -> Vec<String> {
        match ctx.errors.sink() {
            ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
            other => panic!("expected a forwarding reporter, found {other:?}"),
        }
    }

    fn declare(ctx: &mut Context, name: &str, params: &[TypeId], return_type: TypeId) -> FnId {
        let params: Vec<VarId> = params
            .iter()
            .enumerate()
            .map(|(i, &ty)| {
                ctx.pool.add_variable(Variable::new(
                    Position::default(),
                    Position::default(),
                    ModifierFlags::empty(),
                    format!("p{i}"),
                    ty,
                    false,
                    VariableStorage::Parameter,
                ))
            })
            .collect();
        crate::ir::FunctionDeclaration::convert(
            ctx,
            Position::default(),
            &Modifiers::default(),
            name,
            &params,
            Position::default(),
            return_type,
        )
        .expect("declaration")
    }

    fn reference(ctx: &mut Context, overload_chain: FnId) -> ExprId {
        ctx.pool.add_expression(Expression::new(
            Position::default(),
            TypeId::INVALID,
            ExpressionKind::FunctionReference(FunctionReference { overload_chain }),
        ))
    }

    fn variable_of(ctx: &mut Context, name: &str, ty: TypeId) -> ExprId {
        let var = ctx.pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            ModifierFlags::empty(),
            name,
            ty,
            false,
            VariableStorage::Local,
        ));
        ctx.pool.add_expression(Expression::new(
            Position::default(),
            ty,
            ExpressionKind::VariableReference(VariableReference {
                variable: var,
                ref_kind: VariableRefKind::Read,
            }),
        ))
    }

    #[test]
    fn a_single_overload_is_chosen_without_checking_arguments() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", &[TypeId::FLOAT], TypeId::FLOAT);
        let arg = variable_of(&mut ctx, "x", TypeId::FLOAT4);
        assert_eq!(
            FunctionCall::find_best_function_for_call(&ctx, f, &[arg]),
            Some(f)
        );
    }

    #[test]
    fn overloads_resolve_to_the_cheapest_match() {
        let mut ctx = context(ProgramKind::Compute);
        let float_overload = declare(&mut ctx, "g", &[TypeId::FLOAT], TypeId::FLOAT);
        // The newest declaration heads the overload chain.
        let int_overload = declare(&mut ctx, "g", &[TypeId::INT], TypeId::INT);
        assert_eq!(
            ctx.pool.function(int_overload).next_overload,
            Some(float_overload)
        );
        let int_arg = variable_of(&mut ctx, "i", TypeId::INT);
        assert_eq!(
            FunctionCall::find_best_function_for_call(&ctx, int_overload, &[int_arg]),
            Some(int_overload)
        );
        let float_arg = variable_of(&mut ctx, "f", TypeId::FLOAT);
        assert_eq!(
            FunctionCall::find_best_function_for_call(&ctx, int_overload, &[float_arg]),
            Some(float_overload)
        );
        let vector_arg = variable_of(&mut ctx, "v", TypeId::FLOAT4);
        assert_eq!(
            FunctionCall::find_best_function_for_call(&ctx, int_overload, &[vector_arg]),
            None
        );
    }

    #[test]
    fn a_call_with_the_wrong_argument_count_reports_it() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", &[TypeId::FLOAT], TypeId::FLOAT);
        let callee = reference(&mut ctx, f);
        assert!(FunctionCall::convert(&mut ctx, Position::default(), callee, vec![]).is_none());
        assert_eq!(
            errors(&ctx),
            ["call to 'f' expected 1 argument, but found 0"]
        );
    }

    #[test]
    fn a_call_matching_no_overload_lists_the_argument_types() {
        let mut ctx = context(ProgramKind::Compute);
        let _ = declare(&mut ctx, "f", &[TypeId::FLOAT], TypeId::FLOAT);
        let f2 = declare(&mut ctx, "f", &[TypeId::FLOAT2], TypeId::FLOAT2);
        let callee = reference(&mut ctx, f2);
        let arg = variable_of(&mut ctx, "v", TypeId::FLOAT4);
        assert!(FunctionCall::convert(&mut ctx, Position::default(), callee, vec![arg]).is_none());
        assert_eq!(errors(&ctx), ["no match for f(float4)"]);
    }

    #[test]
    fn a_value_that_is_not_callable_is_reported() {
        let mut ctx = context(ProgramKind::Compute);
        let value = variable_of(&mut ctx, "x", TypeId::FLOAT);
        assert!(FunctionCall::convert(&mut ctx, Position::default(), value, vec![]).is_none());
        assert_eq!(errors(&ctx), ["not a function"]);
    }

    #[test]
    fn main_cannot_be_called() {
        let mut ctx = context(ProgramKind::Fragment);
        let main = declare(&mut ctx, "main", &[], TypeId::VOID);
        let callee = reference(&mut ctx, main);
        assert!(FunctionCall::convert(&mut ctx, Position::default(), callee, vec![]).is_none());
        assert_eq!(errors(&ctx), ["call to 'main' is not allowed"]);
    }

    #[test]
    fn make_records_the_id_the_call_was_allocated_at() {
        let mut ctx = context(ProgramKind::Compute);
        let f = declare(&mut ctx, "f", &[], TypeId::VOID);
        let call = FunctionCall::make(&mut ctx, Position::default(), TypeId::VOID, f, vec![]);
        let ExpressionKind::FunctionCall(c) = &ctx.pool.expression(call).kind else {
            panic!("expected a function call");
        };
        assert_eq!(c.stable_pointer, call);
        assert_eq!(c.function, f);
        assert_eq!(ctx.pool.expression(call).description(&ctx.pool), "f()");
    }
}
