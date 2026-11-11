use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::{App, Page};
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;

pub(crate) const WIDTH: u32 = 120;
pub(crate) const HEIGHT: u32 = 80;

pub(crate) fn open<S: 'static>(app: App<S>) -> Harness<Page<S>> {
    let page = app
        .page(WIDTH, HEIGHT)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    harness
}

pub(crate) fn render<S: 'static>(harness: &mut Harness<Page<S>>) -> Vec<u8> {
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 1.0, WIDTH, HEIGHT, 0, 0),
        WIDTH,
        HEIGHT,
    )
}

pub(crate) fn pixel(buffer: &[u8], x: usize, y: usize) -> [u8; 4] {
    let width = usize::try_from(WIDTH).expect("test width fits in usize");
    let start = (y * width + x) * 4;
    let mut rgba = [0; 4];
    rgba.copy_from_slice(&buffer[start..start + 4]);
    rgba
}
