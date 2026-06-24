use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub fn unix_zip(paths: &[PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .system(zip::System::Unix)
            .unix_permissions(0o755);

        for path in paths {
            if path.is_dir() {
                let parent = path.parent().unwrap();
                for e in WalkDir::new(path).sort_by_file_name() {
                    let e = e.unwrap();
                    let p = e.path();
                    let arc = p
                        .strip_prefix(parent)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    if p.is_dir() {
                        zw.add_directory(&arc, opts).unwrap();
                    } else {
                        zw.start_file(&arc, opts).unwrap();
                        zw.write_all(&fs::read(p)?)?;
                    }
                }
            } else {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                zw.start_file(&name, opts)?;
                zw.write_all(&fs::read(path)?)?;
            }
        }
        zw.finish()?;
    }
    Ok(buf)
}
