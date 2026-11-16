# Footsteps: lost to re-simulation (patch 101), silent ground (patch 153)

The players' B20 (backlog 2026-10-21): "When walking, the sound randomly disappears sometimes".
Later builds: 0.51c5 "Fixed a bug which caused faulty player footstep sound playback (in some
situations). Now the sound is consistent." The code below is 0.27d0's, the nearest later build we
have; the rule it removed is the one this patch relaxes.

## What 0.26g does (PROVEN, the original exe)

- A step is an animation event of the body's `player_sound_events` channel
  (crouch_steps.md): `player_equipment_sound_effect::on_player_sound_event` (0x2A6470) is called
  with the event; domain 0 and 1 are a step of one foot or the other (`on_foot_step` 0x2A6520 with
  the foot's position, effect +0h / +18h), 2..4 three rustles.
- Before the switch (0x2A6475..0x2A6493): if the player has a world core (+120h), an event whose
  time (+10h) is **at or before the core's `maximum_time` (+18FC4h)** is dropped. `maximum_time` is
  the newest tick simulated once (set to a history item's time after its move, 0x2D0FCB; 50 by the
  clear, 0x2D0776). The same "first simulation only" test is at nine other places (0x23EA20..
  0x23F634, the time rule 0x267AD2, 0x29618C, 0x297BDC).
- The body animates in the simulation, so a re-simulated tick fires its steps again, and the test
  keeps them from playing twice. A game re-simulates whenever another player's input reaches it
  after its tick (every relayed input that is late: match.md section 5.1) and at a resync. A step
  that **only the re-simulation has** (the prediction kept the other player standing or on his old
  course; the real input moved his gait) falls at a played time and is never heard. So another
  player's steps drop out when he starts, stops or changes pace under lag, and come back once
  his ticks are first simulated again.

## What 0.27d0 does (PROVEN, the BCJ-decoded exe)

- `player_equipment_sound_effect` is a `drawable_object` (RTTI 0xB15F1C, vtable 0xA364A4): its
  event handler (0x29A980) no longer plays anything and has no time test. A step stores the
  event's time per foot (+54h / +58h) and the movement state (+5Ch / +60h); the rustles set flags.
- Its per-frame update (vtable slot 0, 0x29AFA0 -> 0x29AEF0) plays a foot's step when its stored
  time is newer than the one at the last update (+68h / +6Ch, 0x29AF5B / 0x29AF74), then keeps the
  times. A step a re-simulation makes plays (a frame later); one it only repeats does not.
- None of the ten `maximum_time` tests is left in 0.27d0 (its `maximum_time` is +1906Ch; no compare
  with it). 0.69d records the step times the same way (on_player_sound_event 0x2002A2AA0 stores
  them at +7B8h..+7C8h).

## The patch

One lohook at the test (`player_sound.step_guard`, 0x2A6485, 14 bytes), Rust `footsteps`:

- a rustle, another object's event, feet not read: the game's own test, unchanged;
- a step past `maximum_time`: plays, as in the original;
- a step at or before it (a re-simulation): plays when it is less than 250 ms before
  `maximum_time` and the same foot of the same effect played nothing in the 250 ms before it;
  otherwise it is the step already played, its time moved by the correction, or an old step of a
  long re-simulated window (a resync), and is dropped. A foot steps once a cycle (600 ms at the
  quickest, the sprint clips), so 250 ms keeps every real step and drops the copies, and a
  window of any length plays at most each foot's newest step, never a burst in one frame.
- The newest played time per foot is kept for 128 effects (three player objects a player); a
  new effect takes a free slot, else the oldest. An effect not remembered plays its step.
- Never fewer steps than the original: every step the original plays still plays.

Nothing a match computes: a step is a sound, a decal and particles, and differs between games
already (first-person set for oneself). No simulation_changes.tsv row.

## In-game check

Two clients, one with 150-250 ms of added latency (or a real far player). From the other client,
watch and listen to him: walk/stop/walk in short bursts and change between walking and sprinting
near the listener, on concrete. Before the patch some first steps after each change are silent;
after it every step is heard, none doubled. The listener's own steps are unchanged.

## Silent ground (patch 153 and the edit list step-sounds-1)

The owner, 2026-11-16: "even right now when walking on rails, sometimes no sound is heard at
all", a proper fix, checked against 0.69. The research (the bytes of 0.26g and the 0.69d listing,
every map's collision measured): `private/ladder-work/step-sounds-research.md`; the census
scripts and their outputs: `private/ladder-work/step-sounds-build/census/`.

### Every path to a silent step (PROVEN, the original exe)

The chain: the held item's `player_equipment_sound_effect` (the item's +884h, not the player's:
`weapon_controller_client::set_user` 0x2985D0 hands it the item's user at 0x2985FC) gets the
body's step events (`on_player_sound_event` 0x2A6470); `on_foot_step` 0x2A6520 takes the foot
material, the player's u16 at +0F2C4h + 4 x movement id + 2 x third person (the boots' eight
materials from +0F2C8h), and calls `step_manager::on_step` 0x248AD0 (ECX the manager, the game
world's +4D5Ch, `get_step_manager` 0x1F63E0; EAX the foot, kept by `set_feet` 0x2A6410 from the
toe bones; stack: direction, foot material, view flag; ret 0Ch). The landing calls it too
(0x23FBE0 from `player::on_landing`: the run material, or the jump's past the hard threshold).
`on_step` casts one ray from 1 m above the foot straight down, 2 m (group 30h, mask 8: the
collision bullets hit), and plays the pair (foot material, the hit triangle's material) at the hit
point. A step is silent when:

1. its time is at or before the core's `maximum_time` (a re-simulation): patch 101, above;
2. the event is another object's, or the feet were not read (+44h 0);
3. the world has no step manager (no match);
4. **the ray meets nothing** (0x248B60): no hittable surface from 1 m above to 1 m below the foot.
   The walker collision (cgroup 2, cmask 4: what the body stands on, which bullets go through) is
   never met, so a walker-only floor whose hittable surface is more than 1 m under it is silent: a
   hopper wagon's top, a loading station, a wall top, School's first part. Fixed by patch 153;
5. **the pair has no step sound** (0x248BA5: pair +20h 0): 33 pairs of the six foot materials
   with a ground one stands on (below). Fixed by step-sounds-1;
6. no pair (0x248B92): a foot material that is no pair master. That is the on-site logic states
   (`stand_on_site`, `crouch_on_site`, `dead`: movement id 0), for which `on_foot_step` reads the
   word before the table (player +0F2C4h). Whether a step event ever comes in such a state is NOT
   VERIFIED: the step log shows it (`movement 0` and its foot material); not patched blind;
7. past `on_step`: emitted and not heard (no free voice, the cull under fire: `sound_voices.md`,
   patch 96; the quiet samples go first: thick metal's own step has `volume_k` 0.06).

The rails themselves are not silent (the research's section 3): their ray always meets metal,
asphalt, concrete or ground. What is silent "on rails" is what stands on and by them: Cologne's ICE
cars (their collision faces are `foliage_dry`, path 5), the Chemical Plant's hopper wagons and
loading stations (walker-only tops, path 4), one's own sprint and landing on the mud beside them
(path 5).

### What 0.69d does (PROVEN, `private/music-research/retail-069d.asm`)

The same ray (group 30h, mask 8, 1 m up, 2 m long), but `on_step` 0x2001FAC50 first takes the
material the character controller stands on (0x2001FAC9F..0x2001FACAA: the player's controller
+612Eh, which its `step_down` writes from its sweep's hit beside the supporting normal, 0FFFFh when
the sweep met nothing); a ray hit replaces it (0x2001FAD70), a miss keeps it (0x2001FAD79), and the
step is silent only when neither has one (0x2001FAD7D). The sound is placed at the foot, not at
the hit. 0.26g's controller keeps the normal alone (+2C710h), no material.

### Patch 153: the ground of a step whose ray met nothing

One lohook after the ray (`step.ground` 0x248B5A, `mov ecx,[esp+4Ch]` / `test ecx,ecx`), the Rust
patch `step_ground` over `vostok/step.rs`. A ray that met something: the game's own path, byte for
byte. A miss asks 0.69d's question with the same `ray_test` (0x75E20), into a result of its own:

1. **What the body stands on**: the ray again with the walker collision too (group 34h, mask 0Ah),
   as far as the game's (1 m under the foot). Its face's material, when the face was given one
   (a wagon's walker top is metall_thick, a wall's brick).
2. **A walker face without a material** (0FFFFh at the cook, 3FFFh after the mask; most walker
   faces: `walker_col_s`, `walker_coll`, the editor's boxes): the hittable surface under it, the
   game's filter, to 4 m under the foot (a station's metal floor, the School's concrete). None
   there: the face's own (3FFFh), which `material_pair` steps as `default`, the dirt step.
3. **Nothing under the foot within 1 m**: the body is in the air; the game's silent return.

A found ground goes on at the exit past the hit object's material (0x248B77, EAX the material),
the result's point the foot (0.69d's place: the sound is not put 4 m under the player) and its
normal the hit's. The order is the data's: under a walker face with a material, the hittable
surface below is another material on 42.8 m² of the Chemical Plant (the wagons' metal tops over
ground and grass) and 56.1 m² of the Fort (brick wall tops over grass and gravel); the face's is
the right one.

What each look gives, on the standing room where on_step's ray meets nothing (m², the census
`steps.py` of each map's `day_4` project, the body's centre, the foot 0.1 m above the walker face;
"miss" includes wall tops and border boxes no one stands on):

| map | miss | walker face's material | the surface under it (to 4 m) | default |
|---|---:|---:|---:|---:|
| Rudnya level_01 | 1,657 | 633 (ground: terrain's walker) | 81 | 943 |
| School level_02 | 1,278 | 12 | 224 | 1,042 (school_basement 778) |
| Radar level_03 | 2,210 | 61 | 86 | 2,063 (fences, bunker) |
| Chemical Plant level_04 | 6,134 | 87 (wagons' metal) | 407 (stations' metal 128, concrete, ground) | 5,640 (central_factory 3,386, level_border 1,780) |
| Fort level_05 | 2,407 | 501 (brick walls) | 375 (brick, stone, gravel) | 1,531 |
| Mamayev Kurgan level_06 | 318 | 9 | 113 | 195 |
| Vector level_07 | 60 | 16 | 28 | 16 |
| Cologne level_08 | 35,598 | 41 | 551 | 35,007 (the border's boxes) |

In this model every standing place is on a walker face, so the walker look finds ground for all
of it; 2.7 % gets its face's material, 3.8 % the surface under it, the rest `default`. The ported
STALKER maps have no walker-only collision (every triangle is 10/20): nothing changes there.
Under the walker-only room the hittable surface is 1 to 4 m down on 2,180 m², 4 to 10 m on
1,378 m²; the look stops at 4 m, a storey (deeper is another floor's or the ground's material).

**Not simulation (PROVEN in the code).** `on_step` writes its manager's decal counter (+0, only for
a pair with a decal, which no foot pair has) and nothing else of its own; it makes a sound, a decal
and particles. The hook reads the physics world (`ray_test` is a query; on_step calls it itself),
calls `get_triangle_material` (a read of the shape's table), writes `on_step`'s own result (a local
of its frame) and EAX, and keeps its own state (the log switch, the armed ground) in the arena. The
steps already differ between the games of a match (the foot material by view, the feet from each
game's drawn bones, patch 101). No `simulation_changes.tsv` row.

`check_step_ground_026g.py` runs `on_step` whole in both exes over made-up ground (the rays,
materials, pair and sound stubbed and recorded): a hit is the original's calls and memory; a
walker floor, an unnamed one over metal, an unnamed one alone and the air as above, own and third
person; the log off reads the switch once and writes nothing, on writes each line.

### step-sounds-1: the empty pairs

`game_materials/material.pairs` (patch 27's, the live one) had 45 empty step sounds among the six
foot materials (actor 10 and actor_sprint 53: what others hear; actor_local 42 and
actor_local_sprint 56: the player himself; actor_crouch 54 and actor_local_crouch 57: crouched).
Twelve stay empty on purpose: `restrictor` 37 (a limiter's faces) and `actor` 10 (another
player). The other 33 play the stock sound of the nearest material's pair of the same foot
(`scripts/content-edit/step-sounds-1.json`; the owner decides how things sound, a retune is a
pair's `to`):

| ground | foot | sound (type: 258 a collection, 257 a composite) |
|---|---|---|
| foliage_dry 49 | 10 / 42 / 53 / 54 / 56 / 57 | `material/foliage/step/foliage_step_m` / `foliage_step` / `foliage_step_sprint_m` / `crouch/material/foliage/step/foliage_step_m` / `material/foliage/step/foliage_step` / `crouch/material/foliage/step/foliage_step` (258) |
| mud 23 | 56 | `material/mud/step/mud_steps` (258, its own walking) |
| foliage_coniferous 24 | 56 | `material/foliage/step/foliage_step` (258) |
| foliage_deciduous 44 | 56 | `material/foliage_2/step/foliage_2_step` (258) |
| wood_tree 43 | 10, 54 | `material/wood/step/wood_steps_m`, `crouch/material/wood/step/wood_steps_m` (258) |
| metal_arm_gran_pass 40, metal_arm_gran_not_path 41 | 10 / 42 / 53 / 54 / 56 / 57 | rabitz's: `material/rabitz/step/rabitz_m` (258) / `step/rabitz` (257) / `material/rabitz/step/rabitz_sprint_m` (258) / `crouch/material/rabitz/step/rabitz_m` (258) / `step/rabitz_sprint` (257) / `crouch/step/rabitz` (257) |
| glass_car 47 | 10 / 42 / 53 / 54 / 56 / 57 | glass's: `material/glass/step/glass_steps_m` (258) / `step/default` (257) / `material/default/step/default_steps_sprint_m` (258) / `crouch/material/glass/step/glass_steps_m` (258) / `step/default_sprint` (257) / `crouch/step/default` (257) |
| metal_gas_tank 48 | 53, 56 | `material/metall/step/metall_thin_sprint_m` (258), `step/metal_thin_sprint` (257) |
| metal_car 46 | 53 | `material/car_metal/step/car_metal_sprint` (258) |
| body 15 | 53 | `material/default/step/default_steps_sprint_m` (258) |

`foliage_dry`'s faces are leaves (`coll_leaves_shurshur_1` of the ICE cars, `coll_foliage` of
Mamayev's ground, `foliage` of Vector's lab): the foliage set; `foliage_2` (the bushes') is the
other choice. No own sprint sample exists for mud and foliage: their own walking sound. Every sound
named is in the stock db or crouch-steps-1's copies. A content change only: srp reads none of it.

### A step another patch plays (for the ladders)

`vostok::step::play(player, ground, at, normal)`: a step of the player's walking foot material
(movement 2, first or third person as `is_first_view` 0x1F3BD0 says) on `ground` at `at`, through
the game's own `on_step` (`engine_on_step`, the manager by `engine_step_manager`, the world's
vtable +34h): it arms the ground for that one call, patch 153's hook takes it in place of the ray
(`Ground::Armed`), and the pair plays as a foot's: on the hud for the player in first person, a
point sound at `at` for everyone else. A rung's material: metall_thin 17 or wood_thin 35 have all
six foot sounds. A step made in the simulation's tick comes again in each re-simulated tick:
`footsteps::decide(thread, Step::new(StepSource::player(player), foot, time, maximum_time))` is
patch 101's rule for it (its source the player, so it never meets the feet's; its two `Foot`s two
limbs). Nothing calls `play` yet.

### The step log (`SKHID_STEP_LOG`)

A diagnostic, never players': a game started with `SKHID_STEP_LOG=on` (or `1`; read once, at the
first step) writes a line a step to the run's patch log. Off, the hook reads one cell more.
Two lines as the check writes them:

```
srp: step: own foot, movement 2, first person, foot 42 at 1000 550 -300 cm; game missed; walker met 16383 at 25 cm; below met 18 at 150 cm; ground 18 (below): plays
srp: step: own foot, movement 2, first person, foot 42 at 1000 550 -300 cm; game missed; walker missed; nothing under the foot: silent
```

Whose (own: the local player's), the caller (`foot, movement <id>` from `on_foot_step`'s logic
state, `landing, movement 2|4`, `a patch's step`), the view, the foot material (an odd value with
movement 0 is path 6), the foot in centimetres (the game's frame), each ray (`game`, `walker`,
`below`: the triangle's material id and how far under the foot, or `missed`), the ground and how
it was found (`game`, `walker`, `below`, `walker, no material`, `armed`), and what its pair plays
(`plays`, `silent, the pair has no sound`, `silent, no pair`), or `nothing under the foot:
silent`. Material ids are game.materials' (`step-sounds-research.md` 2.1; 16383 is a face without
a material). A played step can still be culled (path 7): the log says what `on_step` asks for.

### In-game check (not done)

Nothing of patch 153 or step-sounds-1 has been heard. With the log on: the Chemical Plant's
hopper wagons and loading stations, the Fort's wall tops, Cologne's ICE cars (their foliage step),
sprinting on mud, others walking on stumps: each step heard and its line `plays`. And the rails
of the owner's report, to name what was silent there.
