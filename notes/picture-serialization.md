# Picture serialization, parts 1-2 (branch `port/picture-serial`)

## What landed

Part 1 (container, op stream, reader) is as before; part 2 adds the factory set, text blobs, and
the glyph drawables that are pictures.

### Part 1

- `picture_data.rs`: `PictInfo` (the `SkPictInfo` header), `PictureData` (the `SkPictureData`
  sections: `read`, `fact`, `tpfc`, `aray` with the paint, path, text blob and slug tables, `eof `),
  its `serialize` and `parse_stream`, and the `flattenToBuffer` tables.
- `picture_record.rs`: `backport`, which plays a picture into a no-pixels canvas whose hooks are
  the `SkPictureRecord` overrides, so the op stream is written by the canvas's own save/clip/matrix
  logic (deferred saves included, as in `SkCanvas`).
- `picture_playback.rs`: `forward_port` (`SkPicture::Forwardport`), which reads the op stream and
  plays it into a `PictureRecorder`. Clip ops skip to their restore when the clip is empty.
- `picture_flat.rs`: the op codes used so far (numbered as `DrawType` in `SkPictureFlat.h`, checked
  against a compiled copy of the enum), the save layer header bits, clip parameter packing.
- `picture.rs`: `serialize`, `serialize_into`, `from_data`, `from_data_with_registry`,
  `from_stream` (`MakeFromData`, `MakeFromStream`). `serialize` returns `None` for a picture that
  has an unported command.
- `read_buffer.rs` / `write_buffer.rs`: `read_paint` / `write_paint` (`SkPaintPriv::Flatten` and
  `Unflatten`), `read_color4f`, `read_rrect`, `write_color4f`, `write_rrect`, `offset`,
  `Writer32::read32_at`.

### Part 2

- Factory set (`SkWriteBuffer::setFactoryRecorder`, `SkFactorySet`, `SkReadBuffer::setFactoryArray`):
  with a factory recorder, a flattenable is written as its index (1-based, not shifted) and its
  name goes to the `fact` section; the reader resolves the index through the names of that
  section. A factory is identified by its name (`getTypeName`), which matches Skia's
  one-factory-per-name. Paints with a path effect or mask filter now serialize to Skia's bytes.
- Text blobs in pictures: `DRAW_TEXT_BLOB` (op 45) with its paint, blob index and origin;
  `TEXTBLOB` section written after the paths (`SkTextBlobPriv::Flatten`) and read back with
  `new_array_from_buffer` semantics. Blobs are shared by unique id (`find_or_append`).
- Typeface section: written from the buffer's typeface recorder (the typefaces the paints and
  blobs index), after the factories and before the buffer, as in `SkPictureData::serialize`.
- `SkDrawable::makePictureSnapshot`, `SkPictureBackedGlyphDrawable::FlattenDrawable` and
  `MakeFromBuffer`, `SkGlyph::flattenDrawable` and `addDrawableFromBuffer`.
- Tests:
  - `crates/skia-rust-effects/tests/picture_factories.rs`: the exact bytes of a picture whose paint
    has a dash path effect and a blur mask filter (factory indices, `fact` names, the paint in the
    buffer), and a read back through `REGISTRY` that writes the same bytes.
  - Flipped to `passing`: `tests/TextBlobTest.cpp::SkCanvas_drawTextBlob_b513820666`,
    `tests/SkGlyphTest.cpp::SkGlyph_SendWithDrawable`,
    `tests/SkGlyphTest.cpp::SkPictureBackedGlyphDrawable_Basic`.
  - `Serialization_PictureTypeface` is ported (both the no-procs and the procs call) but is
    `#[ignore]`d and `failing`: see below.

## Ops encoded and read

save, saveLayer (bounds, paint, flags, backdrop tile mode), restore, concat (`CONCAT44`, from
`translate`, `scale`, `concat`), `setMatrix` (`SET_M44`), clipRect, clipRRect, clipPath, resetClip,
drawPaint, drawRect, drawOval, drawArc, drawRRect, drawDRRect, drawPath, drawPoints, drawTextBlob.

## Not ported yet (each one makes `serialize` return `None`)

- Ops: `CLIP_REGION` and `DRAW_REGION` (regions), `CLIP_SHADER_IN_PAINT` (clip shaders),
  `DRAW_PICTURE` / `DRAW_PICTURE_MATRIX_PAINT` (nested pictures), `DRAW_IMAGE*` and
  `DRAW_IMAGE_LATTICE2` (images), `DRAW_VERTICES_OBJECT`, `DRAW_PATCH`, `DRAW_ATLAS`,
  `DRAW_DRAWABLE*`, `DRAW_SLUG`, `DRAW_ANNOTATION`, `DRAW_EDGEAA_*`, `DRAW_SHADOW_REC`,
  `SAVE_BEHIND`. (Of these, `Record` holds images, lattices and nested pictures; the rest are
  never recorded by `RecordCanvas`.)
- Save layer fields: backdrop filter (`SAVELAYERREC_HAS_BACKDROP`), backdrop scale, several
  filters (`fFilters` is not in our `SaveLayerRec`), and the obsolete clip mask and matrix.
- Paint effects: shader, color filter, image filter, custom blender. Only path effects and mask
  filters are written and read, and only with a null blender.
- Sections: `PICTURE`, `DRAWABLE`, `IMAGE`, `VERTICES`, the slug data. The reader rejects any of
  these unless the section is empty.
- `SkSerialProcs::fPictureProc` and the custom picture format (`kCustom_TrailingStreamByte...`).
- The glyph drawable's `SkDeserialProcs` tag filter (`fAllowTagsProc`) is not ported, so a
  glyph picture with an unexpected section is accepted here (Skia rejects it).

## Deviations to keep in mind

- Corrupt data: Skia's `Forwardport` returns the part of the picture it read; here
  `from_data` returns `None`.
- Old formats: clip ops with region op values above 1 are invalid for every version (Skia
  accepts `kReplace` below version 89). Versions are checked only for the save layer fields.
- `from_data` / `from_stream` use `FlattenableRegistry::EMPTY`. Path effects and mask filters
  need `from_data_with_registry` with `skia_rust_effects::flattenable::REGISTRY`. The glyph
  drawable read (`PictureBackedGlyphDrawable::from_buffer`) uses the empty registry too, so a
  glyph picture with a path effect or mask filter does not load.
- `addDraw` for an op larger than 24 bits adds 1 to its size, as Skia does (the reader does not use
  the size to find the next op).
- A typeface in a picture is only written through the typeface recorder when no `SerialProcs`
  typeface proc takes it; the custom arm is wired.

## Serialization_PictureTypeface (ignored, failing)

The test is ported in full (`tests/src/unit/serialization_test.rs`), with the resource typefaces
loaded through `test_font_mgr()`. Under the `NativeFontations` configuration, the call without
serial procs fails five pixel comparisons: `Typeface::make_deserialize(stream, None, None)`
resolves the font data through `builtin_decoders()`, which only knows the empty typeface, so the
loaded picture draws the empty font. The call with procs passes in both configurations, and the
portable configuration cannot load these files at all (every case skips). Fixing this needs the
font factories in the default decoders (a text-module decision), so the test is `#[ignore]`d with a
reason.

## Part 3 (this batch)

- Images in pictures: `IMAGE` section (`SkBinaryWriteBuffer::writeImage` / `SkReadBuffer::readImage`
  without mipmaps), `DRAW_IMAGE_RECT2` (op 73) recorded from `on_draw_image_rect2` and played back,
  `SerialProcs::image` / `DeserialProcs::image` and `image_data`, `Writer32::write_sampling` and
  `ReadBuffer::read_sampling`. `Image_Serialize_Encoding_Failure` flipped to `passing`.
- Deviations: an image with mipmap levels is refused by the writer (nothing is written); an image
  has no encoded data here, so without a serial image proc it is written as an empty byte array (Skia
  would use `refEncodedData`). An image that cannot be read becomes a transparent 1x1 raster image
  instead of Skia's lazy empty image (same op count, draws nothing with source-over).
- `tests/tests/image_in_picture.rs`: our own round trip (raw pixels as the serialized bytes) draws the
  same pixels as the direct draw.
- `PictureTest::Picture`: all sub-tests are ported (`test_bad_bitmap`, `test_unbalanced_save_restores`,
  `test_peephole`, `test_clip_bound_opt`, `test_gen_id`, `test_cull_rect_reset`, `test_typeface`, the
  SK_DEBUG empty picture tests) and pass. It stays `todo`: `drawImage(nullptr, 0, 0)` has no Rust
  spelling (the same signature as skia-safe). Flipping it needs an API decision.
- `SkPictureBackedGlyphDrawable_RejectsAnyShadersThatNeedSkSL` is blocked on SkRuntimeEffect (no SkSL
  port); its reason is updated.

### Step 1 (not done): typeface deserialization without procs

`Typeface::make_deserialize(stream, None, None)` cannot reach the Fontations decoder: Skia's static
decoder list (`SkTypeface.cpp` `decoders()`, filled by `SK_TYPEFACE_FACTORY_FONTATIONS`) has no Rust
equivalent here. The options checked:

- A compile-time table in `skia-rust-core` cannot name the text crate's `make_from_stream` (text
  depends on core, and core must not depend on text or on skrifa).
- `inventory` (already used by the GM crate) is life-before-main registration. `docs/design/text.md`
  §5.2 rejects that, and on wasm32 `inventory` needs a `__wasm_call_ctors` hook; CI builds core for
  wasm32.
- A process-wide `OnceLock` set by the text crate is global state, and nothing calls it on its own.

So `Serialization_PictureTypeface` stays `failing`/ignored. The design note's Q3 trigger ("a ported test
deserializes with a null manager and expects a non-portable typeface back") is now met. This needs a
decision: e.g. make the default decoder list a parameter of `make_deserialize` taken from a
`FontMgr` the caller passes, or move the Fontations factory table up into the facade crate that
depends on both.

### Step 2 (not done): paint shaders, color filters, image filters, blenders

Not started. Core has shaders and color filters but none of them has a `flatten` (no SkShader or
SkColorFilter flattenables, no factories), and the image filters are not ported. Each needs its
flatten/unflatten arm with the exact bytes. The `Serialization` picture parts depend on this.

### Steps 3 and 4 (not done)

Nested pictures, drawables, regions, clip shaders, vertices, patches, atlases, annotations, shadows,
and the corrupt-data partial-picture behaviour are not started.

## Remaining targets (manifest)

- `tests/PictureTest.cpp::Picture`: `test_typeface` now serializes; the other sub-tests of the
  `DEF_TEST` (`test_bad_bitmap`, `test_unbalanced_save_restores`, `test_peephole`,
  `test_clip_bound_opt`, `test_gen_id`, `test_cull_rect_reset`) are not ported.
- `tests/ImageTest.cpp::Image_Serialize_Encoding_Failure`: needs the `IMAGE` section.
- `tests/SerializationTest.cpp::Serialization`: the picture parts need the paints and ops above.
- `tests/SkGlyphTest.cpp::SkPictureBackedGlyphDrawable_RejectsAnyShadersThatNeedSkSL`: needs
  shaders in a picture's paints.

## Next steps (superseded by Part 3 above; kept for history)

1. Paints with shaders, color filters, image filters and blenders (`SkPaintPriv::Flatten` arms,
   with their factories), and the save layer backdrop and filters.
2. Images: the `IMAGE` section (`writeImage`/`readImage`, `SkSerialProcs::fImageProc`), then
   `DRAW_IMAGE*` and lattices.
3. Nested pictures (`PICTURE` section, recursion limit), drawables (`DRAW_DRAWABLE*`).
4. Regions and clip shaders, vertices, patches, atlases, annotations, the remaining ops.
5. Corrupt-data behaviour: return the partial picture as `Forwardport` does, where a test observes
   it.
