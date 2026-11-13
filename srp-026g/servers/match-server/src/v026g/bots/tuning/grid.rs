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
    /// [A347a] Whether the maps' walk grids keep a walking jump onto a short top by its press window and give the route its
    /// latest press (`mapdata::Body::press_window`): the clip's root end (2.67 m, PROVEN) within the top's far edge plus the
    /// stand radius 0.38 x sin 40 deg (a still body's slide, exe 0x7bc20; PROVEN), replacing the 0.2 m GUESS; the motor backs
    /// off from a press later than that (Vector Laboratory's 0.905 m ledge: down from presses 0.15-0.38 m before the take-off,
    /// up from 0.53-0.63; the window's least is 0.55). Client-proven: ON, and the all-off arm keeps it (`KEPT_RIGHT`). A grid is
    /// the map's (see `gap_links`).
    pub press_window: bool,
    /// [A347] Whether the maps' walk grids keep a pocket's exits within what every clip of the client's sprint jump reaches (the
    /// lowest clip top 1.282 m, PROVEN, `mapdata::grid::SPRINT_JUMP_RISE`), as the bots' own jump reach does
    /// (`mapdata::Body::exits_in_reach`). Off: up to 1.35 m (the highest clip): Chemical Plant day 2's ground-floor pocket kept a
    /// 1.316 m sprint exit that bots pressed and never went up (3 of 3, TOO_LOW; the A347 census of 48 matches). It also puts
    /// the exits A270's test refuses (a wall over the landing on the way, no run-up for a sprint exit) last in the escape's
    /// tries, one try each (`mapdata::ExitReach::Walled`): the same pocket's 0.633 m exit through a wall 2.9 m tall was tried
    /// first and failed 16 of 16 times (UP_THEN_DOWN; no grid link there, only the escape jumps it), and dropping them left a
    /// shut-in bot no try where a walled exit had got one out by a slide. Client-proven: ON, and the all-off arm keeps it
    /// (`KEPT_RIGHT`). A grid is the map's (see `gap_links`).
    pub exits_in_reach: bool,
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
    /// [A364 stall census] With `walk_every_link`: whether the walk check counts a refusal under a low ceiling as the client's
    /// stall and cuts a link two of whose lines are so refused (`mapdata::Body::walk_ceiling_stalls`; the slide 0x7DF20 is
    /// ported, the refusal is the exe's own verdict). Off: such links pass and bots stall crouched on them (Rudnya, 11 stalls
    /// in the owner's 14 recordings). Client-proven, so ON and kept by `all_off`. A grid is the map's (see `gap_links`).
    pub walk_ceiling_stalls: bool,
    /// [A364 C2 rows 11/12] Whether jumps up a ledge or across a gap to a top over what the Jump and gap links reach are made, each
    /// flown on the exact tick over the presses a client can make (walking and sprint, the gap links' leads) and kept as a GapJump
    /// carrying the press that lands (`mapdata::Body::rim_jumps`): a run-up jump's capsule rests on a top's edge as the clip comes
    /// down and the ground steps climb the rest (exactness's Chemical Plant flights, 143 of 424 sprint presses onto a deck 1.20-1.67
    /// m up; the flyer matches the rig's flights to 2 mm). Off: no jump to such a top, though players make it. Client-proven, so ON
    /// and kept by `all_off`. A grid is the map's (see `gap_links`).
    pub rim_jumps: bool,
    /// [routes corpus] Whether a jump onto a top too narrow to walk on along the jump lands where a wall beyond the top stops
    /// the body (`mapdata::Body::wall_held_tops`: a window's sill, a ledge along a wall; a fence's top with a drop beyond stays no
    /// landing). Off: no jump reaches such a top, and places real players stand on many times are out of the bots' reach
    /// (School's sill at 41.9 21.7 142.9: 144 visits, 89 players). The map's geometry, so ON and kept by `all_off`. A grid is
    /// the map's (see `gap_links`).
    pub wall_held_tops: bool,
    /// [A318] Whether the floors the erosion rounds took beside a wall the body clears are given back in the maps' walk grids
    /// (`mapdata::Body::wall_give_back`): the exact pass keeps them, a wall face stands within 0.6 m, and the client's walk tick
    /// stands a body there (A318: the rounds took the floor under the client in 376 of the drift lab's grid holds, the exact
    /// pass alone keeps 346). Client-proven (the walk tick stands the body there), so ON and kept by `all_off` (the owner,
    /// 2026-11-05: correctness fixes go in). The reach guard keeps it from opening regions no player reaches. A grid is the
    /// map's (see `gap_links`).
    pub wall_give_back: bool,
    /// [A325] Whether the grid's walk-tick passes (`client_controller`, `drop_check`) run the client's controller with the exe's
    /// own collision rules (`mapdata::Body::exe_collision`): its Bullet caster and its wall normal by rays, both read in the exe
    /// and proven closer to it in Unicorn (A325 P1 trust table: walk wall-slide divergences over 5 mm 11 -> 3 of 235 sampled
    /// ticks, the floor-seam miss gone; fx-smoke's A325 tail stalls: the exe walks on at 3 of 4 where the port stalled, and
    /// this rule follows it within 1 mm). Exe-proven, so ON and kept by `all_off` (the owner, 2026-11-05: correctness fixes go
    /// in). A grid is the map's (see `gap_links`).
    pub exe_collision: bool,
    /// [A329] Whether `client_controller`'s walk test starts a body where a standing body can be: out of every steep face its
    /// capsule overlaps, from the feet up, by up to its radius (`mapdata::Body::walk_start_clear`, the client's push out of
    /// penetrations 0x763D0). Off: out of the faces above the step only, so a floor beside a ledge lower than the step starts the
    /// body inside it, the exe pops it on top, and the pass keeps climbs no client makes (fx-gear-2's step-down cases: level_04
    /// day_4, a 0.785 m ledge the client and the exe stop at; the exe climbs a vertical face walking up to 0.75 m). Exe-proven
    /// (0x763D0), so ON and kept by `all_off` (the owner, 2026-11-05: correctness fixes go in); A329 nav-quick2-v3 with
    /// `crack_bridges`: client-test 2.87 -> 2.79, live 6.30 -> 6.25, every map better. A grid is the map's (see `gap_links`).
    pub walk_start_clear: bool,
    /// [A329] Whether `client_controller`'s bridges also cross a crack the grid crossed by a drop (`mapdata::Body::crack_bridges`):
    /// a one-cell crack between floors within a step, which the client walks over, gets the walked bridge, not only the drop
    /// `drop_check` takes away when the body never drops there (level_06 day_2 (18.4, 103), live: with the exe's start-overlap
    /// contact the drop is taken away and the floor had no other way across). Client-proven (the client's walk tick crosses it),
    /// so ON and kept by `all_off` (the owner, 2026-11-05: correctness fixes go in); A329 nav-quick2 alone: live level_06 back
    /// to its base (+0.06 -> +0.00). A grid is the map's.
    pub crack_bridges: bool,
    /// [A329] Whether `exe_collision`'s Bullet caster puts a contact met already overlapping at the start at the face's point
    /// nearest the body's core (`mapdata::Body::start_witness`), as the exe's caster takes the penetration solver's witness on
    /// the face; off, at GJK's zero point, which the move test's top check (0x85F50) reads as a hit at the feet (level_08
    /// Cologne corner: fx-smoke's reference, ticks 7 on, the exe walks under an overhang the port stopped at; with it fx-smoke's
    /// 1 m+ stalls there 68 -> 32). Exe-proven (Unicorn), so ON and kept by `all_off` (the owner, 2026-11-05: correctness fixes
    /// go in); A329 nav-quick2 with `crack_bridges` and `walk_start_clear`: every map better on both columns. A grid is the
    /// map's.
    pub start_witness: bool,
    /// [A331] With `wall_give_back`: whether a floor the rounds took also comes back where the exact pass takes it at its
    /// cell's centre but the client's push out of walls (the standing cylinder and its margin: 0.42 m off a wall, Unicorn) sets
    /// a body clear inside the cell and the walk tick stands it there (`mapdata::Body::give_back_in_cell`). fx-zones' grid edge
    /// cuts (the reckoning held at a cell edge the client walks across): 184 of the 201 rounds-taken floors under them. OFF:
    /// nav-quick2-v5 on ac2d1b16 improves both totals (client-test 2.76 -> 2.73, live 5.80 -> 5.75) but three maps go over
    /// +0.05 (client-test level_05 +0.06, live level_06 +0.07: a given-back walk up 0.55 m in one cell, live level_07 +0.07).
    /// A grid is the map's.
    pub give_back_in_cell: bool,
    /// [2a] Whether a fall in the maps' walk grids stops at any ground top under the body, not only at a kept floor
    /// (`mapdata::Body::fall_stops_on_solids`): the gap links' flights and walk-offs, and the plain Drop links (a floor's only
    /// way in or back is kept). The erosion takes the strip a radius wide beside every rail and wall, and the fall test asked
    /// for floors only, so it went on through the slab: owner-14 report row 2, Vector Laboratory's spawn platform (rail top
    /// 25.16, slab 24.26, "landing" 20.76): 0 of 95 presses by 18 bots got down, the exe's own tick rests the body on the slab
    /// at 24.258 (VERIFY-3 §1); 113 of 124 pressed drops of 2 m+ in the 14 matches were on such links, 139 stuck episodes
    /// (1,058 s). Exe- and recording-proven, so ON and kept by `all_off` (a link the client cannot take is wrong by the game).
    /// A grid is the map's.
    pub fall_stops_on_solids: bool,
    /// [2c] With `client_controller`: whether its walk check walks every Walk link a body may meet trouble on (a face no body
    /// walks on near either floor, a rise of 0.15 m or more, a floor with no standing room), crouched where the route
    /// crouches, from the floor and on from where the link before along the same way left the body, kept only where the
    /// body arrives on two lines of three or comes onto the far floor's level walked on (`mapdata::Body::walk_every_link`);
    /// of the failing links only a riser the body stalls under is cut, where a walking way round it exists and no floor
    /// loses its only way (or only walking way) in or back; the risers kept so stay, marked (`WalkGrid::walk_marked`). Off: links beside a face over 55 deg only, one at a time from a start
    /// pushed out of the faces, kept at half the way: owner-14 report row 12 (the client's own tick stalls within 0.5 m of
    /// the body in 924 blocked worlds, 1.1 % of calm ones; Mamayev Kurgan's rock step passes its check from the pushed start
    /// at 1.00 and gets 0.00 from where a body stands; the exe's tick stalls there as recorded, VERIFY-3 §3). Client-proven
    /// (the ported tick, Unicorn), so ON and kept by `all_off`. A grid is the map's.
    pub walk_every_link: bool,
    /// [A364 P6] Whether the maps' gap links fly a jump's game air phase (`mapdata::Body::jump_air_steps`): twelve steps from
    /// the clip's take-off, the short jump ending at frame 22.5 (2.50 m on, 0.35 m up) and falling from there, as the bots'
    /// reckoning flies it since #1655 (`jump_arc::AIR_STEPS`). Off, the builder flew the clip to frame 24 (2.67 m, landing at
    /// the take-off's height): links built on a longer flight than the client makes (with the client-true stand-in, missed
    /// jumps 21 -> 32, z 2.6). Exe-proven (A364's traces, 20 of 20 jumps), so ON and kept by `all_off`. A grid is the map's.
    pub jump_air_steps: bool,
    /// [A333] Whether a route's straight legs (the pull, `path.rs`) cross a floor nearer a wall than the client's body holds
    /// ([`PULL_HOLD_M`], the grid's table of wall clearances) only as a floor of the route's own path
    /// (`mapdata::WalkGrid::with_pull_hold`): a leg pulled past a wall's corner or along a wall keeps to the cells the search
    /// took. Off: the pull's line may graze a wall at the grid's 0.3175 m and cut corners there, where the client's body stops
    /// (A333: of the 278 stuck notes with the client-tick stand-in, 51 hard stops on pulled legs over several cells). Not a
    /// build input: the grids and their key are the same either way. OFF: harm-batch2 on the client-true stand-in (c7a6d66d)
    /// stuck 207 -> 194 and stuck s 2,111 -> 1,994, failed jumps 85 -> 67, but routes stuck 260 -> 326 and missed jumps 37 ->
    /// 62; nav-quick2-v6 the same (the lab replays clients, no routes).
    pub wall_clear_pull: bool,
    /// [A334] Whether a route's straight legs (the pull, `path.rs`) are walked by the client's walk tick (the grid's walker,
    /// `mapdata::WalkGrid::pull_walk_fails`: the controller pass's walk test with the exe's collision rules), and a leg it does
    /// not walk is pulled back a floor at a time to one it walks (or to the path's next floor). With the client-tick stand-in
    /// 93 % of the routes stuck are on pulled legs over several cells (fx-zones' route-stuck table on 65c6990b: 290 of 314).
    /// Only a risky leg is walked (a floor under the wall hold or a step), shortened to 11.4 m first, with the walks of a poll
    /// in a budget of walk ticks (`path::PULL_WALK_TICKS_A_POLL`): the worst poll is no worse than with it off. Not a build
    /// input: the grids and their key are the same either way. OFF: with the stand-in human pinned (#1460) the bots' own harm
    /// is worse at jumps (16 matches: jumps failed 52 -> 124, jumps wrong +35 %, stuck s +10 %); ON again only after that is
    /// root-caused and fixed.
    pub pull_walk: bool,
}

/// [A333] How near a wall the client's body stands, metres: its standing cylinder's radius 0.38 (the stand capsule's 0.76 m
/// diameter, `startup.cfg`, the controller's +0x60) plus the move test's margin 0.04 (`move_step_margin`, `startup.cfg`); the
/// port's `controller::tick` test `a_wall_stops_the_body_at_the_cylinder_and_margin` stops a body there as the exe does
/// (Unicorn, A313).
pub const PULL_HOLD_M: f32 = 0.42;

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
            press_window: true,
            exits_in_reach: true,
            gap_no_vaults: true,
            ramp_faces: true,
            client_controller: true,
            drop_room_walk_faces: true,
            narrow_doors: true,
            drop_check: true,
            walk_ceiling_stalls: true,
            rim_jumps: true,
            wall_held_tops: true,
            wall_give_back: true,
            exe_collision: true,
            walk_start_clear: true,
            crack_bridges: true,
            start_witness: true,
            give_back_in_cell: false,
            fall_stops_on_solids: true,
            walk_every_link: true,
            jump_air_steps: true,
            wall_clear_pull: false,
            pull_walk: false,
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
            press_window: self.press_window,
            exits_in_reach: self.exits_in_reach,
            gap_no_vaults: self.gap_no_vaults,
            ramp_faces: self.ramp_faces,
            client_controller: self.client_controller,
            drop_room_walk_faces: self.drop_room_walk_faces,
            narrow_doors: self.narrow_doors,
            drop_check: self.drop_check,
            walk_ceiling_stalls: self.walk_ceiling_stalls,
            rim_jumps: self.rim_jumps,
            wall_held_tops: self.wall_held_tops,
            wall_give_back: self.wall_give_back,
            exe_collision: self.exe_collision,
            walk_start_clear: self.walk_start_clear,
            crack_bridges: self.crack_bridges,
            start_witness: self.start_witness,
            give_back_in_cell: self.give_back_in_cell,
            fall_stops_on_solids: self.fall_stops_on_solids,
            walk_every_link: self.walk_every_link,
            jump_air_steps: self.jump_air_steps,
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
