// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the IR core: pools and layering, `description()` against Skia goldens, `clone()`,
//! and the visitor/writer traversals. The IR is built by hand (the parser comes later), and the
//! expected texts are copied from `tests/sksl` goldens of the pinned tree.

// The hand-built programs use the single-letter names of the goldens they reproduce.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::items_after_statements
)]

use std::sync::Arc;

use super::*;
use crate::analysis::{ProgramVisitor, walk_expression};
use crate::context::Context;
use crate::error_reporter::ErrorReporter;
use crate::modules::ModuleType;
use crate::operator::{Operator, OperatorKind, OperatorPrecedence};
use crate::position::{ForLoopPositions, Position};
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings};
use crate::transform::{ProgramWriter, walk_expression_mut};

/// Builds IR by hand, as the converters of later tasks will.
struct Builder {
    pool: IrPool,
}

impl Builder {
    fn new() -> Self {
        Self {
            pool: IrPool::new(),
        }
    }

    fn expr(&mut self, ty: TypeId, kind: ExpressionKind) -> ExprId {
        self.pool
            .add_expression(Expression::new(Position::default(), ty, kind))
    }

    fn stmt(&mut self, kind: StatementKind) -> StmtId {
        self.pool
            .add_statement(Statement::new(Position::default(), kind))
    }

    fn elem(&mut self, kind: ProgramElementKind) -> ElemId {
        self.pool
            .add_element(ProgramElement::new(Position::default(), kind))
    }

    fn array_type(&mut self, component: TypeId, count: i32) -> TypeId {
        let ty = self.pool.ty(component);
        let name = ty.array_name(count);
        let abbrev = ty.abbreviated_name;
        self.pool
            .add_type(Type::new_array_type(name, abbrev, component, count, false))
    }

    fn var(
        &mut self,
        name: &str,
        ty: TypeId,
        flags: ModifierFlags,
        storage: VariableStorage,
    ) -> VarId {
        self.pool.add_variable(Variable::new(
            Position::default(),
            Position::default(),
            flags,
            name,
            ty,
            false,
            storage,
        ))
    }

    fn param(&mut self, name: &str, ty: TypeId) -> VarId {
        self.var(name, ty, ModifierFlags::empty(), VariableStorage::Parameter)
    }

    fn func(&mut self, name: &str, ret: TypeId, params: Vec<VarId>, flags: ModifierFlags) -> FnId {
        self.pool.add_function(FunctionDeclaration {
            position: Position::default(),
            name: name.into(),
            definition: None,
            next_overload: None,
            parameters: params,
            return_type: ret,
            modifier_flags: flags,
            intrinsic_kind: None,
            module_type: ModuleType::Program,
            is_main: name == "main",
            has_main_coords_parameter: false,
            has_main_input_color_parameter: false,
            has_main_dest_color_parameter: false,
        })
    }

    fn vref(&mut self, var: VarId) -> ExprId {
        let ty = self.pool.variable(var).ty;
        self.expr(
            ty,
            ExpressionKind::VariableReference(VariableReference {
                variable: var,
                ref_kind: VariableRefKind::Read,
            }),
        )
    }

    fn int(&mut self, value: i64) -> ExprId {
        #[allow(clippy::cast_precision_loss)] // Small test values.
        let value = value as f64;
        self.expr(TypeId::INT, ExpressionKind::Literal(Literal { value }))
    }

    fn boolean(&mut self, value: bool) -> ExprId {
        let value = if value { 1.0 } else { 0.0 };
        self.expr(TypeId::BOOL, ExpressionKind::Literal(Literal { value }))
    }

    fn float(&mut self, value: f64) -> ExprId {
        self.expr(TypeId::FLOAT, ExpressionKind::Literal(Literal { value }))
    }

    fn binary(&mut self, left: ExprId, op: OperatorKind, right: ExprId, ty: TypeId) -> ExprId {
        self.expr(
            ty,
            ExpressionKind::Binary(BinaryExpression {
                left,
                operator: Operator::from(op),
                right,
            }),
        )
    }

    fn call(&mut self, function: FnId, arguments: Vec<ExprId>) -> ExprId {
        let ty = self.pool.function(function).return_type;
        let stable_pointer = self.pool.next_expression_id();
        self.expr(
            ty,
            ExpressionKind::FunctionCall(FunctionCall {
                function,
                arguments,
                stable_pointer,
            }),
        )
    }

    fn ternary(&mut self, test: ExprId, if_true: ExprId, if_false: ExprId) -> ExprId {
        let ty = self.pool.expression(if_true).ty;
        self.expr(
            ty,
            ExpressionKind::Ternary(TernaryExpression {
                test,
                if_true,
                if_false,
            }),
        )
    }

    fn prefix(&mut self, op: OperatorKind, operand: ExprId) -> ExprId {
        let ty = self.pool.expression(operand).ty;
        self.expr(
            ty,
            ExpressionKind::Prefix(PrefixExpression {
                operator: Operator::from(op),
                operand,
            }),
        )
    }

    fn ret(&mut self, e: Option<ExprId>) -> StmtId {
        self.stmt(StatementKind::Return(ReturnStatement { expression: e }))
    }

    fn expr_stmt(&mut self, e: ExprId) -> StmtId {
        self.stmt(StatementKind::Expression(ExpressionStatement {
            expression: e,
        }))
    }

    fn block(&mut self, children: Vec<StmtId>) -> StmtId {
        self.stmt(StatementKind::Block(Block {
            children,
            block_kind: BlockKind::BracedScope,
            symbol_table: None,
        }))
    }

    /// `type name[array_size] = value;` for a local (or, with `global`, a uniform).
    fn var_decl(
        &mut self,
        var: VarId,
        base_type: TypeId,
        array_size: i32,
        value: Option<ExprId>,
    ) -> StmtId {
        let id = self.stmt(StatementKind::VarDeclaration(VarDeclaration {
            var,
            base_type,
            array_size,
            value,
        }));
        self.pool.variable_mut(var).set_var_declaration(id);
        id
    }

    fn uniform(&mut self, name: &str, ty: TypeId) -> (VarId, ElemId) {
        let var = self.var(name, ty, ModifierFlags::UNIFORM, VariableStorage::Global);
        let decl = self.var_decl(var, ty, 0, None);
        let elem = self.elem(ProgramElementKind::GlobalVar(GlobalVarDeclaration {
            declaration: decl,
        }));
        self.pool.variable_mut(var).set_global_var_declaration(elem);
        (var, elem)
    }

    fn define(&mut self, function: FnId, body: Vec<StmtId>) -> ElemId {
        let body = self.block(body);
        let elem = self.elem(ProgramElementKind::Function(FunctionDefinition {
            declaration: function,
            body,
        }));
        self.pool.function_mut(function).set_definition(elem);
        elem
    }

    fn describe(&self, elements: &[ElemId]) -> String {
        elements
            .iter()
            .map(|&e| self.pool.element_description(e))
            .collect()
    }
}

/// Removes whitespace the way `sksl-minify` does for these programs: a run of whitespace
/// survives (as one space) only between two identifier characters.
fn strip_whitespace(text: &str) -> String {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    let mut out = String::new();
    let mut pending_space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && out.chars().last().is_some_and(ident) && ident(c) {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }
    out
}

/// `tests/sksl/folding/ArraySizeFolding.minified.sksl`, built by hand.
fn array_size_folding(b: &mut Builder) -> Vec<ElemId> {
    use OperatorKind as K;
    let mut elements = Vec::new();
    let (_, e) = b.uniform("colorRed", TypeId::HALF4);
    elements.push(e);
    let color_red = b.pool.find_var("colorRed");
    let (_, e) = b.uniform("colorGreen", TypeId::HALF4);
    elements.push(e);
    let color_green = b.pool.find_var("colorGreen");

    // bool a(int[2] b) { return true; }
    let int2 = b.array_type(TypeId::INT, 2);
    let p = b.param("b", int2);
    let fa = b.func("a", TypeId::BOOL, vec![p], ModifierFlags::empty());
    let t = b.boolean(true);
    let r = b.ret(Some(t));
    elements.push(b.define(fa, vec![r]));

    // bool b() { int i[2]; … return ((((a(i) && a(j)) && a(k)) && a(l)) && a(m)) && a(n); }
    let fb = b.func("b", TypeId::BOOL, vec![], ModifierFlags::empty());
    let mut body = Vec::new();
    let mut calls = Vec::new();
    for name in ["i", "j", "k", "l", "m", "n"] {
        let v = b.var(name, int2, ModifierFlags::empty(), VariableStorage::Local);
        body.push(b.var_decl(v, TypeId::INT, 2, None));
        let r = b.vref(v);
        calls.push(b.call(fa, vec![r]));
    }
    let mut chain = calls[0];
    for &c in &calls[1..] {
        chain = b.binary(chain, K::LogicalAnd, c, TypeId::BOOL);
    }
    body.push(b.ret(Some(chain)));
    elements.push(b.define(fb, body));

    // bool c(float[3] d) { return true; }
    let float3 = b.array_type(TypeId::FLOAT, 3);
    let p = b.param("d", float3);
    let fc = b.func("c", TypeId::BOOL, vec![p], ModifierFlags::empty());
    let t = b.boolean(true);
    let r = b.ret(Some(t));
    elements.push(b.define(fc, vec![r]));

    // bool d(float[3] e, float[3] f) { return c(e) && c(f); }
    let pe = b.param("e", float3);
    let pf = b.param("f", float3);
    let fd = b.func("d", TypeId::BOOL, vec![pe, pf], ModifierFlags::empty());
    let re = b.vref(pe);
    let ce = b.call(fc, vec![re]);
    let rf = b.vref(pf);
    let cf = b.call(fc, vec![rf]);
    let and = b.binary(ce, K::LogicalAnd, cf, TypeId::BOOL);
    let r = b.ret(Some(and));
    elements.push(b.define(fd, vec![r]));

    // half4 main(float2 e) { float h[3]; float i[3]; return b() && d(h, i) ? colorGreen : colorRed; }
    let coords = b.param("e", TypeId::FLOAT2);
    let fmain = b.func("main", TypeId::HALF4, vec![coords], ModifierFlags::empty());
    let h = b.var("h", float3, ModifierFlags::empty(), VariableStorage::Local);
    let i = b.var("i", float3, ModifierFlags::empty(), VariableStorage::Local);
    let dh = b.var_decl(h, TypeId::FLOAT, 3, None);
    let di = b.var_decl(i, TypeId::FLOAT, 3, None);
    let call_b = b.call(fb, vec![]);
    let rh = b.vref(h);
    let ri = b.vref(i);
    let call_d = b.call(fd, vec![rh, ri]);
    let test = b.binary(call_b, K::LogicalAnd, call_d, TypeId::BOOL);
    let green = b.vref(color_green);
    let red = b.vref(color_red);
    let ternary = b.ternary(test, green, red);
    let r = b.ret(Some(ternary));
    elements.push(b.define(fmain, vec![dh, di, r]));
    elements
}

impl IrPool {
    /// Test helper: the local variable named `name`.
    fn find_var(&self, name: &str) -> VarId {
        (0..self.next_variable_id().index())
            .map(VarId)
            .find(|&v| &*self.variable(v).name == name)
            .expect("variable")
    }
}

#[test]
fn description_matches_array_size_folding_golden() {
    // Expected: tests/sksl/folding/ArraySizeFolding.minified.sksl.
    let mut b = Builder::new();
    let elements = array_size_folding(&mut b);
    assert_eq!(
        strip_whitespace(&b.describe(&elements)),
        "uniform half4 colorRed;uniform half4 colorGreen;bool a(int[2]b){return true;}bool b(){\
         int i[2];int j[2];int k[2];int l[2];int m[2];int n[2];\
         return((((a(i)&&a(j))&&a(k))&&a(l))&&a(m))&&a(n);}bool c(float[3]d){return true;}\
         bool d(float[3]e,float[3]f){return c(e)&&c(f);}half4 main(float2 e){float h[3];\
         float i[3];return b()&&d(h,i)?colorGreen:colorRed;}"
    );
    // The unstripped text keeps Skia's spacing and block layout.
    assert_eq!(
        b.pool.element_description(elements[2]),
        "bool a(int[2] b) {\nreturn true;\n}\n"
    );
}

#[test]
fn description_matches_ternary_folding_golden() {
    // Expected: tests/sksl/folding/TernaryFolding.minified.sksl.
    use OperatorKind as K;
    let mut b = Builder::new();
    let mut elements = Vec::new();
    let (red, e) = b.uniform("colorRed", TypeId::HALF4);
    elements.push(e);
    let (green, e) = b.uniform("colorGreen", TypeId::HALF4);
    elements.push(e);

    // bool a(out bool b) { b = true; return false; }
    let pb = b.var(
        "b",
        TypeId::BOOL,
        ModifierFlags::OUT,
        VariableStorage::Parameter,
    );
    let fa = b.func("a", TypeId::BOOL, vec![pb], ModifierFlags::empty());
    let lhs = b.vref(pb);
    let t = b.boolean(true);
    let assign = b.binary(lhs, K::Eq, t, TypeId::BOOL);
    let s1 = b.expr_stmt(assign);
    let f = b.boolean(false);
    let s2 = b.ret(Some(f));
    elements.push(b.define(fa, vec![s1, s2]));

    // half4 main(float2 b) { … bool g = (a(f), true); return (c && f) && g ? d : e; }
    let coords = b.param("b", TypeId::FLOAT2);
    let fmain = b.func("main", TypeId::HALF4, vec![coords], ModifierFlags::empty());
    let local = |b: &mut Builder, name: &str, ty: TypeId| {
        b.var(name, ty, ModifierFlags::empty(), VariableStorage::Local)
    };
    let c = local(&mut b, "c", TypeId::BOOL);
    let d = local(&mut b, "d", TypeId::HALF4);
    let e = local(&mut b, "e", TypeId::HALF4);
    let fv = local(&mut b, "f", TypeId::BOOL);
    let g = local(&mut b, "g", TypeId::BOOL);
    let v = b.boolean(true);
    let dc = b.var_decl(c, TypeId::BOOL, 0, Some(v));
    let v = b.vref(green);
    let dd = b.var_decl(d, TypeId::HALF4, 0, Some(v));
    let v = b.vref(red);
    let de = b.var_decl(e, TypeId::HALF4, 0, Some(v));
    let v = b.boolean(false);
    let df = b.var_decl(fv, TypeId::BOOL, 0, Some(v));
    let rf = b.vref(fv);
    let call = b.call(fa, vec![rf]);
    let t = b.boolean(true);
    let comma = b.binary(call, K::Comma, t, TypeId::BOOL);
    let dg = b.var_decl(g, TypeId::BOOL, 0, Some(comma));
    let rc = b.vref(c);
    let rf = b.vref(fv);
    let cf = b.binary(rc, K::LogicalAnd, rf, TypeId::BOOL);
    let rg = b.vref(g);
    let test = b.binary(cf, K::LogicalAnd, rg, TypeId::BOOL);
    let rd = b.vref(d);
    let re = b.vref(e);
    let ternary = b.ternary(test, rd, re);
    let r = b.ret(Some(ternary));
    elements.push(b.define(fmain, vec![dc, dd, de, df, dg, r]));

    assert_eq!(
        strip_whitespace(&b.describe(&elements)),
        "uniform half4 colorRed;uniform half4 colorGreen;bool a(out bool b){b=true;return false;}\
         half4 main(float2 b){bool c=true;half4 d=colorGreen;half4 e=colorRed;bool f=false;\
         bool g=(a(f),true);return(c&&f)&&g?d:e;}"
    );
}

#[test]
fn description_matches_child_effect_simple_golden() {
    // Expected: tests/sksl/runtime/ChildEffectSimple.minified.sksl.
    let mut b = Builder::new();
    let mut elements = Vec::new();
    let (green, e) = b.uniform("shaderGreen", TypeId::SHADER);
    elements.push(e);
    let (red, e) = b.uniform("shaderRed", TypeId::SHADER);
    elements.push(e);
    let coords = b.param("a", TypeId::FLOAT2);
    let fmain = b.func("main", TypeId::HALF4, vec![coords], ModifierFlags::empty());
    let a1 = b.vref(coords);
    let eval_green = b.expr(
        TypeId::HALF4,
        ExpressionKind::ChildCall(ChildCall {
            child: green,
            arguments: vec![a1],
        }),
    );
    let a2 = b.vref(coords);
    let eval_red = b.expr(
        TypeId::HALF4,
        ExpressionKind::ChildCall(ChildCall {
            child: red,
            arguments: vec![a2],
        }),
    );
    use swizzle_component as c;
    let swizzled = b.expr(
        TypeId::HALF4,
        ExpressionKind::Swizzle(Swizzle {
            base: eval_red,
            components: ComponentArray::from_slice(&[c::X, c::X, c::X, c::W]),
        }),
    );
    let product = b.binary(eval_green, OperatorKind::Star, swizzled, TypeId::HALF4);
    let r = b.ret(Some(product));
    elements.push(b.define(fmain, vec![r]));
    assert_eq!(
        strip_whitespace(&b.describe(&elements)),
        "uniform shader shaderGreen;uniform shader shaderRed;half4 main(float2 a){\
         return shaderGreen.eval(a)*shaderRed.eval(a).xxxw;}"
    );
}

#[test]
fn for_statement_description_matches_array_indexing_golden() {
    // Expected: the loop of `float c()` in tests/sksl/runtime/ArrayIndexing.minified.sksl:
    // `for(int e=0;e<4;++e)d*=u3[e<2?0:e];`
    use OperatorKind as K;
    let mut b = Builder::new();
    let float4 = b.array_type(TypeId::FLOAT, 4);
    let u3 = b.var(
        "u3",
        float4,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    let d = b.var(
        "d",
        TypeId::FLOAT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let e = b.var(
        "e",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let zero = b.int(0);
    let init = b.var_decl(e, TypeId::INT, 0, Some(zero));
    let re = b.vref(e);
    let four = b.int(4);
    let test = b.binary(re, K::Lt, four, TypeId::BOOL);
    let re = b.vref(e);
    let next = b.prefix(K::PlusPlus, re);
    let re = b.vref(e);
    let two = b.int(2);
    let lt = b.binary(re, K::Lt, two, TypeId::BOOL);
    let zero = b.int(0);
    let re = b.vref(e);
    let index = b.ternary(lt, zero, re);
    let base = b.vref(u3);
    let indexed = b.expr(
        TypeId::FLOAT,
        ExpressionKind::Index(IndexExpression { base, index }),
    );
    let rd = b.vref(d);
    let mul = b.binary(rd, K::StarEq, indexed, TypeId::FLOAT);
    let body = b.expr_stmt(mul);
    let for_stmt = b.stmt(StatementKind::For(ForStatement {
        for_loop_positions: ForLoopPositions::default(),
        symbol_table: None,
        initializer: Some(init),
        test: Some(test),
        next: Some(next),
        statement: body,
        unroll_info: None,
    }));
    let text = b.pool.statement_description(for_stmt);
    assert_eq!(text, "for (int e = 0; e < 4; ++e) d *= u3[e < 2 ? 0 : e];");
    assert_eq!(
        strip_whitespace(&text),
        "for(int e=0;e<4;++e)d*=u3[e<2?0:e];"
    );
}

#[test]
fn function_declaration_descriptions_match_error_goldens() {
    let mut b = Builder::new();
    // tests/sksl/errors/Ossfuzz38140.glsl
    let src = b.param("src", TypeId::HALF4);
    let dst = b.param("dst", TypeId::HALF4);
    let f = b.func(
        "blend_src_over",
        TypeId::HALF4,
        vec![src, dst],
        ModifierFlags::empty(),
    );
    assert_eq!(
        b.pool.function(f).description(&b.pool),
        "half4 blend_src_over(half4 src, half4 dst)"
    );
    b.pool.function_mut(f).modifier_flags = ModifierFlags::PURE;
    assert_eq!(
        b.pool.function(f).description(&b.pool),
        "$pure half4 blend_src_over(half4 src, half4 dst)"
    );
    // tests/sksl/errors/IntrinsicRedefinition.glsl-style overload messages
    // ("functions 'int cos(out half3 a)' and '$pure $genHType cos($genHType angle)' …").
    let a = b.var(
        "a",
        TypeId::HALF3,
        ModifierFlags::OUT,
        VariableStorage::Parameter,
    );
    let cos = b.func("cos", TypeId::INT, vec![a], ModifierFlags::empty());
    assert_eq!(
        b.pool.function(cos).description(&b.pool),
        "int cos(out half3 a)"
    );
    let angle = b.param("angle", TypeId::GEN_HTYPE);
    let builtin = b.func("cos", TypeId::GEN_HTYPE, vec![angle], ModifierFlags::PURE);
    assert_eq!(
        b.pool.function(builtin).description(&b.pool),
        "$pure $genHType cos($genHType angle)"
    );
    let func = b.func("func", TypeId::VOID, vec![], ModifierFlags::empty());
    assert_eq!(b.pool.function(func).description(&b.pool), "void func()");
}

#[test]
fn expression_descriptions_follow_operator_precedence() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let y = b.var(
        "y",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    // -(x + y) * 2
    let rx = b.vref(x);
    let ry = b.vref(y);
    let sum = b.binary(rx, K::Plus, ry, TypeId::INT);
    let neg = b.prefix(K::Minus, sum);
    let two = b.int(2);
    let product = b.binary(neg, K::Star, two, TypeId::INT);
    assert_eq!(b.pool.expression_description(product), "-(x + y) * 2");
    // x - (y - 1): the right operand of the same precedence is parenthesized too.
    let rx = b.vref(x);
    let ry = b.vref(y);
    let one = b.int(1);
    let inner = b.binary(ry, K::Minus, one, TypeId::INT);
    let outer = b.binary(rx, K::Minus, inner, TypeId::INT);
    assert_eq!(b.pool.expression_description(outer), "x - (y - 1)");
    // (x++).  Postfix inside postfix-precedence parents gets parentheses: (x++)[…] style.
    let rx = b.vref(x);
    let post = b.expr(
        TypeId::INT,
        ExpressionKind::Postfix(PostfixExpression {
            operand: rx,
            operator: Operator::from(K::PlusPlus),
        }),
    );
    assert_eq!(b.pool.expression_description(post), "x++");
    assert_eq!(
        b.pool
            .expression_description_with(post, OperatorPrecedence::Postfix),
        "(x++)"
    );
    // Literals: integers, booleans and floats in `skstd::to_string` form.
    let f = b.float(0.5);
    assert_eq!(b.pool.expression_description(f), "0.5");
    let f = b.float(1.0);
    assert_eq!(b.pool.expression_description(f), "1.0");
    let f = b.float(1e10);
    assert_eq!(b.pool.expression_description(f), "1e+10");
    let lit = b.expr(
        TypeId::FLOAT_LITERAL,
        ExpressionKind::Literal(Literal { value: 2.0 }),
    );
    assert_eq!(b.pool.expression_description(lit), "2.0");
    // Constructors print their type and their arguments at sequence precedence.
    let one = b.float(1.0);
    let comma_l = b.int(3);
    let comma_r = b.int(4);
    let comma = b.binary(comma_l, K::Comma, comma_r, TypeId::INT);
    let ctor = b.expr(
        TypeId::FLOAT2,
        ExpressionKind::ConstructorCompound(ConstructorCompound {
            arguments: vec![one, comma],
        }),
    );
    assert_eq!(b.pool.expression_description(ctor), "float2(1.0, (3, 4))");
    let splat_arg = b.int(0);
    let splat = b.expr(
        TypeId::HALF4,
        ExpressionKind::ConstructorSplat(ConstructorSplat {
            argument: splat_arg,
        }),
    );
    assert_eq!(b.pool.expression_description(splat), "half4(0)");
    // Leaves.
    let setting = b.expr(
        TypeId::BOOL,
        ExpressionKind::Setting(Setting {
            caps: CapsFlag::IntegerSupport,
        }),
    );
    assert_eq!(
        b.pool.expression_description(setting),
        "sk_Caps.integerSupport"
    );
    let poison = b.expr(TypeId::POISON, ExpressionKind::Poison(Poison));
    assert_eq!(b.pool.expression_description(poison), "<POISON>");
    let empty = b.expr(TypeId::VOID, ExpressionKind::Empty(EmptyExpression));
    assert_eq!(b.pool.expression_description(empty), "false");
    let type_ref = b.expr(
        TypeId::INVALID,
        ExpressionKind::TypeReference(TypeReference {
            value: TypeId::HALF3,
        }),
    );
    assert_eq!(b.pool.expression_description(type_ref), "half3");
}

#[test]
fn statement_descriptions() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let rx = b.vref(x);
    let zero = b.int(0);
    let test = b.binary(rx, K::EqEq, zero, TypeId::BOOL);
    let brk = b.stmt(StatementKind::Break(BreakStatement));
    let cont = b.stmt(StatementKind::Continue(ContinueStatement));
    let if_stmt = b.stmt(StatementKind::If(IfStatement {
        test,
        if_true: brk,
        if_false: Some(cont),
    }));
    assert_eq!(
        b.pool.statement_description(if_stmt),
        "if (x == 0) break; else continue;"
    );
    // An empty unbraced block still prints braces; a non-empty one prints only its lines.
    let nop = b.stmt(StatementKind::Nop(Nop));
    let empty = b.stmt(StatementKind::Block(Block {
        children: vec![nop],
        block_kind: BlockKind::UnbracedBlock,
        symbol_table: None,
    }));
    assert_eq!(b.pool.statement_description(empty), "{\n;\n}\n");
    let discard = b.stmt(StatementKind::Discard(DiscardStatement));
    let unbraced = b.stmt(StatementKind::Block(Block {
        children: vec![discard],
        block_kind: BlockKind::UnbracedBlock,
        symbol_table: None,
    }));
    assert_eq!(b.pool.statement_description(unbraced), "\ndiscard;\n");
    // switch (x) { case 1: break; default: … }
    let one_case = b.stmt(StatementKind::SwitchCase(SwitchCase {
        is_default: false,
        value: 1,
        statement: brk,
    }));
    let ret = b.ret(None);
    let default_case = b.stmt(StatementKind::SwitchCase(SwitchCase {
        is_default: true,
        value: 0,
        statement: ret,
    }));
    let cases = b.block(vec![one_case, default_case]);
    let value = b.vref(x);
    let switch = b.stmt(StatementKind::Switch(SwitchStatement {
        value,
        case_block: cases,
    }));
    assert_eq!(
        b.pool.statement_description(switch),
        "switch (x) {\ncase 1: \nbreak;\ndefault: \nreturn;\n}\n"
    );
    let StatementKind::Switch(sw) = &b.pool.statement(switch).kind else {
        unreachable!()
    };
    assert_eq!(sw.cases(&b.pool), [one_case, default_case]);
    // do x++; while (x < 3);
    let rx = b.vref(x);
    let three = b.int(3);
    let lt = b.binary(rx, K::Lt, three, TypeId::BOOL);
    let rx = b.vref(x);
    let inc = b.expr(
        TypeId::INT,
        ExpressionKind::Postfix(PostfixExpression {
            operand: rx,
            operator: Operator::from(K::PlusPlus),
        }),
    );
    let body = b.expr_stmt(inc);
    let do_stmt = b.stmt(StatementKind::Do(DoStatement {
        statement: body,
        test: lt,
    }));
    assert_eq!(
        b.pool.statement_description(do_stmt),
        "do x++; while (x < 3);"
    );
    // for (;;) with every clause empty.
    let body = b.stmt(StatementKind::Nop(Nop));
    let for_stmt = b.stmt(StatementKind::For(ForStatement {
        for_loop_positions: ForLoopPositions::default(),
        symbol_table: None,
        initializer: None,
        test: None,
        next: None,
        statement: body,
        unroll_info: None,
    }));
    assert_eq!(b.pool.statement_description(for_stmt), "for (; ; ) ;");
}

#[test]
fn element_descriptions() {
    let mut b = Builder::new();
    // struct S { float x; int[2] y; };
    let int2 = b.array_type(TypeId::INT, 2);
    let field = |name: &str, ty: TypeId| Field {
        position: Position::default(),
        layout: Layout::new(),
        modifier_flags: ModifierFlags::empty(),
        name: name.into(),
        ty,
    };
    let data = StructType::new(
        &b.pool,
        vec![field("x", TypeId::FLOAT), field("y", int2)],
        1,
        false,
        false,
    );
    let s = b
        .pool
        .add_type(Type::new_struct_type(Position::default(), "S".into(), data));
    let def = b.elem(ProgramElementKind::StructDefinition(StructDefinition {
        ty: s,
    }));
    assert_eq!(
        b.pool.element_description(def),
        "struct S {  float x;  int[2] y; };"
    );
    // layout(binding = 1) uniform Block { half4 color; } block[2];
    let data = StructType::new(&b.pool, vec![field("color", TypeId::HALF4)], 1, true, false);
    let block_type = b.pool.add_type(Type::new_struct_type(
        Position::default(),
        "Block".into(),
        data,
    ));
    let array = b.array_type(block_type, 2);
    let var = b.var(
        "block",
        array,
        ModifierFlags::UNIFORM,
        VariableStorage::Global,
    );
    {
        let v = b.pool.variable_mut(var);
        v.extended = true;
        v.layout.binding = 1;
    }
    let ib = b.elem(ProgramElementKind::InterfaceBlock(InterfaceBlock { var }));
    b.pool.variable_mut(var).set_interface_block(ib);
    assert_eq!(
        b.pool.element_description(ib),
        "layout (binding = 1)uniform Block {\nhalf4 color;\n} block[2];"
    );
    let ext = b.elem(ProgramElementKind::Extension(Extension {
        name: "GL_EXT_foo".into(),
    }));
    assert_eq!(
        b.pool.element_description(ext),
        "#extension GL_EXT_foo : enable"
    );
    let mut layout = Layout::new();
    layout.flags = LayoutFlags::BLEND_SUPPORT_ALL_EQUATIONS;
    let mods = b.elem(ProgramElementKind::Modifiers(ModifiersDeclaration {
        layout,
        flags: ModifierFlags::OUT,
    }));
    assert_eq!(
        b.pool.element_description(mods),
        "layout (blend_support_all_equations) out;"
    );
    let f = b.func("f", TypeId::VOID, vec![], ModifierFlags::empty());
    let proto = b.elem(ProgramElementKind::FunctionPrototype(FunctionPrototype {
        declaration: f,
    }));
    assert_eq!(b.pool.element_description(proto), "void f();");
}

/// Port of: tests/SkSLTest.cpp#L808-L829 (chrome/m156), `test_clone`'s `CloneVisitor`: every
/// expression's clone describes itself exactly like the original. The visited pool is frozen,
/// and clones go into a pool layered on it, as a program's pool extends its module's.
struct CloneVisitor {
    scratch: IrPool,
    checked: usize,
}

impl ProgramVisitor for CloneVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        let original = pool.expression_description(expr);
        let cloned = self.scratch.clone_expression(expr);
        assert_eq!(
            original,
            self.scratch.expression_description(cloned),
            "Mismatch after clone!"
        );
        self.checked += 1;
        walk_expression(self, pool, expr)
    }
}

fn program_from(b: Builder, elements: Vec<ElemId>, kind: ProgramKind) -> Program {
    let mut pool = b.pool;
    let symbols = pool.add_symbol_table(SymbolTable::new(None, false));
    Program {
        source: Arc::from(""),
        config: ProgramConfig::new(ModuleType::Program, kind, ProgramSettings::default()),
        pool,
        symbols,
        owned_elements: elements,
        shared_elements: Vec::new(),
        interface: ProgramInterface::default(),
    }
}

#[test]
fn clone_visitor_on_hand_built_programs() {
    let mut b = Builder::new();
    let elements = array_size_folding(&mut b);
    let program = program_from(b, elements, ProgramKind::RuntimeShader);
    assert!(
        program
            .description()
            .starts_with("#version 100\nuniform half4 colorRed;")
    );
    assert!(
        program.get_function("main").is_none(),
        "no symbols were added"
    );
    let elements: Vec<ElemId> = program.elements().collect();
    let frozen = program.pool.freeze();
    let mut visitor = CloneVisitor {
        scratch: IrPool::extend(Arc::clone(&frozen)),
        checked: 0,
    };
    for e in elements {
        assert!(!visitor.visit_program_element(&frozen, e));
    }
    // a(): 1; b(): 6 calls, 6 references, 5 `&&`; c(): 1; d(): 5; main(): 8.
    assert_eq!(visitor.checked, 32);
}

#[test]
fn clone_copies_subtrees_and_keeps_stable_call_pointers() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let f = b.func("f", TypeId::INT, vec![], ModifierFlags::empty());
    let call = b.call(f, vec![]);
    let rx = b.vref(x);
    let sum = b.binary(call, K::Plus, rx, TypeId::INT);
    b.pool.expression_mut(rx).position = Position::range(4, 5);
    let clone = b.pool.clone_expression_at(sum, Position::range(10, 20));
    assert_ne!(clone, sum);
    let cloned = b.pool.expression(clone).clone();
    assert_eq!(cloned.position, Position::range(10, 20));
    let ExpressionKind::Binary(bin) = cloned.kind else {
        unreachable!()
    };
    assert_ne!(bin.left, call);
    assert_ne!(bin.right, rx);
    // Children keep their own positions.
    assert_eq!(b.pool.expression(bin.right).position, Position::range(4, 5));
    let ExpressionKind::FunctionCall(fc) = &b.pool.expression(bin.left).kind else {
        unreachable!()
    };
    assert_eq!(fc.stable_pointer, call);
    // Symbols are shared, not copied.
    let ExpressionKind::VariableReference(v) = &b.pool.expression(bin.right).kind else {
        unreachable!()
    };
    assert_eq!(v.variable, x);
}

#[test]
fn layered_pools_resolve_parent_ids_and_allocate_above_them() {
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Global,
    );
    let rx = b.vref(x);
    let int3 = b.array_type(TypeId::INT, 3);
    let table = b.pool.add_symbol_table(SymbolTable::new(None, true));
    b.pool.inject_symbol(table, SymbolId::Variable(x));
    let module = b.pool.freeze();

    let mut program = IrPool::extend(Arc::clone(&module));
    // Parent ids resolve through the chain.
    assert_eq!(&*program.variable(x).name, "x");
    assert_eq!(program.ty(int3).name(), "int[3]");
    assert_eq!(program.expression_description(rx), "x");
    assert!(!program.is_local_expression(rx));
    // New ids are above the parent's.
    let child_table = program.add_symbol_table(SymbolTable::new(Some(table), false));
    let y = program.add_variable(Variable::new(
        Position::default(),
        Position::default(),
        ModifierFlags::empty(),
        "y",
        TypeId::INT,
        false,
        VariableStorage::Local,
    ));
    assert!(y.index() > x.index());
    assert!(program.is_local_variable(y));
    program.inject_symbol(child_table, SymbolId::Variable(y));
    assert_eq!(
        program.find_symbol(child_table, "y"),
        Some(SymbolId::Variable(y))
    );
    assert_eq!(
        program.find_symbol(child_table, "x"),
        Some(SymbolId::Variable(x))
    );
    assert_eq!(program.find_symbol(child_table, "z"), None);
    assert_eq!(
        program.find_builtin_symbol(child_table, "x"),
        Some(SymbolId::Variable(x))
    );
    // Cloning a module expression copies it into the program.
    let copy = program.clone_expression(rx);
    assert!(program.is_local_expression(copy));
    assert_eq!(program.expression_description(copy), "x");
    // A second layer sees both.
    let program = program.freeze();
    let third = IrPool::extend(program);
    assert_eq!(&*third.variable(y).name, "y");
    assert_eq!(&*third.variable(x).name, "x");
    // Built-in types resolve in every layer.
    assert_eq!(third.ty(TypeId::HALF4).name(), "half4");
}

#[test]
#[should_panic(expected = "frozen parent pool")]
fn frozen_parents_are_immutable() {
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Global,
    );
    let mut program = IrPool::extend(b.pool.freeze());
    program.variable_mut(x).name = "renamed".into();
}

#[test]
#[should_panic(expected = "frozen parent pool")]
fn built_in_types_are_immutable() {
    let mut pool = IrPool::new();
    let _ = pool.type_mut(TypeId::FLOAT);
}

#[test]
fn relocation_and_moves_keep_slots() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let rx = b.vref(x);
    let one = b.int(1);
    let sum = b.binary(rx, K::Plus, one, TypeId::INT);
    let stmt = b.expr_stmt(sum);
    // Wrap the statement in a block that takes over its slot (the inliner's pattern).
    let moved = b.pool.relocate_statement(stmt);
    let block = Statement::new(
        Position::default(),
        StatementKind::Block(Block {
            children: vec![moved],
            block_kind: BlockKind::BracedScope,
            symbol_table: None,
        }),
    );
    b.pool.replace_statement(stmt, block);
    assert_eq!(b.pool.statement_description(stmt), "{\nx + 1;\n}\n");
    // `e = std::move(binary.left())`: the slot takes the child's node.
    b.pool.move_expression_into(sum, rx);
    assert_eq!(b.pool.statement_description(stmt), "{\nx;\n}\n");
}

/// A writer that replaces every integer literal 1 with 2 and every `x + 0` with `x`.
struct FoldWriter;

impl ProgramWriter for FoldWriter {
    // Literal values are exact small integers.
    #[allow(clippy::float_cmp)]
    fn visit_expression_ptr(&mut self, ctx: &mut Context, expr: ExprId) -> bool {
        let node = ctx.pool.expression(expr).clone();
        match node.kind {
            ExpressionKind::Literal(Literal { value: 1.0 }) => {
                ctx.pool.replace_expression(
                    expr,
                    Expression::new(
                        node.position,
                        node.ty,
                        ExpressionKind::Literal(Literal { value: 2.0 }),
                    ),
                );
                false
            }
            ExpressionKind::Binary(b)
                if ctx
                    .pool
                    .expression(b.right)
                    .as_literal()
                    .is_some_and(|l| l.value == 0.0) =>
            {
                ctx.pool.move_expression_into(expr, b.left);
                self.visit_expression_ptr(ctx, expr)
            }
            _ => walk_expression_mut(self, ctx, expr),
        }
    }
}

#[test]
fn program_writer_rewrites_in_place() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let f = b.func("f", TypeId::INT, vec![], ModifierFlags::empty());
    let rx = b.vref(x);
    let one = b.int(1);
    let sum = b.binary(rx, K::Plus, one, TypeId::INT);
    let zero = b.int(0);
    let plus_zero = b.binary(sum, K::Plus, zero, TypeId::INT);
    let one = b.int(1);
    let call = b.call(f, vec![one]);
    let r = b.ret(Some(plus_zero));
    let s = b.expr_stmt(call);
    let def = b.define(f, vec![s, r]);
    let mut program = program_from(b, vec![def], ProgramKind::Fragment);
    let mut ctx = Context::new(ErrorReporter::no_op());
    let stopped = ctx.with_program(&mut program, |ctx| {
        FoldWriter.visit_program_element(ctx, def)
    });
    assert!(!stopped);
    assert_eq!(
        program.description(),
        "int f() {\nf(2);\nreturn x + 2;\n}\n"
    );
}

/// Records the order in which expressions are visited.
struct OrderVisitor(Vec<String>);

impl ProgramVisitor for OrderVisitor {
    fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
        self.0.push(pool.expression_description(expr));
        walk_expression(self, pool, expr)
    }
}

#[test]
fn program_visitor_order_and_early_exit() {
    use OperatorKind as K;
    let mut b = Builder::new();
    let x = b.var(
        "x",
        TypeId::INT,
        ModifierFlags::empty(),
        VariableStorage::Local,
    );
    let rx = b.vref(x);
    let one = b.int(1);
    let sum = b.binary(rx, K::Plus, one, TypeId::INT);
    let two = b.int(2);
    let rx2 = b.vref(x);
    let lt = b.binary(two, K::Lt, rx2, TypeId::BOOL);
    let other_one = b.int(1);
    let t = b.ternary(lt, sum, other_one);
    let mut v = OrderVisitor(Vec::new());
    assert!(!v.visit_expression(&b.pool, t));
    assert_eq!(
        v.0,
        [
            "2 < x ? x + 1 : 1",
            "2 < x",
            "2",
            "x",
            "x + 1",
            "x",
            "1",
            "1"
        ]
    );

    /// Stops at the first literal 2.
    struct FindTwo;
    impl ProgramVisitor for FindTwo {
        // Literal values are exact small integers.
        #[allow(clippy::float_cmp)]
        fn visit_expression(&mut self, pool: &IrPool, expr: ExprId) -> bool {
            pool.expression(expr)
                .as_literal()
                .is_some_and(|l| l.value == 2.0)
                || walk_expression(self, pool, expr)
        }
    }
    assert!(FindTwo.visit_expression(&b.pool, t));
    assert!(!FindTwo.visit_expression(&b.pool, sum));
}

#[test]
fn is_incomplete_reports_dangling_references() {
    let mut ctx = Context::new(ErrorReporter::forwarding());
    let f = ctx.pool.add_function(FunctionDeclaration {
        position: Position::default(),
        name: "f".into(),
        definition: None,
        next_overload: None,
        parameters: Vec::new(),
        return_type: TypeId::VOID,
        modifier_flags: ModifierFlags::empty(),
        intrinsic_kind: None,
        module_type: ModuleType::Program,
        is_main: false,
        has_main_coords_parameter: false,
        has_main_input_color_parameter: false,
        has_main_dest_color_parameter: false,
    });
    let reference = ctx.pool.add_expression(Expression::new(
        Position::range(3, 4),
        TypeId::INVALID,
        ExpressionKind::FunctionReference(FunctionReference { overload_chain: f }),
    ));
    let literal = ctx.pool.add_expression(Expression::new(
        Position::range(0, 1),
        TypeId::INT,
        ExpressionKind::Literal(Literal { value: 1.0 }),
    ));
    assert!(Expression::is_incomplete(&mut ctx, reference));
    assert!(!Expression::is_incomplete(&mut ctx, literal));
    assert_eq!(ctx.errors.error_count(), 1);
    let crate::error_reporter::ErrorSink::Forwarding { errors } = ctx.errors.sink() else {
        unreachable!()
    };
    assert_eq!(
        errors,
        &[(
            "expected '(' to begin function call".to_owned(),
            Position::range(4, 5)
        )]
    );
}
