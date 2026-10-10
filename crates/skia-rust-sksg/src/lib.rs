//! Scene graph of skia-rust (`sksg`), ported from Skia's `modules/sksg`: the node DAG with
//! invalidation and damage tracking, geometry, paints, render nodes and their effects.
//!
//! Nodes are `Rc`s with `Cell`/`RefCell` state, not `Arc`s: the graph is mutable and
//! single-threaded, as in Skia. A node observes its descendants through `Weak` back pointers,
//! which Skia keeps as raw pointers.

pub mod clip_effect;
pub mod color_filter;
pub mod draw;
pub mod effect_node;
pub mod geometry_effect;
pub mod geometry_node;
pub mod gradient;
pub mod group;
pub mod image;
pub mod invalidation_controller;
pub mod mask_effect;
pub mod merge;
pub mod node;
pub mod opacity_effect;
pub mod paint_node;
pub mod path;
pub mod plane;
pub mod rect;
pub mod render_effect;
pub mod render_node;
pub mod scene;
pub mod shader;
pub mod text;
pub mod transform;
pub mod util;

pub use clip_effect::ClipEffect;
pub use color_filter::{ExternalColorFilter, GradientColorFilter, ModeColorFilter};
pub use draw::Draw;
pub use geometry_effect::{
    DashEffect, FillTypeOverride, GeometryTransform, OffsetEffect, RoundEffect, TrimEffect,
};
pub use geometry_node::GeometryNode;
pub use gradient::{ColorStop, LinearGradient, RadialGradient};
pub use group::Group;
pub use image::Image;
pub use invalidation_controller::InvalidationController;
pub use mask_effect::{MaskEffect, MaskMode};
pub use merge::{Merge, MergeMode, MergeRec};
pub use node::{Node, NodeCore};
pub use opacity_effect::OpacityEffect;
pub use paint_node::{Color, PaintNode, ShaderPaint};
pub use path::Path;
pub use plane::Plane;
pub use rect::{RRect, Rect};
pub use render_effect::{
    BlenderEffect, BlurImageFilter, DropShadowImageFilter, ImageFilterEffect, LayerEffect,
};
pub use render_node::{RenderContext, RenderNode, ScopedRenderContext};
pub use scene::Scene;
pub use shader::{MaskShaderEffect, ShaderEffect, ShaderNode};
pub use text::Text;
pub use transform::{Transform, TransformEffect};
