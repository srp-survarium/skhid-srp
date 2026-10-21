//! What the build scripts of the players' Windows programs share
//! (`bugtrap/build.rs`, `updater/exe/build.rs`, the updater tests' stand-in
//! program): Microsoft's C runtime to link, and the version and icon
//! resources, compiled by `llvm-rc`. `srp-026g/flake.nix` supplies both.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the linker finds `libcmt.lib`, `libvcruntime.lib`, `libucrt.lib`
/// and the import libraries the standard library names: the flake's
/// `SKHID_MSVC_LIB`.
pub fn link_msvc_runtime() {
    println!("cargo:rerun-if-env-changed=SKHID_MSVC_LIB");
    let Ok(lib) = env::var("SKHID_MSVC_LIB") else {
        panic!(
            "SKHID_MSVC_LIB is not set: build in `nix develop ./srp-026g`, whose flake supplies \
             Microsoft's C runtime libraries (or point it to a folder with libcmt.lib, \
             libvcruntime.lib, libucrt.lib and the 32-bit import libraries srp-026g/flake.nix \
             lists)"
        );
    };
    println!("cargo:rustc-link-search=native={lib}");
}

/// The Skhid release revision the build is for: `SKHID_REVISION` (1 to 255,
/// the patch revision of the release, `release/build-zip` sets it), or none
/// for a local build.
fn revision() -> Option<u8> {
    println!("cargo:rerun-if-env-changed=SKHID_REVISION");
    let text = env::var("SKHID_REVISION").ok().filter(|text| !text.is_empty())?;
    match text.parse::<u8>() {
        Ok(revision) if revision >= 1 => Some(revision),
        _ => panic!("SKHID_REVISION={text}: a release revision is 1 to 255"),
    }
}

/// Compiles the resource script `rc` (relative to the crate) into a `.res`
/// the linker puts into the program. In the script, `@VERSION_FIXED@`,
/// `@VERSION@` and `@PRODUCT_VERSION@` become the revision (`27,0,0,0`,
/// `27.0.0.0`, `revision 27`; a local build: `0,0,0,0`, `0.0.0.0`,
/// `local build`) and `@DIR@` the crate's folder (for an `ICON` file).
pub fn resources(rc: &str) {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let script = dir.join(rc);
    println!("cargo:rerun-if-changed={}", script.display());
    let (fixed, version, product) = match revision() {
        Some(n) => (format!("{n},0,0,0"), format!("{n}.0.0.0"), format!("revision {n}")),
        None => ("0,0,0,0".into(), "0.0.0.0".into(), "local build".into()),
    };
    let text = fs::read_to_string(&script).unwrap_or_else(|e| panic!("{}: {e}", script.display()));
    let text = text
        .replace("@VERSION_FIXED@", &fixed)
        .replace("@VERSION@", &version)
        .replace("@PRODUCT_VERSION@", &product)
        .replace("@DIR@", dir.to_str().expect("a crate folder of UTF-8"));
    for line in text.lines().filter(|line| line.contains("ICON")) {
        if let Some(file) = line.split('"').nth(1) {
            println!("cargo:rerun-if-changed={file}");
        }
    }
    let name = Path::new(rc).file_stem().unwrap().to_str().unwrap();
    let generated = out.join(format!("{name}.rc"));
    let res = out.join(format!("{name}.res"));
    fs::write(&generated, text).unwrap();
    // No preprocessor (the script has no #include or #define), code page
    // 65001: the script is UTF-8.
    let status = Command::new("llvm-rc")
        .args(["-no-preprocess", "/C", "65001", "/FO"])
        .arg(&res)
        .arg(&generated)
        .status()
        .unwrap_or_else(|e| panic!("llvm-rc: {e} (build in `nix develop ./srp-026g`)"));
    assert!(status.success(), "llvm-rc failed on {}", generated.display());
    println!("cargo:rustc-link-arg={}", res.display());
}
