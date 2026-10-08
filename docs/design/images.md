# Design: raster images, the image shader and image drawing

## As implemented

Scope: `SkImage`/`SkImage_Base`/`SkImage_Raster`, `SkImages::Raster*`, `SkBitmap::asImage` and
`makeShader`, `SkSurface::makeImageSnapshot`, `SkSamplingOptions`, `SkMipmap`/`SkMipmapAccessor`,
`SkImageShader`, `SkLocalMatrixShader`, `drawImage`/`drawImageRect`/nine/lattice, and the
`SkBitmapDevice`/`SkDraw` paths below them (drawBitmap, the sprite blitter), plus the raster
pipeline sampling stages. Everything else about images (GPU, lazy and encoded images, image
filters, text) is out of scope.

### Handles and sharing

`sk_sp<SkImage>` is `Image`, an `Arc<dyn ImageBase>`. Images are immutable, so the handle is
shared and `ptr_eq` stands for pointer comparison (`SurfaceTest` compares snapshots that way).
`ImageRaster` owns a `Bitmap`, which shares its `PixelRef` (`docs/design/pixels.md`).

`makeImageSnapshot` shares the surface's pixel ref with the image. Skia's explicit
`onCopyOnWrite` is the pixel ref's copy-on-write: the next write through the surface's bitmap
detaches it, so the surface keeps drawing into private pixels and the snapshot is unchanged.
`SkSurface_Raster` caches the snapshot until the content changes
(`notifyContentWillChange`/`aboutToDraw` drop it), so `snapshot == snapshot` holds as in Skia.
`SkBitmap::asImage` shares an immutable bitmap and copies a mutable one (`kIfMutable`).

### Mipmaps

The high quality downsampler (`SkMipmap::Build` with `HQDownSampler`) is used, as in Skia.
Skia stores a built mipmap in `SkResourceCache`; here `ImageRaster` holds a
`OnceLock<Option<Arc<Mipmap>>>` built on first use (`tryLoadMips`/`onPeekMips`). The
`MipmapAccessor` picks a level from the matrix and sampling options exactly as
`SkMipmapAccessor::Make` does and hands the level to the shader; its owner lives in the arena.

### The image shader

m156 is built without `SK_ENABLE_LEGACY_SHADERCONTEXT`, so `ImageShader` has only `appendStages`.
The fused `bilerp_clamp_8888` and `bicubic_clamp_8888` stages are the fast paths; other
combinations use `gather` plus `bilinear`/`bicubic` sampling stages with the tile-mode and
decal stages. Both CPU stage families (highp, lowp) are stamped per tier; the lowp bilerp uses the
Q15 lerps of `SkRasterPipeline_opts.h`. The arithmetic and stage order are Skia's, and the
results match the oracle goldens on every tier (GMs `aarectmodes`, `hairmodes`, `bigmatrix`,
`bitmapshader`, `lattice`, `bitmaprect`, `smallcircles`, ...).

### Arena lifetime: image pixels and writable scratch

`ArenaAlloc` only holds `'static` data, but a stage context built from an image has to keep the
image's pixels. `GatherCtx` therefore stores `GatherPixels`: a borrowed slice (tests, oracle
replay) or `Shared(Arc<dyn PixelBytes>)`, which the arena owns. `Bitmap::shared_pixel_bytes`
produces the `Arc` from the pixel ref without copying.

Shaders that need writable memory while running (Skia's `SkArenaAlloc` scratch, as in
`SkBlendShader`'s two-child evaluation) use `ArenaAlloc::alloc_scratch`, which reserves bytes in a
per-blitter scratch buffer named by the memory slot `effect_priv::SHADER_SCRATCH` (`MemSlot(6)`).
The raster pipeline blitter binds a buffer of `scratch_bytes()` at run time (`MemoryBindings`), so
no pointers are stored in contexts. `BlendShader::append_stages` and the image shader's two-pass
(`MatrixRec`) paths use it.

### Drawing

`Canvas::draw_image*` forward to `Device::draw_image_rect`, which `BitmapDevice` implements by
the same decision tree as `SkBitmapDevice::drawImageRect` (sprite, drawBitmap or the shader
path). Skia's `goto` flow is a `bool`. `Draw::draw_bitmap` picks the sprite blitter when
`SkTreatAsSprite` allows it, and otherwise `make_paint_with_image` builds the image shader
(and mipmaps, through `make_paint_with_image_and_mips`). `drawImageNine`/`drawImageLattice` use
`LatticeIter` and `SkDevice::drawImageLattice`; `kFixedColor` colors are read from the image as the
GM does.

### Not ported

Image filters, GPU/lazy/encoded images, `SkImage::scalePixels`/`encodeToData`,
`SkImages::DeferredFromEncodedData`, text-based GMs. `surface_image_unity` and `ImageRawShader`
need an unsafe-addressed pixmap and a PNG decoder respectively.
