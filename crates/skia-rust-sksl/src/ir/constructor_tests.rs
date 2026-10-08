// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the constructors and literals of task S7a: `Constructor::Convert` and the `Make`
//! factories, the constant values and comparisons they answer, and `coerce_expression`. The
//! messages are the texts of Skia's goldens (`tests/sksl/errors`), copied character for character.

use super::*;
use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::modules::ModuleType;
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

fn context() -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        ProgramKind::Fragment,
        ProgramSettings::default(),
    ));
    ctx
}

/// The messages reported so far, in order.
fn messages(ctx: &Context) -> Vec<String> {
    match ctx.errors.sink() {
        ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
        other => panic!("expected a forwarding reporter, found {other:?}"),
    }
}

fn lit(ctx: &mut Context, value: f64, ty: TypeId) -> ExprId {
    Literal::make(&mut ctx.pool, Position::default(), value, ty)
}

/// A variable of type `ty` read through a reference: a value the constructors cannot fold.
fn variable(ctx: &mut Context, name: &str, ty: TypeId) -> ExprId {
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

fn describe(ctx: &Context, id: ExprId) -> String {
    ctx.pool.expression_description(id)
}

fn kind(ctx: &Context, id: ExprId) -> &ExpressionKind {
    &ctx.pool.expression(id).kind
}

fn convert(ctx: &mut Context, ty: TypeId, args: Vec<ExprId>) -> Option<ExprId> {
    constructor::convert(ctx, Position::default(), ty, args)
}

#[test]
fn literal_make_rounds_to_its_type() {
    let mut ctx = context();
    // A float literal is rounded to `float`.
    let f = lit(&mut ctx, 0.1, TypeId::FLOAT);
    assert_eq!(
        ctx.pool.expression(f).as_literal().unwrap().value,
        f64::from(0.1_f32)
    );
    // An integer literal is truncated toward zero.
    let i = lit(&mut ctx, 2.9, TypeId::INT);
    assert_eq!(ctx.pool.expression(i).as_literal().unwrap().value, 2.0);
    // A boolean literal is `value != 0`.
    let b = lit(&mut ctx, 0.5, TypeId::BOOL);
    assert_eq!(ctx.pool.expression(b).as_literal().unwrap().value, 1.0);

    assert_eq!(describe(&ctx, f), "0.1");
    assert_eq!(describe(&ctx, i), "2");
    assert_eq!(describe(&ctx, b), "true");
    let one = Literal::make_float_literal(&mut ctx.pool, Position::default(), 1.0);
    assert_eq!(ctx.pool.expression(one).ty, TypeId::FLOAT_LITERAL);
    assert_eq!(describe(&ctx, one), "1.0");
}

#[test]
fn compound_from_scalars() {
    let mut ctx = context();
    let args = (1..=4)
        .map(|v| lit(&mut ctx, f64::from(v), TypeId::FLOAT))
        .collect();
    let id = convert(&mut ctx, TypeId::FLOAT4, args).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorCompound(_)
    ));
    assert_eq!(describe(&ctx, id), "float4(1.0, 2.0, 3.0, 4.0)");
    assert_eq!(messages(&ctx), Vec::<String>::new());
}

#[test]
fn a_single_scalar_splats() {
    let mut ctx = context();
    let one = lit(&mut ctx, 1.0, TypeId::INT_LITERAL);
    let id = convert(&mut ctx, TypeId::FLOAT4, vec![one]).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorSplat(_)
    ));
    assert_eq!(describe(&ctx, id), "float4(1.0)");
}

#[test]
fn repeated_scalars_become_a_splat() {
    let mut ctx = context();
    let args = (0..4).map(|_| lit(&mut ctx, 2.0, TypeId::FLOAT)).collect();
    let id = convert(&mut ctx, TypeId::FLOAT4, args).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorSplat(_)
    ));
    assert_eq!(describe(&ctx, id), "float4(2.0)");
}

#[test]
fn nested_compound_is_flattened() {
    let mut ctx = context();
    let a = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let b = lit(&mut ctx, 2.0, TypeId::FLOAT);
    let xy = convert(&mut ctx, TypeId::FLOAT2, vec![a, b]).unwrap();
    let three = lit(&mut ctx, 3.0, TypeId::FLOAT);
    let four = lit(&mut ctx, 4.0, TypeId::FLOAT);
    let id = convert(&mut ctx, TypeId::FLOAT4, vec![xy, three, four]).unwrap();
    assert_eq!(describe(&ctx, id), "float4(1.0, 2.0, 3.0, 4.0)");
}

#[test]
fn diagonal_matrix_constant_slots() {
    let mut ctx = context();
    let two = lit(&mut ctx, 2.0, TypeId::FLOAT);
    let id = convert(&mut ctx, TypeId::FLOAT2X2, vec![two]).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorDiagonalMatrix(_)
    ));
    let slots: Vec<_> = (0..4)
        .map(|n| ctx.pool.expression(id).get_constant_value(&ctx.pool, n))
        .collect();
    assert_eq!(slots, [Some(2.0), Some(0.0), Some(0.0), Some(2.0)]);
    assert_eq!(describe(&ctx, id), "float2x2(2.0)");
}

#[test]
fn matrix_resize_keeps_the_inner_slots() {
    let mut ctx = context();
    let m = variable(&mut ctx, "m", TypeId::FLOAT2X2);
    let id = convert(&mut ctx, TypeId::FLOAT3X3, vec![m]).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorMatrixResize(_)
    ));
    let expr = ctx.pool.expression(id);
    // Slot 0 is a cell of the wrapped variable, which is not a constant.
    assert_eq!(expr.get_constant_value(&ctx.pool, 0), None);
    // Row 2, column 0 is outside the wrapped 2x2: the identity has 0 there.
    assert_eq!(expr.get_constant_value(&ctx.pool, 2), Some(0.0));
    // Row 2, column 2 is the identity's 1.
    assert_eq!(expr.get_constant_value(&ctx.pool, 8), Some(1.0));
}

#[test]
fn out_of_range_scalar_reports_the_golden_message() {
    let mut ctx = context();
    let big = lit(&mut ctx, 65536.0, TypeId::INT_LITERAL);
    assert_eq!(convert(&mut ctx, TypeId::USHORT, vec![big]), None);
    assert_eq!(
        messages(&ctx),
        ["value is out of range for type 'ushort': 65536"]
    );
}

#[test]
fn vector_argument_to_a_scalar_suggests_a_swizzle() {
    let mut ctx = context();
    let v = variable(&mut ctx, "v", TypeId::HALF4);
    assert_eq!(convert(&mut ctx, TypeId::HALF, vec![v]), None);
    assert_eq!(
        messages(&ctx),
        ["'half4' is not a valid parameter to 'half' constructor; use '.x' instead"]
    );
}

#[test]
fn vector_slice_suggests_a_swizzle() {
    let mut ctx = context();
    let v = variable(&mut ctx, "v", TypeId::FLOAT4);
    assert_eq!(convert(&mut ctx, TypeId::FLOAT2, vec![v]), None);
    assert_eq!(
        messages(&ctx),
        ["'float4' is not a valid parameter to 'float2' constructor; use '.xy' instead"]
    );
}

#[test]
fn array_element_count_is_checked() {
    let mut ctx = context();
    let name = ctx.pool.ty(TypeId::FLOAT).array_name(1);
    let abbrev = ctx.pool.ty(TypeId::FLOAT).abbreviated_name;
    let arr = ctx
        .pool
        .add_type(Type::new_array_type(name, abbrev, TypeId::FLOAT, 1, false));
    assert_eq!(convert(&mut ctx, arr, vec![]), None);
    assert_eq!(
        messages(&ctx),
        ["invalid arguments to 'float[1]' constructor (expected 1 elements, but found 0)"]
    );
}

#[test]
fn array_constructor_builds_its_elements() {
    let mut ctx = context();
    let name = ctx.pool.ty(TypeId::FLOAT).array_name(2);
    let abbrev = ctx.pool.ty(TypeId::FLOAT).abbreviated_name;
    let arr = ctx
        .pool
        .add_type(Type::new_array_type(name, abbrev, TypeId::FLOAT, 2, false));
    let a = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let b = lit(&mut ctx, 2.0, TypeId::FLOAT);
    let id = convert(&mut ctx, arr, vec![a, b]).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorArray(_)
    ));
    assert_eq!(describe(&ctx, id), "float[2](1.0, 2.0)");
}

#[test]
fn struct_constructor_checks_the_field_count() {
    let mut ctx = context();
    let fields = vec![
        Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "a".into(),
            ty: TypeId::FLOAT,
        },
        Field {
            position: Position::default(),
            layout: Layout::new(),
            modifier_flags: ModifierFlags::empty(),
            name: "b".into(),
            ty: TypeId::INT,
        },
    ];
    let data = StructType::new(&ctx.pool, fields, 1, false, false);
    let s = ctx
        .pool
        .add_type(Type::new_struct_type(Position::default(), "S".into(), data));

    let one = lit(&mut ctx, 1.0, TypeId::FLOAT);
    assert_eq!(convert(&mut ctx, s, vec![one]), None);
    assert_eq!(
        messages(&ctx),
        ["invalid arguments to 'S' constructor (expected 2 elements, but found 1)"]
    );

    let a = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let b = lit(&mut ctx, 2.0, TypeId::INT_LITERAL);
    let id = convert(&mut ctx, s, vec![a, b]).unwrap();
    assert!(matches!(
        kind(&ctx, id),
        ExpressionKind::ConstructorStruct(_)
    ));
    assert_eq!(describe(&ctx, id), "S(1.0, 2)");
}

#[test]
fn coerce_reports_an_impossible_conversion() {
    let mut ctx = context();
    let f = lit(&mut ctx, 1.0, TypeId::FLOAT);
    assert_eq!(TypeId::INT.coerce_expression(&mut ctx, f), None);
    assert_eq!(messages(&ctx), ["expected 'int', but found 'float'"]);
}

#[test]
fn coerce_converts_a_literal() {
    let mut ctx = context();
    let i = lit(&mut ctx, 3.0, TypeId::INT_LITERAL);
    let id = TypeId::FLOAT
        .coerce_expression(&mut ctx, i)
        .expect("an int literal coerces to float");
    assert_eq!(describe(&ctx, id), "3.0");
    assert_eq!(messages(&ctx), Vec::<String>::new());
}

#[test]
fn array_size_must_be_an_integer_constant() {
    let mut ctx = context();
    let v = variable(&mut ctx, "n", TypeId::INT);
    assert_eq!(
        TypeId::FLOAT.convert_array_size(&mut ctx, Position::default(), v),
        0
    );
    assert_eq!(messages(&ctx), ["array size must be an integer"]);

    let zero = lit(&mut ctx, 0.0, TypeId::INT_LITERAL);
    assert_eq!(
        TypeId::FLOAT.convert_array_size(&mut ctx, Position::default(), zero),
        0
    );
    assert_eq!(
        messages(&ctx).last().map(String::as_str),
        Some("array size must be positive")
    );

    let three = lit(&mut ctx, 3.0, TypeId::INT_LITERAL);
    assert_eq!(
        TypeId::FLOAT.convert_array_size(&mut ctx, Position::default(), three),
        3
    );
}

#[test]
fn compare_constant_of_constructors() {
    let mut ctx = context();
    let a1 = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let a2 = lit(&mut ctx, 2.0, TypeId::FLOAT);
    // Each node owns its arguments, so every constructor gets its own literals.
    let b1 = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let b2 = lit(&mut ctx, 2.0, TypeId::FLOAT);
    let c1 = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let c2 = lit(&mut ctx, 3.0, TypeId::FLOAT);
    let x = convert(&mut ctx, TypeId::FLOAT2, vec![a1, a2]).unwrap();
    let same = convert(&mut ctx, TypeId::FLOAT2, vec![b1, b2]).unwrap();
    let other = convert(&mut ctx, TypeId::FLOAT2, vec![c1, c2]).unwrap();
    let v = variable(&mut ctx, "v", TypeId::FLOAT2);

    let pool = &ctx.pool;
    let x_expr = pool.expression(x);
    assert_eq!(
        x_expr.compare_constant(pool, pool.expression(same)),
        ComparisonResult::Equal
    );
    assert_eq!(
        x_expr.compare_constant(pool, pool.expression(other)),
        ComparisonResult::NotEqual
    );
    assert_eq!(
        x_expr.compare_constant(pool, pool.expression(v)),
        ComparisonResult::Unknown
    );
}

#[test]
fn compare_constant_of_literals() {
    let mut ctx = context();
    let f = lit(&mut ctx, 1.0, TypeId::FLOAT);
    let i = lit(&mut ctx, 1.0, TypeId::INT);
    let pool = &ctx.pool;
    // Different number kinds never compare.
    assert_eq!(
        pool.expression(f)
            .compare_constant(pool, pool.expression(i)),
        ComparisonResult::Unknown
    );
    assert_eq!(
        pool.expression(f)
            .compare_constant(pool, pool.expression(f)),
        ComparisonResult::Equal
    );
}
