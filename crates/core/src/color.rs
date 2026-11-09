//! Colour maths: sRGB, linear light and CIELAB (D65), plus plain-language names for yarn colours.

pub type Lab = [f64; 3];

const WHITE: [f64; 3] = [0.950_47, 1.0, 1.088_83];

/// sRGB byte to linear light.
pub fn srgb_to_linear(v: u8) -> f64 {
    let c = f64::from(v) / 255.0;
    if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear light to an sRGB byte (clamped).
pub fn linear_to_srgb(v: f64) -> u8 {
    let v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let c = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

fn f(t: f64) -> f64 {
    const D: f64 = 6.0 / 29.0;
    if t > D * D * D { t.cbrt() } else { t / (3.0 * D * D) + 4.0 / 29.0 }
}

fn f_inv(t: f64) -> f64 {
    const D: f64 = 6.0 / 29.0;
    if t > D { t * t * t } else { 3.0 * D * D * (t - 4.0 / 29.0) }
}

/// Linear RGB (0..1) to CIELAB.
pub fn linear_to_lab(rgb: [f64; 3]) -> Lab {
    let [r, g, b] = rgb;
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    let (fx, fy, fz) = (f(x / WHITE[0]), f(y / WHITE[1]), f(z / WHITE[2]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// sRGB bytes to CIELAB.
pub fn srgb_to_lab(rgb: [u8; 3]) -> Lab {
    linear_to_lab([srgb_to_linear(rgb[0]), srgb_to_linear(rgb[1]), srgb_to_linear(rgb[2])])
}

/// CIELAB to sRGB bytes (clamped into gamut).
pub fn lab_to_srgb(lab: Lab) -> [u8; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let (fx, fz) = (fy + lab[1] / 500.0, fy - lab[2] / 200.0);
    let (x, y, z) = (f_inv(fx) * WHITE[0], f_inv(fy) * WHITE[1], f_inv(fz) * WHITE[2]);
    let r = 3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z;
    let g = -0.969_266_0 * x + 1.876_010_8 * y + 0.041_556_0 * z;
    let b = 0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z;
    [linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b)]
}

/// Squared colour distance (CIE76 delta-E squared).
pub fn delta_e_sq(a: Lab, b: Lab) -> f64 {
    let (dl, da, db) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    dl * dl + da * da + db * db
}

/// A plain generic colour name ("dark red", "light gray"); never a brand name.
pub fn generic_name(rgb: [u8; 3]) -> String {
    let (r, g, b) = (f64::from(rgb[0]) / 255.0, f64::from(rgb[1]) / 255.0, f64::from(rgb[2]) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    if s < 0.15 {
        return match l {
            l if l < 0.15 => "black",
            l if l < 0.40 => "dark gray",
            l if l < 0.70 => "gray",
            l if l < 0.90 => "light gray",
            _ => "white",
        }
        .to_string();
    }
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let hue = match h {
        h if !(15.0..345.0).contains(&h) => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 160.0 => "green",
        h if h < 195.0 => "teal",
        h if h < 255.0 => "blue",
        h if h < 290.0 => "purple",
        _ => "pink",
    };
    let hue = if hue == "orange" && l < 0.35 { "brown" } else { hue };
    match l {
        l if l < 0.28 => format!("dark {hue}"),
        l if l > 0.72 => format!("light {hue}"),
        _ => hue.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_and_black_have_the_expected_lab() {
        let w = srgb_to_lab([255, 255, 255]);
        assert!((w[0] - 100.0).abs() < 0.05 && w[1].abs() < 0.05 && w[2].abs() < 0.05, "{w:?}");
        let k = srgb_to_lab([0, 0, 0]);
        assert!(k[0].abs() < 0.01);
    }

    #[test]
    fn lab_round_trips_every_grey_and_some_colours() {
        for v in 0..=255u8 {
            assert_eq!(lab_to_srgb(srgb_to_lab([v, v, v])), [v, v, v]);
        }
        for c in [[200, 30, 40], [10, 120, 220], [250, 244, 230], [34, 139, 34]] {
            assert_eq!(lab_to_srgb(srgb_to_lab(c)), c);
        }
    }

    #[test]
    fn names_are_generic() {
        assert_eq!(generic_name([0, 0, 0]), "black");
        assert_eq!(generic_name([255, 255, 255]), "white");
        assert_eq!(generic_name([200, 30, 40]), "red");
        assert_eq!(generic_name([10, 20, 120]), "dark blue");
        assert_eq!(generic_name([128, 128, 128]), "gray");
    }

    #[test]
    fn bad_floats_do_not_panic() {
        assert_eq!(linear_to_srgb(f64::NAN), 0);
        let _ = lab_to_srgb([f64::NAN, f64::INFINITY, -1e300]);
    }
}
