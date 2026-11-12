//! Per-user Explorer registration. Never writes UserChoice or another app's
//! default. Windows can ask the player to select Skhid Replay on first open.
use core::ptr::{null, null_mut};
use updater_logic::local_replay::{PROG_ID, association_command};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegSetValueExW,
};
use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};

use crate::{install::Layout, log::log, win};

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: owned registry handle from RegCreateKeyExW, closed once.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn set(path: &str, name: &str, value: &str) -> Result<(), win::Code> {
    let mut key = null_mut();
    // SAFETY: NUL-terminated strings and live output; HKCU needs no elevation.
    let code = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            win::wide(path).as_ptr(),
            0,
            null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            null(),
            &mut key,
            null_mut(),
        )
    };
    if code != 0 {
        return Err(code);
    }
    let key = Key(key);
    let value = win::wide(value);
    // SAFETY: value includes its terminator; byte count covers all UTF-16 units.
    let code = unsafe {
        RegSetValueExW(
            key.0,
            win::wide(name).as_ptr(),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    };
    if code == 0 { Ok(()) } else { Err(code) }
}

pub fn register(layout: &Layout) -> Result<(), win::Code> {
    let exe = format!("{}\\updater_skhid.exe", layout.exe_dir);
    let command = association_command(&exe).ok_or(87_u32)?;
    let class = format!("Software\\Classes\\{PROG_ID}");
    // Publish the extension last, after its handler is complete. Repeated
    // registration repairs our own paths when an installation has moved.
    set(&class, "", "Skhid Replay")?;
    set(
        &format!("{class}\\DefaultIcon"),
        "",
        &format!("\"{exe}\",0"),
    )?;
    set(&format!("{class}\\shell\\open\\command"), "", &command)?;
    let app = "Software\\Classes\\Applications\\updater_skhid.exe";
    set(app, "FriendlyAppName", "Skhid Replay")?;
    set(&format!("{app}\\shell\\open\\command"), "", &command)?;
    set(&format!("{app}\\SupportedTypes"), ".sur", "")?;
    let capabilities = "Software\\Skhid\\ReplayCapabilities";
    set(capabilities, "ApplicationName", "Skhid Replay")?;
    set(
        capabilities,
        "ApplicationDescription",
        "Play Survarium .sur replays with Skhid.",
    )?;
    set(
        &format!("{capabilities}\\FileAssociations"),
        ".sur",
        PROG_ID,
    )?;
    set(
        "Software\\RegisteredApplications",
        "Skhid Replay",
        capabilities,
    )?;
    set("Software\\Classes\\.sur\\OpenWithProgids", PROG_ID, "")?;
    // SAFETY: association-change notification has no item pointers.
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, null(), null());
    }
    log!(
        "replay: registered Skhid Replay for .sur files ({exe}); Windows keeps the user's default choice"
    );
    Ok(())
}
