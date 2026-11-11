# Face choice

The owner, 2026-11-08: "some survarium version added the ability to change faces, can we do the
same?", "we want faces from 069 + stk2 faces", "we need to make sure they all fit fine in all
helmets, or how this is done", "let's do that good"; 2026-11-09: "no need for loner" (no
neutral_03_head), "remove bandit stk2 face completely", "novice face shouldn't have a hat on
top", "let's first make faces look correct ... render them in 0.69 and iterate until they are
exact", "it should be the most thorough and full version", "with all features ported". A line
marked [pick] is a worker's choice the owner may change.

## The faces

- The eight 0.69d0 faces (head_default, head_01..07), exactly as 0.69d0 draws them: its meshes,
  all its bone weights, its own base, helmet and mask variants and hair, its materials input for
  input (only what 0.26g's renderer needs to give the same light: the smoothness map baked to
  0.26g's roughness curve, the metalness as 0.26g's Fresnel). No subsurface scattering: 0.69d0
  has none.
- One STK2 face: the Novice (neutral_01_head), his knit cap taken off and a bare scalp grafted on
  that looks like his own (the one deliberate change to a donor face). No STK2 bandit face.
- Face 0 is today's 0.26g head, for everyone who has not chosen. [pick] If 0.69's head_default
  is the same face, it may replace face 0; the content worker decides by look.
- The list is game data, never code: `gameplay/bodyparts/default`, `body_parts.heads[i]`, in the
  0.31a2/0.46c2 form: `heads[i].data`, `data_helmet`, `data_mask`, each
  `{base_model, part_name, additional_models}`. `heads[0]` is today's `body_parts.head`.
- The faces' count is `heads`' length; data without `heads` has one face (today's `head`).

## Who has which face

- One face an account, not a kit. Chosen in the lobby, free, any time out of a match (in one, or
  on the way to one, it is refused, as a preset's choice). [pick] 0.69 charged the rename price
  after the first login; we charge nothing.
- New accounts: face 0. [pick]
- srp keeps only a face the game data lists: a choice outside the list is refused, and a kept face
  the data no longer lists reads as face 0.
- Bots: one face a bot, fixed for the match (drawn over all the faces from the second the match
  is made and the bot's number in it). [pick]

## What is drawn

- As 0.69's cook, by the slots worn:
  - a worn mask: the face's mask variant (a mask wins over a helmet);
  - else a worn helmet: its helmet variant;
  - else the base face;
  - a missing variant: the base face.
- An item with `remove_head_model` shows no face, as today.
- First person is unchanged: the head is hidden as today (the head's records keep tag 0).
- The same in a match, a replay, the lobby's own character, the squad's characters and the
  victory screen's player.
- A face the client does not know (the data has fewer) is face 0: never a crash.
- Two players in the same gear with other faces are other skins: the face is in the combined
  skin's name (cache key). Face 0 adds nothing, so today's skins keep their names.

## Fit

- The faces are never reshaped to fit: the look is 0.69d0's first.
- They fit our items 0.69d0's way: its generic helmet and mask variants, and its per-face item
  models (`models.head_N`) for the items that have them. What else our live items need is the
  owner's call (the next step).
- An offline report in the pack build measures every face against every live helmet, mask,
  helmet+mask pair and torso collar, in the bind pose and the test poses; it does not stop the
  pack.
- The owner judges the look and the fit by eye, front and back.

## Wire

- Lobby request (the movie's): `3B 00 <start> <count>`, as the presets (`lobby.md` §2.16.4):
  - `2 097 000 000`: the state;
  - `2 097 000 010 + face`: choose that face (0..89);
  - each is answered with one `91` at `2 097 000 000`, its row `F1;<done>;<face>;<count>`
    (`done` 0 the state, 1 chosen, 2 refused; `face` the account's; `count` the faces of the
    game data).
  - A choice is answered with the account (`87`, every kit's revision bumped, so the lobby
    redraws its character) before the `91`.
- `87` (own account): one srp byte after `exp_next_level`, the account's face.
- `88` (a squad member's account): one srp byte after its revision, the member's face.
- `E4` (the match's options): srp's block entry tag 3, one byte a place (the roster's length),
  each place's face. Sent only when a place's face is not 0; no entry: every face 0, so older
  replays read as before.

## Client (exe patch)

- The faces are known by the player's name (the profile's +8h): the lobby's (own `87`, squad
  `88`) and the last match's or replay's options (`E4`, chunk 1). The lobby's are asked first.
  A name not known: face 0.
- The cook (`profile_skin_visual_cook::on_configs_loaded` 0x204560) makes the head's records in
  Rust (the native block 0x205659..0x2056F0 replaced, patch 123): `heads[face]`, its variant by
  the worn helmet (tag 5) and mask (tag 6) records, its base model, part and material, and a
  record for each additional model (tag 0, hidden in first person with the head); `f<face>_` is
  added to the skin's name for a face other than 0.
- A late joiner is face 0 to the players already in (its late-join options carry no srp block),
  but to its squad. [open]
- Without `heads` in the data the native head is made, as today.

## Lobby

- A face picker by the Character panel's paper doll: the faces in turn, the preview the lobby's
  own character. [pick] Short text in the dictionaries.

## Release

- Cosmetic only: no simulation_changes row.
- The faces pack, the server and the exe ship together, the server first. Until the pack ships
  the data has one face and nothing changes.
- An exe from before this release refuses an `E4` with the faces entry (its options check,
  srp-validate). The entry is sent only once a face but 0 is in play (a player's choice or a
  bot's draw), which needs the pack's `heads`: the server's data lists them only with this exe.
