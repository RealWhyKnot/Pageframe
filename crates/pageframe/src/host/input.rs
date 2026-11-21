use std::sync::Arc;

use keyboard_types::Modifiers;
use pageframe_traits::events::{
    MouseEventButton, MouseEventButtons, PageframeImeEvent, PageframeKeyEvent,
    PageframePointerEvent, PageframePointerId, PageframeWheelDelta, PageframeWheelEvent,
    PointerCoords, PointerDetails, UiEvent,
};

#[derive(Clone, Debug)]
pub enum HostInputEvent {
    Resized {
        width: u32,
        height: u32,
    },
    ScaleFactorChanged(f64),
    PointerMoved {
        x: f64,
        y: f64,
    },
    PointerDown {
        x: f64,
        y: f64,
        button: MouseEventButton,
    },
    PointerUp {
        x: f64,
        y: f64,
        button: MouseEventButton,
    },
    PointerLeft,
    Wheel(PageframeWheelDelta),
    KeyDown(PageframeKeyEvent),
    KeyUp(PageframeKeyEvent),
    Ime(PageframeImeEvent),
    Paste(String),
    ModifiersChanged(Modifiers),
    Focused(bool),
}

#[derive(Default)]
pub(super) struct PointerState {
    x: f64,
    y: f64,
    buttons: MouseEventButtons,
    mods: Modifiers,
}

fn narrow(value: f64) -> f32 {
    num_traits::cast(value).unwrap_or_default()
}

impl PointerState {
    pub(super) fn set_modifiers(&mut self, mods: Modifiers) {
        self.mods = mods;
    }

    pub(super) fn moved(&mut self, x: f64, y: f64) -> bool {
        let changed = (self.x - x).abs() > f64::EPSILON || (self.y - y).abs() > f64::EPSILON;
        self.x = x;
        self.y = y;
        changed
    }

    pub(super) fn press(&mut self, button: MouseEventButton) {
        self.buttons.insert(button.into());
    }

    pub(super) fn release(&mut self, button: MouseEventButton) {
        self.buttons.remove(button.into());
    }

    fn coords(&self, scale: f64, scroll: (f64, f64)) -> PointerCoords {
        let scale = if super::session::valid_scale(scale) {
            scale
        } else {
            1.0
        };
        let client_x = self.x / scale;
        let client_y = self.y / scale;
        PointerCoords {
            screen_x: narrow(client_x),
            screen_y: narrow(client_y),
            client_x: narrow(client_x),
            client_y: narrow(client_y),
            page_x: narrow(client_x + scroll.0),
            page_y: narrow(client_y + scroll.1),
        }
    }

    pub(super) fn pointer(
        &self,
        button: MouseEventButton,
        scale: f64,
        scroll: (f64, f64),
    ) -> PageframePointerEvent {
        PageframePointerEvent {
            id: PageframePointerId::Mouse,
            is_primary: true,
            coords: self.coords(scale, scroll),
            button,
            buttons: self.buttons,
            mods: self.mods,
            details: PointerDetails::default(),
            element: pageframe_traits::events::Point::default(),
            active_pointers: Arc::default(),
        }
    }

    pub(super) fn wheel(
        &self,
        delta: PageframeWheelDelta,
        scale: f64,
        scroll: (f64, f64),
    ) -> UiEvent {
        UiEvent::Wheel(PageframeWheelEvent {
            delta,
            coords: self.coords(scale, scroll),
            buttons: self.buttons,
            mods: self.mods,
            element: pageframe_traits::events::Point::default(),
        })
    }
}
