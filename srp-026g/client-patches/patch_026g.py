"""Builds the srp copy of the 0.26g client (build 2777) from the original
survarium.exe. The full patch includes the startup VRAM-check bypass used
by the Linux launcher.

USAGE (this script never touches the source exe):

    cd srp-026g/client-patches
    python patch_026g.py <game>/binaries/x86/survarium.exe <game>/binaries/x86/survarium_srp.exe
        full patch, upload code in a new PE section (the default)
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --cave=hash-mismatch
        full patch, upload code over game_world_core::on_hash_mismatch
        (the fallback if a client with one more section misbehaves)
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --cert-only
        only the certificate patch (what step 0 installed)
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --no-late-join
        the full patch without the late join: patch revision 4, the exe of
        the players' download before revision 5
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --server=<host>:<port>
        full patch with an embedded default login server; -server= at
        game launch still overrides it, and -replay keeps its original path
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --revision=<n>
        the default build saying patch revision <n> instead of REVISION:
        a release's (release/build-zip passes it). Only the revision byte
        and the PE checksum differ; <n> is REVISION to 255. Not with
        --no-late-join, --cave=hash-mismatch or --cert-only
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --required-patches=<n>,<n>,...
        the default build that stops at its start when one of these
        resources.db.patch_<n> is not there (patch 49): a release's, the
        patches of its offered-patch list (release/build-zip passes them).
        Only the bitmap at the end of the patches' mount and the PE
        checksum differ; each <n> is 1 to 255. Not with --no-late-join,
        --cave=hash-mismatch or --cert-only
    python patch_026g.py <survarium.exe> <survarium_srp.exe> --ragdoll-trace
        the default build with patch 76's diagnostics on: every living hit
        and every death's ragdoll start as a line of srp_ragdoll.csv in the
        user data folder. A local build for us (never a release's:
        release/build-zip does not pass it); only the Rust crate's
        SRP_DIAGNOSTICS and the PE checksum differ
    python patch_026g.py <survarium.exe> --check [--cave=...] [--cert-only]
        every check and patch in memory, nothing written
    python selftest_026g.py
        builds every variant, re-parses and disassembles it, writes the
        results to SRP_026G_SCRATCH (never the game folder)

    <survarium.exe> must be the ORIGINAL 0.26g exe (the sha256 is checked);
    the output must be another file. Close the game before replacing an exe
    that is in use. The upload code comes from snapshot_upload_026g.asm:
        nasm -f bin -DCAVE=0xA425000 snapshot_upload_026g.asm -o snapshot_upload_026g.section.bin
        nasm -f bin -DCAVE=0x325A60  snapshot_upload_026g.asm -o snapshot_upload_026g.hash-mismatch.bin
    (both .bin files are checked in; --bin=<file> names another one).
    The stale shell_32.dll check comes from stale_dll_026g.asm:
        nasm -f bin -DCAVE=0xA425E00 stale_dll_026g.asm -o stale_dll_026g.section.bin
    (checked in). The mount of resources.db's patches comes from
    mount_patches_026g.asm:
        nasm -f bin -DCAVE=0xA425400 -DSTATE=0xA428800 mount_patches_026g.asm -o mount_patches_026g.section.bin
    (checked in; STATE is 0x800 of the late join's state page, checked).
    The forward effect's UV scrolling comes from forward_scroll_026g.asm:
        nasm -f bin -DCAVE=0xA425B00 forward_scroll_026g.asm -o forward_scroll_026g.section.bin
    (checked in). "Watch replay"'s link code comes from replay_link_026g.asm:
        nasm -f bin -DCAVE=0xA425220 replay_link_026g.asm -o replay_link_026g.section.bin
    (checked in). One upgrade on several items comes from shared_upgrades_026g.asm:
        nasm -f bin -DCAVE=0xA425700 shared_upgrades_026g.asm -o shared_upgrades_026g.section.bin
    (checked in). A kit chosen before the start comes from kit_change_026g.asm:
        nasm -f bin -DCAVE=0xA425EC0 kit_change_026g.asm -o kit_change_026g.section.bin
    (checked in). The archive reads' order comes from read_order_026g.asm, which
    replaces hdd_manager_sorting_predicate's body in place:
        nasm -f bin read_order_026g.asm -o read_order_026g.bin
    (checked in). A weapon's upgrade window showing the weapon comes from
    weapon_preview_026g.asm:
        nasm -f bin -DCAVE=0xA427E50 -DSTATE=0xA428E00 -DFIND=0xA42C00F -DCAMERA=0xA42A6B0 weapon_preview_026g.asm -o weapon_preview_026g.section.bin
    (checked in; STATE is 0xE00 of the late join's state page, checked; FIND is
    upgrade_preview_026g.asm in `.srpw`, which shows an upgrade dropped on the
    window's weapon at once and calls weapon_stash_026g.asm's find first:
        nasm -f bin -DCAVE=0xA42C000 -DSTATE=0xA428D60 -DPREVIEW=0xA428E00 -DFIND=0xA42A36F upgrade_preview_026g.asm -o upgrade_preview_026g.section.bin
    checked in; its STATE is 0xD60 of the late join's state page, checked;
    weapon_stash_026g.asm also shows a weapon no kit holds and armour the
    chosen kit does not wear:
        nasm -f bin -DCAVE=0xA42A360 -DSTATE=0xA428E00 -DPREVIEW=0xA427E50 weapon_stash_026g.asm -o weapon_stash_026g.section.bin
    checked in; CAMERA is weapon_table_026g.asm in `.srpw`, which shows the
    weapon in the air above the lobby's workbench:
        nasm -f bin -DCAVE=0xA42A6B0 -DSTATE=0xA428F00 -DPREVIEW=0xA428E00 weapon_table_026g.asm -o weapon_table_026g.section.bin
    checked in; its STATE is 0xF00 of the late join's state page, checked), and its
    draw from weapon_draw_026g.asm:
        nasm -f bin -DCAVE=0xA425D20 -DSTATE=0xA428E00 weapon_draw_026g.asm -o weapon_draw_026g.section.bin
    (checked in). The warmup camera turned by a drag and moved by the wheel while
    the kits are shown before the start comes from warmup_look_026g.asm and
    warmup_limits_026g.asm:
        nasm -f bin -DCAVE=0xA425D40 -DSTATE=0xA428E20 -DLIMITS=0xA425A98 warmup_look_026g.asm -o warmup_look_026g.section.bin
        nasm -f bin -DCAVE=0xA425A98 warmup_limits_026g.asm -o warmup_limits_026g.section.bin
    (checked in; STATE is 0xE20 of the late join's state page, checked)
    (checked in). The commit_suicide key is the Rust patch suicide_key, in
    the arena's block patches.code (rust/patches, hooks_026g.py, link_026g.py;
    docs/notes/rust_patches.md). A spot's hit on a battery's body comes from
    spot_026g.asm:
        nasm -f bin -DCAVE=0xA4257C0 spot_026g.asm -o spot_026g.section.bin
    (checked in). The camera on the death screen comes from death_camera_026g.asm,
    a section of its own, `.srpc`, on the page after the late join's state page:
        nasm -f bin -DCAVE=0xA429000 -DSTATE=0xA428E60 -DLOOK=0xA425D40 -DLOOK_STATE=0xA428E20 -DDRAG_END=0xA425A98 death_camera_026g.asm -o death_camera_026g.section.bin
    (checked in; STATE is 0xE60 of the late join's state page, LOOK_STATE warmup_look's, checked).
    A weapon's camouflage comes from weapon_camo_026g.asm, in a section of its
    own, `.srpw`, on the page after `.srpc`, and what it paints (the camouflage,
    the decals) and the decal editor on a weapon from weapon_decals_026g.asm, at
    0x1200 of that section (after patch 45's code):
        nasm -f bin -DCAVE=0xA42A000 -DDECALS=0xA42B200 weapon_camo_026g.asm -o weapon_camo_026g.section.bin
        nasm -f bin -DCAVE=0xA42B200 -DPREVIEW=0xA428E00 -DTABLE=0xA428F00 -DPRESS=0xA428FC0 weapon_decals_026g.asm -o weapon_decals_026g.section.bin
    (checked in; PREVIEW and TABLE are patch 32's and patch 45's states, PRESS
    its own, 0xFC0 of the late join's state page, checked).
    The arena, `.srpx` (Research, the artefacts' code, the engine-function
    library, the kit keys and the match chat option, the hide-the-interface
    key), is linked from its C and
    NASM sources at every build
    (link_026g.py: gcc -m32, GNU ld and NASM 2.16 are needed) and must be
    arena_026g.lock.json's (`python link_026g.py --write` after a change).
    The late-join code is late_join_026g.section.bin, checked in too; it is
    made by `late_join_026g.py --write-bin`, which needs nasm, a C compiler
    for x86 and GNU ld (--late-join-bin=<file> names another one).

Port of ../srp-021d/client-patches/patch.py; the evidence for every address
is in docs/notes/client_patch.md.

Patches:
- the code-signing certificate check (0x12A10 -> `xor eax,eax; ret`). At
  start the client takes the survarium.com certificate from its own
  Authenticode signature, asks Windows whether its chain is valid now, and
  only then verifies the resources.db file table signature with its key.
  The certificate expired on 2015-06-06, so the chain is "not time valid"
  and every stock 0.26g client stops with "Resources archive is
  corrupted". The patch skips only the chain check; the file table
  signature is still verified (except in the default build since patch
  revision 12, below). verify_sign takes the certificate out of the
  signature blob before either check and fails without it, so the blob
  must stay readable in every build (see --cave=section below);
- the single-instance check (0x124F9 `jz fail` -> two nops), so several
  clients run on one PC;
- the minimum VRAM check (0x106411 `jae` -> `jmp`), skipping the fatal
  warning when detected video memory is below 320 MiB. The detected size
  and the original fallback for an unknown size are kept;
- optionally, a default login server (--server=host:port at patch time).
  The missing-server error at 0x1EB25B becomes a jump into a 51-byte stub
  appended to the upload code. It initializes the parsed server key and
  repeats the original option read and connection path. Explicit server/replay options
  take their original paths. The lobby address still comes from login;
- the default of -hash_check_interval_in_ms, 100 -> 50 (the imm32 of
  `mov dword [esp+18h],64h` at 0x27CC4F, one byte at 0x27CC53). A stock
  client reports its world hash (0xDF) for every second 50 ms tick, on a
  phase of its own; two clients on opposite phases never name the same
  tick, and the match server then cannot compare their worlds (match.md
  section 14). With 50 every client reports every tick. The key on the
  command line still overrides it;
- the patch revision in the sign-in (sign_in_on_connected 0x3807D0). The
  first login message has a 12-byte version field: "0.26g0" and six zero
  bytes. `mov cl,[9DAE9Eh]` at 0x380862 (it reads the string's own zero,
  which 0x38087B writes into the field once more) becomes
  `mov byte [esp+27h],REVISION` / `nop`: byte 7 of the field says which
  of these patches the exe has. The text stays "0.26g0", so a server that
  does not read the byte is not affected, and an exe without this patch
  says 0. REVISION is the revision of the last release: a release builds
  with --revision=<the live exe's + 1> and then records that number here
  itself (release/release.sh); a change that makes a full build something
  else for the game never raises it. A server can only tell two clients
  apart by it, so a server rule for a game with such a change asks for the
  next release's revision, REVISION + 1;
- the battery rule of modes 2 and 3 (gather_victory_items_rule_core's
  container event handler 0x313A50): two instructions become nops. When a
  team's count reaches the number of batteries the handler stores the
  completion time (+0x15C) and clears the "containers can be used" byte
  (+0x160); from then on it counts no event (`jnz` at 0x313A62) and no
  container can be used (0x31CB16). Neither field is in the rule's
  serialize/deserialize, so a rollback takes the tenth battery out of the
  container again and leaves both as they were: the replay of the same
  tick cannot put it in, and nobody can use a container for the rest of
  the match (9 of 9 live matches in which a team had nine; match.md
  section 26). Without the two instructions every event is counted and
  the containers stay usable, so the replay ends as the first run did.
  The match's end is the server's (it reads the points);
- a start with a Documents folder the game may not write to (two sites,
  eight bytes, client_patch.md section 3.10). The user folder is the
  folder Survarium in Documents, mounted as `user_data`. When it does not exist
  and cannot be made, the mount fails and has no root; mounter::finish
  0x469F0 still called the "mounted" callback, resources_manager::on_mounted
  0xF9A80, with a null node (access violation at 0xF9A8F). `xor ecx,ecx` /
  `jmp short 46B12h` at 0x46B0B becomes `jmp short 46B36h` / two nops: a
  mount without a root is not announced. When the folder exists but a
  file cannot be made in it, the journal `Survarium_<user>.journal`, made
  at every start with create_always, stopped the game with a fatal
  assertion (0x40141). The default of -record_journal, the text "1" given
  to the option object 0xA9D628 by its initializer at 0x851CC7
  (`mov eax,9DB51Ch`), becomes the exe's own "0" at 0x9DBF0C: no journal
  is made unless -record_journal=1 asks for one. game_world::on_activate
  gave the hud its player's name only while the journal recorded: 0x1F9478
  `test eax,eax` becomes `jmp short 1F9487h` (patch 43, TAB_SELF), so the
  Tab list's own row is gold again;
- the lobby's `stage.focus` with a focused object whose ActionScript
  object is gone (patch 52, every full build): Scaleform's Stage.focus
  getter (0x771FA0) read that object's traits through a null pointer
  (access violation at 0x77200B, a key pressed in the lobby, 2026-10-22).
  The 46 bytes at 0x771FED test the pointer as FocusManager.getFocus does
  and read null instead (STAGE_FOCUS);
- a held weapon shown while its player aims (patch 91, every full build):
  weapon::on_show (0x243DD0) took its aimed branch and never added the
  weapon's body, which a red dot's release of the aim does not add back
  (after a resync: only the sight and the hands drawn). `jz near` at
  0x243E32 becomes `jmp`: the idle sight and the body always, the next
  draw aims (WEAPON_SHOW);
- the late join (patch revision 5; the default build only, not
  --cave=hash-mismatch): a player enters a match that is being played.
  The game keeps room for twenty players from the first connection (three
  bytes of input masks, twenty histories and statistics records), says so
  in its match connection (0xD8), and on the server's 0xE4 loads the
  newcomer's profile and kits while the match goes on. Seven sites of the
  game's code are rewritten and nine calls or jumps lead into new code on
  the second page of `.srp` (late_join_026g.asm and three C files, built
  into late_join_026g.section.bin); a writable page `.srpd` holds its
  state. Such a game computes a match unlike every game without it: the
  server never puts the two in one match (client_patch.md section 3.11);
- a game that goes on without the focus (patch revision 5, with the late
  join): engine_world::logic_tick 0x1A030 calls game::tick only while the
  window is the active one; otherwise it only sends what is queued, and
  the game stands still until the player comes back (alt-tab, a click on
  another window). Such a game answers nothing the match asks of it: the
  others see it frozen, and no player can be taken into the match while
  it is away, because every game in it has to prepare for the newcomer.
  The `jz` after the active check (0x1A0EE) becomes two nops: the window
  being active no longer matters to the simulation (client_patch.md
  section 3.12);
- three faults of a match that takes players in, found with the first
  late joins of real players (patch revision 6, client_patch.md section
  3.13). The hud's lists of players (the scoreboard, a second list, the
  enemies' markers) are made once, when the match is ready: the late join
  makes them again when a newcomer has its place (one more jump into the
  new code, at 0x23572B). loose_oct_tree::insert 0x39E30 takes a box with
  a number that is no number as a box at the origin (a jump at 0x39E52):
  with such a box the tree doubled its size without end, a node each
  time, until none was left, and read through a null one, which killed a
  game that was given a newcomer. And
  scene::remove_model 0x14FF60, which went on to take a model out of its
  list when the model was not in it (and wrote through what it found
  there), lets go of the model and returns when the model is not in the
  scene (a jump at 0x14FF9B to the function's end): that killed a game
  whose join the server gave up (seen on the test server);
- the weapons' random state at a spawn (patch revision 10, with the late
  join, client_patch.md section 3.18). A weapon's random state (its shots'
  spread) was 1 from its constructor and set by nothing but the load of a
  world, which holds the current kit's player alone: a game that loaded one
  during a match had the other kits' weapons at 1, the others had them
  drawn from, and a respawn into such a kit put it out of step. The
  respawn rule's spawn (0x316DA0) calls into the new code at 0x316EA0,
  after the new kit's player is initialised and before it is put into the
  world: each weapon of its inventory gets a state from its place, kit,
  slot and the spawn's time, the same in every game. And a game's first
  world of a match puts each place's input and kit choice into the place's
  lists in time order (calls at 0x2D1050 and 0x2D1055), the kit choice
  being the one the world made for the next spawn, not the kit in use;
- Team Deathmatch's rule counts a death after its own end (patch revision
  11, with the late join, client_patch.md section 3.21). tdm_rule_core's
  death handler 0x316090 returned at once when the rule's completion time
  (+0x12C) was set; the time is not serialized, so a rollback over the
  death that completed the rule left it set and the replay of that death
  was skipped: one dead player of the team that was out stayed in the alive
  mask, the server saw a player alive and the match ran to the time limit
  (match.md section 28). The `jnz` at 0x3160F1 becomes six nops, as the
  battery rule's does;
- the resources.db file table's signature is not checked (patch revision
  12, client_patch.md section 0.1): verify_sign's call of verify_sign_impl
  at 0x135D3 becomes `xor eax,eax` and three nops, verify_sign_impl's
  "good signature", so a game starts with an archive of new files. (A first
  writing of revision 12, never released, checked it with Skhid's key
  instead; the revision 12 that was built drops that code);
- the game mounts resources.db's patches (patch revision 12,
  client_patch.md section 0.2): query_db's call of query_mount_archive at
  0xE7E3E calls mount_patches_026g.asm at 0x400 of `.srp`, which mounts
  ../../resources.db as the game did and then every
  ../../resources.db.patch_<n> that is there, n from 1 to 255 in order (a
  patch is numbered by the patch revision that brought it; a missing number
  is skipped), at the same place, "resources": a later archive's file
  replaces an earlier one's, and its new files are added. on_mounted_resources 0x147E0 jumps there first:
  each start-up mount is held (a reference) and kept in `.srpd`; the last
  calls the game's code with resources.db's mount, then lets resources.db's
  reference go, so resources.db is held by engine_world alone, as in the
  retail game, and each patch by `.srpd` alone. A patch file that is there
  and cannot be opened (any CreateFileA error but file or path not found)
  stops the game ("Cannot open resources.db.patch_<n> (error <e>)"), and so
  does a patch that fails to mount ("Cannot mount resources.db.patch_<n>");
- the transparent effects scroll their textures as in 0.29 (patch revision
  13, client_patch.md section 3.26): effect_fstage_default_materials::
  compile reads a material's use_uv_scrolling into the shader
  configuration's CONFIG_USE_UV_SCROLLING bit (a jump at 0x1D90DC, unless
  use_sequence is on) and with it sends uv_scrolling_parameters0/1 from the
  four *_map_scroll_*_direction values (a jump at 0x1D94CB); the renderer
  binds scene_slomo, which 0.29's shaders multiply the time by, to a 1.0 (a
  jump at 0x14990F). The code is forward_scroll_026g.asm at 0xB00 of
  `.srp`. With the retail resources.db no shader the game loads changes
  (shaders/masks lists no such macro for forward_base);
- the sign-in reports a stale shell_32.dll in the temporary folder (patch
  revision 12, written as revision 13, client_patch.md section 3.25): sign_in_on_connected's
  `mov [edi+8],eax` / `mov [edi+0Ch],dl` at 0x380889 become a call of
  stale_dll_026g.asm at 0xE00 of `.srp`, which writes both, with byte 8 of
  the version field 80h, or 81h when %TEMP%\\shell_32.dll exists;
- "Watch replay" (patch revision 14, with the late join, client_patch.md
  section 3.27). The lobby's video_tutorial_link_clicked (0x28CDA0) opened
  whatever text the server's 9E put into the lobby client's link buffer with
  ShellExecuteA. A jump at 0x28CDB6 goes to replay_link
  (replay_link_026g.asm, .srp+220h, after the upload code and the default
  server, before the patches' mount): only `replay <8 hex> <32 hex> <64 hex>
  <1..10 digits>` starts updater_skhid.exe of this exe's folder with the
  command line `updater_skhid.exe <link>`; anything else does nothing;
- one upgrade (a camouflage, a decal) on several items at once (patch 30,
  with the late join, client_patch.md section 3.28). Taking an upgrade off
  named no item: the lobby's `46 02 <kit> <upgrade>` and the server's
  answer `83 46 02 <upgrade>`, which took it off the first item of the
  inventory holding it. Four calls or jumps into shared_upgrades_026g.asm
  (.srp+700h, after the patches' mount, before the pickup code): the
  upgrade window's drop (0x29AAD5) sends `46 02 <kit> <upgrade> <item>`, the
  item the window shows; `83 46 02 <upgrade> <item>` (0x275756) and
  `83 46 01`'s detach_first (0x275589, the item the answer puts it on)
  give process_detach_upgrade an item, which it looks up by its id
  (0x27534D) instead of the first that holds the upgrade;
- a kit chosen before the match starts is played at once (patch 31, with
  the late join, client_patch.md section 3.29): the respawn rule's tick
  (0x316820) first respawns, where it stands, a live player whose next kit
  is another while the match waits or counts down and who has no statistics
  yet; the view of that spawn keeps the warmup's camera and input until play
  begins (0x26794D); the hud shows the kits to a player who becomes alive
  then (0x23B7DA), keeps them and the cursor after a pick, also on the
  death screen (0x233F28), and hides them from a live one once play begins
  (0x23B7FC); while they are shown before play a drag (the left button,
  past a click's few moves, or the lobby's buttons) turns the warmup camera
  and the wheel moves it nearer or further, as the lobby's character view
  (0x25D0E0, the game world's input handler's mouse move), within the
  lobby's limits (0x24225E). The code is kit_change_026g.asm at 0xEC0 of
  `.srp`, warmup_look_026g.asm at 0xD40 and warmup_limits_026g.asm at
  0xA98;
- a weapon's upgrade window ("Optika", the scope slot) shows the weapon
  (patch 32, with the late join, client_patch.md section 3.30): the lobby's
  character shows the kit whose held weapon (slot 7, or 10 without a 7) is
  the window's item, also through every refresh while the window is open
  (setup_profile 0x284B10), and the lobby camera looks at the weapon from
  nearer (start_item_upgrade's camera call 0x265E73; the least distance at
  0x284081, 0x28428C and 0x284316 is the original's 1.5 again once the
  camera is back there); the window's close shows the kit chosen in the
  lobby again (0x265F44); while it shows the weapon, the player is drawn
  as in the lobby, not for the decal editor (0x284A33: no bind pose, the
  weapon in its hands). Any other window is the original's. The code is
  weapon_preview_026g.asm at 0x2E50 of `.srp`, after the late join, and
  weapon_draw_026g.asm at 0xD20;
- the order of the archive reads is a strict weak ordering (patch 33, with
  the late join, client_patch.md section 3.31): hdd_manager's sort of the
  pending reads (0x10BE80, std::sort with unguarded loops) used a predicate
  (0x10BCC0) that cycles when a batch reads from two archives (resources.db
  and a patch mounted over it), and the sort then read past its array: the
  lobby crash of round 4d. Its 0x1A0 bytes are read_order_026g.asm: a null
  iterator first, then the priority (+2B8h) descending, archive reads before
  others, then the archive's path and the file offset (others: the parent
  +158h or the query);
- the commit_suicide key (K) kills one's own player (patch 34, with the late
  join, client_patch.md section 3.32): the input builder (0x241250) never
  asked for action 1Fh, so the action mask never had bit 24, which the match
  simulation still reads as a hit on one's own head (0x2D8BC1). Its last
  action call (0x242196) calls the Rust patch suicide_key (the block
  patches.code) first, which sets bit 24 on the key going down;
- a killed player falls as a ragdoll (patch 76, with the late join;
  docs/notes/ragdoll.md): the Rust patch ragdoll (the blocks patches.code
  and .data, over the solver crate srp-026g/crates/ragdoll), six lohooks
  (ragdoll.toml, sites_026g.tsv): game_world::tick after the simulation's
  move (0x1F7C3C) steps every place's corpse (alive, or dead with its
  life's spawn time +328h as the death's key: a rollback that undoes the
  death drops the ragdoll, the same death simulated again keeps it) and
  the ragdolls (eleven capsules, XPBD, the level read by ray_test);
  mt_draw's cull (0x24757C) draws a corpse of ours; mt_draw after the bones
  (0x247716) captures the death clip and writes the ragdoll's bones over
  its stack array; mt_draw's read of the held item (0x24754F) poses a
  corpse holding nothing (a battery carrier's once the battery drops,
  when mt_draw updates no bone) from its ragdoll through update_skeleton;
  update_camera (0x1F3643) gives the local player's view its ragdoll's
  eye; bullet::check_collision after its ray (0x32D627) tests the bullet's
  segment against the ragdolls: the part hit is pushed (once a bullet a
  victim: the bullet's shooter and id and the corpse's death, so neither
  a re-simulated tick nor the bullet's next segment pushes again), a
  settled body wakes, and the game's body-hit particle and sound play
  there; a bullet in a living player's hit model is kept (once a bullet)
  and its death starts with its push (the round's momentum: the ammo's
  mass, at most 0.04 kg, times the bullet's speed; a battery drops no gun),
  the body a powered ragdoll (muscles that weaken from the death, the
  kit's weight from the inventory on its parts). Drawn only: nothing a
  match computes, keeps or hashes changes, the bullet goes on as before;
- Blackout, a battery match with no interface (patch 65, a prototype, with
  the late join; docs/notes/blackout.md; client_patch.md section 3.58): the
  Rust patch blackout (rust/patches/src/blackout.rs, blackout.toml), five
  lohooks at sites of its own (sites_026g.tsv): srp's block's tag 2 read
  after each match_options::deserialize (0x27B92D, 0x1F0971); each frame's
  update_player_icons (0x23AA10) tells the hud and the player icons the
  forced interface and Tab; on_keyboard_action after its Tab test (0x25CD2C)
  lets a key go on while Tab is held; the input builder's end (0x2421B0)
  takes the moves and acts out of the mask while Tab is held;
- a spot has no cooldown, and a hit on a battery's body spots it (patch 38,
  with the late join, client_patch.md section 3.35, docs/notes/spotting.md):
  base_player::apply_new_input (0x2D86E0) set the spotter's
  m_time_left_to_try_spot_in_ms (+34Ch) to 3000 at every spot, hit or miss
  (0x2D87C1); it is set to 0, so the next press spots again. A ray hit on a
  battery's rigid body (group 0A18h) found neither a player nor a geometry
  through its owner (victory_item_core+58h, vtable 0xA13EEC) and spotted
  nothing; 0x2D8A0F jumps to spot_026g.asm at 0x7C0 of `.srp`, which spots
  the battery through its spottable interface (+54h), as its use box does;
- a baked camouflage/decal texture that is no longer in the baker's list
  is baked again instead of stopping the game (patch 35, with the late
  join, client_patch.md section 3.33). The baker keeps a list of the
  textures it saved to user_data/streamed_textures (name, CRC of the bytes,
  size, age, "a load is under way"); a translate query finds the name there
  and loads the file, check_loaded_baked_texture (0x131B40) compares the
  loaded bytes' CRC with the list's: equal, the file is used; different,
  the entry is erased and the query asks to be made again (result_requery:
  "Texture %s is corrupted!"), which bakes it anew. A second load of that
  name under way finds no entry and stopped the game with the fatal
  "Saved baked texture %s not found" (seen live 2026-10-17, Rudnya). The
  `jnz` at 0x131C1E after the assertion's "ignore" flag test becomes a
  `jmp`: the function's own path for that flag, `return false`, so that
  load is made again as the first was;
- the memory arena "ai" is 4 MiB instead of 256 KiB (patch 36, with the
  late join, client_patch.md section 3.34). A static initializer registers
  the arena with its size as an immediate (`mov dword [esp+18h],40000h` at
  0x1E8CCB, the name "ai" at 0x9FA4E0, then 0xE21C0); no option is read
  there. Only ai::fsm allocates from it (fsm::append_transition 0x38C970,
  0x28 bytes a transition), and every weapon's state machine is made in
  it: a 20-player match with three kits each (one player object per kit)
  ran out of it and stopped the game with "not enough memory for arena
  [ai]" (seen live 2026-10-17). The immediate becomes 400000h;
- the default multithreaded arena and resource-memory floor are both 768
  MiB, matching the launcher (patch 37, with the late join). Only the two
  default-only immediates change; explicit numeric options keep their native
  branches. See README section 37 for the crash evidence and its limits;
- the scoreboard (Tab) shows a row for the places in the match and none for
  a place whose player left (patch 39, with the late join, client_patch.md
  section 3.36; the rules: docs/notes/tab-list-left-places.md): its rows
  are put in by place in the hud's initialize_match (0x234880) and were
  never taken out, so a player who left kept a grey row, and one who came
  back had a second. The row goes in only for a place whose kd_stats
  `online` is set (tab_row, late_join_026g.asm), the shown list is made
  again when those places change (tab_shown, at 0x23B421), and the
  refresh leaves the other places alone (0x23B447..0x23B476 in place);
- a model is measured as the original's 65 degrees would see it at a wider
  field of view (patch 40, with the late join, client_patch.md section
  3.37, docs/notes/fov_lod.md). select_lod (0x16EEC0) hides a static model
  of screen-size LOD below a share of the screen, and the screen factor
  (0x158D80) drops small shadow casters: both through the projection, so
  at uro_fov 100 models went out at about half the distance.
  fov_projection keeps k = max(1, tan(A/2) / tan(65 z/2)) in the state
  page; select_lod's x0.5 (0x16F2A4) and the factor's x100 (0x1590AF)
  call fov_lod_scale and fov_screen_scale, which multiply by k after the
  original's constant (k is 1 at 65 and less, and through a scope);
- on the death screen a drag turns the camera; the wheel does nothing
  (patch 46, with the late join, client_patch.md section 3.43, docs/notes/
  death-camera.md). A death leaves the player's input handler in the first
  person mode: the camera is the corpse's eyes (player+44Ch), and with the
  kits shown the cursor takes every move. The game world's on_mouse_move
  (0x25D0E0, WARMUP_LOOK_HOOK) jumps to death_look first: while the kits
  are shown, the handler is in the first person mode and the local player
  is dead, a drag as patch 31's (the lobby's buttons at once, the left one
  past a click) turns a yaw and a pitch of its own (within 70 degrees
  either side, 60 up, 40 down) and takes the move, and the wheel stays the
  cursor's; any other move goes on to warmup_look. update_camera's call of
  the view's update (0x1F364B) calls death_view, which turns the eyes' view
  by them relative to the head's pose (the update's own third-person step
  on the identity, times the eyes' transform) while that player is dead; a
  live one clears them. The code
  is death_camera_026g.asm, the section `.srpc`; a dead player sends no
  input (0x1F7883), so nothing a match computes changes;
- the artefacts as the owner set them (patch 54, with the late join,
  client_patch.md section 3.51, docs/notes/artefacts.md; their numbers in
  the configs, release/content.tsv artefacts-1.json). A Spring's loader
  (0x33ACF0) reads a fixed set of keys into its character modifier nodes,
  none of them the aim time; artefacts_026g.asm (the arena's block artefacts.code) makes its
  passive's unused max_carried_weight node aim_time with
  `passive/aim_time_mod` and adds two active nodes, aim_time with
  `active/aim_time_mod` at +3B8h and list 42 with
  `active/stopping_power_mod` at +3C8h (a Spring is 3D8h bytes, 0x25E7A8,
  0x2EEA74), linked and unlinked with the active's (0x311BC0, 0x311DA0);
  the player's stagger (0x2DBCD0, at 0x2DBD89) is times max(0, 1 + list
  42), so bullets do not slow an active Spring's holder. A
  Lifebone's active (0x312250) removed injuries only; after them it fills
  the pain, body and head pools (0x3122AA, increase_health as a medkit). An artefact's
  press (artefact_base::action 0x313300, at its entry) activates every
  ready artefact of the pressed one's class in the artefact slots, so one
  key fires all of a kind that are ready, even when the pressed one is not.
  Onyx: n fired together are each active 1 s longer per other one (3 s + (n
  - 1) s), and only the first active Onyx of the slots reduces a hit (its
  hit callback 0x312BF0, at 0x312C03), so n active Onyx reduce it once; its
  activation hint (0x25F9DD) shows once a press, for the active's time;
- the keys 1, 2 and 3 pick kit 1, 2 and 3 while the kits are shown, and an
  option hides the other players' chat lines in a match (patch 58, with the
  late join; docs/notes/match-hud-options.md). match_keys_026g.asm (the
  arena's blocks match_keys.code and .state): the game world's
  on_keyboard_action (0x25CD00, at its kind's test 0x25CD41) invokes the hud
  movie's root.select_kit_key for a key 1 to 3 going down while the hud
  shows the cursor and has shown it for 300 ms (its handler's slot +14h,
  0x9FC59C, after each poll, GetTickCount: a key pressed before then, or in
  the frame the kits came, picks nothing), and goes on as before; the hud's constructor call (0x1F6845)
  registers a cc_bool named as the gameplay options' dead option 4
  (g_private_messages_in_game), which makes that option live and saved in
  user.cfg; add_message's call for a received line (0x26B916) drops a
  player's line in a match while it is on (channels 3 and 5 to 9);
- one key hides the whole interface and shows it again, in a match and in
  the lobby (patch 62, with the late join; docs/notes/hide-ui-key.md): the
  game's action toggle_replay_ui (19h, F11), which only worked in a replay.
  hide_ui_026g.asm (the arena's blocks hide_ui.code and .data): the game
  world's on_keyboard_action case 19h (0x25CED1) without the replay's movie
  asks the hud movie root.interface_hidden and gives it and the player icons'
  movie root.hide_interface(not that), the kits before the start too (the
  movies go on running: what happens while hidden plays out unseen; a death
  keeps the state; initialize_match's root.reset (0x234AE8) shows both
  again: a new match starts shown); lobby_menu::on_keyboard_action's binder call
  (0x26CCD2) asks again for kind 1 when the key has no lobby action, and on
  toggle_replay_ui invokes the lobby movie's root.toggle_interface and takes
  the key (back at 0x26CCAE, al 1); while the movie says its interface is
  hidden (root.interface_hidden) any other key goes straight to the movie
  (back at 0x26CDD2), no window key or F6's Play acting;
- the kill feed shows a kill through a wall, by a ricochet and by pain (patch 67,
  with the late join; docs/notes/kill-feed-icons.md). kill_feed_026g.asm
  (the arena's block kill_feed.code): generate_killed_event (0x2DA170, at
  0x2DA3D1) writes after the event's cause icon a byte of flags, for a
  bullet and someone else the killer (the cause icon not 3), 0.69d0's
  penetration_icon and ricochet_icon rules: 4 when it ricocheted (its
  reflections +88h not 0), else 1 when it went through something first
  (its segments +84h not 0); and 2 when the victim's pain pool is empty
  and its body and head pools are not. The hud's event handler (0x23B6F0)
  passes the word, so on_player_killed's fifth argument carries them in
  its second byte, and at 0x23814F the description gets penetration_icon,
  ricochet_icon and pain_icon (0 or 1) for the hud movie's row. The
  simulation's own state, read alike in every game; nothing a match
  computes changes;
- a fall's speed counts the time it fell (patch 72, with the late join;
  docs/notes/fall_damage.md "Split ticks"): step_down's free fall
  (0x7E6E6..0x7E75C, rewritten in place, FALL_GRAVITY) takes gravity x
  min(player_step's dt, 0.05), not x 0.05 at every step, so a tick the
  world steps in parts (each precise input) adds one tick of speed in all;
  whole ticks bit for bit as before. A simulation change;
- the baked camouflage and decal textures are kept from game to game, and
  a texture asked for again while its bake or load is under way is made
  once (patch 70, with the late join; docs/notes/baked-textures.md).
  bake_cache_026g.asm and .c (the arena's blocks bake_cache.code and
  .state): game::game deletes user_data/streamed_textures only when its
  stamp file names another revision (0x1E9455), and puts its whole DDS
  files back into the baker's list (CRC 0, the first load's taken at
  0x131C90); game::exit's delete is gone (0x1EC832); translate_query's
  look-up (0x12D4CD) lets a query whose path is under way wait, and the
  texture made (0x12F18F a bake, 0x12DDB4 a load) is given to it; an owner
  that ends otherwise has its waiting queries made again (finish_query,
  0xF0D00). Textures only;
- a weapon's upgrades in at most three upgrade records, the weapon kit
  (patch 68, with the late join; docs/notes/weapon-kit.md): the Rust patch
  weapon_kit over the crate srp-026g/crates/weapon-kit (weapon_kit.toml).
  translate_query's call of 0x2F2060 (0x2F1771) goes to the code hook
  weapon_kit_026g.asm, which makes it and takes room below the cook's frame;
  at 0x2F1776 a weapon whose records hold a kit is cooked from the records
  and one for each dict id the kit names, in that room; the lobby's upgrades
  to Flash (0x2882E3) give a kit's object kit_own, kit_camo, kit_muzzle and
  kit_grip; setup_inventory_data (0x2885CE) marks the items a kit names as
  attached; patch 48's apply calls weapon_kit_preview_apply through its
  dword +23h, which the build writes. Nothing a match computes changes;
- the option B snapshot upload: two 5-byte jumps (0x27AEA1 in
  on_match_ready, 0x27A505 in the 0xF6 case of on_match_packet_received)
  into the upload code, and the upload code itself:
  --cave=section (default): a new section `.srp` (RVA 0xA415000, VA
    0xA425000, 0x200 bytes of file) after .reloc. The section table gets an
    8th entry (40 of the 216 free header bytes), SizeOfImage grows by one
    page, and the Authenticode blob moves 0x200 bytes towards the end of
    the file, where the security data directory finds it again. Nothing of
    the original code is lost: -debug_hash_mismatch keeps its function
    (but the 0xF6 branch that called it now uploads instead);
  --cave=hash-mismatch: the code overwrites game_world_core::on_hash_mismatch
    (0x325A60, 0x47B bytes, reachable only through the branch the request
    hook replaces); the rest of it becomes int3. No header changes;
- base relocations that overlap rewritten bytes become no-op (type 0)
  entries, so a rebase could not corrupt the new code;
- the PE checksum is recomputed (full patch only; --cert-only keeps the
  file byte-identical to the copy installed in step 0).
The 0.21d mode 3 battery patch is NOT ported: 0.26g serializes the
next-battery time (match_time_vi_spawn_rule_core +0x16C), the 0.21d bug.

Each site is checked against the original bytes first, so a different build
or an already patched file is refused rather than patched twice.
"""

import collections
import hashlib
import os
import re
import struct
import sys

import hooks_026g
import link_026g as linker

IMAGE_BASE = 0x10000
TEXT_VA, TEXT_FILE = 0x11000, 0x400          # same in 0.26g as in 0.21d
TEXT_VSIZE = 0x85E5A3
EXE_SHA256 = "ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153"

# vostok::crypt chain check of the signing certificate: sub esp,2Ch -> xor eax,eax; ret
CERT = (0x12A10, bytes.fromhex("83ec2c"), bytes.fromhex("31c0c3"))

# survarium_or_its_installer_is_already_running: cmp eax,0B7h / jz fail
MUTEX = (0x124F9, bytes.fromhex("7409"), bytes.fromhex("9090"))

# Skip the fatal low-VRAM warning after cmp ebx,14000000h (320 MiB).
# Same one-byte change as play_skhid.sh: file offset 0xF5811, 73 -> EB.
# The branch still targets 0x106422, which returns the detected size.
MEMORY_CHECK = (0x106411, bytes.fromhex("730f"), bytes.fromhex("eb0f"))

# Missing-server fatal error, reached only after both server and replay
# options were absent. Replace the push+call with jmp + nops when embedding.
DEFAULT_SERVER_HOOK = (0x1EB25B, bytes.fromhex("6890a99f00e81b83f2ff"))
DEFAULT_SERVER_OFFSET = 420  # immediately after the snapshot upload code
DEFAULT_SERVER_RETURN = 0x1EAE2D
DEFAULT_SERVER_OPTION = 0xA9C550


# network_client::try_send_all, the one-time read of the option object
# 0xAA19F8 (-hash_check_interval_in_ms) into the static 0xA2B393C:
# mov dword [esp+18h],64h (the default, kept when the key is absent) -> 32h.
# The whole instruction is checked; only the imm32's low byte changes.
HASH_INTERVAL_VA = 0x27CC4F
HASH_INTERVAL = (HASH_INTERVAL_VA, bytes.fromhex("c744241864000000"), bytes.fromhex("c744241832000000"))
HASH_INTERVAL_BYTE_VA = HASH_INTERVAL_VA + 4  # the one byte that differs
HASH_INTERVAL_MS = 50                         # = one tick

# sign_in_on_connected 0x3807D0 builds the 12-byte version field of the first
# login message on the stack at [esp+20h]: eight zero bytes, "0.26" at +0,
# "g0" at +4, the string's own zero at +6 (through cl, read here) and a zero
# at +6 again (`mov [esp+26h],al`, 0x38087B); four more zero bytes follow in
# the message. mov cl,[9DAE9Eh] -> mov byte [esp+27h],REVISION / nop.
# The base relocation of the address (0x380864) is disabled with the rest.
# 0: an exe without this patch (the download of 2026-10-12). 1: the patches
# above and the revision field. 2: adds the optional embedded-server patch.
# 3: adds the battery rule patch below, the first that changes what a match
# computes: a game of revision 3 and an older one part at a team's last
# battery (match.md section 26). 4: adds the two start-up patches below
# (a Documents folder the game may not write to); a match is not affected.
# 5: adds the late join below. A game with it and one without never play
# together: the world they keep and the packets they send differ. With it
# goes the game that goes on without the focus (RUN_UNFOCUSED).
# 6: adds the three faults of a match that takes players in (the hud's
# lists made again, BOX below, REMOVE_MODEL below). A game of 5 in a match
# can still be killed by a newcomer, so a server that takes players into
# matches asks for 6. 5 and 6 compute a match alike and play together.
# 7: a slot's grenades and traps go in front of the list the world is saved
# by, whenever the slot is bound (late_join_026g.asm, bind_slot_reserved),
# and into the states a game rolls back to (late_join_install_026g.c). In 5
# and 6 a player with a grenade or a trap in a kit who was taken into a
# match being played was never let in. The order of the world's game
# objects is another one than 5 and 6 keep: a game of 7 and an older one
# part as soon as two players' grenades are not as they began, so a server
# asks for 7.
# 8: the game's clock is brought to the match clock at 30 % instead of 5 %
# (CLOCK_FACTOR below). Nothing a match computes: 7 and 8 play together,
# and the match server times its corrections by the revision a game says.
# 9: the statistics handler keeps the scoreboard's score of all twenty
# places (LATE_JOIN_PATCHES, start_match). In 5 to 8 a newcomer's score was
# added again at every rollback over its events, and its row showed a
# score of an earlier match until its first. And the hud draws the team
# points while a match counts down (PREGAME_PROGRESS); the original kept
# the last match's until play began. Neither is saved in the world or
# hashed: 7, 8 and 9 play together.
# 10: every spawn gives each weapon of the new kit a random state every game
# works out alike (WEAPON_RANDOM below). In 9 and before a game that loaded
# a world during a match kept the other kits' weapons at the constructor's
# 1, and a respawn into such a kit put it out of step at once. What a match
# computes changes from a match's first spawn: a game of 10 and an older one
# part there, so a server asks for 10. With it, a game's first world of a
# match puts each place's input and kit choice where their time puts them in
# the place's lists, the kit choice being the one the world has made for the
# next spawn (FIRST_LOAD below): in 9 and before a newcomer could respawn a
# player in another kit than the other games did.
# 11: Team Deathmatch's rule counts a death after its own end (TDM_DEATHS
# below). In 10 and before a rollback over the deciding death left the
# rule's completion time set, the replay skipped the death, and one dead
# player of the team that was out stayed in the alive mask: the match the
# server reads never ended before the time limit. What a match computes
# changes at the first death after a team is out: a game of 11 and an
# older one part there, so a server asks for 11. With it, a field-of-view
# setting (FOV_PATCHES and FOV_PROJECTION_HOOK below): the video options'
# slider is bound to uro_fov, from 50 to 100, which the camera's angle
# follows unaimed; through a scope the angle is the original's. A camera is
# not in what a match computes. And a battery under the floor one stands
# on can be picked up (PICKUP_* below): a use is in what a match computes,
# so this too parts a game of 11 from an older one.
# Also in 11, for the replay viewer (docs/notes/replay_sur.md): a replay's
# camera follows the player it is on into another kit (REPLAY_REATTACH
# below; a replay watched in first or third person died at that player's
# first respawn into another kit), a weapon's full sound queue drops the new
# sound (SOUND_QUEUE below; a replay died about 50 s after a seek), and a
# replay's seek empties the weapons' sound queues (SEEK_CLEAR below; sounds
# played late after a seek). Nothing a match computes: no revision of their
# own (owner, 2026-10-15).
# 12: the resources.db signature is not checked at all (DB_SIGNATURE below):
# a retail archive and one with new files both start the game; the game
# mounts resources.db.patch_<n> over resources.db (MOUNT_PATCHES below); and
# the sign-in reports whether %TEMP%\shell_32.dll exists (STALE_DLL below),
# in byte 8 of the version field. Nothing a match computes: 11 and 12 play
# together. (Owner, 2026-10-16: this build is revision 12. The numbers 12
# to 14 were used while it was written, never released: a 12 that started
# only with a resources.db Skhid's key signed, a 13 that added the
# shell_32.dll report to it, a 14 with the signature skip and the patches.)
# 13: the transparent effects (forward_default) scroll their textures as in
# 0.29 when a material asks for it (FORWARD_SCROLL_HOOKS below). Rendering
# only, nothing a match computes: 12 and 13 play together.
# 14: "Watch replay" from the lobby's History (REPLAY_LINK below): the lobby's
# link callback starts updater_skhid.exe with a link of the one form
# `replay <id> <token> <sha256> <size>` and opens nothing else, where the
# original opened any text the server sent with ShellExecute. Nothing a match
# computes: 11 to 14 play together. A server sends a replay link only to a
# game that says 14 or more (an older one would open the text as a file).
# 15 and 16: 14 with the revision byte alone changed (releases that needed a
# number above the live one). After 16 each release has the live exe's + 1,
# and README.md's revision table has its row, which the release writes.
#
# The revision of the last release. The release sets it (release/release.sh,
# its "Release revision <n>" commit), never a feature change: a release
# builds with --revision=<n>, the live exe's + 1, which changes the revision
# byte alone, and records <n> here once it went out.
REVISION = 40
# The first revision whose default build takes the lobby's replay link
# (REPLAY_LINK): the server's replay rule asks for it
# (session::v026g::replays::MIN_REVISION).
REPLAY_LINK_REVISION = 14
# What a full build without the late join says (--no-late-join, and every
# --cave=hash-mismatch build): the players' download before revision 5.
REVISION_WITHOUT_LATE_JOIN = 4
REVISION_VA, REVISION_ORIGINAL = 0x380862, bytes.fromhex("8a0d9eae9d00")


def revision_site(revision):
    """(address, the original's bytes, `mov byte [esp+27h],revision` / `nop`)"""
    return REVISION_VA, REVISION_ORIGINAL, bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90])


REVISION_SITE = revision_site(REVISION)

# gather_victory_items_rule_core's container event handler 0x313A50:
# `cmp dword [ecx+15Ch],-1` / `jnz 313B3Bh` (an event after the completion
# is not counted) -> the jnz becomes nops;
GATHER_EVENTS = (0x313A62, bytes.fromhex("0f85d3000000"), b"\x90" * 6)
# `mov [ecx+15Ch],edx` / `mov byte [ecx+160h],0` / `ret 0Ch` (at the
# completion no container can be used any more) -> the mov byte becomes nops.
GATHER_LOCK = (0x313B34, bytes.fromhex("c6816001000000"), b"\x90" * 7)
GATHER = (GATHER_EVENTS, GATHER_LOCK)

# tdm_rule_core's death handler 0x316090 (event 1): `cmp dword
# [esi+12Ch],-1` / `jnz 3161DDh` (once the rule has set its completion time,
# +12Ch, no death is counted: not the ticket, not the player's leaving the
# alive mask) -> the jnz becomes nops, as for the battery rule above. The
# completion time is not serialized (the constructor's 0x31601D is the only
# write of -1), so a rollback over the death that completed the rule keeps
# it, and the replay of that death is skipped (match.md section 28).
TDM_DEATHS = (0x3160F1, bytes.fromhex("0f85e6000000"), b"\x90" * 6)

# camouflage_decal_texture_type_baker_cook::check_loaded_baked_texture
# 0x131B40 (`ret 0Ch`: the baker, the loaded resource, the path) looks the
# path up in the baker's list (baker+230h begin, +234h end; 124h a record:
# the path's buffer_string, +114h CRC, +118h size, +11Ch age, +120h "a load
# is under way"). Not found: `cmp byte [226091Dh],0` (the assertion's
# "ignore" flag) / `jnz 131C73h`, else the fatal assertion "Saved baked
# texture %s not found" at 0x131C58 and `int3` at 0x131C72. 0x131C73 lets
# the resource go and returns false, which on_baked_texture_loaded
# (0x12DB40) answers with "Texture %s is corrupted!" and result_requery,
# as for a CRC that differs. jnz -> jmp: the flag test is kept, unused.
BAKED_TEXTURE_MISSING = (0x131C1E, bytes.fromhex("7553"), bytes.fromhex("eb53"))

# The memory arena "ai": a static initializer (0x1E8B50) makes the doug_lea
# arena 0xA967D8 (vtable 0x9E68DC at 0x1E8C60) and registers it in the
# arenas' list at [0xA808FC] with 0xE21C0: {allocator, ?, size (u64),
# base, name} on its stack, the size `mov dword [esp+18h],40000h` at
# 0x1E8CCB (256 KiB), the name "ai" (0x9FA4E0) at 0x1E8CDB. Its one user
# is ai::fsm (append_transition 0x38C970: 0x28 bytes a transition,
# pop_state, generalized_action): the weapons' state machines of every
# player object. Full: memory_doug_lea_allocator.cpp:53 "not enough memory
# for arena [ai]" (out_of_memory_with_crash 0xE4D70). 4 MiB; a number,
# no relocation under it.
AI_ARENA_VA = 0x1E8CCB
AI_ARENA_SIZE = 0x400000
AI_ARENA = (AI_ARENA_VA, bytes.fromhex("c744241800000400"), bytes.fromhex("c744241800004000"))

# Match the shipped launcher's budgets when command-line keys are absent.
# mt_allocator starts at 16 MiB, then adds 360 MiB on the default branch;
# an explicit value instead takes the unchanged `shl esi,20` branch.
# minimum_resources_memory_size defaults to 336 MiB before the native u64
# multiplication by MiB. Its explicit numeric path jumps over this immediate.
# Both values are numbers, not addresses; neither has a base relocation.
MEMORY_DEFAULT_MIB = 768
MEMORY_DEFAULTS = (
    (0xE100F, bytes.fromhex("81c600008016"),
     b"\x81\xc6" + struct.pack("<I", (MEMORY_DEFAULT_MIB - 16) << 20)),
    (0xE9985, bytes.fromhex("b850010000"),
     b"\xb8" + struct.pack("<I", MEMORY_DEFAULT_MIB)),
)

# replay_network_client's "is this place mine" (vtable 0x9FF344, slot +0x54
# at 0x9FF398) is 0x54E10, `xor al,al` / `ret 4`. The client respawn rule's
# kit change (0x2678E0) asks it and re-attaches the camera to the place's
# new kit object when it says yes, as the live client's 0x27B550 does for
# its own place; the replay never did, kept drawing the old kit's object as
# the one watched and gave the new one's weapon a HUD of 0 + 0x2E4
# (weapon::draw 0x24502B). The slot gets a thunk in the int3 padding after
# is_player_current 0x242A40 (the client in eax, the place on the stack,
# `ret 4`): `mov eax,ecx` / `jmp 242A40h`, so "mine" is "the place the
# camera is on". The slot keeps its base relocation.
REPLAY_REATTACH_THUNK = (0x242A69, bytes.fromhex("cc") * 7, bytes.fromhex("8bc1e9d0ffffff"))
REPLAY_REATTACH_SLOT = (0x9FF398, struct.pack("<I", 0x54E10), struct.pack("<I", 0x242A69))

# A weapon's delayed sounds (delayed_callbacks_queue<64>, weapon+9FCh:
# begin, end, capacity end; 0x38 bytes an item) are emptied by nothing but
# the weapon's destructor. Each id keeps one item for good and a chain of up
# to 2 or 4 waiting ones; a seek moves the game clock and leaves the queue
# as it was. Its add (0x2454A0: drain, then look for the id's chain) pushes
# without looking, and push_back (0x245AD0) asserts at 64 (0x245B32: seen
# in a replay, about 50 s after a seek). After the drain, at 0x2454C1, `mov
# eax,[esi]` / `mov ecx,[esi+4]` (esi = the queue) becomes a jump to the
# cave in int3 padding after a function that ends at 0x835A15: with the
# queue full it goes to 0x24569A, the game's own exit for a sound past its
# chain's cap (the same frame: it calls the functor with flag 0, which the
# sound player 0x297E20 takes as "do not play", and returns `ret 18h`; the
# queue is not touched); otherwise it does the two instructions and goes
# back to 0x2454C6. A weapon with a full queue drops its new sounds until it
# drains, where the game died. Sounds only: nothing a match computes.
SOUND_QUEUE_HOOK_VA, SOUND_QUEUE_RETURN, SOUND_QUEUE_DROP = 0x2454C1, 0x2454C6, 0x24569A
SOUND_QUEUE_CAVE_VA = 0x835A20
SOUND_QUEUE_PADDING = (0x835A16, 42)          # the int3 run the cave is in
SOUND_QUEUE_CAVE = (bytes.fromhex("8b4e04" "3b4e08" "7307" "8b06")
                    + bytes([0xE9]) + struct.pack("<i", SOUND_QUEUE_RETURN - (SOUND_QUEUE_CAVE_VA + 15))
                    + bytes([0xE9]) + struct.pack("<i", SOUND_QUEUE_DROP - (SOUND_QUEUE_CAVE_VA + 20)))
SOUND_QUEUE_CAVE_SITE = (SOUND_QUEUE_CAVE_VA, bytes.fromhex("cc") * len(SOUND_QUEUE_CAVE), SOUND_QUEUE_CAVE)
SOUND_QUEUE_HOOK = (SOUND_QUEUE_HOOK_VA, bytes.fromhex("8b068b4e04"),
                    bytes([0xE9]) + struct.pack("<i", SOUND_QUEUE_CAVE_VA - (SOUND_QUEUE_HOOK_VA + 5)))

# vfs::mounter::finish 0x469F0, the last step of every mount: `mov
# eax,[eax+34h]` (the mount's root) / `test eax,eax` / `jnz 46B0Fh`, then
# for a mount without a root `xor ecx,ecx` / `jmp short 46B12h`, which goes
# on to call file_system.on_mounted(NULL) -> `jmp short 46B36h` (the
# function's end) / two nops. A mount has no root when it failed, as the
# user folder's does when <Documents>\Survarium is not there and cannot be
# made; resources_manager::on_mounted 0xF9A80 reads the node's flags at
# +30h without looking (client_patch.md section 3.10).
MOUNT_NO_ROOT = (0x46B0B, bytes.fromhex("33c9eb03"), bytes.fromhex("eb299090"))
MOUNT_NO_ROOT_END = 0x46B36

# The initializer of the option object -record_journal (0xA9D628):
# `push 0A9D628h` / `mov eax,9DB51Ch` ("1") / `call 0DDFF0h`. The imm32
# becomes the address of the exe's own "0" literal, the one the initializers
# of other options give in the same three instructions (0x84CF92, 0x84E4F2,
# 0x853B02). The journal is made by journaling 0xEB310 when
# the option reads as set (game start, 0x1F69B2); with "0" it does not, and
# -record_journal=1 still makes one. Only the imm32 is checked and
# changed, and its base relocation stays as it is: the new value is an
# address of the image too.
JOURNAL_DEFAULT_VA = 0x851CC7                 # the `mov eax,imm32`
JOURNAL_DEFAULT_ON, JOURNAL_DEFAULT_OFF = 0x9DB51C, 0x9DBF0C
JOURNAL_DEFAULT = (JOURNAL_DEFAULT_VA, b"\xB8" + struct.pack("<I", JOURNAL_DEFAULT_ON),
                   b"\xB8" + struct.pack("<I", JOURNAL_DEFAULT_OFF))
JOURNAL_OPTION = 0xA9D628

# game_world_core::on_hash_mismatch, 0x47B bytes of code + 5 int3 of padding
HASH_MISMATCH_VA, HASH_MISMATCH_SIZE, HASH_MISMATCH_REGION = 0x325A60, 0x47B, 0x480
HASH_MISMATCH_SHA256 = "f757a41458ada034433e7476bb47447a433c7c0a33f5b403cd4168e0688a040c"

# The new section of --cave=section, right after .reloc.
SECTION_NAME = b".srp\0\0\0\0"
SECTION_RVA = 0xA415000                       # = the original SizeOfImage
SECTION_VA = IMAGE_BASE + SECTION_RVA         # 0xA425000, the CAVE of the .section.bin
SECTION_CHARACTERISTICS = 0x60000020          # code, execute, read (as .text)

CAVES = {"section": SECTION_VA, "hash-mismatch": HASH_MISMATCH_VA}

# vostok::crypt::verify_sign 0x13310, after the survarium.com certificate
# was found and its chain check (CERT, 0x12A10) passed: `mov eax,esi` /
# `call 12CB0h` (verify_sign_impl: eax = the certificate's context, four
# cdecl arguments: the file table, its size, the signature, its size; 0 for
# a good signature) / `add esp,10h` / ... / `mov ebx,eax`, verify_sign's
# result, which the mounter tests (0x432DD: 0 mounts, any other value is
# "Resources archive is corrupted"). The call becomes `xor eax,eax` and
# three nops: no signature is checked; the `add esp,10h` after it still
# takes the four arguments off. The call's rel32 holds no address: no base
# relocation is under it.
DB_SIGNATURE_VA, DB_SIGNATURE_IMPL = 0x135D3, 0x12CB0
DB_SIGNATURE = (DB_SIGNATURE_VA, bytes.fromhex("e8d8f6ffff"), bytes.fromhex("31c0909090"))

# resources.db's patches (patch 27, client_patch.md section 0.2). The game
# mounts one archive at start: the start-up (0x14E8A) makes a
# mount_by_config_helper (0xE7D20) for "resources", whose query_db (0xE7D90)
# formats "../../%s.db" and calls query_mount_archive (0xE7EF0: esi = the
# allocator; the virtual path, the file table's path, the archive's path,
# the descriptor, the callback, cdecl) once, at 0xE7E3E. Its callback,
# on_mounted_resources (0x147E0, the only one: its one reference is the
# bind at 0x14E34), keeps the mount in engine_world+2D4h (m_resources_mount;
# +2C0h is the shaders' masks) and goes on with the start-up. The call at 0xE7E3E calls mount_all of mount_patches_026g.asm
# (MOUNT_PATCHES_OFFSET of `.srp`, after the upload code and the default
# server's stub and name, before the pickup code) instead; the first six bytes of 0x147E0 (`push ebp` / `mov ebp,esp` /
# `and esp,-8`) jump to its on_mounted and a nop. What the code does: the
# .asm's head. Neither site holds an address: no base relocation is under
# them. The code keeps its state at MOUNT_STATE_OFFSET of the late join's
# state page (the late join's own state is its first 60 bytes); the .bin
# ends with the address it was assembled for, which build() checks. A patch
# is numbered by the patch revision that brought it, a byte: n = 1..255,
# each that is there, in order.
MOUNT_PATCHES_OFFSET = 0x400
MOUNT_PATCHES_VA = SECTION_VA + MOUNT_PATCHES_OFFSET
MOUNTED_OFFSET, MOUNT_ALL_OFFSET = 0, 0x88      # the .asm's two entries
MOUNT_STATE_OFFSET = 0x800
MOUNT_MAX_PATCHES = 255
MOUNT_STATE_SIZE = 8 + 4 * (1 + MOUNT_MAX_PATCHES) + MOUNT_MAX_PATCHES
# The patches an exe requires (patch 49, README; client_patch.md section 3.46):
# a missing resources.db.patch_<n> whose bit is set stops the game at its
# start with the "Cannot open resources.db.patch_<n> (error 2)" fatal, as
# one that cannot be opened. The bitmap (bit n of byte n/8) is the .asm's
# `needed`, the MOUNT_REQUIRED_SIZE bytes before the state's address at the
# .bin's end, right after the "Cannot open" text (release/lib.sh's
# exe_required_patches finds it so). build() writes it; the checked-in .bin
# holds zeros, and a build without required_patches requires none. A
# release passes its offered-patch list's numbers (release/build-zip), so
# its exe never runs on older content than its server offers.
MOUNT_REQUIRED_SIZE = (MOUNT_MAX_PATCHES + 1) // 8
MOUNT_UNREADABLE = b"Cannot open resources.db.patch_%u (error %u). Please run the Skhid launcher again.\0"
MOUNT_QUERY_VA, MOUNTED_VA = 0xE7E3E, 0x147E0
MOUNT_QUERY_MOUNT_ARCHIVE = 0xE7EF0
MOUNT_QUERY = (MOUNT_QUERY_VA, b"\xE8" + struct.pack("<i", MOUNT_QUERY_MOUNT_ARCHIVE - MOUNT_QUERY_VA - 5),
               b"\xE8" + struct.pack("<i", MOUNT_PATCHES_VA + MOUNT_ALL_OFFSET - MOUNT_QUERY_VA - 5))
MOUNTED = (MOUNTED_VA, bytes.fromhex("55" "8bec" "83e4f8"),
           b"\xE9" + struct.pack("<i", MOUNT_PATCHES_VA + MOUNTED_OFFSET - MOUNTED_VA - 5) + b"\x90")

# UV scrolling for the transparent effects (patch 28, README; client_patch.md
# section 3.26): forward_scroll_026g.asm at FORWARD_SCROLL_OFFSET of `.srp`,
# after the replay's seek code and before the shell_32.dll check, three
# entries the three jumps below go to. effect_fstage_default_materials::
# compile (0x1D8F40): at 0x1D90DC (`mov dl,[esp+56h]` / `mov esi,[ebp+10h]`,
# after use_angle_depended_attenuation is read, before compile_begin at
# 0x1D9126) the switch use_uv_scrolling sets bit 6 of the shader
# configuration's byte 0Ch, unless use_sequence is on; at 0x1D94CB (`mov
# cl,[esp+57h]` / `and cl,3`, where both ends of the uv distortion's block
# meet) that bit sends uv_scrolling_parameters0/1 with set_constant<float4>
# (0x1A8900), as 0.29's compile does and as 0.26g's g_stage does (0x1E0815).
# renderer_context's constructor (0x148570): at 0x14990F (`mov
# [esp+2Ch],edi` / `lea esi,[ebx+123Ch]`, after its last
# register_constant_binding, scene_time's) the constant scene_slomo is bound
# to a 1.0 in the code. No site holds an address: no base relocation is
# under them. The .bin ends with the address it was assembled for, which
# build() checks. Rendering only: nothing a match computes reads any of it.
FORWARD_SCROLL_OFFSET = 0xB00
FORWARD_SCROLL_VA = SECTION_VA + FORWARD_SCROLL_OFFSET
FORWARD_SWITCH_OFFSET, FORWARD_CONSTANTS_OFFSET, SCENE_SLOMO_OFFSET = 0, 0x40, 0x100   # the .asm's entries
FORWARD_COMPILE_VA = 0x1D8F40
RENDERER_CONTEXT_VA = 0x148570
FORWARD_SWITCH_HOOK = (0x1D90DC, bytes.fromhex("8a542456" "8b7510"), FORWARD_SWITCH_OFFSET, 0xE9)
FORWARD_CONSTANTS_HOOK = (0x1D94CB, bytes.fromhex("8a4c2457" "80e103"), FORWARD_CONSTANTS_OFFSET, 0xE9)
SCENE_SLOMO_HOOK = (0x14990F, bytes.fromhex("897c242c" "8db33c120000"), SCENE_SLOMO_OFFSET, 0xE9)
FORWARD_SCROLL_HOOKS = (FORWARD_SWITCH_HOOK, FORWARD_CONSTANTS_HOOK, SCENE_SLOMO_HOOK)

# sign_in_on_connected 0x380800, where it finishes the 12-byte version field
# (section 6 of client_patch.md): `mov [edi+8],eax` (bytes 8..11, eax 0) /
# `mov [edi+0Ch],dl` (the EULA byte). Both become a call of
# stale_dll_026g.asm and a nop; the code there does both after its check,
# with the result in byte 8. It sits at STALE_DLL_OFFSET in the first page
# of `.srp`, after the replay's seek code (SEEK_CLEAR_OFFSET; 0xC00 to
# 0xE00 held the never-released first revision 12's signature code); build() checks that they do not
# overlap. The two instructions hold no address: no base relocation is
# under them.
STALE_DLL_VA = 0x380889
STALE_DLL_OFFSET = 0xE00
STALE_DLL_ENTRY = SECTION_VA + STALE_DLL_OFFSET
STALE_DLL = (STALE_DLL_VA, bytes.fromhex("894708" "88570c"),
             b"\xE8" + struct.pack("<i", STALE_DLL_ENTRY - STALE_DLL_VA - 5) + b"\x90")

# A kit chosen before the match starts is played at once (patch 31, README;
# client_patch.md section 3.29; the rules: docs/notes/kit-change-before-start.md):
# kit_change_026g.asm at KIT_CHANGE_OFFSET of `.srp`, after the shell_32.dll
# check and before the late join, four entries, and nops at one site.
# - The respawn rule's tick (0x316820, slot +50h of every respawn rule's
#   vtable; `push ebp` / `mov ebp,esp` / `sub esp,34h`) jumps first to
#   kit_change: while the time rule's state (core+18FE4h) is 1 or 2, a place
#   whose next kit (place+0Dh, the relayed kit choice as the place's tick
#   0x321790 took it) is another than the kit in use (place+0Ch), whose
#   player is alive and in the world, and whose statistics record is all
#   zero (no event of any kind, no damage taken from a player), is taken out
#   of the world (vtable+24h, 1) and spawned (rule vtable+5Ch) with a respawn
#   point of its feet (player+43Ch), its yaw (+490h) and what is left of its
#   invulnerability (damage model+32Ch), as the tick does for a place it
#   respawns. No death and no respawn time. In the builds with the late
#   join it ticks before the places (tick list 0 by key: the late join's
#   binds key the places (place+1) << 16, the rules have the core's counter
#   from 0) and the time rule after both (list 1, 0x2D1E3F): a choice for a
#   tick is played at the next one, a switch is of a tick before play or of
#   the tick whose time rule begins play, before that rule's change, and a
#   choice for the tick a dead player respawns in is its next life's. Every
#   input it reads is in the world, so every game and
#   every replay of a tick does the same; what a match computes changes from
#   the first such kit choice, so a release with it asks for its revision.
# - The client's view of a spawn into another kit (0x2678E0) sets the
#   player camera (`push 1` / `call 0x1F8200` at 0x26794D); it calls kit_view
#   instead, which sets the warmup's camera and input (0x1F8190) unless the
#   state the network client set the camera for (+1F1C60h, which the time
#   rule's tick changes, 0x267AA0 -> 0x27B1A0, for a tick first played) is 3,
#   as that change itself does.
# - game_world_ui (0x23B740, the local player's state): when the local
#   player becomes alive, `call 0x23A1A0` / `push 0` at 0x23B7DA (then
#   0x2383B0, which shows or hides the kits) jumps to ui_alive, which has
#   them shown while that state is not 3 and hidden when it is; at every
#   update `lea eax,[ebp+0D0h]` at 0x23B7FC calls ui_start, which does the
#   same for a live player.
# - the profile_changed callback hid the kits after a pick (`push 0` /
#   `mov eax,ebx` / `call 0x2383B0` at 0x233F28), and with them the hud's
#   cursor (0x23A930), which showing them again put back at the centre: the
#   nine bytes are nops (KIT_PICK). The kits stay shown, the cursor where it
#   is, for another pick, before the start and on the death screen; they
#   are hidden when the player becomes alive in play, as before.
# No site holds an address: no base relocation under them.
KIT_CHANGE_OFFSET = 0xEC0
KIT_CHANGE_VA = SECTION_VA + KIT_CHANGE_OFFSET
KIT_CHANGE_TICK_VA = 0x316820                 # player_respawn_rule's tick
KIT_CHANGE_SHOWN_STATE = 0x1F1C60             # network client: the state the camera is set for
KIT_CHANGE_HOOKS = (
    (KIT_CHANGE_TICK_VA, bytes.fromhex("558bec83ec34"), 0, 0xE9),   # the respawn rule's tick: kit_change
    (0x26794D, bytes.fromhex("6a01e8ac08f9ff"), 0xE1, 0xE8),        # the view of a kit change: kit_view
    (0x23B7DA, bytes.fromhex("e8c1e9ffff6a00"), 0xF9, 0xE9),        # the local player alive: ui_alive
    (0x23B7FC, bytes.fromhex("8d85d0000000"), 0x108, 0xE8),         # every update: ui_start
)
KIT_PICK = (0x233F28, bytes.fromhex("6a008bc3e87f440000"), b"\x90" * 9)   # a pick: the kits stay
# The same patch's camera before the start: warmup_look_026g.asm at
# WARMUP_LOOK_OFFSET of `.srp` (after the weapon preview's draw, before the
# shell_32.dll check) and warmup_limits_026g.asm at WARMUP_LIMITS_OFFSET
# (after the seek code, before the forward effect's scrolling); a state of
# 12 bytes at WARMUP_LOOK_STATE_OFFSET of the late join's state page (after
# the weapon preview's). The input world gives a mouse move to its handlers
# in their priority's order until one answers 1 (0x3B9F10): the game world's
# (game world+1F8h, priority 7) before the player's input handler (game
# world+4D58h, priority 10), whose warmup mode turns the warmup camera and
# moves it by the wheel (0x240200, 0x242200). The game world's
# on_mouse_move (0x25D0E0) moves the hud's cursor and answers whether the
# cursor is shown; the kits show it (0x2383B0 -> 0x23A930), so with them
# shown before the start the cursor took every move and the camera stood
# still. Its first four instructions (ten bytes) are a jump to warmup_look
# and five nops: while the kits are shown and the player's handler is in the
# warmup mode (+358h = 3, set by the warmup camera 0x1F8190 alone: before
# play), a move with a button held is a drag as the lobby camera's
# (0x283D30: its buttons 2, 5 and 7 at once; the left one past CLICK of
# moves, a click before), which goes on to the player's handler with the
# hud's pointer kept off the screen, put back at the drag's end; a move of
# the wheel goes on to it alone; any other move is the cursor's. process_
# warmup_mode's store of the pitch (`movss [esi+350h],xmm0` at 0x24225E,
# eight bytes) is a call of warmup_limits and three nops: the pitch within
# the lobby camera's +-60 degrees, the distance within its 1.5 and 4.6
# (0x283EF0). No base relocation under either site.
WARMUP_LOOK_OFFSET = 0xD40
WARMUP_LOOK_VA = SECTION_VA + WARMUP_LOOK_OFFSET
WARMUP_LOOK_HOOK = (0x25D0E0, bytes.fromhex("5356" "8bf1" "8b8e704b0000"), 0, 0xE9)   # on_mouse_move: warmup_look
WARMUP_LOOK_STATE_OFFSET = 0xE20
WARMUP_LOOK_STATE_SIZE = 12
WARMUP_LIMITS_OFFSET = 0xA98
WARMUP_LIMITS_VA = SECTION_VA + WARMUP_LIMITS_OFFSET
WARMUP_DRAG_END_OFFSET, WARMUP_LIMITS_ENTRY_OFFSET = 0, 0x20       # the .asm's two entries
WARMUP_LIMITS_HOOK = (0x24225E, bytes.fromhex("f30f118650030000"), WARMUP_LIMITS_ENTRY_OFFSET, 0xE8)   # the pitch's store
# And a mouse button held from before the start is not the player's until it
# is let go: warmup_keys_026g.asm at WARMUP_KEYS_OFFSET of `.srp` (after the
# suicide key, before the seek code), the buttons marked in a dword at
# WARMUP_KEYS_STATE_OFFSET of the late join's state page (after the warmup
# look's). The input world tells its handlers of each button each frame
# until one answers 1 (0x3B9D90): kind 0 and 2 when it goes down, 1 when it
# is let go, 2 at every frame it stays held; the game world's
# on_mouse_key_action (0x25CF60) takes them while the cursor is shown, the
# player's input handler queues each kind with the action (0x23FF50) and its
# play step takes the weapon's action at any kind (0x242460): a drag of the
# warmup camera still held at the start fired the weapon at the next frame
# and again at its release. Its first five instructions (ten bytes) are a
# jump to warmup_keys and five nops: a button pressed while the cursor is
# shown is marked, and while the cursor is not shown a marked button's kind
# 2 and its release are taken there, the release unmarking it. No base
# relocation under the site.
WARMUP_KEYS_OFFSET = 0x9A8
WARMUP_KEYS_VA = SECTION_VA + WARMUP_KEYS_OFFSET
WARMUP_KEYS_HOOK = (0x25CF60, bytes.fromhex("5556" "8bf1" "8b4c240c" "8b01"), 0, 0xE9)   # on_mouse_key_action: warmup_keys
WARMUP_KEYS_STATE_OFFSET = 0xE30
WARMUP_KEYS_STATE_SIZE = 4

# The camera on the death screen (patch 46, README; client_patch.md section
# 3.43; the rules: docs/notes/death-camera.md). A death leaves the player's
# input handler in the first person mode (on_player_death 0x1F28E0 sets no
# mode): update_camera (0x1F3480) gives the view's update (0x2422B0) the
# eyes' transform (player+44Ch), which it copies (modes 0 and 4); the kits
# and the cursor are shown, so the game world's handler takes every move and
# the wheel. death_camera_026g.asm, in a section of its own (`.srpc`, code,
# the page after the late join's state page, death_camera_va), has two
# entries: death_look (DEATH_LOOK_OFFSET), where WARMUP_LOOK_HOOK's jump goes
# instead of warmup_look, which it goes on to for every move but a dead local
# player's with the kits shown in the first person mode: a drag as patch 31's
# (its state, its drag_end) turns a yaw and a pitch of its own (8 bytes at
# DEATH_CAMERA_STATE_OFFSET of the late join's state page) and is taken (1),
# a move without a button the cursor's; and death_view (DEATH_VIEW_OFFSET),
# which update_camera's `call 0x2422B0` (0x1F364B) calls: while that player
# is dead, in the first person mode, the kits shown and a turn kept, the
# update's own third-person step (0x2422DE) on the identity with that yaw
# and pitch and the distance 0 (the handler's warmup fields borrowed and put
# back), the turn times the eyes' (the head's) transform; a live
# player clears the turn. No base relocation under the site; a dead local
# player sends no input (game_world::tick 0x1F7883).
DEATH_CAMERA_NAME = b".srpc\0\0\0"
DEATH_CAMERA_CHARACTERISTICS = 0x60000020     # code, execute, read (as .srp)
DEATH_CAMERA_SIZE = 0x1000                     # .srpc's one page: `.srpw` follows it
DEATH_LOOK_OFFSET, DEATH_VIEW_OFFSET = 0, 0x200   # the .asm's two entries
DEATH_VIEW_HOOK = (0x1F364B, bytes.fromhex("e860ec0400"), DEATH_VIEW_OFFSET, 0xE8)   # update_camera: death_view
DEATH_CAMERA_STATE_OFFSET = 0xE60             # after the FOV LOD scale's (FOV_LOD_STATE_OFFSET)
DEATH_CAMERA_STATE_SIZE = 8
DEATH_UPDATE_VA, DEATH_THIRD_VA = 0x2422B0, 0x2422DE   # the view's update and its third-person step
# What the code leaves to: warmup_look (twice: not a death screen's move, a
# move that is the cursor's), drag_end, the view's update and its step.
DEATH_CAMERA_EXITS = (WARMUP_LOOK_VA, WARMUP_LIMITS_VA + WARMUP_DRAG_END_OFFSET, DEATH_UPDATE_VA, DEATH_THIRD_VA)
DEATH_CAMERA_EXITS_EXPECTED = sorted([(0xE9, WARMUP_LOOK_VA), (0xE9, WARMUP_LOOK_VA),
                                      (0xE8, WARMUP_LIMITS_VA + WARMUP_DRAG_END_OFFSET),
                                      (0xE9, DEATH_UPDATE_VA), (0xE9, DEATH_THIRD_VA)])

# The order of the archive reads (patch 33, README; client_patch.md section
# 3.31). vostok::resources::hdd_manager_sorting_predicate::operator()
# (0x10BCC0, eax = one query_result, [esp+4] the other, `ret 4`, al = less;
# the name is 0.21d's, 0xCD023, the same code) is called by the std::sort of
# hdd_manager::grab_sorted_queries (0x10BE80) alone: its fifteen calls are in
# the sort's helpers 0x10BF70..0x10C3ED. Its body, 0x1A0 bytes to the next
# function (code to 0x10BE58, int3 after), holds no address (no base
# relocation) and nothing aims inside it; read_order_026g.asm replaces it in
# place, the same entry and convention.
READ_ORDER_VA, READ_ORDER_SIZE = 0x10BCC0, 0x1A0
READ_ORDER_SHA256 = "3c49ddd03438f137bbf3ec844d41ed3c4247d6d815595f1d494a515e112d0aa8"
# A weapon's upgrade window shows the weapon (patch 32, README; client_patch.md
# section 3.30): weapon_preview_026g.asm at WEAPON_PREVIEW_OFFSET of `.srp`,
# after the late join's code (which must end before it) and before `.srpd`;
# its state (three dwords) at WEAPON_PREVIEW_STATE_OFFSET of `.srpd`, after
# the patches' mount's. Five entries, six sites:
# - start_item_upgrade's handler (0x29A870, the delegate {lobby ui, 0x29A870}
#   at 0x281843) calls 0x265DE0 with esi = the lobby's character (lobby
#   ui+694h, survarium::lobby_character, ctor 0x284730) and edi = the item;
#   its `call 0x2845C0` (the lobby camera's setter: xmm0 the look-at height,
#   xmm1 the distance, [esp+4] the side angle, `ret 4`) at 0x265E73 calls
#   start_cam: when a kit of the account's profile (lobby ui+6A0h) holds the
#   item as its weapon 1 (kit slot 7) or, without one, its weapon 2 (slot
#   10), the character shows that kit and the camera gets WEAPON_HEIGHT and
#   WEAPON_DISTANCE; else the original's values;
# - exit_item_upgrade's 0x265F10, `call 0x2845C0` at 0x265F44: exit_cam
#   shows the kit chosen in the lobby (profile+4632h) again after a weapon;
# - setup_profile 0x284B10 (eax = a profile, the character and the kit on
#   the stack, `ret 8`; called by the lobby's refresh 0x222A42, the squad
#   panels' 0x223029 and its own re-call 0x2850C5): its first six bytes
#   (`push ebp` / `mov ebp,esp` / `and esp,-8`) jump to setup_hook, which,
#   while a weapon is shown and the character has an upgrade window open
#   (+D515h, which only the lobby's own character ever has), gives it the
#   weapon's kit;
# - the camera tick's least distance (0x283EF0, the float 1.5 at 0xA7AFA4,
#   `movss xmmN,[0xA7AFA4]` at three sites: the wheel, the step and the last
#   clamp) is a call of min_step or min_final and three nops: 1.5, or
#   NEAR_MIN from a weapon's window until the camera is back at 1.5. The
#   address's three base relocations are disabled;
# - the lobby character's update (0x2849F0) gives its player the window's
#   mode at every frame (`mov [ecx+0F49Ch],al` at 0x284A33; the player's
#   ctor clears it at 0x1F1BFB, nothing else writes it): set, the player is
#   drawn for the decal editor, its skeleton in the bind pose (mt_draw
#   0x2474A0 at 0x2476F8: the T-pose) and its held item 10 m under it
#   (0x24778F), with another transform (insert 0x1F30E2). The six bytes are
#   a call of weapon_draw_026g.asm's player_mode (WEAPON_DRAW_OFFSET of
#   `.srp`, in the first page after the forward effect's scrolling) and a
#   nop: 0 while a weapon is shown (the state's mode), the window's mode
#   otherwise.
# The code finds its own address (call/pop), so a rebase changes nothing.
WEAPON_PREVIEW_OFFSET = 0x2E50
WEAPON_PREVIEW_VA = SECTION_VA + WEAPON_PREVIEW_OFFSET
WEAPON_PREVIEW_STATE_OFFSET = 0xE00
WEAPON_PREVIEW_STATE_SIZE = 24                 # its three dwords, then weapon_stash_026g.asm's three
WEAPON_PREVIEW_LEAST = 0xA7AFA4
WEAPON_PREVIEW_HOOKS = (
    (0x265E73, bytes.fromhex("e848e70100"), 0x00, 0xE8),            # start_item_upgrade's camera: start_cam
    (0x265F44, bytes.fromhex("e877e60100"), 0x6D, 0xE8),            # exit_item_upgrade's camera: exit_cam
    (0x284B10, bytes.fromhex("558bec83e4f8"), 0xAD, 0xE9),          # setup_profile: setup_hook
    (0x284081, bytes.fromhex("f30f100da4afa700"), 0xE6, 0xE8),      # the wheel's least: min_step
    (0x28428C, bytes.fromhex("f30f100da4afa700"), 0xE6, 0xE8),      # the step's least: min_step
    (0x284316, bytes.fromhex("f30f1005a4afa700"), 0xFC, 0xE8),      # the last clamp's least: min_final
)
WEAPON_PREVIEW_RELOCS = [0x284085, 0x284290, 0x28431A]
WEAPON_DRAW_OFFSET = 0xD20
WEAPON_DRAW_VA = SECTION_VA + WEAPON_DRAW_OFFSET
WEAPON_DRAW_HOOK = (0x284A33, bytes.fromhex("88819cf40000"), 0, 0xE8)   # the player's decal pose: player_mode
# The commit_suicide key kills one's own player (patch 34, README;
# client_patch.md section 3.32). commit_suicide is game action 1Fh (action
# table 0xA74AF0, entry 0xA74D20, default key kK); a key bound to it reaches
# the player input handler's event buffer (+98h) like any other (0x23FEAC),
# but the input builder (player_input_handler::process_first_person_mode,
# 0x241250) asks action_present 0x242460(handler, action, &state) for 35
# actions and 1Fh is not one of them: bit 24 of the action mask
# (handler+33Ch) is never set. The match simulation reads it in
# base_player::process_quick_slots (0x2D8BC1, called by the input apply
# 0x2D86E0 for every player's input): a hit on the player's own head bone
# "brain" (vtable+20h = 0x2D9AE0 -> hit_impl 0x2D9B90), the attacker the
# player; on_player_killed (0x2F4F38) counts a death and a suicide for it,
# no kill. The builder's last call (spot, 0x242196, ebx = the handler) is a
# Rust `call` hook, suicide_key::on_spot (the arena's block patches.code,
# rust/patches, the framework's example patch, docs/notes/rust_patches.md):
# before the spot query it asks action_present (0x242460) for 1Fh and sets
# bit 24 when the key went down (state 0, as spot does), then does the spot
# query the site wanted and returns its answer. No base relocation under the
# site. (Until the Rust framework it was NASM at 0x980 of `.srp`; the port
# before it, a Rust crate per patch with a tail jump.)
ACTION_STATE_VA = 0x242460                    # player_input_handler: the state of a buffered action

# The Rust exe patches (docs/notes/rust_patches.md): the module names, one
# per TOML (for link_026g.py --sizes), and the hooks the patches declare in
# it, applied like the other arena hooks (site VA, the bytes replaced, the
# arena symbol of the generated entry, the opcode).
RUST_PATCHES = tuple(toml["name"] for toml in hooks_026g.read_tomls())


def rust_hooks(original):
    """The Rust hooks' installs (site VA, the bytes replaced, the entry's
    arena symbol, the opcode): a span hook's bytes read from `original`, the
    exe being built from."""
    read = hooks_026g.original_reader(original)
    return tuple((va, old, symbol, opcode) for va, old, symbol, opcode, _owner in hooks_026g.installs(exe=read))


def __getattr__(name):
    """RUST_HOOKS, for the checks: rust_hooks() of the exe SRP_026G_EXE names."""
    if name == "RUST_HOOKS":
        path = os.environ.get("SRP_026G_EXE")
        if not path:
            raise AttributeError("RUST_HOOKS needs the original exe: SRP_026G_EXE")
        return rust_hooks(open(path, "rb").read())
    raise AttributeError(name)

# The scoreboard (Tab) shows the places in the match and no place whose
# player left (patch 39, README; client_patch.md section 3.36; the rules:
# docs/notes/tab-list-left-places.md). A place is in the match from the
# server's 0xE9 for it to its 0xEA (a leave by the player's own word, 0xE0,
# or a link the server gave up on): kd_stats_rule's `online` (+114h + 8 x
# place; set by on_player_entered_match 0x3239F0, cleared by
# on_player_left_match 0x323A00, which game_world_core's 0x2D2040 calls for
# every rule; in every game at the same tick, and in the worlds), copied
# into the hud's state (hud+0C0h + 8 x place) every tick by
# fill_match_hud_state 0x323980. The game made the scoreboard's rows once,
# in the hud's initialize_match (0x234880, again after a late join's
# install: rebuild_lists), for every place below the count, and greyed a
# row whose place was not in (set_online_player); none was ever taken out,
# so a player who left kept a grey row and one who came back as a new
# place had two. The code is the late join's (late_join_026g.asm), its one
# dword at TAB_LIST_STATE_OFFSET of the late join's state page:
# - 0x23537A in 0x234880's first loop, where a row goes into the
#   scoreboard's array by SetElement(data, place, row) (`lea eax,[esp+58h]`
#   / `push eax` / `mov eax,[esp+138h]` / `push ebx` / `push eax` / `call
#   edx`), calls tab_row: the row goes in at the array's end (PushBack, the
#   interface's vtable+3Ch) when the place is in the match, and the dword
#   becomes -1 (a list made);
# - 0x23B421 in update_ui (0x23B200), `cmp byte [edi+0BAh],0` before the
#   rows are refreshed, calls tab_shown: while the list is shown, when the
#   places in the match are not the mask the dword holds, the three lists
#   are made again (rebuild_lists; lists_hook ends that run as it ends the
#   install's) and the dword is that mask; it sets the flags as the `cmp`;
# - the refresh's loop body (0x23B447..0x23B476: set_online_player and
#   list_update_player for every place below the count) is rewritten in
#   place: a place not in the match is left alone. Same calls, same
#   arguments otherwise (list_update_player's place from ebx's low byte,
#   which it reads alone, instead of the byte stored at [esp+14h]).
# No site holds an address: no base relocation under them. Nothing a match
# computes: the hud's lists only.
TAB_LIST_STATE_OFFSET = 0xE40
TAB_LIST_STATE_SIZE = 4
TAB_ROW_OFFSET, TAB_SHOWN_OFFSET = 2480, 3600
TAB_LIST_HOOKS = (
    (0x23537A, bytes.fromhex("8d442458" "50" "8b842438010000" "53" "50" "ffd2"), TAB_ROW_OFFSET, 0xE8),
    (0x23B421, bytes.fromhex("80bfba00000000"), TAB_SHOWN_OFFSET, 0xE8),
)
# The first bytes of the two entries: `call get_state` (to 2304) and `or
# dword [eax+TAB_LIST_STATE_OFFSET],-1`; `cmp byte [edi+0BAh],0`.
TAB_ROW_START = b"\xE8" + struct.pack("<i", 2304 - TAB_ROW_OFFSET - 5) + b"\x83\x88" + \
    struct.pack("<I", TAB_LIST_STATE_OFFSET) + b"\xFF"
TAB_SHOWN_START = bytes.fromhex("80bfba00000000")
TAB_LIST_LOOP_VA = 0x23B447
TAB_LIST_LOOP = (TAB_LIST_LOOP_VA,
                 bytes.fromhex("0fb6c3" "8bd0" "0fb684c7c0000000" "69d2fc450000" "03ca" "50" "8d4148" "83c108"
                               "57" "e887f3ffff" "8b4c2414" "51" "e8bdb8ffff" "fec3" "ebb5"),
                 # movzx eax,bl / imul edx,eax,45FCh / movzx eax,byte [edi+eax*8+0C0h] / test al,al /
                 # jz .next / add ecx,edx / push eax / lea eax,[ecx+48h] / add ecx,8 / push edi /
                 # call 0x23A7F0 / push ebx / call 0x236D30 / .next: inc bl / jmp short 0x23B42C / nop / nop
                 bytes.fromhex("0fb6c3" "69d0fc450000" "0fb684c7c0000000" "84c0" "7415" "01d1" "50" "8d4148"
                               "83c108" "57" "e885f3ffff" "53" "e8bfb8ffff" "fec3" "ebb7" "9090"))

# The player's own row in the Tab list is gold (patch 43, README;
# client_patch.md section 3.40; the rule: ../docs/notes/tab-list-left-places.md).
# hud.swf's PlayerListRenderer gives the row whose `isLocal` is set the name
# colour Colors.PLAYER (0xF1E545), the one CombatLogItem gives the kill
# list's own name (who_team / victim_team 2); PlayerList sets `isLocal` on
# the row of the name `root.set_local_player` gave. In a match only
# game_world::on_activate (0x1F9340) gives it: after chat_handler::set_mode
# it makes the name as the rows have it ("[clan] name", or the name) and
# calls chat_handler::set_local_player_name (0x209AC0) on the hud, but only
# while the journal records: `mov eax,[0A8394Ch]` / `test eax,eax` / `jz` /
# `mov ecx,[0A83948h]` / `cmp dword [ecx],1` / `jnz`, the journal's flag
# and its use (1 record). Patch 14 (revision 4) turned the journal off by
# default, so no hud has had its player's name since. TAB_SELF makes
# `test eax,eax` / `jz` a `jmp short 1F9487h` past the two tests: the name
# is given whatever the journal does. The two addresses (and their base
# relocations) are not under the two bytes. Every full build, as JOURNAL_DEFAULT.
# The hud's lists and chat only.
TAB_SELF = (0x1F9478, bytes.fromhex("85c0"), bytes.fromhex("eb0d"))

# A held weapon shown while its player aims keeps its body (patch 91,
# README; client_patch.md section 3.60; the rules: ../docs/notes/weapon-show.md).
# weapon::on_show (0x243DD0) took the aimed branch when the weapon's "aim"
# byte (+181Eh, the world's: weapon::deserialize 0x244A40 sets it from a
# snapshot) was set and the weapon is the local player's in first person
# (0x243940): it added the optic's aimed model, set +181Dh (the aimed model
# is in the scene) and so skipped the body's add (0x243EFD), whatever the
# optic's hide_weapon_on_aim (+11Ch). With an optic that shows the gun when
# aimed (+11Ch 0) nothing added the body back: releasing the aim
# (weapon::draw 0x244FAD -> remove_aimed_rifle_scope_from_scene 0x244C50)
# restores the body only for +11Ch 1. A world load (a resync, a late join)
# shows the weapon while +181Eh is set: only the sight and the hands were
# drawn. `je 0x243EC7` at 0x243E32 becomes `jmp 0x243EC7` / nop: on_show
# always adds the idle sight and the body, as 0.10b's on_show did, and the
# next weapon::draw (0x244F80) aims through add_aimed_rifle_scope_to_scene
# (0x244B60), which takes the body out for +11Ch 1 alone. Only the
# scene's models and +181Dh change (+181Eh is never written); no base
# relocation is under the six bytes and nothing aims inside them. Every
# full build, as TAB_SELF. What is drawn only.
WEAPON_SHOW = (0x243E32, bytes.fromhex("0f848f000000"), bytes.fromhex("e99000000090"))

# The lobby's `stage.focus` reads null for a focused object whose
# ActionScript object is gone (patch 52, README; client_patch.md section
# 3.49). Scaleform's flash.display.Stage focus getter (0x771FA0, called by
# the thunk 0x7750F0 of the ThunkInfo "focus" at 0x8F1E40) takes MovieImpl's
# focused InteractiveObject (the weak pointer of
# FocusGroups[FocusGroupIndexes[0]], +3B64h + 40h x byte [+3F54h], locked
# by 0x514370), its AvmInteractiveObj (vtable +4 of the interface at byte
# [obj+41h], minus 1Ch) and that one's ActionScript object (the raw pointer
# +8, else the collectible one +4, bit 0 cleared), and reads its traits'
# type ([as3+14h], +3Ch) for an InteractiveObject (12h, or 17h and up)
# with no test of either: Scaleform's Stage::focusGet tests only the
# focused object (its FocusManager.getFocus tests the ActionScript object
# too). That object is gone when the collector freed it while the display
# object lived on (~DisplayObject calls AvmDisplayObj::ReleaseAS3Obj), and
# AS3's removeChild leaves the focus where it is (only an unload resets it,
# MovieImpl::ResetFocusForChar). The lobby then died at 0x77200B on a key:
# lobby_menu's key handler (0x26CB80) invokes root.has_active_window, which
# reads `stage.focus is DropdownMenu` (2026-10-22 04:44, a second after an
# upgrade). The 46 bytes from 0x771FED (`test eax,eax` after the
# interface call) to 0x77201B (the result's assignment) become the same
# reads with both tests: no AvmInteractiveObj or no ActionScript object
# goes to 0x772031, the null result, as a focused object of another type
# did. ecx only, as before; eax at 0x77201B the object, as before. Every
# jump of the old bytes stayed inside them; nothing else aims inside, and
# no base relocation is under them. Every full build. The lobby and the
# hud only.
STAGE_FOCUS = (0x771FED,
               bytes.fromhex("85c0" "7405" "83c0e4" "eb02" "33c0" "8b4808" "85c9" "7404" "8bc1" "eb03" "8b4004"
                             "a801" "7401" "48" "8b4814" "8b493c" "83f912" "7405" "83f917" "7c16"),
               # test eax,eax / jz null / mov ecx,[eax-14h] / test ecx,ecx / jnz +3 / mov ecx,[eax-18h] /
               # and ecx,-2 / jz null / mov eax,ecx / mov ecx,[eax+14h] / mov ecx,[ecx+3Ch] / cmp ecx,12h /
               # jz 0x77201B / cmp ecx,17h / jge 0x77201B / null: jmp short 0x772031 / 7 nop
               bytes.fromhex("85c0" "7421" "8b48ec" "85c9" "7503" "8b48e8" "83e1fe" "7412" "89c8" "8b4814" "8b493c"
                             "83f912" "740e" "83f917" "7d09" "eb1d") + b"\x90" * 7)

# Spotting (patch 38, README; client_patch.md section 3.35; the rules:
# ../docs/notes/spotting.md). base_player::apply_new_input (0x2D86E0): an
# input with bit 28 (the spot key going down, 0x2421A6) and the spotter's
# m_time_left_to_try_spot_in_ms (+34Ch) at 0 casts a 100 m ray (0xA710D0)
# from the eye and spots the first spottable hit before a walkable and
# hittable stop. The field became 3000 at every attempt, hit or miss
# (0x2D87C1), and the player's tick counts it down (0x2DAB44, to 0 at the
# least); a spawn sets it to 0 (0x2DAE6B) and the snapshot keeps it
# (base_player::serialize 0x2DB34D, read back at 0x2DB95C). SPOT_COOLDOWN
# puts 0 there: the field is 0 at every tick, in every game, and in every
# snapshot. A number, not an address: no base relocation under it.
# The walk asks a spottable hit's owner (+0Ch of the hit object) for a
# player (slot 2) and then for a geometry (slot 1, at 0x2D8A0F). A
# battery's rigid body (group 0A18h, 0x301CF5) has the owner
# victory_item_core+58h, vtable 0xA13EEC (written by the two constructors
# alone, 0x30011D and 0x3004EA), whose slots 1 and 2 return 0; its use box
# answers through slot 1. SPOT_HOOK jumps to spot_026g.asm at SPOT_OFFSET
# of `.srp` (after the shared upgrades' code, before the pickup code): for
# that owner it spots the battery through its spottable interface at +54h
# (vtable 0xA13EE0: set_spotted 0x301C40, can_be_spotted_by 0x301CA0) and
# ends the walk as a spotted target does (0x2D888F); any other owner goes
# on to 0x2D8A14 as before. No base relocation under the site.
SPOT_COOLDOWN = (0x2D87C1, bytes.fromhex("c7874c030000b80b0000"), bytes.fromhex("c7874c03000000000000"))
SPOT_OFFSET = 0x7C0
SPOT_VA = SECTION_VA + SPOT_OFFSET
SPOT_HOOK = (0x2D8A0F, bytes.fromhex("8b0e8b490c"), 0, 0xE9)
SPOT_OWNER_VTABLE = 0xA13EEC                  # victory_item_core at +58h: its rigid body's owner
SPOT_EXITS = (0x2D888F, 0x2D8A8F, 0x2D8A14)   # spotted; not spotted, the next hit; the original

# Fall speed (patch 72, ../docs/notes/fall_damage.md "Split ticks"). The
# world steps to each precise input (a change of the fire bit, 0xDD) inside
# a tick (tick_events 0x2D21E0), and every step ticks the physics
# (0x2D1D00 -> bullet_physics_world::tick 0x75550 -> updateActions 0x9B810,
# dt = the step's ms / 1000). bullet_character_controller::updateAction
# 0x7B730 runs player_step 0x7BDA0 for a step shorter than 0.05 s when the
# body moved, as a falling one has; player_step moves it dt / 0.05 of a tick
# (0x7C734), but step_down's free fall (0x7E6E6..0x7E75C) took a whole tick
# of gravity, g x 0.05 (+2C708h x +2C704h, the constant): a tick cut in k
# parts gave k ticks of speed, and the landing (0x2D8220) took that speed.
# FALL_GRAVITY rewrites that free fall in place: g x min(dt, 0.05), dt read
# from player_step's frame (step_down's ebp -> player_step's ebp, +8; step_down
# has that one caller, 0x7C4F6, and both have `push ebp / mov ebp,esp`). A
# whole tick's dt is 0.05 (the float updateAction compares with +2C704h) and
# minss gives the constant back: unsplit ticks are bit-identical. The three
# products by xmm3 between the two sites (0x7E6F6..0x7E720, which hold the
# three relocated addresses of the up axis) are the original's; the first
# site jumps to the dt load in the second and back. eax and ecx are free
# there (written at 0x7E762 and 0x7E75C before any read). No base relocation
# under either site; no branch lands inside them.
FALL_GRAVITY = (
    (0x7E6E6, bytes.fromhex("f30f108308c70200" "f30f109b04c70200"),
     bytes.fromhex("8d8304c70200" "f30f104004" "eb59" "cccccc")),
    (0x7E720, bytes.fromhex("f30f109b70c70200" "f30f5cd9" "f30f108b74c70200" "f30f5cca" "f30f118b74c70200"
                            "f30f108b78c70200" "f30f5cc8" "f30f119b70c70200" "f30f118b78c70200"),
     bytes.fromhex("f30f10586c" "f30f5cd9" "f30f104870" "f30f5cca" "f30f114870" "f30f104874" "f30f5cc8"
                   "f30f11586c" "f30f114874" "eb10" "8b4d00" "f30f105908" "f30f5d18" "eb9c" "cccc")),
)

# "Watch replay" (patch revision 14, client_patch.md section 3.27). The
# lobby movie's ExternalInterface call video_tutorial_link_clicked is the
# delegate {lobby ui, 0x28CDA0} (registered at 0x283053; the name is entry
# 106 of the callback names at 0x9C56C8): thiscall, one argument, `ret 4`,
# al = 1. It takes the lobby client from its owner (vtable+3Ch, in eax at
# 0x28CDB6) and called ShellExecuteA(NULL, "open", client+1E6144h, NULL,
# NULL, SW_SHOWNORMAL) (the delay-loaded import at 0xAEFE74), then
# enable_element("root.statusPanel.videoTutorial", 1) (0x22A660). The buffer
# is what the server's 9E sent, UTF-8 with its zero (0x271426..0x271444):
# any text the server sent was opened as a file, a program or a URL. `push 1`
# / `push 0` / `push 0` at 0x28CDB6 becomes a jump to replay_link
# (replay_link_026g.asm at REPLAY_LINK_VA: in the first page of `.srp`,
# after the upload code, the default server's stub and its address, 0x21D
# at the most, and before the patches' mount at MOUNT_PATCHES_OFFSET; the
# page has no other room for it; build() checks that they do not overlap):
# only a link `replay <8 hex> <32 hex> <64 hex> <1..10 digits>` (lower-case
# hex) starts <this exe's folder>\updater_skhid.exe with the command line
# `updater_skhid.exe <link>` (CreateProcessW); anything else does nothing,
# and the element is not enabled. The rest of the original's body stays,
# never reached. No base relocation is under the hook.
REPLAY_LINK_OFFSET = 0x220
REPLAY_LINK_VA = SECTION_VA + REPLAY_LINK_OFFSET
REPLAY_LINK_HANDLER = 0x28CDA0
REPLAY_LINK_HOOK = (0x28CDB6, bytes.fromhex("6a016a006a00"), 0, 0xE9)
REPLAY_LINK_BUFFER = 0x1E6144                 # in the lobby client

# The Knowledge database callback (0x28CAB0) copies this fixed prefix to
# its stack, appends ru/en and /Knowledgebase/List, then ShellExecuteA's it.
# Keep the native browser call and stack cleanup, but skip both appends:
# the wiki chooses its language from the browser and its own language switch.
KNOWLEDGE_BASE_URL = b"https://026g.skhid.link/wiki"
KNOWLEDGE_BASE_ORIGINAL = b"https://support.survarium.com/index.php?/\0"
KNOWLEDGE_BASE = (
    (0xA0A7A8, KNOWLEDGE_BASE_ORIGINAL,
     (KNOWLEDGE_BASE_URL + b"\0").ljust(len(KNOWLEDGE_BASE_ORIGINAL), b"\0")),
    (0x28CBA3, bytes.fromhex("e8d3a31900"), b"\x90" * 5),
    (0x28CBB7, bytes.fromhex("e8bfa31900"), b"\x90" * 5),
)

# One upgrade on several items (patch 30, client_patch.md section 3.28):
# shared_upgrades_026g.asm at SHARED_UPGRADES_OFFSET of `.srp`, after the
# patches' mount (which ends at +6C3h) and before the pickup code; its four
# entries at fixed offsets, one for each site:
# - drop_upgrade_item's handler 0x29A9D0 (the delegate {lobby ui, 0x29A9D0},
#   `mov eax,0x29a9d0` at 0x2818FA; Flash drop_upgrade_item(<item>,
#   <upgrade>): the upgrade window's drop and cancel, and on_item_sold) at
#   0x29AAD5: `push edi` / `push ecx` / `call 0x2708C0` (the only call of
#   0x2708C0, which writes `46 02 <kit> <upgrade>`) -> a call of
#   drop_upgrade, which writes `46 02 <kit> <upgrade> <item>`, and two nops;
# - process_succeeded_inventory_action 0x275520, case 2 (`83 46 02`) at
#   0x275756: `mov ecx,[eax]` / `push ecx` / `add eax,4` / `push ebp` /
#   `mov [esi+4],eax` / `call 0x275220` -> a jump to detach_answer, which
#   reads `<upgrade> <item>`, and ten nops;
# - case 1 (`83 46 01`), detach_first, at 0x275589: `push edi` / `push
#   ebp` / `call 0x275220` -> a call of detach_first (the item the answer
#   puts the upgrade on, in ebx) and two nops;
# - process_detach_upgrade 0x275220 (called from those two sites alone), at
#   0x27534D: `call 0x277CF0` (find_if: the first item of the inventory
#   holding the upgrade, the only call of it) -> a call of find_host, the
#   item of the id its callers now give it as a third argument.
# No site holds an address: no base relocation is under them.
SHARED_UPGRADES_OFFSET = 0x700
SHARED_UPGRADES_VA = SECTION_VA + SHARED_UPGRADES_OFFSET
SHARED_UPGRADES_HOOKS = (
    (0x29AAD5, bytes.fromhex("5751" "e8e45dfdff"), 0x00, 0xE8),                          # drop_upgrade
    (0x275756, bytes.fromhex("8b08" "51" "83c004" "55" "894604" "e8bbfaffff"), 0x60, 0xE9),  # detach_answer
    (0x275589, bytes.fromhex("5755" "e890fcffff"), 0x80, 0xE8),                          # detach_first
    (0x27534D, bytes.fromhex("e89e290000"), 0x90, 0xE8),                                 # find_host
)
# The first bytes of each entry, as the .bin must have them.
SHARED_UPGRADES_ENTRIES = {0x00: bytes.fromhex("8b5508"), 0x60: bytes.fromhex("8b08"),
                           0x80: bytes.fromhex("535755"), 0x90: bytes.fromhex("8b542458")}
DROP_UPGRADE_HANDLER = 0x29A9D0
PROCESS_DETACH_UPGRADE = 0x275220
INVENTORY_ACTION = 0x275520

# The late join (client_patch.md section 3.11). Its code is the second page
# of `.srp` and what follows it: 4096 bytes of late_join_026g.asm, whose
# entries are at fixed offsets, then the C code. The first page stays what
# it is without the late join (the upload code, the default-server stub and
# the address), so no address in the code depends on the server given at
# patch time. Its state is one zeroed, writable page of its own, `.srpd`,
# on the page after the code.
LATE_JOIN_OFFSET = 0x1000
LATE_JOIN_VA = SECTION_VA + LATE_JOIN_OFFSET
LATE_JOIN_ASM_SIZE = 0x1000
LATE_JOIN_STATE_NAME = b".srpd\0\0\0"
LATE_JOIN_STATE_SIZE = 0x1000
LATE_JOIN_STATE_CHARACTERISTICS = 0xC0000080   # uninitialized data, read, write

# base_game_statistics_handler (the network client's, +1EE8D8h; vtable
# 0xA0755C) keeps the scoreboard's score of each place, `m_scores[20]` at
# +31F0h, which the world does not save. on_event (0x26DB80, slot +44h, on
# every statistics event) adds the event's points to any place's score;
# recalculate_scores (0x26DC30), which deserialize (0x26DB60, slot +34h)
# runs after every state a game loads, works them out again from the
# statistics, and start_match (0x26D8D0, slot +3Ch) tells the hud all of
# them, both for as many places as the handler's count, +3240h. start_match
# copies that count from the roster of the start (options+577B5h); the late
# join raises the roster and the shared statistics' live count (+31E8h) but
# not this one. So a newcomer's score was never worked out again: every
# rollback over its events added them once more (2.7 to 4 times its score
# in a match of ten, seen live), and its hud cell kept the score a player
# of an earlier match had had in that place until its first. With twenty, every
# place is worked out again at every state loaded, and start_match tells
# the hud a 0 for every place at a match's start. Places with no player
# have zero statistics: a score of 0, told once. The live count beside it
# stays the roster's. `mov al,[edx+577B5h]` -> `mov al,14h`.
STATISTICS_START_MATCH_VA = 0x26D8D0
STATISTICS_COUNT = (0x26D8F3, bytes.fromhex("8a82b5770500"), b"\xb0\x14" + b"\x90" * 4)

# A weapon's random state (weapon_core+5BCh: boost's minstd_rand, x * 48271
# mod 2^31-1, drawn from by its shots' spread alone, 0x2E1D50 and 0x2E2120)
# is 1 from the constructor (0x2DF429), comes out of a world in deserialize
# (0x2E34C1), and is set by nothing else: not at a respawn, which keeps the
# kit's weapon objects. A place has a player object for each of its three
# kits, and a world holds only the current one's (0x3218E0). So a game that
# loaded a world during a match (a newcomer, a resynced game) had every other
# kit's weapons at 1 where the games that played them had drawn from them,
# and a respawn into such a kit put that game out of step at once (seen live:
# match 0x1ce45cbe, six desyncs of its newcomers on the kit timeline); so
# did a rollback over a respawn into another kit, which loads the current
# kit's player alone. The respawn rule's spawn (0x316DA0, slot +5Ch of every
# respawn rule; its tick 0x316820 spawns every player, a match's first spawn
# too, in every game and in every replay of a tick) makes the new kit
# current (0x316DE3), initialises its player (vtable+1Ch: the inventory from
# the profile, a weapon in hand) and puts it into the world (vtable+20h).
# Between the two, at 0x316EA0 (`mov edx,[esi]` / `mov eax,[edx+20h]`, the
# load of that call; esi = the player, ebp = the spawn's time), a call into
# weapon_random_hook (late_join_026g.asm) gives each weapon of the player's
# inventory a state from its place, kit, slot and the spawn's time, in
# 1..2^31-2, then does the two instructions. A weapon is an item whose
# vtable+70h is weapon_core's bind 0x2E30F0, which only the vtables of
# weapon_core and the client's weapon hold.
WEAPON_RANDOM_VA, WEAPON_RANDOM_OFFSET = 0x316EA0, 3712
WEAPON_RANDOM_HOOK = (WEAPON_RANDOM_VA, bytes.fromhex("8b168b4220"), WEAPON_RANDOM_OFFSET, 0xE8)
WEAPON_SPAWN_VA = 0x316DA0                    # the respawn rule's spawn
WEAPON_BIND_VA = 0x2E30F0                     # weapon_core's bind, vtable+70h of its item
WEAPON_RANDOM_FIELD = 0x5BC                   # weapon_core's random state
WEAPON_RANDOM_MODULUS = 0x7FFFFFFF


def weapon_random_state(place, kit, slot, time):
    """The state weapon_random_hook gives a weapon at a spawn."""
    h = ((((place & 0xFF) << 16 | (kit & 0xFF) << 8 | slot) * 0x85EBCA6B)
         ^ (time * 0x9E3779B1)) & 0xFFFFFFFF
    return (h ^ h >> 15) % (WEAPON_RANDOM_MODULUS - 1) + 1


# The same spawn, before the initialize: at 0x316E57 (`mov edx,[esi]` /
# `mov edx,[edx+1Ch]`, the load of the initialize's call; esi = the new
# kit's player, [esp+10h] the kit before, [esp+18h] the new kit, as
# 0x316E02/0x316E06 put them) a call into kit_state_hook
# (late_join_026g.asm). Each weapon of the inventory gets -1 (the
# constructor's) as m_last_tick_time_in_ms (+5A4h): only the weapon in hand
# is ticked, and the activation (0x2E2A30) the initialize runs for the
# weapon it puts in hand sets the field to the spawn's time before any tick.
# At a spawn into another kit than the one before, the fields a weapon keeps
# through deaths and that the initialize and the activation do not set
# again, or set only for the weapon in hand, get the constructor's values
# (0x2DF0D0): KIT_STATE_FIELDS. A respawn into the same kit keeps them: that
# kit's player is in every world.
KIT_STATE_VA, KIT_STATE_OFFSET = 0x316E57, 3168
KIT_STATE_HOOK = (KIT_STATE_VA, bytes.fromhex("8b168b521c"), KIT_STATE_OFFSET, 0xE8)
KIT_STATE_TICK_FIELD = 0x5A4                  # m_last_tick_time_in_ms, -1 at every spawn
# (weapon offset, size, the constructor's value) set at a spawn into another kit
KIT_STATE_FIELDS = (
    (0x5A8, 4, 0xFFFFFFFF),                   # m_ammo_switch_time_in_ms
    (0x5B1, 1, 0), (0x5B2, 1, 0),             # m_current_ammo_id, m_next_ammo_id
    (0x5E5, 1, 0), (0x5E6, 1, 0),             # m_ammo_in_magazine, m_fire_queue_type
    (0x5E8, 1, 0),                            # m_aimed
    (0x5EC, 4, 6),
    (0x5F1, 1, 0),                            # m_is_round_chambered
    (0x5F5, 1, 0),                            # m_need_to_auto_reload
    (0x5FC, 4, 0), (0x600, 4, 0), (0x604, 4, 0), (0x608, 4, 0),   # 0.0
    (0x60C, 4, 0xFFFFFFFF),
)


# A game's first world of a match (game_world_core::deserialize 0x2D0B70,
# the block 0x2D0FE3..0x2D105C while the world is still frozen: every game's
# at a match's start, a newcomer's, a game's that comes back) gives each
# place of the world an input and a kit choice at the world's time: `call
# 0x2d4ab0` at 0x2D1050 (the current player's input, +32Ch/+330h, into the
# place's input list, core+2FCCh) and `call 0x2d4b50` at 0x2D1055 (a kit
# choice into core+89D8h), edi -> {core, the world's time, place}. Both put
# their entry at the newest end of the list when it has none of that time,
# even after newer entries (inputs and kit choices relayed between the
# server's 0xE5 and the world), where every other writer keeps the lists in
# time order (the network's 0x1FB42A and 0x1FBF00 insert through 0x2D4850).
# And the kit choice was the kit in use (player+115h), not the one the world
# has chosen for the next spawn (place+0Dh, read from the world at 0x2D2A37).
# Each tick 0x321790 sets the place's next kit from the newest entry at or
# before the tick, looked for from the newest end: the world's choice, or a
# choice relayed for later, was replaced by the kit in use, and the newcomer
# respawned that player in another kit than the other games (reported by
# the analysis of the matches after 0x1ce45cbe for joins that were never let
# in). The two calls go to first_load_input and first_load_kit
# (late_join_026g.asm): the same entries, the kit choice being place+0Dh,
# put before the oldest newer entry (0x2D4850), an entry of that time given
# the value as before. With no newer entry the lists end as the game's own
# left them, but for the kit choice's value.
FIRST_LOAD_INPUT_HOOK = (0x2D1050, bytes.fromhex("e85b3a0000"), 3840, 0xE8)
FIRST_LOAD_KIT_HOOK = (0x2D1055, bytes.fromhex("e8f63a0000"), 3920, 0xE8)
FIRST_LOAD_DESERIALIZE_VA = 0x2D0B70

# The replay's seek (game 0x1EE2E0: esi = game, edi = the target; it starts
# from the replay reader [game+53C8h], which only -match_replay sets, and its
# callers are the replay movie's slider and the replay's queued seeks) sets
# the clock to a key frame, re-simulates to the target and sets the clock to
# the target; a weapon's delayed sounds (weapon+9FCh, SOUND_QUEUE) keep the
# times of the old timeline, and the re-simulated ones all come due at once:
# sounds late by the seek's distance, a burst, a queue that fills. For a
# user seek only (`test bl,bl` / `jnz 1EE3B5h`, a key frame in play takes the
# jnz), `lea eax,[edi-3E8h]` at 0x1EE3A3 becomes a jump to seek_clear
# (replay_seek_026g.asm at SEEK_CLEAR_VA, after the pickup code): every
# client weapon (item vtable 0xA0422C) in the 23
# inventory slots of each place's three kits has its queue's functors
# destroyed (0x245C50, which plays nothing), the queue emptied, and its fx
# history (weapon+828h, the duplicate check) emptied, as ~weapon 0x243790
# does; then the lea, and back. A live game never runs the seek.
SEEK_CLEAR_OFFSET = 0xA00
SEEK_CLEAR_VA = SECTION_VA + SEEK_CLEAR_OFFSET
SEEK_CLEAR_HOOK = (0x1EE3A3, bytes.fromhex("8d8718fcffff"), 0, 0xE9)

# Rewritten in place: (address, the original's bytes, the new ones). The
# actual roster count stays what resources and the countdown read; only
# what must survive a growing roster takes the fixed capacity.
LATE_JOIN_PATCHES = (
    # base_match_client::set_players_count, both implementations (the
    # journaling vtable 0xA083D8+34h and the live match_client's
    # 0xA08864+34h): the ED/EE input masks have three bytes from the first
    # connection, whatever the count. `mov eax,3` and nops.
    (0x279610, bytes.fromhex("0fb6442404489983e20703c2c1f803fec0"), bytes.fromhex("b803000000") + b"\x90" * 12),
    (0x27A2E0, bytes.fromhex("0fb6442404489983e20703c2c1f803fec0"), bytes.fromhex("b803000000") + b"\x90" * 12),
    # kd_stats_rule: all twenty entries of its array, which the constructor
    # zeroes. `mov dl,[ecx+577B5h]` -> `mov dl,14h`.
    (0x3237B4, bytes.fromhex("8a91b5770500"), b"\xb2\x14" + b"\x90" * 4),
    # base_game_statistics_handler::start_match: all twenty scores
    # (STATISTICS_COUNT above).
    STATISTICS_COUNT,
    # shared_statistics::serialize and deserialize: twenty records, not the
    # live count (which rewards and event handlers go on reading).
    # `cmp [esi+31E0h],bl` / `cmp bl,[esi+31E0h]` -> `cmp bl,14h`.
    (0x2F5ACF, bytes.fromhex("389ee0310000"), b"\x80\xfb\x14" + b"\x90" * 3),
    (0x2F5AF4, bytes.fromhex("3a9ee0310000"), b"\x80\xfb\x14" + b"\x90" * 3),
    (0x2F5BE6, bytes.fromhex("389ee0310000"), b"\x80\xfb\x14" + b"\x90" * 3),
    (0x2F5C03, bytes.fromhex("3a9ee0310000"), b"\x80\xfb\x14" + b"\x90" * 3),
)
# Calls and jumps into the new code: (address, the original's bytes, the
# entry's offset in the code, 0xE8 call or 0xE9 jmp); the rest of the
# original's bytes become nops.
# The camera's projection (patch 22, FOV_PATCHES below): its first
# instruction, `fld dword [ecx+8Ch]`, jumps to fov_projection, which does the
# whole projection with the angle of the field-of-view setting.
FOV_PROJECTION_VA = 0x242900
FOV_PROJECTION_OFFSET = 576
FOV_PROJECTION_HOOK = (FOV_PROJECTION_VA, bytes.fromhex("d9818c000000"), FOV_PROJECTION_OFFSET, 0xE9)
# The original's whole body, which the self-test reads (int3 after it).
FOV_PROJECTION_BODY = bytes.fromhex(
    "d9818c0000008b442408f30f1000f30f5e4004568b74240883ec0cd95c2408"
    "d98188000000d95c2404f30f110424f30f108190000000f30f5905ecd9a200"
    "e8edc4eaff83c40c8bc65ec20800cccccccc")
LATE_JOIN_HOOKS = (
    (0x2CBF9B, bytes.fromhex("e860c10000"), 3072, 0xE8),         # a player's animations: register_animations
    (0x2D3880, bytes.fromhex("e89bdf0400"), 0, 0xE8),            # a slot's ids: bind_slot
    (0x2D3906, bytes.fromhex("03f28d8de4e30000"), 96, 0xE9),     # twenty histories: history_capacity
    (0x27A23D, bytes.fromhex("881c388b44241c"), 128, 0xE9),      # 0xD8's capability byte: connect_capability
    (0x27A378, bytes.fromhex("83be641c1f0001"), 1856, 0xE9),     # the 0xE4 case: prepare_hook
    (0x1F2013, bytes.fromhex("e8e8000000"), 2048, 0xE8),         # player's destructor: player_destroyed_hook
    (0x27B517, bytes.fromhex("8b8f401c1f00"), 2336, 0xE9),       # on_match_disconnected: disconnect_hook
    (0x219E60, bytes.fromhex("558bec83e4f8"), 2432, 0xE9),       # network_client's destructor: client_destroyed_hook
    (0x23572B, bytes.fromhex("8b8c2498000000"), 3456, 0xE9),     # the hud's lists of players, made again: lists_hook
    (0x39E52, bytes.fromhex("f30f7e00660fd64704"), 3520, 0xE9),  # a box that is no box, in the tree: box_hook
    WEAPON_RANDOM_HOOK,                                          # a spawn's weapons' random state: weapon_random_hook
    KIT_STATE_HOOK,                                              # a spawn's weapons' kept state: kit_state_hook
    FIRST_LOAD_INPUT_HOOK,                                       # a first world's inputs, in time order: first_load_input
    FIRST_LOAD_KIT_HOOK,                                         # a first world's kit choices: first_load_kit
    FOV_PROJECTION_HOOK,                                         # the camera's angle: fov_projection
)
# on_match_ready's jump (HOOKS[0], which goes to the upload code without
# the late join) goes to ready_hook, which goes on to the upload code.
LATE_JOIN_READY_VA = 0x27AEA1
LATE_JOIN_READY_OFFSET = 1664

# engine_world::logic_tick 0x1A030, after the window was shown
# (engine_world+313h): `mov edx,[ebx+8]` / `mov eax,[edx+30h]` /
# `lea ecx,[ebx+8]` / `call eax` (is_application_active) / `test al,al` /
# `jz 1A113h`. Not active: the branch at 0x1A113 calls the game's
# on_waiting_for_render (it sends what is queued and ticks the lobby link)
# and game::tick is not called (transport.md section 6.1). The `jz`
# becomes two nops; the call stays, so whatever it does is still done.
# In the builds of patch revision 5 only (the late join needs every game
# of a match to answer). (address, the original's bytes, the new ones.)
# vostok::render::scene::remove_model 0x14FF60. It looks the model up in the
# scene's list (0x14FF88); when it is not there (`cmp edi,esi` / `jnz`,
# 0x14FF93) the original writes "remove_model: model is not inserted now"
# and goes on all the same: it takes the model's object out of the tree and
# erases the list's end (0x1501B4), which moves what lies after the list
# and adds one to a reference count through it (access violation at
# 0x15EE8D, live 2026-10-13: a game that had loaded a match and never
# entered it, when the server closed its link). The tree's erase before
# that did not fault, so the model's object was in a tree: LIKELY the model
# is another scene's, which holds it and lets go of it in its own time. The
# first instruction of that branch, `cmp [1F1D60Ch],ebx`, becomes a jump to
# the function's own end (0x1501F2): there it lets go of the model it was
# given (the pointer is passed by value, and the function that takes it
# counts it down: `lock xadd [model+0D0h]`, and 0xE0210 at zero), restores
# the three registers it saved and returns (`ret 8`). A jump past that
# (0x150215) would keep the model for ever. The branch's address has a base
# relocation, which is disabled.
REMOVE_MODEL_VA, REMOVE_MODEL_END = 0x14FF9B, 0x1501F2
REMOVE_MODEL = (REMOVE_MODEL_VA, bytes.fromhex("391d0cd6f101"),
                b"\xE9" + struct.pack("<i", REMOVE_MODEL_END - REMOVE_MODEL_VA - 5) + b"\x90")

# The game's own setting `time_floating_factor`: a console variable (the
# object at 0xA9D0D0, registered by 0x851330: its value is the float at
# 0xA799C8 in .data, from 0.001 to 0.5, the two floats the registration
# reads at 0x91A8F0 and 0x927A44). The `game` constructor copies the value
# into the game clock's timer once (0x1E9848: `movss xmm0,[0A799C8h]`, then
# timer+28h at 0x1E986D); nothing else reads it, and it is not among the
# settings the game keeps in user.cfg. When the server tells a game how far
# its clock is off (0xE7 -> floating_timer::set_offset 0x1EE9E0), the clock
# runs at 1 + the factor, or 1 - it, until the whole is made up: `offset /
# factor` long. With the original's 0.05 a game that starts from a world
# 465 ms behind (a round trip of 200 ms) needs 9.3 s, and the server drops
# every input that is more than 250 ms late: for 6 to 9 s after every
# start, late join and resync the player's keys do nothing (live,
# 2026-10-14; 13 to 15 s at a round trip of 300 ms). The default becomes
# 0.3, which the game's own range allows: 1.6 s. Four bytes of data, no
# code and no address (no base relocation is under them). Nothing that a
# match computes reads the factor, so games of either play one match.
CLOCK_FACTOR_VA = 0xA799C8
CLOCK_FACTOR_ORIGINAL, CLOCK_FACTOR = 0.05, 0.3
CLOCK_FACTOR_READ_VA = 0x1E9848               # the constructor's `movss`
CLOCK_FACTOR_REGISTER_VA = 0x851369           # `mov dword [0A9D110h],0A799C8h`
CLOCK_FACTOR_OPTION = 0xA9D0D0                # the console variable; its name at +10h
CLOCK_FACTOR_MIN_VA, CLOCK_FACTOR_MAX_VA = 0x91A8F0, 0x927A44
# .data: the only section but .text that a patch writes into.
DATA_VA, DATA_FILE, DATA_FILE_SIZE = 0xA5E000, 0xA4C400, 0x92200
CLOCK_FACTOR_PATCH = (CLOCK_FACTOR_VA, struct.pack("<f", CLOCK_FACTOR_ORIGINAL), struct.pack("<f", CLOCK_FACTOR))

# game_world_ui::update_ui (0x23B200), when the hud's state has changed:
# while the match waits or counts down (state+138h, the timelimit rule's)
# it shows the countdown (0x236AD0, 0x236B50) and jumps past the rest to the
# players' rows (0x23B328 -> 0x23B421); otherwise it shows the match time
# (0x236A00) and goes on at 0x23B383: the team points (0x2364C0,
# `root.set_game_progress`), in Team Deathmatch the last players
# (0x236610, `root.set_last_players`), and the respawn time (0x236C30,
# `root.set_respawn_time`). The rules fill the state's points every tick,
# the countdown too (tdm 0x316430: the tickets; gather 0x3140C0: the
# points), so they are the match's own then, but the hud kept the bars of
# the match before until play began (seen: 63 and 53, the last match's end,
# under "Awaiting match start"). The jump goes on to 0x23B383 instead: the
# same three, with the match time and the start's tip left out. During the
# countdown the respawn time is 0 (no player waits for one: the saved
# worlds of a real match), which shows an empty text, and the hud movie
# hides the last players while a team has tickets (HUD.as,
# set_game_progress): nothing but the bars changes, and they are drawn as
# play would draw them a few seconds later. Both paths reach 0x23B383 with
# the stack and the registers the code after it reads alike. The jump's
# rel32 holds no address. `jmp 0x23b421` -> `jmp 0x23b383`.
PREGAME_PROGRESS_VA, PREGAME_PROGRESS_TARGET = 0x23B328, 0x23B383
PREGAME_PROGRESS = (PREGAME_PROGRESS_VA, bytes.fromhex("e9f4000000"),
                    b"\xE9" + struct.pack("<i", PREGAME_PROGRESS_TARGET - PREGAME_PROGRESS_VA - 5))

# A field-of-view setting (patch 22). The video options' constructor
# (0x262929) makes option 7, a float slider of snap 1 labelled
# `st_fov_option` in all five languages, and names its console variable by
# `mov eax,offset "fov"` (0x262930; "fov" at 0xA0659C, read nowhere else).
# No console variable is called "fov", so the slider was never shown (the
# menu has no instance of it either: fov_slider_026g.py adds one to
# main_menu.swf). `uro_fov` (float at 0xA6FA28, 65.0, saved in user.cfg; the
# console variable at 0xAA6510, its name "uro_fov" at 0xA281D0) is
# registered with the range 0 to 100 and read by nothing. The option now
# names it: the instruction's relocation (0x262931) stays active, an
# address for an address.
FOV_OPTION_NAME = (0x262930, bytes.fromhex("b89c65a000"), b"\xb8" + struct.pack("<I", 0xA281D0))
FOV_VALUE = 0xA6FA28
# The camera's projection (FOV_PROJECTION_HOOK above; 0x242900, slot 0 of
# the vtables of game_camera, lobby_camera, free_fly_camera, demo_camera and
# player_input_handler): far, near and the viewport's aspect on the stack,
# xmm0 = the zoom (this+90h) * 1.134464 (65 degrees in radians, the float at
# 0xA2D9EC, read here alone), then the matrix builder 0xEEE30, `ret 8`. The
# zoom is set at every frame by 0x1F3480: 1 + (target - 1) * the aim's
# progress (weapon+5FCh), the target being a fitted scope's fov_factor
# (scope+114h, 0.25 in all three scopes' configs) or else the weapon's
# 1 / aim_zoom_factor (weapon+5B4h, from 1.0 to 1.2: 1 to 0.833), 1 when no
# weapon is aimed. fov_projection (late_join_026g.asm) passes the same
# arguments to the same builder with the angle
#   65 z + (uro_fov - 65) * max(0, (z - 0.25) / 0.75)  degrees:
# uro_fov unaimed, nearly it with iron sights, and through a scope exactly
# the original's 65 * 0.25, whatever the setting, so no setting magnifies a
# scope more than another. The rest of the original's body stays, never
# reached, with the constant's relocation.
# uro_fov's registration (0x85ADF0): `xorps xmm0,xmm0; movss [least],xmm0`
# becomes `mov dword [least],50.0; nop` (xmm0 is loaded with the most, 100,
# right after). At 0 the projection would divide by tan 0. The setter of every float console variable (0xECB70) stores the
# least for a value out of range or not a number, as the game always did:
# a wrong uro_fov in user.cfg gives 50. The relocation of the address in
# the replaced store (0x85AE0D) is disabled.
FOV_MIN = 50.0
FOV_MINIMUM = (0x85AE06, bytes.fromhex("0f57c0f30f11055465aa00"),
               bytes.fromhex("c7055465aa00") + struct.pack("<f", FOV_MIN) + b"\x90")
FOV_MINIMUM_RELOC = 0x85AE0D
FOV_PATCHES = (FOV_OPTION_NAME, FOV_MINIMUM)
# The high-FOV LOD compensation (patch 40; client_patch.md section 3.37,
# ../docs/notes/fov_lod.md). The game measures a model by its size on the
# screen, through the projection the camera gave: a static model's LOD
# (static_render_model_instance::select_lod 0x16EEC0: type 1, half the
# larger NDC extent of its box, LOD 3 below param2 draws nothing, 0x16F46A)
# and each surface's screen factor (render_surface_instance::
# calculate_screen_factor 0x158D80: the share of the screen times 100; the
# shader LOD, the sun's shadow casters 0x176340, spot shadows, rain
# occluders). They were tuned at 65 degrees or less. A wider angle makes a
# model smaller on the screen, so it lost detail and went out sooner: at
# 100 at 0.53 of the distance (tan 32.5 / tan 50). fov_projection
# (late_join_026g.asm) keeps k = max(1, tan(A/2) / tan(65 z/2)) at
# FOV_LOD_STATE_OFFSET of the state page, A its angle and 65 z the
# original's at that zoom. The two constants' instructions call
# fov_lod_scale and fov_screen_scale, which do the original's
# multiplication and then by k; with the page still zero (before the first
# projection) they leave the original's. Each site held the constant's
# address: its base relocation is disabled (FOV_LOD_RELOCS). Rendering
# only: nothing a match computes reads either value.
FOV_LOD_HOOKS = (
    (0x16F2A4, bytes.fromhex("f30f5905447a9200"), 1760, 0xE8),  # select_lod's `mulss xmm0,[0x927A44]` (0.5)
    (0x1590AF, bytes.fromhex("d80dd010a700"), 1792, 0xE8),      # the screen factor's `fmul dword [0xA710D0]` (100)
)
FOV_LOD_RELOCS = [0x1590B1, 0x16F2A8]
FOV_LOD_STATE_OFFSET = 0xE50                  # after the scoreboard's (TAB_LIST_STATE_OFFSET)
FOV_LOD_STATE_SIZE = 4

# A battery under the floor (patch 23, pickup_below_026g.asm at
# PICKUP_VA): client_patch.md section 3.23.
PICKUP_OFFSET = 0x800
PICKUP_VA = SECTION_VA + PICKUP_OFFSET
PICKUP_ACROSS = 1.0
PICKUP_FRAME = (0x2D84C9, bytes.fromhex("83ec34"), bytes.fromhex("83ec3c"))
PICKUP_HOOKS = (
    (0x2D84E9, bytes.fromhex("d905d810a700"), 0, 0xE8),          # the ray's length: reach
    (0x2D861C, bytes.fromhex("240a3c0a744b"), 32, 0xE9),         # a wall or a floor: solid
    (0x2D85BE, bytes.fromhex("8b45088b168b5230"), 64, 0xE9),     # a usable object: candidate
)
# The one address in what is replaced: the distance's, in the reach hook.
PICKUP_RELOC = 0x2D84EB

RUN_UNFOCUSED_VA = 0x1A0E1
RUN_UNFOCUSED_BRANCH_VA = 0x1A0EE
RUN_UNFOCUSED = (RUN_UNFOCUSED_VA, bytes.fromhex("8b53088b42308d4b08ffd084c07423"),
                 bytes.fromhex("8b53088b42308d4b08ffd084c09090"))


def late_join_state_va(code):
    """Where the state page of this late-join code is: the page after it."""
    end = LATE_JOIN_VA + len(code)
    return (end + 0xFFF) // 0x1000 * 0x1000


def death_camera_va(late_join):
    """Where `.srpc` is: the page after the late join's state page."""
    return late_join_state_va(late_join) + LATE_JOIN_STATE_SIZE


# A weapon wears the camouflage its item holds (patch 44, client_patch.md
# section 3.41): weapon_camo_026g.asm, in its own section `.srpw` on the page
# after `.srpc` (the death camera's one page; weapon_camo_va). weapon_cook::
# on_weapon_subresources_ready's `lea eax,[esp+20h] / push eax` (0x24B0D2,
# the model loaded, before it is given to the weapon at 0x243A30) calls
# weapon_ready, which asks for the camouflage's bake of each of the model's
# surfaces when the weapon's merged config has `data/camouflage`, and does
# the two instructions. No base relocation is under the site.
WEAPON_CAMO_NAME = b".srpw\0\0\0"
WEAPON_CAMO_CHARACTERISTICS = 0x60000020      # code, execute, read (as .srp)
WEAPON_CAMO_HOOK = (0x24B0D2, bytes.fromhex("8d44242050"), 0, 0xE8)
WEAPON_CAMO_ENTRY = bytes.fromhex("609c89e5") # pushad; pushfd; mov ebp,esp
WEAPON_CAMO_NAME_FORMAT = b"srpw1_%s_%s_%d\0"
# Patch 32 grown (client_patch.md section 3.30): weapon_stash_026g.asm at
# WEAPON_STASH_OFFSET of `.srpw`, after the camouflage's code. An upgrade
# window's item the lobby character does not show is shown on it from a
# scratch copy of the account's profile: a weapon no kit holds in the chosen
# kit's hands (find_kit_or_show, at WEAPON_STASH_FIND, which
# weapon_preview_026g.asm's two finds call: -DFIND), armour the chosen kit
# does not wear worn (patch 32's start, exit and setup_profile hooks go to
# the stash's entries first, which go on to patch 32's: -DPREVIEW).
WEAPON_STASH_OFFSET = 0x360
WEAPON_STASH_FIND = 0x0F
WEAPON_STASH_ENTRIES = {0x265E73: (0x00, 0x00), 0x265F44: (0x05, 0x6D), 0x284B10: (0x0A, 0xAD)}
#   hook site: (the stash's entry, patch 32's entry it goes on to)
WEAPON_PREVIEW_FIND_CALLS = (0x2B, 0xCC)       # weapon_preview_026g.section.bin: the two `call FIND`
# A weapon's upgrade window shows the weapon in the air above the lobby's
# workbench (patch 45, README; client_patch.md section 3.42):
# weapon_table_026g.asm at WEAPON_TABLE_OFFSET of `.srpw`, after the stash
# code; its state (WEAPON_TABLE_STATE_SIZE bytes) at WEAPON_TABLE_STATE_OFFSET
# of `.srpd`, after the others'. It begins with a table of five `jmp near`:
# camera (+0), which patch 32's start_cam and exit_cam jump to instead of the
# lobby camera's setter (WEAPON_PREVIEW_CAMERA_JUMPS, -DCAMERA), and the four
# hooks' entries:
# - the lobby camera's tick (0x283EF0, ebx = camera+4), `movss xmm0,
#   [ebx+0C8h]` at 0x284020 (the drag's yaw of the frame; every tick passes
#   it): tick, which turns the weapon by the drag and flies the camera;
# - player::mt_draw (0x2474A0), `cmp byte [ebp-1],0 / jne 247592h` at
#   0x247576 (a player not seen and not the world's own goes to the culled
#   draw): cull, which sends the table's player to the draw path;
# - mt_draw, `cmp byte [ebx+0F49Ch],0 / je 2477C2h` at 0x24778F (the held
#   item's transform: the decal pose's or the hand's): matrix, which gives
#   the table's player's held item the bench's transform, its middle at the
#   bench, once the character makes no player;
# - the lobby camera's on_mouse_move (0x283D30), `test byte [eax+0Ch],52h /
#   setne al` at 0x283D81 (its drag buttons): left, which at the bench also
#   drags with the left button pressed off the movie (is_mouse_over_ui);
# - the camera's tick, `call 0x2844D0` at 0x284309 (its collision ray from
#   the point back to the eye, which shortens the distance at a hit): ray,
#   which casts none while the camera is the table's (round 5c).
# No base relocation is under a site; the code holds no absolute address but
# the game's functions' and finds its state with call/pop.
WEAPON_TABLE_OFFSET = 0x6B0                    # round 5d: 0x800 until it outgrew the page (the stash ends at +6A5h)
WEAPON_TABLE_STATE_OFFSET = 0xF00
WEAPON_TABLE_STATE_SIZE = 0xBC                 # 0xA0 until the lobby's spot (SPOT, VIEW, TURN, 2026-10-29)
# The interface other patches read (weapon decals): while the state's TABLE
# (+0) and READY (+28h) are 1, the transform the held weapon was last drawn
# with at the bench, a float4x4 in rows (v * M, as the game's: row 3 the
# translation), at +WEAPON_TABLE_MATRIX_OFFSET of the state, and a u32 stamp
# at +WEAPON_TABLE_STAMP_OFFSET that grows by 1 at each one written.
WEAPON_TABLE_STAMP_OFFSET = 0x48
WEAPON_TABLE_MATRIX_OFFSET = 0x50
WEAPON_PREVIEW_CAMERA_JUMPS = (0x68, 0xA8)     # weapon_preview_026g.section.bin: the two `jmp CAMERA`
WEAPON_TABLE_ENTRIES = (0x0, 0x5, 0xA, 0xF, 0x14, 0x19)  # camera, tick, cull, matrix, left, ray: `jmp near` each
# A weapon's upgrade window shows an upgrade dropped on the weapon at once
# (patch 48, README; client_patch.md section 3.45): upgrade_preview_026g.asm
# at UPGRADE_PREVIEW_OFFSET of `.srpw`, a page of its own (the second page is
# the weapon decals', patch 47); its state at UPGRADE_PREVIEW_STATE_OFFSET of
# `.srpd`, after the patches' mount's (MOUNT_STATE_OFFSET) and before patch
# 32's (WEAPON_PREVIEW_STATE_OFFSET). Seven `jmp near`, the entries:
# - relocate_upgrade_item's handler (0x29A8B0), `mov edx,[esi+4620h]` at
#   0x29A9B2 (before its `46 01` is written): relocate, which queues the
#   change for the window's weapon and makes the character again at once;
# - drop_upgrade_item's handler (0x29A9D0), `mov ecx,[esi+4620h]` at
#   0x29AACF (before patch 30's `46 02`): drop, the same;
# - the 0x87 reader (0x270260), `call 0x270320` at 0x270313, its last call:
#   account, the server's account answers the oldest queued change;
# - patch 32's two `call FIND` (WEAPON_PREVIEW_FIND_CALLS): find, which calls
#   weapon_stash_026g.asm's find_kit_or_show and builds the window's weapon
#   from the scratch with the queue applied;
# - setup_profile past its early return, `cmp dword [ebx+0D2C4h],-1` at
#   0x284B89: again, which returns when the player being made is of the
#   kit's id and revision asked;
# - on_player_ready, `cmp byte [ebx+0D2C9h],0` at 0x284F7F: made, a log line
#   with the time the lobby character's player took;
# - setup_profile's early return, `cmp byte [ebx+0D2C8h],0` at 0x284B7C (its
#   last test before `jz 284CF8h`): early, which for the lobby's own
#   character returns early only while no player is being made (patch 32's
#   window closed while its scratch's player was being made: the owner's
#   report of 2026-10-23, client_patch.md section 3.42).
# No base relocation is under a site and no branch aims inside one; the code
# holds the absolute addresses of its state, patch 32's and its strings (the
# exe is never rebased).
UPGRADE_PREVIEW_OFFSET = 0x2000
UPGRADE_PREVIEW_STATE_OFFSET = 0xD60
UPGRADE_PREVIEW_STATE_SIZE = 0xA0
UPGRADE_PREVIEW_FIND = 0x0F
UPGRADE_PREVIEW_ENTRIES = (0x0, 0x5, 0xA, 0xF, 0x14, 0x19, 0x1E)  # relocate, drop, account, find, again, made, early
# a dword after the entries: patch 68's entry weapon_kit.preview_apply (the Rust
# patch weapon_kit, a generated entry that aligns the stack), which the build writes once the arena
# is linked (0 in the file and without an arena: patch 48's own apply)
UPGRADE_PREVIEW_KIT_APPLY = 0x23
WEAPON_KIT_PREVIEW_APPLY = "weapon_kit_on_preview_apply"   # the generated entry of weapon_kit.preview_apply
UPGRADE_PREVIEW_HOOKS = (
    (0x29A9B2, bytes.fromhex("8b9620460000"), 0x0, 0xE8),          # relocate_upgrade_item: relocate
    (0x29AACF, bytes.fromhex("8b8e20460000"), 0x5, 0xE8),          # drop_upgrade_item: drop
    (0x270313, bytes.fromhex("e808000000"), 0xA, 0xE8),            # the 0x87 reader's last call: account
    (0x284B89, bytes.fromhex("83bbc4d20000ff"), 0x14, 0xE8),       # setup_profile past its early return: again
    (0x284F7F, bytes.fromhex("80bbc9d2000000"), 0x19, 0xE8),       # on_player_ready: made
    (0x284B7C, bytes.fromhex("80bbc8d2000000"), 0x1E, 0xE8),       # setup_profile's early return: early
)
WEAPON_TABLE_HOOKS = (
    (0x284020, bytes.fromhex("f30f1083c8000000"), 0x5, 0xE8),        # the camera's tick: tick
    (0x247576, bytes.fromhex("807dff007516"), 0xA, 0xE8),            # mt_draw's culled player: cull
    (0x24778F, bytes.fromhex("80bb9cf4000000742a"), 0xF, 0xE8),      # mt_draw's held item's transform: matrix
    (0x283D81, bytes.fromhex("f6400c520f95c0"), 0x14, 0xE8),        # the camera's drag buttons: left
    (0x284309, bytes.fromhex("e8c2010000"), 0x19, 0xE8),            # the camera's collision ray: ray
)

# Decals on a weapon (patch 47, README; client_patch.md section 3.44):
# weapon_decals_026g.asm at WEAPON_DECALS_OFFSET of `.srpw`, after patch
# 45's code (the section's second page until patch 45 grew, 2026-10-29); its state (WEAPON_DECALS_STATE_SIZE bytes: the left button's
# press) at WEAPON_DECALS_STATE_OFFSET of `.srpd`, after patch 45's, whose
# published transform (WEAPON_TABLE_MATRIX_OFFSET) it reads, as patch 32's
# mode and item. It begins with seven `jmp near`: lookup (+0) and fill (+5), which patch 44 calls
# once each (WEAPON_CAMO_DECALS_CALLS: what a weapon's bake paints, the
# camouflage and the decals of `data/decals`), and the decal editor's four
# sites, for a weapon above the workbench:
# - the preview's pick, `call 0x264960` at 0x265999 of lobby_character's
#   per-frame decal preview (0x265980): pick;
# - the preview's matrix, `call 0x263D00` at 0x2659B2: decode;
# - the editor's bake, 0x2641A0 (`push ebp / mov ebp,esp / and esp,-8`, the
#   entry): `jmp bake` and a nop;
# - the start's query, `call 0x264CF0` at 0x263938 (the end of 0x2636B0):
#   start;
# - the character's mouse key handler, 0x265C20 (`push ebp / mov ebp,esp /
#   and esp,-8`, the entry; in the input vtable 0xA09F6C): `jmp click` and a
#   nop: the left button places the decal on a click, not after a drag.
# No base relocation is under a site and no branch aims inside one.
WEAPON_DECALS_OFFSET = 0x1200                  # 0x1000 until patch 45 read the lobby's spot (2026-10-29)
WEAPON_DECALS_ENTRIES = (0x0, 0x5, 0xA, 0xF, 0x14, 0x19, 0x1E)  # lookup, fill, pick, decode, bake, start, click
WEAPON_DECALS_STATE_OFFSET = 0xFC0            # 0xFA0 until patch 45's state grew (2026-10-29)
WEAPON_DECALS_STATE_SIZE = 12
WEAPON_DECALS_HOOKS = (
    (0x265999, bytes.fromhex("e8c2efffff"), 0xA, 0xE8),         # the preview's pick
    (0x2659B2, bytes.fromhex("e849e3ffff"), 0xF, 0xE8),         # the preview's matrix
    (0x2641A0, bytes.fromhex("558bec83e4f8"), 0x14, 0xE9),      # the editor's bake
    (0x263938, bytes.fromhex("e8b3130000"), 0x19, 0xE8),        # the start's query
    (0x265C20, bytes.fromhex("558bec83e4f8"), 0x1E, 0xE9),      # the mouse key handler
)
WEAPON_CAMO_DECALS_CALLS = (0x0, 0x5)                           # the entries patch 44 calls

# The artefacts (patch 54, README; client_patch.md section 3.51; the rules:
# ../docs/notes/artefacts.md): artefacts_026g.asm in the arena's block
# artefacts.code (ARENA below). Its numbers are the artefacts'
# configs' (release/content.tsv, artefacts-1.json). It begins with eight
# `jmp near`, one for each site, each a global symbol the hook jumps to:
# - Spring's loader 0x33ACF0, `mov dword [esi+48h],13h` at 0x33AE13 (its
#   active's third node): loader. The passive's second node (+350h,
#   max_carried_weight 0 in retail) becomes aim_time (49) with the value of
#   `passive/aim_time_mod`; two more nodes are the active's: +3B8h aim_time
#   with `active/aim_time_mod`, +3C8h list 42 with
#   `active/stopping_power_mod`;
# - the link of Spring's active nodes, 0x311BC0 (`push ebx / push esi /
#   mov esi,eax / mov eax,[esi+250h]`, the entry): link the two too;
# - their unlink, 0x311DA0 (`push ebx / mov ebx,[esp+8]`, the entry): unlink;
# - Lifebone's enable_active after its remove_affects, `mov ecx,[edi+250h]`
#   at 0x3122AA: the pain, body and head pools filled to their maximum
#   (increase_health, as a medkit);
# - artefact_base::action 0x313300 (`push esi / mov esi,ecx / cmp dword
#   [esi+280h],3`, the entry): a key-down activates every ready artefact of
#   the pressed one's class in the artefact slots, as action activates one,
#   and returns; a key-up goes on in action;
# - the player's stagger 0x2DBCD0, `movss xmm2,[esi+31Ch]` at 0x2DBD89 (the
#   recent hits' stopping power summed): the sum times max(0, 1 + list 42),
#   list 42 (artefact_synergy) being read by no code of 0.26g and set by no
#   config, so an active Spring's -1 takes the slow away;
# - Onyx's hit callback 0x312BF0, `cmp dword [ecx+280h],3` at 0x312C03 (after
#   its hit type test): an active Onyx that is not the first active one of
#   the slots leaves the hit as it is (`ret 0Ch`), so n active Onyx reduce a
#   hit once; the passive as before. With the action's entry: each Onyx
#   fired with others adds 1 s to the active time (+F14h) of all of them;
# - Onyx's client update 0x25F640, `mov ecx,[ebx-0C30h] / push 0BB8h` at
#   0x25F9DD (its activation hint's set_parameterized_message): the time on
#   screen is the active's time left (the hud writes it for the hint's %s),
#   and only the first Onyx of the slots fired by the same press shows it.
# The two nodes are after a Spring's own fields: the two sizes of a Spring,
# artefact<artefact_spring_core> (0x3B8, 0x25E7A8) and the bare core (0x398,
# 0x2EEA74), become ARTEFACTS_SPRING_SIZE. No base relocation is under a
# site and no branch aims inside one; the code holds the absolute addresses
# of its two names (the exe is never rebased).
ARTEFACTS_HOOKS = (
    (0x33AE13, bytes.fromhex("c7464813000000"), "artefacts_loader", 0xE9),        # Spring's loader: its active's third node
    (0x311BC0, bytes.fromhex("53568bf08b8650020000"), "artefacts_link", 0xE9),    # Spring's active link
    (0x311DA0, bytes.fromhex("538b5c2408"), "artefacts_unlink", 0xE9),            # Spring's active unlink
    (0x3122AA, bytes.fromhex("8b8f50020000"), "artefacts_lifebone", 0xE9),        # Lifebone's enable_active
    (0x313300, bytes.fromhex("568bf183be8002000003"), "artefacts_fire", 0xE9),    # artefact_base::action: its entry
    (0x2DBD89, bytes.fromhex("f30f10961c030000"), "artefacts_stagger", 0xE9),     # the player's stagger: its sum
    (0x312C03, bytes.fromhex("83b98002000003"), "artefacts_onyx", 0xE9),          # Onyx's hit callback: its state test
    (0x25F9DD, bytes.fromhex("8b8bd0f3ffff68b80b0000"), "artefacts_popup", 0xE9), # Onyx's client update: its pop-up
)
ARTEFACTS_BACKS = (0x33AE1A, 0x311BCA, 0x311DA5, 0x3122B0, 0x31330A, 0x2DBD91, 0x312C0A,
                   0x25F9E8)                    # where each goes on (the pop-up's skip: 0x25F9F8)
ARTEFACTS_SPRING_SIZE = 0x3D8                  # the active's two nodes at +3B8h and +3C8h
ARTEFACTS_SIZES = (
    (0x25E7A8, bytes.fromhex("b8b8030000"), b"\xB8" + struct.pack("<I", ARTEFACTS_SPRING_SIZE)),
    (0x2EEA74, bytes.fromhex("b898030000"), b"\xB8" + struct.pack("<I", ARTEFACTS_SPRING_SIZE)),
)

# The kit keys and the match chat option (patch 58, README; client_patch.md
# section 3.53; the rules: ../docs/notes/match-hud-options.md):
# match_keys_026g.asm in the arena's blocks match_keys.code and
# match_keys.state (ARENA below), three entries:
# - the game world's on_keyboard_action 0x25CD00 at its kind's test,
#   `cmp dword [ebp+10h],1 / jnz near 0x25CF0D` at 0x25CD41 (after the Tab
#   test; edi = its input handler): a key 1, 2 or 3 going down while the hud
#   shows the cursor (the kits) invokes the hud movie's
#   root.select_kit_key(kit), then the function goes on as before at
#   0x25CD4B or 0x25CF0D (the key is not taken);
# - the game world's constructor's `call 0x233810` (the hud's constructor)
#   at 0x1F6845: the hud made, then a cc_bool named as the gameplay options'
#   dead option 4 ("g_private_messages_in_game" 0xA064FC) registered on the
#   option's byte, saved in user.cfg, before the configs run and the options
#   look their variables up;
# - process_incoming_text_message's `call 0x208AD0` (add_message) at
#   0x26B916: with the option on, in a match, a player's line on a channel
#   the hud shows (3, 5..9) is dropped (`ret 14h`, add_message's).
# No base relocation is under a site and no branch aims inside one.
# And a slot (MATCH_KEYS_POLL_SLOT): the game world's input handler's +14h
# (its vtable 0x9FC588; `ret 4` at 0x75AF0 in the original), which the input
# world's tick (0x3B7BD0) calls for every handler after it polled the devices
# and before they hand out the keys: match_keys_poll sets the kits' screen's
# state from the hud's cursor and GetTickCount (CLOSED; OPENING from the
# first poll the kits are up at, with its time; OPEN from the first poll 300
# ms, KIT_KEYS_DELAY, after it), and a key picks a kit only in OPEN: a key
# that went down in the frame the kits came, in play, is handed out in their
# first tick and picks nothing, nor does one pressed in their first 300 ms
# (the owner, 2026-10-25: "I press 3 mid-fight to heal with the medkit, I
# die, and I swap to my 3rd kit"; then: ignore them for 300 ms).
MATCH_KEYS_HOOKS = (
    (0x25CD41, bytes.fromhex("837d1001" "0f85c2010000"), "match_keys_keyboard", 0xE9),   # on_keyboard_action: the kind
    (0x1F6845, bytes.fromhex("e8c6cf0300"), "match_keys_hud", 0xE8),          # the hud's constructor call
    (0x26B916, bytes.fromhex("e8b5d1f9ff"), "match_keys_chat", 0xE8),         # a received line's add_message
)
MATCH_KEYS_BACKS = (0x25CD4B, 0x25CF0D)        # on_keyboard_action: a key down, any other
MATCH_KEYS_POLL_SLOT = (0x9FC59C, struct.pack("<I", 0x75AF0), "match_keys_poll")   # the handler's slot +14h

# The key that hides the interface (patch 62, README; client_patch.md section
# 3.54; the rules: ../docs/notes/hide-ui-key.md): hide_ui_026g.asm in the
# arena's blocks hide_ui.code and hide_ui.data (ARENA below), three entries:
# - the game world's on_keyboard_action 0x25CD00, its case of action 19h
#   (toggle_replay_ui, F11; the switch's table 0x25CF30 + 8 * 4), `mov
#   esi,[esp+10h] / cmp dword [esi+18h],0 / jz 0x25CF0D` at 0x25CED1: with
#   the replay's movie (the handler's +18h) it goes on at 0x25CEDB as before;
#   without it the hud movie ([[hud+8]+108h]+4, the hud the handler's +4) is
#   asked root.interface_hidden and it and the player icons' movie
#   ([[hud+10h]+108h]+4) get root.hide_interface(not that), whatever the
#   cursor; both ways the key is not taken (0x25CF0D). Not the game world's
#   show_ui (0x1F95F0): the movies it hides stand still, and what came
#   meanwhile played at once when shown (the owner's test of 64ce3ce7);
# - lobby_menu::on_keyboard_action's `call 0x207D50` (get_binded_action,
#   kind 2) at 0x26CCD2: a key with no lobby action is asked again for kind
#   1; toggle_replay_ui invokes the lobby movie's root.toggle_interface and
#   returns at 0x26CCAE (al 1: taken, the movie does not get the key); any
#   other key, while the movie answers root.interface_hidden true, returns at
#   0x26CDD2 (the key to the movie, which shows the interface; no window key,
#   no F6 Play).
# - initialize_match's `call 0x511600` (Invoke, root.reset) at 0x234AE8 (ecx
#   the hud movie, ebp the hud): the reset, then the hud and the player icons'
#   root.hide_interface(false): a new match starts shown (the movies are the
#   last match's).
# No base relocation is under a site and no branch aims inside one.
HIDE_UI_HOOKS = (
    (0x25CED1, bytes.fromhex("8b742410" "837e1800" "7432"), "hide_ui_match", 0xE9),   # on_keyboard_action: case 19h
    (0x26CCD2, bytes.fromhex("e879b0f9ff"), "hide_ui_lobby", 0xE8),                 # the lobby's binder call
    (0x234AE8, bytes.fromhex("e813cb2d00"), "hide_ui_reset", 0xE8),                 # a match's root.reset
)
# The options' controls (the same patch): the six functions of game_options
# that read the table of 28h rows at 0xA9DB28 read `controls` (hide_ui.data,
# 2Ah rows, the 29h toggle_replay_ui, the 2Ah patch 78's artefact) instead: (site, its bytes, where in
# them the address is, the row's field it names) and (site, its bytes, where
# the count 28h is).
HIDE_UI_TABLE = 0xA9DB28
HIDE_UI_ROWS = 0x28
HIDE_UI_TABLE_REFS = (
    (0x201150, bytes.fromhex("b828dba900"), 1, 0x0),           # the conflicts' loop: mov eax,table
    (0x201248, bytes.fromhex("be2cdba900"), 1, 0x4),           # the conflict message: mov esi,table+4
    (0x2013BF, bytes.fromhex("0528dba900"), 1, 0x0),           # a key set: add eax,table
    (0x201A19, bytes.fromhex("be34dba900"), 1, 0xC),           # the movie's rows: mov esi,table+0Ch
    (0x201D05, bytes.fromhex("c744240c28dba900"), 4, 0x0),     # the keys from the binder: mov [esp+0Ch],table
    (0x201F2B, bytes.fromhex("be38dba900"), 1, 0x10),          # the apply: mov esi,table+10h
)
HIDE_UI_TABLE_COUNTS = (
    (0x201159, bytes.fromhex("c744241828000000"), 4),
    (0x20124D, bytes.fromhex("bb28000000"), 1),
    (0x20140B, bytes.fromhex("80fb28"), 2),
    (0x201A1E, bytes.fromhex("c744244428000000"), 4),
    (0x201D0D, bytes.fromhex("c744241028000000"), 4),
    (0x201F34, bytes.fromhex("c745f828000000"), 3),
)


def hide_ui_table_writes(linked):
    """[(site, its bytes, the new bytes)] of the six functions' table and
    counts, `controls` being the link's."""
    table = linked.symbols["controls"]
    out = [(va, old, old[:at] + struct.pack("<I", table + field) + old[at + 4:])
           for va, old, at, field in HIDE_UI_TABLE_REFS]
    out += [(va, old, old[:at] + bytes([HIDE_UI_ROWS + len(CONTROLS_ADDED)]) + old[at + 1:])
            for va, old, at in HIDE_UI_TABLE_COUNTS]
    return out


# The rows `controls` adds after the game's 28h: (action, label key).
CONTROLS_ADDED = ((0x19, "st_hide_interface_action"), (0x23, "st_hold_artifact_action"))
# Patch 78's carry key (../docs/notes/held_artefact_transfer.md): the game's
# own action `artefact` (23h, kinds 5; action table entry 0xA74D5C), which
# nothing in the game reads and had no default key, gets H (the binder's
# set_default_controls 0x2078A0 binds each entry's +10h at its construction,
# before user.cfg; a key name the exe has, "kH" at 0x9FDCA8). Its row is the
# 42nd of `controls`.
ARTEFACT_KEY_DEFAULT = (0xA74D6C, bytes(4), struct.pack("<I", 0x9FDCA8))


HIDE_UI_BACKS = (0x25CEDB, 0x25CF0D, 0x26CCAE, 0x26CDD2)
# the replay's toggle, the key not taken; the lobby's key taken, and the key to the movie

# The kill feed's wall, ricochet and pain icons (patch 67, README; client_patch.md
# section 3.55; the rules: ../docs/notes/kill-feed-icons.md):
# kill_feed_026g.asm in the arena's block kill_feed.code (ARENA below), two
# entries called in place of the site's instructions (INLINE), and one load
# widened:
# - generate_killed_event 0x2DA170, `mov [eax+4],bl / mov edx,[esi+120h]` at
#   0x2DA3D1 (eax = the event's node, esi = the victim, ebp = the bullet or
#   0): kill_feed_event writes the cause icon and the flags byte (+5), and
#   edx as the site did;
# - the hud's event handler 0x23B5F0, `movzx edx,byte [edi+4]` at 0x23B6F0
#   (the fifth argument of on_player_killed): the word, the flags its second
#   byte (KILL_FEED_ARGUMENT);
# - on_player_killed 0x237C70, `mov edx,[esp+30h] / mov ecx,[esp+2Ch]` at
#   0x23814F (after extra_icon): kill_feed_row sets penetration_icon,
#   ricochet_icon and pain_icon and does the two moves.
# No base relocation is under a site and no branch aims inside one.
KILL_FEED_HOOKS = (
    (0x2DA3D1, bytes.fromhex("885804" "8b9620010000"), "kill_feed_event", 0xE8),   # the event's cause icon
    (0x23814F, bytes.fromhex("8b542430" "8b4c242c"), "kill_feed_row", 0xE8),       # before mastery_icon
)
KILL_FEED_ARGUMENT = (0x23B6F0, bytes.fromhex("0fb65704"), bytes.fromhex("0fb75704"))

# The baked camouflage and decal textures kept, and baked or loaded once while
# asked for again (patch 70, README; client_patch.md section 3.57; the rules:
# ../docs/notes/baked-textures.md): bake_cache_026g.asm and .c in the arena's
# blocks bake_cache.code and .state (ARENA below):
# - game::game's delete of user_data/streamed_textures (`call 0x1E8DA0`,
#   0x1E9455): bake_cache_start, the delete only for another revision's
#   folder, else the kept files back in the baker's list (CALLEE);
# - check_loaded_baked_texture's `cmp eax,[edi+114h]` (0x131C90): a kept
#   file's record takes the CRC loaded (INLINE);
# - translate_query's look-up (`call 0x131ED0`, 0x12D4CD): a path under way
#   waits (CALLEE);
# - process_compression's and on_baked_texture_loaded's `call 0x420789`
#   (0x12F18F, 0x12DDB4), the texture made: the waiting queries get it
#   (CALLEE);
# - finish_query's entry (0xF0D00): a path's owner ended otherwise, its
#   waiting queries made again (ENTRY);
# - the CRC-32 step (0x42050, a byte a step; every bake and every load of a
#   baked texture runs it over the whole file): the same value eight bytes a
#   step (ENTRY);
# and game::exit's delete (0x1EC832) nops (BAKE_CACHE_KEEP).
# No base relocation is under a site and no branch aims inside one.
BAKE_CACHE_HOOKS = (
    (0x1E9455, bytes.fromhex("e846f9ffff"), "bake_cache_start", 0xE8),       # game::game's delete
    (0x131C90, bytes.fromhex("3b8714010000"), "bake_cache_adopt", 0xE8),     # check_loaded's CRC compare
    (0x12D4CD, bytes.fromhex("e8fe490000"), "bake_cache_lookup", 0xE8),      # translate_query's look-up
    (0x12F18F, bytes.fromhex("e8f5152f00"), "bake_cache_baked", 0xE8),       # a bake's texture made
    (0x12DDB4, bytes.fromhex("e8d0292f00"), "bake_cache_loaded", 0xE8),      # a load's texture made
    (0xF0D00, bytes.fromhex("558bec83e4f8"), "bake_cache_finish", 0xE9),     # finish_query
    (0x42050, bytes.fromhex("8b4c240457"), "bake_cache_crc32", 0xE9),        # the CRC-32 step, slice by 8
)
BAKE_CACHE_KEEP = (0x1EC832, bytes.fromhex("e869c5ffff"), bytes.fromhex("0f1f440000"))   # game::exit's delete


def weapon_camo_va(late_join):
    """Where `.srpw` is: the page after `.srpc`, which is one page."""
    return death_camera_va(late_join) + DEATH_CAMERA_SIZE


# Every function a patch installs: what the game calls of ours, and the shape
# it must have (client_patch.md section 0.4; README "Call shapes"). A
# function the game calls must pop off the stack what the game's own function
# in its place pops (its `ret n`), or the caller's stack is off by the
# difference: Research's rule slot 16 (#297) popped 4 bytes where the game's
# rules pop 8, and the game died at the first sync. build() fails on a call,
# jump or pointer into our code, at any offset of any write, that is not
# declared here (audit_installs). selftest_026g.py (test_call_shapes) reads
# what ours pops from the built bytes and its model's from the original exe
# and fails on a mismatch for every SLOT, ENTRY, CALLEE and ABI site (and
# every CALLBACK and INSTALLED_VTABLES slot); INLINE and JUMP sites are
# declared but not checked by it (each patch's Unicorn check covers them).
# The kinds, by what the site was in the original:
#   SLOT    a pointer the game calls through (a vtable slot, a delegate), at
#           the site: the model is the function the original's pointer names;
#   ENTRY   a function's first bytes jump to ours, or its body is ours in
#           place: the model is that function;
#   CALLEE  `[push ...] call F` calls ours: the model is F, less the bytes of
#           the pushes the site no longer makes;
#   INLINE  a call of ours where the site was other instructions: ours does
#           them and returns with their stack effect (a plain `ret` when they
#           push nothing);
#   ABI     a call of ours that pops what `pops` says, for the reason `why`
#           (the site's instructions have an effect no model gives);
#   JUMP    a jump into ours from the middle of the game's code: every way
#           back must be at the stack depth the site's instructions leave.
# {site VA: (kind, what, pops, why)}; pops and why for ABI only.
SLOT, ENTRY, CALLEE, INLINE, ABI, JUMP = "slot", "entry", "callee", "inline", "abi", "jump"
INSTALLS = {
    0x23B207: (CALLEE, "update_ui's update_player_icons, then the units' markers: icons", None, None),
    # the weapon kit, patch 68 (the Rust patch weapon_kit's one code hook)
    0x2F1771: (JUMP, "item_config_cook::translate_query 0x2F1750, its `call 0x2F2060` (esi the query's holder, edi "
                     "ebp+38h): weapon_kit_query_room, which calls it, takes the kit's room below the frame and "
                     "goes on at 0x2F1776 (the lohook weapon_kit.query_records)", None, None),
    # the snapshot upload (every full build) and the default server's stub
    0x27AEA1: (JUMP, "on_match_ready: the snapshot upload (the late join's ready_hook first)", None, None),
    0x27A505: (JUMP, "the 0xF6 case: the snapshot upload", None, None),
    0x1EB25B: (JUMP, "the missing server's fatal: the default server's stub", None, None),
    # the late join (patch 11 and those it grew)
    0x2CBF9B: (CALLEE, "a player's animations: register_animations", None, None),
    0x2D3880: (CALLEE, "a slot's ids: bind_slot", None, None),
    0x2D3906: (JUMP, "twenty histories: history_capacity", None, None),
    0x27A23D: (JUMP, "0xD8's capability byte: connect_capability", None, None),
    0x27A378: (JUMP, "the 0xE4 case: prepare_hook", None, None),
    0x1F2013: (CALLEE, "the player's destructor: player_destroyed_hook", None, None),
    0x27B517: (JUMP, "on_match_disconnected: disconnect_hook", None, None),
    0x219E60: (ENTRY, "network_client's destructor: client_destroyed_hook", None, None),
    0x23572B: (JUMP, "the hud's lists made again: lists_hook", None, None),
    0x39E52: (JUMP, "a box that is no box: box_hook", None, None),
    0x316EA0: (INLINE, "a spawn's weapons' random state: weapon_random_hook", None, None),
    0x316E57: (INLINE, "a spawn's weapons' kept state: kit_state_hook", None, None),
    0x2D1050: (CALLEE, "a first world's inputs: first_load_input", None, None),
    0x2D1055: (CALLEE, "a first world's kit choices: first_load_kit", None, None),
    0x242900: (ENTRY, "the camera's projection: fov_projection", None, None),
    0x16F2A4: (INLINE, "select_lod's x0.5: fov_lod_scale", None, None),
    0x1590AF: (INLINE, "the screen factor's x100: fov_screen_scale", None, None),
    0x23537A: (ABI, "the scoreboard's row: tab_row", 0,
               "the site's `push eax / push ebx / push eax / call edx` is SetElement(data, place, row), thiscall, "
               "`ret 0Ch`: no stack effect, as tab_row's PushBack (vtable+3Ch) or SetElement"),
    0x23B421: (INLINE, "the scoreboard's refresh: tab_shown", None, None),
    # patch 23, 26, 25, 31, 34, 32, 46
    0x2D84E9: (INLINE, "the use scan's ray length: reach", None, None),
    0x2D861C: (JUMP, "the use scan's wall or floor: solid", None, None),
    0x2D85BE: (JUMP, "the use scan's usable object: candidate", None, None),
    0x1EE3A3: (JUMP, "the replay's seek: seek_clear", None, None),
    0x380889: (INLINE, "the version field's last writes: stale_dll", None, None),
    0x316820: (ENTRY, "the respawn rule's tick: kit_change", None, None),
    0x26794D: (CALLEE, "the view of a kit change: kit_view", None, None),
    0x23B7DA: (JUMP, "the local player alive: ui_alive", None, None),
    0x23B7FC: (INLINE, "the hud's update: ui_start", None, None),
    0x25D0E0: (ENTRY, "the game world's on_mouse_move: death_look, then warmup_look", None, None),
    0x24225E: (INLINE, "the warmup mode's pitch: warmup_limits", None, None),
    0x25CF60: (ENTRY, "the game world's on_mouse_key_action: warmup_keys", None, None),
    0x1F364B: (CALLEE, "update_camera's view update: death_view", None, None),
    0x242196: (CALLEE, "the input builder's last action call: suicide_key (Rust `call` hook)", None, None),
    0x265E73: (CALLEE, "start_item_upgrade's camera: the stash's start, then start_cam", None, None),
    0x265F44: (CALLEE, "exit_item_upgrade's camera: the stash's exit, then exit_cam", None, None),
    0x284B10: (ENTRY, "setup_profile: the stash's setup, then setup_hook", None, None),
    0x284081: (INLINE, "the lobby camera's wheel least: min_step", None, None),
    0x28428C: (INLINE, "the lobby camera's step least: min_step", None, None),
    0x284316: (INLINE, "the lobby camera's last clamp: min_final", None, None),
    0x284A33: (INLINE, "the lobby character's decal pose: player_mode", None, None),
    # patch 27/49, 28, 29, 30, 33
    0xE7E3E: (CALLEE, "query_db's query_mount_archive: mount_all", None, None),
    0x147E0: (ENTRY, "on_mounted_resources: on_mounted", None, None),
    0x1D90DC: (JUMP, "the forward compile's switch: use_uv_scrolling", None, None),
    0x1D94CB: (JUMP, "the forward compile's constants: uv_scrolling_parameters", None, None),
    0x14990F: (JUMP, "renderer_context: scene_slomo", None, None),
    0x28CDB6: (JUMP, "video_tutorial_link_clicked: replay_link", None, None),
    0x29AAD5: (CALLEE, "the upgrade window's drop: drop_upgrade", None, None),
    0x275756: (JUMP, "`83 46 02`: detach_answer", None, None),
    0x275589: (CALLEE, "`83 46 01`'s detach: detach_first", None, None),
    0x27534D: (CALLEE, "process_detach_upgrade's find_if: find_host", None, None),
    READ_ORDER_VA: (ENTRY, "hdd_manager_sorting_predicate, in place: read_order", None, None),
    # patch 38, 44, 45, 47, 48
    0x2D8A0F: (JUMP, "the spot's owner: spot", None, None),
    0x24B0D2: (INLINE, "the weapon's model ready: weapon_ready", None, None),
    0x284020: (INLINE, "the lobby camera's tick: tick", None, None),
    0x247576: (INLINE, "mt_draw's culled player: cull", None, None),
    0x24778F: (INLINE, "mt_draw's held item's transform: matrix", None, None),
    0x283D81: (INLINE, "the lobby camera's drag buttons: left", None, None),
    0x284309: (CALLEE, "the lobby camera's collision ray: ray", None, None),
    0x265999: (CALLEE, "the decal preview's pick: pick", None, None),
    0x2659B2: (CALLEE, "the decal preview's matrix: decode", None, None),
    0x2641A0: (ENTRY, "the decal editor's bake: bake", None, None),
    0x263938: (CALLEE, "the decal editor's start query: start", None, None),
    0x265C20: (ENTRY, "the lobby character's mouse keys: click", None, None),
    0x29A9B2: (INLINE, "relocate_upgrade_item: relocate", None, None),
    0x29AACF: (INLINE, "drop_upgrade_item: drop", None, None),
    0x270313: (CALLEE, "the 0x87 reader's last call: account", None, None),
    0x284B89: (INLINE, "setup_profile past its early return: again", None, None),
    0x284F7F: (INLINE, "on_player_ready: made", None, None),
    0x284B7C: (INLINE, "setup_profile's early return: early", None, None),
    # the replay viewer (patch revision 11): a vtable slot, and the sound queue's cave
    REPLAY_REATTACH_SLOT[0]: (SLOT, "replay_network_client's \"is this place mine\" (vtable 0x9FF344 +54h): "
                              "the thunk to is_player_current", None, None),
    SOUND_QUEUE_HOOK_VA: (JUMP, "a weapon's delayed sound: the full queue's cave", None, None),
    # Research, patch 53 (in `.srpx`)
    0x1E9B4C: (INLINE, "the game's constructor, `mov eax,ebx / call 0x1EC170`: register (it does both, the call "
                       "as a tail jump with the return address moved past the nops)", None, None),
    0x23B3DE: (INLINE, "update_ui after the teams' points: hud", None, None),
    0x23B351: (JUMP, "update_ui's first message by mode: welcome", None, None),
    # the artefacts, patch 54 (in `.srpx`)
    0x33AE13: (JUMP, "Spring's loader 0x33ACF0, `mov dword [esi+48h],13h` (esi = the config, ebx = `data`): "
                     "loader", None, None),
    0x311BC0: (ENTRY, "add_active_modifiers, the link of Spring's active nodes (eax = the Spring): link_active, "
                      "then the function", None, None),
    0x311DA0: (ENTRY, "remove_active_modifiers, their unlink (stdcall, the Spring): unlink_active, then the "
                      "function", None, None),
    0x3122AA: (JUMP, "Lifebone's enable_active after its remove_affects, `mov ecx,[edi+250h]`: "
                     "lifebone_active", None, None),
    0x313300: (ENTRY, "artefact_base::action (thiscall: pressed, time; inventory::action's vtable +2Ch call): "
                      "fire_kind, a key-up back in the function", None, None),
    0x2DBD89: (JUMP, "the player's stagger 0x2DBCD0, `movss xmm2,[esi+31Ch]` after the hits' sum: stagger",
               None, None),
    0x25F9DD: (JUMP, "Onyx's client update 0x25F640 (ebx = the Onyx +F20h, esi the game), `mov ecx,[ebx-0C30h] / "
                     "push 0BB8h` before set_parameterized_message: onyx_popup, back at 0x25F9E8 with the hint in "
                     "ecx and the time pushed, or at 0x25F9F8 (not shown)", None, None),
    0x312C03: (JUMP, "Onyx's hit callback 0x312BF0 (thiscall: the hit, &damage, &armour piercing; `ret 0Ch`), "
                     "`cmp dword [ecx+280h],3` after its hit type test: onyx_hit, back at 0x312C0A with the "
                     "flags of that test, or `ret 0Ch`", None, None),
    # the kit keys and the match chat option, patch 58 (in `.srpx`)
    0x25CD41: (JUMP, "the game world's on_keyboard_action 0x25CD00, `cmp dword [ebp+10h],1 / jnz near 0x25CF0D` "
                     "(edi = its input handler): keyboard", None, None),
    0x1F6845: (CALLEE, "the game world's constructor's call of the hud's (eax the game world, the hud): hud, "
                       "which calls it and registers the chat option's variable", None, None),
    0x26B916: (CALLEE, "process_incoming_text_message's add_message (stdcall: the handler, the channel, the "
                       "body, the name, the tag): chat, which drops the line or jumps to it", None, None),
    MATCH_KEYS_POLL_SLOT[0]: (SLOT, "the game world's input handler's slot +14h (vtable 0x9FC588; thiscall, the "
                                    "input world; the input world's tick calls it after the poll): poll", None, None),
    # the key that hides the interface, patch 62 (in `.srpx`)
    0x25CED1: (JUMP, "the game world's on_keyboard_action 0x25CD00, its case of action 19h (toggle_replay_ui), "
                     "`mov esi,[esp+10h] / cmp dword [esi+18h],0 / jz 0x25CF0D`: match, back at 0x25CEDB (the "
                     "replay's controls, esi the handler) or 0x25CF0D", None, None),
    0x234AE8: (CALLEE, "initialize_match's Invoke of root.reset (thiscall: the hud movie, the name, the result, "
                       "the arguments, the count; `ret 10h`): reset, which invokes it and shows the hud and the "
                       "icons", None, None),
    0x26CCD2: (CALLEE, "lobby_menu::on_keyboard_action's get_binded_action (eax the binder, edi 2, esi the key, "
                       "the toggle's place pushed; `ret 4`): lobby, which on toggle_replay_ui returns to 0x26CCAE "
                       "instead", None, None),
    # the kill feed's wall, ricochet and pain icons, patch 67 (in `.srpx`)
    0x2DA3D1: (INLINE, "generate_killed_event's `mov [eax+4],bl / mov edx,[esi+120h]` (eax the event, esi the "
                       "victim, ebp the bullet): event, which writes the cause icon, the flags and edx", None, None),
    0x23814F: (INLINE, "on_player_killed's `mov edx,[esp+30h] / mov ecx,[esp+2Ch]` before mastery_icon: row, "
                       "which sets penetration_icon, ricochet_icon and pain_icon and does the two moves", None, None),
    # the baked textures kept and baked or loaded once, patch 70 (in `.srpx`)
    0x1E9455: (CALLEE, "game::game's delete of user_data/streamed_textures (0x1E8DA0, eax the folder's "
                       "buffer_string): start, which deletes it for another revision's folder only", None, None),
    0x131C90: (INLINE, "check_loaded_baked_texture's `cmp eax,[edi+114h]` (eax the CRC loaded, edi the record): "
                       "adopt, which gives a kept file's record (CRC 0) that CRC, then compares", None, None),
    0x12D4CD: (CALLEE, "translate_query's look-up 0x131ED0 (eax the cook, the path; `ret 4`): lookup, which "
                       "tail-jumps to it, or leaves translate_query at 0x12D42D (its own `ret 4`) for a path "
                       "under way", None, None),
    0x12F18F: (CALLEE, "process_compression's `call 0x420789` (ecx 0xAC3334) after its texture's slot: baked, "
                       "which shares the texture and returns ecx+8", None, None),
    0x12DDB4: (CALLEE, "on_baked_texture_loaded's `call 0x420789` (ecx 0xAC3334) after its texture's slot: "
                       "loaded, which shares the texture and returns ecx+8", None, None),
    0xF0D00: (ENTRY, "finish_query (stdcall: the query, the result, two more; `ret 10h`): finish, then the "
                     "function (or the function, then the waiting queries made again)", None, None),
    0x42050: (ENTRY, "the CRC-32 step (eax &the CRC, edx the bytes, the count; `ret 4`): crc32, the same "
                     "value eight bytes a step", None, None),
}
# Explain weight in the lobby; presentation only, no simulation changes.
WEIGHT_UI_HOOKS = (
    (0x22110F, bytes.fromhex("8b8608010000"), "weight_ui_config", 0xE9),
    (0x2222EB, bytes.fromhex("e810f32e00"), "weight_ui_display", 0xE8),
)
INSTALLS[0x22110F] = (JUMP, "cache player stamina config for this lobby; preserve machine, replay mov eax,[esi+108h]", None, None)
INSTALLS[0x2222EB] = (CALLEE, "weight GFx Invoke: same current/capacity plus weight penalties and sprint state; ret 16", None, None)

# The bodies a patch replaces in place, at a function's entry: {VA: size}.
BODIES = {READ_ORDER_VA: READ_ORDER_SIZE}
# A vtable our code builds and installs at run time (a rule's, a cook's):
# the game calls through its slots, so each slot of ours must pop what the
# game's own vtable pops in the same place (the self-test's generalisation of
# check 53, test_call_shapes: our slot's function read from the built bytes,
# up to the next symbol, the model's from the original exe; a slot that holds
# the game's own function is the model itself). (what, the .section.json of
# the arena's block whose code holds it, its symbol (the RTTI locator, the slots after it),
# the model vtable's first slot or None (every slot must be a game function),
# the slots, how many of them are ours). Research's rule slot 16
# (rn_deserialize) popped 4 where tdm's pops 8 in #297's first build and
# crashed the game at its first sync: this is the check that catches it, for
# every patch. Every `*_vtable` symbol of the arena's link must be here
# (audit_vtables).
#
# The last field: {slot: (check script, case)} for each slot of ours, the
# case of that patch's Unicorn check that reaches the slot through the
# game's own call site (the loop or dispatcher that calls it in the game,
# with what the game passes there), not by calling our function directly.
# A `ret` comparison cannot see the order of the arguments: Research's slot 7
# took (type, time, data) where the game passes (type, data, time) (#297's
# review). The self-test fails a slot of ours with no such case, or a case
# its script does not have (test_call_shapes).
RESEARCH_CHECK = "check_research_026g.py"
INSTALLED_VTABLES = (
    ("Research's rule", "research.code", "rn_rule_vtable", 0xA15DA4, 20, 9, {
        7: (RESEARCH_CHECK, "driven: slot 7, on_event(type, data, time), through the dispatcher 0x2D1EA0"),
        8: (RESEARCH_CHECK, "driven: slot 8, entered(place, time), through the world's loop 0x2D2010"),
        9: (RESEARCH_CHECK, "driven: slot 9, left(place, time), through the world's loop 0x2D2130"),
        11: (RESEARCH_CHECK, "driven: slot 11, set_game_world_core, through the world's loop 0x2D3B40"),
        12: (RESEARCH_CHECK, "driven: slot 12, the blackboard, through the world's loop 0x2D5906"),
        15: (RESEARCH_CHECK, "the game's serialize loop (0x2D2661)"),
        16: (RESEARCH_CHECK, "the game's deserialize loop (0x2D2B42)"),
        17: (RESEARCH_CHECK, "driven: slot 17, fill_match_hud_state(hud, ?), through the hud's loop 0x2D36D0"),
        18: (RESEARCH_CHECK, "driven: slot 18, rule_completed(time), through the world's loop 0x2D2730"),
    }),
    ("Research's cook", "research.code", "rn_cook_vtable", 0x9FD8AC, 11, 2, {
        9: (RESEARCH_CHECK, "driven: the cook's create and size through translate 0x2F4290's calls"),
        10: (RESEARCH_CHECK, "driven: the cook's create and size through translate 0x2F4290's calls"),
    }),
    ("Research's rule's link resolver", "research.code", "rn_resolver_vtable", None, 1, 0, {}),

) + hooks_026g.installed_vtables()
# What our code hands the game at run time to call later (a callback, a
# boost::function, a tick delegate): no write of build() shows them, so each
# is declared with the instruction that hands it over and the game's own
# function it stands in for: (what, the code's name (its .section.bin, or an
# arena block), the bytes before the
# pointer in that instruction (the code holds them once), the model, why
# that model, (check script, the case that calls it through the game's own
# caller)) (the last as INSTALLED_VTABLES' slots).
CALLBACKS = (
    ("the late join's query: query_completed", "late_join", bytes.fromhex("6a00566a008d83"), 0x27AA20,
     "0x27D280 makes the network client's boost member callback of its resource queries; its one other "
     "call (0x27B04C) binds 0x27AA20, the client's own queries_completed (ecx = the client, [esp+4] the "
     "queries_result); the marker is the `push 0 / push esi / push 0 / lea eax,[ebx+query_completed]` "
     "before `push eax` (query_completed's VA follows the lea's `8D 83`)",
     ("check_late_join_026g.py", "driven: query_completed through the game's boost invoker 0x202300")),
    ("a weapon's camouflage bake: bake_done", "weapon_camo", b"\xC7\x40\x08", 0x126830,
     "BIND_MANAGER 0x9ED148 binds a `void (*)(A*, B*)`; the game binds 0x126830 with it at 0x126A2C "
     "(a bake query of query_resources 0xE5B60, as weapon_camo's); the marker is bind's "
     "`mov dword [eax+8], bake_done` (bake_done's VA follows `C7 40 08`)",
     ("check_weapon_camo_026g.py", "the callback deallocates P from the game's allocator")),
    ("Research's tick: rn_tick", "research.code", bytes.fromhex("c78114010000"), 0x317F10,
     "rn_set_core puts it in the world's second tick list as timelimit_rule_core's set_game_world_core "
     "(0x318050) puts its tick 0x317F10; the list's runner 0x2D1D20 calls it with ecx the object and "
     "(dt, time, step); the marker is `mov dword [ecx+114h], rn_tick` (the delegate's function)",
     (RESEARCH_CHECK, "driven: the tick's (dt, time) through the tick list's runner 0x2D1D20")),
)


def is_cave(old):
    """A write into the int3 padding between the game's functions: our code."""
    return len(old) >= 5 and set(old) == {0xCC}


def our_ranges(data, writes):
    """[(lo, hi)] of our code: the executable sections build() adds (`.srp`
    and after; not `.srpd`, state), the caves it wrote into the padding and
    the code over on_hash_mismatch."""
    headers = Headers(data)
    ours = [(IMAGE_BASE + rva, IMAGE_BASE + rva + vsize) for name, rva, vsize, _, _, flags in headers.sections(data)
            if name.startswith(b".srp") and flags & 0x20000000]
    return ours + [(va, va + len(new)) for va, old, new in writes if is_cave(old) or va == HASH_MISMATCH_VA]


def arena_data_ranges():
    """[(lo, hi)] of the arena's data and state blocks (arena_026g.lock.json,
    which every build's link must be): no code is there, so an address of one
    the game is given (a table it reads) is data, not an install."""
    lock = linker.read_lock()
    blocks = lock["blocks"].values() if lock else ()
    return [(int(b["va"], 16), int(b["va"], 16) + b["size"]) for b in blocks if b["kind"] != "code"]


def installs_of(data, writes):
    """{site VA: (how, our VA, the original's bytes from the site)} of every
    function build() made the game call, found at any offset of any write
    into the game's sections: a `call` (E8) or `jmp` (E9) rel32 to our code,
    any other 4 bytes that name our code ("pointer": a slot, a delegate, a
    `push imm32`, a `mov imm32`), and a body replaced in place ("body")."""
    ours = our_ranges(data, writes)
    data_blocks = arena_data_ranges()
    inside = lambda target: any(lo <= target < hi for lo, hi in ours) \
        and not any(lo <= target < hi for lo, hi in data_blocks)
    out = {}
    for va, old, new in writes:
        if is_cave(old) or va == HASH_MISMATCH_VA:
            continue
        if va in BODIES and len(new) == BODIES[va]:
            out[va] = ("body", va, old)
            continue
        taken = set()
        for at in range(len(new) - 4):
            if new[at] in (0xE8, 0xE9) and at + 5 <= len(new):
                target = (va + at + 5 + struct.unpack_from("<i", new, at + 1)[0]) & 0xFFFFFFFF
                if inside(target):
                    out[va + at] = ("call" if new[at] == 0xE8 else "jmp", target, old[at:])
                    taken.update(range(at, at + 5))
        for at in range(len(new) - 3):
            value = struct.unpack_from("<I", new, at)[0]
            if at not in taken and inside(value):
                out[va + at] = ("pointer", value, old[at:])
    return out


def audit_installs(data, writes, installs=None):
    """Every function build() made the game call is declared in INSTALLS
    (or `installs`), with a kind that fits the site; else the build stops."""
    installs = INSTALLS if installs is None else installs
    pushes = re.compile(rb"(?:[\x50-\x57]|\x6A.|\x68.{4})*", re.S)
    for va, (how, target, old) in sorted(installs_of(data, writes).items()):
        if va not in installs:
            sys.exit(f"{va:#x}: a {how} into our code ({target:#x}) with no call shape declared: give it a kind "
                     f"and its model in patch_026g.py's INSTALLS (client_patch.md section 0.4)")
        kind = installs[va][0]
        fits = {"call": (CALLEE, INLINE, ABI), "jmp": (ENTRY, JUMP), "pointer": (SLOT,), "body": (ENTRY,)}[how]
        if kind not in fits:
            sys.exit(f"{va:#x}: a {how} into our code declared {kind}; a {how} is {' or '.join(fits)}")
        if kind == CALLEE and not (len(old) >= 5 and old[-5] == 0xE8 and pushes.fullmatch(old[:-5])):
            sys.exit(f"{va:#x}: declared {CALLEE}, but the original's {old.hex()} is not `[push ...] call`")
        if kind == INLINE and old[0] == 0xE8:
            sys.exit(f"{va:#x}: declared {INLINE}, but the original is a call: {CALLEE}")
        if kind == ABI and not (type(installs[va][2]) is int and installs[va][3]):
            sys.exit(f"{va:#x}: declared {ABI} without the bytes it pops and why")


# The Rust lohooks (hooks_026g.py): each a jmp to its generated entry,
# which goes on past the span or replays it.
for _va, _owner, _what in hooks_026g.lohook_sites():
    INSTALLS[_va] = (JUMP, f"{_what}: {_owner} (Rust lohook)", None, None)


# `.srpx`, the arena of the later patches (client_patch.md section 0.3;
# README "The arena"). The exe's section table has room for twelve entries
# (it is at 210h and the headers end at 400h, where the first section's
# bytes begin); the game's seven, `.srp`, `.srpd`, `.srpc` and `.srpw` take
# eleven. The twelfth is `.srpx`, code, data and writable, on the page after
# `.srpw` (whose three pages end at ARENA_VA). A patch that needs room
# declares its blocks in ARENA, each with its C or NASM sources, and the
# tooling links them (link_026g.py): every source an ELF32 object, all of
# them linked by GNU ld at ARENA_VA, a block an output section, in ARENA's
# order, each at the next multiple of its alignment, its size what its
# sources hold. A patch calls another's functions (the engine library's) by
# their names, and a hook site takes its target from the link by symbol
# (arena_target), so no block keeps a VA of its own: the exe is made whole
# for every release and never rebased, and nothing outside it holds an
# address of the arena. The file holds the whole arena (a state block as
# zeros), only the section's sizes and SizeOfImage grow, and the linked bytes
# must be arena_026g.lock.json's (arena()). With no block declared there
# would be no `.srpx` at all.
#   ArenaBlock(name, kind, align, sources): name "<patch>.<what>", unique;
#   kind "code" (the sources' .text and .rodata, int3 after them), "data"
#   (their .data) or "state" (their .bss: zeros); align a power of two from
#   16, 0x1000 for a page of its own; sources the .c and .asm files.
# A new patch adds its blocks (anywhere: the order is only the layout's) and
# its source files, then `python link_026g.py --write`.
ARENA_NAME = b".srpx\0\0\0"
ARENA_CHARACTERISTICS = 0xE0000060              # code, initialized data, execute, read, write
ARENA_VA = 0xA42D000                            # weapon_camo_va + .srpw's three pages
# The most the arena may take: a block of a runaway size (a state array, a
# table) stops the link instead of growing the image. Raise it on purpose.
ARENA_LIMIT = 0x100000
ArenaBlock = linker.ArenaBlock

ARENA = (
    ArenaBlock("weight_ui.code", "code", 0x1000, ("weight_ui_026g.asm", "weight_ui_026g.c")),
    ArenaBlock("weight_ui.state", "state", 0x1000, ("weight_ui_026g.c",)),
    # Research, patch 53: four aligned asm entries and stubs, then Rust
    ArenaBlock("research.code", "code", 0x1000,
               ("research_026g.asm", "research_arena/Cargo.toml")),
    # its rule's cook and the hud's cache (research_arena's STATE)
    ArenaBlock("research.state", "state", 0x1000, ("research_arena/Cargo.toml",)),
    ArenaBlock("artefacts.code", "code", 0x1000, ("artefacts_026g.asm",)),       # patch 54
    ArenaBlock("engine.code", "code", 0x1000, ("engine_026g.asm",)),             # the engine-function library
    ArenaBlock("match_keys.code", "code", 0x1000, ("match_keys_026g.asm",)),     # patch 58
    ArenaBlock("match_keys.state", "state", 0x1000, ("match_keys_026g.asm",)),   # its option's variable
    ArenaBlock("hide_ui.code", "code", 0x1000, ("hide_ui_026g.asm",)),           # patch 62
    ArenaBlock("hide_ui.data", "data", 0x1000, ("hide_ui_026g.asm",)),           # the options' 41 controls
    ArenaBlock("kill_feed.code", "code", 0x1000, ("kill_feed_026g.asm",)),       # patch 67
    ArenaBlock("bake_cache.code", "code", 0x1000, ("bake_cache_026g.asm", "bake_cache_026g.c")),   # patch 70
    ArenaBlock("bake_cache.state", "state", 0x1000, ("bake_cache_026g.c",)),     # its owners and waiters
    # The Rust exe patches (the framework's example is patch 34, the
    # commit_suicide key): one crate, its generated hook entries, the arena's
    # block patches.code (docs/notes/rust_patches.md, hooks_026g.py).
    ArenaBlock("patches.code", "code", 0x1000, hooks_026g.arena_sources()),
    # the crate's statics: patch 76's corpses and pool, shots and trace
    # (rust/patches/src/ragdoll.rs; vostok's Lock and GameCell)
    ArenaBlock("patches.data", "data", 0x1000, ("rust/patches/Cargo.toml",)),
    ArenaBlock("patches.state", "state", 0x1000, ("rust/patches/Cargo.toml",)),
)


def arena(blocks=None, tools=None):
    """The linked arena (link_026g.Linked) of `blocks` (ARENA when None, or
    the self-test's made-up arenas); the build stops when it cannot be
    linked, or when ARENA's is not arena_026g.lock.json's."""
    default = blocks is None
    blocks = ARENA if default else tuple(blocks)
    if default:
        try:
            hooks_026g.check_layers()                 # raw/vostok/patches (rust_patches.md)
        except hooks_026g.Bad as error:
            sys.exit(f"the Rust patches: {error}")
    try:
        linked = linker.link(blocks, ARENA_VA, ARENA_LIMIT, tools)
    except linker.LinkError as error:
        sys.exit(f".srpx: {error}")
    if default:
        why = linker.lock_differs(linked, linker.read_lock())
        if why is not None:
            sys.exit(f".srpx: the arena linked from its sources is not {linker.LOCK.name}'s: {why}. After a "
                     "change of an arena source: python link_026g.py --write (and commit the lock); with no "
                     "change, the tools differ from the lock's")
    return linked


def arena_target(linked, symbol):
    """The VA of `symbol`, a global of a code block of the arena: what a hook
    calls or jumps to; else the build stops."""
    va = linked.symbols.get(symbol)
    home = next((b for b in linked.blocks.values() if va is not None and b.va <= va < b.va + b.size), None)
    if home is None or home.kind != "code":
        sys.exit(f".srpx: {symbol!r} is no global symbol of a code block of the arena (a hook's target is a "
                 f"`global` of its block's NASM, or a C function)")
    return va


def add_arena(data, linked):
    """Appends `.srpx`, the linked arena's bytes at ARENA_VA, after the last
    section; returns the new file. No arena (None): `data` as it is."""
    if linked is None:
        return data
    headers = Headers(data)
    if IMAGE_BASE + struct.unpack_from("<I", data, headers.size_of_image_at)[0] != ARENA_VA or linked.va != ARENA_VA:
        sys.exit(f".srpx: the image does not end at {ARENA_VA:#x}, where the arena begins (has `.srpw` grown "
                 f"past its three pages?)")
    return add_code_section(data, ARENA_NAME, ARENA_VA, linked.image, ARENA_CHARACTERISTICS)


# Research, mode 5 (patch 53, README; client_patch.md section 3.50;
# docs/notes/research_mode.md): research_026g.asm's four aligned entries and stubs,
# then research_arena and research-rule's Rust code and constant data, in
# the arena's block research.code; the Rust code's state (the
# rule's cook, the hud's cache: rn_state) in research.state, zeroed.
# Four hooks, `call` or `jmp` to an entry:
# - the game's constructor, `mov eax,ebx / call 0x1EC170` at 0x1E9B4C (after
#   its own client rule cooks; ebx = the game): register, which registers the
#   rule's cook (class RESEARCH_CLASS) and does the two instructions;
# - game_world_ui::update_ui, `cmp dword [edi+20h],4 / jne 0x23B3FC` at
#   0x23B3DE (after the teams' points went to the movie): hud, the units on
#   the movie (root.research_update) and their messages, then the compare;
# - update_ui, `cmp eax,4 / jne 0x23B36A` at 0x23B351 (the match's first
#   message by mode): welcome, mode 5's st_start_match_welcome_message_5.
# - update_ui's update_player_icons call at 0x23B207: world markers, before
#   the HUD's 40 ms gate.
# No base relocation is under a site and no branch aims inside one.
RESEARCH_ASM_SIZE = 0x100                      # research_026g.asm, int3 to 0x100: the Rust code after it
RESEARCH_CLASS = 0x1E0                         # research_arena CLASS: the mode configs' rules[2]
RESEARCH_HOOKS = (
    (0x23B207, bytes.fromhex("e804f8ffff"), "research_icons", 0xE8),
    (0x1E9B4C, bytes.fromhex("8bc3e81d260000"), "research_register", 0xE8),  # the game's constructor
    (0x23B3DE, bytes.fromhex("837f20047518"), "research_hud", 0xE8),         # update_ui after the teams' points
    (0x23B351, bytes.fromhex("83f8047514"), "research_welcome", 0xE9),       # update_ui's first message
)
# The engine-function library (engine_026g.py, client_patch.md section 0.5):
# a cdecl thunk per engine function our code calls, each the global symbol
# `engine_...` in the arena's block engine.code. One copy for every patch, so
# one ABI check (check_engine_026g.py) covers every caller.


def arena_symbols(linked):
    """{block: {symbol: VA}} of every symbol the link defined (locals too)."""
    out = {name: {} for name in linked.blocks}
    for name, va, block, _ in linked.every:
        out[block][name] = va
    return out


def audit_vtables(vtables=None, symbols=None):
    """Every `*_vtable` symbol of the arena's link is declared in
    INSTALLED_VTABLES; else the build stops (a vtable our code hands the game
    with no model would go unchecked)."""
    vtables = INSTALLED_VTABLES if vtables is None else vtables
    symbols = arena_symbols(arena()) if symbols is None else symbols
    declared = {(row[1], row[2]) for row in vtables}
    for block, table in sorted(symbols.items()):
        for symbol in sorted(table):
            if symbol.endswith("_vtable") and (block, symbol) not in declared:
                sys.exit(f"{block}: {symbol} is a vtable our code installs with no model: declare it in "
                         f"patch_026g.py's INSTALLED_VTABLES (client_patch.md section 0.4)")


def hook_bytes(base, va, expected, offset, opcode):
    """The bytes a hook site gets: the call or jump to base+offset and nops."""
    return bytes([opcode]) + struct.pack("<i", base + offset - va - 5) + b"\x90" * (len(expected) - 5)


def late_join_hook(va, expected, offset, opcode):
    return hook_bytes(LATE_JOIN_VA, va, expected, offset, opcode)

HOOKS = [
    # network_client::on_match_ready, ebx = network_client: mov ebx,[ebx+1F1C48h]
    (0x27AEA1, bytes.fromhex("8b9b481c1f00")),
    # on_match_packet_received, case 0xF6: cmp byte [g_debug_hash_mismatches],0
    (0x27A505, bytes.fromhex("803d2cd3f10100")),
]


def parse_server(text):
    """The login client has a 64-byte host buffer; reserve its NUL byte."""
    match = re.fullmatch(r"([A-Za-z0-9](?:[A-Za-z0-9.-]{0,61}[A-Za-z0-9])?):([0-9]{1,5})", text)
    if not match or not 1 <= int(match.group(2)) <= 65535:
        sys.exit("server must be host:port (ASCII host up to 63 bytes, port 1..65535)")
    return f"{match.group(1)}:{int(match.group(2))}"


def file_offset(va):
    assert TEXT_VA <= va < TEXT_VA + TEXT_VSIZE
    return va - TEXT_VA + TEXT_FILE


def data_file_offset(va):
    assert DATA_VA <= va < DATA_VA + DATA_FILE_SIZE
    return va - DATA_VA + DATA_FILE


# Every write of build() into the original's sections: (VA, the original's
# bytes, the new ones), in order. audit_installs reads it.
WRITES = []

# The one place build() writes (docs/notes/rust_patches.md, "The build"):
# every write into the original's bytes (an install, a data or constant
# patch, an in-place rewrite, a cave, a base relocation turned off) and every
# section it adds goes through apply() / record_section(), which records its
# owner (the named instance: the install table that holds its VA, a Rust
# patch, or a section) and its span (file offsets), and stops the build when
# two writes cover the same bytes. The PE header's counters (the number of
# sections, SizeOfImage, SizeOfCode, the certificate directory, the
# checksum) are the file's bookkeeping, updated by the section adders, not a
# patch's write. selftest_026g.py replays APPLIED onto the original and
# checks the image is exactly what build() made.
Applied = collections.namedtuple("Applied", "owner kind va at new")
APPLIED = []
# Install tables that hold other named tables' rows: a write in both is the
# aggregate's (one patch's whole install set) - GATHER is the gather events
# and the gather lock, the late join's hooks and patches its many parts.
INSTANCE_AGGREGATES = ("GATHER", "LATE_JOIN_HOOKS", "LATE_JOIN_PATCHES", "FOV_PATCHES", "FORWARD_SCROLL_HOOKS")
_INSTANCE_INDEX = {}
_BUILDING = [False]                # inside build(): apply() records


def _install_rows(value):
    """The install rows a module value holds: (VA, original bytes, ...) or a
    tuple or list of them."""
    is_row = lambda v: isinstance(v, tuple) and len(v) >= 2 and isinstance(v[0], int) \
        and isinstance(v[1], (bytes, bytearray))
    if is_row(value):
        return [value]
    if isinstance(value, (tuple, list)) and value and all(is_row(v) for v in value):
        return list(value)
    return []


def instance_of(va):
    """The named instance a write at `va` belongs to: the Rust patch whose
    hook it is, else the one install table of this module that holds the VA
    (an aggregate over its parts); the build stops on none or two."""
    if not _INSTANCE_INDEX:
        for name, value in globals().items():
            if name.isupper() and name not in ("WRITES", "APPLIED"):
                for row in _install_rows(value):
                    _INSTANCE_INDEX.setdefault(row[0], set()).add(name)
        for site, owner in hooks_026g.install_sites():
            _INSTANCE_INDEX[site] = {f"rust:{owner}"}
    names = _INSTANCE_INDEX.get(va, set())
    names = (names & set(INSTANCE_AGGREGATES)) or names
    if len(names) != 1:
        sys.exit(f"{va:#x}: a write {'in no named instance' if not names else 'of ' + ', '.join(sorted(names))}: "
                 "every write is one install table's (or a Rust patch's)")
    return next(iter(names))


def _claim(owner, kind, va, at, new):
    """Records a write's span; stops on an overlap with any earlier one. A
    write outside build() (a check patching its own copy) is not recorded."""
    if not _BUILDING[0]:
        return
    hi = at + len(new)
    for other in APPLIED:
        if other.at < hi and at < other.at + len(other.new):
            sys.exit(f"{kind} of {owner} at file {at:#x} ({va:#x}) overlaps {other.kind} of {other.owner} at file "
                     f"{other.at:#x} ({other.va:#x}): two writes over the same bytes")
    APPLIED.append(Applied(owner, kind, va, at, bytes(new)))


def apply(data, at, va, expected, new, owner=None):
    """Writes `new` over `expected` at file offset `at` (VA `va`): the build's
    one writer of the original's bytes."""
    found = bytes(data[at:at + len(expected)])
    if found != expected or len(new) != len(expected):
        sys.exit(f"{va:#x}: expected {expected.hex()}, found {found.hex()}")
    if _BUILDING[0]:
        _claim(owner or instance_of(va), "write", va, at, new)
    data[at:at + len(new)] = new
    WRITES.append((va, bytes(expected), bytes(new)))


def instances_text(applied=None):
    """Every named instance and what it wrote (`patch_026g.py <exe> --check
    --installs`): its installs (VA, bytes), relocations turned off and
    sections."""
    applied = APPLIED if applied is None else applied
    lines = []
    for owner in sorted({a.owner for a in applied}):
        mine = [a for a in applied if a.owner == owner]
        lines.append(f"{owner}: {len(mine)}")
        for a in mine:
            where = f"{a.va:#x}" if a.va is not None else "-"
            lines.append(f"    {a.kind:<15} {where:>10}  file {a.at:#x}  {len(a.new)} bytes")
    return "\n".join(lines)


def record_section(owner, at, size, slot):
    """A section build() adds: its header slot and its file bytes, owned."""
    _claim(owner, "section header", None, slot, bytes(40))
    _claim(owner, "section", None, at, bytes(size))


def patch_value(data, va, expected, new):
    """As `patch`, for a value in .data."""
    apply(data, data_file_offset(va), va, expected, new)


def patch_section_value(data, va, expected, new):
    """As `patch`, for a value in any section of the original's (the
    replay client's vtable is in .rdata)."""
    headers = Headers(data)
    for _, rva, vsize, raw, _, _ in headers.sections(data):
        if IMAGE_BASE + rva <= va < IMAGE_BASE + rva + vsize:
            at = va - IMAGE_BASE - rva + raw
            break
    else:
        sys.exit(f"{va:#x}: in no section")
    apply(data, at, va, expected, new)


def patch(data, va, expected, new, owner=None):
    apply(data, file_offset(va), va, expected, new, owner)


class Headers:
    """The few PE header fields the patch reads or changes (PE32 only)."""

    def __init__(self, data):
        self.pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[self.pe:self.pe + 4] != b"PE\0\0":
            sys.exit("not a PE file")
        self.count_at = self.pe + 6
        self.optional = self.pe + 24
        if struct.unpack_from("<H", data, self.optional)[0] != 0x10B:
            sys.exit("not a PE32 image")
        self.size_of_code_at = self.optional + 4
        self.size_of_image_at = self.optional + 56
        self.checksum_at = self.optional + 64
        self.security_at = self.optional + 96 + 8 * 4
        self.reloc_at = self.optional + 96 + 8 * 5
        self.debug_at = self.optional + 96 + 8 * 6
        self.table = self.optional + struct.unpack_from("<H", data, self.pe + 20)[0]
        self.section_alignment, self.file_alignment = struct.unpack_from("<II", data, self.optional + 32)
        self.size_of_headers = struct.unpack_from("<I", data, self.optional + 60)[0]

    def sections(self, data):
        """[(name, rva, virtual size, raw offset, raw size, characteristics)]"""
        out = []
        for i in range(struct.unpack_from("<H", data, self.count_at)[0]):
            o = self.table + 40 * i
            vsize, rva, rsize, raw = struct.unpack_from("<IIII", data, o + 8)
            out.append((bytes(data[o:o + 8]), rva, vsize, raw, rsize, struct.unpack_from("<I", data, o + 36)[0]))
        return out


def pe_checksum(data, checksum_at):
    """The checksum of IMAGEHLP CheckSumMappedFile: a 16-bit one's-complement
    sum of the file with the checksum field as zero, plus the file size."""
    total = 0
    padded = bytes(data) + b"\0" * (len(data) % 2)
    for (word,) in struct.iter_unpack("<H", padded[:checksum_at] + b"\0\0\0\0" + padded[checksum_at + 4:]):
        total += word
    while total >> 16:
        total = (total & 0xFFFF) + (total >> 16)
    return total + len(data)


def neutralize_relocs(data, ranges):
    """Turn HIGHLOW base relocations that overlap a [lo, hi) VA range into
    type 0 (skip). Returns the VAs of the changed entries."""
    headers = Headers(data)
    rva, size = struct.unpack_from("<II", data, headers.reloc_at)
    base_off = None
    for _, srva, vsize, raw, rsize, _ in headers.sections(data):
        if srva <= rva < srva + max(vsize, rsize):
            base_off = rva - srva + raw
    if base_off is None:
        sys.exit("no .reloc section")
    o, end, changed = base_off, base_off + size, []
    while o < end:
        page, block = struct.unpack_from("<II", data, o)
        if block < 8:
            break
        for i in range((block - 8) // 2):
            at = o + 8 + 2 * i
            e = struct.unpack_from("<H", data, at)[0]
            va = IMAGE_BASE + page + (e & 0xFFF)
            if e >> 12 == 3 and any(va + 4 > lo and va < hi for lo, hi in ranges):
                # owned by the write the relocated dword is under
                if _BUILDING[0]:
                    owner = next((w.owner for w in APPLIED if w.kind == "write" and w.va is not None
                                  and w.va <= va < w.va + len(w.new)), None) or instance_of(va)
                    _claim(owner, "relocation", va, at, struct.pack("<H", e & 0xFFF))
                struct.pack_into("<H", data, at, e & 0xFFF)
                changed.append(va)
        o += block
    return changed


def split_code(code):
    sizes = [len(original) for _, original in HOOKS]
    return code[:-sum(sizes)], code[-sum(sizes):], sizes


def add_state_section(data, state_va):
    """Adds `.srpd`, the late join's zeroed, writable page at `state_va`,
    after `.srp`. It has no bytes in the file."""
    headers = Headers(data)
    sections = headers.sections(data)
    name, rva, vsize, _, _, _ = sections[-1]
    align = lambda value, to: (value + to - 1) // to * to
    slot = headers.table + 40 * len(sections)
    if slot + 40 > headers.size_of_headers or any(data[slot:slot + 40]):
        sys.exit("no free section table entry for the late join's state")
    state_rva = state_va - IMAGE_BASE
    if name != SECTION_NAME or state_rva != align(rva + vsize, headers.section_alignment):
        sys.exit("the late join's state does not follow the code")
    _claim(LATE_JOIN_STATE_NAME.rstrip(b"\0").decode(), "section header", None, slot, bytes(40))
    data[slot:slot + 40] = LATE_JOIN_STATE_NAME + struct.pack(
        "<IIIIIIHHI", LATE_JOIN_STATE_SIZE, state_rva, 0, 0, 0, 0, 0, 0, LATE_JOIN_STATE_CHARACTERISTICS)
    struct.pack_into("<H", data, headers.count_at, len(sections) + 1)
    struct.pack_into("<I", data, headers.size_of_image_at, state_rva + LATE_JOIN_STATE_SIZE)
    uninitialized_at = headers.optional + 12
    struct.pack_into("<I", data, uninitialized_at,
                     struct.unpack_from("<I", data, uninitialized_at)[0] + LATE_JOIN_STATE_SIZE)


def add_section(data, cave):
    """Appends the .srp section holding `cave` and moves the Authenticode
    blob behind it. Returns the new file contents."""
    headers = Headers(data)
    sections = headers.sections(data)
    _, last_rva, last_vsize, last_raw, last_rsize, _ = sections[-1]
    align = lambda value, to: (value + to - 1) // to * to
    slot = headers.table + 40 * len(sections)
    if slot + 40 > headers.size_of_headers or any(data[slot:slot + 40]):
        sys.exit("no free section table entry")
    if align(last_rva + last_vsize, headers.section_alignment) != SECTION_RVA \
            or struct.unpack_from("<I", data, headers.size_of_image_at)[0] != SECTION_RVA:
        sys.exit("the image does not end where the new section starts")
    raw = last_raw + last_rsize
    security_offset, security_size = struct.unpack_from("<II", data, headers.security_at)
    # The attribute certificate table is the only thing after the last
    # section, and its data directory holds a FILE offset: it must move with
    # the bytes, and stays the last thing in the file (8-byte aligned), as
    # the PE format asks. vostok::crypt::verify_sign reads it at every start.
    if security_offset != raw or security_offset + security_size != len(data) or raw % headers.file_alignment:
        sys.exit("unexpected layout after the last section")
    if struct.unpack_from("<I", data, security_offset)[0] > security_size:
        sys.exit("the certificate table entry is larger than the table")
    debug_rva, debug_size = struct.unpack_from("<II", data, headers.debug_at)
    # IMAGE_DEBUG_DIRECTORY.PointerToRawData is the other file offset in the
    # headers; it must point before the bytes that move.
    debug_at = next(debug_rva - rva + offset for _, rva, vsize, offset, rsize, _ in sections
                    if rva <= debug_rva < rva + rsize)
    for entry in range(debug_size // 28):
        if struct.unpack_from("<I", data, debug_at + 28 * entry + 24)[0] >= raw:
            sys.exit("debug data after the last section would move")
    raw_size = align(len(cave), headers.file_alignment)
    if (raw + raw_size) % 8:
        sys.exit("the certificate table would lose its 8-byte alignment")

    record_section(SECTION_NAME.rstrip(b"\0").decode(), raw, raw_size, slot)
    out = bytearray(data[:raw]) + cave + b"\xCC" * (raw_size - len(cave)) + data[raw:]
    out[slot:slot + 40] = SECTION_NAME + struct.pack(
        "<IIIIIIHHI", len(cave), SECTION_RVA, raw_size, raw, 0, 0, 0, 0, SECTION_CHARACTERISTICS)
    struct.pack_into("<H", out, headers.count_at, len(sections) + 1)
    struct.pack_into("<I", out, headers.size_of_image_at,
                     align(SECTION_RVA + len(cave), headers.section_alignment))
    struct.pack_into("<I", out, headers.size_of_code_at,
                     struct.unpack_from("<I", data, headers.size_of_code_at)[0] + raw_size)
    struct.pack_into("<I", out, headers.security_at, security_offset + raw_size)
    return out


def add_code_section(data, name, va, code, characteristics):
    """Appends a code section at `va` (the next aligned RVA after the last
    section, the late join's state page) whose file bytes follow the last
    file bytes (.srp's), and moves the Authenticode blob behind them, as
    add_section does. Returns the new file contents."""
    headers = Headers(data)
    sections = headers.sections(data)
    _, last_rva, last_vsize, _, _, _ = sections[-1]
    align = lambda value, to: (value + to - 1) // to * to
    slot = headers.table + 40 * len(sections)
    if slot + 40 > headers.size_of_headers or any(data[slot:slot + 40]):
        sys.exit(f"no free section table entry for {name.rstrip(bytes(1)).decode()}")
    rva = va - IMAGE_BASE
    if rva != align(last_rva + last_vsize, headers.section_alignment) \
            or struct.unpack_from("<I", data, headers.size_of_image_at)[0] != rva:
        sys.exit(f"{name.rstrip(bytes(1)).decode()} does not follow the last section")
    raw = max(offset + size for _, _, _, offset, size, _ in sections)
    security_offset, security_size = struct.unpack_from("<II", data, headers.security_at)
    if security_offset != raw or security_offset + security_size != len(data) or raw % headers.file_alignment:
        sys.exit("unexpected layout after the last section's bytes")
    raw_size = align(len(code), headers.file_alignment)
    if (raw + raw_size) % 8:
        sys.exit("the certificate table would lose its 8-byte alignment")
    record_section(name.rstrip(b"\0").decode(), raw, raw_size, slot)
    out = bytearray(data[:raw]) + code + b"\xCC" * (raw_size - len(code)) + data[raw:]
    out[slot:slot + 40] = name + struct.pack("<IIIIIIHHI", len(code), rva, raw_size, raw, 0, 0, 0, 0, characteristics)
    struct.pack_into("<H", out, headers.count_at, len(sections) + 1)
    struct.pack_into("<I", out, headers.size_of_image_at, align(rva + len(code), headers.section_alignment))
    struct.pack_into("<I", out, headers.size_of_code_at,
                     struct.unpack_from("<I", data, headers.size_of_code_at)[0] + raw_size)
    struct.pack_into("<I", out, headers.security_at, security_offset + raw_size)
    return out


def build(*args, **kwargs):
    """build_image() with every write recorded by apply() (APPLIED, its
    owners and spans, overlaps refused); APPLIED and WRITES stay this build's
    after it returns."""
    WRITES.clear()
    APPLIED.clear()
    _BUILDING[0] = True
    try:
        return build_image(*args, **kwargs)
    finally:
        _BUILDING[0] = False


def build_image(original, mode, code=None, server=None, late_join=None, revision=None, required_patches=None,
                arena_blocks=None):
    """Returns (patched file, description). mode: "cert-only", "section" or
    "hash-mismatch"; code = the assembled .bin for that cave. An optional
    server embeds a default host:port in full builds only. late_join = the
    late-join code (late_join_026g.section.bin), in "section" builds only:
    with it the build is patch revision REVISION, without it
    REVISION_WITHOUT_LATE_JOIN; with it the game also checks no resources.db
    signature, mounts resources.db's patches (mount_patches_026g.section.bin),
    its sign-in says whether %TEMP%\\shell_32.dll exists
    (stale_dll_026g.section.bin), and its transparent effects scroll as in
    0.29 (forward_scroll_026g.section.bin), and the lobby's link starts the
    updater for a replay link alone (replay_link_026g.section.bin), and an
    upgrade taken off names the item it comes off
    (shared_upgrades_026g.section.bin).
    revision: what the build with the late join says instead of REVISION
    (a release's, REVISION to 255). It is the revision byte and nothing
    else: every patch above is chosen by the kind of build, never by its
    number.
    required_patches: the n of each resources.db.patch_<n> (1 to 255) the
    build with the late join requires at its start (patch 49; a release's
    offered patches); none when not given. It is the mount's bitmap and
    nothing else.
    arena_blocks: the blocks of `.srpx` instead of ARENA (the self-test's made-up
    arenas: ARENA's own reordered, or more blocks after them; no lock is
    compared, ARENA's own hooks are made); () for none; the build with the
    late join only."""
    if hashlib.sha256(original).hexdigest() != EXE_SHA256:
        sys.exit("the source is not the original 0.26g survarium.exe (sha256 mismatch)")
    if server is not None:
        server = parse_server(server)
        if mode == "cert-only":
            sys.exit("embedding a server requires a full patch, not --cert-only")
    if late_join is not None:
        if mode != "section":
            sys.exit("the late join needs the default build: its code and its state are sections of their own")
        if len(late_join) <= LATE_JOIN_ASM_SIZE:
            sys.exit("the late-join code is the 4096 bytes of late_join_026g.asm and the C code after them")
    if revision is not None:
        if late_join is None:
            sys.exit(f"--revision is of the default build, with the late join (the others say "
                     f"{REVISION_WITHOUT_LATE_JOIN}, or nothing)")
        if type(revision) is not int or not REVISION <= revision <= 255:
            sys.exit(f"--revision={revision}: a revision from {REVISION} (REVISION, the last release's) to 255")
    if required_patches is not None:
        if late_join is None:
            sys.exit("--required-patches is of the default build, with the late join (the patches' mount)")
        required_patches = sorted(required_patches)
        if any(type(n) is not int or not 1 <= n <= MOUNT_MAX_PATCHES for n in required_patches) \
                or len(set(required_patches)) != len(required_patches):
            sys.exit(f"--required-patches: each patch once, a number from 1 to {MOUNT_MAX_PATCHES}: {required_patches}")
    if late_join is None:
        revision = REVISION_WITHOUT_LATE_JOIN
    elif revision is None:
        revision = REVISION
    if late_join is not None:
        stale_dll = open(stale_dll_bin(), "rb").read()
        mount_patches = open(mount_patches_bin(), "rb").read()
        state = late_join_state_va(late_join) + MOUNT_STATE_OFFSET
        if mount_patches[-4:] != struct.pack("<I", state):
            sys.exit(f"mount_patches_026g.section.bin keeps its state elsewhere: assemble it with -DSTATE={state:#x}")
        if mount_patches[MOUNTED_OFFSET:MOUNTED_OFFSET + 5] != bytes.fromhex("e800000000") \
                or mount_patches[MOUNT_ALL_OFFSET] != 0x60:
            sys.exit("mount_patches_026g.section.bin: its entries are not where patch_026g.py calls them")
        needed = len(mount_patches) - 4 - MOUNT_REQUIRED_SIZE
        if mount_patches[needed - len(MOUNT_UNREADABLE):needed] != MOUNT_UNREADABLE \
                or any(mount_patches[needed:needed + MOUNT_REQUIRED_SIZE]):
            sys.exit("mount_patches_026g.section.bin: no zeroed bitmap of the required patches after its "
                     "\"Cannot open\" text (patch 49)")
        mount_patches = mount_patches[:needed] + required_bitmap(required_patches or ()) + mount_patches[-4:]
        forward_scroll = open(forward_scroll_bin(), "rb").read()
        if forward_scroll[-4:] != struct.pack("<I", FORWARD_SCROLL_VA):
            sys.exit(f"forward_scroll_026g.section.bin was assembled for another address: -DCAVE={FORWARD_SCROLL_VA:#x}")
        # The first instruction of each entry: `test byte [esp+54h],10h`,
        # `test byte [esp+5Ch],40h`, `pushad`.
        if forward_scroll[FORWARD_SWITCH_OFFSET:FORWARD_SWITCH_OFFSET + 5] != bytes.fromhex("f644245410") \
                or forward_scroll[FORWARD_CONSTANTS_OFFSET:FORWARD_CONSTANTS_OFFSET + 5] != bytes.fromhex("f644245c40") \
                or forward_scroll[SCENE_SLOMO_OFFSET] != 0x60:
            sys.exit("forward_scroll_026g.section.bin: its entries are not where patch_026g.py jumps to them")
    data = bytearray(original)
    linked = None
    WRITES.clear()
    patch(data, *CERT)
    if mode == "cert-only":
        return data, "certificate check only"

    cave_va = CAVES[mode]
    cave, hooks, sizes = split_code(code)
    patch(data, *MUTEX)
    patch(data, *MEMORY_CHECK)
    patch(data, *HASH_INTERVAL)
    rewritten = [(HASH_INTERVAL_BYTE_VA, HASH_INTERVAL_BYTE_VA + 1),
                 (MEMORY_CHECK[0], MEMORY_CHECK[0] + 1)]
    patch(data, *revision_site(revision))
    rewritten.append((REVISION_VA, REVISION_VA + len(REVISION_ORIGINAL)))
    upload_size = len(cave)
    if server is not None:
        if upload_size != DEFAULT_SERVER_OFFSET:
            sys.exit("embedding a server requires the standard 420-byte snapshot cave")
        stub = open(default_server_bin(mode), "rb").read()
        # The checked-in stub ends with the relative jump to the original
        # server-option read; reject a stub assembled for the other placement.
        if len(stub) < 5 or stub[-5] != 0xE9 or \
                cave_va + upload_size + len(stub) + struct.unpack_from("<i", stub, len(stub) - 4)[0] != DEFAULT_SERVER_RETURN:
            sys.exit("the default-server stub has the wrong return target")
        target = cave_va + upload_size
        va, expected = DEFAULT_SERVER_HOOK
        hook = b"\xE9" + struct.pack("<i", target - va - 5) + b"\x90" * (len(expected) - 5)
        patch(data, va, expected, hook)
        rewritten.append((va, va + len(expected)))
        cave += stub + server.encode("ascii") + b"\0"
    for site in GATHER:
        patch(data, *site)
        rewritten.append((site[0], site[0] + len(site[1])))
    patch(data, *MOUNT_NO_ROOT)
    rewritten.append((MOUNT_NO_ROOT[0], MOUNT_NO_ROOT[0] + len(MOUNT_NO_ROOT[1])))
    # Not in `rewritten`: the relocation of this address must stay active.
    patch(data, *JOURNAL_DEFAULT)
    # Every build: the hud gets its player's name, which patch 14's default
    # had withheld (patch 43; no relocation under the two bytes).
    patch(data, *TAB_SELF)
    rewritten.append((TAB_SELF[0], TAB_SELF[0] + len(TAB_SELF[1])))
    # Every build: a weapon shown while aimed adds its body (patch 91).
    patch(data, *WEAPON_SHOW)
    rewritten.append((WEAPON_SHOW[0], WEAPON_SHOW[0] + len(WEAPON_SHOW[1])))
    # Every build: `stage.focus` reads null for a focused object without its
    # ActionScript object (patch 52; no relocation under the 46 bytes).
    patch(data, *STAGE_FOCUS)
    for va, expected, new in KNOWLEDGE_BASE:
        patch_section_value(data, va, expected, new)
    rewritten.extend((va, va + len(expected)) for va, expected, _ in KNOWLEDGE_BASE[1:])
    rewritten.append((STAGE_FOCUS[0], STAGE_FOCUS[0] + len(STAGE_FOCUS[1])))
    if mode == "hash-mismatch":
        if len(cave) > HASH_MISMATCH_SIZE:
            sys.exit(f"the cave code is {len(cave)} bytes; {HASH_MISMATCH_SIZE} fit")
        at = file_offset(HASH_MISMATCH_VA)
        if hashlib.sha256(data[at:at + HASH_MISMATCH_REGION]).hexdigest() != HASH_MISMATCH_SHA256:
            sys.exit(f"{HASH_MISMATCH_VA:#x}: on_hash_mismatch is not the original")
        apply(data, at, HASH_MISMATCH_VA, bytes(data[at:at + HASH_MISMATCH_REGION]),
              cave + b"\xCC" * (HASH_MISMATCH_REGION - len(cave)), owner="HASH_MISMATCH")
        rewritten.append((HASH_MISMATCH_VA, HASH_MISMATCH_VA + HASH_MISMATCH_REGION))
        where = "over on_hash_mismatch"
    else:
        if len(cave) > LATE_JOIN_OFFSET:
            sys.exit(f"the cave code is {len(cave)} bytes; one page fits")
        where = f"in the new section .srp at {SECTION_VA:#x}"

    pos = 0
    for (va, original_bytes), size in zip(HOOKS, sizes):
        hook = hooks[pos:pos + size]
        pos += size
        if hook[0] != 0xE9 or set(hook[5:]) != {0x90}:
            sys.exit(f"{va:#x}: the hook is not jmp + nops: {hook.hex()}")
        target_va = va + 5 + struct.unpack_from("<i", hook, 1)[0]
        if not cave_va <= target_va < cave_va + len(cave):
            sys.exit(f"{va:#x}: the hook jumps to {target_va:#x}, outside the cave "
                     f"(was the .bin assembled with -DCAVE={cave_va:#x}?)")
        if late_join is not None and va == LATE_JOIN_READY_VA:
            # The late join's ready_hook first; it jumps on to `target_va`.
            if target_va != SECTION_VA:
                sys.exit(f"{va:#x}: the late join expects the upload's ready hook at the cave's start")
            hook = late_join_hook(va, original_bytes, LATE_JOIN_READY_OFFSET, 0xE9)
        patch(data, va, original_bytes, hook)
        rewritten.append((va, va + size))
    rewritten += [(CERT[0], CERT[0] + len(CERT[1])), (MUTEX[0], MUTEX[0] + len(MUTEX[1]))]
    relocs = neutralize_relocs(data, rewritten)
    if late_join is not None:
        sites = []
        for va, expected, new in LATE_JOIN_PATCHES:
            patch(data, va, expected, new)
            sites.append((va, va + len(expected)))
        for va, expected, offset, opcode in LATE_JOIN_HOOKS:
            patch(data, va, expected, late_join_hook(va, expected, offset, opcode))
            sites.append((va, va + len(expected)))
        patch(data, *RUN_UNFOCUSED)
        sites.append((RUN_UNFOCUSED_VA, RUN_UNFOCUSED_VA + len(RUN_UNFOCUSED[1])))
        patch(data, *PREGAME_PROGRESS)
        sites.append((PREGAME_PROGRESS_VA, PREGAME_PROGRESS_VA + len(PREGAME_PROGRESS[1])))
        patch(data, *TDM_DEATHS)
        sites.append((TDM_DEATHS[0], TDM_DEATHS[0] + len(TDM_DEATHS[1])))
        patch(data, *BAKED_TEXTURE_MISSING)
        sites.append((BAKED_TEXTURE_MISSING[0], BAKED_TEXTURE_MISSING[0] + len(BAKED_TEXTURE_MISSING[1])))
        patch(data, *AI_ARENA)
        sites.append((AI_ARENA_VA, AI_ARENA_VA + len(AI_ARENA[1])))
        for site in MEMORY_DEFAULTS:
            patch(data, *site)
            sites.append((site[0], site[0] + len(site[1])))
        patch(data, *SPOT_COOLDOWN)
        sites.append((SPOT_COOLDOWN[0], SPOT_COOLDOWN[0] + len(SPOT_COOLDOWN[1])))
        for site in FALL_GRAVITY:
            patch(data, *site)
            sites.append((site[0], site[0] + len(site[1])))
        for site in ARTEFACTS_SIZES:
            patch(data, *site)
            sites.append((site[0], site[0] + len(site[1])))
        patch(data, *REPLAY_REATTACH_THUNK)
        sites.append((REPLAY_REATTACH_THUNK[0], REPLAY_REATTACH_THUNK[0] + len(REPLAY_REATTACH_THUNK[1])))
        # An address: its relocation stays.
        patch_section_value(data, *REPLAY_REATTACH_SLOT)
        for site in (SOUND_QUEUE_CAVE_SITE, SOUND_QUEUE_HOOK):
            patch(data, *site)
            sites.append((site[0], site[0] + len(site[1])))
        patch(data, *REMOVE_MODEL)
        # The one address in what is replaced: the loader must not add to it.
        if neutralize_relocs(data, [(REMOVE_MODEL_VA, REMOVE_MODEL_VA + len(REMOVE_MODEL[1]))]) != [REMOVE_MODEL_VA + 2]:
            sys.exit("the base relocation of scene::remove_model's branch is not where it was read")
        # None of these instructions holds an address: a relocation under
        # one would mean another exe than the one they were read in.
        if neutralize_relocs(data, sites):
            sys.exit("a base relocation under a late-join site")
        patch_value(data, *CLOCK_FACTOR_PATCH)
        # A number, not an address.
        if neutralize_relocs(data, [(CLOCK_FACTOR_VA, CLOCK_FACTOR_VA + 4)]):
            sys.exit("a base relocation under time_floating_factor's value")
        for va, expected, new in FOV_PATCHES:
            patch(data, va, expected, new)
        # The option's name stays an address the loader adds to; the least's
        # one replaced address must not be added to.
        va, expected, _ = FOV_MINIMUM
        if neutralize_relocs(data, [(va, va + len(expected))]) != [FOV_MINIMUM_RELOC]:
            sys.exit(f"{va:#x}: the base relocation under uro_fov's least is not where it was read")
        if FOV_LOD_STATE_OFFSET < TAB_LIST_STATE_OFFSET + TAB_LIST_STATE_SIZE \
                or FOV_LOD_STATE_OFFSET + FOV_LOD_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the FOV LOD scale does not fit after the scoreboard's state")
        k_address = struct.pack("<I", late_join_state_va(late_join) + FOV_LOD_STATE_OFFSET)
        for va, expected, offset, opcode in FOV_LOD_HOOKS:
            stub = late_join[offset:offset + 32]
            if stub[:len(expected)] != expected or stub.count(k_address) != 2:
                sys.exit(f"the late join's code at {offset} is not patch 40's scale "
                         f"(its k at {late_join_state_va(late_join) + FOV_LOD_STATE_OFFSET:#x})")
            patch(data, va, expected, late_join_hook(va, expected, offset, opcode))
        if sorted(neutralize_relocs(data, [(va, va + len(expected)) for va, expected, _, _ in FOV_LOD_HOOKS])) \
                != FOV_LOD_RELOCS:
            sys.exit("the base relocations under the FOV LOD sites are not where they were read")
        pickup = open(pickup_bin(), "rb").read()
        if len(cave) > PICKUP_OFFSET or PICKUP_OFFSET + len(pickup) > LATE_JOIN_OFFSET:
            sys.exit("the upload code and the pickup code do not fit before the late join")
        patch(data, *PICKUP_FRAME)
        sites = [(PICKUP_FRAME[0], PICKUP_FRAME[0] + len(PICKUP_FRAME[1]))]
        for va, expected, offset, opcode in PICKUP_HOOKS:
            patch(data, va, expected, hook_bytes(PICKUP_VA, va, expected, offset, opcode))
            sites.append((va, va + len(expected)))
        if neutralize_relocs(data, sites) != [PICKUP_RELOC]:
            sys.exit("the base relocations under the pickup sites are not where they were read")
        seek_clear = open(seek_clear_bin(), "rb").read()
        if PICKUP_OFFSET + len(pickup) > SEEK_CLEAR_OFFSET or SEEK_CLEAR_OFFSET + len(seek_clear) > LATE_JOIN_OFFSET:
            sys.exit("the pickup code and the seek code do not fit before the late join")
        va, expected, offset, opcode = SEEK_CLEAR_HOOK
        patch(data, va, expected, hook_bytes(SEEK_CLEAR_VA, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the seek's hook")
        patch(data, *DB_SIGNATURE)
        if neutralize_relocs(data, [(DB_SIGNATURE_VA, DB_SIGNATURE_VA + len(DB_SIGNATURE[1]))]):
            sys.exit("a base relocation under verify_sign's call")
        patch(data, *STALE_DLL)
        if neutralize_relocs(data, [(STALE_DLL_VA, STALE_DLL_VA + len(STALE_DLL[1]))]):
            sys.exit("a base relocation under the version field's last two writes")
        kit_change = open(kit_change_bin(), "rb").read()
        if STALE_DLL_OFFSET + len(stale_dll) > KIT_CHANGE_OFFSET \
                or KIT_CHANGE_OFFSET + len(kit_change) > LATE_JOIN_OFFSET:
            sys.exit("the shell_32.dll check and the kit change do not fit before the late join")
        # The first instruction of each entry: `pushad`, `cmp dword
        # [esi+1F1C60h],3`, `call 0x23A1A0`, `cmp byte [ebp+33Ch],0`.
        view, alive, update = (offset for _, _, offset, _ in KIT_CHANGE_HOOKS[1:])
        to_ui = b"\xE8" + struct.pack("<i", 0x23A1A0 - (KIT_CHANGE_VA + alive) - 5)
        if kit_change[0] != 0x60 \
                or kit_change[view:view + 7] != bytes.fromhex("83be601c1f0003") \
                or kit_change[alive:alive + 5] != to_ui \
                or kit_change[update:update + 7] != bytes.fromhex("80bd3c03000000"):
            sys.exit(f"kit_change_026g.section.bin: its entries are not where patch_026g.py jumps to them "
                     f"(assembled with -DCAVE={KIT_CHANGE_VA:#x}?)")
        for va, expected, offset, opcode in KIT_CHANGE_HOOKS:
            patch(data, va, expected, hook_bytes(KIT_CHANGE_VA, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the kit change")
        patch(data, *KIT_PICK)
        if neutralize_relocs(data, [(KIT_PICK[0], KIT_PICK[0] + len(KIT_PICK[1]))]):
            sys.exit("a base relocation under the hiding of the kits after a pick")
        warmup_look = open(warmup_look_bin(), "rb").read()
        warmup_limits = open(warmup_limits_bin(), "rb").read()
        # warmup_look begins `cmp byte [ecx+2E3h],0` and keeps its state where
        # the late join's page has room for it; it calls warmup_limits'
        # drag_end, which begins `mov ecx,[ebx]`; warmup_limits begins
        # `movd eax,xmm0`.
        if warmup_look[:7] != bytes.fromhex("80b9e302000000") \
                or warmup_look_state(warmup_look) != late_join_state_va(late_join) + WARMUP_LOOK_STATE_OFFSET \
                or warmup_look_calls(warmup_look) != [WARMUP_LIMITS_VA + WARMUP_DRAG_END_OFFSET] \
                or warmup_limits[WARMUP_DRAG_END_OFFSET:WARMUP_DRAG_END_OFFSET + 2] != bytes.fromhex("8b0b") \
                or warmup_limits[WARMUP_LIMITS_ENTRY_OFFSET:WARMUP_LIMITS_ENTRY_OFFSET + 4] != bytes.fromhex("660f7ec0"):
            sys.exit(f"warmup_look_026g.section.bin or warmup_limits_026g.section.bin: their entries or the state "
                     f"are not where patch_026g.py puts them (assembled with -DCAVE={WARMUP_LOOK_VA:#x} "
                     f"-DSTATE={late_join_state_va(late_join) + WARMUP_LOOK_STATE_OFFSET:#x} "
                     f"-DLIMITS={WARMUP_LIMITS_VA:#x} and -DCAVE={WARMUP_LIMITS_VA:#x}?)")
        if WEAPON_PREVIEW_STATE_OFFSET + WEAPON_PREVIEW_STATE_SIZE > WARMUP_LOOK_STATE_OFFSET \
                or WARMUP_LOOK_STATE_OFFSET + WARMUP_LOOK_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the warmup look's state is not between the weapon preview's and the page's end")
        warmup_keys = open(warmup_keys_bin(), "rb").read()
        # warmup_keys begins `mov eax,[esp+8]` and ends with the ten bytes it
        # replaced and the jump back.
        tail = WARMUP_KEYS_HOOK[1] + b"\xE9" + struct.pack(
            "<i", WARMUP_KEYS_HOOK[0] + len(WARMUP_KEYS_HOOK[1]) - (WARMUP_KEYS_VA + len(warmup_keys)))
        if warmup_keys[:4] != bytes.fromhex("8b442408") or not warmup_keys.endswith(tail) \
                or warmup_keys_state(warmup_keys) != late_join_state_va(late_join) + WARMUP_KEYS_STATE_OFFSET:
            sys.exit(f"warmup_keys_026g.section.bin: its entry, its end or its state are not where patch_026g.py "
                     f"puts them (assembled with -DCAVE={WARMUP_KEYS_VA:#x} "
                     f"-DSTATE={late_join_state_va(late_join) + WARMUP_KEYS_STATE_OFFSET:#x}?)")
        if WARMUP_LOOK_STATE_OFFSET + WARMUP_LOOK_STATE_SIZE > WARMUP_KEYS_STATE_OFFSET \
                or WARMUP_KEYS_STATE_OFFSET + WARMUP_KEYS_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the warmup keys' state is not between the warmup look's and the page's end")
        # Patch 46: the mouse move's jump goes to death_look, which goes on
        # to warmup_look; update_camera's call of the view's update calls
        # death_view.
        death_camera = open(death_camera_bin(), "rb").read()
        death_va = death_camera_va(late_join)
        death_state = late_join_state_va(late_join) + DEATH_CAMERA_STATE_OFFSET
        look_state = late_join_state_va(late_join) + WARMUP_LOOK_STATE_OFFSET
        if death_camera[DEATH_LOOK_OFFSET:DEATH_LOOK_OFFSET + 7] != bytes.fromhex("80b9e302000000") \
                or death_camera[DEATH_VIEW_OFFSET:DEATH_VIEW_OFFSET + 4] != bytes.fromhex("8b442404") \
                or death_camera_exits(death_camera, death_va, DEATH_CAMERA_EXITS) != DEATH_CAMERA_EXITS_EXPECTED \
                or death_camera.count(b"\xBB" + struct.pack("<I", look_state)) != 1 \
                or not death_camera.count(struct.pack("<I", death_state)) \
                or not death_camera.count(struct.pack("<I", death_state + 4)):
            sys.exit(f"death_camera_026g.section.bin: its entries, its exits or its states are not where "
                     f"patch_026g.py puts them (assembled with -DCAVE={death_va:#x} -DSTATE={death_state:#x} "
                     f"-DLOOK={WARMUP_LOOK_VA:#x} -DLOOK_STATE={look_state:#x} "
                     f"-DDRAG_END={WARMUP_LIMITS_VA + WARMUP_DRAG_END_OFFSET:#x}?)")
        if FOV_LOD_STATE_OFFSET + FOV_LOD_STATE_SIZE > DEATH_CAMERA_STATE_OFFSET \
                or DEATH_CAMERA_STATE_OFFSET + DEATH_CAMERA_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the death camera's state is not between the FOV LOD scale's and the page's end")
        for (va, expected, offset, opcode), base in ((WARMUP_LOOK_HOOK, death_va + DEATH_LOOK_OFFSET),
                                                     (WARMUP_LIMITS_HOOK, WARMUP_LIMITS_VA),
                                                     (WARMUP_KEYS_HOOK, WARMUP_KEYS_VA),
                                                     (DEATH_VIEW_HOOK, death_va)):
            patch(data, va, expected, hook_bytes(base, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the warmup or the death camera")
        read_order = open(read_order_bin(), "rb").read()
        at = file_offset(READ_ORDER_VA)
        if len(read_order) != READ_ORDER_SIZE:
            sys.exit(f"read_order_026g.bin is {len(read_order)} bytes, not {READ_ORDER_SIZE:#x}")
        if hashlib.sha256(data[at:at + READ_ORDER_SIZE]).hexdigest() != READ_ORDER_SHA256:
            sys.exit(f"{READ_ORDER_VA:#x}: hdd_manager_sorting_predicate is not the original")
        apply(data, at, READ_ORDER_VA, bytes(data[at:at + READ_ORDER_SIZE]), read_order, owner="READ_ORDER")
        if neutralize_relocs(data, [(READ_ORDER_VA, READ_ORDER_VA + READ_ORDER_SIZE)]):
            sys.exit("a base relocation under hdd_manager_sorting_predicate")
        weapon_preview = open(weapon_preview_bin(), "rb").read()
        if LATE_JOIN_OFFSET + len(late_join) > WEAPON_PREVIEW_OFFSET \
                or WEAPON_PREVIEW_VA + len(weapon_preview) > late_join_state_va(late_join):
            sys.exit("the late join and the weapon preview do not fit before the late join's state page")
        if MOUNT_STATE_OFFSET + MOUNT_STATE_SIZE > WEAPON_PREVIEW_STATE_OFFSET \
                or WEAPON_PREVIEW_STATE_OFFSET + WEAPON_PREVIEW_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the weapon preview's state is not between the patches' mount's and the page's end")
        if weapon_preview_state(weapon_preview) != late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET \
                or any(weapon_preview[offset] != first for (_, _, offset, _), first
                       in zip(WEAPON_PREVIEW_HOOKS, (0x60, 0x60, 0x50, 0x50, 0x50, 0x50))):
            sys.exit(f"weapon_preview_026g.section.bin: its entries or its state are not where patch_026g.py "
                     f"puts them (assembled with -DCAVE={WEAPON_PREVIEW_VA:#x} "
                     f"-DSTATE={late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET:#x}?)")
        stash_va = weapon_camo_va(late_join) + WEAPON_STASH_OFFSET
        upgrade_preview_va = weapon_camo_va(late_join) + UPGRADE_PREVIEW_OFFSET
        if any(weapon_preview[at] != 0xE8
               or WEAPON_PREVIEW_VA + at + 5 + struct.unpack_from("<i", weapon_preview, at + 1)[0]
               != upgrade_preview_va + UPGRADE_PREVIEW_FIND for at in WEAPON_PREVIEW_FIND_CALLS):
            sys.exit(f"weapon_preview_026g.section.bin does not call upgrade_preview_026g.asm "
                     f"(-DFIND={upgrade_preview_va + UPGRADE_PREVIEW_FIND:#x}?)")
        for va, expected, offset, opcode in WEAPON_PREVIEW_HOOKS:
            if va in WEAPON_STASH_ENTRIES:              # through the stash's entry, which goes on to patch 32's
                patch(data, va, expected, hook_bytes(stash_va, va, expected, WEAPON_STASH_ENTRIES[va][0], opcode))
            else:
                patch(data, va, expected, hook_bytes(WEAPON_PREVIEW_VA, va, expected, offset, opcode))
        if sorted(neutralize_relocs(data, [(va, va + len(expected)) for va, expected, _, _ in WEAPON_PREVIEW_HOOKS])) \
                != WEAPON_PREVIEW_RELOCS:
            sys.exit("the base relocations under the weapon preview's sites are not where they were read")
        weapon_draw = open(weapon_draw_bin(), "rb").read()
        if FORWARD_SCROLL_OFFSET + len(forward_scroll) > WEAPON_DRAW_OFFSET \
                or WEAPON_DRAW_OFFSET + len(weapon_draw) > WARMUP_LOOK_OFFSET \
                or WARMUP_LOOK_OFFSET + len(warmup_look) > STALE_DLL_OFFSET:
            sys.exit("the weapon preview's draw and the warmup look do not fit between the forward effect's "
                     "scrolling and the shell_32.dll check")
        if weapon_draw_state(weapon_draw) != late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET \
                or weapon_draw[0] != 0x50:
            sys.exit(f"weapon_draw_026g.section.bin: its entry or its state is not where patch_026g.py puts "
                     f"them (assembled with -DCAVE={WEAPON_DRAW_VA:#x} "
                     f"-DSTATE={late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET:#x}?)")
        va, expected, offset, opcode = WEAPON_DRAW_HOOK
        patch(data, va, expected, hook_bytes(WEAPON_DRAW_VA, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the weapon preview's draw site")
        if PICKUP_OFFSET + len(pickup) > WARMUP_KEYS_OFFSET \
                or WARMUP_KEYS_OFFSET + len(warmup_keys) > SEEK_CLEAR_OFFSET:
            sys.exit("the pickup code, the warmup keys and the seek code do not fit")
        if late_join[TAB_ROW_OFFSET:TAB_ROW_OFFSET + len(TAB_ROW_START)] != TAB_ROW_START \
                or late_join[TAB_SHOWN_OFFSET:TAB_SHOWN_OFFSET + len(TAB_SHOWN_START)] != TAB_SHOWN_START:
            sys.exit("late_join_026g.section.bin: the scoreboard's entries (tab_row, tab_shown) or its state are "
                     "not where patch_026g.py puts them")
        if WARMUP_KEYS_STATE_OFFSET + WARMUP_KEYS_STATE_SIZE > TAB_LIST_STATE_OFFSET \
                or TAB_LIST_STATE_OFFSET + TAB_LIST_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the scoreboard's state is not between the warmup keys' and the page's end")
        sites = []
        for va, expected, offset, opcode in TAB_LIST_HOOKS:
            patch(data, va, expected, late_join_hook(va, expected, offset, opcode))
            sites.append((va, va + len(expected)))
        patch(data, *TAB_LIST_LOOP)
        sites.append((TAB_LIST_LOOP_VA, TAB_LIST_LOOP_VA + len(TAB_LIST_LOOP[1])))
        if neutralize_relocs(data, sites):
            sys.exit("a base relocation under a site of the scoreboard's rows")
        for site in (MOUNT_QUERY, MOUNTED):
            patch(data, *site)
            if neutralize_relocs(data, [(site[0], site[0] + len(site[1]))]):
                sys.exit(f"{site[0]:#x}: a base relocation under a hook of the patches' mount")
        for va, expected, offset, opcode in FORWARD_SCROLL_HOOKS:
            patch(data, va, expected, hook_bytes(FORWARD_SCROLL_VA, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the forward effect's scrolling")
        if SEEK_CLEAR_OFFSET + len(seek_clear) > WARMUP_LIMITS_OFFSET \
                or WARMUP_LIMITS_OFFSET + len(warmup_limits) > FORWARD_SCROLL_OFFSET \
                or FORWARD_SCROLL_OFFSET + len(forward_scroll) > STALE_DLL_OFFSET \
                or STALE_DLL_OFFSET + len(stale_dll) > LATE_JOIN_OFFSET:
            sys.exit("the seek code, the warmup camera's limits, the forward effect's scrolling and the "
                     "shell_32.dll check do not fit before the late join")
        if len(cave) > MOUNT_PATCHES_OFFSET or MOUNT_PATCHES_OFFSET + len(mount_patches) > PICKUP_OFFSET:
            sys.exit("the upload code (and the default server) and the patches' mount do not fit before the pickup code")
        if MOUNT_STATE_OFFSET + MOUNT_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the patches' mount keeps more state than the late join's page has")
        replay_link = open(replay_link_bin(), "rb").read()
        if len(cave) > REPLAY_LINK_OFFSET or REPLAY_LINK_OFFSET + len(replay_link) > MOUNT_PATCHES_OFFSET:
            sys.exit("the upload code (and the default server), the replay link code and the patches' mount "
                     "do not fit before the pickup code")
        va, expected, offset, opcode = REPLAY_LINK_HOOK
        patch(data, va, expected, hook_bytes(REPLAY_LINK_VA, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the replay link's hook")
        shared_upgrades = open(shared_upgrades_bin(), "rb").read()
        if MOUNT_PATCHES_OFFSET + len(mount_patches) > SHARED_UPGRADES_OFFSET \
                or SHARED_UPGRADES_OFFSET + len(shared_upgrades) > PICKUP_OFFSET:
            sys.exit("the patches' mount, the shared upgrades' code and the pickup code do not fit")
        if any(shared_upgrades[at:at + len(first)] != first for at, first in SHARED_UPGRADES_ENTRIES.items()):
            sys.exit("shared_upgrades_026g.section.bin: its entries are not where patch_026g.py calls them")
        for va, expected, offset, opcode in SHARED_UPGRADES_HOOKS:
            patch(data, va, expected, hook_bytes(SHARED_UPGRADES_VA, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the shared upgrades")
        weapon_camo = open(weapon_camo_bin(), "rb").read()
        camo_va = weapon_camo_va(late_join)
        name_at = weapon_camo.find(WEAPON_CAMO_NAME_FORMAT)
        if not weapon_camo.startswith(WEAPON_CAMO_ENTRY) or name_at < 0 \
                or struct.pack("<I", camo_va + name_at) not in weapon_camo[:name_at]:
            sys.exit(f"weapon_camo_026g.section.bin: its entry is not where patch_026g.py calls it, or it was "
                     f"assembled for another address (-DCAVE={camo_va:#x})")
        weapon_stash = open(weapon_stash_bin(), "rb").read()
        if len(weapon_camo) > WEAPON_STASH_OFFSET or WEAPON_STASH_OFFSET + len(weapon_stash) > 0x1000:
            sys.exit("the weapon camouflage's code and the weapon stash's do not fit in .srpw's page")
        if weapon_preview_state(weapon_stash, camo_va + WEAPON_STASH_OFFSET) \
                != late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET:
            sys.exit(f"weapon_stash_026g.section.bin: assembled for another state "
                     f"(-DSTATE={late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET:#x}?)")
        stash_va = camo_va + WEAPON_STASH_OFFSET
        entries = {va: (entry, preview) for va, (entry, preview) in WEAPON_STASH_ENTRIES.items()}
        jumps = [WEAPON_PREVIEW_VA + preview for _, preview in sorted(entries.values())]
        tails = sorted({stash_va + at + 5 + struct.unpack_from("<i", weapon_stash, at + 1)[0]
                        for at in range(len(weapon_stash) - 4) if weapon_stash[at] == 0xE9}
                       & {WEAPON_PREVIEW_VA + preview for _, preview in entries.values()})
        if any(weapon_stash[entry] != 0xE9 for entry, _ in entries.values()) or tails != sorted(jumps) \
                or weapon_stash[WEAPON_STASH_FIND] != 0xE8:
            sys.exit(f"weapon_stash_026g.section.bin: its entries are not where patch_026g.py puts them, or it "
                     f"does not go on to patch 32's (-DPREVIEW={WEAPON_PREVIEW_VA:#x}?)")
        weapon_table = open(weapon_table_bin(), "rb").read()
        table_va = camo_va + WEAPON_TABLE_OFFSET
        table_state = late_join_state_va(late_join) + WEAPON_TABLE_STATE_OFFSET
        preview_state = late_join_state_va(late_join) + WEAPON_PREVIEW_STATE_OFFSET
        if WEAPON_STASH_OFFSET + len(weapon_stash) > WEAPON_TABLE_OFFSET \
                or WEAPON_TABLE_OFFSET + len(weapon_table) > WEAPON_DECALS_OFFSET:
            sys.exit("the weapon stash's code and the weapon table's do not fit before the weapon decals'")
        if DEATH_CAMERA_STATE_OFFSET + DEATH_CAMERA_STATE_SIZE > WEAPON_TABLE_STATE_OFFSET \
                or WEAPON_TABLE_STATE_OFFSET + WEAPON_TABLE_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the weapon table's state is not between the others' and the page's end")
        # Its state routine as patch 32's; its one read of patch 32's mode is
        # `cmp byte [edi+PREVIEW-STATE],0` (edi = its state).
        if weapon_preview_state(weapon_table, table_va) != table_state \
                or weapon_table.count(b"\x80\xBF" + struct.pack("<i", preview_state - table_state) + b"\x00") != 1 \
                or any(weapon_table[at] != 0xE9 for at in WEAPON_TABLE_ENTRIES):
            sys.exit(f"weapon_table_026g.section.bin: its entries or its state are not where patch_026g.py puts "
                     f"them (assembled with -DCAVE={table_va:#x} -DSTATE={table_state:#x} "
                     f"-DPREVIEW={preview_state:#x}?)")
        if any(weapon_preview[at] != 0xE9
               or WEAPON_PREVIEW_VA + at + 5 + struct.unpack_from("<i", weapon_preview, at + 1)[0] != table_va
               for at in WEAPON_PREVIEW_CAMERA_JUMPS):
            sys.exit(f"weapon_preview_026g.section.bin does not jump to weapon_table_026g.asm (-DCAMERA={table_va:#x}?)")
        weapon_decals = open(weapon_decals_bin(), "rb").read()
        decals_va = camo_va + WEAPON_DECALS_OFFSET
        decals_calls = {decals_va + entry: 0 for entry in WEAPON_CAMO_DECALS_CALLS}
        for at in range(len(weapon_camo) - 4):
            target = camo_va + at + 5 + struct.unpack_from("<i", weapon_camo, at + 1)[0]
            if weapon_camo[at] == 0xE8 and target in decals_calls:
                decals_calls[target] += 1
        if WEAPON_TABLE_OFFSET + len(weapon_table) > WEAPON_DECALS_OFFSET \
                or WEAPON_DECALS_OFFSET + len(weapon_decals) > 0x2000:
            sys.exit("the weapon table's code and the weapon decals' do not fit before the upgrade preview's page")
        press_state = late_join_state_va(late_join) + WEAPON_DECALS_STATE_OFFSET
        if WEAPON_TABLE_STATE_OFFSET + WEAPON_TABLE_STATE_SIZE > WEAPON_DECALS_STATE_OFFSET \
                or WEAPON_DECALS_STATE_OFFSET + WEAPON_DECALS_STATE_SIZE > LATE_JOIN_STATE_SIZE:
            sys.exit("the weapon decals' state is not between the weapon table's and the page's end")
        # its reads of patch 45's `on`, its transform and patch 32's mode; its press
        if any(weapon_decals[at] != 0xE9 for at in WEAPON_DECALS_ENTRIES) \
                or weapon_decals.count(b"\x8D\x15" + struct.pack("<I", table_state + WEAPON_TABLE_MATRIX_OFFSET)) != 1 \
                or weapon_decals.count(b"\xC7\x05" + struct.pack("<II", press_state, 1)) != 1 \
                or list(decals_calls.values()) != [1] * len(WEAPON_CAMO_DECALS_CALLS) \
                or weapon_decals.count(b"\x83\x3D" + struct.pack("<I", table_state) + b"\x00") != 1 \
                or weapon_decals.count(b"\x80\x3D" + struct.pack("<I", preview_state) + b"\x00") != 1:
            sys.exit(f"weapon_decals_026g.section.bin: its entries are not where patch_026g.py and patch 44 call "
                     f"them, or it was assembled for other states (-DCAVE={decals_va:#x} "
                     f"-DPREVIEW={preview_state:#x} -DTABLE={table_state:#x} -DPRESS={press_state:#x}; "
                     f"patch 44 with -DDECALS={decals_va:#x}?)")
        weapon_camo = weapon_camo + b"\xCC" * (WEAPON_STASH_OFFSET - len(weapon_camo)) + weapon_stash
        weapon_camo = weapon_camo + b"\xCC" * (WEAPON_TABLE_OFFSET - len(weapon_camo)) + weapon_table
        weapon_camo = weapon_camo + b"\xCC" * (WEAPON_DECALS_OFFSET - len(weapon_camo)) + weapon_decals
        for va, expected, offset, opcode in WEAPON_DECALS_HOOKS:
            patch(data, va, expected, hook_bytes(decals_va, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the weapon decals")
        upgrade_preview = open(upgrade_preview_bin(), "rb").read()
        # `.srpx`, the arena, linked here: patch 48's kit_apply takes patch
        # 68's weapon_kit_preview_apply (UPGRADE_PREVIEW_KIT_APPLY; 0 without it).
        linked = arena(arena_blocks) if arena_blocks != () else None
        upgrade_preview = with_kit_apply(upgrade_preview, linked)
        upgrade_preview_state = late_join_state_va(late_join) + UPGRADE_PREVIEW_STATE_OFFSET
        if WEAPON_DECALS_OFFSET + len(weapon_decals) > UPGRADE_PREVIEW_OFFSET \
                or len(upgrade_preview) > 0x1000:
            sys.exit("the weapon decals' code and the upgrade preview's do not fit in .srpw")
        if MOUNT_STATE_OFFSET + MOUNT_STATE_SIZE > UPGRADE_PREVIEW_STATE_OFFSET \
                or UPGRADE_PREVIEW_STATE_OFFSET + UPGRADE_PREVIEW_STATE_SIZE > WEAPON_PREVIEW_STATE_OFFSET:
            sys.exit("the upgrade preview's state is not between the patches' mount's and the weapon preview's")
        if upgrade_preview_targets(upgrade_preview, upgrade_preview_va) \
                != (stash_va + WEAPON_STASH_FIND, upgrade_preview_state, preview_state):
            sys.exit(f"upgrade_preview_026g.section.bin: its entries, its state or its find are not where "
                     f"patch_026g.py puts them (assembled with -DCAVE={upgrade_preview_va:#x} "
                     f"-DSTATE={upgrade_preview_state:#x} -DPREVIEW={preview_state:#x} "
                     f"-DFIND={stash_va + WEAPON_STASH_FIND:#x}?)")
        weapon_camo = weapon_camo + b"\xCC" * (UPGRADE_PREVIEW_OFFSET - len(weapon_camo)) + upgrade_preview
        for va, expected, offset, opcode in UPGRADE_PREVIEW_HOOKS:
            patch(data, va, expected, hook_bytes(upgrade_preview_va, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the upgrade preview")
        for va, expected, offset, opcode in WEAPON_TABLE_HOOKS:
            patch(data, va, expected, hook_bytes(table_va, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the weapon table")
        va, expected, offset, opcode = WEAPON_CAMO_HOOK
        patch(data, va, expected, hook_bytes(camo_va, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the weapon camouflage's hook")
        spot = open(spot_bin(), "rb").read()
        if SHARED_UPGRADES_OFFSET + len(shared_upgrades) > SPOT_OFFSET or SPOT_OFFSET + len(spot) > PICKUP_OFFSET:
            sys.exit("the shared upgrades' code, the spot code and the pickup code do not fit")
        # It begins `mov ecx,[esi]; mov ecx,[ecx+0Ch]` (the five bytes it
        # replaces) and leaves by its three jumps.
        exits = [SPOT_VA + at + 5 + struct.unpack_from("<i", spot, at + 1)[0]
                 for at in range(len(spot) - 4) if spot[at] == 0xE9]
        if spot[:5] != SPOT_HOOK[1] or exits != list(SPOT_EXITS):
            sys.exit(f"spot_026g.section.bin was assembled for another address: -DCAVE={SPOT_VA:#x}")
        va, expected, offset, opcode = SPOT_HOOK
        patch(data, va, expected, hook_bytes(SPOT_VA, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the spot's hook")

    if mode == "section":
        if late_join is not None:
            cave = cave + b"\xCC" * (REPLAY_LINK_OFFSET - len(cave)) + replay_link
            cave = cave + b"\xCC" * (MOUNT_PATCHES_OFFSET - len(cave)) + mount_patches
            cave = cave + b"\xCC" * (SHARED_UPGRADES_OFFSET - len(cave)) + shared_upgrades
            cave = cave + b"\xCC" * (SPOT_OFFSET - len(cave)) + spot
            cave = cave + b"\xCC" * (PICKUP_OFFSET - len(cave)) + pickup
            cave = cave + b"\xCC" * (WARMUP_KEYS_OFFSET - len(cave)) + warmup_keys
            cave = cave + b"\xCC" * (SEEK_CLEAR_OFFSET - len(cave)) + seek_clear
            cave = cave + b"\xCC" * (WARMUP_LIMITS_OFFSET - len(cave)) + warmup_limits
            cave = cave + b"\xCC" * (FORWARD_SCROLL_OFFSET - len(cave)) + forward_scroll
            cave = cave + b"\xCC" * (WEAPON_DRAW_OFFSET - len(cave)) + weapon_draw
            cave = cave + b"\xCC" * (WARMUP_LOOK_OFFSET - len(cave)) + warmup_look
            cave = cave + b"\xCC" * (STALE_DLL_OFFSET - len(cave)) + stale_dll
            cave = cave + b"\xCC" * (KIT_CHANGE_OFFSET - len(cave)) + kit_change
            cave = cave + b"\xCC" * (LATE_JOIN_OFFSET - len(cave)) + late_join
            cave = cave + b"\xCC" * (WEAPON_PREVIEW_OFFSET - len(cave)) + weapon_preview
        data = add_section(data, cave)
        if late_join is not None:
            add_state_section(data, late_join_state_va(late_join))
            data = add_code_section(data, DEATH_CAMERA_NAME, death_camera_va(late_join), death_camera,
                                    DEATH_CAMERA_CHARACTERISTICS)
            data = add_code_section(data, WEAPON_CAMO_NAME, weapon_camo_va(late_join), weapon_camo,
                                    WEAPON_CAMO_CHARACTERISTICS)
            # `.srpx`, the arena (linked above, with patch 48's code): each
            # hook's target taken from the link by its symbol.
            if linked is not None:
                for va, expected, symbol, opcode in RESEARCH_HOOKS + ARTEFACTS_HOOKS + MATCH_KEYS_HOOKS + HIDE_UI_HOOKS \
                        + KILL_FEED_HOOKS + BAKE_CACHE_HOOKS + WEIGHT_UI_HOOKS + rust_hooks(original):
                    patch(data, va, expected, hook_bytes(arena_target(linked, symbol), va, expected, 0, opcode))
                    if neutralize_relocs(data, [(va, va + len(expected))]):
                        sys.exit(f"{va:#x}: a base relocation under the hook to {symbol}")
                slot, old, symbol = MATCH_KEYS_POLL_SLOT
                patch_section_value(data, slot, old, struct.pack("<I", arena_target(linked, symbol)))
                for va, old, new in hide_ui_table_writes(linked):
                    patch(data, va, old, new)
                patch_section_value(data, *ARTEFACT_KEY_DEFAULT)
                patch(data, *KILL_FEED_ARGUMENT)
                patch(data, *BAKE_CACHE_KEEP)
                if artefacts_backs(linked) != list(ARTEFACTS_BACKS):
                    sys.exit("artefacts_026g.asm: its entries do not go on where patch_026g.py's ARTEFACTS_BACKS "
                             "says")
            data = add_arena(data, linked)
    if arena_blocks is not None and late_join is None:
        sys.exit("the arena (`.srpx`) is of the build with the late join")
    if mode != "cert-only":
        audit_installs(data, WRITES)
    if late_join is not None and linked is not None:
        audit_vtables(symbols=arena_symbols(linked))
    headers = Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, pe_checksum(data, headers.checksum_at))
    return data, (f"certificate check, mutex, minimum VRAM check, hash interval {HASH_INTERVAL_MS} ms, "
                  f"patch revision {revision}, battery rule, mount without a root, no journal by default, "
                  "stage.focus null for a focused object without its ActionScript object, "
                  f"{upload_size} bytes of upload code {where} "
                  f"({len(relocs)} relocations disabled), {len(HOOKS)} snapshot hooks"
                  + (f", late join ({len(late_join)} bytes of code at {LATE_JOIN_VA:#x}, "
                     f"{len(LATE_JOIN_PATCHES)} sites rewritten, {len(LATE_JOIN_HOOKS) + 1} hooks), "
                     "goes on without the focus, a model not in the scene stays out of it, "
                     f"the clock is brought up at {CLOCK_FACTOR:g} (time_floating_factor), "
                     "the countdown shows the match's team points, "
                     "a spawn gives its weapons a random state of the match's own, "
                     "and the state a new weapon has where a game could not know the kit's last life, "
                     "a first world's kit choices are the world's and in time order, "
                     "a Team Deathmatch rule counts a death after its own end, "
                     f"a field-of-view setting (uro_fov, {FOV_MIN:g} to 100) the camera follows, a scope's view the original's, "
                     f"a battery under the floor one stands on taken (within {PICKUP_ACROSS:g} m across), "
                     "a replay's camera follows its player into another kit, "
                     "a weapon with a full sound queue drops the new sound, "
                     "a replay's seek empties the weapons' sound queues, "
                     "no resources.db signature check, "
                     f"each resources.db.patch_1 to _{MOUNT_MAX_PATCHES} there mounted over resources.db, "
                     "the sign-in says whether %TEMP%\\shell_32.dll exists, "
                     "the transparent effects scroll their textures as in 0.29 (scene_slomo bound to 1.0), "
                     "the lobby's link starts the updater for a replay link and opens nothing else, "
                     "an upgrade taken off names the item it comes off (46 02, 83 46 02, detach_first), "
                     "a kit chosen before the match starts is played at once where the player stands, "
                     "a drag turns the warmup camera and the wheel moves it while the kits are shown, "
                     "the archive reads' order is a strict weak ordering, "
                     f"the ai arena is {AI_ARENA_SIZE >> 20} MiB, "
                     "a weapon's upgrade window shows the weapon, "
                     "the commit_suicide key (K) kills one's own player, "
                     "a killed player falls as a ragdoll (drawn only) that bullets push and bloody, the death screen's camera on its head, "
                     "the scoreboard shows the places in the match and none whose player left, "
                     "a spot has no cooldown and a battery's body spots it, "
                     "a fall gains speed for the time it fell, not for the steps of a split tick, "
                     "a model's LOD and screen factor at a wider field of view are the original's, "
                     "a drag turns the camera on the death screen and the wheel does nothing there, "
                     "a weapon wears the camouflage its item holds, "
                     "a weapon's upgrade window shows it in the air above the lobby's workbench, "
                     "a weapon wears the decals its item holds, placed on it above the workbench, "
                     "an upgrade dropped on a weapon in its window is shown at once, "
                     f"Research (mode 5: rule class {RESEARCH_CLASS:#x}, its hud and messages), "
                     "a Spring's passive and active take aim time from its config and its active takes the bullets' slow away, a Lifebone's active fills "
                     "pain, body and head, and one press fires every artefact of its kind, "
                     "Onyx fired together each 1 s longer per other one and reducing a hit once, "
                     "their activation hint shown once a press for their active time, "
                     "the keys 1, 2 and 3 pick a kit while the kits are shown (none for a key pressed before "
                     "they came or in their first 300 ms), "
                     "an option hides the other players' chat lines in a match, "
                     "F11 (toggle_replay_ui) hides the interface in a match and in the lobby, "
                     "the kill feed tells a kill through a wall, by a ricochet and by pain, "
                     "baked camouflage textures kept from game to game and made once while asked for again, "
                     "a Blackout match has no interface but its Tab view and the players' markers on Tab, "
                     "a weapon's upgrades (two decals, camouflage, scope) in at most three records (the weapon kit), "
                     + (f"it stops at its start without resources.db.patch_{', _'.join(map(str, required_patches))}"
                        if required_patches else "it requires no resources.db patch")
                     if late_join is not None else "")
                  + (f", default login server {server}" if server is not None else ""))


def required_bitmap(numbers):
    """The mount's bitmap of the required patches: bit n of byte n/8."""
    bitmap = bytearray(MOUNT_REQUIRED_SIZE)
    for n in numbers:
        bitmap[n >> 3] |= 1 << (n & 7)
    return bytes(bitmap)


def required_patches_of(exe):
    """The n of every resources.db.patch_<n> a built exe requires (patch
    49): the bitmap right after the mount's "Cannot open" text, which is in
    the exe once; None when it is not (not a build with the patches' mount)."""
    at = exe.find(MOUNT_UNREADABLE)
    if at < 0 or exe.find(MOUNT_UNREADABLE, at + 1) >= 0:
        return None
    bitmap = exe[at + len(MOUNT_UNREADABLE):at + len(MOUNT_UNREADABLE) + MOUNT_REQUIRED_SIZE]
    return [n for n in range(8 * MOUNT_REQUIRED_SIZE) if bitmap[n >> 3] >> (n & 7) & 1]


def parse_required_patches(text):
    """--required-patches=<n>,<n>,...: the numbers, each a whole number
    without a sign or a leading zero, increasing (a list in revision order,
    as release/patches.tsv); build() checks their range."""
    numbers = text.split(",")
    if not all(re.fullmatch(r"[1-9][0-9]{0,2}", n) for n in numbers) \
            or [int(n) for n in numbers] != sorted(set(int(n) for n in numbers)):
        sys.exit(f"--required-patches={text}: patch numbers from 1 to {MOUNT_MAX_PATCHES}, increasing, commas between")
    return [int(n) for n in numbers]


def parse_revision(text):
    """--revision=<n>: n, a whole number without a sign or a leading zero
    (build() checks its range)."""
    if not re.fullmatch(r"[1-9][0-9]{0,2}", text):
        sys.exit(f"--revision={text}: a whole number from {REVISION} to 255")
    return int(text)


def default_bin(mode):
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), f"snapshot_upload_026g.{mode}.bin")


def default_server_bin(mode):
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), f"default_server_026g.{mode}.bin")


def replay_link_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay_link_026g.section.bin")


def shared_upgrades_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "shared_upgrades_026g.section.bin")


def stale_dll_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "stale_dll_026g.section.bin")


def mount_patches_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "mount_patches_026g.section.bin")


def weapon_camo_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_camo_026g.section.bin")


def spot_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "spot_026g.section.bin")


def pickup_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "pickup_below_026g.section.bin")


def forward_scroll_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "forward_scroll_026g.section.bin")


def seek_clear_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "replay_seek_026g.section.bin")


def read_order_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "read_order_026g.bin")


def kit_change_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "kit_change_026g.section.bin")


def warmup_look_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "warmup_look_026g.section.bin")


def warmup_limits_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "warmup_limits_026g.section.bin")


def warmup_keys_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "warmup_keys_026g.section.bin")


def warmup_keys_state(code):
    """The state address warmup_keys_026g.asm was assembled for: its one
    `call $+5` / `pop edx` / `add edx,imm32`; 0 when it is not there once."""
    find = bytes.fromhex("e800000000" "5a" "81c2")
    at = code.find(find)
    if at < 0 or code.find(find, at + 1) >= 0:
        return 0
    return (WARMUP_KEYS_VA + at + 5 + struct.unpack_from("<i", code, at + 8)[0]) & 0xFFFFFFFF


def warmup_look_state(code):
    """The state address warmup_look_026g.asm was assembled for: its one
    `call $+5` / `pop ebx` / `add ebx,imm32`; 0 when it is not there once."""
    find = bytes.fromhex("e800000000" "5b" "81c3")
    at = code.find(find)
    if at < 0 or code.find(find, at + 1) >= 0:
        return 0
    return (WARMUP_LOOK_VA + at + 5 + struct.unpack_from("<i", code, at + 8)[0]) & 0xFFFFFFFF


def warmup_look_calls(code):
    """The target of warmup_look_026g.asm's call at the end of a drag:
    `call rel32` / `xor eax,eax` / `cmp [edi+0Ch],eax`, once; [] when not."""
    find = bytes.fromhex("31c039470c")
    at = code.find(find)
    if at < 5 or code.find(find, at + 1) >= 0 or code[at - 5] != 0xE8:
        return []
    return [(WARMUP_LOOK_VA + at + struct.unpack_from("<i", code, at - 4)[0]) & 0xFFFFFFFF]


def death_camera_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "death_camera_026g.section.bin")


def death_camera_exits(code, va, targets):
    """[(opcode, target)] of each `jmp rel32` (E9) or `call rel32` (E8) in
    death_camera_026g.section.bin, assembled for `va`, whose target is one of
    `targets`: what the code leaves to (a .bin assembled for another place,
    or with other addresses, has none of them)."""
    out = []
    for at in range(len(code) - 4):
        if code[at] in (0xE8, 0xE9):
            target = (va + at + 5 + struct.unpack_from("<i", code, at + 1)[0]) & 0xFFFFFFFF
            if target in targets:
                out.append((code[at], target))
    return sorted(out)


def weapon_stash_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_stash_026g.section.bin")


def weapon_table_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_table_026g.section.bin")


def weapon_decals_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_decals_026g.section.bin")


def upgrade_preview_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "upgrade_preview_026g.section.bin")


def with_kit_apply(code, linked):
    """upgrade_preview_026g.section.bin (`code`) with its kit_apply dword
    (UPGRADE_PREVIEW_KIT_APPLY, zeros in the file) holding patch 68's
    weapon_kit_preview_apply of the arena `linked`; as it is without one."""
    if code[UPGRADE_PREVIEW_KIT_APPLY:UPGRADE_PREVIEW_KIT_APPLY + 4] != bytes(4):
        sys.exit("upgrade_preview_026g.section.bin: its kit_apply dword (+23h) is not the zeros the build fills")
    if linked is None or WEAPON_KIT_PREVIEW_APPLY not in linked.symbols:
        return code
    target = struct.pack("<I", arena_target(linked, WEAPON_KIT_PREVIEW_APPLY))
    return code[:UPGRADE_PREVIEW_KIT_APPLY] + target + code[UPGRADE_PREVIEW_KIT_APPLY + 4:]


def artefacts_backs(linked):
    """Where each of the artefacts' eight entries (ARTEFACTS_HOOKS' symbols)
    goes on: the target of the first `jmp near` back into .text after the
    entry's own code, when the entry is a `jmp near`; else None."""
    block = linked.blocks["artefacts.code"]
    code, va = linker.block_bytes(linked, "artefacts.code"), block.va
    entries = [linked.symbols.get(symbol, 0) - va for _, _, symbol, _ in ARTEFACTS_HOOKS]
    if any(not 0 <= at < len(code) - 4 or code[at] != 0xE9 for at in entries):
        return None
    backs = []
    for at in entries:
        start = at + 5 + struct.unpack_from("<i", code, at + 1)[0]
        target = next((va + i + 5 + struct.unpack_from("<i", code, i + 1)[0] for i in range(start, len(code) - 4)
                       if code[i] == 0xE9 and va + i + 5 + struct.unpack_from("<i", code, i + 1)[0] < SECTION_VA),
                      None)
        backs.append(target)
    return backs


def upgrade_preview_targets(code, va):
    """upgrade_preview_026g.section.bin assembled at `va`: (the find's
    `call`, its state, patch 32's state) when its seven entries are `jmp
    near`, find (+0Fh's) begins `push eax / call <find>` and the state is
    the immediate of pop_op's `mov esi,<state>` (the one after account's
    `call`), patch 32's that of preview's first `mov esi,<state>`; else None."""
    if any(code[at] != 0xE9 for at in UPGRADE_PREVIEW_ENTRIES):
        return None
    target = lambda at: va + at + 5 + struct.unpack_from("<i", code, at + 1)[0] - va
    find = target(UPGRADE_PREVIEW_FIND)
    account = target(UPGRADE_PREVIEW_ENTRIES[2])
    preview = target(UPGRADE_PREVIEW_ENTRIES[0])
    if code[find] != 0x50 or code[find + 1] != 0xE8 or code[account] != 0xE8:
        return None
    pop_op = target(account)
    # relocate: pushad / movzx edx,cl / mov eax,[esp+34h] / call preview
    call_at = code.find(b"\xE8", preview)
    if code[pop_op] != 0x60 or code[pop_op + 1] != 0xBE or call_at < 0:
        return None
    body = target(call_at)
    if code[body] != 0xBE:
        return None
    return ((va + find + 1 + 5 + struct.unpack_from("<i", code, find + 2)[0]) & 0xFFFFFFFF,
            struct.unpack_from("<I", code, pop_op + 2)[0], struct.unpack_from("<I", code, body + 1)[0])


def weapon_preview_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_preview_026g.section.bin")


def weapon_draw_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "weapon_draw_026g.section.bin")


def weapon_draw_state(code):
    """The state address weapon_draw_026g.asm was assembled for: `push eax` /
    `call $+5` / `pop eax` / `cmp byte [eax+disp32],0`; 0 when it is not so."""
    if code[:9] != bytes.fromhex("50e80000000058" "80b8") or code[13:14] != b"\x00":
        return 0
    return WEAPON_DRAW_VA + 6 + struct.unpack_from("<i", code, 9)[0]


def weapon_preview_state(code, base=WEAPON_PREVIEW_VA):
    """The state address weapon_preview_026g.asm (at `base`; or another code
    with the same routine, weapon_stash_026g.asm) was assembled for: its
    `state` routine is `call $+5` / `pop eax` / `lea eax,[eax+disp32]` /
    `ret`; 0 when it is not there exactly once."""
    head = bytes.fromhex("e80000000058" "8d80")
    at = code.find(head)
    if at < 0 or code.find(head, at + 1) >= 0 or code[at + 12:at + 13] != b"\xC3":
        return 0
    return base + at + 5 + struct.unpack_from("<i", code, at + 8)[0]


def late_join_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "late_join_026g.section.bin")


# vostok::trace's switch (rust/patches/src/vostok/trace.rs): "OFF\0" in
# every build, 1 in a diagnostics build.
DIAGNOSTICS_OFF = struct.pack("<I", int.from_bytes(b"OFF\0", "big"))


def with_diagnostics(data):
    """The build with the Rust patches' opt-in diagnostics on (patch 76's
    srp_ragdoll.csv): the crate's SRP_DIAGNOSTICS, DIAGNOSTICS_OFF in every
    build, made 1. A local build only: release/build-zip never passes
    --ragdoll-trace."""
    va = arena().symbols["SRP_DIAGNOSTICS"]
    data = bytearray(data)
    at = next((raw + va - IMAGE_BASE - rva for _, rva, vsize, raw, rsize, _ in Headers(data).sections(data)
               if IMAGE_BASE + rva <= va < IMAGE_BASE + rva + min(vsize, rsize)), None)
    if at is None:
        raise SystemExit(f"SRP_DIAGNOSTICS at {va:#x} is in no section of the build")
    if bytes(data[at:at + 4]) != DIAGNOSTICS_OFF:
        raise SystemExit(f"SRP_DIAGNOSTICS at {va:#x} is not the build's off value")
    data[at:at + 4] = struct.pack("<I", 1)
    headers = Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, pe_checksum(data, headers.checksum_at))
    return bytes(data)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--") or a == "--check"]
    options = [a for a in sys.argv[1:] if a.startswith("--") and a != "--check"]
    mode, bin_path, server, list_installs = "section", None, None, False
    late_join, late_join_path, revision, required = True, None, None, None
    diagnostics = False
    for option in options:
        if option == "--installs":
            list_installs = True
        elif option == "--cert-only":
            mode = "cert-only"
        elif option == "--no-late-join":
            late_join = False
        elif option.startswith("--late-join-bin="):
            late_join_path = option[16:]
        elif option.startswith("--cave=") and option[7:] in CAVES:
            mode = option[7:]
        elif option.startswith("--bin="):
            bin_path = option[6:]
        elif option.startswith("--server="):
            server = parse_server(option[9:])
        elif option.startswith("--revision="):
            revision = parse_revision(option[11:])
        elif option.startswith("--required-patches="):
            required = parse_required_patches(option[len("--required-patches="):])
        elif option == "--ragdoll-trace":
            diagnostics = True
        else:
            sys.exit(f"unknown option {option}\n\n{__doc__}")
    if len(args) == 3:                                    # the draft's form: <exe> <out> <bin>
        bin_path = args.pop()
    if len(args) != 2:
        sys.exit(__doc__)
    source, target = args
    if target != "--check" and os.path.exists(target) and os.path.samefile(source, target):
        sys.exit("the output must not be the original exe")
    code = None
    if mode != "cert-only":
        code = open(bin_path or default_bin(mode), "rb").read()
    late_join_code = None
    if mode == "section" and late_join:
        late_join_code = open(late_join_path or late_join_bin(), "rb").read()
    elif late_join_path:
        sys.exit("--late-join-bin goes with the default build only")
    if (revision is not None or required is not None) and late_join_code is None:
        sys.exit("--revision and --required-patches go with the default build only "
                 "(not --no-late-join, --cave=hash-mismatch or --cert-only)")
    data, description = build(open(source, "rb").read(), mode, code, server, late_join_code, revision=revision,
                              required_patches=required)
    if diagnostics:
        if late_join_code is None:
            sys.exit("--ragdoll-trace goes with the default build only (patch 76 is in the late join's arena)")
        data = with_diagnostics(data)
        description += ", the ragdoll's diagnostics on (srp_ragdoll.csv; a local build, never a release)"
    if target != "--check":
        with open(target, "wb") as out:
            out.write(data)
    print(f"{target}: {description}")
    if list_installs:
        print(instances_text())


if __name__ == "__main__":
    main()
