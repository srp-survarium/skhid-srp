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
        nasm -f bin -DCAVE=0xA427E50 -DSTATE=0xA428E00 weapon_preview_026g.asm -o weapon_preview_026g.section.bin
    (checked in; STATE is 0xE00 of the late join's state page, checked), and its
    draw from weapon_draw_026g.asm:
        nasm -f bin -DCAVE=0xA425D20 -DSTATE=0xA428E00 weapon_draw_026g.asm -o weapon_draw_026g.section.bin
    (checked in). The warmup camera turned by a drag and moved by the wheel while
    the kits are shown before the start comes from warmup_look_026g.asm and
    warmup_limits_026g.asm:
        nasm -f bin -DCAVE=0xA425D40 -DSTATE=0xA428E20 -DLIMITS=0xA425A98 warmup_look_026g.asm -o warmup_look_026g.section.bin
        nasm -f bin -DCAVE=0xA425A98 warmup_limits_026g.asm -o warmup_limits_026g.section.bin
    (checked in; STATE is 0xE20 of the late join's state page, checked)
    (checked in). The commit_suicide key comes from suicide_key_026g.asm:
        nasm -f bin -DCAVE=0xA425980 suicide_key_026g.asm -o suicide_key_026g.section.bin
    (checked in). A spot's hit on a battery's body comes from spot_026g.asm:
        nasm -f bin -DCAVE=0xA4257C0 spot_026g.asm -o spot_026g.section.bin
    (checked in). The late-join code is late_join_026g.section.bin, checked in too; it is
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
  is made unless -record_journal=1 asks for one;
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
  action call (0x242196) calls suicide_key_026g.asm at 0x980 of `.srp`
  first, which sets bit 24 on the key going down;
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

import hashlib
import os
import re
import struct
import sys

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
REVISION = 25
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
WEAPON_PREVIEW_STATE_SIZE = 12
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
# no kill. The builder's last call (spot, 0x242196, ebx = the handler) calls
# suicide_key_026g.asm at SUICIDE_KEY_OFFSET of `.srp` (after the pickup
# code, before the seek code): it asks 0x242460 for 1Fh, sets bit 24 when
# the key went down (state 0, as spot does), and jumps to 0x242460 with the
# original arguments and return address. No base relocation under the site.
SUICIDE_KEY_OFFSET = 0x980
SUICIDE_KEY_VA = SECTION_VA + SUICIDE_KEY_OFFSET
ACTION_STATE_VA = 0x242460                    # player_input_handler: the state of a buffered action
SUICIDE_KEY_HOOK = (0x242196, bytes.fromhex("e8c5020000"), 0, 0xE8)

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


def patch_value(data, va, expected, new):
    """As `patch`, for a value in .data."""
    at = data_file_offset(va)
    found = bytes(data[at:at + len(expected)])
    if found != expected or len(new) != len(expected):
        sys.exit(f"{va:#x}: expected {expected.hex()}, found {found.hex()}")
    data[at:at + len(new)] = new


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
    found = bytes(data[at:at + len(expected)])
    if found != expected or len(new) != len(expected):
        sys.exit(f"{va:#x}: expected {expected.hex()}, found {found.hex()}")
    data[at:at + len(new)] = new


def patch(data, va, expected, new):
    at = file_offset(va)
    found = bytes(data[at:at + len(expected)])
    if found != expected:
        sys.exit(f"{va:#x}: expected {expected.hex()}, found {found.hex()}")
    data[at:at + len(new)] = new


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


def build(original, mode, code=None, server=None, late_join=None, revision=None):
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
    number."""
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
    if mode == "hash-mismatch":
        if len(cave) > HASH_MISMATCH_SIZE:
            sys.exit(f"the cave code is {len(cave)} bytes; {HASH_MISMATCH_SIZE} fit")
        at = file_offset(HASH_MISMATCH_VA)
        if hashlib.sha256(data[at:at + HASH_MISMATCH_REGION]).hexdigest() != HASH_MISMATCH_SHA256:
            sys.exit(f"{HASH_MISMATCH_VA:#x}: on_hash_mismatch is not the original")
        data[at:at + HASH_MISMATCH_REGION] = cave + b"\xCC" * (HASH_MISMATCH_REGION - len(cave))
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
        for (va, expected, offset, opcode), base in ((WARMUP_LOOK_HOOK, WARMUP_LOOK_VA),
                                                     (WARMUP_LIMITS_HOOK, WARMUP_LIMITS_VA),
                                                     (WARMUP_KEYS_HOOK, WARMUP_KEYS_VA)):
            patch(data, va, expected, hook_bytes(base, va, expected, offset, opcode))
            if neutralize_relocs(data, [(va, va + len(expected))]):
                sys.exit(f"{va:#x}: a base relocation under a hook of the warmup camera")
        read_order = open(read_order_bin(), "rb").read()
        at = file_offset(READ_ORDER_VA)
        if len(read_order) != READ_ORDER_SIZE:
            sys.exit(f"read_order_026g.bin is {len(read_order)} bytes, not {READ_ORDER_SIZE:#x}")
        if hashlib.sha256(data[at:at + READ_ORDER_SIZE]).hexdigest() != READ_ORDER_SHA256:
            sys.exit(f"{READ_ORDER_VA:#x}: hdd_manager_sorting_predicate is not the original")
        data[at:at + READ_ORDER_SIZE] = read_order
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
        for va, expected, offset, opcode in WEAPON_PREVIEW_HOOKS:
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
        suicide_key = open(suicide_key_bin(), "rb").read()
        if PICKUP_OFFSET + len(pickup) > SUICIDE_KEY_OFFSET \
                or SUICIDE_KEY_OFFSET + len(suicide_key) > WARMUP_KEYS_OFFSET \
                or WARMUP_KEYS_OFFSET + len(warmup_keys) > SEEK_CLEAR_OFFSET:
            sys.exit("the pickup code, the suicide key, the warmup keys and the seek code do not fit")
        # It begins `push dword [esp+0Ch]` and ends `jmp 0x242460`.
        if suicide_key[:4] != bytes.fromhex("ff74240c") or suicide_key[-5:] != \
                b"\xE9" + struct.pack("<i", ACTION_STATE_VA - (SUICIDE_KEY_VA + len(suicide_key))):
            sys.exit(f"suicide_key_026g.section.bin was assembled for another address: -DCAVE={SUICIDE_KEY_VA:#x}")
        va, expected, offset, opcode = SUICIDE_KEY_HOOK
        patch(data, va, expected, hook_bytes(SUICIDE_KEY_VA, va, expected, offset, opcode))
        if neutralize_relocs(data, [(va, va + len(expected))]):
            sys.exit("a base relocation under the suicide key's hook")
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
            cave = cave + b"\xCC" * (SUICIDE_KEY_OFFSET - len(cave)) + suicide_key
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
    headers = Headers(data)
    struct.pack_into("<I", data, headers.checksum_at, pe_checksum(data, headers.checksum_at))
    return data, (f"certificate check, mutex, minimum VRAM check, hash interval {HASH_INTERVAL_MS} ms, "
                  f"patch revision {revision}, battery rule, mount without a root, no journal by default, "
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
                     "the scoreboard shows the places in the match and none whose player left, "
                     "a spot has no cooldown and a battery's body spots it, "
                     "a model's LOD and screen factor at a wider field of view are the original's"
                     if late_join is not None else "")
                  + (f", default login server {server}" if server is not None else ""))


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
def suicide_key_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "suicide_key_026g.section.bin")


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


def weapon_preview_state(code):
    """The state address weapon_preview_026g.asm was assembled for: its
    `state` routine is `call $+5` / `pop eax` / `lea eax,[eax+disp32]` /
    `ret`; 0 when it is not there exactly once."""
    head = bytes.fromhex("e80000000058" "8d80")
    at = code.find(head)
    if at < 0 or code.find(head, at + 1) >= 0 or code[at + 12:at + 13] != b"\xC3":
        return 0
    return WEAPON_PREVIEW_VA + at + 5 + struct.unpack_from("<i", code, at + 8)[0]


def late_join_bin():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "late_join_026g.section.bin")


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--") or a == "--check"]
    options = [a for a in sys.argv[1:] if a.startswith("--") and a != "--check"]
    mode, bin_path, server = "section", None, None
    late_join, late_join_path, revision = True, None, None
    for option in options:
        if option == "--cert-only":
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
    if revision is not None and late_join_code is None:
        sys.exit("--revision goes with the default build only (not --no-late-join, --cave=hash-mismatch or --cert-only)")
    data, description = build(open(source, "rb").read(), mode, code, server, late_join_code, revision=revision)
    if target != "--check":
        with open(target, "wb") as out:
            out.write(data)
    print(f"{target}: {description}")


if __name__ == "__main__":
    main()
