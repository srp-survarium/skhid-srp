# Weapon hands: on the gun

Players reported "broken animations" on the ported guns (second-hand, 2026-11-09); a static audit
found the hands off the gun on 16 of them. Fixed by packs weapons-031@14, weapons-046@14 and
weapons-069@18, made by resource-porter's hands step (`src/weapons_026/hands.rs`; resource-porter
#46 the core, #47 the 0.46c2 guns, #48 the 0.69d0 ones over #45) and versioned over the live
packs by `scripts/weapon-hands/pack_hands.py` (only the hands clips and the pistols'
`user_animations` change).

## How 0.26g puts the hands and the gun

Read from the exe (names and ABI in `tools/re-kit/v026g`):

- A state's hands clip (`.../player/.../hands_only/<state>`, `animation_type` 1) is additive on
  the body clip of the gun's `user_animations` set (`stand_hud[0]` in the first view, `stand[0]`
  in the third; 0.26g's sets name `weapons/ak/player/stand/3rd_view/on_site/on_site_idle` for
  both, rifles and pistols alike). Per bone: the body's rotation first, then the clip's
  (`rotation = base * additive`), translations added. 0.26g's hands clips move only the arms and
  the `Weapon` bone.
- The gun hangs on the player's `Weapon` bone, a child of `Root` (not of a hand), turned half a
  turn about its own Y (`portable_interactive_object_core::update_bones_matrices` 0x2FF600,
  0x293AE0). The gun's clip moves its bones in that frame.
- Hand IK (`hand_to_weapon_ik_solver`, 0x392910..0x3931A0, console switch `ik_on_hands`, on):
  the clip's `left_hand_ik` / `right_hand_ik` event values set each hand's target
  (`activate_hand` 0x392D90: value/2 = 0 the hand's own place, 1 the gun's `*_hand_cont` bone,
  2..5 the model's `<Hand>_idle`, `_reload`, `_reload_pivot`, `_lobby` locator, a missing one the
  `_idle` one; odd values blend in 100 ms, even ones in 300 ms, by a fermi curve, 0x393A80).
  `process_hand` (0x3931A0) bends the arm and forearm onto the target and turns the hand as it.
  0.69d0's exe maps the values the same way (0x200564C50).

## Measuring it, proven on the stock guns

`vostok-port hand-contact --db <stock> [--db <patch>].. [--files <pack>/files].. --config
gameplay/items/weapons/wpn_<gun>.options [--stance <body clip>]` prints, per hands clip, each
hand's distance from the gun's contact bone of that hand (median and worst over the frames) as
0.26g composes them. On 0.26g's stance the stock guns' clips put both hands on the contact bones
within 0.1 mm in every frame (AK-12, SR-2M, Val, PPSh, TT-33, PM, APS, SVD, UMP45 and the others;
the AK-74 family's older shared clips, the RPK's and the AKMN's, are 2 to 9 cm off in the
reload, the IK's to cover). So are patch 19's 0.29a3 guns, the AKMSU, FAMAS, KS-23M and Fort-401.
The earlier probes read 6-47 cm on the stock guns because they hung the gun without the half
turn, composed the rotations the other way round, or measured against `*_hand_cont` where the
IK targets a locator.

## What was wrong

- **The later builds' hands clips are drawn on another stance.** On 0.31a2's, 0.46c2's and
  0.69d0's third-person stance (`base/player/3rd_view/stand/[pistol/]on_site_idle_long`) their
  guns' hands are on the contact bones within 0.1 mm; their first view plays them on a
  first-person stance and leaves the IK 1 to 26 cm to cover. Their rifles' first-person stance
  holds the gun where 0.26g's does (1 mm, 0.1 degree), so on 0.26g the rifles were as in the
  donors' first view, with the IK still to cover the gap.
- **0.26g's first view does not show that IK.** With 0.46c2's own hands the MP-443 showed no hands,
  only the gun (resource-porter faa703a, 2026-11-01), its idle points well within the arms' reach.
- **The pistols.** 0.46c2's first-person pistol stance holds the gun 22 cm further out than
  0.26g's (0.46 m from the head against 0.24 m, the rifles' place). Pack @8 gave the five pistols
  the live TT-33 hands, which put the gun where 0.26g's pistol clips do (0.43 m) while these
  guns' clips keep it on the `Weapon` bone: the gun 20 cm back, the hands 20 cm ahead of it, and
  the Glock's and Colt's hands clips 65 frames against their guns' 95.

## The fix

The port (`hands.rs`) now, for each hands clip it carries:

1. where the donor's first-person stance holds the gun elsewhere than 0.26g's (the pistols),
   re-bases the clip onto 0.26g's stance: the arms and the gun where the donor's first view has
   them, relative to the chest (0.26g's head stands on the chest as the donor's first-person
   head does, so the view is the donor's: the pistol 0.46 m out);
2. unless its hands are on the contact bones all through (1 mm), bakes the IK in: per frame each
   hand's target as the events set it and the engine blends it (a frame's length the state's:
   `reload_time`, `tactical_reload_time` for a `reload_tactic`, `show_time`, `hide_time`), the
   arm bent onto it as `process_hand` bends it. Where the IK runs it then has nothing to do;
   where it does not, the arms are as it would have put them.

A clip two guns play (0.46c2's pm hands: CZ 75, MP-443, Beretta 93R) ships as the first makes
it; the Beretta, which reloads faster, needs its reloads' blends elsewhere and plays its own
copies (`.../hands_only/reload_beretta93r`, `reload_empty_beretta93r`).

## Contact before and after (mm, left/right, median over the clip)

| gun | idle | reload | tactical / empty reload |
|---|---|---|---|
| OTs-14 | 5.5/64.1 -> 0/0 | 51.8/124.5 -> 0/0 | 54.1/131.6 -> 0/0 |
| SCAR-H | 11.6/29.1 -> 0/0 | 17.4/73.0 -> 0/0 | 18.7/72.9 -> 0/0 |
| VHS-2 | 8.9/65.1 -> 0/0 | 49.3/127.5 -> 0/0 | 49.2/131.0 -> 0/0 |
| FN Minimi | 20.3/32.4 -> 0.5/0 | 67.6/52.0 -> 0/0 | |
| MP5 | 18.4/24.2 -> 0/0 | 20.8/42.7 -> 0/0 | 22.5/42.9 -> 0/0 |
| SVU | 12.8/65.2 -> 0/0 | 42.4/195.2 -> 0/0 | |
| L85A2 | 6.9/42.3 -> 0/0 | 76.4/107.7 -> 43.6/0 (the left hand on the magazine's `_reload` point) | 76.0/114.6 -> 0.9/0 |
| Jackhammer | 6.9/42.3 -> 0/11.5 (the `_idle` point) | 52.3/78.8 -> 0.8/11.5 | |
| G3SG/1 | 23.9/24.0 -> 0/0 | 34.8/42.5 -> 0/0 | 28.1/42.6 -> 0/0 |
| Mauser M.712 | 6.8/80.6 -> 0/0 | 48.7/125.1 -> 0/0 | 57.2/125.1 -> 0/0 |
| PPSh Drum | 17.2/18.4 -> 0/0 | 21.6/37.9 -> 0/0 | 25.8/36.4 -> 0/0 |
| Glock 17 | 202.0/201.2 -> 0/0 | 339.3/261.4 -> 0/0 | 270.6/266.3 -> 0/0 |
| Colt M1911 | 202.0/201.2 -> 0/0 | 333.3/261.4 -> 0/0 | 269.6/266.3 -> 0/0 |
| CZ 75, MP-443 | 202.0/201.2 -> 1.4/1.2 | 203.2/201.5 -> 0/0 | 203.8/201.2 -> 0/0 |
| Beretta 93R | 202.0/201.2 -> 1.4/1.2 | 203.2/201.5 -> 0/0 | 203.8/201.2 -> 127.3/37.3 (its fast empty reload is mostly blends) |

The worst frames left are the IK's own blends (a hand moving from one target to the next over
100 or 300 ms) and the draws and holsters, where the clips take the hands off the gun.

## Not done, not seen

- Nobody has looked at it in the game. The open question is the third view and whether 0.26g's
  first view runs the hand IK at all (the code path reads as on; the MP-443 says otherwise). The
  baked clips are right either way.
- The 0.69 tactical-reload port (resource-porter `feat/tactical-clips-weapon-turn`) can measure
  its clips with `hand-contact` (or `hands::contact`) on 0.26g's stance; a clip off the gun there
  takes the same `hands::bake_ik` (and `hands::rebase` if its donor's first-person stance holds
  the gun elsewhere).
