// Copyright © 2018  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset-plan.cc, src/hb-subset-plan.hh, src/hb-subset-input.cc (harfbuzz 9cb1fee5)

//! The subset plan: which glyphs and code points are retained and how glyph ids map. The plan is
//! built for Skia's input (glyph ids only, `RETAIN_GIDS`, optionally `NOTDEF_OUTLINE`, default
//! sets for everything else), so the code paths for requested unicodes, custom glyph maps and
//! instancing are not ported.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::bytes::tag;
use crate::cmap::Cmap;
use crate::glyf::GlyfAccelerator;
use crate::sfnt::{Face, FaceBuilder};
use crate::{FLAG_RETAIN_GIDS, Res, SubsetError};

const HB_SET_VALUE_INVALID: u32 = 0xFFFF_FFFF;
const HB_MAX_NESTING_LEVEL: u32 = 64;
const HB_MAX_COMPOSITE_OPERATIONS_PER_GLYPH: i64 = 64;

/// `default_layout_features` (hb-subset-input.cc#L82-L186).
const DEFAULT_LAYOUT_FEATURES: [&[u8; 4]; 72] = [
    b"rvrn", b"ccmp", b"liga", b"locl", b"mark", b"mkmk", b"rlig", b"frac", b"numr", b"dnom",
    b"calt", b"clig", b"curs", b"kern", b"rclt", b"valt", b"vert", b"vkrn", b"vpal", b"vrt2",
    b"ltra", b"ltrm", b"rtla", b"rtlm", b"rand", b"jalt", b"chws", b"vchw", b"halt", b"vhal",
    b"Harf", b"HARF", b"Buzz", b"BUZZ", b"init", b"medi", b"fina", b"isol", b"med2", b"fin2",
    b"fin3", b"cswh", b"mset", b"stch", b"ljmo", b"vjmo", b"tjmo", b"abvs", b"blws", b"abvm",
    b"blwm", b"nukt", b"akhn", b"rphf", b"rkrf", b"pref", b"blwf", b"half", b"abvf", b"pstf",
    b"cfar", b"vatu", b"cjct", b"init", b"pres", b"abvs", b"blws", b"psts", b"haln", b"dist",
    b"abvm", b"blwm",
];

/// Port of `hb_subset_plan_t` (hb-subset-plan.hh#L116-L238), for the fields Skia's input reaches.
pub(crate) struct Plan<'a> {
    pub source: &'a Face<'a>,
    pub flags: u32,
    pub glyphs_requested: BTreeSet<u32>,
    pub unicodes: BTreeSet<u32>,
    /// `(code point, glyph)`; the glyph is the new glyph id once the plan is built.
    pub unicode_to_new_gid_list: Vec<(u32, u32)>,
    pub codepoint_to_glyph: BTreeMap<u32, u32>,
    pub glyphset_gsub: BTreeSet<u32>,
    pub glyphset: BTreeSet<u32>,
    /// `_glyphset_mathed`: `glyphset_gsub` plus the glyphs `MATH` refers to.
    pub glyphset_mathed: BTreeSet<u32>,
    pub glyph_map: HashMap<u32, u32>,
    /// `glyph_map_gsub`: the new glyph id of each glyph of `glyphset_gsub`.
    pub glyph_map_gsub: HashMap<u32, u32>,
    pub used_mark_sets_map: HashMap<u32, u32>,
    pub layout: crate::layout::LayoutPlan,
    /// `layout_features`: the feature tags kept by default (hb-subset-input.cc#L82-L187).
    pub layout_features: BTreeSet<u32>,
    pub reverse_glyph_map: HashMap<u32, u32>,
    pub new_to_old_gid_list: Vec<(u32, u32)>,
    pub num_output_glyphs: u32,
    pub os2_min_cmap_codepoint: u32,
    pub os2_max_cmap_codepoint: u32,
    pub name_ids: BTreeSet<u32>,
    /// `colr_palettes`: old palette index to new, from the COLR closure.
    pub colr_palettes: HashMap<u32, u32>,
    /// `colrv1_layers`: old `LayerList` index to new.
    pub colrv1_layers: HashMap<u32, u32>,
    /// `_glyphset_colred`: `glyphset_mathed` plus the glyphs `COLR` refers to.
    pub glyphset_colred: BTreeSet<u32>,
    pub name_languages: BTreeSet<u32>,
    pub drop_tables: BTreeSet<u32>,
    pub no_subset_tables: BTreeSet<u32>,
    dest: FaceBuilder,
    /// `buf.allocated` of `hb_subset_plan_execute_or_fail`: the serialization buffer shared by the
    /// tables.
    buf_allocated: u32,
}

impl<'a> Plan<'a> {
    /// Port of the `hb_subset_input_t` constructor defaults (hb-subset-input.cc#L33-L100) for the
    /// sets Skia leaves alone, and of `hb_subset_plan_t::hb_subset_plan_t`
    /// (hb-subset-plan.cc#L610-L760).
    pub(crate) fn new(source: &'a Face<'a>, flags: u32, glyphs: BTreeSet<u32>) -> Res<Plan<'a>> {
        let name_ids: BTreeSet<u32> = (0..=6).collect();
        let name_languages: BTreeSet<u32> = [0x0409].into_iter().collect();
        let drop_tables: BTreeSet<u32> = [
            b"morx", b"mort", b"kerx", b"kern", b"JSTF", b"DSIG", b"EBDT", b"EBLC", b"EBSC",
            b"SVG ", b"PCLT", b"LTSH", b"Feat", b"Glat", b"Gloc", b"Silf", b"Sill",
        ]
        .iter()
        .map(|t| tag(t))
        .collect();
        let no_subset_tables: BTreeSet<u32> = [b"gasp", b"fpgm", b"prep", b"VDMX", b"DSIG"]
            .iter()
            .map(|t| tag(t))
            .collect();

        let mut plan = Plan {
            source,
            flags,
            glyphs_requested: glyphs,
            unicodes: BTreeSet::new(),
            unicode_to_new_gid_list: Vec::new(),
            codepoint_to_glyph: BTreeMap::new(),
            glyphset_gsub: BTreeSet::new(),
            glyphset: BTreeSet::new(),
            glyphset_mathed: BTreeSet::new(),
            glyph_map: HashMap::new(),
            glyph_map_gsub: HashMap::new(),
            used_mark_sets_map: HashMap::new(),
            layout: crate::layout::LayoutPlan::default(),
            layout_features: DEFAULT_LAYOUT_FEATURES.iter().map(|t| tag(t)).collect(),
            reverse_glyph_map: HashMap::new(),
            buf_allocated: vector_grow(0, 8192 - 16),
            new_to_old_gid_list: Vec::new(),
            num_output_glyphs: 0,
            os2_min_cmap_codepoint: 0,
            os2_max_cmap_codepoint: 0,
            name_ids,
            colr_palettes: HashMap::new(),
            colrv1_layers: HashMap::new(),
            glyphset_colred: BTreeSet::new(),
            name_languages,
            drop_tables,
            no_subset_tables,
            dest: FaceBuilder::default(),
        };

        plan.populate_unicodes_to_retain();
        plan.populate_gids_to_retain()?;
        plan.create_old_gid_to_new_gid_map();

        // `_create_glyph_map_gsub`
        for &g in &plan.glyphset_gsub {
            plan.glyph_map_gsub.insert(
                g,
                plan.glyph_map
                    .get(&g)
                    .copied()
                    .unwrap_or(HB_SET_VALUE_INVALID),
            );
        }

        // Now that we have old to new gid map update the unicode to new gid list.
        for p in &mut plan.unicode_to_new_gid_list {
            p.1 = plan
                .glyph_map
                .get(&p.1)
                .copied()
                .unwrap_or(HB_SET_VALUE_INVALID);
        }
        if !plan.drop_tables.contains(&tag(b"GDEF")) {
            let gdef = plan.source.table(tag(b"GDEF"));
            plan.used_mark_sets_map =
                crate::gdef::remap_used_mark_sets(crate::ot::View::new(gdef), &plan.glyphset_gsub);
        }
        Ok(plan)
    }

    /// Port of `_populate_unicodes_to_retain` (hb-subset-plan.cc#L193-L328) for an empty set of
    /// requested unicodes.
    fn populate_unicodes_to_retain(&mut self) {
        let num_glyphs = self.source.num_glyphs();
        let cmap = Cmap::new(self.source.table(tag(b"cmap")));

        if self.glyphs_requested.is_empty() {
            // The fast path: `unicodes.get_population () < size_threshold` holds for the empty
            // unicode set, and nothing is collected.
            let _ = num_glyphs;
        } else {
            // This approach is slower, but can handle adding in glyphs to the subset and will
            // match them with cmap entries.
            let (cmap_unicodes, unicode_glyphid_map) = cmap
                .as_ref()
                .map_or_else(Default::default, Cmap::collect_mapping);
            for &cp in &cmap_unicodes {
                // `(*unicode_glyphid_map)[cp]`
                let gid = unicode_glyphid_map
                    .get(&cp)
                    .copied()
                    .unwrap_or(HB_SET_VALUE_INVALID);
                if !self.glyphs_requested.contains(&gid) {
                    continue;
                }
                self.codepoint_to_glyph.insert(cp, gid);
                self.unicode_to_new_gid_list.push((cp, gid));
            }
            // Add gids which where requested, but not mapped in cmap
            for &g in &self.glyphs_requested {
                if g >= num_glyphs {
                    break;
                }
                self.glyphset_gsub.insert(g);
            }
        }

        for &(cp, gid) in &self.unicode_to_new_gid_list {
            self.unicodes.insert(cp);
            self.glyphset_gsub.insert(gid);
        }
        // The min and max codepoints for OS/2 do not consider variation selectors (which the empty
        // unicode input never retains).
        self.os2_min_cmap_codepoint = self
            .unicodes
            .first()
            .copied()
            .unwrap_or(HB_SET_VALUE_INVALID);
        self.os2_max_cmap_codepoint = self
            .unicodes
            .last()
            .copied()
            .unwrap_or(HB_SET_VALUE_INVALID);
    }

    /// Port of `_populate_gids_to_retain` (hb-subset-plan.cc#L449-L506).
    fn populate_gids_to_retain(&mut self) -> Res<()> {
        let num_glyphs = self.source.num_glyphs();
        let glyf = GlyfAccelerator::new(self.source);

        self.glyphset_gsub.insert(0); // Not-def

        // `_cmap_closure`
        if let Some(cmap) = Cmap::new(self.source.table(tag(b"cmap"))) {
            cmap.closure_glyphs(&self.unicodes, &mut self.glyphset_gsub);
        }

        crate::layout::populate_gids_to_retain(self)?;

        remove_invalid_gids(&mut self.glyphset_gsub, num_glyphs);

        let mut mathed = self.glyphset_gsub.clone();
        if !self.drop_tables.contains(&tag(b"MATH")) {
            crate::layout::math_closure(self, &mut mathed);
            remove_invalid_gids(&mut mathed, num_glyphs);
        }

        self.glyphset_mathed = mathed.clone();
        let mut cur_glyphset = mathed;
        if !self.drop_tables.contains(&tag(b"COLR")) {
            crate::colr::colr_closure(self, &mut cur_glyphset)?;
            remove_invalid_gids(&mut cur_glyphset, num_glyphs);
        }

        self.glyphset_colred = cur_glyphset.clone();

        crate::layout::nameid_closure(self);

        // Populate a full set of glyphs to retain by adding all referenced composite glyphs.
        if glyf.has_data() {
            let operation_count = cur_glyphset.len() as i64 * HB_MAX_COMPOSITE_OPERATIONS_PER_GLYPH;
            for &gid in &cur_glyphset {
                glyf_add_gid_and_children(&glyf, gid, &mut self.glyphset, operation_count, 0);
            }
        } else {
            self.glyphset.extend(cur_glyphset.iter().copied());
        }
        crate::cff::seac_closure(self, &cur_glyphset)?;

        remove_invalid_gids(&mut self.glyphset, num_glyphs);
        Ok(())
    }

    /// Port of `_create_old_gid_to_new_gid_map` (hb-subset-plan.cc#L536-L639) without a custom
    /// glyph map.
    fn create_old_gid_to_new_gid_map(&mut self) {
        let retain_gids = self.flags & FLAG_RETAIN_GIDS != 0;
        if retain_gids {
            self.new_to_old_gid_list = self.glyphset.iter().map(|&g| (g, g)).collect();
            let max_glyph = self
                .glyphset
                .last()
                .copied()
                .unwrap_or(HB_SET_VALUE_INVALID);
            self.num_output_glyphs = max_glyph.wrapping_add(1);
        } else {
            self.new_to_old_gid_list = self
                .glyphset
                .iter()
                .enumerate()
                .map(|(i, &g)| (i as u32, g))
                .collect();
            self.num_output_glyphs = self.new_to_old_gid_list.len() as u32;
        }
        for &(new, old) in &self.new_to_old_gid_list {
            self.reverse_glyph_map.insert(new, old);
            self.glyph_map.insert(old, new);
        }
    }

    /// The buffer handling of `_hb_subset_table` and `_hb_subset_table_try`
    /// (hb-subset-table.hh#L36-L190) for a table of `blob_len` source bytes whose serialization
    /// needed `peak` bytes: the buffer starts at the estimated size (and never shrinks), doubles
    /// while the serializer runs out of room, and the table fails once the doubled size exceeds
    /// 256 times the source table. Returns whether the table gets subset.
    pub(crate) fn account_table(&mut self, t: u32, blob_len: usize, peak: usize) -> bool {
        let estimate = self.estimate_table_size(blob_len, t);
        self.buf_allocated = vector_grow(self.buf_allocated, estimate);
        loop {
            if peak as u64 <= u64::from(self.buf_allocated) {
                return true;
            }
            let buf_size = self.buf_allocated.wrapping_mul(2).wrapping_add(16);
            if buf_size > (blob_len as u32).wrapping_mul(256) {
                return false;
            }
            self.buf_allocated = buf_size;
        }
    }

    /// `_hb_subset_estimate_table_size` (hb-subset-table.hh#L72-L100).
    fn estimate_table_size(&self, table_len: usize, t: u32) -> u32 {
        let src_glyphs = self.source.num_glyphs();
        let dst_glyphs = self.glyphset.len() as u32;
        let mut bulk: u32 = 8192;
        let same_size = [tag(b"GSUB"), tag(b"GPOS"), tag(b"GDEF"), tag(b"name")].contains(&t);
        if self.flags & FLAG_RETAIN_GIDS != 0 {
            if t == tag(b"CFF ") {
                bulk += src_glyphs * 16;
            } else if t == tag(b"CFF2") {
                bulk += src_glyphs * 4;
            }
        }
        let table_len = table_len as u32;
        if src_glyphs == 0 || same_size {
            return bulk + table_len;
        }
        bulk + (f64::from(table_len) * (f64::from(dst_glyphs) / f64::from(src_glyphs)).sqrt())
            as u32
    }

    /// The length of the table in the destination face.
    pub(crate) fn dest_table_len(&self, t: u32) -> usize {
        self.dest.table_len(t)
    }

    /// Port of `hb_subset_plan_t::add_table`.
    pub(crate) fn add_table(&mut self, t: u32, contents: Vec<u8>) {
        self.dest.add_table(t, contents);
    }

    /// Port of `hb_face_reference_blob (plan->dest)`.
    pub(crate) fn build(&self) -> Vec<u8> {
        self.dest.build()
    }

    /// Port of `plan->old_gid_for_new_gid`.
    pub(crate) fn old_gid_for_new_gid(&self, new_gid: u32) -> Option<u32> {
        self.reverse_glyph_map.get(&new_gid).copied()
    }
}

/// `hb_vector_t::alloc (size)` from `allocated` (hb-vector.hh#L516-L540): grows by 1.5 times plus
/// 8 until the size fits.
fn vector_grow(allocated: u32, size: u32) -> u32 {
    let mut new_allocated = allocated;
    while size > new_allocated {
        new_allocated += (new_allocated >> 1) + 8;
    }
    new_allocated
}

/// Port of `_remove_invalid_gids` (hb-subset-plan.cc#L156-L160).
fn remove_invalid_gids(glyphs: &mut BTreeSet<u32>, num_glyphs: u32) {
    let doomed: Vec<u32> = glyphs.range(num_glyphs..).copied().collect();
    for g in doomed {
        glyphs.remove(&g);
    }
}

/// Port of `_glyf_add_gid_and_children` (hb-subset-plan.cc#L406-L430). Returns the operation
/// count left.
fn glyf_add_gid_and_children(
    glyf: &GlyfAccelerator<'_>,
    gid: u32,
    gids_to_retain: &mut BTreeSet<u32>,
    mut operation_count: i64,
    depth: u32,
) -> i64 {
    // Check if is already visited
    if gids_to_retain.contains(&gid) {
        return operation_count;
    }
    gids_to_retain.insert(gid);

    let depth_here = depth;
    if depth_here > HB_MAX_NESTING_LEVEL {
        return operation_count;
    }
    operation_count -= 1;
    if operation_count < 0 {
        return operation_count;
    }
    for item in glyf.component_gids(gid) {
        operation_count =
            glyf_add_gid_and_children(glyf, item, gids_to_retain, operation_count, depth_here + 1);
    }
    operation_count
}

/// The error for a table whose subsetting is not ported.
pub(crate) fn unsupported<T>(what: &'static str) -> Res<T> {
    Err(SubsetError::Unsupported(what))
}
