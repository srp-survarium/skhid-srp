//! Batch extraction through the split-aware resource-porter reader. Binary
//! configs are widened to 0.26's value layout; mesh/bind-pose bytes stay native.
use std::{fs, path::Path};
use vostok_resource_porter::{ResourceDb, binary_config::BinaryConfig};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(args.len() == 3, "usage: extract-donor <archive> <paths.txt> <out>");
    let mut archive = ResourceDb::open(&args[0])?;
    for path in fs::read_to_string(&args[1])?.lines().filter(|p| !p.is_empty()) {
        anyhow::ensure!(!path.starts_with('/') && !path.split('/').any(|p| p == ".."), "unsafe path");
        let mut data = archive.read(path)?;
        if path.ends_with("/settings") || path.ends_with("/export_properties")
            || path.ends_with("/skeleton") || path.ends_with(".material") || path.ends_with(".options")
        {
            data = BinaryConfig::parse(&data)?.to_bytes()?;
        }
        let out = Path::new(&args[2]).join(path);
        fs::create_dir_all(out.parent().unwrap())?;
        fs::write(out, data)?;
    }
    Ok(())
}
