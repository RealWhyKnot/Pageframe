use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::{App, Page};
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;
const STYLE: &str = "html,body{margin:0;background:white}body{font:16px/20px Arial}p{margin:0}\
.c{width:320px;column-count:3;column-gap:10px}.b{height:20px;background:#00ff00}";

const GREEN: [u8; 4] = [0, 255, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn page(body: &str) -> Harness<Page<()>> {
    let html = format!("<html><head><style>{STYLE}</style></head><body>{body}</body></html>");
    let page = App::new((), &html)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn blocks(count: usize) -> String {
    (0..count)
        .map(|index| format!("<div id=\"b{index}\" class=\"b\"></div>"))
        .collect()
}

fn rect(harness: &Harness<Page<()>>, id: &str) -> [f64; 4] {
    let doc = harness.base();
    let node_id = doc
        .query_selector_all(&format!("#{id}"))
        .expect("box selector is valid")[0];
    let rect = doc
        .get_client_bounding_rect(node_id)
        .expect("box has a rectangle");
    drop(doc);
    [rect.x, rect.y, rect.width, rect.height]
}

fn check(harness: &Harness<Page<()>>, expected: &[(&str, [f64; 4])]) {
    for &(id, expected) in expected {
        let actual = rect(harness, id);
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < 0.5),
            "{id}: {actual:?}, expected {expected:?}"
        );
    }
}

fn hit_id(harness: &Harness<Page<()>>, x: f32, y: f32) -> Option<String> {
    let doc = harness.base();
    let hit = doc.hit(x, y)?;
    let mut node = doc.get_node(hit.node_id)?;
    loop {
        if let Some(id) = node.element_data().and_then(|element| element.id.as_ref()) {
            return Some(id.to_string());
        }
        node = doc.get_node(node.parent?)?;
    }
}

fn draw(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 4] {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let start = (y * width + x) * 4;
    let mut rgba = [0; 4];
    rgba.copy_from_slice(&buffer[start..start + 4]);
    rgba
}

#[test]
fn blocks_balance_across_a_column_count() {
    let harness = page(&format!("<div id=\"c\" class=\"c\">{}</div>", blocks(6)));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 40.0]),
            ("b0", [0.0, 0.0, 100.0, 20.0]),
            ("b1", [0.0, 20.0, 100.0, 20.0]),
            ("b2", [110.0, 0.0, 100.0, 20.0]),
            ("b3", [110.0, 20.0, 100.0, 20.0]),
            ("b4", [220.0, 0.0, 100.0, 20.0]),
            ("b5", [220.0, 20.0, 100.0, 20.0]),
        ],
    );
    assert_eq!(hit_id(&harness, 115.0, 5.0).as_deref(), Some("b2"));
    assert_eq!(hit_id(&harness, 225.0, 25.0).as_deref(), Some("b5"));
    assert_eq!(hit_id(&harness, 105.0, 5.0).as_deref(), Some("c"));
}

#[test]
fn a_column_width_sets_the_column_count() {
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"column-count:auto;column-width:90px\">{}</div>",
        blocks(6)
    ));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 40.0]),
            ("b2", [110.0, 0.0, 100.0, 20.0]),
            ("b5", [220.0, 20.0, 100.0, 20.0]),
        ],
    );
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"column-count:2;column-width:90px\">{}</div>",
        blocks(6)
    ));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 60.0]),
            ("b2", [0.0, 40.0, 155.0, 20.0]),
            ("b3", [165.0, 0.0, 155.0, 20.0]),
        ],
    );
}

#[test]
fn a_block_split_between_columns_reports_the_union_box() {
    let harness = page(&format!("<div id=\"c\" class=\"c\">{}</div>", blocks(7)));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 46.671_875]),
            ("b1", [0.0, 20.0, 100.0, 20.0]),
            ("b2", [0.0, 0.0, 210.0, 46.671_875]),
            ("b3", [110.0, 13.328_125, 100.0, 20.0]),
            ("b4", [110.0, 0.0, 210.0, 46.671_875]),
            ("b5", [220.0, 6.656_25, 100.0, 20.0]),
            ("b6", [220.0, 26.656_25, 100.0, 20.0]),
        ],
    );
}

#[test]
fn padding_moves_the_columns_and_a_normal_gap_is_one_em() {
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"padding:8px\">{}</div>",
        blocks(6)
    ));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 336.0, 56.0]),
            ("b0", [8.0, 8.0, 100.0, 20.0]),
            ("b3", [118.0, 28.0, 100.0, 20.0]),
            ("b4", [228.0, 8.0, 100.0, 20.0]),
        ],
    );
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"column-gap:normal;font-size:20px\">{}</div>",
        blocks(6)
    ));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 40.0]),
            ("b0", [0.0, 0.0, 93.328_125, 20.0]),
            ("b2", [113.328_125, 0.0, 93.328_125, 20.0]),
            ("b4", [226.671_875, 0.0, 93.328_125, 20.0]),
        ],
    );
    let exact = rect(&harness, "b4");
    assert!((exact[0] - 226.671_875).abs() < 0.001, "{exact:?}");
    assert!((exact[2] - 93.328_125).abs() < 0.001, "{exact:?}");
}

#[test]
fn text_lines_move_whole_to_the_next_column() {
    let text = (0..5)
        .map(|index| format!("<span id=\"l{index}\">line{index}</span>"))
        .collect::<Vec<_>>()
        .join("<br>");
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"column-count:2;width:220px\"><p id=\"p\">{text}</p></div>"
    ));
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 220.0, 60.0]),
            ("p", [0.0, 0.0, 220.0, 60.0]),
        ],
    );
    let lines = ["l0", "l1", "l2", "l3", "l4"].map(|id| rect(&harness, id));
    let expected = [
        (0.0, 1.0),
        (0.0, 21.0),
        (0.0, 41.0),
        (115.0, 1.0),
        (115.0, 21.0),
    ];
    for (line, (x, y)) in lines.iter().zip(expected) {
        assert!(
            (line[0] - x).abs() < 0.5 && (line[1] - y).abs() < 1.5,
            "{line:?}, expected {x} {y}"
        );
    }
    assert_eq!(hit_id(&harness, 120.0, 8.0).as_deref(), Some("l3"));
    assert_eq!(hit_id(&harness, 120.0, 28.0).as_deref(), Some("l4"));
    assert_eq!(hit_id(&harness, 5.0, 48.0).as_deref(), Some("l2"));
}

#[test]
fn a_bordered_block_is_sliced_without_repeating_its_border() {
    let mut harness = page(&format!(
        "<div class=\"c\" style=\"column-count:2\"><div style=\"border:4px solid red\">{}</div></div>",
        blocks(6)
    ));
    let buffer = draw(&mut harness);
    assert_eq!(pixel(&buffer, 20, 1), RED);
    assert_eq!(pixel(&buffer, 20, 5), GREEN);
    assert_eq!(pixel(&buffer, 20, 62), GREEN);
    assert_eq!(pixel(&buffer, 20, 65), WHITE);
    assert_eq!(pixel(&buffer, 160, 30), WHITE);
    assert_eq!(pixel(&buffer, 180, 1), GREEN);
    assert_eq!(pixel(&buffer, 180, 58), GREEN);
    assert_eq!(pixel(&buffer, 180, 62), RED);
    assert_eq!(pixel(&buffer, 180, 65), WHITE);
}

#[test]
fn a_huge_column_count_stays_bounded() {
    let harness = page(&format!(
        "<div id=\"c\" class=\"c\" style=\"column-count:1000000;column-gap:0\">{}</div>",
        blocks(3)
    ));
    let container = rect(&harness, "c");
    assert!((container[3] - 0.015_625).abs() < 0.001, "{container:?}");
}

#[test]
fn a_container_that_stops_being_a_block_drops_its_columns() {
    let mut harness = page(&format!("<div id=\"c\" class=\"c\">{}</div>", blocks(6)));
    check(&harness, &[("b2", [110.0, 0.0, 100.0, 20.0])]);
    let id = harness
        .base()
        .query_selector_all("#c")
        .expect("the selector parses")[0];
    let name = pageframe_dom::QualName::new(
        None,
        pageframe_dom::ns!(),
        pageframe_dom::local_name!("style"),
    );
    harness
        .base_mut()
        .mutate()
        .set_attribute(id, name, "display:grid");
    harness.pump();
    check(
        &harness,
        &[
            ("c", [0.0, 0.0, 320.0, 120.0]),
            ("b2", [0.0, 40.0, 320.0, 20.0]),
        ],
    );
    assert_eq!(hit_id(&harness, 250.0, 50.0).as_deref(), Some("b2"));
}

#[test]
fn many_blocks_in_one_pixel_columns_lay_out_quickly() {
    let harness = page(&format!(
        "<div id=\"c\" style=\"width:320px;column-width:1px;column-gap:0\">{}</div>",
        blocks(5000)
    ));
    let container = rect(&harness, "c");
    assert!(container[3] >= 20.0, "{container:?}");
}

#[test]
fn column_rules_sit_between_filled_columns() {
    let mut harness = page(&format!(
        "<div class=\"c\" style=\"column-rule:4px solid #f00\">{}</div>\
         <div style=\"margin-top:20px;width:301px;columns:3;column-gap:12px;\
         font:14px Arial;color:#fff;column-rule:3px solid #f00\">\
         One two three four five six seven eight nine</div>",
        blocks(6)
    ));
    let buffer = draw(&mut harness);
    let row = |y: usize| -> Vec<usize> {
        (0..WIDTH)
            .map(|x| usize::try_from(x).expect("test width fits in usize"))
            .filter(|&x| pixel(&buffer, x, y) == RED)
            .collect()
    };
    let both = vec![103, 104, 105, 106, 213, 214, 215, 216];
    assert_eq!(row(0), both);
    assert_eq!(row(39), both);
    assert_eq!(row(40), Vec::<usize>::new());
    assert_eq!(row(60), vec![97, 98, 99]);
    assert_eq!(row(91), vec![97, 98, 99]);
    assert_eq!(row(92), Vec::<usize>::new());
    assert_eq!(pixel(&buffer, 50, 10), GREEN);
    assert_eq!(pixel(&buffer, 108, 10), WHITE);
}

#[test]
fn padded_text_columns_balance_like_chrome() {
    let harness = page(
        "<div id=\"t\" style=\"width:301px;columns:3;column-gap:12px;font:14px Arial;\
         padding:5px 7px;border:3px solid #ddd\">One two three four five six seven eight nine</div>\
         <div id=\"u\" style=\"width:301px;columns:3;column-gap:12px;font:14px Arial;\
         padding:12px 4px 2px\">One two three four five six seven eight nine ten eleven twelve thirteen fourteen</div>",
    );
    check(
        &harness,
        &[
            ("t", [0.0, 0.0, 321.0, 48.0]),
            ("u", [0.0, 48.0, 309.0, 62.0]),
        ],
    );
}
