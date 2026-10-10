// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skresources/include/SkResources.h (`ResourceProvider` and its
// subclasses), modules/skresources/src/SkResources.cpp (chrome/m156)

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use skia_rust_core::data::Data;
use skia_rust_core::font_mgr::FontMgr;
use skia_rust_core::typeface::Typeface;

use crate::base64;
use crate::image_asset::{
    ExternalTrackAsset, ImageAsset, ImageDecodeStrategy, MultiFrameImageAsset,
};

/// The separator of path components on this platform (`SEPARATOR` of `SkOSPath`).
// Port of: src/utils/SkOSPath.cpp#L9-L12 (chrome/m156) (`SEPARATOR`)
#[cfg(windows)]
const SEPARATOR: char = '\\';
/// The separator of path components on this platform (`SEPARATOR` of `SkOSPath`).
// Port of: src/utils/SkOSPath.cpp#L9-L12 (chrome/m156) (`SEPARATOR`)
#[cfg(not(windows))]
const SEPARATOR: char = '/';

/// Joins `relative` to `root`, with one separator between them (`SkOSPath::Join`).
// Port of: src/utils/SkOSPath.cpp#L14-L21 (chrome/m156) (`SkOSPath::Join`)
#[doc(alias = "SkOSPath::Join")]
#[must_use]
pub fn join_path(root: &str, relative: &str) -> String {
    let mut result = String::from(root);
    if !result.ends_with(SEPARATOR) && !result.is_empty() {
        result.push(SEPARATOR);
    }
    result.push_str(relative);
    result
}

/// Provides the resources of an animation: encoded data, image assets, audio tracks and fonts
/// (`ResourceProvider`). Each method returns `None` when the resource is not available.
// Port of: modules/skresources/include/SkResources.h#L199-L255 (chrome/m156) (`class ResourceProvider`)
#[doc(alias = "skresources::ResourceProvider")]
pub trait ResourceProvider {
    /// Loads a generic resource, such as a nested animation, as data (`load`).
    // Port of: modules/skresources/include/SkResources.h#L207-L211 (chrome/m156) (`ResourceProvider::load`)
    fn load(&self, _resource_path: &str, _resource_name: &str) -> Option<Data> {
        None
    }

    /// Loads the image asset at `path` and `name`, identified by `id` (`loadImageAsset`).
    // Port of: modules/skresources/include/SkResources.h#L213-L217 (chrome/m156) (`ResourceProvider::loadImageAsset`)
    #[doc(alias = "loadImageAsset")]
    fn load_image_asset(
        &self,
        _resource_path: &str,
        _resource_name: &str,
        _resource_id: &str,
    ) -> Option<Rc<dyn ImageAsset>> {
        None
    }

    /// Loads the audio track at `path` and `name`, identified by `id` (`loadAudioAsset`).
    // Port of: modules/skresources/include/SkResources.h#L219-L224 (chrome/m156) (`ResourceProvider::loadAudioAsset`)
    #[doc(alias = "loadAudioAsset")]
    fn load_audio_asset(
        &self,
        _resource_path: &str,
        _resource_name: &str,
        _resource_id: &str,
    ) -> Option<Rc<dyn ExternalTrackAsset>> {
        None
    }

    /// Loads a font as data (`loadFont`). Deprecated in favour of [`ResourceProvider::load_typeface`].
    // Port of: modules/skresources/include/SkResources.h#L226-L243 (chrome/m156) (`ResourceProvider::loadFont`)
    #[doc(alias = "loadFont")]
    fn load_font(&self, _name: &str, _url: &str) -> Option<Data> {
        None
    }

    /// Loads a font as a typeface, by its name and web URL (`loadTypeface`).
    // Port of: modules/skresources/include/SkResources.h#L245-L252 (chrome/m156) (`ResourceProvider::loadTypeface`)
    #[doc(alias = "loadTypeface")]
    fn load_typeface(&self, _name: &str, _url: &str) -> Option<Typeface> {
        None
    }
}

/// Loads resources from a directory on disk (`FileResourceProvider`).
// Port of: modules/skresources/include/SkResources.h#L257-L268 (chrome/m156) (`class FileResourceProvider`)
#[doc(alias = "skresources::FileResourceProvider")]
#[derive(Debug)]
pub struct FileResourceProvider {
    dir: String,
    strategy: ImageDecodeStrategy,
}

impl FileResourceProvider {
    /// A provider for the directory `base_dir`, or `None` if it is not a directory
    /// (`FileResourceProvider::Make`).
    // Port of: modules/skresources/src/SkResources.cpp#L211-L215 (chrome/m156) (`FileResourceProvider::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(base_dir: &str, strategy: ImageDecodeStrategy) -> Option<Rc<Self>> {
        Path::new(base_dir).is_dir().then(|| {
            Rc::new(Self {
                dir: base_dir.to_owned(),
                strategy,
            })
        })
    }
}

impl ResourceProvider for FileResourceProvider {
    // Port of: modules/skresources/src/SkResources.cpp#L223-L229 (chrome/m156) (`FileResourceProvider::load`)
    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        let full_dir = join_path(&self.dir, resource_path);
        let full_path = join_path(&full_dir, resource_name);
        std::fs::read(full_path)
            .ok()
            .map(|bytes| Data::new_copy(&bytes))
    }

    // Port of: modules/skresources/src/SkResources.cpp#L231-L240 (chrome/m156) (`FileResourceProvider::loadImageAsset`)
    fn load_image_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        _resource_id: &str,
    ) -> Option<Rc<dyn ImageAsset>> {
        let data = self.load(resource_path, resource_name)?;
        // Video assets are not built: Skia compiles them only under HAVE_VIDEO_DECODER.
        MultiFrameImageAsset::make(data, self.strategy).map(|asset| asset as Rc<dyn ImageAsset>)
    }
}

/// The forwarding base of the proxy providers: each request goes to the wrapped provider, if there
/// is one (`ResourceProviderProxyBase`).
// Port of: modules/skresources/include/SkResources.h#L270-L287 (chrome/m156) (`class ResourceProviderProxyBase`)
#[doc(alias = "skresources::ResourceProviderProxyBase")]
pub struct ResourceProviderProxyBase {
    proxy: Option<Rc<dyn ResourceProvider>>,
}

impl std::fmt::Debug for ResourceProviderProxyBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceProviderProxyBase")
            .field("has_proxy", &self.proxy.is_some())
            .finish()
    }
}

impl ResourceProviderProxyBase {
    // Port of: modules/skresources/src/SkResources.cpp#L242-L243 (chrome/m156) (`ResourceProviderProxyBase::ResourceProviderProxyBase`)
    fn new(proxy: Option<Rc<dyn ResourceProvider>>) -> Self {
        Self { proxy }
    }

    // Port of: modules/skresources/src/SkResources.cpp#L245-L249 (chrome/m156) (`ResourceProviderProxyBase::load`)
    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        self.proxy.as_ref()?.load(resource_path, resource_name)
    }

    // Port of: modules/skresources/src/SkResources.cpp#L251-L255 (chrome/m156) (`ResourceProviderProxyBase::loadImageAsset`)
    fn load_image_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ImageAsset>> {
        self.proxy
            .as_ref()?
            .load_image_asset(resource_path, resource_name, resource_id)
    }

    // Port of: modules/skresources/src/SkResources.cpp#L257-L261 (chrome/m156) (`ResourceProviderProxyBase::loadTypeface`)
    fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
        self.proxy.as_ref()?.load_typeface(name, url)
    }

    // Port of: modules/skresources/src/SkResources.cpp#L263-L267 (chrome/m156) (`ResourceProviderProxyBase::loadFont`)
    fn load_font(&self, name: &str, url: &str) -> Option<Data> {
        self.proxy.as_ref()?.load_font(name, url)
    }

    // Port of: modules/skresources/src/SkResources.cpp#L269-L274 (chrome/m156) (`ResourceProviderProxyBase::loadAudioAsset`)
    fn load_audio_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ExternalTrackAsset>> {
        self.proxy
            .as_ref()?
            .load_audio_asset(resource_path, resource_name, resource_id)
    }
}

/// Caches the image assets of a provider, by resource id (`CachingResourceProvider`).
// Port of: modules/skresources/include/SkResources.h#L289-L301 (chrome/m156) (`class CachingResourceProvider`)
#[doc(alias = "skresources::CachingResourceProvider")]
pub struct CachingResourceProvider {
    base: ResourceProviderProxyBase,
    /// The assets by resource id. A failed load is cached too, as `null` (`fImageCache`).
    image_cache: RefCell<HashMap<String, Option<Rc<dyn ImageAsset>>>>,
}

impl std::fmt::Debug for CachingResourceProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachingResourceProvider")
            .field("base", &self.base)
            .field("cached_images", &self.image_cache.borrow().len())
            .finish()
    }
}

impl CachingResourceProvider {
    /// A caching provider over `rp`, or `None` if there is no provider (`CachingResourceProvider::Make`).
    // Port of: modules/skresources/include/SkResources.h#L291-L295 (chrome/m156) (`CachingResourceProvider::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(rp: Option<Rc<dyn ResourceProvider>>) -> Option<Rc<Self>> {
        rp.map(|rp| {
            Rc::new(Self {
                base: ResourceProviderProxyBase::new(Some(rp)),
                image_cache: RefCell::new(HashMap::new()),
            })
        })
    }
}

impl ResourceProvider for CachingResourceProvider {
    // Port of: modules/skresources/src/SkResources.cpp#L299-L309 (chrome/m156) (`CachingResourceProvider::loadImageAsset`)
    fn load_image_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ImageAsset>> {
        if let Some(asset) = self.image_cache.borrow().get(resource_id) {
            return asset.clone();
        }
        let asset = self
            .base
            .load_image_asset(resource_path, resource_name, resource_id);
        self.image_cache
            .borrow_mut()
            .insert(resource_id.to_owned(), asset.clone());
        asset
    }

    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        self.base.load(resource_path, resource_name)
    }

    fn load_audio_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ExternalTrackAsset>> {
        self.base
            .load_audio_asset(resource_path, resource_name, resource_id)
    }

    fn load_font(&self, name: &str, url: &str) -> Option<Data> {
        self.base.load_font(name, url)
    }

    fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
        self.base.load_typeface(name, url)
    }
}

/// Decodes the base64 payload of a data URI with the given `prefix` (`decode_datauri`). The URI
/// must be `prefix` followed by `;base64,` and the encoded data; anything else is `None`.
// Port of: modules/skresources/src/SkResources.cpp#L321-L346 (chrome/m156) (`decode_datauri`)
fn decode_datauri(prefix: &str, uri: &str) -> Option<Data> {
    // We only handle B64 encoded image dataURIs: data:image/<type>;base64,<data>
    const ENCODING: &str = ";base64,";
    let rest = uri.strip_prefix(prefix)?;
    let encoding = rest.find(ENCODING)?;
    // strlen: the payload ends at the first NUL.
    let b64 = rest[encoding + ENCODING.len()..]
        .split('\0')
        .next()
        .unwrap_or_default()
        .as_bytes();
    let data_len = base64::decode(b64, None).ok()?;
    let mut bytes = vec![0_u8; data_len];
    base64::decode(b64, Some(&mut bytes)).ok()?;
    Some(Data::new_copy(&bytes))
}

/// Serves the image assets and the fonts of data URIs, and forwards everything else
/// (`DataURIResourceProviderProxy`).
// Port of: modules/skresources/include/SkResources.h#L303-L318 (chrome/m156) (`class DataURIResourceProviderProxy`)
#[doc(alias = "skresources::DataURIResourceProviderProxy")]
pub struct DataURIResourceProviderProxy {
    base: ResourceProviderProxyBase,
    strategy: ImageDecodeStrategy,
    font_mgr: Option<FontMgr>,
}

impl std::fmt::Debug for DataURIResourceProviderProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataURIResourceProviderProxy")
            .field("base", &self.base)
            .field("strategy", &self.strategy)
            .field("has_font_mgr", &self.font_mgr.is_some())
            .finish()
    }
}

impl DataURIResourceProviderProxy {
    /// A proxy over `rp`, which decodes data URIs with `strategy` and fonts with `font_mgr`
    /// (`DataURIResourceProviderProxy::Make`).
    // Port of: modules/skresources/src/SkResources.cpp#L348-L353 (chrome/m156) (`DataURIResourceProviderProxy::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        rp: Option<Rc<dyn ResourceProvider>>,
        strategy: ImageDecodeStrategy,
        font_mgr: Option<FontMgr>,
    ) -> Rc<Self> {
        Rc::new(Self {
            base: ResourceProviderProxyBase::new(rp),
            strategy,
            font_mgr,
        })
    }
}

impl ResourceProvider for DataURIResourceProviderProxy {
    // Port of: modules/skresources/src/SkResources.cpp#L380-L390 (chrome/m156) (`DataURIResourceProviderProxy::loadImageAsset`)
    fn load_image_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ImageAsset>> {
        // First try to decode the data as base64 using codecs registered with SkCodecs::Register()
        if let Some(data) = decode_datauri("data:image/", resource_name) {
            return MultiFrameImageAsset::make(data, self.strategy)
                .map(|asset| asset as Rc<dyn ImageAsset>);
        }
        // Fallback to asking the ProviderProxy to load this image for us.
        self.base
            .load_image_asset(resource_path, resource_name, resource_id)
    }

    // Port of: modules/skresources/src/SkResources.cpp#L392-L400 (chrome/m156) (`DataURIResourceProviderProxy::loadTypeface`)
    fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
        if let Some(font_mgr) = &self.font_mgr
            && let Some(data) = decode_datauri("data:font/", url)
        {
            return font_mgr.make_from_data(Some(&data), 0);
        }
        self.base.load_typeface(name, url)
    }

    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        self.base.load(resource_path, resource_name)
    }

    fn load_audio_asset(
        &self,
        resource_path: &str,
        resource_name: &str,
        resource_id: &str,
    ) -> Option<Rc<dyn ExternalTrackAsset>> {
        self.base
            .load_audio_asset(resource_path, resource_name, resource_id)
    }

    fn load_font(&self, name: &str, url: &str) -> Option<Data> {
        self.base.load_font(name, url)
    }
}
