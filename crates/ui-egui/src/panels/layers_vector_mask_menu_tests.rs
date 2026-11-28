//! A vector mask thumbnail's right-click menu (#16781): Disable Vector Mask ― Delete Vector Mask ·
//! Rasterize Vector Mask, as in the reference application; a disabled mask reads Enable Vector Mask
//! and greys Rasterize. Each entry acts on the clicked layer in one history step.

use craft::layers::ui::panel::{LayersEvent, MenuTarget};
use craft::layers::ui::tree::{LayerPart, LayerRowKey};
use photocraft_doc::{Layer, LayerId};
use photocraft_engine::Session;
use serde_json::json;

use super::{PhotoLayers, VECTOR_MASK, row_menu};
use crate::PhotocraftApp;

/// A pixel layer "Masked" with a vector mask below a plain layer "Top", which is active.
fn setup() -> (Box<PhotocraftApp>, LayerId) {
    let mut app = PhotocraftApp::new(Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 200, "height": 100, "background": "white"})).unwrap();
    app.run("layer.new.layer", json!({"name": "Masked"})).unwrap();
    app.run("select.rect", json!({"x": 20, "y": 10, "width": 60, "height": 40})).unwrap();
    app.run("edit.fill", json!({"contents": "color", "color": "#336699"})).unwrap();
    app.run("select.deselect", json!({})).unwrap();
    app.run("layer.vectorMask.revealAll", json!({})).unwrap();
    let masked = active_id(&app);
    app.run("layer.new.layer", json!({"name": "Top"})).unwrap();
    (app, masked)
}

fn active_id(app: &PhotocraftApp) -> LayerId {
    app.session.active().unwrap().active_layer.unwrap()
}

fn layer(app: &PhotocraftApp, id: LayerId) -> Layer {
    app.session.active().unwrap().doc.layer(id).unwrap().clone()
}

fn steps(app: &PhotocraftApp) -> Vec<String> {
    app.session.active().unwrap().history.entries().to_vec()
}

fn target(id: LayerId) -> MenuTarget<LayerId> {
    let mut t = MenuTarget::new(Some(LayerRowKey::Layer(id)));
    t.part = Some(LayerPart::VectorMask);
    t
}

/// The menu as (command, label, enabled) rows, `None` for a separator.
fn menu(app: &PhotocraftApp, id: LayerId) -> Vec<Option<(String, String, bool)>> {
    let st = app.session.active().unwrap();
    let tree = PhotoLayers::new(app, st, &Default::default(), false, None);
    row_menu(app, &tree, &target(id))
        .iter()
        .map(|e| e.value().map(|v| (v.strip_prefix(VECTOR_MASK).unwrap_or(v).to_string(), e.label().as_str().to_string(), e.is_enabled())))
        .collect()
}

fn row(command: &str, label: &str, enabled: bool) -> Option<(String, String, bool)> {
    Some((command.to_string(), label.to_string(), enabled))
}

fn pick(app: &mut PhotocraftApp, id: LayerId, command: &str) {
    let ctx = app.ctx().clone();
    let event = LayersEvent::menu(target(id), format!("{VECTOR_MASK}{command}"));
    crate::panels::layers_events::apply_layers_events(app, &ctx, vec![event]);
}

#[test]
fn an_enabled_vector_mask_offers_disable_delete_and_rasterize() {
    let (app, masked) = setup();
    assert_eq!(
        menu(&app, masked),
        [
            row("layer.vectorMask.enabled", "Disable Vector Mask", true),
            None,
            row("layer.vectorMask.delete", "Delete Vector Mask", true),
            row("layer.rasterize.vectorMask", "Rasterize Vector Mask", true),
        ]
    );
}

#[test]
fn a_disabled_vector_mask_reads_enable_and_greys_rasterize() {
    let (mut app, masked) = setup();
    app.run("layer.vectorMask.enabled", json!({"layer": masked.0, "enabled": false})).unwrap();
    assert_eq!(
        menu(&app, masked),
        [
            row("layer.vectorMask.enabled", "Enable Vector Mask", true),
            None,
            row("layer.vectorMask.delete", "Delete Vector Mask", true),
            row("layer.rasterize.vectorMask", "Rasterize Vector Mask", false),
        ]
    );
}

#[test]
fn the_toggle_acts_on_the_clicked_layer_in_one_named_step() {
    let (mut app, masked) = setup();
    let top = active_id(&app);
    let before = steps(&app).len();
    pick(&mut app, masked, "layer.vectorMask.enabled");
    assert!(!layer(&app, masked).vector_mask.unwrap().enabled, "the clicked layer's mask is disabled");
    assert_eq!(active_id(&app), top, "the pick itself selects nothing");
    let after = steps(&app);
    assert_eq!(after.len(), before + 1);
    assert_eq!(after.last().map(String::as_str), Some("Disable Vector Mask"));
    pick(&mut app, masked, "layer.vectorMask.enabled");
    assert!(layer(&app, masked).vector_mask.unwrap().enabled, "a second pick enables it again");
    assert_eq!(steps(&app).last().map(String::as_str), Some("Enable Vector Mask"));
}

#[test]
fn delete_and_rasterize_act_on_the_clicked_layer_in_one_step_each() {
    let (mut app, masked) = setup();
    let before = steps(&app).len();
    pick(&mut app, masked, "layer.rasterize.vectorMask");
    let l = layer(&app, masked);
    assert!(l.vector_mask.is_none() && l.mask.is_some(), "rasterized into a layer mask");
    assert_eq!(steps(&app).len(), before + 1);

    let (mut app, masked) = setup();
    let before = steps(&app).len();
    pick(&mut app, masked, "layer.vectorMask.delete");
    let l = layer(&app, masked);
    assert!(l.vector_mask.is_none() && l.mask.is_none(), "deleted, nothing rasterized");
    assert_eq!(steps(&app).len(), before + 1);
}

/// The issue did not measure a vector mask on a group or an adjustment layer: they keep no menu.
#[test]
fn unmeasured_layer_kinds_keep_no_menu() {
    let (mut app, _) = setup();
    app.run("layer.groupLayers", json!({"name": "G"})).unwrap();
    let group = active_id(&app);
    app.run("layer.vectorMask.revealAll", json!({"layer": group.0})).unwrap();
    assert!(layer(&app, group).vector_mask.is_some());
    assert!(menu(&app, group).is_empty(), "a group's vector mask");

    app.run("layer.newAdjustmentLayer.invert", json!({})).unwrap();
    let adjustment = active_id(&app);
    app.run("layer.vectorMask.revealAll", json!({"layer": adjustment.0})).unwrap();
    assert!(layer(&app, adjustment).vector_mask.is_some());
    assert!(menu(&app, adjustment).is_empty(), "an adjustment layer's vector mask");
}

/// A pick on a layer whose vector mask is gone (or a layer that is gone) changes nothing and does not panic.
#[test]
fn a_stale_pick_changes_nothing() {
    let (mut app, masked) = setup();
    app.run("layer.vectorMask.delete", json!({"layer": masked.0})).unwrap();
    assert!(menu(&app, masked).is_empty(), "no vector mask, no menu");
    let before = steps(&app);
    pick(&mut app, masked, "layer.vectorMask.enabled");
    pick(&mut app, LayerId(9999), "layer.vectorMask.enabled");
    assert_eq!(steps(&app), before);
}
