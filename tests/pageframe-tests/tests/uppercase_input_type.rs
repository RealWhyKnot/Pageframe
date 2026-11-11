//! The `type` attribute of `<input>` is matched case-insensitively.

use pageframe_test_harness::Harness;

#[test]
fn uppercase_checkbox_and_radio_toggle_on_click() {
    let mut h = Harness::from_html(
        r#"<html><body>
            <input type="CHECKBOX" id="check">
            <input type="Radio" id="radio" name="r">
        </body></html>"#,
    );
    assert!(h.query("#check:checked").is_none());
    h.click("#check");
    assert!(h.query("#check:checked").is_some());
    h.click("#check");
    assert!(h.query("#check:checked").is_none());

    h.click("#radio");
    assert!(h.query("#radio:checked").is_some());
}

#[test]
fn uppercase_types_lay_out_like_lowercase() {
    let h = Harness::from_html(
        r#"<html><body>
            <div><input type="password" id="lower-password"></div>
            <div><input type="PASSWORD" id="upper-password"></div>
            <div><input type="checkbox" id="lower-check"></div>
            <div><input type="CheckBox" id="upper-check"></div>
            <div><input type="HIDDEN" id="hidden"></div>
        </body></html>"#,
    );
    let lower = h.layout_rect("#lower-password");
    let upper = h.layout_rect("#upper-password");
    assert_eq!((lower.width, lower.height), (upper.width, upper.height));

    let lower = h.layout_rect("#lower-check");
    let upper = h.layout_rect("#upper-check");
    assert_eq!((lower.width, lower.height), (upper.width, upper.height));

    let hidden = h.layout_rect("#hidden");
    assert_eq!((hidden.width, hidden.height), (0.0, 0.0));
}
