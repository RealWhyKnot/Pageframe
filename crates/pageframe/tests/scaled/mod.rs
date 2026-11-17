use crate::support::{HEIGHT, WIDTH};
use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use pageframe::Page;
use pageframe_paint::paint_scene;
use pageframe_test_harness::Harness;
use pageframe_traits::shell::Viewport;

pub(crate) fn render_at_scale<S: 'static>(harness: &mut Harness<Page<S>>, scale: u16) -> Vec<u8> {
    render_at_ratio(harness, (scale, 1), WIDTH)
}

pub(crate) fn render_at_ratio<S: 'static>(
    harness: &mut Harness<Page<S>>,
    (numerator, denominator): (u16, u16),
    css_width: u32,
) -> Vec<u8> {
    let device = |css: u32| css * u32::from(numerator) / u32::from(denominator);
    let (width, height) = (device(css_width), device(HEIGHT));
    let scale = f32::from(numerator) / f32::from(denominator);
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
