#[path = "../../../windows_build.rs"]
#[allow(dead_code)]
mod windows_build;

fn main() {
    windows_build::link_msvc_runtime();
}
