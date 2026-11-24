use std::cell::RefCell;
use std::rc::Rc;

use pageframe::NamedKey;
use pageframe::events::{Code, Key, KeyEvent, KeyState, Location, Modifiers, MouseEventButton};
use pageframe::host::{HostConfig, HostInputEvent, HostSession};
use url::Url;

fn session(html: &str) -> (HostSession, Rc<RefCell<Vec<String>>>) {
    let base = Url::parse("http://app.test/").expect("a valid base URL");
    let mut session =
        HostSession::new(HostConfig::html(html, Some(base))).expect("inline HTML always loads");
    let posts = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&posts);
    session.on_post_message(move |message| sink.borrow_mut().push(message.to_owned()));
    session.poll();
    session.take_events();
    (session, posts)
}

fn taken(posts: &Rc<RefCell<Vec<String>>>) -> Vec<String> {
    std::mem::take(&mut *posts.borrow_mut())
}

fn assert_clean(session: &mut HostSession) {
    assert_eq!(
        session.document_mut().take_js_errors(),
        Vec::<String>::new()
    );
}

fn key(key: Key) -> KeyEvent {
    KeyEvent {
        key,
        code: Code::Unidentified,
        modifiers: Modifiers::empty(),
        location: Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: KeyState::Pressed,
        text: None,
    }
}

fn press(session: &mut HostSession, named: NamedKey) {
    session.handle_input(HostInputEvent::KeyDown(key(Key::Named(named))));
    session.handle_input(HostInputEvent::KeyUp(key(Key::Named(named))));
}

fn press_space(session: &mut HostSession) {
    session.handle_input(HostInputEvent::KeyDown(key(Key::Character(" ".into()))));
    session.handle_input(HostInputEvent::KeyUp(key(Key::Character(" ".into()))));
}

fn pointer_down(session: &mut HostSession, x: f64, y: f64) {
    let button = MouseEventButton::Main;
    session.handle_input(HostInputEvent::PointerDown { x, y, button });
}

fn pointer_up(session: &mut HostSession, x: f64, y: f64) {
    let button = MouseEventButton::Main;
    session.handle_input(HostInputEvent::PointerUp { x, y, button });
}

const SELECTS: &str = r#"<body style="margin:0">
<select id="drop"><option value="a">Apple</option><option value="b">Banana</option><option value="c">Cherry</option></select>
<select id="list" size="3"><option>one</option><option>two</option><option>three</option></select>
<script>
for (const id of ["drop", "list"]) {
    const select = document.getElementById(id);
    for (const type of ["input", "change"]) {
        select.addEventListener(type, (event) => ipc.postMessage(
            [id, type, select.value, select.selectedIndex, event.bubbles, event.composed,
             event.isTrusted, event.cancelable].join(":")));
    }
}
document.body.addEventListener("change", (event) => ipc.postMessage("body-change:" + event.target.id));
</script></body>"#;

#[test]
fn keyboard_selection_fires_input_then_change() {
    let (mut session, posts) = session(SELECTS);
    session.eval(r#"document.getElementById("drop").focus()"#);
    press(&mut session, NamedKey::ArrowDown);
    assert_eq!(
        taken(&posts),
        [
            "drop:input:b:1:true:true:true:false",
            "drop:change:b:1:true:false:true:false",
            "body-change:drop",
        ]
    );
    press(&mut session, NamedKey::End);
    press(&mut session, NamedKey::End);
    assert_eq!(
        taken(&posts),
        [
            "drop:input:c:2:true:true:true:false",
            "drop:change:c:2:true:false:true:false",
            "body-change:drop",
        ]
    );
    assert_clean(&mut session);
}

#[test]
fn the_popup_commits_once_when_it_closes() {
    let (mut session, posts) = session(SELECTS);
    session.eval(r#"document.getElementById("drop").focus()"#);
    press_space(&mut session);
    press(&mut session, NamedKey::ArrowDown);
    press(&mut session, NamedKey::ArrowDown);
    assert_eq!(taken(&posts), Vec::<String>::new());
    press(&mut session, NamedKey::Enter);
    assert_eq!(
        taken(&posts),
        [
            "drop:input:c:2:true:true:true:false",
            "drop:change:c:2:true:false:true:false",
            "body-change:drop",
        ]
    );
    press_space(&mut session);
    press(&mut session, NamedKey::Enter);
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert_clean(&mut session);
}

#[test]
fn list_boxes_fire_input_then_change() {
    let (mut session, posts) = session(SELECTS);
    session.eval(r#"document.getElementById("list").focus()"#);
    press(&mut session, NamedKey::ArrowDown);
    assert_eq!(
        taken(&posts),
        [
            "list:input:one:0:true:true:true:false",
            "list:change:one:0:true:false:true:false",
            "body-change:list",
        ]
    );
    press(&mut session, NamedKey::ArrowDown);
    assert_eq!(
        taken(&posts),
        [
            "list:input:two:1:true:true:true:false",
            "list:change:two:1:true:false:true:false",
            "body-change:list",
        ]
    );
    assert_clean(&mut session);
}

#[test]
fn script_changes_to_a_select_fire_nothing() {
    let (mut session, posts) = session(SELECTS);
    session.eval(
        r#"
        const drop = document.getElementById("drop");
        drop.value = "c";
        drop.selectedIndex = 1;
        document.getElementById("list").value = "two";
        ipc.postMessage("done:" + drop.value + ":" + document.getElementById("list").value);
    "#,
    );
    assert_eq!(taken(&posts), ["done:b:two"]);
    assert_clean(&mut session);
}

const RANGE: &str = r#"<body style="margin:0">
<input type="range" id="r" style="position:absolute;left:0;top:0;width:116px;height:16px;margin:0;padding:0;border:0">
<input type="range" id="off" disabled style="position:absolute;left:0;top:40px;width:116px;height:16px;margin:0;padding:0;border:0">
<script>
for (const id of ["r", "off"]) {
    const range = document.getElementById(id);
    for (const type of ["input", "change"]) {
        range.addEventListener(type, (event) => ipc.postMessage(
            [id, type, range.value, event.bubbles, event.isTrusted].join(":")));
    }
}
</script></body>"#;

fn fraction(session: &HostSession, selector: &str) -> f64 {
    use pageframe_dom::Document;
    let doc = session.document().inner();
    let id = doc
        .query_selector_all(selector)
        .expect("the selector parses")[0];
    doc.get_node(id)
        .expect("the node exists")
        .element_data()
        .expect("inputs are elements")
        .range_input_fraction()
}

#[test]
fn clicking_and_dragging_a_range_updates_it_live() {
    let (mut session, posts) = session(RANGE);
    assert_eq!(fraction(&session, "#r"), 0.5);
    pointer_down(&mut session, 33.0, 8.0);
    assert_eq!(taken(&posts), ["r:input:25:true:true"]);
    assert_eq!(fraction(&session, "#r"), 0.25);
    session.handle_input(HostInputEvent::PointerMoved { x: 33.4, y: 8.0 });
    assert_eq!(taken(&posts), Vec::<String>::new());
    session.handle_input(HostInputEvent::PointerMoved { x: 78.0, y: 30.0 });
    session.handle_input(HostInputEvent::PointerMoved { x: 500.0, y: 300.0 });
    assert_eq!(
        taken(&posts),
        ["r:input:70:true:true", "r:input:100:true:true"]
    );
    assert_eq!(fraction(&session, "#r"), 1.0);
    pointer_up(&mut session, 500.0, 300.0);
    assert_eq!(taken(&posts), ["r:change:100:true:true"]);
    session.eval(r#"ipc.postMessage("value:" + r.value + ":" + r.valueAsNumber)"#);
    assert_eq!(taken(&posts), ["value:100:100"]);
    pointer_down(&mut session, 108.0, 8.0);
    pointer_up(&mut session, 108.0, 8.0);
    assert_eq!(taken(&posts), Vec::<String>::new());
    session.handle_input(HostInputEvent::PointerMoved { x: -50.0, y: 8.0 });
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert_clean(&mut session);
}

#[test]
fn dragging_back_to_the_start_value_fires_no_change() {
    let (mut session, posts) = session(RANGE);
    pointer_down(&mut session, 58.0, 8.0);
    assert_eq!(taken(&posts), Vec::<String>::new());
    session.handle_input(HostInputEvent::PointerMoved { x: 68.0, y: 8.0 });
    session.handle_input(HostInputEvent::PointerMoved { x: 58.0, y: 8.0 });
    pointer_up(&mut session, 58.0, 8.0);
    assert_eq!(
        taken(&posts),
        ["r:input:60:true:true", "r:input:50:true:true"]
    );
    assert_clean(&mut session);
}

#[test]
fn leaving_the_window_ends_a_range_drag() {
    let (mut session, posts) = session(RANGE);
    pointer_down(&mut session, 33.0, 8.0);
    assert_eq!(taken(&posts), ["r:input:25:true:true"]);
    session.handle_input(HostInputEvent::PointerLeft);
    assert_eq!(taken(&posts), ["r:change:25:true:true"]);
    session.handle_input(HostInputEvent::PointerMoved { x: 78.0, y: 8.0 });
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert_eq!(fraction(&session, "#r"), 0.25);
    assert_clean(&mut session);
}

#[test]
fn keys_step_a_focused_range() {
    let (mut session, posts) = session(RANGE);
    session.eval(r#"document.getElementById("r").focus()"#);
    press(&mut session, NamedKey::ArrowRight);
    press(&mut session, NamedKey::ArrowUp);
    press(&mut session, NamedKey::ArrowLeft);
    press(&mut session, NamedKey::PageUp);
    press(&mut session, NamedKey::PageDown);
    press(&mut session, NamedKey::PageDown);
    press(&mut session, NamedKey::Home);
    press(&mut session, NamedKey::Home);
    press(&mut session, NamedKey::ArrowDown);
    press(&mut session, NamedKey::End);
    let values: Vec<String> = taken(&posts)
        .into_iter()
        .filter_map(|post| post.strip_prefix("r:change:").map(str::to_owned))
        .collect();
    assert_eq!(
        values,
        [
            "51:true:true",
            "52:true:true",
            "51:true:true",
            "61:true:true",
            "51:true:true",
            "41:true:true",
            "0:true:true",
            "100:true:true"
        ]
    );
    assert_eq!(fraction(&session, "#r"), 1.0);
    assert_clean(&mut session);
}

#[test]
fn disabled_ranges_ignore_pointer_and_keys() {
    let (mut session, posts) = session(RANGE);
    pointer_down(&mut session, 100.0, 48.0);
    session.handle_input(HostInputEvent::PointerMoved { x: 10.0, y: 48.0 });
    pointer_up(&mut session, 10.0, 48.0);
    session.eval(r#"document.getElementById("off").focus()"#);
    press(&mut session, NamedKey::End);
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert_eq!(fraction(&session, "#off"), 0.5);
    assert_clean(&mut session);
}

#[test]
fn script_writes_to_a_range_are_sanitized_and_silent() {
    let (mut session, posts) = session(
        r#"<input type="range" id="r" min="0" max="10" step="2">
<input type="number" id="n" value="4.5"><input id="t" value="7">
<script>
const r = document.getElementById("r");
r.addEventListener("input", () => ipc.postMessage("input"));
r.addEventListener("change", () => ipc.postMessage("change"));
</script>"#,
    );
    session.eval(
        r#"
        const out = [r.value, r.valueAsNumber];
        r.value = "7"; out.push(r.value, r.getAttribute("value"));
        r.value = "-3"; out.push(r.value);
        r.value = "junk"; out.push(r.value);
        r.valueAsNumber = 9; out.push(r.value, r.valueAsNumber);
        r.setAttribute("max", "5"); out.push(r.value);
        try { r.valueAsNumber = Infinity; } catch (error) { out.push(error.name); }
        const n = document.getElementById("n");
        out.push(n.valueAsNumber);
        n.valueAsNumber = 12; out.push(n.value);
        out.push(Number.isNaN(document.getElementById("t").valueAsNumber));
        ipc.postMessage(out.join(","));
    "#,
    );
    assert_eq!(taken(&posts), ["6,6,8,,0,6,10,10,4,TypeError,4.5,12,true"]);
    assert_eq!(fraction(&session, "#r"), 0.8);
    assert_clean(&mut session);
}

const VISIBILITY: &str = r#"<script>
document.addEventListener("visibilitychange", (event) => ipc.postMessage(
    ["doc", document.hidden, document.visibilityState, event.bubbles, event.target === document].join(":")));
window.addEventListener("visibilitychange", () => ipc.postMessage("window"));
ipc.postMessage("start:" + document.hidden + ":" + document.visibilityState);
</script>"#;

#[test]
fn hiding_the_host_updates_visibility_and_fires_once() {
    let (mut session, posts) = session(VISIBILITY);
    assert_eq!(taken(&posts), ["start:false:visible"]);
    assert!(!session.is_hidden());
    assert!(session.set_hidden(true));
    assert!(session.is_hidden());
    assert_eq!(taken(&posts), ["doc:true:hidden:true:true", "window"]);
    assert!(!session.set_hidden(true));
    assert_eq!(taken(&posts), Vec::<String>::new());
    assert!(session.set_hidden(false));
    assert_eq!(taken(&posts), ["doc:false:visible:true:true", "window"]);
    assert!(session.set_hidden(true));
    assert_eq!(taken(&posts), ["doc:true:hidden:true:true", "window"]);
    session.eval(r#"ipc.postMessage(document.hidden + ":" + document.visibilityState)"#);
    assert_eq!(taken(&posts), ["true:hidden"]);
    assert_clean(&mut session);
}

#[test]
fn script_documents_toggle_visibility() {
    let base = Url::parse("http://app.test/").expect("a valid base URL");
    let mut session = HostSession::new(HostConfig::html("<p></p>", Some(base)))
        .expect("inline HTML always loads");
    let doc = session.document_mut();
    assert!(!doc.is_hidden());
    assert!(doc.set_hidden(true));
    assert!(doc.is_hidden());
    assert!(!doc.set_hidden(true));
    assert!(doc.set_hidden(false));
    assert!(!doc.is_hidden());
}
