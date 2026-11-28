//! The shape layer's contextual task bar (#16755) through the real canvas, its widgets and the existing commands.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use photocraft_doc::vector::LiveShape;
use photocraft_doc::{LayerContent, LayerId};
use serde_json::json;

use crate::PhotocraftApp;
use crate::app::framework::BuildApp as _;
use crate::state::Tool;

type AppHarness = Harness<'static, Box<PhotocraftApp>>;

/// A 400 x 300 document with a red 120 x 80 rectangle shape layer "Shape" at 100,100, selected, with `tool`.
fn harness(tool: Tool) -> (AppHarness, LayerId) {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    let id = app.run("shape.create", json!({"kind": "rect", "rect": [100, 100, 120, 80], "fill": "#ff0000"})).unwrap()["layer"].as_u64().unwrap();
    app.run("layer.renameLayer", json!({"layer": id, "name": "Shape"})).unwrap();
    app.sync_views();
    app.ui.extras.rulers = false;
    app.ui.tool = tool;
    let mut h = Harness::builder().with_context(app.ctx().clone()).with_size(egui::vec2(1000.0, 700.0)).build_app(
        |ui, app: &mut PhotocraftApp| {
            egui::CentralPanel::default().show(ui, |ui| crate::canvas::testing::document_area(app, ui));
        },
        app,
    );
    h.run_steps(4);
    let v = &mut h.state_mut().ui.views[0];
    v.zoom = 1.0;
    v.center = [200.0, 150.0];
    v.clear_pending();
    h.run_steps(4);
    (h, LayerId(id))
}

fn bar(h: &AppHarness) -> Option<[f32; 4]> {
    super::shown_rect(h.state(), &h.ctx)
}

/// `ui.inspect`'s `taskBar`, through the control channel.
fn inspect_task_bar(h: &mut AppHarness) -> serde_json::Value {
    let ctx = h.ctx.clone();
    let (request, _reply) = crate::control::ControlRequest::new("ui.inspect", json!({}));
    let crate::control::Reply::Done(reply) = crate::control::handle(h.state_mut(), &ctx, &request) else { panic!("ui.inspect replies at once") };
    reply["result"]["taskBar"].clone()
}

fn history(h: &AppHarness) -> usize {
    h.state().session.active().unwrap().history.past_len()
}

fn radii(h: &AppHarness, id: LayerId) -> [f64; 4] {
    match &h.state().session.active().unwrap().doc.layer(id).unwrap().content {
        LayerContent::Shape(sh) => match sh.live {
            Some(LiveShape::Rect { radii, .. }) => radii,
            ref other => panic!("not a live rectangle: {other:?}"),
        },
        _ => panic!("not a shape layer"),
    }
}

/// The left edge of the widget named `label` on screen.
fn left(h: &AppHarness, label: &str) -> f32 {
    h.query_all_by_label(label).next().unwrap_or_else(|| panic!("no {label}")).rect().min.x
}

#[test]
fn bar_shows_under_the_shape_left_aligned_with_its_bounds() {
    let (mut h, _) = harness(Tool::Move);
    let rect = bar(&h).expect("a selected rectangle shape layer shows the bar");
    let inspected = inspect_task_bar(&mut h);
    assert_eq!((inspected["visible"].as_bool(), inspected["rect"].clone()), (Some(true), json!(rect)), "ui.inspect reports the bar");
    let view = crate::canvas::testing::view(h.state()).unwrap();
    let (top_left, bottom_left) =
        (view.to_screen(craft::behavior::kurbo::Point::new(100.0, 100.0)), view.to_screen(craft::behavior::kurbo::Point::new(100.0, 180.0)));
    assert!(rect[1] > bottom_left.y, "below the shape: bar top {} vs shape bottom {}", rect[1], bottom_left.y);
    assert!(rect[1] - bottom_left.y < 40.0, "close under the shape: {}", rect[1] - bottom_left.y);
    assert!((rect[0] - top_left.x).abs() < 1.0, "left-aligned with the shape: bar left {} vs shape left {}", rect[0], top_left.x);
}

#[test]
fn bar_controls_come_in_the_reference_order_with_their_names() {
    let (h, _) = harness(Tool::Move);
    let order = ["Fill", "Stroke", "Stroke width", "Stroke style", "Corner radius", "Duplicate layer", "More properties", "More options"];
    let xs: Vec<f32> = order.iter().map(|label| left(&h, label)).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "left to right {order:?}: {xs:?}");
    // The visible labels in front of the chips.
    assert!(h.query_by_label("Fill").is_some() && h.query_by_label("Stroke").is_some());
}

#[test]
fn no_bar_for_crop_transform_several_layers_other_layers_or_unmeasured_shapes() {
    let (mut h, _) = harness(Tool::Crop);
    h.run_steps(2);
    assert!(bar(&h).is_none() && !super::eligible(h.state()), "the Crop tool has its own bar");

    let (mut h, id) = harness(Tool::Move);
    let other = h.state_mut().run("shape.create", json!({"kind": "rect", "rect": [10, 10, 20, 20], "fill": "#00ff00"})).unwrap()["layer"].as_u64().unwrap();
    h.state_mut().run("layer.select", json!({"layer": id.0})).unwrap();
    h.state_mut().run("layer.select", json!({"layer": other, "mode": "add"})).unwrap();
    h.run_steps(3);
    assert!(!super::eligible(h.state()), "two layers selected: no bar");

    let (mut h, _) = harness(Tool::Move);
    h.state_mut().run("shape.create", json!({"kind": "ellipse", "rect": [10, 10, 60, 40], "fill": "#00ff00"})).unwrap();
    h.run_steps(3);
    assert!(!super::eligible(h.state()), "an ellipse was not measured: no bar");

    let (mut h, _) = harness(Tool::Move);
    h.state_mut().run("layer.new.layer", json!({})).unwrap();
    h.run_steps(3);
    assert!(!super::eligible(h.state()), "a pixel layer is not this bar's");

    let (mut h, _) = harness(Tool::Move);
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "edit.freeTransform", json!({})).unwrap();
    h.run_steps(4);
    assert!(!super::eligible(h.state()), "Free Transform shows its own bar");

    let (mut h, _) = harness(Tool::Move);
    h.state_mut().ui.panels.context_task_bar = false;
    h.run_steps(3);
    assert!(bar(&h).is_none(), "Window > Contextual Task Bar off: no bar");
}

#[test]
fn duplicate_layer_adds_one_step_and_selects_the_copy_above() {
    let (mut h, id) = harness(Tool::Move);
    let before = history(&h);
    h.get_by_label("Duplicate layer").click();
    h.run_steps(4);
    assert_eq!(history(&h), before + 1, "one history step");
    assert!(!h.state().dialog_open(), "no Duplicate Layer dialog");
    let st = h.state().session.active().unwrap();
    assert_eq!(st.history.undo_label(), Some("Duplicate Layer"));
    let copy = st.active_layer.unwrap();
    assert_ne!(copy, id);
    assert_eq!(st.selected_layers(), &[copy], "the copy is selected");
    assert_eq!(st.doc.layer(copy).unwrap().name, "Shape copy");
    let order: Vec<LayerId> = st.doc.layers.iter().map(|l| l.id).collect();
    let (at, copy_at) = (order.iter().position(|l| *l == id).unwrap(), order.iter().position(|l| *l == copy).unwrap());
    assert_eq!(copy_at, at + 1, "the copy sits right above the original");
    let bounds = |l: LayerId| match &st.doc.layer(l).unwrap().content {
        LayerContent::Shape(sh) => sh.path.control_bounds(),
        _ => None,
    };
    assert_eq!(bounds(copy), bounds(id), "same bounds");
}

#[test]
fn more_properties_shows_the_properties_panel() {
    let (mut h, _) = harness(Tool::Move);
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, "window.panel.properties", json!({"show": false})).unwrap();
    h.run_steps(2);
    assert!(!crate::panels::panel_docking::is_shown(h.state(), "properties"));
    let before = history(&h);
    h.get_by_label("More properties").click();
    h.run_steps(4);
    assert!(crate::panels::panel_docking::is_shown(h.state(), "properties"), "the Properties panel shows");
    // A second click keeps it shown: More properties never hides the panel.
    h.get_by_label("More properties").click();
    h.run_steps(4);
    assert!(crate::panels::panel_docking::is_shown(h.state(), "properties"));
    assert_eq!(history(&h), before, "no document edit");
}

/// Types `text` into the Corner radius popover's field named `name` and commits it with Enter.
fn type_radius(h: &mut AppHarness, name: &str, text: &str) {
    let field = h.get_all_by_label(name).find(|n| n.accesskit_node().role() != egui::accesskit::Role::Button).expect("a field");
    field.click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    let field = h.get_all_by_label(name).find(|n| n.accesskit_node().role() != egui::accesskit::Role::Button).expect("a field");
    field.type_text(text);
    h.key_press(Key::Enter);
    h.run_steps(4);
}

#[test]
fn corner_radius_popover_sets_all_corners_then_one_corner_one_step_each() {
    let (mut h, id) = harness(Tool::Move);
    h.get_by_label("Corner radius").click();
    h.run_steps(4);
    for name in ["Top Left", "Top Right", "Bottom Left", "Bottom Right"] {
        assert!(h.query_by_label(name).is_some(), "the popover has {name}");
    }
    let before = history(&h);
    type_radius(&mut h, "Corner radius", "12");
    assert_eq!(radii(&h, id), [12.0; 4], "all corners");
    assert_eq!(history(&h), before + 1, "one step for all corners");
    type_radius(&mut h, "Bottom Left", "4");
    assert_eq!(radii(&h, id), [12.0, 12.0, 12.0, 4.0], "Bottom Left is the radii's fourth corner");
    assert_eq!(history(&h), before + 2, "one step for one corner");
}

#[test]
fn fill_and_stroke_edit_the_layer_as_the_options_bar_does() {
    let (mut h, id) = harness(Tool::Move);
    let layer = |h: &AppHarness| match &h.state().session.active().unwrap().doc.layer(id).unwrap().content {
        LayerContent::Shape(sh) => sh.clone(),
        _ => panic!("not a shape layer"),
    };
    let t = super::target(h.state()).unwrap();
    let gradients = [
        photocraft_doc::Fill::Solid(photocraft_doc::Color::rgba(0.0, 0.0, 0.0, 1.0)),
        photocraft_doc::Fill::Solid(photocraft_doc::Color::rgba(0.0, 0.0, 0.0, 1.0)),
    ];
    let blue = craft::color_ui::ui::Paint::Solid(craft::theme::Srgba::new(0.0, 0.0, 1.0, 1.0));
    let set = |paint: craft::color_ui::ui::Paint| {
        craft::behavior::EditEvent::set(craft::behavior::Mixed::Same(craft::color_ui::ui::Paint::None), paint, std::sync::Arc::from("Fill"), None)
    };
    let before = history(&h);
    let ctx = h.ctx.clone();
    super::apply(h.state_mut(), &ctx, &t, [0.0; 4], &gradients, vec![super::Change::Fill(set(blue.clone())), super::Change::Stroke(set(blue))]);
    let sh = layer(&h);
    assert_eq!(crate::shape_paint_ui::color_of(sh.fill.as_ref().unwrap()), Some([0.0, 0.0, 1.0, 1.0]), "the fill is blue");
    let stroke = sh.stroke.expect("a stroke was added");
    assert_eq!(crate::shape_paint_ui::color_of(&stroke.paint), Some([0.0, 0.0, 1.0, 1.0]));
    assert!(stroke.width > 0.0, "the stroke shows: {}", stroke.width);
    assert_eq!(history(&h), before + 2, "one step per change");
    let t = super::target(h.state()).unwrap();
    super::apply(h.state_mut(), &ctx, &t, [0.0; 4], &gradients, vec![super::Change::Stroke(set(craft::color_ui::ui::Paint::None))]);
    assert!(layer(&h).stroke.is_none(), "No Color removes the stroke");
}

#[test]
fn stroke_width_cell_edits_the_layer() {
    use craft::draw::ui::shape::ShapeChange;
    let (mut h, id) = harness(Tool::Move);
    let ctx = h.ctx.clone();
    let black = photocraft_doc::Fill::Solid(photocraft_doc::Color::rgba(0.0, 0.0, 0.0, 1.0));
    let gradients = [black.clone(), black];
    let stroke = |h: &AppHarness| match &h.state().session.active().unwrap().doc.layer(id).unwrap().content {
        LayerContent::Shape(sh) => sh.stroke.clone(),
        _ => None,
    };
    let set = |before, after| craft::behavior::EditEvent::set(craft::behavior::Mixed::Same(before), after, std::sync::Arc::from("Stroke"), None);
    let before = history(&h);
    let t = super::target(h.state()).unwrap();
    let width = set(ShapeChange::StrokeWidth(0.0), ShapeChange::StrokeWidth(5.0));
    super::apply(h.state_mut(), &ctx, &t, [0.0; 4], &gradients, vec![super::Change::Cells(width)]);
    assert_eq!(stroke(&h).map(|s| s.width), Some(5.0), "the width cell gives the shape a 5 px stroke");
    assert_eq!(history(&h), before + 1, "one step");
}

#[test]
fn hide_bar_from_the_more_menu_turns_the_bar_off() {
    let (mut h, _) = harness(Tool::Move);
    h.get_by_label("More options").click();
    h.run_steps(3);
    h.get_by_label("Hide bar").click();
    h.run_steps(4);
    assert!(!h.state().ui.panels.context_task_bar, "Hide bar unticks Window > Contextual Task Bar");
    assert!(bar(&h).is_none());
}
