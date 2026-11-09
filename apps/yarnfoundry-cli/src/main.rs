//! YarnFoundry command line.
//!
//! `yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]` turns a picture into a chart project.
//! `yarnfoundry-cli print <file.yarnfoundry> <out.pdf|out.svg|out.png> [--paper letter|a4] [--bw|--symbols] [--page N] [--dpi N]` writes the printable chart.
//! `yarnfoundry-cli text "<words>" <out.yarnfoundry> [--rows N] [--width N] [--spacing N] [--weight regular|bold|heavy] [--min-rows N] [--outline]` turns words into one.

use yarnfoundry_core::{Construction, Craft, Gauge, PaletteColor, PaletteSpec, Picture, PictureOptions, Technique, TextOptions, TextWeight, Unit};

const USAGE: &str = "usage: yarnfoundry-cli [--version]\n       yarnfoundry-cli print <file.yarnfoundry> <out.pdf|out.svg|out.png> [--paper letter|a4] [--bw|--symbols] [--page N] [--dpi N] [--title T]\n       yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]\n       yarnfoundry-cli instructions <file.yarnfoundry>\n       yarnfoundry-cli model <file.stl|file.obj|sample:NAME> [--height N] [--unit in|cm] [--up x|y|z] [--png out.png]\n       yarnfoundry-cli rounds <file.stl|file.obj|sample:NAME> [--height N] [--unit in|cm] [--up x|y|z] [--sts N] [--rows N] [--png out.png]\n       yarnfoundry-cli pattern <file.stl|file.obj|sample:NAME> [--height N] [--unit in|cm] [--up x|y|z] [--sts N] [--rows N] [--uk] [--weight worsted] [--png out.png]\n       yarnfoundry-cli text \"<words>\" <out.yarnfoundry> [--rows N] [--width N] [--spacing N] [--weight regular|bold|heavy] [--min-rows N] [--outline]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None | Some("--version") | Some("version") => {
            println!("{}", yarnfoundry_core::banner());
            Ok(())
        }
        Some("picture") => picture(&args[1..]),
        Some("text") => text(&args[1..]),
        Some("print") => print(&args[1..]),
        Some("instructions") => instructions(&args[1..]),
        Some("model") => model(&args[1..]),
        Some("rounds") => rounds(&args[1..]),
        Some("pattern") => pattern(&args[1..]),
        Some(other) => Err(format!("unknown command: {other}\n{USAGE}")),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

fn flag(args: &[String], name: &str, default: u32) -> Result<u32, String> {
    match args.iter().position(|a| a == name) {
        None => Ok(default),
        Some(i) => args.get(i + 1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{name} needs a whole number")),
    }
}

fn picture(args: &[String]) -> Result<(), String> {
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else { return Err(USAGE.to_string()) };
    let width = flag(args, "--width", 60)?;
    let colors = flag(args, "--colors", 4)?;
    let pic = Picture::decode(&std::fs::read(input).map_err(|e| format!("{input}: {e}"))?).map_err(|e| e.to_string())?;
    let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).map_err(|e| e.to_string())?;
    let height = yarnfoundry_core::rows_for_width(&gauge, pic.width, pic.height, width).map_err(|e| e.to_string())?;
    let opts = PictureOptions { width, height, palette: PaletteSpec::Auto(colors as usize), cleanup: 1 };
    let chart =
        yarnfoundry_core::chart_from_picture(&pic, &opts, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge).map_err(|e| e.to_string())?;
    std::fs::write(output, yarnfoundry_core::to_json(&chart).map_err(|e| e.to_string())?).map_err(|e| format!("{output}: {e}"))?;
    println!("{width} sts x {height} rows, {colors} colours -> {output}");
    Ok(())
}

fn text(args: &[String]) -> Result<(), String> {
    let (Some(words), Some(output)) = (args.first(), args.get(1)) else { return Err(USAGE.to_string()) };
    let mut opts = TextOptions::new(words);
    opts.rows_tall = flag(args, "--rows", 18)?;
    if args.iter().any(|a| a == "--bold") {
        opts.weight = TextWeight::Bold;
    }
    if let Some(i) = args.iter().position(|a| a == "--weight") {
        let name = args.get(i + 1).ok_or("--weight needs regular, bold or heavy")?;
        opts.weight = TextWeight::parse(name).ok_or("--weight must be regular, bold or heavy")?;
    }
    opts.min_rows = flag(args, "--min-rows", yarnfoundry_core::DEFAULT_MIN_ROWS)?;
    opts.outline = args.iter().any(|a| a == "--outline");
    opts.letter_spacing = flag(args, "--spacing", 1)? as i32;
    if args.iter().any(|a| a == "--width") {
        opts.width = Some(flag(args, "--width", 0)?);
    }
    let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).map_err(|e| e.to_string())?;
    let navy = PaletteColor { name: "dark blue".into(), rgb: [31, 42, 85] };
    let gold = PaletteColor { name: "gold".into(), rgb: [232, 185, 35] };
    let chart = yarnfoundry_core::chart_from_text(&opts, navy, gold, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge)
        .map_err(|e| e.to_string())?;
    std::fs::write(output, yarnfoundry_core::to_json(&chart).map_err(|e| e.to_string())?).map_err(|e| format!("{output}: {e}"))?;
    println!("{} sts x {} rows -> {output}", chart.width, chart.height);
    Ok(())
}

fn instructions(args: &[String]) -> Result<(), String> {
    let Some(input) = args.first() else { return Err(USAGE.to_string()) };
    let text = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let chart = yarnfoundry_core::from_json(&text).map_err(|e| e.to_string())?;
    let written = yarnfoundry_core::instructions(&chart);
    for row in &written.rows {
        println!("{}", row.text);
    }
    println!();
    for t in &written.totals {
        println!("{} ({}): {}", t.label, t.name, t.count);
    }
    Ok(())
}

/// `print`: a PDF holds every page; SVG and PNG hold one page (`--page N`, default 2 = the first chart page, 1 = the legend page).
fn print(args: &[String]) -> Result<(), String> {
    use yarnfoundry_render::{Look, Paper, PrintOptions};
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else { return Err(USAGE.to_string()) };
    let text = std::fs::read_to_string(input).map_err(|e| format!("{input}: {e}"))?;
    let chart = yarnfoundry_core::from_json(&text).map_err(|e| e.to_string())?;
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let paper = match value("--paper") {
        Some(p) => Paper::parse(p).ok_or("--paper must be letter or a4")?,
        None => Paper::Letter,
    };
    let look = if args.iter().any(|a| a == "--bw") {
        Look::BlackWhite
    } else if args.iter().any(|a| a == "--symbols") {
        Look::ColourSymbols
    } else {
        Look::Colour
    };
    let title = value("--title").cloned().unwrap_or_else(|| {
        std::path::Path::new(input).file_stem().map_or_else(|| "YarnFoundry chart".to_string(), |s| s.to_string_lossy().into_owned())
    });
    let pages = yarnfoundry_render::pages(&chart, &PrintOptions { paper, look, title: title.clone() }).map_err(|e| e.to_string())?;
    let ext = std::path::Path::new(output).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let page_no = flag(args, "--page", 2)? as usize;
    let page = pages.get(page_no.saturating_sub(1)).ok_or_else(|| format!("page {page_no} does not exist (the print has {} pages)", pages.len()))?;
    let bytes = match ext.as_str() {
        "pdf" => yarnfoundry_render::pdf::to_pdf(&pages, &title),
        "svg" => yarnfoundry_render::svg::page_to_svg(page).into_bytes(),
        "png" => yarnfoundry_render::png::page_to_png(page, flag(args, "--dpi", 150)?).map_err(|e| e.to_string())?,
        _ => return Err("the output file must end in .pdf, .svg or .png".to_string()),
    };
    std::fs::write(output, bytes).map_err(|e| format!("{output}: {e}"))?;
    println!("{} pages -> {output}", pages.len());
    Ok(())
}

/// `model`: loads an STL/OBJ (or a built-in `sample:sphere|capsule|cone|snowman`), checks it, sets it upright at the
/// finished height and says what it found. `--png` draws it.
fn model(args: &[String]) -> Result<(), String> {
    let Some(input) = args.first() else { return Err(USAGE.to_string()) };
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let (fitted, report) = load_fitted(args)?;
    let (lo, hi) = fitted.bounds();
    let _ = input;
    println!(
        "{} triangles, {} piece(s); {:.2} in tall, {:.2} x {:.2} in across",
        report.triangles,
        report.bodies,
        hi[2] - lo[2],
        hi[0] - lo[0],
        hi[1] - lo[1]
    );
    for note in report.notes() {
        println!("note: {note}");
    }
    if let Some(out) = value("--png") {
        let page = yarnfoundry_render::view3d::mesh_page(&fitted, 600.0, 600.0, &yarnfoundry_render::view3d::Camera::default(), [196, 120, 96]);
        let png = yarnfoundry_render::png::page_to_png(&page, 144).map_err(|e| e.to_string())?;
        std::fs::write(out, png).map_err(|e| format!("{out}: {e}"))?;
        println!("picture -> {out}");
    }
    Ok(())
}

/// Loads, checks and fits the model named by the first argument, using --height, --unit and --up.
fn load_fitted(args: &[String]) -> Result<(yarnfoundry_core::Mesh, yarnfoundry_core::MeshReport), String> {
    use yarnfoundry_core::{Axis, models};
    let Some(input) = args.first() else { return Err(USAGE.to_string()) };
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let (mesh, report) = match input.strip_prefix("sample:") {
        Some(name) => {
            let m = match name {
                "sphere" => models::sphere(1.0),
                "capsule" => models::capsule(1.0, 1.5),
                "cone" => models::cone(1.0, 2.0),
                "snowman" => models::snowman(),
                "limbs" => models::limb_figure(),
                _ => return Err("samples: sphere, capsule, cone, snowman, limbs".to_string()),
            };
            m.checked().map_err(|e| e.to_string())?
        }
        None => yarnfoundry_core::import_model(&std::fs::read(input).map_err(|e| format!("{input}: {e}"))?).map_err(|e| e.to_string())?,
    };
    let height: f64 = value("--height").map_or(Ok(8.0), |v| v.parse().map_err(|_| "--height needs a number".to_string()))?;
    let unit = match value("--unit").map(String::as_str) {
        None | Some("in") => Unit::Inches,
        Some("cm") => Unit::Cm,
        Some(_) => return Err("--unit must be in or cm".to_string()),
    };
    let up = match value("--up").map(String::as_str) {
        None => None,
        Some("x") => Some(Axis::X),
        Some("y") => Some(Axis::Y),
        Some("z") => Some(Axis::Z),
        Some(_) => return Err("--up must be x, y or z".to_string()),
    };
    let fitted = mesh.fitted(height, unit, up).map_err(|e| e.to_string())?;
    Ok((fitted, report))
}

/// `rounds`: the stitch count of every round, bottom to top.
fn rounds(args: &[String]) -> Result<(), String> {
    let (fitted, _) = load_fitted(args)?;
    let sts = flag(args, "--sts", 20)?;
    let rows = flag(args, "--rows", 20)?;
    let gauge = Gauge::new(f64::from(sts), f64::from(rows), 4.0, Unit::Inches).map_err(|e| e.to_string())?;
    let rounds = yarnfoundry_core::rounds_for_mesh(&fitted, &gauge).map_err(|e| e.to_string())?;
    for r in &rounds {
        println!("Rnd {}: {} sts", r.index, r.stitches);
    }
    println!("{} rounds, {} stitches", rounds.len(), rounds.iter().map(|r| r.stitches).sum::<u32>());
    if let Some(out) = args.iter().position(|a| a == "--png").and_then(|i| args.get(i + 1)) {
        use yarnfoundry_render::draw::{Anchor, Prim};
        use yarnfoundry_render::view3d::{Camera, Frame, draw_mesh};
        let mut page = yarnfoundry_render::draw::Page::new(720.0, 520.0);
        page.rect(0.0, 0.0, 720.0, 520.0, Some([244, 241, 235]), None);
        draw_mesh(&mut page, &fitted, &Camera::default(), Frame { x: 10.0, y: 40.0, w: 330.0, h: 460.0 }, [196, 120, 96]);
        let text = |page: &mut yarnfoundry_render::draw::Page, x: f32, y: f32, size: f32, t: String, bold: bool| {
            page.prims.push(Prim::Text { x, y, size, text: t, anchor: Anchor::Start, color: [40, 40, 40], bold });
        };
        text(&mut page, 20.0, 28.0, 18.0, "Rounds from the 3D model".to_string(), true);
        let per_col = 20;
        for (i, r) in rounds.iter().enumerate() {
            let (col, row) = (i / per_col, i % per_col);
            text(&mut page, 370.0 + col as f32 * 170.0, 62.0 + row as f32 * 22.0, 14.0, format!("Rnd {}: {} sts", r.index, r.stitches), false);
        }
        let total: u32 = rounds.iter().map(|r| r.stitches).sum();
        text(&mut page, 370.0, 500.0, 14.0, format!("{} rounds, {total} stitches", rounds.len()), true);
        let png = yarnfoundry_render::png::page_to_png(&page, 144).map_err(|e| e.to_string())?;
        std::fs::write(out, png).map_err(|e| format!("{out}: {e}"))?;
        println!("picture -> {out}");
    }
    Ok(())
}

/// `pattern`: the written amigurumi pattern for the whole model as one part.
fn pattern(args: &[String]) -> Result<(), String> {
    use yarnfoundry_core::{Terms, YarnWeight};
    let (fitted, _) = load_fitted(args)?;
    let sts = flag(args, "--sts", 20)?;
    let rows = flag(args, "--rows", 20)?;
    let gauge = Gauge::new(f64::from(sts), f64::from(rows), 4.0, Unit::Inches).map_err(|e| e.to_string())?;
    let rounds = yarnfoundry_core::rounds_for_mesh(&fitted, &gauge).map_err(|e| e.to_string())?;
    let terms = if args.iter().any(|a| a == "--uk") { Terms::Uk } else { Terms::Us };
    let weight = match args.iter().position(|a| a == "--weight").and_then(|i| args.get(i + 1)).map(String::as_str) {
        None | Some("worsted") => YarnWeight::Worsted,
        Some("lace") => YarnWeight::Lace,
        Some("fingering") => YarnWeight::Fingering,
        Some("sport") => YarnWeight::Sport,
        Some("dk") => YarnWeight::DK,
        Some("aran") => YarnWeight::Aran,
        Some("bulky") => YarnWeight::Bulky,
        Some("super-bulky") => YarnWeight::SuperBulky,
        Some(_) => return Err("--weight must be lace, fingering, sport, dk, worsted, aran, bulky or super-bulky".to_string()),
    };
    let figure = yarnfoundry_core::split_figure(&fitted, &gauge, &yarnfoundry_core::ManualSplits::default()).map_err(|e| e.to_string())?;
    let _ = rounds;
    let pats = figure.patterns(terms, weight, &gauge, None);
    let (mut rounds_n, mut stitches, mut yards) = (0, 0, 0.0);
    for p in &pats {
        println!("== {} ==", p.name);
        for r in &p.rounds {
            println!("{}", r.text);
        }
        for l in &p.finishing {
            println!("{l}");
        }
        for n in &p.notes {
            println!("Note: {n}");
        }
        println!();
        rounds_n += p.rounds.len();
        stitches += p.total_stitches;
        yards += p.yards;
    }
    if let Some(out) = args.iter().position(|a| a == "--png").and_then(|i| args.get(i + 1)) {
        use yarnfoundry_render::view3d::{Camera, Frame, draw_figure};
        let mut page = yarnfoundry_render::draw::Page::new(700.0, 520.0);
        page.rect(0.0, 0.0, 700.0, 520.0, Some([244, 241, 235]), None);
        draw_figure(&mut page, &figure, &Camera::default(), Frame { x: 10.0, y: 40.0, w: 400.0, h: 470.0 }, None);
        let label = |page: &mut yarnfoundry_render::draw::Page, y: f32, t: String, bold: bool| {
            page.prims.push(yarnfoundry_render::draw::Prim::Text {
                x: 430.0,
                y,
                size: 15.0,
                text: t,
                anchor: yarnfoundry_render::draw::Anchor::Start,
                color: [40, 40, 40],
                bold,
            });
        };
        label(&mut page, 28.0, "Parts and rounds from the 3D model".to_string(), true);
        for (i, (part, pat)) in figure.parts.iter().zip(&pats).enumerate() {
            let y = 70.0 + i as f32 * 26.0;
            let col = yarnfoundry_render::view3d::PART_COLOURS[i % 8];
            page.rect(430.0, y - 12.0, 14.0, 14.0, Some(col), None);
            page.prims.push(yarnfoundry_render::draw::Prim::Text {
                x: 452.0,
                y,
                size: 14.0,
                text: format!("{}: {} rounds, {} sts", part.name, pat.rounds.len(), pat.total_stitches),
                anchor: yarnfoundry_render::draw::Anchor::Start,
                color: [40, 40, 40],
                bold: false,
            });
        }
        let mut y = 70.0 + (figure.parts.len() as f32 + 1.0) * 26.0;
        for n in figure.assembly_notes() {
            let mut line = String::new();
            for word in n.split(' ') {
                if line.len() + word.len() > 38 {
                    page.prims.push(yarnfoundry_render::draw::Prim::Text {
                        x: 430.0,
                        y,
                        size: 12.0,
                        text: std::mem::take(&mut line),
                        anchor: yarnfoundry_render::draw::Anchor::Start,
                        color: [40, 40, 40],
                        bold: false,
                    });
                    y += 17.0;
                }
                line.push_str(word);
                line.push(' ');
            }
            page.prims.push(yarnfoundry_render::draw::Prim::Text {
                x: 430.0,
                y,
                size: 12.0,
                text: line,
                anchor: yarnfoundry_render::draw::Anchor::Start,
                color: [40, 40, 40],
                bold: false,
            });
            y += 24.0;
        }
        let png = yarnfoundry_render::png::page_to_png(&page, 144).map_err(|e| e.to_string())?;
        std::fs::write(out, png).map_err(|e| format!("{out}: {e}"))?;
        println!("picture -> {out}");
    }
    if !figure.assembly_notes().is_empty() {
        println!("== Assembly ==");
        for n in figure.assembly_notes() {
            println!("{n}");
        }
        println!();
    }
    for n in &figure.notes {
        println!("Note: {n}");
    }
    println!("{} part(s), {rounds_n} rounds, {stitches} stitches, about {:.0} yards of {} (an estimate)", pats.len(), yards.ceil(), weight.label());
    Ok(())
}
