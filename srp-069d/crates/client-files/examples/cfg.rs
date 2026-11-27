//! Print a config of the game's volumes as JSON (indent 1), or list paths.
//! usage: cfg <resources dir> <path> [volume] [utf8]  |  cfg <resources dir> --list <substring>
use client_files::pyjson::{dumps, Style};
use client_files::{Encoding, GameFiles};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = GameFiles::open(a[1].as_ref()).expect("open");
    if a[2] == "--list" {
        for p in g.paths() {
            if p.contains(a.get(3).map(String::as_str).unwrap_or("")) {
                println!("{p}\t{:?}", g.volumes_of(p));
            }
        }
        return;
    }
    let vol = a.get(3).and_then(|v| v.parse().ok());
    let enc = if a.iter().any(|x| x == "utf8") { Encoding::Utf8 } else { Encoding::Cp1251 };
    let b = match vol { Some(v) => g.read_in(v, &a[2]), None => g.read(&a[2]) }.expect("read");
    let v = client_files::decode_options(&b, enc).expect("decode");
    println!("{}", dumps(&v, &Style::indent(1).utf8()));
}
