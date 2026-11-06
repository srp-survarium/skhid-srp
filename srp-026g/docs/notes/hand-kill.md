# Gun-stock hits kill (patch 108)

- **The owner (2026-11-05):** a melee (gun-stock) hit kills at once, for everyone, wherever it
  lands. Falls and every other hit stay as they are. (First asked for the Anomalous Force Protein's
  carrier only, then for every player's hits to a hand (revision 50); the owner then saw arm and
  boot hits take many swings and asked that a gun-stock hit always kill.)
- **A gun-stock hit before** (PROVEN, data and code): a weapon's melee is a bullet of the weapon's
  `melee/data` (`damage_type blunt` in every live weapon, `k_damage` 1.5 for the ported guns, 2
  stock), so its hit is blunt (hit type 1). Its damage is `k_damage x k_min_damage_amount` through
  the player modifier 59 `melee_damage` (0x2E1C80): 1.5 for the ported guns. `damage_model::
  hit_body_part` (0x2E5080) takes the hit's damage (+34h) times (1 - invulnerability) into
  `body_part_parameters::hit_by_type` (0x329FF0) of the hit's zone (`human_hit_params.options`): the
  zone's armour for the type (none for blunt: 0.26g's armour has none, in no live item either), then
  its `bdb_coeff` sends the damage on to pools (0x33FA80, from the same damage). Blunt into pain:
  torso (`back`, `front`) x1 and x1 to `body`, head (`brain`, `face`) x1 and x1 to `head`, feet x0.8,
  hips x0.05, arms and forearms x0.05. Death is affect 0 of an empty `body`, `head`, `pain` or
  `infection` pool (`on_affect` 0x2DA480 sets the dying flag). So a torso, head or foot hit killed in
  one swing (a foot's 80 % of 1.5 is 1.2), and a hip or an arm hit took about 14 (0.075 each,
  less pain regenerating).
- **The zones of the body** (`base_character_full.skinned_model/hit_targets`, no patch changes it):
  the thigh (`RightUpLeg`) and the shin (`RightLeg`) are both `right_hip`; only a small box on the
  foot bone is `right_foot`; the upper arm and the forearm are `right_forearm`, the hand
  `right_arm` (and the left side the same). A "boot" hit is a shin or a foot hit.
- **The hit marker:** its size is the pain the victim lost (player vtable +40h, 0x1F3C70: the pain
  drop over pain's maximum, added to the collector, at most 1). A hip or arm hit's 5 % of 1.5 is the
  "extremely small x".
- **The change** (exe patch 108, Rust `hand_kill`): a lohook at `medical.hit` (0x2E50C7, in
  hit_body_part once invulnerability is applied, before armour; patch 94's site, ours first in its
  chain). A blunt hit with an attacker (hit +44h, a place, not 0FFh and not the victim's own)
  deals 1000 from there, on any zone. Its share of pain (50 at the least) empties pain, so the
  victim dies the game's own pain death in the same hit, as a limb shot kills; **the kill feed
  shows the pain-death mark** (patch 67's lightning: pain empty, body and head not) for a limb hit.
  Nothing in 0.26g stops it: no medicine protects against blunt (the painkiller's protection is
  `ranged` only), an active Onyx takes 75 % (pain still gets 12.5), second chance is never armed
  (the damage model's flag +3BCh is only ever cleared). A fall (blunt, its attacker the victim's own
  place: seen in the replay below) and every other hit are unchanged; the hit's own damage (+34h,
  which hit_impl's listeners get) is left as it was.
- **Simulation:** every client deals every hit: a `next` row.
- **Seen (revision 50, hands only):** match 0x73d0500e (Chemical Plant, 2026-11-05) replayed with a
  logging exe (`-match_replay`): a gun-stock hit to a `left_forearm` (by place 14) was set to 1000
  and its victim died in the same hit; the owner's swings landed on `left_hip`/`right_hip` (1.5,
  unchanged: no kill, the small marker) and on a `back` (pain and body emptied: a kill, stock).
- **Checked:** `check_hand_kill_026g.py` (Unicorn: the chain in the patched exe on made-up
  objects); `check_medical_skills_026g.py` (the shared site); `cargo test`, Clippy.
- **In game:** a gun-stock hit to another player kills him at once, wherever it lands; a fall breaks
  no rule it did not before.
