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
        BlitzFocusEvent as FocusEvent, BlitzImeEvent as ImeEvent, BlitzInputEvent as InputEvent,
        BlitzKeyEvent as KeyEvent, BlitzPointerEvent as PointerEvent, BlitzPointerId as PointerId,
        BlitzScrollEvent as ScrollEvent, BlitzWheelDelta as WheelDelta,
        BlitzWheelEvent as WheelEvent, DomEventData as EventData, DomEventKind as EventKind,
        KeyState, MouseEventButton, MouseEventButtons, Point, PointerCoords, PointerDetails,
    };
}

pub use app::App;
pub use error::Error;
pub use event::Event;
pub use events::{EventData, EventKind, Key, NamedKey};
pub use page::Page;
pub use pageframe_traits::NodeId;
pub use ui::{Selection, Ui};
