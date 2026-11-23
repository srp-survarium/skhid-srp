//! [incremental grids] Bakes the source digests of this crate's builds into `mapdata::source` (`OUT_DIR/source_digests.rs`):
//! the digests of the tokens the grid's and the sight tree's builds reach (`mapdata-srchash`), so a comment, a doc or a test
//! edit renames no built file, and a binary names its files by the source it was built from, not by the tree it runs in.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let workspace = manifest.join("../..");
    let encoded = std::env::var("CARGO_ENCODED_RUSTFLAGS").ok();
    let plain = std::env::var("RUSTFLAGS").ok();
    let cfgs = mapdata_srchash::rustflags_cfgs(encoded.as_deref(), plain.as_deref());
    let digests = mapdata_srchash::mapdata_digests(&workspace, &cfgs)
        .unwrap_or_else(|why| panic!("mapdata's source digests: {why}"));
    for input in &digests.inputs {
        println!("cargo:rerun-if-changed={}", workspace.join(input).display());
    }
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("source_digests.rs");
    let text = format!(
        "/// The digest of every item of the crate (and the crates it uses), and the outside inputs.\n\
         pub const SRC_ALL: u128 = 0x{:032x};\n\
         /// The digest of what a grid's build and its file reach.\n\
         pub const SRC_GRID: u128 = 0x{:032x};\n\
         /// The digest of what a sight tree's build and its file reach.\n\
         pub const SRC_SIGHT: u128 = 0x{:032x};\n\
         /// Each root's items by file (`mapdata-srchash`'s report).\n\
         pub const REPORT: &str = {:?};\n",
        digests.all, digests.grid, digests.sight, digests.report
    );
    std::fs::write(&out, text).unwrap_or_else(|why| panic!("{}: {why}", out.display()));
}
