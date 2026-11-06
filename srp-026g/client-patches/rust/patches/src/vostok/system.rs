//! The system the game runs on: Windows, or Wine (ntdll exports
//! `wine_get_version`, which Windows' does not).

use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    Windows,
    Wine,
}

impl System {
    pub fn running() -> System {
        // SAFETY: kernel32's GetModuleHandleA and GetProcAddress (the game's
        // imports), given NUL-terminated names; ntdll is in every process.
        // The answer is only compared with 0, never called.
        let wine = unsafe {
            let ntdll = (raw::engine_get_module_handle_a)(c"ntdll.dll".as_ptr().cast());
            !ntdll.is_null() && !(raw::engine_get_proc_address)(ntdll, c"wine_get_version".as_ptr().cast()).is_null()
        };
        if wine { System::Wine } else { System::Windows }
    }
}
