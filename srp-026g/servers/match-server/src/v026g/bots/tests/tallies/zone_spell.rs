use super::*;

/// A body's stretch of zone damage ([`Game::zone_tick`]): from its first
/// zone hit to [`SPELL_GAP_MS`] with none (a crossing, or a stand in a
/// zone): when it began, the health then, the health the zones took.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ZoneSpell {
    pub(crate) start: u32,
    pub(crate) health: f32,
    pub(crate) lost: f32,
    pub(crate) last: u32,
    /// Its bot's motives over it (`Bot::bench_why`), or'ed.
    pub(crate) why: u16,
}

/// A zone spell ends after this long with no zone hit, ms.
pub(crate) const SPELL_GAP_MS: u32 = 1_000;

/// An enemy hit this long before a zone spell, or in it, marks the spell
/// as in a fight, ms.
pub(crate) const SPELL_FIGHT_MS: u32 = 3_000;

impl ZoneSpell {
    /// The spell as kept when it ends on `body` ([`Game::zone_spells`]).
    pub(crate) fn ended(&self, body: &Body) -> [f32; 5] {
        let fight = body.last_hit != 0 && body.last_hit + SPELL_FIGHT_MS >= self.start;
        [self.lost, body.health.max(0.0), self.health, f32::from(u8::from(fight)), f32::from(self.why)]
    }
}
