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
  swings again once a swing ends). Not during the take or the put, not while switching away.
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

- **Chosen:** the battery's own put clip (`hands_only/hidetest`, both hands push the battery
  forward into a slot, with the battery's grip) played as a thrust in 0.3 s (the put plays it
  in 0.6 s), then the game's own blend back to the carry idle. Why: it holds the battery as
  the battery is held (a rifle clip would put the hands in a rifle's grip), it is a forward
  thrust, it is loaded and registered wherever batteries are (the put's), and it needs no
  content.
- **Seen by all:** the swing is a state of the battery's machine; the player's animation tree
  is simulation state, drawn for every view; first person draws the same body. The battery
  follows the hands' `Weapon` bone each tick (0x344E40) as in the carry.
- **Other routes, if it does not look good:** the rifles' `kick` (a fast gun-stock strike; the
  battery would follow the rifle's path, hands in a rifle's grip; needs `states[3]` in the
  battery's config and its clips registered); a procedural arm layer on the carry pose
  (patch 78's `artefact_carry::pose` way: an overhead swing of our own, deterministic from game
  time), the "cool" one, more work.
- **Not seen yet:** the clip has never been played this fast or as a hit. The look needs the
  owner in game.

## Timing

- Press to strike 0.3 s (the thrust's length, `battery_hit::SWING_S`), then straight to the carry
  (its idle clip; GUESS: the game blends the two, not traced). Held, the next swing starts at
  once: one strike every 0.3 s at most. The gun-stock strike lasts the `kick` clip's length.
- The strike is when the clip ends (the state's readiness, set by the animation player's end
  event, as the take's), on the same tick in every game.

## Edge cases

- **Dropping or switching mid-swing:** the swing ends first (the state goes on only once its
  clip ended), then the put; the strike still lands (PROVEN in the check).
- **Delivering mid-swing:** GUESS: a container takes the battery as from the carry (its use is
  instant, 0x31CC40; not traced through the swing).
- **Death mid-swing:** LIKELY the battery is dropped as from the carry (the death path not
  traced); its finalize then strikes nothing, the clip not ended (PROVEN in the check). A new
  pickup starts at the take (initialize 0x300BF6).
- **Crouch, prone, sprint:** the put's clips as they are (its 4 player clips are one).
- **Late join, resync, replays:** the swing is the machine's fourth state (index 3), saved and
  loaded by the game's own fsm serialize (0x38CC90: the index, then the state's fields, a
  take's layout); `srp-validate` reads index 3 as `BatteryState::Swing`. Replays recorded before
  the release are refused (`simulation_changes.tsv` `next`).

## The change (exe patch 143, Rust `battery_hit`)

- `battery.logic` (0x2FA7E9, the cook callback after `initialize_logic`): a fourth state, a
  take state (0x338940) on the put's six clips with `SWING_S`, kind 3 (carry, so every native
  "is carrying" test holds through it), its vtable a copy whose finalize strikes; appended to
  the machine's list; transitions carry -> swing (a function-pointer functor, our function: the
  carrier alive, melee down; after carry -> put), swing -> put and swing -> carry (byte copies of
  carry -> put's and take -> carry's functors), tried once its clip ended.
- A battery whose machine is not the three states, or no memory: a crash (`expect`), not a
  game left without the swing (it would play otherwise than the others).
- The strike: `emit_bullet` with the arguments the gun-stock hit gives it (0x33AB50), EDI the
  gun (slot 7, else 10; none: no strike), the time the carrier's.

## Checked

- `check_battery_hit_026g.py` (Unicorn, the patched exe beside the original): initialize_logic
  and the site make the four states and five transitions as above; fsm::tick from the carry takes
  the swing only for the melee key and a living carrier, the put first on a switch; in the swing
  nothing until its clip ended, then the carry (or the put) and one strike with the gun-stock
  arguments (slot 10's gun when 7 is empty); left early (a drop), no strike. Stubbed: the
  allocator, a clip's time scale, the animation event channels, the battery's transform calls,
  emit_bullet itself.
- `cargo test` and Clippy of the patches crate and srp-validate (state 3 read, 4 refused).
- Not played: the clip at this speed, a real clip end, the bullet's hit in a match.

## Release

- Exe: the patch; `simulation_changes.tsv` `next`.
- World format: battery state 3 (srp-validate, and so patch 87's check in every game).
- No content.

## Open questions for the owner

- The look: the put clip as a thrust, or the rifles' `kick`, or a procedural overhead swing?
- Kill at once (patch 108) or a battery's own damage?
- Speed: 0.3 s a swing; a cooldown?
- A sound (the rifles' `weapons/melee/melee_rifle`)?
- A battery icon in the kill feed instead of the melee one?
