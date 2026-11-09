//! The guided setup: what a new user chooses in five steps, with a default for every choice, and the chart it builds.
//! No UI and no I/O here: the UI keeps a [`Setup`] and calls [`Setup::build`].
//!
//! Steps: 1 craft, technique, flat or round; 2 picture or words; 3 size and gauge ("I don't know my gauge" uses a
//! yarn-weight default); 4 colours (2 to 8, from a generic palette or picked from the picture); 5 preview, print, save.

use crate::chart::{Chart, Construction, Craft, PaletteColor, Technique};
use crate::gauge::{Gauge, GaugeError, Unit};
use crate::picture::{PaletteSpec, Picture, PictureError, PictureOptions, chart_from_picture, rows_for_width};
use crate::technique::{TechniqueOptions, TechniqueReport, apply as apply_technique, filet_stitches};
use crate::text::{TextError, TextOptions, chart_from_text};

/// The setup has this many steps.
pub const STEPS: usize = 5;
pub const MIN_COLOURS: usize = 2;
pub const MAX_COLOURS: usize = 8;
/// Widest chart the setup offers, in stitches.
pub const MAX_WIDTH: u32 = 400;

/// Yarn weights for "I don't know my gauge".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YarnWeight {
    Lace,
    Fingering,
    Sport,
    DK,
    Worsted,
    Aran,
    Bulky,
    SuperBulky,
}

impl YarnWeight {
    pub const ALL: [YarnWeight; 8] = [Self::Lace, Self::Fingering, Self::Sport, Self::DK, Self::Worsted, Self::Aran, Self::Bulky, Self::SuperBulky];

    pub fn label(self) -> &'static str {
        match self {
            Self::Lace => "Lace",
            Self::Fingering => "Fingering (sock)",
            Self::Sport => "Sport",
            Self::DK => "DK (light worsted)",
            Self::Worsted => "Worsted",
            Self::Aran => "Aran",
            Self::Bulky => "Bulky",
            Self::SuperBulky => "Super bulky",
        }
    }

    /// Typical gauge over 4 inches (10 cm): (stitches, rows). Knit stockinette, crochet single crochet.
    /// Round figures from common public yarn-weight tables; a swatch always beats them.
    pub fn default_gauge(self, craft: Craft) -> (f64, f64) {
        match (craft, self) {
            (Craft::Knit, Self::Lace) => (33.0, 44.0),
            (Craft::Knit, Self::Fingering) => (28.0, 36.0),
            (Craft::Knit, Self::Sport) => (24.0, 32.0),
            (Craft::Knit, Self::DK) => (22.0, 30.0),
            (Craft::Knit, Self::Worsted) => (20.0, 28.0),
            (Craft::Knit, Self::Aran) => (18.0, 24.0),
            (Craft::Knit, Self::Bulky) => (14.0, 20.0),
            (Craft::Knit, Self::SuperBulky) => (10.0, 14.0),
            (Craft::Crochet, Self::Lace) => (32.0, 34.0),
            (Craft::Crochet, Self::Fingering) => (26.0, 28.0),
            (Craft::Crochet, Self::Sport) => (22.0, 24.0),
            (Craft::Crochet, Self::DK) => (20.0, 22.0),
            (Craft::Crochet, Self::Worsted) => (16.0, 18.0),
            (Craft::Crochet, Self::Aran) => (14.0, 16.0),
            (Craft::Crochet, Self::Bulky) => (12.0, 13.0),
            (Craft::Crochet, Self::SuperBulky) => (8.0, 9.0),
        }
    }
}

/// A named set of generic colours (no brand names, no colour cards).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub colours: Vec<PaletteColor>,
}

fn c(name: &str, rgb: [u8; 3]) -> PaletteColor {
    PaletteColor { name: name.to_string(), rgb }
}

/// The built-in palettes, 8 colours each, lightest or most neutral first.
pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "Classic",
            colours: vec![
                c("cream", [245, 238, 220]),
                c("black", [28, 28, 30]),
                c("red", [196, 32, 44]),
                c("navy", [28, 40, 86]),
                c("gold", [230, 183, 40]),
                c("forest green", [34, 96, 56]),
                c("gray", [128, 128, 132]),
                c("brown", [110, 70, 44]),
            ],
        },
        Preset {
            name: "Bright",
            colours: vec![
                c("white", [250, 250, 250]),
                c("black", [20, 20, 22]),
                c("bright red", [228, 38, 40]),
                c("orange", [244, 130, 32]),
                c("yellow", [250, 214, 40]),
                c("green", [60, 168, 72]),
                c("sky blue", [64, 160, 224]),
                c("purple", [130, 70, 180]),
            ],
        },
        Preset {
            name: "Pastel",
            colours: vec![
                c("ivory", [252, 248, 236]),
                c("light gray", [210, 210, 214]),
                c("blush pink", [246, 200, 210]),
                c("peach", [250, 210, 176]),
                c("butter yellow", [250, 238, 170]),
                c("mint", [188, 232, 208]),
                c("baby blue", [184, 214, 244]),
                c("lavender", [214, 196, 240]),
            ],
        },
        Preset {
            name: "Earth",
            colours: vec![
                c("oatmeal", [226, 214, 190]),
                c("charcoal", [60, 60, 64]),
                c("rust", [168, 76, 40]),
                c("mustard", [200, 156, 40]),
                c("olive", [112, 118, 56]),
                c("sage", [158, 176, 140]),
                c("teal", [40, 112, 120]),
                c("chocolate", [84, 54, 40]),
            ],
        },
    ]
}

/// Where the colours come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Colours {
    /// Pick the best colours from the picture (not for words).
    FromPicture,
    /// These indices of the chosen preset, in order (for words: background, then lettering).
    Preset { preset: usize, picked: Vec<usize> },
}

/// Picture or words.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Picture(Picture),
    Words(String),
}

/// How the size is given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Size {
    /// Finished width in the gauge unit; the height follows the picture (or the lettering).
    Width(f64),
    /// Stitches across.
    Stitches(u32),
}

/// Everything the five steps collect.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub craft: Craft,
    pub technique: Technique,
    pub construction: Construction,
    pub source: Option<Source>,
    pub size: Size,
    pub unit: Unit,
    /// `None`: "I don't know my gauge", use the yarn weight's default.
    pub known_gauge: Option<Gauge>,
    pub yarn_weight: YarnWeight,
    pub colours: Colours,
    /// Chosen number of colours for a picture with `Colours::FromPicture` (2 to 8).
    pub colour_count: usize,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SetupError {
    #[error("choose a picture or type some words first")]
    NoSource,
    #[error("{0} does not go with {1}")]
    BadTechnique(&'static str, &'static str),
    #[error("pick between {MIN_COLOURS} and {MAX_COLOURS} colours")]
    ColourCount,
    #[error("that palette or colour does not exist")]
    BadPreset,
    #[error("words need exactly two colours: the background and the letters")]
    WordsNeedTwo,
    #[error("size: {0}")]
    Size(String),
    #[error("{0}")]
    Gauge(#[from] GaugeError),
    #[error("{0}")]
    Picture(#[from] PictureError),
    #[error("{0}")]
    Text(#[from] TextError),
}

/// The techniques that suit a craft: tapestry and filet are crochet; intarsia and fair isle are knit.
pub fn techniques_for(craft: Craft) -> &'static [Technique] {
    match craft {
        Craft::Knit => &[Technique::Intarsia, Technique::FairIsle],
        Craft::Crochet => &[Technique::Tapestry, Technique::Filet],
    }
}

pub fn technique_label(t: Technique) -> &'static str {
    match t {
        Technique::Tapestry => "Tapestry crochet",
        Technique::Intarsia => "Intarsia",
        Technique::FairIsle => "Fair isle",
        Technique::Filet => "Filet crochet",
    }
}

impl Default for Setup {
    /// A beginner's defaults: knit, intarsia, flat, worsted gauge, about 8 inches wide, the Classic palette.
    fn default() -> Self {
        Self {
            craft: Craft::Knit,
            technique: Technique::Intarsia,
            construction: Construction::Flat,
            source: None,
            size: Size::Width(8.0),
            unit: Unit::Inches,
            known_gauge: None,
            yarn_weight: YarnWeight::Worsted,
            colours: Colours::Preset { preset: 0, picked: vec![0, 2] },
            colour_count: 4,
        }
    }
}

impl Setup {
    /// Switches craft and moves the technique to one that fits it.
    pub fn set_craft(&mut self, craft: Craft) {
        self.craft = craft;
        if !techniques_for(craft).contains(&self.technique) {
            self.technique = techniques_for(craft).first().copied().unwrap_or(Technique::Tapestry);
        }
    }

    /// The gauge in use: the known one, or the yarn weight's default.
    pub fn gauge(&self) -> Result<Gauge, GaugeError> {
        match self.known_gauge {
            Some(g) => {
                g.validate()?;
                Ok(g)
            }
            None => {
                let (s, r) = self.yarn_weight.default_gauge(self.craft);
                let measure = if self.unit == Unit::Cm { 10.0 } else { 4.0 };
                Gauge::new(s, r, measure, self.unit)
            }
        }
    }

    /// The palette the user picked from a preset (`None` for `FromPicture`).
    pub fn picked_colours(&self) -> Result<Option<Vec<PaletteColor>>, SetupError> {
        match &self.colours {
            Colours::FromPicture => Ok(None),
            Colours::Preset { preset, picked } => {
                let all = presets();
                let p = all.get(*preset).ok_or(SetupError::BadPreset)?;
                let mut out = Vec::with_capacity(picked.len());
                for &i in picked {
                    out.push(p.colours.get(i).cloned().ok_or(SetupError::BadPreset)?);
                }
                Ok(Some(out))
            }
        }
    }

    /// Builds the chart (steps 1 to 4) and applies the technique rules; the report is for the preview (step 5).
    pub fn build(&self) -> Result<(Chart, TechniqueReport), SetupError> {
        if !techniques_for(self.craft).contains(&self.technique) {
            let (t, c) = (technique_label(self.technique), if self.craft == Craft::Knit { "knitting" } else { "crochet" });
            return Err(SetupError::BadTechnique(t, c));
        }
        let gauge = self.gauge()?;
        let source = self.source.as_ref().ok_or(SetupError::NoSource)?;
        let picked = self.picked_colours()?;
        let mut chart = match source {
            Source::Words(words) => {
                let Some(colours) = picked else { return Err(SetupError::WordsNeedTwo) };
                let [background, lettering] = <[PaletteColor; 2]>::try_from(colours).map_err(|_| SetupError::WordsNeedTwo)?;
                let mut opts = TextOptions::new(words);
                let wanted = self.stitches(&gauge)?;
                opts.width = Some(wanted);
                let make =
                    |o: &TextOptions| chart_from_text(o, background.clone(), lettering.clone(), self.craft, self.technique, self.construction, gauge);
                match make(&opts) {
                    // Words wider than the chosen size: widen the chart to fit them rather than refuse.
                    Err(TextError::TooWide { needed, .. }) if needed <= MAX_WIDTH => {
                        opts.width = Some(if self.technique == Technique::Filet { filet_stitches(needed.div_ceil(3)) } else { needed });
                        make(&opts)?
                    }
                    other => other?,
                }
            }
            Source::Picture(pic) => {
                let width = self.stitches(&gauge)?;
                let height = rows_for_width(&gauge, pic.width, pic.height, width)?;
                let palette = match picked {
                    None => {
                        if !(MIN_COLOURS..=MAX_COLOURS).contains(&self.colour_count) {
                            return Err(SetupError::ColourCount);
                        }
                        PaletteSpec::Auto(self.colour_count)
                    }
                    Some(colours) => {
                        if !(MIN_COLOURS..=MAX_COLOURS).contains(&colours.len()) {
                            return Err(SetupError::ColourCount);
                        }
                        PaletteSpec::Fixed(colours)
                    }
                };
                chart_from_picture(pic, &PictureOptions { width, height, palette, cleanup: 1 }, self.craft, self.technique, self.construction, gauge)?
            }
        };
        let report = apply_technique(&mut chart, &TechniqueOptions::default());
        Ok((chart, report))
    }

    /// Whether the choices of `step` (1 to 4) are complete enough to go on; the message says what is missing.
    pub fn check_step(&self, step: usize) -> Result<(), SetupError> {
        match step {
            1 => {
                if techniques_for(self.craft).contains(&self.technique) {
                    Ok(())
                } else {
                    Err(SetupError::BadTechnique(technique_label(self.technique), if self.craft == Craft::Knit { "knitting" } else { "crochet" }))
                }
            }
            2 => match &self.source {
                None => Err(SetupError::NoSource),
                Some(Source::Words(w)) if w.trim().is_empty() => Err(SetupError::NoSource),
                Some(_) => Ok(()),
            },
            3 => {
                let gauge = self.gauge()?;
                self.stitches(&gauge).map(|_| ())
            }
            4 => {
                let is_words = matches!(self.source, Some(Source::Words(_)));
                match self.picked_colours()? {
                    None if is_words => Err(SetupError::WordsNeedTwo),
                    None if (MIN_COLOURS..=MAX_COLOURS).contains(&self.colour_count) => Ok(()),
                    None => Err(SetupError::ColourCount),
                    Some(c) if is_words && c.len() != 2 => Err(SetupError::WordsNeedTwo),
                    Some(c) if (MIN_COLOURS..=MAX_COLOURS).contains(&c.len()) => Ok(()),
                    Some(_) => Err(SetupError::ColourCount),
                }
            }
            _ => Ok(()),
        }
    }

    /// Stitches across, from the chosen size (filet: 3 x squares + 1).
    pub fn stitches(&self, gauge: &Gauge) -> Result<u32, SetupError> {
        let n = match self.size {
            Size::Stitches(n) => n,
            Size::Width(w) => gauge.grid_for(w, w, self.unit).map_err(|e| SetupError::Size(e.to_string()))?.0,
        };
        if !(1..=MAX_WIDTH).contains(&n) {
            return Err(SetupError::Size(format!("choose between 1 and {MAX_WIDTH} stitches across")));
        }
        Ok(if self.technique == Technique::Filet { filet_stitches((n.saturating_sub(1) / 3).max(1)) } else { n })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    fn picture() -> Picture {
        let rgb = (0..20 * 10).map(|i| if (i % 20) < 10 { [220, 30, 40] } else { [20, 40, 160] }).collect();
        Picture::new(20, 10, rgb).unwrap()
    }

    #[test]
    fn yarn_weight_defaults_get_coarser_and_worsted_is_the_classic_gauge() {
        for craft in [Craft::Knit, Craft::Crochet] {
            let sts: Vec<f64> = YarnWeight::ALL.iter().map(|w| w.default_gauge(craft).0).collect();
            assert!(sts.windows(2).all(|p| p[0] > p[1]), "{craft:?} {sts:?}");
        }
        assert_eq!(YarnWeight::Worsted.default_gauge(Craft::Knit), (20.0, 28.0));
        for w in YarnWeight::ALL {
            let (s, r) = w.default_gauge(Craft::Knit);
            assert!(r > s, "knit stitches are wider than tall: {w:?}");
        }
    }

    #[test]
    fn an_unknown_gauge_uses_the_yarn_weight_in_the_chosen_unit() {
        let mut s = Setup::default();
        s.yarn_weight = YarnWeight::DK;
        let g = s.gauge().unwrap();
        assert_eq!((g.stitches, g.rows, g.measure, g.unit), (22.0, 30.0, 4.0, Unit::Inches));
        s.unit = Unit::Cm;
        assert_eq!(s.gauge().unwrap().measure, 10.0);
        s.known_gauge = Some(Gauge::new(18.0, 26.0, 4.0, Unit::Inches).unwrap());
        assert_eq!(s.gauge().unwrap().stitches, 18.0);
    }

    #[test]
    fn presets_have_generic_names_only_and_valid_colours() {
        const BRAND_WORDS: [&str; 12] =
            ["lion", "caron", "red heart", "bernat", "paton", "cascade", "malabrigo", "drops", "patons", "wool-ease", "pantone", "dmc"];
        let all = presets();
        assert!(all.len() >= 4);
        for p in &all {
            assert_eq!(p.colours.len(), 8, "{}", p.name);
            let mut seen = std::collections::HashSet::new();
            for col in &p.colours {
                assert!(seen.insert(col.name.clone()), "duplicate {} in {}", col.name, p.name);
                let lower = col.name.to_lowercase();
                assert!(BRAND_WORDS.iter().all(|b| !lower.contains(b)), "{lower}");
                assert!(lower.chars().all(|ch| ch.is_ascii_lowercase() || ch == ' '), "{lower}");
            }
            let distinct: std::collections::HashSet<_> = p.colours.iter().map(|c| c.rgb).collect();
            assert_eq!(distinct.len(), 8);
        }
    }

    #[test]
    fn craft_and_technique_go_together() {
        let mut s = Setup::default();
        assert_eq!(techniques_for(Craft::Knit), &[Technique::Intarsia, Technique::FairIsle]);
        s.set_craft(Craft::Crochet);
        assert_eq!(s.technique, Technique::Tapestry);
        s.technique = Technique::FairIsle;
        s.source = Some(Source::Words("HI".into()));
        assert!(matches!(s.build(), Err(SetupError::BadTechnique(..))));
        s.set_craft(Craft::Knit);
        assert_eq!(s.technique, Technique::FairIsle);
    }

    #[test]
    fn five_steps_from_defaults_to_a_chart_from_words() {
        let mut s = Setup::default();
        assert!(matches!(s.build(), Err(SetupError::NoSource)));
        s.source = Some(Source::Words("KING CASHEW".into()));
        s.size = Size::Stitches(180);
        s.colours = Colours::Preset { preset: 0, picked: vec![3, 4] }; // navy background, gold letters
        let (chart, _) = s.build().unwrap();
        assert_eq!((chart.width, chart.palette[0].name.as_str(), chart.palette[1].name.as_str()), (180, "navy", "gold"));
        assert!(chart.cells.contains(&1));
        assert_eq!(chart.gauge.stitches, 20.0);
    }

    #[test]
    fn words_wider_than_the_chosen_size_widen_the_chart_instead_of_failing() {
        let mut s = Setup::default();
        s.source = Some(Source::Words("KING CASHEW".into()));
        s.size = Size::Width(8.0); // 40 stitches: far too narrow
        let (chart, _) = s.build().unwrap();
        assert!(chart.width > 100 && chart.width <= MAX_WIDTH, "{}", chart.width);
        s.source = Some(Source::Words("W".repeat(60)));
        assert!(s.build().is_err(), "past the largest chart size it is still refused");
    }

    #[test]
    fn a_picture_with_a_fixed_palette_uses_only_the_picked_colours() {
        let mut s = Setup::default();
        s.source = Some(Source::Picture(picture()));
        s.size = Size::Stitches(40);
        s.colours = Colours::Preset { preset: 1, picked: vec![2, 6, 0] };
        let (chart, _) = s.build().unwrap();
        assert_eq!(chart.palette.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["bright red", "sky blue", "white"]);
        assert_eq!(chart.width, 40);
        // 40 stitches at 20 sts / 28 rows per 4 in over a 2:1 picture: 20 x 10 -> height follows the gauge
        assert_eq!(chart.height, rows_for_width(&s.gauge().unwrap(), 20, 10, 40).unwrap());
    }

    #[test]
    fn auto_colours_give_the_requested_count_and_the_count_is_checked() {
        let mut s = Setup::default();
        s.source = Some(Source::Picture(picture()));
        s.size = Size::Stitches(30);
        s.colours = Colours::FromPicture;
        for n in [2usize, 3] {
            s.colour_count = n;
            assert_eq!(s.build().unwrap().0.palette.len(), n);
        }
        for bad in [0usize, 1, 9, 100] {
            s.colour_count = bad;
            assert_eq!(s.build().unwrap_err(), SetupError::ColourCount);
        }
        s.colours = Colours::Preset { preset: 0, picked: vec![0] };
        assert_eq!(s.build().unwrap_err(), SetupError::ColourCount);
        s.colours = Colours::Preset { preset: 0, picked: (0..8).chain([0]).collect() };
        assert!(s.build().is_err());
    }

    #[test]
    fn words_need_exactly_two_colours_and_the_picture_option_does_not_apply() {
        let mut s = Setup::default();
        s.source = Some(Source::Words("HI".into()));
        s.size = Size::Stitches(60);
        s.colours = Colours::FromPicture;
        assert_eq!(s.build().unwrap_err(), SetupError::WordsNeedTwo);
        s.colours = Colours::Preset { preset: 0, picked: vec![0, 1, 2] };
        assert_eq!(s.build().unwrap_err(), SetupError::WordsNeedTwo);
    }

    #[test]
    fn filet_charts_have_three_stitches_per_square_plus_one() {
        let mut s = Setup::default();
        s.set_craft(Craft::Crochet);
        s.technique = Technique::Filet;
        s.source = Some(Source::Picture(picture()));
        s.size = Size::Stitches(40);
        s.colours = Colours::Preset { preset: 0, picked: vec![0, 1] };
        let (chart, _) = s.build().unwrap();
        assert_eq!(chart.width % 3, 1);
    }

    #[test]
    fn each_step_says_what_is_missing_before_going_on() {
        let mut s = Setup::default();
        assert_eq!(s.check_step(1), Ok(()));
        assert_eq!(s.check_step(2), Err(SetupError::NoSource));
        s.source = Some(Source::Words("   ".into()));
        assert_eq!(s.check_step(2), Err(SetupError::NoSource));
        s.source = Some(Source::Words("HI".into()));
        assert_eq!(s.check_step(2), Ok(()));
        assert_eq!(s.check_step(3), Ok(()));
        s.size = Size::Stitches(0);
        assert!(s.check_step(3).is_err());
        s.size = Size::Stitches(60);
        assert_eq!(s.check_step(4), Ok(())); // classic palette, two colours
        s.colours = Colours::Preset { preset: 0, picked: vec![0] };
        assert_eq!(s.check_step(4), Err(SetupError::WordsNeedTwo));
        s.source = Some(Source::Picture(picture()));
        assert_eq!(s.check_step(4), Err(SetupError::ColourCount));
        s.colours = Colours::FromPicture;
        assert_eq!(s.check_step(4), Ok(()));
        s.colour_count = 12;
        assert_eq!(s.check_step(4), Err(SetupError::ColourCount));
        s.technique = Technique::Filet;
        assert!(matches!(s.check_step(1), Err(SetupError::BadTechnique(..))));
    }

    #[test]
    fn hostile_choices_are_errors_not_panics() {
        let mut s = Setup::default();
        s.source = Some(Source::Words("HI".into()));
        for size in [Size::Width(f64::NAN), Size::Width(-3.0), Size::Width(1e12), Size::Stitches(0), Size::Stitches(MAX_WIDTH + 1)] {
            s.size = size;
            assert!(s.build().is_err(), "{size:?}");
        }
        s.size = Size::Stitches(60);
        s.colours = Colours::Preset { preset: 99, picked: vec![0, 1] };
        assert_eq!(s.build().unwrap_err(), SetupError::BadPreset);
        s.colours = Colours::Preset { preset: 0, picked: vec![0, 99] };
        assert_eq!(s.build().unwrap_err(), SetupError::BadPreset);
        s.colours = Colours::Preset { preset: 0, picked: vec![0, 1] };
        s.known_gauge = Some(Gauge { stitches: f64::NAN, rows: 1.0, measure: 1.0, unit: Unit::Inches });
        assert!(s.build().is_err());
    }
}
