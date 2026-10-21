# Crouched steps

The owner's request and what it changes. Data only: no exe change. The change ships as a
resources.db patch (`scripts/content-edit/crouch-steps-1.json`, its `release/content.tsv` row).

- **Request (the owner, 2026-10-21):** "when crouched player should still make sound, right now
  they don't at all, it should be lower volume (don't know by how much) but not 0".
- **Why crouching is silent** (stock 0.26g; no Skhid patch up to 26 carries these files, PROVEN):
  - a step is an animation sound event: `player_equipment_sound_effect::on_foot_step` (0x2A6520)
    takes a foot material from the player, by the logic state's movement id (state+24h: 1 crouch
    walk, 2 stand run, 3 sprint, 4 jump; set by the state constructors at 0x2FAC00..0x2FAD80,
    0x336B87) and the view (first person for the player himself, third for everyone else), and
    calls `step_manager::on_step` (0x248AD0). PROVEN;
  - the eight foot materials come from the boots (`game_materials/{walk,run,sprint,jump}_{1st,3rd}_view_material_id`,
    read at 0x203A1B, copied to the player at 0x1F1B42): crouch walk is `walk`, every one of the 17
    boots gives it `actor_crouch` (54, third person) and `actor_local_crouch` (57, first person);
    standing `run` gives `actor` (10) and `actor_local` (42). PROVEN;
  - `on_step` plays the sound of the material pair (foot material, ground under the foot) and plays
    nothing when the pair has none (0x248B98: pair+20h null). In `game_materials/material.pairs`
    the 80 pairs of 54 and 57 have an empty `collision_sound`. So the silence is data, not a skip in
    the code. PROVEN.
- **Later builds:** 0.27d0, 0.28a2 and 0.28d0 are the same (the 54/57 pairs empty). 0.31a2 added
  crouch steps (kept in 0.46c2 and 0.69d): the same pairs, one sound for every ground (mud and deep
  water their own): a cloth rustle and a soft step, 3D for others. Measured (sample loudness x
  `volume_k` x distance curve), 0.31a2's crouch is about 58 % of its walking at 1 m, 21 % at 3 m,
  10 % at 5 to 8 m; the rustle is heard to 17 m, the step to 5 m.
- **Values (proposed):** each crouch pair plays a copy of what the walking pair of the same ground
  plays (54 takes 10's, 57 takes 42's): the same samples, every `volume_k` x 0.4 (-8 dB), the
  same distance curve. A ground silent for walking stays silent crouched (13 of the 80 pairs: for
  others tree trunks, car glass, dry foliage, ...). Crouch : walk : sprint is then about 0.4 : 1 : 3 for
  others (sprint's samples have 3 times walking's `volume_k` on dirt).
- **What it means** (others, dirt; the `steps` curve, read linearly between its keys):

  | distance | walking | crouched |
  |---|---|---|
  | 1 m | 0.067 | 0.027 |
  | 3 m | 0.039 | 0.016 |
  | 5 m | 0.023 | 0.009 |
  | 8 m | 0.011 | 0.004 |
  | 17 m | silent | silent |

  A crouched player at 1 m sounds like a walking one at about 4.5 m; at 3 m like one at about 7 m.
  Louder grounds stay louder (concrete, metal, slate): every ground keeps its own sound, at 40 %.
- **Who hears it:** everyone in the match near the crouching player: a 3D sound (`pan_3d`) at the
  ground point under the foot, by each listener's distance on the `steps` curve (silent from 17 m).
  The player himself hears his own steps through the first-person set (not 3D, like his walking),
  also at 40 %.
- **Who computes it:** nothing of the simulation: the step is presentation (an animation event:
  a ray to the ground, a sound, the pair's decal and particle, and crouch pairs have neither). The
  game already picks steps by view (first-person sounds for oneself, third-person for others), so
  step sounds differ between games of one match and cannot feed it. No `simulation_changes.tsv`
  row; srp reads none of these files.
- **The files:** 1,577 new files under `crouch/` (`sounds/single/crouch/...`: 506 samples copied
  whole, their `.raw_options` with `volume_k` x 0.4; 43 collections in
  `sounds/collections/crouch/`, 16 composites in `sounds/composite/crouch/`, their filenames
  prefixed `crouch/`), about 6.5 MB, and `game_materials/material.pairs` (67 pairs get a sound).
  `content_edit.py` makes them (its `copy`, `scale`, `prefix_string` and `set_entry` edits).
- **If it is too loud or too quiet:** the list's one `scale` factor (0.4); 0.3 is -10.5 dB,
  0.5 is -6 dB.
