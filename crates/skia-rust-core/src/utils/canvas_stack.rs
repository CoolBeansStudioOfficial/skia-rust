// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkCanvasStack.h, src/utils/SkCanvasStack.cpp

//! [`CanvasStack`]: an n-way canvas that owns its canvases, each at an origin, and keeps them
//! from drawing into the space occupied by the canvases above them (`SkCanvasStack`).
//!
//! skia-rust: as for [`NWayCanvas`], the overrides of `SkCanvasStack` are the hooks of the
//! canvas: a stack is an n-way canvas whose hooks have the stack's data, so the functions here
//! are the bodies the n-way hooks call for a stack. The list of the n-way canvas and the stack's
//! `fCanvasData` are kept in step, and the stack's canvases are owned: the canvas data holds the
//! only handle outside the list.

use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

use crate::canvas::Canvas;
use crate::clip_op::ClipOp;
use crate::m44::M44;
use crate::point::IPoint;
use crate::rect::IRect;
use crate::region::{Op, Region};
use crate::utils::n_way_canvas::{CanvasList, NWayCanvas};

/// What a stack keeps for each of its canvases (`SkCanvasStack::CanvasData`).
// Port of: src/utils/SkCanvasStack.h#L58-L67 (chrome/m156)
#[derive(Debug)]
pub(crate) struct CanvasData {
    origin: IPoint,
    required_clip: Region,
    // `ownedCanvas`: dropped with the data.
    #[allow(dead_code)] // held for its drop, like the unique_ptr it stands for
    owned_canvas: Rc<Canvas>,
}

type StackData = Rc<RefCell<Vec<CanvasData>>>;

// Port of: src/utils/SkCanvasStack.cpp#L52-L58 (chrome/m156)
pub(crate) fn clip_to_z_ordered_bounds(list: &CanvasList, data: &StackData) {
    debug_assert_eq!(list.borrow().len(), data.borrow().len());
    let mut i = 0;
    loop {
        let (canvas, clip) = {
            let list = list.borrow();
            let data = data.borrow();
            let Some(canvas) = list.get(i) else {
                break;
            };
            (Rc::clone(canvas), data[i].required_clip.clone())
        };
        canvas.clip_region(&clip, None);
        i += 1;
    }
}

// Port of: src/utils/SkCanvasStack.cpp#L62-L69 (chrome/m156)
pub(crate) fn did_set_m44(list: &CanvasList, data: &StackData, mx: &M44) {
    debug_assert_eq!(list.borrow().len(), data.borrow().len());
    let mut i = 0;
    loop {
        let (canvas, origin) = {
            let list = list.borrow();
            let data = data.borrow();
            let Some(canvas) = list.get(i) else {
                break;
            };
            (Rc::clone(canvas), data[i].origin)
        };
        #[allow(clippy::cast_precision_loss)] // SkIntToScalar
        let translate = M44::translate(-(origin.x as f32), -(origin.y as f32), 0.0);
        canvas.set_matrix(&(&translate * mx));
        i += 1;
    }
    // this->SkCanvas::didSetM44(mx) is the base class's, which does nothing
}

// Port of: src/utils/SkCanvasStack.cpp#L95-L105 (chrome/m156)
pub(crate) fn on_clip_region(list: &CanvasList, data: &StackData, device_rgn: &Region, op: ClipOp) {
    debug_assert_eq!(list.borrow().len(), data.borrow().len());
    let mut i = 0;
    loop {
        let (canvas, temp_region) = {
            let list = list.borrow();
            let data = data.borrow();
            let Some(canvas) = list.get(i) else {
                break;
            };
            let mut temp_region = Region::new();
            device_rgn.translate_to(-data[i].origin.x, -data[i].origin.y, &mut temp_region);
            temp_region.op_region(&data[i].required_clip, Op::Intersect);
            (Rc::clone(canvas), temp_region)
        };
        canvas.clip_region(&temp_region, op);
        i += 1;
    }
}

/// An n-way canvas that owns its canvases (`SkCanvasStack`).
// Port of: src/utils/SkCanvasStack.h#L28-L74 (chrome/m156)
#[doc(alias = "SkCanvasStack")]
pub struct CanvasStack {
    n_way: NWayCanvas,
    list: CanvasList,
    canvas_data: StackData,
}

impl std::fmt::Debug for CanvasStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasStack")
            .field("canvas_count", &self.canvas_data.borrow().len())
            .finish_non_exhaustive()
    }
}

impl CanvasStack {
    /// A stack of `width` by `height` with no canvases (`SkCanvasStack(int, int)`).
    // Port of: src/utils/SkCanvasStack.cpp#L22-L23 (chrome/m156)
    #[must_use]
    pub fn new(width: i32, height: i32) -> CanvasStack {
        let canvas_data: StackData = Rc::new(RefCell::new(Vec::new()));
        let (canvas, list) = NWayCanvas::make_canvas(width, height, Some(Rc::clone(&canvas_data)));
        CanvasStack {
            n_way: NWayCanvas::from_parts(canvas, Rc::clone(&list)),
            list,
            canvas_data,
        }
    }

    /// Pushes a canvas on the top of the stack, at `origin`; the stack owns it
    /// (`pushCanvas`).
    // Port of: src/utils/SkCanvasStack.cpp#L27-L50 (chrome/m156)
    #[doc(alias = "pushCanvas")]
    pub fn push_canvas(&self, canvas: Canvas, origin: impl Into<IPoint>) {
        let origin = origin.into();
        let canvas = Rc::new(canvas);

        // compute the bounds of this canvas
        let canvas_bounds = IRect::from_size(canvas.base_layer_size());

        // push the canvas onto the stack
        self.list.borrow_mut().push(Rc::clone(&canvas));

        // push the canvas data onto the stack
        let mut required_clip = Region::new();
        required_clip.set_rect(canvas_bounds);
        self.canvas_data.borrow_mut().push(CanvasData {
            origin,
            required_clip,
            owned_canvas: canvas,
        });

        // subtract this region from the canvas objects already on the stack.
        // This ensures they do not draw into the space occupied by the layers
        // above them.
        let count = self.list.borrow().len();
        for i in (1..count).rev() {
            let mut local_bounds = canvas_bounds;
            let (below, clip) = {
                let mut data = self.canvas_data.borrow_mut();
                let below_origin = data[i - 1].origin;
                local_bounds.offset((origin.x - below_origin.x, origin.y - below_origin.y));

                data[i - 1]
                    .required_clip
                    .op_rect(local_bounds, Op::Difference);
                (
                    Rc::clone(&self.list.borrow()[i - 1]),
                    data[i - 1].required_clip.clone(),
                )
            };
            below.clip_region(&clip, None);
        }
        debug_assert_eq!(self.list.borrow().len(), self.canvas_data.borrow().len());
    }

    /// Removes (and drops) every canvas (`removeAll`).
    // Port of: src/utils/SkCanvasStack.cpp#L52-L55 (chrome/m156)
    #[doc(alias = "removeAll")]
    pub fn remove_all(&self) {
        self.n_way.remove_all(); // call the baseclass *before* we actually delete the canvases
        self.canvas_data.borrow_mut().clear();
    }
}

impl Deref for CanvasStack {
    type Target = Canvas;

    fn deref(&self) -> &Canvas {
        &self.n_way
    }
}

impl Drop for CanvasStack {
    // Port of: src/utils/SkCanvasStack.cpp#L25-L25 (chrome/m156)
    fn drop(&mut self) {
        self.remove_all();
    }
}
