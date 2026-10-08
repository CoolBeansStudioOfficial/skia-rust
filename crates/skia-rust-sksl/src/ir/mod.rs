// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir (the node shapes, `description` and `clone`). See
// docs/design/sksl.md §4 and "As implemented in S5" for the representation.

//! The `SkSL` IR: arenas of nodes named by typed ids.
//!
//! Every node lives in an [`IrPool`]. Expressions, statements and program elements are plain
//! data (`Expression { position, ty, kind }`), and their children are ids. Symbols (types,
//! variables, functions, fields) and symbol tables are pool nodes too. Skia's pointer
//! identity becomes id equality.
//!
//! Each Skia IR class is a payload struct of the same name (`BinaryExpression`, `ForStatement`,
//! …) wrapped in the [`ExpressionKind`], [`StatementKind`] or [`ProgramElementKind`] variant
//! that mirrors Skia's `Kind` enum. A class's `Convert`/`Make` factories are associated
//! functions of its payload struct that take `&mut Context` and return ids.

mod binary_expression;
mod block;
mod child_call;
mod constant_folder_stub;
pub mod constructor;
mod constructor_array;
mod constructor_array_cast;
mod constructor_compound;
mod constructor_compound_cast;
mod constructor_diagonal_matrix;
mod constructor_matrix_resize;
mod constructor_scalar_cast;
mod constructor_splat;
mod constructor_struct;
mod control_statements;
mod expression;
mod field_access;
mod field_symbol;
mod function_call;
mod function_declaration;
mod function_definition;
mod ids;
mod index_expression;
mod layout;
mod literal;
mod modifier_flags;
mod pool;
mod prefix_postfix;
mod program;
mod program_element;
mod s7b_shims;
mod setting;
mod simple_expressions;
mod simple_statements;
mod statement;
mod swizzle;
mod symbol;
mod symbol_table;
mod ternary_expression;
mod types;
mod var_declarations;
mod variable;
mod variable_reference;

pub use binary_expression::BinaryExpression;
pub use block::{Block, BlockKind};
pub use child_call::ChildCall;
pub use constructor::{
    ConstructorArray, ConstructorArrayCast, ConstructorCompound, ConstructorCompoundCast,
    ConstructorDiagonalMatrix, ConstructorMatrixResize, ConstructorScalarCast, ConstructorSplat,
    ConstructorStruct,
};
pub use control_statements::{
    DoStatement, ExpressionStatement, ForStatement, IfStatement, LoopUnrollInfo, SwitchCase,
    SwitchStatement,
};
pub use expression::{ComparisonResult, Expression, ExpressionKind};
pub use field_access::{FieldAccess, FieldAccessOwnerKind};
pub use field_symbol::FieldSymbol;
pub use function_call::FunctionCall;
pub use function_declaration::FunctionDeclaration;
pub use ids::{ElemId, ExprId, FieldId, FnId, StmtId, SymTabId, SymbolId, TypeId, VarId};
pub use index_expression::IndexExpression;
pub use layout::{Layout, LayoutFlags};
pub use literal::Literal;
pub use modifier_flags::{ModifierFlags, Modifiers};
pub use pool::IrPool;
pub use prefix_postfix::{PostfixExpression, PrefixExpression};
pub use program::{Program, ProgramInterface, RtFlip, Uniform, UniformInfo};
pub use program_element::{
    Extension, FunctionDefinition, FunctionPrototype, InterfaceBlock, ModifiersDeclaration,
    ProgramElement, ProgramElementKind, StructDefinition,
};
pub use setting::{CapsFlag, Setting};
pub use simple_expressions::{
    EmptyExpression, FunctionReference, MethodReference, Poison, TypeReference,
};
pub use simple_statements::{
    BreakStatement, ContinueStatement, DiscardStatement, Nop, ReturnStatement,
};
pub use statement::{Statement, StatementKind};
pub use swizzle::{ComponentArray, Swizzle, swizzle_component};
pub use symbol::SymbolKind;
pub use symbol_table::instantiate_symbol_ref;
pub use symbol_table::{
    SymbolTable, add_array_dimension, add_symbol, insert_new_parent, move_symbol_to, remove_symbol,
    rename_symbol, would_shadow_symbols_from,
};
pub use ternary_expression::TernaryExpression;
pub use types::{
    CoercionCost, Field, NumberKind, SpvDim, StructType, TextureAccess, Type, TypeClass, TypeKind,
    TypeRef,
};
pub use var_declarations::{GlobalVarDeclaration, VarDeclaration};
pub use variable::{DeclaringElement, Variable, VariableStorage};
pub use variable_reference::{VariableRefKind, VariableReference};

#[cfg(test)]
mod constructor_tests;
#[cfg(test)]
mod s7b_tests;
#[cfg(test)]
mod semantics_tests;
#[cfg(test)]
mod tests;
