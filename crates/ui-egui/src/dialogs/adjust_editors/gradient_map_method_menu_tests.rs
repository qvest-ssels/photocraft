//! Gradient Map's Method list (#14687): the params to the editor's value and back, and what each method renders.

use craft::draw::ui::gradient::GradientMethod;
use photocraft_doc::ColorMode;
use photocraft_engine::Session;
use photocraft_geom::Rect;
use serde_json::{Value, json};

use super::AdjustValue;
use super::values::{ValueCx, from_params, to_params};

fn map_of(params: &Value) -> craft::draw::ui::gradient::GradientMap {
    match from_params("gradientMap", params, &ValueCx::new(ColorMode::Rgb, params.clone())) {
        Ok(AdjustValue::GradientMap { value }) => value,
        other => panic!("a Gradient Map value, got {other:?}"),
    }
}

/// The params OK runs for `params` opened in the dialog with `method` picked.
fn picked(params: &Value, method: GradientMethod) -> Value {
    let value = AdjustValue::GradientMap { value: map_of(params).with_method(method) };
    to_params(&value, &ValueCx::new(ColorMode::Rgb, params.clone()), true)
}

/// Gray 16 / 128 / 240 mapped black -> white with `params`, as 8-bit levels.
fn mapped(params: &Value) -> [u8; 3] {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 3, "height": 1, "mode": "rgb", "depth": 8})).unwrap();
    s.edit("Gray steps", |doc, _| {
        let surface = doc.layers[0].surface_mut().unwrap();
        for (x, level) in [16.0f32, 128.0, 240.0].into_iter().enumerate() {
            let v = level / 255.0;
            surface.fill_rect(Rect::new(x as i32, 0, x as i32 + 1, 1), &photocraft_raster::from_rgba(&surface.format(), [v, v, v, 1.0]));
        }
        Ok(())
    })
    .unwrap();
    s.execute("image.adjustments.gradientMap", params.clone()).unwrap();
    let st = s.active().unwrap();
    let surface = st.doc.layer(st.active_layer.unwrap()).unwrap().surface().unwrap();
    let mut px = [0.0f32; 4];
    let mut out = [0u8; 3];
    for (x, o) in out.iter_mut().enumerate() {
        surface.read_pixel(x as i32, 0, &mut px);
        *o = (px[0] * 255.0).round() as u8;
    }
    out
}

/// A new map (the engine's neutral params, which Reset also opens on) shows Smooth.
#[test]
fn a_map_without_a_method_opens_on_smooth() {
    let neutral = photocraft_engine::adjust_params::to_params(&photocraft_engine::adjust_params::default_for("gradientMap", ColorMode::Rgb).unwrap());
    assert_eq!(map_of(&neutral).method, GradientMethod::Smooth);
}

/// A gradient kept from a PSD shows its own method, and a pick writes only the method over it: its smoothness stays.
#[test]
fn a_kept_gradient_shows_its_method_and_keeps_its_smoothness() {
    let kept = json!({"source": {"stops": [[0, "#000000"], [1, "#ffffff"]], "midpoints": [0.5], "smoothness": 0.25, "method": "linear"}});
    assert_eq!(map_of(&kept).method, GradientMethod::Linear);
    let params = picked(&kept, GradientMethod::Classic);
    assert_eq!(params["source"]["method"], "classic");
    assert_eq!(params["source"]["smoothness"], 0.25);
    assert_eq!(map_of(&params).method, GradientMethod::Classic, "the params read back as the pick");
}

/// Each method renders the measured levels within one (black -> white at Smoothness 100 %, gray 16 / 128 / 240).
#[test]
fn each_method_renders_the_measured_levels() {
    let plain = json!({"stops": [[0, "#000000"], [1, "#ffffff"]]});
    let measured = [
        (GradientMethod::Perceptual, [0, 100, 243]),
        (GradientMethod::Linear, [54, 188, 251]),
        (GradientMethod::Classic, [9, 128, 246]),
        (GradientMethod::Smooth, [0, 100, 241]),
    ];
    for (method, want) in measured {
        let got = mapped(&picked(&plain, method));
        assert!(got.iter().zip(want).all(|(g, w)| (i32::from(*g) - w).abs() <= 1), "{method:?}: got {got:?}, measured {want:?}");
    }
}
