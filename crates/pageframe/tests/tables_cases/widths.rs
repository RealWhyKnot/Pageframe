use super::*;

#[test]
fn a_table_min_width_holds_when_content_overflows_its_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="a" style="width:50px;min-width:200px"><tr><td id="a1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("a", [0.0, 0.0, 200.0, 18.0]),
            ("a1", [2.0, 2.0, 196.0, 14.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="b" style="width:50px;min-width:80px"><tr><td id="b1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("b", [0.0, 0.0, 108.0, 18.0]),
            ("b1", [2.0, 2.0, 104.0, 14.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="f" style="width:50px;min-width:200px;box-sizing:border-box;border:5px solid blue"><tr><td id="f1"><b style="width:100px"></b></td></tr></table></div>"#
        ),
        &[
            ("f", [0.0, 0.0, 200.0, 28.0]),
            ("f1", [7.0, 7.0, 186.0, 14.0]),
        ],
    );
}

#[test]
fn a_table_min_width_stretches_its_columns() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div><table id="g" style="min-width:200px"><tr><td id="g1"><b style="width:40px"></b></td><td id="g2"><b style="width:80px"></b></td></tr></table></div>"#
        ),
        &[
            ("g", [0.0, 0.0, 200.0, 18.0]),
            ("g1", [2.0, 2.0, 66.6875, 14.0]),
            ("g2", [70.6875, 2.0, 127.3125, 14.0]),
        ],
    );
}

#[test]
fn a_table_max_width_wraps_content_in_a_table_without_a_width() {
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="c" style="max-width:60px"><tr><td id="c1"><b style="width:40px"></b><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("c", [0.0, 0.0, 60.0, 38.0]),
            ("c1", [2.0, 2.0, 56.0, 34.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="e" style="max-width:60px"><tr><td id="e1"><b style="width:40px"></b><b style="width:40px"></b></td><td id="e2"><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("e", [0.0, 0.0, 94.0, 28.0]),
            ("e1", [2.0, 2.0, 44.0, 24.0]),
            ("e2", [48.0, 2.0, 44.0, 24.0]),
        ],
    );
    check_table_boxes(
        &format!(
            r#"{SPAN_STYLE}<div style="width:500px"><table id="h" style="max-width:60px;min-width:90px"><tr><td id="h1"><b style="width:40px"></b><b style="width:40px"></b><b style="width:40px"></b></td></tr></table></div>"#
        ),
        &[
            ("h", [0.0, 0.0, 90.0, 28.0]),
            ("h1", [2.0, 2.0, 86.0, 24.0]),
        ],
    );
}

#[test]
fn auto_columns_share_spare_width_in_device_layout_units_like_chrome() {
    let html = "<!doctype html><html><head><style>html,body{margin:0;font:14px Arial}\
                td{padding:0}table{border-spacing:0}</style></head><body>\
                <table style=\"width:150px\"><tr>\
                <td id=wide>wavy lines fold here quick brown</td>\
                <td id=narrow>mild tiny kilt wrap ruby text</td></tr></table></body></html>";
    for (scale, factor, expected) in [
        (1, 1.0, [5308.0 / 64.0, 4292.0 / 64.0]),
        (2, 2.0, [10617.0 / 128.0, 8583.0 / 128.0]),
    ] {
        let mut harness = open(App::new((), html));
        {
            let mut doc = harness.base_mut();
            let color_scheme = doc.get_viewport().color_scheme;
            doc.set_viewport(pageframe_traits::shell::Viewport::new(
                support::WIDTH * scale,
                support::HEIGHT * scale,
                factor,
                color_scheme,
            ));
        }
        harness.pump();
        let doc = harness.base_mut();
        let width = |id: &str| {
            let node = doc
                .query_selector(&format!("#{id}"))
                .expect("valid selector")
                .expect("the cell exists");
            doc.get_client_bounding_rect(node)
                .expect("the cell is laid out")
                .width
        };
        assert_eq!([width("wide"), width("narrow")], expected, "scale {factor}");
    }
}
