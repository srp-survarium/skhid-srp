use super::*;

/// What the owner decided the delay to be, ms (2026-10-26): a train turns the default to it.
pub const SUICIDE_AFTER_OWNER_MS: u32 = 60_000;

/// The last resort of a bot with nothing left to try: it kills itself
/// (`stuck::SelfKill`). Every number is the owner's rule of 2026-10-24..26 or a
/// GUESS where it says so.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UnstickTuning {
    /// A bot with nothing left to try (every exit of its pocket tried, or the
    /// ladder at its last step three times at one spot) kills itself this
    /// long after the run began, ms; none: never. The default is the owner's 60 s ([`SUICIDE_AFTER_OWNER_MS`]).
    pub suicide_after_ms: Option<u32>,
    /// … only with no enemy sensed and no hit taken for this long, ms: a bot in
    /// a fight is left to the enemy, whose kill it is.
    pub suicide_quiet_ms: u32,
    /// … only when the life is this old, ms (spawn protection 6 s, then the
    /// first route), and the match is this old, ms. GUESS.
    pub suicide_life_ms: u32,
    pub suicide_match_ms: u32,
    /// … at most once a bot in this long, ms, and this many a team in a
    /// minute. Owner: one a bot in 3 minutes, two a team a minute.
    pub suicide_bot_gap_ms: u32,
    pub suicide_team_per_min: u32,
    /// … in Team Deathmatch only when the team has more tickets than this
    /// (a suicide costs one). Owner: above 5.
    pub suicide_min_tickets: u32,
    /// The world must show the body dead within this long after the bit's
    /// ticks, ms, else it is sent once more, then never again in the life
    /// (the J0 run's deaths came with the next world, 1-2 s apart).
    pub suicide_wait_ms: u32,
    /// The bit is held this many ticks (the J0 run's, `SUICIDE_TICKS`).
    pub suicide_ticks: u8,
    /// A world the bot has no reckoning of (`BodyTrack::last_meant` none: it
    /// came newer than every tick the bot decided, as in a burst) says how far
    /// the bot's keys meant to move it only by the keys that world shows: a
    /// walk key held means a walk, none means the bot stood. Off,
    /// an unknown way is a walk, and a body at rest is flagged stuck
    /// (`stuck::Stuck::walking`; School 12a83438 2:38.2: four resting bots at
    /// once).
    pub unknown_meant_needs_keys: bool,
    /// A body that held walk keys this long, ms, without getting [`PROGRESS_M`](super::super::stuck::PROGRESS_M) further along
    /// its route is not getting there (it goes round a post it cannot pass: J0 0cec7eab, the notch of the team 0 deck, 95 s):
    /// the leg it fails on is kept clear of for the spot's time and the route is planned again. None: the watch is off (the
    /// held-keys watch only sees a body that stays within half a metre).
    pub progress_ms: Option<u32>,
    /// A world this long after the one before it, ms, ends the stuck check (the watch starts again at the next
    /// world, no note, no recovery, no spot for the team): the bot was not shown the body for that long, so a body
    /// that "did not move" is not known to have been stuck. None: the check goes on (the default: 32 of the 35
    /// stuck notes after a 3 s gap in the J0 matches were bodies really pinned).
    pub fix_gap_ms: Option<u32>,
    /// A bot whose last world is this old, ms, stands still (no walk key, no sprint) until another world comes: its
    /// reckoning is all it has, and with no world a body that the reckoning walks on may be pinned or fallen (J0 0146be4a:
    /// 72 s of sprint into a fence panel on a reckoning frozen at one mid-fall world). None: it walks on.
    pub blind_stop_ms: Option<u32>,
    /// A carrier whose way home has been found shut by its own searches (`Bot::no_way_to`, kept while they go on failing) for this long, ms, lets go of the battery (as a carrier
    /// whose escape from a pocket is given up does, `escape.rs`) and fights from where it stands; the play ends. The bench's tip (the 16 offered maps, batteries and Team
    /// Deathmatch, 16 matches each): Vostok Radar Station batteries had a Collect carrier stand at its no-way goal for 10 s or more in 7 of 16 matches, 5 of them for 30-60 s
    /// (the fallback of the Bring state walks `to(home)` for ever); every other map 0-2 cases, none over 30 s. 20 s is a GUESS between a zone that goes off soon and a
    /// carrier that will not move again. None: it stands as before.
    pub home_no_way_ms: Option<u32>,
}

impl Default for UnstickTuning {
    fn default() -> Self {
        Self {
            suicide_after_ms: Some(SUICIDE_AFTER_OWNER_MS),
            suicide_quiet_ms: 15_000,
            suicide_life_ms: 20_000,
            suicide_match_ms: 120_000,
            suicide_bot_gap_ms: 180_000,
            suicide_team_per_min: 2,
            suicide_min_tickets: 5,
            suicide_wait_ms: 4_000,
            suicide_ticks: 3,
            unknown_meant_needs_keys: true,
            progress_ms: Some(6_000),
            fix_gap_ms: None,
            blind_stop_ms: Some(4_500),
            home_no_way_ms: Some(20_000),
        }
    }
}

impl Check for UnstickTuning {
    fn check(&self) -> Result<(), String> {
        check(
            self.suicide_wait_ms >= 2_000 && (1..=8).contains(&self.suicide_ticks),
            "unstick.suicide_wait_ms, suicide_ticks",
        )?;
        check(
            self.suicide_after_ms.is_none_or(|after| after >= self.suicide_quiet_ms),
            "unstick.suicide_after_ms",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off is no number at all: `null` is none, a number is on (the defaults have the owner's 60 s), and a patch
    /// turns it on or off.
    #[test]
    fn the_self_kill_is_off_with_no_number_and_on_with_one() {
        let parse = |json: &str| serde_json::from_str::<Tuning>(json).unwrap().unstick.suicide_after_ms;
        assert_eq!(Tuning::default().unstick.suicide_after_ms, Some(SUICIDE_AFTER_OWNER_MS));
        assert_eq!(parse("{}"), Some(SUICIDE_AFTER_OWNER_MS));
        assert_eq!(parse(r#"{"unstick":{"suicide_after_ms":null}}"#), None);
        assert_eq!(parse(r#"{"unstick":{"suicide_after_ms":60000}}"#), Some(SUICIDE_AFTER_OWNER_MS));
        // Under the quiet time it would act in a fight: refused.
        let mut tuning = Tuning::default();
        tuning.unstick.suicide_after_ms = Some(1_000);
        assert_eq!(tuning.validate().unwrap_err(), "unstick.suicide_after_ms");
    }
}
