// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8 token buffer of libwebp 1.4.0 (`src/enc/token_enc.c`): `VP8TBufferInit`,
//! `AddToken`, `AddConstantToken`, `VP8RecordCoeffTokens` and `VP8EmitTokens`, with the extra
//! probabilities `VP8Cat3`..`VP8Cat6` of `frame_enc.c`.
//!
//! The C buffer is a list of pages filled from the end; emitting it walks the pages and slots
//! in insertion order, so the port keeps the tokens in one `Vec` in insertion order, which is
//! the order `VP8EmitTokens` writes them in. `VP8EstimateTokenSize` is used only by the
//! target-size search, which `SkWebpEncoder` does not use (`target_size = 0`), so it is not
//! ported.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::too_many_lines)]

use super::vp8_bit_writer::VP8BitWriter;
use super::vp8_cost::{NUM_BANDS, NUM_CTX, NUM_PROBAS, VP8EncProba, VP8Residual};
use super::vp8_cost::record_stats;
use super::cost_tables::VP8_ENC_BANDS;

/// Port of `FIXED_PROBA_BIT`: a token whose probability is a constant, not `probas[idx]`.
const FIXED_PROBA_BIT: u16 = 1 << 14;

/// Port of `VP8Cat3` (`frame_enc.c`).
const VP8_CAT3: [u8; 3] = [173, 148, 140];
/// Port of `VP8Cat4`.
const VP8_CAT4: [u8; 4] = [176, 155, 140, 135];
/// Port of `VP8Cat5`.
const VP8_CAT5: [u8; 5] = [180, 157, 141, 134, 130];
/// Port of `VP8Cat6`.
const VP8_CAT6: [u8; 11] = [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129];

/// Port of `VP8TBuffer`: the recorded tokens, in order. A token is `bit << 15` with either the
/// probability index (`< FIXED_PROBA_BIT`) or `FIXED_PROBA_BIT | constant probability`.
#[derive(Debug, Clone, Default)]
pub struct VP8TBuffer {
    tokens: Vec<u16>,
}

/// Port of `TOKEN_ID(t, b, ctx)`.
#[inline]
fn token_id(t: usize, b: usize, ctx: usize) -> u32 {
    (NUM_PROBAS * (ctx + NUM_CTX * (b + NUM_BANDS * t))) as u32
}

impl VP8TBuffer {
    /// Port of `VP8TBufferInit`.
    #[must_use]
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Port of `AddToken`: records a token with probability `proba_idx`, and counts `bit` in the
    /// statistics `stats`. Returns `bit`.
    fn add_token(&mut self, bit: u32, proba_idx: u32, stats: &mut u32) -> u32 {
        self.tokens
            .push(((bit as u16) << 15) | (proba_idx as u16));
        record_stats(bit as i32, stats);
        bit
    }

    /// Port of `AddConstantToken`: records a token with the constant probability `proba`.
    fn add_constant_token(&mut self, bit: u32, proba: u32) {
        self.tokens
            .push(((bit as u16) << 15) | FIXED_PROBA_BIT | (proba as u16));
    }

    /// Port of `VP8RecordCoeffTokens`: records the tokens of `res` in context `ctx`, counting
    /// the statistics in `proba`. Returns 1 (the C success value).
    pub fn record_coeff_tokens(
        &mut self,
        proba: &mut VP8EncProba,
        ctx: usize,
        res: &VP8Residual<'_>,
    ) -> i32 {
        let coeffs = res.coeffs;
        let ct = res.coeff_type;
        let last = res.last;
        let mut n = res.first;
        let mut base_id = token_id(ct, n, ctx);
        // `s` is the (band, ctx) of the C `proba_t* s`.
        let mut s = (n, ctx);
        if self.add_token_at(proba, ct, s, 0, u32::from(last >= 0), base_id) == 0 {
            return 0;
        }
        while n < 16 {
            let c = i32::from(coeffs[n]);
            n += 1;
            let sign = c < 0;
            let v: u32 = if sign { (-c) as u32 } else { c as u32 };
            if self.add_token_at(proba, ct, s, 1, u32::from(v != 0), base_id + 1) == 0 {
                base_id = token_id(ct, usize::from(VP8_ENC_BANDS[n]), 0); // ctx=0
                s = (usize::from(VP8_ENC_BANDS[n]), 0);
                continue;
            }
            if self.add_token_at(proba, ct, s, 2, u32::from(v > 1), base_id + 2) == 0 {
                base_id = token_id(ct, usize::from(VP8_ENC_BANDS[n]), 1); // ctx=1
                s = (usize::from(VP8_ENC_BANDS[n]), 1);
            } else {
                if self.add_token_at(proba, ct, s, 3, u32::from(v > 4), base_id + 3) == 0 {
                    if self.add_token_at(proba, ct, s, 4, u32::from(v != 2), base_id + 4) != 0 {
                        self.add_token_at(proba, ct, s, 5, u32::from(v == 4), base_id + 5);
                    }
                } else if self.add_token_at(proba, ct, s, 6, u32::from(v > 10), base_id + 6) == 0 {
                    if self.add_token_at(proba, ct, s, 7, u32::from(v > 6), base_id + 7) == 0 {
                        self.add_constant_token(u32::from(v == 6), 159);
                    } else {
                        self.add_constant_token(u32::from(v >= 9), 165);
                        self.add_constant_token(u32::from(v & 1 == 0), 145);
                    }
                } else {
                    let mut residue = v - 3;
                    let mask: u32;
                    let tab: &[u8];
                    if residue < (8 << 1) {
                        // VP8Cat3  (3b)
                        self.add_token_at(proba, ct, s, 8, 0, base_id + 8);
                        self.add_token_at(proba, ct, s, 9, 0, base_id + 9);
                        residue -= 8 << 0;
                        mask = 1 << 2;
                        tab = &VP8_CAT3;
                    } else if residue < (8 << 2) {
                        // VP8Cat4  (4b)
                        self.add_token_at(proba, ct, s, 8, 0, base_id + 8);
                        self.add_token_at(proba, ct, s, 9, 1, base_id + 9);
                        residue -= 8 << 1;
                        mask = 1 << 3;
                        tab = &VP8_CAT4;
                    } else if residue < (8 << 3) {
                        // VP8Cat5  (5b)
                        self.add_token_at(proba, ct, s, 8, 1, base_id + 8);
                        self.add_token_at(proba, ct, s, 9, 0, base_id + 10);
                        residue -= 8 << 2;
                        mask = 1 << 4;
                        tab = &VP8_CAT5;
                    } else {
                        // VP8Cat6 (11b)
                        self.add_token_at(proba, ct, s, 8, 1, base_id + 8);
                        self.add_token_at(proba, ct, s, 9, 1, base_id + 10);
                        residue -= 8 << 3;
                        mask = 1 << 10;
                        tab = &VP8_CAT6;
                    }
                    let mut mask = mask;
                    let mut t = 0;
                    while mask != 0 {
                        self.add_constant_token(u32::from(residue & mask != 0), u32::from(tab[t]));
                        mask >>= 1;
                        t += 1;
                    }
                }
                base_id = token_id(ct, usize::from(VP8_ENC_BANDS[n]), 2); // ctx=2
                s = (usize::from(VP8_ENC_BANDS[n]), 2);
            }
            self.add_constant_token(u32::from(sign), 128);
            if n == 16 || self.add_token_at(proba, ct, s, 0, u32::from(n as i32 <= last), base_id) == 0 {
                return 1; // EOB
            }
        }
        1
    }

    /// `AddToken` at statistics slot `k` of the (band, ctx) `s` of coefficient type `ct`.
    fn add_token_at(
        &mut self,
        proba: &mut VP8EncProba,
        ct: usize,
        s: (usize, usize),
        k: usize,
        bit: u32,
        proba_idx: u32,
    ) -> u32 {
        self.add_token(bit, proba_idx, &mut proba.stats[ct][s.0][s.1][k])
    }

    /// Port of `VP8TBufferClear` (the pages are released; the buffer is empty again).
    pub fn clear(&mut self) {
        self.tokens.clear();
    }

    /// Port of `VP8EmitTokens`: writes the recorded tokens with the frame probabilities `probas`
    /// (the flattened `coeffs_`), in order. Returns 1.
    pub fn emit_tokens(&self, bw: &mut VP8BitWriter, probas: &[u8]) -> i32 {
        for &token in &self.tokens {
            let bit = i32::from((token >> 15) & 1);
            if token & FIXED_PROBA_BIT != 0 {
                bw.put_bit(bit != 0, i32::from(token & 0xff)); // constant proba
            } else {
                bw.put_bit(bit != 0, i32::from(probas[usize::from(token & 0x3fff)]));
            }
        }
        1
    }
}
