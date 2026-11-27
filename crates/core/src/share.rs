//! The "Share it on MakerFoundry?" prompt after an export: when to ask, and what the draft holds (no UI, no I/O).
//!
//! Publishing, price and Seller Terms happen on MakerFoundry, never here. This module only decides whether to
//! ask, and checks the pre-filled draft before the front end hands it to the publish path.

use serde::{Deserialize, Serialize};

/// Most tags a draft carries.
pub const MAX_TAGS: usize = 12;
/// Longest title, tag or remix id we pass on (project names are untrusted).
pub const MAX_TEXT: usize = 120;

/// The MakerFoundry category for every YarnFoundry design.
pub const CATEGORY: &str = "yarn";

/// What the user picked in the prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShareChoice {
    ShareFree,
    Sell,
    NotNow,
}

/// The licence picked for a shared design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum License {
    CcBy,
    CcBySa,
    CcByNc,
    AllRightsReserved,
}

impl License {
    /// Non-commercial licences cannot be sold.
    pub fn allows_selling(self) -> bool {
        !matches!(self, License::CcByNc)
    }
}

/// App-wide setting: Settings > MakerFoundry > "Offer to share after export" (default on).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharePrefs {
    pub offer_after_export: bool,
}

impl Default for SharePrefs {
    fn default() -> Self {
        Self { offer_after_export: true }
    }
}

/// Per-project memory, saved with the project.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareState {
    #[serde(default)]
    pub asked: bool,
    #[serde(default)]
    pub dont_ask_again: bool,
}

impl ShareState {
    /// True when the non-blocking prompt should be shown after a successful print or export.
    /// A failed export never prompts. Showing it counts as asking (at most once per project).
    pub fn should_prompt(&self, prefs: &SharePrefs, export_succeeded: bool) -> bool {
        export_succeeded && prefs.offer_after_export && !self.asked && !self.dont_ask_again
    }

    /// Records that the prompt was shown and answered; `dont_ask_again` is the checkbox.
    pub fn record(&mut self, dont_ask_again: bool) {
        self.asked = true;
        self.dont_ask_again |= dont_ask_again;
    }
}

/// Why a draft was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShareError {
    #[error("the title is empty")]
    EmptyTitle,
    #[error("confirm you made this or own the rights")]
    RightsNotConfirmed,
    #[error("non-commercial licences cannot be sold")]
    NotSellable,
    #[error("nothing to share: choose Share free or Sell it")]
    NoChoice,
}

/// The pre-filled MakerFoundry draft. The file and preview image travel alongside it in the publish path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareDraft {
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub app: String,
    pub app_version: String,
    pub license: License,
    pub owns_rights: bool,
    pub sell: bool,
    /// The MakerFoundry design this project was opened from, for remix credit.
    pub remix_of: Option<String>,
}

impl ShareDraft {
    /// Builds a checked draft from the prompt. Text is trimmed and capped, blank and duplicate tags dropped.
    pub fn new(
        choice: ShareChoice,
        title: &str,
        tags: &[&str],
        license: License,
        owns_rights: bool,
        remix_of: Option<&str>,
    ) -> Result<Self, ShareError> {
        let sell = match choice {
            ShareChoice::NotNow => return Err(ShareError::NoChoice),
            ShareChoice::Sell => true,
            ShareChoice::ShareFree => false,
        };
        let title = clip(title);
        if title.is_empty() {
            return Err(ShareError::EmptyTitle);
        }
        if !owns_rights {
            return Err(ShareError::RightsNotConfirmed);
        }
        if sell && !license.allows_selling() {
            return Err(ShareError::NotSellable);
        }
        let mut kept: Vec<String> = Vec::new();
        for tag in tags {
            let tag = clip(tag).to_lowercase();
            if !tag.is_empty() && !kept.contains(&tag) && kept.len() < MAX_TAGS {
                kept.push(tag);
            }
        }
        Ok(Self {
            title,
            category: CATEGORY.to_string(),
            tags: kept,
            app: crate::APP_NAME.to_string(),
            app_version: crate::VERSION.to_string(),
            license,
            owns_rights,
            sell,
            remix_of: remix_of.map(clip).filter(|s| !s.is_empty()),
        })
    }
}

fn clip(text: &str) -> String {
    text.trim().chars().take(MAX_TEXT).collect()
}

/// Analytics event names, sent only with consent (desktop default is off).
pub const EVENT_SHOWN: &str = "share_prompt_shown";
pub const EVENT_CHOICE: &str = "share_prompt_choice";

/// The event to record for a prompt step, or `None` without analytics consent.
pub fn analytics_event(consent: bool, shown: bool) -> Option<&'static str> {
    consent.then_some(if shown { EVENT_SHOWN } else { EVENT_CHOICE })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(choice: ShareChoice, license: License, rights: bool) -> Result<ShareDraft, ShareError> {
        ShareDraft::new(choice, " Sheep ", &["Wool", "wool", " ", "sheep"], license, rights, None)
    }

    #[test]
    fn prompts_once_per_project() {
        let prefs = SharePrefs::default();
        let mut state = ShareState::default();
        assert!(state.should_prompt(&prefs, true));
        state.record(false);
        assert!(!state.should_prompt(&prefs, true));
    }

    #[test]
    fn failed_export_does_not_prompt_or_use_up_the_ask() {
        let state = ShareState::default();
        assert!(!state.should_prompt(&SharePrefs::default(), false));
        assert!(state.should_prompt(&SharePrefs::default(), true));
    }

    #[test]
    fn dont_ask_again_sticks() {
        let mut state = ShareState::default();
        state.record(true);
        assert!(state.dont_ask_again);
        state.record(false);
        assert!(state.dont_ask_again);
    }

    #[test]
    fn setting_off_never_prompts() {
        let prefs = SharePrefs { offer_after_export: false };
        assert!(!ShareState::default().should_prompt(&prefs, true));
    }

    #[test]
    fn old_project_files_without_state_default_to_unasked() {
        let state: ShareState = serde_json::from_str("{}").unwrap();
        assert_eq!(state, ShareState::default());
    }

    #[test]
    fn not_now_is_a_cancel_not_a_draft() {
        assert_eq!(draft(ShareChoice::NotNow, License::CcBy, true), Err(ShareError::NoChoice));
    }

    #[test]
    fn draft_is_prefilled_and_tidied() {
        let d = draft(ShareChoice::ShareFree, License::CcBySa, true).unwrap();
        assert_eq!((d.title.as_str(), d.category.as_str()), ("Sheep", "yarn"));
        assert_eq!(d.tags, ["wool", "sheep"]);
        assert_eq!(d.app, "YarnFoundry");
        assert_eq!(d.app_version, crate::VERSION);
        assert!(!d.sell);
    }

    #[test]
    fn rights_must_be_confirmed() {
        assert_eq!(draft(ShareChoice::ShareFree, License::CcBy, false), Err(ShareError::RightsNotConfirmed));
    }

    #[test]
    fn non_commercial_cannot_be_sold_but_can_be_shared() {
        assert_eq!(draft(ShareChoice::Sell, License::CcByNc, true), Err(ShareError::NotSellable));
        assert!(draft(ShareChoice::ShareFree, License::CcByNc, true).is_ok());
        assert!(draft(ShareChoice::Sell, License::AllRightsReserved, true).is_ok());
    }

    #[test]
    fn hostile_text_is_capped_or_refused() {
        assert_eq!(ShareDraft::new(ShareChoice::ShareFree, "  ", &[], License::CcBy, true, None), Err(ShareError::EmptyTitle));
        let long = "x".repeat(10_000);
        let tags: Vec<&str> = (0..100).map(|_| long.as_str()).collect();
        let d = ShareDraft::new(ShareChoice::ShareFree, &long, &tags, License::CcBy, true, Some(&long)).unwrap();
        assert_eq!(d.title.chars().count(), MAX_TEXT);
        assert_eq!(d.tags.len(), 1);
        assert_eq!(d.remix_of.map(|s| s.chars().count()), Some(MAX_TEXT));
    }

    #[test]
    fn analytics_needs_consent() {
        assert_eq!(analytics_event(false, true), None);
        assert_eq!(analytics_event(true, true), Some("share_prompt_shown"));
        assert_eq!(analytics_event(true, false), Some("share_prompt_choice"));
    }
}
