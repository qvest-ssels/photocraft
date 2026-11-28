//! #16746: while the foreground or background Color Picker is open the Eyedropper is the active tool, with its options
//! bar, and a click on the image samples with its Sample Size and Sample. OK, Cancel, Enter, Escape and the close box
//! put the previous tool back, with its settings as they were. The switch is no history step, and a picker opened with
//! the Eyedropper already active leaves it active. A tool the user chooses while the picker is open stands, and the
//! pickers other than the foreground and background ones keep sampling every layer.

use craft::color_ui::ui::{ColorPickerDialog, SampleSize, SampleSource};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use super::*;
use crate::control::{ControlRequest, Reply, handle};
use crate::state::Tool;

/// A 40 x 20 document with `tool` selected, in a harness that runs the app's dialog pass, so a click or a key reaches
/// the hosted picker.
fn harness(tool: Tool) -> Harness<'static, Box<PhotocraftApp>> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 40, "height": 20})).unwrap();
    crate::tool_memory::select(&mut app, tool);
    let mut h = Harness::builder()
        .with_context(app.ctx().clone())
        .with_size(vec2(760.0, 480.0))
        .build_ui_state(|ui, app: &mut Box<PhotocraftApp>| crate::dialogs::run(app, ui), app);
    h.run_steps(2);
    h
}

/// The tool `ui.inspect` reports.
fn inspected_tool(app: &mut PhotocraftApp) -> Value {
    let ctx = app.ctx().clone();
    let (req, _rx) = ControlRequest::new("ui.inspect", json!({}));
    let Reply::Done(reply) = handle(app, &ctx, &req) else { panic!("ui.inspect replies at once") };
    reply["result"]["tool"].clone()
}

fn history_steps(app: &PhotocraftApp) -> usize {
    app.session.active().unwrap().history.past_len()
}

/// Closes the picker the way `action` names.
fn close_with(h: &mut Harness<'static, Box<PhotocraftApp>>, action: &str) {
    match action {
        "Enter" => h.key_press(egui::Key::Enter),
        "Escape" => h.key_press(egui::Key::Escape),
        "close box" => h.get_by_label("Close").click(),
        _ => h.get_by_label(action).click(),
    }
    h.run_steps(3);
}

#[test]
fn the_eyedropper_is_active_while_the_picker_is_open_and_the_previous_tool_comes_back() {
    for target in ["foreground", "background"] {
        for action in ["OK", "Cancel", "Enter", "Escape", "close box"] {
            let mut h = harness(Tool::Brush);
            let steps = history_steps(h.state());
            let id = open(h.state_mut(), target);
            h.run_steps(4);
            assert_eq!(h.state().ui.tool, Tool::Eyedropper, "{target}, {action}: the Eyedropper is active while the picker is open");
            assert_eq!(inspected_tool(h.state_mut()), json!(Tool::Eyedropper), "{target}, {action}: ui.inspect");
            close_with(&mut h, action);
            assert!(h.state().framework.dialogs.get_by_id::<ColorPickerDialog>(id).is_none(), "{target}: {action} closes the picker");
            assert_eq!(h.state().ui.tool, Tool::Brush, "{target}, {action}: the previous tool comes back");
            assert_eq!(inspected_tool(h.state_mut()), json!(Tool::Brush), "{target}, {action}: ui.inspect");
            assert_eq!(history_steps(h.state()), steps, "{target}, {action}: the tool switch is no history step");
        }
    }
}

/// The Brush's settings are the ones it had before the picker opened.
#[test]
fn the_previous_tool_keeps_its_settings() {
    let mut h = harness(Tool::Brush);
    h.state_mut().session.tools.brush.size = 37.0;
    h.state_mut().session.tools.brush.opacity = 0.4;
    h.run_steps(2);
    let before = h.state().session.tools.brush.clone();
    let options = serde_json::to_value(&h.state().ui.tool_options).unwrap();
    open(h.state_mut(), "foreground");
    h.run_steps(4);
    assert_eq!(h.state().ui.tool, Tool::Eyedropper);
    close_with(&mut h, "Cancel");
    h.run_steps(2);
    assert_eq!(h.state().ui.tool, Tool::Brush);
    let after = &h.state().session.tools.brush;
    assert_eq!((after.size, after.opacity), (before.size, before.opacity), "the Brush keeps its size and opacity");
    assert_eq!(serde_json::to_value(&h.state().ui.tool_options).unwrap(), options, "no tool option changed");
}

/// A picker opened with the Eyedropper already active leaves it active after it closes.
#[test]
fn an_eyedropper_already_active_stays_active() {
    for action in ["OK", "Cancel", "Escape"] {
        let mut h = harness(Tool::Eyedropper);
        open(h.state_mut(), "foreground");
        h.run_steps(4);
        assert_eq!(h.state().ui.tool, Tool::Eyedropper, "{action}");
        close_with(&mut h, action);
        assert_eq!(h.state().ui.tool, Tool::Eyedropper, "{action}: the Eyedropper stays");
    }
}

/// OK applies the picked colour as the restored tool says: the tool is back before the colour is committed.
#[test]
fn ok_sets_the_colour_with_the_previous_tool_back() {
    let mut h = harness(Tool::Move);
    let id = open(h.state_mut(), "foreground");
    testing::set_color(h.state_mut(), id, "#336699");
    h.run_steps(4);
    close_with(&mut h, "OK");
    assert_eq!(h.state().ui.tool, Tool::Move);
    let fg = h.state().session.tools.foreground;
    assert_eq!(hex([fg[0], fg[1], fg[2]]), "#336699");
}

/// A click on the image samples into the picker with the Eyedropper's Sample: Current Layer picks the selected
/// layer's colour where a layer above covers it. The foreground stays until OK.
#[test]
fn a_canvas_click_samples_with_the_eyedroppers_sample_setting() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 40, "height": 20, "background": "transparent"})).unwrap();
    let below = app.run("shape.create", json!({"kind": "rect", "rect": [0, 0, 40, 20], "fill": "#ff0000"})).unwrap()["layer"].as_u64().unwrap();
    app.run("shape.create", json!({"kind": "rect", "rect": [0, 0, 40, 20], "fill": "#00ff00"})).unwrap();
    app.run("layer.select", json!({"layer": below})).unwrap();
    crate::tool_memory::select(&mut app, Tool::Brush);
    app.ui.tool_options.eyedropper.size = SampleSize::Point;
    app.ui.tool_options.eyedropper.source = SampleSource::Current;
    let foreground = app.session.tools.foreground;
    let id = open(&mut app, "foreground");
    crate::canvas::testing::clear_of_dialogs(&mut app, [10.0, 10.0]);
    crate::canvas::testing::click(&mut app, [10.0, 10.0], egui::Modifiers::NONE);
    assert_eq!(app.ui.tool, Tool::Eyedropper);
    assert_eq!(rgb_hex(testing::current(&app, id).expect("the picker is open").color), "#ff0000", "the selected layer's colour");
    assert_eq!(app.session.tools.foreground, foreground, "only OK sets the foreground");

    app.ui.tool_options.eyedropper.source = SampleSource::All;
    crate::canvas::testing::click(&mut app, [12.0, 10.0], egui::Modifiers::NONE);
    assert_eq!(rgb_hex(testing::current(&app, id).expect("the picker is open").color), "#00ff00", "all layers");
}

/// A Move drag that runs when the picker opens keeps the Move tool to its release (switching would cancel the drag);
/// the Eyedropper is current from the pass after, and the Move tool comes back when the picker closes.
#[test]
fn a_running_drag_keeps_its_tool_until_the_release() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 40, "height": 20})).unwrap();
    crate::tool_memory::select(&mut app, Tool::Move);
    crate::canvas::testing::clear_of_dialogs(&mut app, [10.0, 10.0]);
    crate::canvas::testing::press(&mut app, [10.0, 10.0], egui::Modifiers::NONE);
    crate::canvas::testing::move_to(&mut app, [12.0, 11.0], egui::Modifiers::NONE);
    assert!(crate::canvas::testing::in_gesture(&app));
    let id = open(&mut app, "foreground");
    crate::canvas::testing::frame(&mut app);
    assert_eq!(app.ui.tool, Tool::Move, "the drag keeps its tool");
    crate::canvas::testing::release(&mut app, [14.0, 12.0], egui::Modifiers::NONE);
    crate::canvas::testing::frame(&mut app);
    assert_eq!(app.ui.tool, Tool::Eyedropper, "the Eyedropper after the release");
    let ctx = app.ctx().clone();
    app.framework.dialogs.cancel_by_id(&ctx, id);
    crate::canvas::testing::frame(&mut app);
    assert!(app.framework.dialogs.get_by_id::<ColorPickerDialog>(id).is_none());
    assert_eq!(app.ui.tool, Tool::Move);
}

/// While type is being edited the picker leaves the Type tool: another tool would commit the edit.
#[test]
fn a_type_edit_keeps_the_type_tool() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 200, "height": 100})).unwrap();
    crate::tool_memory::select(&mut app, Tool::Type);
    let layer = app.run("type.create", json!({"text": "Hello", "color": "#000000", "size": 24, "x": 20, "y": 50})).unwrap()["layer"].as_u64().unwrap();
    app.ui.text_edit = Some(crate::state::TextEdit { layer, document: None, caret: 0, anchor: 0, session: "s".into(), created: false, mask: false });
    assert!(app.ui.text_edit.is_some());
    let id = open(&mut app, "foreground");
    crate::canvas::testing::frame(&mut app);
    crate::canvas::testing::frame(&mut app);
    assert_eq!(app.ui.tool, Tool::Type);
    assert!(app.ui.text_edit.is_some(), "the edit goes on");
    let ctx = app.ctx().clone();
    app.framework.dialogs.cancel_by_id(&ctx, id);
    crate::canvas::testing::frame(&mut app);
    assert_eq!(app.ui.tool, Tool::Type);
}

/// A picker opened with the Eyedropper already active does not take a tool chosen afterwards back to the Eyedropper.
#[test]
fn a_tool_chosen_after_opening_with_the_eyedropper_stands() {
    let mut h = harness(Tool::Eyedropper);
    open(h.state_mut(), "foreground");
    h.run_steps(4);
    crate::tool_memory::select(h.state_mut(), Tool::Move);
    h.run_steps(4);
    assert_eq!(h.state().ui.tool, Tool::Move, "the chosen Move tool stays while the picker is open");
    close_with(&mut h, "Cancel");
    assert_eq!(h.state().ui.tool, Tool::Move, "and after it closes");
}

/// The Eyedropper chosen explicitly while the picker is open (after another tool) stays when it closes: the tool the
/// picker replaced does not come back over the user's choice.
#[test]
fn an_eyedropper_chosen_while_open_stays_after_close() {
    let mut h = harness(Tool::Brush);
    open(h.state_mut(), "foreground");
    h.run_steps(4);
    assert_eq!(h.state().ui.tool, Tool::Eyedropper);
    crate::tool_memory::select(h.state_mut(), Tool::Move);
    crate::tool_memory::select(h.state_mut(), Tool::Eyedropper);
    h.run_steps(2);
    close_with(&mut h, "OK");
    assert_eq!(h.state().ui.tool, Tool::Eyedropper, "the explicit choice stands");
}

/// The same with a tool change that does not go through `tool_memory::select` (a letter tap sets the tool itself): the
/// picker sees it on its next pass.
#[test]
fn a_tool_tapped_while_open_stands() {
    let mut h = harness(Tool::Brush);
    open(h.state_mut(), "foreground");
    h.run_steps(4);
    h.state_mut().ui.tool = Tool::Move;
    h.run_steps(2);
    h.state_mut().ui.tool = Tool::Eyedropper;
    h.run_steps(2);
    close_with(&mut h, "Cancel");
    assert_eq!(h.state().ui.tool, Tool::Eyedropper, "the tapped Eyedropper stands");
}

/// A picker other than the foreground or background one (the Note tool's colour) keeps sampling every layer: the
/// Eyedropper's Sample setting is the toolbar pickers' alone, and the tool stays.
#[test]
fn another_picker_samples_every_layer() {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    app.run("file.new", json!({"width": 40, "height": 20, "background": "transparent"})).unwrap();
    let below = app.run("shape.create", json!({"kind": "rect", "rect": [0, 0, 40, 20], "fill": "#ff0000"})).unwrap()["layer"].as_u64().unwrap();
    app.run("shape.create", json!({"kind": "rect", "rect": [0, 0, 40, 20], "fill": "#00ff00"})).unwrap();
    app.run("layer.select", json!({"layer": below})).unwrap();
    crate::tool_memory::select(&mut app, Tool::Brush);
    app.ui.tool_options.eyedropper.size = SampleSize::Point;
    app.ui.tool_options.eyedropper.source = SampleSource::Current;
    let id = open_for(&mut app, "Note Color", craft::theme::Srgba::from_rgb_array([0.0, 0.0, 0.0]), None, PickTarget::NoteColor);
    crate::canvas::testing::clear_of_dialogs(&mut app, [10.0, 10.0]);
    crate::canvas::testing::click(&mut app, [10.0, 10.0], egui::Modifiers::NONE);
    assert_eq!(app.ui.tool, Tool::Brush, "the Note colour picker keeps the tool");
    assert_eq!(rgb_hex(testing::current(&app, id).expect("the picker is open").color), "#00ff00", "the composite colour");
}
