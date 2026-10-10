// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGTypes.h

//! The value types of SVG attributes (`SkSVGTypes.h`).

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

/// `SkSVGColorType`.
#[doc(alias = "SkSVGColorType")]
pub type ColorType = Color;
/// `SkSVGIntegerType`.
#[doc(alias = "SkSVGIntegerType")]
pub type IntegerType = i32;
/// `SkSVGNumberType`.
#[doc(alias = "SkSVGNumberType")]
pub type NumberType = scalar;
/// `SkSVGStringType`.
#[doc(alias = "SkSVGStringType")]
pub type StringType = String;
/// `SkSVGViewBoxType`.
#[doc(alias = "SkSVGViewBoxType")]
pub type ViewBoxType = Rect;
/// `SkSVGTransformType`.
#[doc(alias = "SkSVGTransformType")]
pub type TransformType = Matrix;
/// `SkSVGPointsType`.
#[doc(alias = "SkSVGPointsType")]
pub type PointsType = Vec<Point>;

/// Whether a property was specified, inherits or has a value.
// Port of: modules/svg/include/SkSVGTypes.h#L33-L37 (chrome/m156)
#[doc(alias = "SkSVGPropertyState")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PropertyState {
    #[default]
    Unspecified,
    Inherit,
    Value,
}

/// An SVG property, <https://www.w3.org/TR/SVG11/intro.html#TermProperty>.
// Port of: modules/svg/include/SkSVGTypes.h#L39-L119 (chrome/m156)
#[doc(alias = "SkSVGProperty")]
#[derive(Debug, Clone)]
pub struct Property<T, const INHERITABLE: bool> {
    state: PropertyState,
    value: Option<T>,
}

impl<T, const INHERITABLE: bool> Default for Property<T, INHERITABLE> {
    fn default() -> Self {
        Self {
            state: PropertyState::Unspecified,
            value: None,
        }
    }
}

impl<T, const INHERITABLE: bool> Property<T, INHERITABLE> {
    /// A property without a value (`SkSVGProperty(SkSVGPropertyState)`).
    #[must_use]
    pub fn from_state(state: PropertyState) -> Self {
        Self { state, value: None }
    }

    /// A property with a value (`SkSVGProperty(const T&)`).
    #[must_use]
    pub fn from_value(value: T) -> Self {
        Self {
            state: PropertyState::Value,
            value: Some(value),
        }
    }

    pub fn init(&mut self, value: T) {
        self.state = PropertyState::Value;
        self.value = Some(value);
    }

    #[doc(alias = "isInheritable")]
    #[must_use]
    pub const fn is_inheritable(&self) -> bool {
        INHERITABLE
    }

    #[doc(alias = "isValue")]
    #[must_use]
    pub fn is_value(&self) -> bool {
        self.state == PropertyState::Value
    }

    #[doc(alias = "getMaybeNull")]
    #[must_use]
    pub fn get_maybe_null(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// `set(SkSVGPropertyState)`.
    pub fn set_state(&mut self, state: PropertyState) {
        self.state = state;
        if self.state != PropertyState::Value {
            self.value = None;
        }
    }

    /// `set(const T&)`.
    pub fn set(&mut self, value: T) {
        self.state = PropertyState::Value;
        self.value = Some(value);
    }
}

impl<T, const INHERITABLE: bool> Deref for Property<T, INHERITABLE> {
    type Target = T;

    fn deref(&self) -> &T {
        debug_assert_eq!(self.state, PropertyState::Value);
        self.value.as_ref().expect("a property with a value")
    }
}

impl<T, const INHERITABLE: bool> DerefMut for Property<T, INHERITABLE> {
    fn deref_mut(&mut self) -> &mut T {
        debug_assert_eq!(self.state, PropertyState::Value);
        self.value.as_mut().expect("a property with a value")
    }
}

/// The unit of a [`Length`].
// Port of: modules/svg/include/SkSVGTypes.h#L121-L135 (chrome/m156)
#[doc(alias = "SkSVGLength::Unit")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Unknown,
    Number,
    Percentage,
    EMS,
    EXS,
    PX,
    CM,
    MM,
    IN,
    PT,
    PC,
}

/// `SkSVGLength`.
// Port of: modules/svg/include/SkSVGTypes.h#L120-L158 (chrome/m156)
#[doc(alias = "SkSVGLength")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    value: scalar,
    unit: LengthUnit,
}

impl Default for Length {
    fn default() -> Self {
        Self {
            value: 0.0,
            unit: LengthUnit::Unknown,
        }
    }
}

impl Length {
    /// `SkSVGLength(v, Unit::kNumber)`.
    #[must_use]
    pub const fn new(value: scalar) -> Self {
        Self {
            value,
            unit: LengthUnit::Number,
        }
    }

    /// `SkSVGLength(v, u)`.
    #[must_use]
    pub const fn with_unit(value: scalar, unit: LengthUnit) -> Self {
        Self { value, unit }
    }

    #[must_use]
    pub const fn value(&self) -> scalar {
        self.value
    }

    #[must_use]
    pub const fn unit(&self) -> LengthUnit {
        self.unit
    }
}

/// The kind of an [`IRI`].
// Port of: modules/svg/include/SkSVGTypes.h#L162-L166 (chrome/m156)
#[doc(alias = "SkSVGIRI::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IriType {
    Local,
    Nonlocal,
    DataURI,
}

/// <https://www.w3.org/TR/SVG11/linking.html#IRIReference>
// Port of: modules/svg/include/SkSVGTypes.h#L160-L185 (chrome/m156)
#[doc(alias = "SkSVGIRI")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iri {
    ty: IriType,
    iri: StringType,
}

impl Default for Iri {
    fn default() -> Self {
        Self {
            ty: IriType::Local,
            iri: StringType::new(),
        }
    }
}

impl Iri {
    #[must_use]
    pub fn new(ty: IriType, iri: impl Into<StringType>) -> Self {
        Self {
            ty,
            iri: iri.into(),
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> IriType {
        self.ty
    }

    #[must_use]
    pub fn iri(&self) -> &str {
        &self.iri
    }
}

/// The kind of a [`Color`].
// Port of: modules/svg/include/SkSVGTypes.h#L189-L193 (chrome/m156)
#[doc(alias = "SkSVGColor::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKind {
    CurrentColor,
    Color,
    ICCColor,
}

/// <https://www.w3.org/TR/SVG11/types.html#InterfaceSVGColor>
// Port of: modules/svg/include/SkSVGTypes.h#L187-L242 (chrome/m156)
#[doc(alias = "SkSVGColor")]
#[derive(Debug, Clone)]
pub struct Fill {
    ty: ColorKind,
    color: ColorType,
    // `sk_sp<RefCntVars>`: shared, and compared by identity.
    vars: Option<Arc<Vec<String>>>,
}

impl Default for Fill {
    fn default() -> Self {
        Self::new(Color::BLACK)
    }
}

impl PartialEq for Fill {
    // Port of: modules/svg/include/SkSVGTypes.h#L216-L219 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.color == other.color
            && match (&self.vars, &other.vars) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl Fill {
    /// `SkSVGColor(const SkSVGColorType&)`.
    #[must_use]
    pub fn new(color: ColorType) -> Self {
        Self {
            ty: ColorKind::Color,
            color,
            vars: None,
        }
    }

    /// `SkSVGColor(Type, Vars&&)`.
    #[must_use]
    pub fn with_type_and_vars(ty: ColorKind, vars: Vec<String>) -> Self {
        Self {
            ty,
            color: Color::BLACK,
            vars: if vars.is_empty() {
                None
            } else {
                Some(Arc::new(vars))
            },
        }
    }

    /// `SkSVGColor(const SkSVGColorType&, Vars&&)`.
    #[must_use]
    pub fn with_color_and_vars(color: ColorType, vars: Vec<String>) -> Self {
        Self {
            ty: ColorKind::Color,
            color,
            vars: if vars.is_empty() {
                None
            } else {
                Some(Arc::new(vars))
            },
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> ColorKind {
        self.ty
    }

    #[must_use]
    pub fn color(&self) -> ColorType {
        debug_assert_eq!(self.ty, ColorKind::Color);
        self.color
    }

    #[must_use]
    pub fn vars(&self) -> &[String] {
        self.vars.as_ref().map_or(&[], |v| v.as_slice())
    }
}

/// The kind of a [`Paint`].
// Port of: modules/svg/include/SkSVGTypes.h#L246-L250 (chrome/m156)
#[doc(alias = "SkSVGPaint::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintType {
    None,
    Color,
    IRI,
}

/// `SkSVGPaint`.
// Port of: modules/svg/include/SkSVGTypes.h#L244-L286 (chrome/m156)
#[doc(alias = "SkSVGPaint")]
#[derive(Debug, Clone, PartialEq)]
pub struct Paint {
    ty: PaintType,
    // Logical union.
    color: Fill,
    iri: Iri,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            ty: PaintType::None,
            color: Fill::new(Color::BLACK),
            iri: Iri::default(),
        }
    }
}

impl Paint {
    /// `SkSVGPaint(Type)`.
    #[must_use]
    pub fn with_type(ty: PaintType) -> Self {
        Self {
            ty,
            color: Fill::new(Color::BLACK),
            iri: Iri::default(),
        }
    }

    /// `SkSVGPaint(SkSVGColor)`.
    #[must_use]
    pub fn from_color(color: Fill) -> Self {
        Self {
            ty: PaintType::Color,
            color,
            iri: Iri::default(),
        }
    }

    /// `SkSVGPaint(const SkSVGIRI&, SkSVGColor fallback_color)`.
    #[must_use]
    pub fn from_iri(iri: Iri, fallback_color: Fill) -> Self {
        Self {
            ty: PaintType::IRI,
            color: fallback_color,
            iri,
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> PaintType {
        self.ty
    }

    #[must_use]
    pub fn color(&self) -> &Fill {
        debug_assert!(self.ty == PaintType::Color || self.ty == PaintType::IRI);
        &self.color
    }

    #[must_use]
    pub fn iri(&self) -> &Iri {
        debug_assert_eq!(self.ty, PaintType::IRI);
        &self.iri
    }
}

/// The kind of a [`FuncIri`].
// Port of: modules/svg/include/SkSVGTypes.h#L291-L294 (chrome/m156)
#[doc(alias = "SkSVGFuncIRI::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncIriType {
    None,
    IRI,
}

/// `<funciri> | none` (used for clip/mask/filter properties).
// Port of: modules/svg/include/SkSVGTypes.h#L289-L312 (chrome/m156)
#[doc(alias = "SkSVGFuncIRI")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncIri {
    ty: FuncIriType,
    iri: Iri,
}

impl Default for FuncIri {
    fn default() -> Self {
        Self {
            ty: FuncIriType::None,
            iri: Iri::default(),
        }
    }
}

impl FuncIri {
    /// `SkSVGFuncIRI(Type)`.
    #[must_use]
    pub fn with_type(ty: FuncIriType) -> Self {
        Self {
            ty,
            iri: Iri::default(),
        }
    }

    /// `SkSVGFuncIRI(SkSVGIRI&&)`.
    #[must_use]
    pub fn from_iri(iri: Iri) -> Self {
        Self {
            ty: FuncIriType::IRI,
            iri,
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> FuncIriType {
        self.ty
    }

    #[must_use]
    pub fn iri(&self) -> &Iri {
        debug_assert_eq!(self.ty, FuncIriType::IRI);
        &self.iri
    }
}

/// `SkSVGLineCap`.
// Port of: modules/svg/include/SkSVGTypes.h#L314-L318 (chrome/m156)
#[doc(alias = "SkSVGLineCap")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// `SkSVGLineJoin`.
// Port of: modules/svg/include/SkSVGTypes.h#L320-L345 (chrome/m156)
#[doc(alias = "SkSVGLineJoin")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineJoin {
    ty: LineJoinType,
}

/// `SkSVGLineJoin::Type`.
#[doc(alias = "SkSVGLineJoin::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineJoinType {
    Miter,
    Round,
    Bevel,
    Inherit,
}

impl Default for LineJoin {
    fn default() -> Self {
        Self {
            ty: LineJoinType::Inherit,
        }
    }
}

impl LineJoin {
    #[must_use]
    pub const fn new(ty: LineJoinType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> LineJoinType {
        self.ty
    }
}

/// `SkSVGSpreadMethod::Type`. These values match Skia's `SkTileMode`.
// Port of: modules/svg/include/SkSVGTypes.h#L347-L372 (chrome/m156)
#[doc(alias = "SkSVGSpreadMethod::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMethodType {
    Pad,
    Repeat,
    Reflect,
}

/// `SkSVGSpreadMethod`.
#[doc(alias = "SkSVGSpreadMethod")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpreadMethod {
    ty: SpreadMethodType,
}

impl Default for SpreadMethod {
    fn default() -> Self {
        Self {
            ty: SpreadMethodType::Pad,
        }
    }
}

impl SpreadMethod {
    #[must_use]
    pub const fn new(ty: SpreadMethodType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> SpreadMethodType {
        self.ty
    }
}

/// `SkSVGFillRule::Type`.
#[doc(alias = "SkSVGFillRule::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRuleType {
    NonZero,
    EvenOdd,
    Inherit,
}

/// `SkSVGFillRule`.
// Port of: modules/svg/include/SkSVGTypes.h#L374-L408 (chrome/m156)
#[doc(alias = "SkSVGFillRule")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillRule {
    ty: FillRuleType,
}

impl Default for FillRule {
    fn default() -> Self {
        Self {
            ty: FillRuleType::Inherit,
        }
    }
}

impl FillRule {
    #[must_use]
    pub const fn new(ty: FillRuleType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> FillRuleType {
        self.ty
    }

    #[doc(alias = "asFillType")]
    #[must_use]
    pub fn as_fill_type(&self) -> PathFillType {
        // should never be called for unresolved values.
        debug_assert_ne!(self.ty, FillRuleType::Inherit);
        if self.ty == FillRuleType::EvenOdd {
            PathFillType::EvenOdd
        } else {
            PathFillType::Winding
        }
    }
}

/// `SkSVGVisibility::Type`.
#[doc(alias = "SkSVGVisibility::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityType {
    Visible,
    Hidden,
    Collapse,
    Inherit,
}

/// `SkSVGVisibility`.
// Port of: modules/svg/include/SkSVGTypes.h#L410-L437 (chrome/m156)
#[doc(alias = "SkSVGVisibility")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    ty: VisibilityType,
}

impl Default for Visibility {
    fn default() -> Self {
        Self {
            ty: VisibilityType::Visible,
        }
    }
}

impl Visibility {
    #[must_use]
    pub const fn new(ty: VisibilityType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> VisibilityType {
        self.ty
    }
}

/// `SkSVGDashArray::Type`.
#[doc(alias = "SkSVGDashArray::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashArrayType {
    None,
    DashArray,
    Inherit,
}

/// `SkSVGDashArray`.
// Port of: modules/svg/include/SkSVGTypes.h#L439-L470 (chrome/m156)
#[doc(alias = "SkSVGDashArray")]
#[derive(Debug, Clone, PartialEq)]
pub struct DashArray {
    ty: DashArrayType,
    dash_array: Vec<Length>,
}

impl Default for DashArray {
    fn default() -> Self {
        Self {
            ty: DashArrayType::None,
            dash_array: Vec::new(),
        }
    }
}

impl DashArray {
    /// `SkSVGDashArray(Type)`.
    #[must_use]
    pub fn with_type(ty: DashArrayType) -> Self {
        Self {
            ty,
            dash_array: Vec::new(),
        }
    }

    /// `SkSVGDashArray(std::vector<SkSVGLength>&&)`.
    #[must_use]
    pub fn from_dashes(dash_array: Vec<Length>) -> Self {
        Self {
            ty: DashArrayType::DashArray,
            dash_array,
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> DashArrayType {
        self.ty
    }

    #[doc(alias = "dashArray")]
    #[must_use]
    pub fn dash_array(&self) -> &[Length] {
        &self.dash_array
    }
}

/// `SkSVGStopColor::Type`.
#[doc(alias = "SkSVGStopColor::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopColorType {
    Color,
    CurrentColor,
    ICCColor,
    Inherit,
}

/// `SkSVGStopColor`.
// Port of: modules/svg/include/SkSVGTypes.h#L472-L498 (chrome/m156)
#[doc(alias = "SkSVGStopColor")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopColor {
    ty: StopColorType,
    color: ColorType,
}

impl Default for StopColor {
    fn default() -> Self {
        Self {
            ty: StopColorType::Color,
            color: Color::BLACK,
        }
    }
}

impl StopColor {
    #[must_use]
    pub fn with_type(ty: StopColorType) -> Self {
        Self {
            ty,
            color: Color::BLACK,
        }
    }

    #[must_use]
    pub fn from_color(color: ColorType) -> Self {
        Self {
            ty: StopColorType::Color,
            color,
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> StopColorType {
        self.ty
    }

    #[must_use]
    pub fn color(&self) -> ColorType {
        debug_assert_eq!(self.ty, StopColorType::Color);
        self.color
    }
}

/// `SkSVGObjectBoundingBoxUnits::Type`.
#[doc(alias = "SkSVGObjectBoundingBoxUnits::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectBoundingBoxUnitsType {
    UserSpaceOnUse,
    ObjectBoundingBox,
}

/// `SkSVGObjectBoundingBoxUnits`.
// Port of: modules/svg/include/SkSVGTypes.h#L500-L525 (chrome/m156)
#[doc(alias = "SkSVGObjectBoundingBoxUnits")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectBoundingBoxUnits {
    ty: ObjectBoundingBoxUnitsType,
}

impl Default for ObjectBoundingBoxUnits {
    fn default() -> Self {
        Self {
            ty: ObjectBoundingBoxUnitsType::UserSpaceOnUse,
        }
    }
}

impl ObjectBoundingBoxUnits {
    #[must_use]
    pub const fn new(ty: ObjectBoundingBoxUnitsType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> ObjectBoundingBoxUnitsType {
        self.ty
    }
}

/// `SkSVGFontFamily::Type`.
#[doc(alias = "SkSVGFontFamily::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamilyType {
    Family,
    Inherit,
}

/// `SkSVGFontFamily`.
// Port of: modules/svg/include/SkSVGTypes.h#L527-L553 (chrome/m156)
#[doc(alias = "SkSVGFontFamily")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFamily {
    ty: FontFamilyType,
    family: String,
}

impl Default for FontFamily {
    fn default() -> Self {
        Self {
            ty: FontFamilyType::Inherit,
            family: String::new(),
        }
    }
}

impl FontFamily {
    #[must_use]
    pub fn new(family: &str) -> Self {
        Self {
            ty: FontFamilyType::Family,
            family: family.to_owned(),
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> FontFamilyType {
        self.ty
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }
}

/// `SkSVGFontStyle::Type`.
#[doc(alias = "SkSVGFontStyle::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyleType {
    Normal,
    Italic,
    Oblique,
    Inherit,
}

/// `SkSVGFontStyle`.
// Port of: modules/svg/include/SkSVGTypes.h#L555-L578 (chrome/m156)
#[doc(alias = "SkSVGFontStyle")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontStyle {
    ty: FontStyleType,
}

impl Default for FontStyle {
    fn default() -> Self {
        Self {
            ty: FontStyleType::Inherit,
        }
    }
}

impl FontStyle {
    #[must_use]
    pub const fn new(ty: FontStyleType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> FontStyleType {
        self.ty
    }
}

/// `SkSVGFontSize::Type`.
#[doc(alias = "SkSVGFontSize::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSizeType {
    Length,
    Inherit,
}

/// `SkSVGFontSize`.
// Port of: modules/svg/include/SkSVGTypes.h#L580-L607 (chrome/m156)
#[doc(alias = "SkSVGFontSize")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontSize {
    ty: FontSizeType,
    size: Length,
}

impl Default for FontSize {
    fn default() -> Self {
        Self {
            ty: FontSizeType::Inherit,
            size: Length::new(0.0),
        }
    }
}

impl FontSize {
    #[must_use]
    pub const fn new(size: Length) -> Self {
        Self {
            ty: FontSizeType::Length,
            size,
        }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> FontSizeType {
        self.ty
    }

    #[must_use]
    pub const fn size(&self) -> &Length {
        &self.size
    }
}

/// `SkSVGFontWeight::Type`.
#[doc(alias = "SkSVGFontWeight::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeightType {
    W100,
    W200,
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
    Normal,
    Bold,
    Bolder,
    Lighter,
    Inherit,
}

/// `SkSVGFontWeight`.
// Port of: modules/svg/include/SkSVGTypes.h#L609-L643 (chrome/m156)
#[doc(alias = "SkSVGFontWeight")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontWeight {
    ty: FontWeightType,
}

impl Default for FontWeight {
    fn default() -> Self {
        Self {
            ty: FontWeightType::Inherit,
        }
    }
}

impl FontWeight {
    #[must_use]
    pub const fn new(ty: FontWeightType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> FontWeightType {
        self.ty
    }
}

/// `SkSVGPreserveAspectRatio::Align`. Bits [0,1] encode the X alignment, bits [2,3] the Y
/// alignment.
// Port of: modules/svg/include/SkSVGTypes.h#L645-L665 (chrome/m156)
#[doc(alias = "SkSVGPreserveAspectRatio::Align")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Align {
    XMinYMin = 0x00,
    XMidYMin = 0x01,
    XMaxYMin = 0x02,
    XMinYMid = 0x04,
    XMidYMid = 0x05,
    XMaxYMid = 0x06,
    XMinYMax = 0x08,
    XMidYMax = 0x09,
    XMaxYMax = 0x0a,
    None = 0x10,
}

/// `SkSVGPreserveAspectRatio::Scale`.
#[doc(alias = "SkSVGPreserveAspectRatio::Scale")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Meet,
    Slice,
}

/// `SkSVGPreserveAspectRatio`.
// Port of: modules/svg/include/SkSVGTypes.h#L645-L672 (chrome/m156)
#[doc(alias = "SkSVGPreserveAspectRatio")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreserveAspectRatio {
    pub align: Align,
    pub scale: Scale,
}

impl Default for PreserveAspectRatio {
    fn default() -> Self {
        Self {
            align: Align::XMidYMid,
            scale: Scale::Meet,
        }
    }
}

/// `SkSVGTextAnchor::Type`.
#[doc(alias = "SkSVGTextAnchor::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchorType {
    Start,
    Middle,
    End,
    Inherit,
}

/// `SkSVGTextAnchor`.
// Port of: modules/svg/include/SkSVGTypes.h#L674-L697 (chrome/m156)
#[doc(alias = "SkSVGTextAnchor")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextAnchor {
    ty: TextAnchorType,
}

impl Default for TextAnchor {
    fn default() -> Self {
        Self {
            ty: TextAnchorType::Inherit,
        }
    }
}

impl TextAnchor {
    #[must_use]
    pub const fn new(ty: TextAnchorType) -> Self {
        Self { ty }
    }

    #[doc(alias = "type")]
    #[must_use]
    pub const fn ty(&self) -> TextAnchorType {
        self.ty
    }
}

/// `SkSVGFeInputType::Type`.
#[doc(alias = "SkSVGFeInputType::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeInputTypeKind {
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterPrimitiveReference,
    Unspecified,
}

/// `SkSVGFeInputType`: <https://www.w3.org/TR/SVG11/filters.html#FilterPrimitiveInAttribute>
// Port of: modules/svg/include/SkSVGTypes.h#L699-L732 (chrome/m156)
#[doc(alias = "SkSVGFeInputType")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeInputType {
    ty: FeInputTypeKind,
    id: String,
}

impl Default for FeInputType {
    fn default() -> Self {
        Self {
            ty: FeInputTypeKind::Unspecified,
            id: String::new(),
        }
    }
}

impl FeInputType {
    #[must_use]
    pub fn with_type(ty: FeInputTypeKind) -> Self {
        Self {
            ty,
            id: String::new(),
        }
    }

    #[must_use]
    pub fn with_id(id: &str) -> Self {
        Self {
            ty: FeInputTypeKind::FilterPrimitiveReference,
            id: id.to_owned(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        debug_assert_eq!(self.ty, FeInputTypeKind::FilterPrimitiveReference);
        &self.id
    }

    #[doc(alias = "type")]
    #[must_use]
    pub fn ty(&self) -> FeInputTypeKind {
        self.ty
    }
}

/// `SkSVGFeColorMatrixType`.
#[doc(alias = "SkSVGFeColorMatrixType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeColorMatrixType {
    Matrix,
    Saturate,
    HueRotate,
    LuminanceToAlpha,
}

/// `SkSVGFeColorMatrixValues`.
#[doc(alias = "SkSVGFeColorMatrixValues")]
pub type FeColorMatrixValues = Vec<NumberType>;

/// `SkSVGFeCompositeOperator`.
#[doc(alias = "SkSVGFeCompositeOperator")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeCompositeOperator {
    Over,
    In,
    Out,
    Atop,
    Xor,
    Arithmetic,
}

/// `SkSVGFeTurbulenceBaseFrequency`.
// Port of: modules/svg/include/SkSVGTypes.h#L751-L765 (chrome/m156)
#[doc(alias = "SkSVGFeTurbulenceBaseFrequency")]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FeTurbulenceBaseFrequency {
    freq_x: NumberType,
    freq_y: NumberType,
}

impl FeTurbulenceBaseFrequency {
    #[must_use]
    pub const fn new(freq_x: NumberType, freq_y: NumberType) -> Self {
        Self { freq_x, freq_y }
    }

    #[doc(alias = "freqX")]
    #[must_use]
    pub const fn freq_x(&self) -> NumberType {
        self.freq_x
    }

    #[doc(alias = "freqY")]
    #[must_use]
    pub const fn freq_y(&self) -> NumberType {
        self.freq_y
    }
}

/// `SkSVGFeTurbulenceType::Type`.
#[doc(alias = "SkSVGFeTurbulenceType::Type")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeTurbulenceKind {
    FractalNoise,
    Turbulence,
}

/// `SkSVGFeTurbulenceType`.
#[doc(alias = "SkSVGFeTurbulenceType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeTurbulenceType {
    pub ty: FeTurbulenceKind,
}

impl Default for FeTurbulenceType {
    fn default() -> Self {
        Self {
            ty: FeTurbulenceKind::Turbulence,
        }
    }
}

/// `SkSVGXmlSpace`.
#[doc(alias = "SkSVGXmlSpace")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XmlSpace {
    Default,
    Preserve,
}

/// `SkSVGColorspace`.
#[doc(alias = "SkSVGColorspace")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colorspace {
    Auto,
    SRGB,
    LinearRGB,
}

/// `SkSVGDisplay`: <https://www.w3.org/TR/SVG11/painting.html#DisplayProperty>
#[doc(alias = "SkSVGDisplay")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    None,
}

/// `SkSVGFeFuncType`: <https://www.w3.org/TR/SVG11/filters.html#TransferFunctionElementAttributes>
#[doc(alias = "SkSVGFeFuncType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeFuncType {
    Identity,
    Table,
    Discrete,
    Linear,
    Gamma,
}
