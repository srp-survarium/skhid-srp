use super::*;

/// An enemy within this of the human stand-in, with a line to him, meets
/// him, m.
pub(crate) const MEET_M: f32 = 25.0;

/// A take met: an enemy met him this long before it, ms.
pub(crate) const MET_BEFORE_MS: u32 = 10_000;

/// A teammate this near at a take supports him, m.
pub(crate) const SUPPORT_M: f32 = 15.0;

/// His death traded: his killer dies this soon after, ms.
pub(crate) const TRADED_MS: u32 = 5_000;

/// The human stand-in (`SRP_BENCH_HUMAN=<place>`, CG-12): a bot the plays
/// take for a human, whose commander gives it Raid on the enemy's container
/// whenever it is a candidate. Read each tick: his takes from their
/// container and from the floor, how many an enemy met (a line to him
/// within [`MEET_M`] in the [`MET_BEFORE_MS`] before), the time from his
/// first theft to the next meeting, his deliveries, his takes with a
/// teammate within [`SUPPORT_M`], and his deaths traded.
pub(crate) struct HumanTally {
    pub(crate) place: Place,
    /// The tick an enemy last met him.
    pub(crate) met_at: Option<u32>,
    pub(crate) steals: u32,
    pub(crate) steals_met: u32,
    pub(crate) takes: u32,
    pub(crate) takes_met: u32,
    pub(crate) supported: u32,
    pub(crate) first_steal: Option<u32>,
    /// From his first theft to the next meeting, ms.
    pub(crate) first_meet_ms: Option<u32>,
    pub(crate) delivered: u32,
    pub(crate) deaths: u32,
    pub(crate) traded: u32,
    /// His deaths not yet traded: (tick, killer).
    pub(crate) open_deaths: Vec<(u32, Place)>,
    pub(crate) kills_seen: usize,
    pub(crate) last_kill: Option<(u32, Place, Place)>,
    pub(crate) carrying: Option<usize>,
}

impl HumanTally {
    pub(crate) fn start() -> Option<Self> {
        let place = bench_settings().var("SRP_BENCH_HUMAN").ok()?.parse().ok()?;
        Some(HumanTally {
            place,
            met_at: None,
            steals: 0,
            steals_met: 0,
            takes: 0,
            takes_met: 0,
            supported: 0,
            first_steal: None,
            first_meet_ms: None,
            delivered: 0,
            deaths: 0,
            traded: 0,
            open_deaths: Vec::new(),
            kills_seen: 0,
            last_kill: None,
            carrying: None,
        })
    }

    /// After the tick's step; `thieves` are the tick's takes from a
    /// container.
    pub(crate) fn tick(&mut self, game: &Game, thieves: &[Place]) {
        let tick = game.tick;
        let Some(me) = game.bodies.get(self.place) else {
            return;
        };
        if me.alive {
            let crouched = me.keys & key::CROUCH != 0;
            let met = game.bodies.iter().any(|b| {
                b.alive
                    && b.team != me.team
                    && b.feet.distance_xz(me.feet) <= MEET_M
                    && super::aimpoint::shows(Game::eye(b), me.feet, crouched, &game.world.sight, || {
                        true
                    })
            });
            if met {
                if let Some(first) = self.first_steal
                    && self.first_meet_ms.is_none()
                    && tick > first
                {
                    self.first_meet_ms = Some(tick - first);
                }
                self.met_at = Some(tick);
            }
        }
        let met_lately = self
            .met_at
            .is_some_and(|at| tick.saturating_sub(at) <= MET_BEFORE_MS);
        let supported = game.bodies.iter().enumerate().any(|(p, b)| {
            p != self.place && b.alive && b.team == me.team && b.feet.distance_xz(me.feet) <= SUPPORT_M
        });
        let stole = thieves.contains(&self.place);
        let took = game.took[self.place] == Some(tick) && !stole;
        if stole {
            self.steals += 1;
            self.steals_met += u32::from(met_lately);
            self.first_steal.get_or_insert(tick);
        }
        if took {
            self.takes += 1;
            self.takes_met += u32::from(met_lately);
        }
        if stole || took {
            self.supported += u32::from(supported);
        }
        self.delivered += u32::from(
            self.carrying.is_some() && me.carrying.is_none() && game.put_in.contains(&self.place),
        );
        self.carrying = me.carrying;
        // The events' world takes the log: what was seen is gone if its
        // last entry is not where it was.
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        for &(at, shooter, victim) in &game.kill_log[self.kills_seen..] {
            if victim == self.place {
                self.deaths += 1;
                self.open_deaths.push((at, shooter));
            } else if let Some(i) = self
                .open_deaths
                .iter()
                .position(|&(died, killer)| killer == victim && at <= died + TRADED_MS)
            {
                self.traded += 1;
                self.open_deaths.remove(i);
            }
        }
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        self.open_deaths.retain(|&(died, _)| tick <= died + TRADED_MS);
    }

    /// The bench's `human:` line.
    pub(crate) fn line(&self) -> String {
        format!(
            "place {} steals {} steals_met {} takes {} takes_met {} supported {} first_meet_ms {} delivered {} deaths {} traded {}",
            self.place,
            self.steals,
            self.steals_met,
            self.takes,
            self.takes_met,
            self.supported,
            self.first_meet_ms.map_or(-1, i64::from),
            self.delivered,
            self.deaths,
            self.traded,
        )
    }
}
