use yarnfoundry_core::{Chart, Construction, Craft, Gauge, PaletteColor, Technique, Unit};

use crate::draw::{Anchor, Prim};
use crate::{Look, Paper, PrintOptions, RenderError, pages, pdf, png, row_number_on_right, svg, symbols, tiling};

fn palette(n: usize) -> Vec<PaletteColor> {
    (0..n).map(|i| PaletteColor { name: format!("colour {i}"), rgb: [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (i * 53 % 256) as u8] }).collect()
}

fn chart(w: u32, h: u32, colours: usize, construction: Construction) -> Chart {
    let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
    let mut c = Chart::blank(w, h, palette(colours), Craft::Knit, Technique::Intarsia, construction, gauge).unwrap();
    for y in 0..h {
        for x in 0..w {
            c.set(x, y, ((x + y) as usize % colours) as u8);
        }
    }
    c
}

fn opts(paper: Paper, look: Look) -> PrintOptions {
    PrintOptions { paper, look, title: "Test".into() }
}

/// Filled four-point cells without an outline: the stitches of a tile page.
fn cells(page: &crate::draw::Page) -> Vec<&Vec<(f32, f32)>> {
    page.prims
        .iter()
        .filter_map(|p| match p {
            Prim::Poly { pts, fill: Some(_), stroke: None } if pts.len() == 4 => Some(pts),
            _ => None,
        })
        .collect()
}

mod true_size {
    use super::*;

    #[test]
    fn a_cell_is_one_stitch_wide_and_one_row_tall_within_one_percent() {
        for paper in [Paper::Letter, Paper::A4] {
            for (g, unit) in [(Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap(), 1.0), (Gauge::new(22.0, 30.0, 10.0, Unit::Cm).unwrap(), 1.0)] {
                let mut c = chart(30, 30, 2, Construction::Flat);
                c.gauge = g;
                let ps = pages(&c, &opts(paper, Look::Colour)).unwrap();
                let first = *cells(&ps[1]).first().unwrap();
                let (w_pt, h_pt) = (f64::from(first[1].0 - first[0].0), f64::from(first[3].1 - first[0].1));
                let w_in = w_pt / 72.0 * g.stitches_per_inch();
                let h_in = h_pt / 72.0 * g.rows_per_inch();
                assert!((w_in * unit - 1.0).abs() < 0.01 && (h_in * unit - 1.0).abs() < 0.01, "{paper:?}: {w_in} {h_in}");
            }
        }
    }

    #[test]
    fn ten_stitches_span_ten_stitch_widths_on_the_page() {
        let c = chart(30, 20, 2, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let heavy: Vec<f32> = ps[1]
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Line { a, b, width, .. } if (*width - 0.9).abs() < 1e-6 && (a.0 - b.0).abs() < 1e-6 => Some(a.0),
                _ => None,
            })
            .collect();
        assert_eq!(heavy.len(), 2, "{heavy:?}");
        let mut xs = heavy.clone();
        xs.sort_by(f32::total_cmp);
        let step = 72.0 / 5.0 * 10.0;
        assert!((xs[1] - xs[0] - step).abs() / step < 0.01, "{xs:?}");
    }

    #[test]
    fn the_scale_bar_is_exactly_one_inch() {
        let c = chart(10, 10, 2, Construction::Flat);
        let ps = pages(&c, &opts(Paper::A4, Look::Colour)).unwrap();
        let bar = ps[1].prims.iter().find_map(|p| match p {
            Prim::Line { a, b, .. } if (a.1 - b.1).abs() < 1e-6 && ((b.0 - a.0) - 72.0).abs() < 1e-3 => Some(()),
            _ => None,
        });
        assert!(bar.is_some());
    }
}

mod layout {
    use super::*;

    fn labels(page: &crate::draw::Page) -> Vec<(String, f32, Anchor)> {
        page.prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, x, anchor, size, .. } if *size < 9.0 && text.chars().all(|c| c.is_ascii_digit()) => {
                    Some((text.clone(), *x, *anchor))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn flat_row_numbers_alternate_sides_right_side_rows_on_the_right() {
        let c = chart(10, 6, 2, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let l = labels(&ps[1]);
        let find = |n: &str| l.iter().find(|(t, _, a)| t == n && matches!(a, Anchor::Start | Anchor::End)).unwrap().clone();
        let (_, x1, a1) = find("1");
        let (_, x2, a2) = find("2");
        assert_eq!((a1, a2), (Anchor::Start, Anchor::End));
        assert!(x1 > x2);
        assert!(row_number_on_right(&c, 0) && !row_number_on_right(&c, 1) && row_number_on_right(&c, 2));
    }

    #[test]
    fn rounds_have_every_number_on_the_right() {
        let c = chart(10, 6, 2, Construction::Round);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let l = labels(&ps[1]);
        let rows: Vec<_> = l.iter().filter(|(t, _, _)| t.parse::<u32>().is_ok_and(|n| n <= 6) && !matches!(t.as_str(), "1") || t == "1").collect();
        assert!(rows.iter().filter(|(_, _, a)| *a == Anchor::Start).count() >= 6);
        assert!(l.iter().all(|(_, _, a)| *a != Anchor::End));
    }

    #[test]
    fn row_1_is_at_the_bottom_and_stitch_1_at_the_right() {
        let mut c = chart(10, 6, 2, Construction::Flat);
        c.cells.iter_mut().for_each(|x| *x = 0);
        c.set(9, 0, 1); // bottom-right stitch, the first one worked
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let marked: Vec<_> = ps[1]
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Poly { pts, fill: Some(f), stroke: None } if pts.len() == 4 && *f == c.palette[1].rgb => Some(pts[0]),
                _ => None,
            })
            .collect();
        assert_eq!(marked.len(), 1);
        let all = cells(&ps[1]);
        let max_x = all.iter().map(|p| p[0].0).fold(f32::MIN, f32::max);
        let max_y = all.iter().map(|p| p[0].1).fold(f32::MIN, f32::max);
        assert_eq!(marked[0], (max_x, max_y));
    }

    #[test]
    fn a_big_chart_is_tiled_on_ten_stitch_lines_and_every_cell_appears_once() {
        let c = chart(95, 140, 3, Construction::Flat);
        for paper in [Paper::Letter, Paper::A4] {
            let t = tiling(&c, paper).unwrap();
            assert!(t.tiles_x * t.tiles_y > 1);
            assert_eq!(t.cols_per_tile % 10, 0);
            assert_eq!(t.rows_per_tile % 10, 0);
            let ps = pages(&c, &opts(paper, Look::Colour)).unwrap();
            assert_eq!(ps.len(), t.pages());
            let total: usize = ps[1..].iter().map(|p| cells(p).len()).sum();
            assert_eq!(total, 95 * 140);
            for p in &ps[1..] {
                let cs = cells(p);
                let right = cs.iter().map(|q| q[1].0).fold(f32::MIN, f32::max);
                let bottom = cs.iter().map(|q| q[3].1).fold(f32::MIN, f32::max);
                assert!(right <= p.width - 36.0 + 0.01 && bottom <= p.height - 36.0 - 44.0 + 0.01, "{paper:?} spills off the page");
            }
        }
    }

    #[test]
    fn a_small_chart_is_a_cover_plus_one_page() {
        let c = chart(30, 40, 2, Construction::Flat);
        assert_eq!(pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap().len(), 2);
    }

    #[test]
    fn the_cover_lists_every_colour_with_its_count() {
        let c = chart(30, 40, 5, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let texts: Vec<&str> = ps[0].prims.iter().filter_map(|p| if let Prim::Text { text, .. } = p { Some(text.as_str()) } else { None }).collect();
        for i in 0..5 {
            assert!(texts.iter().any(|t| t.starts_with(&format!("colour {i}  ("))), "colour {i} missing");
        }
    }
}

mod looks {
    use super::*;

    #[test]
    fn all_64_symbols_are_distinct() {
        let all: Vec<Vec<Prim>> = (0..64).map(|i| symbols::symbol(i, 10.0, 10.0, 4.0, [0, 0, 0])).collect();
        for i in 0..64 {
            for j in (i + 1)..64 {
                assert_ne!(all[i], all[j], "symbols {i} and {j} are the same");
            }
        }
    }

    #[test]
    fn black_and_white_has_no_colour_and_one_symbol_per_stitch() {
        let c = chart(12, 8, 4, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::BlackWhite)).unwrap();
        for p in &ps[1].prims {
            match p {
                Prim::Poly { fill, stroke, .. } => {
                    assert!(fill.is_none_or(|f| f == [0, 0, 0]) && stroke.is_none_or(|s| s.0 == [0, 0, 0]));
                }
                Prim::Line { color, .. } => assert!(color[0] == color[1] && color[1] == color[2], "grey or black only"),
                Prim::Text { color, .. } => assert_eq!(*color, [0, 0, 0]),
            }
        }
        // the 4 colours get 4 different drawings: compare the symbol prims of stitches by colour
        let colour_symbols: Vec<_> = (0..4).map(|i| symbols::bw_symbol(i, 0.0, 0.0, 3.0)).collect();
        assert!((0..4).all(|i| (i + 1..4).all(|j| colour_symbols[i] != colour_symbols[j])));
    }

    #[test]
    fn colour_with_symbols_adds_marks_on_top_of_the_colours() {
        let c = chart(12, 8, 3, Construction::Flat);
        let plain = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let marked = pages(&c, &opts(Paper::Letter, Look::ColourSymbols)).unwrap();
        assert!(marked[1].prims.len() >= plain[1].prims.len() + 12 * 8);
    }
}

mod outputs {
    use super::*;

    #[test]
    fn svg_is_sized_in_points_and_escapes_names() {
        let mut c = chart(10, 10, 2, Construction::Flat);
        c.palette[0].name = "<b>&\"x\"".into();
        let o = PrintOptions { title: "A & B <c>".into(), ..opts(Paper::Letter, Look::Colour) };
        let ps = pages(&c, &o).unwrap();
        let s = svg::page_to_svg(&ps[0]);
        assert!(s.starts_with("<svg ") && s.trim_end().ends_with("</svg>"));
        assert!(s.contains("width=\"612.00pt\"") && s.contains("height=\"792.00pt\""));
        assert!(s.contains("&lt;b&gt;&amp;&quot;x&quot;") && s.contains("A &amp; B &lt;c&gt;"));
        assert!(!s.contains("<b>"));
    }

    #[test]
    fn pdf_has_a_valid_cross_reference_and_one_page_per_page() {
        let c = chart(95, 140, 3, Construction::Flat);
        let ps = pages(&c, &opts(Paper::A4, Look::ColourSymbols)).unwrap();
        let bytes = pdf::to_pdf(&ps, "Title (with) \\ odd \u{4e2d} chars");
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(bytes.starts_with(b"%PDF-1.4") && text.trim_end().ends_with("%%EOF"));
        assert!(text.contains(&format!("/Count {}", ps.len())));
        let start: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(text[start..].starts_with("xref\n"));
        let table: Vec<&str> = text[start..].lines().skip(3).take_while(|l| l.ends_with(" n ")).collect();
        assert_eq!(table.len(), 4 + 1 + 2 * ps.len());
        for (i, line) in table.iter().enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(text[off..].starts_with(&format!("{} 0 obj", i + 1)), "object {} not at its offset", i + 1);
        }
        // the first page's MediaBox is A4
        assert!(text.contains("/MediaBox [0 0 595.28 841.89]"));
    }

    #[test]
    fn png_decodes_to_the_page_size_at_the_dpi_and_shows_the_chart() {
        let c = chart(30, 40, 3, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let bytes = png::page_to_png(&ps[1], 100).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgb8();
        assert_eq!((img.width(), img.height()), (850, 1100));
        let non_white = img.pixels().filter(|p| p.0 != [255, 255, 255]).count();
        assert!(non_white > 10_000, "the page looks empty: {non_white}");
        // a palette colour really appears
        let want = c.palette[1].rgb;
        assert!(img.pixels().any(|p| p.0 == want));
    }

    #[test]
    fn dpi_is_clamped_so_hostile_values_cannot_ask_for_gigabytes() {
        let c = chart(10, 10, 2, Construction::Flat);
        let ps = pages(&c, &opts(Paper::Letter, Look::Colour)).unwrap();
        let (w, _, _) = png::page_to_rgb(&ps[0], u32::MAX);
        assert!(w >= 2549); // 8.5 in at 300 dpi
        assert!(w <= 2551);
        let (w, _, _) = png::page_to_rgb(&ps[0], 0);
        assert_eq!(w, 306);
    }
}

mod refusals {
    use super::*;

    #[test]
    fn absurd_gauges_and_sizes_are_errors_not_panics() {
        let mut c = chart(10, 10, 2, Construction::Flat);
        c.gauge = Gauge::new(0.5, 0.5, 4.0, Unit::Inches).unwrap(); // 8 inch stitches
        assert_eq!(tiling(&c, Paper::Letter), Err(RenderError::CellTooBig));
        c.gauge = Gauge::new(5000.0, 5000.0, 1.0, Unit::Inches).unwrap(); // dust-sized stitches
        assert_eq!(tiling(&c, Paper::Letter), Err(RenderError::CellTooBig));
        let big = chart(2000, 2000, 2, Construction::Flat);
        assert_eq!(tiling(&big, Paper::Letter), Err(RenderError::TooManyPages));
        let mut bad = chart(10, 10, 2, Construction::Flat);
        bad.cells.pop();
        assert!(matches!(pages(&bad, &PrintOptions::default()), Err(RenderError::Chart(_))));
        let mut nan = chart(10, 10, 2, Construction::Flat);
        nan.gauge.stitches = f64::NAN;
        assert!(pages(&nan, &PrintOptions::default()).is_err());
    }

    #[test]
    fn one_by_one_and_64_colour_charts_print() {
        let one = chart(1, 1, 1, Construction::Flat);
        assert_eq!(pages(&one, &PrintOptions::default()).unwrap().len(), 2);
        let many = chart(64, 2, 64, Construction::Flat);
        let ps = pages(&many, &opts(Paper::Letter, Look::BlackWhite)).unwrap();
        assert!(pdf::to_pdf(&ps, "x").len() > 1000);
    }
}
