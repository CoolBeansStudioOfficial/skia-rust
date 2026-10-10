// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The SVG module of skia-rust: a port of Skia's `modules/svg` (the SVG DOM and its renderer).
//!
//! This is the DOM core (`docs/design/modules.md` M13): the document, the node, container and
//! transformable nodes, `svg`, `g`, `a`, `defs`, `use`, the basic shapes, the attribute parser,
//! presentation attributes and the `style` attribute, and the render context. Paint servers, clips,
//! masks, filters, images and text arrive with M14 and M15.

pub mod attribute;
pub mod attribute_parser;
pub mod circle;
pub mod container;
pub mod dom;
pub mod ellipse;
pub mod id_mapper;
pub mod line;
pub mod node;
pub mod path;
pub mod poly;
pub mod rect;
pub mod render_context;
pub mod shape;
pub mod svg;
pub mod transformable_node;
pub mod types;
pub mod use_;
pub mod value;

pub use attribute::{Attribute, PresentationAttributes};
pub use circle::Circle;
pub use container::{Container, Defs, G, HiddenContainer};
pub use dom::{Dom, LoadError};
pub use ellipse::Ellipse;
pub use id_mapper::IdMapper;
pub use line::Line;
pub use node::{Node, NodeBase, PresentationAttributesExt, SvgNode, Tag};
pub use path::Path;
pub use poly::Poly;
pub use rect::Rect;
pub use render_context::{
    BboxContext, BorrowedNode, LengthContext, LengthType, ObbScope, ObbTransform,
    PresentationContext, RenderContext,
};
pub use svg::{Svg, SvgType};
pub use transformable_node::TransformableNode;
pub use types::*;
pub use use_::Use;
pub use value::Value;
