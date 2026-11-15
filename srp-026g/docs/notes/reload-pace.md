# Reload clips at their builds' pace

The owner (2026-11-15): "don't change reload times, fix animations deeply". The times
(`weapon-stats.tsv` `reload`, `reload_empty`, and what the engine makes of them) stay; the clips
change. Edit list `reload-timings-1.json` (`reload_pace`), `content_edit.py` (`retime_reload`,
`pace_warp`); measured with resource-porter's `examples/reload-motion.rs`.

## What "natural" is

A clip's length in frames is not its pace: 0.26g itself plays its stock reload clips at 1.0 to 2.3
frames a 1/30 s (the AK-74 95 frames in 3.167 s, the PP-2000 190 in 2.8 s, the AK-12 159 in 2.8 s),
and they look right; the hands move at 1-3 m/s at most in all of them. So the yardstick is what the
original builds did with the same clip (the same name and length): **a clip plays naturally at a
length some build played it at** (a build plays a clip in its time as 0.26g does, frames over 30 over the time: 0.69d0's weapon states compute it so, `tools/re-kit/v069d`). Counted are the lengths a build chose for that clip: both of a
0.46c2 or 0.69d0 two-time reload (`tactical_reload_time`, `reload_time`), the timed clip of a
one-time build (0.26g, 0.29a3, 0.31a2: the tactical clip normalized to `reload_time`, 0x30FC50);
the empty clip's by-product length there only when no build timed it (the SKS's, the pistols' tt33
`reload_empty`). The configs read: 0.26g's stock db, 0.29a3's `v029a3.split`, 0.31a2's and 0.46c2's
survey leaves, 0.69d0's `game.options` (`/mnt/hdd/Projects/weapon-survey-20261016/work`); the clips'
lengths from each build's own archive.

So the AK-74 family plays as it should: 0.46c2 and 0.69d0 played this same motion (their `ak`
clips are 0.29a3's, 145 and 200 frames, the hands within 0.01 m/s of each other) in exactly our
2.4/3.4 s (AKMN 2.6/3.7, RPK 3.0/3.5). The same holds for the AS Val, A-545, ASh-12, 9A-91, VSK-94,
PP-2000, Vityaz and FAMAS: their times are the build's they came from.

## The compile

For a reload whose clip plays more than 3 % away from every such length, `retime_reload` (after
`weapon-stats.tsv`) writes private clips (`animations/single/weapons/srp_reload/<weapon>/`) that
keep the times exactly (an explicit pair, or what the engine plays now: `reload_time` and
`reload_time` x empty frames / tactical frames) and move every knot and event through one map a
slot (hands and item alike, so the gun's motion and the sounds stay on the hands):

- the motion runs at the nearest length a build played it at;
- the difference goes where the clip is slowest: a source frame whose fastest part (either hand,
  the gun, a gun part in the gun's frame; `speed` in the entry) moves under 0.1 m/s takes all of
  its share, over 0.6 m/s none, in between in proportion, smoothed over 13 frames; a still frame
  lasts at most 5x, at least 0.15x its natural length; the first 5 % and the last 10 % are never
  lengthened (the reload starts at once and is done when it looks done);
- what the slow frames cannot take, all frames take alike (the Glock, Mauser, Beretta: reported).

Each entry pins the clips' sha256; a changed clip, an entry no reload plays, or a warped clip whose
channels part from the source's by over 0.12 rad / 1 cm stops the build. The knots move as an
animator moves keys: a channel with keys far apart takes the new timing between them (worst: the
G3SG/1's empty reload's left arm roll bone, 0.10 rad; the vz. 58's and Valmet's, 5 mm).

Retune the table as before; a new time only changes the map. A gun whose times come back inside
its builds' range compiles to one speed again (an explicit pair) or is left alone.

## The guns (2026-11-15)

Speed: the motion against the nearest length a build played the clip at (1.00 natural, under 1
slow motion, over 1 fast-forward). Contact: each hand's median distance from the gun's contact
bone over the clip, mm, left/right, `vostok-port hand-contact` on 0.26g's stance (the hands clip of
a slot with the item clip of that slot).

| gun | clip (source) | frames | now s | builds' s | speed before -> after | contact before -> after |
|---|---|---|---|---|---|---|
| ak_12, ak_12_premium | reload_tactic, tactical-clips-069 (0.69d0) | 120 | 2.8 | 2.200-2.400 | 0.86 -> 1.00 | 0.1/0.0 -> 0.2/0.1 |
| ak_12, ak_12_premium | reload_069d0, tactical-clips-069 (0.69d0) | 159 | 3.71 | 2.800-3.200 | 0.86 -> 1.00 | 0.1/0.0 -> 0.2/0.1 |
| akmsu | reload, weapons-069 (0.69d0) | 200 | 4.414 | 3.400-3.700 | 0.84 -> 1.00 | 0.1/0.0 -> 0.2/0.2 |
| beretta93r | reload_beretta93r, weapons-046 (0.46c2, own copies) | 65 | 1.0 | 1.300-1.600 | 1.30 -> 1.01 | 0.0/0.0 -> 0.4/0.1 |
| beretta93r | reload_empty_beretta93r, weapons-046 (0.46c2, own copies) | 125 | 1.923 | 2.000-2.600 | 1.04 -> 1.00 | 0.0/0.0 -> 0.3/0.1 |
| colt_m1911 | reload, weapons-046 (0.46c2) | 95 | 1.4 | 1.500-2.000 | 1.07 -> 1.00 | 0.0/0.0 -> 0.3/0.1 |
| colt_m1911 | reload_empty, weapons-046 (0.46c2) | 110 | 1.621 | 2.000-2.500 | 1.23 -> 1.00 | 0.0/0.0 -> 0.5/0.2 |
| fn_fnc | reload, patch 19 (0.29a3) | 269 | 4.611 | 3.000 | 0.65 -> 1.00 | 0.0/0.0 -> 0.7/0.7 |
| fort_401 | reload, weapons-046 (0.46c2) | 200 | 6.9 | 5.000-5.500 | 0.80 -> 1.00 | 0.1/0.0 -> 0.4/0.1 |
| g3sg1 | reload_tactic, weapons-069 (0.69d0) | 135 | 3.2 | 3.500 | 1.09 -> 1.00 | 0.0/0.0 -> 0.3/0.1 |
| g3sg1 | reload, weapons-069 (0.69d0) | 215 | 5.096 | 4.000 | 0.78 -> 1.00 | 0.0/0.0 -> 0.4/0.1 |
| glock_17 | reload, weapons-046 (0.46c2) | 95 | 1.25 | 2.000 | 1.60 -> 1.16 | 0.0/0.0 -> 0.5/0.3 |
| glock_17 | reload_empty, weapons-046 (0.46c2) | 110 | 1.447 | 2.500 | 1.73 -> 1.08 | 0.0/0.0 -> 0.7/0.4 |
| hk416 | reload, patch 19 (0.29a3) | 180 | 3.375 | 3.000-3.200 | 0.95 -> 1.00 | 0.0/0.0 -> 0.1/0.0 |
| jackhammer | reload, weapons-046 (0.46c2) | 210 | 2.56 | 3.200 | 1.25 -> 1.00 | 0.8/11.5 -> 2.3/11.6 |
| kedr | reload_empty, stock 0.26g | 100 | 2.5 | 3.000-3.500 | 1.20 -> 1.00 | 142.2/107.4 -> 152.0/107.6 |
| kiparis, kiparis_premium | reload_empty, stock 0.26g | 100 | 2.5 | 3.000-3.500 | 1.20 -> 1.00 | 0.0/0.0 -> 0.2/0.1 |
| m4a1 | reload, patch 19 (0.29a3) | 180 | 3.6 | 3.000-3.200 | 0.89 -> 1.00 | 0.0/0.0 -> 0.2/0.1 |
| mauser_m712 | reload_tactic, weapons-069 (0.69d0) | 155 | 1.5 | 2.000 | 1.33 -> 1.12 | 0.0/0.0 -> 0.4/0.2 |
| mauser_m712 | reload, weapons-069 (0.69d0) | 202 | 1.955 | 3.000 | 1.53 -> 1.26 | 0.0/0.0 -> 0.8/0.1 |
| mp5a1 | reload_tactic, weapons-046 (0.46c2) | 135 | 3.37 | 2.800 | 0.83 -> 1.00 | 0.0/0.0 -> 0.4/0.1 |
| mp5a1 | reload, weapons-046 (0.46c2) | 215 | 5.367 | 3.500 | 0.65 -> 1.00 | 0.0/0.0 -> 0.4/0.2 |
| mp7, mp7_silencer, mp7_silencer_special | reload, patch 19 (0.29a3) | 195 | 3.284 | 2.800 | 0.85 -> 1.00 | 0.0/0.0 -> 0.3/0.3 |
| msbs_carbine | reload, patch 19 (0.29a3) | 185 | 3.265 | 3.000 | 0.92 -> 1.00 | 0.0/0.0 -> 0.2/0.1 |
| ots14 | reload_tactic, weapons-031 (0.31a2) | 130 | 3.17 | 2.500 | 0.79 -> 1.00 | 0.0/0.0 -> 0.5/0.4 |
| ots14 | reload, weapons-031 (0.31a2) | 180 | 4.389 | 3.462 | 0.79 -> 1.00 | 0.0/0.0 -> 0.4/0.5 |
| ppsh_drum | reload_tactic_drum_magazine, weapons-069 (0.69d0) | 130 | 4.6 | 5.000 | 1.09 -> 1.00 | 0.0/0.0 -> 0.4/0.1 |
| ppsh_drum | reload_drum_magazine, weapons-069 (0.69d0) | 200 | 7.077 | 5.000 | 0.71 -> 1.00 | 0.0/0.0 -> 0.4/0.5 |
| saiga_12, saiga_12_premium | reload, patch 19 (0.29a3) | 209 | 3.901 | 3.500 | 0.90 -> 1.00 | 0.0/0.0 -> 0.1/0.1 |
| scar_h | reload, weapons-031 (0.31a2) | 170 | 4.103 | 3.500 | 0.85 -> 1.00 | 0.0/0.0 -> 0.4/0.2 |
| sks, sks_premium, sks_premium_2 | reload, stock 0.26g | 247 | 5.7 | 5.000 | 0.88 -> 1.00 | 0.0/0.0 -> 0.0/0.0 |
| sks, sks_premium, sks_premium_2 | reload_empty, stock 0.26g | 159 | 3.7 | 3.219 | 0.87 -> 1.00 | 0.0/0.1 -> 0.0/0.1 |
| soc_vepr | reload, patch 19 (0.29a3) | 209 | 4.319 | 3.100 | 0.72 -> 1.00 | 0.0/0.0 -> 0.3/0.2 |
| svd | reload_empty, stock 0.26g | 210 | 4.34 | 3.500 | 0.81 -> 1.00 | 0.0/0.0 -> 0.1/0.1 |
| svu_a | reload, weapons-046 (0.46c2) | 255 | 3.1 | 3.500 | 1.13 -> 1.00 | 0.0/0.0 -> 0.3/0.2 |
| uzi, uzi_premium, uzi_premium_2 | reload_tactic, tactical-clips-029 (0.29a3; empty = 0.26g's) | 55 | 2.6 | 2.000-2.300 | 0.88 -> 1.00 | 0.2/0.0 -> 0.2/0.1 |
| uzi, uzi_premium, uzi_premium_2 | reload_029a3, tactical-clips-029 (0.29a3; empty = 0.26g's) | 80 | 3.5 | 2.300 | 0.66 -> 1.00 | 0.2/0.0 -> 0.3/0.1 |
| valmet | reload, patch 19 (0.29a3) | 250 | 4.412 | 3.000 | 0.68 -> 1.00 | 0.0/0.0 -> 0.4/0.4 |
| vepr_12, vepr_12_premium | reload, patch 19 (0.29a3) | 209 | 3.901 | 3.400 | 0.87 -> 1.00 | 0.0/0.0 -> 0.2/0.1 |
| vss, vss_premium | reload_tactic, tactical-clips-029 (0.29a3; empty = 0.26g's Val) | 85 | 2.7 | 2.000-2.500 | 0.93 -> 1.00 | 0.1/0.0 -> 0.4/0.1 |
| vss, vss_premium | reload, tactical-clips-029 (0.29a3; empty = 0.26g's Val) | 120 | 3.7 | 2.500-3.500 | 0.95 -> 1.00 | 0.1/0.0 -> 0.2/0.1 |
| vz58 | reload, patch 19 (0.29a3) | 239 | 4.218 | 3.000 | 0.71 -> 1.00 | 0.0/0.0 -> 0.4/0.3 |

Only the slots outside their builds' lengths are listed; the other slot of those guns plays as
before. The Kedr's hands were off its gun before (it plays the Kiparis's clips on its own model,
stock) and stay so. The worst frames of the contact are where the clips take the hands off the gun (the IK's), and
the same in the source: sampled three times finer, the MP5's and OTs-14's own tactical clips leave
the right hand 15 and 30 mm off the gun between their frames.

## Not done

- The Glock 17, Mauser M.712 and Beretta 93R are faster than any build played them even with
  their slow frames squeezed (`reload` 1.25, 1.5, 1.0 s against 2.0/2.5, 2.0/3.0, 1.3/2.0 s): their
  motion stays 8-26 % fast. Only a longer time (a balance call) would make them natural.
- No new donor clips: 0.69d0's own reloads were looked at first. Its AK clips are 0.29a3's motion
  (no gain; its safety lever sits 0.19 rad elsewhere), its Val/VSS clips slide the gun 5 cm (0.69
  moved the placement to the hands' `Weapon` bone), its Uzi clips drive the remodelled 0.69 Uzi
  (`bolt`, `fire_switch`, `safe`; ours has `shackle`, `zatvor_1`).
- 21 of the 41 guns are ones Skhid's balance never retimed: the stock SVD, SKS, Kedr and Kiparis
  and patch 19's 0.29a3 guns (FN FNC, HK416, M4A1, MP7, MSBS, Saiga-12, SOC Vepr, Valmet, Vepr-12,
  vz. 58). Their empty clip plays as 0.26g or 0.29a3 played it, as a by-product of the tactical
  time; 0.46c2 and 0.69d0 later timed that clip themselves, shorter (the vz. 58's 4.2 s to 3.0 s).
  Their entries can go from `reload_pace` one by one to leave a gun as it was.
- Shotguns and the Mosin (state 306: per-round clips) are not measured.
- Not played: nobody has seen it in the game.
