// The name of a run's log, parsed: `<kind>.<stamp>[_<n>].log`, the stamp the
// run's start (local time) `YYYY-MM-DD_HH-MM-SS`, `_<n>` (2..99) a further file
// of the same stamp (a second run in the same second, or a run's next part).
// The client patches (the game's own log's name and srp_patches) and the
// updater write these names. Pure: no OS calls, so the host tests include it
// (tests/run_name_tests.rs).

/// Whose log it is: the only words of a name the ZIP repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The engine's `Survarium.<stamp>.log` or `Survarium_<user>.<stamp>.log`
    /// (Documents\Survarium).
    Game,
    /// The client patches' `srp_patches.<stamp>.log` beside the exe.
    Patches,
    /// The updater's `updater_skhid.<stamp>.log` beside it.
    Updater,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Game => "game",
            Kind::Patches => "srp_patches",
            Kind::Updater => "updater_skhid",
        }
    }
}

/// `YYYY-MM-DD_HH-MM-SS` and the optional `_<n>`: 19 to 22 ASCII bytes.
pub const STAMP_MAX: usize = 22;

/// A run's log name: its kind and its stamp (with `_<n>`), digits, `-` and
/// `_` only, so it may be written where a player's file name may not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunName {
    pub kind: Kind,
    stamp: [u8; STAMP_MAX],
    len: usize,
}

impl RunName {
    /// `2026-11-15_14-03-07` or `2026-11-15_14-03-07_2`.
    pub fn stamp(&self) -> &str {
        // Only ASCII digits, '-' and '_' were copied in (parse).
        core::str::from_utf8(&self.stamp[..self.len]).unwrap_or("")
    }
}

fn ascii_eq(wide: &[u16], ascii: &[u8]) -> bool {
    wide.len() == ascii.len()
        && wide
            .iter()
            .zip(ascii)
            .all(|(&w, &a)| w <= 127 && (w as u8).eq_ignore_ascii_case(&a))
}

fn digit(c: u16) -> bool {
    (b'0' as u16..=b'9' as u16).contains(&c)
}

/// The stamp's shape: `DDDD-DD-DD_DD-DD-DD`, then nothing or `_` and a
/// number 2..99 without a leading zero.
fn stamp(text: &[u16]) -> bool {
    const SHAPE: &[u8; 19] = b"dddd-dd-dd_dd-dd-dd";
    if text.len() < SHAPE.len() {
        return false;
    }
    let (stamp, rest) = text.split_at(SHAPE.len());
    let shaped = stamp
        .iter()
        .zip(SHAPE)
        .all(|(&c, &s)| if s == b'd' { digit(c) } else { c == s as u16 });
    let number = match rest {
        [] => true,
        [u, n] => *u == b'_' as u16 && digit(*n) && *n >= b'2' as u16,
        [u, a, b] => *u == b'_' as u16 && digit(*a) && *a != b'0' as u16 && digit(*b),
        _ => false,
    };
    shaped && number
}

/// `name` (a file name, no folder) as a run's log, none when it is not one.
pub fn parse(name: &[u16]) -> Option<RunName> {
    let stem = name
        .len()
        .checked_sub(4)
        .filter(|&n| ascii_eq(&name[n..], b".log"))?;
    let stem = &name[..stem];
    // The stamp has no '.', a user name in Survarium_<user> may.
    let dot = stem.iter().rposition(|&c| c == b'.' as u16)?;
    let (head, text) = (&stem[..dot], &stem[dot + 1..]);
    if !stamp(text) {
        return None;
    }
    let kind = if ascii_eq(head, b"srp_patches") {
        Kind::Patches
    } else if ascii_eq(head, b"updater_skhid") {
        Kind::Updater
    } else if ascii_eq(head, b"Survarium")
        || (head.len() > 10 && ascii_eq(&head[..10], b"Survarium_"))
    {
        Kind::Game
    } else {
        return None;
    };
    let mut run = RunName {
        kind,
        stamp: [0; STAMP_MAX],
        len: text.len(),
    };
    for (slot, &c) in run.stamp.iter_mut().zip(text) {
        *slot = c as u8;
    }
    Some(run)
}
