// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkPerlinNoiseShaderImpl.{h,cpp}, include/effects/SkPerlinNoiseShader.h

//! `SkPerlinNoiseShader`: Perlin fractal noise and turbulence (`SkShaders::MakeFractalNoise`,
//! `SkShaders::MakeTurbulence`).
//!
//! The lattice and gradient tables are generated from the seed with the SVG `feTurbulence`
//! generator (`random`), exactly as Skia does; the per-pixel work is the raster pipeline stage
//! `perlin_noise`.

use std::sync::OnceLock;

use skia_rust_core::effect_priv::StageRec;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::raster_pipeline::Stage;
use skia_rust_core::raster_pipeline::contexts::{PerlinNoiseCtx, PerlinNoiseShaderType};
use skia_rust_core::scalar::{
    scalar, scalar_ceil_to_scalar, scalar_floor_to_scalar, scalar_round_to_int, scalar_trunc_to_int,
};
use skia_rust_core::shaders::{MatrixRec, ShaderBase, ShaderType};
use skia_rust_core::size::ISize;

/// `SkPerlinNoiseShader::kBlockSize`.
const BLOCK_SIZE: usize = 256;
/// `kBlockSize` as an `int`.
const BLOCK_SIZE_I32: i32 = 256;
/// Half of the largest possible value for a 16 bit unsigned int (`kHalfMax16bits`).
const HALF_MAX_16BITS: scalar = 32767.5;
/// `1.0 / SkIntToScalar(kBlockSize)` (`kInvBlockSizef`).
const INV_BLOCK_SIZE: scalar = 1.0 / 256.0;
/// `SkPerlinNoiseShader::kPerlinNoise`.
const PERLIN_NOISE: i32 = 4096;
/// `SkPerlinNoiseShader::kRandMaximum` (`SK_MaxS32`, 2**31 - 1).
const RAND_MAXIMUM: i32 = i32::MAX;
/// `SkPerlinNoiseShader::kMaxOctaves`.
pub const MAX_OCTAVES: i32 = 255;
/// `fNoise[4][kBlockSize][2]`, flattened.
const NOISE_LEN: usize = 4 * BLOCK_SIZE * 2;

/// Index of `fNoise[channel][i][j]` in the flattened table.
fn noise_index(channel: usize, i: usize, j: usize) -> usize {
    (channel * BLOCK_SIZE + i) * 2 + j
}

/// `SkPerlinNoiseShader::PaintingData::random`: one step of the SVG `feTurbulence` generator
/// (Park-Miller, `m = 2**31 - 1`, `a = 7**5`).
// Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L109-L123 (chrome/m156)
fn random(seed: &mut i32) -> i32 {
    // See https://www.w3.org/TR/SVG11/filters.html#feTurbulenceElement
    const RAND_AMPLITUDE: i32 = 16807; // 7**5; primitive root of m
    const RAND_Q: i32 = 127_773; // m / a
    const RAND_R: i32 = 2836; // m % a
    let mut result = RAND_AMPLITUDE * (*seed % RAND_Q) - RAND_R * (*seed / RAND_Q);
    if result <= 0 {
        result += RAND_MAXIMUM;
    }
    *seed = result;
    result
}

/// `SkPerlinNoiseShader::StitchData`: how much to subtract to wrap for stitching. Skia's
/// `fWrapX`/`fWrapY` only take part in the shader's equality, which this port does not need.
// Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L40-L53 (chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StitchData {
    /// How much to subtract to wrap for stitching.
    width: i32,
    height: i32,
}

impl StitchData {
    // Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L46-L50 (chrome/m156)
    fn new(w: scalar, h: scalar) -> StitchData {
        StitchData {
            width: scalar_round_to_int(w).min(RAND_MAXIMUM - PERLIN_NOISE),
            height: scalar_round_to_int(h).min(RAND_MAXIMUM - PERLIN_NOISE),
        }
    }
}

/// `SkPerlinNoiseShader::PaintingData`: the frequency, the stitch data and the lattice and
/// gradient tables generated from the seed.
// Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L56-L63 (chrome/m156)
#[derive(Debug)]
struct PaintingData {
    /// `fLatticeSelector[kBlockSize]`.
    lattice_selector: [u8; BLOCK_SIZE],
    /// `fNoise[4][kBlockSize][2]`, flattened (see [`noise_index`]).
    noise: [u16; NOISE_LEN],
    base_frequency: Point,
    stitch_data_init: StitchData,
}

impl PaintingData {
    // Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L64-L76 (chrome/m156)
    fn new(
        tile_size: ISize,
        seed: scalar,
        base_frequency_x: scalar,
        base_frequency_y: scalar,
    ) -> PaintingData {
        let mut data = PaintingData {
            lattice_selector: [0; BLOCK_SIZE],
            noise: [0; NOISE_LEN],
            base_frequency: Point::new(base_frequency_x, base_frequency_y),
            stitch_data_init: StitchData::default(),
        };
        data.init(seed);
        if !tile_size.is_empty() {
            data.stitch(tile_size);
        }
        data
    }

    // Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L125-L189 (chrome/m156)
    // The casts mirror the C++ int/byte/float conversions of the table generation: every value
    // cast is in range (a lattice index below 256, a noise value below 512 or 65536).
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    fn init(&mut self, seed: scalar) {
        // According to the SVG spec, we must truncate (not round) the seed value.
        let mut fseed = scalar_trunc_to_int(seed);
        // The seed value clamp to the range [1, kRandMaximum - 1].
        if fseed <= 0 {
            fseed = -(fseed % (RAND_MAXIMUM - 1)) + 1;
        }
        if fseed > RAND_MAXIMUM - 1 {
            fseed = RAND_MAXIMUM - 1;
        }

        for channel in 0..4 {
            for i in 0..BLOCK_SIZE {
                self.lattice_selector[i] = i as u8;
                for j in 0..2 {
                    let value = random(&mut fseed) % (2 * BLOCK_SIZE_I32);
                    self.noise[noise_index(channel, i, j)] = value as u16;
                }
            }
        }

        for i in (1..BLOCK_SIZE).rev() {
            let k = self.lattice_selector[i];
            // `random() % kBlockSize`, with a positive random value.
            let j = (random(&mut fseed) % BLOCK_SIZE_I32) as usize;
            self.lattice_selector[i] = self.lattice_selector[j];
            self.lattice_selector[j] = k;
        }

        // Perform the permutations now: `fNoise[c][i][j] = noise[c][fLatticeSelector[i]][j]`.
        let noise = self.noise;
        for channel in 0..4 {
            for i in 0..BLOCK_SIZE {
                let src = usize::from(self.lattice_selector[i]);
                for j in 0..2 {
                    self.noise[noise_index(channel, i, j)] = noise[noise_index(channel, src, j)];
                }
            }
        }

        // Compute gradients from permuted noise data
        for channel in 0..4 {
            for i in 0..BLOCK_SIZE {
                // `(fNoise - kBlockSize) * kInvBlockSizef`: an int converted to a float.
                let nx = i32::from(self.noise[noise_index(channel, i, 0)]) - BLOCK_SIZE_I32;
                let ny = i32::from(self.noise[noise_index(channel, i, 1)]) - BLOCK_SIZE_I32;
                let mut gradient =
                    Point::new(nx as scalar * INV_BLOCK_SIZE, ny as scalar * INV_BLOCK_SIZE);
                gradient.normalize();
                // Put the normalized gradient back into the noise data (in [0, 65535]).
                self.noise[noise_index(channel, i, 0)] =
                    scalar_round_to_int((gradient.x + 1.0) * HALF_MAX_16BITS) as u16;
                self.noise[noise_index(channel, i, 1)] =
                    scalar_round_to_int((gradient.y + 1.0) * HALF_MAX_16BITS) as u16;
            }
        }
    }

    // Port of: src/shaders/SkPerlinNoiseShaderImpl.h#L191-L221 (chrome/m156)
    // `SkIntToScalar` of tile sizes, which are far below 2**24 in any real use.
    #[allow(clippy::cast_precision_loss)]
    fn stitch(&mut self, tile_size: ISize) {
        let tile_width = tile_size.width as scalar;
        let tile_height = tile_size.height as scalar;

        // When stitching tiled turbulence, the frequencies must be adjusted
        // so that the tile borders will be continuous.
        // `if (fBaseFrequency.fX)`: a float is true unless it is zero (NaN is true).
        #[allow(clippy::float_cmp)] // C truthiness of a float
        if self.base_frequency.x != 0.0 {
            let base = self.base_frequency.x;
            let low_frequency = scalar_floor_to_scalar(tile_width * base) / tile_width;
            let high_frequency = scalar_ceil_to_scalar(tile_width * base) / tile_width;
            // BaseFrequency should be non-negative according to the standard.
            // lowFrequencx can be 0 if fBaseFrequency.fX is very small.
            self.base_frequency.x = if base / low_frequency < high_frequency / base {
                low_frequency
            } else {
                high_frequency
            };
        }
        #[allow(clippy::float_cmp)] // C truthiness of a float
        if self.base_frequency.y != 0.0 {
            let base = self.base_frequency.y;
            let low_frequency = scalar_floor_to_scalar(tile_height * base) / tile_height;
            let high_frequency = scalar_ceil_to_scalar(tile_height * base) / tile_height;
            // lowFrequency can be 0 if fBaseFrequency.fY is very small.
            self.base_frequency.y = if base / low_frequency < high_frequency / base {
                low_frequency
            } else {
                high_frequency
            };
        }

        self.stitch_data_init = StitchData::new(
            tile_width * self.base_frequency.x,
            tile_height * self.base_frequency.y,
        );
    }
}

/// `SkPerlinNoiseShader`: fractal noise or turbulence (`SkShaders::MakeFractalNoise` and
/// `SkShaders::MakeTurbulence`).
#[doc(alias = "SkPerlinNoiseShader")]
#[derive(Debug)]
pub struct PerlinNoiseShader {
    noise_type: PerlinNoiseShaderType,
    base_frequency_x: scalar,
    base_frequency_y: scalar,
    num_octaves: i32,
    seed: scalar,
    tile_size: ISize,
    stitch_tiles: bool,
    /// The tables, built on first use (`fInitPaintingDataOnce`).
    painting_data: OnceLock<PaintingData>,
}

impl PerlinNoiseShader {
    /// `SkPerlinNoiseShader(type, baseFrequencyX, baseFrequencyY, numOctaves, seed, tileSize)`;
    /// `tile_size` is `None` for no stitching.
    // Port of: src/shaders/SkPerlinNoiseShaderImpl.cpp#L26-L47 (chrome/m156)
    #[must_use]
    pub fn new(
        noise_type: PerlinNoiseShaderType,
        base_frequency_x: scalar,
        base_frequency_y: scalar,
        num_octaves: i32,
        seed: scalar,
        tile_size: Option<ISize>,
    ) -> PerlinNoiseShader {
        let tile_size = tile_size.unwrap_or_default();
        PerlinNoiseShader {
            noise_type,
            base_frequency_x,
            base_frequency_y,
            // [0,255] octaves allowed
            num_octaves: num_octaves.min(MAX_OCTAVES),
            seed,
            tile_size,
            stitch_tiles: !tile_size.is_empty(),
            painting_data: OnceLock::new(),
        }
    }

    /// The noise type (`noiseType`).
    #[must_use]
    pub fn noise_type(&self) -> PerlinNoiseShaderType {
        self.noise_type
    }

    /// The number of octaves (`numOctaves`).
    #[must_use]
    pub fn num_octaves(&self) -> i32 {
        self.num_octaves
    }

    /// Whether the noise stitches tiles (`stitchTiles`).
    #[must_use]
    pub fn stitch_tiles(&self) -> bool {
        self.stitch_tiles
    }

    /// The tile size (`tileSize`), empty when not stitching.
    #[must_use]
    pub fn tile_size(&self) -> ISize {
        self.tile_size
    }

    // Port of: src/shaders/SkPerlinNoiseShaderImpl.cpp#L83-L107 (chrome/m156) (painting data)
    fn painting_data(&self) -> &PaintingData {
        self.painting_data.get_or_init(|| {
            PaintingData::new(
                self.tile_size,
                self.seed,
                self.base_frequency_x,
                self.base_frequency_y,
            )
        })
    }
}

impl ShaderBase for PerlinNoiseShader {
    fn is_opaque(&self) -> bool {
        false
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::PerlinNoise
    }

    // Port of: src/shaders/SkPerlinNoiseShaderImpl.cpp#L83-L107 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        if m_rec.apply(rec, Matrix::i()).is_none() {
            return false;
        }

        let data = self.painting_data();
        let alloc = rec.alloc;
        // `fWidth`/`fHeight` are ints (stitch sizes, far below 2**24), converted to floats.
        #[allow(clippy::cast_precision_loss)]
        let stitch_x = data.stitch_data_init.width as scalar;
        #[allow(clippy::cast_precision_loss)]
        let stitch_y = data.stitch_data_init.height as scalar;
        let ctx = alloc.make(PerlinNoiseCtx {
            noise_type: self.noise_type,
            base_frequency_x: data.base_frequency.x,
            base_frequency_y: data.base_frequency.y,
            stitch_data_in_x: stitch_x,
            stitch_data_in_y: stitch_y,
            stitching: self.stitch_tiles,
            num_octaves: self.num_octaves,
            lattice_selector: data.lattice_selector,
            noise_data: data.noise,
        });
        rec.pipeline.append(Stage::PerlinNoise(ctx));
        true
    }
}

/// `SkShaders::MakeFractalNoise` and `MakeTurbulence`.
pub mod shaders {
    use skia_rust_core::color::{Color, Color4f};
    use skia_rust_core::color_space::ColorSpace;
    use skia_rust_core::raster_pipeline::contexts::PerlinNoiseShaderType;
    use skia_rust_core::scalar::scalar;
    use skia_rust_core::shader::Shader;
    use skia_rust_core::shaders::{color, color_in_space};
    use skia_rust_core::size::ISize;

    use super::{MAX_OCTAVES, PerlinNoiseShader};

    /// `valid_input`.
    // Port of: src/shaders/SkPerlinNoiseShaderImpl.cpp#L111-L126 (chrome/m156)
    fn valid_input(
        base_x: scalar,
        base_y: scalar,
        num_octaves: i32,
        tile_size: Option<ISize>,
        seed: scalar,
    ) -> bool {
        if !(base_x >= 0.0 && base_y >= 0.0) {
            return false;
        }
        if !(0..=MAX_OCTAVES).contains(&num_octaves) {
            return false;
        }
        if tile_size.is_some_and(|tile| !(tile.width >= 0 && tile.height >= 0)) {
            return false;
        }
        if !seed.is_finite() {
            return false;
        }
        true
    }

    /// Perlin fractal noise (`SkShaders::MakeFractalNoise`). `None` if the inputs are invalid
    /// (negative or non-finite frequencies, more than 255 octaves, a negative tile size or a
    /// non-finite seed). With no octaves the shader is the constant `0.5` gray.
    ///
    /// - `base_frequency`: the frequency in x and y, both non-negative.
    /// - `num_octaves`: at most 255.
    /// - `seed`: truncated to an integer and clamped to `[1, 2**31 - 2]`.
    /// - `tile_size`: if `Some` and not empty, the frequencies are adjusted so the noise tiles
    ///   seamlessly.
    #[doc(alias = "MakeFractalNoise")]
    #[must_use]
    pub fn fractal_noise(
        base_frequency: (scalar, scalar),
        num_octaves: usize,
        seed: scalar,
        tile_size: impl Into<Option<ISize>>,
    ) -> Option<Shader> {
        let tile_size = tile_size.into();
        let num_octaves = i32::try_from(num_octaves).ok()?;
        if !valid_input(
            base_frequency.0,
            base_frequency.1,
            num_octaves,
            tile_size,
            seed,
        ) {
            return None;
        }

        if num_octaves == 0 {
            // For kFractalNoise, w/o any octaves, the entire shader collapses to:
            //    [0,0,0,0] * 0.5 + 0.5
            const TRANSPARENT_GRAY: Color4f = Color4f::new(0.5, 0.5, 0.5, 0.5);
            return color_in_space(TRANSPARENT_GRAY, None::<ColorSpace>);
        }

        Some(Shader::from_base(PerlinNoiseShader::new(
            PerlinNoiseShaderType::FractalNoise,
            base_frequency.0,
            base_frequency.1,
            num_octaves,
            seed,
            tile_size,
        )))
    }

    /// Perlin turbulence (`SkShaders::MakeTurbulence`): the absolute value of the noise. `None`
    /// if the inputs are invalid (see [`fractal_noise`]). With no octaves the shader is
    /// transparent.
    #[doc(alias = "MakeTurbulence")]
    #[must_use]
    pub fn turbulence(
        base_frequency: (scalar, scalar),
        num_octaves: usize,
        seed: scalar,
        tile_size: impl Into<Option<ISize>>,
    ) -> Option<Shader> {
        let tile_size = tile_size.into();
        let num_octaves = i32::try_from(num_octaves).ok()?;
        if !valid_input(
            base_frequency.0,
            base_frequency.1,
            num_octaves,
            tile_size,
            seed,
        ) {
            return None;
        }

        if num_octaves == 0 {
            // For kTurbulence, w/o any octaves, the entire shader collapses to: [0,0,0,0]
            return Some(color(Color::TRANSPARENT));
        }

        Some(Shader::from_base(PerlinNoiseShader::new(
            PerlinNoiseShaderType::Turbulence,
            base_frequency.0,
            base_frequency.1,
            num_octaves,
            seed,
            tile_size,
        )))
    }
}
