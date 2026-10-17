use crate::text::Wide;
use core::{fmt::Write, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::SystemServices::*,
    UI::{Controls::*, Shell::ShellExecuteW, WindowsAndMessaging::*},
};

// Original BugTrap icon and white message panel above a system-color footer.
// Submit Bug opens SRP Discord in the browser; it never uploads the report.
// ICO payloads start two bytes past a DWORD boundary. Add two leading bytes
// so CreateIconFromResourceEx receives DWORD-aligned resource bits.
#[repr(C, align(4))]
struct IconFile {
    padding: [u8; 2],
    bytes: [u8; include_bytes!("../res/Bug.ico").len()],
}
static BUG_ICON: IconFile = IconFile {
    padding: [0; 2],
    bytes: *include_bytes!("../res/Bug.ico"),
};

unsafe fn bug_icon(small: bool) -> HICON {
    // The original ICO contains 16, 32 and 48 pixel images, in that order.
    let bytes = &BUG_ICON.bytes;
    let entry = 6 + if small { 0 } else { 16 };
    let dword = |offset| {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };
    let size = dword(entry + 8);
    let offset = dword(entry + 12) as usize;
    let pixels = if small { SM_CXSMICON } else { SM_CXICON };
    let pixels = GetSystemMetrics(pixels);
    CreateIconFromResourceEx(
        bytes.as_ptr().add(offset),
        size,
        1,
        0x00030000,
        pixels,
        pixels,
        LR_DEFAULTCOLOR,
    )
}

#[repr(C, align(4))]
struct Template {
    data: [u16; 512],
    n: usize,
}
struct DialogData {
    message: *const u16,
    font: HFONT,
    icon: HICON,
    small_icon: HICON,
    folder: *const u16,
    filename: *const u16,
    have_dump: bool,
    zipped: bool,
}

impl Template {
    fn word(&mut self, v: u16) {
        self.data[self.n] = v;
        self.n += 1;
    }
    fn dword(&mut self, v: u32) {
        self.word(v as u16);
        self.word((v >> 16) as u16);
    }
    fn text(&mut self, s: &str) {
        for c in s.encode_utf16() {
            self.word(c);
        }
        self.word(0);
    }
    fn align(&mut self) {
        if self.n % 2 != 0 {
            self.word(0);
        }
    }
    fn control(&mut self, style: u32, bounds: [u16; 4], id: u16, class: u16, text: &str) {
        self.align();
        self.dword(style);
        self.dword(0);
        for v in bounds {
            self.word(v);
        }
        self.word(id);
        self.word(0xffff);
        self.word(class);
        self.text(text);
        self.word(0);
    }
}
unsafe extern "system" fn dialog(window: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    match msg {
        WM_INITDIALOG => {
            let data = &mut *(lp as *mut DialogData);
            // Use the same compact TrueType face for every control. Explicit
            // smoothing avoids Wine's jagged small-font rendering.
            let base = SendMessageW(window, WM_GETFONT, 0, 0) as HFONT;
            let mut font: LOGFONTW = core::mem::zeroed();
            if GetObjectW(
                base,
                core::mem::size_of::<LOGFONTW>() as i32,
                (&mut font as *mut LOGFONTW).cast(),
            ) != 0
            {
                font.lfQuality = ANTIALIASED_QUALITY;
                data.font = CreateFontIndirectW(&font);
                if !data.font.is_null() {
                    for id in [1030, 1031, 1034, 1035, 1036, 1037, 1038, 1013, IDCANCEL] {
                        SendDlgItemMessageW(window, id, WM_SETFONT, data.font as usize, 1);
                    }
                }
            }
            SetDlgItemTextW(window, 1031, data.message);
            if data.have_dump {
                SetDlgItemTextW(window, 1034, data.filename);
                SetDlgItemTextW(window, 1035, data.folder);
            } else {
                for id in [1034, 1035, 1037, 1038] {
                    ShowWindow(GetDlgItem(window, id), SW_HIDE);
                }
            }
            SetDlgItemTextW(
                window,
                1036,
                if data.zipped {
                    windows_sys::w!("Use Submit Bug to open SRP Discord and upload this .zip file.")
                } else if data.have_dump {
                    windows_sys::w!("Use Submit Bug to open SRP Discord and attach this file.")
                } else {
                    windows_sys::w!("Use Submit Bug to open SRP Discord.")
                },
            );
            data.icon = bug_icon(false);
            data.small_icon = bug_icon(true);
            if !data.icon.is_null() {
                SendDlgItemMessageW(window, 1032, STM_SETICON, data.icon as usize, 0);
                SendMessageW(window, WM_SETICON, ICON_BIG as usize, data.icon as isize);
            }
            if !data.small_icon.is_null() {
                SendMessageW(
                    window,
                    WM_SETICON,
                    ICON_SMALL as usize,
                    data.small_icon as isize,
                );
            }
            1
        }
        WM_CTLCOLORSTATIC if matches!(GetDlgCtrlID(lp as HWND), 1030..=1032 | 1034..=1039) => {
            SetTextColor(wp as HDC, GetSysColor(COLOR_WINDOWTEXT));
            SetBkColor(wp as HDC, GetSysColor(COLOR_WINDOW));
            GetSysColorBrush(COLOR_WINDOW) as isize
        }
        WM_DRAWITEM if wp == 1013 => {
            let item = &*(lp as *const DRAWITEMSTRUCT);
            let saved = SaveDC(item.hDC);
            let mut rect = item.rcItem;
            let pressed = item.itemState & ODS_SELECTED != 0;
            let disabled = item.itemState & ODS_DISABLED != 0;
            FillRect(item.hDC, &rect, GetSysColorBrush(COLOR_BTNFACE));
            DrawFrameControl(
                item.hDC,
                &mut rect,
                DFC_BUTTON,
                DFCS_BUTTONPUSH
                    | if pressed { DFCS_PUSHED } else { 0 }
                    | if disabled { DFCS_INACTIVE } else { 0 },
            );
            // Draw directly in the button's full client rectangle, using the
            // same font for measurement and painting. Wine's native caption
            // path can offset this label even with BS_CENTER set.
            SelectObject(
                item.hDC,
                SendMessageW(item.hwndItem, WM_GETFONT, 0, 0) as HFONT,
            );
            SetBkMode(item.hDC, TRANSPARENT as i32);
            SetTextColor(
                item.hDC,
                GetSysColor(if disabled {
                    COLOR_GRAYTEXT
                } else {
                    COLOR_BTNTEXT
                }),
            );
            rect = item.rcItem;
            if pressed {
                OffsetRect(&mut rect, 1, 1);
            }
            let mut size: SIZE = core::mem::zeroed();
            let mut advances = [0; 10];
            GetTextExtentExPointW(
                item.hDC,
                windows_sys::w!("Submit Bug"),
                10,
                0,
                null_mut(),
                advances.as_mut_ptr(),
                &mut size,
            );
            for i in (1..advances.len()).rev() {
                advances[i] -= advances[i - 1];
            }
            SetTextAlign(item.hDC, TA_LEFT | TA_TOP | TA_NOUPDATECP);
            ExtTextOutW(
                item.hDC,
                rect.left + (rect.right - rect.left - size.cx) / 2,
                rect.top + (rect.bottom - rect.top - size.cy) / 2,
                ETO_CLIPPED,
                &rect,
                windows_sys::w!("Submit Bug"),
                10,
                advances.as_ptr(),
            );
            if item.itemState & ODS_FOCUS != 0 && item.itemState & ODS_NOFOCUSRECT == 0 {
                rect = item.rcItem;
                InflateRect(&mut rect, -3, -3);
                DrawFocusRect(item.hDC, &rect);
            }
            RestoreDC(item.hDC, saved);
            1
        }
        WM_COMMAND if wp as u16 == 1013 => {
            let result = ShellExecuteW(
                window,
                windows_sys::w!("open"),
                windows_sys::w!("https://discord.gg/Pxa4prXfwu"),
                core::ptr::null(),
                core::ptr::null(),
                SW_SHOWNORMAL,
            );
            if result as isize <= 32 {
                MessageBoxW(
                    window,
                    windows_sys::w!(
                        "Could not open your browser. Visit discord.gg/Pxa4prXfwu to report the crash."
                    ),
                    windows_sys::w!("SRP Discord"),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            1
        }
        WM_COMMAND if wp as u16 == IDCANCEL as u16 || wp as u16 == IDOK as u16 => {
            EndDialog(window, 0);
            1
        }
        WM_CLOSE => {
            EndDialog(window, 0);
            1
        }
        _ => 0,
    }
}
pub unsafe fn show(path: &Wide<32768>, saved: bool, partial: bool, error: u32, zipped: bool) {
    let mut message = Wide::<34000>::new();
    let split = path.buf[..path.len]
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map_or(0, |i| i + 1);
    let mut folder = Wide::<32768>::new();
    let _ = folder.wide(&path.buf[..split]);
    if saved {
        let _ = write!(message, "Crash dump saved:");
    } else if partial {
        let _ = write!(
            message,
            "A dump was saved, but could not be fully verified (Windows error {error}):\r\n"
        );
    } else {
        let _ = write!(
            message,
            "Could not save a crash dump beside the executable or in the temporary folder (Windows error {error})."
        );
    }
    let mut t = Template {
        data: [0; 512],
        n: 0,
    };
    t.dword(
        WS_POPUP
            | WS_CAPTION
            | WS_SYSMENU
            | DS_SETFOREGROUND as u32
            | DS_FIXEDSYS as u32
            | DS_SETFONT as u32
            | DS_CENTER as u32,
    );
    t.dword(0);
    t.word(12);
    t.word(0);
    t.word(0);
    t.word(330);
    t.word(118);
    t.word(0);
    t.word(0);
    t.text("Vostok Engine");
    // Match IDD_SIMPLE_DLG in the original BugTrap.rc. The white panel is
    // taller to fit our persistent dump location and local-reporting text.
    t.word(8);
    t.text("Tahoma");
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [0, 0, 330, 77],
        1039,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_ICON,
        [7, 7, 20, 20],
        1032,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
        [35, 7, 288, 8],
        1030,
        0x82,
        "A crash has been detected by BugTrap",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
        [35, 19, 288, 18],
        1031,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [35, 38, 24, 8],
        1037,
        0x82,
        "File:",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFTNOWORDWRAP | SS_NOPREFIX | SS_ENDELLIPSIS,
        [59, 38, 264, 8],
        1034,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [35, 48, 24, 8],
        1038,
        0x82,
        "Folder:",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFTNOWORDWRAP | SS_NOPREFIX | SS_PATHELLIPSIS,
        [59, 48, 264, 8],
        1035,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
        [35, 62, 288, 8],
        1036,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_ETCHEDHORZ,
        [0, 76, 330, 1],
        1033,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON | BS_CENTER | BS_VCENTER) as u32,
        [271, 97, 52, 14],
        IDCANCEL as u16,
        0x80,
        "Close",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        [7, 97, 52, 14],
        1013,
        0x80,
        "Submit Bug",
    );
    let mut data = DialogData {
        message: message.ptr(),
        font: null_mut(),
        icon: null_mut(),
        small_icon: null_mut(),
        folder: folder.ptr(),
        filename: path.buf.as_ptr().add(split),
        have_dump: saved || partial,
        zipped,
    };
    let result = DialogBoxIndirectParamW(
        null_mut(),
        t.data.as_ptr().cast(),
        null_mut(),
        Some(dialog),
        (&mut data as *mut DialogData) as isize,
    );
    if !data.font.is_null() {
        DeleteObject(data.font);
    }
    for icon in [data.icon, data.small_icon] {
        if !icon.is_null() {
            DestroyIcon(icon);
        }
    }
    if result == -1 {
        if saved || partial {
            let _ = write!(message, "\r\n");
            let _ = message.wide(&path.buf[..path.len]);
        }
        MessageBoxW(
            null_mut(),
            message.ptr(),
            windows_sys::w!("Vostok Engine"),
            MB_OK | MB_ICONERROR | MB_TASKMODAL,
        );
    }
}
