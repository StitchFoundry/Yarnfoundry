//! PNG back end: a small software rasteriser (filled polygons, thick lines, anti-aliased text), 2x supersampled.

use ab_glyph::{Font, PxScale, ScaleFont, point};
use image::{ImageEncoder, codecs::png::PngEncoder};

use crate::RenderError;
use crate::draw::{Page, Prim, Rgb, WHITE, font, text_left};

pub const MIN_DPI: u32 = 36;
pub const MAX_DPI: u32 = 300;

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>, // RGB
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self { w, h, px: vec![255; w * h * 3] }
    }

    fn blend(&mut self, x: i64, y: i64, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 3;
        for (k, &channel) in c.iter().enumerate() {
            if let Some(p) = self.px.get_mut(i + k) {
                *p = (f32::from(*p) * (1.0 - a) + f32::from(channel) * a).round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    /// Even-odd scanline fill, sampled at pixel centres.
    fn fill_poly(&mut self, pts: &[(f32, f32)], c: Rgb) {
        if pts.len() < 3 {
            return;
        }
        let (min_y, max_y) = pts.iter().fold((f32::MAX, f32::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
        if !(min_y.is_finite() && max_y.is_finite()) {
            return;
        }
        let y0 = (min_y - 0.5).ceil().max(0.0) as i64;
        let y1 = ((max_y - 0.5).floor() as i64).min(self.h as i64 - 1);
        let mut xs: Vec<f32> = Vec::new();
        for y in y0..=y1 {
            let sy = y as f32 + 0.5;
            xs.clear();
            for (i, a) in pts.iter().enumerate() {
                let Some(b) = pts.get((i + 1) % pts.len()) else { continue };
                if (a.1 <= sy && b.1 > sy) || (b.1 <= sy && a.1 > sy) {
                    xs.push(a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|p, q| p.total_cmp(q));
            for pair in xs.chunks(2) {
                if let [l, r] = pair {
                    let xa = (l - 0.5).ceil().max(0.0) as i64;
                    let xb = ((r - 0.5).floor() as i64).min(self.w as i64 - 1);
                    for x in xa..=xb {
                        self.blend(x, y, c, 1.0);
                    }
                }
            }
        }
    }

    fn stroke_segment(&mut self, a: (f32, f32), b: (f32, f32), width: f32, c: Rgb, square_caps: bool) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 || !len.is_finite() {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        let hw = width.max(1.0) / 2.0;
        let ext = if square_caps { hw } else { 0.0 };
        let (a, b) = ((a.0 - ux * ext, a.1 - uy * ext), (b.0 + ux * ext, b.1 + uy * ext));
        let (nx, ny) = (-uy * hw, ux * hw);
        self.fill_poly(&[(a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny)], c);
    }

    fn text(&mut self, x: f32, y: f32, size: f32, s: &str, c: Rgb) {
        let Some(f) = font() else { return };
        let scale = PxScale::from(size);
        let scaled = f.as_scaled(scale);
        let mut pen = x;
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(g) = f.outline_glyph(id.with_scale_and_position(scale, point(pen, y))) {
                let b = g.px_bounds();
                g.draw(|gx, gy, cov| self.blend(b.min.x as i64 + i64::from(gx), b.min.y as i64 + i64::from(gy), c, cov));
            }
            pen += scaled.h_advance(id);
        }
    }
}

/// The page as RGB bytes at `dpi` (clamped to [`MIN_DPI`], [`MAX_DPI`]): (width px, height px, pixels).
pub fn page_to_rgb(page: &Page, dpi: u32) -> (u32, u32, Vec<u8>) {
    let dpi = dpi.clamp(MIN_DPI, MAX_DPI);
    let ss = if dpi <= 150 { 2usize } else { 1 };
    let scale = dpi as f32 / 72.0 * ss as f32;
    let out_w = ((page.width * dpi as f32 / 72.0).round() as usize).max(1);
    let out_h = ((page.height * dpi as f32 / 72.0).round() as usize).max(1);
    let mut cv = Canvas::new(out_w * ss, out_h * ss);
    cv.fill_poly(&[(0.0, 0.0), (cv.w as f32, 0.0), (cv.w as f32, cv.h as f32), (0.0, cv.h as f32)], WHITE);
    let sp = |p: &(f32, f32)| (p.0 * scale, p.1 * scale);
    for prim in &page.prims {
        match prim {
            Prim::Poly { pts, fill, stroke } => {
                let dev: Vec<(f32, f32)> = pts.iter().map(sp).collect();
                if let Some(f) = fill {
                    cv.fill_poly(&dev, *f);
                }
                if let Some((c, w)) = stroke {
                    for (i, a) in dev.iter().enumerate() {
                        if let Some(b) = dev.get((i + 1) % dev.len()) {
                            cv.stroke_segment(*a, *b, w * scale, *c, true);
                        }
                    }
                }
            }
            Prim::Line { a, b, color, width } => cv.stroke_segment(sp(a), sp(b), width * scale, *color, true),
            Prim::Text { x, y, size, text, anchor, color, .. } => {
                let left = text_left(*x, *size, text, *anchor);
                cv.text(left * scale, y * scale, size * scale, text, *color);
            }
        }
    }
    if ss == 1 {
        return (out_w as u32, out_h as u32, cv.px);
    }
    let mut out = vec![0u8; out_w * out_h * 3];
    for y in 0..out_h {
        for x in 0..out_w {
            for k in 0..3 {
                let mut sum = 0u32;
                for dy in 0..ss {
                    for dx in 0..ss {
                        sum += u32::from(cv.px.get(((y * ss + dy) * cv.w + x * ss + dx) * 3 + k).copied().unwrap_or(255));
                    }
                }
                if let Some(o) = out.get_mut((y * out_w + x) * 3 + k) {
                    *o = (sum / (ss * ss) as u32) as u8;
                }
            }
        }
    }
    (out_w as u32, out_h as u32, out)
}

/// The page as a PNG file.
pub fn page_to_png(page: &Page, dpi: u32) -> Result<Vec<u8>, RenderError> {
    let (w, h, px) = page_to_rgb(page, dpi);
    let mut out = Vec::new();
    PngEncoder::new(&mut out).write_image(&px, w, h, image::ExtendedColorType::Rgb8).map_err(|e| RenderError::Encode(e.to_string()))?;
    Ok(out)
}
