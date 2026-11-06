use blitz_traits::NodeId;
use blitz_traits::events::{DomEventData, DomEventKind, EventState};
use keyboard_types::Key;

pub struct Event<'e> {
    data: &'e DomEventData,
    target: NodeId,
    current: NodeId,
    state: &'e mut EventState,
}

impl<'e> Event<'e> {
    pub(crate) fn new(
        data: &'e DomEventData,
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

    pub fn kind(&self) -> DomEventKind {
        self.data.kind()
    }

    pub fn data(&self) -> &DomEventData {
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
            DomEventData::Input(input) => Some(&input.value),
            _ => None,
        }
    }

    pub fn key(&self) -> Option<&Key> {
        match self.data {
            DomEventData::KeyDown(key) | DomEventData::KeyUp(key) | DomEventData::KeyPress(key) => {
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
