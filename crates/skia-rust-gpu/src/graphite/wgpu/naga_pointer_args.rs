//! Storage-buffer pointer parameters for naga (`docs/design/gpu.md` §6.3, W4).
//!
//! `SkSL` functions that take an unsized array parameter (the more-than-8-stops gradients read
//! their stops from a storage buffer) become WGSL functions with a `ptr<storage, array<f32>,
//! read>` parameter, and the call sites pass `&(_storage2.fsStorageBuffer)`. Dawn's Tint accepts
//! that: it is the WGSL language feature `unrestricted_pointer_parameters`. naga 30 does not
//! implement it (its front end rejects the `requires` directive and its validator accepts only
//! `private` and `function` pointer parameters), so [`prepare_shader_source`] rewrites the naga IR
//! of such a module before wgpu sees it.
//!
//! The rewrite is a specialisation: every function with a storage-pointer parameter is cloned
//! once per distinct set of globals its callers pass, the parameter is dropped, and the uses of
//! the parameter become the access chain on the global (`GlobalVariable`, then `AccessIndex`
//! for each member). The clones are specialised in turn through their own calls. WGSL has no
//! recursion, so this terminates.
//!
//! The generated WGSL text is not touched: the transformation runs only where the backend creates
//! the shader module, and only for a module that naga rejects with `InvalidArgumentPointerSpace`
//! on a `storage` pointer. A call site that passes anything but a chain of constant member
//! accesses on a storage global is an error, never a guess.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use naga::{
    Arena, Block, Expression, Function, GlobalVariable, Handle, Module, Range, SampleLevel,
    Statement,
};

/// What [`prepare_shader_source`] decided.
#[derive(Debug)]
pub enum PreparedSource<'a> {
    /// Hand wgpu the WGSL text unchanged (every module that naga accepts, and every one it rejects
    /// for another reason: wgpu reports that error itself).
    Wgsl(&'a str),
    /// The module after the rewrite; it validates.
    Rewritten(Box<Module>),
}

/// Why the rewrite gave up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteError(pub String);

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "storage pointer parameter rewrite: {}", self.0)
    }
}

impl std::error::Error for RewriteError {}

fn err<T>(message: impl Into<String>) -> Result<T, RewriteError> {
    Err(RewriteError(message.into()))
}

fn validate(module: &Module) -> Result<(), Box<naga::WithSpan<naga::valid::ValidationError>>> {
    // Every capability: the point is to tell this one error from the others, not to check the
    // device's limits (wgpu does that with the module we hand it).
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(module)
    .map(|_| ())
    .map_err(Box::new)
}

/// Whether `error` is naga's rejection of a `storage` pointer function parameter.
fn is_storage_pointer_parameter_error(error: &naga::valid::ValidationError) -> bool {
    matches!(
        error,
        naga::valid::ValidationError::Function {
            source: naga::valid::FunctionError::InvalidArgumentPointerSpace {
                space: naga::AddressSpace::Storage { .. },
                ..
            },
            ..
        }
    )
}

/// Decides what to give wgpu for `wgsl`.
///
/// Only a source that mentions `ptr<storage` is parsed here, and only a module that naga rejects
/// with `InvalidArgumentPointerSpace` on a storage pointer is rewritten; everything else is
/// [`PreparedSource::Wgsl`], so wgpu compiles it and reports its errors exactly as before.
///
/// # Errors
///
/// The module needs the rewrite and it cannot be done (see [`RewriteError`]), or the result does
/// not validate.
pub fn prepare_shader_source(wgsl: &str) -> Result<PreparedSource<'_>, RewriteError> {
    if !wgsl.contains("ptr<storage") {
        return Ok(PreparedSource::Wgsl(wgsl));
    }
    let Ok(module) = naga::front::wgsl::parse_str(wgsl) else {
        return Ok(PreparedSource::Wgsl(wgsl));
    };
    match validate(&module) {
        Err(error) if is_storage_pointer_parameter_error(error.as_inner()) => {}
        _ => return Ok(PreparedSource::Wgsl(wgsl)),
    }
    let module = specialize_storage_pointer_parameters(module)?;
    if let Err(error) = validate(&module) {
        return err(format!("the rewritten module does not validate: {error:?}"));
    }
    Ok(PreparedSource::Rewritten(Box::new(module)))
}

/// The source for `wgpu::ShaderModuleDescriptor`.
///
/// # Errors
///
/// As [`prepare_shader_source`].
pub fn shader_source(wgsl: &str) -> Result<wgpu::ShaderSource<'_>, RewriteError> {
    Ok(match prepare_shader_source(wgsl)? {
        PreparedSource::Wgsl(text) => wgpu::ShaderSource::Wgsl(Cow::Borrowed(text)),
        PreparedSource::Rewritten(module) => wgpu::ShaderSource::Naga(Cow::Owned(*module)),
    })
}

/// A pointer argument resolved to its storage global: the global and the member indices.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Root {
    global: Handle<GlobalVariable>,
    path: Vec<u32>,
}

struct Specializer {
    /// The functions of the input module.
    old: Arena<Function>,
    /// The `GlobalVariable` handles whose address space is `storage`.
    storage_globals: Vec<bool>,
    /// For each old function, which parameters are `storage` pointers.
    pointer_parameters: Vec<Vec<bool>>,
    /// The output functions, callees first.
    new: Arena<Function>,
    memo: HashMap<(Handle<Function>, Vec<Root>), Handle<Function>>,
}

/// Rewrites `module` so that no function has a `storage` pointer parameter.
///
/// # Errors
///
/// A call site passes something other than a chain of constant member accesses on a storage
/// global, or the module uses an expression or statement the rewrite does not map.
pub fn specialize_storage_pointer_parameters(mut module: Module) -> Result<Module, RewriteError> {
    let storage_globals = module
        .global_variables
        .iter()
        .map(|(_, global)| matches!(global.space, naga::AddressSpace::Storage { .. }))
        .collect();
    let pointer_parameters = module
        .functions
        .iter()
        .map(|(_, function)| {
            function
                .arguments
                .iter()
                .map(|argument| {
                    matches!(
                        module.types[argument.ty].inner.pointer_space(),
                        Some(naga::AddressSpace::Storage { .. })
                    )
                })
                .collect()
        })
        .collect();
    let mut specializer = Specializer {
        old: module.functions.take(),
        storage_globals,
        pointer_parameters,
        new: Arena::new(),
        memo: HashMap::new(),
    };
    // Every function without a storage pointer parameter, in order (so the unreachable ones are
    // kept too). The ones with such a parameter exist only as specialisations of their callers.
    let handles: Vec<_> = specializer.old.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        if !specializer.pointer_parameters[handle.index()].contains(&true) {
            specializer.lower(handle, Vec::new())?;
        }
    }
    for entry_point in &mut module.entry_points {
        specializer.rewrite_calls(&mut entry_point.function)?;
    }
    module.functions = specializer.new;
    Ok(module)
}

impl Specializer {
    /// The new handle of `handle` with its storage pointer parameters replaced by `roots` (one per
    /// such parameter, in order).
    fn lower(
        &mut self,
        handle: Handle<Function>,
        roots: Vec<Root>,
    ) -> Result<Handle<Function>, RewriteError> {
        let key = (handle, roots);
        if let Some(&done) = self.memo.get(&key) {
            return Ok(done);
        }
        let mut function = self.old[handle].clone();
        if !key.1.is_empty() {
            let flags = self.pointer_parameters[handle.index()].clone();
            substitute_parameters(&mut function, &flags, &key.1)?;
        }
        self.rewrite_calls(&mut function)?;
        let span = self.old.get_span(handle);
        let new_handle = self.new.append(function, span);
        self.memo.insert(key, new_handle);
        Ok(new_handle)
    }

    /// Redirects every call of `function` (which has no storage pointer parameter left) to the new
    /// callee, specialised for the globals the call passes.
    fn rewrite_calls(&mut self, function: &mut Function) -> Result<(), RewriteError> {
        // The statements borrow the body, the arguments are resolved in the expressions.
        let mut expressions = std::mem::take(&mut function.expressions);
        let result = walk_block(&mut function.body, &mut |statement| {
            let Statement::Call {
                function: callee,
                arguments,
                result,
            } = statement
            else {
                return Ok(());
            };
            let flags = self.pointer_parameters[callee.index()].clone();
            let mut roots = Vec::new();
            let mut kept = Vec::with_capacity(arguments.len());
            for (index, &argument) in arguments.iter().enumerate() {
                if flags.get(index).copied().unwrap_or(false) {
                    roots.push(self.resolve(&expressions, argument, *callee)?);
                } else {
                    kept.push(argument);
                }
            }
            let old_callee = *callee;
            *callee = self.lower(old_callee, roots)?;
            *arguments = kept;
            // The result expression names the callee too.
            if let Some(result) = result
                && let Expression::CallResult(target) = &mut expressions[*result]
            {
                *target = *callee;
            }
            Ok(())
        });
        function.expressions = expressions;
        result
    }

    /// The storage global that the pointer expression `argument` addresses.
    fn resolve(
        &self,
        expressions: &Arena<Expression>,
        argument: Handle<Expression>,
        callee: Handle<Function>,
    ) -> Result<Root, RewriteError> {
        let name = self.old[callee].name.as_deref().unwrap_or("<unnamed>");
        let mut path = Vec::new();
        let mut current = argument;
        loop {
            match expressions[current] {
                Expression::AccessIndex { base, index } => {
                    path.push(index);
                    current = base;
                }
                Expression::GlobalVariable(global) if self.storage_globals[global.index()] => {
                    path.reverse();
                    return Ok(Root { global, path });
                }
                ref other => {
                    return err(format!(
                        "a call of `{name}` passes {other:?} as a storage pointer; only a \
                         constant member access chain on a storage global is supported"
                    ));
                }
            }
        }
    }
}

/// Removes the parameters flagged in `flags` from `function` and makes their uses the access
/// chains of `roots`.
fn substitute_parameters(
    function: &mut Function,
    flags: &[bool],
    roots: &[Root],
) -> Result<(), RewriteError> {
    // The new parameter index of each kept parameter.
    let mut new_index = vec![0u32; flags.len()];
    let mut kept = 0;
    for (index, &dropped) in flags.iter().enumerate() {
        if !dropped {
            new_index[index] = kept;
            kept += 1;
        }
    }
    // The parameters to remove, in order, pair up with `roots`.
    let mut root_of = vec![None; flags.len()];
    let mut next_root = roots.iter();
    for (index, &dropped) in flags.iter().enumerate() {
        if dropped {
            root_of[index] = next_root.next();
        }
    }
    let mut index = 0;
    function.arguments.retain(|_| {
        index += 1;
        !flags[index - 1]
    });

    // Rebuild the expression arena: the chain of a dropped parameter takes the place of its
    // `FunctionArgument`, and everything after it shifts.
    let old_expressions: Vec<_> = function.expressions.drain().collect();
    let mut table: Vec<Handle<Expression>> = Vec::with_capacity(old_expressions.len());
    // `Emit` statements for the access chains, which are not "pre-emitted" expressions.
    let mut emits = Vec::new();
    for (_, mut expression, span) in old_expressions {
        if let Expression::FunctionArgument(argument) = expression {
            let argument = argument as usize;
            if let Some(Some(root)) = root_of.get(argument) {
                let mut handle = function
                    .expressions
                    .append(Expression::GlobalVariable(root.global), span);
                let first_access = function.expressions.len();
                for &member in &root.path {
                    handle = function.expressions.append(
                        Expression::AccessIndex {
                            base: handle,
                            index: member,
                        },
                        span,
                    );
                }
                if !root.path.is_empty() {
                    emits.push(Statement::Emit(
                        function.expressions.range_from(first_access),
                    ));
                }
                table.push(handle);
                continue;
            }
            expression = Expression::FunctionArgument(new_index[argument]);
        } else {
            map_expression(&mut expression, &|handle| table[handle.index()])?;
        }
        table.push(function.expressions.append(expression, span));
    }

    let adjust = |handle: Handle<Expression>| table[handle.index()];
    for (_, local) in function.local_variables.iter_mut() {
        local.init = local.init.map(adjust);
    }
    function.named_expressions = function
        .named_expressions
        .drain(..)
        .map(|(handle, name)| (adjust(handle), name))
        .collect();
    walk_block(&mut function.body, &mut |statement| {
        map_statement(statement, &adjust)
    })?;
    if !emits.is_empty() {
        let mut body = Block::with_capacity(function.body.len() + emits.len());
        for emit in emits {
            body.push(emit, naga::Span::UNDEFINED);
        }
        body.append(&mut function.body);
        function.body = body;
    }
    Ok(())
}

/// Calls `visit` on every statement of `block`, including the nested ones.
fn walk_block(
    block: &mut Block,
    visit: &mut dyn FnMut(&mut Statement) -> Result<(), RewriteError>,
) -> Result<(), RewriteError> {
    for statement in block.iter_mut() {
        visit(statement)?;
        match statement {
            Statement::Block(inner) => walk_block(inner, visit)?,
            Statement::If { accept, reject, .. } => {
                walk_block(accept, visit)?;
                walk_block(reject, visit)?;
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    walk_block(&mut case.body, visit)?;
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                walk_block(body, visit)?;
                walk_block(continuing, visit)?;
            }
            _ => {}
        }
    }
    Ok(())
}

type Adjust<'a> = &'a dyn Fn(Handle<Expression>) -> Handle<Expression>;

fn map_all(handles: &mut [Handle<Expression>], adjust: Adjust<'_>) {
    for handle in handles {
        *handle = adjust(*handle);
    }
}

fn map_option(handle: &mut Option<Handle<Expression>>, adjust: Adjust<'_>) {
    *handle = handle.map(adjust);
}

/// Applies `adjust` to the expression handles of `statement` (not its nested blocks, which
/// [`walk_block`] visits). Statements the Skia shaders never produce are an error, not skipped.
fn map_statement(statement: &mut Statement, adjust: Adjust<'_>) -> Result<(), RewriteError> {
    match statement {
        Statement::Emit(range) => {
            let Some((first, last)) = range.first_and_last() else {
                return Ok(());
            };
            let (first, last) = (adjust(first), adjust(last));
            let new_range = Range::new_from_bounds(first, last);
            if new_range.index_range().len() != range.index_range().len() {
                return err("an Emit range was split by the rewrite");
            }
            *range = new_range;
        }
        Statement::Block(_)
        | Statement::Break
        | Statement::Continue
        | Statement::Kill
        | Statement::ControlBarrier(_)
        | Statement::MemoryBarrier(_)
        | Statement::Return { value: None } => {}
        Statement::If { condition, .. } => *condition = adjust(*condition),
        Statement::Switch { selector, .. } => *selector = adjust(*selector),
        Statement::Loop { break_if, .. } => map_option(break_if, adjust),
        Statement::Return { value: Some(value) } => *value = adjust(*value),
        Statement::Store { pointer, value } => {
            *pointer = adjust(*pointer);
            *value = adjust(*value);
        }
        Statement::ImageStore {
            image,
            coordinate,
            array_index,
            value,
        } => {
            *image = adjust(*image);
            *coordinate = adjust(*coordinate);
            map_option(array_index, adjust);
            *value = adjust(*value);
        }
        Statement::Call {
            arguments, result, ..
        } => {
            map_all(arguments, adjust);
            map_option(result, adjust);
        }
        other => return err(format!("unsupported statement {other:?}")),
    }
    Ok(())
}

/// Applies `adjust` to the expression handles of `expression`; one the Skia shaders never
/// produce is an error.
fn map_expression(expression: &mut Expression, adjust: Adjust<'_>) -> Result<(), RewriteError> {
    match expression {
        Expression::Literal(_)
        | Expression::Constant(_)
        | Expression::ZeroValue(_)
        | Expression::GlobalVariable(_)
        | Expression::LocalVariable(_)
        | Expression::FunctionArgument(_)
        | Expression::CallResult(_) => {}
        Expression::Compose { components, .. } => map_all(components, adjust),
        Expression::Access { base, index } => {
            *base = adjust(*base);
            *index = adjust(*index);
        }
        Expression::AccessIndex { base, .. } => *base = adjust(*base),
        Expression::Splat { value, .. } => *value = adjust(*value),
        Expression::Swizzle { vector, .. } => *vector = adjust(*vector),
        Expression::Load { pointer } => *pointer = adjust(*pointer),
        Expression::ImageSample {
            image,
            sampler,
            coordinate,
            array_index,
            offset,
            level,
            depth_ref,
            ..
        } => {
            *image = adjust(*image);
            *sampler = adjust(*sampler);
            *coordinate = adjust(*coordinate);
            map_option(array_index, adjust);
            map_option(offset, adjust);
            match level {
                SampleLevel::Auto | SampleLevel::Zero => {}
                SampleLevel::Exact(level) | SampleLevel::Bias(level) => *level = adjust(*level),
                SampleLevel::Gradient { x, y } => {
                    *x = adjust(*x);
                    *y = adjust(*y);
                }
            }
            map_option(depth_ref, adjust);
        }
        Expression::ImageLoad {
            image,
            coordinate,
            array_index,
            sample,
            level,
        } => {
            *image = adjust(*image);
            *coordinate = adjust(*coordinate);
            map_option(array_index, adjust);
            map_option(sample, adjust);
            map_option(level, adjust);
        }
        Expression::ImageQuery { image, query } => {
            *image = adjust(*image);
            if let naga::ImageQuery::Size { level } = query {
                map_option(level, adjust);
            }
        }
        Expression::Unary { expr, .. }
        | Expression::Derivative { expr, .. }
        | Expression::As { expr, .. }
        | Expression::ArrayLength(expr) => *expr = adjust(*expr),
        Expression::Binary { left, right, .. } => {
            *left = adjust(*left);
            *right = adjust(*right);
        }
        Expression::Select {
            condition,
            accept,
            reject,
        } => {
            *condition = adjust(*condition);
            *accept = adjust(*accept);
            *reject = adjust(*reject);
        }
        Expression::Relational { argument, .. } => *argument = adjust(*argument),
        Expression::Math {
            arg,
            arg1,
            arg2,
            arg3,
            ..
        } => {
            *arg = adjust(*arg);
            map_option(arg1, adjust);
            map_option(arg2, adjust);
            map_option(arg3, adjust);
        }
        other => return err(format!("unsupported expression {other:?}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "
struct Stops { data : array<f32>, }
@group(0) @binding(0) var<storage, read> buffer : Stops;
@group(0) @binding(1) var<storage, read> other : Stops;
";

    fn rewritten(wgsl: &str) -> Module {
        match prepare_shader_source(wgsl).expect("rewrite") {
            PreparedSource::Rewritten(module) => *module,
            PreparedSource::Wgsl(_) => panic!("expected a rewrite"),
        }
    }

    #[test]
    fn naga_rejects_the_original() {
        let wgsl = format!(
            "{HEADER}
fn read_stop(p : ptr<storage, array<f32>, read>, i : i32) -> f32 {{ return (*p)[i]; }}
@fragment fn main() -> @location(0) vec4<f32> {{
    return vec4<f32>(read_stop(&(buffer.data), 1));
}}"
        );
        let module = naga::front::wgsl::parse_str(&wgsl).expect("parses");
        let error = validate(&module).expect_err("naga rejects the parameter");
        assert!(is_storage_pointer_parameter_error(error.as_inner()));
    }

    #[test]
    fn one_level() {
        let wgsl = format!(
            "{HEADER}
fn read_stop(p : ptr<storage, array<f32>, read>, i : i32, scale : f32) -> f32 {{
    return (*p)[i] * scale + f32(arrayLength(p));
}}
@fragment fn main() -> @location(0) vec4<f32> {{
    return vec4<f32>(read_stop(&(buffer.data), 1, 2.0));
}}"
        );
        let module = rewritten(&wgsl);
        assert_eq!(module.functions.len(), 1);
        let (_, function) = module.functions.iter().next().unwrap();
        // `i` and `scale` stay.
        assert_eq!(function.arguments.len(), 2);
    }

    #[test]
    fn nested_and_two_buffers() {
        let wgsl = format!(
            "{HEADER}
fn read_stop(p : ptr<storage, array<f32>, read>, i : i32) -> f32 {{ return (*p)[i]; }}
fn mid(p : ptr<storage, array<f32>, read>, i : i32) -> f32 {{
    var x = 1.0;
    if (i > 0) {{ x = read_stop(p, i - 1); }}
    return x + read_stop(p, i);
}}
fn top(p : ptr<storage, array<f32>, read>, q : ptr<storage, array<f32>, read>, i : i32) -> f32 {{
    return mid(p, i) + mid(q, i);
}}
@fragment fn main() -> @location(0) vec4<f32> {{
    return vec4<f32>(top(&(buffer.data), &(other.data), 3), mid(&(buffer.data), 0), 0.0, 1.0);
}}"
        );
        let module = rewritten(&wgsl);
        // read_stop and mid for each buffer, and top.
        assert_eq!(module.functions.len(), 5);
        for (_, function) in module.functions.iter() {
            for argument in &function.arguments {
                assert!(module.types[argument.ty].inner.pointer_space().is_none());
            }
        }
    }

    #[test]
    fn whole_global_without_a_member() {
        let wgsl = "
@group(0) @binding(0) var<storage, read> raw : array<f32>;
fn fetch(p : ptr<storage, array<f32>, read>) -> f32 { return (*p)[0]; }
@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(fetch(&raw)); }";
        let module = rewritten(wgsl);
        assert_eq!(module.functions.len(), 1);
    }

    #[test]
    fn other_modules_are_untouched() {
        let plain = "@fragment fn main() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }";
        assert!(matches!(
            prepare_shader_source(plain),
            Ok(PreparedSource::Wgsl(text)) if text == plain
        ));
        // Mentions a storage pointer but naga accepts it: no rewrite.
        let accepted = "
@group(0) @binding(0) var<storage, read> raw : array<f32>;
@fragment fn main() -> @location(0) vec4<f32> {
    let p = &raw;
    return vec4<f32>((*p)[0]);
}
// ptr<storage";
        assert!(matches!(
            prepare_shader_source(accepted),
            Ok(PreparedSource::Wgsl(_))
        ));
        // A syntax error is wgpu's to report.
        assert!(matches!(
            prepare_shader_source("ptr<storage fn"),
            Ok(PreparedSource::Wgsl(_))
        ));
    }

    #[test]
    fn a_call_through_anything_else_is_an_error() {
        let wgsl = format!(
            "{HEADER}
fn read_stop(p : ptr<storage, array<f32>, read>, i : i32) -> f32 {{ return (*p)[i]; }}
@fragment fn main() -> @location(0) vec4<f32> {{
    var which = 0;
    return vec4<f32>(read_stop(&(buffer.data), which));
}}"
        );
        assert!(prepare_shader_source(&wgsl).is_ok());

        // A pointer picked at run time cannot be written in WGSL, so build the failure on the IR:
        // the pointer argument becomes the integer variable.
        let mut module = naga::front::wgsl::parse_str(&wgsl).unwrap();
        let function = &mut module.entry_points[0].function;
        let local = function
            .expressions
            .fetch_if(|e| matches!(e, Expression::LocalVariable(_)))
            .expect("the local");
        let arguments = function
            .body
            .iter_mut()
            .find_map(|statement| match statement {
                Statement::Call { arguments, .. } => Some(arguments),
                _ => None,
            })
            .expect("the call");
        arguments[0] = local;
        let error = specialize_storage_pointer_parameters(module).unwrap_err();
        assert!(
            error.0.contains("only a constant member access chain"),
            "{error}"
        );
    }
}
