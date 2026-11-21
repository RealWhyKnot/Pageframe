use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const STYLE: &str =
    "<!doctype html><style>html,body{margin:0;font:14px Arial}body{width:300px}</style>";

const SIZES: &str = "<div>ab<span style=\"font-size:40px\" id=a1></span>cd</div>\
<div style=\"font:16px/20px Arial\">x<span style=\"font-size:32px\" id=b1></span></div>\
<div>x<span style=\"line-height:60px\" id=c1></span></div>\
<div style=\"font:16px/20px Arial\">x<span style=\"line-height:40px\" id=d1></span></div>\
<div>ab<span id=e1></span>cd</div>\
<div>ab <span id=f1></span>cd</div>\
<div style=\"width:30px\">abc <span id=g1></span>def</div>\
<div><span id=h1></span></div>\
<div>ab<span id=i1><span id=i2></span></span>cd</div>\
<div style=\"padding:5px\">ab<span id=j1 style=\"vertical-align:10px\"></span>cd</div>";

const PLACES: &str = "<div dir=rtl>ab<span id=a1></span>cd</div>\
<div dir=rtl style=\"padding:3px 7px\"><span id=b1></span></div>\
<div style=\"padding:3px 7px\"><span id=c1></span></div>\
<div>a<br><span id=d1></span><br>b</div>\
<div style=\"writing-mode:vertical-rl;height:100px\">ab<span id=e1></span>cd</div>\
<div>ab<span id=f1 style=\"position:relative;left:5px;top:3px\"></span>cd</div>\
<div>ab<img style=\"width:10px;height:10px\"><span id=h2></span><img style=\"width:10px;height:10px\">cd</div>\
<div style=\"width:40px\">abcd efgh<span id=i1></span> ijkl</div>\
<div style=\"text-align:center\">ab<span id=j1></span></div>";

const SPACES: &str = "<div>ab <span id=a1></span>cd</div>\
<div>ab<span id=b1></span> cd</div>\
<div>ab <span id=c1></span> cd</div>\
<div>ab<span> <span id=d1></span></span>cd</div>\
<div style=\"white-space:pre\">ab <span id=e1></span>cd</div>\
<div>ab<span id=f1 style=\"vertical-align:-4px\"></span>cd</div>\
<div>ab<span id=g1 style=\"vertical-align:super\"></span>cd</div>\
<div>ab <span id=k1>x</span>cd</div>\
<div>ab <span id=l1></span>cd <span id=l2></span>ef</div>";

fn open(body: &str, scale: f32) -> Harness<Page<()>> {
    let page = App::new((), &format!("{STYLE}{body}"))
        .page(640, 800)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        let size = |css: u16| {
            num_traits::ToPrimitive::to_u32(&(f32::from(css) * scale).round())
                .expect("test viewports fit in u32")
        };
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            size(640),
            size(800),
            scale,
            color_scheme,
        ));
    }
    harness.pump();
    harness
}

fn rects(harness: &Harness<Page<()>>, ids: &[&str]) -> Vec<(String, Vec<[i64; 4]>)> {
    let doc = harness.base();
    ids.iter()
        .map(|id| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the element exists");
            let rects = doc
                .node_client_rects(node)
                .iter()
                .map(|rect| {
                    [rect.x, rect.y, rect.width, rect.height].map(|value| {
                        num_traits::ToPrimitive::to_i64(&(value * 64.0).round())
                            .expect("test rects fit in i64")
                    })
                })
                .collect();
            ((*id).to_owned(), rects)
        })
        .collect()
}

fn expected(rows: &[(&str, [i64; 4])]) -> Vec<(String, Vec<[i64; 4]>)> {
    rows.iter()
        .map(|(id, rect)| ((*id).to_owned(), vec![*rect]))
        .collect()
}

#[test]
fn empty_spans_get_a_caret_rect_sized_by_their_own_font_like_chrome() {
    let ids = [
        "a1", "b1", "c1", "d1", "e1", "f1", "g1", "h1", "i1", "i2", "j1",
    ];
    assert_eq!(
        rects(&open(SIZES, 1.0), &ids),
        expected(&[
            ("a1", [997, 0, 0, 2816]),
            ("b1", [512, 2368, 0, 2304]),
            ("c1", [448, 5952, 0, 1024]),
            ("d1", [512, 9088, 0, 1088]),
            ("e1", [997, 10944, 0, 1024]),
            ("f1", [1246, 11968, 0, 1024]),
            ("g1", [1445, 12992, 0, 1024]),
            ("h1", [0, 15040, 0, 0]),
            ("i1", [997, 15040, 0, 1024]),
            ("i2", [997, 15040, 0, 1024]),
            ("j1", [1317, 16384, 0, 1024]),
        ])
    );
}

#[test]
fn empty_spans_sit_at_the_caret_for_direction_padding_breaks_and_alignment_like_chrome() {
    let ids = ["a1", "b1", "c1", "d1", "e1", "f1", "h2", "i1", "j1"];
    assert_eq!(
        rects(&open(PLACES, 1.0), &ids),
        expected(&[
            ("a1", [18253, 0, 0, 1024]),
            ("b1", [18752, 1216, 0, 0]),
            ("c1", [448, 1600, 0, 0]),
            ("d1", [0, 2816, 0, 1024]),
            ("e1", [0, 5861, 1024, 0]),
            ("f1", [1317, 11456, 0, 1024]),
            ("h2", [1637, 12288, 0, 1024]),
            ("i1", [1744, 14336, 0, 1024]),
            ("j1", [10098, 16384, 0, 1024]),
        ])
    );
}

#[test]
fn empty_spans_follow_the_source_side_of_collapsed_spaces_and_their_baseline_shift() {
    let ids = ["a1", "b1", "c1", "d1", "e1", "f1", "g1", "k1", "l1", "l2"];
    assert_eq!(
        rects(&open(SPACES, 1.0), &ids),
        expected(&[
            ("a1", [1246, 0, 0, 1024]),
            ("b1", [997, 1024, 0, 1024]),
            ("c1", [1246, 2048, 0, 1024]),
            ("d1", [1246, 3072, 0, 1024]),
            ("e1", [1246, 4096, 0, 1024]),
            ("f1", [997, 5376, 0, 1024]),
            ("g1", [997, 6400, 0, 1024]),
            ("k1", [1246, 7786, 448, 1024]),
            ("l1", [1246, 8810, 0, 1024]),
            ("l2", [2442, 8810, 0, 1024]),
        ])
    );
}

#[test]
fn empty_span_baseline_shifts_use_device_units_at_fractional_scales_like_chrome() {
    let rows = rects(&open(SPACES, 1.75), &["f1", "g1"]);
    let tops: Vec<i64> = rows.iter().map(|(_, rects)| rects[0][1]).collect();
    assert_eq!(tops, vec![5376, 6400]);
}
