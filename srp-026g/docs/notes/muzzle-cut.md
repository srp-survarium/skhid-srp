# Muzzle cut: can the stock muzzle device come out of the gun's mesh?

Research for the weapon-kit work (#420): 0.26g gun models carry their stock muzzle device in the
mesh, so a device put on `barrel_point` stacks in front of it. This survey asks, for every gun,
whether that stock device can be cut out cheaply so a 0.69d0 device replaces it.

## Answer

- No model has a muzzle bone: every barrel_point is on bone `offset`, and the device's vertices
  are on `offset` too (class b never happens).
- 28 weapons (25 models) have the stock device as its **own closed part** (class c): a set of
  triangles sharing no vertex with the barrel. Cutting it is dropping those triangles from one
  surface's index list; no remodelling, no new vertices. Behind it is the barrel or its stub (the
  part was sleeved over it) or the front sight block.
- 5 weapons (4 models) have a suppressor as a **whole surface** (class a): hidden by dropping the
  surface. Two are real cut targets (Kriss Vector, MP7 with suppressor); the VSSK's is integral.
- 3 weapons (3 models) are **fused** (class d): AK-12 (+premium), whose brake is one part with the
  barrel and front sight (991 vertices), and the vz.58.
- 47 weapons have nothing to cut: bare barrels, integral suppressors, shotguns, pistols.
- Every class c gun has its own stock device in 0.69d0 (column "0.69d0
  device", under `models/weapons/modules/muzzlebrakes/` or `firearrestors/`): the cut gun plus the
  0.69d0 stock device looks like the stock gun, and any other device then replaces it.
- A cut leaves at most a small open ring (8 to 18 open edges) at the barrel's end on HK416, M4A1,
  SOCOM 16: the bore. No cap is needed for the swap (the new device covers it); a bare gun shows
  an open muzzle, which is what a muzzle looks like.

## Classes

    a  the device is a whole surface of the model (hide or drop the surface)
    b  the device's vertices are on their own bone (none found)
    c  the device is its own closed part: no shared vertex with the rest (drop its triangles)
    d  the device is one part with the barrel: a plane cut and a cap, or remodelling
    -  nothing to cut

## Table

Columns: `past` is how far the mesh reaches past barrel_point (cm; #420's number); `part v/t` the
device part's vertices and triangles; `len` its length along the barrel (cm); `cut leaves` what
shows at the front once it is gone. Premium skins share their base model's row values.

```
weapon                     | model                                          | cls | past  | part v/t | len  | cut leaves             | 0.69d0 device        | note
---------------------------+------------------------------------------------+-----+-------+----------+------+------------------------+----------------------+------------------------------------------------
kriss_vector               | kriss_vector/kriss_vector                      | a   | +4.1  | 318/452  | 29.7 | barrel                 | -                    | suppressor surface
mp7_silencer               | hk_mp7/hk_mp7_silencer                         | a   | +1.3  | 493/516  | 23.0 | barrel                 | hk_mp7               | suppressor surface
vssk                       | vssk/vssk                                      | a   | +8.9  | 249/310  | 65.0 | no barrel              | -                    | integral suppressor
vssk_camo                  | vssk/vssk_camo_red                             | a   | +8.9  | 249/310  | 65.0 | no barrel              | -                    | integral suppressor
wpn_mp7_silencer_special   | hk_mp7/hk_mp7_silencer                         | a   | +1.3  | 493/516  | 23.0 | barrel                 | hk_mp7               | suppressor surface
a_545                      | a_545/a_545                                    | c   | +8.6  | 374/424  | 9.1  | sight block            | a545_m24x15          |
ak_74m                     | ak74m/ak74m                                    | c   | +2.9  | 255/260  | 8.1  | sight block            | ak74_m24x15          |
ak_74n                     | ak74n/ak_74n                                   | c   | +2.9  | 255/260  | 8.1  | sight block            | ak74_m24x15          |
ak_74n_premium             | ak74n/ak_74n                                   | c   | +2.9  | 255/260  | 8.1  | sight block            | ak74_m24x15          |
akmn                       | akmn/akmn                                      | c   | -1.7  | 218/312  | 3.2  | sight block            | akm_m14x1l           | device ends before barrel_point
akmn_premium               | akmn/akmn_camo                                 | c   | -1.7  | 218/312  | 3.2  | sight block            | akm_m14x1l           | device ends before barrel_point
akmn_premium_2             | akmn/akmn_camo_st                              | c   | -1.7  | 218/312  | 3.2  | sight block            | akm_m14x1l           | device ends before barrel_point
ash_12                     | ash_12/ash_12                                  | c   | +5.7  | 726/740  | 18.0 | shroud                 | ash12_m24x15         | auto-pick took shroud vents too
fn_fnc                     | fn_fnc/fn_fnc                                  | c   | +3.5  | 218/206  | 9.1  | barrel (hidden inside) | fnfnc_i05x28         |
fn_p90                     | fn_p90/fn_p90                                  | c   | +0.4  | 207/240  | 6.1  | barrel                 | fnp90                |
hk416                      | hk416_a5/hk416_a5                              | c   | +2.1  | 137/148  | 4.4  | barrel, open end       | hk416a5_i05x28       | own part of hk416_barrel_s
m4a1                       | m4a1/m4a1                                      | c   | -1.7  | 287/438  | 4.4  | barrel, open end       | m4a1_i05x28          | device ends at barrel_point
msbs_carbine               | msbs/msbs_carbine                              | c   | +2.6  | 150/158  | 5.2  | barrel                 | msbsradon_i05x28     |
pkp_pecheneg               | pkp_pecheneg/pkp_pecheneg                      | c   | +0.5  | 154/140  | 4.3  | barrel                 | pkp_pecheneg         |
pp2000                     | pp2000/pp2000                                  | c   | +3.2  | 744/758  | 5.5  | barrel                 | pp2000               | auto-pick took the hand stop too
rpk                        | rpk/rpk                                        | c   | +17.6 | 169/260  | 4.1  | barrel                 | rpk_74 hider         | barrel_point 13.5 cm short (AK copy)
socom16                    | socom16/socom16                                | c   | +0.3  | 66/72    | 3.2  | barrel, open end       | socom16_i06x24       |
sr3m                       | sr3m/sr3m                                      | c   | +2.6  | 255/416  | 4.2  | sight block            | sr3m                 |
sr3m_premium               | sr3m/sr3m                                      | c   | +2.6  | 255/416  | 4.2  | sight block            | sr3m                 |
sv_98                      | sv98/sv_98                                     | c   | +0.3  | 204/252  | 6.2  | barrel                 | sv98_m30x1_barrelnut |
svd                        | svd_riffle/svd                                 | c   | +7.4  | 294/345  | 11.9 | barrel                 | svd_m14x1l           | part holds the sight base collar too: plane cut
valmet                     | valmet_rk62/valmet_rk62                        | c   | +4.6  | 446/396  | 7.3  | thread stub            | valmet_rk62          |
vepr                       | vepr                                           | c   | +5.3  | 255/260  | 8.1  | barrel stub            | vepr_m24x15          |
vepr_12                    | vepr_12/vepr_12_short                          | c   | +4.8  | 621/660  | 7.5  | barrel stub            | vepr12_m12x075       |
vepr_12_premium            | vepr_12_camo/vepr_12_medium_camo               | c   | +19.8 | 621/660  | 7.5  | barrel                 | vepr12_m12x075       | barrel_point of the short barrel
vepr_premium               | vepr_camo/vepr_camo                            | c   | +5.3  | 255/260  | 8.1  | barrel stub            | vepr_m24x15          |
vityaz                     | vityaz/vityaz                                  | c   | +3.2  | 134/160  | 4.0  | sight block            | pp_1901_vityaz       |
vityaz_premium             | vityaz/vityaz                                  | c   | +3.2  | 134/160  | 4.0  | sight block            | pp_1901_vityaz       |
ak_12                      | ak_12/ak_12                                    | d   | +2.5  | -        | -    | -                      | ak_12_m14x1l         | brake fused with barrel + sight
ak_12_premium              | ak_12_camo/ak_12_camo                          | d   | +2.5  | -        | -    | -                      | ak_12_m14x1l         | brake fused with barrel + sight
vz58                       | vz58/vz58                                      | d   | +0.8  | -        | -    | -                      | vz58_m14x1l          | fused, no visible device
a9_91                      | a9_91/a9_91                                    | -   | +0.3  | -        | -    | -                      | -                    | integral suppressor
aek_919k                   | aek_919k/aek_919k                              | -   | +3.8  | -        | -    | -                      | aek919k              | bare barrel tip
ak_74u                     | ak74u                                          | -   | -17.3 | -        | -    | -                      | aks74u_m24x15        | barrel_point 17 cm past mesh
ak_74u_premium             | aks74u_camo/ak74u_camo                         | -   | -17.3 | -        | -    | -                      | aks74u_m24x15        | barrel_point 17 cm past mesh
aps                        | aps/aps                                        | -   | +0.4  | -        | -    | -                      | -                    |
benelli_m4                 | benelli_m4/benelli_m4                          | -   | -4.2  | -        | -    | -                      | -                    |
fort_17                    | fort_17/fort_17                                | -   | +1.7  | -        | -    | -                      | -                    |
izh_27_lancaster           | izh_27_lancaster/izh_27_lancaster_             | -   | +0.1  | -        | -    | -                      | -                    |
izh_27_lancaster_premium   | izh_27_lancaster/izh_27_lancaster_             | -   | +0.1  | -        | -    | -                      | -                    |
izh_27_lancaster_premium_2 | izh_27_lancaster_camo/izh_27_lancaster_camo_st | -   | +0.1  | -        | -    | -                      | -                    |
kedr                       | pp9_klin/pp9_klin                              | -   | +0.0  | -        | -    | -                      | -                    | bare barrel
kiparis                    | kiparis                                        | -   | -0.4  | -        | -    | -                      | -                    | bare barrel
kiparis_premium            | kiparis                                        | -   | -0.4  | -        | -    | -                      | -                    | bare barrel
magnum                     | magnum/magnum                                  | -   | +13.4 | -        | -    | -                      | -                    | revolver barrel
mosin_nagant               | mosin_nagant/mosin_nagant                      | -   | +0.0  | -        | -    | -                      | -                    |
mp7                        | hk_mp7/hk_mp7                                  | -   | +1.4  | -        | -    | -                      | hk_mp7               | bare barrel stub
mp_153                     | mp153/mp153                                    | -   | +10.8 | -        | -    | -                      | -                    | tube past barrel_point
neostead                   | neostead_camo/neostead_camo                    | -   | +1.0  | -        | -    | -                      | -                    |
neostead_2                 | neostead/neostead                              | -   | +1.0  | -        | -    | -                      | -                    |
pm                         | pm/pm                                          | -   | +0.2  | -        | -    | -                      | -                    |
ppsh                       | ppsh/ppsh                                      | -   | -0.2  | -        | -    | -                      | -                    |
rem_700                    | rem_700                                        | -   | -0.1  | -        | -    | -                      | -                    |
rem_700_premium            | rem_700_camo_st                                | -   | -0.1  | -        | -    | -                      | -                    |
rem_870                    | rem_870                                        | -   | +3.9  | -        | -    | -                      | -                    | tube past barrel_point
rpd                        | rpd/rpd                                        | -   | -0.1  | -        | -    | -                      | -                    |
saiga_12                   | saiga_12s/saiga_12s                            | -   | +0.0  | -        | -    | -                      | -                    |
saiga_12_premium           | saiga_12s/saiga_12s_camo                       | -   | +0.0  | -        | -    | -                      | -                    |
sks                        | sks                                            | -   | +0.1  | -        | -    | -                      | -                    |
sks_premium                | sks_camo/sks_camo                              | -   | +0.1  | -        | -    | -                      | -                    |
sks_premium_2              | sks_camo/sks_camo_st                           | -   | +0.1  | -        | -    | -                      | -                    |
soc_vepr                   | soc_vepr/soc_vepr                              | -   | -0.0  | -        | -    | -                      | -                    |
spas_12                    | spas_12/spas_12                                | -   | +0.1  | -        | -    | -                      | -                    |
sr2m                       | sr2m/sr2m                                      | -   | +0.2  | -        | -    | -                      | -                    |
toz_122                    | toz_122                                        | -   | +0.2  | -        | -    | -                      | -                    |
toz_34                     | toz34/toz_34                                   | -   | +0.0  | -        | -    | -                      | -                    |
toz_66                     | toz_66                                         | -   | +0.0  | -        | -    | -                      | -                    |
tt_33                      | tt33                                           | -   | -0.0  | -        | -    | -                      | -                    |
ump45                      | ump_45/ump_45                                  | -   | +0.1  | -        | -    | -                      | -                    |
uzi                        | uzi                                            | -   | +0.0  | -        | -    | -                      | -                    | bare barrel
uzi_premium                | uzi                                            | -   | +0.0  | -        | -    | -                      | -                    | bare barrel
uzi_premium_2              | uzi_camo/uzi_camo_st                           | -   | +0.0  | -        | -    | -                      | -                    | bare barrel
val                        | val/val                                        | -   | +0.1  | -        | -    | -                      | -                    | integral suppressor
vsk_94                     | vsk_94/vsk_94                                  | -   | +0.3  | -        | -    | -                      | -                    | integral suppressor
vsk_94_premium             | vsk_94/vsk_94_camo                             | -   | +0.3  | -        | -    | -                      | -                    | integral suppressor
vss                        | vss/vss                                        | -   | +0.1  | -        | -    | -                      | -                    | integral suppressor
vss_premium                | vss/vss                                        | -   | +0.1  | -        | -    | -                      | -                    | integral suppressor
winchester_1894            | winchester_1894                                | -   | +13.5 | -        | -    | -                      | -                    | tube past barrel_point
```

Notes on single guns:

- SVD: the flash hider's part also holds the lower collar of the front sight base. Dropping it
  leaves the sight hanging; keep the collar by cutting the part at a plane (sight's front face)
  instead of dropping it whole, or leave the SVD out at first.
- ASh-12, PP-2000: the automatic pick also took nearby parts (shroud vents, the hand stop); the
  brake itself is its own part, so the cut is a hand-checked part list, not the automatic one.
- RPK, Vepr-12 premium (medium barrel), AK-74U: barrel_point is wrong for the model (copied from
  another gun: 13.5 cm short, the short barrel's, 17 cm past the mesh). A device needs a measured
  barrel_point there whatever the cut.
- AKMN, M4A1: the device ends at or before barrel_point, so #420's offset is 0 and a device stacks
  in front of the stock one all the same; the cut fixes them too.
- AEK-919K: the 0.26g model has a bare barrel tip (3.8 cm past barrel_point once the `offset`
  bone's translation is counted; #420's offsets.tsv note "0.316 m" left it out).

## Method

Read from the models the release ships (stock `resources.db` + patches 12 to 37, extracted with
`srp-game-data get`), per weapon's `skinned_model`:

1. barrel_point: `render/export_properties` `/locators[]`; its bone (`render/bind_pose`) is
   `offset`, bound rotated pi about y, so model space is (-x, y, -z) + the bone's translation. The
   barrel points along +z in every model.
2. Each surface's `converted_model`: positions (float3 at offset 0), bone index (byte 12),
   triangles (u16 indices). Vertices welded by position (0.1 mm), triangles joined into connected
   parts.
3. Device: the parts whose centre is within 3 cm of the barrel axis, lying wholly in the last 15 cm
   before barrel_point and reaching the gun's front (within 5 mm), plus axis parts inside their
   span. Class c when nothing else on the axis reaches past barrel_point; d otherwise.
4. Under the device: remaining triangles inside its span and radius (what it hid) and their open
   edges (a hole to cap).
5. A side view of the last 35 cm of each gun with and without the part, checked by eye; the class
   and notes above are after that check (pistol slides, front sights, bare barrels and wrong
   barrel_points the automatic pick caught were set by hand).

`docs/images/muzzle-cut.png`: before and after for AK-74M, HK416, FNC, A-545, Valmet, SVD,
Vityaz, and the fused AK-12.

## First step

One gun, end to end: the AK-74M (also AK-74N and its premium: same part, 255 vertices / 260
triangles in surface `ak74lpsg4`). A content edit that rewrites that surface's index chunk
without the part (vertex chunk untouched, so skinning and bounds stay valid), plus 0.69d0's
`ak74_m24x15_muzzlebrake` as the gun's default device on barrel_point with offset 0. The stock
look comes back from the device; any other device then replaces it instead of stacking. The same
edit then covers the other class c guns by part lists, HK416 and FNC first (0.69d0 has both
devices).
