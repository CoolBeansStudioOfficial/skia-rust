// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset-cff*.cc, src/hb-ot-cff*-table.* (harfbuzz 9cb1fee5)

//! `CFF` and `CFF2` subsetting. Not ported yet.

use std::collections::BTreeSet;

use crate::Res;
use crate::bytes::tag;
use crate::plan::{Plan, unsupported};

/// Port of the `_add_cff_seac_components` pass of `_populate_gids_to_retain`
/// (hb-subset-plan.cc#L486-L498).
pub(crate) fn seac_closure(plan: &mut Plan<'_>, _cur_glyphset: &BTreeSet<u32>) -> Res<()> {
    if !plan.source.table(tag(b"CFF ")).is_empty() {
        return unsupported("CFF");
    }
    Ok(())
}
