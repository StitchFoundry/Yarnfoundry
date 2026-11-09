//! Words to chart: rasterise a line of text with one bundled OFL font (Liberation Sans Bold, see ASSETS.md) onto the
//! stitch grid. Stitches are not square, so the font is scaled with separate horizontal and vertical factors.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};

use crate::chart::{Chart, ChartError, Construction, Craft, PaletteColor, Technique};
use crate::gauge::{Gauge, MAX_SIDE};

static FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");

/// Longest text we rasterise (characters).
pub const MAX_CHARS: usize = 200;
/// Allowed letter height, in rows.
pub const MIN_ROWS: u32 = 3;
pub const MAX_ROWS: u32 = 400;

/// Why text could not be turned into a chart.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    #[error("type some text first")]
    Empty,
    #[error("the text is longer than {MAX_CHARS} characters")]
    TooLong,
    #[error("letter height must be between {MIN_ROWS} and {MAX_ROWS} rows")]
    BadHeight,
    #[error("the text needs {needed} stitches but the chart is {width} wide")]
    TooWide { needed: u32, width: u32 },
    #[error("the text would be wider than {MAX_SIDE} stitches")]
    TooLarge,
    #[error("the bundled font could not be read")]
    Font,
    #[error("chart: {0}")]
    Chart(#[from] ChartError),
}

/// What to write and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextOptions {
    pub text: String,
    /// Font size: the height of the letters' line in rows (capitals fill most of it).
    pub rows_tall: u32,
    /// Extra weight: thickens strokes.
    pub bold: bool,
    /// Hollow letters: only the edge stitches are kept.
    pub outline: bool,
    /// Extra stitches between letters (may be negative, -3 to 20).
    pub letter_spacing: i32,
    /// Empty stitches around the text.
    pub margin_cols: u32,
    pub margin_rows: u32,
    /// Chart width in stitches; the text is centred in it. `None` = just as wide as the text plus margins.
    pub width: Option<u32>,
}

impl TextOptions {
    /// Sensible defaults for `text`: 12 rows tall, 2 stitches of margin.
    pub fn new(text: &str) -> Self {
        Self { text: text.to_string(), rows_tall: 12, bold: false, outline: false, letter_spacing: 1, margin_cols: 2, margin_rows: 2, width: None }
    }
}

/// An on/off picture of the text, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMask {
    pub width: u32,
    pub height: u32,
    pub on: Vec<bool>,
}

impl TextMask {
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.on.get((y * self.width + x) as usize).copied().unwrap_or(false)
    }

    /// The mask as lines of `#` and `.`, for tests and logs.
    pub fn to_ascii(&self) -> String {
        (0..self.height).map(|y| (0..self.width).map(|x| if self.get(x, y) { '#' } else { '.' }).collect::<String>() + "\n").collect()
    }
}

/// Rasterises the text. `gauge` gives the stitch proportions so letters keep their shape.
pub fn text_to_mask(opts: &TextOptions, gauge: &Gauge) -> Result<TextMask, TextError> {
    let chars: Vec<char> = opts.text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if chars.iter().all(|c| c.is_whitespace()) {
        return Err(TextError::Empty);
    }
    if chars.len() > MAX_CHARS {
        return Err(TextError::TooLong);
    }
    if !(MIN_ROWS..=MAX_ROWS).contains(&opts.rows_tall) {
        return Err(TextError::BadHeight);
    }
    gauge.validate().map_err(ChartError::from)?;
    let aspect = gauge.stitch_aspect() as f32;
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|_| TextError::Font)?;
    // A stitch is wider than tall by `aspect`, so the same physical letter needs fewer columns than rows.
    let scale = PxScale { x: opts.rows_tall as f32 / aspect, y: opts.rows_tall as f32 };
    let scaled = font.as_scaled(scale);
    let spacing = opts.letter_spacing.clamp(-3, 20) as f32;

    // Lay out on a float line, collecting glyph coverage.
    let mut pen = 0.0f32;
    let mut prev = None;
    let mut pixels: Vec<(i32, i32, f32)> = Vec::new();
    for &c in &chars {
        let id = scaled.glyph_id(c);
        if let Some(p) = prev {
            pen += scaled.kern(p, id);
        }
        if let Some(outlined) = font.outline_glyph(id.with_scale_and_position(scale, point(pen, scaled.ascent()))) {
            let b = outlined.px_bounds();
            let (bx, by) = (b.min.x.floor() as i32, b.min.y.floor() as i32);
            outlined.draw(|gx, gy, cov| pixels.push((bx + gx as i32, by + gy as i32, cov)));
        }
        pen += scaled.h_advance(id) + spacing;
        prev = Some(id);
    }
    if pen > MAX_SIDE as f32 * 2.0 {
        return Err(TextError::TooLarge);
    }
    let threshold = if opts.bold { 0.25 } else { 0.5 };
    let on_pixels: Vec<(i32, i32)> = pixels.iter().filter(|p| p.2 >= threshold).map(|p| (p.0, p.1)).collect();
    let Some((min_x, max_x, min_y, max_y)) = bbox(&on_pixels) else { return Err(TextError::Empty) };
    let (iw, ih) = ((max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32);
    let mut ink = vec![false; (iw * ih) as usize];
    for (x, y) in on_pixels {
        ink[((y - min_y) as u32 * iw + (x - min_x) as u32) as usize] = true;
    }
    let ink = if opts.outline { edge_only(&ink, iw, ih) } else { ink };

    let needed = iw + 2 * opts.margin_cols;
    let width = opts.width.unwrap_or(needed);
    if needed > width {
        return Err(TextError::TooWide { needed, width });
    }
    let height = ih + 2 * opts.margin_rows;
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(TextError::TooLarge);
    }
    let left = (width - iw) / 2;
    let mut on = vec![false; (width * height) as usize];
    for y in 0..ih {
        for x in 0..iw {
            if ink.get((y * iw + x) as usize).copied().unwrap_or(false) {
                on[((y + opts.margin_rows) * width + left + x) as usize] = true;
            }
        }
    }
    Ok(TextMask { width, height, on })
}

/// A chart of the text: palette index 0 = background, 1 = lettering.
pub fn chart_from_text(
    opts: &TextOptions,
    background: PaletteColor,
    lettering: PaletteColor,
    craft: Craft,
    technique: Technique,
    construction: Construction,
    gauge: Gauge,
) -> Result<Chart, TextError> {
    let mask = text_to_mask(opts, &gauge)?;
    let (w, h) = (mask.width as usize, mask.height as usize);
    let mut cells = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            // chart row 0 is the bottom of the piece
            cells[(h - 1 - y) * w + x] = u8::from(mask.on.get(y * w + x).copied().unwrap_or(false));
        }
    }
    Ok(Chart::new(mask.width, mask.height, cells, vec![background, lettering], craft, technique, construction, gauge)?)
}

fn bbox(points: &[(i32, i32)]) -> Option<(i32, i32, i32, i32)> {
    let first = points.first()?;
    Some(points.iter().fold((first.0, first.0, first.1, first.1), |b, p| (b.0.min(p.0), b.1.max(p.0), b.2.min(p.1), b.3.max(p.1))))
}

/// Keeps only stitches that touch an empty one (or the border) above, below, left or right.
fn edge_only(ink: &[bool], w: u32, h: u32) -> Vec<bool> {
    let at = |x: i64, y: i64| {
        x >= 0 && y >= 0 && x < i64::from(w) && y < i64::from(h) && ink.get((y as u32 * w + x as u32) as usize).copied().unwrap_or(false)
    };
    let mut out = vec![false; ink.len()];
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            if at(x, y) && !(at(x - 1, y) && at(x + 1, y) && at(x, y - 1) && at(x, y + 1)) {
                out[(y as u32 * w + x as u32) as usize] = true;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::test_support::gauge;
    use crate::gauge::Unit;
    use proptest::prelude::*;

    fn square() -> Gauge {
        Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap()
    }

    fn ink(m: &TextMask) -> usize {
        m.on.iter().filter(|&&b| b).count()
    }

    mod golden {
        use super::*;

        fn king_cashew() -> TextMask {
            let mut o = TextOptions::new("KING CASHEW");
            o.rows_tall = 18;
            text_to_mask(&o, &gauge()).unwrap()
        }

        #[test]
        fn king_cashew_matches_the_golden_picture() {
            let mask = king_cashew();
            let ascii = mask.to_ascii();
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/king_cashew.txt");
            if std::env::var_os("UPDATE_GOLDEN").is_some() {
                std::fs::write(&path, &ascii).unwrap();
            }
            let golden = std::fs::read_to_string(&path).expect("golden file (run with UPDATE_GOLDEN=1 to create)");
            assert_eq!(ascii, golden, "the picture of KING CASHEW changed:\n{ascii}");
        }

        #[test]
        fn king_cashew_is_readable_sized_and_both_words_are_there() {
            let mask = king_cashew();
            assert!(mask.height >= 14 && mask.height <= 24, "height {}", mask.height);
            // a clear gap between the two words: a run of empty columns wider than the letter gaps
            let empty_cols: Vec<bool> = (0..mask.width).map(|x| (0..mask.height).all(|y| !mask.get(x, y))).collect();
            let mut runs = Vec::new();
            let mut run = 0;
            for &e in &empty_cols[2..empty_cols.len() - 2] {
                if e {
                    run += 1;
                } else if run > 0 {
                    runs.push(run);
                    run = 0;
                }
            }
            assert!(runs.iter().any(|&r| r >= 3), "no word gap in {runs:?}");
        }
    }

    mod options {
        use super::*;

        #[test]
        fn a_capital_i_is_a_solid_bar_about_as_tall_as_asked() {
            let mut o = TextOptions::new("I");
            o.margin_cols = 0;
            o.margin_rows = 0;
            o.rows_tall = 20;
            let m = text_to_mask(&o, &square()).unwrap();
            assert!(m.height >= 12 && m.height <= 20, "height {}", m.height);
            assert!((0..m.height).all(|y| (0..m.width).any(|x| m.get(x, y))));
        }

        #[test]
        fn bold_has_more_ink_and_outline_has_less_and_is_hollow() {
            let g = gauge();
            let plain = text_to_mask(&TextOptions::new("OM"), &g).unwrap();
            let mut b = TextOptions::new("OM");
            b.bold = true;
            let bold = text_to_mask(&b, &g).unwrap();
            assert!(ink(&bold) > ink(&plain));
            let mut o = TextOptions::new("OM");
            o.rows_tall = 24;
            let filled = text_to_mask(&o, &g).unwrap();
            o.outline = true;
            let hollow = text_to_mask(&o, &g).unwrap();
            assert!(ink(&hollow) < ink(&filled) && ink(&hollow) > 0);
            assert_eq!((hollow.width, hollow.height), (filled.width, filled.height));
        }

        #[test]
        fn letter_spacing_widens_the_text() {
            let g = gauge();
            let mut a = TextOptions::new("WOOL");
            a.letter_spacing = 0;
            let mut b = a.clone();
            b.letter_spacing = 3;
            let (wa, wb) = (text_to_mask(&a, &g).unwrap().width, text_to_mask(&b, &g).unwrap().width);
            assert!(wb >= wa + 6, "{wa} vs {wb}");
        }

        #[test]
        fn text_is_centred_in_a_given_width_with_margins_kept() {
            let g = gauge();
            let mut o = TextOptions::new("HI");
            let natural = text_to_mask(&o, &g).unwrap();
            o.width = Some(natural.width + 20);
            let m = text_to_mask(&o, &g).unwrap();
            assert_eq!(m.width, natural.width + 20);
            let first = (0..m.width).find(|&x| (0..m.height).any(|y| m.get(x, y))).unwrap();
            let last = (0..m.width).rev().find(|&x| (0..m.height).any(|y| m.get(x, y))).unwrap();
            let (left, right) = (first, m.width - 1 - last);
            assert!(left.abs_diff(right) <= 1, "{left} vs {right}");
            assert!(left >= o.margin_cols);
            let top = (0..m.height).find(|&y| (0..m.width).any(|x| m.get(x, y))).unwrap();
            assert_eq!(top, o.margin_rows);
        }

        #[test]
        fn the_gauge_changes_the_proportions_not_the_height() {
            let o = TextOptions::new("OOO");
            let tall = text_to_mask(&o, &gauge()).unwrap(); // stitches 1.4x taller... rows are finer, fewer columns
            let sq = text_to_mask(&o, &square()).unwrap();
            assert!(tall.width < sq.width, "{} vs {}", tall.width, sq.width);
            assert_eq!(tall.height, sq.height);
        }
    }

    mod errors {
        use super::*;

        #[test]
        fn bad_input_is_an_error() {
            let g = gauge();
            assert_eq!(text_to_mask(&TextOptions::new(""), &g), Err(TextError::Empty));
            assert_eq!(text_to_mask(&TextOptions::new("   \n\t"), &g), Err(TextError::Empty));
            assert_eq!(text_to_mask(&TextOptions::new(&"A".repeat(MAX_CHARS + 1)), &g), Err(TextError::TooLong));
            let mut o = TextOptions::new("A");
            o.rows_tall = 2;
            assert_eq!(text_to_mask(&o, &g), Err(TextError::BadHeight));
            o.rows_tall = MAX_ROWS + 1;
            assert_eq!(text_to_mask(&o, &g), Err(TextError::BadHeight));
            let mut narrow = TextOptions::new("KING CASHEW");
            narrow.width = Some(10);
            assert!(matches!(text_to_mask(&narrow, &g), Err(TextError::TooWide { width: 10, .. })));
            let mut huge = TextOptions::new(&"W".repeat(MAX_CHARS));
            huge.rows_tall = MAX_ROWS;
            assert!(text_to_mask(&huge, &g).is_err());
        }

        #[test]
        fn glyphs_the_font_lacks_do_not_panic() {
            let _ = text_to_mask(&TextOptions::new("\u{1F9F6} yarn \u{4E2D}"), &gauge());
        }

        proptest! {
            #[test]
            fn any_text_and_options_never_panic(text in ".{0,40}", rows in 0u32..60, spacing in -10i32..30, bold in any::<bool>(), outline in any::<bool>()) {
                let mut o = TextOptions::new(&text);
                (o.rows_tall, o.letter_spacing, o.bold, o.outline) = (rows, spacing, bold, outline);
                let _ = text_to_mask(&o, &gauge());
            }
        }
    }

    #[test]
    fn a_text_chart_has_two_colours_and_the_letters_run_bottom_up() {
        let bg = PaletteColor { name: "cream".into(), rgb: [250, 244, 230] };
        let fg = PaletteColor { name: "red".into(), rgb: [200, 30, 40] };
        let chart = chart_from_text(&TextOptions::new("T"), bg, fg, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge()).unwrap();
        assert_eq!(chart.palette.len(), 2);
        // The bar of the T is at the top of the piece: the last row has lettering, the first row (margin) is empty.
        let row = |y: u32| (0..chart.width).filter(|&x| chart.get(x, y) == Some(1)).count();
        assert_eq!(row(0), 0);
        assert!(row(chart.height - 1 - 2) > 0);
    }
}
