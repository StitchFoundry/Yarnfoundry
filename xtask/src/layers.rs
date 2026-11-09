//! Layering: `yarnfoundry-core` depends on no other workspace crate; nothing below the UI depends on egui/eframe.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// (package, workspace crates it may depend on)
const ALLOWED: &[(&str, &[&str])] = &[
    ("yarnfoundry-core", &[]),
    ("yarnfoundry-render", &["yarnfoundry-core"]),
    ("yarnfoundry-ui-egui", &["yarnfoundry-core", "yarnfoundry-render"]),
    ("yarnfoundry", &["yarnfoundry-core", "yarnfoundry-render", "yarnfoundry-ui-egui"]),
    ("yarnfoundry-cli", &["yarnfoundry-core", "yarnfoundry-render"]),
    ("xtask", &[]),
];
const UI_ONLY: &[&str] = &["egui", "eframe", "winit", "rfd"];

pub fn run(root: &Path) -> Result<(), String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut problems = Vec::new();
    for p in meta["packages"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let name = p["name"].as_str().unwrap_or_default();
        let Some((_, allowed)) = ALLOWED.iter().find(|(n, _)| *n == name) else {
            problems.push(format!("{name}: not listed in xtask/src/layers.rs"));
            continue;
        };
        for d in p["dependencies"].as_array().map(Vec::as_slice).unwrap_or_default() {
            let dep = d["name"].as_str().unwrap_or_default();
            let ui_ok = matches!(name, "yarnfoundry-ui-egui" | "yarnfoundry");
            if dep.starts_with("yarnfoundry") && !allowed.contains(&dep) {
                problems.push(format!("{name} must not depend on {dep}"));
            }
            if UI_ONLY.contains(&dep) && !ui_ok {
                problems.push(format!("{name} must not depend on {dep}"));
            }
        }
    }
    if problems.is_empty() {
        println!("layers: ok");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}
