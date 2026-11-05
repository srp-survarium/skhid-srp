# What a player sees and hears of a thrown grenade (0.26g), and what the server knows

For the bots' live-grenade list (`bots/live_grenades.rs`) and the avoidance built on it. Tags: PROVEN = read in the exe or
the data (VAs with image base 0x10000), LIKELY, GUESS.

## The client

- The thrown RGD-5 is a render model, `item/grenade_grd5` (`data/model` of `rgd-5.options`), capsule 0.05 x 0.10 m. The
  config has no particle, light or trail field. PROVEN.
- The client adds it to the scene at the launch (`0x248520` calls `scene_renderer::add_model`), moves it each tick from the
  physics body (`0x248650` calls `update_model`) and removes it at the blast (`0x248720` calls vtable+0x1C, which is
  `0x248600` calling `remove_model`). So it is visible from the launch to the blast: about 0.7 s in flight and about 2.2 s on
  the ground (the launch is press + 1.1 s, LIKELY; the blast press + 4.0 s, PROVEN).
- No fuse cue: `start_fuse` (`0x303B90`) only inserts the object into tick list 6 (`0x2D2D70`); the config has no fuse
  sound or blink field. PROVEN.
- No HUD warning, marker, direction arrow or minimap icon for a grenade: the minimap icon table has only base, flag,
  artifact, container, player, battery spawn, resupply and research point (`flash/hud/src/Minimap.as:50-66`); the only
  grenade code in the HUD is `GrenadeEffect.as`, the screen overlay of `gameplay/effects/effect_hit_explosion.options`
  (`hud_effect_type` 4, keyframes 1.0 at 0 s, 0.5 at 2 s, 0 at 4 s), probably played only for a player hit by an explosion
  (LIKELY, the trigger is not traced). PROVEN for the Flash sources.
- Sounds (the distance curves are the `.spl` volume curves; that the client applies them as the attenuation is LIKELY):
  - throw (`states/throw_grenade` of `wpn_pm`, `wpn_ak_74n`, `wpn_svd`, the same for first and third view): the pin out
    (`melee_wear`, silent from 5 m), `grenade_wear` (silent from 5 m) and `grenade_second_pin_out` (0.8 at 0.1 m, 0.24 at
    5 m, silent from 17 m). PROVEN as data.
  - bounce: on contact (`0x248980`) the client looks up the pair's hit sound for the grenade's material 45 (`0x306D50`)
    and plays it if at least `min_roll_time_ms` 400 passed since the last. Stock `weapons_reloading` (silent from 9 m);
    the live patch `grenade-bounce-1` makes it `mid_range`: 1.0 at 0 m, 0.43 at 3.5 m, 0.14 at 8 m, silent from 20 m, times
    `volume_k` 0.5 (ground). A resting grenade makes no sound; whether a rolling one keeps firing contacts is not known.
  - blast: `f1.composite_sound`, audible to 200 m, after the blast.
- `ttl_ms` 4000 from the press, whichever key began the throw; `throw_power` 17; `splash_radius` 10; damage 1.3 at the
  centre to 0.05 at the edge; `splash_pierce` 0.15. PROVEN.

Not determined: the class behind `[grenade+0x224]` (so the client's real hearing rule), the mesh size and draw distance,
the time of each throw sound inside the animation, whether remote players see a third-person throw.

## The server

The match hands over no object states: the worlds' objects section is 5 s apart and `grenade_core::serialize` (0x303DA0)
writes a body only while one exists, and the recordings' worlds show none. The one grenade fact a place gives is its
input: the throw key (`key::GRENADE`, the first pocket that holds a grenade with units left, PROVEN 0x2FE8B0) or a
quick-use key whose slot holds one (`loadouts::QUICK_USE_ORDER`). `live_grenades.rs` turns that edge into an entry (thrower,
team, press time, blast = press + 4000 ms), and at the launch the arc model of `arc.rs` over the bullet mesh gives the
rest point. A human's pockets are its profile's first kit (LIKELY the kit it spawned in; a picked kit is not tracked), a
server bot's throw key is a frag.

## Do players leave the radius? (live 12+ player matches, `grenades-record/avoid-design.md` 2)

No. Of 4,338 frag throws with a player inside 10 m at the landing, the thrower's mates left the radius by the blast as often
as the same point's players 20 s before or after (22 % against 25 %) and the enemies as often (20 % against 20 %); the
outward-speed curves of the real landings and the controls lie within 3-5 points at every time and facing. 78-80 % are
still inside at the blast. Method and limits (positions dead-reckoned from the relay's move keys, a median error of 1.1 m
over 5 s) are in the record.
