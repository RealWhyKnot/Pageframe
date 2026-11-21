use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><style>
body{margin:0}
p{margin:0;width:600px;height:40px;font:20px/40px Arial;white-space:nowrap}
.e{unicode-bidi:embed}
.i{unicode-bidi:isolate}
</style></head><body>
<p id="l1">abc <span class="e" style="direction:rtl">&#x5d0;&#x5d1;&#x5d2; <span class="e" style="direction:ltr">def</span> &#x5d3;&#x5d4;&#x5d5;</span> xyz</p>
<p id="l2">abc <span class="i" style="direction:rtl">&#x5d0;&#x5d1;&#x5d2; <span class="i" style="direction:ltr">def</span> &#x5d3;&#x5d4;&#x5d5;</span> xyz</p>
<p id="l3" dir="rtl">&#x5d0;&#x5d1;&#x5d2; <span class="e" style="direction:ltr">abc <span class="e" style="direction:rtl">&#x5d3;&#x5d4;&#x5d5;</span> def</span> &#x5d6;&#x5d7;&#x5d8;</p>
<p id="l4" dir="rtl">&#x5d0;&#x5d1;&#x5d2; <span class="i" style="direction:ltr">abc <span class="i" style="direction:rtl">&#x5d3;&#x5d4;&#x5d5;</span> def</span> &#x5d6;&#x5d7;&#x5d8;</p>
</body></html>"#;

fn is_bidi_control(c: char) -> bool {
    matches!(u32::from(c), 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069)
}

fn caret_neighbours(
    harness: &Harness<Page<()>>,
    line: usize,
    x: f32,
) -> (Option<char>, Option<char>) {
    let doc = harness.base();
    let root = doc
        .query_selector_all(&format!("#l{line}"))
        .expect("valid selector")[0];
    let text = doc
        .get_node(root)
        .and_then(|node| node.element_data())
        .and_then(|element| element.inline_layout_data.as_ref())
        .map(|layout| layout.text.to_string())
        .expect("paragraph has an inline layout");
    let y = f32::from(u16::try_from(line).expect("small line")) * 40.0 - 20.0;
    let (id, offset) = doc
        .find_text_position(x, y)
        .expect("point hits the paragraph");
    drop(doc);
    assert_eq!(id, root);
    let before = text[..offset].chars().rev().find(|c| !is_bidi_control(*c));
    let after = text[offset..].chars().find(|c| !is_bidi_control(*c));
    (before, after)
}

#[test]
fn carets_at_embedding_and_isolate_edges_land_in_the_base_run_like_chrome() {
    let page = App::new((), PAGE).page(640, 200).expect("page loads");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let cases = [
        (1, 39.0, Some(' '), Some('\u{5d0}')),
        (1, 100.0, Some('f'), Some(' ')),
        (2, 39.0, Some(' '), Some('\u{5d0}')),
        (2, 69.0, Some(' '), Some('d')),
        (2, 100.0, Some('f'), Some(' ')),
        (3, 504.0, Some('\u{5d5}'), Some(' ')),
        (3, 563.0, Some(' '), Some('a')),
        (4, 504.0, Some('\u{5d5}'), Some(' ')),
        (4, 563.0, Some(' '), Some('a')),
    ];
    for (line, x, before, after) in cases {
        assert_eq!(
            caret_neighbours(&harness, line, x),
            (before, after),
            "line {line} at x {x}"
        );
    }
}
