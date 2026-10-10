// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFGraphicStackState.{h,cpp} (chrome/m156)

//! The state of the `q`/`Q` graphic stack of a content stream: which matrix, clip, color and
//! graphic state resource are current, so that a draw emits only what changed.
//!
//! Do not confuse this with [`graphic_state`](crate::graphic_state), the PDF objects that hold
//! the paint's alpha, blend mode and stroke.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::clip_stack::{ClipStack, DeviceSpaceType, Iter, IterStart, WIDE_OPEN_GEN_ID};
use skia_rust_core::color::Color4f;
use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::stream::WStream;
use skia_rust_pathops::path_op::PathOp;

use crate::clip_stack_device::clip_stack_as_path;
use crate::utils::{
    EmptyArea, EmptyPath, EmptyVerb, append_color_component_f, append_rectangle, append_scalar,
    append_transform, apply_graphic_state, apply_pattern, emit_path,
};

// Port of: src/pdf/SkPDFGraphicStackState.cpp#L18-L25 (chrome/m156)
fn emit_pdf_color(color: &Color4f, result: &mut dyn WStream) {
    debug_assert_eq!((color.a - 1.0).abs(), 0.0); // We handle alpha elsewhere.
    append_color_component_f(color.r, result);
    result.write_text(" ");
    append_color_component_f(color.g, result);
    result.write_text(" ");
    append_color_component_f(color.b, result);
    result.write_text(" ");
}

// Port of: src/pdf/SkPDFGraphicStackState.cpp#L27-L30 (chrome/m156)
fn rect_intersect(mut u: Rect, v: Rect) -> Rect {
    if u.is_empty() || v.is_empty() {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    }
    if u.intersect(v) {
        u
    } else {
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Test to see if the clipstack is a simple rect, If so, we can avoid all `PathOps` code and speed
/// thing up.
// Port of: src/pdf/SkPDFGraphicStackState.cpp#L32-L61 (chrome/m156)
fn is_rect(clip_stack: &ClipStack, bounds: &Rect) -> Option<Rect> {
    let mut current_clip = *bounds;
    let mut iter = Iter::new(clip_stack, IterStart::Bottom);
    while let Some(element) = iter.next() {
        let mut element_rect = Rect::new(0.0, 0.0, 0.0, 0.0);
        match element.device_space_type() {
            DeviceSpaceType::Empty => {}
            DeviceSpaceType::Rect => {
                element_rect = *element.device_space_rect();
            }
            _ => return None,
        }
        if element.is_replace_op() {
            current_clip = rect_intersect(*bounds, element_rect);
        } else if element.op() == ClipOp::Intersect {
            current_clip = rect_intersect(current_clip, element_rect);
        } else {
            return None;
        }
    }
    Some(current_clip)
}

// TODO: When there's no expanding clip ops, this function may not be necessary anymore.
// Port of: src/pdf/SkPDFGraphicStackState.cpp#L63-L72 (chrome/m156)
fn is_complex_clip(stack: &ClipStack) -> bool {
    let mut iter = Iter::new(stack, IterStart::Bottom);
    while let Some(element) = iter.next() {
        if element.is_replace_op() {
            return true;
        }
    }
    false
}

// Port of: src/pdf/SkPDFGraphicStackState.cpp#L74-L101 (apply_clip, chrome/m156)
fn apply_clip(stack: &ClipStack, outer_bounds: &Rect, mut f: impl FnMut(&Path)) {
    // assumes clipstack is not complex.
    const HUGE: Rect = Rect::new(-30000.0, -30000.0, 30000.0, 30000.0);
    let mut iter = Iter::new(stack, IterStart::Bottom);
    let mut bounds = *outer_bounds;
    while let Some(element) = iter.next() {
        let mut operand = element.as_device_space_path();
        let op = match element.op() {
            ClipOp::Difference => PathOp::Difference,
            ClipOp::Intersect => PathOp::Intersect,
        };
        if (op == PathOp::Difference
            || operand.is_inverse_fill_type()
            || !HUGE.contains(operand.bounds()))
            && let Some(result) = skia_rust_pathops::op(&Path::rect(bounds, None), &operand, op)
        {
            operand = result;
        }
        debug_assert!(!operand.is_inverse_fill_type());
        f(&operand);
        if !bounds.intersect(*operand.bounds()) {
            return; // return early;
        }
    }
}

// Port of: src/pdf/SkPDFGraphicStackState.cpp#L103-L125 (chrome/m156)
fn append_clip_path(clip_path: &Path, w_stream: &mut dyn WStream) {
    if !emit_path(
        clip_path,
        EmptyPath::Preserve,
        EmptyVerb::Discard,
        EmptyArea::Preserve,
        w_stream,
        0.25,
    ) {
        return;
    }
    let clip_fill = clip_path.fill_type();
    // NOT_IMPLEMENTED(clipFill == SkPathFillType::kInverseEvenOdd, false);
    // NOT_IMPLEMENTED(clipFill == SkPathFillType::kInverseWinding, false);
    if clip_fill == PathFillType::EvenOdd {
        w_stream.write_text("W* n\n");
    } else {
        w_stream.write_text("W n\n");
    }
}

// Port of: src/pdf/SkPDFGraphicStackState.cpp#L127-L158 (chrome/m156)
fn append_clip(clip_stack: &ClipStack, bounds: &IRect, w_stream: &mut dyn WStream) {
    // The bounds are slightly outset to ensure this is correct in the
    // face of floating-point accuracy and possible SkRegion bitmap
    // approximations.
    let outset_bounds = Rect::from_irect(bounds.with_outset((1, 1)));

    if let Some(clip_stack_rect) = is_rect(clip_stack, &outset_bounds) {
        append_rectangle(&clip_stack_rect, w_stream);
        w_stream.write_text("W* n\n");
        return;
    }

    if is_complex_clip(clip_stack) {
        if let Some(clip_path) = skia_rust_pathops::op(
            &clip_stack_as_path(clip_stack),
            &Path::rect(outset_bounds, None),
            PathOp::Intersect,
        ) {
            append_clip_path(&clip_path, w_stream);
        }
        // If Op() fails (pathological case; e.g. input values are
        // extremely large or NaN), emit no clip at all.
    } else {
        apply_clip(clip_stack, &outset_bounds, |path| {
            append_clip_path(path, w_stream);
        });
    }
}

/// `SkPDFGraphicStackState::Entry`: one level of the graphic stack.
// Port of: src/pdf/SkPDFGraphicStackState.h#L25-L32 (chrome/m156)
#[doc(alias = "SkPDFGraphicStackState::Entry")]
#[derive(Debug, Clone)]
pub struct Entry {
    /// `fMatrix`.
    pub matrix: Matrix,
    /// `fClipStackGenID`.
    pub clip_stack_gen_id: u32,
    /// `fColor`.
    pub color: Color4f,
    /// `fTextScaleX`: zero means we don't care what the value is.
    pub text_scale_x: f32,
    /// `fShaderIndex`.
    pub shader_index: i32,
    /// `fGraphicStateIndex`.
    pub graphic_state_index: i32,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            matrix: Matrix::new_identity(),
            clip_stack_gen_id: WIDE_OPEN_GEN_ID,
            color: Color4f::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
            text_scale_x: 1.0,
            shader_index: -1,
            graphic_state_index: -1,
        }
    }
}

/// Which stream of the device a graphic stack writes to (`fContentStream` of Skia, a pointer to
/// the device's `fContent` or `fContentBuffer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSelector {
    /// `fContent`.
    Content,
    /// `fContentBuffer`.
    ContentBuffer,
}

/// `kMaxStackDepth`: must use stack for matrix, and for clip, plus one for no matrix or clip.
const MAX_STACK_DEPTH: usize = 2;

/// `SkPDFGraphicStackState`.
///
/// skia-rust: the stream is given to each method by the device, which owns both; `stream` says
/// which one it is (Skia's `fContentStream`, null when there is none).
// Port of: src/pdf/SkPDFGraphicStackState.h#L21-L45 (chrome/m156)
#[doc(alias = "SkPDFGraphicStackState")]
#[derive(Debug, Clone, Default)]
pub struct GraphicStackState {
    entries: [Entry; MAX_STACK_DEPTH + 1],
    stack_depth: usize,
    /// `fContentStream`.
    pub stream: Option<StreamSelector>,
}

impl GraphicStackState {
    /// `SkPDFGraphicStackState(SkDynamicMemoryWStream* s)`.
    #[must_use]
    pub fn new(stream: Option<StreamSelector>) -> Self {
        Self {
            entries: Default::default(),
            stack_depth: 0,
            stream,
        }
    }

    /// `currentEntry`.
    pub fn current_entry(&mut self) -> &mut Entry {
        &mut self.entries[self.stack_depth]
    }

    /// `updateClip`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L162-L184 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // asserts the C++ preconditions
    pub fn update_clip(
        &mut self,
        out: &mut dyn WStream,
        clip_stack: Option<&ClipStack>,
        bounds: &IRect,
    ) {
        let clip_stack_gen_id = clip_stack.map_or(WIDE_OPEN_GEN_ID, ClipStack::topmost_gen_id);
        if clip_stack_gen_id == self.current_entry().clip_stack_gen_id {
            return;
        }
        while self.stack_depth > 0 {
            self.pop(out);
            if clip_stack_gen_id == self.current_entry().clip_stack_gen_id {
                return;
            }
        }
        debug_assert_eq!(self.current_entry().clip_stack_gen_id, WIDE_OPEN_GEN_ID);
        if clip_stack_gen_id != WIDE_OPEN_GEN_ID {
            let clip_stack = clip_stack.expect("a clip stack with an element");
            self.push(out);

            self.current_entry().clip_stack_gen_id = clip_stack_gen_id;
            append_clip(clip_stack, bounds, out);
        }
    }

    /// `updateMatrix`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L187-L207 (chrome/m156)
    pub fn update_matrix(&mut self, out: &mut dyn WStream, matrix: &Matrix) {
        if *matrix == self.current_entry().matrix {
            return;
        }

        if self.current_entry().matrix.get_type() != TypeMask::empty() {
            debug_assert!(self.stack_depth > 0);
            debug_assert_eq!(
                self.entries[self.stack_depth].clip_stack_gen_id,
                self.entries[self.stack_depth - 1].clip_stack_gen_id
            );
            self.pop(out);

            debug_assert_eq!(self.current_entry().matrix.get_type(), TypeMask::empty());
        }
        if matrix.get_type() == TypeMask::empty() {
            return;
        }

        self.push(out);
        append_transform(matrix, out);
        self.current_entry().matrix = matrix.clone();
    }

    /// `updateDrawingState`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L209-L238 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparisons, as in Skia
    pub fn update_drawing_state(&mut self, out: &mut dyn WStream, state: &Entry) {
        // PDF treats a shader as a color, so we only set one or the other.
        if state.shader_index >= 0 {
            if state.shader_index != self.current_entry().shader_index {
                apply_pattern(state.shader_index, out);
                self.current_entry().shader_index = state.shader_index;
            }
        } else if state.color != self.current_entry().color
            || self.current_entry().shader_index >= 0
        {
            emit_pdf_color(&state.color, out);
            out.write_text("RG ");
            emit_pdf_color(&state.color, out);
            out.write_text("rg\n");
            self.current_entry().color = state.color;
            self.current_entry().shader_index = -1;
        }

        if state.graphic_state_index != self.current_entry().graphic_state_index {
            apply_graphic_state(state.graphic_state_index, out);
            self.current_entry().graphic_state_index = state.graphic_state_index;
        }

        if state.text_scale_x != 0.0 && state.text_scale_x != self.current_entry().text_scale_x {
            let pdf_scale = state.text_scale_x * 100.0;
            append_scalar(pdf_scale, out);
            out.write_text(" Tz\n");
            self.current_entry().text_scale_x = state.text_scale_x;
        }
    }

    /// `push`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L240-L245 (chrome/m156)
    pub fn push(&mut self, out: &mut dyn WStream) {
        debug_assert!(self.stack_depth < MAX_STACK_DEPTH);
        out.write_text("q\n");
        self.stack_depth += 1;
        self.entries[self.stack_depth] = self.entries[self.stack_depth - 1].clone();
    }

    /// `pop`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L247-L252 (chrome/m156)
    pub fn pop(&mut self, out: &mut dyn WStream) {
        debug_assert!(self.stack_depth > 0);
        out.write_text("Q\n");
        self.entries[self.stack_depth] = Entry::default();
        self.stack_depth -= 1;
    }

    /// `drainStack`.
    // Port of: src/pdf/SkPDFGraphicStackState.cpp#L254-L261 (chrome/m156)
    pub fn drain_stack(&mut self, out: &mut dyn WStream) {
        if self.stream.is_some() {
            while self.stack_depth != 0 {
                self.pop(out);
            }
        }
        debug_assert_eq!(self.stack_depth, 0);
    }
}
