# Free for all (mode 8)

The owner (2026-11-01): "I only want plain FFA": everyone for themselves, normal respawns, the most
kills wins, no elimination phases (not 0.31's "The Last Stand"). His rules of that day and of
2026-11-11, when it was ported onto main and finished:

- 10 points a kill (the normal kill score), nothing lost on death.
- The first to 30 kills ends the match, or the timer does (15:00).
- Spawn protection 2 s ("3 seconds sounds too long, it should be 1.5 or 2 MAX").
- Safe respawns across the map. On 2026-11-12 the owner revised the earlier
  farthest-point preference because it kept sending players to corners: all
  safe points now enter the choice, including the middle; distance ranks only
  the fallback when no point is safe.
- The results screen and the Tab list keep the game's two columns, filled as one ranking.

## Rules

- **Mode 8, "Free for all"** (`st_mode_08_name`, `st_mode_08_description`; 6 is Blackout, 7 is
  kept for Scarcity, PR #1083). `maps::RuleMode::FreeForAll`, `maps::GameMode::FreeForAll`; its
  lobby number is 8 too.
- **Configs** `gameplay/game_modes/ffa/level_0X_<weather>_8.options` (content edit `ffa-1`, in a
  folder of their own as Blackout's): copies of Team Deathmatch's (mode 4) as every list before
  leaves them, with:
  - `mode.mode` 8 and rules 93, 91, 94 (time, respawn, kd): no rule of its own. TDM's rule 90 has
    arrays of two teams.
  - TDM's own project, timings (15:00, 20 s countdown) and respawn time.
  - `mode/parameters/kills_to_win` 30 and `mode/match_making_params/max_players` 10. The server
    reads both from the config (`maps::MapMode`); a retune is an edit-list change.
- **Sides:** each player is his own side, team 4 + his roster place (`srp_validate::own_side`,
  `maps::free_for_all_team`).
  - The match server gives it, to late joiners and its own bots too, and squad 0
    (`registry::own_side`). The lobby's two sides are only the seating.
  - The game reads 2 as "neutral" and 3 as "none", so those are never given.
  - No squads in the match: squad mates are opponents. The lobby squad itself stays.
  - Every other player is an enemy wherever the game compares teams: names, markers, minimap,
    kill feed, kills, assists, spotting.
  - The options a client is sent say so twice, and each check wants both or neither
    (`static_info::check_team`, `srp_validate::messages::options` in the client too): every
    place's team its own side, and srp's entry tag 3 with the kills that win (u16).
- **Kills decide everything:**
  - The native kd rule counts the first kill reaching `kills_to_win`, including its victim's
    death, then ignores further death events. It also ignores events at or after the exact
    time limit. The counters themselves serialize the terminal standing: rollback, late join
    and replay restore it with the world, without a process-local winner flag.
  - The server ends the match when a trusted snapshot's kd rule shows a player with
    `kills_to_win` kills (`rules::ffa_finished`), after the usual end delay, or at the time limit.
    The first terminal trusted snapshot owns its results throughout that delay.
  - The winners are everyone with the most kills, at least one (`rewards::Winners::MostKills`).
    Ties share the win.
  - The results screen and the Tab list rank by kills, then score, then fewer deaths, then place.
  - The score itself is the game's, as in every mode: 10 a kill, 5 an assist. Team Deathmatch's
    three times a kill is mode 4's alone.
- **Pay:** as TDM: the score's pay and win bonus. Each finished FFA winner
  earns the normal Elo victory gain against the other participants' average
  rating (+2 with equal ratings). Tied leaders each earn a win; everyone else
  gets zero rating change, including leavers. No team-balance rating change.
  Solo, untrusted, private and bot matches retain their no-rating rules.
- **Players:** 10 at the most, the server's bots among them (`maps::player_cap`). No minimum: two
  play a duel, and a private room's owner may start alone.

## Spawns (the free-for-all exe patch)

A match's first spawn and every respawn go through the same respawn tick, so both are these.

- **Points:** only the map's team-2 points: 0.31a2's mode-6 points, added to each TDM project
  (`scripts/ffa-mode`). TDM's own points are never a free-for-all's, and TDM's tick serves teams 0
  and 1 only, so TDM spawns as before.
- **Choice,** for each player due at a tick, in the tick's order:
  - every point that passes the game's clear test (no other player within its
    enemy radius: 20 m on every map), and has no point already assigned
    at this tick inside that radius, enters the pool in map order;
  - choose from the whole safe pool, without favouring greater distance;
    safe central points are as eligible as safe edge points;
  - a wrapping u32 integer mix of match milliseconds and player place picks
    the pool index. Mixing before modulo avoids locking regular respawns
    to a subset of points. It needs no persistent random state;
  - with no safe point, choose among the five best-separated points from the
    nearest living player or spawn already assigned at this tick, using the
    same mix. Equal ranks keep map order; smaller maps use all their points.
- **Deterministic:** the same synchronized inputs give clients, late joiners
  and replays the same result. TDM retains the game's selection.
- **Protection:** 2 s on every team-2 point (`PROTECTION` in `scripts/ffa-mode/make_edits.py`;
  the exe reads the key as whole seconds). TDM's own points keep theirs (`maps.tsv`
  `spawn_protection_s`, teams 0 and 1 only).
- **Resupply:** every ammo zone serves a free-for-all player (TDM's maps have one zone per team).
- **Checked against 0.26g's collision** (`scripts/ffa-mode/check-points`,
  `scripts/ffa-mode/checked.tsv`): each 0.31 point is kept, nudged to the ground (under 0.43 m) or
  dropped (ground over 0.5 m off, body blocked, or no reachable floor).

## Maps

30 configs: levels 1-8 in their three weathers, London (day), and all five ported day maps. Only maps whose project has
team-2 points: a mode-8 config without them stops the server (`maps::MapMode::from_config`).

| Level | Weathers | Solo points (kept / donor, or authored) |
|---|---|---|
| level_01 Rudnya | day, evening, rain | 28 / 29 |
| level_02 School | day, evening, rain | 25 / 25, and the project's own one |
| level_03 Vostok Radar Station | day, evening, rain | 24 / 24 |
| level_04 Chemical Plant | day, evening, rain | 30 / 33 |
| level_05 Tarakanovsky Fort | day, evening, rain | 24 / 24 |
| level_06 Mamayev Kurgan | day, evening, rain | 25 / 26 |
| level_07 Vector Laboratory | day, evening, rain | 29 / 30 |
| level_08 Cologne | day, evening, prerain | 23 / 24 |
| level_09 London | day | 22 / 27 + 6 authored central points = 28 |
| v021d_level_03 Vostok Radar Station | day | 22 authored |
| v021d_level_06 Mamayev Kurgan | day | 22 authored |
| v011e_level_04 Chemical Plant | day | 22 authored |
| v011e_level_03 Vostok Radar Station | day | 22 authored |
| v069d_level_01 Rudnya | day | 22 authored |

- `maps_dict` ids 46-60 and 154-163 retain their meanings; the five ports use
  164, 165, 167, 168 and 169 in table order. IDs 166/175 already belong to ports.
- `ffa-ported-1.json` follows `ffa-1.json`, cloning each port's final day TDM config
  and adding only team-2 points. These ports have no other TDM weather variant.
- The TDM projects' snapshots carry the new points' bytes: a `next` row of
  `simulation_changes.tsv`.

London also has six additional central spawns requested on 2026-11-12, recorded
in `scripts/ffa-mode/london-center.tsv`. The `central` authoring binary checks
actual London geometry within X [-35, 40], Z [-30, 30], with the same body,
hazard and facing checks as the ports, and at least 12 m horizontal spacing
from all 22 existing solo spawns and the other additions. `make_london_edits.py`
emits `ffa-london-center-1.json`, applied after `ffa-1.json`; existing points
and IDs remain unchanged. London now has 28 solo spawns.

Validation (2026-11-12): geometry generation checked 918 safe candidates on
278,660 navigation floors, followed by visual review of the floor/point plot.
The selected additions are at least 18.79 m from any existing or other new
solo point. `tools/check ffa-maps` passed (3 Rust tests, 6 Python tests, clean
Clippy; key `54c17651e96a`). Applying the source edit preserved all 42 existing
London records; original-executable relocation and zone lookup passed for all
48 final records, and native respawn loading passed for all 28 solo points.
The combined manifest quick build completed successfully; its packed London
project is byte-identical to the native-tested output. Local test patch SHA256:
`493042b814ae008661d83c84ee83733ceb8c88010cfb39491ce8069be0c07f53`;
server data SHA256:
`faa4629559db6c1a66de953b83a559ca61a50a15511026a409a125be91d389e3`.
This is geometry and native-loader validation, not an in-game playtest.
Installed this combined package into the isolated `local-clients-ffa` setup
and restarted FfaOne/FfaTwo/FfaThree. The server reports 30 playable maps and
all three processes are running; no central-spawn gameplay claim is made.
Nothing was released and the release revision is unchanged.

The ported layouts are explicit source in `scripts/ffa-mode/ported.tsv`, made by the
`ported` binary of `scripts/ffa-mode/check-points` and emitted by
`make_ported_edits.py`. It selects 22 points per project from reachable standing
navigation floors, using the same walker collision/body checks as the imported
0.31 points. Each point stays at least 2 m from harmful zone shapes, at least
12 m from another solo point, and faces a standing, walkable 6 m path using the
engine's yaw convention. Candidates remain within 12 m horizontally and 5 m
vertically of the established spawn footprint: the corresponding 0.26 FFA
layout, including Rudnya, so the newer map gets points across its play area
rather than only around its two team bases.
The target map's geometry, not the footprint's donor geometry, decides safety.
Mamayev fits 22 points at that spacing; the checks are not relaxed to add more.

The generated solo records have empty ally/enemy collision zones, including
older ports whose team spawns contain base zones. Their original team spawns
are unchanged. Missing radius fields in v0.11 are added to solo records only.
All map/config/point data reaches the server through the generated game data;
there is no runtime map table in code. These layouts have geometry validation
and spatial review; they still need multiplayer playtesting for balance.

## Where it is played

- **Rooms:** one more mode of the pool, as Research is: in every room's default pool, a mode tick
  of the create window, cards of the vote.
  - A room's free-for-all seats 10 of its members who can play: who had no place in its last
    match first (`Rooms::left_out`), then in the order they came. The others wait for the next
    match. A full match takes no late joiner (`registry::join`: Full), and a member waiting beside
    it is told so.
  - A squad of the room plays it each on his own. A squad alone in an open room waits for a third
    as before any match: the vote does not begin for it, whatever the cards.
  - A room with bots is filled with them up to the 10.
  - The pool of a create names 256 entries at the most (16 pieces of 16 bits, `lobby::rooms`
    `POOL_BITS`; 160 before): the live data has 167 playable maps with the free-for-alls. The
    lobby movie and the server change together.
  - A room's match with a minimum waits for a player of each side before it starts; a
    free-for-all's for its players alone (`game/start.rs`).
- **Player vs Bots:** offers the day free-for-all of each level it offers, beside its other maps:
  the player and min(2N, 10) - 1 bots, each on his own. Its size menu still says "N vs N".
- **Not in plain Play** (a clan match, the old queue): those are matches of two teams.
- **Bots:** the server's bots play it as a Team Deathmatch with no teammates (`bots::view::Side`:
  one of the two teams, or a place alone; the per-side state is one a place).
  - Every other living place, bot or human, is a target. What a bot knows (callouts, danger
    spots) is its own.
  - No base: it roams (a walkable floor 20 to 60 m off, a look, again), flanks and heals. The team
    plays (pair up, hunt, retreat to the base's ammo, every battery play) are not offered to it.
  - Its skill balances by its own duels with humans, which are few in one match.
  - The bots' map files (`release/bot-data.tsv`) were dumped before `ffa-1`. Dumped again from
    projects with the free-for-all points, each team-2 point would become a spawn area Team
    Deathmatch's bots keep out of (`bots/tactics.rs`): that reading must leave team 2 out first.
- A live server with `SRP_MATCH_MODES` set must name 8 for it to be played.
- **The site's live table** shows a free-for-all's players in all, not "N v N".

## The player's screens

- **Top bar (hud):** the match timer only, without opposing bars, team counters or alive-player
  counters (owner, 2026-11-12). It uses the existing timer's frame; `FfaProgress` keeps its team
  controls hidden, including after native `set_ffa_progress` / `set_game_progress` callbacks.
- **Messages (the exe patch):** at the start "Free for all. First to 30 kills wins"
  (`st_start_match_welcome_message_8`, the number the match's); at the end (the target reached,
  or no time left) victory to who has the most kills, defeat to the others
  (`st_finish_match_mode_08_winner_message`, `..._loser_message`).
- **Loading screen:** the mode's name and `st_mode_08_description`.
- **Tab list (hud):** one list ordered by kills descending, then score descending, fewer deaths,
  and roster place. Fill the left column's ten rows before continuing on the right. Score updates
  reorder the whole list, moving players across the column boundary and preserving local-player
  selection. Names have no ranking-number prefixes. Team names/totals stay hidden. The optional
  "Hide player scores" setting retains its alphabetical names-only view.
- **Results screen (lobby):** the same kill-sorted list as Tab: ten rows on the
  left, then overflow right, plain names without rank prefixes, no team names
  or totals. Updates rebuild the whole ordering and selection can cross columns.
  The native results ABI still accepts only teams 0/1, so the server transports
  ranking halves using those two values (`rewards::ffa_columns`) and constructs
  the reader's win/defeat (`rewards::ffa_screen`); Flash ignores those transport
  groups when laying out the FFA list.

## Why two sides are the limit (PROVEN, 2026-11-01, 0.26g exe)

- **Respawn tick** 0x316820 (vtable 0xA16084 +50h):
  - It batches the players who are due into **two** stack vectors (`alloca(18h)`), the vector index
    being the player's team (0x3168C8). Team 2 or more writes past them, onto the stack.
  - It then calls select (+54h, 0x3171D0) for teams 0 and 1 only. Select indexes the point groups
    (+188h/+194h, three groups: team 0, team 1, team 2) by team.
- **TDM rule:** `m_team_tickets[2]` at +110h (0x31612C, 0x31617D) and the alive counts `[2]` at
  +11Ch (0x316234) are indexed by the victim's team. FFA's configs leave the rule out.
- **Statistics:** `m_team_victory_items[2]` +31D8h is indexed by team (0x2F5590). This is batteries
  only.
- **Resupply:** a zone serves its own team or team 2 (0x2F808F, 0x2F80CF).
- **Special team values:** team 2 is "neutral" for names (0x1F2B4B), resupply (0x2F8097) and the
  minimap (0x239B3C). The HUD turns local team 3 into 0 (0x234975).
- **Equality only (works with any value):** kd kills (0x32384F), the statistics' enemy events
  (0x2F548A, 0x2F590B, 0x2F596B), spotting (0x2DC0D8), HUD allies (0x234FF5), kill feed (0x237EBD)
  and quests (0x2A342B). These 86 reads of `player_profile+45ECh` were all checked.
- **Results screen:** a row's team indexes a two-byte count (0x29B682, results.md §6): 0 or 1 only.

## Minor picks (mine)

- Kills, not score, decide the end, the winner and the ranking, so the top row is the winner.
- No player minimum (0.31's mode had 3).
- A room with more members than 10 seats the first 10 (the room's order); no FFA-only card filter.
- Player vs Bots offers the day free-for-alls only.


## Review fixes, 2026-11-12

- Native late-join profiles accept 0/1 or the engine's bounded solo identities 4..23;
  admission requires a solo identity to equal 4 + its place and squad 0. Neutral/none
  and out-of-range values remain invalid. The decoder and complete native
  prepare/query/install/ACK lifecycle have FFA regression fixtures.
- The HUD reads the match wrapper's live options roster each frame (the exact object
  the installer updates, even with a stale separate client options object), including
  after native late-join installation and when a replay watches the new player. It reads the actual time
  rule against the completed simulation time, not the truncated displayed seconds.
- The kill-event hook freezes native KD records at the goal/deadline; their native
  deserialize restores play before the terminal event and the winner after it.
  The match server separately retains its first terminal trusted result during close.
- **Implemented and tested under Nix:** `cargo test` in `client-patches/rust`:
  295 passed, 2 ignored; `cargo clippy --all-targets -- -D warnings`: clean.
  `tools/check cargo:match-server`: 632 unit tests passed (19 ignored), its
  integration test passed, Clippy clean. The new server test deliberately
  submits a later trusted world with another player leading during a 30-second
  closing delay; the original terminal counters, result tick and scores remain.
- **Native emulation passed:** `check_free_for_all_026g.py` and the full
  `check_late_join_026g.py` against the original 0.26g executable. This includes
  the native profile decoder, prepare/query/install/ACK/cleanup, zero displayed
  seconds with 999/1 ms still playable, the live roster with a deliberately stale
  separate client options object, native death events and KD deserialize before
  and after the winning event. `tools/check hud-options` also passed on the final
  tree. Generated engine/hook checks and `late_join_026g.py --check-bin` passed.
- **Lifecycle sanitizers passed:** `test_late_join_runtime_026g.c` with
  AddressSanitizer and UBSan. `ASAN_OPTIONS=detect_leaks=0` is needed in this
  ptrace environment; LeakSanitizer itself cannot run here. The fixture's own
  allocation/reference balance assertions passed, including rejected solo
  identities and squads.
- **Broader check remains red:** `tools/check --base MERGE_HEAD` passed the
  affected Rust crates and reused the passing content build. The same review
  failures remain: localization harness missing Calibri's `∞` glyph;
  world-check (44,137 checked: 22,397 passed, 21,740 refused, 72 not checked);
  world-drift aborting its initial native fixture at `0xA4CCFD` with an unmapped
  read, before comparing sampled worlds. Existing recordings include incompatible
  respawn layouts; no parser constraint was weakened to accept them.
- **Not played or released:** no real-client FFA match was run for these fixes;
  native emulation is not a playtest. The pending simulation-change row records
  the new terminal-counter behavior; the release revision remains untouched.
  Final arena SHA-256: `991a3b545d21e9063bd44084ff1b62a02d7c6414728bff41b364c527c4cc06af`.
  Late-join payload SHA-256: `c53353e7ff3d88ad06edc04b4d943880b0604a426d1d4ac100347ccf7f440c00`.


## Real-client loading crash, 2026-11-12

The three-client test reached the lobby. FfaOne and FfaThree then crashed loading
Chemical Plant (`level_04_8`, map 55), at local times 16:40:04 and 16:40:16.
Both dumps fault at `binary_config_value::operator[]` 0xF6BC2, called by
`respawn_point_core::load` at 0x2F46AF for `ally_zones/collision_geometries`.
This is a content pointer-corruption bug, independently of the earlier fullscreen
Vulkan swapchain failures (the local clients were restarted at 1280x720 windowed).

`content_edit.copied_table` copied a spawn's outer records but shared unchanged
nested table records. Chemical Plant's original point and its 30 FFA copies thus
shared both zone records (file offsets 0x31A88 and 0x31AA0). Native relocation
0xF6D00 visits and changes each reachable record in place without deduplication.
The `collision_geometries` name offset 541625 was relocated 31 times:

- FfaOne: config base 0x8405AF48, `(541625 + 31 * base) mod 2^32 = 0xFCB87D71`.
- FfaThree: config base 0x84036E40, the same calculation gives 0xFC729D79.

Those are the exact invalid read addresses in the two dumps. The copier now
recursively gives every named/indexed table its own records, including unchanged
subtrees. Immutable string/vector payloads may still be shared. No client-code
workaround or gameplay-rule change is needed for this crash.

Validation:

- `tools/check content-edit`: all 67 tests passed (key `a65c3317395b`). The new
  spawn-zone regression fails with the original copier, and checks unique record
  ownership plus native-style in-place relocation of nested named/indexed tables.
- Unicorn running the original executable's relocation function reproduced the
  two corrupted records in the crashing project's bytes.
- `release/build-content 56` completed as a **local quick build**, from the
  manifest. Patch SHA256: `0439578bd64d38bd24034894fea713af805a0209166d23e02f1fa9ed5a7cdbb4`;
  server data SHA256: `08775f2c79d712500af0b2295b9d6da825609d6810a9ef1b72e8f32ee67800b7`.
  No release revision was changed and nothing was released.
- Extracted all 25 FFA projects from that built patch. Native 0xF6D00 relocation
  and 0xF6B10 zone-key lookup passed for all 1,527 spawn points. Full native
  `respawn_point_core::load` 0x2F4580 returned for all 646 FFA additions, preserving
  their point IDs, solo-spawn group, priority, positions and 2,000 ms protection.
- At 16:58 local time, the local server and FfaOne/FfaTwo/FfaThree were relaunched
  with those exact rebuilt content/data files. All three processes and game windows
  remained running at 1280x720, with successful swapchain creation. At 16:59:22,
  FfaOne and FfaTwo both loaded School FFA (map 49, match 0x205B42E4), entered and
  synchronized successfully; the server accepted its initial world (2,078 bytes,
  50 ms, CRC 8F0E993A) and started the countdown. This verifies actual match loading
  after the fix; a completed real-client match remains to be verified.


## HUD feedback, 2026-11-12

The owner's screenshot `~/screenshots/2026-11-12-17-01-39.png` showed three players split
2/1 across the columns with duplicate `1.` prefixes. At the owner's request, the Tab list
now fills the left column before overflowing right, shows plain names, and re-sorts by
kills across both columns. FFA's top HUD retains only the match timer.

The changed movie was built and `scripts/ffa-lists/check.sh` passed both Ruffle harnesses
on CPU rendering. Cases cover 1/3/4/10/11/20 players, equal kills with score/death/place
ordering, updates moving players in both directions across the column boundary, selected
player movement, offline rows, names-only mode and return to TDM. Timer-only checks cover
initialization, subsequent native score callbacks, end-of-match values and return to TDM.
Rendered three-player, eleven-player and timer screenshots were inspected. The lobby
results harness also passed; its two-column results layout is unchanged by this UI fix.

## Five ported maps, 2026-11-12

The five additional day variants add 110 authored solo points (22 per map).
The generator checked 767/208/763/818/1089 safe candidates respectively on
Vostok v0.21, Mamayev v0.21, Chemical Plant v0.11, Vostok v0.11 and Rudnya
v0.69. Final minimum spacing is 12.369 m on Mamayev and 21.6–21.8 m on the
other four. Floor plots were reviewed; Rudnya uses the corresponding base
FFA footprint so points cover the map instead of clustering at its team bases.

Validation completed:

- `tools/check ffa-maps`: two Rust selection/spacing tests, three Python
  generated-content tests, and the authoring tool's Clippy pass.
- The survarium FFA fixture test and affected crate Clippy pass. The real-content
  test passes on both built server databases: all 30 variants, the five new IDs
  with 22 solo points each, matching TDM projects, rules, 30 kills and 10 players.
- The official manifest edit stage writes 11 files with zero unexpected
  differences. All 519 original ported-map team records compare unchanged.
- Original client 0xF6D00 relocation and required zone lookup pass on all five
  generated projects (629 total respawn records, zero invalid relocated pointers).
  Full native 0x2F4580 loading passes on all 110 additions, including position,
  ID, group and the 2,000 ms protection value.

`release/build-content 56 --out /mnt/hdd/Projects/ffa-build/ported-maps/content`
completed successfully from the full pre-London manifest, including the HUD edits.
It produced 76 files, 114,010,662 bytes, with patch SHA256
`82918523f3bae25e717f04db9654ecf0f0b507e85ebc8e66bf8aee94c8fcb317` and
server-data SHA256
`c551d9f94983064a2c5fa00357dc28bd5253b4244935a049fd6cb6dbd3bfb3d6`.
This is a quick build for local tests: the pack was read back, but `--full`'s
post-patch whole-manifest comparison and item-table build were not requested.

The five projects extracted from that patch, and from the final combined
London patch `493042b814ae008661d83c84ee83733ceb8c88010cfb39491ce8069be0c07f53`,
are byte-identical to the native-tested outputs. The same compiled real-content
test also passes on the combined server data
`faa4629559db6c1a66de953b83a559ca61a50a15511026a409a125be91d389e3`.
Logs, final floor plots and the exact pre-London manifest are in
`/mnt/hdd/Projects/ffa-build/ported-maps/`.

These are content, geometry and native-loading checks. The new ported layouts
have not been multiplayer playtested or released.

## Spawn selection bias, 2026-11-12

The owner observed corner-heavy spawning on every map and requested a shared
selection fix. The previous selector ranked safe points by distance and kept
only the three farthest; adding central points could not make them competitive.
The selector now admits every point passing the existing native safety test and
same-tick separation check, in map order, then selects using a deterministic
wrapping u32 mix of match milliseconds and player place. The distance ranking
is used only when no safe point exists. Mixing before modulo also removes the
short fixed rotation caused by periodic respawns and `(time / 50 + 7 * place)`.
No safety radius, spawn protection, content placement or TDM behavior changed.

Validation under `nix develop ./srp-026g`:

- `cargo test --locked` in `client-patches/rust`: 296 passed, 2 ignored;
  `cargo clippy --locked --all-targets -- -D warnings`: clean.
- Host regressions exercise all 64 candidates on an empty world, the nearest
  safe point formerly excluded, regular 5/10/15-second respawns, simultaneous
  separation, zero/one/two safe choices, crowded fallback and clock wrap.
- `check_free_for_all_026g.py` passed against the original executable with the
  rebuilt arena. In 500 periodic respawns, the safe middle was selected 96
  times and the four corners 105/90/110/99 times. All 300 generated match
  cases agreed with the independent model; native safety, TDM routing and
  the existing FFA lifecycle tests passed.
- `link_026g.py --write` refreshed the arena lock; arena SHA256:
  `ba2f6f9a49fad7bcc4941f3718d832baf80ad9c37ca7e861e74a3aa548fd95d8`.
- Evidence: `/mnt/hdd/Projects/ffa-build/selection-fix/{rust,native}.log`.
  The existing `next` simulation row records the changed selection behavior.

Implemented and host/native-tested; not installed into the running games,
not played and not released. Per the owner's instruction, no games were stopped
or restarted for this change and the local executable/content setup is untouched.


## Final balance and results corrections, 2026-11-12

The owner requested a 15–20 m maximum spawn exclusion distance; the final
`ffa-radius-1.json` retune sets all 765 solo spawns across 30 projects to
20 m. It follows all placement edits, including the six London additions,
and covers School's three original solo points (50 m). The 693 other changed
points were 30 m; Cologne's 69 were already 20 m. Team spawn properties,
positions and protection remain untouched. The source generator is
`scripts/ffa-mode/make_radius_edits.py`; the placement TSVs retain their
original donor/authoring evidence and this final retune owns the live radius.

The end-of-match screenshot still showed rank prefixes and a half/half split
because the previous correction only changed the HUD list. The lobby list
now follows the same plain-name, left-first, kill-sorted behavior. Its Ruffle
regressions cover 1/3/4/10/11/20 players, ties, unordered incoming rows, updates
across columns, selection, offline state and unchanged TDM layout.

The owner selected winner-only rating gains with no losses. FFA's previous
team-only rating gate returned zero for everyone. A finished winner now gets
the normal Elo win calculation using their own rating against the average
of the other participants who actually entered. Other players get zero.
All tied leaders can win; there is no award without an opponent, a trusted
result or a completed winning participation. Existing private/bot exclusions
and the unchanged team-balance rating remain in force.

The crowded-map fallback now mixes among the five points with greatest nearest-player
separation (or all points on a smaller map). The rank includes points assigned
earlier in the same tick and breaks equal-distance ties by map order. It does
not always pick the farthest point, nor include every unsafe point on a large
map. The all-safe pool and its 20 m exclusion remain unchanged.

## Integration on current main, 2026-11-12

The FFA-only delta was transferred into a clean checkout of `ec63bb36e51c`.
Main's replay-controls patch already owns number 140, so FFA is patch 142;
engine bindings, hook entries and the arena lock were regenerated together.
The released revision remains 56; this feature does not assign a release.

The final crowded fallback chooses among up to five best-separated points.
Host regressions cover periodic respawns, ties, small maps and empty maps.
Against the native game, 500 crowded respawns visited those five points
96/105/90/110/99 times, with zero visits to the five excluded nearer points.
The 20 m boundary, all-safe selection, 300 generated cases, native KD
completion, late-installed roster, replay and UI lifecycle checks passed.

Validation of this integrated native/UI tree:

- Rust client: 318 tests passed, 2 ignored; Clippy with `-D warnings` clean.
- Native FFA, late-join and world roundtrip checks passed. The roundtrip
  loaded/saved every fixture in fresh A, fresh B and resync without drift.
- Ruffle FFA results and HUD harnesses passed; results include score-only
  tie updates, plain names, ten-left overflow and cross-column selection.
  Room and player-menu harnesses passed. All use a private CPU-only display.
- Arena SHA256: `4c047b8ca7d1dc649214269d838789dd6f2635d8eb3aa7e02744c3af2938a7a3`.
- Integrated lobby movie SHA256:
  `4e33d25933eea69c0ebdb3184ee25479309c779d29c54d22ee14de4231099443`.
- Broader localization harness: existing failure, Calibri lacks `∞` in
  `st_srp_inv_pocket_units_whole`. Reproduced with main's own unchanged movie
  (`5fe7f664b968abbd2dcb63f51aaa5cc460775711c433076e915f7c4a8f87d512`)
  and main's own harness. The key and embedded fonts are unchanged by FFA.

These final corrections have not been installed into, or played in, the
owner's running games. No game process was stopped or restarted and no
release or deployment was performed.

The main integration also preserves Research map 249 by advancing the FFA
dictionary append guards one entry. The base FFA and ported recipes were
regenerated from the builder's exact input folders and all operations matched.
The final radius recipe reproduced byte-for-byte from the composed 30-project
prefix (Cologne already has 20 m, so its inputs come from the base FFA stage).

The broader `hud-options` check fails at its `configs.start` registration
expectation on both this tree and clean main `ec63bb36e51c`, tested in a
separate detached checkout. FFA's own native checks, generated ABI/arena
checks, renderer/detail check and replay-file checks pass. The registered
`ffa-native` and `ffa-ui` checks both pass.

The server end-to-end suite initially passed eight tests and timed out in
`a_new_login_joins_a_running_match_through_the_lobby_and_receives_results`
waiting for newcomer input, with no lobby or match protocol faults. The exact
test passed in isolation on the unchanged code (92.49 s). This is recorded
as a timing-sensitive test result, not evidence that a gameplay fault was fixed.

The final project probe loaded 2,162 spawn records, including all 765 solo
records, through the native binary-config relocator and spawn loader without
invalid pointers. All solo exclusion radii read back as 20 m, including
School's three original integer-valued records.

Composing with main's restored Vostok electrical anomalies exposed one unsafe
v0.11 solo point (`ffa_ported_12`, point 40). Its authored position moved
3.26 m to `[-38.503, 5.99284, -22.748001]`, facing yaw `2.3561945`.
The replacement has reachable standing floor, body clearance, at least 12 m
from other solo points and 2 m from harmful zones; the complete six-metre
exit remains walkable and clear of those zones. The composed-project geometry
probe passes all 22 solo points on each of the two Vostok ports. The source
TSV and content recipe carry the same correction.

Current-main integration also includes the weapon requests and the shared
artefact-skill fix through `1823f0fd5`. The regenerated combined arena SHA256
is `be3392cafe51318a4d32f124c72c83f5a3f04988e7870484fa33f96420ecc527`.

The manifest-built local test archive completed successfully (105 files,
141,585,705 bytes; SHA256
`c392265045ac16437b6b3571831ed899141e4d6274ae9870fc38472223e04b3b`).
Its server-data SHA256 is
`05d95ce1532e62b64a319bcd65c7cef79ef8630dc55d99125afe6e8afdb75561`.
The archive was read back by the builder. All 30 extracted projects exactly
match the independently verified final project trees; all 765 solo spawns
have 20 m exclusion and two seconds of protection and pass native loading.
The actual archive also passes both Vostok geometry checks and the strict
composed preservation check, including Research map 249. The ignored
real-data test `the_released_free_for_alls_play_on_their_tdm_projects`
passes against this server data. These are local quick-build artifacts;
they do not reserve or publish a revision.

The refreshed server end-to-end run passes all nine match tests. A separate
password-reset web test failed its zero packet-order-drop assertion under
load (WebBob: 40 drops); the exact test passes unchanged in isolation
(10.24 s). Neither intermittent test result is described as a gameplay fix.

The final server/end-to-end checks pass after integrating main through
`9f7d31bde`, including the previously intermittent password-reset test.
Some passing results were not cached: the check runner captured its input
snapshot before that integration, although these server checks ran after it.
Native FFA/late-join/roundtrip and main's artefact-skill checks pass in the
combined arena; client Rust remains 318 passed, two ignored, with clean Clippy.

The generic corpus check uses the candidate's data for every recording, which
is incompatible with pre-FFA TDM projects: the native respawn cook saves all
three teams' points, including the newly added solo points. With candidate
data it checks 44,218 worlds (22,690 pass, 21,528 are refused). This is not
resolved by changing the parser. Comparing the same complete 103-recording
corpus with its recorded content proves all 44,218 worlds valid: 44,117 worlds
from revisions through 56 pass with the released revision-56 data; the 101
revision-57 local Vostok worlds pass with the composed current data. There are
no omitted worlds in that revision-matched comparison. The simulation-change
entry remains necessary so old replays are refused when the content ships.

The earlier three-player School FFA recording `205B42E4` also passes the final
server validator and data: all 140 worlds, zero refusals or omissions. Three
sampled worlds pass the native/host drift comparison in explicit mode 8 with
56 respawn points: 54 checks including mutated copies, zero differences.
Explicit context matters: the generic raw-dump inference can also interpret
these rule bytes as mode 2 with 47 points. This diagnostic uses the match's
actual FFA context instead. These recorded worlds predate the final selection
and UI corrections; they establish format compatibility, not a fresh playtest
of those corrections. No running client was changed or stopped.

The complete native/host drift run passes all 1,029 sampled worlds (928 live
samples plus all 101 local worlds), including mutated copies, with zero failed
chunks. Main's subsequent executable-reporting and calculator changes through
`e2ac60362` are also integrated. The resulting arena SHA256 is
`5ade738f2b827b71d2ea27f727777b586138308514187fa5f160ebe94b976b34`;
the content archive and server-data hashes above are unchanged.

The final registered FFA map and Ruffle UI checks pass. On the latest combined
main, match-server and srp-validate tests/Clippy pass, and client Rust passes
319 tests (two ignored) with clean Clippy. The registered Vostok layout check
passed archive preservation but its bare Cargo geometry stage lacked `cc`;
rerunning that exact stage under `nix develop` against the same archive passes
all 45 battery support/pickup checks and access to all three Research stations.
The full runner therefore reports four nonzero checks: the two reproduced
baseline harness failures, the documented historical-content corpus mismatch,
and that resolved Vostok environment failure. None was hidden by relaxing a
validator, altering the archive, or deleting a test.
