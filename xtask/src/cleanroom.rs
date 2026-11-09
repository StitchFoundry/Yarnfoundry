//! Clean-room guard: fails on files or text that point at the no-read list (competitor code).
//! The list of forbidden markers lives here; docs may name competitors, source and fixtures may not.

use std::path::Path;

/// Text that must never appear in code or fixtures (case-insensitive). Docs (`*.md`) are exempt.
const FORBIDDEN: &[&str] = &["copied from crochetparade", "ported from crochetparade", "crochetparade source"];

pub fn run(root: &Path) -> Result<(), String> {
    let mut hits = Vec::new();
    scan(root, &mut hits)?;
    if hits.is_empty() {
        println!("cleanroom: ok");
        Ok(())
    } else {
        Err(format!("cleanroom violations: {}", hits.join(", ")))
    }
}

fn scan(dir: &Path, hits: &mut Vec<String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if name == "target" || name == ".git" {
            continue;
        }
        if path.is_dir() {
            scan(&path, hits)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") && name != "cleanroom.rs" {
            let text = std::fs::read_to_string(&path).unwrap_or_default().to_lowercase();
            if FORBIDDEN.iter().any(|f| text.contains(f)) {
                hits.push(path.display().to_string());
            }
        }
    }
    Ok(())
}
