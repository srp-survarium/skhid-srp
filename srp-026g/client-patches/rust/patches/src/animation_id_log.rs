//! Patch 95: a diagnostic of the animation ids the registry does not have
//! (docs/notes/animation-ids.md).
//!
//! The rev47 crashes at 0x1F18C8 read the registry's ids' vector at 0FFFFh:
//! a clip pair the lookup had no entry for. Which pair, and why it was
//! never registered, is not known from the dumps (they hold no heap). Each
//! distinct missing pair is logged once, at most `LOGGED` of them a run: the
//! clips' names, the registry's size, the return address of the
//! resolver and the stack above it. The game goes on as it did (the lookup still returns 0FFFFh).

#![forbid(unsafe_code)]

use crate::vostok::animation_id::{Clip, MissingId, Registry};
use crate::vostok::log::{Piece::*, line};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// The most distinct pairs logged in a run: a few name the cause; a
/// registry gone wrong as a whole would fill the log every frame.
const LOGGED: usize = 8;

/// The pairs logged so far, by their resources' addresses.
struct Logged {
    pairs: [Option<(u32, u32)>; LOGGED],
}

impl Logged {
    /// True the first time `pair` is seen while there is room.
    fn first_time(&mut self, pair: (u32, u32)) -> bool {
        if self.pairs.contains(&Some(pair)) {
            return false;
        }
        match self.pairs.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => {
                *slot = Some(pair);
                true
            }
            None => false,
        }
    }
}

/// Every thread that resolves an animation may reach it.
static LOGGED_PAIRS: Lock<Logged> = Lock::new(Logged { pairs: [None; LOGGED] });

/// The lookup is about to return 0FFFFh (`animations.id_missing`).
pub fn on_id_missing(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(miss) = MissingId::at_site(ctx) {
        let pair = (miss.first.address, miss.second.map_or(miss.first.address, |second| second.address));
        if LOGGED_PAIRS.with(miss.thread, |logged| logged.first_time(pair)) {
            report(&miss);
        }
    }
    Flow::Continue
}

fn report(miss: &MissingId<'_>) {
    clip_line("animation_id: no id for ", &miss.first);
    if let Some(second) = &miss.second {
        clip_line("animation_id:   paired with ", second);
    }
    let caller = miss.caller.unwrap_or(0);
    match miss.registry {
        Some(Registry::Numbered { ids }) => {
            line(&[Text("animation_id:   registry of "), Dec(ids), Text(" ids; asked from "), Hex(caller)])
        }
        Some(Registry::Unnumbered) => line(&[Text("animation_id:   registry not yet numbered; asked from "), Hex(caller)]),
        None => line(&[Text("animation_id:   no registry; asked from "), Hex(caller)]),
    }
    if let Some(above) = miss.above {
        let mut pieces = [Text(""); 13];
        pieces[0] = Text("animation_id:   stack above:");
        for (i, word) in above.iter().enumerate() {
            pieces[1 + 2 * i] = Text(" ");
            pieces[2 + 2 * i] = Hex(*word);
        }
        line(&pieces);
    }
}

fn clip_line(lead: &str, clip: &Clip<'_>) {
    let name = clip.name.and_then(|name| name.to_str().ok()).unwrap_or("(no name)");
    line(&[Text(lead), Hex(clip.address), Text(" "), Text(name)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pair_once_up_to_the_cap() {
        let mut logged = Logged { pairs: [None; LOGGED] };
        assert!(logged.first_time((1, 1)));
        assert!(!logged.first_time((1, 1)));
        assert!(logged.first_time((1, 2)));
        for i in 3..=LOGGED as u32 {
            assert!(logged.first_time((i, i)));
        }
        assert!(!logged.first_time((100, 100)));
        assert!(!logged.first_time((1, 2)));
    }
}
