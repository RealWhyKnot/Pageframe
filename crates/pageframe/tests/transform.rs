mod scaled;
mod support;

use pageframe::App;
use scaled::render_at_scale;
use support::{HEIGHT, WIDTH, open, pixel, render};

fn transformed_extent(transform: &str) -> (usize, usize) {
    let page = format!(
        r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:40px;top:20px;width:40px;height:40px;background:rgb(0,0,200);transform:{transform}"></div>
</body></html>"#
    );
    let buffer = render(&mut open(App::new((), &page)));
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    let blue = |x: usize, y: usize| pixel(&buffer, x, y)[0] < 128;
    let columns = (0..width)
        .filter(|&x| (0..height).any(|y| blue(x, y)))
        .count();
    let rows = (0..height)
        .filter(|&y| (0..width).any(|x| blue(x, y)))
        .count();
    (columns, rows)
}

#[test]
fn three_d_rotations_flatten_onto_the_page() {
    assert_eq!(transformed_extent("rotateX(60deg)"), (40, 20));
    assert_eq!(transformed_extent("rotateY(60deg)"), (20, 40));
    assert_eq!(
        transformed_extent("rotateX(45deg) rotateX(-45deg)"),
        (40, 40)
    );
    assert_eq!(transformed_extent("scale3d(1.5, 0.5, 3)"), (60, 20));
}

#[test]
fn translate_z_keeps_the_rest_of_the_transform() {
    assert_eq!(transformed_extent("translateZ(50px)"), (40, 40));
    assert_eq!(
        transformed_extent("translateZ(50px) rotate(10deg)"),
        (46, 46)
    );
}

fn painted(page: &str) -> Vec<Vec<bool>> {
    let buffer = render(&mut open(App::new((), page)));
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    (0..height)
        .map(|y| (0..width).map(|x| pixel(&buffer, x, y)[0] < 128).collect())
        .collect()
}

fn column_heights(rows: &[Vec<bool>]) -> Vec<usize> {
    let width = rows.first().map_or(0, Vec::len);
    (0..width)
        .map(|x| rows.iter().filter(|row| row[x]).count())
        .filter(|&count| count > 0)
        .collect()
}

fn row_widths(rows: &[Vec<bool>]) -> Vec<usize> {
    rows.iter()
        .map(|row| row.iter().filter(|&&inked| inked).count())
        .filter(|&count| count > 0)
        .collect()
}

#[test]
fn perspective_draws_a_turned_box_as_a_trapezoid() {
    let rows = painted(
        r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:40px;top:20px;width:40px;height:40px;background:rgb(0,0,200);transform:perspective(200px) rotateY(45deg)"></div>
</body></html>"#,
    );
    let heights = column_heights(&rows);
    assert!(
        (27..=29).contains(&heights.len()),
        "{} columns",
        heights.len()
    );
    let near = heights[1];
    let far = heights[heights.len() - 2];
    assert!((41..=43).contains(&near), "near side is {near} tall");
    assert!((37..=39).contains(&far), "far side is {far} tall");
}

#[test]
fn perspective_leaves_no_seams_in_translucent_boxes() {
    let page = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:40px;top:20px;width:60px;height:40px;background:rgba(0,0,255,0.5);transform:perspective(200px) rotateY(-40deg) rotateX(20deg)"></div>
</body></html>"#;
    let buffer = render(&mut open(App::new((), page)));
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let height = usize::try_from(HEIGHT).expect("test height fits in usize");
    let red = |x: usize, y: usize| pixel(&buffer, x, y)[0];
    let mut inside = 0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let surrounded = (y - 1..=y + 1).all(|ny| (x - 1..=x + 1).all(|nx| red(nx, ny) < 200));
            if surrounded {
                inside += 1;
                assert!(
                    red(x, y).abs_diff(127) <= 3,
                    "red {} at ({x}, {y})",
                    red(x, y)
                );
            }
        }
    }
    assert!(inside > 1000, "{inside} pixels inside");
}

#[test]
fn perspective_draws_the_part_in_front_of_the_viewer() {
    let rows = painted(
        r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:20px;top:20px;width:80px;height:40px;background:rgb(0,0,200);transform:perspective(30px) rotateY(70deg)"></div>
</body></html>"#,
    );
    let height = |x: usize| rows.iter().filter(|row| row[x]).count();
    assert_eq!(height(2), rows.len());
    assert!(
        (50..=54).contains(&height(56)),
        "column 56 is {}",
        height(56)
    );
    assert!(
        (22..=26).contains(&height(64)),
        "column 64 is {}",
        height(64)
    );
    assert!((68..rows[0].len()).all(|x| height(x) == 0));
}

#[test]
fn perspective_on_an_empty_box_still_turns_its_children() {
    let rows = painted(
        r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:30px;top:20px;width:60px;height:0;transform:perspective(100px) rotateX(50deg)"><div style="width:60px;height:40px;background:rgb(0,0,200)"></div></div>
</body></html>"#,
    );
    let widths = row_widths(&rows);
    let top = widths[1];
    let bottom = widths[widths.len() - 2];
    assert!(bottom > top + 15, "rows go from {top} to {bottom} wide");
}

const TURNED: &str = "position:absolute;left:4px;top:4px;width:40px;height:40px;background:rgb(0,0,200);transform:perspective(40px) rotateY(60deg) rotateX(30deg)";

#[test]
fn many_perspective_boxes_leave_layers_for_later_clips() {
    let turned = format!(r#"<div style="{TURNED}"></div>"#).repeat(24);
    let page = format!(
        r#"<html><body style="margin:0;background:white">{turned}
<div style="position:absolute;left:70px;top:20px;width:20px;height:20px;overflow:hidden"><div style="width:50px;height:50px;background:rgb(255,0,0)"></div></div>
</body></html>"#
    );
    let buffer = render(&mut open(App::new((), &page)));
    assert_eq!(pixel(&buffer, 80, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&buffer, 80, 60), [255, 255, 255, 255]);
    assert_eq!(pixel(&buffer, 105, 30), [255, 255, 255, 255]);
}

#[test]
fn perspective_keeps_its_shape_when_children_use_up_the_layers() {
    let blues = |children: &str| -> Vec<u8> {
        let page = format!(
            r#"<html><body style="margin:0;background:white"><div style="{TURNED}">{children}</div></body></html>"#
        );
        render(&mut open(App::new((), &page)))
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|px| px[0] < 8)
            .map(|px| px[2])
            .collect()
    };
    let alone = blues("").len();
    let crowded = blues(&r#"<div style="width:4px;height:1px;overflow:hidden"></div>"#.repeat(40));
    assert!(
        crowded.len().abs_diff(alone) * 50 <= alone,
        "{} blue pixels, {alone} without children",
        crowded.len()
    );
    assert!(
        crowded.iter().all(|&blue| blue.abs_diff(200) <= 3),
        "blue ranges from {:?} to {:?}",
        crowded.iter().min(),
        crowded.iter().max()
    );
}

#[test]
fn perspective_scales_with_the_display() {
    let page = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:40px;top:20px;width:40px;height:40px;background:rgb(0,0,200);transform:perspective(200px) rotateY(45deg)"></div>
</body></html>"#;
    let buffer = render_at_scale(&mut open(App::new((), page)), 2);
    let width = usize::try_from(WIDTH * 2).expect("test width fits in usize");
    let height = buffer.len() / 4 / width;
    let inked = |x: usize, y: usize| buffer[(y * width + x) * 4] < 128;
    let heights: Vec<usize> = (0..width)
        .map(|x| (0..height).filter(|&y| inked(x, y)).count())
        .filter(|&count| count > 0)
        .collect();
    assert!(
        (54..=58).contains(&heights.len()),
        "{} columns",
        heights.len()
    );
    let near = heights[2];
    let far = heights[heights.len() - 3];
    assert!((82..=86).contains(&near), "near side is {near} tall");
    assert!((74..=78).contains(&far), "far side is {far} tall");
}

#[test]
fn rotation_keeps_its_origin_when_the_display_scale_changes() {
    let page = r#"<html><body style="margin:0;background:white">
<div style="position:absolute;left:40px;top:20px;width:40px;height:40px;background:rgb(0,0,200);transform:rotate(45deg)"></div>
</body></html>"#;
    let buffer = render_at_scale(&mut open(App::new((), page)), 2);
    let width = usize::try_from(WIDTH * 2).expect("test width fits in usize");
    let height = buffer.len() / 4 / width;
    let inked = |x: usize, y: usize| buffer[(y * width + x) * 4] < 128;
    let columns: Vec<usize> = (0..width)
        .filter(|&x| (0..height).any(|y| inked(x, y)))
        .collect();
    let rows: Vec<usize> = (0..height)
        .filter(|&y| (0..width).any(|x| inked(x, y)))
        .collect();
    let middle = |span: &[usize]| span.first().zip(span.last()).map(|(a, b)| a + b);
    assert_eq!(
        middle(&columns).map(|sum| sum.abs_diff(239) <= 2),
        Some(true),
        "{columns:?}"
    );
    assert_eq!(
        middle(&rows).map(|sum| sum.abs_diff(159) <= 2),
        Some(true),
        "{rows:?}"
    );
}
