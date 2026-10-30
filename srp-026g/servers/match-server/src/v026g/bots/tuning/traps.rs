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
