# Hitting with the battery (patch 143)

- **The owner (2026-11-14):** "start work to allow hitting with battery", "do implement some cool
  animation for that", "similar to dirty bomb".
- **Read as (for the owner to correct):** the melee key works while carrying a battery: a quick,
  punchy thrust of the battery itself (Dirty Bomb's quick melee: fast, the carried thing is the
  weapon, straight back to the carry), the carrier keeps the battery, everyone sees it.

## Today (PROVEN, static)

- A carried battery is the player's held item (+128h): `victory_item_core` with a state machine
  (`vostok::ai::fsm` at item +168h, its current state at +178h) of three states made by
  `initialize_logic` 0x301920, in this order: take (0x338940, kind +20h = 1), carry (0x338BE0,
  kind 3), put (0x338F00, kind 2). Transitions: take -> carry when the take's clip ended
  (0x372420 after its readiness [+28h]); carry -> put when the user switches away (0x301B30:
  held +128h != next +12Ch and +33Ch set).
- Melee is the gun's alone: `weapon_core::melee_pred` 0x2E4100 reads the user's actions
  (+330h bit 27) for the gun's state machine; the battery's states never read it. While a
  battery is held the gun is not the held item, so the key does nothing.
- A gun-stock hit: the melee state's "shoot" clip event (0x33AB50, subscribed by
  `weapon_core_melee_state_base::initialize` 0x33A990) emits one bullet (`emit_bullet` 0x322830)
  from the eye (player +44Ch) along its forward row times the gun's `melee/data/distance`
  (+578h, 1.7 m) with the gun's melee data (+53Ch: blunt, `combat_log_icon` 48).
- Every game deals every hit (lockstep): what is added changes what a match computes.
- Each state's clips come from the battery's config (`gameplay/victory_items/default.options`
  `states[i]`: 4 player clips, 2 battery clips); the cook loads every `states[]` entry
  (0x2FA1A0 loop) and the callback hands `initialize_logic` three arrays of 6 (0x2FA7C0).
- Only five battery clips exist (`key_objects`: show, idle, hidetest, idle_look,
  on_site_death_fwd); the only melee clips are the rifles' and pistols' `kick` (gun-stock).

## The rules

- **When:** carrying (the carry state), the melee key down (level, as the gun's: held, it
  swings again once a swing ends). Not during the take or the put, not while switching away,
  not in the air (the gun-stock hit's rule, weapon_core::melee_pred 0x2E4100: the posture's
  motion +24h 4 is a jump, player_logic_stand_jump_state 0x336B87; 0 on site, 2 run, 3 sprint,
  so sprinting may swing as the gun may; read from the battery's clip set). The gun's other
  rule, not aimed (+5E8h), has no carrier's counterpart. melee_pred has no +5ECh test.
- **Who:** every carrier, bots too (their `attack_melee` presses the key in melee range).
- **Effect:** at the swing's end, one melee bullet as the gun-stock hit's: from the eye, along
  the look, the carrier's primary gun's (slot 7) melee data and distance. The carrier keeps the
  battery.
- **Damage (assumed):** the gun's melee data is blunt with an attacker, so patch 108 kills at
  once wherever it lands. A battery's own damage would need its own bullet data: the owner's
  call.
- **Kill feed:** the melee icon (the data's `combat_log_icon` 48), no headshot mark (patch
  67's melee rule); statistics count a melee kill (13) of the kit.

## The animation

- **The owner (2026-11-15, after playing 8dd7a44df):** the put clip as a thrust "is very bad",
  "it clips", "it doesn't have a force"; a cool animation "similar to Dirty Bomb" (a quick melee
  with weight: a clear wind-up, a snappy strike, follow-through, a camera punch); "it should
  properly do that, we don't even need a game for animating": authored offline on the real
  assets, previewed before any in-game test.
- **The first-person camera** (where the keys are judged): the Item bone. computed_head_transform
  (0x2DA4B0) stores bone "Item" (0x9DE38C) times the player's transform (+40Ch) as the eye
  (+44Ch); update_camera (0x1F3480) hands +44Ch to the game camera (0x2422B0) unchanged and sets
  the near plane to 0.05 (0x9CC878) times the zoom (1 unaimed); the projection is uro_fov
  degrees vertical (patch 22; 65.0 by default and in the owner's user.cfg). Item is a child of
  Root, so the body's moves do not move the view; the look pitch does (the battery clip set's
  additive `idle_look`, frame 18 level, Item 66 deg down at 36). First person draws the body
  without the head, the hud hands for the hands: the AK's idle drawn so puts the hand and the
  sleeve where a screenshot of the game has them.
- **What the stock carry shows in first person:** nothing, looking level. The battery's nearest
  edge is 40 deg below the view (the screen's edge is 32.5): it shows only when looking 10 deg
  down or more (`carry_idle_fp.png`: 0, 20 and 35 deg). The put's clips that the first swing
  played (`hidetest`, `hidetest_electrobox`) move the hands 7-9 cm and the battery a few cm and
  nothing else, so that swing showed nothing either, looking level.
- **Authored** (`scripts/battery-hit`): keys over the stock carry idle on the stock rig, both
  hands solved onto the battery's handles every frame (two-bone IK, the elbow in its idle
  plane), the battery attached as the game attaches it (Ry(pi) x the Weapon bone, which on the
  carry idle puts both handle bones exactly on the hands); designed for the first-person view,
  each key's battery centre solved onto a chosen point of the screen at a chosen depth within the
  arms' reach:
  - lift (0.07 s): in from the lower right;
  - wind-up (0.13 s): cocked high at the screen's right edge (over the right shoulder in third
    person), the chest wound right;
  - the strike down across the lower half, the impact at 0.2 s at the screen's centre, both
    hands on screen, the chest turned into it;
  - follow-through to the lower left (0.3 s, past the left hip), a camera punch (the Item bone:
    5 deg down, 2.5 deg aside, after the impact so the strike's aim is the clip's own);
  - recovery: down out of view, the carry idle by 0.57 s.
  The battery (0.66 x 0.36 x 0.31 m) held within reach is 0.25-0.45 m from the eye, where it
  spans most of the screen's width: at the wind-up and the follow-through one handle (and its
  hand) is past the screen's edge.
- **Two clips** at 30 fps, written in the engine's format with its own spline basis
  (`clips.py`, resource-porter's `skeleton_animation.rs` transcribed): `battery_hit` (6 frames,
  the wind-up and strike, its last frame the impact) and `battery_hit_recover` (11 frames), with
  the carry idle's bone names; shipped by the edit list `battery-hit-1.json` (op
  `files_from_repo`, the files pinned by sha256, `make_clips.py --check` keeps them equal to
  the keys) as the battery config's `states[3]` and `states[4]`, which the cook loads as it
  loads states 0-2.
- **Checked offline** (`preview.py`, the renders in `private/renders/battery-hit/`, with
  `metrics.txt`): in first person (the camera above) the battery is on screen from 0.07 s to
  0.43 s, both hands from 0.17 to 0.23 s (the impact at 0.2 s), and nothing crosses the near
  plane in view on any frame; in third person (three views, every frame) no torso, neck, head
  or helmet vertex is inside the battery's box and the hands are on the handles within 12 mm;
  the before (the put's clips) beside it.
- **The spine and the camera in a hands_only clip:** the stock gun-stock hit's clip
  (`ak/.../hands_only/kick`, additive) turns the spine 16 deg and moves the neck, head and Item,
  so the engine plays those bones of such a clip; these clips (non-additive, as the battery's
  stock ones) are not seen in game yet.
- **Seen by all:** the swing and the recovery are states of the battery's machine; the
  player's animation tree is simulation state, drawn for every view; first person draws the
  same body. The battery follows the Weapon bone each tick (0x344E40).
- **Sound:** the strike is the gun-stock bullet (melee data, material 74 `melee_blunt`), so a
  body hit plays stock `material/body/melee/melee_body_hits` (a 0.2 s thud), metal and wood
  their melee pairs; concrete, ground, stone and glass are silent, as for the gun-stock hit.
  Heavier candidates (STALKER SoC thuds, a port) are in `private/renders/battery-hit/sounds/`
  for the owner to pick; none wired.
- **Not seen in game yet:** the engine's blending between clips, locomotion, the look pitch.

## Timing

- Press to impact 0.2 s (`battery_hit::SWING_S`, the clip's 6 frames; the take state scales
  its clip to it), the strike on the swing's end; the recovery 0.37 s (`RECOVER_S`), then the
  carry. Held, the next swing starts from the carry: one strike every 0.57 s at most. The
  gun-stock strike's clip (`ak/.../kick`) is 6 frames too.

- The strike is when the swing's clip ends (the state's readiness, set by the animation
  player's end event, as the take's), on the same tick in every game.

## Edge cases

- **Dropping or switching mid-swing:** the swing ends first (the state goes on only once its
  clip ended), then the put; the strike still lands (PROVEN in the check).
- **Put away mid-swing** (a delivery, a drop, a death: `victory_item_core::deactivate` 0x300F40
  calls the current state's finalize, 0x300FA5): the swing's finalize strikes only when its clip
  ended and its carrier lives (PROVEN in the check, the game's own deactivate). So a battery put
  away on the tick its thrust ended still strikes; a dead carrier never does.
- **Death mid-swing:** the swing answers "may be put away" as the carry does (its vtable +20h:
  yes once the carrier is dead, 0x338E90's rule; the take's own answers never, 0x397C20), so a
  dead carrier's battery is let go as from the carry (PROVEN in the check through
  is_ready_to_be_deactivated 0x301270). A new pickup starts at the take (initialize 0x300BF6).
- **Crouch, prone, sprint:** one clip for all (the config's 4 player clips are one), as the
  stock battery states.
- **Late join, resync, replays:** the swing and the recovery are the machine's fourth and fifth
  states (indices 3, 4), saved and
  loaded by the game's own fsm serialize (0x38CC90: the index, then the state's fields, a
  take's layout; PROVEN in the check: saved mid-swing, loaded by another game into its own
  swing, every field the same); `srp-validate` reads 3 as `BatteryState::Swing`, 4 as
  `BatteryState::Recover`. A bullet
  in flight is loaded with its gun from the shooter's slot (0x32BDA1..0x32BDAE), deactivated
  there too: the substitutions below need nothing the loading game lacks. Replays recorded
  before the release are refused (`simulation_changes.tsv` `next`).

## The change (exe patch 143, Rust `battery_hit`)

- `battery.logic` (0x2FA7E9, the cook callback after `initialize_logic`): two states, take
  states (0x338940) over the config's `states[3]` and `states[4]` clips, which the cook loaded
  (its queries_result, the callback's argument: state i's six clips at results 2 + 6i, the
  managed resources at +128h, 2E0h apart, as 0x2FA52D reads the take's), with `SWING_S` and
  `RECOVER_S`; appended to the machine's list; transitions carry -> swing (a function-pointer
  functor, our function: the carrier alive, melee down, not in the air; after carry -> put),
  swing -> put and swing -> recovery, recovery -> put and recovery -> carry (byte copies of
  carry -> put's and take -> carry's functors), each tried once its clip ended. Results without
  states 3 and 4 (a game without the content) are the patch's crash.
- Their identity, slot by slot (both): kind 3, the carry's (read by is_carrying 0x3018C8 and the clip
  set's key 0x3012EA: the carrier's body animates and counts as carrying through it); its
  vtable a copy of the take's (readiness +18h: its clip ended; expression +24h: the take's, the
  thrust starts at once, the put's blends in over 0.3 s, 0x339107; is_user_dead +28h shared by
  all three), with finalize (+0Ch: the strike, the swing's only) and may-be-put-away (+20h: the
  carry's rule) ours.
- A battery whose machine is not the three states, or no memory, or a carrier who cannot
  strike: a crash (`expect`), not a skip.
- The strike: `emit_bullet` with the arguments the gun-stock hit gives it (0x33AB50), EDI the
  gun (slot 7, else 10), the time the carrier's. Nothing of the gun is written.
- **The bullet and the deactivated gun.** A carrier's guns are deactivated (deactivate
  0x2E2EE0: user +8 0, its clip set's posture +278h -> +20h 0). The bullet keeps its gun (+0Ch,
  the gun's item, 0x32BB12) and reads that user and posture through it all its life:
  - the stance at the constructor (`weapon_core::player_stance` 0x2E43A0, the gun's vtable
    +7Ch: the user's alive +33Ch, the posture's kind);
  - the melee damage, `weapon::damage` 0x2E1CF0 and `weapon::min_damage` 0x2E1C80, melee branch
    (the user's kit +115h, profile +6B8h): the constructor 0x32BB38, update_bullet_position at
    every flight step 0x32F09A and 0x32F0B6, the hit dispatch 0x32DA5A, the pierce or ricochet
    0x32E35C (the five callers).
  Four filtered lohooks at those reads (`gun.stance_user` 0x2E43A3, `gun.stance_posture`
  0x2E43B9, `gun.melee_damage_user` 0x2E1D00, `gun.melee_falloff_user` 0x2E1C99), their entries
  calling the handler only when the read gave 0 (never natively: no gun shoots while not held),
  put in the register what a held gun would give: the gun's player (its item's inventory +250h,
  the inventory's holder +174h, the player's +108h interface: 0x1F17A0 in both player classes'
  vtables, so every game has it, a late joiner's and a replay's too) and that player's held
  battery's posture (+1ACh -> +20h, the clip set states of the gun's own kind). No write, so no
  lifetime and nothing in a saved world. Should the gun have no player or its player no
  battery's posture, the handler panics with the patch's name (never in a match: kits are built
  with their inventory). At the two damage sites (after `fld [eax+8]`, after `fld1`) one x87
  slot is in use: their handlers do no x87 work, and must not.
  Not used: the gun's +594h (only set_user writes it; a
  joiner's never-held gun has 0); a lend of the user (weapon_core::serialize writes its in-hand
  part only when +8 is set, 0x2E3232: a lend alive at a save would misalign every loading game).

## The in-game tests

- **2026-11-14, build 56dfcf96d:** the owner pressed melee while carrying: a crash at 0x2E43A3
  reading 0x33C. The dump: the gun 0x7C932B48 (vtable 0xA04174) with user +8 0 and no posture;
  0x32BAE2 (the constructor's call of +7Ch), 0x3228A4 (emit_bullet), 0xA4373BD (the strike).
  The first check had stubbed emit_bullet whole.
- **2026-11-15, build 30f8ea538** (the gun lent its user and posture for the emit only): a
  crash at 0x2E1D00 reading 0x115. The dump: the bullet 0x02AC323C (+0Ch 0x7C944990 the gun's
  item, +40h the gun's melee data, +8 its shooter), the gun 0x7C944980 with user 0 again;
  0x32F09F (update_bullet_position after 0x2E1CF0), 0x32C2AC (bullet::tick), 0x32271A
  (tick_bullets), 0x321E77 (bullet_manager::tick). That check stubbed the live-list add, so
  bullet::tick never ran; and the spec's "the flight reads no user" scan looked for direct
  reads only, not for calls of 0x2E1CF0 / 0x2E1C80.
- Both are the check's controls now, in the original exe.

## Checked

- `check_battery_hit_026g.py` (Unicorn, the patched exe beside the original):
  - initialize_logic and the site make the five states (the swing and the recovery over the
    cook's clips of states 3 and 4, scaled to 0.2 s and 0.367 s) and their transitions; the
    vtable copy has exactly slots +0Ch and +20h ours; results without states 3 and 4: the
    patch's crash;
  - fsm::tick from the carry takes the swing only for the melee key and a living carrier, the
    put first on a switch;
  - in the swing nothing until the animation player's end event (0x344830) of its own clip,
    another clip's end changes nothing; then the recovery (or the put) and one strike, made by the
    game's own emit_bullet and bullet constructor: the gun's item its source, the melee data,
    damage 1.5, its stance the one the original gives a held gun in that posture; the gun's
    bytes unchanged; the recovery's clip end: the carry, no second strike;
  - the bullet's flight with the game's own bullet::tick 0x32C180: a tick meeting nothing (every
    step reading the gun's melee damage), and one meeting a body (check_collision, the step to
    the point, try_pierce hitting the target once with the carrier as shooter and the gun's item
    as source, reading the damage again at 0x32DA5A); the gun never written;
  - put away mid-swing through the game's deactivate: the strike only with the clip ended and
    the carrier alive; a dead carrier's swing may be put away, a living one's not;
  - saved mid-swing and loaded by another game: its swing current, every field the same;
  - no gun in either slot (never in a match): the patch's own crash;
  - the melee key while the carrier's posture is a jump: the carry stays (sprint, run: the
    swing); a strike whose gun has no player: the substitution's named panic;
  - controls in the original exe: the constructor faults at 0x2E43A3, the flight at 0x2E1D00.
  Stubbed: the allocator, a clip's time scale, the animation event channels, the battery's
  transform calls, the bullet pool's room and live-list add, the physics world's ray test and
  bounds, the struck target.
- `cargo test` and Clippy of the patches crate and srp-validate (states 3 and 4 read, 5 refused).
- The clips: `make_clips.py --check` (the committed files are the keys'); the offline previews
  (no near-plane crossing, no body inside the battery, every frame).
- `check_world_roundtrip_026g.py`: its three worlds pass; its three coverage failures (0x252B48,
  0x310224, 0x33A51B, other patches' sites) fail the same on origin/main.
- Not proven: the pierce or ricochet at 0x32E35C (the same 0x2E1C80, its lohook, not run);
  other hittables' hit (+24h) with a strike's source item (lamps, a battery's body): not
  traced; the victim's hit_impl and the kill and hit events read the source item's slot and
  dictionary entry, never its gun's user (static, 0x2D9F97, 0x2DA1EA, 0x2DA3F7).
- Not played: the authored swing, the bullet's hit in a match.

## Release

- Exe: the patch; `simulation_changes.tsv` `next`.
- World format: battery states 3 and 4 (srp-validate, and so patch 87's check in every game).
- Content: `battery-hit-1.json` (the two clips, the config's states 3 and 4). The exe needs it:
  without it every battery crashes the game at its cook (by design: it would play otherwise).

## Open questions for the owner

- The look: the authored swing (the previews); faster, bigger, overhead?
- Kill at once (patch 108) or a battery's own damage?
- Speed: impact at 0.2 s, 0.57 s a swing; a cooldown?
- A sound: the stock body thud plays already; a heavier one (the SoC candidates) or a swing
  whoosh (the rifles' `melee_rifle`)?
- A battery icon in the kill feed instead of the melee one?
