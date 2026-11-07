//! Patch 116: a banned account's sign-in says why, in srp's words, and
//! when the ban ends. srp's login server tells a ban's reason and end after
//! its code (crates/ban-reason); `on_answer` (the sign-in answer, the
//! network's thread) keeps them; `on_status` (set_status, the game's) writes
//! the status text anew: st_login_menu_status_user_banned, the reason's
//! st_srp_ban_reason_*, then st_srp_login_ban_forever or
//! st_srp_login_ban_ends with the end in the machine's time (`14:10`, a later
//! day `08.11 14:10`) and the time left (`29 min`, `2 h 15 min`, `3 d 4 h`).
//! An answer that tells nothing after the code keeps the game's own text.
//!
//! Presentation only: nothing a match computes.

#![forbid(unsafe_code)]

use ban_reason::{BanEnd, BanReason, BanTold};

use crate::vostok::clock::{FileTime, LocalTime};
use crate::vostok::login::{self, BanStatusText};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// The ban the last sign-in answer told, if it was one srp told.
static TOLD: Lock<Option<Told>> = Lock::new(None);

#[derive(Clone, Copy)]
struct Told {
    reason: BanReason,
    end: Ends,
}

#[derive(Clone, Copy)]
enum Ends {
    Never,
    At(FileTime),
}

pub fn on_answer(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let told = login::ban_told(ctx).map(|BanTold { reason, end }| Told {
        reason,
        end: match end {
            BanEnd::Never => Ends::Never,
            BanEnd::In(seconds) => Ends::At(FileTime::now(thread).after_seconds(seconds)),
        },
    });
    TOLD.with(thread, |kept| *kept = told);
    Flow::Continue
}

pub fn on_status(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(told) = TOLD.with(thread, |kept| *kept) else { return Flow::Continue };
    // The end's local time and today's, and the minutes left (at least 1).
    let end = match told.end {
        Ends::Never => None,
        Ends::At(at) => {
            let now = FileTime::now(thread);
            let (Some(local), Some(today)) = (at.local(thread), now.local(thread)) else { return Flow::Continue };
            Some((local, today, now.seconds_until(at).div_ceil(60).max(1)))
        }
    };
    let Some(mut text) = BanStatusText::at_status_text(ctx) else { return Flow::Continue };

    let mut out = [0u8; 0x200];
    let banned: Text = text.translate(c"st_login_menu_status_user_banned", &mut out).to_bytes().try_into().unwrap_or_default();
    let reason: Text = text.translate(told.reason.key(), &mut out).to_bytes().try_into().unwrap_or_default();
    let ends = match end {
        None => text.translate(c"st_srp_login_ban_forever", &mut out).to_bytes().try_into().unwrap_or_default(),
        Some((local, today, minutes)) => {
            let units = Units {
                minutes: text.translate(c"st_srp_unit_minutes", &mut out).to_bytes().try_into().unwrap_or_default(),
                hours: text.translate(c"st_srp_unit_hours", &mut out).to_bytes().try_into().unwrap_or_default(),
                days: text.translate(c"st_srp_unit_days", &mut out).to_bytes().try_into().unwrap_or_default(),
            };
            ends_line(text.translate(c"st_srp_login_ban_ends", &mut out).to_bytes(), local, today, minutes, &units)
        }
    };
    text.clear();
    // The header ends with a space in some languages ("You are banned. "),
    // in others not ("Вы забанены.").
    text.append(banned.bytes().trim_ascii_end());
    text.append(b" ");
    text.append(reason.bytes());
    text.append(b" ");
    text.append(ends.bytes());
    Flow::Continue
}

/// The dictionary's units, each `%1$s` and its unit.
struct Units {
    minutes: Text,
    hours: Text,
    days: Text,
}

/// `template` (`Ends at %1$s (in %2$s).`) with the end and the time left.
fn ends_line(template: &[u8], at: LocalTime, today: LocalTime, minutes: u32, units: &Units) -> Text {
    let mut when = Text::default();
    if (at.month, at.day) != (today.month, today.day) {
        when.two(at.day).push(b".").two(at.month).push(b" ");
    }
    when.two(at.hour).push(b":").two(at.minute);
    let (days, hours, minutes) = (minutes / (24 * 60), minutes / 60 % 24, minutes % 60);
    let mut left = Text::default();
    let parts = if days > 0 { [(days, &units.days), (hours, &units.hours)] } else { [(hours, &units.hours), (minutes, &units.minutes)] };
    let shown = parts.iter().filter(|(count, _)| *count > 0);
    for (i, (count, unit)) in shown.enumerate() {
        if i > 0 {
            left.push(b" ");
        }
        let mut number = Text::default();
        number.number(*count);
        left.fill(unit.bytes(), &[number.bytes()]);
    }
    let mut line = Text::default();
    line.fill(template, &[when.bytes(), left.bytes()]);
    line
}

/// A short text of ours, cut at its room.
#[derive(Clone)]
struct Text {
    bytes: [u8; 0x100],
    len: usize,
}

impl Default for Text {
    fn default() -> Text {
        Text { bytes: [0; 0x100], len: 0 }
    }
}

impl TryFrom<&[u8]> for Text {
    type Error = ();
    fn try_from(bytes: &[u8]) -> Result<Text, ()> {
        let mut text = Text::default();
        if bytes.len() > text.bytes.len() {
            return Err(());
        }
        text.push(bytes);
        Ok(text)
    }
}

impl Text {
    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn push(&mut self, bytes: &[u8]) -> &mut Text {
        let count = bytes.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + count].copy_from_slice(&bytes[..count]);
        self.len += count;
        self
    }

    fn number(&mut self, value: u32) -> &mut Text {
        let mut digits = [0u8; 10];
        let mut at = digits.len();
        let mut rest = value;
        loop {
            at -= 1;
            digits[at] = b'0' + (rest % 10) as u8;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        self.push(&digits[at..])
    }

    fn two(&mut self, value: u8) -> &mut Text {
        self.push(&[b'0' + value / 10 % 10, b'0' + value % 10])
    }

    /// `template` with `%1$s`, `%2$s`.. its arguments (the dictionary's
    /// placeholders); any other byte as it is.
    fn fill(&mut self, template: &[u8], arguments: &[&[u8]]) -> &mut Text {
        let mut rest = template;
        while let Some((&first, tail)) = rest.split_first() {
            if let [b'%', n @ b'1'..=b'9', b'$', b's', after @ ..] = rest
                && let Some(argument) = arguments.get(usize::from(n - b'1'))
            {
                self.push(argument);
                rest = after;
                continue;
            }
            self.push(&[first]);
            rest = tail;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units() -> Units {
        Units {
            minutes: b"%1$s min".as_slice().try_into().unwrap(),
            hours: b"%1$s h".as_slice().try_into().unwrap(),
            days: b"%1$s d".as_slice().try_into().unwrap(),
        }
    }

    fn line(at: LocalTime, today: LocalTime, minutes: u32) -> String {
        let text = ends_line(b"Ends at %1$s (in %2$s).", at, today, minutes, &units());
        String::from_utf8(text.bytes().to_vec()).unwrap()
    }

    const TODAY: LocalTime = LocalTime { month: 11, day: 7, hour: 13, minute: 41 };

    #[test]
    fn the_end_and_the_time_left() {
        let at = LocalTime { hour: 14, minute: 10, ..TODAY };
        assert_eq!(line(at, TODAY, 29), "Ends at 14:10 (in 29 min).");
        assert_eq!(line(at, TODAY, 60), "Ends at 14:10 (in 1 h).");
        assert_eq!(line(at, TODAY, 135), "Ends at 14:10 (in 2 h 15 min).");
        let later = LocalTime { month: 11, day: 10, hour: 9, minute: 5 };
        assert_eq!(line(later, TODAY, 3 * 24 * 60 + 4 * 60 + 59), "Ends at 10.11 09:05 (in 3 d 4 h).");
        assert_eq!(line(later, TODAY, 2 * 24 * 60), "Ends at 10.11 09:05 (in 2 d).");
    }

    #[test]
    fn a_template_keeps_what_is_no_placeholder() {
        let mut text = Text::default();
        text.fill(b"100% %2$s, %1$s %3$s %", &[b"a", b"b"]);
        assert_eq!(text.bytes(), b"100% b, a %3$s %");
    }
}
