#[path = "../../windows_build.rs"]
mod windows_build;

fn main() {
    windows_build::link_msvc_runtime();
    windows_build::resources("res/updater_skhid.rc");
    // The release's public key (`src/signed.rs`): `SKHID_SIGNING_KEY`
    // when the build is given one (the tests'), else release/signing.pub.
    println!("cargo:rerun-if-env-changed=SKHID_SIGNING_KEY");
    let public = "../../../release/signing.pub";
    println!("cargo:rerun-if-changed={public}");
    if std::env::var_os("SKHID_SIGNING_KEY").is_none()
        && let Ok(text) = std::fs::read_to_string(public)
    {
        println!("cargo:rustc-env=SKHID_SIGNING_KEY={}", text.trim());
    }
}
