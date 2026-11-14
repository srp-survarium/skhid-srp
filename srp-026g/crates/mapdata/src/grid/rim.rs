//! [A364 C2 rows 11/12, A385] The rim jumps ([`Body::rim_jumps`]): jumps up a ledge, or across a gap, to a top higher than the
//! Jump and gap links reach, each flown on the exact tick ([`crate::controller::jump`]) over the presses a client can make, made
//! a GapJump link carrying the press that lands most robustly and its share, and not made where none lands.
//!
//! The client's air branch never steps up (`player_step` 0x7BF26: the jump branch has no `step_up`; 0.0000 m over the clip in
//! 2,245 flights): a top over the clip's apex is got onto when the capsule's round bottom comes to rest on its edge as the clip
//! comes down and the ground steps after `end_jump` climb the rest, and only from a run-up (exactness `notes-mapflights.md` §2 C,
//! Chemical Plant's deck: walking 44 of 396, sprint 143 of 424, standing 0 of 396). Whether it does depends on the edge's
//! shape and where the jump is pressed, so no rule of heights says it: each link is flown.
//!
//! The presses (A385-FORMAT, `private/pvb-343-check/nav-builder2-20261112/A385-FORMAT.md`, signed by nav-motor and nav-builder,
//! the client claims VERIFIED by exactness 2026-11-14): the short jump from the walk at the body's cap and the sprint jump from
//! the sprint (all [`jump::sprint_variants`]: the leg and the prepare's phase are the client's, so a sprint press lands only when
//! every variant does), each from the exactness rig's run-up ([`jump::run_up`]) and pressed at each of the gap links' leads
//! ([`gap_leads`], the built file's range) before the take-off floor's centre, the keys let go at the landing. A press lands
//! when the body lands on a floor up from the take-off ([`WalkGrid::rim_lands_on`]) and stays there. Of the presses that land, the
//! link carries the one whose neighbourhood ([`jump::neighbourhood`]: the motor's measured spread of lead, side and heading,
//! the world 1-2 cm off, the bots' fastest kit) lands most ([`Share`]); a walking press on a tie. The share prices the link
//! ([`WalkGrid::link_cost`]); nothing is cut (the owner, 2026-11-13: the bots are taught the press, the path is not taken
//! away). Made after the erosion, with no press from a floor only a crouched body fits (a jump is never pressed from a crouch:
//! the jump's gate 0x2FBDC0 -> 0x2FC230 reads `on_ground` 0x7EF90, false for a crouched capsule, exactness 2026-11-14), and
//! before the reach, so a top only a rim jump gets to is kept.
use super::gap::{self, OffLink, PressLead, Share, Takeoff};
use super::{LinkKind, NodeId, WalkGrid, Wide, DIRS, MARGINAL_JUMP_M, NONE, RIM_REACH_M};
use crate::controller::jump::{self, FlightStep, Kind, Kits, Neighbour};
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::math::Vec3;

/// A flight lands when the feet end within this of the landing floor's height, metres (the walk check's [`super::controlled`]
/// arrival band, `AT_HEIGHT_M`).
const LAND_HEIGHT_M: f32 = 0.2;

/// What the rim pass did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RimStats {
    /// Rim jumps flown …
    pub tried: usize,
    /// … made with a walking press …
    pub walked: usize,
    /// … and with a sprint press; the rest no press made.
    pub sprinted: usize,
    /// Of the made, those whose presses land on another floor of the top than the one tried: the link goes to the floor reached.
    pub retargeted: usize,
    /// Of the sprint-made, those that carry a walking press too ([`OffLink::walk`]).
    pub with_walk: usize,
    /// The made links by share: under 0.2, 0.2 to 0.6, 0.6 and over (owner-5's knife-edge / middle / robust classes).
    pub by_share: [usize; 3],
    /// [A385 rule (d)] Risers flown ([`WalkGrid::riser`]: Walk links up [`super::Body::walked_step`] or more) …
    pub risers: usize,
    /// … got up by a walking press …
    pub risers_walked: usize,
    /// … and by a sprint press; the rest no press gets up ([`Riser::NoPress`]).
    pub risers_sprinted: usize,
    /// The pressed risers by share, as [`Self::by_share`].
    pub risers_by_share: [usize; 3],
    /// Flights flown.
    pub flights: usize,
}

/// A press that lands: its gait and lead, the floor it lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Landing {
    kind: Kind,
    lead: f32,
    to: NodeId,
}

/// [A385 rule (d)] How a body jumps up a riser (a Walk link rising [`super::Body::walked_step`] or more,
/// [`super::WalkGait::SprintOrJump`]), flown as a rim jump (A385-FORMAT §5): the press the most of whose neighbourhood lands
/// on the riser's top and its share, or no press gets up it (priced as a share of 0, [`Share::failed_tries`]; a sprinting body
/// still steps up it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Riser {
    /// A short jump or a sprint jump ([`Takeoff::Press`], [`Takeoff::Sprint`]; never a walk-off) and its share.
    Pressed { press: Takeoff, share: Share },
    /// No flown press lands on the top.
    NoPress,
}

impl Riser {
    /// The share that prices it: a pressed riser's own, none (0 of the neighbourhood) for one no press gets up.
    pub fn share(self) -> Share {
        match self {
            Riser::Pressed { share, .. } => share,
            Riser::NoPress => Share::landed(0),
        }
    }

    /// The run a failed try walks again ([`Takeoff::run_up_m`]); a walking press's for one no press gets up (the motor's
    /// riser press is a walking one).
    pub fn run_up_m(self) -> f32 {
        match self {
            Riser::Pressed { press, .. } => press.run_up_m(),
            Riser::NoPress => Takeoff::Press(PressLead::from_metres(gap::GAP_PRESS_MIN_M)).run_up_m(),
        }
    }

    /// In a built file: the press's centimetres ([`Takeoff`]'s, 0 for none) and the share's byte.
    pub(super) fn bytes(self) -> (u16, u8) {
        match self {
            Riser::Pressed { press, share } => (press.centimetres(), share.byte()),
            Riser::NoPress => (0, 0),
        }
    }

    /// From a built file's press and share: None when they are no riser's (a walk-off press, a press out of the built range, a
    /// share that is no count of the neighbourhood or is not flown, a no-press riser with a share).
    pub(super) fn from_bytes(cm: u16, share: u8) -> Option<Self> {
        let share = Share::from_byte(share).filter(|s| s.flown().is_some())?;
        match Takeoff::from_centimetres(cm) {
            Takeoff::WalkOff => (share == Share::landed(0)).then_some(Riser::NoPress),
            press => press.is_valid().then_some(Riser::Pressed { press, share }),
        }
    }
}

/// A rim link made: where it goes, its press and share, and a sprint link's walking press.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Made {
    to: NodeId,
    press: Takeoff,
    share: Share,
    walk: Option<(PressLead, Share)>,
}

impl WalkGrid {
    /// The floor under `feet`: in the feet's own cell (the feet within half a cell of its centre on both axes, the grid's own
    /// test), the one within [`LAND_HEIGHT_M`] of the feet's height, the nearest in height; where that cell has none, the floor
    /// within [`LAND_HEIGHT_M`] whose centre is nearest across, within the capsule's radius ([`super::Body::radius`]): a body
    /// stands on a top over a cell the grid has no floor in at that height (the erosion's, or a top one cell wide). Chemical
    /// Plant (-64.71, 7.58, 45.97): a walking press let go at its landing rests there, on the 7.625 strip (one cell row, its
    /// floors' centres at z 45.625, 0.35 m across), its own cell's floors 12.2 m up.
    fn floor_under(&self, feet: Vec3) -> Option<NodeId> {
        let near = |f: NodeId| (self.height[f as usize] - feet.y).abs() <= LAND_HEIGHT_M;
        let i = ((feet.x - self.x0) / self.cell).floor() as i32;
        let k = ((feet.z - self.z0) / self.cell).floor() as i32;
        let cell = self.cell_index(i, k)?;
        let own = (self.start[cell]..self.start[cell + 1])
            .filter(|&f| near(f))
            .min_by(|&p, &q| (self.height[p as usize] - feet.y).abs().total_cmp(&(self.height[q as usize] - feet.y).abs()));
        if own.is_some() {
            return own;
        }
        let r = self.body.radius;
        let reach = (r / self.cell).ceil() as i32;
        let across = |f: NodeId| self.position(f).distance_xz(feet);
        (-reach..=reach)
            .flat_map(|dk| (-reach..=reach).map(move |di| (di, dk)))
            .filter_map(|(di, dk)| self.cell_index(i + di, k + dk))
            .flat_map(|c| self.start[c]..self.start[c + 1])
            .filter(|&f| near(f) && across(f) <= r)
            .min_by(|&p, &q| across(p).total_cmp(&across(q)))
    }

    /// The floor a flight's `steps` (keys let go at the landing, [`jump::STAY_KEYS`]) from the take-off floor `a` lands on and
    /// stays on (A385-FORMAT §1): its landing ([`jump::landing_and_rest`]: the first step out of the jump on the ground,
    /// `on_ground` 0x7EF90) over a top floor ([`Self::floor_under`]: up from `a`, higher than a step, the walk's own climb, and
    /// not one `a` has a link to, `linked`), the body on the ground from there to its rest, and at rest over a top floor too: the
    /// floor it rests on, where the link goes. On a sloped or stepped top the ground steps after the landing climb on (Rudnya
    /// (33.26, 24.26, -85.21): out of the jump over the 24.39 floor, at rest on the 24.67 one beside it). Not where a straight
    /// walk on ends: a bot that has landed follows its route (Vector Laboratory's ledge by the shelf, (23.76, 25.39, -28.8), is
    /// landed on, then walked off its far end by a straight walk); and on the floor the body is on, whichever of the top it is
    /// (Vector Laboratory's (-23.72, 23.18, -22.0): a jump meant for the floor at 23.49 stops on the one at 23.18 beside it), any
    /// floor up from the take-off (Vector Laboratory's (-45.35, 25.61, 4.93), 0.88 m over the take-off at 24.73: no fixed height).
    fn rim_lands_on(&self, steps: &[FlightStep], a: NodeId, linked: &[NodeId]) -> Option<NodeId> {
        let up = self.height[a as usize] + self.body.step;
        let onto = |feet: Vec3| self.floor_under(feet).filter(|&g| self.height[g as usize] > up && !linked.contains(&g));
        let (_, rest) = jump::landing_and_rest(steps, |f| onto(f.feet).is_some())?;
        onto(rest.feet)
    }

    /// [A385 rule (d)] The floor a flight's `steps` up the riser to `b` lands on and stays on: `b` when its landing (as
    /// [`Self::rim_lands_on`]'s, the first step out of the jump on the ground) is over a floor of `b`'s level (within
    /// [`LAND_HEIGHT_M`]) in the feet's own cell, the body on the ground from there to its rest, and at rest on that level.
    fn riser_lands_on(&self, steps: &[FlightStep], b: NodeId) -> Option<NodeId> {
        let level = self.height[b as usize];
        let onto = |feet: Vec3| self.floor_under(feet).filter(|&g| (self.height[g as usize] - level).abs() <= LAND_HEIGHT_M);
        let (landing, rest) = jump::landing_and_rest(steps, |f| onto(f.feet).is_some())?;
        onto(landing.feet)?;
        onto(rest.feet).map(|_| b)
    }

    /// The press that gets a body from the take-off floor `a` up toward the floor `b` along `way`, among the leads `leads` (metres
    /// before the take-off) and both gaits (A385-FORMAT §3), and the flights flown; `lands` is the floor a flight lands and
    /// stays on ([`Self::rim_lands_on`], [`Self::riser_lands_on`]):
    ///
    /// 1. each (gait, lead) is flown ([`jump::neighbour_flights`] at the press itself): the floors its flights land on;
    /// 2. the link goes to `b` when some press lands there, else to the floor the most presses land on (re-targeted: the link
    ///    goes where the body lands), the lowest id on a tie;
    /// 3. each press onto that floor is scored by its neighbourhood ([`jump::neighbourhood`]; its lead row reuses step 1's
    ///    flights), and the link carries the highest share; on a tie a walking press, then the lead nearest the middle of that
    ///    gait's leads onto the floor; a sprint link also carries the best walking press onto it ([`Made::walk`]), for a body
    ///    the client refuses the sprint.
    ///
    /// None when no press lands.
    #[allow(clippy::too_many_arguments)]
    fn best_press(
        &self,
        world: &Collision,
        s: &Settings,
        a: NodeId,
        b: NodeId,
        way: (f32, f32),
        leads: &[f32],
        lands: impl Fn(&[FlightStep]) -> Option<NodeId>,
        kits: Kits,
    ) -> (Option<Made>, usize) {
        let takeoff = self.position(a);
        let own = jump::neighbourhood()[0];
        let mut flights = 0;
        // Rule (e): no jump is pressed from a crouch (the jump's gate 0x2FBDC0 -> 0x2FC230 reads `on_ground` 0x7EF90, false for a
        // crouched capsule): a press whose press point is over a floor only a crouched body fits ([`WalkGrid::can_stand`]) is not
        // flown and does not land. The press point, not the take-off floor: the body presses `lead` back and flies over the
        // take-off, which may be under the top's overhang (Mamayev Kurgan: 17,363 of 48,606 rim take-offs are crouch-only floors).
        let fly = |kind: Kind, lead: f32, n: Neighbour| {
            let at = jump::press_point(takeoff, way, lead, n);
            if self.floor_under(at).is_some_and(|f| !self.can_stand(f)) {
                return None;
            }
            jump::neighbour_flights(world, s, takeoff, way, kind, lead, n, kits)
        };
        // Step 1: every press at the press itself: the floors its flights land on (one, or one per sprint variant).
        let mut flown: Vec<(Kind, f32, Vec<Option<NodeId>>)> = Vec::new();
        for kind in [Kind::Walk, Kind::Sprint] {
            for &lead in leads {
                let floors = fly(kind, lead, own).map_or_else(Vec::new, |fl| {
                    flights += fl.len();
                    fl.iter().map(|st| lands(st)).collect()
                });
                flown.push((kind, lead, floors));
            }
        }
        // A press is a candidate when some flight of it lands: a sprint press some of whose variants land is a way a client
        // makes (the owner's rule, 2026-11-13: teach, do not cut), onto the floor the most of its variants land on; its share
        // counts only neighbours where every variant lands (A385-FORMAT §2, rule (b)), so it may be 0 (the largest price).
        let landings: Vec<Landing> = flown
            .iter()
            .filter_map(|(kind, lead, floors)| {
                let on = |f: NodeId| floors.iter().filter(|&&g| g == Some(f)).count();
                let to = floors.iter().flatten().copied().max_by_key(|&f| (on(f), std::cmp::Reverse(f)))?;
                Some(Landing { kind: *kind, lead: *lead, to })
            })
            .collect();
        if landings.is_empty() {
            return (None, flights);
        }
        // Step 2: the floor.
        let count = |f: NodeId| landings.iter().filter(|l| l.to == f).count();
        let to = if count(b) > 0 {
            b
        } else {
            landings.iter().map(|l| l.to).max_by_key(|&f| (count(f), std::cmp::Reverse(f))).expect("a landing")
        };
        // Step 3: each press onto `to` scored by its neighbourhood.
        let reused = |kind: Kind, lead: f32| {
            flown
                .iter()
                .find(|(k, l, _)| *k == kind && (l - lead).abs() < 1e-3)
                .map(|(.., floors)| !floors.is_empty() && floors.iter().all(|&f| f == Some(to)))
        };
        let mut scored: Vec<(Kind, f32, u8)> = Vec::new();
        for l in landings.iter().filter(|l| l.to == to) {
            let mut landed = 0u8;
            for n in jump::neighbourhood() {
                let lead_only = n == Neighbour { lead: n.lead, ..own };
                let lands = match lead_only.then(|| reused(l.kind, l.lead + n.lead)).flatten() {
                    Some(lands) => lands,
                    None => fly(l.kind, l.lead, n).is_some_and(|fl| {
                        flights += fl.len();
                        fl.iter().all(|st| lands(st) == Some(to))
                    }),
                };
                landed += u8::from(lands);
            }
            scored.push((l.kind, l.lead, landed));
        }
        let best_of = |kind: Option<Kind>| {
            let of: Vec<&(Kind, f32, u8)> = scored.iter().filter(|s| kind.is_none_or(|k| s.0 == k)).collect();
            let mid = |k: Kind| {
                let ls: Vec<f32> = scored.iter().filter(|s| s.0 == k).map(|s| s.1).collect();
                ls.iter().sum::<f32>() / ls.len().max(1) as f32
            };
            of.into_iter().copied().max_by(|p, q| {
                p.2.cmp(&q.2)
                    .then((q.0 == Kind::Walk).cmp(&(p.0 == Kind::Walk)).reverse())
                    .then((q.1 - mid(q.0)).abs().total_cmp(&(p.1 - mid(p.0)).abs()))
            })
        };
        let (kind, lead, landed) = best_of(None).expect("a scored press");
        let walk = (kind == Kind::Sprint)
            .then(|| best_of(Some(Kind::Walk)))
            .flatten()
            .map(|(_, l, n)| (PressLead::from_metres(l), Share::landed(n)));
        let press = match kind {
            Kind::Walk => Takeoff::Press(PressLead::from_metres(lead)),
            Kind::Sprint => Takeoff::Sprint(PressLead::from_metres(lead)),
        };
        (Some(Made { to, press, share: Share::landed(landed), walk }), flights)
    }
}

/// The leads a gap link may carry: [`gap::GAP_PRESS_MIN_M`] to [`gap::GAP_PRESS_MAX_M`] by [`gap::GAP_PRESS_STEP_M`] (the built
/// file's range, `PressLead::is_valid`).
fn gap_leads() -> Vec<f32> {
    let n = ((gap::GAP_PRESS_MAX_M - gap::GAP_PRESS_MIN_M) / gap::GAP_PRESS_STEP_M).round() as usize;
    (0..=n).map(|i| gap::GAP_PRESS_MIN_M + gap::GAP_PRESS_STEP_M * i as f32).collect()
}

/// The farthest a rim gap's landing face is searched from the take-off floor's centre, metres: the sprint clip's travel to its
/// apex (2.65 m, frame 14, PROVEN, the clip) from the nearest press a gap link makes ([`gap::GAP_PRESS_MIN_M`] 0.6), and the
/// capsule's radius (0.38, it meets the face with its side): a face farther is not met before the clip comes down. Only bounds
/// the search; each link is flown.
pub const RIM_GAP_ACROSS_M: f32 = 2.65 - gap::GAP_PRESS_MIN_M + 0.38;

impl WalkGrid {
    /// [`Body::rim_jumps`]: from each floor along each way whose side has no link up (none, or a drop), across cells with no
    /// floor within a step of it, to the first cell no farther than [`RIM_GAP_ACROSS_M`] with a floor higher than a gap link
    /// reaches (the body's standing jump, [`MARGINAL_JUMP_M`] at most) and no higher than [`RIM_REACH_M`] (a Jump link up a
    /// ledge goes to [`super::SPRINT_JUMP_LOW_M`]; one there takes the side, so a rim jump is above it); flown over the presses
    /// ([`best_press`]) and made a GapJump with the press that lands. After the gap links (`wide.off`, kept sorted). Returns
    /// the stats and the jumps no press made, as (take-off, landing) (none was a link before).
    pub(super) fn fly_rim_jumps(&self, wide: &mut Wide, world: &Collision, s: &Settings) -> (RimStats, Vec<(NodeId, NodeId)>) {
        let body = &self.body;
        let low = body.max_jump.min(MARGINAL_JUMP_M);
        let mut todo: Vec<(u32, u32, usize)> = Vec::new();
        for a in 0..self.height.len() {
            let ha = self.height[a];
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            for (d, &(di, dk)) in DIRS.iter().enumerate() {
                let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
                if wide.links[a][d] != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                let mut g = 1;
                while g as f32 * stride <= RIM_GAP_ACROSS_M + 1e-4 {
                    let Some(cell) = self.cell_index(i + g * di, k + g * dk) else { break };
                    let floors = self.start[cell]..self.start[cell + 1];
                    if let Some(b) = floors.clone().find(|&f| {
                        let rise = self.height[f as usize] - ha;
                        rise > low && rise <= RIM_REACH_M
                    }) {
                        if wide.off.binary_search_by_key(&(a as u32, b), |l| (l.from, l.to)).is_err() {
                            todo.push((a as u32, b, d));
                        }
                        break;
                    }
                    // A floor within a step goes on (no gap), one higher than the reach is a wall: either ends the search.
                    if floors.clone().any(|f| {
                        let rise = self.height[f as usize] - ha;
                        rise.abs() <= body.step || rise > RIM_REACH_M
                    }) {
                        break;
                    }
                    g += 1;
                }
            }
        }
        let leads = gap_leads();
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: body.fastest_walk_cap };
        let flown = super::controlled::parallel(&todo, |&(a, b, d)| {
            let (di, dk) = DIRS[d];
            let l = ((di * di + dk * dk) as f32).sqrt();
            // The floors `a` has a link to already (its sides and its off-mesh links): a landing there makes no link.
            let mut linked: Vec<NodeId> = wide.links[a as usize].iter().copied().filter(|&n| n != NONE).collect();
            let off = wide.off.partition_point(|o| o.from < a);
            linked.extend(wide.off[off..].iter().take_while(|o| o.from == a).map(|o| o.to));
            self.best_press(world, s, a, b, (di as f32 / l, dk as f32 / l), &leads, |st| self.rim_lands_on(st, a, &linked), kits)
        });
        let mut stats = RimStats { tried: todo.len(), ..RimStats::default() };
        let mut none = Vec::new();
        for (&(a, b, _), &(made, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            let Some(Made { to, press, share, walk }) = made else {
                none.push((a, b));
                continue;
            };
            match press {
                Takeoff::Press(_) => stats.walked += 1,
                _ => stats.sprinted += 1,
            }
            stats.retargeted += usize::from(to != b);
            stats.with_walk += usize::from(walk.is_some());
            stats.by_share[share.class()] += 1;
            wide.off.push(OffLink { from: a, to, kind: LinkKind::GapJump, press, share, walk });
        }
        // Two candidates of one take-off may land on the same floor: one link (the first, by the way's order).
        wide.off.sort_by_key(|l| (l.from, l.to));
        wide.off.dedup_by_key(|l| (l.from, l.to));
        (stats, none)
    }

    /// [A385 rule (d)] The risers' presses ([`Riser`]): every Walk link rising [`super::Body::walked_step`] or more, flown over the rim jumps' presses ([`Self::best_press`], landing on the
    /// riser's top, [`Self::riser_lands_on`]) into `wide.riser`, sorted; `stats` counts them.
    pub(super) fn fly_risers(&self, wide: &mut Wide, world: &Collision, s: &Settings, stats: &mut RimStats) {
        let walked = self.body.walked_step();
        let mut todo: Vec<(u32, u32, usize)> = Vec::new();
        for a in 0..self.height.len() {
            for d in 0..DIRS.len() {
                let b = wide.links[a][d];
                if b != NONE && wide.kinds[a][d] == LinkKind::Walk && self.height[b as usize] - self.height[a] >= walked {
                    todo.push((a as u32, b, d));
                }
            }
        }
        let leads = gap_leads();
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let flown = super::controlled::parallel(&todo, |&(a, b, d)| {
            let (di, dk) = DIRS[d];
            let l = ((di * di + dk * dk) as f32).sqrt();
            self.best_press(world, s, a, b, (di as f32 / l, dk as f32 / l), &leads, |st| self.riser_lands_on(st, b), kits)
        });
        stats.risers += todo.len();
        for (&(a, b, _), &(made, flights)) in todo.iter().zip(&flown) {
            stats.flights += flights;
            let riser = match made {
                Some(Made { press, share, .. }) => {
                    match press {
                        Takeoff::Sprint(_) => stats.risers_sprinted += 1,
                        _ => stats.risers_walked += 1,
                    }
                    stats.risers_by_share[share.class()] += 1;
                    Riser::Pressed { press, share }
                }
                None => Riser::NoPress,
            };
            wide.riser.push((a, b, riser));
        }
        wide.riser.sort_by_key(|&(a, b, _)| (a, b));
        wide.riser.dedup_by_key(|&mut (a, b, _)| (a, b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Body, OffMeshLinks, STANDING_JUMP_M};
    use crate::{Triangle, WalkerBehaviour};

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    fn body(rim_jumps: bool) -> Body {
        Body { rim_jumps, exe_collision: true, start_witness: true, max_jump: STANDING_JUMP_M, ..Body::client() }
    }

    /// Ground at 0 for x < 5 and a block from x 5 to 12 whose top is `top` high, 6 m wide; a body starts at x 1: the grid, its
    /// rim stats and whether the top is reached.
    fn onto(top: f32, rim_jumps: bool) -> (WalkGrid, RimStats, bool) {
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, top, 0.0), Vec3::new(5.0, top, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(rim_jumps), &start, &start, OffMeshLinks::Gaps);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (grid, stats.rim, up)
    }

    /// A 1.30 m top (over the sprint jump's lowest top, 1.28) and a 1.40 m one are got onto by some client press: rim jumps are
    /// made with the press that lands (a gap link's, in the built file too) and the tops are reached; without rim jumps they are
    /// not. A 2.10 m top no press gets onto: no rim jump is made, and the build reports every one it flew.
    #[test]
    fn a_rim_jump_is_made_with_a_press_that_lands_and_not_where_none_does() {
        for top in [1.30, 1.40] {
            let (grid, rim, up) = onto(top, true);
            assert!(rim.walked + rim.sprinted > 0 && up, "{top}: {rim:?}");
            assert_eq!(grid.rim_unmade().len(), rim.tried - rim.walked - rim.sprinted, "{top}");
            let path = std::env::temp_dir().join(format!("srp-rim-{}-{top}.bin", std::process::id()));
            grid.write(&path, 11).unwrap();
            let back = WalkGrid::read(&path, 11, body(true)).unwrap();
            std::fs::remove_file(&path).ok();
            assert_eq!(back.digest(), grid.digest());
            let (_, _, up_off) = onto(top, false);
            assert!(!up_off, "{top}: no rim jumps, over the old cap");
        }
        let (grid, rim, up) = onto(2.10, true);
        assert!(rim.tried > 0 && rim.walked + rim.sprinted == 0 && !up, "2.10: {rim:?}");
        assert_eq!(grid.rim_unmade().len(), rim.tried);
    }

    /// [A385 §4] The share's classes and price: every neighbour lands, no extra try; 7 of 14, one; none, 14 (the largest, kept);
    /// not flown, none; owner-5's classes at 0.2 and 0.6 of 14; a built byte over 14 that is not the unflown mark is refused.
    #[test]
    fn a_share_prices_the_tries_that_fail() {
        assert_eq!(Share::FULL.failed_tries(), 0.0);
        assert_eq!(Share::landed(7).failed_tries(), 1.0);
        assert_eq!(Share::landed(0).failed_tries(), jump::NEIGHBOURS as f32);
        assert_eq!(Share::UNFLOWN.failed_tries(), 0.0);
        let classes: Vec<usize> = (0..=14).map(|n| Share::landed(n).class()).collect();
        assert_eq!(classes, [0, 0, 0, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2]);
        assert_eq!(Share::UNFLOWN.class(), 2);
        assert_eq!(Share::from_byte(14), Some(Share::FULL));
        assert_eq!(Share::from_byte(u8::MAX), Some(Share::UNFLOWN));
        assert_eq!(Share::from_byte(15), None);
    }

    /// [A385 rule (d)] A riser's record parses only as a riser's: a press (walking in range, or sprint) with a flown share, or
    /// no press with a share of 0.
    #[test]
    fn a_riser_record_parses_only_as_a_riser() {
        let walk = Takeoff::Press(PressLead::from_metres(1.0));
        let sprint = Takeoff::Sprint(PressLead::from_metres(0.8));
        for r in [Riser::Pressed { press: walk, share: Share::landed(9) }, Riser::Pressed { press: sprint, share: Share::landed(0) }, Riser::NoPress] {
            let (cm, share) = r.bytes();
            assert_eq!(Riser::from_bytes(cm, share), Some(r));
        }
        assert_eq!(Riser::from_bytes(0, 3), None, "no press with a share");
        assert_eq!(Riser::from_bytes(walk.centimetres(), u8::MAX), None, "not flown");
        assert_eq!(Riser::from_bytes(walk.centimetres(), 15), None, "over the neighbourhood");
        assert_eq!(Riser::from_bytes(30, 5), None, "a walking press under the built range");
    }

    /// [A385 rule (d)] A 0.78 m riser (walked from 0.76, climbed by a sprint or a jump): its Walk links up are flown and carry
    /// a press or none ([`WalkGrid::riser`]), priced by its share, kept through the built file; without rim jumps none.
    #[test]
    fn a_riser_is_flown_priced_and_kept_in_the_file() {
        let (grid, rim, up) = onto(0.78, true);
        assert!(up && rim.risers > 0 && grid.riser_count() > 0, "{rim:?}");
        let (a, b, r) = grid.riser[0];
        assert_eq!(grid.link_gait(a, b), Some(super::super::WalkGait::SprintOrJump));
        let base = (grid.position(b) - grid.position(a)).length() + LinkKind::Walk.penalty();
        let extra = r.share().failed_tries() * ((grid.position(b) - grid.position(a)).length() + r.run_up_m());
        let lips = grid.link_cost(a, b, LinkKind::Walk) - base - extra;
        assert!((-1e-3..=super::super::FALL_EDGE_PENALTY.max(super::super::LIP_PENALTY) + 1e-3).contains(&lips), "{lips}");
        assert_eq!(rim.risers_walked + rim.risers_sprinted, rim.risers_by_share.iter().sum::<usize>());
        let path = std::env::temp_dir().join(format!("srp-riser-{}.bin", std::process::id()));
        grid.write(&path, 11).unwrap();
        let back = WalkGrid::read(&path, 11, body(true)).unwrap();
        std::fs::remove_file(&path).ok();
        assert_eq!(back.digest(), grid.digest());
        assert_eq!(back.riser(a, b), Some(r));
        let (off, rim_off, _) = onto(0.78, false);
        assert_eq!((off.riser_count(), rim_off.risers), (0, 0));
    }

    /// One rim jump on a real map, every press's outcome printed: `RIM_MAP` (a format 6 map), `RIM_FROM` and `RIM_TO` ("x z y",
    /// the take-off and landing floors). `cargo test -- --ignored --nocapture fly_one`.
    #[test]
    #[ignore]
    fn fly_one() {
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let map = crate::MapData::from_bytes(&std::fs::read(var("RIM_MAP")).unwrap()).unwrap();
        let world = Collision::build_exe(&map).unwrap();
        let s = body(true).controller().1;
        let p = |k: &str| {
            let v: Vec<f32> = var(k).split_whitespace().map(|x| x.parse().unwrap()).collect();
            Vec3::new(v[0], v[2], v[1])
        };
        let (from, to) = (p("RIM_FROM"), p("RIM_TO"));
        let l = (to.x - from.x).hypot(to.z - from.z);
        let way = ((to.x - from.x) / l, (to.z - from.z) / l);
        // Every press at the press itself and at each neighbour: its flights' landings and ends.
        let kits = Kits { body: jump::WALK_CAP_MS, fastest: jump::WALK_CAP_MS };
        for kind in [Kind::Walk, Kind::Sprint] {
            for lead in gap_leads() {
                for (i, n) in jump::neighbourhood().into_iter().enumerate() {
                    let Some(fl) = jump::neighbour_flights(&world, &s, from, way, kind, lead, n, kits) else {
                        println!("{kind:?} lead {lead:.1} n{i}: no run-up");
                        continue;
                    };
                    for steps in fl {
                        let e = steps.last().unwrap().feet;
                        let land = jump::landing_and_rest(&steps, |_| true).map(|(l, _)| l.feet);
                        // Each step's feet and the client's flags: J in the jump, G on the ground (`on_ground` 0x7EF90).
                        let trace: Vec<String> = steps
                            .iter()
                            .map(|f| {
                                let flags = format!("{}{}", if f.in_jump { "J" } else { "" }, if f.on_ground { "G" } else { "" });
                                format!("{:.2},{:.2},{:.2}{flags}", f.feet.x, f.feet.y, f.feet.z)
                            })
                            .collect();
                        println!("{kind:?} lead {lead:.1} n{i}: land {land:?} end ({:.3} {:.3} {:.3}) | {}", e.x, e.y, e.z, trace.join(" "));
                    }
                }
            }
        }
    }

    /// Vector Laboratory's shelf, made: ground at 0 for x < 5, a pit 3 m deep across `gap` m, then a top 1.10 m up (over the
    /// standing jump, so no gap link reaches it). With rim jumps and the gap links: the rim stats and whether the top is reached.
    fn over_a_gap(gap: f32, rim_jumps: bool) -> (RimStats, bool) {
        let (x1, top) = (5.0 + gap, 1.10);
        let mut tris = Vec::new();
        quad(&mut tris, Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, 0.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, -3.0, 6.0), Vec3::new(x1, -3.0, 6.0), Vec3::new(x1, -3.0, 0.0));
        quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, -3.0, 6.0));
        quad(&mut tris, Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(12.0, top, 6.0), Vec3::new(12.0, top, 0.0));
        quad(&mut tris, Vec3::new(x1, -3.0, 0.0), Vec3::new(x1, top, 0.0), Vec3::new(x1, top, 6.0), Vec3::new(x1, -3.0, 6.0));
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [-2.0, 0.0, 12.0, 6.0], 0.25, body(rim_jumps), &start, &start, OffMeshLinks::Gaps);
        let up = grid.locate(Vec3::new(9.0, top, 3.0), 0.3).is_some_and(|n| grid.reached(n));
        (stats.rim, up)
    }

    /// A 1.10 m top across a 1.25 m pit (the shelf's, 1.26 m and 1.1 m) is got onto: the rim gap pass makes links with a press
    /// that lands, and the top is reached; without rim jumps it is not.
    #[test]
    fn a_top_over_a_gap_higher_than_the_standing_jump_is_reached_by_its_flown_press() {
        let (rim, up) = over_a_gap(1.25, true);
        assert!(rim.walked + rim.sprinted > 0 && up, "{rim:?}");
        let (_, up_off) = over_a_gap(1.25, false);
        assert!(!up_off, "no rim jumps: no gap link reaches 1.10 m");
    }
}
