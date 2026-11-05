# Anomalous Force Protein: gun-stock hits to the hand kill (patch 108)

- **The owner (2026-11-05):** with the Anomalous Force Protein (`weight_booster`, item 339) in a
  pocket, a melee (gun-stock) hit to a hand kills. Only melee, only the carrier as attacker.
- **A hand hit today** (PROVEN, data and code): a weapon's melee is a bullet of the weapon's
  `melee/data` (`damage_type blunt`, `k_damage` 1.5 for the ported guns, 2 stock), so its hit is
  blunt (hit type 1). `damage_model::hit_body_part` (0x2E5080) takes the hit's damage (+34h) times
  (1 - invulnerability) into `body_part_parameters::hit_by_type` (0x329FF0) of the hit's zone
  (`human_hit_params.options`): the zone's armour for the type (none for blunt: 0.26g's armour has
  no blunt), then its `bdb_coeff` sends the damage on to pools. The hand zones (`right_arm`,
  `left_arm`, `right_forearm`, `left_forearm`, health 0) send blunt x1 to their hand pool
  (`right_hand`/`left_hand`, health 0.5) and x0.05 to `pain` (health 1); a bullet sends x0.8 or x1 to
  pain. A hand pool at 0 has affect 3 (the broken arm), not a death; nothing reaches `body`. Death is
  affect 0 of an empty `body`, `head` (below 0.5 a concussion first), `pain` or `infection` pool
  (`on_affect` 0x2DA480 sets the dying flag). So a gun-stock hit to a hand breaks it; a stock melee
  hit's 5 % into pain never empties it.
- **The carrier:** the booster's own parameter `stamina_boost_interval_modifier` 1 (player
  modifier 29: no 0.26g code reads it, no getter call names it, and nothing sets it but skill 205,
  to 0; additive).
  A booster in a pocket adds each non-zero parameter to its carrier's kit at every spawn
  (`survarium::booster` 0x33F690 / 0x33F990, data.md), so the kit sums it above 0 exactly while the
  booster is carried. Data: `force-protein-hand-kill-1.json`.
- **The change** (exe patch 108, Rust `hand_kill`): a lohook at `medical.hit` (0x2E50C7, in
  hit_body_part once invulnerability is applied, before armour; patch 94's site, ours first in its
  chain). A blunt hit on a hand zone whose attacker (hit +44h, the place; its current kit's player,
  not the victim) sums modifier 29 above 0 deals 1000 from there. Its 5 % (50) empties pain through
  any armour short of full, so the victim dies the game's own pain death in the same hit, as a limb
  shot kills; the kill feed shows the pain mark. A fall (blunt, no attacker) and every other hit are
  unchanged; the hit's own damage (+34h, which hit_impl's listeners get) is left as it was.
- **The card:** the description's last line, "Gun-stock hits to the hand kill." The card shows a
  built-in bonus only as "name: +value" (four render paths of the lobby movie), so a line without a
  number would need a movie change.
- **Simulation:** every client deals every hit: a `next` row.
- **Checked:** `check_hand_kill_026g.py` (Unicorn: the hook in the patched exe on made-up objects,
  the game's own apply_modifier); `cargo test`, Clippy; `release/build-content`. **Not run in a
  game.**
- **In game:** two players; A with the Force Protein in a pocket, B without. A's gun-stock hit to B's
  arm or forearm kills B at once; to B's body it deals what it did. Without the booster, a hit to the
  hand breaks it as before.
