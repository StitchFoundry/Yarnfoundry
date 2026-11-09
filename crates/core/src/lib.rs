//! YarnFoundry core: the chart model, gauge math and file format (no UI, no I/O).

pub mod chart;
pub mod color;
pub mod file;
pub mod gauge;
pub mod instructions;
pub mod mesh;
pub mod models;
pub mod parts;
pub mod pattern;
pub mod picture;
pub mod rounds;
pub mod setup;
pub mod slice;
pub mod technique;
pub mod text;

pub use chart::{Chart, ChartError, Construction, Craft, PaletteColor, Technique};
pub use file::{FileError, from_json, to_json};
pub use gauge::{Gauge, GaugeError, Unit};
pub use instructions::{ColourTotal, Instructions, RowInstruction, Side, colour_label, instructions};
pub use mesh::{Axis, Mesh, MeshError, MeshReport, import as import_model};
pub use parts::{Attach, Figure, ManualSplits, Part, split_figure};
pub use pattern::{EyeMark, PartPattern, PatternRound, Terms, part_pattern, written_rounds};
pub use picture::{PaletteSpec, Picture, PictureError, PictureOptions, chart_from_picture, rows_for_width};
pub use rounds::{Round, RoundsError, Stitch, rounds_for_mesh};
pub use setup::{Colours, Setup, SetupError, Size, Source, YarnWeight, presets, techniques_for};
pub use technique::{Float, TechniqueOptions, TechniqueReport, apply as apply_technique, filet_stitches};
pub use text::{DEFAULT_MIN_ROWS, TextError, TextOptions, TextWeight, chart_from_text, text_to_mask};

/// The product name shown in window titles and the CLI.
pub const APP_NAME: &str = "YarnFoundry";

/// The version of this build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A one-line banner for the CLI and about boxes.
pub fn banner() -> String {
    format!("{APP_NAME} {VERSION}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_names_the_product_and_version() {
        assert!(banner().starts_with("YarnFoundry "));
        assert!(banner().ends_with(VERSION));
    }
}
