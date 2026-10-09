# pictureimagegenerator (PictureGeneratorGM)

Status: failing. Every config of every tier mismatches (8888, 565, f16).

Diff (golden | ours | magenta) shows the vector logo shapes match, but the SKIA glyphs differ
everywhere the text is drawn (both the GetPath outlines and the drawSimpleText call with the
gradient shader). Ported: SkTextUtils::GetPath (core utils/text_utils.rs get_path), PictureImageGenerator
and make_from_picture (skia-rust-raster image_picture.rs), draw_vector_logo.

Hypotheses to check next: glyph outline offset/scale in get_path (compare with a SkFont::getPath dump),
subpixel/embolden flags on the portable font, and the SkColorConverter / gradient colour conversion.
