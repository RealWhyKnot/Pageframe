mod support;

use pageframe::App;
use support::{open, pixel, render};

#[test]
fn native_checkbox_and_radio_ignore_author_padding_and_border() {
    let page = r#"<html><body style="margin:0;background:white">
<input id="plain" type="checkbox">
<input id="padded" type="checkbox" style="padding:5px">
<input id="bordered" type="checkbox" style="border:4px solid rgb(255,0,0)">
<input id="roomy" type="radio" style="padding:10px">
<input id="zoomed" type="checkbox" style="zoom:2;padding:2px">
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    let sizes = ["#plain", "#padded", "#bordered", "#roomy", "#zoomed"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the input exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| (rect.width, rect.height))
            .expect("the input has a box")
    });
    assert_eq!(
        sizes,
        [
            (13.0, 13.0),
            (13.0, 13.0),
            (13.0, 13.0),
            (13.0, 13.0),
            (26.0, 26.0)
        ]
    );
    let computed = [
        ("#bordered", "border-top-width"),
        ("#bordered", "border-left-width"),
        ("#padded", "padding-left"),
        ("#roomy", "padding-top"),
    ]
    .map(|(selector, property)| {
        let doc = harness.base();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the input exists");
        doc.resolved_style_value(id, property)
    });
    assert_eq!(computed, ["0px", "0px", "0px", "0px"]);
    let red = buffer
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|rgba| rgba[0] > 200 && rgba[1] < 60 && rgba[2] < 60)
        .count();
    assert_eq!(red, 0);
}

#[test]
fn appearance_none_drops_the_native_checkbox_and_radio() {
    let page = r#"<html><body style="margin:0;background:white">
<input id="bare" type="checkbox" checked style="appearance:none;position:absolute;left:0;top:0;margin:0">
<input id="dot" type="radio" checked style="appearance:none;position:absolute;left:30px;top:0;margin:0">
<input type="checkbox" checked style="appearance:none;position:absolute;left:60px;top:0;width:20px;height:20px;margin:0;background:rgb(255,165,0)">
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    let sizes = ["#bare", "#dot"].map(|selector| {
        let doc = harness.base_mut();
        let id = doc
            .query_selector(selector)
            .expect("valid selector")
            .expect("the input exists");
        doc.get_client_bounding_rect(id)
            .map(|rect| (rect.width, rect.height))
            .expect("the input has a box")
    });
    assert_eq!(sizes, [(0.0, 0.0), (0.0, 0.0)]);
    assert_eq!(pixel(&buffer, 63, 16), [255, 165, 0, 255]);
    assert_eq!(pixel(&buffer, 70, 10), [255, 165, 0, 255]);
}

#[test]
fn dark_color_scheme_themes_native_toggles_and_preserves_custom_paint() {
    let page = r#"<!doctype html><html style="color-scheme:dark"><style>
body { margin: 0; background: rgb(16, 17, 18); }
input { position: absolute; top: 4px; width: 16px; height: 16px; margin: 0; }
.light { color-scheme: light; }
.custom { appearance: none; background: rgb(32, 42, 52); border: 2px solid rgb(90, 100, 110); }
</style><body>
<input type="checkbox" style="left:4px">
<input type="radio" style="left:24px">
<input class="light" type="checkbox" style="left:44px">
<input class="custom" type="checkbox" style="left:64px">
<input class="custom" type="checkbox" style="left:84px" checked>
</body></html>"#;
    let mut harness = open(App::new((), page));
    let buffer = render(&mut harness);
    assert_eq!(pixel(&buffer, 12, 12), [59, 59, 59, 255]);
    assert_eq!(pixel(&buffer, 32, 12), [59, 59, 59, 255]);
    assert_eq!(pixel(&buffer, 52, 12), [255, 255, 255, 255]);
    for left in [64, 84] {
        assert_eq!(pixel(&buffer, left + 8, 12), [32, 42, 52, 255]);
        assert_eq!(pixel(&buffer, left, 12), [90, 100, 110, 255]);
    }
}
