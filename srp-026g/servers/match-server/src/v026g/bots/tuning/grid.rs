//! The maps' walk grids.

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridTuning {
    /// Whether the maps' walk grids have the gap links (a walking jump
    /// across a gap, up or down: `mapdata::grid::gap`), which bots then
    /// route over and jump. A grid is the map's, built once for every
    /// match on it: the server's grids take the server's tuning
    /// (`SRP_BOT_TUNING`, read once at its start: `Maps::configured`), the
    /// bench's this run's tuning (`SRP_BENCH_TUNING`).
    pub gap_links: bool,
    /// `Ctx::path_dist` is the regions' walk between floors when a target
    /// is a storey off; off, the straight
    /// ground distance whatever the floor, with Collect's batteries a storey
    /// off priced by the walk as before (the bench's switch,
    /// `SRP_BENCH_FLOOR_PATHS=1` on). On by default..
    pub floor_paths: bool,
    /// Whether the maps' walk grids are built with the numbers the 0.26g clients run
    /// ([`mapdata::Body::client`]: step 0.8 and ground walked up to 55°, `startup.cfg`), not the exe's
    /// defaults (0.6, 50°): the pile of barrels the owner walks up from one side, and no jump onto ground
    /// steeper than the clients walk. Off: the grids as they were. On by default.
    /// A grid is the map's (see `gap_links`).
    pub client_numbers: bool,
    /// Whether the maps' walk grids take the floors nearer to a wall than the body's radius, measured exactly,
    /// after the cell rounds ([`mapdata::Erosion::Exact`]): a floor beside a post's corner, in an opening the
    /// body does not fit (A84 item 13). Off: the cells' erosion as it was. On by default. A grid is the map's
    /// (see `gap_links`).
    pub exact_erosion: bool,
    /// Whether a Drop link of the maps' walk grids is kept only where the body that walks off the edge (3.5 m/s,
    /// carried on through the air) comes down on a floor within the harmless fall (`mapdata::Body::drop_flight_mps`):
    /// a chain of harmless steps below an edge is one fall of 7.5 m (the team 1 spawn deck on Chemical Plant, A95).
    /// Off: the cell model. On by default. A grid is the map's (see `gap_links`).
    pub honest_drops: bool,
    /// Whether the take-off floor of a walk-off gap drop is a lip, as a Drop link's is (`mapdata::Body::walk_off_lips`): the
    /// lip penalty, the near-lip band, the pull's corner cut, the lip guard and resting off a lip then keep a body off the
    /// edge. Off: the walk-offs are no lips (63 of 245 falls off the route began on a floor no guard knew). On by default.
    /// Only where there are walk-offs, so with `gap_links` off it changes nothing. A grid is the map's (see `gap_links`).
    pub walk_off_lips: bool,
    /// Whether a Jump link of the maps' walk grids is kept only where the top it lands on goes on as far as the jump's clip
    /// carries the body (`mapdata::Body::landing_carry`; 1.3 m on a walking jump, 3 m on a sprint jump): the clip runs on to its
    /// end whatever the body has landed on, so on a wall's top or a rail it walks off the far side and comes down (Chemical Plant,
    /// the spawn exit's wall at (94.9, 67.1): 0 of 74 presses got up). Off: a landing needs the body's width. On by default. A
    /// grid is the map's (see `gap_links`).
    pub landing_carry: bool,
    /// With `landing_carry`: whether a Jump link no press keeps the body on the top of is kept where it is the only way to a floor
    /// or back from it (`mapdata::Body::keep_last_resorts`). Off (the default): they are taken away, but those that are the only way back out of a
    /// floor a body can walk to (a pit); the floors only they led to are out of reach: the live players stood on those floors in 0-7 lives of 1,000-3,000 a map and never within 2 m of the 4
    /// objectives that only they reached (A170); on the bench 3 of 81 probe presses of them got up. A grid is the map's (see `gap_links`).
    pub keep_last_resorts: bool,
    /// With `exact_erosion`: whether a floor only a crouched body fits by the walls (a beam or lip between 1.1 and 1.8 m over it) is
    /// kept, walked crouched (`mapdata::Body::crouch_only_floors`). Off: it is taken away as the exact erosion did before A170. A grid
    /// is the map's (see `gap_links`).
    pub crouch_floors: bool,
    /// The standing jump's cap on the clients' body, metres: a Jump link above it is a sprint jump (a run-up with sprint
    /// held, apex 1.28 m, `mapdata::grid::SPRINT_JUMP_LOW_M`), kept without a run-up as the marginal jump the base made.
    /// The clip's root tops at 0.998 (`mapdata::grid::STANDING_JUMP_M`, PROVEN); whether the body reaches a ledge above it
    /// is not (J0 climbed flat tops of 1.00-1.02 m, the 8f rows mounted +1.08), so the client test G-V4 may move the
    /// number. 1.05 is the exe's, the base's. With `client_numbers` off the body keeps the exe's. A grid is the map's.
    pub jump_cap_m: Positive,
    /// Whether the maps' walk grids walk up a ledge's edge the capsule rides (`mapdata::Body::capsule_steps`): a rise up to the
    /// step plus the part of the radius over which the capsule's round bottom meets an edge within the slope (0.96 m with the
    /// clients' numbers) is a Walk link up, not a jump. Vostok Radar Station's 0.86-0.90 m edge at (8.41, -16.51) is walked up
    /// by the live players with no jump; 59 % of the offered maps' standing-jump links rise that much in one edge (A198). Off
    /// (the default) until a client test shows the walk. A grid is the map's (see `gap_links`).
    pub capsule_steps: bool,
    /// Whether a walk-off of the maps' walk grids (a Drop link, a walk-off gap drop) is kept only where the capsule walking off the
    /// edge has room (`mapdata::Body::drop_room`): no wall on the move's side of the take-off, between a step and the crouched
    /// height over it, within the radius less half a cell of the way down. The links check the landing's ceiling only; a wall
    /// past the edge, between the floors a drop spans, holds the body at the edge (A206, the offered maps: 4,350 links cut; their
    /// take-off cells had a live stuck note 1.75 times as often as the others, live bodies walked off 4 of them, at one spot of
    /// Tarakanovsky Fort, and no live body stood on the floors only they led to). On by default (A206's bench, both teams with it
    /// against both without, 192 pairs: stuck cases -46 %, stuck seconds -56 %, falls off the route -10 %). The old value is wrong
    /// by the client (a walk-off its capsule cannot make: live bodies walked off none of the 1,567 cut walk-off gap drops), so the
    /// all-off arm keeps the default (`KEPT_RIGHT`). A grid is the map's (see `gap_links`).
    pub drop_room: bool,
    /// [A238] Whether the floor a body comes down on when it slips off a walk-off's take-off (slower than a walk-off is flown
    /// for) is reached in the maps' walk grids (`mapdata::Body::slip_landings`): the highest floor under the edge, where the
    /// walk-off's flight lands cells out. No link, so no route plans a slip; but a floor only a slip gets to is reached, so a
    /// pocket when no walk returns, which the pocket marks, the escape (and its minute's self-kill) and the bench's pocket counts
    /// then see. Vector Laboratory's 16.22 ledge under the 20.6 lip held 173-326 bot-s of the all-maps bench unseen by them
    /// (`fx-smoke-pockets.md`, #980's bench). Off (the default) until its bench. A grid is the map's (see `gap_links`).
    pub slip_landings: bool,
    /// Whether the maps' walk grids ask a sprint jump's run-up (1.5 m straight behind its take-off) after the erosion, of the
    /// take-offs and floors it leaves, in metres (`mapdata::Body::run_ups_after_erosion`), not before it: the erosion makes
    /// jumps of its own (a jump past a floor that went starts a cell back) and takes floors off a run-up's line, so checked before
    /// it 366 of the offered maps' 3,716 sprint-band jumps over 1.05 m keep less than 1.5 m straight behind them, 290 of them
    /// jumps the erosion made (A236); the motor gives such a jump up for no clear run. Off (the default) until its bench. A grid
    /// is the map's (see `gap_links`).
    pub run_ups_after_erosion: bool,
    /// Whether the maps' walk grids measure a walk up off perched steep ground (a by-slope floor over the clients' 55 degrees whose cell's
    /// centre stands over ground more than 0.3 m lower) from where the body stands: the walkable ground its climb began on, or the ground
    /// under the floor if higher, whatever the floor it reaches; a landing on such a floor gives it no base (`mapdata::Body::steep_walk_base`):
    /// the client's body does not rest on such ground. Mamayev Kurgan day 4's boulder floor (-2.90, 29.58, 105.38), a drop's landing,
    /// walks 0.58-0.76 m onto the top; a live bot walked onto the boulder and held forward for 5 s with no jump, its feet sinking to
    /// 29.00, 0.9-1.2 m under the top, the walkable floors its climb began on (A237, PROVEN). On by default (A248's bench: routes lost
    /// 614 -> 503, falls off 326 -> 283, stuck notes 178 -> 180; its no_way bursts have the same grid paths on and off); the all-off arm
    /// keeps it (its old value is the proven walk). A grid is the map's (see `gap_links`).
    pub steep_walk_base: bool,
}

impl Default for GridTuning {
    fn default() -> Self {
        Self {
            gap_links: true,
            floor_paths: true,
            client_numbers: true,
            exact_erosion: true,
            honest_drops: true,
            walk_off_lips: true,
            landing_carry: true,
            keep_last_resorts: false,
            crouch_floors: true,
            jump_cap_m: Positive::of(mapdata::grid::STANDING_JUMP_M),
            capsule_steps: false,
            drop_room: true,
            slip_landings: false,
            run_ups_after_erosion: false,
            steep_walk_base: true,
        }
    }
}

/// How fast a body walks off a ledge, m/s: the walk (3.5, `kin`); the bots' motor walks planned drops (`Feel::drop_walk`).
pub const WALK_OFF_MPS: f32 = 3.5;

impl GridTuning {
    /// The body the maps' grids are built for.
    pub fn body(&self) -> mapdata::Body {
        let body = if self.client_numbers {
            mapdata::Body::client()
        } else {
            mapdata::Body::default()
        };
        mapdata::Body {
            erosion: if self.exact_erosion { mapdata::Erosion::Exact } else { mapdata::Erosion::Cells },
            drop_flight_mps: if self.honest_drops { WALK_OFF_MPS } else { 0.0 },
            max_jump: if self.client_numbers { self.jump_cap_m.get() } else { body.max_jump },
            walk_off_lips: self.walk_off_lips,
            landing_carry: self.landing_carry,
            keep_last_resorts: self.keep_last_resorts,
            crouch_only_floors: self.crouch_floors,
            capsule_steps: self.capsule_steps,
            drop_room: self.drop_room,
            slip_landings: self.slip_landings,
            run_ups_after_erosion: self.run_ups_after_erosion,
            steep_walk_base: self.steep_walk_base,
            ..body
        }
    }

    /// The cap is a standing jump's: not above the exe's top, not below a step. A cap above the exe's 1.05 m (the client
    /// test G-V4 may find the body reaches 1.08) needs this bound moved, and the band `WalkGrid::drop_short_run_ups` keeps
    /// without a run-up (it ends at the exe's top), with a test: it is a code change, not a number.
    pub(super) fn check(&self) -> Result<(), String> {
        let cap = self.jump_cap_m.get();
        let (low, high) = (mapdata::Body::client().step, mapdata::Body::default().max_jump);
        if !(low..=high).contains(&cap) {
            return Err("grid.jump_cap_m".to_owned());
        }
        Ok(())
    }

    /// The off-mesh links of the maps' grids.
    pub fn off_mesh_links(&self) -> mapdata::OffMeshLinks {
        if self.gap_links {
            mapdata::OffMeshLinks::Gaps
        } else {
            mapdata::OffMeshLinks::None
        }
    }
}
