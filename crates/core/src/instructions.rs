//! Written row-by-row instructions: run-length encode each row for knit and crochet.
//!
//! Row 1 is chart row 0 (the first row worked). Flat: right-side rows read right to left, wrong-side rows left to
//! right; in the round every round reads right to left. Knit: `k` on the right side and in the round, `p` on the
//! wrong side; crochet: `sc` (tapestry); filet: `dc` and `ch`.

use crate::chart::{Chart, Construction, Craft, Technique};
use crate::color::srgb_to_lab;

/// Which side of the work a row is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    RightSide,
    WrongSide,
    Round,
}

/// One written row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowInstruction {
    /// Row or round number, from 1.
    pub number: u32,
    pub side: Side,
    /// The instruction, e.g. `Row 5 (RS): with A k12, with B k3, with A k5 (20 sts)`.
    pub text: String,
    /// Stitches in the row (3 x squares + 1 for filet).
    pub stitches: u32,
    /// Runs in reading order: (palette index, count). For filet the count is squares.
    pub runs: Vec<(u8, u32)>,
}

/// How much of one colour the chart uses (stitches; squares for filet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColourTotal {
    pub colour: u8,
    pub label: String,
    pub name: String,
    pub count: u32,
}

/// All the written instructions for a chart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instructions {
    pub rows: Vec<RowInstruction>,
    pub totals: Vec<ColourTotal>,
}

/// The letter label of palette colour `index`: A, B, ... Z, AA, AB, ...
pub fn colour_label(index: u8) -> String {
    let letter = |n: usize| char::from(b'A' + (n % 26) as u8);
    let i = usize::from(index);
    if i < 26 { letter(i).to_string() } else { format!("{}{}", letter(i / 26 - 1), letter(i)) }
}

/// Writes the instructions for `chart`.
pub fn instructions(chart: &Chart) -> Instructions {
    let w = chart.width as usize;
    let filet = chart.technique == Technique::Filet;
    let (open, filled) = filet_roles(chart);
    let mut rows = Vec::with_capacity(chart.height as usize);
    for y in 0..chart.height {
        let number = y + 1;
        let side = match chart.construction {
            Construction::Round => Side::Round,
            Construction::Flat if number % 2 == 1 => Side::RightSide,
            Construction::Flat => Side::WrongSide,
        };
        let cells = chart.cells.get(y as usize * w..(y as usize + 1) * w).unwrap_or(&[]);
        let mut seq: Vec<u8> = cells.to_vec();
        if side != Side::WrongSide {
            seq.reverse(); // right to left
        }
        let runs = rle(&seq);
        let (stitches, body) =
            if filet { (filet_stitches(chart.width), filet_text(&runs, open, filled)) } else { (chart.width, stitch_text(&runs, chart.craft, side)) };
        let heading = match side {
            Side::RightSide => format!("Row {number} (RS)"),
            Side::WrongSide => format!("Row {number} (WS)"),
            Side::Round => format!("Rnd {number}"),
        };
        rows.push(RowInstruction { number, side, text: format!("{heading}: {body} ({stitches} sts)"), stitches, runs });
    }
    let mut counts = vec![0u32; chart.palette.len()];
    for &c in &chart.cells {
        if let Some(n) = counts.get_mut(usize::from(c)) {
            *n += 1;
        }
    }
    let totals = chart
        .palette
        .iter()
        .enumerate()
        .map(|(i, p)| ColourTotal { colour: i as u8, label: colour_label(i as u8), name: p.name.clone(), count: counts.get(i).copied().unwrap_or(0) })
        .collect();
    Instructions { rows, totals }
}

fn filet_stitches(squares: u32) -> u32 {
    crate::technique::filet_stitches(squares)
}

fn rle(seq: &[u8]) -> Vec<(u8, u32)> {
    let mut out: Vec<(u8, u32)> = Vec::new();
    for &c in seq {
        match out.last_mut() {
            Some(last) if last.0 == c => last.1 += 1,
            _ => out.push((c, 1)),
        }
    }
    out
}

fn stitch_text(runs: &[(u8, u32)], craft: Craft, side: Side) -> String {
    let stitch = match (craft, side) {
        (Craft::Crochet, _) => "sc",
        (Craft::Knit, Side::WrongSide) => "p",
        (Craft::Knit, _) => "k",
    };
    runs.iter().map(|&(c, n)| format!("with {} {stitch}{n}", colour_label(c))).collect::<Vec<_>>().join(", ")
}

/// Filet: an edge dc, then each square is open (ch 2, dc) or filled (dc 3), grouped in runs.
fn filet_text(runs: &[(u8, u32)], open: u8, filled: u8) -> String {
    let mut parts = vec!["dc".to_string()];
    for &(c, n) in runs {
        let square = if c == filled && c != open { "filled (dc 3)" } else { "open (ch 2, dc)" };
        parts.push(format!("{n} {square}"));
    }
    parts.join(", ")
}

/// The palette colours meaning open and filled in a filet chart: lightest is open, darkest is filled.
fn filet_roles(chart: &Chart) -> (u8, u8) {
    let l = |i: usize| chart.palette.get(i).map_or(0.0, |c| srgb_to_lab(c.rgb)[0]);
    let n = chart.palette.len();
    let open = (0..n).max_by(|&a, &b| l(a).total_cmp(&l(b)).then(b.cmp(&a))).unwrap_or(0);
    let filled = (0..n).min_by(|&a, &b| l(a).total_cmp(&l(b)).then(a.cmp(&b))).unwrap_or(0);
    (open as u8, filled as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::PaletteColor;
    use crate::chart::test_support::gauge;
    use proptest::prelude::*;

    fn palette(n: usize) -> Vec<PaletteColor> {
        let all = [[250, 244, 230], [200, 30, 40], [20, 40, 120], [30, 120, 50], [240, 200, 40], [10, 10, 10]];
        (0..n).map(|i| PaletteColor { name: format!("c{i}"), rgb: all[i % all.len()] }).collect()
    }

    fn chart(craft: Craft, technique: Technique, construction: Construction, w: u32, h: u32, cells: Vec<u8>, colours: usize) -> Chart {
        Chart::new(w, h, cells, palette(colours), craft, technique, construction, gauge()).unwrap()
    }

    /// Rows made of runs given in reading order for a right-to-left row (so the first run is at the right).
    fn right_to_left(runs: &[(u8, usize)]) -> Vec<u8> {
        let mut seq: Vec<u8> = runs.iter().flat_map(|&(c, n)| std::iter::repeat_n(c, n)).collect();
        seq.reverse();
        seq
    }

    mod directions {
        use super::*;

        #[test]
        fn the_example_row_5_reads_right_to_left_in_knit_terms() {
            let mut cells = vec![0u8; 20 * 5];
            cells[4 * 20..].copy_from_slice(&right_to_left(&[(0, 12), (1, 3), (0, 5)]));
            let c = chart(Craft::Knit, Technique::Intarsia, Construction::Flat, 20, 5, cells, 2);
            let i = instructions(&c);
            assert_eq!(i.rows[4].text, "Row 5 (RS): with A k12, with B k3, with A k5 (20 sts)");
        }

        #[test]
        fn wrong_side_rows_read_left_to_right_and_purl() {
            let mut cells = vec![0u8; 10 * 2];
            // row 2 (chart row 1), left to right: 3 B then 7 A
            for x in 0..3 {
                cells[10 + x] = 1;
            }
            let c = chart(Craft::Knit, Technique::Intarsia, Construction::Flat, 10, 2, cells, 2);
            let i = instructions(&c);
            assert_eq!(i.rows[1].text, "Row 2 (WS): with B p3, with A p7 (10 sts)");
            assert_eq!(i.rows[1].side, Side::WrongSide);
            assert_eq!(i.rows[0].side, Side::RightSide);
        }

        #[test]
        fn in_the_round_every_round_reads_right_to_left_and_knits() {
            let mut cells = vec![0u8; 8 * 2];
            for row in 0..2 {
                cells[row * 8] = 1; // leftmost stitch is colour B, so it is the last in reading order
            }
            let c = chart(Craft::Knit, Technique::FairIsle, Construction::Round, 8, 2, cells, 2);
            let i = instructions(&c);
            assert_eq!(i.rows[0].text, "Rnd 1: with A k7, with B k1 (8 sts)");
            assert_eq!(i.rows[1].text, "Rnd 2: with A k7, with B k1 (8 sts)");
            assert!(i.rows.iter().all(|r| r.side == Side::Round));
        }
    }

    mod crochet {
        use super::*;

        #[test]
        fn tapestry_uses_single_crochet_on_both_sides() {
            let cells = vec![0, 0, 1, 1, 1, 0, 1, 0];
            let c = chart(Craft::Crochet, Technique::Tapestry, Construction::Flat, 4, 2, cells, 2);
            let i = instructions(&c);
            // chart row 0, right to left: cells 0,0,1,1 reversed = B, B, A, A
            assert_eq!(i.rows[0].text, "Row 1 (RS): with B sc2, with A sc2 (4 sts)");
            assert!(i.rows.iter().all(|r| r.text.contains("sc") && !r.text.contains(" k") && !r.text.contains(" p")));
        }

        #[test]
        fn filet_rows_are_an_edge_stitch_plus_three_per_square() {
            // palette 0 cream = open, 5 black = filled. Row 1 right to left: open, open, filled, filled, filled, open
            let cells = right_to_left(&[(0, 2), (5, 3), (0, 1)]);
            let c = chart(Craft::Crochet, Technique::Filet, Construction::Flat, 6, 1, cells, 6);
            let i = instructions(&c);
            assert_eq!(i.rows[0].stitches, 3 * 6 + 1);
            assert_eq!(i.rows[0].text, "Row 1 (RS): dc, 2 open (ch 2, dc), 3 filled (dc 3), 1 open (ch 2, dc) (19 sts)");
        }
    }

    mod totals {
        use super::*;

        #[test]
        fn stitches_per_colour_add_up_to_the_chart() {
            let c = chart(Craft::Knit, Technique::Intarsia, Construction::Flat, 3, 2, vec![0, 1, 1, 2, 2, 2], 3);
            let t = instructions(&c).totals;
            assert_eq!(t.iter().map(|x| (x.label.as_str(), x.count)).collect::<Vec<_>>(), [("A", 1), ("B", 2), ("C", 3)]);
        }

        #[test]
        fn labels_run_a_to_z_then_aa() {
            assert_eq!(colour_label(0), "A");
            assert_eq!(colour_label(25), "Z");
            assert_eq!(colour_label(26), "AA");
            assert_eq!(colour_label(27), "AB");
            assert_eq!(colour_label(63), "BL");
        }
    }

    proptest! {
        #[test]
        fn every_rows_counts_add_up_to_the_row_width(
            w in 1u32..40, h in 1u32..12, colours in 1usize..7, seed in proptest::collection::vec(any::<u8>(), 480),
            craft in prop_oneof![Just(Craft::Knit), Just(Craft::Crochet)],
            technique in prop_oneof![Just(Technique::Tapestry), Just(Technique::Intarsia), Just(Technique::FairIsle), Just(Technique::Filet)],
            round in any::<bool>(),
        ) {
            let cells: Vec<u8> = (0..w * h).map(|i| seed[i as usize % seed.len()] % colours as u8).collect();
            let construction = if round { Construction::Round } else { Construction::Flat };
            let c = chart(craft, technique, construction, w, h, cells, colours);
            let i = instructions(&c);
            prop_assert_eq!(i.rows.len(), h as usize);
            for (y, r) in i.rows.iter().enumerate() {
                let total: u32 = r.runs.iter().map(|x| x.1).sum();
                prop_assert_eq!(total, w);
                let expected = if technique == Technique::Filet { 3 * w + 1 } else { w };
                prop_assert_eq!(r.stitches, expected);
                let tail = format!("({expected} sts)");
                prop_assert!(r.text.ends_with(&tail));
                prop_assert_eq!(r.number as usize, y + 1);
                // adjacent runs differ
                prop_assert!(r.runs.windows(2).all(|p| p[0].0 != p[1].0));
            }
            prop_assert_eq!(i.totals.iter().map(|t| t.count).sum::<u32>(), w * h);
        }
    }
}
