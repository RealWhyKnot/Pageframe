use core::ops::Range;

use parley_engine::shape::Whitespace;
use smallvec::SmallVec;

use super::run::Run;
use super::whitespace::is_collapsible_space;
use crate::style::Brush;

#[derive(Clone, Default)]
pub(super) struct RtlRounding {
    changes: SmallVec<[(usize, f32); 2]>,
}

impl RtlRounding {
    pub(super) fn for_run<B: Brush>(run: &Run<'_, B>, visual_clusters: Range<u32>) -> Self {
        let mut rounding = Self::default();
        let layout = run.layout;
        if !run.is_rtl() || layout.rtl_item_ends.is_empty() {
            return rounding;
        }

        let start = visual_clusters.start as usize;
        let end = visual_clusters.end as usize;
        let item = layout
            .rtl_item_ends
            .partition_point(|&(last, _)| last < start);
        let Some(&(last, extra)) = layout.rtl_item_ends.get(item) else {
            return rounding;
        };
        let line = run.line_index as usize;
        let data = &layout.data;
        let line_data = &data.lines[line];
        let mut record = |cluster: usize, amount: f32| {
            if amount == 0.0 || !(start..end).contains(&cluster) || cluster > last {
                return;
            }
            let shaped = &data.shaped_text.shaped_clusters()[cluster];
            if shaped.glyph_len() == 0
                || (cluster >= line_data.justification.justification_end_cluster as usize
                    && is_collapsible_space(
                        shaped.whitespace(),
                        &data.styles[usize::from(shaped.style_index)],
                    ))
            {
                return;
            }
            rounding.changes.push((cluster, amount));
        };
        if data.shaped_text.shaped_clusters()[last].whitespace() != Whitespace::Tab {
            record(last, extra);
        }
        let first = data
            .split_item_ceils
            .partition_point(|&(index, _)| index < line);
        for (_, growth) in data.split_item_ceils[first..]
            .iter()
            .take_while(|&&(index, _)| index == line)
        {
            record(growth.cluster(), growth.extra());
        }
        rounding
    }

    pub(super) fn offset(&self, cluster: u32) -> f32 {
        self.changes
            .iter()
            .filter(|&&(end, _)| end > cluster as usize)
            .map(|&(_, extra)| extra)
            .sum()
    }
}
