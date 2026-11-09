//! SVG back end. The page is sized in points (`pt`) so it prints at true size.

use crate::draw::{Anchor, Page, Prim, Rgb, num};

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn escape(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

pub fn page_to_svg(page: &Page) -> String {
    let (w, h) = (num(page.width), num(page.height));
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}pt\" height=\"{h}pt\" viewBox=\"0 0 {w} {h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n"
    );
    for prim in &page.prims {
        match prim {
            Prim::Poly { pts, fill, stroke } => {
                let d: Vec<String> = pts.iter().map(|(x, y)| format!("{},{}", num(*x), num(*y))).collect();
                let fill = fill.map_or("none".to_string(), hex);
                let stroke =
                    stroke.map_or(String::new(), |(c, w)| format!(" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", hex(c), num(w)));
                s.push_str(&format!("<polygon points=\"{}\" fill=\"{fill}\"{stroke}/>\n", d.join(" ")));
            }
            Prim::Line { a, b, color, width } => {
                s.push_str(&format!(
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"square\"/>\n",
                    num(a.0),
                    num(a.1),
                    num(b.0),
                    num(b.1),
                    hex(*color),
                    num(*width)
                ));
            }
            Prim::Text { x, y, size, text, anchor, color, bold } => {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                let weight = if *bold { "bold" } else { "normal" };
                s.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"{}\" font-weight=\"{weight}\" text-anchor=\"{anchor}\" fill=\"{}\">{}</text>\n",
                    num(*x),
                    num(*y),
                    num(*size),
                    hex(*color),
                    escape(text)
                ));
            }
        }
    }
    s.push_str("</svg>\n");
    s
}
