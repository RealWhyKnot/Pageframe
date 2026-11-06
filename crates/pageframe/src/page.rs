use blitz_dom::{DocGuard, DocGuardMut, Document, EventDriver, EventHandler};
use blitz_html::HtmlDocument;
use blitz_traits::NodeId;
use blitz_traits::events::{DomEvent, EventState, UiEvent};
use selectors::SelectorList;
use style::selector_parser::SelectorImpl;

use crate::event::Event;
use crate::events::EventKind;
use crate::ui::Ui;

pub(crate) type Callback<S> = Box<dyn FnMut(&mut S, &mut Ui<'_>, &mut Event<'_>)>;

pub(crate) struct Handler<S> {
    pub(crate) kind: EventKind,
    pub(crate) selector: SelectorList<SelectorImpl>,
    pub(crate) callback: Callback<S>,
}

pub struct Page<S> {
    doc: HtmlDocument,
    state: S,
    handlers: Vec<Handler<S>>,
}

impl<S> Page<S> {
    pub(crate) fn new(doc: HtmlDocument, state: S, handlers: Vec<Handler<S>>) -> Self {
        Self {
            doc,
            state,
            handlers,
        }
    }

    pub fn state(&self) -> &S {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    pub fn ui(&mut self) -> Ui<'_> {
        Ui::new(&mut self.doc)
    }

    pub fn update(&mut self, change: impl FnOnce(&mut S, &mut Ui<'_>)) {
        change(&mut self.state, &mut Ui::new(&mut self.doc));
    }
}

struct Dispatch<'p, S> {
    state: &'p mut S,
    handlers: &'p mut [Handler<S>],
}

impl<S> EventHandler for Dispatch<'_, S> {
    fn handle_event(
        &mut self,
        chain: &[NodeId],
        event: &mut DomEvent,
        doc: &mut dyn Document,
        event_state: &mut EventState,
    ) {
        let kind = event.data.kind();
        if !self.handlers.iter().any(|handler| handler.kind == kind) {
            return;
        }
        let mut guard = doc.inner_mut();
        for &node in chain {
            for handler in self.handlers.iter_mut().filter(|h| h.kind == kind) {
                let matches = guard
                    .get_node(node)
                    .is_some_and(|n| n.matches_selector_raw(&handler.selector));
                if matches {
                    let mut ui = Ui::new(&mut guard);
                    let mut current = Event::new(&event.data, event.target, node, event_state);
                    (handler.callback)(self.state, &mut ui, &mut current);
                }
            }
            if event_state.propagation_is_stopped() {
                break;
            }
        }
        if guard.get_node(event.target).is_none() {
            event_state.prevent_default();
        }
        drop(guard);
    }
}

impl<S: 'static> Document for Page<S> {
    fn inner(&self) -> DocGuard<'_> {
        self.doc.inner()
    }

    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        self.doc.inner_mut()
    }

    fn handle_ui_event(&mut self, event: UiEvent) {
        let dispatch = Dispatch {
            state: &mut self.state,
            handlers: &mut self.handlers,
        };
        let mut driver = EventDriver::new(&mut self.doc, dispatch);
        driver.handle_ui_event(event);
    }
}
