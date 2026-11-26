//! #16808: Escape in an options-bar number field puts back the value the field had when it took
//! the focus, past typed values (which apply as they are typed) and arrow-key steps, and leaves
//! the field; Enter, Tab and a click elsewhere commit, and Enter leaves the field too, so the next
//! arrow key does not step it. Tool options add no history state.
//!
//! The bar asks its fields for these keys with `NumberKeys::Bar` (`panels/options_bar.rs`).

use egui::accesskit::Role;
use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;

use crate::PhotocraftApp;
use crate::state::Tool;

type Bar = Harness<'static, Box<PhotocraftApp>>;

/// The options bar of a new document with `tool`, and the dialog host for its alerts.
fn bar(tool: Tool) -> Bar {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 80, "height": 80})).unwrap();
    app.ui.tool = tool;
    crate::theme::install_fonts(app.ctx());
    crate::theme::apply(app.ctx(), &crate::theme::AppTheme::pro());
    let mut h = Harness::builder().with_context(app.ctx().clone()).with_size(vec2(1800.0, 300.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        move |ui, app: &mut Box<PhotocraftApp>| {
            crate::panels::options_bar(app, ui);
            crate::dialogs::run(app, ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

/// The field named `label` ("Opacity", "Flow", "Feather"), with or without its caption's colon.
fn field<'a>(h: &'a Bar, label: &str) -> egui_kittest::Node<'a> {
    h.query_all_by_role(Role::SpinButton)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.trim_end_matches(':') == label))
        .unwrap_or_else(|| panic!("the {label} field"))
}

fn click_at(h: &mut Bar, at: Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(1);
}

/// Clicks the field named `label`.
fn focus(h: &mut Bar, label: &str) {
    let at = field(h, label).rect().center();
    click_at(h, at);
}

/// Focuses the field named `label` and types `text` over its value.
fn typed(h: &mut Bar, label: &str, text: &str) {
    focus(h, label);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.event(egui::Event::Text(text.into()));
    h.run_steps(1);
}

fn press(h: &mut Bar, key: Key) {
    h.key_press(key);
    h.run_steps(3);
}

/// Types `text` into the field named `label` and presses Enter: the starting value.
fn set(h: &mut Bar, label: &str, text: &str) {
    typed(h, label, text);
    press(h, Key::Enter);
}

fn focused(h: &Bar, label: &str) -> bool {
    field(h, label).accesskit_node().is_focused()
}

fn opacity(h: &Bar) -> f32 {
    (h.state().session.tools.brush.opacity * 100.0).round()
}

fn flow(h: &Bar) -> f32 {
    (h.state().session.tools.brush.flow * 100.0).round()
}

fn feather(h: &Bar) -> f32 {
    h.state().ui.tool_options.feather
}

fn history(h: &Bar) -> usize {
    h.state().session.active().unwrap().history.entries().len()
}

#[test]
fn escape_puts_back_the_value_the_field_had_when_it_took_the_focus() {
    let mut h = bar(Tool::Brush);
    set(&mut h, "Opacity", "50");
    let states = history(&h);
    typed(&mut h, "Opacity", "60");
    press(&mut h, Key::Escape);
    assert_eq!(opacity(&h), 50.0, "Opacity 50, type 60, Escape");

    focus(&mut h, "Opacity");
    press(&mut h, Key::ArrowUp);
    press(&mut h, Key::ArrowUp);
    assert!(opacity(&h) > 50.0, "the steps apply: {}", opacity(&h));
    press(&mut h, Key::Escape);
    assert_eq!(opacity(&h), 50.0, "Opacity 50, Up, Up, Escape");
    assert!(!focused(&h, "Opacity"), "Escape leaves the field");
    press(&mut h, Key::ArrowUp);
    assert_eq!(opacity(&h), 50.0, "an Up after the Escape does not step the field");

    set(&mut h, "Flow", "50");
    typed(&mut h, "Flow", "60");
    press(&mut h, Key::Escape);
    assert_eq!(flow(&h), 50.0, "Flow 50, type 60, Escape");
    assert_eq!(history(&h), states, "tool options add no history state");

    let mut m = bar(Tool::RectMarquee);
    set(&mut m, "Feather", "10");
    let states = history(&m);
    typed(&mut m, "Feather", "60");
    assert_eq!(feather(&m), 60.0, "a typed number applies as it is typed");
    press(&mut m, Key::Escape);
    assert_eq!(feather(&m), 10.0, "Feather 10, type 60, Escape");
    focus(&mut m, "Feather");
    press(&mut m, Key::ArrowUp);
    press(&mut m, Key::ArrowUp);
    assert_eq!(feather(&m), 12.0, "two steps apply");
    press(&mut m, Key::Escape);
    assert_eq!(feather(&m), 10.0, "Feather 10, Up, Up, Escape");
    assert_eq!(history(&m), states, "tool options add no history state");
}

#[test]
fn enter_tab_and_a_click_elsewhere_commit_and_enter_leaves_the_field() {
    let mut h = bar(Tool::Brush);
    set(&mut h, "Opacity", "50");
    let states = history(&h);
    typed(&mut h, "Opacity", "40");
    press(&mut h, Key::Enter);
    assert_eq!(opacity(&h), 40.0, "Enter commits");
    assert!(!focused(&h, "Opacity"), "Enter leaves the field");
    press(&mut h, Key::ArrowUp);
    assert_eq!(opacity(&h), 40.0, "an Up after the Enter does not step the field");

    typed(&mut h, "Opacity", "45");
    press(&mut h, Key::Tab);
    assert_eq!(opacity(&h), 45.0, "Tab commits");

    // A click away from the bar's controls (the canvas, in the app).
    typed(&mut h, "Opacity", "35");
    click_at(&mut h, Pos2::new(900.0, 280.0));
    assert_eq!(opacity(&h), 35.0, "typed 35 + a click elsewhere");
    assert!(!focused(&h, "Opacity"));
    // The next visit starts from the committed value.
    typed(&mut h, "Opacity", "70");
    press(&mut h, Key::Escape);
    assert_eq!(opacity(&h), 35.0);
    assert_eq!(history(&h), states, "tool options add no history state");

    let mut m = bar(Tool::RectMarquee);
    set(&mut m, "Feather", "10");
    typed(&mut m, "Feather", "40");
    click_at(&mut m, Pos2::new(900.0, 280.0));
    assert_eq!(feather(&m), 40.0, "Feather: typed 40 + a click elsewhere");
    assert!(!focused(&m, "Feather"));
}
