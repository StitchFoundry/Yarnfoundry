//! YarnFoundry UI: a thin egui front end. Panels read core state; colours come from [`theme`].

pub mod figure3d;
pub mod figure_wizard;

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

/// The application state shown in the window: the five-step setup, ending in the chart preview (step 5).
pub struct YarnFoundryApp {
    /// 1 to 4 are setup pages; 5 is the preview of `chart`.
    step: usize,
    setup: yarnfoundry_core::Setup,
    words: String,
    /// Step 2: words (true) or a picture (false).
    words_mode: bool,
    /// The last picture chosen, kept when switching to words and back.
    picture: Option<yarnfoundry_core::Picture>,
    chart: Option<yarnfoundry_core::Chart>,
    report: Option<yarnfoundry_core::TechniqueReport>,
    /// The chart came from a file, so there is no setup to go back to.
    opened_from_file: bool,
    picture_path: String,
    width_text: f64,
    error: String,
    message: String,
    export: Option<PrintRequest>,
    save_project: bool,
    load_picture: Option<String>,
    /// False in the browser: no file paths, only a file chooser.
    can_read_paths: bool,
    /// The 3D figure wizard, when it is open (instead of the chart setup).
    wizard: Option<figure_wizard::FigureWizard>,
    /// The "Feedback / report a problem" panel is showing.
    feedback_open: bool,
}

impl Default for YarnFoundryApp {
    fn default() -> Self {
        Self::new()
    }
}

/// A print the user asked for; the host app does the file writing (this crate does no I/O).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintRequest {
    pub paper: yarnfoundry_render::Paper,
    pub look: yarnfoundry_render::Look,
}

/// The sample picture offered in step 2, so a new user can try the app without any file (also in the browser).
static SAMPLE_PICTURE: &[u8] = include_bytes!("../../../assets/samples/sunset-hills.png");

/// Where feedback goes for now (until the web form ships).
pub const FEEDBACK_EMAIL: &str = "hello@sitesentrix.com";
pub const FEEDBACK_FACEBOOK: &str = "https://www.facebook.com/stitchfoundryapp";

/// A `mailto:` link with the subject and the app version prefilled. A convenience only: the address is also shown to copy.
pub fn feedback_mailto(version: &str) -> String {
    format!("mailto:{FEEDBACK_EMAIL}?subject=%5BYarnFoundry%5D%20Feedback&body=App%20version%3A%20{version}%0A%0A")
}

pub const STEP_TITLES: [&str; yarnfoundry_core::setup::STEPS] =
    ["What are you making?", "Picture or words?", "Size and gauge", "Colours", "Preview, print, save"];

impl YarnFoundryApp {
    /// The setup, starting at step 1.
    pub fn new() -> Self {
        Self {
            step: 1,
            setup: yarnfoundry_core::Setup::default(),
            words: String::new(),
            words_mode: true,
            picture: None,
            chart: None,
            report: None,
            opened_from_file: false,
            picture_path: String::new(),
            width_text: 8.0,
            error: String::new(),
            message: String::new(),
            export: None,
            save_project: false,
            load_picture: None,
            can_read_paths: true,
            wizard: None,
            feedback_open: false,
        }
    }

    /// An app with the 3D figure wizard open on a loaded model (for demos and screenshots), at `step` of 5.
    pub fn with_model(raw: yarnfoundry_core::Mesh, name: &str, height_in: f64, step: usize) -> Self {
        Self { wizard: Some(figure_wizard::FigureWizard::with_model(raw, name, height_in, step)), ..Self::new() }
    }

    /// Opens the 3D figure wizard.
    pub fn open_figure_wizard(&mut self) {
        self.wizard = Some(figure_wizard::FigureWizard::new(self.can_read_paths));
    }

    /// A model file path the user asked to load (empty: open a file chooser); the host reads it and calls [`Self::use_model_bytes`].
    pub fn take_load_model(&mut self) -> Option<String> {
        self.wizard.as_mut().and_then(figure_wizard::FigureWizard::take_load_request)
    }

    /// Hands the wizard the bytes of a model file (STL or OBJ).
    pub fn use_model_bytes(&mut self, name: &str, bytes: &[u8]) {
        if let Some(w) = &mut self.wizard {
            w.use_model_bytes(name, bytes);
        }
    }

    /// The finished figure pattern as a PDF (file name and bytes), if the user asked for one.
    pub fn take_figure_pdf(&mut self) -> Option<(String, Vec<u8>)> {
        self.wizard.as_mut().and_then(figure_wizard::FigureWizard::take_pdf)
    }

    /// An app showing `chart` (opened from a file).
    pub fn with_chart(chart: yarnfoundry_core::Chart) -> Self {
        Self { step: yarnfoundry_core::setup::STEPS, chart: Some(chart), opened_from_file: true, ..Self::new() }
    }

    /// The setup at `step` with the words KING CASHEW filled in (for screenshots and demos). Step 5 builds the chart.
    pub fn demo_at_step(step: usize) -> Self {
        use yarnfoundry_core::{Colours, Size, Source};
        let mut app = Self::new();
        app.words = "KING CASHEW".to_string();
        app.setup.source = Some(Source::Words(app.words.clone()));
        app.setup.size = Size::Stitches(180);
        app.setup.colours = Colours::Preset { preset: 0, picked: vec![3, 4] };
        app.step = step.clamp(1, yarnfoundry_core::setup::STEPS);
        if app.step == yarnfoundry_core::setup::STEPS {
            app.build_preview();
        }
        app
    }

    /// Whether the host can read a file by path (desktop). In the browser the picture step offers a file chooser instead.
    pub fn set_can_read_paths(&mut self, yes: bool) {
        self.can_read_paths = yes;
        if let Some(w) = &mut self.wizard {
            w.set_can_read_paths(yes);
        }
    }

    pub fn step(&self) -> usize {
        self.step
    }

    pub fn setup_mut(&mut self) -> &mut yarnfoundry_core::Setup {
        &mut self.setup
    }

    pub fn error(&self) -> &str {
        &self.error
    }

    /// Goes to the next step if this one is complete; at step 4 the chart is built. Returns whether it moved.
    pub fn go_next(&mut self) -> bool {
        if self.step >= yarnfoundry_core::setup::STEPS {
            return false;
        }
        if self.step == 2 {
            self.apply_source();
        }
        if let Err(e) = self.setup.check_step(self.step) {
            self.error = e.to_string();
            return false;
        }
        self.error.clear();
        self.step += 1;
        if self.step == yarnfoundry_core::setup::STEPS {
            self.build_preview();
            if self.chart.is_none() {
                self.step -= 1;
                return false;
            }
        }
        true
    }

    /// Goes back one step (from a chart opened from a file there is nothing to go back to).
    pub fn back(&mut self) {
        if self.step > 1 && !self.opened_from_file {
            self.step -= 1;
            self.error.clear();
        }
    }

    /// Starts a new chart from step 1.
    pub fn restart(&mut self) {
        *self = Self::new();
    }

    /// Puts the step-2 choice into the setup.
    fn apply_source(&mut self) {
        self.setup.source = if self.words_mode {
            Some(yarnfoundry_core::Source::Words(self.words.clone()))
        } else {
            self.picture.clone().map(yarnfoundry_core::Source::Picture)
        };
    }

    fn build_preview(&mut self) {
        match self.setup.build() {
            Ok((chart, report)) => {
                self.chart = Some(chart);
                self.report = Some(report);
                self.error.clear();
            }
            Err(e) => {
                self.chart = None;
                self.report = None;
                self.error = e.to_string();
            }
        }
    }

    /// Uses `bytes` (a PNG, JPEG or WebP file) as the picture.
    pub fn use_picture_bytes(&mut self, bytes: &[u8]) {
        match yarnfoundry_core::Picture::decode(bytes) {
            Ok(pic) => {
                self.picture = Some(pic);
                self.words_mode = false;
                self.apply_source();
                self.error.clear();
            }
            Err(e) => self.error = e.to_string(),
        }
    }

    /// The print the user asked for since the last call, if any.
    pub fn take_print_request(&mut self) -> Option<PrintRequest> {
        self.export.take()
    }

    /// Whether the user asked to save the project file since the last call.
    pub fn take_save_project(&mut self) -> bool {
        std::mem::take(&mut self.save_project)
    }

    /// A picture file path the user asked to load since the last call (the host reads it and calls [`Self::use_picture_bytes`]).
    pub fn take_load_picture(&mut self) -> Option<String> {
        self.load_picture.take()
    }

    /// The chart on screen.
    pub fn chart(&self) -> Option<&yarnfoundry_core::Chart> {
        self.chart.as_ref()
    }

    /// Shows a line of feedback (for example where a file was saved).
    pub fn set_message(&mut self, message: String) {
        if let Some(w) = &mut self.wizard {
            w.set_message(message.clone());
        }
        self.message = message;
    }

    /// Draws one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.ctx().set_visuals(egui::Visuals::light());
        egui::Panel::top("title").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(yarnfoundry_core::APP_NAME);
                ui.label(format!("v{}", yarnfoundry_core::VERSION));
                if ui.button("Feedback / report a problem").clicked() {
                    self.feedback_open = !self.feedback_open;
                }
                ui.separator();
                let step = self.step.clamp(1, yarnfoundry_core::setup::STEPS);
                if self.wizard.is_some() {
                    ui.label(egui::RichText::new("3D figure: from model to crochet pattern").strong());
                } else {
                    ui.label(egui::RichText::new(format!("Step {step} of {}: {}", yarnfoundry_core::setup::STEPS, STEP_TITLES[step - 1])).strong());
                }
                if let (5, Some(chart)) = (self.step, &self.chart) {
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
        if let Some(w) = &mut self.wizard {
            if w.ui(ui) {
                self.wizard = None;
            }
        } else if self.step == yarnfoundry_core::setup::STEPS && self.chart.is_some() {
            self.preview_ui(ui);
        } else {
            self.setup_ui(ui);
        }
        self.feedback_ui(ui.ctx());
    }

    /// The small feedback panel: plain wording, the address to copy, and the app version.
    fn feedback_ui(&mut self, ctx: &egui::Context) {
        if !self.feedback_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Feedback / report a problem").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Tell us what's wrong or what you'd like.");
            ui.label(format!("Email {FEEDBACK_EMAIL}"));
            ui.horizontal(|ui| {
                if ui.button("Copy address").clicked() {
                    ui.ctx().copy_text(FEEDBACK_EMAIL.to_string());
                }
                ui.hyperlink_to("Open email", feedback_mailto(yarnfoundry_core::VERSION));
            });
            ui.label("or message us on Facebook:");
            ui.hyperlink(FEEDBACK_FACEBOOK);
            ui.label(egui::RichText::new(format!("YarnFoundry version {}", yarnfoundry_core::VERSION)).color(theme::HINT));
        });
        self.feedback_open = open;
    }

    fn nav_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal(|ui| {
            if ui.add_enabled(self.step > 1, egui::Button::new("< Back")).clicked() {
                self.back();
            }
            let last_setup = self.step == yarnfoundry_core::setup::STEPS - 1;
            if ui.button(if last_setup { "Make my chart >" } else { "Next >" }).clicked() {
                self.go_next();
            }
            if !self.error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(178, 40, 40), &self.error);
            }
        });
    }

    fn setup_ui(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS_BG).inner_margin(20.0)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            match self.step {
                1 => self.step1(ui),
                2 => self.step2(ui),
                3 => self.step3(ui),
                _ => self.step4(ui),
            }
            self.nav_ui(ui);
        });
    }

    fn step1(&mut self, ui: &mut egui::Ui) {
        use yarnfoundry_core::{Construction, Craft};
        if ui.button("Make a 3D figure (amigurumi from a 3D model) instead >").clicked() {
            self.open_figure_wizard();
        }
        ui.separator();
        ui.label("I want to:");
        let mut craft = self.setup.craft;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut craft, Craft::Knit, "Knit");
            ui.selectable_value(&mut craft, Craft::Crochet, "Crochet");
        });
        if craft != self.setup.craft {
            self.setup.set_craft(craft);
        }
        ui.label("Colorwork technique:");
        ui.horizontal(|ui| {
            for &t in yarnfoundry_core::techniques_for(self.setup.craft) {
                ui.selectable_value(&mut self.setup.technique, t, yarnfoundry_core::setup::technique_label(t));
            }
        });
        ui.label(match self.setup.technique {
            yarnfoundry_core::Technique::Intarsia => "Intarsia: blocks of colour, a separate bobbin for each area.",
            yarnfoundry_core::Technique::FairIsle => "Fair isle: two colours a row, the unused yarn carried behind.",
            yarnfoundry_core::Technique::Tapestry => "Tapestry crochet: single crochet, colours carried inside the stitches.",
            yarnfoundry_core::Technique::Filet => "Filet crochet: filled and open squares, three stitches a square.",
        });
        ui.label("Worked:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.setup.construction, Construction::Flat, "Flat (back and forth)");
            ui.selectable_value(&mut self.setup.construction, Construction::Round, "In the round");
        });
    }

    fn step2(&mut self, ui: &mut egui::Ui) {
        ui.label("What should the chart show?");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.words_mode, true, "Words");
            ui.selectable_value(&mut self.words_mode, false, "A picture");
        });
        if self.words_mode {
            ui.label("Type your words (a name, a saying):");
            ui.add(egui::TextEdit::singleline(&mut self.words).hint_text("KING CASHEW").desired_width(360.0));
            ui.label("Letters are bold and at least 20 rows tall so they read clearly.");
        } else {
            if ui.button("Use the sample picture (sunset over hills)").clicked() {
                self.use_picture_bytes(SAMPLE_PICTURE);
            }
            if self.can_read_paths {
                ui.horizontal(|ui| {
                    ui.label("or a file (PNG, JPEG, WebP):");
                    ui.add(egui::TextEdit::singleline(&mut self.picture_path).hint_text("path to a picture").desired_width(300.0));
                    if ui.button("Load").clicked() && !self.picture_path.trim().is_empty() {
                        self.load_picture = Some(self.picture_path.trim().to_string());
                    }
                });
            } else if ui.button("Choose a picture file (PNG, JPEG, WebP)...").clicked() {
                self.load_picture = Some(String::new()); // empty: the host opens its own chooser
            }
            if let Some(pic) = &self.picture {
                ui.label(format!("Picture loaded: {} x {} pixels", pic.width, pic.height));
            }
        }
        self.apply_source();
    }

    fn step3(&mut self, ui: &mut egui::Ui) {
        use yarnfoundry_core::{Size, Unit, YarnWeight};
        ui.horizontal(|ui| {
            ui.label("Measure in:");
            ui.selectable_value(&mut self.setup.unit, Unit::Inches, "inches");
            ui.selectable_value(&mut self.setup.unit, Unit::Cm, "cm");
        });
        let unit = if self.setup.unit == Unit::Cm { "cm" } else { "in" };
        let mut by_width = matches!(self.setup.size, Size::Width(_));
        ui.horizontal(|ui| {
            ui.label("Size by:");
            ui.selectable_value(&mut by_width, true, "finished width");
            ui.selectable_value(&mut by_width, false, "stitches across");
        });
        match (by_width, self.setup.size) {
            (true, Size::Width(w)) => {
                let mut w = w;
                ui.horizontal(|ui| {
                    ui.label("Finished width:");
                    ui.add(egui::DragValue::new(&mut w).range(0.5..=120.0).speed(0.1).suffix(format!(" {unit}")));
                });
                self.setup.size = Size::Width(w);
            }
            (true, _) => self.setup.size = Size::Width(self.width_text),
            (false, Size::Stitches(n)) => {
                let mut n = n;
                ui.horizontal(|ui| {
                    ui.label("Stitches across:");
                    ui.add(egui::DragValue::new(&mut n).range(1..=yarnfoundry_core::setup::MAX_WIDTH));
                });
                self.setup.size = Size::Stitches(n);
            }
            (false, _) => self.setup.size = Size::Stitches(60),
        }
        ui.separator();
        let mut knows = self.setup.known_gauge.is_some();
        ui.checkbox(&mut knows, "I know my gauge");
        if knows && self.setup.known_gauge.is_none() {
            self.setup.known_gauge = self.setup.gauge().ok();
        } else if !knows {
            self.setup.known_gauge = None;
        }
        if let Some(g) = &mut self.setup.known_gauge {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut g.stitches).range(1.0..=60.0).speed(0.1));
                ui.label("stitches and");
                ui.add(egui::DragValue::new(&mut g.rows).range(1.0..=80.0).speed(0.1));
                ui.label(format!("rows over {} {unit}", g.measure));
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("I don't know my gauge. My yarn is:");
                egui::ComboBox::from_id_salt("yarn-weight").selected_text(self.setup.yarn_weight.label()).show_ui(ui, |ui| {
                    for w in YarnWeight::ALL {
                        ui.selectable_value(&mut self.setup.yarn_weight, w, w.label());
                    }
                });
            });
        }
        if let Ok(g) = self.setup.gauge() {
            let stitches = self.setup.stitches(&g).map_or_else(|_| "?".to_string(), |n| n.to_string());
            ui.label(format!("Gauge used: {} stitches and {} rows over {} {unit}. Chart width: {stitches} stitches.", g.stitches, g.rows, g.measure));
        }
        if let Size::Width(w) = self.setup.size {
            self.width_text = w;
        }
    }

    fn step4(&mut self, ui: &mut egui::Ui) {
        use yarnfoundry_core::{Colours, Source};
        let is_words = matches!(self.setup.source, Some(Source::Words(_)));
        let all = yarnfoundry_core::presets();
        let (mut preset, mut picked) = match &self.setup.colours {
            Colours::Preset { preset, picked } => (*preset, picked.clone()),
            Colours::FromPicture => (0, vec![0, 1]),
        };
        let mut from_picture = matches!(self.setup.colours, Colours::FromPicture);
        if !is_words {
            ui.horizontal(|ui| {
                ui.label("Colours:");
                ui.selectable_value(&mut from_picture, true, "pick them from my picture");
                ui.selectable_value(&mut from_picture, false, "choose from a palette");
            });
        } else {
            from_picture = false;
            if picked.len() != 2 {
                picked = vec![0, 2];
            }
            ui.label("Pick two colours: click one swatch for the background, then another for the letters.");
        }
        if from_picture {
            ui.horizontal(|ui| {
                ui.label("How many colours?");
                ui.add(
                    egui::DragValue::new(&mut self.setup.colour_count)
                        .range(yarnfoundry_core::setup::MIN_COLOURS..=yarnfoundry_core::setup::MAX_COLOURS),
                );
            });
            self.setup.colours = Colours::FromPicture;
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Palette:");
            for (i, p) in all.iter().enumerate() {
                if ui.selectable_label(preset == i, p.name).clicked() && preset != i {
                    preset = i;
                    picked = if is_words { vec![0, 2] } else { vec![0, 1] };
                }
            }
        });
        let colours = all.get(preset).map(|p| p.colours.clone()).unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            for (i, col) in colours.iter().enumerate() {
                let on = picked.contains(&i);
                let label = picked.iter().position(|&p| p == i).map_or_else(String::new, |pos| {
                    if is_words { if pos == 0 { "background".into() } else { "letters".into() } } else { (pos + 1).to_string() }
                });
                ui.vertical(|ui| {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(72.0, 44.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 6.0, egui::Color32::from_rgb(col.rgb[0], col.rgb[1], col.rgb[2]));
                    let stroke = if on {
                        egui::Stroke::new(3.0, egui::Color32::from_rgb(40, 90, 200))
                    } else {
                        egui::Stroke::new(1.0, theme::GRID_LINE_MAJOR)
                    };
                    ui.painter().rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
                    ui.label(format!("{}{}", col.name, if label.is_empty() { String::new() } else { format!(" ({label})") }));
                    if resp.clicked() {
                        if is_words {
                            if on {
                                picked.retain(|&p| p != i);
                            } else {
                                picked.push(i);
                                if picked.len() > 2 {
                                    picked.remove(0);
                                }
                            }
                        } else if on {
                            picked.retain(|&p| p != i);
                        } else if picked.len() < yarnfoundry_core::setup::MAX_COLOURS {
                            picked.push(i);
                        }
                    }
                });
            }
        });
        if !is_words {
            ui.label(format!("{} colours picked (2 to 8).", picked.len()));
        }
        self.setup.colours = Colours::Preset { preset, picked };
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let Some(chart) = self.chart.clone() else { return };
        egui::Panel::right("legend").default_size(250.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                if !self.opened_from_file && ui.button("< Change setup").clicked() {
                    self.back();
                }
                if ui.button("New chart").clicked() {
                    self.restart();
                }
            });
            if ui.button("Feedback / report a problem").clicked() {
                self.feedback_open = true;
            }
            ui.heading("Colours");
            legend(ui, &chart);
            if let Some(report) = &self.report {
                for w in report.warnings.iter().take(3) {
                    ui.colored_label(egui::Color32::from_rgb(160, 90, 10), w);
                }
            }
            ui.separator();
            ui.label("Print at true size:");
            use yarnfoundry_render::{Look, Paper};
            for (label, paper, look) in [
                ("PDF, Letter, colour", Paper::Letter, Look::Colour),
                ("PDF, A4, colour", Paper::A4, Look::Colour),
                ("PDF, Letter, black and white symbols", Paper::Letter, Look::BlackWhite),
                ("PDF, A4, black and white symbols", Paper::A4, Look::BlackWhite),
            ] {
                if ui.button(label).clicked() {
                    self.export = Some(PrintRequest { paper, look });
                }
            }
            if ui.button("Save project (.yarnfoundry)").clicked() {
                self.save_project = true;
            }
            if !self.message.is_empty() {
                ui.label(&self.message);
            }
        });
        egui::Panel::bottom("rows").default_size(150.0).show(ui, |ui| {
            egui::CollapsingHeader::new("Written instructions").default_open(true).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    for row in yarnfoundry_core::instructions(&chart).rows.iter().take(60) {
                        ui.label(&row.text);
                    }
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::CANVAS_BG)).show(ui, |ui| {
            let rect = ui.available_rect_before_wrap();
            paint_chart(&ui.painter_at(rect), rect, &chart);
        });
    }
}

/// Colour swatches with their names and stitch counts.
fn legend(ui: &mut egui::Ui, chart: &yarnfoundry_core::Chart) {
    let mut counts = vec![0u32; chart.palette.len()];
    for &c in &chart.cells {
        if let Some(n) = counts.get_mut(usize::from(c)) {
            *n += 1;
        }
    }
    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
        for (colour, count) in chart.palette.iter().zip(&counts) {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(colour.rgb[0], colour.rgb[1], colour.rgb[2]));
                ui.label(format!("{} ({count})", colour.name));
            });
        }
    });
}

/// Paints `chart` scaled to fit `rect` (row 0, the first row worked, at the bottom), with a stronger line every 10 stitches/rows.
pub fn paint_chart(painter: &egui::Painter, rect: egui::Rect, chart: &yarnfoundry_core::Chart) {
    let margin = 16.0;
    let gutter = 28.0; // room for the row numbers on both sides and the stitch numbers below
    let avail = egui::Rect::from_min_max(
        egui::pos2(rect.left() + margin + gutter, rect.top() + margin),
        egui::pos2(rect.right() - margin - gutter, rect.bottom() - margin - 14.0),
    );
    // A stitch is wider than it is tall (rows per inch > stitches per inch), so draw cells in that proportion.
    let aspect = chart.gauge.stitch_aspect() as f32;
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    let cell = (avail.width() / chart.width as f32).min(avail.height() * aspect / chart.height as f32).max(1.0);
    let cell_h = cell / aspect;
    let size = egui::vec2(cell * chart.width as f32, cell_h * chart.height as f32);
    let origin = egui::pos2(avail.center().x - size.x / 2.0, avail.center().y - size.y / 2.0);
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
    // Row numbers on the side each row is worked from (odd right-side rows on the right; rounds always on the right),
    // stitch numbers from the right edge under the chart. Thin out the numbers when cells get small.
    let step = if cell_h >= 9.0 {
        1
    } else if cell_h >= 4.5 {
        2
    } else {
        5
    };
    let font = egui::FontId::proportional(cell_h.clamp(7.0, 11.0));
    for y in 0..chart.height {
        if (y + 1) % step != 0 && y != 0 {
            continue;
        }
        let py = origin.y + size.y - (y as f32 + 0.5) * cell_h;
        let label = (y + 1).to_string();
        if yarnfoundry_render::row_number_on_right(chart, y) {
            painter.text(egui::pos2(origin.x + size.x + 4.0, py), egui::Align2::LEFT_CENTER, label, font.clone(), theme::HINT);
        } else {
            painter.text(egui::pos2(origin.x - 4.0, py), egui::Align2::RIGHT_CENTER, label, font.clone(), theme::HINT);
        }
    }
    let cstep = if cell >= 14.0 {
        1
    } else if cell >= 5.0 {
        5
    } else {
        10
    };
    for n in 1..=chart.width {
        if n == 1 || n % 10 == 0 || (cstep < 10 && n % cstep == 0) {
            let px = origin.x + size.x - (n as f32 - 0.5) * cell;
            painter.text(
                egui::pos2(px, origin.y + size.y + 3.0),
                egui::Align2::CENTER_TOP,
                n.to_string(),
                egui::FontId::proportional(9.0),
                theme::HINT,
            );
        }
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

    /// Runs one frame and returns all the text it drew.
    fn frame_text(ctx: &egui::Context, app: &mut YarnFoundryApp) -> String {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
        out.textures_delta.clear();
        let mut text = String::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &clipped.shape {
                text.push_str(t.galley.text());
                text.push('\n');
            }
        }
        text
    }

    #[test]
    fn the_feedback_panel_renders_with_the_version_and_both_ways_to_reach_us() {
        let ctx = egui::Context::default();
        let mut app = YarnFoundryApp::new();
        let closed = frame_text(&ctx, &mut app);
        assert!(closed.contains("Feedback / report a problem"), "button is in the top bar");
        assert!(!closed.contains(FEEDBACK_EMAIL));
        app.feedback_open = true;
        // A window is sized on its first frame and drawn on the next.
        frame_text(&ctx, &mut app);
        let text = frame_text(&ctx, &mut app);
        assert!(text.contains(FEEDBACK_EMAIL), "{text}");
        assert!(text.contains("Copy address") && text.contains("Open email"), "{text}");
        assert!(text.contains(FEEDBACK_FACEBOOK), "{text}");
        assert!(text.contains(&format!("YarnFoundry version {}", yarnfoundry_core::VERSION)), "{text}");
    }

    #[test]
    fn the_mailto_link_has_the_subject_and_version() {
        let link = feedback_mailto("1.2.3");
        assert!(link.starts_with("mailto:hello@sitesentrix.com?subject=%5BYarnFoundry%5D%20Feedback"));
        assert!(link.contains("App%20version%3A%201.2.3"));
    }

    #[test]
    fn the_feedback_button_is_on_the_preview_screen_too() {
        let mut app = YarnFoundryApp::demo_at_step(5);
        let text = frame_text(&egui::Context::default(), &mut app);
        assert_eq!(text.matches("Feedback / report a problem").count(), 2, "top bar and step 5 panel");
    }

    mod wizard {
        use super::*;

        fn frame(app: &mut YarnFoundryApp) -> usize {
            let ctx = egui::Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.ui(ui));
            out.textures_delta.clear();
            out.shapes.len()
        }

        #[test]
        fn a_new_user_gets_from_launch_to_a_chart_in_five_steps_without_an_account() {
            let mut app = YarnFoundryApp::new();
            assert_eq!(app.step(), 1);
            assert!(frame(&mut app) > 0);
            assert!(app.go_next()); // 1 -> 2: the defaults are fine
            assert_eq!(app.step(), 2);
            assert!(!app.go_next(), "no words yet");
            assert!(!app.error().is_empty());
            app.words = "KING CASHEW".into();
            assert!(frame(&mut app) > 0);
            assert!(app.go_next()); // 2 -> 3
            assert!(frame(&mut app) > 0);
            assert!(app.go_next()); // 3 -> 4
            assert!(frame(&mut app) > 0);
            assert!(app.go_next()); // 4 -> 5: the chart is built
            assert_eq!(app.step(), 5);
            let chart = app.chart().expect("a chart");
            let (cells, width, colours) = (chart.cells.len(), chart.width, chart.palette.len());
            assert!(width > 100 && colours == 2);
            assert!(frame(&mut app) > cells, "the preview draws every stitch");
            assert!(!app.go_next(), "step 5 is the last");
            app.back();
            assert_eq!(app.step(), 4);
        }

        #[test]
        fn a_picture_goes_through_the_same_steps_and_the_sample_picture_needs_no_file() {
            let mut app = YarnFoundryApp::new();
            app.go_next();
            app.words_mode = false;
            assert!(!app.go_next(), "a picture must be chosen");
            app.use_picture_bytes(SAMPLE_PICTURE);
            assert!(app.error().is_empty());
            assert!(app.go_next() && app.go_next());
            app.setup_mut().colours = yarnfoundry_core::Colours::FromPicture;
            app.setup_mut().colour_count = 5;
            assert!(app.go_next());
            assert_eq!(app.chart().map(|c| c.palette.len()), Some(5));
        }

        #[test]
        fn bad_pictures_and_bad_colours_are_messages_not_crashes() {
            let mut app = YarnFoundryApp::new();
            app.use_picture_bytes(b"definitely not a picture");
            assert!(!app.error().is_empty());
            let mut app = YarnFoundryApp::demo_at_step(4);
            app.setup_mut().colours = yarnfoundry_core::Colours::Preset { preset: 0, picked: vec![1] };
            assert!(!app.go_next());
            assert_eq!(app.step(), 4);
            assert!(frame(&mut app) > 0);
        }

        #[test]
        fn every_step_draws_and_a_file_chart_has_no_setup_to_go_back_to() {
            for step in 1..=5 {
                let mut app = YarnFoundryApp::demo_at_step(step);
                assert_eq!(app.step(), step);
                assert!(frame(&mut app) > 0, "step {step}");
            }
            let chart = YarnFoundryApp::demo_at_step(5).chart().cloned().unwrap();
            let mut app = YarnFoundryApp::with_chart(chart);
            app.back();
            assert_eq!(app.step(), 5);
            app.restart();
            assert_eq!(app.step(), 1);
        }

        #[test]
        fn switching_craft_moves_the_technique_to_one_that_fits() {
            let mut app = YarnFoundryApp::new();
            app.setup_mut().set_craft(yarnfoundry_core::Craft::Crochet);
            assert_eq!(app.setup_mut().technique, yarnfoundry_core::Technique::Tapestry);
        }
    }
}
