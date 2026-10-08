// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkTableColorFilter.{h,cpp}

//! `SkTableColorFilter`: maps each channel of the unpremultiplied color through a 256-entry
//! table. The factories are in [`color_filters`](crate::color_filters).

use crate::color_filter::{ColorFilterBase, ColorFilterType};
use crate::color_table::ColorTable;
use crate::effect_priv::StageRec;
use crate::raster_pipeline::{Stage, contexts::TablesCtx};

/// A color filter that maps each channel through a [`ColorTable`] (`SkTableColorFilter`).
// Port of: src/effects/colorfilters/SkTableColorFilter.h#L22-L41 (chrome/m156)
#[doc(alias = "SkTableColorFilter")]
#[derive(Clone, Debug)]
pub struct TableColorFilter {
    table: ColorTable,
}

impl TableColorFilter {
    /// Wraps `table`.
    #[must_use]
    pub fn new(table: ColorTable) -> Self {
        Self { table }
    }

    /// The table (`table`).
    #[must_use]
    pub fn table(&self) -> &ColorTable {
        &self.table
    }
}

impl ColorFilterBase for TableColorFilter {
    // Port of: src/effects/colorfilters/SkTableColorFilter.cpp#L25-L44 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, shader_is_opaque: bool) -> bool {
        if !shader_is_opaque {
            rec.pipeline.append(Stage::Unpremul);
        }

        // skia-rust: the context owns copies of the tables, so it can live in the arena.
        let tables = rec.alloc.make(TablesCtx {
            r: *self.table.red_table(),
            g: *self.table.green_table(),
            b: *self.table.blue_table(),
            a: *self.table.alpha_table(),
        });
        rec.pipeline.append(Stage::ByteTables(tables));

        let definitely_opaque = shader_is_opaque && tables.a[0xff] == 0xff;
        if !definitely_opaque {
            rec.pipeline.append(Stage::Premul);
        }
        true
    }

    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Table
    }
}
