//! "Never break wasm": the browser build compiles these crates for `wasm32-unknown-unknown`.
//! Every workspace package is either in [`WASM`] (checked) or [`NATIVE_ONLY`]; new packages must be classified.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

const TARGET: &str = "wasm32-unknown-unknown";

pub const WASM: &[&str] = &["yarnfoundry-core", "yarnfoundry-render", "yarnfoundry-ui-egui", "yarnfoundry-web"];
pub const NATIVE_ONLY: &[&str] = &["yarnfoundry", "yarnfoundry-cli", "xtask"];

pub fn run(root: &Path) -> Result<(), String> {
    let names = workspace_packages(root)?;
    let unclassified = unclassified(&names);
    if !unclassified.is_empty() {
        return Err(format!("not classified for the wasm check (add to xtask/src/wasm.rs): {}", unclassified.join(", ")));
    }
    let installed =
        Command::new("rustup").args(["target", "list", "--installed"]).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    if installed.is_some_and(|t| !t.lines().any(|l| l.trim() == TARGET)) {
        return Err(format!("the {TARGET} target is not installed: rustup target add {TARGET}"));
    }
    let mut failed = Vec::new();
    for package in WASM {
        eprintln!("==> wasm: {package}");
        let status = Command::new("cargo")
            .args(["check", "--target", TARGET, "-p", package])
            .current_dir(root)
            .status()
            .map_err(|e| format!("could not run cargo: {e}"))?;
        if !status.success() {
            failed.push(*package);
        }
    }
    if failed.is_empty() {
        println!("wasm: ok ({} crates check for {TARGET})", WASM.len());
        Ok(())
    } else {
        Err(format!("these crates no longer build for {TARGET}: {}", failed.join(", ")))
    }
}

pub fn unclassified(names: &[String]) -> Vec<String> {
    names.iter().filter(|n| !WASM.contains(&n.as_str()) && !NATIVE_ONLY.contains(&n.as_str())).cloned().collect()
}

fn workspace_packages(root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    Ok(meta["packages"].as_array().map(|ps| ps.iter().filter_map(|p| p["name"].as_str().map(str::to_string)).collect()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_package_must_be_classified() {
        let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(unclassified(&names(&["yarnfoundry-core", "xtask"])).is_empty());
        assert_eq!(unclassified(&names(&["yarnfoundry-mystery"])), ["yarnfoundry-mystery"]);
    }

    #[test]
    fn every_real_workspace_package_is_classified() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert_eq!(unclassified(&workspace_packages(root).unwrap()), Vec::<String>::new());
    }
}
