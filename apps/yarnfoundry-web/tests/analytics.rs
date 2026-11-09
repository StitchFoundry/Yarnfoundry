#![allow(clippy::expect_used)]
//! The analytics hook: with the placeholder token the page adds no beacon script tag and makes no request.
//! Runs the page's inline script in Node with a fake `document` (skipped with a note if Node is missing).

use std::process::Command;

const INDEX: &str = include_str!("../index.html");

/// The page's analytics snippet, with the token line replaced by `token`.
fn snippet(token: &str) -> String {
    let start = INDEX.find("(function () {").expect("analytics snippet present");
    let end = INDEX[start..].find("})();").expect("snippet end") + start + "})();".len();
    INDEX[start..end].replacen("var token = \"YF_CF_TOKEN\";", &format!("var token = \"{token}\";"), 1)
}

fn added_tags(token: &str) -> Option<String> {
    let js = format!(
        "var added=[];var document={{head:{{appendChild:function(e){{added.push(e)}}}},createElement:function(){{return {{attrs:{{}},setAttribute:function(k,v){{this.attrs[k]=v}}}}}}}};{}\nconsole.log(JSON.stringify(added.map(function(e){{return [e.src,e.attrs['data-cf-beacon']]}})));",
        snippet(token)
    );
    let out = Command::new("node").arg("-e").arg(js).output().ok()?;
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[test]
fn the_token_lives_in_one_place_and_the_page_has_no_literal_beacon_tag() {
    assert_eq!(INDEX.matches("var token = \"YF_CF_TOKEN\";").count(), 1);
    assert!(!INDEX.contains("<script defer"), "the beacon tag is only ever added by the snippet");
    assert!(INDEX.contains("https://static.cloudflareinsights.com/beacon.min.js"));
}

#[test]
fn the_placeholder_token_adds_no_script_tag() {
    if let Some(added) = added_tags("YF_CF_TOKEN") {
        assert_eq!(added, "[]");
    } else {
        eprintln!("node not found: skipped the placeholder run");
    }
}

#[test]
fn a_real_token_adds_the_beacon_tag() {
    if let Some(added) = added_tags("abc123") {
        assert!(added.contains("https://static.cloudflareinsights.com/beacon.min.js"), "{added}");
        assert!(added.contains("abc123"), "{added}");
    }
}
