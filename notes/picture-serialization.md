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
drawPaint, drawRect, drawOval, drawArc, drawRRect, drawDRRect, drawPath, drawPoints, drawTextBlob,
clipRegion, drawRegion (part 4).

## Not ported yet (each one makes `serialize` return `None`)

- Ops: `DRAW_PICTURE` / `DRAW_PICTURE_MATRIX_PAINT` (nested pictures: the `PICTURE` section and the
  recursion limit), `DRAW_IMAGE*` lattices (`DRAW_IMAGE_LATTICE2`), `DRAW_VERTICES_OBJECT`,
  `DRAW_PATCH`, `DRAW_ATLAS`, `DRAW_DRAWABLE*` (the `DRAWABLE` section), `DRAW_SLUG`,
  `DRAW_ANNOTATION` (`Canvas::draw_annotation` is not ported at all), `DRAW_EDGEAA_*`,
  `DRAW_SHADOW_REC`, `SAVE_BEHIND`, `CLIP_SHADER_IN_PAINT`.
- Save layer fields: backdrop filter (`SAVELAYERREC_HAS_BACKDROP`), backdrop scale, several
  filters (`fFilters` is not in our `SaveLayerRec`), and the obsolete clip mask and matrix.
- Paint effects: image filters (not ported). A shader, color filter or blender without a
  flattenable (`type_name` empty) makes `write_paint` fail, and so does any nested one.
- Shaders not flattenable yet: image shader (needs `writeImage` in its flatten), gradients
  (`SkGradientBaseShader::flatten`, the effects crate), picture shader, perlin noise, runtime
  shaders, `SkCoordClampShader`, `SkWorkingColorSpaceShader`.
- Color filters not flattenable yet: `SkWorkingFormatColorFilter`, `SkGaussianColorFilter`,
  `SkRuntimeColorFilter`, lighting (`SkColorMatrixFilter` in m156, not a separate flattenable).
- Sections: `PICTURE`, `DRAWABLE`, `IMAGE` for mipmaps, `VERTICES`, the slug data. The reader
  rejects any of these unless the section is empty.
- `SkSerialProcs::fPictureProc` and the custom picture format (`kCustom_TrailingStreamByte...`).
- The glyph drawable's `SkDeserialProcs` tag filter (`fAllowTagsProc`) is not ported, so a
  glyph picture with an unexpected section is accepted here (Skia rejects it).

## Deviations to keep in mind

- Corrupt data: Skia's `Forwardport` returns the part of the picture it read; here
  `from_data` returns `None`.
- Old formats: clip ops with region op values above 1 are invalid for every version (Skia
  accepts `kReplace` below version 89). Versions are checked only for the save layer fields.
- `from_data` / `from_stream` use `FlattenableRegistry::EMPTY`. Path effects, mask filters,
  shaders, color filters and blenders need `from_data_with_registry` with
  `skia_rust_effects::flattenable::REGISTRY`. The glyph drawable read
  (`PictureBackedGlyphDrawable::from_buffer`) uses the empty registry too, so a glyph picture with
  any of those effects does not load.
- `addDraw` for an op larger than 24 bits adds 1 to its size, as Skia does (the reader does not use
  the size to find the next op).
- A typeface in a picture is only written through the typeface recorder when no `SerialProcs`
  typeface proc takes it; the custom arm is wired.

## Serialization_PictureTypeface (passing, decision A)

Passes since part 4 (see Step 1 below): the NativeFontations half runs every case; the Portable half
skips, as resource-dependent tests do in that configuration.

## Part 3

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

## Part 4 (decisions A and B, steps 1-3)

### Step 1 (done, decision A): typeface deserialization without procs

`Typeface::register_decoder(factory_id, make_from_stream)` is a process-wide, append-only registry
(`RwLock<Vec<TypefaceDecoder>>` in a `static` in core), the one deliberate exception to "no global
mutable state" (docs/design/text.md Q3). `Typeface::make_deserialize(stream, None)` consults the
built-in empty decoder, then the registry, then the manager's list. The test tooling's
`FontConfig::NativeFontations` registers the Fontations decoder once per process
(`tests/tools/src/font_tool_utils.rs`); the portable configuration does not.

`Serialization_PictureTypeface` passes: its NativeFontations half runs every case (the font
resources load and the pictures compare); its Portable half skips every case, because the portable
manager loads no font data (docs/design/text.md §1.2). That skip is the existing convention for
resource-dependent tests, and it is why the entry is `passing` only with that caveat.

### Step 2 (done for the ported flattenables): shaders, color filters, blenders

`ShaderBase`, `ColorFilterBase` and `BlenderBase` have `type_name` (default empty: not
flattenable) and `flatten` (default: nothing). `FlattenableRegistry` has tables for shaders,
color filters and blenders besides the path effects and mask filters. `effects::flattenable::REGISTRY`
lists them, with the names that `SkGlobalInitialization_default.cpp` registers:

- shaders: `SkColorShader` (and the legacy `SkColorShader4`), `SkEmptyShader`,
  `SkLocalMatrixShader`, `SkShader_Blend` (custom blenders are read but `blend_blender` only takes
  blend modes), `SkColorFilterShader`;
- color filters: `SkModeColorFilter` (with its 8-bit legacy color), `SkColorFilter_Matrix`,
  `SkComposeColorFilter`, `ColorSpaceXformColorFilter`, the legacy `SkSRGBGammaColorFilter`,
  `SkTable_ColorFilter` (`SkColorTable::flatten`/`Deserialize`, 1024 bytes, alpha first);
- blenders: `SkBlendModeBlender`.

`ReadBuffer::read_shader`, `read_color_filter`, `read_blender` and `BinaryWriteBuffer::write_shader`,
`write_color_filter`, `write_blender`. `read_paint` / `write_paint` carry shader, color filter and
blender (the image filter is still null). `BinaryWriteBuffer::flatten_failed()` is set when a
flattenable without a name is written (a shader that is not flattenable, or one nested in a
flattenable that is not); `write_paint` then returns false.

Not done in step 2: gradients (the effects crate has none of `flatten`), the image shader (needs
the image flatten and sampling), the picture shader, `SkColorSpaceXform` for a color space that
does not serialize, and the image filters.

Tests:
- `ColorFilterTest::ColorFilter` ported and flipped (`tests/src/unit/color_filter_test.rs`): the
  blend color filter round trip through `write_color_filter` and `read_color_filter`.
- `tests/tests/paint_flattenables_in_picture.rs` (ours, not a port): paints with each flattenable
  above, serialized and read back through the registry, draw the same pixels as the recorded
  picture.

### Step 3 (partly done): regions

`CLIP_REGION` (op 2) and `DRAW_REGION` (op 61) are recorded (`SkPictureRecord::recordClipRegion`,
`onDrawRegion`) and played back (`do_clip_op` rules: difference and intersect; the region's
clip params use the same packing as the other clips). `ReadBuffer::read_region` is
`SkReadBuffer::readRegion`. `tests/tests/region_in_picture.rs` (ours) checks the round trip.

Not done, with the reason for each:
- Nested pictures (`DRAW_PICTURE`, `DRAW_PICTURE_MATRIX_PAINT`): need the `PICTURE` section (each
  nested picture written with `SkPicture::serialize` and read with the recursion limit minus one),
  and `find_or_append` identity for pictures. Not cheap; no manifest target is blocked on it except
  `Picture_recursion_limit` (which also needs drawables).
- Drawables (`DRAW_DRAWABLE*`, the `DRAWABLE` section, `Picture_nested_draw_drawable`): need the
  drawable flatten (`SkRecordedDrawable`) and the section. `drawable.rs` exists, but not the
  recording hook.
- Annotations (`DRAW_ANNOTATION`): no `Canvas::draw_annotation` exists in core, so there is
  nothing to record. Adding it is an API change, not a picture change.
- Clip shaders, vertices, patches, atlases, shadows, `SAVE_BEHIND` and the corrupt-data partial
  picture behaviour: not started.

## Remaining targets (manifest)

- `tests/PictureTest.cpp::Picture`: passing. `drawImage(nullptr, 0, 0)` is not ported (decision B):
  a null image has no Rust spelling, and the comment in `test_bad_bitmap` says so.
- `tests/ColorFilterTest.cpp::ColorFilter`: passing (decision A's flattenables).
- `tests/SerializationTest.cpp::Serialization_PictureTypeface`: passing (decision A).
- `tests/SerializationTest.cpp::Serialization`: `todo`. Needs image filters, clip shaders, nested
  pictures, drawables and annotations (see Step 3).
- `tests/ImageTest.cpp::Image_Serialize_Encoding_Failure`: passing (Part 3).
- `tests/SkGlyphTest.cpp::SkPictureBackedGlyphDrawable_RejectsAnyShadersThatNeedSkSL`: blocked on
  SkRuntimeEffect (no SkSL port).
- `tests/PictureTest.cpp::Picture_nested_draw_drawable`, `Picture_recursion_limit`: need drawables.

## Next steps (superseded; kept for history)

1. Paints with shaders, color filters, image filters and blenders (`SkPaintPriv::Flatten` arms,
   with their factories), and the save layer backdrop and filters.
2. Images: the `IMAGE` section (`writeImage`/`readImage`, `SkSerialProcs::fImageProc`), then
   `DRAW_IMAGE*` and lattices.
3. Nested pictures (`PICTURE` section, recursion limit), drawables (`DRAW_DRAWABLE*`).
4. Regions and clip shaders, vertices, patches, atlases, annotations, the remaining ops.
5. Corrupt-data behaviour: return the partial picture as `Forwardport` does, where a test observes
   it.
