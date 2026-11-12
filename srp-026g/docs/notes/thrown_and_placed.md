# Thrown and placed things in the 0.26g client: frag and smoke grenades, the bear trap

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game data.
PROVEN where an address is given, unless said otherwise. Counted on the saved worlds of recorded matches read with
the repository's own parser: 949 recordings with bots on one side (436,790 worlds) and 446 of the later set (191,201
worlds); a body with no client revision note is a bot. The matches ran revisions 40 to 50, so their bots ran the
code released then (the tags `026g-rev40` to `026g-rev50`: one grenade play in `reflex.rs`, no arc model): the
counts of the bots' throws below are of that code, and the files named under "Against the bots" are this branch's,
not yet measured the same way. See also `grenade_ccd.md`, `grenade_bounce.md`, `grenade_visibility.md`, `bear_traps.md`,
`damage_model.md`, `stamina_and_modifiers.md` (S<n>: the kit's modifier of index n), `spotting.md`.

## What a world holds

A game uploads a world about every second (not every five), and **every thrown grenade is in it**: its body's
position and velocity while it is in the physics world, its explode time and its id (`grenade_core::serialize`
0x303DA0). Bit 2 of its saved state is the core's "exploded" byte (+154h of the core: set by `explode_base` at
0x303B85, cleared by `start_fuse` at 0x303BA8).

## The throw

- A quick key's or the grenade key's press writes the grenade set's slot into the inventory's grenade slot (+17Ch),
  the release writes none (`want_throw` 0x309DC0; `process_quick_slots` 0x2D8BB0).
- Refused (`throw_grenade_pred` 0x2E4090): no unit left or the next grenade already thrown (`can_throw` 0x30A090),
  both hands broken (0x2E40EC), **the jump state** (the selector's state kind is 4, written by the jump state's
  constructor 0x336B87; tested at 0x2E409C), and **a sprint**: while the player sprints and until the weapon's
  sprint animation has ended (`m_sprint_animation_to_wait_for` +5ECh is not 6, 0x2E40A2; set by the sprint state
  0x3364F0, cleared at 0x2E2639). The same two tests gate the fire.
- The throw state has three stages: pull the pin, idle, throw. **The fuse starts at the press** (`start_throw`
  0x30A120, called by the first stage's start 0x3103A0): explode = the first 50 ms grid time after press + `ttl_ms`
  (4,000). Counted: 7,907 of 7,907 frag throws and 736 smokes. **The idle stage waits for the key's release**
  (0x310450): a held key cooks the grenade. The throw stage launches at its clip's "shoot" event (0x3104E7 ->
  0x310680 -> `launch_grenade` 0x30A250).
- So the launch is at **max(press + 400 ms, release) + 500 ms** (measured: a tap launches 900 ms after the press, 88
  throws, P10 to P90 900 to 950; a key held 600 ms or more launches 500 ms after the release, 140 throws, the
  median 500.00). The pull-pin clip `grenade_swing` is 12 frames at rate 1.0: 400 ms by the mixer's clock. The
  throw clip's `shoot` knot is at frame 16, 533 ms, not 500: the launch solved from the flight falls on the 50 ms
  grid at + 500, and that is the code's: a tick runs its lists in order (`atomic_tick` 0x2D1E20): the players'
  update, where the event makes the grenade's body at the hand (list 2), then the physics step of the tick's whole
  50 ms (list 3). So a grenade whose event falls at S + 533 is made in the tick that ends at S + 550 and has by
  then flown 50 ms from where it was made: its flight starts at S + 500 at the release point.
- The velocity is `throw_power` 17 m/s x max(0, 1 + S27) along the head's forward (0x30A2D5).
- The release point, measured on 147 throwers standing still: 1.73 m over the feet standing (1.29 m crouched),
  0.13 m behind the throw line and 0.21 m to its right (the right hand's bone at the event, `hand_transform`
  0x309F00).
- A grenade never launched explodes in the hand (0x303AB8). The blast is where the body is at the tick whose time is
  the explode time (0x303AEA).
- **After the first contact**: the physics world's swept test caps the speed. When a step's move is over the
  body's motion threshold and the swept sphere meets something, the body is put at the time of impact and its
  speed is cut to the threshold over the step, 0.03 m / (1 / 60 s) = **1.8 m/s** (0x9CA78..0x9CB60); then friction
  and no bounce. Counted on 911 flights flown from a seen position and velocity over the map's mesh: the seen burst
  is a median 1.0 m past the first contact (P80 5.3, P90 10.5). A flight with the cap puts the burst a median 0.53 m
  from the seen one (554 within 1 m); without the cap 4.2 m; stopping at the first contact 1.43 m; a fixed 2 m along
  the heading (the bots' rule) 2.13 m (84 within 1 m). In 142 of the 911 the grenade fell through the floor
  (`grenade_ccd.md`).

## The blast (`bullet_manager::explode` 0x339630)

- The numbers are the set's (`splash_parameters` 0x30A350): the radius x max(0, 1 + S12 of the thrower); the least
  and the most damage, the stamina damage and the stopping power x max(0, 1 + S11). Both modifiers are 0 in every
  kit read.
- Every body in the radius R: d = the distance from the centre to the body's origin, **its feet** (counted: the law
  from the feet fits 41 of 68 still victims to 0.01, from the capsule's centre 29); the amount = least + (most -
  least) x (R - d) / R (0x339736..0x339750). **No team test and no thrower test.**
- **Walls** (0x3393F0): a ray from the centre to each of six points of the body; every front face of a material
  with armour v cuts the power s (it starts at `splash_pierce` 0.15) to s x (1 - (1 - c)^2), c = clamp(s / v - 1,
  0, 1); the hit is the amount x what is left / `splash_pierce`. So **a face of armour 0.15 or more stops the blast
  outright** (ground, concrete, brick, stone, thick metal, tree trunks, a body); thin metal keeps 0.44, wood and
  hard glass 0.75, a car 0.98; glass, foliage, cloth, chain-link and water keep all.
- The six points are the hit model's bones RightArm, RightUpLeg, LeftUpLeg, LeftArm, Chest and Head (the set's
  `splash_affected_bodies`), at their pose of the moment; the Head's box feeds no pool from an explosive hit, though
  it counts for the stamina and the stopping power.
- They take the body, both legs, both hands and the pain pool (of 3,560 bodies hit that lived: 3,128,
  2,872, 2,856, 3,174, 3,139, 3,385; the head 33). One blast's hits carry one id, so a pool takes the largest.
- **The stamina damage and the stopping power are taken for each part that took damage**: a full exposure costs 60
  stamina (1,107 bodies at exactly 60) and six stopping hits of 0.1.
- The amount is 1 or more (the body and the pain pool emptied) within 2.4 m, and 0.5 or more (a leg and a hand
  broken) within 6.4 m.

## Smoke

The same object and fuse (4,000 ms from the press); its blast only starts the client's particle effect (0x248720).
**Nothing in the game's logic reads a cloud**: the particle code makes no physics object, so bullets, the blast's
ray, the use ray and the spot's ray (0x2D87C1: 100 m from the head) pass through it. A spotted enemy's name
(0x1F2B50) and map mark stay drawn through smoke and walls for the spot's 10 s. So a player sees nothing through a
cloud, but an enemy he or a mate spotted stays known to him through it.

The effect's times are seconds (the particle world's step is the frame's time, capped at 1 / 15 s, 0x13D1CD..
0x13D21D), so after the blast: the cloud's 50 puffs appear at 200 ms, are opaque by 2,085 ms, fade from 17,716 ms
and are gone at 29,200 ms (the effect's file; radius 8 m). **Each client draws its own cloud**: every random of the
particle code is one generator of the process that nothing seeds (0xAA48F8), so the puffs' places differ from
player to player.

## The trap (`booby_trap_core::on_enter` 0x304BD0)

The nearest body in the sensor sets it off, **the placer and his mates too**, unless the owner's S44 is over 0 and
the body is of his team (0x304CFE..0x304D58; S44 is 0 in every kit). The sensor is a cylinder of radius 0.25 m
against the body's capsule of 0.38 m: 0.63 m between centres. Each foot takes the configured damage x max(0, 1 +
S11 of the owner) (0x304A40). Counted: a caught body loses both leg pools and 0.60 of the pain pool and nothing of
the body pool (57 of 57). A fired or disarmed trap stays 8 s. **An armed trap is drawn for every team**: its model
is swapped by its state alone (0x247EF0), and the modifier `spot_enemy_traps` (index 47) is read nowhere.

## Counted

- **Who a frag's blast killed** (dead at the next world, the last hit at the explode time), first set: bots' 4,286
  frags: 367 enemies, 470 mates, **676 the thrower himself**; the other bodies' 4,990 frags: 457 enemies, 73 mates,
  24 themselves. Second set: bots 123 / 152 / 224 of 1,208 frags; the others 148 / 13 / 4 of 2,691.
- Why: a bot's burst is a median 9.0 m from where it threw (the others 20.6 m); within 3 m in 435 of 2,882 bot
  throws (21 of 951). And of the bursts 6 m or more away the bot was over 1 m closer at the blast in 1,375 of 1,948
  (378 of 898). Of 587 bot self-kills placed: 335 landed within 6 m, 252 farther, of which the bot walked in 236.
- Bots held the key a median 700 ms and so launched a median 1,200 ms after the press; the others 150 ms and 900.
- Walls: of 1,993 enemies within 10 m of a bot's burst, 952 had every point walled off by the rule above.
- Smokes: 819 and 771 throws, none by a bot (6 of 25,020 bot kits carry one). Traps: none placed by a bot (4 of
  25,020 kits carry one); the others placed 886 and 861, of which 432 and 389 fired.

## Against the bots

- The launch is taken at press + 1,100 ms (`arc.rs`, `live_grenades.rs`, `reflex.rs`); the client's is the release's
  + 500 ms. The release point is taken 0.3 m under the eye, about 0.55 m too low.
- The comments say worlds are 5 s apart and carry no grenade; they carry every grenade's body, so a landing can be
  seen instead of predicted.
- The threat's distance is taken from the chest (`grenade_threat.rs`); the client's is from the feet. The bots' own
  and mates' blast checks take no walls.
- The stand-in's blast (`tests/standin_grenade.rs`, `damage.rs`) hits other parts than the client's six, takes no
  stamina and one stopping hit.
- A spotted enemy is not kept known through a cloud (`smoke.rs`, `sense.rs`).
- The same as the client: 17 m/s along the view, the fuse from the press, the flight's constants, no team test, the
  trap's trigger and damage.

## Not read

The rest of the roll's error after the speed cap (P80 4.3 m: the capsule's own contacts; a sphere was flown); the
release point from the hand's bone (it is measured).
