//! #16846: with every document in a floating window, Full Screen Mode With Menu Bar and Full Screen Mode show only the
//! active document, without its window, in the main window's document area. Ctrl+Tab shows the next document there,
//! and Standard Screen Mode floats every window again at its old place and size, the last active document still
//! active. The measurement had two documents floating, a big one and a small active one.

use craft::ui::canvas::ToolHost;
use craft::ui::shell::ScreenMode;
use egui::Rect;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;

use super::document_area::single_canvas_document;
use super::document_display::CanvasHost;
use super::document_windows::full_screen_shows_active;
use super::testing;
use crate::PhotocraftApp;

type AppHarness = Harness<'static, (Box<PhotocraftApp>, bool)>;

/// The measured start: a 1200 x 800 document and a 400 x 300 one, the small one active, each floated into its own
/// window by Window › Arrange › Float All in Windows; then the document area and the floating windows drawn in one
/// window (the windows on the fallback layer, where they have no OS window of their own).
fn floating_pair() -> AppHarness {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 1200, "height": 800, "name": "big"})).unwrap();
    app.run("file.new", json!({"width": 400, "height": 300, "name": "small"})).unwrap();
    app.sync_views();
    let ctx = app.ctx().clone();
    crate::menus::invoke(&mut app, &ctx, "window.arrange.floatAllInWindows", json!({})).expect("both documents float");
    let mut h = Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_context(ctx).build_ui_state(
        |ui, (app, ready): &mut (Box<PhotocraftApp>, bool)| {
            if *ready {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| testing::document_area(app, ui));
                crate::canvas::extra_windows(app, ui);
            }
        },
        (app, false),
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::AppTheme::pro());
    h.state_mut().1 = true;
    h.run_steps(3);
    h
}

fn app(h: &AppHarness) -> &PhotocraftApp {
    &h.state().0
}

fn set_mode(h: &mut AppHarness, mode: ScreenMode) {
    h.state_mut().0.ui.view.screen_mode = mode;
    h.run_steps(3);
}

/// The titles of the floating windows drawn this pass: each window, a group on the fallback layer, is named
/// "<document> — window <id>".
fn windows_shown(h: &AppHarness) -> Vec<String> {
    h.query_all_by_role(egui::accesskit::Role::Group).filter_map(|n| n.accesskit_node().label()).filter(|label| label.contains("— window")).collect()
}

fn index_of(app: &PhotocraftApp, name: &str) -> usize {
    app.session.documents().iter().position(|st| st.doc.name == name).expect("the document is open")
}

/// The rect the main area's canvas of document `idx` showed in its last pass, `None` before its first.
fn main_canvas_rect(app: &PhotocraftApp, idx: usize) -> Option<Rect> {
    ToolHost::shown_space(app.ctx(), CanvasHost::main(idx).canvas_id()).map(|space| space.view.rect)
}

#[test]
fn standard_screen_mode_keeps_both_windows_and_no_document_in_the_main_area() {
    let h = floating_pair();
    let app = app(&h);
    assert_eq!(app.ui.view.screen_mode, ScreenMode::Standard);
    assert!(!full_screen_shows_active(app), "Standard Screen Mode leaves the floating windows as they are");
    assert_eq!(windows_shown(&h).len(), 2, "both floating windows show: {:?}", windows_shown(&h));
    assert_eq!(single_canvas_document(app), None, "the main area shows no document of its own");
    assert!(app.drop_canvas_rect.is_none(), "so no canvas drew there");
}

#[test]
fn the_full_screen_modes_show_only_the_active_document_without_its_window() {
    for mode in [ScreenMode::FullScreenWithMenuBar, ScreenMode::FullScreen] {
        let mut h = floating_pair();
        let small = index_of(app(&h), "small");
        assert_eq!(app(&h).session.active_index(), Some(small), "the small document is active");
        set_mode(&mut h, mode);
        let app = app(&h);
        assert!(full_screen_shows_active(app), "{mode:?}");
        assert_eq!(windows_shown(&h), Vec::<String>::new(), "{mode:?}: no floating window shows, framed or not");
        assert_eq!(single_canvas_document(app), Some(small), "{mode:?}: the main area shows the active document");
        let area = app.drop_canvas_rect.expect("a canvas drew in the main area");
        assert_eq!(main_canvas_rect(app, small), Some(area), "{mode:?}: the active document's canvas takes the whole document area");
        assert_eq!(CanvasHost::primary(app, small), CanvasHost::main(small), "{mode:?}: the tools and dialogs work on that canvas");
        let big = index_of(app, "big");
        assert_eq!(main_canvas_rect(app, big), None, "{mode:?}: the other document is not drawn");
    }
}

#[test]
fn ctrl_tab_in_a_full_screen_mode_shows_the_next_document_in_the_same_place() {
    for mode in [ScreenMode::FullScreenWithMenuBar, ScreenMode::FullScreen] {
        let mut h = floating_pair();
        set_mode(&mut h, mode);
        let area = app(&h).drop_canvas_rect.expect("the active document shows");
        // Ctrl+Tab runs document.activate one document on.
        h.state_mut().0.run("document.activate", json!({"offset": 1})).expect("the next document activates");
        h.run_steps(3);
        let app = app(&h);
        let big = index_of(app, "big");
        assert_eq!(app.session.active_index(), Some(big), "{mode:?}: Ctrl+Tab wrapped to the big document");
        assert_eq!(single_canvas_document(app), Some(big), "{mode:?}: the main area now shows it");
        assert_eq!(main_canvas_rect(app, big), Some(area), "{mode:?}: in the place the other one had");
        assert_eq!(windows_shown(&h), Vec::<String>::new(), "{mode:?}: still no floating window");
    }
}

#[test]
fn standard_screen_mode_floats_every_window_again_where_it_was() {
    let mut h = floating_pair();
    let before: Vec<(u64, Rect)> = app(&h).ui.document_windows.floating.iter().map(|(g, rect)| (g.id, *rect)).collect();
    assert_eq!(before.len(), 2);
    set_mode(&mut h, ScreenMode::FullScreenWithMenuBar);
    set_mode(&mut h, ScreenMode::FullScreen);
    h.state_mut().0.run("document.activate", json!({"offset": 1})).expect("the next document activates");
    h.run_steps(3);
    let last_active = app(&h).session.active_index();
    set_mode(&mut h, ScreenMode::Standard);
    let app = app(&h);
    let after: Vec<(u64, Rect)> = app.ui.document_windows.floating.iter().map(|(g, rect)| (g.id, *rect)).collect();
    assert_eq!(after, before, "every window is back at its old place and size");
    assert_eq!(windows_shown(&h).len(), 2, "both windows float again: {:?}", windows_shown(&h));
    assert_eq!(app.session.active_index(), last_active, "the document active last stays active");
    assert_eq!(single_canvas_document(app), None, "the main area gives the document back to its window");
}

/// Not measured: a mix of docked and floating documents keeps today's behaviour, the floating window over the full
/// screen and the docked document in the main area.
#[test]
fn a_mix_of_docked_and_floating_documents_keeps_its_windows() {
    let mut h = floating_pair();
    let big = index_of(app(&h), "big");
    let big_id = app(&h).session.documents()[big].doc.id;
    let window = app(&h).ui.document_windows.floating.iter().find(|(g, _)| g.documents.contains(&big_id)).map(|(g, _)| g.id).expect("big floats");
    // Docking the big document back: its window closes and it joins the tabs.
    h.state_mut().0.ui.document_windows.close_window(window);
    h.run_steps(2);
    set_mode(&mut h, ScreenMode::FullScreen);
    let app = app(&h);
    assert!(!full_screen_shows_active(app), "a docked document keeps the full-screen modes as they were");
    assert_eq!(windows_shown(&h).len(), 1, "the floating window still shows: {:?}", windows_shown(&h));
}
