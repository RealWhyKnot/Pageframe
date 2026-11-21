use pageframe::{App, Page};
use pageframe_dom::{BaseDocument, NodeId};
use pageframe_test_harness::Harness;

const RUBY: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
<div>ab<ruby>XY<rt>zz</rt></ruby>cd</div><p>after</p>";

const INLINE_BLOCK: &str = "<!doctype html><style>html,body{margin:0;font:14px Arial}</style>\
<div>ab<span style=\"display:inline-block\">PQ</span>cd</div><p>later</p>";

fn node(doc: &BaseDocument, selector: &str) -> NodeId {
    doc.query_selector(selector)
        .expect("valid selector")
        .expect("the element exists")
}

fn root_text(doc: &BaseDocument, root: NodeId) -> &str {
    doc.get_node(root)
        .and_then(|node| node.element_data())
        .and_then(|data| data.inline_layout_data.as_ref())
        .map_or("", |layout| layout.text.as_str())
}

fn inline_root(doc: &BaseDocument, selector: &str) -> NodeId {
    let element = node(doc, selector);
    if !root_text(doc, element).is_empty() {
        return element;
    }
    doc.get_node(element)
        .and_then(|node| node.layout_children.borrow().clone())
        .into_iter()
        .flatten()
        .find(|&child| !root_text(doc, child).is_empty())
        .expect("the element holds an inline root")
}

fn selected(doc: &BaseDocument) -> Vec<String> {
    let mut pieces: Vec<String> = doc
        .get_text_selection_ranges()
        .into_iter()
        .map(|(root, start, end)| {
            root_text(doc, root)
                .get(start..end)
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    pieces.sort();
    pieces
}

fn select(page: &str, from: (&str, usize), to: (&str, usize)) -> Vec<String> {
    let page: Page<()> = App::new((), page).page(320, 200).expect("the page parses");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let mut doc = harness.base_mut();
    let anchor = inline_root(&doc, from.0);
    let focus = inline_root(&doc, to.0);
    doc.set_text_selection(anchor, from.1, focus, to.1);
    let pieces = selected(&doc);
    drop(doc);
    pieces
}

#[test]
fn a_selection_ending_in_an_annotation_stops_at_the_ruby() {
    assert_eq!(select(RUBY, ("div", 1), ("rt", 1)), ["XY", "b", "z"]);
}

#[test]
fn a_selection_starting_in_an_annotation_keeps_the_text_after_the_ruby() {
    assert_eq!(select(RUBY, ("rt", 1), ("p", 2)), ["af", "cd", "z"]);
}

#[test]
fn a_backwards_selection_from_a_later_block_into_an_annotation_matches_forwards() {
    assert_eq!(select(RUBY, ("p", 2), ("rt", 1)), ["af", "cd", "z"]);
}

#[test]
fn a_selection_inside_one_annotation_stays_there() {
    assert_eq!(select(RUBY, ("rt", 0), ("rt", 1)), ["z"]);
}

#[test]
fn an_inline_block_endpoint_splits_its_ancestor_line_at_the_box() {
    assert_eq!(
        select(INLINE_BLOCK, ("div", 1), ("div > span", 1)),
        ["P", "b"]
    );
    assert_eq!(
        select(INLINE_BLOCK, ("div > span", 1), ("p", 2)),
        ["Q", "cd", "la"]
    );
}
