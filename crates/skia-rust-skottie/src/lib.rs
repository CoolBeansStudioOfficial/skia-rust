//! Lottie player of skia-rust, ported from Skia's `modules/skottie` and `modules/jsonreader`.
//!
//! [`Animation`] and its [`Builder`] parse a Lottie file into a scene graph of
//! [`skia_rust_sksg`] nodes and the animators that drive it. [`json`] is the JSON reader
//! (Skia's `skjson` namespace), [`internal`] the animation builder (`skottie::internal`).
//!
//! Layer effects are M21 and text layers M22: the builders have their boundaries in
//! [`internal::effects`] and [`internal::layers::text_layer`].

/// Implements `Debug` for a type that has no useful fields to print.
macro_rules! opaque_debug {
    ($($ty:ident $(<$($lt:lifetime),+>)?),+ $(,)?) => {
        $(
            impl $(<$($lt),+>)? std::fmt::Debug for $ty $(<$($lt),+>)? {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(stringify!($ty)).finish_non_exhaustive()
                }
            }
        )+
    };
}

pub mod external_layer;
pub mod internal;
pub mod json;
pub mod skottie;
pub mod skottie_json;
pub mod skottie_property;
pub mod skottie_value;
pub mod slot_manager;

pub use external_layer::{ExternalLayer, PrecompInterceptor};
pub use skottie::{
    Animation, Builder, BuilderFlags, ExpressionEvaluator, ExpressionManager, ImageAsset,
    LayerInfo, Logger, LoggerLevel, MarkerObserver, RenderFlags, Stats,
};
pub use skottie_property::{
    ColorPropertyHandle, ColorPropertyValue, NodeType, OpacityPropertyHandle,
    OpacityPropertyValue, PropertyHandle, PropertyObserver, TransformPropertyHandle,
    TransformPropertyValue,
};
pub use skottie_value::{ColorValue, ScalarValue, ShapeValue, Vec2Value, VectorValue};
pub use slot_manager::{SlotID, SlotInfo, SlotManager};
