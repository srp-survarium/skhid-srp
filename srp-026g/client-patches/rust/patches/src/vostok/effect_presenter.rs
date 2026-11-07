//! What a game-effect presenter's search found for an event's effect, read
//! once from the register the native code goes on with. The code after the
//! search takes its result for an entry even when it holds none. And whether
//! a particle presenter has room for a play's entry: its list is six inline
//! entries, and the push past them asserts.
use super::{Context, Reg};
use crate::raw;

/// The presenter's entry for the event's effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Search {
    /// It plays the effect.
    Held,
    /// It holds no entry of it: a clear (base_player::remove 0x2DA81C, the
    /// player's presenter) forgot it while the effect played on.
    Missing,
}

/// A searched index: the search starts it at -1.
fn index(raw: u32) -> Search {
    if raw == u32::MAX { Search::Missing } else { Search::Held }
}

/// A searched entry's address: the search starts it at 0.
fn entry(raw: u32) -> Search {
    if raw == 0 { Search::Missing } else { Search::Held }
}

/// `effect_presenter.sound_stop`: EDI the index.
pub fn sound_stop(ctx: &Context<'_>) -> Search {
    index(ctx.reg(Reg::Edi))
}

/// `effect_presenter.sound_update`: ESI the entry.
pub fn sound_update(ctx: &Context<'_>) -> Search {
    entry(ctx.reg(Reg::Esi))
}

/// `effect_presenter.particle_stop`: EAX the index.
pub fn particle_stop(ctx: &Context<'_>) -> Search {
    index(ctx.reg(Reg::Eax))
}

/// A particle presenter's list at a play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Room {
    /// It has room for the play's entry.
    Free,
    /// Its end is its capacity end (six entries): push_back 0x2ACC20 would
    /// assert "buffer overflow". `presenter` its address, for the log.
    Full { presenter: u32 },
}

/// From the list's end and capacity end (addresses).
fn room(end: u32, capacity: u32, presenter: u32) -> Room {
    if end >= capacity { Room::Full { presenter } } else { Room::Free }
}

/// `effect_presenter.particle_play`: EBX the presenter, its list read once.
pub fn particle_play(ctx: &Context<'_>) -> Room {
    // SAFETY: the site's presenter, alive for its on_event; its list's two
    // pointers are only read.
    unsafe {
        let Some(presenter) = raw::site::effect_presenter_particle_play::presenter(ctx) else { return Room::Free };
        let end = presenter.entries_end().map_or(0, |end| end.address());
        let capacity = presenter.entries_capacity().map_or(0, |capacity| capacity.address());
        room(end, capacity, presenter.address())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_searches_start_values_are_missing() {
        assert_eq!(index(u32::MAX), Search::Missing);
        assert_eq!(entry(0), Search::Missing);
        for raw in [0, 1, 15, 0x7FFF_FFFF] {
            assert_eq!(index(raw), Search::Held);
        }
        for raw in [1, 0x00A9_9474, u32::MAX] {
            assert_eq!(entry(raw), Search::Held);
        }
    }

    #[test]
    fn only_a_list_at_its_capacity_end_is_full() {
        // the third-person presenter of place 16 in the crash: storage 0xA9AD3C, six entries
        let (begin, capacity) = (0x00A9_AD3C, 0x00A9_AD3C + 6 * 0x18);
        for n in 0..6 {
            assert_eq!(room(begin + n * 0x18, capacity, 0x00A9_AD24), Room::Free);
        }
        assert_eq!(room(capacity, capacity, 0x00A9_AD24), Room::Full { presenter: 0x00A9_AD24 });
    }
}
