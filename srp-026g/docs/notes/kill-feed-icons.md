# Kill feed: wall, ricochet and pain icons (patch 67)

The rules the code and the tests follow. Code: `client-patches/kill_feed_026g.asm` and
`KILL_FEED_*` in `client-patches/patch_026g.py` (README patch 67, `client_patch.md` section 3.55),
the hud movie's `survarium/controls/CombatLogItem.as` and `flash/hud/bitmaps/`; the check:
`client-patches/check_kill_feed_026g.py`; the hud's rows in Ruffle:
`scripts/kill-feed-icons/check-combat-log/check.sh <hud.swf>`.

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
  - a kill with no bullet (a grenade, an anomaly, a fall) or a melee hit shows neither (the melee
    test is generate_killed_event's own flag, as 0.69d0's; not the bullet's counters).
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
  - ricochet: 0.69d0's `flash_movies/textures/ui_icons/log/icon_ricochet.png` (40x40), its empty rows above and below
    cropped, not scaled: 40x32, art and shadow whole (the owner: to fit the row; 2 px taller than
    the wall icon);
  - pain: a lightning bolt for pain shock (the owner's pick, 2026-10-25, of 15 candidates: neither
    0.26g nor retail has a pain or shock icon; the Anesthetic's ampoule read as a bullet), drawn,
    white, 23 px high, a dark shadow one pixel down and right as the hud's own cause icons;
  - each sits 5 px after the one before, centred on the weapon's icon (never above the row);
    a suicide row has no weapon icon (its frame removes it): the pain icon is centred on the
    cause icon;
  - made by `scripts/kill-feed-icons/make_icons.py`; the build adds each `bitmaps/<class>.png`
    to the movie (`tools/actionscript/flash_tree.py`).
- **Every game alike:** the flags come from the simulation's own state (the bullet, the pools) in
  every game's lockstep run, in the kill event itself; a replay shows them too. Visual only:
  nothing a match computes changes, no `simulation_changes` row.
- **Tested in game** by the owner, 2026-10-25 ("works nicely"); the explicit melee test came after
  that test (Unicorn only).
- **Rev 32's kill feed going blank:** a suicide row with the pain icon (a fall,
  an anomaly, one's own grenade, the suicide key) threw in the icon's placement (no weapon icon on
  that row). The row's renderer, left outside the list, deleted the list's first row on every frame
  from its end on, so the feed stayed empty. A row whose data throws now never leaves a renderer
  behind.
- **Release:** the exe patch (the default build) and the hud movie together; an older exe with
  the new movie shows rows without the three icons.

## The ported weapons' icons (2026-10-26)

- **The owner, 2026-10-26:** "new weapons that were added have incorrect icons" in the kill feed.
- **How the icon is picked:** a weapon's config `ui_desc/combat_log_icon` (an int; the exe passes it
  as the row's `object_icon`, `client_patch.md` 3.55) is frame (icon + 1) of the row's weapon clip
  (`object_icon` of `CombatLogItem`'s kill frame). 0.26g's clip has 53 frames: icons 0 to 52 (17 is an
  empty frame no weapon uses, 21 an M4 no 0.26g weapon used).
- **The cause:** base patch 19's weapons (0.27d0 to 0.31a2 builds, which give each the same icon:
  53 to 68) came with stand-ins of stock weapons' icons, as the clip ends at 52: the AK-74M's 31 for
  the Saiga-12K, Vepr-12, FNC, HK416, MSBS, Valmet and M4A1, the AKMN's 29 for the SOK-94 and the
  vz. 58, the UMP45's 47 for the P90 and the KRISS, the Vityaz's 1 for the three MP7s, the RPK's 38
  for the PKP, the UZI's 3 for the AEK-919K, the Kiparis's 22 for the Kedr, the Remington 870's 12
  for the Neostead.
- **The fix:**
  - the icons after the clip's last are bitmaps of the hud movie, `CombatLogWeapon<icon>` (53 to 68;
    `bitmaps/`, made by `scripts/kill-feed-icons/make_weapon_icons.py`): 53 to 63 the bitmaps of
    0.28d0's clip's frames 54 to 64, as they are (its frames 1 to 53 are 0.26g's bit for bit), 64 to
    68 0.31a2's kill log icons `icon_65.png` to `icon_69.png` (0.28d0's clip ends at 63; 0.31a2 loads
    its icons from textures);
  - `CombatLogItem`: a kill row whose icon is past the clip's frames shows that bitmap in the clip's
    place (the clip hidden), centred where the clip's frames centre theirs (12.5 px down it), never
    above it; the icons after it and the victim follow it as they follow the clip; an icon past 68
    shows the clip's last frame, as before;
  - (2026-11-01, release 41's weapons) the bitmap is found by its class name, `CombatLogWeapon<icon>`
    (`getDefinitionByName`, as CLIK's lists find their renderers under Scaleform; every class of `src/`
    is compiled), so an icon with no bitmap shows the clip's last frame and a ported weapon's icon is
    one bitmap and its class (`make_weapon_icons_046.py`: 0.46c2's 75, 77 to 80, 82 to 84, 0.69d0's
    95 and the Colt M1911's 99, drawn from its lobby cell); the lobby's `KillFeedIcons` finds `KillFeedWeapon<icon>` the same way;
  - `content-edit/kill-feed-weapons-1.json`: the 21 configs take their builds' icons again (the
    premium Saiga and Vepr-12, the Neostead's second config and the MP7 Special their weapon's); the
    M4A1 21, the clip's own M4.
  - The premium copies of stock weapons (base patch 19's `_premium`, `_premium_2`, `_camo`) keep
    their plain weapon's icon, as in those builds.
- **Checked:** `scripts/kill-feed-icons/check-combat-log/check.sh` also adds a kill row for every
  icon 0 to 69 and checks what it shows (the frame, or `CombatLogWeapon<icon>` with the clip hidden)
  and where (after the killer, within the row, before the victim).
- **Release:** the hud movie and the edit list together; the movie alone shows the stand-ins as
  before, the edit list alone the clip's last frame (the VSSK) for icons 53 to 68.

## In the lobby (2026-10-30)

- **The owner:** "for weapons we can use icons from the kill list": the speed tooltip's weapon row
  shows the weapon's kill feed icon.
- **The images:** `scripts/kill-feed-icons/make_lobby_weapon_icons.py` takes them from the hud: the
  bitmap of each frame of the weapon icon clip (sprite 698, `object_icon` of `CombatLogItem`; each
  frame one shape filled 1:1 with its icon's bitmap, read from the shape's fill styles, exported by
  FFDec as it is) for icons 0 to 52, and the hud's own `CombatLogWeapon53..68` for 53 to 68; icon 17
  has none (an empty frame). It writes `flash/lobby_menu/bitmaps/KillFeedWeapon<icon>.png`, their
  classes, and `survarium/player/KillFeedIcons.as` (the index). Rerun it when the hud's icons change.
- **The number:** `content-edit/weapon-kill-icon-ui-1.json` copies each weapon's
  `ui_desc/combat_log_icon` (after `kill-feed-weapons-1`) into its lobby descriptor as property 161,
  not comparable (no card shows it).
- **The row:** the icon fitted by the glyph box's width (they are wide), centred; a weapon whose
  icon the index lacks shows its class's shop filter icon.
- **Colour:** the images are white (the hud tints them per side); in the tooltip they take the
  modifier icons' own colour: a ColorTransform from the average opaque pixel of the atlas's speed
  icon (11), read once at run time (the owner: "too white").
- **Checked:** `make_lobby_weapon_icons.py --check` (run by `scripts/weight-ui/check_ui.sh`, with
  the hud's original read from the published patches) fails when the lobby's files differ from what
  the hud's images make.
- **Size (owner: "a bit bigger", "not too big"):** a weapon's icon box is 1.5 glyph boxes wide
  (39 x 21 in the atlas cell's units): its silhouette shows 1.5 times the glyph-box fit, never
  taller than the other icons. With a weapon row the card's icon column is that wide; the other
  icons stay centred in it, every text after it. The generator trims a transparent margin (the
  hud's images have none today: their alpha reaches every edge).
