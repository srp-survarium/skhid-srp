# A world no game could load: refused by the server and by the game (0.26g)

The owner, 2026-10-22: one client must not be able to send the others data that crashes them; and
"most of this should go to clients too". This is the smallest part of that (PR #282's packet check,
the reasoning of #285's trust map and of #478): the checks of a world's frame against the game's
own fixed limits, which no world a game made can fail. Written 2026-10-31.

## What a game does with a world

- The 0xE8 handler (`network_client::process_dynamic_match_info` 0x27BFF0) takes the generation
  and the flag, unpacks a packed world into 0x8000 bytes of its stack (0x27C059..0x27C0A8), and
  hands the world, its time (u32) then the body, to `game_world_core::deserialize` 0x2D0B70: at
  0x27C0D9 (unpacked) and 0x27C111 (raw). PROVEN.
- Nothing checks the body before `game_state_history_item::deserialize` 0x324D80 reads it. Its
  reader has no end test; it puts mask bit k's block into slot k of the history item with no
  bound (20 slots, constructor 0x324510); it copies each section and each player state into a
  fixed buffer of the history item. A longer one goes to `mutable_buffer::reallocate` 0x7AB70,
  whose allocator is null: the assert "mutable buffer overflow" (0x9E3398), then a read through
  null, in every game that loads it. PROVEN (#285, relay_trust.md section 3).
- Every world the server sends was uploaded by one game (0xE0 chunks).

## The checks (crate `world-frame`, `Frame::parse`)

- **The frame walks from the front to exactly its end:** the u32 mask, then per set bit a u16
  length, the state, a byte, and when it is not 0 an animation bit stream (`animation_len`,
  `decode_data` 0x3A9900), then six u16-prefixed sections (bullets, statistics, game objects,
  physics, rules, effects). No byte may be left over, and none may be missing. The server's
  older from-the-end reading (`Sections::candidates`) does not count.
- **Every mask bit is below the places:** the match's roster on the server (`snapshot_players`,
  20 in the growing-roster dialect), all 20 slots in the game (it does not know a late join's
  roster).
- **Every section fits its buffer:** bullets 0x2400, statistics 0xC00, objects 0x2000, physics
  0x4000, rules 0x800, effects 0xC00 bytes after the length; **every player state is at most
  0x1000** (`player_history_item` 0x3247D0).
- Nothing else: not the contents of a section or a state (PR #289's checks), not the match
  messages 0xE3..0xF6, not the game objects.
- A refusal is one enum (`Refusal`), written `<part> at <offset>: <reason>`.

## The server (`match-server`, `game.rs::uploaded`)

- An upload that does not parse goes to nobody, before anything else is done with it: never the
  initial world, a candidate, `recent`, or a world to look at.
- It is saved (`SRP_SNAPSHOT_DIR`, kind `unloadable`), counted (`uploads_refused`, in the
  player's summary line) and logged up to 8 times a player: the player, the time, the size and
  the refusal.
- The game stays asked: a world of its that loads may still come in time.
- Until 2026-10-31 a world that did not split was used as it was, and nothing else was checked.

## The game (exe patch 87, `rust/patches/src/world_check.rs`)

- Two lohooks, `world_load.unpacked` (0x27C0D0, the world in the frame's mutable buffer at
  EBP-24h) and `world_load.raw` (0x27C0FF, the rest of the packet's reader in ESI), before each
  load, run the same `Frame::parse` (`parse_state`: the time, then the body).
- A world that parses loads as before.
- A refused one is never loaded. The packet's reader is read to its end (as the stale-world path,
  0x27C02E), `close_current_match(2)` (the leave button's: 0xE0 to the match server, `31 .. 01`
  to the lobby, the world ended) is called, one line goes to `srp_patches.log`, and the hook's
  exit is the handler's epilogue (0x27C1E0), past the post-load steps. The unpacked world's
  buffer has a null allocator, so the skipped destructor frees nothing.
- Why leave and not drop: a first world dropped leaves the game frozen, a resync dropped leaves
  it out of step (#478, its client_world_check.md section 5).
- LIKELY: `close_current_match` may be called from inside the 0xE8 handler; the 0xEA handler makes
  the same call from inside the dispatch (0x27C720..0x27C76F). Not run in a game.

## No false refusals

- A game serializes into the same fixed buffers, so its own worlds never exceed them.
- The kept worlds of real games under `private/` (local-clients 9516, local-clients-40b 291,
  fov-rev11 314, ragdoll-lab 374, rudnya-local 215, vegetation-local-20261028 20; 10 730 in all):
  0 refused at 20 places. The largest: bullets 0xC66, statistics 0x656, objects 0x3ED, physics 0,
  rules 0x776, effects 0xFC, a state 0x4E5; the highest slot 19.
- `private/capacity`'s worlds are srp's own test client's, not a game's: 1012 of 1036 are refused
  (bullets 0x4000 and 0x7D00, its old ballast). Its ballast now goes into the bullets up to
  0x23FF and the rest into the physics (`Sim::ballast`).

## Tests

- `cargo test -p world-frame`: each check and its edge (at a cap, one over, slot 19 and 20, cut
  at every byte, a byte too many, the time first).
- `cargo test -p match-server a_world_no_game_could_load_goes_to_nobody`: garbage; a place past a
  roster of two and bit 31; a state over its cap; each section one byte over; a mask naming a block
  that is not there; a world at the bullets' cap passes; the next game's world starts the match.
- `check_world_check_026g.py`: both loads under Unicorn beside the original exe; `--drift` runs
  every kept world and 16 mutated copies of each through the arena's i686 build and compares
  with the host build's verdicts (`cargo run -p world-frame --example verdicts`).
