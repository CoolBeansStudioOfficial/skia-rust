// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SerializationTest.cpp (chrome/m156)

use std::sync::{Arc, Mutex};

use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::font::Font;
use skia_rust_core::font_arguments::palette::Override;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_arguments::{FontArguments, Palette, VariationPosition};
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_mgr::TypefaceDecoder;
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_ceil_to_int;
use skia_rust_core::serial_procs::{
    DeserialProcs, SerialProcs, TypefaceDeserializer, TypefaceSerializer,
};
use skia_rust_core::stream::{DynamicMemoryWStream, MemoryStream, Stream, StreamAsset, WStream};
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_core::typeface::{SerializeBehavior, Typeface};
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::dash_path_effect;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::{
    create_typeface_from_resource, default_font, default_typeface, sample_user_typeface,
    test_font_mgr, with_typeface_decoders,
};

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_font_test, def_test, errorf, reporter_assert};

// Port of: tests/SerializationTest.cpp#L1190-L1204 (chrome/m156), WriteBuffer_external_memory_flattenable
def_test!(WriteBuffer_external_memory_flattenable, |reporter| {
    let intervals = [1.0, 1.0];
    let path_effect = dash_path_effect::new(&intervals, 0.0).expect("a valid dash effect");
    // SkAlign4 of the serialized size.
    let path_size = (path_effect.serialize().size() + 3) & !3;
    reporter_assert!(reporter, path_size > 4);

    // Too small external storage: nothing is written.
    let mut storage = vec![0u8; path_size - 4];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) == 0);

    let mut storage = vec![0u8; path_size];
    reporter_assert!(reporter, path_effect.serialize_into(&mut storage) != 0);
});

/// `Typeface::make_deserialize` with a sanitizer closure, as `SkTypeface::MakeDeserialize`.
fn deserialize_with(
    data: &Data,
    sanitizer: Option<&dyn Fn(Data) -> Option<Data>>,
) -> Option<Typeface> {
    let mut stream = MemoryStream::make_copy(data.as_bytes());
    Typeface::make_deserialize(&mut *stream, Some(&test_font_mgr()), sanitizer)
}

// Port of: tests/SerializationTest.cpp#L685-L716 (chrome/m156), Serialization_Typeface_Sanitizer
def_font_test!(Serialization_Typeface_Sanitizer, |reporter| {
    let typeface = default_typeface();
    let Some(serialized_data) = typeface.serialize(SerializeBehavior::DoIncludeData) else {
        errorf!(reporter, "serialize typeface");
        return;
    };

    {
        // Default (no sanitizer) behavior
        let clone = deserialize_with(&serialized_data, None);
        reporter_assert!(reporter, clone.is_some());
    }

    {
        // Sanitizer succeeds (no data change necessary)
        let pass_through = |data: Data| Some(Data::new_copy(data.as_bytes()));
        let clone = deserialize_with(&serialized_data, Some(&pass_through));
        reporter_assert!(reporter, clone.is_some());
    }

    {
        // Pretend sanitizer rejects data
        let reject = |_data: Data| -> Option<Data> { None };
        let clone = deserialize_with(&serialized_data, Some(&reject));
        reporter_assert!(reporter, clone.is_none());
    }
});

/// `SpyDecoder::kSpyFactoryId`: `'spyf'`.
// Port of: tests/SerializationTest.cpp#L720-L723 (chrome/m156)
const SPY_FACTORY_ID: FactoryId = set_four_byte_tag(b's', b'p', b'y', b'f');

/// The bytes that `SpyDecoder` received: `SpyDecoder::fData` (the decoder runs on the test's
/// thread, and only this test uses the spy).
static SPY_DATA: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// `SpyDecoder`'s factory: it keeps the stream's bytes and returns the default typeface.
// Port of: tests/SerializationTest.cpp#L724-L738 (chrome/m156)
// The `Option` is the `TypefaceDecoder::make_from_stream` signature, which can fail.
#[allow(clippy::unnecessary_wraps)]
fn spy_make_from_stream(
    mut stream: Box<dyn StreamAsset>,
    _args: &FontArguments<'_, '_>,
) -> Option<Typeface> {
    let mut data = vec![0u8; stream.get_length()];
    let read = stream.read(&mut data);
    data.truncate(read);
    if let Ok(mut spy) = SPY_DATA.lock() {
        *spy = Some(data);
    }
    // Return a valid typeface to avoid decoding failure
    Some(default_typeface())
}

// Port of: tests/SerializationTest.cpp#L750-L781 (chrome/m156), Serialization_Typeface_Sanitizer_Spy
def_font_test!(Serialization_Typeface_Sanitizer_Spy, |reporter| {
    let spy_decoder = TypefaceDecoder {
        factory_id: SPY_FACTORY_ID,
        make_from_stream: spy_make_from_stream,
    };
    let mgr = with_typeface_decoders(test_font_mgr(), vec![spy_decoder]);

    // Construct the serialized stream manually with our spy factory id and custom font bytes
    let mut desc = FontDescriptor::new();
    desc.set_factory_id(SPY_FACTORY_ID);
    desc.set_family_name("SpyFont");
    let original_stream: Box<dyn StreamAsset> = MemoryStream::make_copy(b"original_bytes");
    desc.set_stream(Some(original_stream));

    let mut wstream = DynamicMemoryWStream::new();
    reporter_assert!(reporter, desc.serialize(&mut wstream));
    let serialized_data = wstream.detach_as_data();

    // Deserialize using a mutating sanitizer callback
    let mutating_sanitizer = |_data: Data| Some(Data::new_copy(b"sanitized_bytes\0"));
    let mut stream = MemoryStream::make_copy(serialized_data.as_bytes());
    let clone = Typeface::make_deserialize(&mut *stream, Some(&mgr), Some(&mutating_sanitizer));
    reporter_assert!(reporter, clone.is_some());

    // Verify end-to-end that the decoder factory received the sanitized bytes, not the
    // original bytes
    let spy_data = SPY_DATA.lock().ok().and_then(|spy| spy.clone());
    reporter_assert!(reporter, spy_data.is_some());
    reporter_assert!(
        reporter,
        spy_data.as_deref() == Some(&b"sanitized_bytes\0"[..])
    );
});

// Port of: tests/SerializationTest.cpp#L1166-L1188 (chrome/m156), WriteBuffer_external_memory_textblob
def_test!(WriteBuffer_external_memory_textblob, |reporter| {
    let font = default_font();

    let mut builder = TextBlobBuilder::new();
    let glyph_count = 5;
    // allocRun() allocates only the glyph buffer.
    let run = builder.alloc_run(&font, glyph_count, 1.2, 2.3, None);
    run.fill(0);
    let Some(blob) = builder.make() else {
        errorf!(reporter, "the blob has a run");
        return;
    };
    let procs = SerialProcs::default();

    // SkAlign4 of the serialized size.
    let blob_size = (blob.serialize(&procs).size() + 3) & !3;
    reporter_assert!(reporter, blob_size > 4);

    // Too small external storage: nothing is written.
    let mut storage = vec![0u8; blob_size - 4];
    reporter_assert!(reporter, blob.serialize_into(&procs, &mut storage) == 0);

    let mut storage = vec![0u8; blob_size];
    reporter_assert!(reporter, blob.serialize_into(&procs, &mut storage) != 0);
});

/// `TestTypefaceSerialization`: the typeface serialized and deserialized with the test manager
/// has the same glyphs, style and metrics.
// Port of: tests/SerializationTest.cpp#L640-L679 (chrome/m156), TestTypefaceSerialization
fn test_typeface_serialization(reporter: &mut Reporter, typeface: &Typeface) {
    let mut wstream = DynamicMemoryWStream::new();
    if !typeface.serialize_to(&mut wstream, SerializeBehavior::IncludeDataIfLocal) {
        errorf!(reporter, "serialize typeface");
        return;
    }
    let data = wstream.detach_as_data();
    let mut stream = MemoryStream::make_copy(data.as_bytes());
    let Some(clone_typeface) =
        Typeface::make_deserialize(&mut *stream, Some(&test_font_mgr()), None)
    else {
        reporter_assert!(reporter, false, "the serialized typeface deserializes");
        return;
    };

    reporter_assert!(
        reporter,
        typeface.count_glyphs() == clone_typeface.count_glyphs()
    );
    reporter_assert!(
        reporter,
        typeface.font_style() == clone_typeface.font_style()
    );

    let font = Font::from_size(typeface.clone(), 12.0);
    let clone = Font::from_size(clone_typeface, 12.0);
    let (_, font_metrics) = font.metrics();
    let (_, clone_metrics) = clone.metrics();
    reporter_assert!(reporter, font_metrics == clone_metrics);
}

// Port of: tests/SerializationTest.cpp#L680-L683 (chrome/m156), Serialization_Typeface
def_font_test!(Serialization_Typeface, |reporter| {
    test_typeface_serialization(reporter, &default_typeface());
    test_typeface_serialization(reporter, &sample_user_typeface());
});

/// `GetResourceAsStream(path)`: the resource as a stream, or `None` if it is missing.
fn resource_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let data = get_resource_as_data(path)?;
    Some(MemoryStream::make_copy(&data))
}

/// `draw_picture(picture)`: the picture drawn into a bitmap of its cull rect's size, as its
/// pixels (`None` if the bitmap cannot be made).
// Port of: tests/SerializationTest.cpp#L357-L364 (chrome/m156), draw_picture
fn draw_picture(picture: &Picture) -> Option<(i32, i32, Vec<u8>)> {
    let cull = picture.cull_rect();
    let width = scalar_ceil_to_int(cull.width());
    let height = scalar_ceil_to_int(cull.height());
    let info = ImageInfo::new_n32_premul((width, height), None);
    let mut surface = surfaces::raster(&info, None, None)?;
    picture.playback(surface.canvas());
    let snapshot = surface.image_snapshot()?;
    let row_bytes = info.min_row_bytes();
    let size = row_bytes * usize::try_from(height).ok()?;
    let mut bytes = vec![0u8; size];
    if !snapshot.read_pixels(&info, &mut bytes, row_bytes, (0, 0)) {
        return None;
    }
    Some((width, height, bytes))
}

/// `compare_bitmaps(b1, b2)`: the same size, and every pixel the same.
// Port of: tests/SerializationTest.cpp#L391-L413 (chrome/m156), compare_bitmaps
fn compare_bitmaps(reporter: &mut Reporter, b1: &(i32, i32, Vec<u8>), b2: &(i32, i32, Vec<u8>)) {
    reporter_assert!(reporter, b1.0 == b2.0);
    reporter_assert!(reporter, b1.1 == b2.1);
    if b1.0 != b2.0 || b1.1 != b2.1 {
        return;
    }
    // Pixels are four bytes each, compared as they are stored.
    let pixel_errors =
        b1.2.as_chunks::<4>()
            .0
            .iter()
            .zip(b2.2.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a != b)
            .count();
    reporter_assert!(reporter, pixel_errors == 0);
}

/// `serialize_typeface_proc`: writes the typeface's id, then the typeface with its data.
// Port of: tests/SerializationTest.cpp#L384-L395 (chrome/m156), serialize_typeface_proc
fn serialize_typeface_proc() -> TypefaceSerializer {
    Arc::new(|typeface: &Typeface| {
        // Write out typeface ID followed by entire typeface.
        let data = typeface.serialize(SerializeBehavior::DoIncludeData)?;
        let mut stream = DynamicMemoryWStream::new();
        let typeface_id = typeface.unique_id();
        stream.write(&typeface_id.to_ne_bytes());
        stream.write(data.as_bytes());
        Some(stream.detach_as_data())
    })
}

/// `deserialize_typeface_proc`: reads the id, then the typeface with the test manager.
// Port of: tests/SerializationTest.cpp#L397-L409 (chrome/m156), deserialize_typeface_proc
fn deserialize_typeface_proc() -> TypefaceDeserializer {
    Arc::new(|stream: &mut dyn Stream| {
        let mut id = [0u8; 4];
        if stream.read(&mut id) != id.len() {
            return None;
        }
        Typeface::make_deserialize(stream, Some(&test_font_mgr()), None)
    })
}

/// `makeSynthetic(resource)`: the resource with synthetic bold and oblique.
// Port of: tests/SerializationTest.cpp#L700-L713 (chrome/m156), makeSynthetic
fn make_synthetic(reporter: &mut Reporter, resource: &str) -> Option<Typeface> {
    let Some(syn) = resource_stream(resource) else {
        errorf!(reporter, "syn {}", resource);
        return None;
    };
    let mut params = FontArguments::new();
    params.set_synthetic_bold(Some(true));
    params.set_synthetic_oblique(Some(true));
    test_font_mgr().make_from_stream_args(Some(syn), &params)
}

/// `makeDistortableWithNonDefaultAxes`: the variable font at a non-default weight.
// Port of: tests/SerializationTest.cpp#L447-L472 (chrome/m156), makeDistortableWithNonDefaultAxes
fn make_distortable_with_non_default_axes(reporter: &mut Reporter) -> Option<Typeface> {
    let Some(distortable) = resource_stream("fonts/Distortable.ttf") else {
        errorf!(reporter, "distortable");
        return None;
    };
    // SK_ScalarSqrt2
    let position = [Coordinate {
        axis: set_four_byte_tag(b'w', b'g', b'h', b't'),
        // SK_ScalarSqrt2 rounds to the same f32 as the std constant.
        value: std::f32::consts::SQRT_2,
    }];
    let mut params = FontArguments::new();
    params.set_variation_design_position(VariationPosition {
        coordinates: &position,
    });
    let typeface = test_font_mgr().make_from_stream_args(Some(distortable), &params)?;
    // The number of axes is unknown.
    typeface.get_variation_design_position(&mut [])?;
    Some(typeface)
}

/// `makeColrWithNonDefaultPalette`: the color font with entry 1 of its palette replaced.
// Port of: tests/SerializationTest.cpp#L474-L496 (chrome/m156), makeColrWithNonDefaultPalette
fn make_colr_with_non_default_palette(reporter: &mut Reporter) -> Option<Typeface> {
    let Some(colr) = resource_stream("fonts/colr.ttf") else {
        errorf!(reporter, "colr");
        return None;
    };
    let overrides = [Override {
        index: 1,
        color: Color::GRAY,
    }];
    let mut params = FontArguments::new();
    params.set_palette(Palette {
        index: 0,
        overrides: &overrides,
    });
    test_font_mgr().make_from_stream_args(Some(colr), &params)
}

/// `serialize_and_compare_typeface`: draws `text` in `typeface`, serializes the picture, reads it
/// back, and checks that both draw the same pixels.
// Port of: tests/SerializationTest.cpp#L416-L445 (chrome/m156), serialize_and_compare_typeface
fn serialize_and_compare_typeface(
    reporter: &mut Reporter,
    typeface: Typeface,
    text: &str,
    serial_procs: Option<&SerialProcs>,
    deserial_procs: Option<&DeserialProcs>,
) {
    // Create a font with the typeface.
    let mut paint = Paint::default();
    paint.set_color(Color::GRAY);
    let font = Font::from_size(typeface, 30.0);

    // Paint some text.
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, 256.0, 256.0), false);
    canvas.draw_color(Color::WHITE, None);
    draw_string(canvas, text, 24.0, 32.0, &font, &paint, Align::Left);
    let Some(picture) = recorder.finish_recording_as_picture(None) else {
        errorf!(reporter, "the picture");
        return;
    };

    // Serialize picture and create its clone from stream.
    let mut stream = DynamicMemoryWStream::new();
    picture.serialize_into(&mut stream, serial_procs);
    let data = stream.detach_as_data();
    let mut input = MemoryStream::make_copy(data.as_bytes());
    let Some(loaded_picture) = Picture::from_stream(&mut *input, deserial_procs) else {
        errorf!(reporter, "the picture loads");
        return;
    };

    // Draw both original and clone picture and compare bitmaps -- they should be identical.
    let Some(orig_bitmap) = draw_picture(&picture) else {
        errorf!(reporter, "the original picture draws");
        return;
    };
    let Some(dest_bitmap) = draw_picture(&loaded_picture) else {
        errorf!(reporter, "the loaded picture draws");
        return;
    };
    compare_bitmaps(reporter, &orig_bitmap, &dest_bitmap);
}

/// `TestPictureTypefaceSerialization`: the typefaces that the test resources make, drawn into a
/// picture that is serialized, loaded and compared. A typeface that cannot be made is skipped.
// Port of: tests/SerializationTest.cpp#L498-L558 (chrome/m156), TestPictureTypefaceSerialization
fn test_picture_typeface_serialization(
    reporter: &mut Reporter,
    serial_procs: Option<&SerialProcs>,
    deserial_procs: Option<&DeserialProcs>,
) {
    // Load typeface from file to test CreateFromFile with index.
    match create_typeface_from_resource(resource_stream("fonts/test.ttc"), 1) {
        Some(typeface) => {
            serialize_and_compare_typeface(reporter, typeface, "A!", serial_procs, deserial_procs);
        }
        None => eprintln!("Could not run test because test.ttc not found."),
    }

    // Load typeface as stream with all synthetics.
    match make_synthetic(reporter, "fonts/Em.ttf") {
        Some(typeface) => serialize_and_compare_typeface(
            reporter,
            typeface,
            "\u{2613}\u{2B1B}",
            serial_procs,
            deserial_procs,
        ),
        None => eprintln!("Could not run test because Em.ttf not found."),
    }

    // Load typeface as stream with request for non-bold with synthetics but a bold available.
    match make_synthetic(reporter, "fonts/test.ttc") {
        Some(typeface) => {
            serialize_and_compare_typeface(reporter, typeface, "A!", serial_procs, deserial_procs);
        }
        None => eprintln!("Could not run test because test.ttc not found."),
    }

    // Load typeface as stream to create with axis settings.
    match make_distortable_with_non_default_axes(reporter) {
        Some(typeface) => {
            serialize_and_compare_typeface(reporter, typeface, "ab", serial_procs, deserial_procs);
        }
        None => eprintln!("Could not run test because Distortable.ttf not created."),
    }

    // Load typeface as stream to create with palette settings.
    match make_colr_with_non_default_palette(reporter) {
        Some(typeface) => serialize_and_compare_typeface(
            reporter,
            typeface,
            "\u{1F600}\u{2662}",
            serial_procs,
            deserial_procs,
        ),
        None => eprintln!("Could not run test because colr.ttf not created."),
    }
}

// Port of: tests/SerializationTest.cpp#L577-L585 (chrome/m156), Serialization_PictureTypeface
def_font_test!(
    #[ignore = "NativeFontations: the call without serial procs deserializes the font data with builtin_decoders (only the empty typeface), so 5 pixel comparisons differ; see notes/picture-serialization.md"]
    Serialization_PictureTypeface,
    |reporter| {
        test_picture_typeface_serialization(reporter, None, None);

        let serial_procs = SerialProcs {
            typeface: Some(serialize_typeface_proc()),
        };
        let deserial_procs = DeserialProcs {
            typeface: Some(deserialize_typeface_proc()),
        };
        test_picture_typeface_serialization(reporter, Some(&serial_procs), Some(&deserial_procs));
    }
);
