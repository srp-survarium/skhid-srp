//! UTF-16 / ANSI helpers. ANSI code page = 1252 (GetACP answers 1252).

pub unsafe fn wlen(p: u64) -> usize {
    let mut n = 0;
    unsafe {
        while *((p as *const u16).add(n)) != 0 {
            n += 1;
        }
    }
    n
}

pub unsafe fn wslice<'a>(p: u64, n: i64) -> &'a [u16] {
    if p == 0 {
        return &[];
    }
    let len = if n < 0 { unsafe { wlen(p) } } else { n as usize };
    unsafe { std::slice::from_raw_parts(p as *const u16, len) }
}

pub unsafe fn wstr(p: u64) -> String {
    String::from_utf16_lossy(unsafe { wslice(p, -1) })
}

pub unsafe fn cstr(p: u64) -> String {
    if p == 0 {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p as *const libc::c_char) };
    s.to_bytes().iter().map(|&b| cp1252_to_char(b)).collect()
}

pub unsafe fn cbytes<'a>(p: u64, n: i64) -> &'a [u8] {
    if p == 0 {
        return &[];
    }
    let len = if n < 0 { unsafe { libc::strlen(p as *const libc::c_char) } } else { n as usize };
    unsafe { std::slice::from_raw_parts(p as *const u8, len) }
}

const CP1252_HI: [u16; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F,
    0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

pub fn cp1252_to_u16(b: u8) -> u16 {
    if (0x80..0xA0).contains(&b) { CP1252_HI[(b - 0x80) as usize] } else { b as u16 }
}
pub fn cp1252_to_char(b: u8) -> char {
    char::from_u32(cp1252_to_u16(b) as u32).unwrap_or('?')
}
pub fn u16_to_cp1252(c: u16) -> Option<u8> {
    if c < 0x80 || (0xA0..0x100).contains(&c) {
        return Some(c as u8);
    }
    CP1252_HI.iter().position(|&x| x == c).map(|i| 0x80 + i as u8)
}

pub fn is_utf8_cp(cp: u32) -> bool {
    cp == 65001
}

/// Bytes -> UTF-16 for a code page.
pub fn mb_to_wide(cp: u32, src: &[u8]) -> Vec<u16> {
    if is_utf8_cp(cp) {
        String::from_utf8_lossy(src).encode_utf16().collect()
    } else {
        src.iter().map(|&b| cp1252_to_u16(b)).collect()
    }
}

/// UTF-16 -> bytes for a code page. Returns (bytes, used_default).
pub fn wide_to_mb(cp: u32, src: &[u16]) -> (Vec<u8>, bool) {
    if is_utf8_cp(cp) {
        (String::from_utf16_lossy(src).into_bytes(), false)
    } else {
        let mut used = false;
        let v = src
            .iter()
            .map(|&c| {
                u16_to_cp1252(c).unwrap_or_else(|| {
                    used = true;
                    b'?'
                })
            })
            .collect();
        (v, used)
    }
}

/// Write a NUL-terminated UTF-16 string into buf (cap in u16 units). Returns chars written (no NUL),
/// or the required size (with NUL) when it does not fit.
pub unsafe fn put_wstr(buf: u64, cap: usize, s: &str) -> (usize, bool) {
    let w: Vec<u16> = s.encode_utf16().collect();
    if buf == 0 || w.len() + 1 > cap {
        return (w.len() + 1, false);
    }
    unsafe {
        std::ptr::copy_nonoverlapping(w.as_ptr(), buf as *mut u16, w.len());
        *((buf as *mut u16).add(w.len())) = 0;
    }
    (w.len(), true)
}

pub unsafe fn put_cstr(buf: u64, cap: usize, s: &str) -> (usize, bool) {
    let (b, _) = wide_to_mb(1252, &s.encode_utf16().collect::<Vec<_>>());
    if buf == 0 || b.len() + 1 > cap {
        return (b.len() + 1, false);
    }
    unsafe {
        std::ptr::copy_nonoverlapping(b.as_ptr(), buf as *mut u8, b.len());
        *((buf as *mut u8).add(b.len())) = 0;
    }
    (b.len(), true)
}

/// Leak a NUL-terminated UTF-16 copy.
/// Copy into the low heap (exe-visible memory must stay below 2^44).
pub fn leak_wstr(s: &str) -> u64 {
    let mut w: Vec<u16> = s.encode_utf16().collect();
    w.push(0);
    let p = unsafe { crate::mem::heap_alloc(w.len() * 2, false) };
    unsafe { std::ptr::copy_nonoverlapping(w.as_ptr(), p as *mut u16, w.len()) };
    p as u64
}
pub fn leak_cstr(s: &str) -> u64 {
    let (mut b, _) = wide_to_mb(1252, &s.encode_utf16().collect::<Vec<_>>());
    b.push(0);
    let p = unsafe { crate::mem::heap_alloc(b.len(), false) };
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), p as *mut u8, b.len()) };
    p as u64
}

/// CT_CTYPE1 classification for one UTF-16 unit (Latin-1 range, "C" locale-like).
pub fn ctype1(c: u16) -> u16 {
    const UPPER: u16 = 1;
    const LOWER: u16 = 2;
    const DIGIT: u16 = 4;
    const SPACE: u16 = 8;
    const PUNCT: u16 = 0x10;
    const CNTRL: u16 = 0x20;
    const BLANK: u16 = 0x40;
    const XDIGIT: u16 = 0x80;
    const ALPHA: u16 = 0x100;
    const DEFINED: u16 = 0x200;
    let Some(ch) = char::from_u32(c as u32) else { return 0 };
    let mut t = DEFINED;
    if ch.is_uppercase() {
        t |= UPPER | ALPHA;
    }
    if ch.is_lowercase() {
        t |= LOWER | ALPHA;
    }
    if ch.is_alphabetic() {
        t |= ALPHA;
    }
    if ch.is_ascii_digit() {
        t |= DIGIT;
    }
    if ch.is_ascii_hexdigit() {
        t |= XDIGIT;
    }
    if ch == ' ' || ch == '\t' {
        t |= BLANK;
    }
    if ch.is_whitespace() {
        t |= SPACE;
    }
    if ch.is_control() {
        t |= CNTRL;
    }
    if ch.is_ascii_punctuation() || (!ch.is_alphanumeric() && !ch.is_whitespace() && !ch.is_control() && (c as u32) > 0x7f) {
        t |= PUNCT;
    }
    t
}

pub fn wlower(c: u16) -> u16 {
    char::from_u32(c as u32).and_then(|ch| ch.to_lowercase().next()).map(|x| x as u32).filter(|&x| x <= 0xFFFF).unwrap_or(c as u32) as u16
}
pub fn wupper(c: u16) -> u16 {
    char::from_u32(c as u32).and_then(|ch| ch.to_uppercase().next()).map(|x| x as u32).filter(|&x| x <= 0xFFFF).unwrap_or(c as u32) as u16
}
