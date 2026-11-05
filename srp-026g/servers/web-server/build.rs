//! Lists `damage/` for `src/damage.rs` to embed: the calculator's page serves
//! every file there but itself and its record, typed by its extension.

use std::{env, fs, path::Path};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("damage");
    println!("cargo::rerun-if-changed={}", root.display());
    let (mut files, mut out) = (vec![root.clone()], String::from("&["));
    while let Some(file) = files.pop() {
        if file.is_dir() {
            let entries = fs::read_dir(&file).unwrap();
            files.extend(entries.map(|entry| entry.unwrap().path()));
            continue;
        }
        let path = file.strip_prefix(&root).unwrap().to_str().unwrap();
        let kind = match file.extension().and_then(|ending| ending.to_str()) {
            Some("js") => "text/javascript; charset=utf-8",
            Some("woff2") => "font/woff2",
            Some("webp") => "image/webp",
            // The page itself and the patches it was built from.
            _ if ["damage.html", "built-from.tsv"].contains(&path) => continue,
            _ => panic!("damage/{path}: no content type for its extension"),
        };
        out += &format!("(\"/damage/{path}\", \"{kind}\", include_bytes!({file:?}) as &[u8]),");
    }
    let list = Path::new(&env::var("OUT_DIR").unwrap()).join("damage_files.rs");
    fs::write(list, out + "]").unwrap();
}
