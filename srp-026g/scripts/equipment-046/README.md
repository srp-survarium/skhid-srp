# Equipment from 0.31a2 and 0.46c2

`equipment-046@2` adds 35 equipment pieces absent from the released 0.26g
dictionary (the owner's pick of 0.69d equipment): 34 regular pieces and one
premium copy. What ships, the balance proposals, what is left out and the
open questions are in
[docs/notes/equipment-046.md](../../docs/notes/equipment-046.md).
`items.tsv` is the balance source; the method is equipment-029's
([BALANCE.md](../equipment-029/BALANCE.md)), whose machinery `make_pack.py`
reuses (icon atlas, shader switches, premium metadata).

The donors are the 0.31a2 and 0.46c2 game folders unpacked from their
installers (the weapon survey of 2026-10-16:
`builds/v031a2/out/game`, `builds/v046c2/out/game`). Their item texts are the
localization catalog's rows (`client-patches/localization/*.tsv`), which the
pack writes as they are.

## Rebuild

Run from the repository root with Python 3 and Pillow. `inputs.sha256` pins
every extracted input; `base-patches.tsv` pins the published patches through
38. Neither a server database nor a release working directory is an input.

```sh
python srp-026g/scripts/equipment-046/prepare_inputs.py \
  --donor031 <0.31a2 game folder> --donor046 <0.46c2 game folder> \
  --stock /path/to/stock-026g/resources.db \
  --patches ~/.cache/skhid-release/patches \
  --game-data-bin srp-026g/target/release/srp-game-data \
  --out <inputs folder>
python srp-026g/scripts/equipment-046/make_pack.py \
  --inputs <inputs folder> --out private/content-packs/equipment-046-2
cmp private/content-packs/equipment-046-2/ui/Equipment046.as \
  srp-026g/flash/lobby_menu/src/survarium/constants/Equipment046.as
python srp-026g/scripts/equipment-046/check_pack.py \
  private/content-packs/equipment-046-2 <inputs folder>/base
python srp-026g/scripts/equipment-046/balance_audit.py \
  private/content-packs/equipment-046-2 <inputs folder>/base --out <report.json>
```

`prepare_inputs.py --pin` computes the inputs from `items.tsv` instead (the
donor models' dependency closure, the released game's assets reused) and
writes `inputs.sha256` and `shipped_textures.txt`. Output folders must be new.
A changed pack is a new version: its folder, `VERSION` in `make_pack.py` and
the `pack.json` hash in `release/content.tsv`. The release patch is made only
by `release/build-content <revision>`.

## Checks

- `check_pack.py`: the 35 configs (native parameters, card values, hit types,
  surface names, no later-build fields, built-in bonuses at the promoted
  strength, sounds the released game has), the premium copy's exact parity,
  icons in free cells, the six dictionaries equal to the catalog, the generated
  Flash mapping, every surface's chunks, no 16-byte config left, every shader
  compiled and every map shipped.
- `balance_audit.py`: no strict stat dominance against native, equipment-029
  or new pieces of the same level, kind, regions and slots.
- `client-patches/check_localization_026g.py` for the catalog rows.
- The compiled lobby: `bash srp-026g/scripts/equipment-029/check_ui.sh <movie>
  <work dir>` (its harness covers both packs' premium copies).
- The composed content: `release/build-content`, then `srp-check-game-data` on
  its server data and the porter's `validate-overlay`.

None of these shows clothing fit or animation in the game; see the notes for
the in-game checks.
