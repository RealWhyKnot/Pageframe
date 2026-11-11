use keyboard_types::Key;
use pageframe_traits::NodeId;
use pageframe_traits::events::EventState;

use crate::events::{EventData, EventKind};

pub struct Event<'e> {
    data: &'e EventData,
    target: NodeId,
    current: NodeId,
    state: &'e mut EventState,
}

impl<'e> Event<'e> {
    pub(crate) fn new(
        data: &'e EventData,
        target: NodeId,
        current: NodeId,
        state: &'e mut EventState,
    ) -> Self {
        Self {
            data,
            target,
            current,
            state,
        }
    }

    pub fn kind(&self) -> EventKind {
        self.data.kind()
    }

    pub fn data(&self) -> &EventData {
        self.data
    }

    pub fn target(&self) -> NodeId {
        self.target
    }

    pub fn current_target(&self) -> NodeId {
        self.current
    }

    pub fn value(&self) -> Option<&str> {
        match self.data {
            EventData::Input(input) => Some(&input.value),
            _ => None,
        }
    }

    pub fn key(&self) -> Option<&Key> {
        match self.data {
            EventData::KeyDown(key) | EventData::KeyUp(key) | EventData::KeyPress(key) => {
                Some(&key.key)
            }
            _ => None,
        }
    }

    pub fn prevent_default(&mut self) {
        self.state.prevent_default();
    }

    pub fn stop_propagation(&mut self) {
        self.state.stop_propagation();
    }
}
