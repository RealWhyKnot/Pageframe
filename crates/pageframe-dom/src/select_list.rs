use keyboard_types::Key;
use markup5ever::local_name;
use pageframe_traits::events::{DomEvent, DomEventData, PageframeInputEvent};
use pageframe_traits::node_id::NodeId;

use crate::BaseDocument;
use crate::node::ElementData;
use crate::select_popup::key_step;

const DEFAULT_ROWS: u32 = 4;
const MAX_ROWS: u32 = 10_000;

pub fn select_list_rows(element: &ElementData) -> Option<u32> {
    if element.name.local != local_name!("select") {
        return None;
    }
    let size = element.attr(local_name!("size")).map_or(0, leading_number);
    let multiple = element.attr(local_name!("multiple")).is_some();
    match size {
        0 if multiple => Some(DEFAULT_ROWS),
        0 | 1 => None,
        rows => Some(rows.min(MAX_ROWS)),
    }
}

fn leading_number(text: &str) -> u32 {
    let text = text.trim_start();
    let text = text.strip_prefix('+').unwrap_or(text);
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return 0;
    }
    text[..digits].parse().unwrap_or(MAX_ROWS)
}

impl BaseDocument {
    pub(crate) fn select_is_list(&self, select: NodeId) -> bool {
        self.get_node(select)
            .and_then(|node| node.element_data())
            .and_then(select_list_rows)
            .is_some()
    }

    fn pick_in_list(
        &mut self,
        select: NodeId,
        option: NodeId,
        toggle: bool,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) {
        let before = self.selected_options(select);
        if toggle {
            let selected = before.contains(&option);
            self.set_option_selected(option, !selected);
        } else {
            for other in self.select_options(select) {
                self.set_option_selected(other, other == option);
            }
        }
        if self.selected_options(select) == before {
            return;
        }
        self.shell_provider.request_redraw();
        let value = self.select_value(select).unwrap_or_default();
        dispatch_event(DomEvent::new(
            select,
            DomEventData::Input(PageframeInputEvent { value }),
        ));
    }

    pub(crate) fn click_list_option(
        &mut self,
        option: NodeId,
        toggle: bool,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> Option<NodeId> {
        let select = self.option_select(option)?;
        let disabled = self.nodes[select].is_disabled();
        if disabled || !self.select_is_list(select) {
            return None;
        }
        if !self.option_is_disabled(option) {
            let multiple = self.nodes[select].attr(local_name!("multiple")).is_some();
            self.pick_in_list(select, option, toggle && multiple, dispatch_event);
        }
        Some(select)
    }

    pub(crate) fn list_key(
        &mut self,
        select: NodeId,
        key: &Key,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> bool {
        let Some(step) = key_step(key) else {
            return false;
        };
        let current = self.selected_options(select).last().copied();
        if let Some(option) = self.stepped_option(select, current, step) {
            self.pick_in_list(select, option, false, dispatch_event);
        }
        true
    }
}
