// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the type, symbol and declaration rules of task S6 (`docs/design/sksl.md` §10), checked
//! against the messages and results Skia's code gives. The Skia goldens reach most of these only
//! through the front end, which is not yet ported.

use crate::context::Context;
use crate::error_reporter::{ErrorReporter, ErrorSink};
use crate::ir::{
    CoercionCost, Field, IrPool, Layout, LayoutFlags, ModifierFlags, SymbolId, SymbolTable, Type,
    TypeId, add_array_dimension, add_symbol, would_shadow_symbols_from,
};
use crate::modules::ModuleType;
use crate::operator::{BinaryTypes, Operator, OperatorKind};
use crate::position::Position;
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};

fn context(kind: ProgramKind) -> Context {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    ctx.config = Some(ProgramConfig::new(
        ModuleType::Program,
        kind,
        ProgramSettings::default(),
    ));
    ctx
}

/// The messages reported so far, in order.
fn errors(ctx: &Context) -> Vec<String> {
    match ctx.errors.sink() {
        ErrorSink::Forwarding { errors } => errors.iter().map(|(msg, _)| msg.clone()).collect(),
        other => panic!("expected a forwarding reporter, found {other:?}"),
    }
}

fn field(name: &str, ty: TypeId) -> Field {
    Field {
        position: Position::default(),
        layout: Layout::new(),
        modifier_flags: ModifierFlags::empty(),
        name: name.into(),
        ty,
    }
}

#[test]
fn coercion_costs_follow_skia() {
    let pool = IrPool::new();
    let float = pool.ty(TypeId::FLOAT);
    assert_eq!(float.coercion_cost(TypeId::FLOAT), CoercionCost::free());
    // Different number kinds never coerce.
    assert_eq!(float.coercion_cost(TypeId::INT), CoercionCost::impossible());
    // An integer literal coerces freely to `float`.
    assert_eq!(
        pool.ty(TypeId::INT_LITERAL).coercion_cost(TypeId::FLOAT),
        CoercionCost::free()
    );
    // Narrowing (float to half) costs its priority gap; widening is a normal conversion.
    assert_eq!(
        float.coercion_cost(TypeId::HALF),
        CoercionCost::narrowing(1)
    );
    assert_eq!(
        pool.ty(TypeId::HALF).coercion_cost(TypeId::FLOAT),
        CoercionCost::normal(1)
    );
    // Vectors of different sizes never coerce; vectors of the same size coerce by component.
    assert_eq!(
        pool.ty(TypeId::FLOAT2).coercion_cost(TypeId::FLOAT3),
        CoercionCost::impossible()
    );
    assert_eq!(
        pool.ty(TypeId::FLOAT2).coercion_cost(TypeId::HALF2),
        CoercionCost::narrowing(1)
    );
    assert!(float.can_coerce_to(TypeId::HALF, true));
    assert!(!float.can_coerce_to(TypeId::HALF, false));
}

#[test]
fn to_compound_builds_the_vector_or_matrix_of_the_component() {
    let pool = IrPool::new();
    assert_eq!(pool.ty(TypeId::FLOAT).to_compound(1, 1), TypeId::FLOAT);
    assert_eq!(pool.ty(TypeId::FLOAT).to_compound(4, 1), TypeId::FLOAT4);
    assert_eq!(pool.ty(TypeId::FLOAT).to_compound(3, 2), TypeId::FLOAT3X2);
    assert_eq!(pool.ty(TypeId::HALF).to_compound(2, 4), TypeId::HALF2X4);
    assert_eq!(pool.ty(TypeId::INT_LITERAL).to_compound(2, 1), TypeId::INT2);
    assert_eq!(pool.ty(TypeId::BOOL).to_compound(4, 1), TypeId::BOOL4);
}

#[test]
fn precision_qualifiers_narrow_to_the_mediump_type() {
    let mut ctx = context(ProgramKind::RuntimeShader);
    let mut flags = ModifierFlags::MEDIUMP;
    let ty = TypeId::FLOAT2.apply_qualifiers(&mut ctx, &mut flags, Position::default());
    assert_eq!(ty, TypeId::HALF2);
    // The consumed qualifier is removed from the declaration's flags.
    assert_eq!(flags, ModifierFlags::empty());
    assert_eq!(errors(&ctx), Vec::<String>::new());

    let mut flags = ModifierFlags::HIGHP;
    let ty = TypeId::HALF.apply_qualifiers(&mut ctx, &mut flags, Position::default());
    assert_eq!(ty, TypeId::POISON);
    assert_eq!(
        errors(&ctx),
        ["type 'half' does not support precision qualifiers"]
    );
}

#[test]
fn precision_qualifiers_are_only_for_runtime_effects() {
    let mut ctx = context(ProgramKind::Fragment);
    let mut flags = ModifierFlags::MEDIUMP;
    let ty = TypeId::FLOAT.apply_qualifiers(&mut ctx, &mut flags, Position::default());
    assert_eq!(ty, TypeId::POISON);
    assert_eq!(errors(&ctx), ["precision qualifiers are not allowed"]);
}

#[test]
fn texture2d_needs_an_access_qualifier() {
    let mut ctx = context(ProgramKind::RuntimeShader);
    let mut flags = ModifierFlags::empty();
    let ty = TypeId::TEXTURE2D.apply_qualifiers(&mut ctx, &mut flags, Position::default());
    assert_eq!(ty, TypeId::TEXTURE2D);
    assert_eq!(
        errors(&ctx),
        ["'texture2D' requires a 'readonly' or 'writeonly' access qualifier"]
    );

    let mut flags = ModifierFlags::READ_ONLY;
    let ty = TypeId::TEXTURE2D.apply_qualifiers(&mut ctx, &mut flags, Position::default());
    assert_eq!(ty, TypeId::READ_ONLY_TEXTURE2D);
    assert_eq!(flags, ModifierFlags::empty());
}

#[test]
fn struct_checks_report_skias_messages() {
    let pos = Position::default();

    let mut ctx = context(ProgramKind::Fragment);
    let empty = Type::make_struct_type(&mut ctx, pos, "S", Vec::new(), false);
    assert_eq!(empty.name, "S");
    assert_eq!(errors(&ctx), ["struct 'S' must contain at least one field"]);

    let mut ctx = context(ProgramKind::Fragment);
    let fields = vec![field("a", TypeId::FLOAT), field("a", TypeId::INT)];
    let _ = Type::make_struct_type(&mut ctx, pos, "S", fields, false);
    assert_eq!(
        errors(&ctx),
        ["field 'a' was already defined in the same struct ('S')"]
    );

    let mut ctx = context(ProgramKind::Fragment);
    let _ = Type::make_struct_type(&mut ctx, pos, "S", vec![field("v", TypeId::VOID)], false);
    assert_eq!(errors(&ctx), ["type 'void' is not permitted in a struct"]);

    let mut ctx = context(ProgramKind::Fragment);
    let _ = Type::make_struct_type(&mut ctx, pos, "IB", vec![field("b", TypeId::BOOL)], true);
    assert_eq!(
        errors(&ctx),
        ["type 'bool' is not permitted in an interface block"]
    );
}

#[test]
fn array_dimensions_are_added_once_and_reused() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = ctx.pool.add_symbol_table(SymbolTable::new(None, false));
    ctx.symbol_table = Some(table);

    let array = add_array_dimension(&mut ctx, table, TypeId::FLOAT, 3);
    assert_eq!(ctx.pool.ty(array).name(), "float[3]");
    assert!(ctx.pool.ty(array).is_array());
    // The same array type is found by name, so the second request returns the same id.
    assert_eq!(
        add_array_dimension(&mut ctx, table, TypeId::FLOAT, 3),
        array
    );
    // An array of zero elements is the element type itself.
    assert_eq!(
        add_array_dimension(&mut ctx, table, TypeId::FLOAT, 0),
        TypeId::FLOAT
    );
    assert_eq!(errors(&ctx), Vec::<String>::new());

    // Adding the array type's name again is a duplicate symbol.
    add_symbol(&mut ctx, table, SymbolId::Type(array));
    assert_eq!(errors(&ctx), ["symbol 'float[3]' was already defined"]);
}

#[test]
fn symbol_tables_shadow_by_name() {
    let mut ctx = context(ProgramKind::Fragment);
    let table = ctx.pool.add_symbol_table(SymbolTable::new(None, false));
    let other = ctx.pool.add_symbol_table(SymbolTable::new(None, false));
    let array = add_array_dimension(&mut ctx, table, TypeId::INT, 2);
    assert!(!would_shadow_symbols_from(&ctx.pool, table, other));
    add_symbol(&mut ctx, other, SymbolId::Type(array));
    assert!(would_shadow_symbols_from(&ctx.pool, table, other));
    assert!(would_shadow_symbols_from(&ctx.pool, other, table));
}

#[test]
fn layout_checks_report_unpermitted_qualifiers() {
    let mut ctx = context(ProgramKind::Fragment);
    let layout = Layout {
        flags: LayoutFlags::LOCATION,
        location: 0,
        ..Layout::new()
    };
    assert!(layout.check_permitted_layout(&mut ctx, Position::default(), LayoutFlags::LOCATION));
    assert_eq!(errors(&ctx), Vec::<String>::new());
    assert!(!layout.check_permitted_layout(&mut ctx, Position::default(), LayoutFlags::empty()));
    assert_eq!(
        errors(&ctx),
        ["layout qualifier 'location' is not permitted here"]
    );
}

#[test]
fn modifier_checks_report_unpermitted_modifiers() {
    let mut ctx = context(ProgramKind::Fragment);
    let flags = ModifierFlags::UNIFORM | ModifierFlags::CONST;
    assert!(!flags.check_permitted_flags(&mut ctx, Position::default(), ModifierFlags::CONST));
    assert_eq!(errors(&ctx), ["'uniform' is not permitted here"]);
}

#[test]
fn binary_types_follow_determine_binary_type() {
    let ctx = context(ProgramKind::Fragment);
    let plus = Operator::from(OperatorKind::Plus);
    assert_eq!(
        plus.determine_binary_type(&ctx, TypeId::FLOAT, TypeId::FLOAT),
        Some(BinaryTypes {
            left: TypeId::FLOAT,
            right: TypeId::FLOAT,
            result: TypeId::FLOAT,
        })
    );
    // `float + int` has no operator: neither side coerces to the other.
    assert_eq!(
        plus.determine_binary_type(&ctx, TypeId::FLOAT, TypeId::INT),
        None
    );
    // A scalar widens to the vector it is combined with; only the right side is converted.
    assert_eq!(
        plus.determine_binary_type(&ctx, TypeId::FLOAT, TypeId::FLOAT4),
        Some(BinaryTypes {
            left: TypeId::FLOAT,
            right: TypeId::FLOAT4,
            result: TypeId::FLOAT4,
        })
    );
    // `mat4 * vec4` is a vec4.
    let times = Operator::from(OperatorKind::Star);
    assert_eq!(
        times.determine_binary_type(&ctx, TypeId::FLOAT4X4, TypeId::FLOAT4),
        Some(BinaryTypes {
            left: TypeId::FLOAT4X4,
            right: TypeId::FLOAT4,
            result: TypeId::FLOAT4,
        })
    );
    // A relational operator gives `bool`.
    let less = Operator::from(OperatorKind::Lt);
    assert_eq!(
        less.determine_binary_type(&ctx, TypeId::FLOAT, TypeId::FLOAT)
            .map(|types| types.result),
        Some(TypeId::BOOL)
    );
}
