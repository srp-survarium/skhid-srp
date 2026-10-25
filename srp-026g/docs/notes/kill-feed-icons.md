# Kill feed: wall, ricochet and pain icons (patch 67)

The rules the code and the tests follow. Code: `client-patches/kill_feed_026g.asm` and
`KILL_FEED_*` in `client-patches/patch_026g.py` (README patch 67, `client_patch.md` section 3.55),
the hud movie's `survarium/controls/CombatLogItem.as` and `flash/hud/bitmaps/`; the check:
`client-patches/check_kill_feed_026g.py`.

- **The owner, 2026-10-25:** "when a character is killed from pain, the character should get
  <weapon> <white painkiller> icon"; "when through a wall: <weapon> <wall shot> icon (available in
  last patches)"; then yes to retail's ricochet icon.
- **The row:** killer, weapon, then the wall or the ricochet icon, the pain icon, the cause icon
  (headshot, skull), victim. Each only when it applies; 0.69d0's order (its description icons,
  penetration then ricochet, come before the cause icon; the ricochet takes the wall's place).
- **Through a wall and by a ricochet:** retail 0.69d0's rules for its `penetration_icon` and
  `ricochet_icon` (its exe, proven), for a kill by a bullet and someone else (not the skull row:
  no killer or the victim himself):
  - ricochet: the killing bullet reflected before the hit (bullet +88h, its reflections, not 0);
  - else wall: it changed course before the hit (bullet +84h, its trajectory's segments past the
    first, one for each pierce and each reflection, not 0): it pierced something;
  - the ricochet wins: a bullet that ricocheted, before or after a pierce, shows the ricochet only;
  - anything the bullet pierced counts for the wall: a wall, a prop, another player's body
    (0.69d0's try_pierce adds a segment after any object or geometry it goes through; no material
    check, no threshold);
  - a kill with no bullet (a grenade, an anomaly, a fall) or a melee hit shows neither.
- **By pain:** at the death the victim's pain pool is empty and its body and head pools are not
  (the pain pool killed; when body or head is empty too, they killed).
  - The owner, 2026-10-25: "if someone dies from pain and damage at the same time, they die from
    damage, not pain": one hit that empties pain and body (or pain and head) together shows no
    pain icon.
  - A headshot that killed through pain (the head pool left) shows both the pain and the headshot
    icons. Any killer, any weapon, with the wall or the ricochet too; a suicide row (no killer, a
    fall) shows it too.
- **The icons:**
  - wall: 0.69d0's kill log icon `flash_movies/textures/ui_icons/log/icon_penetration.png`
    (40x30), as it is;
  - ricochet: 0.69d0's `flash_movies/textures/ui_icons/log/icon_ricochet.png` (40x40), as it is;
  - pain: a lightning bolt for pain shock (the owner's pick, 2026-10-25, of 15 candidates: neither
    0.26g nor retail has a pain or shock icon; the Anesthetic's ampoule read as a bullet), drawn,
    white, 23 px high, a dark shadow one pixel down and right as the hud's own cause icons;
  - each sits 5 px after the one before, centred on the weapon's icon (never above the row);
  - made by `scripts/kill-feed-icons/make_icons.py`; the build adds each `bitmaps/<class>.png`
    to the movie (`tools/actionscript/flash_tree.py`).
- **Every game alike:** the flags come from the simulation's own state (the bullet, the pools) in
  every game's lockstep run, in the kill event itself; a replay shows them too. Visual only:
  nothing a match computes changes, no `simulation_changes` row.
- **Release:** the exe patch (the default build) and the hud movie together; an older exe with
  the new movie shows rows without the three icons.
