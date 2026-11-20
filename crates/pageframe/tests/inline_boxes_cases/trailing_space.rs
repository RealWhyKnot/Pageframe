use super::*;

fn padded_span_line(direction: &str, align: &str, style: &str, bounds: [u16; 2], width: f64) {
    let html = format!(
        "<style>body{{margin:0;font:14px/24px Arial;color:transparent}}\
         div{{margin-left:10px;width:{}px;text-align:{align}}}\
         span{{background:#0f0}}</style>\
         <div dir={direction}>ab <span id=target style='{style}'>cd </span>efghijklmn</div>",
        bounds[1] - 10
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    let doc = harness.base_mut();
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .collect();
    assert_eq!(rects.len(), 1);
    let rect = &rects[0];
    let columns: Vec<_> = (0..120)
        .filter(|&column| pixel(&buffer, column, 12) == LIME)
        .collect();
    let [left, right] = bounds;
    assert_eq!(
        columns,
        (usize::from(left)..usize::from(right)).collect::<Vec<_>>(),
        "{style}"
    );
    assert_eq!(rect.width, width, "{style}");
    assert_eq!(rect.x + rect.width, f64::from(right), "{style}");
    assert_eq!(
        doc.element_from_point(f32::from(right) - 0.5, 12.0),
        Some(target)
    );
    assert_ne!(
        doc.element_from_point(f32::from(left) - 1.5, 12.0),
        Some(target)
    );
}

#[test]
fn an_aligned_padded_span_removes_its_trailing_collapsible_space() {
    padded_span_line("ltr", "right", "", [85, 100], 14.796_875);
    padded_span_line("ltr", "right", "padding-right:6px", [79, 100], 20.796_875);
    padded_span_line("ltr", "right", "padding-right:6px", [30, 51], 20.796_875);
}

#[test]
fn a_right_to_left_padded_span_removes_its_trailing_collapsible_space() {
    padded_span_line("rtl", "start", "padding:0 5px", [75, 100], 24.796_875);
}

#[test]
fn a_padded_span_keeps_the_space_before_an_author_zero_width_space() {
    let html = "<style>body{margin:0;font:14px/24px Arial}div{margin-left:10px;width:90px;\
        text-align:right}#target{padding-right:6px;background:#0f0}</style>\
        <div>ab <span id=target>cd <span>&#x200b;</span></span><br>efghijklmn</div>";
    let mut harness = open(App::new((), html));
    let doc = harness.base_mut();
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .map(|rect| [rect.x, rect.width])
        .collect();
    drop(doc);
    assert_eq!(rects, [[75.312_5, 24.687_5]]);
}

#[test]
fn a_padded_span_removes_trailing_collapsible_space_from_intrinsic_widths() {
    let html = "<style>body{margin:0;font:14px/24px Arial}\
        span{padding-right:6px}</style>\
        <div id=min style='width:min-content'><span>cd </span>abc</div>\
        <div id=max style='width:max-content'><span>cd </span></div>\
        <div id=tight style='width:max-content'><span>cd</span></div>\
        <div id=word style='width:max-content'>abc</div>";
    let mut harness = open(App::new((), html));
    let doc = harness.base_mut();
    let width = |id| {
        let node = doc
            .query_selector(id)
            .expect("valid selector")
            .expect("block exists");
        doc.get_node(node)
            .expect("block exists")
            .final_layout()
            .size
            .width
    };
    assert_eq!(width("#min"), width("#word"));
    assert_eq!(width("#max"), width("#tight"));
}

fn edge_trim_geometry(
    align: &str,
    style: &str,
    before: &str,
    after: &str,
) -> ([f64; 2], Vec<[u8; 4]>) {
    let html = format!(
        "<style>body{{margin:0;font:14px/24px Arial;color:transparent;background:white}}\
         div{{margin-left:10px;width:90px;text-align:{align}}}</style>\
         <div>{before}<span id=target style='{style}'>cd </span>{after}</div>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    let doc = harness.base_mut();
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .collect();
    drop(doc);
    assert_eq!(rects.len(), 1);
    let rect = &rects[0];
    let row = (0..120).map(|column| pixel(&buffer, column, 12)).collect();
    ([rect.x, rect.x + rect.width], row)
}

#[test]
fn edge_trim_justifies_using_the_space_between_words() {
    let (rect, row) = edge_trim_geometry(
        "justify",
        "padding-right:6px;background:#0f0",
        "ab ",
        "efghijklmn opq",
    );
    assert_eq!(rect, [79.203_125, 100.0]);
    assert_eq!(row[78], WHITE);
    assert_eq!(row[79..100], [LIME; 21]);
    assert_eq!(row[100], WHITE);
    let (no_gap, _) = edge_trim_geometry(
        "justify",
        "padding-right:6px;background:#0f0",
        "ab",
        " efghijklmn opq",
    );
    assert_eq!(no_gap, [25.578_125, 46.375]);
}

#[test]
fn edge_trim_removes_space_before_a_closing_margin() {
    let (rect, _) = edge_trim_geometry("right", "margin-right:6px", "ab ", "efghijklmn");
    assert_eq!(rect, [79.203_125, 94.0]);
    let (painted_rect, row) = edge_trim_geometry(
        "right",
        "margin-right:6px;background:#0f0",
        "ab ",
        "efghijklmn",
    );
    assert_eq!(painted_rect, rect);
    assert_eq!(row[79..94], [LIME; 15]);
    assert_eq!(row[94..100], [WHITE; 6]);
}

#[test]
fn edge_trim_removes_space_before_a_closing_border() {
    let (rect, row) = edge_trim_geometry(
        "right",
        "border-right:6px solid #00f;background:#0f0",
        "ab ",
        "efghijklmn",
    );
    assert_eq!(rect, [79.203_125, 100.0]);
    assert_eq!(row[79..94], [LIME; 15]);
    assert_eq!(row[94..100], [BLUE; 6]);
}

fn pre_wrap_fragment(
    block_style: &str,
    span_style: &str,
    spaces: &str,
    paint: [u16; 2],
    bounds: [f64; 2],
) {
    let html = format!(
        "<style>body{{margin:0;font:14px/24px Arial;color:transparent}}\
         div{{margin-left:10px;width:90px;white-space:pre-wrap}}span{{background:#0f0}}</style>\
         <div style='{block_style}'>ab <span id=target style='{span_style}'>cd{spaces}</span>\
         efghijklmn</div>"
    );
    let mut harness = open(App::new((), &html));
    let buffer = render(&mut harness);
    let painted: Vec<_> = (0..120)
        .filter(|&column| pixel(&buffer, column, 12) == LIME)
        .collect();
    let [left, right] = paint;
    assert_eq!(painted.first(), Some(&usize::from(left)), "{block_style}");
    assert_eq!(
        painted.last(),
        Some(&(usize::from(right) - 1)),
        "{block_style}"
    );
    let doc = harness.base_mut();
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .collect();
    assert_eq!(rects.len(), 1);
    let rect = &rects[0];
    assert!((rect.x - bounds[0]).abs() <= 1.0 / 32.0, "{rect:?}");
    assert!(
        (rect.x + rect.width - bounds[1]).abs() <= 1.0 / 32.0,
        "{rect:?}"
    );
    assert_eq!(
        doc.element_from_point(f32::from(right) - 0.5, 12.0),
        Some(target)
    );
    assert_ne!(
        doc.element_from_point(f32::from(right) + 0.5, 12.0),
        Some(target)
    );
}

#[test]
fn padded_pre_wrap_space_hangs_at_end_alignment() {
    pre_wrap_fragment(
        "text-align:left",
        "padding-right:6px",
        " ",
        [29, 54],
        [29.468_75, 54.156_25],
    );
    pre_wrap_fragment(
        "white-space:break-spaces;text-align:right",
        "padding-right:6px",
        " ",
        [75, 100],
        [75.312_5, 100.0],
    );
    pre_wrap_fragment(
        "text-align:end",
        "",
        " ",
        [85, 104],
        [85.218_75, 103.906_25],
    );
    pre_wrap_fragment(
        "direction:rtl",
        "padding-right:6px",
        " ",
        [56, 100],
        [55.843_75, 100.0],
    );
    pre_wrap_fragment(
        "text-align:end",
        "padding-right:6px",
        " ",
        [79, 104],
        [79.187_5, 103.875],
    );
    pre_wrap_fragment(
        "width:41px;text-align:right",
        "padding-right:6px",
        " ",
        [30, 55],
        [30.203_125, 54.890_625],
    );
}

#[test]
fn padded_pre_wrap_space_hangs_at_center_alignment() {
    pre_wrap_fragment(
        "text-align:center",
        "padding-right:6px",
        " ",
        [54, 79],
        [54.328_125, 79.015_625],
    );
}

#[test]
fn padded_pre_wrap_preserves_two_hanging_spaces() {
    pre_wrap_fragment(
        "text-align:right",
        "padding-right:6px",
        "  ",
        [79, 108],
        [79.187_5, 107.765_625],
    );
    let html = "<style>body{margin:0;font:14px/24px Arial;white-space:pre-wrap}\
        span{padding-right:6px}</style>\
        <div id=min style='width:min-content'><span>cd  </span>a</div>\
        <div id=max style='width:max-content'><span>cd  </span></div>\
        <div id=tight style='width:max-content'><span>cd</span></div>";
    let mut harness = open(App::new((), html));
    let doc = harness.base_mut();
    let width = |id| {
        let node = doc
            .query_selector(id)
            .expect("valid selector")
            .expect("block exists");
        doc.get_node(node)
            .expect("block exists")
            .final_layout()
            .size
            .width
    };
    assert_eq!(width("#min"), width("#tight"));
    assert_eq!(width("#max"), width("#tight") + 8.0);
}

#[test]
fn rtl_pre_wrap_min_content_matches_the_line_breaker() {
    let html = "<style>body{margin:0;font:14px/24px Arial}\
        div{width:min-content;white-space:pre-wrap}span{padding-left:6px}</style>\
        <div id=block dir=rtl><span id=target>cd </span>&#x5d0;</div>";
    let mut harness = open(App::new((), html));
    let doc = harness.base_mut();
    let block = doc
        .query_selector("#block")
        .expect("valid selector")
        .expect("block exists");
    let node = doc.get_node(block).expect("block exists");
    let width = node.unrounded_layout().size.width;
    let layout = &node
        .element_data()
        .expect("block element")
        .inline_layout_data
        .as_ref()
        .expect("inline layout exists")
        .layout;
    assert_eq!(width, 20.796_875);
    assert_eq!(width, layout.width());
    assert_eq!(layout.len(), 2);
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .map(|rect| rect.width)
        .collect();
    drop(doc);
    assert_eq!(rects, [24.687_5]);
}

fn author_zwsp_fragment(
    align: &str,
    padding: u8,
    span_end: &str,
    after: &str,
    bounds: [f64; 2],
    paint: [usize; 2],
) {
    let html = format!(
        "<style>body{{margin:0;font:14px/24px Arial;color:transparent}}\
         div{{margin-left:10px;width:90px;text-align:{align}}}\
         span{{background:#0f0;padding-right:{padding}px}}</style>\
         <div id=block>ab <span id=target>cd {span_end}</span>{after}efghijklmn</div>"
    );
    check_zwsp_fragment(&html, bounds, 2, Some("efghijklmn"), Some(paint), None);
}

fn check_zwsp_fragment(
    html: &str,
    bounds: [f64; 2],
    lines: usize,
    next_text: Option<&str>,
    paint: Option<[usize; 2]>,
    inline_box: Option<[f64; 3]>,
) {
    let mut harness = open(App::new((), html));
    let buffer = paint.map(|_| render(&mut harness));
    let doc = harness.base_mut();
    let target = doc
        .query_selector("#target")
        .expect("valid selector")
        .expect("span exists");
    let rects: Vec<_> = doc
        .inline_fragment_rects(target)
        .expect("span is laid out")
        .map(|rect| [rect.x, rect.x + rect.width, rect.y])
        .collect();
    let root = doc
        .query_selector("div")
        .expect("valid selector")
        .expect("block exists");
    let inline = doc
        .get_node(root)
        .and_then(|node| node.element_data())
        .and_then(|element| element.inline_layout_data.as_deref())
        .expect("inline layout exists");
    assert_eq!(inline.layout.len(), lines, "line count for {html}");
    if let Some(expected) = next_text {
        let next_line = inline.layout.get(1).expect("the next word wraps");
        let actual = &inline.text[next_line.text_range()];
        assert_eq!(actual, expected, "next line for {html}");
    }
    if let Some(expected) = inline_box {
        let next = doc
            .query_selector(".box")
            .expect("valid selector")
            .expect("inline block exists");
        let rect = doc
            .get_client_bounding_rect(next)
            .expect("inline block is laid out");
        let actual = [rect.x, rect.y, rect.width];
        assert_eq!(actual, expected, "inline block for {html}");
    }
    drop(doc);
    let [left, right] = bounds;
    assert_eq!(rects, [[left, right, 4.0]], "span fragment for {html}");
    if let Some(([left, right], buffer)) = paint.zip(buffer) {
        let columns: Vec<_> = (0..120)
            .filter(|&column| pixel(&buffer, column, 12) == LIME)
            .collect();
        let expected: Vec<_> = (left..right).collect();
        assert_eq!(columns, expected, "painted columns for {html}");
    }
}

#[test]
fn author_zwsp_wrap_keeps_the_closing_padding_on_the_first_line() {
    author_zwsp_fragment("right", 6, "&#x200b;", "", [75.312_5, 100.0], [75, 100]);
}

#[test]
fn author_zwsp_wrap_preserves_the_space_in_an_unpadded_span() {
    author_zwsp_fragment("right", 0, "&#x200b;", "", [81.312_5, 100.0], [81, 100]);
}

#[test]
fn author_zwsp_wrap_can_follow_the_padded_span() {
    author_zwsp_fragment("right", 6, "", "&#x200b;", [75.312_5, 100.0], [75, 100]);
}

#[test]
fn author_zwsp_wrap_preserves_the_left_aligned_fragment() {
    author_zwsp_fragment("left", 6, "&#x200b;", "", [29.468_75, 54.156_25], [29, 54]);
}

#[test]
fn a_space_before_an_author_zwsp_stays_on_the_line() {
    for (content, visible, hanging) in [
        ("aaaa bbbb &#x200b;ccccccccc", 70.078_125, 0.0),
        ("aaaa bbbb&#x200b; ccccccccc", 66.187_5, 3.890_625),
    ] {
        let html = format!(
            "<!doctype html><style>body{{margin:0;font:14px/24px Arial}}\
             div{{margin-left:10px;width:90px;text-align:right}}</style><div>{content}</div>"
        );
        let mut harness = open(App::new((), &html));
        let doc = harness.base_mut();
        let id = doc
            .query_selector("div")
            .expect("valid selector")
            .expect("block exists");
        let inline = doc
            .get_node(id)
            .and_then(|node| node.element_data())
            .and_then(|element| element.inline_layout_data.as_deref())
            .expect("inline layout exists");
        assert_eq!(inline.layout.len(), 2, "{content}");
        let first_line = inline.layout.get(0).expect("first line");
        let first = first_line.metrics();
        let second = inline.layout.get(1).expect("second line");
        assert_eq!(&inline.text[second.text_range()], "ccccccccc", "{content}");
        assert!(
            (first.hanging_advance - hanging).abs() <= 1.0 / 64.0,
            "{content}: {first:?}"
        );
        let aligned = first.advance - first.hanging_advance;
        assert!(
            (aligned - visible).abs() <= 1.0 / 64.0,
            "{content}: {first:?}"
        );
        let right = 10.0 + first.offset + aligned;
        assert!(
            (right - 100.0).abs() <= 1.0 / 64.0,
            "{content}: right edge {right}"
        );
        drop(doc);
    }
}

fn author_zwsp_closing_fragment(
    tail: &str,
    before_zwsp: &str,
    width: u16,
    bounds: [f64; 2],
    lines: usize,
    inline_box: bool,
) {
    let html = format!(
        "<!doctype html><style>body{{margin:0;font:14px/24px Arial}}\
         div{{margin-left:10px;width:{width}px;text-align:right}}\
         #target{{padding-right:6px;background:#0f0}}\
         .box{{display:inline-block;width:30px;height:10px;background:#00f}}</style>\
         <div>ab <span id=target>cd{before_zwsp}&#x200b;</span>{tail}</div>"
    );
    check_zwsp_fragment(
        &html,
        bounds,
        lines,
        None,
        None,
        inline_box.then_some([40.0, 31.0, 30.0]),
    );
}

#[test]
fn text_after_a_zwsp_and_closing_edge_wraps_when_it_overflows() {
    author_zwsp_closing_fragment("xyzxyzxyzxyz", "", 90, [79.203_125, 100.0], 2, false);
    author_zwsp_closing_fragment("xyz", "", 90, [58.203_125, 79.0], 1, false);
}

#[test]
fn a_closing_edge_after_an_author_zwsp_stays_before_block_end_and_br() {
    for (tail, lines) in [("", 1), ("<br>xyz", 2)] {
        author_zwsp_closing_fragment(tail, "", 90, [79.203_125, 100.0], lines, false);
        author_zwsp_closing_fragment(tail, " ", 90, [75.312_5, 100.0], lines, false);
    }
}

#[test]
fn a_closing_edge_after_an_author_zwsp_stays_when_an_inline_block_wraps() {
    let inline_box = "<span class=box></span>";
    author_zwsp_closing_fragment(inline_box, "", 60, [49.203_125, 70.0], 2, true);
    author_zwsp_closing_fragment(inline_box, " ", 60, [45.312_5, 70.0], 2, true);
}

#[test]
fn an_author_zwsp_keeps_a_space_that_hangs_before_an_inserted_mark() {
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::<[u8; 4]>::new();
    for text in ["aaaa bbbb \u{200b}ccccccccc", "aaaa bbbb\u{200b} ccccccccc"] {
        let style = parley::TextStyle::default();
        let mut tree = layouts.tree_builder(&mut fonts, 1.0, true, &style);
        tree.push_text(text);
        let (tree, source) = tree.build();
        assert_eq!(source, text, "tree builder for {text:?}");
        let (prefix, suffix) = text.split_once('\u{200b}').expect("author ZWSP");
        let mut marked = layouts.tree_builder(&mut fonts, 1.0, true, &style);
        marked.push_text(prefix);
        marked.push_inline_end_mark("\u{200b}");
        marked.push_text(suffix);
        let (marked, source) = marked.build();
        assert_eq!(source, text, "registered marker for {text:?}");
        let ranged = layouts
            .ranged_builder(&mut fonts, text, 1.0, true)
            .build(text);
        let mut runs = layouts.style_run_builder(&mut fonts, text, 1.0, true);
        let style_index = runs.push_style(style);
        runs.push_style_run(style_index, ..);
        let metrics = |mut layout: parley::Layout<[u8; 4]>| {
            let intrinsic = layout.calculate_content_widths();
            layout.break_all_lines(Some(100.0));
            let lines = layout
                .lines()
                .map(|line| {
                    let metrics = line.metrics();
                    (line.text_range(), metrics.advance, metrics.hanging_advance)
                })
                .collect::<Vec<_>>();
            (intrinsic.min, intrinsic.max, lines)
        };
        let expected = metrics(tree);
        assert_eq!(metrics(ranged), expected, "ranged builder for {text:?}");
        let style_run_metrics = metrics(runs.build(text));
        assert_eq!(
            style_run_metrics, expected,
            "style-run builder for {text:?}"
        );
        let (_, _, marked_lines) = metrics(marked);
        let author_lines = &expected.2;
        let line_count = author_lines.len();
        assert_eq!(marked_lines.len(), line_count, "line count for {text:?}");
        for (marked, author) in marked_lines.iter().zip(author_lines) {
            assert_eq!(marked.0, author.0, "line range for {text:?}");
        }
        let author_hanging = author_lines[0].2;
        let marked_hanging = marked_lines[0].2;
        if prefix.ends_with(' ') {
            assert_eq!(author_hanging, 0.0, "author ZWSP in {text:?}");
            assert!(marked_hanging > 0.0, "synthetic marker in {text:?}");
        } else {
            assert_eq!(marked_hanging, author_hanging, "{text:?}");
        }
    }
}
