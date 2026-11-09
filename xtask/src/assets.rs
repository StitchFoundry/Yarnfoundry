//! Every file under `assets/` needs a row in `ASSETS.md` (name it by file name).

use std::path::Path;

pub fn run(root: &Path) -> Result<(), String> {
    let dir = root.join("assets");
    if !dir.exists() {
        println!("assets: ok (no assets folder)");
        return Ok(());
    }
    let table = std::fs::read_to_string(root.join("ASSETS.md")).map_err(|e| format!("ASSETS.md: {e}"))?;
    let mut missing = Vec::new();
    walk(&dir, &mut |p| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if !table.contains(name) {
            missing.push(p.display().to_string());
        }
    })?;
    if missing.is_empty() {
        println!("assets: ok");
        Ok(())
    } else {
        Err(format!("assets without a row in ASSETS.md: {}", missing.join(", ")))
    }
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            walk(&path, f)?;
        } else {
            f(&path);
        }
    }
    Ok(())
}
