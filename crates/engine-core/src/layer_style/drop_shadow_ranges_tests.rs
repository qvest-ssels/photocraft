//! #14893: the Drop Shadow page's ranges on the command side. The page types Distance
//! 0..30000 px, Size 0..250 px and an Angle of -360..360; `layer.layerStyle.dropShadow` keeps
//! those values inside the page's ranges and wraps an Angle past half a turn into -180..180 when
//! it stores it. Values already inside keep today's params.

use super::*;
use crate::Session;
use serde_json::json;

fn session() -> Session {
    let mut s = crate::session();
    s.execute("file.new", json!({"width": 32, "height": 32, "background": "white"})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s
}

/// The one Drop Shadow the active layer carries after `params`.
fn drop_shadow(params: Value) -> Shadow {
    let mut s = session();
    s.execute("layer.layerStyle.dropShadow", params).unwrap();
    let d = s.active().unwrap();
    let l = d.doc.layer(d.active_layer.unwrap()).unwrap();
    match l.effects.items.as_slice() {
        [Effect::DropShadow(sh)] => sh.clone(),
        other => panic!("no drop shadow: {other:?}"),
    }
}

/// A typed Angle of 200 is stored as -160 and -200 as 160; past the field's ±360 the value is
/// clamped first (400 -> 360 -> 0).
#[test]
fn drop_shadow_wraps_an_angle_past_half_a_turn() {
    for (typed, stored) in [(200.0, -160.0), (-200.0, 160.0), (270.0, -90.0), (360.0, 0.0), (400.0, 0.0), (-400.0, 0.0)] {
        let angle = drop_shadow(json!({"angle": typed, "useGlobalLight": false})).angle;
        assert!((angle - stored).abs() < 1e-4, "angle {typed} stored as {angle}, not {stored}");
    }
}

/// Distance is kept inside 0..30000 px, a negative value at 0.
#[test]
fn drop_shadow_clamps_distance_to_the_page_range() {
    for (sent, kept) in [(40000.0, 30000.0), (30001.0, 30000.0), (-5.0, 0.0), (30000.0, 30000.0), (500.0, 500.0)] {
        assert_eq!(drop_shadow(json!({"distance": sent})).distance, kept, "distance {sent}");
    }
}

/// Size is kept inside 0..250 px, 0 included.
#[test]
fn drop_shadow_clamps_size_to_the_page_range() {
    for (sent, kept) in [(300.0, 250.0), (-3.0, 0.0), (0.0, 0.0), (250.0, 250.0)] {
        assert_eq!(drop_shadow(json!({"size": sent})).size, kept, "size {sent}");
    }
}

/// Values inside the ranges are stored exactly as given, so in-range params, and with them the
/// pixels, stay as they were. 180 stays 180 rather than becoming -180.
#[test]
fn in_range_drop_shadow_values_are_kept_exactly() {
    for (angle, distance, size) in [(0.0, 0.0, 0.0), (120.0, 5.0, 5.0), (-45.5, 10.0, 3.0), (180.0, 300.0, 250.0), (-180.0, 1234.5, 17.5)] {
        let sh = drop_shadow(json!({"angle": angle, "distance": distance, "size": size, "useGlobalLight": false}));
        assert_eq!((sh.angle, sh.distance, sh.size), (angle, distance, size));
    }
}

/// The shared light angle is not the effect's: a lit Drop Shadow sent Angle 200 still stores it
/// wrapped, and `layer.layerStyle.globalLight` keeps its own rule.
#[test]
fn drop_shadow_wrap_applies_with_the_global_light_on() {
    assert!((drop_shadow(json!({"angle": 200.0})).angle + 160.0).abs() < 1e-4);
}
