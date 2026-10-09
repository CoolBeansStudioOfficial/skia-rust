# pictureimagegenerator (PictureGeneratorGM)

Status: passing (45/45 checks match on every tier and config).

Root cause (fixed): text drawn with `SkCanvas::drawSimpleText` while recording a picture was lost.
`SkRecordCanvas::onDrawGlyphRunList` (src/core/SkRecordCanvas.cpp) turns the glyph run list into a
text blob (`GlyphRunList::makeBlob`, src/text/GlyphRun.cpp) and records it as a DrawTextBlob. The
Rust canvas had no such override, so the run list went to the record device and nothing was
recorded. The logo shapes (paths) were recorded and matched, which is why only the SKIA glyphs
differed. Fix: `CanvasHooks::on_draw_glyph_run_list` (checked first in `draw_glyph_run_list`),
`RecordCanvas` override, and `GlyphRunList::make_blob` in skia-rust-core `glyph_run.rs`.

Glyph outline extraction (`SkTextUtils::GetPath`, `SkFont::getPaths`/`setupForAsPaths`) was
checked against the C++ and matched; it was not the cause.
