//! #16780: a zoom typed into the status bar outside the document's zoom range, zero and negative ones included,
//! applies the nearest end and opens the "A number between {low} and {high} is required. Closest value inserted."
//! alert; an end typed exactly and a non-number open none, and no entry adds a History state.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;

use crate::PhotocraftApp;

/// A 2000 x 1500 px document at 100 %, its status bar in view.
fn harness() -> Harness<'static, Box<PhotocraftApp>> {
    let mut h = Harness::builder().with_size(vec2(1200.0, 800.0)).with_pixels_per_point(1.0).build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut session = photocraft_engine::Session::new();
        session.prefs.edit(|prefs| {
            prefs.interface.language = "en".into();
            prefs.general.animated_zoom = false;
        });
        let mut app = PhotocraftApp::with_context(cc.egui_ctx.clone(), session, crate::Services::default());
        app.run("file.new", json!({"width": 2000, "height": 1500})).unwrap();
        app
    });
    h.run_steps(6);
    let view = &mut h.state_mut().ui.views[0];
    view.zoom = 1.0;
    view.center = [1000.0, 750.0];
    view.clear_pending();
    h.run_steps(3);
    h
}

/// The open alerts' messages.
fn alerts(app: &PhotocraftApp) -> Vec<String> {
    app.framework.dialogs.open_dialogs().iter().filter_map(|d| d.fields.as_ref()?.get("message")?.as_str().map(str::to_string)).collect()
}

/// Closes every open alert with OK.
fn close_alerts(h: &mut Harness<'static, Box<PhotocraftApp>>) {
    while let Some(id) = h.state().framework.dialogs.open_dialogs().first().map(|d| d.id) {
        let ctx = h.ctx.clone();
        crate::dialogs::confirm(h.state_mut(), &ctx, id).unwrap();
        h.run_steps(2);
    }
}

/// Types `entry` into the status bar's Zoom field, over its whole text, and presses Enter; returns the view's zoom
/// (1.0 is 100 %) and the alerts that opened.
fn enter(h: &mut Harness<'static, Box<PhotocraftApp>>, entry: &str) -> (f64, Vec<String>) {
    let bottom = crate::canvas::document_display::active_view(h.state()).expect("the document is shown").rect.bottom();
    let field = h
        .query_all_by_role(Role::SpinButton)
        .find(|node| node.rect().center().y > bottom && node.accesskit_node().label().as_deref() == Some("Zoom"))
        .expect("the status bar has its Zoom field");
    field.click();
    h.run_steps(2);
    let select_all = Modifiers::COMMAND;
    h.event(Event::ModifiersChanged(select_all));
    for pressed in [true, false] {
        h.event(Event::Key { key: Key::A, physical_key: None, pressed, repeat: false, modifiers: select_all });
    }
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
    h.event(Event::Text(entry.into()));
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(4);
    (f64::from(h.state().ui.views[0].zoom), alerts(h.state()))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn an_out_of_range_zoom_applies_the_nearest_end_and_opens_the_number_alert() {
    let mut h = harness();
    let steps = h.state().session.active().unwrap().history.past_len();
    let mut low = None;
    for (entry, top) in [("99999", true), ("13000", true), ("0", false), ("-50", false), ("1/10000", false)] {
        let (zoom, shown) = enter(&mut h, entry);
        assert_eq!(shown.len(), 1, "{entry}: one alert: {shown:?}");
        let message = &shown[0];
        assert!(message.starts_with("A number between ") && message.ends_with(" is required. Closest value inserted."), "{entry}: {message}");
        if top {
            assert!(close(zoom, 128.0), "{entry}: the top end applied: {zoom}");
        } else {
            assert!(zoom > 0.0 && zoom < 0.01, "{entry}: the low end applied: {zoom}");
            assert!(low.is_none_or(|low| close(low, zoom)), "{entry}: the same low end: {zoom}");
            low = Some(zoom);
        }
        close_alerts(&mut h);
    }
    assert_eq!(h.state().session.active().unwrap().history.past_len(), steps, "no History state");
}

#[test]
fn an_end_typed_exactly_or_a_non_number_opens_no_alert() {
    let mut h = harness();
    let (zoom, shown) = enter(&mut h, "12800");
    assert!(close(zoom, 128.0) && shown.is_empty(), "12800: {zoom} {shown:?}");
    let (zoom, shown) = enter(&mut h, "250");
    assert!(close(zoom, 2.5) && shown.is_empty(), "250: {zoom} {shown:?}");
    let (zoom, shown) = enter(&mut h, "abc");
    assert!(close(zoom, 2.5) && shown.is_empty(), "abc changes nothing: {zoom} {shown:?}");
}
