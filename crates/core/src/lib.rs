//! YarnFoundry core: the chart model, gauge math and file format (no UI, no I/O).

pub mod chart;
pub mod color;
pub mod file;
pub mod gauge;
pub mod instructions;
pub mod picture;
pub mod technique;
pub mod text;

pub use chart::{Chart, ChartError, Construction, Craft, PaletteColor, Technique};
pub use file::{FileError, from_json, to_json};
pub use gauge::{Gauge, GaugeError, Unit};
pub use instructions::{ColourTotal, Instructions, RowInstruction, Side, colour_label, instructions};
pub use picture::{PaletteSpec, Picture, PictureError, PictureOptions, chart_from_picture, rows_for_width};
pub use technique::{Float, TechniqueOptions, TechniqueReport, apply as apply_technique, filet_stitches};
pub use text::{TextError, TextOptions, chart_from_text, text_to_mask};

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
