# Death poses

The owner's rule and what it changes. Data: the weapons' animation lists
`gameplay/items/weapons/common/rifles.user_animations` and `pistols_new.user_animations`
(`death`, `death_hud`); the change ships as a resources.db patch
(`scripts/content-edit/death-poses-1.json`, `remove_item`).

- **Report (the owner, 2026-10-21, testing the death camera, exe patch 46,
  [death-camera.md](death-camera.md)):** "we need to remove one death pose which makes head
  look into arms"; "it is the only one broken". In it the corpse lies face down with its arms
  under the face, so the eyes' look cone is full of its own arms.
- **The lists:** a weapon's `user_animations` names its death animations (`death`, the body;
  `death_hud`, the first person, the same files in the same order). 107 weapon configs use
  `rifles` (4: ak `death/on_site_back`, `on_site_fwd`, `on_site_left`, `on_site_right`),
  8 use `pistols_new` (2: tt33 `death/on_site_back`, `on_site_left`), the battery carrier
  `victory_items/default` (1: key_objects `on_site_death_fwd`). No Skhid patch changes them.
- **The pick (PROVEN, 0x338330, `player_logic_dead_state`'s enter):** random, not by the hit:
  the weapon's animation selector steps its own random (`m_random`, `x·0x8088405 + 1`) and
  takes `index = (x · count) >> 32`, count = the list's length (`death` and `death_hud` are
  read into one record, +590h and +A20h, the count at +594h); `death[index]` plays
  on the body and `death_hud[index]` in the first person. The index (`m_animation_index`) and
  the random's seed are serialized with the player's state: it is the lockstep simulation's,
  the same on every client. No ragdoll (the exe has none): the corpse keeps the animation's
  last frame.
- **Removing an item:** each death still steps the random once; the index is then uniform
  over the shorter list, so the pick is among the rest, each the more often. A given random
  value maps to another animation (the later items move down), so a recording of an earlier
  revision shows other poses: a `next` row in `simulation_changes.tsv`.
- **The broken pose:** one final pose, reached by three animations: rifles' `on_site_back`
  and `on_site_fwd` (their last frames differ by 7 cm at the arms, 36 cm at the legs, in the
  head's frame) and pistols' `on_site_back` (the rifles' `on_site_back` last frame exactly).
  The arms cover 16 % of the look cone (70° either side, 60° up, 40° down of the head's
  forward), 24-25 % of its middle (30° either side, 20° up and down), the hands 20-30° from
  the forward at 30-40 cm. Every other pose: 0 % arms (the right one sees 1.7 % of its
  body at the cone's edge).
- **Change:** those three items out of `death` and `death_hud` both: rifles keep `on_site_left`
  and `on_site_right`, pistols `on_site_left`; the battery carrier's list is unchanged.
- **Not changed:** the animations themselves, the death camera, the respawn time, srp
  (it reads none of these files).
- **Fewer poses now (the owner, 2026-10-21: "we can think later what to do with those"):**
  rifles have two death poses left, pistols one, all face up. Options for later: the face-down
  pose back with its last frame fixed (the arm moved away from the face), death clips from other
  versions, or the death camera treating that pose specially.
