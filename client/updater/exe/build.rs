#[path = "../../windows_build.rs"]
mod windows_build;

fn main() {
    windows_build::link_msvc_runtime();
    windows_build::resources("res/updater_skhid.rc");
}
