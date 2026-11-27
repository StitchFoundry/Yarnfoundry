//! Gauge math: stitches and rows per measure, and the grid size for a target size.

use serde::{Deserialize, Serialize};

/// Largest grid side we ever produce, so hostile numbers cannot ask for gigabytes.
pub const MAX_SIDE: u32 = 2000;

const INCH_PER_CM: f64 = 1.0 / 2.54;

/// The unit a size or gauge measure is given in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Inches,
    Cm,
}

impl Unit {
    fn to_inches(self, value: f64) -> f64 {
        match self {
            Unit::Inches => value,
            Unit::Cm => value * INCH_PER_CM,
        }
    }
}

/// Why a gauge or size was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GaugeError {
    #[error("gauge numbers must be finite and greater than zero")]
    InvalidGauge,
    #[error("size must be finite and greater than zero")]
    InvalidSize,
    #[error("the chart would be larger than {MAX_SIDE} stitches or rows on a side")]
    TooLarge,
}

/// `stitches` and `rows` counted over a `measure` of `unit` (e.g. 20 sts / 28 rows per 4 in).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gauge {
    pub stitches: f64,
    pub rows: f64,
    pub measure: f64,
    pub unit: Unit,
}

impl Gauge {
    /// A validated gauge.
    pub fn new(stitches: f64, rows: f64, measure: f64, unit: Unit) -> Result<Self, GaugeError> {
        let g = Self { stitches, rows, measure, unit };
        g.validate()?;
        Ok(g)
    }

    /// Checks every number is finite and positive (files and user input are untrusted).
    pub fn validate(&self) -> Result<(), GaugeError> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if ok(self.stitches) && ok(self.rows) && ok(self.measure) { Ok(()) } else { Err(GaugeError::InvalidGauge) }
    }

    /// Stitches per inch.
    pub fn stitches_per_inch(&self) -> f64 {
        self.stitches / self.unit.to_inches(self.measure)
    }

    /// Rows per inch.
    pub fn rows_per_inch(&self) -> f64 {
        self.rows / self.unit.to_inches(self.measure)
    }

    /// Width over height of one stitch (so a picture is not stretched when resampled).
    pub fn stitch_aspect(&self) -> f64 {
        self.rows / self.stitches
    }

    /// Grid size (stitches, rows) for a finished width x height, rounded to the nearest whole stitch/row, at least 1.
    pub fn grid_for(&self, width: f64, height: f64, unit: Unit) -> Result<(u32, u32), GaugeError> {
        self.validate()?;
        if !(width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0) {
            return Err(GaugeError::InvalidSize);
        }
        let w = (unit.to_inches(width) * self.stitches_per_inch()).round().max(1.0);
        let h = (unit.to_inches(height) * self.rows_per_inch()).round().max(1.0);
        if w > f64::from(MAX_SIDE) || h > f64::from(MAX_SIDE) {
            return Err(GaugeError::TooLarge);
        }
        Ok((w as u32, h as u32))
    }

    /// Finished size (width, height) in `unit` of a grid of `stitches` x `rows`.
    pub fn size_of(&self, stitches: u32, rows: u32, unit: Unit) -> (f64, f64) {
        let to_unit = |inches: f64| match unit {
            Unit::Inches => inches,
            Unit::Cm => inches * 2.54,
        };
        (to_unit(f64::from(stitches) / self.stitches_per_inch()), to_unit(f64::from(rows) / self.rows_per_inch()))
    }
}

/// Stitch and row counts re-worked from a pattern's gauge to a maker's own gauge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adjusted {
    /// Stitches before rounding to the repeat.
    pub exact_stitches: f64,
    /// Stitches rounded to the nearest whole repeat (at least one repeat).
    pub stitches: u32,
    /// Rows, rounded to the nearest whole row (at least 1).
    pub rows: u32,
    /// True when rounding to the repeat moved the count by more than one stitch.
    pub rounding_warning: bool,
}

/// Counts to work so a piece comes out the pattern's size at `own` gauge.
/// `stitches` and `rows` are the pattern's counts at `pattern` gauge; `repeat` is the stitch repeat (0 is treated as 1).
pub fn adjust_counts(pattern: &Gauge, own: &Gauge, stitches: u32, rows: u32, repeat: u32) -> Result<Adjusted, GaugeError> {
    pattern.validate()?;
    own.validate()?;
    let repeat = repeat.max(1);
    let exact = f64::from(stitches) * own.stitches_per_inch() / pattern.stitches_per_inch();
    let exact_rows = f64::from(rows) * own.rows_per_inch() / pattern.rows_per_inch();
    let r = f64::from(repeat);
    let rounded = ((exact / r).round() * r).max(r);
    let rows = exact_rows.round().max(1.0);
    if rounded > f64::from(MAX_SIDE) || rows > f64::from(MAX_SIDE) {
        return Err(GaugeError::TooLarge);
    }
    Ok(Adjusted { exact_stitches: exact, stitches: rounded as u32, rows: rows as u32, rounding_warning: (rounded - exact).abs() > 1.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looser_own_gauge_means_fewer_counts_rounded_to_the_repeat() {
        let pat = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let own = Gauge::new(18.0, 26.0, 4.0, Unit::Inches).unwrap();
        // 80 sts * 18/20 = 72 exactly; 56 rows * 26/28 = 52.
        let a = adjust_counts(&pat, &own, 80, 56, 12).unwrap();
        assert_eq!((a.stitches, a.rows), (72, 52));
        assert!(!a.rounding_warning);
        // 85 sts -> 76.5 exact, nearest repeat of 12 is 72: off by 4.5, so warn.
        let b = adjust_counts(&pat, &own, 85, 56, 12).unwrap();
        assert_eq!(b.stitches, 72);
        assert!(b.rounding_warning);
    }

    #[test]
    fn units_mix_and_bad_input_is_an_error() {
        let pat = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let own = Gauge::new(20.0, 28.0, 10.16, Unit::Cm).unwrap();
        assert_eq!(adjust_counts(&pat, &own, 40, 56, 0).map(|a| (a.stitches, a.rows)), Ok((40, 56)));
        let bad = Gauge { stitches: f64::NAN, ..own };
        assert_eq!(adjust_counts(&pat, &bad, 40, 56, 1), Err(GaugeError::InvalidGauge));
        assert_eq!(adjust_counts(&pat, &own, 40, 56, 1_000_000), Err(GaugeError::TooLarge));
    }

    #[test]
    fn gauge_20_by_28_per_4_inches_for_8_by_8_inches_is_40_by_56() {
        let g = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        assert_eq!(g.grid_for(8.0, 8.0, Unit::Inches), Ok((40, 56)));
    }

    #[test]
    fn centimetres_and_inches_agree() {
        let inch = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let cm = Gauge::new(20.0, 28.0, 10.16, Unit::Cm).unwrap();
        assert_eq!(cm.grid_for(20.32, 20.32, Unit::Cm), Ok((40, 56)));
        assert_eq!(inch.grid_for(20.32, 20.32, Unit::Cm), Ok((40, 56)));
        assert_eq!(cm.grid_for(8.0, 8.0, Unit::Inches), Ok((40, 56)));
    }

    #[test]
    fn stitches_are_not_square_and_the_aspect_says_so() {
        let g = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        assert!((g.stitch_aspect() - 1.4).abs() < 1e-9);
    }

    #[test]
    fn size_of_inverts_grid_for() {
        let g = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let (w, h) = g.size_of(40, 56, Unit::Inches);
        assert!((w - 8.0).abs() < 1e-9 && (h - 8.0).abs() < 1e-9);
        let (wc, _) = g.size_of(40, 56, Unit::Cm);
        assert!((wc - 20.32).abs() < 1e-9);
    }

    #[test]
    fn bad_numbers_are_errors_not_panics() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(Gauge::new(bad, 28.0, 4.0, Unit::Inches), Err(GaugeError::InvalidGauge));
            assert_eq!(Gauge::new(20.0, bad, 4.0, Unit::Inches), Err(GaugeError::InvalidGauge));
            assert_eq!(Gauge::new(20.0, 28.0, bad, Unit::Inches), Err(GaugeError::InvalidGauge));
        }
        let g = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        assert_eq!(g.grid_for(f64::NAN, 8.0, Unit::Inches), Err(GaugeError::InvalidSize));
        assert_eq!(g.grid_for(8.0, -2.0, Unit::Inches), Err(GaugeError::InvalidSize));
        assert_eq!(g.grid_for(1e9, 8.0, Unit::Inches), Err(GaugeError::TooLarge));
    }

    #[test]
    fn tiny_sizes_give_at_least_one_stitch() {
        let g = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        assert_eq!(g.grid_for(0.001, 0.001, Unit::Inches), Ok((1, 1)));
    }
}
