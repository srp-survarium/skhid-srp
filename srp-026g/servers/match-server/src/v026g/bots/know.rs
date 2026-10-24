//! What a bot and its team know (`docs/notes/player_bots_research.md` B1,
//! "Knowledge (Brewer)"): every enemy fact carries the sense it came by
//! ([`Source`]), so the fairness rules of `player_bots_behaviour.md` §2 are
//! rules on types:
//! - the motor aims only at a [`SeenTrack`], which only [`Memory`] makes,
//!   and only from a sight older than the noticing delay: "never aim at
//!   what no sense gave" and "never react faster than noticing" cannot be
//!   broken by a play;
//! - a teammate's word ([`Source::Told`]) and the cheats
//!   ([`Source::Cheat`]) steer routes and where to look, never a shot;
//! - an enemy not sensed for a while becomes a last known position that
//!   fades, then is dropped.

use mapdata::Vec3;

use super::view::{BatteryState, MatchView, Place};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CheatKind {
    Thief,
    Radar,
}

/// How a fact came to the bot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Seen,
    Heard,
    /// A bot teammate's callout.
    Told {
        by: Place,
    },
    /// What a human's HUD shows (the kill feed, markers).
    Hud,
    Cheat(CheatKind),
}

impl Source {
    /// How much the bot trusts a report of this kind: its own sight over
    /// its own hearing over a teammate's word (and the HUD) over a cheat.
    pub fn strength(self) -> u8 {
        match self {
            Self::Seen => 3,
            Self::Heard => 2,
            Self::Told { .. } | Self::Hud => 1,
            Self::Cheat(_) => 0,
        }
    }

    /// The team's own senses: a bot's sight or hearing, a teammate's word.
    pub fn own(self) -> bool {
        matches!(self, Self::Seen | Self::Heard | Self::Told { .. })
    }

    /// A track of this kind sensed within this, ms, is fresh: a weaker
    /// report does not replace it. A sight for a moment (as before: a heard
    /// shot from where he went is better than a sight half a second old),
    /// the rest for [`FRESH_MS`].
    pub fn fresh_ms(self) -> u32 {
        match self {
            Self::Seen => 500,
            _ => FRESH_MS,
        }
    }
}

/// A heard or told track sensed within this, ms, is fresh: a weaker report
/// does not replace it (a teammate's callout does not wipe what the bot
/// heard itself a moment ago; play 6's heard shot is as long).
pub const FRESH_MS: u32 = 1_500;

/// A sense's report: who, where (with its error), when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percept {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    pub time: u32,
    pub source: Source,
    /// Seen moving or firing: noticed sooner.
    pub busy: bool,
}

/// What a bot knows of one enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    /// When it was last sensed.
    pub time: u32,
    pub source: Source,
    /// Since when it has been in sight without a break (`Seen` only).
    pub sight_since: Option<u32>,
    /// The noticing delay for this sight, ms.
    pub notice_ms: u32,
    /// The last place the bot's or its team's own senses gave (seen,
    /// heard, told) and when: a cheat's place does not count.
    pub own: Option<(Vec3, u32)>,
}

/// An enemy seen, and noticed: the only thing the motor may aim at. Made
/// only by [`Memory::seen`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeenTrack(Track);

impl SeenTrack {
    pub fn track(&self) -> &Track {
        &self.0
    }

    pub fn who(&self) -> Place {
        self.0.who
    }

    pub fn at(&self) -> Vec3 {
        self.0.at
    }
}

/// One bot's memory of the enemies.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    tracks: Vec<Track>,
}

impl Memory {
    /// Takes in a think's percepts. `notice_ms(percept)` is the noticing
    /// delay for a new sight; `forget_ms` and `drop_ms` the fading.
    pub fn update(
        &mut self,
        percepts: &[Percept],
        now: u32,
        notice_ms: impl Fn(&Percept) -> u32,
        forget_ms: u32,
        drop_ms: u32,
    ) {
        for percept in percepts {
            let notice = notice_ms(percept);
            match self.tracks.iter_mut().find(|t| t.who == percept.who) {
                Some(track) => {
                    let seen = percept.source == Source::Seen;
                    // A report replaces the place only if it is no older
                    // than the track and not weaker than a fresh one: a
                    // teammate's callout (sensed a second or more ago) does
                    // not wipe the bot's own hearing, a cheat not a
                    // callout, and nothing a sight.
                    let fresh = track.time + track.source.fresh_ms() >= now;
                    let weaker = percept.source.strength() < track.source.strength();
                    let replaces = percept.time >= track.time && !(fresh && weaker);
                    if replaces {
                        track.at = percept.at;
                        track.error_m = percept.error_m;
                        track.source = percept.source;
                    }
                    if percept.source.own()
                        && track.own.is_none_or(|(_, time)| percept.time >= time)
                    {
                        track.own = Some((percept.at, percept.time));
                    }
                    if seen {
                        // A sight goes on if the last one was a moment ago.
                        let continuing =
                            track.sight_since.is_some() && track.time + 400 >= percept.time;
                        if !continuing {
                            track.sight_since = Some(percept.time);
                            track.notice_ms = notice;
                        }
                    } else if track.time + 400 < percept.time {
                        track.sight_since = None;
                    }
                    // When it was last sensed by a report it took: a weaker
                    // one it did not take keeps no sight alive (a cheat's
                    // place of a thief is no sight of him).
                    if replaces {
                        track.time = percept.time;
                    }
                }
                None => self.tracks.push(Track {
                    who: percept.who,
                    at: percept.at,
                    error_m: percept.error_m,
                    time: percept.time,
                    source: percept.source,
                    sight_since: (percept.source == Source::Seen).then_some(percept.time),
                    notice_ms: notice,
                    own: percept.source.own().then_some((percept.at, percept.time)),
                }),
            }
        }
        for track in &mut self.tracks {
            // Out of sight now.
            if track.sight_since.is_some() && track.time + 400 < now {
                track.sight_since = None;
            }
            // Not sensed for a while: a last known position, fading.
            if track.time + forget_ms < now {
                track.error_m += 0.5;
            }
        }
        self.tracks.retain(|track| track.time + drop_ms >= now);
    }

    /// The enemy at `who` seen now and noticed.
    pub fn seen(&self, who: Place, now: u32) -> Option<SeenTrack> {
        let track = self.tracks.iter().find(|t| t.who == who)?;
        let since = track.sight_since?;
        (track.source == Source::Seen && now >= since + track.notice_ms && track.time + 400 >= now)
            .then_some(SeenTrack(*track))
    }

    /// Every enemy seen now and noticed.
    pub fn all_seen(&self, now: u32) -> Vec<SeenTrack> {
        self.tracks
            .iter()
            .filter_map(|track| self.seen(track.who, now))
            .collect()
    }

    /// Everything known, however it came and however old.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// An enemy died: it is not where it was.
    pub fn forget(&mut self, who: Place) {
        self.tracks.retain(|track| track.who != who);
    }
}

/// A teammate's death at a spot: danger there for a while (and a trade to
/// make).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DangerSpot {
    pub at: Vec3,
    pub time: u32,
    pub victim: Place,
}

/// A teammate's callout "shot from there": hit with no shooter it sensed,
/// it gives the direction its HUD's hit marker showed (no distance, no
/// shooter), from where it stood.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotFrom {
    pub by: Place,
    pub at: Vec3,
    /// The marker's direction (a yaw, radians, with the marker's error).
    pub bearing: f32,
    /// When the team knows it (the callout's delay after the hit).
    pub time: u32,
}

/// A battery of ours taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theft {
    pub battery: usize,
    /// Where it was last known.
    pub at: Vec3,
    pub time: u32,
    pub thief: Option<Place>,
}

/// Where a team believes a battery lies (the fair battery knowledge,
/// `player_bots_behaviour.md` §2), used with the battery cheat off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownBattery {
    pub at: Vec3,
    /// When it was last put right.
    pub time: u32,
    /// Shown on the team's HUD: it follows the battery while it lies about
    /// and goes when the game takes it off the HUD. Else seen by a bot of
    /// the team, and left where it was seen until a bot sees that spot
    /// empty.
    pub hud: bool,
}

/// A known battery's spot for a note: where, and whether the HUD shows it.
fn spot(known: KnownBattery) -> String {
    let at = known.at;
    let how = if known.hud { "on the HUD" } else { "seen" };
    format!("[{:.1}, {:.1}, {:.1}] {how}", at.x, at.y, at.z)
}

/// A teammate's death (the kill feed), kept longer than a danger spot for
/// the spawners' search: a spot where the team keeps dying is worth less.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loss {
    pub at: Vec3,
    pub time: u32,
    /// The spawner the bot was going to look at, if it was.
    pub spawner: Option<usize>,
}

/// Losses are kept this long, ms.
pub const LOSS_KEEP_MS: u32 = 120_000;

/// An enemy a bot of the team saw carrying a battery (a carrier holds it in
/// his hands, in sight).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierSeen {
    pub who: Place,
    pub battery: usize,
    pub at: Vec3,
    pub time: u32,
}

/// Seen carriers are kept this long, ms.
pub const CARRIER_SEEN_KEEP_MS: u32 = 60_000;

/// An enemy carrier's place as a bot of the team sensed it (seen, heard,
/// or the thief cheat), with its error: whether he stays put
/// ([`TeamKnowledge::carrier_still`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierPlace {
    pub who: Place,
    pub at: Vec3,
    pub error_m: f32,
    pub time: u32,
}

/// Carriers' places are kept this long, ms.
pub const CARRIER_PLACES_KEEP_MS: u32 = 30_000;

/// A bot of the team fell near a battery it went for: it was far off its
/// reckoning there (a ledge by the battery; [`TeamKnowledge::fell_near`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fall {
    pub who: Place,
    pub at: Vec3,
    pub time: u32,
}

/// Falls are kept this long, ms; a bot's falls this near in time are one.
pub const FALLS_KEEP_MS: u32 = 120_000;
const FALL_SAME_MS: u32 = 5_000;
/// Falls this near a spot are at it, metres across and up or down.
const FALL_SPOT_M: f32 = 1.5;
const FALL_SPOT_UP_M: f32 = 2.0;

/// A battery an enemy carrier dropped where he died, unseen: the kill feed
/// says he died, the team knew he carried it (a theft of ours, or seen
/// carrying), and the game's HUD shows the dropped battery only to his
/// team. A human walks to where he last saw him; the bots look there.
///
/// Or a battery of ours a thief dropped alive (F2 of the analysis of the
/// owner's matches 0x652d/0x1a09 of 2026-10-16): the HUD told the theft and
/// the container's count, the carrier no longer has it, and the battery is
/// on his team's HUD only. The team looks round its container and where it
/// last sensed him (`theft`: one bot for all these spots).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookSpot {
    pub battery: usize,
    /// His last place the team itself sensed (the killer's sight, else
    /// its last own sight, hearing or callout of him); or a spot of the
    /// ring round our container.
    pub at: Vec3,
    pub time: u32,
    /// Dropped after this (match ms) if nobody has looked.
    pub until: u32,
    /// A spot after a theft whose thief dropped the battery alive.
    pub theft: bool,
}

/// A look spot nobody has looked at is dropped after this, ms.
pub const LOOK_KEEP_MS: u32 = 120_000;

/// What a team's bots share: callouts (one bot's sense, the others' a
/// moment later), danger spots, thefts, and with the battery cheat off
/// what they know of the batteries and the spawners they have looked at.
#[derive(Clone, Debug, Default)]
pub struct TeamKnowledge {
    /// Callouts: (when the others know it, the percept, the places that
    /// took it already as a bit mask).
    callouts: Vec<(u32, Percept, u64)>,
    pub danger: Vec<DangerSpot>,
    /// "Shot from there" callouts of the last 20 s.
    pub shots_from: Vec<ShotFrom>,
    pub thefts: Vec<Theft>,
    /// By battery number.
    pub batteries: Vec<Option<KnownBattery>>,
    /// By battery spawner: when a bot of the team last saw its spot.
    pub spawners_seen: Vec<Option<u32>>,
    /// The team's bots checking spawners now and the spawner each looks
    /// at (the commander's, from the plays it handed out).
    pub searching: Vec<(Place, usize)>,
    /// The team's deaths of the last [`LOSS_KEEP_MS`].
    pub losses: Vec<Loss>,
    /// Enemies seen carrying lately.
    pub carriers_seen: Vec<CarrierSeen>,
    /// Where enemy carriers died with a battery the team cannot see.
    pub looks: Vec<LookSpot>,
    /// Enemy carriers' places of the last [`CARRIER_PLACES_KEEP_MS`].
    pub carrier_places: Vec<CarrierPlace>,
    /// The team's falls by batteries of the last [`FALLS_KEEP_MS`].
    pub falls: Vec<Fall>,
    /// What changed in [`Self::batteries`] and why, for the log (taken by
    /// the bots' notes; [`Self::change_battery`]).
    pub battery_notes: Vec<String>,
    /// The known lying batteries each bot of the team may not take now,
    /// and why (`collect::take_blocks`): a note goes out when it changes.
    pub take_blocked: Vec<(Place, usize, &'static str)>,
}

impl TeamKnowledge {
    /// A bot of the team sensed something: its teammates know it after
    /// `delay_ms`. Only first-hand senses are called out.
    pub fn call_out(&mut self, by: Place, percept: &Percept, delay_ms: u32) {
        if !matches!(percept.source, Source::Seen | Source::Heard) {
            return;
        }
        self.callouts.push((
            percept.time + delay_ms,
            Percept {
                source: Source::Told { by },
                ..*percept
            },
            0,
        ));
    }

    /// The callouts the bot at `me` hears by `now` (not its own), each
    /// once (handed back at every think, a callout came again with its old
    /// time and place over what the bot sensed since), and the old ones
    /// dropped.
    pub fn heard_by(&mut self, me: Place, now: u32) -> Vec<Percept> {
        self.callouts.retain(|(at, _, _)| *at + 3_000 >= now);
        // Places past the mask's 64 take a callout at every think, as
        // before (the memory takes none older than what it knows).
        let bit = (me < 64).then(|| 1u64 << me);
        let mut out = Vec::new();
        for (at, percept, taken) in &mut self.callouts {
            if *at > now
                || percept.source == (Source::Told { by: me })
                || bit.is_some_and(|bit| *taken & bit != 0)
            {
                continue;
            }
            *taken |= bit.unwrap_or(0);
            out.push(*percept);
        }
        out
    }

    /// Danger spots of the last `ms`.
    pub fn danger_within(&self, now: u32, ms: u32) -> impl Iterator<Item = &DangerSpot> {
        self.danger.iter().filter(move |spot| spot.time + ms >= now)
    }

    /// The player at `who` left the match: no callout about it is heard,
    /// and no theft names it. The battery it took is still where it was
    /// last known.
    pub fn forget(&mut self, who: Place) {
        self.forget_callouts(who);
        for theft in &mut self.thefts {
            if theft.thief == Some(who) {
                theft.thief = None;
            }
        }
    }

    /// The enemy at `who` died: no callout about him is heard any more (he
    /// is not where he was; a callout of the moment before his death would
    /// bring him back to the memories that forgot him).
    pub fn forget_callouts(&mut self, who: Place) {
        self.callouts.retain(|(_, percept, _)| percept.who != who);
    }

    /// Where the team believes battery `battery` lies.
    pub fn battery(&self, battery: usize) -> Option<&KnownBattery> {
        self.batteries.get(battery)?.as_ref()
    }

    /// Battery `battery` lies at `at` as far as the team knows (`None`:
    /// it does not know).
    pub fn set_battery(&mut self, battery: usize, known: Option<KnownBattery>) {
        if self.batteries.len() <= battery {
            if known.is_none() {
                return;
            }
            self.batteries.resize(battery + 1, None);
        }
        self.batteries[battery] = known;
    }

    /// [`Self::set_battery`], with a note of the change and `why` when the
    /// team comes to know the battery, forgets it, sees it moved over 1 m,
    /// or it comes onto or goes off the HUD; a spot only put right again
    /// writes nothing.
    pub fn change_battery(
        &mut self,
        battery: usize,
        known: Option<KnownBattery>,
        why: impl FnOnce() -> String,
    ) {
        let was = self.battery(battery).copied();
        let text = match (was, known) {
            (None, Some(now)) => Some(format!(
                "battery {battery} known at {} ({})",
                spot(now),
                why()
            )),
            (Some(then), None) => Some(format!(
                "battery {battery} no longer known at {} ({})",
                spot(then),
                why()
            )),
            (Some(then), Some(now)) if (then.at - now.at).length() > 1.0 || then.hud != now.hud => {
                Some(format!(
                    "battery {battery} known at {}, was {} ({})",
                    spot(now),
                    spot(then),
                    why()
                ))
            }
            _ => None,
        };
        self.battery_notes.extend(text);
        self.set_battery(battery, known);
    }

    /// The batteries on the HUD of team `team`, from two worlds' states
    /// (`player_bots_behaviour.md` §2). The client (PROVEN in 0.21d's
    /// `gather_victory_items_rule::fill_match_hud_state` 0x1FBDF4 and
    /// `victory_item_core::initialize`/`deactivate`, the same in 0.26g's
    /// 0x3140C0, 0x300BEA, 0x30104E, 0x3010A4) shows a lying battery to a
    /// team whose bit is set in the battery's mask; the mask is empty when
    /// the battery appears, set to the picker's team alone on a pickup, to
    /// the carrier's team alone on a drop (a death too), emptied when it is
    /// put in, and a player's spot adds his team. The world carries that
    /// mask (`m_spotted_mask`, `MatchView::marks`; `1 << team`: 1 is team
    /// 0's, 2 team 1's, as the matches of 2026-10-16 show: every drop gave
    /// the carrier's team's alone, never both), and a lying battery is on
    /// our HUD exactly when our bit is set. It stays set when the game puts back a
    /// battery that fell through the floor about 2 m away (level_01's hole
    /// at about (-43.5, 3.8), matches 0x1a09…/0x7298…: team 1 lost its
    /// battery for 125 s and 32 s when the HUD was reckoned from moves).
    /// So:
    /// - a battery dropped by our carrier is on our HUD where it lies,
    ///   wherever the game moves it while it lies;
    /// - one our HUD showed goes from it when anybody picks it up, or when
    ///   it is put in;
    /// - one our teammate carries is known by him (the HUD shows him);
    /// - one only seen by a bot stays where it was seen.
    ///
    /// A view without the masks (a tool or test that gives none) reckons
    /// the HUD from the states: our carrier's drop is ours, and a battery
    /// that moved more than 1 m between two worlds left the HUD.
    pub fn hud_update(&mut self, team: u8, before: &MatchView, after: &MatchView, time: u32) {
        let team_of = |carrier: Option<Place>| carrier.and_then(|p| after.place(p)).map(|p| p.team);
        for (battery, now) in after.batteries.iter().enumerate() {
            let was = before.batteries.get(battery).copied();
            let known = self.battery(battery).copied();
            match *now {
                BatteryState::Free(at) => {
                    let hud = match (after.marks.get(battery), was) {
                        // The client's own HUD.
                        (Some(&mask), _) => mask & (1 << (team & 1)) != 0,
                        // Lying still: as it was.
                        (None, Some(BatteryState::Free(then))) if then.distance_xz(at) <= 1.0 => {
                            known.is_some_and(|k| k.hud)
                        }
                        (None, Some(BatteryState::Carried(carrier))) => {
                            team_of(carrier) == Some(team)
                        }
                        // New, moved unseen, or out of a container.
                        _ => false,
                    };
                    if hud {
                        self.change_battery(battery, Some(KnownBattery { at, time, hud }), || {
                            "the HUD shows it".into()
                        });
                    } else if known.is_some_and(|k| k.hud) {
                        self.change_battery(battery, None, || "the HUD no longer shows it".into());
                    }
                }
                BatteryState::Carried(carrier) => {
                    if known.is_some_and(|k| k.hud) || team_of(carrier) == Some(team) {
                        self.change_battery(battery, None, || match carrier {
                            Some(p) => format!("carried by player {p}"),
                            None => "carried".into(),
                        });
                    }
                }
                BatteryState::Home(id) => {
                    if known.is_some_and(|k| k.hud) {
                        self.change_battery(battery, None, || format!("put in container {id}"));
                    }
                }
            }
        }
    }

    /// When a bot of the team last saw spawner `spawner`'s spot.
    pub fn spawner_seen(&self, spawner: usize) -> Option<u32> {
        self.spawners_seen.get(spawner).copied().flatten()
    }

    pub fn saw_spawner(&mut self, spawner: usize, now: u32) {
        if self.spawners_seen.len() <= spawner {
            self.spawners_seen.resize(spawner + 1, None);
        }
        self.spawners_seen[spawner] = Some(now);
    }

    pub fn forget_old(&mut self, now: u32) {
        self.danger.retain(|spot| spot.time + 30_000 >= now);
        self.shots_from.retain(|shot| shot.time + 20_000 >= now);
        self.thefts.retain(|theft| theft.time + 60_000 >= now);
        self.losses.retain(|loss| loss.time + LOSS_KEEP_MS >= now);
        self.carriers_seen
            .retain(|seen| seen.time + CARRIER_SEEN_KEEP_MS >= now);
        self.looks.retain(|look| look.until >= now);
        self.carrier_places
            .retain(|p| p.time + CARRIER_PLACES_KEEP_MS >= now);
        self.falls.retain(|f| f.time + FALLS_KEEP_MS >= now);
    }

    /// `who`, going for a battery lying at `at`, fell near it (once in
    /// [`FALL_SAME_MS`] a bot).
    pub fn fell_near(&mut self, who: Place, at: Vec3, time: u32) {
        if self
            .falls
            .iter()
            .any(|f| f.who == who && f.time + FALL_SAME_MS > time)
        {
            return;
        }
        self.falls.push(Fall { who, at, time });
    }

    /// Whether a battery lying at `at` is set aside for the team: `count`
    /// falls at it within `window_ms`, the last less than `aside_ms` ago
    /// (0x1b8ee975: battery 6 lay 455 s by a ledge, 166 Collect plays, 17
    /// falls, 7 of them deaths).
    pub fn fallen_at(
        &self,
        at: Vec3,
        now: u32,
        count: usize,
        window_ms: u32,
        aside_ms: u32,
    ) -> bool {
        let falls: Vec<u32> = self
            .falls
            .iter()
            .filter(|f| {
                f.at.distance_xz(at) <= FALL_SPOT_M
                    && (f.at.y - at.y).abs() <= FALL_SPOT_UP_M
                    && f.time + window_ms >= now
            })
            .map(|f| f.time)
            .collect();
        count > 0
            && falls.len() >= count
            && falls
                .iter()
                .max()
                .is_some_and(|&last| now < last + aside_ms)
    }

    /// A bot of the team sensed the enemy carrier at `who` at `at`, within
    /// `error_m`. `carrying(place)`: whether a place carries now; the
    /// places of one who no longer does are dropped (a new carry starts
    /// afresh).
    pub fn carrier_at(
        &mut self,
        who: Place,
        at: Vec3,
        error_m: f32,
        time: u32,
        carrying: impl Fn(Place) -> bool,
    ) {
        self.carrier_places.retain(|p| carrying(p.who));
        self.carrier_places.push(CarrierPlace {
            who,
            at,
            error_m,
            time,
        });
    }

    /// Where the enemy carrier at `who` has stayed: his places of the last
    /// `window_ms` and more (the first at least that old) all lie within
    /// their error and `still_m` of their middle, three of them at least;
    /// that middle. A thief cheat's place (10 m off) is drawn every 3 s,
    /// so a carrier who walks home on it is seen to move within seconds.
    pub fn carrier_still(&self, who: Place, now: u32, window_ms: u32, still_m: f32) -> Option<Vec3> {
        let places: Vec<&CarrierPlace> = self
            .carrier_places
            .iter()
            .filter(|p| p.who == who)
            .collect();
        let first = places.iter().map(|p| p.time).min()?;
        if places.len() < 3 || first + window_ms > now {
            return None;
        }
        let n = places.len() as f32;
        let sum = places.iter().fold(Vec3::ZERO, |sum, p| sum + p.at);
        let middle = sum * (1.0 / n);
        places
            .iter()
            .all(|p| p.at.distance_xz(middle) <= p.error_m + still_m)
            .then_some(middle)
    }

    /// A bot of the team saw the enemy at `who` carrying `battery`.
    pub fn saw_carrying(&mut self, who: Place, battery: usize, at: Vec3, time: u32) {
        self.carriers_seen.retain(|seen| seen.who != who);
        self.carriers_seen.push(CarrierSeen {
            who,
            battery,
            at,
            time,
        });
    }

    /// Whether the team knows the enemy at `who` carried `battery`: he
    /// stole it from us (the HUD's theft), or a bot saw him carrying it.
    pub fn knew_carrying(&self, who: Place, battery: usize) -> bool {
        self.thefts
            .iter()
            .any(|t| t.battery == battery && t.thief == Some(who))
            || self
                .carriers_seen
                .iter()
                .any(|s| s.who == who && s.battery == battery)
    }

    /// Where the team looks for `battery`, dropped by a carrier it lost.
    pub fn add_look(&mut self, look: LookSpot) {
        self.looks.retain(|l| l.battery != look.battery);
        self.looks.push(look);
    }

    /// Where the team looks for `battery`, dropped by a thief alive: the
    /// spots `at`, until `until`.
    pub fn add_theft_looks(&mut self, battery: usize, at: &[Vec3], time: u32, until: u32) {
        self.looks.retain(|l| l.battery != battery);
        self.looks.extend(at.iter().map(|&at| LookSpot {
            battery,
            at,
            time,
            until,
            theft: true,
        }));
    }

    /// The look spots still worth a look for team `team`: not the ones
    /// whose battery the team knows of now (seen lying, on its HUD, home,
    /// or carried by a teammate).
    pub fn settle_looks(&mut self, team: u8, view: &MatchView) {
        if self.looks.is_empty() {
            return;
        }
        let known: Vec<bool> = (0..view.batteries.len())
            .map(|b| {
                self.battery(b).is_some()
                    || match view.batteries[b] {
                        BatteryState::Home(_) => true,
                        BatteryState::Carried(Some(p)) => {
                            view.place(p).is_some_and(|p| p.team == team)
                        }
                        _ => false,
                    }
            })
            .collect();
        self.looks
            .retain(|look| !known.get(look.battery).copied().unwrap_or(true));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(who: Place, time: u32) -> Percept {
        Percept {
            who,
            at: Vec3::new(10.0, 0.0, 0.0),
            error_m: 0.0,
            time,
            source: Source::Seen,
            busy: false,
        }
    }

    /// A carrier the thief cheat places within its 10 m every 3 s is still
    /// once 20 s of places lie within their error and 3 m of their middle;
    /// one who walks home is not, nor one known for less than 20 s, and a
    /// new carry starts afresh.
    #[test]
    fn a_carrier_is_still_when_his_places_keep_within_their_error() {
        let mut team = TeamKnowledge::default();
        let at = Vec3::new(50.0, 0.0, 30.0);
        let offsets = [(9.0f32, 0.0f32), (-6.0, 6.0), (0.0, -9.0), (5.0, 5.0)];
        for k in 0..8u32 {
            let (dx, dz) = offsets[k as usize % 4];
            team.carrier_at(1, at + Vec3::new(dx, 0.0, dz), 10.0, k * 3_000, |_| true);
        }
        let still = team.carrier_still(1, 21_000, 20_000, 3.0).expect("still");
        assert!(still.distance_xz(at) < 3.0, "{still:?}");
        assert_eq!(team.carrier_still(1, 19_000, 20_000, 3.0), None, "too short");
        assert_eq!(team.carrier_still(2, 21_000, 20_000, 3.0), None);
        // Walking home at 1.5 m/s on the same cheat: not still.
        let mut walking = TeamKnowledge::default();
        for k in 0..8u32 {
            let (dx, dz) = offsets[k as usize % 4];
            let on = Vec3::new(4.5 * k as f32, 0.0, 0.0);
            walking.carrier_at(1, at + on + Vec3::new(dx, 0.0, dz), 10.0, k * 3_000, |_| true);
        }
        assert_eq!(walking.carrier_still(1, 21_000, 20_000, 3.0), None);
        // He no longer carries: the next place starts a new carry.
        team.carrier_at(3, at, 0.0, 22_000, |p| p != 1);
        assert_eq!(team.carrier_still(1, 22_000, 20_000, 3.0), None);
    }

    #[test]
    fn a_sight_is_a_seen_track_only_after_the_noticing_delay() {
        let mut memory = Memory::default();
        memory.update(&[seen(3, 1_000)], 1_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 1_000).is_none());
        assert!(memory.seen(3, 1_200).is_none());
        memory.update(&[seen(3, 1_300)], 1_300, |_| 300, 3_000, 20_000);
        assert_eq!(memory.seen(3, 1_300).map(|t| t.who()), Some(3));
        // Out of sight: no seen track, but it is still known.
        memory.update(&[], 2_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 2_000).is_none());
        assert_eq!(memory.tracks().len(), 1);
        // Seen again: a new noticing delay.
        memory.update(&[seen(3, 2_500)], 2_500, |_| 300, 3_000, 20_000);
        assert!(memory.seen(3, 2_600).is_none());
        // A heard report does not make a seen track, and old ones go.
        let heard = Percept {
            source: Source::Heard,
            ..seen(4, 3_000)
        };
        memory.update(&[heard], 3_000, |_| 300, 3_000, 20_000);
        assert!(memory.seen(4, 9_000).is_none());
        memory.update(&[], 30_000, |_| 300, 3_000, 20_000);
        assert!(memory.tracks().is_empty());
    }

    fn battery_view(batteries: Vec<BatteryState>) -> MatchView {
        use crate::v026g::bots::view::{Mode, PlaceView};
        let place = |place: Place, team: u8| PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            known_at: 0,
        };
        MatchView {
            clock: 10_000,
            mode: Mode::Batteries,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places: vec![place(0, 0), place(1, 1)],
            batteries,
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 10_000,
            can_respawn: [true, true],
            next_battery: None,
        }
    }

    /// The client's HUD mask (`hud_update`): a battery that appears is on
    /// no HUD; one our carrier dropped is on ours where it lies, not on
    /// theirs; picked up by them it leaves our HUD, and put in it is on
    /// none. A battery only seen stays where it was seen.
    #[test]
    fn the_hud_shows_what_the_team_last_carried_and_drops_it_when_taken() {
        let (a, b) = (Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 5.0));
        let mut ours = TeamKnowledge::default();
        let mut theirs = TeamKnowledge::default();
        let step = |ours: &mut TeamKnowledge, theirs: &mut TeamKnowledge, from, to| {
            let (before, after) = (battery_view(from), battery_view(to));
            ours.hud_update(0, &before, &after, 1);
            theirs.hud_update(1, &before, &after, 1);
        };
        // It appears: nobody's HUD.
        step(&mut ours, &mut theirs, vec![], vec![BatteryState::Free(a)]);
        assert!(ours.battery(0).is_none() && theirs.battery(0).is_none());
        // Our carrier (place 0) takes it and drops it at b (he died).
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(a)],
            vec![BatteryState::Carried(Some(0))],
        );
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Carried(Some(0))],
            vec![BatteryState::Free(b)],
        );
        assert_eq!(ours.battery(0).map(|k| (k.at, k.hud)), Some((b, true)));
        assert!(theirs.battery(0).is_none());
        // Lying still: still on our HUD.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(b)],
            vec![BatteryState::Free(b)],
        );
        assert!(ours.battery(0).is_some_and(|k| k.hud));
        // Their player takes it: off our HUD; dropped, on theirs.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(b)],
            vec![BatteryState::Carried(Some(1))],
        );
        assert!(ours.battery(0).is_none());
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Carried(Some(1))],
            vec![BatteryState::Free(a)],
        );
        assert!(ours.battery(0).is_none());
        assert!(theirs.battery(0).is_some_and(|k| k.hud && k.at == a));
        // Put in: on no HUD.
        step(
            &mut ours,
            &mut theirs,
            vec![BatteryState::Free(a)],
            vec![BatteryState::Home(1)],
        );
        assert!(theirs.battery(0).is_none());
        // Seen by one of our bots, then taken by them: still believed
        // where it was seen (no HUD said otherwise); taken by our own
        // teammate, known to be his.
        ours.set_battery(
            1,
            Some(KnownBattery {
                at: a,
                time: 1,
                hud: false,
            }),
        );
        let lying = vec![BatteryState::Home(1), BatteryState::Free(a)];
        let taken = vec![BatteryState::Home(1), BatteryState::Carried(Some(1))];
        step(&mut ours, &mut theirs, lying.clone(), taken);
        assert_eq!(ours.battery(1).map(|k| k.at), Some(a));
        let ours_taken = vec![BatteryState::Home(1), BatteryState::Carried(Some(0))];
        step(&mut ours, &mut theirs, lying, ours_taken);
        assert!(ours.battery(1).is_none());
    }

    /// Matches 0x1a09…/0x7298… (level_01's floor hole at about (-43.5,
    /// 3.8)): team 1's carrier died, the battery fell (y 19.6 -> -86) and
    /// the game put it back 2 m away; the world's mask stayed 2 (team 1's
    /// HUD) all the while, and so does the team's knowledge. Team 0's HUD
    /// never had it. An enemy who picks it up and drops it between two
    /// worlds within a metre takes it off our HUD (the mask says his team).
    #[test]
    fn the_hud_follows_the_worlds_mask_through_a_fall_and_a_reset() {
        let marked = |batteries: Vec<BatteryState>, marks: Vec<u32>| MatchView {
            marks,
            ..battery_view(batteries)
        };
        let mut teams = [TeamKnowledge::default(), TeamKnowledge::default()];
        let mut step = |from: &MatchView, to: &MatchView, time| {
            for (team, knowledge) in teams.iter_mut().enumerate() {
                knowledge.hud_update(team as u8, from, to, time);
            }
            teams.clone().map(|k| k.battery(0).copied())
        };
        let dropped = Vec3::new(-43.58, 19.55, 3.80);
        let fallen = Vec3::new(-43.58, -86.0, 3.80);
        let reset = Vec3::new(-41.6, 21.6, 3.5);
        let spawned = marked(vec![BatteryState::Free(Vec3::ZERO)], vec![0]);
        let carried = marked(vec![BatteryState::Carried(Some(1))], vec![0]);
        let lying = |at| marked(vec![BatteryState::Free(at)], vec![2]);
        assert_eq!(step(&spawned, &spawned, 1), [None, None]);
        step(&spawned, &carried, 2);
        let [ours, theirs] = step(&carried, &lying(dropped), 3);
        assert!(ours.is_none());
        assert_eq!(theirs.map(|k| (k.at, k.hud)), Some((dropped, true)));
        // It falls, and the game puts it back 2 m away: still on team 1's
        // HUD, where it lies now.
        step(&lying(dropped), &lying(fallen), 4);
        let [ours, theirs] = step(&lying(fallen), &lying(reset), 5);
        assert!(ours.is_none());
        assert!(dropped.distance_xz(reset) > 1.9);
        assert_eq!(
            theirs.map(|k| (k.at, k.time, k.hud)),
            Some((reset, 5, true))
        );
        let [_, theirs] = step(&lying(reset), &lying(reset), 6);
        assert_eq!(theirs.map(|k| k.at), Some(reset));
        // Team 0's player picks it up and drops it within a metre between
        // two worlds: the mask is team 0's now.
        let near = reset + Vec3::new(0.5, 0.0, 0.0);
        let theirs_now = marked(vec![BatteryState::Free(near)], vec![1]);
        let [ours, theirs] = step(&lying(reset), &theirs_now, 7);
        assert_eq!(ours.map(|k| (k.at, k.hud)), Some((near, true)));
        assert!(theirs.is_none());
        // Put in: on no HUD.
        let home = marked(vec![BatteryState::Home(0)], vec![0]);
        assert_eq!(step(&theirs_now, &home, 8), [None, None]);
    }

    #[test]
    fn callouts_reach_teammates_later_and_never_cheats() {
        let mut team = TeamKnowledge::default();
        team.call_out(1, &seen(5, 1_000), 1_000);
        let cheat = Percept {
            source: Source::Cheat(CheatKind::Radar),
            ..seen(6, 1_000)
        };
        team.call_out(1, &cheat, 1_000);
        assert!(team.heard_by(2, 1_500).is_empty());
        let heard = team.heard_by(2, 2_000);
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].source, Source::Told { by: 1 });
        assert!(team.heard_by(1, 2_000).is_empty(), "not its own");
    }

    /// Match 0x5402a730: the human's track in the enemy bots' memories was
    /// told 252 times, heard 3. A teammate's callout (older, or weaker
    /// than a fresh report) does not replace what the bot heard itself; a
    /// cheat does not replace a fresh callout; a stale report gives way to
    /// a newer one of any kind.
    #[test]
    fn a_callout_does_not_wipe_the_bots_own_hearing() {
        let at = |x: f32| Vec3::new(x, 0.0, 0.0);
        let report = |source, x, time| Percept {
            source,
            at: at(x),
            ..seen(7, time)
        };
        let told = Source::Told { by: 2 };
        let mut memory = Memory::default();
        let update = |memory: &mut Memory, p: Percept, now| {
            memory.update(&[p], now, |_| 300, 3_000, 20_000);
            let t = memory.tracks()[0];
            (t.source, t.at.x, t.time)
        };
        assert_eq!(
            update(&mut memory, report(Source::Heard, 1.0, 5_000), 5_000),
            (Source::Heard, 1.0, 5_000)
        );
        // A callout of a moment before (its old time and place): kept.
        assert_eq!(
            update(&mut memory, report(told, 2.0, 4_200), 5_200),
            (Source::Heard, 1.0, 5_000)
        );
        // A callout newer but weaker while the hearing is fresh: kept.
        assert_eq!(
            update(&mut memory, report(told, 3.0, 5_100), 5_400),
            (Source::Heard, 1.0, 5_000)
        );
        // Heard again: the newer hearing.
        assert_eq!(
            update(&mut memory, report(Source::Heard, 4.0, 5_500), 5_500),
            (Source::Heard, 4.0, 5_500)
        );
        // The hearing stale (1.5 s): a newer callout takes it.
        assert_eq!(
            update(&mut memory, report(told, 5.0, 6_100), 7_100),
            (told, 5.0, 6_100)
        );
        // A cheat does not replace a fresh callout, then does.
        let radar = Source::Cheat(CheatKind::Radar);
        assert_eq!(
            update(&mut memory, report(radar, 6.0, 7_200), 7_200),
            (told, 5.0, 6_100)
        );
        assert_eq!(
            update(&mut memory, report(radar, 6.0, 7_700), 7_700),
            (radar, 6.0, 7_700)
        );
        // A sight replaces anything; a cheat's newer place keeps no sight
        // alive.
        assert_eq!(
            update(&mut memory, report(Source::Seen, 8.0, 7_800), 7_800),
            (Source::Seen, 8.0, 7_800)
        );
        update(&mut memory, report(Source::Seen, 8.0, 8_100), 8_100);
        assert!(memory.seen(7, 8_150).is_some());
        let thief = Source::Cheat(CheatKind::Thief);
        update(&mut memory, report(thief, 9.0, 8_300), 8_300);
        assert!(memory.seen(7, 8_550).is_none(), "no sight since 8.1 s");
    }

    #[test]
    fn a_callout_is_heard_once_and_a_dead_enemys_not_at_all() {
        let mut team = TeamKnowledge::default();
        team.call_out(1, &seen(5, 1_000), 1_000);
        team.call_out(1, &seen(6, 1_000), 1_000);
        assert_eq!(team.heard_by(2, 2_000).len(), 2);
        assert!(team.heard_by(2, 2_200).is_empty(), "taken already");
        assert_eq!(team.heard_by(3, 2_200).len(), 2, "another bot's own");
        team.forget_callouts(5);
        let left: Vec<Place> = team.heard_by(4, 2_300).iter().map(|p| p.who).collect();
        assert_eq!(left, vec![6]);
    }

    /// Three falls of the team at a battery within 120 s set it aside for
    /// 60 s after the last; one bot's fall is counted once in 5 s, falls at
    /// another battery or long ago do not count, and `count` 0 is off.
    #[test]
    fn falls_at_a_battery_set_it_aside_for_a_while() {
        let mut team = TeamKnowledge::default();
        let at = Vec3::new(5.0, 35.5, 60.0);
        let aside = |team: &TeamKnowledge, now| team.fallen_at(at, now, 3, 120_000, 60_000);
        team.fell_near(1, at, 10_000);
        team.fell_near(1, at, 12_000);
        team.fell_near(2, Vec3::new(20.0, 35.5, 60.0), 13_000);
        team.fell_near(3, at + Vec3::new(0.0, 3.0, 0.0), 14_000);
        team.fell_near(2, at + Vec3::new(1.0, 0.0, 0.0), 20_000);
        assert!(!aside(&team, 21_000), "two falls here: {:?}", team.falls);
        team.fell_near(1, at, 30_000);
        assert!(aside(&team, 31_000));
        assert!(aside(&team, 89_000));
        assert!(!aside(&team, 91_000), "60 s after the last");
        assert!(!team.fallen_at(at, 31_000, 0, 120_000, 60_000), "off");
        // The first fall out of the window: two left.
        assert!(!team.fallen_at(at, 31_000, 3, 15_000, 60_000));
    }
}
