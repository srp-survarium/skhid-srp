//! The climbs of the stand-in's bodies, counted against what the client's body can climb (A199: a body got from a ledge back up 1.14 m onto
//! the platform it fell from, in 2-16 s, where the live bodies stayed 50 s to 9 min).
//!
//! A body gains height in three ways in the stand-in: it walks up a link of the lattice (`slide_supported`, at most the grid body's step, the
//! client's `cc_step_height` 0.80), the client's sprint lift carries it up a low top (`standin_jump::lift_check`, at most
//! `motor::LIFT_MAX_RISE_M` 1.11), or a jump lands it on a floor higher than the one it left (`standin_jump::fly`). Each is counted by its way and
//! the rise it made (the floor left to the floor reached); a rise over what that way can make in the client is an *over-limit climb* and is
//! kept as a line with where, when and how. Counting only: nothing here changes a decision of a bot or a move of a body.

use mapdata::Vec3;

/// The ways a body gains height, in the order of [`Climbs::counts`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Way {
    /// A walk link of the lattice.
    Walk,
    /// The sprint lift.
    Lift,
    /// A jump that is not a sprint jump (in place, short).
    ShortJump,
    /// A sprint jump.
    SprintJump,
}

impl Way {
    const ALL: [Way; 4] = [Way::Walk, Way::Lift, Way::ShortJump, Way::SprintJump];

    fn name(self) -> &'static str {
        match self {
            Way::Walk => "walk",
            Way::Lift => "lift",
            Way::ShortJump => "short_jump",
            Way::SprintJump => "sprint_jump",
        }
    }

    /// The highest rise the client's body makes this way, m: the step (`startup.cfg` `cc_step_height` 0.80); the sprint lift (1.11); the
    /// plain jump's apex (0.998, the jump law); the sprint jump's reach (1.30, `tallies/owner/jumps.rs` `REACH_M`).
    pub(crate) fn limit_m(self) -> f32 {
        match self {
            Way::Walk => 0.80,
            Way::Lift => 1.11,
            Way::ShortJump => 1.03,
            Way::SprintJump => 1.30,
        }
    }
}

/// A climb is counted from this rise on, m: below it the lattice's own small steps of a slope.
const MIN_RISE_M: f32 = 0.3;

/// Most over-limit climbs kept as lines.
const KEPT: usize = 400;

/// What the bodies of one game climbed.
#[derive(Default)]
pub(crate) struct Climbs {
    /// Body-ticks alive on a floor (the denominator of a rate).
    pub(crate) alive_ticks: u64,
    /// Climbs of [`MIN_RISE_M`] or more, by [`Way`].
    pub(crate) made: [u32; 4],
    /// Of those, the ones over the way's [`Way::limit_m`].
    pub(crate) over: [u32; 4],
    lines: Vec<String>,
}

impl Climbs {
    /// A body of `team` at `place` went from `from` to `to` by `way` at `tick`; `note` says how (the jump's clip, the keys).
    pub(crate) fn note(&mut self, way: Way, tick: u32, place: usize, team: u8, from: Vec3, to: Vec3, note: &str) {
        let rise = to.y - from.y;
        if rise < MIN_RISE_M {
            return;
        }
        let i = Way::ALL.iter().position(|&w| w == way).expect("a way");
        self.made[i] += 1;
        if rise > way.limit_m() + 0.01 {
            self.over[i] += 1;
            if self.lines.len() < KEPT {
                self.lines.push(format!(
                    "climb_over: tick {tick} place {place} team {team} {} rise {rise:.2} limit {:.2} from ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2}) {note}",
                    way.name(),
                    way.limit_m(),
                    from.x,
                    from.y,
                    from.z,
                    to.x,
                    to.y,
                    to.z
                ));
            }
        }
    }

    /// The counts for a cell's log.
    pub(crate) fn line(&self) -> String {
        let mut parts: Vec<String> = Way::ALL
            .iter()
            .enumerate()
            .map(|(i, w)| format!("{} {}/{}", w.name(), self.over[i], self.made[i]))
            .collect();
        let hours = self.alive_ticks as f64 * f64::from(survarium::v026g::match_msgs::TICK_MS) / 3_600_000.0;
        parts.push(format!("body_hours {hours:.2}"));
        format!("climbs (over limit/made): {}", parts.join(" "))
    }

    /// The over-limit climbs kept.
    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rise_over_the_ways_limit_is_kept_and_a_small_one_is_not_counted() {
        let mut c = Climbs::default();
        let at = |y| Vec3::new(1.0, y, 2.0);
        c.note(Way::Walk, 100, 3, 1, at(0.0), at(0.2), "");
        c.note(Way::Walk, 100, 3, 1, at(0.0), at(0.7), "");
        c.note(Way::Lift, 100, 3, 1, at(0.0), at(1.14), "");
        c.note(Way::SprintJump, 100, 3, 1, at(0.0), at(1.14), "");
        assert_eq!(c.made, [1, 1, 0, 1]);
        assert_eq!(c.over, [0, 1, 0, 0]);
        assert_eq!(c.lines().len(), 1);
        assert!(c.lines()[0].contains("lift rise 1.14"), "{}", c.lines()[0]);
        assert!(c.line().contains("lift 1/1") && c.line().contains("walk 0/1"), "{}", c.line());
    }
}
