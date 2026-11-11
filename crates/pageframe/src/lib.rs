mod app;
mod error;
mod event;
mod net;
mod page;
mod ui;

pub mod events {
    pub use keyboard_types::{Code, Key, Location, Modifiers, NamedKey};
    pub use pageframe_traits::SmolStr;
    pub use pageframe_traits::events::{
        DomEventData as EventData, DomEventKind as EventKind, KeyState, MouseEventButton,
        MouseEventButtons, PageframeFocusEvent as FocusEvent, PageframeImeEvent as ImeEvent,
        PageframeInputEvent as InputEvent, PageframeKeyEvent as KeyEvent,
        PageframePointerEvent as PointerEvent, PageframePointerId as PointerId,
        PageframeScrollEvent as ScrollEvent, PageframeWheelDelta as WheelDelta,
        PageframeWheelEvent as WheelEvent, Point, PointerCoords, PointerDetails,
    };
}

pub use app::App;
pub use error::Error;
pub use event::Event;
pub use events::{EventData, EventKind, Key, NamedKey};
pub use page::Page;
pub use pageframe_traits::NodeId;
pub use ui::{Selection, Ui};
