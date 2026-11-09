//! YarnFoundry core: the chart model, gauge math and file format live here (no UI, no I/O).

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
