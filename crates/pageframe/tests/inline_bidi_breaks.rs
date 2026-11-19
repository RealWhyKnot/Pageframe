use pageframe::{App, Page};
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

const BREAKS: &str = "\
<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div{min-height:24px}</style>\n\
<div dir=\"rtl\" style=\"unicode-bidi:bidi-override\"><i id=\"a1\">a</i><i id=\"a2\">b</i><br><i id=\"a3\">c</i><i id=\"a4\">d</i></div>\n\
<div><i id=\"b0\">x</i><span dir=\"rtl\" style=\"unicode-bidi:bidi-override\"><i id=\"b1\">a</i><i id=\"b2\">b</i><br><i id=\"b3\">c</i><i id=\"b4\">d</i></span><i id=\"b5\">y</i></div>\n\
<div><i id=\"c0\">x</i><span dir=\"rtl\" style=\"unicode-bidi:isolate\"><i id=\"c1\">&#1488;</i> <i id=\"c2\">b</i><br><i id=\"c3\">c</i> <i id=\"c4\">&#1489;</i></span><i id=\"c5\">y</i></div>\n\
<div><i id=\"d0\">x</i><span dir=\"rtl\" style=\"unicode-bidi:embed\"><i id=\"d1\">&#1488;</i> <i id=\"d2\">b</i><br><i id=\"d3\">c</i> <i id=\"d4\">&#1489;</i></span><i id=\"d5\">y</i></div>\n\
<div style=\"unicode-bidi:plaintext\"><i id=\"e1\">&#1488;</i><i id=\"e2\">b</i><br><i id=\"e3\">c</i><i id=\"e4\">&#1489;</i></div>\n\
<div style=\"unicode-bidi:plaintext;white-space:pre\"><i id=\"f1\">&#1488;</i><i id=\"f2\">b</i>\n\
<i id=\"f3\">c</i><i id=\"f4\">&#1489;</i></div>\n\
<div dir=\"rtl\" style=\"unicode-bidi:bidi-override;white-space:pre\"><i id=\"g1\">a</i><i id=\"g2\">b</i>\n\
<i id=\"g3\">c</i><i id=\"g4\">d</i></div>";

const BREAKS_1X: [(&str, [f64; 4]); 26] = [
    ("a1", [18701.0, 0.0, 499.0, 1024.0]),
    ("a2", [18202.0, 0.0, 499.0, 1024.0]),
    ("a3", [18752.0, 1024.0, 448.0, 1024.0]),
    ("a4", [18253.0, 1024.0, 499.0, 1024.0]),
    ("b0", [0.0, 2048.0, 448.0, 1024.0]),
    ("b1", [947.0, 2048.0, 499.0, 1024.0]),
    ("b2", [448.0, 2048.0, 499.0, 1024.0]),
    ("b3", [499.0, 3072.0, 448.0, 1024.0]),
    ("b4", [0.0, 3072.0, 499.0, 1024.0]),
    ("b5", [947.0, 3072.0, 448.0, 1024.0]),
    ("c0", [0.0, 4096.0, 448.0, 1024.0]),
    ("c1", [1196.0, 4096.0, 505.0, 1024.0]),
    ("c2", [448.0, 4096.0, 499.0, 1024.0]),
    ("c3", [735.0, 5120.0, 448.0, 1024.0]),
    ("c4", [0.0, 5120.0, 486.0, 1024.0]),
    ("c5", [1183.0, 5120.0, 448.0, 1024.0]),
    ("d0", [0.0, 6144.0, 448.0, 1024.0]),
    ("d1", [1196.0, 6144.0, 505.0, 1024.0]),
    ("d2", [448.0, 6144.0, 499.0, 1024.0]),
    ("d3", [735.0, 7168.0, 448.0, 1024.0]),
    ("d4", [0.0, 7168.0, 486.0, 1024.0]),
    ("d5", [1183.0, 7168.0, 448.0, 1024.0]),
    ("g1", [18701.0, 12288.0, 499.0, 1024.0]),
    ("g2", [18202.0, 12288.0, 499.0, 1024.0]),
    ("g3", [18752.0, 13312.0, 448.0, 1024.0]),
    ("g4", [18253.0, 13312.0, 499.0, 1024.0]),
];

const BREAKS_2X: [(&str, [f64; 4]); 26] = [
    ("a1", [18701.5, 0.0, 498.5, 992.0]),
    ("a2", [18203.0, 0.0, 498.5, 992.0]),
    ("a3", [18752.0, 1024.0, 448.0, 992.0]),
    ("a4", [18253.5, 1024.0, 498.5, 992.0]),
    ("b0", [0.0, 2048.0, 448.0, 992.0]),
    ("b1", [946.5, 2048.0, 498.5, 992.0]),
    ("b2", [448.0, 2048.0, 498.5, 992.0]),
    ("b3", [498.5, 3072.0, 448.0, 992.0]),
    ("b4", [0.0, 3072.0, 498.5, 992.0]),
    ("b5", [946.5, 3072.0, 448.0, 992.0]),
    ("c0", [0.0, 4096.0, 448.0, 992.0]),
    ("c1", [1195.5, 4096.0, 504.5, 992.0]),
    ("c2", [448.0, 4096.0, 498.5, 992.0]),
    ("c3", [735.0, 5120.0, 448.0, 992.0]),
    ("c4", [0.0, 5120.0, 486.0, 992.0]),
    ("c5", [1183.0, 5120.0, 448.0, 992.0]),
    ("d0", [0.0, 6144.0, 448.0, 992.0]),
    ("d1", [1195.5, 6144.0, 504.5, 992.0]),
    ("d2", [448.0, 6144.0, 498.5, 992.0]),
    ("d3", [735.0, 7168.0, 448.0, 992.0]),
    ("d4", [0.0, 7168.0, 486.0, 992.0]),
    ("d5", [1183.0, 7168.0, 448.0, 992.0]),
    ("g1", [18701.5, 12288.0, 498.5, 992.0]),
    ("g2", [18203.0, 12288.0, 498.5, 992.0]),
    ("g3", [18752.0, 13312.0, 448.0, 992.0]),
    ("g4", [18253.5, 13312.0, 498.5, 992.0]),
];

const SPACES: &str = "\
<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div{min-height:24px}</style>\n\
<div><i id=\"a1\">a</i> <span dir=\"rtl\" id=\"a0\"> <i id=\"a2\">b</i></span></div>\n\
<div><i id=\"b1\">a</i> <span dir=\"rtl\" id=\"b0\" style=\"unicode-bidi:normal\"> <i id=\"b2\">b</i></span></div>\n\
<div><span dir=\"rtl\" id=\"c0\"><i id=\"c1\">a</i> </span> <i id=\"c2\">b</i></div>\n\
<div><i id=\"d1\">a</i> <bdo dir=\"rtl\" id=\"d0\"> <i id=\"d2\">b</i> </bdo> <i id=\"d3\">c</i></div>\n\
<div style=\"width:30px\"><i id=\"e1\">aa</i> <span dir=\"rtl\" id=\"e0\"> <i id=\"e2\">bb</i></span></div>";

const SPACES_1X: [(&str, [f64; 4]); 14] = [
    ("a1", [0.0, 0.0, 499.0, 1024.0]),
    ("a0", [748.0, 0.0, 499.0, 1024.0]),
    ("a2", [748.0, 0.0, 499.0, 1024.0]),
    ("b1", [0.0, 1536.0, 499.0, 1024.0]),
    ("b0", [748.0, 1536.0, 499.0, 1024.0]),
    ("b2", [748.0, 1536.0, 499.0, 1024.0]),
    ("c0", [0.0, 3072.0, 748.0, 1024.0]),
    ("c1", [249.0, 3072.0, 499.0, 1024.0]),
    ("c2", [748.0, 3072.0, 499.0, 1024.0]),
    ("d1", [0.0, 4608.0, 499.0, 1024.0]),
    ("d0", [748.0, 4608.0, 748.0, 1024.0]),
    ("d2", [997.0, 4608.0, 499.0, 1024.0]),
    ("d3", [1496.0, 4608.0, 448.0, 1024.0]),
    ("e0", [0.0, 7168.0, 997.0, 1024.0]),
];

const SPACES_2X: [(&str, [f64; 4]); 14] = [
    ("a1", [0.0, 0.0, 498.5, 992.0]),
    ("a0", [747.5, 0.0, 498.5, 992.0]),
    ("a2", [747.5, 0.0, 498.5, 992.0]),
    ("b1", [0.0, 1536.0, 498.5, 992.0]),
    ("b0", [747.5, 1536.0, 498.5, 992.0]),
    ("b2", [747.5, 1536.0, 498.5, 992.0]),
    ("c0", [0.0, 3072.0, 747.5, 992.0]),
    ("c1", [249.0, 3072.0, 498.5, 992.0]),
    ("c2", [747.5, 3072.0, 498.5, 992.0]),
    ("d1", [0.0, 4608.0, 498.5, 992.0]),
    ("d0", [747.5, 4608.0, 747.5, 992.0]),
    ("d2", [996.5, 4608.0, 498.5, 992.0]),
    ("d3", [1495.0, 4608.0, 448.0, 992.0]),
    ("e0", [0.0, 7168.0, 997.0, 992.0]),
];
fn open_at(page: &'static str, scale: u16) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(320, 480)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        let size = |css: u32| css * u32::from(scale);
        doc.set_viewport(Viewport::new(
            size(320),
            size(480),
            f32::from(scale),
            color_scheme,
        ));
    }
    harness.pump();
    harness
}

fn check(page: &'static str, scale: u16, chrome: &[(&'static str, [f64; 4])]) {
    let mut harness = open_at(page, scale);
    let doc = harness.base_mut();
    let found: Vec<(&str, [f64; 4])> = chrome
        .iter()
        .flat_map(|(id, _)| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("every probed id exists");
            doc.node_client_rects(node).into_iter().map(move |rect| {
                let values = [rect.x, rect.y, rect.width, rect.height];
                (*id, values.map(|value| value * 64.0))
            })
        })
        .collect();
    let ids: Vec<&str> = found.iter().map(|(id, _)| *id).collect();
    let expected: Vec<&str> = chrome.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, expected, "one rect per id at {scale}x");
    let wrong: Vec<String> = found
        .iter()
        .zip(chrome)
        .filter(|((_, have), (_, want))| {
            have.iter()
                .zip(want)
                .any(|(have, want)| (have - want).abs() >= 0.01)
        })
        .map(|((id, have), (_, want))| format!("{scale}x #{id}: {have:?}, Chrome {want:?}"))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn bidi_contexts_reopen_after_forced_breaks_like_chrome() {
    check(BREAKS, 1, &BREAKS_1X);
}

#[test]
fn bidi_contexts_reopen_after_forced_breaks_at_2x_like_chrome() {
    check(BREAKS, 2, &BREAKS_2X);
}

#[test]
fn bidi_controls_leave_collapsible_spaces_alone_like_chrome() {
    check(SPACES, 1, &SPACES_1X);
}

#[test]
fn bidi_controls_leave_collapsible_spaces_alone_at_2x_like_chrome() {
    check(SPACES, 2, &SPACES_2X);
}
