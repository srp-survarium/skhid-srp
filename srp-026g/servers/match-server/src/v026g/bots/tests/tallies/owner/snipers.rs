//! The owner counts of the bench's snipers by the rule the live humans were measured by (A182, private
//! `a182-sniper/HUMAN-SNIPERS.md`, `live_sniper.py`): a body holding a bolt or marksman rifle in a still spell (10 s or
//! more within 2 m, one life, the rifle in hand for 80 % of it), every crossing of its sight line by an enemy (an enemy
//! some part of whom shows from its standing, or crouched, eye through the bullet mesh within 150 m, a new one after 3 s
//! unseen), and what the holder did about it: shot at him (FIRE with the view within 10 degrees of him), when, and
//! killed him within 15 s of the last exposed tick. Also hits per shot by the target's distance, and kills per minute with
//! the rifle in hand. Counted by kind of rifle (bolt, marksman) and by who holds it: the stand-in camper (`SRP_BENCH_CAMPER`)
//! or the bots.
//!
//! The live numbers it is read against (190 recordings, 145 with a mesh): kill share per crossing bolt 7.7 %, marksman
//! 11.9 %; by distance 15 / 10 / 5 / 2 % (0-40 / 40-80 / 80-120 / 120-150 m); a shot at 23 % of the crossings (36 % when looking
//! that way), the first one a median 2.75 s after the enemy shows; hits per shot bolt 31 %, marksman 20 %; kills per minute with
//! the rifle in hand bolt 0.57, marksman 0.82.
//!
//! What differs: the bench's positions are exact (the live ones are reckoned, error 0.5 m), its exposure is judged every tick
//! (the live one every 200 ms, refined to 50 ms), and its matches are short (10 s spells are rarer).

use super::*;
#[cfg(test)]
use crate::v026g::bots::guns::gun;
use crate::v026g::bots::tests::tallies::shot_offset::ShotClass;

const REACH_M: f32 = 150.0;
const STILL_M: f32 = 2.0;
const SPELL_MS: u32 = 10_000;
const NEW_ENTRY_GAP_MS: u32 = 3_000;
const FATAL_MS: u32 = 15_000;
const AIMED_DEG: f32 = 10.0;
const SNIPER_SHARE: f32 = 0.8;
const EYE_STAND_M: f32 = 1.6;
const EYE_CROUCH_M: f32 = 1.0;
/// The upper edges of the distance bands, m (0-40, 40-80, 80-120, 120-150 for crossings; the last also takes more for shots).
const BANDS_M: [f32; 4] = [40.0, 80.0, 120.0, 150.0];
/// The view classes of a crossing: the holder's view within this of the enemy at the entry, degrees (looking, roughly, away).
const VIEW_DEG: [f32; 2] = [15.0, 30.0];
/// The first shot's histogram: bins of this many ms, and the last bin takes the rest.
const SHOT_BIN_MS: u32 = 250;
const SHOT_BINS: usize = 41;
/// Crossings that stay at least this long (s) and the shot within it.
const STAY_S: [u32; 3] = [1, 2, 3];
/// The parts of a standing enemy tested from the eye: (height of the point, sideways) as `sense/los.py` and the live reading.
const PARTS: [(f32, f32); 7] = [(1.26, 0.0), (1.60, 0.0), (1.02, 0.0), (0.66, 0.0), (1.26, -0.18), (1.26, 0.18), (0.30, 0.0)];

/// The rifle kinds counted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Bolt,
    Marksman,
}

impl Kind {
    const ALL: [Self; 2] = [Self::Bolt, Self::Marksman];

    fn of(class: &str) -> Option<Self> {
        match ShotClass::of(class) {
            ShotClass::Bolt => Some(Self::Bolt),
            ShotClass::Marksman => Some(Self::Marksman),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bolt => "bolt",
            Self::Marksman => "marksman",
        }
    }
}

/// Who holds the rifle: the stand-in camper or a bot.
fn who_of(run: &Run, place: Place) -> usize {
    usize::from(run.camper == Some(place))
}

const WHO: [&str; 2] = ["bots", "camper"];

fn group(kind: Kind, who: usize) -> usize {
    (kind as usize) * 2 + who
}

fn band_of(d: f32) -> usize {
    BANDS_M.iter().position(|&edge| d <= edge).unwrap_or(BANDS_M.len() - 1)
}

fn view_of(off: f32) -> usize {
    VIEW_DEG.iter().position(|&edge| off <= edge).unwrap_or(VIEW_DEG.len())
}

/// A still spell of a body: where it began, its life, the ticks it held each kind of rifle (bolt, marksman, other), and
/// whether it reached 10 s as a sniper's (judged once).
struct Spell {
    anchor: Vec3,
    spawned: u32,
    start_ms: u32,
    held: [u32; 3],
    judged: bool,
    committed: Option<Kind>,
}

/// A crossing of a holder's sight line.
struct Crossing {
    spell: usize,
    holder: Place,
    enemy: Place,
    t_entry: u32,
    t_last: u32,
    dist: f32,
    view: usize,
    first_aimed_ms: Option<u32>,
    presses: u32,
}

/// The counts of one group (kind of rifle, holder).
#[derive(Default, Clone)]
struct Counts {
    spells: u32,
    spell_ms: u64,
    entries: u32,
    fatal: u32,
    engaged: u32,
    holder_died: u32,
    by_band: [[u32; 3]; 4],
    by_view: [[u32; 3]; 3],
    /// The crossings the holder shot at, and the histogram of the first shot's delay.
    shot_at: u32,
    first_shot: Vec<u32>,
    /// Crossings that stay `STAY_S[i]` s, and those with a shot within it.
    stay: [[u32; 2]; 3],
    /// Kills made during a committed spell.
    spell_kills: u32,
    /// Shots at a target by the target's distance band: shots, landed on an enemy.
    shots: [[u32; 2]; 4],
    /// Exposure in the crossings, ms.
    exposed_ms: u64,
}

impl Counts {
    fn new() -> Self {
        Self { first_shot: vec![0; SHOT_BINS], ..Self::default() }
    }
}

pub(super) struct SniperCounts {
    spells: Vec<Spell>,
    current: Vec<Option<usize>>,
    open: Vec<Crossing>,
    prev_fire: Vec<bool>,
    groups: Vec<Counts>,
    /// Alive ms by who and class (`ShotClass::ALL` order), and the kills by who and class of the killer's rifle.
    alive_ms: [[u64; 7]; 2],
    kills: [[u32; 7]; 2],
    kills_seen: usize,
    last_ms: u32,
}

impl SniperCounts {
    pub(super) fn new(n: usize) -> Self {
        Self {
            spells: Vec::new(),
            current: vec![None; n],
            open: Vec::new(),
            prev_fire: vec![false; n],
            groups: vec![Counts::new(); 4],
            alive_ms: [[0; 7]; 2],
            kills: [[0; 7]; 2],
            kills_seen: 0,
            last_ms: 0,
        }
    }
}

fn class_index(class: &str) -> usize {
    let c = ShotClass::of(class);
    ShotClass::ALL.iter().position(|&x| x == c).unwrap_or(0)
}

/// Whether some part of a standing enemy at `at` shows from `eye`.
fn shows(sight: &mapdata::RayTree, eye: Vec3, at: Vec3, rays: &mut u64) -> bool {
    let (dx, dz) = (at.x - eye.x, at.z - eye.z);
    let h = (dx * dx + dz * dz).sqrt().max(1e-3);
    PARTS.iter().any(|&(up, side)| {
        *rays += 1;
        let to = Vec3::new(at.x + dz / h * side, at.y + up, at.z - dx / h * side);
        sight.visible(eye, to)
    })
}

impl Topic for SniperCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// The shots this tick: by the shooter's rifle (bolt, marksman), who holds it and the target's distance.
    fn begin(&mut self, run: &Run, game: &mut Game) {
        for fact in &game.shot_facts {
            let Some(body) = game.bodies.get(fact.shooter) else { continue };
            let (Some(kind), Some((_, d, _))) = (Kind::of(body.gun.class()), fact.target) else { continue };
            let cell = &mut self.groups[group(kind, who_of(run, fact.shooter))].shots[band_of(d)];
            cell[0] += 1;
            cell[1] += u32::from(fact.landed.on_enemy > 0);
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let now = game.tick.saturating_sub(run.first_tick);
        self.last_ms = now;
        self.kills_read(run, game);
        for place in 0..game.bodies.len().min(self.current.len()) {
            self.hold(run, game, place, now);
        }
        for place in 0..game.bodies.len().min(self.current.len()) {
            self.sample(game, sight, place, now, rays);
        }
        self.follow(run, game, now);
        self.resolve(run, now, false);
    }

    fn parts(&mut self, run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        self.resolve(run, self.last_ms, true);
        vec![("snipers", self.part())]
    }
}

impl SniperCounts {
    /// The kills since the last read: by the killer's rifle class and who holds it, and those made in a committed spell.
    fn kills_read(&mut self, run: &Run, game: &Game) {
        for &(_, shooter, _) in &run.kills[self.kills_seen.min(run.kills.len())..] {
            let Some(body) = game.bodies.get(shooter) else { continue };
            self.kills[who_of(run, shooter)][class_index(body.gun.class())] += 1;
            if let Some(kind) = self.current.get(shooter).copied().flatten().and_then(|s| self.spells[s].committed) {
                self.groups[group(kind, who_of(run, shooter))].spell_kills += 1;
            }
        }
        self.kills_seen = run.kills.len();
    }

    /// One body's still spell this tick: it begins when the body moved 2 m from its anchor or began a life; at 10 s it is
    /// judged a sniper's when the rifle was in hand 80 % of it.
    fn hold(&mut self, run: &Run, game: &Game, place: Place, now: u32) {
        let body = &game.bodies[place];
        if !body.alive {
            self.current[place] = None;
            return;
        }
        let class = body.gun.class();
        self.alive_ms[who_of(run, place)][class_index(class)] += u64::from(TICK_MS);
        let kind_slot = match Kind::of(class) {
            Some(Kind::Bolt) => 0,
            Some(Kind::Marksman) => 1,
            None => 2,
        };
        let same = self.current[place].is_some_and(|s| {
            let spell = &self.spells[s];
            spell.spawned == body.spawned && body.feet.distance_xz(spell.anchor) <= STILL_M && (body.feet.y - spell.anchor.y).abs() <= STILL_M
        });
        if !same {
            self.spells.push(Spell { anchor: body.feet, spawned: body.spawned, start_ms: now, held: [0; 3], judged: false, committed: None });
            self.current[place] = Some(self.spells.len() - 1);
        }
        let s = self.current[place].expect("a spell");
        let spell = &mut self.spells[s];
        spell.held[kind_slot] += 1;
        let age = now - spell.start_ms;
        if !spell.judged && age >= SPELL_MS {
            spell.judged = true;
            let total: u32 = spell.held.iter().sum();
            let sniper = spell.held[0] + spell.held[1];
            if total > 0 && sniper as f32 >= SNIPER_SHARE * total as f32 {
                let kind = if spell.held[0] >= spell.held[1] { Kind::Bolt } else { Kind::Marksman };
                spell.committed = Some(kind);
                let g = &mut self.groups[group(kind, who_of(run, place))];
                g.spells += 1;
                g.spell_ms += u64::from(age);
            }
        } else if let Some(kind) = spell.committed {
            self.groups[group(kind, who_of(run, place))].spell_ms += u64::from(TICK_MS);
        }
    }

    /// The enemies in this holder's sight line now: a crossing goes on, or a new one begins.
    fn sample(&mut self, game: &Game, sight: &mapdata::RayTree, place: Place, now: u32, rays: &mut u64) {
        let (Some(s), body) = (self.current[place], &game.bodies[place]) else { return };
        if !body.alive || Kind::of(body.gun.class()).is_none() {
            return;
        }
        let eye = body.feet + Vec3::new(0.0, if body.keys & key::CROUCH != 0 { EYE_CROUCH_M } else { EYE_STAND_M }, 0.0);
        for (enemy, other) in game.bodies.iter().enumerate() {
            if !other.alive || other.team == body.team {
                continue;
            }
            let d = (other.feet - body.feet).length();
            if d > REACH_M || (other.feet.y - body.feet.y).abs() > 40.0 || !shows(sight, eye, other.feet, rays) {
                continue;
            }
            match self.open.iter_mut().find(|c| c.holder == place && c.enemy == enemy && now <= c.t_last + NEW_ENTRY_GAP_MS && c.spell == s) {
                Some(c) => c.t_last = now,
                None => self.open.push(Crossing {
                    spell: s,
                    holder: place,
                    enemy,
                    t_entry: now,
                    t_last: now,
                    dist: d,
                    view: view_of(off_deg(body.yaw, body.feet, other.feet)),
                    first_aimed_ms: None,
                    presses: 0,
                }),
            }
        }
    }

    /// The holders' shots at the enemies of their open crossings: the first aimed one, and the presses.
    fn follow(&mut self, _run: &Run, game: &Game, now: u32) {
        for c in &mut self.open {
            let body = &game.bodies[c.holder];
            if !body.alive || now > c.t_last + 1_000 || body.keys & key::FIRE == 0 {
                continue;
            }
            let Some(enemy) = game.bodies.get(c.enemy) else { continue };
            if off_deg(body.yaw, body.feet, enemy.feet) > AIMED_DEG {
                continue;
            }
            c.first_aimed_ms.get_or_insert(now.saturating_sub(c.t_entry));
            if !self.prev_fire[c.holder] {
                c.presses += 1;
            }
        }
        for (place, body) in game.bodies.iter().enumerate().take(self.prev_fire.len()) {
            self.prev_fire[place] = body.alive && body.keys & key::FIRE != 0;
        }
    }

    /// The crossings whose window has closed (all, at the end): counted in their group when their spell was judged a sniper's.
    fn resolve(&mut self, run: &Run, now: u32, all: bool) {
        let mut i = 0;
        while i < self.open.len() {
            let c = &self.open[i];
            if !all && now <= c.t_last + FATAL_MS {
                i += 1;
                continue;
            }
            let c = self.open.swap_remove(i);
            let Some(kind) = self.spells[c.spell].committed else { continue };
            let first = run.first_tick;
            let within = |a: Place, b: Place| {
                run.kills
                    .iter()
                    .any(|&(t, shooter, victim)| shooter == a && victim == b && t.saturating_sub(first) + 200 >= c.t_entry && t.saturating_sub(first) <= c.t_last + FATAL_MS)
            };
            let fatal = within(c.holder, c.enemy);
            let died = within(c.enemy, c.holder);
            let g = &mut self.groups[group(kind, who_of(run, c.holder))];
            let stays_ms = c.t_last - c.t_entry + TICK_MS;
            g.entries += 1;
            g.fatal += u32::from(fatal);
            g.engaged += u32::from(c.presses > 0);
            g.holder_died += u32::from(died);
            g.exposed_ms += u64::from(stays_ms);
            let b = band_of(c.dist);
            g.by_band[b][0] += 1;
            g.by_band[b][1] += u32::from(fatal);
            g.by_band[b][2] += u32::from(c.presses > 0);
            g.by_view[c.view][0] += 1;
            g.by_view[c.view][1] += u32::from(fatal);
            g.by_view[c.view][2] += u32::from(c.presses > 0);
            if let Some(ms) = c.first_aimed_ms {
                g.shot_at += 1;
                g.first_shot[((ms / SHOT_BIN_MS) as usize).min(SHOT_BINS - 1)] += 1;
            }
            for (k, &s) in STAY_S.iter().enumerate() {
                if stays_ms >= s * 1_000 {
                    g.stay[k][0] += 1;
                    g.stay[k][1] += u32::from(c.first_aimed_ms.is_some_and(|ms| ms <= s * 1_000));
                }
            }
        }
    }

    fn part(&self) -> Part {
        let mut json = serde_json::Map::new();
        let mut lines = Vec::new();
        for kind in Kind::ALL {
            for (who, who_name) in WHO.iter().enumerate() {
                let g = &self.groups[group(kind, who)];
                let name = format!("{}_{}", kind.label(), who_name);
                let class = class_index(kind.label());
                let alive_min = self.alive_ms[who][class] as f64 / 60_000.0;
                json.insert(
                    name.clone(),
                    serde_json::json!({
                        "spells": g.spells,
                        "spell_s": g.spell_ms / 1_000,
                        "entries": g.entries,
                        "fatal": g.fatal,
                        "engaged": g.engaged,
                        "holder_died": g.holder_died,
                        "exposed_s": g.exposed_ms / 1_000,
                        "by_band": g.by_band,
                        "by_view": g.by_view,
                        "shot_at": g.shot_at,
                        "first_shot_hist": g.first_shot,
                        "stay": g.stay,
                        "spell_kills": g.spell_kills,
                        "shots": g.shots,
                        "alive_s": (alive_min * 60.0).round() as u64,
                        "kills": self.kills[who][class],
                    }),
                );
                let pct = |a: u32, b: u32| Scale::pct(f64::from(a), f64::from(b));
                let hit: (u32, u32) = g.shots.iter().fold((0, 0), |s, c| (s.0 + c[0], s.1 + c[1]));
                lines.push(format!(
                    "{name}: {} spells, {} crossings, kill share {:.1} % (bands {}), shot at {:.0} %, hits per shot {:.1} % of {}, kills per minute in hand {:.2}",
                    g.spells,
                    g.entries,
                    pct(g.fatal, g.entries),
                    g.by_band.iter().map(|b| format!("{:.0}", pct(b[1], b[0]))).collect::<Vec<_>>().join("/"),
                    pct(g.shot_at, g.entries),
                    pct(hit.1, hit.0),
                    hit.0,
                    if alive_min > 0.0 { f64::from(self.kills[who][class]) / alive_min } else { 0.0 },
                ));
            }
        }
        Part::new(serde_json::json!({ "snipers": json }), format!("owner-test snipers: {}", lines.join("; ")))
    }
}

/// A bolt rifle held 12 s on one spot sees an enemy walk in 40 m off; it shoots at him 2 s after he shows and kills him, the
/// crossing is counted as shot at and fatal in the bolt group of the bots; the same body held 5 s only counts nothing.
#[test]
pub(crate) fn a_bolt_holder_that_shoots_and_kills_a_crosser_is_counted_by_the_human_rule() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    game.bodies[0].gun = gun("sv_98").expect("a bolt rifle");
    assert_eq!(game.bodies[0].gun.class(), "bolt");
    let ticks = 20 * 40;
    play(&mut game, &mut tally, ticks, |k, game| {
        // The holder stands at (10, 10) looking along +z (yaw 0); the enemy appears at tick 300 (15 s) 30 m ahead.
        put(game, 0, 10.0, 10.0, 0);
        game.bodies[0].yaw = 0.0;
        game.bodies[0].gun = gun("sv_98").expect("a bolt rifle");
        put(game, 1, 10.0, 40.0, 0);
        game.bodies[1].alive = k >= 300;
        game.bodies[0].keys = if (340..345).contains(&k) { key::FIRE } else { 0 };
        if k == 360 {
            game.kill_log.push((game.tick, 0, 1));
            game.bodies[1].alive = false;
        }
        vec![bot_now(); 2]
    });
    let (_, json) = tally.finish(&game);
    let g = &json["snipers"]["bolt_bots"];
    assert_eq!(g["spells"], 1, "{json}");
    assert_eq!(g["entries"], 1, "{json}");
    assert_eq!(g["fatal"], 1, "{json}");
    assert_eq!(g["engaged"], 1, "{json}");
    assert_eq!(g["shot_at"], 1, "{json}");
    assert_eq!(g["by_band"][0][0], 1, "{json}");
    assert_eq!(json["snipers"]["bolt_camper"]["entries"], 0);
    assert_eq!(json["snipers"]["marksman_bots"]["entries"], 0);
}

/// A holder that moves every 4 s never makes a spell, so its crossings are not counted.
#[test]
pub(crate) fn a_body_that_keeps_moving_holds_no_spell() {
    let (mut game, mut tally) = made_up(&[0, 1]);
    play(&mut game, &mut tally, 20 * 30, |k, game| {
        put(game, 0, 10.0 + 4.0 * (k / 80) as f32, 10.0, 0);
        game.bodies[0].gun = gun("sv_98").expect("a bolt rifle");
        put(game, 1, 10.0, 40.0, 0);
        vec![bot_now(); 2]
    });
    let (_, json) = tally.finish(&game);
    assert_eq!(json["snipers"]["bolt_bots"]["spells"], 0, "{json}");
    assert_eq!(json["snipers"]["bolt_bots"]["entries"], 0, "{json}");
}
