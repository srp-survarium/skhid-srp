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
- Every game draws every player with that face: the match's players, its late joiners, its
  bots. The server's bots never join late (they are put in when a match is made; the registry
  takes none into a running match); a test bot (an srp-bot account) joins as any account, with
  its account's face.

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
- `E4` (the match's options): srp's block entry tag 5, one byte a place (the roster's length),
  each place's face. Sent only when a place's face is not 0; no entry: every face 0, so older
  replays read as before. A face the reader's data does not list (a recording read with a
  shorter list) reads as face 0, in srp's readers of recordings as in the exe.
- `E4` (a late joiner's proposal, `FF 'L' 'J' 01`, to the games already in; `late_join.rs`):
  after its profile, srp's block as after the options with the faces entry of its one place,
  `SRPX 05 01 <face>`. Sent only when its face is not 0; no block: face 0. The late joiner's own
  `E4` is the options, every place's face in it, its own too.

## Client (exe patch)

- A match's or a replay's players are drawn by its options (`E4`, chunk 1): a profile the cook is
  given that is a place of the options shown has that place's face, whatever the lobby says of
  its name. So every game draws a player alike, and a replay as it was recorded. The options'
  faces are kept from their read until the match's scene is left (game_world::on_deactivate).
- A late joiner's proposal is read at the dispatch (`match_dispatch.message`, after patch 114's
  check): its place, its name, its profile stepped over with the items dictionary (an item id
  looked up among the dictionary's own entries, as the late join's check does), the block after
  it. Its face is kept at that place of the match shown, and by its name among the match's late
  joiners: the late join cooks its characters from its own copy of the profile, which is no
  place of the options until it is installed. The block is then taken off the message: the late
  join's code (`lj_prepare`) reads the native profile to the message's end, as its check wants
  it. Bytes after the profile that are no such block are logged and left there: that check
  refuses the proposal.
- Any other profile is the lobby's (its own character, the squad's, a results screen's player),
  drawn by the lobby's messages by the player's name (the profile's +8h): the account's own
  (`87`), a squad member's (`88`; the eight told last). A name neither says: face 0.
- The cook (`profile_skin_visual_cook::on_configs_loaded` 0x204560) makes the head's records in
  Rust (the native block 0x205659..0x2056F0 replaced, patch 123): `heads[face]`, its variant by
  the worn helmet (tag 5) and mask (tag 6) records, its base model, part and material, and a
  record for each additional model (tag 0, hidden in first person with the head); `f<face>_` is
  added to the skin's name for a face other than 0.
- A worn helmet or mask is drawn from its config's `data_head_<face>` when the config has it (a
  table; `faces-pack.md`: the item's `data` with the face's models), else from its `data`, as
  0.69d0's `models.head_<n>` (0x204B36, where the cook has chosen the key it reads the item's
  models from). The face is the head's: the same tables, a face the data does not list face 0,
  which has no key of its own. `f<face>_` is in the skin's name then too, once.
- Without `heads` in the data the native head is made, as today, and every item's `data`.

## Lobby

- The owner, 2026-11-09: "the button for choosing a face should be to the left of LOBBY button".
  A FACE button (`st_srp_face`, every dictionary) in the player panel, 8 px left of LOBBY, its
  size and look (`survarium/controls/PanelButton`, the two buttons' class).
- The owner, 2026-11-09, playing the drop-down: "faces select should be in style of lobby
  select". A click opens the faces' window as LOBBY opens the lobby window, and of its kind (both
  `survarium/rooms/CardWindow`): in the middle of the lobby, a modal (Esc or the button again
  close it), one of the two at a time (opening one closes the other; a match's load closes the
  one open); the title (`st_srp_face`, upper case); a card a face (140 x 180), its portrait over
  the card and "Face <n>" in white on the dark band (`st_srp_face_card`, every dictionary), in
  rows of at most six, evened (11: six and five, the shorter row centred); the account's face
  framed gold. A press on a card chooses that face at once and closes the window, as a lobby's
  pick does, which shows the lobby's character. [pick] (`survarium/face/FaceButton`,
  `FaceWindow`, `FaceChoice`.)
- A card's portrait is the face's `heads[i].icon`: the cell `[i + 1, 16]` of the lobby's
  equipment atlas, which the faces pack draws (the movie does not read the game data, so column
  16 is `FaceWindow.ICON_COLUMN`, as `faces_026::ICON_COLUMN`), its middle 80 px across the card
  on a dark backdrop. An atlas without that cell: the face's number.
- Hidden until srp answers, and when the data has one face.
- A pick shows at once. One choice is asked at a time, and the last pick wins: when an answer
  comes and the player went on meanwhile, the next is asked. A refusal brings srp's face back.
  No answer in 5 s: srp's face, and the state asked again.
- The preview is the lobby's own character: the exe redraws it when srp's `87` comes. The
  camera stays as it is. [pick]
- Checked in Ruffle: `scripts/face-picker/check.sh <lobby_menu.swf> [work dir] [atlas.png]`.

## Release

- Cosmetic only: no simulation_changes row.
- The faces pack, the server and the exe ship together, the server first. Until the pack ships
  the data has one face and nothing changes.
- An exe from before the faces patch takes the new bytes and draws everyone with face 0: its
  options reader stops at the squads, the patches that read srp's block (65, 142, 154) step over
  an entry they do not know, and its `87` and `88` readers stop before srp's byte. No game runs
  srp-validate's options check (`messages::options`); it is the server's tests'.
- What keeps such an exe out is its patch revision: srp does not start with
  `SRP_MIN_PATCH_REVISION` under the newest patch of its game data (`check_min_patch_revision`),
  so with the pack live only a game of the pack's revision or later signs in. That holds for
  the faces when the exe with the patch ships no later than the pack.
- A late joiner's proposal with srp's block is the one thing such an exe refuses: its late join
  check takes the profile to the message's end (`late_join_validate_026g.c`), so the
  preparation fails and the joiner is not let in.
