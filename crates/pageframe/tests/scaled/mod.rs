use crate::support::{HEIGHT, WIDTH};
use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_paint::paint_scene;
use blitz_test_harness::Harness;
use blitz_traits::shell::Viewport;
use pageframe::Page;

pub(crate) fn render_at_scale<S: 'static>(harness: &mut Harness<Page<S>>, scale: u16) -> Vec<u8> {
    let (width, height) = (WIDTH * u32::from(scale), HEIGHT * u32::from(scale));
    let scale = f32::from(scale);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        doc.set_viewport(Viewport::new(width, height, scale, color_scheme));
    }
    harness.pump();
    let mut doc = harness.base_mut();
    render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, f64::from(scale), width, height, 0, 0),
        width,
        height,
    )
}
