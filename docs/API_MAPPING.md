# API mapping

Deviations from the reference API (rust-skia's `skia-safe`, see `docs/PORTING.md` §3), and non-mechanical names for symbols `skia-safe` doesn't expose. One row per symbol, grouped by module, kept sorted.

| Skia | skia-safe | skia-rust | Why |
|---|---|---|---|
| **alpha_type** | | | |
| `SkAlphaType` | `AlphaType` (in `image_info`) | `alpha_type::AlphaType` | `SkAlphaType.h` is its own file (PORTING §2); the `ImageInfo` port will re-export it |
| `SkAlphaTypeIsOpaque` | none | `AlphaType::is_opaque` | mechanical name |
| **color** | | | |
| `SkColor` | `Color` (no raw accessor) | `Color` + `From<Color> for u32` | safe replacement for skia-safe's crate-private `into_native` |
| `SkColor4f::toSkColor` | `Color4f::to_color` (truncates) | `Color4f::to_color` (Skia rounding via `Sk4f_toL32`) | skia-safe's version differs from Skia; skia-rust ports Skia |
| `SkPMColor4f` / `SkRGBA4f<kPremul>` | none | `PMColor4f` | alpha-type template modelled as two concrete types sharing a macro |
| `SkPMColor` helpers | none | `color::{pm_color_set_argb, pm_color_get_*}` | mechanical rule; SkPMColor byte order is Skia's platform default (BGRA on Windows, RGBA elsewhere) |
| `SkUnPreMultiply` | none | `un_pre_multiply::{get_scale, apply_scale, pm_color_to_color}` | class with only statics becomes a module |
| **color_space** | | | |
| `SkColorSpace` | `ColorSpace = RCHandle<SkColorSpace>` | `ColorSpace(Arc<..>)` | cheaply clonable shared handle, same methods |
| `SkColorSpace::MakeSRGB`, `MakeSRGBLinear` | `new_srgb`, `new_srgb_linear` | same | same as skia-safe |
| `SkColorSpace::MakeRGB` | missing (`// TODO: makeRGB`) | `ColorSpace::new_rgb(&TransferFunction, &Matrix3x3) -> Option<ColorSpace>` | mechanical name |
| `SkColorSpace::Make(const skcms_ICCProfile&)` | `new_icc(&[u8])` (parses the bytes) | `ColorSpace::make(&IccProfile) -> Option` and `ColorSpace::new_icc(&[u8]) -> Option` | both entry points |
| `SkColorSpace::MakeCICP` | `new_cicp` | same | same as skia-safe |
| `SkColorSpace::toProfile(skcms_ICCProfile*)` | missing | `to_profile() -> IccProfile` | out-param becomes a return value |
| `SkColorSpace::toXYZD50(skcms_Matrix3x3*)` (always true) | missing | `to_xyzd50() -> Matrix3x3` | the C++ cannot fail |
| `SkColorSpace::isNumericalTransferFn` | missing | `is_numerical_transfer_fn() -> Option<TransferFunction>` | bool + out-param |
| `SkColorSpace::makeLinearGamma`, `makeSRGBGamma`, `makeColorSpin` | `with_linear_gamma`, `with_srgb_gamma`, `with_color_spin` | same | same as skia-safe |
| `SkColorSpace::serialize`, `writeToMemory`, `Deserialize` | `serialize() -> Data`, no `writeToMemory`, `deserialize(Data) -> Self` | `serialize() -> Vec<u8>`, `write_to_memory(Option<&mut [u8]>) -> usize`, `deserialize(&[u8]) -> Option<ColorSpace>` | `SkData` is not ported; failure is `None`, not a panic |
| `SkColorSpace::Equals` / pointer comparison `a.get() == b.get()` | `PartialEq` (`Equals`) | `ColorSpace::equals(Option<&..>, Option<&..>)`, `PartialEq` (`Equals`), `ColorSpace::ptr_eq` | null handling; `ptr_eq` for identity, as Skia's tests need |
| `SkColorSpace::transferFn(float[7])` (deprecated overload) | missing | not ported | deprecated; use `transfer_fn` |
| `SkColorSpace::transferFn`, `invTransferFn`, `gamutTransformTo`, `transferFnHash`, `hash` | `transfer_fn`, `inv_transfer_fn`, no `gamutTransformTo`, `transfer_fn_hash`, `hash` | same, plus `gamut_transform_to(&ColorSpace) -> Matrix3x3` | mechanical name |
| `SkColorSpacePrimaries` | `ColorSpacePrimaries` (`rx`, ...) | same; `to_xyzd50() -> Option<Matrix3x3>` | bool + out-param |
| `skcms_TransferFunction` in `SkColorSpace.h` | `ColorSpaceTransferFn` struct | `ColorSpaceTransferFn = skcms::TransferFunction` | one type shared with `skia-rust-skcms` |
| `SkNamedTransferFn::kSRGB`, `k2Dot2`, ... | `named_transfer_fn::{SRGB, DOT22, ...}` | same names; `kSRGB` is computed as in C++ (`(float)(1/1.055)` in `f64`) | skia-safe computes it in `f32` |
| `SkNamedGamut::k*` | missing (`// TODO: SkNamedGamut`) | `named_gamut::{SRGB, ADOBE_RGB, DISPLAY_P3, REC2020, XYZ}` | mechanical names |
| `SkNamedPrimaries::CicpId`, `SkNamedTransferFn::CicpId` | bindgen enums | Rust enums (`from_u8`; `CicpId::SRGB` is an alias constant of `IEC61966_2_1`) | a Rust enum cannot hold arbitrary `uint8_t` values |
| `SkNamedPrimaries::GetCicp`, `GetCicpFromMatrix`, `SkNamedTransferFn::GetCicp` | not exposed | `named_primaries::{get_cicp, get_cicp_from_matrix}`, `named_transfer_fn::get_cicp` | out-params become `Option` |
| `SkColorSpacePriv` (`sk_srgb_singleton`, `gNarrow_toXYZD50`, `is_almost_srgb`, ...) | not exposed | `#[doc(hidden)] color_space_priv::{srgb_singleton, srgb_linear_singleton, NARROW_TO_XYZD50, is_almost_srgb, ...}` | PORTING §3 |
| `SkColorSpaceXformSteps` | not exposed | `color_space_xform_steps::ColorSpaceXformSteps` (`flags`, `src_tf`, `dst_tf_inv`, `src_to_dst_matrix`, ...) | fields lose the `f` prefix |
| `SkColorSpaceXformSteps::apply(float*)` | not exposed | `apply(&mut [f32; 4])` | no raw pointer |
| `SkColorSpaceXformSteps::apply(SkRasterPipeline*)` | not exposed | not ported yet | needs `SkRasterPipeline` |
| `SkColorSpaceXformSteps::operator bool` | not exposed | `is_needed()` | mechanical name |
| **data / stream** | | | |
| `SkData` | `Data = RCHandle<SkData>` | `data::Data` (`Clone`, `Arc`-backed; `Deref<Target=[u8]>`, `PartialEq`/`Eq` with Skia's `operator==`) | PORTING §3 |
| `SkData::MakeWithCopy`, `MakeEmpty`, `MakeZeroInitialized`, `MakeSubset` | `new_copy`, `new_empty`, `new_zero_initialized`, `new_subset` | same | same as skia-safe |
| `SkData::MakeUninitialized` | `unsafe new_uninitialized` | `Data::new_uninitialized(len) -> Data` (safe, zero-filled) + `writable_data(&mut self) -> Option<&mut [u8]>` | safe Rust cannot expose uninitialised bytes; `writable_data` works only while the buffer is uniquely owned (Skia: "use with caution") |
| `SkData::MakeWithoutCopy` | `unsafe new_bytes` | `Data::new_static(&'static [u8])` | a borrowed pointer cannot be held safely; static memory is the safe subset |
| `SkData::MakeWithProc(ptr, len, proc, ctx)` | `new_with_owner(T: AsRef<[u8]> + Send)` | `Data::new_with_owner(T: AsRef<[u8]> + Send + Sync + 'static)` | the owner's `Drop` is the release proc; `Sync` because `Data` is shared across threads |
| `SkData::MakeFromMalloc` | none | `Data::new_from_vec(Vec<u8>)` | an owned buffer |
| `SkData::MakeWithCString` | `new_cstr(&CStr)` | `Data::new_with_cstring(Option<&CStr>)` | `None` is Skia's `nullptr` (the empty string, size 1); includes the nul like Skia |
| `SkData::MakeFromFILE`, `MakeFromFD` | none | `Data::new_from_file(&File)` | a `File` is both; reads the file instead of mmap, `None` for an empty file (as the failed mmap), position unchanged |
| `SkData::MakeFromFileName` | `from_filename` | same | same as skia-safe |
| `SkData::MakeFromStream` | `from_stream(impl io::Read, size)` | `Data::from_stream(&mut dyn Stream, size)` | takes a skia-rust `Stream` |
| `SkData::shareSubset`, `copySubset` | `share_subset` | `share_subset`, `copy_subset` (`Option<Data>`) | same as skia-safe, plus `copy_subset` |
| `SkData::copyRange` | `copy_range(offset, &mut [u8]) -> &Self` | `copy_range(offset, length, Option<&mut [u8]>) -> usize` | Skia returns the count, and the buffer may be null |
| `SkData::equals`, `Equals` | none | `Data::equals(Option<&Data>)`, `Data::equals_opt(a, b)` | mechanical names |
| `SkData::byteSpan`, `bytes`, `data`, `size`, `empty`, `isEmpty` | `as_bytes`, `size`, `is_empty` | same | |
| `SkDataTable` | not exposed | `data_table::DataTable` (`Clone`, `Arc`-backed); `count`/indices are `usize` | `int` becomes `usize` |
| `SkDataTable::at(i, size*)`, `atT<T>`, `atSize`, `atStr` | not exposed | `at(i) -> &[u8]` (its length is the size), `at_size`, `at_str(i) -> &str` | no raw pointers; `atT<T>` is a reinterpret of the bytes |
| `SkDataTable::MakeCopyArrays(ptrs, sizes, count)` | not exposed | `make_copy_arrays(&[&[u8]])` | sizes are the slice lengths |
| `SkDataTable::MakeCopyArray(array, elemSize, count)` | not exposed | `make_copy_array(&[u8], elem_size, count)` | |
| `SkDataTable::MakeArrayProc(array, elemSize, count, proc, ctx)` | not exposed | `make_array_proc(impl AsRef<[u8]> + Send + Sync + 'static, elem_size, count)` | the owner's `Drop` is the free proc |
| `SkStream` | `Stream<N>` pointer wrapper | `stream::Stream` trait (all capabilities, as m156's `SkStream`) | `SkStream` in Skia is one class declaring every capability with a "not supported" default; the trait has the same methods and defaults |
| `SkStreamRewindable`, `SkStreamSeekable`, `SkStreamAsset`, `SkStreamMemory` | `StreamAsset`, `MemoryStream` wrappers | marker sub-traits `StreamRewindable: Stream`, `StreamSeekable`, `StreamAsset`, `StreamMemory` | an implementation declares the capability it promises; `StreamAsset` also has `duplicate_asset` / `fork_asset` (the covariant `duplicate()` / `fork()` return types) |
| `SkStream::read(void* buffer, size_t size)` | n/a | `read(&mut self, &mut [u8]) -> usize`, plus `skip(size)` | the `buffer == nullptr` "skip" mode is `skip`; the default `skip` reads into scratch, streams override it |
| `SkStream::peek(buffer, size) const` | n/a | `peek(&mut self, &mut [u8]) -> usize` | `&mut self`: `FrontBufferedStream::peek` buffers through the wrapped stream, which `const_cast`s in Skia |
| `SkStream::readS8`... `readScalar`, `readPackedUInt`, `readBool` | n/a | `read_s8` ... `read_scalar`, `read_packed_uint`, `read_bool` returning `Option<T>` | out-param + `bool` becomes `Option`; native-endian like Skia |
| `SkStream::duplicate()`, `fork()` | n/a | `Option<Box<dyn Stream>>` | |
| `SkStream::move(long)` | n/a | `move_by(i64)` | `move` is a keyword; `long` is 64-bit as on Linux/macOS |
| `SkStream::getMemoryBase()`, `getData()` | n/a | `get_memory_base(&self) -> Option<&[u8]>`, `get_data(&self) -> Option<Data>` | `None` for an empty `MemoryStream` (Skia's `SkData::MakeEmpty()->data()` is null) |
| `SkStream::MakeFromFile` | n/a | `stream::make_from_file(path) -> Option<Box<dyn StreamAsset>>` | a free function |
| `SkMemoryStream` | `MemoryStream<'a>` (`from_bytes`) | `stream::MemoryStream` (holds a `Data`): `new`, `with_length`, `from_data(Option<Data>)`, `make`, `make_copy`, `make_direct(&'static [u8])`, `set_data`, `set_memory_copy`, `set_memory_static`, `set_memory_owned`, `get_at_pos` | borrowed memory is not expressible; `SkMemoryStream(size)` is zero-filled and not writable through `getMemoryBase()`; inherent `duplicate()` / `fork()` return `Box<MemoryStream>` |
| `SkFILEStream` | none | `stream::FileStream` (`new(path)`, `from_file(File)`, `from_file_with_size`, `make`, `is_valid`, `close`) over `std::fs::File` | reads at absolute offsets (`sk_qread`), so duplicates and forks share one open file |
| `SkFILEWStream` | none | `stream::FileWStream` (`new(path)`, `is_valid`, `fsync`) | buffered by `BufWriter`; `bytes_written` counts the bytes written |
| `SkWStream` | `WStream` pointer wrapper | `stream::WStream` trait | |
| `SkWStream::write8/16/32/64`, `writeText`, `writeBool`, `writeScalar`, `writePackedUInt` | n/a | `write8(u8)`, `write16(u16)`, `write32`, `write64`, `write_text(&str)`, `write_bool`, `write_scalar`, `write_packed_uint` | native-endian like Skia; Skia's `U8CPU`/`U16CPU` parameters are `u8`/`u16` |
| `SkWStream::writeDecAsText`, `writeBigDecAsText`, `writeHexAsText`, `writeScalarAsText` | n/a | `write_dec_as_text`, `write_big_dec_as_text` (formats as unsigned, like Skia), `write_hex_as_text`, `write_scalar_as_text` | use `string::str_append_*` (below) |
| `SkWStream::SizeOfPackedUInt` | n/a | `stream::size_of_packed_uint` | a free function (a static in C++) |
| `SkNullWStream` | none | `stream::NullWStream` | |
| `SkDynamicMemoryWStream` | `DynamicMemoryWStream` | `stream::DynamicMemoryWStream` | the block list is a `Vec` of blocks with Skia's block sizes; `read(buffer, offset)`, `copy_to(&mut [u8])` (copies the first `bytes_written()` bytes), `copy_to_and_reset(Option<&mut [u8]>)`, `write_to_stream`, `write_to_and_reset(&mut dyn WStream)` and `write_to_and_reset_dynamic(&mut DynamicMemoryWStream)` (overloads get distinct names), `prepend_to_and_reset`, `detach_as_data/vector/stream`, `pad_to_align4`, `reset` |
| `SkStreamPriv` | not exposed | `#[doc(hidden)] stream_priv::{copy_stream_to_data, copy, DebugfStream, write_u16_be.., read_u16_be.., remaining_length_is_below}` | PORTING §3 |
| `SkRBuffer` | not exposed | `buffer::RBuffer<'a>` over a `&[u8]` | `read(&mut [u8]) -> bool`, `read_u8/s32/u32 -> Option`, `skip -> Option<&[u8]>`; `skipToAlign4` aligns the offset, not the address; `SkWBuffer` is not ported (unused so far) |
| `SkStrAppendU32/S32/U64/S64/Scalar`, `SkString::appendHex` | not exposed | `string::{str_append_u32, str_append_s32, str_append_u64, str_append_s64, str_append_scalar, str_append_hex}` appending to a `String` | `SkString` itself is `String`; the number formatting is ported because Skia's output format is observable (`%.8g` with `nan`/`inf` spelled out) |
| `android::skia::FrontBufferedStream::Make` (`client_utils/android`) | not exposed | `front_buffered_stream::FrontBufferedStream::make(Option<Box<dyn Stream>>, size) -> Option<Box<dyn StreamRewindable>>` | |
| **float_bits** | | | |
| `SkFloat2Bits`, `SkBits2Float`, `SkFloatAs2sCompliment`, ... (`SkFloatBits.h`) | not exposed | `float_bits::{float_to_bits, bits_to_float, float_as_2s_compliment, ...}` | mechanical names; needed by `ScalarTest` |
| **image_info / pixmap / bitmap** | | | |
| `SkColorType` | `ColorType` (`Alpha8`, `RGB565`, ..., `R8UNorm`; `ColorType::N32`, `COUNT`) | `color_type::ColorType` (same variants and discriminants; `N32`, `LAST_ENUM`, `COUNT`, `n32()`) | `N32` follows Skia's platform default: BGRA on Windows, RGBA elsewhere (`color_priv::PMCOLOR_IS_BGRA`) |
| `(SkColorType)i` C-style cast, `SkColorTypeIsValid` | none | `ColorType::from_i32(i32) -> Option<ColorType>`, `image_info_priv::color_type_is_valid(u32)` | there is no integer cast to an enum in Rust |
| `SkColorTypeBytesPerPixel`, `SkColorTypeIsAlwaysOpaque`, `SkColorTypeValidateAlphaType` | `ColorType::{bytes_per_pixel, is_always_opaque, validate_alpha_type}` | same; `validate_alpha_type(AlphaType) -> Option<AlphaType>` | out-param becomes the `Option` payload |
| `SkImageInfoPriv.h` (`SkColorTypeChannelFlags`, `SkColorTypeNumChannels`, `SkColorTypeIsAlphaOnly`, `SkColorTypeShiftPerPixel`, `SkColorTypeMinRowBytes`, `SkColorTypeComputeOffset`, `SkColorTypeIsNormalized`, `SkColorTypeMaxBitsPerChannel`, `SkColorInfoIsValid`, `SkImageInfoIsValid`, `SkImageInfoValidConversion`, `SkAlphaTypeIsValid`) | not exposed | `#[doc(hidden)] image_info_priv::{color_type_channel_flags, ...}` | PORTING §3. `ColorChannelFlag` has no zero constant: `color_type_channel_flags(Unknown)` is `ALPHA & RED` (bits 0) |
| `SkYUVColorSpace` | `YUVColorSpace` (bindgen names) | `image_info::YUVColorSpace` (`JPEGFull`, `Rec601Limited`, `BT2020_8BitFull`, `YCgCo_8BitFull`, ..., `Identity`; consts `JPEG`, `REC601`, `REC709`, `BT2020`) | UpperCamelCase variants; the `kJPEG_SkYUVColorSpace` aliases are associated consts. `SkYUVColorSpaceIsLimitedRange` is `YUVColorSpace::is_limited_range` |
| `SkColorInfo` | `ColorInfo` | `image_info::ColorInfo` (`Clone`, `Default`, `PartialEq` = `SkColorInfo::operator==`) | `color_space() -> Option<ColorSpace>` (a clone of the handle); `color_space_ref() -> Option<&ColorSpace>` is the borrowing `colorSpace()`; `is_gamma_close_to_srgb` |
| `SkImageInfo` | `ImageInfo` (`Handle<SkImageInfo>`) | `image_info::ImageInfo` (plain struct; `Clone`, `Default`, `PartialEq` = `operator==`) | no FFI handle |
| `SkImageInfo::Make*` | `ImageInfo::{new, from_color_info, new_n32, new_s32, new_n32_premul, new_a8, new_unknown}` | same; `dimensions: impl Into<ISize>` (so `(w, h)` works), `cs: impl Into<Option<ColorSpace>>` | same as skia-safe |
| `SkImageInfo::makeWH` | none | `ImageInfo::with_wh(w, h)` | mechanical name |
| `SkImageInfo::minRowBytes64` | none | `ImageInfo::min_row_bytes64() -> u64` | |
| `SkImageInfo::minRowBytes` | `min_row_bytes` (no 31-bit check) | `min_row_bytes` returns 0 if the row bytes do not fit in an `i32` | Skia's semantics (`SkTFitsIn<int32_t>`) |
| `SkImageInfo::validRowBytes` | compares to `minRowBytes()` | compares to `min_row_bytes64()` | Skia's semantics |
| `SkImageInfo::computeOffset` | `compute_offset(impl Into<IPoint>, usize)` | same (debug-asserts the point is in bounds) | same as skia-safe |
| `SkImageInfo::computeByteSize` | `compute_byte_size` | same, with `SkSafeMath` and the `SK_MaxS32` limit ported | same as skia-safe; `usize::MAX` on overflow |
| `SkImageInfo::ByteSizeOverflowed` | none | `ImageInfo::byte_size_overflowed(usize)` | mechanical name |
| `SkImageInfo::validate` (debug) | none | `ImageInfo::validate()` | |
| `SkPixmap` | `Pixmap<'a>` (raw pointer inside) | `pixmap::Pixmap<'a>`: shared `&[u8]` (`new_readonly`), unique `&mut [u8]` (`new`), or a lock guard on a `PixelRef` (`Bitmap::peek_pixels` / `peek_pixels_mut`) | the safe replacement for `const void*` + `const_cast`. Writing methods (`erase`, `erase_4f`, `writable_addr`, `set_addr*`) need a writable kind: `erase*` return `false` on a read-only pixmap, the rest panic or return `None` |
| `SkPixmap::SkPixmap(info, addr, rowBytes)`, `reset(info, addr, rowBytes)` | `Pixmap::new(&ImageInfo, &mut [u8], row_bytes) -> Option<Self>` | same, plus `new_readonly(&ImageInfo, &[u8], row_bytes)`; `None` if `row_bytes < min_row_bytes()` or the slice is shorter than `compute_byte_size(row_bytes)`; no `reset(info, addr, rb)` | Skia performs no such checks; safe indexing needs them |
| `SkPixmap::reset(const SkMask&)`, `readPixels` (3 overloads), `scalePixels` | | not ported | `SkMask`, `SkConvertPixels`, `SkRasterPipeline`, sampling are not ported |
| `SkPixmap::extractSubset(SkPixmap*, const SkIRect&)` | `extract_subset(&self, area) -> Option<Pixmap>` | `extract_subset(&self, area) -> Option<Pixmap<'_>>` (read-only view) and `extract_subset_mut(&mut self, area)` | bool + out-param becomes `Option` |
| `SkPixmap::addr()` | `addr() -> *const c_void` | `addr() -> Option<&[u8]>` (and `bytes()`, `bytes_mut()`) | no raw pointers; `None` for a null address |
| `SkPixmap::addr(x, y)`, `writable_addr()`, `writable_addr(x, y)` | `addr_at`, `writable_addr`, `writable_addr_at` (pointers) | `addr_at(p) -> Option<&[u8]>`, `writable_addr() -> Option<&mut [u8]>`, `writable_addr_at(p) -> Option<&mut [u8]>` | slices from the pixel to the end of the buffer |
| `SkPixmap::addr8/16/32/64(x, y)`, `addrF16(x, y)`, `writable_addr8/16/32/64(x, y)` | none | `addr8/16/32/64(x, y) -> u8/u16/u32/u64`, `addr_f16(x, y) -> [u16; 4]`, `set_addr8/16/32/64(x, y, value)` (native-endian) | a `&[u8]` is not aligned for `u16`..`u64`, so pixel values are returned and written instead of pointers. The no-argument `addr8()`.. overloads are not ported |
| `SkPixmap::getColor/getColor4f/getAlphaf` | `get_color`, `get_color_4f`, `get_alpha_f` (`impl Into<IPoint>`) | same | same as skia-safe; ported from Skia exactly (`SkColorSetARGB` truncating conversions, `powf` for sRGB: `// skia-rust: libm`) |
| `SkPixmap::erase` (3 overloads) | `erase(color, Option<&IRect>)`, `erase_4f` | same | `erase(SkColor)` is `erase(color, None)`. The conversion goes through `convert_pixels::convert_rgba_f32_premul_pixel`, a one-pixel stand-in for `SkConvertPixels` (see below) |
| `SkConvertPixels` (as used by `SkPixmap::erase`) | not exposed | `#[doc(hidden)] convert_pixels::convert_rgba_f32_premul_pixel(&ImageInfo, &PMColor4f) -> Option<[u8; 16]>` | the `rect_memcpy` / `convert_to_alpha8` fast paths and the highp `SkRasterPipeline` stages (`unpremul`, transfer functions with `approx_powf`, `matrix_3x3`, `premul`, every `store_*`) for one `RGBA_F32` premul pixel. `mad` is `a*b+c` and `round` is ties-to-even, as the SSE/portable tiers; the AVX2 tiers use an FMA in `mad`. Replace with the real `SkConvertPixels` when `SkRasterPipeline` lands |
| `SkPixelRef` | `PixelRef = RCHandle<SkPixelRef>` | `pixel_ref::PixelRef(Arc<..>)`: cheaply clonable, **owns** its bytes (`Vec<u8>` in a `RwLock`, or shared `Data`) | safe replacement for `void* + rowBytes`. `pixels() -> PixelsRead` (derefs to `[u8]`), `pixels_mut() -> Option<PixelsWrite>` (`None` for `Data`-backed refs). Mutating methods take `&self` (atomics), as `Arc` has no `&mut` |
| `SkPixelRef::SkPixelRef(w, h, addr, rowBytes)` | none | `PixelRef::new(w, h, Vec<u8>, row_bytes)` | the pixel ref takes ownership |
| `SkPixelRef::getGenerationID`, `notifyPixelsChanged`, `isImmutable`, `setImmutable`, `addGenIDChangeListener`, `notifyAddedToCache`, `dimensions`, `width`, `height`, `rowBytes` | `generation_id`, `notify_pixels_changed`, `is_immutable`, `set_immutable`, ... | same, `&self`; `add_gen_id_change_listener(Option<Arc<IdChangeListener>>)` | |
| `SkPixelRef::android_only_reset`, `diagnostic_only_getDiscardable`, `SkPixelStorage` base class, `SkNotifyBitmapGenIDIsStale` | | not ported | Android-only / GPU proxy / `SkBitmapCache` not ported |
| `SkIDChangeListener`, `SkIDChangeListener::List` | none | `id_change_listener::{IdChangeListener, IdChangeListenerList}`; `IdChangeListener::new(impl Fn() + Send + Sync) -> Arc<IdChangeListener>`, `mark_should_deregister`, `should_deregister`, `notify_changed` (the virtual `changed()`) | the C++ abstract class is a closure holder; `sk_sp` is `Arc`, `unique()` is `Arc::strong_count(..) == 1` |
| `SkMakePixelRefWithProc(w, h, rowBytes, addr, releaseProc, ctx)` | none | `#[doc(hidden)] pixel_ref_priv::make_pixel_ref_with_proc(w, h, row_bytes, Vec<u8>, Option<ReleaseProc>) -> PixelRef`; `ReleaseProc = Box<dyn FnOnce(Vec<u8>) + Send>` | the pixel ref owns the bytes and hands them back to the proc; the context is whatever the closure captures |
| `SkMallocPixelRef::MakeAllocate`, `MakeWithData` | none | `malloc_pixel_ref::{make_allocate(&ImageInfo, row_bytes) -> Option<PixelRef>, make_with_data(&ImageInfo, row_bytes, Data) -> Option<PixelRef>}` | namespace of functions becomes a module; `nullptr` becomes `None`. A `Data`-backed pixel ref is immutable and read-only |
| `SkData::unique()` (`SkRefCnt`) | none | `Data::unique()` | needed by `MallocPixelRefTest` |
| `SkBitmap` | `Bitmap = Handle<SkBitmap>` | `bitmap::Bitmap` (plain struct: info, row bytes, `Option<PixelRef>`, pixel offset; `Clone` shares the pixel ref) | no FFI handle |
| `SkBitmap::pixmap`, `peekPixels`, `getPixels` | `pixmap() -> &Pixmap`, `peek_pixels() -> Option<Pixmap>`, `pixels() -> *mut c_void` | `pixmap() -> Pixmap<'_>` (read lock), `peek_pixels() -> Option<Pixmap<'_>>` (read lock), `peek_pixels_mut() -> Option<Pixmap<'_>>` (write lock; `None` for `Data`-backed pixels) | the C++ writes through `const`. A lock is held while the pixmap lives: do not call other pixel-touching bitmap methods on the same thread meanwhile |
| `SkBitmap::getAddr(x, y)`, `getAddr8/16/32(x, y)` | `get_addr` (pointer) | `get_addr8/16/32(x, y) -> u8/u16/u32`, `set_addr8/16/32(x, y, value)` | pointer replaced by value access; `getAddr` is not ported |
| `SkBitmap::setInfo`, `tryAllocPixels` (info overloads), `allocPixels` (info overloads), `tryAllocN32Pixels`, `allocN32Pixels`, `tryAllocPixelsFlags`, `allocPixelsFlags` | `set_info`, `try_alloc_pixels_info`, `alloc_pixels_info`, `try_alloc_n32_pixels`, `alloc_n32_pixels`, `try_alloc_pixels_flags`, `alloc_pixels_flags` | same; `row_bytes: impl Into<Option<usize>>` (`None` = `minRowBytes()`; `set_info` treats `None` as 0), `dimensions: impl Into<ISize>` | same as skia-safe. `tryAllocPixels()` / `allocPixels()` without arguments are `try_alloc_pixels()` / `alloc_pixels()`. `SkBitmap::Allocator` is not ported (heap allocation only); `alloc_*` panic where the C++ aborts |
| `SkBitmap::installPixels(info, pixels, rowBytes[, releaseProc, context])` | not wrapped | `install_pixels(&ImageInfo, impl Into<Option<Vec<u8>>>, row_bytes) -> bool` and `install_pixels_with_proc(.., Option<ReleaseProc>)` | the bitmap owns the bytes (see `PixelRef`); too-short bytes are rejected (`false`) instead of being trusted. `installPixels(const SkPixmap&)` is not ported (a pixmap only borrows its bytes) |
| `SkBitmap::setPixels(void*)` | none | `set_pixels(impl Into<Option<Vec<u8>>>)` | takes ownership |
| `SkBitmap::setPixelRef(sk_sp<SkPixelRef>, dx, dy)` | `set_pixel_ref(pixel_ref, offset)` | `set_pixel_ref(Option<PixelRef>, impl Into<IPoint>)` | |
| `SkBitmap::pixelRef`, `pixelRefOrigin`, `getSubset`, `getGenerationID`, `notifyPixelsChanged`, `isImmutable`, `setImmutable`, `readyToDraw` | `pixel_ref`, `pixel_ref_origin`, `get_subset`, `generation_id`, `notify_pixels_changed`, `is_immutable`, `set_immutable`, `is_ready_to_draw` | same | same as skia-safe |
| `SkBitmap::extractSubset(SkBitmap*, const SkIRect&)` | `extract_subset(&self, &mut Bitmap, rect) -> bool` | same | same as skia-safe |
| `SkBitmap::eraseColor`, `eraseARGB`, `erase`, `eraseArea` | `erase_color`, `erase_color_4f`, `erase_argb`, `erase`, `erase_4f` | same, plus `erase_area` (deprecated in Skia) | `&self`, as the C++ methods are `const` |
| `SkBitmap::ComputeIsOpaque` | `compute_is_opaque(&Bitmap)` | same | same as skia-safe |
| `SkBitmap::readPixels` (2 overloads), `writePixels`, `extractAlpha`, `asImage`, `makeShader` (4 overloads), `getBounds` (out-parameter forms) | | not ported | `SkConvertPixels`, `SkImage`, shaders, mask filters not ported; `bounds()` replaces `getBounds` |
| `SkReadPixelsRec`, `SkWritePixelsRec` | not exposed | `#[doc(hidden)] read_pixels_rec::ReadPixelsRec<'a>`, `write_pixels_rec::WritePixelsRec<'a>` with public fields `pixels: Option<&[u8]>` (`&mut` for read), `offset`, `row_bytes`, `info`, `x`, `y`; `trim` | the advanced `fPixels` pointer is the byte `offset` into the original slice (`rec.fPixels == pixels + n` is `rec.offset == n`) |
| **m44** | | | |
| `SkM44` | `M44` (`Clone`) | `M44` (`Copy + Clone`) | plain data, so `Copy` is a superset of skia-safe's API |
| `SkM44::operator==` | `PartialEq` via FFI | `PartialEq`, with the C++ `this == &other` shortcut (`std::ptr::eq`) | `m == m` is true even for NaN members, as in C++ |
| `SkM44::operator*` | `Mul` for `&M44` / `&V3` / `&V4` | same (`&M44 * &M44`, `&M44 * V3`, `&M44 * V4`) | same as skia-safe |
| `SkM44::preConcat(const SkMatrix&)` | not exposed | `M44::pre_concat_matrix` | overloads get distinct names |
| `SkM44::dump` | `dump` | not ported | needs `SkDebugf` |
| `SkM44::kUninitialized_Constructor` | none | not ported | no uninitialised values in safe Rust |
| `SkMatrixPriv::MapRect(const SkM44&, ...)` | not exposed | `matrix_priv::map_rect` | defined in `SkM44.cpp`, declared in `SkMatrixPriv.h` |
| `SkMatrixInvert.h` | not exposed | `matrix_invert::{invert_2x2_matrix, invert_3x3_matrix, invert_4x4_matrix}` | `outMatrix` may be null: `Option<&mut [scalar; N]>`; the determinant is returned as a `scalar` (a tiny double determinant underflows to 0, as in C++) |
| `SkV2` / `SkV3` / `SkV4` | `V2` / `V3` / `V4` | same | `as_array` / `as_mut_array` (unsafe in skia-safe) become `to_array` (by value); `SkV4::operator[]` is `Index` + `IndexMut`; the static `Dot` / `Cross` / `Normalize` are the same methods |
| **matrix** | | | |
| `SkMatrix` | `Matrix` (`Copy + Clone`) | `Matrix` (`Clone`, not `Copy`) | `fTypeMask` is `mutable` in C++ and is updated through `const` methods (`getType()`, ...). Rust stores it in an `AtomicU32` (relaxed), so `Matrix` is `Send + Sync` and `&self` getters keep the exact C++ caching behaviour, but it cannot be `Copy` |
| `SkMatrix::operator[]` (non-const) | `IndexMut<usize>` / `IndexMut<Member>` | same | dirties the type cache, as in C++ |
| `SkMatrix::kASkewY`, ... | `AffineMember` (+ `Index<AffineMember>`) | `AffineMember` (no `Index` impl) | skia-safe's `Index<AffineMember>` indexes the 3x3 array with the affine index, which is wrong; use `to_affine()` |
| `SkMatrix::get(int)`, `set(int, v)` | `IndexGet` / `IndexSet` traits | inherent `get` / `set` taking `impl Into<usize>` (`usize`, `Member`, `AffineMember`) | |
| `SkMatrix::setScale(sx, sy[, px, py])` and the other pivot overloads (`setRotate`, `setSkew`, `setSinCos`, `preScale`, ... `postSkew`) | `(.., pivot: impl Into<Option<Point>>)`; `None` is passed as pivot `(0, 0)` to the 4-arg C++ overload | same signature, but `None` calls the C++ overload *without* pivot | the two C++ overloads give different type masks (and NaN results), so skia-rust keeps them distinct |
| `SkMatrix::isSimilarity(tol)`, `preservesRightAngles(tol)` | no `tol` | `is_similarity` / `is_similarity_tol`, `preserves_right_angles` / `preserves_right_angles_tol` | overloads get distinct names |
| `SkMatrix::getMinMaxScales` | `min_max_scales() -> (scalar, scalar)` (ignores the `bool`) | `min_max_scales() -> Option<(scalar, scalar)>` | `None` when the C++ returns false |
| `SkMatrix::mapRadius` | `map_radius() -> Option<scalar>` (`None` with perspective) | `map_radius() -> scalar` | C++ also handles perspective |
| `SkMatrix::mapRect` | `map_rect() -> (Rect, bool)` | same | perspective goes through `PathBuilder::transform` (clipped to `w > 0` by `path_priv::perspective_clip`), as in C++ |
| `SkMatrix::mapRectScaleTranslate` | `map_rect_scale_translate() -> Option<Rect>` | same | same as skia-safe |
| `SkMatrix::mapPoints` (span overloads) | `map_points(dst, src)` (asserts `dst.len() >= src.len()`), `map_points_inplace` | same names; maps `min(dst.len(), src.len())` points | C++ `min_count`; same for `map_vectors`, `map_homogeneous_points`, `map_points_to_homogeneous` |
| `SkMatrix::RectToRect`, `MakeRectToRect`, `setRectToRect` (`SK_SUPPORT_LEGACY_MATRIX_RECTTORECT`) | deprecated `rect_to_rect -> Option` / `from_rect_to_rect` | `make_rect_to_rect -> Matrix` (identity on failure), `set_rect_to_rect -> bool` (resets on failure) | C++ semantics; use `rect_2_rect` / `rect_to_rect_or_identity` in new code |
| `SkMatrix::PolyToPoly`, `setPolyToPoly` | `poly_to_poly`, `from_poly_to_poly`, `set_poly_to_poly` | `poly_to_poly`, `set_poly_to_poly` | `from_poly_to_poly` is a duplicate of `poly_to_poly` |
| `SkMatrix::postIDiv` (private) | deprecated `post_idiv` | `matrix_priv::post_i_div` | `SkMatrixPriv::PostIDiv` |
| `SkMatrix::I`, `InvalidMatrix` | `Matrix::i()`, `invalid_matrix()`, `matrix::IDENTITY` (`const`) | same, `matrix::IDENTITY` is a `static` | `Matrix` holds an atomic, so a `const` would trip `declare_interior_mutable_const` |
| `SkMatrix::setRSXform` | `set_rsxform` | not ported | needs `SkRSXform` |
| `SkMatrix::dump` | `dump` | not ported | needs `SkString` / `SkDebugf` |
| `SkMatrix::getMapPtsProc`, `SkMatrixPriv::GetMapPtsProc` | not exposed | not ported | function-pointer table; `map_points` dispatches on the type mask |
| `SkMatrixPriv::MapPointsWithStride` (2 overloads), `MapHomogeneousPointsWithStride` | not exposed | not ported | walk raw memory by byte stride: not expressible without `unsafe` |
| `SkMatrixPriv` | not exposed | `#[doc(hidden)] matrix_priv` free functions | PORTING §3 |
| `SkMatrixPriv::WriteToMemory`, `ReadFromMemory` | not exposed | `matrix_priv::write_to_memory(&Matrix, Option<&mut [u8]>) -> usize`, `read_from_memory(&mut Matrix, &[u8]) -> usize` | `Option` for the null buffer; native-endian floats; panics if `buffer` is shorter than the returned size |
| `SkMatrixPriv::InverseMapRect` | not exposed | `matrix_priv::inverse_map_rect(&Matrix, &Rect) -> Option<Rect>` | `bool` + out-param becomes `Option` |
| `SkMatrixPriv::CheapEqual` | not exposed | `matrix_priv::cheap_equal` | bitwise compare of the nine members (`memcmp`) |
| `SkMatrixPriv::M44ColMajor` | not exposed | `matrix_priv::m44_col_major` (returns `[scalar; 16]` by value) | no pointer into the matrix |
| `SkMatrixPriv::NearlyAffine` | not exposed | `matrix_priv::nearly_affine(m, bounds, tolerance)` | no default argument: pass `SCALAR_NEARLY_ZERO` |
| `SkMatrixPriv::kMaxFlattenSize` | not exposed | `matrix_priv::MAX_FLATTEN_SIZE` | |
| `SkPathPriv::kW0PlaneDistance` | not exposed | `path_priv::W0_PLANE_DISTANCE` (re-exported as `matrix_priv::W0_PLANE_DISTANCE`) | PORTING §3 |
| `SkDecomposeUpper2x2` (`SkMatrixUtils.h`) | not exposed | `#[doc(hidden)] matrix_utils::decompose_upper_2x2(&Matrix, Option<&mut Point>, Option<&mut Point>, Option<&mut Point>) -> bool` | null out-params become `Option` |
| `SkTreatAsSprite` (`SkMatrixUtils.h`) | not exposed | not ported | needs `SkSamplingOptions` |
| **geometry** | | | |
| `SkGeometry.h` free functions (`SkChopCubicAt`, `SkEvalQuadAt`, ...) | not exposed | `geometry::{chop_cubic_at, eval_quad_at, ...}` | mechanical names; `src`/`dst` pointers become slices, a nullable `dst` becomes `Option<&mut [Point]>`, out-arrays stay `&mut [scalar; N]` parameters and counts are `usize` |
| `SkChopCubicAt` (3 overloads) | not exposed | `chop_cubic_at`, `chop_cubic_at_t0_t1`, `chop_cubic_at_ts` | overloads get distinct names; `tCount` is `t_values.len()` |
| `SkEvalQuadAt(src, t, SkPoint*, SkVector*)` | not exposed | `eval_quad_at_pos_tangent` (with `eval_quad_at`, `eval_quad_tangent_at`) | overloads get distinct names; the pointers are `Option<&mut _>` |
| `SkClassifyCubic(p, t, s, d)` | not exposed | `classify_cubic(p)`, `classify_cubic_with(p, Option<&mut [f64; 2]>, ..)` | default arguments split into two functions |
| `SkCubicType`, `SkCubicIsDegenerate`, `SkCubicTypeName` | not exposed | `CubicType` (+ `is_degenerate`, `name`) | enum with methods |
| `SkConic` | not exposed | `geometry::Conic { pts, w }` | public fields as in C++; `set` overloads are `set` / `set_points`; the constructors are `new` / `from_points` |
| `SkConic::evalAt(t, SkPoint*, SkVector*)`, `chopAt(t1, t2, SkConic*)` | not exposed | `eval_at_pos_tangent`, `chop_at_interval` | overloads get distinct names |
| `SkConic::findXExtrema/findYExtrema(SkScalar*)`, `computeAsQuadError(SkVector*)`, `computeTightBounds(SkRect*)`, `computeFastBounds(SkRect*)` | not exposed | return `Option<scalar>` / `Vector` / `Rect` | out-parameters become return values |
| `SkConic::TransformW`, `SkConic::BuildUnitArc` | not exposed | `Conic::transform_w(pts, w, &Matrix)`, `Conic::build_unit_arc(u_start, u_stop, dir, Option<&Matrix>, &mut [Conic; MAX_CONICS_FOR_ARC]) -> usize` | statics become associated functions; the nullable matrix is an `Option` |
| `SkAutoConicToQuads::computeQuads` (3 overloads) | not exposed | `AutoConicToQuads::{compute_quads, compute_quads_with_weight}` | the pointer and `SkSpan` overloads merge; the storage is a `Vec<Point>` instead of `AutoSTMalloc` |
| `SkQuadCoeff`, `SkConicCoeff`, `SkCubicCoeff` | not exposed | `geometry::{QuadCoeff, ConicCoeff, CubicCoeff}` with public `a, b, c, d` / `numer, denom` fields | public so `CubicMapTest` can use them |
| `skgpu::tess::FindCubicConvex180Chops(pts, T, bool*)` | not exposed | `tessellation::find_cubic_convex_180_chops(pts, &mut t, &mut are_cusps)` | lives in `skia-rust-core` until a GPU crate exists |
| `SkQuads`, `SkCubics` (classes of statics) | not exposed | modules `quads`, `cubics` (`roots_real`, `roots_valid_t`, `eval_at`, ...) | class with only statics becomes a module; `solution` out-arrays stay `&mut [f64; N]` |
| `SkBezierCubic`, `SkBezierQuad` | not exposed | `bezier_curves::{BezierCubic, BezierQuad}` (unit structs with associated functions) | `SkSpan<const float>` results are slices of the caller's storage array |
| `SkCubicClipper::ChopMonoAtY(pts, y, SkScalar*)` | not exposed | `CubicClipper::chop_mono_at_y(pts, y) -> Option<scalar>` | bool + out-param becomes `Option` |
| `SkCubicMap` | `CubicMap` | `cubic_map::CubicMap` | same API as skia-safe (`new`, `is_linear`, `compute_y_from_x`, `compute_from_t`) |
| **path** | | | |
| `SkPath` | `Path = Handle<SkPath>` | `Path { data: Arc<PathData>, fill_type, is_volatile }` (`Clone`, not `Copy`) | m156 paths are immutable and share `SkPathData`; a clone shares the data (the C++ copy); see `docs/design/path.md` |
| `SkPath::getGenerationID` | `generation_id() -> u32` | `generation_id() -> u64` | m156 returns `uint64_t` (the `SkPathData` unique id) |
| `SkPath::Rect(r, ft, dir, startIndex)`, `Rect(r, dir, startIndex)` | `rect_with_fill_type(r, ft, dir)`, `rect(r, dir)` (no start index) | adds `rect_with_start_index(r, dir, start)` and `rect_with_fill_type_and_start_index(r, ft, dir, start)` | the start index overloads are missing from skia-safe |
| `SkPath::RRect(bounds, rx, ry, dir)` | missing | `Path::rrect_xy(bounds, rx, ry, dir)` | overloads get distinct names |
| `SkPath::Raw` / `Make` / `Polygon` / `Line` / `Circle` / `Oval` | `raw`, `new_from`, `polygon`, `line`, `circle`, `oval`, `oval_with_start_index` | same | invalid or non-finite input gives a path with `is_finite() == false`, as in C++ |
| `SkPath::isOval(SkRect*)`, `isRRect(SkRRect*)`, `isLine(SkPoint[2])`, `isRect(SkRect*, bool*, SkPathDirection*)` | `Option` returns | same | out-params become `Option` |
| `SkPath::getPoint`, `getPoints`, `getVerbs` (deprecated) | `get_point`, `get_points`, `get_verbs` | same | `get_point` returns `None` out of range (C++ returns (0, 0)) |
| `SkPath::getLastPt` | `last_pt` | same | |
| `SkPath::interpolate(ending, w, SkPath*)`, `makeInterpolate` | `interpolate -> Option`, `interpolate_inplace` | same, plus `make_interpolate` | |
| `SkPath::writeToMemory(void*)`, `ReadFromMemory`, `serialize` | `serialize() -> Data`, `deserialize(&Data)` | `write_to_memory(Option<&mut [u8]>) -> usize`, `read_from_memory(&[u8]) -> (Option<Path>, usize)`, `serialize() -> Vec<u8>`, `deserialize(&[u8]) -> Option<Path>` | `SkData` is not ported; bytes match Skia (native-endian) |
| `SkPath::dump(SkWStream*, bool)`, `dump()`, `dumpHex()` | `dump_as_data(hex) -> Data`, `dump`, `dump_hex` | `dump_to_string(hex) -> String`, `dump`, `dump_hex` (stderr) | `SkWStream` / `SkData` are not ported |
| `SkPath::Iter::next(SkPoint[4])`, `next() -> optional<IterRec>` | `Iterator<Item = (Verb, Vec<Point>)>` | same `Iterator`, plus `next_verb(&mut [Point; 4]) -> Verb` and `next_rec() -> Option<IterRec>` | overloads get distinct names; `IterRec` is `PathIterRec` |
| `SkPath::Iter::conicWeight` | `conic_weight() -> Option<scalar>` | same | `None` until a conic is returned |
| `SkPath::RawIter` | deprecated `RawIter` | same, plus `next_verb` / `next_rec` | |
| `SkPath::RangeIter`, `SkPathPriv::Iterate` | not exposed | `path_priv::RangeIter` (`Iterator<Item = (PathVerb, &[Point], Option<scalar>)>`), `path_priv::iterate(&Path)`, `iterate_raw(verbs, points, weights)` | the points slice starts at the C++ "backset" (the current point); `iter == end()` is `is_done()` |
| `SkPathIter::Rec`, `SkPathContourIter::Rec` | `PathIterRec<'a>`, `PathContourIterRec<'a>` | `PathIterRec` (owns its <= 4 points; `points()`, `verb()`, `conic_weight()`), `PathContourIterRec<'a>` | an `Iterator` cannot lend the iterator's close-point storage |
| `SkPathBuilder` | `PathBuilder = RefHandle<SkPathBuilder>` | `PathBuilder` (`Clone`, `PartialEq`, `Default`) | plain struct |
| `SkPathBuilder::operator=(const SkPath&)` | missing | `PathBuilder::assign_path(&Path)` | operator overload |
| `SkPathBuilder::snapshot(const SkMatrix*)`, `detach(const SkMatrix*)` | `snapshot`, `snapshot_and_transform`, `detach`, `detach_and_transform` | same | |
| `SkPathBuilder::snapshotData`, `detachData` | missing | `snapshot_data`, `detach_data -> Option<Arc<PathData>>` | |
| `SkPathBuilder::incReserve(int, int, int)` | `inc_reserve(usize, usize, usize)` | `inc_reserve(i32, i32, i32)` | C++ `int` (negative counts are ignored; `PathTest` passes `0xffffffff`) |
| `SkPathBuilder::arcTo` (3 overloads) | `arc_to`, `arc_to_tangent`, `arc_to_radius` | same | same as skia-safe |
| `SkPathBuilder::addPath(src, dx, dy, mode)`, `addPath(src, matrix, mode)` | `add_path`, `add_path_with_offset`, `add_path_with_transform` (returns `()`) | same; `add_path_with_transform` returns `&mut Self` | builder-style return |
| `SkPathBuilder::addRaw(const SkPathRaw&, Reserve)` | missing | `add_raw(&PathRaw, Reserve)` | |
| `SkPathBuilder::dumpToString(DumpFormat)`, `dump(DumpFormat)` | `dump_to_string`, `dump` | same | `SkString` is `String` |
| `SkPathBuilder::setLastPt`, `setLastPoint`, `setPoint` | `set_last_pt`, `set_last_point`, `set_point` | same | |
| `SkPathData` (`sk_sp`, nullable) | missing | `path_data::PathData`, factories return `Option<Arc<PathData>>` | `nullptr` becomes `None` |
| `SkPathData::Rect/Oval/RRect` default arguments | missing | `rect(r, dir, start)` + `rect_default(r)`, `oval` + `oval_default`, `rrect` + `rrect_default(rr, dir)` | default arguments split into two functions |
| `SkPathData::MakeTransform(const SkPathRaw&, ...)`, `makeTransform`, `makeOffset` | missing | `make_transform_raw(&PathRaw, &Matrix)`, `make_transform(self: &Arc<Self>, &Matrix)`, `make_offset` | overloads get distinct names; identity returns the same `Arc` |
| `SkPathData::addGenIDChangeListener`, `genIDChangeListenerCount`, `SkPathPriv::AddGenIDChangeListener` | missing | not ported | `SkIDChangeListener` is not ported |
| `SkPathRaw` | missing | `path_raw::PathRaw<'a>` (public fields `points`, `verbs`, `conics`, `bounds`, `fill_type`, `convexity`, `segment_mask`) | borrowed slices |
| `SkPathRawShapes::{Rect, Oval, RRect, Triangle}` | missing | `path_raw_shapes::{Rect, Oval, RRect}` (`Shape<N>` with `.raw()`), `path_raw_shapes::triangle(pts, bounds) -> PathRaw` | C++ inheritance from `SkPathRaw` becomes a `raw()` view |
| `SkPathConvexity`, `SkPathFirstDirection`, `SkResolveConvexity` (`SkPathEnums.h`) | missing | `path_enums::{PathConvexity, PathFirstDirection, ResolveConvexity}` with the free helpers as methods | `#[doc(hidden)]` private enums |
| `SkPathRectInfo`, `SkPathOvalInfo`, `SkPathRRectInfo`, `SkPathIsAType`, `SkPathIsAData` (`SkPathRef.h`) | missing | `path_ref::*` | `#[doc(hidden)]`; fields lose the `f` prefix |
| `SkPathPriv` | not exposed | `#[doc(hidden)] path_priv` free functions | PORTING §3; `Raw(const SkPathBuilder&)` is `raw_builder`, `IsNestedFillRects` returns `Option<([Rect; 2], [PathDirection; 2])>`, `PerspectiveClip` returns `Option<Path>`, `ComputeFirstDirection(const SkPath&)` is `compute_first_direction_path` |
| `SkPathPriv::CreateDrawArcPath`, `DrawArcIsConvex` | not exposed | not ported | need `SkArc` |
| `SkPathEdgeIter` | not exposed | `path_priv::PathEdgeIter` (`next() -> Option<EdgeResult>`) | the result carries a copy of the edge's points |
| `SkEdgeClipper`, `SkLineClipper` | not exposed | `#[doc(hidden)] edge_clipper::EdgeClipper` (`clip_path` takes a closure), `line_clipper::{clip_line, intersect_line}` | function pointer + `void*` context becomes a closure |
| `SkPathUtils` (`FillPathWithPaint`) | `path_utils::fill_path_with_paint` | not ported | needs `SkPaint` and the stroker |
| `SkContourMeasure` | `ContourMeasure = RCHandle<..>` | `ContourMeasure` (`Clone`, shares an `Arc`) | |
| `SkContourMeasure::getPosTan(d, SkPoint*, SkVector*)` | `pos_tan(d) -> Option<(Point, Vector)>` | same, plus `get_pos_tan(d, Option<&mut Point>, Option<&mut Vector>) -> bool` | the nullable out-params of the C++ |
| `SkContourMeasure::begin()/end()` (`ForwardVerbIterator`) | `verbs() -> ForwardVerbIterator` | same (`Iterator<Item = VerbMeasure>`) | |
| `SkContourMeasureIter::next() -> sk_sp` | `Iterator` | same | |
| `SkPathMeasure::setPath(const SkPath*, bool)` | `set_path(&Path, bool)` | same | |
| `SkPathMeasure::getLength`, `getPosTan` | `length(&mut self)`, `pos_tan(&mut self, ..)` | `length(&self)`, `pos_tan(&self, ..)`, plus `get_pos_tan(d, Option<&mut Point>, Option<&mut Vector>)` | no interior state changes |
| `SkPathMeasurePriv::CountSegments` | not exposed | `path_measure::path_measure_priv::count_segments` | PORTING §3 |
| `SkParsePath::FromSVGString`, `ToSVGString` | `utils::parse_path::{from_svg, to_svg, to_svg_with_encoding}`, `Path::from_svg` / `to_svg` | same | lives in `skia_rust_core::utils::parse_path` |
| `SkParse` | not exposed | `utils::parse::{find_scalar, find_scalars, find_s32, find_hex, find_bool, find_list, count}` | strings are byte slices and the returned pointer is an index; `strtod` is reimplemented (`utils::parse::strtod`) |
| `SkAppendScalar`, `SkStrAppendScalar` | not exposed | `#[doc(hidden)] string_utils::{append_scalar, str_append_scalar, format_g}` | `SkString` is `String`; `format_g` reproduces `printf("%.*g")` |
| **point** | | | |
| `SkIVector` | `pub use IPoint as IVector` | `pub type IVector = IPoint` | a type alias is equivalent |
| `SkIPoint` `+ - += -=` | plain `+`/`-` (panics on overflow) | saturating (`Sk32_sat_add` / `Sk32_sat_sub`) | Skia's semantics (PORTING §3) |
| `SkIPoint::operator-()` | `-x` | `wrapping_neg` | C++ negation wraps in practice; no debug panic |
| `SkPointPriv` | not exposed | `#[doc(hidden)] point::point_priv` free functions | PORTING §3 |
| `SkPointPriv::AsScalars` | not exposed | `point_priv::as_scalars(&Point) -> [scalar; 2]` | no `unsafe`: returns a copy, not a pointer |
| `SkPointPriv::DistanceToLine*BetweenSqd(..., Side*)` | not exposed | `point_priv::distance_to_line_between_sqd(.., Option<&mut Side>)` | optional out-param |
| `SkPointPriv::EqualsWithinTolerance` (2 overloads) | not exposed | `equals_within_tolerance`, `equals_within_tolerance_tol` | overloads get distinct names |
| `SkPointPriv::Negate` / `RotateCCW` / `RotateCW` | not exposed | `negate`, `rotate_ccw`, `rotate_ccw_in_place`, `rotate_cw`, `rotate_cw_in_place` | `src`/`dst` overloads split into by-value and in-place |
| `SkPointPriv::SetLengthFast` | not exposed | `point_priv::set_length_fast` | m156 shares the double-precision path with `setLength` |
| `SkPointPriv::SetRectFan`, `SetRectTriStrip` | not exposed | not ported | write through a byte `stride` into raw vertex memory; not expressible without `unsafe` |
| **point3** | | | |
| `SkPoint3::makeScale` | `Point3::scaled` | `Point3::scaled` | same as skia-safe |
| **rect** | | | |
| `SkIRect::asInt32s`, `SkRect::asScalars` | `&[i32]` / `&[f32; 4]` into the struct | `[i32; 4]` / `[scalar; 4]` by value | no `unsafe` |
| `SkIRect::inset` / `makeInset` | via `with_outset(-delta)` | direct `sat_add`/`sat_sub`, as in C++ | differs from skia-safe only at saturation |
| `SkIRect::MakePtSize` | non-saturating `From<(IPoint, ISize)>` | saturating `from_pt_size` / `From<(IPoint, ISize)>` | Skia's semantics |
| `SkIRect::offsetTo` | `with_offset_to` returns `(pin(..), pin(..), new_x, new_y)` (edges swapped) | `(new_x, new_y, pin(..), pin(..))` | skia-safe's ordering looks like a bug |
| `SkIRect::topLeft` | missing | `IRect::top_left` | mechanical name |
| `SkRect::Bounds` | via FFI | `Rect::bounds`, `Rect::bounds_or_empty`, `Rect::from_bounds` | 64-bit variant of the C++ (both variants compute the same numerics); `std::min/max` NaN semantics are kept for `join`, `intersect`, `sorted`, `set_bounds2` |
| `SkRect::centerX/centerY` | `left * 0.5 + right * 0.5` | `float_midpoint` (`sk_float_midpoint`, double math) | Skia's m156 formula |
| `SkRect::contains`, `SkIRect::contains` overloads | `Contains<T>` trait (crate root) | `rect::Contains<T>` | same shape; trait lives in `rect` |
| `SkRect::dump`, `dumpToString`, `dumpHex` | `dump`, `dump_to_string`, `dump_hex` | not ported yet | need `SkString` / `SkAppendScalar` |
| `SkRect::offsetTo` | `with_offset_to` returns `(x, y, x - left, y - top)` | `offset_to` / `with_offset_to` mirror C++ (`right += newX - left`, ...) | skia-safe's version looks like a bug |
| `SkRect::roundOut(SkIRect*)`, `roundOut(SkRect*)` | `RoundOut<R>` trait | `rect::RoundOut<R>` | same as skia-safe |
| `SkRect::set(SkPoint, SkPoint)`, `intersect(a, b)`, `join(a, b)` | `set_bounds2`, `intersect2`, `join2` | same | same as skia-safe |
| `SkRect::toQuad`, `copyToQuad` | `to_quad`, `copy_to_quad` | `to_quad(dir: impl Into<Option<PathDirection>>) -> [Point; 4]`, `copy_to_quad(&mut [Point], dir)` | `None` is the C++ default `kCW`; the deprecated `toQuad(SkPoint[4])` is `copy_to_quad` |
| `SkRectPriv` | not exposed | `#[doc(hidden)] rect::rect_priv` free functions | PORTING §3 |
| `SkRectPriv::FitsInFixed` | not exposed | `rect_priv::fits_in_fixed_rect` | avoids clashing with `math_priv::fits_in_fixed` |
| `SkRectPriv::QuadContainsRect` (2 overloads), `QuadContainsRectMask` | not exposed | `rect_priv::quad_contains_rect` (`&Matrix`, `&IRect`), `quad_contains_rect_m44`, `quad_contains_rect_mask` (returns `vx::Int4`) | overloads get distinct names; the C++ default `tol = 0.f` is an explicit `tol` argument |
| `SkRectPriv::Subtract` (4 overloads) | not exposed | `subtract`, `subtract_irect` (bool + out-param), `subtract_diff`, `subtract_irect_diff` | overloads get distinct names |
| **rrect** | | | |
| `SkRRect::Type`, `SkRRect::Corner` | `rrect::Type`, `rrect::Corner` (`Empty`, `Rect`, ..; `UpperLeft`, ..) | same; `Type::LAST` for `kLastType` | same as skia-safe; `Corner` is `repr(usize)` so `corner as usize` indexes the radii |
| `SkRRect::getType` / `type` | `get_type` | `get_type` (`debug_assert!(is_valid())` as in C++) | `type` is a Rust keyword; one name |
| `SkRRect::getSimpleRadii` | `simple_radii` | `simple_radii` | same as skia-safe |
| `SkRRect::radii()` (span) | `radii_ref -> &[Vector; 4]` | `radii_ref` | same as skia-safe |
| `SkRRect::getBounds` / `rect` | `bounds`, `rect` | same | same as skia-safe |
| `SkRRect::Make*` | `new_rect`, `new_oval`, `new_rect_xy`, `new_rect_radii`, `new_nine_patch`, `new_empty` | same | same as skia-safe |
| `SkRRect::inset/outset(dx, dy[, dst])` | `inset(delta)`, `with_inset(delta)`, `outset`, `with_outset` | same | same as skia-safe; the `dst` overload is `with_*` |
| `SkRRect::makeOffset` | `with_offset` | `with_offset` | same as skia-safe |
| `SkRRect::contains(SkPoint)` / `contains(SkRect)` | `contains_point`, `contains` | same | overloads get distinct names |
| `SkRRect::writeToMemory` | `write_to_memory(&mut Vec<u8>)` | same; native-endian floats, replaces the vector's contents | same as skia-safe |
| `SkRRect::readFromMemory` | `read_from_memory(&[u8]) -> usize` | same | same as skia-safe |
| `SkRRect::kSizeInMemory` | `SIZE_IN_MEMORY` | `SIZE_IN_MEMORY` (`12 * 4`) | same as skia-safe |
| `SkRRect::transform` (both overloads) | `transform(&Matrix) -> Option<RRect>` | same (the `bool transform(const SkMatrix&, SkRRect*)` overload is not ported) | the non-legacy m156 implementation (`SK_SUPPORT_LEGACY_RRECT_TRANSFORM` is not defined) |
| `SkRRect::dump`, `dumpToString`, `dumpHex` | `dump`, `dump_to_string`, `dump_hex` | not ported yet | need `SkString` / `SkAppendScalar` |
| `SkRRect::operator==` / `!=` | `PartialEq` | `PartialEq` comparing the rect and the 8 radii as floats (not the type) | Skia's semantics |
| `SkRRectPriv` | not exposed | `#[doc(hidden)] rrect::rrect_priv` free functions | PORTING §3 |
| `SkRRectPriv::ReadFromBuffer`, `WriteToBuffer` | not exposed | not ported yet | need `SkRBuffer` / `SkWBuffer` |
| `SkRRectPriv::{IsNearlySimpleCircular, AllCornersCircular, AllCornersRelativelyCircular, IsRelativelyCircular}` default `tolerance` | not exposed | `tolerance: impl Into<Option<scalar>>`, `None` = `SK_ScalarNearlyZero` | C++ default argument |
| `SkScaleToSides::AdjustRadii`, `SkFloatingPoint<float, 4>::AlmostEquals` | not exposed | private helpers in `rrect.rs` | only used by `SkRRect` so far; move out if another module needs them |
| **region** | | | |
| `SkRegion::RunHead` (ref-counted run array) | internal | private `Arc<RunHead>` (`Vec<i32>` runs), rebuilt on write | safe replacement for the ref-counted malloc block; run array contents and `writeToMemory` bytes match Skia |
| `SkRegion::Op` | `RegionOp` | `region::Op` (+ `pub type RegionOp = Op`) | both spellings available |
| `SkRegion::translate(dx, dy, dst)` | not exposed | `Region::translate_to(dx, dy, &mut dst)` | mechanical name; `translate(d)` / `translated(d)` as in skia-safe |
| `SkRegion::op(rgna, rgnb, op)` | not exposed | `Region::op_region_region(a, b, op)` | the other `op` overloads keep skia-safe's `op_rect`, `op_region`, `op_rect_region`, `op_region_rect` |
| `SkRegion::writeToMemory(nullptr)` | not exposed | `Region::write_to_memory_size()` | size query split from the write; `write_to_memory(&mut Vec<u8>)` as in skia-safe (native-endian `i32`s) |
| `SkRegion::Iterator::reset` | `reset(self, &Region) -> Iterator` | same (consumes `self`) | the iterator borrows the region, so a reset may change its lifetime |
| `SkRegion::Spanerator::next(int*, int*)` | `Iterator<Item = (i32, i32)>` | same | out-parameters become a tuple |
| `QuickReject` | crate-root trait | `region::QuickReject` | `skia-safe` defines it in `core.rs`; lives in `region` until another port needs it elsewhere |
| `SkRegionPriv::VisitSpans`, `Validate`, `kRunTypeSentinel`, `SkRegionValueIsSentinel` | not exposed | `#[doc(hidden)] region::region_priv::{visit_spans, validate, RUN_TYPE_SENTINEL, region_value_is_sentinel}` | PORTING §3 |
| `SkRegion::setPath`, `addBoundaryPath`, `getBoundaryPath` (`SkRegion_path.cpp`) | `set_path`, `add_boundary_path`, `boundary_path` | not ported yet | need scan conversion (`SkScan`) |
| `SkRegion::toString` (Android framework only) | not exposed | not ported | `SK_BUILD_FOR_ANDROID_FRAMEWORK` only |
| **skcms** | | | |
| `skcms_Matrix3x3`, `skcms_Matrix3x4` | missing | `Matrix3x3`, `Matrix3x4` (`vals`) | drop prefix; `bit_eq` is the C++ `memcmp` |
| `skcms_Matrix3x3_invert`, `skcms_Matrix3x3_concat` | missing | `Matrix3x3::invert() -> Option<Matrix3x3>`, `Matrix3x3::concat`; free-function forms `matrix3x3_invert`, `matrix3x3_concat` | out-param becomes `Option` |
| `skcms_TransferFunction` | `ColorSpaceTransferFn` | `TransferFunction` (`g, a, b, c, d, e, f`) | drop prefix |
| `skcms_TransferFunction_eval`, `_invert` | missing | `TransferFunction::eval`, `invert() -> Option<TransferFunction>` | out-param becomes `Option` |
| `skcms_TransferFunction_getType`, `_isSRGBish`, `_isPQish`, `_isHLGish`, `_isPQ`, `_isHLG` | missing | `tf_type() -> TfType`, `is_srgbish`, `is_pqish`, `is_hlgish`, `is_pq`, `is_hlg` | mechanical names |
| `skcms_TransferFunction_makePQish`, `makeScaledHLGish`, `makeHLGish`, `makePQ`, `makeHLG` | missing | `TransferFunction::make_pqish`, `make_scaled_hlgish`, `make_hlgish`, `make_pq`, `make_hlg` returning the function | out-param becomes a return value |
| `skcms_TFType` (`skcms_TFType_sRGBish`, ...) | missing | `TfType::{Invalid, SRGBish, PQish, HLGish, HLGinvish, PQ, HLG}` | enum without prefix |
| `skcms_Curve` (a union) | missing | `Curve::{Parametric(TransferFunction), Table8 { entries, table }, Table16 { entries, table }}`; `table_entries()` | tagged enum instead of a union with raw table pointers |
| `skcms_A2B`, `skcms_B2A`, `skcms_CICP`, `skcms_HAGC` | missing | `A2B`, `B2A`, `Cicp`, `Hagc` | `const uint8_t*` table/grid pointers become `Option<ByteView>` (shared buffer + offset) |
| `skcms_ICCProfile` | missing | `IccProfile` (`buffer: Option<Arc<[u8]>>`, `to_xyzd50`, `has_to_xyzd50`, `a2b`, `has_a2b`, `cicp`, `has_cicp`, ...) | the profile owns (shares) its bytes instead of borrowing them; `bit_eq` is the C++ `memcmp` |
| `skcms_Init`, `skcms_SetTransferFunction`, `skcms_SetXYZD50` | missing | `IccProfile::new`, `set_transfer_function`, `set_xyzd50` | mechanical names |
| `skcms_sRGB_profile`, `skcms_XYZD50_profile` | missing | `srgb_profile()`, `xyzd50_profile() -> &'static IccProfile` | drop prefix |
| `skcms_sRGB_TransferFunction`, `_sRGB_Inverse_TransferFunction`, `_Identity_TransferFunction` | missing | `srgb_transfer_function()`, `srgb_inverse_transfer_function()`, `identity_transfer_function()` | drop prefix |
| `skcms_Parse`, `skcms_ParseWithA2BPriority` | missing | `parse(&[u8]) -> Option<IccProfile>`, `parse_with_a2b_priority(&[u8], &[i32])` | the bytes are copied into the profile; failure is `None` |
| `skcms_ApproximatelyEqualProfiles`, `skcms_AreApproximateInverses`, `skcms_TRCs_AreApproximateInverse` | missing | `approximately_equal_profiles`, `are_approximate_inverses`, `trcs_are_approximate_inverse` | drop prefix |
| `skcms_ApproximateCurve` | missing | `approximate_curve(&Curve) -> Option<(TransferFunction, f32)>` | out-params become the `Option` payload |
| `skcms_MaxRoundtripError`, `skcms_GetTagByIndex`, `skcms_GetTagBySignature`, `skcms_252_random_bytes`, `powf_` (`skcms_internals.h`) | missing | `max_roundtrip_error`, `get_tag_by_index`, `get_tag_by_signature` (return `Option<IccTag>`), `RANDOM_BYTES_252`, `powf_` | exposed for tests |
| `skcms_GetCHAD`, `skcms_GetWTPT`, `skcms_GetInputChannelCount` | missing | `get_chad -> Option<Matrix3x3>`, `get_wtpt -> Option<[f32; 3]>`, `get_input_channel_count` | out-params become `Option` |
| `skcms_Signature_*` | missing | `signature::{RGB, XYZ, CMYK, GRAY, LAB, ...}` | constants |
| `skcms_PixelFormat_*` | missing | `PixelFormat::{A8, Rgb888, Rgba8888, RgbaFfff, ...}` (`Swap`/BGR variants are the odd values) | UpperCamelCase variants |
| `skcms_AlphaFormat_*` | missing | `AlphaFormat::{Opaque, Unpremul, PremulAsEncoded}` | enum without prefix |
| `skcms_Transform` | missing | `transform(src: &[u8], .., dst: &mut [u8], .., npixels) -> bool`; `transform_in_place` for `dst == src` | slices instead of aliasing pointers; `None` profile is sRGB; buffers are bounds-checked |
| `skcms_MakeUsableAsDestination`, `skcms_MakeUsableAsDestinationWithSingleCurve` | missing | `make_usable_as_destination(&mut IccProfile) -> bool`, `make_usable_as_destination_with_single_curve` | drop prefix |
| `skcms_AdaptToXYZD50`, `skcms_PrimariesToXYZD50` | missing | `adapt_to_xyzd50 -> Option<Matrix3x3>`, `primaries_to_xyzd50 -> Option<Matrix3x3>` | out-param becomes `Option` |
| `skcms_DisableRuntimeCPUDetection` | missing | `disable_runtime_cpu_detection()` | a no-op: only the portable baseline exists so far |
| `Transform_inl.h` HSW / SKX variants, NEON paths | missing | not ported yet | the portable scalar (`N == 1`) baseline only; per-tier kernels come later through `skia-rust-simd` |
