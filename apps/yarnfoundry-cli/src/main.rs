//! YarnFoundry command line.
//!
//! `yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]` turns a picture into a chart project.
//! `yarnfoundry-cli print <file.yarnfoundry> <out.pdf|out.svg|out.png> [--paper letter|a4] [--bw|--symbols] [--page N] [--dpi N]` writes the printable chart.
//! `yarnfoundry-cli text "<words>" <out.yarnfoundry> [--rows N] [--width N] [--spacing N] [--weight regular|bold|heavy] [--min-rows N] [--outline]` turns words into one.

use yarnfoundry_core::{Construction, Craft, Gauge, PaletteColor, PaletteSpec, Picture, PictureOptions, Technique, TextOptions, TextWeight, Unit};

const USAGE: &str = "usage: yarnfoundry-cli [--version]\n       yarnfoundry-cli print <file.yarnfoundry> <out.pdf|out.svg|out.png> [--paper letter|a4] [--bw|--symbols] [--page N] [--dpi N] [--title T]\n       yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]\n       yarnfoundry-cli instructions <file.yarnfoundry>\n       yarnfoundry-cli text \"<words>\" <out.yarnfoundry> [--rows N] [--width N] [--spacing N] [--weight regular|bold|heavy] [--min-rows N] [--outline]";

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
