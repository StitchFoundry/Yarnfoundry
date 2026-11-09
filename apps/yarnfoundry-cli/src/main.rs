//! YarnFoundry command line.
//!
//! `yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]` turns a picture into a chart project.

use yarnfoundry_core::{Construction, Craft, Gauge, PaletteSpec, Picture, PictureOptions, Technique, Unit};

const USAGE: &str = "usage: yarnfoundry-cli [--version]\n       yarnfoundry-cli picture <image> <out.yarnfoundry> [--width N] [--colors K]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None | Some("--version") | Some("version") => {
            println!("{}", yarnfoundry_core::banner());
            Ok(())
        }
        Some("picture") => picture(&args[1..]),
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
