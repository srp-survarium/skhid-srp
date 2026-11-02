use crate::{crash_text, details, notice_text::language, report::Report, text::Wide};
use core::{fmt::Write, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Globalization::GetUserDefaultUILanguage,
    Graphics::Gdi::*,
    System::{
        DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
        Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        SystemServices::*,
    },
    UI::{Controls::*, Shell::ShellExecuteW, WindowsAndMessaging::*},
};

// Original BugTrap icon and white message panel above a system-color footer.
// Open Discord opens SRP Discord in the browser; it never uploads the report.
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
    data: [u16; 2048],
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
    text: &'static crash_text::Text,
    /// The short details the dialog shows, and what "Copy" copies.
    details: *const u16,
    clip: *const [u16],
    /// The non-ASCII folder warning, or empty.
    warning: *const u16,
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
            // This DLL is Win32-only: the dialog's user slot is a LONG.
            SetWindowLongW(window, DWL_USER as i32, lp as i32);
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
                    for id in [
                        1030, 1031, 1034, 1035, 1036, 1037, 1038, 1040, 1041, 1042, 1013, IDCANCEL,
                    ] {
                        SendDlgItemMessageW(window, id, WM_SETFONT, data.font as usize, 1);
                    }
                }
            }
            SetDlgItemTextW(window, 1031, data.message);
            SetDlgItemTextW(window, 1040, data.details);
            SetDlgItemTextW(window, 1042, data.warning);
            // Aligned with the static text above and below it.
            SendDlgItemMessageW(
                window,
                1040,
                EM_SETMARGINS,
                (EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize,
                0,
            );
            tighten(window);
            if data.have_dump {
                SetDlgItemTextW(window, 1034, data.filename);
                SetDlgItemTextW(window, 1035, data.folder);
            } else {
                for id in [1034, 1035, 1037, 1038] {
                    ShowWindow(GetDlgItem(window, id), SW_HIDE);
                }
                // No file: the instructions take the File and Folder rows' place.
                let slack = place(window, 1036).top - place(window, 1037).top;
                lift(window, &[1039, 1036, 1033, 1013, 1041, IDCANCEL], slack);
            }
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
        WM_CTLCOLORSTATIC if matches!(GetDlgCtrlID(lp as HWND), 1030..=1032 | 1034..=1040 | 1042) =>
        {
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
            let mut label = Wide::<64>::new();
            label.len = GetWindowTextW(item.hwndItem, label.buf.as_mut_ptr(), 64) as usize;
            let mut advances = [0; 64];
            GetTextExtentExPointW(
                item.hDC,
                label.ptr(),
                label.len as i32,
                0,
                null_mut(),
                advances.as_mut_ptr(),
                &mut size,
            );
            for i in (1..label.len).rev() {
                advances[i] -= advances[i - 1];
            }
            SetTextAlign(item.hDC, TA_LEFT | TA_TOP | TA_NOUPDATECP);
            ExtTextOutW(
                item.hDC,
                rect.left + (rect.right - rect.left - size.cx) / 2,
                rect.top + (rect.bottom - rect.top - size.cy) / 2,
                ETO_CLIPPED,
                &rect,
                label.ptr(),
                label.len as u32,
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
                windows_sys::w!("https://discord.gg/QUmJg3gBhg"),
                core::ptr::null(),
                core::ptr::null(),
                SW_SHOWNORMAL,
            );
            if result as isize <= 32 {
                let data = &*(GetWindowLongW(window, DWL_USER as i32) as *const DialogData);
                let mut message = Wide::<512>::new();
                let mut title = Wide::<128>::new();
                let _ = write!(
                    message,
                    "{}\r\n\r\n{}",
                    data.text.browser_failed, data.text.manual
                );
                let _ = write!(title, "{}", data.text.title);
                MessageBoxW(
                    window,
                    message.ptr(),
                    title.ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            1
        }
        WM_COMMAND if wp as u16 == 1041 => {
            let data = &*(GetWindowLongW(window, DWL_USER as i32) as *const DialogData);
            if copy(window, &*data.clip) {
                let mut label = Wide::<64>::new();
                let _ = write!(label, "{}", data.text.copied);
                SetDlgItemTextW(window, 1041, label.ptr());
            } else {
                // No clipboard: select the details for Ctrl+C.
                let details = GetDlgItem(window, 1040);
                SendMessageW(window, WM_NEXTDLGCTL, details as usize, 1);
                SendMessageW(details, EM_SETSEL, 0, -1);
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
/// The status line has room for four lines; with fewer, everything below it
/// moves up by what is left over and the dialog is that much shorter.
unsafe fn tighten(window: HWND) {
    let message = GetDlgItem(window, 1031);
    let mut bounds: RECT = core::mem::zeroed();
    GetWindowRect(message, &mut bounds);
    MapWindowPoints(null_mut(), window, (&mut bounds as *mut RECT).cast(), 2);
    let mut text = [0u16; 1024];
    let n = GetWindowTextW(message, text.as_mut_ptr(), text.len() as i32);
    let mut needed = RECT {
        left: 0,
        top: 0,
        right: bounds.right - bounds.left,
        bottom: 0,
    };
    let dc = GetDC(message);
    let old = SelectObject(dc, SendMessageW(message, WM_GETFONT, 0, 0) as HGDIOBJ);
    DrawTextW(
        dc,
        text.as_ptr(),
        n,
        &mut needed,
        DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
    );
    SelectObject(dc, old);
    ReleaseDC(message, dc);
    // A gap of 4 dialog units stays between the status and the details.
    let mut gap = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 4,
    };
    MapDialogRect(window, &mut gap);
    let slack = bounds.bottom - bounds.top - needed.bottom - gap.bottom;
    if n == 0 || slack <= 0 {
        return;
    }
    let width = bounds.right - bounds.left;
    SetWindowPos(
        message,
        null_mut(),
        0,
        0,
        width,
        needed.bottom + gap.bottom,
        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
    lift(
        window,
        &[
            1039, 1040, 1042, 1037, 1034, 1038, 1035, 1036, 1033, 1013, 1041, IDCANCEL,
        ],
        slack,
    );
}

/// Where a control is, in the dialog's client coordinates.
unsafe fn place(window: HWND, id: i32) -> RECT {
    let mut r: RECT = core::mem::zeroed();
    GetWindowRect(GetDlgItem(window, id), &mut r);
    MapWindowPoints(null_mut(), window, (&mut r as *mut RECT).cast(), 2);
    r
}

/// Moves the controls `ids` up by `slack` pixels (the white panel, 1039, gets
/// that much shorter instead) and makes the dialog that much shorter.
unsafe fn lift(window: HWND, ids: &[i32], slack: i32) {
    if slack <= 0 {
        return;
    }
    for &id in ids {
        let control = GetDlgItem(window, id);
        if control.is_null() {
            continue;
        }
        let r = place(window, id);
        // The white panel behind the text gets shorter; the rest moves up.
        let (top, height) = if id == 1039 {
            (r.top, r.bottom - r.top - slack)
        } else {
            (r.top - slack, r.bottom - r.top)
        };
        SetWindowPos(
            control,
            null_mut(),
            r.left,
            top,
            r.right - r.left,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    let mut frame: RECT = core::mem::zeroed();
    GetWindowRect(window, &mut frame);
    SetWindowPos(
        window,
        null_mut(),
        frame.left,
        frame.top + slack / 2,
        frame.right - frame.left,
        frame.bottom - frame.top - slack,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

/// Puts `text` on the clipboard as Unicode text.
unsafe fn copy(window: HWND, text: &[u16]) -> bool {
    const CF_UNICODETEXT: u32 = 13;
    if OpenClipboard(window) == 0 {
        return false;
    }
    let mut ok = false;
    if EmptyClipboard() != 0 {
        let memory = GlobalAlloc(GMEM_MOVEABLE, (text.len() + 1) * 2);
        if !memory.is_null() {
            let p = GlobalLock(memory).cast::<u16>();
            if !p.is_null() {
                core::ptr::copy_nonoverlapping(text.as_ptr(), p, text.len());
                *p.add(text.len()) = 0;
                GlobalUnlock(memory);
                ok = !SetClipboardData(CF_UNICODETEXT, memory).is_null();
            }
            if !ok {
                GlobalFree(memory);
            }
        }
    }
    CloseClipboard();
    ok
}

/// `path`: the kept dump (ZIP or `.dmp`), or with none the text report, or
/// empty. `capture`: the last dump error came from the capture itself.
pub unsafe fn show(
    path: &Wide<32768>,
    saved: bool,
    partial: bool,
    error: u32,
    capture: bool,
    report: &Report,
) {
    let text = crash_text::text(language(GetUserDefaultUILanguage()));
    let mut title = Wide::<128>::new();
    let _ = write!(title, "{}", text.title);
    let mut message = Wide::<34000>::new();
    let split = path.buf[..path.len]
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map_or(0, |i| i + 1);
    let mut folder = Wide::<32768>::new();
    let _ = folder.wide(&path.buf[..split]);
    let failure = details::failure(capture, error);
    let _ = write!(
        message,
        "{}",
        crash_text::status(text, saved, partial, failure)
    );
    if !saved {
        let _ = write!(message, "\r\n({} ", text.error);
        let _ = details::write_code(&mut message, error);
        let _ = write!(message, ")");
    }
    let have_file = path.len > 0;
    let instructions = if have_file {
        text.instructions
    } else {
        text.no_file
    };
    // The details: shown in short, copied for Discord as a code block.
    let mut short = Wide::<512>::new();
    let _ = write!(short, "{}", report.short.as_str());
    let mut clip = Wide::<17000>::new();
    let head = core::str::from_utf8(&report.text.buf[..report.head]).unwrap_or("");
    let _ = write!(clip, "```\r\n{head}```");
    // A game folder with non-ASCII letters: one more line, below the details.
    let game = crate::dump::folder();
    let mut warning = Wide::<512>::new();
    if details::needs_simple_path(&game.buf[..game.len]) {
        let _ = write!(warning, "{}", text.simple_path[0]);
        let _ = details::write_short_path(&mut warning, &game.buf[..game.len], 60);
        let _ = write!(warning, "{}", text.simple_path[1]);
    }
    let extra: u16 = if warning.len > 0 { 30 } else { 0 };
    let mut t = Template {
        data: [0; 2048],
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
    // The number of controls below.
    t.word(if extra != 0 { 15 } else { 14 });
    t.word(0);
    t.word(0);
    t.word(380);
    t.word(200 + extra);
    t.word(0);
    t.word(0);
    t.text(text.title);
    // Match IDD_SIMPLE_DLG in the original BugTrap.rc. The white panel is
    // taller to fit our persistent dump location, the crash details and the
    // local-reporting text.
    t.word(8);
    t.text("Tahoma");
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [0, 0, 380, 166 + extra],
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
        [35, 7, 338, 18],
        1030,
        0x82,
        text.intro,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
        [35, 27, 338, 34],
        1031,
        0x82,
        "",
    );
    // The details, selectable: the exception and where it happened.
    t.control(
        WS_CHILD | WS_VISIBLE | (ES_LEFT | ES_MULTILINE | ES_READONLY) as u32,
        [35, 63, 338, 26],
        1040,
        0x81,
        "",
    );
    if extra != 0 {
        t.control(
            WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
            [35, 93, 338, 26],
            1042,
            0x82,
            "",
        );
    }
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [35, 93 + extra, 38, 8],
        1037,
        0x82,
        text.file,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFTNOWORDWRAP | SS_NOPREFIX | SS_ENDELLIPSIS,
        [75, 93 + extra, 298, 8],
        1034,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        [35, 105 + extra, 38, 8],
        1038,
        0x82,
        text.folder,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFTNOWORDWRAP | SS_NOPREFIX | SS_PATHELLIPSIS,
        [75, 105 + extra, 298, 8],
        1035,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
        [35, 119 + extra, 338, 42],
        1036,
        0x82,
        instructions,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | SS_ETCHEDHORZ,
        [0, 165 + extra, 380, 1],
        1033,
        0x82,
        "",
    );
    t.control(
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_DEFPUSHBUTTON | BS_CENTER | BS_VCENTER) as u32,
        [303, 177 + extra, 70, 16],
        IDCANCEL as u16,
        0x80,
        text.close,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
        [7, 177 + extra, 86, 16],
        1013,
        0x80,
        text.discord,
    );
    t.control(
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON | BS_CENTER | BS_VCENTER) as u32,
        [99, 177 + extra, 70, 16],
        1041,
        0x80,
        text.copy,
    );
    let mut data = DialogData {
        message: message.ptr(),
        font: null_mut(),
        icon: null_mut(),
        small_icon: null_mut(),
        folder: folder.ptr(),
        filename: path.buf.as_ptr().add(split),
        have_dump: have_file,
        text,
        details: short.ptr(),
        clip: &clip.buf[..clip.len],
        warning: warning.ptr(),
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
        if have_file {
            let _ = write!(message, "\r\n");
            let _ = message.wide(&path.buf[..path.len]);
        }
        if short.len > 0 {
            let _ = write!(message, "\r\n\r\n");
            let _ = message.wide(&short.buf[..short.len]);
        }
        if warning.len > 0 {
            let _ = write!(message, "\r\n\r\n");
            let _ = message.wide(&warning.buf[..warning.len]);
        }
        let _ = write!(message, "\r\n\r\n{}", text.manual);
        MessageBoxW(
            null_mut(),
            message.ptr(),
            title.ptr(),
            MB_OK | MB_ICONERROR | MB_TASKMODAL,
        );
    }
}
