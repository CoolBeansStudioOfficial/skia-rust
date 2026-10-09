// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRecords.h, src/core/SkRecords.cpp

//! `SkRecords`: the canvas calls an [`SkRecord`](crate::record::Record) can store.
//!
//! Each call is a struct (`DrawRect`, `ClipRect`, ...) and [`Command`] is the sum of them, which
//! is what the record holds (Skia keeps a type tag next to a pointer; the enum is the tag).
//!
//! skia-rust: only the calls of the features that are ported have a record type. Missing are
//! `SaveBehind`, `DrawBehind`, `DrawDrawable`,
//! `DrawPatch`, `DrawSlug`, `DrawAtlas`, `DrawVertices`, `DrawMesh`,
//! `DrawShadowRec`, `DrawAnnotation`, `DrawEdgeAAQuad` and `DrawEdgeAAImageSet`, whose types
//! (`SkDrawable`, `SkSlug`, ...) are not ported. `SkTypedMatrix` is a plain
//! [`Matrix`] (it only precomputes the matrix type for thread safety, and a `Matrix` is
//! immutable data here). `Optional<T>` is `Option<T>` and `PODArray<T>` a `Vec<T>`.

use crate::canvas::{PointMode, SaveLayerFlags, SrcRectConstraint, lattice::RectType};
use crate::clip_op::ClipOp;
use crate::color::Color;
use crate::image::Image;
use crate::image_filter::ImageFilter;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::path::Path;
use crate::picture::Picture;
use crate::point::Point;
use crate::rect::{IRect, Rect};
use crate::region::Region;
use crate::rrect::RRect;
use crate::sampling_options::{FilterMode, SamplingOptions};
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::text_blob::TextBlob;
use crate::tile_mode::TileMode;

/// Draw tags (`SkRecords::Tags`).
// Port of: src/core/SkRecords.h#L170-L179 (chrome/m156)
#[doc(alias = "SkRecords::Tags")]
pub mod tags {
    /// May draw something (usually named `DrawFoo`) (`kDraw_Tag`).
    pub const DRAW: u32 = 1;
    /// Contains an `SkImage` or `SkBitmap` (`kHasImage_Tag`).
    pub const HAS_IMAGE: u32 = 2;
    /// Contains text (`kHasText_Tag`).
    pub const HAS_TEXT: u32 = 4;
    /// May have an `SkPaint` field, at least optionally (`kHasPaint_Tag`).
    pub const HAS_PAINT: u32 = 8;
    /// Drawing operations that render multiple independent primitives. These draws are capable
    /// of blending with themselves (`kMultiDraw_Tag`).
    pub const MULTI_DRAW: u32 = 16;
    /// `kDrawWithPaint_Tag`.
    pub const DRAW_WITH_PAINT: u32 = DRAW | HAS_PAINT;
}

use tags::{DRAW_WITH_PAINT, MULTI_DRAW};

/// A clip op and whether the clip is anti-aliased (`SkRecords::ClipOpAndAA`).
// Port of: src/core/SkRecords.h#L216-L227 (chrome/m156)
#[doc(alias = "SkRecords::ClipOpAndAA")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ClipOpAndAA {
    op: ClipOp,
    aa: bool,
}

impl Default for ClipOpAndAA {
    fn default() -> Self {
        ClipOpAndAA {
            op: ClipOp::Intersect,
            aa: false,
        }
    }
}

impl ClipOpAndAA {
    /// `ClipOpAndAA(SkClipOp op, bool aa)`.
    #[must_use]
    pub fn new(op: ClipOp, aa: bool) -> ClipOpAndAA {
        ClipOpAndAA { op, aa }
    }

    /// The clip op (`op`).
    #[must_use]
    pub fn op(&self) -> ClipOp {
        self.op
    }

    /// True if the clip is anti-aliased (`aa`).
    #[must_use]
    pub fn aa(&self) -> bool {
        self.aa
    }
}

// Port of: src/core/SkRecords.h#L184-L190 (chrome/m156)
/// `SkRecords::NoOp`: does nothing.
#[derive(Clone, Debug, Default)]
pub struct NoOp;

/// `SkRecords::Restore`.
#[derive(Clone, Debug, Default)]
pub struct Restore {
    /// The total matrix after the restore.
    pub matrix: Matrix,
}

/// `SkRecords::Save`.
#[derive(Clone, Debug, Default)]
pub struct Save;

/// `SkRecords::SaveLayer`.
// Port of: src/core/SkRecords.h#L192-L199 (chrome/m156)
#[derive(Clone, Debug)]
pub struct SaveLayer {
    pub bounds: Option<Rect>,
    pub paint: Option<Paint>,
    pub backdrop: Option<ImageFilter>,
    pub save_layer_flags: SaveLayerFlags,
    pub backdrop_scale: scalar,
    pub backdrop_tile_mode: TileMode,
    pub filters: Vec<ImageFilter>,
}

impl Default for SaveLayer {
    fn default() -> Self {
        SaveLayer {
            bounds: None,
            paint: None,
            backdrop: None,
            save_layer_flags: SaveLayerFlags::empty(),
            backdrop_scale: 1.0,
            backdrop_tile_mode: TileMode::Clamp,
            filters: Vec::new(),
        }
    }
}

/// `SkRecords::SetMatrix`.
#[derive(Clone, Debug, Default)]
pub struct SetMatrix {
    pub matrix: Matrix,
}

/// `SkRecords::SetM44`.
#[derive(Clone, Debug, Default)]
pub struct SetM44 {
    pub matrix: M44,
}

/// `SkRecords::Concat`.
#[derive(Clone, Debug, Default)]
pub struct Concat {
    pub matrix: Matrix,
}

/// `SkRecords::Concat44`.
#[derive(Clone, Debug, Default)]
pub struct Concat44 {
    pub matrix: M44,
}

/// `SkRecords::Translate`.
#[derive(Clone, Debug, Default)]
pub struct Translate {
    pub dx: scalar,
    pub dy: scalar,
}

/// `SkRecords::Scale`.
#[derive(Clone, Debug, Default)]
pub struct Scale {
    pub sx: scalar,
    pub sy: scalar,
}

/// `SkRecords::ClipPath`.
#[derive(Clone, Debug, Default)]
pub struct ClipPath {
    pub path: Path,
    pub op_aa: ClipOpAndAA,
}

/// `SkRecords::ClipRRect`.
#[derive(Clone, Debug, Default)]
pub struct ClipRRect {
    pub rrect: RRect,
    pub op_aa: ClipOpAndAA,
}

/// `SkRecords::ClipRect`.
#[derive(Clone, Debug, Default)]
pub struct ClipRect {
    pub rect: Rect,
    pub op_aa: ClipOpAndAA,
}

/// `SkRecords::ClipRegion`.
#[derive(Clone, Debug)]
pub struct ClipRegion {
    pub region: Region,
    pub op: ClipOp,
}

impl Default for ClipRegion {
    fn default() -> Self {
        ClipRegion {
            region: Region::default(),
            op: ClipOp::Intersect,
        }
    }
}

/// `SkRecords::ClipShader`.
#[derive(Clone, Debug)]
pub struct ClipShader {
    pub shader: Shader,
    pub op: ClipOp,
}

/// `SkRecords::ResetClip`.
#[derive(Clone, Debug, Default)]
pub struct ResetClip;

// While not strictly required, if you have a Paint, it's fastest to put it first.

/// `SkRecords::DrawArc`.
#[derive(Clone, Debug, Default)]
pub struct DrawArc {
    pub paint: Paint,
    pub oval: Rect,
    pub start_angle: scalar,
    pub sweep_angle: scalar,
    pub use_center: bool,
}

/// `SkRecords::DrawDRRect`.
#[derive(Clone, Debug, Default)]
pub struct DrawDRRect {
    pub paint: Paint,
    pub outer: RRect,
    pub inner: RRect,
}

/// `SkRecords::DrawImage`.
// Port of: src/core/SkRecords.h#L256-L261 (chrome/m156)
#[derive(Clone, Debug)]
pub struct DrawImage {
    pub paint: Option<Paint>,
    pub image: Image,
    pub left: scalar,
    pub top: scalar,
    pub sampling: SamplingOptions,
}

/// `SkRecords::DrawImageLattice` (`xCount`, `yCount` and `flagCount` are the lengths of the
/// arrays; `flags` and `colors` are empty when `flagCount` is 0 or the lattice had none).
// Port of: src/core/SkRecords.h#L262-L274 (chrome/m156)
#[derive(Clone, Debug)]
pub struct DrawImageLattice {
    pub paint: Option<Paint>,
    pub image: Image,
    pub x_divs: Vec<i32>,
    pub y_divs: Vec<i32>,
    pub flag_count: usize,
    pub flags: Vec<RectType>,
    pub colors: Vec<Color>,
    pub src: IRect,
    pub dst: Rect,
    pub filter: FilterMode,
}

/// `SkRecords::DrawImageRect`.
// Port of: src/core/SkRecords.h#L275-L282 (chrome/m156)
#[derive(Clone, Debug)]
pub struct DrawImageRect {
    pub paint: Option<Paint>,
    pub image: Image,
    pub src: Rect,
    pub dst: Rect,
    pub sampling: SamplingOptions,
    pub constraint: SrcRectConstraint,
}

/// `SkRecords::DrawOval`.
#[derive(Clone, Debug, Default)]
pub struct DrawOval {
    pub paint: Paint,
    pub oval: Rect,
}

/// `SkRecords::DrawPaint`.
#[derive(Clone, Debug, Default)]
pub struct DrawPaint {
    pub paint: Paint,
}

/// `SkRecords::DrawPath`.
#[derive(Clone, Debug, Default)]
pub struct DrawPath {
    pub paint: Paint,
    pub path: Path,
}

/// `SkRecords::DrawPicture`.
#[derive(Clone, Debug)]
pub struct DrawPicture {
    pub paint: Option<Paint>,
    pub picture: Picture,
    pub matrix: Matrix,
}

/// `SkRecords::DrawPoints` (`count` is `pts.len()`).
#[derive(Clone, Debug)]
pub struct DrawPoints {
    pub paint: Paint,
    pub mode: PointMode,
    pub pts: Vec<Point>,
}

/// `SkRecords::DrawTextBlob`: the blob is kept by reference, as C++ keeps its `sk_sp`.
#[derive(Clone, Debug)]
pub struct DrawTextBlob {
    pub paint: Paint,
    pub blob: TextBlob,
    pub x: scalar,
    pub y: scalar,
}

/// `SkRecords::DrawRRect`.
#[derive(Clone, Debug, Default)]
pub struct DrawRRect {
    pub paint: Paint,
    pub rrect: RRect,
}

/// `SkRecords::DrawRect`.
#[derive(Clone, Debug, Default)]
pub struct DrawRect {
    pub paint: Paint,
    pub rect: Rect,
}

/// `SkRecords::DrawRegion`.
#[derive(Clone, Debug, Default)]
pub struct DrawRegion {
    pub paint: Paint,
    pub region: Region,
}

/// Defines [`Type`], [`Command`] and the [`RecordKind`] impls from the list of record types
/// (`SK_RECORD_TYPES`).
macro_rules! record_types {
    ($($name:ident => $tags:expr),* $(,)?) => {
        /// The type of a record (`SkRecords::Type`), in the order of `SK_RECORD_TYPES`.
        // Port of: src/core/SkRecords.h#L75-L81 (chrome/m156)
        #[doc(alias = "SkRecords::Type")]
        #[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
        pub enum Type {
            $($name,)*
        }

        /// One recorded canvas call: the sum of the record structs.
        #[derive(Clone, Debug)]
        pub enum Command {
            $($name($name),)*
        }

        impl Command {
            /// The type of the call (`fType`).
            #[must_use]
            pub fn record_type(&self) -> Type {
                match self {
                    $(Command::$name(_) => Type::$name,)*
                }
            }

            /// The tags of the call (`kTags`).
            #[must_use]
            pub fn tags(&self) -> u32 {
                match self {
                    $(Command::$name(_) => $tags,)*
                }
            }
        }

        $(
            impl RecordKind for $name {
                const TYPE: Type = Type::$name;
                const TAGS: u32 = $tags;

                fn from_command(command: &Command) -> Option<&Self> {
                    if let Command::$name(record) = command {
                        Some(record)
                    } else {
                        None
                    }
                }

                fn from_command_mut(command: &mut Command) -> Option<&mut Self> {
                    if let Command::$name(record) = command {
                        Some(record)
                    } else {
                        None
                    }
                }

                fn into_command(self) -> Command {
                    Command::$name(self)
                }
            }
        )*
    };
}

/// A record struct: what a [`Record`](crate::record::Record) can append (the
/// `static const Type kType` and `kTags` every `SkRecords::*` struct has).
pub trait RecordKind: Sized {
    /// `kType`.
    const TYPE: Type;
    /// `kTags`.
    const TAGS: u32;

    /// The record if `command` is of this type.
    fn from_command(command: &Command) -> Option<&Self>;
    /// The record if `command` is of this type.
    fn from_command_mut(command: &mut Command) -> Option<&mut Self>;
    /// Wraps the record into a [`Command`].
    fn into_command(self) -> Command;
}

// A list of all the types of canvas calls we can record. Order doesn't technically matter here,
// but it's nice to leave NoOp at 0.
// Port of: src/core/SkRecords.h#L48-L97 (chrome/m156)
record_types! {
    NoOp => 0,
    Restore => 0,
    Save => 0,
    SaveLayer => tags::HAS_PAINT,
    SetMatrix => 0,
    SetM44 => 0,
    Translate => 0,
    Scale => 0,
    Concat => 0,
    Concat44 => 0,
    ClipPath => 0,
    ClipRRect => 0,
    ClipRect => 0,
    ClipRegion => 0,
    ClipShader => 0,
    ResetClip => 0,
    DrawArc => DRAW_WITH_PAINT,
    DrawImage => DRAW_WITH_PAINT | tags::HAS_IMAGE,
    DrawImageLattice => DRAW_WITH_PAINT | tags::HAS_IMAGE,
    DrawImageRect => DRAW_WITH_PAINT | tags::HAS_IMAGE,
    DrawDRRect => DRAW_WITH_PAINT,
    DrawOval => DRAW_WITH_PAINT,
    DrawPaint => DRAW_WITH_PAINT,
    DrawPath => DRAW_WITH_PAINT,
    DrawPicture => DRAW_WITH_PAINT,
    DrawPoints => DRAW_WITH_PAINT | MULTI_DRAW,
    DrawRRect => DRAW_WITH_PAINT,
    DrawRect => DRAW_WITH_PAINT,
    DrawRegion => DRAW_WITH_PAINT,
    DrawTextBlob => DRAW_WITH_PAINT | tags::HAS_TEXT,
}

impl Command {
    /// The paint of a draw command, whether it is always there or optional (`AsPtr` in
    /// `SkRecords::IsDraw`); `None` for the commands without one.
    pub fn paint_mut(&mut self) -> Option<&mut Paint> {
        match self {
            Command::SaveLayer(r) => r.paint.as_mut(),
            Command::DrawArc(r) => Some(&mut r.paint),
            Command::DrawImage(r) => r.paint.as_mut(),
            Command::DrawImageLattice(r) => r.paint.as_mut(),
            Command::DrawImageRect(r) => r.paint.as_mut(),
            Command::DrawDRRect(r) => Some(&mut r.paint),
            Command::DrawOval(r) => Some(&mut r.paint),
            Command::DrawPaint(r) => Some(&mut r.paint),
            Command::DrawPath(r) => Some(&mut r.paint),
            Command::DrawPicture(r) => r.paint.as_mut(),
            Command::DrawPoints(r) => Some(&mut r.paint),
            Command::DrawRRect(r) => Some(&mut r.paint),
            Command::DrawRect(r) => Some(&mut r.paint),
            Command::DrawRegion(r) => Some(&mut r.paint),
            Command::DrawTextBlob(r) => Some(&mut r.paint),
            _ => None,
        }
    }
}
