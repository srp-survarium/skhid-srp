//! What a game-effect presenter's search found for an event's effect, read
//! once from the register the native code goes on with. The code after the
//! search takes its result for an entry even when it holds none.
use super::{Context, Reg};

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
}
