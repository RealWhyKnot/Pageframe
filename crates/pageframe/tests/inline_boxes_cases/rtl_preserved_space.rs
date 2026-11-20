use super::*;

fn row(white_space: &str, preceding_text: &str) -> String {
    format!(
        "<html><head><style>html,body{{margin:0;background:white;color:transparent;\
         font:14px/24px Arial}}div{{margin-left:10px;width:90px;white-space:{white_space}}}\
         span{{background:#f80;padding-right:6px}}</style></head><body>\
         <div id=line dir=rtl>{preceding_text}<span id=target>cd </span>efghijklmn</div>\
         </body></html>"
    )
}

#[test]
fn reordered_trailing_space_paints_separately_from_its_span_text() {
    for (white_space, orange) in [
        ("normal", (79..100).collect::<Vec<_>>()),
        ("pre-wrap", (56..60).chain(79..100).collect()),
    ] {
        let mut harness = open(App::new((), &row(white_space, "ab ")));
        let buffer = render(&mut harness);
        let columns: Vec<_> = (0..120)
            .filter(|&column| pixel(&buffer, column, 12) == [255, 136, 0, 255])
            .collect();
        assert_eq!(columns, orange, "{white_space}");
    }
}

#[test]
fn reordered_trailing_space_has_its_own_client_rect() {
    const BODY_RECT: [f64; 4] = [79.203_125, 4.0, 20.796_875, 16.0];
    for (white_space, expected) in [
        ("normal", vec![BODY_RECT]),
        (
            "pre-wrap",
            vec![[55.843_75, 4.0, 3.890_625, 16.0], BODY_RECT],
        ),
    ] {
        let mut harness = open(App::new((), &row(white_space, "ab ")));
        let doc = harness.base_mut();
        let target = doc
            .query_selector("#target")
            .expect("valid selector")
            .expect("span exists");
        let rects: Vec<_> = doc
            .node_client_rects(target)
            .iter()
            .map(|rect| [rect.x, rect.y, rect.width, rect.height])
            .collect();
        drop(doc);
        assert_eq!(rects.len(), expected.len(), "{white_space}: {rects:?}");
        for (rect, expected) in rects.into_iter().zip(expected) {
            for (actual, expected) in rect.into_iter().zip(expected) {
                assert!(
                    (actual - expected).abs() <= 1.0 / 64.0,
                    "{white_space}: {rect:?}"
                );
            }
        }
    }
}

#[test]
fn gap_between_reordered_trailing_space_and_span_text_hits_the_block() {
    for (white_space, preceding_text) in [
        ("normal", "ab "),
        ("pre-wrap", "ab "),
        (
            "pre-wrap",
            "<i style='font-style:normal;pointer-events:none'>ab </i>",
        ),
    ] {
        let mut harness = open(App::new((), &row(white_space, preceding_text)));
        let doc = harness.base_mut();
        let line = doc.query_selector("#line").expect("valid selector");
        let target = doc.query_selector("#target").expect("valid selector");
        assert!(line.is_some() && target.is_some(), "fixture elements exist");
        let space = if white_space == "pre-wrap" {
            target
        } else {
            line
        };
        for (column, expected) in [(57.0, space), (69.0, line), (88.0, target), (98.0, target)] {
            assert_eq!(
                doc.element_from_point(column, 12.0),
                expected,
                "{white_space} at x={column}, preceding text {preceding_text:?}"
            );
        }
    }
}
