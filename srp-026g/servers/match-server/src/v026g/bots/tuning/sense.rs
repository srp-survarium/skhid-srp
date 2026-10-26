use super::*;

/// How far sounds are heard: the flat ranges (the default) or the game's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HearingRanges {
    /// The ranges of `SenseTuning` as they are, by the ground distance; a
    /// crouched enemy is never heard.
    Flat,
    /// The game's (H1b, `sense.rs`): a distance in 3D (a sound is not
    /// damped by a floor, LIKELY: the sound module's emit path has no ray,
    /// 0x5CFF0); an enemy moving crouched is heard within `crouch_step_m`
    /// (the walking step's curve at 0.4 x the volume: crouch-steps-2, live
    /// since revision 31; the ratio PROVEN, so the range follows from the
    /// walking `step_m`, which is the one GUESS: 2.3 m at 6, 4.5 at 8, 5.5
    /// at 10); no range past the sound's cull in the data, a step or a
    /// sprint 17 m (`steps.spl`) and a reload 9 m (`weapons_reloading.spl`),
    /// whatever the hearing cheat multiplies.
    Game { crouch_step_m: f32 },
}

/// What a bot sees and hears (`player_bots_behaviour.md` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SenseTuning {
    /// The view's full width, degrees.
    pub view_deg: f32,
    /// An enemy this near (metres, ground distance) with some part in a
    /// clear line is seen whichever way the bot faces, as a player feels a
    /// body he bumps and hears a man at arm's length (0: off). Match
    /// 0x6da44bbb at 662-677 s: two bots 0.8 m apart for 15 s, each mostly
    /// outside the other's 90 degrees, known to each other only as heard.
    /// GUESS: a long step past arm's length.
    pub touch_m: f32,
    /// Farther than this nothing is seen, metres.
    pub sight_m: f32,
    /// The noticing delay, ms: near and far (it grows with distance up to
    /// `notice_far_m`), and the share of it left for an enemy who moves or
    /// fires.
    pub notice_near_ms: u32,
    pub notice_far_ms: u32,
    pub notice_far_m: f32,
    pub notice_moving: f32,
    /// A sight that begins through bushes the map draws and gives no collision
    /// object (`foliage.rs`) is noticed this much later for each metre of bush on
    /// the line to the chest: the delay times `1 + bush_notice * metres`, at most
    /// `foliage_cap` times (0: bushes hide nobody, as it was; the default until a train
    /// turns it on, 0.5 was the benched strength). A sight that goes on
    /// is not delayed again. GUESS: a player cannot see through the leaves, the
    /// game has no rule for it, and nothing in the recordings measures the delay
    /// (worlds 5 s apart); strong for a bush, the owner tunes it.
    pub bush_notice: f32,
    pub foliage_cap: f32,
    /// Hearing ranges, metres.
    pub shot_m: f32,
    pub sprint_m: f32,
    pub reload_m: f32,
    pub use_m: f32,
    /// Footsteps: an enemy walking upright (faster than `step_speed`, m/s,
    /// not crouched) is heard within this, metres (0: never; the view has
    /// no walking sound, only a sprint's). Audit issue 4: in match
    /// 0x4da3b51d a bot stood 1-4 m from an enemy for 8 s and knew him only
    /// by an old sound. GUESS: half the sprint's range; the client's
    /// footstep sound ranges are not read. Bench, tiers 9-10: enemies
    /// within reach engaged within 1 s rose 8.6 ± 2.5 points a seed.
    pub step_m: f32,
    pub step_speed: f32,
    /// Flat ranges (the default) or the game's ([`HearingRanges`]).
    pub ranges: HearingRanges,
    /// The sprint sound follows the body (H1a). `None` (the default, off): a
    /// bot or player holding SPRINT and FORWARD makes the sprint sound, as it
    /// was. `Some(m/s)`: only when the body also moves that fast, so a body
    /// the client keeps out of the sprint state (`base_player::can_sprint`
    /// 0x2DBF40: a battery in hand, overburdened, low stamina) is heard as
    /// the walking step it plays (3.5 m/s walk, 5.0 sprint, `kin`; the gap
    /// between them is the threshold). PROVEN rule, the threshold a GUESS in
    /// 4.25 (the middle) to 4.6; a body slowed by its kit can sprint under it.
    pub sprint_body_mps: Option<f32>,
    /// A heard position's error per metre of distance.
    pub heard_error: f32,
    /// What one bot senses its teammates know this much later, ms.
    pub callout_ms: u32,
    /// An enemy not sensed for this long is a fading last known position.
    pub forget_ms: u32,
    /// A last known position is dropped after this, ms.
    pub drop_ms: u32,
    /// With the battery cheat off (`CheatTuning::batteries`): a battery,
    /// or a battery spawner's spot, is seen within this, metres, in the
    /// view and in a clear line; within `battery_near_m` (on the ground)
    /// at once. GUESS: a lying battery is small but lit.
    pub battery_sight_m: f32,
    pub battery_near_m: f32,
    /// Rays a think may cast at batteries and spawners (nearest first).
    pub battery_rays: u32,
    /// A battery the team knows is put right where it is seen lying
    /// farther than this from the spot known, metres (`sense_batteries`).
    pub battery_moved_m: f32,
    /// Timed batteries: a station's spawner seen this long ago is worth a
    /// look again, ms.
    pub spawner_recheck_ms: u32,
    /// A sight no look has tested goes on this long, ms on the information
    /// clock (`know::SIGHT_UNTESTED_MS`): the longest think gap and a tick.
    pub sight_untested_ms: u32,
    /// A sight whose look failed goes on this long, ms on the information
    /// clock, from that look (`know::SIGHT_GRACE_MS`): not below 150, the
    /// look gap and a tick. The human measurement, 250 (range 150 to
    /// 400); the bench runs 150, 250 and 400. It covers a late input's snap
    /// of a reckoned player (a lateness is about 0.45 of the ping; the
    /// relay discards one later than its reach, 225 to 425 ms). A flip from
    /// the model's error between world fixes (0.21 m median at 1 s, 0.55 m
    /// at 3 s, p99 2.5 to 4.6 m; a visible line read as blocked in 6 to 18 %
    /// of cases) is separate from lateness snaps, independent of ping, and
    /// belongs to grid's 17.4 reckoning: the grace does not cover it.
    pub sight_grace_ms: u32,
}

impl Default for SenseTuning {
    fn default() -> Self {
        Self {
            view_deg: 90.0,
            touch_m: 2.0,
            sight_m: 150.0,
            notice_near_ms: 200,
            notice_far_ms: 500,
            notice_far_m: 60.0,
            notice_moving: 0.7,
            bush_notice: 0.0,
            foliage_cap: 3.0,
            shot_m: 60.0,
            sprint_m: 15.0,
            reload_m: 10.0,
            use_m: 8.0,
            step_m: 8.0,
            step_speed: 1.5,
            ranges: HearingRanges::Flat,
            sprint_body_mps: None,
            heard_error: 0.15,
            callout_ms: 1_000,
            forget_ms: 3_000,
            drop_ms: 20_000,
            battery_sight_m: 35.0,
            battery_near_m: 3.0,
            battery_rays: 3,
            battery_moved_m: 0.2,
            spawner_recheck_ms: 45_000,
            sight_untested_ms: crate::v026g::bots::know::SIGHT_UNTESTED_MS,
            sight_grace_ms: crate::v026g::bots::know::SIGHT_GRACE_MS,
        }
    }
}

/// The small cheats (`player_bots_behaviour.md` §2: "a bit, not too
/// much"). Never adaptive; never in the aim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CheatTuning {
    /// Every battery's state and place, not only the HUD's. Off by
    /// default (the owner's word, 2026-10-16: the all-knowing bots ran
    /// straight to the batteries): the fair battery knowledge instead
    /// (`player_bots_behaviour.md` §2.1).
    pub batteries: bool,
    /// A thief's place with this error, metres, this often, ms (0: off).
    pub thief_error_m: f32,
    pub thief_every_ms: u32,
    /// The nearest enemy with this error, this often, while none is
    /// sensed (0: off).
    pub radar_error_m: f32,
    pub radar_every_ms: u32,
    /// Hearing ranges times this.
    pub hearing: f32,
    /// The forgetting time instead of the sense's.
    pub forget_ms: u32,
    /// An enemy that is not the target and was seen at a test stays seen
    /// with no test, at the last tested place only (never his live one):
    /// what a player has of an enemy he saw a moment ago, in place of a
    /// look at every non-target at every tick. At most this long, ms on
    /// the information clock, and at most `SenseTuning::sight_untested_ms`
    /// (validated: not above 450). It is the test that is the sampling
    /// interval, not a human's memory: a hold shorter than the longest
    /// think gap and a tick (450) blanks a visible enemy for the rest of
    /// the gap (250 did: the camper's visible-but-no-target time rose); the
    /// human release curve (250) belongs to the grace after a failed look.
    /// The target is always looked at, never held.
    pub sight_hold_ms: u32,
    /// ... and, when true, only until the bot's own next think and a tick,
    /// the test that comes next (`know::SightWindow::held_until`).
    pub sight_hold_to_next_think: bool,
}

impl Default for CheatTuning {
    fn default() -> Self {
        Self {
            batteries: false,
            thief_error_m: 10.0,
            thief_every_ms: 3_000,
            radar_error_m: 15.0,
            radar_every_ms: 5_000,
            hearing: 1.25,
            forget_ms: 5_000,
            sight_hold_ms: crate::v026g::bots::know::SIGHT_HOLD_MS,
            sight_hold_to_next_think: true,
        }
    }
}

impl Check for SenseTuning {
    /// The sense's ranges in order.
    fn check(&self) -> Result<(), String> {
        check(
            self.view_deg > 0.0 && self.view_deg <= 360.0,
            "sense.view_deg",
        )?;
        check(
            self.notice_near_ms <= self.notice_far_ms,
            "sense.notice_*_ms",
        )?;
        check(self.bush_notice >= 0.0 && self.foliage_cap >= 1.0, "sense.bush_notice, foliage_cap")?;
        check(self.sprint_body_mps.is_none_or(|v| v > 0.0), "sense.sprint_body_mps")?;
        check(
            match self.ranges {
                HearingRanges::Flat => true,
                HearingRanges::Game { crouch_step_m } => crouch_step_m > 0.0 && crouch_step_m <= self.step_m,
            },
            "sense.ranges.game.crouch_step_m",
        )?;
        check(
            self.sight_grace_ms >= 150 && self.sight_untested_ms >= self.sight_grace_ms,
            "sense.sight_grace_ms (150 or more: the look gap and a tick) and sight_untested_ms",
        )?;
        Ok(())
    }
}

/// The sight windows against each other and the think cadence: nothing is
/// seen with no test longer than the longest think gap and a tick (450 ms
/// with the defaults, the approved cap), a grace is a human's (150 to 400
/// ms), and a hold is within the untested bound.
pub(super) fn validate_sight(
    sense: &SenseTuning,
    cheats: &CheatTuning,
    think: &super::ThinkTuning,
) -> Result<(), String> {
    check(
        sense.sight_untested_ms <= think.think_ms.1 + 50,
        "sense.sight_untested_ms: at most the longest think gap and a tick (450)",
    )?;
    check(
        sense.sight_grace_ms <= 400,
        "sense.sight_grace_ms: at most 400 (the humans' range)",
    )?;
    check(
        cheats.sight_hold_ms <= sense.sight_untested_ms,
        "cheats.sight_hold_ms: at most sense.sight_untested_ms",
    )?;
    Ok(())
}
