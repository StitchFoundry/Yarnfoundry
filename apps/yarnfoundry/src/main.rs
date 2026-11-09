//! YarnFoundry desktop app: hosts the egui front end in an eframe window.

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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 750.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title(yarnfoundry_core::APP_NAME),
        ..Default::default()
    };
    eframe::run_native(yarnfoundry_core::APP_NAME, options, Box::new(|_cc| Ok(Box::new(Host { app: YarnFoundryApp::new() }))))
}
