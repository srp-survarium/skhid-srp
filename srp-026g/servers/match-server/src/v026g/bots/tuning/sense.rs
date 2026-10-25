use super::*;

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
            shot_m: 60.0,
            sprint_m: 15.0,
            reload_m: 10.0,
            use_m: 8.0,
            step_m: 8.0,
            step_speed: 1.5,
            heard_error: 0.15,
            callout_ms: 1_000,
            forget_ms: 3_000,
            drop_ms: 20_000,
            battery_sight_m: 35.0,
            battery_near_m: 3.0,
            battery_rays: 3,
            battery_moved_m: 0.2,
            spawner_recheck_ms: 45_000,
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
        }
    }
}

/// The sense's ranges in order.
pub(super) fn validate(sense: &SenseTuning) -> Result<(), String> {
    check(
        sense.view_deg > 0.0 && sense.view_deg <= 360.0,
        "sense.view_deg",
    )?;
    check(
        sense.notice_near_ms <= sense.notice_far_ms,
        "sense.notice_*_ms",
    )?;
    Ok(())
}
