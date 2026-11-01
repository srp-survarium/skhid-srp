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
//! | `SRP_BENCH_HIT_MODEL` ([`standin_hitbox`](super::standin_hitbox)) | off: the old 0.45 m disc about the chest, the centre ray, no scatter; `on`: a shot is a ray against the 19 hit shapes, a shotgun's ten pellets rays, and the line is scattered by the live position error |
//! | `SRP_BENCH_BATTERIES` | one battery at every spawner |
//! | `SRP_BENCH_START` | 0,0: every battery lying |
//! | `SRP_BENCH_ZONE_DAMAGE` | off: zones do not hurt the bodies |
//! | `SRP_BENCH_ZONE_INTENSITY` | on: a zone's hit is the client's rule (its hit curve at the body's place in it, the interval included); `0`: the full hit wherever a body stands in it |
//! | `SRP_BENCH_PUT_MS` | [`PUT_MS`], 600 ms |
//! | `SRP_BENCH_DRAW_TIME` | `weapon` (live): its `show_time`; `off`: [`DRAW_MS`], 800 ms for every weapon |
//! | `SRP_BENCH_BODY_BLOCK_M` | `0.76` (live): enemy bodies block (friends never); `off`: bodies pass through each other |
//! | `SRP_BENCH_JUMP_AHEAD_M` ([`standin_jump`](super::standin_jump)) | `STANDING_JUMP.ahead_m` |
//! | `SRP_BENCH_STAND_IN` | the honest jump (the client's clips); `old`: the lift up any floor |
//! | `SRP_BENCH_STAGGER_MODEL` ([`standin_stagger`](super::standin_stagger)) | on: a hit slows by the weapon's `stopping_power`; `off`: 0.75 for 0.5 s |
//! | `SRP_BENCH_HEAL_MODEL` ([`standin_heal`](super::standin_heal)) | on: the drugs and their stock; `off`: none |
//! | `SRP_BENCH_TRAPS_MODEL` ([`standin_traps`](super::standin_traps)) | on: the bear traps; `off`: none |
//!
//! The model's constants stay here: `RESPAWN_BATTERIES_MS`, `RESPAWN_TDM_MS`,
//! `DRAW_MS`, `PUT_MS`, `STAND_IN_LEG_*`, `STAND_IN_STAGGER*`.

use super::*;
use mapdata::{DamageShare, FaceArmor, Load};
use super::super::pitch::{Pitch, PitchScale};

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

/// A respawn point is taken while a living body stands within this of it, m (the check volumes of the game's point: GUESS, the shapes were not read; the points lie 1.5-3 m apart).
const SPAWN_FREE_M: f32 = 1.0;

/// Whether a spawn goes onto a free point of the team's, as the game's respawn rule does (`SRP_BENCH_SPAWN_POINTS`, on unless `random`: the old draw).
pub(crate) fn spawn_on_free_points() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    !OLD_DRAW.get() && *ON.get_or_init(|| bench_settings().var("SRP_BENCH_SPAWN_POINTS").map_or(true, |v| v != "random"))
}

thread_local! {
    /// A test whose counts were pinned on the old draw of a spawn (a random point with replacement, 2 m of jitter) holds an [`OldSpawnDraw`].
    static OLD_DRAW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The old spawn draw for this thread while it lives (the pinned soaks of `eye.rs`).
pub(crate) struct OldSpawnDraw(());

impl OldSpawnDraw {
    pub(crate) fn scope() -> Self {
        OLD_DRAW.set(true);
        Self(())
    }
}

impl Drop for OldSpawnDraw {
    fn drop(&mut self) {
        OLD_DRAW.set(false);
    }
}

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
    let (time_s, modifier) = respawn_rule_from_data(level, weather, mode)?;
    Some(respawn_ms_of(time_s, modifier))
}

/// The mode's `respawn_time` (s) and the radio's `respawn_time_modifier`, from the game data: a body that carries the radio waits
/// [`respawn_ms_of`] of both, one that does not waits the time itself (the radio works by being carried, `loadouts::BOT_RADIO`).
pub(crate) fn respawn_rule_from_data(level: u8, weather: u8, mode: u8) -> Option<(u32, f32)> {
    let row = survarium::v026g::maps::find(level, weather, mode)?;
    let radio = survarium::v026g::dictionary::Dictionary::shared()
        .item_named("respawn_booster")?
        .dict_id;
    let modifier = survarium::v026g::loadouts::default_modifier(radio, "respawn_time_modifier");
    Some((row.respawn_time, modifier))
}

/// The radio's modifier when the data is not read: -20 %, which with the modes' 10 s (batteries) and 8 s (Team Deathmatch) is
/// [`RESPAWN_BATTERIES_MS`] and [`RESPAWN_TDM_MS`].
pub(crate) const RADIO_MODIFIER: f32 = -0.2;
/// The modes' respawn times when the data is not read, s.
pub(crate) const RESPAWN_BATTERIES_S: u32 = 10;
pub(crate) const RESPAWN_TDM_S: u32 = 8;

/// How long after a carrier lets go of the battery his weapon fires again,
/// ms, when the stand-in does not read it from the weapon: the weapon's show
/// time follows the drop, about 0.8 s for most weapons (LIKELY: private
/// sprint-jump-re FINDINGS section 9; the stand-in's bodies hold their
/// primary).
pub(crate) const DRAW_MS: u32 = 800;

/// [`draw_ms`] with the choice made: the weapon's `show_time`
/// ([`Gun::show_ms`], read from its config) when `from_weapon`, else
/// [`DRAW_MS`]. Of the 83 weapons of the live data 73 show in 0.8 s; the
/// TT-33, PM, APS, Fort-17, Magnum, TOZ-66, SVD and PP-2000 in 0.5 s and the
/// RPD and PKP in 1.0 s (PROVEN in the configs), so the weapon's time moves
/// the stand-in for the bodies that hold those.
pub(crate) fn draw_ms_of(gun: &Gun, from_weapon: bool) -> u32 {
    if from_weapon { gun.show_ms } else { DRAW_MS }
}

/// How long after a carrier drops a battery his weapon fires again, ms:
/// the weapon's own `show_time` with `SRP_BENCH_DRAW_TIME=weapon`, else
/// [`DRAW_MS`] for every weapon (`off`: the stand-in as it was; `weapon` is the
/// default, the game's own timing).
pub(crate) fn draw_ms(gun: &Gun) -> u32 {
    static FROM_WEAPON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let from_weapon = *FROM_WEAPON
        .get_or_init(|| bench_settings().var("SRP_BENCH_DRAW_TIME").as_deref() == Ok("weapon"));
    draw_ms_of(gun, from_weapon)
}

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
    /// How far along the use ray the container's box is entered, if it is
    /// ([`mapdata::stands::ray_distance`]): the length of use the press
    /// needed.
    pub(crate) reach: Option<f32>,
}

/// Whose body a kill is: an enemy's, a teammate's (the game has friendly
/// fire), or the killer's own (a grenade at one's feet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kill {
    Enemy,
    Mate,
    Own,
}

/// A teammate's death, for the owner counts of the team kills (`tallies/owner/team_kills.rs`): when, who, by what.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TeamKill {
    pub(crate) tick: u32,
    pub(crate) killer: Place,
    pub(crate) target: Place,
    /// A grenade's blast; else a bullet.
    pub(crate) frag: bool,
    pub(crate) killer_feet: Vec3,
    pub(crate) target_feet: Vec3,
}

/// A bullet or pellet that met a body's hit shape: the body, the distance along the ray, the hit box.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoxStrike {
    pub(crate) body: Place,
    pub(crate) along: f32,
    pub(crate) param: standin_hitbox::HitParam,
    /// What the bullet carries at the body, its damage and its pierce, after the distance and the surfaces it crossed to
    /// get there (`mapdata::RayTree::fly`: the client's penetration), and the share of the damage that is of what an
    /// empty line would give.
    pub(crate) load: Load,
    pub(crate) share: DamageShare,
}

/// What the bullets' lines met in the stand-in: the strikes' lines by verdict and the armor that stopped them, and the
/// bots' shots at their target whose ray met the world short of him by the share the bullet kept. For the
/// `owner-test pierce` counts ([`super::tallies`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct PierceStats {
    /// A strike line (a bullet's way to a body it would hit): nothing in the way or only free armor, through cover
    /// that took some of it, stopped.
    pub(crate) strikes_open: u32,
    pub(crate) strikes_partial: u32,
    pub(crate) strikes_stopped: u32,
    /// Front faces the strikes' bullets crossed, in all.
    pub(crate) faces_crossed: u32,
    /// The stopped strikes by the face that stopped them (the bits of [`FaceArmor::to_wire`]; the one NaN is a face
    /// with no material).
    pub(crate) stopped_by: std::collections::BTreeMap<u32, u32>,
    /// By team of the shooter, the walled shots by share kept: stopped, up to 0.25, up to 0.5, under 1, all.
    pub(crate) walled_bands: [[u32; 5]; 2],
}

impl PierceStats {
    pub(crate) fn band(share: DamageShare) -> usize {
        match share.value() {
            s if s <= 0.0 => 0,
            s if s <= 0.25 => 1,
            s if s <= 0.5 => 2,
            s if s < 1.0 => 3,
            _ => 4,
        }
    }

    pub(crate) fn strike(&mut self, line: &mapdata::Followed) {
        self.faces_crossed += line.crossed;
        match line.share.value() {
            s if s <= 0.0 => {
                self.strikes_stopped += 1;
                if let Some(face) = line.stopped_by {
                    *self.stopped_by.entry(face.to_wire().to_bits()).or_default() += 1;
                }
            }
            s if s < 1.0 => self.strikes_partial += 1,
            _ => self.strikes_open += 1,
        }
    }
}

/// A body of the stand-in game.
#[derive(Clone, Debug)]
pub(crate) struct Body {
    /// The weapon in hand (the bot's, as its last input was made), its
    /// kick and its trigger.
    pub(crate) gun: &'static Gun,
    pub(crate) recoil: Recoil,
    pub(crate) cadence: Cadence,
    /// The bot's mirror of the kick as its last input was made: the kick after the turn, before the shots (pitch units), and
    /// the shots it counted ([`Reflex::seen`]); read against `recoil` at the shot ([`KickPair`], counts only).
    pub(crate) mirror: Option<(f32, u8)>,
    pub(crate) team: u8,
    pub(crate) feet: Vec3,
    pub(crate) node: Option<NodeId>,
    pub(crate) yaw: f32,
    /// The base pitch (`m_look_pitch`, as the worlds give it); the view is
    /// it and the kick.
    pub(crate) pitch: f32,
    /// How the pitch unit relates to the elevation the shots and the eye go along (`SRP_BENCH_PITCH_SCALE`).
    pub(crate) scale: PitchScale,
    pub(crate) blend: Blend,
    pub(crate) keys: u32,
    pub(crate) health: f32,
    /// Its other pools ([`standin_damage::Pools`]): `health` is the body's.
    pub(crate) pools: standin_damage::Pools,
    /// Its weapon's rounds and ammo ([`standin_weapon`]).
    pub(crate) stock: standin_weapon::Stock,
    /// Its stamina bar ([`standin_stamina`]).
    pub(crate) stamina: standin_stamina::Stamina,
    pub(crate) alive: bool,
    pub(crate) dead_at: u32,
    pub(crate) spawned: u32,
    /// Whether each of its three kits carries the radio (`respawn_booster`): the respawn booster works by being carried, so the delay
    /// of a death is the mode's time less the radio's only in a kit that has one ([`Game::set_radio_kits`]).
    pub(crate) radio: [bool; 3],
    pub(crate) carrying: Option<usize>,
    /// No shot before this tick: the weapon coming up after a drop
    /// ([`draw_ms`]).
    pub(crate) drawn_at: u32,
    pub(crate) moved: f32,
    /// Until when a jump pressed keeps the body in the air, ms
    /// ([`STAND_IN_AIR_MS`]; the old stand-in).
    pub(crate) air_until: u32,
    /// The jump under way (the honest stand-in, [`Flight`]), and the
    /// ground distance moved at the last tick, m.
    pub(crate) flight: Option<Flight>,
    pub(crate) step_m: f32,
    /// The ground displacement of the last tick, m (`SRP_BENCH_HIT_FLIGHT`: where a bullet in the air will find it).
    pub(crate) step_vec: Vec3,
    /// The tick a sprint run came to the face of a jump link up and was held there ([`lift_to`]); the lift takes
    /// [`LIFT_HOLD_MS`] of it.
    pub(crate) lift_since: Option<(u32, Option<f32>)>,
    /// The speed of the slide off steep ground it is on, m/s ([`standin_slide`], `SRP_BENCH_STAND_IN_SLIDE=on`); none: not sliding.
    pub(crate) slide_mps: Option<f32>,
    /// The leg pools (0.5 full), until when an empty one is broken, and
    /// the last hit's tick (0: none this life), [`STAND_IN_LEG_SHARE`].
    pub(crate) legs: [f32; 2],
    pub(crate) leg_broken_until: u32,
    pub(crate) last_hit: u32,
    /// The hits of the last half second and their stopping power ([`standin_stagger`]).
    pub(crate) stagger: standin_stagger::Stagger,
    /// The movement level of the body's kit ([`standin_level`], `SRP_BENCH_STAND_IN_LEVEL`): 1.0, the nominal speed, unless set.
    pub(crate) speed_level: f32,
    /// The bot's target as its last input was made (the reflex's).
    pub(crate) target: Option<Place>,
    /// Its shots' spread ([`bench_spread`]).
    pub(crate) spread: bench_spread::Spread,
    /// Its stretch of zone damage under way ([`ZoneSpell`]).
    pub(crate) zone_spell: Option<ZoneSpell>,
    /// At its last tick it stood in a zone's shape with no zone there on ([`ZoneNow`]).
    pub(crate) zone_prev_off: bool,
    /// Its feet height at each tick of the last second, by the tick mod 20 (how far it came down into a zone).
    pub(crate) zone_ys: [f32; 20],
    /// Its bot's motives at its last tick (`Bot::bench_why`).
    pub(crate) why_now: u16,
    /// Its drugs, and the pools the fields above lack ([`standin_heal`]).
    pub(crate) heal: standin_heal::Heal,
    pub(crate) rest: standin_heal::Rest,
    /// Its frag grenades: the store a spawn fills the hand from ([`standin_grenade::Stock`]).
    pub(crate) grenades: standin_grenade::Stock,
    /// Its traps and the catch's slowing ([`standin_traps`]).
    pub(crate) trap: standin_traps::Rig,
}

impl Body {
    /// The elevation the view looks along, radians: the pitch and the weapon's kick (0x2E0F60) through the unit's scale.
    pub(crate) fn view_elevation(&self) -> f32 {
        Pitch::view(self.pitch, self.recoil.y()).elevation(self.scale)
    }

    /// The elevation of the base pitch alone, radians.
    pub(crate) fn base_elevation(&self) -> f32 {
        Pitch::new(self.pitch).elevation(self.scale)
    }
}

/// A bench run has begun (the bench's setup calls it before any game is made): the pitch unit is then the game's unless
/// `SRP_BENCH_PITCH_SCALE` says otherwise. The unit tests' bodies keep the unit as a radian, the geometry their numbers were made in.
pub(crate) fn start_bench_run() {
    BENCH_RUN.store(true, std::sync::atomic::Ordering::Relaxed);
}

static BENCH_RUN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// How the stand-in's pitch unit relates to the elevation (`SRP_BENCH_PITCH_SCALE`: `radian` or `client`; unset: `client` in a bench
/// run, `radian` in a unit test).
pub(crate) fn pitch_scale() -> PitchScale {
    static SCALE: std::sync::OnceLock<PitchScale> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| match bench_settings().var("SRP_BENCH_PITCH_SCALE").as_deref() {
        Ok("radian") => PitchScale::Radian,
        Ok("client") => PitchScale::Client,
        _ if BENCH_RUN.load(std::sync::atomic::Ordering::Relaxed) => PitchScale::Client,
        _ => PitchScale::Radian,
    })
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
    /// Of the `walled` shots, those whose bullet the world did not stop at the target's place (it crossed
    /// surfaces that let it through, `RayTree::damage_share` over 0), by the shooter's team.
    pub(crate) walled_through_by: [u32; 2],
    /// Enemy hits and kills by bullets that had crossed cover on the way (a share under 1), by the shooter's team.
    pub(crate) cover_hits_by: [u32; 2],
    pub(crate) cover_kills_by: [u32; 2],
    /// What the bullets' lines met: [`PierceStats`] (counted where a line is tested, which takes `&self`).
    pub(crate) pierce_stats: std::cell::RefCell<PierceStats>,
    /// Whether bullets go through cover by the client's rule ([`Self::cover_flight`]); off, the first surface of the mesh
    /// stops them, as it did before the mesh had materials (the bench's all-off arm). On by default.
    pub(crate) pierce: bool,
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
    /// Zones hurt the pools their hits take from, through the body's armour on the parts they hit
    /// (`HazardTuning::zone_pools` routed, `SRP_BENCH_ZONE_POOLS=routed`: [`Game::zone_tick_routed`]); else
    /// the body pool and the raw hit.
    pub(crate) zone_routed: bool,
    /// A zone's hit is the client's rule (`SRP_BENCH_ZONE_INTENSITY`, on in the bench; [`Hazard::hit_rate_at`],
    /// [`Hazard::client_cost_per_s`] at the body's centre): the hit curve at its place in the zone, the interval included;
    /// else the full hit wherever a body stands in it.
    pub(crate) zone_intensity: bool,
    /// What the routed zones took by pool (body, head, infection, pain) and the deaths in them by the pool
    /// that ran out.
    pub(crate) zone_pool_lost: [f32; 4],
    pub(crate) zone_pool_deaths: [u32; 4],
    /// The anomaly fields of the redesign, made by the bench ([`standin_fields`]); off unless `SRP_BENCH_LIVE_FIELDS`.
    pub(crate) fields: standin_fields::FieldGen,
    /// Zone spells ended ([`ZoneSpell::ended`]): the health the zones took,
    /// the body's health at the end (enemy fire counts; a death, 0) and at
    /// the start, and 1 if in a fight ([`SPELL_FIGHT_MS`]).
    pub(crate) zone_spells: Vec<[f32; 5]>,
    /// The same spells with how they began ([`ZoneNote`]), for the owner topic.
    pub(crate) zone_notes: Vec<ZoneNote>,
    pub(crate) took_from: Vec<Vec3>,
    /// For each take of a lying battery, how far along the use ray its
    /// middle lay (m): the length of use the take needed.
    pub(crate) take_alongs: Vec<f32>,
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
    /// Test only: one `CARRYKILL` line a carrier killed (where both stood, the time since the pick-up, the distance), for the bench log.
    pub(crate) carry_kill_lines: Vec<String>,
    /// Every kill: (tick, the shooter, the killed), for the bench's
    /// events (`SRP_BENCH_EVENTS`).
    pub(crate) kill_log: Vec<(u32, Place, Place)>,
    /// Every teammate's death ([`TeamKill`]), for the owner counts.
    pub(crate) team_kill_log: Vec<TeamKill>,
    /// The next [`Self::kill`] is a grenade's blast (set around its call).
    pub(crate) kill_by_frag: bool,
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
    /// The mode's respawn time (s) and the radio's modifier, when the run is on the live game data (`SRP_GAME_DATA`:
    /// [`respawn_rule_from_data`]); `None` keeps [`RESPAWN_BATTERIES_S`] / [`RESPAWN_TDM_S`] and [`RADIO_MODIFIER`]. A body's
    /// delay is [`Game::respawn_ms_of_body`]: with the radio of the kit it died in or without it.
    pub(crate) respawn_rule: Option<(u32, f32)>,
    /// Batteries no use takes off the floor (`SRP_BENCH_UNTAKEABLE=<n>`:
    /// the first n; as the sunk ones of audit #7, which a use never took).
    pub(crate) untakeable: usize,
    /// How long a put-in takes ([`PUT_MS`]), and each place's put under
    /// way: the battery, the container, and when it began. A use pressed
    /// while one is under way does nothing (GUESS); the put ends unmade if
    /// the body died or no longer carries that battery.
    pub(crate) put_ms: u32,
    /// Enemy bodies block each other on the ground (`SRP_BENCH_BODY_BLOCK_M=<m>`, the two radii: 0.76 m in the game, PROVEN
    /// from the J0 matches' bodies files); none: bodies pass through each other, friends always do.
    pub(crate) body_block_m: Option<f32>,
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
    /// A place whose every life starts on a point of its own, not on a spawn point of its team ([`PlacedStart`]).
    pub(crate) placed_start: Option<PlacedStart>,
    /// Batteries moved unseen (audit #7, `SRP_BENCH_NUDGE=<m>`): every
    /// [`NUDGE_EVERY_MS`] one lying battery, in turn, moves this far on its
    /// floor, as one an enemy took and dropped dead a step on (the owner's
    /// match 0x4da3b51d, battery 3: 0.7 m); the next nudge's time, and how
    /// many moved.
    pub(crate) nudge: Option<(f32, u32)>,
    pub(crate) nudged: u32,
    /// Every shot, for the bench's fight tally ([`fight_tally`]).
    pub(crate) shot_facts: Vec<fight_tally::ShotFact>,
    /// The kick of a shooter's first shot of a tick against the bot's mirror of it, and each tick's shots against the mirror's
    /// ([`fight_tally::KickPair`], [`fight_tally::KickTick`]; the owner counts' `kick_gap`).
    pub(crate) kick_pairs: Vec<fight_tally::KickPair>,
    pub(crate) kick_ticks: Vec<fight_tally::KickTick>,
    /// Shots spread as the game's do ([`bench_spread`], `SRP_BENCH_SPREAD`).
    pub(crate) spread: bool,
    /// What a shot is tested against ([`HitVolume`]): the old disc here; the bench sets its own
    /// ([`HitVolume::bench`], the old disc unless `SRP_BENCH_HIT_MODEL=on`).
    pub(crate) volume: HitVolume,
    /// The bullet flies the client's path (`SRP_BENCH_HIT_FLIGHT`, A182; [`Self::nearest_shape_flown`]): muzzle speed, drag and
    /// gravity, each body found where its last step carries it by the time, not where it stood at the shot.
    pub(crate) flight: standin_hitbox::BulletPath,
    /// Whether bodies move as the client's do ([`honest_stand_in`]); the
    /// walker mesh a body in the air bumps into (built at the first jump;
    /// the sight's when the map has none, as the tests' fields); every
    /// jump's landing.
    pub(crate) honest: bool,
    pub(crate) walls: Option<Arc<mapdata::RayTree>>,
    /// The walls' horizontal cross-sections for the capsule rule ([`standin_capsule`]), built at the first move.
    pub(crate) sections: Option<Arc<standin_capsule::Sections>>,
    /// The ground of the floors bodies stand on, by face material and slope ([`standin_slide::Grounds`]), built at the first need.
    pub(crate) grounds: Option<standin_slide::Grounds>,
    /// What the bodies slid ([`standin_slide::Slides`]).
    pub(crate) slides: standin_slide::Slides,
    pub(crate) landings: Vec<Landing>,
    /// The height the bodies gained, by way, against what the client's body can climb ([`standin_climb`]).
    pub(crate) climbs: standin_climb::Climbs,
    /// Every press of a quick-use key ([`standin_heal::press_keys`]), and the
    /// bodies' pools and drugs in the worlds of the last two seconds
    /// ([`Game::seen_at`]).
    pub(crate) drug_log: Vec<standin_heal::DrugPress>,
    pub(crate) seen: VecDeque<(u32, Vec<standin_heal::Seen>)>,
    /// The bear traps placed, and what befell them ([`standin_traps`]).
    pub(crate) traps: standin_traps::Traps,
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

/// A place whose every life starts on a point of its own, not on a spawn point of its team: the stand-in camper on his spot
/// (`SRP_BENCH_CAMPER_START=spot`); and how long its next life waits after the respawn, ms: [A239b] his walk from a spawn to that
/// point (`SRP_BENCH_CAMPER_ARRIVE=live`), else 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PlacedStart {
    pub(crate) place: Place,
    pub(crate) at: Vec3,
    pub(crate) walk_ms: u32,
}

impl Game {
    /// [A239b] The wait after the respawn of `place` for its walk to its own start point ([`PlacedStart`]), ms; 0 for any other.
    pub(crate) fn placed_walk_ms(&self, place: Place) -> u32 {
        self.placed_start.filter(|s| s.place == place).map_or(0, |s| s.walk_ms)
    }

    /// The respawn delay after the death of `place`, ms: the mode's time times `1 + the radio's modifier` if the kit it died in carries
    /// the radio, the time itself if not.
    pub(crate) fn respawn_ms_of_body(&self, place: Place) -> u32 {
        let (time_s, modifier) = self.respawn_rule.unwrap_or_else(|| {
            (if self.tickets.is_some() { RESPAWN_TDM_S } else { RESPAWN_BATTERIES_S }, RADIO_MODIFIER)
        });
        let body = &self.bodies[place];
        let carries = body.radio.get(body.heal.kit()).copied().unwrap_or(true);
        respawn_ms_of(time_s, if carries { modifier } else { 0.0 })
    }

    /// Which of each bot's three kits carry the radio, from the profiles (by place): a profile whose kit has no `respawn_booster`
    /// pocket waits the whole time. A place with no profile keeps the radio in every kit (today's bots', all of which carry one).
    pub(crate) fn set_radio_kits(&mut self, profiles: &[survarium::v026g::match_msgs::static_info::MatchProfile]) {
        for (body, profile) in self.bodies.iter_mut().zip(profiles) {
            let Some(pockets) = crate::v026g::bots::pockets::ProfilePockets::of_profile(profile, true) else {
                continue;
            };
            for (k, kit) in pockets.kits().iter().enumerate().take(3) {
                body.radio[k] = kit.iter().flatten().any(|pocket| pocket.name == "respawn_booster");
            }
        }
    }

    /// The frag grenades each bot's three kits hold, by place ([`standin_grenade::Stock`]): the first life's hand is filled at
    /// once. A match that does not call this carries a grenade for every throw.
    pub(crate) fn set_grenade_kits(&mut self, kits: &[[standin_grenade::KitGrenades; 3]]) {
        for (body, kits) in self.bodies.iter_mut().zip(kits) {
            body.grenades = standin_grenade::Stock::of(*kits);
            body.grenades.spawn();
        }
    }

    /// The movement level of each place's body ([`standin_level::levels`]); a place with none keeps the nominal speed.
    pub(crate) fn set_speed_levels(&mut self, levels: &[Option<f32>]) {
        for (body, level) in self.bodies.iter_mut().zip(levels) {
            if let Some(level) = *level {
                body.speed_level = level;
            }
        }
    }

    /// The drugs each bot's own profile holds ([`standin_heal::Heal::of_profile`]), by place: the first life's slots are filled
    /// at once. A match that does not call this has the standard kit's, all in store as the old profiles had it.
    pub(crate) fn set_heal_kits(&mut self, profiles: &[survarium::v026g::match_msgs::static_info::MatchProfile]) {
        for (body, profile) in self.bodies.iter_mut().zip(profiles) {
            body.heal = standin_heal::Heal::of_profile(profile);
            body.heal.spawn();
        }
    }

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
            mirror: None,
            team,
            feet: Vec3::ZERO,
            node: None,
            yaw: 0.0,
            pitch: 0.0,
            scale: pitch_scale(),
            blend: Blend::default(),
            keys: 0,
            health: 1.0,
            pools: standin_damage::Pools::default(),
            stock: standin_weapon::Stock::new(bench_tier(), gun),
            stamina: {
                let _kit = super::team_kit::scope(team);
                standin_stamina::Stamina::new(0, bench_tier(), gun.item)
            },
            alive: false,
            dead_at: 0,
            spawned: 0,
            radio: [true; 3],
            carrying: None,
            drawn_at: 0,
            moved: 0.0,
            air_until: 0,
            flight: None,
            step_m: 0.0,
            step_vec: Vec3::ZERO,
            lift_since: None,
            slide_mps: None,
            legs: [0.5; 2],
            leg_broken_until: 0,
            last_hit: 0,
            stagger: standin_stagger::Stagger::default(),
            speed_level: 1.0,
            target: None,
            spread: bench_spread::Spread::default(),
            zone_spell: None,
            zone_prev_off: false,
            zone_ys: [f32::NEG_INFINITY; 20],
            why_now: 0,
            heal: standin_heal::Heal::new(),
            rest: standin_heal::Rest::FULL,
            grenades: standin_grenade::Stock::Unlimited,
            trap: standin_traps::Rig::default(),
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
            walled_through_by: [0; 2],
            cover_hits_by: [0; 2],
            cover_kills_by: [0; 2],
            pierce_stats: Default::default(),
            pierce: true,
            hidden_by: [0; 2],
            hidden: 0,
            metric_rays: 0,
            delivered: [0, 0],
            stolen: 0,
            drops: [0, 0],
            zone_deaths: 0,
            zone_hp: 0.0,
            zone_routed: false,
            zone_intensity: false,
            zone_pool_lost: [0.0; 4],
            zone_pool_deaths: [0; 4],
            fields: standin_fields::FieldGen::from_env(&world, seed),
            zone_spells: Vec::new(),
            zone_notes: Vec::new(),
            took_from: Vec::new(),
            take_alongs: Vec::new(),
            carry_m: 0.0,
            carry_ticks: 0,
            thieves: Vec::new(),
            carrier_kills: Vec::new(),
            carry_kill_lines: Vec::new(),
            kill_log: Vec::new(),
            team_kill_log: Vec::new(),
            kill_by_frag: false,
            hit_log: Vec::new(),
            stolen_marks: Vec::new(),
            stolen_drops: Vec::new(),
            stolen_log: Vec::new(),
            taken: 0,
            took: vec![None; teams.len()],
            taken_killed: 0,
            use_line: false,
            take_m: BATTERY_M,
            respawn_rule: None,
            untakeable: 0,
            put_ms: bench_settings().number("SRP_BENCH_PUT_MS")
                .unwrap_or(PUT_MS),
            body_block_m: bench_settings().number("SRP_BENCH_BODY_BLOCK_M").filter(|&m: &f32| m > 0.0),
            putting: vec![None; teams.len()],
            put_presses: Vec::new(),
            put_in_log: Vec::new(),
            camp_m: None,
            camper: None,
            camp_done: false,
            camp_ended: None,
            placed_start: None,
            nudge: None,
            nudged: 0,
            shot_facts: Vec::new(),
            kick_pairs: Vec::new(),
            kick_ticks: Vec::new(),
            spread: false,
            volume: HitVolume::Disc,
            flight: standin_hitbox::BulletPath::Instant,
            honest: honest_stand_in(),
            walls: None,
            sections: None,
            grounds: None,
            slides: standin_slide::Slides::default(),
            landings: Vec::new(),
            climbs: standin_climb::Climbs::default(),
            drug_log: Vec::new(),
            seen: VecDeque::new(),
            traps: standin_traps::Traps::default(),
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
        // The game's respawn rule gives a spawning place a free point of its team and spawns it ON the point (`select_respawn_points` 0x3171D0: a point whose
        // two check volumes meet no body, taken in the rule's random order; 1,809 live bodies in their first 3 s: median 0.01 m from a point, 8 of 1,801 on a
        // point another team-mate stood on, 0 of 790 pairs within 1 m). The bench does too (`SRP_BENCH_SPAWN_POINTS=random` keeps the old draw: a random point of the team with replacement and 2 m of jitter). The old draw is
        // not what puts bots on each other: the overlap in a life's first 10 s is the same with either (A175).
        let (point, at) = if let Some(PlacedStart { at: spot, .. }) = self.placed_start.filter(|s| s.place == place) {
            (spot, spot)
        } else if spawn_on_free_points() {
            let free: Vec<Vec3> = spawns
                .iter()
                .copied()
                .filter(|&s| {
                    !self.bodies.iter().enumerate().any(|(other, b)| {
                        other != place && b.alive && b.feet.distance_xz(s) < SPAWN_FREE_M && (b.feet.y - s.y).abs() < 2.0
                    })
                })
                .collect();
            let pool = if free.is_empty() { &spawns } else { &free };
            let point = pool[self.rng.below(pool.len())];
            (point, point)
        } else {
            let point = spawns[self.rng.below(spawns.len())];
            (point, point + Vec3::new(self.rng.range(-2.0, 2.0), 0.0, self.rng.range(-2.0, 2.0)))
        };
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
        // The team's kit switches (`SRP_BENCH_UPGRADES_ONLY`): its arms, ammo and stamina rates are built under them.
        let _kit = super::team_kit::scope(body.team);
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
        body.stagger.reset();
        body.flight = None;
        body.step_m = 0.0;
        body.step_vec = Vec3::ZERO;
        body.zone_spell = None;
        body.zone_prev_off = false;
        body.zone_ys = [f32::NEG_INFINITY; 20];
        body.alive = true;
        body.spawned = tick;
        body.carrying = None;
        body.heal.spawn();
        body.grenades.spawn();
        body.rest = standin_heal::Rest::FULL;
        // Weapon 1 in hand afresh.
        body.gun = Arms::fallback().primary.0;
        body.recoil = Recoil::new(kick_of(body.gun.item));
        body.cadence = Cadence::default();
        body.spread.reset();
        body.stock.spawn(tier, body.gun);
        body.stamina.spawn(tick, tier, body.gun.item);
    }

    /// Where the game's bullet starts, from the bots' own function
    /// ([`super::view::eye_height`]) at the body's pace and view.
    pub(crate) fn eye(body: &Body) -> Vec3 {
        let speed = body.step_m / TICK_S;
        let pitch = body.view_elevation();
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
        let mut released = vec![0u32; n];
        for body in &mut self.bodies {
            body.recoil.update(tick);
        }
        let tier = self.damage.tier;
        // Where the bodies stood at the start of the tick, for the enemy block (`body_block_m`).
        let mut standing: Vec<(Vec3, u8, bool)> = if self.body_block_m.is_some() {
            self.bodies.iter().map(|b| (b.feet, b.team, b.alive)).collect()
        } else {
            Vec::new()
        };
        let mut suicides = Vec::new();
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
                    // A broken leg refuses it, and so does a bar under its cost or
                    // the low flag (nothing is spent then).
                    if body.legs.iter().all(|&pool| pool > 0.0) && body.stamina.jump() {
                        if self.honest {
                            jump = true;
                        } else {
                            body.air_until = tick + STAND_IN_AIR_MS;
                        }
                    } else {
                        self.jumps_refused += 1;
                    }
                }
                released[place] = body.keys & !keys;
                // The suicide bit (24): the game's own hit on the player's own head, the first input that has it.
                if pressed[place] & key::SUICIDE != 0 && body.alive {
                    suicides.push(place);
                }
                body.keys = keys;
                body.target = target;
                // Another weapon taken, or a reload: no kick.
                if gun.item != body.gun.item {
                    let _kit = super::team_kit::scope(body.team);
                    body.gun = gun;
                    body.recoil = Recoil::new(kick_of(gun.item));
                    body.recoil.update(tick);
                    body.cadence = Cadence::default();
                    body.spread.reset();
                    body.stock.switch(tier, gun);
                    body.stamina.switch(tier, gun.item);
                } else if pressed[place] & key::RELOAD != 0 {
                    body.recoil.reset();
                    body.stock.reload_key(tick);
                }
                body.yaw += turn.0;
                let taken = body.recoil.turn(turn.1);
                body.pitch = (body.pitch + turn.1 + taken).clamp(-1.0, 1.0);
                // The honest stand-in's jump: the keys and the facing at
                // the press pick its clip and way; in the air only the
                // air control steers (`SRP_BENCH_STAND_IN_AIR`).
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
                        air: [0.0; 2],
                    });
                }
            }
        }
        // A death, a ticket, the battery dropped, and the respawn of the mode: as the game's, no one's kill.
        for place in suicides {
            self.kill(place, place, Kill::Own, tick);
        }
        if self.walls.is_none() && (self.bodies.iter().any(|b| b.flight.is_some()) || (stand_in_edge() && stand_in_fall())) {
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
        self.trap_place(&released, tick);
        self.trap_tick(tick);
        self.camp(tick, &mut pressed);
        // The shots of the tick leave from where the world of the tick
        // shows the bodies, before this tick's move (a bullet starts at
        // the world's position of its own tick, PROVEN live: 0.08 +-0.03
        // tick of travel, `fact-own-eye.md` 4), as `lead_own` reckons.
        if super::view::live_eye() {
            self.shoot(tick);
        }
        let mut segs: Vec<standin_capsule::Seg> = Vec::new();
        for place in 0..n {
            if self.camper.is_some_and(|(p, _)| p == place) {
                continue;
            }
            if !self.bodies[place].alive {
                let team = usize::from(self.bodies[place].team);
                let may = self.tickets.is_none_or(|tickets| tickets[team] > 0);
                let respawn = self.respawn_ms_of_body(place) + self.placed_walk_ms(place);
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
            let mut level = if body.carrying.is_some() { 0.95 } else { 1.0 } * body.speed_level;
            if standin_stagger::model_on() {
                level *= 1.0 - body.stagger.share(tick);
            } else if body.last_hit != 0 && tick < body.last_hit + STAND_IN_STAGGER_MS {
                level *= STAND_IN_STAGGER;
            }
            // A body whose legs a trap took does not sprint and walks slower
            // ([`standin_traps`]).
            let caught = body.trap.slowed(tick);
            if caught {
                level *= standin_traps::SLOW;
            }
            // A carrier cannot sprint: the sprint check asks the item in
            // hand, and the battery's answer is no (0x397C20), PROVEN; seen
            // in 0x60aeff4f, the owner held sprint carrying at 3.35 m/s.
            // A body with the low flag set, or over its capacity, walks at
            // its walk speed with the sprint keys held; the bar is spent on
            // the keys all the same.
            let keys = if body.carrying.is_some() || caught || !body.stamina.may_sprint() {
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
                self.climbs.alive_ticks += 1;
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
                    // Keys held in the air steer it only with `SRP_BENCH_STAND_IN_AIR` (`standin_jump::air_control`).
                    let walls = self.walls.as_deref().expect("built at the press");
                    let took_off = body.flight.map(|f| (f.from, f.base));
                    let sections = stand_in_corner().then(|| {
                        self.sections
                            .get_or_insert_with(|| Arc::new(standin_capsule::Sections::new(&self.world.map.walker_triangles(), grid.body().min_ground_normal_y)))
                            .clone()
                    });
                    let wall = sections.as_deref().map(|sections| standin_jump::JumpWall {
                        sections,
                        hold: stand_in_jump_wall(),
                        ticks: &mut self.climbs.prewalk_ticks,
                        held: &mut self.climbs.prewalk_held,
                    });
                    if let Some(landing) = fly(grid, walls, wall, body, place, tick) {
                        if let Some((from, base)) = took_off {
                            // The rise is from the floor the body left (the walk before the jump is the walk's), not from the press.
                            let from = Vec3::new(from.x, base.unwrap_or(from.y), from.z);
                            let way = if landing.sprint { standin_climb::Way::SprintJump } else { standin_climb::Way::ShortJump };
                            self.climbs.note(way, tick, place, body.team, from, landing.to);
                        }
                        self.landings.push(landing);
                    }
                } else {
                    // The client's capsule against the walls (A191): the move the walker mesh lets it make, the lattice walks the rest.
                    let target = if stand_in_corner() {
                        let sections = self.sections.get_or_insert_with(|| {
                            Arc::new(standin_capsule::Sections::new(&self.world.map.walker_triangles(), grid.body().min_ground_normal_y))
                        });
                        let mv = (to.x - body.feet.x, to.z - body.feet.z);
                        let reach = mv.0.hypot(mv.1) + standin_capsule::RADIUS_M + 0.6;
                        sections.near((body.feet.x, body.feet.z), body.feet.y, body.keys & key::CROUCH != 0, reach, &mut segs);
                        let held = standin_capsule::client_move(&segs, (body.feet.x, body.feet.z), mv, standin_capsule::RADIUS_M);
                        standin_capsule::counts::note(&held, mv.0.hypot(mv.1) > 1e-4);
                        Vec3::new(held.to.0, to.y, held.to.1)
                    } else {
                        to
                    };
                    // The rise a walk climbs is the grid's (with `capsule_steps`, the capsule's edge step).
                    let (mut node, mut feet, _) = if stand_in_edge() {
                        let slid = slide_supported(grid, node, body.feet, target, grid.body().walk_climb(), body::SUPPORT_M);
                        // Past a ledge's edge with no drop link ahead: on, or down, as the client's capsule (`stand_in_fall`).
                        let walls = self.walls.as_deref().filter(|_| stand_in_fall());
                        match walls.and_then(|walls| past_edge(grid, walls, slid.0, slid.1, target)) {
                            Some((n, f)) => (n, f, false),
                            None => slid,
                        }
                    } else {
                        slide_capped(grid, node, body.feet, target, grid.body().walk_climb())
                    };
                    // The client's sprint lift: a sprint run held at the face of a low top is carried up it.
                    let pushing = keys & key::SPRINT != 0 && keys & key::FORWARD != 0 && feet.distance_xz(to) > 1e-3;
                    if !pushing {
                        if let Some((_, rise)) = body.lift_since.take() {
                            lift_count(rise, LiftOutcome::Released);
                        }
                    } else if body.lift_since.is_none() && stand_in_lift() {
                        let rise = lift_probe(grid, node, feet, to);
                        if body.step_m / TICK_S >= LIFT_MIN_SPEED_MS {
                            body.lift_since = Some((tick, rise));
                        } else if rise.is_some() {
                            lift_count(rise, LiftOutcome::SlowContact);
                        }
                    }
                    let mut lifted = false;
                    if let Some((since, rise)) = body.lift_since
                        && tick >= since + LIFT_HOLD_MS
                    {
                        match lift_check(grid, node, feet, to) {
                            Ok(up) => {
                                lift_count(rise, LiftOutcome::Made);
                                lifted = true;
                                node = up;
                                feet = grid.position(up);
                            }
                            Err(NoLift::NoLink) => lift_count(rise, LiftOutcome::NoLink),
                            Err(NoLift::TooHigh) => lift_count(rise, LiftOutcome::TooHigh),
                            Err(NoLift::TopShort) => lift_count(rise, LiftOutcome::TopShort),
                        }
                        body.lift_since = None;
                    }
                    // An enemy body in the way: the move that closes on it within the two radii is not made.
                    let blocked = self.body_block_m.is_some_and(|m| {
                        standing.iter().enumerate().any(|(other, &(at, team, alive))| {
                            other != place
                                && alive
                                && team != body.team
                                && (at.y - body.feet.y).abs() < 1.5
                                && feet.distance_xz(at) < m
                                && feet.distance_xz(at) < body.feet.distance_xz(at)
                        })
                    });
                    if !blocked {
                        let way = if lifted { standin_climb::Way::Lift } else { standin_climb::Way::Walk };
                        self.climbs.note(way, tick, place, body.team, body.feet, feet);
                        body.node = Some(node);
                        body.feet = feet;
                        if let Some(stood) = standing.get_mut(place) {
                            stood.0 = feet;
                        }
                    }
                }
                body.step_m = body.feet.distance_xz(before);
                body.step_vec = Vec3::new(body.feet.x - before.x, 0.0, body.feet.z - before.z);
                body.moved += body.step_m;
                // C14: the ground the client slides a body off, counted when the body stands still, and slid down with
                // `SRP_BENCH_STAND_IN_SLIDE=on` (the client's still test is on the body's speed: no move key and slower than STILL_MS).
                let moving = body.keys & (key::FORWARD | key::BACK | key::LEFT | key::RIGHT) != 0;
                let still = !moving && body.step_m / TICK_S < standin_climb::STILL_MS;
                if body.flight.is_none()
                    && tick >= body.air_until
                    && let (Some(node), Some(walls)) = (body.node, self.walls.as_ref())
                {
                    let walker = || self.world.map.walker_triangles();
                    let ground = self
                        .grounds
                        .get_or_insert_with(|| standin_slide::Grounds::new(&walker(), walls.clone()))
                        .at(node, body.feet);
                    if still {
                        self.climbs.still.note(ground);
                    }
                    body.slide_mps = match ground {
                        Some(ground) if standin_slide::stand_in_slide() && ground.slides(still || body.slide_mps.is_some()) => {
                            let speed = body.slide_mps.unwrap_or(0.0) + standin_slide::gain_mps(ground);
                            let step = standin_slide::slide_step(&self.world.grid, node, body.feet, speed);
                            self.slides.note(body.slide_mps.is_none(), body.feet, step.map(|(_, feet)| feet));
                            step.map(|(to, feet)| {
                                body.node = Some(to);
                                body.feet = feet;
                                speed
                            })
                        }
                        _ => None,
                    };
                }
                if body.carrying.is_some() {
                    self.carry_m += body.step_m;
                    self.carry_ticks += 1;
                }
            }
            let sprint_keys = Kinematics::sprints(body.keys);
            body.stamina
                .tick(tick, sprint_keys && body.carrying.is_none(), sprint_keys);
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
            // [counts only] Why the trigger was not down for the weapon this tick (`kick_gap`'s `REFUSALS`).
            let why = if !body.alive {
                1
            } else if body.keys & key::FIRE == 0 {
                2
            } else if body.carrying.is_some() {
                3
            } else if tick < body.drawn_at {
                4
            } else {
                body.stock.why_not(tick).unwrap_or(8)
            };
            let mut fired = 0u8;
            for at in times {
                if self.bodies[shooter].stock.fired(at) {
                    self.shot(shooter, at);
                    fired = fired.saturating_add(1);
                }
            }
            if let Some((_, counted)) = self.bodies[shooter].mirror
                && (fired > 0 || counted > 0)
            {
                self.kick_ticks.push(fight_tally::KickTick { shooter, tick, fired, counted, why });
            }
        }
    }

    /// The line of a shot: its origin and direction. **The one place a shot line is scattered.** The view's direction
    /// is turned by the weapon's cone (`SRP_BENCH_SPREAD`), as the game's `get_dispersed_bullet_direction` turns
    /// it; with the whole hit cut on ([`HitVolume::Boxes`] with a `scatter_m`) the origin is moved by one random
    /// offset across the line, the live position error the bench's bodies lack (C3, [`standin_hitbox`]:
    /// GUESS for its size). The origin is where the hit tests start their rays; the eye stays where the bullet
    /// starts for the world's blocking. With the old disc, nothing moves and nothing is drawn beyond the cone.
    pub(crate) fn shot_line(&mut self, eye: Vec3, dir: Vec3, cone: f32) -> (Vec3, Vec3) {
        let dir = if self.spread {
            bench_spread::deflect(dir, cone, &mut self.rng)
        } else {
            dir
        };
        let HitVolume::Boxes { scatter_m: Some(sigma) } = self.volume else {
            return (eye, dir);
        };
        // Two Gaussian offsets across the line (Box-Muller from two draws): along the horizontal across it
        // and along the line's own up.
        let u1 = self.rng.unit().max(1e-9);
        let u2 = self.rng.unit();
        let radius = sigma * (-2.0 * u1.ln()).sqrt();
        let angle = std::f32::consts::TAU * u2;
        let side = dir.cross(Vec3::Y);
        let side = if side.length() < 1e-4 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            side * (1.0 / side.length())
        };
        let up = side.cross(dir);
        (eye + side * (radius * angle.cos()) + up * (radius * angle.sin()), dir)
    }

    /// One shot of `shooter` at `at` ms (within the tick).
    pub(crate) fn shot(&mut self, shooter: Place, at: f32) {
        self.shots += 1;
        self.shots_hits[shooter].0 += 1;
        let body = &self.bodies[shooter];
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let pitch = body.view_elevation();
        let (c, s) = (pitch.cos(), pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let kick = body.recoil.kick();
        let top = kick.top(self.rng.unit());
        let (gun, keys) = (body.gun, body.keys);
        let truth = body.recoil.y();
        let mirror = body.mirror.map(|m| m.0);
        self.bodies[shooter].recoil.shot(at, top, 0.0);
        let (aim_state, cone) = self.bodies[shooter].spread.shot(gun, keys, at);
        let view_dir = dir;
        let (origin, dir) = self.shot_line(eye, dir, cone);
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
                let kept = self.cover_flight(shooter, eye, dir, reach).share;
                if kept > DamageShare::NONE {
                    self.walled_through_by[team] += 1;
                }
                self.pierce_stats.borrow_mut().walled_bands[team][PierceStats::band(kept)] += 1;
            }
            self.metric_rays += super::aimpoint::rays_cast() - before;
        }
        // What the shot's bullets met: the old disc's nearest chest, or the hit shapes of each pellet's ray.
        let ammo = standin_damage::ammo_of(&self.damage, gun);
        let (disc, boxes) = match self.volume {
            HitVolume::Disc => (self.strike_disc(shooter, eye, dir), Vec::new()),
            HitVolume::Boxes { .. } => (None, self.strikes_boxes(shooter, eye, origin, dir, ammo)),
        };
        let best: Option<(Place, f32)> = disc.or_else(|| {
            boxes
                .iter()
                .min_by(|a, b| a.along.total_cmp(&b.along))
                .map(|s| (s.body, s.along))
        });
        // The bullet meets a bear trap's cylinder before any body: it is disarmed.
        self.trap_shot(shooter, eye, dir, best.map(|b| b.1), tick);
        let shooter_body = &self.bodies[shooter];
        let team = shooter_body.team;
        let (moving, aimed, crouched) = fight_tally::stance(shooter_body.keys);
        let aimed_at = shooter_body
            .target
            .filter(|&t| self.bodies[t].alive)
            .map(|t| {
                let (d, off) = fight_tally::aim_off(origin, dir, Self::chest(&self.bodies[t]));
                (t, d, off)
            });
        // [counts only] Where the shot line passed the target's chest, for the owner counts' `shot_offsets`.
        let offset = aimed_at.map(|(t, distance, _)| {
            let target = &self.bodies[t];
            super::tallies::shot_offset::ShotOffset::of(
                origin,
                dir,
                Self::chest(target),
                distance,
                super::tallies::shot_offset::ShotClass::of(gun.class()),
                target.keys & key::CROUCH != 0,
            )
        });
        // [counts only] The bullets or pellets fired, those that landed on an enemy and the parts the shapes gave.
        let mut landed = fight_tally::Landed {
            fired: ammo.buck_shot.clamp(1, 255) as u8,
            ..Default::default()
        };
        if let Some((body, along)) = disc
            && self.bodies[body].team != team
        {
            landed.on_enemy = standin_damage::pellets_hitting(ammo.buck_shot, along).min(255) as u8;
        }
        for strike in boxes.iter().filter(|s| self.bodies[s.body].team != team) {
            landed.on_enemy += 1;
            landed.parts[strike.param.part().index()] += 1;
        }
        // The enemy the shot hit: the shooter's target if a bullet landed on it, else the nearest enemy hit.
        let hit = {
            let enemies = || {
                disc.into_iter()
                    .chain(boxes.iter().map(|s| (s.body, s.along)))
                    .filter(|&(body, _)| self.bodies[body].team != team)
            };
            aimed_at
                .map(|(t, ..)| t)
                .filter(|&t| enemies().any(|(body, _)| body == t))
                .or_else(|| {
                    enemies()
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(body, _)| body)
                })
        };
        // [counts only] A shooter's first shot of the tick: its kick against the mirror's (a second shot of the tick has the first's).
        if let Some(mirror) = mirror
            && !self.shot_facts.iter().rev().take_while(|f| f.tick == tick).any(|f| f.shooter == shooter)
        {
            self.kick_pairs.push(fight_tally::KickPair {
                shooter,
                tick,
                truth,
                mirror,
                distance: aimed_at.map(|(_, distance, _)| distance),
            });
        }
        self.shot_facts.push(fight_tally::ShotFact {
            shooter,
            tick,
            hit,
            target: aimed_at,
            moving,
            aimed,
            crouched,
            aim_state,
            cone,
            offset,
            rise: aimed_at.map(|(t, ..)| fight_tally::Rise::of(eye, view_dir, Self::chest(&self.bodies[t]))),
            landed,
        });
        match self.volume {
            HitVolume::Disc => {
                if let Some((target, along)) = best {
                    self.land_disc(shooter, target, along, [eye, dir, view_dir], cone, tick);
                }
            }
            HitVolume::Boxes { .. } => self.land_boxes(shooter, &boxes, [eye, dir, view_dir], cone, tick),
        }
    }

    /// The old rule ([`HitVolume::Disc`]): the nearest body, within 100 m along the line, whose chest the line
    /// passes within [`standin_hitbox::DISC_M`] and sees (a clear line from the eye to the chest, not a clear shot ray).
    fn strike_disc(&self, shooter: Place, eye: Vec3, dir: Vec3) -> Option<(Place, f32)> {
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
            if miss < standin_hitbox::DISC_M
                && best.is_none_or(|(_, d)| along < d)
                && self.world.sight.visible(eye, Self::chest(other))
            {
                best = Some((target, along));
            }
        }
        best
    }

    /// The nearest body hit shape on the ray from `origin` along `dir`, within the stand-in's 100 m, if the
    /// bullet mesh (the world) does not meet the ray from `eye` first (`origin` is `eye` moved across the line
    /// by the position scatter, [`Game::shot_line`]). With [`Game::flight`] the bullet flies instead
    /// ([`Self::nearest_shape_flown`]).
    fn nearest_shape(&self, shooter: Place, eye: Vec3, origin: Vec3, dir: Vec3) -> Option<BoxStrike> {
        if self.flight != standin_hitbox::BulletPath::Instant {
            return self.nearest_shape_flown(shooter, eye, origin, dir);
        }
        let mut best: Option<BoxStrike> = None;
        for (body, other) in self.bodies.iter().enumerate() {
            if !other.alive || body == shooter {
                continue;
            }
            let reach = best.map_or(super::tallies::shot_offset::REACH_M, |b| b.along);
            let hit = standin_hitbox::ray_at_body(
                other.feet,
                forward(other.yaw),
                other.keys & key::CROUCH != 0,
                other.keys & key::AIM != 0,
                origin,
                dir,
                reach,
            );
            if let Some((along, param)) = hit
                && best.is_none_or(|b| along < b.along)
            {
                best = Some(BoxStrike { body, along, param, load: Load { damage: 0.0, pierce: 0.0 }, share: DamageShare::ALL });
            }
        }
        // The bullet goes through the surfaces on the way that its pierce gets through, with the share they leave.
        best.and_then(|b| {
            let line = self.cover_flight(shooter, eye, dir, b.along);
            self.pierce_stats.borrow_mut().strike(&line);
            (line.share > DamageShare::NONE).then_some(BoxStrike { load: line.load, share: line.share, ..b })
        })
    }

    /// [`Self::nearest_shape`] for a bullet in flight (`SRP_BENCH_HIT_FLIGHT`): the client's path
    /// ([`standin_hitbox::bullet_at`]: the muzzle speed, the drag, gravity) in 10 ms steps, each chord tested against the
    /// bodies where their last step carries them at the step's time (the bodies move on at that velocity; the client moves
    /// them tick by tick), the first chord that meets a shape wins; then the world's blocking from the eye to that point
    /// ([`Self::cover_flight`]). The shot's direction is not corrected for the drop: nothing in the bots reads it.
    fn nearest_shape_flown(&self, shooter: Place, eye: Vec3, origin: Vec3, dir: Vec3) -> Option<BoxStrike> {
        const STEP_S: f32 = 0.01;
        let ammo = standin_damage::ammo_of(&self.damage, self.bodies[shooter].gun);
        let end = standin_hitbox::flight_end_s(ammo);
        let (mut t, mut travelled, mut from) = (0.0_f32, 0.0_f32, origin);
        while t < end && travelled < super::tallies::shot_offset::REACH_M {
            let next = (t + STEP_S).min(end);
            let to = standin_hitbox::bullet_at(origin, dir, ammo, next);
            let chord = to - from;
            let length = chord.length();
            if length > 1e-4 {
                let direction = chord * (1.0 / length);
                let mid = (t + next) / 2.0;
                let mut hit: Option<(Place, f32, standin_hitbox::HitParam)> = None;
                for (body, other) in self.bodies.iter().enumerate() {
                    if !other.alive || body == shooter {
                        continue;
                    }
                    let feet = match self.flight {
                        standin_hitbox::BulletPath::FlownBodiesStill => other.feet,
                        _ => other.feet + other.step_vec * (mid / TICK_S),
                    };
                    let reach = hit.map_or(length, |h| h.1);
                    if let Some((along, param)) = standin_hitbox::ray_at_body(
                        feet,
                        forward(other.yaw),
                        other.keys & key::CROUCH != 0,
                        other.keys & key::AIM != 0,
                        from,
                        direction,
                        reach,
                    ) && hit.is_none_or(|h| along < h.1)
                    {
                        hit = Some((body, along, param));
                    }
                }
                if let Some((body, along, param)) = hit {
                    let to_point = from + direction * along - eye;
                    let distance = to_point.length().max(1e-3);
                    let line = self.cover_flight(shooter, eye, to_point * (1.0 / distance), distance);
                    self.pierce_stats.borrow_mut().strike(&line);
                    return (line.share > DamageShare::NONE).then_some(BoxStrike {
                        body,
                        along: travelled + along,
                        param,
                        load: line.load,
                        share: line.share,
                    });
                }
                travelled += length;
            }
            from = to;
            t = next;
        }
        None
    }

    /// `shooter`'s bullet `along` m down the line from `eye`: the client's penetration over the bullet mesh's materials,
    /// the bullet flown from the muzzle ([`mapdata::RayTree::fly`]: the pierce and the damage it has at each wall, what
    /// each crossing takes off). Without [`Self::pierce`], or for a gun the data does not know, the old rule: the
    /// first surface stops it (`ray_cast`, faces within a centimetre of the end included).
    fn cover_flight(&self, shooter: Place, eye: Vec3, dir: Vec3, along: f32) -> mapdata::Followed {
        let sight = &self.world.sight;
        let gun = self.bodies[shooter].gun;
        let flight = standin_damage::flight_of(&self.damage, gun);
        if let Some(flight) = flight.as_ref().filter(|_| self.pierce) {
            return sight.fly(eye, eye + dir * along, flight, 0.0, 0.0);
        }
        let share = if sight.ray_cast(eye, dir, along).is_none() { DamageShare::ALL } else { DamageShare::NONE };
        let load = match &flight {
            Some(flight) if share > DamageShare::NONE => flight(along),
            _ => Load { damage: 0.0, pierce: 0.0 },
        };
        mapdata::Followed {
            share,
            load,
            stopped_by: (share == DamageShare::NONE).then_some(FaceArmor::Unknown),
            crossed: 0,
        }
    }

    /// [`HitVolume::Boxes`]: a bullet is one ray along `dir`; a shotgun's shot is its `buck_shot` pellets, each
    /// its own ray, the shot's direction turned again by the ammunition's cone (`buck_dispersion`) as the game's
    /// `get_dispersed_buckshot_direction` turns it (`fact-hitboxes-026g.md` 4). One strike per ray that met a
    /// body.
    fn strikes_boxes(
        &mut self,
        shooter: Place,
        eye: Vec3,
        origin: Vec3,
        dir: Vec3,
        ammo: &survarium::v026g::damage::AmmoParams,
    ) -> Vec<BoxStrike> {
        if ammo.buck_shot <= 1 {
            return self.nearest_shape(shooter, eye, origin, dir).into_iter().collect();
        }
        (0..ammo.buck_shot)
            .filter_map(|_| {
                let pellet = bench_spread::deflect(dir, ammo.buck_dispersion, &mut self.rng);
                self.nearest_shape(shooter, eye, origin, pellet)
            })
            .collect()
    }

    /// The old rule's hit: `shooter`'s shot hit `target` at `along` m; its part drawn by chance, a leg hit by
    /// a hash. `line` is the eye, the shot's direction and the view's.
    fn land_disc(&mut self, shooter: Place, target: Place, along: f32, line: [Vec3; 3], cone: f32, tick: u32) {
        let [eye, dir, view_dir] = line;
        let gun = self.bodies[shooter].gun;
        let team_hit = self.bodies[target].team == self.bodies[shooter].team;
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
        let stamina = standin_damage::hit(&self.damage, other, gun, along, &mut self.rng, tick);
        other.stamina.hit(stamina);
        other.last_hit = tick;
        other.stagger.hit(tick, standin_stagger::power_of(&self.damage, gun));
        if let Some(leg) = stand_in_leg_hit(tick, shooter, target) {
            other.legs[leg] = (other.legs[leg] - STAND_IN_LEG_HIT).max(0.0);
            if other.legs[leg] <= 0.0 && other.leg_broken_until == 0 {
                other.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
            }
        }
        self.settle(shooter, target, team_hit, pain_before, tick);
    }

    /// [`HitVolume::Boxes`]' hits: every body a bullet or pellet landed on takes them together, each with the
    /// hit box its shape is; a leg shape hurts that leg (once for the shot). The shot counts as one hit on an
    /// enemy if any landed on one. `line` is the eye, the shot's direction and the view's.
    fn land_boxes(&mut self, shooter: Place, strikes: &[BoxStrike], line: [Vec3; 3], cone: f32, tick: u32) {
        let [eye, dir, view_dir] = line;
        let (gun, team) = (self.bodies[shooter].gun, self.bodies[shooter].team);
        let mut bodies: Vec<Place> = Vec::new();
        let mut enemy_hit = false;
        for strike in strikes {
            if bodies.contains(&strike.body) {
                continue;
            }
            let target = strike.body;
            bodies.push(target);
            let landed: Vec<(standin_hitbox::HitParam, Load)> =
                strikes.iter().filter(|s| s.body == target).map(|s| (s.param, s.load)).collect();
            let through_cover = strikes.iter().any(|s| s.body == target && s.share < DamageShare::ALL);
            let team_hit = self.bodies[target].team == team;
            if team_hit {
                self.team_hits += 1;
                self.team_hit_shots.push((shooter, target, eye, dir, cone, view_dir));
            } else {
                enemy_hit = true;
                self.hit_log.push((tick, shooter, target));
                if through_cover {
                    self.cover_hits_by[usize::from(team).min(1)] += 1;
                }
            }
            let pain_before = self.bodies[target].pools.pain;
            let other = &mut self.bodies[target];
            standin_damage::hit_boxes(&self.damage, other, gun, &landed, tick);
            other.last_hit = tick;
            // Every pellet that landed is a bullet of its own.
            other.stagger.hit(tick, standin_stagger::power_of(&self.damage, gun) * landed.len() as f32);
            for leg in [0, 1] {
                if landed.iter().any(|(param, ..)| param.leg() == Some(leg)) {
                    other.legs[leg] = (other.legs[leg] - STAND_IN_LEG_HIT).max(0.0);
                    if other.legs[leg] <= 0.0 && other.leg_broken_until == 0 {
                        other.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
                    }
                }
            }
            let was_alive = self.bodies[target].alive;
            self.settle(shooter, target, team_hit, pain_before, tick);
            if through_cover && !team_hit && was_alive && !self.bodies[target].alive {
                self.cover_kills_by[usize::from(team).min(1)] += 1;
            }
        }
        if enemy_hit {
            self.hits += 1;
            self.shots_hits[shooter].1 += 1;
        }
    }

    /// After a hit on `target`: a team hit's pain is noted, and a lethal one is a kill.
    fn settle(&mut self, shooter: Place, target: Place, team_hit: bool, pain_before: f32, tick: u32) {
        let lethal = self.bodies[target].health <= 1e-3;
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
        let killer_feet = self.bodies.get(killer).map(|b| b.feet);
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
                let (c, k) = (other.feet, killer_feet.unwrap_or(other.feet));
                self.carry_kill_lines.push(format!(
                    "CARRYKILL t={tick} carrier={target} team={} at=({:.1},{:.1},{:.1}) killer={killer} from=({:.1},{:.1},{:.1}) dist={:.1} carried_ms={}",
                    other.team,
                    c.x,
                    c.y,
                    c.z,
                    k.x,
                    k.y,
                    k.z,
                    c.distance_xz(k),
                    self.took[target].map_or("-".to_owned(), |at| tick.saturating_sub(at).to_string())
                ));
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
            Kill::Mate => {
                self.team_kills += 1;
                self.team_kill_log.push(TeamKill {
                    tick,
                    killer,
                    target,
                    frag: self.kill_by_frag,
                    killer_feet: self.bodies[killer].feet,
                    target_feet: self.bodies[target].feet,
                });
            }
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
        if self.zone_routed {
            return self.zone_tick_routed(inside, on, baked);
        }
        let tick = self.tick;
        for place in 0..self.bodies.len() {
            let body = &mut self.bodies[place];
            if !body.alive {
                if let Some(spell) = body.zone_spell.take() {
                    self.zone_spells.push(spell.ended(body));
                    self.zone_notes.push(ZoneNote::of(place, &spell, body, None));
                }
                continue;
            }
            let Some(node) = self.world.grid.locate(body.feet, 1.0) else {
                continue;
            };
            let centre = self.body_centre(place);
            let now = self.zone_now(node, inside, on, tick);
            let was_off = std::mem::replace(&mut self.bodies[place].zone_prev_off, now.in_shape && !now.any_active);
            let drop_m = {
                let body = &mut self.bodies[place];
                let slot = (tick / 50 % 20) as usize;
                let hi = body.zone_ys.iter().copied().fold(body.feet.y, f32::max);
                body.zone_ys[slot] = body.feet.y;
                hi - body.feet.y
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
                    .map(|zone| self.zone_hit(zone, centre))
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
                    self.zone_notes.push(ZoneNote::of(place, &spell, body, None));
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
                restored: false,
                unmarked: false,
                came_on: was_off,
                fixed: now.fixed,
                at: body.feet,
                drop_m,
                zone: now.zone,
            });
            spell.restored |= now.restored;
            spell.unmarked |= now.unmarked;
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
            self.zone_notes.push(ZoneNote::of(place, &spell, body, Some(0)));
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

    /// The centre of the body at `place`: half the grid body's height over its feet (the client asks the zone about the
    /// capsule's centre: the live bots' losses fit the curve at it, `fx-zones-bots-hurt.md`).
    fn body_centre(&self, place: usize) -> Vec3 {
        self.bodies[place].feet + Vec3::new(0.0, 0.5 * self.world.grid.body().stand_height, 0.0)
    }

    /// What the floor `node` is to the damage zones at `tick` ([`ZoneNow`]): `inside` holds the zones' own shapes, `on` the bots' marks and timing.
    fn zone_now(&self, node: NodeId, inside: &Hazards, on: &Hazards, tick: u32) -> ZoneNow {
        let active = |zone: &Hazard| on.zones.iter().find(|o| o.zone == zone.zone).is_some_and(|o| o.activity.active(tick));
        let (mut in_shape, mut any_active, mut fixed, mut unmarked, mut first) = (false, false, true, false, None);
        for zone in inside.zones_at(node) {
            in_shape = true;
            if active(zone) {
                any_active = true;
                first.get_or_insert(zone.zone);
                fixed &= !zone.switches();
                unmarked |= !on.zones_at(node).any(|m| m.zone == zone.zone);
            }
        }
        ZoneNow { restored: !self.world.grid.can_stand(node), unmarked, fixed: any_active && fixed, in_shape, any_active, zone: first }
    }

    /// What `zone` takes a second from a body whose centre is at `centre`, before armour: the client's rule under
    /// [`Self::zone_intensity`], else the full hit.
    fn zone_hit(&self, zone: &Hazard, centre: Vec3) -> f32 {
        if self.zone_intensity { zone.hit_rate_at(centre) } else { zone.hit_per_s() }
    }

    /// [`Self::zone_hit`] in the pools that kill under `armour`.
    fn zone_cost(&self, zone: &Hazard, armour: &crate::v026g::bots::zone_cost::ZoneArmour, centre: Vec3) -> crate::v026g::bots::zone_cost::Cost {
        if self.zone_intensity { zone.client_cost_per_s(armour, centre) } else { zone.cost_per_s(armour) }
    }

    /// [`Self::zone_tick`] with the zones' hits going where the client sends them (`zone_cost.rs`): each zone's
    /// hit a second, worked out for the armour the body wears (its kit's), goes to the body, the head, the
    /// infection (no regeneration) and the pain pools; the anomaly fields still hurt the body pool by their raw
    /// hit. A body dies where it stands when any of those pools runs out (the client kills at 0 in each).
    fn zone_tick_routed(&mut self, inside: &Hazards, on: &Hazards, baked: bool) {
        use crate::v026g::bots::zone_cost::Cost;
        let tick = self.tick;
        for place in 0..self.bodies.len() {
            let body = &mut self.bodies[place];
            if !body.alive {
                if let Some(spell) = body.zone_spell.take() {
                    self.zone_spells.push(spell.ended(body));
                    self.zone_notes.push(ZoneNote::of(place, &spell, body, None));
                }
                continue;
            }
            let Some(node) = self.world.grid.locate(body.feet, 1.0) else {
                continue;
            };
            let armour = super::standin_damage::zone_armour_of(self.damage.tier, self.damage.rusher, self.damage.mask, body.gun.item);
            let centre = self.body_centre(place);
            let now = self.zone_now(node, inside, on, tick);
            let was_off = std::mem::replace(&mut self.bodies[place].zone_prev_off, now.in_shape && !now.any_active);
            let drop_m = {
                let body = &mut self.bodies[place];
                let slot = (tick / 50 % 20) as usize;
                let hi = body.zone_ys.iter().copied().fold(body.feet.y, f32::max);
                body.zone_ys[slot] = body.feet.y;
                hi - body.feet.y
            };
            let baked_cost: Cost = if baked {
                inside
                    .zones_at(node)
                    .filter(|zone| {
                        on.zones
                            .iter()
                            .find(|o| o.zone == zone.zone)
                            .is_some_and(|o| o.activity.active(tick))
                    })
                    .map(|zone| self.zone_cost(zone, &armour, centre))
                    .sum()
            } else {
                Cost::ZERO
            };
            let per_live = self.fields.hit_per_s_at(node);
            let body = &mut self.bodies[place];
            // The antitoxin halves a toxic hit: the infection pool's.
            let factor = body.heal.toxic_factor(tick);
            let cost = Cost {
                body: baked_cost.body + per_live * factor,
                infection: baked_cost.infection * factor,
                ..baked_cost
            };
            if cost.most() <= 0.0 {
                if let Some(spell) = body.zone_spell
                    && tick >= spell.last + SPELL_GAP_MS
                {
                    self.zone_spells.push(spell.ended(body));
                    self.zone_notes.push(ZoneNote::of(place, &spell, body, None));
                    body.zone_spell = None;
                }
                continue;
            }
            let per_s = cost.body + cost.head + cost.infection + cost.pain;
            let take = |pool: &mut f32, per_s: f32| {
                let hit = (per_s * TICK_S).min(pool.max(0.0));
                *pool -= hit;
                hit
            };
            let lost = [
                take(&mut body.health, cost.body),
                take(&mut body.pools.head, cost.head),
                take(&mut body.rest.infection, cost.infection),
                take(&mut body.pools.pain, cost.pain),
            ];
            let hit: f32 = lost.iter().sum();
            for (total, each) in self.zone_pool_lost.iter_mut().zip(lost) {
                *total += each;
            }
            if per_live > 0.0 {
                self.fields.stats.hp += lost[0].min(per_live * factor * TICK_S);
            }
            let spell = body.zone_spell.get_or_insert(ZoneSpell {
                start: tick,
                health: body.health,
                lost: 0.0,
                last: tick,
                why: 0,
                restored: false,
                unmarked: false,
                came_on: was_off,
                fixed: now.fixed,
                at: body.feet,
                drop_m,
                zone: now.zone,
            });
            spell.restored |= now.restored;
            spell.unmarked |= now.unmarked;
            spell.why |= body.why_now;
            spell.lost += hit;
            spell.last = tick;
            self.zone_hp += hit;
            let empty = [
                body.health <= 1e-3,
                body.pools.head <= 1e-3,
                body.rest.infection <= 1e-3,
                body.pools.pain <= 1e-3,
            ];
            let Some(which) = empty.iter().position(|&e| e) else {
                continue;
            };
            let spell = body.zone_spell.take().expect("just set");
            self.zone_spells.push(spell.ended(body));
            self.zone_notes.push(ZoneNote::of(place, &spell, body, Some(which as u8)));
            eprintln!(
                "  zone death ({}): at {:.1} s, in zones {:.1} s at {:.2} a second, health {:.2} then, the zones took {:.2}",
                ["body", "head", "infection", "pain"][which],
                f64::from(tick) / 1000.0,
                f64::from(tick - spell.start) / 1000.0,
                per_s,
                spell.health,
                spell.lost,
            );
            body.alive = false;
            body.dead_at = tick;
            if let Some(battery) = body.carrying.take() {
                self.batteries[battery] = Lying::Free(body.feet, 1 << body.team);
            }
            let team = usize::from(body.team);
            self.zone_deaths += 1;
            self.zone_pool_deaths[which] += 1;
            self.fields.stats.deaths += u32::from(per_live > 0.0 && baked_cost.most() <= 0.0);
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
        Self::uses_reach(body, point, radius, super::view::use_ray_m())
    }

    /// [`Self::uses`] with a ray of `reach` m.
    fn uses_reach(body: &Body, point: Vec3, radius: f32, reach: f32) -> bool {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
        let dir = Vec3::new(fx * c, s, fz * c);
        let to = point - eye;
        let along = to.x * dir.x + to.y * dir.y + to.z * dir.z;
        if !(0.0..=reach).contains(&along) {
            return false;
        }
        (to - dir * along).length() <= radius
    }

    /// How far along `body`'s view ray from the eye `point` lies (m).
    pub(crate) fn ray_along(body: &Body, point: Vec3) -> f32 {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
        (point - eye).dot(Vec3::new(fx * c, s, fz * c))
    }

    /// How far `body`'s use ray (from the eye, [`super::view::use_ray_m`] long)
    /// passes `point`.
    pub(crate) fn ray_miss(body: &Body, point: Vec3) -> f32 {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
        let dir = Vec3::new(fx * c, s, fz * c);
        let along = (point - eye).dot(dir).clamp(0.0, super::view::use_ray_m());
        (eye + dir * along - point).length()
    }

    /// How far along the view's ray from `body`'s eye container `id`'s use
    /// box is entered, if it is.
    pub(crate) fn container_box_reach(&self, body: &Body, id: usize) -> Option<f32> {
        let shape = self
            .world
            .map
            .containers
            .iter()
            .find(|c| c.id == id as i32)
            .and_then(|c| c.use_shapes.first())?;
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
        mapdata::stands::ray_distance(eye, Vec3::new(fx * c, s, fz * c), shape)
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
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
        let dir = Vec3::new(fx * c, s, fz * c);
        mapdata::stands::ray_meets(&self.world.sight, eye, dir, shape, super::view::use_ray_m())
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
        let (c, s) = { let e = body.base_elevation(); (e.cos(), e.sin()) };
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
                self.bodies[place].drawn_at = self.tick + draw_ms(body.gun);
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
                            reach: self.container_box_reach(&body, id),
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
                            self.take_alongs.push(Self::ray_along(&body, at + BATTERY_MIDDLE));
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
        self.trap_record();
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

/// The delay of a death is by the kit the body died in: the mode's time less the radio's 20 % with the radio, the whole time without
/// it; every kit the bots carry today has the radio, so a match that sets nothing waits as it did.
#[test]
fn a_body_without_the_radio_waits_the_whole_respawn_time() {
    let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
    assert_eq!(game.respawn_ms_of_body(0), RESPAWN_BATTERIES_MS, "radio in every kit, as the bodies start");
    game.bodies[0].radio = [false; 3];
    assert_eq!(game.respawn_ms_of_body(0), 10_000, "no radio: the batteries modes' 10 s");
    assert_eq!(game.respawn_ms_of_body(1), RESPAWN_BATTERIES_MS, "the other body keeps its radio");
    let mut tdm = Game::new(soak_world(), &[0, 1], 1, Some(60), None);
    assert_eq!(tdm.respawn_ms_of_body(0), RESPAWN_TDM_MS);
    tdm.bodies[0].radio = [false; 3];
    assert_eq!(tdm.respawn_ms_of_body(0), 8_000);
    // The kit in use decides: radio in kit 0 only.
    game.bodies[0].radio = [true, false, false];
    game.bodies[0].heal.kit_next = 1;
    game.bodies[0].heal.spawn();
    assert_eq!(game.respawn_ms_of_body(0), 10_000, "died in kit 1, which has none");
}

/// [A239b] A place with its own start point comes back on it after the respawn and its walk, not at the respawn; another place
/// waits the respawn alone.
#[test]
fn a_placed_start_waits_its_walk_after_the_respawn() {
    let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
    let at = game.bodies[1].feet;
    game.placed_start = Some(PlacedStart { place: 0, at, walk_ms: 20_000 });
    assert_eq!((game.placed_walk_ms(0), game.placed_walk_ms(1)), (20_000, 0));
    let respawn = game.respawn_ms_of_body(0);
    for body in &mut game.bodies {
        body.alive = false;
        body.dead_at = game.tick;
    }
    let died = game.tick;
    while game.tick < died + respawn {
        game.step();
    }
    assert!(!game.bodies[0].alive, "not at the respawn");
    assert!(game.bodies[1].alive, "the other place at its respawn");
    while game.tick < died + respawn + 20_000 {
        assert!(!game.bodies[0].alive, "dead through his walk ({} ms after the respawn)", game.tick - died - respawn);
        game.step();
    }
    assert!(game.bodies[0].alive, "back after the walk");
    assert!(game.bodies[0].feet.distance_xz(at) < 3.0, "on his point");
}

/// The bots' profiles all carry the radio in every kit (today's decisions: set_radio_kits changes nothing for them).
#[test]
fn the_bots_profiles_carry_the_radio_in_every_kit() {
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, bot_profile};
    let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
    game.bodies[0].radio = [false; 3];
    let equipment = BotEquipment::of(BotTier::Mid, RusherKits::default());
    let profiles: Vec<_> = [BotRole::Support, BotRole::Rusher].iter().enumerate().map(|(n, &role)| bot_profile(n + 1, n as u8, role, equipment)).collect();
    game.set_radio_kits(&profiles);
    assert_eq!((game.bodies[0].radio, game.bodies[1].radio), ([true; 3], [true; 3]));
}
