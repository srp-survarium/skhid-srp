//! A229 (the coordinator, 2026-11-01): where a bot looks while it walks a play. A222 (the live bots, read-only) found that a
//! quarter of the killed bots fired nothing, 68 % of those died within about 1 s of the human's first hit, and at that hit
//! their view was a median 30 degrees off him: seen first while walking a team play facing elsewhere. Our tip's plays mostly
//! walk with `Look::Path` (the view along the route) and the reflex turns the view only to an enemy it senses, a sound, a
//! smoke wait or a throw; nothing turns it along the held lanes the team knows (K1, `threat::Picture::held`). Two counts:
//!
//! - **walking in a held lane**, by play: a living bot that walks (a step with a goal to walk to, over 0.5 m/s, neither the aim
//!   nor the fire key down) inside a held line of its own team (`HeldLine::covers`): the angle between its view and the
//!   bearing to that line's spot (the nearest such line), in bands (15, 45, 90 degrees), and the ticks it walked in all;
//! - **the first enemy hit of a life**, by play and by shooter (the stand-in human or a bot): the victim's view off the
//!   shooter at that hit (bands as above), whether it pressed fire before it died or 5 s passed, and whether it died within
//!   1 s of the hit (A222's window); apart, the victims that walked at the hit by what each knew of its shooter (in his
//!   held lane; his place known to its own or its team's senses within 10 s, `Track::own`; nothing): the look each would
//!   have needed; and any victim that stood in a held lane of its team whose shooter is the one who hit it.
//!
//! The angles are on the ground (yaw only). Nothing decides by these counts.

use super::*;
use crate::v026g::bots::kin;
use crate::v026g::bots::threat::HeldLine;

/// The view bands, degrees: up to each, then past the last.
const BANDS_DEG: [f32; 3] = [15.0, 45.0, 90.0];
/// A walk is a step with a goal over this, m/s (a body standing, or pressing into a wall, is under it).
const WALK_MPS: f32 = 0.5;
/// A first hit is followed this long for a shot of the victim's own, ms.
const FOLLOW_MS: u32 = 5_000;
/// A222's window: a death this soon after the first hit, ms.
const QUICK_MS: u32 = 1_000;

/// The band of an angle off, degrees.
fn band(off_deg: f32) -> usize {
    BANDS_DEG.iter().position(|&b| off_deg <= b).unwrap_or(BANDS_DEG.len())
}

/// The angle between the view's yaw and the bearing from `from` to `to`, degrees, 0 to 180.
fn off_deg(yaw: f32, from: Vec3, to: Vec3) -> f32 {
    let bearing = kin::yaw_of([to.x - from.x, to.z - from.z]);
    let d = (yaw - bearing + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    d.abs().to_degrees()
}

/// A walk: a step with a goal to walk to, over [`WALK_MPS`], neither the aim nor the fire key down.
fn walks(rec: &Rec) -> bool {
    rec.travel && rec.speed > WALK_MPS && rec.keys & (key::AIM | key::FIRE) == 0
}

/// The band of the view off the spot of the nearest held line that covers `feet`, if one does.
fn lane_band(held: &[HeldLine], feet: Vec3, yaw: f32) -> Option<usize> {
    held.iter()
        .filter(|l| l.covers(feet))
        .min_by(|a, b| a.at.distance_xz(feet).total_cmp(&b.at.distance_xz(feet)))
        .map(|line| band(off_deg(yaw, feet, line.at)))
}

/// What a walking victim knew of the one who hit it, at the hit: the look each case asks of a fix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Knew {
    /// It stood in a held lane of its team whose shooter he is (a watch of the known lanes meets him).
    HisLane,
    /// Not in his lane, but its own or its team's senses placed him within [`KNOWN_MS`] (a look at where he was meets him).
    WhereHeWas,
    /// Neither.
    Nothing,
}

impl Knew {
    const ALL: [Knew; 3] = [Knew::HisLane, Knew::WhereHeWas, Knew::Nothing];

    fn of(in_his_lane: bool, placed: bool) -> Self {
        match (in_his_lane, placed) {
            (true, _) => Knew::HisLane,
            (false, true) => Knew::WhereHeWas,
            (false, false) => Knew::Nothing,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Knew::HisLane => "his_lane",
            Knew::WhereHeWas => "where_he_was",
            Knew::Nothing => "nothing",
        }
    }
}

/// A known enemy's place this fresh counts, ms (as the detour count's `KNOWN_MS`).
const KNOWN_MS: u32 = 10_000;

/// A first hit of a life, followed until the victim's death or [`FOLLOW_MS`].
#[derive(Clone, Copy, Debug)]
struct FirstHit {
    at: u32,
    play: u16,
    by_human: bool,
    band: usize,
    fired: bool,
    /// The victim walked at the hit (as the walking count's ticks), and what it knew of its shooter.
    walking: Option<Knew>,
    /// The victim stood in a held lane of its team whose shooter is the one who hit it: the team knew where he shoots from.
    in_his_lane: bool,
}

/// Counts by play name.
#[derive(Clone, Debug, Default)]
struct ByPlay {
    /// Walking ticks in all, and those inside a held lane by the view band off its spot.
    walk_ticks: u64,
    lane_ticks: [u64; BANDS_DEG.len() + 1],
    /// First hits by view band; of them, fired before the end, died within [`QUICK_MS`] (by the stand-in human's apart).
    first_hits: [u64; BANDS_DEG.len() + 1],
    fired: u64,
    quick_deaths: u64,
    human_first_hits: [u64; BANDS_DEG.len() + 1],
    human_quick_deaths: u64,
    /// The first hits on a walking victim by what it knew of its shooter ([`Knew::ALL`]'s order), and on any victim in its
    /// shooter's held lane: by band, died within [`QUICK_MS`].
    walking_first_hits: [[u64; BANDS_DEG.len() + 1]; 3],
    walking_quick_deaths: [u64; 3],
    lane_first_hits: [u64; BANDS_DEG.len() + 1],
    lane_quick_deaths: u64,
}

#[derive(Default)]
pub(super) struct LookCounts {
    human: Option<Place>,
    /// Each team's held lines at the last commander picture read.
    held: [Vec<HeldLine>; 2],
    /// The open first hit of each place's life.
    open: Vec<Option<FirstHit>>,
    /// Whether each place has had its life's first hit (reset when it comes alive again).
    hit_this_life: Vec<bool>,
    /// The enemies each bot's own or its team's senses placed within [`KNOWN_MS`], at its last read (`Track::own`).
    placed: Vec<Vec<Place>>,
    hits_seen: usize,
    kills_seen: usize,
    by_play: BTreeMap<u16, ByPlay>,
}

impl LookCounts {
    pub(super) fn new(n: usize) -> Self {
        Self { open: vec![None; n], hit_this_life: vec![false; n], placed: vec![Vec::new(); n], ..Self::default() }
    }

    pub(super) fn set_human(&mut self, human: Option<Place>) {
        self.human = human;
    }

    fn close(&mut self, place: Place, died_at: Option<u32>) {
        let Some(hit) = self.open.get_mut(place).and_then(Option::take) else {
            return;
        };
        let counts = self.by_play.entry(hit.play).or_default();
        counts.first_hits[hit.band] += 1;
        counts.fired += u64::from(hit.fired);
        let quick = died_at.is_some_and(|t| t.saturating_sub(hit.at) <= QUICK_MS);
        counts.quick_deaths += u64::from(quick);
        if hit.by_human {
            counts.human_first_hits[hit.band] += 1;
            counts.human_quick_deaths += u64::from(quick);
        }
        if let Some(knew) = hit.walking {
            counts.walking_first_hits[knew as usize][hit.band] += 1;
            counts.walking_quick_deaths[knew as usize] += u64::from(quick);
        }
        if hit.in_his_lane {
            counts.lane_first_hits[hit.band] += 1;
            counts.lane_quick_deaths += u64::from(quick);
        }
    }
}

impl Topic for LookCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if place >= self.placed.len() {
            self.placed.resize(place + 1, Vec::new());
        }
        let placed = &mut self.placed[place];
        placed.clear();
        if let Some(bot) = bot {
            placed.extend(
                bot.memory
                    .tracks()
                    .iter()
                    .filter(|t| t.own.is_some_and(|(_, when)| tick.wrapping_sub(when) <= KNOWN_MS))
                    .map(|t| t.who),
            );
        }
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        for (team, held) in self.held.iter_mut().enumerate() {
            *held = bots.teams.get(team).map(|state| state.knowledge.threat.held.clone()).unwrap_or_default();
        }
    }

    fn body(&mut self, run: &Run, at: &BodyAt<'_>, _rays: &mut u64) {
        let (rec, place) = (at.rec, at.place);
        if place >= self.open.len() {
            self.open.resize(place + 1, None);
            self.hit_this_life.resize(place + 1, false);
        }
        if rec.alive && !at.prev.alive {
            self.hit_this_life[place] = false;
        }
        // The stand-in human is a bot place: his own walks are not a bot's.
        if !rec.alive || !run.is_bot.get(place).copied().unwrap_or(false) || self.human == Some(place) {
            return;
        }
        if let Some(hit) = self.open[place].as_mut()
            && rec.keys & key::FIRE != 0
        {
            hit.fired = true;
        }
        if !walks(rec) {
            return;
        }
        let team = usize::from(at.game.bodies[place].team).min(1);
        let lane = lane_band(&self.held[team], rec.feet, rec.yaw);
        let counts = self.by_play.entry(rec.play).or_default();
        counts.walk_ticks += 1;
        if let Some(lane) = lane {
            counts.lane_ticks[lane] += 1;
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        for &(t, shooter, victim) in &run.hits[self.hits_seen.min(run.hits.len())..] {
            let (Some(v), Some(s)) = (game.bodies.get(victim), game.bodies.get(shooter)) else {
                continue;
            };
            if v.team == s.team
                || !run.is_bot.get(victim).copied().unwrap_or(false)
                || self.human == Some(victim)
                || self.hit_this_life.get(victim).copied().unwrap_or(true)
            {
                continue;
            }
            let (Some(vr), Some(sr)) = (run.recs[victim].last(), run.recs[shooter].last()) else {
                continue;
            };
            self.hit_this_life[victim] = true;
            let fired = vr.keys & key::FIRE != 0;
            let held = &self.held[usize::from(v.team).min(1)];
            let in_his_lane = held.iter().any(|l| l.shooter == shooter && l.covers(vr.feet));
            let placed = self.placed.get(victim).is_some_and(|p| p.contains(&shooter));
            self.open[victim] = Some(FirstHit {
                at: t,
                play: vr.play,
                by_human: self.human == Some(shooter),
                band: band(off_deg(vr.yaw, vr.feet, sr.feet)),
                fired,
                walking: walks(vr).then(|| Knew::of(in_his_lane, placed)),
                in_his_lane,
            });
        }
        self.hits_seen = run.hits.len();
        let deaths: Vec<(u32, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].iter().map(|&(t, _, v)| (t, v)).collect();
        self.kills_seen = run.kills.len();
        for (t, victim) in deaths {
            self.close(victim, Some(t));
        }
        let now = game.tick;
        let stale: Vec<Place> =
            (0..self.open.len()).filter(|&p| self.open[p].is_some_and(|h| now.saturating_sub(h.at) > FOLLOW_MS)).collect();
        for place in stale {
            self.close(place, None);
        }
    }

    fn parts(&mut self, run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let open: Vec<Place> = (0..self.open.len()).filter(|&p| self.open[p].is_some()).collect();
        for place in open {
            self.close(place, None);
        }
        let name = |play: u16| run.plays.get(usize::from(play)).cloned().unwrap_or_else(|| "none".to_owned());
        let mut json = serde_json::Map::new();
        let mut lines = vec![format!(
            "owner-test look (A229: the view while walking a play; bands up to {BANDS_DEG:?} deg, then past): by play, walking ticks, \
             of them in a held lane of the team by the view off its spot; first enemy hits of a life by the view off the shooter, \
             fired before death or 5 s, died within 1 s (the stand-in human's apart; the walking victims' by what each knew of its \
             shooter: in his held lane, his place known within 10 s, nothing; any victim in its shooter's held lane apart)"
        )];
        // A walking victim's first hits by what it knew of its shooter: by band, died within 1 s.
        let walking = |c: &ByPlay| -> serde_json::Map<String, serde_json::Value> {
            Knew::ALL
                .iter()
                .map(|&k| {
                    let by = serde_json::json!({ "by_band": c.walking_first_hits[k as usize], "died_1s": c.walking_quick_deaths[k as usize] });
                    (k.name().to_owned(), by)
                })
                .collect()
        };
        for (play, c) in &self.by_play {
            let play = name(*play);
            json.insert(
                play.clone(),
                serde_json::json!({
                    "walk_ticks": c.walk_ticks,
                    "lane_ticks_by_band": c.lane_ticks,
                    "first_hits_by_band": c.first_hits,
                    "first_hit_fired": c.fired,
                    "first_hit_died_1s": c.quick_deaths,
                    "human_first_hits_by_band": c.human_first_hits,
                    "human_first_hit_died_1s": c.human_quick_deaths,
                    "walking_first_hits": walking(c),
                    "lane_first_hits_by_band": c.lane_first_hits,
                    "lane_first_hit_died_1s": c.lane_quick_deaths,
                }),
            );
            lines.push(format!(
                "  {play}: walking {} ticks, in a held lane {:?}; first hits {:?}, fired {}, died within 1 s {}; by the human {:?}, died within 1 s {}; \
                 walking by what it knew of him {}; in his held lane {:?}, died within 1 s {}",
                c.walk_ticks,
                c.lane_ticks,
                c.first_hits,
                c.fired,
                c.quick_deaths,
                c.human_first_hits,
                c.human_quick_deaths,
                Knew::ALL
                    .iter()
                    .map(|&k| format!("{} {:?} ({})", k.name(), c.walking_first_hits[k as usize], c.walking_quick_deaths[k as usize]))
                    .collect::<Vec<_>>()
                    .join(", "),
                c.lane_first_hits,
                c.lane_quick_deaths
            ));
        }
        vec![("look", Part::new(serde_json::json!({ "look": json }), lines.join("\n")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_off_a_bearing_is_on_the_ground_and_wraps() {
        let at = Vec3::new(0.0, 0.0, 0.0);
        let ahead = Vec3::new(0.0, 5.0, 10.0);
        let yaw = kin::yaw_of([0.0, 1.0]);
        assert!(off_deg(yaw, at, ahead) < 0.01, "straight ahead, whatever the height");
        assert!((off_deg(yaw, at, Vec3::new(10.0, 0.0, 0.0)) - 90.0).abs() < 0.01);
        assert!((off_deg(yaw, at, Vec3::new(0.0, 0.0, -10.0)) - 180.0).abs() < 0.01, "behind");
        assert_eq!([band(10.0), band(15.0), band(30.0), band(90.0), band(91.0)], [0, 0, 1, 2, 3]);
    }

    #[test]
    fn a_walk_in_a_held_lane_is_banded_by_the_view_off_the_nearest_spot() {
        // Two shooters at the ends of one lane along z: the walker at z 20 is in both, the one at z 30 nearer.
        let near = HeldLine::for_test(1, 7, Vec3::new(0.0, 0.0, 30.0), &[Vec3::new(0.0, 0.0, 0.0)], None);
        let far = HeldLine::for_test(2, 8, Vec3::new(0.0, 0.0, 0.0), &[Vec3::new(0.0, 0.0, 40.0)], None);
        let held = [far, near];
        let feet = Vec3::new(0.0, 0.0, 20.0);
        assert_eq!(lane_band(&held, feet, kin::yaw_of([0.0, 1.0])), Some(0), "facing the nearer spot");
        assert_eq!(lane_band(&held, feet, kin::yaw_of([1.0, 0.0])), Some(2), "side on");
        assert_eq!(lane_band(&held, feet, kin::yaw_of([0.0, -1.0])), Some(3), "facing the farther one, the nearer behind");
        assert_eq!(lane_band(&held, Vec3::new(30.0, 0.0, 20.0), 0.0), None, "out of every lane");
        assert_eq!(lane_band(&[], feet, 0.0), None);
    }

    #[test]
    fn what_a_walking_victim_knew_of_its_shooter_puts_his_lane_first() {
        assert_eq!(Knew::of(true, true), Knew::HisLane);
        assert_eq!(Knew::of(true, false), Knew::HisLane);
        assert_eq!(Knew::of(false, true), Knew::WhereHeWas);
        assert_eq!(Knew::of(false, false), Knew::Nothing);
        assert_eq!(Knew::ALL.map(|k| k as usize), [0, 1, 2], "the counts' order");
    }
}
