mod support;

use pageframe::{App, Event, EventKind, Key, NamedKey, Page, Ui};
use pageframe_test_harness::Harness;
use support::{HEIGHT, open, pixel, render};

type Opened = Harness<Page<Vec<String>>>;

const ROW: f32 = 17.0;

const PAGE: &str = r#"<html><head><style>
body{margin:0;font:16px Arial}
#fruit{width:100px}
#low{position:absolute;left:0;top:60px}
</style></head><body>
<select id="fruit"><option value="a">Apple</option><option selected>Banana
  split</option><option disabled>Cherry</option></select><br>
<select id="plain"><option disabled>Pick</option><option>One</option><option>Two</option><option>Three</option></select>
<select id="low"><option>Up</option><option>Top</option></select>
</body></html>"#;

fn page() -> Opened {
    open(
        App::new(Vec::new(), PAGE)
            .on("#fruit", EventKind::Input, record)
            .on("#plain", EventKind::Input, record)
            .on("#low", EventKind::Input, record),
    )
}

fn record(seen: &mut Vec<String>, _: &mut Ui<'_>, event: &mut Event<'_>) {
    seen.push(event.value().unwrap_or_default().to_owned());
}

fn value(harness: &mut Opened, selector: &str) -> Option<String> {
    harness.doc.ui().select(selector).value()
}

fn is_open(harness: &Opened) -> bool {
    harness.base().select_popup_is_open()
}

fn click_row(harness: &mut Opened, selector: &str, row: u8) {
    let rect = harness.layout_rect(selector);
    let y = rect.y + rect.height + 1.0 + ROW * f32::from(row) + ROW / 2.0;
    harness.click_at(rect.x + 10.0, y);
}

fn named(key: NamedKey) -> Key {
    Key::Named(key)
}

#[test]
fn clicking_a_row_picks_it_and_fires_input() {
    let mut harness = page();
    harness.click("#fruit");
    assert!(is_open(&harness));
    assert_eq!(harness.doc.state().len(), 0);
    click_row(&mut harness, "#fruit", 0);
    assert!(!is_open(&harness));
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("a"));
    assert_eq!(harness.doc.state().as_slice(), ["a"]);
}

#[test]
fn clicking_the_current_row_closes_without_an_event() {
    let mut harness = page();
    harness.click("#fruit");
    click_row(&mut harness, "#fruit", 1);
    assert!(!is_open(&harness));
    assert_eq!(harness.doc.state().len(), 0);
}

#[test]
fn disabled_rows_cannot_be_picked() {
    let mut harness = page();
    harness.click("#fruit");
    click_row(&mut harness, "#fruit", 2);
    assert!(is_open(&harness));
    assert_eq!(
        value(&mut harness, "#fruit").as_deref(),
        Some("Banana split")
    );
    assert_eq!(harness.doc.state().len(), 0);
}

#[test]
fn clicking_elsewhere_or_the_select_again_closes_the_popup() {
    let mut harness = page();
    harness.click("#fruit");
    harness.click_at(115.0, 50.0);
    assert!(!is_open(&harness));
    harness.click("#fruit");
    harness.click("#fruit");
    assert!(!is_open(&harness));
    assert_eq!(harness.doc.state().len(), 0);
}

#[test]
fn the_popup_takes_clicks_meant_for_what_is_under_it() {
    let mut harness = page();
    harness.click("#fruit");
    let fruit = harness.node("#fruit");
    let under = harness.layout_rect("#plain");
    assert_eq!(harness.hit_node(under.x + 10.0, under.y + 5.0), fruit);
    harness.click_at(under.x + 10.0, under.y + 5.0);
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("a"));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("One"));
}

#[test]
fn arrow_keys_change_a_closed_select() {
    let mut harness = page();
    harness.click("#plain");
    harness.press(named(NamedKey::Escape));
    assert!(!is_open(&harness));
    harness.press(named(NamedKey::ArrowDown));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Two"));
    harness.press(named(NamedKey::ArrowUp));
    harness.press(named(NamedKey::ArrowUp));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("One"));
    harness.press(named(NamedKey::End));
    harness.press(named(NamedKey::ArrowDown));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Three"));
    harness.press(Key::Character("t".to_owned()));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Two"));
    harness.press(Key::Character("t".to_owned()));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Three"));
    assert_eq!(
        harness.doc.state().as_slice(),
        ["Two", "One", "Three", "Two", "Three"]
    );
}

#[test]
fn keys_in_an_open_popup_only_commit_on_enter() {
    let mut harness = page();
    harness.click("#plain");
    harness.press(named(NamedKey::ArrowDown));
    harness.press(named(NamedKey::ArrowDown));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("One"));
    harness.press(named(NamedKey::Escape));
    assert!(!is_open(&harness));
    assert_eq!(harness.doc.state().len(), 0);
    harness.press(Key::Character(" ".to_owned()));
    assert!(is_open(&harness));
    harness.press(named(NamedKey::ArrowDown));
    harness.press(named(NamedKey::Enter));
    assert!(!is_open(&harness));
    assert_eq!(value(&mut harness, "#plain").as_deref(), Some("Two"));
    assert_eq!(harness.doc.state().as_slice(), ["Two"]);
}

#[test]
fn tabbing_away_closes_the_popup() {
    let mut harness = page();
    harness.click("#fruit");
    harness.press(named(NamedKey::Tab));
    assert!(!is_open(&harness));
    assert_ne!(harness.focused(), Some(harness.node("#fruit")));
}

#[test]
fn hovering_moves_the_highlight() {
    let mut harness = page();
    harness.click("#fruit");
    let rect = harness.layout_rect("#fruit");
    harness.move_mouse_to(rect.x + 10.0, rect.y + rect.height + 1.0 + ROW * 0.5);
    harness.press(named(NamedKey::Enter));
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("a"));
}

#[test]
fn open_popup_is_painted_over_the_page() {
    let mut harness = page();
    let closed = render(&mut harness);
    harness.click("#fruit");
    let open = render(&mut harness);
    let rect = harness.layout_rect("#fruit");
    let layout = harness
        .base()
        .select_popup_layout()
        .expect("the popup is open");
    assert!((layout.x - f64::from(rect.x)).abs() < 0.01);
    assert!((layout.y - f64::from(rect.y + rect.height)).abs() < 0.01);
    assert!((layout.width - f64::from(rect.width)).abs() < 0.01);
    assert_eq!(layout.rows.len(), 3);
    let blue = |buffer: &[u8], x: usize, y: usize| {
        let [red, _, blue, _] = pixel(buffer, x, y);
        blue > 180 && red < 80
    };
    assert!(blue(&open, 95, 45));
    assert!(!blue(&closed, 95, 45));
    assert_eq!(pixel(&open, 95, 28), [255, 255, 255, 255]);
    assert_eq!(pixel(&open, 0, 30), [118, 118, 118, 255]);
}

#[test]
fn popup_opens_upwards_when_there_is_no_room_below() {
    let mut harness = page();
    harness.click("#low");
    let rect = harness.layout_rect("#low");
    let layout = harness
        .base()
        .select_popup_layout()
        .expect("the popup is open");
    assert!((layout.y + layout.height() - f64::from(rect.y)).abs() < 0.01);
    assert!(layout.y >= 0.0);
    harness.click_at(rect.x + 10.0, rect.y - 1.0 - ROW * 1.5);
    assert_eq!(value(&mut harness, "#low").as_deref(), Some("Up"));
    harness.click("#low");
    harness.click_at(rect.x + 10.0, rect.y - 1.0 - ROW * 0.5);
    assert_eq!(value(&mut harness, "#low").as_deref(), Some("Top"));
}

#[test]
fn oversized_lists_stay_inside_the_window_and_scroll() {
    let mut harness = page();
    let rows: String = (0..2_000)
        .map(|index| format!("<option>Row {index}</option>"))
        .collect();
    let html = format!("<option>{}</option>{rows}", "W".repeat(20_000));
    harness.doc.ui().select("#fruit").set_html(&html);
    harness.pump();
    harness.click("#fruit");
    let rect = harness.layout_rect("#fruit");
    let shown = |harness: &Opened| {
        let layout = harness
            .base()
            .select_popup_layout()
            .expect("the popup is open");
        assert!(layout.y + layout.height() <= f64::from(HEIGHT));
        assert!((layout.width - f64::from(rect.width)).abs() < 0.01);
        layout
            .rows
            .iter()
            .map(|row| row.label.clone())
            .collect::<Vec<_>>()
    };
    let first = shown(&harness);
    assert_eq!(first.len(), 3);
    assert_eq!(first[1], "Row 0");
    harness.wheel_at(rect.x + 10.0, rect.y + rect.height + 10.0, 0.0, -40.0);
    assert_eq!(shown(&harness)[0], "Row 2");
    harness.press(named(NamedKey::End));
    assert_eq!(shown(&harness)[2], "Row 1999");
    harness.press(named(NamedKey::Enter));
    assert_eq!(value(&mut harness, "#fruit").as_deref(), Some("Row 1999"));
    assert_eq!(harness.layout_rect("#fruit").width, rect.width);
}
