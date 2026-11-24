use std::cell::RefCell;
use std::rc::Rc;

use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::NamedKey;
use pageframe::events::{Code, Key, KeyEvent, KeyState, Location, Modifiers, MouseEventButton};
use pageframe::host::{HostConfig, HostInputEvent, HostSession};
use pageframe_dom::Document as _;
use pageframe_paint::paint_scene;
use url::Url;

const WIDTH: u32 = 200;
const HEIGHT: u32 = 150;
const ROW_PIXELS: usize = 200;

const WHITE: [u8; 3] = [255, 255, 255];
const YELLOW: [u8; 3] = [255, 255, 0];
const GREEN: [u8; 3] = [0, 128, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const RED_OVER_WHITE: [u8; 3] = [255, 128, 128];
const RED_OVER_YELLOW: [u8; 3] = [255, 128, 0];

const PAGE: &str = r#"<!doctype html><html><head><style>
html,body{margin:0;background:#fff}
#cover{position:absolute;left:60px;top:50px;width:80px;height:50px;z-index:9999;background:rgb(255,255,0)}
#clip{position:absolute;left:0;top:0;width:20px;height:20px;overflow:hidden;transform:translateX(5px)}
#opener{position:absolute;left:0;top:120px;width:40px;height:20px}
#outside{position:absolute;left:150px;top:120px;width:40px;height:20px}
dialog{padding:0;border:0;width:60px;height:40px;background:rgb(0,0,255)}
dialog:modal{background:rgb(0,128,0)}
#modal::backdrop{background:rgba(255,0,0,0.5)}
#first{position:absolute;left:0;top:0;width:20px;height:20px;border:0;padding:0}
#second{position:absolute;left:40px;top:20px;width:20px;height:20px;border:0;padding:0}
#plain{position:absolute;left:0;top:0;margin:0;width:40px;height:20px}
#plainButton{width:10px;height:10px;padding:0;border:0}
#inner{width:20px;height:16px;background:rgb(0,128,0)}
#deep{width:6px;height:6px;padding:0;border:0}
#inner::backdrop{background:rgba(0,0,255,0.5)}
</style></head><body>
<button id="opener">open</button>
<button id="outside">out</button>
<div id="cover"></div>
<div id="clip"><dialog id="modal"><button id="first">a</button><button id="second">b</button><dialog id="inner"><button id="deep">c</button></dialog></dialog></div>
<dialog id="plain"><button id="plainButton">p</button></dialog>
<script>
const report = (text) => ipc.postMessage(text);
addEventListener("click", (event) => report("click:" + (event.target.id || event.target.localName)));
for (const id of ["modal", "inner", "plain"]) {
    const dialog = document.getElementById(id);
    dialog.addEventListener("cancel", () => report("cancel:" + id));
    dialog.addEventListener("close", () => report("close:" + id));
}
globalThis.focused = () => report("focus:" + (document.activeElement ? document.activeElement.id || document.activeElement.localName : "none"));
</script></body></html>"#;

fn session() -> (HostSession, Rc<RefCell<Vec<String>>>) {
    let base = Url::parse("http://app.test/").expect("a valid base URL");
    let config = HostConfig::html(PAGE, Some(base)).size(f64::from(WIDTH), f64::from(HEIGHT));
    let mut session = HostSession::new(config).expect("inline HTML always loads");
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

fn focused(session: &mut HostSession, posts: &Rc<RefCell<Vec<String>>>) -> String {
    session.eval("focused()");
    let mut messages = taken(posts);
    assert_eq!(messages.len(), 1, "{messages:?}");
    messages.remove(0)
}

fn eval(session: &mut HostSession, posts: &Rc<RefCell<Vec<String>>>, code: &str) -> Vec<String> {
    session.eval(code);
    session.poll();
    taken(posts)
}

fn key(key: Key, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
        key,
        code: Code::Unidentified,
        modifiers,
        location: Location::Standard,
        is_auto_repeating: false,
        is_composing: false,
        state: KeyState::Pressed,
        text: None,
    }
}

fn press(session: &mut HostSession, named: NamedKey, modifiers: Modifiers) {
    session.handle_input(HostInputEvent::KeyDown(key(Key::Named(named), modifiers)));
    session.handle_input(HostInputEvent::KeyUp(key(Key::Named(named), modifiers)));
    session.poll();
}

fn click(session: &mut HostSession, x: f64, y: f64) {
    let button = MouseEventButton::Main;
    session.handle_input(HostInputEvent::PointerDown { x, y, button });
    session.handle_input(HostInputEvent::PointerUp { x, y, button });
    session.poll();
}

fn render(session: &mut HostSession) -> Vec<u8> {
    session.resolve();
    let mut inner = session.document_mut().inner_mut();
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut inner, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    );
    drop(inner);
    buffer
}

fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 3] {
    let start = (y * ROW_PIXELS + x) * 4;
    [buffer[start], buffer[start + 1], buffer[start + 2]]
}

fn assert_near(buffer: &[u8], x: usize, y: usize, expected: [u8; 3]) {
    let actual = pixel(buffer, x, y);
    let close = actual
        .iter()
        .zip(expected)
        .all(|(&a, e)| a.abs_diff(e) <= 3);
    assert!(
        close,
        "pixel ({x}, {y}) is {actual:?}, expected {expected:?}"
    );
}

fn assert_page_restored(buffer: &[u8]) {
    assert_near(buffer, 100, 75, YELLOW);
    assert_near(buffer, 100, 140, WHITE);
    assert_near(buffer, 70, 10, WHITE);
}

fn open_from_opener(session: &mut HostSession, posts: &Rc<RefCell<Vec<String>>>) {
    let opened = eval(
        session,
        posts,
        r#"document.getElementById("opener").focus(); document.getElementById("modal").showModal(); report(String(document.getElementById("modal").matches(":modal")));"#,
    );
    assert_eq!(opened, ["true"]);
}

#[test]
fn a_modal_matches_modal_and_paints_above_everything() {
    let (mut session, posts) = session();
    let before = render(&mut session);
    assert_page_restored(&before);

    open_from_opener(&mut session, &posts);
    let after = render(&mut session);
    for (x, y) in [(100, 75), (72, 92), (128, 57), (95, 60)] {
        assert_near(&after, x, y, GREEN);
    }
    assert_near(&after, 68, 75, RED_OVER_YELLOW);
    assert_near(&after, 132, 75, RED_OVER_YELLOW);
    assert_near(&after, 100, 52, RED_OVER_YELLOW);
    assert_near(&after, 100, 98, RED_OVER_YELLOW);
    assert_eq!(focused(&mut session, &posts), "focus:first");
    assert_clean(&mut session);
}

#[test]
fn b_the_backdrop_uses_the_author_colour_over_the_page() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);
    let buffer = render(&mut session);
    assert_near(&buffer, 5, 5, RED_OVER_WHITE);
    assert_near(&buffer, 195, 145, RED_OVER_WHITE);
    assert_near(&buffer, 65, 52, RED_OVER_YELLOW);
    assert_near(&buffer, 138, 98, RED_OVER_YELLOW);
    assert_clean(&mut session);
}

#[test]
fn b_the_backdrop_falls_back_to_the_user_agent_tint() {
    let (mut session, posts) = session();
    let opened = eval(
        &mut session,
        &posts,
        r#"document.getElementById("modal").id = "other"; document.getElementById("other").showModal();"#,
    );
    assert_eq!(opened, Vec::<String>::new());
    let buffer = render(&mut session);
    assert_near(&buffer, 5, 5, [230, 230, 230]);
    assert_near(&buffer, 65, 52, [230, 230, 0]);
    assert_clean(&mut session);
}

#[test]
fn c_the_page_behind_a_modal_is_inert() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);

    click(&mut session, 170.0, 130.0);
    assert_eq!(taken(&posts), ["click:modal"]);
    assert_eq!(focused(&mut session, &posts), "focus:first");

    click(&mut session, 20.0, 130.0);
    assert_eq!(taken(&posts), ["click:modal"]);

    click(&mut session, 120.0, 85.0);
    assert_eq!(taken(&posts), ["click:second"]);
    assert_eq!(focused(&mut session, &posts), "focus:first");

    click(&mut session, 100.0, 60.0);
    assert_eq!(taken(&posts), ["click:modal"]);

    eval(
        &mut session,
        &posts,
        r#"document.getElementById("first").focus()"#,
    );
    assert_eq!(focused(&mut session, &posts), "focus:first");
    eval(
        &mut session,
        &posts,
        r#"document.getElementById("outside").focus()"#,
    );
    assert_eq!(focused(&mut session, &posts), "focus:first");

    press(&mut session, NamedKey::Tab, Modifiers::empty());
    assert_eq!(focused(&mut session, &posts), "focus:second");
    press(&mut session, NamedKey::Tab, Modifiers::empty());
    assert_eq!(focused(&mut session, &posts), "focus:first");
    press(&mut session, NamedKey::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&mut session, &posts), "focus:second");
    press(&mut session, NamedKey::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&mut session, &posts), "focus:first");
    assert_clean(&mut session);
}

#[test]
fn d_close_restores_the_page_and_the_opener() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);
    let closed = eval(
        &mut session,
        &posts,
        r#"const dialog = document.getElementById("modal"); dialog.close("done"); report(dialog.matches(":modal") + ":" + dialog.returnValue);"#,
    );
    assert_eq!(closed, ["false:done", "close:modal"]);
    assert_eq!(focused(&mut session, &posts), "focus:opener");
    assert_page_restored(&render(&mut session));
    click(&mut session, 170.0, 130.0);
    assert_eq!(taken(&posts), ["click:outside"]);
    assert_clean(&mut session);
}

#[test]
fn d_escape_cancels_closes_and_restores_the_opener() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);
    press(&mut session, NamedKey::Escape, Modifiers::empty());
    assert_eq!(taken(&posts), ["cancel:modal", "close:modal"]);
    assert_eq!(focused(&mut session, &posts), "focus:opener");
    assert_page_restored(&render(&mut session));
    assert_clean(&mut session);
}

#[test]
fn d_removing_the_dialog_restores_the_page_and_the_opener() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);
    let removed = eval(
        &mut session,
        &posts,
        r#"document.getElementById("clip").remove(); report(String(document.querySelector(":modal")));"#,
    );
    assert_eq!(removed, ["null"]);
    assert_eq!(focused(&mut session, &posts), "focus:opener");
    let buffer = render(&mut session);
    assert_near(&buffer, 100, 75, YELLOW);
    assert_near(&buffer, 100, 140, WHITE);
    click(&mut session, 170.0, 130.0);
    assert_eq!(taken(&posts), ["click:outside"]);
    assert_clean(&mut session);
}

#[test]
fn d_closing_with_a_detached_opener_does_not_throw() {
    let (mut session, posts) = session();
    let unfocused = focused(&mut session, &posts);
    let closed = eval(
        &mut session,
        &posts,
        r#"const button = document.createElement("button"); button.id = "temp"; document.body.append(button); button.focus(); const dialog = document.getElementById("modal"); dialog.showModal(); button.remove(); dialog.close(); report("closed");"#,
    );
    assert_eq!(closed, ["closed", "close:modal"]);
    assert_eq!(focused(&mut session, &posts), unfocused);
    assert_clean(&mut session);
}

#[test]
fn e_nested_modals_stack_and_unwind() {
    let (mut session, posts) = session();
    open_from_opener(&mut session, &posts);
    let opened = eval(
        &mut session,
        &posts,
        r#"document.getElementById("inner").showModal(); report(String(document.getElementById("inner").matches(":modal")));"#,
    );
    assert_eq!(opened, ["true"]);
    assert_eq!(focused(&mut session, &posts), "focus:deep");

    let buffer = render(&mut session);
    assert_near(&buffer, 100, 75, GREEN);
    assert_near(&buffer, 5, 5, [128, 64, 192]);
    assert_near(&buffer, 100, 60, [0, 64, 128]);

    click(&mut session, 100.0, 60.0);
    assert_eq!(taken(&posts), ["click:inner"]);
    eval(
        &mut session,
        &posts,
        r#"document.getElementById("first").focus()"#,
    );
    assert_eq!(focused(&mut session, &posts), "focus:deep");
    press(&mut session, NamedKey::Tab, Modifiers::empty());
    assert_eq!(focused(&mut session, &posts), "focus:deep");

    press(&mut session, NamedKey::Escape, Modifiers::empty());
    assert_eq!(taken(&posts), ["cancel:inner", "close:inner"]);
    assert_eq!(focused(&mut session, &posts), "focus:first");
    let buffer = render(&mut session);
    assert_near(&buffer, 5, 5, RED_OVER_WHITE);
    assert_near(&buffer, 100, 75, GREEN);

    press(&mut session, NamedKey::Escape, Modifiers::empty());
    assert_eq!(taken(&posts), ["cancel:modal", "close:modal"]);
    assert_eq!(focused(&mut session, &posts), "focus:opener");
    assert_page_restored(&render(&mut session));
    assert_clean(&mut session);
}

#[test]
fn f_show_stays_in_flow_without_a_backdrop() {
    let (mut session, posts) = session();
    let shown = eval(
        &mut session,
        &posts,
        r#"const dialog = document.getElementById("plain"); dialog.show(); report(dialog.open + ":" + dialog.matches(":modal") + ":" + String(document.querySelector(":modal")));"#,
    );
    assert_eq!(shown, ["true:false:null"]);
    let buffer = render(&mut session);
    assert_near(&buffer, 30, 15, BLUE);
    assert_near(&buffer, 100, 140, WHITE);
    assert_near(&buffer, 100, 75, YELLOW);

    click(&mut session, 170.0, 130.0);
    assert_eq!(taken(&posts), ["click:outside"]);
    eval(
        &mut session,
        &posts,
        r#"document.getElementById("outside").focus()"#,
    );
    assert_eq!(focused(&mut session, &posts), "focus:outside");
    press(&mut session, NamedKey::Tab, Modifiers::SHIFT);
    assert_ne!(focused(&mut session, &posts), "focus:outside");
    press(&mut session, NamedKey::Escape, Modifiers::empty());
    assert_eq!(taken(&posts), Vec::<String>::new());

    let closed = eval(
        &mut session,
        &posts,
        r#"document.getElementById("plain").close()"#,
    );
    assert_eq!(closed, ["close:plain"]);
    assert_page_restored(&render(&mut session));
    assert_near(&render(&mut session), 30, 15, WHITE);
    assert_clean(&mut session);
}
