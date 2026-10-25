//! Research (mode 5, capture points): the rule's logic, apart from the game.
//!
//! 0.27d's `capture_the_point_rule_core` and `capturable_zone_core`, written
//! again on integers so that every game computes the same bytes
//! (`docs/notes/research_mode.md`). The client's glue
//! (`client-patches/research_arena`, exe patch 53) reads the players out of
//! the world and the parameters out of the mode config; this crate holds no
//! pointer into the game, keeps no static storage and is tested on the host
//! (`cargo test -p research-rule`).
//!
//! `no_std`, no allocation. The types are `#[repr(C)]` where the game's
//! memory holds them (the rule's parameters and state live in the rule the
//! game allocates; `check_research_026g.py` reads them by offset), and the
//! state's bytes on the wire are [`write`]'s and [`read`]'s. Arithmetic that
//! can pass 32 bits wraps, as the first build's C did.

#![no_std]

use core::mem::{MaybeUninit, offset_of, size_of};

/// Roster places a world keeps (late join: always 20).
pub const PLAYERS: usize = 20;
/// Research units of a map (0.27d: `cp_id` 0, 1, 2).
pub const MAX_ZONES: usize = 3;
/// Collision meshes of one unit's geometry.
pub const MAX_SHAPES: usize = 8;
/// `capture_time_in_s`: 1..8 players.
pub const MAX_CAPTURE: usize = 8;
/// `resource_per_s`: 1..3 units owned.
pub const MAX_OWNED: usize = 3;
/// A whole capture, in progress units.
pub const FULL: u32 = 1_000_000_000;
/// Points are kept in millionths.
pub const MICRO: u32 = 1_000_000;
/// `points_rate` is per this many ms (`resource_per_s` x 1.25 is whole there).
pub const RATE_MS: u32 = 4;
/// The most ms one tick may move the rule.
pub const MAX_DT: u32 = 10_000;
/// The most awards one tick gives: two shares of every player at every unit, and a kill.
pub const MAX_AWARDS: usize = 2 * MAX_ZONES * PLAYERS + 1;
/// A unit's bytes on the wire.
pub const ZONE_BYTES: usize = 4 + 1 + 1 + 2 + 8 + 4 * PLAYERS;
/// The state's bytes on the wire before its units.
pub const HEAD_BYTES: usize = 8 + 4 + 4 + 4 + 1;
/// "Nobody" in a team's byte (a unit's owner or capturer, the statistics'
/// winner, the wire): `Option<Team>`'s `None`.
pub const NOBODY: u8 = 2;

/// A team of the two.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Zero = 0,
    One = 1,
}

// `Option<Team>` is the byte the game's memory and the wire hold: 0, 1, or
// 2 for nobody (the layout check_research_026g.py reads).
const _: () = assert!(size_of::<Option<Team>>() == 1);
const _: () = assert!(unsafe { core::mem::transmute::<Option<Team>, u8>(None) } == NOBODY);

impl Team {
    pub const BOTH: [Team; 2] = [Team::Zero, Team::One];

    /// A team value of the game's (a profile's): 0 or 1, anything else none.
    pub fn from_raw(raw: u32) -> Option<Team> {
        match raw {
            0 => Some(Team::Zero),
            1 => Some(Team::One),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// The byte of a team or nobody (the statistics' winner, the wire).
pub fn team_byte(team: Option<Team>) -> u8 {
    team.map_or(NOBODY, |t| t as u8)
}

/// A team's byte as the wire carries it: 0, 1, 2 (nobody); `Err` otherwise.
fn parse_team_byte(byte: u8) -> Result<Option<Team>, ()> {
    match byte {
        0 => Ok(Some(Team::Zero)),
        1 => Ok(Some(Team::One)),
        NOBODY => Ok(None),
        _ => Err(()),
    }
}

/// A roster place, 0..20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub fn new(raw: u8) -> Option<Place> {
        (usize::from(raw) < PLAYERS).then_some(Place(raw))
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// A set of roster places (bit i: place i).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Roster(pub u32);

impl Roster {
    pub fn has(self, place: usize) -> bool {
        self.0 >> place & 1 != 0
    }

    pub fn len(self) -> u32 {
        let (mut mask, mut count) = (self.0, 0);
        while mask != 0 {
            mask &= mask - 1;
            count += 1;
        }
        count
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The time the match ends at, ms (the goal reached, plus 3 s): none until then.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndTime(u32);

impl EndTime {
    /// Not set (its bytes: FFFFFFFFh).
    pub const NONE: EndTime = EndTime(u32::MAX);

    pub fn get(self) -> Option<u32> {
        (self.0 != u32::MAX).then_some(self.0)
    }

    /// The bytes as the wire carries them (FFFFFFFFh none).
    pub fn from_raw(raw: u32) -> EndTime {
        EndTime(raw)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

/// A shape of a unit's zone, as the `collision_geometry` meshes of 0.27d's
/// (and 0.31a2's London) units.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// `half[0]` the radius.
    Sphere = 0,
    Box = 1,
    /// `half[0]` the radius, `half[1]` the half height about its local y axis.
    Cylinder = 2,
}

impl ShapeKind {
    pub fn from_raw(raw: u32) -> Option<ShapeKind> {
        match raw {
            0 => Some(ShapeKind::Sphere),
            1 => Some(ShapeKind::Box),
            2 => Some(ShapeKind::Cylinder),
            _ => None,
        }
    }
}

/// A shape in world space: centre, the three local axes (unit rows: local =
/// axes * (p - centre)), half extents.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    pub centre: [f32; 3],
    pub axes: [[f32; 3]; 3],
    pub half: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneDef {
    pub cp_id: u32,
    pub shape_count: u32,
    /// The unit's object position (hud, minimap).
    pub position: [f32; 3],
    pub shapes: [Shape; MAX_SHAPES],
}

/// From the mode config (`mode.parameters`), never serialized.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Entries of `capture_ms`.
    pub capture_count: u32,
    /// A whole capture by n players, ms.
    pub capture_ms: [u32; MAX_CAPTURE],
    /// `FULL / capture_ms`: units per ms ([`prepare`]).
    pub capture_rate: [u32; MAX_CAPTURE],
    /// `capture_rate[0]` x the restore multiplier ([`prepare`]).
    pub restore_rate: u32,
    pub owned_count: u32,
    /// Millionths of a point per [`RATE_MS`] ms, k units owned.
    pub points_rate: [u32; MAX_OWNED],
    /// `resources_count` in millionths.
    pub target: u32,
    /// Points shared at a capture or neutralisation.
    pub award: u32,
    pub zone_count: u32,
    pub zones: [ZoneDef; MAX_ZONES],
    /// A player's capsule radius, m.
    pub reach: f32,
    /// Half a capsule's height, m, when the game gives none.
    pub half_height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zone {
    /// 0..FULL of `capturing`'s capture.
    pub progress: u32,
    /// Whose progress it is.
    pub capturing: Option<Team>,
    pub owner: Option<Team>,
    /// Players of each team inside.
    pub count: [u8; 2],
    /// Those players.
    pub present: [Roster; 2],
    /// Each player's share at entry, FULL = all.
    pub marks: [u32; PLAYERS],
}

/// The rule's whole state: what a world saves and loads.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    /// Millionths, 0..target.
    pub points: [u32; 2],
    /// Points shown, 0..resources_count.
    pub score: [u16; 2],
    pub end: EndTime,
    /// Roster places entered and not left.
    pub entered: Roster,
    pub zone_count: u32,
    pub zones: [Zone; MAX_ZONES],
}

// The layouts research_native's C had (check_research_026g.py reads them).
const _: () = assert!(size_of::<Shape>() == 64 && size_of::<ZoneDef>() == 0x214);
const _: () = assert!(offset_of!(Params, zones) == 0x64 && size_of::<Params>() == 0x6A8);
const _: () = assert!(size_of::<Zone>() == 0x60 && offset_of!(Zone, present) == 8);
const _: () = assert!(offset_of!(State, end) == 12 && offset_of!(State, zones) == 0x18);
const _: () = assert!(size_of::<State>() == 0x138);

impl Params {
    /// All bytes zero (as the rule's memory is before the config is read).
    pub const ZERO: Params = Params {
        capture_count: 0,
        capture_ms: [0; MAX_CAPTURE],
        capture_rate: [0; MAX_CAPTURE],
        restore_rate: 0,
        owned_count: 0,
        points_rate: [0; MAX_OWNED],
        target: 0,
        award: 0,
        zone_count: 0,
        zones: [ZoneDef {
            cp_id: 0,
            shape_count: 0,
            position: [0.0; 3],
            shapes: [Shape { kind: ShapeKind::Sphere, centre: [0.0; 3], axes: [[0.0; 3]; 3], half: [0.0; 3] }; MAX_SHAPES],
        }; MAX_ZONES],
        reach: 0.0,
        half_height: 0.0,
    };
}

impl State {
    /// All bytes zero (as the rule's memory is before [`reset`]).
    pub const ZERO: State = State {
        points: [0; 2],
        score: [0; 2],
        end: EndTime(0),
        entered: Roster(0),
        zone_count: 0,
        zones: [Zone {
            progress: 0,
            capturing: Some(Team::Zero),
            owner: Some(Team::Zero),
            count: [0; 2],
            present: [Roster(0); 2],
            marks: [0; PLAYERS],
        }; MAX_ZONES],
    };
}

/// One player as the world has him this tick, alive, in the world and of a
/// team: his capsule stands on `feet` (the player's transform:
/// research_mode.md "Inside") and is 2 x `half_height` tall,
/// `Params::reach` round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub team: Team,
    pub feet: [f32; 3],
    /// Half the capsule's height (0.9 standing).
    pub half_height: f32,
}

/// What a tick or an event asks the game to count (statistics events).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardKind {
    Capture,
    Neutralise,
    KillInvader,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Award {
    pub kind: AwardKind,
    pub player: Place,
    /// Index into the zones.
    pub zone: u8,
    pub team: Team,
    /// Points of `award`, or 1 for a kill.
    pub amount: u32,
}

/// The awards of a tick, in the order given.
pub struct Awards {
    list: [MaybeUninit<Award>; MAX_AWARDS],
    len: usize,
}

impl Default for Awards {
    fn default() -> Self {
        Self::new()
    }
}

impl Awards {
    pub fn new() -> Awards {
        Awards { list: [const { MaybeUninit::uninit() }; MAX_AWARDS], len: 0 }
    }

    fn push(&mut self, award: Award) {
        if let Some(slot) = self.list.get_mut(self.len) {
            slot.write(award);
            self.len += 1;
        }
    }

    pub fn as_slice(&self) -> &[Award] {
        // SAFETY: the first `len` were written by push.
        unsafe { core::slice::from_raw_parts(self.list.as_ptr().cast::<Award>(), self.len) }
    }
}

/// Bytes of the state on the wire.
pub fn serialized_size(state: &State) -> usize {
    HEAD_BYTES + state.zone_count as usize * ZONE_BYTES
}

/// Fills `capture_rate` and `restore_rate` from `capture_ms` and a restore
/// multiplier in thousandths; false when the parameters cannot be played.
pub fn prepare(params: &mut Params, restore_permille: u32) -> bool {
    let capture = params.capture_count as usize;
    let owned = params.owned_count as usize;
    let zones = params.zone_count as usize;
    if capture == 0
        || capture > MAX_CAPTURE
        || owned == 0
        || owned > MAX_OWNED
        || zones == 0
        || zones > MAX_ZONES
        || params.target == 0
        || restore_permille > 1000
    {
        return false;
    }
    for i in 0..capture {
        let ms = params.capture_ms[i];
        if !(100..=3_600_000).contains(&ms) {
            return false;
        }
        params.capture_rate[i] = FULL / ms;
    }
    if params.points_rate[..owned].iter().any(|&rate| rate == 0 || rate > params.target) {
        return false;
    }
    if params.zones[..zones].iter().any(|z| z.shape_count == 0 || z.shape_count as usize > MAX_SHAPES) {
        return false;
    }
    let rate = params.capture_rate[0];
    params.restore_rate = rate / 1000 * restore_permille + rate % 1000 * restore_permille / 1000;
    true
}

/// A fresh state for these parameters.
pub fn reset(state: &mut State, params: &Params) {
    *state = State::ZERO;
    state.end = EndTime::NONE;
    state.zone_count = params.zone_count;
    for zone in state.zones.iter_mut().take(params.zone_count as usize) {
        zone.capturing = None;
        zone.owner = None;
        zone.marks = [FULL; PLAYERS];
    }
}

fn absolute(value: f32) -> f32 {
    if value < 0.0 { -value } else { value }
}

fn positive(value: f32) -> f32 {
    if value > 0.0 { value } else { 0.0 }
}

/// The square root as the first build took it: `sqrtss`.
fn root(value: f32) -> f32 {
    let mut out = value;
    // SAFETY: sqrtss on a register: no memory, no flags.
    unsafe { core::arch::asm!("sqrtss {0}, {0}", inout(xmm_reg) out, options(pure, nomem, nostack)) };
    out
}

/// The squared distance from a point to a shape (0 inside it). Every float
/// operation in the first build's order (single precision, SSE, no fused
/// multiply-add), so that every game gets the same bits.
fn distance2(shape: &Shape, p: &[f32; 3]) -> f32 {
    let d = [p[0] - shape.centre[0], p[1] - shape.centre[1], p[2] - shape.centre[2]];
    let mut l = [0.0f32; 3];
    for axis in 0..3 {
        let a = &shape.axes[axis];
        l[axis] = a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
    }
    match shape.kind {
        ShapeKind::Sphere => {
            let out = positive(root(l[0] * l[0] + l[1] * l[1] + l[2] * l[2]) - shape.half[0]);
            out * out
        }
        ShapeKind::Box => {
            let x = positive(absolute(l[0]) - shape.half[0]);
            let y = positive(absolute(l[1]) - shape.half[1]);
            let z = positive(absolute(l[2]) - shape.half[2]);
            x * x + y * y + z * z
        }
        ShapeKind::Cylinder => {
            let radial = positive(root(l[0] * l[0] + l[2] * l[2]) - shape.half[0]);
            let axial = positive(absolute(l[1]) - shape.half[1]);
            radial * radial + axial * axial
        }
    }
}

/// 0.27d's test is the physics' overlap of the unit's trigger shapes and the
/// player's capsule (capturable_zone_core's enter 0x324350 gets the colliding
/// objects' owners). Here: the capsule's segment, vertical, from reach above
/// the feet to reach below its top, and the shape: the nearest they come
/// (the distance along the segment is convex: a ternary search) is at most
/// the capsule's radius. Bullet's collision margin is not counted.
#[inline(never)]
fn inside_shape(params: &Params, shape: &Shape, feet: &[f32; 3], half_height: f32) -> bool {
    let r = params.reach;
    let mut lo = feet[1] + r;
    let mut hi = feet[1] + 2.0 * half_height - r;
    if hi < lo {
        lo = feet[1] + half_height;
        hi = lo;
    }
    let mut p = [feet[0], lo, feet[2]];
    let mut i = 0;
    while i < 40 && hi - lo > 0.0005 {
        let a = lo + (hi - lo) / 3.0;
        let b = hi - (hi - lo) / 3.0;
        p[1] = a;
        let da = distance2(shape, &p);
        p[1] = b;
        let db = distance2(shape, &p);
        if da <= db {
            hi = b;
        } else {
            lo = a;
        }
        i += 1;
    }
    p[1] = (lo + hi) / 2.0;
    distance2(shape, &p) <= r * r
}

/// Whether a capsule standing on `feet`, 2 x `half_height` tall and
/// `params.reach` round, overlaps one of the zone's shapes.
pub fn inside(params: &Params, zone: usize, feet: &[f32; 3], half_height: f32) -> bool {
    let def = &params.zones[zone];
    def.shapes.iter().take(def.shape_count as usize).any(|shape| inside_shape(params, shape, feet, half_height))
}

/// Marks count in 1/65536ths of a capture in [`share`] (`FULL >> 14` is
/// 61035) so that every product fits 32 bits.
const MARK_SHIFT: u32 = 14;
const MARK_ONE: u32 = FULL >> MARK_SHIFT;

/// `amount` shared over the players of `mask` as 0.27d's handler of a
/// capture event (capture_the_point_rule_core 0x31E920) water-fills it:
/// while any mark is left, the smallest one m is taken off every mark still
/// above zero, and each of those c players gets m / c of `amount`; each
/// share is then rounded (+0.5, down). So the capture is split, moment by
/// moment, among those who were there, and the whole amount goes out when
/// one was there from the start (mark 1).
fn share(zone: &Zone, mask: Roster, amount: u32, kind: AwardKind, index: u8, team: Team, out: &mut Awards) {
    if mask.is_empty() || amount == 0 || amount > 4000 {
        return;
    }
    let mut mark = [0u32; PLAYERS];
    let mut sum = [0u32; PLAYERS];
    for i in 0..PLAYERS {
        if mask.has(i) {
            mark[i] = zone.marks[i] >> MARK_SHIFT;
        }
    }
    loop {
        let mut least = 0;
        for &m in &mark {
            if m != 0 && (least == 0 || m < least) {
                least = m;
            }
        }
        if least == 0 {
            break;
        }
        let count = mark.iter().filter(|&&m| m != 0).count() as u32;
        let each = least * amount / count;
        for i in 0..PLAYERS {
            if mark[i] != 0 {
                sum[i] += each;
                mark[i] -= least;
            }
        }
    }
    for (i, &total) in sum.iter().enumerate() {
        let given = (total + MARK_ONE / 2) / MARK_ONE;
        if given != 0 {
            let player = Place(i as u8);
            out.push(Award { kind, player, zone: index, team, amount: given });
        }
    }
}

/// 0.27d's capturable_zone_core enter: the part of the capture still to
/// come when the player came in: for the team taking it 1 - progress, for
/// the other team (who must drain it first) the progress.
fn mark_of(zone: &Zone, team: Team) -> u32 {
    if zone.capturing == Some(team) { FULL.wrapping_sub(zone.progress) } else { zone.progress }
}

#[inline(never)]
fn tick_zone(zone: &mut Zone, index: u8, params: &Params, dt: u32, out: &mut Awards) {
    let team = match (zone.count[0], zone.count[1]) {
        (0, 0) => {
            if zone.owner.is_none() && zone.progress != 0 {
                let fall = dt.wrapping_mul(params.restore_rate);
                zone.progress = if fall > zone.progress { 0 } else { zone.progress - fall };
            }
            return;
        }
        (_, 0) => Team::Zero,
        (0, _) => Team::One,
        _ => return,
    };
    if zone.owner == Some(team) && zone.progress == FULL {
        return;
    }
    let n = u32::from(zone.count[team.index()]).min(params.capture_count);
    let rate = dt.wrapping_mul(params.capture_rate[n as usize - 1]);
    if zone.capturing == Some(team) {
        zone.progress = zone.progress.wrapping_add(rate).min(FULL);
    } else if rate > zone.progress {
        if zone.owner.is_some() {
            share(zone, zone.present[team.index()], params.award, AwardKind::Neutralise, index, team, out);
        }
        zone.progress = (rate - zone.progress).min(FULL);
        zone.owner = None;
        zone.capturing = Some(team);
        zone.marks = [FULL; PLAYERS];
    } else {
        zone.progress -= rate;
    }
    if zone.progress == FULL && zone.capturing == Some(team) && zone.owner != Some(team) {
        zone.owner = Some(team);
        share(zone, zone.present[team.index()], params.award, AwardKind::Capture, index, team, out);
    }
}

/// One tick of the match in play (the time rule's state 3): the zones, then
/// the teams' points. `players[i]` is roster place i (only `entered` ones are
/// read). The awards go to `out`.
pub fn tick(
    state: &mut State,
    params: &Params,
    players: &[Option<Body>; PLAYERS],
    dt: u32,
    time: u32,
    out: &mut Awards,
) {
    let dt = dt.min(MAX_DT);
    // who is inside, by the world of this tick
    let entered = state.entered;
    let zones = state.zone_count as usize;
    for (z, zone) in state.zones.iter_mut().take(zones).enumerate() {
        let mut present = [Roster(0); 2];
        for (i, player) in players.iter().enumerate() {
            let Some(body) = player else { continue };
            if entered.has(i) && inside(params, z, &body.feet, body.half_height) {
                present[body.team.index()].0 |= 1 << i;
            }
        }
        for team in Team::BOTH {
            let t = team.index();
            let came = present[t].0 & !zone.present[t].0;
            for i in 0..PLAYERS {
                if came >> i & 1 != 0 {
                    zone.marks[i] = mark_of(zone, team);
                }
            }
            zone.present[t] = present[t];
            zone.count[t] = present[t].len() as u8;
        }
    }
    // the end is set: nothing moves any more
    if state.end.get().is_some() {
        return;
    }
    for (z, zone) in state.zones.iter_mut().take(zones).enumerate() {
        tick_zone(zone, z as u8, params, dt, out);
    }
    // the teams' research: by the units each owns
    for team in Team::BOTH {
        let t = team.index();
        let owned = state.zones.iter().take(zones).filter(|zone| zone.owner == Some(team)).count() as u32;
        if owned != 0 {
            let owned = owned.min(params.owned_count);
            let gain = dt.wrapping_mul(params.points_rate[owned as usize - 1]) / RATE_MS;
            state.points[t] = state.points[t].wrapping_add(gain).min(params.target);
        }
        let score = state.points[t].wrapping_add(MICRO / 2) / MICRO;
        state.score[t] = score.min(params.target / MICRO) as u16;
    }
    if state.points[0] >= params.target || state.points[1] >= params.target {
        state.end = EndTime(time.wrapping_add(3000));
    }
}

/// A kill (event 1 {killer, victim}): an award when the victim was inside a
/// unit his team was taking (0.27d's "kill invader").
pub fn kill(state: &State, killer: Place, victim: Place, killer_team: Team, victim_team: Team) -> Option<Award> {
    if killer == victim || killer_team == victim_team || state.end.get().is_some() {
        return None;
    }
    let zones = state.zone_count as usize;
    for (z, zone) in state.zones.iter().take(zones).enumerate() {
        if !zone.present[victim_team.index()].has(victim.index()) {
            continue;
        }
        // 0.27d (0x31E9E7..0x31EA11): inside a unit his team does not own:
        // taking it, or draining the killer's team's
        if zone.owner == Some(victim_team) {
            continue;
        }
        return Some(Award { kind: AwardKind::KillInvader, player: killer, zone: z as u8, team: killer_team, amount: 1 });
    }
    None
}

pub fn entered(state: &mut State, place: Place) {
    state.entered.0 |= 1 << place.index();
}

pub fn left(state: &mut State, place: Place) {
    let bit = 1u32 << place.index();
    state.entered.0 &= !bit;
    let zones = state.zone_count as usize;
    for zone in state.zones.iter_mut().take(zones) {
        for t in 0..2 {
            if zone.present[t].0 & bit != 0 {
                zone.present[t].0 &= !bit;
                zone.count[t] = zone.count[t].wrapping_sub(1);
            }
        }
    }
}

/// The leader, none for a draw: by the shown scores, equal a draw; once a
/// team reached the goal (the end set), by the millionths: 100 shown for
/// both (99.5 rounds up) is not a draw when one team reached 100.
pub fn leader(state: &State) -> Option<Team> {
    if state.end.get().is_some() && state.points[0] != state.points[1] {
        return Some(if state.points[0] > state.points[1] { Team::Zero } else { Team::One });
    }
    match state.score[0].cmp(&state.score[1]) {
        core::cmp::Ordering::Greater => Some(Team::Zero),
        core::cmp::Ordering::Less => Some(Team::One),
        core::cmp::Ordering::Equal => None,
    }
}

pub fn completed(state: &State, time: u32) -> bool {
    state.end.get().is_some_and(|end| time >= end)
}

fn put32(out: &mut [u8], value: u32) {
    out[..4].copy_from_slice(&value.to_le_bytes());
}

/// The little-endian word at `at` of a block of the wire.
fn word<const N: usize>(bytes: &[u8; N], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The state's little-endian bytes, as the rules blob carries them (the
/// order the game's writers get them in, `research_arena`'s serialize).
/// Returns the bytes written ([`serialized_size`]).
pub fn write(state: &State, out: &mut [u8]) -> usize {
    put32(&mut out[0..], state.points[0]);
    put32(&mut out[4..], state.points[1]);
    out[8..10].copy_from_slice(&state.score[0].to_le_bytes());
    out[10..12].copy_from_slice(&state.score[1].to_le_bytes());
    put32(&mut out[12..], state.end.raw());
    put32(&mut out[16..], state.entered.0);
    out[20] = state.zone_count as u8;
    let mut at = HEAD_BYTES;
    for zone in state.zones.iter().take(state.zone_count as usize) {
        let bytes = &mut out[at..at + ZONE_BYTES];
        put32(bytes, zone.progress);
        bytes[4] = team_byte(zone.capturing);
        bytes[5] = team_byte(zone.owner);
        bytes[6] = zone.count[0];
        bytes[7] = zone.count[1];
        put32(&mut bytes[8..], zone.present[0].0);
        put32(&mut bytes[12..], zone.present[1].0);
        for (i, &mark) in zone.marks.iter().enumerate() {
            put32(&mut bytes[16 + 4 * i..], mark);
        }
        at += ZONE_BYTES;
    }
    at
}

/// Reads the state [`write`] wrote, for these parameters: the bytes read,
/// or none (and `state` as it was) for bytes that are not a state of them.
pub fn read(state: &mut State, params: &Params, input: &[u8]) -> Option<usize> {
    let zones = (params.zone_count as usize).min(MAX_ZONES);
    let (head, mut rest) = input.split_first_chunk::<HEAD_BYTES>()?;
    if u32::from(head[20]) != params.zone_count || rest.len() < zones * ZONE_BYTES {
        return None;
    }
    let mut read = State::ZERO;
    reset(&mut read, params);
    read.points = [word(head, 0), word(head, 4)];
    read.score = [u16::from_le_bytes([head[8], head[9]]), u16::from_le_bytes([head[10], head[11]])];
    read.end = EndTime(word(head, 12));
    read.entered = Roster(word(head, 16));
    if read.points[0] > params.target || read.points[1] > params.target {
        return None;
    }
    for zone in read.zones.iter_mut().take(zones) {
        let (bytes, next) = rest.split_first_chunk::<ZONE_BYTES>()?;
        rest = next;
        zone.progress = word(bytes, 0);
        zone.capturing = parse_team_byte(bytes[4]).ok()?;
        zone.owner = parse_team_byte(bytes[5]).ok()?;
        zone.count = [bytes[6], bytes[7]];
        zone.present = [Roster(word(bytes, 8)), Roster(word(bytes, 12))];
        for (i, mark) in zone.marks.iter_mut().enumerate() {
            *mark = word(bytes, 16 + 4 * i);
        }
        if zone.progress > FULL
            || u32::from(zone.count[0]) != zone.present[0].len()
            || u32::from(zone.count[1]) != zone.present[1].len()
            || (zone.present[0].0 | zone.present[1].0) >> PLAYERS != 0
        {
            return None;
        }
    }
    *state = read;
    Some(HEAD_BYTES + zones * ZONE_BYTES)
}

#[cfg(test)]
mod tests;
