# PKP Pecheneg: newer animations

The owner (2026-11-01): "can PKP Pecheneg get newer animations too?" Pack `pkp-animations-069`
(resource-porter `port-animation-set`, `src/weapons_026/animation_set.rs`, manifest
`ports/animations-pkp-069d.json`; `scripts/pkp-animations/make_pack.sh`).

## What each build has

- **0.26g** (live; the model and clips came with base patch 19 from 0.29a3): idle, shoot, show,
  hide, reload (479 frames), two lobby clips, the shared AK clips (grenade, kick, recoil).
- **0.29a3**: the same clips byte for byte.
- **0.46c2**: idle, shoot, show, hide and reload made again: the same lengths, the same reload
  sound events (frames and order), a new left-hand grip (its hands `LeftHand` turned about 1.6 rad
  in the idle) with hand IK in the idle and shoot clips, the gun where ours holds it (the hands'
  `Weapon` bone and the gun's `offset` as ours). Its model: our bind pose and skeleton bones, its
  left hand's 16 `*_idle` points (where the IK puts the hand and fingers) 8 to 16 cm from ours, the
  right hand's ours. No `_reload` or `_reload_pivot` points in any build.
- **0.69d0**: 0.46c2's clips and hand points; its hands `show` clip adds a sound event
  (`weapons/show_hide_aimed_states/show_state`). The newest; this is the one ported.
- **Tactical reload:** none; every build reloads the PKP with one clip (state 303).

## What the PKP gets (0.69d0)

- `states.idle`, `fire`, `hide` and `reload`: 0.69d0's hands and item clips, at
  `animations/single/weapons/pkp_pecheneg_069d0/` (our clips of those names stay). The lobby clips
  are the same in every build and stay.
- `states.show` stays ours: 0.69d0's sounds the draw from a list our config keeps as an empty
  table (a key keeps its type: build-content's check), and its hands are 0.46c2's but for that
  event. As the draw ends, the left hand moves to the new grip with the idle's blend.
- Its models (`pkp_pecheneg` and the scoped `pkp_pecheneg_n_cobra_ekp_8`): the left hand's 16
  `*_idle` points 0.69d0's (position, rotation, bone; the bone index each model's own).
- Its reload: still one clip (303), `reload_time` 10 s; the reload's sound lists are ours.

## Checked (the porter refuses a gun otherwise)

The hands clips drive 0.26g's 70-bone player rig bone for bone; the item clips the bones of the
clips they replace; the sound lists the clips' events index are ours; at
the idle's start the gun's parts sit in the hands where our idle has them (5 mm, 0.02 rad).
Accepted: the ammo box's lid (`magazin_door`) 4 degrees from ours in the idle.

The match computes the same: the reload and hide clips keep their lengths (the reload's
speed is its clip over `reload_time`), the fire clip's shot event is at its first frame as ours.
