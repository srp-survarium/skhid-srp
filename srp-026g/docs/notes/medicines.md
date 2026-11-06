# Medicines (the owner, 2026-10-31)

The bandages, the Medikit and three coloured medkits (blue, green, yellow), each made whole from one
table: `scripts/medicines/medicines.tsv`. `scripts/medicines/make_pack.py` makes the pack `medicines`
from it (its row in `release/content.tsv`); content-edit `medicines-1` points the bandages' and the
Medikit's dictionary rows at their new configs; the texts are in `client-patches/localization/*.tsv`.

## Pain-relief investigation (2026-11-03, live revision 44)

The original per-second interpretation was incorrect. The native medkit
constructor (0x33CAC0) divides each configured influence amount by the
configured activity duration: 0x33CD15..0x33CD1D calculates its reciprocal,
and 0x33CDB2..0x33CDB9 stores `amount / duration` in the runtime influence.
`medkit::tick` then multiplies that rate by elapsed seconds. Thus a config
amount is the **total health restored over the course**, before modifiers
and pool clamping. The old card generator multiplied it by duration, masking the too-small totals.

Checked the currently offered revision-44 ZIP (SHA-256
`f5cb33d42148b61e96fa5dc002721da2739694a6250334a89800a6b83d6a2e18`)
and its server-bound executable (SHA-256
`0f3e3ab4c637a743777b496d43c87014bb8048265be8bab712b7619c113c5622`).
Relocated the actual shipped medicine configs, ran the native constructor
through influence loading (including native config lookup), and ran the
native tick and `increase_health` routines in Unicorn in 100 ms steps.
Fixture modifiers are identity; owner/pool lookup and unrelated observer,
injury-removal and activation-list bookkeeping are stubs. No incoming hits,
natural regeneration, network rollback or HUD is simulated. The pain pool
starts empty with its native maximum of 1.0.

| Medicine | Delay + activity | Advertised pain relief | Native result |
|---|---|---|---|
| Orange Medikit | 2 + 3 s | 40% | 13.33% |
| Yellow Science Medkit | 2 + 3 s | 50% | 16.67% |
| Blue Military Medkit | 2 + 1 s | 50% | 50% |
| Green Black Market Medkit | 2 + 2 s | 100% | 100% (clamped) |
| Bandages | 1 + 1 s | 35% | 35% |
| Antitoxin | 1 + 10 s | None | None (50% infection healing) |
| Painkiller | 1 + 10 s | None | None (protection only) |

This confirms under-healing, **not** a yellow-specific zero-healing result.
Yellow's extra infection influence is loaded and processed without dropping
its pain influence. The reported orange/yellow difference still needs an
in-match reproduction; this isolated check cannot settle it. Correct totals
for the intended orange/yellow pain relief are 0.4 and 0.5, respectively,
with the card formula corrected too. The fix changes those two totals only;
all other medicine amounts, durations and protection stay unchanged. Blue and
bandages have a one-second active duration, so the old calculation happened
to be correct. Green's amount of 1 already fills the pain pool, so clamping
hid the incorrect card formula. Other full-heal influences similarly still
fill their pools by the end of the course.

`client-patches/check_medicine_healing_026g.py` runs the native constructor,
tick and health-add routine for all seven medicines, with independent expected
amounts for every pool. It checks activation delay, half and full courses,
pool caps and unchanged totals when duration doubles. `--configs` also checks
extracted final archive files. The calculator now exports course totals and
correctly derived rates; medicine cards calculate total / pool maximum.

## The owner's rules

- Green, yellow and blue medkits, from the builds that have them ("they should be present in some
  patches"); not the later medkit model (0.69 changed it): the old box, the stock medkit's look.
- All five stand on their own: none better than another in everything.
- One table for all of them ("we can just rebuild it completely for all items").
- Yellow heals intoxication and radiation, always completely; gradually is fine.
- Pain (the owner, 2026-10-31): orange 40 %, blue and yellow 50 %, green 100 %, bandages 35 %.
- A bullet hit stops a drug's use or its healing: an exe patch of its own (branch
  `feat/drug-hit-interrupt`), for every drug.

## The table (the owner's, 2026-10-31)

The numbers are content-edit's `scripts/content-edit/medicines.tsv` (the one source: `make_pack.py`
reads them too); `scripts/medicines/medicines.tsv` holds each one's ids, keys, icon and seller.

- What each wins at: bandages light, cheap, a bit of everything; orange the full heal with some pain;
  blue a big heal in 1 s; green the one full pain cure; yellow the full heal that cures
  intoxication and radiation.

## How the table becomes a config

- A drug restores `amount` total health over its active time, after its use time.
  The constructor converts it to a rate; the tick applies rate x elapsed seconds.
  Pools: body 1.0, head 0.4, each arm and leg 0.5, pain 1.0, infection 1.0.
- A full heal needs at least the pool's maximum as its total. Partial heals are
  that share of the maximum: bandages' 35% body = 0.35; orange's 40% pain = 0.4
  over three seconds; yellow's 50% pain = 0.5 over three seconds.
- The immutable base pack supplies the medicine configs. The manifest's later
  `scripts/content-edit/medicines.tsv` edit applies the authoritative amounts
  and cards, so retuning that table does not require rebuilding the base pack.
- Every config is 0.26g's `medkit.options` with the row's keys, icon cell, faction, levels, times,
  weight, heals, injuries and card, at `gameplay/items/medicines/<name>.options`; read back, it is
  the row and nothing else of the template differs. Also the stamina key spelt as the exe reads it,
  as every live item has it (`weapon-balance-1`). `parameters/heal_amount` stays the template's: no
  code or movie reads it.
- New paths: no live config's value changes its type (the release's type check refuses that; the
  stock Medikit's amounts are whole numbers, its 40 % is not). The old `gameplay/items/drugs/`
  bandages and medkit stay in the archive, named by nothing.
- The cards: use, active time and weight, and `medical-cards-1`'s rows (135 arms and legs, 136
  body and head, 137 pain), yellow's also 13 (intoxication, the antitoxin's); no "Heal amount"
  (12).
- Descriptions: the coloured ones one line of what they heal, as the bandages'; the Medikit keeps
  its own.
- Levels: `item_level` 6 for blue and yellow, so Renaissance and Fringe sell them from rank 1
  (srp's rule: the level - 5); green 3, Black Market rank 3. `match_making_level` 3 for the
  medkits, 1 for the bandages, as before: a kit with one is matched as before.
- Sellers: a config's `parameters/faction` (the coloured ones); srp sells a supply whose config
  names a faction from that faction, as gear (`prices.rs`); no stock supply names one. The bandages
  and the Medikit stay the Scavengers' (`EXCEPTIONS`).
- The match changes: the Medikit's pain heal (`simulation_changes.tsv` `next` row). The new items
  change no older game (none has one).

## Intoxication and radiation (PROVEN in the data and the exe's tables)

- The `infection` pool takes both toxic and radiation damage
  (`gameplay/hit_params/human_hit_params.options`: of a player's hit zones only `infection` and the
  hips are not invulnerable to radiation; toxic and radiation hits on the others go to `infection`
  through `bdb_coeff`). Below full it gives `poisoning`; `radiation_sickness` lasts 10 s
  (`affect_duration_sec`).
- The affects (`hit_affects_type_enum`, its names' table in the exe's `.data`): death 0, bleeding
  1, concussion 2, hand_damage 3, leg_damage 4, critical_poisoning 5, poisoning 6,
  radiation_sickness 7, blindness 8 (the medkit's own 3 on the hands and 4 on the legs agree).
- Yellow: `infection` 1 total (full over its 3 s active course), and `remove_affects` infection 6 and 7. That a
  radiation sickness sits on `infection` is LIKELY (not seen in a game): an in-game test in a
  radiation zone checks it.
- The antitoxin keeps its own place: the light cure (0.1 kg) that also halves toxic damage for
  10 s.

## The coloured medkits

| id | name | colour | from | sold by | icon cell |
|---|---|---|---|---|---|
| 390 | `military_medkit`, Military Medkit | blue | 0.27d0 (id 390) | 3 Renaissance | [16, 3] |
| 1196 | `blackmarket_medkit`, Black Market Medkit | green | 0.46c2 (id 1196) | 2 Black Market | [17, 3] |
| 391 | `science_medkit`, Science Medkit | yellow | 0.27d0 (id 391) | 4 Fringe | [18, 3] |

- 0.26g has no medkit model, in the world or in the hands: a medkit is its config, its dictionary
  row, its lobby icon (`LobbyMenu_AmmoIcons`, the "consumables" atlas) and its quick-slot icon in
  the match.
- Icons: 0.27d's atlas cells as they are (the same box as 0.26g's orange one); the green from
  0.46c2's single icon, placed in its cell where 0.27d placed the same box. The cells are free in
  the live movie (other packs draw in columns 4 and 5). The yellow is 0.27d's [14, 1] drawn in
  [18, 3]: the quick slot (below) would show [14, 1] as the Scavengers' oxygen tank.
- The quick slot: the hud movie turns an icon cell into an id, (column - 1) x 22 + row - 1, whatever
  atlas the cell is of, and shows the bitmap it lists for that id (`QuickSlots.as`: 0.26g's
  `NATIVE_HUD_ICONS`, srp's `SRP_HUD_ICONS`). The bandages (23) and the Medikit (14) are 0.26g's;
  the blue (59), green (60) and yellow (61) are srp's rows, their bitmaps the pack's lobby icons
  (`flash/hud/bitmaps/HudItem*Medkit.png`). [14, 1] is id 13, 0.26g's first oxygen tank's. Used up
  in its use time, a coloured medkit's slot shows its gray copy, the Medikit's and the bandages'
  their icon dimmed (`scripts/hud-quick-icons/check.sh`). A new medicine's cell needs an id no
  quick-slot item has, and its row there.
- Ids: the donor's own, free in 0.26g and every pack (0.46c2's 759 and 760 for the first two are
  the Rattle pack's).
- Faction: 0.46c2's names (The Renaissance Army / Fringe Settlers / Black Market Medkit).
- Names: 0.27d's ("Military Medkit", "Science Medkit"), the Black Market one and the other
  languages new, each from its table's word for the medkit and the Black Market.
- Not done: 0.46c2's Military perk (-20 % ranged damage for 2 s). A drug can protect
  (`data/damage_protection`, as the antitoxin's `toxic`), but it is not in the owner's table.
