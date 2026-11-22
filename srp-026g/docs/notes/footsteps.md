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
   word before the table (player +0F2C4h). VERIFIED by a crash (a player's rev71 dump, Freeze Tag
   on School, his own step in first person): the word was a pointer's low half, 0x6860, and
   `material_pair` read `[materials + 0x6860*4 + 108h]`, far past its tables, and handed `on_step`
   garbage (the write at 0x248BB6). Fixed by patch 101: `on_foot_step`'s read (0x2A6545,
   `player_sound.foot_material`, past both paths in, with a world core or not) ends a foot step in
   a state with no foot material (movement 0, or an id past 4) silently; the patch log says so at
   the 1st, 10th, 100th, .. of a run, the step log's event line says `dropped (no foot material)`;
   and patch 193 takes any id past the 128 materials as the default in `material_pair` itself
   (below, "Material ids past the table");
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
byte. A miss steps as 0.69d does (below, "The contact"): on what the player's character controller
touched at its last step-down sweep; a touched face without a material (3FFFh: most walker faces,
`walker_col_s`, `coll_restrictSG`, the editor's boxes) takes the named hittable surface at its
height near the touch (the game's filter, from 10 cm over the touch to 30 cm under it, under it and
then on rings every 5 cm to 30 cm across: its model's own collision, the walkway's beam), else its
own, which `material_pair` steps as `default`; nothing touched (the sweep met nothing: in the air):
the game's silent return.

A found ground goes on at the exit past the hit object's material (0x248B77, EAX the material),
the result's point the foot (0.69d's place: the sound is not put 4 m under the player) and its
normal the hit's. The order is the data's: under a walker face with a material, the hittable
surface below is another material on 42.8 m² of the Chemical Plant (the wagons' metal tops over
ground and grass) and 56.1 m² of the Fort (brick wall tops over grass and gravel); the face's is
the right one.

What the first rule's looks (#2128's walker look, then the surface under it to 4 m) gave, on the
standing room where on_step's ray meets nothing (m², the census
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
person; the log off reads the switch once and writes nothing, on writes each line. The contact's
choices are the crate's tests (`step_ground::tests`); the check's made-up player has no
controller, so its contact looks meet nothing.

### The contact: what the capsule rests on (PROVEN by two walks)

The owner's report ("rails are not heard at all"), walked with a diagnostic build and the log on
(Chemical Plant, FFA `level_04/evening_8`, 2026-11-16: rails, tracks, gravel, a wagon top and the
loading station): 53 of 153 steps silent, every one `game missed; walker missed`, the body on the
ground (`on_ground`, support normal y 0.88 to 0.95), the foot 10 to 25 cm from the root.

- **Where:** 50 of the 53 on `level/04/loading_station_01.model`, the walkway along its top at
  y 11.19 m (x 19.2 to 19.8, z 20 to 90). Its walker collision there is a strip **32 cm wide**
  (x 19.314..19.635, 2/4, `walker_col_s`, no material); its bullet collision an **8 cm metal beam**
  (x 19.33..19.41, 8/16, metall_thick) at the same height; nothing else within 9 m under it. The
  capsule rests on the strip, its centre up to 12 cm beyond the strip's edge; a foot 10 to 25 cm
  aside casts its vertical ray beside the strip and the beam: nothing within 1 m, either filter.
  A thin floor, as the rails of the report: no shape skipped, no one-sided mesh, no group missing.
  The other 22 steps there met the strip under the foot (16383) and went 1 to 2.6 m down to the
  ground or the station's frame: the wrong sound on a metal walkway.
- **0.26g keeps no contact:** the controller's `step_down` sweep (0x7EA80 -> 0x84FE0) hands back
  the point, normal and fraction, no object or triangle; the support normal (+2C710h) alone stays.
  0.69d keeps the material (+612Eh, its `step_down` 0x2005F408C) and steps on it on a miss.
- **The first rule (#2128, walk 2's build):** looks around the body's root, 50 rays on three
  rings. Replaced by 0.69d's way below; the review after its merge found a handrail or a crate top
  within 1 m over the feet could be taken for the support, and a foot event in the air near a ledge
  could step.
- **0.69d's way (this rule):** the step-down sweep's callback,
  `character_step_down_sweep_test_callback::addSingleResult` (0x84420, vtable 0x9E3F34, made at
  0x842E0 with the controller's filter, group 4 / mask 2), sees each hit's LocalConvexResult: its
  collision object (+0; its +0F8h the game's object, whose get_triangle_material 0x3570A0 gives the
  material), its shape info (+4: the triangle +4, the primitive flag +8) and point (+20h, Bullet
  frame). Hooks note the hit it takes as its first (`step_down.first` 0x846A1) and its second
  (`step_down.second` 0x845A8); the sweep (0x84FE0) answers with the second when it is taken and
  not further than the first, else the first when one was taken (0x85212..0x85269), and
  `step_down.result` (0x852EF) keeps that hit's material and point for the controller (keyed by its
  helper, controller +210h), or nothing; `step_down.cached` (0x85083) clears it on a cached miss
  (a cached hit keeps it). A step whose ray met nothing steps on it: a named one as it is, an
  unnamed one by the look under the touch. 0.69d's own order (its controller's material, the ray's
  hit replacing it, silent with neither). **Read only:** the hooks read the callback, the hit and
  the shape's material table; the sweep, the callback and the controller are as the game leaves
  them (`check_step_ground_026g.py` runs each site in both exes and compares the callback, its
  frame, the controller and the registers). No `simulation_changes.tsv` row.
- **The review's findings are gone by construction:** a handrail or a crate top beside the feet is
  not what the capsule's sweep touches under it, and in the air the sweep touches nothing, so a
  foot event there is silent as before.
- **Walk 3** (the owner, 2026-11-16, Chemical Plant, this rule with the look under the touch to 4 m):
  "sometimes when walking on rail I hear sound of ground which is 5 meters at least below me", "not
  just metal". Of 72 log lines (each step is logged twice) on an unnamed touch whose look met something, it met
  ground_walkable 1.3 to 2.65 m down (16, the ground under the walkway and its stairs), metall_thick
  54 to 102 cm down (26: the stairs and the floor under the walkway), and metall_thick 0 to 8 cm down
  (30: the walkway's own beam); 40 met nothing named (`default`).
- **What the owner stood on (walk 3's 56 distinct unnamed touches):** two walker meshes, both
  without a material in 0.26g and 0.69d alike (0.69d's foot ray misses at 55 of the 56 steps too:
  silent there):
  - `loading_station_01.model/walker_collision` (face `coll_restrictSG`; 47 touches): the loading
    station's walkway (x 19.2..19.4, y 11.19, z 27..89) and its stairs down to the east (x 20..35,
    y 11.2 to 4.0). Its own `collision` (metall_thick 6,778 faces, metall_thin 667) lies 3 cm over
    to 25 cm under the strip, within 26 cm across.
  - `admin_building.model/walker_collision` (face `walker_col_s`; 9 touches): the admin building's
    stair (x 57.7..59.5, y 3.1..4.7, z 85..90), over its metall_thick steps within 10 cm.
  No other unnamed surface was touched where the ray missed; every other step was the game's ray
  (log lines, each step twice: ground_walkable 111, metall_thick 49, metall_tank 38, mud 16, ground 15).
- **This rule (after walk 3):** the named surface near the touch, from 10 cm over it to 30 cm
  under it (the strip's own metal: 3 cm over to 25 cm under; the next surfaces under the walkway
  54 cm and more), under the touch and then on rings of 5 to 30 cm across in eight directions, the
  nearest ring that meets one, there the hit nearest the touch's height. Over walk 3's 56
  touches (these looks over the level's collision, offline): metall_thick 52 (the loading
  station's 43, the admin building's 9: 17 under the touch, 35 beside it), metall_thin 3 (the
  walkway's cross members), `default` 1 (a touch our data puts 1.8 m off the strip). Nothing far
  under the walkway plays. The touched model is not checked (the hit objects carry no model): the
  nearest surface at the touch's height stands for it, which walk 3's data bears out (the nearest
  named game face is the touched model's own at every touch). 0.69d is silent at these spots; here
  they play their models' metal. Not yet walked with this build.
- **Walk 2** (the same walk with it): 5 of 127 steps silent, all in the air (falling 2.5 to
  7.3 m/s, jumps off the station); 62 found their ground by the contact: metall_thick 58,
  metall_thin 2, rabitz 1, 16383 (`default`) 1. The owner: "now it works".
- **Each step plays twice (stock, not changed here).** In both walks every foot event reaches
  `on_player_sound_event` twice in the same frame with the same time and the same effect (walk 2:
  one effect, 0x7C2F1B34, for all 616 events; 120 of the 122 played foot events came twice, both
  past `maximum_time`), so `on_step` runs twice and the step is two sounds at once (+6 dB, two
  voices). The original's guard passes both, patch 101's too. Patch 101's rule could drop a copy
  of the same foot and time; not done: it changes how loud every step is, the owner's ears first.

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
| foliage_dry 49 | 10 / 42 / 53 / 54 / 56 / 57 | `material/default/step/default_steps_m` (258) / `step/default` (257) / `material/default/step/default_steps_sprint_m` (258) / `crouch/material/default/step/default_steps_m` (258) / `step/default_sprint` (257) / `crouch/step/default` (257): the default pairs' own |
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
Mamayev's ground, `foliage` of Vector's lab), but it steps as `default`, the plain dirt step, as
0.69d plays it: the bush rustle was 9 to 13 dB louder than 0.69d's step on Mamayev Kurgan's
ground. No own sprint sample exists for mud and foliage: their own walking sound. Every sound
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
Lines of walk 2 (the event at patch 101's guard, before its time test; the step at `step.ground`;
walk 2's build had no water look):

```
srp: step event: own, effect 0x7c2f1b34, domain 1 (right foot), time 38557 (max 38553), movement 3, feet read: plays
srp: step: own foot, movement 3, first person, foot 56 at 5910 347 8984 cm; root 5921 333 8978 cm, foot-root -11 13 5 cm; support y x100 84, velocity y 0 cm/s, on_ground; game missed; contact top met 16383 at -21 cm; named met 18 at 0 cm (16 of 50 looks); ground 18 (contact): plays
```

The event line: whose, the effect, the domain (0 and 1 the feet, 2..4 the rustles), the event's
time and the core's `maximum_time`, the logic state's movement id, whether the feet were read, and
what the guard did (`plays`, `skipped (a re-simulated copy)`, `the game's test`), or `dropped (no
foot material)` for a foot step that `on_foot_step` then ends (path 6). The step line's
body: the root (the player's simulated feet), the foot's offset from it, the controller's support
normal's y (x100), its vertical velocity and `on_ground` (0x7EF90's test).

Whose (own: the local player's), the caller (`foot, movement <id>` from `on_foot_step`'s logic
state, `landing, movement 2|4`, `a patch's step`), the view, the foot material (an odd value with
movement 0 is path 6), the foot in centimetres (the game's frame), each ray (`game`, `water`,
`walker`, `below`, `contact top` and `named`: the triangle's material id and how far under the
foot (the contact's: under the root), negative over it, or `missed`), the ground and how it was
found (`game`, `wading`, `walker`, `below`, `walker, no material`, `contact`, `armed`), and what
its pair plays
(`plays`, `silent, the pair has no sound`, `silent, no pair`), or `nothing under the foot:
silent`. Material ids are game.materials' (`step-sounds-research.md` 2.1; 16383 is a face without
a material). A played step can still be culled (path 7): the log says what `on_step` asks for.

### In-game check

Played by the owner, 2026-11-16, on a diagnostic build of this rule with the log on: the Chemical
Plant's rails, tracks, gravel, a wagon top and the loading station's walkway (walk 2 above). Not
yet heard: the Fort's wall tops, Cologne's ICE cars, sprinting on mud, others walking on stumps.

## Wading (patch 153 and the pack water-steps)

The owner, 2026-11-16: walking in the water in the middle of Rudnya plays no wading sound in
0.26g; in 0.69d it does. "I want them there."

### Why 0.69d wades there (PROVEN on the data)

- 0.69d decides a step's ground as described above (`on_step` 0x2001FAC50): the controller's
  ground material (+612Eh) first, replaced by what the ray (group 30h, mask 8) meets
  (0x2001FAD70, the triangle's material). The pond's water itself has no collision in 0.69d:
  `level/01/terrain/terrain_water_01..04` are `collision_enabled false` (only `_02` has a mesh,
  deep_water, and nothing places it: the day_tdm project has no collision object of them).
- The ray meets the pond's **bed**: the `collision` of `terrain_game_center_1..4`, whose shading
  group `dirt_01_s` is game material `deep_water` (32). On a 0.5 m grid over the pond, where the
  water is 0.3 m deep or more the ray's first hit is deep_water at nearly every point; in the band
  less than 0.2 m deep it is the bank's ground (9). So 0.69d wades by the bed's material, not by
  the water.
- Our port (`v069d_level_01`, maps 204, 216-218) has those meshes byte for byte (collision
  vertices, indices, face_data and the settings' deep_water), placed 8/16: **by the data it wades
  already** (5,681 of 7,042 wet grid points meet deep_water, 1,174 the bank's ground). Not heard in a
  game.
- 0.26g's own Rudnya (`level_01`, its Team Deathmatch, the other modes and Free for All in every
  weather) draws the same pond (`level/01/terrain_water_01`, no collision, its settings' material
  "not assigned"); its bed is **mud** (23): the step is mud's. That is the Rudnya without wading.

### The rule

A step whose foot is under drawn water wades, on every map, from the map's own data:

- **The water** (`scripts/water-steps/make_pack.py`, the pack `water-steps@1`, the edit list
  `water-steps-1.json`): every project that draws a surface of a water material
  (`material_instances/water/*`, `ported/<build>/water/*`, its model's LOD0 surfaces) gets one
  model, `models/srp/water/<level>.model`: the triangles that water draws, placed as the project
  draws them (scale, then position), those within 60 degrees of level, each once; a `collision`
  mesh alone of game material deep_water, placed at the origin as a collision object of **cgroup
  and cmask 1000h**. 123 projects of 12 levels; a level's projects share one model.
- **Nothing else meets it.** No object of the live data or the code has bit 1000h in its group or
  mask (ragdoll.md section 19's table, stalker-maps.md's list of every ray): bullets, blasts,
  cameras, grenades, batteries, walkers and their foot placing, the use ray, traps, dead bodies
  and a spot's ray (cgroup & 80Ah) pass as before, and the physics world's box (groups & 0Ah,
  `bullet_physics_world::add` 0x758B0, `bullet_bounds.md`) is the same. The water does not stop a
  shot at legs under the surface. vostok::step's test lists the filters.
- **The look** (`Look::Water`, `vostok/step.rs`): every step's first, after the game's ray, with
  the same `ray_test`, origin and reach (1 m over the foot to 1 m under it), group and mask 1000h.
- **The decision** (`step_ground.rs`): water at least 0.2 m (`WADE`) over what `on_step`'s ray met
  (over the foot, when it met nothing) is the step's ground: deep_water, its pair's sound and
  particles at the water's surface over the foot. Less (a film over the ground, a pond's edge,
  where 0.69d's own bed is ground too), water under a plank, and a stock water plane `on_step`
  meets itself (Chemical Plant, Fort: its own `water` splash) are the game's step, as before.
  Water deeper than 1 m over the foot is not met (the ray starts under it): the bed's step.
- The crouch clips of 0.69d (`deep_water_crouch*`) are not ported: patch 27's crouch pairs for
  deep_water play its walk clips.

What it changes, on a 0.5 m grid over each map's water (its day_4 project; a standing place is the
highest walker floor from 1 m under the water to its surface, the foot 0.1 m over it; the m² that
wade and what the game's step played there):

| map | water drawn | wades | was |
|---|---:|---:|---|
| Rudnya (`level_01`) | 2,293 | 1,573 | mud 1,390, ground 171, metal and other props 11 |
| Rudnya v0.69 (`v069d_level_01`) | 2,293 | 1,570 | deep_water 1,387 (already wading), ground 171, metal and other props 11 |
| Chemical Plant (`level_04`) | 1,264 | 329 | gravel 309, concrete 12, metal 8 |
| Mamayev Kurgan v0.21 (`v021d_level_06`) | 648 | 12 | gravel 11 |
| Radar (`level_03`, v0.11, v0.21), Fort (`level_05`), Mamayev Kurgan (`level_06`), Vector (`level_07`), Cologne (`level_08`), Chemical Plant v0.11 | 330, 330, 330, 588, 560, 80, 116,671, 2,042 | 0 | |

Where nothing changes the drawn water is the game's ground already (the Fort's and Chemical
Plant's stock planes, `water`), less than 0.2 m over it, or under it (Cologne's swamp plane,
116,671 m² at y 14.74, lies under the ground but where it shows); the "water drawn" not counted
in either has no floor within 1 m under it (deep or out of reach). Vector's cellar water
(`base_v_cellar_perehod`, 79 m²) is within 0.2 m of its floor or under it.

**Not simulation (PROVEN in the code and the data).** The models are collision only the step's
water look meets (above); the look is a query of the physics world, its result ours. The step's
ground is presentation (the section on patch 153 above). No `simulation_changes.tsv` row.

`check_step_ground_026g.py` runs `on_step` whole in both exes: the original's ray passes through
the water (1000h); the patched exe's water look comes after the game's ray on every step, and 0.75
m of water over the bed wades (deep_water at the surface, first and third person), 0.125 m over
the ground or water under a plank is the game's step, no water is the game's step after one more
ray.

### In-game check (not done)

Not heard. With `SKHID_STEP_LOG=on`: walking, sprinting and crouching through native Rudnya's pond
(lines `ground 32 (wading): plays`), its edge (`ground 23 (game)`), the port's pond (`game` or
`wading`, deep_water both), the Chemical Plant's shallow pools. And whether the wading sound is
what the owner heard in 0.69d.

## Material ids past the table (patch 193)

`game_material_manager::material_pair` (0x306D50) takes two u16 ids and indexes the manager's 128
material slots (+108h) and its 128 x 128 pair slots (+308h) with no bound (the manager is 10310h
bytes, 0x2CD1F2; an empty slot gives the default id 11, +1030Ch). Patch 193 (`material_pair.entry`
at its start) takes an id past the table as the default, what it gives an empty slot, by the
game's own call with the bounded ids, and answers 0 (no pair) if the default is past it too; every
caller tests for 0. 3FFFh (a face without a material) is let through: its read is pair slot
[127][127], inside the manager and empty. The patch log says `material_bounds: material id ..`
at the 1st, 10th, 100th, .. of a run.

Where the ids come from (the 2026-11-22 audit after the rev71 crash):

| Site | Ids | In range |
|---|---|---|
| on_step 0x248B89 | the foot material (on_foot_step's table by movement, the landing's run or jump material, patch 153's own step: the run materials), the ground's get_triangle_material (0x3570A0: & 3FFFh, or 0FFFFh for a shape without a table) | movement 0: no (0x6860 own, 0A9h others', the dump; patch 101). 0FFFFh: no, read 40104h past the manager (code; whether a live shape has no table: not verified) |
| grenade 0x248A6F, 0x2487F3 | its material (+230h), the ground it hit (+250h, get_triangle_material at 0x248A3F) | 0FFFFh as above: not verified |
| booby trap 0x2481A1 | +2BAh, +2B8h | not verified |
| bullets 0x32E599, 0x32E7F3, 0x32EA19, 0x32EC98 | the bullet's material and the surface's (+48h, +4Ch: a loaded material, the default for 0FFFFh, 0x32D87B) | yes |
| patches: vostok/step.rs `pair`, vostok/bullet.rs | the foot and ground as on_step's; a bounded `MaterialId` and the actor's 10 | as on_step's; yes |
| inline lookups of a slot by a triangle's id (0x3065F1, 0x32D866) | 0FFFFh taken as the default material first | memory-safe: 3FFFh at most, inside the manager (the others reading +10308h, 0x2CD4D9 0x32BBA2 0x32BE05 0x32BE5F 0x339560 0x82C7A 0x83293: not looked at) |
| a material's pair master (+66h), the pair loader 0x307010 | game.materials' data | not bounded, not patched (the game's own files) |

The movement id's other readers: the rustle 0x2A6580 takes slot id - 1 for 1..3 and its first
slot otherwise (bounded); the landing passes 2 or 4 (0x23FC00); the breath sound 0x296310 reads
fixed slots. The states are made with 0..4 only (0x2FABB0..0x2FADCC, stand_jump 4 at 0x336B87); no
patch makes or writes a logic state. on_step's callers: on_foot_step 0x2A6572, the landing
0x23FC2C, patch 153's `step::play`; on_foot_step's: 0x2A64B6 and 0x2A64D2 only (direct calls; no
other reference to either).
