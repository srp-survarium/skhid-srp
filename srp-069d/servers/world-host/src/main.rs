//! world-host: runs the 0.69d client's own code (survarium.exe) inside a Linux process.
//! Linux x86-64 only; on other targets the binary only prints a note.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    std::process::exit(world_host::run::main());
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("world-host runs on Linux x86-64 only");
    std::process::exit(2);
}
