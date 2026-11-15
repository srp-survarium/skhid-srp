#[path = "../../windows_build.rs"]
mod windows_build;

fn main() {
    windows_build::link_msvc_runtime();
    windows_build::resources("res/updater_skhid.rc");
    // The release's public key (`src/signed.rs`): `SKHID_TEST_PUBLIC_KEY`
    // when the build is given one (the tests'), else release/signing.pub.
    // Either must be 64 hex digits: anything else stops the build, so a
    // stray value is never built in as a key that verifies nothing.
    println!("cargo:rerun-if-env-changed=SKHID_TEST_PUBLIC_KEY");
    let public = "../../../release/signing.pub";
    println!("cargo:rerun-if-changed={public}");
    let key = match std::env::var("SKHID_TEST_PUBLIC_KEY") {
        Ok(text) => Some(("SKHID_TEST_PUBLIC_KEY", text)),
        Err(_) => std::fs::read_to_string(public).ok().map(|text| (public, text)),
    };
    if let Some((from, text)) = key {
        let text = text.trim();
        assert!(
            text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
            "{from}: not a public key (64 hex digits)"
        );
        println!("cargo:rustc-env=SKHID_BUILT_PUBLIC_KEY={text}");
    }
}
