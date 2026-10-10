// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Color/*, src/hb-subset-table-color.cc (harfbuzz 9cb1fee5)

//! Colour tables (`COLR`, `CPAL`, `CBLC`/`CBDT`, `sbix`). Not ported yet.

use std::collections::BTreeSet;

use crate::Res;
use crate::bytes::tag;
use crate::plan::{Plan, unsupported};

/// Port of `_colr_closure` (hb-subset-plan.cc#L98-L134).
pub(crate) fn colr_closure(plan: &mut Plan<'_>, _glyphs: &mut BTreeSet<u32>) -> Res<()> {
    if !plan.source.table(tag(b"COLR")).is_empty() {
        return unsupported("COLR");
    }
    Ok(())
}
