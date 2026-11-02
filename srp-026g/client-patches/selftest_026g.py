"""Self-test of patch_026g.py. Builds every variant from the ORIGINAL 0.26g
exe (read only), writes the results to srp-026g/target-a4/scratch/ (never the
game folder) and checks them. Nothing is run and nothing is installed.

    python selftest_026g.py [<survarium.exe>]

Checks, per variant (cert-only, section, hash-mismatch; the last two here
without the late join, as patch revision 4 was, which check 15 builds on):
 1. the checked-in .bin files are what nasm makes from the .asm today;
 2. the PE parses: section table, alignments, SizeOfImage, data
    directories inside the file, the certificate table last and intact;
 3. every byte outside the intended sites equals the original;
 4. no active base relocation overlaps a rewritten byte, and the relocation
    table is otherwise untouched;
 5. the hooks and the cave disassemble (ndisasm) to the intended
    instructions: every call/jmp target is a known address;
 6. nothing in the original branches into the middle of a hook;
 7. Windows computes the same PE checksum (imagehlp);
 8. Windows still reads the survarium.com certificate out of the file with
    the exact CryptQueryObject call the client makes at start (0x13310);
 9. full builds only: the default hash interval at 0x27CC4F is 50 ms, one
    byte changed, under no relocation; cert-only keeps the original 100.
10. full builds only: the minimum VRAM check jumps over the fatal warning;
    the detected size and unknown-size fallback remain unchanged;
11. the version field of the first login message, worked out from the
    instructions that fill it (0x380847..0x38088F): "0.26g0" and six zero
    bytes in the original and in cert-only; in the full builds byte 7 is
    patch_026g.REVISION and nothing else differs. The relocation under the
    rewritten instruction is disabled, the two beside it are not, and no
    branch of the function lands inside it.
12. optional embedded login server: the missing-server hook, relative stub,
    address bytes and unchanged explicit-server/replay paths in both caves.
13. full builds only: the battery rule's two instructions (0x313A62,
    0x313B34) are nops between the instructions they stood between, under
    no relocation, and no branch lands inside them; cert-only keeps them.
14. full builds only: mounter::finish leaves at once for a mount without a
    root (0x46B0B, `jmp short 0x46b36` and two nops; the path of a mount
    with a root is the original's), and the default of -record_journal is
    the exe's "0" (0x851CC7; the relocation of the address stays active,
    the option is read only at the game's start); cert-only keeps both.
15. the late join (patch revision 5, the default build): the checked-in
    late_join_026g.section.bin is its .json's, and its first 4096 bytes
    are what nasm makes of late_join_026g.asm with the .json's addresses
    (the C code after them is not built here: late_join_026g.py
    --check-bin does that where a C compiler is). The build with it
    differs from the build without it, of the same server, in the sites
    patch_026g.py names and in nothing else of the original's bytes; each
    rewritten site disassembles to what it is meant to be, each hook to a
    call or jump to its entry and nops; no relocation is under a site;
    `.srp` has the first page unchanged, then the code; `.srpd` is one
    zeroed writable page after it with no bytes in the file; the checks
    of 2 and 7 to 14 hold for it too, and the revision is REVISION.
16. the same build: the default of the game's setting
    time_floating_factor (0xA799C8, in .data) is CLOCK_FACTOR, four bytes
    under no relocation, inside the range the game itself allows; in the
    original the value is read by the `game` constructor alone, into the
    field of the game clock's timer that set_offset multiplies the
    correction's sign by, and the console variable registered for it has
    that name. The build without the late join keeps the original's 0.05.
17. the same build: the statistics handler's start_match (0x26D8D0)
    keeps 20 as its count, `mov al,0x14` and four nops where the original
    and the build without the late join copy the roster; in the original
    that count goes into +3240h alone, start_match tells the hud the score
    of each counted place, the roster still goes into the shared
    statistics' live count (+31E8h) apart, deserialize runs
    recalculate_scores over the counted places, the vtable's slots are
    those functions, +3240h is in .text only in those loops and in stores
    of a zeroed register, and no branch or address aims inside the six
    bytes.
18. the same build: game_world_ui::update_ui's jump of the countdown
    (0x23B328) goes to 0x23B383 instead of the players' rows; in the
    original the countdown shows the pregame and jumps to the rows, play
    shows the match time and goes on at 0x23B383, an instruction of its
    own, from which the code draws the team points, the last players and
    the respawn time, lets go of the player it took and leaves to nowhere
    else; every call before both takes its arguments off the stack; the
    three are the hud movie's set_game_progress, set_last_players and
    set_respawn_time; no branch or address aims inside the jump.
19. the same build: the respawn rule's spawn calls weapon_random_hook at
    0x316EA0 (`call` where the original and the build without the late
    join load the call that puts the player into the world); in the
    original the spawn makes the target kit current, takes its player into
    esi and initialises it with the time in ebp, neither register written
    again before the hook and no flag read after it; the spawn is slot +5Ch
    of every respawn rule and nothing calls it otherwise, the rules' tick
    spawns through it; the player's initialize sets its inventory (23
    slots at +10Ch) up and makes no object; weapon_core's constructor puts
    1 into +5BCh, which .text names only there, in the two spreads (48271
    mod 2^31-1), the serialize and the deserialize; weapon_core's bind is
    in the file only in the item vtables of weapon_core and of the
    client's weapon; the hook's code tests that bind, writes +5BCh alone,
    and ends with the two instructions; no branch or address aims inside
    the five bytes.
20. the same build: the two calls of a game's first world (0x2D1050,
    0x2D1055) go to first_load_input and first_load_kit; in the original
    they are the only calls of 0x2D4AB0 and 0x2D4B50, in deserialize's
    block of the first world after the world is read (which puts the
    world's next kit into place+0Dh), the kit choice is the kit in use and
    both append at the newest end; each tick takes the next kit from the
    newest entry at or before it, and the network inserts in time order;
    the new code reads place+0Dh, calls the game's allocator and the
    list's insert only, writes a node's value and time only and leaves to
    nowhere else; nothing aims inside the two calls.
22. the same build: Team Deathmatch's death handler (0x316090) goes on at
    0x3160F1 where the original and the build without the late join jump
    to its return when the rule's completion time (+12Ch) is not -1: six
    nops between `cmp dword [esi+12Ch],-1` and `mov ecx,[esi+128h]`; the
    handler's only jumps to its return are that one and the one of the
    other event; no relocation under, no branch into; the rule's code
    touches +12Ch at seven places (the constructor, the guard, the end
    test's four, rule_completed). run_tdm_rule.ps1 runs the handler.
23. the same build: the field-of-view setting (patch 22). The video
    options' option 7 names "uro_fov" (0x262930) where the original names
    "fov", a string nothing else points to; "uro_fov" is the name of the
    console variable at 0xAA6510, whose value (0xA6FA28, 65.0) only its
    registration pointed to; the camera's projection (0x242900, slot 0 of
    the vtables of player_input_handler, game_camera, free_fly_camera,
    demo_camera and lobby_camera, which RTTI names, and pointed to from
    nowhere else) jumps at its first instruction to fov_projection, the
    rest of its body (the only reader of the 65-degree constant 0xA2D9EC)
    the original's; fov_projection reads uro_fov, takes the scope's blend
    with its four constants, calls the same matrix builder, writes no XMM
    register but xmm0 and returns as the original (`ret 8`); the
    registration stores 50.0 as the least, the most stays the float 100 at
    0xA710D0; the build without the late join keeps all three; no branch or
    address aims inside the two sites rewritten in place.
24. the same build: a battery under the floor (patch 23). The scan's
    frame and its three hooks; the code (.srp+800h) leaves only back to the
    scan and calls only a battery's get_transform (vtable+44h; only
    batteries' vtables have victory_item_core::can_use at +30h); nothing
    aims inside the four sites.
25. the same build: verify_sign's call of verify_sign_impl (0x135D3) is
    `xor eax,eax` and three nops (`mov eax,esi` before it and
    `add esp,10h` after it are the original's; no relocation under it, no
    branch aims inside it): no resources.db signature is checked; the build
    without the late join keeps the original call.
26. the same build: sign_in_on_connected's `mov [edi+8],eax` /
    `mov [edi+0Ch],dl` (0x380889) are `call` stale_dll at 0xE00 of `.srp`
    and a nop (no relocation under them, no branch aims inside them);
    stale_dll_026g.section.bin is what nasm makes and sits there, after the
    seek code and before the late join; it saves every
    register (pushad / popad), calls itself once for its address, then
    GetModuleHandleA, GetProcAddress, CreateFileA, CloseHandle through the
    import table and GetTempPathA, branches only inside itself, does the two
    writes it replaced and ends with `ret`; it names kernel32.dll,
    GetTempPathA and shell_32.dll and nothing else. Check 11 works out its
    field: byte 8 the check's. The build without the late join keeps the
    original instructions.
27. the same build: the mount of resources.db's patches (test_mount_patches).
28. the same build: the transparent effects scroll as in 0.29 (patch 28,
    forward_scroll_026g.asm at FORWARD_SCROLL_OFFSET of `.srp`). The three
    sites (0x1D90DC and 0x1D94CB in effect_fstage_default_materials::
    compile, 0x14990F in renderer_context's constructor) are a jump to their
    entry and nops where the original and the build without the late join
    have the two instructions each replaces; in the original, both ends of
    the angle attenuation's read meet at the first and compile_begin is
    given the configuration at [esp+50h], both ends of the uv distortion's
    block meet at the second, and the third follows scene_time's binding,
    the last of the constructor's 19 register_constant_binding calls (20 in
    the exe); compile writes the configuration's byte 0Ch only where it
    zeroes it (the stack followed instruction by instruction), use_sequence
    is byte 4 bit 4; g_stage's compile sets bit 6 of byte 0Ch from
    use_uv_scrolling and sends uv_scrolling_parameters0/1 with 0x1A8900;
    fill_shader_configuration_macros makes CONFIG_USE_UV_SCROLLING of that
    bit; no relocation under a site and nothing aims inside one. The code is
    forward_scroll_026g.section.bin (what nasm makes, check 1) and sits
    between the seek code and the shell_32.dll check (check 15); its only
    ways out are the config's two helpers, set_constant<float4>, the shared
    string and its destroy, register_constant_binding and the three jumps
    back, each after the two instructions its hook replaced; it reads the
    exe's six texts and the resource manager's pointer and its own data
    (1.0 and three names) plus the delta in ebp; three pushad, three popad.
    check_forward_scroll_026g.py runs it.
29. the same build: "Watch replay" (patch 29, revision 14). The lobby's
    video_tutorial_link_clicked (0x28CDA0, entry 106 of the callback
    names, its address in the file once, called by nothing) jumps at
    0x28CDB6 to replay_link where the original pushes ShellExecuteA's
    arguments; the original's body (the delay-loaded ShellExecuteA of the
    link buffer, enable_element) and the link buffer's every use (emptied,
    filled by 9E as UTF-8 with its zero, tested, opened) are read;
    replay_link reaches GetModuleFileNameW, GetModuleHandleA,
    GetProcAddress and CloseHandle through the game's own IAT slots,
    leaves only by its return, and holds the link's pattern and the fixed
    program name; nothing aims inside the hook and no relocation is under
    it; its code sits at 0x220 of `.srp`, after the upload code and the
    default server and before the patches' mount (check 15's layout).
    check_replay_link_026g.py runs the handler on made-up links.
30. the same build with --revision=<n> (a release's, REVISION + 1 and 255):
    against the build without it, only the revision byte and the PE
    checksum differ, and the version field says <n> with the shell_32.dll
    flags as before (no patch is chosen by the number); REVISION - 1, 256
    and a build without the late join are refused.
31. the same build: one upgrade on several items (patch 30). The upgrade
    window's drop (0x29AAD5, in drop_upgrade_item's handler 0x29A9D0, the
    delegate of the callback of that name), the `83 46 02` and `83 46 01`
    cases (0x275756, 0x275589) and process_detach_upgrade's search
    (0x27534D) call or jump into shared_upgrades_026g.asm at 0x700 of
    `.srp`, after the patches' mount and before the pickup code (check
    15's layout); each site is the original's in the original and the
    build without the late join, nothing aims inside one and no
    relocation is under one; the functions they replace calls of are
    called from those sites alone; the handler reads the item and the
    upgrade from the Flash arguments where the code reads them; the code
    calls only the packet functions and process_detach_upgrade and holds
    no address. check_shared_upgrades_026g.py runs the original's and the
    build's code on inventories with an upgrade on several items.
32. the same build: a kit chosen before the match starts is played at
    once (patch 31, kit_change_026g.asm at KIT_CHANGE_OFFSET of `.srp`).
    The respawn rule's tick (0x316820, slot +50h of the respawn rules'
    vtables) is a jump to kit_change and a nop where the original and the
    build without the late join have `push ebp` / `mov ebp,esp` /
    `sub esp,34h`; the client's view of a spawn into another kit (0x2678E0,
    the client rule's +60h) calls kit_view where it pushed 1 and called the
    player camera; the hud's update of the local player's state has
    `jmp ui_alive` and two nops where it called 0x23A1A0 and pushed 0 for
    the call that hides the kits (0x2383B0, which alone calls
    root.show_profiles_selector, and which only the profile_changed callback
    and this update call), and `call ui_start` and a nop where it begins to
    let go of the player; the profile_changed callback has nine nops where
    it pushed 0 and called 0x2383B0. In the original: the tick, the
    spawn and its point,
    the view's registers; the time rule's states at core+18FE4h; the order
    of a tick (tick list 0 before the time rule's list 1; in the build the
    respawn rule's key, from the core's counter, before the places', the late
    join's (place+1) << 16) and the camera's state (the client's
    time rule sets the player camera in play, the warmup's otherwise, when
    its state changes in a tick first played, and keeps it at network
    client+1F1C60h), byte for byte. The code calls and leaves only to what
    it names, stores nothing but on its stack; no relocation under a site
    and nothing aims inside one. Its camera before the start
    (warmup_look_026g.asm at WARMUP_LOOK_OFFSET, warmup_limits_026g.asm
    at WARMUP_LIMITS_OFFSET and warmup_keys_026g.asm at WARMUP_KEYS_OFFSET,
    check 15's layout): the game world's input
    handler's on_mouse_move (0x25D0E0, its vtable 0x9FC588's +0Ch,
    priority 7) jumps to warmup_look with five nops where it began, and
    process_warmup_mode's store of the pitch (0x24225E) calls
    warmup_limits with three nops; the player's input handler (vtable
    0xA04048) has priority 10; the input world gives a move to its
    handlers by priority, lowest first, until one answers 1, its mouse and
    the buttons held where the code reads them; the lobby camera's turn
    (buttons 52h), wheel step and limits (1.5, 4.6, +-60 degrees) as the
    code has them; the warmup camera (0x1F8190) sets mode 3, the warmup
    mode stores the distance and the pitch with no limit; the hud's
    cursor and kits flags; the code holds no address and calls only the
    game world's vtable+30h, +3Ch and the end of a drag; its state is in
    the late join's page. check_kit_change_026g.py runs it.
33. the same build: a weapon's upgrade window shows the weapon (patch 32,
    weapon_preview_026g.asm at WEAPON_PREVIEW_OFFSET of `.srp`, after the
    late join's code, its state at WEAPON_PREVIEW_STATE_OFFSET of `.srpd`).
    The camera calls of start_item_upgrade's 0x265DE0 (0x265E73) and
    exit_item_upgrade's 0x265F10 (0x265F44) call start_cam and exit_cam;
    setup_profile's first six bytes (0x284B10) jump to setup_hook; the
    three loads of the camera's least distance in its tick (0x284081,
    0x28428C, 0x284316) call min_step or min_final, with three nops. In the
    original and the build without the late join each site is what it was;
    the movie's callbacks start_item_upgrade and exit_item_upgrade are the
    delegates of 0x29A870 and 0x29A890, which call 0x265DE0 and 0x265F10
    with the lobby ui's character (+694h); the camera's setter 0x2845C0 is
    called from those two sites alone, setup_profile from the lobby's
    refresh, the squad panels' and its own re-call alone; the float at
    0xA7AFA4 is 1.5 and the tick reads it at those three sites alone; the
    upgrade mode (+D515h) is set by 0x265DE0 alone, cleared by 0x265F10 and
    the character's constructor. Nothing aims inside a site; the base
    relocations under the three loads are disabled, none other. The code
    calls and leaves only to setup_profile, its body after the six bytes
    and the camera's setter, holds no absolute address and keeps its state
    where it was assembled for. The draw: the character's update's write of
    the player's decal pose (0x284A33, the only write but the player's
    ctor; its readers are player::mt_draw and player::insert alone; +D515h
    is read there alone) calls weapon_draw_026g.asm's player_mode, at
    WEAPON_DRAW_OFFSET of `.srp` (check 15's layout), which reads the same
    state. check_weapon_preview_026g.py runs it.
34. the same build: the order of the archive reads is a strict weak
    ordering (patch 33). hdd_manager_sorting_predicate's 0x1A0 bytes at
    0x10BCC0 are the original's in the original and the build without the
    late join, read_order_026g.bin here; no base relocation is in them,
    nothing outside aims inside them, its fifteen calls are all in the
    std::sort helpers of grab_sorted_queries (0x10BF70..0x10C3ED), which it
    alone calls; the new code calls only get_physical_path, strcmp_s and
    get_file_offs, leaves only by `ret 4`, every branch inside lands on an
    instruction, it holds no absolute address and int3 fills the rest.
    check_read_order_026g.py runs it.
35. the same build: the commit_suicide key kills one's own player (patch
    34, the Rust patch suicide_key, the arena's block patches.code). The
    original: the action table's entry 0xA74D20 is commit_suicide, action
    1Fh, default key kK; the input builder 0x241250 asks 0x242460 for 35
    actions, 1Fh not among them, and sets no 1000000h in the action mask;
    base_player::process_quick_slots reads bit 24 at 0x2D8BC1 and is called
    from the input apply 0x2D86E0 alone (0x2D8777). The spot call at
    0x242196 is `call 0x242460` in the original and the build without the
    late join, `call` the generated entry here; the entry (RUST_CALL) aligns
    the stack, passes action_present's three arguments to the shim and
    `ret 0xc` as action_present (stdcall) pops; no relocation under the site
    and nothing aims inside it. check_suicide_key_026g.py runs the site to
    the reconvergence point against the original exe; port/equiv_026g.py is
    the port-only gate (docs/notes/rust_patches.md).
36. the same build: a baked texture no longer in the baker's list is made
    again (patch 35). check_loaded_baked_texture (0x131B40) has `jmp short
    0x131c73` at 0x131C1E where the original and the build without the late
    join have `jnz 0x131c73` after `cmp byte [226091Dh],0` (the not-found
    assertion's "ignore" flag); in the original the assertion "Saved baked
    texture %s not found" and `int3` follow, 0x131C73 returns false, and
    only the list walk reaches that end; no relocation under the two bytes,
    no branch into them. check_baked_texture_026g.py runs the original's
    and the build's list on the loads of the crash.
37. the same build: the memory arena "ai" is 4 MiB (patch 36). In the
    static initializer 0x1E8B50 the original and the build without the late
    join have `mov dword [esp+0x18],0x40000` at 0x1E8CCB, this build
    `mov dword [esp+0x18],0x400000`, between `mov esi,[0xa808fc]` (the
    arenas' list) and `mov dword [esp+0x24],0x9fa4e0` ("ai") / `call
    0xe21c0`, after the arena 0xA967D8's vtable store; only byte 0x1E8CD1
    differs (04h -> 40h); "ai" is named there alone; no relocation under
    the instruction, no branch of the initializer into it.
38. the same build: spotting (patch 38). In base_player::apply_new_input
    (0x2D86E0) the original's spot block: bit 28 of the input (set by the
    input builder on the spot key going down, 0x2421A6), the wait +34Ch
    at 0, a 100 m ray (0xA710D0) with filter 80Ah, the wait set to 3000
    at 0x2D87C1 before the ray is cast, the hits walked with the owner's
    slot 2 (a player) and, at 0x2D8A0F, slot 1 (a geometry); the wait
    counted down by the player's tick (0x2DAB44) and kept by
    base_player::serialize (0x2DB34D) and its read (0x2DB95C).
    victory_item_core's vtables: +54h 0xA13EE0 (set_spotted 0x301C40,
    can_be_spotted_by 0x301CA0, `mov al,1`), +58h 0xA13EEC, the rigid
    body's owner (0x301CC0, group 0A18h at 0x301CF5), whose slots 1 and 2
    are 0x300040 `xor eax,eax; ret`, stored by the two constructors alone.
    The build: `mov dword [edi+0x34c],0x0` at 0x2D87C1 (two bytes
    differ), and 0x2D8A0F a jump to spot_026g.section.bin at SPOT_VA,
    which leaves only to 0x2D888F, 0x2D8A8F and 0x2D8A14, calls only
    through the battery's vtable, holds one absolute value (0xA13EEC) and
    whose branches inside land on instructions; the build without the
    late join keeps both; no relocation under the sites and nothing aims
    inside them. check_spot_026g.py runs it.
40. the same build: a model's LOD and screen factor at a wider field of
    view are the original's (patch 40). select_lod's `mulss xmm0,[0x927a44]`
    (0.5, 0x16F2A4) and calculate_screen_factor's `fmul dword [0xa710d0]`
    (100, 0x1590AF) are each a call of the original's length into the late
    join, to fov_lod_scale and fov_screen_scale: the original instruction,
    then, when the state page's k (FOV_LOD_STATE_OFFSET) is not 0, the same
    by k, and ret. The original's two are where the original reads them
    (select_lod's screen-size branch before its params, the factor before
    its store at +58h); their relocations are disabled; fov_projection
    alone writes k (fptan twice, then the store); k is after the other
    states of the page; no branch lands inside the two sites.
    check_fov_lod_026g.py runs the projection, select_lod and the screen
    factor of the original and the build.
46. the same build: on the death screen a drag turns the camera, the wheel
    does nothing (patch 46, death_camera_026g.asm in `.srpc`, the section
    after `.srpd`). update_camera's `call 0x2422b0` (0x1F364B,
    ebx the player) calls death_view; the game world's on_mouse_move jump
    (check 32) goes to death_look. In the original: the view's update copies
    the transform in modes 0 and 4 and its third-person step (0x2422DE)
    reads the yaw, pitch and distance and writes the view and +35Ch alone;
    a death sets no mode; a dead player's input is not sent (0x1F7883); the
    handler's game world, game, client and local player; the window's size
    (vtable+1Ch) and the look's step. The code: its exits (warmup_look, the
    update, its step on the identity, then the turn times the eyes' rows),
    calls (the pointer, the window's size, drag_end), the turn's limits,
    absolute operands (its state, the options, its two constants) and
    returns; branches inside land on instructions; no relocation under the
    site, nothing aims inside it. check_death_camera_026g.py runs it.
44. the same build: a weapon wears the camouflage its item holds (patch
    44, weapon_camo_026g.asm at the start of `.srpw`, the last section,
    on the page after `.srpc`): its hook, its entry and name format, its
    calls of patch 47's lookup and fill. check_weapon_camo_026g.py runs it.
45. the same build: a weapon's upgrade window shows it above the lobby's
    workbench (patch 45, weapon_table_026g.asm at `.srpw`+6B0h; patch 32
    grown: weapon_stash_026g.asm at +360h): the sites, the entries, the
    states. check_weapon_table_026g.py and check_weapon_stash_026g.py run them.
47. the same build: decals on a weapon (patch 47, weapon_decals_026g.asm on
    `.srpw`'s second page): the sites, the entries, the states it reads,
    its 16-byte-aligned constants. check_weapon_decals_026g.py runs it.
48. the same build: an upgrade dropped on a weapon in its window is shown at
    once (patch 48, upgrade_preview_026g.asm on `.srpw`'s third page, its
    state at .srpd+D60h): the sites, the entries, the find patch 32 calls.
    check_upgrade_preview_026g.py runs it.
49. the same build with --required-patches= (a release's offered patches):
    the patches' mount stops the game when a required
    resources.db.patch_<n> is not there (patch 49). The default build's
    bitmap (the 32 bytes after the mount's "Cannot open" text) is zeros and
    requires none; against the build without the option only the bitmap's
    bytes and the PE checksum differ, bit n of byte n/8 set for each n
    (1, 12, 14, 17, 255; revision 27's list), and patch_026g's
    required_patches_of and release/lib.sh's exe_required_patches read the
    list back; the probe's missing case tests the bit with `bt` on that
    bitmap (plus the delta) and goes on to the same fatal call as an
    unreadable file. 0, 256, a number twice, a build without the late join
    and the option's text out of order are refused.
    check_mount_patches_026g.py runs the code on folders missing a
    required patch.
52. every full build: Scaleform's Stage focus getter (0x771FA0) gives null
    for a focused object without its ActionScript object (patch 52,
    STAGE_FOCUS), where the original read its traits through null: the 46
    bytes at 0x771FED and the getter around them, its member pointer and
    ThunkInfo "focus", no relocation under them, nothing aims inside, the
    lobby's key handler invoking root.has_active_window; cert-only keeps
    the original. check_stage_focus_026g.py runs it.
53. the same build: Research, mode 5 (patch 53, the arena's blocks
    research.code and research.state in `.srpx`, the twelfth and last
    section, the later patches' code, on the page after `.srpw`, linked from
    their sources by link_026g.py): the arena is its lock's, the three sites (the game's constructor, update_ui's two) are the
    original's instructions before and the hooks after, no relocation
    under one and no branch into one, the entries are jumps, the stubs
    leave only where they should, the C code calls the game's functions
    through the engine-function library (each slot of the rule's and the cook's vtables that is ours
    pops what tdm_rule_core's (0xA15DA4) and tdm_rule_cook's (0x9FD8AC)
    pops: the call shapes' INSTALLED_VTABLES check, for every patch).
    check_research_026g.py runs the hooks, the cook and the rule.
54. the same build: the artefacts (patch 54, artefacts_026g.asm, the
    arena's block artefacts.code): the original's seven sites (Spring's loader and
    its keys, the active's link and unlink, Lifebone's enable_active after
    its remove_affects, artefact_base::action's entry, the player's
    stagger, Onyx's hit callback's state test) and the two sizes of a Spring, the hooks and sizes in the
    build, the code, its exits and calls, its data. check_artefacts_026g.py runs it.
91. every full build: a held weapon shown while its player aims keeps its
    body (patch 91, WEAPON_SHOW): weapon::on_show's `jz near 0x243EC7` at
    0x243E32 is `jmp 0x243EC7` / nop (the idle sight and the body always),
    the aimed branch and the body's test on +181Dh as the original's,
    weapon::draw's scope test and add_aimed_rifle_scope_to_scene as the
    original's, no write of +181Eh in on_show, draw or the scope's add and
    remove, no relocation under the six bytes, nothing aims inside them;
    cert-only keeps the original. check_weapon_show_026g.py runs it.
The engine-function library (engine_026g.py, the arena's block engine.code;
client_patch.md section 0.5): its files are what engine_026g.tsv and the
re-kit's ABI rows give, and engine_026g.py refuses a spec row whose
parameters are not where the re-kit's roles say (each names a word of its
location's role: two parameters swapped are refused) and a cleanup that is
no number. check_engine_026g.py
(run here, test_engine) proves every thunk against the rows and the original
exe: each argument in its register or slot at the real function's entry, the
exe's `ret` against abi.tsv's cleanup (a wrong cleanup fails), the stack
balanced, EBX ESI EDI EBP kept, the result where C takes it; the config's
look-ups, the field writers, the statistics' listener, the pool lookup and
the allocation's name run for real. Not caught here: a re-kit row whose
location is itself wrong (role and all) for a function not run for real;
Research's other ones (malloc, the rule base's constructor, the message,
Invoke, register_cook) run for real in check_research_026g.py (not run here).
"""

import ast
import ctypes
import hashlib
import itertools
import csv
import json
import math
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile

import engine_026g
import patch_026g as P

HERE = os.path.dirname(os.path.abspath(__file__))
# What the self-test needs of this machine, each from the environment:
# the original 0.26g survarium.exe (or pass it as the first argument), a
# folder to build into (never the game's), the folder of nasm and ndisasm
# (empty: found on PATH), and optionally an exe built with --cert-only to
# compare with.
ORIGINAL = os.environ.get("SRP_026G_EXE", "survarium.exe")
INSTALLED_CERT_ONLY = os.environ.get("SRP_026G_CERT_ONLY_EXE", "")
SCRATCH = os.environ.get("SRP_026G_SCRATCH", os.path.join(tempfile.gettempdir(), "srp-026g-selftest"))
NASM_DIR = os.environ.get("NASM_DIR", "")


def tool(name):
    return os.path.join(NASM_DIR, name) if NASM_DIR else name

# Every address the injected code may call or jump to outside itself
# (client_patch.md section 3).
EXTERNAL = {
    0x324BE0: "game_state_history_item::serialize",
    0x07AC10: "buffer_writer::w",
    0x424CA0: "_alloca_probe_16",
    0x2D1070: "is_there_event_horizon_item",
    0x2D0B30: "event_horizon_history_item",
    0x27AEA7: "on_match_ready, after the hook",
    0x27A5AB: "0xF6 case: skip the payload and return",
}
INDIRECT = {"call [eax+0x18]": "new_packet", "call [eax+0x1c]": "enqueue"}
# The functions that hold the hooks: [start, end) of straight code.
HOSTS = [(0x27A300, 0x27A738), (0x27AA20, 0x27AEC8), (0x1EA9F0, 0x1EB2C1)]
JUMP_TABLE = (0x27A738, 20)                   # on_match_packet_received, ids 0xE3..0xF6

failures = []


def check(ok, text):
    print(("  ok    " if ok else "  FAIL  ") + text)
    if not ok:
        failures.append(text)


def ndisasm(code, va):
    """[(address, hex bytes, text)]"""
    with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as chunk:
        chunk.write(code)
    try:
        out = subprocess.run([tool("ndisasm"), "-b", "32", "-o", str(va), chunk.name],
                             capture_output=True, text=True, check=True, timeout=60).stdout
    finally:
        os.unlink(chunk.name)
    rows = []
    for line in out.splitlines():
        parts = line.split(None, 2)
        if len(parts) == 3 and not parts[0].startswith("-"):
            # ndisasm 3 spells the same 32-bit indirect call with explicit
            # size/distance qualifiers; keep the 2.x spelling for checks.
            text = parts[2].replace("call dword near [", "call [")
            rows.append((int(parts[0], 16), parts[1], text))
    return rows


BRANCH = re.compile(r"^(call|jmp|j[a-z]+|loop[a-z]*)\s+(?:near |short )?(0x[0-9a-f]+)$")


def branch_target(text):
    match = BRANCH.match(text)
    return int(match.group(2), 16) if match else None


class Image:
    def __init__(self, data):
        self.data = data
        self.headers = P.Headers(data)
        self.sections = self.headers.sections(data)

    def offset(self, va):
        rva = va - P.IMAGE_BASE
        for _, srva, vsize, raw, rsize, _ in self.sections:
            if srva <= rva < srva + rsize:
                return rva - srva + raw
        raise ValueError(f"{va:#x} is in no section's file data")

    def read(self, va, size):
        at = self.offset(va)
        return bytes(self.data[at:at + size])

    def directory(self, index):
        return struct.unpack_from("<II", self.data, self.headers.optional + 96 + 8 * index)

    def relocations(self):
        """{file offset of the entry: (type, va)}"""
        rva, size = self.directory(5)
        o = self.offset(P.IMAGE_BASE + rva)
        end, out = o + size, {}
        while o < end:
            page, block = struct.unpack_from("<II", self.data, o)
            if block < 8:
                break
            for i in range((block - 8) // 2):
                e = struct.unpack_from("<H", self.data, o + 8 + 2 * i)[0]
                out[o + 8 + 2 * i] = (e >> 12, P.IMAGE_BASE + page + (e & 0xFFF))
            o += block
        return out


def test_bins():
    print("assembled code")
    for mode, cave in P.CAVES.items():
        for stem in ("snapshot_upload_026g", "default_server_026g"):
            path = os.path.join(HERE, f"{stem}.{mode}.bin")
            with tempfile.TemporaryDirectory() as tmp:
                out = os.path.join(tmp, "out.bin")
                subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={cave:#x}",
                                os.path.join(HERE, stem + ".asm"), "-o", out],
                               check=True, timeout=60)
                fresh = open(out, "rb").read()
            check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.PICKUP_VA:#x}",
                        os.path.join(HERE, "pickup_below_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.pickup_bin(), "rb").read(),
          f"{os.path.basename(P.pickup_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SEEK_CLEAR_VA:#x}",
                        os.path.join(HERE, "replay_seek_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.seek_clear_bin(), "rb").read(),
          f"{os.path.basename(P.seek_clear_bin())} is current ({len(fresh)} bytes)")
    path = P.stale_dll_bin()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.STALE_DLL_ENTRY:#x}",
                        os.path.join(HERE, "stale_dll_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    path = P.forward_scroll_bin()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.FORWARD_SCROLL_VA:#x}",
                        os.path.join(HERE, "forward_scroll_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    path = P.mount_patches_bin()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.MOUNT_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.MOUNT_PATCHES_VA:#x}", f"-DSTATE={state:#x}",
                        os.path.join(HERE, "mount_patches_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.KIT_CHANGE_VA:#x}",
                        os.path.join(HERE, "kit_change_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.kit_change_bin(), "rb").read(),
          f"{os.path.basename(P.kit_change_bin())} is current ({len(fresh)} bytes)")
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_LOOK_STATE_OFFSET
    for source, defines, path in (
            ("warmup_look_026g.asm", [f"-DCAVE={P.WARMUP_LOOK_VA:#x}", f"-DSTATE={state:#x}",
                                      f"-DLIMITS={P.WARMUP_LIMITS_VA:#x}"], P.warmup_look_bin()),
            ("warmup_limits_026g.asm", [f"-DCAVE={P.WARMUP_LIMITS_VA:#x}"], P.warmup_limits_bin()),
            ("warmup_keys_026g.asm", [f"-DCAVE={P.WARMUP_KEYS_VA:#x}",
                                      f"-DSTATE={state - P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_KEYS_STATE_OFFSET:#x}"],
             P.warmup_keys_bin())):
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "out.bin")
            subprocess.run([tool("nasm"), "-f", "bin", *defines, os.path.join(HERE, source), "-o", out],
                           check=True, timeout=60)
            fresh = open(out, "rb").read()
        check(fresh == open(path, "rb").read(), f"{os.path.basename(path)} is current ({len(fresh)} bytes)")
    late_join = open(P.late_join_bin(), "rb").read()
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.death_camera_va(late_join):#x}",
                        f"-DSTATE={P.late_join_state_va(late_join) + P.DEATH_CAMERA_STATE_OFFSET:#x}",
                        f"-DLOOK={P.WARMUP_LOOK_VA:#x}", f"-DLOOK_STATE={state:#x}",
                        f"-DDRAG_END={P.WARMUP_LIMITS_VA + P.WARMUP_DRAG_END_OFFSET:#x}",
                        os.path.join(HERE, "death_camera_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.death_camera_bin(), "rb").read(),
          f"{os.path.basename(P.death_camera_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SPOT_VA:#x}",
                        os.path.join(HERE, "spot_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.spot_bin(), "rb").read(),
          f"{os.path.basename(P.spot_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.REPLAY_LINK_VA:#x}",
                        os.path.join(HERE, "replay_link_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.replay_link_bin(), "rb").read(),
          f"{os.path.basename(P.replay_link_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.SHARED_UPGRADES_VA:#x}",
                        os.path.join(HERE, "shared_upgrades_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.shared_upgrades_bin(), "rb").read(),
          f"{os.path.basename(P.shared_upgrades_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", os.path.join(HERE, "read_order_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.read_order_bin(), "rb").read(),
          f"{os.path.basename(P.read_order_bin())} is current ({len(fresh)} bytes)")
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_PREVIEW_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        stash = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_STASH_OFFSET
        table = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_OFFSET
        upgrades = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_OFFSET
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.WEAPON_PREVIEW_VA:#x}", f"-DSTATE={state:#x}",
                        f"-DFIND={upgrades + P.UPGRADE_PREVIEW_FIND:#x}", f"-DCAMERA={table:#x}",
                        os.path.join(HERE, "weapon_preview_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_preview_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_preview_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={stash:#x}", f"-DSTATE={state:#x}",
                        f"-DPREVIEW={P.WEAPON_PREVIEW_VA:#x}", os.path.join(HERE, "weapon_stash_026g.asm"),
                        "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_stash_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_stash_bin())} is current ({len(fresh)} bytes)")
    table_state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={table:#x}", f"-DSTATE={table_state:#x}",
                        f"-DPREVIEW={state:#x}", os.path.join(HERE, "weapon_table_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_table_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_table_bin())} is current ({len(fresh)} bytes)")
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    decals = camo_va + P.WEAPON_DECALS_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={camo_va:#x}", f"-DDECALS={decals:#x}",
                        os.path.join(HERE, "weapon_camo_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_camo_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_camo_bin())} is current ({len(fresh)} bytes)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        press = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_DECALS_STATE_OFFSET
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={decals:#x}", f"-DPREVIEW={state:#x}",
                        f"-DTABLE={table_state:#x}", f"-DPRESS={press:#x}",
                        os.path.join(HERE, "weapon_decals_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_decals_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_decals_bin())} is current ({len(fresh)} bytes)")
    upgrade_state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_STATE_OFFSET
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={upgrades:#x}", f"-DSTATE={upgrade_state:#x}",
                        f"-DPREVIEW={state:#x}", f"-DFIND={stash + P.WEAPON_STASH_FIND:#x}",
                        os.path.join(HERE, "upgrade_preview_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.upgrade_preview_bin(), "rb").read(),
          f"{os.path.basename(P.upgrade_preview_bin())} is current ({len(fresh)} bytes)")
    import engine_026g
    try:
        stale = engine_026g.stale()
    except engine_026g.Refused as error:
        stale = [f"refused: {error}"]
    check(not stale, "engine_026g.h, .inc, .asm and .rs are what engine_026g.tsv and the re-kit's ABI rows give "
          "(engine_026g.py --write)" + (": not " + ", ".join(stale) if stale else ""))
    import hooks_026g
    try:
        hook_stale = hooks_026g.stale()
        hooks_026g.check_layers()
        layers = None
    except hooks_026g.Bad as error:
        hook_stale, layers = [], str(error)
    check(not hook_stale and layers is None,
          "hooks_026g.rs and hook_entries_026g.asm are what the patches' TOML gives (hooks_026g.py --write), and "
          "the Rust patches keep the raw/vostok/patches layers (no address or hand-made binding outside raw, each "
          "patch module forbids unsafe and names only vostok)"
          + (": stale " + ", ".join(hook_stale) if hook_stale else "") + (f": {layers}" if layers else ""))
    # The arena (Research, the artefacts, the engine library) is linked from
    # its sources here, as by every build, and is the lock's.
    linked = P.arena()
    check(P.linker.lock_differs(linked, P.linker.read_lock()) is None,
          f"the arena linked from its sources is {P.linker.LOCK.name}'s ({linked.size:#x} bytes, SHA-256 "
          f"{linked.sha256[:16]}..., blocks {', '.join(f'{b.name} {b.va:#x}' for b in linked.blocks.values())}; "
          "link_026g.py --write)")
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", f"-DCAVE={P.WEAPON_DRAW_VA:#x}", f"-DSTATE={state:#x}",
                        os.path.join(HERE, "weapon_draw_026g.asm"), "-o", out], check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(fresh == open(P.weapon_draw_bin(), "rb").read(),
          f"{os.path.basename(P.weapon_draw_bin())} is current ({len(fresh)} bytes)")


def test_pe(original, image, mode):
    headers, data = image.headers, image.data
    sections = image.sections
    count = len(sections)
    check(count == (8 if mode == "section" else 7), f"{count} sections")
    check(headers.table + 40 * count <= headers.size_of_headers, "the section table fits in the headers")
    previous_end_rva, previous_end_raw = headers.size_of_headers, headers.size_of_headers
    ordered = True
    for name, rva, vsize, raw, rsize, _ in sections:
        ordered &= raw % headers.file_alignment == 0 and rsize % headers.file_alignment == 0
        ordered &= raw == previous_end_raw
        ordered &= rva == (previous_end_rva + headers.section_alignment - 1) // headers.section_alignment \
            * headers.section_alignment
        previous_end_rva, previous_end_raw = rva + vsize, raw + rsize
    check(ordered, "sections are aligned, contiguous in memory and in the file")
    size_of_image = struct.unpack_from("<I", data, headers.size_of_image_at)[0]
    expected = (previous_end_rva + headers.section_alignment - 1) // headers.section_alignment * headers.section_alignment
    check(size_of_image == expected, f"SizeOfImage {size_of_image:#x} = end of the last section")
    entry = struct.unpack_from("<I", data, headers.optional + 16)[0]
    check(entry == struct.unpack_from("<I", original, headers.optional + 16)[0], "entry point unchanged")

    security_offset, security_size = image.directory(4)
    check(security_offset == previous_end_raw and security_offset + security_size == len(data)
          and security_offset % 8 == 0,
          f"certificate table at {security_offset:#x}, size {security_size:#x}: right after the last section, "
          "8-byte aligned, last in the file")
    old_offset, old_size = struct.unpack_from("<II", original, headers.security_at)
    check(bytes(data[security_offset:]) == bytes(original[old_offset:old_offset + old_size]),
          "certificate table bytes identical to the original")
    length, revision, kind = struct.unpack_from("<IHH", data, security_offset)
    check(length <= security_size and revision == 0x200 and kind == 2,
          f"WIN_CERTIFICATE length {length:#x}, revision {revision:#x}, type {kind} (PKCS#7 signed data)")
    inside = True
    for index in range(16):
        if index == 4:
            continue
        rva, size = image.directory(index)
        if size:
            inside &= any(srva <= rva and rva + size <= srva + max(vsize, rsize)
                          for _, srva, vsize, _, rsize, _ in sections)
        inside &= (rva, size) == struct.unpack_from("<II", original, headers.optional + 96 + 8 * index)
    check(inside, "the other 15 data directories are unchanged and inside sections")
    if mode == "section":
        name, rva, vsize, raw, rsize, characteristics = sections[-1]
        check(name == P.SECTION_NAME and rva == P.SECTION_RVA and characteristics == 0x60000020,
              f".srp at RVA {rva:#x} (VA {P.IMAGE_BASE + rva:#x}), virtual {vsize:#x}, file {raw:#x}+{rsize:#x}, "
              "code | execute | read")
    characteristics = struct.unpack_from("<H", data, headers.pe + 22)[0]
    dll = struct.unpack_from("<H", data, headers.optional + 70)[0]
    check(characteristics == 0x122 and dll == 0x8000,
          "file characteristics 0x122, DLL characteristics 0x8000 (no ASLR flag), as the original")


def allowed_ranges(original, mode, cave_size, server=None):
    """File ranges that may differ from the original (same offsets)."""
    headers = P.Headers(original)
    ranges = [(P.file_offset(P.CERT[0]), len(P.CERT[1]))]
    if mode == "cert-only":
        return ranges, []
    knowledge = Image(original)
    for va, old, _ in P.KNOWLEDGE_BASE:
        ranges.append((knowledge.offset(va), len(old)))
    ranges.append((P.file_offset(P.MUTEX[0]), len(P.MUTEX[1])))
    rewritten = [(P.CERT[0], P.CERT[0] + 3), (P.MUTEX[0], P.MUTEX[0] + 2)]
    rewritten.extend((va, va + len(old)) for va, old, _ in P.KNOWLEDGE_BASE[1:])
    ranges.append((P.file_offset(P.HASH_INTERVAL_BYTE_VA), 1))
    rewritten.append((P.HASH_INTERVAL_BYTE_VA, P.HASH_INTERVAL_BYTE_VA + 1))
    for va, old, _ in P.GATHER:
        ranges.append((P.file_offset(va), len(old)))
        rewritten.append((va, va + len(old)))
    ranges.append((P.file_offset(P.MEMORY_CHECK[0]), 1))
    rewritten.append((P.MEMORY_CHECK[0], P.MEMORY_CHECK[0] + 1))
    ranges.append((P.file_offset(P.MOUNT_NO_ROOT[0]), len(P.MOUNT_NO_ROOT[1])))
    rewritten.append((P.MOUNT_NO_ROOT[0], P.MOUNT_NO_ROOT[0] + len(P.MOUNT_NO_ROOT[1])))
    # The journal default's imm32 changes and keeps its relocation: it is
    # an allowed range, and not a rewritten one (test_start_up_folder).
    ranges.append((P.file_offset(P.JOURNAL_DEFAULT_VA + 1), 4))
    # Patch 43 gives back what that default withheld (test_tab_self).
    ranges.append((P.file_offset(P.TAB_SELF[0]), len(P.TAB_SELF[1])))
    rewritten.append((P.TAB_SELF[0], P.TAB_SELF[0] + len(P.TAB_SELF[1])))
    # Patch 91: on_show's aimed branch's six bytes (test_weapon_show).
    ranges.append((P.file_offset(P.WEAPON_SHOW[0]), len(P.WEAPON_SHOW[1])))
    rewritten.append((P.WEAPON_SHOW[0], P.WEAPON_SHOW[0] + len(P.WEAPON_SHOW[1])))
    # Patch 52: the Stage focus getter's 46 bytes (test_stage_focus).
    ranges.append((P.file_offset(P.STAGE_FOCUS[0]), len(P.STAGE_FOCUS[1])))
    rewritten.append((P.STAGE_FOCUS[0], P.STAGE_FOCUS[0] + len(P.STAGE_FOCUS[1])))
    if server is not None:
        va, old = P.DEFAULT_SERVER_HOOK
        ranges.append((P.file_offset(va), len(old)))
        rewritten.append((va, va + len(old)))
    ranges.append((P.file_offset(P.REVISION_SITE[0]), len(P.REVISION_SITE[1])))
    rewritten.append((P.REVISION_SITE[0], P.REVISION_SITE[0] + len(P.REVISION_SITE[1])))
    for va, old in P.HOOKS:
        ranges.append((P.file_offset(va), len(old)))
        rewritten.append((va, va + len(old)))
    ranges.append((headers.checksum_at, 4))
    if mode == "hash-mismatch":
        ranges.append((P.file_offset(P.HASH_MISMATCH_VA), P.HASH_MISMATCH_REGION))
        rewritten.append((P.HASH_MISMATCH_VA, P.HASH_MISMATCH_VA + P.HASH_MISMATCH_REGION))
    else:
        ranges += [(headers.count_at, 2), (headers.size_of_code_at, 4), (headers.size_of_image_at, 4),
                   (headers.security_at, 4), (headers.table + 40 * 7, 40)]
    return ranges, rewritten


def test_bytes(original, image, mode, cave, server=None):
    data = image.data
    ranges, rewritten = allowed_ranges(original, mode, len(cave), server)
    old_relocations = Image(original).relocations()
    disabled = [at for at, (kind, va) in old_relocations.items()
                if kind == 3 and any(va + 4 > lo and va < hi for lo, hi in rewritten)]
    ranges += [(at, 2) for at in disabled]
    common = len(original)
    if mode == "section":
        common = struct.unpack_from("<I", original, P.Headers(original).security_at)[0]
        raw_size = len(data) - len(original)
        alignment = image.headers.file_alignment
        check(raw_size == (len(cave) + alignment - 1) // alignment * alignment
              and bytes(data[common:common + raw_size]) == cave + b"\xCC" * (raw_size - len(cave)),
              f"the new section's file data is the cave ({len(cave)} bytes) + int3 padding")
    else:
        check(len(data) == len(original), "file size unchanged")
    allowed = bytearray(common)
    for at, size in ranges:
        allowed[at:at + size] = b"\1" * size
    a, b = bytes(original[:common]), bytes(data[:common])
    stray = changed = 0
    if a != b:
        for i in range(0, common, 1 << 16):
            if a[i:i + (1 << 16)] != b[i:i + (1 << 16)]:
                for j in range(i, min(i + (1 << 16), common)):
                    if a[j] != b[j]:
                        changed += 1
                        stray += not allowed[j]
    check(stray == 0, f"{changed} bytes differ from the original, all inside the {len(ranges)} intended ranges")

    new_relocations = image.relocations()
    active = [va for kind, va in new_relocations.values()
              if kind == 3 and any(va + 4 > lo and va < hi for lo, hi in rewritten)]
    same = all(new_relocations[at] == old for at, old in old_relocations.items() if at not in disabled) \
        and len(new_relocations) == len(old_relocations)
    check(not active and same and all(new_relocations[at][0] == 0 for at in disabled),
          f"{len(disabled)} relocation entries disabled, none active in rewritten bytes, the rest untouched")
    return rewritten


def test_code(original, image, mode, cave):
    cave_va = P.CAVES[mode]
    check(image.read(cave_va, len(cave)) == cave, f"the cave is mapped at {cave_va:#x}")
    rows = ndisasm(image.read(cave_va, len(cave)), cave_va)
    starts = {address for address, _, _ in rows}
    check(rows[-1][2] == "ret" and rows[-1][0] + 1 == cave_va + len(cave), "the cave disassembles cleanly to its last ret")
    seen, bad, indirect = set(), [], []
    for address, _, text in rows:
        target = branch_target(text)
        if target is not None:
            if cave_va <= target < cave_va + len(cave):
                if target not in starts:
                    bad.append(f"{address:#x}: {text} (not an instruction start)")
            elif target in EXTERNAL and (text.startswith("call") or text.startswith("jmp")):
                seen.add(target)
            else:
                bad.append(f"{address:#x}: {text}")
        elif text.startswith(("call", "jmp")):
            indirect.append(text)
            if text not in INDIRECT:
                bad.append(f"{address:#x}: {text}")
        elif re.search(r"\[(dword )?0x", text):
            bad.append(f"{address:#x}: {text} (absolute address operand)")
    check(not bad, "every branch in the cave goes to a known address" + "".join("\n          " + b for b in bad))
    check(seen == set(EXTERNAL), "the cave reaches all 7 external addresses: " +
          ", ".join(f"{va:#x}" for va in sorted(seen)))
    check(sorted(set(indirect)) == sorted(INDIRECT), "indirect calls: " + ", ".join(sorted(set(indirect))))

    entries = []
    for va, old in P.HOOKS:
        hook = ndisasm(image.read(va, len(old)), va)
        target = branch_target(hook[0][2])
        entries.append(target)
        nops = all(text == "nop" for _, _, text in hook[1:])
        check(hook[0][2].startswith("jmp") and target in starts and nops and len(hook) == len(old) - 4,
              f"hook {va:#x}: {hook[0][2]} + {len(hook) - 1} nop")
    # The two hook targets start with pushad, and each path ends by running
    # the replaced instruction or jumping where the original went.
    text_at = {address: text for address, _, text in rows}
    check(all(text_at[e] == "pusha" for e in entries), "both cave entries start with pushad")
    check("mov ebx,[ebx+0x1f1c48]" in text_at.values(), "the cave replays the replaced `mov ebx,[ebx+1F1C48h]`")
    tail = image.read(P.HOOKS[1][0] + 7, 6)
    check(tail == bytes.fromhex("0f85a6000000"), "the dead `jnz` after the request hook is the original's")
    done = Image(original).read(0x27A5AB, 13)
    check(done == bytes.fromhex("8b530803138953 04e944010000".replace(" ", "")),
          "0x27A5AB is `mov edx,[ebx+8]; add edx,[ebx]; mov [ebx+4],edx; jmp 27A6FCh`")


def test_hash_interval(original, image, mode):
    """The default of -hash_check_interval_in_ms (match.md section 14)."""
    va, old, new = P.HASH_INTERVAL
    found = image.read(va, len(old))
    if mode == "cert-only":
        check(found == old, f"{va:#x} is the original `mov dword [esp+0x18],0x64` (cert-only: 100 ms)")
        return
    rows = ndisasm(image.read(va - 10, 10 + len(new) + 5), va - 10)
    texts = [text for _, _, text in rows]
    check(found == new and texts == ["lea ecx,[esp+0x14]", "push ecx", "mov esi,0xaa19f8",
                                     "mov dword [esp+0x18],0x32", "call 0x141b0"],
          f"{va:#x} is `mov dword [esp+0x18],0x32` between `mov esi,0xaa19f8` and `call 0x141b0`: "
          f"the default hash interval is {P.HASH_INTERVAL_MS} ms")
    differing = [i for i in range(len(old)) if found[i] != old[i]]
    check(differing == [P.HASH_INTERVAL_BYTE_VA - va] and found[differing[0]] == P.HASH_INTERVAL_MS,
          f"one byte differs there: {P.HASH_INTERVAL_BYTE_VA:#x} 64 -> 32")
    relocated = sorted(rva for kind, rva in Image(original).relocations().values()
                       if kind == 3 and rva + 4 > va + 4 and rva < va + len(old))
    check(not relocated, "no base relocation of the original covers the immediate (0x27CC53..0x27CC56)")
    # The static the default lands in has one writer and the gate's two
    # reads; the option object is named only here and where it is registered.
    refs = lambda value: sorted(P.TEXT_VA + at - P.TEXT_FILE for at in
                                (m.start() for m in re.finditer(re.escape(struct.pack("<I", value)), original))
                                if P.TEXT_FILE <= at < P.TEXT_FILE + P.TEXT_VSIZE)
    check(refs(0xA2B393C) == [0x27CC62, 0x27CD30, 0x27CD5A] and refs(0xAA19F8) == [0x27CC4B, 0x853F51],
          "the interval static 0xA2B393C is written at 0x27CC60 and read at 0x27CD2E/0x27CD58 only; "
          "the option 0xAA19F8 is used at 0x27CC4A and registered at 0x853F50 only")
    inside = range(va + 1, va + len(old))
    hits = [f"{address:#x}: {text}" for address, _, text in ndisasm(Image(original).read(0x27CC45, 0x30B), 0x27CC45)
            if branch_target(text) in inside]
    check(not hits, "no branch of try_send_all lands inside that instruction")


def test_gather(original, image, mode):
    """The battery rule's container event handler (match.md section 26)."""
    # The handler: 0x313A50 to its last `ret 0Ch` at 0x313B3B.
    start, end = 0x313A50, 0x313B3E
    before = {text for _, _, text in ndisasm(Image(original).read(start, end - start), start)}
    check({"cmp dword [ecx+0x15c],byte -0x1", "jnz near 0x313b3b", "mov [ecx+0x15c],edx",
           "mov byte [ecx+0x160],0x0", "ret 0xc"} <= before,
          "the original handler has `cmp [ecx+0x15c],-1` / `jnz 0x313b3b` and `mov [ecx+0x15c],edx` / "
          "`mov byte [ecx+0x160],0` / `ret 0xc`")
    rows = ndisasm(image.read(start, end - start), start)
    at = {address: text for address, _, text in rows}
    if mode == "cert-only":
        check(all(image.read(va, len(old)) == old for va, old, _ in P.GATHER),
              "cert-only: the handler is the original's")
        return
    (events_va, events_old, _), (lock_va, lock_old, _) = P.GATHER
    nops = lambda va, size: all(at.get(va + i) == "nop" for i in range(size))
    check(at.get(0x313A5B) == "cmp dword [ecx+0x15c],byte -0x1" and nops(events_va, len(events_old))
          and at.get(0x313A68) == "mov eax,[esp+0x8]",
          f"{events_va:#x}: six nops between `cmp dword [ecx+0x15c],-1` and `mov eax,[esp+0x8]`: "
          "an event after the completion is counted")
    check(at.get(0x313B2E) == "mov [ecx+0x15c],edx" and nops(lock_va, len(lock_old))
          and at.get(0x313B3B) == "ret 0xc",
          f"{lock_va:#x}: seven nops between `mov [ecx+0x15c],edx` and `ret 0xc`: "
          "the containers stay usable")
    relocated = sorted(rva for kind, rva in Image(original).relocations().values()
                       if kind == 3 and any(rva + 4 > va and rva < va + len(old) for va, old, _ in P.GATHER))
    check(not relocated, "no base relocation of the original covers either instruction")
    interiors = [(va + 1, va + len(old)) for va, old, _ in P.GATHER]
    hits = [f"{address:#x}: {text}" for address, _, text in ndisasm(Image(original).read(start, end - start), start)
            if branch_target(text) is not None
            and any(lo <= branch_target(text) < hi for lo, hi in interiors)]
    check(not hits, "no branch of the handler lands inside either instruction")
    # The byte the second one cleared: set by the rule's constructor,
    # tested by the container's can_use, and written nowhere else as a
    # `mov byte [reg+160h],imm` of this class.
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    clears = sorted(P.TEXT_VA + m.start() for m in re.finditer(rb"\xC6[\x80-\x87]\x60\x01\x00\x00\x00", text))
    sets = sorted(P.TEXT_VA + m.start() for m in re.finditer(rb"\xC6[\x80-\x87]\x60\x01\x00\x00\x01", text))
    check(lock_va in clears and 0x313A1F in sets
          and not any(0x313000 <= va < 0x315000 for va in clears if va != lock_va),
          f"`mov byte [reg+0x160],0` is at {lock_va:#x} and nowhere else in the rule's code "
          "(the constructor sets it at 0x313A1F)")


def test_tdm_deaths(original, image, before):
    """Team Deathmatch's rule counts a death after its own end (patch
    revision 11, client_patch.md section 3.21): the `jnz` after `cmp dword
    [esi+12Ch],-1` in the death handler 0x316090 is six nops; the build
    without the late join keeps it."""
    va, old, new = P.TDM_DEATHS
    stock = Image(original)
    start, end = 0x316090, 0x316200
    rows = {address: text for address, _, text in ndisasm(image.read(start, end - start), start)}
    original_rows = ndisasm(stock.read(start, end - start), start)
    original_at = {address: text for address, _, text in original_rows}
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new,
          f"{va:#x}: the original's six bytes `{old.hex()}`, kept by the build without the late join, "
          "six 0x90 in this one")
    check(original_at.get(0x3160E1) == "cmp eax,byte +0x1" and original_at.get(0x3160E4) == "jnz near 0x3161dd"
          and original_at.get(0x3160EA) == "cmp dword [esi+0x12c],byte -0x1"
          and original_at.get(va) == "jnz near 0x3161dd" and original_at.get(0x3160F7) == "mov ecx,[esi+0x128]",
          "the original's handler: event 1 (`cmp eax,1` / `jnz 0x3161dd`), then `cmp dword [esi+0x12c],-1` / "
          "`jnz 0x3161dd` / `mov ecx,[esi+0x128]`")
    check(rows.get(0x3160EA) == "cmp dword [esi+0x12c],byte -0x1"
          and all(rows.get(va + i) == "nop" for i in range(len(old))) and rows.get(0x3160F7) == "mov ecx,[esi+0x128]",
          f"{va:#x}: six nops between `cmp dword [esi+0x12c],-1` and `mov ecx,[esi+0x128]`: a death after the "
          "rule's end is counted")
    # What the patch changes is one thing: the handler goes on to the
    # ticket, the alive mask and the end test where it returned at once.
    leaves = sorted(address for address, text in original_at.items() if branch_target(text) == 0x3161DD)
    check(leaves == [0x3160E4, va], "the handler's jumps to its return (0x3161dd) are those of the other event "
          f"(0x3160e4) and the one patched ({va:#x}): {[hex(a) for a in leaves]}")
    relocated = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not relocated, "no base relocation of the original is under the six bytes")
    hits = [f"{address:#x}: {text}" for address, _, text in original_rows
            if branch_target(text) is not None and va + 1 <= branch_target(text) < va + len(old)]
    check(not hits, "no branch of the handler lands inside the six bytes")
    # The completion time: set to -1 by the rule's constructor, set by the
    # end test, read by the end test, `rule_completed` and this guard, and
    # in no other function of the rule (so nothing else reads it as
    # "the rule has ended").
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    users = sorted(P.TEXT_VA + m.start() for m in re.finditer(rb"[\x80-\xBF]\x2C\x01\x00\x00", text)
                   if 0x315F00 <= P.TEXT_VA + m.start() < 0x316700)
    check(len(users) == 7, "the rule's code touches the completion time (+0x12c) at seven places "
          f"(constructor, this guard, the end test's four, rule_completed): {[hex(a) for a in users]}")


def test_baked_texture_missing(original, image, before):
    """A baked texture no longer in the baker's list is made again (patch
    35, client_patch.md section 3.33): in check_loaded_baked_texture
    (0x131B40), the `jnz` after the not-found assertion's "ignore" flag test
    is a `jmp` to the function's `return false`; the build without the late
    join keeps it."""
    va, old, new = P.BAKED_TEXTURE_MISSING
    # ndisasm 2.16 writes the absolute operand as `[dword ...]`.
    FLAG_TEST = ("cmp byte [0x226091d],0x0", "cmp byte [dword 0x226091d],0x0")
    stock = Image(original)
    start, end = 0x131B40, 0x131ED0
    original_rows = ndisasm(stock.read(start, end - start), start)
    original_at = {address: text for address, _, text in original_rows}
    rows = {address: text for address, _, text in ndisasm(image.read(start, end - start), start)}
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new,
          f"{va:#x}: the original's `{old.hex()}` (jnz short), kept by the build without the late join, "
          f"`{new.hex()}` (jmp short) in this one")
    text = stock.read(0x9EE3F4, 33)
    check(original_at.get(0x131C11) == "jnz 0x131bb3" and original_at.get(0x131C17) in FLAG_TEST
          and original_at.get(va) == "jnz 0x131c73" and original_at.get(0x131C27) == "push dword 0x9ee3f4"
          and original_at.get(0x131C58) == "call 0x113450" and original_at.get(0x131C72) == "int3"
          and original_at.get(0x131C73) == "lea ecx,[esp+0x48]" and original_at.get(0x131C7C) == "xor al,al"
          and original_at.get(0x131C85) in ("ret 0xc", "ret word 0xc")
          and text == b"Saved baked texture %s not found\0",
          "the original: after the list's last record (`jnz 0x131bb3`) the flag test, the assertion "
          "\"Saved baked texture %s not found\" (0x113450), `int3`; 0x131c73 lets the resource go and "
          "returns false (`xor al,al` ... `ret 0xc`)")
    check(rows.get(0x131C17) in FLAG_TEST and rows.get(va) in ("jmp short 0x131c73", "jmp 0x131c73")
          and all(rows.get(a) == original_at.get(a) for a in (0x131C20, 0x131C73, 0x131C7C, 0x131C85)),
          f"{va:#x}: `jmp short 0x131c73`, the rest of the function the original's")
    # The not-found end is reached from the list walk alone, and nothing
    # else lands on the assertion's code.
    into = sorted(address for address, _, text in original_rows
                  if not 0x131C17 <= address < 0x131C73
                  and branch_target(text) is not None and 0x131C17 <= branch_target(text) < 0x131C73)
    check(into == [0x131BAB], f"only the empty list's `je 0x131c17` (0x131bab) jumps into the not-found end: "
          f"{[hex(a) for a in into]}")
    relocated = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not relocated, "no base relocation of the original is under the two bytes")
    hits = [f"{address:#x}: {text}" for address, _, text in original_rows
            if branch_target(text) is not None and va + 1 <= branch_target(text) < va + len(old)]
    check(not hits, "no branch of the function lands inside the two bytes")


def test_ai_arena(original, image, before):
    """The memory arena "ai" is 4 MiB (patch 36, client_patch.md section
    3.34): the size its static initializer registers, an immediate; the
    build without the late join keeps 256 KiB."""
    va, old, new = P.AI_ARENA
    stock = Image(original)
    start, end = 0x1E8B50, 0x1E8D9C
    original_rows = ndisasm(stock.read(start, end - start), start)
    original_at = {address: text for address, _, text in original_rows}
    rows = {address: text for address, _, text in ndisasm(image.read(start, end - start), start)}
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new,
          f"{va:#x}: the original's `{old.hex()}` (0x40000), kept by the build without the late join, "
          f"`{new.hex()}` (0x400000) in this one")
    check(original_at.get(0x1E8C60) == "mov dword [dword 0xa967d8],0x9e68dc"
          and original_at.get(0x1E8CBD) == "mov esi,[dword 0xa808fc]" and original_at.get(0x1E8CC3) == "lea edi,[esp+0x10]"
          and original_at.get(va) == "mov dword [esp+0x18],0x40000"
          and original_at.get(0x1E8CDB) == "mov dword [esp+0x24],0x9fa4e0" and original_at.get(0x1E8CE3) == "call 0xe21c0"
          and original_at.get(0x1E8D9B) == "ret" and stock.read(0x9FA4E0, 3) == b"ai\0",
          "the original: the arena 0xa967d8's vtable store, the arenas' list [0xa808fc], the size 0x40000, "
          "the name \"ai\" (0x9fa4e0), the registration 0xe21c0; the initializer ends at 0x1e8d9b")
    check(rows.get(va) == f"mov dword [esp+0x18],{P.AI_ARENA_SIZE:#x}"
          and all(rows.get(a) == original_at.get(a) for a in (0x1E8CBD, 0x1E8CD3, 0x1E8CDB, 0x1E8CE3, 0x1E8D9B)),
          f"{va:#x}: `mov dword [esp+0x18],{P.AI_ARENA_SIZE:#x}`, the rest of the initializer the original's")
    found = image.read(va, len(old))
    check([i for i in range(len(old)) if found[i] != old[i]] == [6], f"one byte differs there: {va + 6:#x} 04 -> 40")
    named = [P.TEXT_VA + m.start() - P.TEXT_FILE for m in re.finditer(re.escape(struct.pack("<I", 0x9FA4E0)), original)
             if P.TEXT_FILE <= m.start() < P.TEXT_FILE + P.TEXT_VSIZE]
    check(named == [0x1E8CDF], f"the name \"ai\" (0x9fa4e0) is used in the code at 0x1e8cdb alone: {[hex(a) for a in named]}")
    relocated = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not relocated, "no base relocation of the original is under the instruction")
    hits = [f"{address:#x}: {text}" for address, _, text in original_rows
            if branch_target(text) is not None and va + 1 <= branch_target(text) < va + len(old)]
    check(not hits, "no branch of the initializer lands inside the instruction")


def test_memory_defaults(original, image, before):
    stock = Image(original)
    for va, old, new in P.MEMORY_DEFAULTS:
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
              and image.read(va, len(new)) == new,
              f"{va:#x}: only the full build uses the launcher's 768 MiB default")
        relocated = [rva for kind, rva in stock.relocations().values()
                     if kind == 3 and rva + 4 > va and rva < va + len(old)]
        check(not relocated, f"{va:#x}: no relocation under the numeric default")
    for va, length in ((0xE0FF0, 31), (0xE1015, 20), (0xE9956, 47), (0xE998A, 15)):
        check(image.read(va, length) == stock.read(va, length),
              f"{va:#x}: option parsing, explicit-value branch and units remain unchanged")


def test_start_up_folder(original, image, mode):
    """A Documents folder the game may not write to (client_patch.md
    section 3.10): the mount without a root, and no journal by default."""
    stock = Image(original)
    va, old, new = P.MOUNT_NO_ROOT
    journal_va, journal_old, journal_new = P.JOURNAL_DEFAULT
    if mode == "cert-only":
        check(image.read(va, len(old)) == old and image.read(journal_va, 5) == journal_old
              and image.read(P.TAB_SELF[0], len(P.TAB_SELF[1])) == P.TAB_SELF[1],
              "cert-only: mounter::finish, the journal's default and on_activate's name (patch 43) are the "
              "original's")
        check(image.read(P.STAGE_FOCUS[0], len(P.STAGE_FOCUS[1])) == P.STAGE_FOCUS[1],
              "cert-only: the Stage focus getter (patch 52) is the original's")
        check(image.read(P.WEAPON_SHOW[0], len(P.WEAPON_SHOW[1])) == P.WEAPON_SHOW[1],
              "cert-only: weapon::on_show (patch 91) is the original's")
        return
    check(image.read(P.TAB_SELF[0], len(P.TAB_SELF[2])) == P.TAB_SELF[2],
          f"{P.TAB_SELF[0]:#x}: patch 43's `jmp short` with the journal off by default: on_activate gives the hud "
          "its player's name (test_tab_self)")
    check(image.read(P.STAGE_FOCUS[0], len(P.STAGE_FOCUS[2])) == P.STAGE_FOCUS[2],
          f"{P.STAGE_FOCUS[0]:#x}: patch 52's tests in the Stage focus getter (test_stage_focus)")
    check(image.read(P.WEAPON_SHOW[0], len(P.WEAPON_SHOW[2])) == P.WEAPON_SHOW[2],
          f"{P.WEAPON_SHOW[0]:#x}: patch 91's `jmp` in weapon::on_show (test_weapon_show)")
    # mounter::finish from the read of the mount's root to its `ret 0Ch`.
    start, end = 0x46B04, 0x46B3D
    tail = ["mov eax,[eax+0x34]", "test eax,eax", "jnz 0x46b0f", "xor ecx,ecx", "jmp short 0x46b12",
            "mov ecx,[eax+0x40]", "mov ebp,[ebp+0x524]", "mov edx,[ebp+0x201c8]", "lea eax,[ebp+0x201c8]",
            "neg edx", "sbb edx,edx", "test edx,0x1af330", "jz 0x46b36", "push ecx", "call 0x10e2a0",
            "pop edi", "pop esi", "pop ebp", "pop ecx", "ret 0xc"]
    before = [text for _, _, text in ndisasm(stock.read(start, end - start), start)]
    check(before == tail,
          "the original mounter::finish ends: root = [mount+0x34]; ecx = root ? [root+0x40] : 0; "
          "the on_mounted callback, when there is one, is called with ecx")
    after = [text for _, _, text in ndisasm(image.read(start, end - start), start)]
    check(after == tail[:3] + [f"jmp short {P.MOUNT_NO_ROOT_END:#x}", "nop", "nop"] + tail[5:],
          f"{va:#x}: `jmp short {P.MOUNT_NO_ROOT_END:#x}` and two nops: a mount without a root goes to the "
          "function's end (the four pops and `ret 0xc`), the path of a mount with a root is the original's")
    relocated = [rva for kind, rva in stock.relocations().values()
                 if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not relocated, "no base relocation of the original covers the four bytes")
    whole = ndisasm(stock.read(0x469F0, 0x14D), 0x469F0)
    hits = [f"{address:#x}: {text}" for address, _, text in whole
            if branch_target(text) is not None and va < branch_target(text) < va + len(old)]
    targets = sorted({address for address, _, text in whole if branch_target(text) == P.MOUNT_NO_ROOT_END})
    check(not hits and len(targets) >= 3,
          f"no branch of mounter::finish lands inside them; {P.MOUNT_NO_ROOT_END:#x} is where its own "
          f"{len(targets)} early exits go")
    # The callback this protects: it reads the node's flags without looking.
    check(stock.read(0xF9A8B, 8) == bytes.fromhex("8b74240c0fb74630"),
          "resources_manager::on_mounted 0xF9A80 does `mov esi,[esp+0xc]` / `movzx eax,word [esi+0x30]`: "
          "a null node is an access violation at 0xF9A8F")

    # The journal: the option's initializer.
    literal = lambda value: stock.read(value - 1, 3)
    check(literal(P.JOURNAL_DEFAULT_ON) == b"\x001\x00" and literal(P.JOURNAL_DEFAULT_OFF) == b"\x000\x00",
          f'{P.JOURNAL_DEFAULT_ON:#x} is the text "1" and {P.JOURNAL_DEFAULT_OFF:#x} the text "0", '
          "both in the original's read-only data")
    initializer = lambda source, value: [text for _, _, text in ndisasm(source.read(journal_va - 5, 16), journal_va - 5)] \
        == [f"push dword {P.JOURNAL_OPTION:#x}", f"mov eax,{value:#x}", "call 0xddff0", "ret"]
    check(initializer(stock, P.JOURNAL_DEFAULT_ON) and initializer(image, P.JOURNAL_DEFAULT_OFF),
          f"{journal_va:#x}: the initializer of -record_journal ({P.JOURNAL_OPTION:#x}) gives \"0\" "
          "in the place of \"1\"; the push before it and the call after it are the original's")
    others = [at for at in (0x84CF92, 0x84E4F2, 0x853B02)
              if [text for _, _, text in ndisasm(stock.read(at, 16), at)][1:]
              == [f"mov eax,{P.JOURNAL_DEFAULT_OFF:#x}", "call 0xddff0", "ret"]]
    check(len(others) == 3, "three other options of the original are given that \"0\" by the same three instructions")
    name, description = stock.read(0x9FB560, 15), stock.read(0x9FB544, 26)
    check(name == b"record_journal\0" and description == b"disable journal recording\0"
          and stock.read(P.JOURNAL_OPTION + 0x210, 4) == struct.pack("<I", 0x9FB560),
          "the option object's name is `record_journal` (its description: \"disable journal recording\")")
    kinds = lambda source: sorted(kind for kind, rva in source.relocations().values() if rva == journal_va + 1)
    check(kinds(stock) == [3] and kinds(image) == [3],
          f"the base relocation of the address ({journal_va + 1:#x}) is active in the original and stays so")
    # One reader: the game's start decides between replay, record and none.
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    readers = sorted(P.TEXT_VA + m.start() for m in re.finditer(re.escape(struct.pack("<I", P.JOURNAL_OPTION)), text))
    check(readers == [0x1F69B3, 0x851CB1, 0x851CC3] and stock.read(0x1F69B2, 19) == bytes.fromhex(
        "be28d6a900e804dce1ff33d284c00f95c28bf2"),
          "the option is named where it is registered and at 0x1F69B2 only: `mov esi,0xa9d628` / "
          "`call 0x145c0` / `setnz dl` / `mov esi,edx`, the journal's use (0: none, 1: record)")
    check(stock.read(0xEB317, 4) == bytes.fromhex("85f67463"),
          "journaling's start 0xEB310 makes no journal for use 0 (`test esi,esi` / `jz 0xeb37e`)")


def test_memory_check(original, image, mode):
    """The launcher's 73 -> EB patch skips only the fatal 320 MiB gate."""
    va, old, new = P.MEMORY_CHECK
    found = image.read(va, len(old))
    check(P.file_offset(va) == 0xF5811, "VRAM patch uses the Linux launcher's file offset 0xF5811")
    if mode == "cert-only":
        check(found == old, "cert-only keeps the original minimum VRAM check")
        return
    rows = ndisasm(found, va)
    check(found == new and len(rows) == 1 and rows[0][2].startswith("jmp ")
          and branch_target(rows[0][2]) == 0x106422,
          "0x106411 unconditionally jumps to the detected-size return at 0x106422")
    check([i for i in range(len(old)) if found[i] != old[i]] == [0],
          "only the branch opcode changes: 73 -> EB; displacement 0F stays")
    # A zero/unknown size returns 320 MiB before reaching the patched gate.
    fallback = bytes.fromhex("8bc30bc7750eb80000001433d25f5e5b83c408c3")
    measured = bytes.fromhex("8bd75f5e8bc35b83c408c3")
    check(image.read(0x1063F1, len(fallback)) == Image(original).read(0x1063F1, len(fallback)) == fallback,
          "unknown-size detection and its 320 MiB fallback are unchanged")
    check(image.read(0x106422, len(measured)) == Image(original).read(0x106422, len(measured)) == measured,
          "the return still uses the detected size in EDI:EBX without inflating it")
    relocated = [rva for kind, rva in Image(original).relocations().values()
                 if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not relocated, "no base relocation overlaps the VRAM branch")
    # Exercise the site's byte guard separately from the full-file hash guard.
    bad = bytearray(original)
    bad[P.file_offset(va)] ^= 1
    before = bytes(bad)
    try:
        P.patch(bad, *P.MEMORY_CHECK)
    except SystemExit:
        check(bytes(bad) == before, "a changed VRAM instruction is rejected without modifying it")
    else:
        check(False, "a changed VRAM instruction must be rejected")


def test_default_server(original, image, mode, server):
    va, old = P.DEFAULT_SERVER_HOOK
    if server is None:
        check(image.read(va, len(old)) == old, "without an embedded server the original missing-server path stays")
        return
    start = P.CAVES[mode] + P.DEFAULT_SERVER_OFFSET
    stub = open(P.default_server_bin(mode), "rb").read()
    hook = ndisasm(image.read(va, len(old)), va)
    check(hook[0][2].startswith("jmp ") and branch_target(hook[0][2]) == start
          and all(text == "nop" for _, _, text in hook[1:]),
          "missing-server error jumps to the embedded-address stub")
    rows = ndisasm(image.read(start, len(stub)), start)
    key_delta = P.DEFAULT_SERVER_OPTION - (start + 9)
    key_operand = f"esi{'+' if key_delta >= 0 else '-'}{abs(key_delta):#x}"
    check([text.replace(",byte +", ",") for _, _, text in rows] == [
        "push ecx", "push edx", "push esi", "push edi", f"call {start + 9:#x}", "pop esi",
        f"lea edx,[{key_operand}]", "add esi,0x2a", "mov edi,[edx]", "cld", "lodsb", "stosb",
        "test al,al", f"jnz {start + 22:#x}", "dec edi", "mov [edx+0x4],edi",
        "mov dword [edx+0x224],0x4", "pop edi", "pop esi", "pop edx", "pop ecx",
        f"jmp {P.DEFAULT_SERVER_RETURN:#x}"],
        "stub initializes the server key and type_string, preserves registers and repeats the original option read")
    # Re-enter before all string initialization. At the hook EDX no longer
    # holds the server buffer limit; entering at 0x1EAE47 stores a stale EDX.
    check(P.DEFAULT_SERVER_RETURN == 0x1EAE2D and image.read(P.DEFAULT_SERVER_RETURN, 50)
          == bytes.fromhex("8d8578fbffff8bc889856cfbffff898d70fbffff"
                           "8d9578fdffff8d856cfbffffb950c5a900"
                           "899574fbffffc68578fbffff00"),
          "fallback rebuilds the stack string, including its 512-byte buffer limit, before reading the key")
    encoded = P.parse_server(server).encode("ascii") + b"\0"
    check(image.read(start + len(stub), len(encoded)) == encoded,
          "embedded host:port is terminated and fits the client buffers")
    # Preserve all option checks, address parsing and client creation up to
    # the old error. Explicit -server and -replay never enter our stub.
    begin = 0x1EAE2D
    check(image.read(begin, va - begin) == Image(original).read(begin, va - begin),
          "explicit-server and replay selection, parsing and connection code are unchanged")
    missing = ndisasm(image.read(0x1EB0AB, 6), 0x1EB0AB)
    check(missing[0][2].startswith("jz ") and branch_target(missing[0][2]) == va,
          "the original no-server/no-replay branch is what reaches the hook")


def test_server_options(original):
    import make_client_026g as overlay

    for value in ("026g.skhid.link:1234", "localhost:1", "a" * 63 + ":65535"):
        check(P.parse_server(value) == value, f"accepts build-time server {value}")
    for value in ("host", "host:0", "host:65536", "host:-1", "host:１２３４",
                  "a" * 64 + ":1234", "host:1234\0", "host:1234\n", "host;other:1234"):
        try:
            P.parse_server(value)
        except SystemExit:
            pass
        else:
            check(False, f"invalid address accepted: {value!r}")
    try:
        P.build(original, "cert-only", server="localhost:1234")
    except SystemExit:
        check(True, "cert-only refuses an embedded server")
    else:
        check(False, "cert-only must refuse an embedded server")
    for embedded in (False, True):
        line = overlay.launch_line("localhost:1234", embedded)
        batch = overlay.bat("localhost:1234", embedded).decode("ascii")
        start = next(line for line in batch.splitlines() if line.startswith('start "" '))
        check(("-server=" in line) != embedded and ("-server=" in start) != embedded
              and "-localization" not in line and "-localization" not in start,
              f"overlay launch line and batch use {'embedded' if embedded else 'explicit'} server, no language")


# sign_in_on_connected: from the copy of the version string to the write of
# the byte after the field (client_patch.md section 6).
VERSION_FIELD = (0x380847, 0x38088F)
SIGN_IN_ON_CONNECTED = (0x3807D0, 0x380940)   # to `sign_in`
REGISTERS = {"eax": ("eax", 4), "ax": ("eax", 2), "al": ("eax", 1),
             "ecx": ("ecx", 4), "cx": ("ecx", 2), "cl": ("ecx", 1),
             "edx": ("edx", 4), "dx": ("edx", 2), "dl": ("edx", 1)}


def version_field(image):
    """The 13 bytes sign_in_on_connected writes at edi after the user name
    (the 12-byte version field and the byte after it), worked out from its
    instructions. A byte is an int, "eula" (the flag read from [ebp+10h]) or
    None (not known). Returns None and says why when an instruction is not
    one of the forms the original has."""
    start, end = VERSION_FIELD
    registers = {"eax": [None] * 4, "ecx": [None] * 4, "edx": [None] * 4}
    xmm0, stack, out = [None] * 8, {}, [None] * 13

    def get(name):
        register, size = REGISTERS[name]
        return registers[register][:size]

    def put(name, values):
        register, size = REGISTERS[name]
        assert len(values) == size
        registers[register][:size] = values

    absolute = re.compile(r"^mov (\w+),\[(?:dword )?0x([0-9a-f]+)\]$")
    to_stack = re.compile(r"^mov \[esp\+0x([0-9a-f]+)\],(\w+)$")
    immediate = re.compile(r"^mov byte \[esp\+0x([0-9a-f]+)\],0x([0-9a-f]+)$")
    to_field = re.compile(r"^mov \[edi\+0x([0-9a-f]+)\],(\w+)$")
    for address, _, text in ndisasm(image.read(start, end - start), start):
        if m := absolute.match(text):
            size = REGISTERS[m.group(1)][1]
            put(m.group(1), list(image.read(int(m.group(2), 16), size)))
        elif text == "pxor xmm0,xmm0":
            xmm0 = [0] * 8
        elif m := re.match(r"^movq \[esp\+0x([0-9a-f]+)\],xmm0$", text):
            for i, value in enumerate(xmm0):
                stack[int(m.group(1), 16) + i] = value
        elif m := re.match(r"^movq xmm0,\[esp\+0x([0-9a-f]+)\]$", text):
            xmm0 = [stack.get(int(m.group(1), 16) + i) for i in range(8)]
        elif m := to_stack.match(text):
            for i, value in enumerate(get(m.group(2))):
                stack[int(m.group(1), 16) + i] = value
        elif m := immediate.match(text):
            stack[int(m.group(1), 16)] = int(m.group(2), 16)
        elif text == "xor eax,eax":
            put("eax", [0] * 4)
        elif text == "mov ecx,[ebp+0xc]":
            put("ecx", [None] * 4)
        elif text == "mov dl,[ebp+0x10]":
            put("dl", ["eula"])
        elif text == "movq [edi],xmm0":
            out[:8] = xmm0
        elif m := to_field.match(text):
            for i, value in enumerate(get(m.group(2))):
                out[int(m.group(1), 16) + i] = value
        elif text == f"call {P.STALE_DLL_ENTRY:#x}":
            # stale_dll_026g.asm: what `mov [edi+8],eax` / `mov [edi+0Ch],dl`
            # did, with its flags in byte 8 (check 26).
            out[8:13] = ["stale_dll", 0, 0, 0] + get("dl")
        elif text not in ("nop", "add edi,esi"):
            check(False, f"{address:#x}: `{text}` is not an instruction of the version field's code")
            return None
    return out


def test_revision(original, image, mode, revision, late_join=False):
    """The patch revision in the sign-in (client_patch.md section 6).
    late_join: the default build, whose sign-in also has the shell_32.dll
    check."""
    va, old, new = P.revision_site(revision)
    stock = list(b"0.26g0" + bytes(6)) + ["eula"]
    check(version_field(Image(original)) == stock,
          'the original fills the version field with "0.26g0" and six zero bytes, then the EULA flag')
    field = version_field(image)
    if mode == "cert-only":
        check(image.read(va, len(old)) == old and field == stock, "cert-only: the version field's code is the original's")
        return
    check(1 <= revision <= 255 and new == bytes([0xC6, 0x44, 0x24, 0x27, revision, 0x90]),
          f"the revision is {revision}: a byte, not 0")
    rows = {address: text for address, _, text in ndisasm(image.read(*[VERSION_FIELD[0], VERSION_FIELD[1] - VERSION_FIELD[0]]),
                                                         VERSION_FIELD[0])}
    check(rows.get(0x38085E) == "mov [esp+0x20],ecx" and rows.get(va) == f"mov byte [esp+0x27],{revision:#x}"
          and rows.get(va + 5) == "nop" and rows.get(0x380868) == "xor eax,eax",
          f"{va:#x}: `mov byte [esp+0x27],{revision:#x}` / `nop` between `mov [esp+0x20],ecx` and `xor eax,eax`")
    stale_dll = late_join                       # the build with the late join (from revision 12) has it
    expected = list(b"0.26g0" + bytes([0, revision])) + (["stale_dll"] if stale_dll else [0]) + [0, 0, 0, "eula"]
    check(field == expected,
          f'the version field is "0.26g0", a zero, the revision {revision} in byte 7, '
          + ("the shell_32.dll check's flags in byte 8 and three zero bytes" if stale_dll else "four zero bytes")
          + "; the EULA flag follows as before")
    inside = lambda relocations: sorted((address, kind) for kind, address in relocations.values()
                                        if VERSION_FIELD[0] <= address < VERSION_FIELD[1])
    check(inside(Image(original).relocations()) == [(0x380849, 3), (0x380850, 3), (0x380864, 3)],
          "the original has three base relocations in this code: 0x380849, 0x380850, 0x380864")
    check(inside(image.relocations()) == [(0x380849, 3), (0x380850, 3), (0x380864, 0)],
          "the one under the rewritten instruction (0x380864) is disabled, the other two are untouched")
    start, end = SIGN_IN_ON_CONNECTED
    hits = [f"{address:#x}: {text}" for address, _, text in ndisasm(Image(original).read(start, end - start), start)
            if branch_target(text) is not None and va < branch_target(text) < va + len(old)]
    check(not hits, "no branch of sign_in_on_connected lands inside that instruction")


def test_no_branch_into_hooks(original):
    print("branches into the hooks (original exe)")
    image = Image(original)
    interiors = [(va + 1, va + len(old)) for va, old in [*P.HOOKS, P.DEFAULT_SERVER_HOOK]]
    inside = lambda target: any(lo <= target < hi for lo, hi in interiors)
    hits = []
    for start, end in HOSTS:
        for address, _, text in ndisasm(image.read(start, end - start), start):
            target = branch_target(text)
            if target is not None and inside(target):
                hits.append(f"{address:#x}: {text}")
    check(not hits, "no branch of the host functions lands inside a hook" + "".join("\n   " + h for h in hits))
    table = struct.unpack_from(f"<{JUMP_TABLE[1]}I", original, image.offset(JUMP_TABLE[0]))
    check(not any(inside(t) for t in table) and table[19] == 0x27A4F4,
          "the dispatcher's jump table has no entry inside a hook; 0xF6 -> 0x27A4F4")
    # The whole .text: any rel32 call/jmp/jcc or rel8 jmp/jcc whose target is
    # inside a hook, and any 4-byte value equal to such an address.
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    found = []
    for lo, hi in interiors:
        for target in range(lo, hi):
            for at in range(max(0, target - P.TEXT_VA - 0x90), min(len(text), target - P.TEXT_VA + 0x90)):
                op = text[at]
                if (op == 0xEB or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3) and at + 2 <= len(text):
                    if P.TEXT_VA + at + 2 + struct.unpack_from("<b", text, at + 1)[0] == target:
                        found.append((P.TEXT_VA + at, "rel8"))
            pointer = struct.pack("<I", target)
            at = original.find(pointer)
            while at != -1:
                found.append((at, "absolute value (file offset)"))
                at = original.find(pointer, at + 1)
    position = 0
    while True:
        e8, e9 = text.find(b"\xE8", position), text.find(b"\xE9", position)
        candidates = [x for x in (e8, e9) if x != -1]
        if not candidates:
            break
        at = min(candidates)
        position = at + 1
        if at + 5 <= len(text):
            target = (P.TEXT_VA + at + 5 + struct.unpack_from("<i", text, at + 1)[0]) & 0xFFFFFFFF
            if inside(target):
                found.append((P.TEXT_VA + at, "rel32"))
    position = 0
    while True:
        at = text.find(b"\x0F", position)
        if at == -1:
            break
        position = at + 1
        if at + 6 <= len(text) and 0x80 <= text[at + 1] <= 0x8F:
            target = (P.TEXT_VA + at + 6 + struct.unpack_from("<i", text, at + 2)[0]) & 0xFFFFFFFF
            if inside(target):
                found.append((P.TEXT_VA + at, "jcc rel32"))
    # A rel8 hit inside the host function is real only if the linear
    # disassembly above also saw it; anything else is a byte coincidence.
    check(not found, "brute-force scan of .text: no rel8/rel32 branch bytes or pointer values aim inside a hook"
          + "".join(f"\n          {a:#x} {kind}" for a, kind in found))


# What the late join's rewritten sites are meant to disassemble to.
LATE_JOIN_TEXT = {
    0x279610: "mov eax,0x3", 0x27A2E0: "mov eax,0x3", 0x3237B4: "mov dl,0x14", 0x26D8F3: "mov al,0x14",
    0x2F5ACF: "cmp bl,0x14", 0x2F5AF4: "cmp bl,0x14", 0x2F5BE6: "cmp bl,0x14", 0x2F5C03: "cmp bl,0x14",
}


def late_join_sites():
    """[(address, length)] of every site the late join changes beside the
    revision and the ready hook, which the build without it changes too."""
    return [(va, len(old)) for va, old, _ in P.LATE_JOIN_PATCHES] + \
        [(va, len(old)) for va, old, _, _ in P.LATE_JOIN_HOOKS] + \
        [(P.RUN_UNFOCUSED_VA, len(P.RUN_UNFOCUSED[1])), (P.REMOVE_MODEL_VA, len(P.REMOVE_MODEL[1])),
         (P.PREGAME_PROGRESS_VA, len(P.PREGAME_PROGRESS[1])), (P.TDM_DEATHS[0], len(P.TDM_DEATHS[1])),
         (P.REPLAY_REATTACH_THUNK[0], len(P.REPLAY_REATTACH_THUNK[1])),
         (P.SOUND_QUEUE_CAVE_SITE[0], len(P.SOUND_QUEUE_CAVE_SITE[1])),
         (P.SOUND_QUEUE_HOOK[0], len(P.SOUND_QUEUE_HOOK[1])),
         (P.SEEK_CLEAR_HOOK[0], len(P.SEEK_CLEAR_HOOK[1])),
         (P.REPLAY_LINK_HOOK[0], len(P.REPLAY_LINK_HOOK[1]))] + \
        [(va, len(old)) for va, old, _, _ in P.SHARED_UPGRADES_HOOKS] + \
        [(va, len(old)) for va, old, _ in P.FOV_PATCHES] + pickup_sites() + \
        [(va, len(old)) for va, old, _, _ in P.KIT_CHANGE_HOOKS] + [(P.KIT_PICK[0], len(P.KIT_PICK[1]))] + \
        [(P.WARMUP_LOOK_HOOK[0], len(P.WARMUP_LOOK_HOOK[1])), (P.WARMUP_LIMITS_HOOK[0], len(P.WARMUP_LIMITS_HOOK[1])),
         (P.WARMUP_KEYS_HOOK[0], len(P.WARMUP_KEYS_HOOK[1]))] + \
        [(P.READ_ORDER_VA, P.READ_ORDER_SIZE)] + \
        [(P.BAKED_TEXTURE_MISSING[0], len(P.BAKED_TEXTURE_MISSING[1]))] + \
        [(P.AI_ARENA_VA, len(P.AI_ARENA[1]))] + \
        [(va, len(old)) for va, old, _ in P.MEMORY_DEFAULTS] + \
        [(va, len(old)) for va, old, _, _ in P.WEAPON_PREVIEW_HOOKS] + \
        [(P.WEAPON_DRAW_HOOK[0], len(P.WEAPON_DRAW_HOOK[1]))] + [(va, len(old)) for va, old, _, _ in P.RUST_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.TAB_LIST_HOOKS] + [(P.TAB_LIST_LOOP_VA, len(P.TAB_LIST_LOOP[1]))] + \
        [(P.SPOT_COOLDOWN[0], len(P.SPOT_COOLDOWN[1])), (P.SPOT_HOOK[0], len(P.SPOT_HOOK[1]))] + \
        [(va, len(old)) for va, old, _, _ in P.FOV_LOD_HOOKS] + \
        [(P.DEATH_VIEW_HOOK[0], len(P.DEATH_VIEW_HOOK[1]))] + \
        [(P.WEAPON_CAMO_HOOK[0], len(P.WEAPON_CAMO_HOOK[1]))] + \
        [(va, len(old)) for va, old, _, _ in P.WEAPON_TABLE_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.WEAPON_DECALS_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.UPGRADE_PREVIEW_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.RESEARCH_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.ARTEFACTS_HOOKS] + \
        [(va, len(old)) for va, old, _ in P.ARTEFACTS_SIZES] + \
        [(va, len(old)) for va, old, _, _ in P.MATCH_KEYS_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.HIDE_UI_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.HIDE_UI_TABLE_REFS] + \
        [(va, len(old)) for va, old, _ in P.HIDE_UI_TABLE_COUNTS] + \
        [(va, len(old)) for va, old, _, _ in P.KILL_FEED_HOOKS] + \
        [(P.KILL_FEED_ARGUMENT[0], len(P.KILL_FEED_ARGUMENT[1]))] + \
        [(va, len(old)) for va, old, _ in P.FALL_GRAVITY] + \
        [(va, len(old)) for va, old, _, _ in P.BAKE_CACHE_HOOKS] + \
        [(va, len(old)) for va, old, _, _ in P.WEIGHT_UI_HOOKS] + \
        [(P.BAKE_CACHE_KEEP[0], len(P.BAKE_CACHE_KEEP[1]))]


def pickup_sites():
    """[(address, length)] of patch 23's sites: the frame and the three hooks."""
    return [(P.PICKUP_FRAME[0], len(P.PICKUP_FRAME[1]))] + [(va, len(old)) for va, old, _, _ in P.PICKUP_HOOKS]


def test_late_join_bin():
    print("late-join code (late_join_026g.section.bin)")
    code = open(P.late_join_bin(), "rb").read()
    kept = json.load(open(os.path.splitext(P.late_join_bin())[0] + ".json", encoding="utf-8"))
    check(kept["size"] == len(code) and kept["sha256"] == hashlib.sha256(code).hexdigest(),
          f"the .bin is the one its .json describes ({len(code)} bytes, built with {kept['built_with']['cc']})")
    names = {"LJ_COMPLETE": "lj_complete", "LJ_PREPARE": "lj_prepare", "LJ_ACTIVATE": "lj_activate",
             "LJ_DEACTIVATE": "lj_deactivate", "LJ_DESTROYED": "lj_player_destroyed",
             "LJ_INSTALL": "lj_native_install", "LJ_ANIMATIONS": "lj_native_animations"}
    c_code = P.LATE_JOIN_VA + P.LATE_JOIN_ASM_SIZE
    defines = [f"-DCAVE={P.LATE_JOIN_VA}", f"-DC_CODE={c_code}"]
    defines += [f"-D{define}={c_code + kept['symbols'][name]}" for define, name in names.items()]
    defines += [f"-DSTATE_VA={P.late_join_state_va(code)}", f"-DUPLOAD_READY={P.SECTION_VA}"]
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "out.bin")
        subprocess.run([tool("nasm"), "-f", "bin", *defines, os.path.join(HERE, "late_join_026g.asm"), "-o", out],
                       check=True, timeout=60)
        fresh = open(out, "rb").read()
    check(len(fresh) == P.LATE_JOIN_ASM_SIZE and fresh == code[:P.LATE_JOIN_ASM_SIZE],
          f"its first {P.LATE_JOIN_ASM_SIZE} bytes are what nasm makes of late_join_026g.asm today")
    check(all(0 < offset < len(code) - P.LATE_JOIN_ASM_SIZE for offset in kept["symbols"].values()),
          "the C functions the assembly calls are inside the C code")
    entries = sorted({offset for _, _, offset, _ in P.LATE_JOIN_HOOKS + P.TAB_LIST_HOOKS + P.FOV_LOD_HOOKS}
                     | {P.LATE_JOIN_READY_OFFSET})
    check(all(code[offset] != 0xCC and (offset == 0 or code[offset - 1] == 0xCC) for offset in entries),
          f"the {len(entries)} entries the hooks go to start code after int3 padding")
    return code


def test_clock_factor(original, image, before):
    """Check 16: the game's time_floating_factor."""
    source = Image(original)
    va, old, new = P.CLOCK_FACTOR_PATCH
    value = lambda blob: struct.unpack("<f", blob)[0]
    check(source.read(va, 4) == old and before.read(va, 4) == old and image.read(va, 4) == new
          and abs(value(new) - P.CLOCK_FACTOR) < 1e-7 and abs(value(old) - P.CLOCK_FACTOR_ORIGINAL) < 1e-8,
          f"{va:#x}: the default of time_floating_factor is {value(image.read(va, 4)):g}; the original and the "
          f"build without the late join have {value(source.read(va, 4)):g}")
    low, high = value(source.read(P.CLOCK_FACTOR_MIN_VA, 4)), value(source.read(P.CLOCK_FACTOR_MAX_VA, 4))
    register = [text for _, _, text in ndisasm(source.read(P.CLOCK_FACTOR_REGISTER_VA - 0x39, 0x63),
                                               P.CLOCK_FACTOR_REGISTER_VA - 0x39)]
    wanted = [f"mov ecx,0x{P.CLOCK_FACTOR_OPTION:x}",
              f"movss xmm0,dword [dword 0x{P.CLOCK_FACTOR_MIN_VA:x}]",
              f"movss dword [dword 0x{P.CLOCK_FACTOR_OPTION + 0x44:x}],xmm0",
              f"movss xmm0,dword [dword 0x{P.CLOCK_FACTOR_MAX_VA:x}]",
              f"mov dword [dword 0x{P.CLOCK_FACTOR_OPTION + 0x40:x}],0x{va:x}",
              f"movss dword [dword 0x{P.CLOCK_FACTOR_OPTION + 0x48:x}],xmm0"]
    name_at = struct.unpack("<I", source.read(P.CLOCK_FACTOR_OPTION + 0x10, 4))[0]
    name = source.read(name_at, 21)
    check(all(text in register for text in wanted) and register[-1] == "ret"
          and name == b"time_floating_factor\0",
          f"the console variable at {P.CLOCK_FACTOR_OPTION:#x} is `time_floating_factor`: its value is the float at "
          f"{va:#x}, its least {low:g} and its most {high:g} (registered at {P.CLOCK_FACTOR_REGISTER_VA - 0x39:#x})")
    check(abs(low - 0.001) < 1e-9 and high == 0.5 and low <= P.CLOCK_FACTOR <= high,
          f"{P.CLOCK_FACTOR:g} is inside the range the game allows the setting")
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    needle, held, at = struct.pack("<I", va), [], 0
    while (at := text.find(needle, at)) >= 0:
        held.append(P.TEXT_VA + at)
        at += 1
    read = [text for _, _, text in ndisasm(source.read(P.CLOCK_FACTOR_READ_VA, 0x2A), P.CLOCK_FACTOR_READ_VA)]
    check(held == [P.CLOCK_FACTOR_READ_VA + 4, P.CLOCK_FACTOR_REGISTER_VA + 6]
          and read[0] == f"movss xmm0,dword [dword 0x{va:x}]" and read[1] == "lea esi,[ebx+0x52d8]"
          and "call 0x146b60" in read and read[-1] == "movss dword [esi+0x28],xmm0",
          f"the value's address is in the code twice: the registration, and the `game` constructor "
          f"({P.CLOCK_FACTOR_READ_VA:#x}), which puts it into the game clock's timer (game+52D8h) at +28h")
    offset = [text for _, _, text in ndisasm(source.read(0x1EE9E0, 0x4C), 0x1EE9E0)]
    handler = [text for _, _, text in ndisasm(source.read(0x27B7F0, 0x34), 0x27B7F0)]
    check("movss xmm0,dword [esi+0x28]" in offset and "mulss xmm0,xmm1" in offset
          and "movss dword [esi+0x24],xmm0" in offset
          and "add esi,0x52d8" in handler and "call 0x1ee9e0" in handler,
          "set_offset (0x1EE9E0) makes the timer's rate of that field and the correction's sign, and the 0xE7 "
          "handler (0x27B7F0) calls it for the game clock's timer with the offset as it came")
    under = lambda relocations: [at for kind, at in relocations.values() if kind == 3 and at + 4 > va and at < va + 4]
    check(not under(source.relocations()) and not under(image.relocations()),
          "no base relocation is under the four bytes, in the original or in the build")


def test_statistics_count(original, image, before):
    """Check 17: the statistics handler keeps the score of all twenty places."""
    source = Image(original)
    va, old, new = P.STATISTICS_COUNT
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new,
          f"{va:#x}: start_match's count is `mov al,0x14` + 4 nop; the original and the build without the late join "
          "copy the roster's")
    start = P.STATISTICS_START_MATCH_VA
    texts = lambda blob, at: [text for _, _, text in ndisasm(blob.read(at, 0xE5), at)]
    was, now = texts(source, start), texts(image, start)
    wanted = ["mov [esi+0x3238],ecx", "mov [ecx+0x18fa8],esi", "mov [esi+0x323c],edx", "mov al,[edx+0x577b5]",
              "push edi", "mov [esi+0x3240],al", "mov eax,[esi+0x3218]"]
    patched = wanted[:3] + ["mov al,0x14"] + ["nop"] * 4 + wanted[4:]
    at = was.index(wanted[0]) if wanted[0] in was else -1
    loop = ["cmp bl,[esi+0x3240]", "movzx edx,word [esi+ecx*2+0x31f0]", "call 0x4d370"]
    live = ["mov al,[edx+0x577b5]", "mov [esi+0x31e8],al"]
    pairs = lambda rows: [i for i in range(len(rows) - 1) if rows[i:i + 2] == live]
    check(at >= 0 and was[at:at + len(wanted)] == wanted and all(text in was for text in loop)
          and len(pairs(was)) == 1 and len(pairs(now)) == 1 and pairs(was)[0] + 4 == pairs(now)[0]
          and now[at:at + len(patched)] == patched and was[at + len(wanted):] == now[at + len(patched):],
          f"start_match ({start:#x}) puts the loaded count in +3240h alone, tells the hud the score of every counted "
          "place (+31F0h, through 0x4D370), and copies the roster into the shared statistics' live count (+31E8h) "
          "apart; the build keeps that copy")
    deserialize = texts(source, 0x26DB60)[:7]
    recalculate = [text for _, _, text in ndisasm(source.read(0x26DC30, 0x190), 0x26DC30)]
    slots = [struct.unpack("<I", image.read(0xA0755C + slot, 4))[0] for slot in (0x34, 0x3C, 0x44)]
    check(deserialize[5:7] == ["call 0x2f5be0", "call 0x26dc30"] and "cmp al,[edi+0x3240]" in recalculate
          and "mov [edi+eax*2+0x31f0],bx" in recalculate and slots == [0x26DB60, start, 0x26DB80],
          "the handler's deserialize (slot +34h) reads the shared statistics and runs recalculate_scores (0x26DC30), "
          "which works out every counted place's score into +31F0h; slot +3Ch is start_match, +44h on_event")
    # Every instruction of .text with +3240h on a [register+disp32] operand.
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    sites, at = [], 0
    while (at := text.find(b"\x40\x32\x00\x00", at)) >= 0:
        if at >= 2 and text[at - 1] & 0xC0 == 0x80 and text[at - 1] & 7 != 4:
            sites.append(P.TEXT_VA + at - 2)
        at += 1
    first = lambda address: ndisasm(source.read(address, 8), address)[0][2]
    zeroing = {0x26D78E: (0x26D76A, "xor ebx,ebx"), 0x26D8C4: (0x26D89B, "xor edx,edx"),
               0x26D9E0: (0x26D9DE, "xor eax,eax"), 0x278CD0: (0x278CCE, "xor eax,eax")}
    readers = {0x26D91D: "cmp bl,[esi+0x3240]", 0x26DC54: "cmp al,[edi+0x3240]", 0x26D8FA: "mov [esi+0x3240],al"}
    check(sorted(sites) == sorted([*zeroing, *readers])
          and all(first(site) == text_ for site, text_ in readers.items())
          and all(first(site).startswith("mov [esi+0x3240],") and first(zero) == text_
                  for site, (zero, text_) in zeroing.items()),
          f"+3240h is in .text {len(sites)} times: start_match's store and loop, recalculate_scores' loop, and four "
          "stores of a register zeroed before them (the constructor, clear, and the two finish_match); on_event "
          "(0x26DB80, any place) has none")
    inside = lambda target: va < target < va + len(old)
    found = []
    for at in range(len(text) - 6):
        op, here = text[at], P.TEXT_VA + at
        if op in (0xE8, 0xE9) and inside(here + 5 + struct.unpack_from("<i", text, at + 1)[0]):
            found.append(here)
        elif op == 0x0F and 0x80 <= text[at + 1] <= 0x8F and inside(here + 6 + struct.unpack_from("<i", text, at + 2)[0]):
            found.append(here)
        elif (op == 0xEB or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3) and va - 0x82 <= here < va + len(old) \
                and inside(here + 2 + struct.unpack_from("<b", text, at + 1)[0]):
            found.append(here)
    found += [va + 1 + i for i in range(len(old) - 1) if original.find(struct.pack("<I", va + 1 + i)) != -1]
    # clear (0x26D880) and start_match, as the game runs them.
    rows = ndisasm(source.read(0x26D880, start + 0xE5 - 0x26D880), 0x26D880)
    check(not found and not any(inside(branch_target(text) or 0) for _, _, text in rows),
          "no rel32 branch of .text, no rel8 branch byte near them, no branch of clear or start_match and no 4-byte "
          "value aims inside the six bytes" + "".join(f"\n          {a:#x}" for a in found))


def test_weapon_camo(original, image, before):
    """Check 44: a weapon wears the camouflage its item holds (patch 44).
    check_weapon_camo_026g.py runs the site and the code on made-up configs
    and models."""
    source = Image(original)
    plain = lambda text: text.replace("[dword 0x", "[0x").replace("dword [", "[")
    at = lambda start, size: {a: plain(t) for a, _, t in ndisasm(source.read(start, size), start)}
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    code = open(P.weapon_camo_bin(), "rb").read()
    start = at(0x24ACF0, 0x40)
    model = at(0x24AD74, 0x20)
    site = at(0x24B0CC, 0x20)
    check(start.get(0x24ACF0) == "sub esp,byte +0x24" and start.get(0x24ACF3) == "push ebx"
          and start.get(0x24ACF4) == "push ebp" and start.get(0x24ACFC) == "push esi" and start.get(0x24ACFD) == "push edi"
          and start.get(0x24ACFE) == "mov edi,[esp+0x3c]" and start.get(0x24AD06) == "mov eax,[edi+0x108]"
          and model.get(0x24AD7E) == "mov edi,eax" and model.get(0x24AD80) == "mov [esp+0x20],edi"
          and source.read(0x24B18C, 3) == bytes.fromhex("c21400"),
          "the original's weapon_cook::on_weapon_subresources_ready: 24h of locals and four registers, the "
          "config's root at [[esp+3Ch]+108h], the model (counted) at [esp+20h], `ret 14h`; no push between")
    check(site.get(0x24B0D2) == "lea eax,[esp+0x20]" and site.get(0x24B0D6) == "push eax"
          and site.get(0x24B0D7) == "push edi" and site.get(0x24B0D8) == "lea ecx,[esp+0x2c]"
          and site.get(0x24B0DC) == "call 0x243a30",
          "the original at 0x24B0D2: `lea eax,[esp+20h]; push eax; push edi`, then the model goes to the weapon "
          "(0x243A30)")
    va, old, offset, opcode = P.WEAPON_CAMO_HOOK
    hooked = image.read(va, len(old))
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and hooked[0] == opcode
          and va + 5 + struct.unpack_from("<i", hooked, 1)[0] == camo_va + offset,
          f"{va:#x}: `{old.hex()}` becomes a call to {camo_va + offset:#x}; the build without the late join keeps it")
    texts = [plain(t) for _, _, t in ndisasm(code, camo_va)]
    calls = sorted({branch_target(t) for t in texts if t.startswith("call ")} - {None})
    outside = [c for c in calls if not camo_va <= c < camo_va + len(code)]
    decals_va = camo_va + P.WEAPON_DECALS_OFFSET
    check(set(outside) == {0x11540, 0x119590, 0x119760, 0x159700, 0x1539D0, 0xE45B0, 0xE4F90, 0xE5B60,
                           0xF6B10, 0xF6C00, 0x420789} | {decals_va + at for at in P.WEAPON_CAMO_DECALS_CALLS}
          and not [t for t in texts if t.startswith("jmp ") and not camo_va <= (branch_target(t) or 0) < camo_va + len(code)],
          "the code calls the game's release, type_to_int, the parameters' constructor, get_all_surfaces, "
          "deallocate, appendf, allocate, query_resources, the config's lookup and key check and the allocation "
          "name, and patch 47's lookup and fill (what is painted), and jumps nowhere outside")
    hits = [hit for hit in aimed_inside(original, va, len(old), 0x82)
            if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
    check(not hits, "no rel32 branch of .text, no rel8 branch near it and no 4-byte value aims inside the site"
          + "".join(f"\n          {a:#x}" for a in hits))
    relocated = [r for kind, r in source.relocations().values() if kind == 3 and r + 4 > va and r < va + len(old)]
    check(not relocated, "no base relocation under the site")


def test_weapon_table(original, image, before):
    """Check 45: a weapon's upgrade window shows it in the air above the
    lobby's workbench (patch 45, WEAPON_TABLE_HOOKS). check_weapon_table_026g.py
    runs the camera's handlers and tick and mt_draw's two sites."""
    source = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    table_va = camo_va + P.WEAPON_TABLE_OFFSET
    code = open(P.weapon_table_bin(), "rb").read()
    stock_texts = {
        0x284020: ["movss xmm0,dword [ebx+0xc8]"],
        0x247576: ["cmp byte [ebp-0x1],0x0", "jnz 0x247592"],
        0x24778F: ["cmp byte [ebx+0xf49c],0x0", "jz 0x2477c2"],
        0x283D81: ["test byte [eax+0xc],0x52", "setnz al"],
        0x284309: ["call 0x2844d0"],
    }
    for va, old, offset, opcode in P.WEAPON_TABLE_HOOKS:
        entry = table_va + offset
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows(source, va, len(old)) == stock_texts[va]
              and rows(image, va, len(old)) == [f"call {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(stock_texts[va])}` in the original and the build without the late join, "
              f"`call {entry:#x}` and {len(old) - 5} nops here")
        hits = [hit for hit in aimed_inside(original, va, len(old), 0x82)
                if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
        relocated = [r for kind, r in source.relocations().values() if kind == 3 and r + 4 > va and r < va + len(old)]
        check(not hits and not relocated, f"{va:#x}: no branch or value aims inside the site, no base relocation "
              "is under it" + "".join(f"\n          {a:#x}" for a in hits))
    tick = {a: t for a, _, t in ndisasm(source.read(0x283FAD, 0xA3), 0x283FAD)}
    check(tick.get(0x283FAD) == "movss dword [esp+0x10],xmm4" and tick.get(0x284028) == "addss xmm0,dword [ebx+0xb4]"
          and tick.get(0x284038) == "movss dword [ebx+0xb4],xmm0" and tick.get(0x284040) == "movss xmm0,dword [ebx+0xcc]"
          and tick.get(0x284048) == "addss xmm0,dword [ebx+0xb8]"
          and rows(source, 0x283FB3, 2) == ["ja 0x284020"] and rows(source, 0x284004, 2) == ["jna 0x284020"]
          and source.read(0x283EF0, 0x10) == bytes.fromhex("558bec83e4f081ece4000000538bd98b"),
          "the lobby camera's tick (ebx = camera+4) keeps the frame's time, at most 0.1 s, at [esp+10h] and comes "
          "to 0x284020 by both ways of the height's step; there it adds the drag's yaw (+CCh of the camera) to "
          "the yaw (+B8h) and its pitch (+D0h) to the pitch (+BCh)")
    ctor = [t for _, _, t in ndisasm(source.read(0x283BC0, 0x13C), 0x283BC0)]
    writes = [a for a, _, t in ndisasm(source.read(0x283B00, 0x1000), 0x283B00)
              if re.search(r"^movss (?:dword )?\[e[a-z]x\+0xa[08]\],", t)]
    look = [t for _, _, t in ndisasm(source.read(0x2842F2, 0x58), 0x2842F2)]
    check("movss dword [eax+0xa0],xmm0" in ctor and "movss dword [eax+0xa8],xmm0" in ctor
          and writes == [0x283C34, 0x283C3C, 0x283FE3, 0x284018, 0x2841DA, 0x28421A]
          and "lea esi,[ebx+0x9c]" in look and "movss xmm3,dword [esi]" in look,
          "the camera's point (+A0h, +A4h, +A8h, the tick's esi = ebx+9Ch) is written by its constructor alone "
          "but the height (+A4h, the tick's [ebx+0A0h]); the tick's other writes at +A0h/+A8h of ebx are the "
          "height and the side angle")
    character = [t for _, _, t in ndisasm(source.read(0x21CCAF, 0x28), 0x21CCAF)]
    check(source.read(0x21CCD2, 5) == b"\xe8" + struct.pack("<i", 0x284730 - 0x21CCD7)
          and "xorps xmm0,xmm0" in character and "movss dword [esp+0x2c],xmm0" in character
          and "movss dword [esp+0x34],xmm0" in character
          and rows(source, 0x21CCE6, 6) == ["mov [ebx+0x694],eax"],
          "the lobby's own character (lobby menu+694h) is made at (0, 0, 0) (0x21CCD2 -> 0x284730): the stock "
          "camera's point is the character's")
    update = {a: t for a, _, t in ndisasm(source.read(0x2849F0, 0x93), 0x2849F0)}
    mt = [t for _, _, t in ndisasm(source.read(0x2474A0, 0x3A0), 0x2474A0)]
    callers = [h for h in branches_into(original, 0x2474A0, 0x2474A1)]
    check(update.get(0x2849F4) == "mov ebx,[esp+0x4c]" and update.get(0x284A78) == "mov ecx,[ebx+0xd2b8]"
          and update.get(0x284A7E) == "call 0x2474a0"
          and source.read(0x2474A0, 10) == bytes.fromhex("558bec81ecec00000053")
          and callers == ["0x1f7dce rel32", "0x284a7e rel32", "0x2474a0 as a value"]
          and rows(source, 0x1F7CC6, 5) == ["mov ecx,0x2474a0"] and original.count(struct.pack("<I", 0x2474A0)) == 1
          and len([t for t in mt if "ebp-0xe8" in t]) == 1 and "lea eax,[ebp-0xe8]" in mt,
          "lobby_character's update (ebx = the character) draws its player (+D2B8h) by `call 0x2474A0` at "
          "0x284A7E (return 0x284A83); player::mt_draw's prologue keeps that ebx at [ebp-0F0h]; mt_draw is called "
          "from there, the match's 0x1F7DCE and the match's draw task (its address at 0x1F7CC6) alone; its "
          "[ebp-0E8h] is the decal pose's transform alone")
    texts = [t for _, _, t in ndisasm(code, table_va)]
    listing = ndisasm(code[:code.index(struct.pack("<4f", -0.5, 0.5, -0.5, 0.0))].rstrip(b"\0"), table_va)   # to its data (`flip_half`)
    targets = {branch_target(t) for _, _, t in listing if branch_target(t) is not None}
    outside = {t for t in targets if not table_va <= t < table_va + len(code)}
    starts = {a for a, _, _ in listing}
    check(outside == {0x2845C0, 0x2844D0, 0xF6C00, 0xF6B10} and all(t in starts for t in targets - outside)
          and [rows(image, table_va + at, 5)[0].startswith("jmp ") for at in P.WEAPON_TABLE_ENTRIES] == [True] * len(P.WEAPON_TABLE_ENTRIES)
          and [t for _, _, t in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[", t)] == ["call [eax+0x24]"]
          and not [t for _, _, t in listing if re.search(r"\[(?:dword )?0x", t)]
          and image.read(table_va, len(code)) == code
          and P.weapon_preview_state(code, table_va)
          == P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_STATE_OFFSET,
          f"the weapon table at {table_va:#x} (.srpw+{P.WEAPON_TABLE_OFFSET:#x}) is weapon_table_026g.section.bin "
          f"({len(code)} bytes), its state at 0x{P.WEAPON_TABLE_STATE_OFFSET:X} of .srpd; it begins with six "
          "`jmp near`, jumps only to the camera's setter and its collision ray outside itself and calls only the "
          "config's key check and lookup (the lobby's spot) and the lobby menu's vtable+24h (is_mouse_over_ui), "
          "every branch inside it lands on an instruction, and no operand is an absolute address")
    scene = [t for _, _, t in ndisasm(source.read(0x221975, 0x2F), 0x221975)]
    check(rows(source, 0x220E01, 6) == ["mov [ebx+0x114],eax"]
          and scene[:5] == ["mov eax,[ebx+0x114]", "mov ecx,[eax+0x17c]", "mov eax,[ecx+0x108]", "mov edi,0x9fb600",
                            "call 0xf6b10"]
          and scene[7:10] == ["mov eax,[eax]", "movq xmm0,[eax]", "mov edx,[eax+0x8]"]
          and source.read(0x9FB600, 7) == b"camera\0",
          "the lobby's spot: on_render_scenes_ready keeps the scene's project at lobby menu+114h (0x220E01) and "
          "reads its config as [[project+17Ch]+108h] -> camera -> position, a float3 whose value's +0 points to "
          "its three floats (0x221975..0x2219A1): what patch 45 reads srp_lobby/weapon by")
    mouse = {a: t for a, _, t in ndisasm(source.read(0x283D79, 0x13), 0x283D79)}
    check(mouse.get(0x283D7F) == "call eax" and mouse.get(0x283D88) == "test al,al"
          and mouse.get(0x283D8A) == "jz 0x283dbe"
          and rows(source, 0x283D95, 6) == ["mov eax,[esi+0xb4]"] and rows(source, 0x283DC7, 6) == ["mov eax,[esi+0xdc]"]
          and rows(source, 0x283E00, 6) == ["mov edx,[esi+0x88]"]
          and rows(source, 0x283D3C, 6) == ["mov ecx,[esi+0x88]"] and rows(source, 0x283D44, 3) == ["mov edx,[eax+0x24]"]
          and source.read(0x225220, 3) == bytes.fromhex("558bec"),
          "the lobby camera's on_mouse_move tests the mouse's buttons (+0Ch) at 0x283D81 and reads only al of it "
          "(every way after loads eax again); its wheel asks the lobby menu (+88h) vtable+24h (is_mouse_over_ui "
          "0x225220, the movie's root.get_mouse_over)")
    texts = lambda va, size: [t for _, _, t in ndisasm(source.read(va, size), va)]
    check(texts(0x243656, 6) == ["mov dword [ebx],0xa04174"] and original.count(struct.pack("<I", 0xA04174)) == 2
          and texts(0x243A9A, 6) == ["mov [ebx+0x1810],eax"]
          and struct.unpack("<I", source.read(0xA04174 + 0x48, 4))[0] == 0x244E10
          and texts(0x15A5AA, 6) == ["mov eax,[edi+0x62b4]"] and texts(0x15A5B0, 8) == ["movq xmm0,[eax+0x108]"]
          and texts(0x15A77F, 5) == ["call 0xf1fd0"] and texts(0x15A7DC, 6) == ["mov edx,[edx+0x140]"]
          and texts(0x15A7EC, 5) == ["call 0xee700"]
          and struct.unpack("<I", source.read(0x9F0F24 + 0x54, 4))[0] == 0x15A4C0
          and texts(0x284B89, 7) == ["cmp dword [ebx+0xd2c4],byte -0x1"]
          and texts(0x284F86, 10) == ["mov dword [ebx+0xd2c4],0xffffffff"]
          and texts(0x284BB2, 7) == ["mov byte [ebx+0xd2c8],0x1"],
          "the weapon's middle: the weapon (vtable 0xA04174, its ctor's alone; +48h weapon::draw) keeps its "
          "skeleton model at +1810h (load_weapon); skeleton_render_model_instance (vtable 0x9F0F24, +54h get_aabb) "
          "takes its box from its model's +108h (0x15A5B0), turns it by rotY(pi) (0x15A77F) and skins with the "
          "model's inverse binds (+140h) times the bones given (0x15A7EC); a lobby character makes its player "
          "while +D2C4h is a query (-1 none, on_player_ready) or +D2C8h is set")


def test_weapon_decals(original, image, before):
    """Check 47: decals on a weapon (patch 47, WEAPON_DECALS_HOOKS).
    check_weapon_decals_026g.py runs the bake and the four editor sites."""
    source = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    camo_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read())
    decals_va = camo_va + P.WEAPON_DECALS_OFFSET
    code = open(P.weapon_decals_bin(), "rb").read()
    # A packed SSE operation on a 16-byte memory operand faults (#GP) unless the
    # address is 16-byte aligned; Unicorn does not enforce it (the game crashed
    # on patch 47's andps of a misaligned constant, 2026-10-21). Every such
    # operand of .srpw's code at a fixed address must be aligned.
    packed = re.compile(r"\b(andn?ps|x?orps|addps|subps|mulps|divps|minps|maxps|cmpps|sqrtps|rsqrtps|rcpps|"
                        r"unpck[lh]ps|shufps|movaps|movdqa|pand|por|pxor|padd[bwdq]|psub[bwdq]|pcmpeq[bwd]|"
                        r"andpd|xorpd|movapd)\b.*\[(?:dword )?(0x[0-9a-f]+)\]")
    misaligned = []
    for bin_path, base_va in ((P.weapon_camo_bin(), camo_va),
                              (P.weapon_stash_bin(), camo_va + P.WEAPON_STASH_OFFSET),
                              (P.weapon_table_bin(), camo_va + P.WEAPON_TABLE_OFFSET),
                              (P.weapon_decals_bin(), decals_va),
                              (P.upgrade_preview_bin(), camo_va + P.UPGRADE_PREVIEW_OFFSET)):
        for address, _, text in ndisasm(open(bin_path, "rb").read(), base_va):
            m = packed.search(text)
            if m and int(m.group(2), 16) % 16:
                misaligned.append(f"{address:#x} {text}")
    check(not misaligned, "every packed SSE operand of .srpw's code at a fixed address is 16-byte aligned"
          + "".join("; " + m for m in misaligned))
    stock_texts = {
        0x265999: ["call 0x264960"],
        0x2659B2: ["call 0x263d00"],
        0x2641A0: ["push ebp", "mov ebp,esp", "and esp,byte -0x8"],
        0x263938: ["call 0x264cf0"],
        0x265C20: ["push ebp", "mov ebp,esp", "and esp,byte -0x8"],
    }
    for va, old, offset, opcode in P.WEAPON_DECALS_HOOKS:
        entry = decals_va + offset
        kind = "call" if opcode == 0xE8 else "jmp"
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows(source, va, len(old)) == stock_texts[va]
              and rows(image, va, len(old)) == [f"{kind} {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(stock_texts[va])}` in the original and the build without the late join, "
              f"`{kind} {entry:#x}`" + (f" and {len(old) - 5} nop" if len(old) > 5 else "") + " here")
        hits = [hit for hit in aimed_inside(original, va, len(old), 0x82)
                if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
        relocated = [r for kind, r in source.relocations().values() if kind == 3 and r + 4 > va and r < va + len(old)]
        check(not hits and not relocated, f"{va:#x}: no branch or value aims inside the site, no base relocation "
              "is under it" + "".join(f"\n          {a:#x}" for a in hits))
    preview = {a: t for a, _, t in ndisasm(source.read(0x265980, 0x3C), 0x265980)}
    check(preview.get(0x265986) == "mov esi,eax" and preview.get(0x265988) == "push byte +0x20"
          and preview.get(0x2659A6) == "call 0x264a10" and preview.get(0x2659AB) == "push edx"
          and rows(source, 0x284A8C, 7) == ["mov eax,ebx", "call 0x265980"]
          and rows(source, 0x284A83, 9) == ["cmp byte [ebx+0xd514],0x0", "jz 0x284a93"],
          "lobby_character's update calls the decal preview 0x265980 (eax = esi = the character) while +D514h "
          "is set; it picks with the collision mask 20h, encodes (0x264A10) and unpacks the u64 at 0x2659B2")
    bake_calls = sorted(branches_into(original, 0x2641A0, 0x2641A1))
    check(bake_calls == ["0x263b46 rel32", "0x264f63 rel32", "0x265c4c rel32"],
          "the editor's bake (0x2641A0) is called by the cancel (0x263B46), the start's callback (0x264F63) and "
          "the click (0x265C4C) alone" + ("" if bake_calls == ["0x263b46 rel32", "0x264f63 rel32", "0x265c4c rel32"]
                                          else f": {bake_calls}"))
    key = {a: t for a, _, t in ndisasm(source.read(0x265C20, 0x3C), 0x265C20)}
    vtable = struct.unpack("<4I", source.read(0xA09F6C, 16))
    check(key.get(0x265C29) == "cmp dword [ebp+0xc],0x151" and key.get(0x265C32) == "mov esi,ecx"
          and key.get(0x265C36) == "cmp dword [ebp+0x10],byte +0x1" and key.get(0x265C4C) == "call 0x2641a0"
          and source.read(0x265C58, 3) == bytes.fromhex("c20c00") and vtable[2] == 0x265C20
          and sorted(h for h in branches_into(original, 0x265C20, 0x265C21) if not h.endswith("value")) == [],
          "the character's mouse key handler 0x265C20 (its input vtable 0xA09F6C slot 2, called through it alone; "
          "ecx = the character, `ret 0Ch`) bakes the edit's place on the left button's (151h) release (kind 1)")
    start = rows(source, 0x263936, 7)
    check(start == ["mov esi,ebp", "call 0x264cf0"] and source.read(0x263942, 3) == bytes.fromhex("c20800"),
          "the editor's start (0x2636B0) ends `mov esi,ebp / call 0x264CF0` (the character) and `ret 8`")
    texts = [t for _, _, t in ndisasm(code, decals_va)]
    listing = ndisasm(code[:code.index(struct.pack("<4I", 0x20080000, 0x8A30CE00, 0x20080200, 0x8A30CE00))],
                      decals_va)
    targets = {branch_target(t) for _, _, t in listing if branch_target(t) is not None}
    outside = {t for t in targets if not decals_va <= t < decals_va + len(code)}
    starts = {a for a, _, _ in listing}
    check(outside == {0xF6C00, 0xF6B10, 0x127C0, 0xE45B0, 0x263D00, 0x264A10, 0x264960, 0x26D130, 0x263950,
                      0x511600, 0x277AA0, 0x264CF0, 0x2641A6, 0x265C26}
          and all(t in starts for t in targets - outside)
          and [rows(image, decals_va + at, 5)[0].startswith("jmp ") for at in P.WEAPON_DECALS_ENTRIES]
          == [True] * len(P.WEAPON_DECALS_ENTRIES)
          and not [t for _, _, t in listing if re.search(r"(?:call|jmp) (?:dword )?\[(?!eax\+0x3c\])", t)]
          and image.read(decals_va, len(code)) == code,
          f"patch 47 at {decals_va:#x} (.srpw+{P.WEAPON_DECALS_OFFSET:#x}) is weapon_decals_026g.section.bin "
          f"({len(code)} bytes); it begins with seven `jmp near`, calls only the config's key check and lookup, "
          "assign, appendf, the decal's unpacking and packing, the collision pick, the mouse's ray, the accept, "
          "the movie call, find_item_by_id and the start's query, jumps only to the bake's and the key handler's "
          "bodies, every branch "
          "inside it lands on an instruction, and its one indirect call is the inventory owner's (vtable+3Ch)")


def test_upgrade_preview(original, image, before):
    """Check 48: a weapon's upgrade window shows an upgrade dropped on the
    weapon at once (patch 48, UPGRADE_PREVIEW_HOOKS). check_upgrade_preview_026g.py
    runs the sites with the game's setup_profile, profile copy and config lookups."""
    source = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    late_join = open(P.late_join_bin(), "rb").read()
    code_va = P.weapon_camo_va(late_join) + P.UPGRADE_PREVIEW_OFFSET
    stash_find = P.weapon_camo_va(late_join) + P.WEAPON_STASH_OFFSET + P.WEAPON_STASH_FIND
    state = P.late_join_state_va(late_join) + P.UPGRADE_PREVIEW_STATE_OFFSET
    preview_state = P.late_join_state_va(late_join) + P.WEAPON_PREVIEW_STATE_OFFSET
    code = open(P.upgrade_preview_bin(), "rb").read()
    stock_texts = {
        0x29A9B2: ["mov edx,[esi+0x4620]"],
        0x29AACF: ["mov ecx,[esi+0x4620]"],
        0x270313: ["call 0x270320"],
        0x284B89: ["cmp dword [ebx+0xd2c4],byte -0x1"],
        0x284F7F: ["cmp byte [ebx+0xd2c9],0x0"],
        0x284B7C: ["cmp byte [ebx+0xd2c8],0x0"],
    }
    for va, old, offset, opcode in P.UPGRADE_PREVIEW_HOOKS:
        entry = code_va + offset
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows(source, va, len(old)) == stock_texts[va]
              and rows(image, va, len(old)) == [f"call {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(stock_texts[va])}` in the original and the build without the late join, "
              f"`call {entry:#x}` and {len(old) - 5} nops here")
        hits = [hit for hit in aimed_inside(original, va, len(old), 0x82)
                if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
        relocated = [r for kind, r in source.relocations().values() if kind == 3 and r + 4 > va and r < va + len(old)]
        check(not hits and not relocated, f"{va:#x}: no branch or value aims inside the site, no base relocation "
              "is under it" + "".join(f"\n          {a:#x}" for a in hits))
    relocate = {a: t for a, _, t in ndisasm(source.read(0x29A9AE, 0x15), 0x29A9AE)}
    drop = {a: t for a, _, t in ndisasm(source.read(0x29AAB2, 0x2A), 0x29AAB2)}
    check(rows(source, 0x29A8C6, 4) == ["mov [esp+0x10],ecx"] and rows(source, 0x29A9EA, 4) == ["mov [esp+0x10],ecx"]
          and rows(source, 0x29A8BE, 3) == ["mov esi,[eax+0x8]"] and rows(source, 0x29A984, 3) == ["mov al,[esi+0x38]"]
          and rows(source, 0x29A987, 3) == ["mov edi,[esi+0x8]"] and rows(source, 0x29A98A, 3) == ["mov ebx,[esi+0x20]"]
          and rows(source, 0x29A9A3, 6) == ["mov esi,[eax+0x6a0]"]
          and relocate.get(0x29A9AE) == "mov ecx,[esp+0x14]" and relocate.get(0x29A9BE) == "call 0x2707c0"
          and rows(source, 0x29A9D9, 6) == ["mov eax,[ebp+0x8]", "mov eax,[eax+0x8]"]
          and rows(source, 0x29A9E7, 3) == ["mov edi,[eax+0x20]"]
          and drop.get(0x29AAB6) == "mov esi,[eax+0x6a0]" and drop.get(0x29AACD) == "call eax"
          and drop.get(0x29AAD7) == "call 0x2708c0",
          "relocate_upgrade_item's handler (0x29A8B0) keeps its object, the lobby menu, at [esp+10h], and at "
          "0x29A9B2 holds the host in edi, the upgrade in ebx, the slot in ecx and the account's profile in "
          "esi before it sends (0x2707C0); drop_upgrade_item's (0x29A9D0) the upgrade in edi and the profile "
          "in esi at 0x29AACF, the host at [[ebp+8]+8]+8, before patch 30's call (0x29AAD5)")
    reader = [t for _, _, t in ndisasm(source.read(0x27030B, 0x14), 0x27030B)]
    check(reader[:4] == ["mov esi,ebx", "mov [ebx+0x1ea96c],ecx", "call 0x270320", "pop edi"]
          and branches_into(original, 0x270260, 0x270261) == ["0x274b3f rel32"],
          "the 0x87 reader (0x270260, called from 0x274B3F alone) ends with `call 0x270320` (esi = the lobby "
          "client), once for each account the server sends")
    setup = {a: t for a, _, t in ndisasm(source.read(0x284B10, 0x200), 0x284B10)}
    check(setup.get(0x284B5B) == "jnz 0x284b89" and setup.get(0x284B75) == "jnz 0x284b89"
          and setup.get(0x284B7A) == "jnz 0x284b89" and setup.get(0x284B83) == "jz near 0x284cf8"
          and setup.get(0x284B93) == "jz 0x284bc2" and setup.get(0x284BDA) == "call 0x284d10"
          and setup.get(0x284BB2) == "mov byte [ebx+0xd2c8],0x1" and setup.get(0x284CB7) == "mov [ebx+0xd2c4],eax"
          and setup.get(0x284CF8) == "pop edi" and setup.get(0x284CFE) == "ret 0x8"
          and rows(source, 0x284F86, 10) == ["mov dword [ebx+0xd2c4],0xffffffff"]
          and rows(source, 0x284F90, 6) == ["jnz near 0x2850ca"]
          and setup.get(0x284B54) == "cmp [esi+ebx+0x76c],eax" and setup.get(0x284B6E) == "cmp esi,[edi+edx*4+0x45fc]",
          "setup_profile comes to 0x284B89 by its early return's three jumps (kit id, revision, the last id) "
          "and a waiting call (+D2C8h); with no query (+D2C4h -1) it copies the profile into the next copy "
          "(0x284BDA) and queries, else it copies it into the one after and waits (+D2C8h 1); its end is at "
          "0x284CF8; on_player_ready's comparison at 0x284F7F is read by the `jnz` after a `mov`")
    listing = ndisasm(code[:code.index(b"srp_scope_fit\0")], code_va)
    targets = {branch_target(t) for _, _, t in listing if branch_target(t) is not None}
    outside = {t for t in targets if not code_va <= t < code_va + len(code)}
    starts = {a for a, _, _ in listing}
    check(outside == {0x284B10, 0x284CF8, 0x270320, 0x277AA0, 0x1F0090, 0x0F6C00, 0x0F6B10, 0x284D10, 0x2F73F0,
                      0x0E4F90, 0x420789, 0x111ED0, 0x111860, stash_find}
          and all(t in starts for t in targets - outside)
          and all(code_va + at in starts and image.read(code_va + at, 1) == b"\xE9" for at in P.UPGRADE_PREVIEW_ENTRIES)
          and [t for _, _, t in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[", t)]
          == [f"call [dword {code_va + P.UPGRADE_PREVIEW_KIT_APPLY:#x}]"]
          and image.read(code_va, len(code)) == P.with_kit_apply(code, P.arena())
          and image.read(code_va + P.UPGRADE_PREVIEW_KIT_APPLY, 4)
          == struct.pack("<I", P.arena_target(P.arena(), P.WEAPON_KIT_PREVIEW_APPLY))
          and P.upgrade_preview_targets(code, code_va) == (stash_find, state, preview_state),
          f"the upgrade preview at {code_va:#x} (.srpw+{P.UPGRADE_PREVIEW_OFFSET:#x}) is "
          f"upgrade_preview_026g.section.bin ({len(code)} bytes), its state at 0x{P.UPGRADE_PREVIEW_STATE_OFFSET:X} "
          "of .srpd, patch 32's read; it begins with seven `jmp near`, calls only setup_profile, the inventory's "
          "find, item_by_id, the config's has_key and child, the profile's ctor and copy, the allocator, the "
          "log and the stash's find_kit_or_show, jumps only to setup_profile's end and the 0x87 reader's last "
          "call outside itself, and every branch inside it lands on an instruction; its one indirect call is "
          f"apply's through its kit_apply dword (+{P.UPGRADE_PREVIEW_KIT_APPLY:#x}), which the build filled with "
          "patch 68's weapon_kit_preview_apply")


def test_research(original, image, before):
    """Check 53: Research, mode 5 (patch 53, RESEARCH_HOOKS; link_026g.py links it,
    check_research_026g.py runs the hooks, the cook and the rule on made-up
    games). Here: each site is the original's instructions before and the
    hook to its symbol after, with no relocation under it and no branch into
    it; the entries are jumps; the code's calls outside it are the game's
    functions research_arena names. Each slot of ours
    popping what tdm's in the same place pops is test_call_shapes'
    (INSTALLED_VTABLES)."""
    source = Image(original)
    plain = lambda text: text.replace("[dword 0x", "[0x").replace("dword [", "[")
    linked = P.arena()
    code = P.linker.block_bytes(linked, "research.code")
    research_va = linked.blocks["research.code"].va
    check(image.read(research_va, len(code)) == code and linked.blocks["research.state"].kind == "state"
          and not any(image.read(linked.blocks["research.state"].va, linked.blocks["research.state"].size)),
          f"the arena's block research.code at {research_va:#x} ({len(code)} bytes) is the link's, research.state "
          f"at {linked.blocks['research.state'].va:#x} zeros")
    originals = {0x1E9B4C: ["mov eax,ebx", "call 0x1ec170"], 0x23B3DE: ["cmp dword [edi+0x20],0x4", "jnz 0x23b3fc"],
                 0x23B351: ["cmp eax,0x4", "jnz 0x23b36a"], 0x23B207: ["call 0x23aa10"]}
    loose = lambda text: text.replace("byte +", "")     # ndisasm 2.16 writes `byte +0x4`, 3.x `0x4`
    for va, old, symbol, opcode in P.RESEARCH_HOOKS:
        rows = ndisasm(source.read(va, len(old)), va)
        hooked = ndisasm(image.read(va, len(old)), va)
        word = "call" if opcode == 0xE8 else "jmp"
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
              and [loose(text) for _, _, text in rows] == originals[va]
              and hooked[0][2].startswith(word) and branch_target(hooked[0][2]) == linked.symbols[symbol]
              and research_va <= linked.symbols[symbol] < research_va + 20
              and all(text == "nop" for _, _, text in hooked[1:]),
              f"{va:#x}: `{'; '.join(originals[va])}` becomes {hooked[0][2]} ({symbol}) + {len(hooked) - 1} nop; "
              "the build without the late join keeps it")
        hits = [hit for hit in aimed_inside(original, va, len(old), 0x82)
                if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
        relocated = [r for kind, r in source.relocations().values() if kind == 3 and r + 4 > va and r < va + len(old)]
        check(not hits and not relocated, f"{va:#x}: no branch aims inside the site, no base relocation under it")
    entries = ndisasm(image.read(research_va, 20), research_va)
    check([text.split()[0] for _, _, text in entries] == ["jmp"] * 4,
          "the section starts with its four entries, `jmp near` each: register, hud, welcome, icons")
    stubs = code[:code.index(b"st_start_match_welcome_message_5")]
    texts = [plain(t) for _, _, t in ndisasm(stubs, research_va)]
    targets = {int(t.split()[-1], 16) for t in texts if t.startswith(("call ", "j")) and t.split()[-1].startswith("0x")}
    outside = {c for c in targets if not research_va <= c < research_va + len(code)}
    check(outside == {0x1EC170, 0x23B35B, 0x23B36A, 0x23AA10} and "mov [esp],0x23b3fc" in texts
          and "mov ecx,0xa03aac" in texts,
          "the stubs leave to the constructor's replaced call, update_ui's message (TDM's key or mode 5's) and its "
          f"way on without one, and the hud's to update_ui past Team Deathmatch's part ({sorted(hex(c) for c in outside)}); "
          "the Rust code calls the game through the engine library")
    # The Rust code reaches the game's functions through the engine-function
    # library (check_engine_026g.py proves each thunk): a call or a pointer
    # to each one's entry, and none of their VAs in the code.
    import engine_026g
    entries = {f["symbol"]: (linked.symbols[f["symbol"]], f["va"]) for f in engine_026g.functions()}
    c_listing = [plain(t) for _, _, t in ndisasm(code[P.RESEARCH_ASM_SIZE:], research_va + P.RESEARCH_ASM_SIZE)]
    for symbol in ("engine_register_cook", "engine_malloc", "engine_match_rule_base", "engine_cfg_child",
                   "engine_cfg_has", "engine_write_u8", "engine_write_u16", "engine_write_u32",
                   "engine_statistics_notify", "engine_movie_invoke", "engine_show_message", "engine_point_to_screen"):
        entry, real = entries[symbol]
        named = any(t == f"call {entry:#x}" or t.endswith(f",{entry:#x}") for t in c_listing)
        check(named and struct.pack("<I", real) not in code[P.RESEARCH_ASM_SIZE:],
              f"the Rust code calls {symbol} ({entry:#x}, the library's thunk of {real:#x}), not {real:#x} itself")
    # Each slot of the rule's, the cook's and the link resolver's vtables
    # that is ours pops what the game's counterpart pops: test_call_shapes,
    # from INSTALLED_VTABLES (the same check for every patch's vtables).
    read32 = lambda img, va: struct.unpack("<I", img.read(va, 4))[0]
    # The cook's RTTI: its own complete object locator (research_rule_cook, a
    # translate_query_cook as tdm_rule_core_cook is); the rule's the base's.
    def type_name(img, td):
        raw = img.read(td + 8, 64)
        return raw[:raw.index(b"\0")].decode()

    def hierarchy(img, locator):
        signature, offset, _, td, chd = struct.unpack("<5I", img.read(locator, 20))
        _, attributes, count, array = struct.unpack("<4I", img.read(chd, 16))
        bases = [read32(img, array + 4 * i) for i in range(count)]
        return (signature, offset, read32(img, td), type_name(img, td), attributes,
                [(type_name(img, read32(img, b)),) + struct.unpack("<6I", img.read(b + 4, 24)) for b in bases])
    cook_locator = read32(image, linked.symbols["rn_cook_vtable"])
    ours = hierarchy(image, cook_locator)
    core = hierarchy(source, 0xA524C0)                    # tdm_rule_core_cook's locator (vtable 0xA12C14)
    check(ours[:2] == (0, 0) and ours[2] == read32(source, 0xACCC38) and ours[3] == ".?AVresearch_rule_cook@srp@@"
          and ours[4] == core[4] and ours[5][1:] == core[5][1:] and ours[5][0][1:6] == core[5][0][1:6]
          and ours[5][0][0] == ours[3] and research_va <= cook_locator < research_va + len(code),
          f"the cook's RTTI locator ({cook_locator:#x}) is its own: {ours[3]}, a translate_query_cook as "
          f"tdm_rule_core_cook is ({[b[0] for b in ours[5]]})")
    check(read32(image, linked.symbols["rn_rule_vtable"]) == 0xA4C970
          and hierarchy(source, 0xA4C970)[3] == ".?AVgame_match_rule_base@survarium@@",
          "the rule's vtable has game_match_rule_base's locator (0xA4C970)")


def instruction(image, va):
    """(length, text) of the instruction at `va` (ndisasm, a window decoded
    at a time and cached on the image), or None."""
    cache = image.__dict__.setdefault("instructions", {})
    if va not in cache:
        try:
            window = image.read(va, 0x200)
        except ValueError:
            return None
        rows = ndisasm(window, va)
        # an instruction's length is the next one's distance: ndisasm prints
        # the bytes of a long one (10 or more) on a second, `-` line, which
        # ndisasm() leaves out
        for (at, code_hex, text), following in zip(rows, rows[1:] + [None]):
            if following is None or following[0] > va + len(window) - 16:
                break
            cache.setdefault(at, (following[0] - at, text))
        if va not in cache and rows:
            cache[va] = (len(rows[0][1]) // 2, rows[0][2])
    return cache.get(va)


def function_pops(image, entry, limit=20000):
    """The bytes the function at `entry` pops: every `ret n` reachable from
    its entry, its branches followed (a body split by the compiler, a tail
    jump into the game too), its calls stepped over. ({n, ...}, [what could
    not be followed])."""
    todo, seen, found, lost = [entry], set(), set(), []
    while todo and len(seen) < limit:
        va = todo.pop()
        while va not in seen:
            seen.add(va)
            got = instruction(image, va)
            if got is None:
                lost.append(f"{va:#x}: outside the image")
                break
            length, text = got
            words = text.split()
            while words and words[0] in ("rep", "repe", "repz", "repne", "repnz", "bnd"):
                words = words[1:]                          # `rep ret`, `bnd ret`: a ret
            op = words[0] if words else ""
            if op in ("ret", "retn"):
                found.add(int(words[1], 16) if len(words) > 1 else 0)
                break
            if op in ("int3", "ud2", "hlt"):
                break
            target = branch_target(text)
            if op == "jmp":
                if target is None:
                    lost.append(f"{va:#x}: {text}")
                    break
                va = target
                continue
            if (op.startswith("j") or op.startswith("loop")) and target is not None:
                todo.append(target)
            va += length
    if todo or len(seen) >= limit:
        lost.append(f"{entry:#x}: more than {limit} instructions, not all followed")
    return found, lost


def popped(image, start, end):
    """The bytes the function at `start` pops: its first `ret`'s immediate
    (None without a `ret` before `end`)."""
    for _, _, text in ndisasm(image.read(start, end - start), start):
        if text == "ret":
            return 0
        if text.startswith("ret "):
            return int(text.split()[1], 16)
    return None


def aimed_inside(original, va, size, near):
    """Every rel32 branch of .text, rel8 branch byte within `near` bytes
    and 4-byte value of the file that aims inside bytes 1.. of [va, va+size)."""
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    inside = lambda target: va < target < va + size
    found = []
    for at in range(len(text) - 6):
        op, here = text[at], P.TEXT_VA + at
        if op in (0xE8, 0xE9) and inside(here + 5 + struct.unpack_from("<i", text, at + 1)[0]):
            found.append(here)
        elif op == 0x0F and 0x80 <= text[at + 1] <= 0x8F and inside(here + 6 + struct.unpack_from("<i", text, at + 2)[0]):
            found.append(here)
        elif (op == 0xEB or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3) and va - near <= here < va + size \
                and inside(here + 2 + struct.unpack_from("<b", text, at + 1)[0]):
            found.append(here)
    return found + [va + i for i in range(1, size) if original.find(struct.pack("<I", va + i)) != -1]


def starts_instruction(source, at):
    """Whether `at` starts an instruction of the function around it, read
    from the function's start (after the int3 before it)."""
    start = at
    while source.read(start - 1, 1) != b"\xcc" or start % 16:
        start -= 1
    return at in {address for address, _, _ in ndisasm(source.read(start, at - start + 16), start)}


def test_pregame_progress(original, image, before):
    """Check 18: the hud draws the team points while a match counts down."""
    source = Image(original)
    va, old, new = P.PREGAME_PROGRESS
    target = P.PREGAME_PROGRESS_TARGET
    rows = lambda blob, at, size: ndisasm(blob.read(at, size), at)
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new
          and [text for _, _, text in rows(image, va, len(new))] == [f"jmp 0x{target:x}"]
          and [text for _, _, text in rows(source, va, len(old))] == ["jmp 0x23b421"],
          f"{va:#x}: the countdown's `jmp 0x23b421` is `jmp 0x{target:x}`; the original and the build without the "
          "late join keep it")
    # update_ui from the test of the countdown flag to the players' rows.
    was = rows(source, 0x23B307, 0x23B421 - 0x23B307)
    texts = [text for _, _, text in was]
    at = {address: i for i, (address, _, _) in enumerate(was)}
    countdown = texts[at[0x23B307]:at[va] + 1]
    play = texts[at[0x23B371]:at[target]]
    check(countdown == ["cmp byte [edi+0x15c],0x0", "jz 0x23b32d", "mov al,0x1", "mov ecx,edi", "call 0x236ad0",
                        "mov eax,[edi+0xb0]", "lea ecx,[edi+0x30]", "push edi", "call 0x236b50", "jmp 0x23b421"]
          and play == ["xor al,al", "mov ecx,edi", "call 0x236ad0", "mov eax,[edi+0x28]", "push edi", "call 0x236a00"]
          and target in at,
          f"in the original the countdown shows the pregame (0x236AD0 with 1, 0x236B50) and jumps to the rows; play "
          f"shows the match time (0x236AD0 with 0, 0x236A00) and goes on at {target:#x}, an instruction of its own")
    after = texts[at[target]:]
    calls = [text for text in after if text.startswith("call")]
    leaving = [text for text in after if branch_target(text) is not None and not text.startswith("call")
               and not target <= branch_target(text) <= 0x23B421]
    check(calls == ["call 0x2364c0", "call 0x236610", "call 0x236c30", "call 0xe0210"] and not leaving
          and after[0] == "mov eax,[edi+0x18]",
          f"from {target:#x} to the rows the code draws the team points (0x2364C0), in Team Deathmatch the last "
          "players (0x236610) and the respawn time (0x236C30), lets go of the player it took (0xE0210 at the last "
          "count) and leaves to nowhere else")
    ret = lambda function: next(text for _, _, text in ndisasm(source.read(function, 0x400), function)
                                if text.startswith("ret"))
    pushed = lambda function: {source.read(int(text.split()[-1], 16), 32).split(b"\0")[0]
                               for _, _, text in ndisasm(source.read(function, 0x200), function)
                               if text.startswith("push dword 0xa")}
    rets = {function: ret(function) for function in (0x236AD0, 0x236B50, 0x236A00, 0x2375C0)}
    check(rets == {0x236AD0: "ret", 0x236B50: "ret 0x4", 0x236A00: "ret 0x4", 0x2375C0: "ret 0x10"}
          and b"root.set_game_progress" in pushed(0x2364C0) and b"root.set_last_players" in pushed(0x236610)
          and b"root.set_respawn_time" in pushed(0x236C30),
          "each call before the jump and before the target takes its arguments off the stack (both paths reach the "
          "target as deep), and the three after it are the hud movie's set_game_progress, set_last_players and "
          "set_respawn_time")
    found = aimed_inside(original, va, len(old), 0x82)
    check(not found, "no rel32 branch of .text, no rel8 branch byte near it and no 4-byte value aims inside the jump"
          + "".join(f"\n          {a:#x}" for a in found))


# The vtables whose slot 0 is the camera's projection, by RTTI.
FOV_VTABLES = {0xA0406C: b".?AVplayer_input_handler@survarium@@", 0xA04084: b".?AVgame_camera@survarium@@",
               0xA047F0: b".?AVfree_fly_camera@survarium@@", 0xA0482C: b".?AVdemo_camera@survarium@@",
               0xA09ED8: b".?AVlobby_camera@survarium@@"}


def test_fov(original, image, before):
    """Check 23: the field-of-view setting (patch 22)."""
    source = Image(original)
    pack = lambda value: struct.pack("<I", value)

    def refs(value):
        """The addresses of every 4-byte copy of `value` in the file's sections."""
        found, at = [], original.find(pack(value))
        while at != -1:
            found += [P.IMAGE_BASE + rva + at - raw for _, rva, _, raw, rsize, _ in source.sections
                      if raw <= at < raw + rsize]
            at = original.find(pack(value), at + 1)
        return sorted(found)
    for va, old, new in P.FOV_PATCHES:
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(new)) == new,
              f"{va:#x}: {len(old)} bytes rewritten; the original and the build without the late join keep them")
    string = lambda blob, at: blob.read(at, 64).split(b"\0")[0]
    name = struct.unpack("<I", P.FOV_OPTION_NAME[2][1:])[0]
    check(string(source, 0xA0659C) == b"fov" and refs(0xA0659C) == [P.FOV_OPTION_NAME[0] + 1]
          and string(image, name) == b"uro_fov" and source.read(0xAA6510 + 0x10, 4) == pack(name)
          and refs(name) == [0xAA6510 + 0x10] and refs(P.FOV_VALUE) == [0x85AE24 + 6]
          and source.read(0x85AE24, 6) == bytes.fromhex("c70550 65aa00".replace(" ", ""))
          and struct.unpack("<f", source.read(P.FOV_VALUE, 4))[0] == 65.0,
          "option 7 names \"uro_fov\" for \"fov\" (pointed to from there alone); \"uro_fov\" names the console variable "
          f"at 0xAA6510, whose value ({P.FOV_VALUE:#x}, 65.0) only its registration pointed to")
    va, old, offset, _ = P.FOV_PROJECTION_HOOK
    entry = P.LATE_JOIN_VA + offset
    hook = [text for _, _, text in ndisasm(image.read(va, len(old)), va)]
    rest = len(P.FOV_PROJECTION_BODY)
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
          and branch_target(hook[0]) == entry and hook[1:] == ["nop"]
          and image.read(va + len(old), rest - len(old)) == source.read(va + len(old), rest - len(old))
          and source.read(va, rest) == P.FOV_PROJECTION_BODY and refs(0xA2D9EC) == [va + 0x3A],
          f"the projection's first instruction jumps to fov_projection ({entry:#x}); the rest of its body, which "
          "alone reads the 65-degree constant 0xA2D9EC, is the original's; the build without the late join keeps it")
    rows = ndisasm(image.read(entry, 0xC0), entry)
    end_at = next(i for i, (_, _, text) in enumerate(rows) if text.startswith("ret"))
    texts = [text for _, _, text in rows[:end_at + 1]]
    data = image.read(rows[end_at][0] + len(rows[end_at][1]) // 2, 20)
    returns = lambda text: text.replace("ret word ", "ret ")  # ndisasm 3 spells `ret 0x8` so
    was = [text for _, _, text in ndisasm(source.read(va, rest), va)]
    calls = lambda texts: [branch_target(text) for text in texts if text.startswith("call")]
    check(calls(texts) == calls(was) == [0xEEE30] and returns(texts[-1]) == "ret 0x8"
          and f"fld dword [0x{P.FOV_VALUE:x}]" in [t.replace("[dword 0x", "[0x") for t in texts]  # ndisasm 2.x spells [dword 0x..]
          and "fcmovb st1" in texts
          and not any("xmm" in text and "xmm0" not in text for text in texts)
          and data == struct.pack("<5f", 0.25, 1.3333334, 65.0, 0.017453292, 0.008726646),
          "fov_projection reads uro_fov, takes max(0, (z - 0.25) / 0.75) (0.25, 4/3, 65, pi/180 and pi/360 after its code), "
          "calls the matrix builder 0xEEE30 as the original, writes no XMM register but xmm0 and returns with `ret 0x8`")
    check(all(source.read(vtable, 4) == pack(P.FOV_PROJECTION_VA)
              and string(source, struct.unpack("<I", source.read(struct.unpack("<I", source.read(vtable - 4, 4))[0] + 12,
                                                                  4))[0] + 8) == klass
              for vtable, klass in FOV_VTABLES.items())
          and refs(P.FOV_PROJECTION_VA) == sorted(FOV_VTABLES),
          "the projection is slot 0 of the vtables of " + ", ".join(k[4:].split(b"@")[0].decode()
                                                                   for k in FOV_VTABLES.values())
          + " and pointed to from nowhere else")
    least = struct.unpack("<I", P.FOV_MINIMUM[1][7:11])[0]
    check(image.read(P.FOV_MINIMUM[0] + 2, 8) == struct.pack("<If", least, P.FOV_MIN) and least == 0xAA6510 + 0x44
          and source.read(0x85AE11, 8) == bytes.fromhex("f30f1005d010a700")
          and struct.unpack("<f", source.read(0xA710D0, 4))[0] == 100.0,
          f"uro_fov's registration stores {P.FOV_MIN:g} as its least (+44h); its most stays the float 100 at 0xA710D0")

    # A rel8 byte is a branch only where the function's own instructions
    # have one; the others are bytes of other instructions.
    found = [hit for va, old, _ in P.FOV_PATCHES for hit in aimed_inside(original, va, len(old), 0x82)
             if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
    check(not found, "no rel32 branch of .text, no rel8 branch near them (read as the functions' instructions) and "
          "no 4-byte value aims inside the two sites rewritten in place" + "".join(f"\n          {a:#x}" for a in found))


def test_fov_lod(original, image, before):
    """Check 40: a model's LOD and screen factor at a wider field of view
    are the original's (patch 40)."""
    source = Image(original)
    k = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.FOV_LOD_STATE_OFFSET
    pack = lambda value: struct.pack("<I", value)
    plain = lambda text: text.replace("[dword 0x", "[0x").replace("dword [", "[")
    # The two constants where the original reads them: select_lod's half of
    # the larger extent, just before param2 (+8) is compared; the screen
    # factor's 100 after pow and before the store at +58h.
    lod = {a: plain(t) for a, _, t in ndisasm(source.read(0x16F280, 0x40), 0x16F280)}
    factor = {a: plain(t) for a, _, t in ndisasm(source.read(0x15909A, 0x30), 0x15909A)}
    check(lod.get(0x16F2A4) == "mulss xmm0,[0x927a44]" and lod.get(0x16F2AC) == "movss xmm1,[eax+0x8]"
          and lod.get(0x16F2B1) == "comiss xmm1,xmm0"
          and factor.get(0x1590AA) == "call 0x3a9b0" and factor.get(0x1590AF) == "fmul [0xa710d0]"
          and factor.get(0x1590BF) == "fstp [esi+0x58]"
          and struct.unpack("<f", source.read(0x927A44, 4))[0] == 0.5
          and struct.unpack("<f", source.read(0xA710D0, 4))[0] == 100.0,
          "the original: select_lod's `mulss xmm0,[0x927a44]` (0.5) before param2 is compared (0x16f2b1), "
          "the screen factor's `fmul dword [0xa710d0]` (100) after pow (0x3a9b0) and before the store at +58h")
    for va, old, offset, _ in P.FOV_LOD_HOOKS:
        stub = image.read(P.LATE_JOIN_VA + offset, 32)
        times = b"\xf3\x0f\x59\x05" if old[0] == 0xF3 else b"\xd8\x0d"
        expected = old + b"\x83\x3d" + pack(k) + b"\x00" + bytes([0x74, len(times) + 4]) + times + pack(k) + b"\xc3"
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and stub.startswith(expected)
              and set(stub[len(expected):]) <= {0xCC},
              f"{va:#x}: calls {P.LATE_JOIN_VA + offset:#x}: `{old.hex()}`, then by k ({k:#x}) when it is not 0, ret; "
              "the original and the build without the late join keep the instruction")
    body = image.read(P.SECTION_VA, P.late_join_state_va(open(P.late_join_bin(), "rb").read()) - P.SECTION_VA)
    found = [m.start() for m in re.finditer(re.escape(pack(k)), body)]
    entry = P.LATE_JOIN_VA + P.FOV_PROJECTION_OFFSET
    texts = [plain(t) for _, _, t in ndisasm(image.read(entry, 0xC0), entry)]
    store = texts.index(f"fstp [{k:#x}]") if f"fstp [{k:#x}]" in texts else -1
    check(len(found) == 5 and store > 0 and texts[:store].count("fptan") == 2 and "fcmovbe st1" in texts[:store]
          and "call 0xeee30" in texts[store + 1:],
          f"k ({k:#x}) is named five times in .srp: written by fov_projection alone (after its two fptan and the "
          "clamp to 1, before the matrix builder's call), read by the two scales")
    check(P.FOV_LOD_STATE_OFFSET >= P.TAB_LIST_STATE_OFFSET + P.TAB_LIST_STATE_SIZE
          and P.FOV_LOD_STATE_OFFSET % 4 == 0
          and P.FOV_LOD_STATE_OFFSET + P.FOV_LOD_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"k is at {P.FOV_LOD_STATE_OFFSET:#x} of the state page, aligned, after the other states "
          "(the scoreboard's the last)")
    hits = [hit for va, old, _, _ in P.FOV_LOD_HOOKS for hit in aimed_inside(original, va, len(old), 0x82)
            if not (va <= hit < va + len(old)) and starts_instruction(source, hit)]
    check(not hits, "no rel32 branch of .text, no rel8 branch near them and no 4-byte value aims inside the two sites"
          + "".join(f"\n          {a:#x}" for a in hits))
    relocated = sorted(va for kind, va in source.relocations().values() if kind == 3
                       and any(va + 4 > site and va < site + len(old) for site, old, _, _ in P.FOV_LOD_HOOKS))
    check(relocated == P.FOV_LOD_RELOCS, "the original has a base relocation under each site, at its constant's "
          f"address ({', '.join(f'{va:#x}' for va in P.FOV_LOD_RELOCS)}), disabled in the build")


# A battery's usable-object vtables: can_use at +30h, get_transform at +44h.
BATTERY_VTABLES = {0xA13E8C: b".?AVvictory_item_core@survarium@@", 0xA0437C: b".?AVvictory_item@survarium@@"}


def test_pickup(original, image, before):
    """Check 24: a battery under the floor one stands on (patch 23)."""
    source = Image(original)
    word = lambda blob, at: struct.unpack("<I", blob.read(at, 4))[0]
    va, old, new = P.PICKUP_FRAME
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(new)) == new,
          f"{va:#x}: the scan's frame grows by 8 (sub esp,34h -> 3Ch); the build without the late join keeps it")
    for va, old, offset, opcode in P.PICKUP_HOOKS:
        rows = [text for _, _, text in ndisasm(image.read(va, len(old)), va)]
        check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows[0].split()[0] == ("call" if opcode == 0xE8 else "jmp")
              and branch_target(rows[0]) == P.PICKUP_VA + offset and set(rows[1:]) <= {"nop"},
              f"{va:#x}: {rows[0]} (+{offset}) and nops; the original and the build without the late join keep it")
    code = open(P.pickup_bin(), "rb").read()
    rows = ndisasm(image.read(P.PICKUP_VA, len(code)), P.PICKUP_VA)
    outside = sorted({branch_target(text) for _, _, text in rows
                      if branch_target(text) is not None
                      and not P.PICKUP_VA <= branch_target(text) < P.PICKUP_VA + len(code)})
    check(image.read(P.PICKUP_VA, len(code)) == code and outside == [0x2D85C6, 0x2D85FC, 0x2D8622]
          and [text for _, _, text in rows if text.startswith("call")] == ["call [edx+0x44]"]
          and struct.unpack("<f", source.read(0xA710D8, 4))[0] == 2.0
          and code[-4:] == struct.pack("<f", P.PICKUP_ACROSS ** 2),
          f"the code at {P.PICKUP_VA:#x} is the .bin; it leaves only to the scan (0x2D85C6, 0x2D85FC, 0x2D8622), "
          f"calls only get_transform (vtable+44h); the distance (0xA710D8) is 2.0; across, {P.PICKUP_ACROSS:g} m")
    string = lambda at: source.read(at, 64).split(b"\0")[0]
    klass = lambda vtable: string(word(source, word(source, vtable - 4) + 12) + 8)
    can_use = struct.pack("<I", 0x300710)
    at, holders = original.find(can_use), []
    while at != -1:
        holders += [P.IMAGE_BASE + rva + at - raw for _, rva, _, raw, rsize, _ in source.sections if raw <= at < raw + rsize]
        at = original.find(can_use, at + 1)
    check(sorted(holders) == sorted(vtable + 0x30 for vtable in BATTERY_VTABLES)
          and all(klass(vtable) == name and word(source, vtable + 0x44) == 0x246ED0
                  for vtable, name in BATTERY_VTABLES.items())
          and source.read(0x246ED0, 8) == bytes.fromhex("83e914e9") + struct.pack("<i", 0x300A20 - 0x246ED8),
          "victory_item_core::can_use (0x300710) is at +30h of victory_item_core's and victory_item's usable "
          "vtables and nowhere else; their +44h, get_transform, is the battery's (sub ecx,14h; jmp 0x300A20)")
    found = [hit for va, size in pickup_sites() for hit in aimed_inside(original, va, size, 0x82)
             if not (va <= hit < va + size) and starts_instruction(source, hit)]
    check(not found, "no rel32 branch of .text, no rel8 branch near them and no 4-byte value aims inside the "
          "four sites" + "".join(f"\n          {a:#x}" for a in found))


# The respawn rules' vtables: player_respawn_rule_base, _core, the client's
# player_respawn_rule, single_player_respawn_rule.
RESPAWN_RULE_VTABLES = (0xA16014, 0xA16084, 0xA06D1C, 0xA06D8C)


def branches_to(original, target):
    """Every rel32 call, jmp or jcc of .text whose target is `target`."""
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    found = []
    for at in range(len(text) - 6):
        op, here = text[at], P.TEXT_VA + at
        if op in (0xE8, 0xE9) and here + 5 + struct.unpack_from("<i", text, at + 1)[0] == target:
            found.append(here)
        elif op == 0x0F and 0x80 <= text[at + 1] <= 0x8F and here + 6 + struct.unpack_from("<i", text, at + 2)[0] == target:
            found.append(here)
    return found


def rtti_name(image, vtable):
    """(the class name, the subobject's offset) of a vtable, from its
    complete object locator at vtable-4."""
    locator = struct.unpack("<I", image.read(vtable - 4, 4))[0]
    offset, descriptor = struct.unpack("<I", image.read(locator + 4, 4))[0], struct.unpack("<I", image.read(locator + 12, 4))[0]
    return image.read(descriptor + 8, 64).split(b"\0")[0].decode("latin1"), offset


def test_weapon_random(original, image, before):
    """Check 19: a spawn gives the new kit's weapons a random state of the
    match's own."""
    source = Image(original)
    va, old, offset, _ = P.WEAPON_RANDOM_HOOK
    entry = P.LATE_JOIN_VA + offset
    rows = lambda blob, at, size: ndisasm(blob.read(at, size), at)
    texts = lambda blob, at, size: [text for _, _, text in rows(blob, at, size)]
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
          and texts(source, va, len(old)) == ["mov edx,[esi]", "mov eax,[edx+0x20]"]
          and texts(image, va, len(old)) == [f"call 0x{entry:x}"],
          f"{va:#x}: `mov edx,[esi]` / `mov eax,[edx+0x20]` is `call 0x{entry:x}`; the original and the build without "
          "the late join keep them")
    # The spawn, 0x316DA0 to its `ret 0Ch`.
    spawn = rows(source, P.WEAPON_SPAWN_VA, 0x316EB3 - P.WEAPON_SPAWN_VA)
    at = {address: text for address, _, text in spawn}
    order = [address for address, _, _ in spawn]
    between = lambda lo, hi: [at[a] for a in order if lo <= a < hi]
    writes = lambda register, lo, hi: [text for text in between(lo, hi)
                                       if re.match(rf"(mov|lea|pop|xchg|add|sub|xor|or|and|inc|dec) {register}\b", text)]
    check(at.get(0x316DC8) == "mov ebp,[esp+0x20]" and at.get(0x316DE3) == "mov [eax+0xc],dl"
          and at.get(0x316E50) == "mov esi,[eax+ecx*4]" and at.get(0x316EB0) == "ret 0xc"
          and between(0x316E57, 0x316E72) == ["mov edx,[esi]", "mov edx,[edx+0x1c]", "sub esp,byte +0x8",
                                               "lea eax,[edi+0x8]", "mov ecx,esi", "fstp dword [esp+0x4]",
                                               "fld dword [edi+0x14]", "fstp dword [esp]", "push eax", "push ebp",
                                               "call edx"]
          and at.get(0x316E76) == "jna 0x316ea0"
          and between(0x316EA0, 0x316EB3) == ["mov edx,[esi]", "mov eax,[edx+0x20]", "push byte +0x1", "mov ecx,esi",
                                               "call eax", "pop edi", "pop esi", "pop ebp", "pop ebx", "pop ecx",
                                               "ret 0xc"]
          and not writes("ebp", 0x316DCC, 0x316EAD) and not writes("esi", 0x316E52, 0x316EAC),
          "the spawn (0x316DA0) makes the target kit current (0x316DE3), takes its player into esi (0x316E50), "
          "initialises it (vtable+1Ch) with the time of its third argument (ebp), and at 0x316EA0, which the path "
          "past the protection's timer also jumps to, loads the call that puts it into the world (vtable+20h); "
          "esi and ebp are written nowhere between, and nothing after the hook reads a flag")
    slot = lambda vtable, number: struct.unpack("<I", source.read(vtable + number, 4))[0]
    holders = []
    position = original.find(struct.pack("<I", P.WEAPON_SPAWN_VA))
    while position != -1:
        holders.append(position)
        position = original.find(struct.pack("<I", P.WEAPON_SPAWN_VA), position + 1)
    tick = texts(source, 0x316820, 0x316A24 - 0x316820)
    check(all(slot(vtable, 0x5C) == P.WEAPON_SPAWN_VA and slot(vtable, 0x50) in (0x316820, 0x267980)
              for vtable in RESPAWN_RULE_VTABLES)
          and sorted(holders) == sorted(source.offset(vtable + 0x5C) for vtable in RESPAWN_RULE_VTABLES)
          and not branches_to(original, P.WEAPON_SPAWN_VA)
          and texts(source, 0x317D30, 5) == ["mov eax,[ecx]", "jmp [eax+0x50]"]
          and "mov dword [esi+0x120],0x317d30" in texts(source, 0x316700, 0x60)
          and all(text in tick for text in ("mov edx,[ebp+0xc]", "mov eax,[eax+0x5c]", "call eax")),
          "the spawn is slot +5Ch of every respawn rule's vtable and nowhere else (no call aims at it); the rules "
          "of a match tick through 0x317D30 (slot +50h, 0x316820), which spawns each place through slot +5Ch with "
          "the time it was given")
    player = struct.unpack("<I", source.read(0x9FB35C + 0x1C, 4))[0]
    initialize = texts(source, 0x2DAC00, 0x350)
    check(player == 0x1F2540 and "call 0x2dac00" in texts(source, player, 0x100)
          and "mov edi,[esi+0x10c]" in initialize and "call 0x2fcb70" in initialize
          and initialize.index("call 0x2fcb70") == initialize.index("mov edi,[esi+0x10c]") + 3
          and all(text in texts(source, 0x2FD19A, 0x30) for text in ("lea esi,[edi+0x10c]", "mov ebp,0x17")),
          "the player's initialize (vtable+1Ch, 0x1F2540) runs base_player's (0x2DAC00), which sets the inventory "
          "(player+10Ch) up from the profile (0x2FCB70) and makes no object; the inventory's 23 slots are at +10Ch")
    weapon_core = texts(source, 0x2DF0D0, 0x400)
    weapon = texts(source, 0x243640, 0x30)
    bound = []
    position = original.find(struct.pack("<I", P.WEAPON_BIND_VA))
    while position != -1:
        bound.append(position)
        position = original.find(struct.pack("<I", P.WEAPON_BIND_VA), position + 1)
    names = {rtti_name(source, item_vtable) for item_vtable in (0xA0422C, 0xA1114C)}
    check(all(text in weapon_core for text in ("lea edi,[ebp+0x10]", "mov dword [edi],0xa1114c",
                                               "mov dword [ebp+0x0],0xa11094", "mov dword [ebp+0x5bc],0x1"))
          and weapon[weapon.index("call 0x2df0d0"):][1:3] == ["mov dword [ebx],0xa04174", "mov dword [ebx+0x10],0xa0422c"]
          and sorted(bound) == sorted(source.offset(item_vtable + 0x70) for item_vtable in (0xA0422C, 0xA1114C))
          and names == {(".?AVweapon@survarium@@", 0x10), (".?AVweapon_core@survarium@@", 0x10)},
          f"weapon_core's constructor puts its item at +10h and 1 into +5BCh; the client's weapon is one; "
          f"{P.WEAPON_BIND_VA:#x} (weapon_core's bind) is in the file only as +70h of their two item vtables")
    # Every [register+5BCh] of .text that is not on the stack.
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    sites, position = [], 0
    while (position := text.find(struct.pack("<I", P.WEAPON_RANDOM_FIELD), position)) >= 0:
        modrm, sib = text[position - 1], text[position - 2]
        if modrm & 0xC0 == 0x80 and modrm & 7 != 4:
            sites.append(P.TEXT_VA + position - 2)
        elif sib & 0xC0 == 0x80 and sib & 7 == 4 and modrm & 7 != 4:
            sites.append(P.TEXT_VA + position - 3)
        position += 1
    field = {0x2DF429: "mov dword [ebp+0x5bc],0x1", 0x2E1DCC: "lea esi,[ebx+0x5bc]", 0x2E2195: "lea eax,[esi+0x5bc]",
             0x2E315D: "lea eax,[ebx+0x5bc]", 0x2E34C1: "mov [edi+0x5bc],ecx"}
    check(sorted(sites) == sorted(field) and all(texts(source, site, 10)[0] == want for site, want in field.items())
          and "call 0x2e4850" in texts(source, 0x2E1DD2, 0x30) and "mov [esp+0x78],eax" in texts(source, 0x2E219B, 0x30)
          and all("mov ecx,0xbc8f" in texts(source, draw, 0x40) and "push dword 0x7fffffff" in texts(source, draw, 0x50)
                  for draw in (0x2AB6E0, 0x2E49F0)),
          "+5BCh of a register is in .text five times: the constructor's 1, the two spreads that draw from it "
          "(minstd_rand: 48271 mod 2^31-1, 0x2AB6E0 and 0x2E49F0), the serialize and the deserialize's store")
    hook = rows(image, entry, 0x80)
    hook = hook[:next(i for i, (_, _, text) in enumerate(hook) if text == "ret") + 1]
    hook_texts = [text for _, _, text in hook]
    pc = next(i for i, (_, _, text) in enumerate(hook) if text.startswith("call"))
    add = re.match(r"add ebx,0x([0-9a-f]+)$", hook_texts[pc + 2])
    stores = [text for text in hook_texts if re.match(r"mov \[", text)]
    inside = all(entry <= branch_target(text) < hook[-1][0] + 1 for text in hook_texts
                 if branch_target(text) is not None)
    check(hook_texts[0] == "pusha" and hook_texts[-4:] == ["popa", "mov edx,[esi]", "mov eax,[edx+0x20]", "ret"]
          and hook_texts[pc + 1] == "pop ebx" and branch_target(hook_texts[pc]) == hook[pc + 1][0]
          and add is not None and (hook[pc + 1][0] + int(add.group(1), 16)) & 0xFFFFFFFF == P.WEAPON_BIND_VA
          and "cmp [eax+0x70],ebx" in hook_texts and "mov ecx,[esi+0x10c]" in hook_texts
          and "cmp edi,byte +0x17" in hook_texts and "mov ecx,0x7ffffffe" in hook_texts
          and stores == [f"mov [eax+0x{P.WEAPON_RANDOM_FIELD - 0x10:x}],edx"] and inside,
          f"weapon_random_hook ({entry:#x}): pushad; for the 23 slots of the player's inventory, an item whose "
          f"vtable+70h is {P.WEAPON_BIND_VA:#x} (taken relative to the code) gets a state at item+5ACh (weapon+5BCh) "
          "and nothing else is written; popad, the two instructions, ret; no branch leaves it")
    found = aimed_inside(original, va, len(old), 0x82)
    check(not found, "no rel32 branch of .text, no rel8 branch byte near it and no 4-byte value aims inside the five "
          "bytes (0x316E76 aims at their start)" + "".join(f"\n          {a:#x}" for a in found))

def register_sites(original, field):
    """Every [register+field] (disp32, not esp-based) of .text, by the
    address of its instruction's opcode (a 0x66 prefix not counted)."""
    text_start = P.file_offset(P.TEXT_VA)
    text = bytes(original[text_start:text_start + P.TEXT_VSIZE])
    sites, position = [], 0
    while (position := text.find(struct.pack("<I", field), position)) >= 0:
        modrm, sib = text[position - 1], text[position - 2]
        if modrm & 0xC0 == 0x80 and modrm & 7 != 4:
            sites.append(P.TEXT_VA + position - 2)
        elif sib & 0xC0 == 0x80 and sib & 7 == 4 and modrm & 7 != 4:
            sites.append(P.TEXT_VA + position - 3)
        position += 1
    return sites


def test_kit_state(original, image, before):
    """Check 21: a spawn gives the new kit's weapons the state a weapon
    every game has alike."""
    source = Image(original)
    va, old, offset, _ = P.KIT_STATE_HOOK
    entry = P.LATE_JOIN_VA + offset
    rows = lambda blob, at, size: ndisasm(blob.read(at, size), at)
    texts = lambda blob, at, size: [text for _, _, text in rows(blob, at, size)]
    check(source.read(va, len(old)) == old and before.read(va, len(old)) == old
          and texts(source, va, len(old)) == ["mov edx,[esi]", "mov edx,[edx+0x1c]"]
          and texts(image, va, len(old)) == [f"call 0x{entry:x}"],
          f"{va:#x}: `mov edx,[esi]` / `mov edx,[edx+0x1c]` is `call 0x{entry:x}`; the original and the build "
          "without the late join keep them")
    spawn = rows(source, P.WEAPON_SPAWN_VA, 0x316EB3 - P.WEAPON_SPAWN_VA)
    at = {address: text for address, _, text in spawn}
    order = [address for address, _, _ in spawn]
    between = lambda lo, hi: [at[a] for a in order if lo <= a < hi]
    check(at.get(0x316DDD) == "mov cl,[ecx+0x115]" and at.get(0x316DE3) == "mov [eax+0xc],dl"
          and at.get(0x316DFC) == "mov al,[eax+0x115]" and at.get(0x316E02) == "mov [esp+0x10],cl"
          and at.get(0x316E06) == "mov [esp+0x18],al" and at.get(0x316E0A) == "cmp cl,al"
          and at.get(0x316E0C) == "jz 0x316e3b" and at.get(0x316E53) == "mov edi,[esp+0x1c]"
          and between(0x316E3B, 0x316E57) == ["mov eax,[esi+0x134]", "fldz", "mov eax,[eax+0x18458]",
                                               "movzx ecx,byte [eax+edi+0xc]", "add eax,edi",
                                               "mov esi,[eax+ecx*4]", "mov edi,[esp+0x1c]"]
          and not [text for text in between(0x316E0A, 0x316E57) if re.match(r"mov (byte )?\[esp", text)],
          "the spawn keeps the kit of the player that was current at [esp+10h] (0x316E02) and the new one's at "
          "[esp+18h] (0x316E06) and tests them for a change of kit (0x316E0A); both paths read the spawn point at "
          "[esp+1Ch] at 0x316E53 with the same esp, nothing writes the two bytes again, and esi is the new kit's "
          "player at 0x316E57")
    ctor = texts(source, 0x2DF0D0, 0x480)
    check(all(text in ctor for text in (
        "or eax,byte -0x1", "mov [ebp+0x5a4],eax", "mov [ebp+0x5a8],eax", "mov [ebp+0x5b1],bl",
        "mov [ebp+0x5b2],bl", "mov [ebp+0x5e5],bl", "mov [ebp+0x5e6],bl", "mov [ebp+0x5e8],bl",
        "mov [ebp+0x5f1],bl", "mov [ebp+0x5f5],bl", "mov dword [ebp+0x5ec],0x6", "movss dword [ebp+0x5fc],xmm0",
        "movss dword [ebp+0x600],xmm0", "movss dword [ebp+0x604],xmm0", "movss dword [ebp+0x608],xmm0",
        "mov dword [ebp+0x60c],0xffffffff", "xor ebx,ebx"))
          and ctor.index("xor ebx,ebx") < ctor.index("mov [ebp+0x5b1],bl")
          and not [text for text in ctor[ctor.index("xor ebx,ebx") + 1:ctor.index("mov [ebp+0x5f5],bl")]
                   if re.match(r"(mov|xor|or|and|add|sub|inc|dec|pop|lea|movzx|setn?z) (ebx|bl|bx),", text)]
          and ctor[ctor.index("mov [ebp+0x5a4],eax") - 1] == "or eax,byte -0x1"
          and ctor[ctor.index("mov [ebp+0x5a8],eax") - 1] == "mov [ebp+0x5a4],eax"
          and "xorps xmm0,xmm0" in ctor[ctor.index("mov [ebp+0x5e0],ebx"):ctor.index("movss dword [ebp+0x5fc],xmm0")]
          and not [text for text in ctor[ctor.index("mov [ebp+0x5e0],ebx"):ctor.index("movss dword [ebp+0x608],xmm0")]
                   if re.match(r"\w+ xmm0,", text) and text != "xorps xmm0,xmm0"],
          "weapon_core's constructor (0x2DF0D0) gives +5A4h and +5A8h -1, +5B1h, +5B2h, +5E5h, +5E6h, +5E8h, "
          "+5F1h and +5F5h 0, +5ECh 6, +5FCh..+608h 0.0 and +60Ch -1: KIT_STATE_FIELDS")
    initialize = texts(source, 0x2DAC00, 0x260)
    activate = texts(source, 0x2E2A30, 0x290)
    core_vtable = lambda number: struct.unpack("<I", source.read(0xA11094 + number, 4))[0]
    check("mov [esi+0x324],ebx" in initialize and initialize.index("mov ebx,[ebp+0x8]") < initialize.index(
        "mov [esi+0x324],ebx") and "call 0x2fcb70" in initialize and "mov [esi+0x128],ecx" in initialize
          and initialize.index("call 0x2fcb70") < initialize.index("mov [esi+0x128],ecx")
          and "mov eax,[edx+0x14]" in initialize[initialize.index("mov [esi+0x128],ecx"):]
          and core_vtable(0x14) == 0x2E2A30
          and "mov esi,[eax+0x324]" in activate and "mov [edi+0x5a4],esi" in activate
          and activate.index("mov esi,[eax+0x324]") < activate.index("mov [edi+0x5a4],esi")
          and all(text in activate for text in ("mov byte [edi+0x5e8],0x0", "mov dword [edi+0x5ec],0x6",
                                                "mov byte [edi+0x5f5],0x0", "mov dword [edi+0x60c],0xffffffff",
                                                "mov dword [edi+0x5a8],0xffffffff")),
          "the initialize keeps the spawn's time at +324h, sets the inventory up (0x2FCB70) and activates the item "
          "it puts in hand (+128h, vtable+14h: weapon_core's 0x2E2A30), which sets +5A4h to that time and the aim, "
          "+5ECh, +5F5h, +60Ch and +5A8h as a new weapon has them")
    tick_sites = {0x244E20: "mov eax,[ebx+0x5a4]", 0x26D028: "mov ecx,[esi+0x5a4]", 0x2DF405: "mov [ebp+0x5a4],eax",
                  0x2E09EF: "sub edi,[ebp+0x5a4]", 0x2E09F5: "mov [ebp+0x5a4],ebx", 0x2E1BAF: "mov ecx,[edi+0x5a4]",
                  0x2E1C4B: "mov ecx,[edi+0x5a4]", 0x2E2696: "mov eax,[ecx+0x5a4]", 0x2E2C2B: "mov [edi+0x5a4],esi",
                  0x2E317D: "lea eax,[ebx+0x5a4]", 0x2E34D2: "mov [edi+0x5a4],ecx"}
    sites = [site for site in register_sites(original, 0x5A4) if "+0x5a4]" in texts(source, site, 8)[0]]
    check(sorted(sites) == sorted(tick_sites)
          and all(texts(source, site, 8)[0] == want for site, want in tick_sites.items())
          and core_vtable(0x28) == 0x2E09E0,
          "+5A4h of a register is in .text eleven times: the constructor, the weapon's tick (vtable+28h, 0x2E09E0: "
          "the time since it, then the tick's time), the activation, the serialize and the deserialize, the "
          "aim's two (0x2E1B50, 0x2E1BF0, of the weapon in hand), the change of ammunition (0x2E2650, by the "
          "player's input), the client's draw (0x244E10) and 0x26D028 (another class's +5A4h, read by hand)")
    fire_mode = {0x2DF4A0: "mov [ebp+0x5e6],bl", 0x2E0CE1: "mov [ecx+0x5e6],dl", 0x2E3504: "mov [edi+0x5e6],cl"}
    sites = [site for site in register_sites(original, 0x5E6) if "+0x5e6]" in texts(source, site, 8)[0]]
    writers = [site for site in sites if not re.match(r"(movzx|cmp|lea|test|push) ", texts(source, site, 8)[0])]
    check(sorted(writers) == sorted(fire_mode)
          and all(texts(source, site, 8)[0] == want for site, want in fire_mode.items()),
          "the fire mode (+5E6h) is written by the constructor, the player's input (0x2E0CA0) and the deserialize "
          "alone: it is kept through deaths and spawns")
    hook = rows(image, entry, 0xA0)
    hook = hook[:next(i for i, (_, _, text) in enumerate(hook) if text == "ret") + 1]
    hook_texts = [text for _, _, text in hook]
    pc = next(i for i, (_, _, text) in enumerate(hook) if text.startswith("call"))
    add = re.match(r"add ebx,0x([0-9a-f]+)$", hook_texts[pc + 2])
    base = 0x594 + 0x10
    stores = {}
    for text in hook_texts:
        match = re.match(r"(or|mov) (byte |dword )?\[edx(?:\+0x([0-9a-f]+))?\],(.*)$", text)
        if not match:
            continue
        field = base + int(match.group(3) or "0", 16)
        operand = match.group(4)
        if operand == "byte -0x1":
            stores[field] = (4, 0xFFFFFFFF)
        elif operand == "bp":
            stores[field], stores[field + 1] = (1, 0), (1, 0)
        elif operand == "ebp":
            stores[field] = (4, 0)
        elif match.group(2) == "byte " and operand == "0x0":
            stores[field] = (1, 0)
        elif match.group(2) == "dword ":
            stores[field] = (4, int(operand, 16))
        else:
            stores[field] = None
    wanted = {offset: (size, value) for offset, size, value in P.KIT_STATE_FIELDS}
    wanted[P.KIT_STATE_TICK_FIELD] = (4, 0xFFFFFFFF)
    tick_store = hook_texts.index("or dword [edx],byte -0x1")
    inside = all(entry <= branch_target(text) < hook[-1][0] + 1 for text in hook_texts
                 if branch_target(text) is not None)
    other_writes = [text for text in hook_texts if re.match(r"(mov|or|and|add|sub|xor|inc|dec) [a-z ]*\[", text)
                    and "[edx" not in text]
    check(hook_texts[0] == "pusha" and hook_texts[-4:] == ["popa", "mov edx,[esi]", "mov edx,[edx+0x1c]", "ret"]
          and hook_texts[pc + 1] == "pop ebx" and branch_target(hook_texts[pc]) == hook[pc + 1][0]
          and add is not None and (hook[pc + 1][0] + int(add.group(1), 16)) & 0xFFFFFFFF == P.WEAPON_BIND_VA
          and "cmp [ebp+0x70],ebx" in hook_texts and "mov ecx,[esi+0x10c]" in hook_texts
          and "mov al,[esp+0x34]" in hook_texts and "cmp al,[esp+0x3c]" in hook_texts and "setnz al" in hook_texts
          and "add edx,0x594" in hook_texts and "cmp edi,byte +0x17" in hook_texts
          and hook_texts[tick_store + 1:tick_store + 3] == ["test al,al", f"jz 0x{hook[-7][0]:x}"]
          and hook_texts[-7:-4] == ["inc edi", "cmp edi,byte +0x17", f"jc 0x{hook[hook_texts.index('mov edx,[ecx+edi*4+0x10c]')][0]:x}"]
          and stores == wanted and not other_writes and inside,
          f"kit_state_hook ({entry:#x}): pushad; for the 23 slots of the player's inventory, an item whose "
          f"vtable+70h is {P.WEAPON_BIND_VA:#x} gets -1 at weapon+5A4h and, when the kit before (the spawn's "
          "[esp+10h]) is not the new one ([esp+18h]), KIT_STATE_FIELDS; nothing else is written; popad, the two "
          "instructions, ret; no branch leaves it")
    found = aimed_inside(original, va, len(old), 0x82)
    check(not found, "no rel32 branch of .text, no rel8 branch byte near it and no 4-byte value aims inside the five "
          "bytes" + "".join(f"\n          {a:#x}" for a in found))


def test_first_load(original, image, before):
    """Check 20: a first world's inputs and kit choices, in time order."""
    source = Image(original)
    texts = lambda blob, at, size: [text for _, _, text in ndisasm(blob.read(at, size), at)]
    sites = []
    for (va, old, offset, _), was in ((P.FIRST_LOAD_INPUT_HOOK, 0x2D4AB0), (P.FIRST_LOAD_KIT_HOOK, 0x2D4B50)):
        entry = P.LATE_JOIN_VA + offset
        sites.append((va, old, entry, was))
        check(source.read(va, 5) == old and before.read(va, 5) == old
              and texts(source, va, 5) == [f"call 0x{was:x}"] and texts(image, va, 5) == [f"call 0x{entry:x}"]
              and branches_to(original, was) == [va],
              f"{va:#x}: `call 0x{was:x}` is `call 0x{entry:x}`; it was the only call of 0x{was:x}; the build "
              "without the late join keeps it")
    block = texts(source, 0x2D0FBE, 0x2D1067 - 0x2D0FBE)
    check(block[0] == "cmp byte [edi+0x18fdf],0x0" and "jz near 0x2d105e" in block
          and "mov byte [edi+0x18fdf],0x0" in block
          and all(text in block for text in ("mov [esp+0x24],ecx", "mov [esp+0x28],ecx", "mov [esp+0x2c],dl",
                                             "lea edi,[esp+0x24]", "and esi,ebx", "jnz 0x2d0ff2"))
          and block[block.index("lea edi,[esp+0x24]") + 1:block.index("lea edi,[esp+0x24]") + 4]
          == ["mov [esp+0x2c],dl", "call 0x2d4ab0", "call 0x2d4b50"]
          and texts(source, 0x2D0FB9, 5) == ["call 0x2d2770"]
          and "mov [edi+0xd],al" in texts(source, 0x2D2A30, 0x10),
          "the two calls are in deserialize's block of a game's first world (the frozen flag, +18FDFh, cleared), "
          "for each place of the world's mask, with edi -> {core, the world's time, place}, after the world is "
          "read (0x2D2770, which puts the world's next kit into place+0Dh at 0x2D2A37)")
    kit, given = texts(source, 0x2D4B50, 0x87), texts(source, 0x2D4AB0, 0x95)
    check("mov bl,[edx+0x115]" in kit and "add esi,[ecx+0x89d8]" in kit and "call 0x1fcb60" in kit
          and "add esi,[ecx+0x2fcc]" in given and "mov ebx,[eax+0x32c]" in given and "mov ebp,[eax+0x330]" in given
          and "call 0x1fcb60" in given and "call 0x2d4850" not in kit + given,
          "in the original the kit choice is the kit in use (player+115h) and both put a new entry at the newest "
          "end themselves, not through the list's insert (0x2D4850)")
    tick = texts(source, 0x321790, 0x4B)
    network = texts(source, 0x1FBF00, 0x70)
    inputs = texts(source, 0x1FB41B, 0x60)
    check(all(text in tick for text in ("mov edx,[edx+0x89d8]", "mov eax,[eax+edx+0x8]", "cmp [eax+0x20],edx",
                                        "mov eax,[eax+0x4]", "mov [ecx+0xd],al"))
          and all(text in network for text in ("add ebx,[edi+0x89d8]", "mov eax,[ebx+0x4]", "cmp [eax+0x20],esi",
                                               "mov eax,[eax]", "call 0x2d4850"))
          and all(text in inputs for text in ("add eax,[ebx+0x2fcc]", "mov esi,[eax+0x4]", "cmp [esi+0x20],edi",
                                              "mov esi,[esi]", "call 0x1fcb60")),
          "each tick (0x321790) takes the next kit from the newest entry at or before it, looked for from the "
          "newest end; the network puts kit choices and inputs before the oldest newer entry (0x1FBF00, 0x1FB41B)")
    code = ndisasm(image.read(sites[0][2], 0xC0), sites[0][2])
    code = code[:next(i for i, (address, _, text) in enumerate(code)
                      if text == "ret" and address > sites[1][2] + 0x2E) + 1]
    code_texts = [text for _, _, text in code]
    end = code[-1][0] + 1
    calls = sorted({branch_target(text) for text in code_texts if text.startswith("call")})
    leaving = [text for text in code_texts if branch_target(text) is not None and not text.startswith("call")
               and not sites[0][2] <= branch_target(text) < end]
    stores = sorted({text for text in code_texts if text.startswith("mov [")})
    check(code[0][0] == sites[0][2] and any(address == sites[1][2] for address, _, _ in code)
          and "mov bl,[eax+0xd]" in code_texts and "mov ebx,[eax+0x32c]" in code_texts
          and calls[:2] == [0x1FCB60, 0x2D4850] and all(sites[0][2] <= c < end for c in calls[2:])
          and not leaving and stores == ["mov [eax+0x20],ecx", "mov [eax+0x8],bl", "mov [eax+0x8],ebx",
                                         "mov [eax+0xc],ebp"],
          f"first_load_input ({sites[0][2]:#x}) and first_load_kit ({sites[1][2]:#x}): the kit choice is place+0Dh; "
          "they call the game's allocator (0x1FCB60) and the list's insert (0x2D4850) and their own search alone, "
          "write a node's value and time alone, and no branch leaves them")
    found = [address for va, old, _, _ in sites for address in aimed_inside(original, va, len(old), 0x82)]
    check(not found, "no rel32 branch of .text, no rel8 branch byte near them and no 4-byte value aims inside the "
          "two calls" + "".join(f"\n          {a:#x}" for a in found))


def test_db_signature(original, image, before):
    """Check 25: no resources.db signature is checked."""
    va, old, new = P.DB_SIGNATURE
    # ndisasm 2.x spells the last one `add esp,byte +0x10`, 3.x `add esp,0x10`.
    texts_at = lambda blob: [text.replace("byte +", "") for _, _, text in ndisasm(blob.read(va - 2, 10), va - 2)]
    check(texts_at(Image(original)) == ["mov eax,esi", f"call {P.DB_SIGNATURE_IMPL:#x}", "add esp,0x10"]
          and texts_at(image) == ["mov eax,esi", "xor eax,eax", "nop", "nop", "nop", "add esp,0x10"]
          and image.read(va, len(new)) == new and before.read(va, len(old)) == old,
          f"{va:#x}: verify_sign's `call {P.DB_SIGNATURE_IMPL:#x}` is `xor eax,eax` and three nops between the "
          "original's instructions; the build without the late join keeps the original call")
    under = [v for kind, v in Image(original).relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    # rel32 branches of .text and 4-byte values of the file; the short
    # branches of verify_sign itself (0x13310..0x13608), instruction by
    # instruction (a byte-wise scan reads `lea esi,[esp+10h]` at 0x135AD
    # as a jz into the call).
    found = aimed_inside(original, va, len(old), 0)
    verify_sign = ndisasm(Image(original).read(0x13310, 0x2F8), 0x13310)
    found += [address for address, _, text in verify_sign
              if (target := branch_target(text)) is not None and va < target < va + len(old)]
    check(not under and not found and any(address == va for address, _, _ in verify_sign),
          "no base relocation under the call, no branch or address aims inside it" + "".join(f"\n          {a:#x}" for a in found))


def test_stale_dll(original, image, before):
    r"""Check 26: the sign-in says whether %TEMP%\shell_32.dll exists."""
    va, old, new = P.STALE_DLL
    entry = P.STALE_DLL_ENTRY
    check(image.read(va, len(new)) == new and before.read(va, len(old)) == old
          and [text for _, _, text in ndisasm(image.read(va, 6), va)] == [f"call {entry:#x}", "nop"]
          and [text for _, _, text in ndisasm(Image(original).read(va, 6), va)]
          == ["mov [edi+0x8],eax", "mov [edi+0xc],dl"],
          f"{va:#x}: `mov [edi+0x8],eax` / `mov [edi+0xc],dl` are `call {entry:#x}` / `nop`; "
          "the build without the late join keeps them")
    under = [v for kind, v in Image(original).relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    start, end = SIGN_IN_ON_CONNECTED
    found = aimed_inside(original, va, len(old), 0)
    found += [address for address, _, text in ndisasm(Image(original).read(start, end - start), start)
              if (target := branch_target(text)) is not None and va < target < va + len(old)]
    check(not under and not found,
          "no base relocation under the two instructions, no branch or address aims inside them"
          + "".join(f"\n          {a:#x}" for a in found))
    code = open(P.stale_dll_bin(), "rb").read()
    check(image.read(entry, len(code)) == code, f"stale_dll at {entry:#x} is stale_dll_026g.section.bin")
    strings = code[code.index(b"kernel32.dll\0"):]
    listing = ndisasm(code[:len(code) - len(strings)], entry)
    texts = [text for _, _, text in listing]
    here = listing[2][0]
    slots = {0x87003C, 0x870040, 0x870048, 0x870084}
    calls = [text for text in texts if text.startswith("call")]
    indirect = {int(m.group(1), 16) for text in calls
                if (m := re.match(r"^call (?:dword (?:near )?)?\[ebp\+0x([0-9a-f]+)\]$", text))}
    other = [text for text in calls if not re.match(r"^call (?:dword (?:near )?)?\[ebp\+0x", text)]
    jumps = [branch_target(text) for text in texts if text.startswith("j")]
    check(texts[:2] == ["pusha", f"call {here:#x}"]
          and texts[-4:] == ["popa", "mov [edi+0x8],eax", "mov [edi+0xc],dl", "ret"]
          and indirect == slots and other == [f"call {here:#x}", "call eax"]
          and all(entry <= target < entry + len(code) for target in jumps),
          "it saves every register (pushad / popad), calls itself once for its address, GetModuleHandleA, "
          "GetProcAddress, CreateFileA and CloseHandle through the import table and GetTempPathA, "
          "branches only inside itself, then does the two writes it replaced and returns")
    check(strings == b"kernel32.dll\0GetTempPathA\0shell_32.dll\0",
          "the names it holds are kernel32.dll, GetTempPathA and shell_32.dll, and nothing else")


# query_db and on_mounted_resources, for the branches of their own that
# could aim inside the two sites of patch 27.
QUERY_DB = (0xE7D90, 0xE7E75)
ON_MOUNTED_RESOURCES = (0x147E0, 0x148A5)


def test_mount_patches(original, image, before):
    """Check 27: the game mounts resources.db.patch_1, _2, ... over
    resources.db (mount_patches_026g.asm; check_mount_patches_026g.py runs
    it)."""
    code = open(P.mount_patches_bin(), "rb").read()
    entry, mounted = P.MOUNT_PATCHES_VA + P.MOUNT_ALL_OFFSET, P.MOUNT_PATCHES_VA + P.MOUNTED_OFFSET
    texts = lambda blob, va, size: [text for _, _, text in ndisasm(blob.read(va, size), va)]
    va, old, new = P.MOUNT_QUERY
    check(image.read(va, len(new)) == new and before.read(va, len(old)) == old
          and texts(Image(original), va, 5) == [f"call {P.MOUNT_QUERY_MOUNT_ARCHIVE:#x}"]
          and texts(image, va, 5) == [f"call {entry:#x}"],
          f"{va:#x}: query_db's `call {P.MOUNT_QUERY_MOUNT_ARCHIVE:#x}` (query_mount_archive) is `call {entry:#x}`; "
          "the build without the late join keeps it")
    va, old, new = P.MOUNTED
    rows = ndisasm(Image(original).read(va, len(old)), va)
    check(image.read(va, len(new)) == new and before.read(va, len(old)) == old
          and [text for _, _, text in rows[:2]] == ["push ebp", "mov ebp,esp"] and len(rows) == 3
          and rows[2][1].lower() == "83e4f8" and texts(image, va, 6) == [f"jmp {mounted:#x}", "nop"],
          f"{va:#x}: on_mounted_resources' `push ebp` / `mov ebp,esp` / `and esp,-8` are `jmp {mounted:#x}` / `nop`; "
          "the build without the late join keeps them")
    relocated = {v for kind, v in Image(original).relocations().values() if kind == 3}
    sections = Image(original).sections

    def an_address(value):
        """Whether `value` is in the file as an address of the image: a
        4-byte value with a base relocation (`call 3DB6F0h` at 0x3C6F0A is
        `E8 E1 47 01 00`, the bytes of 0x147E1)."""
        at = original.find(struct.pack("<I", value))
        while at != -1:
            if any(raw <= at < raw + rsize and P.IMAGE_BASE + rva + at - raw in relocated
                   for _, rva, _, raw, rsize, _ in sections):
                return True
            at = original.find(struct.pack("<I", value), at + 1)
        return False

    for (va, old, _), (start, end) in ((P.MOUNT_QUERY, QUERY_DB), (P.MOUNTED, ON_MOUNTED_RESOURCES)):
        under = [v for v in relocated if va - 4 < v < va + len(old)]
        found = [a for a in aimed_inside(original, va, len(old), 0) if not va < a < va + len(old) or an_address(a)]
        found += [address for address, _, text in ndisasm(Image(original).read(start, end - start), start)
                  if (target := branch_target(text)) is not None and va < target < va + len(old)]
        check(not under and not found, f"{va:#x}: no base relocation under it, no branch or address aims inside it"
              + "".join(f"\n          {a:#x}" for a in found))
    check(image.read(P.MOUNT_PATCHES_VA, len(code)) == code,
          f"the patches' mount at {P.MOUNT_PATCHES_VA:#x} is mount_patches_026g.section.bin ({len(code)} bytes)")
    state = struct.unpack_from("<I", code, len(code) - 4)[0]
    late_join = open(P.late_join_bin(), "rb").read()
    page = P.late_join_state_va(late_join)
    size = P.MOUNT_STATE_SIZE
    check(state == page + P.MOUNT_STATE_OFFSET and P.MOUNT_STATE_OFFSET + size <= P.LATE_JOIN_STATE_SIZE,
          f"its state is {size} bytes at {state:#x}, +{P.MOUNT_STATE_OFFSET:#x} of the late join's state page "
          f"{page:#x}, inside it")
    strings = code[code.index(b"../../%s.db.patch_%u\0"):len(code) - 4 - P.MOUNT_REQUIRED_SIZE]
    check(strings == b"../../%s.db.patch_%u\0"
          b"Cannot mount resources.db.patch_%u. Please run the Skhid launcher again.\0"
          b"Cannot open resources.db.patch_%u (error %u). Please run the Skhid launcher again.\0",
          "the texts it holds are the path's format, ../../%s.db.patch_%u, and the two fatal errors' "
          "(a patch that fails to mount; one that is there and cannot be opened)")
    listing = ndisasm(code[:len(code) - 4 - P.MOUNT_REQUIRED_SIZE - len(strings)], P.MOUNT_PATCHES_VA)
    end = P.MOUNT_PATCHES_VA + len(code)
    inside = lambda target: P.MOUNT_PATCHES_VA <= target < end
    outside = sorted({target for _, _, text in listing if (target := branch_target(text)) is not None
                      and not inside(target)})
    indirect = sorted({int(m.group(1), 16) for _, _, text in listing
                       if (m := re.match(r"^call (?:dword (?:near )?)?\[edx\+0x([0-9a-f]+)\]$", text))})
    at = {address: text for address, _, text in listing}
    # The six bytes the hook replaced, then on into the game. ndisasm 2.x
    # spells the `and` `and esp,byte -0x8`, 3.x `and esp,0xfffffffffffffff8`.
    games = [i for i, (_, _, text) in enumerate(listing) if text == "push ebp"]
    tail = [text.replace("byte -0x8", "0xfffffff8").replace("0xfffffffffffffff8", "0xfffffff8")
            for _, _, text in listing[games[0]:games[0] + 4]] if len(games) == 1 else []
    calls = sorted([0x18190, P.MOUNTED_VA + 6, P.MOUNT_QUERY_MOUNT_ARCHIVE, 0x113580, 0xDFE30])
    check(outside == calls and indirect == [0x870040, 0x870048, 0x87005C]
          and at.get(mounted, "").startswith("call") and at.get(entry) == "pusha"
          and tail == ["push ebp", "mov ebp,esp", "and esp,0xfffffff8", f"jmp {P.MOUNTED_VA + 6:#x}"],
          "its only ways out are the path's format (0x18190), query_mount_archive, the fatal error (0x113580), "
          "the unmount of a mount nobody holds (0xDFE30, as the game lets a mount go), "
          "CreateFileA, GetLastError and CloseHandle (through the import table, plus the delta), and the six bytes it "
          f"replaced followed by a jump to {P.MOUNTED_VA + 6:#x}; on_mounted at +{P.MOUNTED_OFFSET:#x} starts "
          f"with a call for its address, mount_all at +{P.MOUNT_ALL_OFFSET:#x} with pushad"
          + ("" if outside == calls else f": {[hex(t) for t in outside]}"))


# effect_fstage_default_materials::compile and renderer_context's
# constructor, [start, end) of their code, for the branches of their own
# that could aim inside the sites of patch 28.
FORWARD_COMPILE = (0x1D8F40, 0x1D9740)
RENDERER_CONTEXT = (0x148570, 0x149A41)
# What the functions compile calls take off the stack (`ret N`), for the
# depth of its stack at each instruction.
FORWARD_CALLEES = {0xF6C00: 0, 0xF6B10: 0, 0x1CD0B0: 0x18, 0x3D98B0: 0x20, 0x1C0960: 8, 0x1A8900: 8,
                   0x1BFCB0: 0, 0x1D9740: 8, 0x3D9D80: 4, 0x3DA450: 4, 0x3DA6B0: 4}
FORWARD_EXE_TEXTS = {0x9F8E04: b"use_uv_scrolling", 0x9E1C48: b"value",
                     0x9F971C: b"emissive_map_scroll_u_direction", 0x9F973C: b"emissive_map_scroll_v_direction",
                     0x9F975C: b"uv_scrolling_parameters0", 0x9F9778: b"uv_scrolling_parameters1"}
FORWARD_RESOURCE_MANAGER = 0x226047C


def forward_configuration_accesses(stock):
    """[(address, text, offset in the shader configuration, written)] of
    every [esp+X] operand of compile from its prologue to its epilogue that
    falls inside the configuration ([esp+50h] with no push pending), the
    depth of the stack followed instruction by instruction (each call takes
    off what FORWARD_CALLEES says; compile pushes only for its calls).
    None when an instruction moves esp otherwise."""
    start, end = FORWARD_COMPILE
    rows = ndisasm(stock.read(start, end - start), start)
    depth, out = 0, []
    for address, _, text in rows:
        if address < 0x1D8F51 or address >= 0x1D9737:       # the prologue's and the epilogue's own pushes
            continue
        operand = re.search(r"\[esp\+0x([0-9a-f]+)\]", text)
        if operand:
            offset = int(operand.group(1), 16) - depth - 0x50
            if 0 <= offset < 16:
                written = re.match(r"^(mov[a-z]*|or|and|xor|add|sub|inc|dec|not|neg|shl|shr|set[a-z]+)\s+"
                                   r"(byte |word |dword |qword |oword )?\[esp", text) is not None
                out.append((address, text, offset, written))
        if text.startswith("push"):
            depth += 4
        elif text.startswith("pop"):
            depth -= 4
        elif text.startswith("call"):
            target = branch_target(text)
            if target not in FORWARD_CALLEES:
                return None
            depth -= FORWARD_CALLEES[target]
        elif re.match(r"^[a-z]+ esp,", text):
            return None                                     # esp moved otherwise
    return out if depth == 0 else None


def test_forward_scroll(original, image, before):
    """Check 28: the transparent effects scroll as in 0.29
    (forward_scroll_026g.asm; check_forward_scroll_026g.py runs it)."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    texts = {0x1D90DC: ["mov dl,[esp+0x56]", "mov esi,[ebp+0x10]"],
             0x1D94CB: ["mov cl,[esp+0x57]", "and cl,0x3"],
             0x14990F: ["mov [esp+0x2c],edi", "lea esi,[ebx+0x123c]"]}
    returns = {}
    for va, old, offset, opcode in P.FORWARD_SCROLL_HOOKS:
        entry = P.FORWARD_SCROLL_VA + offset
        new = P.hook_bytes(P.FORWARD_SCROLL_VA, va, old, offset, opcode)
        returns[entry] = va + len(old)
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new
              and rows(stock, va, len(old)) == texts[va]
              and rows(image, va, len(old)) == [f"jmp {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(texts[va])}` are `jmp {entry:#x}` and {len(old) - 5} nop; the build without "
              "the late join keeps them")
    # compile around the two hooks.
    check(rows(stock, 0x1D90C8, 0x14) == ["mov edi,0x9e1c48", "call 0xf6b10", "cmp dword [eax],byte +0x0",
                                          "setnz al", "jmp short 0x1d90dc", "xor al,al"]
          and rows(stock, 0x1D90E3, 0xE) == ["mov ecx,[esi+0x8]", "mov edi,[ebp+0x8]", "shl al,byte 0x6",
                                             "and dl,0x3f", "or al,dl"]
          and rows(stock, 0x1D90FC, 0x2F) == ["push esi", "and eax,ecx", "push ebx", "lea edx,[esp+0x58]", "push edx",
                                              "mov [esp+0x3c],eax", "mov al,[esp+0x63]", "setnz cl", "push edi",
                                              "shl cl,byte 0x6", "and al,0x3f", "push dword 0x9f8128", "or cl,al",
                                              "push dword 0x9f8138", "mov [esp+0x6f],cl", "call 0x1cd0b0"],
          "0x1D90DC: both ends of use_angle_depended_attenuation's read meet there (al the switch, the flags not "
          "read again before `and eax,ecx`), esi and edi are written again before they are read, and compile_begin "
          "(0x1CD0B0) is given the configuration at [esp+50h] (`lea edx,[esp+58h]` after two pushes)")
    check(rows(stock, 0x1D94C1, 0x16) == ["call 0x1c0960", "mov esi,0x2", "mov cl,[esp+0x57]", "and cl,0x3",
                                          "cmp cl,0x2", "jnz 0x1d9512"]
          and rows(stock, 0x1D9319, 0xB) == ["test byte [esp+0x52],0x80", "jz near 0x1d94cb"]
          and rows(stock, 0x1D92DD, 3) == ["mov eax,[ebp+0x8]"] and rows(stock, 0x1D9309, 1) == ["push eax"]
          and rows(stock, 0x1D9314, 5) == ["call 0x1a8900"],
          "0x1D94CB: the uv distortion's block ends there from both sides (esi 2 on both), the flags are written "
          "again (`cmp cl,2`) before they are read; compile's own set_constant<float4> (0x1A8900) is given [ebp+8], "
          "its first argument, as the compiler")
    accesses = forward_configuration_accesses(stock)
    writes = sorted({offset for _, _, offset, written in accesses or [] if written})
    scroll = [(address, text) for address, text, offset, written in accesses or [] if offset == 0x0C]
    check(accesses is not None and scroll == [(0x1D8FA3, "mov [esp+0x5c],esi")]
          and any(offset == 4 and text == "mov [esp+0x54],cl" and address == 0x1D9075
                  for address, text, offset, _ in accesses)
          and any(address == 0x1D9228 and text == "test byte [esp+0x54],0x10" for address, text, _, _ in accesses),
          "the configuration's byte 0Ch is written in compile only where it is zeroed (0x1D8FA3), before any "
          "switch is read; use_sequence is bit 4 of byte 4 (written at 0x1D9075, read at 0x1D9228); bytes written: "
          + ", ".join(f"{offset:#x}" for offset in writes))
    # The g_stage effect: the same bit from the same switch, the same two constants.
    check(rows(stock, 0x1DE4F1, 0x46) == ["mov esi,0x9f8e04", "mov eax,ebx", "call 0xf6c00", "test al,al",
                                          "jz 0x1de51d", "mov edi,esi", "mov eax,ebx", "call 0xf6b10",
                                          "mov edi,0x9e1c48", "call 0xf6b10", "cmp dword [eax],byte +0x0",
                                          "jz 0x1de51d", "mov al,0x1", "jmp short 0x1de51f", "xor al,al",
                                          "mov cl,[esp+0x24]", "shl al,byte 0x6", "xor al,cl", "and al,0x40",
                                          "xor cl,al", "mov esi,0x9f8e18", "mov eax,ebx", "mov [esp+0x24],cl"]
          and rows(stock, 0x1DE6D1, 0xA) == ["push eax", "push ebx", "mov [esp+0x28],cl", "lea ecx,[esp+0x20]"]
          and rows(stock, 0x1E0812, 0x24) == ["mov esi,[ebp+0x8]", "push dword 0x9f975c", "push esi",
                                              "lea ecx,[esp+0x48]", "call 0x1a8900", "push dword 0x9f9778",
                                              "push esi", "lea ecx,[esp+0xa8]", "call 0x1a8900"],
          "g_stage's compile reads use_uv_scrolling into bit 6 of the configuration's byte 0Ch ([esp+24h], the "
          "configuration being [esp+18h]: `lea ecx,[esp+20h]` after two pushes) and sends uv_scrolling_parameters0/1 "
          "with 0x1A8900 and [ebp+8] as the compiler")
    fill = forward_fill_offset(stock)
    check(fill == 0x0C, f"fill_shader_configuration_macros (0x3C9B80) makes CONFIG_USE_UV_SCROLLING of bit 6 of "
          f"the configuration's byte {fill if fill is None else hex(fill)} (0x3C9F51..0x3C9F5F)")
    # The constructor around its hook: scene_time's binding, the last of them.
    check(rows(stock, 0x1498A6, 0x21) == ["lea eax,[ebx+0x4140]", "mov esi,0x4", "push dword 0x9efd64",
                                          "mov [esp+0x14],eax", "mov [esp+0x18],esi", "call 0xe0b50",
                                          "mov [esp+0x18],edi"]
          and rows(stock, 0x1498D8, 0x18) == ["mov edx,[dword 0x226047c]", "push edx", "lea eax,[esp+0x14]",
                                              "mov [esp+0x20],edi", "mov [esp+0x24],esi", "call 0x3bccc0"]
          and rows(stock, 0x1498F6, 0x19) == ["mov eax,[esp+0x18]", "cmp eax,edi", "jz 0x14990f", "mov ecx,eax",
                                              "or edx,byte -0x1", "lock xadd [ecx],edx", "jnz 0x14990f", "push eax",
                                              "call 0xe0ae0"]
          and rows(stock, 0x149919, 0x10) == ["lea esp,[esp+0x0]", "mov eax,[esp+0x2c]", "call 0x162550"]
          and image.read(0x149919, 0x40) == stock.read(0x149919, 0x40)
          and stock.read(0x9EFD64, 11) == b"scene_time\0",
          "0x14990F: after scene_time's binding (its float at [ebx+4140h], 4 bytes, the name, 0, 4; then "
          "register_constant_binding 0x3BCCC0 with the resource manager [226047Ch], then the name let go), esi "
          "written again by the second instruction, the loop after it the original's")
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    sites = sorted(P.TEXT_VA + at for at in range(len(text) - 5) if text[at] == 0xE8
                   and P.TEXT_VA + at + 5 + struct.unpack_from("<i", text, at + 1)[0] == 0x3BCCC0)
    inside = [s for s in sites if RENDERER_CONTEXT[0] <= s < RENDERER_CONTEXT[1]]
    check(len(sites) == 20 and len(inside) == 19 and inside[-1] == 0x1498EB,
          f"register_constant_binding has {len(sites)} callers, {len(inside)} in renderer_context's constructor; "
          f"the last of those ({inside[-1] if inside else 0:#x}) is scene_time's, right before the hook")
    for va, old, _, _ in P.FORWARD_SCROLL_HOOKS:
        start, end = RENDERER_CONTEXT if va == 0x14990F else FORWARD_COMPILE
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
        found = aimed_inside(original, va, len(old), 0)
        found += [address for address, _, t in ndisasm(stock.read(start, end - start), start)
                  if (target := branch_target(t)) is not None and va < target < va + len(old)]
        check(not under and not found, f"{va:#x}: no base relocation under it, no branch or address aims inside it"
              + "".join(f"\n          {a:#x}" for a in found))
    # The code itself.
    code = open(P.forward_scroll_bin(), "rb").read()
    base = P.FORWARD_SCROLL_VA
    check(image.read(base, len(code)) == code,
          f"the forward effect's scrolling at {base:#x} is forward_scroll_026g.section.bin ({len(code)} bytes)")
    one = code.index(struct.pack("<f", 1.0) + b"transparency_map_scroll_u_direction\0")
    tail = code[one:]
    check(tail == struct.pack("<f", 1.0) + b"transparency_map_scroll_u_direction\0transparency_map_scroll_v_direction\0"
          b"scene_slomo\0" + struct.pack("<I", base) and one % 4 == 0,
          "its data: the float 1.0 (scene_slomo's value), the names transparency_map_scroll_u_direction, "
          "transparency_map_scroll_v_direction and scene_slomo, and the address it was assembled for")
    listing = ndisasm(code[:one], base)
    at = {address: t for address, _, t in listing}
    end = base + len(code)
    calls = sorted({branch_target(t) for _, _, t in listing if t.startswith("call")} - set(range(base, end)))
    jumps = sorted({branch_target(t) for _, _, t in listing if t.startswith("j")} - set(range(base, end)))
    data = sorted({value for _, _, t in listing if (m := re.search(r"\[ebp\+0x([0-9a-f]+)\]", t))
                   and (value := int(m.group(1), 16)) >= P.IMAGE_BASE})
    exe_data = [d for d in data if not base <= d < end]
    own = [d for d in data if base <= d < end]
    texts_ok = all(original[stock.offset(va):stock.offset(va) + len(t) + 1] == t + b"\0"
                   for va, t in FORWARD_EXE_TEXTS.items())
    check(calls == sorted([0xF6C00, 0xF6B10, 0x1A8900, 0xE0B50, 0xE0AE0, 0x3BCCC0])
          and jumps == sorted(returns.values())
          and exe_data == sorted(list(FORWARD_EXE_TEXTS) + [FORWARD_RESOURCE_MANAGER]) and texts_ok
          and own == sorted([base + one, base + one + 4, base + one + 40, base + one + 76]),
          "its only ways out are the config's key check (0xF6C00) and lookup (0xF6B10), set_constant<float4> "
          "(0x1A8900), the shared string (0xE0B50) and its destroy (0xE0AE0), register_constant_binding "
          "(0x3BCCC0), and a jump back after each hook; it reads the exe's texts "
          + ", ".join(t.decode() for t in FORWARD_EXE_TEXTS.values())
          + " and the resource manager's pointer (0x226047C), and its own data, each plus the delta in ebp"
          + ("" if calls == sorted([0xF6C00, 0xF6B10, 0x1A8900, 0xE0B50, 0xE0AE0, 0x3BCCC0]) else f": {[hex(c) for c in calls]}")
          + ("" if exe_data == sorted(list(FORWARD_EXE_TEXTS) + [FORWARD_RESOURCE_MANAGER]) else f": {[hex(d) for d in exe_data]}"))
    entries_ok = True
    for va, old, offset, _ in P.FORWARD_SCROLL_HOOKS:
        entry = base + offset
        body = [t for address, _, t in listing if entry <= address]
        stop = next(i for i, t in enumerate(body) if t.startswith("jmp") and branch_target(t) in returns.values())
        entries_ok &= body[stop] == f"jmp {returns[entry]:#x}" and body[stop - 2:stop] == texts[va]
        entries_ok &= offset == 0 or code[offset - 1] == 0xCC
    check(entries_ok and at.get(base + P.SCENE_SLOMO_OFFSET) == "pusha"
          and at.get(base + P.FORWARD_SWITCH_OFFSET) == "test byte [esp+0x54],0x10"
          and at.get(base + P.FORWARD_CONSTANTS_OFFSET) == "test byte [esp+0x5c],0x40",
          "each entry ends with the two instructions its jump replaced and a jump back to the instruction after "
          "them; the switch's entry tests use_sequence (byte 4 bit 4), the constants' entry the bit it sets (byte "
          "0Ch bit 6), and the binding's entry starts with pushad")
    check(sum(1 for t in at.values() if t == "pusha") == sum(1 for t in at.values() if t == "popa") == 3,
          "three pushad and three popad: every register is restored before each jump back")


def forward_fill_offset(stock):
    """Which byte of the shader configuration fill_shader_configuration_macros
    reads at 0x3C9F51 for CONFIG_USE_UV_SCROLLING (bit 6): the stack followed
    from its start (add_bool_macro 0x3C9670 and the helpers 0x3C9730 and
    0x3C97D0 are cdecl), or None."""
    rows = ndisasm(stock.read(0x3C9B80, 0x400), 0x3C9B80)
    depth = 0
    for address, _, text in rows:
        if address == 0x3C9F51:
            following = [t for a, _, t in rows if 0x3C9F51 < a <= 0x3C9F5F]
            operand = re.search(r"\[esp\+0x([0-9a-f]+)\]", text)
            if not operand or following != ["shr cl,byte 0x6", "and cl,0x1", "movzx edx,cl", "push edx",
                                            "push dword 0xa27ddc"]:
                return None
            name = stock.read(0xA27DDC, 24)
            return int(operand.group(1), 16) - depth - 4 if name == b"CONFIG_USE_UV_SCROLLING\0" else None
        if text.startswith("push"):
            depth += 4
        elif text.startswith("pop"):
            depth -= 4
        elif m := re.match(r"(add|sub) esp,(?:byte \+)?0x([0-9a-f]+)$", text):
            depth += -int(m.group(2), 16) if m.group(1) == "add" else int(m.group(2), 16)
        elif text.startswith("call") and branch_target(text) not in (0x3C9670, 0x3C9730, 0x3C97D0):
            return None                                     # the three macro helpers end with a plain `ret`
    return None


def test_replay_reattach(original, image, before):
    """A replay's camera follows its player into another kit (patch
    revision 11, REPLAY_REATTACH): replay_network_client's slot +0x54 is
    a thunk to is_player_current 0x242A40 in that function's int3 padding;
    the build without the late join keeps both."""
    stock = Image(original)
    va, old, new = P.REPLAY_REATTACH_THUNK
    slot, slot_old, slot_new = P.REPLAY_REATTACH_SLOT
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(old)) == new,
          f"{va:#x}: seven int3 in the original and the build without the late join, the thunk in this one")
    rows = [text for _, _, text in ndisasm(image.read(va, len(new)), va)]
    check(rows == ["mov eax,ecx", "jmp 0x242a40"], f"{va:#x}: `mov eax,ecx` / `jmp 0x242a40`: {rows}")
    target = [text for _, _, text in ndisasm(stock.read(0x242A40, 0x29), 0x242A40)]
    check(target == ["mov eax,[eax+0x4]", "test eax,eax", "jz 0x242a64", "mov ecx,0x119190", "test ecx,ecx",
                     "jz 0x242a64", "mov dl,[eax+0x114]", "cmp dl,[esp+0x4]", "jnz 0x242a64", "mov eax,0x1",
                     "ret 0x4", "xor eax,eax", "ret 0x4"],
          "is_player_current 0x242A40: the client in eax, the attached player at +4, its place (+0x114) against "
          "the one pushed, `ret 4`; the padding after it is the thunk's")
    # Nothing aims into the padding: rel8 and rel32 branch bytes of .text,
    # and 4-byte values anywhere in the file.
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    inside = lambda target: va <= target < va + len(old)
    hits = []
    for at in range(va - P.TEXT_VA - 0x90, va - P.TEXT_VA + len(old) + 0x90):
        op = text[at]
        if (op == 0xEB or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3) and inside(P.TEXT_VA + at + 2 + struct.unpack_from("<b", text, at + 1)[0]):
            hits.append(f"{P.TEXT_VA + at:#x} rel8")
    for opcode, size in ((bytes([0xE8]), 5), (bytes([0xE9]), 5), (bytes([0x0F]), 6)):
        at = text.find(opcode)
        while at != -1:
            if at + size <= len(text) and (size == 5 or 0x80 <= text[at + 1] <= 0x8F):
                target = (P.TEXT_VA + at + size + struct.unpack_from("<i", text, at + size - 4)[0]) & 0xFFFFFFFF
                if inside(target):
                    hits.append(f"{P.TEXT_VA + at:#x} rel32")
            at = text.find(opcode, at + 1)
    for target in range(va, va + len(old)):
        if original.find(struct.pack("<I", target)) != -1:
            hits.append(f"{target:#x} as a value")
    check(not hits, "no branch bytes or pointer values aim into the padding" + "".join(", " + h for h in hits))
    check(stock.read(slot, 4) == slot_old and before.read(slot, 4) == slot_old and image.read(slot, 4) == slot_new,
          f"{slot:#x}: 0x54e10 (`xor al,al` / `ret 4`) in the original and the build without, {va:#x} in this one")
    check([text for _, _, text in ndisasm(stock.read(0x54E10, 5), 0x54E10)] == ["xor al,al", "ret 0x4"],
          "0x54E10 is `xor al,al` / `ret 4`")
    check(rtti_name(stock, 0x9FF344)[0] == ".?AVreplay_network_client@survarium@@",
          "the vtable at 0x9FF344 is replay_network_client's")
    relocated = lambda img: [rva for kind, rva in img.relocations().values() if kind == 3 and rva == slot]
    check(relocated(stock) and relocated(image), f"the slot {slot:#x} keeps its base relocation")
    under = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not under, "no base relocation under the thunk")


def branches_into(original, lo, hi):
    """Where .text's rel8/rel32 branch bytes, or 4-byte values anywhere in
    the file, aim into lo..hi-1."""
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    inside = lambda target: lo <= target < hi
    hits = []
    for at in range(lo - P.TEXT_VA - 0x90, hi - P.TEXT_VA + 0x90):
        op = text[at]
        if (op == 0xEB or 0x70 <= op <= 0x7F or 0xE0 <= op <= 0xE3) and inside(P.TEXT_VA + at + 2 + struct.unpack_from("<b", text, at + 1)[0]):
            hits.append(f"{P.TEXT_VA + at:#x} rel8")
    for opcode, size in ((bytes([0xE8]), 5), (bytes([0xE9]), 5), (bytes([0x0F]), 6)):
        at = text.find(opcode)
        while at != -1:
            if at + size <= len(text) and (size == 5 or 0x80 <= text[at + 1] <= 0x8F):
                target = (P.TEXT_VA + at + size + struct.unpack_from("<i", text, at + size - 4)[0]) & 0xFFFFFFFF
                if inside(target):
                    hits.append(f"{P.TEXT_VA + at:#x} rel32")
            at = text.find(opcode, at + 1)
    for target in range(lo, hi):
        if original.find(struct.pack("<I", target)) != -1:
            hits.append(f"{target:#x} as a value")
    return hits


def test_sound_queue(original, image, before):
    """A weapon's full sound queue drops the new sound (patch revision 11,
    SOUND_QUEUE): delayed_callbacks_queue::add 0x2454A0 jumps after its
    drain to a cave in int3 padding, which goes to the game's own drop exit
    when the queue is full; the build without the late join keeps both."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    for va, old, new in (P.SOUND_QUEUE_HOOK, P.SOUND_QUEUE_CAVE_SITE):
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old and image.read(va, len(new)) == new,
              f"{va:#x}: the original's bytes in the original and the build without the late join, the new ones in this one")
    check(rows(image, P.SOUND_QUEUE_HOOK_VA, 5) == ["jmp 0x835a20"], "0x2454C1: `jmp 0x835a20`")
    check(rows(image, P.SOUND_QUEUE_CAVE_VA, len(P.SOUND_QUEUE_CAVE)) ==
          ["mov ecx,[esi+0x4]", "cmp ecx,[esi+0x8]", "jnc 0x835a2f", "mov eax,[esi]", "jmp 0x2454c6", "jmp 0x24569a"],
          "the cave: the queue's end against its capacity end; full -> 0x24569A, else the two instructions and back")
    start, size = P.SOUND_QUEUE_PADDING
    run = stock.read(start - 1, size + 2)
    check(run[0] == 0xC3 and run[1:-1] == bytes.fromhex("cc") * size and run[-1] == 0x55
          and start <= P.SOUND_QUEUE_CAVE_VA and P.SOUND_QUEUE_CAVE_VA + len(P.SOUND_QUEUE_CAVE) <= start + size,
          "the cave lies in 42 int3 between a `ret` and a `push ebp`")
    check(rows(stock, 0x2454B0, 0x18) == ["mov esi,[esp+0x19c]", "push edi", "push ebx", "push esi", "mov edi,eax",
                                          "call 0x245860", "mov eax,[esi]", "mov ecx,[esi+0x4]", "xor edx,edx"],
          "add: esi = the queue, its drain (which takes its two arguments off), then the queue's begin and end")
    between = rows(stock, 0x2454C6, 0x2454F7 - 0x2454C6)
    check(between[-1] == "jnz near 0x245691" and not [r for r in between if ("push" in r or "pop" in r or "esp," in r) and r != "lea esp,[esp+0x0]"],
          "no push, pop or esp change (but a `lea esp,[esp+0]` filler) from 0x2454C6 to the chain branch: 0x245691 and 0x24569A have the hook's esp")
    check(rows(stock, 0x245691, 0x9) == ["cmp edx,[esp+0x1a4]", "jna 0x2456e1"],
          "0x245691: chain length against the cap; past it falls into 0x24569A")
    check(rows(stock, 0x24569A, 0xC) == ["mov edi,[esp+0x1ac]", "cmp dword [edi],byte +0x0", "jnz 0x2456c0"]
          and rows(stock, 0x2456C0, 0x14) == ["mov eax,[edi]", "push byte +0x0", "and eax,byte -0x2",
                                              "mov ecx,[eax+0x4]", "push ebx", "add edi,byte +0x8", "push edi",
                                              "call ecx", "add esp,byte +0xc"],
          "0x24569A: the functor from the frame, called with flag 0")
    check(rows(stock, 0x2456D4, 0xD) == ["pop edi", "pop esi", "pop ebp", "pop ebx", "add esp,0x18c", "ret 0x18"],
          "then add's own epilogue, `ret 18h`")
    check(rows(stock, 0x297E20, 0xD) == ["push ebp", "mov ebp,esp", "and esp,byte -0x8", "sub esp,byte +0x74",
                                         "cmp byte [ebp+0x14],0x0"],
          "the sound player 0x297E20 looks at its flag first")
    check(rows(stock, 0x245AD5, 0x9) == ["mov eax,[ebx+0x4]", "push esi", "cmp eax,[ebx+0x8]", "jc 0x245b33"],
          "push_back 0x245AD0 asserts on the same two fields, end against capacity end")
    hits = branches_into(original, P.SOUND_QUEUE_HOOK_VA + 1, P.SOUND_QUEUE_HOOK_VA + 5) + \
        branches_into(original, start, start + size)
    check(not hits, "no branch bytes or pointer values aim inside the hook or into the padding" + "".join(", " + h for h in hits))
    for va, old, _ in (P.SOUND_QUEUE_HOOK, P.SOUND_QUEUE_CAVE_SITE):
        under = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
        check(not under, f"no base relocation under {va:#x}")


def test_seek_clear(original, image, before):
    """A replay's user seek empties every client weapon's delayed sounds
    (patch revision 11, SEEK_CLEAR_HOOK): the site, that only the replay
    reaches it, and seek_clear as the image has it. check_late_join_026g.py
    runs it on made-up places, kits and weapons."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    va, old, offset, _ = P.SEEK_CLEAR_HOOK
    cave = P.SEEK_CLEAR_VA + offset
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
          and rows(image, va, 6) == [f"jmp {cave:#x}", "nop"],
          f"{va:#x}: `lea eax,[edi-0x3e8]` in the original and the build without the late join, `jmp {cave:#x}` here")
    check(rows(stock, 0x1EE2E0, 6) == ["mov eax,[esi+0x53c8]"],
          "the seek 0x1EE2E0 starts from the replay reader [game+53C8h]")
    check(rows(stock, 0x1EE38B, 0x2A) == ["call 0x1ee4a0", "push edi", "push esi", "call 0x1edef0", "push edi",
                                          "mov eax,esi", "call 0x1ee4a0", "test bl,bl", "jnz 0x1ee3b5",
                                          "lea eax,[edi-0x3e8]", "mov [esi+0x4ec],eax", "mov [esi+0x57c],eax"],
          "set_time_in_ms(key frame), the re-simulation, set_time_in_ms(target), then a user seek's own part")
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    callers = []
    for opcode in (0xE8, 0xE9):
        at = text.find(bytes([opcode]))
        while at != -1:
            if at + 5 <= len(text) and (P.TEXT_VA + at + 5 + struct.unpack_from("<i", text, at + 1)[0]) & 0xFFFFFFFF == 0x1EE2E0:
                callers.append(P.TEXT_VA + at)
            at = text.find(bytes([opcode]), at + 1)
    check(sorted(callers) == [0x1EF8C0, 0x233E2F],
          "the seek's only callers: the replay seek functor 0x1EF8C0 and the replay movie's slider 0x233E2F")
    check(original.count(struct.pack("<I", 0xA0422C)) == 2 and rows(stock, 0x24365C, 7) == ["mov dword [ebx+0x10],0xa0422c"],
          "0xA0422C, the client weapon's item vtable, is written by its constructor (and destructor) alone")
    check(rows(stock, 0x2438BC, 0x1A)[:6] == ["mov eax,[esi+0x9fc]", "lea edi,[esi+0xa00]", "push edi",
                                              "call 0x245c50", "mov edx,[esi+0x9fc]", "mov [edi],edx"],
          "~weapon empties the queue as seek_clear does: 0x245C50 on begin..&end, then end = begin")
    hits = branches_into(original, va + 1, va + len(old))
    check(not hits, "no branch bytes or pointer values aim inside the hook" + "".join(", " + h for h in hits))
    under = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not under, f"no base relocation under {va:#x}")
    code = rows(image, cave, 0x97)
    check(code[:3] == ["pusha", f"call {cave + 6:#x}", "pop ebp"]
          and code[3] == f"add ebp,{(0xA0422C - (cave + 6)) & 0xFFFFFFFF:#x}"
          and "call 0x245c50" in code and code[-3:] == ["popa", "lea eax,[edi-0x3e8]", "jmp 0x1ee3a9"],
          "seek_clear: the vtable found relative to itself, 0x245C50 for the functors, the lea and back to 0x1EE3A9")


# The hud's update of the local player's state (0x23B740): its body.
UPDATE_PLAYER_STATE = (0x23B740, 0x23B81A)
RESPAWN_TICK = (0x316820, 0x316A24)
KIT_VIEW = (0x2678E0, 0x26797F)                      # the client's view of a spawn into another kit
RESPAWN_VTABLES = (0xA16014, 0xA16084, 0xA06D1C)    # the core's, the server's, the client's (+50h, +5Ch)
# (address, the original's bytes, what they are) that the order of a tick and
# the camera's state rest on.
KIT_CHANGE_ORDER = (
    (0x316A3A, "8b90b08f01008b1289912c010000", "the respawn rule's key: the core's counter"),
    (0x316A50, "81c11c010000c6411400ff8040880100", "the respawn rule's node into tick list 0 (core+18840h)"),
    (0x318070, "81c110010000c6411401ff8050880100", "the time rule's node into tick list 1 (core+18850h)"),
    (0x321877, "8b88b08f01008b115f895520", "a place's key at its bind: the core's counter"),
    (0x2D28F6, "8d48108b4028c6411400ff8040880100", "a place's node into tick list 0 when it enters"),
    (0x2D385A, "89742420", "the core's counter of keys is 0 when it binds its places"),
    (0x2D3B2B, "8bb5248801008bbd28880100", "the core gives its rules the core, after the places"),
    (0x2D1E3F, "8db1408801008d99b0880100", "the world's tick goes through lists 0 to 6"),
    (0x2D1E5C, "e8bffeffff83c610", "list by list, each sorted by key (0x2D1D20)"),
    (0x267A75, "c74004a07a2600", "the client's time rule ticks through 0x267AA0"),
    (0x267AAA, "e851060b00", "which steps the rule (logic_tick)"),
    (0x267AD2, "3bb1c48f0100762e", "and only for a tick first played"),
    (0x267AE6, "8bbf4801000039be601c1f007414", "compares its state with the camera's"),
    (0x267AFB, "89be601c1f007405e898360100", "keeps it and sets the camera (0x27B1A0)"),
    (0x27B1C8, "83be601c1f0003740c", "the player camera in play"),
    (0x27B1D8, "e9b3cff7ff", "the warmup's otherwise (0x1F8190)"),
    (0x27B1E3, "6a01e816d0f7ff", "first person in play"),
)


def test_kit_change(original, image, before):
    """Check 32: a kit chosen before the match starts is played at once."""
    stock = Image(original)
    texts = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    names = ("kit_change", "kit_view", "ui_alive", "ui_start")
    wanted = (["push ebp", "mov ebp,esp", "sub esp,byte +0x34"],
              ["push byte +0x1", "call 0x1f8200"],
              ["call 0x23a1a0", "push byte +0x0"],
              ["lea eax,[ebp+0xd0]"])
    for (va, old, offset, opcode), name, original_texts in zip(P.KIT_CHANGE_HOOKS, names, wanted):
        rows = ndisasm(image.read(va, len(old)), va)
        word = "call" if opcode == 0xE8 else "jmp"
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
              and texts(stock, va, len(old)) == original_texts
              and rows[0][2].startswith(word) and branch_target(rows[0][2]) == P.KIT_CHANGE_VA + offset
              and all(text == "nop" for _, _, text in rows[1:]) and len(rows) == len(old) - 4,
              f"{va:#x}: `{' / '.join(original_texts)}` is `{rows[0][2]}` + {len(rows) - 1} nop ({name}); "
              "the build without the late join keeps it")
    # The tick: every respawn rule's +50h, the spawn their +5Ch; the tick
    # reads the respawn times, takes a player in the world out with 1, and
    # spawns through +5Ch with (place, point, time). The view of a spawn
    # into another kit is the client's rule's +60h, which the spawn calls.
    slot = lambda vt, at: struct.unpack("<I", stock.read(vt + at, 4))[0]
    slots = [(slot(vt, 0x50), slot(vt, 0x5C)) for vt in RESPAWN_VTABLES]
    start, end = RESPAWN_TICK
    tick = texts(stock, start, end - start)
    check(slots == [(P.KIT_CHANGE_TICK_VA, 0x316DA0)] * 3 and slot(0xA06D1C, 0x60) == KIT_VIEW[0]
          and "cmp [edx],ecx" in tick and "mov edx,[eax+0x24]" in tick and "mov eax,[eax+0x5c]" in tick
          and tick[-1] == "ret 0xc",
          "the respawn rules' vtables (core, server, client) have the tick at +50h and the spawn at +5Ch, the "
          "client's the view of a kit change at +60h; the tick compares the respawn times, takes a player out "
          "through vtable+24h and spawns through +5Ch, `ret 0Ch`")
    spawn = texts(stock, 0x316DA0, 0x113)
    check("mov edi,[esp+0x1c]" in spawn and "lea eax,[edi+0x8]" in spawn and "fld dword [edi+0x14]" in spawn
          and "cmp dword [edi+0x20],byte +0x0" in spawn and "mov [eax+0x32c],ecx" in spawn
          and "mov [eax+0xc],dl" in spawn and "mov eax,[edx+0x60]" in spawn and spawn[-1] == "ret 0xc",
          "the spawn makes the next kit current, tells the view of a change of kit (+60h) and reads its point's "
          "+8 (the position), +14h (the yaw) and +20h (the protection, into the damage model's +32Ch), `ret 0Ch`")
    view = texts(stock, KIT_VIEW[0], KIT_VIEW[1] - KIT_VIEW[0])
    check(view[:3] == ["push ebx", "mov ebx,ecx", "mov eax,[ebx+0x1a8]"] and "mov esi,[eax+0x53ac]" in view
          and view.count("call 0x1f8200") == 1 and view[-1] == "ret 0x4"
          and not any(text.startswith("mov esi") for text in view[view.index("mov esi,[eax+0x53ac]") + 1:]),
          "the view of a kit change keeps the network client in esi to its end and gives the player camera once")
    states = [address for address, _, text in ndisasm(stock.read(0x317EE0, 0x480), 0x317EE0)
              if text.endswith("+0x18fe4],eax") or text.endswith("+0x18fe4],ecx") or text.endswith("+0x18fe4],esi")
              or text.endswith("+0x18fe4],edx")]
    check(len(states) >= 4, f"the time rule writes its state to core+18FE4h ({len(states)} stores: "
          "start, countdown, in process, deserialize)")
    # A pick: the profile_changed callback's hiding of the kits is nops.
    va, old, new = P.KIT_PICK
    check(image.read(va, len(old)) == new and stock.read(va, len(old)) == old and before.read(va, len(old)) == old,
          f"{va:#x}: `push 0` / `mov eax,ebx` / `call 0x2383b0` after a pick are nine nops: the kits stay shown and "
          "the hud's cursor where it is; the build without the late join keeps them")
    # The order of a tick and the camera's state. In the build the late
    # join binds each place with a counter of its own from (place+1) << 16
    # (bind_slot_reserved, at the core's bind 0x2D3880) and gives the core's
    # counter back after: the rules' keys come from it, from 0.
    wrong = [f"{va:#x} ({what})" for va, old, what in KIT_CHANGE_ORDER if stock.read(va, len(old) // 2).hex() != old]
    binding = texts(stock, 0x2D3880, 0x2D3B47 - 0x2D3880)
    late = open(P.late_join_bin(), "rb").read()[:P.LATE_JOIN_ASM_SIZE]
    own_keys = bytes.fromhex("0fb6462c40c1e0108945f08b83b08f01008945f48d45f08983b08f0100")
    given_back = bytes.fromhex("8b45f48983b08f0100")
    rows = ndisasm(image.read(0x2D3880, 5), 0x2D3880)
    check(not wrong and not any(text.startswith("ret") for text in binding)
          and late.count(own_keys) == 1 and late.find(given_back) > late.find(own_keys)
          and branch_target(rows[0][2]) == P.LATE_JOIN_VA,
          "a tick plays list 0 before the time rule's list 1; in the build list 0 has the respawn rule (its key from "
          "the core's counter, from 0) before the places (keys (place+1) << 16, the late join's bind_slot, which "
          "gives the counter back): a kit choice for a tick is played by the respawn rule at the next one; the "
          "client's time rule sets the camera, the player's in play (state 3) and the warmup's otherwise, when its "
          "state changes in a tick first played, and keeps that state at network client+1F1C60h"
          + "".join(f"\n          {w}" for w in wrong))
    text = bytes(original[P.file_offset(P.TEXT_VA):P.file_offset(P.TEXT_VA) + P.TEXT_VSIZE])
    callers = [P.TEXT_VA + at for at in range(len(text) - 5)
               if text[at] == 0xE8 and P.TEXT_VA + at + 5 + struct.unpack_from("<i", text, at + 1)[0] == 0x2383B0]
    selector = struct.pack("<I", 0xA03704)                     # "root.show_profiles_selector"
    users = [P.TEXT_VA + at for at in range(len(text) - 4) if text[at:at + 4] == selector]
    pick = texts(stock, 0x233F06, 0x31)
    check(callers == [0x233F2C, 0x23B7F7] and users == [0x238451] and stock.read(0xA03704, 28) == b"root.show_profiles_selector\0"
          and stock.read(0x233F07, 4) == struct.pack("<I", 0xA03280) and stock.read(0xA03280, 16) == b"profile_changed\0"
          and pick[pick.index("call 0x1f8070"):] == ["call 0x1f8070", "push byte +0x0", "mov eax,ebx", "call 0x2383b0",
                                                    "pop edi", "pop esi", "pop ebx", "ret 0x10"],
          "the kits are shown or hidden by 0x2383B0 alone (root.show_profiles_selector, at 0x238451), which the "
          "profile_changed callback (0x233F2C, to hide them after it queued the pick, 0x1F8070) and this update "
          "(0x23B7F7) call, nothing else")
    code = open(P.kit_change_bin(), "rb").read()
    check(image.read(P.KIT_CHANGE_VA, len(code)) == code,
          f"the code at {P.KIT_CHANGE_VA:#x} is kit_change_026g.section.bin ({len(code)} bytes)")
    rows = [row for row in ndisasm(code, P.KIT_CHANGE_VA) if row[2] != "int3"]
    inside = lambda target: P.KIT_CHANGE_VA <= target < P.KIT_CHANGE_VA + len(code)
    calls = sorted({text for _, _, text in rows if text.startswith("call")
                    and not ((target := branch_target(text)) is not None and inside(target))})
    leaves = sorted({target for _, _, text in rows if (target := branch_target(text)) is not None
                     and not inside(target)})
    stores = sorted({text for _, _, text in rows if re.match(r"^mov \[", text)})
    check(calls == sorted({"call 0x1f8200", "call 0x23a1a0", "call 0x2383b0", "call [edx+0x8]",
                           "call [eax+0x24]", "call [eax+0x5c]"})
          and leaves == [0x1F8190, 0x1F8200, 0x2383B0, 0x23A1A0, 0x23B7FC, 0x316826] and stores == [],
          "outside itself it calls only the damage model's holder (+108h vtable+8), the player's way out of the "
          "world (vtable+24h), the rule's spawn (vtable+5Ch), the player camera, 0x23A1A0 and the kits' show or "
          "hide (0x2383B0); it leaves only to 0x316826, 0x23B7FC and the warmup camera and by its returns; it "
          "stores nothing but on its stack"
          + "".join(f"\n          {text}" for text in calls + [hex(t) for t in leaves] + stores))
    # Nothing aims inside the four sites; no relocation is under them. A
    # pointer of the image has a base relocation: the four bytes of the
    # file that happen to spell an address inside a site (0x23B800 and
    # 0x23B801 are in .text's code) are not pointers unless one is there.
    found = []
    sites = [(va, old) for va, old, _, _ in P.KIT_CHANGE_HOOKS] + [P.KIT_PICK[:2]]
    pointers = {}
    for kind, at in stock.relocations().values():
        if kind == 3:
            try:
                pointers[at] = struct.unpack("<I", stock.read(at, 4))[0]
            except ValueError:                     # past a section's file data: zero
                pass
    for va, old in [(va, old) for va, old, _, _ in P.KIT_CHANGE_HOOKS] + [P.KIT_PICK[:2]]:
        hits = aimed_inside(original, va, len(old), 0)
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        found += hits[:len(hits) - len(spelled)]
        found += [at for at, value in pointers.items() if va < value < va + len(old)]
    for lo, hi in (RESPAWN_TICK, UPDATE_PLAYER_STATE, KIT_VIEW, (0x233E80, 0x233F37)):
        found += [address for address, _, text in ndisasm(stock.read(lo, hi - lo), lo)
                  if (target := branch_target(text)) is not None
                  and any(va < target < va + len(old) for va, old in sites)]
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(va - 4 < v < va + len(old) for va, old in sites)]
    check(not found and not under, "no branch or address aims inside the five sites, no base relocation under them"
          + "".join(f"\n          {a:#x}" for a in found + under))


def test_warmup_look(original, image, before):
    """Check 32, the warmup camera before the start (WARMUP_LOOK_*,
    WARMUP_LIMITS_*)."""
    stock = Image(original)
    texts = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    slot = lambda vt, at: struct.unpack("<I", stock.read(vt + at, 4))[0]
    floats = lambda va: struct.unpack("<f", stock.read(va, 4))[0]
    sites = []
    # The mouse move's jump goes to patch 46's death_look first, which goes
    # on to warmup_look (check 46).
    death_look = P.death_camera_va(open(P.late_join_bin(), "rb").read()) + P.DEATH_LOOK_OFFSET
    for (va, old, offset, opcode), base, original_texts, built in (
            (P.WARMUP_LOOK_HOOK, death_look, ["push ebx", "push esi", "mov esi,ecx", "mov ecx,[esi+0x4b70]"],
             [f"jmp {death_look:#x}"] + ["nop"] * 5),
            (P.WARMUP_LIMITS_HOOK, P.WARMUP_LIMITS_VA, ["movss dword [esi+0x350],xmm0"],
             [f"call {P.WARMUP_LIMITS_VA + P.WARMUP_LIMITS_ENTRY_OFFSET:#x}"] + ["nop"] * 3)):
        new = P.hook_bytes(base, va, old, offset, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              and texts(stock, va, len(old)) == original_texts and texts(image, va, len(old)) == built,
              f"{va:#x}: `{' / '.join(original_texts)}` in the original and the build without the late join, "
              f"`{built[0]}` and {len(built) - 1} nops here")
        sites.append((va, old))
    move = texts(stock, 0x25D0E0, 0x25D128 - 0x25D0E0)
    check("mov edx,[eax]" in move and "lea ecx,[esi-0x1f8]" in move and "mov eax,[eax+0x30]" in move
          and move[-6:] == ["cmp byte [esi+0x2e0],0x0", "pop edi", "pop esi", "setnz al", "pop ebx", "ret 0x10"]
          and stock.read(0x25D0EA, 2) == bytes.fromhex("8b01"),
          "the game world's on_mouse_move (0x25D0E0) takes the system's cursor ([esi+4B70h] vtable+0), puts the "
          "hud's pointer there (game world vtable+30h) and answers whether the cursor is shown (hud+2DCh), "
          "`ret 10h`; 0x25D0EA, where warmup_look goes on, begins an instruction")
    check(slot(0x9FC588, 0xC) == 0x25D0E0 and slot(0x9FC588, 0x10) == 0x1F63D0
          and stock.read(0x1F63D0, 6) == bytes.fromhex("b807000000c3")
          and slot(0xA04048, 0xC) == 0x240200 and slot(0xA04048, 0x10) == 0x283BB0
          and stock.read(0x283BB0, 6) == bytes.fromhex("b80a000000c3")
          and stock.read(0x1F683B, 10) == bytes.fromhex("c785f801000088c59f00")
          and stock.read(0x1F6821, 6) == bytes.fromhex("8db5fc010000")
          and slot(0x9FC4F4, 0x30) == 0x25D160 and slot(0x9FC4F4, 0x3C) == 0x25D130
          and "mov [ebx],eax" in texts(stock, 0x25D160, 0x30) and "lea ebx,[edi+0x8c]" in texts(stock, 0x25D160, 0x30)
          and "call 0x29a680" in texts(stock, 0x25D130, 0x28) and "mov edx,[eax+0x30]" in texts(stock, 0x25D130, 0x28),
          "the game world's input handler (+1F8h, vtable 0x9FC588; the hud at +1FCh) has on_mouse_move 0x25D0E0 "
          "and priority 7, the player's input handler (vtable 0xA04048) on_mouse_move 0x240200 and priority 10; "
          "the game world's vtable+30h (0x25D160) keeps the hud's pointer at +8Ch, +90h and moves it, its +3Ch "
          "(0x25D130) puts the system's cursor (0x29A680) and the pointer (+30h) at a place")
    dispatch = texts(stock, 0x3B9F10, 0x3B9F34 - 0x3B9F10)
    order = texts(stock, 0x3B8023, 0x3B804A - 0x3B8023)
    buttons = texts(stock, 0x3B9610, 0x3B9620)
    check(dispatch[:4] == ["mov edx,[esi+0xc]", "mov ecx,[edi]", "mov eax,[ecx]", "mov eax,[eax+0xc]"]
          and dispatch[-6:] == ["call eax", "test al,al", "jnz 0x3b9f34", "add edi,byte +0x4", "cmp edi,ebp",
                                "jnz 0x3b9f10"]
          and "mov edx,[esi+0x8]" in dispatch and "mov edx,[esi+0x4]" in dispatch
          and "mov edx,[esi+0x2c]" in dispatch
          and "mov eax,[edx+0x10]" in order and "cmp eax,ebp" in order and "jnl 0x3b8048" in order
          and stock.read(0x3B7E1F, 5) == bytes.fromhex("e8ec010000")
          and slot(0xA252D4, 0x28) == 0x3B7FB0 and stock.read(0x3B7FB0, 4) == bytes.fromhex("8b4128c3")
          and slot(0xA25518, 0) == 0x94370 and stock.read(0x94370, 4) == bytes.fromhex("8d4104c3")
          and buttons[:3] == ["mov byte [ecx+0xc],0x0", "cmp byte [eax+0xc],0x0", "jz 0x3b961e"]
          and stock.read(0x3B961A, 4) == bytes.fromhex("80490c01"),
          "the input world gives a mouse move (the mouse's +4, +8, +0Ch; +2Ch the world) to its handlers in their "
          "order (vtable+0Ch) until one answers 1 (0x3B9F10); add_handler (0x3B7D60) inserts a handler by its "
          "priority (vtable+10h), lowest first (0x3B8010); the world's mouse is its +28h (vtable+28h, 0x3B7FB0), "
          "its state its +4 (vtable+0, 0x94370), the buttons held at +10h, the left one bit 0 (0x3B9610)")
    lobby = texts(stock, 0x283D30, 0x283EAA - 0x283D30)
    check(slot(0xA09EB4, 0xC) == 0x283D30 and slot(0xA09EB4, 0x10) == 0x283BB0
          and stock.read(0x21E9A0, 6) == bytes.fromhex("b864000000c3") and slot(0x9FFEB8, 0x10) == 0x21E9A0
          and "mov edx,[eax+0x28]" in lobby and "test byte [eax+0xc],0x52" in lobby
          and "mulss xmm0,dword [dword 0x91a8f0]" in lobby and "call 0x29a680" in lobby
          and lobby[-1] == "ret 0x10" and floats(0x91A8F0) == struct.unpack("<f", struct.pack("<f", 0.001))[0],
          "the lobby camera's on_mouse_move (0x283D30, vtable 0xA09EB4, priority 10, before the lobby menu's "
          "100) moves its distance by 0.001 a unit of the wheel and turns while a button of 52h is held (the "
          "mouse's state +0Ch), putting the system's cursor back at the drag's end (0x29A680)")
    tick = texts(stock, 0x283EF0, 0x284340 - 0x283EF0)
    check("movss xmm1,dword [dword 0xa7afa4]" in tick and "movss xmm2,dword [dword 0xa2db0c]" in tick
          and "movss xmm0,dword [dword 0xa2db08]" in tick and "movss xmm0,dword [dword 0x9c56c0]" in tick
          and [round(floats(va), 6) for va in (0xA7AFA4, 0xA2DB0C, 0xA2DB08, 0x9C56C0)]
          == [1.5, 4.6, -1.047198, 1.047198],
          "the lobby camera's tick (0x283EF0) keeps its distance within 1.5 and 4.6 and its pitch within "
          "+-1.047198 (60 degrees)")
    warmup = texts(stock, 0x1F8190, 0x1F81C7 - 0x1F8190)
    frame = texts(stock, 0x241170, 0x2411F9 - 0x241170)
    step = texts(stock, 0x242200, 0x2422A6 - 0x242200)
    show = texts(stock, 0x23A930, 0x23A9C8 - 0x23A930)
    kits = texts(stock, 0x2383B0, 0x23845B - 0x2383B0)
    after = step[step.index("movss dword [esi+0x350],xmm0") + 1:]
    check(warmup[0] == "mov eax,[edx+0x4d58]" and "mov esi,0x3" in warmup and "mov [eax+0x358],esi" in warmup
          and "mov eax,[esi+0x358]" in frame and "cmp eax,byte +0x3" in frame and "call 0x242200" in frame
          and step[:3] == ["push ecx", "movss xmm0,dword [esi+0x348]", "addss xmm0,dword [esi+0x354]"]
          and step.count("movss dword [esi+0x354],xmm0") == 1 and step.count("movss dword [esi+0x350],xmm0") == 1
          and after[:3] == ["xorps xmm0,xmm0", "movss dword [esp+0x4],xmm0", "mov eax,[esp+0x4]"]
          and "mov edx,[esp+0xc]" in after and not [t for t in after if "xmm1" in t] and after[-1] == "ret"
          and "mov edx,0x8f" in texts(stock, 0xF65A0, 0x50)
          and "cmp [edi+0x2dc],bl" in show and "mov [edi+0x2dc],bl" in show
          and "mov [edi+0x2df],bl" in kits and stock.read(0x23845A, 6) == bytes.fromhex("53e8d0240000"),
          "the warmup camera (0x1F8190) sets the player's input handler (game world+4D58h) to mode 3 (+358h), "
          "which its end of the frame (0x241170) plays as the warmup mode (0x242200): the distance (+354h) plus "
          "the wheel (+348h), stored once, then the pitch (+350h), both with no limit; eax and edx are written "
          "after the pitch's store before they are read (0xF65A0 too), xmm1 not used; show_cursor (0x23A930) "
          "keeps the cursor at hud+2DCh, the kits' show or hide (0x2383B0) the kits at hud+2DFh and the cursor "
          "with them")
    look = open(P.warmup_look_bin(), "rb").read()
    limits = open(P.warmup_limits_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_LOOK_STATE_OFFSET
    for code, base, calls, leaves, rets, name in (
            (look, P.WARMUP_LOOK_VA, ["call [eax+0x30]", f"call {P.WARMUP_LIMITS_VA:#x}"], [0x25D0EA], ["ret 0x10"],
             "warmup_look"),
            (limits, P.WARMUP_LIMITS_VA, ["call [eax+0x3c]"], [], ["ret", "ret"], "warmup_limits")):
        listing = ndisasm(code, base)
        inside = lambda target: base <= target < base + len(code)
        starts = {address for address, _, _ in listing}
        targets = [t for _, _, text in listing if (t := branch_target(text)) is not None and not text.startswith("call")]
        made = [text for address, _, text in listing if text.startswith("call")
                and not (branch_target(text) == address + 5 and text.startswith("call 0x"))]
        check(image.read(base, len(code)) == code
              and all(t in starts for t in targets if inside(t)) and [t for t in targets if not inside(t)] == leaves
              and made == calls and [text for _, _, text in listing if text.startswith("ret")] == rets
              and not [t for _, _, t in listing if re.search(r"\[(?:dword |byte )?0x", t)],
              f"the code at {base:#x} is {name}'s .bin ({len(code)} bytes): its branches land on its instructions"
              + (f" or leave to {', '.join(hex(t) for t in leaves)}" if leaves else "")
              + f", it calls {', '.join(calls)} alone and holds no address")
    check(P.warmup_look_state(look) == state and P.warmup_look_calls(look) == [P.WARMUP_LIMITS_VA]
          and P.WEAPON_PREVIEW_STATE_OFFSET + P.WEAPON_PREVIEW_STATE_SIZE <= P.WARMUP_LOOK_STATE_OFFSET
          and P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_LOOK_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"warmup_look keeps its state at {state:#x} (the late join's page +{P.WARMUP_LOOK_STATE_OFFSET:#x}, after "
          "the weapon preview's) and calls the end of a drag at warmup_limits' start")
    found = []
    for va, old in sites:
        hits = [h for h in aimed_inside(original, va, len(old), 0x80) if not va <= h < va + len(old)]
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        found += hits[:len(hits) - len(spelled)]
        found += [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    for lo, hi in ((0x25D0E0, 0x25D128), (0x242200, 0x2422A6)):
        found += [address for address, _, text in ndisasm(stock.read(lo, hi - lo), lo)
                  if (target := branch_target(text)) is not None and any(va < target < va + len(old) for va, old in sites)]
    check(not found, "nothing aims inside the two sites and no base relocation is under them"
          + "".join(f", {h:#x}" for h in found))


def test_warmup_keys(original, image, before):
    """Check 32 (continued): a mouse button held from before the start is
    not the player's until it is let go (WARMUP_KEYS_*).
    check_kit_change_026g.py runs the code on made-up events."""
    stock = Image(original)
    texts = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    slot = lambda vt, at: struct.unpack("<I", stock.read(vt + at, 4))[0]
    va, old, offset, opcode = P.WARMUP_KEYS_HOOK
    new = P.hook_bytes(P.WARMUP_KEYS_VA, va, old, offset, opcode)
    original_texts = ["push ebp", "push esi", "mov esi,ecx", "mov ecx,[esp+0xc]", "mov eax,[ecx]"]
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and texts(stock, va, len(old)) == original_texts
          and texts(image, va, len(old)) == [f"jmp {P.WARMUP_KEYS_VA:#x}"] + ["nop"] * 5,
          f"{va:#x}: `{' / '.join(original_texts)}` in the original and the build without the late join, "
          f"`jmp {P.WARMUP_KEYS_VA:#x}` and 5 nops here")
    key = texts(stock, 0x25CF60, 0x25D0E0 - 0x25CF60)
    check(slot(0x9FC588, 0x8) == 0x25CF60 and "cmp byte [esi+0x2e0],0x0" in key
          and key.count("mov al,0x1") == 3 and key.count("xor al,al") == 1 and key.count("ret 0xc") == 4
          and key.count("cmp byte [esi+0x2e0],0x0") == 2 and stock.read(0x25CF6A, 3) == bytes.fromhex("8b5024"),
          "the game world's input handler (vtable 0x9FC588) has on_mouse_key_action 0x25CF60 (vtable+8; the input "
          "world, the key, the kind on the stack, `ret 0Ch`), which takes the mouse's keys (1) while the cursor is "
          "shown (+2E0h); 0x25CF6A, where warmup_keys goes on, begins an instruction")
    process = texts(stock, 0x3B9D90, 0x3B9F10 - 0x3B9D90)
    codes = texts(stock, 0x3B9CAE, 0x3B9CDE - 0x3B9CAE)
    player = texts(stock, 0x23FF50, 0x240046 - 0x23FF50)
    step = texts(stock, 0x24128A, 0x2421B0 - 0x24128A)
    play = [step[i + 1] for i, text in enumerate(step) if text == "call 0x242460"]
    kinds = step.count("cmp dword [esp+0x44],byte +0x0")
    check("mov al,[esi+0x10]" in process and "mov dl,[esi+0x20]" in process
          and process.count("mov edx,[edx+0x8]") + process.count("mov eax,[eax+0x8]") >= 3
          and process.count("push byte +0x2") == 2 and process.count("push byte +0x1") == 1
          and process.count("push byte +0x0") == 1
          and codes[0] == "mov eax,0x151" and codes[-2] == "mov eax,0x158"
          and process.count("test al,al") >= 3 and "cmp al,0x1" not in process
          and slot(0xA04048, 0x8) == 0x23FF50 and player.count("call 0x242580") == 3
          and "lea ecx,[ebx+0x98]" in player
          and len(play) == 35 and all(after == "test al,al" for after in play) and kinds == 3,
          "the mouse's process (0x3B9D90; held now +10h, the frame before +20h) tells the handlers (vtable+8) of a "
          "button gone down (kind 0 and 2), let go (1) and still held (2, every frame) and takes any answer not 0 "
          "(`test al,al`); its codes are 151h..158h (0x3B9CA0); the player's input handler (vtable 0xA04048) "
          "queues each kind with the action at +98h (0x23FF50), and the play step (0x24128A..0x2421B0) asks "
          "action_present (0x242460) of 35 actions, only three of them for a press: the rest, the weapon's "
          "among them, are taken at a release too")
    code = open(P.warmup_keys_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WARMUP_KEYS_STATE_OFFSET
    listing = ndisasm(code, P.WARMUP_KEYS_VA)
    inside = lambda target: P.WARMUP_KEYS_VA <= target < P.WARMUP_KEYS_VA + len(code)
    starts = {address for address, _, _ in listing}
    targets = [t for _, _, text in listing if (t := branch_target(text)) is not None and not text.startswith("call")]
    calls = [text for address, _, text in listing if text.startswith("call")
             and not (branch_target(text) == address + 5 and text.startswith("call 0x"))]
    check(image.read(P.WARMUP_KEYS_VA, len(code)) == code
          and all(t in starts for t in targets if inside(t)) and [t for t in targets if not inside(t)] == [0x25CF6A]
          and not calls and [text for _, _, text in listing if text.startswith("ret")] == ["ret 0xc"]
          and not [t for _, _, t in listing if re.search(r"\[(?:dword |byte )?0x", t)]
          and P.warmup_keys_state(code) == state
          and P.WARMUP_LOOK_STATE_OFFSET + P.WARMUP_LOOK_STATE_SIZE <= P.WARMUP_KEYS_STATE_OFFSET
          and P.WARMUP_KEYS_STATE_OFFSET + P.WARMUP_KEYS_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE,
          f"the code at {P.WARMUP_KEYS_VA:#x} is warmup_keys' .bin ({len(code)} bytes): its branches land on its "
          f"instructions or leave to 0x25cf6a, it calls nothing, holds no address and keeps its state at {state:#x} "
          f"(the late join's page +{P.WARMUP_KEYS_STATE_OFFSET:#x}, after the warmup look's)")
    hits = [h for h in aimed_inside(original, va, len(old), 0x80) if not va <= h < va + len(old)]
    spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
    found = hits[:len(hits) - len(spelled)]
    found += [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    found += [address for address, _, text in ndisasm(stock.read(0x25CF60, 0x25D0E0 - 0x25CF60), 0x25CF60)
              if (target := branch_target(text)) is not None and va < target < va + len(old)]
    check(not found, "nothing aims inside the site and no base relocation is under it"
          + "".join(f", {h:#x}" for h in found))


def test_death_camera(original, image, before):
    """Check 46: on the death screen a drag turns the camera; the wheel does
    nothing (DEATH_*, death_camera_026g.asm in `.srpc`).
    check_death_camera_026g.py runs it on made-up moves and views."""
    stock = Image(original)
    texts = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    at = lambda img, va, size: {address: text for address, _, text in ndisasm(img.read(va, size), va)}
    late_join = open(P.late_join_bin(), "rb").read()
    death_va = P.death_camera_va(late_join)
    view_va = death_va + P.DEATH_VIEW_OFFSET
    state = P.late_join_state_va(late_join) + P.DEATH_CAMERA_STATE_OFFSET
    look_state = P.late_join_state_va(late_join) + P.WARMUP_LOOK_STATE_OFFSET
    drag_end = P.WARMUP_LIMITS_VA + P.WARMUP_DRAG_END_OFFSET
    # The site: update_camera's call of the view's update.
    va, old, offset, opcode = P.DEATH_VIEW_HOOK
    new = P.hook_bytes(death_va, va, old, offset, opcode)
    camera = texts(stock, 0x1F3480, va - 0x1F3480)
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and texts(stock, va, len(old)) == ["call 0x2422b0"] and texts(image, va, len(old)) == [f"call {view_va:#x}"]
          and camera[:6] == ["push ebp", "mov ebp,esp", "and esp,byte -0x8", "sub esp,byte +0x44", "push ebx",
                             "mov ebx,[ebp+0x8]"]
          and not [t for t in camera[6:] if re.match(r"(mov|lea|pop|xchg|add|sub|and|or|xor|inc|dec) ebx\b", t)]
          and at(stock, 0x1F3643, 8) == {0x1F3643: "mov ecx,[ebx+0xf470]", 0x1F3649: "push eax", 0x1F364A: "push ecx"},
          f"{va:#x}: update_camera's `call 0x2422b0` (the handler and the transform pushed, ebx the player from "
          f"[ebp+8], never written before it) in the original and the build without the late join, "
          f"`call {view_va:#x}` (death_view) here")
    hits = [h for h in aimed_inside(original, va, len(old), 0x80) if not va <= h < va + len(old)]
    spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
    found = hits[:len(hits) - len(spelled)]
    found += [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    found += [address for address, _, text in ndisasm(stock.read(0x1F3480, 0x1F366B - 0x1F3480), 0x1F3480)
              if (target := branch_target(text)) is not None and va < target < va + len(old)]
    check(not found, "nothing aims inside the site and no base relocation is under it"
          + "".join(f", {h:#x}" for h in found))
    # The view's update: modes 0 and 4 copy the transform; the others take
    # the third-person step at 0x2422DE, which reads the handler's yaw, pitch
    # and distance and writes its view and its changed flag alone.
    update = texts(stock, 0x2422B0, 0x2422DE - 0x2422B0)
    step = ndisasm(stock.read(0x2422DE, 0x242459 - 0x2422DE), 0x2422DE)
    fields = sorted({int(m, 16) for _, _, t in step for m in re.findall(r"\[ebp\+(0x[0-9a-f]+)\]", t)})
    check(update == ["sub esp,0xd4", "push ebx", "mov ebx,[esp+0xe0]", "push ebp", "mov ebp,[esp+0xe0]",
                     "mov eax,[ebp+0x358]", "push esi", "push edi", "test eax,eax", "jz 0x2422d7",
                     "cmp eax,byte +0x4", "jnz 0x2422de", "mov esi,ebx", "jmp 0x24243b"]
          and step[0][2] == "xorps xmm0,xmm0" and fields == [0x48, 0x34C, 0x350, 0x354, 0x35C]
          and "lea edi,[ebp+0x48]" in [t for _, _, t in step] and step[-1][2] == "ret 0x8"
          and not [t for _, _, t in step if re.search(r"\b(eax|ebx|esi|edi|ebp)\b.*\b0x35[8c]\b", t)
                   and "ebp+0x35c" not in t],
          "the view's update (0x2422B0; the handler and the transform on the stack, `ret 8`): the modes 0 and 4 "
          "copy the transform to the view (+48h); the others' step (0x2422DE, an instruction) reads the yaw "
          "(+34Ch), pitch (+350h) and distance (+354h) and writes the view and the changed flag (+35Ch) alone")
    # A death leaves the mode; a dead local player sends no input; the
    # handler's world, game, client and local player; the kits' flag.
    death = texts(stock, 0x1F28E0, 0x1F2985 - 0x1F28E0)
    send = at(stock, 0x1F7883, 13)
    chain = texts(stock, 0x2410F8, 0x241128 - 0x2410F8)
    tick = texts(stock, 0x1F7E7E, 0x1F7E8F - 0x1F7E7E)
    check(not [t for t in death if "0x358" in t] and "call 0x2d99c0" in death
          and send == {0x1F7883: "cmp byte [eax+0x33c],0x0", 0x1F788A: "jz near 0x1f7928"}
          and "mov eax,[esi+0x334]" in chain and "mov ecx,[eax+0x9c]" in chain and "mov eax,[ecx+0x53ac]" in chain
          and "add eax,byte +0x4" in chain
          and tick == ["mov eax,[ebx+0x9c]", "mov eax,[eax+0x53ac]", "add eax,byte +0x4", "mov eax,[eax]"],
          "player::on_player_death (0x1F28E0) sets no mode of the input handler; game_world::tick builds and "
          "sends the local player's input only while it is alive (+33Ch, 0x1F7883); the handler's +334h is the "
          "game world (0x2410F8), whose +9Ch is the game, +53ACh its network client and +4 the local player "
          "(0x1F7E7E); the kits' flag is the hud's +2DFh (check 32), the game world's +4DBh")
    # The window's size and the look's step, as the player's look and the lobby's.
    lobby = texts(stock, 0x283E0C, 0x283E6B - 0x283E0C)
    look = texts(stock, 0x240226, 0x240330 - 0x240226)
    floats = lambda va: struct.unpack("<f", stock.read(va, 4))[0]
    check(lobby[:6] == ["mov ecx,[eax+0x5348]", "mov edx,[ecx]", "mov edx,[edx+0x1c]", "lea eax,[esp+0x8]",
                        "push eax", "call edx"]
          and "movss xmm3,dword [esp+0xc]" in lobby and "divss xmm3,dword [esp+0x8]" in lobby
          and "mov esi,[edx+0x5348]" in look and "mov edx,[eax+0x1c]" in look
          and "mulss xmm2,dword [dword 0xa74aec]" in look and "mulss xmm2,dword [dword 0x921948]" in look
          and "mulss xmm1,dword [dword 0xa2dac4]" in look and "cmp [dword 0x1f1d9d3],bl" in look
          and all(abs(floats(a) - value) < 1e-6 for a, value in ((0x921948, 0.1), (0xA2D7E8, 1 / 180),
                                                                  (0x9CC970, math.pi), (0xA2DAC4, 0.95492965))),
          "the render window (game+5348h) gives its width and height (floats) at the address on the stack and "
          "pops it (vtable+1Ch, 0x283E0C); the player's look turns by a unit x sensitivity (0xA74AEC) x 0.1 x "
          "pi/180, the pitch also x height / width x 0.9549 and inverted by mouse_invertion (0x1F1D9D3)")
    # The code.
    code = open(P.death_camera_bin(), "rb").read()
    data_at = code.index(struct.pack("<f", math.pi / 180 * 0.1))
    step_value, aspect, yaw_max, yaw_min, pitch_up, pitch_down = struct.unpack_from("<6f", code, data_at)
    identity_at = (data_at + 24 + 15) // 16 * 16
    identity = struct.pack("<16f", 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)
    limits = [round(math.degrees(a), 4) for a in (yaw_max, yaw_min, pitch_up, pitch_down)] == [70, -70, 60, -40]
    parts = ((0, data_at, ["call [eax+0x30]", "call [eax+0x1c]", f"call {drag_end:#x}"], ["ret 0x10"],
              {P.WARMUP_LOOK_VA}, {0xA74AEC, 0x1F1D9D3, state, state + 4}
              | {death_va + data_at + 4 * i for i in range(6)}),
             (P.DEATH_VIEW_OFFSET, len(code), None, ["ret 0x8"], {P.DEATH_UPDATE_VA, P.DEATH_THIRD_VA},
              {state, state + 4}))
    good = image.read(death_va, len(code)) == code and limits \
        and code[identity_at:identity_at + 64] == identity \
        and set(code[data_at + 24:identity_at] + code[identity_at + 64:P.DEATH_VIEW_OFFSET]) <= {0xCC}
    for begin, end, calls, rets, exits, absolute in parts:
        listing = ndisasm(code[begin:end], death_va + begin)
        starts = {address for address, _, _ in listing}
        targets = [t for _, _, text in listing if (t := branch_target(text)) is not None and not text.startswith("call")]
        made = [text for _, _, text in listing if text.startswith("call")]
        inside = lambda t: death_va + begin <= t < death_va + end
        good &= all(t in starts for t in targets if inside(t)) and {t for t in targets if not inside(t)} == exits
        good &= [t for _, _, t in listing if t.startswith("ret")] == rets
        good &= {int(m, 16) for _, _, t in listing for m in re.findall(r"\[(?:dword |byte )?(0x[0-9a-f]+)\]", t)} \
            == absolute
        if calls is None:                          # the third-person step's start, then the jump to it
            third = [branch_target(t) for _, _, t in listing if t.startswith("call")]
            good &= len(third) == 1 and texts(image, third[0], 0x1D) == update[:5] + ["push esi", "push edi",
                                                                                   f"jmp {P.DEATH_THIRD_VA:#x}"]
        else:
            good &= made == calls
    first = texts(image, death_va, 0x60)
    good &= first[:2] == ["cmp byte [ecx+0x2e3],0x0", f"jz {death_va + 0x40:#x}"] \
        and f"mov ebx,{look_state:#x}" in texts(image, death_va, data_at) \
        and texts(image, view_va, 4) == ["mov eax,[esp+0x4]"] \
        and f"push dword {death_va + identity_at:#x}" in texts(image, view_va, len(code) - P.DEATH_VIEW_OFFSET) \
        and [t for t in texts(image, view_va, len(code) - P.DEATH_VIEW_OFFSET) if t.startswith(("mulps", "addps"))] \
        == ["mulps xmm1,xmm4", "mulps xmm2,xmm5", "addps xmm1,xmm2", "mulps xmm0,xmm6", "addps xmm1,xmm0"] * 3
    check(good and P.FOV_LOD_STATE_OFFSET + P.FOV_LOD_STATE_SIZE <= P.DEATH_CAMERA_STATE_OFFSET
          and P.DEATH_CAMERA_STATE_OFFSET + P.DEATH_CAMERA_STATE_SIZE <= P.LATE_JOIN_STATE_SIZE
          and abs(step_value - math.pi / 180 * 0.1) < 1e-9 and abs(aspect - 0.95492965) < 1e-7,
          f"the code at {death_va:#x} (.srpc) is death_camera_026g.section.bin ({len(code)} bytes): death_look "
          f"begins with warmup_look's test of the kits, leaves to warmup_look ({P.WARMUP_LOOK_VA:#x}) alone, "
          "calls the hud's pointer (vtable+30h), the window's size (vtable+1Ch) and the end of a drag, keeps the "
          f"drag in warmup_look's state ({look_state:#x}) and the turn at {state:#x} (the late join's page "
          f"+{P.DEATH_CAMERA_STATE_OFFSET:#x}), reads the options sensitivity, mouse_invertion and its two constants (pi/180 "
          "x 0.1, 0.9549) and the turn's limits (70 degrees either side, 60 up, 40 down); death_view leaves to "
          "the view's update or its third-person step after the update's own start, which it gives the "
          "identity, then multiplies the turn by the eyes' three rows; every branch inside lands on an "
          "instruction; `ret 10h` and `ret 8`")


def test_suicide_key(original, image, before):
    """Check 35: the commit_suicide key (patch 34, the Rust patch
    `suicide_key`, RUST_HOOKS). check_suicide_key_026g.py runs the builder's
    last call on made-up input handlers."""
    stock = Image(original)
    name, action, kind, _, key = struct.unpack("<5I", stock.read(0xA74D20, 20))
    cstr = lambda va: stock.read(va, 32).split(b"\0")[0]
    check(cstr(name) == b"commit_suicide" and action == 0x1F and kind == 5 and cstr(key) == b"kK",
          "the action table's entry 0xA74D20 is commit_suicide, action 1Fh, kind 5, default key kK")
    rows = ndisasm(stock.read(0x241250, 0x2421B0 - 0x241250), 0x241250)
    # The pushes before each call, nearest first: ebx (the handler), the
    # action, &state (one call has other instructions between them).
    pushes = lambda i: [text for _, _, text in reversed(rows[max(0, i - 10):i]) if text.startswith("push ")]
    asked = [int(pushes(i)[1].split("+")[1], 16) for i, row in enumerate(rows)
             if row[2] == "call 0x242460" and pushes(i)[0] == "push ebx" and pushes(i)[1].startswith("push byte +")]
    calls = [row for row in rows if row[2] == "call 0x242460"]
    masks = [int(row[2].split(",")[1].split()[-1].lstrip("+"), 16) for row in rows
             if row[2].startswith("or dword [ebx+0x33c],")]
    callers = sorted(int(h.split()[0], 16) for h in branches_into(original, 0x242460, 0x242461))
    check(len(calls) == len(asked) == 35 and 0x1F not in asked and masks and 0x1000000 not in masks,
          f"the input builder 0x241250 asks 0x242460 for 35 actions ({len(asked)} read), 1Fh not among them, and "
          "never ors 1000000h into the action mask (+33Ch)")
    check(all(0x241250 <= at < 0x2421B0 for at in callers) and len(callers) == 35,
          "0x242460 is called by the input builder alone, 35 times")
    quick = [int(h.split()[0], 16) for h in branches_into(original, 0x2D8BB0, 0x2D8BB1)]
    check(stock.read(0x2D8BC1, 7) == bytes.fromhex("f7470400000001") and quick == [0x2D8777],
          "base_player::process_quick_slots tests bit 24 of the input (0x2D8BC1 `test dword [edi+4],1000000h`) and "
          "is called from the input apply 0x2D86E0 alone (0x2D8777)")
    va, old, symbol, opcode = next(h for h in P.rust_hooks(original) if h[2].startswith("suicide_key_"))
    linked = P.arena()
    block = linked.blocks["patches.code"]
    entry = linked.symbols[symbol]
    new = P.hook_bytes(entry, va, old, 0, opcode)
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and block.va <= entry < block.va + block.size,
          f"{va:#x}: `call 0x242460` (spot) in the original and the build without the late join, "
          f"`call {entry:#x}` here ({symbol}, in the arena's block patches.code: the Rust patches crate)")
    code = P.linker.block_bytes(linked, "patches.code")
    check(image.read(block.va, len(code)) == code,
          f"the code at {block.va:#x} is the link's patches.code ({len(code)} bytes: the Rust patches crate and "
          "the generated hook entries)")
    # The generated entry (RUST_CALL hook_suicide_key_on_spot, 3, 12): align
    # the stack, pass the site's three arguments to the shim, clean up as
    # action_present (stdcall) did.
    entry_code = code[entry - block.va:entry - block.va + 48]
    texts = [text for _, _, text in ndisasm(entry_code, entry)]
    shim = linked.symbols["hook_suicide_key_on_spot"]
    check(texts[:11] == ["push ebp", "mov ebp,esp", "and esp,byte -0x10", "sub esp,byte +0x4",
                         "push dword [ebp+0x10]", "push dword [ebp+0xc]", "push dword [ebp+0x8]",
                         f"call 0x{shim:x}", "mov esp,ebp", "pop ebp", "ret 0xc"],
          f"the entry {symbol} is RUST_CALL: the stack aligned to 16, action_present's three arguments passed to "
          f"the shim hook_suicide_key_on_spot ({shim:#x}), and `ret 0xc` (stdcall, as action_present pops)")
    hits = aimed_inside(original, va, len(old), 0x80)
    spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
    under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    check(len(hits) == len(spelled) and not under,
          "nothing aims inside the site (a 4-byte value spelling an address there is no pointer without a "
          "relocation) and no base relocation is under it" + "".join(f", {h:#x}" for h in hits + under))


def test_blackout(original, image, before):
    """Check 65: Blackout (patch 65, the Rust patch `blackout`, six
    lohooks of blackout.toml at sites of its own). check_blackout_026g.py
    runs each from the game's code before its site on made-up objects."""
    stock = Image(original)
    rows = lambda va, size: [text for _, _, text in ndisasm(stock.read(va, size), va)]
    check(rows(0x27B921, 12) == ["add esi,byte +0xc", "push esi", "lea eax,[ebp-0x2c]", "call 0x1f0e70"]
          and rows(0x27B903, 6) == ["mov [ebp-0x2c],eax", "mov [ebp-0x28],eax"]
          and rows(0x1F095C, 0x15)[:5] == ["mov ecx,[esp+0x40]", "push eax", "push ecx", "lea eax,[esp+0x1c]",
                                          "mov [edi+0x680],edx"]
          and rows(0x1F0971, 8) == ["pop esi", "pop ebx", "add esp,byte +0x34", "ret 0x8"]
          and stock.read(0x1F0F35, 3) == bytes.fromhex("c20800"),
          "match_options::deserialize (0x1F0E70, eax the reader, its first argument the options, `ret 8`) is "
          "called with ESI the options and the reader at EBP-2Ch (0x27B928) and, for a replay's chunk 1, the "
          "options from [esp+40h] and the reader at esp+14h (0x1F096C), back at 0x27B92D and 0x1F0971")
    check(rows(0x25CD22, 0x1F) == ["push byte +0xf", "call eax", "mov bl,al", "mov [esp+0x14],bl",
                                   "mov ecx,[esp+0x14]", "lea esi,[edi+0x4]", "push ecx", "call 0x2384c0",
                                   "test bl,bl", "jnz near 0x25cf0d"]
          and rows(0x23B206, 6) == ["push edi", "call 0x23aa10"],
          "on_keyboard_action asks for Tab (0x25CD22), keeps it in bl and [esp+14h], gives show_player_list the "
          "dword and sends every key to its end while bl is set (0x25CD39); update_ui calls update_player_icons "
          "with the ui pushed (0x23B206)")
    import hooks_026g
    hooks = [h for h in hooks_026g.hooks() if h["patch"] == "blackout"]
    linked = P.arena()
    block = linked.blocks["patches.code"]
    state = linked.blocks.get("patches.state")
    sites = {h["site_name"]: h for h in hooks}
    check(sorted(sites) == ["input_builder.end", "keyboard.tab_state", "player_icons.update_start", "player_names.visibility",
                            "replay.options_read", "static_info.options_read"]
          and all(h["kind"] == "lohook" for h in hooks) and state is not None,
          "blackout.toml's six hooks are lohooks at the sites static_info.options_read, replay.options_read, "
          "player_icons.update_start, player_names.visibility, keyboard.tab_state and input_builder.end; the crate's statics are the "
          "block patches.state")
    check(rows(0x1F2DCD, 12) == ["push byte +0x38", "call eax", "test al,al", "jnz near 0x1f2f10"]
          and rows(0x1F2F10, 7) == ["movzx ecx,byte [esi+0x114]"]
          and rows(0x1F2F33, 0x10) == ["mov edx,[esi+0x6b8]", "mov eax,[esp+0x14]", "cmp eax,[edx+0x45ec]"],
          "render_name's left-Alt (0x38) or crosshair result joins at 0x1F2F10 in AL; ESI is the player, "
          "ESP+14h the local team and the profile +45ECh the player's team")
    for h in hooks:
        va, old = h["site"], h["original"]
        entry = linked.symbols[h["entry"]]
        new = P.hook_bytes(entry, va, old, 0, 0xE9)
        # a branch into bytes 1.. of the span (a 4-byte value spelling an address there is no pointer
        # without a relocation; a pattern inside another instruction is no branch)
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        branches = [x for x in aimed_inside(original, va, len(old), 0x82)
                    if x not in spelled and starts_instruction(stock, x)]
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              and block.va <= entry < block.va + block.size and not under and not branches,
              f"{va:#x} ({h['site_name']}): {old.hex()} in the original and the build without the late join, "
              f"`jmp {entry:#x}` ({h['entry']}, patches.code) here; no branch into it, no relocation under it")


def test_language(original, image, before):
    """Check 79: the game language is a setting kept in user.cfg (patch 79,
    the Rust patch `language`, three lohooks of language.toml;
    ../docs/notes/language-option.md). check_language_026g.py runs each from
    the game's code on made-up objects, the original beside."""
    stock = Image(original)
    rows = lambda va, size: [text for _, _, text in ndisasm(stock.read(va, size), va)]
    text_at = lambda at: bytes(original[stock.offset(at):original.index(b"\0", stock.offset(at))])
    check(rows(0x1EA2C0, 7) == ["mov eax,edi", "call 0x1ec380"]
          and rows(0x1EA2FF, 0x13) == ["mov eax,edi", "call 0x1ec380", "lea eax,[edi+0x52a8]", "push eax",
                                       "call 0x1feed0"]
          and rows(0x1EA210, 6) == ["push ebp", "mov ebp,esp", "and esp,byte -0x8"],
          "game::on_configs_loaded (0x1EA210) runs load_cc_script twice (startup.cfg, user.cfg) and then "
          "load_text_localization with the game's text translator (+52A8h): the variable is registered at its "
          "start, the language resolved between the two (0x1EA306)")
    check(rows(0x1FEED6, 5) == ["mov eax,[0x21e75e0]"] and rows(0x1EA868, 6) == ["mov edx,[dword 0x21e75e0]"]
          and rows(0xE3C48, 10) == ["mov eax,0x21e75e0", "mov ecx,0xa81a88"]
          and text_at(0x9E654C) == b"original" and text_at(0x9FC7F4) == b"resources/localization/",
          "the game language is the buffer_string 0x21E75E0 (default \"original\"), which core::preinitialize "
          "fills from -localization= (key 0xA81A88) and the dictionary's and the swear filter's paths read")
    check(rows(0x1FF5D0, 6) == ["push ebp", "push esi", "mov esi,[esp+0x10]"]
          and rows(0x1FF96E, 6) == ["pop edi", "pop esi", "pop ebp", "ret 0x10"]
          and stock.read(0x9FCCD0, 15) == b"accept_changes\0",
          "game_options::scaleform_callback (0x1FF5D0, the method's name at [esp+8]) compares the name with its "
          "methods and returns at 0x1FF96E (`ret 10h` at 0x1FF971) for one it does not know")
    table = imports(stock)
    check(table.get(0x87027C) == ("KERNEL32.dll", "GetUserDefaultLCID")
          and not any(name == "GetUserDefaultUILanguage" for _, name in table.values()),
          "0x87027C is the KERNEL32 GetUserDefaultLCID import (a new player's language: the user locale's); the "
          "game imports no GetUserDefaultUILanguage")
    import hooks_026g
    hooks = [h for h in hooks_026g.hooks() if h["patch"] == "language"]
    linked = P.arena()
    block = linked.blocks["patches.code"]
    check(sorted(h["site_name"] for h in hooks) == ["configs.start", "configs.texts", "options.callback"]
          and all(h["kind"] == "lohook" for h in hooks),
          "language.toml's three hooks are lohooks at configs.start, configs.texts and options.callback")
    for h in hooks:
        va, old = h["site"], h["original"]
        entry = linked.symbols[h["entry"]]
        new = P.hook_bytes(entry, va, old, 0, 0xE9)
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        branches = [x for x in aimed_inside(original, va, len(old), 0x82)
                    if x not in spelled and starts_instruction(stock, x)]
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              and block.va <= entry < block.va + block.size and not under and not branches,
              f"{va:#x} ({h['site_name']}): {old.hex()} in the original and the build without the late join, "
              f"`jmp {entry:#x}` ({h['entry']}, patches.code) here; no branch into it, no relocation under it")


def test_weapon_kit(original, image, before):
    """Check 68: a weapon's upgrades in at most three records, the weapon kit
    (patch 68, the Rust patch `weapon_kit`, weapon_kit.toml: one code hook and
    three lohooks; ../docs/notes/weapon-kit.md). check_weapon_kit_026g.py runs
    each from the game's code on made-up objects, the original beside."""
    stock = Image(original)
    rows = lambda va, size: [text for _, _, text in ndisasm(stock.read(va, size), va)]
    loop = {a: t for a, _, t in ndisasm(stock.read(0x2F1771, 0x8E0), 0x2F1771)}
    check(rows(0x2F1771, 0xB) == ["call 0x2f2060", "mov ecx,[ebp+0x38]", "mov eax,[ebp+0x44]"]
          and loop.get(0x2F1799) == "mov eax,[ebp+0x40]" and loop.get(0x2F18D0) == "mov eax,[ebp+0x40]"
          and [a for a, t in loop.items() if "[ebp+0x40]" in t] == [0x2F1799, 0x2F18D0, 0x2F1F1F, 0x2F1F29]
          and loop.get(0x2F1F1F) == "mov [ebp+0x40],ebx"
          and rows(0x2F2041, 0x12) == ["lea esp,[ebp-0xa0]", "pop edi", "pop esi", "pop ebx", "add ebp,byte +0x74",
                                       "mov esp,ebp", "pop ebp", "ret 0x4"],
          "translate_query reads [ebp+40h] (the item's records) at 0x2F1799 and 0x2F18D0 alone after its call "
          "of 0x2F2060 (0x2F1771) (0x2F1F1F writes the local anew, for its own use), and its epilogue takes esp "
          "from ebp (0x2F2041): the room below its frame goes with it")
    check(rows(0x2882E3, 6) == ["mov ecx,[esp+0x48]", "mov eax,[ecx]"]
          and rows(0x2882D6, 0xD) == ["mov edx,[ecx]", "mov edx,[edx+0x14]", "push dword 0xa023dc", "push eax",
                                      "call edx"]
          and rows(0x2885C9, 0xA) == ["call 0x28b0f0", "mov al,[ebp-0x5]", "inc al"]
          and stock.read(0x8FC288 + 0x14, 4) == struct.pack("<I", 0x7C6A20)
          and original.count(struct.pack("<I", 0x8FC288)) == 1,
          "the upgrades to Flash set an object's `param` through its interface's vtable +14h (0x2882D6) and go "
          "on at 0x2882E3; setup_inventory_data pushes a record's pair at 0x2885C9; the AS3 object interface's "
          "vtable 0x8FC288 (written once, the AS3 movie root's 0x7AB0CB) holds SetMember 0x7C6A20 at +14h")
    import hooks_026g
    hooks = [h for h in hooks_026g.hooks() if h["patch"] == "weapon_kit"]
    linked = P.arena()
    block = linked.blocks["patches.code"]
    check(sorted((h["site_name"], h["kind"]) for h in hooks)
          == [("weapon_kit.attached_pair", "lohook"), ("weapon_kit.flash_upgrade", "lohook"),
              ("weapon_kit.preview_apply", "entry"), ("weapon_kit.query_records", "lohook"),
              ("weapon_kit.query_room", "code")],
          "weapon_kit.toml's hooks: the code hook query_room, the lohooks query_records, flash_upgrade and "
          "attached_pair, and the entry preview_apply (patch 48's call, check 48)")
    for h in (h for h in hooks if h["kind"] != "entry"):
        va, old = h["site"], h["original"]
        entry = linked.symbols[h["entry"]]
        new = P.hook_bytes(entry, va, old, 0, 0xE9)
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        branches = [x for x in aimed_inside(original, va, len(old), 0x82)
                    if x not in spelled and starts_instruction(stock, x)]
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              and block.va <= entry < block.va + block.size and not under and not branches,
              f"{va:#x} ({h['site_name']}): {old.hex()} in the original and the build without the late join, "
              f"`jmp {entry:#x}` ({h['entry']}, patches.code) here; no branch into it, no relocation under it")
    entry = linked.symbols[P.WEAPON_KIT_PREVIEW_APPLY]
    listing = [t for _, _, t in ndisasm(image.read(entry, 0x50), entry)]
    shim = linked.symbols["hook_weapon_kit_on_preview_apply"]
    calls = [i for i, t in enumerate(listing) if t.startswith("call ")]
    aligned = [i for i, t in enumerate(listing) if t == "and esp,byte -0x10"]
    check(calls and listing[calls[0]] == f"call {shim:#x}" and aligned and aligned[-1] < calls[0]
          and listing[0] == "lea esp,[esp-0x4]" and "ret" in listing,
          f"patch 48's call goes to the generated entry {P.WEAPON_KIT_PREVIEW_APPLY} ({entry:#x}), which aligns "
          "the stack to 16 before it calls the Rust shim and returns with `ret`")
    room = linked.symbols["weapon_kit_query_room"]
    cave = [t for _, _, t in ndisasm(image.read(room, 16), room)]
    check(cave == ["call 0x2f2060", "sub esp,0xa0", "jmp 0x2f1776"],
          "the code hook's cave makes the site's call, takes A0h (the room, survarium::weapon_kit_room) and goes "
          "on at 0x2F1776")


def test_lobby_scene(original, image, before):
    """Check 81: the lobby's scene is the player's pick (the lobby window's;
    the squad commander's in a squad, 0xA1's srp bytes), the pick kept in
    user.cfg, a change swapped behind a fade at once (patch 81, the Rust
    patch `lobby_scene`, nine lohooks of lobby_scene.toml: configs.start in a
    chain after patch 79's and 83's, lobby_camera.view after patch 45's flight and
    the diagnostics' sweep; ../docs/notes/lobby-scenes.md).
    check_lobby_scene_026g.py runs each from the game's code on made-up
    objects, the original beside."""
    stock = Image(original)
    rows = lambda va, size: [text for _, _, text in ndisasm(stock.read(va, size), va)]
    text_at = lambda at: bytes(original[stock.offset(at):original.index(b"\0", stock.offset(at))])
    check(rows(0x21E5A2, 0xE) == ["mov dword [ebp-0x68],0x9ffd98", "mov dword [ebp-0x64],0x40"]
          and text_at(0x9FFD98) == b"lobby_scene/day"
          and rows(0x21E569, 3) == ["mov [ebp-0x34],edx"] and rows(0x21E54A, 6) == ["lea edx,[ebp-0xe0]"]
          and rows(0x21E5CC, 10) == ["movq [eax+0x8],xmm0", "call 0x21e860"]
          and rows(0x21E5E7, 11) == ["lea ecx,[esi+0x8]", "lea eax,[ebp-0x80]", "call 0xe5b60"]
          and rows(0x21CDBB, 7) == ["mov eax,ebx", "call 0x21e450"],
          "lobby_menu::query_resources (0x21E450, its one call in the lobby menu's ctor 0x21CDBD) asks its "
          "fourth request {\"lobby_scene/day\", 40h} at [ebp-68h] with the lobby menu's variant ([ebp-34h]) "
          "among eight (0xE5B60 at 0x21E5ED); the list is whole at 0x21E5CC")
    check(rows(0x220B20, 6) == ["push ebp", "mov ebp,esp", "and esp,byte -0x8"]
          and rows(0x220DA2, 6) == ["mov eax,[edi+0x9cc]"]
          and rows(0x220E7A, 0xC) == ["mov eax,[ebx+0x114]", "mov ecx,[eax+0x1e4]"]
          and rows(0x221E77, 3) == ["ret 0x4"],
          "lobby_menu::on_render_scenes_ready (0x220B20, ECX the menu, [esp+4] the results) reads result 3's "
          "resource at +9CCh unchecked and its project's track (+1E4h) unchecked; it returns at 0x221E77 (ret 4)")
    check(rows(0x2747A8, 6) == ["mov [ebx+0x1eaaa4],ebp"]
          and rows(0x2747BC, 0x14) == ["cmp dword [ebx+0x1eaaa4],byte +0x0", "mov byte [ebx+0x1eaaac],0x0",
                                       "jz near 0x2748c2"]
          and rows(0x2748B1, 0x11) == ["mov eax,[esi+0x4]", "mov ecx,[eax]", "add eax,byte +0x4", "mov [esi+0x4],eax",
                                        "mov [ebx+0x1eaaa0],ecx"],
          "read_squad_info (0x274720, 0xA1) keeps the squad id at the lobby client's +1EAAA4h (0x2747A8), "
          "compares it with 0 and clears +1EAAACh (0x2747C3, both paths) and, for no squad, jumps over the "
          "rest; a squad's path reads its revision last and keeps it (0x2748BC, the reader ESI past it)")
    check(rows(0x1B14A6, 0xA) == ["cmp byte [dword 0xac914c],0x0", "lea ebx,[ebp+0x1]"]
          and rows(0x1B14F2, 7) == ["mov byte [dword 0xac914c],0x0"]
          and stock.read(0xAC914C, 1) == b"\x01",
          "the eye adaptation's byte (0xAC914C, 1 in the image: the first frame): while set, "
          "compute_per_pixel_eye_adaptated_luminance (0x1B1400) gives t_previous_luminance the frame's own "
          "luminance (+275Ch, its t_current), then clears it (0x1B14F2): the reset patch 81 sets")
    check(rows(0x2277E1, 6) == ["mov ecx,[edi+0x690]"] and rows(0x227801, 9) == ["mov edx,[ecx+0x108]", "mov ecx,[edx+0x4]"],
          "fill_squad_info (EDI the lobby menu) invokes the lobby movie [[menu+690h]+108h]+4 (root.show_squad): "
          "the movie srp_scene_fade and srp_lobby_state are invoked on")
    check(rows(0x28C982, 0xD) == ["push edx", "mov edx,[ecx]", "mov eax,[edx+0x3c]", "call eax", "call 0x270070"]
          and rows(0x270075, 0x9) == ["mov edi,eax", "add edi,byte +0x8", "push byte +0x3b", "mov eax,edi"]
          and rows(0x2700F5, 3) == ["ret 0x8"],
          "the movie's request_players_range (0x28C960) sends 3B through the lobby client's 0x270070 (EAX the "
          "client, [esp+4] the start, [esp+8] the count; ret 8): the exe's own pick at the sign-in goes so")
    vtable = lambda at, n: list(struct.unpack(f"<{n}I", stock.read(at, 4 * n)))
    check(rows(0x21D2E0, 6) == rows(0x21D4A0, 6) == ["push ebp", "mov ebp,esp", "and esp,byte -0x8"]
          and vtable(0x9FFE74, 2) == [0x21D2E0, 0x21D4A0] and vtable(0x9FFE74 + 0x28, 1) == [0x21D080],
          "the lobby menu's vtable (0x9FFE74) is on_activate (0x21D2E0), on_deactivate (0x21D4A0), ..., "
          "clear_resources (+28h, 0x21D080); both start push ebp / mov ebp,esp / and esp,-8")
    check(rows(0x1ED030, 0xA) == ["push ebx", "push esi", "mov esi,ecx", "mov ecx,[esi+0x535c]"]
          and rows(0x1ED0B0, 0x11) == ["mov ecx,[esi+0x539c]", "cmp ecx,ebx", "jz 0x1ed0c1", "mov eax,[ecx]",
                                        "mov edx,[eax+0x4]", "call edx"]
          and vtable(0x9FAC8C + 0x10, 1) == [0x1ED030],
          "game::clear_resources (the game's vtable 0x9FAC8C+10h, the game closing) starts push ebx / push esi / "
          "mov esi,ecx / mov ecx,[esi+535Ch] and deactivates the active scene (+539Ch, its vtable+4) at 0x1ED0B0")
    # the swap redoes these halves (vostok::lobby_menu::put_in)
    check(rows(0x21D085, 6) == ["mov esi,[ebp+0x114]"] and rows(0x21D093, 5) == ["call 0x2855b0"]
          and rows(0x21D0B1, 0xF) == ["mov edx,[edx+0x4]", "lea ecx,[eax+esi]", "mov eax,[ebp+0x3104]", "push eax",
                                       "call edx"]
          and rows(0x21D0C6, 0x11) == ["lea edi,[ebp+0x120]", "call 0x2de7f0", "mov [ebp+0x118],ebx"]
          and rows(0x21D0F8, 8) == ["add ecx,byte +0x38", "call 0xe0210"]
          and rows(0x220E4A, 0x1B) == ["mov ecx,[ebx+0x114]", "mov eax,[ecx]", "mov edx,[eax+0xc]", "call edx",
                                        "lea esi,[ebx+0x118]", "lea edi,[esi+0x8]", "call 0x2de7f0"]
          and rows(0x220E86, 5) == ["mov eax,[ecx]", "mov eax,[eax+0x20]"] and rows(0x220EA1, 5) == ["call 0x2de090"]
          and rows(0x220EAE, 0x15) == ["add dword [eax+0x1c],byte -0x1", "jnz 0x220ec3", "mov eax,[esp+0x10]",
                                        "mov ecx,[eax+0xc]", "mov edx,[ecx]", "push eax", "mov eax,[edx+0x1c]",
                                        "call eax"]
          and rows(0x22193E, 0xB) == ["mov eax,[ebx+0x114]", "call 0x285540"]
          and rows(0x221961, 0xE) == ["mov edx,[edx]", "lea ecx,[eax+esi]", "mov eax,[ebx+0x3104]", "push eax",
                                       "call edx"]
          and vtable(0xA0A064 + 0xC, 1) == [0x2F8720] and vtable(0xA029A8, 2) == [0x2F8850, 0x2F8970],
          "the project halves the swap redoes: clear_resources' (remove 0x2855B0, each static collision's "
          "vtable+4 with the physics world +3104h, the track player's stop 0x2DE7F0 and +118h 0, the release "
          "0xE0210) and on_render_scenes_ready's (the project's vtable+0Ch, simple_game_project's 0x2F8720; the "
          "stop; the track's emitter's vtable+20h, play 0x2DE090, the effect's emitter's vtable+1Ch at 0; "
          "insert 0x285540, each collision's vtable+0); static_collision's vtable 0xA029A8: insert 0x2F8850, "
          "remove 0x2F8970")
    import hooks_026g
    every = hooks_026g.hooks()
    members = [m for h in every for m in h.get("chain", [h])]
    hooks = {h["site_name"]: h for h in members if h["patch"] == "lobby_scene"}
    heads = {h["site_name"]: h for h in every if h.get("chain")}
    linked = P.arena()
    block = linked.blocks["patches.code"]
    check(sorted(hooks) == ["configs.start", "game.clear_resources", "lobby_camera.view", "lobby_client.squad_id",
                            "lobby_client.squad_read", "lobby_menu.activate", "lobby_menu.deactivate",
                            "lobby_menu.scene_request", "lobby_menu.scenes_ready"]
          and all(h["kind"] == "lohook" for h in hooks.values())
          and [m["patch"] for m in heads["configs.start"]["chain"]] == ["language", "hud_options", "lobby_scene", "music"]
          and [m["patch"] for m in heads["game.clear_resources"]["chain"]] == ["music", "lobby_scene"]
          and heads["configs.start"]["chain"][2]["shim"] == hooks["configs.start"]["shim"]
          and [m["patch"] for m in heads["lobby_camera.view"]["chain"]] == ["weapon_flight", "camera_sweep", "lobby_scene"]
          and heads["lobby_camera.view"]["chain"][-1]["shim"] == hooks["lobby_camera.view"]["shim"],
          "lobby_scene.toml's nine hooks are lohooks; at configs.start in the chain after patch 79's and 83's handlers "
          "and before 88's (one entry, language's, calls the shims); at game.clear_resources after patch 88's; at "
          "lobby_camera.view the last of the chain after patch 45's flight and the diagnostics' sweep")
    for site in ("lobby_client.squad_id", "lobby_client.squad_read", "lobby_menu.scene_request",
                 "lobby_menu.scenes_ready",
                 "lobby_menu.activate", "lobby_menu.deactivate", "game.clear_resources"):
        # a shared site (game.clear_resources, with patch 88's music) jumps to its chain's one entry
        h = heads.get(site, hooks[site])
        va, old = h["site"], h["original"]
        entry = linked.symbols[h["entry"]]
        new = P.hook_bytes(entry, va, old, 0, 0xE9)
        spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
        branches = [x for x in aimed_inside(original, va, len(old), 0x82)
                    if x not in spelled and starts_instruction(stock, x)]
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              and block.va <= entry < block.va + block.size and not under and not branches,
              f"{va:#x} ({site}): {old.hex()} in the original and the build without the late join, "
              f"`jmp {entry:#x}` ({h['entry']}, patches.code) here; no branch into it, no relocation under it")


def test_weapon_flight(original, image, before):
    """Check 45 (round 7): patch 45's camera flight, the Rust patch
    `weapon_flight` (one lohook, weapon_flight.toml, at lobby_camera.view;
    client_patch.md section 3.42): the lobby camera's tick with its yaw and
    pitch final, the frame's time at ESP+10h, then the steps, the ray and the
    eye; patch 45's state its interface (fields_026g.tsv's weapon_table, the
    NASM's STATE). check_weapon_table_026g.py flies it with the game's tick."""
    stock = Image(original)
    rows = lambda va, size: [text for _, _, text in ndisasm(stock.read(va, size), va)]
    check(rows(0x283FAD, 6) == ["movss dword [esp+0x10],xmm4"] and rows(0x2840EA, 0xF) == ["movss dword [ebx+0xb8],xmm0",
                                                                                     "fld dword [ebx+0xb4]",
                                                                                     "push ecx"]
          and rows(0x284309, 5) == ["call 0x2844d0"] and rows(0x28430E, 8) == ["movss xmm1,dword [ebx+0xc0]"],
          "lobby_camera::tick keeps the frame's time, at most 0.1 s, at [esp+10h] (0x283FAD); 0x2840F2 "
          "loads the clamped yaw for rotY after the pitch's store; the ray at 0x284309, the last clamp after")
    import hooks_026g
    import engine_026g
    hooks = [h for h in hooks_026g.hooks() if h["patch"] == "weapon_flight"]
    linked = P.arena()
    block = linked.blocks["patches.code"]
    late_join = open(P.late_join_bin(), "rb").read()
    state = P.late_join_state_va(late_join) + P.WEAPON_TABLE_STATE_OFFSET
    row = [x for x in engine_026g.fields() if x["module"] == "image" and x["field"] == "weapon_table"]
    check(len(hooks) == 1 and hooks[0]["kind"] == "lohook" and hooks[0]["site_name"] == "lobby_camera.view"
          and len(row) == 1 and row[0]["offset"] == state,
          f"weapon_flight.toml's one hook is a lohook at lobby_camera.view; fields_026g.tsv's weapon_table is "
          f"patch 45's STATE ({state:#x}, .srpd+{P.WEAPON_TABLE_STATE_OFFSET:#x})")
    h = hooks[0]
    va, old = h["site"], h["original"]
    entry = linked.symbols[h["entry"]]
    new = P.hook_bytes(entry, va, old, 0, 0xE9)
    branches = [x for x in aimed_inside(original, va, len(old), 0x82) if starts_instruction(stock, x)]
    under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and block.va <= entry < block.va + block.size and not under and not branches,
          f"{va:#x} (lobby_camera.view): {old.hex()} in the original and the build without the late join, "
          f"`jmp {entry:#x}` ({h['entry']}, patches.code) here; no branch into it, no relocation under it")


def test_stage_focus(original, image, before):
    """Check 52: Scaleform's Stage focus getter (0x771FA0) gives null for a
    focused object without its ActionScript object (patch 52, STAGE_FOCUS),
    where the original read that object's traits through null (the lobby's
    access violation at 0x77200B, 2026-10-22). Every full build.
    check_stage_focus_026g.py runs it."""
    stock = Image(original)
    rows = lambda img, at, size: [text for _, _, text in ndisasm(img.read(at, size), at)]
    va, old, new = P.STAGE_FOCUS
    check(stock.read(va, len(old)) == old and before.read(va, len(new)) == new == image.read(va, len(new)),
          f"{va:#x}: the original's 46 bytes, the new ones here and in the build without the late join")
    head = ["push ecx", "mov eax,[ecx+0x14]", "mov ecx,[eax+0x40]", "mov edx,[ecx+0x1a8]", "mov eax,[edx+0x8]",
            "movzx edx,byte [eax+0x3f54]", "push esi", "lea ecx,[esp+0x4]", "shl edx,byte 0x6", "push ecx",
            "lea ecx,[edx+eax+0x3b64]", "call 0x514370", "mov esi,[esp+0x4]", "test esi,esi", "jz 0x772031",
            "inc dword [esi+0x4]", "mov ecx,esi", "call 0x505c90", "movzx eax,byte [esi+0x41]",
            "mov edx,[esi+eax*4]", "lea ecx,[esi+eax*4]", "mov eax,[edx+0x4]", "call eax"]
    tail = ["mov ecx,[esp+0xc]", "push eax", "call 0x790e40", "mov ecx,esi", "call 0x505c90", "pop esi", "pop ecx",
            "ret 0x4", "push edi", "mov edi,[esp+0x10]", "mov ecx,[edi]", "test ecx,ecx"]
    reads = ["mov ecx,[eax+0x14]", "mov ecx,[ecx+0x3c]", "cmp ecx,byte +0x12"]
    check(rows(stock, 0x771FA0, 0x77203A - 0x771FA0) == head + [
              "test eax,eax", "jz 0x771ff6", "add eax,byte -0x1c", "jmp short 0x771ff8", "xor eax,eax",
              "mov ecx,[eax+0x8]", "test ecx,ecx", "jz 0x772003", "mov eax,ecx", "jmp short 0x772006",
              "mov eax,[eax+0x4]", "test al,0x1", "jz 0x77200b", "dec eax"] + reads + [
              "jz 0x77201b", "cmp ecx,byte +0x17", "jl 0x772031"] + tail,
          "the original: the focus group's weak pointer (MovieImpl +3B64h + 40h x [+3F54h]) locked by 0x514370; "
          "no focus -> 0x772031 (null); else the interface's vtable +4 minus 1Ch (0 for none), its raw pointer "
          "(+8) or collectible one (+4, bit 0 cleared), and that object's traits type ([+14h], +3Ch) read untested: "
          "12h or 17h and up -> 0x77201B (the result), else 0x772031")
    check(rows(image, 0x771FA0, 0x77203A - 0x771FA0) == head + [
              "test eax,eax", "jz 0x772012", "mov ecx,[eax-0x14]", "test ecx,ecx", "jnz 0x771ffb",
              "mov ecx,[eax-0x18]", "and ecx,byte -0x2", "jz 0x772012", "mov eax,ecx"] + reads + [
              "jz 0x77201b", "cmp ecx,byte +0x17", "jnl 0x77201b", "jmp short 0x772031"] + ["nop"] * 7 + tail,
          "here the same reads with no interface or no ActionScript object -> 0x772031 (null); eax at 0x77201B "
          "the object, as before")
    # The getter is flash.display.Stage's `focus`: the static initializer
    # sets the thunk's member pointer, and the ThunkInfo names it.
    check(rows(stock, 0x865D10, 12) == ["mov eax,0x771fa0", "xor ecx,ecx", "mov [0xaff180],eax"]
          and "call [dword 0xaff180]" in rows(stock, 0x7750F0, 0x35)
          and struct.unpack("<I", stock.read(0x8F1E40, 4))[0] == 0x7750F0
          and stock.read(struct.unpack("<I", stock.read(0x8F1E48, 4))[0], 6) == b"focus\0",
          "0x865D10 makes 0x771FA0 the member pointer at 0xAFF180, which the thunk 0x7750F0 calls; the ThunkInfo "
          "at 0x8F1E40 gives that thunk the name \"focus\"")
    # The lobby's key handler asks the movie whether a window is open, which
    # reads `stage.focus` (LobbyMenu.has_active_window).
    check("push dword 0xa07404" in rows(stock, 0x26CBDB, 0x28) and "call 0x511600" in rows(stock, 0x26CBDB, 0x28)
          and stock.read(0xA07404, 23) == b"root.has_active_window\0",
          "0x26CBF9 invokes root.has_active_window (0xA07404) on every key in the lobby")
    under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    spelled = [va + i for i in range(1, len(old)) if original.find(struct.pack("<I", va + i)) != -1]
    hits = [h for h in aimed_inside(original, va, len(old), 0x82) if h not in spelled and not va <= h < va + len(old)
            and starts_instruction(stock, h)]
    check(not under and not hits,
          "no base relocation under the 46 bytes, and no branch from outside them aims inside (their own jumps "
          "stay inside)" + "".join(f", {h:#x}" for h in hits + under))


def test_artefacts(original, image, before):
    """Check 54: the artefacts (patch 54, ARTEFACTS_*). check_artefacts_026g.py
    runs Spring's loader and links, Lifebone's active and the action with
    the game's own code on made-up artefacts."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 32).split(b"\0")[0]
    check(rows(stock, 0x33AD3B, 0x39) == [
              "call 0xf6b10", "mov edi,0xa1c218", "call 0xf6b10", "cmp word [eax+0x14],byte +0x2", "jnz 0x33ad57",
              "movss xmm0,dword [eax]", "jmp short 0x33ad5b", "cvtsi2ss xmm0,dword [eax]", "mov edi,0xa1c230",
              "mov eax,ebx", "mov [esi+0x10],ebp", "mov [esi+0x14],ebp", "mov dword [esi+0x18],0x5",
              "movss dword [esi+0x1c],xmm0"]
          and rows(stock, 0x33AE06, 0x1E) == [
              "mov edi,0xa1c230", "mov eax,ebx", "mov [esi+0x40],ebp", "mov [esi+0x44],ebp",
              "mov dword [esi+0x48],0x13", "movss dword [esi+0x4c],xmm0", "call 0xf6b10"]
          and [cstr(va) for va in (0xA1C1FC, 0xA1C218, 0xA1C230, 0x9FD6C4)]
          == [b"passive", b"max_carried_weight_mod", b"active", b"data"],
          "Spring's loader (0x33ACF0, esi = the config at +340h, ebx = `data`): the passive's second node is "
          "max_carried_weight (5) from `passive/max_carried_weight_mod` (0xA1C218), the active's third (+40h, "
          "index 13h at 0x33AE13) before the duration's lookup in `active` (0xA1C230)")
    check(rows(stock, 0x311BC0, 10) == ["push ebx", "push esi", "mov esi,eax", "mov eax,[esi+0x250]"]
          and rows(stock, 0x311DA0, 5) == ["push ebx", "mov ebx,[esp+0x8]"]
          and stock.read(0x311946, 5) == b"\xE8" + struct.pack("<i", 0x311BC0 - 0x31194B)
          and stock.read(0x311974, 5) == b"\xE8" + struct.pack("<i", 0x311DA0 - 0x311979),
          "the link of Spring's active nodes (0x311BC0, eax = the Spring, called by enable_active 0x311946) and "
          "their unlink (0x311DA0, stdcall, called by disable_active 0x311974)")
    check(rows(stock, 0x31227B, 0x3B)[-6:] == ["add esi,byte +0x20", "cmp esi,ebx", "jnz 0x312282", "pop ebp",
                                              "mov ecx,[edi+0x250]", "mov ecx,[ecx+0x174]"]
          and stock.read(0x312279, 2) == bytes.fromhex("742f"),
          "Lifebone's enable_active (0x312250): its remove_affects loop and the way around it both reach "
          "0x3122AA `mov ecx,[edi+0x250]` with the function's four pushes")
    check(rows(stock, 0x313300, 0x45) == [
              "push esi", "mov esi,ecx", "cmp dword [esi+0x280],byte +0x3", "jnz 0x313344",
              "cmp byte [esp+0x8],0x0", "jz 0x313344", "cmp dword [esi+0x28c],byte +0x0", "jnz 0x313344",
              "mov eax,[esi]", "mov edx,[eax+0x80]", "push edi", "mov edi,[esp+0x10]", "push edi", "call edx",
              "push edi", "push esi", "call 0x313350", "mov [esi+0x2a0],edi", "mov dword [esi+0x280],0x4",
              "pop edi", "pop esi"],
          "artefact_base::action (0x313300): a press of one in its passive (state 3) with no cooldown calls "
          "its enable_active, its effect, keeps the time and sets state 4 at 0x313339; its entry's ten bytes "
          "are whole instructions, the jne after them at 0x31330A")
    for va, old, new in P.ARTEFACTS_SIZES:
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new,
              f"{va:#x}: a Spring's size {struct.unpack('<I', old[1:])[0]:#x} in the original and the build "
              f"without the late join, {P.ARTEFACTS_SPRING_SIZE:#x} here")
    check(rows(stock, 0x25E7A6, 8) == ["pop edi", "pop esi", "mov eax,0x3b8", "pop ebx"]
          and stock.read(0x25EA4F, 10) == bytes.fromhex("c78398030000a442a000")
          and stock.read(0x25EA83, 6) == bytes.fromhex("8983b4030000"),
          "the size of artefact<artefact_spring_core> (0x25E770, type 6), whose constructor 0x25EA10 fills "
          "+398h..+3B7h; the bare core's (0x2EEA30) is 398h")
    linked = P.arena()
    artefacts_va = linked.blocks["artefacts.code"].va
    check(linked.blocks["artefacts.code"].kind == "code" and artefacts_va % 0x1000 == 0,
          f"the artefacts' code is the arena's block artefacts.code, a code page at {artefacts_va:#x}")
    for va, old, symbol, opcode in P.ARTEFACTS_HOOKS:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook
              and artefacts_va <= target < artefacts_va + 5 * len(P.ARTEFACTS_HOOKS),
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`jmp {target:#x}` ({symbol}, an entry of the block) here")
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(va - 4 < v < va + len(old) for va, old, _, _ in P.ARTEFACTS_HOOKS + tuple(
                 (s, o, 0, 0) for s, o, _ in P.ARTEFACTS_SIZES))]
    hits = [h for va, old, _, _ in P.ARTEFACTS_HOOKS for h in aimed_inside(original, va, len(old), 0x82)
            if not va <= h < va + len(old) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside a hook"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "artefacts.code")
    check(image.read(artefacts_va, len(code)) == code,
          f"the code at {artefacts_va:#x} is the link's artefacts.code ({len(code)} bytes)")
    listing = ndisasm(code, artefacts_va)
    names = code.index(b"aim_time_mod\0")
    listing = [row for row in listing if row[0] < artefacts_va + names]
    texts = [text for _, _, text in listing]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not artefacts_va <= branch_target(t) < artefacts_va + len(code)})
    check(P.artefacts_backs(linked) == list(P.ARTEFACTS_BACKS)
          and outside == sorted(set(P.ARTEFACTS_BACKS) | {0xF6B10, 0x2D5D00, 0x2D5AE0, 0x2E58B0, 0x32A420,
                                                          0x313350, 0x25F9F8}),
          "its eight entries go on where they came from (" + ", ".join(hex(a) for a in P.ARTEFACTS_BACKS)
          + "); it calls the config lookup 0xF6B10, the unlink 0x2D5D00, the getter 0x2D5AE0, the pool "
          "lookup 0x2E58B0, increase_health 0x32A420 and the activation effect 0x313350, and nothing else "
          "outside (and 0x25F9F8, the Onyx pop-up's way around set_parameterized_message)")
    heals = artefacts_va + names + 47
    check(code[names:] == b"aim_time_mod\0stopping_power_mod\0pain\0body\0head\0"
          + struct.pack("<III", *(artefacts_va + names + at for at in (32, 37, 42)))
          and f"mov ebp,{artefacts_va + names + 13:#x}" in texts
          and f"mov ebp,{artefacts_va + names:#x}" in texts and f"mov ebx,{heals:#x}" in texts
          and f"cmp ebx,{heals + 12:#x}" in texts and "mov edi,0xa1c1fc" in texts and "mov edi,0xa1c230" in texts,
          "its data: the keys aim_time_mod (in `passive` 0xA1C1FC and `active` 0xA1C230) and "
          "stopping_power_mod (in `active`), and the pools Lifebone's active fills (pain, body, head)")
    check(rows(stock, 0x312BF0, 0x20) == [
              "mov eax,[ecx+0xee8]", "mov edx,[esp+0x4]", "cmp eax,[edx+0x1c]", "jnz near 0x312cb9",
              "cmp dword [ecx+0x280],byte +0x3", "mov eax,[esp+0x8]", "jnz 0x312c47"]
          and stock.read(0x312777, 5) == b"\xB8" + struct.pack("<I", 0x312BF0)
          and [struct.unpack("<I", stock.read(vt + 0x80, 4))[0] for vt in (0xA06084, 0xA157BC)] == [0x3128C0] * 2
          and rows(stock, 0x312934, 18) == ["fstp dword [edi+0xf18]", "mov [edi+0xf14],eax", "pop edi", "pop esi",
                                            "pop ecx", "ret 0x4"],
          "Onyx's hit callback (0x312BF0, bound to each Onyx by its constructor at 0x312777; thiscall: the hit, "
          "&damage, &piercing): its hit type test, then its state test at 0x312C03 (seven bytes, whole) before "
          "`mov eax,[esp+8]` at 0x312C0A and the passive's way or the active's (0x312C47); Onyx's enable_active "
          "0x3128C0 (vtable +80h of both its classes) sets the active time left, ms, at +F14h")
    check(rows(stock, 0x25F9A5, 0x5C) == [
              "cmp eax,byte +0x4", "jnz 0x25f9f8", "mov edx,[ebx-0xcd0]", "mov ecx,[edx+0x174]", "mov eax,[ecx]",
              "mov edx,[eax+0xc]", "call edx", "movzx eax,byte [eax+0x114]", "mov esi,[ebx+0xc]",
              "mov ecx,[esi+0x9c]", "push eax", "mov eax,[ecx+0x53ac]", "call 0x242a40", "test al,al",
              "jz 0x25f9f8", "mov ecx,[ebx-0xc30]", "push dword 0xbb8", "push byte +0x14", "push byte +0x21",
              "add esi,0x1fc", "push esi", "call 0x2375c0", "mov edx,[ebx-0xca0]", "mov [ebx+0x10],edx"]
          and struct.unpack("<I", stock.read(0xA06118, 4))[0] == 0x25F640
          and rows(stock, 0x25F640, 0x15)[:3] == ["push ebp", "mov ebp,esp", "and esp,byte -0x10"]
          and stock.read(0x25F64D, 8) == bytes.fromhex("8bd98b8b60f3ffff")
          and stock.read(0xA0360C, 31) == b"root.set_parameterized_message\0",
          "Onyx's client update (0x25F640, slot 0 of its wrapper's vtable 0xA06118, ebx = the Onyx +F20h, its "
          "state +280h at ebx-0CA0h): when it sees state 4 for the local player's Onyx it shows the activation hint "
          "(+2F0h, at ebx-0C30h) with set_parameterized_message 0x2375C0 (the text, 21h, 14h, 3000 ms) once for "
          "each Onyx; 0x25F9DD's eleven bytes are whole instructions before `push 14h` at 0x25F9E8")
    hud_source = open(os.path.join(HERE, "..", "flash", "hud", "src", "HUD.as"), encoding="utf-8").read()
    from localization_026g import CATALOG, LANGUAGES, read_translations
    hints = [read_translations(CATALOG / f'{language}.tsv')['st_onyx_activation_hint']
             for language, *_ in LANGUAGES]
    check('text = text.split("%s").join(String(Math.ceil(time / 1000)));' in hud_source.split(
              "public function set_parameterized_message(")[1].split("public function")[0]
          and len(hints) == len(LANGUAGES) and all(h.count("%s") == 1 and "75" in h for h in hints),
          "the hud's set_parameterized_message writes a message's %s as its time on screen in seconds, and the "
          "all Onyx activation hints say 75 % for %s")
    check(rows(stock, 0x2DBD86, 0x1E) == ["xorps xmm1,xmm1", "movss xmm2,dword [esi+0x31c]", "comiss xmm2,xmm0",
                                          "lea eax,[esi+0x31c]", "jna 0x2dbda0", "lea eax,[esp+0x14]",
                                          "movss xmm0,dword [eax]"]
          and "jna near 0x2dbd89" in rows(stock, 0x2DBCF9, 6)
          and stock.read(0x2D8FE3, 7) == bytes.fromhex("8bc7e8e62c0000"),
          "the player's stagger (0x2DBCD0, eax = the player, from its tick 0x2D8FE5): xmm0 the recent hits' "
          "stopping power, from the loop or none (0x2DBCF9) at 0x2DBD89, -min(it, [+31Ch]) the slow")


def test_match_keys(original, image, before):
    """Check 58: the kit keys and the match chat option (patch 58,
    MATCH_KEYS_HOOKS, MATCH_KEYS_POLL_SLOT). check_match_keys_026g.py runs the four entries from
    the game's sites on made-up objects."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 40).split(b"\0")[0]
    check(rows(stock, 0x25CD00, 14) == ["push ebp", "mov ebp,esp", "and esp,byte -0x8", "sub esp,byte +0xc",
                                          "push ebx", "push esi", "push edi", "mov edi,ecx"]
          and struct.unpack("<I", stock.read(0x9FC588, 4))[0] == 0x25CD00
          and rows(stock, 0x25CD39, 0x1B) == ["test bl,bl", "jnz near 0x25cf0d", "cmp dword [ebp+0x10],byte +0x1",
                                              "jnz near 0x25cf0d", "mov ebx,[edi-0x15c]", "mov esi,[ebp+0xc]"]
          and rows(stock, 0x25CF0D, 11) == ["xor al,al", "pop edi", "pop esi", "pop ebx", "mov esp,ebp", "pop ebp",
                                           "ret 0xc"]
          and rows(stock, 0x25CED1, 0x20)[-3:] == ["cmp byte [esi+0x2e0],0x0", "lea edi,[esi+0x4]", "setz cl"]
          and stock.read(0x23A93A, 6) == bytes.fromhex("389fdc020000") and stock.read(0x23A9A1, 6) == bytes.fromhex("889fdc020000")
          and stock.read(0x2384D0, 3) == bytes.fromhex("8b4e08") and stock.read(0x2384EE, 9) == bytes.fromhex("8b91080100008b4a04"),
          "the game world's on_keyboard_action (0x25CD00, slot 0 of its input vtable 0x9FC588; edi = the handler "
          "from 0x25CD0C): after the Tab test, the kind's test at 0x25CD41 (1, a key down, goes on at 0x25CD4B; any "
          "other to 0x25CF0D, al 0); the handler's +2E0h is the hud's (handler+4) cursor flag +2DCh, which "
          "show_cursor (0x23A930) keeps; the hud invokes its movie at [[[hud+8]+108h]+4] (0x2384D0)")
    slot, slot_old, slot_symbol = P.MATCH_KEYS_POLL_SLOT
    check(rows(stock, 0x1ECB56, 0x13) == ["mov ecx,[ebx+0x5358]", "mov edx,[ecx]", "mov eax,[ebx+0x584c]",
                                          "mov edx,[edx]", "push eax", "call edx"]
          and stock.read(0xAA5468, 4) == struct.pack("<I", 0xAA5430) and rows(stock, 0x207369, 10) == [
              "mov esi,0xaa5430", "call 0x3b7b30"] and stock.read(0x3B7B3C, 6) == bytes.fromhex("c706d452a200")
          and stock.read(0xA252D4, 4) == struct.pack("<I", 0x3B7BD0)
          and rows(stock, 0x3B7C22, 0x10)[2:] == ["mov ecx,[esi+0x24]", "mov edx,[ecx]", "mov eax,[edx+0x20]", "call eax"]
          and rows(stock, 0x3B7C42, 0x1F)[6:13] == ["mov ecx,[edi]", "mov edx,[ecx]", "mov eax,[edx+0x14]", "push esi",
                                                   "call eax", "add edi,byte +0x4", "cmp edi,ebx"]
          and rows(stock, 0x3B7C72, 0x11)[2:] == ["mov ecx,[esi+0x24]", "mov edx,[ecx]", "mov eax,[edx+0x24]", "push ebp",
                                                "call eax"]
          and stock.read(0xA25498, 8) == struct.pack("<II", 0x3B8D20, 0x3B9AF0)
          and rows(stock, 0x3B9B20, 0xF)[:2] == ["mov ebx,[eax-0x4]", "test byte [eax],0x80"]
          and [rows(stock, va, 6) for va in (0x3B9B41, 0x3B9B6C, 0x3B9BCC)] == [
              ["push byte +0x1", "push ebx", "push edx", "call eax"], ["push byte +0x2", "push ebx", "push eax", "call edx"],
              ["push byte +0x3", "push ebx", "push eax", "call edx"]]
          and stock.read(slot, 4) == slot_old == struct.pack("<I", 0x75AF0) and rows(stock, 0x75AF0, 3) == ["ret 0x4"],
          "game::tick runs the input world's tick (0x3B7BD0, slot 0 of the input world 0xAA5430, [game+5358h]) at "
          "0x1ECB67: the keyboard's (+24h) execute (vtable+20h, 0x3B8D20, the poll) first, then every handler's slot "
          "+14h (the input world), then the keyboard's process (vtable+24h, 0x3B9AF0): each polled event a kind 1 "
          "(down, 0x80) or 2 (up), then each key down a kind 3; the game world's handler's slot +14h "
          f"({slot:#x}) is `ret 4` (0x75AF0)")
    check(rows(stock, 0x1F6821, 0x29)[1:3] == ["push esi", "mov eax,ebp"] and rows(stock, 0x1F6845, 5) == ["call 0x233810"]
          and rows(stock, 0x1E90A3, 5) == ["call 0x1f67b0"] and rows(stock, 0x1E9B4C, 7)[1] == "call 0x1ec170"
          and popped(stock, 0x233810, 0x233BE0) == 4,
          "0x1F6845 is the game world's (0x1F67B0, made by game::game at 0x1E90A3, before query_base_resources "
          "0x1EC170 queries the configs) call of the hud's constructor, eax the game world, the hud pushed")
    check(stock.read(0x262454, 10) == bytes.fromhex("6a00" "6a04" "53" "b8fc64a000") and cstr(0xA064FC) == b"g_private_messages_in_game"
          and stock.read(0x26245E, 5) == b"\xE8" + struct.pack("<i", 0x260E90 - 0x262463)
          and [m.start() for m in re.finditer(re.escape(struct.pack("<I", 0xA064FC)), original)] == [P.file_offset(0x26245A)]
          and stock.read(0x2339C9, 0x29) == bytes.fromhex("6a026a01" "8d855c030000" "50" "687031a000" "b860e12a0a"
                                                          "e8cda3deff" "6820c98600" "c70560e12a0a4c7c9e00"),
          "the gameplay options' option 4 is an options_item_bool (0x260E90) named \"g_private_messages_in_game\", "
          "used there alone (no command has the name); the hud registers its show_quests_progress as a cc_bool the "
          "same way (0x1DDB0, then the vtable 0x9E7C4C)")
    check(rows(stock, 0x26B90B, 0x13) == ["push eax", "lea ecx,[esp+0xc8]", "push ecx", "push edi", "push esi",
                                          "call 0x208ad0", "mov edx,[ebx+0x8]"]
          and popped(stock, 0x208AD0, 0x209224) == 0x14,
          "0x26B916 is process_incoming_text_message's call of add_message (0x208AD0, five arguments, `ret 14h`)")
    linked = P.arena()
    block = linked.blocks["match_keys.code"]
    for va, old, symbol, opcode in P.MATCH_KEYS_HOOKS:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`{'jmp' if opcode == 0xE9 else 'call'} {target:#x}` ({symbol}, an entry of match_keys.code) here")
    target = linked.symbols[slot_symbol]
    check(before.read(slot, 4) == slot_old and image.read(slot, 4) == struct.pack("<I", target)
          and block.va <= target < block.va + block.size
          and any(kind == 3 and va == slot for kind, va in stock.relocations().values()),
          f"{slot:#x}: 0x75AF0 in the build without the late join, {target:#x} ({slot_symbol}, an entry of "
          "match_keys.code) here, a pointer the base relocation table already covers")
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(va - 4 < v < va + len(old) for va, old, _, _ in P.MATCH_KEYS_HOOKS)]
    hits = [h for va, old, _, _ in P.MATCH_KEYS_HOOKS for h in aimed_inside(original, va, len(old), 0x82)
            if not va <= h < va + len(old) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside a hook"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "match_keys.code")
    check(image.read(block.va, len(code)) == code, f"the code at {block.va:#x} is the link's match_keys.code ({len(code)} bytes)")
    names = code.index(b"root.select_kit_key\0")
    texts = [text for va, _, text in ndisasm(code[:names], block.va)]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not block.va <= branch_target(t) < block.va + len(code)})
    state = linked.blocks["match_keys.state"]
    library = {linked.symbols["engine_movie_invoke"], linked.symbols["engine_cc_bool"]}
    check(outside == sorted({*P.MATCH_KEYS_BACKS, 0x233810, 0x208AD0} | library)
          and f"push dword {block.va + names:#x}" in texts and "push dword 0xa064fc" in texts
          and "mov eax,0x9e7c4c" in texts and f"mov [{linked.symbols['chat_variable']:#x}],eax" in texts
          and state.va <= linked.symbols["chat_variable"] < linked.symbols["hide_chat"] < state.va + state.size
          and state.va <= linked.symbols["kit_keys"] < state.va + state.size
          and f"cmp byte [dword {linked.symbols['kit_keys']:#x}],0x2" in texts
          and f"mov byte [dword {linked.symbols['kit_keys']:#x}],0x2" in texts
          and state.va <= linked.symbols["kit_keys_since"] < state.va + state.size
          and texts.count("call [dword 0x870090]") == 2 and "cmp eax,0x12c" in texts
          and f"sub eax,[dword {linked.symbols['kit_keys_since']:#x}]" in texts
          and struct.unpack("<I", stock.read(0x870090, 4))[0] == struct.unpack("<I", before.read(0x870090, 4))[0]
          and stock.read(P.IMAGE_BASE + struct.unpack("<I", stock.read(0x870090, 4))[0] + 2, 13) == b"GetTickCount\0",
          "it goes on in on_keyboard_action (0x25CD4B, 0x25CF0D), calls the hud's constructor (0x233810), the library's "
          "Invoke and cc_value<bool>, GetTickCount (its import) and nothing else, jumps to add_message (0x208AD0); it invokes "
          "root.select_kit_key, names its variable \"g_private_messages_in_game\" as a cc_bool, both in "
          "match_keys.state; the kits' screen's state kit_keys there too, set by the poll (OPEN once GetTickCount, import 0x870090, is 300 ms past OPENING's kit_keys_since) and OPEN (2) asked "
          "by the keys")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    check("public function select_kit_key(index:uint)" in here("flash", "hud", "src", "HUD.as")
          and "public function pickByKey(index:uint)" in here("flash", "hud", "src", "survarium", "player",
                                                               "ProfileSelector.as")
          and 'this.settings_content["_no_in_batle"]' in here("flash", "main_menu", "src", "survarium", "menu",
                                                                "Settings.as")
          and "this._no_in_batle.enabled = true;" in here("flash", "main_menu", "src", "main_menu_fla",
                                                          "settingsTabs_15.as")
          and "\nedit\tlocalization-1.json\t-\t" in open(os.path.join(HERE, "..", "..", "release", "content.tsv"),
                                                            encoding="utf-8").read(),
          "the hud movie has root.select_kit_key, the main menu enables the option's checkbox (_no_in_batle: its "
          "timeline and create_content), "
          "release/content.tsv labels it (localization-1.json)")


def test_hide_ui(original, image, before):
    """Check 62: the key that hides the interface (patch 62,
    HIDE_UI_HOOKS). check_hide_ui_026g.py runs the three entries from the
    game's sites on made-up objects, with the game's own show_ui and
    get_binded_action."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 40).split(b"\0")[0]
    entry = struct.unpack("<5I", stock.read(0xA74CA8, 20))
    check(cstr(entry[0]) == b"toggle_replay_ui" and entry[1:4] == (0x19, 5, 0) and cstr(entry[4]) == b"kF11"
          and rows(stock, 0x25CD5A, 0x11) == ["lea edx,[esp+0x14]", "push edx", "mov edi,0x1", "call 0x207d50",
                                              "mov edi,eax"]
          and rows(stock, 0x25CE10, 0x13) == ["lea ecx,[edi-0x11]", "cmp ecx,byte +0x8", "ja near 0x25cf0d",
                                              "jmp [ecx*4+0x25cf30]"]
          and struct.unpack("<I", stock.read(0x25CF30 + 8 * 4, 4))[0] == 0x25CED1
          and rows(stock, 0x25CED1, 0x0A) == ["mov esi,[esp+0x10]", "cmp dword [esi+0x18],byte +0x0",
                                              "jz 0x25cf0d"]
          and rows(stock, 0x25CEE4, 0x29)[:5] == ["cmp byte [esi+0x2e0],0x0", "lea edi,[esi+0x4]", "setz cl",
                                                 "movzx edx,cl", "push edx"]
          and stock.read(0x25CD16, 4) == bytes.fromhex("897c2410"),
          "toggle_replay_ui is action 19h (kinds 5, F11; the action table's entry 0xA74CA8); the game world's "
          "on_keyboard_action asks the binder for kind 1 (0x25CD5F) and its case 19h (the table 0x25CF30 + 20h) "
          "at 0x25CED1 takes the handler from [esp+10h] (0x25CD16) and does nothing without the replay's movie "
          "(+18h); with it, the replay's controls and the cursor (0x25CEDB..)")
    check(rows(stock, 0x1F95F0, 0x10)[6:9] == ["push esi", "push edi", "mov edi,ecx"]
          and stock.read(0x1F95FF, 6) == bytes.fromhex("389fa0000000")
          and stock.read(0x1F960B, 6) == bytes.fromhex("8db704020000") and stock.read(0x1F9618, 6) == bytes.fromhex("8db70c020000")
          and stock.read(0x1F9634, 6) == bytes.fromhex("8d8704020000") and stock.read(0x1F9641, 6) == bytes.fromhex("8d870c020000")
          and popped(stock, 0x1F95F0, 0x1F965B) == 4
          and struct.unpack("<I", stock.read(0x9FC514, 4))[0] == 0x1F95F0
          and rows(stock, 0x1F8966, 0x0F) == ["lea esi,[ebx+0x1fc]", "lea eax,[esp+0x30]", "call 0x233f40"]
          and stock.read(0x23383C, 3) == bytes.fromhex("897d18"),
          "game_world::show_ui (0x1F95F0, slot 8 of its vtable 0x9FC4F4, `ret 4`) shows or hides the hud movie "
          "(+204h) and the player icons' (+20Ch), its state +0A0h; the hud is the game world's +1FCh (0x1F8966), "
          "the game world its +18h (0x23383C)")
    check(image.read(0x23845A, 6) == stock.read(0x23845A, 6) == bytes.fromhex("53e8d0240000"),
          "the kits' show_cursor call (0x23845B) is the original's: a death or a respawn keeps the interface "
          "hidden (the owner, 2026-10-24)")
    check(rows(stock, 0x234AC1, 0x2F) == ["mov ecx,[ebp+0x8]", "add esp,byte +0xc", "push esi", "lea ebx,[ebp+0x8]",
                                          "push esi", "mov [ebp+0x334],esi", "mov edx,[ecx+0x108]", "mov ecx,[edx+0x4]",
                                          "push esi", "push dword 0xa03348", "mov [esp+0xa8],ebx", "call 0x511600",
                                          "mov eax,[ebp+0x10]"]
          and cstr(0xA03348) == b"root.reset",
          "0x234AE8 is initialize_match's (0x234880, ebp the hud: [+8] its movie, [+10h] the player icons') Invoke "
          "of root.reset on the hud movie, its four arguments pushed: a new match, the movies the last match's")
    check(rows(stock, 0x26CCC2, 0x15) == ["mov eax,[eax+0x5344]", "lea edx,[esp+0x10]", "push edx",
                                          "mov edi,0x2", "call 0x207d50"]
          and rows(stock, 0x26CCAE, 0x14) == ["lea ecx,[esp+0x28]", "call 0x2afa70", "mov al,0x1", "pop edi",
                                              "pop esi", "pop ebx", "mov esp,ebp", "pop ebp", "ret 0xc"]
          and rows(stock, 0x26CCA9, 5) == ["call 0x1fdd70"] and popped(stock, 0x1FDD70, 0x1FDEC9) == 0x14
          and rows(stock, 0x26CDA2, 7) == ["cmp esi,byte +0x40", "jnz 0x26ce11", "xor eax,eax"]
          and rows(stock, 0x26CDD2, 0x30)[:3] == ["mov eax,[ebx+0x9c]", "mov ecx,[eax+0x5854]", "mov edx,[ebx+0x690]"]
          and "call 0x1fdd70" in rows(stock, 0x26CDD2, 0x30) and "xor al,al" in rows(stock, 0x26CDD2, 0x3D)
          and popped(stock, 0x207D50, 0x207DB3) == 4 and stock.read(0x26CBDB, 6) == bytes.fromhex("8b9390060000"),
          "0x26CCD2 is lobby_menu::on_keyboard_action's (ebx the lobby menu, its movie [[+690h]+108h]+4) call of "
          "get_binded_action (0x207D50, `ret 4`) for kind 2; 0x26CCAE is where it goes after a key the movie "
          "typed (0x1FDD70 pops its five arguments): al 1, the key taken")
    linked = P.arena()
    block = linked.blocks["hide_ui.code"]
    for va, old, symbol, opcode in P.HIDE_UI_HOOKS:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`{'jmp' if opcode == 0xE9 else 'call'} {target:#x}` ({symbol}, an entry of hide_ui.code) here")
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(va - 4 < v < va + len(old) for va, old, _, _ in P.HIDE_UI_HOOKS)]
    hits = [h for va, old, _, _ in P.HIDE_UI_HOOKS for h in aimed_inside(original, va, len(old), 0x82)
            if not va <= h < va + len(old) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside a hook"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "hide_ui.code")
    check(image.read(block.va, len(code)) == code, f"the code at {block.va:#x} is the link's hide_ui.code ({len(code)} bytes)")
    names = code.index(b"root.toggle_interface\0")
    texts = [text for va, _, text in ndisasm(code[:names], block.va)]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not block.va <= branch_target(t) < block.va + len(code)})
    library = {linked.symbols[name] for name in ("engine_binded_action", "engine_movie_invoke")}
    pushed = lambda text: f"push dword {block.va + code.index(text + bytes(1)):#x}" in texts
    check(outside == sorted(set(P.HIDE_UI_BACKS[:2]) | library)
          and f"mov dword [esp],{P.HIDE_UI_BACKS[2]:#x}" in texts and f"mov dword [esp],{P.HIDE_UI_BACKS[3]:#x}" in texts
          and all(pushed(t) for t in (b"root.toggle_interface", b"root.interface_hidden", b"root.hide_interface"))
          and "hide_ui.state" not in linked.blocks,
          "it goes on in on_keyboard_action (0x25CEDB, 0x25CF0D), returns to "
          "0x26CCAE or 0x26CDD2, calls the library's get_binded_action and Invoke and nothing else (not the game "
          "world's show_ui: its hidden movies stand still); it invokes root.toggle_interface, "
          "root.interface_hidden and root.hide_interface, and keeps no state of its own")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    # the options' controls: the game's 40 rows, the six functions that read them, the arena's 41
    table, count = P.HIDE_UI_TABLE, P.HIDE_UI_ROWS
    empty = lambda at: struct.pack("<III", at + 0x1C, at + 0x1C, at + 0x3C) + bytes(32) + \
        struct.pack("<III", at + 0x48, at + 0x48, at + 0x68) + bytes(32)
    original_rows = [stock.read(table + 0x68 * i, 0x68) for i in range(count)]
    check(all(row[16:] == empty(table + 0x68 * i) for i, row in enumerate(original_rows))
          and 0x19 not in [struct.unpack_from("<I", row)[0] for row in original_rows]
          and cstr(struct.unpack_from("<I", original_rows[-1], 4)[0]) == b"st_ptt_action",
          f"the options' controls are {count} static rows of 68h bytes at {table:#x} (the action, its label's "
          "key, the group, the type, two empty buffer_strings pointing into the row), the last ptt's; none is 19h")
    check([rows(stock, va, len(old))[0] for va, old, _, _ in P.HIDE_UI_TABLE_REFS]
          == ["mov eax,0xa9db28", "mov esi,0xa9db2c", "add eax,0xa9db28", "mov esi,0xa9db34",
              "mov dword [esp+0xc],0xa9db28", "mov esi,0xa9db38"]
          and [rows(stock, va, len(old))[0] for va, old, _ in P.HIDE_UI_TABLE_COUNTS]
          == ["mov dword [esp+0x18],0x28", "mov ebx,0x28", "cmp bl,0x28", "mov dword [esp+0x44],0x28",
              "mov dword [esp+0x10],0x28", "mov dword [ebp-0x8],0x28"],
          "the table's six readers (0x201150, 0x201248, 0x2013BF, 0x201A19, 0x201D05, 0x201F2B) and their six "
          "counts of 28h")
    controls = linked.symbols["controls"]
    data_block = linked.blocks["hide_ui.data"]
    added = len(P.CONTROLS_ADDED)
    arena_rows = [image.read(controls + 0x68 * i, 0x68) for i in range(count + added)]
    new_rows = arena_rows[count:]
    labels = [struct.unpack_from("<I", row, 4)[0] for row in new_rows]
    check(data_block.va <= controls and controls + 0x68 * (count + added) <= data_block.va + data_block.size
          and all(arena_rows[i][:16] == original_rows[i][:16] and arena_rows[i][16:] == empty(controls + 0x68 * i)
                  for i in range(count))
          and [struct.unpack_from("<4I", row) for row in new_rows]
          == [(action, label, 2, 1) for (action, _), label in zip(P.CONTROLS_ADDED, labels)]
          and all(row[16:] == empty(controls + 0x68 * (count + n)) for n, row in enumerate(new_rows))
          and [image.read(label, len(key) + 1) for (_, key), label in zip(P.CONTROLS_ADDED, labels)]
          == [key.encode() + b"\0" for _, key in P.CONTROLS_ADDED],
          f"`controls` ({controls:#x}, hide_ui.data) is the game's 40 rows, each pointing into itself, a 41st: "
          "19h, st_hide_interface_action, and a 42nd: 23h, st_hold_artifact_action, group 2, type 1")
    va, old, new = P.ARTEFACT_KEY_DEFAULT
    entry = va - 0x10
    check(struct.unpack("<4I", stock.read(entry, 16))[1:] == (0x23, 5, 0) and stock.read(entry, 4) != bytes(4)
          and image.read(struct.unpack_from("<I", stock.read(entry, 4))[0], 9) == b"artefact\0"
          and stock.read(va, 4) == old and image.read(va, 4) == new and image.read(0x9FDCA8, 3) == b"kH\0",
          "the action table's entry 0xA74D5C is artefact (23h, kinds 5, no toggle), without a default key in the "
          "original; patched its default is kH (0x9FDCA8)")
    writes = P.hide_ui_table_writes(linked)
    check(all(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
              for va, old, new in writes)
          and [rows(image, va, len(new))[0] for va, _, new in writes[:6]]
          == [f"mov eax,{controls:#x}", f"mov esi,{controls + 4:#x}", f"add eax,{controls:#x}",
              f"mov esi,{controls + 0xC:#x}", f"mov dword [esp+0xc],{controls:#x}", f"mov esi,{controls + 0x10:#x}"]
          and [rows(image, va, len(new))[0][-4:] for va, _, new in writes[6:]] == ["0x2a"] * 6
          and {va + at for va, _, at, _ in P.HIDE_UI_TABLE_REFS}
          <= {v for kind, v in image.relocations().values() if kind == 3},
          "the six readers read `controls`, their counts are 2Ah, their addresses keep their base relocations; "
          "the build without the late join has them as the original")
    check("\nedit\tlocalization-1.json\t-\t" in open(os.path.join(HERE, "..", "..", "release", "content.tsv"),
                                                       encoding="utf-8").read(),
          "release/content.tsv makes the label (localization-1.json)")
    # the movies' side: each movie's interface is its root, hidden in one step; every layer the
    # game put on the stage is in the root's _stageLayer (the lobby's: _layers and _overlay)
    tree = lambda movie: {os.path.relpath(os.path.join(d, f), os.path.join(HERE, "..", "flash", movie, "src")):
                          open(os.path.join(d, f), encoding="utf-8").read()
                          for d, _, fs in os.walk(os.path.join(HERE, "..", "flash", movie, "src")) for f in fs
                          if f.endswith(".as")}
    stage_add = re.compile(r"((?:\w+\.)*\w*stage)\.addChild(?:At)?\(([^,)]+)")
    lobby_tree = tree("lobby_menu")
    lobby = lobby_tree["LobbyMenu.as"]
    found = sorted((path, m.group(1), m.group(2)) for path, text in lobby_tree.items()
                   for m in stage_add.finditer(text))
    # Message's `_stage` is the messages' container (Message.initStage(this._top_container)), the
    # name editor's `MainScene.stage` the root (MainScene.stage = this): both inside the root
    allowed = {("survarium/controls/Message.as", "_stage"), ("survarium/char/CharWindow.as", "MainScene.stage")}
    layered = sorted({m.group(1) for m in re.finditer(r"this\._(?:layers|overlay)\.addChild\(([\w.]+)\)", lobby)})
    check(not [f for f in found if (f[0], f[1]) not in allowed]
          and "Message.initStage(this._top_container);" in lobby and "MainScene.stage = this;" in lobby
          and "SrvContextMenu.registerStage(stage,this._layers);" in lobby
          and "DragManager.init(this.stage,this._overlay);" in lobby and "PopUpManager.layer = this._transients;" in lobby
          and "_layer.addChild(_id);" in lobby_tree["survarium/controls/SrvContextMenu.as"]
          and "(layer ? layer : _stage).addChild(_dragCanvas);" in lobby_tree["scaleform/clik/managers/DragManager.as"]
          and "host.addChild(_defaultPopupCanvas);" in lobby_tree["scaleform/clik/managers/PopUpManager.as"]
          and lobby.index("this.makeStageLayer();") < lobby.index("this.init();")
          and "super.addChild(this._stageLayer);" in lobby
          and all(f"override public function {f}" in lobby for f in ("addChild(", "addChildAt(", "setChildIndex("))
          and "this._stageLayer.scaleX = this._stageLayer.scaleY = 1 / scaleFactor;" in lobby
          and layered == ["this._cards", "this._corner", "this._front_container", "this._top2_container",
                          "this._top_container", "this._wheelBack", "this._wheelsMC", "this.fadeBack",
                          "this.lobbyGameLoad", "this.tipContainer"]
          and lobby.index("this._overlay.addChild(this._corner);") < lobby.index("this._overlay.addChild(this._cards);")
          and lobby.count("this._cards.addChild(this.itemHintRenderer") == 5
          and "setTopmostLevel(this.itemHintRenderer" not in lobby
          and "InterfaceHider" not in "".join(lobby_tree.values()),
          "the lobby's interface is its root: nothing of it is added to the stage (Message's container and "
          "MainScene.stage are inside the root); what the game put on the stage is in the root's last child, "
          "_stageLayer, at the stage's scale, made before anything else and kept last (addChild, addChildAt, "
          f"setChildIndex): {', '.join(layered)}, the right-click menu in _layers, CLIK's popup canvas in _transients; "
          "_overlay, above every modal: _corner, _cards (the item cards, no longer topmost), the drag canvas, the "
          "tooltips; no InterfaceHider, no timer")
    toggle = lobby[lobby.index("public function toggle_interface() : void"):]
    toggle = toggle[:toggle.index("public function interface_hidden()")]
    check("if(SrvContextMenu.visisble)\n         {\n            SrvContextMenu.hide();" in toggle
          and toggle.count("SrvContextMenu.hide()") == 1 and "this.hideItemHint(null);" in toggle
          and "visible = false;" in toggle and "_hiddenUi" not in lobby,
          "the lobby's toggle_interface closes a right-click menu only when one is open (none ever opened: "
          "SrvContextMenu.hide's removeChild(null) threw, owner's test of 64ce3ce7) and the item cards (on "
          "Scaleform's topmost level, drawn whatever their parents), then hides the root")
    names = lambda text: [m.start() for m in re.finditer(re.escape(text + b"\0"), original)]
    enable = lobby[lobby.index("public function enable_element(value:*, value2:Boolean = true) : void"):]
    enable = enable[:enable.index("public function highlight_element(")]
    check(names(b"root.enable_element") and names(b"root.statusPanel.btn_off") and names(b"root.statusPanel.btn_settings")
          and stock.read(0x2A3F22, 0x1B) == bytes.fromhex("6a01" "68a011a000" "e8" + struct.pack("<i", 0x22A660 - 0x2A3F2E).hex()
                                                          + "8b4720" "6a01" "68bc11a000" "e8"
                                                          + struct.pack("<i", 0x22A660 - 0x2A3F3D).hex())
          and struct.unpack("<I", stock.read(0xA0CBF8, 4))[0] == 0x2A3EA0
          and "setTopmostLevel" not in enable and "this._corner.addChild(button);" in enable
          and "this.statusPanel.addChildAt(button," in enable and "button != this.statusPanel.btn_off" in enable
          and "_topmost" not in lobby and "dispatchEvent(new Event(LAID_OUT));" in lobby_tree["survarium/controls/StatusPanel.as"]
          and "this.statusPanel.addEventListener(StatusPanel.LAID_OUT,this.onStatusPanelLaidOut);" in lobby
          and not any(f"function {t}(" in lobby for t in ("testDragTutor", "testPlayTutor", "testAmmoTutor",
                                                         "testAmmoTutor2", "testFactionPreTutor", "testFactionTutor",
                                                         "testCloseSlide", "showSlideOnVs", "testTutorComplete")),
          "the exe's lobby state (lobby_menu_state 0xA0CBF4, slot 1 0x2A3EA0, at every lobby entry) calls "
          "root.enable_element(root.statusPanel.btn_off / btn_settings, true), which put the corner buttons on "
          "Scaleform's topmost level (above every modal, and drawn whatever their parents: they stayed in the owner's "
          "test of 335343b6); the lobby's enable_element puts them in _corner instead (above every modal, inside the "
          "root, drawn where the status panel draws them after each of its layouts) and back with false; the "
          "tutorial's private test functions are gone")
    hud = here("flash", "hud", "src", "HUD.as")
    icons = here("flash", "player_icons", "src", "player_icons_fla", "MainTimeline.as")
    own = lambda movie: {p: t for p, t in tree(movie).items() if not p.startswith("scaleform")}
    hud_adds = sorted((p, m.group(1), m.group(2)) for p, t in own("hud").items() for m in stage_add.finditer(t))
    icon_adds = [(p, m.group(1)) for p, t in own("player_icons").items() for m in stage_add.finditer(t)]
    check(all("public function hide_interface(hidden:Boolean) : void" in t and "visible = !hidden;" in t
              for t in (hud, icons)) and "public function interface_hidden() : Boolean" in hud
          and hud_adds == [("HUD.as", "stage", "this._keptLayer"), ("HUD.as", "stage", "this.deathEyes"),
                           ("HUD.as", "stage", "this.effectContainer")]
          and not icon_adds
          and "stage.addChildAt(this.effectContainer,stage.getChildIndex(this));" in hud
          and "stage.addChildAt(this.deathEyes,stage.getChildIndex(this));" in hud
          and "this._stageLayer.addChild(this.gameInfo);" in hud and "place.concat(this.transform.matrix);" in hud
          and all(f"override public function {f}" in hud for f in ("addChild(", "addChildAt(", "setChildIndex("))
          and "InterfaceHider" not in "".join(tree("hud").values()) + "".join(tree("player_icons").values())
          and all(f"this.effects[this.{name}] = " in hud for name in ("BLOOD_EFFECT", "BLOOD_DRIP1", "GRENADE_EFFECT")),
          "the hud's and the player icons' interface is their root, hidden in one step (root.hide_interface; the "
          "hud's interface_hidden); the hud's fps line is in the root's _stageLayer; the two things on the hud's "
          "stage beside the root are the screen effects' container (add_effect's blood, its drips, the grenade's "
          "flash), below the root, placed as the root is scaled: the scene's, so it stays (the owner, 2026-10-24), "
          "and the death fade (DeathEyes, #529) between them, which F11 leaves too")
    check("public function toggle_interface() : void" in lobby and "public function interface_hidden() : Boolean" in lobby,
          "the lobby movie has root.toggle_interface and root.interface_hidden")


def test_kill_feed(original, image, before):
    """Check 67: the kill feed's wall and pain icons (patch 67,
    KILL_FEED_HOOKS, KILL_FEED_ARGUMENT). check_kill_feed_026g.py runs the
    three sites from the game's code on made-up victims, bullets and
    descriptions, with the game's own part lookup."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 40).split(b"\0")[0]
    check(rows(stock, 0x2DA3A8, 0x37) == ["call 0x2d43b0", "mov ecx,[esp+0x44]", "mov [eax+0x24],ecx",
                                          "mov cl,[esp+0x48]", "xor edi,edi", "mov [eax+0x18],edi",
                                          "movzx edx,byte [esi+0x114]", "mov [eax],dl", "movzx edx,byte [esp+0x4c]",
                                          "mov [eax+0x1],cl", "mov [eax+0x2],dl", "mov [eax+0x4],bl",
                                          "mov edx,[esi+0x120]", "call 0x2d0a60"]
          and rows(stock, 0x2DA179, 4) == ["mov ebp,[esp+0x50]"]
          and stock.read(0x2D9EFC, 0x1E) == bytes.fromhex("8b45fc50" "8b450c8b48408b1051" "0fb6484452" "8b503c5152" "8bf3e856020000")
          and stock.read(0x2D43F6, 8) == bytes.fromhex("8908" "894804" "894808"),
          "generate_killed_event (0x2DA170, esi the victim, ebp its fifth argument: hit_impl's bullet [ebp-4], "
          "0x2D9EFC) writes the kill event's victim, killer, weapon icon and cause icon (+4) into a node whose "
          "+0..+0Bh the allocation (0x2D43B0) zeroes, then queues it (0x2D0A60, eax the node, edx the world)")
    check(rows(stock, 0x23B6F0, 0x19) == ["movzx edx,byte [edi+0x4]", "movzx eax,byte [edi+0x2]",
                                          "movzx ecx,byte [edi+0x1]", "push edx", "movzx edx,byte [edi]", "push eax",
                                          "push ecx", "push edx", "push ebx", "call 0x237c70"]
          and popped(stock, 0x237C70, 0x238227) == 0x14
          and [t for t in rows(stock, 0x237C70, 0x5B7) if "esp+0x104" in t] == ["movzx eax,byte [esp+0x104]"]
          and "push dword 0xa036a4" in rows(stock, 0x23814F, 0x36)
          and cstr(0xA03698) == b"extra_icon" and cstr(0xA036A4) == b"mastery_icon"
          and cstr(0xA036B4) == b"root.add_log_message",
          "the hud's event handler (0x23B5F0) gives on_player_killed (0x237C70, `ret 14h`) the node's bytes, the "
          "cause icon fifth, which it reads as a byte alone (0x2380EA) for extra_icon; 0x23814F starts its "
          "mastery_icon, before root.add_log_message")
    check(rows(stock, 0x2D9C3D, 0x15)[:4] == ["mov eax,[ebx+0x2e0]", "add eax,0x108", "lea ecx,[ebp+0x8]",
                                              "mov dword [ebp+0x8],0x9fb1ec"]
          and cstr(0x9FB1EC) == b"pain" and stock.read(0x2D9C5E, 8) == bytes.fromhex("f30f1080a0000000")
          and stock.read(0x32EE20, 6) == bytes.fromhex("ff8684000000") and stock.read(0x32E33A, 6) == bytes.fromhex("fe8388000000")
          and stock.read(0x32BB1B, 12) == bytes.fromhex("899e84000000" "889e88000000"),
          "the damage model's parts by name (0x2E58B0, hit_impl's lookup of \"pain\"; health +0A0h); a bullet's "
          "segments +84h (one more for each pierce and reflection, 0x32EE20) and reflections +88h (0x32E33A), "
          "zero at its construction")
    linked = P.arena()
    block = linked.blocks["kill_feed.code"]
    for va, old, symbol, opcode in P.KILL_FEED_HOOKS:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`call {target:#x}` ({symbol}, an entry of kill_feed.code) and nops here")
    va, old, new = P.KILL_FEED_ARGUMENT
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and rows(image, va, len(new)) == ["movzx edx,word [edi+0x4]"],
          f"{va:#x}: the handler pushes the node's word +4 (the cause icon, the flags) here")
    sites = list(P.KILL_FEED_HOOKS) + [(va, old, None, None)]
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(s - 4 < v < s + len(o) for s, o, _, _ in sites)]
    hits = [h for s, o, _, _ in sites for h in aimed_inside(original, s, len(o), 0x82)
            if not s <= h < s + len(o) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside one"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "kill_feed.code")
    check(image.read(block.va, len(code)) == code, f"the code at {block.va:#x} is the link's kill_feed.code ({len(code)} bytes)")
    names = code.index(struct.pack("<I", block.va + code.index(b"pain\0")))   # the names' pointers: the data
    texts = [text for va, _, text in ndisasm(code[:names], block.va)]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not block.va <= branch_target(t) < block.va + len(code)})
    pushed = lambda text: f"push dword {block.va + code.index(text + bytes(1)):#x}" in texts
    check(outside == [0x2E58B0] and pushed(b"penetration_icon") and pushed(b"ricochet_icon") and pushed(b"pain_icon")
          and "kill_feed.state" not in linked.blocks
          and [t for t in texts if t.startswith("mov [") and "esp" not in t] == ["mov [eax+0x4],bl", "mov [eax+0x5],dl"],
          "it jumps to the part lookup (0x2E58B0) and calls nothing else of the game's (SetMember through the "
          "description's interface), sets penetration_icon, ricochet_icon and pain_icon, writes the node's +4 and +5 and nothing "
          "else outside its frame, and keeps no state")
    here = lambda *path: open(os.path.join(HERE, "..", *path), encoding="utf-8").read()
    item = here("flash", "hud", "src", "survarium", "controls", "CombatLogItem.as")
    check("data.penetration_icon == 1" in item and "data.ricochet_icon == 1" in item and "data.pain_icon == 1" in item
          and all(f"new {n}()" in item for n in ("CombatLogWallIcon", "CombatLogRicochetIcon", "CombatLogPainIcon"))
          and all(os.path.isfile(os.path.join(HERE, "..", "flash", "hud", "bitmaps", f"{n}.png"))
                  for n in ("CombatLogWallIcon", "CombatLogRicochetIcon", "CombatLogPainIcon")),
          "the hud movie's kill feed row shows penetration_icon, ricochet_icon and pain_icon (its bitmaps "
          "CombatLogWallIcon, CombatLogRicochetIcon and CombatLogPainIcon)")


def test_fall_gravity(original, image, before):
    """Check 72: free fall takes gravity for the step's dt (patch 72,
    FALL_GRAVITY; ../docs/notes/fall_damage.md "Split ticks").
    check_fall_gravity_026g.py runs step_down's free fall on a made-up
    controller: whole ticks bit for bit the original's, a split tick one
    tick of gravity in all."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    check(rows(stock, 0x7E6E6, 0x76) == [
              "movss xmm0,dword [ebx+0x2c708]", "movss xmm3,dword [ebx+0x2c704]", "movaps xmm1,xmm0",
              "mulss xmm1,dword [dword 0xa7dc00]", "mulss xmm1,xmm3", "movaps xmm2,xmm0", "mulss xmm2,dword [dword 0xa7dc04]",
              "mulss xmm0,dword [dword 0xa7dc08]", "mulss xmm2,xmm3", "mulss xmm0,xmm3",
              "movss xmm3,dword [ebx+0x2c770]", "subss xmm3,xmm1", "movss xmm1,dword [ebx+0x2c774]", "subss xmm1,xmm2",
              "movss dword [ebx+0x2c774],xmm1", "movss xmm1,dword [ebx+0x2c778]", "subss xmm1,xmm0",
              "movss dword [ebx+0x2c770],xmm3", "movss dword [ebx+0x2c778],xmm1"]
          and rows(stock, 0x7E75C, 0xB)[:2] == ["mov ecx,[ebx+0x1f0]", "lea eax,[ecx+0x2]"],
          "the original's free fall (0x7E6E6..0x7E75C): velocity (+2C770h) -= gravity (+2C708h) x up x the tick "
          "(+2C704h); then ecx and eax are written before any read")
    check(stock.read(0x7E1B0, 3) == bytes.fromhex("558bec") and stock.read(0x7BDA0, 3) == bytes.fromhex("558bec")
          and rows(stock, 0x7C13B, 0xD) == ["movss xmm0,dword [ebp+0x8]", "divss xmm0,dword [edi+0x2c704]"]
          and rows(stock, 0x7B75B, 0xC) == ["movss xmm0,dword [ebp+0xc]", "ucomiss xmm0,dword [esi+0x2c704]"]
          and stock.read(0x7B4A1, 8) == bytes.fromhex("f30f1185 04c70200".replace(" ", ""))
          and stock.read(0x7B48D, 8) == bytes.fromhex("f30f1005 78c89c00".replace(" ", ""))
          and stock.read(0x9CC878, 4) == struct.pack("<f", 0.05),
          "step_down and player_step both start `push ebp / mov ebp,esp`; player_step's dt is its [ebp+8]; "
          "updateAction steps it with dt compared to +2C704h, the 0.05 the constructor writes (0x7B4A1)")
    text = original[P.TEXT_FILE:P.TEXT_FILE + P.TEXT_VSIZE]
    callers = [P.TEXT_VA + i for i in range(len(text) - 5) if text[i] in (0xE8, 0xE9)
               and P.TEXT_VA + i + 5 + struct.unpack_from("<i", text, i + 1)[0] == 0x7E1B0]
    check(callers == [0x7C4F6] and struct.pack("<I", 0x7E1B0) not in original,
          "step_down's one caller is player_step (0x7C4F6), and no pointer names it")
    for va, old, new in P.FALL_GRAVITY:
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new,
              f"{va:#x}: the original's {len(old)} bytes in the original and the build without the late join, "
              "patch 72's here")
    check(rows(image, 0x7E6E6, 0x76) == [
              "lea eax,[ebx+0x2c704]", "movss xmm0,dword [eax+0x4]", "jmp short 0x7e74c", "int3", "int3", "int3",
              "movaps xmm1,xmm0", "mulss xmm1,dword [dword 0xa7dc00]", "mulss xmm1,xmm3", "movaps xmm2,xmm0",
              "mulss xmm2,dword [dword 0xa7dc04]", "mulss xmm0,dword [dword 0xa7dc08]", "mulss xmm2,xmm3", "mulss xmm0,xmm3",
              "movss xmm3,dword [eax+0x6c]", "subss xmm3,xmm1", "movss xmm1,dword [eax+0x70]", "subss xmm1,xmm2",
              "movss dword [eax+0x70],xmm1", "movss xmm1,dword [eax+0x74]", "subss xmm1,xmm0",
              "movss dword [eax+0x6c],xmm3", "movss dword [eax+0x74],xmm1", "jmp short 0x7e75c",
              "mov ecx,[ebp+0x0]", "movss xmm3,dword [ecx+0x8]", "minss xmm3,dword [eax]", "jmp short 0x7e6f6",
              "int3", "int3"],
          "the build's free fall: xmm3 = min(player_step's dt, the tick), then the original's products and "
          "the velocity -= them (eax = the controller +2C704h)")
    relocated = sorted(v for kind, v in stock.relocations().values() if kind == 3 and 0x7E6E6 - 4 < v < 0x7E75C)
    built_relocated = sorted(v for kind, v in image.relocations().values() if kind == 3 and 0x7E6E6 - 4 < v < 0x7E75C)
    check(relocated == [0x7E6FD, 0x7E70C, 0x7E714] and built_relocated == relocated,
          "the three base relocations in the free fall are the up axis's addresses, between the two sites, "
          "and stay")
    hits = [h for s, o, _ in P.FALL_GRAVITY for h in aimed_inside(original, s, len(o), 0x82)
            if not s <= h < s + len(o) and starts_instruction(stock, h)]
    check(not hits, "no branch aims inside the sites" + "".join(f", {h:#x}" for h in hits))


def test_bake_cache(original, image, before):
    """Check 70: the baked textures kept and made once (patch 70,
    BAKE_CACHE_HOOKS, BAKE_CACHE_KEEP). check_bake_cache_026g.py runs the
    sites from the game's code on a made-up baker, folder and queries."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    cstr = lambda va: stock.read(va, 60).split(b"\0")[0]
    check(rows(stock, 0x1E944E, 0xC) == ["lea eax,[esp+0x470]", "call 0x1e8da0"]
          and rows(stock, 0x1EC82B, 0xC) == ["lea eax,[esp+0x358]", "call 0x1e8da0"]
          and stock.read(0x1E8DCF, 7) == bytes.fromhex("c745e403000000") and stock.read(0x1E8DF4, 5) == bytes.fromhex("b814040000")
          and stock.read(0x1E8E01, 6) == bytes.fromhex("ff1578feae00")
          and cstr(0x9EE470) == b"user_data/streamed_textures",
          "game::game (0x1E9455) and game::exit (0x1EC832) delete user_data/streamed_textures: 0x1E8DA0 is "
          "SHFileOperationA (delay import 0xAEFE78) with FO_DELETE (3) and flags 414h")
    check(rows(stock, 0x131C88, 0x14) == ["mov eax,[esp+0x18]", "mov [esp+0x44],edi", "cmp eax,[edi+0x114]",
                                          "jz near 0x131dc2"]
          and rows(stock, 0x12D4BE, 0x16) == ["mov eax,[esp+0x58]", "add esp,byte +0xc", "lea edx,[esp+0x1b8]",
                                              "push edx", "call 0x131ed0", "test al,al"]
          and rows(stock, 0x12D41E, 0x41)[5:] == ["call 0xf0d00", "mov eax,[esp+0xa8]", "mov [esp+0xac],eax",
                                                  "test edi,edi", "jz near 0x12da56", "lea eax,[edi+0xd0]",
                                                  "mov ecx,eax", "or edx,byte -0x1", "lock xadd [ecx],edx",
                                                  "jnz near 0x12da56", "mov ecx,edi", "call 0xe0210"]
          and rows(stock, 0x12D45F, 9) == ["pop edi", "pop esi", "pop ebx", "mov esp,ebp", "pop ebp", "ret 0x4"]
          and rows(stock, 0x12DA56, 9) == ["pop edi", "pop esi", "pop ebx", "mov esp,ebp", "pop ebp", "ret 0x4"]
          and rows(stock, 0x12D2F7, 0x14)[:2] == ["mov edi,eax", "mov [esp+0x48],edi"]
          and rows(stock, 0x12F184, 0x10) == ["mov esi,[dword 0x1f1d960]", "mov ecx,0xac3334", "call 0x420789"]
          and rows(stock, 0x12DDA9, 0x10) == ["mov esi,[dword 0x1f1d960]", "mov ecx,0xac3334", "call 0x420789"]
          and stock.read(0x420789, 4) == bytes.fromhex("8d4108c3")
          and rows(stock, 0x12F0E0, 0xD) == ["mov edx,[esp+0x610]", "mov ecx,eax", "mov eax,[edx]", "mov [edx],ecx"]
          and stock.read(0xF0D00, 6) == bytes.fromhex("558bec83e4f8") and popped(stock, 0xF0D00, 0xF0F90) == 0x10,
          "check_loaded's CRC compare (0x131C90), translate_query's look-up (0x12D4CD) and its end after a refused "
          "query (0x12D42D: the surfaces emptied, edi's model instance released, `ret 4`), the texture's slot "
          "set before `call 0x420789` (`lea eax,[ecx+8]`) in process_compression (0x12F18F) and "
          "on_baked_texture_loaded (0x12DDB4), finish_query's entry (0xF0D00, `ret 10h`)")
    check(stock.read(0x42050, 0x31) == bytes.fromhex("8b4c2404" "57" "8d3c0a" "3bd7" "7321" "56" "8d4900" "8a08" "320a"
                                                     "c12808" "8b30" "0fb6c9" "8b0c8d68fd2502" "33ce" "42" "8908"
                                                     "3bd7" "72e4" "5e" "5f" "c20400")
          and stock.read(0x1C235, 5) == bytes.fromhex("35b71dc104") and stock.read(0x1C210, 7) == bytes.fromhex("803d64fd250200"),
          "the CRC-32 step 0x42050 (eax &the CRC, edx the bytes, [esp+4] the count; `ret 4`): a byte a step, "
          "crc = T[(crc ^ byte) & 0xFF] ^ (crc >> 8), T at 0x225FD68 made by 0x1C210 from the polynomial 04C11DB7h "
          "reflected (EDB88320h, bake_cache_026g.c's tables)")
    linked = P.arena()
    block = linked.blocks["bake_cache.code"]
    for va, old, symbol, opcode in P.BAKE_CACHE_HOOKS:
        target = linked.symbols[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook
              and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`{'call' if opcode == 0xE8 else 'jmp'} {target:#x}` ({symbol}, an entry of bake_cache.code) here")
    va, old, new = P.BAKE_CACHE_KEEP
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new,
          f"{va:#x}: game::exit's delete is a 5-byte nop")
    sites = [(s, o) for s, o, _, _ in P.BAKE_CACHE_HOOKS] + [(va, old)]
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(s - 4 < v < s + len(o) for s, o in sites)]
    hits = [h for s, o in sites for h in aimed_inside(original, s, len(o), 0x82)
            if not s <= h < s + len(o) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside one"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "bake_cache.code")
    check(image.read(block.va, len(code)) == code and "bake_cache.state" in linked.blocks,
          f"the code at {block.va:#x} is the link's bake_cache.code ({len(code)} bytes), its tables bake_cache.state")


def test_lamps(original, image, before):
    """The lamp's generated detours and its shared frame/ray chains."""
    import hooks_026g
    linked = P.arena()
    stock = Image(original)
    for hook in hooks_026g.hooks(exe=hooks_026g.original_reader(original)):
        owners = [member["patch"] for member in hook.get("chain", [hook])]
        if "lamps" not in owners or hook["kind"] != "lohook":
            continue
        va, old = hook["site"], hook["original"]
        expected = P.hook_bytes(linked.symbols[hook["entry"]], va, old, 0, 0xE9)
        check(before.read(va, len(old)) == old and image.read(va, len(old)) == expected,
              f"lamp hook {hook['site_name']}: native without the arena, generated target with it")
        hits = [h for h in aimed_inside(original, va, len(old), 0x82)
                if not va <= h < va + len(old) and starts_instruction(stock, h)]
        under = [v for kind, v in stock.relocations().values() if kind == 3
                 and va - 4 < v < va + len(old)]
        check(not hits and not under,
              f"lamp hook {hook['site_name']}: no branch enters the displaced instructions or relocation overlaps them"
              + "".join(f", {h:#x}" for h in hits + under))


def test_ragdoll(original, image, before):
    """Check 76: the death ragdoll (patch 76, the Rust patch `ragdoll`;
    ../docs/notes/ragdoll.md). Static, on the bytes: the nine sites and what
    follows each in the original, the jmps to the generated lohook entries in
    patches.code in the build, no base relocation under a site and no
    branch inside one, the block the link's, the entries' shapes, the
    game's code the crate branches to directly (the engine library's
    functions, the arena's memcpy, memmove and memset, the hooks' ways back)
    and its absolute stores (its own statics alone). What it does runs elsewhere: check_ragdoll_026g.py
    (Unicorn: the five hooks from their sites, a death, a rollback, the same
    death again, bullets through a corpse, a body on batteries) and `cargo
    test -p ragdoll` (the solver, the shots and the corpses' machine)."""
    stock = Image(original)
    rows = lambda source, va, size: [text for _, _, text in ndisasm(source.read(va, size), va)]
    check(rows(stock, 0x2DA3DA, 0x11) == ["call 0x2d0a60", "mov eax,[esi+0x120]", "mov ecx,[eax+0x18fa8]"],
          "fatal-hit hook follows the event enqueue, outside patch 67's span; ESI victim and BL cause survive")
    check(rows(stock, 0x1F7C37, 0xC) == ["call 0x2d2e40", "cmp byte [esp+0xf],0x0", "jz 0x1f7c6c"]
          and rows(stock, 0x1F7C24, 2) == ["jz 0x1f7c3c"] and rows(stock, 0x1F7C2D, 2) == ["jz 0x1f7c3c"],
          "game_world::tick: the join after game_world_core::move (0x1F7C3C, the target of 0x1F7C24 and "
          "0x1F7C2D), then the je on [esp+0Fh]")
    check(rows(stock, 0x247569, 0x2F) ==["cmp byte [ecx+0x10d],0x0", "jz near 0x247811", "cmp byte [ebp-0x1],0x0",
                                          "jnz 0x247592", "cmp byte [ebx+0xf4e2],0x0", "jz 0x247592",
                                          "cmp dword [ebx+0x12c],byte +0x0", "jnz near 0x247811",
                                          "mov edi,[eax+0x10c]"],
          "mt_draw's cull: a model not seen the last frame (+0F4E2h) and holding an item goes to 0x247811 (no "
          "bones); 0x247592 draws (EAX the model's instance)")
    check(rows(stock, 0x2476FF, 0x23)[0] == "jz 0x247716"
          and rows(stock, 0x247716, 0x22) == ["mov ecx,[ebx+0xf290]", "mov eax,[ecx+0x10c]", "mov ecx,0x9de384",
                                              "lea esi,[eax+0x110]", "lea edi,[ebx+0x40c]", "call 0x1113a0"],
          "mt_draw: the bones made (0x2476AB) and the decal pose's bind pose (0x2476F8) both join at 0x247716, "
          "then the \"Weapon\" bone's index (0x1113A0)")
    check(rows(stock, 0x1F356E, 0xB) == ["lea eax,[ebx+0x44c]", "jmp 0x1f3643"]
          and rows(stock, 0x1F35A5, 0xB) == ["lea eax,[ebx+0x44c]", "jmp 0x1f3643"]
          and rows(stock, 0x1F363F, 0x11) == ["lea eax,[esp+0x10]", "mov ecx,[ebx+0xf470]", "push eax", "push ecx",
                                              "call 0x2422b0"],
          "update_camera: the first person's eye (&player+44Ch) and the third person's copy join at 0x1F3643, "
          "EAX the transform the view's update (0x1F364B, patch 46's death_view in the build) gets")
    check(rows(stock, 0x32D5E8, 0x3F)[:4] == ["mov edx,[ebp+0x0]", "mov edx,[edx+0x48]", "push byte +0x1",
                                              "push edi"]
          and rows(stock, 0x32D625, 0x17) == ["call edx", "movq xmm0,[eax]", "movq [esp+0x38],xmm0",
                                              "movq xmm0,[eax+0x8]", "movq [esp+0x40],xmm0"]
          and rows(stock, 0x32D5F3, 0x13) == ["push byte +0x10", "push ecx", "fstp dword [esp]",
                                              "lea eax,[esp+0x34]", "push eax", "lea ecx,[esp+0xa8]", "push ecx"],
          "check_collision: the bullet's ray through the world's vtable+48h (ray_test: the result, &[esp+90h] the "
          "origin, &[esp+20h] the direction, the length, group 10h, mask 48h, the bullet, 1), then its result's "
          "copy from EAX at 0x32D627")
    check(rows(stock, 0x32BA8E, 0x12)[:4] == ["movq xmm0,[eax]", "movq [esi+0x1c],xmm0", "mov edx,[eax+0x8]",
                                         "mov [esi+0x24],edx"]
          and rows(stock, 0x32BAAC, 0x0D)[:3] == ["movq xmm0,[eax]", "mov ecx,[esp+0x28]", "movq [esi+0x34],xmm0"],
          "the bullet's ctor writes its fire velocity (EAX) to +1Ch and +34h: the speed patch 76 reads for a push")
    trace = P.arena().symbols["SRP_DIAGNOSTICS"]
    check(image.read(trace, 4) == P.DIAGNOSTICS_OFF,
          f"the build's diagnostics are off (SRP_DIAGNOSTICS at {trace:#x} \"OFF\": only --ragdoll-trace makes it "
          "1, a local build)")
    linked = P.arena()
    block = linked.blocks["patches.code"]
    data = [linked.blocks[name] for name in ("patches.data", "patches.state")]
    sym = linked.symbols
    declarations = {h["entry"]: h for h in P.hooks_026g.hooks(exe=P.hooks_026g.original_reader(original))
                    if any(m["patch"] == "ragdoll" for m in h.get("chain", [h]))}
    hooks = [h for h in P.rust_hooks(original) if h[2] in declarations]
    check(len(hooks) == 9 and {h[0] for h in hooks} == {0x1F7C3C, 0x24757C, 0x247716, 0x1F3643, 0x32D627, 0x24754F,
                                                       0x2477DF, 0x15A950, 0x2DA3DF},
          "the Rust patch ragdoll's nine lohooks (rust/patches/src/ragdoll.toml, sites_026g.tsv)")
    check(rows(stock, 0x2477DF, 9) == ["mov ecx,[ebp-0x1c]", "mov edx,[ebx+0xf290]"]
          and rows(stock, 0x15A94D, 10) == ["mov edx,[ebp+0x14]", "cmp byte [edi+0x62d8],0x0"],
          "headwear: before mt_draw pushes its render command; after the render bone loop, count in EDX")
    check(rows(stock, 0x24754F, 0xE) == ["mov esi,[ebx+0x128]", "test esi,esi", "jz near 0x24781e"]
          and rows(stock, 0x2477FE, 0x20)[:4] == ["mov edx,[ecx+0x5360]", "mov edi,[edx+0x200064]", "call 0x22490",
                                                  "jmp short 0x24781e"],
          "mt_draw: with no held item (+128h 0) the je at 0x247557 skips every bone update to 0x24781E, where the "
          "draw with bones joins after update_skeleton (0x24780A, EDI the renderer's +200064h)")
    for va, old, symbol, opcode in hooks:
        target = sym[symbol]
        hook = P.hook_bytes(target, va, old, 0, opcode)
        check(opcode == 0xE9 and stock.read(va, len(old)) == old == before.read(va, len(old))
              and image.read(va, len(hook)) == hook and block.va <= target < block.va + block.size,
              f"{va:#x}: {old.hex()} in the original and the build without the late join, "
              f"`jmp {target:#x}` ({symbol}, a generated entry of patches.code) and nops here")
    sites = [(s, o) for s, o, _, _ in hooks]
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(s - 4 < v < s + len(o) for s, o in sites)]
    hits = [h for s, o in sites for h in aimed_inside(original, s, len(o), 0x82)
            if not s <= h < s + len(o) and starts_instruction(stock, h)]
    check(not under and not hits, "no base relocation under the sites, and no branch aims inside one"
          + "".join(f", {h:#x}" for h in hits + under))
    code = P.linker.block_bytes(linked, "patches.code")
    # patches.data and .state hold every Rust patch's statics; their size is
    # the arena's to bound (ARENA_LIMIT, the link and test_arena)
    check(image.read(block.va, len(code)) == code,
          f"the code at {block.va:#x} is the link's patches.code ({len(code)} bytes: the Rust crate and the "
          f"generated entries)")
    # each entry: hooks_026g.lohook_entry's (the machine saved, the shim
    # called, its Flow taken), its replay the site's own bytes, then back
    for va, old, symbol, _ in hooks:
        texts = [text for _, _, text in ndisasm(image.read(sym[symbol], 0x60), sym[symbol])]
        shim = sym[declarations[symbol]["shim"]]
        check(texts[:5] == ["lea esp,[esp-0x4]", "pushf", "pusha", "cld", "mov ebp,esp"]
              and f"call {shim:#x}" in texts,
              f"{symbol}: the generated lohook entry (the flags, registers and fxsave kept, the shim {shim:#x} "
              f"called, its Flow taken)")
    # the block's .text alone: its .rodata (constants, tables) is not code
    text_size = sym.get(P.linker.rodata_symbol("patches.code"), block.va + len(code)) - block.va
    texts = [text for _, _, text in ndisasm(code[:text_size], block.va)]
    outside = sorted({branch_target(t) for t in texts if branch_target(t) is not None
                      and not block.va <= branch_target(t) < block.va + len(code)})
    library = {sym[f["symbol"]] if f["call"] in engine_026g.THUNKED else f["va"] for f in engine_026g.every_function()
               if f["call"] != "import" and (f["symbol"] in sym or f["call"] not in engine_026g.THUNKED)}
    backs = {va + len(old) for va, old, _, _ in P.rust_hooks(original)} \
        | {addr for site in P.hooks_026g.read_sites().values() for _, addr in site["exits"]}
    # the branches in the bytes a hook replaces, which its entry or cave replays
    # (patch 68's query_room: the site's `call 0x2F2060`)
    replays = {branch_target(t) for va, old, _, _ in P.rust_hooks(original)
               for _, _, t in ndisasm(old, va) if branch_target(t) is not None}
    # arena_rt exports weak intrinsics so multiple Rust arena crates share
    # them. The final ELF's full symbol map includes the selected weak
    # definitions; the public map intentionally contains strong globals only.
    runtime = {va for name, va, _, _ in linked.every if name in ("memcpy", "memmove", "memset")}
    allowed = library | backs | replays | runtime
    check(outside and set(outside) <= allowed,
          "the crate's direct branches out of the block: the engine library's functions (raw), the arena's "
          "memcpy, memmove and memset, each hook's way back past its site or to its named exit, and the "
          "branches of the bytes a hook replays (the imports go by pointer)"
          + "".join(f"; {t:#x}" for t in sorted(set(outside) - allowed)))
    stores = [t for t in texts if re.match(r"(mov|add|sub|and|or|xor|inc|dec)\w* (byte |word |dword |qword )?\[(dword )?0x", t)]
    absolute = [int(m, 16) for t in stores for m in re.findall(r"\[(?:dword )?(0x[0-9a-f]+)\]", t)]
    # an `::image` row in our own `.srpd` is an older patch's state the crate shares (patch 45's
    # weapon_table: its FLIGHT word, weapon_flight.rs's alone): a field of that row's kind
    srpd = P.late_join_state_va(open(P.late_join_bin(), "rb").read())
    shared = [(x["offset"], x["target"]) for x in engine_026g.fields() if x["module"] == "image"
              and srpd <= x["offset"] < srpd + P.LATE_JOIN_STATE_SIZE]
    shared_fields = {base + x["offset"] for base, kind in shared for x in engine_026g.fields() if x["kind"] == kind}
    # the game's own data a patch sets on purpose, by its image row: patch 81's reset of the eye
    # adaptation (stage_postprocess's byte, which the render thread reads and clears; lobby-scenes.md)
    game_flags = {x["offset"]: f'{x["field"]}' for x in engine_026g.fields() if x["module"] == "image"
                  and x["field"] in ("postprocess",)}
    inside = lambda a: any(d.va <= a < d.va + d.size for d in data) or a in shared_fields or a in game_flags
    check(all(inside(a) for a in absolute),
          f"its stores to absolute addresses ({len(absolute)}) are all in patches.data and .state, or fields of "
          f"an older patch's state it shares (`.srpd`: {', '.join(f'{b:#x} {k}' for b, k in shared)}), or a flag "
          f"of the game's a patch sets on purpose ({', '.join(f'{a:#x} {n}' for a, n in game_flags.items())}): "
          "nothing else of the game's is written by address" + "".join(f"; {a:#x}" for a in absolute if not inside(a)))


def test_spot(original, image, before):
    """Check 38: spotting (patch 38, SPOT_*). check_spot_026g.py runs the
    input apply with the game's own spottable code on made-up hits."""
    stock = Image(original)
    start, end = 0x2D86E0, 0x2D8AC8
    rows = ndisasm(stock.read(start, end - start), start)
    at = {address: text for address, _, text in rows}
    check([at.get(a) for a in (0x2D877C, 0x2D8782, 0x2D8787, 0x2D878F, 0x2D8795, 0x2D87A1, 0x2D87A7, 0x2D87AC,
                               0x2D87C1, 0x2D87E1, 0x2D87EB)]
          == ["mov eax,[edi+0x330]", "shr eax,byte 0x1c", "test al,0x1", "cmp [edi+0x34c],esi", "jnz near 0x2d88b1",
              "fld dword [dword 0xa710d0]", "push dword 0x80a", "push dword 0xffff",
              "mov dword [edi+0x34c],0xbb8", "mov edx,[edx+0x4c]", "call edx"]
          and stock.read(0xA710D0, 4) == struct.pack("<f", 100.0)
          and stock.read(0x2421A6, 10) == bytes.fromhex("818b3c03000000000010"),
          "the original's spot: bit 28 (the input builder's on the key going down, 0x2421A6), the wait +34Ch at "
          "0, a 100 m ray (0xa710d0) with filter 80Ah; the wait is 3000 at 0x2D87C1 before the ray, hit or miss")
    check([at.get(a) for a in (0x2D8839, 0x2D884E, 0x2D885C, 0x2D8872, 0x2D888D, 0x2D888F, 0x2D8A0F, 0x2D8A11,
                               0x2D8A14, 0x2D8A16, 0x2D8A1F, 0x2D8A41, 0x2D8A58, 0x2D8A65, 0x2D8A8F, 0x2D8AA7)]
          == ["test dword [esp+0x10],0x800", "mov eax,[edx+0x8]", "jz near 0x2d8a0f", "call eax", "call eax",
              "xor esi,esi", "mov ecx,[esi]", "mov ecx,[ecx+0xc]", "mov edx,[ecx]", "mov eax,[edx+0x4]",
              "jz 0x2d8a8f", "call 0x2dc390", "call eax", "call eax", "and dword [esp+0x10],0xf7ff",
              "jz near 0x2d888f"],
          "the hits' walk: a spottable (800h) hit's owner slot 2 (a player, spotted at 0x2D888D), else at 0x2D8A0F "
          "slot 1 (a geometry, its first spottable spotted at 0x2D8A65); not spotted: 800h out of the mask "
          "(0x2D8A8F); the end 0x2D888F")
    countdown = ndisasm(stock.read(0x2DAB44, 0x25), 0x2DAB44)
    check([text for _, _, text in countdown][:10] == ["mov eax,[ebx+0x34c]", "cmp eax,ebp", "jna 0x2dab5d", "pop edi",
                                                     "sub eax,ebp", "pop ebp", "mov [ebx+0x34c],eax", "pop ebx",
                                                     "pop ecx", "ret 0x4"]
          and stock.read(0x2DAB5D, 10) == bytes.fromhex("5f33c05d89834c030000")
          and stock.read(0x2DB34D, 6) == bytes.fromhex("8d834c030000") and stock.read(0x2DB354, 1) == b"\xE8"
          and stock.read(0x2DB95C, 6) == bytes.fromhex("898b4c030000")
          and stock.read(0x2DAE6B, 6) == bytes.fromhex("899e4c030000"),
          "the wait: counted down by the player's tick to 0 at the least (0x2DAB44), kept by "
          "base_player::serialize (0x2DB34D) and read back (0x2DB95C), reset at a spawn (0x2DAE6B)")
    u32 = lambda va: struct.unpack("<I", stock.read(va, 4))[0]
    stores = [P.TEXT_VA + m.start() - P.TEXT_FILE - 3 for m in re.finditer(re.escape(struct.pack("<I", P.SPOT_OWNER_VTABLE)), original)
              if P.TEXT_FILE <= m.start() < P.TEXT_FILE + P.TEXT_VSIZE]
    check(rtti_name(stock, 0xA13EE0) == (".?AVvictory_item_core@survarium@@", 0x54)
          and rtti_name(stock, P.SPOT_OWNER_VTABLE) == (".?AVvictory_item_core@survarium@@", 0x58)
          and (u32(0xA13EE0), u32(0xA13EE4)) == (0x301C40, 0x301CA0) and stock.read(0x301CA0, 5) == bytes.fromhex("b001c20400")
          and stock.read(0x301C95, 3) == bytes.fromhex("c20400")
          and u32(P.SPOT_OWNER_VTABLE + 4) == u32(P.SPOT_OWNER_VTABLE + 8) == 0x300040
          and stock.read(0x300040, 3) == bytes.fromhex("33c0c3")
          and stores == [0x30011D, 0x3004EA]
          and stock.read(0x30011D, 3) == bytes.fromhex("c74358") and stock.read(0x3004EA, 3) == bytes.fromhex("c74558")
          and stock.read(0x301CC0, 3) == bytes.fromhex("8d4658") and stock.read(0x301CCD, 3) == bytes.fromhex("89410c")
          and stock.read(0x301CF5, 10) == bytes.fromhex("6818040000" "68180a0000"),
          "victory_item_core: +54h 0xa13ee0 (set_spotted 0x301c40, can_be_spotted_by 0x301ca0 `mov al,1`, both "
          "`ret 4`), +58h 0xa13eec, its rigid body's owner (0x301cc0, group 0a18h at 0x301cf5), slots 1 and 2 "
          f"0x300040 `xor eax,eax; ret`, stored by the two constructors alone ({', '.join(hex(a) for a in stores)})")
    va, old, new = P.SPOT_COOLDOWN
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new
          and ndisasm(new, va)[0][2] == "mov dword [edi+0x34c],0x0"
          and [i for i in range(len(old)) if old[i] != new[i]] == [6, 7],
          f"{va:#x}: `mov dword [edi+0x34c],0xbb8` in the original and the build without the late join, "
          "`mov dword [edi+0x34c],0x0` here (two bytes differ)")
    va, old, offset, opcode = P.SPOT_HOOK
    hook = P.hook_bytes(P.SPOT_VA, va, old, offset, opcode)
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(hook)) == hook,
          f"{va:#x}: `mov ecx,[esi]; mov ecx,[ecx+0xc]` in the original and the build without the late join, "
          f"`jmp {P.SPOT_VA:#x}` here")
    code = open(P.spot_bin(), "rb").read()
    check(image.read(P.SPOT_VA, len(code)) == code,
          f"the code at {P.SPOT_VA:#x} is spot_026g.section.bin ({len(code)} bytes)")
    listing = [row for row in ndisasm(code, P.SPOT_VA) if row[2] != "int3"]
    inside = lambda target: P.SPOT_VA <= target < P.SPOT_VA + len(code)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    starts = {address for address, _, _ in listing}
    texts = [text for _, _, text in listing]
    calls = [text for text in texts if text.startswith("call")]
    absolute = [text for text in texts if re.search(r"0x[0-9a-f]{5,}", text) and branch_target(text) is None]
    check({t for t in targets if not inside(t)} == set(P.SPOT_EXITS) and all(t in starts for t in targets if inside(t))
          and texts[:2] == ["mov ecx,[esi]", "mov ecx,[ecx+0xc]"] and calls == ["call [eax+0x4]", "call [eax]"]
          and absolute == [f"cmp dword [ecx],{P.SPOT_OWNER_VTABLE:#x}"]
          and set(code[listing[-1][0] - P.SPOT_VA + 5:]) <= {0xCC},
          "the code begins with the two instructions it replaces, leaves only to 0x2d888f, 0x2d8a8f and 0x2d8a14, "
          "calls only the battery's can_be_spotted_by and set_spotted through its vtable, holds one absolute "
          "value (0xa13eec), every branch inside lands on an instruction, int3 after it")
    for va, size in ((P.SPOT_COOLDOWN[0], len(P.SPOT_COOLDOWN[1])), (P.SPOT_HOOK[0], len(P.SPOT_HOOK[1]))):
        hits = aimed_inside(original, va, size, 0x80)
        spelled = [va + i for i in range(1, size) if original.find(struct.pack("<I", va + i)) != -1]
        under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + size]
        check(len(hits) == len(spelled) and not under,
              f"{va:#x}: nothing aims inside the site and no base relocation is under it"
              + "".join(f", {h:#x}" for h in hits + under))


def imports(image):
    """{IAT slot VA: (dll, function)} of the import table and the delay-load
    table (its addresses are VAs in this exe)."""
    out = {}
    cstr = lambda va: bytes(image.data[image.offset(va):image.data.index(b"\0", image.offset(va))]).decode()
    rva, _ = image.directory(1)
    at = P.IMAGE_BASE + rva
    while True:
        lookup, _, _, name, first = struct.unpack("<5I", image.read(at, 20))
        if not name:
            break
        for index in itertools.count():
            thunk = struct.unpack("<I", image.read(P.IMAGE_BASE + (lookup or first) + 4 * index, 4))[0]
            if not thunk:
                break
            if not thunk & 0x80000000:
                out[P.IMAGE_BASE + first + 4 * index] = (cstr(P.IMAGE_BASE + name), cstr(P.IMAGE_BASE + thunk + 2))
        at += 20
    rva, _ = image.directory(13)
    at = P.IMAGE_BASE + rva
    while True:
        attributes, name, _, iat, names, _, _, _ = struct.unpack("<8I", image.read(at, 32))
        if not name:
            break
        fix = (lambda v: P.IMAGE_BASE + v) if attributes & 1 else (lambda v: v)
        for index in itertools.count():
            thunk = struct.unpack("<I", image.read(fix(names) + 4 * index, 4))[0]
            if not thunk:
                break
            if not thunk & 0x80000000:
                out[fix(iat) + 4 * index] = (cstr(fix(name)), cstr(fix(thunk) + 2))
        at += 32
    return out


def test_tab_list(original, image, before):
    """Check 39: the scoreboard shows the places in the match and no place
    whose player left (patch 39, TAB_LIST_*). check_tab_list_026g.py runs
    the rows' refresh and the lists made again on made-up huds."""
    stock = Image(original)
    rows = lambda blob, at, size: [text for _, _, text in ndisasm(blob.read(at, size), at)]
    # What `online` is: kd_stats_rule's +114h + 8 x place, set on entering,
    # cleared on leaving (vtable 0xA177D4: +20h, +24h), copied by
    # fill_match_hud_state (+44h) into the hud's state at +9Ch + 8 x place.
    slots = struct.unpack("<3I", stock.read(0xA177F4, 8) + stock.read(0xA17818, 4))
    fill = rows(stock, 0x323980, 0x50)
    check(slots == (0x3239F0, 0x323A00, 0x323980)
          and rows(stock, 0x3239F0, 0x10) == ["movzx eax,byte [esp+0x4]", "mov byte [ecx+eax*8+0x114],0x1",
                                             "ret 0x8"]
          and rows(stock, 0x323A00, 0x10) == ["movzx eax,byte [esp+0x4]", "mov byte [ecx+eax*8+0x114],0x0",
                                             "ret 0x8"]
          and "mov bl,[ecx+eax*8+0x114]" in fill and "mov [esi+eax*8+0x9c],bl" in fill,
          "kd_stats_rule's on_player_entered_match (vtable 0xA177D4 +20h, 0x3239F0) sets the place's online byte "
          "(+114h + 8 x place), on_player_left_match (+24h, 0x323A00) clears it, fill_match_hud_state (+44h, "
          "0x323980) copies it into the hud's state at +9Ch + 8 x place")
    left = rows(stock, 0x2D2119, 0x31)
    check(left[:2] == ["mov esi,[ebp+0x18824]", "mov ebp,[ebp+0x18828]"] and "mov edx,[edx+0x24]" in left
          and "call edx" in left,
          "game_world_core::on_player_left_match (0x2D2040) calls slot +24h of every rule of the core "
          "(+18824h..+18828h)")
    check(rows(stock, 0x23B2D2, 0xB) == ["lea eax,[edi+0x24]", "lea ecx,[edi+0x2d8]", "cmp eax,ecx"],
          "update_ui (0x23B200) hashes the hud's state, hud+24h..+2D8h: online of place i is hud+0C0h + 8i")
    # The sites.
    for va, old, offset, opcode in P.TAB_LIST_HOOKS:
        new = P.late_join_hook(va, old, offset, opcode)
        check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new,
              f"{va:#x}: the original's {len(old)} bytes in the original and the build without the late join, "
              f"`call {P.LATE_JOIN_VA + offset:#x}` and nops here")
    va, old, new = P.TAB_LIST_LOOP
    check(stock.read(va, len(old)) == old == before.read(va, len(old)) and image.read(va, len(new)) == new,
          f"{va:#x}: the rows' refresh is the original's in the original and the build without the late join, "
          "rewritten here")
    check(rows(stock, va, len(old)) == ["movzx eax,bl", "mov edx,eax", "movzx eax,byte [edi+eax*8+0xc0]",
                                         "imul edx,edx,dword 0x45fc", "add ecx,edx", "push eax", "lea eax,[ecx+0x48]",
                                         "add ecx,byte +0x8", "push edi", "call 0x23a7f0", "mov ecx,[esp+0x14]",
                                         "push ecx", "call 0x236d30", "inc bl", "jmp short 0x23b42c"]
          and rows(image, va, len(new)) == ["movzx eax,bl", "imul edx,eax,dword 0x45fc",
                                             "movzx eax,byte [edi+eax*8+0xc0]", "test al,al", "jz 0x23b471",
                                             "add ecx,edx", "push eax", "lea eax,[ecx+0x48]", "add ecx,byte +0x8",
                                             "push edi", "call 0x23a7f0", "push ebx", "call 0x236d30", "inc bl",
                                             "jmp short 0x23b42c", "nop", "nop"],
          "the loop body: set_online_player (0x23A7F0) with the place's online byte and list_update_player "
          "(0x236D30) for every place in the original; here only for a place whose byte is not 0, the same "
          "arguments (the place from ebx, whose low byte is the one stored at [esp+14h])")
    check(stock.read(0x23B421, 9) == bytes.fromhex("80bfba00000000744d")
          and image.read(0x23B428, 2) == bytes.fromhex("744d"),
          "0x23B421: the shown flag's test is followed by `jz 0x23b477`, which stays (the call sets its flags)")
    check(rows(stock, 0x23536E, 0x1C) == ["mov ecx,[esp+0x12c]", "mov edx,[ecx]", "mov edx,[edx+0x34]",
                                          "lea eax,[esp+0x58]", "push eax", "mov eax,[esp+0x138]", "push ebx",
                                          "push eax", "call edx"]
          and rows(stock, 0x234F04, 0x2B)[-3:] == ["mov eax,[eax+0x3c]", "push edx", "call eax"],
          "0x23537A: the scoreboard's row goes in by its array's SetElement (vtable+34h: data, the place, the row); "
          "the enemies' ids go in by PushBack (vtable+3Ch: data, the value, 0x234F2D)")
    code = open(P.late_join_bin(), "rb").read()
    for offset, start, size, wanted in ((P.TAB_ROW_OFFSET, P.TAB_ROW_START, 0x2F, {P.LATE_JOIN_VA + 2304}),
                                        (P.TAB_SHOWN_OFFSET, P.TAB_SHOWN_START, 0x62,
                                         {P.LATE_JOIN_VA + 2304, P.LATE_JOIN_VA + 3328})):
        at = P.LATE_JOIN_VA + offset
        listing = ndisasm(code[offset:offset + size], at)
        calls = {branch_target(text) for _, _, text in listing if text.startswith("call 0x")}
        leaving = [text for _, _, text in listing if branch_target(text) is not None
                   and not text.startswith("call") and not at <= branch_target(text) < at + size]
        check(code[offset:offset + len(start)] == start and image.read(at, size) == code[offset:offset + size]
              and calls == wanted and not leaving and listing[-1][2] == "ret"
              and not [t for _, _, t in listing if re.search(r"\[(?:dword )?0x", t)],
              f"{at:#x}: the code calls get_state{' and rebuild_lists' if len(wanted) > 1 else ''} alone, leaves "
              "by its `ret`, and holds no absolute address (the state page is found from its own address)")
    hook = ndisasm(code[3456:3520], P.LATE_JOIN_VA + 3456)
    back = P.LATE_JOIN_VA + P.TAB_SHOWN_OFFSET + code[P.TAB_SHOWN_OFFSET:].index(bytes.fromhex("57e8")) + 6
    pc = P.LATE_JOIN_VA + 3456 + 5
    adds = [int(text.split(",")[1], 16) for _, _, text in hook if text.startswith("add ecx,")]
    check(len(adds) == 2 and (pc + adds[0]) & 0xFFFFFFFF == P.LATE_JOIN_VA + 1120 + 100
          and (pc + adds[0] + adds[1]) & 0xFFFFFFFF == back and code[back - P.LATE_JOIN_VA:][:2] == b"\x8f\x06",
          "lists_hook ends a run of the lists whose return address is install_context's or tab_shown's (the "
          "instruction after its call of rebuild_lists)")
    sites = [(h[0], h[1]) for h in P.TAB_LIST_HOOKS] + [(va, old)]
    # A rel8 branch byte near a site counts when it is an instruction of the
    # host function's own listing; a 4-byte value spelling an address
    # inside is no pointer without a relocation (none is under the sites).
    spelled = {site + i for site, bytes_ in sites for i in range(1, len(bytes_))
               if original.find(struct.pack("<I", site + i)) != -1}
    hosts = ((0x234E71, 0x23572B), (0x23B200, 0x23B4A0))
    starts = {address for lo, hi in hosts for address, _, _ in ndisasm(stock.read(lo, hi - lo), lo)}
    found = [a for site, bytes_ in sites for a in aimed_inside(original, site, len(bytes_), 0x82)
             if a not in spelled and (a in starts or not any(lo <= a < hi for lo, hi in hosts))]
    under = [v for kind, v in stock.relocations().values() if kind == 3
             and any(site - 4 < v < site + len(bytes_) for site, bytes_ in sites)]
    check(not found and not under, "nothing aims inside the three sites (no branch of .text, and no 4-byte value "
          "that is a pointer) and no base relocation is under them" + "".join(f", {h:#x}" for h in found + under))


def test_tab_self(original, image, before):
    """Check 43: the player's own row in the Tab list is gold (patch 43,
    TAB_SELF): game_world::on_activate gives the hud its player's name
    whatever the journal does. check_tab_self_026g.py runs it. Every full
    build has it, as it has patch 14's default (test_start_up_folder)."""
    stock = Image(original)
    rows = lambda blob, at, size: [text for _, _, text in ndisasm(blob.read(at, size), at)]
    va, old, new = P.TAB_SELF
    check(stock.read(va, len(old)) == old and before.read(va, len(new)) == new == image.read(va, len(new)),
          f"{va:#x}: the original's `{old.hex()}` (test eax,eax), `{new.hex()}` (jmp short) here and in the "
          "build without the late join")
    guard = ["mov eax,[0xa8394c]", "test eax,eax", "jz 0x1f949d", "mov ecx,[dword 0xa83948]",
             "cmp dword [ecx],byte +0x1", "jnz 0x1f949d", "mov edx,[esi+0x9c]", "mov eax,[edx+0x5398]", "push eax",
             "lea edi,[esp+0x24]", "call 0x209ac0", "mov ecx,[dword 0xa8394c]"]
    check(rows(stock, 0x1F9473, 0x30) == guard,
          "the original: 0x1F9473 calls chat_handler::set_local_player_name (0x209AC0) with the chat handler "
          "(game+5398h) and the name at [esp+24h] only when the journal's flag [0xa8394c] is set and its use "
          "[[0xa83948]] is 1 (record)")
    check(rows(image, 0x1F9473, 0x30) == guard[:1] + ["jmp short 0x1f9487"] + guard[2:],
          "here `jmp short 0x1f9487` past the two tests; the call and what follows are the original's")
    # The name it gives is the rows' name: "[clan] name" (the format at
    # 0x9FB140), or the name, from the lobby client (+1E5650h the clan,
    # +1E55D0h the name), as initialize_match makes a row's from the place's
    # profile (+48h the clan, +8 the name).
    activate = rows(stock, 0x1F93A1, 0xD2)
    initialize = rows(stock, 0x23506F, 0x40)
    check(stock.read(0x9FB140, 8) == b"[%s] %s\0"
          and "add eax,0x1e5650" in activate and "add eax,0x1e55d0" in activate
          and activate.count("push dword 0x9fb140") == 1 and activate.count("call 0x1f53d0") == 1
          and "call 0x209430" in activate
          and initialize[0] == "lea ecx,[esi+0x48]" and "add esi,byte +0x8" in initialize
          and "push dword 0x9fb140" in initialize,
          "on_activate (after chat_handler::set_mode 0x209430) makes the name as the rows have it: \"[%s] %s\" "
          "(0x9FB140) of the lobby client's clan and name, or the name; initialize_match's row name is the same "
          "format of the place's clan (+48h) and name (+8)")
    check(rows(stock, 0x209AF9, 0xA) == ["push dword 0x9fe534", "call 0x511600"]
          and stock.read(0x9FE534, 22) == b"root.set_local_player\0",
          "0x209AC0 invokes root.set_local_player (0x9FE534) on the chat handler's movie, the hud in a match")
    check(rows(stock, 0xEB377, 7) == ["mov eax,0xa8394c", "xchg [eax],edx"] and rows(stock, 0xEB372, 5) == ["mov edx,0x1"]
          and rows(stock, 0xEB317, 4) == ["test esi,esi", "jz 0xeb37e"]
          and image.read(P.JOURNAL_DEFAULT_VA, 5) == P.JOURNAL_DEFAULT[2],
          "the flag [0xa8394c] is set by journaling's start (0xEB310) for a use other than 0 alone, and the "
          "build's journal is off by default (patch 14): without the patch the call never comes")
    under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    found = aimed_inside(original, va, len(old), 0x82)
    check(not under and not found and sorted(v for kind, v in stock.relocations().values()
                                               if kind == 3 and 0x1F9473 <= v < 0x1F9487) == [0x1F9474, 0x1F947E],
          "no base relocation under the two bytes (the two addresses' are at 0x1F9474 and 0x1F947E) and nothing "
          "aims at the second byte" + "".join(f", {h:#x}" for h in found + under))


def test_weapon_show(original, image, before):
    """Check 91: a held weapon shown while its player aims keeps its body
    (patch 91, WEAPON_SHOW): weapon::on_show always takes its idle branch
    (the idle sight and the body), and the next weapon::draw aims.
    check_weapon_show_026g.py runs it. Every full build, as patch 43."""
    stock = Image(original)
    rows = lambda blob, at, size: [text for _, _, text in ndisasm(blob.read(at, size), at)]
    va, old, new = P.WEAPON_SHOW
    check(stock.read(va, len(old)) == old and before.read(va, len(new)) == new == image.read(va, len(new)),
          f"{va:#x}: the original's `{old.hex()}` (jz near), `{new.hex()}` (jmp, nop) here and in the build without "
          "the late join")
    head = ["cmp byte [esi+0x181e],0x0"]
    aimed = ["mov eax,esi", "call 0x243940", "test al,al", "jz near 0x243ec7"]
    check(rows(stock, 0x243E2B, 0x1C) == head + ["jz near 0x243ec7"] + aimed,
          "the original: on_show takes its aimed branch when +181Eh is set and 0x243940 (the local player's, in "
          "first person) says so")
    check(rows(image, 0x243E2B, 0x1C) == head + ["jmp 0x243ec7", "nop"] + aimed,
          "here `jmp 0x243ec7`: the idle branch always; the aimed branch's bytes stay, unreached")
    branch = rows(stock, 0x243E47, 0x243EC7 - 0x243E47)
    idle = rows(stock, 0x243EC7, 0x243F3C - 0x243EC7)
    check(branch[:12] == ["mov edx,[esi+0x180c]", "lea eax,[esi+0x9bc]", "push eax", "push eax", "mov eax,[edi+0x10c]",
                          "lea ecx,[esp+0x14]", "push ecx", "mov ecx,[edx+0x9c]", "mov edx,[ecx+0x5360]",
                          "mov ecx,[edx+0x200064]", "add eax,0x108", "push ecx"]
          and branch[12] == "call 0x21cd0" and branch[-2:] == ["mov byte [esi+0x181d],0x1", "jmp short 0x243efd"]
          and "mov eax,[edi+0x108]" in idle and idle[idle.index("cmp byte [esi+0x181d],0x0") + 1] == "jnz 0x243f3c"
          and "mov eax,[esi+0x1810]" in idle and idle[-1] == "call 0x21cd0",
          "the aimed branch adds the optic's aimed model (+10Ch) and sets +181Dh; the idle one adds the idle model "
          "(+108h); then the body (+1810h) is added only while +181Dh is 0 (0x243EFD)")
    # draw's scope: the local player's in first person with +181Eh and not
    # +181Dh aims (add_aimed_rifle_scope_to_scene); +181Dh without +181Eh
    # (or not in first person) goes back (remove_aimed_rifle_scope_from_scene).
    check(rows(stock, 0x244F80, 0x244FBF - 0x244F80) == [
              "mov eax,ebx", "call 0x243940", "test al,al", "jz 0x244fad", "mov al,[ebx+0x181e]", "test al,al",
              "jz 0x244fad", "cmp byte [ebx+0x181d],0x0", "jnz 0x244fa9", "lea edi,[esp+0x10]", "call 0x244b60",
              "jmp short 0x244fbf", "test al,al", "jnz 0x244fbf", "cmp byte [ebx+0x181d],0x0", "jz 0x244fbf",
              "lea edi,[esp+0x10]", "call 0x244c50"],
          "weapon::draw (0x244F80) aims when the weapon is the local player's in first person, +181Eh is set and "
          "+181Dh is not, and goes back when +181Dh is set otherwise")
    add = rows(stock, 0x244B60, 0x244C48 - 0x244B60)
    check(add.count("call 0x21f40") == 2 and add.count("call 0x21cd0") == 1
          and add.index("mov eax,[eax+0x108]") < add.index("call 0x21f40") < add.index("call 0x21cd0")
          and add[add.index("cmp byte [eax+0x11c],0x0") + 1:add.index("cmp byte [eax+0x11c],0x0") + 5]
          == ["jz 0x244c3f", "mov eax,esi", "call 0x243940", "test al,al"]
          and "mov eax,[esi+0x1810]" in add and add[-3:] == ["mov byte [esi+0x181d],0x1", "pop ecx", "ret"],
          "add_aimed_rifle_scope_to_scene (0x244B60) takes the idle model out, adds the aimed one, takes the body "
          "out only for hide_weapon_on_aim (+11Ch) in first person, and sets +181Dh")
    # Nothing on_show, draw or the two scope functions run writes +181Eh.
    writes = [r for at, size in ((0x243DD0, 0x243F94 - 0x243DD0), (0x244B60, 0x244D35 - 0x244B60),
                                 (0x244E10, 0x245241 - 0x244E10)) for r in rows(image, at, size)
              if "+0x181e]" in r.split(",")[0] and not r.startswith(("cmp ", "test "))]
    # weapon::deserialize's three writes, read the same way (the filter sees a write).
    known = [r for r in rows(stock, 0x244A40, 0x244AA4 - 0x244A40)
             if "+0x181e]" in r.split(",")[0] and not r.startswith(("cmp ", "test "))]
    check(not writes and known == ["mov [edi+0x181e],dl", "mov [edi+0x181e],al", "mov [edi+0x181e],al"],
          "on_show, draw and the scope's add and remove never write +181Eh (the world's; weapon::deserialize "
          "0x244A40 writes it three times)")
    under = [v for kind, v in stock.relocations().values() if kind == 3 and va - 4 < v < va + len(old)]
    found = aimed_inside(original, va, len(old), 0x82)
    check(not under and not found, "no base relocation under the six bytes and nothing aims inside them"
          + "".join(f", {h:#x}" for h in found + under))


def test_replay_link(original, image, before):
    """"Watch replay" (patch revision 14, REPLAY_LINK_HOOK): the lobby's
    video_tutorial_link_clicked opened its link buffer with ShellExecuteA;
    the build jumps to replay_link instead, which starts updater_skhid.exe
    beside the exe for a replay link alone. check_replay_link_026g.py runs
    the handler on made-up links."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    va, old, offset, _ = P.REPLAY_LINK_HOOK
    cave = P.REPLAY_LINK_VA + offset
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
          and rows(image, va, 6) == [f"jmp {cave:#x}", "nop"],
          f"{va:#x}: `push 1 / push 0 / push 0` in the original and the build without the late join, "
          f"`jmp {cave:#x}` here")
    check(rows(stock, P.REPLAY_LINK_HANDLER, 0x43) ==
          ["push esi", "mov esi,ecx", "mov eax,[esi+0x9c]", "mov ecx,[eax+0x53ac]", "mov edx,[ecx]",
           "mov eax,[edx+0x3c]", "call eax", "push byte +0x1", "push byte +0x0", "push byte +0x0",
           "add eax,0x1e6144", "push eax", "push dword 0xa0a7e8", "push byte +0x0", "call [dword 0xaefe74]",
           "push byte +0x1", "push dword 0xa09e20", "mov eax,esi", "call 0x22a660", "mov al,0x1", "pop esi",
           "ret 0x4"],
          "the handler 0x28CDA0: the lobby client through its owner's vtable+3Ch, ShellExecuteA(0, \"open\", "
          "client+1E6144h, 0, 0, SW_SHOWNORMAL), enable_element, al = 1, `ret 4`")
    table = imports(stock)
    text_at = lambda at: bytes(original[stock.offset(at):original.index(b"\0", stock.offset(at))])
    check(table.get(0xAEFE74) == ("SHELL32.dll", "ShellExecuteA") and text_at(0xA0A7E8) == b"open"
          and text_at(0xA09E20) == b"root.statusPanel.videoTutorial",
          "0xAEFE74 is the delay-loaded SHELL32 ShellExecuteA; 0xA0A7E8 \"open\"; 0xA09E20 the tutorial element")
    names = struct.unpack("<I", stock.read(0x9C56C8 + 106 * 4, 4))[0]
    found, at = [], 0
    needle = struct.pack("<I", P.REPLAY_LINK_HANDLER)
    while (at := original.find(needle, at)) != -1:
        found.append(at)
        at += 1
    check(text_at(names) == b"video_tutorial_link_clicked"
          and [P.TEXT_VA + at - P.TEXT_FILE for at in found] == [0x283054]
          and rows(stock, 0x283053, 5) == ["mov eax,0x28cda0"]
          and not [h for h in branches_into(original, P.REPLAY_LINK_HANDLER, P.REPLAY_LINK_HANDLER + 1)
                   if h.endswith("rel32")],
          "entry 106 of the callback names (0x9C56C8) is video_tutorial_link_clicked; the handler's address is "
          "in the file once, the delegate's `mov eax,0x28cda0` at 0x283053; no call or jump goes to it")
    uses = []
    needle, at = struct.pack("<I", P.REPLAY_LINK_BUFFER), 0
    while (at := original.find(needle, at)) != -1:
        uses.append(P.TEXT_VA + at - P.TEXT_FILE)
        at += 1
    check(uses == [0x26F351, 0x271428, 0x271447, 0x2768A3, 0x28CDBD]
          and rows(stock, 0x26F34F, 6) == ["mov [ebp+0x1e6144],bl"]
          and rows(stock, 0x271426, 6) == ["lea edx,[ebp+0x1e6144]"]
          and rows(stock, 0x271439, 0x13) == ["push dword 0xfde9", "call [dword 0x8701e0]",
                                              "mov byte [eax+ebp+0x1e6144],0x0"]
          and table.get(0x8701E0) == ("KERNEL32.dll", "WideCharToMultiByte")
          and rows(stock, 0x2768A1, 7) == ["cmp byte [eax+0x1e6144],0x0"],
          "the link buffer (lobby client+1E6144h): emptied at 0x26F34F, filled by 9E as UTF-8 (CP 65001) "
          "with its zero (0x271426..0x271444), tested for empty at 0x2768A1, opened by the handler alone")
    hits = branches_into(original, va + 1, va + len(old))
    check(not hits, "no branch bytes or pointer values aim inside the hook" + "".join(", " + h for h in hits))
    under = [rva for kind, rva in stock.relocations().values() if kind == 3 and rva + 4 > va and rva < va + len(old)]
    check(not under, f"no base relocation under {va:#x}")
    code = open(P.replay_link_bin(), "rb").read()
    body = code[:code.index(b"replay ")]
    listing = ndisasm(body, cave)
    texts = [text for _, _, text in listing]
    pc = cave + 6
    slots = set()
    for _, _, text in listing:
        match = re.fullmatch(r"call \[ebp-0x([0-9a-f]+)\]", text)
        if match:
            slots.add((pc - int(match.group(1), 16)) & 0xFFFFFFFF)
    branches = {branch_target(text) for text in texts if branch_target(text) is not None}
    check(texts[:3] == ["pusha", f"call {pc:#x}", "pop ebp"] and texts[-4:] == ["popa", "mov al,0x1", "pop esi", "ret 0x4"]
          and all(cave <= target < cave + len(body) for target in branches)
          and {text for text in texts if text.startswith("call") and branch_target(text) is None}
          == {f"call [ebp-{pc - slot:#x}]" for slot in (0x870074, 0x87003C, 0x870084, 0x870040)} | {"call edx"}
          and {s: table.get(s) for s in slots} == {
              0x870074: ("KERNEL32.dll", "GetModuleFileNameW"), 0x87003C: ("KERNEL32.dll", "GetModuleHandleA"),
              0x870084: ("KERNEL32.dll", "GetProcAddress"), 0x870040: ("KERNEL32.dll", "CloseHandle")}
          and code[len(body):] == (b"replay \x88 \xa0 \xc0 \0"
                                   + "updater_skhid.exe\0".encode("utf-16-le") + b"kernel32.dll\0CreateProcessW\0"),
          "replay_link: pushad, its own address in ebp, the game's GetModuleFileNameW, GetModuleHandleA, "
          "GetProcAddress (for CreateProcessW) and CloseHandle through their IAT slots, no branch out but its "
          "return (`popad / mov al,1 / pop esi / ret 4`); the link's pattern and the program's fixed name")


def test_shared_upgrades(original, image, before):
    """Check 31: one upgrade on several items (patch 30, SHARED_UPGRADES_HOOKS).
    The lobby's `46 02` and the answer `83 46 02` named no item, and the
    answer took the upgrade off the first item holding it; the build's
    four sites call or jump into shared_upgrades_026g.asm, which names the
    item. check_shared_upgrades_026g.py runs the original's and the build's
    code on inventories with an upgrade on several items."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    stock_texts = {
        0x29AAD5: ["push edi", "push ecx", "call 0x2708c0"],
        0x275756: ["mov ecx,[eax]", "push ecx", "add eax,byte +0x4", "push ebp", "mov [esi+0x4],eax",
                   "call 0x275220"],
        0x275589: ["push edi", "push ebp", "call 0x275220"],
        0x27534D: ["call 0x277cf0"],
    }
    relocations = [rva for kind, rva in stock.relocations().values() if kind == 3]
    for va, old, offset, opcode in P.SHARED_UPGRADES_HOOKS:
        word = "call" if opcode == 0xE8 else "jmp"
        entry = P.SHARED_UPGRADES_VA + offset
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows(stock, va, len(old)) == stock_texts[va]
              and rows(image, va, len(old)) == [f"{word} {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(stock_texts[va])}` in the original and the build without the late join, "
              f"`{word} {entry:#x}`" + (f" and {len(old) - 5} nops" if len(old) > 5 else "") + " here")
        hits = branches_into(original, va + 1, va + len(old))
        under = [rva for rva in relocations if rva + 4 > va and rva < va + len(old)]
        check(not hits and not under, f"{va:#x}: no branch or address aims inside the site, no base relocation "
              "is under it" + "".join(", " + h for h in hits))
    calls = lambda target: [h for h in branches_into(original, target, target + 1) if h.endswith("rel32")]
    check(calls(0x2708C0) == ["0x29aad7 rel32"] and calls(P.PROCESS_DETACH_UPGRADE) == ["0x27558b rel32", "0x275760 rel32"]
          and calls(0x277CF0) == ["0x27534d rel32"]
          and not [h for h in branches_into(original, P.PROCESS_DETACH_UPGRADE, P.PROCESS_DETACH_UPGRADE + 1)
                   if not h.endswith("rel32")],
          "0x2708C0 (`46 02`) is called from drop_upgrade_item's handler alone, process_detach_upgrade 0x275220 "
          "from the `83 46 01` and `83 46 02` cases alone (and is no value anywhere), find_if 0x277CF0 from "
          "process_detach_upgrade alone")
    names = struct.unpack("<I", stock.read(0x9C56E8, 4))[0]
    text_at = lambda at: bytes(original[stock.offset(at):original.index(b"\0", stock.offset(at))])
    found, at = [], 0
    while (at := original.find(struct.pack("<I", P.DROP_UPGRADE_HANDLER), at)) != -1:
        found.append(P.TEXT_VA + at - P.TEXT_FILE)
        at += 1
    check(text_at(names) == b"drop_upgrade_item" and found == [0x2818FB]
          and rows(stock, 0x2818FA, 5) == [f"mov eax,{P.DROP_UPGRADE_HANDLER:#x}"]
          and rows(stock, 0x29A9D9, 0x11) == ["mov eax,[ebp+0x8]", "mov eax,[eax+0x8]", "push ebx", "push esi",
                                              "mov esi,[eax+0x8]", "xor ebx,ebx", "push edi", "mov edi,[eax+0x20]"]
          and rows(stock, 0x29AACF, 6) == ["mov ecx,[esi+0x4620]"]
          and rows(stock, 0x29AADC, 0xB) == ["pop edi", "pop esi", "mov al,0x1", "pop ebx", "mov esp,ebp",
                                             "pop ebp", "ret 0x4"],
          "the callback named drop_upgrade_item (0x9C56E8) is the delegate's `mov eax,0x29a9d0` at 0x2818FA, "
          "the handler's one reference; it reads the item (esi) and the upgrade (edi) from the Flash arguments "
          "([ebp+8] -> +8 -> +8 and +20h), the kit at +4620h, and after the call only pops what it pushed")
    code = open(P.shared_upgrades_bin(), "rb").read()
    check(image.read(P.SHARED_UPGRADES_VA, len(code)) == code,
          f"shared_upgrades at {P.SHARED_UPGRADES_VA:#x} is shared_upgrades_026g.section.bin ({len(code)} bytes)")
    listing = [row for row in ndisasm(code, P.SHARED_UPGRADES_VA) if row[2] != "int3"]
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not P.SHARED_UPGRADES_VA <= t < P.SHARED_UPGRADES_VA + len(code)}
    starts = {address for address, _, _ in listing}
    check(outside == {0x37A0E0, 0x7AB30, 0x379640, P.PROCESS_DETACH_UPGRADE, 0x275765, 0x277CF0}
          and all(t in starts for t in targets - outside)
          and all(P.SHARED_UPGRADES_VA + offset in starts for _, _, offset, _ in P.SHARED_UPGRADES_HOOKS)
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|0x[0-9a-f]{6,}\]", text)],
          "the code calls only new_packet, mutable_buffer::resize, enqueue and process_detach_upgrade, jumps only "
          "to the `83 46 02` case's end and the original find_if, every branch inside it lands on an instruction, "
          "and it holds no address (nothing to relocate)")


def test_read_order(original, image, before):
    """Check 34: the order of the archive reads (patch 33, READ_ORDER_*).
    check_read_order_026g.py runs the original's and the build's predicate
    and the game's sort on made-up read queries."""
    stock = Image(original)
    va, size = P.READ_ORDER_VA, P.READ_ORDER_SIZE
    code = open(P.read_order_bin(), "rb").read()
    check(hashlib.sha256(stock.read(va, size)).hexdigest() == P.READ_ORDER_SHA256
          and before.read(va, size) == stock.read(va, size) and image.read(va, size) == code and len(code) == size,
          f"{va:#x}: hdd_manager_sorting_predicate's {size:#x} bytes are the original's in the original and the build "
          "without the late join, read_order_026g.bin here")
    stock_rows = ndisasm(stock.read(va, size), va)
    rets = [address for address, _, t in stock_rows if t.startswith("ret")]
    check(stock_rows[0][2] == "sub esp,0x250" and [t for _, _, t in stock_rows if t.startswith("ret")] == ["ret 0x4"] * 8
          and rets[-1] == 0x10BE56 and set(stock.read(0x10BE59, 7)) == {0xCC},
          "the original body: `sub esp,0x250`, eight `ret 4`, the last at 0x10BE56; int3 from 0x10BE59 to the next "
          "function, 0x10BE60")
    relocations = [rva for kind, rva in stock.relocations().values() if kind == 3]
    # A byte pattern is a branch only where an instruction starts: the scan of
    # the bytes around is checked against the linear listing of the
    # predicate and grab_sorted_queries (0x10BCC0..0x10BF70).
    around = {address for address, _, _ in ndisasm(stock.read(va, 0x10BF70 - va), va)}
    hits = [h for h in branches_into(original, va + 1, va + size)
            if not (va <= int(h.split()[0], 16) < va + size)
            and (h.endswith("value") or int(h.split()[0], 16) >= 0x10BF70 or int(h.split()[0], 16) in around)]
    check(not [rva for rva in relocations if rva + 4 > va and rva < va + size] and not hits,
          f"no base relocation is in {va:#x}..{va + size:#x} and nothing outside aims inside it (a byte pattern "
          "counted where an instruction starts)" + "".join(", " + h for h in hits))
    calls = lambda target: [h for h in branches_into(original, target, target + 1)]
    helpers = sorted(int(h.split()[0], 16) for h in calls(va))
    check(len(helpers) == 15 and all(0x10BF70 <= at < 0x10C3F0 for at in helpers)
          and calls(0x10BF70) == ["0x10bf13 rel32", "0x10c025 rel32"] and calls(0x10C060) == ["0x10bf1e rel32"],
          "the predicate's fifteen calls are all in the sort's helpers 0x10BF70..0x10C3ED; the introsort 0x10BF70 is "
          "called by grab_sorted_queries (0x10BF13) and itself (0x10C025), the final insertion sort 0x10C060 by "
          "grab_sorted_queries alone (0x10BF1E)")
    listing = [row for row in ndisasm(code, va) if row[2] != "int3"]
    end = listing[-1][0] + len(listing[-1][1]) // 2
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not va <= t < va + size}
    starts = {address for address, _, _ in listing}
    check(outside == {0x4BC10, 0x3B5A0, 0x47160} and all(t in starts for t in targets - outside)
          and [t for _, _, t in listing if t.startswith("ret")] == ["ret 0x4"] and set(code[end - va:]) == {0xCC}
          and not [t for _, _, t in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", t)],
          "the new code calls only get_physical_path (0x4BC10), strcmp_s (0x3B5A0) and get_file_offs (0x47160), "
          f"returns by its one `ret 4`, every branch inside lands on an instruction, it holds no absolute address, "
          f"and int3 fills it from {end:#x}")
def test_weapon_preview(original, image, before):
    """Check 33: a weapon's upgrade window shows the weapon (patch 32,
    WEAPON_PREVIEW_HOOKS). check_weapon_preview_026g.py runs the original's
    and the build's code on a made-up lobby."""
    stock = Image(original)
    rows = lambda img, va, size: [text for _, _, text in ndisasm(img.read(va, size), va)]
    stock_texts = {
        0x265E73: ["call 0x2845c0"], 0x265F44: ["call 0x2845c0"],
        0x284B10: ["push ebp", "mov ebp,esp", "and esp,byte -0x8"],
        0x284081: ["movss xmm1,dword [dword 0xa7afa4]"], 0x28428C: ["movss xmm1,dword [dword 0xa7afa4]"],
        0x284316: ["movss xmm0,dword [dword 0xa7afa4]"],
    }
    relocations = [rva for kind, rva in stock.relocations().values() if kind == 3]
    now = [rva for kind, rva in image.relocations().values() if kind == 3]
    stash_entries_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_STASH_OFFSET
    for va, old, offset, opcode in P.WEAPON_PREVIEW_HOOKS:
        word = "call" if opcode == 0xE8 else "jmp"
        entry = P.WEAPON_PREVIEW_VA + offset
        if va in P.WEAPON_STASH_ENTRIES:          # through weapon_stash_026g.asm's entry, which goes on to patch 32's
            entry = stash_entries_va + P.WEAPON_STASH_ENTRIES[va][0]
        check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
              and rows(stock, va, len(old)) == stock_texts[va]
              and rows(image, va, len(old)) == [f"{word} {entry:#x}"] + ["nop"] * (len(old) - 5),
              f"{va:#x}: `{' / '.join(stock_texts[va])}` in the original and the build without the late join, "
              f"`{word} {entry:#x}`" + (f" and {len(old) - 5} nops" if len(old) > 5 else "") + " here")
        hits = branches_into(original, va + 1, va + len(old))
        under = [rva for rva in relocations if rva + 4 > va and rva < va + len(old)]
        expected = [va + 4] if old[:3] == b"\xf3\x0f\x10" else []
        check(not hits and under == expected and not [rva for rva in now if rva + 4 > va and rva < va + len(old)],
              f"{va:#x}: no branch or address aims inside the site; "
              + (f"the base relocation of its address ({va + 4:#x}) is disabled" if expected
                 else "no base relocation is under it") + "".join(", " + h for h in hits))
    calls = lambda target: [h for h in branches_into(original, target, target + 1)]
    check(calls(0x2845C0) == ["0x265e73 rel32", "0x265f44 rel32"]
          and calls(P.WEAPON_PREVIEW_HOOKS[2][0]) == ["0x222a42 rel32", "0x223029 rel32", "0x2850c5 rel32"]
          and calls(0x265DE0) == ["0x29a882 rel32"] and calls(0x265F10) == ["0x29a897 rel32"],
          "the camera's setter 0x2845C0 is called from the two sites alone, setup_profile 0x284B10 from the "
          "lobby's refresh 0x222A42, the squad panels' 0x223029 and its own re-call 0x2850C5 alone, 0x265DE0 and "
          "0x265F10 from the start_item_upgrade and exit_item_upgrade handlers alone (and none is a value anywhere)")
    names = struct.unpack("<120I", stock.read(0x9C56C8, 480))
    text_at = lambda at: bytes(original[stock.offset(at):original.index(b"\0", stock.offset(at))])
    # The delegates are registered in the order of the names, 20h apart:
    # drop_upgrade_item (8) is `mov eax,0x29a9d0` at 0x2818FA (check 31).
    check(text_at(names[5]) == b"start_item_upgrade" and text_at(names[6]) == b"exit_item_upgrade"
          and text_at(names[8]) == b"drop_upgrade_item"
          and rows(stock, 0x281843, 5) == ["mov eax,0x29a870"] and rows(stock, 0x281880, 5) == ["mov eax,0x29a890"]
          and rows(stock, 0x2818FA, 5) == ["mov eax,0x29a9d0"]
          and rows(stock, 0x29A870, 0x1E) == ["mov eax,[esp+0x4]", "mov edx,[eax+0x8]", "push esi",
                                              "mov esi,[ecx+0x694]", "push edi", "mov edi,[edx+0x8]",
                                              "call 0x265de0", "pop edi", "mov al,0x1", "pop esi", "ret 0x4"]
          and rows(stock, 0x29A890, 0x12) == ["push esi", "mov esi,[ecx+0x694]", "call 0x265f10", "mov al,0x1",
                                              "pop esi", "ret 0x4"],
          "the callbacks start_item_upgrade (5) and exit_item_upgrade (6) are the delegates 0x29A870 and 0x29A890, "
          "registered as drop_upgrade_item's (8); they call 0x265DE0 (esi = the lobby ui's character +694h, edi = "
          "the item) and 0x265F10 (esi = the character)")
    floats = struct.unpack("<f", stock.read(P.WEAPON_PREVIEW_LEAST, 4))[0]
    tick = [address for address, _, text in ndisasm(stock.read(0x283EF0, 0x5D7), 0x283EF0) if "0xa7afa4" in text]
    everywhere = [address for address, _, text in ndisasm(stock.read(0x283B00, 0x1000), 0x283B00)
                  if "0xa7afa4" in text]
    check(floats == 1.5 and tick == [0x284081, 0x28428C, 0x284316] and everywhere == tick,
          "the float at 0xA7AFA4 is 1.5; the lobby camera's tick (0x283EF0) and its other functions read it at "
          "the three sites alone")
    writes = [address for address, _, text in ndisasm(stock.read(0x263000, 0x23000), 0x263000)
              if re.search(r"mov (?:byte )?\[e[a-z]{2}\+0xd515\],", text)]
    init = rows(stock, 0x265B76, 6)
    check(writes == [0x265E0B, 0x265F1D] and rows(stock, 0x265E0B, 7) == ["mov byte [esi+0xd515],0x1"]
          and init == ["mov [esi+0x24],al", "mov [esi+0x25],al"] and rows(stock, 0x2847D0, 0xB)
          == ["lea esi,[ebp+0xd4f0]", "call 0x265b40"],
          "the upgrade mode (+D515h) is set to 1 by 0x265DE0 alone and to 0 by 0x265F10; the constructor "
          "(0x284730 -> 0x265B40 on +D4F0h) clears it (+25h)")
    code = open(P.weapon_preview_bin(), "rb").read()
    state = P.late_join_state_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_PREVIEW_STATE_OFFSET
    check(image.read(P.WEAPON_PREVIEW_VA, len(code)) == code and P.weapon_preview_state(code) == state,
          f"the weapon preview at {P.WEAPON_PREVIEW_VA:#x} is weapon_preview_026g.section.bin ({len(code)} bytes), "
          f"its state at {state:#x} (0x{P.WEAPON_PREVIEW_STATE_OFFSET:X} of .srpd, after the patches' mount's)")
    listing = ndisasm(code[:code.index(struct.pack("<f", 1.5))], P.WEAPON_PREVIEW_VA)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not P.WEAPON_PREVIEW_VA <= t < P.WEAPON_PREVIEW_VA + len(code)}
    starts = {address for address, _, _ in listing}
    stash_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_STASH_OFFSET
    table_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.WEAPON_TABLE_OFFSET
    find_va = P.weapon_camo_va(open(P.late_join_bin(), "rb").read()) + P.UPGRADE_PREVIEW_OFFSET \
        + P.UPGRADE_PREVIEW_FIND
    check(outside == {0x284B10, 0x284B16, table_va, find_va}
          and all(t in starts for t in targets - outside)
          and all(P.WEAPON_PREVIEW_VA + offset in starts for _, _, offset, _ in P.WEAPON_PREVIEW_HOOKS)
          and [address - P.WEAPON_PREVIEW_VA for address, _, text in listing
               if branch_target(text) == find_va]
          == list(P.WEAPON_PREVIEW_FIND_CALLS)
          and [address - P.WEAPON_PREVIEW_VA for address, _, text in listing if branch_target(text) == table_va]
          == list(P.WEAPON_PREVIEW_CAMERA_JUMPS)
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", text)],
          "the code calls only setup_profile and upgrade_preview_026g.asm's find (at its two finds; it calls "
          "weapon_stash_026g.asm's find_kit_or_show first), "
          "jumps only to setup_profile's body after the six bytes and to weapon_table_026g.asm's camera (at "
          "start_cam's and exit_cam's ends, where it jumped to the camera's setter), every branch inside it "
          "lands on an instruction, and no operand is an absolute address")
    stash = open(P.weapon_stash_bin(), "rb").read()
    check(image.read(stash_va, len(stash)) == stash and P.weapon_preview_state(stash, stash_va) == state,
          f"the weapon stash at {stash_va:#x} (.srpw+{P.WEAPON_STASH_OFFSET:#x}) is weapon_stash_026g.section.bin "
          f"({len(stash)} bytes), its state patch 32's (+0Ch..+17h of it, inside WEAPON_PREVIEW_STATE_SIZE)")
    sites = all(image.read(va, 5) == bytes([opcode]) + struct.pack("<i", stash_va + entry - va - 5)
                and image.read(stash_va + entry, 1) == b"\xE9"
                for (va, _, _, opcode), (entry, _) in ((h, P.WEAPON_STASH_ENTRIES[h[0]]) for h in P.WEAPON_PREVIEW_HOOKS
                                                       if h[0] in P.WEAPON_STASH_ENTRIES))
    check(sites and len(P.WEAPON_STASH_ENTRIES) == 3,
          "patch 32's start (0x265E73), exit (0x265F44) and setup_profile (0x284B10) sites go to the stash's "
          "entries (+0, +5, +0Ah), each a jump")
    listing = ndisasm(stash, stash_va)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    outside = {t for t in targets if not stash_va <= t < stash_va + len(stash)}
    starts = {address for address, _, _ in listing}
    absolute = {int(m, 16) for _, _, text in listing for m in re.findall(r"\[(?:dword )?(0x[0-9a-f]+)\]", text)}
    check(outside == {0x1F0090, 0x2CA910, 0x277AA0, 0x270390, 0x284D10, 0x2F73F0, 0x0E4F90, 0x420789, 0x284B10}
          | {P.WEAPON_PREVIEW_VA + preview for _, preview in P.WEAPON_STASH_ENTRIES.values()}
          and all(t in starts for t in targets - outside) and stash_va in starts
          and absolute == {0x1F1D9C4}
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[", text)],
          "the stash code calls only item_by_id, can_move_item_to_slot, the inventory's find, the item and "
          "profile copies, the profile ctor, the game's allocator and setup_profile, leaves only to patch 32's "
          "start_cam, exit_cam and setup_hook, every branch inside it lands on an instruction, and its only "
          "absolute operand is the game's allocator (0x1F1D9C4)")
    # The draw.
    va, old, offset, opcode = P.WEAPON_DRAW_HOOK
    check(stock.read(va, len(old)) == old and before.read(va, len(old)) == old
          and rows(stock, va, len(old)) == ["mov [ecx+0xf49c],al"]
          and rows(image, va, len(old)) == [f"call {P.WEAPON_DRAW_VA:#x}", "nop"]
          and rows(stock, 0x2849F8, 6) == ["mov al,[ebx+0xd515]"]
          and rows(stock, 0x284A20, 0x13) == ["mov ecx,[ebx+0xd2b8]", "test ecx,ecx", "jz 0x284a83",
                                              "mov edx,0x119190", "test edx,edx", "jz 0x284a83"]
          and rows(stock, 0x284A39, 0xB) == ["fld dword [ebx+0x14]", "push ecx", "lea esi,[esp+0x14]",
                                             "fstp dword [esp]"],
          f"{va:#x}: `mov [ecx+0xf49c],al` (the player's decal pose, al = the character's +D515h read at "
          f"0x2849F8, ecx its player +D2B8h) in the original and the build without the late join, "
          f"`call {P.WEAPON_DRAW_VA:#x}` and a nop here; what follows reads no flag before setting it")
    hits = branches_into(original, va + 1, va + len(old))
    check(not hits and not [rva for rva in relocations if rva + 4 > va and rva < va + len(old)],
          f"{va:#x}: no branch or address aims inside the site, no base relocation is under it"
          + "".join(", " + h for h in hits))
    pose = [(address, text) for address, _, text in ndisasm(stock.read(P.TEXT_VA, P.TEXT_VSIZE), P.TEXT_VA)
            if "+0xf49c]" in text]
    upgrade = [address for address, _, text in ndisasm(stock.read(0x263000, 0x23000), 0x263000)
               if "+0xd515]" in text]
    check([a for a, t in pose if t.startswith("mov")] == [0x1F1BFB, 0x284A33]
          and [a for a, t in pose if t.startswith("cmp")] == [0x1F30E2, 0x2476F8, 0x24778F]
          and len(pose) == 5 and upgrade == [0x265E0B, 0x265F1D, 0x2849F8],
          "the decal pose (+F49Ch) is written by the player's ctor (0x1F1BFB, 0) and 0x284A33 alone and read by "
          "player::insert (0x1F30E2) and player::mt_draw (0x2476F8 the bind pose, 0x24778F the held item) alone; "
          "the upgrade mode (+D515h) is read at 0x2849F8 alone")
    draw = open(P.weapon_draw_bin(), "rb").read()
    listing = ndisasm(draw, P.WEAPON_DRAW_VA)
    targets = {branch_target(text) for _, _, text in listing if branch_target(text) is not None}
    starts = {address for address, _, _ in listing}
    check(image.read(P.WEAPON_DRAW_VA, len(draw)) == draw and P.weapon_draw_state(draw) == state
          and all(P.WEAPON_DRAW_VA <= t < P.WEAPON_DRAW_VA + len(draw) and t in starts for t in targets)
          and [text for _, _, text in listing].count("ret") == 2
          and not [text for _, _, text in listing if re.search(r"call (?:dword )?\[|jmp (?:dword )?\[|\[(?:dword )?0x", text)],
          f"the draw at {P.WEAPON_DRAW_VA:#x} is weapon_draw_026g.section.bin ({len(draw)} bytes), reads the state "
          f"at {state:#x}, branches only inside itself, returns at its two ends and holds no absolute address")


def test_revision_override(original, default, server):
    """Check 30: a release's --revision=<n> against the default build of the
    same server: the revision byte and the PE checksum, nothing else."""
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    byte, checksum_at = P.file_offset(P.REVISION_VA) + 4, P.Headers(default).checksum_at
    for revision in (P.REVISION + 1, 255):
        data, _ = P.build(original, "section", upload, server, code, revision=revision)
        changed = [i for i in range(max(len(data), len(default)))
                   if i >= len(data) or i >= len(default) or data[i] != default[i]]
        check(len(data) == len(default) and data[byte] == revision
              and all(i == byte or checksum_at <= i < checksum_at + 4 for i in changed),
              f"--revision={revision}: the revision byte (file {byte:#x}) is {revision}; besides it only the PE "
              f"checksum differs from the build of REVISION {P.REVISION} ({len(changed)} bytes)")
        test_revision(original, Image(data), "section", revision, late_join=True)
    for revision, late_join in ((P.REVISION - 1, code), (256, code), (P.REVISION + 1, None)):
        try:
            P.build(original, "section", upload, server, late_join, revision=revision)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--revision={revision}{'' if late_join else ' without the late join'} is refused")


def test_required_patches(original, default, server):
    """Check 49: a release's --required-patches= against the default build
    of the same server: the mount's bitmap and the PE checksum, nothing else."""
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    mount = open(P.mount_patches_bin(), "rb").read()
    needed = len(mount) - 4 - P.MOUNT_REQUIRED_SIZE
    image = Image(default)
    bitmap_va = P.MOUNT_PATCHES_VA + needed
    check(image.read(bitmap_va - len(P.MOUNT_UNREADABLE), len(P.MOUNT_UNREADABLE)) == P.MOUNT_UNREADABLE
          and image.read(bitmap_va, P.MOUNT_REQUIRED_SIZE) == bytes(P.MOUNT_REQUIRED_SIZE)
          and P.required_patches_of(bytes(default)) == [] and lib_required_patches(default) in ("", None),
          f"the default build requires no patch: its bitmap at {bitmap_va:#x}, after the \"Cannot open\" text, is "
          f"{P.MOUNT_REQUIRED_SIZE} zeros")
    # The probe: a missing file (error 2 or 3) tests its n in the bitmap
    # and goes on to the "Cannot open" fatal's push when the bit is set.
    listing = ndisasm(mount[:needed - 200], P.MOUNT_PATCHES_VA)
    texts = [text for _, _, text in listing]
    bt = [i for i, text in enumerate(texts) if text.startswith("bt ")]
    unreadable = P.MOUNT_PATCHES_VA + mount.index(P.MOUNT_UNREADABLE)
    ok = len(bt) == 1
    if ok:
        i = bt[0]
        fatal_push = listing[i + 2][0]
        ok = (texts[i] == f"bt [edx+{bitmap_va:#x}],ecx" and texts[i - 2] == "mov edx,[esp]"
              and texts[i - 1] == "mov ecx,[esp+0x8]" and texts[i + 1].startswith("jnc ")
              and texts[i - 4] in ("cmp eax,byte +0x3", "cmp eax,0x3"))
        ok = ok and texts[i - 3] == f"jnz {fatal_push:#x}" and texts[i + 2] == "push eax" \
            and texts[i + 5] == f"lea eax,[edx+{unreadable:#x}]" and texts[i + 7] == "call 0x113580"
    check(ok, "the probe tests a missing patch's n (error 2 or 3) with `bt` on the bitmap plus the delta; a set bit "
          "goes on to the \"Cannot open resources.db.patch_%u (error %u)\" fatal as an unreadable file, a clear one "
          "to the next n" + ("" if ok else "\n          " + "\n          ".join(texts[bt[0] - 6:bt[0] + 9] if bt else texts)))
    checksum_at = P.Headers(default).checksum_at
    bitmap_at = image.offset(bitmap_va)
    for numbers in ((1, 12, 14, 17, 255), (12, 14, 17, *range(19, 28))):
        data, description = P.build(original, "section", upload, server, code, required_patches=numbers)
        changed = [i for i in range(max(len(data), len(default)))
                   if i >= len(data) or i >= len(default) or data[i] != default[i]]
        bitmap = Image(data).read(bitmap_va, P.MOUNT_REQUIRED_SIZE)
        names = ", ".join(map(str, numbers))
        check(len(data) == len(default) and bitmap == P.required_bitmap(numbers)
              and all(bitmap[n >> 3] >> (n & 7) & 1 == (n in numbers) for n in range(256))
              and P.required_patches_of(bytes(data)) == list(numbers)
              and lib_required_patches(data) in (" ".join(map(str, numbers)), None)
              and all(checksum_at <= i < checksum_at + 4 or bitmap_at <= i < bitmap_at + P.MOUNT_REQUIRED_SIZE
                      for i in changed)
              and description.endswith(f"without resources.db.patch_{', _'.join(map(str, numbers))}"
                                       + ("" if server is None else f", default login server {server}")),
              f"--required-patches={','.join(map(str, numbers))}: the bitmap has bit n of byte n/8 for {names}; "
              f"besides it only the PE checksum differs ({len(changed)} bytes); patch_026g.py and release/lib.sh "
              "read the list back")
    for numbers, late_join in (((0,), code), ((256,), code), ((12, 12), code), ((12,), None)):
        try:
            P.build(original, "section", upload, server, late_join, required_patches=numbers)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--required-patches={','.join(map(str, numbers))}"
              f"{'' if late_join else ' without the late join'} is refused")
    for text in ("14,12", "12,,14", "012", "", "12, 14"):
        try:
            P.parse_required_patches(text)
            refused = False
        except SystemExit:
            refused = True
        check(refused, f"--required-patches={text!r} is refused (increasing whole numbers, commas between)")


def lib_required_patches(data):
    """release/lib.sh's exe_required_patches of the exe `data`: what the
    release reads back from a built exe; None where there is no bash (it
    is read on the release's Linux machine)."""
    if shutil.which("bash") is None or os.name == "nt":
        return None
    path = os.path.join(SCRATCH, "required-patches.exe")
    with open(path, "wb") as out:
        out.write(data)
    lib = os.path.join(HERE, "..", "..", "release", "lib.sh")
    result = subprocess.run(["bash", "-c", f'. "$1" && exe_required_patches "$2"', "-", lib, path],
                            capture_output=True, text=True)
    return result.stdout.strip() if result.returncode == 0 else f"failed: {result.stderr.strip()}"


ABI_TSV = os.path.join(HERE, "..", "..", "tools", "re-kit", "v026g", "abi.tsv")


def abi_cleanup():
    """{VA: ret_cleanup} of every re-kit abi.tsv row with one (its number of
    bytes popped, or -1 for several). The models' call shapes come from here
    (the re-kit is the source, not hand-written notes); 0.26g never rebases,
    so a VA is an address."""
    out = {}
    with open(ABI_TSV, newline="") as f:
        for row in csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
            cleanup = row["ret_cleanup"]
            if cleanup not in ("", "None"):
                values = {int(c) for c in re.split(r"[ ,;/]+", cleanup.strip()) if c}
                out.setdefault(int(row["va"], 16), -1 if len(values) != 1 else values.pop())
    return out


def effective_cleanup(image, va):
    """The bytes the function at `va` pops (function_pops), or None when its
    rets differ, none is found or a branch could not be followed."""
    if va is None:
        return None
    found, lost = function_pops(image, va)
    return found.pop() if len(found) == 1 and not lost else None


_ORIGINALS = {}


def model_cleanup(original, image, model, abi):
    """The bytes the model pops, read from the original exe; when re-kit
    abi.tsv has the model's row, its ret_cleanup must say the same (the
    reviewed table and the bytes agree, or the check fails: a wrong
    hand-written note is how Research's slot 16 went wrong). (cleanup or None,
    where it came from)."""
    source = _ORIGINALS.setdefault(hashlib.sha256(original).hexdigest(), Image(original))
    read = effective_cleanup(source, model)
    if abi.get(model, -1) >= 0:
        if abi[model] != read:
            return None, f"abi.tsv says {abi[model]}, the original exe {read}"
        return read, "the original exe, as abi.tsv"
    return read, "the original exe; no abi.tsv row"


def callee_pushes(code):
    """The bytes the pushes before a replaced `call` pushed (`push r32`,
    `push imm8`, `push imm32`: four each); None for anything else."""
    pushed, at = 0, 0
    while at < len(code):
        if 0x50 <= code[at] <= 0x57:
            at += 1
        elif code[at] == 0x6A:
            at += 2
        elif code[at] == 0x68:
            at += 5
        else:
            return None
        pushed += 4
    return pushed if at == len(code) else None


# Every site test_call_shapes saw installed, over every variant built: an
# INSTALLS row no variant installs is stale (main checks at its end).
INSTALLED_SEEN = set()


def test_call_shapes(original, data, report=None):
    """Every function a patch makes the game call is declared (INSTALLS), and
    every one the game calls through a pointer our code hands it (a vtable
    slot, a delegate, a callback) pops off the stack exactly what the game's
    own function in its place does: the self-test reads our function's `ret`
    from the built bytes and the model's from re-kit abi.tsv or the original
    exe, and fails on a mismatch. This is the check that would have caught
    Research's rule slot 16 (`ret 4` where the game's rules `ret 8`),
    client_patch.md section 0.4. `report(ok, text)` defaults to the self-test's
    `check`; the negative test passes its own to capture without printing."""
    report = check if report is None else report
    check_, globals()["check"] = check, report      # the sub-checks below use `check`
    try:
        return _call_shapes_body(original, data)
    finally:
        globals()["check"] = check_


def _call_shapes_body(original, data):
    print("  call shapes (every installed function's model)")
    image = Image(data)
    installs = P.installs_of(data, P.WRITES)
    # 1. Every call/jmp/pointer into our code is declared, with a fitting kind.
    try:
        P.audit_installs(data, P.WRITES)
        audited = True
    except SystemExit as exit:
        audited = False
        print(f"        {exit}")
    check(audited and all(va in P.INSTALLS for va in installs),
          f"every one of the {len(installs)} functions the game calls of ours is declared in INSTALLS")
    INSTALLED_SEEN.update(installs)
    # 2. Each function the game calls of ours pops what its model does: a
    # slot's (the function the original's pointer named), an entry's (the
    # function at the site), a callee's (the replaced call's target, less the
    # bytes of the pushes replaced with it), an ABI's (its declared pops),
    # each run-time callback's. Ours is read from the built bytes, every
    # `ret` reachable from its entry (function_pops); the model's from the
    # original exe (abi.tsv agreeing where it has the row).
    abi = abi_cleanup()
    source = Image(original)
    for site, (how, target, old) in sorted(installs.items()):
        kind, what, pops, _ = P.INSTALLS[site]
        if kind in (P.JUMP, P.INLINE):
            continue
        if kind == P.SLOT:
            model = struct.unpack("<I", source.read(site, 4))[0]
            want, whence = model_cleanup(original, image, model, abi)
            whence = f"its model {model:#x}, {whence}"
        elif kind == P.ENTRY:
            want, whence = model_cleanup(original, image, site, abi)
            whence = f"the original's function at the site, {whence}"
        elif kind == P.CALLEE:
            model = (site + len(old) + struct.unpack_from("<i", old, len(old) - 4)[0]) & 0xFFFFFFFF
            pushed = callee_pushes(old[:-5])
            want, whence = model_cleanup(original, image, model, abi)
            whence = f"its model {model:#x}, {whence}" + (f", less the {pushed} bytes pushed at the site"
                                                          if pushed else "")
            want = None if want is None or pushed is None else want - pushed
        else:                                              # ABI
            want, whence = pops, "declared"
        found, lost = function_pops(image, target)
        ours = found.pop() if len(found) == 1 and not lost else None
        check(want is not None and ours == want,
              f"{site:#x} {kind} ({what}): ours ({target:#x}) pops {ours if ours is not None else sorted(found)}, "
              f"as it must: {want} ({whence})" + (f"; not followed: {lost[:2]}" if lost else ""))
    callbacks = []
    linked = P.arena()
    for what, name, before_bytes, model, _why, driven in P.CALLBACKS:
        code = P.linker.block_bytes(linked, name) if name in linked.blocks \
            else open(os.path.join(HERE, f"{name}_026g.section.bin"), "rb").read()
        at = code.find(before_bytes)
        got = -1 if at < 0 or code.find(before_bytes, at + 1) >= 0 \
            else struct.unpack_from("<I", code, at + len(before_bytes))[0]
        callbacks.append((got, what, model, driven))
    for our_va, what, model, driven in callbacks:
        want, whence = model_cleanup(original, image, model, abi)
        ours = effective_cleanup(image, our_va) if our_va not in (None, -1) else None
        check(want is not None and ours == want,
              f"{what}: `ret {ours}` pops what its model {model:#x} does (`ret {want}`, {whence})")
        check_driven(f"{what}", driven)
    # 3. Every vtable our code builds (a rule's, a cook's): each slot of ours
    # pops what the game's vtable pops in the same place (check 53, for
    # every patch).
    check_vtables(original, image, P.INSTALLED_VTABLES, P.arena_symbols(linked))


def made_up(folder, name, text):
    """A made-up block's NASM source in `folder`: its path."""
    path = os.path.join(folder, name)
    with open(path, "w") as out:
        out.write("bits 32\n" + text)
    return path


def check_vtables(original, image, vtables, symbols):
    """Each declared vtable's slots against its model's: a slot that is the
    game's own function is its model; a slot of ours must pop (its first
    `ret` before the next symbol of its code) what the model's slot pops; a
    vtable without a model must hold only the game's functions. The number of
    our own slots is the declaration's (a slot that changes owner is seen)."""
    source = Image(original)
    abi = abi_cleanup()
    read32 = lambda img, va: struct.unpack("<I", img.read(va, 4))[0]
    in_text = lambda va: P.TEXT_VA <= va < P.TEXT_VA + P.TEXT_VSIZE
    for what, json_name, symbol, model, slots, own, driven in vtables:
        table = symbols[json_name]
        ours_at = table[symbol] + 4                        # the RTTI locator, then the slots
        compared = []
        for slot in range(slots):
            ours = read32(image, ours_at + 4 * slot)
            theirs = read32(source, model + 4 * slot) if model is not None else None
            if in_text(ours) and (model is None or ours == theirs):
                continue                                   # the game's own function
            compared.append(slot)
            if model is None:
                check(False, f"{what} vtable slot {slot}: {ours:#x} is ours, but the vtable has no model")
                continue
            mine, lost = function_pops(image, ours)
            want, whence = model_cleanup(original, image, theirs, abi)
            check(want is not None and mine == {want} and not lost,
                  f"{what} vtable slot {slot}: ours ({ours:#x}) pops {sorted(mine)} bytes, the model's "
                  f"({theirs:#x}, vtable {model:#x}) {want} ({whence})"
                  + (f"; not followed: {lost[:2]}" if lost else ""))
            check_driven(f"{what} vtable slot {slot}", driven.get(slot))
        check(len(compared) == own,
              f"{what} vtable ({symbol}): {own} slots of ours, each compared with "
              + (f"vtable {model:#x}'s ({compared})" if model is not None else "none (the game's functions only)"))


def check_driven(what, driven):
    """A function of ours the game calls through a pointer (a vtable slot,
    a callback) has a case of its patch's Unicorn check that reaches it
    through the game's own caller (INSTALLED_VTABLES' and CALLBACKS' last
    field): the `ret` comparison cannot see the order of the arguments
    (Research's slot 7, #297's review). The case must be the label of a
    case of its script: the text of a string argument of a `check(`,
    `say(` or `print(` call there (case_labels), not a comment or a
    docstring; the script runs it (a patch's check is run with the patch)."""
    script, case = driven if driven else (None, None)
    path = os.path.join(HERE, script) if script else ""
    labels = case_labels(path) if script and os.path.exists(path) else []
    check(bool(case) and any(case in label for label in labels),
          f"{what}: reached through the game's own caller in {script}: \"{case}\"" if case
          else f"{what}: no case of its check reaches it through the game's own caller (declare one)")


def case_labels(path):
    """The labels of a check script's cases: every string argument of its
    `check(`, `say(` and `print(` calls (an f-string's literal parts with
    `{}` for each field; `a + b` of strings joined), parsed, so a comment or
    a docstring is no case."""
    def text(node):
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return node.value
        if isinstance(node, ast.JoinedStr):
            return "".join(text(v) if isinstance(v, ast.Constant) else "{}" for v in node.values)
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            left, right = text(node.left), text(node.right)
            return left + right if left is not None and right is not None else left or right
        if isinstance(node, ast.IfExp):
            return text(node.body)
        return None
    out = []
    for node in ast.walk(ast.parse(open(path).read())):
        if isinstance(node, ast.Call):
            name = getattr(node.func, "id", None) or getattr(node.func, "attr", None)
            if name in ("check", "say", "print"):
                out += [label for label in map(text, node.args) if label]
    return out


def test_call_shapes_negative(original):
    """The call-shape check rejects a wrong `ret` and an undeclared install:
    a fake build whose one installed function pops the wrong number of bytes,
    and one with a call into our code that INSTALLS does not name. It proves
    the check is live (Research's slot 16 was `ret 4` where the model is
    `ret 8`)."""
    print("  call shapes: the check rejects a wrong ret and an undeclared install")
    image = Image(original)
    # A made-up rule vtable in made-up `.srpx` blocks (after the arena's own,
    # linked from NASM sources): tdm_rule_core's twenty slots (vtable
    # 0xA15DA4), but slot 16, the deserialize, ours and `ret 4` where tdm's
    # pops 8: Research's first rn_deserialize. (Named `_slots`, not
    # `_vtable`: build() would refuse an undeclared vtable before the check.)
    read32 = lambda va: struct.unpack_from("<I", original, image.offset(va))[0]
    model = read32(0xA15DA4 + 4 * 16)
    want = popped(image, model, model + 0x400)
    code = open(P.default_bin("section"), "rb").read()
    late_join = open(P.late_join_bin(), "rb").read()
    slots = [f"{read32(0xA15DA4 + 4 * i):#x}" for i in range(20)]
    slots[16] = "fake_deserialize"
    for wrong, name in ((bytes.fromhex("c20400"), "pops 4"), (bytes.fromhex("c20800"), "pops 8")):
        with tempfile.TemporaryDirectory(prefix="srp-fake-") as folder:
            blocks = (P.ArenaBlock("fake.code", "code", 0x1000, (made_up(folder, "fake_code.asm", (
                          "section .text\nglobal fake_deserialize\nfake_deserialize:\n    db "
                          + ", ".join(f"{b:#x}" for b in wrong) + "\n")),)),
                      P.ArenaBlock("fake.table", "data", 0x1000, (made_up(folder, "fake_table.asm", (
                          "section .data\nglobal fake_rule_slots\nextern fake_deserialize\nfake_rule_slots:\n"
                          "    dd 0, " + ", ".join(slots) + "\n")),)))
            data, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
            symbols = P.arena_symbols(P.arena(P.ARENA + blocks))
        table_va = symbols["fake.table"]["fake_rule_slots"]
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append((ok, text))
        try:
            check_vtables(original, Image(data), (("a made-up rule", "fake.table", "fake_rule_slots",
                                                     0xA15DA4, 20, 1,
                                                     {16: (P.RESEARCH_CHECK,
                                                           "the game's deserialize loop (0x2D2B42)")}),),
                          symbols)
        finally:
            globals()["check"] = saved
        failed = [text for ok, text in captured if not ok]
        if name == "pops 4":
            check(want == 8 and any("slot 16" in text for text in failed),
                  f"a rule vtable slot 16 that pops 4 where tdm_rule_core's ({model:#x}) pops {want} is rejected "
                  "(Research's first rn_deserialize)")
        else:
            check(not failed, "... the same slot popping 8 passes")
    # A vtable symbol of the arena's code with no declaration stops the build.
    try:
        P.audit_vtables(vtables=(), symbols={"fake.table": {"fake_rule_vtable": table_va}})
        refused = False
    except SystemExit:
        refused = True
    check(refused, "a `*_vtable` of the arena's code with no INSTALLED_VTABLES row is refused")
    # A slot of ours with no case through the game's caller, or with a case
    # its script does not have, is refused.
    for driven, name in (({}, "no case"), ({16: (P.RESEARCH_CHECK, "no such case of the check")}, "a missing case")):
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append((ok, text))
        try:
            check_vtables(original, Image(data), (("a made-up rule", "fake.table", "fake_rule_slots",
                                                     0xA15DA4, 20, 1, driven),), symbols)
        finally:
            globals()["check"] = saved
        check(any(not ok and "caller" in text for ok, text in captured),
              f"a slot of ours with {name} through the game's own caller is refused")
    # A case named only in a comment or a docstring is no case.
    with tempfile.TemporaryDirectory(prefix="srp-driven-") as folder:
        fake = os.path.join(folder, "check_fake.py")
        open(fake, "w").write('def cases():\n    """driven: a docstring\'s case"""\n'
                              '    # driven: a comment\'s case\n    check(True, "driven: a real case")\n')
        captured = []
        saved = globals()["check"]
        globals()["check"] = lambda ok, text: captured.append(ok)
        try:
            for case in ("driven: a docstring's case", "driven: a comment's case", "driven: a real case"):
                check_driven("a made-up slot", (fake, case))
        finally:
            globals()["check"] = saved
        check(captured == [False, False, True],
              "a case named only in a comment or a docstring is refused; a `check(` label passes")
    # An undeclared install: a call into our code with no INSTALLS row.
    fake_writes = [(0x500000, bytes(5), b"\xE8" + struct.pack("<i", P.SECTION_VA - 0x500000 - 5))]
    try:
        P.audit_installs(data, fake_writes, installs={})
        refused = False
    except SystemExit:
        refused = True
    check(refused, "a call into our code with no INSTALLS row is rejected")
    # ... and one written anywhere in a write: a `push imm32` of our code's
    # address in the middle of rewritten bytes.
    fake_writes = [(0x500000, bytes(8), b"\x90\x90\x68" + struct.pack("<I", P.SECTION_VA + 0x10) + b"\x90")]
    found = P.installs_of(data, fake_writes)
    try:
        P.audit_installs(data, fake_writes, installs={})
        refused = False
    except SystemExit:
        refused = True
    check(refused and list(found) == [0x500003] and found[0x500003][0] == "pointer",
          "a `push imm32` of our code's address inside a write (at +3) is found and, undeclared, rejected")


def link_refusal(blocks, tools=None, lock=None):
    """The text the arena of `blocks` (ARENA's when None, compared with
    `lock` when given) is refused with; None when it links."""
    saved = P.linker.read_lock
    if lock is not None:
        P.linker.read_lock = lambda: lock
    try:
        P.arena(blocks, P.linker.toolchain(**tools) if tools else None)
    except (SystemExit, P.linker.LinkError) as stop:
        return str(stop)
    finally:
        P.linker.read_lock = saved
    return None


def test_arena(original):
    """`.srpx`, the arena of the later patches (client_patch.md section 0.3),
    linked from its blocks' sources (link_026g.py). Deterministic, in
    ARENA's order, aligned, none overlapping, every block inside the
    section, code gaps int3, state zeros, a symbol of one block used by
    another, the headers and the PE checksum right; ARENA's blocks in another
    order link and work (the hooks and the library's calls follow the
    symbols, the library's check passes there); with no block there is no
    `.srpx`. Refused: a symbol no source defines, a relocation the arena
    does not link, a section no block takes, a block past ARENA_LIMIT, a
    missing tool, NASM 3, a hook symbol the link lacks, and an arena that is
    not the lock's."""
    print("  the arena (.srpx)")
    code = open(P.default_bin("section"), "rb").read()
    late_join = open(P.late_join_bin(), "rb").read()
    without, _ = P.build(original, "section", code, None, late_join)
    none, _ = P.build(original, "section", code, None, late_join, arena_blocks=())
    check([s[0] for s in Image(none).headers.sections(none)] == [s[0] for s in Image(without).headers.sections(without)][:-1]
          and Image(without).headers.sections(without)[-1][0] == P.ARENA_NAME
          and bytes(P.add_arena(bytearray(b"MZ"), None)) == b"MZ",
          "with no block at all there is no `.srpx` (add_arena adds no section)")
    with tempfile.TemporaryDirectory(prefix="srp-arena-") as folder:
        # Blocks of each kind, out of alignment order, after ARENA's own: a
        # code block whose source has state too, a data block pointing at
        # it, a code block calling it.
        a = made_up(folder, "a_part.asm", "section .text\nglobal a_entry\na_entry:\n    times 16 nop\n    ret\n"
                    "section .bss\nglobal a_state\na_state:\n    resb 0x40\n")
        b = made_up(folder, "b_part.asm", "extern a_entry, a_state\nsection .data\nglobal b_data\nb_data:\n"
                    "    db \"hello\", 0\n    dd a_entry, a_state\nsection .text\nglobal b_entry\nb_entry:\n"
                    "    call a_entry\n    ret\n")
        blocks = (P.ArenaBlock("a.code", "code", 0x1000, (a,)), P.ArenaBlock("a.state", "state", 0x1000, (a,)),
                  P.ArenaBlock("b.data", "data", 0x10, (b,)), P.ArenaBlock("b.code", "code", 0x40, (b,)))
        one, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
        P.linker._links.clear()
        P.linker._objects.clear()
        two, _ = P.build(original, "section", code, None, late_join, arena_blocks=P.ARENA + blocks)
        linked = P.arena(P.ARENA + blocks)
    check(one == two, "the arena is byte for byte deterministic (linked twice from its sources, the same exe)")
    image = Image(one)
    sections = image.headers.sections(one)
    name, rva, vsize, raw, rsize, flags = sections[-1]
    align = lambda v, to: (v + to - 1) // to * to
    prev = sections[-2]
    check(name == P.ARENA_NAME and flags == P.ARENA_CHARACTERISTICS
          and P.IMAGE_BASE + rva == P.ARENA_VA and rva == align(prev[1] + prev[2], image.headers.section_alignment),
          f".srpx at {P.ARENA_VA:#x}: code | data | execute | read | write, on the page after .srpw")
    # Every block inside, aligned, in ARENA's order, none overlapping the next.
    order = [linked.blocks[b.name] for b in P.ARENA + blocks]
    inside = all(rva <= b.va - P.IMAGE_BASE and b.va + b.size - P.IMAGE_BASE <= rva + vsize for b in order)
    aligned = all(linked.blocks[b.name].va % b.align == 0 for b in P.ARENA + blocks)
    in_order = all(x.va + x.size <= y.va for x, y in zip(order, order[1:]))
    check(inside and aligned and in_order and order[0].va == P.ARENA_VA and vsize == linked.size
          and linked.size == order[-1].va + order[-1].size - P.ARENA_VA,
          f"the {len(order)} blocks are aligned, in ARENA's order, inside .srpx and do not overlap; VirtualSize "
          f"{vsize:#x} = the arena's size, to its last block's end ({', '.join(f'{b.name} @ {b.va:#x}' for b in order)})")
    # The bytes: each code block's bytes, then int3 to the next block; data
    # and state blocks and their gaps zeros; a reference across blocks
    # resolved by the link.
    body = bytes(one[raw:raw + vsize])
    at = lambda va: va - P.ARENA_VA
    gaps = all(set(body[at(x.va + x.size):at(y.va)]) <= ({0xCC} if x.kind == "code" else {0})
               for x, y in zip(order, order[1:]))
    states = all(not any(body[at(b.va):at(b.va + b.size)]) for b in order if b.kind == "state")
    s = linked.symbols
    b_data = body[at(s["b_data"]):at(s["b_data"]) + 14]
    call = body[at(s["b_entry"]):at(s["b_entry"]) + 5]
    check(gaps and states and body == linked.image
          and body[at(s["a_entry"]):at(s["a_entry"]) + 17] == b"\x90" * 16 + b"\xc3"
          and b_data == b"hello\0" + struct.pack("<II", s["a_entry"], s["a_state"])
          and call[0] == 0xE8 and s["b_entry"] + 5 + struct.unpack_from("<i", call, 1)[0] == s["a_entry"]
          and linked.blocks["a.state"].size == 0x40 and s["a_state"] == linked.blocks["a.state"].va,
          "each code block's bytes and int3 to the next block, data and state blocks zeros past their bytes; "
          "a.state is a_part.asm's .bss (0x40 bytes); b.data's pointers and b.code's call reach a.code's "
          "a_entry and a.state's a_state through the link")
    # The headers and the PE checksum, as test_pe checks them for the sections.
    get = lambda at: struct.unpack_from("<I", one, at)[0]
    checksum = P.pe_checksum(one, image.headers.checksum_at)
    security = image.directory(4)
    check(get(image.headers.size_of_image_at) == align(rva + vsize, image.headers.section_alignment)
          and raw == prev[3] + prev[4] and rsize == align(vsize, image.headers.file_alignment)
          and get(image.headers.checksum_at) == checksum
          and security[0] == raw + rsize and security[0] + security[1] == len(one),
          "SizeOfImage ends past .srpx, its file bytes follow .srpw's, the certificate table stays last, "
          "the PE checksum is right")
    # ARENA's blocks in another order (research.code and artefacts.code
    # swapped): every block elsewhere, and still each hook at its symbol,
    # Research's calls at the library's thunks and the library's check passing.
    names = [b.name for b in P.ARENA]
    swapped = list(P.ARENA)
    i, j = names.index("research.code"), names.index("artefacts.code")
    swapped[i], swapped[j] = swapped[j], swapped[i]
    data, _ = P.build(original, "section", code, None, late_join, arena_blocks=tuple(swapped))
    moved = P.arena(tuple(swapped))
    default = P.arena()
    built = Image(data)
    hooks = [(va, symbol, branch_target(ndisasm(built.read(va, 5), va)[0][2]))
             for va, _, symbol, _ in P.RESEARCH_HOOKS + P.ARTEFACTS_HOOKS]
    research_va = moved.blocks["research.code"].va
    listing = [t for _, _, t in ndisasm(P.linker.block_bytes(moved, "research.code")[P.RESEARCH_ASM_SIZE:],
                                        research_va + P.RESEARCH_ASM_SIZE)]
    calls = {s for s in moved.symbols if s.startswith("engine_") and any(
        t == f"call {moved.symbols[s]:#x}" or t.endswith(f",{moved.symbols[s]:#x}") for t in listing)}
    check(moved.blocks["artefacts.code"].va == default.blocks["research.code"].va != default.blocks["artefacts.code"].va
          and moved.blocks["research.code"].va != default.blocks["research.code"].va
          and all(target == moved.symbols[symbol] != default.symbols[symbol] for _, symbol, target in hooks)
          and len(calls) >= 11 and P.artefacts_backs(moved) == list(P.ARTEFACTS_BACKS)
          and built.read(moved.blocks["research.code"].va, 0x100) == P.linker.block_bytes(moved, "research.code")[:0x100],
          f"ARENA with research.code and artefacts.code swapped links (artefacts.code at "
          f"{moved.blocks['artefacts.code'].va:#x}, research.code at {research_va:#x}): the {len(hooks)} hooks "
          f"reach their symbols there, Research's C calls {len(calls)} library functions at their new thunks, "
          "the artefacts' entries go on where they came from")
    try:
        import check_engine_026g as engine_check
        bad = [text for ok, text in engine_check.run_checks(original, (
            moved.blocks["engine.code"].va, P.linker.block_bytes(moved, "engine.code"),
            {k: v for k, v in moved.symbols.items() if k.startswith("engine_")}))
            if not ok]
    except ImportError as error:
        bad = [f"check_engine_026g.py needs Python unicorn ({error})"]
    check(not bad, "... and check_engine_026g.py passes on the library there" + (": " + "; ".join(bad) if bad else ""))
    # What the link refuses, loudly.
    with tempfile.TemporaryDirectory(prefix="srp-arena-") as folder:
        cases = [
            ("a symbol no source defines", "nowhere_026g", (P.ArenaBlock("lost.code", "code", 16, (made_up(
                folder, "lost.asm", "extern nowhere_026g\nsection .text\n    call nowhere_026g\n"),)),)),
            ("a 16-bit address (R_386_16)", "R_386_16", (P.ArenaBlock("narrow.code", "code", 16, (made_up(
                folder, "narrow.asm", "section .text\nnarrow:\n    dw narrow\n"),)),)),
            ("a block past the arena's limit (ARENA_LIMIT)", "past its limit", (P.ArenaBlock(
                "huge.state", "state", 16, (made_up(folder, "huge.asm", f"section .bss\n    resb {P.ARENA_LIMIT:#x}\n"),)),)),
            ("a symbol defined in two sources", "multiple definition", tuple(
                P.ArenaBlock(f"twice{n}.code", "code", 16, (made_up(folder, f"twice{n}.asm",
                             "section .text\nglobal research_hud\nresearch_hud:\n    ret\n"),)) for n in (1, 2))),
        ]
        c_data = os.path.join(folder, "counter.c")
        with open(c_data, "w") as out:
            out.write("int counter = 1;\nint count(void) { return ++counter; }\n")
        cases.append(("a C source's initialized data (.data) in a code block only", "no data block",
                      (P.ArenaBlock("counter.code", "code", 16, (c_data,)),)))
        for what, word, extra in cases:
            text = link_refusal(P.ARENA + extra)
            said = next((line.strip() for line in (text or "linked").splitlines() if word in line), text or "linked")
            check(text is not None and word in text, f"the link refuses {what}: {said[:160]}")
        text = link_refusal(None, {"cc": os.path.join(folder, "no-such-cc")})
        check(text is not None and "no-such-cc" in text, f"a missing compiler is refused: {(text or 'linked')[:160]}")
        if os.name == "posix":
            fake = os.path.join(folder, "nasm")
            with open(fake, "w") as out:
                out.write("#!/bin/sh\necho 'NASM version 3.01 compiled on Oct 11 2025'\n")
            os.chmod(fake, 0o755)
            text = link_refusal(None, {"nasm": fake})
            check(text is not None and "NASM 2.16" in text, f"NASM 3 is refused: {(text or 'linked')[:160]}")
    try:
        P.arena_target(default, "nowhere_026g")
        text = None
    except SystemExit as stop:
        text = str(stop)
    check(text is not None and "nowhere_026g" in text, "a hook whose symbol the link lacks stops the build")
    lock = P.linker.read_lock()
    text = link_refusal(None, lock=dict(lock, sha256="0" * 64))
    check(lock is not None and text is not None and P.linker.LOCK.name in text,
          f"an arena whose bytes are not the lock's stops the build: {(text or 'built')[:160]}")


def test_apply(original):
    """The build's one writer (patch_026g.py apply(), rust_patches.md "The
    build"): every write of the default build into the original's bytes, and
    every section it adds, is a named instance's, no two overlap, and
    replaying the recorded writes onto the original gives the build's bytes
    exactly (outside the PE header's counters and the section table)."""
    late_join = open(P.late_join_bin(), "rb").read()
    code = open(P.default_bin("section"), "rb").read()
    data, _ = P.build(original, "section", code, None, late_join)
    applied = list(P.APPLIED)
    owners = sorted({a.owner for a in applied})
    check(applied and all(a.owner for a in applied),
          f"every write of the default build is a named instance's ({len(applied)} writes, relocations and "
          f"sections of {len(owners)} instances; patch_026g.py --installs lists them)")
    spans = sorted((a.at, a.at + len(a.new)) for a in applied)
    check(all(hi <= lo2 for (_, hi), (lo2, _) in zip(spans, spans[1:])),
          "no two of them cover the same bytes (apply() refuses an overlap at build time)")
    headers = P.Headers(original)
    end = max(raw + rsize for _, _, _, raw, rsize, _ in headers.sections(original))
    replay = bytearray(original)
    for a in applied:
        if a.kind in ("write", "relocation"):
            replay[a.at:a.at + len(a.new)] = a.new
    bookkeeping = set()
    for at, size in ((headers.count_at, 2), (headers.size_of_code_at, 4), (headers.optional + 12, 4),
                     (headers.size_of_image_at, 4), (headers.checksum_at, 4), (headers.security_at, 8)):
        bookkeeping.update(range(at, at + size))
    for a in applied:
        if a.kind == "section header":
            bookkeeping.update(range(a.at, a.at + 40))
    differ = [o for o in range(end) if o not in bookkeeping and replay[o] != data[o]]
    check(not differ, f"replaying the recorded writes onto the original gives the build's first {end:#x} bytes "
          "exactly (every change there went through apply())"
          + (f": {len(differ)} bytes differ, the first at file {differ[0]:#x}" if differ else ""))
    # an overlap and a write no instance holds stop the build
    saved = (list(P.APPLIED), P._BUILDING[0])
    try:
        P.APPLIED.clear()
        P._BUILDING[0] = True
        scratch = bytearray(original)
        va, old, new = P.TAB_SELF
        P.patch(scratch, va, old, new)
        try:
            P.apply(scratch, P.file_offset(va), va, new, old, owner="a second patch")
            overlap = False
        except SystemExit:
            overlap = True
        try:
            P.instance_of(0x401000)
            unowned = False
        except SystemExit:
            unowned = True
    finally:
        P.APPLIED[:] = saved[0]
        P._BUILDING[0] = saved[1]
    check(overlap and unowned, "apply() refuses a second write over TAB_SELF's bytes, and a write in no named "
          "instance")


def test_engine(original):
    """The engine-function library (engine_026g.py; client_patch.md section
    0.5): check_engine_026g.py's cases, each thunk against the re-kit's rows
    and the original exe, and its negative cases, wrong rows the check must
    reject. Needs Python `unicorn` (as the check_*.py scripts): without it the
    check fails, as a wrong row must never pass unseen."""
    print("  the engine-function library: every thunk against the ABI rows and the exe")
    try:
        import check_engine_026g as engine_check
    except ImportError as error:
        check(False, f"check_engine_026g.py needs Python unicorn ({error})")
        return
    results = engine_check.run_checks(original)
    bad = [text for ok, text in results if not ok]
    check(not bad, f"check_engine_026g.py: {len(results)} cases on the {len(engine_check.engine.functions())} "
          "functions of engine_026g.tsv" + (": " + "; ".join(bad) if bad else ""))
    for what, failed in engine_check.negative(original):
        check(failed, f"the library's check rejects {what}")


def test_hook_kinds(original):
    """The generated hook-entry kinds (hooks_026g.py; rust_patches.md): each
    kind's entry template, assembled and run beside the original exe, is
    transparent (check_hooks_026g.py). Needs Python `unicorn` and NASM/ld."""
    print("  the hook-entry kinds: each template transparent against the exe")
    try:
        import check_hooks_026g as hook_check
    except ImportError as error:
        check(False, f"check_hooks_026g.py needs Python unicorn ({error})")
        return
    results = hook_check.run_checks(original)
    for ok, text in results:
        check(ok, text)
    check(bool(results), f"check_hooks_026g.py: {len(results)} entry-kind checks")
    import io
    import unittest
    output = io.StringIO()
    suite = unittest.defaultTestLoader.loadTestsFromNames(["test_hooks_026g", "test_runtime_hooks_026g", "port.test_equiv_026g"])
    result = unittest.TextTestRunner(stream=output).run(suite)
    check(result.wasSuccessful(), f"the hook generator and equivalence build paths: {result.testsRun} tests"
          + ("" if result.wasSuccessful() else "\n" + output.getvalue()))


def test_late_join(original, without, server, source):
    """The default build with the late join against `without`, the build
    of the same server without it (checked by everything above)."""
    print(f"  the late join on top (patch revision {P.REVISION})")
    code = open(P.late_join_bin(), "rb").read()
    upload = open(P.default_bin("section"), "rb").read()
    data, description = P.build(original, "section", upload, server, code)
    print("  " + description)
    test_call_shapes(original, data)       # while P.WRITES still holds this build's
    image, before = Image(data), Image(without)
    headers = image.headers
    sections, old_sections = image.sections, before.sections
    name, rva, vsize, raw, rsize, flags = sections[-5]
    align = lambda value, to: (value + to - 1) // to * to
    weapon_preview = open(P.weapon_preview_bin(), "rb").read()
    weapon_draw = open(P.weapon_draw_bin(), "rb").read()
    first_page = bytes(without[old_sections[-1][3]:old_sections[-1][3] + old_sections[-1][2]])
    check(len(sections) == 12 and sections[:7] == old_sections[:7], "12 sections; the original's seven are untouched")
    check(name == P.SECTION_NAME and rva == P.SECTION_RVA and flags == P.SECTION_CHARACTERISTICS
          and vsize == P.WEAPON_PREVIEW_OFFSET + len(weapon_preview)
          and rsize == align(vsize, headers.file_alignment) and raw == old_sections[-1][3],
          f".srp: virtual {vsize:#x} = one page, the {len(code)} bytes of late-join code and the weapon preview's "
          f"{len(weapon_preview)} at +{P.WEAPON_PREVIEW_OFFSET:#x}, file {raw:#x}+{rsize:#x}")
    body = bytes(data[raw:raw + rsize])
    pickup = open(P.pickup_bin(), "rb").read()
    seek = open(P.seek_clear_bin(), "rb").read()
    stale_dll = open(P.stale_dll_bin(), "rb").read()
    mount_patches = open(P.mount_patches_bin(), "rb").read()
    forward_scroll = open(P.forward_scroll_bin(), "rb").read()
    kit_change = open(P.kit_change_bin(), "rb").read()
    warmup_look = open(P.warmup_look_bin(), "rb").read()
    warmup_limits = open(P.warmup_limits_bin(), "rb").read()
    warmup_keys = open(P.warmup_keys_bin(), "rb").read()
    # The first page's pieces, in order, none overlapping the next.
    link = open(P.replay_link_bin(), "rb").read()
    shared = open(P.shared_upgrades_bin(), "rb").read()
    spot = open(P.spot_bin(), "rb").read()
    pieces = [(0, len(first_page), "the build without the late join's first page"),
              (P.REPLAY_LINK_OFFSET, len(link), "the replay link code"),
              (P.MOUNT_PATCHES_OFFSET, len(mount_patches), "the patches' mount"),
              (P.SHARED_UPGRADES_OFFSET, len(shared), "the shared upgrades' code"),
              (P.SPOT_OFFSET, len(spot), "the spot code"),
              (P.PICKUP_OFFSET, len(pickup), "the pickup code"),
              (P.WARMUP_KEYS_OFFSET, len(warmup_keys), "the warmup keys"),
              (P.SEEK_CLEAR_OFFSET, len(seek), "the seek code"),
              (P.WARMUP_LIMITS_OFFSET, len(warmup_limits), "the warmup camera's limits"),
              (P.FORWARD_SCROLL_OFFSET, len(forward_scroll), "the forward effect's scrolling"),
              (P.WEAPON_DRAW_OFFSET, len(weapon_draw), "the weapon preview's draw"),
              (P.WARMUP_LOOK_OFFSET, len(warmup_look), "the warmup look"),
              (P.STALE_DLL_OFFSET, len(stale_dll), "the shell_32.dll check"),
              (P.KIT_CHANGE_OFFSET, len(kit_change), "the kit change"),
              (P.LATE_JOIN_OFFSET, len(code), "the late-join code"),
              (P.WEAPON_PREVIEW_OFFSET, len(weapon_preview), "the weapon preview"),
              (P.late_join_state_va(code) - P.SECTION_VA, 0, "the state page")]
    overlaps = [f"{a[2]} ends at {a[0] + a[1]:#x}, past {b[2]} at {b[0]:#x}"
                for a, b in zip(pieces, pieces[1:]) if a[0] + a[1] > b[0]]
    check(not overlaps, "the first page's code does not overlap: " + ", ".join(
        f"{name} [{start:#x}, {start + size:#x})" for start, size, name in pieces) + "".join("; " + o for o in overlaps))
    check(body[:len(first_page)] == first_page
          and set(body[len(first_page):P.REPLAY_LINK_OFFSET]) <= {0xCC}
          and body[P.REPLAY_LINK_OFFSET:P.REPLAY_LINK_OFFSET + len(link)] == link
          and set(body[P.REPLAY_LINK_OFFSET + len(link):P.MOUNT_PATCHES_OFFSET]) <= {0xCC}
          and body[P.MOUNT_PATCHES_OFFSET:P.MOUNT_PATCHES_OFFSET + len(mount_patches)] == mount_patches
          and set(body[P.MOUNT_PATCHES_OFFSET + len(mount_patches):P.SHARED_UPGRADES_OFFSET]) <= {0xCC}
          and body[P.SHARED_UPGRADES_OFFSET:P.SHARED_UPGRADES_OFFSET + len(shared)] == shared
          and set(body[P.SHARED_UPGRADES_OFFSET + len(shared):P.SPOT_OFFSET]) <= {0xCC}
          and body[P.SPOT_OFFSET:P.SPOT_OFFSET + len(spot)] == spot
          and set(body[P.SPOT_OFFSET + len(spot):P.PICKUP_OFFSET]) <= {0xCC}
          and body[P.PICKUP_OFFSET:P.PICKUP_OFFSET + len(pickup)] == pickup
          and set(body[P.PICKUP_OFFSET + len(pickup):P.WARMUP_KEYS_OFFSET]) <= {0xCC}
          and body[P.WARMUP_KEYS_OFFSET:P.WARMUP_KEYS_OFFSET + len(warmup_keys)] == warmup_keys
          and set(body[P.WARMUP_KEYS_OFFSET + len(warmup_keys):P.SEEK_CLEAR_OFFSET]) <= {0xCC}
          and body[P.SEEK_CLEAR_OFFSET:P.SEEK_CLEAR_OFFSET + len(seek)] == seek
          and set(body[P.SEEK_CLEAR_OFFSET + len(seek):P.WARMUP_LIMITS_OFFSET]) <= {0xCC}
          and body[P.WARMUP_LIMITS_OFFSET:P.WARMUP_LIMITS_OFFSET + len(warmup_limits)] == warmup_limits
          and set(body[P.WARMUP_LIMITS_OFFSET + len(warmup_limits):P.FORWARD_SCROLL_OFFSET]) <= {0xCC}
          and body[P.FORWARD_SCROLL_OFFSET:P.FORWARD_SCROLL_OFFSET + len(forward_scroll)] == forward_scroll
          and set(body[P.FORWARD_SCROLL_OFFSET + len(forward_scroll):P.WEAPON_DRAW_OFFSET]) <= {0xCC}
          and body[P.WEAPON_DRAW_OFFSET:P.WEAPON_DRAW_OFFSET + len(weapon_draw)] == weapon_draw
          and set(body[P.WEAPON_DRAW_OFFSET + len(weapon_draw):P.WARMUP_LOOK_OFFSET]) <= {0xCC}
          and body[P.WARMUP_LOOK_OFFSET:P.WARMUP_LOOK_OFFSET + len(warmup_look)] == warmup_look
          and set(body[P.WARMUP_LOOK_OFFSET + len(warmup_look):P.STALE_DLL_OFFSET]) <= {0xCC}
          and body[P.STALE_DLL_OFFSET:P.STALE_DLL_OFFSET + len(stale_dll)] == stale_dll
          and set(body[P.STALE_DLL_OFFSET + len(stale_dll):P.KIT_CHANGE_OFFSET]) <= {0xCC}
          and body[P.KIT_CHANGE_OFFSET:P.KIT_CHANGE_OFFSET + len(kit_change)] == kit_change
          and set(body[P.KIT_CHANGE_OFFSET + len(kit_change):P.LATE_JOIN_OFFSET]) <= {0xCC}
          and body[P.LATE_JOIN_OFFSET:P.LATE_JOIN_OFFSET + len(code)] == code
          and set(body[P.LATE_JOIN_OFFSET + len(code):P.WEAPON_PREVIEW_OFFSET]) <= {0xCC}
          and body[P.WEAPON_PREVIEW_OFFSET:P.WEAPON_PREVIEW_OFFSET + len(weapon_preview)] == weapon_preview
          and set(body[P.WEAPON_PREVIEW_OFFSET + len(weapon_preview):]) <= {0xCC},
          f"its first page is the build's without the late join ({len(first_page)} bytes, then int3) with the "
          f"replay link code at +{P.REPLAY_LINK_OFFSET:#x}, the patches' mount at +{P.MOUNT_PATCHES_OFFSET:#x}, "
          f"the shared upgrades' code at +{P.SHARED_UPGRADES_OFFSET:#x}, the spot code at +{P.SPOT_OFFSET:#x}, "
          f"the pickup code at +{P.PICKUP_OFFSET:#x}, "
          f"the warmup keys at +{P.WARMUP_KEYS_OFFSET:#x}, the seek "
          f"code at +{P.SEEK_CLEAR_OFFSET:#x}, the warmup camera's limits at +{P.WARMUP_LIMITS_OFFSET:#x}, the forward effect's scrolling at +{P.FORWARD_SCROLL_OFFSET:#x}, "
          f"the weapon preview's draw at +{P.WEAPON_DRAW_OFFSET:#x}, the warmup look at +{P.WARMUP_LOOK_OFFSET:#x}, "
          f"the shell_32.dll check at +{P.STALE_DLL_OFFSET:#x} "
          f"and the kit change at +{P.KIT_CHANGE_OFFSET:#x}, the late-join code follows, then int3 padding, "
          f"the weapon preview at +{P.WEAPON_PREVIEW_OFFSET:#x} and int3 padding")
    name, state_rva, state_size, state_raw, state_rsize, flags = sections[-4]
    check(name == P.LATE_JOIN_STATE_NAME and flags == P.LATE_JOIN_STATE_CHARACTERISTICS
          and state_size == P.LATE_JOIN_STATE_SIZE and (state_raw, state_rsize) == (0, 0)
          and state_rva == align(rva + vsize, headers.section_alignment)
          and P.IMAGE_BASE + state_rva == P.late_join_state_va(code),
          f".srpd at VA {P.IMAGE_BASE + state_rva:#x}: one page, uninitialized | read | write, no bytes in the "
          "file, on the page after the code (where the code looks for it)")
    death = open(P.death_camera_bin(), "rb").read()
    name_c, rva_c, vsize_c, raw_c, rsize_c, flags_c = sections[-3]
    check(name_c == P.DEATH_CAMERA_NAME and flags_c == P.DEATH_CAMERA_CHARACTERISTICS
          and rva_c == state_rva + state_size and P.IMAGE_BASE + rva_c == P.death_camera_va(code)
          and vsize_c == len(death) and raw_c == raw + rsize and rsize_c == align(vsize_c, headers.file_alignment)
          and bytes(data[raw_c:raw_c + vsize_c]) == death and set(data[raw_c + vsize_c:raw_c + rsize_c]) <= {0xCC},
          f".srpc at VA {P.IMAGE_BASE + rva_c:#x}: death_camera_026g.section.bin ({len(death)} bytes) and int3 "
          "padding, code | execute | read, on the page after .srpd, its file bytes right after .srp's")
    camo = open(P.weapon_camo_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.WEAPON_STASH_OFFSET - len(camo)) + open(P.weapon_stash_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.WEAPON_TABLE_OFFSET - len(camo)) + open(P.weapon_table_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.WEAPON_DECALS_OFFSET - len(camo)) + open(P.weapon_decals_bin(), "rb").read()
    camo = camo + b"\xCC" * (P.UPGRADE_PREVIEW_OFFSET - len(camo)) \
        + P.with_kit_apply(open(P.upgrade_preview_bin(), "rb").read(), P.arena())
    name_w, rva_w, vsize_w, raw_w, rsize_w, flags_w = sections[-2]
    check(name_w == P.WEAPON_CAMO_NAME and flags_w == P.WEAPON_CAMO_CHARACTERISTICS
          and rva_w == align(rva_c + vsize_c, headers.section_alignment) and P.IMAGE_BASE + rva_w == P.weapon_camo_va(code)
          and vsize_w == len(camo) and raw_w == raw_c + rsize_c and rsize_w == align(vsize_w, headers.file_alignment)
          and bytes(data[raw_w:raw_w + vsize_w]) == camo and set(data[raw_w + vsize_w:raw_w + rsize_w]) <= {0xCC},
          f".srpw at VA {P.IMAGE_BASE + rva_w:#x}: weapon_camo_026g.section.bin, int3 padding, "
          f"weapon_stash_026g.section.bin at +{P.WEAPON_STASH_OFFSET:#x}, int3 padding and "
          f"weapon_table_026g.section.bin at +{P.WEAPON_TABLE_OFFSET:#x}, int3 padding and "
          f"weapon_decals_026g.section.bin at +{P.WEAPON_DECALS_OFFSET:#x}, int3 padding and "
          f"upgrade_preview_026g.section.bin at +{P.UPGRADE_PREVIEW_OFFSET:#x} ({len(camo)} bytes), "
          "code | execute | read, on the page after .srpc, its file bytes right after .srpc's")
    linked = P.arena()
    name_r, rva_r, vsize_r, raw_r, rsize_r, flags_r = sections[-1]
    check(name_r == P.ARENA_NAME and flags_r == P.ARENA_CHARACTERISTICS
          and rva_r == align(rva_w + vsize_w, headers.section_alignment) and P.IMAGE_BASE + rva_r == P.ARENA_VA
          and linked.va == P.ARENA_VA and list(linked.blocks) == [b.name for b in P.ARENA]
          and vsize_r == linked.size and raw_r == raw_w + rsize_w
          and rsize_r == align(vsize_r, headers.file_alignment)
          and bytes(data[raw_r:raw_r + vsize_r]) == linked.image,
          f".srpx, the arena, at VA {P.IMAGE_BASE + rva_r:#x}: the linked arena ({linked.size:#x} bytes), its "
          "blocks " + ", ".join(f"{b.name} ({b.kind}) at {b.va:#x}" for b in linked.blocks.values())
          + "; code | data | execute | read | write, on the page after .srpw, its file bytes right after .srpw's")
    get = lambda blob, at: struct.unpack_from("<I", blob, at)[0]
    check(get(data, headers.size_of_image_at) == align(rva_r + vsize_r, headers.section_alignment)
          and get(data, headers.optional + 12) == get(without, headers.optional + 12) + state_size
          and get(data, headers.size_of_code_at) == get(original, headers.size_of_code_at) + rsize + rsize_c + rsize_w
          + rsize_r and headers.table + 40 * len(sections) <= headers.size_of_headers,
          "SizeOfImage ends at .srpx, SizeOfUninitializedData grew by .srpd's page, SizeOfCode by .srp's, "
          ".srpc's, .srpw's and .srpx's file sizes; the section table fits in the headers (its last entry)")
    security_offset, security_size = image.directory(4)
    old_offset, old_size = struct.unpack_from("<II", original, headers.security_at)
    check(security_offset == raw_r + rsize_r and security_offset + security_size == len(data) and security_offset % 8 == 0
          and bytes(data[security_offset:]) == bytes(original[old_offset:old_offset + old_size]),
          f"certificate table at {security_offset:#x}: right after .srpx's bytes, 8-byte aligned, last in the file, "
          "identical to the original's")
    same = all(image.directory(index) == before.directory(index) for index in range(16) if index != 4)
    check(same, "the other 15 data directories are those of the build without it")

    # Every byte before .srp's file data, against the build without.
    ranges = [(P.file_offset(va), size) for va, size in late_join_sites()]
    ranges.append((P.data_file_offset(P.CLOCK_FACTOR_VA), 4))
    ranges.append((image.offset(P.REPLAY_REATTACH_SLOT[0]), 4))
    ranges.append((image.offset(P.MATCH_KEYS_POLL_SLOT[0]), 4))
    ranges.append((image.offset(P.ARTEFACT_KEY_DEFAULT[0]), len(P.ARTEFACT_KEY_DEFAULT[2])))   # the artefact action's default key (ARTEFACT_KEY_DEFAULT)
    ranges += [(P.file_offset(P.REVISION_VA) + 4, 1), (P.file_offset(P.LATE_JOIN_READY_VA) + 1, 4),
               (P.file_offset(P.DB_SIGNATURE_VA), len(P.DB_SIGNATURE[1])), (P.file_offset(P.STALE_DLL_VA), len(P.STALE_DLL[1])),
               (P.file_offset(P.MOUNT_QUERY_VA), len(P.MOUNT_QUERY[1])), (P.file_offset(P.MOUNTED_VA), len(P.MOUNTED[1])),
               *((P.file_offset(va), len(old)) for va, old, _, _ in P.FORWARD_SCROLL_HOOKS),
               (headers.count_at, 2), (headers.size_of_code_at, 4), (headers.optional + 12, 4),
               (headers.size_of_image_at, 4), (headers.checksum_at, 4), (headers.security_at, 4),
               (headers.table + 40 * 7 + 8, 4), (headers.table + 40 * 7 + 16, 4), (headers.table + 40 * 8, 40),
               (headers.table + 40 * 9, 40), (headers.table + 40 * 10, 40), (headers.table + 40 * 11, 40)]
    # The entries of the relocation table that are disabled (scene::remove_model's
    # branch, uro_fov's least, the pickup's distance, the lobby camera's least,
    # the two constants of patch 40).
    disabled = {P.REMOVE_MODEL_VA + 2, P.FOV_MINIMUM_RELOC, P.PICKUP_RELOC, *P.WEAPON_PREVIEW_RELOCS,
                *P.FOV_LOD_RELOCS}
    ranges += [(at, 2) for at, (kind, va) in before.relocations().items()
               if kind == 3 and va in disabled]
    allowed = bytearray(raw)
    for at, size in ranges:
        allowed[at:at + size] = b"\1" * size
    a, b = bytes(without[:raw]), bytes(data[:raw])
    changed = [i for i in range(raw) if a[i] != b[i]] if a != b else []
    stray = [i for i in changed if not allowed[i]]
    check(not stray, f"{len(changed)} bytes before .srp differ from the build without the late join, all inside the "
          f"{len(ranges)} intended ranges" + "".join(f"\n          file {i:#x}" for i in stray[:8]))

    rows_at = lambda va, size: ndisasm(image.read(va, size), va)
    for va, old, new in P.LATE_JOIN_PATCHES:
        rows = rows_at(va, len(old))
        check(image.read(va, len(old)) == new and Image(original).read(va, len(old)) == old
              and rows[0][2] == LATE_JOIN_TEXT[va] and all(text == "nop" for _, _, text in rows[1:])
              and sum(len(code_hex) // 2 for _, code_hex, _ in rows) == len(old),
              f"{va:#x}: `{rows[0][2]}` + {len(rows) - 1} nop")
    for va, old, offset, opcode in P.LATE_JOIN_HOOKS + P.FOV_LOD_HOOKS + \
            ((P.LATE_JOIN_READY_VA, P.HOOKS[0][1], P.LATE_JOIN_READY_OFFSET, 0xE9),):
        rows = rows_at(va, len(old))
        word = "call" if opcode == 0xE8 else "jmp"
        check(Image(original).read(va, len(old)) == old and rows[0][2].startswith(word)
              and branch_target(rows[0][2]) == P.LATE_JOIN_VA + offset
              and all(text == "nop" for _, _, text in rows[1:]) and len(rows) == len(old) - 4,
              f"hook {va:#x}: {rows[0][2]} + {len(rows) - 1} nop (entry {offset})")
    # The game that goes on without the focus: the same instructions up to
    # the test, then two nops where the original (and the build without the
    # late join) jumps to the branch that does not call game::tick.
    va, old, new = P.RUN_UNFOCUSED
    rows, original_rows = rows_at(va, len(old)), ndisasm(Image(original).read(va, len(old)), va)
    texts = [text for _, _, text in rows]
    check(image.read(va, len(old)) == new and Image(original).read(va, len(old)) == old
          and before.read(va, len(old)) == old
          and texts[:-2] == [text for _, _, text in original_rows[:-1]]
          and texts[-3:] == ["test al,al", "nop", "nop"] and texts[-4] == "call eax"
          and original_rows[-1][0] == P.RUN_UNFOCUSED_BRANCH_VA and original_rows[-1][2].startswith("jz")
          and branch_target(original_rows[-1][2]) == 0x1A113,
          f"{va:#x}: the active check's `{original_rows[-1][2]}` at {P.RUN_UNFOCUSED_BRANCH_VA:#x} is two nops; "
          "the call and the test before it are the original's; the build without the late join keeps the jump")
    # scene::remove_model: the branch of a model that is not in the scene
    # is a jump to the function's own end, where it lets go of the model it
    # was given and returns.
    va, old, new = P.REMOVE_MODEL
    rows, original_rows = rows_at(va, len(old)), ndisasm(Image(original).read(va - 8, len(old) + 8), va - 8)
    check(image.read(va, len(old)) == new and Image(original).read(va, len(old)) == old
          and before.read(va, len(old)) == old
          and [text for _, _, text in original_rows[:2]] == ["cmp edi,esi", "jnz near 0x15006c"]
          and rows[0][2].startswith("jmp") and branch_target(rows[0][2]) == P.REMOVE_MODEL_END
          and [text for _, _, text in rows[1:]] == ["nop"]
          and [text for _, _, text in ndisasm(Image(original).read(P.REMOVE_MODEL_END, 0x2C), P.REMOVE_MODEL_END)]
          == ["mov eax,[ebp+0xc]", "test eax,eax", "jz 0x150215", "add eax,0xd0", "or ecx,byte -0x1",
              "lock xadd [eax],ecx", "jnz 0x150215", "mov ecx,[ebp+0xc]", "lea eax,[ecx+0xd0]", "call 0xe0210",
              "pop edi", "pop esi", "pop ebx", "mov esp,ebp", "pop ebp", "ret 0x8"],
          f"{va:#x}: scene::remove_model's branch for a model that is not in the scene is `{rows[0][2]}` + nop, "
          "to the function's own end (it lets go of the model it was given, then returns); the build without "
          "the late join keeps the original")
    ready = ndisasm(image.read(P.LATE_JOIN_VA + P.LATE_JOIN_READY_OFFSET, 128), P.LATE_JOIN_VA + P.LATE_JOIN_READY_OFFSET)
    onward = [branch_target(text) for _, _, text in ready if text.startswith("jmp")]
    check(ready[0][2] == "pusha" and onward[:1] == [P.SECTION_VA],
          f"the ready hook's code starts with pushad and jumps on to the upload code at {P.SECTION_VA:#x}")

    sites = [(va, va + size) for va, size in late_join_sites()]
    under = lambda relocations: [va for kind, va in relocations.values()
                                 if kind == 3 and any(va + 4 > lo and va < hi for lo, hi in sites)]
    # The sites with an address in them: scene::remove_model's branch, the
    # field-of-view setting's two in place, the pickup's distance and the
    # lobby camera's three loads of its least distance.
    fov_sites = {va for va, _, _ in P.FOV_PATCHES}
    with_address = {P.REMOVE_MODEL_VA, P.PICKUP_HOOKS[0][0]} | fov_sites | \
        {va for va, old, _, _ in P.WEAPON_PREVIEW_HOOKS if old[:3] == b"\xf3\x0f\x10"} | \
        {va for va, _, _, _ in P.FOV_LOD_HOOKS} | \
        {va for va, _, _, _ in P.HIDE_UI_TABLE_REFS}   # the options' table, now the arena's (relocated alike)
    kept = [(lo, hi) for lo, hi in sites if lo not in with_address]
    under_kept = [va for kind, va in Image(original).relocations().values()
                  if kind == 3 and any(va + 4 > lo and va < hi for lo, hi in kept)]
    under_fov = sorted(va for kind, va in Image(original).relocations().values()
                       if kind == 3 and any(va + 4 > lo and va < hi for lo, hi in sites if lo in fov_sites))
    option = P.FOV_OPTION_NAME[0] + 1
    now, was = image.relocations(), before.relocations()
    differing = sorted(key for key in set(now) | set(was) if now.get(key) != was.get(key))
    check(not under_kept and sorted(was[key][1] for key in differing) == sorted(disabled)
          and all(was[key][0] == 3 and now[key][0] == 0 for key in differing)
          and under_fov == sorted({option, P.FOV_MINIMUM_RELOC})
          and any(kind == 3 and va == option for kind, va in now.values()),
          "no base relocation is under a late-join site but those of scene::remove_model's branch "
          f"({P.REMOVE_MODEL_VA + 2:#x}), of uro_fov's least ({P.FOV_MINIMUM_RELOC:#x}) and of the pickup's "
          f"distance ({P.PICKUP_RELOC:#x}) and of the lobby camera's least "
          f"({', '.join(f'{va:#x}' for va in P.WEAPON_PREVIEW_RELOCS)}) and of patch 40's two constants "
          f"({', '.join(f'{va:#x}' for va in P.FOV_LOD_RELOCS)}), which are disabled, "
          "and of the options' controls table's six readers (patch 62: their address is the arena's, kept "
          "relocatable) "
          f"and of the field-of-view option's name "
          f"({option:#x}, an address for an address), which stays; the rest of the table is the build's without it")

    test_clock_factor(original, image, before)
    test_statistics_count(original, image, before)
    test_pregame_progress(original, image, before)
    test_fov(original, image, before)
    test_fov_lod(original, image, before)
    test_weapon_camo(original, image, before)
    test_pickup(original, image, before)
    test_weapon_random(original, image, before)
    test_kit_state(original, image, before)
    test_first_load(original, image, before)
    test_tdm_deaths(original, image, before)
    test_replay_reattach(original, image, before)
    test_sound_queue(original, image, before)
    test_seek_clear(original, image, before)
    test_db_signature(original, image, before)
    test_stale_dll(original, image, before)
    test_mount_patches(original, image, before)
    test_forward_scroll(original, image, before)
    test_replay_link(original, image, before)
    test_shared_upgrades(original, image, before)
    test_kit_change(original, image, before)
    test_warmup_look(original, image, before)
    test_warmup_keys(original, image, before)
    test_death_camera(original, image, before)
    test_read_order(original, image, before)
    test_baked_texture_missing(original, image, before)
    test_ai_arena(original, image, before)
    test_memory_defaults(original, image, before)
    test_weapon_preview(original, image, before)
    test_weapon_table(original, image, before)
    test_weapon_decals(original, image, before)
    test_upgrade_preview(original, image, before)
    test_research(original, image, before)
    test_suicide_key(original, image, before)
    test_blackout(original, image, before)
    test_tab_list(original, image, before)
    test_tab_self(original, image, before)
    test_weapon_show(original, image, before)
    test_spot(original, image, before)
    test_artefacts(original, image, before)
    test_match_keys(original, image, before)
    test_hide_ui(original, image, before)
    test_kill_feed(original, image, before)
    test_fall_gravity(original, image, before)
    test_bake_cache(original, image, before)
    test_ragdoll(original, image, before)
    test_lamps(original, image, before)
    test_language(original, image, before)
    test_weapon_kit(original, image, before)
    test_lobby_scene(original, image, before)
    test_weapon_flight(original, image, before)
    test_stage_focus(original, image, before)
    test_revision(original, image, "section", P.REVISION, late_join=True)
    test_revision_override(original, data, server)
    test_required_patches(original, data, server)
    test_hash_interval(original, image, "section")
    test_memory_check(original, image, "section")
    test_default_server(original, image, "section", server)
    test_gather(original, image, "section")
    test_start_up_folder(original, image, "section")
    suffix = "" if server is None else (".embedded-max" if len(server) > 63 else ".embedded")
    path = os.path.join(SCRATCH, f"survarium_srp.section{suffix}.late-join.exe")
    with open(path, "wb") as out:
        out.write(data)
    print(f"  written: {path} (sha256 {hashlib.sha256(data).hexdigest()})")
    test_windows(path, source, "section")


def test_windows(path, original_path, mode):
    crypt32 = ctypes.WinDLL("crypt32", use_last_error=True)
    imagehlp = ctypes.WinDLL("imagehlp", use_last_error=True)
    header_sum, check_sum = ctypes.c_uint32(), ctypes.c_uint32()
    result = imagehlp.MapFileAndCheckSumW(ctypes.c_wchar_p(path), ctypes.byref(header_sum), ctypes.byref(check_sum))
    if mode == "cert-only":
        check(result == 0, f"imagehlp reads the file (header checksum kept at {header_sum.value:#x}, "
              f"computed {check_sum.value:#x}; the loader does not check it for an exe)")
    else:
        check(result == 0 and header_sum.value == check_sum.value,
              f"PE checksum {header_sum.value:#x} = imagehlp's {check_sum.value:#x}")

    crypt32.CryptQueryObject.argtypes = [ctypes.c_uint32, ctypes.c_void_p] + [ctypes.c_uint32] * 3 + [ctypes.c_void_p] * 6
    crypt32.CertFindCertificateInStore.restype = ctypes.c_void_p
    crypt32.CertFindCertificateInStore.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32,
                                                   ctypes.c_uint32, ctypes.c_void_p, ctypes.c_void_p]

    def certificate(file):
        store, message = ctypes.c_void_p(), ctypes.c_void_p()
        name = ctypes.create_unicode_buffer(file)
        # The client's call at 0x1335B: CERT_QUERY_OBJECT_FILE (1),
        # CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED (0x400),
        # CERT_QUERY_FORMAT_FLAG_BINARY (2), then store and message out.
        if not crypt32.CryptQueryObject(1, name, 0x400, 2, 0, None, None, None,
                                        ctypes.byref(store), ctypes.byref(message), None):
            return None
        # CertFindCertificateInStore(store, X509 | PKCS7, 0, CERT_FIND_SUBJECT_STR_W, L"survarium.com", 0)
        subject = ctypes.create_unicode_buffer("survarium.com")
        context = crypt32.CertFindCertificateInStore(store, 0x10001, 0, 0x00080007, subject, None)
        if not context:
            return None
        # CERT_CONTEXT {DWORD encoding; BYTE *encoded; DWORD size; ...}, native layout
        encoding, encoded, size = struct.unpack_from("@IPI", ctypes.string_at(context, 3 * ctypes.sizeof(ctypes.c_void_p)))
        return ctypes.string_at(encoded, size)

    ours, theirs = certificate(path), certificate(original_path)
    check(ours is not None and ours == theirs,
          "CryptQueryObject (the client's own call) finds the survarium.com certificate, identical to the "
          f"original's ({len(ours or b'')} bytes, sha1 {hashlib.sha1(ours or b'').hexdigest()})")


def main():
    source = sys.argv[1] if len(sys.argv) > 1 else ORIGINAL
    os.environ["SRP_026G_EXE"] = os.path.abspath(source)     # the Rust span hooks' bytes (hooks_026g.py)
    original = open(source, "rb").read()
    os.makedirs(SCRATCH, exist_ok=True)
    game = os.path.abspath(os.path.dirname(source))
    try:
        inside = os.path.commonpath([os.path.abspath(SCRATCH), game]) == game
    except ValueError:  # another drive: not inside
        inside = False
    if inside:
        sys.exit("the scratch directory must not be inside the game folder")
    test_bins()
    test_late_join_bin()
    test_no_branch_into_hooks(original)
    test_server_options(original)
    print("variant section, the arena and the call shapes")
    test_arena(original)
    test_engine(original)
    test_hook_kinds(original)
    test_apply(original)
    test_call_shapes_negative(original)
    variants = [(mode, None) for mode in ("cert-only", "section", "hash-mismatch")]
    variants += [(mode, "026g.skhid.link:1234") for mode in P.CAVES]
    variants += [(mode, "a" * 63 + ":65535") for mode in P.CAVES]
    for mode, server in variants:
        print(f"variant {mode}, embedded server {server}")
        code = None if mode == "cert-only" else open(P.default_bin(mode), "rb").read()
        data, description = P.build(original, mode, code, server)
        print("  " + description)
        if mode != "cert-only":                            # the variant's own installs, for the stale check
            P.audit_installs(data, P.WRITES)
            INSTALLED_SEEN.update(P.installs_of(data, P.WRITES))
        check(hashlib.sha256(original).hexdigest() == P.EXE_SHA256, "the original in memory is untouched")
        image = Image(data)
        cave = b"" if code is None else P.split_code(code)[0]
        payload = cave if server is None else cave + open(P.default_server_bin(mode), "rb").read() + server.encode("ascii") + b"\0"
        test_pe(original, image, mode)
        test_bytes(original, image, mode, payload, server)
        test_hash_interval(original, image, mode)
        test_memory_check(original, image, mode)
        test_default_server(original, image, mode, server)
        test_revision(original, image, mode, P.REVISION_WITHOUT_LATE_JOIN)
        test_gather(original, image, mode)
        test_start_up_folder(original, image, mode)
        if mode == "section":
            test_late_join(original, data, server, source)
        if mode == "cert-only":
            if os.path.exists(INSTALLED_CERT_ONLY):
                check(open(INSTALLED_CERT_ONLY, "rb").read() == bytes(data),
                      "identical to the survarium_srp.exe installed in step 0 (read only)")
            check(image.read(P.CERT[0], 3) == P.CERT[2] and ndisasm(P.CERT[2], P.CERT[0])[1][2] == "ret",
                  "0x12A10 is `xor eax,eax; ret`")
        else:
            test_code(original, image, mode, cave)
            check(image.read(P.MUTEX[0], 2) == b"\x90\x90" and image.read(P.CERT[0], 3) == P.CERT[2],
                  "mutex and certificate patches present")
        suffix = "" if server is None else (".embedded-max" if len(server) > 63 else ".embedded")
        path = os.path.join(SCRATCH, f"survarium_srp.{mode}{suffix}.exe")
        with open(path, "wb") as out:
            out.write(data)
        print(f"  written: {path} (sha256 {hashlib.sha256(data).hexdigest()})")
        test_windows(path, source, mode)
    stale = sorted(set(P.INSTALLS) - INSTALLED_SEEN)
    check(not stale, f"every INSTALLS row is installed by some variant built here ({len(INSTALLED_SEEN)} sites)"
          + (": stale " + ", ".join(f"{va:#x}" for va in stale) if stale else ""))
    print()
    print(f"{len(failures)} FAILED" if failures else "all checks passed")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
