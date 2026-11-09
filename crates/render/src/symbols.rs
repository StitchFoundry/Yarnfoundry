//! Black-and-white stitch symbols: 16 drawn shapes, then numbers, so every palette colour (up to 64) stays distinct.

use crate::draw::{Anchor, BLACK, Prim, Rgb, circle_pts, rect_pts};

/// How many colours get a drawn shape; the rest get their number.
pub const SHAPES: usize = 16;

/// The symbol for palette colour `index`, centred at (`cx`, `cy`), `r` points from the centre to the edge.
pub fn symbol(index: usize, cx: f32, cy: f32, r: f32, ink: Rgb) -> Vec<Prim> {
    let w = (r * 0.28).max(0.3);
    let thick = w * 2.0;
    let outline = |pts: Vec<(f32, f32)>| Prim::Poly { pts, fill: None, stroke: Some((ink, w)) };
    let solid = |pts: Vec<(f32, f32)>| Prim::Poly { pts, fill: Some(ink), stroke: None };
    let line = |a: (f32, f32), b: (f32, f32), width: f32| Prim::Line { a, b, color: ink, width };
    let up = vec![(cx, cy - r), (cx + r, cy + r * 0.8), (cx - r, cy + r * 0.8)];
    let down = vec![(cx, cy + r), (cx + r, cy - r * 0.8), (cx - r, cy - r * 0.8)];
    let diamond = vec![(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)];
    let square = rect_pts(cx - r * 0.85, cy - r * 0.85, r * 1.7, r * 1.7);
    match index {
        0 => vec![outline(circle_pts(cx, cy, r))],
        1 => vec![solid(circle_pts(cx, cy, r))],
        2 => vec![outline(square)],
        3 => vec![solid(square)],
        4 => vec![outline(up)],
        5 => vec![solid(up)],
        6 => vec![outline(diamond)],
        7 => vec![solid(diamond)],
        8 => vec![line((cx - r, cy), (cx + r, cy), w), line((cx, cy - r), (cx, cy + r), w)],
        9 => vec![line((cx - r, cy), (cx + r, cy), thick), line((cx, cy - r), (cx, cy + r), thick)],
        10 => vec![line((cx - r, cy - r), (cx + r, cy + r), w), line((cx - r, cy + r), (cx + r, cy - r), w)],
        11 => vec![line((cx - r, cy - r), (cx + r, cy + r), thick), line((cx - r, cy + r), (cx + r, cy - r), thick)],
        12 => vec![outline(down)],
        13 => vec![solid(down)],
        14 => vec![line((cx - r, cy - r * 0.45), (cx + r, cy - r * 0.45), w), line((cx - r, cy + r * 0.45), (cx + r, cy + r * 0.45), w)],
        15 => vec![line((cx - r * 0.45, cy - r), (cx - r * 0.45, cy + r), w), line((cx + r * 0.45, cy - r), (cx + r * 0.45, cy + r), w)],
        n => vec![Prim::Text {
            x: cx,
            y: cy + r * 0.55,
            size: (r * 1.5).max(2.0),
            text: (n + 1).to_string(),
            anchor: Anchor::Middle,
            color: ink,
            bold: true,
        }],
    }
}

/// Black ink for the black-and-white print.
pub fn bw_symbol(index: usize, cx: f32, cy: f32, r: f32) -> Vec<Prim> {
    symbol(index, cx, cy, r, BLACK)
}
