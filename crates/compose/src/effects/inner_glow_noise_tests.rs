//! #14926 - BUG-LAYER-STYLE-INNER-GLOW-NOISE-TOO-WEAK.
//!
//! Measured on the issue's test document: 400 x 300 px, RGB 8 bit, white Background, an orange
//! (255, 128, 0) pixel layer covering x 100..299, y 75..224. Inner Glow black, Normal, 100 %,
//! Softer, Source Edge, Choke 0, Size 10, Contour Linear, Range 50 %. The issue counts the G
//! channel of the flattened top band y 75..83, x 110..289 (1620 px): G 128 is the layer's own
//! orange (no glow), G 0 is full black glow. The shared `noise` law (#14936, #14952) leaves 244
//! band pixels without glow at Noise 50, while the reference application shows 343.

use super::*;

const DOC_W: usize = 400;
const DOC_H: usize = 300;

/// The layer's alpha over the whole document: solid over x 100..=299, y 75..=224.
fn document_shape() -> Map {
    let mut m = Map::new(DOC_W, DOC_H, 0.0);
    for y in 75..=224 {
        for x in 100..=299 {
            m.v[y * DOC_W + x] = 1.0;
        }
    }
    m
}

/// The issue's Inner Glow settings: black, Normal, 100 %, Softer, Source Edge, Choke 0, Size 10,
/// Contour Linear, Range 50 %, Jitter 0, Anti-aliased off, Noise `noise`.
fn glow(noise: f32) -> Glow {
    Glow {
        common: photocraft_doc::FxCommon::new(BlendMode::Normal, 1.0),
        paint: FxPaint::Color(photocraft_color::Color::BLACK),
        technique: GlowTechnique::Softer,
        spread: 0.0,
        size: 10.0,
        contour: Contour::Linear,
        anti_alias: false,
        range: 0.5,
        jitter: 0.0,
        noise,
        source: GlowSource::Edge,
    }
}

fn map(noise: f32) -> Map {
    glow_map(&document_shape(), &glow(noise), true, (0, 0))
}

/// The G channel of the orange layer under a black Normal inner glow of coverage `c`.
fn g_u8(c: f32) -> u8 {
    (128.0 * (1.0 - c)).round().clamp(0.0, 128.0) as u8
}

/// The issue's band, y 75..=83 and x 110..=289 (1620 px), as G channel values.
fn band(m: &Map) -> Vec<u8> {
    let mut out = Vec::with_capacity(180 * 9);
    for y in 75..=83 {
        for x in 110..=289 {
            out.push(g_u8(m.get(x, y)));
        }
    }
    out
}

/// Done when: Noise 50 on that band leaves 260..430 pixels at G 128 (no glow at all), at most
/// 30 pixels equal to the noiseless result, and reaches G 0. The shared law alone leaves 244.
#[test]
fn noise_fifty_scatters_the_glow_across_the_band() {
    let plain = band(&map(0.0));
    let noisy = band(&map(0.5));
    let gone = noisy.iter().filter(|v| **v == 128).count();
    let equal = noisy.iter().zip(&plain).filter(|(a, b)| a == b).count();
    let min = noisy.iter().copied().min().unwrap_or(128);
    assert!((260..=430).contains(&gone), "Noise 50: {gone} pixels without glow, want 260..430 (reference: 343)");
    assert!(equal <= 30, "Noise 50: {equal} pixels equal to the noiseless result, want at most 30 (reference: 8)");
    assert_eq!(min, 0, "Noise 50: minimum G {min}, want 0");
}

/// Done when: Noise 100 leaves 400..580 pixels at G 128 (reference: 490).
#[test]
fn noise_hundred_thins_the_glow_further() {
    let noisy = band(&map(1.0));
    let gone = noisy.iter().filter(|v| **v == 128).count();
    assert!((400..=580).contains(&gone), "Noise 100: {gone} pixels without glow, want 400..580 (reference: 490)");
}

/// Done when: the interior beyond the glow's reach stays G 128 at every Noise.
#[test]
fn noise_keeps_the_interior_beyond_the_glow_clean() {
    let plain = map(0.0);
    for noise in [0.25f32, 0.5, 1.0] {
        let noisy = map(noise);
        for y in (100..=200).step_by(5) {
            for x in (130..=270).step_by(7) {
                assert_eq!(plain.get(x, y), 0.0, "noiseless coverage at ({x},{y}) is not empty");
                assert_eq!(noisy.get(x, y), 0.0, "noise {noise} lit up ({x},{y}): {}", noisy.get(x, y));
                assert_eq!(g_u8(noisy.get(x, y)), 128, "noise {noise} moved the interior at ({x},{y}) off 128");
            }
        }
    }
}

/// Done when: the same settings give the identical image twice.
#[test]
fn the_same_noise_renders_identically_twice() {
    for noise in [0.5f32, 1.0] {
        let a = map(noise);
        let b = map(noise);
        assert_eq!(a.v, b.v, "noise {noise} is not deterministic");
    }
}

/// Noise that is not a positive number leaves the glow exactly as Noise 0 does, and the inner
/// strength does not reach the Outer Glow, whose band was measured on the shared law (#14952).
#[test]
fn odd_noise_values_and_outer_glows_keep_the_shared_law() {
    let plain = map(0.0);
    for odd in [-0.5f32, f32::NAN, f32::INFINITY] {
        assert_eq!(map(odd).v, plain.v, "noise {odd} changed the inner glow");
    }
    let shape = document_shape();
    let outer = glow_map(&shape, &glow(0.0), false, (0, 0));
    let mut by_law = outer.clone();
    noise(&mut by_law, 0.5, 0, 0);
    assert_eq!(glow_map(&shape, &glow(0.5), false, (0, 0)).v, by_law.v, "the outer glow left the shared law");
}

/// A hash over every coverage value of a map: the map is bit-for-bit unchanged iff this is.
fn bit_hash(m: &Map) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in &m.v {
        h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Noise 0 stays bit-identical: the whole-map FNV hash `glow_map` gives for the issue's
/// document with Noise 0 on origin/main.
#[test]
fn noise_zero_maps_stay_bit_identical() {
    assert_eq!(bit_hash(&map(0.0)), 0xefad_fffb_8195_d0b9, "the Noise 0 inner glow map moved");
}
