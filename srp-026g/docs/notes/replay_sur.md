# The 0.26g client's own replay file (`.sur`), and a converter from match recordings

**Version scope:** client **0.26g** (build 2777, patch revisions 7 to 10 on the players' server). Every address is a VA in the original 0.26g exe, read with `tools/re-kit/common/scripts/dis.py 026g` and `xref.py`. Written 2026-10-15.

**Status: the format was read from the exe; no file made here has been played by a client yet.** The server makes these files itself for the lobby's "Watch replay" (`replay_watch.md`). The code (`servers/match-server/src/v026g/sur.rs`, `sur_convert.rs`, the binary `srp-recording-to-sur`) makes files that pass their own checks and parse back with a reader written from the client's. Whether the game plays them is the first thing to find out (section 6).

Tags: **PROVEN** = read in the 0.26g code at the address given. **LIKELY** = strong inference. **GUESS** = weak.

**Checked twice.** On 2026-10-15 the claims below were read again in the exe by two readers who had not seen this note's first version; what they corrected is in the text (8 MiB is a cut, not an overrun; chunk 4 PROVEN; the key frames' grid; the input flag bit; the start record's type; the cost of the type 0 records).

## 1. What the client has

The 0.26g exe has a replay player the 0.21d exe only began (0.21d: `replay_match_reader::open_replay` with three fixed header reads, no key frames). PROVEN (strings and code):

- the command-line key `-match_replay=<name>` (description "replay match"; key object 0xA9C9B0, read with `is_set_as_string` in `game::on_base_resources_created` at 0x1EB099, after `-spectator`);
- a file reader, `replay_match_reader` (0x694 bytes, constructor 0x1F03D0), with **key frames** (`key_frame_type`, at most 128), seeking (`game::enqueue_replay_seek_to` 0x1EE3C0 -> 0x1EE2E0) and the check "Replay of other engine version is used" (0x1F067A);
- a replay UI, `resources/flash_movies/replay_player.swf`: play/pause, speed 0.01 to 10, a seek bar over the match's length, one-frame steps, a player list (`select_player(id, 0|1)`, first or third person), a free view, `smooth_replay_camera`, `toggle_replay_ui`;
- no lobby list of replays, and **no writer**: the strings "replays" and ".sur" each have one reader, nothing in the exe opens a `.sur` for writing, and `key_frame_type::push_back` is called only by the reader (LIKELY: no writer path found). The game's own recorder is the journal (`-record_journal`), which is off since patch revision 4 and is another thing (it holds the packets one client got and its device input, and plays only with `-replay_journal`).

## 2. Where the file is

PROVEN (0x1F02C0, 0x1F0FA0, 0x1F1020):

- `<user data dir>\replays\<name>.sur` (`replays` 0x9FAEC0, `.sur` 0x9FAEC8); the user data dir is `Documents\Survarium` (LIKELY: it comes from a virtual call not resolved; it is the engine's `get_user_data_directory` as in 0.21d, where the journal and the logs are);
- if `<name>` starts with `\\` or has a `:` before any `\` (0x41050) it is a path and is used **as given**, without `.sur` added (0x1F0307..0x1F0311).

The file is read whole into a resource of its size + 0x1800000 (0x1EB165..0x1EB3AD, "unmanaged_replay_match_reader_resources_allocation"): the file, a 16 MiB LZMA work area, the 8 MiB decode area (0x1F0419..0x1F052B). A missing file is not reported: its size is 0 and the open is skipped (0x1F05C3).

## 3. The file

```text
file  = chunk*                        any order; ids 0..=4 must each be there
chunk = u32 id, u32 word, size bytes  size = word & 0x3FFFFFFF; the top 2 bits are read and ignored
```

PROVEN (0xF1490 / 0xF1540): the reader scans from the start for the id and skips other chunks by their size; an unknown id is skipped, of two with one id the first wins, and the top 2 bits go to an out parameter no caller reads (0xF14D7). A missing chunk is not checked for (the search gives -1 and the reader takes a "size" from the file's 4th byte: garbage), nor that a chunk fits in the file.

| id | content | tag |
|---|---|---|
| 0 | raw: a NUL-terminated string (skipped; an empty one is fine), the string `0.26g0` (`strcmp` with the exe's own, 0x9DAE98), `u32 8` right after it, unaligned (the format version, kept at reader+0x63C and handed to the world's `deserialize` for every key frame). Another string or version stops the game: `debug::terminate` (0x113580) "Replay of other engine version is used" | PROVEN (0x1F060D..0x1F068D) |
| 1 | LZMA2: the match options, **a 0xE4 payload without its first byte** (the local player index): `static_info.md` | PROVEN (0x1F08E0 -> `match_options::deserialize` 0x1F0E70, the function `process_static_match_info` uses) |
| 2 | LZMA2: the body, records, section 4 | PROVEN (0x1F069C..0x1F0735) |
| 3 | `u16 count`, then LZMA2 of `count` entries `u16 len, u32 body_offset, len bytes`: the key frames, **in time order** (the lookup is a binary search on each entry's first u32, 0x1EEF90); the vector holds 128, the 129th hits a "buffer overflow" assertion (0x1F0DF0; whether it is fatal in this build was not traced) | PROVEN (0x1F0AC0, 0x1F0BA4..0x1F0BBA) |
| 4 | LZMA2: `u32 len`, then `len` bytes of a Vostok binary config, the mode's `gameplay/game_modes/<config>.options` file **as it is in `resources.db`**; the game copies it and relocates it in place (0xF6D00) as its resource loader does with any converted config (0x10AE10, around 0x10AEB8), then asks it for `mode` -> `match_making_params` -> `map_name` | PROVEN (0x1F0980, 0xF6D00, 0x218F08..0x218F3C). The blob has no header: the root value is at 0, a value is 0x18 bytes (+0 data or children, +8 name, +0x10 CRC32 of the name, +0x14 u16 type, +0x16 u16 count); an extracted `level_02_day_4.options` starts `18 00 00 00 ... 03 00 01 00`. The lookup (0xF6B10 -> 0xF6D50) wants the children sorted by name CRC and has no end check, so the key must be there and the file must not be re-made by hand |

**LZMA2.** Every chunk is a raw LZMA2 stream with no properties byte, decoded by the same wrapper and function as the 0xE4 options and the 0xE8 world (0x357EB0; the property byte is made from the set dictionary, 0x100000). It is the SDK's one-call `Lzma2Decode` (0x3EE810), which uses the output as its dictionary, so the dictionary size only has to be in range. The result is not checked (0x357EFC). Stored chunks do (control bytes 1 and 2, 0x3EE42E; `lzma2::stored`), and so do compressed ones (`lzma2::packed`, as the worlds of 0xE8 go to the clients). PROVEN.

**Memory.** All decoded chunks (in the order 2, 3 in the constructor, then 1 and 4 at 0x218DDA / 0x218DE5) go into one 8 MiB area, each decode given what is left (+0x684). **Nothing checks the total: past 8 MiB a chunk is cut short without a word** (not an overrun). The writer refuses that and more than 128 key frames ([`sur::write`]).

**Key frame.** `data` is `u32 time_ms` and then the body of a 0xE8, which is the body of an upload (`client_patch.md` 3.4): `u32 active_clients_mask`, the player blocks, the six sections. It goes to `game_world_core::deserialize` 0x2D0B70 with the version of chunk 0 (8, 0x1EE353), as 0xE8 goes with the constant 8 (0x27C0B5); the world takes the first u32 as its time (0x324D98) - PROVEN. `body_offset` is where in the decoded body the reading goes on after it (0x1EE31E..0x1EE347). The player finds the last key frame with a time at or before the time asked (0x1E8EC0; 0 if none is, -1 if there are none).

## 4. The body

```text
u8 type, u32 time_ms, then:
  0, 4         nothing more
  5..=20       f = type - 5
               f & 8: u32 mask, a u8 for each set bit (0..20)          0xF3 kit choices
               f & 4: u32 mask                                         entered
               f & 2: u32 mask                                         left
               f & 1: u32 digits, then for each non-zero digit a u32 (1) or a u64 (2)
```

PROVEN, the reader 0x1F0BE0. Types 1 to 3 read as f = 0xFC..0xFE (garbage fields); a type above 20 reads as f & 0xF. The kit choices are read for bits 0..19 only (0x1F0C4A): a higher bit stays in the mask and reads no byte. The digits are 20 base-3 numerals, player 0 the most significant (checked against the client's two loops, a test in `sur.rs`). A u64 is `{u32 rotation = two half floats (radians), u32 actions mask}`: the world player holds the rotation at +0x32C and the actions at +0x330 (0x2D6C63..0x2D6C77), the slot array is initialised from each player's current actions mask in its high half (0x1EDFD4) and a zero rotation in its low half, and a u32 only writes the low half. 0xEB/0xEC (0x318400) and 0xED (0x27C4F4..0x27C565) fill the same slots in the same order, so a relay's 8 bytes go in as they are - PROVEN - **except bit 31 of the actions**: both parsers split it off as a per-input flag (0x318479, 0x27C501), and the replay hands the u64 on with that flag 0 (0x1EE2A1); left in, it would be an action. The converter clears it (0 inputs in the 20 samples had it). The writer always writes u64.

**Playing** (0x1EDEF0, called with t, the game clock of the frame, which `game_world::tick` also moves the world to): `target = (t / 50 + 1) * 50` (0x1EDEFC). While the reader's cursor (the time of the last record read) is below `target` (strictly): the world is moved to the cursor (`game_world_core::move` 0x2D2E40, which simulates: every 50 ms boundary is run and saved, the inputs at a time first), the next record is read, the cursor becomes its time, and then the record is **applied at once, whatever its time**: `enter_match(player, time)` 0x2D1470, `leave_match` 0x2D18A0, the kit choices (`change_input<profile_input>` 0x1FB730), the inputs (`change_input<player_input>` 0x1FAB50; the "precise" flag is `cursor % 50 != 0`, 0x1EE27F), in this order. A type 0 record ends the loop after the cursor took its time (0x1EE035, 0x1EE03D); type 4 does nothing and the loop goes on; at the body's end the cursor becomes the end time (0x1EE018). PROVEN (0x1EDEF0..0x1EE2DE).

Every apply carries its time, and the replay skips the "too far in the future" check (the flag is 0 at 0x1EE290; the check 0x1FAFB6), so a record read early does no harm. The writer ends every tick's records with a **type 0 record timed at the start of the next tick that has records** (and the file with one), so the cursor stays at that tick until the frame reaches it (`sur.rs`, the test `a_tick_is_closed_before_the_next_ticks_records`). That is correct but costs: the next frame first moves the world to that tick, saving its boundary, and only then reads its inputs, so an input at a tick's start is marked at the boundary before (`insert_impl` 0x2D1100, 0x2D12D0..0x2D1328) and the world rolls back one tick (0x2D2E83) - for nearly every tick (about 18 000 a match). Keeping only the last type 0 would let those records be read a tick early, harmlessly. Left as it is until a first file has been played: then it is one change to try.

**Start and end.** The start is the time of the body's first record: `on_match_replay_ready` (0x218FC0) reads it (0x2192F2) and uses only its time, which `set_time_in_ms` (0x1EE4A0) puts on the game clock (+0x5854) and the cursor (+0x57F0); its contents are dropped, so any type would do, and the writer puts a type 4. The end is the time of the last record, found by parsing from the **last** key frame's `body_offset` to the body's end (0x1F0D20, called at 0x1F0779), so the last record is a type 0 at the end. The world is built fresh from chunks 1 and 4 (`game_world::load` 0x1F8CA0, `serialize` 0x2D04F0); a fresh world core is at 50 ms (0x2D0765..0x2D0786; that this reset runs on the replay path was not traced), so the start should be 50 (it is, in every sample), and an input at exactly the start would never be applied (`move` takes only what is later than a saved state, 0x27E870; none in the samples).

**Key frames while playing.** A seek fires when `(t / 50 + 1) * 50` equals the next key frame's time (0x218C47..0x218C73), so **a key frame's time must be on the 50 ms grid** (one that is not never fires, and holds back every later one; every item time is on it). It calls `enqueue_replay_seek_to(time - start, true)`: the key frame is deserialized (mode 2), the body reading goes on from its `body_offset` (0x1EE347), and the cursor and the clock are set to **the key frame's time** K (`set_time_in_ms` 0x1EE382), not to the time of the record at the offset (0x218BF0, 0x1EE2E0). The world's last-resync time becomes K (0x2D0FD7) and `change_input` refuses any input at or before it ("older than last resync", 0x1FABBC); an input at K is already in the saved state (a rollback goes to a state strictly older than the input, 0x2D1130). So `body_offset` = the first record **after** K is right - PROVEN. The sim is re-synchronised at every key frame: a replay that differs from the match in between is put right there. Index 1 is the first key frame watched (0x218DF3); key frame 0 serves any seek to before key frame 1. A seek more than 10 s from the clock calls 0x219420 (not read).

## 5. The converter

`srp-recording-to-sur <file.srprec> --game-modes <dir> [--out <dir>] [--stored] [--every <ms>]` (`cargo run -p match-server --bin srp-recording-to-sur`). `<dir>` holds the game's `gameplay/game_modes/*.options` files under their own names (`tools/re-kit/common/dbgrab get gameplay/game_modes/ <dir>` writes them as `gameplay__game_modes__<name>`; strip the prefix); the one the recording's `map_id` names in `maps_dict` is used. It prints a summary and writes `<name>.sur`.

| `.sur` part | made from |
|---|---|
| chunk 1 | the stored LZMA2 stream of the 0xE4 of the place that got the **largest roster** (with late joins, the roster grows: 6 to 20 seen), without its first byte |
| chunk 4 | the mode config file |
| body | the relays the server sent to the places: 0xEB, 0xEC, 0xED, 0xEE (inputs), 0xF3 (kit choice), 0xE9, 0xEA (enter, leave), every message of every place put together and a copy of one dropped (one input at one time of one player is one input: 1.5 to 4.3 million copies in a match, **0 conflicts**) |
| key frames | the uploaded worlds (0xE0 chunks of patched clients, `Reassembler`), one body for each item time (the most common one when clients disagree: 1 to 2 times in a match), the first world, one at least every 10 s, and **one soon after each late joiner enters**, at most 128 |

Choices, each one in the summary:

- **The match clock** is the recording's time less a constant, the median over the notes that carry both (`entered`, `clock`: 1100 to 2900 ms); its use is only to read the relays' one-byte ticks (`tick_time`: the tick within ±127 of the clock's, so ±6.35 s). An event the clock puts more than 1 s ahead of the recording is dropped.
- **A place whose link is backed up** gets its copies late: match 0x57b21c79 (14 Oct 13:36) sent place 2 its relays up to 30 s late from 340 s to 680 s, through its resyncs. A copy more than 6.35 s late reads as another tick: the first converter put about 2 200 inputs 12.8 s late there, and counted 38 965 conflicting copies and 249 604 "future" ones (its summary claimed none). Now a copy no other place got within 2 s before it (or 1 s after), but another place got within 60 s before, is a sign of a backed-up link, and **every copy that place gets within 15 s of a sign is dropped**: the server sends an event to every other place at once, so the others had it on time. In that match 799 085 copies of place 2 are dropped, no message is left without a copy, and there are 0 conflicts and 0 "future"; the other 19 come out as before (no copy dropped). A match with one recipient per event (2 places) has nothing to compare with, and a late copy there would still be read as another tick.
- **Masks.** A match that lets players in late (`growing_roster`, every sample) sizes the 0xED/0xEE masks for 20 from the start (`Game::snapshot_players`); the server sends no 0xED/0xEE today, but the converter reads them so.
- **Inputs, not the players' own packets.** The relays hold what the server accepted (late inputs moved, refused ones dropped, `game.rs::input`); each input is relayed to every place but its owner's, so the union over the places is all of them. A match with one place only would lose its inputs.
- **A 0xEA to the leaver itself** is its farewell (tick 0 for a game that never loaded), not a leave: ignored.
- **The start** is the first uploaded world's time (50 ms in every sample); the first record is the marker.

**Samples.** All 20 recordings made 2026-10-14/15 convert and verify, and an independent reader (Python, liblzma at the client's 1 MiB dictionary, written from this note) reads every file back: the five chunks, every record, every tick closed, every key frame's offset right both ways (`sur::verify`: the five chunks once, the version chunk, a body that parses with a marker first and a type 0 last, key frames in order that point at record boundaries, nothing after a key frame's time skipped, under 8 MiB). Sizes (`--stored` is about 2.9 times larger than packed: 2.2 MB for the first match below):

| match | roster | recording | `.sur` packed | key frames | inputs |
|---|---|---|---|---|---|
| level_07_evn_4, mode 4 | 10 to 13 | 8.1 MB | 0.77 MB | 95 | 97 709 |
| level_02_4, mode 4 | 11 to 20 | 13.5 MB | 1.17 MB | 96 | 148 801 |
| level_05_evn_2, mode 2 | 6 to 10 | 10.0 MB | 0.80 MB | 94 | 101 610 |
| level_07_4, mode 4 | 2 to 7 | 2.0 MB | 0.24 MB | 94 | 28 399 |

Overall a `.sur` is about **a tenth** of the recording (0.24 to 1.2 MB against 2 to 20 MB), because a recording holds every datagram once for each place and a `.sur` holds each event once. The body is 1.0 to 1.5 MB decoded (about 10 bytes for each of 100 to 150 thousand inputs), the decoded total 2.2 to 3.4 MB (the client's limit is 8 MiB).

## 6. Not proven, and the first run

Not proven (each would show on a first run):

1. **That the client plays a file of this layout at all**: `-match_replay` working without the launcher or a sign-in, the replay needing nothing else in the folder. (Chunk 4 as the raw `.options` file and a free first string of chunk 0 are now PROVEN.)
2. **The patch revision of the viewer must be the revision of the recording's clients** (LIKELY): the world is a key frame's `deserialize`, and the patch revisions change what the world holds (revision 9 raises the statistics' player count to 20, `client_patch.md` 3.16; revision 10 the weapons' random state). The 10 recordings of 2026-10-14 09:04 to 13:36 UTC were made by games of revision 7 or 8 (the minimum was 7, revision 8 released 03:48 UTC), the 10 of 21:29 UTC on and after by games of revision 10 (the minimum was 10 from 21:26 UTC). Use a recording of the revision the exe is.
3. **Late joiners**: the client's late join (the exe's `lj_native_install`) does not run in a replay, and `enter_match` for a place beyond the first roster has not been seen to work. Every sample has them (the recorder keeps a match once 6 places have entered, and the server fills a match). The key frames soon after each entry carry the newcomer's player in the world, so it should appear at the latest then.
4. **The start**: the fresh world is built from the options and the config; the first seconds differ from the match if it does not agree with the first world (50 ms) until the first key frame (about 10 s).
5. **The record rules** of section 4 in a running game: the type 0 at every tick's end (correct by the code, with a one-tick rollback per tick), and the seek to the first record after a key frame's time (PROVEN by the code).
6. **Statistics of the replay** (the UI's score screen): the replay restarts the match's score; a replay does not run the server's rules.
7. A recording made with `SRP_RECORD_MIN_PLAYERS` below 2 places has no relays to take inputs from.

**The first run** (not done: the owner's decision to run a game):

1. Make the file: `srp-recording-to-sur <recording of the exe's revision> --game-modes <dir> --out <dir>` (the stored variant too, `--stored`, to exclude the encoder).
2. Put it in `Documents\Survarium\replays\<name>.sur` (make the folder).
3. Start the exe of the same revision from its own folder (`game\binaries\x86`), as the launcher does, with `-match_replay=<name>` added: `survarium_skhid.exe -localization=english -mt_allocator=768 -minimum_resources_memory_size=768 -match_replay=<name>`. If the name is not found, the full path `-match_replay=<drive>:\...\<name>.sur` is the other form.
4. Look at: a crash (the dump is beside the exe), "Replay of other engine version is used" (chunk 0), the map loading, the replay UI, whether the players move and whether the clock and the key frames keep the world together (a world that drifts is put right every 10 s).

## 7. Key frames before a late joiner's install (found 2026-10-15, fixed in the converter)

The viewer binds every place of chunk 1 at its start, so its list of game objects holds each late joiner's grenades and traps from the first tick (the revision 7 order: newest slot first, then the map's objects; `client_patch.md` 3.14). The recorded games got them only at the install, as one zero byte per grenade or trap put in front of the saved objects. A key frame from before an install therefore had fewer object bytes than the viewer has objects, and every object read out of step (PROVEN in the file: 0x3cf24270's key frames before 554 750 ms lack place 10's 4 bytes): a grenade read `ff` and took a NaN transform (LIKELY the black terrain and vanishing buildings, and the second crash, a render-thread `loose_oct_tree::insert` recursion), and a usable object read a user count over its 20 (`buffer_vector<u8>::push_back` "buffer overflow", 0x2F84C0 -> 0xF3090, the first crash). Details: `private/replay-format-026g/RE-REPLAY-2.md` (owner's PC).

**Fix:** the converter puts, in front of the objects of every key frame from before a late place's install, one zero byte per grenade and trap of that place (`find_installs`, `pad_objects`).
- **How many** (`kit_objects`): counted from the place's profile in the largest roster's 0xE4, which is chunk 1 and what the viewer binds: the kit slot's count of each grenade stack and each trap stack (items of `type` `grenade` or `trap`), summed over every kit, as the client's binds add them (the grenade item's bind 0x309EC0 one object per grenade, the trap item's 0x3067A0 one per trap; `client_patch.md` 3.14). Not the item's stack number: in the samples the stacks hold 10 to 30, the slots 1 or 2. A place's profile never changes (appended places are never reused), and a re-entry of the same place adds no bytes (0x57b21c79 place 7).
- **When**: the first world whose objects are the world before with that many zero bytes in front, between the first 0xE4 that had the place (less 1 s) and the first world after its entry. The rest need not be the same: a grenade thrown or a door's state in the same 5 s changes later bytes, so at least 8 bytes past the zeros (or the whole of the world before) agree with the world before, and more than without them. Two places installed in one step share it with the sum of their objects.
- **Checked against the worlds**: every exact step (the rest unchanged) must be explained by a place's count. In the 20 samples of 2026-10-14/15 there are 14 late places with grenades or traps, each with exactly one step of its count, 6 of them exact (0x0d881ba4 place 7 = 1, 0x57b21c79 place 7 = 1, 0x3753f263 place 5 = 1, 0x3cf24270 place 10 = 4, 0x0f46e040 place 3 = 2, 0x1011937a place 7 = 5 at 294 700 ms), and no step left over.
- **When unsure** (no step of the count or more than one, a step back to a world without the bytes, an exact step no count explains, or 0xE4s that disagree on the count): the key frames from the place's first 0xE4 to its entry are left out; before that they get the bytes, after the entry they are as the games had them. None in the 20 samples (before, 24 to 79 key frames were left out in 6 of them).

**Run 2026-10-15 15:27** (0x3cf24270, revision 10 exe): 22 key frames loaded past the old crash points (74.6 s, 114.6 s), the map whole in every screenshot, the kill feed shown. (That run used the earlier detection from the worlds alone; the count from the kits gives the same bytes for that match.)

**Watching** (RE-REPLAY-2.md, PROVEN in the code, not all tried): F11 shows the replay UI (player list, first/third person, seek bar, speed); F1 first person on the last target, F3 third person, F2 free view, F6/F5 next/previous player (only while attached); the console (`~`) has `attach_to_player <place>`, `detach_from_player`, `pause`, `resume`, `time_scale <0..10>`; Pause/Break pauses. Seeking is only the UI's slider: it loads the last key frame at or before the target and plays on from it.

## 8. Two crashes of the viewer, fixed in the client (2026-10-15, patch revision 11)

`client_patch.md` 3.24 has the fixes (patches 24 to 26 in `client-patches/README.md`).
- **First or third person, at the watched player's first respawn into another kit.** The camera stayed on the old kit's object, and `weapon::draw` died. Revision 11 re-attaches it. Run: followed past such a kit change.
- **About 50 s after a seek, at normal speed.** A weapon's delayed-sound queue (64 items) overflowed. Revision 11 drops the new sound when it is full.
- **Late sounds after a seek** (reloads more than footsteps; LIKELY what was heard). The queued sounds kept their times from before the seek, and new copies waited behind them. Revision 11 empties every weapon's queue at a user seek, in the replay only.

Run 2026-10-15 (revision 11 test exe, 0x3cf24270, first person, seeks back and forth): no crash, sounds on time after the seeks.
