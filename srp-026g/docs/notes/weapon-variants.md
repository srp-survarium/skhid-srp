# Skins of live guns from 0.69d0 (pack weapon-variants)

The owner's pick of 0.69d0's skins of guns 0.26g already has (the catalogue, 2026-11-01: every
"yes" whose gun is live; the specials left out), for release 41. Built by
`scripts/weapon-variants` (its README: the rebuild and the checks).

## Rules

- A skin is a **golden weapon** (golden_weapons.md): a gold-bought copy of its gun, the gun's
  stats, the silver bonus as its first modification of its own, upgraded and crafted as the gun
  (either is the other's donor), gold background and name. Sold for gold at no rank, never dropped,
  no faction shop.
- **Stats: the gun's**, the two 0.69d0 premiums too (below).
- **Everything the gun takes, the skin takes**: its ammo (items_compatibility), its camouflage and
  decal slot (weapon-camo-1, weapon-decals-1), its kill-feed icon (kill-feed-weapons-1), its level
  (svd-level-1), its scope fits. Each edit list names the skin beside its gun (check_pack.py holds
  them to it).
- **Gold price** by the gun's level, as the live golden weapons: 500 to level 3, 1,000 to 9 (the
  AK-74N's copy 1,200, as its live one), 2,000 at 12, 2,500 at 15 (the Vepr-12's 2,000 as its live
  copy).
- **What players see**: 0.69d0's paint on 0.26g's own texture of the gun (paint.py); parts 0.69d0
  made separate modules (the AKM's handguard and magazine, the AK-74N's magazine, ...) keep the
  gun's look. The TOZ-122 "Leather" is 0.69d0's model: its own mesh and the leather wrap.

## The skins

| Skin | Name (English) | Dict id | Gun | Gold | Lobby cell | Ported |
|---|---|---|---|---|---|---|
| ak_74n_premium_2 | AK-74N "Avenger" | 2300 | AK-74N | 1,200 | [11, 6] | texture (receiver, stock) |
| akmn_victoryday2019 | AKM "Victory" | 2301 | AKM | 1,000 | [12, 6] | texture (receiver, stock) |
| kedr_premium | PP-91 "Kedr" Taiga | 2302 | PP-91 | 500 | [13, 6] | texture |
| kiparis_victoryday2020 | OTs-02 Kiparis "Victory" | 2303 | OTs-02 | 500 | [14, 6] | texture |
| pkp_pecheneg_premium | PKP "Pecheneg" Pixel | 2304 | PKP | 2,500 | [15, 6] | texture (both) |
| pp2000_summer17 | PP-2000 "Ornament" | 2305 | PP-2000 | 1,000 | [16, 6] | texture |
| rem_700_summer17 | Remington 700 "Soot" | 2306 | Remington 700 | 1,000 | [17, 6] | texture |
| rem_870_premium | Remington 870 "Urban" | 2307 | Remington 870 | 1,000 | [18, 6] | texture |
| sks_victoryday2020 | SKS "Victory" | 2308 | SKS | 1,000 | [19, 6] | texture |
| spas_12_premium | Franchi SPAS-12 "Hexagon" | 2309 | SPAS-12 | 1,000 | [20, 6] | texture |
| sr2m_premium | SR-2M "Veresk" Steppe | 2310 | SR-2M | 1,000 | [21, 6] | texture (stats: below) |
| svd_snow_20 | SVD "Snow-20" | 2311 | SVD | 2,500 | [22, 6] | texture (both) |
| toz_122_hw2017 | TOZ-122 "Hunter" | 2312 | TOZ-122 | 500 | [13, 5] | texture |
| toz_122_premium | TOZ-122 "Leather" | 2313 | TOZ-122 | 500 | [14, 5] | model: 0.69d0's mesh and wrap |
| valmet_victoryday2020 | Valmet Rk.62 "Victory" | 2314 | Valmet | 1,000 | [15, 5] | texture (receiver, grip) |
| vepr_12_legend_0919 | Vepr-12 "Legend" | 2315 | Vepr-12 | 2,000 | [16, 5] | texture (both) |

Scope fits (weapon_scopes.md), every one of the gun's: a fit that only puts the scope on (ten:
the AK-74N's PSO-1, ACOG and Elcan, the AKM's PSO-1 and ACOG, the Remington 700's two, the SVD's
PSO-1 (old), Leupold and 1P69, the TOZ-122's Leupold) names the
skin beside its gun (`config_append` on its `srp_scope_fit.weapons`, as the live AK-74N's copy
shares its gun's fits); a fit that puts the gun's model with a mount on is copied for the skin
(21: 2316-2336, category 22, `fit_<skin>_<scope>` / `fitn_<scope>_<skin>`), its model the fit's model
with the skin's materials. The TOZ-122 "Leather" takes the Leupold, not the Docter (its fit model is
the TOZ-122's mesh with the mount: the wrap would go).
Kill feed: the gun's icon. Texts:
`st_srp_variant_<name>_name` in the ten tables (names.tsv: the gun's name and the skin's word),
the gun's description.

## Stats of the two 0.69d0 premiums

Both ship with their gun's numbers.

| Skin | 0.69d0 / 0.46c2 against the gun | Ships |
|---|---|---|
| AK-74N "Avenger" | 0.69d0: none. 0.46c2: full damage to 37 -> 40 m, min damage from 74 -> 80 m, pierce 12 -> 15 (0.46's scale), hip move speed 0.89 -> 0.87, hit animations 0.09 -> 0.08, move-speed damage 0.5 -> 0.4 | the AK-74N's |
| SR-2M "Veresk" Steppe | 0.69d0: recoil recovery delay 0.08 -> 0.145 s, aimed recoil table +35 % (27 rows) | the SR-2M's |
| (OTs-02 "Victory") | 0.69d0: an unused min-damage key 50 -> 0 | the OTs-02's |

Why: 0.69d0, the last build, took the AK-74N premium's 0.46c2 bonuses back; the SR-2M's are rows of
0.69d0's aimed-recoil table (`aim_recoil.shots_params`, `compensation_cooldown`), a recoil model
0.26g does not have (its recoil is min/max top, left and right), and both would be a better gun for
gold, which the golden weapons' rule ("remove extra bonuses from all golden weapons") rules out.

## How the paint reaches 0.26g's texture (paint.py)

0.69d0's guns are metalness PBR: its albedo of a gun is 0.26g's diffuse lightened (bare metal 2-3x,
paint 1.0-1.4x, per texture). A skin's albedo is that albedo repainted. The skin's texture is 0.26g's
diffuse of the gun with only the repainted texels changed (where the skin's albedo differs from the
gun's, and only on the UV islands 0.69d0's mesh of the skin draws: the islands of 0.69d0's modules
are padding there), each the skin's albedo darkened by the factors that take this gun's 0.69d0
albedo to its 0.26g diffuse and shaded by the ratio of the two. Only those 4x4 blocks are encoded
again; every other block keeps 0.26g's bytes. Measured on the AKM premium texture both builds ship:
half the error of 0.69d0's albedo used as it is.

The UVs: 0.26g's gun meshes are retail's (0.69d0's = ours less the module triangles). For every
skin, at least half of each of our surfaces' vertices sit where a 0.69d0 vertex of the skin is,
all at the same UV (the SPAS-12 is moved as a whole in 0.69d0: same triangles, same UVs);
`report.json` has the numbers.

## Icons

The gun's live icon with the paint: the gun's model posed to the icon (orthographic, silhouette
fitted: IoU 0.82-0.95, the scoped SVD 0.74), each pixel's texels averaged to its footprint, the
repainted ones at the live golden icons' exposure (1.4x the texture, the AKM and SKS premium icons)
and the icon's light across the gun. The outline, shadow and framing are the gun's.

## Not in this pack

- The specials (AKM, PM, VSSK "Special"): the owner set them to no.
- The weapon kit (#893, not merged): the skins carry what their guns carry today; a kit change to
  the guns' configs needs the skins beside them in the same edit list.
- The live AK-74N's golden copy shares its gun's Cobra and EOTech fits, which put the plain gun's
  model on (its paint goes while one is on): a live data matter, left as it is here.
