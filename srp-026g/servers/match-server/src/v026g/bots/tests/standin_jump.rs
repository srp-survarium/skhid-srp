use super::*;

/// How long a jump keeps a stand-in body in the air, ms: walking into a
/// floor its floor jumps to ([`jumped_to`]) meanwhile, it gets up on it.
/// The client's jump is in the air 12 controller steps, 600 ms (the
/// take-off at frame 4.5-6.0 of the 24-frame 0.8 s clip, the air ended at
/// the clip's end by `end_jump` 0x7F050; 116 of 116 traced jumps): PROVEN,
/// rig `notes-jump-state.md` 0, 8. A bot jumps up its route's ledge 0.8 m
/// short of it (`motor.rs` `jump_key`), 0.25 s at a run. The stand-in had no jumps: a route up a jump link
/// stood at its foot, and 100 of the 118 held-keys stuck notes of a bench
/// were within a metre of a jump floor (`player_bots_stuck.md` §8.6).
pub(crate) const STAND_IN_AIR_MS: u32 = 600;

/// A jump of the stand-in: how high the feet get over the floor jumped
/// from, and how far ahead a jump with no grid link lands, m.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StandInJump {
    pub(crate) rise_m: f32,
    pub(crate) ahead_m: f32,
}

/// The client's jump standing or walking: the `on_site*` clips' root tops
/// at 0.998 m (PROVEN, the jump study). The stand-in lands it 0.6 m ahead.
/// Not the client's: from a standstill with no key it lands in place
/// (`on_site` has no travel); with forward pressed it plays `on_site_fwd`
/// scaled by s = min(max(1, v) / 4, 0.99), 0.50 m from take-off to landing
/// (0.604 m from the press) with the key let go, 1.10 m (1.204 m) with it
/// held (air control 1.0 m/s); other keys and S per key in exactness
/// `routes-readings` 1.5 (PROVEN inputs 0x337280, 0x2D7990, 0x2D6BDA;
/// DERIVED legs). Kept as the stand-in's model until changed (register
/// G47).
pub(crate) const STANDING_JUMP: StandInJump = StandInJump {
    rise_m: mapdata::grid::STANDING_JUMP_M,
    ahead_m: 0.6,
};

/// [`STANDING_JUMP`] as the stand-in game jumps it: `SRP_BENCH_JUMP_AHEAD_M`
/// sets how far ahead a jump with no grid link lands (a sensitivity row;
/// the grid lists pocket exits up to 1.0 m across), else 0.6 m (not the
/// client's: see [`STANDING_JUMP`]; register G48).
pub(crate) fn standing_jump() -> StandInJump {
    static AHEAD: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    let ahead_m = *AHEAD.get_or_init(|| {
        bench_settings().number("SRP_BENCH_JUMP_AHEAD_M")
            .unwrap_or(STANDING_JUMP.ahead_m)
    });
    StandInJump {
        ahead_m,
        ..STANDING_JUMP
    }
}

/// The client's sprint jump: 1.282-1.350 m by clip (PROVEN, the jump
/// study), played at a press with forward and sprint held at walk speed or
/// more, any weapon, no battery carried (PROVEN,
/// `player_logic_jump_state::initialize` 0x337040). No bot's jump picks it
/// yet. The distance is [`STANDING_JUMP`]'s, 0.6 m. Not the client's: from
/// the press to the take-off 0.875 F m (F the kit's walk factor), in the air
/// 3.257..3.286 m, the press to the landing 4.13..4.16 m at F = 1 (DERIVED
/// from the clips' roots and the take-off clip at time scale 1.0, 0x342E05;
/// exactness `routes-readings` 1.6). Kept as the stand-in's model until
/// changed (register G49).
pub(crate) const SPRINT_JUMP: StandInJump = StandInJump {
    rise_m: 1.30,
    ahead_m: 0.6,
};

/// Where a stand-in body in the air, walking from `feet` (on `node`) into
/// `to` and stopped, gets up with `jump`:
/// - by the grid's jump link ([`jumped_to`]) when its rise is within the
///   jump: a higher link is not made, as in the client;
/// - else, with no link, on the highest floor (then the nearest) within
///   `ahead_m` ahead on the walk's line and up to the jump's rise: a jump
///   the grid does not plan (out of a pocket, up a ledge a stuck bot hops
///   at).
pub(crate) fn stand_in_jump(
    grid: &mapdata::WalkGrid,
    node: NodeId,
    feet: Vec3,
    to: Vec3,
    jump: StandInJump,
) -> Option<NodeId> {
    let base = grid.position(node).y;
    if let Some(up) = jumped_to(grid, node, feet, to)
        && grid.position(up).y - base <= jump.rise_m
    {
        return Some(up);
    }
    let (dx, dz) = (to.x - feet.x, to.z - feet.z);
    let len = (dx * dx + dz * dz).sqrt();
    if len <= 1e-6 {
        return None;
    }
    let (ux, uz) = (dx / len, dz / len);
    let side = grid.cell_size() * 0.5 + 1e-4;
    let reach = jump.ahead_m;
    let area = [
        feet.x - reach,
        feet.z - reach,
        feet.x + reach,
        feet.z + reach,
    ];
    grid.floors_in(area)
        .filter(|&n| n != node && grid.can_stand(n))
        .filter_map(|n| {
            let at = grid.position(n);
            let (ex, ez) = (at.x - feet.x, at.z - feet.z);
            let ahead = ex * ux + ez * uz;
            let across = (ex * uz - ez * ux).abs();
            let rise = at.y - base;
            (ahead > 0.0 && ahead <= reach && across <= side && rise > 0.0 && rise <= jump.rise_m)
                .then_some((n, at.y, ahead))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
        .map(|(n, _, _)| n)
}

/// The client's sprint lift (A55, J0 8e rows, PROVEN): a body that runs into the face of a top at a sprint (it came to it at
/// [`LIFT_MIN_SPEED_MS`] or more, never from a stand) and is held there with sprint and forward is carried up it after the
/// client's own hold at the face, 0.15-0.3 s.
pub(crate) const LIFT_HOLD_MS: u32 = 300;
/// The speed a body comes to the face at for the lift, m/s: a sprint is 5 m/s; the walk cap is 3.5.
pub(crate) const LIFT_MIN_SPEED_MS: f32 = 3.5;

/// [T3] Whether the stand-in flies a jump as the client does (`SRP_BENCH_STAND_IN_JUMP`, `client` unless `old`): the clip of the
/// keys' direction from the stock clips (`jump_arc::Clip`, 24 frames, PROVEN data), the body moved each tick by the tick's root
/// delta turned by its yaw now (the root re-based on the body every tick on all axes, fx-review A325g: no catch-up), and the fall
/// after the clip from the root's last descent (`end_jump` 0x7F050 keeps the downward speed; fx-gear-2's read). `old`: the
/// forward clip's 13 samples along the press's way, chased from the press, and a fall from rest.
pub(crate) fn stand_in_client_jump() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_JUMP").map_or(true, |v| v != "old"))
}

/// [stand-in jump] Whether the stand-in flies a forward jump by the client's own flight (`SRP_BENCH_STAND_IN_JUMP`: `controller`, the
/// default; `client` the clip's root as before, `old` the clip chased): `body::FlownJump` from the body's kept controller, the very
/// flight the reckoning and the press forecast play (`mapdata::controller::jump::Flying`, `WalkGrid::kept_fly_step`; A388: the
/// port's flight is the exe's on 91 of 91 rig flights). A press with a side or back key keeps the clip, as the reckoning's does.
pub(crate) fn stand_in_controller_jump() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_JUMP").map_or(true, |v| v != "client" && v != "old"))
}

/// [stand-in jump] The client's own flight of a press ([`stand_in_controller_jump`]): the reckoning's press (`body::FlownJump::pressed`)
/// from the body's kept controller at `feet`, its keys `keys` (sprint taken off where the body may not sprint), its last move
/// `speed` m/s and its speed level `level`; none with the switch off and for a side or back press (counted, [`controller_jumps`]).
pub(crate) fn controller_flight(
    grid: &mapdata::WalkGrid,
    kept: Option<crate::v026g::bots::body::KeptBody>,
    feet: Vec3,
    keys: u32,
    speed: f32,
    level: f32,
    may_sprint: bool,
) -> Option<crate::v026g::bots::body::FlownJump> {
    if !stand_in_controller_jump() {
        return None;
    }
    let keys = if may_sprint { keys } else { keys & !key::SPRINT };
    let flight = crate::v026g::bots::body::FlownJump::pressed(grid, kept, feet, keys, speed, level, crate::v026g::bots::body::kept_clock::Of::StandIn);
    controller_jumps::count(if flight.is_some() { controller_jumps::Kind::Flown } else { controller_jumps::Kind::Clip });
    flight
}

/// [stand-in jump] The stand-in's jumps by their flight ([`stand_in_controller_jump`]): flown by the client's flight, kept on the clip
/// (a side or back press), and how the flights ended: on the ground, falling on. With `SRP_BENCH_STAND_IN_JUMP_LOG` set each flight's
/// end is printed (`stand-in jump: ...`).
pub(crate) mod controller_jumps {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    #[derive(Clone, Copy)]
    pub(crate) enum Kind {
        Flown,
        Clip,
        Landed,
        Fell,
    }

    static COUNTS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

    pub(crate) fn count(kind: Kind) {
        COUNTS[kind as usize].fetch_add(1, Relaxed);
    }

    /// (flown, kept on the clip, landed, fell on) so far.
    pub(crate) fn counts() -> [u64; 4] {
        std::array::from_fn(|i| COUNTS[i].load(Relaxed))
    }

    /// Whether each flight's end is printed (`SRP_BENCH_STAND_IN_JUMP_LOG`).
    pub(crate) fn log() -> bool {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ON.get_or_init(|| std::env::var_os("SRP_BENCH_STAND_IN_JUMP_LOG").is_some())
    }
}

/// [T3] The client's clip for a press ([`stand_in_client_jump`]): `jump_arc::Clip::of` by the keys and the body's speed (a body that
/// may not sprint takes the short jump), none with the old flight.
pub(crate) fn client_clip(keys: u32, speed: f32, may_sprint: bool) -> Option<crate::v026g::bots::jump_arc::Clip> {
    stand_in_client_jump().then(|| crate::v026g::bots::jump_arc::Clip::of(keys, if may_sprint { speed } else { 0.0 }))
}

/// Whether the stand-in lifts (`SRP_BENCH_STAND_IN_LIFT`, on unless `off`).
pub(crate) fn stand_in_lift() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_LIFT").map_or(true, |v| v != "off"))
}

/// [A313] How the stand-in body takes a walk-off bridge (`SRP_BENCH_STAND_IN_BRIDGES`, walked unless `off`): the client's capsule
/// crosses its one cell no floor covers in a stride, as the controller (the exe's walk tick, proven in Unicorn) walks every bridge the
/// grid keeps; the lattice stopped the body in that cell and held it at the face of each bridge up (the pass's 324-pair bench:
/// missed jumps 11 -> 29). A grid with no bridges (`grid.client_controller` off) moves as before.
pub(crate) fn stand_in_bridges() -> body::Bridges {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_BRIDGES").map_or(true, |v| v != "off")) {
        body::Bridges::Walked
    } else {
        body::Bridges::Leap
    }
}

/// [A318] Along what the stand-in body's step the grid stops goes on (`SRP_BENCH_STAND_IN_WALLS`, `round` unless `axes` or
/// `diagonal`): the client's capsule slides along walls, corners and thin posts too ([`crate::v026g::bots::kin::WallSlide`]).
pub(crate) fn stand_in_walls() -> crate::v026g::bots::kin::WallSlide {
    use crate::v026g::bots::kin::WallSlide;
    static WALLS: std::sync::OnceLock<WallSlide> = std::sync::OnceLock::new();
    *WALLS.get_or_init(|| match bench_settings().var("SRP_BENCH_STAND_IN_WALLS").as_deref() {
        Ok("axes") => WallSlide::Axes,
        Ok("diagonal") => WallSlide::Diagonal,
        _ => WallSlide::Round,
    })
}

/// [A318] How the stand-in body's blend goes on when its keys change again before it is over (`SRP_BENCH_STAND_IN_MIXER`,
/// `layers` unless `frozen`): the client's lexeme stack ([`kin::Mixer::Layers`], fitted on the client's speed at the worlds of 15
/// client-test recordings); `frozen` moves it as before.
pub(crate) fn stand_in_mixer() -> crate::v026g::bots::kin::Mixer {
    static FROZEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if OLD_MIXER.get() || *FROZEN.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_MIXER").is_ok_and(|v| v == "frozen")) {
        crate::v026g::bots::kin::Mixer::Frozen
    } else {
        crate::v026g::bots::kin::Mixer::Layers
    }
}

thread_local! {
    static OLD_MIXER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OLD_STEP_INPUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// [A318] The stand-in's step order: the bench tuning's ([`crate::v026g::bots::kin::StepInput`], `SRP_BENCH_TUNING`, so an
/// identity run's OFF arm is the old game on both sides), or the old same-tick one under [`OldStepInput`].
pub(crate) fn stand_in_step_input() -> crate::v026g::bots::kin::StepInput {
    static TUNED: std::sync::OnceLock<crate::v026g::bots::kin::StepInput> = std::sync::OnceLock::new();
    if OLD_STEP_INPUT.get() {
        crate::v026g::bots::kin::StepInput::Same
    } else {
        *TUNED.get_or_init(|| super::bench::bench_tuning().kin.step_input)
    }
}

/// [A364 P9] Whether every ground step of the stand-in's walk is the client's own controller kept across the body's steps
/// (`SRP_BENCH_STAND_IN_CLIENT_STEP`: `kept`, the default since #1681's bench; `tuning` with the bench tuning's
/// `kin.client_step = kept`): the bench's world walks as the client's body (the port is the game's feet on 99.5 % of 221,000 traced
/// steps), so a reckoning is judged against the client and not against the lattice. `on` and `off` are the older worlds.
/// [body physics S2] Whether the stand-in's bodies move by the shared model of the client's body (`SRP_BENCH_STAND_IN_BODY`, default
/// `client`; `lattice` the stand-in before S2). Needs the grid's client walker; a grid without it moves as before.
pub(crate) fn stand_in_client_body() -> bool {
    static TUNED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TUNED.get_or_init(|| super::bench_settings().var("SRP_BENCH_STAND_IN_BODY").as_deref() != Ok("lattice"))
}

/// [body physics S2] Whether a shared-model body's world carries its controller block (`SRP_BENCH_STAND_IN_WORLD_CONTROLLER`, default
/// on, as the client's worlds do).
pub(crate) fn stand_in_world_controller() -> bool {
    static TUNED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TUNED.get_or_init(|| super::bench_settings().var("SRP_BENCH_STAND_IN_WORLD_CONTROLLER").as_deref() != Ok("off"))
}

/// [body physics S2] The walk grid's client walker as the shared model's world: its walk tick and flight step (`WalkGrid::kept_tick`,
/// `WalkGrid::kept_fly_step`: the game's shapes, the grid body's controller settings, the client's with `exe_collision`).
pub(crate) struct GridWalker<'g>(pub(crate) &'g mapdata::WalkGrid);

impl client_body::Walker for GridWalker<'_> {
    fn step(&self, c: &mut mapdata::controller::tick::Controller, input: mapdata::controller::tick::TickInput) -> mapdata::controller::tick::StepNotes {
        self.0.kept_tick(c, input).expect("a client-body stand-in needs the grid's client walker") // [the shared model's world]
    }

    fn fly(
        &self,
        c: &mut mapdata::controller::tick::Controller,
        flying: &mut mapdata::controller::jump::Flying,
        heading: (f32, f32),
        air: (f32, f32),
    ) -> mapdata::controller::jump::FlightStep {
        self.0.kept_fly_step(c, flying, heading, air).expect("a client-body stand-in needs the grid's client walker") // [the shared model's world]
    }
}

/// [body physics S2] How the stand-in's shared-model bodies moved, for the bench's line: ticks stepped, controllers placed anew (a
/// spawn, or feet another rule moved), forward presses flown, side or back presses left to the clip, moves an enemy body held.
pub(crate) mod client_moves {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    #[derive(Clone, Copy)]
    pub(crate) enum Kind {
        Stepped,
        Placed,
        Flown,
        Clip,
        Blocked,
    }

    static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

    pub(crate) fn count(kind: Kind) {
        COUNTS[kind as usize].fetch_add(1, Relaxed);
    }

    /// (stepped, placed, flown, clip, blocked) so far.
    pub(crate) fn counts() -> [u64; 5] {
        std::array::from_fn(|i| COUNTS[i].load(Relaxed))
    }
}

pub(crate) fn stand_in_kept() -> bool {
    static TUNED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TUNED.get_or_init(|| match super::bench_settings().var("SRP_BENCH_STAND_IN_CLIENT_STEP").as_deref() {
        Ok("on" | "off") => false,
        Ok("tuning") => super::bench::bench_tuning().kin.client_step == crate::v026g::bots::kin::ClientStep::Kept,
        _ => true,
    })
}

/// [A318] Whether the stand-in's walk takes the client's walk tick where the lattice has a step rising or blocked
/// (`SRP_BENCH_STAND_IN_CLIENT_STEP=on`; the default is now [`stand_in_kept`]'s, which comes first); `off` the lattice alone;
/// `tuning` as the bench tuning's `kin.client_step` (the reckoning's rule).
pub(crate) fn stand_in_client_step() -> bool {
    static TUNED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *TUNED.get_or_init(|| match super::bench_settings().var("SRP_BENCH_STAND_IN_CLIENT_STEP").as_deref() {
        Ok("off") => false,
        Ok("tuning") => super::bench::bench_tuning().kin.client_step == crate::v026g::bots::kin::ClientStep::Client,
        _ => true,
    })
}

/// The stand-in's old step order ([`crate::v026g::bots::kin::StepInput::Same`]: a tick moves with its own input) for this thread
/// while it lives (tests whose pinned numbers predate it).
pub(crate) struct OldStepInput(());

impl OldStepInput {
    pub(crate) fn scope() -> Self {
        OLD_STEP_INPUT.set(true);
        Self(())
    }
}

impl Drop for OldStepInput {
    fn drop(&mut self) {
        OLD_STEP_INPUT.set(false);
    }
}

/// The stand-in's old blend ([`crate::v026g::bots::kin::Mixer::Frozen`]) for this thread while it lives (tests whose pinned
/// numbers predate A318).
pub(crate) struct OldMixer(());

impl OldMixer {
    pub(crate) fn scope() -> Self {
        OLD_MIXER.set(true);
        Self(())
    }
}

impl Drop for OldMixer {
    fn drop(&mut self) {
        OLD_MIXER.set(false);
    }
}

/// Whether the stand-in body is held at a ledge's edge as the client's capsule is (`SRP_BENCH_STAND_IN_EDGE`, on unless `off`): it falls
/// [`body::SUPPORT_M`] past the last floor's centre, not at the lattice's cell boundary.
pub(crate) fn stand_in_edge() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_EDGE").map_or(true, |v| v != "off"))
}

/// Whether the stand-in body past a ledge's edge goes on as the client's capsule does whatever the grid's links say
/// (`SRP_BENCH_STAND_IN_FALL`, on unless `off`; it acts only with [`stand_in_edge`]): [`body::slide_supported`] moves a body
/// past its floor's boundary only toward a drop link of that floor or along a lip's floor, so a body held 0.2-0.4 m past a
/// floor with neither stood there pushing, where the client's capsule walks on while its hemisphere still touches the edge and
/// falls once it does not (`step_down` 0x7E1B0: no contact in the sweep, airborne; PROVEN). The client's fall is not straight
/// down: the body stays in its run state (no jump state without a press), so it keeps its walk speed across while the walk keys
/// are held and stops moving across at once when they are let go; it lands about the walk speed times the air time past the
/// edge (a 2 m drop: 3.50 m out at a 5.0 m/s sprint, 0.65 s in the air; 2.60 m at a 3.5 m/s walk; PROVEN, exactness, the exact
/// tick's `walkoff.tsv`, `turning_and_jumps.md` "Walking off an edge"; register G50).
pub(crate) fn stand_in_fall() -> bool {
    if let Some(on) = FALL_FORCED.with(std::cell::Cell::get) {
        return on;
    }
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_FALL").map_or(true, |v| v != "off"))
}

thread_local! {
    /// A test's word on [`stand_in_fall`], for its own thread (the tests run one to a thread).
    static FALL_FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// A test puts [`stand_in_fall`] on or off for its own thread (`None`: as the run says): the pinned soaks predate it.
pub(crate) fn force_fall(on: Option<bool>) {
    FALL_FORCED.with(|f| f.set(on));
}

/// [A318] Whether a stand-in body falling (after a jump's clip, a walk off an edge) falls as the client's capsule does
/// (`SRP_BENCH_STAND_IN_SLOW_FALL`, on unless `off`; [`super::body::client_fall_tick`]: `step_down` 0x7E1B0's slow start,
/// 0.05 m a tick while the speed at the tick's start is under 2.0 m/s, then the speed at the tick's start, `g` 9.8 after the move,
/// PROVEN). Off: as before (the speed after the tick's `g` from the first tick; benches before A318's fall start).
pub(crate) fn stand_in_slow_fall() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_SLOW_FALL").map_or(true, |v| v != "off"))
}

/// Whether a stand-in body that walks off an edge falls from it keeping the walk's speed across (`SRP_BENCH_STAND_IN_WALKOFF`, on
/// unless `off`): the client's capsule leaves the edge at its walk speed and comes down along a ballistic arc, so a walk off a 1.2 m ledge
/// lands on a walkway 1.5 m ahead of it (J0 g2a at Chemical Plant's pedestrian bridge: 7 of 7 walk-offs on the walkway, PROVEN), where the
/// stand-in puts the body on the floor straight below the edge in one tick ([`past_edge`], or a walk along a Drop link). Off: as before
/// (the benches before it became the default, 2026-11-02, before A93, are comparable only with it off).
/// [A388 step 2] Whether a stand-in body's kept controller ([`stand_in_kept`]) that leaves the ground takes the body off its floor
/// there (`SRP_BENCH_STAND_IN_KEPT_EDGE`, on unless `off`): the client's capsule leaves a ledge where its `step_down` (0x7E1B0) finds
/// no ground within the step, which the lattice's hold to `body::SUPPORT_M` past a floor's centre passes by a median 0.26 m (falls
/// report R4, the exe's tick at its two largest spots).
pub(crate) fn stand_in_kept_edge() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_KEPT_EDGE").map_or(true, |v| v != "off"))
}

/// [stand-in fidelity] Whether a climb the stand-in body's kept controller makes on a ground step takes the body up
/// (`SRP_BENCH_STAND_IN_KEPT_CLIMB`, on unless `off`): the client's sprinting capsule climbs an edge to `jump::SPRINT_CLIMB_MAX_M`
/// (1.15 m) with no jump (`step_up` 0x7C900 and the plain capsule 0x7D33D), where the lattice's walk is held at the face over the
/// client step; the body goes on the floor at the controller's height ([`kept_climb`]). The same holds where the lattice walks the
/// body down a ledge its controller stays on the ground over (the controller ends over the lattice's walk). The suite's riser stucks
/// of 887697d4a: the game's body on the bot's own keys got onto the top at 12 of 15 faces the stand-in was held at (fix-move's exe
/// check, 2026-11-22); its falls where the game's body stays (fix-lip's exe check of S065, S285, S074).
pub(crate) fn stand_in_kept_climb() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_KEPT_CLIMB").map_or(true, |v| v != "off"))
}

/// [stand-in fidelity] Whether a stand-in body that leaves the ground falls by its kept controller (`SRP_BENCH_STAND_IN_KEPT_FALL`, on
/// unless `off`): a walk-off its kept controller left the ground at ([`stand_in_kept_edge`]), and a jump's flight that ends in the air
/// ([`stand_in_controller_jump`]'s "falls on"), go on as the client's walk tick with the tick's walk keys (the walk branch's fall,
/// `player_step` 0x7BDA0) until the controller is on the ground; off: the stand-in's own fall (a jump's straight down, a walk-off's
/// speed carried across). The suite's walk-off stucks and falls of 887697d4a where the game's body went elsewhere (fix-move's and
/// fix-lip's exe checks, 2026-11-22).
pub(crate) fn stand_in_kept_fall() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_KEPT_FALL").map_or(true, |v| v != "off"))
}

/// [stand-in fidelity] How much higher than the lattice's walk the kept controller must end a ground step for its climb to count
/// ([`kept_climb`]), m: over the lattice's own height noise on a slope.
pub(crate) const KEPT_CLIMB_OVER_M: f32 = 0.15;
/// [stand-in fidelity] How far from the controller's height the floor it climbed onto may be, m ([`kept_climb`]).
pub(crate) const KEPT_CLIMB_FLOOR_M: f32 = 0.2;
/// [stand-in fidelity] The longest fall by the kept controller ([`stand_in_kept_fall`]) before the stand-in's own fall takes over, ms.
pub(crate) const KEPT_FALL_MAX_MS: u32 = 8_000;

/// [stand-in fidelity] Where a ground step whose kept controller ended over the lattice's walk ([`stand_in_kept_climb`]: it climbed,
/// or it stayed up where the lattice walked down) puts the body: the grid's floor at the controller's end `to` (flat) and its own
/// height `top` (`KeptBody::height`), when that is over the lattice's walk `walked` by [`KEPT_CLIMB_OVER_M`] and a floor lies within
/// [`KEPT_CLIMB_FLOOR_M`] of it; none for a step the lattice follows, or a height with no floor of the grid (a climb's first step,
/// still rising: the controller is kept, the next step tells).
///
/// None either for a controller in the air after the step (`airborne`, [`body::KeptStep::airborne`]): it stands on nothing, so it
/// climbed nothing; it is falling, slower than the lattice walks down under it. The combined bench's B-tdm-m6-s2 bot 10 (level_06,
/// 2026-11-23, traced): running off a top down the walker's ground, its controller in the air from 5.85 s, each tick put it back on
/// the top's floor 0.4-0.5 m over the lattice's, until the top's edge held it, frozen for 120 s at (102.25, 34.01, 123.37) while the
/// game's body ran on down to (100.1, 32.7, 119.5) by 7.0 s (fix-lip's exe replay, rig_real_v3).
pub(crate) fn kept_climb(grid: &mapdata::WalkGrid, to: Vec3, top: f32, walked: Vec3, airborne: bool) -> Option<(NodeId, Vec3)> {
    if top <= walked.y + KEPT_CLIMB_OVER_M {
        return None;
    }
    if airborne {
        kept_moves::count(kept_moves::Kind::NoClimbInAir);
        return None;
    }
    let at = Vec3::new(to.x, top, to.z);
    let node = grid.locate(at, 0.6)?;
    let floor = grid.position(node).y;
    ((floor - top).abs() <= KEPT_CLIMB_FLOOR_M).then_some((node, Vec3::new(at.x, floor, at.z)))
}

/// [stand-in fidelity] The stand-in's climbs and falls by its kept controller ([`stand_in_kept_climb`], [`stand_in_kept_fall`]):
/// climbs made, falls started, falls landed, falls handed back to the stand-in's own fall (too long, or no controller), and steps over
/// the lattice's walk that climbed nothing for a controller in the air ([`kept_climb`]).
pub(crate) mod kept_moves {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    #[derive(Clone, Copy)]
    pub(crate) enum Kind {
        Climbed,
        FallStarted,
        FallLanded,
        FallHandedBack,
        NoClimbInAir,
    }

    static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

    pub(crate) fn count(kind: Kind) {
        COUNTS[kind as usize].fetch_add(1, Relaxed);
    }

    /// (climbed, falls started, landed, handed back, no climb in the air) so far.
    pub(crate) fn counts() -> [u64; 5] {
        std::array::from_fn(|i| COUNTS[i].load(Relaxed))
    }
}

pub(crate) fn stand_in_walk_off() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_WALKOFF").map_or(true, |v| v != "off"))
}

/// What [`stand_in_walk_off`] did: walk-offs that fell from the edge, those that came down, and the metres across they carried in all
/// (tenths).
static WALK_OFFS: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];

pub(crate) fn walk_off_stats() -> Vec<u64> {
    WALK_OFFS.iter().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// [stand-in fidelity] The stand-in's falls that came down on the client's walker ground where the grid had no floor of that height
/// under the body ([`fly`]): how many, and the metres the ground stood over the grid's floor there in all (hundredths; the grid's
/// floor under, or the fall's height where it had none).
pub(crate) mod walker_landings {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub(crate) fn landed(over_m: f32) {
        COUNTS[0].fetch_add(1, Relaxed);
        COUNTS[1].fetch_add((over_m.max(0.0) * 100.0) as u64, Relaxed);
    }

    /// (landings, metres over the grid's floor in hundredths) so far.
    pub(crate) fn counts() -> [u64; 2] {
        std::array::from_fn(|i| COUNTS[i].load(Relaxed))
    }
}

/// A walk off an edge starts its fall ([`stand_in_walk_off`]).
pub(crate) fn walk_off_started() {
    WALK_OFFS[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// What [`past_edge`] did: walked on past the boundary, fell to the floor below, found no floor below.
static EDGE_FALLS: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];

pub(crate) fn edge_fall_stats() -> Vec<u64> {
    EDGE_FALLS.iter().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// A stand-in body (on `node`) that the slide stopped short of `to` with no wall in the way (`walls`, the walker's triangles, met
/// above a step: a wall stops the capsule, an edge does not): on to `to` while a floor of its level within [`body::SUPPORT_M`] still
/// holds it, else down on the highest floor under `to` (straight below, within the capsule's width). None at a wall, or with no
/// floor below.
pub(crate) fn past_edge(grid: &mapdata::WalkGrid, walls: &mapdata::RayTree, node: NodeId, feet: Vec3, to: Vec3) -> Option<(NodeId, Vec3)> {
    use std::sync::atomic::Ordering::Relaxed;
    let ahead = feet.distance_xz(to);
    if ahead <= 1e-3 {
        return None;
    }
    let (base, step) = (grid.position(node).y, grid.body().step);
    let dir = Vec3::new((to.x - feet.x) / ahead, 0.0, (to.z - feet.z) / ahead);
    if walls.ray_cast(Vec3::new(feet.x, base + step + 0.1, feet.z), dir, ahead + grid.body().radius).is_some() {
        return None;
    }
    let level = |n: NodeId| (grid.position(n).y - base).abs() <= step;
    let held = std::iter::once(node)
        .chain(grid.links(node).filter(|(_, kind)| !kind.is_leap()).map(|(next, _)| next))
        .filter(|&n| level(n) && grid.position(n).distance_xz(to) <= body::SUPPORT_M)
        .min_by(|&a, &b| grid.position(a).distance_xz(to).total_cmp(&grid.position(b).distance_xz(to)));
    if let Some(n) = held {
        EDGE_FALLS[0].fetch_add(1, Relaxed);
        return Some((n, Vec3::new(to.x, grid.position(n).y, to.z)));
    }
    // Down: the first floor under `to` lower than a step, probing in overlapping height windows (`locate` looks 2.5 m down),
    // within the capsule's width across: the floor at a wall's foot is eroded, and the falling capsule comes down off the face.
    let mut y = base - step - 1.0;
    while y > base - 60.0 {
        if let Some(n) = grid.locate(Vec3::new(to.x, y, to.z), 2.0 * grid.body().radius)
            && grid.position(n).y < base - step
        {
            EDGE_FALLS[1].fetch_add(1, Relaxed);
            return Some((n, grid.position(n)));
        }
        y -= 3.0;
    }
    EDGE_FALLS[2].fetch_add(1, Relaxed);
    None
}

/// Whether the stand-in's jump is steered in the air by the keys held (`SRP_BENCH_STAND_IN_AIR`, on unless `off`): the client's
/// controller adds its air control ([`air_control`]) to each airborne tick's move; off, the jump goes only the way of the keys at
/// the press, as the benches before #966 had it (with it on, A211's arms got up 46 % of their route jumps against 53 % off,
/// toward the live 45 % at Vostok Radar Station).
pub(crate) fn stand_in_air_control() -> bool {
    if let Some(on) = AIR_FORCED.with(std::cell::Cell::get) {
        return on;
    }
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_AIR").map_or(true, |v| v != "off"))
}

thread_local! {
    /// A test's word on [`stand_in_air_control`], for its own thread.
    static AIR_FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// A test puts [`stand_in_air_control`] on or off for its own thread (`None`: as the run says).
pub(crate) fn force_air_control(on: Option<bool>) {
    AIR_FORCED.with(|f| f.set(on));
}

/// The client's air control, m/s: `air_control_speed` (player +0x304, 1.0 in `default.player`), the length of
/// [`air_control`]'s vector.
pub(crate) const AIR_CONTROL_MS: f32 = 1.0;

/// The air control of a body in a jump with `keys` held now, facing `yaw`: m/s on the ground's axes. The client's
/// `get_air_control_vector` 0x2D7990 (a switch over the move direction, table 0x2D7B68; its constants read from the exe: 0xA70884
/// 1.0, 0xA0D984 0.0, 0xA2D778 -1.0; PROVEN) gives [`AIR_CONTROL_MS`] along the keys' way, the diagonals normalised, and none with
/// no key or with forward alone in a sprint jump (its forward case is 0 for the jump codes 2-4; the diagonals have no such case).
/// `player_step` 0x7BFC5 adds it to the root motion's speed (the walk vector times 1 / dt, 0x7BF36) each tick while the
/// controller's jump flag is set, and only then (its air branch, 0x7BF2C; the flag set by `base_player::jump` 0x2D7920 at the
/// take-off clip's event, cleared by `end_jump` 0x7F050 at the landing; 1,392 of 1,392 air steps: PROVEN, exactness, register
/// G51). A walk-off fall never sets the flag, so it has no air control.
pub(crate) fn air_control(keys: u32, yaw: f32, sprint: bool) -> [f32; 2] {
    let held = |bit| f32::from(u8::from(keys & bit != 0));
    let ahead = held(key::FORWARD) - held(key::BACK);
    let left = held(key::LEFT) - held(key::RIGHT);
    if (ahead == 0.0 && left == 0.0) || (sprint && ahead > 0.0 && left == 0.0) {
        return [0.0, 0.0];
    }
    let len = ahead.hypot(left);
    let dir = to_world([ahead / len, left / len], yaw);
    [dir[0] * AIR_CONTROL_MS, dir[1] * AIR_CONTROL_MS]
}

/// What [`stand_in_air_control`] did: airborne ticks steered, airborne ticks with no air control (no key, or forward alone in a
/// sprint jump), jumps come down, and of them those the air control moved 0.25 m or more.
static AIR_STEERS: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

pub(crate) fn air_steer_stats() -> Vec<u64> {
    AIR_STEERS.iter().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// Whether a jump's walk before it leaves the floor is held by the walls and the ledge as the walking body is (`SRP_BENCH_STAND_IN_JUMP_WALL`,
/// off unless `on`, and only with `SRP_BENCH_STAND_IN_CORNER=on`): the client's capsule is held at a wall whatever the keys, so a body
/// that presses jump at the wall it is held by does not walk through it in the jump's first moments (A199).
pub(crate) fn stand_in_jump_wall() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_JUMP_WALL").is_ok_and(|v| v == "on"))
}

/// What a jump's walk on the floor needs of the capsule rule ([`stand_in_jump_wall`], [`super::standin_capsule`]): the walls' sections, whether
/// the rule holds the body (else it only counts the moves it would hold), and the counts of walk ticks and held ones.
pub(crate) struct JumpWall<'a> {
    pub(crate) sections: &'a super::standin_capsule::Sections,
    pub(crate) hold: bool,
    pub(crate) ticks: &'a mut u32,
    pub(crate) held: &'a mut u32,
}

/// The floor a body held at a face (on `node`, at `feet`, walking toward `to`) is lifted up: the jump link ahead
/// ([`jumped_to`]) when its rise is the lift's at most ([`LIFT_MAX_RISE_M`]) and its top deep enough ([`LIFT_TOP_M`]).
pub(crate) fn lift_to(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Option<NodeId> {
    lift_check(grid, node, feet, to).ok()
}

/// What [`lift_check`] found against a lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoLift {
    /// No jump link ahead of the body toward the point it pushes at.
    NoLink,
    /// The link's rise is over the lift's ([`LIFT_MAX_RISE_M`]).
    TooHigh,
    /// The top is shallower than [`LIFT_TOP_M`].
    TopShort,
}

/// [`lift_to`] with what was against it.
pub(crate) fn lift_check(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Result<NodeId, NoLift> {
    use crate::v026g::bots::motor::{LIFT_MAX_RISE_M, LIFT_TOP_M};
    let up = jumped_to(grid, node, feet, to).ok_or(NoLift::NoLink)?;
    let rise = grid.position(up).y - grid.position(node).y;
    if rise > LIFT_MAX_RISE_M {
        Err(NoLift::TooHigh)
    } else if crate::v026g::bots::path::top_depth(grid, node, up) < LIFT_TOP_M {
        Err(NoLift::TopShort)
    } else {
        Ok(up)
    }
}

/// The rise of the jump link ahead of a body at a face, if there is one.
pub(crate) fn lift_probe(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Option<f32> {
    jumped_to(grid, node, feet, to).map(|up| grid.position(up).y - grid.position(node).y)
}

/// How the stand-in's lifts came out, by the band of the link's rise (up to 0.80, 0.80-0.95, 0.95-1.05, 1.05-1.11, over 1.11,
/// none known) and the outcome ([`LiftOutcome`]).
static LIFT_STATS: [[std::sync::atomic::AtomicU64; 6]; 6] = [const { [const { std::sync::atomic::AtomicU64::new(0) }; 6] }; 6];

#[derive(Clone, Copy, Debug)]
pub(crate) enum LiftOutcome {
    /// Held at the face the client's hold and carried up.
    Made,
    /// The sprint or forward key was let go before the hold was up.
    Released,
    /// Held the hold, and the link's rise is over the lift's.
    TooHigh,
    /// Held the hold, and the top is too shallow.
    TopShort,
    /// Held the hold, and no jump link lay ahead of the point it pushed at.
    NoLink,
    /// Came to the face of a jump link under the sprint speed the client asks.
    SlowContact,
}

pub(crate) fn lift_count(rise: Option<f32>, outcome: LiftOutcome) {
    let band = match rise {
        None => 5,
        Some(r) if r <= 0.8 => 0,
        Some(r) if r <= 0.95 => 1,
        Some(r) if r <= 1.05 => 2,
        Some(r) if r <= 1.11 => 3,
        Some(_) => 4,
    };
    LIFT_STATS[band][outcome as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The stand-in lift outcomes: `[band][outcome]`, flattened.
pub(crate) fn lift_stats() -> Vec<u64> {
    LIFT_STATS.iter().flatten().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// A stand-in world: a floor at 0 and a ledge `rise` m high from x 10 on,
/// its grid built for `body`.
pub(crate) fn ledge_grid(rise: f32, body: mapdata::Body) -> mapdata::WalkGrid {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, rise));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(rise));
        t
    }));
    mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, body)
}

/// Where a stand-in body walking east from x 8 at 0.2 m a tick ends after
/// 40 ticks: in the air all the way with `jump` (one jump), or never.
pub(crate) fn walk_into_ledge(grid: &mapdata::WalkGrid, jump: Option<StandInJump>) -> Vec3 {
    let mut node = grid.locate(Vec3::new(8.0, 0.0, 5.0), 0.5).unwrap();
    let mut feet = grid.position(node);
    let mut jumped = false;
    for _ in 0..40 {
        let to = feet + Vec3::new(0.2, 0.0, 0.0);
        (node, feet, _) = slide(grid, node, feet, to);
        if let Some(jump) = jump
            && !jumped
            && feet.distance_xz(to) > 1e-3
            && let Some(up) = stand_in_jump(grid, node, feet, to, jump)
        {
            node = up;
            feet = grid.position(up);
            jumped = true;
        }
    }
    feet
}

/// A stand-in body walking into a 0.9 m ledge stops at its foot; in the
/// air it gets up on it, by the grid's jump link.
#[test]
pub(crate) fn a_stand_in_body_in_the_air_gets_up_a_ledge() {
    let grid = ledge_grid(0.9, mapdata::Body::default());
    let stopped = walk_into_ledge(&grid, None);
    assert!(stopped.x < 10.0 && stopped.y.abs() < 0.1, "{stopped:?}");
    let up = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(up.x > 12.0 && (up.y - 0.9).abs() < 0.1, "{up:?}");
}

/// A body held at the face of a 0.9 m top with a deep floor on it is lifted up it by the client's sprint lift; a top
/// above the lift's ceiling (1.2 m) is not lifted.
#[test]
pub(crate) fn a_stand_in_lift_takes_a_held_body_up_a_low_top_only() {
    let at_face = |grid: &mapdata::WalkGrid| {
        let node = grid.locate(Vec3::new(9.75, 0.0, 5.0), 0.3).unwrap();
        let feet = grid.position(node);
        (node, feet, feet + Vec3::new(0.25, 0.0, 0.0))
    };
    let low = ledge_grid(0.9, mapdata::Body::default());
    let (node, feet, to) = at_face(&low);
    let up = lift_to(&low, node, feet, to).expect("a 0.9 m top is lifted");
    assert!((low.position(up).y - 0.9).abs() < 0.05, "{:?}", low.position(up));
    let high = ledge_grid(1.2, mapdata::Body::client());
    let (node, feet, to) = at_face(&high);
    assert!(lift_to(&high, node, feet, to).is_none(), "a 1.2 m top is not");
}

/// A grid jump link higher than the client's jump (1.03 m, linked under
/// the grid's 1.05 m) is not made: the body stays at the ledge's foot.
#[test]
pub(crate) fn a_stand_in_jump_does_not_make_a_link_over_the_clients_height() {
    let grid = ledge_grid(1.03, mapdata::Body::default());
    let stayed = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(stayed.x < 10.0 && stayed.y.abs() < 0.1, "{stayed:?}");
}

/// With no grid link (a grid built for a 0.5 m jump), the jump still gets
/// up a 0.9 m ledge; not up a 1.2 m one, which only a sprint jump clears.
#[test]
pub(crate) fn a_stand_in_jump_gets_up_a_ledge_with_no_link_within_its_height() {
    let low = mapdata::Body {
        max_jump: 0.5,
        ..mapdata::Body::default()
    };
    let grid = ledge_grid(0.9, low);
    let up = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(up.x > 10.0 && (up.y - 0.9).abs() < 0.1, "{up:?}");
    let grid = ledge_grid(1.2, low);
    let stayed = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(stayed.x < 10.0 && stayed.y.abs() < 0.1, "{stayed:?}");
    let sprint = walk_into_ledge(&grid, Some(SPRINT_JUMP));
    assert!(
        sprint.x > 10.0 && (sprint.y - 1.2).abs() < 0.1,
        "{sprint:?}"
    );
}

/// Whether the stand-in moves bodies as the client does (the default): no
/// step up over the client's ([`super::standin::client_step`]: the grid body's
/// `step`, 0.80 m on the clients' body, `startup.cfg` `cc_step_height 0.80` in the
/// game's `resources.db`; the exe's own default, `s_step_height` 0.6, is replaced
/// by that cvar), and a jump that follows the keys held
/// at its press along the game's jump clip ([`Flight`]).
/// `SRP_BENCH_STAND_IN=old` brings back the old stand-in (any step the
/// grid walks, a jump that lifts the body up any floor in reach,
/// [`stand_in_jump`]), only to compare with benches of older bases.
pub(crate) fn honest_stand_in() -> bool {
    static HONEST: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HONEST.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN").map_or(true, |v| v != "old"))
}

/// A jump clip's root over the floor it leaves, every 2 of its 24 frames
/// (PROVEN: the stock clips' root curves, private sprint-jump-re
/// `work/jumpcurves.txt` and FINDINGS §3-5): its length, when the body
/// leaves the floor (the clip's `jump` event, frame 6), its rise and how
/// far ahead it has gone, m.
#[derive(Debug)]
pub(crate) struct Clip {
    pub(crate) ms: f32,
    pub(crate) flag_ms: f32,
    pub(crate) rise: [f32; 13],
    pub(crate) ahead: [f32; 13],
}

impl Clip {
    /// How far ahead and how high the root is `ms` into the clip (linear
    /// between the samples).
    pub(crate) fn at(&self, ms: f32) -> (f32, f32) {
        let f = (ms / self.ms).clamp(0.0, 1.0) * 12.0;
        let i = (f.floor() as usize).min(11);
        let t = f - i as f32;
        let lerp = |c: &[f32; 13]| c[i] + (c[i + 1] - c[i]) * t;
        (lerp(&self.ahead), lerp(&self.rise))
    }

    /// When the root is at its highest, ms into the clip.
    pub(crate) fn apex_ms(&self) -> f32 {
        let top = (0..13)
            .max_by(|&a, &b| self.rise[a].total_cmp(&self.rise[b]))
            .unwrap_or(0);
        self.ms * top as f32 / 12.0
    }
}

/// The short jump (`on_site_fwd`, codes 5-12): 0.80 s, 1.0 m up at 1.78 m,
/// 2.67 m forward over the clip. With no movement key the body jumps in
/// place (`on_site`, the same rise). The back clip and the four diagonals
/// carry the forward clip's 2.667 m along their way on the same table (EXACT);
/// the right clip carries 2.000 m (linear) and the left 2.222 m (uneven), and
/// the client scales each by s = min(max(1, v) / S, 0.99), S 4, back 3,
/// back-right 2.75 (PROVEN as data, the clips' roots; exactness
/// `routes-readings` 1.5, 1.7). The stand-in flies the forward table for
/// every way, unscaled: kept as its model until changed (register G52).
pub(crate) static SHORT_CLIP: Clip = Clip {
    ms: 800.0,
    flag_ms: 200.0,
    rise: [
        0.0, 0.0, 0.0, 0.0, 0.36, 0.64, 0.84, 0.96, 1.00, 0.94, 0.77, 0.46, 0.0,
    ],
    ahead: [
        0.0, 0.22, 0.44, 0.67, 0.89, 1.11, 1.33, 1.56, 1.78, 2.00, 2.22, 2.44, 2.67,
    ],
};

/// The sprint jump (`sprint_from_left_leg`, codes 2-4), played here in
/// 0.88 s: 1.28 m up, 4.42 m forward. The client plays the take-off clip at
/// time scale 1.0, 0.80 s (0x342D70 sets +44h to 1.0 at 0x342E05; 12 air
/// steps in 16 of 16 sprint jumps; exactness `routes-readings` 1.6): kept as
/// the stand-in's model until changed (register G49); the right leg's
/// tops at 1.35 m (the left one taken: the bot cannot see the leg). Its
/// diagonals carry 3.12 m on each axis, 4.41 m along the way.
pub(crate) static SPRINT_CLIP: Clip = Clip {
    ms: 880.0,
    flag_ms: 220.0,
    rise: [
        0.0, 0.0, 0.0, 0.0, 0.57, 1.08, 1.24, 1.28, 1.23, 1.01, 0.70, 0.35, 0.0,
    ],
    ahead: [
        0.0, 0.38, 0.76, 1.15, 1.54, 1.92, 2.29, 2.65, 3.01, 3.36, 3.71, 4.06, 4.42,
    ],
};

/// The grid builds its gap links (`mapdata::grid::gap`) by flying the same
/// clip: the numbers must be the same.
#[test]
pub(crate) fn the_grids_gap_links_fly_this_short_clip() {
    assert_eq!(SHORT_CLIP.ahead, mapdata::grid::gap::SHORT_JUMP_AHEAD);
    assert_eq!(SHORT_CLIP.rise, mapdata::grid::gap::SHORT_JUMP_RISE);
}

/// Before the sprint clip, the prepare phase finishes the run's step: the stand-in's 0 to 0.15 s and 0 to 1.64 m, by a
/// phase drawn uniform at the press (sprint-jump RE `FINDINGS.md` §3.2, `re-sprint-prepare.md`). Not the client's:
/// `jump_logic_state_prepare::initialize` 0x342540 waits for the run clip's next `anim_intervals` mark (`on_interval_end`
/// 0x342A50 starts the jump), played on at the rate of `get_prepare_time_factor` 0x3375A0, so the prepare lasts
/// 2AC / (A + BC) < 2C = 80 ms with C = startup.cfg `jump_prepare_interval_length` 0.040 (not the exe's default 0.1);
/// counted 52-81 ms from the press to the take-off clip's start, the body moving at its walk cap meanwhile, 0.19-0.30 m
/// (rig `notes-jump-state.md` 0, 3, 7, 9). Kept as the stand-in's model until changed (register G53).
pub(crate) const SPRINT_PREPARE_MS: f32 = 150.0;

pub(crate) const SPRINT_PREPARE_M: f32 = 1.64;

/// The walk cap, m/s: the body's speed at the press must be at least this
/// for a sprint jump, and a body on the ground in a jump moves at most
/// this fast (the root runs ahead; the body takes the difference when it
/// leaves the floor), PROVEN (FINDINGS §1.2, §3.2).
pub(crate) const WALK_CAP: f32 = 3.5;

/// A body falling after its clip, m/s²: the client's `fall_params.gravity` (PROVEN, `docs/notes/fall_damage.md`: `step_down` 0x7E1B0
/// adds 0.49 m/s a 0.05 s tick), the reckoning's own (`body.rs` `GRAVITY`). Was 9.81 with no source.
pub(crate) const GRAVITY: f32 = 9.8;

/// The heights over the feet at which a face holds a body in the air
/// (rays on the walker mesh), m: its feet (a little over, so a floor's
/// own bumps do not), its middle and its head; a body clears an edge once
/// its feet are over it (FINDINGS §5, the capsule rule). The body's
/// radius is the grid's.
pub(crate) const AIR_RAYS: [f32; 3] = [0.1, 0.9, 1.7];

/// A jump under way in the honest stand-in ([`honest_stand_in`]): the
/// body follows the clip's root from where it pressed, along the way of
/// the keys held at the press; on the floor before the clip's jump event
/// (at the walk cap), in the air after it, held by any face in its way
/// (it rises with the clip and catches up once clear: the chase rule,
/// FINDINGS §5). It lands on the highest floor under it once on the way
/// down, or falls after the clip. No stamina; the air control with
/// [`stand_in_air_control`] (on the clip, not in the fall after it).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flight {
    /// The press, ms; the feet then; the ground way (unit, or none).
    pub(crate) at: u32,
    pub(crate) from: Vec3,
    pub(crate) dir: [f32; 2],
    pub(crate) clip: &'static Clip,
    /// The sprint jump's prepare phase: its length (ms) and travel (m).
    pub(crate) prepare: (f32, f32),
    /// The floor's height when the body left it.
    pub(crate) base: Option<f32>,
    /// After the clip, falling at this speed, m/s.
    pub(crate) fall: Option<f32>,
    /// How far the air control has moved the body off the clip's way so far, m ([`stand_in_air_control`]).
    pub(crate) air: [f32; 2],
    /// The speed across kept while it falls, m/s: a walk off an edge's ([`stand_in_walk_off`]); none for a jump (its fall after the
    /// clip is straight down).
    pub(crate) carry: [f32; 2],
    /// [T3] The client's clip ([`stand_in_client_jump`]): the tick's root delta moves the body; none: the old flight ([`Self::root`]).
    pub(crate) arc: Option<crate::v026g::bots::jump_arc::Clip>,
    /// [A364] The body's last horizontal move at the press, m/s (the game's player +0x334): a short client jump travels its direction
    /// clip x `jump_arc::Clip::carry` of it.
    pub(crate) speed: f32,
    /// [stand-in jump] The client's own flight from the kept controller ([`stand_in_controller_jump`]): it moves the body, not the
    /// clip; none: the clip's flight (a side or back press, or the switch off).
    pub(crate) flown: Option<crate::v026g::bots::body::FlownJump>,
    /// [stand-in fidelity] Falling by the body's kept controller ([`stand_in_kept_fall`]): each tick the client's walk tick with the
    /// tick's walk keys moves it (`body::kept_step`, the controller's own height kept), down to where the controller is on the ground
    /// again; false: the stand-in's own fall ([`Self::fall`], [`Self::carry`]).
    pub(crate) kept_fall: bool,
}

impl Flight {
    /// The root `ms` after the press: how far ahead, how high, whether
    /// the body has left the floor, and whether the clip is over.
    pub(crate) fn root(&self, ms: f32) -> (f32, f32, bool, bool) {
        let (pm, pd) = self.prepare;
        if ms < pm {
            return (pd * ms / pm, 0.0, false, false);
        }
        let c = ms - pm;
        let (ahead, rise) = self.clip.at(c);
        (pd + ahead, rise, c >= self.clip.flag_ms, c >= self.clip.ms)
    }

    pub(crate) fn descending(&self, ms: f32) -> bool {
        ms - self.prepare.0 >= self.clip.apex_ms()
    }

    pub(crate) fn sprint(&self) -> bool {
        std::ptr::eq(self.clip, &SPRINT_CLIP)
    }
}

/// The jump the keys held at a press make (FINDINGS §4, the codes): none,
/// in place; forward, forward-right or forward-left the sprint jump when
/// sprint is held with none of back, fire, crouch or aim, the body moves
/// at the walk cap or more and may sprint (`can_sprint`: no battery, no
/// stagger), else the short jump; any other way the short jump that way.
/// The clip and the ground way, from the body's yaw.
pub(crate) fn jump_of(keys: u32, yaw: f32, speed: f32, may_sprint: bool) -> (&'static Clip, [f32; 2]) {
    let held = |bit| f32::from(u8::from(keys & bit != 0));
    let ahead = held(key::FORWARD) - held(key::BACK);
    let left = held(key::LEFT) - held(key::RIGHT);
    if ahead == 0.0 && left == 0.0 {
        return (&SHORT_CLIP, [0.0, 0.0]);
    }
    let len = ahead.hypot(left);
    let dir = to_world([ahead / len, left / len], yaw);
    let sprint = ahead > 0.0
        && keys & key::SPRINT != 0
        && keys & (key::BACK | key::FIRE | key::CROUCH | key::AIM) == 0
        && speed >= WALK_CAP
        && may_sprint;
    (if sprint { &SPRINT_CLIP } else { &SHORT_CLIP }, dir)
}

/// The floor a body in the air at `at` comes down on: the highest whose
/// cell holds it at or below `top`; else (its middle over the ground a
/// wall's or an edge's erosion leaves out of the grid) the highest within
/// the body's radius, the nearest of them. Its floor, and where the feet
/// stand on it.
pub(crate) fn air_floor(grid: &mapdata::WalkGrid, at: Vec3, top: f32) -> Option<(NodeId, Vec3)> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let box_of = |r: f32| [at.x - r, at.z - r, at.x + r, at.z + r];
    let under = grid
        .floors_in(box_of(half))
        .filter(|&n| {
            let p = grid.position(n);
            (p.x - at.x).abs() <= half && (p.z - at.z).abs() <= half && p.y <= top
        })
        .max_by(|&a, &b| grid.position(a).y.total_cmp(&grid.position(b).y));
    if let Some(n) = under {
        return Some((n, Vec3::new(at.x, grid.position(n).y, at.z)));
    }
    let r = grid.body().radius;
    grid.floors_in(box_of(r))
        .filter(|&n| {
            let p = grid.position(n);
            p.distance_xz(at) <= r && p.y <= top
        })
        .max_by(|&a, &b| {
            let (pa, pb) = (grid.position(a), grid.position(b));
            pa.y.total_cmp(&pb.y)
                .then(pb.distance_xz(at).total_cmp(&pa.distance_xz(at)))
        })
        .map(|n| (n, grid.position(n)))
}

/// Where a body in the air at `feet` (its feet at `y`) gets moving toward
/// `(x, z)`: all the way, else along one axis (along a face), else
/// nowhere; a move is held when a ray at one of [`AIR_RAYS`] meets the
/// walker mesh within the move and the body's radius.
pub(crate) fn air_move(walls: &mapdata::RayTree, radius: f32, feet: Vec3, to: (f32, f32), y: f32) -> (f32, f32) {
    let held = |from: (f32, f32), to: (f32, f32)| {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let len = dx.hypot(dz);
        if len < 1e-6 {
            return false;
        }
        let dir = Vec3::new(dx / len, 0.0, dz / len);
        AIR_RAYS.iter().any(|&h| {
            walls
                .ray_cast(Vec3::new(from.0, y + h, from.1), dir, len + radius)
                .is_some()
        })
    };
    let from = (feet.x, feet.z);
    [to, (to.0, from.1), (from.0, to.1)]
        .into_iter()
        .find(|&t| t != from && !held(from, t))
        .unwrap_or(from)
}

/// How a jump ended: who pressed it and when, where the body came down,
/// and whether it was a sprint jump.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Landing {
    pub(crate) place: Place,
    pub(crate) at: u32,
    pub(crate) to: Vec3,
    pub(crate) sprint: bool,
    /// The vertical speed of the free fall at the landing tick, m/s (`Flight::fall`; none: the clip ended on the floor). The client's fall damage
    /// is a function of it alone (`docs/notes/fall_damage.md`); the bench does not apply it ([`fall_damage`] only counts).
    pub(crate) fall_v: Option<f32>,
}

/// The client's fall damage at a landing speed `v` m/s with `fatal_damage` 1.0 (the resources.db patch): none up to `harmless_height` 5 m
/// (9.9 m/s), `(v - 9.9) / (14 - 9.9)` up to `fatal_height` 10 m (14 m/s), 1.0 beyond (EXE + DATA, PROVEN, `docs/notes/fall_damage.md`:
/// the law 0x2d8220, its loader 0x2d6600 reading `gameplay/players/default.player` `fall_params`).
pub(crate) fn fall_damage(v: f32) -> f32 {
    ((v - HARMLESS_V) / (FATAL_V - HARMLESS_V)).clamp(0.0, 1.0)
}

/// `sqrt(2 g h)` for `harmless_height` 5 m and `fatal_height` 10 m at the client's gravity 9.8.
const HARMLESS_V: f32 = 9.899_495;
const FATAL_V: f32 = 14.0;
/// A fall of this damage or more kills a body at full health (about 0.6, `fall_damage.md`).
pub(crate) const LETHAL_FALL_DAMAGE: f32 = 0.6;

/// [stand-in fidelity] A tick's walk for [`fly`]: the walk's heading (the input before the tick's, as the stand-in's walk) and the
/// tick's walk move (flat, m), which steers a fall by the kept controller ([`stand_in_kept_fall`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct FlyWalk {
    pub(crate) heading: f32,
    pub(crate) walk: Vec3,
}

/// One tick of `body`'s jump ([`Flight`]) at `tick`; the landing when it
/// came down.
///
/// `wall` is the capsule rule for the walk before the jump's first airborne moment ([`stand_in_jump_wall`]).
pub(crate) fn fly(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    mut wall: Option<JumpWall>,
    body: &mut Body,
    place: Place,
    tick: u32,
    FlyWalk { heading, walk }: FlyWalk,
) -> Option<Landing> {
    let mut f = body.flight?;
    let Some(node) = body.node else {
        body.flight = None;
        return None;
    };
    let ms = tick.saturating_sub(f.at) as f32;
    let landed = |body: &mut Body, f: &Flight, n: NodeId, feet: Vec3, fall_v: Option<f32>| {
        if stand_in_air_control() {
            use std::sync::atomic::Ordering::Relaxed;
            AIR_STEERS[2].fetch_add(1, Relaxed);
            if f.air[0].hypot(f.air[1]) >= 0.25 {
                AIR_STEERS[3].fetch_add(1, Relaxed);
            }
        }
        body.node = Some(n);
        body.feet = feet;
        body.flight = None;
        Some(Landing {
            place,
            at: f.at,
            to: feet,
            sprint: f.sprint(),
            fall_v,
        })
    };
    // [stand-in jump] The client's own flight ([`stand_in_controller_jump`]): its step from the walk's heading `heading` (the input
    // before the tick's, as the stand-in's walk) with the air control of the tick's own keys, as the reckoning steps it; at its
    // landing step the controller is the body's kept one, on the ground, or falling on from there by the stand-in's fall.
    if let Some(mut flown) = f.flown {
        use crate::v026g::bots::body::FlownStep;
        let Some(step) = flown.step(grid, heading, body.keys, body.yaw) else {
            body.flight = None;
            return None;
        };
        match step {
            FlownStep::Air(feet) => {
                body.feet = feet;
                f.flown = Some(flown);
                body.flight = Some(f);
                return None;
            }
            FlownStep::Last { feet, landed: on_ground, after, vertical, travel } => {
                // The client's walk after the landing resumes from the jump's own travel, straight ahead (the reckoning's rule
                // in `body::advance`; recorded: exactness's `landing-walk-check-v2`), not from the blend the keys held through
                // the flight would have reached.
                body.blend = Blend::at([travel, 0.0]);
                body.landing_walk = Some(travel);
                let under = grid.locate(feet, 1.5).unwrap_or(node);
                if controller_jumps::log() {
                    eprintln!(
                        "stand-in jump: place {place} pressed {} ms ago from ({:.2}, {:.2}, {:.2}) at {:.2} m/s: {} at ({:.2}, {:.2}, {:.2}), vertical {vertical:.2} m/s, walk on from the jump's travel {travel:.2} m/s",
                        tick.saturating_sub(f.at),
                        f.from.x,
                        f.from.y,
                        f.from.z,
                        f.speed,
                        if on_ground { "landed" } else { "falls on" },
                        feet.x,
                        feet.y,
                        feet.z
                    );
                }
                if on_ground {
                    controller_jumps::count(controller_jumps::Kind::Landed);
                    body.kept = Some(after);
                    return landed(body, &f, under, feet, None);
                }
                controller_jumps::count(controller_jumps::Kind::Fell);
                body.feet = feet;
                f.flown = None;
                f.fall = Some((-vertical).max(0.0));
                if stand_in_kept_fall() {
                    // [stand-in fidelity] It falls on by its kept controller, the flight's own ([`stand_in_kept_fall`]).
                    kept_moves::count(kept_moves::Kind::FallStarted);
                    body.kept = Some(after);
                    f.kept_fall = true;
                } else {
                    body.kept = None;
                }
                body.flight = Some(f);
                return None;
            }
        }
    }
    // [stand-in fidelity] The fall by the kept controller ([`stand_in_kept_fall`]): the client's walk tick with this tick's walk, the
    // controller's own height kept, until it is on the ground; [`KEPT_FALL_MAX_MS`] or no controller hands it to the stand-in's fall.
    if f.kept_fall {
        use crate::v026g::bots::body;
        let stepped = (tick.saturating_sub(f.at) <= KEPT_FALL_MAX_MS)
            .then(|| body::kept_step(grid, &mut body.kept, body.feet, walk, body::crouch_input(body.keys, false), false, body::kept_clock::Of::StandIn))
            .flatten();
        match stepped {
            Some(step) => {
                let height = body.kept.map_or(body.feet.y, |kept| kept.height());
                let feet = Vec3::new(step.to.x, height, step.to.z);
                let down = ((body.feet.y - height) / TICK_S).max(0.0);
                if !step.airborne {
                    kept_moves::count(kept_moves::Kind::FallLanded);
                    let under = grid.locate(feet, 1.5).unwrap_or(node);
                    let fall_v = f.fall.map_or(down, |v| v.max(down));
                    return landed(body, &f, under, feet, Some(fall_v));
                }
                body.feet = feet;
                f.fall = Some(down);
                body.flight = Some(f);
                return None;
            }
            None => {
                kept_moves::count(kept_moves::Kind::FallHandedBack);
                body.kept = None;
                f.kept_fall = false;
                f.fall = Some(f.fall.unwrap_or(0.0));
            }
        }
    }
    if let Some(v) = f.fall {
        // [A318] The client's fall ([`stand_in_slow_fall`]): the slow start, then the speed at the tick's start; else as before.
        let (v, down) = if stand_in_slow_fall() {
            super::body::client_fall_tick(v)
        } else {
            let v = v + GRAVITY * TICK_S;
            (v, v * TICK_S)
        };
        let y = body.feet.y - down;
        // A walk off an edge goes on across at its speed, held by the walls as a jump is ([`air_move`]).
        let (x, z) = if f.carry == [0.0, 0.0] {
            (body.feet.x, body.feet.z)
        } else {
            let to = (body.feet.x + f.carry[0] * TICK_S, body.feet.z + f.carry[1] * TICK_S);
            air_move(walls, grid.body().radius, body.feet, to, y)
        };
        let at = Vec3::new(x, body.feet.y, z);
        // [stand-in fidelity] The client's fall comes down on its walker's ground (`step_down` 0x7E1B0 on the walker mesh), not on
        // the grid's floors only: a falling body whose fall this tick reaches the walker's walkable ground under it stands there,
        // where the grid may have no floor of that height (the suite's S007/1: off a rail's top at 22.1 m the stand-in fell on
        // through the walker's ground at 21.49 m under (-32.47, -7.51), the grid having none there, and on 5 m down; the walker's
        // ground per the capture's collision world, `ground_probe_v1.py`).
        // Only where the grid's floors would not have it down this tick: the stand-in's landings on them stay as they were.
        let grid_down = air_floor(grid, at, body.feet.y + 0.05).is_some_and(|(_, feet)| feet.y >= y);
        if !grid_down
            && let Some(ground) = grid.walker_ground_below(at, body.feet.y + 0.05, body.feet.y + 0.05 - y)
            && ground >= y
        {
            let feet = Vec3::new(x, ground, z);
            let floor = air_floor(grid, at, ground + 0.05);
            walker_landings::landed(ground - floor.map_or(y, |(_, f)| f.y));
            if controller_jumps::log() {
                eprintln!(
                    "stand-in fall: came down on the walker's ground at ({:.2}, {:.2}, {:.2}), {:.2} m over the grid's floor under it",
                    feet.x,
                    feet.y,
                    feet.z,
                    floor.map_or(f32::NAN, |(_, f)| ground - f.y)
                );
            }
            let n = floor.map(|(n, _)| n).or_else(|| grid.locate(feet, 1.5)).unwrap_or(node);
            return landed(body, &f, n, feet, Some(v));
        }
        match air_floor(grid, at, body.feet.y + 0.05) {
            Some((n, feet)) if feet.y >= y => {
                if f.carry != [0.0, 0.0] {
                    use std::sync::atomic::Ordering::Relaxed;
                    WALK_OFFS[1].fetch_add(1, Relaxed);
                    WALK_OFFS[2].fetch_add((feet.distance_xz(f.from) * 10.0) as u64, Relaxed);
                }
                return landed(body, &f, n, feet, Some(v));
            }
            Some(_) => {
                body.feet = Vec3::new(x, y, z);
                f.fall = Some(v);
                body.flight = Some(f);
                return None;
            }
            // Nothing under it at all: back on the floor it left (not seen
            // on a bench; kept from falling for ever).
            None => {
                let feet = grid.position(node);
                return landed(body, &f, node, feet, Some(v));
            }
        }
    }
    // [T3] The client's flight: this tick's root delta from where the body is (no target kept), the vertical with it.
    let step = f.arc.map(|arc| client_step(&f, arc, ms, body.yaw));
    let (airborne, over, mut target) = match step {
        Some(c) => (c.airborne, c.over, (body.feet.x + c.ahead[0], body.feet.z + c.ahead[1])),
        None => {
            let (ahead, _, airborne, over) = f.root(ms);
            (airborne, over, (f.from.x + f.dir[0] * ahead, f.from.z + f.dir[1] * ahead))
        }
    };
    // Off the floor, the keys held now steer it ([`air_control`]).
    if airborne && stand_in_air_control() {
        let steer = air_control(body.keys, body.yaw, f.sprint());
        AIR_STEERS[usize::from(steer == [0.0, 0.0])].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        f.air = [f.air[0] + steer[0] * TICK_S, f.air[1] + steer[1] * TICK_S];
        target = match step {
            // Re-based: this tick's steer alone.
            Some(_) => (target.0 + steer[0] * TICK_S, target.1 + steer[1] * TICK_S),
            None => (target.0 + f.air[0], target.1 + f.air[1]),
        };
    }
    if !airborne {
        // On the floor before the jump event: toward the root at the walk
        // cap at most, no step over the client's.
        let (dx, dz) = (target.0 - body.feet.x, target.1 - body.feet.z);
        let len = dx.hypot(dz);
        if len > 1e-6 {
            let k = (WALK_CAP * TICK_S / len).min(1.0);
            let to = Vec3::new(body.feet.x + dx * k, body.feet.y, body.feet.z + dz * k);
            let mut target = to;
            if let Some(wall) = wall.as_mut() {
                let mut segs = Vec::new();
                let mv = (to.x - body.feet.x, to.z - body.feet.z);
                let reach = mv.0.hypot(mv.1) + super::standin_capsule::RADIUS_M + 0.6;
                wall.sections.near((body.feet.x, body.feet.z), body.feet.y, body.keys & key::CROUCH != 0, reach, &mut segs);
                let held = super::standin_capsule::client_move(&segs, (body.feet.x, body.feet.z), mv, super::standin_capsule::RADIUS_M);
                *wall.ticks += 1;
                if (held.to.0 - to.x).hypot(held.to.1 - to.z) > 0.01 {
                    *wall.held += 1;
                    if wall.hold {
                        target = Vec3::new(held.to.0, to.y, held.to.1);
                    }
                }
            }
            // The walk before the take-off keeps its own (older) slide: moving it onto the stand-in's walk raised jumps failed 62 -> 87 on
            // the bench (2026-11-06, 16 pairs); that change is judged in T3 against the recordings (where the client took off and landed),
            // not by bench counts.
            let (n, feet, _) = if wall.as_ref().is_some_and(|w| w.hold) && stand_in_edge() {
                super::body::slide_supported(grid, node, body.feet, target, super::standin::client_step(grid), super::body::SUPPORT_M)
            } else {
                super::body::slide_capped(grid, node, body.feet, target, super::standin::client_step(grid))
            };
            body.node = Some(n);
            body.feet = feet;
        }
        body.flight = Some(f);
        return None;
    }
    let base = *f.base.get_or_insert(body.feet.y);
    let y = match step {
        Some(c) => body.feet.y + c.rise,
        None => base + f.root(ms).1,
    };
    let was = body.feet.y;
    let (mut x, mut z) = air_move(walls, grid.body().radius, body.feet, target, y);
    // The capsule rule in the air too (A199): the root's step of the first airborne tick is 0.75 m, which the three rays above can pass
    // through a thin wall; the client's capsule is swept along it.
    if let Some(w) = wall.as_ref().filter(|w| w.hold) {
        let mut segs = Vec::new();
        let mv = (x - body.feet.x, z - body.feet.z);
        let reach = mv.0.hypot(mv.1) + super::standin_capsule::RADIUS_M + 0.6;
        // Cut at the floor it left: the sections' heights are above a body standing, and the capsule has the whole of its length in the air.
        w.sections.near((body.feet.x, body.feet.z), base.min(y), false, reach, &mut segs);
        let held = super::standin_capsule::client_move(&segs, (body.feet.x, body.feet.z), mv, super::standin_capsule::RADIUS_M);
        (x, z) = held.to;
    }
    body.feet = Vec3::new(x, y, z);
    let descending = step.map_or_else(|| f.descending(ms), |c| c.rise < 0.0);
    if (descending || over)
        && let Some((n, feet)) = air_floor(grid, body.feet, was.max(y) + 0.05)
        && feet.y >= y - 1e-4
    {
        return landed(body, &f, n, feet, None);
    }
    if over {
        // [T3] The client's fall starts at the root's last descent; the old one from rest.
        f.fall = Some(f.arc.map_or(0.0, |arc| arc.end_descent()));
    }
    body.flight = Some(f);
    None
}

/// [T3] One tick of the client's flight ([`stand_in_client_jump`]): the root's delta over the tick, from the clip's frame `ms - 50`
/// to `ms` after the press (the sprint jump's prepare first: its travel along the press's way), turned by the body's yaw now.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClientStep {
    /// The flat move, map frame, m.
    pub(crate) ahead: [f32; 2],
    /// The rise over the tick, m (down negative).
    pub(crate) rise: f32,
    /// The clip's `jump` event has passed (frame 6): in the air.
    pub(crate) airborne: bool,
    /// The air phase's end (its twelfth step, `jump_arc::Clip::air_end_frame`): `end_jump`.
    pub(crate) over: bool,
}

pub(crate) fn client_step(f: &Flight, arc: crate::v026g::bots::jump_arc::Clip, ms: f32, yaw: f32) -> ClientStep {
    use crate::v026g::bots::jump_arc::TAKEOFF_FRAME;
    let (pm, pd) = f.prepare;
    let prev = (ms - TICK_S * 1_000.0).max(0.0);
    // [A364 P6] The air phase ends at its twelfth step, the clip short of its end (`jump_arc::AIR_STEPS`).
    let end = arc.air_end_frame();
    let frame = |m: f32| arc.frame_after(m - pm).min(end);
    let prep = |m: f32| if pm > 0.0 { pd * m.min(pm) / pm } else { 0.0 };
    let (f0, f1) = (frame(prev), frame(ms));
    let (a, b) = (arc.root(f0), arc.root(f1));
    // [A364] A short jump travels its direction clip x the carry of the press's speed (0x337280: 100 of 100 traced short jumps), a
    // sprint jump its clip whole.
    let k = arc.carry(f.speed);
    let d = to_world([(b[1] - a[1]) * k, -(b[2] - a[2]) * k], yaw);
    let p = prep(ms) - prep(prev);
    ClientStep {
        ahead: [d[0] + f.dir[0] * p, d[1] + f.dir[1] * p],
        rise: b[0] - a[0],
        airborne: f1 >= TAKEOFF_FRAME - 1e-3,
        over: f1 >= end - 1e-3,
    }
}

/// A press's draw in [0, 1) (the sprint jump's prepare phase): a hash, so
/// the game's draws are not moved.
pub(crate) fn press_draw(tick: u32, place: Place) -> f32 {
    let h = (tick ^ (place as u32).wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B);
    let h = h ^ (h >> 15);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The triangles of [`ledge_grid`]'s world (with `walker` on the ledge's
/// top), for the walker mesh's rays.
pub(crate) fn ledge_triangles(rise: f32, walker: mapdata::WalkerBehaviour) -> Vec<mapdata::Triangle> {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, rise).map(|mut t| {
        t.walker = walker;
        t
    }));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(rise));
        t.walker = walker;
        t
    }));
    triangles
}

/// A stand-in body of the honest stand-in at `at` on `grid`, facing east.
pub(crate) fn honest_body(grid: &mapdata::WalkGrid, at: Vec3) -> Body {
    let game = Game::new(soak_world(), &[0], 1, None, None);
    let mut body = game.bodies[0].clone();
    let node = grid.locate(at, 0.5).expect("a floor there");
    body.node = Some(node);
    body.feet = Vec3::new(at.x, grid.position(node).y, at.z);
    body.yaw = -std::f32::consts::FRAC_PI_2;
    body
}

/// `body` jumps with `keys` held at `speed` (m/s, the last tick's): where
/// it comes down, and the highest its feet got over the floor it left.
pub(crate) fn honest_jump(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    body: Body,
    keys: u32,
    speed: f32,
) -> (Vec3, f32) {
    honest_jump_in(grid, walls, None, body, keys, speed)
}

/// [`honest_jump`] with the walk before the jump held by the capsule rule (`wall`: its sections).
pub(crate) fn honest_jump_in(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    wall: Option<&super::standin_capsule::Sections>,
    mut body: Body,
    keys: u32,
    speed: f32,
) -> (Vec3, f32) {
    let start = 1_000;
    let (clip, dir) = jump_of(keys, body.yaw, speed, true);
    let u = press_draw(start, 0);
    let prepare = if std::ptr::eq(clip, &SPRINT_CLIP) {
        (SPRINT_PREPARE_MS * u, SPRINT_PREPARE_M * u)
    } else {
        (0.0, 0.0)
    };
    let base = body.feet.y;
    // The keys of the press stay held through the jump (a player's, a bot's carry: the air control acts on them), unless the caller set
    // the keys held in the air.
    if body.keys == 0 {
        body.keys = keys & !key::JUMP;
    }
    body.flight = Some(Flight {
        at: start,
        from: body.feet,
        dir,
        clip,
        prepare,
        base: None,
        fall: None,
        air: [0.0; 2],
        carry: [0.0; 2],
        arc: client_clip(keys, speed, true),
        speed,
        flown: None,
        kept_fall: false,
    });
    let mut top = 0.0f32;
    for k in 1..=80 {
        let (mut ticks, mut held) = (0, 0);
        let wall = wall.map(|sections| JumpWall { sections, hold: true, ticks: &mut ticks, held: &mut held });
        let heading = body.yaw;
        let landing = fly(grid, walls, wall, &mut body, 0, start + k * TICK_MS, FlyWalk { heading, walk: Vec3::ZERO });
        top = top.max(body.feet.y - base);
        if let Some(landing) = landing {
            assert_eq!(landing.to, body.feet);
            return (body.feet, top);
        }
    }
    panic!("no landing: {:?}", body.feet);
}

/// The honest stand-in's jumps against the game's: [A364] a short jump moves its direction clip x the carry of the press's speed
/// (`jump_arc::Clip::carry`: 1/4 of the clip from a standstill with a direction key) plus the air control (1 m/s along the keys,
/// `stand_in_air_control`) over its twelve air steps, and rises as its clip (1.0 m); in place 1.0 m up where it stood; a sprint jump at a run goes its clip's travel to the last air step (3.89 m, and the prepare phase's 0-1.64 m) and 1.28 m up;
/// forward with sprint too slow (under the walk cap) is the short jump. The old flight (`stand_in_client_jump` off) played the clip's
/// travel to its end.
#[test]
pub(crate) fn an_honest_jump_follows_the_games_clips() {
    let triangles = ledge_triangles(0.9, mapdata::WalkerBehaviour::BySlope);
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let from = Vec3::new(2.0, 0.0, 5.0);
    let body = honest_body(&grid, from);
    use crate::v026g::bots::jump_arc::{AIR_STEPS, Clip, Direction};
    let client = stand_in_client_jump();
    let control = if stand_in_air_control() { AIR_STEPS as f32 * TICK_S } else { 0.0 };
    // A short jump pressed walking at `speed` along its way: the clip's travel to the last air step x the carry, the control on top.
    let short = |speed: f32| {
        let clip = Clip::Short(Direction::Forward);
        if client { clip.root(clip.air_end_frame())[1] * clip.carry(speed) + control } else { clip.root(24.0)[1] }
    };
    let (to, top) = honest_jump(&grid, &walls, body.clone(), key::FORWARD | key::JUMP, 0.0);
    assert!((to.x - from.x - short(0.0)).abs() < 0.15 && to.y.abs() < 0.05, "{to:?} {}", short(0.0));
    assert!((top - 1.0).abs() < 0.03, "{top}");
    let (to, top) = honest_jump(&grid, &walls, body.clone(), key::JUMP, 0.0);
    assert!(to.distance_xz(from) < 0.05 && (top - 1.0).abs() < 0.03, "{to:?} {top}");
    let run = key::FORWARD | key::SPRINT | key::JUMP;
    let (to, top) = honest_jump(&grid, &walls, body.clone(), run, 5.0);
    let ahead = to.x - from.x;
    // [T3] The client's: before the take-off the walk cap (3.5 m/s, `max_speed` 0x2DC010; skipped only under the jump flag, A328)
    // holds the body behind the sprint clip's 5 m/s root, and with the root re-based every tick (A325g) it does not catch up: about
    // 0.4 m short of the clip's travel. The old flight chased it back.
    let sprint_clip = Clip::Sprint(Direction::Forward);
    let sprint = if client { sprint_clip.root(sprint_clip.air_end_frame())[1] } else { sprint_clip.root(24.0)[1] };
    let least = if client { sprint - 0.52 } else { 4.3 };
    assert!((least..=sprint + 1.64 + 0.15).contains(&ahead), "{to:?} {sprint}");
    assert!((top - 1.28).abs() < 0.03, "{top}");
    let (to, top) = honest_jump(&grid, &walls, body.clone(), run, 3.0);
    assert!((to.x - from.x - short(3.0)).abs() < 0.15 && (top - 1.0).abs() < 0.03, "{to:?} {top} {}", short(3.0));
    // To the side from a standstill: that way by a quarter of its clip's travel and the air control ([A364]; the old flight took
    // that way's whole clip travel).
    let (to, _) = honest_jump(&grid, &walls, body, key::LEFT | key::JUMP, 0.0);
    let left = Clip::of(key::LEFT, 0.0);
    let side = if client { left.root(left.air_end_frame())[2].abs() * left.carry(0.0) + control } else { left.root(24.0)[2].abs() };
    assert!(((to.z - from.z).abs() - side).abs() < 0.15 && to.distance_xz(from) < side + 0.15, "{to:?} {side}");
}

/// The air control ([`air_control`], the client's `get_air_control_vector`): 1.0 m/s along the keys, the diagonals normalised,
/// none with no key or with forward alone in a sprint jump (back still steers). With [`stand_in_air_control`] on, a short jump
/// pressed forward and steered to the side in the air comes down its airborne time (0.2-0.8 s of the clip) x 1.0 m/s to that
/// side and as far ahead; off, on its way.
#[test]
fn the_air_control_steers_a_jump_by_the_keys_held_in_the_air() {
    let yaw = 0.3;
    let forward = air_control(key::FORWARD, yaw, false);
    assert!((forward[0].hypot(forward[1]) - AIR_CONTROL_MS).abs() < 1e-5, "{forward:?}");
    assert_eq!(air_control(key::FORWARD, yaw, true), [0.0, 0.0]);
    assert_eq!(air_control(key::FORWARD | key::BACK, yaw, false), [0.0, 0.0]);
    let back = air_control(key::BACK, yaw, true);
    assert!((back[0] + forward[0]).abs() < 1e-5 && (back[1] + forward[1]).abs() < 1e-5, "{back:?}");
    let diagonal = air_control(key::FORWARD | key::LEFT, yaw, true);
    assert!((diagonal[0].hypot(diagonal[1]) - AIR_CONTROL_MS).abs() < 1e-5, "{diagonal:?}");

    let triangles = ledge_triangles(0.9, mapdata::WalkerBehaviour::BySlope);
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let from = Vec3::new(2.0, 0.0, 5.0);
    let mut body = honest_body(&grid, from);
    body.keys = key::LEFT;
    let land = |on| {
        force_air_control(Some(on));
        let (to, _) = honest_jump(&grid, &walls, body.clone(), key::FORWARD | key::JUMP, 0.0);
        force_air_control(None);
        to
    };
    let (off, on) = (land(false), land(true));
    assert!((off.z - from.z).abs() < 0.05, "{off:?}");
    let aside = (on.z - from.z).abs();
    assert!((0.5..0.8).contains(&aside) && (on.x - off.x).abs() < 0.05, "{on:?} {off:?}");
}

/// From a standstill at its face, a forward jump gets up a 0.9 m ledge
/// (held by the face, it rises with the clip and goes on once its feet are
/// over the edge); not up a 1.2 m one, which the sprint jump at a run
/// makes; never up one a press with no movement key meets.
#[test]
pub(crate) fn an_honest_jump_gets_up_what_its_clip_clears() {
    for (rise, standing, sprint) in [(0.9, true, true), (1.2, false, true), (1.45, false, false)] {
        let triangles = ledge_triangles(rise, mapdata::WalkerBehaviour::BySlope);
        let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
        let walls = mapdata::RayTree::build(&triangles);
        let at_face = honest_body(&grid, Vec3::new(9.5, 0.0, 5.0));
        let (to, _) = honest_jump(&grid, &walls, at_face.clone(), key::FORWARD | key::JUMP, 0.0);
        assert_eq!((to.y - rise).abs() < 0.05, standing, "{rise}: {to:?}");
        assert!(standing || (to.y.abs() < 0.05 && to.x < 10.0), "{rise}: {to:?}");
        let run_up = honest_body(&grid, Vec3::new(7.5, 0.0, 5.0));
        let keys = key::FORWARD | key::SPRINT | key::JUMP;
        let (to, _) = honest_jump(&grid, &walls, run_up, keys, 5.0);
        assert_eq!((to.y - rise).abs() < 0.05, sprint, "{rise} sprint: {to:?}");
        let (to, _) = honest_jump(&grid, &walls, at_face, key::JUMP, 0.0);
        assert!(to.y.abs() < 0.05, "{rise} in place: {to:?}");
    }
}

/// A jump off a ledge's edge comes down on the floor below (it falls
/// after its clip). [A364 P6] Pressed walking: a short jump goes as the press's walk.
#[test]
pub(crate) fn an_honest_jump_off_an_edge_falls_to_the_floor_below() {
    let triangles = ledge_triangles(3.0, mapdata::WalkerBehaviour::BySlope);
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let mut body = honest_body(&grid, Vec3::new(10.6, 3.0, 5.0));
    body.yaw = std::f32::consts::FRAC_PI_2;
    let (to, _) = honest_jump(&grid, &walls, body, key::FORWARD | key::JUMP, 3.5);
    assert!(to.y.abs() < 0.05 && to.x < 9.0, "{to:?}");
}

/// The client's fall damage by landing speed (`docs/notes/fall_damage.md`, `fatal_damage` 1.0): none up to 9.9 m/s, linear to 14 m/s, 1 beyond; a
/// fall of 8 m (v = 12.5 m/s) is about 0.64 and kills from full health (0.6), one of 5 m (9.9 m/s) hurts no one.
#[test]
pub(crate) fn the_fall_damage_follows_the_clients_rule_by_landing_speed() {
    assert_eq!(fall_damage(0.0), 0.0);
    assert!(fall_damage(9.89) == 0.0);
    assert!((fall_damage(10.8) - 0.22).abs() < 0.01, "{}", fall_damage(10.8));
    let eight_m = (2.0f32 * 9.8 * 8.0).sqrt();
    assert!((fall_damage(eight_m) - 0.64).abs() < 0.02, "{}", fall_damage(eight_m));
    assert!(fall_damage(eight_m) >= LETHAL_FALL_DAMAGE);
    assert_eq!(fall_damage(14.0), 1.0);
    assert_eq!(fall_damage(30.0), 1.0);
}

/// The honest walk steps up to the grid body's step (0.6 m on this test's default body, the exe's `s_step_height`), and not up the grid's
/// "forced" stair links of up to 0.75 m (`walk` faces) that the old
/// walk took.
#[test]
pub(crate) fn the_honest_walk_steps_no_higher_than_the_client() {
    let walk_east = |grid: &mapdata::WalkGrid, capped: bool| {
        let mut node = grid.locate(Vec3::new(8.0, 0.0, 5.0), 0.5).unwrap();
        let mut feet = grid.position(node);
        for _ in 0..40 {
            let to = feet + Vec3::new(0.2, 0.0, 0.0);
            (node, feet, _) = if capped {
                slide_capped(grid, node, feet, to, grid.body().step)
            } else {
                slide(grid, node, feet, to)
            };
        }
        feet
    };
    let forced = |rise: f32| {
        let triangles = ledge_triangles(rise, mapdata::WalkerBehaviour::Walk);
        let mut triangles = triangles;
        // The floor before the step forced too: the grid's stair rule
        // links two forced floors.
        for t in triangles.iter_mut().take(2) {
            t.walker = mapdata::WalkerBehaviour::Walk;
        }
        mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    };
    let low = forced(0.5);
    assert!((walk_east(&low, true).y - 0.5).abs() < 0.05);
    let high = forced(0.7);
    let old = walk_east(&high, false);
    assert!((old.y - 0.7).abs() < 0.05, "the grid links a 0.7 m forced step: {old:?}");
    let honest = walk_east(&high, true);
    assert!(honest.y.abs() < 0.05 && honest.x < 10.0, "{honest:?}");
}

/// Two blocks (z 0-10) with tops `a` and `b` high, the gap between them
/// from x 6 to `6 + gap`, over ground at `ground`: the triangles.
pub(crate) fn gap_triangles(a: f32, b: f32, gap: f32, ground: f32) -> Vec<mapdata::Triangle> {
    let walker = mapdata::WalkerBehaviour::BySlope;
    let quad = |p: [Vec3; 4]| {
        [
            mapdata::Triangle { v: [p[0], p[1], p[2]], walker },
            mapdata::Triangle { v: [p[0], p[2], p[3]], walker },
        ]
    };
    let top = |x0: f32, x1: f32, y: f32| {
        quad([
            Vec3::new(x0, y, 0.0),
            Vec3::new(x1, y, 0.0),
            Vec3::new(x1, y, 10.0),
            Vec3::new(x0, y, 10.0),
        ])
    };
    let face = |x: f32, y0: f32, y1: f32| {
        quad([
            Vec3::new(x, y0, 0.0),
            Vec3::new(x, y0, 10.0),
            Vec3::new(x, y1, 10.0),
            Vec3::new(x, y1, 0.0),
        ])
    };
    let mut t = top(0.0, 14.0, ground).to_vec();
    t.extend(top(0.0, 6.0, a));
    t.extend(face(6.0, ground, a));
    t.extend(top(6.0 + gap, 14.0, b));
    t.extend(face(6.0 + gap, ground, b));
    t
}

/// The grid's gap links (`mapdata::grid::gap`) east across a gap are
/// made by the honest stand-in's jump: walking at the walk cap, pressed
/// at the link's own point (and the spread earlier or later), with
/// forward held, it comes down on the link's landing level.
#[test]
pub(crate) fn the_grids_gap_links_are_made_by_the_honest_jump() {
    use mapdata::grid::gap::GAP_PRESS_SPREAD_M;
    for (a, b, gap, ground) in [(2.0, 2.0, 1.0, 0.0), (2.0, 2.6, 0.75, 0.0), (2.0, 0.5, 1.5, -3.0)] {
        let mut triangles = gap_triangles(a, b, gap, ground);
        let mut area = [0.0, 0.0, 14.0, 10.0];
        if b < a - 0.5 {
            // A drop is a link only with a way back up (a one-way drop would keep the yard as reached), and a long
            // one, or the links save no walk round: out of the near top north, 20 m on, east, and down a 45 degree
            // ramp onto the far block's north edge.
            let walker = mapdata::WalkerBehaviour::BySlope;
            let plate = |p: [Vec3; 4]| {
                [
                    mapdata::Triangle { v: [p[0], p[1], p[2]], walker },
                    mapdata::Triangle { v: [p[0], p[2], p[3]], walker },
                ]
            };
            let flat = |x0: f32, z0: f32, x1: f32, z1: f32| {
                plate([Vec3::new(x0, a, z0), Vec3::new(x1, a, z0), Vec3::new(x1, a, z1), Vec3::new(x0, a, z1)])
            };
            triangles.extend(flat(1.0, 10.0, 3.0, 30.0));
            triangles.extend(flat(1.0, 28.0, 13.0, 30.0));
            triangles.extend(flat(11.0, 11.5, 13.0, 30.0));
            triangles.extend(plate([
                Vec3::new(11.0, b, 10.0),
                Vec3::new(13.0, b, 10.0),
                Vec3::new(13.0, a, 11.5),
                Vec3::new(11.0, a, 11.5),
            ]));
            area = [0.0, 0.0, 14.0, 32.0];
        }
        let grid = mapdata::WalkGrid::build_with_links(
            &triangles,
            area,
            0.25,
            mapdata::Body::default(),
            mapdata::OffMeshLinks::Gaps,
        );
        let walls = mapdata::RayTree::build(&triangles);
        let east: Vec<_> = grid
            .off_links()
            .iter()
            .filter(|l| {
                let (p, q) = (grid.position(l.from), grid.position(l.to));
                p.x < 6.0 && (q.z - p.z).abs() < 0.01 && (3.0..7.0).contains(&p.z)
            })
            .copied()
            .collect();
        assert!(!east.is_empty(), "{a} {b} {gap}: no gap link east");
        for l in east {
            let (from, to) = (grid.position(l.from), grid.position(l.to));
            // A walk-off has no press to fly (its own test: `gap.rs`).
            let mapdata::grid::gap::Takeoff::Press(press) = l.press else {
                continue;
            };
            let press = press.metres();
            for lead in [press - GAP_PRESS_SPREAD_M, press, press + GAP_PRESS_SPREAD_M] {
                let body = honest_body(&grid, Vec3::new(from.x - lead, from.y, from.z));
                let (down, _) = honest_jump(&grid, &walls, body, key::FORWARD | key::JUMP, WALK_CAP);
                assert!((down.y - to.y).abs() < 0.1, "{a} {b} {gap} lead {lead}: {from:?} -> {to:?} came down at {down:?}");
            }
        }
    }
}

/// [`stand_in_walk_off`]'s fall: a body that leaves a 2 m top's edge at a walk (3.5 m/s) keeps it across while it falls and comes down
/// on a walkway 1.1 m lower that starts 0.9 m out (J0 g2a's bridge: the walk-off lands on it); with no speed kept it comes down at the
/// foot of the face, on the ground.
#[test]
fn a_walk_off_falls_on_at_its_walk_speed() {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, 2.0));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(2.0));
        t
    }));
    triangles.extend(floor(6.0, 0.0, 8.6, 10.0, 0.9));
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let fall = |carry: [f32; 2]| {
        let mut body = honest_body(&grid, Vec3::new(10.2, 2.0, 5.0));
        // Past the top's floors by more than the body's radius: it is falling.
        body.feet = Vec3::new(9.5, 2.0, 5.0);
        body.flight = Some(Flight {
            at: 1_000,
            from: body.feet,
            dir: [0.0, 0.0],
            clip: &SHORT_CLIP,
            prepare: (0.0, 0.0),
            base: Some(2.0),
            fall: Some(0.0),
            air: [0.0; 2],
            carry,
            arc: None,
            speed: 0.0,
            flown: None,
            kept_fall: false,
        });
        let heading = body.yaw;
        (1..=100).find_map(|k| fly(&grid, &walls, None, &mut body, 0, 1_000 + k * TICK_MS, FlyWalk { heading, walk: Vec3::ZERO })).map(|landing| landing.to)
    };
    let carried = fall([-3.5, 0.0]).expect("it comes down");
    assert!((carried.y - 0.9).abs() < 0.05 && carried.x < 8.6, "kept its walk: on the walkway: {carried:?}");
    let straight = fall([0.0, 0.0]).expect("it comes down");
    assert!(straight.y.abs() < 0.05, "no speed kept: on the ground: {straight:?}");
}

/// [`past_edge`]: a body past the top's last floor at a ledge walks on while that floor is within [`body::SUPPORT_M`] of where it
/// steps, and comes down on the floor below once it is not, with no drop link asked; at the foot of the face (a wall) nothing changes.
#[test]
fn a_body_past_a_ledges_edge_walks_on_within_the_support_and_falls_past_it() {
    use super::body::tests::{floor, wall_x};
    // The ground at 0, a top 2 m high from x 10 on, its face at x 10.
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, 2.0));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(2.0));
        t
    }));
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    // The top's last floor toward the face along z 5.
    let last = (0..grid.len() as NodeId)
        .filter(|&n| grid.position(n).y > 1.9 && (grid.position(n).z - 5.0).abs() < 0.2)
        .min_by(|&a, &b| grid.position(a).x.total_cmp(&grid.position(b).x))
        .unwrap();
    let edge = grid.position(last);
    let feet = Vec3::new(edge.x - 0.3, edge.y, edge.z);
    let on = past_edge(&grid, &walls, last, feet, Vec3::new(edge.x - 0.4, edge.y, edge.z));
    assert!(on.is_some_and(|(n, f)| n == last && (f.y - edge.y).abs() < 1e-3), "within the support it walks on: {on:?}");
    let down = past_edge(&grid, &walls, last, feet, Vec3::new(edge.x - 0.6, edge.y, edge.z));
    assert!(down.is_some_and(|(_, f)| f.y < 0.5), "past the support it comes down: {down:?}");
    // At the face's foot, walking into it: the wall stops it.
    let foot = (0..grid.len() as NodeId)
        .filter(|&n| grid.position(n).y < 0.1 && (grid.position(n).z - 5.0).abs() < 0.2 && grid.position(n).x < 10.0)
        .max_by(|&a, &b| grid.position(a).x.total_cmp(&grid.position(b).x))
        .unwrap();
    let at = grid.position(foot);
    assert_eq!(past_edge(&grid, &walls, foot, at, at + Vec3::new(0.2, 0.0, 0.0)), None);
}

/// Ground at 0 for x under 6 and a block from x 6 to 14 whose top is `top` high (z 0-10, its face at x 6 a wall), with the client's
/// numbers (`Body::client`, step 0.80, the exe's collision) and its walker: the grid.
fn face_grid(top: f32) -> mapdata::WalkGrid {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 6.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(6.0, 0.0, 14.0, 10.0, top));
    triangles.extend(wall_x(6.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(top));
        t
    }));
    // The live grids' caster (the exe's collision, `Tuning::grid_body`): the controller's climb needs it.
    let body = mapdata::Body { exe_collision: true, start_witness: true, ..mapdata::Body::client() };
    mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 14.0, 10.0], 0.25, body).with_client_walker(&triangles)
}

/// [stand-in fidelity] [`kept_climb`]: a body run east into a face by its kept controller, the lattice holding it at the face's foot
/// (as `walk_slide` does over the client step): at a sprint the controller climbs a 0.95 m edge and the body goes on the top's floor;
/// at a walk it is held at the foot. A 1.4 m face (over `jump::SPRINT_CLIMB_MAX_M`) is climbed by neither.
#[test]
fn a_kept_controllers_sprint_climb_takes_the_stand_in_up() {
    use crate::v026g::bots::body;
    let run = |grid: &mapdata::WalkGrid, sprinting: bool| {
        let (cap, keys) = if sprinting { (5.0, key::FORWARD | key::SPRINT) } else { (3.5, key::FORWARD) };
        let mut kept = None;
        let mut feet = Vec3::new(3.0, 0.0, 5.0);
        for _ in 0..40 {
            let by = Vec3::new(cap * TICK_S, 0.0, 0.0);
            let Some(step) = body::kept_step(grid, &mut kept, feet, by, body::crouch_input(keys, false), sprinting, body::kept_clock::Of::StandIn)
            else {
                return feet;
            };
            // The lattice: on the ground's floors, held at the face's foot.
            let walked = if feet.y < 0.5 { Vec3::new(step.to.x.min(6.0 - 0.38), feet.y, step.to.z) } else { Vec3::new(step.to.x, feet.y, step.to.z) };
            let top = kept.map_or(feet.y, |kept: body::KeptBody| kept.height());
            feet = match kept_climb(grid, step.to, top, walked, step.airborne) {
                Some((_, at)) => at,
                None => walked,
            };
            if feet.x > 6.0 + 0.5 {
                break;
            }
        }
        feet
    };
    let low = face_grid(0.95);
    let up = run(&low, true);
    assert!((up.y - 0.95).abs() < 0.05 && up.x > 6.0, "a sprint climbs 0.95 m: {up:?}");
    let held = run(&low, false);
    assert!(held.y.abs() < 0.05 && held.x < 6.0, "a walk is held at the foot: {held:?}");
    let high = run(&face_grid(1.4), true);
    assert!(high.y.abs() < 0.05 && high.x < 6.0, "1.4 m is not climbed: {high:?}");
}

/// [stand-in fidelity] [`kept_climb`] climbs nothing for a controller in the air (the combined bench's B-tdm-m6-s2 bot 10): a body
/// run east at a sprint off a 2 m top down a 45 degree ramp, the lattice walking it down the ramp's floors (`walk_slide`) while its
/// kept controller is in the air over them, goes on down to the ground beyond, with no climb on the way; and at one step past the
/// top's edge with the controller over the lattice's walk, the climb is made on the ground (as before) and not in the air.
#[test]
fn a_kept_controller_in_the_air_climbs_nothing() {
    use super::body::tests::floor;
    use crate::v026g::bots::body;
    let walker = mapdata::WalkerBehaviour::BySlope;
    let (a, b, c, d) = (Vec3::new(6.0, 2.0, 0.0), Vec3::new(8.0, 0.0, 0.0), Vec3::new(8.0, 0.0, 10.0), Vec3::new(6.0, 2.0, 10.0));
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 6.0, 10.0, 2.0).to_vec();
    triangles.extend([mapdata::Triangle { v: [a, b, c], walker }, mapdata::Triangle { v: [a, c, d], walker }]);
    triangles.extend(floor(8.0, 0.0, 14.0, 10.0, 0.0));
    let shape = mapdata::Body { exe_collision: true, start_witness: true, ..mapdata::Body::client() };
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 14.0, 10.0], 0.25, shape).with_client_walker(&triangles);
    // The stand-in's ground tick at the kept controller's level: the lattice's walk to the controller's end, then the climb.
    let run = || {
        let mut kept = None;
        let mut feet = Vec3::new(4.0, 2.0, 5.0);
        let mut node = grid.locate(feet, 0.5).expect("the top's floor");
        let (mut airborne_steps, mut air_climbs) = (0, 0);
        for _ in 0..30 {
            let by = Vec3::new(5.0 * TICK_S, 0.0, 0.0);
            let keys = body::crouch_input(key::FORWARD | key::SPRINT, false);
            let Some(step) = body::kept_step(&grid, &mut kept, feet, by, keys, true, body::kept_clock::Of::StandIn) else {
                break;
            };
            let (slid, walked, _) = super::standin::walk_slide(&grid, node, feet, step.to);
            let top = kept.map_or(feet.y, |kept: body::KeptBody| kept.height());
            airborne_steps += usize::from(step.airborne);
            (node, feet) = match kept_climb(&grid, step.to, top, walked, step.airborne) {
                Some(up) => {
                    air_climbs += usize::from(step.airborne);
                    up
                }
                None => (slid, walked),
            };
        }
        (feet, airborne_steps, air_climbs)
    };
    let (down, airborne, air_climbs) = run();
    assert!(airborne > 0, "the controller leaves the ground over the ramp");
    assert_eq!(air_climbs, 0);
    assert!(down.x > 8.0 && down.y < 0.1, "down the ramp to the ground: {down:?}");
    // The rule at one step just past the top's edge, the controller 0.18 m over the lattice's walk down the ramp (bot 10's ticks
    // had it 0.4-0.5 m over): on the ground, a climb onto the floor at its height, as before; in the air, none.
    let (to, top) = (Vec3::new(6.1, 1.9, 5.0), 2.08);
    let walked = Vec3::new(6.1, 1.9, 5.0);
    assert!(kept_climb(&grid, to, top, walked, false).is_some(), "on the ground: the climb, as before");
    assert_eq!(kept_climb(&grid, to, top, walked, true), None, "in the air: no climb");
}

/// [stand-in fidelity] [`stand_in_kept_fall`]: a body walking east off a 2 m top's edge by its kept controller falls by it with the
/// walk held (the client's walk tick in the air) and comes down on the ground beyond the edge, its feet at the ground's height.
#[test]
fn a_walk_off_falls_by_the_kept_controller() {
    use super::body::tests::{floor, wall_x};
    use crate::v026g::bots::body;
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(0.0, 0.0, 10.0, 10.0, 2.0));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(2.0));
        t
    }));
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::client()).with_client_walker(&triangles);
    let walls = mapdata::RayTree::build(&triangles);
    let mut body = honest_body(&grid, Vec3::new(8.0, 2.0, 5.0));
    let by = Vec3::new(3.5 * TICK_S, 0.0, 0.0);
    // Walk east by the kept controller until its step leaves the ground.
    let mut left = None;
    for _ in 0..40 {
        let step = body::kept_step(&grid, &mut body.kept, body.feet, by, body::crouch_input(key::FORWARD, false), false, body::kept_clock::Of::StandIn)
            .expect("a kept controller on the walker");
        if step.airborne {
            left = Some(step);
            break;
        }
        body.feet = Vec3::new(step.to.x, body.kept.map_or(body.feet.y, |kept| kept.height()), step.to.z);
    }
    let step = left.expect("it walks off the edge");
    body.feet = Vec3::new(step.to.x, body.kept.map_or(body.feet.y, |kept| kept.height()), step.to.z);
    body.keys = key::FORWARD;
    body.flight = Some(Flight {
        at: 1_000,
        from: body.feet,
        dir: [0.0, 0.0],
        clip: &SHORT_CLIP,
        prepare: (0.0, 0.0),
        base: Some(2.0),
        fall: Some(0.0),
        air: [0.0; 2],
        carry: [0.0; 2],
        arc: None,
        speed: 0.0,
        flown: None,
        kept_fall: true,
    });
    let heading = body.yaw;
    let landing = (1..=100).find_map(|k| fly(&grid, &walls, None, &mut body, 0, 1_000 + k * TICK_MS, FlyWalk { heading, walk: by })).expect("it comes down");
    assert!(landing.to.y.abs() < 0.05 && landing.to.x > 10.0, "down on the ground past the edge: {:?}", landing.to);
    assert!(body.kept.is_some(), "the controller kept on the ground");
}
