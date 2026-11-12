#[path = "../windows_build.rs"]
#[allow(dead_code)] // its 32-bit runtime search (link_msvc_runtime) is not this program's
mod windows_build;

use std::env;

fn main() {
    // Microsoft's 64-bit C and C++ runtime and import libraries (the flake's
    // SKHID_MSVC_LIB64), and NVIDIA's NGX library (SKHID_NGX_LIB: the DLSS
    // SDK's lib/Windows_x86_64/x64/nvsdk_ngx_s.lib, pinned in the flake).
    for var in ["SKHID_MSVC_LIB64", "SKHID_NGX_LIB"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    let lib = env::var("SKHID_MSVC_LIB64")
        .expect("SKHID_MSVC_LIB64 is not set: build in `nix develop ./srp-026g` (srp-026g/flake.nix, msvcLibs64)");
    let ngx = env::var("SKHID_NGX_LIB")
        .expect("SKHID_NGX_LIB is not set: build in `nix develop ./srp-026g` (srp-026g/flake.nix, ngxStaticLib)");
    println!("cargo:rustc-link-search=native={lib}");
    println!("cargo:rustc-link-arg={ngx}");
    // What nvsdk_ngx_s.lib asks for besides the C runtime (its imports).
    for name in ["libcpmt", "advapi32", "user32"] {
        println!("cargo:rustc-link-lib={name}");
    }
    windows_build::resources("res/dlss_helper.rc");
}
