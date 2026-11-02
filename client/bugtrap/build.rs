#[path = "../windows_build.rs"]
mod windows_build;

fn main() {
    // Full-memory dumps are for local test builds: a release (SKHID_REVISION)
    // never has them.
    if std::env::var_os("CARGO_FEATURE_FULL_MEMORY_DUMPS").is_some()
        && std::env::var("SKHID_REVISION").is_ok_and(|r| !r.is_empty())
    {
        panic!("the full-memory-dumps feature is for local builds, never with SKHID_REVISION");
    }
    windows_build::link_msvc_runtime();
    windows_build::resources("res/BugTrap.rc");
}
