// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLParser.{h,cpp}.

//! The `SkSL` [`Parser`]: consumes `SkSL` text and converts it into IR, element by element.
//!
//! The parser is a recursive-descent parser that converts as it goes: every grammar rule calls the
//! `convert` function of the IR node it builds (S6-S8), so the errors of parsing and of type
//! checking interleave exactly as in Skia. It runs against the [`Context`] it is given, which
//! must have a program config and a current symbol table (the module's or program's global
//! table), as Skia's `Compiler::initializeContext` sets up.
//!
//! # The driver hooks
//!
//! Skia's `Parser::programInheritingFrom` and `Parser::moduleInheritingFrom` finish with calls
//! into the compiler driver (`Compiler::releaseProgram`, `fCompiler.fGlobalSymbols`). Here they
//! stop at the parse step and return the program elements, and the driver in
//! [`crate::compiler`] (task S11) does the rest:
//!
//! - [`Parser::program_inheriting_from`] returns `Some(elements)` when parsing reported no errors,
//!   and `None` otherwise. `Compiler::convert_program` builds the `Program` from them.
//! - [`Parser::module_inheriting_from`] returns the module's elements. `Compiler::compile_module`
//!   builds the `Module` (`fParent`, `fSymbols`, `fModuleType`) around them.

use std::borrow::Cow;

use crate::constant_folder;
use crate::context::Context;
use crate::defines::{SkslFloat, SkslInt};
use crate::error_reporter::{ErrorReporter, forward_errors};
use crate::ir::{
    BinaryExpression, Block, BlockKind, BreakStatement, ContinueStatement, DiscardStatement,
    DoStatement, ElemId, ExprId, ExpressionKind, ExpressionStatement, Extension, Field,
    FieldAccess, ForStatement, FunctionCall, FunctionDeclaration, FunctionDefinition,
    FunctionPrototype, GlobalVarDeclaration, IfStatement, IndexExpression, InterfaceBlock, Layout,
    LayoutFlags, Literal, ModifierFlags, Modifiers, ModifiersDeclaration, Nop, Poison,
    PostfixExpression, PrefixExpression, ReturnStatement, StmtId, StructDefinition,
    SwitchStatement, Swizzle, SymTabId, SymbolId, SymbolTable, TernaryExpression, Type, TypeId,
    TypeReference, VarDeclaration, VarId, Variable, VariableStorage, add_array_dimension,
    add_symbol, instantiate_symbol_ref,
};
use crate::lexer::{Lexer, Token, TokenKind};
use crate::operator::{Operator, OperatorKind};
use crate::position::{ForLoopPositions, Position};
use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings, Version};
use crate::string::{stod_float, stoi};

/// `kMaxParseDepth`: the recursion limit that keeps pathological inputs from overflowing the
/// stack.
const MAX_PARSE_DEPTH: i32 = 50;

/// `parse_modifier_token`: the modifier a token spells, or none.
// Port of: src/sksl/SkSLParser.cpp#L72-L96 (chrome/m156)
fn parse_modifier_token(token: Option<TokenKind>) -> ModifierFlags {
    match token {
        Some(TokenKind::Uniform) => ModifierFlags::UNIFORM,
        Some(TokenKind::Const) => ModifierFlags::CONST,
        Some(TokenKind::In) => ModifierFlags::IN,
        Some(TokenKind::Out) => ModifierFlags::OUT,
        Some(TokenKind::Inout) => ModifierFlags::IN | ModifierFlags::OUT,
        Some(TokenKind::Flat) => ModifierFlags::FLAT,
        Some(TokenKind::Noperspective) => ModifierFlags::NO_PERSPECTIVE,
        Some(TokenKind::Pure) => ModifierFlags::PURE,
        Some(TokenKind::Inline) => ModifierFlags::INLINE,
        Some(TokenKind::Noinline) => ModifierFlags::NO_INLINE,
        Some(TokenKind::Highp) => ModifierFlags::HIGHP,
        Some(TokenKind::Mediump) => ModifierFlags::MEDIUMP,
        Some(TokenKind::Lowp) => ModifierFlags::LOWP,
        Some(TokenKind::Export) => ModifierFlags::EXPORT,
        Some(TokenKind::Es3) => ModifierFlags::ES3,
        Some(TokenKind::Workgroup) => ModifierFlags::WORKGROUP,
        Some(TokenKind::Readonly) => ModifierFlags::READ_ONLY,
        Some(TokenKind::Writeonly) => ModifierFlags::WRITE_ONLY,
        Some(TokenKind::Buffer) => ModifierFlags::BUFFER,
        Some(TokenKind::Pixellocal) => ModifierFlags::PIXEL_LOCAL,
        _ => ModifierFlags::empty(),
    }
}

/// `is_whitespace`.
// Port of: src/sksl/SkSLParser.cpp#L231-L241 (chrome/m156)
fn is_whitespace(kind: Option<TokenKind>) -> bool {
    matches!(
        kind,
        Some(TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment)
    )
}

/// `range_of_at_least_one_char`.
// Port of: src/sksl/SkSLParser.cpp#L1585-L1587 (chrome/m156)
fn range_of_at_least_one_char(start: i32, end: i32) -> Position {
    Position::range(start, end.max(start + 1))
}

/// `Parser::Checkpoint`: the parser state to return to if a speculative parse fails. While a
/// checkpoint is open, errors go to a forwarding reporter: [`Parser::accept`] reports them to the
/// real reporter, [`Parser::rewind`] drops them.
// Port of: src/sksl/SkSLParser.cpp#L137-L212 (chrome/m156)
#[derive(Debug)]
struct Checkpoint {
    pushback: Token,
    lexer: crate::lexer::Checkpoint,
    /// `fOldErrorReporter`.
    old_error_reporter: ErrorReporter,
    old_encountered_fatal_error: bool,
}

/// `Parser::VarDeclarationsPrefix`.
#[derive(Debug)]
struct VarDeclarationsPrefix {
    position: Position,
    modifiers: Modifiers,
    ty: TypeId,
    name: Token,
}

/// The pieces of a `for` statement that its scope produces.
struct ForParts {
    initializer: Option<StmtId>,
    test: Option<ExprId>,
    next: Option<ExprId>,
    statement: StmtId,
    first_semicolon_offset: i32,
    second_semicolon: Token,
    rparen: Token,
}

/// `SkSL::Parser`: consumes `SkSL` text and converts it into IR.
// Port of: src/sksl/SkSLParser.h#L42-L331 (chrome/m156)
#[doc(alias = "SkSL::Parser")]
#[derive(Debug)]
pub struct Parser<'a> {
    /// `fCompiler.context()`.
    ctx: &'a mut Context,
    /// `fSettings`.
    #[allow(dead_code)] // Skia stores the settings but the parser never reads them.
    settings: ProgramSettings,
    /// `fKind`.
    kind: ProgramKind,
    /// `fText`.
    text: &'a [u8],
    /// `fProgramElements`.
    program_elements: Vec<ElemId>,
    /// `fLexer`.
    lexer: Lexer<'a>,
    /// `fDepth`: the current parse depth, which enforces a recursion limit.
    depth: i32,
    /// `fPushback`.
    pushback: Token,
    /// `fEncounteredFatalError`.
    encountered_fatal_error: bool,
}

impl<'a> Parser<'a> {
    /// `Parser(compiler, settings, kind, text)`. The context must have its program config and
    /// current symbol table set (and the error reporter's source, for the error text).
    #[must_use]
    pub fn new(
        ctx: &'a mut Context,
        settings: ProgramSettings,
        kind: ProgramKind,
        text: &'a [u8],
    ) -> Self {
        Self {
            ctx,
            settings,
            kind,
            text,
            program_elements: Vec::new(),
            lexer: Lexer::new(text),
            depth: 0,
            pushback: Token::new(Some(TokenKind::None), -1, -1),
            encountered_fatal_error: false,
        }
    }

    /// `programInheritingFrom(module)`: parses `declaration* END_OF_FILE`. Returns the program
    /// elements when no error was reported. (`Compiler::convert_program` turns the elements into
    /// a `Program` through `release_program`; Skia returns null when there were errors.)
    // Port of: src/sksl/SkSLParser.cpp#L406-L416 (chrome/m156)
    #[must_use]
    pub fn program_inheriting_from(mut self) -> Option<Vec<ElemId>> {
        self.declarations();
        if self.ctx.errors.error_count() == 0 {
            Some(std::mem::take(&mut self.program_elements))
        } else {
            None
        }
    }

    /// `moduleInheritingFrom(parentModule)`: parses a module's declarations and returns its
    /// elements. (`Compiler::compile_module` wraps them, the global symbol table and the parent
    /// in a `Module`, and keeps the source text alive.)
    // Port of: src/sksl/SkSLParser.cpp#L418-L427 (chrome/m156)
    #[must_use]
    pub fn module_inheriting_from(mut self) -> Vec<ElemId> {
        self.declarations();
        std::mem::take(&mut self.program_elements)
    }

    /// `text(token)`: the source text of `token`. Bytes that are not UTF-8 (a token cut inside a
    /// character) are replaced.
    // Port of: src/sksl/SkSLParser.cpp#L333-L335 (chrome/m156)
    #[must_use]
    pub fn text(&self, token: Token) -> Cow<'a, str> {
        let start = usize::try_from(token.offset).unwrap_or(0);
        let length = usize::try_from(token.length).unwrap_or(0);
        String::from_utf8_lossy(&self.text[start..start + length])
    }

    /// `position(token)`.
    // Port of: src/sksl/SkSLParser.cpp#L337-L343 (chrome/m156)
    #[must_use]
    pub fn position(&self, token: Token) -> Position {
        if token.offset >= 0 {
            Position::range(token.offset, token.offset + token.length)
        } else {
            Position::default()
        }
    }

    /// `symbolTable()`: the context's current symbol table.
    fn symbol_table(&self) -> SymTabId {
        self.ctx
            .symbol_table
            .expect("Parser: the context has no current symbol table")
    }

    fn is_type_name(&self, name: &str) -> bool {
        self.ctx.pool.is_type(self.symbol_table(), name)
    }

    fn is_builtin_type_name(&self, name: &str) -> bool {
        self.ctx.pool.is_builtin_type(self.symbol_table(), name)
    }

    /// `AutoDepth`: runs `f`, then restores the depth that `f`'s increases raised.
    // Port of: src/sksl/SkSLParser.cpp#L98-L120 (chrome/m156)
    fn auto_depth<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = self.depth;
        let result = f(self);
        self.depth = saved;
        result
    }

    /// `AutoDepth::increase`: one more level of depth, which fails past the limit.
    fn increase_depth(&mut self) -> bool {
        self.depth += 1;
        if self.depth > MAX_PARSE_DEPTH {
            let peeked = self.peek();
            self.error(peeked, "exceeded max parse depth");
            self.encountered_fatal_error = true;
            return false;
        }
        true
    }

    /// `AutoSymbolTable`: runs `f` inside a new symbol table (when `enable`), which `f` is given.
    /// The context's table is the enclosing one again afterwards. Returns the new table.
    // Port of: src/sksl/SkSLParser.cpp#L122-L141 (chrome/m156)
    fn with_symbol_table<R>(
        &mut self,
        enable: bool,
        f: impl FnOnce(&mut Self, Option<SymTabId>) -> R,
    ) -> (Option<SymTabId>, R) {
        if !enable {
            return (None, f(self, None));
        }
        let parent = self.symbol_table();
        let builtin = self.ctx.pool.symbol_table(parent).is_builtin();
        let table = self
            .ctx
            .pool
            .add_symbol_table(SymbolTable::new(Some(parent), builtin));
        self.ctx.symbol_table = Some(table);
        let result = f(self, Some(table));
        self.ctx.symbol_table = self.ctx.pool.symbol_table(table).parent;
        (Some(table), result)
    }

    /// `Checkpoint(parser)`: saves the parse state and starts collecting errors.
    fn checkpoint(&mut self) -> Checkpoint {
        let old_error_reporter = self.ctx.set_error_reporter(ErrorReporter::forwarding());
        Checkpoint {
            pushback: self.pushback,
            lexer: self.lexer.checkpoint(),
            old_error_reporter,
            old_encountered_fatal_error: self.encountered_fatal_error,
        }
    }

    /// `Checkpoint::accept`: keeps the parse, and reports the errors it collected.
    // Port of: src/sksl/SkSLParser.cpp#L156-L162 (chrome/m156)
    fn accept(&mut self, checkpoint: Checkpoint) {
        let forwarding = self.ctx.set_error_reporter(checkpoint.old_error_reporter);
        // Parser errors should have been fatal, but we can encounter other errors like type
        // mismatches despite accepting the parse. Forward those messages to the actual error
        // handler now.
        forward_errors(&forwarding, &mut self.ctx.errors);
    }

    /// `Checkpoint::rewind`: returns to the saved parse state, dropping the collected errors.
    // Port of: src/sksl/SkSLParser.cpp#L164-L169 (chrome/m156)
    fn rewind(&mut self, checkpoint: Checkpoint) {
        drop(self.ctx.set_error_reporter(checkpoint.old_error_reporter));
        self.pushback = checkpoint.pushback;
        self.lexer.rewind_to_checkpoint(checkpoint.lexer);
        self.encountered_fatal_error = checkpoint.old_encountered_fatal_error;
    }

    /// `nextRawToken`: the next token, including whitespace, from the parse stream.
    // Port of: src/sksl/SkSLParser.cpp#L199-L230 (chrome/m156)
    fn next_raw_token(&mut self) -> Token {
        if self.pushback.kind != Some(TokenKind::None) {
            // Retrieve the token from the pushback buffer.
            let token = self.pushback;
            self.pushback.kind = Some(TokenKind::None);
            return token;
        }
        // Fetch a token from the lexer.
        let mut token = self.lexer.next_token();

        // Some tokens are always invalid, so we detect and report them here.
        match token.kind {
            Some(TokenKind::PrivateIdentifier)
                if ProgramConfig::allows_private_identifiers(self.kind) =>
            {
                token.kind = Some(TokenKind::Identifier);
            }
            Some(TokenKind::PrivateIdentifier | TokenKind::Reserved) => {
                let msg = format!("name '{}' is reserved", self.text(token));
                self.error(token, &msg);
                token.kind = Some(TokenKind::Identifier); // reduces additional follow-up errors
            }
            Some(TokenKind::BadOctal) => {
                let msg = format!("'{}' is not a valid octal number", self.text(token));
                self.error(token, &msg);
            }
            _ => {}
        }
        token
    }

    /// `expectNewline`: if the next token is a newline, consumes it.
    // Port of: src/sksl/SkSLParser.cpp#L243-L257 (chrome/m156)
    fn expect_newline(&mut self) -> bool {
        let token = self.next_raw_token();
        if token.kind == Some(TokenKind::Whitespace) {
            // The lexer doesn't distinguish newlines from other forms of whitespace, so we check
            // for newlines by searching through the token text.
            let text = self.text(token);
            if text.contains('\r') || text.contains('\n') {
                return true;
            }
        }
        // We didn't find a newline.
        self.pushback(token);
        false
    }

    /// `nextToken`: the next non-whitespace token.
    // Port of: src/sksl/SkSLParser.cpp#L259-L266 (chrome/m156)
    fn next_token(&mut self) -> Token {
        loop {
            let token = self.next_raw_token();
            if !is_whitespace(token.kind) {
                return token;
            }
        }
    }

    /// `pushback`: makes `t` the next token read. Only one level is supported.
    // Port of: src/sksl/SkSLParser.cpp#L268-L271 (chrome/m156)
    fn pushback(&mut self, t: Token) {
        debug_assert_eq!(self.pushback.kind, Some(TokenKind::None));
        self.pushback = t;
    }

    /// `peek`: the next non-whitespace token, without consuming it.
    // Port of: src/sksl/SkSLParser.cpp#L273-L278 (chrome/m156)
    fn peek(&mut self) -> Token {
        if self.pushback.kind == Some(TokenKind::None) {
            self.pushback = self.next_token();
        }
        self.pushback
    }

    /// `peek().fKind`.
    fn peek_kind(&mut self) -> Option<TokenKind> {
        self.peek().kind
    }

    /// `checkNext`: consumes the next token if it is of `kind`.
    // Port of: src/sksl/SkSLParser.cpp#L280-L292 (chrome/m156)
    fn check_next(&mut self, kind: TokenKind) -> Option<Token> {
        if self.pushback.kind != Some(TokenKind::None) && self.pushback.kind != Some(kind) {
            return None;
        }
        let next = self.next_token();
        if next.kind == Some(kind) {
            return Some(next);
        }
        self.pushback(next);
        None
    }

    /// `expect`: reads the next token and reports an error if it is not of `kind`.
    // Port of: src/sksl/SkSLParser.cpp#L294-L307 (chrome/m156)
    fn expect(&mut self, kind: TokenKind, expected: &str) -> Option<Token> {
        let next = self.next_token();
        if next.kind == Some(kind) {
            Some(next)
        } else {
            let mut msg = format!("expected {expected}, but found '").into_bytes();
            msg.extend_from_slice(self.text_bytes(next));
            msg.push(b'\'');
            self.error_bytes(next, &msg);
            self.encountered_fatal_error = true;
            None
        }
    }

    /// `expectIdentifier`: like `expect(TK_IDENTIFIER)`, but the identifier may not be a type.
    // Port of: src/sksl/SkSLParser.cpp#L309-L320 (chrome/m156)
    fn expect_identifier(&mut self) -> Option<Token> {
        let result = self.expect(TokenKind::Identifier, "an identifier")?;
        let text = self.text(result);
        if self.is_builtin_type_name(&text) {
            let msg = format!("expected an identifier, but found type '{text}'");
            self.error(result, &msg);
            self.encountered_fatal_error = true;
            return None;
        }
        Some(result)
    }

    /// `checkIdentifier`: like `checkNext(TK_IDENTIFIER)`, but a builtin type is not an
    /// identifier.
    // Port of: src/sksl/SkSLParser.cpp#L322-L331 (chrome/m156)
    fn check_identifier(&mut self) -> Option<Token> {
        let result = self.check_next(TokenKind::Identifier)?;
        if self.is_builtin_type_name(&self.text(result)) {
            self.pushback(result);
            return None;
        }
        Some(result)
    }

    /// `error(token, msg)`.
    fn error(&mut self, token: Token, msg: &str) {
        let pos = self.position(token);
        self.error_at(pos, msg);
    }

    /// `error(position, msg)`.
    fn error_at(&mut self, position: Position, msg: &str) {
        self.ctx.errors.error(position, msg);
    }

    /// `error(token, msg)` for a message in bytes, which may quote a source byte that is not
    /// UTF-8.
    fn error_bytes(&mut self, token: Token, msg: &[u8]) {
        let pos = self.position(token);
        self.ctx.errors.error_bytes(pos, msg);
    }

    /// `text(token)` as the exact source bytes, for messages that quote a token (a token may
    /// hold a byte that is not UTF-8, and Skia's messages are bytes).
    fn text_bytes(&self, token: Token) -> &'a [u8] {
        let start = usize::try_from(token.offset).unwrap_or(0);
        let length = usize::try_from(token.length).unwrap_or(0);
        &self.text[start..start + length]
    }

    /// `rangeFrom(start)`: the range from `start` to the current parse position.
    // Port of: src/sksl/SkSLParser.cpp#L353-L362 (chrome/m156)
    fn range_from(&self, start: Position) -> Position {
        let offset = if self.pushback.kind == Some(TokenKind::None) {
            self.lexer.checkpoint().offset()
        } else {
            self.pushback.offset
        };
        Position::range(start.start_offset(), offset)
    }

    /// `rangeFrom(Token)`.
    fn range_from_token(&self, start: Token) -> Position {
        self.range_from(self.position(start))
    }

    fn expr_position(&self, expr: ExprId) -> Position {
        self.ctx.pool.expression(expr).position
    }

    /// `declarations`: `declaration* END_OF_FILE`.
    // Port of: src/sksl/SkSLParser.cpp#L429-L463 (chrome/m156)
    fn declarations(&mut self) {
        self.encountered_fatal_error = false;

        // If the program is 8MB or longer (Position::kMaxOffset), error reporting goes off the
        // rails. At any rate, there's no good reason for a program to be this long.
        if self.text.len() >= usize::try_from(Position::MAX_OFFSET).unwrap_or(usize::MAX) {
            self.error_at(Position::default(), "program is too large");
            return;
        }

        // Any #version directive must appear as the first thing in a file
        if self.peek_kind() == Some(TokenKind::Directive) {
            self.directive(true);
        }

        let global_symbols = self.symbol_table();
        while !self.encountered_fatal_error {
            // We should always be at global scope when processing top-level declarations.
            debug_assert_eq!(self.ctx.symbol_table, Some(global_symbols));

            match self.peek_kind() {
                Some(TokenKind::EndOfFile) => return,
                Some(TokenKind::Invalid) => {
                    let peeked = self.peek();
                    self.error(peeked, "invalid token");
                    return;
                }
                Some(TokenKind::Directive) => self.directive(false),
                _ => {
                    self.declaration();
                }
            }
        }
    }

    /// `DIRECTIVE(#extension) IDENTIFIER COLON IDENTIFIER NEWLINE`.
    // Port of: src/sksl/SkSLParser.cpp#L465-L490 (chrome/m156)
    fn extension_directive(&mut self, start: Position) {
        let Some(name) = self.expect_identifier() else {
            return;
        };
        if self.expect(TokenKind::Colon, "':'").is_none() {
            return;
        }
        let Some(behavior) = self.expect(TokenKind::Identifier, "an identifier") else {
            return;
        };
        // We expect a newline immediately after `#extension name : behavior`.
        if self.expect_newline() {
            let pos = self.range_from(start);
            let name = self.text(name);
            let behavior = self.text(behavior);
            if let Some(ext) = Extension::convert(self.ctx, pos, &name, &behavior) {
                self.program_elements.push(ext);
            }
        } else {
            self.error_at(start, "invalid #extension directive");
        }
    }

    /// `DIRECTIVE(#version) INTLITERAL NEWLINE`.
    // Port of: src/sksl/SkSLParser.cpp#L492-L518 (chrome/m156)
    fn version_directive(&mut self, start: Position, allow_version: bool) {
        if !allow_version {
            self.error_at(start, "#version directive must appear before anything else");
            return;
        }
        let Some(version) = self.int_literal() else {
            return;
        };
        let required = match version {
            100 => Version::K100,
            300 => Version::K300,
            _ => {
                self.error_at(start, "unsupported version number");
                return;
            }
        };
        self.ctx
            .config
            .as_mut()
            .expect("Parser: the context has no ProgramConfig")
            .required_sksl_version = required;
        // We expect a newline after a #version directive.
        if !self.expect_newline() {
            self.error_at(start, "invalid #version directive");
        }
    }

    /// `DIRECTIVE(#extension) IDENTIFIER COLON IDENTIFIER NEWLINE |
    /// DIRECTIVE(#version) INTLITERAL NEWLINE`.
    // Port of: src/sksl/SkSLParser.cpp#L520-L534 (chrome/m156)
    fn directive(&mut self, allow_version: bool) {
        let Some(start) = self.expect(TokenKind::Directive, "a directive") else {
            return;
        };
        let text = self.text(start);
        if text == "#extension" {
            let pos = self.position(start);
            return self.extension_directive(pos);
        }
        if text == "#version" {
            let pos = self.position(start);
            return self.version_directive(pos, allow_version);
        }
        let msg = format!("unsupported directive '{text}'");
        self.error(start, &msg);
    }

    /// `modifiersDeclarationEnd`.
    // Port of: src/sksl/SkSLParser.cpp#L536-L544 (chrome/m156)
    fn modifiers_declaration_end(&mut self, mods: &Modifiers) -> bool {
        let Some(decl) = ModifiersDeclaration::convert(self.ctx, mods) else {
            return false;
        };
        self.program_elements.push(decl);
        true
    }

    /// `modifiers (structVarDeclaration | type IDENTIFIER ((LPAREN parameter (COMMA parameter)*
    /// RPAREN (block | SEMICOLON)) | SEMICOLON) | interfaceBlock)`.
    // Port of: src/sksl/SkSLParser.cpp#L546-L588 (chrome/m156)
    fn declaration(&mut self) -> bool {
        let start = self.peek();
        if start.kind == Some(TokenKind::Semicolon) {
            self.next_token();
            self.error(start, "expected a declaration, but found ';'");
            return false;
        }
        let mut modifiers = self.modifiers();
        let lookahead = self.peek();
        if lookahead.kind == Some(TokenKind::Identifier)
            && !self.is_type_name(&self.text(lookahead))
        {
            // we have an identifier that's not a type, could be the start of an interface block
            return self.interface_block(&modifiers);
        }
        if lookahead.kind == Some(TokenKind::Semicolon) {
            self.next_token();
            return self.modifiers_declaration_end(&modifiers);
        }
        if lookahead.kind == Some(TokenKind::Struct) {
            let pos = self.position(start);
            self.struct_var_declaration(pos, &modifiers);
            return true;
        }
        let Some(ty) = self.type_(&mut modifiers) else {
            return false;
        };
        let Some(name) = self.expect_identifier() else {
            return false;
        };
        if self.check_next(TokenKind::Lparen).is_some() {
            let pos = self.position(start);
            self.function_declaration_end(pos, &modifiers, ty, name)
        } else {
            let pos = self.position(start);
            self.global_var_declaration_end(pos, &modifiers, ty, name);
            true
        }
    }

    /// `(RPAREN | VOID RPAREN | parameter (COMMA parameter)* RPAREN) (block | SEMICOLON)`.
    // Port of: src/sksl/SkSLParser.cpp#L590-L639 (chrome/m156)
    fn function_declaration_end(
        &mut self,
        start: Position,
        modifiers: &Modifiers,
        return_type: TypeId,
        name: Token,
    ) -> bool {
        let lookahead = self.peek();
        let mut parameters: Vec<VarId> = Vec::new();
        if lookahead.kind == Some(TokenKind::Rparen) {
            // `()` means no parameters at all.
        } else if lookahead.kind == Some(TokenKind::Identifier) && self.text(lookahead) == "void" {
            // `(void)` also means no parameters at all.
            self.next_token();
        } else {
            loop {
                // `Variable::Convert` always yields a variable, so (unlike Skia's null check)
                // every parameter list is valid here.
                let Some(param) = self.parameter() else {
                    return false;
                };
                parameters.push(param);
                if self.check_next(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        if self.expect(TokenKind::Rparen, "')'").is_none() {
            return false;
        }

        let pos = self.range_from(start);
        let name = self.text(name);
        let decl = FunctionDeclaration::convert(
            self.ctx,
            pos,
            modifiers,
            &name,
            &parameters,
            start,
            return_type,
        );

        if self.check_next(TokenKind::Semicolon).is_some() {
            self.prototype_function(decl)
        } else {
            self.define_function(decl)
        }
    }

    /// `prototypeFunction`.
    // Port of: src/sksl/SkSLParser.cpp#L641-L647 (chrome/m156)
    fn prototype_function(&mut self, decl: Option<crate::ir::FnId>) -> bool {
        let Some(decl) = decl else {
            return false;
        };
        let pos = self.ctx.pool.function(decl).position;
        let proto = FunctionPrototype::make(&mut self.ctx.pool, pos, decl);
        self.program_elements.push(proto);
        true
    }

    /// `defineFunction`.
    // Port of: src/sksl/SkSLParser.cpp#L649-L690 (chrome/m156)
    fn define_function(&mut self, decl: Option<crate::ir::FnId>) -> bool {
        let body_start = self.peek();

        // Create a symbol table for the function which includes the parameters.
        let (_, body) = self.with_symbol_table(true, |this, symbol_table| {
            let symbol_table = symbol_table.expect("a new symbol table");
            if let Some(decl) = decl {
                let params = this.ctx.pool.function(decl).parameters.clone();
                for param in params {
                    add_symbol(this.ctx, symbol_table, SymbolId::Variable(param));
                }
            }

            // Parse the function body.
            this.block(false, Some(symbol_table))
        });

        // If there was a problem with the declarations or body, don't actually create a
        // definition.
        let (Some(decl), Some(block)) = (decl, body) else {
            return false;
        };

        let pos = self.range_from_token(body_start);
        self.ctx.pool.statement_mut(block).position = pos;

        let Some(function) = FunctionDefinition::convert(self.ctx, pos, decl, Some(block)) else {
            return false;
        };
        // `FunctionDefinition::Convert` records the definition on the declaration.
        self.program_elements.push(function);
        true
    }

    /// `arraySize`: parses an expression representing an array size. Reports errors if the array
    /// size is not valid (out of bounds, not a literal integer). Returns `Some` if an expression
    /// was successfully parsed, even if that array size is not actually valid. In that case the
    /// result is always a valid array size (invalid array sizes result in a 1 to avoid additional
    /// errors downstream).
    // Port of: src/sksl/SkSLParser.cpp#L692-L723 (chrome/m156)
    fn array_size(&mut self) -> Option<SkslInt> {
        // Start out with a safe value that won't generate any errors downstream
        let mut result = 1;
        let next = self.peek();
        if next.kind == Some(TokenKind::Rbracket) {
            let pos = self.position(next);
            self.error_at(pos, "unsized arrays are not permitted here");
            return Some(result);
        }
        let size_literal = self.expression()?;
        if !matches!(
            self.ctx.pool.expression(size_literal).kind,
            ExpressionKind::Poison(_)
        ) {
            let pos = self.expr_position(size_literal);
            let Some(size) = constant_folder::get_constant_int(&self.ctx.pool, size_literal) else {
                self.error_at(pos, "array size must be an integer");
                return Some(result);
            };
            if size > SkslInt::from(i32::MAX) {
                self.error_at(pos, "array size out of bounds");
                return Some(result);
            }
            if size <= 0 {
                self.error_at(pos, "array size must be positive");
                return Some(result);
            }
            // Now that we've validated it, output the real value
            result = size;
        }
        Some(result)
    }

    /// `arrayType`.
    // Port of: src/sksl/SkSLParser.cpp#L725-L732 (chrome/m156)
    fn array_type(&mut self, base: TypeId, count: SkslInt, pos: Position) -> TypeId {
        let count = base.convert_array_size_value(self.ctx, pos, pos, count);
        if count == 0 {
            return TypeId::POISON;
        }
        let table = self.symbol_table();
        // The size was validated against `INT32_MAX`.
        let count = i32::try_from(count).unwrap_or(i32::MAX);
        add_array_dimension(self.ctx, table, base, count)
    }

    /// `unsizedArrayType`.
    // Port of: src/sksl/SkSLParser.cpp#L734-L741 (chrome/m156)
    fn unsized_array_type(&mut self, base: TypeId, pos: Position) -> TypeId {
        if !base.check_if_usable_in_array(self.ctx, pos) {
            return TypeId::POISON;
        }
        let table = self.symbol_table();
        add_array_dimension(self.ctx, table, base, Type::UNSIZED_ARRAY)
    }

    /// `allowUnsizedArrays`.
    fn allow_unsized_arrays(&self) -> bool {
        ProgramConfig::is_compute(self.kind)
            || ProgramConfig::is_fragment(self.kind)
            || ProgramConfig::is_vertex(self.kind)
    }

    /// `parseArrayDimensions`: `(LBRACKET expression? RBRACKET)*`. Returns false on a parse
    /// error.
    // Port of: src/sksl/SkSLParser.cpp#L743-L766 (chrome/m156)
    fn parse_array_dimensions(&mut self, pos: Position, ty: &mut TypeId) -> bool {
        while self.check_next(TokenKind::Lbracket).is_some() {
            if self.check_next(TokenKind::Rbracket).is_some() {
                if self.allow_unsized_arrays() {
                    let range = self.range_from(pos);
                    *ty = self.unsized_array_type(*ty, range);
                } else {
                    let range = self.range_from(pos);
                    self.error_at(range, "unsized arrays are not permitted here");
                }
            } else {
                let Some(size) = self.array_size() else {
                    return false;
                };
                if self.expect(TokenKind::Rbracket, "']'").is_none() {
                    return false;
                }
                let range = self.range_from(pos);
                *ty = self.array_type(*ty, size, range);
            }
        }
        true
    }

    /// `parseInitializer`: `(EQ assignmentExpression)?`. `Err` is a parse error.
    // Port of: src/sksl/SkSLParser.cpp#L768-L774 (chrome/m156)
    fn parse_initializer(&mut self) -> Result<Option<ExprId>, ()> {
        if self.check_next(TokenKind::Eq).is_some() {
            return self.assignment_expression().map(Some).ok_or(());
        }
        Ok(None)
    }

    /// `addGlobalVarDeclaration`.
    // Port of: src/sksl/SkSLParser.cpp#L776-L780 (chrome/m156)
    fn add_global_var_declaration(&mut self, decl: Option<StmtId>) {
        if let Some(decl) = decl {
            let element = GlobalVarDeclaration::make(&mut self.ctx.pool, decl);
            self.program_elements.push(element);
        }
    }

    /// `(LBRACKET expression? RBRACKET)* (EQ assignmentExpression)? (COMMA IDENTIFER
    /// (LBRACKET expression? RBRACKET)* (EQ assignmentExpression)?)* SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L782-L833 (chrome/m156)
    fn global_var_declaration_end(
        &mut self,
        pos: Position,
        mods: &Modifiers,
        base_type: TypeId,
        name: Token,
    ) {
        let mut ty = base_type;
        if !self.parse_array_dimensions(pos, &mut ty) {
            return;
        }
        let Ok(initializer) = self.parse_initializer() else {
            return;
        };
        let range = self.range_from(pos);
        let name_text = self.text(name);
        let decl = VarDeclaration::convert(
            self.ctx,
            range,
            mods,
            ty,
            &name_text,
            VariableStorage::Global,
            initializer,
        );
        self.add_global_var_declaration(decl);
        while self.check_next(TokenKind::Comma).is_some() {
            ty = base_type;
            let Some(identifier_name) = self.expect_identifier() else {
                return;
            };
            if !self.parse_array_dimensions(pos, &mut ty) {
                return;
            }
            let Ok(another_initializer) = self.parse_initializer() else {
                return;
            };
            let range = self.range_from_token(identifier_name);
            let name_text = self.text(identifier_name);
            let decl = VarDeclaration::convert(
                self.ctx,
                range,
                mods,
                ty,
                &name_text,
                VariableStorage::Global,
                another_initializer,
            );
            self.add_global_var_declaration(decl);
        }
        self.expect(TokenKind::Semicolon, "';'");
    }

    /// `localVarDeclarationEnd`: the same grammar as [`Self::global_var_declaration_end`].
    // Port of: src/sksl/SkSLParser.cpp#L835-L892 (chrome/m156)
    fn local_var_declaration_end(
        &mut self,
        pos: Position,
        mods: &Modifiers,
        base_type: TypeId,
        name: Token,
    ) -> StmtId {
        let mut ty = base_type;
        if !self.parse_array_dimensions(pos, &mut ty) {
            return self.statement_or_nop(Position::default(), None);
        }
        let Ok(initializer) = self.parse_initializer() else {
            return self.statement_or_nop(Position::default(), None);
        };
        let range = self.range_from(pos);
        let name_text = self.text(name);
        let mut result = VarDeclaration::convert(
            self.ctx,
            range,
            mods,
            ty,
            &name_text,
            VariableStorage::Local,
            initializer,
        );
        loop {
            if self.check_next(TokenKind::Comma).is_none() {
                self.expect(TokenKind::Semicolon, "';'");
                break;
            }
            ty = base_type;
            let Some(identifier_name) = self.expect_identifier() else {
                break;
            };
            if !self.parse_array_dimensions(pos, &mut ty) {
                break;
            }
            let Ok(another_initializer) = self.parse_initializer() else {
                break;
            };
            let range = self.range_from_token(identifier_name);
            let name_text = self.text(identifier_name);
            let next = VarDeclaration::convert(
                self.ctx,
                range,
                mods,
                ty,
                &name_text,
                VariableStorage::Local,
                another_initializer,
            );

            result = Block::make_compound_statement(&mut self.ctx.pool, result, next);
        }
        let pos = self.range_from(pos);
        self.statement_or_nop(pos, result)
    }

    /// `(varDeclarations | expressionStatement)`.
    // Port of: src/sksl/SkSLParser.cpp#L894-L927 (chrome/m156)
    fn var_declarations_or_expression_statement(&mut self) -> Option<StmtId> {
        let next_token = self.peek();
        if next_token.kind == Some(TokenKind::Const) {
            // Statements that begin with `const` might be variable declarations, but can't be
            // legal SkSL expression-statements. (SkSL constructors don't take a `const`
            // modifier.)
            return self.var_declarations();
        }

        if matches!(
            next_token.kind,
            Some(TokenKind::Highp | TokenKind::Mediump | TokenKind::Lowp)
        ) || self.is_type_name(&self.text(next_token))
        {
            // Statements that begin with a typename are most often variable declarations, but
            // occasionally the type is part of a constructor, and these are actually expression-
            // statements in disguise. First, attempt the common case: parse it as a vardecl.
            let checkpoint = self.checkpoint();
            if let Some(prefix) = self.var_declarations_prefix() {
                self.accept(checkpoint);
                return Some(self.local_var_declaration_end(
                    prefix.position,
                    &prefix.modifiers,
                    prefix.ty,
                    prefix.name,
                ));
            }

            // If this statement wasn't actually a vardecl after all, rewind and try parsing it as
            // an expression-statement instead.
            self.rewind(checkpoint);
        }
        self.expression_statement()
    }

    /// Helper function for `varDeclarations`. If this function succeeds, we assume that the rest
    /// of the statement is a variable-declaration statement, not an expression-statement.
    // Port of: src/sksl/SkSLParser.cpp#L929-L938 (chrome/m156)
    fn var_declarations_prefix(&mut self) -> Option<VarDeclarationsPrefix> {
        let peeked = self.peek();
        let position = self.position(peeked);
        let mut modifiers = self.modifiers();
        let ty = self.type_(&mut modifiers)?;
        let name = self.expect_identifier()?;
        Some(VarDeclarationsPrefix {
            position,
            modifiers,
            ty,
            name,
        })
    }

    /// `modifiers type IDENTIFIER varDeclarationEnd`.
    // Port of: src/sksl/SkSLParser.cpp#L940-L948 (chrome/m156)
    fn var_declarations(&mut self) -> Option<StmtId> {
        let prefix = self.var_declarations_prefix()?;
        Some(self.local_var_declaration_end(
            prefix.position,
            &prefix.modifiers,
            prefix.ty,
            prefix.name,
        ))
    }

    /// `STRUCT IDENTIFIER LBRACE varDeclaration* RBRACE`.
    // Port of: src/sksl/SkSLParser.cpp#L950-L1019 (chrome/m156)
    fn struct_declaration(&mut self) -> Option<TypeId> {
        self.auto_depth(|this| {
            let peeked = this.peek();
            let start = this.position(peeked);
            this.expect(TokenKind::Struct, "'struct'")?;
            let name = this.expect_identifier()?;
            this.expect(TokenKind::Lbrace, "'{'")?;
            if !this.increase_depth() {
                return None;
            }
            let mut fields: Vec<Field> = Vec::new();
            while this.check_next(TokenKind::Rbrace).is_none() {
                let field_start = this.peek();
                let mut modifiers = this.modifiers();
                let ty = this.type_(&mut modifiers)?;

                loop {
                    let mut actual_type = ty;
                    let member_name = this.expect_identifier()?;

                    while this.check_next(TokenKind::Lbracket).is_some() {
                        let size = this.array_size()?;
                        this.expect(TokenKind::Rbracket, "']'")?;
                        let range = this.range_from_token(field_start);
                        actual_type = this.array_type(actual_type, size, range);
                    }

                    fields.push(Field {
                        position: this.range_from_token(field_start),
                        layout: modifiers.layout,
                        modifier_flags: modifiers.flags,
                        name: this.text(member_name).into(),
                        ty: actual_type,
                    });
                    if this.check_next(TokenKind::Comma).is_none() {
                        break;
                    }
                }

                this.expect(TokenKind::Semicolon, "';'")?;
            }
            let pos = this.range_from(start);
            let name = this.text(name);
            let def = StructDefinition::convert(this.ctx, pos, &name, fields);

            let crate::ir::ProgramElementKind::StructDefinition(definition) =
                &this.ctx.pool.element(def).kind
            else {
                unreachable!("StructDefinition::convert makes a struct definition");
            };
            let result = definition.ty;
            this.program_elements.push(def);
            Some(result)
        })
    }

    /// `structDeclaration ((IDENTIFIER varDeclarationEnd) | SEMICOLON)`.
    // Port of: src/sksl/SkSLParser.cpp#L1021-L1031 (chrome/m156)
    fn struct_var_declaration(&mut self, _start: Position, modifiers: &Modifiers) {
        let Some(ty) = self.struct_declaration() else {
            return;
        };
        if let Some(name) = self.check_identifier() {
            let pos = self.range_from_token(name);
            self.global_var_declaration_end(pos, modifiers, ty, name);
        } else {
            self.expect(TokenKind::Semicolon, "';'");
        }
    }

    /// `modifiers type IDENTIFIER (LBRACKET INT_LITERAL RBRACKET)?`.
    // Port of: src/sksl/SkSLParser.cpp#L1033-L1062 (chrome/m156)
    fn parameter(&mut self) -> Option<VarId> {
        let peeked = self.peek();
        let pos = self.position(peeked);
        let mut modifiers = self.modifiers();
        let mut ty = self.type_(&mut modifiers)?;
        // `Variable::Convert` does not use the name's position, so it is not computed.
        let name_text = self
            .check_identifier()
            .map_or(Cow::Borrowed(""), |name| self.text(name));
        if !self.parse_array_dimensions(pos, &mut ty) {
            return None;
        }
        let range = self.range_from(pos);
        Some(Variable::convert(
            self.ctx,
            range,
            modifiers.position,
            modifiers.layout,
            modifiers.flags,
            ty,
            &name_text,
            VariableStorage::Parameter,
        ))
    }

    /// `EQ INT_LITERAL`.
    // Port of: src/sksl/SkSLParser.cpp#L1064-L1080 (chrome/m156)
    fn layout_int(&mut self) -> i32 {
        if self.expect(TokenKind::Eq, "'='").is_none() {
            return -1;
        }
        let Some(result_token) = self.expect(TokenKind::IntLiteral, "a non-negative integer")
        else {
            return -1;
        };
        let result_frag = self.text(result_token);
        let Some(result_value) = stoi(&result_frag) else {
            let msg = format!("value in layout is too large: {result_frag}");
            self.error(result_token, &msg);
            return -1;
        };
        // `return resultValue` narrows the `SKSL_INT` to `int`.
        #[allow(clippy::cast_possible_truncation)] // Mirrors the C++ implicit conversion.
        {
            result_value as i32
        }
    }

    /// `layout`: `LAYOUT LPAREN IDENTIFIER (EQ INT_LITERAL)? (COMMA IDENTIFIER (EQ INT_LITERAL)?)*
    /// RPAREN`.
    // Port of: src/sksl/SkSLParser.cpp#L1082-L1180 (chrome/m156)
    fn layout(&mut self) -> Layout {
        let mut result = Layout::new();
        if self.check_next(TokenKind::Layout).is_some()
            && self.expect(TokenKind::Lparen, "'('").is_some()
        {
            loop {
                let t = self.next_token();
                let text = self.text(t);
                let found = match &*text {
                    "location" => Some(LayoutFlags::LOCATION),
                    "offset" => Some(LayoutFlags::OFFSET),
                    "binding" => Some(LayoutFlags::BINDING),
                    "texture" => Some(LayoutFlags::TEXTURE),
                    "sampler" => Some(LayoutFlags::SAMPLER),
                    "index" => Some(LayoutFlags::INDEX),
                    "set" => Some(LayoutFlags::SET),
                    "builtin" => Some(LayoutFlags::BUILTIN),
                    "input_attachment_index" => Some(LayoutFlags::INPUT_ATTACHMENT_INDEX),
                    "origin_upper_left" => Some(LayoutFlags::ORIGIN_UPPER_LEFT),
                    "blend_support_all_equations" => Some(LayoutFlags::BLEND_SUPPORT_ALL_EQUATIONS),
                    "push_constant" => Some(LayoutFlags::PUSH_CONSTANT),
                    "color" => Some(LayoutFlags::COLOR),
                    "vulkan" => Some(LayoutFlags::VULKAN),
                    "metal" => Some(LayoutFlags::METAL),
                    "webgpu" => Some(LayoutFlags::WEB_GPU),
                    "direct3d" => Some(LayoutFlags::DIRECT3D),
                    "rgba8" => Some(LayoutFlags::RGBA8),
                    "rgba32f" => Some(LayoutFlags::RGBA32F),
                    "r32f" => Some(LayoutFlags::R32F),
                    "local_size_x" => Some(LayoutFlags::LOCAL_SIZE_X),
                    "local_size_y" => Some(LayoutFlags::LOCAL_SIZE_Y),
                    "local_size_z" => Some(LayoutFlags::LOCAL_SIZE_Z),
                    _ => None,
                };

                if let Some(found) = found {
                    if result.flags.intersects(found) {
                        let msg = format!("layout qualifier '{text}' appears more than once");
                        self.error(t, &msg);
                    }

                    result.flags |= found;

                    if found == LayoutFlags::LOCATION {
                        result.location = self.layout_int();
                    } else if found == LayoutFlags::OFFSET {
                        result.offset = self.layout_int();
                    } else if found == LayoutFlags::BINDING {
                        result.binding = self.layout_int();
                    } else if found == LayoutFlags::INDEX {
                        result.index = self.layout_int();
                    } else if found == LayoutFlags::SET {
                        result.set = self.layout_int();
                    } else if found == LayoutFlags::TEXTURE {
                        result.texture = self.layout_int();
                    } else if found == LayoutFlags::SAMPLER {
                        result.sampler = self.layout_int();
                    } else if found == LayoutFlags::BUILTIN {
                        result.builtin = self.layout_int();
                    } else if found == LayoutFlags::INPUT_ATTACHMENT_INDEX {
                        result.input_attachment_index = self.layout_int();
                    } else if found == LayoutFlags::LOCAL_SIZE_X {
                        result.local_size_x = self.layout_int();
                    } else if found == LayoutFlags::LOCAL_SIZE_Y {
                        result.local_size_y = self.layout_int();
                    } else if found == LayoutFlags::LOCAL_SIZE_Z {
                        result.local_size_z = self.layout_int();
                    }
                } else {
                    let msg = format!("'{text}' is not a valid layout qualifier");
                    self.error(t, &msg);
                }
                if self.check_next(TokenKind::Rparen).is_some() {
                    break;
                }
                if self.expect(TokenKind::Comma, "','").is_none() {
                    break;
                }
            }
        }
        result
    }

    /// `layout? (UNIFORM | CONST | IN | OUT | INOUT | LOWP | MEDIUMP | HIGHP | FLAT |
    /// NOPERSPECTIVE | VARYING | INLINE | WORKGROUP | READONLY | WRITEONLY | BUFFER)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1182-L1212 (chrome/m156)
    fn modifiers(&mut self) -> Modifiers {
        let start = self.peek().offset;
        let layout = self.layout();
        let raw = self.next_raw_token();
        let mut end = raw.offset;
        if !is_whitespace(raw.kind) {
            self.pushback(raw);
        }
        let mut flags = ModifierFlags::empty();
        loop {
            let peeked = self.peek_kind();
            let token_flag = parse_modifier_token(peeked);
            if token_flag.is_empty() {
                break;
            }
            let modifier = self.next_token();
            let duplicate_flags = token_flag & flags;
            if !duplicate_flags.is_empty() {
                let msg = format!("'{}' appears more than once", duplicate_flags.description());
                self.error(modifier, &msg);
            }
            flags |= token_flag;
            end = self.position(modifier).end_offset();
        }
        Modifiers {
            position: Position::range(start, end),
            layout,
            flags,
        }
    }

    /// `statementOrNop`: `stmt`, or a `Nop` when it is missing, given the position `pos` if it
    /// has none.
    // Port of: src/sksl/SkSLParser.cpp#L1214-L1222 (chrome/m156)
    fn statement_or_nop(&mut self, pos: Position, stmt: Option<StmtId>) -> StmtId {
        let stmt = stmt.unwrap_or_else(|| Nop::make(&mut self.ctx.pool));
        if pos.valid() && !self.ctx.pool.statement(stmt).position.valid() {
            self.ctx.pool.statement_mut(stmt).position = pos;
        }
        stmt
    }

    /// `ifStatement | forStatement | doStatement | whileStatement | block | expression`.
    // Port of: src/sksl/SkSLParser.cpp#L1224-L1264 (chrome/m156)
    fn statement(&mut self) -> Option<StmtId> {
        self.statement_with_scope(true)
    }

    /// `statement(bracesIntroduceNewScope)`.
    fn statement_with_scope(&mut self, braces_introduce_new_scope: bool) -> Option<StmtId> {
        self.auto_depth(|this| {
            if !this.increase_depth() {
                return None;
            }
            match this.peek_kind() {
                Some(TokenKind::If) => this.if_statement(),
                Some(TokenKind::For) => this.for_statement(),
                Some(TokenKind::Do) => this.do_statement(),
                Some(TokenKind::While) => this.while_statement(),
                Some(TokenKind::Switch) => this.switch_statement(),
                Some(TokenKind::Return) => this.return_statement(),
                Some(TokenKind::Break) => this.break_statement(),
                Some(TokenKind::Continue) => this.continue_statement(),
                Some(TokenKind::Discard) => this.discard_statement(),
                Some(TokenKind::Lbrace) => this.block(braces_introduce_new_scope, None),
                Some(TokenKind::Semicolon) => {
                    this.next_token();
                    Some(Nop::make(&mut this.ctx.pool))
                }
                Some(TokenKind::Const) => this.var_declarations(),
                Some(
                    TokenKind::Highp | TokenKind::Mediump | TokenKind::Lowp | TokenKind::Identifier,
                ) => this.var_declarations_or_expression_statement(),
                _ => this.expression_statement(),
            }
        })
    }

    /// `findType`: the type named `name`, with the qualifiers in `modifiers` applied.
    // Port of: src/sksl/SkSLParser.cpp#L1266-L1291 (chrome/m156)
    fn find_type(&mut self, pos: Position, modifiers: &mut Modifiers, name: &str) -> TypeId {
        let table = self.symbol_table();
        let Some(symbol) = self.ctx.pool.find_symbol(table, name) else {
            self.error_at(pos, &format!("no symbol named '{name}'"));
            return TypeId::POISON;
        };
        let SymbolId::Type(ty) = symbol else {
            self.error_at(pos, &format!("symbol '{name}' is not a type"));
            return TypeId::POISON;
        };
        if !self.ctx.config().is_builtin_code() && !TypeReference::verify_type(self.ctx, ty, pos) {
            return TypeId::POISON;
        }
        let mut qualifier_range = modifiers.position;
        if qualifier_range.start_offset() == qualifier_range.end_offset() {
            qualifier_range = self.range_from(qualifier_range);
        }
        ty.apply_qualifiers(self.ctx, &mut modifiers.flags, qualifier_range)
    }

    /// `type`: `IDENTIFIER(type) (LBRACKET intLiteral? RBRACKET)* QUESTION?`.
    // Port of: src/sksl/SkSLParser.cpp#L1293-L1329 (chrome/m156)
    fn type_(&mut self, modifiers: &mut Modifiers) -> Option<TypeId> {
        let type_token = self.expect(TokenKind::Identifier, "a type")?;
        let type_text = self.text(type_token);
        if !self.is_type_name(&type_text) {
            let msg = format!("no type named '{type_text}'");
            self.error(type_token, &msg);
            return Some(TypeId::INVALID);
        }
        let pos = self.position(type_token);
        let mut result = self.find_type(pos, modifiers, &type_text);
        if self.ctx.pool.ty(result).is_interface_block() {
            // SkSL puts interface blocks into the symbol table, but they aren't general-purpose
            // types; you can't use them to declare a variable type or a function return type.
            let msg = format!("expected a type, found '{type_text}'");
            self.error(type_token, &msg);
            return Some(TypeId::INVALID);
        }
        while let Some(bracket) = self.check_next(TokenKind::Lbracket) {
            if self.check_next(TokenKind::Rbracket).is_some() {
                if self.allow_unsized_arrays() {
                    let range = self.range_from_token(type_token);
                    result = self.unsized_array_type(result, range);
                } else {
                    let range = self.range_from_token(bracket);
                    self.error_at(range, "unsized arrays are not permitted here");
                }
            } else {
                let size = self.array_size()?;
                self.expect(TokenKind::Rbracket, "']'");
                let range = self.range_from_token(type_token);
                result = self.array_type(result, size, range);
            }
        }
        Some(result)
    }

    /// `IDENTIFIER LBRACE varDeclaration+ RBRACE (IDENTIFIER (LBRACKET expression RBRACKET)*)?
    /// SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1331-L1425 (chrome/m156)
    #[allow(clippy::too_many_lines)] // Mirrors Skia's single function.
    fn interface_block(&mut self, modifiers: &Modifiers) -> bool {
        let Some(type_name) = self.expect_identifier() else {
            return false;
        };
        if self.peek_kind() != Some(TokenKind::Lbrace) {
            // we only get into interfaceBlock if we found a top-level identifier which was not a
            // type. 99% of the time, the user was not actually intending to create an interface
            // block, so it's better to report it as an unknown type
            let msg = format!("no type named '{}'", self.text(type_name));
            self.error(type_name, &msg);
            return false;
        }
        self.next_token();
        let mut fields: Vec<Field> = Vec::new();
        while self.check_next(TokenKind::Rbrace).is_none() {
            let peeked = self.peek();
            let field_pos = self.position(peeked);
            let mut field_modifiers = self.modifiers();
            let Some(ty) = self.type_(&mut field_modifiers) else {
                return false;
            };
            loop {
                let Some(field_name) = self.expect_identifier() else {
                    return false;
                };
                let mut actual_type = ty;
                if self.check_next(TokenKind::Lbracket).is_some() {
                    let size_token = self.peek();
                    if size_token.kind == Some(TokenKind::Rbracket) {
                        if self.allow_unsized_arrays() {
                            let pos = self.position(type_name);
                            actual_type = self.unsized_array_type(actual_type, pos);
                        } else {
                            self.error(size_token, "unsized arrays are not permitted here");
                        }
                    } else {
                        let Some(size) = self.array_size() else {
                            return false;
                        };
                        let pos = self.position(type_name);
                        actual_type = self.array_type(actual_type, size, pos);
                    }
                    self.expect(TokenKind::Rbracket, "']'");
                }

                let position = self.range_from(field_pos);
                fields.push(Field {
                    position,
                    layout: field_modifiers.layout,
                    modifier_flags: field_modifiers.flags,
                    name: self.text(field_name).into(),
                    ty: actual_type,
                });
                if self.check_next(TokenKind::Comma).is_none() {
                    break;
                }
            }

            if self.expect(TokenKind::Semicolon, "';'").is_none() {
                return false;
            }
        }
        let mut instance_name = Cow::Borrowed("");
        let mut size: SkslInt = 0;
        if let Some(instance_name_token) = self.check_identifier() {
            instance_name = self.text(instance_name_token);
            if self.check_next(TokenKind::Lbracket).is_some() {
                let Some(array_size) = self.array_size() else {
                    return false;
                };
                size = array_size;
                self.expect(TokenKind::Rbracket, "']'");
            }
        }
        self.expect(TokenKind::Semicolon, "';'");

        let pos = self.position(type_name);
        let type_name = self.text(type_name);
        // The size was validated against `INT32_MAX`.
        let size = i32::try_from(size).unwrap_or(i32::MAX);
        if let Some(ib) = InterfaceBlock::convert(
            self.ctx,
            pos,
            modifiers,
            &type_name,
            fields,
            &instance_name,
            size,
        ) {
            self.program_elements.push(ib);
            return true;
        }
        false
    }

    /// `IF LPAREN expression RPAREN statement (ELSE statement)?`.
    // Port of: src/sksl/SkSLParser.cpp#L1427-L1456 (chrome/m156)
    fn if_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::If, "'if'")?;
        self.expect(TokenKind::Lparen, "'('")?;
        let test = self.expression()?;
        self.expect(TokenKind::Rparen, "')'")?;
        let if_true = self.statement()?;
        let mut if_false = None;
        if self.check_next(TokenKind::Else).is_some() {
            if_false = Some(self.statement()?);
        }
        let pos = self.range_from_token(start);
        let converted = IfStatement::convert(self.ctx, pos, test, if_true, if_false);
        Some(self.statement_or_nop(pos, converted))
    }

    /// `DO statement WHILE LPAREN expression RPAREN SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1458-L1486 (chrome/m156)
    fn do_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Do, "'do'")?;
        let statement = self.statement()?;
        self.expect(TokenKind::While, "'while'")?;
        self.expect(TokenKind::Lparen, "'('")?;
        let test = self.expression()?;
        self.expect(TokenKind::Rparen, "')'")?;
        self.expect(TokenKind::Semicolon, "';'")?;
        let pos = self.range_from_token(start);
        let converted = DoStatement::convert(self.ctx, pos, statement, test);
        Some(self.statement_or_nop(pos, converted))
    }

    /// `WHILE LPAREN expression RPAREN STATEMENT`.
    // Port of: src/sksl/SkSLParser.cpp#L1488-L1509 (chrome/m156)
    fn while_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::While, "'while'")?;
        self.expect(TokenKind::Lparen, "'('")?;
        let test = self.expression()?;
        self.expect(TokenKind::Rparen, "')'")?;
        let statement = self.statement()?;
        let pos = self.range_from_token(start);
        let converted = ForStatement::convert_while(self.ctx, pos, test, statement);
        Some(self.statement_or_nop(pos, converted))
    }

    /// `COLON statement*`.
    // Port of: src/sksl/SkSLParser.cpp#L1511-L1531 (chrome/m156)
    fn switch_case_body(
        &mut self,
        values: &mut Vec<Option<ExprId>>,
        case_blocks: &mut Vec<StmtId>,
        case_value: Option<ExprId>,
    ) -> bool {
        if self.expect(TokenKind::Colon, "':'").is_none() {
            return false;
        }
        let mut statements: Vec<StmtId> = Vec::new();
        while self.peek_kind() != Some(TokenKind::Rbrace)
            && self.peek_kind() != Some(TokenKind::Case)
            && self.peek_kind() != Some(TokenKind::Default)
        {
            let Some(s) = self.statement() else {
                return false;
            };
            statements.push(s);
        }
        values.push(case_value);
        case_blocks.push(Block::make(
            &mut self.ctx.pool,
            Position::default(),
            statements,
            BlockKind::UnbracedBlock,
            None,
        ));
        true
    }

    /// `CASE expression COLON statement*`.
    // Port of: src/sksl/SkSLParser.cpp#L1533-L1546 (chrome/m156)
    fn switch_case(
        &mut self,
        values: &mut Vec<Option<ExprId>>,
        case_blocks: &mut Vec<StmtId>,
    ) -> bool {
        if self.expect(TokenKind::Case, "'case'").is_none() {
            return false;
        }
        let Some(case_value) = self.expression() else {
            return false;
        };
        self.switch_case_body(values, case_blocks, Some(case_value))
    }

    /// `SWITCH LPAREN expression RPAREN LBRACE switchCase* (DEFAULT COLON statement*)? RBRACE`.
    // Port of: src/sksl/SkSLParser.cpp#L1548-L1600 (chrome/m156)
    fn switch_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Switch, "'switch'")?;
        self.expect(TokenKind::Lparen, "'('")?;
        let value = self.expression()?;
        self.expect(TokenKind::Rparen, "')'")?;
        self.expect(TokenKind::Lbrace, "'{'")?;

        let mut values: Vec<Option<ExprId>> = Vec::new();
        let mut case_blocks: Vec<StmtId> = Vec::new();
        // Keeping a tight scope around the symbol table is important here.
        // `SwitchStatement::Convert` may end up creating a new symbol table if the
        // `HoistSwitchVarDeclarationsAtTopLevel` transform is used. We want the scope to end
        // first, so the context's active symbol table is the enclosing block's instead of the
        // switch's inner block.
        let (symbol_table, ok) = self.with_symbol_table(true, |this, _| {
            while this.peek_kind() == Some(TokenKind::Case) {
                if !this.switch_case(&mut values, &mut case_blocks) {
                    return false;
                }
            }
            // Requiring `default:` to be last (in defiance of C and GLSL) was a deliberate
            // decision. Other parts of the compiler are allowed to rely upon this assumption.
            if this.check_next(TokenKind::Default).is_some()
                && !this.switch_case_body(&mut values, &mut case_blocks, None)
            {
                return false;
            }
            this.expect(TokenKind::Rbrace, "'}'").is_some()
        });
        if !ok {
            return None;
        }

        let pos = self.range_from_token(start);
        let converted = SwitchStatement::convert(
            self.ctx,
            pos,
            value,
            values,
            case_blocks,
            symbol_table.expect("the switch's symbol table"),
        );
        Some(self.statement_or_nop(pos, converted))
    }

    /// `FOR LPAREN (declaration | expression)? SEMICOLON expression? SEMICOLON expression? RPAREN
    /// STATEMENT`.
    // Port of: src/sksl/SkSLParser.cpp#L1607-L1695 (chrome/m156)
    fn for_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::For, "'for'")?;
        let lparen = self.expect(TokenKind::Lparen, "'('")?;
        let (symbol_table, parts) = self.with_symbol_table(true, |this, _| {
            let next_token = this.peek();
            let mut initializer = None;
            let first_semicolon_offset;
            if next_token.kind == Some(TokenKind::Semicolon) {
                // An empty init-statement.
                first_semicolon_offset = this.next_token().offset;
            } else {
                // The init-statement must be an expression or variable declaration.
                initializer = Some(this.var_declarations_or_expression_statement()?);
                first_semicolon_offset = this.lexer.checkpoint().offset() - 1;
            }
            let mut test = None;
            if this.peek_kind() != Some(TokenKind::Semicolon) {
                test = Some(this.expression()?);
            }
            let second_semicolon = this.expect(TokenKind::Semicolon, "';'")?;
            let mut next = None;
            if this.peek_kind() != Some(TokenKind::Rparen) {
                next = Some(this.expression()?);
            }
            let rparen = this.expect(TokenKind::Rparen, "')'")?;
            let statement = this.statement_with_scope(false)?;
            Some(ForParts {
                initializer,
                test,
                next,
                statement,
                first_semicolon_offset,
                second_semicolon,
                rparen,
            })
        });
        let parts = parts?;
        let pos = self.range_from_token(start);
        let loop_positions = ForLoopPositions {
            init_position: range_of_at_least_one_char(
                lparen.offset + 1,
                parts.first_semicolon_offset,
            ),
            condition_position: range_of_at_least_one_char(
                parts.first_semicolon_offset + 1,
                parts.second_semicolon.offset,
            ),
            next_position: range_of_at_least_one_char(
                parts.second_semicolon.offset + 1,
                parts.rparen.offset,
            ),
        };
        let converted = ForStatement::convert(
            self.ctx,
            pos,
            loop_positions,
            parts.initializer,
            parts.test,
            parts.next,
            parts.statement,
            symbol_table,
        );
        Some(self.statement_or_nop(pos, converted))
    }

    /// `RETURN expression? SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1697-L1715 (chrome/m156)
    fn return_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Return, "'return'")?;
        let mut expression = None;
        if self.peek_kind() != Some(TokenKind::Semicolon) {
            expression = Some(self.expression()?);
        }
        self.expect(TokenKind::Semicolon, "';'")?;
        // We do not check for errors, or coerce the value to the correct type, until the return
        // statement is actually added to a function. (This is done in
        // `FunctionDefinition::Convert`.)
        let pos = self.range_from_token(start);
        Some(ReturnStatement::make(&mut self.ctx.pool, pos, expression))
    }

    /// `BREAK SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1717-L1727 (chrome/m156)
    fn break_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Break, "'break'")?;
        self.expect(TokenKind::Semicolon, "';'")?;
        let pos = self.position(start);
        Some(BreakStatement::make(&mut self.ctx.pool, pos))
    }

    /// `CONTINUE SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1729-L1739 (chrome/m156)
    fn continue_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Continue, "'continue'")?;
        self.expect(TokenKind::Semicolon, "';'")?;
        let pos = self.position(start);
        Some(ContinueStatement::make(&mut self.ctx.pool, pos))
    }

    /// `DISCARD SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1741-L1752 (chrome/m156)
    fn discard_statement(&mut self) -> Option<StmtId> {
        let start = self.expect(TokenKind::Discard, "'discard'")?;
        self.expect(TokenKind::Semicolon, "';'")?;
        let pos = self.position(start);
        let converted = DiscardStatement::convert(self.ctx, pos);
        Some(self.statement_or_nop(pos, converted))
    }

    /// `LBRACE statement* RBRACE`. A block either introduces a new scope, adopts an existing
    /// symbol table (a function body, whose parameters are in it), or has none.
    // Port of: src/sksl/SkSLParser.cpp#L1754-L1795 (chrome/m156)
    fn block(
        &mut self,
        introduce_new_scope: bool,
        adopt_existing_symbol_table: Option<SymTabId>,
    ) -> Option<StmtId> {
        // We can't introduce a new scope _and_ adopt an existing symbol table.
        debug_assert!(!(introduce_new_scope && adopt_existing_symbol_table.is_some()));

        self.auto_depth(|this| {
            let start = this.expect(TokenKind::Lbrace, "'{'")?;
            if !this.increase_depth() {
                return None;
            }

            let mut statements: Vec<StmtId> = Vec::new();
            let (new_symbol_table, ok) = this.with_symbol_table(introduce_new_scope, |this, _| {
                // Consume statements until we reach the closing brace.
                loop {
                    let token_kind = this.peek_kind();
                    if token_kind == Some(TokenKind::Rbrace) {
                        this.next_token();
                        break;
                    }
                    if token_kind == Some(TokenKind::EndOfFile) {
                        let peeked = this.peek();
                        this.error(peeked, "expected '}', but found end of file");
                        return false;
                    }
                    if let Some(statement) = this.statement() {
                        statements.push(statement);
                    }
                    if this.encountered_fatal_error {
                        return false;
                    }
                }
                true
            });
            if !ok {
                return None;
            }
            let symbol_table_to_use = adopt_existing_symbol_table.or(new_symbol_table);
            let pos = this.range_from_token(start);
            Some(Block::make_block(
                &mut this.ctx.pool,
                pos,
                statements,
                BlockKind::BracedScope,
                symbol_table_to_use,
            ))
        })
    }

    /// `expression SEMICOLON`.
    // Port of: src/sksl/SkSLParser.cpp#L1797-L1809 (chrome/m156)
    fn expression_statement(&mut self) -> Option<StmtId> {
        let expr = self.expression()?;
        self.expect(TokenKind::Semicolon, "';'")?;
        let pos = self.expr_position(expr);
        let converted = ExpressionStatement::convert(self.ctx, expr);
        Some(self.statement_or_nop(pos, converted))
    }

    /// `poison`.
    // Port of: src/sksl/SkSLParser.cpp#L1811-L1813 (chrome/m156)
    fn poison(&mut self, pos: Position) -> ExprId {
        Poison::make(self.ctx, pos)
    }

    /// `expressionOrPoison`: `expr`, or a poison expression when it is missing.
    // Port of: src/sksl/SkSLParser.cpp#L1815-L1827 (chrome/m156)
    fn expression_or_poison(&mut self, pos: Position, expr: Option<ExprId>) -> ExprId {
        // If no expression was passed in, create a poison expression.
        let expr = expr.unwrap_or_else(|| self.poison(pos));
        // If a valid position was passed in, it must match the expression's position.
        debug_assert!(
            !pos.valid() || self.expr_position(expr) == pos,
            "expected expression position ({}-{}), but received ({}-{})",
            pos.start_offset(),
            pos.end_offset(),
            self.expr_position(expr).start_offset(),
            self.expr_position(expr).end_offset(),
        );
        expr
    }

    /// `operatorRight`: consumes the operator, parses its right operand with `right_fn`, and
    /// folds the pair into `expr`. Returns false on a parse error.
    // Port of: src/sksl/SkSLParser.cpp#L1829-L1846 (chrome/m156)
    fn operator_right(
        &mut self,
        op: OperatorKind,
        right_fn: fn(&mut Self) -> Option<ExprId>,
        expr: &mut ExprId,
    ) -> bool {
        self.next_token();
        if !self.increase_depth() {
            return false;
        }
        let Some(right) = right_fn(self) else {
            return false;
        };
        let pos = self
            .expr_position(*expr)
            .range_through(self.expr_position(right));
        let converted = BinaryExpression::convert(self.ctx, pos, *expr, Operator::from(op), right);
        *expr = self.expression_or_poison(pos, converted);
        true
    }

    /// `assignmentExpression (COMMA assignmentExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1848-L1873 (chrome/m156)
    fn expression(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let start = this.peek();
            let mut result = this.assignment_expression()?;
            while this.peek_kind() == Some(TokenKind::Comma) {
                if !this.operator_right(
                    OperatorKind::Comma,
                    Self::assignment_expression,
                    &mut result,
                ) {
                    return None;
                }
            }
            debug_assert!(
                this.expr_position(result).valid(),
                "Expression {} has invalid position",
                this.ctx.pool.expression_description(result)
            );
            debug_assert!(
                this.expr_position(result).start_offset() == this.position(start).start_offset(),
                "Expected {} to start at {} (first token: '{}'), but it has range {}-{}",
                this.ctx.pool.expression_description(result),
                this.position(start).start_offset(),
                this.text(start),
                this.expr_position(result).start_offset(),
                this.expr_position(result).end_offset(),
            );
            Some(result)
        })
    }

    /// `ternaryExpression ((EQEQ | STAREQ | SLASHEQ | PERCENTEQ | PLUSEQ | MINUSEQ | SHLEQ |
    /// SHREQ | BITWISEANDEQ | BITWISEXOREQ | BITWISEOREQ | LOGICALANDEQ | LOGICALXOREQ |
    /// LOGICALOREQ) assignmentExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1875-L1906 (chrome/m156)
    fn assignment_expression(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let mut result = this.ternary_expression()?;
            loop {
                let op = match this.peek_kind() {
                    Some(TokenKind::Eq) => OperatorKind::Eq,
                    Some(TokenKind::Stareq) => OperatorKind::StarEq,
                    Some(TokenKind::Slasheq) => OperatorKind::SlashEq,
                    Some(TokenKind::Percenteq) => OperatorKind::PercentEq,
                    Some(TokenKind::Pluseq) => OperatorKind::PlusEq,
                    Some(TokenKind::Minuseq) => OperatorKind::MinusEq,
                    Some(TokenKind::Shleq) => OperatorKind::ShlEq,
                    Some(TokenKind::Shreq) => OperatorKind::ShrEq,
                    Some(TokenKind::Bitwiseandeq) => OperatorKind::BitwiseAndEq,
                    Some(TokenKind::Bitwisexoreq) => OperatorKind::BitwiseXorEq,
                    Some(TokenKind::Bitwiseoreq) => OperatorKind::BitwiseOrEq,
                    _ => return Some(result),
                };
                if !this.operator_right(op, Self::assignment_expression, &mut result) {
                    return None;
                }
            }
        })
    }

    /// `logicalOrExpression ('?' expression ':' assignmentExpression)?`.
    // Port of: src/sksl/SkSLParser.cpp#L1908-L1937 (chrome/m156)
    fn ternary_expression(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let base = this.logical_or_expression()?;
            if this.check_next(TokenKind::Question).is_none() {
                return Some(base);
            }
            if !this.increase_depth() {
                return None;
            }
            let true_expr = this.expression()?;
            this.expect(TokenKind::Colon, "':'")?;
            let false_expr = this.assignment_expression()?;
            let pos = this
                .expr_position(base)
                .range_through(this.expr_position(false_expr));
            let converted = TernaryExpression::convert(this.ctx, pos, base, true_expr, false_expr);
            Some(this.expression_or_poison(pos, converted))
        })
    }

    /// One left-associative binary level, `next (OP next)*`, as Skia writes out `logicalOr`
    /// through `bitwiseAnd`: `next` parses both operands.
    fn binary_level(
        &mut self,
        next: fn(&mut Self) -> Option<ExprId>,
        token: TokenKind,
        op: OperatorKind,
    ) -> Option<ExprId> {
        self.auto_depth(|this| {
            let mut result = next(this)?;
            while this.peek_kind() == Some(token) {
                if !this.operator_right(op, next, &mut result) {
                    return None;
                }
            }
            Some(result)
        })
    }

    /// One left-associative level over several operators (`equality` through `multiplicative`).
    fn binary_level_ops(
        &mut self,
        next: fn(&mut Self) -> Option<ExprId>,
        op_for: fn(Option<TokenKind>) -> Option<OperatorKind>,
    ) -> Option<ExprId> {
        self.auto_depth(|this| {
            let mut result = next(this)?;
            loop {
                let Some(op) = op_for(this.peek_kind()) else {
                    return Some(result);
                };
                if !this.operator_right(op, next, &mut result) {
                    return None;
                }
            }
        })
    }

    /// `logicalXorExpression (LOGICALOR logicalXorExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1939-L1953 (chrome/m156)
    fn logical_or_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::logical_xor_expression,
            TokenKind::Logicalor,
            OperatorKind::LogicalOr,
        )
    }

    /// `logicalAndExpression (LOGICALXOR logicalAndExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1955-L1969 (chrome/m156)
    fn logical_xor_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::logical_and_expression,
            TokenKind::Logicalxor,
            OperatorKind::LogicalXor,
        )
    }

    /// `bitwiseOrExpression (LOGICALAND bitwiseOrExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1971-L1985 (chrome/m156)
    fn logical_and_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::bitwise_or_expression,
            TokenKind::Logicaland,
            OperatorKind::LogicalAnd,
        )
    }

    /// `bitwiseXorExpression (BITWISEOR bitwiseXorExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L1987-L2001 (chrome/m156)
    fn bitwise_or_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::bitwise_xor_expression,
            TokenKind::Bitwiseor,
            OperatorKind::BitwiseOr,
        )
    }

    /// `bitwiseAndExpression (BITWISEXOR bitwiseAndExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2003-L2017 (chrome/m156)
    fn bitwise_xor_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::bitwise_and_expression,
            TokenKind::Bitwisexor,
            OperatorKind::BitwiseXor,
        )
    }

    /// `equalityExpression (BITWISEAND equalityExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2019-L2033 (chrome/m156)
    fn bitwise_and_expression(&mut self) -> Option<ExprId> {
        self.binary_level(
            Self::equality_expression,
            TokenKind::Bitwiseand,
            OperatorKind::BitwiseAnd,
        )
    }

    /// `relationalExpression ((EQEQ | NEQ) relationalExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2035-L2055 (chrome/m156)
    fn equality_expression(&mut self) -> Option<ExprId> {
        self.binary_level_ops(Self::relational_expression, |kind| match kind {
            Some(TokenKind::Eqeq) => Some(OperatorKind::EqEq),
            Some(TokenKind::Neq) => Some(OperatorKind::Neq),
            _ => None,
        })
    }

    /// `shiftExpression ((LT | GT | LTEQ | GTEQ) shiftExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2057-L2079 (chrome/m156)
    fn relational_expression(&mut self) -> Option<ExprId> {
        self.binary_level_ops(Self::shift_expression, |kind| match kind {
            Some(TokenKind::Lt) => Some(OperatorKind::Lt),
            Some(TokenKind::Gt) => Some(OperatorKind::Gt),
            Some(TokenKind::Lteq) => Some(OperatorKind::LtEq),
            Some(TokenKind::Gteq) => Some(OperatorKind::GtEq),
            _ => None,
        })
    }

    /// `additiveExpression ((SHL | SHR) additiveExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2081-L2101 (chrome/m156)
    fn shift_expression(&mut self) -> Option<ExprId> {
        self.binary_level_ops(Self::additive_expression, |kind| match kind {
            Some(TokenKind::Shl) => Some(OperatorKind::Shl),
            Some(TokenKind::Shr) => Some(OperatorKind::Shr),
            _ => None,
        })
    }

    /// `multiplicativeExpression ((PLUS | MINUS) multiplicativeExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2103-L2123 (chrome/m156)
    fn additive_expression(&mut self) -> Option<ExprId> {
        self.binary_level_ops(Self::multiplicative_expression, |kind| match kind {
            Some(TokenKind::Plus) => Some(OperatorKind::Plus),
            Some(TokenKind::Minus) => Some(OperatorKind::Minus),
            _ => None,
        })
    }

    /// `unaryExpression ((STAR | SLASH | PERCENT) unaryExpression)*`.
    // Port of: src/sksl/SkSLParser.cpp#L2125-L2146 (chrome/m156)
    fn multiplicative_expression(&mut self) -> Option<ExprId> {
        self.binary_level_ops(Self::unary_expression, |kind| match kind {
            Some(TokenKind::Star) => Some(OperatorKind::Star),
            Some(TokenKind::Slash) => Some(OperatorKind::Slash),
            Some(TokenKind::Percent) => Some(OperatorKind::Percent),
            _ => None,
        })
    }

    /// `postfixExpression | (PLUS | MINUS | NOT | PLUSPLUS | MINUSMINUS) unaryExpression`.
    // Port of: src/sksl/SkSLParser.cpp#L2148-L2174 (chrome/m156)
    fn unary_expression(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let start = this.peek();
            let op = match start.kind {
                Some(TokenKind::Plus) => OperatorKind::Plus,
                Some(TokenKind::Minus) => OperatorKind::Minus,
                Some(TokenKind::Logicalnot) => OperatorKind::LogicalNot,
                Some(TokenKind::Bitwisenot) => OperatorKind::BitwiseNot,
                Some(TokenKind::Plusplus) => OperatorKind::PlusPlus,
                Some(TokenKind::Minusminus) => OperatorKind::MinusMinus,
                _ => return this.postfix_expression(),
            };
            this.next_token();
            if !this.increase_depth() {
                return None;
            }
            let expr = this.unary_expression()?;
            let pos = Position::range(start.offset, this.expr_position(expr).end_offset());
            let converted = PrefixExpression::convert(this.ctx, pos, Operator::from(op), expr);
            Some(this.expression_or_poison(pos, converted))
        })
    }

    /// `term suffix*`.
    // Port of: src/sksl/SkSLParser.cpp#L2176-L2205 (chrome/m156)
    fn postfix_expression(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let mut result = this.term()?;
            loop {
                let t = this.peek();
                match t.kind {
                    Some(TokenKind::FloatLiteral)
                        if this.text(t).as_bytes().first() != Some(&b'.') =>
                    {
                        return Some(result);
                    }
                    Some(
                        TokenKind::FloatLiteral
                        | TokenKind::Lbracket
                        | TokenKind::Dot
                        | TokenKind::Lparen
                        | TokenKind::Plusplus
                        | TokenKind::Minusminus,
                    ) => {
                        if !this.increase_depth() {
                            return None;
                        }
                        result = this.suffix(result)?;
                    }
                    _ => return Some(result),
                }
            }
        })
    }

    /// `swizzle`: a field access on a non-vector base, a swizzle otherwise.
    // Port of: src/sksl/SkSLParser.cpp#L2207-L2219 (chrome/m156)
    fn swizzle(
        &mut self,
        pos: Position,
        base: ExprId,
        swizzle_mask: &str,
        mask_pos: Position,
    ) -> ExprId {
        debug_assert_ne!(swizzle_mask, "");
        let base_type = self.ctx.pool.expression(base).ty;
        let (is_vector, is_scalar) = {
            let t = self.ctx.pool.ty(base_type);
            (t.is_vector(), t.is_scalar())
        };
        let converted = if !is_vector && !is_scalar {
            FieldAccess::convert(self.ctx, pos, base, swizzle_mask)
        } else {
            Swizzle::convert(self.ctx, pos, mask_pos, base, swizzle_mask)
        };
        self.expression_or_poison(pos, converted)
    }

    /// `call`.
    // Port of: src/sksl/SkSLParser.cpp#L2221-L2229 (chrome/m156)
    fn call(&mut self, pos: Position, base: ExprId, args: Vec<ExprId>) -> ExprId {
        let converted = FunctionCall::convert(self.ctx, pos, base, args);
        self.expression_or_poison(pos, converted)
    }

    /// `LBRACKET expression? RBRACKET | DOT IDENTIFIER | LPAREN arguments RPAREN | PLUSPLUS |
    /// MINUSMINUS | COLONCOLON IDENTIFIER | FLOAT_LITERAL [IDENTIFIER]`.
    // Port of: src/sksl/SkSLParser.cpp#L2231-L2339 (chrome/m156)
    #[allow(clippy::too_many_lines)] // Mirrors Skia's single function.
    fn suffix(&mut self, base: ExprId) -> Option<ExprId> {
        self.auto_depth(|this| {
            let next = this.next_token();
            if !this.increase_depth() {
                return None;
            }
            let base_pos = this.expr_position(base);
            match next.kind {
                Some(TokenKind::Lbracket) => {
                    if this.check_next(TokenKind::Rbracket).is_some() {
                        let range = this.range_from_token(next);
                        this.error_at(range, "missing index in '[]'");
                        let pos = this.range_from(base_pos);
                        return Some(this.poison(pos));
                    }
                    let index = this.expression()?;
                    this.expect(
                        TokenKind::Rbracket,
                        "']' to complete array access expression",
                    );

                    let pos = this.range_from(base_pos);
                    let converted = IndexExpression::convert(this.ctx, pos, base, index);
                    Some(this.expression_or_poison(pos, converted))
                }
                Some(TokenKind::Dot) => {
                    if let Some(text) = this.identifier() {
                        let pos = this.range_from(base_pos);
                        let mask_pos = this.range_from(this.position(next).after());
                        return Some(this.swizzle(pos, base, &text, mask_pos));
                    }
                    Some(this.suffix_float_literal(next, base))
                }
                Some(TokenKind::FloatLiteral) => Some(this.suffix_float_literal(next, base)),
                Some(TokenKind::Lparen) => {
                    let mut args: Vec<ExprId> = Vec::new();
                    if this.peek_kind() != Some(TokenKind::Rparen) {
                        loop {
                            let expr = this.assignment_expression()?;
                            args.push(expr);
                            if this.check_next(TokenKind::Comma).is_none() {
                                break;
                            }
                        }
                    }
                    this.expect(TokenKind::Rparen, "')' to complete function arguments");
                    let pos = this.range_from(base_pos);
                    Some(this.call(pos, base, args))
                }
                Some(TokenKind::Plusplus | TokenKind::Minusminus) => {
                    let op = if next.kind == Some(TokenKind::Plusplus) {
                        OperatorKind::PlusPlus
                    } else {
                        OperatorKind::MinusMinus
                    };
                    let pos = this.range_from(base_pos);
                    let converted =
                        PostfixExpression::convert(this.ctx, pos, base, Operator::from(op));
                    Some(this.expression_or_poison(pos, converted))
                }
                _ => {
                    let mut msg = b"expected expression suffix, but found '".to_vec();
                    msg.extend_from_slice(this.text_bytes(next));
                    msg.push(b'\'');
                    this.error_bytes(next, &msg);
                    None
                }
            }
        })
    }

    /// The `TK_FLOAT_LITERAL` case of `suffix` (which `TK_DOT` falls through to when no
    /// identifier follows the dot).
    // Port of: src/sksl/SkSLParser.cpp#L2273-L2305 (chrome/m156)
    fn suffix_float_literal(&mut self, next: Token, base: ExprId) -> ExprId {
        // Swizzles that start with a constant number, e.g. '.000r', will be tokenized as
        // floating point literals, possibly followed by an identifier. Handle that here.
        let token_text = self.text(next);
        debug_assert!(token_text.starts_with('.'));
        let field = &token_text[1..];
        // use the next *raw* token so we don't ignore whitespace - we only care about
        // identifiers that directly follow the float
        let base_pos = self.expr_position(base);
        let mut pos = self.range_from(base_pos);
        let mut start = self.position(next);
        // skip past the "."
        start = Position::range(start.start_offset() + 1, start.end_offset());
        let mut mask_pos = self.range_from(start);
        let id = self.next_raw_token();
        if id.kind == Some(TokenKind::Identifier) {
            pos = self.range_from(base_pos);
            mask_pos = self.range_from(start);
            let mask = format!("{field}{}", self.text(id));
            return self.swizzle(pos, base, &mask, mask_pos);
        }
        if field.is_empty() {
            self.error_at(pos, "expected field name or swizzle mask after '.'");
            return self.poison(pos);
        }
        self.pushback(id);
        self.swizzle(pos, base, field, mask_pos)
    }

    /// `IDENTIFIER | intLiteral | floatLiteral | boolLiteral | '(' expression ')'`.
    // Port of: src/sksl/SkSLParser.cpp#L2341-L2412 (chrome/m156)
    fn term(&mut self) -> Option<ExprId> {
        self.auto_depth(|this| {
            let t = this.peek();
            match t.kind {
                Some(TokenKind::Identifier) => {
                    if let Some(text) = this.identifier() {
                        let pos = this.position(t);
                        let table = this.symbol_table();
                        let instantiated = instantiate_symbol_ref(this.ctx, table, &text, pos);
                        return Some(this.expression_or_poison(pos, instantiated));
                    }
                }
                Some(TokenKind::IntLiteral) => {
                    let i = this.int_literal().unwrap_or(0);
                    let pos = this.position(t);
                    let literal = Literal::make_int_literal(&mut this.ctx.pool, pos, i);
                    return Some(this.expression_or_poison(pos, Some(literal)));
                }
                Some(TokenKind::FloatLiteral) => {
                    let f = this.float_literal().unwrap_or(0.0);
                    let pos = this.position(t);
                    let literal = Literal::make_float_literal(&mut this.ctx.pool, pos, f);
                    return Some(this.expression_or_poison(pos, Some(literal)));
                }
                Some(TokenKind::TrueLiteral | TokenKind::FalseLiteral) => {
                    let b = this
                        .bool_literal()
                        .expect("a true or false literal token is a bool literal");
                    let pos = this.position(t);
                    let literal = Literal::make_bool_literal(&mut this.ctx.pool, pos, b);
                    return Some(this.expression_or_poison(pos, Some(literal)));
                }
                Some(TokenKind::Lparen) => {
                    this.next_token();
                    if !this.increase_depth() {
                        return None;
                    }
                    if let Some(result) = this.expression() {
                        this.expect(TokenKind::Rparen, "')' to complete expression");
                        let range = this.range_from_token(t);
                        this.ctx.pool.expression_mut(result).position = range;
                        return Some(result);
                    }
                }
                _ => {
                    this.next_token();
                    let msg = format!("expected expression, but found '{}'", this.text(t));
                    this.error(t, &msg);
                    this.encountered_fatal_error = true;
                }
            }
            None
        })
    }

    /// `INT_LITERAL`.
    // Port of: src/sksl/SkSLParser.cpp#L2414-L2426 (chrome/m156)
    fn int_literal(&mut self) -> Option<SkslInt> {
        let t = self.expect(TokenKind::IntLiteral, "integer literal")?;
        let s = self.text(t);
        let Some(value) = stoi(&s) else {
            let msg = format!("integer is too large: {s}");
            self.error(t, &msg);
            return None;
        };
        Some(value)
    }

    /// `FLOAT_LITERAL`.
    // Port of: src/sksl/SkSLParser.cpp#L2428-L2440 (chrome/m156)
    fn float_literal(&mut self) -> Option<SkslFloat> {
        let t = self.expect(TokenKind::FloatLiteral, "float literal")?;
        let s = self.text(t);
        let Some(value) = stod_float(&s) else {
            let msg = format!("floating-point value is too large: {s}");
            self.error(t, &msg);
            return None;
        };
        Some(value)
    }

    /// `TRUE_LITERAL | FALSE_LITERAL`.
    // Port of: src/sksl/SkSLParser.cpp#L2442-L2457 (chrome/m156)
    fn bool_literal(&mut self) -> Option<bool> {
        let t = self.next_token();
        match t.kind {
            Some(TokenKind::TrueLiteral) => Some(true),
            Some(TokenKind::FalseLiteral) => Some(false),
            _ => {
                let msg = format!("expected 'true' or 'false', but found '{}'", self.text(t));
                self.error(t, &msg);
                None
            }
        }
    }

    /// `IDENTIFIER`.
    // Port of: src/sksl/SkSLParser.cpp#L2459-L2467 (chrome/m156)
    fn identifier(&mut self) -> Option<Cow<'a, str>> {
        let t = self.expect(TokenKind::Identifier, "identifier")?;
        Some(self.text(t))
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::context::Context;
    use crate::error_reporter::{ErrorReporter, ErrorSink};
    use crate::ir::{ElemId, SymbolId, SymbolTable, TypeId};
    use crate::modules::ModuleType;
    use crate::position::Position;
    use crate::program_settings::{ProgramConfig, ProgramKind, ProgramSettings, Version};

    /// The types these tests use, as the root module declares them.
    const TYPES: &[TypeId] = &[
        TypeId::VOID,
        TypeId::FLOAT,
        TypeId::FLOAT2,
        TypeId::HALF,
        TypeId::HALF4,
        TypeId::INT,
        TypeId::BOOL,
    ];

    /// What a parse produced.
    struct Outcome {
        elements: Vec<ElemId>,
        messages: Vec<String>,
        required_version: Version,
    }

    fn parse(kind: ProgramKind, text: &str) -> Outcome {
        let mut ctx = Context::new(ErrorReporter::forwarding());
        let settings = ProgramSettings::default();
        ctx.config = Some(ProgramConfig::new(ModuleType::Program, kind, settings));
        let table = ctx.pool.add_symbol_table(SymbolTable::new(None, true));
        for &ty in TYPES {
            ctx.pool.inject_symbol(table, SymbolId::Type(ty));
        }
        ctx.symbol_table = Some(table);
        let elements =
            Parser::new(&mut ctx, settings, kind, text.as_bytes()).module_inheriting_from();
        let messages = match ctx.errors.sink() {
            ErrorSink::Forwarding { errors } => errors
                .iter()
                .map(|(m, _)| String::from_utf8(m.clone()).expect("a UTF-8 message"))
                .collect(),
            other => panic!("expected a forwarding reporter, found {other:?}"),
        };
        Outcome {
            elements,
            messages,
            required_version: ctx.config().required_sksl_version,
        }
    }

    fn messages(text: &str) -> Vec<String> {
        parse(ProgramKind::Fragment, text).messages
    }

    fn first_message(text: &str) -> Option<String> {
        messages(text).into_iter().next()
    }

    #[test]
    fn nesting_past_the_limit_is_a_fatal_error() {
        let deep = format!(
            "void f() {{ int x = {}1{}; }}",
            "(".repeat(60),
            ")".repeat(60)
        );
        assert_eq!(
            first_message(&deep).as_deref(),
            Some("exceeded max parse depth")
        );
        let ok = format!(
            "void f() {{ int x = {}1{}; }}",
            "(".repeat(20),
            ")".repeat(20)
        );
        assert_eq!(messages(&ok), Vec::<String>::new());
    }

    #[test]
    fn version_directive() {
        let version = |text| parse(ProgramKind::Fragment, text).required_version;
        assert_eq!(version("#version 300\n"), Version::K300);
        assert_eq!(version("#version 100\n"), Version::K100);
        assert_eq!(messages("#version 400\n"), ["unsupported version number"]);
        assert_eq!(
            messages("#version 300 int x;"),
            ["invalid #version directive"]
        );
        assert_eq!(
            messages("int x;\n#version 300\n"),
            [
                "#version directive must appear before anything else",
                "expected a type, but found '300'"
            ]
        );
        assert_eq!(
            messages("#version 99999999999\n"),
            ["integer is too large: 99999999999"]
        );
    }

    #[test]
    fn directives() {
        assert_eq!(
            messages("#pragma once\n"),
            ["unsupported directive '#pragma'", "no type named 'once'"]
        );
        let ok = parse(
            ProgramKind::Fragment,
            "#extension GL_EXT_foo : enable\nint x;\n",
        );
        assert_eq!(ok.messages, Vec::<String>::new());
        assert_eq!(ok.elements.len(), 2, "an extension and a variable");
        assert_eq!(
            messages("#extension GL_EXT_foo : enable int x;"),
            ["invalid #extension directive"]
        );
        assert_eq!(
            messages("#extension float : enable\n"),
            ["expected an identifier, but found type 'float'"]
        );
        assert_eq!(
            messages("#extension a enable\n"),
            ["expected ':', but found 'enable'"]
        );
    }

    #[test]
    fn layout_qualifiers() {
        assert_eq!(
            first_message("layout(bogus) int x;").as_deref(),
            Some("'bogus' is not a valid layout qualifier")
        );
        assert_eq!(
            first_message("layout(binding = 1, binding = 2) uniform int x;").as_deref(),
            Some("layout qualifier 'binding' appears more than once")
        );
        assert_eq!(
            first_message("layout(location = 99999999999) in int x;").as_deref(),
            Some("value in layout is too large: 99999999999")
        );
        assert_eq!(
            first_message("layout(location = x) in int x;").as_deref(),
            Some("expected a non-negative integer, but found 'x'")
        );
        assert_eq!(
            first_message("layout(location 1) in int x;").as_deref(),
            Some("expected '=', but found '1'")
        );
        assert_eq!(
            first_message("layout(location = 1 binding = 2) in int x;").as_deref(),
            Some("expected ',', but found 'binding'")
        );
    }

    #[test]
    fn modifiers_report_repeats() {
        assert_eq!(
            first_message("const const int x = 1;").as_deref(),
            Some("'const' appears more than once")
        );
    }

    #[test]
    fn private_names_need_a_program_kind_that_allows_them() {
        assert_eq!(
            parse(ProgramKind::Fragment, "int $x;").messages,
            Vec::<String>::new()
        );
        assert_eq!(
            parse(ProgramKind::RuntimeShader, "int $x;").messages,
            ["name '$x' is reserved"]
        );
        assert_eq!(messages("int asm;"), ["name 'asm' is reserved"]);
    }

    #[test]
    fn speculative_declarations_rewind_without_reporting() {
        // `float(x);` starts like a declaration; the failed attempt must leave no trace.
        let outcome = parse(
            ProgramKind::Fragment,
            "void f() { float x = 1; float(x); half4 y; float2(1); }",
        );
        assert_eq!(outcome.messages, Vec::<String>::new());
        assert_eq!(outcome.elements.len(), 1);
    }

    #[test]
    fn checkpoints_forward_conversion_errors_of_accepted_declarations() {
        let outcome = parse(ProgramKind::Fragment, "void f() { int x = 1.5; }");
        assert_eq!(outcome.messages, ["expected 'int', but found 'float'"]);
    }

    #[test]
    fn expression_and_literal_errors() {
        assert_eq!(
            messages("void f() { float x = 1; x.; }"),
            [
                "expected identifier, but found ';'",
                "expected field name or swizzle mask after '.'",
                "expected ';', but found '}'"
            ]
        );
        assert_eq!(
            messages("void f() { float x = 1e999; }"),
            ["floating-point value is too large: 1e999"]
        );
        assert_eq!(
            messages("int x = 1 +;"),
            ["expected expression, but found ';'"]
        );
        assert_eq!(
            messages("int x = 08;"),
            [
                "'08' is not a valid octal number",
                "expected expression, but found '08'"
            ]
        );
    }

    #[test]
    fn unexpected_tokens_end_the_parse() {
        assert_eq!(
            messages("int x;;"),
            ["expected a declaration, but found ';'"]
        );
        assert_eq!(messages("int x = 1 `"), ["expected ';', but found '`'"]);
        assert_eq!(messages("int x; `"), ["invalid token"]);
    }

    #[test]
    fn programs_of_8mb_or_more_are_refused() {
        let text = " ".repeat(usize::try_from(Position::MAX_OFFSET).unwrap());
        assert_eq!(messages(&text), ["program is too large"]);
    }
}
