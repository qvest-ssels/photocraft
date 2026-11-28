//! #14893: the Drop Shadow page's Distance reaches 30000 px, values the dialog sends are kept
//! inside the page's ranges by the command, and an untouched OK keeps what a file imported.

use craft::imaging::layer_style::{EffectKind, EffectSpec, FieldKind, StyleValue};
use photocraft_doc::Effect;
use serde_json::{Map, json};

use super::tests::{app_with_layer, edit, first_of, layer_effects, ok};
use super::{blend_keys, open, specs};
use crate::PhotocraftApp;

fn slider(spec: &EffectSpec, key: &str) -> (f64, f64) {
    match spec.fields.iter().find(|f| f.key == key).map(|f| &f.kind) {
        Some(FieldKind::Slider { min, max, .. }) => (*min, *max),
        other => panic!("{key} is not a slider: {other:?}"),
    }
}

fn shadow(app: &PhotocraftApp) -> photocraft_doc::effects::Shadow {
    match layer_effects(&app.session).as_slice() {
        [Effect::DropShadow(sh)] => sh.clone(),
        other => panic!("no drop shadow: {other:?}"),
    }
}

#[test]
fn the_drop_shadow_distance_slider_reaches_30000_px() {
    let specs = specs(&blend_keys(), &Map::new(), 120.0);
    let drop = specs.iter().find(|s| s.kind == EffectKind::DropShadow).unwrap();
    assert_eq!(slider(drop, "distance"), (0.0, 30_000.0));
    // The other rows keep the framework's table.
    let factory = EffectSpec::photoshop(EffectKind::DropShadow);
    for key in ["opacity", "spread", "size", "noise"] {
        assert_eq!(slider(drop, key), slider(&factory, key), "{key}");
    }
    // Inner Shadow is its own item; its page is unchanged here.
    let inner = specs.iter().find(|s| s.kind == EffectKind::InnerShadow).unwrap();
    assert_eq!(slider(inner, "distance"), slider(&EffectSpec::photoshop(EffectKind::InnerShadow), "distance"));
}

/// The dialog's values reach the layer through the command, which keeps them in the page's ranges:
/// an Angle of 200 with Use Global Light off is stored as -160, a Distance of 20000 as it is.
#[test]
fn ok_on_a_drop_shadow_stores_the_angle_wrapped_and_a_long_distance() {
    let mut app = app_with_layer(48);
    let id = open(&mut app, Some(EffectKind::DropShadow.key())).unwrap();
    let fx = first_of(&app, id, EffectKind::DropShadow);
    edit(&mut app, id, |d| {
        d.set_param(fx, "useGlobalLight", StyleValue::Bool(false));
        d.set_param(fx, "angle", StyleValue::Number(200.0));
        d.set_param(fx, "distance", StyleValue::Number(20_000.0));
    });
    ok(&mut app, id);
    let sh = shadow(&app);
    assert!((sh.angle + 160.0).abs() < 1e-4, "angle {}", sh.angle);
    assert_eq!(sh.distance, 20_000.0);
}

/// A file can carry a Drop Shadow outside the page's ranges; the page re-sends every field on OK.
/// An untouched field keeps what the file had (Size 300 becoming 250 would move pixels), while an
/// edited Distance past 30000 is still clamped.
#[test]
fn an_untouched_ok_keeps_imported_out_of_range_drop_shadow_values() {
    for typed in [None, Some(40_001.0)] {
        let mut app = app_with_layer(48);
        app.run("layer.layerStyle.dropShadow", json!({})).unwrap();
        app.session
            .edit("imported out-of-range values", |doc, active| {
                let Some(Effect::DropShadow(sh)) = doc.layer_mut(active.unwrap()).unwrap().effects.items.first_mut() else { panic!("drop shadow") };
                sh.distance = 40_000.0;
                sh.size = 300.0;
                sh.angle = 200.0;
                sh.use_global_light = false;
                Ok(())
            })
            .unwrap();
        let id = open(&mut app, Some(EffectKind::DropShadow.key())).unwrap();
        if let Some(distance) = typed {
            let fx = first_of(&app, id, EffectKind::DropShadow);
            edit(&mut app, id, |d| d.set_param(fx, "distance", StyleValue::Number(distance)));
        }
        ok(&mut app, id);
        let sh = shadow(&app);
        assert_eq!(sh.distance, typed.map_or(40_000.0, |_| 30_000.0), "distance {typed:?}");
        assert_eq!(sh.size, 300.0, "an untouched Size keeps what the file had");
        assert_eq!(sh.angle, 200.0, "an untouched Angle keeps what the file had");
    }
}
