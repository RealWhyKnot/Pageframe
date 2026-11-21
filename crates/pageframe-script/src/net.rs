use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::task::Waker;

use pageframe_traits::shell::ShellProvider;

use crate::http::{HttpClient, HttpRequest, HttpResponse};

pub(crate) enum NetEvent {
    Response {
        id: u64,
        result: Result<HttpResponse, String>,
    },
    StreamOpen {
        id: u64,
    },
    StreamMessage {
        id: u64,
        kind: String,
        data: String,
        last_event_id: String,
    },
    StreamRetry {
        id: u64,
        millis: u64,
    },
    StreamEnd {
        id: u64,
        error: String,
        reconnect: bool,
    },
}

#[derive(Clone)]
struct Notifier {
    sender: Sender<NetEvent>,
    waker: Arc<Mutex<Option<Waker>>>,
    shell: Option<Arc<dyn ShellProvider>>,
}

impl Notifier {
    fn send(&self, event: NetEvent) -> bool {
        if self.sender.send(event).is_err() {
            return false;
        }
        if let Some(waker) = self.waker.lock().ok().and_then(|waker| waker.clone()) {
            waker.wake();
        }
        if let Some(shell) = &self.shell {
            shell.request_redraw();
        }
        true
    }
}

pub(crate) struct NetQueue {
    sender: Sender<NetEvent>,
    receiver: Receiver<NetEvent>,
    pub(crate) waker: Arc<Mutex<Option<Waker>>>,
    next_id: u64,
    in_flight: HashSet<u64>,
    streams: HashMap<u64, Arc<AtomicBool>>,
}

impl Default for NetQueue {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver,
            waker: Arc::new(Mutex::new(None)),
            next_id: 0,
            in_flight: HashSet::new(),
            streams: HashMap::new(),
        }
    }
}

impl NetQueue {
    fn notifier(&self, shell: Option<Arc<dyn ShellProvider>>) -> Notifier {
        Notifier {
            sender: self.sender.clone(),
            waker: Arc::clone(&self.waker),
            shell,
        }
    }

    fn allocate(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    pub(crate) fn drain(&mut self) -> Vec<NetEvent> {
        let events: Vec<NetEvent> = self.receiver.try_iter().collect();
        events
            .into_iter()
            .filter(|event| match event {
                NetEvent::Response { id, .. } => {
                    self.in_flight.remove(id);
                    true
                }
                NetEvent::StreamOpen { id }
                | NetEvent::StreamMessage { id, .. }
                | NetEvent::StreamRetry { id, .. } => self.streams.contains_key(id),
                NetEvent::StreamEnd { id, .. } => self.streams.remove(id).is_some(),
            })
            .collect()
    }

    pub(crate) fn start_request(
        &mut self,
        client: HttpClient,
        request: HttpRequest,
        shell: Option<Arc<dyn ShellProvider>>,
    ) -> u64 {
        let id = self.allocate();
        self.in_flight.insert(id);
        let notifier = self.notifier(shell);
        let spawned = std::thread::Builder::new()
            .name("pageframe-script-fetch".to_string())
            .spawn(move || {
                let result = client.request(&request).map_err(|error| error.to_string());
                notifier.send(NetEvent::Response { id, result });
            });
        if let Err(error) = spawned {
            let _ = self.sender.send(NetEvent::Response {
                id,
                result: Err(error.to_string()),
            });
        }
        id
    }

    pub(crate) fn open_stream(
        &mut self,
        client: HttpClient,
        request: HttpRequest,
        shell: Option<Arc<dyn ShellProvider>>,
    ) -> u64 {
        let id = self.allocate();
        let closed = Arc::new(AtomicBool::new(false));
        self.streams.insert(id, Arc::clone(&closed));
        let notifier = self.notifier(shell);
        let spawned = std::thread::Builder::new()
            .name("pageframe-script-event-source".to_string())
            .spawn(move || run_stream(id, &client, &request, &closed, &notifier));
        if let Err(error) = spawned {
            let _ = self.sender.send(NetEvent::StreamEnd {
                id,
                error: error.to_string(),
                reconnect: false,
            });
        }
        id
    }

    pub(crate) fn close_stream(&mut self, id: u64) {
        if let Some(closed) = self.streams.remove(&id) {
            closed.store(true, Ordering::Relaxed);
        }
    }
}

fn run_stream(
    id: u64,
    client: &HttpClient,
    request: &HttpRequest,
    closed: &AtomicBool,
    notifier: &Notifier,
) {
    let end = |error: String, reconnect: bool| {
        notifier.send(NetEvent::StreamEnd {
            id,
            error,
            reconnect,
        });
    };
    let (response, body) = match client.open(request) {
        Ok(opened) => opened,
        Err(error) => return end(error.to_string(), true),
    };
    let is_event_stream = response.header("content-type").is_some_and(|value| {
        value
            .trim()
            .to_ascii_lowercase()
            .starts_with("text/event-stream")
    });
    if response.status != 200 || !is_event_stream {
        return end(
            format!("event stream answered with HTTP {}", response.status),
            false,
        );
    }
    if !notifier.send(NetEvent::StreamOpen { id }) {
        return;
    }
    let mut reader = BufReader::new(body);
    let mut parser = StreamParser::default();
    let mut line = String::new();
    loop {
        if closed.load(Ordering::Relaxed) {
            return;
        }
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return end("event stream closed".to_string(), true),
            Ok(_) => {}
            Err(error) => return end(error.to_string(), true),
        }
        if closed.load(Ordering::Relaxed) {
            return;
        }
        if let Some(event) = parser.feed(id, line.trim_end_matches(['\r', '\n']))
            && !notifier.send(event)
        {
            return;
        }
    }
}

#[derive(Default)]
struct StreamParser {
    kind: String,
    data: Vec<String>,
    last_event_id: String,
}

impl StreamParser {
    fn feed(&mut self, id: u64, line: &str) -> Option<NetEvent> {
        if line.is_empty() {
            if self.data.is_empty() {
                self.kind.clear();
                return None;
            }
            let kind = std::mem::take(&mut self.kind);
            let data = std::mem::take(&mut self.data).join("\n");
            return Some(NetEvent::StreamMessage {
                id,
                kind: if kind.is_empty() {
                    "message".to_string()
                } else {
                    kind
                },
                data,
                last_event_id: self.last_event_id.clone(),
            });
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => self.kind = value.to_string(),
            "data" => self.data.push(value.to_string()),
            "id" if !value.contains('\0') => self.last_event_id = value.to_string(),
            "retry" => {
                if let Ok(millis) = value.parse::<u64>() {
                    return Some(NetEvent::StreamRetry { id, millis });
                }
            }
            _ => {}
        }
        None
    }
}
