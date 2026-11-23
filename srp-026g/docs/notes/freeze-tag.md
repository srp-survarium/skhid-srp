# Freeze Tag

The initial mode uses two teams and a best-of-nine match: first to five
round wins (the owner, 2026-11-21: "let's make it 5 rounds win"; it was
three). No artifact rescue in this version (owner, 2026-11-17).

- A lethal hit freezes the player where they were hit. Movement, weapons
  and interactions stop; the ordinary death teardown and ragdoll do not run.
- While frozen, the camera uses the existing pre-match orbit and mouse-wheel
  zoom around that body. Thaw restores the previous first/third-person mode.
- A living teammate within 2 metres (the owner, 2026-11-21: "closer a bit";
  it was 3), with a clear static-world ray between
  their bodies, thaws them after 4 continuous seconds (the owner, 2026-11-22;
  it was 3). Losing that proximity
  resets progress. Extra teammates do not accelerate it.
- Thaw restores health in place and clears input. It preserves equipment
  and ammunition. Frozen teammates cannot thaw each other.
- A thaw is credited as a kill (the owner, 2026-11-21: "kill rn is fine"):
  every fighting teammate within reach of the body (2 m, clear sight: the
  thaw's own test) at the step it thaws counts score event 14, one each
  (`Actions::Play::thawers`; the patch's `World::credit_thaw`, in its kit's
  cell, the scoreboard told, at the rule's tick in every game alike). 14 is
  the silent kill's number, which 0.26g counts nowhere. It is not event 0:
  a record's kills must add up to its weapons' (patch 165, the world
  check), and the Tab list's K/D is the kd rule's, which a thaw leaves as
  it is. srp weighs it as a kill in mode 12 (`static_info::
  FREEZE_TAG_WEIGHTS`: the scoreboard's 10) and pays it as one, in the
  results' kills line (`rewards`: `SRP_KILL`); quests' kills are the kd
  rule's and do not count it.
- A frozen player thaws by himself 120 s after his freeze (`AUTO_THAW_MS`),
  a stop gap (the owner, 2026-11-22: "2 min unthaw is good for now as a stop
  gap"), unless the round ends first: the same thaw as a teammate's
  (`Actions::Play::thawed`, his glow and camera with it), crediting nobody.
  The clock runs only while his body is in the world (a place that left
  stays frozen), and a teammate's progress is apart from it. His prompt
  shows the time left ("Frozen (1:42)").
- The freezer's kill is his always (the owner, 2026-11-22: "credit for
  freezing is given always"): the kd rule counts it at the lethal hit, and
  no thaw, a teammate's or the auto-thaw, takes it back or changes it.
- A team's last one fighting, every teammate of his frozen and two of
  them at least, while a round is in play, is on the other team's minimap
  and Tab map at all times, until a teammate of his is thawed or the round
  ends (the owner, 2026-11-22: "last member is always shown on the map").
  So only in a team of three or more (its players in the rule state,
  fighting or frozen): nobody is shown by it in 1 v 1, 2 v 1 (either side)
  or 2 v 2 (the owner, 2026-11-22: "when 1vs1 we see each other on the
  map, bad", "2vs1 should also not show", and asked whether the
  two-player side's last one stays hidden too: "yes"). The game's own
  spotted-enemy path: `minimap.enemy_spotted` (update_minimap_players'
  test of an enemy's spotted time, 0x239505) lists his dot as a spotted
  enemy's, as Artifact Hunt's carrier and a replay's enemies; from the rule
  state (`FreezeTag::last_standing`) each frame, the same in every game,
  nothing simulated. No marker over his head.
- Freezing the entire opposing team awards one round. Simultaneous wipes
  award neither team a point and replay the round. A place that left stays
  frozen, so a team whose players all left counts as frozen.
- A team frozen whole while the other team has nobody (a 1 v 0 private
  room, or the other side all left) draws: no point, the round replayed, so
  a player alone farms nothing and still gets a new round (the owner,
  2026-11-22: alone in a room he stayed frozen, no round came). Nobody
  entered on either side: nothing ends.
- **A round's end and the next round,** the same in Artifact Hunt and Freeze Tag by one code
  (`artefact_hunt_rule::Restart`, the exe's `round_countdown::respawn_tick`; the owner,
  2026-11-23: "I expect the round to end exactly in the same manner in both"):
  1. 3 s (`time_before_new_round`): play goes on, nobody respawns; the hud counts them down.
  2. If that was the match point, the match ends here (the rule's end, which the match server
     takes as it is).
  3. 1 s: the screen fades; at its end every player, alive or dead, respawns at his team's spawns
     (a round start: a life of everyone).
  4. 5 s (`round_countdown_time`): everyone stands at his team's spawns, his input held: the match
     start's countdown (the warmup camera, patch 31's kits; a kit switch is a life).
  5. The next round.
  A player who enters in any of them spawns at his entry (freeze-tag.md, "No place out of the
  world"); before the fade's end, it respawns him with everyone.
- In Freeze Tag's 3 s the frozen stay frozen and held, everyone else moves on, shoots and uses
  his items (the owner, 2026-11-22: "when one team in freeze tag wins a round it shouldn't stop
  moving", "only frozen people do"); no thaw progresses and no auto-thaw clock runs. A drawn
  round restarts the same way, with no point. The match point is a fifth round win. The
  match server takes a team at five as the winner from that round's 3 s on, so a time limit
  that ends the match in them is still its win.
- The existing match time limit still applies. If it expires before either
  team reaches five wins, the unfinished series is a draw.
- A disconnect cannot clear a frozen place. A new place starts on its first
  spawn in a round. Everyone who enters spawns at once, at his team's points,
  in every phase: after a round he moves on as the others who are not
  frozen, and the fade's end spawns him again with the round. A frozen player
  who leaves and comes back appears at his team's points, frozen there until
  a teammate or the auto-thaw thaws him: rejoining is no way out of a freeze
  (the release manager's pick, 2026-11-22). Absent players are excluded
  when a new round starts. ("No place out of the world" below says why an
  entry is never held.)
- The HUD reuses the stock TDM score bars (full at `WINS`, which the patch
  passes to `freeze_tag_score`; their native source, the hud state's team
  points +90h/+92h, which no Freeze Tag rule fills and nothing clears at a
  new match, the patch writes from the rule every frame, so the last match's
  score no longer shows: the owner, 2026-11-22, "freeze tag counter doesn't
  reset after prev game"), collection progress (its bar the progress
  of `THAW_MS`, which the patch passes to `freeze_tag_update`; a round's 3 s its line, the fade
  and the countdown Artifact Hunt's `RoundFade` and "Awaiting match start" with the time left)
  widget and blocked-hit UI sound. Its texts are `st_srp_hud_freeze_*`: the
  content compile gives the hud movie only `st_srp_hud_*` keys. A frozen player's name over his head
  keeps his name and is drawn ice blue (the owner, 2026-11-21: "his name
  should be of 'frozen' color, not just read frozen"), his arrow under it
  too, and a frozen teammate is ice blue on the minimap and the Tab map
  (the owner, 2026-11-22: "your teammates should be blue when frozen", "it
  should also be on map/mini map and game ui"); each gets its own colour
  back on thaw ("The frozen name" below). A rescuer sees progress only for a reachable frozen
  teammate. The frozen player's prompt is "Frozen" and the auto-thaw's time left
  ("Frozen (1:42)"), with the
  thaw's bar under it while a teammate thaws him, no percentage (the owner,
  2026-11-21: "just frozen text is fine"); the rescuer's sentence and bar as
  before. Freeze Tag's bar is 120 px long (`HUD.FREEZE_BAR_WIDTH`, about the
  native use's "Progress: 100%"), not its text's width: the native prompts
  keep theirs (the owner: "unthawing progress line is a bit too long").
- A frozen body glows: the Spring artefact's activation glow in ice blue,
  fainter (the owner, 2026-11-21: "effect can be anomalous artifact spring
  activation but of slightly different hue and less intense"), shrinking as a
  teammate thaws him ("The frozen glow" below).

The first authored map is Rudnya day, using its existing TDM project and
team spawns. `freeze-tag-1.json` makes its config and fills unused map-dictionary
slot 114. Its `ui.icon` is 4: the loading screen's mode icon is Freeze Tag's own
(the owner, 2026-11-21: "freeze tag has tdm icon"), the lobby movie's
`SrvGameModeIcons` frame 5 (`GameModeIconFreezeTag`: a snowflake in the frozen
name's ice blue on Artifact Hunt's round badge, frame 4), where it showed Team
Deathmatch's skull; the loading screen is the only place the lobby draws a
mode's icon. Mode 12 is separate from FFA variations 9–11 and from Artifact
Hunt's 13. Native rules are 93, 91, 94: time, respawn, and kill counters; no
TDM ticket rule. The options carry `SRPX`, tag 6 (4 is the wide map id, 5 the
faces), length 1, version 1, among the block's other entries. Both client and
server understand it.

Every other team map has it too (the owner, 2026-11-21: "make sure freeze tag
is playable on all non FFA-only maps"): each level and weather maps_dict gives
a battery, Team Deathmatch or Research config, 50 more, each config made as
Rudnya's from the map's own Team Deathmatch config (`freeze/<its name>`), maps
400-449 (`freeze-tag-maps-1.json`, made by `scripts/mode-maps/make.py`, which keeps each
map's id in `ids.tsv` beside it: a new map's is the next free one, an id never moves; the
list: `artefact_hunt.md` "Maps"). Free for All's maps alone (the STALKER maps)
have none. Training Camp has its own (maps_dict 450, `training-camp-teams-1.json`,
which `make.py` leaves to it), made the same way from its Team Deathmatch config,
on a project whose team spawns are Free for All's 47 points for both teams
(`stalker-ffa.md`, "Team Deathmatch and Freeze Tag at the Free for All points").

## State and native lifecycle

`srp-validate::world::freeze` owns the deterministic transition and codec.
The rule saves 191 bytes after the native respawn state: version (3), phase
(0 play, 1 the 3 s after a round, 2 the fade, 3 the countdown, 4 the match
over), two scores, the ended round (1 or 2 a team's, 3 a draw; the match's
winner once over), a time (u32: the restart part's deadline, the match's end;
0 in play), the round starts (u16: the fades' ends after the match's first, a
double wipe's replay too), each place's kit switches in a round's countdown
(u16 x20), then twenty seven-byte player records (waiting/fighting/frozen
with team, thaw time (u16), auto-thaw time left (u32, 1 ms to 120 s, a frozen
record's only)). The round starts and the kit switches are
`artefact_hunt_rule::Lives`, Artifact Hunt's own. The parser checks the
closed domains, roster extent (no record nor kit switch past the match's
places), phase/score agreement (a team at five only in its last round's 3 s
or over), the ended round against the records (unchanged from the round's
end to the fade's end; every record waiting in the countdown) and the round
starts against the rounds ended: every one ended (a win in the scores, or a
draw) was restarted but the one whose fade has not ended and the match's
last, so the draws they leave are not negative, and one at least in a
draw's restart. Against the world's time (the timelimit's, `rules::
freeze_tag_fault`): a restart part's deadline is after it by its whole
length at most, the match's end at it or before. Version 2 (revision 71's
5 s pause and its Restarting) is not read: its worlds are the rule before
this one. Rollback and late join restore progress, round score and the
counts.

The match server pays lives by it: `MatchResult::round_starts` (the field
and its use in `rewards.rs` as Artifact Hunt's branch adds them) is the
last trusted world's count (`Game::round_starts`), and `autobuy::used`
counts each round start as a life of every player, so a match of restarts
charges the kits' reloads.

The three native hooks are declared in `sites_026g.tsv`:

- `2DAA60` (`freeze.player_tick`), `base_player::tick`: a frozen player's
  clock advances, the gun in hand's too (its `m_last_tick_time` +5A4h: its
  first tick after a thaw stepped the whole freeze at once, and the recoil's
  spring, 0x331A50, pinned the kick of a shot fired before the freeze at -1
  for seconds: the view moved by itself after a thaw, the owner 2026-11-22), but the deferred death callback and tick are skipped
  through the native `ret 4` at `2DAB69`. Lethal hits already set
  `pending_death` (+33Dh), which also prevents further damage in `hit_impl`.
- `2D86E0` (`carry.input`, patch 78's site, order 0 before its handler at 1):
  `set_input`'s entry. A held body's input leaves through the function's own
  `ret 8` at `2D8A0C` (exit `held`): nothing of the player changes, the
  native "nothing new". Patch 78's take and use never see it.
- `316826` (`respawn.tick`, order 0; Artifact Hunt's handler is 1), after
  the respawn tick's prologue and patch 31's kit-change jump, before the tick
  reads any place's time: Artifact Hunt's path
  (`round_countdown::respawn_tick`, with the rule as it stood before this
  step): in a round's countdown patch 31's kit switch where the player
  stands (`FreezeTag::kit_switched`, a life); the places
  `FreezeTag::voided` names lose their native time: every frozen or lethal
  body, whose death queued a respawn (the hud showed it as "respawn in 8
  seconds": the owner, 2026-11-22), so only a round's start spawns one; then
  `FreezeTag::spawning`: the 3 s and the fade hold a place due, the fade's
  end makes every place with a current player due at this step, and the
  tick's own `31697F..3169BE` takes each out and spawns it. A frozen body
  without a free point at that step is not tried again: the native 50 ms
  retry (`3169D8`) is voided with the other lethal bodies' times, so it stays
  where it is, frozen, and starts the new round frozen (a teammate or the
  auto-thaw thaws it); it takes a team with more players than free points at
  that step, which the mode's maps are not meant to have (the owner,
  2026-11-23: "it is fine since we don't support too little spawns"). No void or
  hold touches an entering place (`RespawnTick::void`, `hold_due`): its entry
  spawn runs in every phase. Then the rule's step. No synthetic deaths.

The typed `vostok::freeze` boundary uses generated fields and engine calls.
Thaw calls the same `damage_model::reset` (`2E5290`, EDI receiver, stack time,
`ret 4`) as native spawn at `2DAD32..39`, then clears pending death/actions
and requests animation selection. The line-of-sight test uses the existing
physics ray (group 24h, mask 2), chest to chest.

`tools/check freeze-native` exercises installed hooks and round scheduling
under Unicorn. Shared-rule tests cover continuous thaw, wall/enemy refusal,
disconnects, draws, 3–0 and 3–2 finishes, codec rejection, rollback during
intermission and the schedule of each phase. Server tests cover map rules,
mode registration and the native rule envelope. `tools/check` also selects
the world corpus/drift and existing world-roundtrip checks.

The presentation hook after the simulation step borrows the native warmup
camera with `game_world::switch_to_warmup_camera` (`1F8190`). The existing
input handler, `process_warmup_mode`, camera view, collision and zoom limits
are reused. It restores `switch_to_player_camera` (`1F8200`) on thaw or match
end and clears pending input on both transitions. It owns no pregame,
spectator or replay camera, and retains only an identity and previous mode;
camera state does not enter the world format.

## No place out of the world

The crash of 2026-11-22 (revision 69, every game of the match at the same
step: the owner's Wine dump and a Windows player's report, the same stack):
place 7 left at 124 900 ms and came back into its place at 144 000 ms, in
the pause. The pause cancelled its entry spawn (`CancelAll`; in play
`CancelFrozen` did the same for a frozen place that came back), so the place
was in the match with its player out of the world. 0.26g never has that
across a saved world: an entry spawns at its entry's step, a death keeps its
body in the world. Every world load (`game_world_core::deserialize`) puts
every active place's current player into the world, one already there with
link 0 (0x2D2D51: `player::insert(0)`, +33Fh 1 only: no models, no player
effect +F540h, no tick lists; its time stays the loaded world's, 146 950
in the dump). The restart's native removal (vtable +24h with 1) then
removed a model not in the scene (`remove_model`'s `m_in_scene` assert) and
stopped the null effect: `game_effect_player::stop` read 0 at 0x2DE36D. The
respawn rule's leave handler (0x316BE0: time -1, then the current player out
with 1, unasked) reads it the same, with no load at all, if such a place
leaves again.

So an entry is never cancelled or held: Freeze Tag's `World::cancel` leaves
a place whose current player is out of the world (`Seat::Entering`) to the
native tick, which spawns it at its entry, and Artifact Hunt's `hold_due`
skips it the same. Every game, every load and every leave then sees each
place of the match in the world, as the stock game does.

## The frozen name and the frozen glow

**The name.** The player icons movie (`flash/player_icons`, HUDMarker and
HUDIconLayer) draws a frozen place's name (the marker's `textField`) and his
arrow under it (the icon's parts named `renderer`, which the game's colour
paints) in `HUDMarker.FROZEN_COLOR`, ice blue, while the place's bit is set in
`root.freeze_tag_markers(mask)` (sent each frame by `freeze_tag::on_hud`, 0
when another mode follows). The marker keeps the colour the movie and the game
give the name and the arrow (its own, the team's by `render_name`'s `rgb` or
`set_player_color`) and puts it back exactly on thaw, the newest one if the game
set another while frozen; a name set while frozen stays blue (names are plain
text), and so does an arrow of another frame (`type`); a marker made while its
place is frozen is blue from the start. The native label is the game's, never
written over. An enemy's marker shows only in a replay (and as Artifact Hunt's
carrier): there a frozen enemy's name was already blue, and his arrow follows
his name.

**The hud.** `freeze_tag::on_hud` sends the same mask to the hud movie,
`root.freeze_tag_rows(mask)` (the name is the first use's; the exe is
unchanged), which the hud reads once into a `FrozenPlaces`
(`survarium.modes.FrozenPlaces`: the places and `COLOR`, the same ice blue as
the icons movie's; `freeze-names` checks they are equal) and gives the Tab list
and both maps; each asks it whether a place is frozen.

- The Tab list (the owner, 2026-11-22: "frozen players should also be colored
  blue when tabbing"): `PlayerList.frozen` draws the name of the row whose id is
  a frozen place in the blue, either team's; the row is drawn again from its
  data on thaw, so it has its own colour back.
- The minimap and the Tab map (the owner, 2026-11-22: "your teammates should be
  blue when frozen", "it should also be on map/mini map and game ui"): a frozen
  teammate's dot (`Minimap`, the players of `update_players` whose team is the
  local player's, himself excepted) has its glyph in the blue, as the squad's
  dots have theirs green (the upper of the frame's two parts named `renderer`,
  `MinimapObjectPlayer.frozen`), a squad mate's and a battery carrier's too (the
  carrier keeps his mark), and his name on the Tab map (Alt) in the blue; the
  glyph's own colour (white, the squad's green) comes back on thaw. An enemy's
  dot (shown only by the spotted rules) is as it is. A Tab map held (the list
  closed) draws it with its next update when it shows again.

The hud clears the mask at a new match (`reset`, which also drops the last
match's players from the maps) and another mode (`set_game_type`).
`tools/check freeze-names` (`scripts/freeze-tag-hud`) checks both movies in
Ruffle at 1920x1080 and 1280x720.

Not blue: the kill feed, the voice chat's speakers and the chat. Each line of
them is an event or a message with a name and a side from the exe, no place
(`on_player_killed` 0x237C70; `srp_voice_talking`): a frozen state there would
need the exe to send the place, or names matched to the Tab list's rows, and
the kill feed's victim is the one just frozen anyway.

**The frozen glow.** `fx/artifacts/human_frozen` (content pack
`freeze-tag-glow`, `scripts/freeze-tag-glow/make_pack.py`): the stock Spring
glow `fx/artifacts/human_spring` with its three blue materials drawn with ice
copies (`fx/artifacts/<name>_ice`, constant_emissive (0.62, 0.88, 1.0) x
0.6), every emitter's rate and maximum x0.6 and its point light ice blue at
half its intensity: the owner's pick of the previews' variants ("a is good",
2026-11-21). Spring's own files are unchanged.

It covers the whole body (the owner, 2026-11-23: "just make the glow cover the
whole body, not just legs"). Almost every emitter of the Spring glow is
body-shaped (an animated domain: points on chosen bones), and the engine takes
those bones from the skeleton the play is given. The first build played it with
two empty attachments, so every such emitter started at the matrix's origin and
the glow was a small light at the feet. It is now played as the effects'
presenter plays a living player's Spring glow (`particle_game_effect_presenter::
play_effect` 0x2AC290): with the frozen player's hit model (+498h, the
`collision::animated_object`) and his model (+F290h, the
`render::skeleton_model_instance` that `particle_engine::get_bone_matrix` reads)
as `scene_renderer::play_particle_system`'s (0x22660) last two arguments (the
0.21d PDB's signature). The play holds them, and a later move changes only the
matrix, so each place's glow remembers the player object it was played on and is
played again on another one (`GLOWING`). `tools/check freeze-native` checks the
play's attachments, the move and the removal.

The thaw shrinks it (the owner: "would be nice if the glow was reducing as the
player was unfrozen", "so others could deem the freeze state by look"; of the
first build's four strengths swapped a quarter at a time: "it just disappears a
few times without becoming smaller"). One system plays; every frame its
instance's matrix is the body's with its axes scaled by 1 - 0.85 x progress /
the thaw time (`THAW_MS`, 4 s) about a point 0.9 m over the feet, so 15 % at
the thaw, and full again if
the progress resets. Swapping systems blinked: `remove_particle_system_instance`
(0x229D0) drops an instance's particles at once and the next starts empty.
What a scaled matrix does (0.10b source, `particle_emitter_instance`,
`v_particle_sprite`/`particle_vertex_input.h`; 0.26g forwards the matrix
unchanged, `update_particle_system_instance` 0x22B30): an emitter in the
instance's space (all of this glow's but `distortion`) places every particle,
spawned and alive, through the matrix each frame, so the cloud's spread and
motion scale at once; a world-space emitter (`distortion`) takes it only at a
particle's birth. A sprite's size is added after the matrix (view-aligned,
`input.size`), so the sprites keep their size, and the point light keeps its
intensity: the glow gathers to the body's middle rather than dims. On his
bones, a particle is born on the body at whatever the scale; whether the cloud
still gathers to the middle through the thaw is to be seen in a match. The
pack's `human_frozen_70`, `_40` and `_15` and their materials are no longer
played (to drop from the pack in its next version).

Each game draws it for itself, never in the simulation: an effect added to a
body's simulated effect list at the first tick after a game loaded it enters
each game's world at another tick (Artifact Hunt's carrier glow desynced so,
2026-11-21). After the simulation's move (`game_world.after_move`, the
frame hook, order 6) the game plays, moves (`update_particle_system_instance`
0x22B30, on the body's simulated transform, scaled as above) and stops one instance a place in
the world's scene, from the world's rule state: every frozen body of a match in
play, at its progress's size, so every game, a rollback and a late join
show the same. It stops at the thaw, the round's restart, the match's end and
another match, and everything of it leaves the scene with the world
(`game_world.deactivate`, order 4). Each of the 20 places has an instance of
its own (a scene plays an instance once): 20 systems in one query at the
first frame of a Freeze Tag match. The frozen player's camera is the orbit
around his own body, so he sees the same particles; there is no first-person
twin.

## With Artifact Hunt

The owner: Freeze Tag "will be similar to artihunt with
respawns". Artifact Hunt (mode 13, patch 171, `artefact_hunt.md`
"Respawns") also respawns everyone, the living too, at a round's start
through the same tick, and holds respawns while a round ends. The two share
two sites, each handler acting only in its own mode (Freeze Tag's from the
options' tag 6, Artifact Hunt's from its rule of the world):

- `respawn.tick` (0x316826): one row (named for the respawn rule, not for
  one mode; it was `freeze.rule_tick`), Freeze Tag's handler order 0,
  Artifact Hunt's order 1.
- `carry.input` (0x2D86E0) with its exit `held@0x2D8A0C`: Freeze Tag's
  handler order 0, patch 78's order 1 (which asks Artifact Hunt first).

Taken from Artifact Hunt, and why:

- **Held, then everyone due at once** (Artifact Hunt's `Spawning`): what falls due in a round's
  3 s and its fade is held, the fade's end makes every present place due at the step, the native
  tick removing and spawning them; a frozen or lethal body's own native time is void in every phase
  (`FreezeTag::voided`), so no respawn countdown shows for it. Neither holds an entry ("No place
  out of the world").
- **The input at set_input's entry.** Freeze Tag dropped a held body's input
  at 0x2D86EE, after patch 78's handler at the entry, which still took an
  artefact (from the ground or a teammate's kit) on a frozen player's Use.
  At the entry, before patch 78, with Artifact Hunt's `held` exit, nothing
  of a held input is applied; the separate `freeze.input` site is gone.
- **A round's end and start** (since 2026-11-23, the owner: "is this exactly the same code that
  runs for arthunt?", "consistency is paramount"): Artifact Hunt's restart, one machine
  (`artefact_hunt_rule::Restart`: its parts, its transition, what it says of the spawns, the
  input hold and the round's countdown; `Lives`: the round starts and the countdown's kit
  switches) and one exe path (`round_countdown::respawn_tick` over `RespawnTick`'s
  `switch_kit`, `void`, `hold_due`, `everyone_now`; `round_countdown::set` for the core's
  time state 4, which the client time rule's camera test reads as the match start's countdown:
  everyone's warmup camera, whose input holds, and patch 31's kits). Freeze Tag keeps what is its
  own: what decides a round, the frozen (held, their native respawns void), the thaw.

Freeze Tag's own pause before it (5 s in which only the frozen saw the countdown, then the
restart at once, the round waiting for every spawn; the final win ending the match at once and
the server 2 s after it) is gone: the 3 s, the fade and the countdown are Artifact Hunt's for
everyone, and the match ends at its rule's end as Artifact Hunt's. A frozen body that gets no
free spawn point at the round start is not tried again: it starts the new round frozen where it
was (above, `respawn.tick`).

## Validation

Played by the owner in three local tests on 2026-11-21 (three windows,
Rudnya day; the matches record into the world corpus):

- Test 1: the hud showed raw keys (`#st_freeze_*`: the hud movie gets only
  `st_srp_hud_*`), the thaw reach was too far (3 m, now 2), the glow blinked
  out in steps (now one system, shrinking), three wins were too few (now
  five), and a third win did not end the match (the server took the
  standing through the batteries' plausibility test; now its own arm).
- Test 2: the TDM icon on the loading screen (now its own), the thaw bar the
  sentence's width (now 120 px), the frozen player's text (now "Frozen");
  the pause was a frozen wait (now the match start's countdown, with the
  kits); a thaw now counts as a kill.
- Test 3: "respawn in 8 seconds" counted in the pause and after the final
  win (the last freeze's death respawn; now every time is cancelled there),
  and the end came 3 s after the final win (now 2 s).

Open:

- A frozen player who leaves and comes back, and one who enters in the
  pause: not played yet with the entry fix ("No place out of the world").
- A late joiner's lives count the match's round starts, those before he
  came in too (the result has the match's count, not his), as in Artifact
  Hunt: his ceiling counts lives he did not have (what he is charged is
  still what his stacks lost, under it).
- Late joiners spawn into the round in play; waiting for the next round (with
  something to watch meanwhile, e.g. the frozen orbit camera around a living
  teammate) is a possible later change.
- Auto-thaw after 2 minutes, a stop gap (the owner, 2026-11-22). Planned
  instead: when a team is down to its last unfrozen player, a 1-minute
  timer starts (not in 1 v 1 or 2 v 1); its outcome still to design.

### 2026-11-21, after the merge of main and the alignment

`tools/check` on the branch (51 checks selected): pass for freeze-native
(every case above, the pause's hold and the post-match hold included; the
ordinary match's input through patch 78's chained handler), the client
crate's tests (465) and Clippy, cargo:srp-validate, survarium, session,
match-server, lobby-server, web-server and the rest, the quick content
build (`freeze-tag-1.json`: 2 files edited, the config new, the dictionary
changed), localization, world-drift and every other exe check but those
below. The failures are main's, the same on other branches the same day:

- world-check: 48,148 of 49,508 worlds pass, 1,360 refused (object fields,
  FFA respawn-point counts), the counts of main's; no world of mode 12 yet;
- ffa-native: the world-roundtrip cases pass (fresh A, fresh B, resync), its
  coverage audit names four unlisted codec sites (`252B48`, `2AC8A6`,
  `310224`, `33A51B`; #2337);
- cargo:srp: the Player vs Bots end-to-end test's status rows (17, not 25;
  #2335);
- vostok-layout (day 2's existing object; #2338), vostok-anomalies (the
  Vostok modes edit; #2339), stalker-maps (cargo outside the dev shell;
  #2336), v021d-weather (breakable-lamps-1 over a 0.21 evening project),
  localization-harness (Calibri has no ∞).

The round starts (69 bytes, the pay of lives) ran the same set again: the
same passes and the same failures of main's, and the new tests pass
(srp-validate's count and its domain, session's lives, survarium's decode,
freeze-native's restart, rollback and replay counts).

### 2026-11-17

The shared rule/codec tests, installed native hooks, generated engine ABI,
client tests and Clippy passed. The manifest content build and stock HUD/lobby
Ruffle regression checks passed. Game/server validator comparison agreed on
all 5,775 sampled worlds and their mutated copies (20 batches).
The camera checks execute the native warmup/player switches, preserve
orbit across frames, clear input at transitions, restore on thaw/match end,
re-enter after rollback, and reuse the existing seven native zoom/pitch-limit
cases. They also cover a late join already in orbit and third-person restore.

The broader checks were not clean then: the badge-price assertion,
player-comment migration (`shop_owned` missing), PvB map-count assertion
and infinity-glyph localization check failed. The full corpus check
accepted 45,697 of 46,578 worlds and refused 881 for object fields or FFA
spawn-point counts; it left 2,386 unchecked. These rules were not relaxed.
The Vostok content-preservation assertion failed for an existing day-2
object. The world-roundtrip cases themselves passed, but its coverage audit
reported three unlisted weapon codec sites (`252B48`, `310224`, `33A51B`).

An uninstalled local test build of that date is in this worktree's
`private/freeze-tag/` (base a08445576, content `patch_64`): stale since the
merge of main on 2026-11-21. A local match needs a new build.
