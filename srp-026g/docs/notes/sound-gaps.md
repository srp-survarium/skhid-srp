# Sound gaps: what players hear wrong in 0.26g, why, and what fixes it

The owner, 2026-11-16: "next task is to figure out sound problems in the game, specifically I know
that in 0.69 sounds were done better, in 0.26 lots of surfaces don't have sounds or even worse
things like rails are thin and for some reason player there walks without making any sounds at all".

This is research only: nothing here is built, played or released. Sources, all read offline:

- 0.26g: the original exe (`private/function-map/inputs/026g/survarium.exe`, base 0x10000; its
  listing `private/music-research/client.asm`), the stock `resources.db` and every live patch
  12..61 (the newest layer of a file wins), with `step-sounds-1` (branch fix/silent-steps)
  applied on top for the step pairs.
- 0.69d: the retail exe (`private/music-research/retail-069d.exe`, its listing
  `retail-069d.asm`) and its data (`/mnt/hdd/Projects/duga-069d-026/src069/resources.*.db`).
- Configs decoded with resource-porter's `vostok-port dump-db-config --leaves`; collision placed as
  the engine places it (the census scripts of `private/ladder-work/step-sounds-build/census`).
- Loudness: each sample's peak (ffmpeg `volumedetect`) times its `volume_k`, averaged over a
  collection and summed over a composite's layers ("peak x vk", dBFS). It compares sources; what a
  listener hears also depends on the distance curve (section 4).

Marks: PROVEN (read in the code or computed from the data), LIKELY, NOT VERIFIED.

The step chain itself (on_foot_step 0x2A6520, step_manager::on_step 0x248AD0, its ray, the seven
ways a step goes silent) is in `footsteps.md` ("Silent ground") and is not repeated here.

## 0. Ranked problems

"In flight" means a branch made today, not merged: **fix/silent-steps** (patch 153 + edit list
`step-sounds-1`), **feat/wading-steps** (water collision for drawn water, over 153),
**feat/puddle-splash** (puddle decals step as water, over 153).

| # | What a player hears | Where | Cause | Fix, size | In flight |
|---|---|---|---|---|---|
| R | Steps on rails and walkways are not heard (the owner, from play) | the Chemical Plant's loading station walkway (50 of 53 silent steps), thin floors anywhere | PROVEN by the walk: the foot's vertical ray passes beside a 32 cm walker strip and its 8 cm beam the capsule stands on (1.2) | exe: patch 153's contact, looks around the body's root (walk 2: 5 of 127 silent, all in the air) | **fix/step-contact** (PR) |
| 1 | Other players' steps die out within a few metres: at 5 m a step is a quarter of its 1 m loudness, at 8 m a tenth, gone at 17 m. 0.69d: three quarters at 5 m, heard to 30 m | everywhere | the `steps` distance preset (`sounds/spl/steps.spl`) and, on top, the third-person samples 5 to 10 dB quieter than 0.69d's (4.1, 4.2) | **data**: 0.69d's `steps` curve, as `steps.spl` (also breathing and bones) or a new preset named by the 618 step samples (an edit list); optionally their `volume_k`. The owner decides: it changes how far enemies are heard | no |
| 2 | A walker-only floor steps silently: hopper-wagon tops, loading stations, wall tops, School's first part | Chemical Plant, Fort, School, Radar, Cologne, both v0.11e/0.21d ports | on_step's ray sees only bullet collision (`footsteps.md`, path 4) | exe, patch 153 | **fix/silent-steps** |
| 3 | No step at all on dry leaves, gratings, car glass; own sprint/landing silent on mud and bushes; others' walking silent on trunks | Mamayev (its ground), Vector's lab, Cologne's ICE cars, gratings everywhere, mud everywhere | 45 empty foot pairs (`footsteps.md`, path 5) | data, `step-sounds-1` | **fix/silent-steps**, but see #4 |
| 4 | (after step-sounds-1) Mamayev's ground would sound like walking through a bush at every step, 11 to 13 dB louder than 0.69d plays it | Mamayev 1,035 m² of ground, its 0.21d port 2,971 m², Vector's lab | step-sounds-1 gives `foliage_dry` the bush rustle (`material/foliage/step/foliage_step*`); 0.69d gives it the plain `default` step (3.2) | data: retune the six foliage_dry rows of step-sounds-1 to the `default` set | review note for **fix/silent-steps** |
| 5 | A bullet hitting dry leaves, a steel grating or soft grass makes no sound (dry leaves and gratings: no puff and no hole either) | Rudnya's grass (stock 6,087 m², Rudnya 0.69's terrain 18,469 m² outside its background models), Cologne's moss 2,257 m², Mamayev's ground (and its 0.21d port), Vector's lab, the loading stations' gratings (Chemical Plant 5,285 m², its 0.11e port 9,837 m²), Radar's fences | the bullet pairs of `foliage_dry` 49, `metal_arm_gran_*` 40/41, `grass_soft` 51 are empty, in 0.69d too (3.3) | data: 4 pair rows of the bullet (sound, particle, decal from ground/grass/rabitz) | no |
| 6 | No room sound: a shot or a step indoors has the same outdoor tail as in a field; no room tone or outdoor bed | every map | 0.26g has no sound environments; 0.69d picks the tail of shots and steps by the room and plays a bed per room (5.1) | **exe** (a zone system) + content (19 rooms' tails, 5 to 10 zones per map): large | no |
| 7 | Bullets passing by make no crack or whizz | everywhere | 0.69d has `bullet_flyby_sound_effect` and 16 directional pass-by collections; 0.26g has neither (5.2) | exe + content: medium | no |
| 8 | A grenade or smoke grenade bouncing off a car, a tank, glass, a wooden bridge, mud, water or a tree is silent; metal bounces are quiet | everywhere | 23 empty grenade pairs; 0.69d fills five of them and plays its bounces 2 to 8 dB louder (3.4) | data: grenade pairs (+ 0.69d's two bounce composites as a pack, or stock samples) | no |
| 9 | A rifle-butt or knife hit on concrete, brick, ground, stone, glass, asphalt makes no hit sound | everywhere | 26 empty `melee_blunt` pairs; no recording exists in either build (3.5) | data: pairs to existing samples, the owner's ears | no |
| 10 | Chemical Plant tracks: steps flip between a hollow tank, rail steel, concrete and dirt from one step to the next; a bullet into the track bed hits a tank | Chemical Plant (stock) | two overlapping collision meshes on the same tracks, `railroad.model` (every face `metall_tank`) over `railroad_01..03` (sleepers concrete, rails thick metal) (1.8) | collision material: changes bullets' piercing, so a simulation change (a `simulation_changes.tsv` row); a step-only fix is exe | no |
| 10b | Concrete rooms and monuments step and are hit as dirt | Radar's garage and arsenal (6,878 m²), Mamayev's monument, Vector's dome, London's buildings | faces with no material resolve to `default` (2) | content (models' settings), a simulation change for bullets | no |
| 11 | Others' steps on stone are 11 dB louder than on concrete or asphalt | the STALKER maps' tiled floors and roofs (1,445 to 3,677 m² walkable a map), stock stone (44 to 465 m² a map) | the stone pair plays `concrete_steps_m` at `volume_k` 0.7, asphalt's at 0.2 (4.3) | data: one pair row (or the 7 samples' `volume_k`) | no |
| 12 | Hits on bodies, others' reloads, shots and handling fade out early | everywhere | 0.69d lengthened `weapons_reloading`, `main_3rd_view`, `mid_range`, `close_range`, `weapon_shutter_close` (4.1) | data: `.spl` files, one each | no |
| 13 | Own steps quiet: 4 to 10 dB under 0.69d's on most grounds, crouched 10 to 20 dB | own first person | 0.26g's own-step composites are a quiet material layer and a cloth rustle; 0.69d has its own recordings plus a gear layer (4.2) | data: a content pack of 0.69d's step sets (837 samples, composites without the room layer) or `volume_k` | no |
| 14 | Wading through the Rudnya pond is a dry step; a puddle steps as the floor | Rudnya, every map's puddles | no water collision / puddles are decals | exe (patch 153's site) + content | **feat/wading-steps**, **feat/puddle-splash** |
| 15 | Rudnya 0.69's mid-map water and background beds missing | v069d_level_01 | the port took 0.26g's emitters (memory note rudnya-water-sounds) | content: the pack's next version | parked |

Already fixed and live: steps lost to re-simulation (patch 101, `footsteps.md`), shots and impacts
silent under heavy fire (patch 96, `sound_voices.md`), crouch steps (`crouch_steps.md`), grenade
bounce reach (`grenade_bounce.md`), ported guns' distant shots (`weapon-reverb-layers.md`).

## 1. The rails

### 1.1 Which maps have rails, what collision, what material (PROVEN, the projects' `collision_objects`)

| map | rail objects | collision | material of what a foot stands on |
|---|---|---|---|
| Chemical Plant `level_04` | `props/railroad/railroad_01..03` | bullet 8/16 + walker 2/4 | rails `metall_thick_walkable` 71 (steps as metall_thick 18), sleepers `concrete_walkable` 67 (as concrete 19); walker faces `walker_col_s`, no material |
| | `props/railroad/railroad.model` | both 10/20 | every face `default` -> **`metall_tank` 55** (the model's settings) |
| | `railroad_double_deadend` | both 10/20 | its own |
| Chemical Plant 0.11e `v011e_level_04` | `ported/v011e/props/railroad/*` (5) | both 10/20 | the port's |
| Cologne `level_08` | `level/08/railroad_keln/rails_keln` (11), `railroad_keln` (21) | bullet 8/16 + walker 2/4 | rails metall_thick 18 (598 triangles, y 31.980..32.138); deck metall_thick and asphalt 20 at y 31.979; walker: 12 boxes a panel, 0.22 m square turned 45 degrees (a ridge 0.157 m over the deck), no material |
| Fort `level_05` | trolleys and a handcar on the narrow gauge | 10/20, 8/16 + 2/4 | their own |
| Repair Depot `v010b_cs_mp_rembasa`, Station, Depository | X-Ray level geometry, every triangle 10/20 | both | X-Ray metals -> metall_thick 18 (1,750 m² of upward metal on Repair Depot; that the rails are among them is LIKELY, not isolated) |

No model named for rails, trains or wagons is placed on Rudnya (stock and 0.69), School, Radar,
Mamayev, Vector or London: Rudnya's are `trailer_2pts` trailers (8/16 + 2/4). Rails modelled
inside a larger model would not show by name.

### 1.2 The owner's report, from play: steps on rails are silent

The owner, 2026-11-16, after the first version of this note: "rails are not heard at all, they
changed algo at the very least in 0.69". Taken as fact. The data reading (1.1, the census) puts
bullet collision with a stepping material under every foot on a rail, so the silence is on a path
the data reading does not model. 1.3 reads every condition of that path; 1.4 is 0.69d's.

**Found by the owner's walk (PROVEN, 2026-11-16, the step log): the foot's ray misses a thin floor
the capsule stands on.** On the Chemical Plant (FFA `level_04/evening_8`) 53 of 153 steps were
silent, every one "game missed; walker missed", the body on the ground and its foot 10 to 25 cm from
the root; 50 of them on the loading station's top walkway (`loading_station_01`), whose walker
collision is a strip 32 cm wide (no material) over an 8 cm metal beam, nothing else within 9 m
below. The capsule rests on the strip; a foot beside it casts its vertical ray past both. The
fix: when the foot finds no named ground, the looks go around the body's root, where the capsule
rests, and take the highest named surface (patch 153's contact, branch fix/step-contact,
footsteps.md "The contact"). Walk 2 with it: 5 of 127 silent (all in the air), 62 grounds found by
the contact (metal 60). Also found: each step event reaches the handler twice (the same effect,
the same time), so stock plays every step twice. The static reading below stands; what it could
not see was where the foot point is against so thin a floor.

The static reading's result: no condition of 0.26g's step code fails specifically on a rail.
Ruled out with the code below: the controller's state (it gates no step event), the ray's start
and length, back faces, slope or normal limits, material flags, the body's own hit model, the
compound and primitive paths, the pair. What the code takes from run time and the data reading
cannot see is **where the foot point is** (1.3 step 4) and **whether the walk clip's events reach
the handler** (step 1). 0.69d differs exactly there and in the miss (1.4). One walk with the log of
1.6 names which; the fix for each case is in 1.5.

### 1.3 0.26g: a step from its source to the sound (PROVEN, the original exe)

1. **The event.** An animation event of the body's `player_sound_events` channel
   (`crouch_steps.md`): the run, sprint and (since crouch-steps-2) crouch clips have it, two steps a
   cycle. The handler is `player_equipment_sound_effect::on_player_sound_event` 0x2A6470 (the held
   item's effect, item +884h). **The character controller decides nothing here.** The game reads the
   controller in two places only: `on_ground` (0x7EF90), whose one caller is the jump gate 0x2FC230
   (jump input, then on_ground, 0x2FC285), and the landing callback (`call_landing_callback` 0x803D0
   from `step_down` 0x7EA67 -> `player::on_landing` 0x1F3B30: fall damage 0x2D8220 and the landing
   step 0x23FBE0, no state change). `need_slide_on_supporting_surface` 0x7BC20 and the slide
   acceleration 0x7BA30 feed only `player_step` 0x7BDA0 inside the controller. So a body standing
   on the ridge does not count as "not grounded, so no step event": no step event depends on the
   controller. (The ridge does make it slide: the support normal more than 50 deg from up
   (cos 50 deg `[0x21E51D8]`), or 40 to 50 deg (cos 40 deg `[0x21E51DC]`, from 0.6981 rad at
   0xA2DCD8, initialiser 0x84D800) at low speed, slides; a 45-degree diamond face of Cologne's rails
   is in that band, so a body that stops on the ridge slides off it. Walking on, it does not.)
2. **The time guard** 0x2A6485: an event at or before the core's `maximum_time` is dropped
   (re-simulation; patch 101 relaxes it).
3. **Whose** 0x2A6495: the event's object must be the effect's user.
4. **The feet** 0x2A64A9 / 0x2A64C2: the effect's +44h, set by `set_feet` 0x2A6410 when
   `update_feet` 0x2A63C0 runs. That runs from the held item's `update_bones_matrices` 0x2956E0
   (vtable slot, call at 0x29573A, skipped while user +0F4E3h is set) or, for a culled player, the
   root transform (0x295750). **A foot point is the LeftFoot / RightFoot bone (indices found by name
   by `set_user` 0x2A62D0 in the user's skeleton, user +408h) of the pose the held item is drawn with,
   times the transform the draw passes (0x2A63D3, 0x2A63E5), kept until the next draw.** For oneself
   in first view that is the first-person draw's pose and transform; that it puts the foot where the
   body's foot is was never checked (step-sounds-research section 9).
5. **The foot material** 0x2A6545: player + 0F2C4h + 4 x movement id + 2 x third view (an on-site
   state, id 0, reads the word before the table: `footsteps.md` path 6).
6. **The step manager** 0x2A6558 (world vtable +34h); none: return.
7. **The ray**, `step_manager::on_step` 0x248AED..0x248B58 -> `ray_test` 0x75E20: origin foot +
   (0, 1.0 `[0xA70884]`, 0), direction (0, -1.0 `[0xA2D778]`, 0), length 2.0 `[0x9CC87C]`, group 30h,
   mask 8, no delegate, the last argument 1 which sets the callback's flag 2 (0x75F2E: Bullet's
   `kF_KeepUnflippedNormal`; `kF_FilterBackfaces`, 1, is never set, so back faces are hit).
   - `needsCollision` 0x75BD0: an object is tried when its group has bit 8 and its mask 10h or 20h.
     Hit models are group 40h, mask 11h (the values patch 103's check uses): 40h & 8 = 0, so **a
     body's hit model never meets the step ray** and cannot hide the ground from its own foot.
   - `addSingleResult` 0x75C40 keeps the closest hit. No slope, normal or material-flag test exists
     on the way. A triangle mesh reports through `BridgeTriangleRaycastCallback::reportHit` 0xA0F60
     (local info: part, triangle, flag 0); a compound's child through `LocalInfoAdder2` 0xA1060
     (part -1, child index, flag 1, only when the child gave none); a convex primitive through the
     GJK cast, which reports nothing for a ray that starts inside it. The rails and their beds are
     triangle meshes (1.1); Cologne's two primitive boxes per deck are the panels' end beams.
   - A second pass (0x75F4C) happens only when a group-40h delegate asked to be posed first: never
     for this ray.
8. **The material** 0x248B75, `get_triangle_material` 0x3570A0: a mesh hit reads the triangle
   table (+10Ch), a primitive hit the primitive table (+110h), & 3FFFh; no table: 0FFFFh.
9. **The pair** 0x248B89 (`material_pair` 0x306D50, one base level), none: return (0x248B92); its
   step sound +20h, none: no sound (0x248BA5); `emit_and_play_once` 0x248BF7, a hud sound for one's
   own first view, else a point sound at the hit.
10. After it: the voice limit (patch 96).

On every rail of 1.1, 3 and 5 to 9 pass by the data and the code (a rail and its bed are bullet
meshes with materials whose pairs have sounds; the census found the ray meets them under the body).
2 is not rail-specific. Left: 1 (do the events reach the handler) and 4 (where the foot point is).
A foot point that is right cannot miss on a rail: under a rail head 4.5 cm wide there is the rail's
web and foot (Chemical Plant, y 2.32 against a head at 2.48), the sleeper or the bed 0.1 to 0.3 m
down. Only a foot point far off the body (more than about 1 m above what is under it, or over
something else) misses there. Not run: Bullet's mesh raycast under Unicorn on the real rail
triangles (patch 103's check runs the game's ray test end to end, on spheres; a mesh needs its
BVH built by the game's cook). It would only confirm 7 and 8, which the code already shows.

### 1.4 0.69d's step (PROVEN, the retail x64 exe)

- **Event** `on_player_sound_event` 0x2002A2AA0: stores, per foot, the event's time and the logic
  state's movement id (+7B8h..+7D0h). No time guard and no play here.
- **Feet** every frame 0x2002A3050: this frame's foot matrices x the user's transform (culled:
  0x2002A3030), then the effect's update 0x2002B5E40 plays a foot whose stored time is newer than
  the one it last played (+58h / +74h, +5Ch / +78h), through `on_foot_step` 0x2002B51C0: both foot
  materials (first and third view) of the stored state, and the player's id.
- **Ground** `step_manager::on_step` 0x2001FAC50:
  1. the player by id (0x2001FAC8E); `g` = the character controller's ground material (+612Eh,
     0x2001FACAA), 0FFFFh without a player. `step_down` writes it with the support normal (+6130h)
     from its own sweep (0x2005FBC60; written at 0x2005F408C and 0x2005F623B; serialized,
     0x2005F7C57);
  2. the same ray as 0.26g: origin +1.0 (0x2009E5D88), direction -1.0 (0x2009E61C8), length 2.0
     (0x2009E5EBC), group 30h, mask 8, no delegate, flag 1 (0x2001FACE5..0x2001FAD56);
  3. a hit replaces `g` with the hit triangle's material (0x2001FAD70); a miss keeps `g`;
  4. `g` 0FFFFh: silent (0x2001FAD7D);
  5. an AI noise event (0x2001FADC6), the pairs of both foot materials (0x2001FADDE, 0x2001FAE0D),
     their sounds given to the world with the player's id **at the foot**, the decal and particles
     at the foot (0x2001FAEA5).
- **Exactly what differs from 0.26g:** (a) a ray that meets nothing falls back to what the body
  stands on, walker collision included (the controller's sweep meets walker faces; an unnamed face
  is 3FFFh after the mask, so it steps as `default`: LIKELY, the sweep's material read not traced);
  (b) the feet are this frame's and the step plays in the frame, not the feet of the last draw at
  the event; (c) the sound is at the foot; (d) no `maximum_time` guard (times compared instead).
  The ray, its filter, the material read and the pairs are the same. So a step 0.69d hears and
  0.26g does not is one whose ray meets nothing (a), or whose foot point 0.26g has wrong and 0.69d
  right (b); with (a), 0.69d is heard even when the foot point is off, as long as the body stands
  on something.

### 1.5 The fix

- **0.69d's (a): patch 153** (fix/silent-steps, `step.ground` 0x248B5A, not merged, another
  session's). On a miss it looks again with the walker collision (34h / 0Ah) to 1 m under the foot,
  takes a walker face's material or the hittable surface to 4 m under it or `default`, and plays at
  the foot. That is 0.69d's rule computed when asked; it makes any rail step whose ray missed heard.
- **What 153 does not cover: a foot point off the body** (0.69d's (b)). 153's looks start from the
  same foot point, so they miss with it. The smallest addition, in the same hook: when every look
  from the foot misses, one more look from the body's root (the player's transform position,
  which the controller keeps on its support) straight down 1 m, the walker filter: what the body
  stands on, as 0.69d's controller material is. One ray more on a miss, nothing on a hit; it
  composes with 153's `Ground` enum as one more variant before `Air`.
- **If the events never reach the handler** (the log shows no event lines while walking the rail),
  neither on_step change helps: the cause is in the clip or the logic state, and the fix is 0.69d's
  per-frame playback (b) or the clip's channel, decided by what the log shows.

### 1.6 One walk settles it: the log

`SKHID_STEP_LOG=on` exists in patch 153 (one line at `step.ground`). Two additions:

- a line at `on_player_sound_event`'s entry (0x2A6470, before the time guard), so a step that never
  reaches on_step is visible: the event's time, `maximum_time`, the domain, the feet flag, and what
  the handler did with it;
- on the step line: the body's root (the player's transform) and the foot's offset from it, and
  the controller's support normal (+2C710h) and `on_ground`.

```
srp: step event: own, foot 0, time 123456 (max 123400), feet read: on_foot_step
srp: step: own foot, movement 2, first person, foot 42 at 1190 248 5000 cm (root 1180 250 5005, foot-root 10 -2 -5 cm); support y 0.71, on_ground; game met 18 at 2 cm; ground 18 (game): plays
srp: step: own foot, movement 2, first person, foot 42 at 1190 395 5000 cm (root 1180 250 5005, foot-root 10 145 -5 cm); support y 0.71, on_ground; game missed; walker missed; root met 16383 at 0 cm: plays
```

The owner walks a rail of the Chemical Plant, of Cologne and of Repair Depot, both on the rail and
beside it. Reading it: no `step event` lines while walking: the event path (1.3 step 1); `game
missed` with a large foot-root offset: the foot point (153 plus the root look); `game missed` with
a small offset: a collision gap (153 alone); `plays` on every step and still nothing heard: lost
after on_step (the voices, the volume).

### 1.7 What the data reading found near rails (first version of this note)

These stand as data and code; they do not explain the owner's silence (1.2).

- Everywhere above, the rail and what is under it are bullet collision with a material that has
  every step pair: the step's ray (1 m above the foot to 1 m below, group 30h / mask 8) meets the
  rail head, the sleeper, the deck or the ballast. A rail is thin, but the ray is vertical from
  the foot: a foot beside the rail meets the sleeper or ballast 0.1 to 0.2 m under it, also with a
  sound. The step census (`private/ladder-work/step-sounds-research.md` section 3) measured it:
  Cologne's rails 9,366 m² of standing room, 0 m² where the ray misses; the Chemical Plant's rails
  213 m², 0.8 m² misses; the ported maps cannot miss (every triangle is also walker collision).
- The rail steps are not quiet either: own walking on metall_thick is -24.0 dBFS peak x vk, louder
  than on concrete (-28.6) or ground (-31.8); others' -12.2, against -14.7 on concrete and -22.2 on ground (4.2).
- The controller's ground and slide tests and why they gate no step event: 1.3 step 1.
- **Silent places near rails the data does show** (step-sounds-research section 5):
  Cologne's ICE cars (their collision is `foliage_dry`, no step sound: fixed by step-sounds-1, see
  #4), the Chemical Plant's hopper wagons (12.6 % of their tops) and loading stations (walker-only
  tops: fixed by patch 153's walker look), own sprint and landing on the mud beside the tracks
  (fixed by step-sounds-1). These are not the owner's rails.

### 1.8 What else is wrong on the rails: the Chemical Plant's material

Besides the silence, the Chemical Plant's tracks have the wrong material. `railroad.model`
(10/20, 28,177 triangles, 682 m²: the whole track bed, rails and sleepers, x 10.4..21.0,
z -15.9..126.1) and `railroad_01..03` (the same tracks, z 17.8..82.8) are both placed and both
met by the ray; the higher face wins, so the ground under a walker is `metall_tank` 37 %,
ground 23 %, metall_thick 21 %, concrete 16 % (census: of 213 m², 78 / 49 / 44 / 35). A
walk along the track plays a hollow tank, rail steel, concrete and dirt in turn. A bullet into
the bed hits `metall_tank` (its pierce and ricochet values too). Fixing it in the collision
changes what bullets do (a simulation change, `simulation_changes.tsv`); a step-only fix would
be patch 153's site choosing another face (exe).

0.69d has no `props/railroad/railroad.model` at all (not in its file list): its tracks are
`railroad_01..03` alone, and their rail faces (`coll_metal_thick_s`) are `metall_tank` there
(0.26g: `metall_thick_walkable`). So 0.69d's tracks step as one material, and the duplicate bed is
0.26g's alone.

## 2. Surfaces: geometry with no pair or no material

From each live map's `day_4` project, every collision triangle placed and its material read
through the model's `settings` (scratch `py/mapmats2.py`). "Bullet" is collision a bullet and the
step ray meet (group 8, mask 10h or 20h); "up" is faces within 50 degrees of horizontal.

Area of bullet collision (m², all faces / faces a walker can stand on) whose material plays
nothing for a bullet (foliage_dry, the gratings, grass_soft, restrictor) or resolves to `default`
(ground_not_walkable, no material), and stone (4.3). Background models (`*_bg*`, `bkg`, `back*`,
`border`, London's light plane) are in the totals; the playable parts are named below.

| map | foliage_dry | gran_pass 40 | gran_not_path 41 | grass_soft | restrictor | ground_not_walkable | no material | stone |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Rudnya level_01 |  |  | 5 / 1 | **6,087 / 6,033** |  |  | 64,223 / 1,451 | 1,989 / 463 |
| School level_02 | 1 / 1 |  |  | 33 / 4 |  |  | 62 / 10 | 356 / 44 |
| Radar level_03 |  |  | 238 / 17 |  |  |  | 12,273 / 2,542 | 552 / 144 |
| Chemical Plant level_04 |  | 9 / 0 | **5,373** / 111 |  |  |  | 241,596 / 51,420 | 811 / 209 |
| Fort level_05 | 6 / 6 |  |  |  |  | 24,708 / 15,645 | 1,230 / 459 | 928 / 100 |
| Mamayev level_06 | **1,608 / 834** |  |  |  |  |  | 625,546 / 381,739 | 1 / 0 |
| Vector level_07 | **1,053 / 464** |  |  |  |  |  | 35,674 / 33,495 | 77 / 1 |
| Cologne level_08 | 205 / 195 |  |  | **3,841 / 3,617** |  |  | 858,179 / 730,876 |  |
| London level_09 | 1 / 1 |  | 4 / 0 |  |  |  | 18,055,666 (light plane) | 4,253 / 173 |
| Radar 0.11e |  | 370 / 37 | 2,745 / 624 |  | 312 / 119 |  | 23,675 / 449 | 633 / 166 |
| Chemical Plant 0.11e |  | **12,247** / 377 | 1,300 / 311 |  |  |  | 240,884 / 50,775 | 744 / 191 |
| Radar 0.21d |  | 314 / 35 | 1,005 / 206 |  | 283 / 109 |  | 8,531 / 638 | 555 / 145 |
| Mamayev 0.21d | **3,601 / 2,741** |  |  |  | 26 / 11 |  | 12,604 / 2,994 |  |
| Rudnya 0.69 |  |  | 5 / 1 | **794,290 / 793,272** (18,469 outside the background) |  |  | 64,250 / 1,453 | 1,995 / 465 |
| Bath, Military Warehouses, Repair Depot, Training Camp, Depository, Station | (Training Camp 2) |  |  |  |  |  |  | 4,244 / 7,293 / 1,680 / 130 / 3,501 / 2,708 |

Where it is played on, by model:

- bullet-silent: Rudnya's `terrain.model` (grass_soft 6,087 m²) and Rudnya 0.69's terrain
  (18,469 m² outside the `terrain_bg_*`): **a shot into the grass of Rudnya makes no sound**
  (the puff and the hole are there); Cologne's moss (`mossmoss_04..06`, 2,257 m²); Mamayev's
  ground (`object_1` 823, `ground_1_*`) and its 0.21d port (`terain1` 1,829); Vector's
  `centre_lab` 709, `base_v_outdoor` 273; the gratings of the Chemical Plant's loading stations
  (2,999 + 2,286 m²; the 0.11e port's 5,060 + 4,777 as `metal_arm_gran_pass`), Radar's
  `bunker_chemical` 220 and the 0.11e/0.21d Radar fences (`fence_outer_part2..5`); limiter faces
  a bullet meets on the 0.11e/0.21d Radar (`blocks`, `garage`).
- no material (so dirt for steps and bullets): Radar's `garage_inside` 5,456 m² and `arsenal_in`
  1,422 (concrete rooms), Mamayev's `monument` 3,772 and `statue_ground` 1,395 (also in the 0.21d
  port), Vector's `centre_lab_dome` 3,090, London's `building_07_3` 11,144 and
  `building_06_1_indoor_01` 2,368, the Fort's `out_wall_base_a_2` 532 and `center_part_01_floor`
  254, the 0.21d Radar's `anomaly_rust_tree_leaves` 3,333. A wrong sound (dirt on concrete or
  stone), not silence. Fix: the models' `settings` (a face name -> material: content, but the
  material is also the bullets', so a simulation change).
- `ground_not_walkable`: the Fort's terrain (`lvl_05_terrain_17..19`, 15,645 m² walkable) steps
  and is hit as `default`; for 0.26g's pairs that is the same sound as `ground` (both
  `step/default` / `default_steps_m`, `ground_hits`), so nothing is heard wrong today. It would
  matter if 0.69d's step sets came (0.69d tells ground from default).

- A face whose name a model's settings do not know is 0FFFFh at cook time and 3FFFh after the
  mask (`get_triangle_material` 0x3570A0). `material_pair` (0x306D50) reads the material table at
  that index, 128 entries long, so 3FFFh reads 0x10104 bytes into the manager, inside the pair
  table at [126][127], which is empty: the side becomes `default` (11). So every face with no
  material steps as dirt and is hit as dirt. Harmless by luck, not by design. PROVEN.
- A base material is followed one level only (0x306D5F, 0x306D92): `ground_not_walkable` 65 ->
  `ground_walkable` 58, which has no pair -> `default`. The same in 0.69d (its game.materials has
  the same base). PROVEN for 0.26g.
- The bushes and fruit bushes drawn without collision (School 82, Rudnya 71, the Fort 43,
  Chemical Plant 5: `stalker-maps.md` "Bushes rustle") neither rustle nor take a hit sound:
  there is nothing to meet.

## 3. material.pairs against 0.69d's

0.26g: 62 materials, 10 pair masters (bullet 7, actor 10, artefact 14, actor_local 42, grenade
45, actor_sprint 53, actor_crouch 54, actor_local_sprint 56, actor_local_crouch 57, melee_blunt
74), 40 slaves each, 400 pairs. 0.69d: 83 materials (21 new: sand, snow, slime, crystal, fog, the
`*_solid` and `transparency1_*` metals, nut, snowball...), 12 masters (+ nut 83, snowball 86), 43
slaves each (+ mud_walkable 61, sand 77, snow 85), 516 pairs. The pair schema is identical (23
fields). New 0.69d materials matter only for 0.69d geometry; no live 0.26g face uses them
(they have no id in 0.26g's game.materials).

The foot materials (from every boot's config, PROVEN on 40 boots): walk 57 own / 54 others, run
42 / 10, sprint 56 / 53, jump (the hard landing) 56 / 53; the soft landing uses run's.

### 3.1 Steps: empty pairs after step-sounds-1

Live + step-sounds-1: only `actor` 10 and `restrictor` 37 stay empty for the six foot masters
(on purpose). 0.69d leaves more empty: the gratings 40/41 and car glass 47 for walk and sprint,
and own sprint on foliage 24/44; it fills crouch on restrictor and actor. So for steps, 0.26g
with step-sounds-1 has more surfaces with a sound than 0.69d.

### 3.2 Steps: what 0.69d plays instead

0.69d replaced the step sets: its pairs name `step/run/<ground>`, `step/run_3rd/<ground>`,
`step/sprint/...`, `step/sprint_3rd/...` and one `step/crouch` / `step/crouch_3rd` for every
ground (mud and snow their own). Each is a composite of three layers: the ground's recording, a
gear layer (`material/run_wear`, `wear_sprint`, `sprint_boots`, `melee_wear`) and the room layer
(`step/environment_run` etc., 14 rooms, section 5.1). Different from 0.26g's choices:

- concrete and brick step as concrete (0.26g: asphalt and gravel); stone as concrete;
- **foliage_dry steps as `default`**, the plain dirt step, for every foot. step-sounds-1 gives it
  the coniferous bush rustle, which is 9.5 dB louder than 0.26g's own `default` for others'
  walking (-4.7 against -22.2 dBFS) and 12.7 dB louder than 0.69d's (-17.5). foliage_dry is the
  ground of Mamayev Kurgan (1,035 m² walkable, its 0.21d port 2,971 m²): a walker there would
  rustle like a bush at every step. Recommendation for fix/silent-steps: the six foliage_dry rows
  take the `default` sounds (others `material/default/step/default_steps_m`, own `step/default`,
  sprints `..._sprint`), as 0.69d.
- glass and glass_hard step as `default`; sand as default; snow its own.

### 3.3 Bullets: the same data in both builds

All 40 shared bullet pairs are byte-identical between live 0.26g and 0.69d (sounds, particles,
decals); 0.69d adds only snow. The same five bullet pairs are empty in both:

| slave | sound | particle | decal | where (census, bullet collision) |
|---|---|---|---|---|
| foliage_dry 49 | none | none | none | Mamayev's ground, Vector's lab, Cologne's ICE cars |
| metal_arm_gran_pass 40, _not_path 41 | none | none | none | gratings: Chemical Plant 5,373 m², Radar 238 m² |
| grass_soft 51 | none | yes | yes | **Rudnya's terrain 6,087 m²**, School's anomaly fruit |
| restrictor 37 | none | none | none | limiter faces (bullets should not meet them) |

The selection rule is the same too: `bullet::process_collision` (0.26g 0x32EBD0) plays the
pair's `collision_sound` (+20h) through `bullet_manager::play_sound` (0x3222F0) only when
`player_shootmarks_storage::add_shootmark` (0x32E460) took the mark for the first time, the
bullet's byte +80h is 0 and its count +84h is 0; `process_front_face_piercing` (0x32E4F0) plays
`piercing_sound` (+18h) while +84h <= 1. 0.69d's (0x2002A5930) has the same three tests
(0x2002A598F, `[rsi+0Ah]`, `[rsi+68h] < 1`). PROVEN. So no impact is better in 0.69d; the empty
pairs are a gap of both. Fix (data): bullet x foliage_dry -> the deciduous foliage hit
(`material/foliage_2/bullet/foliage_hit`, the leaves' particle); x gratings -> rabitz's
(`material_hit` metal); x grass_soft -> grass's.

### 3.4 Grenades (the RGD-5, smoke grenade and trap, material 45)

The bounce is `grenade` vtable 0xA046A0 slot 8, 0x248980: the pair (grenade, the hit triangle's
material) at the contact, its `collision_sound` (+20h) when the grenade's cooldown has passed
(+234h >= +228h + +22Ch). PROVEN. The trap's own handler (`booby_trap` vtable 0xA0455C slot 16,
0x248160) asks the pair for its decal only (LIKELY: no sound call in its body).

Empty in live 0.26g: actor, body, mud, foliage_coniferous, water, plastic, glass, paper,
deep_water, cloth, rubber, restrictor, glass_hard, the gratings, wood_tree, metal_car, glass_car,
metal_gas_tank, foliage_dry, wood_bridge, grass_soft, metall_tank (23). 0.69d fills glass_hard,
metal_car, metal_gas_tank, wood_bridge, metall_tank with two new composites
(`weapons/f1_surface_hit`, `weapons/f1_metal_hit`, preset `vss_main` in two windows) and replaces
the rest of the f1 bounces by them: 2.3 to 2.8 dB louder on ground and wood, 8 dB on metal
(peak x vk). Fix: data (pair rows; the two composites and their samples as a pack, or the stock
`f1_hit_*`).

### 3.5 Melee

`melee_blunt` 74 has sounds only on body/actor, wood (4 kinds), metal (thin, thick, car, gas
tank, tank), mud, rubber; the other 26 slaves are empty, identical in 0.69d, and neither build has
a recording for stone, concrete or ground (`sounds/single/material/melee_hit/`: body, car, gas
tank, thick and thin metal, mud, rubber, wood). Fix: data, pairs to existing samples (the mud hit
for ground and grass, the thick-metal or a bullet's concrete hit for stone); the owner's ears.

### 3.6 Bodies and dropped things

There is no pair master for a body or a dropped item: a dying player plays the death composite
(`animations/death_animation`, `death_fall` + scream, surface-independent, the same in 0.69d).
The ragdoll (patch, visual only) and the dropped gun make no impact sound when they land
(nothing calls `material_pair` for them). A thud on landing would be exe (the ragdoll module).

## 4. Volumes and ranges

### 4.1 The distance curves (PROVEN, the `.spl` files' `volume` keys, read linearly)

| preset | build | 1 m | 3 m | 5 m | 8 m | 12 m | 17 m | 25 m | silent from |
|---|---|---|---|---|---|---|---|---|---|
| `steps` | 0.26g | 0.67 | 0.39 | 0.23 | 0.11 | 0.06 | 0 | 0 | 17 m |
| | 0.69d | 0.94 | 0.83 | 0.78 | 0.60 | 0.21 | 0.10 | 0.02 | 30 m |
| `steps_distance` (0.69d only) | 0.69d | 0.99 | 0.94 | 0.79 | 0.36 | 0.13 | 0.07 | 0 | 25 m |
| `main_3rd_view` | 0.26g | 0.96 | 0.87 | 0.73 | 0.51 | 0.22 | 0.07 | 0 | 19.3 m |
| | 0.69d | 0.98 | 0.94 | 0.81 | 0.61 | 0.36 | 0.14 | 0 | 25 m |
| `weapons_reloading` | 0.26g | 0.61 | 0.19 | 0.06 | 0.01 | 0 | | | 9 m |
| | 0.69d | 1.00 | 0.44 | 0.15 | 0.05 | 0.01 | | | 13 m |
| `mid_range` | 0.26g | 0.83 | 0.50 | 0.30 | 0.14 | 0.06 | 0.02 | 0 | 20 m |
| | 0.69d | 0.89 | 0.67 | 0.46 | 0.25 | 0.12 | 0.08 | 0.05 | 40 m |
| `close_range` | 0.26g | 0.73 | 0.22 | 0 | | | | | 4.5 m |
| | 0.69d | 0.85 | 0.19 | 0.04 | 0.02 | 0.02 | 0.01 | 0.01 | 60 m |
| `weapon_shutter_close` | 0.26g | 0.89 | 0.68 | 0.49 | 0.28 | 0.13 | 0.06 | 0.02 | 30 m |
| | 0.69d | 0.95 | 0.84 | 0.71 | 0.51 | 0.30 | 0.14 | 0.03 | 30 m |

`material_hit` (bullet impacts) and `default` are the same in both. `low_range`, `melee_hit`,
`melee_wear`, `vss_main` differ slightly. The gain is the curve at the distance times the
sample's `volume_k` (`weapon-reverb-layers.md`, 0x5C7A0), so these are data fixes: a `.spl` file
is a config of two curves and nothing else (`grenade_bounce.md`). Who uses each in 0.26g (the 6,557 live `.raw_options`): `steps` 720 samples, 618 of them steps
(`material/step` 404, `crouch/material` 214), the rest breathing (`human/stamina` 36), bones
cracking (8), a few guns' handling (VSSK, AK-12, SOCOM, SVD...: 50); `weapons_reloading` 850,
the guns' reloads and handling; `main_3rd_view` 287, others' shots; `material_hit` 112, the
bullet impacts. A new `steps.spl` changes the stamina and bone sounds' reach too; a step-only
retune is a new preset named in the step samples' options (an edit list over 618 files).

The bullet hit on a body or a corpse (pair bullet x actor/body) is in `weapons_reloading`: in
0.26g it is a fifth of its 1 m loudness at 3 m and gone at 9 m.

### 4.2 Step sources (peak x vk, dBFS; the full tables are the scratch's `out_steps*.txt`)

| foot | ground | 0.26g live | 0.69d | difference |
|---|---|---|---|---|
| own walk 42 | ground, default | -31.8 | -25.4 | +6.4 |
| | concrete / asphalt | -28.6 | -24.1 / -22.6 | +4.4 / +6.0 |
| | grass | -28.2 | -22.8 | +5.4 |
| | metall_thick | -24.0 | -20.6 | +3.5 |
| own sprint, landing 56 | ground | -18.9 | -12.2 | +6.7 |
| own crouch 57 | ground | -39.7 | -22.4 | +17.3 |
| others walk 10 | ground | -22.2 | -17.5 | +4.7 |
| | metall_thick | -12.2 | -12.8 | -0.6 |
| | stone | **-3.3** | -17.5 | -14.2 |
| others sprint 53 | ground / concrete | -12.9 / -14.7 | -5.8 / -5.5 | +7.1 / +9.2 |
| others crouch 54 | ground | -30.2 | -17.1 | +13.1 |

Others' steps in 0.69d are about 5 dB louder walking and 7 to 10 dB sprinting at the source, and
fall off far later (4.1): at 8 m a sprinting enemy is 17 to 24 dB louder in 0.69d (source +7..9 dB, curve +10..15 dB). Own steps:
0.26g's own sets are a quiet ground layer (`volume_k` 0.02 to 0.16) and the cloth rustle
`material/step/steps` (0.28); 0.69d's are their own recordings plus the gear layer. The crouch
figures are crouch-steps-1's 0.4 by design (the owner's), and 0.69d's crouch is louder still.

### 4.3 Stone

The pair (actor 10, stone 21) plays `material/concrete/step/concrete_steps_m`, seven samples with
`volume_k` 0.7, peaks at 0 dBFS; concrete and asphalt play `asphalt_steps_m` at 0.2. Others' steps
on stone are 11 dB louder than on concrete (render 04). Stone is X-Ray's tiles and roofing on the
ported maps (`flooring_tile`, `shifer`), walkable: Military Warehouses 3,677 m², Bath 3,240,
Depository 2,087, Repair Depot 1,470, Station 1,445; on the stock maps 44 to 465 m² each (2). Fix: the pair row (to asphalt's, as 0.69d steps stone as concrete) or
the seven `volume_k`.

## 5. What 0.69d does that 0.26g cannot by data

### 5.1 Sound environments (rooms)

- **The data (PROVEN):** each 0.69d map's client project has `sound_environment` objects
  (`game_object_type` sound_environment, `lib_name` base:sound_environment): an
  `environment_name`, an optional `ambient_sound` bed (a collection) with fade in/out (200 to
  300 ms) and `force_mono`, and links to `sound_environment_geometry` primitives (boxes placed and
  scaled: the room's volume). Per map (tdm): Rudnya 6, School 8, Radar 9, Chemical Plant 7, Fort
  5, Mamayev 7, Vector 10, Cologne 6, London 6. Their names are a shared set of 19 rooms
  (`school_class`, `cp_tunnels`, `rudnya_houses`, `atp`, `rls_main_room`, the `*_outdoor`s...),
  reused across maps.
- 12 composites have items gated by the room: an item carries `sound_environment` <name> (read by
  0.69d's composite reader at 0x2006329EF, at most 32 characters). The four step room layers
  (`step/environment_run`, `_run_3rd`, `_sprint`, `_sprint_3rd`: 14 rooms each, 580 samples) and
  eight weapon tails (`weapons/environments/weapons_1st_view*`, `weapons_3rd_view*`: per room a
  close + distant indoor tail or the outdoor one). So in 0.69d a shot in the School's classroom
  rings like a classroom and one in the Chemical Plant's tunnels like a tunnel (render 06).
- **0.26g (PROVEN):** the string `sound_environment` has one use, in
  `static_model_instance_cook::translate_query` (0x125DB6, a model's settings key); the composite
  reader (`composite_sound` 0x63CB0, emit 0x63D60) never reads it. **A 0.69d composite copied as
  it is would play all 14 room layers of a step at once.** Any port of 0.69d step or gun
  composites must drop the room items (or keep one).
- Neither exe has a real-time reverb: no XAudio2 `AudioReverb` CLSID in either (2.7's
  `XAudio2CreateReverb` creates it by that CLSID), no xapofx, no X3DAudio; 0.26g's
  `XAUDIO2FX_REVERB_I3DL2_PARAMETERS` is an allocator's name, unused. 0.69d's rooms are recorded
  tails and beds, chosen by zone. No occlusion or obstruction of game sounds in either
  (`ally_hearing_radius_obstructed` is the AI's hearing).
- **To have it in 0.26g:** exe work. A Rust patch owning the zones (the boxes of each map, read
  from a file of our own, not from the project: 0.26g's project loader does not know the object
  type), the listener's current zone (NOT VERIFIED whether 0.69d picks the listener's or the
  emitter's zone), a composite item's room tag honoured at emit (0x63D60), and the bed crossfaded
  on the scene. Content: the 19 rooms' tails and beds and the zones of each map as a pack.
  Large. A smaller first step with most of the effect: indoor/outdoor only (two tails) by a
  zone test at the shooter.

### 5.2 Bullet flyby

0.69d: `bullet_flyby_sound_effect`, keys `bullet_flyby_sounds`, `bullet_flyby_collision`, and
`weapons/bullets/pass_by_directional_subsonic` (16 collections: face/back x left-right x up/down,
96 samples). 0.26g has none of them. Exe (a near-pass test on the bullet's segment against the
listener, the side and direction picked) + content. Medium.

### 5.3 Step logic

- What the foot stands on when the ray misses: 0.69d takes the controller's ground material
  (`footsteps.md`); patch 153 gives 0.26g the same answer computed when asked. **In flight.**
- The sound at the foot, not at the hit point: 0.69d; patch 153 places its own found grounds at
  the foot, the game's own hits stay at the hit point (within 1 m: inaudible).
- Water: 0.69d's Rudnya pond wades because its bed is `deep_water`; 0.26g's other waters step as
  what their collision is. **In flight** (feat/wading-steps, feat/puddle-splash).
- Play deferred to the frame (0.27d0 and 0.69d): patch 101 covers what it fixed.

## 6. Other audible problems found on the way

- Others' footsteps carry a third of 0.69d's distance (4.1): the largest single difference a
  player notices in a fight ("I could hear him coming in 0.69").
- Hits on bodies fade by 9 m (4.1).
- Chemical Plant tracks' material flip (1.8) and stone's loud steps (4.3).
- `foliage_dry` in step-sounds-1 (3.2).
- The step-sounds-1 rows for car glass give others' sprint `default_steps_sprint_m` and own walk
  `step/default`, which 0.69d leaves silent: harmless, a choice.
- `pan_3d` in `.raw_options` is not read by 0.26g (`grenade_bounce.md`); 0.69d sets it on some
  creature sounds (rudnya-water-sounds memory): no effect in 0.26g either way.

## 7. Audio examples

`private/renders/sound-gaps/` (each file: the parts one after another, a common gain so the
loudest part peaks at -1 dBFS, relative levels kept; six steps 0.55 s apart unless said):

| file | parts |
|---|---|
| `01-own-walk-concrete_026g-then-069d-then-069d-corridor.ogg` | own walking on concrete: 0.26g `step/asphalt`; 0.69d `step/run/concrete` (ground + gear); the same with the `school_corridor` room layer |
| `02-own-walk-ground_026g-then-069d.ogg` | own walking on ground: 0.26g `step/default`; 0.69d `step/run/ground` |
| `03-others-walk-foliage_dry_stepsounds1-then-069d.ogg` | others on dry leaves: step-sounds-1's `foliage_step_m`; 0.69d's `step/run_3rd/default` |
| `04-others-walk-stone-then-concrete_026g.ogg` | 0.26g others on stone (`concrete_steps_m`), then on concrete (`asphalt_steps_m`) |
| `05-others-step-at-2-5-8-12-16-20m_026g-curve-then-069d-curve.ogg` | one 0.26g step at 2, 5, 8, 12, 16, 20 m on 0.26g's `steps` curve, then on 0.69d's |
| `06-069d-gun-tail-by-room_outdoor-classroom-corridor-tunnel.ogg` | 0.69d's third-person automatic tail in Rudnya outdoors, a School classroom, a School corridor, the Chemical Plant tunnels (all its layers at once, no distance window) |
| `07-grenade-bounce_026g-metal-then-069d-metal-car.ogg` | three bounces: 0.26g on metal (`f1_hit_metal`), 0.69d on metal and cars (`f1_metal_hit`); 0.26g on a car is silence |

## 8. What I would do next, in order

1. Before fix/silent-steps merges: retune step-sounds-1's foliage_dry rows to the `default` set
   (#4). Small.
2. One edit list "bullet-hits-1": bullet x foliage_dry, gratings, grass_soft (#5); grenade x the
   23 empty slaves to the stock bounces (#8); melee x stone-like and ground-like (#9); the stone
   step pair (#11). Data only, minutes each, the owner's ears on a local test.
3. The distance curves (#1, #12): 0.69d's `steps` (and `weapons_reloading`, `main_3rd_view`) as a
   1-to-3-file content change, after the owner hears render 05: it changes how far players hear
   each other.
4. The rails (1.2 to 1.6): add the event line and the root offset to patch 153's step log, and
   the root look to its hook, in fix/silent-steps (that session's call); then one walk along the
   rails with the log on names the cause.
5. Larger, exe: indoor/outdoor tails for shots and steps (5.1, first step), then bullet flyby
   (5.2), then the full room set with beds.
