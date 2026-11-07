//! The bear-trap habit's numbers (`bots/traps.rs`). Each is a GUESS marked so
//! unless it is the game's (`docs/notes/bear_traps.md`); the bench decides.

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TrapTuning {
    /// Whether a bot that carries traps places them. A bot carries none
    /// until its kit does (`traps::Habit::carry`), so this off or the kit
    /// off gives today's bots bit for bit.
    pub enabled: bool,
    /// Traps a bot puts down in one life (the owner's, 2026-10-30: one a life).
    pub per_life: u8,
    /// Traps the team's bots keep standing at once, promised or armed (the owner's, 2026-10-30: four a team).
    pub cap_team: u8,
    /// How far ahead of the feet the floor point is, metres: the head ray
    /// is 2.0 m (PROVEN) and the view looks down at most 1.0 rad
    /// (`body::MAX_PITCH`), so at the eye's 1.6 m the floor is hit 1.0 m out.
    pub reach_m: f32,
    /// The trap lands at least this far from the placer's feet, metres: the habit looks down only so steeply that
    /// the head ray meets the floor this far out from the eye the body has (`view::eye_height`: crouched it is
    /// 0.3 m lower and the steepest look lands 0.55 m out, on the feet), presses only when the landing predicted
    /// from the pose at that moment is this far from the feet and, off the bot's own route ahead, by `lateral_m`,
    /// and plans its spot at the distance that look reaches (`reach_m` at least). The trigger reaches 0.6 m from
    /// a foot point (PROVEN, `bear_traps.md`: 565 of 566 catches), a foot is about 0.2 m, and 0.2 m is margin.
    /// `None`: the habit as it was, the planned spot 1 m out for the standing eye of 1.6 m, so a crouched placer
    /// caught itself (74 % of the stand-in's catches were the placer's, the players' 4 %).
    pub press_landing_m: Option<f32>,
    /// The least the trap lies off the bot's own route ahead, metres: the
    /// placer was the victim in 4.2 % of catches, 3 s after placing (PROVEN).
    pub lateral_m: f32,
    /// How far ahead the route is kept clear of it, metres.
    pub route_ahead_m: f32,
    /// Place on the pass (A169): the habit looks along the carrier's route ahead for the cell with the most enemy
    /// lane presence and, when that is richer than the spot at its feet by `gain`, waits and places when the body
    /// passes it, instead of putting the trap where it happens to stand. No goal, route or play changes. `None`: the
    /// habit as it was (the best of the eight yaws round the feet at once). On by default (A169, with
    /// `enemy_near_m` 20): see `enemy_near_m`.
    pub on_the_pass: Option<PassTuning>,
    /// A trap on the spot of a holder the team killed (A145): the place a killed enemy held (`know::Defeated`: a held threat spot
    /// of his, or a place the killer's senses had him staying at) is a spot the next one comes to; a carrier whose route passes it
    /// waits and puts the trap there, with the same gates as any trap (the enemy near, the friends, the team's own traps and the
    /// spawn area) and a wider guard for mates. **Off by default** (`None`: the habit as it was): the owner's idea, built and benched
    /// on v5 map data and found inert. The narrowed rule (holders only) put 15 traps on such spots in 112 team-matches of the traps
    /// preset and 13 in 112 of the camper configs (stand-in camper at place 10 / 0), caught no more than the lane rule's pick, and
    /// changed nothing in the match (wins 89 = 89, score -0.1 %); the rule over any killer's spot caught less (0.10 a trap). A holder
    /// is back within 3 m of his death place in 120 s in 27-40 % of cases (his team's other bots 21-22 %), but the carriers seldom
    /// pass it: of the candidates at a wished spot the floor, their own route and a mate near refuse most.
    pub at_defeated: Option<DefeatedTuning>,
    /// [keep_off] The team's searches keep every known trap of the team at `hard_m`, hard, instead of `FIELD_M` (0.75 m) and the
    /// soft ring; where that leaves no way (a corridor), the search falls back to `FIELD_M`. At the defaults the habit caught its own
    /// side as often as the enemy (A318 check, every bench's OFF arm: 123 own vs 106 enemy catches): 72 % were mates, most 10 s or
    /// more after the press, walking a route past a known trap with only 0.75 m against a 0.6 m trigger and the body's drift. A bot
    /// already within `hard_m` of a trap (its placer) may still take the floors farther from it than its feet. [A324] On by
    /// default at the size the client and the measured walk give (`KeepOff::hard_m`): of 23 own-team catches on the farming-camper
    /// probe (seed 5) 9 were routes passing a known trap 0.62-0.83 m off.
    pub keep_off: Option<KeepOff>,
    /// [A324] How a world's trap confirms a bot's promise (`trap_know::TrapList::sync`). Reach by default, kept right
    /// (`KEPT_RIGHT`): on the farming-camper probe 3 of 8 promises dropped unconfirmed had a real trap placed at the very release
    /// millisecond 1.50-2.11 m away, which then stood unknown to the team (5 of 23 own-team catches were of traps the victim's
    /// team did not know).
    pub promise_match: PromiseMatch,
    /// [A145, design-p1c §9] A trap on the floor a camper stood on when the team's Breach (in its hold) or its K2 flanker
    /// killed him: the job's bot with a trap nearest his death point is wished to put it there when its route passes the
    /// point or it stands by it (no detour, no new goal), with the gates of any trap and a wider guard for mates. Live 6+
    /// player matches (`fx-gear-trap-payoff.md`, 1,070 cleared campers): he or a mate steps within 1 m of that point in
    /// 41 % of cases within 120 s (he himself 21.5 %, a median 46 s after his death), and the clearing team as often (38 %):
    /// hence the guard. `at_defeated` stays the fallback for the spots this does not cover. Off by default.
    pub at_cleared: Option<ClearedTuning>,
    /// [A324] Where the habit counts a released trap as lying, and so when it lets the key go. The client places a trap on
    /// the key's release where a ray from the head along the view meets the walk geometry within 2.0 m, on flat ground only
    /// (normal.y at least cos 30 deg), not on another trap's sensor (`try_to_place_trap` 0x305EA0, `docs/notes/bear_traps.md`,
    /// PROVEN); otherwise nothing is placed and the trap stays in the pocket. `plane` (the old value) met the flat plane at the
    /// planned spot's height: on the farming-camper bench 15 of 54 releases (28 %) put no trap down while the bot counted its
    /// life's trap spent and the team kept a promise of it for 20 s (A324). That is wrong by the client, so `client` is the
    /// default and the all-off patch keeps it (`KEPT_RIGHT`).
    pub release: TrapRelease,
    /// [A324] The camper trap job: on a held spot the team has a K2 job on (the 2-kill cue), a bot with a trap is SENT to put
    /// one on his way back to the spot (the way from his side's spawns, the stretch just before it, out of the spot's sight) and,
    /// when he dies there, on his death point; it walks there in its own play (`TrapSpot`) and places with the job's own gate (an
    /// enemy seen, not one only known near; no chance roll), instead of the calm habit's no-detour wish (`at_defeated`,
    /// `at_cleared`), which near such a spot the enemy gate refuses first (A324 root cause). Live 6+ matches: a cleared camper or
    /// a mate steps within 1 m of his death point in 41 % of cases within 120 s (`fx-gear-trap-payoff.md`). Off by default until
    /// its by-side bench.
    pub camper_job: Option<CamperJob>,
    /// The ring round a standing own trap the team's searches keep out as a preference (soft: a way round where
    /// that is not much longer, else through), metres from its centre; a route that runs within it is searched
    /// again once for each change of the traps. The hard field (0.75 m) is not enough: of the mates' bodies that
    /// came within 3 m of a standing trap of their team's, 33 % of those whose route cleared it by under 0.8 m,
    /// 13 % under 1.0 m, 4.6 % under 1.3 m and under 1 % beyond were caught (A165): a body walks 0.2-0.7 m off its
    /// route and a foot point 0.6 m from the centre sets it off. On at 1.3 m (112 pairs, v5 map data: the mates'
    /// catches -70 %, the placer's -62 %, the enemies' +66 %, no cost in play); 0: none (the field as it was).
    pub ring_m: f32,
    /// A mate's place, or where it is in 2 and 4 s, this near the spot
    /// forbids it, metres.
    pub friend_m: f32,
    /// Another own trap this near forbids it, metres.
    pub near_own_m: f32,
    /// Open floor needed round the spot, outside its 0.9 m disc, within
    /// 2 m (floors of the grid): a trap never shuts a doorway.
    pub open_floors: u32,
    /// No enemy sensed this lately, seconds, and none known this near,
    /// metres.
    pub enemy_quiet_s: f32,
    /// ... 20 m (was 40, A169): of 1.02 M thinks the gate refused 59 % for an enemy known within 40 m, so a trap was placed
    /// only where no enemy was about, away from the enemy's lanes (60 % of the traps in the own third, the players' 29 %);
    /// at 20 m the traps placed rose 40-53 % and the enemy catches 53-137 % (bench, v5, 56 pairs on each of two seed sets).
    pub enemy_near_m: f32,
    /// No hit taken this lately, seconds.
    pub hit_quiet_s: f32,
    /// Health at least.
    pub min_health: f32,
    /// A jump of the route this near ahead forbids it, metres.
    pub jump_ahead_m: f32,
    /// The spot's score at least (`traps::score`): enemy presence at it
    /// minus friend flow.
    pub min_score: f32,
    /// Weight of the friend term in the score.
    pub w_friend: f32,
    /// Enemy lane count at which the presence term is 63 %.
    pub presence_scale: f32,
    /// The chance a think that passes the gate takes the best spot.
    pub chance: f32,
    /// The view must be this near the spot's (radians) to press.
    pub tolerance_rad: f32,
    /// A look that has not found the spot after this long is given up, ms.
    pub look_ms: u32,
    /// Seconds before another try after a placement or a give-up.
    pub cooldown_s: f32,
    /// The response to a trap that is not the team's (`bots/trap_react.rs`).
    pub react: ReactTuning,
    /// [A292 S1] Whose lanes the spot's score reads: `humans` (today: `know::HumanLanes`, the enemy humans only, so in Player vs
    /// Bots the human's own team never places and the bots' team places only on his sensed path) or `enemies` (a team map of every
    /// enemy the team sensed, bots and humans, by the same rule: its own senses, 6 m cells, 180 s half life; kept apart from
    /// `HumanLanes`, whose other readers stay as they are). A human in a bot's place knows where any enemy he saw or heard walked.
    /// `humans` by default until its by-side bench decides.
    pub lanes: TrapLanes,
    /// [A292 S2] The enemy-near gate's "sensed lately" part only for an enemy sensed this near (`None`: anywhere, today). The
    /// tally of #1207 on the bench: of the thinks the enemy gate stopped, about 70 % were "sensed lately" (a shot or a step
    /// heard across the map counted). Seen now and known within `enemy_near_m` stay as they are. **On by default**, kept right
    /// (`KEPT_RIGHT`): a step or shot heard across the map is no enemy near (A292 by-side bench, 256 pairs, both modes: traps
    /// placed x2, enemy catches 9 vs 3, own-team catches 3 vs 2, the whole match level, no guard worse, the coordinator's go).
    pub sensed_near: Option<SensedNear>,
    /// [A292 S4a] The placer stands from its trap's press until its next think has put the trap in its field and re-planned its
    /// route round it, at most `hold_ms`. Without it the bot walks on its old route for that time: in the S1 bench 13 of 298
    /// traps caught their own placer 0-3 s after the press, landing 0.85-1.25 m from its feet. Off by default.
    pub after_press: Option<AfterPress>,
    /// [A292 S4c] Whether a bot that stays (holds a spot, no route ahead) puts a trap down: `place` (today) or `never`. A stayer's
    /// trap lands about a metre from its feet (its look reaches no farther), and the reflex's side steps and peeks do not keep off
    /// the team's trap field: in the S1 + `after_press` + ring 2.0 bench 17 of 23 self-catches were stayers 4-32 s after their
    /// press. `place` by default until its bench decides.
    pub stayer: Stayer,
    /// [A309] A spot is taken only this far forward on the way from the bot's side's home (its spawns' centre) to the enemy's:
    /// `d_own / (d_own + d_enemy)` at least `min_share`. In the A309 bench (the owner's trap set, 224 pairs against the hunter)
    /// 411 traps caught 13 enemies: 11 of them were 80 m or more from the placer's home, 1 of the 253 traps placed 40-80 m
    /// from it. Off by default.
    pub forward: Option<Forward>,
    /// [A309] For `for_ms` after its press the placer's routes come no nearer its fresh trap than its feet are (within `ring_m`
    /// of it): it walks away from it, never back over it. In the A309 bench the placer caught itself 14 times, median 2.5 s
    /// after the press (the after-press stand ends at 1.5 s), more often than its traps caught an enemy. Off by default.
    pub fresh_ring: Option<FreshRing>,
    /// [A309] The press waits until the body moves at `min_speed_ms` or more: a slow placer (one holding a spot in a fight play)
    /// stays by its trap and steps onto it. Over three A309 benches (835 presses) the presses under 2 m/s caught 8 enemies,
    /// their placer 10 times and a mate 6; those at 2 m/s or more 31 enemies, the placer 13 times, a mate 5. Off by default.
    pub press_moving: Option<PressMoving>,
    /// [A324] The press waits until the landing clears the placer's own path over the next moment (its feet plus its velocity
    /// times 0 to `sweep_s`) by `clear_m`. The client arms a trap at once and it takes any foot within 0.6 m, its placer's too
    /// (`docs/notes/bear_traps.md`: `booby_trap_core::on_enter` 0x304BD0 lets a same-team body go only with the skill
    /// `friendly_trap`, modifier 44, at 0x304CFE..0x304D58; no time test), while the bot's legs keep their course until its next
    /// think (200-400 ms) puts the trap in its field. Without it a walker presses with the landing on its own way and steps on
    /// it 0.1-0.6 s later. **On by default**, kept right (`KEPT_RIGHT`): pressing onto its own path is wrong by the client.
    pub press_clear: Option<PressClear>,
}

/// The habit's rule for the spots of killed holders (`TrapTuning::at_defeated`). The numbers are GUESSES chosen from the K1 spots'
/// own scale (a death is forgotten in two minutes, `threat::DECAY_PER_S`; a spot merges kills within 8 m) and the habit's:
/// the bench decides.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DefeatedTuning {
    /// A killed holder's spot is wanted for this long, seconds.
    pub keep_s: f32,
    /// The trap lies within this of the spot, metres.
    pub within_m: f32,
    /// No mate within this of the trap's cell now, in 2 s or in 4 s (the guard's `friend_m` is 2.5 m): the mates are the ones who
    /// walk onto the spot next.
    pub friend_m: f32,
    /// How far ahead along the route the spot is looked for, metres (at most `route_ahead_m`): a route that passes it within
    /// `within_m` waits for it.
    pub ahead_m: f32,
    /// A held threat spot is a holder's only when it killed at least this many of the team's bots (a holder, as the camper class of
    /// `an-anticamper.md` is: one who kills from one place again and again; one kill in passing is no holder). A place the killer's
    /// own senses had the enemy staying at for `camp_ms` (8 s) counts whatever the spot's kills.
    pub min_kills: u8,
}

impl Default for DefeatedTuning {
    fn default() -> Self {
        Self {
            keep_s: 120.0,
            within_m: 3.0,
            friend_m: 6.0,
            ahead_m: 12.0,
            min_kills: 2,
        }
    }
}

/// [keep_off] How far the team keeps off its own known traps (`TrapTuning::keep_off`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeepOff {
    /// The hard disc round each known trap of the team, metres: where the body touches the trap's sensor plus the body's walk off
    /// its route. [A324] Contact 0.67 m = the sensor cylinder's radius 0.25 m (`docs/notes/bear_traps.md`) + the body capsule's
    /// radius 0.38 m (`mapdata` `grid/controlled.rs`: startup.cfg's stand size 0.76 x 1.8) + the controller's 0.04 m move margin
    /// (`mapdata` `controller/tick.rs`, the exe's under Unicorn); the live trigger, 0.6 m from a foot point, agrees. Walk 0.80 m =
    /// the 90th percentile of the feet off the route of the bots walking one (261,076 samples, farming-camper cell seed 5, tip
    /// dab73194; median 0.25, p95 1.25).
    pub hard_m: f32,
}

impl Default for KeepOff {
    fn default() -> Self {
        Self { hard_m: 1.47 }
    }
}

/// [A324] How near a promise a world's trap of the same time confirms it (`TrapTuning::promise_match`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromiseMatch {
    /// Within 1.5 m of the promise (`trap_know::MATCH_M`): the old rule, which misses a trap the client put down farther from the
    /// bot's reckoned landing.
    Near,
    /// Within the client's reach of it: the promise and the real trap both lie within the item's `max_deploy_distance` (2.0 m,
    /// `[set+0x284]`, `docs/notes/bear_traps.md`) of the placer's eye, so at most twice that apart; the nearest trap placed within
    /// `MATCH_MS` of the release is taken.
    #[default]
    Reach,
}

impl PromiseMatch {
    /// The distance a confirming trap may lie from the promise, metres.
    pub fn metres(self) -> f32 {
        match self {
            Self::Near => super::super::trap_know::MATCH_M,
            Self::Reach => 2.0 * super::super::trap_know::DEPLOY_M,
        }
    }
}

/// [A292 S1] Whose lanes the trap habit's spot score reads (`TrapTuning::lanes`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrapLanes {
    /// The enemy humans' lanes (`know::HumanLanes`): today's habit.
    #[default]
    Humans,
    /// Every enemy the team sensed (`trap_know::TeamTraps::lanes`).
    Enemies,
}

/// [A292 S2] How near an enemy sensed lately must be to keep a bot from placing (`TrapTuning::sensed_near`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensedNear {
    /// Metres: a GUESS, about the reach a rifle fight starts at (the bots' engage range).
    pub within_m: f32,
}

impl Default for SensedNear {
    fn default() -> Self {
        Self { within_m: 40.0 }
    }
}

/// [A309] How far forward a spot must lie (`TrapTuning::forward`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Forward {
    /// The least share of the way from home to the enemy's home, 0-1: 0.5 is the middle of the map. A GUESS from the A309
    /// bench's distances (80 m and more from home caught, 40-80 m did not); the bench decides.
    pub min_share: f32,
    /// When no spot past `min_share` passes, the search is made again past this share (at most `min_share`): the owner wants
    /// more traps, not fewer (the coordinator, 2026-11-05: the first fix cut them from 411 to 113 in the A309 bench).
    pub fallback_share: f32,
}

impl Default for Forward {
    fn default() -> Self {
        Self {
            min_share: 0.5,
            fallback_share: 0.3,
        }
    }
}

/// [A309] How long the placer keeps off its fresh trap (`TrapTuning::fresh_ring`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FreshRing {
    /// From the press, ms: the A309 self-catches came 0.6-14 s after it (median 2.5 s).
    pub for_ms: u32,
}

impl Default for FreshRing {
    fn default() -> Self {
        Self { for_ms: 8_000 }
    }
}

/// [A309] How fast the placer must move to press (`TrapTuning::press_moving`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressMoving {
    /// Ground speed at the press, m/s.
    pub min_speed_ms: f32,
}

impl Default for PressMoving {
    fn default() -> Self {
        Self { min_speed_ms: 2.0 }
    }
}

/// [A324] How far the landing must lie from the placer's own path at the press (`TrapTuning::press_clear`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressClear {
    /// Metres from the swept path: the client's trigger reaches 0.6 m from a foot point (565 of 566 live samples,
    /// `bear_traps.md`); the rest is a placement margin for the feet's spread and the body's drift (fx-gear-2's A292 value).
    pub clear_m: f32,
    /// How far ahead the path is swept, seconds: past the next think (at most 400 ms) and the legs' turn away (a placement choice).
    pub sweep_s: f32,
}

impl Default for PressClear {
    fn default() -> Self {
        Self { clear_m: 1.0, sweep_s: 1.0 }
    }
}

/// [A324] How the habit reckons where a released trap lies (`TrapTuning::release`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrapRelease {
    /// The head ray meets the flat plane at the planned spot's height: the old guess, which releases where the client
    /// places nothing.
    Plane,
    /// The client's rule on the map's mesh: the head ray within 2.0 m, flat ground, no known trap's sensor; the key is held
    /// back (the look goes on) where the client would refuse.
    #[default]
    Client,
}

/// [A292 S4c] Whether a bot that stays puts a trap down (`TrapTuning::stayer`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stayer {
    /// It does, at its feet's ring of spots: today's habit.
    #[default]
    Place,
    /// It does not: only a bot on its way places.
    Never,
}

/// [A292 S4a] How long the placer may stand after its press (`TrapTuning::after_press`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AfterPress {
    /// At most this, ms: a GUESS margin over the few hundred ms between two thinks of a bot.
    pub hold_ms: u32,
    /// The press also waits until the landing lies this far from the body's own path over the next `sweep_s` (its feet plus its
    /// velocity times 0 to `sweep_s`), metres: the trigger reaches 0.6 m from a foot point, a foot is about 0.2 m, so 1.0 keeps a
    /// slowing walker off it (in the S1 bench 6 walkers were caught 0.4-0.6 s after the press, 1.1-1.2 m from their feet, their
    /// route 0.9 m off the trap but their body running on).
    pub clear_m: f32,
    pub sweep_s: f32,
}

impl Default for AfterPress {
    fn default() -> Self {
        Self {
            hold_ms: 1_500,
            clear_m: 1.0,
            sweep_s: 1.0,
        }
    }
}

/// The rule for the death point of a camper a team job just killed (`TrapTuning::at_cleared`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClearedTuning {
    /// His death point is wished this long, seconds: design-p1c §9 places within 20 s; his own return comes P25 33 s after
    /// his death.
    pub keep_s: f32,
    /// The trap lies within this of the point, metres (a foot point 0.6 m from a trap's centre sets it off).
    pub within_m: f32,
    /// No mate within this of the trap's cell now, in 2 s or in 4 s, metres (the guard's `friend_m` is 2.5 m): the
    /// clearing team crosses the point in 38 % of the live cases.
    pub friend_m: f32,
    /// How far ahead along the route the point is looked for, metres (at most `route_ahead_m`): a route that passes it
    /// within `within_m` waits for it.
    pub ahead_m: f32,
}

impl Default for ClearedTuning {
    fn default() -> Self {
        Self { keep_s: 20.0, within_m: 1.5, friend_m: 4.0, ahead_m: 12.0 }
    }
}

/// [A324] The camper trap job's rule (`TrapTuning::camper_job`). The numbers are placement choices (the owner: only where to trap
/// is tuned on benches); the trap's own values are the client's (`bear_traps.md`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CamperJob {
    /// His death point is wanted this long after his death, seconds: live, he is back within 10 m a median 31 s after it and
    /// within 1 m a median 46 s (P75 77 s).
    pub death_keep_s: f32,
    /// The way back: points every `step_m` from `near_m` to `far_m` before the spot along the way from his side's spawns; the
    /// first the spot's eye does not see is the one trapped.
    pub near_m: f32,
    pub far_m: f32,
    pub step_m: f32,
    /// The job's bot is the nearest with a trap within this of the point, metres on the ground.
    pub max_walk_m: f32,
    /// A bot sent and not done in this long is let go (another may be sent), seconds.
    pub give_up_s: f32,
    /// The trap lies within this of the point, metres.
    pub within_m: f32,
    /// No mate within this of the trap's cell now, in 2 s or in 4 s, metres (the habit's `friend_m` is 2.5 m): the clearing team
    /// crosses a cleared camper's point in 38 % of the live cases, and a job's first trap on the bench caught a mate 0.1 s after.
    pub friend_m: f32,
}

impl Default for CamperJob {
    fn default() -> Self {
        Self { death_keep_s: 60.0, near_m: 6.0, far_m: 20.0, step_m: 3.0, max_walk_m: 30.0, give_up_s: 45.0, within_m: 1.5, friend_m: 4.0 }
    }
}

/// The look along the route of the habit's place-on-the-pass rule (`TrapTuning::on_the_pass`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PassTuning {
    /// How far ahead along the route the richer cell is looked for, metres (at most `route_ahead_m`).
    pub route_ahead_m: f32,
    /// A cell ahead is worth waiting for when its lane presence exceeds the spot at the feet by this (the units of
    /// `traps::presence`: lane counts within 6 m).
    pub gain: f32,
}

impl Default for PassTuning {
    fn default() -> Self {
        Self {
            route_ahead_m: 12.0,
            gain: 1.0,
        }
    }
}

/// How a bot notices a trap of the other team or a human's (C18), and what is
/// done about it. **Off** (`on` false) the bots are as before, bit for bit;
/// on by default (`traps.react.on`). Every number is
/// a GUESS: the recordings show no measurable reaction of the players to an
/// armed trap, so the chance and the distances are tuned by the bench against
/// the players' numbers (about 10 % of traps end disarmed; a walk-through
/// share of passers of 28-35 %).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReactTuning {
    /// Whether a bot rolls for the traps it comes near, and counts its passes.
    pub on: bool,
    /// The chance of noticing a trap within `near_m` (once a roll).
    pub p0: f32,
    /// Within this the chance is `p0`, metres.
    pub near_m: f32,
    /// Beyond this a trap is not rolled for, metres; the chance falls
    /// linearly from `near_m` to it.
    pub far_m: f32,
    /// The view the trap must lie in, degrees wide (`sense::in_view`).
    pub cone_deg: f32,
    /// Seconds before a missed trap is rolled for again.
    pub roll_s: f32,
    /// The chance is divided by this while the bot is in a fight.
    pub fight_div: f32,
    /// Whether a noticed trap is shot (needs `on`).
    pub shot: bool,
    /// A trap farther than this is not shot at, metres.
    pub shoot_m: f32,
    /// The view must be this near the trap's centre to fire, radians.
    pub aim_ok_rad: f32,
    /// The aim is held this long before the first shot, ms (the sights come
    /// up in 0.2-0.35 s).
    pub aim_settle_ms: u32,
    /// The aim not on after this long is given up, ms.
    pub aim_timeout_ms: u32,
    /// Ms between single shots.
    pub shot_gap_ms: u32,
    /// Shots at one trap at most, and seconds of shooting at most.
    pub max_shots: u8,
    pub max_shoot_s: f32,
    /// A mate seen shooting at the trap: the bot holds for it this long, ms
    /// (then leaves the trap to it); the mate counts if its shot was this
    /// recent (ms), it stands within `claim_m` of the bot or the trap
    /// (metres) and looks at the trap within `claim_yaw_deg` (the view's
    /// whole width, degrees).
    pub hold_ms: u32,
    pub claim_ms: u32,
    pub claim_m: f32,
    pub claim_yaw_deg: f32,
    /// Whether a bot's routes keep off the traps it noticed: round them where
    /// that is not much longer, else over them (needs `on`).
    pub pass: bool,
    /// Whether a bot whose route runs over a trap it noticed jumps it, with
    /// the motor's jump (needs `on`).
    pub jump: bool,
    /// The jump is pressed this far before the trap, metres along the route
    /// (a GUESS: the body clears 0.22 m for about 0.6 m beyond the trap at a
    /// run).
    pub jump_before_m: f32,
    /// Whether a trap that goes off is heard by the enemies of the body that
    /// set it off, as a sound of the loudness of footsteps (`sense.step_m`).
    /// Independent of `on`. A disarm makes no sound.
    pub hear: bool,
    /// The body nearest a trap that went off, within this, is the one that
    /// set it off, metres (the worlds show no name).
    pub snap_near_m: f32,
    /// Whether a bot that noticed a trap and does not shoot it walks up and
    /// defuses it by hand: USE held on it, crouched beside it (needs `on`).
    pub defuse: bool,
    /// The chance a noticing bot decides to defuse, rolled once a trap and a
    /// life (a GUESS: the players' hand defuses are about 3 % of traps).
    pub defuse_p: f32,
    /// A trap farther than this (metres) or longer to walk to than
    /// `defuse_walk_s` is not defused.
    pub defuse_walk_m: f32,
    pub defuse_walk_s: f32,
    /// The bot stands this far from the trap's centre, metres: outside the
    /// foot's 0.6 m reach, inside the use ray's 2.0 m from the eye.
    pub defuse_stand_m: f32,
    /// The trap's defuse time, seconds (`defuse_time` 5, PROVEN); the hold is
    /// given up 1.5 s after it.
    pub defuse_s: f32,
    /// The view must be this near the trap's centre to press USE, radians,
    /// and is held `use_settle_ms` first.
    pub use_ok_rad: f32,
    pub use_settle_ms: u32,
}

impl Default for ReactTuning {
    fn default() -> Self {
        Self {
            on: true,
            p0: 0.05,
            near_m: 3.0,
            far_m: 8.0,
            cone_deg: 140.0,
            roll_s: 20.0,
            fight_div: 3.0,
            shot: true,
            shoot_m: 14.0,
            aim_ok_rad: 0.012,
            aim_settle_ms: 250,
            aim_timeout_ms: 2500,
            shot_gap_ms: 400,
            max_shots: 4,
            max_shoot_s: 4.0,
            hold_ms: 2000,
            claim_ms: 1500,
            claim_m: 6.0,
            claim_yaw_deg: 40.0,
            pass: true,
            jump: true,
            jump_before_m: 1.1,
            hear: true,
            snap_near_m: 3.0,
            defuse: true,
            defuse_p: 0.3,
            defuse_walk_m: 10.0,
            defuse_walk_s: 8.0,
            defuse_stand_m: 0.85,
            defuse_s: 5.0,
            use_ok_rad: 0.05,
            use_settle_ms: 250,
        }
    }
}

impl Default for TrapTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            per_life: 1,
            cap_team: 4,
            reach_m: 1.0,
            press_landing_m: Some(1.0),
            lateral_m: 0.9,
            route_ahead_m: 12.0,
            on_the_pass: Some(PassTuning::default()),
            at_defeated: None,
            keep_off: Some(KeepOff::default()),
            promise_match: PromiseMatch::Reach,
            at_cleared: None,
            release: TrapRelease::Client,
            camper_job: None,
            ring_m: 1.3,
            friend_m: 2.5,
            near_own_m: 4.0,
            open_floors: 12,
            enemy_quiet_s: 2.0,
            enemy_near_m: 20.0,
            hit_quiet_s: 6.0,
            min_health: 0.5,
            jump_ahead_m: 6.0,
            min_score: 0.05,
            w_friend: 1.0,
            presence_scale: 3.0,
            chance: 0.7,
            tolerance_rad: 0.07,
            look_ms: 900,
            cooldown_s: 20.0,
            react: ReactTuning::default(),
            lanes: TrapLanes::Humans,
            sensed_near: Some(SensedNear { within_m: 40.0 }),
            after_press: None,
            stayer: Stayer::Place,
            forward: None,
            fresh_ring: None,
            press_moving: None,
            press_clear: Some(PressClear::default()),
        }
    }
}

impl Check for TrapTuning {
    fn check(&self) -> Result<(), String> {
        check(
            self.reach_m > 0.0 && self.reach_m <= 1.2 && self.lateral_m >= 0.0 && self.route_ahead_m >= 0.0,
            "traps.reach_m",
        )?;
        check(
            self.press_landing_m
                .is_none_or(|m| m >= super::super::traps::TRIGGER_M)
                && super::super::traps::press_reachable(self),
            "traps.press_landing_m",
        )?;
        check(
            self.on_the_pass
                .is_none_or(|p| p.route_ahead_m >= 2.0 && p.route_ahead_m <= self.route_ahead_m && p.gain >= 0.0),
            "traps.on_the_pass",
        )?;
        check(
            self.at_defeated.is_none_or(|d| {
                d.keep_s > 0.0
                    && d.within_m >= 1.0
                    && d.within_m <= 6.0
                    && d.friend_m >= self.friend_m
                    && d.ahead_m >= 2.0
                    && d.ahead_m <= self.route_ahead_m
                    && d.min_kills <= 5
            }),
            "traps.at_defeated",
        )?;
        check(
            self.at_cleared.is_none_or(|c| {
                c.keep_s > 0.0 && c.within_m >= 1.0 && c.within_m <= 6.0 && c.friend_m >= self.friend_m && c.ahead_m >= 2.0 && c.ahead_m <= self.route_ahead_m
            }),
            "traps.at_cleared",
        )?;
        check(
            self.camper_job.is_none_or(|j| {
                j.death_keep_s > 0.0
                    && j.step_m > 0.0
                    && j.near_m >= 1.0
                    && j.far_m >= j.near_m
                    && j.max_walk_m > 0.0
                    && j.give_up_s > 0.0
                    && (1.0..=6.0).contains(&j.within_m)
                    && j.friend_m >= self.friend_m
            }),
            "traps.camper_job",
        )?;
        check(
            self.friend_m >= 0.0
                && self.near_own_m >= 0.0
                && self.jump_ahead_m >= 0.0
                && (self.ring_m == 0.0 || (super::super::traps::FIELD_M..=4.0).contains(&self.ring_m)),
            "traps.*_m",
        )?;
        check(
            self.presence_scale > 0.0 && self.w_friend >= 0.0,
            "traps.presence_scale",
        )?;
        check(
            (0.0..=1.0).contains(&self.chance) && (0.0..=1.0).contains(&self.min_health),
            "traps.chance",
        )?;
        check(self.tolerance_rad > 0.0 && self.look_ms > 0, "traps.look")?;
        check(
            // Under `enemy_near_m` it is harmless: an enemy sensed that near is also known that near.
            self.sensed_near.is_none_or(|n| n.within_m > 0.0 && n.within_m <= 200.0),
            "traps.sensed_near",
        )?;
        check(
            self.after_press
                .is_none_or(|a| a.hold_ms > 0 && a.hold_ms <= 5_000 && (0.0..=1.3).contains(&a.clear_m) && (0.0..=3.0).contains(&a.sweep_s)),
            "traps.after_press",
        )?;
        check(self.keep_off.is_none_or(|k| (0.75..=4.0).contains(&k.hard_m)), "traps.keep_off")?;
        check(
            self.forward.is_none_or(|f| (0.0..=1.0).contains(&f.min_share) && (0.0..=f.min_share).contains(&f.fallback_share)),
            "traps.forward",
        )?;
        check(self.fresh_ring.is_none_or(|f| f.for_ms <= 60_000), "traps.fresh_ring")?;
        check(self.press_moving.is_none_or(|p| (0.0..=8.0).contains(&p.min_speed_ms)), "traps.press_moving")?;
        check(
            self.press_clear.is_none_or(|c| (0.6..=3.0).contains(&c.clear_m) && (0.0..=3.0).contains(&c.sweep_s)),
            "traps.press_clear",
        )?;
        let r = &self.react;
        check(
            (0.0..=1.0).contains(&r.p0)
                && r.near_m >= 0.0
                && r.far_m > r.near_m
                && r.cone_deg > 0.0
                && r.cone_deg <= 360.0
                && r.roll_s >= 0.0
                && r.fight_div >= 1.0
                && r.shoot_m > 0.0
                && r.aim_ok_rad > 0.0
                && r.max_shots >= 1
                && r.max_shoot_s > 0.0
                && r.claim_m >= 0.0
                && r.claim_yaw_deg > 0.0
                && r.jump_before_m > 0.4
                && r.snap_near_m > 0.0
                && (0.0..=1.0).contains(&r.defuse_p)
                && r.defuse_walk_m > 0.0
                && r.defuse_walk_s > 0.0
                && r.defuse_stand_m > 0.6
                && r.defuse_s > 0.0
                && r.use_ok_rad > 0.0,
            "traps.react",
        )?;
        Ok(())
    }
}
