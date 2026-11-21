use super::*;

pub(crate) fn stroke_text<'a>(
    scene: &mut impl PaintScene,
    lines: impl Iterator<Item = Line<'a, TextBrush>>,
    doc: &BaseDocument,
    transform: Affine,
    options: TextPaintOptions<'_>,
    inline_root_id: NodeId,
    context: &mut DrawTextContext,
) {
    let TextPaintOptions {
        scale,
        brush_override,
        ellipsis,
        text: layout_text,
        group,
        flat_ancestors,
    } = options;
    let DrawTextContext {
        stack,
        path_scratch,
        deco_boxes,
        win_ascent_ratios,
        ink,
        upright,
        upright_x,
        sampled,
        sampled_paths,
    } = context;
    stack.clear();
    path_scratch.clear();
    deco_boxes.clear();
    let trim_root = doc
        .get_node(inline_root_id)
        .zip(layout_text)
        .filter(|_| !flat_ancestors);

    // Persistent stack mirroring the ancestor path (inline root -> current run's
    // node) as we walk the runs. The `text-decoration-*` properties are *not*
    // inherited; instead a decoration set on an ancestor is propagated to the
    // descendant text it wraps. Rather than re-resolving styles for the whole
    // ancestor chain on every run, we cache each node's resolved values here and,
    // for each run, only resolve styles for the nodes newly descended into (popping
    // as we ascend). `path_scratch` is a reusable buffer for the run's node path.
    for (line_index, line) in lines.enumerate() {
        let cut = ellipsis.and_then(|ellipsis| {
            let metrics = line.metrics();
            let origin = metrics.offset + metrics.inline_min_coord;
            let line_end = metrics.inline_max_coord.min(ellipsis.width);
            let overflows =
                origin < 0.0 || origin + metrics.advance - metrics.hanging_advance > line_end;
            let is_clamped = ellipsis.on_line == Some(line_index);
            ((ellipsis.on_overflow && overflows) || is_clamped)
                .then(|| line_cut(&line, ellipsis, is_clamped))
                .flatten()
        });

        // Decorations accumulated for this line, keyed by decorating box, so each box is
        // painted once (spanning all its runs) using its own font — matching Firefox, which
        // draws one decoration per box rather than one stepped segment per differently-sized
        // run. Clearing preserves the allocation for the next line and inline context.
        deco_boxes.clear();
        let mut holes: SmallVec<[(f64, f64); 2]> = SmallVec::new();
        if let Some((left, right)) =
            trim_root.and_then(|(root, text)| LineTrim::new(root, &line, text).hung_span())
        {
            holes.push((f64::from(left), f64::from(right)));
        }

        for (item_index, item) in line.items().enumerate() {
            if let PositionedLayoutItem::InlineBox(inline_box) = &item
                && inline_box.kind == parley::InlineBoxKind::InFlow
            {
                let left = f64::from(inline_box.x);
                holes.push((left, left + f64::from(inline_box.width)));
            }
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let run = glyph_run.run();
                let run_text = layout_text
                    .and_then(|text| text.get(run.text_range()))
                    .unwrap_or_default();
                let snap_origin = covers_its_start(doc, glyph_run.style().brush.id, run.is_rtl())
                    .then(|| transform.translation().x);
                push_spacer_holes(&glyph_run, run_text, snap_origin, &mut holes);
                if is_invisible(doc, glyph_run.style().brush.id) {
                    let left = f64::from(glyph_run.offset());
                    holes.push((left, left + f64::from(glyph_run.advance())));
                    continue;
                }
                let font = run.font();
                let font_size = run.font_size();
                let metrics = run.font_metrics();
                let style = glyph_run.style();
                let synthesis = run.synthesis();
                let skew = synthesis
                    .skew()
                    .map(|angle| Affine::skew(synthetic_skew(angle), 0.0));
                let glyph_xform = if run.is_upright() {
                    Some(UPRIGHT_GLYPH * skew.unwrap_or(Affine::IDENTITY))
                } else {
                    skew
                };

                let css_font_size = font_size as f64 / scale;

                // Reconcile the stack with this run's ancestor path. Build the path
                // (inline root -> run node), keep the shared prefix already on the stack,
                // pop the rest, and resolve styles only for the newly-descended nodes.
                path_scratch.clear();
                let mut walk_id = Some(style.brush.id);
                while let Some(node_id) = walk_id {
                    path_scratch.push(node_id);
                    if node_id == inline_root_id {
                        break;
                    }
                    walk_id = doc.get_node(node_id).and_then(|node| node.parent);
                }
                path_scratch.reverse();

                let shared = stack
                    .iter()
                    .zip(path_scratch.iter())
                    .take_while(|(entry, node_id)| entry.node_id == **node_id)
                    .count();
                stack.truncate(shared);
                for &node_id in &path_scratch[shared..] {
                    let mut entry = resolve_decoration_entry(doc, node_id);
                    if node_id == inline_root_id || flat_ancestors {
                        entry.shift = Vec2::ZERO;
                        entry.group = None;
                    } else if let Some(outer) = stack.last() {
                        entry.shift = outer.shift + entry.shift * scale;
                        entry.group = entry.group.or(outer.group);
                    } else {
                        entry.shift *= scale;
                    }
                    stack.push(entry);
                }
                if stack.last().and_then(|entry| entry.group) != group {
                    let start = f64::from(glyph_run.offset());
                    holes.push((start, start + f64::from(glyph_run.advance())));
                    continue;
                }
                let transform = transform
                    * Affine::translate(stack.last().map_or(Vec2::ZERO, |entry| entry.shift));

                let fill_color = brush_override
                    .unwrap_or_else(|| stack.last().map(|e| e.fill_color).unwrap_or(Color::BLACK));

                let embolden = if FONT_EMBOLDEN_ENABLED {
                    font_embolden(font_size as f64, fill_color, transform)
                } else if cfg!(target_os = "windows") && synthesis.embolden() {
                    synthetic_bold(font_size as f64)
                } else {
                    kurbo::Vec2::default()
                };

                let normalized_coords: &[i16] = bytemuck::cast_slice(run.normalized_coords());
                upright.place_run(upright_x, transform, &glyph_run);
                let placed: &[f32] = upright_x;
                let glyphs = || {
                    glyph_run
                        .positioned_glyphs()
                        .enumerate()
                        .filter(move |(_, glyph)| cut.is_none_or(|cut| cut.keeps(glyph)))
                        .map(|(index, glyph)| anyrender::Glyph {
                            id: glyph.id as _,
                            x: placed.get(index).copied().unwrap_or(glyph.x),
                            y: glyph.y,
                        })
                        .chain(
                            cut.filter(|cut| cut.run_index == item_index)
                                .into_iter()
                                .flat_map(LineCut::glyphs),
                        )
                };

                #[cfg(target_os = "windows")]
                let disclosure = run.text_range().start == 0
                    && font.data.as_ref() == pageframe_dom::BULLET_FONT
                    && doc
                        .get_node(inline_root_id)
                        .and_then(|node| node.element_data())
                        .and_then(|element| element.list_item_data.as_deref())
                        .is_some_and(|list| {
                            matches!(list.marker, pageframe_dom::node::Marker::Char(_))
                        });

                let face = RunFace {
                    #[cfg(target_os = "windows")]
                    installed: (!disclosure).then_some(doc),
                    font,
                    font_size,
                    coords: normalized_coords,
                    embolden,
                    glyph_xform,
                };

                // Shadows are painted beneath the text, with the first shadow on top.
                // Text painted with a brush override (e.g. as a mask) has no shadow.
                let shadows = match (brush_override, stack.last()) {
                    (None, Some(entry)) if font_size > 0.0 => entry.shadows.as_slice(),
                    _ => &[],
                };
                for shadow in shadows
                    .iter()
                    .rev()
                    .filter(|shadow| shadow.color.components[3] > 0.0)
                {
                    let shadow_transform = transform * Affine::translate(shadow.offset * scale);
                    // A shadow's blur radius is twice the standard deviation of its Gaussian.
                    let std_deviation = shadow.blur * scale / 2.0;
                    let run_x0 = glyph_run.offset() as f64;
                    let baseline = glyph_run.baseline() as f64;
                    let ink_margin = font_size as f64;
                    let bounds = Rect::new(
                        run_x0 - ink_margin,
                        baseline - metrics.ascent.abs() as f64 - ink_margin,
                        run_x0 + glyph_run.advance() as f64 + ink_margin,
                        baseline + metrics.descent.abs() as f64 + ink_margin,
                    );
                    #[cfg(target_os = "windows")]
                    let native_shadow =
                        std_deviation > 0.0 && windows::renderer(scene, &face, shadow_transform);
                    let blurred = (std_deviation > 0.0)
                        .then(|| {
                            blurred_shadow_image(
                                bounds,
                                std_deviation,
                                shadow.color,
                                |hasher, origin| {
                                    #[cfg(target_os = "windows")]
                                    native_shadow.hash(hasher);
                                    font.data.id().hash(hasher);
                                    font.index.hash(hasher);
                                    font_size.to_bits().hash(hasher);
                                    normalized_coords.hash(hasher);
                                    [embolden.x, embolden.y].map(f64::to_bits).hash(hasher);
                                    glyph_xform
                                        .map(|xform| xform.as_coeffs().map(f64::to_bits))
                                        .hash(hasher);
                                    for glyph in glyphs() {
                                        glyph.id.hash(hasher);
                                        (f64::from(glyph.x) - origin.x).to_bits().hash(hasher);
                                        (f64::from(glyph.y) - origin.y).to_bits().hash(hasher);
                                    }
                                },
                                |scene, offset| {
                                    #[cfg(target_os = "windows")]
                                    if native_shadow
                                        && windows::draw(
                                            scene,
                                            &face,
                                            offset,
                                            Color::BLACK,
                                            glyphs(),
                                        )
                                    {
                                        return;
                                    }
                                    scene.draw_glyphs(
                                        font,
                                        font_size,
                                        false,
                                        normalized_coords,
                                        embolden,
                                        Fill::NonZero,
                                        &anyrender::Paint::from(Color::BLACK),
                                        1.0,
                                        offset,
                                        glyph_xform,
                                        glyphs(),
                                    );
                                },
                            )
                        })
                        .flatten();
                    match blurred {
                        Some((image, origin)) => {
                            scene.draw_image(
                                image.as_ref(),
                                shadow_transform * Affine::translate(origin),
                            );
                        }
                        None => fill_glyphs(
                            scene,
                            sampled,
                            sampled_paths,
                            &face,
                            (shadow_transform, shadow.color),
                            glyphs,
                        ),
                    }
                }

                // Zero-size text is invisible, but hinting can give its glyphs ink.
                if font_size > 0.0 {
                    fill_glyphs(
                        scene,
                        sampled,
                        sampled_paths,
                        &face,
                        (transform, fill_color),
                        glyphs,
                    );
                }

                // Accumulate this run's contribution to each decorating box on its ancestor
                // path. The decoration is drawn once per box after the whole line has been
                // walked (see `flush_line_decorations`), so mixed font sizes within a box
                // produce a single straight line rather than one stepped segment per run.
                let geometry = DecorationRunGeometry {
                    baseline: glyph_run.baseline(),
                    ascent: metrics.ascent,
                    descent: metrics.descent,
                    underline_offset: metrics.underline_offset,
                    underline_size: metrics.underline_size,
                    strikethrough_size: metrics.strikethrough_size,
                    font: font.clone(),
                    font_size,
                    css_font_size,
                };
                let run_node_id = style.brush.id;
                let is_edge_run = layout_text
                    .and_then(|text| text.get(glyph_run.run().text_range()))
                    .is_some_and(|text| {
                        !text.is_empty()
                            && text.chars().all(|ch| {
                                pageframe_dom::util::INLINE_EDGE_START.contains(ch)
                                    || pageframe_dom::util::INLINE_EDGE_END.contains(ch)
                            })
                    });
                let mut run_x0 = glyph_run.offset() as f64;
                let mut run_x1 = run_x0 + glyph_run.advance() as f64;
                if let Some(cut) = cut {
                    run_x0 = run_x0.max(cut.start as f64).min(cut.end as f64);
                    run_x1 = run_x1.max(cut.start as f64).min(cut.end as f64);
                }

                for entry in stack.iter() {
                    if entry.decoration.is_none() || (is_edge_run && entry.node_id == run_node_id) {
                        continue;
                    }
                    let idx = match deco_boxes.iter().position(|d| d.node_id == entry.node_id) {
                        Some(idx) => idx,
                        None => {
                            deco_boxes.push(LineDecoration {
                                node_id: entry.node_id,
                                deco: entry.decoration.clone().unwrap(),
                                min_x: f64::INFINITY,
                                max_x: f64::NEG_INFINITY,
                                own: None,
                                first: None,
                                runs: SmallVec::new(),
                                shift: entry.shift,
                            });
                            deco_boxes.len() - 1
                        }
                    };
                    let acc = &mut deco_boxes[idx];
                    acc.min_x = acc.min_x.min(run_x0);
                    acc.max_x = acc.max_x.max(run_x1);
                    acc.runs.push(item_index);
                    if acc.first.is_none() {
                        acc.first = Some(geometry.clone());
                    }
                    // A run whose innermost node is the box itself is the box's own text, so
                    // its font is the one Firefox positions and sizes the decoration with.
                    if entry.node_id == run_node_id {
                        acc.own = Some(geometry.clone());
                    }
                }
            }
        }

        holes.sort_by(|a, b| a.0.total_cmp(&b.0));
        flush_line_decorations(
            scene,
            transform,
            options,
            deco_boxes,
            win_ascent_ratios,
            inline_root_id,
            &mut LineInk {
                line: &line,
                cut,
                cache: ink,
                holes,
            },
        );
    }
}
fn synthetic_skew(angle: f32) -> f64 {
    if cfg!(target_os = "windows") {
        0.25
    } else {
        f64::from(angle).to_radians().tan()
    }
}

fn synthetic_bold(font_size: f64) -> kurbo::Vec2 {
    let ratio = 1.0 / 24.0 - (1.0 / 24.0 - 1.0 / 32.0) * ((font_size - 9.0) / 27.0).clamp(0.0, 1.0);
    let side = font_size * ratio / 2.0;
    if side.is_finite() && side > 0.0 {
        kurbo::Vec2::new(side, side)
    } else {
        kurbo::Vec2::default()
    }
}

/// Approximates macOS font smoothing (stem dilation). CoreGraphics dilates each side by an amount
/// in device pixels that grows with the device ppem up to 0.3px (fitted to Safari's ink at 9-72px
/// and 1x-3x scale), and keeps dilating at large sizes, so there is no ppem cutoff. glifo applies
/// the embolden in user units, so the device amounts are divided by the transform's scale.
fn font_embolden(font_size: f64, color: Color, transform: Affine) -> kurbo::Vec2 {
    const MAX_DEVICE_EMBOLDEN: f64 = 0.3;
    let [a, b, c, d, _, _] = transform.as_coeffs();
    let (scale_x, scale_y) = (a.hypot(b), c.hypot(d));
    let device_ppem = font_size * (a * d - b * c).abs().sqrt();
    if scale_x == 0.0 || scale_y == 0.0 || !device_ppem.is_finite() {
        return kurbo::Vec2::default();
    }
    let fs = device_ppem * embolden_scale(color);
    kurbo::Vec2::new(
        (0.0121 * fs).min(MAX_DEVICE_EMBOLDEN) / scale_x,
        (0.015125 * fs).min(MAX_DEVICE_EMBOLDEN) / scale_y,
    )
}

/// Scales Pathfinder's stem-darkening factors by the text's relative luminance. CoreText
/// dilates light text more than dark text; the points are fitted to Safari's ink for text
/// colours from black to white on black and white backgrounds.
fn embolden_scale(color: Color) -> f64 {
    const POINTS: [(f64, f64); 4] = [(0.1, 0.5), (0.125, 0.72), (0.35, 0.72), (1.0, 1.3)];
    let lum = color.discard_alpha().relative_luminance() as f64;
    let mut prev = POINTS[0];
    if lum <= prev.0 {
        return prev.1;
    }
    for point in &POINTS[1..] {
        if lum <= point.0 {
            return prev.1 + (point.1 - prev.1) * (lum - prev.0) / (point.0 - prev.0);
        }
        prev = *point;
    }
    prev.1
}
