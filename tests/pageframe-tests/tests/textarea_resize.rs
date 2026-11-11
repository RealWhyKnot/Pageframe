use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use cursor_icon::CursorIcon;
use pageframe_dom::{DocumentConfig, EventDriver, NoopEventHandler};
use pageframe_html::{HtmlDocument, HtmlProvider};
use pageframe_paint::paint_scene;
use pageframe_test_harness::{Harness, HarnessOptions, mouse_pointer_event};
use pageframe_traits::events::UiEvent;
use pageframe_traits::shell::{ColorScheme, ShellProvider, Viewport};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn open(body: &str) -> Harness<HtmlDocument> {
    open_at(body, 1.0)
}

fn open_at(body: &str, scale: f32) -> Harness<HtmlDocument> {
    let html = format!(r#"<html><body style="margin:0">{body}</body></html>"#);
    Harness::from_html_with(
        &html,
        HarnessOptions {
            width: 400,
            height: 300,
            scale,
            ..Default::default()
        },
    )
}

fn grip(harness: &Harness<HtmlDocument>, selector: &str) -> (f32, f32) {
    let rect = harness.layout_rect(selector);
    (rect.x + rect.width - 6.0, rect.y + rect.height - 6.0)
}

fn drag_grip(harness: &mut Harness<HtmlDocument>, selector: &str, dx: f32, dy: f32) -> (f32, f32) {
    let from = grip(harness, selector);
    harness.drag(from, (from.0 + dx, from.1 + dy), 3);
    let rect = harness.layout_rect(selector);
    (rect.width, rect.height)
}

const TEXTAREA: &str = r#"<textarea style="position:absolute;left:10px;top:10px"></textarea>"#;

#[test]
fn dragging_the_grip_grows_a_textarea_like_chrome() {
    let mut harness = open(TEXTAREA);
    let before = harness.layout_rect("textarea");
    assert_eq!((before.width, before.height), (168.0, 36.0));
    assert_eq!(
        drag_grip(&mut harness, "textarea", 30.0, 20.0),
        (198.0, 56.0)
    );
}

#[test]
fn content_box_textarea_stops_at_fifteen_pixels() {
    let mut harness = open(TEXTAREA);
    assert_eq!(
        drag_grip(&mut harness, "textarea", -500.0, -500.0),
        (15.0, 15.0)
    );
}

#[test]
fn border_box_textarea_clamps_to_its_padding_and_border() {
    let mut harness = open(
        r#"<textarea style="position:absolute;left:10px;top:10px;box-sizing:border-box;padding:5px;border:3px solid"></textarea>"#,
    );
    assert_eq!(
        drag_grip(&mut harness, "textarea", -500.0, -500.0),
        (16.0, 16.0)
    );
}

#[test]
fn vertical_resize_only_changes_height() {
    let mut harness = open(
        r#"<textarea style="position:absolute;left:10px;top:10px;resize:vertical"></textarea>"#,
    );
    assert_eq!(
        drag_grip(&mut harness, "textarea", 30.0, 20.0),
        (168.0, 56.0)
    );
}

#[test]
fn horizontal_resize_only_changes_width() {
    let mut harness = open(
        r#"<textarea style="position:absolute;left:10px;top:10px;resize:horizontal"></textarea>"#,
    );
    assert_eq!(
        drag_grip(&mut harness, "textarea", 30.0, 20.0),
        (198.0, 36.0)
    );
}

#[test]
fn scrolling_div_with_resize_follows_the_pointer() {
    let mut harness = open(
        r#"<div style="position:absolute;left:10px;top:10px;width:80px;height:50px;overflow:auto;resize:both;border:1px solid"></div>"#,
    );
    assert_eq!(drag_grip(&mut harness, "div", 10.5, 10.5), (92.0, 62.0));
}

#[test]
fn resize_needs_a_scroll_container() {
    let mut harness = open(
        r#"<div style="position:absolute;left:10px;top:10px;width:80px;height:50px;resize:both;border:1px solid"></div>"#,
    );
    let at = grip(&harness, "div");
    harness.move_mouse_to(at.0, at.1);
    assert_ne!(harness.base().get_cursor(), Some(CursorIcon::SeResize));
    assert_eq!(drag_grip(&mut harness, "div", 30.0, 20.0), (82.0, 52.0));
}

#[test]
fn resize_none_has_no_grip() {
    let mut harness =
        open(r#"<textarea style="position:absolute;left:10px;top:10px;resize:none"></textarea>"#);
    let at = grip(&harness, "textarea");
    harness.move_mouse_to(at.0, at.1);
    assert_eq!(harness.base().get_cursor(), Some(CursorIcon::Text));
    assert_eq!(
        drag_grip(&mut harness, "textarea", 30.0, 20.0),
        (168.0, 36.0)
    );
}

#[test]
fn grip_cursor_matches_the_resize_direction() {
    let cases = [
        ("both", "ltr", CursorIcon::SeResize),
        ("both", "rtl", CursorIcon::SwResize),
        ("vertical", "ltr", CursorIcon::NsResize),
        ("horizontal", "ltr", CursorIcon::EwResize),
    ];
    for (resize, direction, cursor) in cases {
        let mut harness = open(&format!(
            r#"<textarea dir="{direction}" style="position:absolute;left:10px;top:10px;resize:{resize}"></textarea>"#
        ));
        let rect = harness.layout_rect("textarea");
        let x = if direction == "rtl" {
            rect.x + 6.0
        } else {
            rect.x + rect.width - 6.0
        };
        harness.move_mouse_to(x, rect.y + rect.height - 6.0);
        assert_eq!(
            harness.base().get_cursor(),
            Some(cursor),
            "{resize} {direction}"
        );
    }
}

#[test]
fn rtl_grip_drags_from_the_left_corner() {
    let mut harness =
        open(r#"<textarea dir="rtl" style="position:absolute;left:100px;top:10px"></textarea>"#);
    let rect = harness.layout_rect("textarea");
    let from = (rect.x + 6.0, rect.y + rect.height - 6.0);
    harness.drag(from, (from.0 - 30.0, from.1 + 20.0), 3);
    let after = harness.layout_rect("textarea");
    assert_eq!((after.width, after.height), (198.0, 56.0));
}

#[test]
fn cursor_stays_while_dragging_off_the_grip() {
    let mut harness = open(TEXTAREA);
    let from = grip(&harness, "textarea");
    harness.mouse_down_at(from.0, from.1);
    harness.move_mouse_to(300.0, 250.0);
    assert_eq!(harness.base().get_cursor(), Some(CursorIcon::SeResize));
    harness.mouse_up_at(300.0, 250.0);
    harness.move_mouse_to(390.0, 290.0);
    assert_ne!(harness.base().get_cursor(), Some(CursorIcon::SeResize));
}

#[test]
fn pressing_the_grip_keeps_focus_and_still_clicks() {
    let mut harness = open(TEXTAREA);
    let focused = harness.focused();
    let from = grip(&harness, "textarea");
    let event = mouse_pointer_event(from.0, from.1);
    let events = harness.dispatch_recorded([
        UiEvent::PointerDown(event.clone()),
        UiEvent::PointerUp(event),
    ]);
    harness.pump();
    assert!(events.iter().any(|name| name == "click"), "{events:?}");
    assert_eq!(harness.focused(), focused);
}

#[test]
fn shrinking_past_the_padding_stops_at_the_padding() {
    let mut harness = open(
        r#"<textarea style="position:absolute;left:200px;top:150px;padding:20px"></textarea>"#,
    );
    assert_eq!(
        drag_grip(&mut harness, "textarea", -300.0, -300.0),
        (42.0, 42.0)
    );
}

#[derive(Default)]
struct RecordingShell {
    redraws: AtomicUsize,
}

impl ShellProvider for RecordingShell {
    fn request_redraw(&self) {
        self.redraws.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn every_resize_step_requests_a_redraw() {
    let shell = Arc::new(RecordingShell::default());
    let mut doc = HtmlDocument::from_html(
        &format!(r#"<html><body style="margin:0">{TEXTAREA}</body></html>"#),
        DocumentConfig {
            viewport: Some(Viewport::new(400, 300, 1.0, ColorScheme::Light)),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
            shell_provider: Some(shell.clone() as _),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    let mut driver = EventDriver::new(&mut doc, NoopEventHandler);
    driver.handle_ui_event(UiEvent::PointerDown(mouse_pointer_event(172.0, 40.0)));
    for step in 1..=3u8 {
        let offset = f32::from(step) * 10.0;
        shell.redraws.swap(0, Ordering::Relaxed);
        driver.handle_ui_event(UiEvent::PointerMove(mouse_pointer_event(
            172.0 + offset,
            40.0 + offset,
        )));
        assert!(shell.redraws.swap(0, Ordering::Relaxed) > 0, "step {step}");
    }
}

#[test]
fn grip_scales_with_the_device() {
    let mut harness = open_at(TEXTAREA, 2.0);
    assert_eq!(
        drag_grip(&mut harness, "textarea", 30.0, 20.0),
        (201.0, 58.0)
    );
}

const GRIP_1X: [&str; 12] = [
    "............",
    "............",
    "............",
    ".........#..",
    "........#...",
    ".......#....",
    "......#.....",
    ".....#...#..",
    "....#...#...",
    "...#...#....",
    "............",
    "............",
];

const GRIP_2X: [&str; 24] = [
    "........................",
    "........................",
    "........................",
    "........................",
    "........................",
    "...................#....",
    "..................###...",
    ".................##x....",
    "................##x.....",
    "...............##x......",
    "..............##x.......",
    ".............##x........",
    "............##x....#....",
    "...........##x....###...",
    "..........##x....##x....",
    ".........##x....##x.....",
    "........##x....##x......",
    ".......##x....##x.......",
    "......##x....##x........",
    "......##.....##.........",
    "........................",
    "........................",
    "........................",
    ".......................?",
];

fn grip_rows(scale: u32) -> Vec<String> {
    let mut harness = open_at(
        TEXTAREA,
        f32::from(u16::try_from(scale).expect("small scale")),
    );
    let (width, height) = (400 * scale, 300 * scale);
    let rect = harness.layout_rect("textarea");
    let corner = |edge: f32| {
        (0..u16::MAX)
            .find(|&pixel| f32::from(pixel) >= edge - 1.0)
            .map(usize::from)
            .expect("on screen")
    };
    let mut doc = harness.base_mut();
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, f64::from(scale), width, height, 0, 0),
        width,
        height,
    );
    let scale = usize::try_from(scale).expect("fits");
    let right = corner(rect.x + rect.width) * scale;
    let bottom = corner(rect.y + rect.height) * scale;
    let size = 12 * scale;
    let stride = usize::try_from(width).expect("fits");
    (bottom - size..bottom)
        .map(|y| {
            (right - size..right)
                .map(|x| match buffer[(y * stride + x) * 4] {
                    255 => '.',
                    102 => '#',
                    194 => 'x',
                    _ => '?',
                })
                .collect()
        })
        .collect()
}

#[test]
fn grip_paints_like_chrome_at_1x() {
    assert_eq!(grip_rows(1), GRIP_1X);
}

#[test]
fn grip_paints_like_chrome_at_2x() {
    assert_eq!(grip_rows(2), GRIP_2X);
}
