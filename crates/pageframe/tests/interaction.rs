use blitz_test_harness::Harness;
use pageframe::events::{MouseEventButton, PointerEvent};
use pageframe::{App, Error, EventData, EventKind, Page};

const COUNTER: &str = r#"<html><body>
<button id="inc"><span id="label">Add</span></button>
<span id="count">0</span>
</body></html>"#;

fn open<S: 'static>(app: App<S>) -> Harness<Page<S>> {
    let page = app.page(400, 300).expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn click_button_text<S: 'static>(harness: &mut Harness<Page<S>>) {
    let (x, y) = harness.center_of("#inc");
    harness.click_at(x, y);
}

#[test]
fn click_updates_state_and_dom() {
    let app = App::new(0_u32, COUNTER).on("#inc", EventKind::Click, |count, ui, _| {
        *count += 1;
        ui.select("#count").set_text(&count.to_string());
    });
    let mut harness = open(app);
    harness.click("#inc");
    harness.click("#inc");
    assert_eq!(*harness.doc.state(), 2);
    assert_eq!(harness.text_content("#count"), "2");
}

#[test]
fn click_on_child_bubbles_to_matching_ancestor() {
    let app = App::new(Vec::new(), COUNTER).on("#inc", EventKind::Click, |seen, ui, event| {
        let target = ui.node(event.target()).attr("id");
        let current = ui.node(event.current_target()).attr("id");
        seen.push((target, current));
    });
    let mut harness = open(app);
    click_button_text(&mut harness);
    assert_eq!(
        harness.doc.state().as_slice(),
        [(Some("label".to_owned()), Some("inc".to_owned()))]
    );
}

#[test]
fn stop_propagation_skips_outer_handlers() {
    let app = App::new(Vec::new(), COUNTER)
        .on("#label", EventKind::Click, |seen, _, event| {
            seen.push("inner");
            event.stop_propagation();
        })
        .on("#inc", EventKind::Click, |seen, _, _| seen.push("outer"));
    let mut harness = open(app);
    click_button_text(&mut harness);
    assert_eq!(harness.doc.state().as_slice(), ["inner"]);
}

#[test]
fn typing_fires_input_with_value() {
    let html = r#"<html><body><input id="name"><p id="echo"></p></body></html>"#;
    let app = App::new((), html).on("#name", EventKind::Input, |(), ui, event| {
        let value = event.value().unwrap_or_default().to_owned();
        ui.select("#echo").set_text(&value);
    });
    let mut harness = open(app);
    harness.click("#name");
    harness.type_text("hi there");
    harness.pump();
    assert_eq!(harness.text_content("#echo"), "hi there");
    assert_eq!(
        harness.doc.ui().select("#name").value().as_deref(),
        Some("hi there")
    );
}

#[test]
fn invalid_selector_is_reported() {
    let result = App::new((), COUNTER)
        .on("##nope", EventKind::Click, |(), _, _| {})
        .page(100, 100);
    assert!(matches!(result, Err(Error::Selector { selector }) if selector == "##nope"));
}

#[test]
fn template_rows_are_cloned_into_a_list() {
    let html = r#"<html><body>
<template id="row"><li class="row"><span class="label"></span></li></template>
<ul id="list"></ul>
</body></html>"#;
    let app = App::new(vec!["one", "two", "three"], html).on_start(|items, ui| {
        for item in items.iter() {
            let nodes = ui.instantiate("#row");
            let Some(&row) = nodes.iter().find(|&&n| ui.find_in(n, ".label").is_some()) else {
                continue;
            };
            if let Some(label) = ui.find_in(row, ".label") {
                ui.node(label).set_text(item);
            }
            ui.select("#list").append(&[row]);
        }
    });
    let mut harness = open(app);
    let texts: Vec<String> = harness
        .query_all("#list .row")
        .into_iter()
        .map(|node| harness.doc.ui().node(node).text().unwrap_or_default())
        .collect();
    assert_eq!(texts, ["one", "two", "three"]);
}

#[test]
fn classes_and_attributes_round_trip() {
    let mut harness = open(App::new((), COUNTER));
    let mut ui = harness.doc.ui();
    ui.select("#count")
        .add_class("big")
        .add_class("big")
        .add_class("red");
    assert_eq!(
        ui.select("#count").attr("class").as_deref(),
        Some("big red")
    );
    ui.select("#count")
        .set_class("big", false)
        .set_attr("data-x", "1");
    assert!(!ui.select("#count").has_class("big"));
    assert!(ui.select("#count").has_class("red"));
    assert_eq!(ui.select("#count").attr("data-x").as_deref(), Some("1"));
    ui.select("#count").remove_attr("data-x");
    assert_eq!(ui.select("#count").attr("data-x"), None);
}

#[test]
fn removed_nodes_stop_matching() {
    let mut harness = open(App::new((), COUNTER));
    harness.doc.ui().select("#inc").remove();
    harness.pump();
    assert_eq!(harness.query("#inc"), None);
    assert!(harness.doc.ui().select("#inc").is_empty());
}

#[test]
fn handler_can_remove_the_clicked_element() {
    let app = App::new(0_u32, COUNTER).on("#inc", EventKind::Click, |clicks, ui, event| {
        *clicks += 1;
        ui.node(event.current_target()).remove();
    });
    let mut harness = open(app);
    click_button_text(&mut harness);
    harness.pump();
    assert_eq!(*harness.doc.state(), 1);
    assert_eq!(harness.query("#inc"), None);
    assert_eq!(harness.text_content("#count"), "0");
}

#[test]
fn asset_and_app_stylesheets_apply() {
    let html = r#"<html><head><link rel="stylesheet" href="app.css"></head>
<body style="margin:0"><div id="box"></div></body></html>"#;
    let app = App::new((), html)
        .asset("app.css", b"#box { width: 123px; height: 10px; }")
        .stylesheet("#box { height: 45px; }");
    let mut harness = open(app);
    harness.pump();
    let rect = harness.layout_rect("#box");
    assert_eq!((rect.width, rect.height), (123.0, 45.0));
}

#[test]
fn click_data_uses_pageframe_event_types() {
    let app = App::new(None, COUNTER).on("#inc", EventKind::Click, |seen, _, event| {
        if let EventData::Click(pointer) = event.data() {
            let pointer: &PointerEvent = pointer;
            *seen = Some((pointer.button, pointer.coords.client_x.round()));
        }
    });
    let mut harness = open(app);
    let (x, y) = harness.center_of("#inc");
    harness.click_at(x, y);
    assert_eq!(
        *harness.doc.state(),
        Some((MouseEventButton::Main, x.round()))
    );
}
