// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (`LValue` and its subclasses).

//! Assignable expressions: `load()` is an expression with no side effects that reads the value,
//! `store(value)` is a statement that writes it.

use super::types::to_wgsl_type_simple;
use crate::context::Context;
use crate::ir::{Swizzle, TypeId};

/// `WGSLCodeGenerator::LValue`: `PointerLValue`, `VectorComponentLValue` and `SwizzleLValue`.
#[derive(Debug)]
pub(super) enum LValue {
    /// `PointerLValue`: `name` must be a WGSL expression with no side-effects, which we can
    /// safely take the address of. (e.g. `array[index].field` would be valid, but
    /// `array[Func()]` or `vector.x` are not.)
    Pointer(String),
    /// `VectorComponentLValue`: `name` must be a WGSL expression with no side-effects that points
    /// to a single component of a WGSL vector.
    VectorComponent(String),
    /// `SwizzleLValue`.
    Swizzle(SwizzleLValue),
}

impl LValue {
    /// `load()`: a WGSL expression that loads from the lvalue with no side effects (e.g.
    /// `array[index].field`).
    pub(super) fn load(&self) -> String {
        match self {
            Self::Pointer(name) | Self::VectorComponent(name) => name.clone(),
            Self::Swizzle(s) => s.load(),
        }
    }

    /// `store(value)`: a WGSL statement that stores into the lvalue with no side effects (e.g.
    /// `array[index].field = the_passed_in_value_string;`).
    pub(super) fn store(&self, ctx: &Context, value: &str) -> String {
        match self {
            Self::Pointer(name) | Self::VectorComponent(name) => {
                format!("{name} = {value};")
            }
            Self::Swizzle(s) => s.store(ctx, value),
        }
    }
}

/// `WGSLCodeGenerator::SwizzleLValue`.
#[derive(Debug)]
pub(super) struct SwizzleLValue {
    name: String,
    ty: TypeId,
    components: Vec<i8>,
    untouched_components: Vec<i8>,
    reintegration_swizzle: Vec<i8>,
    reintegrate_new_value_first: bool,
}

impl SwizzleLValue {
    /// `name` must be a WGSL expression with no side-effects that points to a WGSL vector.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1139-L1202 (chrome/m156)
    // The casts convert between the component indices (`int8_t`) and the loop counters, as the
    // C++ does.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(super) fn new(ctx: &Context, name: String, ty: TypeId, components: &[i8]) -> Self {
        // If the component array doesn't cover the entire value, we need to create masks for
        // writing back into the lvalue. For example, if the type is vec4 and the component array
        // holds `zx`, a GLSL assignment would look like:
        //     name.zx = new_value;
        //
        // The equivalent WGSL assignment statement would look like:
        //     name = vec4<f32>(new_value, name.xw).yzxw;
        //
        // This replaces name.zy with new_value.xy, and leaves name.xw at their original values.
        // By convention, we always put the new value first and the original values second; it
        // might be possible to find better arrangements which simplify the assignment overall,
        // but we don't attempt this.
        let full_slot_count = ctx.pool.ty(ty).slot_count();
        debug_assert!(full_slot_count <= 4);

        // First, see which components are used.
        // The assignment swizzle must not reuse components.
        let mut used = [false; 4];
        for &component in components {
            debug_assert!(!used[component as usize]);
            used[component as usize] = true;
        }

        // Any untouched components will need to be fetched from the original value.
        let mut untouched_components = Vec::new();
        for (index, &component_used) in used.iter().enumerate().take(full_slot_count) {
            if !component_used {
                untouched_components.push(index as i8);
            }
        }

        // The reintegration swizzle needs to move the components back into their proper slots.
        let mut reintegration_swizzle = vec![0_i8; full_slot_count];
        let mut reintegrate_index: i8 = 0;

        // This refills the untouched slots with the original values.
        let refill_untouched_slots = |swizzle: &mut Vec<i8>, reintegrate_index: &mut i8| {
            for index in 0..full_slot_count {
                if !used[index] {
                    swizzle[index] = *reintegrate_index;
                    *reintegrate_index += 1;
                }
            }
        };

        // This places the new-value components into the proper slots.
        let insert_new_values_into_slots = |swizzle: &mut Vec<i8>, reintegrate_index: &mut i8| {
            for &component in components {
                swizzle[component as usize] = *reintegrate_index;
                *reintegrate_index += 1;
            }
        };

        // When reintegrating the untouched and new values, if the `x` slot is overwritten, we
        // reintegrate the new value first. Otherwise, we reintegrate the original value first.
        // This increases our odds of getting an identity swizzle for the reintegration.
        let reintegrate_new_value_first;
        if used[0] {
            reintegrate_new_value_first = true;
            insert_new_values_into_slots(&mut reintegration_swizzle, &mut reintegrate_index);
            refill_untouched_slots(&mut reintegration_swizzle, &mut reintegrate_index);
        } else {
            reintegrate_new_value_first = false;
            refill_untouched_slots(&mut reintegration_swizzle, &mut reintegrate_index);
            insert_new_values_into_slots(&mut reintegration_swizzle, &mut reintegrate_index);
        }

        Self {
            name,
            ty,
            components: components.to_vec(),
            untouched_components,
            reintegration_swizzle,
            reintegrate_new_value_first,
        }
    }

    fn load(&self) -> String {
        format!("{}.{}", self.name, Swizzle::mask_string(&self.components))
    }

    fn store(&self, ctx: &Context, value: &str) -> String {
        // `variable = `
        let mut result = self.name.clone();
        result.push_str(" = ");

        if self.untouched_components.is_empty() {
            // `(new_value);`
            result.push('(');
            result.push_str(value);
            result.push(')');
        } else if self.reintegrate_new_value_first {
            // `vec4<f32>((new_value), `
            result.push_str(&to_wgsl_type_simple(ctx, self.ty));
            result.push_str("((");
            result.push_str(value);
            result.push_str("), ");

            // `variable.yz)`
            result.push_str(&self.name);
            result.push('.');
            result.push_str(&Swizzle::mask_string(&self.untouched_components));
            result.push(')');
        } else {
            // `vec4<f32>(variable.yz`
            result.push_str(&to_wgsl_type_simple(ctx, self.ty));
            result.push('(');
            result.push_str(&self.name);
            result.push('.');
            result.push_str(&Swizzle::mask_string(&self.untouched_components));

            // `, (new_value))`
            result.push_str(", (");
            result.push_str(value);
            result.push_str("))");
        }

        if !Swizzle::is_identity(&self.reintegration_swizzle) {
            // `.wzyx`
            result.push('.');
            result.push_str(&Swizzle::mask_string(&self.reintegration_swizzle));
        }

        result.push(';');
        result
    }
}
