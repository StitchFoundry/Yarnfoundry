//! YarnFoundry desktop app: hosts the egui front end in an eframe window.
//! `yarnfoundry [file]` opens a `.yarnfoundry` project or turns a PNG/JPEG/WebP picture into a chart.

use std::path::Path;

use yarnfoundry_core::{Chart, Construction, Craft, Gauge, PaletteSpec, Picture, PictureOptions, Technique, Unit};
use yarnfoundry_ui_egui::YarnFoundryApp;

struct Host {
    app: YarnFoundryApp,
}

impl eframe::App for Host {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let chart = std::env::args().nth(1).and_then(|path| match open(Path::new(&path)) {
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
        Box::new(move |_cc| Ok(Box::new(Host { app: chart.map_or_else(YarnFoundryApp::new, YarnFoundryApp::with_chart) }))),
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
