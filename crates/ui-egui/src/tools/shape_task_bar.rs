//! The shape layer's contextual task bar (#16755), in the framework's shared `TaskBar`, left-aligned under the
//! shape's bounds as in the reference application. Left to right:
//!
//! 1. the grip (the bar's);
//! 2. `Fill` and `Stroke` with their paint chips: the shape options bar's chips (`shape_paint_ui`), editing the layer
//!    with `shape.edit`;
//! 3. the stroke width and the stroke type button (the framework's `StrokeCells`, which the options bar shows too);
//! 4. Corner radius: a popover with the radius of all corners, then Top Left, Top Right, Bottom Left and Bottom Right;
//! 5. Duplicate layer (`layer.duplicate`) and More properties (`window.panel.properties`, shown);
//! 6. the ⋯ menu (the bar's: Hide bar, Pin bar position, Reset bar position).
//!
//! Shown while exactly one layer is selected and it is a shape layer holding one live rectangle, Window › Contextual
//! Task Bar is ticked, and no Crop tool, transform, canvas gesture or dialog is running. Other shapes (an ellipse, a
//! path, several shapes in one layer) were not measured in the reference, so they keep no bar.

use craft::behavior::kurbo::Point;
use craft::behavior::overlay::{Align, Side};
use craft::behavior::{EditEvent, IconId};
use craft::color_ui::ui::{Paint, PaintChip, PaintChipOutput};
use craft::draw::ui::shape::{ShapeChange, ShapePaint, ShapeWords, StrokeCells};
use craft::ui::buttons::IconButton;
use craft::ui::canvas_controls::{TaskBar, TaskBarHang};
use craft::ui::contract::{Bind, LabelScheme, PopupKind, Props as _, Show as _, ShowContainer as _, clicked};
use craft::ui::form::{Field, FieldGroup};
use craft::ui::layout_components::Row;
use craft::ui::popover::Popover;
use craft::ui::runtime::env::{Env, Label, UnitKind};
use craft::ui::text::{NumberField, RangeAlert};
use photocraft_doc::vector::LiveShape;
use photocraft_doc::{LayerContent, LayerId, ShapeLayer};
use photocraft_engine::presets::PresetsExt as _;
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::shape_paint_ui::{ChipLibrary, decorate_chip, fill_param, solid_rgb, srgba};
use crate::shape_stroke_ui::StrokeOptions;
use crate::state::{TaskBarContext, Tool};

/// The id the screen eyedropper's sample is kept under for this bar's chips.
const PICKER: &str = "shape-task-bar";

/// Whether the Corner radius popover is open, kept by the framework per viewer.
#[derive(Clone, Copy, Default)]
struct CornersOpen(bool);

/// Whether the stroke type popover is open, kept by the framework per viewer.
#[derive(Clone, Copy, Default)]
struct StrokeOpen(bool);

/// Where the bar showed, for the layer it showed for.
#[derive(Clone, Copy)]
struct ShownRect {
    layer: u64,
    rect: [f32; 4],
}

fn shown_id() -> egui::Id {
    egui::Id::unique("shape-layer-task-bar-rect")
}

/// The shape layer the bar is for and its bounds `[x0, y0, x1, y1]` in document pixels.
struct Target {
    doc: u64,
    layer: LayerId,
    shape: ShapeLayer,
    bounds: [f64; 4],
    locked: bool,
}

fn target(app: &PhotocraftApp) -> Option<Target> {
    if !app.ui.panels.context_task_bar
        || app.ui.tool == Tool::Crop
        || app.ui.transform.is_some()
        || app.transform_preview.is_some()
        || app.dialog_open()
        || crate::canvas::tools::in_gesture(app)
    {
        return None;
    }
    let st = app.session.active()?;
    let [id] = st.selected_layers() else { return None };
    let layer = st.doc.layer(*id)?;
    if !layer.visible {
        return None;
    }
    let LayerContent::Shape(shape) = &layer.content else { return None };
    // Only a live rectangle was measured in the reference; other shapes keep no bar.
    if !matches!(shape.live, Some(LiveShape::Rect { .. })) {
        return None;
    }
    let (x0, y0, x1, y1) = shape.path.control_bounds()?;
    let bounds = [x0, y0, x1, y1];
    if !bounds.iter().all(|v| v.is_finite()) || x1 < x0 || y1 < y0 {
        return None;
    }
    let locks = st.doc.effective_locks(*id);
    Some(Target { doc: st.doc.id.0, layer: *id, shape: shape.clone(), bounds, locked: locks.all || locks.pixels })
}

/// Whether the bar can show now.
pub fn eligible(app: &PhotocraftApp) -> bool {
    target(app).is_some()
}

/// The bar's rect on screen as last shown, while it can still show for the same layer.
pub(crate) fn shown_rect(app: &PhotocraftApp, ctx: &egui::Context) -> Option<[f32; 4]> {
    let layer = target(app)?.layer.0;
    let shown = ctx.data(|data| data.get_temp::<ShownRect>(shown_id()))?;
    (shown.layer == layer).then_some(shown.rect)
}

/// One change the bar's members asked for in a run of its body.
enum Change {
    Fill(EditEvent<Paint>),
    Stroke(EditEvent<Paint>),
    Cells(EditEvent<ShapeChange>),
    /// The radius of every corner (`None`) or of one corner, in the radii order top left, top right, bottom right,
    /// bottom left.
    Radius(Option<usize>, EditEvent<Option<f64>>),
    Duplicate,
    Properties,
}

/// What one run of the body reports.
#[derive(Default)]
struct Run {
    changes: Vec<Change>,
    screen_pick: Option<(ShapePaint, craft::color_ui::ui::ScreenPickGesture)>,
    pick_color: Option<ShapePaint>,
    out_of_range: Option<RangeAlert>,
}

impl Run {
    fn chip(&mut self, which: ShapePaint, out: PaintChipOutput) {
        if let Some(gesture) = out.screen_pick {
            self.screen_pick = Some((which, gesture));
        }
        if out.pick_color.is_some() {
            self.pick_color = Some(which);
        }
        for event in out.edit.events {
            self.changes.push(match which {
                ShapePaint::Stroke => Change::Stroke(event),
                _ => Change::Fill(event),
            });
        }
    }
}

/// Shows the bar under the selected shape layer, inside `area` (the canvas body's rect).
pub fn show(app: &mut PhotocraftApp, ui: &mut egui::Ui, area: egui::Rect) {
    let ctx = ui.ctx().clone();
    let Some(t) = target(app) else {
        ctx.data_mut(|data| data.remove::<ShownRect>(shown_id()));
        return;
    };
    app.ui.enter_task_bar_context(TaskBarContext::Shape { doc: t.doc, layer: t.layer.0 });
    let [x0, y0, x1, y1] = t.bounds;
    let anchor = [Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)];
    let appearance = crate::shape_paint_ui::appearance_of(&t.shape, t.locked);
    let radii = match t.shape.live {
        Some(LiveShape::Rect { radii, .. }) => radii,
        _ => [0.0; 4],
    };
    let ppi = app.session.active().map_or(72.0, |st| f64::from(st.doc.resolution_dpi));
    let (fg, bg) = (app.session.tools.foreground, app.session.tools.background);
    let sampled = [ShapePaint::Fill, ShapePaint::Stroke].into_iter().find_map(|p| crate::screen_picker::take(app, (PICKER, p)).map(|c| (p, c)));
    let recent_colors: Vec<craft::theme::Srgba> =
        crate::paint_picker::recent_colors(&app.session).into_iter().map(|c| srgba([c[0], c[1], c[2], 1.0])).collect();
    let groups = crate::shape_gradient_ui::ShapeGradients::groups_of(&app.session.presets().gradients, fg, bg);
    let gradients = crate::shape_gradient_ui::ShapeGradients { groups, current: &app.session.presets().gradient, recent: &mut app.ui.recent_gradients, fg, bg };
    let current = [t.shape.fill.clone(), t.shape.stroke.as_ref().map(|s| s.paint.clone())];
    let mut library =
        ChipLibrary::new(&recent_colors, &app.session.presets().swatches, &mut app.ui.paint_folders, gradients, &mut app.ui.shape_gradient_state, current);
    let mut place = app.ui.panels.task_bar;
    let shown = TaskBar::new(tl!("Contextual Task Bar"), &anchor, Bind::bound(&mut place))
        .side(Side::Below, Align::Start)
        .hang_gap(TaskBarHang::ClearOfHandles)
        .show(ui, area, |ui, cx| {
            let mut run = Run::default();
            let words = Env::of(ui).words::<ShapeWords>();
            Row::new().enabled(!appearance.locked).show(ui, |ui, row| {
                for (which, name, paint, fallback) in
                    [(ShapePaint::Fill, &words.fill, &appearance.fill, fg), (ShapePaint::Stroke, &words.stroke, &appearance.stroke, bg)]
                {
                    let (chip_name, tip) = match which {
                        ShapePaint::Stroke => (&words.stroke_color, &words.set_stroke_type),
                        _ => (&words.fill_color, &words.set_fill_type),
                    };
                    let out = Field::new(name.clone()).id_salt(("shape-task-bar-paint", name.as_str())).show(ui, |ui, _| {
                        let mut chip = PaintChip::new(chip_name.clone(), Bind::given(paint.clone()), srgba(fallback)).tooltip(tip.clone());
                        if let Some((_, color)) = sampled.filter(|(on, _)| *on == which) {
                            chip = chip.sampled(color);
                        }
                        decorate_chip(chip, which, &mut library, ui)
                    });
                    if let Some(out) = out.inner {
                        run.chip(which, out);
                    }
                }
                let stroke_open = &mut row.memory.shown::<StrokeOpen>(row.id).0;
                let cells = StrokeCells::new(&appearance, ppi).stroke_open(Bind::bound(stroke_open)).show(ui);
                run.changes.extend(cells.events.into_iter().map(Change::Cells));
                run.out_of_range = cells.out_of_range.or(run.out_of_range.take());
                let corners_open = &mut row.memory.shown::<CornersOpen>(row.id).0;
                corner_radius(ui, &words, radii, corners_open, &mut run);
            });
            if IconButton::new(IconId::new("copy")).label(Label::from(tl!("Duplicate layer").to_owned())).show(ui).clicked() {
                run.changes.push(Change::Duplicate);
            }
            if IconButton::new(IconId::new("adjustments")).label(Label::from(tl!("More properties").to_owned())).show(ui).clicked() {
                run.changes.push(Change::Properties);
            }
            cx.more_menu(ui);
            run
        });
    let gradients = [library.gradient(ShapePaint::Fill), library.gradient(ShapePaint::Stroke)];
    app.ui.panels.task_bar = place;
    if shown.output.hide {
        app.ui.panels.context_task_bar = false;
    }
    let r = shown.output.rect;
    ctx.data_mut(|data| data.insert_temp(shown_id(), ShownRect { layer: t.layer.0, rect: [r.min.x, r.min.y, r.max.x, r.max.y] }));
    let Some(run) = shown.inner else { return };
    apply(app, &ctx, &t, radii, &gradients, run.changes);
    if let Some(which) = run.pick_color {
        // The Color Picker starts from the layer's own paint.
        let which = match which {
            ShapePaint::Stroke => crate::shape_fill_ui::Paint::Stroke,
            _ => crate::shape_fill_ui::Paint::Fill,
        };
        let _ = crate::shape_fill_ui::open_paint(app, which, Some(t.layer), false, fg);
    }
    if let Some((which, gesture)) = run.screen_pick {
        crate::screen_picker::hold(app, &ctx, (PICKER, which), gesture);
    }
    if let Some(alert) = run.out_of_range {
        crate::option_cells::range_alert(app, ui, &alert);
    }
}

/// The Corner radius button and its popover: the radius of all corners, then one field per corner.
fn corner_radius(ui: &mut egui::Ui, words: &ShapeWords, radii: [f64; 4], open: &mut bool, run: &mut Run) {
    let button =
        IconButton::new(IconId::new("stroke_join_round")).label(words.corner_radius.clone()).popup(PopupKind::Dialog).expanded(*open).show(ui).response;
    if clicked(&button) {
        *open = !*open;
    }
    let all = radii.iter().all(|r| (r - radii[0]).abs() < f64::EPSILON).then_some(radii[0]);
    // The reference's order: Top Left, Top Right, Bottom Left, Bottom Right; the radii order is TL, TR, BR, BL.
    let corners = [(&words.top_left, 0), (&words.top_right, 1), (&words.bottom_left, 3), (&words.bottom_right, 2)];
    let _ = Popover::new(words.corner_radius.clone(), Bind::bound(open)).opened_by(&button).show(ui, |ui, _| {
        FieldGroup::new(words.corner_radius.clone()).labels(LabelScheme::Inspector).show(ui, |ui, _| {
            let mut field = |ui: &mut egui::Ui, caption: &Label, value: Option<f64>, corner: Option<usize>| {
                let events = Field::new(caption.clone())
                    .id_salt(("shape-task-bar-radius", corner))
                    .show(ui, |ui, _| NumberField::new(caption.clone(), Bind::given(value)).unit_kind(UnitKind::Pixels).show(ui).edit.events)
                    .inner
                    .unwrap_or_default();
                run.changes.extend(events.into_iter().map(|event| Change::Radius(corner, event)));
            };
            field(ui, &words.corner_radius, all, None);
            for (caption, index) in corners {
                field(ui, caption, radii.get(index).copied(), Some(index));
            }
        });
    });
}

/// Runs each change the bar asked for on the target layer, one history step per change.
fn apply(app: &mut PhotocraftApp, ctx: &egui::Context, t: &Target, radii: [f64; 4], gradients: &[photocraft_doc::Fill; 2], changes: Vec<Change>) {
    let mut recent = None;
    let mut remembered = Vec::new();
    for change in changes {
        let (mut patch, key) = match &change {
            Change::Duplicate => {
                // The engine command, not the menu item: Layer › Duplicate Layer… asks for a name first, the bar does not.
                let result = app.run("layer.duplicate", json!({})).map(|_| ());
                report(app, result);
                continue;
            }
            Change::Properties => {
                let result = crate::menus::invoke(app, ctx, "window.panel.properties", json!({"show": true})).map(|_| ());
                report(app, result);
                continue;
            }
            Change::Fill(event) => {
                let Some(paint) = event.value() else { continue };
                if committed(event) {
                    recent = solid_rgb(paint).or(recent);
                    remembered.extend(picked_gradient(paint, &gradients[0]));
                }
                (json!({"fill": fill_param(paint, &gradients[0])}), coalesce(t, "fill", event))
            }
            Change::Stroke(event) => {
                let Some(paint) = event.value() else { continue };
                if committed(event) {
                    recent = solid_rgb(paint).or(recent);
                    remembered.extend(picked_gradient(paint, &gradients[1]));
                }
                let color = fill_param(paint, &gradients[1]);
                let width = t.shape.stroke.as_ref().map_or(0.0, |s| s.width);
                let stroke = if color.is_null() { Value::Null } else { json!({"color": color, "width": crate::vector_ui::enabled_stroke_width(width)}) };
                (json!({"stroke": stroke}), coalesce(t, "stroke", event))
            }
            Change::Cells(event) => {
                let Some(patch) = event.value().and_then(|c| cell_patch(c, &t.shape)) else { continue };
                (patch, coalesce(t, "stroke-cells", event))
            }
            Change::Radius(corner, event) => {
                let Some(r) = event.value().copied().flatten().filter(|r| r.is_finite()).map(|r| r.max(0.0)) else { continue };
                let next = match corner {
                    None => [r; 4],
                    Some(i) => {
                        let mut next = radii;
                        let Some(slot) = next.get_mut(*i) else { continue };
                        *slot = r;
                        next
                    }
                };
                let field = match corner {
                    None => "radius-all".to_owned(),
                    Some(i) => format!("radius-{i}"),
                };
                (json!({"radii": next}), coalesce(t, &field, event))
            }
        };
        patch["layer"] = json!(t.layer.0);
        if let Some(key) = key {
            patch["coalesce"] = json!(key);
        }
        if let Err(e) = app.run("shape.edit", patch) {
            app.ui.status = e;
            app.ui.status_error = true;
        }
    }
    if let Some(rgb) = recent {
        crate::paint_picker::remember(app, rgb);
    }
    for g in &remembered {
        crate::shape_gradient_ui::remember_recent(&mut app.ui.recent_gradients, g);
    }
}

/// Whether `event` ends an edit (not a step of a running gesture or its cancel).
fn committed<T>(event: &EditEvent<T>) -> bool {
    !matches!(event, EditEvent::Update { .. } | EditEvent::Cancel { .. })
}

/// The history key that folds a gesture's steps on `field` of the target layer into one step.
fn coalesce<T>(t: &Target, field: &str, event: &EditEvent<T>) -> Option<String> {
    crate::history_keys::coalesce(&format!("shape-task-bar-{}-{}-{field}", t.doc, t.layer.0), event)
}

/// The gradient a chip's `paint` stands for over `base`; `None` for a paint that is no gradient.
fn picked_gradient(paint: &Paint, base: &photocraft_doc::Fill) -> Option<photocraft_doc::Fill> {
    match paint {
        Paint::Gradient(preview) => Some(crate::shape_paint_ui::gradient_fill(preview, base)),
        _ => None,
    }
}

/// The `shape.edit` patch of a stroke width or stroke type change on `shape`; `None` for no edit.
fn cell_patch(change: &ShapeChange, shape: &ShapeLayer) -> Option<Value> {
    match change {
        // A step that clamps at 0 on a shape with no stroke is no change.
        ShapeChange::StrokeWidth(w) if shape.stroke.is_none() && *w <= 0.0 => None,
        ShapeChange::StrokeWidth(w) => Some(json!({"stroke": {"width": w}})),
        ShapeChange::StrokeStyle { style, width, .. } => {
            let mut stroke = StrokeOptions::from_style(style).params();
            if let Some(w) = width {
                stroke["width"] = json!(w);
            }
            Some(json!({"stroke": stroke}))
        }
        _ => None,
    }
}

fn report(app: &mut PhotocraftApp, result: Result<(), String>) {
    if let Err(e) = result {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

#[cfg(test)]
#[path = "../task_bar_shape_layer_tests.rs"]
mod task_bar_shape_layer_tests;
