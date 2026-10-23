# Crouched steps

The owner's request and what it changes. Data only: no exe change. The change ships as resources.db
patches: `scripts/content-edit/crouch-steps-1.json` (the sounds, #272, rev 27) and
`crouch-steps-2.json` (the step events, T27), each with its `release/content.tsv` row.

- **T27 (the owner, 2026-10-23):** "for some reason we added sounds but the movement is still
  silent when crouching". Rev 27+ players have crouch-steps-1 (patch 27 gives `material.pairs` and
  the 1,577 `crouch/` files; no later patch overrides them, PROVEN), but a crouch step was never
  played:
  - the player subscribes `player_equipment_sound_effect::on_player_sound_event` (0x2A6470) to the
    clip event channel `player_sound_events` of its body's animation player (0x29562D..0x295658);
    the event's domain value picks the handler (0x2A64A2): 0 and 1 `on_foot_step` for one foot or
    the other, 2, 3 and 4 three rustles (0x2A6580, 0x2A6630, 0x2A66C0). PROVEN;
  - a clip has that channel or no step: every standing run and sprint clip has it (2 steps a cycle,
    2 frames after each `anim_intervals` knot, with that domain's value: run 2 and 14 of 24,
    AK sprint 1 and 10 of 18); the 16 moving crouch clips (`weapons/ak/player/crouch/3rd_view/`, which
    rifles and pistols both play, `_hud` lists too) have only `anim_intervals` (0 and 15 of 30). So
    crouch-walking never reached `on_foot_step`, and #272's pairs had nothing to play. PROVEN (the
    clips' bytes);
  - 0.31a2 (crouch audible) has the channel on its crouch clips (its 1st_view set, knots 2 and 17,
    the intervals' domain values); its 3rd_view crouch clips have none.
- **The T27 change:** the 16 moving crouch clips get the channel as 0.31a2's: knots 2 and 17, the
  `anim_intervals` domain values (0 then 1; 1 then 0 for `aimed_backward*`, `aimed_left`,
  `aimed_right`). The bone tracks stay byte for byte; the idle and `_look` clips stay without it, as
  run's `_look` clips are. A crouched player now steps twice a 30-frame cycle, and each step plays
  the crouch pair below (others 54, himself 57) at 40 %, as below.

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
