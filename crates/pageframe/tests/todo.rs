#[path = "../examples/todo/app.rs"]
mod todo;

use blitz_test_harness::Harness;
use pageframe::Page;
use todo::{MAX_TITLE, Todos};

fn open() -> Harness<Page<Todos>> {
    let page = todo::app()
        .page(420, 560)
        .expect("the example's selectors are valid");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn add(harness: &mut Harness<Page<Todos>>, title: &str) {
    harness.doc.ui().select("#new").set_value(title);
    harness.click("#add");
    harness.pump();
}

fn rows(harness: &mut Harness<Page<Todos>>) -> Vec<String> {
    let nodes = harness.query_all("#list .title");
    let mut ui = harness.doc.ui();
    nodes
        .into_iter()
        .map(|node| ui.node(node).text().unwrap_or_default())
        .collect()
}

#[test]
fn adding_items_fills_the_list_and_clears_the_field() {
    let mut harness = open();
    assert_eq!(harness.attr("#empty", "hidden"), None);
    add(&mut harness, "  Buy milk  ");
    add(&mut harness, "Walk the dog");
    add(&mut harness, "   ");
    assert_eq!(rows(&mut harness), ["Buy milk", "Walk the dog"]);
    assert_eq!(harness.text_content("#left"), "2 items left");
    assert_eq!(harness.doc.ui().select("#new").value().as_deref(), Some(""));
    assert_eq!(harness.attr("#empty", "hidden").as_deref(), Some(""));
}

#[test]
fn toggling_and_removing_update_state_and_counter() {
    let mut harness = open();
    add(&mut harness, "One");
    add(&mut harness, "Two");
    harness.click(".toggle");
    harness.pump();
    assert!(
        harness
            .doc
            .state()
            .items
            .first()
            .is_some_and(|todo| todo.done)
    );
    assert_eq!(harness.text_content("#left"), "1 item left");
    assert_eq!(
        harness.doc.ui().select(".row").attr("data-done").as_deref(),
        Some("true")
    );
    harness.click(".remove");
    harness.pump();
    assert_eq!(rows(&mut harness), ["Two"]);
    assert_eq!(harness.doc.state().items.len(), 1);
}

#[test]
fn oversized_titles_are_capped_and_keep_rows_the_same_size() {
    let mut harness = open();
    add(&mut harness, "Short");
    let short = harness.layout_rect(".row");
    add(&mut harness, &"W".repeat(20_000));
    let titles = rows(&mut harness);
    assert_eq!(
        titles.last().map(|title| title.chars().count()),
        Some(MAX_TITLE)
    );
    let rects: Vec<_> = harness
        .query_all("#list .row")
        .into_iter()
        .map(|node| harness.layout_rect_of(node))
        .collect();
    assert_eq!(rects.len(), 2);
    assert!(
        rects
            .iter()
            .all(|rect| rect.width == short.width && rect.height == short.height)
    );
}

#[test]
fn long_lists_scroll_inside_a_fixed_height() {
    let mut harness = open();
    for index in 0..60 {
        add(&mut harness, &format!("Item {index}"));
    }
    let list = harness.layout_rect("#list");
    let left = harness.layout_rect("#left");
    assert_eq!(list.height, 320.0);
    assert!(left.y < 560.0);
    assert_eq!(harness.doc.state().items.len(), 60);
}
