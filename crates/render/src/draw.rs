//! A tiny display list: pages are lists of [`Prim`]s in points (1/72 inch), y down from the top edge.
//! The SVG, PDF and PNG back ends all draw the same list, so the three outputs agree.

use ab_glyph::{Font, FontRef, ScaleFont};

static FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");

pub type Rgb = [u8; 3];

pub const BLACK: Rgb = [0, 0, 0];
pub const WHITE: Rgb = [255, 255, 255];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    /// A closed polygon, filled and/or outlined (rectangles are four points).
    Poly {
        pts: Vec<(f32, f32)>,
        fill: Option<Rgb>,
        stroke: Option<(Rgb, f32)>,
    },
    Line {
        a: (f32, f32),
        b: (f32, f32),
        color: Rgb,
        width: f32,
    },
    /// Text with its baseline at `y`.
    Text {
        x: f32,
        y: f32,
        size: f32,
        text: String,
        anchor: Anchor,
        color: Rgb,
        bold: bool,
    },
}

/// One page of output.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub prims: Vec<Prim>,
}

impl Page {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height, prims: Vec::new() }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)>) {
        self.prims.push(Prim::Poly { pts: rect_pts(x, y, w, h), fill, stroke });
    }

    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), color: Rgb, width: f32) {
        self.prims.push(Prim::Line { a, b, color, width });
    }

    pub fn text(&mut self, x: f32, y: f32, size: f32, text: &str, anchor: Anchor, bold: bool) {
        self.prims.push(Prim::Text { x, y, size, text: text.to_string(), anchor, color: BLACK, bold });
    }

    pub fn text_in(&mut self, x: f32, y: f32, size: f32, text: &str, anchor: Anchor, color: Rgb) {
        self.prims.push(Prim::Text { x, y, size, text: text.to_string(), anchor, color, bold: false });
    }
}

pub fn rect_pts(x: f32, y: f32, w: f32, h: f32) -> Vec<(f32, f32)> {
    vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
}

/// A regular polygon approximating a circle.
pub fn circle_pts(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    const N: usize = 24;
    (0..N)
        .map(|i| {
            let a = (i as f32) / (N as f32) * std::f32::consts::TAU;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

pub fn font() -> Option<FontRef<'static>> {
    FontRef::try_from_slice(FONT_BYTES).ok()
}

/// Width of `text` at `size` points (Liberation Sans is metric-compatible with Helvetica/Arial).
pub fn text_width(text: &str, size: f32) -> f32 {
    let Some(f) = font() else { return text.chars().count() as f32 * size * 0.55 };
    let scaled = f.as_scaled(ab_glyph::PxScale::from(size));
    text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum()
}

/// Left edge of the text given its anchor.
pub fn text_left(x: f32, size: f32, text: &str, anchor: Anchor) -> f32 {
    match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - text_width(text, size) / 2.0,
        Anchor::End => x - text_width(text, size),
    }
}

/// Number to string with at most 2 decimals; non-finite values become 0 so output stays well formed.
pub fn num(v: f32) -> String {
    if v.is_finite() { format!("{v:.2}") } else { "0".to_string() }
}
