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
        // The URL's path is the file's components joined by `/` on every host (Windows' own separator is `\`).
        let parts: Vec<&str> = file.strip_prefix(&root).unwrap().components().map(|part| part.as_os_str().to_str().unwrap()).collect();
        let path = parts.join("/");
        let kind = match file.extension().and_then(|ending| ending.to_str()) {
            Some("js") => "text/javascript; charset=utf-8",
            Some("woff2") => "font/woff2",
            Some("webp") => "image/webp",
            // The page itself and the patches it was built from.
            _ if ["damage.html", "built-from.tsv"].contains(&path.as_str()) => continue,
            _ => panic!("damage/{path}: no content type for its extension"),
        };
        // Both strings as Rust literals ({:?} escapes them).
        let url = format!("/damage/{path}");
        out += &format!("({url:?}, {kind:?}, include_bytes!({file:?}) as &[u8]),");
    }
    let list = Path::new(&env::var("OUT_DIR").unwrap()).join("damage_files.rs");
    fs::write(list, out + "]").unwrap();
}
