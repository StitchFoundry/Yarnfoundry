//! "Make a 3D figure": load a model, choose size and yarn, look at the parts, preview, and get the written and
//! printable pattern. Five steps, each with a default. No I/O here: the host reads files and saves the PDF.

use yarnfoundry_core::{Axis, Figure, Gauge, ManualSplits, Mesh, PartPattern, Terms, Unit, YarnWeight, models};
use yarnfoundry_render::Paper;

use crate::figure3d::FigureView;
use crate::theme;

pub const STEPS: usize = 5;
pub const TITLES: [&str; STEPS] = ["Load a 3D model", "Size, yarn and hook", "Parts", "Preview", "Pattern"];

/// State of the figure wizard.
pub struct FigureWizard {
    step: usize,
    raw: Option<Mesh>,
    name: String,
    notes: Vec<String>,
    path: String,
    height: f64,
    unit: Unit,
    up: Option<Axis>,
    weight: YarnWeight,
    stitches: f64,
    rows: f64,
    terms: Terms,
    title: String,
    designer: String,
    manual: ManualSplits,
    add_at: f64,
    fitted: Option<Mesh>,
    figure: Option<Figure>,
    patterns: Vec<PartPattern>,
    view: FigureView,
    error: String,
    message: String,
    can_read_paths: bool,
    load_request: Option<String>,
    pdf_request: Option<Paper>,
    pdf: Option<(String, Vec<u8>)>,
}

impl FigureWizard {
    pub fn new(can_read_paths: bool) -> Self {
        let (stitches, rows) = YarnWeight::Worsted.amigurumi_gauge();
        Self {
            step: 1,
            raw: None,
            name: String::new(),
            notes: Vec::new(),
            path: String::new(),
            height: 8.0,
            unit: Unit::Inches,
            up: None,
            weight: YarnWeight::Worsted,
            stitches,
            rows,
            terms: Terms::Us,
            title: "My amigurumi".to_string(),
            designer: String::new(),
            manual: ManualSplits::default(),
            add_at: 3.0,
            fitted: None,
            figure: None,
            patterns: Vec::new(),
            view: FigureView::default(),
            error: String::new(),
            message: String::new(),
            can_read_paths,
            load_request: None,
            pdf_request: None,
            pdf: None,
        }
    }

    /// A wizard with a built-in sample or checked model already loaded, opened at `step` (for demos and screenshots).
    pub fn with_model(raw: Mesh, name: &str, height_in: f64, step: usize) -> Self {
        let mut w = Self::new(true);
        w.raw = Some(raw);
        w.name = name.to_string();
        w.title = if name == "bear" { "Original Bear".to_string() } else { format!("Amigurumi {name}") };
        w.height = height_in;
        w.rebuild();
        w.step = step.clamp(1, STEPS);
        w
    }

    pub fn step(&self) -> usize {
        self.step
    }

    pub fn error(&self) -> &str {
        &self.error
    }

    pub fn figure(&self) -> Option<&Figure> {
        self.figure.as_ref()
    }

    pub fn patterns(&self) -> &[PartPattern] {
        &self.patterns
    }

    pub fn set_message(&mut self, m: String) {
        self.message = m;
    }

    pub fn set_can_read_paths(&mut self, yes: bool) {
        self.can_read_paths = yes;
    }

    /// A model file path the user asked to load (empty: the host opens its own chooser).
    pub fn take_load_request(&mut self) -> Option<String> {
        self.load_request.take()
    }

    /// The finished PDF (file name and bytes) if the user asked for one.
    pub fn take_pdf(&mut self) -> Option<(String, Vec<u8>)> {
        if let Some(paper) = self.pdf_request.take() {
            self.make_pdf(paper);
        }
        self.pdf.take()
    }

    fn make_pdf(&mut self, paper: Paper) {
        let (Some(mesh), Some(figure)) = (&self.fitted, &self.figure) else { return };
        let Ok(gauge) = self.gauge() else { return };
        let pages = yarnfoundry_render::booklet::booklet_pages(&yarnfoundry_render::booklet::BookletInput {
            title: &self.title,
            designer: &self.designer,
            paper,
            mesh,
            figure,
            patterns: &self.patterns,
            terms: self.terms,
            weight: self.weight,
            gauge: &gauge,
            height_in: self.height_in(),
        });
        let stem: String = self.title.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
        let stem = stem.trim_matches('-');
        let name = format!("{}-{}.pdf", if stem.is_empty() { "amigurumi" } else { stem }, if paper == Paper::A4 { "a4" } else { "letter" });
        self.pdf = Some((name, yarnfoundry_render::pdf::to_pdf(&pages, &self.title)));
    }

    fn height_in(&self) -> f64 {
        if self.unit == Unit::Cm { self.height / 2.54 } else { self.height }
    }

    fn gauge(&self) -> Result<Gauge, String> {
        Gauge::new(self.stitches, self.rows, 4.0, Unit::Inches).map_err(|e| e.to_string())
    }

    /// Loads a model the user chose (the host read the bytes).
    pub fn use_model_bytes(&mut self, name: &str, bytes: &[u8]) {
        match yarnfoundry_core::import_model(bytes) {
            Ok((mesh, report)) => {
                self.notes = report.notes();
                self.notes.insert(0, format!("{} triangles, {} piece(s).", report.triangles, report.bodies));
                self.raw = Some(mesh);
                self.name = name.to_string();
                self.manual = ManualSplits::default();
                self.error.clear();
            }
            Err(e) => {
                self.raw = None;
                self.error = e.to_string();
            }
        }
    }

    fn use_sample(&mut self, name: &str, height: f64) {
        let Some(mesh) = models::sample(name) else { return };
        match mesh.checked() {
            Ok((m, report)) => {
                self.notes = vec![format!("Built-in sample: {} triangles.", report.triangles)];
                self.raw = Some(m);
                self.name = name.to_string();
                self.height = height;
                self.unit = Unit::Inches;
                self.manual = ManualSplits::default();
                self.title = if name == "bear" { "Original Bear".to_string() } else { format!("Amigurumi {name}") };
                self.error.clear();
            }
            Err(e) => self.error = e.to_string(),
        }
    }

    /// Fits the model and splits it with the current choices.
    fn rebuild(&mut self) -> bool {
        self.fitted = None;
        self.figure = None;
        self.patterns.clear();
        let Some(raw) = &self.raw else {
            self.error = "Load a 3D model first.".to_string();
            return false;
        };
        let result = raw.fitted(self.height, self.unit, self.up).map_err(|e| e.to_string()).and_then(|fitted| {
            let gauge = self.gauge()?;
            let figure = yarnfoundry_core::split_figure(&fitted, &gauge, &self.manual).map_err(|e| e.to_string())?;
            Ok((fitted, gauge, figure))
        });
        match result {
            Ok((fitted, gauge, figure)) => {
                self.patterns = figure.patterns(self.terms, self.weight, &gauge, None);
                self.fitted = Some(fitted);
                self.figure = Some(figure);
                self.error.clear();
                true
            }
            Err(e) => {
                self.error = e;
                false
            }
        }
    }

    /// Next step if this one is complete. Returns whether it moved.
    pub fn go_next(&mut self) -> bool {
        if self.step >= STEPS {
            return false;
        }
        let ok = match self.step {
            1 => {
                if self.raw.is_none() {
                    self.error = "Load a model, or use the sample bear.".to_string();
                    false
                } else {
                    self.error.clear();
                    true
                }
            }
            2 => self.rebuild(),
            _ => self.figure.is_some(),
        };
        if ok {
            self.step += 1;
        }
        ok
    }

    pub fn back(&mut self) {
        self.step = self.step.saturating_sub(1).max(1);
        self.error.clear();
    }

    /// Draws the wizard. Returns true when the user asks to leave it (back to the chart setup).
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut leave = false;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS_BG).inner_margin(16.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(egui::RichText::new(format!("Make a 3D figure: step {} of {STEPS}: {}", self.step, TITLES[self.step - 1])).strong());
            ui.separator();
            match self.step {
                1 => self.step1(ui),
                2 => self.step2(ui),
                3 => self.step3(ui),
                4 => self.step4(ui),
                _ => self.step5(ui),
            }
            ui.separator();
            ui.horizontal(|ui| {
                if self.step == 1 {
                    if ui.button("< Back to charts").clicked() {
                        leave = true;
                    }
                } else if ui.button("< Back").clicked() {
                    self.back();
                }
                if self.step < STEPS && ui.button(if self.step == 2 { "Make the parts >" } else { "Next >" }).clicked() {
                    self.go_next();
                }
                if !self.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(178, 40, 40), &self.error);
                }
            });
        });
        leave
    }

    fn step1(&mut self, ui: &mut egui::Ui) {
        ui.label("Start from a 3D model (STL or OBJ) of a rounded figure, or try a built-in sample.");
        ui.horizontal(|ui| {
            if ui.button("Use the sample bear").clicked() {
                self.use_sample("bear", 8.0);
            }
            if ui.button("Sample snowman").clicked() {
                self.use_sample("snowman", 6.0);
            }
            if ui.button("Sample ball").clicked() {
                self.use_sample("sphere", 4.0);
            }
        });
        if self.can_read_paths {
            ui.horizontal(|ui| {
                ui.label("or a file:");
                ui.add(egui::TextEdit::singleline(&mut self.path).hint_text("path to a .stl or .obj file").desired_width(320.0));
                if ui.button("Load").clicked() && !self.path.trim().is_empty() {
                    self.load_request = Some(self.path.trim().to_string());
                }
            });
        } else if ui.button("Choose a model file (STL or OBJ)...").clicked() {
            self.load_request = Some(String::new());
        }
        if self.raw.is_some() {
            ui.label(egui::RichText::new(format!("Model loaded: {}", self.name)).strong());
            for n in &self.notes {
                ui.label(n);
            }
        }
        ui.label(egui::RichText::new("Your model stays on this computer. Use only models you made or have the right to use.").color(theme::HINT));
    }

    fn step2(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Finished height:");
            ui.add(egui::DragValue::new(&mut self.height).range(0.5..=200.0).speed(0.1));
            ui.selectable_value(&mut self.unit, Unit::Inches, "in");
            ui.selectable_value(&mut self.unit, Unit::Cm, "cm");
        });
        ui.horizontal(|ui| {
            ui.label("Which way is up in the file:");
            egui::ComboBox::from_id_salt("up")
                .selected_text(match self.up {
                    None => "Automatic (longest side)",
                    Some(Axis::X) => "X",
                    Some(Axis::Y) => "Y",
                    Some(Axis::Z) => "Z",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.up, None, "Automatic (longest side)");
                    ui.selectable_value(&mut self.up, Some(Axis::X), "X");
                    ui.selectable_value(&mut self.up, Some(Axis::Y), "Y");
                    ui.selectable_value(&mut self.up, Some(Axis::Z), "Z");
                });
        });
        let before = self.weight;
        ui.horizontal(|ui| {
            ui.label("Yarn weight:");
            egui::ComboBox::from_id_salt("weight").selected_text(self.weight.label()).show_ui(ui, |ui| {
                for w in YarnWeight::ALL {
                    ui.selectable_value(&mut self.weight, w, w.label());
                }
            });
        });
        if self.weight != before {
            (self.stitches, self.rows) = self.weight.amigurumi_gauge();
        }
        let (us, mm) = self.weight.amigurumi_hook();
        ui.label(format!("Suggested hook: US {us} ({mm}), a tight fabric so the stuffing does not show."));
        ui.horizontal(|ui| {
            ui.label("My gauge (single crochet, over 4 in):");
            ui.add(egui::DragValue::new(&mut self.stitches).range(4.0..=60.0).suffix(" sts"));
            ui.add(egui::DragValue::new(&mut self.rows).range(4.0..=60.0).suffix(" rounds"));
        });
        ui.label("Not sure? Crochet a 4 inch swatch in the round and count; the numbers above are a starting guess.");
        ui.horizontal(|ui| {
            ui.label("Terms:");
            ui.selectable_value(&mut self.terms, Terms::Us, "US (sc)");
            ui.selectable_value(&mut self.terms, Terms::Uk, "UK (dc)");
        });
        ui.horizontal(|ui| {
            ui.label("Pattern name:");
            ui.add(egui::TextEdit::singleline(&mut self.title).desired_width(220.0));
            ui.label("Designer:");
            ui.add(egui::TextEdit::singleline(&mut self.designer).desired_width(160.0));
        });
    }

    fn step3(&mut self, ui: &mut egui::Ui) {
        let Some(figure) = self.figure.clone() else { return };
        let before = self.manual.clone();
        ui.label("The figure is split where the shape narrows or a limb joins. Each part is crocheted on its own and sewn on.");
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            for (i, p) in figure.parts.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    let c = yarnfoundry_render::view3d::PART_COLOURS[i % 8];
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                    ui.label(format!("{}: {} rounds, {} stitches", p.name, p.rounds.len(), p.rounds.iter().map(|r| r.stitches).sum::<u32>()));
                });
            }
        });
        for (i, z) in figure.auto_splits.iter().enumerate() {
            let mut on = !self.manual.remove.contains(&i);
            if ui.checkbox(&mut on, format!("Cut at the neck at {z:.1} in")).changed() {
                self.manual.remove.retain(|&x| x != i);
                if !on {
                    self.manual.remove.push(i);
                }
            }
        }
        let mut drop_at = None;
        for (i, z) in self.manual.add.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Extra cut at {z:.1} in"));
                if ui.button("Remove").clicked() {
                    drop_at = Some(i);
                }
            });
        }
        if let Some(i) = drop_at {
            self.manual.add.remove(i);
        }
        ui.horizontal(|ui| {
            ui.label("Add a cut at height:");
            ui.add(egui::DragValue::new(&mut self.add_at).range(0.1..=200.0).speed(0.05).suffix(" in"));
            if ui.button("Add cut").clicked() {
                self.manual.add.push(self.add_at);
            }
        });
        if self.manual != before {
            self.rebuild();
        }
    }

    fn step4(&mut self, ui: &mut egui::Ui) {
        if let (Some(mesh), Some(figure)) = (&self.fitted, &self.figure) {
            self.view.ui(ui, mesh, figure);
        }
    }

    fn step5(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (label, paper) in [("Save PDF (Letter)", Paper::Letter), ("Save PDF (A4)", Paper::A4)] {
                if ui.button(label).clicked() {
                    self.pdf_request = Some(paper);
                }
            }
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
        });
        let total: u32 = self.patterns.iter().map(|p| p.total_stitches).sum();
        let yards: f64 = self.patterns.iter().map(|p| p.yards).sum();
        ui.label(format!(
            "{} part(s), {} rounds, {total} stitches, about {:.0} yd of {} yarn (an estimate).",
            self.patterns.len(),
            self.patterns.iter().map(|p| p.rounds.len()).sum::<usize>(),
            yards.ceil(),
            self.weight.label().to_lowercase()
        ));
        egui::ScrollArea::vertical().show(ui, |ui| {
            for p in &self.patterns {
                egui::CollapsingHeader::new(format!("{} ({} rounds)", p.name, p.rounds.len())).default_open(false).show(ui, |ui| {
                    for r in &p.rounds {
                        ui.label(&r.text);
                    }
                    for l in p.finishing.iter().chain(&p.notes) {
                        ui.label(egui::RichText::new(l).italics());
                    }
                });
            }
            if let Some(f) = &self.figure {
                ui.heading("Assembly");
                for n in f.assembly_notes() {
                    ui.label(n);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: &mut FigureWizard) -> bool {
        let ctx = egui::Context::default();
        let mut leave = false;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| leave = w.ui(ui));
        out.textures_delta.clear();
        leave
    }

    #[test]
    fn the_sample_bear_goes_from_load_to_a_pdf_in_five_steps() {
        let mut w = FigureWizard::new(true);
        assert!(!w.go_next(), "a model is needed first");
        assert!(!w.error().is_empty());
        w.use_sample("bear", 8.0);
        assert!(w.go_next() && w.step() == 2);
        assert!(w.go_next() && w.step() == 3);
        assert!(w.figure().is_some_and(|f| f.parts.len() >= 5));
        assert!(w.go_next() && w.step() == 4 && w.go_next() && w.step() == 5);
        for _ in 0..5 {
            assert!(!frame(&mut w));
            w.back();
        }
        let mut w = FigureWizard::with_model(models::bear(), "bear", 8.0, 5);
        frame(&mut w);
        w.pdf_request = Some(Paper::Letter);
        let (name, bytes) = w.take_pdf().expect("a pdf");
        assert!(name.ends_with("-letter.pdf") && bytes.starts_with(b"%PDF-"), "{name}");
        assert!(w.take_pdf().is_none());
    }

    #[test]
    fn bad_files_and_numbers_are_messages_not_crashes() {
        let mut w = FigureWizard::new(false);
        w.use_model_bytes("junk", b"not a model at all");
        assert!(!w.error().is_empty() && !w.go_next());
        w.use_model_bytes("empty", &[]);
        assert!(!w.error().is_empty());
        w.use_sample("bear", 8.0);
        w.height = 0.0;
        w.go_next();
        assert!(!w.go_next() && w.step() == 2 && !w.error().is_empty());
        w.height = f64::NAN;
        assert!(!w.rebuild());
        w.height = 6.0;
        w.stitches = 0.0;
        assert!(!w.rebuild());
        w.stitches = 20.0;
        assert!(w.rebuild());
        assert!(w.take_load_request().is_none());
    }

    #[test]
    fn manual_cuts_change_the_parts() {
        let mut w = FigureWizard::with_model(models::snowman(), "snowman", 6.0, 3);
        assert_eq!(w.figure().map(|f| f.parts.len()), Some(2));
        w.manual.remove.push(0);
        w.rebuild();
        assert_eq!(w.figure().map(|f| f.parts.len()), Some(1));
        w.manual.add.push(4.0);
        w.rebuild();
        assert_eq!(w.figure().map(|f| f.parts.len()), Some(2));
        frame(&mut w);
    }

    #[test]
    fn a_model_file_loads_and_back_leaves_the_wizard() {
        let mut w = FigureWizard::new(true);
        let stl = models::capsule(1.0, 2.0).to_binary_stl();
        w.use_model_bytes("capsule.stl", &stl);
        assert!(w.error().is_empty());
        assert!(w.go_next() && w.go_next());
        assert_eq!(w.figure().map(|f| f.parts.len()), Some(1));
        let mut w = FigureWizard::new(true);
        assert!(frame(&mut w) || w.step() == 1);
    }
}
