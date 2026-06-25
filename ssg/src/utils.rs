use std::{fs, path::Path};

use walkdir::WalkDir;

pub fn copy_dir(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> anyhow::Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel_path = entry.path().strip_prefix(src)?;
        let target = dst.join(rel_path);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

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

fn human_file_size(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let size = fs::metadata(path)?.len();
    Ok(human_readable(size))
}

fn human_dir_size(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let mut size = 0;
    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            size += entry.metadata()?.len();
        }
    }
    Ok(human_readable(size))
}

pub fn human_path_size(path: impl AsRef<Path>) -> anyhow::Result<String> {
    if path.as_ref().is_file() {
        human_file_size(&path)
    } else {
        human_dir_size(path)
    }
}
