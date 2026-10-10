//! Resources of skia-rust (`skresources`): resource providers, image assets and the player of
//! animated codecs. Ported from Skia's `modules/skresources`.
//!
//! Providers and assets are `Rc`s, not `Arc`s: the caching provider and the image assets keep
//! mutable state, and the animation that uses them is single-threaded, as in the sksg scene graph.

pub mod anim_codec_player;
pub mod base64;
pub mod image_asset;
pub mod resource_provider;

pub use anim_codec_player::AnimCodecPlayer;
pub use image_asset::{
    ExternalTrackAsset, FrameData, ImageAsset, ImageDecodeStrategy, MultiFrameImageAsset, SizeFit,
};
pub use resource_provider::{
    CachingResourceProvider, DataURIResourceProviderProxy, FileResourceProvider, ResourceProvider,
    ResourceProviderProxyBase,
};
