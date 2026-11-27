//! Funnel events: the shared event names and the only properties an event may carry (no UI, no I/O).
//!
//! An event is built from typed fields, so file names, design content, images and typed text cannot be added.
//! Sending is the front end's job and is opt-in on desktop.

use serde::Serialize;
use serde_json::{Map, Value};

/// The shared event names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventName {
    LandingViewed,
    ProjectStarted,
    FileImported,
    ImportFailed,
    DesignEdited,
    PreviewViewed,
    ExportCreated,
    ExportFailed,
    StepViewed,
    StepCompleted,
    SessionEnded,
    ErrorShown,
    AiAddonViewed,
    AiAddonStarted,
}

impl EventName {
    /// Every event name.
    pub const ALL: [EventName; 14] = [
        EventName::LandingViewed,
        EventName::ProjectStarted,
        EventName::FileImported,
        EventName::ImportFailed,
        EventName::DesignEdited,
        EventName::PreviewViewed,
        EventName::ExportCreated,
        EventName::ExportFailed,
        EventName::StepViewed,
        EventName::StepCompleted,
        EventName::SessionEnded,
        EventName::ErrorShown,
        EventName::AiAddonViewed,
        EventName::AiAddonStarted,
    ];

    /// The wire name (snake case).
    pub fn as_str(self) -> &'static str {
        match self {
            EventName::LandingViewed => "landing_viewed",
            EventName::ProjectStarted => "project_started",
            EventName::FileImported => "file_imported",
            EventName::ImportFailed => "import_failed",
            EventName::DesignEdited => "design_edited",
            EventName::PreviewViewed => "preview_viewed",
            EventName::ExportCreated => "export_created",
            EventName::ExportFailed => "export_failed",
            EventName::StepViewed => "step_viewed",
            EventName::StepCompleted => "step_completed",
            EventName::SessionEnded => "session_ended",
            EventName::ErrorShown => "error_shown",
            EventName::AiAddonViewed => "ai_addon_viewed",
            EventName::AiAddonStarted => "ai_addon_started",
        }
    }
}

/// The property keys an event may carry. Anything else is never sent.
pub const ALLOWED_PROPERTIES: [&str; 12] = [
    "app",
    "app_version",
    "platform",
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "step",
    "format",
    "error_code",
    "duration_ms",
    "ok",
    "utm_content",
];

/// Longest text value a property may hold (characters); longer values are cut.
pub const MAX_VALUE_CHARS: usize = 64;

/// The allowed properties, all optional. Text values are short codes, never user content.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Props {
    pub app: Option<String>,
    pub app_version: Option<String>,
    pub platform: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub utm_content: Option<String>,
    pub step: Option<String>,
    pub format: Option<String>,
    pub error_code: Option<String>,
    pub duration_ms: Option<u64>,
    pub ok: Option<bool>,
}

/// A code-like value: ASCII letters, digits and `_ - . /` only, cut to [`MAX_VALUE_CHARS`]; `None` if nothing is left.
fn code(value: &str) -> Option<String> {
    let s: String = value.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/')).take(MAX_VALUE_CHARS).collect();
    (!s.is_empty()).then_some(s)
}

impl Props {
    /// Properties with the build identity filled in.
    pub fn base(app: &str, app_version: &str, platform: &str) -> Props {
        Props { app: code(app), app_version: code(app_version), platform: code(platform), ..Props::default() }
    }

    /// Sets the guided-setup step id.
    pub fn with_step(mut self, step: &str) -> Props {
        self.step = code(step);
        self
    }

    /// Sets the file format (for example `pdf`, `png`, `stl`).
    pub fn with_format(mut self, format: &str) -> Props {
        self.format = code(format);
        self
    }

    /// Sets a short error code (never the error message).
    pub fn with_error_code(mut self, error_code: &str) -> Props {
        self.error_code = code(error_code);
        self
    }

    /// Sets the time spent, in milliseconds.
    pub fn with_duration_ms(mut self, ms: u64) -> Props {
        self.duration_ms = Some(ms);
        self
    }

    /// Sets whether the action succeeded.
    pub fn with_ok(mut self, ok: bool) -> Props {
        self.ok = Some(ok);
        self
    }

    /// Sets a UTM field by key (`utm_source`, `utm_medium`, `utm_campaign`, `utm_content`); other keys are ignored.
    pub fn with_utm(mut self, key: &str, value: &str) -> Props {
        let v = code(value);
        match key {
            "utm_source" => self.utm_source = v,
            "utm_medium" => self.utm_medium = v,
            "utm_campaign" => self.utm_campaign = v,
            "utm_content" => self.utm_content = v,
            _ => {}
        }
        self
    }

    /// The set properties as a JSON object holding only [`ALLOWED_PROPERTIES`] keys.
    pub fn to_map(&self) -> Map<String, Value> {
        let mut m = Map::new();
        let texts = [
            ("app", &self.app),
            ("app_version", &self.app_version),
            ("platform", &self.platform),
            ("utm_source", &self.utm_source),
            ("utm_medium", &self.utm_medium),
            ("utm_campaign", &self.utm_campaign),
            ("utm_content", &self.utm_content),
            ("step", &self.step),
            ("format", &self.format),
            ("error_code", &self.error_code),
        ];
        for (key, value) in texts {
            if let Some(v) = value {
                m.insert(key.to_string(), Value::String(v.clone()));
            }
        }
        if let Some(ms) = self.duration_ms {
            m.insert("duration_ms".to_string(), Value::from(ms));
        }
        if let Some(ok) = self.ok {
            m.insert("ok".to_string(), Value::Bool(ok));
        }
        m
    }
}

/// The JSON body for one event: `{"event": name, "properties": {...}}`.
pub fn event_json(name: EventName, props: &Props) -> String {
    let mut body = Map::new();
    body.insert("event".to_string(), Value::String(name.as_str().to_string()));
    body.insert("properties".to_string(), Value::Object(props.to_map()));
    Value::Object(body).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Props {
        Props::base("yarnfoundry", "0.2.0", "web")
            .with_utm("utm_source", "facebook")
            .with_utm("utm_medium", "social")
            .with_utm("utm_campaign", "launch")
            .with_utm("utm_content", "post1")
            .with_step("size")
            .with_format("pdf")
            .with_error_code("too_large")
            .with_duration_ms(1200)
            .with_ok(false)
    }

    #[test]
    fn every_emitted_key_is_on_the_allow_list() {
        let keys: Vec<String> = full().to_map().keys().cloned().collect();
        assert_eq!(keys.len(), ALLOWED_PROPERTIES.len(), "every allowed property is exercised");
        for k in keys {
            assert!(ALLOWED_PROPERTIES.contains(&k.as_str()), "{k} is not allowed");
        }
    }

    #[test]
    fn the_wire_names_are_snake_case_and_distinct() {
        let mut seen = Vec::new();
        for n in EventName::ALL {
            let s = n.as_str();
            assert!(s.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{s}");
            assert!(!seen.contains(&s), "{s} repeated");
            seen.push(s);
        }
        assert_eq!(EventName::StepCompleted.as_str(), "step_completed");
    }

    #[test]
    fn typed_text_and_file_names_are_reduced_to_codes() {
        let p = Props::base("yarnfoundry", "0.2.0", "web").with_error_code("my secret note!\n");
        assert_eq!(p.error_code.as_deref(), Some("mysecretnote"));
        let long = "a".repeat(500);
        assert_eq!(Props::default().with_step(&long).step.map(|s| s.len()), Some(MAX_VALUE_CHARS));
        assert_eq!(Props::default().with_step("   !!! ").step, None);
    }

    #[test]
    fn unknown_utm_keys_are_ignored() {
        let p = Props::default().with_utm("email", "a@b.c");
        assert!(p.to_map().is_empty());
    }

    #[test]
    fn event_json_has_the_name_and_only_allowed_properties() {
        let json = event_json(EventName::ExportFailed, &full());
        let v: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
        assert_eq!(v["event"], "export_failed");
        let props = v["properties"].as_object().map(|o| o.len());
        assert_eq!(props, Some(ALLOWED_PROPERTIES.len()));
        assert_eq!(v["properties"]["duration_ms"], 1200);
        assert_eq!(v["properties"]["ok"], false);
    }
}
