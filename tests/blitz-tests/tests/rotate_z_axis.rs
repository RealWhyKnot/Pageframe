//! The `rotate` property's axis form is a planar rotation when the axis is z,
//! the same one `rotate: <angle>` gives, and has to be applied like it.
//!
//! <https://drafts.csswg.org/css-transforms-2/#individual-transforms>
//!
//! Every axis form was dropped as 3d, so `rotate: z 90deg` did nothing while
//! `rotate: 90deg` worked. An axis that leaves the plane is flattened onto it.

use blitz_test_harness::Harness;

fn transform_coeffs(harness: &Harness, selector: &str) -> Option<[f64; 6]> {
    let node_id = harness.node(selector);
    harness
        .base()
        .get_node(node_id)
        .unwrap()
        .transform()
        .as_deref()
        .map(|affine| affine.as_coeffs())
}

fn page() -> Harness {
    Harness::from_html(
        r#"<html><body style="margin:0">
            <div id="plain" style="width:20px; height:20px; rotate: 90deg"></div>
            <div id="z" style="width:20px; height:20px; rotate: z 90deg"></div>
            <div id="vector" style="width:20px; height:20px; rotate: 0 0 1 90deg"></div>
            <div id="negative" style="width:20px; height:20px; rotate: 0 0 -1 90deg"></div>
            <div id="reverse" style="width:20px; height:20px; rotate: -90deg"></div>
            <div id="x" style="width:20px; height:20px; rotate: x 90deg"></div>
        </body></html>"#,
    )
}

#[test]
fn a_z_axis_rotate_matches_the_plain_angle() {
    let harness = page();
    let plain = transform_coeffs(&harness, "#plain").expect("rotate: <angle> must apply");
    assert_eq!(
        transform_coeffs(&harness, "#z"),
        Some(plain),
        "rotate: z <angle>"
    );
    assert_eq!(
        transform_coeffs(&harness, "#vector"),
        Some(plain),
        "rotate: 0 0 1 <angle>"
    );
}

#[test]
fn the_sign_of_the_axis_turns_the_rotation_around() {
    let harness = page();
    let reverse = transform_coeffs(&harness, "#reverse").expect("rotate: <angle> must apply");
    assert_eq!(transform_coeffs(&harness, "#negative"), Some(reverse));
}

#[test]
fn a_rotation_leaving_the_plane_is_flattened_onto_it() {
    let harness = page();
    let [a, b, c, d, e, f] =
        transform_coeffs(&harness, "#x").expect("rotate: x <angle> must apply");
    assert_eq!([a, b, c, e], [1.0, 0.0, 0.0, 0.0]);
    assert!(d.abs() < 1e-9, "a quarter turn about x leaves no height, got {d}");
    assert!((f - 10.0).abs() < 1e-9, "the box collapses onto its centre line, got {f}");
}
