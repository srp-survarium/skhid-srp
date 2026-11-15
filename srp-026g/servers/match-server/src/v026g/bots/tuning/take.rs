//! How a bot takes a battery from the floor (`plays/collect.rs`, the Take): A281 (a battery under a tree root on Mamayev Kurgan, 7 failed
//! takes in the owner's match 0x24ddea92) and A282 (Collect plays ended "no way to its goal" at two zone batteries, 84 in two matches).
//! Each part is off by default (the old take) until its by-side bench decides, but `pitch` (view scale) and `drop_impossible` (the owner's
//! correctness fixes); `traces-20261028/fx-zones-a281-blocked-pickup.md` and
//! `fx-zones-a282-no-way-collect.md` have the traces.

use super::*;

/// A use is pressed at a stand only when the latest world puts the body there (A281 (a)): the reckoning had bots on the ground
/// slot beside the battery while the client had lifted them 1 m onto the root, and they pressed 19 times from up there (PROVEN,
/// the recording). The world's feet must be within `across_m` across and `up_m` up or down of the stand, in a world no older
/// than the arrival; a world that says otherwise gives the stand up (the body cannot hold it); none yet waits up to `wait_ms`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldStand {
    pub across_m: Metres,
    pub up_m: Metres,
    pub wait_ms: u32,
}

impl Default for WorldStand {
    fn default() -> Self {
        // Across: the take's own "arrived" tolerance (`AT_STAND_M` twice); up: a stand is reached within 0.4 m up or down.
        Self { across_m: Metres::of(0.5), up_m: Metres::of(0.4), wait_ms: 1_500 }
    }
}

/// What a team remembers of its failed takes (A281 (b), A282, A394): the stands its bots gave up are the team's, not one bot's,
/// for `keep_s`; [A394] a battery whose Collect plays failed `aside_after` times at one place, by any of the team's bots, for
/// any reason and of any length ("could not take it", "no way to its goal", any other failure), is set aside for the whole team
/// for `aside_s`, re-offered when it moves (another place) or the span ends. A teammate who says "can't get it from there" is
/// told knowledge (SESSION-RULES rule 4). Off: each bot remembers its own stands 90 s and sets the battery aside alone (20 s
/// after "no way", none after "could not take it" past the first 2 s); its mates take it again at once. ON by default (A394,
/// `owner-5/batteries-findings.md` C3: three batteries tried all match by every bot in turn, 2,283 bot-s, 4.3 % of the bots'
/// alive time; #1162 had merged it OFF on a bench with no such loop).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TeamMemory {
    pub keep_s: Positive,
    pub aside_after: u8,
    pub aside_s: Positive,
}

impl Default for TeamMemory {
    fn default() -> Self {
        // keep: the bot's own `FAILED_KEEP_MS` 90 s; [A394] aside after 2 failed plays, for the "no way" span, 20 s (`NO_WAY_KEEP_MS`,
        // C3's span: a bot's own set-aside after "no way", now the team's).
        Self { keep_s: Positive::of(90.0), aside_after: 2, aside_s: Positive::of(20.0) }
    }
}

/// Stands farther round a battery (A281 (c)): the ring's radii 0.45-1.2 m gain these, with the eye at most `use_m` from the
/// battery's point (the client's ray is 1.80 m). Live humans take from 1.72 m across (median, 280 tries, p10 0.77 m) looking
/// 15 degrees down; the near ring looks steeper and meets more of what lies over a battery. The nearest stand still comes first.
/// With `pitch` by the view's scale (the default) the 1.2 m stands pass and these are the same stands (within the ring's
/// 0.35 m `SAME_STAND_M`): they add nothing until the ring keeps one stand per radius (PROVEN by the far-stands test).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FarStands {
    pub radii_m: [Metres; 2],
    pub use_m: Metres,
}

impl Default for FarStands {
    fn default() -> Self {
        // 1.45 m: a crouched eye (1.04 m) is 1.73 m from a floor battery's point there, under the client's 1.80 m ray; at 1.5 m
        // it is 1.77 m, with no room for a body stopped a little off its stand.
        Self { radii_m: [Metres::of(1.35), Metres::of(1.45)], use_m: Metres::of(1.78) }
    }
}

/// The steepest look down a take's stand is judged by (A281 (d)). `ViewScale` is the default and the all-off arm keeps it
/// (`KEPT_RIGHT`): the old constants assume a view that stops at 1 rad, while the client's stops at 67.48 degrees down
/// (`bots/pitch.rs`, `PitchScale::Client`, #833; PROVEN), so they judged stands by a stop the game does not have. The owner,
/// 2026-11-03 (quiz): "ON as a correctness fix". Its by-side bench, 672 fresh pairs (seeds 8-35, 8 battery maps x 6/8/10,
/// tier 9-10): level, score -0.4 % z -0.5, wins 501 vs 501, deliveries -0.1 %, no guard FAIL; short use presses -66 % z -6.1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakePitch {
    /// The constants set when the view was taken to stop at 1 rad (a ring stand 0.95 rad, where it stopped 0.98).
    Fixed,
    /// From the view's real stop, `kin.pitch_scale`'s (67.48 degrees down for the client's): the same margins under it.
    #[default]
    ViewScale,
}

/// A crossing searched again from a zone floor (A282 (e)): a body that stepped onto a zone's floors by its crossing's edge
/// searched with the plain search (which keeps off the zone) and, the regions saying a plain way is there, no crossing's search
/// (`crossing::wanted`): no route, the goal "no way" (PROVEN, probe: 16-32 such floors by each edge). With this, a failed plain
/// search from a floor zones on all match mark asks the crossing's search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CrossFromZone {}

/// A battery the team believes lies where no floor can hold it is forgotten, and the team looks where it last lay on a floor (A296
/// item 1, the owner's match 0x33e42d77): battery 2 fell through the floor where its carrier died (251-255 s, the HUD following it
/// down to y -91.5) and the team kept it "known" there for 570 s, every bot refusing it "no stand to take it from" and none
/// looking for it (PROVEN, the log). A spot is impossible when it lies more than `below_m` under every floor of the grid within
/// `box_m` across of it, or no floor is there at all (off the level). The look (`LookSpot`) is where the HUD last showed it on a
/// floor: the game puts a battery that fell back near there (battery 1 of the same match, 6 s later, 2.4 m off).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DropImpossible {
    pub below_m: Metres,
    pub box_m: Metres,
}

impl Default for DropImpossible {
    fn default() -> Self {
        // 5 m: deeper than any step, drop or pit floor the grid misses under a lying battery; 4 m: a grid cell's error and a body.
        Self { below_m: Metres::of(5.0), box_m: Metres::of(4.0) }
    }
}

/// A spawner whose battery could not be taken (in a damage zone) is still searched (A296 item 6, A298 cause 1): the spawner search
/// skipped every such spot, so no bot went near, and a team never learned a battery lay there (Vostok Radar Station's 0, 3 and 4 in
/// the gas all match, the trailing team one point behind for the last 4 minutes with three free batteries; PROVEN, A298). With
/// this, the search walks to a dry floor within `within_m` of the spot that sees it in a clear line (`memo::Memo::zone_look`) and
/// looks at it from there: a battery seen is known (the sight rule, `sense.rs`, 35 m), and the take decides as for any known one
/// (its crossing, the zone's dose against the bot's health and armour).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoneLook {
    pub within_m: Metres,
}

impl Default for ZoneLook {
    fn default() -> Self {
        // 24 m: well inside the 35 m battery sight, so a look from the spot's floor sees it at the noticing delay of a near one.
        Self { within_m: Metres::of(24.0) }
    }
}

/// A battery in a damage zone is taken when it is worth the dose (A296 item 6b, the owner's match: "bots were not able to find and
/// get 2 more batteries"). Live humans take zone batteries in the light zones (gas, fruit cloud, acid; 57 take tries in 72 matches,
/// median 15 s inside) and walk straight in (91 % of take visits did not wait at the edge); none went near the deadly ones
/// (traces `fx-zones-a296-zone-batteries-human-baseline.md`, PROVEN). With this:
/// - the bot sprints in (`Pace::Run`), and the way in of a crossing is priced at `in_mps` instead of the walk's `CROSS_MPS` (the
///   way out, carrying, keeps the walk's price);
/// - how much of its pools a bot may spend on the walk, and what it must keep, is the battery's class's ([`ZoneSpend`]: the
///   live humans' spend in that class), and a take inside gives up once the way out would leave under `spend.give_up_left`;
/// - with `zone_look` on, a zone spot is looked at only if a battery there could be taken by this rule (no look at a fire that
///   kills: the 6a bench's cost, search time spent on batteries no bot then took).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoneTake {
    pub in_mps: f32,
    pub spend: ZoneSpend,
}

impl Default for ZoneTake {
    fn default() -> Self {
        // 4.5 m/s: the client's sprint is about 5 m/s (`path::SPRINT_FLIGHT_MPS` 5.1), less its start and the line's side steps. GUESS.
        Self { in_mps: 4.5, spend: ZoneSpend::default() }
    }
}

/// [zone price] How much of each pool a walk through zones may cost (`share`), and what it must leave (`left`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spend {
    pub share: f32,
    pub left: f32,
}

/// [zone price] What a zone walk is for ([`ZoneSpend::spend`]): a take of a battery from the floor, or a carrier's way in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ZoneErrand {
    /// A take, with this many batteries outside the bases (not in a container).
    Take { outside: usize },
    /// A carrier's way in, `time_left_s` before the end, the carrier's team `score_gap` points ahead (behind: negative).
    Carry { time_left_s: f32, score_gap: f32 },
}

/// [zone price] The share of its pools a bot may spend on a zone walk, by how much the battery matters. BOT CHOICE values,
/// each the 75th percentile of what live humans spent in that class and the 25th of what they kept
/// (the zone price design, 4.2-4.3: 203 human-only Batteries matches of the players' server, 2026-10-21..23; 612
/// takes, 81 paid; 3,430 put-ins, 349 paid). When several rows apply, the most
/// generous wins (the largest share; the smaller left on a tie). Being behind is no row: players spend the same behind as
/// ahead (0.32 against 0.35 of the pool, p 0.78).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZoneSpend {
    /// A take in the opening, `after_opening_outside` + 1 or more batteries outside: 50 paid takes, p75 0.41, left p25 0.53.
    pub opening: Spend,
    /// A take with `after_opening_outside` or fewer outside: 31 paid takes, p75 0.59, left p25 0.39.
    pub after_opening: Spend,
    /// A take with `last_outside` or fewer outside: 10 paid takes, p75 0.75, left p25 0.25.
    pub last_batteries: Spend,
    /// A carrier on the way in: 344 paid carries (all but the deciding ones), p75 0.51, left p25 0.46.
    pub carrier: Spend,
    /// Any carrier in the last `late_s`: 28 paid carries, p75 0.55, left p25 0.33.
    pub late_carrier: Spend,
    /// The deciding carry, in the last `late_s` with the score within `close_points`: 5 paid carries (p 0.002 against
    /// the earlier ones), p75 0.76, left p25 0.24.
    pub deciding_carry: Spend,
    /// A walk inside gives up once its way out would leave under this: the humans' 10th percentile of what is left (all
    /// paid takes 0.16, later takes 0.15).
    pub give_up_left: f32,
    /// The opening ends with this many batteries outside (the human data's step: 7 or more against 6 or fewer).
    pub after_opening_outside: u8,
    /// The last batteries: this many or fewer outside.
    pub last_outside: u8,
    /// The end of the match, seconds left: the last 3 minutes.
    pub late_s: f32,
    /// The score within this many points.
    pub close_points: f32,
}

impl Default for ZoneSpend {
    fn default() -> Self {
        Self {
            opening: Spend { share: 0.40, left: 0.50 },
            after_opening: Spend { share: 0.60, left: 0.40 },
            last_batteries: Spend { share: 0.75, left: 0.25 },
            carrier: Spend { share: 0.50, left: 0.45 },
            late_carrier: Spend { share: 0.55, left: 0.33 },
            deciding_carry: Spend { share: 0.75, left: 0.25 },
            give_up_left: 0.15,
            after_opening_outside: 6,
            last_outside: 3,
            late_s: 180.0,
            close_points: 1.0,
        }
    }
}

impl ZoneSpend {
    /// The spend of `errand`: the most generous of the rows that apply.
    pub fn spend(&self, errand: ZoneErrand) -> Spend {
        let rows: [(bool, Spend); 3] = match errand {
            ZoneErrand::Take { outside } => [
                (outside > usize::from(self.after_opening_outside), self.opening),
                (outside <= usize::from(self.after_opening_outside), self.after_opening),
                (outside <= usize::from(self.last_outside), self.last_batteries),
            ],
            ZoneErrand::Carry { time_left_s, score_gap } => {
                let late = time_left_s <= self.late_s;
                [(true, self.carrier), (late, self.late_carrier), (late && score_gap.abs() <= self.close_points, self.deciding_carry)]
            }
        };
        rows.into_iter()
            .filter_map(|(applies, row)| applies.then_some(row))
            .reduce(|a, b| if b.share > a.share || (b.share == a.share && b.left < a.left) { b } else { a })
            .expect("a row always applies")
    }

    fn check(&self) -> Result<(), String> {
        let rows = [self.opening, self.after_opening, self.last_batteries, self.carrier, self.late_carrier, self.deciding_carry];
        if rows.iter().any(|r| !(r.share > 0.0 && r.share <= 1.0 && r.left >= self.give_up_left && r.left < 1.0)) {
            return Err("take.zone_take.spend: a row's share in (0, 1], its left in [give_up_left, 1)".into());
        }
        if !(self.give_up_left > 0.0 && self.give_up_left < 1.0) || self.last_outside > self.after_opening_outside || self.late_s.is_nan() || self.late_s < 0.0 {
            return Err("take.zone_take.spend".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TakeTuning {
    pub world_stand: Option<WorldStand>,
    pub team_memory: Option<TeamMemory>,
    pub far_stands: Option<FarStands>,
    pub pitch: TakePitch,
    pub cross_from_zone: Option<CrossFromZone>,
    /// ON by default and kept in the all-off arm (`KEPT_RIGHT`): believing a battery lies where no floor holds it is proven wrong
    /// (A298: Mamayev Kurgan's battery 2 lay 91 m under the map from 255 s to the end, and the team kept it "known" there). The
    /// owner, 2026-11-04 (quiz): "Yes, turn it ON", a belief-correctness fix like `pitch`. Its by-side bench was identical (168
    /// pairs: the bench world drops no battery under a floor).
    pub drop_impossible: Option<DropImpossible>,
    /// ON by default with `zone_take` (A296 6a+6b): their joint by-side bench, 168 pairs (8 battery maps x 6/8/10, seeds 1-7,
    /// tier 9-10): wins 132 vs 124 (+6.5 %, z +0.7), delivered +2.2 % (z +1.2), points +2.0 %, zone takes +18 % (z +1.8), hits
    /// level; no guard FAIL. Alone, `zone_look` was worse (wins -18.6 %, z -2.1): it walked to batteries no bot then took.
    pub zone_look: Option<ZoneLook>,
    pub zone_take: Option<ZoneTake>,
    /// [A353] A take's first press is made on the move (`collect::FirstPress`): walked in upright at the route's pace,
    /// looking at the battery, pressed on the first tick the bot's view ray meets the box within the client's 1.80 m use
    /// ray; the crouch, the settle wait and the ring of stands only after a press that started nothing. The client starts a
    /// use on the key's down edge when one ray meets the shape, with no angle, stance, movement or look-settle test
    /// (`using_the_world.md`, PROVEN). Off: the last stretch walked crouched, a press only once the view settled (0.06 rad
    /// or 1.5 s); the live recordings had 2.66 s median from 3 m to the press against a player's 0.45 s, 73 % of the bots'
    /// presses crouched against none of the player's (the batteries findings of 2026-11-14, C1).
    pub on_the_move: bool,
    /// [take.known_threats] A take reads the enemies it knows of by the battery, not only the ones it sees now. On: an enemy
    /// whose last spot the bot's or its team's own senses gave (seen, heard, told: `Track::own`, never a cheat's) within
    /// `sense.forget_ms` (the memory's own lifetime of a sensed spot, before it fades) and within the take's `exposed_m` of
    /// the battery is a threat of the take as one seen now is (`Ctx::take_threat`): within `hold_near_m` the take waits,
    /// looking at his spot, until he is dead (the kill feed), reloading (heard), kept busy by a mate whose line to his spot
    /// no cloud hides, or his spot fades; `commit.finish_take` does not hold such a take; and the battery's score reads each
    /// such enemy by his distance from it (`collect::held_weight`). Off: only an enemy seen within 700 ms with a line eye to
    /// eye is a threat, and every remembered enemy within 25 m weighs one on the score. The live recordings of 2026-11-13
    /// (`owner-5/own-base-smoke-and-bait-findings.md`, part 2): 13 bots killed walking to a battery a player stood by, all
    /// 13 knowing of him (heard 8, told 3, seen 2), none aiming at him, 7 never firing, at a median 3.4 m.
    pub known_threats: bool,
}

impl Default for TakeTuning {
    fn default() -> Self {
        Self {
            world_stand: None,
            team_memory: Some(TeamMemory::default()),
            far_stands: None,
            pitch: TakePitch::default(),
            cross_from_zone: None,
            drop_impossible: Some(DropImpossible::default()),
            zone_look: Some(ZoneLook::default()),
            zone_take: Some(ZoneTake::default()),
            on_the_move: true,
            known_threats: true,
        }
    }
}

impl TakeTuning {
    pub(super) fn check(&self) -> Result<(), String> {
        if let Some(memory) = self.team_memory
            && memory.aside_after == 0
        {
            return Err("take.team_memory.aside_after".into());
        }
        if let Some(far) = self.far_stands
            && (far.use_m.get() > 1.8 || far.radii_m.iter().any(|r| r.get() > far.use_m.get()))
        {
            return Err("take.far_stands".into());
        }
        if let Some(zone) = self.zone_take {
            if !(zone.in_mps > 0.0 && zone.in_mps <= 6.0) {
                return Err("take.zone_take.in_mps".into());
            }
            zone.spend.check()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [zone price] Each class gets its row, and where several apply the most generous wins: the opening's take 0.40 / 0.50,
    /// after it 0.60 / 0.40, the last 3 batteries 0.75 / 0.25 (it is also after the opening); a carrier 0.50 / 0.45, in the
    /// last 180 s 0.55 / 0.33, the deciding carry 0.75 / 0.25 whatever the side of the 1-point score. Being behind is no
    /// row; the defaults pass the check, a row under the give-up does not.
    #[test]
    fn a_zone_walk_spends_the_most_generous_row_of_its_class() {
        let spend = ZoneSpend::default();
        let row = |share, left| Spend { share, left };
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 10 }), row(0.40, 0.50));
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 7 }), row(0.40, 0.50));
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 6 }), row(0.60, 0.40));
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 4 }), row(0.60, 0.40));
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 3 }), row(0.75, 0.25));
        assert_eq!(spend.spend(ZoneErrand::Take { outside: 0 }), row(0.75, 0.25));
        assert_eq!(spend.spend(ZoneErrand::Carry { time_left_s: 600.0, score_gap: -3.0 }), row(0.50, 0.45));
        assert_eq!(spend.spend(ZoneErrand::Carry { time_left_s: 180.0, score_gap: 4.0 }), row(0.55, 0.33));
        for gap in [-1.0, 0.0, 1.0] {
            assert_eq!(spend.spend(ZoneErrand::Carry { time_left_s: 30.0, score_gap: gap }), row(0.75, 0.25), "{gap}");
        }
        assert_eq!(spend.spend(ZoneErrand::Carry { time_left_s: 30.0, score_gap: -2.0 }), row(0.55, 0.33));
        assert!(spend.check().is_ok());
        let low = ZoneSpend { carrier: row(0.5, 0.1), ..spend };
        assert!(low.check().is_err());
        // A row tuned more generous than a narrower one still wins where it applies: the most generous, not the narrowest.
        let wide = ZoneSpend { after_opening: row(0.9, 0.2), ..spend };
        assert_eq!(wide.spend(ZoneErrand::Take { outside: 2 }), row(0.9, 0.2));
    }
}
