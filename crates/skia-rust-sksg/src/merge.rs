// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGMerge.h, modules/sksg/src/SkSGMerge.cpp (chrome/m156)

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::op_builder::OpBuilder;
use skia_rust_pathops::path_op::PathOp;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};

/// How a shape is merged with the shapes before it (`Merge::Mode`).
// Port of: modules/sksg/include/SkSGMerge.h#L17-L28 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MergeMode {
    /// Append path mode.
    #[default]
    Merge,
    /// `SkPathOp` `kUnion_SkPathOp`.
    Union,
    /// `SkPathOp` `kIntersect_SkPathOp`.
    Intersect,
    /// `SkPathOp` `kDifference_SkPathOp`.
    Difference,
    /// `SkPathOp` `kReverseDifference_SkPathOp`.
    ReverseDifference,
    /// `SkPathOp` `kXOR_SkPathOp`.
    Xor,
}

/// One input of a merge (`Merge::Rec`).
// Port of: modules/sksg/include/SkSGMerge.h#L30-L33 (chrome/m156)
#[derive(Debug, Clone)]
pub struct MergeRec {
    pub geo: Rc<dyn GeometryNode>,
    pub mode: MergeMode,
}

/// Combines the shapes of several geometries into one (`Merge`).
// Port of: modules/sksg/include/SkSGMerge.h#L35-L66 (chrome/m156) (`class Merge`)
#[doc(alias = "sksg::Merge")]
#[derive(Debug)]
pub struct Merge {
    core: NodeCore,
    recs: Vec<MergeRec>,
    merged: RefCell<Path>,
}

impl Merge {
    /// The merge of `recs` (`Merge::Make`).
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(recs: &[MergeRec]) -> Rc<Self> {
        let merge = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            recs: recs.to_vec(),
            merged: RefCell::new(Path::default()),
        });
        // Port of: modules/sksg/src/SkSGMerge.cpp#L11-L15 (chrome/m156) (`Merge::Merge`)
        for rec in recs {
            merge.observe_inval(rec.geo.as_ref());
        }
        merge
    }
}

impl Drop for Merge {
    // Port of: modules/sksg/src/SkSGMerge.cpp#L17-L21 (chrome/m156) (`Merge::~Merge`)
    fn drop(&mut self) {
        for rec in &self.recs {
            self.unobserve_inval(rec.geo.as_ref());
        }
    }
}

/// The path-op of a merge mode (`mode_to_op`).
// Port of: modules/sksg/src/SkSGMerge.cpp#L43-L58 (chrome/m156)
fn mode_to_op(mode: MergeMode) -> PathOp {
    match mode {
        MergeMode::Union | MergeMode::Merge => PathOp::Union,
        MergeMode::Intersect => PathOp::Intersect,
        MergeMode::Difference => PathOp::Difference,
        MergeMode::ReverseDifference => PathOp::ReverseDifference,
        MergeMode::Xor => PathOp::Xor,
        // Skia falls back to kUnion for the append mode, which never reaches this function.
    }
}

/// A `SkPathBuilder` holding the contours and fill type of `path`.
// skia-rust: SkPathBuilder(const SkPath&) has no counterpart in skia-rust-core; this copies the
// fill type and appends the verbs.
fn builder_from_path(path: &Path) -> PathBuilder {
    let mut builder = PathBuilder::new();
    builder.set_fill_type(path.fill_type());
    builder.add_path(path, None);
    builder
}

/// Appends `path` to the merger, after resolving any pending path-op builder into it.
// Port of: modules/sksg/src/SkSGMerge.cpp#L63-L77 (chrome/m156) (`append` in `Merge::onRevalidate`)
fn append(builder: &mut OpBuilder, merger: &mut PathBuilder, in_builder: &mut bool, path: &Path) {
    if *in_builder {
        if let Some(result) = builder.resolve() {
            *merger = builder_from_path(&result);
        }
        *in_builder = false;
    }
    if merger.is_empty() {
        // First merge path determines the fill type.
        *merger = builder_from_path(path);
    } else {
        merger.add_path(path, None);
    }
}

impl Node for Merge {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGMerge.cpp#L79-L110 (chrome/m156) (`Merge::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let mut builder = OpBuilder::new();
        let mut merger = PathBuilder::new();
        let mut in_builder = false;
        for rec in &self.recs {
            rec.geo.revalidate(ic.as_deref_mut(), ctm);
            if rec.mode == MergeMode::Merge {
                // Merge (append) is not supported by SkOpBuilder.
                append(
                    &mut builder,
                    &mut merger,
                    &mut in_builder,
                    &rec.geo.as_path(),
                );
                continue;
            }
            if !in_builder {
                builder.add(&merger.detach(), PathOp::Union);
                in_builder = true;
            }
            builder.add(&rec.geo.as_path(), mode_to_op(rec.mode));
        }
        let new_path = if in_builder {
            builder.resolve().unwrap_or_default()
        } else {
            merger.detach()
        };
        let bounds = new_path.compute_tight_bounds();
        *self.merged.borrow_mut() = new_path;
        bounds
    }
}

impl GeometryNode for Merge {
    // Port of: modules/sksg/src/SkSGMerge.cpp#L25-L27 (chrome/m156) (`Merge::onClip`)
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        canvas.clip_path(&self.merged.borrow(), ClipOp::Intersect, anti_alias);
    }

    // Port of: modules/sksg/src/SkSGMerge.cpp#L29-L31 (chrome/m156) (`Merge::onDraw`)
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        canvas.draw_path(&self.merged.borrow(), paint);
    }

    // Port of: modules/sksg/src/SkSGMerge.cpp#L33-L35 (chrome/m156) (`Merge::onContains`)
    fn on_contains(&self, p: Point) -> bool {
        self.merged.borrow().contains(p)
    }

    // Port of: modules/sksg/src/SkSGMerge.cpp#L37-L39 (chrome/m156) (`Merge::onAsPath`)
    fn on_as_path(&self) -> Path {
        self.merged.borrow().clone()
    }
}
