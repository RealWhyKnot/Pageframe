use super::*;
use pageframe::Page;
use pageframe_test_harness::Harness;

const SPACED: &str = "<style>html,body{margin:0;font:14px Arial}table{border-collapse:separate;border:0}td{border:1px solid #36c;padding:3px}</style><table id='t' style='border-spacing:2.3px 5.7px'><tr><td id='a'>a</td><td id='b'>b</td></tr><tr><td id='c'>c</td><td id='d'>d</td></tr></table><table id='z' style='border-spacing:0.3px 1.2px'><tr><td>x</td></tr></table>";

fn open_at(scale: f32) -> Harness<Page<()>> {
    let mut harness = open(App::new((), SPACED));
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(pageframe_traits::shell::Viewport::new(
            support::WIDTH * 2,
            support::HEIGHT * 2,
            scale,
            color_scheme,
        ));
    }
    harness.pump();
    harness
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node = doc
        .query_selector_all(&format!("#{id}"))
        .expect("cell selector is valid")[0];
    let rect = doc.get_client_bounding_rect(node).expect("cell has a box");
    drop(doc);
    [rect.x, rect.y, rect.width, rect.height]
}

fn spacing(harness: &Harness<Page<()>>, id: &str) -> String {
    let doc = harness.base();
    let node = doc
        .query_selector_all(&format!("#{id}"))
        .expect("table selector is valid")[0];
    doc.resolved_style_value(node, "border-spacing")
}

fn gaps(harness: &Harness<Page<()>>) -> [f64; 4] {
    let [left, top, width, height] = rect(harness, "a");
    let [right, ..] = rect(harness, "b");
    let [_, below, ..] = rect(harness, "c");
    [left, top, right - left - width, below - top - height]
}

#[test]
fn border_spacing_truncates_to_whole_pixels_at_one_x() {
    let harness = open_at(1.0);
    assert_eq!(spacing(&harness, "t"), "2px 5px");
    assert_eq!(spacing(&harness, "z"), "0px 1px");
    assert_eq!(gaps(&harness), [2.0, 5.0, 2.0, 5.0]);
}

#[test]
fn border_spacing_truncates_to_whole_device_pixels_at_seven_quarters_scale() {
    let harness = open_at(1.75);
    assert_eq!(spacing(&harness, "t"), "2.28571px 5.14286px");
    assert_eq!(spacing(&harness, "z"), "0px 1.14286px");
    let device = gaps(&harness).map(|gap| gap * 1.75);
    for (actual, expected) in device.iter().zip([4.0, 9.0, 4.0, 9.0]) {
        assert!((actual - expected).abs() < 1e-3, "{device:?}");
    }
}
