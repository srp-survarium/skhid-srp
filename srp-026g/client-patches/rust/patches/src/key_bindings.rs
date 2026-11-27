//! The Settings' Bindings window (the main menu's SrpKeysWindow): its
//! keyboard shows every key's action, and a key picked there is given to an
//! action as a press of it would give it. Two calls of the main menu's, at
//! `options.callback` (hud_options' hook):
//!
//! - `srp_keys` answers `root.srp_keys(table)`: the key binder's keys, a line
//!   `K<TAB>name<TAB>code<TAB>text` each (its name, the dictionary's key
//!   "kSPACE"; its code; the running language's text, "Space"), then the
//!   options' controls, a line `A<TAB>action<TAB>name` each (the row's key
//!   as the menu has it now, set or not yet applied; "" none);
//! - `srp_bind_key` with "action code" gives that key to that row the stock
//!   way (`OptionsCall::bind_key`: the movie hears `root.set_keybind`, or the
//!   game's conflict message shows) and answers `root.srp_keys` again. A row
//!   or a code the tables do not have, Esc's code, or a key wait already
//!   running gives nothing;
//! - `srp_unbind_key` with "code" takes that key from every row that has it,
//!   as a conflict's accepted reassign does (`OptionsCall::clear_key`), and
//!   answers `root.srp_keys` again.
#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::settings::{BindOutcome, OptionsCall};

pub const KEYS: &CStr = c"srp_keys";
pub const BIND_KEY: &CStr = c"srp_bind_key";
pub const UNBIND_KEY: &CStr = c"srp_unbind_key";

/// The answer's room: 135 keys and 62 rows take some 4 KiB.
const ROOM: usize = 0x2000;

/// Answers a call of the Bindings window's; whether it was one.
pub fn on_call(call: &OptionsCall<'_>, method: &CStr) -> bool {
    if method == BIND_KEY {
        if let Some((action, code)) = call.string_argument(0).and_then(|text| parse_pair(text.to_bytes()))
            && let (Some(row), Some(key)) = (call.control_rows().find(|row| row.action == action), call.binder_key(code))
        {
            // Given: the movie has heard root.set_keybind or the conflict's message; Waiting or NoMenu: nothing
            // changed. Either way the table below is the rows' now.
            let _: BindOutcome = call.bind_key(row, key);
        }
    } else if method == UNBIND_KEY {
        if let Some(key) = call.string_argument(0).and_then(|text| parse_decimal(text.to_bytes())).and_then(|code| call.binder_key(code))
        {
            let _: BindOutcome = call.clear_key(key);
        }
    } else if method != KEYS {
        return false;
    }
    let mut table = Table::new();
    let mut room = [0; 0x200];
    for key in call.binder_keys() {
        let text = call.translate(key.name, &mut room);
        table.line(&[b"K", key.name.to_bytes(), Decimal::of(key.code).bytes(), text.to_bytes()]);
    }
    for row in call.control_rows() {
        table.line(&[b"A", Decimal::of(row.action).bytes(), row.key.to_bytes()]);
    }
    call.answer(c"root.srp_keys", &[table.text()]);
    true
}

/// "action code": two decimal numbers and one space.
fn parse_pair(text: &[u8]) -> Option<(u32, u32)> {
    let space = text.iter().position(|&b| b == b' ')?;
    Some((parse_decimal(&text[..space])?, parse_decimal(&text[space + 1..])?))
}

fn parse_decimal(text: &[u8]) -> Option<u32> {
    if text.is_empty() || text.len() > 9 {
        return None;
    }
    text.iter().try_fold(0u32, |n, &b| b.is_ascii_digit().then(|| n * 10 + u32::from(b - b'0')))
}

/// A number's decimal digits.
struct Decimal {
    digits: [u8; 10],
    start: usize,
}

impl Decimal {
    fn of(mut n: u32) -> Decimal {
        let mut digits = [0; 10];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        Decimal { digits, start }
    }

    fn bytes(&self) -> &[u8] {
        &self.digits[self.start..]
    }
}

/// The answer's text: lines of tab-separated fields, a field's tab, newline
/// or zero byte left out (none of the tables' texts has one), a line that
/// does not fit left out whole.
struct Table {
    bytes: [u8; ROOM],
    len: usize,
}

impl Table {
    fn new() -> Table {
        Table { bytes: [0; ROOM], len: 0 }
    }

    fn line(&mut self, fields: &[&[u8]]) {
        let start = self.len;
        for (index, field) in fields.iter().enumerate() {
            if index > 0 && !self.push(b'\t') {
                self.len = start;
                return;
            }
            for &b in field.iter().filter(|&&b| !matches!(b, b'\t' | b'\n' | 0)) {
                if !self.push(b) {
                    self.len = start;
                    return;
                }
            }
        }
        if !self.push(b'\n') {
            self.len = start;
        }
    }

    /// One byte, leaving room for the zero.
    fn push(&mut self, byte: u8) -> bool {
        if self.len + 1 >= ROOM {
            return false;
        }
        self.bytes[self.len] = byte;
        self.len += 1;
        true
    }

    fn text(&mut self) -> &CStr {
        self.bytes[self.len] = 0;
        CStr::from_bytes_until_nul(&self.bytes[..=self.len]).unwrap_or(c"")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs() {
        assert_eq!(parse_pair(b"57 30"), Some((57, 30)));
        assert_eq!(parse_pair(b"4 345"), Some((4, 345)));
        for bad in [&b""[..], b"57", b"57 ", b" 30", b"5a 30", b"57  30", b"57 30 1", b"1234567890 1"] {
            assert_eq!(parse_pair(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn decimals() {
        assert_eq!(Decimal::of(0).bytes(), b"0");
        assert_eq!(Decimal::of(345).bytes(), b"345");
        assert_eq!(Decimal::of(u32::MAX).bytes(), b"4294967295");
    }

    #[test]
    fn lines() {
        let mut table = Table::new();
        table.line(&[b"K", b"kSPACE", b"57", b"Space"]);
        table.line(&[b"A", b"4", b""]);
        table.line(&[b"K", b"k\tX", b"1", b"a\nb"]);
        assert_eq!(table.text().to_bytes(), b"K\tkSPACE\t57\tSpace\nA\t4\t\nK\tkX\t1\tab\n");
        let mut full = Table::new();
        let long = [b'x'; ROOM];
        full.line(&[b"A", b"1", b"kA"]);
        full.line(&[&long[..]]);
        assert_eq!(full.text().to_bytes(), b"A\t1\tkA\n");
    }
}
