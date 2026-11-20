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
    padded_span_line(
        "ltr",
        "end",
        "white-space:pre-wrap;padding-right:6px",
        [75, 100],
        24.687_5,
    );
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
