# Kill feed: wall and pain icons (patch 67)

The rules the code and the tests follow. Code: `client-patches/kill_feed_026g.asm` and
`KILL_FEED_*` in `client-patches/patch_026g.py` (README patch 67, `client_patch.md` section 3.55),
the hud movie's `survarium/controls/CombatLogItem.as` and `flash/hud/bitmaps/`; the check:
`client-patches/check_kill_feed_026g.py`.

- **The owner, 2026-10-25:** "when a character is killed from pain, the character should get
  <weapon> <white painkiller> icon"; "when through a wall: <weapon> <wall shot> icon (available in
  last patches)".
- **The row:** killer, weapon, then the wall icon, the pain icon, the cause icon (headshot,
  skull), victim. Each only when it applies; 0.69d0's order (its description icons, penetration
  among them, come before the cause icon).
- **Through a wall:** the killing bullet went through something before the hit: a pierce of its
  trajectory (bullet +84h, the segments past the first, less the reflections +88h). A wall, a
  prop, or another player it passed through all count; a ricochet alone does not. A kill with no
  bullet (a grenade, an anomaly, a fall) never shows it.
- **By pain:** at the death the victim's pain pool is empty and its body and head pools are not
  (the pain pool killed; when body or head is empty too, they killed). A headshot that killed
  through pain shows both the pain and the headshot icons. Any killer, any weapon; a suicide
  row (no killer, a fall) shows it too.
- **The icons:**
  - wall: 0.69d0's kill log icon `flash_movies/textures/ui_icons/log/icon_penetration.png`
    (40x30), as it is;
  - pain: a lightning bolt for pain shock (the owner's pick, 2026-10-25, of 15 candidates: neither
    0.26g nor retail has a pain or shock icon; the Anesthetic's ampoule read as a bullet), drawn,
    white, 23 px high, a dark shadow one pixel down and right as the hud's own cause icons;
  - both sit 5 px apart in the row, centred on the weapon's icon (never above the row);
  - made by `scripts/kill-feed-icons/make_icons.py`; the build adds each `bitmaps/<class>.png`
    to the movie (`tools/actionscript/flash_tree.py`).
- **Every game alike:** both flags come from the simulation's own state (the bullet, the pools) in
  every game's lockstep run, in the kill event itself; a replay shows them too. Visual only:
  nothing a match computes changes, no `simulation_changes` row.
- **Release:** the exe patch (the default build) and the hud movie together; an older exe with
  the new movie shows rows without the two icons.
