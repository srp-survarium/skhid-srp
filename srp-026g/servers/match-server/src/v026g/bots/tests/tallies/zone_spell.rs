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
    /// Where it was hurt: a floor only a crouched body fits (the floors the exact erosion keeps, `WalkGrid::can_stand`), in any tick of it.
    pub(crate) restored: bool,
    /// A zone that hurt is on no mark of the bots' (`Tactics::hazards` with its margin), in any tick of it.
    pub(crate) unmarked: bool,
    /// The body stood in a zone's shape the tick before the first hit, with no zone there on: the zone came on under it.
    pub(crate) came_on: bool,
    /// Every zone that hurt at the first hit never switches off (no schedule to read), else a switching zone's.
    pub(crate) fixed: bool,
}

/// What a floor is to the damage zones at a tick, for [`ZoneSpell`] ([`Game::zone_now`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ZoneNow {
    pub(crate) restored: bool,
    pub(crate) unmarked: bool,
    pub(crate) fixed: bool,
    /// A zone's shape (no margin) holds the floor.
    pub(crate) in_shape: bool,
    /// A zone that holds it is on.
    pub(crate) any_active: bool,
}

/// A zone spell as the owner topic reads it (`tallies/owner/zone_deaths.rs`): who, how it began and what it took.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ZoneNote {
    pub(crate) team: u8,
    pub(crate) spell: ZoneSpell,
    /// The pool that ran out (0 body, 1 head, 2 infection, 3 pain) when the zones killed; none: the spell ended otherwise (left the zone, or killed by an enemy in it).
    pub(crate) died: Option<u8>,
    /// Hit by an enemy within [`SPELL_FIGHT_MS`] before the spell or in it.
    pub(crate) fight: bool,
}

impl ZoneNote {
    pub(crate) fn of(spell: &ZoneSpell, body: &Body, died: Option<u8>) -> Self {
        let fight = body.last_hit != 0 && body.last_hit + SPELL_FIGHT_MS >= spell.start;
        Self { team: u8::from(body.team != 0), spell: *spell, died, fight }
    }
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
