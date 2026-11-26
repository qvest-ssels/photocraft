//! #16858: a dragged layer snaps to a target from up to 7 screen pixels away, not 8, at any zoom.
//!
//! The issue's document: 1000 x 800 px with pixel layers A (100,100 to 300,260), C (400,100 to 500,200) and B
//! (600,400 to 750,520, on top and active), the Move tool, View > Snap and Smart Guides on.

use serde_json::json;

use crate::canvas::testing::clear_of_dialogs;
use crate::canvas::{ToolEvent, tool_event};
use crate::state::Tool;
use crate::{PhotocraftApp, Services};

/// The issue's document at `zoom`, B active.
fn document(zoom: f32) -> Box<PhotocraftApp> {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    app.run("file.new", json!({"width": 1000, "height": 800})).unwrap();
    app.sync_views();
    app.ui.views[0].zoom = zoom;
    app.ui.views[0].clear_pending();
    let layers = [
        (photocraft_geom::Rect::new(100, 100, 300, 260), [1.0, 0.0, 0.0, 1.0]),
        (photocraft_geom::Rect::new(400, 100, 500, 200), [0.0, 1.0, 0.0, 1.0]),
        (photocraft_geom::Rect::new(600, 400, 750, 520), [0.0, 0.0, 1.0, 1.0]),
    ];
    for (rect, color) in layers {
        app.run("layer.new.layer", json!({})).unwrap();
        app.session
            .edit("paint", |doc, active| {
                doc.layer_mut(active.unwrap()).unwrap().surface_mut().unwrap().fill_rect(rect, &color);
                Ok(())
            })
            .unwrap();
    }
    app.ui.tool = Tool::Move;
    app.ui.tool_options.move_auto_select = false;
    app.ui.extras.snap = true;
    app.ui.view.show.smart_guides = true;
    app
}

/// Drags B by its body so its left edge would land on `left`, and returns where its left edge ends up.
fn drag_left_edge_to(zoom: f32, left: f64) -> i32 {
    let mut app = document(zoom);
    let (x, y) = (675.0, 460.0);
    let to = x + (left - 600.0);
    // The test canvas is narrower than the document: show the drag's whole span.
    clear_of_dialogs(&mut app, [to.min(x) - 10.0, y]);
    let m = egui::Modifiers::NONE;
    tool_event(&mut app, ToolEvent::Down { x, y, pressure: 1.0 }, m);
    tool_event(&mut app, ToolEvent::Move { x: to, y, pressure: 1.0 }, m);
    tool_event(&mut app, ToolEvent::Up { x: to, y }, m);
    let st = app.session.active().unwrap();
    st.doc.layer(st.active_layer.unwrap()).unwrap().surface().unwrap().content_bounds().x0
}

/// At 100 %, B's left edge 7 px from A's left edge (100) snaps to it from either side; 8 px stays where it was dropped.
#[test]
fn at_100_percent_7_screen_pixels_snap_and_8_do_not() {
    for (left, want) in [(107.0, 100), (93.0, 100), (108.0, 108), (92.0, 92)] {
        assert_eq!(drag_left_edge_to(1.0, left), want, "B's left edge dragged to {left}");
    }
}

/// At 200 % the pull distance is still in screen pixels: 3 document px (6 screen px) from C's left edge (400) snaps,
/// 4 document px (8 screen px) does not.
#[test]
fn at_200_percent_3_document_pixels_snap_and_4_do_not() {
    for (left, want) in [(403.0, 400), (397.0, 400), (404.0, 404), (396.0, 396)] {
        assert_eq!(drag_left_edge_to(2.0, left), want, "B's left edge dragged to {left}");
    }
}
