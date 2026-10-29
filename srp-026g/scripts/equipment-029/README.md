# Equipment through 0.29

`equipment-029@3` adds 84 equipment pieces absent from the released 0.26g
dictionary: 62 regular pieces and 22 premium/reskinned or special pieces,
including Keeper of the Sun. The donor is retail **0.29c**, from
`Survarium_archives/Survarium29c/game/resources.00.db` through `.18.db` in the
owner's `Survarium_archives.zip`. Donor Protective mask 948 is deliberately
excluded: its existing port, 1816 from `face-069d`, keeps its model and balance.

`items.tsv` is the balance source. Levels, protection and prices are adapted
to the existing 0.26g progression, as requested by the owner. `template_id`
supplies the native equipment schema, modification masks, repair/degradation
rules and miscellaneous defaults. Weight and built-in modifications are
explicit TSV values, retuned against existing gear after the owner's request
to make the additions competitive with equipment such as Zubr. `peer_id`
names the native benchmark and `role` groups the set. Premium rows inherit
weight and modifiers from their plain piece. Armour and isolation are percentages in the TSV;
both the actual hit protection and item-card values are converted together.
Backpacks use native backpack parameters. The firefighter oxygen tank keeps
its donor capacity of 140 seconds. No bleeding-protection mechanic is added.

The comparisons and trade-offs are in [BALANCE.md](BALANCE.md). The progression is:
Animal skin and Private at 1, Hunter at 3, Saboteur at 5,
Paratrooper at 6, Viper at 8, Warrior/Watchman/Pathfinder at 9,
Omicron/Combat at 12, Firefighter/Mercenary at 14, Nomad at 15.
The Scavenger hood is level 3. Individual helmet and mask protection differs
from torso protection; see the TSV for every value and price. The prices are
explicit balance choices, not donor prices (that dictionary has none).

Premium pieces copy their plain counterparts' combat parameters, body-region
protection and default modifications exactly, with zero random-drop weight.
They receive +5% experience and +5% reputation with their faction as their
first two owned modifications, following the existing premium system.
Keeper of the Sun uses this same rule in place of its donor combat bonuses.
The `srp_premium` config field declares the plain item and bonus IDs; the
server parses it from game data. The Flash movie cannot read custom config
fields, so `Equipment029.as` is a generated mirror, checked against the pack.

The donor models, textures, sounds and item icons are retained. Native
equipment data type 4 replaces donor type 5. Omicron's mask keeps the player's
native head, like the other mask ports. New icons occupy equipment-atlas
columns 12–15. Texts use their own namespace in all six supported languages;
descriptions absent from the donor are explicitly empty. Shared assets
already present in the released game are reused, not overwritten.

## Rebuild

Run from the repository root with Python 3 and Pillow. The extracted inputs
are pinned by `inputs.sha256`; `base-patches.tsv` pins the published base
patches through 36. Neither a server database nor a release working directory
is an input. The donor reader must support directory hard links: older porter
scans silently miss linked model surfaces. `prepare_inputs.py` uses the
repository's `srp-game-data` reader for that reason.

```sh
python srp-026g/scripts/equipment-029/prepare_inputs.py \
  --donor private/equipment-029/donor \
  --stock /path/to/stock-026g/resources.db \
  --patches ~/.cache/skhid-release/patches \
  --game-data-bin srp-026g/target/release/srp-game-data \
  --out private/equipment-029/inputs
python srp-026g/scripts/equipment-029/make_pack.py \
  --donor private/equipment-029/inputs/donor \
  --assets private/equipment-029/inputs/assets \
  --base private/equipment-029/inputs/base \
  --out private/content-packs/equipment-029-3
cmp private/content-packs/equipment-029-3/ui/Equipment029.as \
  srp-026g/flash/lobby_menu/src/survarium/constants/Equipment029.as
python srp-026g/scripts/equipment-029/check_pack.py \
  private/content-packs/equipment-029-3 private/equipment-029/inputs/base
python srp-026g/scripts/equipment-029/balance_audit.py \
  private/content-packs/equipment-029-3 private/equipment-029/inputs/base \
  --out private/equipment-029/balance-audit.json
```

Output directories must be new. If the premium mapping changes, copy the
generated movie source into the Flash tree. Register the pack's `pack.json`
hash in `release/content.tsv`. The release patch is made only through
`release/build-content <revision>`, which composes the whole manifest.
No revision is reserved by this content change.

## Checks

`check_pack.py` checks all 84 configs, exact premium/plain combat parity,
native parameter vocabulary, card values, icon collisions, six localization
tables and the generated Flash mapping. `golden::content` Rust tests check
content-defined premiums and reject malformed relationships and bonuses.
Run `cargo test -p survarium` and `cargo clippy -p survarium --all-targets`
under `nix develop` in `srp-026g`.

`balance_audit.py` compares the new regular pieces with native equipment from
the pinned published data. Its report records the chosen benchmark, built-in
bonuses, actual hit-region protection, weight, prices and occupied slots.
It rejects strict stat dominance in either direction against pieces at the
same level with identical coverage and occupied slots. There are 110 such
comparisons for 56 pieces; the other six have no exact peer and their roles
are reviewed separately in BALANCE.md. This does not claim equal win rates.

Validate the resource dependencies with the porter's `validate-overlay`,
then build the complete content manifest. Check its `server-game-data.db`
with `srp-check-game-data`. Extract the resulting `flash_movies/lobby_menu.swf`
and run `bash srp-026g/scripts/equipment-029/check_ui.sh <movie> <work-dir>`
under the Nix shell: it checks premium bonuses and crafting relationships in
the compiled movie using CPU-only Ruffle on a separate virtual display.

These checks do not establish clothing fit or animation quality in the game.
An in-game fitting pass, especially Omicron's mask/head combination, remains
necessary before release. This change does not deploy content or restart a
server.
