use std::{fs, path::Path};

use walkdir::WalkDir;

fn human_readable(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.2} {}", size, UNITS[i])
}

pub fn human_path_size(path: impl AsRef<Path>) -> anyhow::Result<String> {
    if path.as_ref().is_file() {
        Ok(human_readable(fs::metadata(&path)?.len()))
    } else {
        let mut size = 0;
        for entry in WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                size += entry.metadata()?.len();
            }
        }
        Ok(human_readable(size))
    }
}
