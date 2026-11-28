//! A layer mask thumbnail's right-click menu (#16782): the reference's entries and groups, Disable
//! Layer Mask ― Delete Layer Mask · Apply Layer Mask ― the three selection loads ― Select and Mask… ·
//! Enhance edge ― Mask Options…, with Apply and Enhance edge greyed on an adjustment layer's mask.

use craft::layers::ui::panel::{LayersEvent, MenuTarget};
use craft::layers::ui::tree::{LayerPart, LayerRowKey};
use photocraft_doc::LayerId;
use photocraft_engine::Session;
use serde_json::json;

use crate::PhotocraftApp;
use crate::panels::layers_tree::{self, PhotoLayers, row_menu};

/// A pixel layer with a layer mask, active.
fn setup() -> (Box<PhotocraftApp>, LayerId) {
    let mut app = PhotocraftApp::new(Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 200, "height": 100, "background": "white"})).unwrap();
    app.run("layer.new.layer", json!({"name": "Masked"})).unwrap();
    app.run("select.rect", json!({"x": 20, "y": 10, "width": 60, "height": 40})).unwrap();
    app.run("edit.fill", json!({"contents": "color", "color": "#336699"})).unwrap();
    app.run("layer.layerMask.revealSelection", json!({})).unwrap();
    assert!(app.session.active().unwrap().doc.selection.is_none(), "the selection became the mask");
    let masked = active_id(&app);
    (app, masked)
}

fn active_id(app: &PhotocraftApp) -> LayerId {
    app.session.active().unwrap().active_layer.unwrap()
}

fn target(id: LayerId) -> MenuTarget<LayerId> {
    let mut t = MenuTarget::new(Some(LayerRowKey::Layer(id)));
    t.part = Some(LayerPart::LayerMask);
    t
}

/// The menu as (label, enabled) rows, `None` for a separator.
fn menu(app: &PhotocraftApp, id: LayerId) -> Vec<Option<(String, bool)>> {
    let st = app.session.active().unwrap();
    let tree = PhotoLayers::new(app, st, &Default::default(), false, None);
    row_menu(app, &tree, &target(id)).iter().map(|e| e.value().map(|_| (e.label().as_str().to_string(), e.is_enabled()))).collect()
}

/// The reference's rows, every one enabled except those named in `greyed`.
fn expected(first: &str, greyed: &[&str]) -> Vec<Option<(String, bool)>> {
    let rows: [Option<&str>; 13] = [
        Some(first),
        None,
        Some("Delete Layer Mask"),
        Some("Apply Layer Mask"),
        None,
        Some("Add Mask to Selection"),
        Some("Subtract Mask from Selection"),
        Some("Intersect Mask with Selection"),
        None,
        Some("Select and Mask…"),
        Some("Enhance edge"),
        None,
        Some("Mask Options…"),
    ];
    rows.iter().map(|r| r.map(|label| (label.to_string(), !greyed.contains(&label)))).collect()
}

#[test]
fn a_pixel_layers_mask_menu_has_the_reference_entries_and_groups() {
    let (mut app, masked) = setup();
    assert_eq!(menu(&app, masked), expected("Disable Layer Mask", &[]), "without a selection");
    app.run("select.rect", json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap();
    assert_eq!(menu(&app, masked), expected("Disable Layer Mask", &[]), "with a selection");
}

#[test]
fn a_disabled_mask_reads_enable_layer_mask_first() {
    let (mut app, masked) = setup();
    app.run("layer.layerMask.enabled", json!({"layer": masked.0, "enabled": false})).unwrap();
    assert_eq!(menu(&app, masked), expected("Enable Layer Mask", &[]));
}

#[test]
fn an_adjustment_layers_mask_greys_apply_and_enhance_edge() {
    let (mut app, _) = setup();
    app.run("layer.newAdjustmentLayer.invert", json!({})).unwrap();
    let adjustment = active_id(&app);
    assert!(app.session.active().unwrap().doc.layer(adjustment).unwrap().mask.is_some(), "an adjustment layer comes with a mask");
    assert_eq!(menu(&app, adjustment), expected("Disable Layer Mask", &["Apply Layer Mask", "Enhance edge"]));
}

/// Enhance edge runs what Select and Mask… runs on the mask: the task bar's own Enhance edge (#16753) is
/// not there yet.
#[test]
fn enhance_edge_opens_select_and_mask_on_the_mask() {
    let run = |command: &str| {
        let (mut app, masked) = setup();
        let ctx = app.ctx().clone();
        let event = LayersEvent::menu(target(masked), command);
        crate::panels::layers_events::apply_layers_events(&mut app, &ctx, vec![event]);
        (app.ui.mask_target, crate::distort_ui::workspace_fields(&app, "select.selectAndMask"), app.session.active().unwrap().history.entries().to_vec())
    };
    let enhance = run(layers_tree::MASK_ENHANCE);
    assert!(enhance.0, "the mask is targeted");
    assert!(enhance.1.is_some(), "Select and Mask is open");
    assert_eq!(enhance, run(layers_tree::MASK_REFINE), "the same as Select and Mask…");
}
