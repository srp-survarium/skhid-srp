# Weapon attachments: the grips (the lane's note)

**Version scope:** client 0.26g, 0.69d0 as the reference for content. The system, the modules
table and the client's runtime are `attachments.md`'s; this note is what the grips lane adds: the
ten grip items, the three grips that stay a gun's own part, the hand's rules and checks, the
carriers' state. Tags: PROVEN (read in the
data or the code named), SEEN (in a game, the picture named), COMPUTED (worked out from data, not
seen), OPEN.

## The items

0.69d0 has thirteen foregrips (`scripts/weapon-modules/ids.tsv`, 2659-2671). Ten are items
here, each fitted to the guns 0.69d0 fits it to: 315 gun and grip pairs over 55 gun items, 232 of
them through a carrier that comes with the grip (a tactical forend in place of the gun's own, a
rail bracket, the barrel clamp). What stands where is the modules table's (`attachments.md`); an
item is only what a player buys and what changes the gun's numbers.

- **Three stay their guns' own part**, always on, with no item and no stat (the owner's call,
  2026-11-21; `grips.OWN_PARTS`): the OTs-14's (2667), the SR-3M's (2670) and the premium
  SR-3M's (2669). Today's gun has that grip in its mesh and its left hand holds it. PROVEN in the
  data: the live gun's `LeftHand_idle` locator is the place on that grip, equal to 0.69d0's
  foregrip module's hand locator through the gun's grip point (0.0 mm on the OTs-14, 0.7 mm on the
  SR-3M), while 0.69d0's bare gun holds elsewhere (12.5 cm further back on the SR-3M). The remodel
  keeps them in both bodies; their ids stay theirs, unused.
- The other three own grips are not on today's guns (the Fort-401's, the Mauser M.712's, the
  PKP's: the live gun's hand is 30, 14 and 36 cm from the grip's hand place and the live mesh has
  no such part), so they are items as 0.69d0 has them.

- **Made by** `scripts/weapon-grips/make_pack.py` into the pack `weapon-grips` from 0.69d0's own
  rows (`donor.sh`: its `game.options`, its names, its pictures; pinned by `donor.sha256`) and
  `ids.tsv`. Nothing is typed per grip. `grips.tsv` there is the result, a row a grip.
- **The config** (`gameplay/items/attachments/grip/<item>.options`) has a shop scope item's shape
  (category 22, the client's kind of a weapon's fitted item). Its modification:
  - a type 3 add to the weapon's `data`: `srp_grip` = the item's dict id. That is all the game
    reads to draw the grip, its carrier and the left hand;
  - type 2 on the six kick bounds (`recoil/min|max_top|left|right_recoil`) and, where the grip has
    it, on `parameters/aim_time`: the keys the stock recoil and aim-time rolls change
    (`weapon-rolls.md`). They add up with the weapon's own rolls inside the key.
- **The price** is one for the family, 6,000 silver (0.69d0 prices every module the same). The
  shop sells an item some gun takes as a module (`prices.rs`, the core's rule).
- **The icon** is 0.69d0's own picture of the grip in the stock icons' frame, in
  `LobbyMenu_AmmoIcons` column 8, rows 1 to 10.
- **The texts** are under 0.69d0's own keys (`st_<module>_name`, `_desc`) in the eleven tables:
  the name 0.69d0's own where it ships the language (English, Russian, Ukrainian, German,
  Polish), `names.tsv`'s in the other six; the description the grip's stats in two words, written
  by `texts.py` from the same numbers (never typed).

## The stats

0.69d0 rolls two modifier groups on a module, an effect and a penalty; a bought module has both at
grade 7. A 0.26g weapon reads neither aimed, hip nor first-shots recoil, and no aimed movement
speed, so by the one rule of `grips.py` (the owner, 2026-11-20: what needs an exe reader is
simplified for now):

- the effect, a conditional recoil, becomes plain recoil at a share: aimed x 0.75, hip x 0.25,
  the first three shots x 0.25;
- a penalty on the aim time is kept; a penalty on a key the weapon does not read is dropped.

| Grip | Guns | 0.69d0 at its bought grade | Here |
|---|---|---|---|
| FAB-Defense PTK (2661) | 44 | aimed recoil -8 %, aimed movement -0.75 % | recoil -6 % |
| AFG-1 (2665) | 35 | aimed recoil -8 %, hip recoil +3 % | recoil -6 % |
| Troy Modular (2671) | 53 | aimed recoil -8 %, aim time +1.5 % | recoil -6 %, aim time +1.5 % |
| FSG2 (2664) | 46 | hip recoil -8 %, aimed recoil +1.5 % | recoil -2 % |
| FAB-Defense MIKI (2660) | 32 | first three shots -16 %, aimed movement -0.75 % | recoil -4 % |
| Zenit RK-2 (2659) | 49 | first three shots -16 %, aim time +1.5 % | recoil -4 %, aim time +1.5 % |
| G&P RIS (2663) | 53 | first three shots -16 %, aim time +1.5 % | recoil -4 %, aim time +1.5 % |
| Fort-401's own (2662), Mauser M.712's own (2666) | 1 each | aimed recoil -8 %, aimed movement -0.75 % | recoil -6 % |
| PKP's own (2668) | 1 | aimed recoil -8 %, hip recoil +3 % | recoil -6 % |

- With the penalties dropped, PTK and AFG-1 are one grip in numbers, Troy is PTK with a penalty,
  RK-2 and RIS are MIKI with one. What a player picks is mostly what the gun takes.
- These change what a match computes: the branch's one `next` row of `simulation_changes.tsv`.
- Later, each with its own reader (`attachments.md`, "Stats for now"): aimed and hip recoil, the
  first three shots, the aimed movement speed.

## The hand

- **Where the hand's locators stand on a grip.** COMPUTED from 0.69d0's own module files over the
  pack's meshes: each grip's sixteen hand locators laid over its mesh in all 24 axis
  orientations. The one the table's rule gives (the hand's record without the half turn a part's
  frame has, `attachments.md`) is the best of the 24 for every grip that is not symmetric, and
  puts the finger joints a finger's thickness from the surface on twelve of the thirteen
  (median 12 to 24 mm). The thirteenth is **the PKP's own**: 139 mm away by that rule. It is the
  one grip fit of the 318 whose frame is turned on its gun (a quarter turn: the PKP's grip rail
  is on its side; the others are turned under a degree), and its hand lies on it (20 mm) when
  placed at the grip's position in the gun's axes, not turned with the grip. So a grip's hand
  locators read as: at the grip, in the gun's orientation. The core's `Hand::new` places them so
  (e17ace688 on): SEEN, the PKP's hand at its grip (below); 0.69d0's own code for these locators
  is not read.
- **The finger joints are the hand's own.** COMPUTED over the built table: the finger joints of
  all thirteen grips' locators are one set (0.02 micrometres between grips), and that set is
  0.26g's hand skeleton, bone for bone (3.4 micrometres at most against the bind pose of 0.26g's
  hand). 0.69d0's hand is 0.26g's. The guns' own finger locators are the same joints rounded to a
  millimetre (0.4 mm median, 1.3 mm at most, the SPAS-12 5.8 mm). So a grip's finger matrices go
  in whole, on every gun (the core's `Hand::fingers`), and no finger can stretch.
- **The UMP45's left hand** never went to its idle hand locator: its show clip sends it to the
  gun's `left_hand_cont` bone and nothing sends it on, so a grip, which moves the idle locator's
  record, would have left the hand in the air. 0.69d0's own clips of the same names send it to the
  idle locator at the start of idle and shoot, and to the bone and back in the reload.
  `weapon-grips-hand-events-1.json` (`scripts/weapon-grips/hand_events.py`) gives the three 0.26g
  clips 0.69d0's own `left_hand_ik` channel and nothing else: the right hand and the fingers stay.
  COMPUTED: in the UMP45's own clips that bone sits on the idle locator (0.3 mm, 0.1 degrees, at
  idle, through the shot and at the reload's two ends), so without a grip the hand is where it
  was.
- The other five guns whose hand does not rest on the idle locator (Rem 870, TOZ-66, Magnum,
  SV-98, PPSh) take no grip in 0.69d0: nothing to do.
- The twelve guns with `double_handed` 0 release the left hand in sprint and jump as ever; of them
  only the Mauser M.712 takes a grip (its own).

## The carriers and the refused fits

- Thirteen tactical forends, three rail brackets and the barrel clamp are rows of the table and
  models of the pack `weapon-modules` (the content side's build); they are never items.
- **Eight fits stay refused** (9A-91: RIS, Troy, RK-2; OTs-14: the same three; SR-3M: Troy, RK-2).
  PROVEN in 0.69d0's data: on these guns the clamp goes on a slot only a fitted suppressor
  brings (`vsk_94_suppressor`, `ots144a_groza_lock_suppressor`, `sr3m_suppressor`); without it
  0.69d0 has no place for them either, and suppressors are out by the owner's rule.
- A pair of kinds that does not go together is refused by the servers' one check
  (`weapon_modules::Modules::check`), which runs `resolve` over all the weapon would hold.
  COMPUTED on the built table (pack `weapon-modules@1`), every grip fit with every muzzle device
  and every scope the same gun takes: each of the 318 grip fits resolves alone; 4,496 grip and
  scope pairs go together, none is refused; of the grip and muzzle device pairs 2,224 go together
  and 66 are refused, all of two kinds:
  - **the P90**, 12 pairs: its two devices of the barrel's own thread (2601, 2625) with any of
    its six grips, whose ACM forend sits on that thread (`Taken`). The devices of the forend's
    thread go with a grip;
  - **the VSK-94 and its premium item**, 54 pairs: any of its nine muzzle devices with any of its
    three grips (`Unseated`). The grip's clamp stands on the built-in suppressor, and a device
    takes the suppressor's place.

## Seen in a game (2026-11-21)

Headless clients on a local server, each gun wearing its grip as a real kit-slot-5 record (the
grips lane's rig, `private/local-clients-attachments-grips`); pictures in
`private/renders/attachments-grips/<tag>-*.png`, the first sample with its computed pictures beside
it in `milestone-1-sample.png`. Quick content of 22 guns (`weapon-modules-quick`, the porter's
`--only`) with `weapon-grips@1` and the UMP45's events: table version 2 on exe e17ace688, then
version 3 on c8b28fb43. Every client's log has the table's line and none of a part not asked for,
not loaded or with fewer bones; a drawn grip means its record survived the account's load.

- **Every grip and every carrier is drawn**, on 22 guns: the ten grips, the thirteen tactical
  forends (in place of the gun's own: the AK-74M's Midwest, the M4A1's, the AKMSU's Zenit B-11,
  the AK-74U's Redheat, the L85A2's, the Valmet's long one, the SKS's...), the three rail brackets
  and the barrel clamp (on the VAL's integral suppressor, the RK-2 hanging from it). A grip is
  best seen in the reload, when the hand leaves it; the AFG-1 is too low to make out in the first
  view.
- **The left hand goes to the grip** in the first view: lower and under the forend against the
  same gun without a grip (AK-74M, SKS, UMP45, FNC compared with the plain gun), and in the lobby
  closed round the grip under the forend, where the lobby's random pose holds the gun with both
  hands. Fingers through a part: none seen; at 1280x720 the hand hides most of the grip, so the
  fingers' wrap is judged from the lobby crops and the hand's outline, not a close-up.
- **The FN FNC's** flat forearm from the left is the gun's own hold (the plain FNC has it).
- **The UMP45** (its events): plain, the hand wraps the forend from the side; with the Troy, it
  sits under the front rail at the grip. That plain is main's own place is COMPUTED (0.3 mm).
- **The PKP's own grip**: before `Hand::new` (c81ada3df) the hand is high in the air, off any part;
  with it (e17ace688, c8b28fb43) a lattice arm runs out to the gun's left from its side rail and
  the hand is closed round the handle at its end, as the computed "gun's axes" picture has it. In
  the lobby the arm points away from the camera.
- **The Mauser M.712's own grip**: the hand goes to a vertical grip at the pistol's front (seen in
  the reload, the hand away). Aimed, the plain Mauser's two arms already cover much of the view;
  with the grip the left forearm sits higher and covers more of its right half.
- **The shop and the Attachments window** (c8b28fb43, content with the grips' compatibility rows
  for the 22 guns, a local test only): under Upgrades, found by name, "FAB-Defense PTK foregrip",
  6 000 silver, 0.69d0's picture; its card "Recoil -6%. Can be installed on: AK-74M, AKS-74U, SKS,
  ..." with the loadout's gun highlighted. Bought ("Purchase completed FAB-Defense MIKI foregrip",
  30 000 to 24 000 silver), dragged into the window's grip slot and confirmed: the account's gun
  record became `9100:2660:0:5` (the MIKI in kit slot 5) and the PTK went back to the inventory;
  the window's gun and the lobby's character show the MIKI under the forend. The window shows the
  Optics slot and the grip slot; no muzzle device slot was shown on that gun.
- **Not looked at**: the third view in a match, sprint and crouch with a grip, a second viewer, the
  belt-fed guns' reload (the PKP's and the Fort-401's did not start in these sessions).

## State (2026-11-21)

Done, and checked without a game:

- the pack `weapon-grips@1` (ten items: configs, dictionary rows, icons), the items' rows of
  `items.tsv`, their texts in the eleven tables, `weapon-grips-hand-events-1.json` (the UMP45);
  `tools/check weapon-grips` (the pack built anew has the manifest's hash; `grips.tsv`, the
  items' rows and the texts are what the sources make), `content-edit` and `localization` pass;
- `release/build-content` (quick) with the pack and the UMP45's edit list, on table versions 2 and 3;
- everything tagged COMPUTED above: the finger joints, the hand on twelve grips' meshes and off
  the PKP's, the UMP45's bone on its idle locator, the three own-part grips' hand places, the
  refused pairs on the built table.

Not done:

1. The lobby's words for a refused pair (the P90's and the VSK-94's 66 pairs): not tried in a game.
2. The compatibility rows' manifest row (`weapon-modules-fits-1.json`, the lead's) goes in when the
   lanes merge; the shop above used a local copy cut to the grips.
3. `release/build-content --full` with the grips' pack (the branch's one full build is the
   integration's).
4. The stats' later readers (aimed and hip recoil, the first three shots, aimed movement speed).
