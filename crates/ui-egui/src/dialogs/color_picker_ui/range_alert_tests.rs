//! #16741: a number typed outside a Color Picker field's range, or more than six hex digits (#16739), opens the
//! reference's alert over the picker, titled like it: "An integer between 0 and 255 is required. Closest value
//! inserted." The closest value is in the field and the colour follows it. `ui.inspect` lists the alert and
//! `ui.dialog.confirm` answers it; the picker stays open.

use craft::color_ui::ui::ColorPickerDialog;
use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use serde_json::{Value, json};

use super::*;
use crate::control::{ControlRequest, Reply, handle};

const TITLE: &str = "Color Picker (Foreground Color)";

/// A harness running the app's dialog pass, so keys reach the hosted dialogs.
fn harness() -> Harness<'static, Box<PhotocraftApp>> {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), Default::default());
    let mut h = Harness::builder()
        .with_context(app.ctx().clone())
        .with_size(vec2(900.0, 640.0))
        .build_ui_state(|ui, app: &mut Box<PhotocraftApp>| crate::dialogs::run(app, ui), app);
    PhotocraftApp::setup_context(&h.ctx, crate::theme::AppTheme::studio());
    h.run_steps(2);
    h
}

fn call(app: &mut PhotocraftApp, method: &str, params: Value) -> Value {
    let ctx = app.ctx().clone();
    let (req, _rx) = ControlRequest::new(method, params);
    let Reply::Done(reply) = handle(app, &ctx, &req) else { panic!("{method} replies at once") };
    reply
}

/// The hosted dialogs as `ui.inspect` lists them, the top one last.
fn dialogs(app: &mut PhotocraftApp) -> Vec<Value> {
    call(app, "ui.inspect", json!({}))["result"]["dialogs"].as_array().cloned().unwrap_or_default()
}

/// The messages of the open alerts.
fn alerts(h: &Harness<'static, Box<PhotocraftApp>>) -> Vec<String> {
    h.state().framework.dialogs.open_dialogs().iter().filter_map(|d| d.fields.as_ref()?.get("message")?.as_str().map(str::to_string)).collect()
}

/// Types `text` over the picker field named `name` and commits it with Tab (Enter would confirm the picker).
fn type_into(h: &mut Harness<'static, Box<PhotocraftApp>>, role: Role, name: &str, text: &str) {
    h.query_all_by_role(role).find(|n| n.accesskit_node().label().as_deref() == Some(name)).unwrap_or_else(|| panic!("the {name} field")).click();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text(text.into()));
    h.run_steps(1);
    h.key_press(egui::Key::Tab);
    h.run_steps(4);
}

/// The foreground picker on #336699.
fn picker(h: &mut Harness<'static, Box<PhotocraftApp>>) -> DialogId {
    let id = open(h.state_mut(), "foreground");
    testing::set_color(h.state_mut(), id, "#336699");
    h.run_steps(4);
    id
}

fn hex(h: &Harness<'static, Box<PhotocraftApp>>, id: DialogId) -> Option<String> {
    testing::current(h.state(), id).map(|picked| picked.color.to_hex())
}

#[test]
fn an_out_of_range_channel_opens_the_alert_over_the_picker() {
    for (field, typed, message, colour) in [
        ("Red", "300", "An integer between 0 and 255 is required. Closest value inserted.", "#ff6699"),
        ("Hue", "400", "An integer between 0 and 360 is required. Closest value inserted.", ""),
        ("Saturation", "150", "An integer between 0 and 100 is required. Closest value inserted.", ""),
        ("Green-red", "200", "An integer between -128 and 127 is required. Closest value inserted.", ""),
    ] {
        let mut h = harness();
        let id = picker(&mut h);
        let titles = h.query_all_by_label(TITLE).count();
        type_into(&mut h, Role::SpinButton, field, typed);
        assert_eq!(alerts(&h), [message], "{field} {typed}");
        let listed = dialogs(h.state_mut());
        let alert = listed.last().cloned().unwrap_or(Value::Null);
        assert_eq!(alert["fields"]["message"], message, "ui.inspect lists the alert on top: {alert}");
        assert!(h.query_all_by_label(TITLE).count() > titles, "the alert is titled like the picker");
        if !colour.is_empty() {
            assert_eq!(hex(&h, id).as_deref(), Some(colour), "{field} {typed}: the colour follows the closest value");
        }
        assert_eq!(call(h.state_mut(), "ui.dialog.confirm", json!({"dialog": alert["id"]}))["ok"], true, "ui.dialog.confirm answers it");
        h.run_steps(4);
        assert!(alerts(&h).is_empty(), "{field} {typed}: OK closes the alert");
        assert!(h.state().framework.dialogs.get_by_id::<ColorPickerDialog>(id).is_some(), "and the picker stays open");
    }
}

#[test]
fn seven_hex_digits_insert_ffffff_with_the_alert() {
    let mut h = harness();
    let id = picker(&mut h);
    type_into(&mut h, Role::TextInput, "Hex color", "1234567");
    assert_eq!(alerts(&h), ["A number between 000000 and ffffff is required. Closest value inserted."]);
    assert_eq!(hex(&h, id).as_deref(), Some("#ffffff"));
}

#[test]
fn in_range_input_opens_no_alert() {
    let mut h = harness();
    let id = picker(&mut h);
    type_into(&mut h, Role::SpinButton, "Red", "128");
    type_into(&mut h, Role::TextInput, "Hex color", "abc");
    assert!(alerts(&h).is_empty(), "{:?}", alerts(&h));
    assert_eq!(hex(&h, id).as_deref(), Some("#aabbcc"));
}
