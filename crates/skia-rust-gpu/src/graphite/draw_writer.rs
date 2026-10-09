// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawWriter.h, src/gpu/graphite/DrawWriter.cpp

//! [`DrawWriter`]: records draws whose vertex and instance data is computed at record time, into a
//! [`DrawPassCommandList`] and the GPU buffers of a [`DrawBufferManager`].

use crate::gpu::buffer_writer::VertexWriter;
use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::buffer_manager::{BufferSubAllocator, DrawBufferManager};
use crate::graphite::draw_types::{BarrierType, PrimitiveType, RenderStateFlags};

/// The draw commands a [`DrawWriter`] records (`DrawPassCommands::List`).
///
/// `DrawPassCommands::List` is the command list of `DrawPass`, which is not ported yet (G10a).
/// This trait is the seam: it lists exactly the methods `DrawWriter.cpp` calls on the list, so
/// the `DrawPass` port implements it without changing `DrawWriter`.
// Port of: src/gpu/graphite/DrawCommands.h (the List methods that DrawWriter.cpp calls)
#[doc(alias = "skgpu::graphite::DrawPassCommands::List")]
pub trait DrawPassCommandList {
    /// `bindAppendDataBuffer`.
    fn bind_append_data_buffer(&mut self, append_attribs: BindBufferInfo);
    /// `bindStaticDataBuffer`.
    fn bind_static_data_buffer(&mut self, static_attribs: BindBufferInfo);
    /// `bindIndexBuffer`.
    fn bind_index_buffer(&mut self, indices: BindBufferInfo);
    /// `addBarrier`.
    fn add_barrier(&mut self, barrier: BarrierType);
    /// `draw(type, baseVertex, vertexCount)`.
    fn draw(&mut self, primitive: PrimitiveType, base_vertex: u32, vertex_count: u32);
    /// `drawIndexed(type, baseIndex, indexCount, baseVertex)`.
    fn draw_indexed(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
    );
    /// `drawInstanced(type, baseVertex, vertexCount, baseInstance, instanceCount)`.
    fn draw_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_vertex: u32,
        vertex_count: u32,
        base_instance: u32,
        instance_count: u32,
    );
    /// `drawIndexedInstanced(type, baseIndex, indexCount, baseVertex, baseInstance,
    /// instanceCount)`.
    #[allow(clippy::too_many_arguments)] // Mirrors DrawCommands.h's parameter list.
    fn draw_indexed_instanced(
        &mut self,
        primitive: PrimitiveType,
        base_index: u32,
        index_count: u32,
        base_vertex: u32,
        base_instance: u32,
        instance_count: u32,
    );
}

/// `DrawWriter`: helper around recording draws when the number of draws is not known ahead of
/// time, or the vertex and instance data is computed at record time.
// Port of: src/gpu/graphite/DrawWriter.h#L45-L230 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawWriter")]
pub struct DrawWriter<'a> {
    command_list: &'a mut dyn DrawPassCommandList,
    manager: &'a DrawBufferManager,
    current_buffer: BufferSubAllocator,
    // Storage for the VertexWriter when GPU buffer mapping fails (`fFailureStorage`).
    failure_storage: Vec<u8>,
    render_state: RenderStateFlags,
    primitive_type: PrimitiveType,
    barrier_to_issue_before_draws: BarrierType,
    static_stride: u32,
    append_stride: u32,
    append: BindBufferInfo,
    static_data: BindBufferInfo,
    indices: BindBufferInfo,
    bound_append: BindBufferInfo,
    bound_static: BindBufferInfo,
    bound_indices: BindBufferInfo,
    template_count: u32,
    pending_count: u32,
    // `fAppender` (debug only): set while an appender (Vertices, Instances, DynamicInstances)
    // is alive.
    appender_active: bool,
}

impl std::fmt::Debug for DrawWriter<'_> {
    // The command list is a trait object with no Debug, so it is left out.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DrawWriter")
            .field("render_state", &self.render_state)
            .field("primitive_type", &self.primitive_type)
            .field("static_stride", &self.static_stride)
            .field("append_stride", &self.append_stride)
            .field("template_count", &self.template_count)
            .field("pending_count", &self.pending_count)
            .finish_non_exhaustive()
    }
}

impl<'a> DrawWriter<'a> {
    /// `DrawWriter(commandList, bufferManager)`.
    // Port of: src/gpu/graphite/DrawWriter.cpp#L16-L33 (chrome/m156)
    #[must_use]
    pub fn new(
        command_list: &'a mut dyn DrawPassCommandList,
        buffer_manager: &'a DrawBufferManager,
    ) -> Self {
        Self {
            command_list,
            manager: buffer_manager,
            current_buffer: BufferSubAllocator::default(),
            failure_storage: Vec::new(),
            render_state: RenderStateFlags::NONE,
            primitive_type: PrimitiveType::Triangles,
            barrier_to_issue_before_draws: BarrierType::None,
            static_stride: 0,
            append_stride: 0,
            append: BindBufferInfo::default(),
            static_data: BindBufferInfo::default(),
            indices: BindBufferInfo::default(),
            bound_append: BindBufferInfo::default(),
            bound_static: BindBufferInfo::default(),
            bound_indices: BindBufferInfo::default(),
            template_count: 0,
            pending_count: 0,
            appender_active: false,
        }
    }

    /// `bufferManager()`.
    // Port of: src/gpu/graphite/DrawWriter.h#L52 (chrome/m156)
    #[must_use]
    pub const fn buffer_manager(&self) -> &DrawBufferManager {
        self.manager
    }

    /// `flush()`.
    // Port of: src/gpu/graphite/DrawWriter.h#L53 (chrome/m156)
    pub fn flush(&mut self) {
        let default_append = self.default_append_binding();
        self.flush_internal(&default_append);
    }

    /// `newDynamicState()`: call before recording modifications to other dynamic state.
    // Port of: src/gpu/graphite/DrawWriter.h#L54 (chrome/m156)
    pub fn new_dynamic_state(&mut self) {
        self.flush();
    }

    /// `newPipelineState(...)`: call before binding a new pipeline with a different layout.
    ///
    /// # Panics
    /// If a stride does not fit in `u32`.
    // Port of: src/gpu/graphite/DrawWriter.h#L56-L72 (chrome/m156)
    pub fn new_pipeline_state(
        &mut self,
        primitive: PrimitiveType,
        static_stride: usize,
        append_stride: usize,
        new_render_state: RenderStateFlags,
        barrier_type: BarrierType,
    ) {
        self.flush();
        debug_assert_eq!(self.pending_count, 0);
        self.primitive_type = primitive;
        self.static_stride = u32::try_from(static_stride).expect("stride fits in u32");
        self.append_stride = u32::try_from(append_stride).expect("stride fits in u32");
        self.render_state = new_render_state;
        let base_align = if new_render_state.contains(RenderStateFlags::APPEND_VERTICES) {
            4 * self.append_stride
        } else {
            self.append_stride
        };
        // `std::tie(std::ignore, fAppend) = fCurrentBuffer.getMappedSubrange(0, stride, align)`.
        self.append = self
            .current_buffer
            .get_mapped_subrange(0, self.append_stride as usize, base_align as usize)
            .map_or_else(BindBufferInfo::default, |(_writer, binding)| binding);
        self.barrier_to_issue_before_draws = barrier_type;
    }

    /// `appendStride()`, `staticStride()` and `primitiveType()`: debug-only accessors in Skia.
    // Port of: src/gpu/graphite/DrawWriter.h#L74-L78 (chrome/m156), SK_DEBUG only
    #[must_use]
    pub const fn append_stride(&self) -> u32 {
        self.append_stride
    }

    /// `staticStride()`.
    // Port of: src/gpu/graphite/DrawWriter.h#L75 (chrome/m156), SK_DEBUG only
    #[must_use]
    pub const fn static_stride(&self) -> u32 {
        self.static_stride
    }

    /// `primitiveType()`.
    // Port of: src/gpu/graphite/DrawWriter.h#L76 (chrome/m156), SK_DEBUG only
    #[must_use]
    pub const fn primitive_type(&self) -> PrimitiveType {
        self.primitive_type
    }

    /// `draw(vertices, vertexCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L86-L88 (chrome/m156)
    pub fn draw(&mut self, vertices: BindBufferInfo, vertex_count: u32) {
        self.bind_and_flush(
            BindBufferInfo::default(),
            BindBufferInfo::default(),
            vertices,
            0,
            vertex_count,
        );
    }

    /// `drawIndexed(vertices, indices, indexCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L90-L92 (chrome/m156)
    pub fn draw_indexed(
        &mut self,
        vertices: BindBufferInfo,
        indices: BindBufferInfo,
        index_count: u32,
    ) {
        self.bind_and_flush(vertices, indices, BindBufferInfo::default(), 0, index_count);
    }

    /// `drawInstanced(vertices, vertexCount, instances, instanceCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L94-L98 (chrome/m156)
    pub fn draw_instanced(
        &mut self,
        vertices: BindBufferInfo,
        vertex_count: u32,
        instances: BindBufferInfo,
        instance_count: u32,
    ) {
        debug_assert!(vertex_count > 0);
        self.bind_and_flush(
            vertices,
            BindBufferInfo::default(),
            instances,
            vertex_count,
            instance_count,
        );
    }

    /// `drawIndexedInstanced(vertices, indices, indexCount, instances, instanceCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L100-L106 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // Mirrors the C++ signature.
    pub fn draw_indexed_instanced(
        &mut self,
        vertices: BindBufferInfo,
        indices: BindBufferInfo,
        index_count: u32,
        instances: BindBufferInfo,
        instance_count: u32,
    ) {
        debug_assert!(index_count > 0);
        self.bind_and_flush(vertices, indices, instances, index_count, instance_count);
    }

    /// `getLastAppendedBuffer()` (`GPU_TEST_UTILS` only in Skia).
    // Port of: src/gpu/graphite/DrawWriter.h#L110-L112 (chrome/m156)
    #[must_use]
    pub fn last_appended_buffer(&self) -> BindBufferInfo {
        self.default_append_binding()
    }

    // Port of: src/gpu/graphite/DrawWriter.h#L183-L186 (chrome/m156)
    fn default_append_binding(&self) -> BindBufferInfo {
        BindBufferInfo {
            buffer: self.append.buffer.clone(),
            offset: self.append.offset,
            size: self.pending_count * self.append_stride,
        }
    }

    // Port of: src/gpu/graphite/DrawWriter.h#L188-L198 (chrome/m156)
    fn set_template(
        &mut self,
        static_data: BindBufferInfo,
        indices: BindBufferInfo,
        template_count: u32,
    ) {
        if self.pending_count == 0 {
            self.static_data = static_data;
            self.indices = indices;
            self.template_count = template_count;
        } else {
            debug_assert!(self.static_data == static_data && self.indices == indices);
            debug_assert!(
                self.append_stride == 0 || self.append.offset.is_multiple_of(self.append_stride)
            );
            debug_assert!(
                (template_count == 0
                    && self
                        .render_state
                        .contains(RenderStateFlags::APPEND_DYNAMIC_INSTANCES))
                    || self.template_count == template_count
            );
        }
    }

    // Port of: src/gpu/graphite/DrawWriter.h#L200-L209 (chrome/m156)
    // Mirrors `bindAndFlush`, which takes its binding infos by value.
    #[allow(clippy::needless_pass_by_value)]
    fn bind_and_flush(
        &mut self,
        static_data: BindBufferInfo,
        indices: BindBufferInfo,
        append_data: BindBufferInfo,
        template_count: u32,
        draw_count: u32,
    ) {
        debug_assert!(draw_count > 0);
        debug_assert!(!self.appender_active); // Never append and draw manually at the same time.
        debug_assert_eq!(self.pending_count, 0); // Any prior appends must have been flushed.
        self.set_template(static_data, indices, template_count);
        self.pending_count = draw_count;
        self.flush_internal(&append_data);
    }

    // Port of: src/gpu/graphite/DrawWriter.cpp#L36-L143 (chrome/m156)
    // The body follows `DrawWriter::flushInternal` step by step; splitting it would separate the
    // bind decisions from the draw calls they feed.
    #[allow(clippy::too_many_lines)]
    fn flush_internal(&mut self, append_data: &BindBufferInfo) {
        // Skip flush if no items appended, or dynamic instances resolved to zero count.
        if self.pending_count == 0
            || (self
                .render_state
                .contains(RenderStateFlags::APPEND_DYNAMIC_INSTANCES)
                && self.template_count == 0)
        {
            return;
        }

        // How much to advance `append.offset` when the flush is completed.
        let mut advance_offset = self.pending_count;

        // ARM hardware (b/399631317): unreferenced vertices in sequential indexes of 4 are
        // speculatively executed. Pad the buffer and zero the padding.
        if self
            .render_state
            .contains(RenderStateFlags::APPEND_VERTICES)
        {
            let count_diff = align4(self.pending_count) - self.pending_count;
            if count_diff != 0 {
                let mut z_writer = self
                    .current_buffer
                    .append_mapped_with_stride(count_diff as usize)
                    .expect("the buffer was sized to hold an aligned total");
                z_writer.zero_bytes(count_diff as usize * self.append_stride as usize);
                advance_offset += count_diff;
            }
        }

        // Calculate base offsets from buffer info for the draw commands.
        // - If a valid buffer exists and its offset is stride-aligned, use the pending base as a
        //   pseudo-alias for the offset and rebind the whole buffer.
        // - Otherwise draw from the start of the offset with pending base 0 and bind the buffer.
        let bind = |buffer: &BindBufferInfo,
                    stride: u32,
                    bound: &mut BindBufferInfo,
                    pending_base: &mut u32|
         -> bool {
            let mut should_bind = false;
            if buffer.is_valid() {
                let mut new_binding = buffer.clone();
                if buffer.offset.is_multiple_of(stride) {
                    *pending_base = buffer.offset / stride;
                    new_binding = BindBufferInfo {
                        buffer: buffer.buffer.clone(),
                        offset: 0,
                        size: buffer.buffer.as_ref().map_or(0, |b| {
                            u32::try_from(b.size()).expect("buffer size fits in u32")
                        }),
                    };
                }
                should_bind = *bound != new_binding;
                *bound = new_binding;
            }
            should_bind
        };

        let mut pending_base_append = 0_u32;
        let mut pending_base_static = 0_u32;
        let mut pending_base_indices = 0_u32;
        if bind(
            append_data,
            self.append_stride,
            &mut self.bound_append,
            &mut pending_base_append,
        ) {
            self.command_list
                .bind_append_data_buffer(self.bound_append.clone());
        }
        if bind(
            &self.static_data,
            self.static_stride,
            &mut self.bound_static,
            &mut pending_base_static,
        ) {
            self.command_list
                .bind_static_data_buffer(self.bound_static.clone());
        }
        if bind(
            &self.indices,
            INDEX_SIZE,
            &mut self.bound_indices,
            &mut pending_base_indices,
        ) {
            self.command_list
                .bind_index_buffer(self.bound_indices.clone());
        }

        // Before any draw commands are added, issue the barrier type assigned to this writer.
        if self.barrier_to_issue_before_draws != BarrierType::None {
            self.command_list
                .add_barrier(self.barrier_to_issue_before_draws);
        }

        // Issue the draw (instanced or not) based on the current template count. Because of the
        // initial AppendDynamicInstances && template_count check, a DynamicInstance step has a
        // non-zero template count here.
        if self.template_count != 0 {
            debug_assert!(
                (pending_base_append + self.pending_count) * self.append_stride
                    <= self.bound_append.size
            );
            if self.indices.is_valid() {
                // Only the index count that is drawn can be validated against the index data.
                debug_assert!(
                    self.template_count as usize * std::mem::size_of::<u16>()
                        <= self.indices.size as usize
                );
                self.command_list.draw_indexed_instanced(
                    self.primitive_type,
                    pending_base_indices,
                    self.template_count,
                    pending_base_static,
                    pending_base_append,
                    self.pending_count,
                );
            } else {
                debug_assert!(
                    self.template_count as usize * self.static_stride as usize
                        <= self.static_data.size as usize
                );
                self.command_list.draw_instanced(
                    self.primitive_type,
                    pending_base_static,
                    self.template_count,
                    pending_base_append,
                    self.pending_count,
                );
            }
            // Clear instancing template state after the draw is recorded for non-Fixed state.
            if self
                .render_state
                .contains(RenderStateFlags::APPEND_DYNAMIC_INSTANCES)
            {
                self.template_count = 0;
            }
        } else if self.indices.is_valid() {
            // Indexed, non-instanced draw.
            debug_assert!(
                self.pending_count as usize * std::mem::size_of::<u16>()
                    <= self.indices.size as usize
            );
            self.command_list.draw_indexed(
                self.primitive_type,
                pending_base_indices,
                self.pending_count,
                pending_base_append,
            );
        } else {
            debug_assert!(
                (pending_base_append + self.pending_count) * self.static_stride
                    <= self.bound_append.size
            );
            self.command_list
                .draw(self.primitive_type, pending_base_append, self.pending_count);
        }

        // Mark all appended items as drawn and advance the base offset.
        self.append.offset += advance_offset * self.append_stride;
        self.pending_count = 0;
    }

    /// `realloc<AppendVertices>(count)`: maps a new append buffer with room for `count` items.
    // Port of: src/gpu/graphite/DrawWriter.h#L212-L222 (chrome/m156)
    fn realloc(&mut self, count: u32, append_vertices: bool) {
        self.flush();
        debug_assert_eq!(
            append_vertices,
            self.render_state
                .contains(RenderStateFlags::APPEND_VERTICES)
        );
        let base_multiple: u32 = if append_vertices { 4 } else { 1 };
        let reserved = align_to(count, base_multiple);
        let mapped = self.manager.get_mapped_vertex_buffer(
            0,
            self.append_stride as usize,
            reserved as usize,
            (base_multiple * self.append_stride) as usize,
        );
        // A failed mapping leaves the writer with no buffer, so every append takes the
        // failure-storage path.
        if let Some(info) = mapped {
            self.append = info.binding;
            self.current_buffer = info.allocator;
        } else {
            self.append = BindBufferInfo::default();
            self.current_buffer = BufferSubAllocator::default();
        }
    }

    /// `append(count)`: reserves `count` items of the append stride and returns a writer for
    /// them. A failed mapping returns a writer into scratch storage, so the caller always has a
    /// valid place to write.
    // Port of: src/gpu/graphite/DrawWriter.h#L224-L238 (chrome/m156)
    fn append(&mut self, count: u32) -> VertexWriter<'_> {
        debug_assert!(count > 0);
        // Fields are borrowed directly (not through a `&mut self` method) so that the
        // scratch-storage branch can use `failure_storage` while the buffer writer is alive.
        if let Some(writer) = self
            .current_buffer
            .append_mapped_with_stride(count as usize)
        {
            self.pending_count += count;
            VertexWriter::from_buffer_writer(writer)
        } else {
            let size = u64::from(count) * u64::from(self.append_stride);
            let size = usize::try_from(size).expect("container size fits in size_t");
            self.failure_storage.resize(size, 0);
            VertexWriter::new(&mut self.failure_storage[..size])
        }
    }
}

/// The size of one index, `sizeof(uint16_t)`, as the `u32` that the bind decisions use.
const INDEX_SIZE: u32 = 2;

/// `SkAlign4` for `u32`.
// Port of: include/private/base/SkAlign.h (SkAlign4), used by DrawWriter.cpp
const fn align4(x: u32) -> u32 {
    (x + 3) & !3
}

/// `SkAlignTo(count, multiple)` for `u32`.
// Port of: include/private/base/SkAlign.h (SkAlignTo), used by DrawWriter.h
const fn align_to(x: u32, multiple: u32) -> u32 {
    x.div_ceil(multiple) * multiple
}

/// `DrawWriter::Appender`: the base of `Vertices`, `Instances` and `DynamicInstances`. It marks
/// the writer as appending until it is dropped.
// Port of: src/gpu/graphite/DrawWriter.h#L240-L257 (chrome/m156)
struct Appender<'w, 'a> {
    drawer: &'w mut DrawWriter<'a>,
}

impl std::fmt::Debug for Appender<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Appender")
            .field("drawer", &self.drawer)
            .finish()
    }
}

impl<'w, 'a> Appender<'w, 'a> {
    fn new(drawer: &'w mut DrawWriter<'a>, render_state: RenderStateFlags) -> Self {
        debug_assert!(drawer.append_stride > 0);
        debug_assert!(!drawer.appender_active);
        debug_assert_eq!(drawer.render_state, render_state);
        drawer.appender_active = true;
        Self { drawer }
    }
}

impl Drop for Appender<'_, '_> {
    fn drop(&mut self) {
        debug_assert!(self.drawer.appender_active);
        self.drawer.appender_active = false;
    }
}

/// `DrawWriter::Vertices`: appends vertices whose count is not known ahead of time.
// Port of: src/gpu/graphite/DrawWriter.h#L259-L278 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawWriter::Vertices")]
#[derive(Debug)]
pub struct Vertices<'w, 'a> {
    appender: Appender<'w, 'a>,
}

impl<'w, 'a> Vertices<'w, 'a> {
    /// `Vertices(writer)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L262-L266 (chrome/m156)
    pub fn new(writer: &'w mut DrawWriter<'a>) -> Self {
        writer.set_template(BindBufferInfo::default(), BindBufferInfo::default(), 0);
        Self {
            appender: Appender::new(writer, RenderStateFlags::APPEND_VERTICES),
        }
    }

    /// `reserve(count)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L267-L274 (chrome/m156)
    pub fn reserve(&mut self, count: u32) {
        let drawer = &mut *self.appender.drawer;
        let pending = drawer.pending_count;
        let count = std::cmp::max(align4(pending + count) - pending, count);
        if count > drawer.current_buffer.available_with_stride() {
            drawer.realloc(count, true);
        }
    }

    /// `append(count)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L275-L278 (chrome/m156)
    pub fn append(&mut self, count: u32) -> VertexWriter<'_> {
        self.reserve(count);
        self.appender.drawer.append(count)
    }
}

/// `DrawWriter::Instances`: appends instances drawn with a fixed vertex template.
// Port of: src/gpu/graphite/DrawWriter.h#L281-L305 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawWriter::Instances")]
#[derive(Debug)]
pub struct Instances<'w, 'a> {
    appender: Appender<'w, 'a>,
}

impl<'w, 'a> Instances<'w, 'a> {
    /// `Instances(writer, vertices, indices, vertexCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L284-L292 (chrome/m156)
    pub fn new(
        writer: &'w mut DrawWriter<'a>,
        vertices: BindBufferInfo,
        indices: BindBufferInfo,
        vertex_count: u32,
    ) -> Self {
        debug_assert!(vertex_count > 0);
        writer.set_template(vertices, indices, vertex_count);
        Self {
            appender: Appender::new(writer, RenderStateFlags::APPEND_INSTANCES),
        }
    }

    /// `reserve(count)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L294-L300 (chrome/m156)
    pub fn reserve(&mut self, count: u32) {
        let drawer = &mut *self.appender.drawer;
        if count > drawer.current_buffer.available_with_stride() {
            drawer.realloc(count, false);
        }
    }

    /// `append(count)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L302-L305 (chrome/m156)
    pub fn append(&mut self, count: u32) -> VertexWriter<'_> {
        self.reserve(count);
        self.appender.drawer.append(count)
    }
}

/// The per-instance vertex count that a [`DynamicInstances`] appender accumulates
/// (`VertexCountProxy`). Its `operator<<` becomes [`accumulate`](Self::accumulate) and its
/// `operator uint32_t` becomes [`count`](Self::count).
// Port of: src/gpu/graphite/DrawWriter.h#L329-L331 (chrome/m156), the VertexCountProxy protocol
pub trait VertexCountProxy: Default {
    /// The argument that `append` takes to describe one instance.
    type Input;
    /// `proxy << value`.
    fn accumulate(&mut self, value: &Self::Input);
    /// `static_cast<uint32_t>(proxy)`.
    fn count(&self) -> u32;
}

/// `DrawWriter::DynamicInstances`: appends instances whose template vertex count is only known
/// after all instances are recorded.
// Port of: src/gpu/graphite/DrawWriter.h#L307-L370 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawWriter::DynamicInstances")]
pub struct DynamicInstances<'w, 'a, P: VertexCountProxy> {
    appender: Appender<'w, 'a>,
    proxy: P,
}

impl<'w, 'a, P: VertexCountProxy> DynamicInstances<'w, 'a, P> {
    /// `DynamicInstances(writer, vertices, indices)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L313-L319 (chrome/m156)
    pub fn new(
        writer: &'w mut DrawWriter<'a>,
        vertices: BindBufferInfo,
        indices: BindBufferInfo,
    ) -> Self {
        writer.set_template(vertices, indices, 0);
        Self {
            appender: Appender::new(writer, RenderStateFlags::APPEND_DYNAMIC_INSTANCES),
            proxy: P::default(),
        }
    }

    /// `reserve(count)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L329-L336 (chrome/m156)
    pub fn reserve(&mut self, count: u32) {
        if count > self.appender.drawer.current_buffer.available_with_stride() {
            self.update_template_count();
            self.appender.drawer.realloc(count, false);
        }
    }

    /// `append(vertexCount, instanceCount)`.
    // Port of: src/gpu/graphite/DrawWriter.h#L338-L345 (chrome/m156)
    pub fn append(&mut self, vertex_count: &P::Input, instance_count: u32) -> VertexWriter<'_> {
        self.reserve(instance_count);
        let proxy = &mut self.proxy;
        proxy.accumulate(vertex_count);
        self.appender.drawer.append(instance_count)
    }

    // Port of: src/gpu/graphite/DrawWriter.h#L347-L352 (chrome/m156)
    fn update_template_count(&mut self) {
        let count = self.proxy.count();
        self.appender.drawer.template_count = self.appender.drawer.template_count.max(count);
        self.proxy = P::default();
    }
}

impl<P: VertexCountProxy> std::fmt::Debug for DynamicInstances<'_, '_, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicInstances")
            .field("appender", &self.appender)
            .finish_non_exhaustive()
    }
}

impl<P: VertexCountProxy> Drop for DynamicInstances<'_, '_, P> {
    // Port of: src/gpu/graphite/DrawWriter.h#L321-L323 (chrome/m156)
    fn drop(&mut self) {
        self.update_template_count();
    }
}
