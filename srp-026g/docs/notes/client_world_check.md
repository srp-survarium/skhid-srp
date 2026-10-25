# Client world check: each game checks a world before it loads it

**Version scope:** client **0.26g (build 2777, no PDB)**, the default
build (with the late join). Addresses are VAs in the original exe (image
base 0x10000, sha256 `ac33d993...3bf153`). Tags: **PROVEN** (read at the
address given), **LIKELY**, **SPIKE** (built and run here, not in a game).

Status: 2026-10-25, on the arena (`.srpx`, link_026g.py's Rust crates).
Built: patch 75 (the two 0xE8 world loads and the match dispatch,
`client_verify_026g.asm` and the crate `rust/client_verify`), patch 76 (the
weapon-FSM state-id guard, in place), 77 (the anomaly state-id guard) and 78
(the artefact-slot guard, section 12). The drift test and the entries' paths
run under Unicorn (`check_client_verify_026g.py`, section 8); the cost is in
section 14, the false refusals found and fixed in section 15, the split
between the server and the client in section 16. Static and Unicorn only,
not yet in a game. Deferred: section 13.

Renumbered 2026-10-25: the first build (branch `exe/client-verify-finish`,
2026-10-23) had 54..57, which main gave to other patches since (54 the
artefacts, 58 the kit keys, ...). 54 is now 75, 55 is 76, 56 is 77, 57 is 78.

## 1. Why

- The owner, 2026-10-22: "most of this should go to clients too, to make
  them less dependant on the server going wrong".
- A game reads a world (0xE8) with no check of its own, and every value
  it trusts goes straight into pointers and indexes (world_validation.md,
  match.md section 36). The match server checks every upload
  (`world::validate`, PR #289), so today one server bug or one missed
  check is enough to crash every game of a match.
- With this patch the game runs the **same check** on every world it is
  sent, before any native reader touches the world. On a refusal it
  leaves the match cleanly instead of crashing.

## 2. Prior work: srp-survarium/srp PR #38 (Codex)

PR #38, "Add no_std Rust packet validation for the 0.26g client" (branch
`security/026g-rust-packet-guard`, `client/026g/packet_guard/`), was read
for this design. It has no hooks and was never run in a client.

- **What it covers:**
  - a `#![no_std]`, `#![forbid(unsafe_code)]` core, with all unsafe code
    in a separate C ABI crate (`capi/`);
  - builds for i686 with `-Z build-std=core,alloc` and `panic=abort`;
  - UDP framing and fragment reassembly;
  - 0xE4 match setup;
  - the 0xE8 envelope only: the mask, each player block's u16 length,
    the animation bitstream (bit counts, dictionary indices, finite
    floats), the six sections' u16 lengths, the exact end. Player and
    world contents are `Unsupported -> reject`;
  - login and lobby.
- **What we reuse:**
  - the split: a safe no_std core and one small unsafe shim;
  - its `Reader` (`split_at_checked`, typed errors, `finish` at the
    exact end) and the build recipe (nightly, build-std, panic=abort,
    LTO);
  - its animation-bitstream checks, which are stricter than ours. They go
    into the shared core after they pass every kept world (section 8);
  - its hook landmarks, checked against our exe (section 4): 0x27BFF0
    (the 0xE8 handler) and 0x324D80 (the frame reader) are real. Here the
    hook goes on the two calls of 0x2D0B70, which is earlier than any
    reader.
- **What we leave out:**
  - login and lobby (the owner's scope is a match);
  - the UDP and fragment layer, and its bounded LZMA2 decoder. The
    0xE8 compression and every datagram's framing are made by our
    server, not by a client: the server re-packs uploaded worlds itself
    (`lzma2::packed`), and the native decoder's output is capped at
    0x8000 (0x27C059..0x27C09F). It can be taken in later as a guard
    against server bugs (section 9);
  - its schema-callback design. Our core holds the schemas itself.

## 3. The shape

```
 server 0xE8 --> process_dynamic_match_info 0x27BFF0 (edi = network_client)
                   |  gen test 0x27C006..0x27C040 (stores the gen)
                   |  compressed? native LZMA2 into a 0x8000 stack buffer
                   v
            call 0x2D0B70   <-- at 0x27C0D9 (compressed) and 0x27C111 (raw)
                   |
                   |  HOOK: both calls go to cv_world_load (NASM, the arena)
                   v
     cv_world_load: state = reader.pos .. reader.begin + reader.size
                   |  (the world's time u32, then the body)
                   |  world_hook (RUST_HOOK): the core, the state
                   |  (no_std Rust: srp_validate::world::validate on the
                   |   body, the time and the match from the core)
          ok  <----+----> refused
          |                 |
          v                 v
   jmp 0x2D0B70      reader.pos = end,
   (the game's own   close_current_match(2): 0xE0 leave to the match
    load, as before)  server, lobby 31 (left a match), world ended,
                      then out through the handler's own epilogue
                      (0x27C1E0), past the post-load steps
```

## 4. Where a world comes in (step 1 of the RE)

- **The only reader of a received world** is
  `game_world_core::deserialize` 0x2D0B70. It has three callers
  (`call rel32` scan of `.text`, PROVEN):
  - 0x27C0D9: 0xE8, compressed body;
  - 0x27C111: 0xE8, raw body;
  - 0x1EE377: a replay's key frame (the seek 0x1EE2E0, mode 0 or 2;
    replay_sur.md). This is not a relay. It is optional here (section 9).
- **0xE8 handler** `network_client::process_dynamic_match_info` 0x27BFF0
  (edi = the network_client, esi = the message reader; called from the
  match dispatch at 0x27A3FD). PROVEN:
  - The generation test is at 0x27C006..0x27C03F. A stale world goes to
    0x27C02E, which sets `reader.pos = begin + size` and returns. An
    equal or newer generation is stored at 0x27C040, **before** the
    load.
  - `compressed != 0`: 0x8000 bytes are taken on the stack
    (0x27C05E), and the codec at `[edi+4Ch]` vtable +4 decompresses into
    them with a cap of 0x8000. A temporary reader at `[ebp-14h]` is
    `{buffer, buffer, size}` (0x27C0C0..0x27C0D6).
  - The load is `push 8; push 1; push reader; push core; call 0x2D0B70`.
    The core is `[edi+1F1C40h] + 1F8h`. 0x2D0B70 pops 16 bytes
    (abi.tsv).
  - After the load (0x27C116..0x27C1DF) come the post-load steps: the
    local player, vtable +4Ch/+5Ch, 0x1F8190 / 0x1F8200, 0x2375C0. Then
    the epilogue at 0x27C1E0: `lea esp,[ebp-30h]; pop edi; pop esi;
    pop ebx; mov esp,ebp; pop ebp; ret`.
- **The reader** is `{u8 *begin; u8 *pos; u32 size}`, and the end is
  `begin + size` (0x27C02E..0x27C033). PROVEN.
- **Nothing reads the body before 0x2D0B70.** The handler takes 2 bytes
  (gen, compressed) and hands the rest on. Inside 0x2D0B70:
  - `0x324D80` walks the frame first (0x2D0B9C). It takes a mask bit k
    as slot k with no bound (`item + k*9054h`, 0x324DC0..0x324E15).
  - Then the history is cleared (0x2D0550, 0x2D0BD1).
  - Then the world is applied (0x2D2770 at 0x2D0FB9).
  So the entry of 0x2D0B70, seen from its two 0xE8 calls, is **the
  earliest point where the whole body and the match it is for are both
  at hand**. PROVEN.
- **Hook bytes.** Both are `call rel32`, and no base relocation is
  under a rel32. PROVEN:
  - 0x27C0D9: `e8 92 4a 05 00` -> `call world_check`;
  - 0x27C111: `e8 5a 4a 05 00` -> `call world_check`.
  Both calls run with ebp = the handler's frame (set at 0x27BFF1 and
  not changed before them), edi = the network_client, and the four
  arguments on the stack. PROVEN.

## 5. The refusal

- **What a refused world does:** it is not loaded, and the game leaves
  the match the way the leave button does, through
  `network_client::close_current_match(2)` 0x27A790 (thiscall, ecx = the
  network_client, one argument, `ret 4`). Reason 2,
  "disconnected_by_initiator", does this (match.md sections 24 and 25,
  PROVEN 0x27A828..0x27A93C):
  - sends 0xE0, a leave, to the match server, which tells the others
    0xEA;
  - sends `31 <order> FF` (left a match) to the lobby (0x2717E0);
  - ends the world (`game_world::on_finish_match` 0x1F96F0), and the
    player is back in the lobby.
- **The call can be made inside a match handler.** The 0xEA handler
  makes the same call from inside the dispatch (`close_current_match(3)`,
  0x27C720..0x27C76F). PROVEN for 0xEA. For 0xE8 it is LIKELY, by the
  same pattern. The in-game test settles it.
- **Out of the handler:** world_check sets `reader.pos = begin + size`,
  as the stale path does (0x27C02E). It then jumps to the handler's
  epilogue at 0x27C1E0 (`lea esp,[ebp-30h]` restores the stack, the 0x8000
  buffer included), so the post-load steps of a world that was never
  loaded do not run.
  - In the compressed path, the mutable buffer at `[ebp-24h]` has a null
    allocator (0x27C070). 0x7AB30 grows nothing within 0x8000, so
    skipping its destructor (0x27C0DE..0x27C0FB) frees nothing. PROVEN.
- **The generation** of the refused world is stored already (0x27C040).
  That is harmless: the match link is closed.
- **Why not "drop it and go on":** a first world that is dropped leaves
  the game frozen (`move()` returns at once until the first world,
  0x2D2E4A). A dropped resync leaves the game out of step, and it is
  resynced again with the same kind of world. A clean leave is the only
  end that is sure.

## 6. Loud

- **Game log:** one line on the `net_sync` channel through the game's own
  logger 0x111ED0, with the same arguments as "--- FULL RESYNC ---"
  (0x2D0F17..0x2D0F61). Its format string lives in the patch: `SRP: world
  refused: %s at 0x%x: %s (%u bytes)`, giving the part, the offset and
  the reason. The part and reason texts are a static table in the Rust
  core. LIKELY: level 2 is printed, as close_current_match's own line is.
- **Lobby:** the 31 message carries the leave flag as one byte
  (0x2717E0 writes 0 or FFh, 0x27182F..0x271844). For a refused world it
  is **02h**: the stub that sets the refusal also changes that one byte.
  The lobby server:
  - logs "player X's game refused a world of match Y";
  - counts it;
  - sends the player an important message (0xA4, `lobby::important_message`,
    as social.rs does), worded by the owner, one line.

  A lobby that does not know 02h reads it as a leave, the same as FFh
  (LIKELY: check how `M::Leave` decodes the byte before shipping).
- **Match server:** it sees an ordinary 0xE0 leave. Together with the
  lobby line, that is enough to find the world it sent (every world it
  sends is in `SRP_SNAPSHOT_DIR`).

## 7. What the check reads from the game, not from the world

Everything the server takes from `formed`, the client takes from the
game's own live objects. Nothing comes from the packet. With the core
(`[network_client+1F1C40h] + 1F8h`):

| context | client source | status |
|---|---|---|
| places of statistics and kd stats | 20: the late join makes both serializers write 20 records (`LATE_JOIN_PATCHES` 0x2F5ACF..0x2F5C03, 0x3237B4) and the server uses 20 for a growing roster (`snapshot_players`) | PROVEN |
| place table | `[core+18458h]..[core+1845Ch]`, 30h bytes a place; count = (end - begin) / 30h, as 0x2D3839..0x2D384F computes. A fixed vector: its storage is in the core at +18464h and ends 3C0h later (0x2CFF50..0x2CFF68), so it holds at most 20 places; the core's constructor fills it with 0x2D4C80 and a count that is its own argument (0x2CFF6F) | layout PROVEN; the count's source LIKELY the roster |
| kits of place p | entry `+0/+4/+8`: the kit's player object, null when the kit was not sent usable (0x321820). A kit byte is good only when its object is not null, which is what world apply dereferences (0x2D2A85..0x2D2AA0) | PROVEN |
| M, the head's item count | `byte [[kit_object+6B8h]+1564h]`: the head reader 0x2DB490 passes `[this+6B8h]` to 0x2F7D40, which reads that many u32 (items 0B4h apart from +1570h). World apply calls the kit object's vtable +3Ch (0x2D2A90); in `base_player`'s vtable 0xA10AAC that slot is 0x2DB490, and the client player's deserialize calls it with the same `this` (0x1F3A05) | PROVEN |
| world time | the body's first u32 (0x324DA0 stores it as the item's time) | PROVEN |
| rules and mode | the rule objects at `[core+18824h]..[core+18828h]`, 4 bytes each (world apply calls each one's vtable +40h, 0x2D2B20..0x2D2B5D). Each rule is known by its vtable, two of each class (table below); the mode is the class of the mode's rule | PROVEN |
| respawn points P | the respawn rule's three group counts `[+194h] + [+198h] + [+19Ch]`: its serializer walks the three groups (pointers +188h/+18Ch/+190h, counts 0Ch after each) and writes two bytes for each point (0x316F49..0x316FB6) | PROVEN |
| batteries | the gather rule's battery list `[+110h]..[+114h]`, 4 bytes each: its serializer builds the u16 mask from it and writes each one (0x313D10..0x313D9F); the containers are `[+130h]..[+134h]` (0x313DA1..0x313DC0). Mode 3's serializer reads the same two lists (0x314A3F, 0x314A98, 0x314ACD..0x314AE6) | PROVEN |

Rule object vtables (the address after the class's RTTI pointer; each
holds the class's serialize at +3Ch and deserialize at +40h, the
functions snapshot_rules.md section 2 names). PROVEN:

| class | vtables | +3Ch serialize | +40h deserialize |
|---|---|---|---|
| gather_victory_items_rule_core (mode 2) | 0xA06AB4, 0xA15B04 | 0x313CA0 | 0x313DD0 |
| match_time_vi_spawn_rule_core (mode 3) | 0xA06C04, 0xA15CAC | 0x3149B0 | 0x314B90 |
| tdm_rule_core (mode 4) | 0xA06CBC, 0xA15DA4 | 0x316340 | 0x3163D0 |
| player_respawn_rule_core | 0xA06D1C, 0xA16084 | 0x316F10 | 0x316FC0 |
| timelimit_rule_core | 0xA06E2C, 0xA06E8C | 0x317F30 | 0x317FD0 |
| kd_stats_rule | 0xA177D4 | 0x323870 | 0x323910 |
| Research's rule (mode 5, patch 53, `research_native_026g.c`) | `rn_rule_vtable + 4` (an arena symbol) | rn_serialize | rn_deserialize |

- **Mode 5 (added 2026-10-25).** Research's rule is ours, in the arena: the
  payload knows it by the address after `rn_rule_vtable`'s locator, which
  the link resolves. Its bytes (`rr_write`: 21, then 96 a unit; the unit
  count is byte 20) say their own length, so the check reads it from the
  kd stats back, as Team Deathmatch's, and holds it to what `rr_read`
  takes (progress, teams, the counts of the places inside) and every place
  it names within the roster. No batteries in the statistics (as on main's
  server, `mode::WITH_BATTERIES`). Without this every Research world was
  refused (section 15).

- The shim finds the respawn rule and the mode's rule in the list by
  these vtables. A list that has neither, or two mode rules, is a
  match the check does not know: the world is refused (strict, and
  loud: the reason says so).

- Then the client check is **at least as strict as the game needs**: a
  kit byte is refused unless the object the game will call exists.
- The client check may be **stricter than the server's** on one point:
  the server checks KIT_USABLE in the profiles it sent, while the
  client checks the objects. They should agree (0x321820 makes objects
  exactly for the usable kits). The drift test checks it on every kept
  world.

## 8. One code base: no_std Rust shared with the server

- **The crate:** `srp-026g/crates/srp-validate`, `#![no_std]`,
  `#![forbid(unsafe_code)]`, no `alloc`; it denies indexing, unwraps,
  panics and unchecked arithmetic outside its tests. It holds the readers
  `world::validate` needs (a bounds-checked `Reader`, the frame walk and
  the animation bit stream's length, the player head and tail, bullets,
  statistics, game objects, rules, physics, effects) and
  `messages::check` (every id 0xE3..0xF6 at the dispatch). The context is
  a plain struct (`world::Context`): the server fills it from its match,
  the game from its own objects.
- **The server side stays in #282/#289.** This PR adds the crate and the
  game's side only; the match server switches its `world::validate` to the
  crate in #289's line of work (the first build's commit "World check: the
  server calls the shared srp-validate crate", c72bbb72 on
  `feat/world-validate`), so the two never drift.
- **Brought up to main (2026-10-25):** Research (mode 5, section 7) and the
  late join's profile proposal in 0xE4 (section 10); both were refusals of
  good traffic (section 15).
- **The game's crate:** `client-patches/rust/client_verify`, a crate of the
  Rust patches' framework (`rust_patches.md`, `patch-core`): the only code
  that reads the game, through `patch-core`'s views (`Field`, `Obj`,
  `GameFn`). It reads the world core's rule list once into a `Match` (the
  mode's rule by its vtable, its batteries, the respawn rule's points; no
  mode rule, two, or no respawn rule is no match the check knows:
  refused), a reader into its bytes, and calls the crate. Its four hooks
  are `RUST_HOOK`s (`client_verify_026g.asm`): `world_hook` (the game's
  stack: the core, the reader; EDI the client), `message_hook` (EBX the
  reader, [EBP+0Ch] the id, ESI the client), `anomaly_hook`,
  `artefact_hook`. Each answers with ZF (set: the game goes on as before);
  a refusal is the hook's own work (the reader consumed,
  close_current_match(2) as a thiscall `GameFn`), the entry only jumps.
  The logic (`Match::of`, `check_world`, `in_table`) is tested on the host
  (`cargo test` in `client-patches/rust`).
- **The build:** link_026g.py's Rust crates: rustc alone, i686, static,
  `opt-level=s`, `panic=abort`, fat LTO, one object, linked into the arena
  block `client_verify.code` with the NASM entries. No separate section, no
  blob in the repo, no target spec: the first build's `.srpv` and its
  soft-float target are gone. The link's guards hold: no undefined symbol,
  no data or state. 2026-10-25: 14,520 bytes.
- **The drift test** (`check_client_verify_026g.py --drift`): every kept
  world (the local sets, 4,138 on 2026-10-25, and the four worst cases of
  section 14), with srp-validate's `drift_dump` example choosing the match
  at 20 places (the payload's roster) and giving the host build's verdict
  for the world and 16 copies with one byte flipped each; the payload runs
  under Unicorn on the same bytes with a world core whose rule list says
  that match. 2026-10-25: **70,350 checks, 0 differ** (3,089 worlds read,
  1,053 refused: the pre-late-join worlds of fewer than 20 records and the
  `SRTD` worlds of section 15).
- **The entries** (same script): cv_world_load, cv_message, cv_anomaly
  and cv_artefact on made-up objects at stack alignments 0, 4, 8 and 12:
  read, each goes on exactly as its site did (the arguments, the stack and
  the kept registers); refused, the reader is consumed,
  close_current_match(2) is called with the network client, and the
  caller's epilogue (0x27C1E0, 0x27A72C) is reached with its frame.

## 9. Numbers, order

- **Patches 75..78** (main and the open PRs use up to 74; 73 stays free for
  the closed #469): 75 the world and message check, 76 the weapon-FSM
  guard (0x38CD22, in place), 77 the anomaly guard (0x31B0FF), 78 the
  artefact guard (0x2FE65E). Hooks by symbol (`CLIENT_VERIFY_HOOKS`,
  `CLIENT_VERIFY_DISPATCH`); the dispatch's jump held the table's address,
  whose base relocation is taken out. With the late join only (the arena).
- **The self-test's call shapes:** cv_world_load is the callee of two
  `call 0x2D0B70` (`ret 10h`); its refusal leaves by the handler's epilogue,
  which `LEAVES` names, so the shape check does not count that `ret` as
  ours.

## 10. Every match message, not only worlds

The owner, 2026-10-22: every match message a client receives is
checked in the exe before a native reader touches it. Login, lobby, the
UDP and fragment layer and LZMA2 stay out.

- **The dispatch** `network_client::on_match_packet_received` 0x27A300
  (thiscall: ecx = the network_client; stack: u8 id, the reader; its
  one reference is a bind at 0x219C81) reads the id and jumps by the
  table at 0x27A738 (20 entries, 0xE3..0xF6). PROVEN.
  - The reader is the message's own: a handler that skips a message
    sets `pos = begin + size` (0x27A36B, 0x27A3B2, 0x27A5AB), which
    must skip that message only. PROVEN by that use.
  - So one hook at the dispatch's entry sees every message with its
    id and exact bytes. Entry bytes `55 8b ec 83 ec 64` (6 bytes:
    push ebp / mov ebp,esp / sub esp,64h) become a `jmp` to the check
    and a nop; the check does them and goes on at 0x27A306.
  - Handlers by id (jump table, PROVEN): E3 0x27B830, E4 0x27B880,
    E5 0x27B570, E6 0x27B770, E7 0x27B7F0, E8 0x27BFF0, E9 0x27C5F0,
    EA 0x27C690, EB/EC 0x27C280, ED/EE 0x27C340, EF/F0 0x27C7B0,
    F1/F2 0x27C8D0, F3 0x27C980, F4 0x27CA30, F5 0x27CB50, F6 inline
    at 0x27A4F4 (srp's upload hook at 0x27A505).
- **0xE4 and 0xE8 are compressed**, so the dispatch checks only their
  envelope; their contents are checked after the native decompressor:
  - 0xE8 at 0x27C0D9 and 0x27C111 (section 4);
  - 0xE4 at 0x27B928, `e8 43 55 f7 ff` = `call 0x1F0E70`
    (`match_options::deserialize`; eax = the decompressed reader at
    `[ebp-2Ch]`, stack: the options at `[vcall +40h]+0Ch`, `[game+53A4h]`).
    PROVEN (0x27B90E..0x27B928).
- **Indexes the handlers take from the message with no bound**
  (PROVEN at the reader; what the code after it does was not traced, so
  the check bounds them):
  - 0xEB/0xEC: the input reader 0x318400 stores the player byte at
    node+0Ch (0x31844F..0x318458) and the two halves and the mask with
    no check; `change_input` 0x1FAB50 then takes the player's lists;
  - 0xE9: the handler passes the place byte to `enter_match` 0x2D1470
    (0x27C652..0x27C67C), which tests only the time first (0x2D1489).
- **The bounds the shared crate checks** (section 8):
  - a player or place byte under 20, the place table's capacity;
  - the input halves as `PlayerInput::rotation_is_sane`, and the action
    bits stock clients set (relay_trust.md: 29 and 30 never);
  - each id's exact length (transport.md section 4), and for 0xE4 the
    profiles and options as #38 checks them;
  - 0xED/0xEE, which srp never sends: refused.
- **Built (2026-10-22): `srp_validate::messages`.**
  - `check(id, payload)`, the payload after the id:
    - 0xE2 and ids outside 0xE3..0xF6: refused; 0xED/0xEE: refused;
    - a fixed id: its exact length (E3 4, E5 7, E6 4, E7 2, E9/EA 2,
      EB 10, EC 11, EF 2, F0 4, F1 1, F2 2, F3 3, F4 2, F5 1, F6 1: srp's
      0xF6 is one byte the patch ignores);
    - E9/EA: the place byte under 20; F3: the player byte under 20;
    - EB/EC: the player byte under 20, both rotation halves numbers under
      1024, no action bit 29 or 30;
    - E4: at least the local index (under 20) and a byte of stream; E8:
      the generation, a compressed flag of 0 or 1 (all srp sends), and at
      least the time and the mask of a raw state.
  - `options(bytes, dictionary)`, 0xE4 unpacked: #38's validator, fitted
    to what srp sends (a place past the roster is 9 zero bytes, team 0 or
    1 in the roster, a dict id 0 is an empty item with nothing after it,
    a kit slot names an item that is not empty); #38's rule that item ids
    are unique and not 0 is not ported (no index use of them is known).
    The game's items and maps come through the `Dictionary` trait: the
    server from its dictionary, the game from its own.
  - `input(rotation, actions)`.
- **The late join's proposal (2026-10-25).** A 0xE4 that starts `FF 'L'
  'J' 01` is the late join's profile proposal (`late_join::Prepare`, every
  client of the dialect takes it mid-match), not static info for local
  place 255: the check takes its 14-byte header (a serial not 0, a place
  1..19) and leaves the profile to the late join's own check
  (`late_join_validate_026g.c`). The first build refused it, so every late
  join would have sent the whole match to the lobby (section 15).
- **The server can use the same functions** (on `feat/world-validate`, for
  #289's line of work; not in this PR): an input that fails `input` is
  not relayed (its owner gets 0xF1, as for a rotation before); options
  that fail `options` are no static info (`StaticInfoError::Refused`);
  a test sends every message kind through `check`
  (`every_message_the_server_sends_is_one_a_game_takes`).

- **The dispatch hook (built 2026-10-23, in the arena 2026-10-25).**
  `message_hook` is hooked at the dispatch, so every message
  0xE3..0xF6 runs it before its native handler. The hook is at
  **0x27A330**, the `jmp [eax*4+0x27A738]` itself (not the entry 0x27A300 of
  the plan above): there the frame is set up and esi = this, ebx = the
  message reader, [ebp+0xc] = the id, so a refusal can leave through the
  dispatch's own epilogue with no prologue to replay. The 7 bytes become a
  `jmp cv_message` (the arena) and two nops; cv_message (a RUST_HOOK) aligns the stack
  for the Rust call and puts it back; its reach is `jmp [eax*4+0x27A738]`
  again on ok, and on a
  refusal: consume the message (reader.pos = begin + size), close the match
  (`close_current_match(2)`, as the 0xE8 refuse, section 5), out through the
  dispatch's epilogue **0x27A72C** (`lea esp,[ebp-70h]`; pops; `ret 8`), the
  native handler never reached. The table displacement's base relocation is
  neutralized (like scene::remove_model's). Because the hook is after the
  bound check (0x27A327), every id it sees is 0xE3..0xF6, and because it
  refuses before the dispatch, the one refuse path is clean for all of them
  (the `.text` clean-exit of the plan's caveat is thus settled: the
  dispatch's epilogue serves every id).
  - PROVEN by `check_client_verify_026g.py` (Unicorn, the linked arena): a
    good message reaches its handler through the jump table with the
    site's stack and registers; a bad one is refused --
    close_current_match(2), the message consumed, out through the epilogue,
    no handler. Every message of the local packet logs (153,820) is read.

## 11. Other relay checks that can move to the client cheaply

| check | today | client side | size |
|---|---|---|---|
| input rotation (0xEB/0xEC halves under 1024, server `PlayerInput::rotation_is_sane`) | server only | the input reader 0x318400 stores dx/dy unchecked: a half with exponent >= 1024 is zeroed there (or the input dropped), by the same Rust function moved into the core | small |
| bit 26 from a hand with no weapon | client, patch 50 (#286) | done | - |
| kit choice with no weapon (0xF3) | server kit filter + patch 51 (#286) | done | - |
| names and chat as HTML in the movies (`LobbyChat.as` `get_styled_text` / `linked`, HUD `GameChat.as`) | server character rules only (#288 closed) | escape `& < > "` in names and text before `htmlText`; staff colour from the server's `message.color` only. A content patch (lobby movie from AS source; the HUD movie apart) | small..medium |
| 0xE4 static info (profiles, names, kits, items) | server checks (`check_profile`, `make_safe`) | #38's E4 validator ported before `match_options::deserialize` 0x1F0E70 (call at 0x27B928); the late join's profile check (`late_join_validate_026g.c`) already does part of it for a newcomer | medium |
| 0xED/0xEE batched relay | never sent by srp | refuse in the client (#38 checks the masks and count) | small, low value |
| UDP framing, parts | server-made | #38's transport layer | large, out of scope (no client bytes) |

## 12. The artefact-slot guard (patch 78; first built as 57)

A relayed world's player state carries artefact-history items the validator
does not reach (the player-tail wall, world_validation.md section 10), so an
artefact-id byte is checked by neither side. A game reads it in
`game_state_artefact_history_item::deserialize` and, for an id other than
0xFF, indexes the global artefact table by it with no bound (PROVEN):

- 0x2FE64F `mov ecx,[edi+108h]` (edi = the player; ecx = the inventory object
  that holds the table), 0x2FE655 `mov edx,[ecx+18940h]` (the table's begin),
  0x2FE65B `movzx eax,al` (the id), 0x2FE65E `lea eax,[edx+eax*4]`
  (`&table[id]`), 0x2FE661 `mov eax,[eax]` (the slot). The vector's end is at
  `[ecx+18944h]`. An id past the table reads a pointer from beyond its end;
  non-null garbage becomes the current artefact and is refcounted through
  (0x2FE66B `lea eax,[ebp+0D0h]` / `lock xadd`) -- the crash. The clean
  no-artefact exit is 0x2FE67A (ebp = 0), which an id of 0xFF reaches.

- **Patch 78.** The index+read (0x2FE65E..0x2FE663, 5 bytes: `lea`, `mov
  [eax]`) becomes a `jmp cv_artefact` (the arena). It bounds `&table[id]`
  against `[ecx+18944h]`: in range it does the read and returns to 0x2FE663
  (the game's own `xor ebp,ebp` / null test continues); past the end it
  takes the game's own no-artefact path (ebp = 0, 0x2FE67A), as 0xFF would.
  No address is in the replaced bytes, and no base relocation is under them.
- **Patch 77, the anomaly's state id,** alike: 0x31B0FF `mov eax,[edi+68h] /
  movzx edx,cl / mov ebp,[eax+edx*4]` (9 bytes) becomes `jmp cv_anomaly`,
  which bounds the id by the state vector ([edi+68h]..[edi+6Ch]) and takes
  the game's no-state path (0x31B0FB, as id 0xFF) past it. The first build
  needed an int3 cave in `.text` (0x842AE2) for it; the arena needs none.
- **Patch 76, the weapon FSM's state id** (ai::fsm::deserialize 0x38CCF0):
  an id no state has leaves [esi+10h] zero and the tail calls through it;
  the tail (0x38CD22, 30 bytes with its int3 padding) is rewritten in place
  to `ret 8` without the call then (FSM_STATE_GUARD).
- **Proof.** `check_client_verify_026g.py` (Unicorn): an id in the table
  reads its slot as the game does; one past it (and 0xFE) takes the none
  path.

## 13. Deferred (needs a game, or risky; not built)

These need in-game memory or carry risk a static/Unicorn pass cannot settle;
the residual guards (patches 76..78) already hold the crash sites that filling
the context would have bounded, so the check is safe without them.

- **The 0xE4 options content check** (`messages::options`, section 10). The
  dispatch checks only 0xE4's envelope (the local index and the LZMA2 bound);
  the unpacked profiles/names/kits/items are checked after
  `match_options::deserialize` 0x1F0E70 (call at 0x27B928), which needs the
  game's own items `Dictionary` from live memory (the `Dictionary` trait,
  section 10). Not hooked.
- **The structured log line on a refusal** (section 6). The payload gives a
  verdict only (the first build's `CRefusal` is gone) and logs nothing: the
  native logger 0x111ED0 is an 11-argument varargs call, risky without an in-game
  trial. A refused world or message leaves the match (and the match server
  keeps the world it sent, `SRP_SNAPSHOT_DIR`); the log line is a later nicety.
- **The lobby 02h leave-reason byte** (section 6). A refusal leaves as an
  ordinary FFh leave for now; the lobby reads it the same. Setting the 02h
  byte (so the lobby counts refusals and messages the player) is deferred.
- **Filling `Context.profiles` / `inventory` / `objects` / `artefacts`** from
  live game memory (section 7). `Match::context` leaves them `None`, as the
  server does, so the check never refuses a good world over a value it had to
  guess; the deep per-player/per-item bounds those would add are held by the
  residual guards instead. Reading them from the live world is a later,
  in-game step.

## 14. The cost (measured 2026-10-25)

The owner, 2026-10-25: "investigate its performance complications"; client
hitches come first. Two measures of the same code:

- **Host build** (`cargo run --release -p srp-validate --example bench --
  --worlds DIR... --packets DIR`): ns a call, the median of five batches.
  This is also what the match server pays (an x86-64 release build).
- **The game's build** (`check_client_verify_026g.py --counts`): the
  instructions the linked i686 payload runs under Unicorn, entry to `ret`.
  No CPU model: at 1 to 3 instructions a ns (branchy code, a 3 GHz CPU of
  the last ten years), 20,000 instructions are 7..20 us.

Data: the local kept worlds (4,138: `private/local-clients/runtime/snapshots`,
`private/fov-rev11/snapshots`), the 133 distinct worlds the server sent in
the local packet logs (0xE8, unpacked), every match message of those logs
(153,820 in 144 logs), and four worlds made to the caps.

| what | host | i686 instructions | i686, est. |
|---|---|---|---|
| a match message, any id (E3..F6) | 3..14 ns (median 4) | 68..169 (0xEB 112) | < 0.1 us |
| a world, median (passing, 2..6 players) | 0.77 us | 19,148 | 6..19 us |
| a world, p99 / max of the real ones | 0.97 / 1.28 us | 24,187 / 25k | 8..24 us |
| 20 players (one kept player 20 times, 24 KB) | 3.0 us | 97,172 | 32..97 us |
| the rules section at its cap (0x800), mode 2 | 3.0 us | 95,945 | 32..95 us |
| 135 bullets that frame at every offset (33 KB) | 43 us | 2,299,165 | 0.8..2.3 ms |

- **Against the frame and the tick:** a world's check is 0.05..0.1 % of a
  16 ms frame, 0.2..0.6 % with 20 players; nothing a player can see. A
  world comes with a resync, a join or a snapshot load, never every tick;
  the game's own work for it is far more: the native LZMA2 decode of the
  same world (the host's Rust decoder takes 19 us for the median one) and
  world apply 0x2D2770, which rebuilds every player and object. Messages:
  0xEB, the most frequent (a relayed input, 109,370 of the 153,820), costs
  112 instructions; 20 players at 20 inputs a second each is under 0.1 ms a
  second.
- **The worst cases are made, not seen.** The bullets case needs a section
  of 9 KB of records whose every byte is 7 (real worlds: 0x230 at most) and
  a body over the client's 0x8000 buffer; without profiles the framing tries
  every pellet offset of every record (`bullets::guessed`, O(n^2), 9,000
  record checks at 135 records). Only a server bug or a server that relays
  unchecked worlds could send one. Two cheap fixes went in with the port:
  - the gather rule's search: with the map's respawn points known (the
    payload always knows them) the rule is read at its one offset, not at
    every even offset (O(n^2) in the section): the rules at their cap went
    from 540,202 instructions to about 95,000, and the median world from about
    23,000 to 19,100;
  - the frame walk: one walk finds both where the blocks end and the first
    state over its buffer (was two; the animation bit stream is read bit by
    bit, about 1,250 instructions a block).
- **Where the rest goes** (a median world, 34k instructions before the
  fixes): the gather search 28 %, the frame walks 22 %, the statistics'
  20 records 20 % (always 20: the late join's roster), the player heads
  and tails 10..15 %.
- **Nothing needs a thread.** The check is pure and runs before the game's
  own reader on the logic thread; at these costs a thread hop would cost
  more than the check.

## 15. False refusals: a good world refused sends a player to the lobby

A refusal is a kick (close_current_match(2)). The checks are strict on
purpose, so every format change of a world or a message that main makes
must reach them first. Found and fixed 2026-10-25, by running the check on
every kept world and every message of the local packet logs:

- **The world's time (the first build, every world).** The reader at
  0x2D0B70 holds the time (u32) before the body, as `client_model.rs`
  splits it (`state[4..]`); the first build gave the whole reader to
  `world::validate`, which takes the body from the mask on, and took the
  time from the body's first u32 (the mask). In a game every world would
  have been refused at the frame. The drift test did not see it: it gave
  both builds the kept body (no time), so both refused alike (its 2,571 of
  2,571 "agree" were refusals).
- **The late join's 0xE4 proposal (every late join).** `FF 'L' 'J' 01`
  (section 10) was refused as static info for place 255: the first late
  join of a match would have sent every game in it to the lobby. 20 of 164
  0xE4 in the packet logs.
- **Research (every mode 5 world).** No mode rule the payload knew: refused.
  878 kept Research worlds now pass.
- **Open, not on main:** #370 (patch 63, team damage) appends a 165-byte
  `SRTD` block to the effects section; the effects template (20 or 87
  bytes) refuses it. 780 kept worlds of local #370 / blackout tests. #370
  (and any PR that changes what a world or a message holds) must change
  srp-validate in the same PR, and its release must ship the exe with it.
- **Result on main's format:** of 4,271 local worlds, every one passes
  under its own match (0 false refusals); the 306 that need fewer than 20
  places are worlds from before the late join's 20 records (server runs up
  to 2026-10-14); 0 of 153,820 messages refused.
- **Live worlds:** the owner runs `client_verify_live.sh` on
  `private/live-worlds-*` (bench, explain, drift, counts); not run here.

## 16. Server or client (the owner, 2026-10-25: "if it is expensive on client, can we do that server side?")

It is not expensive on either side; the question is where a refusal is
safe.

- **What the server pays** for the same checks (the host column of section
  14, its single core): one world check per upload (0xF6) before it is
  relayed, about 1 us (3 us at 20 players); one message check per
  relayed input, 4 ns. At 20 players, 20 inputs a second each and an
  upload every few seconds each: under 0.1 ms of CPU a second.
- **What the server checks exactly:** everything a world holds that the
  match server knows: the frame and caps, the heads (with the profiles it
  sent, which the client does not read), bullets with the kits' weapons
  and ammo (linear, no pellet guessing), statistics, rules, physics,
  effects, every message's length and indexes (#282, #289).
- **What the server cannot check exactly:**
  - the game objects of the 74 anomaly maps: their loop counts are each
    game's runtime state, not in the map data (world_validation.md section
    9): an anomaly's state id (patch 77);
  - a player state past the inventory header: a weapon's FSM has no length
    the exe gives (patch 76), the artefact-history ids (patch 78);
  - what the game has live: its kit objects, its artefact table.
  These stay on the client as read-site guards: a bound at the one
  instruction that indexes, the game's own "none" path past it, no parse,
  no refusal, nothing to keep in step with the server.
- **The proposed split:**
  - server: the full world and message validation (#282, #289, on the
    shared crate);
  - client: the read-site guards (76, 77, 78, and #286's 50 and 51), and
    the dispatch's message check (75's cv_message: lengths and indexes at
    under 170 instructions; it guards against a server bug in a message the
    server makes itself, and its format changes rarely);
  - the client's full world check (75's cv_world_load): its cost is
    negligible (section 14), but a refusal is a kick, and every world
    format change must reach the exe in the same release, or good worlds
    kick players (section 15: three such bugs in one week). It is a guard
    against server bugs only, which the server's own check of what it
    relays already covers. Kept as its own commit so it can be left out.
