# Gun-stock hits to the hand kill (patch 108)

- **The owner (2026-11-05):** a melee (gun-stock) hit to a hand kills, for everyone. Falls and every
  other hit stay as they are. (First asked for the Anomalous Force Protein's carrier only; then made
  the default, the booster getting a ragdoll impulse instead.)
- **A hand hit before** (PROVEN, data and code): a weapon's melee is a bullet of the weapon's
  `melee/data` (`damage_type blunt`, `k_damage` 1.5 for the ported guns, 2 stock), so its hit is
  blunt (hit type 1). `damage_model::hit_body_part` (0x2E5080) takes the hit's damage (+34h) times
  (1 - invulnerability) into `body_part_parameters::hit_by_type` (0x329FF0) of the hit's zone
  (`human_hit_params.options`): the zone's armour for the type (none for blunt: 0.26g's armour has
  no blunt), then its `bdb_coeff` sends the damage on to pools. The hand zones (`right_arm`,
  `left_arm`, `right_forearm`, `left_forearm`, health 0) send blunt x1 to their hand pool
  (`right_hand`/`left_hand`, health 0.5) and x0.05 to `pain` (health 1); a bullet sends x0.8 or x1 to
  pain. A hand pool at 0 has affect 3 (the broken arm), not a death; nothing reaches `body`. Death is
  affect 0 of an empty `body`, `head` (below 0.5 a concussion first), `pain` or `infection` pool
  (`on_affect` 0x2DA480 sets the dying flag). So a gun-stock hit to a hand broke it; its 5 % into
  pain never emptied it.
- **The change** (exe patch 108, Rust `hand_kill`): a lohook at `medical.hit` (0x2E50C7, in
  hit_body_part once invulnerability is applied, before armour; patch 94's site, ours first in its
  chain). A blunt hit on a hand zone with an attacker (hit +44h, a place, not 0FFh and not the
  victim's own) deals 1000 from there. Its 5 % (50) empties pain through any armour short of full,
  so the victim dies the game's own pain death in the same hit, as a limb shot kills; **the kill
  feed shows the pain-death mark** (patch 67's lightning: pain empty, body and head not). A fall
  (blunt, no attacker) and every other hit are unchanged; the hit's own damage (+34h, which
  hit_impl's listeners get) is left as it was.
- **Simulation:** every client deals every hit: a `next` row.
- **Checked:** `check_hand_kill_026g.py` (Unicorn: the chain in the patched exe on made-up
  objects); `check_medical_skills_026g.py` (the shared site); `cargo test`, Clippy. **Not run in a
  game.**
- **In game:** a gun-stock hit to another player's arm or forearm kills him at once; to his body it
  deals what it did; a fall breaks no rule it did not before.
