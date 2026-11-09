//! YarnFoundry UI: a thin egui front end. Panels read core state; colours come from [`theme`].

pub mod theme {
    use egui::Color32;

    /// Colour tokens for the UI.
    pub const CANVAS_BG: Color32 = Color32::from_rgb(250, 247, 242);
    pub const GRID_LINE: Color32 = Color32::from_rgb(214, 207, 196);
    pub const GRID_LINE_MAJOR: Color32 = Color32::from_rgb(170, 160, 146);
    pub const HINT: Color32 = Color32::from_rgb(120, 112, 100);
}

/// Side length of one empty-canvas grid cell, in points.
pub const CELL: f32 = 20.0;

/// The application state shown in the window.
#[derive(Default)]
pub struct YarnFoundryApp;

impl YarnFoundryApp {
    pub fn new() -> Self {
        Self
    }

    /// Draws one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("title").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(yarnfoundry_core::APP_NAME);
                ui.label(format!("v{}", yarnfoundry_core::VERSION));
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS_BG)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_empty_grid(&ui.painter_at(rect), rect);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Your chart will appear here",
                egui::FontId::proportional(22.0),
                theme::HINT,
            );
        });
    }
}

/// Paints the empty chart canvas: a grid with a stronger line every 10 cells.
pub fn paint_empty_grid(painter: &egui::Painter, rect: egui::Rect) {
    let mut i = 0;
    let mut x = rect.left();
    while x <= rect.right() {
        let color = if i % 10 == 0 { theme::GRID_LINE_MAJOR } else { theme::GRID_LINE };
        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0, color));
        x += CELL;
        i += 1;
    }
    let mut j = 0;
    let mut y = rect.top();
    while y <= rect.bottom() {
        let color = if j % 10 == 0 { theme::GRID_LINE_MAJOR } else { theme::GRID_LINE };
        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], egui::Stroke::new(1.0, color));
        y += CELL;
        j += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_runs_without_panicking() {
        let ctx = egui::Context::default();
        let mut app = YarnFoundryApp::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
        assert!(!out.shapes.is_empty(), "the empty canvas draws something");
        out.textures_delta.clear();
    }
}
