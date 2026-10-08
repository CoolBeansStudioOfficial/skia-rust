// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/ir/SkSLSwizzle.{h,cpp} (data, `MaskString`, `IsIdentity`,
// `description`, `Convert`, `Make` and `MakeExact`).

//! [`Swizzle`]: `base.xyzw`.

use super::{
    ConstructorCompound, ConstructorCompoundCast, ConstructorScalarCast, ConstructorSplat,
    Expression, ExpressionKind, IrPool, Literal, ids::ExprId,
};
use crate::constant_folder;
use crate::context::Context;
use crate::operator::OperatorPrecedence;
use crate::position::Position;

impl Swizzle {
    /// `Convert(context, pos, maskPos, base, componentString)`: parses a swizzle mask such as
    /// `xy` or `x0`, checks it against `base`, and builds the expression. Reports errors and
    /// returns `None` on failure.
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L247-L440 (chrome/m156)
    // One function, as in Skia: its checks run in order and share the mask.
    #[allow(clippy::too_many_lines)]
    pub fn convert(
        ctx: &mut Context,
        pos: Position,
        mask_pos: Position,
        base: ExprId,
        component_string: &str,
    ) -> Option<ExprId> {
        if component_string.len() > 4 {
            ctx.errors.error(
                Position::range(mask_pos.start_offset() + 4, mask_pos.end_offset()),
                "too many components in swizzle mask",
            );
            return None;
        }

        // Convert the component string into an equivalent array.
        let mut components = ComponentArray::default();
        for (i, &field) in component_string.as_bytes().iter().enumerate() {
            let component = match field {
                b'0' => swizzle_component::ZERO,
                b'1' => swizzle_component::ONE,
                b'x' => swizzle_component::X,
                b'r' => swizzle_component::R,
                b's' => swizzle_component::S,
                b'L' => swizzle_component::UL,
                b'y' => swizzle_component::Y,
                b'g' => swizzle_component::G,
                b't' => swizzle_component::T,
                b'T' => swizzle_component::UT,
                b'z' => swizzle_component::Z,
                b'b' => swizzle_component::B,
                b'p' => swizzle_component::P,
                b'R' => swizzle_component::UR,
                b'w' => swizzle_component::W,
                b'a' => swizzle_component::A,
                b'q' => swizzle_component::Q,
                b'B' => swizzle_component::UB,
                _ => {
                    let at = mask_pos.start_offset() + to_i32(i);
                    ctx.errors.error(
                        Position::range(at, at + 1),
                        &format!("invalid swizzle component '{}'", char::from(field)),
                    );
                    return None;
                }
            };
            components.push(component);
        }

        if !validate_swizzle_domain(&components) {
            ctx.errors.error(
                mask_pos,
                &format!("invalid swizzle mask '{}'", Self::mask_string(&components)),
            );
            return None;
        }

        let base_scalar = {
            let base_ty = ctx.pool.expression(base).ty;
            ctx.pool.ty(base_ty).scalar_type_for_literal().id()
        };
        let (is_vector, is_scalar, columns, display) = {
            let t = ctx.pool.ty(base_scalar);
            (
                t.is_vector(),
                t.is_scalar(),
                t.columns(),
                t.display_name().to_owned(),
            )
        };
        if !is_vector && !is_scalar {
            ctx.errors
                .error(pos, &format!("cannot swizzle value of type '{display}'"));
            return None;
        }

        let mut mask = ComponentArray::default();
        let mut found_xyzw = false;
        for (i, &component) in components.as_slice().iter().enumerate() {
            // The component of the mask, and the fewest columns the base needs to have it. The
            // `Y`, `Z` and `W` groups fall through in Skia when the base is too small, and each
            // then reports the same error, so one comparison with the needed columns is exact.
            let (mask_component, needed_columns) = match component {
                // Skip over constant fields for now.
                swizzle_component::ZERO | swizzle_component::ONE => continue,
                swizzle_component::X
                | swizzle_component::R
                | swizzle_component::S
                | swizzle_component::UL => {
                    found_xyzw = true;
                    mask.push(swizzle_component::X);
                    continue;
                }
                swizzle_component::Y
                | swizzle_component::G
                | swizzle_component::T
                | swizzle_component::UT => (swizzle_component::Y, 2),
                swizzle_component::Z
                | swizzle_component::B
                | swizzle_component::P
                | swizzle_component::UR => (swizzle_component::Z, 3),
                swizzle_component::W
                | swizzle_component::A
                | swizzle_component::Q
                | swizzle_component::UB => (swizzle_component::W, 4),
                _ => unreachable!("invalid swizzle component {component}"),
            };
            found_xyzw = true;
            if columns >= needed_columns {
                mask.push(mask_component);
            } else {
                // The swizzle component references a field that doesn't exist in the base type.
                let at = mask_pos.start_offset() + to_i32(i);
                ctx.errors.error(
                    Position::range(at, at + 1),
                    &format!("invalid swizzle component '{}'", mask_char(component)),
                );
                return None;
            }
        }
        if !found_xyzw {
            ctx.errors
                .error(mask_pos, "swizzle must refer to base expression");
            return None;
        }

        // Coerce literals in expressions such as `(12345).xxx` to their actual type.
        let base = base_scalar.coerce_expression(ctx, base)?;

        // Swizzles are complicated due to constant components. The most difficult case is a mask
        // like '.x1w0'. A naive approach might turn that into 'float4(base.x, 1, base.w, 0)', but
        // that evaluates 'base' twice. We instead group the swizzle mask ('xw') and constants ('1,
        // 0') together and use a secondary swizzle to put them back into the right order, so in
        // this case we end up with 'float4(base.xw, 1, 0).xzyw'.
        //
        // First, we need a vector expression that is the non-constant portion of the swizzle,
        // packed:
        //   scalar.xxx  -> type3(scalar)
        //   scalar.x0x0 -> type2(scalar)
        //   vector.zyx  -> vector.zyx
        //   vector.x0y0 -> vector.xy
        let expr = Self::make(ctx, pos, base, mask);

        // If we have processed the entire swizzle, we're done.
        if mask.len() == components.len() {
            return Some(expr);
        }

        // Now we create a constructor that has the correct number of elements for the final
        // swizzle, with all fields at the start. It's not finished yet; constants we need will be
        // added below.
        //   scalar.x0x0 -> type4(type2(x), ...)
        //   vector.y111 -> type4(vector.y, ...)
        //   vector.z10x -> type4(vector.zx, ...)
        //
        // The constructor will have at most three arguments: { base expr, constant 0, constant 1 }
        let mut constructor_args = vec![expr];

        // Apply another swizzle to shuffle the constants into the correct place. Any constant
        // values we need are also tacked on to the end of the constructor.
        //   scalar.x0x0 -> type4(type2(x), 0).xyxy
        //   vector.y111 -> type2(vector.y, 1).xyyy
        //   vector.z10x -> type4(vector.zx, 1, 0).xzwy
        let scalar_type = ctx.pool.ty(base_scalar).component_type().id();
        let mut swizzle_components = ComponentArray::default();
        let mut mask_field_idx = 0_i8;
        let mut constant_field_idx = to_i32(mask.len());
        let mut constant_zero_idx = None;
        let mut constant_one_idx = None;
        for &component in components.as_slice() {
            match component {
                swizzle_component::ZERO => {
                    let index = *constant_zero_idx.get_or_insert_with(|| {
                        // Synthesize a '0' argument at the end of the constructor.
                        constructor_args.push(Literal::make(&mut ctx.pool, pos, 0.0, scalar_type));
                        constant_field_idx += 1;
                        constant_field_idx - 1
                    });
                    swizzle_components.push(to_i8(index));
                }
                swizzle_component::ONE => {
                    let index = *constant_one_idx.get_or_insert_with(|| {
                        // Synthesize a '1' argument at the end of the constructor.
                        constructor_args.push(Literal::make(&mut ctx.pool, pos, 1.0, scalar_type));
                        constant_field_idx += 1;
                        constant_field_idx - 1
                    });
                    swizzle_components.push(to_i8(index));
                }
                _ => {
                    // The non-constant fields are already in the expected order.
                    swizzle_components.push(mask_field_idx);
                    mask_field_idx += 1;
                }
            }
        }

        let compound_type = ctx.pool.ty(scalar_type).to_compound(constant_field_idx, 1);
        let expr = ConstructorCompound::make(ctx, pos, compound_type, constructor_args);

        // Create (and potentially optimize-away) the resulting swizzle-expression.
        Some(Self::make(ctx, pos, expr, swizzle_components))
    }

    /// `Make(context, pos, expr, components)`: a swizzle of `expr`, simplified when possible. The
    /// components are `X`, `Y`, `Z` or `W`, and `expr` is a vector or a scalar.
    ///
    /// Not ported: `optimize_constructor_swizzle`, which rewrites a swizzle of a compound
    /// constructor. It needs `Analysis::IsTrivialExpression` and `HasSideEffects` (S9a).
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L451-L532 (chrome/m156), partly
    pub fn make(
        ctx: &mut Context,
        pos: Position,
        expr: ExprId,
        components: ComponentArray,
    ) -> ExprId {
        let expr_ty = ctx.pool.expression(expr).ty;
        debug_assert!(
            ctx.pool.ty(expr_ty).is_vector() || ctx.pool.ty(expr_ty).is_scalar(),
            "cannot swizzle type '{}'",
            ctx.pool.ty(expr_ty).description()
        );
        debug_assert!((1..=4).contains(&components.len()));
        debug_assert!(
            components
                .as_slice()
                .iter()
                .all(|&c| (swizzle_component::X..=swizzle_component::W).contains(&c))
        );
        let n = to_i32(components.len());

        // SkSL supports splatting a scalar via `scalar.xxxx`, but not all versions of GLSL allow
        // this. Replace swizzles with equivalent splat constructors (`scalar.xxx` --> `half3(value)`).
        if ctx.pool.ty(expr_ty).is_scalar() {
            let ty = ctx.pool.ty(expr_ty).to_compound(n, 1);
            return ConstructorSplat::make(ctx, pos, ty, expr);
        }

        // Detect identity swizzles like `color.rgba` and optimize them away.
        if ctx.pool.ty(expr_ty).columns() == n && Self::is_identity(&components) {
            ctx.pool.expression_mut(expr).position = pos;
            return expr;
        }

        // Optimize swizzles of swizzles, e.g. replace `foo.argb.rggg` with `foo.arrr`.
        if let ExpressionKind::Swizzle(base) = ctx.pool.expression(expr).kind.clone() {
            let mut combined = ComponentArray::default();
            for &c in components.as_slice() {
                combined.push(base.components.as_slice()[index_of(c)]);
            }
            // It may actually be possible to further simplify this swizzle. Go again.
            // (e.g. `color.abgr.abgr` --> `color.rgba` --> `color`.)
            return Self::make(ctx, pos, base.base, combined);
        }

        // If we are swizzling a constant expression, we can use its value instead here (so that
        // swizzles like `colorWhite.x` can be simplified to `1`).
        let value = constant_folder::get_constant_value_for_variable(&ctx.pool, expr);
        let value_ty = ctx.pool.expression(value).ty;
        match ctx.pool.expression(value).kind.clone() {
            // `half4(scalar).zyy` can be optimized to `half3(scalar)`, and `half3(scalar).y` can be
            // optimized to just `scalar`. The swizzle components don't actually matter, as every
            // field in a splat constructor holds the same value.
            ExpressionKind::ConstructorSplat(splat) => {
                let ty = ctx.pool.ty(value_ty).component_type().to_compound(n, 1);
                let argument = ctx.pool.clone_expression(splat.argument);
                return ConstructorSplat::make(ctx, pos, ty, argument);
            }
            // Swizzles on casts, like `half4(myFloat4).zyy`, can optimize to `half3(myFloat4.zyy)`.
            ExpressionKind::ConstructorCompoundCast(cast) => {
                let cast_type = ctx.pool.ty(value_ty).component_type().to_compound(n, 1);
                let argument = ctx.pool.clone_expression(cast.argument);
                let swizzled = Self::make(ctx, pos, argument, components);
                return if ctx.pool.ty(cast_type).columns() > 1 {
                    ConstructorCompoundCast::make(ctx, pos, cast_type, swizzled)
                } else {
                    ConstructorScalarCast::make(ctx, pos, cast_type, swizzled)
                };
            }
            _ => {}
        }

        // The swizzle could not be simplified, so apply the requested swizzle to the base
        // expression.
        let ty = ctx.pool.ty(expr_ty).component_type().to_compound(n, 1);
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Swizzle(Self {
                base: expr,
                components,
            }),
        ))
    }

    /// `MakeExact(context, pos, expr, components)`: a swizzle node with no simplification.
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L534-L549 (chrome/m156)
    pub fn make_exact(
        ctx: &mut Context,
        pos: Position,
        expr: ExprId,
        components: ComponentArray,
    ) -> ExprId {
        let expr_ty = ctx.pool.expression(expr).ty;
        debug_assert!(
            ctx.pool.ty(expr_ty).is_vector() || ctx.pool.ty(expr_ty).is_scalar(),
            "cannot swizzle type '{}'",
            ctx.pool.ty(expr_ty).description()
        );
        let ty = ctx
            .pool
            .ty(expr_ty)
            .component_type()
            .to_compound(to_i32(components.len()), 1);
        ctx.pool.add_expression(Expression::new(
            pos,
            ty,
            ExpressionKind::Swizzle(Self {
                base: expr,
                components,
            }),
        ))
    }
}

/// `validate_swizzle_domain`: whether the components of a mask all come from one of the four
/// domains (coordinates, colors, texture coordinates, rectangle coordinates), where `0` and `1`
/// belong to every domain.
// Port of: src/sksl/ir/SkSLSwizzle.cpp#L35-L87 (chrome/m156)
fn validate_swizzle_domain(fields: &[i8]) -> bool {
    #[derive(PartialEq, Clone, Copy)]
    enum SwizzleDomain {
        Coordinate,
        Color,
        Uv,
        Rectangle,
    }
    let mut domain: Option<SwizzleDomain> = None;
    for &field in fields {
        let field_domain = match field {
            swizzle_component::X
            | swizzle_component::Y
            | swizzle_component::Z
            | swizzle_component::W => SwizzleDomain::Coordinate,
            swizzle_component::R
            | swizzle_component::G
            | swizzle_component::B
            | swizzle_component::A => SwizzleDomain::Color,
            swizzle_component::S
            | swizzle_component::T
            | swizzle_component::P
            | swizzle_component::Q => SwizzleDomain::Uv,
            swizzle_component::UL
            | swizzle_component::UT
            | swizzle_component::UR
            | swizzle_component::UB => SwizzleDomain::Rectangle,
            swizzle_component::ZERO | swizzle_component::ONE => continue,
            _ => return false,
        };
        match domain {
            None => domain = Some(field_domain),
            Some(d) if d != field_domain => return false,
            Some(_) => {}
        }
    }
    true
}

/// The index of a component in a swizzle's component list. Components are `0..4` here.
fn index_of(component: i8) -> usize {
    usize::try_from(component).expect("a swizzle component is X, Y, Z or W")
}

/// An array length or count as an `i32`.
fn to_i32(n: usize) -> i32 {
    i32::try_from(n).expect("a count fits in i32")
}

/// A small non-negative index as a swizzle component.
fn to_i8(n: i32) -> i8 {
    i8::try_from(n).expect("a swizzle index fits in i8")
}

/// `SkSL::SwizzleComponent`: the component codes of a swizzle mask. `ZERO` and `ONE` are the
/// constant components (`.x0`, `.1y`).
#[doc(alias = "SkSL::SwizzleComponent")]
#[allow(missing_docs)]
pub mod swizzle_component {
    pub const X: i8 = 0;
    pub const Y: i8 = 1;
    pub const Z: i8 = 2;
    pub const W: i8 = 3;
    pub const R: i8 = 4;
    pub const G: i8 = 5;
    pub const B: i8 = 6;
    pub const A: i8 = 7;
    pub const S: i8 = 8;
    pub const T: i8 = 9;
    pub const P: i8 = 10;
    pub const Q: i8 = 11;
    pub const UL: i8 = 12;
    pub const UT: i8 = 13;
    pub const UR: i8 = 14;
    pub const UB: i8 = 15;
    pub const ZERO: i8 = 16;
    pub const ONE: i8 = 17;
}

/// `SkSL::ComponentArray`: one to four swizzle components (`FixedArray<4, int8_t>`).
#[doc(alias = "SkSL::ComponentArray")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ComponentArray {
    len: u8,
    components: [i8; 4],
}

impl ComponentArray {
    /// An array holding `components`.
    ///
    /// # Panics
    ///
    /// If there are more than four components.
    #[must_use]
    pub fn from_slice(components: &[i8]) -> Self {
        assert!(components.len() <= 4, "a swizzle has at most 4 components");
        let mut result = Self::default();
        for &c in components {
            result.push(c);
        }
        result
    }

    /// Appends a component.
    ///
    /// # Panics
    ///
    /// If the array already holds four components.
    pub fn push(&mut self, component: i8) {
        assert!(self.len < 4, "a swizzle has at most 4 components");
        self.components[usize::from(self.len)] = component;
        self.len += 1;
    }

    /// The components.
    #[must_use]
    pub fn as_slice(&self) -> &[i8] {
        &self.components[..usize::from(self.len)]
    }

    /// `size()`.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl std::ops::Deref for ComponentArray {
    type Target = [i8];

    fn deref(&self) -> &[i8] {
        self.as_slice()
    }
}

/// `SkSL::Swizzle`. Its type is the base's component type widened to the mask's length.
// Port of: src/sksl/ir/SkSLSwizzle.h#L45-L128 (chrome/m156)
#[doc(alias = "SkSL::Swizzle")]
#[derive(Clone, Debug, PartialEq)]
pub struct Swizzle {
    /// `base()`.
    pub base: ExprId,
    /// `components()`.
    pub components: ComponentArray,
}

/// `mask_char`: the letter of a component.
// Port of: src/sksl/ir/SkSLSwizzle.cpp#L89-L111 (chrome/m156)
fn mask_char(component: i8) -> char {
    use swizzle_component as c;
    match component {
        c::X => 'x',
        c::Y => 'y',
        c::Z => 'z',
        c::W => 'w',
        c::R => 'r',
        c::G => 'g',
        c::B => 'b',
        c::A => 'a',
        c::S => 's',
        c::T => 't',
        c::P => 'p',
        c::Q => 'q',
        c::UL => 'L',
        c::UT => 'T',
        c::UR => 'R',
        c::UB => 'B',
        c::ZERO => '0',
        c::ONE => '1',
        _ => unreachable!("invalid swizzle component {component}"),
    }
}

impl Swizzle {
    /// `MaskString(components)`: `"xyz"`, `"x0"`, …
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L113-L119 (chrome/m156)
    #[must_use]
    pub fn mask_string(components: &[i8]) -> String {
        components.iter().map(|&c| mask_char(c)).collect()
    }

    /// `IsIdentity(components)`: `.x`, `.xy`, `.xyz` or `.xyzw`.
    #[must_use]
    pub fn is_identity(components: &[i8]) -> bool {
        components
            .iter()
            .enumerate()
            .all(|(index, &c)| i8::try_from(index).is_ok_and(|i| i == c))
    }

    /// `description()`: `base.mask`.
    // Port of: src/sksl/ir/SkSLSwizzle.cpp#L551-L554 (chrome/m156)
    #[must_use]
    pub fn description(&self, pool: &IrPool) -> String {
        format!(
            "{}.{}",
            pool.expression_description_with(self.base, OperatorPrecedence::Postfix),
            Self::mask_string(&self.components)
        )
    }
}
