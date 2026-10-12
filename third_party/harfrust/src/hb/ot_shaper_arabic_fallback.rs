// Copyright (c) HarfBuzz developers. MIT licence (see LICENSE).
//
// skia-rust addition (SKIA-RUST.md, arabic-fallback-shaping): a port of HarfBuzz's
// hb-ot-shaper-arabic-fallback.hh (HarfBuzz 13.1, the version Skia pins), which HarfRust 0.9.0
// stubs out (`arabic_fallback_shape` returned `false`).
//
// A font that has no Arabic GSUB features but does map the Arabic Presentation Forms code points
// gets the init, medi, fina and isol forms and the ligatures substituted by lookups that are
// synthesized from the code point mapping. HarfBuzz serializes them as binary GSUB subtables and
// applies them through `hb_ot_layout_substitute_lookup`; here the lookups are plain tables
// applied with the same loop (`apply_forward` of ot_layout.rs), skipping iterator, glyph
// property and ligature code, so the buffer ends up identical. The Windows-1256 variant is not
// ported: Skia builds HarfBuzz with `HB_NO_WIN1256`.

use alloc::vec::Vec;

use super::buffer::hb_buffer_t;
use super::font_funcs::FontFuncsDispatch;
use super::ot_layout::TableIndex;
use super::ot_layout_common::lookup_flags;
use super::ot_layout_gsubgpos::OT::{check_glyph_property, hb_ot_apply_context_t};
use super::ot_layout_gsubgpos::{ligate_input, match_glyph, match_input};
use super::ot_shape_plan::hb_ot_shape_plan_t;
use super::ot_shaper_arabic_fallback_table::{
    LIGATURE_3_TABLE, LIGATURE_MARK_TABLE, LIGATURE_TABLE, SHAPING_TABLE, SHAPING_TABLE_FIRST,
    SHAPING_TABLE_LAST,
};
use super::{hb_mask_t, hb_tag_t};

// Features ordered the same as the entries in the shaping table rows, followed by rlig. Don't
// change. (`arabic_fallback_features`.)
const ARABIC_FALLBACK_FEATURES: [hb_tag_t; 7] = [
    hb_tag_t::new(b"init"),
    hb_tag_t::new(b"medi"),
    hb_tag_t::new(b"fina"),
    hb_tag_t::new(b"isol"),
    hb_tag_t::new(b"rlig"),
    hb_tag_t::new(b"rlig"),
    hb_tag_t::new(b"rlig"),
];

/// A synthesized lookup: one subtable.
enum FallbackLookup {
    /// Single substitution, sorted by input glyph.
    Single {
        lookup_props: u32,
        glyphs: Vec<u16>,
        substitutes: Vec<u16>,
    },
    /// Ligature substitution, sorted by first glyph; per first glyph the ligatures in table order
    /// as `(components after the first, ligature glyph)`.
    Ligature {
        lookup_props: u32,
        first_glyphs: Vec<u16>,
        sets: Vec<Vec<(Vec<u16>, u16)>>,
    },
}

// Port of: src/hb-ot-shaper-arabic-fallback.hh#L50-L103 (HarfBuzz 9cb1fee5),
// `arabic_fallback_synthesize_lookup_single`.
fn synthesize_lookup_single(
    font_funcs: &mut FontFuncsDispatch,
    feature_index: usize,
) -> Option<FallbackLookup> {
    let mut pairs: Vec<(u16, u16)> = Vec::new();

    // Populate arrays.
    for u in SHAPING_TABLE_FIRST..SHAPING_TABLE_LAST + 1 {
        let s = u32::from(SHAPING_TABLE[(u - SHAPING_TABLE_FIRST) as usize][feature_index]);
        if s == 0 {
            continue;
        }
        let (Some(u_glyph), Some(s_glyph)) =
            (font_funcs.nominal_glyph(u), font_funcs.nominal_glyph(s))
        else {
            continue;
        };
        let (u_glyph, s_glyph) = (u_glyph.to_u32(), s_glyph.to_u32());
        if u_glyph == s_glyph || u_glyph > 0xFFFF || s_glyph > 0xFFFF {
            continue;
        }
        pairs.push((u_glyph as u16, s_glyph as u16));
    }

    if pairs.is_empty() {
        return None;
    }

    // `hb_stable_sort` by glyph.
    pairs.sort_by_key(|&(glyph, _)| glyph);

    Some(FallbackLookup::Single {
        lookup_props: u32::from(lookup_flags::IGNORE_MARKS),
        glyphs: pairs.iter().map(|&(g, _)| g).collect(),
        substitutes: pairs.iter().map(|&(_, s)| s).collect(),
    })
}

// Port of: src/hb-ot-shaper-arabic-fallback.hh#L105-L212 (HarfBuzz 9cb1fee5),
// `arabic_fallback_synthesize_lookup_ligature`.
fn synthesize_lookup_ligature<const N: usize>(
    font_funcs: &mut FontFuncsDispatch,
    ligature_table: &[(u16, &[([u16; N], u16)])],
    lookup_props: u32,
) -> Option<FallbackLookup> {
    // Sort out the first-glyphs: (glyph, index into the table).
    let mut first_glyphs: Vec<(u16, usize)> = Vec::new();
    for (first_glyph_idx, (first_u, _)) in ligature_table.iter().enumerate() {
        let Some(first_glyph) = font_funcs.nominal_glyph(u32::from(*first_u)) else {
            continue;
        };
        // The glyph is stored as an `HBGlyphID16`.
        first_glyphs.push((first_glyph.to_u32() as u16, first_glyph_idx));
    }
    first_glyphs.sort_by_key(|&(glyph, _)| glyph);

    // Now that the first-glyphs are sorted, walk again, populate ligatures.
    let mut sets: Vec<Vec<(Vec<u16>, u16)>> = Vec::new();
    let mut num_ligatures = 0;
    for &(_, first_glyph_idx) in &first_glyphs {
        let mut set = Vec::new();
        for (components, ligature_u) in ligature_table[first_glyph_idx].1 {
            let Some(ligature_glyph) = font_funcs.nominal_glyph(u32::from(*ligature_u)) else {
                continue;
            };

            let mut component_glyphs = Vec::with_capacity(N);
            let mut matched = true;
            for &component_u in components {
                let component_glyph = if component_u == 0 {
                    None
                } else {
                    font_funcs.nominal_glyph(u32::from(component_u))
                };
                let Some(component_glyph) = component_glyph else {
                    matched = false;
                    break;
                };
                component_glyphs.push(component_glyph.to_u32() as u16);
            }
            if !matched {
                continue;
            }

            set.push((component_glyphs, ligature_glyph.to_u32() as u16));
            num_ligatures += 1;
        }
        sets.push(set);
    }

    if num_ligatures == 0 {
        return None;
    }

    Some(FallbackLookup::Ligature {
        lookup_props,
        first_glyphs: first_glyphs.iter().map(|&(g, _)| g).collect(),
        sets,
    })
}

// Port of: src/hb-ot-shaper-arabic-fallback.hh#L214-L233 (HarfBuzz 9cb1fee5),
// `arabic_fallback_synthesize_lookup`.
fn synthesize_lookup(
    font_funcs: &mut FontFuncsDispatch,
    feature_index: usize,
) -> Option<FallbackLookup> {
    let ignore_marks = u32::from(lookup_flags::IGNORE_MARKS);
    match feature_index {
        0..=3 => synthesize_lookup_single(font_funcs, feature_index),
        4 => synthesize_lookup_ligature(font_funcs, LIGATURE_3_TABLE, ignore_marks),
        5 => synthesize_lookup_ligature(font_funcs, LIGATURE_TABLE, ignore_marks),
        6 => synthesize_lookup_ligature(font_funcs, LIGATURE_MARK_TABLE, 0),
        _ => unreachable!(),
    }
}

// The body of `apply_forward` (ot_layout.rs) for a lookup of this module, with
// `OT::SubstLookup::apply` replaced by `apply_one`.
fn apply_lookup(
    ctx: &mut hb_ot_apply_context_t,
    lookup_props: u32,
    apply_one: impl Fn(&mut hb_ot_apply_context_t) -> Option<()>,
) {
    if ctx.buffer.is_empty() || ctx.lookup_mask() == 0 {
        return;
    }

    ctx.lookup_props = lookup_props;
    ctx.update_matchers();

    // GSUB is not in place: forward substitution through the output buffer.
    ctx.buffer.clear_output();
    ctx.buffer.idx = 0;

    while ctx.buffer.successful {
        let mut j = ctx.buffer.idx;
        while j < ctx.buffer.len && {
            let info = &ctx.buffer.info[j];
            !((info.mask & ctx.lookup_mask()) != 0
                && check_glyph_property(ctx.face, info, ctx.lookup_props))
        } {
            j += 1;
        }
        if j > ctx.buffer.idx {
            ctx.buffer.next_glyphs(j - ctx.buffer.idx);
        }
        if ctx.buffer.idx >= ctx.buffer.len {
            break;
        }

        if apply_one(ctx).is_none() {
            ctx.buffer.next_glyph();
        }
    }

    ctx.buffer.sync();
}

impl FallbackLookup {
    // Port of: OT/Layout/GSUB/SingleSubst*.hh and LigatureSubstFormat1.hh `apply`, as called by
    // `apply_forward`.
    fn apply(&self, ctx: &mut hb_ot_apply_context_t) {
        match self {
            FallbackLookup::Single {
                lookup_props,
                glyphs,
                substitutes,
            } => apply_lookup(ctx, *lookup_props, |ctx| {
                let glyph = ctx.buffer.cur(0).as_glyph();
                let index = glyphs
                    .binary_search(&(glyph.to_u32() as u16))
                    .ok()
                    .filter(|_| glyph.to_u32() <= 0xFFFF)?;
                ctx.replace_glyph(substitutes[index].into());
                Some(())
            }),
            FallbackLookup::Ligature {
                lookup_props,
                first_glyphs,
                sets,
            } => apply_lookup(ctx, *lookup_props, |ctx| {
                let glyph = ctx.buffer.cur(0).as_glyph();
                let index = first_glyphs
                    .binary_search(&(glyph.to_u32() as u16))
                    .ok()
                    .filter(|_| glyph.to_u32() <= 0xFFFF)?;
                for (components, ligature) in &sets[index] {
                    // Port of: OT/Layout/GSUB/Ligature.hh `Ligature::apply`.
                    if components.is_empty() {
                        ctx.replace_glyph((*ligature).into());
                        return Some(());
                    }
                    let f = |info: &mut super::buffer::GlyphInfo, index| {
                        match_glyph(info, components[index as usize])
                    };
                    let mut match_end = 0;
                    let mut total_component_count = 0;
                    if !match_input(
                        ctx,
                        components.len() as u16,
                        f,
                        &mut match_end,
                        Some(&mut total_component_count),
                    ) {
                        ctx.buffer
                            .unsafe_to_concat(Some(ctx.buffer.idx), Some(match_end));
                        continue;
                    }
                    ligate_input(
                        ctx,
                        components.len() + 1,
                        match_end,
                        total_component_count,
                        (*ligature).into(),
                    );
                    return Some(());
                }
                None
            }),
        }
    }
}

// Port of: src/hb-ot-shaper-arabic-fallback.hh#L283-L305 (HarfBuzz 9cb1fee5),
// `arabic_fallback_plan_init_unicode`, `arabic_fallback_plan_create` and
// `arabic_fallback_plan_shape` (#L359-L372).
pub(crate) fn arabic_fallback_plan_shape(
    plan: &hb_ot_shape_plan_t,
    font_funcs: &mut FontFuncsDispatch,
    buffer: &mut hb_buffer_t,
) {
    // The plan is rebuilt per call instead of cached in the shape plan (HarfBuzz caches it with
    // the first font it sees): it depends only on the nominal glyph mapping.
    let mut lookups: Vec<(hb_mask_t, FallbackLookup)> = Vec::new();
    for (i, feature) in ARABIC_FALLBACK_FEATURES.iter().enumerate() {
        let mask = plan.ot_map.get_1_mask(*feature);
        if mask != 0 {
            if let Some(lookup) = synthesize_lookup(font_funcs, i) {
                lookups.push((mask, lookup));
            }
        }
    }

    let scale = *font_funcs.scale();
    let face = font_funcs.font();
    let mut ctx = hb_ot_apply_context_t::new(TableIndex::GSUB, face, scale, buffer);
    for (mask, lookup) in &lookups {
        ctx.set_lookup_mask(*mask);
        lookup.apply(&mut ctx);
    }
}
