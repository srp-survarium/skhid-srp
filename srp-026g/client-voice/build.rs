fn main() {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if windows {
        for name in ["VOICE_OPUS_LIB_DIR", "SKHID_MSVC_LIB"] {
            println!("cargo:rerun-if-env-changed={name}");
            let path = std::env::var(name)
                .unwrap_or_else(|_| panic!("{name} is required; run in nix develop ./srp-026g"));
            println!("cargo:rustc-link-search=native={path}");
        }
        println!("cargo:rustc-link-lib=static=opus");
        // GCC's integer division and stack-probe helpers used by static Opus.
        // No GNU runtime DLL is used: the C runtime is Microsoft's static CRT.
        println!("cargo:rustc-link-lib=static=gcc");
    } else {
        println!("cargo:rustc-link-lib=opus");
    }
}
