//! Health over the match (world host protocol 8, `Final` trailer [`world_host_proto::Vitals`]): achievements 402
//! "Restore more than N% health in one life" and 414 "Destroy the anomaly having 1% of health" (type 9 = the artefact
//! event, like 407).
//!
//! After every stepped DF the session samples each record slot ([`crate::world::MatchWorld::vitals`]): alive (character
//! +0x74C, the byte the alive switch 0x200308AC1 writes), health in hundredths of a percent (the "health" parameter that
//! 0x200344E70 reads: current +0x8C / max +0x84 of the named node in the list `[[char+0x5F8]+0x210]`) and the slot's
//! `artefact` event row (5). Only changes are kept, keyed by time, so a late input (the world simulates again from L)
//! drops every sample at or after L like the kill log does. [`VitalsLog::finish`] replays them:
//! * regeneration: while the player is living (alive byte set and health above 0) in two consecutive samples, every
//!   rise of health adds to the current life; a death ends the life (the best life is kept); a respawn (from health 0)
//!   adds nothing;
//! * artefact: a sample whose artefact row is higher than the previous one records the health percent (truncated as
//!   0x200344E70) of that sample; the lowest one is kept.

use std::collections::BTreeMap;

use world_host_proto::{Vitals, MAX_PLAYERS};

/// One slot's state after a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sample {
    pub slot: u8,
    pub alive: bool,
    /// health, hundredths of a percent of max (0..=10000)
    pub hp: u16,
    /// the slot's `artefact` event row (sum over the 9 profiles)
    pub artefacts: u16,
}

impl Sample {
    /// alive and with health: a dead character's sample can still carry the alive byte (host runs: respawns from 0 to
    /// full health with the byte set), health 0 is dead
    pub fn living(&self) -> bool {
        self.alive && self.hp > 0
    }
}

/// The changed samples of the match, keyed by (time, slot).
#[derive(Debug, Default)]
pub struct VitalsLog {
    by: BTreeMap<(u32, u8), Sample>,
    last: [Option<Sample>; MAX_PLAYERS],
}

impl VitalsLog {
    /// A late input at `l`: the samples at or after `l` will be taken again.
    pub fn rollback(&mut self, l: u32) {
        let before = self.by.len();
        self.by.retain(|&(t, _), _| t < l);
        if self.by.len() != before {
            self.last = [None; MAX_PLAYERS];
            for s in self.by.values() {
                self.last[usize::from(s.slot)] = Some(*s);
            }
        }
    }

    /// The samples of the step at `t`; only the ones that differ from the slot's previous sample are kept.
    pub fn add(&mut self, t: u32, samples: &[Sample]) {
        for s in samples.iter().filter(|s| usize::from(s.slot) < MAX_PLAYERS) {
            if self.last[usize::from(s.slot)] != Some(*s) {
                self.by.insert((t, s.slot), *s);
                self.last[usize::from(s.slot)] = Some(*s);
            }
        }
    }

    /// Every slot's [`Vitals`] up to `t` (a slot without samples: nothing regenerated, no artefact).
    pub fn finish(&self, t: u32) -> [Vitals; MAX_PLAYERS] {
        let mut out = [Vitals { regen_best: 0, artefact_hp_min: 0xFF }; MAX_PLAYERS];
        let mut prev: [Option<Sample>; MAX_PLAYERS] = [None; MAX_PLAYERS];
        let mut life = [0u32; MAX_PLAYERS];
        let mut best = [0u32; MAX_PLAYERS];
        for (_, s) in self.by.range(..=(t, u8::MAX)) {
            let i = usize::from(s.slot);
            if let Some(p) = prev[i] {
                if p.living() && s.living() && s.hp > p.hp {
                    life[i] += u32::from(s.hp - p.hp);
                }
                if p.living() && !s.living() {
                    life[i] = 0;
                }
                if s.artefacts > p.artefacts {
                    out[i].artefact_hp_min = out[i].artefact_hp_min.min((s.hp / 100).min(100) as u8);
                }
            }
            best[i] = best[i].max(life[i]);
            prev[i] = Some(*s);
        }
        for i in 0..MAX_PLAYERS {
            out[i].regen_best = (best[i] / 100).min(u32::from(u16::MAX)) as u16;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(slot: u8, alive: bool, hp: u16, artefacts: u16) -> Sample {
        Sample { slot, alive, hp, artefacts }
    }

    #[test]
    fn regeneration_counts_per_life_and_a_respawn_adds_nothing() {
        let mut v = VitalsLog::default();
        v.add(0, &[s(0, true, 10000, 0)]);
        v.add(50, &[s(0, true, 3000, 0)]); // hit
        v.add(100, &[s(0, true, 8000, 0)]); // medkit +50 %
        v.add(150, &[s(0, true, 2000, 0)]);
        v.add(200, &[s(0, true, 9000, 0)]); // +70 %: 120 % this life
        v.add(250, &[s(0, false, 0, 0)]); // dead
        v.add(300, &[s(0, true, 10000, 0)]); // respawn: nothing
        v.add(350, &[s(0, true, 5000, 0)]);
        v.add(400, &[s(0, true, 6050, 0)]); // +10.5 %
        let f = v.finish(u32::MAX);
        assert_eq!(f[0].regen_best, 120);
        assert_eq!(f[1].regen_best, 0);
        assert_eq!(f[0].artefact_hp_min, 0xFF);
    }

    /// The alive byte can stay set on a dead character: health 0 still ends the life and the respawn adds nothing.
    #[test]
    fn health_zero_is_a_death_even_with_the_alive_byte() {
        let mut v = VitalsLog::default();
        v.add(0, &[s(1, true, 4000, 0)]);
        v.add(50, &[s(1, true, 6000, 0)]); // +20 %
        v.add(100, &[s(1, true, 0, 0)]); // dead, byte still set
        v.add(150, &[s(1, true, 10000, 0)]); // respawn
        assert_eq!(v.finish(u32::MAX)[1].regen_best, 20);
    }

    #[test]
    fn an_artefact_records_the_health_of_its_step_and_a_rollback_retakes_the_samples() {
        let mut v = VitalsLog::default();
        v.add(0, &[s(2, true, 10000, 0)]);
        v.add(50, &[s(2, true, 199, 0)]);
        v.add(100, &[s(2, true, 199, 1)]); // artefact at 1.99 %: 1 %
        assert_eq!(v.finish(u32::MAX)[2].artefact_hp_min, 1);
        // a late input at 100: the artefact step is simulated again, now at 50 % health
        v.rollback(100);
        v.add(100, &[s(2, true, 5000, 1)]);
        let f = v.finish(u32::MAX);
        assert_eq!(f[2].artefact_hp_min, 50);
        assert_eq!(f[2].regen_best, 48, "199 -> 5000 hundredths while alive");
        // unchanged samples are not stored
        let n = v.by.len();
        v.add(150, &[s(2, true, 5000, 1)]);
        assert_eq!(v.by.len(), n);
    }
}
