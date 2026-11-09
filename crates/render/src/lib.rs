//! YarnFoundry render: turns a chart into printable pages (SVG, PDF, PNG), true size, tiled over several pages when big.
//!
//! [`pages`] lays the chart out as a list of [`draw::Page`]s; [`svg::page_to_svg`], [`pdf::to_pdf`] and [`png::page_to_png`] draw them.
//! Layout rules: page 1 is the cover with the legend (and a page map when the chart is tiled); each following page is a tile.
//! Cells are `72 / stitches-per-inch` wide and `72 / rows-per-inch` tall, so a 100% print matches the gauge.
//! Stitch numbers count from the right edge, rows from the bottom. On flat work, right-side (odd) rows read right to left
//! and have their number on the right; wrong-side (even) rows read left to right and have it on the left. Rounds: always right.

pub mod booklet;
pub mod draw;
pub mod pdf;
pub mod png;
pub mod svg;
pub mod symbols;
pub mod view3d;

use yarnfoundry_core::{Chart, Construction, Craft, Technique, Unit};

use draw::{Anchor, BLACK, Page, Rgb, WHITE};

/// Most pages one print may have (cover included): a guard against absurd gauges.
pub const MAX_PAGES: usize = 400;

const MARGIN: f32 = 36.0;
const GUTTER: f32 = 22.0;
const HEADER: f32 = 34.0;
const FOOTER: f32 = 44.0;
const GRID_THIN: Rgb = [170, 170, 170];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    Letter,
    A4,
}

impl Paper {
    /// Width and height in points.
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::Letter => (612.0, 792.0),
            Paper::A4 => (595.28, 841.89),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "letter" => Some(Self::Letter),
            "a4" => Some(Self::A4),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// Colour cells only.
    Colour,
    /// Colour cells with the stitch symbol on top (helps colour-blind knitters and mixed lighting).
    ColourSymbols,
    /// White cells and black symbols only: prints on any printer and photocopies.
    BlackWhite,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrintOptions {
    pub paper: Paper,
    pub look: Look,
    pub title: String,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self { paper: Paper::Letter, look: Look::Colour, title: "YarnFoundry chart".to_string() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    #[error("{0}")]
    Chart(String),
    #[error("one stitch would be larger than the page; check the gauge")]
    CellTooBig,
    #[error("this chart needs more than {MAX_PAGES} pages at true size; make it smaller")]
    TooManyPages,
    #[error("could not encode the picture: {0}")]
    Encode(String),
    #[error("page {0} does not exist")]
    NoSuchPage(usize),
    #[error("the bundled font could not be read")]
    Font,
}

/// How a chart is cut into pages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tiling {
    pub cell_w: f32,
    pub cell_h: f32,
    pub cols_per_tile: u32,
    pub rows_per_tile: u32,
    pub tiles_x: u32,
    pub tiles_y: u32,
}

impl Tiling {
    pub fn pages(&self) -> usize {
        1 + (self.tiles_x as usize) * (self.tiles_y as usize)
    }
}

/// Computes the cell size and the tile grid for `chart` on `paper`.
pub fn tiling(chart: &Chart, paper: Paper) -> Result<Tiling, RenderError> {
    chart.validate().map_err(|e| RenderError::Chart(e.to_string()))?;
    let cell_w = (72.0 / chart.gauge.stitches_per_inch()) as f32;
    let cell_h = (72.0 / chart.gauge.rows_per_inch()) as f32;
    let (pw, ph) = paper.size();
    let (avail_w, avail_h) = (pw - 2.0 * MARGIN - 2.0 * GUTTER, ph - 2.0 * MARGIN - HEADER - FOOTER);
    if !(cell_w.is_finite() && cell_h.is_finite() && cell_w > 0.5 && cell_h > 0.5) || cell_w > avail_w || cell_h > avail_h {
        return Err(RenderError::CellTooBig);
    }
    let fit = |avail: f32, cell: f32, total: u32| {
        let n = (avail / cell).floor() as u32;
        if total <= n {
            total
        } else if n >= 10 {
            n / 10 * 10 // tile edges fall on the 10-stitch lines
        } else {
            n.max(1)
        }
    };
    let cols_per_tile = fit(avail_w, cell_w, chart.width);
    let rows_per_tile = fit(avail_h, cell_h, chart.height);
    let t = Tiling {
        cell_w,
        cell_h,
        cols_per_tile,
        rows_per_tile,
        tiles_x: chart.width.div_ceil(cols_per_tile),
        tiles_y: chart.height.div_ceil(rows_per_tile),
    };
    if t.pages() > MAX_PAGES { Err(RenderError::TooManyPages) } else { Ok(t) }
}

/// Which side of a row its number goes on: true = right.
pub fn row_number_on_right(chart: &Chart, row_index: u32) -> bool {
    chart.construction == Construction::Round || (row_index + 1) % 2 == 1
}

/// The pages of the printable chart: cover and legend first, then the tiles (bottom band first, right to left).
pub fn pages(chart: &Chart, opts: &PrintOptions) -> Result<Vec<Page>, RenderError> {
    let t = tiling(chart, opts.paper)?;
    let total = t.pages();
    let mut out = vec![cover_page(chart, opts, &t)];
    let mut n = 2;
    for ty in 0..t.tiles_y {
        for tx in 0..t.tiles_x {
            out.push(tile_page(chart, opts, &t, tx, ty, n, total));
            n += 1;
        }
    }
    Ok(out)
}

fn ink_for(rgb: Rgb, look: Look) -> Rgb {
    if look == Look::BlackWhite {
        return BLACK;
    }
    let lum = 0.299 * f32::from(rgb[0]) + 0.587 * f32::from(rgb[1]) + 0.114 * f32::from(rgb[2]);
    if lum > 140.0 { BLACK } else { WHITE }
}

fn unit_name(u: Unit) -> &'static str {
    match u {
        Unit::Inches => "in",
        Unit::Cm => "cm",
    }
}

fn reading_note(chart: &Chart) -> &'static str {
    match chart.construction {
        Construction::Round => "Work every round from right to left. Row numbers are on the right.",
        Construction::Flat => "Right-side rows (numbers on the right) read right to left; wrong-side rows (numbers on the left) read left to right.",
    }
}

fn technique_name(t: Technique) -> &'static str {
    match t {
        Technique::Tapestry => "tapestry",
        Technique::Intarsia => "intarsia",
        Technique::FairIsle => "fair isle",
        Technique::Filet => "filet",
    }
}

fn cover_page(chart: &Chart, opts: &PrintOptions, t: &Tiling) -> Page {
    let (pw, ph) = opts.paper.size();
    let mut p = Page::new(pw, ph);
    p.text(MARGIN, MARGIN + 18.0, 20.0, &opts.title, Anchor::Start, true);
    let (fw, fh) = chart.finished_size();
    let u = unit_name(chart.gauge.unit);
    let g = chart.gauge;
    let craft = if chart.craft == Craft::Knit { "knit" } else { "crochet" };
    let lines = [
        format!("{} stitches x {} rows, about {fw:.1} x {fh:.1} {u} finished", chart.width, chart.height),
        format!("Gauge: {} stitches and {} rows over {} {u}", g.stitches, g.rows, g.measure),
        format!("{craft}, {}, {}", technique_name(chart.technique), if chart.construction == Construction::Round { "in the round" } else { "flat" }),
        reading_note(chart).to_string(),
        format!(
            "Print at 100% / actual size. {}",
            if t.pages() > 2 {
                format!("This chart is {} pages: see the page map below.", t.pages() - 1)
            } else {
                "The chart fits on one page.".to_string()
            }
        ),
    ];
    let mut y = MARGIN + 40.0;
    for l in &lines {
        p.text(MARGIN, y, 9.5, l, Anchor::Start, false);
        y += 14.0;
    }
    y += 10.0;
    p.text(MARGIN, y, 13.0, "Colours", Anchor::Start, true);
    y += 8.0;
    let mut counts = vec![0u32; chart.palette.len()];
    for &c in &chart.cells {
        if let Some(n) = counts.get_mut(usize::from(c)) {
            *n += 1;
        }
    }
    let per_col = if chart.palette.len() > 24 { chart.palette.len().div_ceil(2) } else { chart.palette.len() };
    let col_w = (pw - 2.0 * MARGIN) / 2.0;
    let row_h = 17.0;
    let mut legend_bottom = y;
    for (i, colour) in chart.palette.iter().enumerate() {
        let (col, row) = (i / per_col.max(1), i % per_col.max(1));
        let x = MARGIN + col as f32 * col_w;
        let top = y + row as f32 * row_h;
        let (fill, ink) = match opts.look {
            Look::BlackWhite => (WHITE, BLACK),
            _ => (colour.rgb, ink_for(colour.rgb, opts.look)),
        };
        p.rect(x, top, 14.0, 14.0, Some(fill), Some((BLACK, 0.6)));
        if opts.look != Look::Colour {
            p.prims.extend(symbols::symbol(i, x + 7.0, top + 7.0, 4.6, ink));
        }
        let label = format!("{}  ({} stitches)", colour.name, counts.get(i).copied().unwrap_or(0));
        p.text(x + 20.0, top + 11.0, 9.5, &label, Anchor::Start, false);
        legend_bottom = legend_bottom.max(top + row_h);
    }
    if t.pages() > 2 {
        page_map(&mut p, t, MARGIN, legend_bottom + 24.0, pw - 2.0 * MARGIN, ph - MARGIN - legend_bottom - 40.0);
    }
    p
}

/// Small map of the tiles, each box labelled with its page number.
fn page_map(p: &mut Page, t: &Tiling, x: f32, y: f32, max_w: f32, max_h: f32) {
    p.text(x, y, 13.0, "Page map (stitch 1 is at the right, row 1 at the bottom)", Anchor::Start, true);
    let top = y + 10.0;
    let side = (max_w / t.tiles_x as f32).min((max_h - 10.0) / t.tiles_y as f32).clamp(4.0, 36.0);
    let (mw, mh) = (side * t.tiles_x as f32, side * t.tiles_y as f32);
    for ty in 0..t.tiles_y {
        for tx in 0..t.tiles_x {
            let page = 2 + ty * t.tiles_x + tx;
            // right-most tile first: tx = 0 is drawn at the right
            let bx = x + mw - (tx as f32 + 1.0) * side;
            let by = top + mh - (ty as f32 + 1.0) * side;
            p.rect(bx, by, side, side, None, Some((BLACK, 0.6)));
            if side >= 12.0 {
                p.text(bx + side / 2.0, by + side / 2.0 + 3.0, 8.0, &page.to_string(), Anchor::Middle, false);
            }
        }
    }
}

fn tile_page(chart: &Chart, opts: &PrintOptions, t: &Tiling, tx: u32, ty: u32, page_no: u32, total: usize) -> Page {
    let (pw, ph) = opts.paper.size();
    let mut p = Page::new(pw, ph);
    // Stitch numbers n = 1..=width count from the right; rows r = 0..height from the bottom.
    let n_lo = tx * t.cols_per_tile + 1;
    let n_hi = ((tx + 1) * t.cols_per_tile).min(chart.width);
    let r_lo = ty * t.rows_per_tile;
    let r_hi = ((ty + 1) * t.rows_per_tile).min(chart.height);
    let (cols, rows) = (n_hi - n_lo + 1, r_hi - r_lo);
    let (cw, ch) = (t.cell_w, t.cell_h);
    let left = MARGIN + GUTTER;
    let top = MARGIN + HEADER;
    let (grid_w, grid_h) = (cols as f32 * cw, rows as f32 * ch);

    p.text(MARGIN, MARGIN + 12.0, 12.0, &opts.title, Anchor::Start, true);
    p.text(pw - MARGIN, MARGIN + 12.0, 10.0, &format!("Page {} of {}", page_no, total), Anchor::End, false);
    p.text(
        MARGIN,
        MARGIN + 25.0,
        8.0,
        &format!("Stitches {n_lo}-{n_hi} (from the right), rows {}-{}.  {}", r_lo + 1, r_hi, reading_note(chart)),
        Anchor::Start,
        false,
    );

    let x_of = |n: u32| left + (n_hi - n) as f32 * cw; // left edge of stitch number n
    let y_of = |r: u32| top + (r_hi - 1 - r) as f32 * ch; // top edge of row r
    for r in r_lo..r_hi {
        for n in n_lo..=n_hi {
            let x = chart.width - n;
            let Some(c) = chart.get(x, r) else { continue };
            let idx = usize::from(c);
            let rgb = chart.palette.get(idx).map_or(WHITE, |c| c.rgb);
            let (cx, cy) = (x_of(n), y_of(r));
            if opts.look != Look::BlackWhite {
                p.rect(cx, cy, cw, ch, Some(rgb), None);
            }
            if opts.look != Look::Colour {
                let rr = (cw.min(ch) * 0.34).max(0.8);
                p.prims.extend(symbols::symbol(idx, cx + cw / 2.0, cy + ch / 2.0, rr, ink_for(rgb, opts.look)));
            }
        }
    }
    // grid: thin on every stitch, heavy every 10 stitches / rows
    for n in n_lo..=n_hi + 1 {
        let x = left + (n_hi + 1 - n) as f32 * cw;
        let heavy = (n - 1) % 10 == 0 && n != n_lo && n != n_hi + 1;
        let edge = n == n_lo || n == n_hi + 1;
        let (w, color) = if edge {
            (1.2, BLACK)
        } else if heavy {
            (0.9, BLACK)
        } else {
            (0.25, GRID_THIN)
        };
        p.line((x, top), (x, top + grid_h), color, w);
    }
    for r in r_lo..=r_hi {
        let y = top + (r_hi - r) as f32 * ch;
        let heavy = r % 10 == 0 && r != r_lo && r != r_hi;
        let edge = r == r_lo || r == r_hi;
        let (w, color) = if edge {
            (1.2, BLACK)
        } else if heavy {
            (0.9, BLACK)
        } else {
            (0.25, GRID_THIN)
        };
        p.line((left, y), (left + grid_w, y), color, w);
    }
    // row numbers on the side each row is worked from
    let nsize = (ch * 0.7).clamp(5.0, 8.0);
    for r in r_lo..r_hi {
        let ybase = y_of(r) + ch / 2.0 + nsize * 0.35;
        let label = (r + 1).to_string();
        if row_number_on_right(chart, r) {
            p.text(left + grid_w + 3.0, ybase, nsize, &label, Anchor::Start, false);
        } else {
            p.text(left - 3.0, ybase, nsize, &label, Anchor::End, false);
        }
    }
    // stitch numbers under the grid: 1, then every 10th
    for n in n_lo..=n_hi {
        if n == 1 || n % 10 == 0 {
            p.text(x_of(n) + cw / 2.0, top + grid_h + 10.0, nsize, &n.to_string(), Anchor::Middle, false);
        }
    }
    // a 1 inch bar to check the print scale
    let by = ph - MARGIN - 8.0;
    p.line((MARGIN, by), (MARGIN + 72.0, by), BLACK, 1.0);
    p.line((MARGIN, by - 3.0), (MARGIN, by + 3.0), BLACK, 1.0);
    p.line((MARGIN + 72.0, by - 3.0), (MARGIN + 72.0, by + 3.0), BLACK, 1.0);
    p.text(MARGIN + 80.0, by + 3.0, 8.0, "This bar is exactly 1 inch (2.54 cm) when printed at 100% / actual size.", Anchor::Start, false);
    p
}

#[cfg(test)]
mod tests;
