//! The printable amigurumi pattern: cover with the preview render, materials, abbreviations, rounds per part and
//! assembly notes with a simple diagram. Every page has a footer (page x of y). Text is plain ASCII (the PDF uses
//! the standard fonts). Pages are [`Page`]s, so the same list becomes PDF, SVG or PNG.

use yarnfoundry_core::{Figure, Gauge, Mesh, PartPattern, Terms, YarnWeight};

use crate::Paper;
use crate::draw::{Anchor, Page, Prim, circle_pts, text_width};
use crate::view3d::{Camera, Fit, Frame, PART_COLOURS, draw_figure, figure_polys, project};

const MARGIN: f32 = 54.0;
const FOOTER: f32 = 40.0;
const INK: [u8; 3] = [40, 40, 40];
const SOFT: [u8; 3] = [110, 104, 96];

/// Everything the booklet needs.
pub struct BookletInput<'a> {
    pub title: &'a str,
    pub designer: &'a str,
    pub paper: Paper,
    pub mesh: &'a Mesh,
    pub figure: &'a Figure,
    pub patterns: &'a [PartPattern],
    pub terms: Terms,
    pub weight: YarnWeight,
    pub gauge: &'a Gauge,
    /// Finished height in inches.
    pub height_in: f64,
}

/// Lays text out down the pages, starting a new page when one is full.
struct Flow {
    pages: Vec<Page>,
    w: f32,
    h: f32,
    y: f32,
}

impl Flow {
    fn new(paper: Paper) -> Self {
        let (w, h) = paper.size();
        let mut f = Self { pages: Vec::new(), w, h, y: MARGIN };
        f.new_page();
        f
    }

    fn new_page(&mut self) {
        self.pages.push(Page::new(self.w, self.h));
        self.y = MARGIN;
    }

    fn page(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.pages.push(Page::new(self.w, self.h));
        }
        let last = self.pages.len() - 1;
        &mut self.pages[last]
    }

    fn need(&mut self, height: f32) {
        if self.y + height > self.h - FOOTER - 10.0 {
            self.new_page();
        }
    }

    fn text(&mut self, size: f32, text: &str, bold: bool, indent: f32) {
        self.need(size * 1.5);
        let (x, y) = (MARGIN + indent, self.y + size);
        self.page().prims.push(Prim::Text { x, y, size, text: text.to_string(), anchor: Anchor::Start, color: INK, bold });
        self.y += size * 1.5;
    }

    fn heading(&mut self, text: &str, size: f32) {
        self.y += size * 0.5;
        self.need(size * 3.0);
        self.text(size, text, true, 0.0);
        self.y += 2.0;
    }

    /// A paragraph wrapped to the page width.
    fn para(&mut self, size: f32, text: &str, indent: f32) {
        let width = self.w - 2.0 * MARGIN - indent;
        let mut line = String::new();
        for word in text.split_whitespace() {
            let trial = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if text_width(&trial, size) > width && !line.is_empty() {
                self.text(size, &line, false, indent);
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        if !line.is_empty() {
            self.text(size, &line, false, indent);
        }
    }

    fn gap(&mut self, h: f32) {
        self.y += h;
    }
}

/// Plain ASCII, so nothing turns into a question mark in the PDF.
fn ascii(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii() { c } else { '-' }).collect()
}

/// A safety-eye size in mm that suits the head (about 12% of its width), kept to common sizes.
pub fn eye_size_mm(figure: &Figure) -> u32 {
    let head =
        figure.parts.iter().find(|p| p.name == "Head").or_else(|| figure.parts.iter().max_by(|a, b| a.peak_perimeter.total_cmp(&b.peak_perimeter)));
    let width_in = head.map_or(2.0, |h| h.peak_perimeter / std::f64::consts::PI);
    let mm = width_in * 25.4 * 0.12;
    *[6u32, 8, 10, 12, 15, 18, 20].iter().min_by_key(|s| (f64::from(**s) - mm).abs() as u32).unwrap_or(&8)
}

/// The booklet pages.
pub fn booklet_pages(b: &BookletInput<'_>) -> Vec<Page> {
    let mut f = Flow::new(b.paper);
    let (pw, ph) = (f.w, f.h);
    let cw = pw - 2.0 * MARGIN;
    let total_yards: f64 = b.patterns.iter().map(|p| p.yards).sum();
    let total_rounds: usize = b.patterns.iter().map(|p| p.rounds.len()).sum();
    let total_sts: u32 = b.patterns.iter().map(|p| p.total_stitches).sum();
    let (cm_h, in_h) = (b.height_in * 2.54, b.height_in);

    // Cover.
    f.page().rect(0.0, 0.0, pw, ph, Some([248, 244, 237]), None);
    f.y = MARGIN + 20.0;
    f.text(30.0, &ascii(b.title), true, 0.0);
    f.text(13.0, &format!("A crocheted figure, about {in_h:.1} in ({cm_h:.0} cm) tall, made in {} part(s)", b.figure.parts.len()), false, 0.0);
    if !b.designer.trim().is_empty() {
        f.text(12.0, &format!("Designed by {}", ascii(b.designer.trim())), false, 0.0);
    }
    let hero = Frame { x: MARGIN, y: f.y + 16.0, w: cw, h: (ph - f.y - 150.0).max(200.0) };
    let fit = Fit::of_figure(b.figure);
    let polys = figure_polys(b.figure, &Camera { yaw: 0.5, pitch: 0.25, zoom: 1.0 }, &fit, hero, None);
    crate::view3d::draw_polys(f.page(), polys);
    f.y = hero.y + hero.h + 24.0;
    f.text(
        12.0,
        &format!(
            "{total_rounds} rounds | {total_sts} stitches | about {:.0} yd of {} yarn (an estimate)",
            total_yards.ceil(),
            b.weight.label().to_lowercase()
        ),
        false,
        0.0,
    );
    f.text(10.0, "Rounds are drawn as rings; every 5th round is lighter so you can count along.", false, 0.0);

    // Materials.
    f.new_page();
    f.heading("Materials", 18.0);
    let (us, mm) = b.weight.amigurumi_hook();
    let yards = total_yards.ceil();
    f.para(
        11.0,
        &format!(
            "Yarn: {} weight, about {yards:.0} yd ({:.0} m) in all. This is an estimate from stitch counts; buy a little extra.",
            b.weight.label().to_lowercase(),
            yards * 0.9144
        ),
        0.0,
    );
    f.para(11.0, &format!("Hook: US {us} ({mm}). Use a hook a size or two smaller than the yarn label suggests so the fabric is tight and the stuffing does not show."), 0.0);
    f.para(11.0, &format!("Safety eyes: 2 x {} mm (or embroider them for a toy for a small child).", eye_size_mm(b.figure)), 0.0);
    f.para(11.0, "Polyester fiberfill stuffing, 1 stitch marker, a tapestry needle and scissors.", 0.0);
    f.heading("Gauge and size", 14.0);
    f.para(
        11.0,
        &format!(
            "{:.0} {} and {:.0} rounds = 4 in (10 cm) in the round. Finished figure about {in_h:.1} in ({cm_h:.0} cm) tall. A different gauge makes a different size.",
            b.gauge.stitches,
            b.terms.plain(),
            b.gauge.rows
        ),
        0.0,
    );
    f.heading("Parts", 14.0);
    for (i, p) in b.patterns.iter().enumerate() {
        let dot = PART_COLOURS[i % PART_COLOURS.len()];
        let y = f.y;
        f.page().rect(MARGIN, y + 3.0, 10.0, 10.0, Some(dot), None);
        f.text(
            11.0,
            &format!("{}: {} rounds, {} stitches, about {:.0} yd", ascii(&p.name), p.rounds.len(), p.total_stitches, p.yards.ceil()),
            false,
            16.0,
        );
    }
    f.heading("Abbreviations", 14.0);
    for (short, long) in yarnfoundry_core::pattern::abbreviations(b.terms) {
        f.para(11.0, &format!("{short} = {}", ascii(&long)), 0.0);
    }
    f.heading("Notes", 14.0);
    f.para(11.0, "Work in a spiral without joining; mark the first stitch of each round. Stuff as you go (see each part). The number in brackets at the end of a round is the stitch count you should have.", 0.0);

    // Rounds per part.
    for p in b.patterns {
        f.new_page();
        f.heading(&format!("{} ({} rounds, {} stitches)", ascii(&p.name), p.rounds.len(), p.total_stitches), 18.0);
        for r in &p.rounds {
            f.para(11.0, &ascii(&r.text), 0.0);
        }
        f.gap(6.0);
        for line in &p.finishing {
            f.para(11.0, &ascii(line), 0.0);
        }
        for n in &p.notes {
            f.para(11.0, &format!("Note: {}", ascii(n)), 0.0);
        }
    }

    // Assembly with a diagram.
    f.new_page();
    f.heading("Assembly", 18.0);
    let diagram = Frame { x: MARGIN, y: f.y + 6.0, w: cw, h: (ph * 0.38).min(330.0) };
    let cam = Camera { yaw: 0.0, pitch: 0.05, zoom: 1.0 };
    draw_figure(f.page(), b.figure, &cam, diagram, None);
    for (i, part) in b.figure.parts.iter().enumerate() {
        let n = part.rounds.len().max(1) as f64;
        let c = [
            part.rounds.iter().map(|r| r.center[0]).sum::<f64>() / n,
            part.rounds.iter().map(|r| r.center[1]).sum::<f64>() / n,
            part.rounds.iter().map(|r| r.z).sum::<f64>() / n,
        ];
        let (x, y) = project(&cam, &fit, diagram, c);
        let page = f.page();
        page.prims.push(Prim::Poly { pts: circle_pts(x, y, 8.0), fill: Some([255, 255, 255]), stroke: Some((INK, 1.0)) });
        page.prims.push(Prim::Text { x, y: y + 3.5, size: 10.0, text: format!("{}", i + 1), anchor: Anchor::Middle, color: INK, bold: true });
    }
    f.y = diagram.y + diagram.h + 14.0;
    for (i, part) in b.figure.parts.iter().enumerate() {
        f.text(11.0, &format!("{}  {}", i + 1, ascii(&part.name)), false, 0.0);
    }
    f.gap(6.0);
    let notes = b.figure.assembly_notes();
    if notes.is_empty() {
        f.para(11.0, "This figure is made in one piece; nothing to sew together.", 0.0);
    }
    for n in notes {
        f.para(11.0, &ascii(&n), 0.0);
    }
    f.para(
        11.0,
        "Pin the parts in place and check the figure from the front and the side before you sew. Use the long tails and small, even stitches.",
        0.0,
    );

    // Footers.
    let total = f.pages.len();
    let designer = if b.designer.trim().is_empty() { String::new() } else { format!("{} | ", ascii(b.designer.trim())) };
    for (i, page) in f.pages.iter_mut().enumerate() {
        let y = ph - 22.0;
        page.prims.push(Prim::Text {
            x: MARGIN,
            y,
            size: 9.0,
            text: format!("{designer}Made with YarnFoundry"),
            anchor: Anchor::Start,
            color: SOFT,
            bold: false,
        });
        page.prims.push(Prim::Text {
            x: pw - MARGIN,
            y,
            size: 9.0,
            text: format!("Page {} of {total}", i + 1),
            anchor: Anchor::End,
            color: SOFT,
            bold: false,
        });
    }
    f.pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use yarnfoundry_core::{Gauge, ManualSplits, Unit, models, split_figure};

    fn input_for(name: &str, height: f64, terms: Terms) -> (Mesh, Figure, Vec<PartPattern>, Gauge) {
        let weight = YarnWeight::Worsted;
        let (s, r) = weight.amigurumi_gauge();
        let g = Gauge::new(s, r, 4.0, Unit::Inches).unwrap();
        let mesh = models::sample(name).unwrap().fitted(height, Unit::Inches, None).unwrap();
        let fig = split_figure(&mesh, &g, &ManualSplits::default()).unwrap();
        let pats = fig.patterns(terms, weight, &g, None);
        (mesh, fig, pats, g)
    }

    #[test]
    fn the_bear_booklet_has_every_section_and_a_footer_on_every_page() {
        let (mesh, fig, pats, g) = input_for("bear", 8.0, Terms::Us);
        let b = BookletInput {
            title: "Original Bear",
            designer: "Test Designer",
            paper: Paper::Letter,
            mesh: &mesh,
            figure: &fig,
            patterns: &pats,
            terms: Terms::Us,
            weight: YarnWeight::Worsted,
            gauge: &g,
            height_in: 8.0,
        };
        let pages = booklet_pages(&b);
        assert!(pages.len() >= 5, "{} pages", pages.len());
        let text = |p: &Page| -> String {
            p.prims.iter().filter_map(|x| if let Prim::Text { text, .. } = x { Some(text.clone()) } else { None }).collect::<Vec<_>>().join("\n")
        };
        let all: String = pages.iter().map(text).collect::<Vec<_>>().join("\n");
        for needle in [
            "Original Bear",
            "Materials",
            "Hook: US E-4 (3.5 mm)",
            "Safety eyes",
            "Abbreviations",
            "Assembly",
            "Rnd 1: 6 sc in magic ring (6)",
            "Sew the open end",
            "Made with YarnFoundry",
            "Designed by Test Designer",
        ] {
            assert!(all.contains(needle), "missing {needle:?}");
        }
        for (i, p) in pages.iter().enumerate() {
            assert!(text(p).contains(&format!("Page {} of {}", i + 1, pages.len())));
            for prim in &p.prims {
                if let Prim::Text { x, y, .. } = prim {
                    assert!(*x >= 0.0 && *x <= p.width && *y >= 0.0 && *y <= p.height, "text off the page");
                }
            }
        }
        for part in &fig.parts {
            assert!(all.contains(&format!("{} (", part.name)));
        }
        let pdf = crate::pdf::to_pdf(&pages, "Original Bear");
        assert!(pdf.starts_with(b"%PDF-") && pdf.len() < 10_000_000, "{} bytes", pdf.len());
    }

    #[test]
    fn a4_and_one_piece_figures_work_and_text_is_ascii() {
        let (mesh, fig, pats, g) = input_for("sphere", 3.0, Terms::Uk);
        let b = BookletInput {
            title: "Caf\u{e9} Ball \u{2014} test",
            designer: "",
            paper: Paper::A4,
            mesh: &mesh,
            figure: &fig,
            patterns: &pats,
            terms: Terms::Uk,
            weight: YarnWeight::Worsted,
            gauge: &g,
            height_in: 3.0,
        };
        let pages = booklet_pages(&b);
        assert!((pages[0].width - 595.28).abs() < 0.01);
        let mut found_uk = false;
        for p in &pages {
            for prim in &p.prims {
                if let Prim::Text { text, .. } = prim {
                    assert!(text.is_ascii(), "{text:?}");
                    found_uk |= text.contains("dc in magic ring");
                }
            }
        }
        assert!(found_uk && eye_size_mm(&fig) >= 6);
    }
}
