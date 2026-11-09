//! PDF back end: a small hand-written PDF 1.4 writer (vector paths and the two standard Helvetica fonts, no embedded fonts).

use crate::draw::{Anchor, Page, Prim, Rgb, num, text_left};

fn colour(c: Rgb) -> String {
    format!("{:.3} {:.3} {:.3}", f32::from(c[0]) / 255.0, f32::from(c[1]) / 255.0, f32::from(c[2]) / 255.0)
}

/// ASCII only (the standard fonts), with PDF string escapes.
fn pdf_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

fn content(page: &Page) -> String {
    let h = page.height;
    let mut s = String::new();
    for prim in &page.prims {
        match prim {
            Prim::Poly { pts, fill, stroke } => {
                let mut it = pts.iter();
                let Some(first) = it.next() else { continue };
                if fill.is_none() && stroke.is_none() {
                    continue;
                }
                if let Some(f) = fill {
                    s.push_str(&format!("{} rg\n", colour(*f)));
                }
                if let Some((c, w)) = stroke {
                    s.push_str(&format!("{} RG {} w 1 j\n", colour(*c), num(*w)));
                }
                s.push_str(&format!("{} {} m\n", num(first.0), num(h - first.1)));
                for p in it {
                    s.push_str(&format!("{} {} l\n", num(p.0), num(h - p.1)));
                }
                s.push_str(match (fill.is_some(), stroke.is_some()) {
                    (true, true) => "h B\n",
                    (true, false) => "h f\n",
                    _ => "h S\n",
                });
            }
            Prim::Line { a, b, color, width } => {
                s.push_str(&format!(
                    "{} RG {} w 2 J {} {} m {} {} l S\n",
                    colour(*color),
                    num(*width),
                    num(a.0),
                    num(h - a.1),
                    num(b.0),
                    num(h - b.1)
                ));
            }
            Prim::Text { x, y, size, text, anchor, color, bold } => {
                let left = match anchor {
                    Anchor::Start => *x,
                    a => text_left(*x, *size, text, *a),
                };
                s.push_str(&format!(
                    "BT {} rg /{} {} Tf {} {} Td ({}) Tj ET\n",
                    colour(*color),
                    if *bold { "F2" } else { "F1" },
                    num(*size),
                    num(left),
                    num(h - y),
                    pdf_string(text)
                ));
            }
        }
    }
    s
}

/// A PDF with one page per entry of `pages`.
pub fn to_pdf(pages: &[Page], title: &str) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new(); // object i+1 is objects[i]
    let n = pages.len();
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 6 + 2 * i)).collect();
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_string());
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_string());
    objects.push(format!("<< /Title ({}) /Producer (YarnFoundry) >>", pdf_string(title)));
    for (i, page) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            num(page.width),
            num(page.height),
            7 + 2 * i
        ));
        let body = content(page);
        objects.push(format!("<< /Length {} >>\nstream\n{body}endstream", body.len()));
    }
    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    out
}
