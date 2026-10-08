// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// skia-rust: the IR arena that replaces Skia's `SkSL::Pool` and its owning pointers
// (`docs/design/sksl.md` §4.1 and "As implemented in S5").

//! [`IrPool`]: the arena that holds every IR node of one module or one program.
//!
//! Pools are layered. The built-in types (`SkSLBuiltinTypes.cpp`) form a fixed table below every
//! pool, so their ids are constants. A module's pool extends its parent module's pool and is
//! frozen into an [`Arc`] once the module is loaded. A program's pool extends the pool of the
//! module it is compiled against. Ids below a pool's base resolve in its parent chain; ids at or
//! above it are local. Only local nodes can be mutated: a frozen parent is shared by every program
//! and must never change (Skia: "built-in symbol tables must never be mutated").

use std::sync::Arc;

use super::{
    DeclaringElement, Expression, ExpressionKind, FieldSymbol, FunctionDeclaration, ProgramElement,
    Statement, StatementKind, SymbolTable, Type, TypeRef, Variable,
    ids::{ElemId, ExprId, FieldId, FnId, StmtId, SymTabId, TypeId, VarId},
};
use crate::builtin_types::{BUILTIN_TYPE_COUNT, BUILTIN_TYPES};
use crate::position::Position;

/// How many nodes of each kind a pool chain holds, or where a pool's local ids start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    expressions: u32,
    statements: u32,
    elements: u32,
    types: u32,
    variables: u32,
    functions: u32,
    fields: u32,
    symbol_tables: u32,
}

/// The IR of one module or one program, layered on a frozen parent (`docs/design/sksl.md` §4.1).
///
/// Nodes are never freed: a node that a rewrite replaces stays in the pool, unreachable, as
/// Skia's `Pool` keeps freed nodes until the program dies. Each node id has at most one live owner
/// slot (the field of its parent node that names it). Moving a subtree (Skia's `std::move` of a
/// `unique_ptr`) copies the id into the new owner and abandons the old one.
#[derive(Debug)]
pub struct IrPool {
    parent: Option<Arc<IrPool>>,
    base: Counts,
    expressions: Vec<Expression>,
    statements: Vec<Statement>,
    elements: Vec<ProgramElement>,
    types: Vec<Type>,
    variables: Vec<Variable>,
    functions: Vec<FunctionDeclaration>,
    fields: Vec<FieldSymbol>,
    symbol_tables: Vec<SymbolTable>,
}

/// Converts a local length into an id offset.
fn local_len(len: usize) -> u32 {
    u32::try_from(len).expect("IrPool: more than u32::MAX nodes of one kind")
}

/// Generates the accessors of one node arena: `get`, `get_mut`, `add`, `next id` and `is_local`.
macro_rules! arena {
    (
        $field:ident, $Id:ident, $Node:ty, $what:literal,
        $get_vis:vis $get:ident, $get_mut:ident, $add:ident, $next:ident, $is_local:ident
    ) => {
        impl IrPool {
            #[doc = concat!("The ", $what, " named by `id`, wherever it lives in the pool chain.")]
            ///
            /// # Panics
            ///
            /// If `id` was not allocated in this pool chain.
            #[must_use]
            $get_vis fn $get(&self, id: $Id) -> &$Node {
                let mut pool = self;
                loop {
                    if id.0 >= pool.base.$field {
                        return &pool.$field[(id.0 - pool.base.$field) as usize];
                    }
                    pool = pool.parent.as_deref().unwrap_or_else(|| {
                        panic!(concat!("IrPool: ", $what, " id {:?} is not in this chain"), id)
                    });
                }
            }

            #[doc = concat!("The ", $what, " named by `id`, for rewriting in place.")]
            ///
            /// # Panics
            ///
            /// If `id` belongs to a frozen parent pool (a built-in module): shared IR is
            /// immutable.
            #[must_use]
            pub fn $get_mut(&mut self, id: $Id) -> &mut $Node {
                assert!(
                    id.0 >= self.base.$field,
                    concat!("IrPool: ", $what, " {:?} belongs to a frozen parent pool"),
                    id
                );
                &mut self.$field[(id.0 - self.base.$field) as usize]
            }

            #[doc = concat!("Allocates a new ", $what, " in this pool and returns its id.")]
            ///
            /// # Panics
            ///
            /// If the pool chain already holds `u32::MAX` nodes of this kind.
            pub fn $add(&mut self, node: $Node) -> $Id {
                let id = self.$next();
                self.$field.push(node);
                id
            }

            #[doc = concat!("The id the next `", stringify!($add), "` call will return.")]
            ///
            /// # Panics
            ///
            /// If the pool chain already holds `u32::MAX` nodes of this kind.
            #[must_use]
            pub fn $next(&self) -> $Id {
                $Id(self.base.$field + local_len(self.$field.len()))
            }

            #[doc = concat!("Whether `id` is a ", $what, " of this pool (and so mutable).")]
            #[must_use]
            pub fn $is_local(&self, id: $Id) -> bool {
                id.0 >= self.base.$field
            }
        }
    };
}

arena!(
    expressions,
    ExprId,
    Expression,
    "expression",
    pub expression,
    expression_mut,
    add_expression,
    next_expression_id,
    is_local_expression
);
arena!(
    statements,
    StmtId,
    Statement,
    "statement",
    pub statement,
    statement_mut,
    add_statement,
    next_statement_id,
    is_local_statement
);
arena!(
    elements,
    ElemId,
    ProgramElement,
    "program element",
    pub element,
    element_mut,
    add_element,
    next_element_id,
    is_local_element
);
arena!(
    variables,
    VarId,
    Variable,
    "variable",
    pub variable,
    variable_mut,
    add_variable,
    next_variable_id,
    is_local_variable
);
arena!(
    functions,
    FnId,
    FunctionDeclaration,
    "function declaration",
    pub function,
    function_mut,
    add_function,
    next_function_id,
    is_local_function
);
arena!(
    fields,
    FieldId,
    FieldSymbol,
    "field symbol",
    pub field_symbol,
    field_symbol_mut,
    add_field_symbol,
    next_field_symbol_id,
    is_local_field_symbol
);
arena!(
    symbol_tables,
    SymTabId,
    SymbolTable,
    "symbol table",
    pub symbol_table,
    symbol_table_mut,
    add_symbol_table,
    next_symbol_table_id,
    is_local_symbol_table
);
arena!(
    types,
    TypeId,
    Type,
    "type",
    type_in_pools,
    type_mut,
    add_type,
    next_type_id,
    is_local_type
);

impl Default for IrPool {
    fn default() -> Self {
        Self::new()
    }
}

impl IrPool {
    /// A root pool: no parent, directly above the built-in types. Programs and modules normally
    /// extend a module pool with [`IrPool::extend`]; tests and the first built-in module start
    /// here.
    #[must_use]
    pub fn new() -> Self {
        Self::with_base(Counts {
            types: BUILTIN_TYPE_COUNT,
            ..Counts::default()
        })
    }

    /// An empty pool whose local ids start at `base`.
    fn with_base(base: Counts) -> Self {
        Self {
            parent: None,
            base,
            expressions: Vec::new(),
            statements: Vec::new(),
            elements: Vec::new(),
            types: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            fields: Vec::new(),
            symbol_tables: Vec::new(),
        }
    }

    /// A pool layered on the frozen `parent`: every id of the parent chain stays valid, and new
    /// nodes get ids above the parent's.
    #[must_use]
    pub fn extend(parent: Arc<IrPool>) -> Self {
        let mut pool = Self::with_base(parent.end());
        pool.parent = Some(parent);
        pool
    }

    /// Freezes this pool so that modules and programs can extend it.
    #[must_use]
    pub fn freeze(self) -> Arc<IrPool> {
        Arc::new(self)
    }

    /// The frozen pool this one extends.
    #[must_use]
    pub fn parent(&self) -> Option<&Arc<IrPool>> {
        self.parent.as_ref()
    }

    /// One past the last id of each kind in this pool chain.
    fn end(&self) -> Counts {
        Counts {
            expressions: self.base.expressions + local_len(self.expressions.len()),
            statements: self.base.statements + local_len(self.statements.len()),
            elements: self.base.elements + local_len(self.elements.len()),
            types: self.base.types + local_len(self.types.len()),
            variables: self.base.variables + local_len(self.variables.len()),
            functions: self.base.functions + local_len(self.functions.len()),
            fields: self.base.fields + local_len(self.fields.len()),
            symbol_tables: self.base.symbol_tables + local_len(self.symbol_tables.len()),
        }
    }

    /// The type named by `id`: a built-in type or one allocated in this pool chain.
    ///
    /// # Panics
    ///
    /// If `id` was not allocated in this pool chain.
    #[must_use]
    pub fn type_node(&self, id: TypeId) -> &Type {
        if id.0 < BUILTIN_TYPE_COUNT {
            return &BUILTIN_TYPES[id.0 as usize];
        }
        self.type_in_pools(id)
    }

    /// The type named by `id`, as a [`TypeRef`] that answers Skia's `Type` queries.
    #[must_use]
    pub fn ty(&self, id: TypeId) -> TypeRef<'_> {
        TypeRef::new(self, id)
    }

    /// Overwrites the expression at `id` (Skia: `expr = std::move(replacement)` on the owning
    /// `unique_ptr`). Every owner of `id` now sees `replacement`.
    pub fn replace_expression(&mut self, id: ExprId, replacement: Expression) {
        *self.expression_mut(id) = replacement;
    }

    /// Overwrites the statement at `id` (Skia: `stmt = std::move(replacement)`).
    pub fn replace_statement(&mut self, id: StmtId, replacement: Statement) {
        *self.statement_mut(id) = replacement;
    }

    /// Moves the expression at `id` to a fresh id and returns it, leaving a poison placeholder at
    /// `id`. Use it when a slot must receive a new node that wraps the old one (Skia moves the
    /// `unique_ptr` out of the slot into the wrapper, then stores the wrapper in the slot).
    pub fn relocate_expression(&mut self, id: ExprId) -> ExprId {
        let placeholder = Expression::new(
            Position::default(),
            TypeId::POISON,
            ExpressionKind::Poison(super::Poison),
        );
        let node = std::mem::replace(self.expression_mut(id), placeholder);
        self.add_expression(node)
    }

    /// Moves the statement at `id` to a fresh id and returns it, leaving a `Nop` at `id` (see
    /// [`IrPool::relocate_expression`]). The inliner's `inlinedBody->children().push_back(
    /// std::move(*enclosingStmt)); *enclosingStmt = std::move(inlinedBody);` becomes
    /// `let moved = pool.relocate_statement(slot); /* push moved */ pool.replace_statement(slot,
    /// block)`.
    pub fn relocate_statement(&mut self, id: StmtId) -> StmtId {
        let placeholder = Statement::new(Position::default(), StatementKind::Nop(super::Nop));
        let node = std::mem::replace(self.statement_mut(id), placeholder);
        let moved = self.add_statement(node);
        self.retarget_declaration(id, moved);
        moved
    }

    /// Skia moves a `VarDeclaration` object with its `unique_ptr`, so `Variable::declaringElement`
    /// keeps naming that object wherever it goes. An id-based node that moves to a new id must
    /// point its variable at the new id, or the variable's declaration is left at a slot that
    /// no longer holds it. `to` is the id the declaration now has; `from` is the id it left.
    // Port of: the object identity that `std::unique_ptr` moves keep (SkSLVariable.h `fDeclaringElement`).
    fn retarget_declaration(&mut self, from: StmtId, to: StmtId) {
        let StatementKind::VarDeclaration(decl) = &self.statement(to).kind else {
            return;
        };
        let var = decl.var;
        // A declaration in a frozen parent cannot move, so its variable is not local either.
        if !self.is_local_variable(var) {
            return;
        }
        let variable = self.variable_mut(var);
        if variable.declaring_element == Some(DeclaringElement::VarDeclaration(from)) {
            variable.declaring_element = Some(DeclaringElement::VarDeclaration(to));
        }
    }

    /// `Expression::clone()`: a deep copy of the expression at `id`, at the same position.
    /// Children keep their own positions. Symbols, types and functions are shared, not copied,
    /// as in Skia. The copy is allocated in this pool even when `id` lives in a parent (the
    /// inliner copies module function bodies into programs this way).
    pub fn clone_expression(&mut self, id: ExprId) -> ExprId {
        let position = self.expression(id).position;
        self.clone_expression_at(id, position)
    }

    /// `Expression::clone(Position)`: a deep copy of the expression at `id`, moved to
    /// `position`.
    // Port of: src/sksl/ir/SkSL*.h `clone(Position)` overrides (chrome/m156). Every override
    // copies the node with its type and non-child data, at `pos`, and clones each child at its
    // own position; `FunctionCall` keeps its `stablePointer`.
    pub fn clone_expression_at(&mut self, id: ExprId, position: Position) -> ExprId {
        let mut node = self.expression(id).clone();
        node.position = position;
        node.kind
            .for_each_child_mut(&mut |child| *child = self.clone_expression(*child));
        self.add_expression(node)
    }

    /// Moves the node at `source` into the slot `slot` (Skia: `slot = std::move(source)`, where
    /// `source` is a child of the node in `slot`). The node keeps its own position; `source`
    /// is abandoned.
    pub fn move_expression_into(&mut self, slot: ExprId, source: ExprId) {
        let node = self.expression(source).clone();
        self.replace_expression(slot, node);
    }

    /// Moves the statement at `source` into the slot `slot` (see
    /// [`IrPool::move_expression_into`]).
    pub fn move_statement_into(&mut self, slot: StmtId, source: StmtId) {
        let node = self.statement(source).clone();
        self.replace_statement(slot, node);
        self.retarget_declaration(source, slot);
    }

    /// `ExpressionArray::clone()`: clones every element.
    pub fn clone_expression_array(&mut self, array: &[ExprId]) -> Vec<ExprId> {
        array.iter().map(|&e| self.clone_expression(e)).collect()
    }
}
