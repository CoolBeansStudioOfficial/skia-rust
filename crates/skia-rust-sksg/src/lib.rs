//! Scene graph of skia-rust (`sksg`), ported from Skia's `modules/sksg`: the node DAG with
//! invalidation and damage tracking, geometry, paints, render nodes and their effects.
//!
//! Nodes are `Rc`s with `Cell`/`RefCell` state, not `Arc`s: the graph is mutable and
//! single-threaded, as in Skia. A node observes its descendants through `Weak` back pointers,
//! which Skia keeps as raw pointers.

pub mod draw;
pub mod effect_node;
pub mod geometry_node;
pub mod group;
pub mod invalidation_controller;
pub mod merge;
pub mod node;
pub mod paint_node;
pub mod path;
pub mod rect;
pub mod render_effect;
pub mod render_node;
pub mod transform;
pub mod util;

pub use draw::Draw;
pub use geometry_node::GeometryNode;
pub use group::Group;
pub use invalidation_controller::InvalidationController;
pub use merge::{Merge, MergeMode, MergeRec};
pub use node::{Node, NodeCore};
pub use paint_node::{Color, PaintNode};
pub use path::Path;
pub use rect::{RRect, Rect};
pub use render_effect::{BlurImageFilter, DropShadowImageFilter, ImageFilterEffect};
pub use render_node::{RenderContext, RenderNode, ScopedRenderContext};
pub use transform::{Transform, TransformEffect};
