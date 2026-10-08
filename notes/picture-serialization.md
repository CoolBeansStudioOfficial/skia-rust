# Picture serialization, part 1 (branch `port/picture-serial`)

## What landed

Container and op stream for pictures whose commands are all in the list below.

- `picture_data.rs`: `PictInfo` (the `SkPictInfo` header), `PictureData` (the `SkPictureData`
  sections: `read`, `fact`, `tpfc`, `aray` with the paint, path and slug tables, `eof `), its
  `serialize` and `parse_stream`, and the `flattenToBuffer` tables.
- `picture_record.rs`: `backport`, which plays a picture into a no-pixels canvas whose hooks
  are the `SkPictureRecord` overrides, so the op stream is written by the canvas's own
  save/clip/matrix logic (deferred saves included, as in `SkCanvas`).
- `picture_playback.rs`: `forward_port` (`SkPicture::Forwardport`), which reads the op stream and
  plays it into a `PictureRecorder`. Clip ops skip to their restore when the clip is empty.
- `picture_flat.rs`: the op codes used so far (numbered as `DrawType` in `SkPictureFlat.h`), the
  save layer header bits, clip parameter packing.
- `picture.rs`: `serialize`, `serialize_into`, `from_data`, `from_data_with_registry`,
  `from_stream` (`MakeFromData`, `MakeFromStream`). `serialize` returns `None` for a picture that
  has an unported command.
- `read_buffer.rs` / `write_buffer.rs`: `read_paint` / `write_paint` (`SkPaintPriv::Flatten` and
  `Unflatten`), `read_color4f`, `read_rrect`, `write_color4f`, `write_rrect`, `offset`,
  `Writer32::read32_at`.
- Tests: `Picture_empty_serial` and `Picture_preserveCullRect` (`tests/src/unit/picture_test.rs`,
  both flipped to `passing`). Crate tests in `picture.rs` (`serial_tests`): the exact bytes of an
  empty picture, a mixed op set that round-trips to the same bytes, and garbage input.

## Ops encoded and read

save, saveLayer (bounds, paint, flags, backdrop tile mode), restore, concat (`CONCAT44`, from
`translate`, `scale`, `concat`), `setMatrix` (`SET_M44`), clipRect, clipRRect, clipPath, resetClip,
drawPaint, drawRect, drawOval, drawArc, drawRRect, drawDRRect, drawPath, drawPoints.

## Not ported yet (each one makes `serialize` return `None`)

- Ops: `CLIP_REGION` and `DRAW_REGION` (regions), `CLIP_SHADER_IN_PAINT` (clip shaders),
  `DRAW_TEXT_BLOB`, `DRAW_PICTURE` / `DRAW_PICTURE_MATRIX_PAINT` (nested pictures), `DRAW_IMAGE*`
  and `DRAW_IMAGE_LATTICE2` (images), `DRAW_VERTICES_OBJECT`, `DRAW_PATCH`, `DRAW_ATLAS`,
  `DRAW_DRAWABLE*`, `DRAW_SLUG`, `DRAW_ANNOTATION`, `DRAW_EDGEAA_*`, `DRAW_SHADOW_REC`,
  `SAVE_BEHIND`. (Of these, `Record` holds text blobs, images, lattices and nested pictures; the
  rest are never recorded by `RecordCanvas`.)
- Save layer fields: backdrop filter (`SAVELAYERREC_HAS_BACKDROP`), backdrop scale, several
  filters (`fFilters` is not in our `SaveLayerRec`), and the obsolete clip mask and matrix.
- Paint effects: shader, color filter, image filter, custom blender. Only path effects and mask
  filters are written and read, and only with a null blender (a blend mode other than `SrcOver`
  plus an effect needs the `SkBlendMode` blender flattenable).
- Sections: `TEXTBLOB` (`SkTextBlobPriv::Flatten`), typeface section with the typeface recorder
  (`writeTypeface` index arm; the custom arm and the `SerialProcs` typeface proc are wired),
  `PICTURE`, `DRAWABLE`, `IMAGE`, `VERTICES`, the slug data. The reader rejects any of these
  unless the section is empty.
- Factories: flattenables are written by name, and the `fact` section is always empty. Skia writes
  a factory index per flattenable (`SkFactorySet`) and the names in `fact`. Byte output differs
  from Skia when a paint has a path effect or mask filter. The reader handles names only.
- `SkSerialProcs::fPictureProc` and the custom picture format (`kCustom_TrailingStreamByte...`).

## Deviations to keep in mind

- Corrupt data: Skia's `Forwardport` returns the part of the picture it read; here
  `from_data` returns `None`.
- Old formats: clip ops with region op values above 1 are invalid for every version (Skia
  accepts `kReplace` below version 89). Versions are checked only for the save layer fields.
- `from_data` / `from_stream` use `FlattenableRegistry::EMPTY`. Path effects and mask filters
  need `from_data_with_registry` with `skia_rust_effects::flattenable::REGISTRY`.
- `addDraw` for an op larger than 24 bits adds 1 to its size, as Skia does (the reader does not use
  the size to find the next op).

## Remaining targets (manifest)

- `tests/SerializationTest.cpp::Serialization_PictureTypeface`: needs `DRAW_TEXT_BLOB`, the
  `TEXTBLOB` section and the typeface section (in `picture_data.rs`).
- `tests/TextBlobTest.cpp::SkCanvas_drawTextBlob_b513820666`: the same, plus the drawable typeface
  (T17) that draws glyph drawables.
- `tests/SkGlyphTest.cpp::SkGlyph_SendWithDrawable`: glyph drawable flattening, a text-module item
  that does not depend on pictures.
- `tests/SerializationTest.cpp::Serialization` (picture parts): not attempted in this part.

## Next steps

1. Text blobs in pictures (`DRAW_TEXT_BLOB`, `TEXTBLOB` section, typeface section and the
   recorder), then the two text targets.
2. Images (`IMAGE` section with `writeImage`/`readImage`), then `DRAW_IMAGE*` and lattices.
3. Nested pictures (`PICTURE` section, recursion limit) and drawables.
4. Regions and clip shaders, then the remaining paint effects.
5. The factory set (`fact` indices) so the bytes match Skia for effect paints.
