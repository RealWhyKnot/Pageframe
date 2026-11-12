use super::ElementCx;
use crate::color::{Color, ToColorColor as _};
use anyrender::PaintScene;
use kurbo::{Affine, Rect, Shape as _, Vec2};
use pageframe_dom::CopyStyleGetters;
use peniko::{Compose, Fill, Mix};

impl ElementCx<'_, '_> {
    pub(super) fn draw_outset_box_shadow(&self, scene: &mut impl PaintScene) {
        let box_shadow = &self.style.get_effects().box_shadow.0;
        let has_outset_shadow = box_shadow.iter().any(|s| !s.inset);
        if !has_outset_shadow {
            return;
        }

        let current_color = self.style.clone_color();
        let opacity = self.style.get_effects().opacity;
        let bg_color = self
            .style
            .get_background()
            .background_color
            .resolve_to_absolute(&current_color)
            .as_srgb_color();
        let bg_is_opaque = bg_color.components[3] >= 1.0;
        let needs_clip = opacity < 1.0 || !bg_is_opaque || self.covered_border_share().is_some();

        let max_shadow_rect = box_shadow.iter().fold(Rect::ZERO, |prev, shadow| {
            let x = shadow.base.horizontal.px() as f64 * self.scale;
            let y = shadow.base.vertical.px() as f64 * self.scale;
            let blur = shadow.base.blur.px() as f64 * self.scale;
            let spread = shadow.spread.px() as f64 * self.scale;
            let offset = spread + blur * 2.5;

            let rect = self.frame.border_box.inflate(offset, offset) + Vec2::new(x, y);

            prev.union(rect)
        });

        self.context.layer_manager.maybe_with_layer(
            scene,
            needs_clip,
            Mix::Normal.into(),
            1.0,
            self.transform,
            &self.frame.shadow_clip(max_shadow_rect),
            None,
            None,
            |scene| {
                for shadow in box_shadow.iter().filter(|s| !s.inset).rev() {
                    let shadow_color = shadow
                        .base
                        .color
                        .resolve_to_absolute(&current_color)
                        .as_srgb_color();

                    let alpha = shadow_color.components[3];
                    if alpha != 0.0 {
                        let transform = self.transform.then_translate(Vec2 {
                            x: shadow.base.horizontal.px() as f64 * self.scale,
                            y: shadow.base.vertical.px() as f64 * self.scale,
                        });

                        let spread = shadow.spread.px() as f64 * self.scale;
                        let blur = shadow.base.blur.px() as f64;

                        let grown = self.frame.outset_border_box(spread);
                        let rect = grown.border_box;
                        if rect.width() <= 0.0 || rect.height() <= 0.0 {
                            continue;
                        }
                        if blur == 0.0 {
                            scene.fill(
                                Fill::NonZero,
                                transform,
                                shadow_color,
                                None,
                                &grown.border_box_path(),
                            );
                            continue;
                        }

                        let radius = grown.border_radii.average();

                        // Fill the color
                        scene.draw_box_shadow(transform, rect, shadow_color, radius, blur);
                    }
                }
            },
        )
    }

    pub(super) fn draw_inset_box_shadow(&self, scene: &mut impl PaintScene) {
        let current_color = self.style.clone_color();
        let box_shadow = &self.style.get_effects().box_shadow.0;
        let has_inset_shadow = box_shadow.iter().any(|s| s.inset);
        if !has_inset_shadow {
            return;
        }

        let padding_box = self.frame.padding_box_path();

        for shadow in box_shadow.iter().filter(|s| s.inset) {
            let shadow_color = shadow
                .base
                .color
                .resolve_to_absolute(&current_color)
                .as_srgb_color();
            if shadow_color == Color::TRANSPARENT {
                return;
            }

            let offset = Vec2 {
                x: f64::from(shadow.base.horizontal.px()) * self.scale,
                y: f64::from(shadow.base.vertical.px()) * self.scale,
            };
            let transform = self.transform.then_translate(offset);
            let spread = f64::from(shadow.spread.px()) * self.scale;
            let blur = f64::from(shadow.base.blur.px()) * self.scale;
            let hole = self.frame.outset_padding_box(-spread);

            if blur == 0.0 {
                let reach = offset.x.abs().max(offset.y.abs()) + 1.0;
                let mut ring = self.frame.padding_box.inflate(reach, reach).to_path(0.1);
                if hole.border_box.width() > 0.0 && hole.border_box.height() > 0.0 {
                    ring.extend(Affine::translate(offset) * hole.border_box_path());
                }
                scene.push_layer(
                    Fill::NonZero,
                    Mix::Normal,
                    1.0,
                    self.transform,
                    &padding_box,
                    None,
                    None,
                );
                scene.fill(Fill::EvenOdd, self.transform, shadow_color, None, &ring);
                scene.pop_layer();
                continue;
            }

            scene.push_layer(
                Fill::NonZero,
                Mix::Normal,
                1.0,
                self.transform,
                &padding_box,
                None,
                None,
            );
            scene.fill(
                Fill::NonZero,
                self.transform,
                shadow_color,
                None,
                &padding_box,
            );

            scene.push_layer(
                Fill::NonZero,
                Compose::DestOut,
                1.0,
                self.transform,
                &padding_box,
                None,
                None,
            );
            scene.draw_box_shadow(
                transform,
                hole.border_box,
                Color::WHITE,
                hole.border_radii.average(),
                blur,
            );

            scene.pop_layer();
            scene.pop_layer();
        }
    }
}
