# Freeze Tag

The initial mode uses two teams and a best-of-five match: first to three
round wins. No artifact rescue in this version (owner, 2026-11-17).

- A lethal hit freezes the player where they were hit. Movement, weapons
  and interactions stop; the ordinary death teardown and ragdoll do not run.
- While frozen, the camera uses the existing pre-match orbit and mouse-wheel
  zoom around that body. Thaw restores the previous first/third-person mode.
- A living teammate within 3 metres, with a clear static-world ray between
  their bodies, thaws them after 3 continuous seconds. Losing that proximity
  resets progress. Extra teammates do not accelerate it.
- Thaw restores health in place and clears input. It preserves equipment
  and ammunition. Frozen teammates cannot thaw each other.
- Freezing the entire opposing team awards one round. Simultaneous wipes
  award neither team a point and replay the round.
- The existing match time limit still applies. If it expires before either
  team reaches three wins, the unfinished series is a draw.
- Between rounds all players wait 5 seconds, then the existing team spawn
  path removes/reinserts every current player and resets their kit normally.
  The rule waits for delayed native spawns before opening the next round.
- A disconnect cannot clear a frozen place. A new place starts on its first
  spawn. Absent players are excluded when a new round starts.
- The HUD reuses the stock TDM score bars (maximum 3), collection progress
  widget and blocked-hit UI sound. Frozen players have a marker. A rescuer
  sees progress only for a reachable frozen teammate. No new ice assets.

The first authored map is Rudnya day, using its existing TDM project and
team spawns. `freeze-tag-1.json` makes its config and fills unused map-dictionary
slot 114. Mode 12 is separate from FFA variations 9–11. Native rules are
93, 91, 94: time, respawn, and kill counters; no TDM ticket rule. The options
carry `SRPX`, tag 6 (4 is the wide map id, 5 the faces), length 1, version 1. Both client and server understand it.

## State and native lifecycle

`srp-validate::world::freeze` owns the deterministic transition and codec.
The rule saves 67 bytes after the native respawn state: version, phase,
two scores, a 16-bit intermission countdown, round result, then twenty
three-byte player records (waiting/fighting/frozen with team, thaw time).
The parser checks the closed domains, roster extent, phase/score agreement
and round result. Rollback and late join restore progress and round score.

The three native hooks are declared in `sites_026g.tsv`:

- `2DAA60`, `base_player::tick`: a frozen player's clock advances, but the
  deferred death callback and tick are skipped through the native `ret 4`
  at `2DAB69`. Lethal hits already set `pending_death` (+33Dh), which also
  prevents further damage in `hit_impl`.
- `2D86EE`, after `set_input`'s saved-register prologue: frozen input takes
  its common epilogue at `2D8A06`.
- `316826`, after respawn tick's prologue and the existing warmup kit-change
  trampoline: advance the rule, cancel a frozen body's death respawn, or schedule
  all active places at this tick for the next round. Every other place keeps its
  native schedule, so a late arrival's first spawn (its entry time) still runs. The original `31697F`
  path owns removal, spawn point selection and spawn; no synthetic deaths.

The typed `vostok::freeze` boundary uses generated fields and engine calls.
Thaw calls the same `damage_model::reset` (`2E5290`, EDI receiver, stack time,
`ret 4`) as native spawn at `2DAD32..39`, then clears pending death/actions
and requests animation selection. The line-of-sight test uses the existing
physics ray (group 24h, mask 2), chest to chest.

`tools/check freeze-native` exercises installed hooks and round scheduling
under Unicorn. Shared-rule tests cover continuous thaw, wall/enemy refusal,
disconnects, draws, 3–0 and 3–2 finishes, codec rejection and rollback during
intermission. Server tests cover map rules, mode registration and the native
rule envelope. `tools/check` also selects the world corpus/drift and existing
world-roundtrip checks.

The presentation hook after the simulation step borrows the native warmup
camera with `game_world::switch_to_warmup_camera` (`1F8190`). The existing
input handler, `process_warmup_mode`, camera view, collision and zoom limits
are reused. It restores `switch_to_player_camera` (`1F8200`) on thaw or match
end and clears pending input on both transitions. It owns no pregame,
spectator or replay camera, and retains only an identity and previous mode;
camera state does not enter the world format.

An actual local match is still required before merging: body presentation,
sound, camera, native spawn lifecycle and new-version world recordings must
be checked in the running game. The owner deferred the proposed three-client
playtest on 2026-11-17; no game windows were opened.

## Validation of the first implementation (2026-11-17)

The shared rule/codec tests, installed native hooks, generated engine ABI,
client tests and Clippy pass. The manifest content build and stock HUD/lobby
Ruffle regression checks pass. Game/server validator comparison agrees on
all 5,775 sampled worlds and their mutated copies (20 batches).
The final camera checks execute the native warmup/player switches, preserve
orbit across frames, clear input at transitions, restore on thaw/match end,
re-enter after rollback, and reuse the existing seven native zoom/pitch-limit
cases. They also cover a late join already in orbit and third-person restore.

The broader checks are not clean: the existing badge-price assertion,
player-comment migration (`shop_owned` missing), PvB map-count assertion
and infinity-glyph localization check fail. The full existing corpus check
accepts 45,697 of 46,578 worlds and refuses 881 for object fields or FFA
spawn-point counts; it leaves 2,386 unchecked. These rules were not relaxed.
Clippy was also run separately for the crates whose tests stopped before
their lint stage, and passes.

The Vostok content-preservation assertion also fails for an existing day-2
object. The world-roundtrip cases themselves pass (fresh A, fresh B and
resync, eight worlds each), but its coverage audit reports three existing
unlisted weapon codec sites (`252B48`, `310224`, `33A51B`). The FFA and
late-join native cases preceding that audit pass.

An uninstalled local test build is in this worktree's `private/freeze-tag/`:
the executable, manifest-built content patch and matching server data.
Its `README.txt` records the hashes and playtest cases. It is a quick build,
not a release. The repository's release revision and patch list are unchanged.
