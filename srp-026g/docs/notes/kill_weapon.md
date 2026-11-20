# Which weapon a kill, a shot and a hit were made with (research, 2026-11-02)

The owner wants per-weapon player stats on the site: the weapon of each kill, and shots fired and hit
per weapon. What the 0.26g game gives the server, and what a client patch would take. Static analysis
only (the original exe, the 0.21d PDB); nothing run in a game. Addresses are VAs (image base 0x10000).
Tags: PROVEN (read in the code at the address given), LIKELY, GUESS.

## 1. What the server gets today: no weapon

- **No message a client sends names a weapon** (PROVEN). The client->server match messages are
  0xD8..0xE1 (`match.md` 1.1): connect, port probe, ready, time reply, the inputs (`actions_mask`
  and rotation only), the kit choice (0xDE, one byte), the hash, leave and keep-alive. No kill
  feed, death record or hit/damage message exists in either direction (`match.md` 2.4).
- **The patched client's world upload** (0xE0 chunks, `match_msgs/upload.rs`; client patch 3.2) is
  the only state the server sees. In it:
  - the statistics blob counts the 28 events per place **per kit** (`events[28][3]`,
    `snapshot_rules.md` 3.1; `crates/survarium/src/v026g/match_msgs/snapshot.rs:294-296`): kill 0,
    headshot 15, melee 13, grenade kill 25, shots 22, hits 23, each by kit, not by weapon (PROVEN);
  - the damage record of a player is per inflictor and kit (the f32 sum 0x2F58E0 adds to, handler
    slot +18h, called at 0x2DA10A), with `last_hit_ms` and `last_hit_enemy_id`: no weapon (PROVEN);
  - each player block has the inventory: `m_active_slot` and every item with its slot and, for a
    weapon, its magazine (`snapshot_rules.md` 7.4). That is the state at the world's tick, a few
    seconds apart, not an event (PROVEN for the fields; the server reads only the supplies);
  - the bullet manager blob has the bullets in flight, each naming its weapon by the shooter's slot
    (`client_patch.md`, patch 85 "The bullets": bullet +254h written at 0x32C0B5). Transient: a
    bullet that hit is gone by the next world (LIKELY).
- **Where the server's "killer" comes from** (PROVEN): nowhere on the wire.
  `tdm_death_costs_ticket` (`crates/survarium/src/v026g/match_msgs/rules.rs:297`) is a model of the
  client's own Team Deathmatch rule (0x31614E..0x31618C), used by its tests only. The match server
  pairs killer and victim from two vouched worlds' statistics: when exactly one place gained kills
  and the deaths gained add up to them (`servers/match-server/src/v026g/kill_cap.rs:32-68`, called
  from `game.rs:3857`); kills and deaths per place come from the kd rule (`snapshot_rules.md` 2.7).
- **Replays** are the relayed inputs and uploaded worlds (`replay_sur.md` 4, 5): a client playing
  one computes every kill again, but nothing in the file names the weapon (PROVEN by the format).
- The kill feed's row is local hud only: the world's event node (+0 victim, +1 killer, +2 the
  weapon's `combat_log_icon`, +4 cause; `client_patch.md` 3.55) is played by the hud
  (0x23B5F0 -> `game_world_ui::on_player_killed` 0x237C70) and never sent (PROVEN). The icon is
  not an id either: ported weapons share stand-in icons (`kill-feed-icons.md`).

### What the server can do with no patch

- A kit has two weapon slots, 7 and 10 (the client's table 0x9266A0; `session/src/v026g/rules.rs:806`),
  and the static info names each kit's slot items (`static_info.md` 3.1). So the per-kit events
  are **exactly per weapon for a kit with one gun** (slot 10 empty): kills (event 0 less grenade
  kills 25; melee 13 is the gun's own butt, `[bullet+40h] == weapon+53Ch` at 0x2DA2CD), shots 22,
  hits 23 (LIKELY: grenade shrapnel hits may count as event 23 too, see 3).
- A kit with two guns mixes them; a gun picked up off the ground (patch 85) is counted under the
  picker's kit. So the no-patch split is right for one-gun kits and a mix for the rest.

## 2. Where the client knows the weapon

The statistics handler (`base_game_statistics_handler`, vtable 0xA0755C, the same slot order as
0.21d's 0x814A1C) is told the weapon at all three events and drops it (PROVEN):

| event | caller -> handler slot | weapon in the call | the counter |
|---|---|---|---|
| kill | `base_player::generate_killed_event` 0x2DA170 -> slot +0 `on_player_killed` 0x26DA00 (17 stack args, `ret 44h`; call at 0x2DA46C) | arg 4: the damage source's **dict id** (`word [[source+25Ch]]`, 0x2DA3F7..0x2DA404); arg 13: the killer's **item instance id** (below); arg 17: the bullet | `shared_statistics::on_player_killed` 0x2F4EB0 (from 0x26DA3F) gets time, killer, killer kit, victim, victim kit, headshot, melee, carrier, the source's **category**, one more; not the id |
| hit | `base_player::generate_hit_event` 0x2D9F40 -> slot +4 `on_player_damaged` 0x26DA50 (10 args, `ret 28h`; call at 0x2DA164) | arg 7: the attacker's item instance id; the bullet is EBP in 0x2D9F40 | `shared_statistics::on_player_damaged` 0x2F5450 (event 23, from 0x26DA6C) gets attacker, kit, victim, a flag, melee |
| shot | `base_player::on_fire` 0x2D7800 -> slot +8 `on_player_weapon_firing` 0x26DA80 (5 args, `ret 14h`; call at 0x2D78A5) | arg 3: the shooter's item instance id | `shared_statistics::on_player_weapon_firing` 0x2F5500 (event 22, from 0x26DA91) gets place, kit, count |

- **The dict id** (PROVEN): an inventory item's +25Ch is its items-dictionary entry (`is_stack` at
  its +22Ah read at 0x2FD49A, as `static_info.md` 3.2 says); the entry's +0 is the word read from
  `dict_id` and +228h the byte read from `item_category` (the loader 0x2C8FBF and 0x2C90CD, strings
  0xA020EC, 0xA0EAE0). For a bullet the damage source is the weapon's inventory item: try_pierce
  passes `[bullet+0Ch]` (0x32D9E6) as `base_player::hit`'s last argument, which `hit_info::hit_info`
  stores at +40h (0x32BA47), which hit_impl passes on (0x2D9F03); the bullet's +0Ch is the weapon
  core + 10h (0x32BB12, the weapon from instant_fire's `mov edi,ebx` 0x2E2518). For a grenade or a
  trap it is that item (0x2D9AE0's own argument, stored at hit +40h; LIKELY that it is the item).
- **The instance id** (PROVEN): `profile [player+6B8h]`, kit `[player+115h]`, slot `[weapon item+254h]`
  (hit, kill: the bullet's weapon, 0x2D9F97, 0x2DA1EA) or the inventory's selected slot
  `[[player+10Ch]+170h]` (shot, 0x2D7854): `item = word [profile + 98h + (kit x 1C1h + slot) x 4]`
  (the kit's slot table, `static_info.md` 3.1), `id = dword [profile + 1568h + item x 0B4h]`, the
  lobby's item id. No bullet (a grenade, an anomaly): slot 17h, id 0. A picked-up gun (patch 85)
  sits in the picker's slot, so this id is the picker's own item, not the gun: the dict id is the
  one to key on.
- The kill's arg 3 and the shot's arg 4 (and `bullet+58h`) are `weapon_core::player_stance`
  0x2E43A0 at the shot (the bullet takes it at 0x32BAED): no weapon id (PROVEN).

## 3. How the client counts shots and hits

- **Shots** (PROVEN): `weapon_core::instant_fire` 0x2E23A0 fires the ammo's pellet count
  (`[[weapon+538h]+2ACh]`, 0x2E2467) of bullets (`bullet_manager::emit_bullet` 0x322830 each), then
  calls its user's +110h interface slot +4 (`[weapon+594h]`, 0x2E258D) with that count:
  `base_player::on_fire` 0x2D7800 (0.21d 0x1D3980, the same body) tells its fire listeners, then
  the handler, and 0x2F5500 adds the count to `events[22][kit]` (u16, `[record + kit x 2 + 84h]`).
  A shotgun's shot counts its pellets.
- **Hits** (PROVEN): every hit that took health (hit_impl, `hit_impl` row of `abi_inputs.tsv`) ends in
  `generate_hit_event` 0x2D9F40; its handler call reaches 0x2F5450, which adds 1 to
  `events[23][attacker kit]` (`[record + kit x 2 + 8Ah]`) when the attacker is a place, of the other
  team, the call's flag (0x2D9F40's 7th argument) is set and the hit is not melee. One hit a pellet.
  A hit with no bullet (0x2D9AE0's path: grenades, traps) also reaches it (LIKELY counted the same).
- **Kills** (PROVEN, `results.md` 9): 0x2F4EB0 counts event 0 under the killer's kit for an enemy,
  4 for a teammate, 3 for no killer or oneself, plus 15, 13, 10 and 25 (the category test at
  0x2F514E).
- **Per kit in memory, never per weapon** (PROVEN for the record: `events[28][3]` of u16 in each
  0x1F8-byte place record from `shared_statistics`+0, 20 places, then the battery manager at
  +2760h). The three handler thunks are the only sites that see the weapon, and they keep nothing.
  That no other object counts per weapon: LIKELY (the statistics are the only counters of these
  events that the hud and the results read).

## 4. The smallest patch that reports it (proposed, not written)

A Rust patch in the arena (`rust_patches.md`, `client_patch.md` 0.3/0.5), like patch 92
(`battery_progress`): its state travels in the world, so a rollback takes it back with the native
counters. Name proposed `weapon_stats`; the number and revision are the release's (AGENTS.md).

- **State** (a tagged table, one per shared statistics object): per place (20), at most 16 entries
  `{u16 dict_id, u16 kills, u16 shots, u16 hits}`, 2.5 KiB. `dict_id` 0 = no weapon known (a hit
  with no bullet), so the per-place sums equal the native totals over kits of events 0, 22 and 23.
  A 17th weapon in a life of the match goes to entry 0.
- **Sites** (`sites_026g.tsv` lohooks; six, 42 bytes of original code displaced):

| site | VA | span | holds | does |
|---|---|---|---|---|
| `weapon_stats.killed` | 0x26DA00 | 9 (`push esi / mov esi,ecx / mov eax,[esi+3238h]`) | ECX the handler (+8 the shared statistics), frame ESP: +8 killer, +10h dict id, +24h victim | kills +1 for (killer, dict id) when the killer is a place, not the victim, of the other team (0x2F4EB0's own tests: 0x2F5028..0x2F504D) |
| `weapon_stats.hit` | 0x2DA10C | 6 (`mov ecx,[esi+120h]`) | ESI the victim, EBX the victim's place, EBP the bullet or 0, frame ESP: +24h attacker, +34h melee byte, +38h the flag | hits +1 for (attacker, `word [[[ebp+0Ch]+25Ch]]` or 0) under 0x2F5450's tests |
| `weapon_stats.fired` | 0x2D7843 | 9 (`mov ecx,[esi+10h] / mov ebx,[ecx+18FA8h]`) | ESI the player + 110h, frame ESP+14h the count | shots + count for (place `[esi+4]`, the held weapon `[esi+18h]`'s `word [[w+26Ch]]`) |
| `weapon_stats.save` | 0x2F5BD0 | 5 (`pop edi / pop ebx / mov esp,ebp / pop ebp`) | ESI the statistics, EDI the writer | writes the table after the 11-byte tail |
| `weapon_stats.load` | 0x2F5C83 | 7 (`pop edi / mov [esi+31C4h],ecx`; ECX kept) | ESI the statistics, EDI the reader | reads it within the reader's size; an older world (none) loads an empty table |
| `weapon_stats.reset` | 0x2F4C31 | 6 (`mov [edi+31C0h],ebx`) | EDI the statistics | empties the table (the reset between matches, 0x2F4C20) |

- **Wire** (after the tail, so the native reader is untouched): `u8 version 1`; per place
  (N = `[stats+31E0h]`): `u8 n`, n x `{u16 dict_id, u16 kills, u16 shots, u16 hits}`; then `u16`
  the block's own size, so the server finds the tail before it. About 1 + 20 + 8 x weapons bytes
  (some 500 for 20 players with 3 weapons each; worlds are 11 to 14 KB of the 32 KB buffer).
- **Engine calls:** the byte writer/reader the natives use (w<u8> 0x1F3DE0, w<u16> 0x2F5DA0, the
  reader's cursor at +4) through `engine_026g.tsv` rows if they are not there.
- **Size:** about 150 to 250 lines of Rust (patch 92 is 300 with more logic) and a TOML of six
  hooks; 1 to 2 KiB of arena code, 2.5 KiB of state. A Unicorn check like
  `check_battery_progress_026g.py`: the three counting hooks on made-up objects, and the game's own
  serialize and deserialize with the save and load inside.
- **Server:** `snapshot.rs` reads the block (the exact-fit walk ends after it; `StatisticsTail::of`
  skips it by its size) and refuses a world whose sums differ from the kit totals; the result
  carries per place `(dict_id, kills, shots, hits)`; dict id to weapon name from the game data
  (`server-game-data.db`), never a table in code.
- **Who must have it:** the world's bytes and hash change, so every game of a match must run the
  same revision (the server asks for it, `SRP_MIN_PATCH_REVISION`), as with patch 92. What a match
  plays out does not change: LIKELY no `simulation_changes.tsv` row (replays of older games load
  their worlds with an empty table); the reviewer and the owner to confirm.
- **Smaller still** (if accuracy per weapon is enough without the dict id): count per
  (place, kit, slot 7 or 10) by the same three hooks; but a picked-up gun is then the picker's
  slot's item, so the dict id table is the one that is right for every kill.
