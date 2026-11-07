//! The sign-in's refusals (patch 116): the answer the login server sends,
//! at `login.other_answer` (on_sign_in_answer_received 0x37FB10, for every
//! answer but 08h and 25h), and the login screen's status text a ban's
//! answer becomes, at `login_menu.status_text` (login_menu::set_status
//! 0x21EEA0). docs/notes/login_chat.md section 4.4.

use core::ffi::CStr;
use core::marker::PhantomData;
use core::ops::RangeInclusive;

use super::context::Context;
use crate::raw;

/// The answer's buffer: one read of at most 52h bytes (0x38010D).
const ANSWER_SIZE: usize = 0x52;
/// The ban codes (12h..1Fh, a reason each).
const BAN_CODES: RangeInclusive<u8> = 0x12..=0x1F;
/// Their statuses in set_status: the code - 6 (the table 0x21F2A4).
const BAN_STATUSES: RangeInclusive<u32> = 0x0C..=0x19;

/// A ban's time left, in seconds, as srp's login server sends it after a
/// ban's code (u32 LE); the stock game reads the code alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BanLeft(pub u32);

/// The answer at `login.other_answer`: a ban that ends, with its time left;
/// None for any other answer, a ban for good, or one with no time.
pub fn ban_left(ctx: &Context<'_>) -> Option<BanLeft> {
    use raw::site::login_other_answer as site;
    let mut answer = [0u8; ANSWER_SIZE];
    // SAFETY: the context is this site's: the login client, alive while its
    // handler runs, whose answer buffer holds the 52h bytes of the read, and
    // the handler's frame with the bytes read (not 0: 0x37FB3D).
    let read = unsafe {
        let (client, frame) = (site::client(ctx)?, site::frame(ctx)?);
        let first = client.answer();
        for (i, byte) in answer.iter_mut().enumerate() {
            *byte = first.add(i).read();
        }
        *frame.transferred() as usize
    };
    parse_ban_left(&answer[..read.min(ANSWER_SIZE)])
}

/// A ban's answer: its code, then the time left.
fn parse_ban_left(answer: &[u8]) -> Option<BanLeft> {
    match answer {
        [code, a, b, c, d, ..] if BAN_CODES.contains(code) => Some(BanLeft(u32::from_le_bytes([*a, *b, *c, *d]))),
        _ => None,
    }
}

/// The login screen's status text while it is a ban's, at
/// `login_menu.status_text`: st_login_menu_status_user_banned and the
/// reason, built, not yet shown.
pub struct BanStatusText<'c> {
    menu: raw::LoginMenu,
    frame: raw::LoginStatusFrame,
    context: PhantomData<&'c mut Context<'c>>,
}

impl<'c> BanStatusText<'c> {
    pub fn at_status_text(ctx: &'c mut Context<'_>) -> Option<BanStatusText<'c>> {
        use raw::site::login_menu_status_text as site;
        // SAFETY: the context is this site's: the login menu, alive while
        // set_status runs on the game's thread, and its frame.
        let (menu, frame) = unsafe { (site::menu(&*ctx)?, site::frame(&*ctx)?) };
        // SAFETY: as above; set_status wrote the status at 0x21EEB0.
        let status = unsafe { *menu.status() };
        BAN_STATUSES.contains(&status).then_some(BanStatusText { menu, frame, context: PhantomData })
    }

    /// A dictionary text of the running language (text_translator::
    /// translate_text 0x1FF0F0 of the menu's game; the key itself when the
    /// dictionary has none) into `out`; empty without a game.
    pub fn translate<'o>(&self, key: &CStr, out: &'o mut [u8; 0x200]) -> &'o CStr {
        out[0] = 0;
        // SAFETY: the menu's game, alive for the hook; translate_text copies
        // a text shorter than 200h bytes with its zero into `out` (0x1FF15D)
        // and leaves it alone otherwise.
        unsafe {
            if let Some(game) = self.menu.game() {
                raw::engine_translate_text(Some(game.text_translator()), key.as_ptr().cast(), out.as_mut_ptr());
            }
        }
        out[0x1FF] = 0;
        CStr::from_bytes_until_nul(out).unwrap_or(c"")
    }

    /// `text` after the status text, as much of it as its buffer holds, the
    /// zero after it.
    pub fn append(&mut self, text: &[u8]) {
        // SAFETY: the frame's text {cursor, end}: the cursor at the text's
        // zero (0x21F3B4), inside the 200h bytes that end at `end`; set_status
        // shows the text from its start (0x21F222) after this site.
        unsafe {
            let (cursor, end) = (self.frame.cursor(), self.frame.end());
            let room = (end as usize).saturating_sub(cursor as usize + 1);
            let count = text.len().min(room);
            for (i, byte) in text[..count].iter().enumerate() {
                cursor.add(i).write(*byte);
            }
            cursor.add(count).write(0);
            self.frame.set_cursor(cursor.add(count));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ban_with_its_time_left() {
        assert_eq!(parse_ban_left(&[0x1C, 0x08, 0x07, 0, 0]), Some(BanLeft(0x708)));
        assert_eq!(parse_ban_left(&[0x12, 1, 0, 0, 0, 9]), Some(BanLeft(1)));
        // For good, the stock answer: the code alone.
        assert_eq!(parse_ban_left(&[0x1E]), None);
        // Not a ban, or cut short.
        assert_eq!(parse_ban_left(&[0x0A, 1, 0, 0, 0]), None);
        assert_eq!(parse_ban_left(&[0x20, 1, 0, 0, 0]), None);
        assert_eq!(parse_ban_left(&[0x1C, 1, 0, 0]), None);
    }
}
