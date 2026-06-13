use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::Path;

use anyhow::{Context, Result};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Clone)]
pub struct Package {
    pub entries: BTreeMap<String, Vec<u8>>,
}

impl Package {
    pub fn read(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open input package {}", path.display()))?;
        let mut archive = ZipArchive::new(file)
            .with_context(|| format!("failed to read ZIP package {}", path.display()))?;
        read_archive(&mut archive, path)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let cursor = std::io::Cursor::new(bytes);
        let mut archive = ZipArchive::new(cursor)
            .with_context(|| "failed to read ZIP package from bytes")?;
        read_archive(&mut archive, Path::new("memory"))
    }

    pub fn get_text(&self, name: &str) -> Result<String> {
        let bytes = self
            .entries
            .get(name)
            .with_context(|| format!("missing required package entry {name}"))?;
        String::from_utf8(bytes.clone())
            .with_context(|| format!("package entry {name} is not valid UTF-8"))
    }
}

fn read_archive<R: Read + Seek>(archive: &mut ZipArchive<R>, path: &Path) -> Result<Package> {
    let mut entries = BTreeMap::new();

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).with_context(|| {
            format!("failed to read ZIP entry #{index} from {}", path.display())
        })?;
        if file.is_dir() {
            continue;
        }

        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .with_context(|| format!("failed to read ZIP entry {}", file.name()))?;
        entries.insert(file.name().to_string(), bytes);
    }

    Ok(Package { entries })
}

pub fn write_package(path: &Path, entries: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("failed to create output package {}", path.display()))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for (name, bytes) in entries {
        writer
            .start_file(name, options)
            .with_context(|| format!("failed to start ZIP entry {name}"))?;
        writer
            .write_all(bytes)
            .with_context(|| format!("failed to write ZIP entry {name}"))?;
    }

    writer
        .finish()
        .context("failed to finish output ZIP package")?;
    Ok(())
}

pub fn write_package_to_bytes(entries: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    {
        let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);

        for (name, bytes) in entries {
            writer
                .start_file(name, options)
                .with_context(|| format!("failed to start ZIP entry {name}"))?;
            writer
                .write_all(bytes)
                .with_context(|| format!("failed to write ZIP entry {name}"))?;
        }

        writer.finish().context("failed to finish output ZIP package")?;
    }
    Ok(buffer)
}
