# Weapon attachments: the grips (the lane's note)

**Version scope:** client 0.26g, 0.69d0 as the reference for content. The system, the modules
table and the client's runtime are `attachments.md`'s; this note is what the grips lane adds: the
thirteen grip items, the hand's rules and checks, the carriers' state. Tags: PROVEN (read in the
data or the code named), SEEN (in a game, the picture named), COMPUTED (worked out from data, not
seen), OPEN.

## The items

0.69d0 has thirteen foregrips; all thirteen are items here (`scripts/weapon-modules/ids.tsv`,
2659-2671), each fitted to the guns 0.69d0 fits it to: 318 gun and grip pairs over 58 gun items,
232 of them through a carrier that comes with the grip (a tactical forend in place of the gun's
own, a rail bracket, the barrel clamp). What stands where is the modules table's
(`attachments.md`); an item is only what a player buys and what changes the gun's numbers.

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
  `LobbyMenu_AmmoIcons` column 8, rows 1 to 13.
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
| PKP's own (2668), OTs-14's own (2667), SR-3M's own (2670) and its premium's (2669) | 1 each | aimed recoil -8 %, hip recoil +3 % | recoil -6 % |

- With the penalties dropped, PTK and AFG-1 are one grip in numbers, Troy is PTK with a penalty,
  RK-2 and RIS are MIKI with one. What a player picks is mostly what the gun takes.
- These change what a match computes: the branch's one `next` row of `simulation_changes.tsv`.
- Later, each with its own reader (`attachments.md`, "Stats for now"): aimed and hip recoil, the
  first three shots, the aimed movement speed.

## The hand

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
  (`weapon_modules::Modules::check`), which runs `resolve` over all the weapon would hold: on the
  P90 a device of the barrel's own thread and a grip, whose forend sits on that thread.
