//! YarnFoundry desktop app: hosts the egui front end in an eframe window.
//! `yarnfoundry [file]` opens a `.yarnfoundry` project or turns a PNG/JPEG/WebP picture into a chart.

use std::path::Path;

use yarnfoundry_core::{Chart, Construction, Craft, Gauge, PaletteSpec, Picture, PictureOptions, Technique, Unit};
use yarnfoundry_ui_egui::YarnFoundryApp;

struct Host {
    app: YarnFoundryApp,
    /// File name stem for saved prints.
    stem: String,
}

impl eframe::App for Host {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
        if let Some(path) = self.app.take_load_picture() {
            match std::fs::read(&path) {
                Ok(bytes) => self.app.use_picture_bytes(&bytes),
                Err(e) => self.app.set_message(format!("{path}: {e}")),
            }
        }
        if self.app.take_save_project() {
            let message = match self.app.chart().map(|chart| save_project(chart, &self.stem)) {
                Some(Ok(path)) => format!("Saved {path}"),
                Some(Err(e)) => format!("Could not save: {e}"),
                None => String::new(),
            };
            self.app.set_message(message);
        }
        if let Some(req) = self.app.take_print_request() {
            let message = match self.app.chart().map(|chart| save_pdf(chart, &self.stem, req)) {
                Some(Ok(path)) => format!("Saved {path}"),
                Some(Err(e)) => format!("Could not save: {e}"),
                None => String::new(),
            };
            self.app.set_message(message);
        }
    }
}

/// Writes the printable chart as a PDF next to where the app was started; returns the file name.
fn save_pdf(chart: &Chart, stem: &str, req: yarnfoundry_ui_egui::PrintRequest) -> Result<String, String> {
    let opts = yarnfoundry_render::PrintOptions { paper: req.paper, look: req.look, title: stem.to_string() };
    let pages = yarnfoundry_render::pages(chart, &opts).map_err(|e| e.to_string())?;
    let name = format!("{stem}-{}.pdf", if req.paper == yarnfoundry_render::Paper::A4 { "a4" } else { "letter" });
    std::fs::write(&name, yarnfoundry_render::pdf::to_pdf(&pages, stem)).map_err(|e| format!("{name}: {e}"))?;
    Ok(name)
}

/// Writes the project file next to where the app was started; returns the file name.
fn save_project(chart: &Chart, stem: &str) -> Result<String, String> {
    let name = format!("{stem}.yarnfoundry");
    std::fs::write(&name, yarnfoundry_core::to_json(chart).map_err(|e| e.to_string())?).map_err(|e| format!("{name}: {e}"))?;
    Ok(name)
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `--step N` starts the setup at step N with the demo words filled in (for screenshots).
    let demo_step = args.iter().position(|a| a == "--step").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<usize>().ok());
    let file = args.first().filter(|a| !a.starts_with("--")).cloned();
    let stem = file
        .clone()
        .and_then(|p| Path::new(&p).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "yarnfoundry-chart".to_string());
    let chart = file.and_then(|path| match open(Path::new(&path)) {
        Ok(chart) => Some(chart),
        Err(e) => {
            log::error!("could not open {path}: {e}");
            None
        }
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title(yarnfoundry_core::APP_NAME),
        ..Default::default()
    };
    eframe::run_native(
        yarnfoundry_core::APP_NAME,
        options,
        Box::new(move |_cc| {
            Ok(Box::new(Host {
                app: demo_step.map_or_else(|| chart.map_or_else(YarnFoundryApp::new, YarnFoundryApp::with_chart), YarnFoundryApp::demo_at_step),
                stem,
            }))
        }),
    )
}

/// Opens a project file, or converts a picture with sensible defaults (60 stitches wide, 5 colours, 20 sts / 28 rows per 4 in).
fn open(path: &Path) -> Result<Chart, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("yarnfoundry")) {
        let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
        return yarnfoundry_core::from_json(&text).map_err(|e| e.to_string());
    }
    let pic = Picture::decode(&bytes).map_err(|e| e.to_string())?;
    let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).map_err(|e| e.to_string())?;
    let rows = yarnfoundry_core::rows_for_width(&gauge, pic.width, pic.height, 60).map_err(|e| e.to_string())?;
    let opts = PictureOptions { width: 60, height: rows, palette: PaletteSpec::Auto(5), cleanup: 1 };
    yarnfoundry_core::chart_from_picture(&pic, &opts, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge).map_err(|e| e.to_string())
}
