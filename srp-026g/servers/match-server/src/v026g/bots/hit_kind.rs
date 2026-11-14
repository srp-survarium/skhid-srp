//! [Gap 14] Whether a bot's last hit was an enemy's. The damage model's `m_last_hit_time_ms` (`Hurt::last_hit_ms`) moves
//! at any hit that took health: a bullet, a grenade, a trap, a zone, a fall, a mate's (`damage_model::hit_body_part`
//! 0x2E5080, 0x2E5134). The statistics record of the body says more: its `received_damage[kit][inflictor]` grows only for
//! an inflictor of the other team (bullets, grenades, traps; `add_received_damage` 0x2F58E0 from `generate_hit_event`
//! 0x2D9F40), never for a zone or a fall. A player knows he was shot or blasted (the client's hit feedback); the bot
//! reads only its own sum, never who the inflictor was.
//!
//! The rule: between two trusted worlds made at `since` and `time`, a last hit after `since` is an enemy's when the
//! body's sum grew in that interval (or, after it fell back at a death, is over 0 again).

/// One bot's reading of its own worlds' hits ([`HitKind::world`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitKind {
    /// The world before: its time and the body's sum of `received_damage` then.
    last: Option<(u32, f32)>,
    /// The last hit the worlds showed to be an enemy's, the world's ms.
    enemy_hit_ms: Option<u32>,
    /// Worlds whose new last hit was an enemy's, and those whose new last hit was not.
    pub enemy: u32,
    pub other: u32,
}

impl HitKind {
    /// A trusted world made at `time` shows the body's last hit `last_hit` (`m_last_hit_time_ms`, `None`: none) and its
    /// sum of received damage from enemies `received`. Returns, for a new last hit since the world before, whether it was
    /// an enemy's; `None` with no new hit, or for the first world (no interval yet).
    pub fn world(&mut self, time: u32, last_hit: Option<u32>, received: f32) -> Option<bool> {
        let (since, before) = self.last.replace((time, received))?;
        if time <= since {
            self.last = Some((since, before));
            return None;
        }
        let hit = last_hit.filter(|&hit| hit > since)?;
        let grew = if received >= before { received > before } else { received > 0.0 };
        if grew {
            self.enemy_hit_ms = Some(hit);
            self.enemy += 1;
        } else {
            self.other += 1;
        }
        Some(grew)
    }

    /// The last hit an enemy made, as the worlds showed it.
    pub fn enemy_hit_ms(&self) -> Option<u32> {
        self.enemy_hit_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hit in an interval in which the body's received damage grew is an enemy's; one with the sum unchanged (a zone,
    /// a fall, a mate) is not, and leaves the enemy's last hit as it was.
    #[test]
    fn a_hit_is_an_enemys_when_the_received_damage_grew() {
        let mut kind = HitKind::default();
        assert_eq!(kind.world(2_000, None, 0.0), None, "the first world: no interval");
        assert_eq!(kind.world(4_000, Some(3_100), 12.5), Some(true));
        assert_eq!(kind.enemy_hit_ms(), Some(3_100));
        assert_eq!(kind.world(6_000, Some(5_500), 12.5), Some(false), "a zone's hit: the sum unchanged");
        assert_eq!(kind.enemy_hit_ms(), Some(3_100));
        assert_eq!(kind.world(8_000, Some(5_500), 30.0), None, "no new last hit");
        assert_eq!((kind.enemy, kind.other), (1, 1));
    }

    /// The sum falls back at a death: a hit after it is an enemy's when the sum is over 0 again. A world older than the
    /// last read changes nothing.
    #[test]
    fn a_sum_that_fell_back_counts_from_nothing() {
        let mut kind = HitKind::default();
        kind.world(2_000, None, 40.0);
        assert_eq!(kind.world(4_000, Some(3_000), 0.0), Some(false), "a fall after the reset");
        assert_eq!(kind.world(6_000, Some(5_000), 8.0), Some(true));
        assert_eq!(kind.world(5_000, Some(5_900), 9.0), None, "an older world");
        assert_eq!(kind.enemy_hit_ms(), Some(5_000));
    }
}
