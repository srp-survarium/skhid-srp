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
    /// for) is reached in the maps' walk grids (`mapdata::Body::slip_landings`): the highest floor under the edge open above the
    /// take-off (A247: no slab, plate or overhang between them), where the walk-off's flight lands cells out. No link, so no
    /// route plans a slip; but a floor only a slip gets to is reached, so a pocket when no walk returns, which the pocket marks,
    /// the escape (and its minute's self-kill) and the bench's pocket counts then see. Vector Laboratory's 16.22 ledge held
    /// 173-326 bot-s of the all-maps bench unseen by them (#980's bench), but its walk-off ran through the walkway's 0.97 m
    /// parapet, which `drop_room` now cuts, and the ledge lies under a plate (A247). On by default (A93's final sweep: the five A93 candidates on against the defaults, both teams, 576 pairs: stuck -5.2 %, kills +1.5 %, every guard level but A123's gap_failed_above, +45 % at paired z +1.8, in a few Vector Laboratory and Chemical Plant matches). The old value
    /// leaves floors bodies come down on unseen, so the all-off arm keeps the default (`KEPT_RIGHT`). A grid is the map's (see
    /// `gap_links`).
    pub slip_landings: bool,
    /// Whether the maps' walk grids ask a sprint jump's run-up (1.5 m straight behind its take-off) after the erosion, of the
    /// take-offs and floors it leaves, in metres (`mapdata::Body::run_ups_after_erosion`), not before it: the erosion makes
    /// jumps of its own (a jump past a floor that went starts a cell back) and takes floors off a run-up's line, so checked before
    /// it 366 of the offered maps' 3,716 sprint-band jumps over 1.05 m keep less than 1.5 m straight behind them, 290 of them
    /// jumps the erosion made (A236); the motor gives such a jump up for no clear run. On by default (A249's walks: one bot sent
    /// over each changed jump on its map, 188 starts both ways: the motor's give-ups for no run-up 16 -> 6, stuck notes 41 -> 31,
    /// the goals reached and the times the same; at the jumps it takes away the bots made the jump 21 times in 126 arrivals at the
    /// take-off). The old value offers jumps the client's body cannot make from their run, so the all-off arm keeps the default
    /// (`KEPT_RIGHT`). A grid is the map's (see `gap_links`).
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
    /// [A270] Whether the maps' walk grids count a pocket's exit toward its way out only where a body can take it: no wall above
    /// the landing between take-off and landing, and a sprint exit with a 1.5 m straight run-up inside the pocket
    /// (`mapdata::Body::pocket_exits_walled`). A pocket with none is shut, and no route is planned into it. Chemical Plant day 2's
    /// pocket counted exits through a 3.9 m wall and Cologne day 2's two-floor slot sprint exits with 0.25 m to run; routes dropped
    /// bots into both (fx-grid's A257). On by default (A93's final sweep: the five A93 candidates on against the defaults, both teams, 576 pairs: stuck -5.2 %, kills +1.5 %, every guard level but A123's gap_failed_above, +45 % at paired z +1.8, in a few Vector Laboratory and Chemical Plant matches). The old value counts exits a body cannot take, so the
    /// all-off arm keeps the default (`KEPT_RIGHT`). A grid is the map's (see `gap_links`).
    pub pocket_exits_walled: bool,
    /// [A293] Whether the maps' walk grids refuse a gap link down whose flight went over a face that had held the body
    /// (`mapdata::Body::gap_no_vaults`): a rail or a parapet at the edge with a floor more than a step under its far side. The
    /// flight let the body rise with the clip at the face and go on past it once its feet were over; the client does not:
    /// Vector Laboratory's spawn exits (a 0.9 m rail) kept every bot on top, 19 of 19 and 15 of 15 tries in the owner's match
    /// 0x8681e02, and the live bodies went down the maps' downward gap links 12 times in 2,741 visits (A293). A press that vaults
    /// down cuts every press and sprint link of its take-off and way (the far presses left would make a link over the same face).
    /// On by default (A293's bench, 192 pairs, whole matches: downward route jumps 447 -> 192, not down 383 -> 171, deaths within
    /// 2 s of one 9 -> 4, every guard level, CPU level). The old value offers jumps down the client's body does not make, so the
    /// all-off arm keeps the default (`KEPT_RIGHT`). A grid is the map's (see `gap_links`).
    pub gap_no_vaults: bool,
    /// [A304] Whether the maps' walk grids keep, by floor, the slope of a ramp face under it (`mapdata::Body::ramp_faces`,
    /// `WalkGrid::ramp_slope_deg`): a walker ramp laid over steps, which the client walks at cos of its grade (Cologne's stairs),
    /// told from real stairs with flat treads. A mark only, read by the reckoning's slope rule (`kin.slope_speed`); on with it. A
    /// grid is the map's (see `gap_links`).
    pub ramp_faces: bool,
    /// [A313] Whether the maps' walk links are decided by the client's own walk tick (`mapdata::Body::client_controller`):
    /// cracks and riser cells walked (in runs of `mapdata::grid::controlled::BRIDGE_MIN_RUN` side by side), links where the
    /// client's body stalls dropped. ON (A313, fx-zones bench4, with `kin.walk_bridges`, 324 pairs vs OFF on Cologne, Rudnya and
    /// Chemical Plant: no guard FAIL; missed jumps 23 -> 25 z +0.2, jumps pressed low -26 % z -2.2, delivered +1.0 %; search
    /// work +3.9 % z +2.2 the one warn). A grid is the map's (see `gap_links`).
    pub client_controller: bool,
    /// [A304] Whether `drop_room`'s walls count steep faces whatever their walker behaviour (`mapdata::Body::drop_room_walk_faces`): a
    /// parapet with `walk` faces then cuts the walk-off over it as any wall does (level 7's deck). ON (A304: the drift lab, all 15
    /// client-test recordings: level 7 -9.2 %, no other map changed). A grid is the map's (see `gap_links`).
    pub drop_room_walk_faces: bool,
    /// [A313] Whether the floors the erosion rounds took in a narrow door the body fits are given back in the maps' walk grids
    /// (`mapdata::Body::narrow_doors`): 6 doorways on the 8 maps the client walks through and the grid closed (Vector
    /// Laboratory's strip among them). ON and KEPT_RIGHT: the client's own walk tick takes a body through every such doorway both
    /// ways (`tests/narrow_doors.rs`), so the closed grid is wrong by the game (the owner, 2026-11-05: correctness fixes go in).
    /// A313 NAV FIRST, fx-bench's score on all maps: drift 4.02 -> 3.98 %, lower on every map, coverage +0.02, path equal;
    /// level_01 day_2's one raised-stall cell is real ground. A grid is the map's (see `gap_links`).
    pub narrow_doors: bool,
    /// [A313] Whether a Drop link is kept in the maps' walk grids only where the client's own walk tick takes a body down it
    /// (`mapdata::Body::drop_check`): a Drop is cut where the body walks on and never leaves the take-off's level (a landing under a
    /// deck that goes on, a rim the erosion took): 640-2,880 links a map, 4-8 %. ON and KEPT_RIGHT: the rule is the client's own
    /// walk tick, so such a link is wrong by the game (the owner, 2026-11-05: correctness fixes go in). A313 NAV FIRST, fx-bench's
    /// score on all maps: drift 4.02 -> 4.01 %, level_04 -0.11, no map worse; coverage, path and raised stalls the same. A grid is
    /// the map's (see `gap_links`).
    pub drop_check: bool,
    /// [A318] Whether the floors the erosion rounds took beside a wall the body clears are given back in the maps' walk grids
    /// (`mapdata::Body::wall_give_back`): the exact pass keeps them, a wall face stands within 0.6 m, and the client's walk tick
    /// stands a body there (A318: the rounds took the floor under the client in 376 of the drift lab's grid holds, the exact
    /// pass alone keeps 346). Client-proven (the walk tick stands the body there), so ON and kept by `all_off` (the owner,
    /// 2026-11-05: correctness fixes go in). The reach guard keeps it from opening regions no player reaches. A grid is the
    /// map's (see `gap_links`).
    pub wall_give_back: bool,
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
            slip_landings: true,
            run_ups_after_erosion: true,
            steep_walk_base: true,
            pocket_exits_walled: true,
            gap_no_vaults: true,
            ramp_faces: true,
            client_controller: true,
            drop_room_walk_faces: true,
            narrow_doors: true,
            drop_check: true,
            wall_give_back: true,
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
            pocket_exits_walled: self.pocket_exits_walled,
            gap_no_vaults: self.gap_no_vaults,
            ramp_faces: self.ramp_faces,
            client_controller: self.client_controller,
            drop_room_walk_faces: self.drop_room_walk_faces,
            narrow_doors: self.narrow_doors,
            drop_check: self.drop_check,
            wall_give_back: self.wall_give_back,
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
