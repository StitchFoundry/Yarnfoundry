//! Technique rules applied to a chart after mapping: fair isle (two colours a row, floats), intarsia (bobbins, small
//! areas), tapestry (carried colours) and filet (filled or open only). Rows are numbered from the first row worked
//! (chart row 0); messages count rows from 1.

use crate::chart::{Chart, Technique};
use crate::color::{delta_e_sq, srgb_to_lab};

/// Limits for the rules.
#[derive(Clone, Debug, PartialEq)]
pub struct TechniqueOptions {
    /// Fair isle: the longest float, in stitches, before it is flagged (default 5).
    pub max_float: u32,
    /// Intarsia: areas smaller than this many stitches are merged into their neighbour (default 4).
    pub min_area: u32,
    /// Tapestry: warn when a row uses more colours than this (default 3).
    pub max_carried: u32,
    /// Filet: stitches darker than this CIELAB lightness (0-100) are filled, the rest open (default 50).
    pub filet_threshold: f64,
}

impl Default for TechniqueOptions {
    fn default() -> Self {
        Self { max_float: 5, min_area: 4, max_carried: 3, filet_threshold: 50.0 }
    }
}

/// A float of `len` stitches in colour-change terms: the run of one colour that the other yarn must be carried behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Float {
    /// Row, from the first row worked (0-based).
    pub row: u32,
    /// First stitch (0-based, chart column).
    pub x: u32,
    pub len: u32,
    /// Columns where the carried yarn should be caught: every `max_float` stitches inside the float.
    pub catch_at: Vec<u32>,
}

/// What the rules found and changed.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TechniqueReport {
    /// Distinct colours in each row (for tapestry these are the carried colours).
    pub colours_per_row: Vec<u32>,
    /// Fair isle floats longer than the limit.
    pub floats: Vec<Float>,
    /// Intarsia: colour areas in each row, which is the number of bobbins that row needs.
    pub bobbins_per_row: Vec<u32>,
    /// Stitches whose colour was changed to satisfy the rules.
    pub stitches_changed: u32,
    /// Intarsia: small areas merged away.
    pub areas_merged: u32,
    /// Filet: stitches in a row (3 x squares + 1).
    pub filet_stitches_per_row: Option<u32>,
    pub warnings: Vec<String>,
}

/// Applies the rules for the chart's technique, changing its cells where the technique demands, and reports.
pub fn apply(chart: &mut Chart, opts: &TechniqueOptions) -> TechniqueReport {
    let mut report = TechniqueReport::default();
    match chart.technique {
        Technique::FairIsle => fair_isle(chart, opts, &mut report),
        Technique::Intarsia => intarsia(chart, opts, &mut report),
        Technique::Tapestry => {}
        Technique::Filet => filet(chart, opts, &mut report),
    }
    report.colours_per_row = (0..chart.height).map(|y| distinct(row(chart, y)) as u32).collect();
    if chart.technique == Technique::Tapestry {
        for (y, &n) in report.colours_per_row.iter().enumerate() {
            if n > opts.max_carried {
                report.warnings.push(format!("Row {} uses {n} colours; more than {} carried at once is hard to work", y + 1, opts.max_carried));
            }
        }
    }
    report.bobbins_per_row = (0..chart.height).map(|y| runs(row(chart, y)).len() as u32).collect();
    report
}

/// Stitches in a filet row of `squares` squares: each square is 3 stitches and the row ends with one more.
pub fn filet_stitches(squares: u32) -> u32 {
    squares.saturating_mul(3).saturating_add(1)
}

fn row(chart: &Chart, y: u32) -> &[u8] {
    let w = chart.width as usize;
    chart.cells.get(y as usize * w..(y as usize + 1) * w).unwrap_or(&[])
}

fn distinct(cells: &[u8]) -> usize {
    let mut seen = [false; 256];
    cells.iter().for_each(|&c| seen[usize::from(c)] = true);
    seen.iter().filter(|&&s| s).count()
}

/// Maximal runs of one colour: (start, length, colour).
fn runs(cells: &[u8]) -> Vec<(u32, u32, u8)> {
    let mut out: Vec<(u32, u32, u8)> = Vec::new();
    for (i, &c) in cells.iter().enumerate() {
        match out.last_mut() {
            Some(last) if last.2 == c => last.1 += 1,
            _ => out.push((i as u32, 1, c)),
        }
    }
    out
}

fn fair_isle(chart: &mut Chart, opts: &TechniqueOptions, report: &mut TechniqueReport) {
    let labs: Vec<_> = chart.palette.iter().map(|c| srgb_to_lab(c.rgb)).collect();
    let w = chart.width as usize;
    for y in 0..chart.height as usize {
        let Some(cells) = chart.cells.get_mut(y * w..(y + 1) * w) else { continue };
        if distinct(cells) > 2 {
            // keep the two most used colours (ties: lower index); the rest go to the nearest of the two
            let mut counts = [0u32; 256];
            cells.iter().for_each(|&c| counts[usize::from(c)] += 1);
            let mut order: Vec<usize> = (0..256).filter(|&i| counts[i] > 0).collect();
            order.sort_by(|&a, &b| counts[b].cmp(&counts[a]).then(a.cmp(&b)));
            let keep = [order[0] as u8, order[1] as u8];
            for c in cells.iter_mut() {
                if !keep.contains(c) {
                    let d = |k: u8| match (labs.get(usize::from(*c)), labs.get(usize::from(k))) {
                        (Some(&a), Some(&b)) => delta_e_sq(a, b),
                        _ => f64::MAX,
                    };
                    *c = if d(keep[0]) <= d(keep[1]) { keep[0] } else { keep[1] };
                    report.stitches_changed += 1;
                }
            }
        }
        let cells = &*cells;
        if distinct(cells) == 2 {
            for (start, len, _) in runs(cells) {
                if len > opts.max_float {
                    let catch_at = (1..).map(|k| start + k * opts.max_float.max(1)).take_while(|&p| p < start + len).collect();
                    report.floats.push(Float { row: y as u32, x: start, len, catch_at });
                }
            }
        }
    }
    for f in &report.floats {
        report.warnings.push(format!(
            "Row {}: a float of {} stitches from stitch {}; catch the carried yarn at {:?}",
            f.row + 1,
            f.len,
            f.x + 1,
            f.catch_at.iter().map(|c| c + 1).collect::<Vec<_>>()
        ));
    }
}

fn intarsia(chart: &mut Chart, opts: &TechniqueOptions, report: &mut TechniqueReport) {
    let (w, h) = (chart.width as usize, chart.height as usize);
    // Merge small same-colour areas (4-connected) into the neighbouring colour they touch most, smallest first, a few passes.
    for _ in 0..5 {
        let mut label = vec![usize::MAX; w * h];
        let mut areas: Vec<Vec<usize>> = Vec::new();
        for start in 0..w * h {
            if label[start] != usize::MAX {
                continue;
            }
            let colour = chart.cells[start];
            let id = areas.len();
            let mut stack = vec![start];
            let mut cells = Vec::new();
            label[start] = id;
            while let Some(i) = stack.pop() {
                cells.push(i);
                let (x, y) = (i % w, i / w);
                let mut push = |nx: usize, ny: usize| {
                    let j = ny * w + nx;
                    if label[j] == usize::MAX && chart.cells[j] == colour {
                        label[j] = id;
                        stack.push(j);
                    }
                };
                if x > 0 {
                    push(x - 1, y);
                }
                if x + 1 < w {
                    push(x + 1, y);
                }
                if y > 0 {
                    push(x, y - 1);
                }
                if y + 1 < h {
                    push(x, y + 1);
                }
            }
            areas.push(cells);
        }
        let mut small: Vec<usize> = (0..areas.len()).filter(|&a| (areas[a].len() as u32) < opts.min_area).collect();
        small.sort_by_key(|&a| (areas[a].len(), areas[a][0]));
        let mut merged_any = false;
        for a in small {
            // skip if an earlier merge in this pass already changed this area's cells
            let colour = chart.cells[areas[a][0]];
            if areas[a].iter().any(|&i| chart.cells[i] != colour) {
                continue;
            }
            let mut touch = [0u32; 256];
            for &i in &areas[a] {
                let (x, y) = (i % w, i / w);
                let mut look = |nx: usize, ny: usize| {
                    let c = chart.cells[ny * w + nx];
                    if c != colour {
                        touch[usize::from(c)] += 1;
                    }
                };
                if x > 0 {
                    look(x - 1, y);
                }
                if x + 1 < w {
                    look(x + 1, y);
                }
                if y > 0 {
                    look(x, y - 1);
                }
                if y + 1 < h {
                    look(x, y + 1);
                }
            }
            let best = touch.iter().enumerate().fold((0usize, 0u32), |b, (i, &t)| if t > b.1 { (i, t) } else { b });
            if best.1 > 0 {
                for &i in &areas[a] {
                    chart.cells[i] = best.0 as u8;
                }
                report.areas_merged += 1;
                report.stitches_changed += areas[a].len() as u32;
                merged_any = true;
            }
        }
        if !merged_any {
            break;
        }
    }
}

fn filet(chart: &mut Chart, opts: &TechniqueOptions, report: &mut TechniqueReport) {
    let lightness: Vec<f64> = chart.palette.iter().map(|c| srgb_to_lab(c.rgb)[0]).collect();
    let by =
        |f: fn(&f64, &f64) -> std::cmp::Ordering| (0..lightness.len()).min_by(|&a, &b| f(&lightness[a], &lightness[b]).then(a.cmp(&b))).unwrap_or(0);
    let open = by(|a, b| b.total_cmp(a)); // lightest colour
    let filled = by(|a, b| a.total_cmp(b)); // darkest colour
    let threshold = if opts.filet_threshold.is_finite() { opts.filet_threshold } else { 50.0 };
    let before = chart.cells.clone();
    for c in &mut chart.cells {
        let dark = lightness.get(usize::from(*c)).is_some_and(|&l| l < threshold);
        *c = if dark { filled as u8 } else { open as u8 };
    }
    report.stitches_changed = chart.cells.iter().zip(&before).filter(|(a, b)| a != b).count() as u32;
    report.filet_stitches_per_row = Some(filet_stitches(chart.width));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::test_support::gauge;
    use crate::chart::{Construction, Craft, PaletteColor};
    use proptest::prelude::*;

    fn palette(n: usize) -> Vec<PaletteColor> {
        let all = [[250, 244, 230], [200, 30, 40], [20, 40, 120], [30, 120, 50], [240, 200, 40], [10, 10, 10]];
        (0..n).map(|i| PaletteColor { name: format!("c{i}"), rgb: all[i % all.len()] }).collect()
    }

    fn chart(technique: Technique, w: u32, h: u32, cells: Vec<u8>, colours: usize) -> Chart {
        let craft = if matches!(technique, Technique::Tapestry | Technique::Filet) { Craft::Crochet } else { Craft::Knit };
        Chart::new(w, h, cells, palette(colours), craft, technique, Construction::Flat, gauge()).unwrap()
    }

    mod fair_isle {
        use super::*;

        #[test]
        fn a_third_colour_goes_to_the_nearest_of_the_two_most_used() {
            // row: 4 cream, 4 red, 1 dark red-ish? use palette 0 cream,1 red,4 gold; gold is nearer cream than red
            let mut c = chart(Technique::FairIsle, 9, 1, vec![0, 0, 0, 0, 1, 1, 1, 1, 4], 5);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert_eq!(distinct(row(&c, 0)), 2);
            assert_eq!(r.stitches_changed, 1);
            assert_eq!(c.cells[8], 0, "gold is closer to cream than to red");
        }

        #[test]
        fn floats_over_the_limit_are_flagged_with_catch_points() {
            let mut cells = vec![0u8; 14];
            cells[0] = 1;
            cells[13] = 1;
            let mut c = chart(Technique::FairIsle, 14, 1, cells, 2);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert_eq!(r.floats, vec![Float { row: 0, x: 1, len: 12, catch_at: vec![6, 11] }]);
            assert!(r.warnings[0].starts_with("Row 1: a float of 12 stitches from stitch 2"));
        }

        #[test]
        fn short_floats_and_single_colour_rows_are_fine() {
            let mut c = chart(Technique::FairIsle, 10, 2, vec![0, 0, 0, 0, 0, 1, 1, 1, 1, 1, /* row 1 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 2);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert!(r.floats.is_empty(), "{:?}", r.floats);
        }

        proptest! {
            #[test]
            fn no_row_has_more_than_two_colours_and_every_long_run_is_flagged(
                w in 1u32..30, h in 1u32..12, colours in 1usize..6, seed in proptest::collection::vec(any::<u8>(), 360),
                limit in 1u32..8,
            ) {
                let cells: Vec<u8> = (0..w * h).map(|i| seed[i as usize % seed.len()] % colours as u8).collect();
                let mut c = chart(Technique::FairIsle, w, h, cells, colours);
                let opts = TechniqueOptions { max_float: limit, ..Default::default() };
                let r = apply(&mut c, &opts);
                for y in 0..h {
                    let cells = row(&c, y);
                    prop_assert!(distinct(cells) <= 2);
                    let expected: Vec<(u32, u32)> = if distinct(cells) == 2 {
                        runs(cells).into_iter().filter(|r| r.1 > limit).map(|r| (r.0, r.1)).collect()
                    } else { vec![] };
                    let flagged: Vec<(u32, u32)> = r.floats.iter().filter(|f| f.row == y).map(|f| (f.x, f.len)).collect();
                    prop_assert_eq!(flagged, expected);
                }
                prop_assert!(c.validate().is_ok());
            }
        }
    }

    mod intarsia {
        use super::*;

        #[test]
        fn bobbins_are_the_colour_areas_in_each_row() {
            let mut c = chart(Technique::Intarsia, 6, 2, vec![0, 0, 1, 1, 0, 0, /* row 1 */ 0, 0, 0, 0, 0, 0], 2);
            let opts = TechniqueOptions { min_area: 1, ..Default::default() };
            let r = apply(&mut c, &opts);
            assert_eq!(r.bobbins_per_row, vec![3, 1]);
        }

        #[test]
        fn areas_under_the_minimum_merge_into_the_neighbour() {
            let mut cells = vec![0u8; 25];
            cells[12] = 1; // one stitch in the middle
            cells[0] = 1;
            cells[1] = 1;
            cells[5] = 1;
            cells[6] = 1; // a 2x2 block in the corner: 4 stitches, kept at min_area 4
            let mut c = chart(Technique::Intarsia, 5, 5, cells, 2);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert_eq!(c.cells[12], 0, "the lone stitch merged");
            assert_eq!(c.cells[0], 1, "the 4-stitch block stays");
            assert_eq!((r.areas_merged, r.stitches_changed), (1, 1));
        }

        #[test]
        fn a_chart_that_is_one_small_area_is_left_alone() {
            let mut c = chart(Technique::Intarsia, 2, 1, vec![0, 0], 2);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert_eq!(c.cells, vec![0, 0]);
            assert_eq!(r.areas_merged, 0);
        }

        proptest! {
            #[test]
            fn after_merging_no_small_area_that_has_a_neighbour_remains(
                w in 1u32..16, h in 1u32..16, colours in 2usize..5, seed in proptest::collection::vec(any::<u8>(), 256),
            ) {
                let cells: Vec<u8> = (0..w * h).map(|i| seed[i as usize % seed.len()] % colours as u8).collect();
                let mut c = chart(Technique::Intarsia, w, h, cells, colours);
                let r = apply(&mut c, &TechniqueOptions::default());
                prop_assert!(c.validate().is_ok());
                prop_assert_eq!(r.bobbins_per_row.len(), h as usize);
                for y in 0..h {
                    prop_assert_eq!(r.bobbins_per_row[y as usize] as usize, runs(row(&c, y)).len());
                }
            }
        }
    }

    mod tapestry {
        use super::*;

        #[test]
        fn carried_colours_are_reported_and_more_than_three_warns() {
            let mut c = chart(Technique::Tapestry, 5, 2, vec![0, 1, 2, 3, 4, /* row 1 */ 0, 0, 1, 1, 2], 5);
            let r = apply(&mut c, &TechniqueOptions::default());
            assert_eq!(r.colours_per_row, vec![5, 3]);
            assert_eq!(r.warnings.len(), 1);
            assert!(r.warnings[0].starts_with("Row 1 uses 5 colours"));
            assert_eq!(c.cells, vec![0, 1, 2, 3, 4, 0, 0, 1, 1, 2], "tapestry never changes the chart");
        }
    }

    mod filet {
        use super::*;

        #[test]
        fn only_filled_and_open_remain_and_a_row_is_three_stitches_a_square_plus_one() {
            let mut c = chart(Technique::Filet, 6, 2, vec![0, 1, 2, 3, 4, 5, 5, 4, 3, 2, 1, 0], 6);
            let r = apply(&mut c, &TechniqueOptions::default());
            let used: std::collections::BTreeSet<u8> = c.cells.iter().copied().collect();
            assert_eq!(used.len(), 2);
            assert_eq!(r.filet_stitches_per_row, Some(3 * 6 + 1));
            // cream (0), gold (4) are light = open; red/blue/green/black are dark = filled
            let (open, filled) = (0u8, 5u8);
            assert_eq!(c.cells[0], open);
            assert_eq!(c.cells[4], open);
            assert_eq!(c.cells[5], filled);
            assert_eq!(c.cells[1], filled);
        }

        #[test]
        fn the_threshold_moves_the_line() {
            let mut a = chart(Technique::Filet, 2, 1, vec![1, 3], 4);
            apply(&mut a, &TechniqueOptions { filet_threshold: 0.0, ..Default::default() });
            assert_eq!(distinct(&a.cells), 1, "nothing is dark enough at 0");
            let mut b = chart(Technique::Filet, 2, 1, vec![1, 3], 4);
            apply(&mut b, &TechniqueOptions { filet_threshold: 100.0, ..Default::default() });
            assert_eq!(distinct(&b.cells), 1, "everything is dark at 100");
        }

        #[test]
        fn stitch_count_never_overflows() {
            assert_eq!(filet_stitches(0), 1);
            assert_eq!(filet_stitches(u32::MAX), u32::MAX);
        }

        proptest! {
            #[test]
            fn filet_charts_are_always_two_state_and_valid(
                w in 1u32..20, h in 1u32..10, colours in 1usize..6, seed in proptest::collection::vec(any::<u8>(), 200), t in -10.0f64..110.0,
            ) {
                let cells: Vec<u8> = (0..w * h).map(|i| seed[i as usize % seed.len()] % colours as u8).collect();
                let mut c = chart(Technique::Filet, w, h, cells, colours);
                let r = apply(&mut c, &TechniqueOptions { filet_threshold: t, ..Default::default() });
                prop_assert!(distinct(&c.cells) <= 2);
                prop_assert!(c.validate().is_ok());
                prop_assert_eq!(r.filet_stitches_per_row, Some(3 * w + 1));
            }
        }
    }
}
