//! The `.yarnfoundry` project file: versioned JSON. Files are untrusted: every failure is an error, never a panic.

use serde::{Deserialize, Serialize};

use crate::chart::{Chart, ChartError};

/// The schema version this build writes and reads.
pub const SCHEMA_VERSION: u32 = 1;

/// Largest project file we will parse (bytes).
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// Why a project file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("the file is larger than {MAX_FILE_BYTES} bytes")]
    TooLarge,
    #[error("not a YarnFoundry project: {0}")]
    Malformed(String),
    #[error("project file version {found} is newer than this app understands ({SCHEMA_VERSION})")]
    UnsupportedVersion { found: u32 },
    #[error("invalid chart: {0}")]
    Invalid(#[from] ChartError),
}

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    schema_version: u32,
    chart: Chart,
}

const FORMAT: &str = "yarnfoundry";

/// Writes a chart as project-file JSON.
pub fn to_json(chart: &Chart) -> Result<String, FileError> {
    chart.validate()?;
    let env = Envelope { format: FORMAT.to_string(), schema_version: SCHEMA_VERSION, chart: chart.clone() };
    serde_json::to_string_pretty(&env).map_err(|e| FileError::Malformed(e.to_string()))
}

/// Reads project-file JSON into a validated chart.
pub fn from_json(text: &str) -> Result<Chart, FileError> {
    if text.len() > MAX_FILE_BYTES {
        return Err(FileError::TooLarge);
    }
    // Look at the version first so a future file reports "newer", not a confusing field error.
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| FileError::Malformed(e.to_string()))?;
    let version = value.get("schema_version").and_then(serde_json::Value::as_u64);
    match version {
        None => return Err(FileError::Malformed("missing schema_version".into())),
        Some(v) if v > u64::from(SCHEMA_VERSION) => return Err(FileError::UnsupportedVersion { found: u32::try_from(v).unwrap_or(u32::MAX) }),
        Some(0) => return Err(FileError::Malformed("schema_version 0".into())),
        Some(_) => {}
    }
    let env: Envelope = serde_json::from_value(value).map_err(|e| FileError::Malformed(e.to_string()))?;
    if env.format != FORMAT {
        return Err(FileError::Malformed(format!("format is {:?}", env.format)));
    }
    env.chart.validate()?;
    Ok(env.chart)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::test_support::sample;
    use proptest::prelude::*;

    #[test]
    fn save_then_reopen_is_identical() {
        let chart = sample();
        let text = to_json(&chart).unwrap();
        assert_eq!(from_json(&text).unwrap(), chart);
        // and the text itself is stable
        assert_eq!(to_json(&from_json(&text).unwrap()).unwrap(), text);
    }

    #[test]
    fn the_file_carries_a_schema_version() {
        assert!(to_json(&sample()).unwrap().contains("\"schema_version\": 1"));
    }

    #[test]
    fn malformed_files_are_errors() {
        for bad in ["", "{", "null", "[]", "{}", "{\"schema_version\":1}", "{\"format\":\"x\",\"schema_version\":1,\"chart\":{}}", "\u{0}\u{1}"] {
            assert!(from_json(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn newer_versions_and_wrong_formats_are_refused() {
        let good = to_json(&sample()).unwrap();
        let newer = good.replace("\"schema_version\": 1", "\"schema_version\": 99");
        assert!(matches!(from_json(&newer), Err(FileError::UnsupportedVersion { found: 99 })));
        let huge = good.replace("\"schema_version\": 1", "\"schema_version\": 99999999999");
        assert!(matches!(from_json(&huge), Err(FileError::UnsupportedVersion { .. })));
        let other = good.replace("\"format\": \"yarnfoundry\"", "\"format\": \"other\"");
        assert!(matches!(from_json(&other), Err(FileError::Malformed(_))));
    }

    #[test]
    fn inconsistent_charts_are_refused() {
        let good = to_json(&sample()).unwrap();
        let bad_width = good.replace("\"width\": 4", "\"width\": 5");
        assert!(matches!(from_json(&bad_width), Err(FileError::Invalid(ChartError::CellCount { .. }))));
        let bad_gauge = good.replace("\"stitches\": 20.0", "\"stitches\": 0.0");
        assert!(matches!(from_json(&bad_gauge), Err(FileError::Invalid(ChartError::Gauge(_)))));
    }

    #[test]
    fn oversized_input_is_refused_without_parsing() {
        let big = " ".repeat(MAX_FILE_BYTES + 1);
        assert!(matches!(from_json(&big), Err(FileError::TooLarge)));
    }

    proptest! {
        #[test]
        fn arbitrary_text_never_panics(s in ".{0,400}") {
            let _ = from_json(&s);
        }

        #[test]
        fn mutated_valid_files_never_panic(pos in 0usize..2000, ch in any::<char>(), cut in 0usize..2000) {
            let good = to_json(&sample()).unwrap();
            let mut chars: Vec<char> = good.chars().collect();
            let p = pos % chars.len();
            chars[p] = ch;
            let mutated: String = chars.iter().collect();
            let _ = from_json(&mutated);
            let truncated: String = chars.iter().take(cut % chars.len()).collect();
            let _ = from_json(&truncated);
        }
    }
}
