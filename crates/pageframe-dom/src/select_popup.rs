use keyboard_types::{Key, NamedKey};
use markup5ever::local_name;
use pageframe_traits::events::{DomEvent, DomEventData, HitResult, PageframeInputEvent};
use pageframe_traits::node_id::NodeId;

use crate::BaseDocument;
use crate::stylo_to_parley::stylo;

const MAX_ROWS: usize = 20;
const BORDER: f64 = 1.0;
const ROW_PADDING: f64 = 2.0;
const LINE_HEIGHT: f64 = 1.15;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectPopup {
    select: NodeId,
    highlighted: Option<NodeId>,
    first_row: usize,
}

impl SelectPopup {
    pub(crate) fn is_for(&self, select: NodeId) -> bool {
        self.select == select
    }
}

#[derive(Clone, Copy)]
struct Entry {
    node: NodeId,
    heading: bool,
    indented: bool,
}

#[derive(Clone, Debug)]
pub struct PopupRow {
    pub node: NodeId,
    pub label: String,
    pub heading: bool,
    pub disabled: bool,
    pub indented: bool,
    pub highlighted: bool,
}

#[derive(Clone, Debug)]
pub struct PopupLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub row_height: f64,
    pub font_size: f32,
    pub font_family: String,
    pub rows: Vec<PopupRow>,
}

impl PopupLayout {
    pub fn height(&self) -> f64 {
        count(self.rows.len()) * self.row_height + BORDER * 2.0
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height()
    }

    fn row_at(&self, x: f64, y: f64) -> Option<&PopupRow> {
        if !self.contains(x, y) {
            return None;
        }
        let mut top = self.y + BORDER;
        self.rows.iter().find(|_| {
            top += self.row_height;
            y < top
        })
    }
}

#[derive(Clone, Copy)]
enum Step {
    Next,
    Previous,
    First,
    Last,
    PageDown,
    PageUp,
}

const PAGE_STEP: usize = 3;

fn count(rows: usize) -> f64 {
    f64::from(u32::try_from(rows).unwrap_or(u32::MAX))
}

fn generic_name(generic: stylo::GenericFontFamily) -> &'static str {
    match generic {
        stylo::GenericFontFamily::Serif => "serif",
        stylo::GenericFontFamily::Monospace => "monospace",
        stylo::GenericFontFamily::Cursive => "cursive",
        stylo::GenericFontFamily::Fantasy => "fantasy",
        stylo::GenericFontFamily::SystemUi => "system-ui",
        stylo::GenericFontFamily::None | stylo::GenericFontFamily::SansSerif => "sans-serif",
    }
}

impl BaseDocument {
    fn open_popup(&self) -> Option<SelectPopup> {
        let mut popup = self.select_popup?;
        let node = self.get_node(popup.select)?;
        let usable = self.focus_node_id == Some(popup.select)
            && node.flags.is_in_document()
            && self.is_select(popup.select)
            && node.attr(local_name!("disabled")).is_none();
        if !usable {
            return None;
        }
        popup.highlighted = popup
            .highlighted
            .filter(|&option| self.option_select(option) == Some(popup.select));
        Some(popup)
    }

    fn option_label(&self, option: NodeId) -> String {
        match self.nodes[option].attr(local_name!("label")) {
            Some(label) if !label.is_empty() => label.to_owned(),
            _ => self.nodes[option]
                .text_content()
                .split_ascii_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    fn popup_entries(&self, select: NodeId) -> Vec<Entry> {
        let mut entries = Vec::new();
        let is = |id: NodeId, name: &markup5ever::LocalName| {
            self.nodes[id].data.is_element_with_tag_name(name)
        };
        let entry = |node: NodeId, heading: bool, indented: bool| Entry {
            node,
            heading,
            indented,
        };
        for &child in &self.nodes[select].children {
            if is(child, &local_name!("option")) {
                entries.push(entry(child, false, false));
            } else if is(child, &local_name!("optgroup")) {
                entries.push(entry(child, true, false));
                for &grouped in &self.nodes[child].children {
                    if is(grouped, &local_name!("option")) {
                        entries.push(entry(grouped, false, true));
                    }
                }
            }
        }
        entries
    }

    fn popup_row(&self, popup: SelectPopup, entry: Entry) -> PopupRow {
        let label = if entry.heading {
            self.nodes[entry.node]
                .attr(local_name!("label"))
                .unwrap_or_default()
                .to_owned()
        } else {
            self.option_label(entry.node)
        };
        PopupRow {
            node: entry.node,
            label,
            heading: entry.heading,
            disabled: entry.heading || self.option_is_disabled(entry.node),
            indented: entry.indented,
            highlighted: !entry.heading && popup.highlighted == Some(entry.node),
        }
    }

    pub fn select_popup_layout(&self) -> Option<PopupLayout> {
        let popup = self.open_popup()?;
        let node = self.get_node(popup.select)?;
        let style = node.primary_styles()?;
        let font = style.get_font();
        let font_size = font.font_size.computed_size().px();
        let font_family = match font.font_family.families.iter().next() {
            Some(stylo::SingleFontFamily::FamilyName(name)) => name.name.to_string(),
            Some(stylo::SingleFontFamily::Generic(generic)) => generic_name(*generic).to_owned(),
            None => "sans-serif".to_owned(),
        };
        drop(style);
        let row_height = (f64::from(font_size) * LINE_HEIGHT).round() + ROW_PADDING;
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout().size;
        let (x, top) = (f64::from(position.x), f64::from(position.y));
        let bottom = top + f64::from(size.height);
        let viewport = self.viewport();
        let scale = f64::from(viewport.scale());
        let scroll_y = self.viewport_scroll().y;
        let viewport_bottom = scroll_y + f64::from(viewport.window_size.1) / scale;
        let room_below = viewport_bottom - bottom;
        let room_above = top - scroll_y;

        let entries = self.popup_entries(popup.select);
        let wanted = entries.len().min(MAX_ROWS);
        let fits = |room: f64| {
            (0..=wanted)
                .rev()
                .find(|&rows| count(rows) * row_height + BORDER * 2.0 <= room)
                .unwrap_or(0)
        };
        let below = fits(room_below);
        let above = fits(room_above);
        let goes_below = below >= wanted || below >= above;
        let visible = if goes_below { below } else { above }
            .max(1)
            .min(entries.len());
        let first = popup.first_row.min(entries.len() - visible);
        let rows: Vec<PopupRow> = entries[first..first + visible]
            .iter()
            .map(|&entry| self.popup_row(popup, entry))
            .collect();
        let height = count(rows.len()) * row_height + BORDER * 2.0;
        Some(PopupLayout {
            x,
            y: if goes_below { bottom } else { top - height },
            width: f64::from(size.width),
            row_height,
            font_size,
            font_family,
            rows,
        })
    }

    pub(crate) fn select_popup_hit(&self, x: f32, y: f32) -> Option<HitResult> {
        let layout = self.select_popup_layout()?;
        if !layout.contains(f64::from(x), f64::from(y)) {
            return None;
        }
        let select = self.open_popup()?.select;
        let position = self.get_node(select)?.absolute_position(0.0, 0.0);
        Some(HitResult {
            node_id: select,
            is_text: false,
            x: x - position.x,
            y: y - position.y,
        })
    }

    pub fn select_popup_is_open(&self) -> bool {
        self.open_popup().is_some()
    }

    fn show_popup(&mut self, popup: Option<SelectPopup>) {
        self.select_popup = popup;
        self.shell_provider.request_redraw();
    }

    pub(crate) fn close_select_popup(&mut self) {
        if self.select_popup.is_some() {
            self.show_popup(None);
        }
    }

    fn open_select_popup(&mut self, select: NodeId) {
        let highlighted = self.selected_option(select);
        let mut popup = SelectPopup {
            select,
            highlighted,
            first_row: 0,
        };
        popup.first_row = self.first_row_showing(popup);
        self.show_popup(Some(popup));
    }

    fn first_row_showing(&self, popup: SelectPopup) -> usize {
        let rows = self.popup_entries(popup.select);
        let highlighted = |entry: &Entry| !entry.heading && popup.highlighted == Some(entry.node);
        let Some(index) = rows.iter().position(highlighted) else {
            return popup.first_row;
        };
        let visible = self
            .select_popup_layout()
            .map_or(MAX_ROWS.min(rows.len()), |layout| layout.rows.len())
            .max(1);
        if index < popup.first_row {
            index
        } else if index >= popup.first_row + visible {
            index + 1 - visible
        } else {
            popup.first_row
        }
    }

    fn highlight(&mut self, option: Option<NodeId>) {
        let Some(mut popup) = self.open_popup() else {
            return;
        };
        if popup.highlighted == option {
            return;
        }
        popup.highlighted = option;
        self.select_popup = Some(popup);
        popup.first_row = self.first_row_showing(popup);
        self.show_popup(Some(popup));
    }

    pub(crate) fn dismiss_select_popup_outside(&mut self, x: f32, y: f32) {
        let Some(popup) = self.open_popup() else {
            self.select_popup = None;
            return;
        };
        let (x, y) = (f64::from(x), f64::from(y));
        let over_popup = self
            .select_popup_layout()
            .is_some_and(|layout| layout.contains(x, y));
        let node = &self.nodes[popup.select];
        let position = node.absolute_position(0.0, 0.0);
        let size = node.final_layout().size;
        let (left, top) = (f64::from(position.x), f64::from(position.y));
        let over_select = x >= left
            && x < left + f64::from(size.width)
            && y >= top
            && y < top + f64::from(size.height);
        if !over_popup && !over_select {
            self.close_select_popup();
        }
    }

    pub(crate) fn hover_select_popup(&mut self, x: f32, y: f32) {
        let Some(layout) = self.select_popup_layout() else {
            return;
        };
        let row = layout.row_at(f64::from(x), f64::from(y));
        if let Some(option) = row.filter(|row| !row.disabled).map(|row| row.node) {
            self.highlight(Some(option));
        }
    }

    pub(crate) fn scroll_select_popup(&mut self, x: f32, y: f32, rows: isize) -> bool {
        let Some(mut popup) = self.open_popup() else {
            return false;
        };
        let over = self
            .select_popup_layout()
            .is_some_and(|layout| layout.contains(f64::from(x), f64::from(y)));
        if !over {
            return false;
        }
        let total = self.popup_entries(popup.select).len();
        popup.first_row = popup
            .first_row
            .saturating_add_signed(rows)
            .min(total.saturating_sub(1));
        self.show_popup(Some(popup));
        true
    }

    fn commit_option(
        &mut self,
        select: NodeId,
        option: NodeId,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) {
        if self.selected_option(select) == Some(option) {
            return;
        }
        self.set_selected_option(select, option);
        self.shell_provider.request_redraw();
        let value = self.option_value(option);
        dispatch_event(DomEvent::new(
            select,
            DomEventData::Input(PageframeInputEvent { value }),
        ));
    }

    pub(crate) fn click_select(
        &mut self,
        select: NodeId,
        x: f32,
        y: f32,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) {
        if self.select_is_list(select) {
            return;
        }
        let open = self.open_popup().filter(|popup| popup.select == select);
        let Some(layout) = open.and_then(|_| self.select_popup_layout()) else {
            self.open_select_popup(select);
            return;
        };
        let (x, y) = (f64::from(x), f64::from(y));
        match layout.row_at(x, y) {
            Some(row) if !row.disabled => {
                let option = row.node;
                self.commit_option(select, option, dispatch_event);
                self.close_select_popup();
            }
            Some(_) => {}
            None => self.close_select_popup(),
        }
    }

    fn stepped_option(&self, select: NodeId, from: Option<NodeId>, step: Step) -> Option<NodeId> {
        let options: Vec<NodeId> = self
            .select_options(select)
            .into_iter()
            .filter(|&option| !self.option_is_disabled(option))
            .collect();
        let index = from.and_then(|from| options.iter().position(|&option| option == from));
        let target = match (step, index) {
            (Step::First, _) | (Step::Next, None) => 0,
            (Step::Last, _) | (Step::Previous, None) => options.len().checked_sub(1)?,
            (Step::Next, Some(index)) => (index + 1).min(options.len() - 1),
            (Step::Previous, Some(index)) => index.saturating_sub(1),
            (Step::PageDown, index) => {
                (index.unwrap_or(0) + PAGE_STEP).min(options.len().checked_sub(1)?)
            }
            (Step::PageUp, index) => index.unwrap_or(0).saturating_sub(PAGE_STEP),
        };
        options.get(target).copied()
    }

    fn typed_option(&self, select: NodeId, from: Option<NodeId>, typed: &str) -> Option<NodeId> {
        let typed = typed.to_lowercase();
        let options: Vec<NodeId> = self
            .select_options(select)
            .into_iter()
            .filter(|&option| !self.option_is_disabled(option))
            .collect();
        let start = from
            .and_then(|from| options.iter().position(|&option| option == from))
            .map_or(0, |index| index + 1);
        let matches = |option: &NodeId| {
            self.option_label(*option)
                .to_lowercase()
                .starts_with(&typed)
        };
        options[start.min(options.len())..]
            .iter()
            .chain(&options[..start.min(options.len())])
            .find(|option| matches(option))
            .copied()
    }

    pub(crate) fn select_key(
        &mut self,
        select: NodeId,
        key: &Key,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> bool {
        if self.select_is_list(select) {
            return false;
        }
        let open = self.open_popup().filter(|popup| popup.select == select);
        let current = match open {
            Some(popup) => popup.highlighted,
            None => self.selected_option(select),
        };
        let step = match key {
            Key::Named(NamedKey::ArrowDown | NamedKey::ArrowRight) => Some(Step::Next),
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowLeft) => Some(Step::Previous),
            Key::Named(NamedKey::Home) => Some(Step::First),
            Key::Named(NamedKey::End) => Some(Step::Last),
            Key::Named(NamedKey::PageDown) => Some(Step::PageDown),
            Key::Named(NamedKey::PageUp) => Some(Step::PageUp),
            _ => None,
        };
        let target = match (step, key) {
            (Some(step), _) => self.stepped_option(select, current, step),
            (None, Key::Character(typed)) if typed.as_str() != " " => {
                self.typed_option(select, current, typed)
            }
            _ => None,
        };
        match (open, target, key) {
            (Some(_), Some(option), _) => self.highlight(Some(option)),
            (None, Some(option), _) => self.commit_option(select, option, dispatch_event),
            (Some(popup), None, Key::Named(NamedKey::Enter | NamedKey::Tab)) => {
                if let Some(option) = popup.highlighted {
                    self.commit_option(select, option, dispatch_event);
                }
                self.close_select_popup();
            }
            (Some(popup), None, Key::Character(typed)) if typed.as_str() == " " => {
                if let Some(option) = popup.highlighted {
                    self.commit_option(select, option, dispatch_event);
                }
                self.close_select_popup();
            }
            (Some(_), None, Key::Named(NamedKey::Escape)) => self.close_select_popup(),
            (None, None, Key::Character(typed)) if typed.as_str() == " " => {
                self.open_select_popup(select);
            }
            (None, None, Key::Named(NamedKey::F4)) => self.open_select_popup(select),
            _ => return step.is_some(),
        }
        true
    }
}
