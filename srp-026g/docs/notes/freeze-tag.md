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
  their bodies, thaws them after 3 continuous seconds. Losing that proximity
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
- Freezing the entire opposing team awards one round. Simultaneous wipes
  award neither team a point and replay the round.
- The existing match time limit still applies. If it expires before either
  team reaches five wins, the unfinished series is a draw.
- Between rounds all players wait 5 seconds, held in place. Then every
  present place is due at one step of the native respawn rule, which takes
  each out and spawns it at its team's points with its kit's load, the
  living as the frozen. The rule waits for delayed native spawns before
  opening the next round.
- The pause and the restart play the match start's countdown, as Artifact
  Hunt's round does, with its code (the owner, 2026-11-21: "reswapn logic
  should be the same one as in arthunt"; "With Artifact Hunt" below): the
  warmup camera (drag to turn, wheel to zoom) and patch 31's kits, so a
  player can pick another kit; the restart spawns him in it.
- A disconnect cannot clear a frozen place. A new place starts on its first
  spawn in a round; one that enters in the pause or after the match waits
  for the next round. Absent players are excluded when a new round starts.
- The HUD reuses the stock TDM score bars (full at `WINS`, which the patch
  passes to `freeze_tag_score`), collection progress
  widget and blocked-hit UI sound. Its texts are `st_srp_hud_freeze_*`: the
  content compile gives the hud movie only `st_srp_hud_*` keys. A frozen player's name over his head
  keeps his name and is drawn ice blue (the owner, 2026-11-21: "his name
  should be of 'frozen' color, not just read frozen"); it gets its own
  colour back on thaw. A rescuer sees progress only for a reachable frozen
  teammate.
- A frozen body glows: the Spring artefact's activation glow in ice blue,
  fainter (the owner, 2026-11-21: "effect can be anomalous artifact spring
  activation but of slightly different hue and less intense"), shrinking as a
  teammate thaws him ("The frozen glow" below).

The first authored map is Rudnya day, using its existing TDM project and
team spawns. `freeze-tag-1.json` makes its config and fills unused map-dictionary
slot 114. Mode 12 is separate from FFA variations 9–11 and from Artifact
Hunt's 13. Native rules are 93, 91, 94: time, respawn, and kill counters; no
TDM ticket rule. The options carry `SRPX`, tag 6 (4 is the wide map id, 5 the
faces), length 1, version 1, among the block's other entries. Both client and
server understand it.

## State and native lifecycle

`srp-validate::world::freeze` owns the deterministic transition and codec.
The rule saves 69 bytes after the native respawn state: version, phase,
two scores, a 16-bit intermission countdown, round result, twenty
three-byte player records (waiting/fighting/frozen with team, thaw time),
then the round starts (u16): the restarts after the match's first, each of
which spawns everyone with the kit's load, a double wipe's replay too. The
parser checks the closed domains, roster extent, phase/score agreement,
round result and the restarts against the rounds ended: every one ended (a
win in the scores, or a draw) was restarted but the one between rounds or
the match's last, so the draws they leave are not negative, and are one at
least between rounds after a draw; in Restarting the round's restart is
counted. Rollback and late join restore progress, round score and the count.

The match server pays lives by it: `MatchResult::round_starts` (the field
and its use in `rewards.rs` as Artifact Hunt's branch adds them) is the
last trusted world's count (`Game::round_starts`), and `autobuy::used`
counts each round start as a life of every player, so a match of restarts
charges the kits' reloads.

The three native hooks are declared in `sites_026g.tsv`:

- `2DAA60` (`freeze.player_tick`), `base_player::tick`: a frozen player's
  clock advances, but the deferred death callback and tick are skipped
  through the native `ret 4` at `2DAB69`. Lethal hits already set
  `pending_death` (+33Dh), which also prevents further damage in `hit_impl`.
- `2D86E0` (`carry.input`, patch 78's site, order 0 before its handler at 1):
  `set_input`'s entry. A held body's input leaves through the function's own
  `ret 8` at `2D8A0C` (exit `held`): nothing of the player changes, the
  native "nothing new". Patch 78's take and use never see it.
- `316826` (`respawn.tick`, order 0; Artifact Hunt's handler is 1), after
  the respawn tick's prologue and patch 31's kit-change jump, before the tick
  reads any place's time: advance the rule, then do what
  `FreezeTag::schedule` says with the places' native times. In play
  (`CancelFrozen`) a frozen body's death respawn is cancelled and every other
  place keeps its native time, so a late arrival's first spawn (its entry
  time) runs. Between rounds and after the match (`HoldDue`) a place whose
  time is the step's is held. The step that ends the pause makes every place
  with a current player due at this step (`Actions::Restart`); then
  (`Native`) the tick's own `31697F..3169BE` takes each out and spawns it,
  and a place without a free point retries 50 ms later (`3169D8`). No
  synthetic deaths.

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

## The frozen name and the frozen glow

**The name.** The player icons movie (`flash/player_icons`, HUDMarker and
HUDIconLayer) draws a frozen place's name (the marker's `textField`) in
`HUDMarker.FROZEN_NAME_COLOR`, ice blue, while the place's bit is set in
`root.freeze_tag_markers(mask)` (sent each frame by `freeze_tag::on_hud`, 0
when another mode follows). The marker keeps the colour the movie and the game
give the name (its own, the team's by `render_name`'s `rgb` or
`set_player_color`) and puts it back exactly on thaw, the newest one if the game
set another while frozen; a name set while frozen stays blue (names are plain
text); a marker made while its place is frozen is blue from the start. The
native label is the game's, never written over. `tools/check freeze-names`
(`scripts/freeze-tag-hud`) checks it in Ruffle at 1920x1080 and 1280x720.

**The frozen glow.** `fx/artifacts/human_frozen` (content pack
`freeze-tag-glow`, `scripts/freeze-tag-glow/make_pack.py`): the stock Spring
glow `fx/artifacts/human_spring` with its three blue materials drawn with ice
copies (`fx/artifacts/<name>_ice`, constant_emissive (0.62, 0.88, 1.0) x
0.6), every emitter's rate and maximum x0.6 and its point light ice blue at
half its intensity: the owner's pick of the previews' variants ("a is good",
2026-11-21). Spring's own files are unchanged.

The thaw shrinks it (the owner: "would be nice if the glow was reducing as the
player was unfrozen", "so others could deem the freeze state by look"; of the
first build's four strengths swapped a quarter at a time: "it just disappears a
few times without becoming smaller"). One system plays; every frame its
instance's matrix is the body's with its axes scaled by 1 - 0.85 x progress /
3 s about a point 0.9 m over the feet, so 15 % at the thaw, and full again if
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
intensity: the glow gathers to the body's middle rather than dims. The
pack's `human_frozen_70`, `_40` and `_15` and their materials are no longer
played (to drop from the pack in its next version).

Each game draws it for itself, never in the simulation: an effect added to a
body's simulated effect list at the first tick after a game loaded it enters
each game's world at another tick (Artifact Hunt's carrier glow desynced so,
2026-11-21). After the simulation's move (`game_world.after_move`, the
frame hook, order 4) the game plays, moves (`update_particle_system_instance`
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

- **Held, then everyone due at once.** Freeze Tag cancelled only frozen
  bodies' times, so a player who entered during the five-second pause
  spawned into it, was held there, and was taken out and spawned again five
  seconds later; one who entered after the match spawned into a finished
  match. Holding every place that falls due between rounds and after the
  match (Artifact Hunt's `Held`) spawns each once, with the round. The
  restart itself was already the same as Artifact Hunt's `Everyone`: every
  present place made due at the step, the native tick removing and spawning
  them.
- **The input at set_input's entry.** Freeze Tag dropped a held body's input
  at 0x2D86EE, after patch 78's handler at the entry, which still took an
  artefact (from the ground or a teammate's kit) on a frozen player's Use.
  At the entry, before patch 78, with Artifact Hunt's `held` exit, nothing
  of a held input is applied; the separate `freeze.input` site is gone.

- **A round's countdown** (`vostok::round_countdown`, the shared patch
  module `round_countdown` with its hook `time_rule.camera_state` at
  0x267AE6). While `FreezeTag::round_countdown()` (the pause and the
  restart: `Between`, `Restarting`) the rule's tick makes the core's time
  state 4 (`round_countdown::set`, written only over a 3 or a 4), and 3
  again when the round opens or the match ends. The client time rule's
  camera test then sees the match start's countdown: the native switch
  0x27B1A0 gives everyone the warmup camera, patch 31's kit view shows the
  kits, and the player's camera comes back as the round opens. Patch 31
  switches no kit in a 4; a kit picked in the pause is the place's next kit,
  which the restart's spawn makes current (spawn_player 0x316DE3), so it
  needs no switch of its own and no life more than the restart's. Freeze
  Tag's own readers of the state take 3 and 4 alike as play (the rule's
  tick, the hold, the glow); its frozen camera leaves the countdown to the
  native camera. The tick sets it in list 0, before the places' ticks and
  the time rule's; a world loaded between rounds (a late join, a rollback)
  shows 3 until its first tick, which sets it again.
  Artifact Hunt's countdown comes after its restart, with its own in-place
  switch (`RespawnTick::switch_kit`, a life each); Freeze Tag's pause comes
  before it, so the restart's spawn is the switch.

Kept as Freeze Tag's own: cancelling a frozen body's death respawn while a
round is played (a thawed player must not be respawned by the death's old
time), the hold of every player's tick in the pause (Artifact Hunt holds
only the input in its countdown), and the wait for every spawn before the
next round opens (a body still frozen would otherwise freeze again at once).

## Validation

An actual local match is still required before merging: body presentation,
sound, camera, native spawn lifecycle and new-version world recordings must
be checked in the running game. The owner deferred the proposed three-client
playtest on 2026-11-17; no game windows were opened.

Open:

- No local test match yet, so no Freeze Tag worlds in the corpus.
- A frozen player who leaves and comes back. The rule keeps the place frozen
  and cancels the native entry spawn, so it should stay out of the world
  until the next round. Whether a rejoin can bring the body back another
  way (the server's late join), and so escape the freeze at the team's
  spawn, is not known until a local match tries it.
- A late joiner's lives count the match's round starts, those before he
  came in too (the result has the match's count, not his), as in Artifact
  Hunt: his ceiling counts lives he did not have (what he is charged is
  still what his stacks lost, under it).
- Late joiners spawn into the round in play; waiting for the next round (with
  something to watch meanwhile, e.g. the frozen orbit camera around a living
  teammate) is a possible later change.

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
