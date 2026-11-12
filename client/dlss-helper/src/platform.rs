//! What the helper needs of the system before NGX: the log, Wine or Windows,
//! NvAPI (NGX finds the GPU through it), the NGX core's presence.

use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};

static LOG: Mutex<Option<File>> = Mutex::new(None);

/// The log: `dlss_helper.log` beside the exe (else in the temporary folder), new at each start.
pub fn open_log(dir: &Path) {
    let file = File::create(dir.join("dlss_helper.log")).or_else(|_| File::create(std::env::temp_dir().join("skhid_dlss_helper.log")));
    *LOG.lock().unwrap() = file.ok();
}

pub fn log(line: &str) {
    if let Some(f) = LOG.lock().unwrap().as_mut() {
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => { $crate::platform::log(&format!($($t)*)) };
}

pub fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
fn wide_path(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// The folder of this exe.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Windows,
    Wine,
}

pub fn system() -> System {
    // SAFETY: ntdll is always loaded; Wine's exports wine_get_version.
    let found = unsafe {
        let ntdll = GetModuleHandleW(wide("ntdll.dll").as_ptr());
        !ntdll.is_null() && GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_some()
    };
    if found { System::Wine } else { System::Windows }
}

fn load(path: &[u16]) -> Option<PathBuf> {
    // SAFETY: a NUL-ended path; the module stays loaded for the process.
    let module = unsafe { LoadLibraryW(path.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let n = unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) };
    Some(PathBuf::from(OsString::from_wide(&buf[..n as usize])))
}

/// NvAPI loaded into this process, before NGX starts (NGX gets the GPU
/// through it and does not load it itself: without it NGX's init fails with
/// 0xBAD00002, PlatformError).
///
/// - Windows: the driver's own `nvapi64.dll` (System32).
/// - Wine: dxvk-nvapi from `wine\` beside the exe, after DXVK's 64-bit
///   `dxgi.dll` from there (dxvk-nvapi finds the GPU through DXVK's DXGI),
///   both by their full paths, with `DXVK_ENABLE_NVAPI=1` (DXVK then shows
///   the GPU as NVIDIA's). Nothing of `wine\` is touched on Windows.
pub fn load_nvapi(system: System, dir: &Path) -> Result<PathBuf, String> {
    match system {
        System::Windows => load(&wide("nvapi64.dll")).ok_or_else(|| "nvapi64.dll (NVIDIA's driver) cannot be loaded: not an NVIDIA driver".into()),
        System::Wine => {
            // SAFETY: plain environment strings.
            unsafe { windows_sys::Win32::System::Environment::SetEnvironmentVariableW(wide("DXVK_ENABLE_NVAPI").as_ptr(), wide("1").as_ptr()) };
            let wine = dir.join("wine");
            let dxgi = load(&wide_path(&wine.join("dxgi.dll"))).ok_or_else(|| format!("{}: cannot be loaded", wine.join("dxgi.dll").display()))?;
            log!("dxgi: {}", dxgi.display());
            if !dxgi.starts_with(&wine) && !same_dir(&dxgi, &wine) {
                return Err(format!("dxgi.dll came from {}, not DXVK's in {}", dxgi.display(), wine.display()));
            }
            load(&wide_path(&wine.join("nvapi64.dll"))).ok_or_else(|| format!("{}: cannot be loaded", wine.join("nvapi64.dll").display()))
        }
    }
}

fn same_dir(file: &Path, dir: &Path) -> bool {
    let a = file.parent().map(|p| p.to_string_lossy().to_lowercase());
    a == Some(dir.to_string_lossy().to_lowercase())
}

/// Under Wine, whether the prefix has NVIDIA's NGX core where NGX's loader
/// looks (the NGXCore key's folder, as Proton sets it), and if not, why and
/// where the driver keeps it.
pub fn wine_ngx_core() -> Result<PathBuf, String> {
    let key = wide("SOFTWARE\\NVIDIA Corporation\\Global\\NGXCore");
    let name = wide("FullPath");
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: buffers of the given size.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    let hint = || {
        let found = DRIVER_WINE_DIRS.iter().map(|d| PathBuf::from(format!("Z:{}", d.replace('/', "\\")))).find(|d| d.join("_nvngx.dll").is_file());
        match found {
            Some(d) => format!(
                "copy nvngx.dll and _nvngx.dll from {} into C:\\windows\\system32 and set HKLM\\SOFTWARE\\NVIDIA Corporation\\Global\\NGXCore FullPath = C:\\Windows\\System32 (play_skhid.sh does this)",
                d.display()
            ),
            None => "the NVIDIA driver's Wine NGX files (nvidia/wine/_nvngx.dll) were not found: an NVIDIA driver 470 or later with its Wine files is needed".into(),
        }
    };
    if r != 0 {
        return Err(format!("this Wine prefix has no NGX core (no NGXCore key): {}", hint()));
    }
    let len = (size as usize / 2).saturating_sub(1);
    let dir = PathBuf::from(OsString::from_wide(&buf[..len]));
    if !dir.join("_nvngx.dll").is_file() {
        return Err(format!("the NGXCore key names {}, which has no _nvngx.dll: {}", dir.display(), hint()));
    }
    Ok(dir)
}

/// Where Linux distributions put the NVIDIA driver's Wine DLLs.
pub const DRIVER_WINE_DIRS: [&str; 6] = [
    "/usr/lib/nvidia/wine",
    "/usr/lib64/nvidia/wine",
    "/usr/lib/x86_64-linux-gnu/nvidia/wine",
    "/usr/lib/nvidia/wine64",
    "/run/opengl-driver/lib/nvidia/wine",
    "/usr/lib64/nvidia/wine64",
];

/// A folder NGX may write its logs and caches to.
pub fn data_dir() -> Vec<u16> {
    let dir = std::env::temp_dir().join("skhid_dlss");
    let _ = std::fs::create_dir_all(&dir);
    wide_path(&dir)
}
