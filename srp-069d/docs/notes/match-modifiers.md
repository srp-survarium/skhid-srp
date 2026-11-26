# Modifiers in a match: where they enter, how to add one (0.69d)

Part of **Skhid (Схід)**, a private server for Survarium 0.69d. The `srp` program is the Skhid server; the operator guide is [guide-server.md](../guide-server.md).

A match (the client's local match and the world host alike) builds every player from the match options blob
(`crates/local-match/src/options.rs`). The blob is the only way a modifier reaches the match: the client and the world
host run the exe's own cook on it, nothing in the match asks the lobby. Exe VAs: base 0x200000000. PROVEN = read in
the exe or data; LIKELY = inferred.

## 1. The four carriers

| carrier | what it holds | writer | read / applied by the exe |
|---|---|---|---|
| profile modifier block | per player, every profile: 18-byte mask (bit i = modifier id i of the 138 names, `gd.matches.modifier_names()`), then one f32 per set bit | `options::put_mods` from `MatchData::skill_modifiers(skills)` + `HumanLoadout::extra_modifiers` | reader 0x200125680 (PROVEN); e.g. modifier 0x79 medkits at spawn (0x2002D8431, `start_objects.rs`) |
| item n1 list | per item instance: (group, grade) pairs of the data's `modifications` table, at most 8 | `options::human_item` from `HumanLoadout::upgrades` (= `Profile::mods`) | item cook `item_config_cook::translate_query` 0x200335120 (item +0x10, 0x200335480 loop) (PROVEN) |
| module pairs (n4 element) | per installed module: (group, grade) pairs, at most 4 | `options::put_n4` with `ModuleData::instance_mods` | same cook, element +0x30 of each n4 element (0x2003356E0..0x200335803), applied to the WEAPON's config (PROVEN) |
| decals (n3) | per armour piece: camouflage / decal entries | `options::human_item` from `HumanLoadout::decals` | same cook, the decal row's `parameters.modification` (0x20033559E) (PROVEN carrier) |

The cook also applies a vector of the item's dictionary row (0x200123410 result, +0 .. +8; LIKELY the row's
`default_modifications`) before the record's own lists. It never reads `modules_additional_infos`
(`premium_modifiers` / `override_modifiers`, dict+0x32F8): those reach the match only as pairs in the records.

Each (group, grade) is looked up in the `modifications` table (dict+0x2A30, 0x48-byte rows) and its `modifiers`
(`path`, `type`, `value`) are applied to the item config: type 0 sets, type 1 / 2 add or scale (e.g. group 520 grade 7
= `parameters/magazine_capacity` set to 40).

## 2. Weapon modules

* A module instance's pairs: its stored list (`Profile::mods[uid]`), then its type's `override_modifiers` whose group
  is not among them (`ModuleData::instance_mods`). Both the module's own record (n1) and the weapon's n4 element carry
  that list, in 87 / 89 / `83 45 0F` and in the match options. The client copies the module record's list into the
  element itself when the player attaches an owned module (0x2002848A0..0x200284977), so the two always agree.
* A module bought in the workshop (60 01 action 3 or 4) is stored with `ModuleData::bought_mods`: `premium_modifiers`,
  the rarity pair (312, 4) and `override_modifiers`: the list the client gives a module bought in the same save
  (0x2002848B5 -> 0x2002DFD30; it does not tell the two buy types apart).
* Default modules (n4 uid 0xFFFFFFFF) carry no pairs, as in the client's own fill (0x20031F0A8).
* Data: `module_mods` of `data/game_tables_069d.json` (every `modules_additional_infos` row; extractor
  `crates/data-extract`).

## 3. Adding a modifier

* Match-wide per player (skills, perks, boosters, services): add `modifier id -> value` to
  `HumanLoadout::extra_modifiers` (summed with the skill values in the same block). Filled today in
  `servers/srp/src/matches.rs` `human_loadout` (per-match boosters, `lobby_server::boosters::match_modifier_ids`) and
  `servers/srp/src/online.rs` `online_players` (service boosters). Learned skills go through
  `MatchData::skill_modifiers`; bots use the same with their `ai.skills`.
* Per item (weapon or armour upgrades): store the pair in `Profile::mods[item uid]`; the lobby records and the options
  pick it up.
* Per module: store the pair in `Profile::mods[module uid]` (at most 4 with the override pairs), or for a whole module
  type extend `module_mods`.
* Reward bonuses (experience, silver, reputation, spare parts) are not match modifiers: the lobby pays them
  (`boosters::worn_bonus`, `reward_percent_parts`, `parts_percent`), see `decals.md` section 4.
