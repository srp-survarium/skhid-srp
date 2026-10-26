//! The stand-in game: the body and the match around it (`Game`), as the module doc of
//! `tests/mod.rs` describes. A change here moves what the bots meet, so every result of the
//! bench with it: a model change is one visible edit of this file, and one of the switches
//! below when it should not move everyone's numbers.
//!
//! The game-model switches (the environment, read by [`bench_settings`]; `settings.rs` is the
//! list with what each does) and what each is when unset:
//!
//! | switch | when unset |
//! |---|---|
//! | `SRP_BENCH_USE_LINE` | off: a use needs no clear line |
//! | `SRP_BENCH_TAKE_M` | [`BATTERY_M`], 0.2 m |
//! | `SRP_BENCH_UNTAKEABLE` | 0 |
//! | `SRP_BENCH_CAMP` | no camping carrier |
//! | `SRP_BENCH_NUDGE` | no nudged battery |
//! | `SRP_BENCH_SPREAD` | off: no shot spread |
//! | `SRP_BENCH_BATTERIES` | one battery at every spawner |
//! | `SRP_BENCH_START` | 0,0: every battery lying |
//! | `SRP_BENCH_ZONE_DAMAGE` | off: zones do not hurt the bodies |
//! | `SRP_BENCH_PUT_MS` | [`PUT_MS`], 600 ms |
//! | `SRP_BENCH_JUMP_AHEAD_M` ([`standin_jump`](super::standin_jump)) | `STANDING_JUMP.ahead_m` |
//! | `SRP_BENCH_STAND_IN` | the honest jump (the client's clips); `old`: the lift up any floor |
//! | `SRP_BENCH_HEAL_MODEL` ([`standin_heal`](super::standin_heal)) | on: the drugs and their stock; `off`: none |
//!
//! The model's constants stay here: `RESPAWN_BATTERIES_MS`, `RESPAWN_TDM_MS`, `DRAW_MS`,
//! `PUT_MS`, `USE_REACH_M`, `STAND_IN_LEG_*`, `STAND_IN_STAGGER*`.

use super::*;

/// The client's use: a ray from the eye this long (`usable_objects_detection_distance`).
pub(crate) const USE_REACH_M: f32 = 2.0;
/// The patched client's take of a battery under the feet: its ray and the
/// distance across (patch 11, `client_patch.md`).
const PATCHED_REACH_M: f32 = 4.0;
const PATCHED_ACROSS_M: f32 = 1.0;
const PATCHED_BELOW_M: f32 = 0.2;

/// A lying battery's middle over its position, and how near the ray must
/// pass it (its box is about 0.5 by 0.25 m).
pub(crate) const BATTERY_MIDDLE: Vec3 = Vec3::new(0.0, 0.12, 0.0);

pub(crate) const BATTERY_M: f32 = 0.2;

/// A container's use box: its middle over the use point
/// (`Container::use_point`), and how near the ray must pass it.
pub(crate) const USE_BOX_UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);

pub(crate) const USE_BOX_M: f32 = 0.6;

/// How long a put-in takes, ms: the battery's put state lasts
/// `states[2].put_time`, 0.6 s (`gameplay/victory_items/default.options`,
/// `docs/notes/battery_pickup.md`), and the battery goes in at its end
/// (LIKELY: in the owner's Cologne match of 2026-10-24 every put-in a bot
/// judged failed went in 0.3-3.3 s later). `SRP_BENCH_PUT_MS=<ms>` sets
/// another (0: in at the press, as before).
pub(crate) const PUT_MS: u32 = 600;

/// The respawn after a death in the battery modes, ms: `respawn_time` 10 s
/// in every mode-2 and mode-3 config since revision 28 (content edit
/// `battery-respawn-1`, `docs/notes/battery_respawn.md`), less the radio's
/// 20 % (`respawn_booster`, the stand-in's bodies carry one), PROVEN from
/// the client's code (the respawn rule 0x316700, the death 0x316AA0). The
/// player's respawn, not a battery's: a battery never goes home.
pub(crate) const RESPAWN_BATTERIES_MS: u32 = 8_000;

/// The same in Team Deathmatch: 8 s, less the radio's 20 %.
pub(crate) const RESPAWN_TDM_MS: u32 = 6_400;

/// The respawn delay of a death, ms, as the game gives it: the mode's
/// `respawn_time` times `max(0, 1 + the radio's respawn_time_modifier)`
/// (the respawn rule 0x316AA0 through the getter 0x2D5AE0, PROVEN), up to
/// the 50 ms grid. The `- 1e-3` keeps an exact multiple of the grid on it
/// (0.8 is not exact in f32: 8000.0001 would round up to 8050).
pub(crate) fn respawn_ms_of(respawn_time_s: u32, radio_modifier: f32) -> u32 {
    let ms = respawn_time_s as f32 * 1000.0 * (1.0 + radio_modifier).max(0.0);
    (ms / 50.0 - 1e-3).ceil() as u32 * 50
}

/// The respawn delay in the map's mode, ms, read from the game data the
/// process runs on (the mode's `respawn_time`, the radio's modifier): the
/// live game's with `SRP_GAME_DATA`, so the constants above cannot go
/// stale when a content edit changes the time.
pub(crate) fn respawn_ms_from_data(level: u8, weather: u8, mode: u8) -> Option<u32> {
    let row = survarium::v026g::maps::find(level, weather, mode)?;
    let radio = survarium::v026g::dictionary::Dictionary::shared()
        .item_named("respawn_booster")?
        .dict_id;
    let modifier = survarium::v026g::loadouts::default_modifier(radio, "respawn_time_modifier");
    Some(respawn_ms_of(row.respawn_time, modifier))
}

/// How long after a carrier lets go of the battery his weapon fires again,
/// ms: the weapon's show time follows the drop, about 0.8 s for rifles,
/// SMGs and shotguns, 0.5 s for the TT-33 (LIKELY: private
/// sprint-jump-re FINDINGS section 9; the stand-in's bodies hold their
/// primary).
pub(crate) const DRAW_MS: u32 = 800;

/// A carrier's use at his container in the stand-in: when, from where, how
/// far its ray passed the use box's middle, whether it began a put, and
/// whether one was going on already.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PutPress {
    pub(crate) place: usize,
    pub(crate) tick: u32,
    pub(crate) feet: Vec3,
    pub(crate) miss: f32,
    pub(crate) began: bool,
    pub(crate) putting: bool,
}

/// Whose body a kill is: an enemy's, a teammate's (the game has friendly
/// fire), or the killer's own (a grenade at one's feet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kill {
    Enemy,
    Mate,
    Own,
}

/// A body of the stand-in game.
#[derive(Clone, Debug)]
pub(crate) struct Body {
    /// The weapon in hand (the bot's, as its last input was made), its
    /// kick and its trigger.
    pub(crate) gun: &'static Gun,
    pub(crate) recoil: Recoil,
    pub(crate) cadence: Cadence,
    pub(crate) team: u8,
    pub(crate) feet: Vec3,
    pub(crate) node: Option<NodeId>,
    pub(crate) yaw: f32,
    /// The base pitch (`m_look_pitch`, as the worlds give it); the view is
    /// it and the kick.
    pub(crate) pitch: f32,
    pub(crate) blend: Blend,
    pub(crate) keys: u32,
    pub(crate) health: f32,
    /// Its other pools ([`standin_damage::Pools`]): `health` is the body's.
    pub(crate) pools: standin_damage::Pools,
    /// Its weapon's rounds and ammo ([`standin_weapon`]).
    pub(crate) stock: standin_weapon::Stock,
    pub(crate) alive: bool,
    pub(crate) dead_at: u32,
    pub(crate) spawned: u32,
    pub(crate) carrying: Option<usize>,
    /// No shot before this tick: the weapon coming up after a drop
    /// ([`DRAW_MS`]).
    pub(crate) drawn_at: u32,
    pub(crate) moved: f32,
    /// Until when a jump pressed keeps the body in the air, ms
    /// ([`STAND_IN_AIR_MS`]; the old stand-in).
    pub(crate) air_until: u32,
    /// The jump under way (the honest stand-in, [`Flight`]), and the
    /// ground distance moved at the last tick, m.
    pub(crate) flight: Option<Flight>,
    pub(crate) step_m: f32,
    /// The leg pools (0.5 full), until when an empty one is broken, and
    /// the last hit's tick (0: none this life), [`STAND_IN_LEG_SHARE`].
    pub(crate) legs: [f32; 2],
    pub(crate) leg_broken_until: u32,
    pub(crate) last_hit: u32,
    /// The bot's target as its last input was made (the reflex's).
    pub(crate) target: Option<Place>,
    /// Its shots' spread ([`bench_spread`]).
    pub(crate) spread: bench_spread::Spread,
    /// Its stretch of zone damage under way ([`ZoneSpell`]).
    pub(crate) zone_spell: Option<ZoneSpell>,
    /// Its bot's motives at its last tick (`Bot::bench_why`).
    pub(crate) why_now: u16,
    /// Its drugs, and the pools the fields above lack ([`standin_heal`]).
    pub(crate) heal: standin_heal::Heal,
    pub(crate) rest: standin_heal::Rest,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Lying {
    /// Where, and the HUD mask as the client keeps it
    /// (`TeamKnowledge::hud_update`): 0 when it appears, the carrier's
    /// team's bit alone when he drops it or dies with it.
    Free(Vec3, u32),
    Carried(Place),
    Home(u8),
}

pub(crate) type Recorded = (u32, Vec<Fix>, Vec<f32>, WorldFacts);

/// An input for a tick: the tick, the keys, the turn, the bot's target
/// and weapon in hand.
pub(crate) type Queued = (u32, u32, (f32, f32), Option<Place>, &'static Gun);

/// A stolen battery: the team it was stolen from, and when (ms) and how
/// (alive) it fell since, if it did.
pub(crate) type StolenMark = (u8, Option<(u32, bool)>);

/// Every this long one lying battery is moved (`Game::nudge`), ms.
pub(crate) const NUDGE_EVERY_MS: u32 = 20_000;

/// A carrier killed this soon after his take counts as killed taking it
/// (`Game::taken_killed`), ms.
pub(crate) const TAKEN_KILLED_MS: u32 = 3_000;

/// A stand-in body's legs: one hit in this many lands on a leg [GUESS: the
/// hip and foot zones], taking [`STAND_IN_LEG_HIT`] of its pool (0.5 full;
/// one or two rifle hits empty one, LIKELY, sprint-jump RE `FINDINGS.md`
/// §1.8). An empty pool breaks the leg: no jump at all for
/// [`STAND_IN_LEG_BROKEN_MS`] (PROVEN), then it is whole (the game mends
/// it slowly).
pub(crate) const STAND_IN_LEG_SHARE: u32 = 4;

pub(crate) const STAND_IN_LEG_HIT: f32 = 0.25;

pub(crate) const STAND_IN_LEG_BROKEN_MS: u32 = 8_000;

/// After a hit the stand-in walks at this share of its speed for
/// [`STAND_IN_STAGGER_MS`] (the game's stagger: up to half, a rifle hit
/// 15-35 %, PROVEN; the share a GUESS between).
pub(crate) const STAND_IN_STAGGER: f32 = 0.75;

pub(crate) const STAND_IN_STAGGER_MS: u32 = 500;

/// The leg a hit of `shooter` on `target` at `tick` lands on, if any
/// ([`STAND_IN_LEG_SHARE`]): a hash, so the game's draws are not moved.
pub(crate) fn stand_in_leg_hit(tick: u32, shooter: Place, target: Place) -> Option<usize> {
    let from = (shooter as u32).wrapping_mul(0x85EB_CA6B);
    let to = (target as u32).wrapping_mul(0x9E37_79B9);
    let h = (tick ^ from ^ to).wrapping_mul(0xC2B2_AE35);
    let h = h ^ (h >> 16);
    let leg = ((h >> 8) & 1) as usize;
    h.is_multiple_of(STAND_IN_LEG_SHARE).then_some(leg)
}

/// The stand-in game.
pub(crate) struct Game {
    pub(crate) world: Arc<World>,
    pub(crate) kin: Kinematics,
    pub(crate) bodies: Vec<Body>,
    pub(crate) batteries: Vec<Lying>,
    pub(crate) containers: Vec<(u8, Vec3)>,
    pub(crate) stats: Vec<(u32, u32, u32)>,
    pub(crate) points: [u32; 2],
    /// Team Deathmatch's tickets.
    pub(crate) tickets: Option<[u32; 2]>,
    /// Timed batteries: the next one's time, and every how long one
    /// appears by a station (round the stations in turn).
    pub(crate) timed: Option<(u32, u32)>,
    pub(crate) inputs: Vec<VecDeque<Queued>>,
    /// The worlds of the last two seconds.
    pub(crate) history: VecDeque<Recorded>,
    /// The bodies' hurts in those worlds ([`Game::hurts_at`]).
    pub(crate) hurts: VecDeque<(u32, Vec<Hurt>)>,
    pub(crate) tick: u32,
    pub(crate) rng: Rng,
    /// How a hit hurts ([`standin_damage`]).
    pub(crate) damage: standin_damage::Model,
    pub(crate) shots: u32,
    pub(crate) hits: u32,
    /// By place: shots, and hits on an enemy.
    pub(crate) shots_hits: Vec<(u32, u32)>,
    /// The places that put a battery in at the last step.
    pub(crate) put_in: Vec<Place>,
    /// The team hits of the last step: shooter, victim, the shot's eye
    /// and direction.
    pub(crate) team_hit_shots: Vec<(Place, Place, Vec3, Vec3, f32, Vec3)>,
    /// Hits on a teammate (the game has friendly fire), and the teammates
    /// killed so.
    pub(crate) team_hits: u32,
    pub(crate) team_kills: u32,
    /// By team hit: the victim's pain pool before it, and whether it killed (the bandage habit's counts).
    pub(crate) team_hit_pains: Vec<(f32, bool)>,
    /// Jumps pressed, and of them those a broken leg refused.
    pub(crate) jumps_pressed: u32,
    pub(crate) jumps_refused: u32,
    /// Shots at the bot's target whose ray met the world short of him, and
    /// those with nothing of him showing.
    pub(crate) walled: u32,
    pub(crate) hidden: u32,
    /// [PR C] The same by the shooter's team.
    pub(crate) walled_by: [u32; 2],
    pub(crate) hidden_by: [u32; 2],
    /// Rays the counts above cast (not the bots').
    pub(crate) metric_rays: u64,
    pub(crate) delivered: [u32; 2],
    pub(crate) stolen: u32,
    /// Batteries carriers let go of alive: by a fire press, by the drop key.
    pub(crate) drops: [u32; 2],
    /// The damage zones' toll ([`Game::zone_tick`]): deaths in them, health
    /// lost in them; and where batteries were taken from this tick (the
    /// run's loop counts those lying in zones on all match).
    pub(crate) zone_deaths: u32,
    pub(crate) zone_hp: f32,
    /// The anomaly fields of the redesign, made by the bench ([`standin_fields`]); off unless `SRP_BENCH_LIVE_FIELDS`.
    pub(crate) fields: standin_fields::FieldGen,
    /// Zone spells ended ([`ZoneSpell::ended`]): the health the zones took,
    /// the body's health at the end (enemy fire counts; a death, 0) and at
    /// the start, and 1 if in a fight ([`SPELL_FIGHT_MS`]).
    pub(crate) zone_spells: Vec<[f32; 5]>,
    pub(crate) took_from: Vec<Vec3>,
    /// Carriers' ground distance walked (m) and their living ticks: their
    /// mean speed.
    pub(crate) carry_m: f32,
    pub(crate) carry_ticks: u32,
    /// The places that took a battery out of the enemy's container, in
    /// order (the bench reads them by the thief's play).
    pub(crate) thieves: Vec<Place>,
    /// Carriers killed with a battery: (the shooter, the carrier), in
    /// order (the bench reads them by the shooter's play).
    pub(crate) carrier_kills: Vec<(Place, Place)>,
    /// Every kill: (tick, the shooter, the killed), for the bench's
    /// events (`SRP_BENCH_EVENTS`).
    pub(crate) kill_log: Vec<(u32, Place, Place)>,
    /// Every hit on an enemy: (tick, the shooter, the one hit), for the
    /// bench's hit tally ([`HitTally`]).
    pub(crate) hit_log: Vec<(u32, Place, Place)>,
    /// By battery: stolen from this team's container, and when (ms) and
    /// how (alive: dropped by a live carrier) it fell since, if it did.
    pub(crate) stolen_marks: Vec<Option<StolenMark>>,
    /// A stolen battery that fell, until it was home again: its owners'
    /// container (true) or the thief's side's, ms from the fall, alive.
    pub(crate) stolen_drops: Vec<(bool, u32, bool)>,
    /// What befell stolen batteries, for the bench's log.
    pub(crate) stolen_log: Vec<String>,
    /// Lying batteries taken; by place, when its battery was taken; the
    /// carriers killed within [`TAKEN_KILLED_MS`] of their take.
    pub(crate) taken: u32,
    pub(crate) took: Vec<Option<u32>>,
    pub(crate) taken_killed: u32,
    /// Whether a use's ray stops at the bullet mesh short of a lying
    /// battery (`SRP_BENCH_USE_LINE=1` in the real-map runs). LIKELY as
    /// the client: in the owner's match of 2026-10-16 a bot aimed right
    /// at a battery 9 times from where the mesh blocks the line, and took
    /// it at once from a side with a clear one.
    pub(crate) use_line: bool,
    /// How near a battery's middle a take's ray must pass, metres
    /// ([`BATTERY_M`]; `SRP_BENCH_TAKE_M` in the real-map runs makes takes
    /// fail as some do in the owner's matches, audit #7).
    pub(crate) take_m: f32,
    /// The respawn delay after a death, ms, when the run is on the live
    /// game data (`SRP_GAME_DATA`: [`respawn_ms_from_data`]); `None` keeps
    /// [`RESPAWN_BATTERIES_MS`] and [`RESPAWN_TDM_MS`].
    pub(crate) respawn_ms: Option<u32>,
    /// Batteries no use takes off the floor (`SRP_BENCH_UNTAKEABLE=<n>`:
    /// the first n; as the sunk ones of audit #7, which a use never took).
    pub(crate) untakeable: usize,
    /// How long a put-in takes ([`PUT_MS`]), and each place's put under
    /// way: the battery, the container, and when it began. A use pressed
    /// while one is under way does nothing (GUESS); the put ends unmade if
    /// the body died or no longer carries that battery.
    pub(crate) put_ms: u32,
    pub(crate) putting: Vec<Option<(usize, usize, u32)>>,
    /// Each use a carrier pressed within [`ARRIVED_M`] of his container.
    pub(crate) put_presses: Vec<PutPress>,
    /// Every put-in of the run: who, and the tick it went in (`put_in`
    /// holds only the last step's).
    pub(crate) put_in_log: Vec<(Place, u32)>,
    /// A camping carrier (audit #6, `SRP_BENCH_CAMP=<m>`): the first of
    /// team 0's carriers to come within this of his container stops there,
    /// moves no more and puts nothing in until he dies or no longer
    /// carries (as the owner's match 0x4da3b51d's bot 13, 8 m from his
    /// container for 273 s). The camper and when he began, once.
    pub(crate) camp_m: Option<f32>,
    pub(crate) camper: Option<(Place, u32)>,
    pub(crate) camp_done: bool,
    /// How the camping ended: (began, ended, he died).
    pub(crate) camp_ended: Option<(u32, u32, bool)>,
    /// Batteries moved unseen (audit #7, `SRP_BENCH_NUDGE=<m>`): every
    /// [`NUDGE_EVERY_MS`] one lying battery, in turn, moves this far on its
    /// floor, as one an enemy took and dropped dead a step on (the owner's
    /// match 0x4da3b51d, battery 3: 0.7 m); the next nudge's time, and how
    /// many moved.
    pub(crate) nudge: Option<(f32, u32)>,
    pub(crate) nudged: u32,
    /// Every shot, for the bench's fight tally ([`fight_tally`]).
    pub(crate) shot_facts: Vec<fight_tally::ShotFact>,
    /// Shots spread as the game's do ([`bench_spread`], `SRP_BENCH_SPREAD`).
    pub(crate) spread: bool,
    /// Whether bodies move as the client's do ([`honest_stand_in`]); the
    /// walker mesh a body in the air bumps into (built at the first jump;
    /// the sight's when the map has none, as the tests' fields); every
    /// jump's landing.
    pub(crate) honest: bool,
    pub(crate) walls: Option<Arc<mapdata::RayTree>>,
    pub(crate) landings: Vec<Landing>,
    /// Every press of a quick-use key ([`standin_heal::press_keys`]), and the
    /// bodies' pools and drugs in the worlds of the last two seconds
    /// ([`Game::seen_at`]).
    pub(crate) drug_log: Vec<standin_heal::DrugPress>,
    pub(crate) seen: VecDeque<(u32, Vec<standin_heal::Seen>)>,
}

/// The soak map: the 80 x 40 m field (a wall across its middle up to
/// z = 25), side 0 spawning at x = 5, side 1 at x = 75, their containers
/// near their spawns and three batteries between.
pub(crate) fn soak_world() -> Arc<World> {
    soak_world_with(&[])
}

/// A container's use box as the maps have them: a metre over its use
/// point (`Container::use_point`), about 2 m by 1.7 m by 1.3 m.
pub(crate) fn use_box(position: Vec3) -> mapdata::Shape {
    mapdata::Shape {
        kind: mapdata::ShapeKind::Box,
        center: position + USE_BOX_UP,
        rotation: mapdata::Rotation::IDENTITY,
        half: Vec3::new(1.0, 0.85, 0.65),
    }
}

/// [`soak_world`] with `solids` (more triangles) in its geometry.
pub(crate) fn soak_world_with(solids: &[mapdata::Triangle]) -> Arc<World> {
    super::tactics::tests::field_with_solids(solids, |map| {
        let container = |team: i32, id: i32, x: f32, z: f32| Container {
            position: Vec3::new(x, 0.0, z),
            team,
            id,
            use_shapes: vec![use_box(Vec3::new(x, 0.0, z))],
        };
        map.containers = vec![container(0, 0, 12.0, 30.0), container(1, 1, 68.0, 30.0)];
        map.stations = vec![Vec3::new(40.0, 0.0, 33.0), Vec3::new(30.0, 0.0, 5.0)];
        map.battery_spawners = [(40.0, 33.0), (25.0, 12.0), (55.0, 12.0)]
            .iter()
            .map(|&(x, z)| BatterySpawner {
                position: Vec3::new(x, 0.0, z),
                item: Vec3::new(x, 0.3, z),
                layer: "default".into(),
            })
            .collect();
    })
}

impl Game {
    pub(crate) fn new(
        world: Arc<World>,
        teams: &[u8],
        seed: u64,
        tickets: Option<u32>,
        timed: Option<u32>,
    ) -> Self {
        let containers = world
            .map
            .containers
            .iter()
            .map(|c| (c.team as u8, c.use_point()))
            .collect();
        // Timed batteries start with none lying about.
        let batteries = if timed.is_some() {
            Vec::new()
        } else {
            world
                .map
                .battery_spawners
                .iter()
                .map(|s| Lying::Free(s.item, 0))
                .collect()
        };
        let gun = Arms::fallback().primary.0;
        let body = |team| Body {
            gun,
            recoil: Recoil::new(kick_of(gun.item)),
            cadence: Cadence::default(),
            team,
            feet: Vec3::ZERO,
            node: None,
            yaw: 0.0,
            pitch: 0.0,
            blend: Blend::default(),
            keys: 0,
            health: 1.0,
            pools: standin_damage::Pools::default(),
            stock: standin_weapon::Stock::new(bench_tier(), gun),
            alive: false,
            dead_at: 0,
            spawned: 0,
            carrying: None,
            drawn_at: 0,
            moved: 0.0,
            air_until: 0,
            flight: None,
            step_m: 0.0,
            legs: [0.5; 2],
            leg_broken_until: 0,
            last_hit: 0,
            target: None,
            spread: bench_spread::Spread::default(),
            zone_spell: None,
            why_now: 0,
            heal: standin_heal::Heal::new(),
            rest: standin_heal::Rest::FULL,
        };
        let mut game = Self {
            kin: Kinematics::default(),
            bodies: teams.iter().map(|&team| body(team)).collect(),
            batteries,
            containers,
            stats: vec![(0, 0, 0); teams.len()],
            points: [0, 0],
            tickets: tickets.map(|t| [t, t]),
            timed: timed.map(|every| (every, every)),
            inputs: vec![VecDeque::new(); teams.len()],
            history: VecDeque::new(),
            hurts: VecDeque::new(),
            tick: 0,
            rng: Rng::new(seed),
            damage: standin_damage::Model::default(),
            shots: 0,
            hits: 0,
            shots_hits: vec![(0, 0); teams.len()],
            put_in: Vec::new(),
            team_hit_shots: Vec::new(),
            team_hits: 0,
            jumps_pressed: 0,
            jumps_refused: 0,
            team_kills: 0,
            team_hit_pains: Vec::new(),
            walled: 0,
            walled_by: [0; 2],
            hidden_by: [0; 2],
            hidden: 0,
            metric_rays: 0,
            delivered: [0, 0],
            stolen: 0,
            drops: [0, 0],
            zone_deaths: 0,
            zone_hp: 0.0,
            fields: standin_fields::FieldGen::from_env(&world, seed),
            zone_spells: Vec::new(),
            took_from: Vec::new(),
            carry_m: 0.0,
            carry_ticks: 0,
            thieves: Vec::new(),
            carrier_kills: Vec::new(),
            kill_log: Vec::new(),
            hit_log: Vec::new(),
            stolen_marks: Vec::new(),
            stolen_drops: Vec::new(),
            stolen_log: Vec::new(),
            taken: 0,
            took: vec![None; teams.len()],
            taken_killed: 0,
            use_line: false,
            take_m: BATTERY_M,
            respawn_ms: None,
            untakeable: 0,
            put_ms: bench_settings().number("SRP_BENCH_PUT_MS")
                .unwrap_or(PUT_MS),
            putting: vec![None; teams.len()],
            put_presses: Vec::new(),
            put_in_log: Vec::new(),
            camp_m: None,
            camper: None,
            camp_done: false,
            camp_ended: None,
            nudge: None,
            nudged: 0,
            shot_facts: Vec::new(),
            spread: false,
            honest: honest_stand_in(),
            walls: None,
            landings: Vec::new(),
            drug_log: Vec::new(),
            seen: VecDeque::new(),
            world,
        };
        for place in 0..teams.len() {
            game.spawn(place);
        }
        game
    }

    pub(crate) fn spawn(&mut self, place: Place) {
        let side = i32::from(self.bodies[place].team);
        let spawns: Vec<Vec3> = self
            .world
            .map
            .spawns
            .iter()
            .filter(|s| s.team == side)
            .map(|s| s.position)
            .collect();
        let point = spawns[self.rng.below(spawns.len())];
        let at = point + Vec3::new(self.rng.range(-2.0, 2.0), 0.0, self.rng.range(-2.0, 2.0));
        // Off by up to 2 m, but only onto a floor a walk from a spawn gets
        // to and back from: the offset can land on a floor below or beside
        // the spawn that no way leaves (Rudnya: 0.7 % of offsets, one spawn
        // 12 %), where the game never puts a body (it spawns on the point).
        let grid = &self.world.grid;
        let node = grid
            .locate(at, 3.0)
            .filter(|&n| grid.reached(n) && grid.returns(n))
            .or_else(|| grid.locate(point, 3.0));
        let feet = node.map_or(at, |n| self.world.grid.position(n));
        let tick = self.tick;
        let tier = self.damage.tier;
        let body = &mut self.bodies[place];
        body.feet = feet;
        body.node = node;
        // Facing the other side.
        body.yaw = if side == 0 {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        body.pitch = 0.0;
        body.blend = Blend::default();
        body.health = 1.0;
        body.pools = standin_damage::Pools::default();
        body.legs = [0.5; 2];
        body.leg_broken_until = 0;
        body.last_hit = 0;
        body.flight = None;
        body.step_m = 0.0;
        body.zone_spell = None;
        body.alive = true;
        body.spawned = tick;
        body.carrying = None;
        body.heal.spawn();
        body.rest = standin_heal::Rest::FULL;
        // Weapon 1 in hand afresh.
        body.gun = Arms::fallback().primary.0;
        body.recoil = Recoil::new(kick_of(body.gun.item));
        body.cadence = Cadence::default();
        body.spread.reset();
        body.stock.spawn(tier, body.gun);
    }

    /// Where the game's bullet starts, from the bots' own function
    /// ([`super::view::eye_height`]) at the body's pace and view.
    pub(crate) fn eye(body: &Body) -> Vec3 {
        let speed = body.step_m / TICK_S;
        let pitch = body.recoil.view(body.pitch);
        let up = super::view::eye_height(body.keys & key::CROUCH != 0, speed, pitch);
        body.feet + Vec3::new(0.0, up, 0.0)
    }

    pub(crate) fn chest(body: &Body) -> Vec3 {
        let up = if body.keys & key::CROUCH != 0 {
            0.8
        } else {
            1.3
        };
        body.feet + Vec3::new(0.0, up, 0.0)
    }

    /// One tick: each weapon's kick moves on, each body takes its input
    /// (the world of the tick shows it, before the move; a downward turn
    /// is taken off the kick first), then moves; shots, uses, drops,
    /// deaths, respawns.
    pub(crate) fn step(&mut self) {
        self.tick += TICK_MS;
        let tick = self.tick;
        self.put_in.clear();
        self.team_hit_shots.clear();
        let n = self.bodies.len();
        let mut pressed = vec![0u32; n];
        for body in &mut self.bodies {
            body.recoil.update(tick);
        }
        let tier = self.damage.tier;
        for (place, queue) in self.inputs.iter_mut().enumerate() {
            while let Some(&(at, keys, turn, target, gun)) = queue.front() {
                if at > tick {
                    break;
                }
                assert_eq!(at, tick, "an input for a tick already played");
                queue.pop_front();
                let body = &mut self.bodies[place];
                pressed[place] = keys & !body.keys;
                standin_heal::press_keys(body, place, pressed[place], tick, &mut self.drug_log);
                let mut jump = false;
                if pressed[place] & key::JUMP != 0 {
                    self.jumps_pressed += 1;
                    if body.legs.iter().all(|&pool| pool > 0.0) {
                        if self.honest {
                            jump = true;
                        } else {
                            body.air_until = tick + STAND_IN_AIR_MS;
                        }
                    } else {
                        self.jumps_refused += 1;
                    }
                }
                body.keys = keys;
                body.target = target;
                // Another weapon taken, or a reload: no kick.
                if gun.item != body.gun.item {
                    body.gun = gun;
                    body.recoil = Recoil::new(kick_of(gun.item));
                    body.recoil.update(tick);
                    body.cadence = Cadence::default();
                    body.spread.reset();
                    body.stock.switch(tier, gun);
                } else if pressed[place] & key::RELOAD != 0 {
                    body.recoil.reset();
                    body.stock.reload_key(tick);
                }
                body.yaw += turn.0;
                let taken = body.recoil.turn(turn.1);
                body.pitch = (body.pitch + turn.1 + taken).clamp(-1.0, 1.0);
                // The honest stand-in's jump: the keys and the facing at
                // the press pick its clip and way; none in the air.
                if jump && body.alive && body.node.is_some() && body.flight.is_none() {
                    let staggered =
                        body.last_hit != 0 && tick < body.last_hit + STAND_IN_STAGGER_MS;
                    let may_sprint = body.carrying.is_none() && !staggered;
                    let (clip, dir) = jump_of(keys, body.yaw, body.step_m / TICK_S, may_sprint);
                    let u = press_draw(tick, place);
                    let prepare = if std::ptr::eq(clip, &SPRINT_CLIP) {
                        (SPRINT_PREPARE_MS * u, SPRINT_PREPARE_M * u)
                    } else {
                        (0.0, 0.0)
                    };
                    body.flight = Some(Flight {
                        at: tick,
                        from: body.feet,
                        dir,
                        clip,
                        prepare,
                        base: None,
                        fall: None,
                    });
                }
            }
        }
        if self.walls.is_none() && self.bodies.iter().any(|b| b.flight.is_some()) {
            let walker = self.world.map.walker_triangles();
            self.walls = Some(if walker.is_empty() {
                self.world.sight.clone()
            } else {
                Arc::new(mapdata::RayTree::build(&walker))
            });
        }
        for (body, &pressed) in self.bodies.iter_mut().zip(&pressed) {
            if body.alive {
                body.spread.tick(body.gun, body.keys, pressed, tick, TICK_S);
            }
        }
        self.record();
        self.camp(tick, &mut pressed);
        // The shots of the tick leave from where the world of the tick
        // shows the bodies, before this tick's move (a bullet starts at
        // the world's position of its own tick, PROVEN live: 0.08 +-0.03
        // tick of travel, `fact-own-eye.md` 4), as `lead_own` reckons.
        if super::view::live_eye() {
            self.shoot(tick);
        }
        for place in 0..n {
            if self.camper.is_some_and(|(p, _)| p == place) {
                continue;
            }
            if !self.bodies[place].alive {
                let team = usize::from(self.bodies[place].team);
                let may = self.tickets.is_none_or(|tickets| tickets[team] > 0);
                let respawn = self.respawn_ms.unwrap_or(if self.tickets.is_some() {
                    RESPAWN_TDM_MS
                } else {
                    RESPAWN_BATTERIES_MS
                });
                if may && tick >= self.bodies[place].dead_at + respawn {
                    self.spawn(place);
                }
                continue;
            }
            let body = &mut self.bodies[place];
            if body.leg_broken_until != 0 && tick >= body.leg_broken_until {
                body.legs = body.legs.map(|pool| if pool > 0.0 { pool } else { 0.5 });
                body.leg_broken_until = 0;
            }
            standin_heal::advance(body, tick);
            let mut level = if body.carrying.is_some() { 0.95 } else { 1.0 };
            if body.last_hit != 0 && tick < body.last_hit + STAND_IN_STAGGER_MS {
                level *= STAND_IN_STAGGER;
            }
            // A carrier cannot sprint: the sprint check asks the item in
            // hand, and the battery's answer is no (0x397C20), PROVEN; seen
            // in 0x60aeff4f, the owner held sprint carrying at 3.35 m/s.
            let keys = if body.carrying.is_some() {
                body.keys & !key::SPRINT
            } else {
                body.keys
            };
            let local = body
                .blend
                .tick(self.kin.target_local(keys, level), self.kin.blend_ticks);
            let v = to_world(local, body.yaw);
            let to = Vec3::new(
                body.feet.x + v[0] * TICK_S,
                body.feet.y,
                body.feet.z + v[1] * TICK_S,
            );
            if let Some(node) = body.node {
                let grid = &self.world.grid;
                let before = body.feet;
                if !self.honest {
                    let (mut node, mut feet, _) = slide(grid, node, body.feet, to);
                    // In the air, walking into a floor the jump clears: up
                    // on it (once a jump).
                    if tick < body.air_until
                        && feet.distance_xz(to) > 1e-3
                        && let Some(up) = stand_in_jump(grid, node, feet, to, standing_jump())
                    {
                        node = up;
                        feet = grid.position(up);
                        body.air_until = 0;
                    }
                    body.node = Some(node);
                    body.feet = feet;
                } else if body.flight.is_some() {
                    // Keys held in the air do not steer it (no air control).
                    let walls = self.walls.as_deref().expect("built at the press");
                    if let Some(landing) = fly(grid, walls, body, place, tick) {
                        self.landings.push(landing);
                    }
                } else {
                    let (node, feet, _) = slide_capped(grid, node, body.feet, to, grid.body().step);
                    body.node = Some(node);
                    body.feet = feet;
                }
                body.step_m = body.feet.distance_xz(before);
                body.moved += body.step_m;
                if body.carrying.is_some() {
                    self.carry_m += body.step_m;
                    self.carry_ticks += 1;
                }
            }
        }
        if !super::view::live_eye() {
            self.shoot(tick);
        }
        standin_damage::regenerate(&self.damage, &mut self.bodies, tick);
        self.use_and_drop(&pressed);
        self.resupply(&pressed);
        if let Some((m, next)) = self.nudge
            && tick >= next
        {
            self.nudge = Some((m, next + NUDGE_EVERY_MS));
            self.nudge_one(m, tick);
        }
        // Timed batteries: one appears by the next station in turn.
        if let Some((next, every)) = self.timed
            && tick >= next
        {
            let stations = &self.world.map.stations;
            let at = stations[self.batteries.len() % stations.len()];
            self.batteries
                .push(Lying::Free(at + Vec3::new(1.5, 0.3, 0.0), 0));
            self.timed = Some((next + every, every));
        }
    }

    /// A lying battery, the next in turn, moved `m` on its floor (`nudge`).
    pub(crate) fn nudge_one(&mut self, m: f32, tick: u32) {
        let lying: Vec<usize> = (0..self.batteries.len())
            .filter(|&b| matches!(self.batteries[b], Lying::Free(..)))
            .collect();
        if lying.is_empty() {
            return;
        }
        let k = (tick / NUDGE_EVERY_MS) as usize;
        let battery = lying[k % lying.len()];
        let Lying::Free(at, mask) = self.batteries[battery] else {
            return;
        };
        let grid = &self.world.grid;
        let a = k as f32 * 2.4;
        let to = Vec3::new(at.x + m * a.cos(), at.y, at.z + m * a.sin());
        let (Some(from_floor), Some(to_floor)) = (grid.locate(at, 0.6), grid.locate(to, 0.6)) else {
            return;
        };
        let (from_floor, to_floor) = (grid.position(from_floor), grid.position(to_floor));
        if to_floor.distance_xz(to) > 0.3 || (to_floor.y - from_floor.y).abs() > 0.5 {
            return;
        }
        let up = at.y - from_floor.y;
        self.batteries[battery] = Lying::Free(Vec3::new(to.x, to_floor.y + up, to.z), mask);
        self.nudged += 1;
    }

    /// The camping carrier (`camp_m`): the first of team 0's carriers
    /// within `camp_m` of his container becomes him; he presses no use
    /// (and `step` does not move him) until he dies or no longer carries.
    pub(crate) fn camp(&mut self, tick: u32, pressed: &mut [u32]) {
        let Some(camp_m) = self.camp_m else {
            return;
        };
        if let Some((place, began)) = self.camper {
            let body = &self.bodies[place];
            if body.alive && body.carrying.is_some() {
                pressed[place] &= !key::USE;
                return;
            }
            self.camp_ended = Some((began, tick, !body.alive));
            self.camper = None;
            self.camp_done = true;
        }
        if self.camp_done {
            return;
        }
        let Some(&(_, home)) = self.containers.iter().find(|(t, _)| *t == 0) else {
            return;
        };
        if let Some(place) = (0..self.bodies.len()).find(|&p| {
            let body = &self.bodies[p];
            body.alive
                && body.team == 0
                && body.carrying.is_some()
                && body.feet.distance_xz(home) <= camp_m
        }) {
            self.camper = Some((place, tick));
            pressed[place] &= !key::USE;
        }
    }

    /// Shots, each along the view (the base pitch and the kick) and each
    /// kicking it: a hit on the nearest chest within 0.45 m of the ray, a
    /// teammate's too (the game has friendly fire).
    pub(crate) fn shoot(&mut self, tick: u32) {
        for shooter in 0..self.bodies.len() {
            let body = &mut self.bodies[shooter];
            body.stock.tick(tick, body.keys, body.alive);
            let held = body.alive
                && body.keys & key::FIRE != 0
                && body.carrying.is_none()
                && tick >= body.drawn_at
                && body.stock.may_fire(tick);
            let (auto, interval_ms) = (body.gun.auto, body.gun.shot_ms());
            let mut times = Vec::new();
            body.cadence
                .tick(tick, held, auto, interval_ms, |t| times.push(t));
            for at in times {
                if self.bodies[shooter].stock.fired(at) {
                    self.shot(shooter, at);
                }
            }
        }
    }

    /// One shot of `shooter` at `at` ms (within the tick).
    pub(crate) fn shot(&mut self, shooter: Place, at: f32) {
        self.shots += 1;
        self.shots_hits[shooter].0 += 1;
        let body = &self.bodies[shooter];
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let pitch = body.recoil.view(body.pitch);
        let (c, s) = (pitch.cos(), pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let kick = body.recoil.kick();
        let top = kick.top(self.rng.unit());
        let (gun, keys) = (body.gun, body.keys);
        self.bodies[shooter].recoil.shot(at, top, 0.0);
        let (aim_state, cone) = self.bodies[shooter].spread.shot(gun, keys, at);
        let view_dir = dir;
        let dir = if self.spread {
            bench_spread::deflect(dir, cone, &mut self.rng)
        } else {
            dir
        };
        let tick = self.tick;
        let body = &self.bodies[shooter];
        // A shot at the bot's target with nothing of him showing
        // (`aimpoint`'s parts), or whose ray meets the world short of
        // him.
        if let Some(target) = body.target.map(|t| &self.bodies[t])
            && target.alive
        {
            let before = super::aimpoint::rays_cast();
            let crouched = target.keys & key::CROUCH != 0;
            let team = usize::from(body.team).min(1);
            if !super::aimpoint::shows(eye, target.feet, crouched, &self.world.sight, || true) {
                self.hidden += 1;
                self.hidden_by[team] += 1;
            }
            let reach = (Self::chest(target) - eye).length() - 0.5;
            if self.world.sight.ray_cast(eye, dir, reach).is_some() {
                self.walled += 1;
                self.walled_by[team] += 1;
            }
            self.metric_rays += super::aimpoint::rays_cast() - before;
        }
        let mut best: Option<(Place, f32)> = None;
        for (target, other) in self.bodies.iter().enumerate() {
            if !other.alive || target == shooter {
                continue;
            }
            let to = Self::chest(other) - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > 100.0 {
                continue;
            }
            let miss = (to - dir * along).length();
            if miss < 0.45
                && best.is_none_or(|(_, d)| along < d)
                && self.world.sight.visible(eye, Self::chest(other))
            {
                best = Some((target, along));
            }
        }
        let shooter_body = &self.bodies[shooter];
        let (moving, aimed, crouched) = fight_tally::stance(shooter_body.keys);
        let aimed_at = shooter_body
            .target
            .filter(|&t| self.bodies[t].alive)
            .map(|t| {
                let (d, off) = fight_tally::aim_off(eye, dir, Self::chest(&self.bodies[t]));
                (t, d, off)
            });
        self.shot_facts.push(fight_tally::ShotFact {
            shooter,
            tick,
            hit: best
                .map(|b| b.0)
                .filter(|&t| self.bodies[t].team != shooter_body.team),
            target: aimed_at,
            moving,
            aimed,
            crouched,
            aim_state,
            cone,
        });
        let Some((target, along)) = best else {
            return;
        };
        let team_hit = self.bodies[target].team == body.team;
        if team_hit {
            self.team_hits += 1;
            self.team_hit_shots.push((shooter, target, eye, dir, cone, view_dir));
        } else {
            self.hits += 1;
            self.shots_hits[shooter].1 += 1;
            self.hit_log.push((tick, shooter, target));
        }
        let pain_before = self.bodies[target].pools.pain;
        let other = &mut self.bodies[target];
        standin_damage::hit(&self.damage, other, gun, along, &mut self.rng, tick);
        other.last_hit = tick;
        if let Some(leg) = stand_in_leg_hit(tick, shooter, target) {
            other.legs[leg] = (other.legs[leg] - STAND_IN_LEG_HIT).max(0.0);
            if other.legs[leg] <= 0.0 && other.leg_broken_until == 0 {
                other.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
            }
        }
        let lethal = other.health <= 1e-3;
        if team_hit {
            self.team_hit_pains.push((pain_before, lethal));
        }
        if lethal {
            let kind = if team_hit { Kill::Mate } else { Kill::Enemy };
            self.kill(target, shooter, kind, tick);
        }
    }

    /// `target` dies at `tick`, `killer`'s shot or blast the cause: it
    /// stays where it fell, its battery drops there, the kill is written
    /// down and counted as the kinds of [`Kill`] say, and its team loses a
    /// ticket.
    pub(crate) fn kill(&mut self, target: Place, killer: Place, kind: Kill, tick: u32) {
        let other = &mut self.bodies[target];
        other.alive = false;
        other.dead_at = tick;
        if let Some(battery) = other.carrying.take() {
            if self.took[target].is_some_and(|at| tick <= at + TAKEN_KILLED_MS) {
                self.taken_killed += 1;
            }
            self.batteries[battery] = Lying::Free(other.feet, 1 << other.team);
            if kind != Kill::Own {
                self.carrier_kills.push((killer, target));
            }
            if let Some(Some((_, fell @ None))) = self.stolen_marks.get_mut(battery) {
                *fell = Some((tick, false));
                self.stolen_log.push(format!(
                    "{tick} battery {battery} fell at its carrier's death"
                ));
            }
        }
        if kind != Kill::Own {
            self.kill_log.push((tick, killer, target));
        }
        self.stats[target].1 += 1;
        self.stats[target].2 = 0;
        if let Some(tickets) = self.tickets.as_mut() {
            let team = usize::from(other.team);
            tickets[team] = tickets[team].saturating_sub(1);
        }
        match kind {
            Kill::Mate => self.team_kills += 1,
            Kill::Enemy => {
                self.stats[killer].0 += 1;
                self.stats[killer].2 += 1;
            }
            Kill::Own => {}
        }
    }

    /// The damage zones' hits on the living, the bench's model of the game's
    /// (`mapdata::Damage`): a body on a floor inside a zone's own shape
    /// (`inside`, marked with no margin) loses the zone's hit a second
    /// ([`Hazard::hit_per_s`], full intensity, before armour) over the tick
    /// while the zone is on (`on`: the bots' zones, their timing); at no
    /// health it dies where it stands and its battery falls there.
    ///
    /// `baked`: the map's zones hurt (`SRP_BENCH_ZONE_DAMAGE=1`); the anomaly fields of the generator
    /// ([`Game::fields`]) hurt whenever it runs, whatever the bots know of them.
    pub(crate) fn zone_tick(&mut self, inside: &Hazards, on: &Hazards, baked: bool) {
        let tick = self.tick;
        for place in 0..self.bodies.len() {
            let body = &mut self.bodies[place];
            if !body.alive {
                if let Some(spell) = body.zone_spell.take() {
                    self.zone_spells.push(spell.ended(body));
                }
                continue;
            }
            let Some(node) = self.world.grid.locate(body.feet, 1.0) else {
                continue;
            };
            let per_baked: f32 = if baked {
                inside
                    .zones_at(node)
                    .filter(|zone| {
                        on.zones
                            .iter()
                            .find(|o| o.zone == zone.zone)
                            .is_some_and(|o| o.activity.active(tick))
                    })
                    .map(Hazard::hit_per_s)
                    .sum()
            } else {
                0.0
            };
            let per_live = self.fields.hit_per_s_at(node);
            let body = &mut self.bodies[place];
            let per_s = (per_baked + per_live) * body.heal.toxic_factor(tick);
            if per_s <= 0.0 {
                if let Some(spell) = body.zone_spell
                    && tick >= spell.last + SPELL_GAP_MS
                {
                    self.zone_spells.push(spell.ended(body));
                    body.zone_spell = None;
                }
                continue;
            }
            let hit = (per_s * TICK_S).min(body.health.max(0.0));
            if per_live > 0.0 {
                self.fields.stats.hp += hit * per_live / (per_baked + per_live);
            }
            let spell = body.zone_spell.get_or_insert(ZoneSpell {
                start: tick,
                health: body.health,
                lost: 0.0,
                last: tick,
                why: 0,
            });
            spell.why |= body.why_now;
            spell.lost += hit;
            spell.last = tick;
            body.health -= hit;
            self.zone_hp += hit;
            if body.health > 1e-3 {
                continue;
            }
            // What took the rest: the spell, and the enemy hits in it.
            let spell = body.zone_spell.take().expect("just set");
            self.zone_spells.push(spell.ended(body));
            eprintln!(
                "  zone death: at {:.1} s, in zones {:.1} s at {:.2} a second, health {:.2} then, \
                 the zones took {:.2}, last enemy hit {}",
                f64::from(tick) / 1000.0,
                f64::from(tick - spell.start) / 1000.0,
                per_s,
                spell.health,
                spell.lost,
                if body.last_hit == 0 {
                    "none this life".to_string()
                } else {
                    format!("{:.1} s before", f64::from(tick - body.last_hit) / 1000.0)
                }
            );
            body.alive = false;
            body.dead_at = tick;
            if let Some(battery) = body.carrying.take() {
                self.batteries[battery] = Lying::Free(body.feet, 1 << body.team);
            }
            let team = usize::from(body.team);
            self.zone_deaths += 1;
            self.fields.stats.deaths += u32::from(per_live >= per_baked);
            self.stats[place].1 += 1;
            self.stats[place].2 = 0;
            if let Some(tickets) = self.tickets.as_mut() {
                tickets[team] = tickets[team].saturating_sub(1);
            }
        }
    }

    /// Whether the view's ray from `body`'s eye (its base pitch) comes
    /// within `radius` of `point` no farther than the use's 2 m.
    pub(crate) fn uses(body: &Body, point: Vec3, radius: f32) -> bool {
        Self::uses_reach(body, point, radius, USE_REACH_M)
    }

    /// [`Self::uses`] with a ray of `reach` m.
    fn uses_reach(body: &Body, point: Vec3, radius: f32, reach: f32) -> bool {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let to = point - eye;
        let along = to.x * dir.x + to.y * dir.y + to.z * dir.z;
        if !(0.0..=reach).contains(&along) {
            return false;
        }
        (to - dir * along).length() <= radius
    }

    /// How far `body`'s use ray (from the eye, [`USE_REACH_M`] long)
    /// passes `point`.
    pub(crate) fn ray_miss(body: &Body, point: Vec3) -> f32 {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let along = (point - eye).dot(dir).clamp(0.0, USE_REACH_M);
        (eye + dir * along - point).length()
    }

    /// Whether the view's ray from `body`'s eye meets container `id`'s use
    /// box by the game's rule: within the use's 2 m, nothing of the bullet
    /// mesh short of it ([`mapdata::stands::ray_meets`]). A container with
    /// no use box cannot be used.
    pub(crate) fn uses_container_box(&self, body: &Body, id: usize) -> bool {
        let Some(shape) = self
            .world
            .map
            .containers
            .iter()
            .find(|c| c.id == id as i32)
            .and_then(|c| c.use_shapes.first())
        else {
            return false;
        };
        self.uses_shape(body, shape)
    }

    /// Whether the view's ray from `body`'s eye meets `shape` by the game's
    /// use rule ([`Self::uses_container_box`]).
    pub(crate) fn uses_shape(&self, body: &Body, shape: &mapdata::Shape) -> bool {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        mapdata::stands::ray_meets(&self.world.sight, eye, dir, shape)
    }

    /// [`Self::uses`] a lying battery, the ray clear of the bullet mesh up
    /// to it when [`Game::use_line`].
    pub(crate) fn uses_battery(&self, body: &Body, at: Vec3) -> bool {
        let point = at + BATTERY_MIDDLE;
        // The patched client's take (patch 11): a battery whose origin is
        // under the feet, within 1 m across, is taken along a 4 m ray that
        // passes floors (`client_patch.md`), so no line of the bullet mesh.
        // Only for one sunk by [`PATCHED_BELOW_M`] or more: the feet over a
        // floor battery's origin differ by less, and those takes keep the
        // line (the bench's A sides are made with it).
        if at.y + PATCHED_BELOW_M <= body.feet.y
            && body.feet.distance_xz(at) <= PATCHED_ACROSS_M
            && Self::uses_reach(body, point, self.take_m, PATCHED_REACH_M)
        {
            return true;
        }
        if !Self::uses(body, point, self.take_m) {
            return false;
        }
        if !self.use_line {
            return true;
        }
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let to = point - eye;
        let along = to.x * dir.x + to.y * dir.y + to.z * dir.z;
        along <= 0.15 || self.world.sight.ray_cast(eye, dir, along - 0.15).is_none()
    }

    pub(crate) fn use_and_drop(&mut self, pressed: &[u32]) {
        // The puts whose time is up go in.
        for place in 0..self.bodies.len() {
            let Some((battery, id, began)) = self.putting[place] else {
                continue;
            };
            let body = &self.bodies[place];
            if !body.alive || body.carrying != Some(battery) {
                self.putting[place] = None;
            } else if self.tick >= began + self.put_ms {
                self.putting[place] = None;
                self.put_into(place, battery, id);
            }
        }
        for place in 0..self.bodies.len() {
            let body = self.bodies[place].clone();
            if !body.alive {
                continue;
            }
            // The drop key held, or a new fire press, lets go of it: the
            // input handler takes the same branch for both (0x2D8E73..
            // 0x2D8EC8, `inventory::drop_carried_item` 0x2FC810), PROVEN;
            // 653 fire-press drops in 129 public battery matches.
            let by_fire = pressed[place] & key::FIRE != 0;
            if (body.keys & key::DROP != 0 || by_fire)
                && let Some(battery) = body.carrying
            {
                self.batteries[battery] = Lying::Free(body.feet, 1 << body.team);
                self.bodies[place].carrying = None;
                self.bodies[place].drawn_at = self.tick + DRAW_MS;
                self.drops[usize::from(!by_fire)] += 1;
                self.stolen_fell(battery, self.tick, true);
            }
            if pressed[place] & key::USE == 0 {
                continue;
            }
            let team = body.team;
            match body.carrying {
                Some(battery) => {
                    let home = self.containers.iter().position(|(t, _)| *t == team);
                    let was_putting = self.putting[place].is_some();
                    let began = !was_putting
                        && home.is_some_and(|id| {
                            self.uses_container_box(&body, id)
                        });
                    if began && let Some(id) = home {
                        if self.put_ms == 0 {
                            self.put_into(place, battery, id);
                        } else {
                            self.putting[place] = Some((battery, id, self.tick));
                        }
                    }
                    if let Some(id) = home
                        && body.feet.distance_xz(self.containers[id].1) <= ARRIVED_M
                    {
                        self.put_presses.push(PutPress {
                            place,
                            tick: self.tick,
                            feet: body.feet,
                            miss: Self::ray_miss(&body, self.containers[id].1 + USE_BOX_UP),
                            began,
                            putting: was_putting,
                        });
                    }
                }
                None => {
                    let free = self.batteries.iter().enumerate().position(|(i, b)| {
                        i >= self.untakeable
                            && matches!(b, Lying::Free(at, _) if self.uses_battery(&body, *at))
                    });
                    let theirs = self.containers.iter().position(|(t, at)| {
                        *t != team && Self::uses(&body, *at + USE_BOX_UP, USE_BOX_M)
                    });
                    if let Some(battery) = free {
                        if let Lying::Free(at, _) = self.batteries[battery] {
                            self.took_from.push(at);
                        }
                        self.batteries[battery] = Lying::Carried(place);
                        self.bodies[place].carrying = Some(battery);
                        self.taken += 1;
                        self.took[place] = Some(self.tick);
                    } else if let Some(id) = theirs
                        && let Some(battery) = self
                            .batteries
                            .iter()
                            .position(|b| *b == Lying::Home(id as u8))
                    {
                        self.batteries[battery] = Lying::Carried(place);
                        self.bodies[place].carrying = Some(battery);
                        self.points[usize::from(1 - team)] -= 1;
                        self.stolen += 1;
                        self.thieves.push(place);
                        if self.stolen_marks.len() <= battery {
                            self.stolen_marks.resize(battery + 1, None);
                        }
                        self.stolen_marks[battery] = Some((1 - team, None));
                    }
                }
            }
        }
    }

    /// The bodies' use of their own team's resupply box
    /// ([`standin_weapon::Stock::resupply`]).
    pub(crate) fn resupply(&mut self, pressed: &[u32]) {
        let tick = self.tick;
        for place in 0..self.bodies.len() {
            let body = &self.bodies[place];
            let team = i32::from(body.team);
            let shape = self
                .world
                .map
                .zones
                .iter()
                .find(|z| z.kind == mapdata::ZoneKind::Resupply && z.team == team)
                .and_then(|z| z.shapes.first());
            let meets = shape.is_some_and(|shape| self.uses_shape(body, shape));
            let free = body.alive && body.carrying.is_none();
            let (edge, held) = (pressed[place] & key::USE != 0, body.keys & key::USE != 0);
            self.bodies[place].stock.resupply(tick, edge, held, meets, free);
        }
    }

    /// `place`'s battery goes into container `id`.
    pub(crate) fn put_into(&mut self, place: Place, battery: usize, id: usize) {
        let team = self.bodies[place].team;
        self.batteries[battery] = Lying::Home(id as u8);
        self.bodies[place].carrying = None;
        self.stolen_home(battery, team);
        self.points[usize::from(team)] += 1;
        self.delivered[usize::from(team)] += 1;
        self.put_in.push(place);
        self.put_in_log.push((place, self.tick));
    }

    /// A stolen battery fell (its first fall since the theft counts).
    pub(crate) fn stolen_fell(&mut self, battery: usize, tick: u32, alive: bool) {
        if let Some(Some((_, fell @ None))) = self.stolen_marks.get_mut(battery) {
            *fell = Some((tick, alive));
            self.stolen_log.push(format!(
                "{tick} battery {battery} fell {}",
                if alive { "alive" } else { "at death" }
            ));
        }
    }

    /// A battery was put into team `team`'s container.
    pub(crate) fn stolen_home(&mut self, battery: usize, team: u8) {
        if let Some(mark) = self.stolen_marks.get_mut(battery).and_then(Option::take)
            && let (owner, Some((fell, alive))) = mark
        {
            self.stolen_drops
                .push((owner == team, self.tick.saturating_sub(fell), alive));
            self.stolen_log.push(format!(
                "{} battery {battery} home with {}, {} ms after it fell",
                self.tick,
                if owner == team {
                    "its owners"
                } else {
                    "the thieves"
                },
                self.tick.saturating_sub(fell)
            ));
        }
    }

    pub(crate) fn record(&mut self) {
        let fixes = self
            .bodies
            .iter()
            .map(|body| Fix {
                time: self.tick,
                alive: body.alive,
                inserted: body.spawned,
                feet: body.alive.then_some(body.feet),
                yaw: body.yaw,
                pitch: body.pitch,
                keys: body.keys,
                speed: Some(if body.flight.is_some() {
                    body.step_m / TICK_S
                } else {
                    let [a, l] = body.blend.now(self.kin.blend_ticks);
                    a.hypot(l)
                }),
            })
            .collect();
        let health = self.bodies.iter().map(|b| b.health).collect();
        let facts = WorldFacts {
            time: self.tick,
            stats: self.stats.clone(),
            points: Some(self.tickets.unwrap_or(self.points)),
            can_respawn: Some(self.tickets.map_or([true, true], |t| [t[0] > 0, t[1] > 0])),
            next_battery: self.timed.map(|(next, _)| next),
            batteries: self
                .batteries
                .iter()
                .map(|b| match b {
                    Lying::Free(at, _) => BatteryState::Free(*at),
                    Lying::Carried(_) => BatteryState::Carried(None),
                    Lying::Home(id) => BatteryState::Home(*id),
                })
                .collect(),
            marks: self
                .batteries
                .iter()
                .map(|b| match b {
                    Lying::Free(_, mask) => *mask,
                    Lying::Carried(_) | Lying::Home(_) => 0,
                })
                .collect(),
            container_counts: (0..self.containers.len())
                .map(|id| {
                    self.batteries
                        .iter()
                        .filter(|b| **b == Lying::Home(id as u8))
                        .count()
                })
                .collect(),
        };
        self.history.push_back((self.tick, fixes, health, facts));
        while self.history.len() > 40 {
            self.history.pop_front();
        }
        let tick = self.tick;
        let hurts = self
            .bodies
            .iter()
            .map(|b| Hurt::of(b.legs.map(Some), b.last_hit, tick))
            .collect();
        self.hurts.push_back((tick, hurts));
        while self.hurts.len() > 40 {
            self.hurts.pop_front();
        }
        self.seen
            .push_back((tick, self.bodies.iter().map(standin_heal::seen).collect()));
        while self.seen.len() > 40 {
            self.seen.pop_front();
        }
    }

    /// The bodies' hurts in the world of `tick` ([`Self::world_at`]).
    pub(crate) fn hurts_at(&self, tick: u32) -> Option<Vec<Hurt>> {
        let (_, hurts) = self.hurts.iter().find(|(t, _)| *t == tick)?;
        Some(hurts.clone())
    }

    pub(crate) fn world_at(&self, tick: u32) -> Option<Recorded> {
        self.history.iter().find(|(t, ..)| *t == tick).cloned()
    }
}

/// The respawn delay is the mode's time times the radio's `1 + modifier`
/// (not below 0), up to the 50 ms grid: the live battery modes' 10 s and
/// Team Deathmatch's 8 s, less 20 %, are the constants; a time that is not on
/// the grid goes up to it; a modifier past -1 gives none.
#[test]
fn the_respawn_delay_is_the_modes_time_less_the_radio_on_the_50_ms_grid() {
    assert_eq!(respawn_ms_of(10, -0.2), RESPAWN_BATTERIES_MS);
    assert_eq!(respawn_ms_of(8, -0.2), RESPAWN_TDM_MS);
    assert_eq!(respawn_ms_of(8, 0.0), 8_000, "no radio");
    // 9 s less 17 % is 7.47 s: the next 50 ms is 7.5 s (not 7.47, not 7.55).
    assert_eq!(respawn_ms_of(9, -0.17), 7_500);
    assert_eq!(respawn_ms_of(8, -1.5), 0, "a modifier past -1 gives no delay");
}

/// The stand-in reads the respawn from the data it runs on: on the built-in
/// test archive every playable mode has the stock 8 s (the maps test), so
/// the radio's -20 % gives 6.4 s in the battery modes too, not the live 8 s
/// the constant holds. That is why the live bench reads it from
/// `SRP_GAME_DATA`, and the constants stay the default on the built-in one.
#[test]
fn the_respawn_is_read_from_the_data_the_run_is_on() {
    use survarium::v026g::maps::{mode, weather};
    let batteries = respawn_ms_from_data(1, weather::DAY, mode::BATTERIES);
    let tdm = respawn_ms_from_data(1, weather::DAY, mode::TEAM_DEATHMATCH);
    assert_eq!(batteries, Some(6_400), "the built-in archive's 8 s less the radio");
    assert_eq!(tdm, Some(RESPAWN_TDM_MS));
    assert_ne!(batteries, Some(RESPAWN_BATTERIES_MS));
    // A map that does not exist has none.
    assert_eq!(respawn_ms_from_data(99, weather::DAY, mode::BATTERIES), None);
}

/// The bench's weapons follow the data it runs on: on the live file
/// (`SRP_GAME_DATA`) the PP-2000 fires 680 rounds a minute (not the stock 750),
/// the SR-3M 800 (not 900), and the SVD holds 5 (not 10), the content edits of
/// revisions 21 and 27 (`weapon-balance-1`). A measurement of the plumbing;
/// needs the live file.
#[test]
#[ignore = "needs SRP_GAME_DATA, the server's game data"]
fn the_bench_guns_follow_the_live_data() {
    if bench_settings().var_os("SRP_GAME_DATA").is_none() {
        eprintln!("skipping: SRP_GAME_DATA is not set");
        return;
    }
    survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
    let gun = |item: &str| super::super::guns::gun(item).unwrap_or_else(|| panic!("{item}"));
    eprintln!(
        "pp2000 {} rpm, sr3m {} rpm, svd {} rounds, ak_74u {} rpm",
        gun("pp2000").rpm,
        gun("sr3m").rpm,
        gun("svd").magazine,
        gun("ak_74u").rpm
    );
    assert_eq!(gun("pp2000").rpm, 680);
    assert_eq!(gun("sr3m").rpm, 800);
    assert_eq!(gun("svd").magazine, 5);
}
