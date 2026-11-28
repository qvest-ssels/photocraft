//! #16733: the Content-Aware Scale box's right-click menu has the Free Transform menu's rows, every
//! one greyed, as in the reference application. A pick that reaches the box anyway (automation)
//! leaves it as it is; the box still scales and commits.

use craft::behavior::geometry::quad::TransformMode;
use craft::behavior::kurbo::Point;
use craft::draw::ui::transform::{TransformEdit, TransformMenuWords, TransformPreset};
use craft::i18n::WordSet;
use serde_json::json;

use super::COMMAND;
use crate::PhotocraftApp;

/// A 64×48 document with a red 40×24 rectangle on a layer at (8, 8).
fn app() -> Box<PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 64, "height": 48})).unwrap();
    app.run("layer.new.layer", json!({})).unwrap();
    app.run("select.rect", json!({"x": 8, "y": 8, "width": 40, "height": 24})).unwrap();
    app.run("edit.fill", json!({"color": "#c03020"})).unwrap();
    app.run("select.deselect", json!({})).unwrap();
    app.sync_views();
    app
}

/// The open box's menu: each labelled row with whether it is enabled.
fn menu(app: &PhotocraftApp) -> Vec<(String, bool)> {
    let t = app.ui.transform.as_ref().expect("a box is open");
    t.session.menu(&TransformMenuWords::english()).iter().filter(|e| !e.is_separator()).map(|e| (format!("{:?}", e.label()), e.is_enabled())).collect()
}

#[test]
fn the_content_aware_scale_box_menu_is_the_free_transform_menu_all_greyed() {
    let mut app = app();
    let ctx = app.ctx().clone();
    crate::transform_tool::begin(&mut app, &ctx).unwrap();
    let free = menu(&app);
    assert!(free.iter().filter(|(_, enabled)| *enabled).count() >= 7, "Free Transform keeps its rows: {free:?}");
    crate::transform_tool::cancel(&mut app);

    crate::menus::invoke(&mut app, &ctx, COMMAND, json!({})).unwrap();
    let cas = menu(&app);
    assert_eq!(cas.len(), 12);
    assert_eq!(cas.iter().map(|(label, _)| label).collect::<Vec<_>>(), free.iter().map(|(label, _)| label).collect::<Vec<_>>());
    assert!(cas.iter().all(|(_, enabled)| !enabled), "{cas:?}");
}

#[test]
fn a_mode_or_preset_pick_leaves_the_content_aware_scale_box_as_it_is() {
    let mut app = app();
    let ctx = app.ctx().clone();
    crate::menus::invoke(&mut app, &ctx, COMMAND, json!({})).unwrap();
    let t = app.ui.transform.as_mut().unwrap();
    let quad = t.session.quad();
    for preset in [
        TransformPreset::Rotate180,
        TransformPreset::Rotate90Clockwise,
        TransformPreset::Rotate90CounterClockwise,
        TransformPreset::FlipHorizontal,
        TransformPreset::FlipVertical,
    ] {
        t.session.apply(TransformEdit::Preset(preset), 0.0);
    }
    t.session.apply(TransformEdit::Mode(TransformMode::Rotate), 0.0);
    assert_eq!(t.session.quad(), quad, "no preset turns or flips the box");
    assert_eq!(t.session.current_mode(), TransformMode::Free);
    assert!(!t.session.can_undo(), "no step recorded");

    // The handles still scale it, and ↩ commits one Content-Aware Scale step.
    let past = app.session.active().unwrap().history.past_len();
    let narrowed = [Point::new(8.0, 8.0), Point::new(32.0, 8.0), Point::new(32.0, 32.0), Point::new(8.0, 32.0)];
    app.ui.transform.as_mut().unwrap().edit_box(|b| b.quad = narrowed);
    assert_eq!(app.ui.transform.as_ref().unwrap().session.quad(), narrowed);
    crate::transform_tool::commit(&mut app);
    assert!(app.ui.transform.is_none());
    let st = app.session.active().unwrap();
    assert_eq!(st.history.past_len(), past + 1);
    assert_eq!(st.history.undo_label(), Some("Content-Aware Scale"));
}
