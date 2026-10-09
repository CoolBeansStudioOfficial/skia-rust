// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp (uniform and storage buffers,
// interface blocks and the std140 polyfills).

//! Uniform and storage buffers, and the polyfills for the places where WGSL and std140 disagree.

use std::collections::HashSet;

use super::WgslCodeGenerator;
use super::types::{to_wgsl_type, type_is_low_precision};
use crate::ir::{
    ElemId, Field, InterfaceBlock, Layout, LayoutFlags, ModifierFlags, Modifiers,
    ProgramElementKind, TypeId, VarId,
};
use crate::memory_layout::{MemoryLayout, Standard};
use crate::position::Position;
use crate::program_settings::ProgramKind;

/// `FieldPolyfillInfo`: where a polyfilled field comes from, what it is replaced with and whether
/// the shader used it.
#[derive(Clone, Debug)]
pub(super) struct FieldPolyfillInfo {
    /// `fInterfaceBlock`.
    pub(super) interface_block: ElemId,
    /// `fReplacementName`.
    pub(super) replacement_name: String,
    /// `fIsArray`.
    pub(super) is_array: bool,
    /// `fIsMatrix`.
    pub(super) is_matrix: bool,
    /// `fWasAccessed`.
    pub(super) was_accessed: bool,
}

impl WgslCodeGenerator<'_> {
    /// The variable of the interface block element `ib`.
    fn interface_block_var(&self, ib: ElemId) -> VarId {
        match &self.ctx.pool.element(ib).kind {
            ProgramElementKind::InterfaceBlock(block) => block.var,
            _ => unreachable!("an interface block element"),
        }
    }

    /// `ib.var()->type().componentType()`: the struct type of the interface block.
    pub(super) fn interface_block_struct_type(&self, ib: ElemId) -> TypeId {
        let var = self.interface_block_var(ib);
        let ty = self.ctx.pool.variable(var).ty;
        self.ctx.pool.ty(ty).component_type().id()
    }

    /// `writeUniformPolyfills()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L1443-L1646 (chrome/m156)
    #[allow(clippy::too_many_lines)] // One function in Skia.
    pub(super) fn write_uniform_polyfills(&mut self) {
        // If we didn't encounter any uniforms that need polyfilling, there is nothing to do.
        if self.field_polyfill_map.is_empty() {
            return;
        }

        // We store the list of polyfilled fields as pointers in a hash-map, so the order can be
        // inconsistent across runs. For determinism, we sort the polyfilled objects by name here.
        let mut ordered_fields: Vec<((TypeId, usize), FieldPolyfillInfo)> = self
            .field_polyfill_map
            .iter()
            .map(|(key, info)| (*key, info.clone()))
            .collect();
        ordered_fields.sort_by(|a, b| a.1.replacement_name.cmp(&b.1.replacement_name));

        let mut written_array_element_polyfill: HashSet<TypeId> = HashSet::new();
        // {h,f} x m[column][row] for each matrix type
        let mut written_uniform_matrix_polyfill = [[[false; 5]; 5]; 2];
        // {h,f} x for each matrix row-size
        let mut written_uniform_row_polyfill = [[false; 5]; 2];
        let mut any_field_accessed = false;
        for ((struct_type, field_index), info) in &ordered_fields {
            let field: Field = self.ctx.pool.ty(*struct_type).fields()[*field_index].clone();
            let mut field_type = field.ty;
            let field_layout = &field.layout;

            if info.is_array {
                field_type = self.ctx.pool.ty(field_type).component_type().id();
                if written_array_element_polyfill.insert(field_type) {
                    let (abbreviated, columns, rows) = {
                        let t = self.ctx.pool.ty(field_type);
                        if info.is_matrix {
                            (t.abbreviated_name, t.columns(), t.rows())
                        } else {
                            (t.abbreviated_name, 0, 0)
                        }
                    };
                    self.write("struct _skArrayElement_");
                    self.write(abbreviated);
                    self.write_line(" {");

                    if info.is_matrix {
                        // Create a struct representing the array containing std140-padded
                        // matrices.
                        self.write("  e : _skMatrix");
                        self.write(&columns.to_string());
                        self.write(&rows.to_string());
                        let low = type_is_low_precision(self.ctx, field_type);
                        self.write_line(if low { "h" } else { "f" });
                    } else {
                        // Create a struct representing the array with extra padding between
                        // elements.
                        self.write("  @align(16) e : ");
                        let wgsl = to_wgsl_type(self.ctx, field_type, Some(field_layout), false);
                        self.write_line(&wgsl);
                    }
                    self.write_line("};");
                }
            }

            if info.is_matrix {
                // Create structs representing the matrix as an array of vectors, whether or not
                // the matrix is ever accessed by the SkSL. (The struct itself is mentioned in the
                // list of uniforms.)
                let use_f16 = type_is_low_precision(self.ctx, field_type);
                let t = usize::from(!use_f16);
                let (c, r, component) = {
                    let ty = self.ctx.pool.ty(field_type);
                    (
                        usize::try_from(ty.columns()).expect("positive"),
                        usize::try_from(ty.rows()).expect("positive"),
                        ty.component_type().id(),
                    )
                };
                if !written_uniform_row_polyfill[t][r] {
                    written_uniform_row_polyfill[t][r] = true;

                    self.write("struct _skRow");
                    self.write(&r.to_string());
                    self.write(if use_f16 { "h" } else { "f" });
                    self.write_line(" {");
                    self.write("  @align(16) r : vec");
                    self.write(&r.to_string());
                    self.write("<");
                    let wgsl = to_wgsl_type(self.ctx, component, Some(field_layout), false);
                    self.write(&wgsl);
                    self.write_line(">");
                    self.write_line("};");
                }

                if !written_uniform_matrix_polyfill[t][c][r] {
                    written_uniform_matrix_polyfill[t][c][r] = true;

                    self.write("struct _skMatrix");
                    self.write(&c.to_string());
                    self.write(&r.to_string());
                    self.write(if use_f16 { "h" } else { "f" });
                    self.write_line(" {");
                    self.write("  c : array<_skRow");
                    self.write(&r.to_string());
                    self.write(if use_f16 { "h" } else { "f" });
                    self.write(", ");
                    self.write(&c.to_string());
                    self.write_line(">");
                    self.write_line("};");
                }
            }

            // We create a polyfill variable only if the uniform was actually accessed.
            if !info.was_accessed {
                continue;
            }
            any_field_accessed = true;
            self.write("var<private> ");
            self.write(&info.replacement_name);
            self.write(": ");

            let interface_block_var = self.interface_block_var(info.interface_block);
            let interface_block_type = self.ctx.pool.variable(interface_block_var).ty;
            let (ib_is_array, ib_columns) = {
                let t = self.ctx.pool.ty(interface_block_type);
                (t.is_array(), if t.is_array() { t.columns() } else { 0 })
            };
            if ib_is_array {
                self.write("array<");
                let wgsl = to_wgsl_type(self.ctx, field.ty, Some(field_layout), false);
                self.write(&wgsl);
                self.write(", ");
                self.write(&ib_columns.to_string());
                self.write(">");
            } else {
                let wgsl = to_wgsl_type(self.ctx, field.ty, Some(field_layout), false);
                self.write(&wgsl);
            }
            self.write_line(";");
        }

        // If no fields were actually accessed, _skInitializePolyfilledUniforms will not be called
        // and we can avoid emitting an empty, dead function.
        if !any_field_accessed {
            return;
        }

        self.write_line("fn _skInitializePolyfilledUniforms() {");
        self.indentation += 1;

        for ((struct_type, field_index), info) in &ordered_fields {
            // Only initialize a polyfill global if the uniform was actually accessed.
            if !info.was_accessed {
                continue;
            }
            let field: Field = self.ctx.pool.ty(*struct_type).fields()[*field_index].clone();

            // Synthesize the name of this uniform variable
            let interface_block_var = self.interface_block_var(info.interface_block);
            let mut instance_name = self.ctx.pool.variable(interface_block_var).name.to_string();
            let interface_block_type = self.ctx.pool.variable(interface_block_var).ty;
            let (ib_is_array, ib_columns) = {
                let t = self.ctx.pool.ty(interface_block_type);
                (t.is_array(), if t.is_array() { t.columns() } else { 0 })
            };
            if instance_name.is_empty() {
                let component = self.ctx.pool.ty(interface_block_type).component_type().id();
                instance_name = self
                    .interface_block_name_map
                    .get(&component)
                    .cloned()
                    .unwrap_or_default();
            }

            // Initialize the global variable associated with this uniform.
            // If the interface block is arrayed, the associated global will be arrayed as well.
            let num_ib_elements = if ib_is_array { ib_columns } else { 1 };
            for ib_idx in 0..num_ib_elements {
                self.write(&info.replacement_name);
                if ib_is_array {
                    self.write("[");
                    self.write(&ib_idx.to_string());
                    self.write("]");
                }
                self.write(" = ");

                let mut field_type = field.ty;
                let field_layout = &field.layout;

                let num_array_elements;
                if info.is_array {
                    let wgsl = to_wgsl_type(self.ctx, field_type, Some(field_layout), false);
                    self.write(&wgsl);
                    self.write("(");
                    num_array_elements = self.ctx.pool.ty(field_type).columns();
                    field_type = self.ctx.pool.ty(field_type).component_type().id();
                } else {
                    num_array_elements = 1;
                }

                let mut array_separator = crate::string::Separator::new();
                for array_idx in 0..num_array_elements {
                    self.write(array_separator.next_str());

                    let mut field_name = instance_name.clone();
                    if ib_is_array {
                        field_name.push('[');
                        field_name += &ib_idx.to_string();
                        field_name.push(']');
                    }
                    field_name.push('.');
                    field_name += &self.assemble_name(&field.name);

                    if info.is_array {
                        field_name.push('[');
                        field_name += &array_idx.to_string();
                        field_name += "].e";
                    }

                    if info.is_matrix {
                        let wgsl = to_wgsl_type(self.ctx, field_type, Some(field_layout), false);
                        self.write(&wgsl);
                        self.write("(");
                        let num_columns = self.ctx.pool.ty(field_type).columns();
                        let mut matrix_separator = crate::string::Separator::new();
                        for column in 0..num_columns {
                            self.write(matrix_separator.next_str());
                            self.write(&field_name);
                            self.write(".c[");
                            self.write(&column.to_string());
                            self.write("].r");
                        }
                        self.write(")");
                    } else {
                        self.write(&field_name);
                    }
                }

                if info.is_array {
                    self.write(")");
                }

                self.write_line(";");
            }
        }

        self.indentation -= 1;
        self.write_line("}");
    }

    /// `prepareUniformPolyfillsForInterfaceBlock(interfaceBlock, instanceName, nativeLayout)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4840-L4895 (chrome/m156)
    fn prepare_uniform_polyfills_for_interface_block(
        &mut self,
        interface_block: ElemId,
        instance_name: &str,
        native_layout: Standard,
    ) {
        let std140 = MemoryLayout::new(Standard::Std140);
        let native = MemoryLayout::new(native_layout);

        let struct_type = self.interface_block_struct_type(interface_block);
        let fields: Vec<Field> = self.ctx.pool.ty(struct_type).fields().to_vec();
        for (index, field) in fields.iter().enumerate() {
            let mut need_array_polyfill = false;
            let mut need_matrix_polyfill = false;

            let is_polyfillable_matrix_type = |ctx: &crate::context::Context, ty: TypeId| {
                let t = ctx.pool.ty(ty);
                t.is_matrix() && std140.stride(t) != native.stride(t)
            };

            let (is_array_polyfill_candidate, inner_type) = {
                let t = self.ctx.pool.ty(field.ty);
                (
                    t.is_array() && !t.is_unsized_array() && !t.component_type().is_opaque(),
                    t.component_type().id(),
                )
            };
            if is_polyfillable_matrix_type(self.ctx, field.ty) {
                // Matrices will be represented as 16-byte aligned arrays in std140, and
                // reconstituted into proper matrices as they are later accessed. We need to
                // synthesize polyfill.
                need_matrix_polyfill = true;
            } else if is_array_polyfill_candidate {
                if is_polyfillable_matrix_type(self.ctx, inner_type) {
                    // Use a polyfill when the array contains a matrix that requires polyfill.
                    need_array_polyfill = true;
                    need_matrix_polyfill = true;
                } else if native.size(self.ctx.pool.ty(inner_type)) < 16 {
                    // Use a polyfill when the array elements are smaller than 16 bytes, since
                    // std140 will pad elements to a 16-byte stride.
                    need_array_polyfill = true;
                }
            }

            if need_array_polyfill || need_matrix_polyfill {
                // Add a polyfill for this matrix type.
                let replacement_name = format!(
                    "_skUnpacked_{instance_name}_{}",
                    self.assemble_name(&field.name)
                );
                let info = FieldPolyfillInfo {
                    interface_block,
                    replacement_name,
                    is_array: need_array_polyfill,
                    is_matrix: need_matrix_polyfill,
                    was_accessed: false,
                };
                let key = self.field_key(struct_type, index);
                self.field_polyfill_map.insert(key, info);
            }
        }
    }

    /// `writeUniformsAndBuffers()`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4897-L4905 (chrome/m156)
    pub(super) fn write_uniforms_and_buffers(&mut self) {
        for e in self.elements.clone() {
            // Iterate through the interface blocks.
            if matches!(
                self.ctx.pool.element(e).kind,
                ProgramElementKind::InterfaceBlock(_)
            ) {
                self.write_interface_block(e);
            }
        }
    }

    /// `writeInterfaceBlock(ib)`.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4907-L4975 (chrome/m156)
    pub(super) fn write_interface_block(&mut self, ib: ElemId) {
        // Determine if this interface block holds uniforms, buffers, or something else (skip it).
        let ib_var_id = self.interface_block_var(ib);
        let ib_var = self.ctx.pool.variable(ib_var_id).clone();
        let address_space: &str;
        let mut access_mode = "";
        let native_layout: Standard;
        let is_push_constant = ib_var.layout.flags.contains(LayoutFlags::PUSH_CONSTANT);
        let force_high_precision = self.ctx.config().settings.force_high_precision;
        if ib_var.modifier_flags.is_uniform() {
            address_space = if is_push_constant {
                "immediate"
            } else {
                "uniform"
            };
            native_layout = if force_high_precision {
                Standard::WgslUniformBase
            } else {
                Standard::WgslUniformEnableF16
            };
        } else if ib_var.modifier_flags.is_buffer() {
            address_space = "storage";
            native_layout = if force_high_precision {
                Standard::WgslStorageBase
            } else {
                Standard::WgslStorageEnableF16
            };
            access_mode = if ib_var.modifier_flags.is_read_only() {
                ", read"
            } else {
                ", read_write"
            };
        } else {
            return;
        }

        // If we have an anonymous interface block, assign a name like `_uniform0` or `_storage1`.
        let struct_type = self.interface_block_struct_type(ib);
        let instance_name: String;
        if ib_var.name.is_empty() {
            instance_name = format!("_{address_space}{}", self.scratch_count);
            self.scratch_count += 1;
            self.interface_block_name_map
                .insert(struct_type, instance_name.clone());
        } else {
            instance_name = ib_var.name.to_string();
        }

        self.prepare_uniform_polyfills_for_interface_block(ib, &instance_name, native_layout);

        // Create a struct to hold all of the fields from this InterfaceBlock.
        let type_name = InterfaceBlock { var: ib_var_id }
            .type_name(&self.ctx.pool)
            .to_owned();
        debug_assert_ne!(type_name, "");
        self.write("struct ");
        self.write(&type_name);
        self.write_line(" {");

        // Find the struct type and fields used by this interface block.
        debug_assert!(self.ctx.pool.ty(struct_type).is_struct());
        debug_assert_ne!(self.ctx.pool.ty(struct_type).fields().len(), 0);

        let layout = MemoryLayout::new(Standard::Std140);
        self.write_fields(struct_type, Some(layout));
        self.write_line("};");
        if !is_push_constant {
            self.write("@group(");
            self.write(&ib_var.layout.set.max(0).to_string());
            self.write(") @binding(");
            self.write(&ib_var.layout.binding.max(0).to_string());
            self.write(") ");
        }
        self.write("var<");
        self.write(address_space);
        self.write(access_mode);
        self.write("> ");
        self.write(&instance_name);
        self.write(" : ");
        let wgsl = to_wgsl_type(self.ctx, ib_var.ty, Some(&ib_var.layout), false);
        self.write(&wgsl);
        self.write_line(";");
    }

    /// `writeNonBlockUniformsForTests()`: writes all top-level non-opaque global uniform
    /// declarations (i.e. not part of an interface block) into a single uniform block binding.
    ///
    /// In complete fragment/vertex/compute programs, uniforms will be declared only as interface
    /// blocks and global opaque types (like textures and samplers) which we expect to be declared
    /// with a unique binding and descriptor set index. However, test files that are declared as
    /// RTE programs may contain OpenGL-style global uniform declarations with no clear binding
    /// index to use for the containing synthesized block.
    ///
    /// Since we are handling these variables only to generate gold files from RTEs and never run
    /// them, we always declare them at the default bind group and binding index.
    // Port of: src/sksl/codegen/SkSLWGSLCodeGenerator.cpp#L4977-L5014 (chrome/m156)
    pub(super) fn write_non_block_uniforms_for_tests(&mut self) {
        let mut global_var_fields: Vec<Field> = Vec::new();
        let mut global_var_pos = Position::default();
        for e in self.elements.clone() {
            let ProgramElementKind::GlobalVar(decls) = &self.ctx.pool.element(e).kind else {
                continue;
            };
            let var_id = decls.var_declaration(&self.ctx.pool).var;
            let var = self.ctx.pool.variable(var_id);

            if self.is_in_global_uniforms(var) {
                if global_var_fields.is_empty() {
                    global_var_pos = var.position;
                }
                global_var_fields.push(Field {
                    position: var.position,
                    layout: var.layout,
                    modifier_flags: ModifierFlags::empty(),
                    name: var.name.clone(),
                    ty: var.ty,
                });
            }
        }

        if !global_var_fields.is_empty() {
            let settings = self.ctx.config().settings;
            let layout = Layout {
                flags: LayoutFlags::empty(),
                location: 0,
                offset: 0,
                binding: settings.default_uniform_binding,
                index: 0,
                set: settings.default_uniform_set,
                builtin: 0,
                input_attachment_index: 0,
                ..Layout::new()
            };
            let modifiers = Modifiers {
                position: global_var_pos,
                layout,
                flags: ModifierFlags::UNIFORM,
            };

            // Interface blocks (for SkSL) are normally only allowed in fragment, vertex, or
            // compute programs and not the synthetic SkSL files that are allowed to declare
            // non-block uniforms. Arbitrarily pick kFragment to temporarily override the config
            // kind to pass validations.
            let orig = self.ctx.config().kind;
            self.ctx
                .config
                .as_mut()
                .expect("the program's configuration is set")
                .kind = ProgramKind::Fragment;
            let block = InterfaceBlock::convert(
                self.ctx,
                global_var_pos,
                &modifiers,
                "_GlobalUniforms",
                global_var_fields,
                "_globalUniforms",
                /*array_size=*/ 0,
            );
            self.synthetic_global_uniforms_block = block;
            if let Some(block) = block {
                self.write_interface_block(block);
            }
            self.ctx
                .config
                .as_mut()
                .expect("the program's configuration is set")
                .kind = orig;
        }
    }
}
