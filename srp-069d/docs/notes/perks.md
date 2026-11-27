# Perks (skill tree) in 0.69d: audit

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

Status words: PROVEN = exe address, data file or lobby movie; LIKELY = inferred; OPEN = not known.

## What a perk is in 0.69d

The only per-player perk system is the skill tree (`gameplay/skills/skills_tree.options`, `data/skills_069d.json`):
69 skills in 5 branches (1 shooting, 2 physical, 3 survival, 4 engineering, 5 anomaly), ids `B R C`, 1..5 levels each,
a tier row unlocks at 5 points per row in its branch. Every level carries modifiers named after the exe's 138-name
table (0x200995E10). The "specifics" of the lobby code are pocket consumables, not perks.

"Terminator" (`st_use_two_double_handed_weapons_*`: both weapons may be two-handed) and "Deep Pockets"
(`st_convert_weapon_slot_to_pocket_*`) still have client texts and the exe still reads their modifiers, but no 0.69d
skill carries `use_two_double_handed_weapons_modifier` or `disable_second_weapon_modifier` (PROVEN by the tree and
game.options): neither can be learned in 0.69d.

## How skills take effect

- Lobby: learn `47 00`, reset `47 01`, list `8C`, reset prices `8D` (`skills.rs`, `game_data::skills`). The lobby window
  reads three modifiers itself (`fill_skills` 0x2001C0D90, learned skill, level 1): the Terminator flag, the
  second-weapon flag and the "+1 medkit" display flag; ammo bags (base 3 from `default.player`), pocket count and depth
  are computed by the client (0x20031F930).
- Match: every profile of the player record carries the learned levels summed per modifier id (18-byte mask + one f32 per
  set bit, `options::put_mods`; reader 0x200125680 adds them into profile +0x100). The exe's own code applies them
  (0x20031E160 and its wrappers), in the client and in the world host alike. This field is separate from the weapon
  module pairs (`put_n4`, `match-modifiers.md`).
- Economy: "Artifact Collector" (533) makes the world count `artefact_collection` events; the lobby pays them by the
  `events.options` row (25 silver each) like every event of the world's matrix.

## Weapon slots

The lobby movie (`LocalPaperDoll`) lets slots 7 and 10 hold at most one two-handed weapon (`oneHanded = !double_handed`
of game.options; the 25 one-handed weapons are the category 17 pistols, `data/weapon_hands_069d.json`), never the same
weapon twice, and closes slot 10 when `enable_second_weapon(0)`. The server checks the result of every `45 00` that
changes slot 7 or 10 the same way (`game_data::perks`) and undoes a refused request; the two flags come from the learned
skills, so the rule follows the data (Terminator unavailable in 0.69d). A rifle may sit in slot 10 next to a pistol in
slot 7.

## Status

| where | skills | status |
|---|---|---|
| match (all 69) | every learned skill reaches the match options | WORKING (delivery PROVEN; exe read site found for 56, OPEN for 13) |
| lobby | 152 Ammunition (4 ammo bags), 351 Additional Medicine (+1 shown), 353 Supply Pocket (+1 pocket) | WORKING: the server sets no limit they would raise; pockets themselves are behind `pockets_on` (default off) |
| economy | 533 Artifact Collector | WORKING through the world's event matrix (online matches with a world host) |
| weapon slot rule | Terminator / second-weapon flags | was MISSING on the server, fixed |

Open: the reset price per point (1000 silver / 5 gold, no game file has it); a category check of slots 7 / 10 against
the movie's own slot table (event weapons of category 40); whether the lobby's own block holds the learned skills (an
in-game check: ammo bags and pockets shown after learning 152 / 353).
