use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::{App, Page};
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

const WIDTH: u32 = 60;
const HEIGHT: u32 = 160;

const LAYERS: &str = "<!doctype html><style>html,body{margin:0;background:#fff;\
font:10px/10px monospace}div{height:20px}</style>\
<div id=a style=\"position:relative;background:#00f\"></div>\
<div id=b><span id=s style=\"position:relative;top:-15px;background:#ff0\">aaaa</span></div>\
<div id=d><span id=t style=\"position:relative;top:15px;background:#f0f\">aaaa</span></div>\
<div id=e style=\"background:#0f0\"></div>\
<div id=f><span id=u style=\"position:relative;top:15px;background:#f80\">aaaa</span></div>\
<div id=g style=\"position:relative;background:#888\"></div>\
<div id=i style=\"position:relative;background:#00f\"></div>\
<div id=j style=\"margin-top:-10px\"><span id=v style=\"opacity:.5;background:#f00\">aaaa</span>\
</div>";

const POINTS: [(usize, usize); 4] = [(0, 5), (0, 64), (0, 104), (0, 131)];

fn open(page: &str) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

fn render(harness: &mut Harness<Page<()>>) -> Vec<u8> {
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

fn pixel(buffer: &[u8], (x, y): (usize, usize)) -> [u8; 4] {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let start = (y * width + x) * 4;
    let mut rgba = [0; 4];
    rgba.copy_from_slice(&buffer[start..start + 4]);
    rgba
}

fn hit_id(harness: &Harness<Page<()>>, (x, y): (usize, usize)) -> Option<String> {
    let doc = harness.base();
    let x = f32::from(u16::try_from(x).ok()?) + 0.25;
    let y = f32::from(u16::try_from(y).ok()?) + 0.25;
    let hit = doc.hit(x, y)?;
    let mut node = doc.get_node(hit.node_id)?;
    loop {
        if let Some(id) = node.element_data().and_then(|element| element.id.as_ref()) {
            return Some(id.to_string());
        }
        node = doc.get_node(node.parent?)?;
    }
}

#[test]
fn inline_layers_paint_in_tree_order_with_positioned_blocks_like_chrome() {
    let buffer = render(&mut open(LAYERS));
    let chrome: [[u8; 3]; 4] = [[255, 255, 0], [255, 0, 255], [136, 136, 136], [128, 0, 126]];
    for (&point, expected) in POINTS.iter().zip(chrome) {
        let [red, green, blue, _] = pixel(&buffer, point);
        let ours = [red, green, blue];
        let near = ours
            .iter()
            .zip(expected)
            .all(|(&ours, theirs)| ours.abs_diff(theirs) <= 2);
        assert!(near, "{point:?} is {ours:?}, Chrome {expected:?}");
    }
}

#[test]
fn inline_layers_are_hit_in_tree_order_with_positioned_blocks_like_chrome() {
    let harness = open(LAYERS);
    let ids: Vec<Option<String>> = POINTS
        .iter()
        .map(|&point| hit_id(&harness, point))
        .collect();
    let expected: Vec<Option<String>> = ["s", "t", "g", "v"]
        .iter()
        .map(|id| Some((*id).to_string()))
        .collect();
    assert_eq!(ids, expected);
}
