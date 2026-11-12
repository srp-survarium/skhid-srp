# The player's damage model in the 0.26g client: pools, hits, wounds

Read in the executable (virtual addresses, image base 0x10000; names in `tools/re-kit/v026g`) and the game's
`human_hit_params`. PROVEN where an address is given, unless said otherwise. Counted on saved worlds of recorded
matches read with the repository's own parser: 59 matches against bots and 37 between players. The worlds are the
live game's (some client patches change a term: said where it matters). See also `damage_calculator.md`,
`armour-params.md`, `stagger_and_legs.md`, `fall_damage.md`, `weapon_fire.md`.

## The 20 parts: 8 pools and 12 hit boxes

| # | part | pool | a hit on it goes to | regeneration after 8 s | at a threshold |
|---|---|---|---|---|---|
| 0 | body | 1.0, lethal | | 0.03 / s up to 50 % | 0: death |
| 1, 2 | back, front | hit box | body x 1, pain x 1 | | |
| 3 | head | 0.4, lethal | | 0.03 / s up to 50 % | half: concussion; 0: death |
| 4, 5 | brain, face | hit box | head x 1, pain x 1 | | |
| 6, 7 | right leg, left leg | 0.5 | | 0.03 / s up to 20 % | 20 %: speed -0.1; 0: speed -0.2 and `leg_damage` |
| 8, 9 | hips | hit box | leg x 1, pain x 0.9 | | |
| 10, 11 | feet | hit box | leg x 1, pain x 0.8 | | |
| 12, 13 | arms | hit box | hand x 1, pain x 0.8 | | |
| 14, 15 | forearms | hit box | hand x 1, pain x 1 | | |
| 16, 17 | right hand, left hand | 0.5 | | 0.03 / s up to 20 % | 0: `hand_damage` |
| 18 | pain | 1.0, **lethal** | | 0.1 / s up to 100 % | 0: death |
| 19 | infection | 1.0, lethal | (radiation and toxic damage of every box) | none | full: poisoning; 0: death |

**Every hit on any box also takes from the pain pool, and an empty pain pool kills.** Counted: the pools and the
pain follow this routing in 8,968 of 9,038 bullets on bots whose boxes are seen; pain alone (the body and the head
not empty) killed 3,491 of 5,994 bots in the matches against bots and 908 of 1,701 bodies in the matches between
players.

## A hit

`base_player::hit_impl` 0x2D9B90 -> `damage_model::hit_body_part` 0x2E5080 -> `hit_by_type` 0x329FF0:

- nothing for a dead body or a full invulnerability (a spawn's: its time and the point's protection, then falling);
- **the damage times (1 - clamp(armour - penetration, 0, 1))** (0x32A058), the armour being the sum of the worn
  items' values for that box and hit type (0x33F2C0); the item's condition is not read there;
- **a bullet counts once a part, by its largest share**: a part keeps one record a bullet and takes only what goes
  over it (0x32A13F..0x32A215);
- the shares routed to the pools (the table) go in with no penetration (0x33FA80).
- Counted: each pool's loss is its new hit records (the body 7,019 of 7,034; the pain 10,197 of 10,371, the others
  falls, which leave no record). A whole hit foretold from a bullet a world held in flight agrees to the number in
  604 of 2,019 by this rule alone, and in 1,876 of 1,955 with the bullet's own losses (`bullet_flight.md`): the
  fall-off goes on to the hit, and a bullet goes through every body: the first box takes the bullet's damage, every
  later box the weapon's least damage and pierce.

## Wounds and what they gate

- Affects come only from thresholds (one call site, 0x32AB90): `leg_damage` and `hand_damage` last 8 s from the hit
  that empties the pool, concussion and poisoning 10 s. **No threshold starts a bleeding**: 0.26g has none.
- **A wound's end waits for the part's regeneration to begin**: the tick returns before it expires a wound while
  the time is within 8 s of the part's last damaging hit (0x32A551). So a wound ends at the later of its saved end
  and the last hit + 8 s. Counted: 77 leg, 62 hand and 17 head wounds seen past their saved end, all within 8 s of
  the last hit, none beyond.
- The readers of the two wound counts (the damage model's +388h legs, +389h hands), all of them:

| count | gate | where | counted |
|---|---|---|---|
| a leg | no sprint, no jump | 0x2DBF58, 0x2FC254 | no sprint flag in 3,265 bot worlds; a jump flag in 10, in the second of the break |
| both legs | the crouch forced | 0x2FBE05 | crouched in 931 of 939 |
| a hand | no battery use | 0x300739 | |
| both hands | no aim; no grenade; a two-handed weapon neither fires nor reloads | 0x2FC2DE, 0x2E40EC, 0x2E381A | aim refused in 734 of 751 (the others within 1 s of the hit) |
| hands | the cone wider by 10 x a modifier | 0x3317C0 | 3,062 of 3,062 bot worlds by the whole expression (a glove's default modification on modifier 0 gives the factor 0.92 of some) |

- Nothing reads a wound for the reload's time, the recoil, the sway or carrying.
- Regeneration at the table's rates after exactly 8 s: the body 2,297 of 2,700 rising pools (the others 1.12 or
  1.15 times faster: a worn item's default modification on the regeneration modifier, 2,199 of 2,275 such bot rows;
  or with an artefact).
- Death: the death affect a lethal pool starts at 0 (`on_affect(kind, action, time)` 0x2DA480, its only setter). A
  drug in use can lift the pool over 0 again in the same step, after the bullet (23 dead bodies show a pool over 0).
- Concussion, poisoning and the pain's thresholds gate nothing: their only readers are the damage sounds (0x23EFD0,
  0x23F610) and the screen's effects (0x32A7A0). A painkiller in use takes a tenth of the pain's damage
  (`damage_protection`, 0x33DB30).

## Against the bots

- The bots' heal weights the pain least ("hardly matters") and their health and retreat read the body pool; most
  deaths are the pain's.
- `legs.rs` ends a leg wound at its saved end; the client holds 10 % of them longer (88 of 856, 16 by over 3 s).
- The bots' estimate of what a hit took is the weapon's nominal damage off the body: a torso hit took under half
  of it in 791 of 1,088, and 842 hits were not on the torso.
- Broken hands are not known to the bots: with both broken and a two-handed weapon they held FIRE or AIM in 160 of
  670 worlds, and the weapon fired in 3 of 541 windows.

## Not read

The armour of a bullet that meets several boxes; fall damage's entry (cited from `fall_damage.md`); a pain factor
of 0.875 on some played bodies' hits (in none of the stock data: the live client's own patches are to be read).
