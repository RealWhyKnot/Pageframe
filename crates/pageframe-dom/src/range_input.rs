use keyboard_types::{Key, NamedKey};
use markup5ever::{LocalName, local_name};
use pageframe_traits::events::DomEvent;
use pageframe_traits::node_id::NodeId;

use crate::BaseDocument;
use crate::events::{DragMode, dispatch_change, dispatch_input, focus_node};
use crate::node::{ElementData, SpecialElementData, parse_html_float};

pub const RANGE_THUMB_SIZE: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeBounds {
    pub minimum: f64,
    pub maximum: f64,
    pub step: Option<f64>,
    base: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RangeDragState {
    pub(crate) node_id: NodeId,
    pub(crate) start_value: f64,
}

impl RangeBounds {
    pub fn of(element: &ElementData) -> Self {
        let number = |name: LocalName| element.attr(name).and_then(parse_html_float);
        let minimum = number(local_name!("min")).unwrap_or(0.0);
        let maximum = number(local_name!("max")).unwrap_or(100.0).max(minimum);
        let step = match element.attr(local_name!("step")) {
            Some(step) if step.eq_ignore_ascii_case("any") => None,
            Some(step) => Some(
                parse_html_float(step)
                    .filter(|step| *step > 0.0)
                    .unwrap_or(1.0),
            ),
            None => Some(1.0),
        };
        let base = number(local_name!("min"))
            .or_else(|| number(local_name!("value")))
            .unwrap_or(0.0);
        Self {
            minimum,
            maximum,
            step,
            base,
        }
    }

    fn half_span(&self) -> f64 {
        self.maximum / 2.0 - self.minimum / 2.0
    }

    pub fn default_value(&self) -> f64 {
        self.sanitize(self.minimum + self.half_span())
    }

    pub fn sanitize(&self, value: f64) -> f64 {
        if self.half_span() <= 0.0 {
            return self.minimum;
        }
        let value = if value.is_nan() {
            self.minimum + self.half_span()
        } else {
            value
        }
        .clamp(self.minimum, self.maximum);
        let Some(step) = self.step else {
            return value;
        };
        let steps = ((value / 2.0 - self.base / 2.0) / step * 2.0 + 0.5).floor();
        let rounded = (self.base / 2.0 + steps * (step / 2.0)) * 2.0;
        if !rounded.is_finite() {
            value
        } else if rounded > self.maximum {
            rounded - step
        } else if rounded < self.minimum {
            rounded + step
        } else {
            rounded
        }
    }

    pub fn fraction(&self, value: f64) -> f64 {
        let half_span = self.half_span();
        if half_span <= 0.0 {
            return 0.0;
        }
        ((value / 2.0 - self.minimum / 2.0) / half_span).clamp(0.0, 1.0)
    }

    pub fn value_at_fraction(&self, fraction: f64) -> f64 {
        let fraction = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        self.sanitize((self.minimum / 2.0 + fraction * self.half_span()) * 2.0)
    }

    fn key_delta(&self) -> f64 {
        self.step.unwrap_or(self.half_span() / 50.0)
    }

    fn page_delta(&self) -> f64 {
        (self.half_span() / 5.0).max(self.key_delta())
    }

    pub fn value_for_key(&self, value: f64, key: &Key) -> Option<f64> {
        let next = match key {
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => value + self.key_delta(),
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => value - self.key_delta(),
            Key::Named(NamedKey::PageUp) => value + self.page_delta(),
            Key::Named(NamedKey::PageDown) => value - self.page_delta(),
            Key::Named(NamedKey::Home) => self.minimum,
            Key::Named(NamedKey::End) => self.maximum,
            _ => return None,
        };
        Some(self.sanitize(next))
    }
}

impl ElementData {
    pub fn is_range_input(&self) -> bool {
        self.name.local == local_name!("input") && self.input_type().as_deref() == Some("range")
    }

    pub fn range_bounds(&self) -> RangeBounds {
        RangeBounds::of(self)
    }

    pub fn range_value(&self) -> f64 {
        let bounds = self.range_bounds();
        let raw = match self.special_data {
            SpecialElementData::RangeInput(value) => Some(value),
            _ => self.attr(local_name!("value")).and_then(parse_html_float),
        };
        raw.map_or_else(|| bounds.default_value(), |value| bounds.sanitize(value))
    }

    pub fn set_range_value(&mut self, value: f64) {
        self.special_data = SpecialElementData::RangeInput(self.range_bounds().sanitize(value));
    }

    pub fn range_input_fraction(&self) -> f64 {
        self.range_bounds().fraction(self.range_value())
    }
}

pub fn format_input_number(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }
    let plain = value.to_string();
    let exponent = value.abs().log10().floor();
    if (-6.0..21.0).contains(&exponent) {
        return plain;
    }
    let scientific = format!("{value:e}");
    match scientific.split_once('e') {
        Some((mantissa, exponent)) if exponent.starts_with('-') => format!("{mantissa}e{exponent}"),
        Some((mantissa, exponent)) => format!("{mantissa}e+{exponent}"),
        None => scientific,
    }
}

impl BaseDocument {
    pub fn set_range_input_value(&mut self, node_id: NodeId, value: f64) -> bool {
        let Some(element) = self
            .nodes
            .get_mut(node_id)
            .and_then(|n| n.element_data_mut())
        else {
            return false;
        };
        let before = element.range_value();
        element.set_range_value(value);
        let changed = element.range_value() != before;
        self.nodes[node_id].mark_ancestors_dirty();
        self.shell_provider.request_redraw();
        changed
    }

    pub fn set_range_input_text(&mut self, node_id: NodeId, text: &str) -> bool {
        let value = parse_html_float(text).unwrap_or(f64::NAN);
        self.set_range_input_value(node_id, value)
    }

    fn range_track_value(&self, node_id: NodeId, x: f32) -> Option<f64> {
        let node = self.nodes.get(node_id)?;
        let element = node.element_data()?;
        let layout = node.final_layout();
        let origin = node.absolute_position(0.0, 0.0);
        let left =
            f64::from(origin.x) + f64::from(layout.border.left) + f64::from(layout.padding.left);
        let width = f64::from(layout.content_box_width());
        let travel = (width - RANGE_THUMB_SIZE).max(0.0);
        let offset = f64::from(x) - left - RANGE_THUMB_SIZE.min(width) / 2.0;
        let fraction = if travel > 0.0 { offset / travel } else { 0.0 };
        Some(element.range_bounds().value_at_fraction(fraction))
    }

    fn range_value_of(&self, node_id: NodeId) -> f64 {
        self.nodes
            .get(node_id)
            .and_then(|n| n.element_data())
            .map_or(0.0, ElementData::range_value)
    }

    fn move_range_to(
        &mut self,
        node_id: NodeId,
        value: f64,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> bool {
        if !self.set_range_input_value(node_id, value) {
            return false;
        }
        let value = format_input_number(self.range_value_of(node_id));
        dispatch_input(node_id, value, dispatch_event);
        true
    }

    pub(crate) fn start_range_drag(
        &mut self,
        node_id: NodeId,
        x: f32,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) {
        let start_value = self.range_value_of(node_id);
        self.drag_mode = DragMode::Range(RangeDragState {
            node_id,
            start_value,
        });
        focus_node(self, node_id, dispatch_event);
        if let Some(value) = self.range_track_value(node_id, x) {
            self.move_range_to(node_id, value, dispatch_event);
        }
    }

    pub(crate) fn drag_range(
        &mut self,
        state: RangeDragState,
        x: f32,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> bool {
        match self.range_track_value(state.node_id, x) {
            Some(value) => self.move_range_to(state.node_id, value, dispatch_event),
            None => false,
        }
    }

    pub(crate) fn finish_range_drag(
        &mut self,
        state: RangeDragState,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) {
        let value = self.range_value_of(state.node_id);
        if value != state.start_value {
            dispatch_change(state.node_id, format_input_number(value), dispatch_event);
        }
    }

    pub(crate) fn range_key(
        &mut self,
        node_id: NodeId,
        key: &Key,
        dispatch_event: &mut dyn FnMut(DomEvent),
    ) -> bool {
        let Some(element) = self.nodes.get(node_id).and_then(|n| n.element_data()) else {
            return false;
        };
        let Some(next) = element
            .range_bounds()
            .value_for_key(element.range_value(), key)
        else {
            return false;
        };
        if self.move_range_to(node_id, next, dispatch_event) {
            let value = format_input_number(self.range_value_of(node_id));
            dispatch_change(node_id, value, dispatch_event);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(minimum: f64, maximum: f64, step: Option<f64>) -> RangeBounds {
        RangeBounds {
            minimum,
            maximum,
            step,
            base: minimum,
        }
    }

    #[test]
    fn sanitize_clamps_and_snaps() {
        let b = bounds(0.0, 100.0, Some(1.0));
        assert_eq!(b.default_value(), 50.0);
        assert_eq!(b.sanitize(150.0), 100.0);
        assert_eq!(b.sanitize(-3.0), 0.0);
        assert_eq!(b.sanitize(33.4), 33.0);
        assert_eq!(b.sanitize(33.5), 34.0);
        let tens = bounds(0.0, 95.0, Some(10.0));
        assert_eq!(tens.sanitize(95.0), 90.0);
        assert_eq!(bounds(0.0, 1.0, None).sanitize(0.333), 0.333);
        assert_eq!(bounds(10.0, 10.0, Some(3.0)).sanitize(40.0), 10.0);
    }

    #[test]
    fn fractions_map_both_ways() {
        let b = bounds(-50.0, 50.0, Some(5.0));
        assert_eq!(b.fraction(0.0), 0.5);
        assert_eq!(b.value_at_fraction(0.26), -25.0);
        assert_eq!(b.value_at_fraction(-1.0), -50.0);
        assert_eq!(b.value_at_fraction(f64::NAN), -50.0);
        assert_eq!(b.value_at_fraction(2.0), 50.0);
        let huge = bounds(-1e308, 1e308, None);
        assert_eq!(huge.value_at_fraction(1.0), 1e308);
    }

    #[test]
    fn keys_step_page_and_jump() {
        let b = bounds(0.0, 100.0, Some(1.0));
        let key = |named| Key::Named(named);
        assert_eq!(
            b.value_for_key(50.0, &key(NamedKey::ArrowRight)),
            Some(51.0)
        );
        assert_eq!(b.value_for_key(50.0, &key(NamedKey::ArrowDown)), Some(49.0));
        assert_eq!(b.value_for_key(50.0, &key(NamedKey::PageUp)), Some(60.0));
        assert_eq!(b.value_for_key(5.0, &key(NamedKey::PageDown)), Some(0.0));
        assert_eq!(b.value_for_key(50.0, &key(NamedKey::Home)), Some(0.0));
        assert_eq!(b.value_for_key(50.0, &key(NamedKey::End)), Some(100.0));
        assert_eq!(b.value_for_key(50.0, &key(NamedKey::Enter)), None);
        let any = bounds(0.0, 1.0, None);
        assert_eq!(any.value_for_key(0.5, &key(NamedKey::ArrowUp)), Some(0.51));
    }

    #[test]
    fn values_format_like_javascript_numbers() {
        assert_eq!(format_input_number(50.0), "50");
        assert_eq!(format_input_number(-0.0), "0");
        assert_eq!(format_input_number(33.3), "33.3");
        assert_eq!(format_input_number(1e308), "1e+308");
        assert_eq!(format_input_number(1e-7), "1e-7");
        assert_eq!(format_input_number(0.000001), "0.000001");
    }
}
