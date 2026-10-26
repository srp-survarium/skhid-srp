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
    /// long after the run began, ms; none: never (the default until a train
    /// turns it on). Owner: 60 s ([`SUICIDE_AFTER_OWNER_MS`]).
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
}

impl Default for UnstickTuning {
    fn default() -> Self {
        Self {
            suicide_after_ms: None,
            suicide_quiet_ms: 15_000,
            suicide_life_ms: 20_000,
            suicide_match_ms: 120_000,
            suicide_bot_gap_ms: 180_000,
            suicide_team_per_min: 2,
            suicide_min_tickets: 5,
            suicide_wait_ms: 4_000,
            suicide_ticks: 3,
        }
    }
}

pub(super) fn validate_unstick(unstick: &UnstickTuning) -> Result<(), String> {
    check(
        unstick.suicide_wait_ms >= 2_000 && (1..=8).contains(&unstick.suicide_ticks),
        "unstick.suicide_wait_ms, suicide_ticks",
    )?;
    check(
        unstick.suicide_after_ms.is_none_or(|after| after >= unstick.suicide_quiet_ms),
        "unstick.suicide_after_ms",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Off is no number at all: absent or `null` is none, a number is on, and a patch turns it on or off.
    #[test]
    fn the_self_kill_is_off_with_no_number_and_on_with_one() {
        let parse = |json: &str| serde_json::from_str::<Tuning>(json).unwrap().unstick.suicide_after_ms;
        assert_eq!(Tuning::default().unstick.suicide_after_ms, None);
        assert_eq!(parse("{}"), None);
        assert_eq!(parse(r#"{"unstick":{"suicide_after_ms":null}}"#), None);
        assert_eq!(parse(r#"{"unstick":{"suicide_after_ms":60000}}"#), Some(SUICIDE_AFTER_OWNER_MS));
        // Under the quiet time it would act in a fight: refused.
        let mut tuning = Tuning::default();
        tuning.unstick.suicide_after_ms = Some(1_000);
        assert_eq!(tuning.validate().unwrap_err(), "unstick.suicide_after_ms");
    }
}
