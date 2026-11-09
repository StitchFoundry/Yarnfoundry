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
pub struct YarnFoundryApp {
    chart: Option<yarnfoundry_core::Chart>,
}

impl YarnFoundryApp {
    pub fn new() -> Self {
        Self::default()
    }

    /// An app showing `chart`.
    pub fn with_chart(chart: yarnfoundry_core::Chart) -> Self {
        Self { chart: Some(chart) }
    }

    /// Draws one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("title").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(yarnfoundry_core::APP_NAME);
                ui.label(format!("v{}", yarnfoundry_core::VERSION));
                if let Some(chart) = &self.chart {
                    let (w, h) = chart.finished_size();
                    let unit = if chart.gauge.unit == yarnfoundry_core::Unit::Cm { "cm" } else { "in" };
                    ui.separator();
                    ui.label(format!(
                        "{} sts x {} rows, {} colours, about {:.1} x {:.1} {}",
                        chart.width,
                        chart.height,
                        chart.palette.len(),
                        w,
                        h,
                        unit
                    ));
                }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS_BG)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            match &self.chart {
                Some(chart) => paint_chart(&ui.painter_at(rect), rect, chart),
                None => {
                    paint_empty_grid(&ui.painter_at(rect), rect);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Your chart will appear here",
                        egui::FontId::proportional(22.0),
                        theme::HINT,
                    );
                }
            }
        });
    }
}

/// Paints `chart` scaled to fit `rect` (row 0, the first row worked, at the bottom), with a stronger line every 10 stitches/rows.
pub fn paint_chart(painter: &egui::Painter, rect: egui::Rect, chart: &yarnfoundry_core::Chart) {
    let margin = 16.0;
    let avail = rect.shrink(margin);
    // A stitch is wider than it is tall (rows per inch > stitches per inch), so draw cells in that proportion.
    let aspect = chart.gauge.stitch_aspect() as f32;
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    let cell = (avail.width() / chart.width as f32).min(avail.height() * aspect / chart.height as f32).max(1.0);
    let cell_h = cell / aspect;
    let size = egui::vec2(cell * chart.width as f32, cell_h * chart.height as f32);
    let origin = egui::pos2(rect.center().x - size.x / 2.0, rect.center().y - size.y / 2.0);
    for y in 0..chart.height {
        for x in 0..chart.width {
            let Some(c) = chart.get(x, y).and_then(|i| chart.palette.get(usize::from(i))) else { continue };
            let top = origin.y + size.y - (y as f32 + 1.0) * cell_h;
            let r = egui::Rect::from_min_size(egui::pos2(origin.x + x as f32 * cell, top), egui::vec2(cell, cell_h));
            painter.rect_filled(r, 0.0, egui::Color32::from_rgb(c.rgb[0], c.rgb[1], c.rgb[2]));
        }
    }
    let line = |a: egui::Pos2, b: egui::Pos2, major: bool| {
        let (color, width) = if major { (theme::GRID_LINE_MAJOR, 1.5) } else { (egui::Color32::from_black_alpha(40), 0.5) };
        painter.line_segment([a, b], egui::Stroke::new(width, color));
    };
    for x in 0..=chart.width {
        let px = origin.x + x as f32 * cell;
        line(egui::pos2(px, origin.y), egui::pos2(px, origin.y + size.y), x % 10 == 0);
    }
    for y in 0..=chart.height {
        // count rows from the bottom so the heavy lines match the row numbers
        let py = origin.y + size.y - y as f32 * cell_h;
        line(egui::pos2(origin.x, py), egui::pos2(origin.x + size.x, py), y % 10 == 0);
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

    #[test]
    fn a_frame_with_a_chart_runs_and_draws_every_cell() {
        use yarnfoundry_core::{Chart, Construction, Craft, Gauge, PaletteColor, Technique, Unit};
        let palette = vec![PaletteColor { name: "cream".into(), rgb: [250, 244, 230] }, PaletteColor { name: "red".into(), rgb: [200, 30, 40] }];
        let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let chart = Chart::blank(12, 8, palette, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge).unwrap();
        let ctx = egui::Context::default();
        let mut app = YarnFoundryApp::with_chart(chart);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
        out.textures_delta.clear();
        assert!(out.shapes.len() > 12 * 8, "{} shapes", out.shapes.len());
    }
}
