# Imported-weapon balance audit

The proposal and its limits are in
[the balance note](../../docs/notes/imported_weapon_balance.md).
This folder is development tooling, not a second source of server weapon stats.

Run from the checkout root:

```sh
node srp-026g/scripts/weapon-balance/audit.js > /tmp/weapon-balance.tsv
node srp-026g/scripts/weapon-balance/audit.test.js
python3 srp-026g/scripts/weapon-balance/check_content.py "$SKHID_PACKS_DIR"
```

The audit reads current weapon, ammo, armour and modification TSVs, calls the
existing damage calculator engine, and emits 440 rows: 22 guns × ten distances
× torso/head. It chooses the fastest permitted ammo at each distance and
records that choice. `ttk=n/a` with finite shots means a reload is required;
infinite shots means the target is beyond the bullet's reach. It does not
estimate reload duration or actual recoil-driven accuracy. Pump cadence is
bounded by chamber time. Shotgun figures assume all pellets connect; the
additional half-pellet column exposes that sensitivity. See the note before
comparing theoretical shotgun and rifle results at distance.

To reproduce the initial state, extract both tables from the base of the
proposal; changing only the weapon table would leave the new KS-23 ammo active:

```sh
git show 009b2dabe:srp-026g/scripts/content-edit/weapon-stats.tsv > /tmp/before-weapons.tsv
git show 009b2dabe:srp-026g/scripts/content-edit/ammo.tsv > /tmp/before-ammo.tsv
node srp-026g/scripts/weapon-balance/audit.js /tmp/before-weapons.tsv /tmp/before-ammo.tsv
```

`reference.json` stores only extracted metadata not present in those TSVs:
body pools/routes, supported ammunition, drag/lifetime, Zubr coverage and the
armour modifier's paths. The released calculator supplies native data; the
three packs supply the imports and their dictionary operations. The snapshot
records source hashes. `donor-069.tsv` summarizes retail 0.69d0 numeric leaves,
including its own tier and aim-recoil pattern magnitude, without pretending
that its damage/penetration system or tier scale is 0.26's.

To regenerate the input snapshots:

```sh
# Dictionary ids for existing ammo, including patch 19's ported calibres:
vostok-port dump-db-config --db "$PATCHES/resources.db.patch_19" \
  --path gameplay/db_static_dictionaries --leaves > /tmp/dictionary.leaves
# DONOR_LEAVES is the retail 0.69d0 combined gameplay config leaf dump
# (weapons.<name>.parameters.*). Its exact SHA256 is in donor-069.tsv.
python3 srp-026g/scripts/weapon-balance/collect_inputs.py \
  --packs "$SKHID_PACKS_DIR" --dictionary-leaves /tmp/dictionary.leaves \
  --donor-leaves "$DONOR_LEAVES"
```

The collector pins weapons-031@7, weapons-046@10 and weapons-069@11, the
proposal's input packs. The round-trip check independently verifies their
manifest hashes and applies the TSVs to their real binary configs in memory:
no changed type, unrelated leaf change, stale card or hand-built patch folder.
