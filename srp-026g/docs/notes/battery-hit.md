# Hitting with the battery (patch 143)

- **The owner (2026-11-14):** "start work to allow hitting with battery", "do implement some cool
  animation for that", "similar to dirty bomb".
- **Read as (for the owner to correct):** the melee key works while carrying a battery: a quick,
  punchy thrust of the battery itself (Dirty Bomb's quick melee: fast, the carried thing is the
  weapon, straight back to the carry), the carrier keeps the battery, everyone sees it.

## Today (PROVEN, static)

- A carried battery is the player's held item (+128h): `victory_item_core` with a state machine
  (`vostok::ai::fsm` at item +168h, its current state at +178h) of three states made by
  `initialize_logic` 0x301920, in this order: take (0x338940, kind +20h = 1), carry (0x338BE0,
  kind 3), put (0x338F00, kind 2). Transitions: take -> carry when the take's clip ended
  (0x372420 after its readiness [+28h]); carry -> put when the user switches away (0x301B30:
  held +128h != next +12Ch and +33Ch set).
- Melee is the gun's alone: `weapon_core::melee_pred` 0x2E4100 reads the user's actions
  (+330h bit 27) for the gun's state machine; the battery's states never read it. While a
  battery is held the gun is not the held item, so the key does nothing.
- A gun-stock hit: the melee state's "shoot" clip event (0x33AB50, subscribed by
  `weapon_core_melee_state_base::initialize` 0x33A990) emits one bullet (`emit_bullet` 0x322830)
  from the eye (player +44Ch) along its forward row times the gun's `melee/data/distance`
  (+578h, 1.7 m) with the gun's melee data (+53Ch: blunt, `combat_log_icon` 48).
- Every game deals every hit (lockstep): what is added changes what a match computes.
- Each state's clips come from the battery's config (`gameplay/victory_items/default.options`
  `states[i]`: 4 player clips, 2 battery clips); the cook loads every `states[]` entry
  (0x2FA1A0 loop) and the callback hands `initialize_logic` three arrays of 6 (0x2FA7C0).
- Only five battery clips exist (`key_objects`: show, idle, hidetest, idle_look,
  on_site_death_fwd); the only melee clips are the rifles' and pistols' `kick` (gun-stock).

## The rules

- **When:** carrying (the carry state), the melee key down (level, as the gun's: held, it
  swings again once a swing ends). Not during the take or the put, not while switching away,
  not in the air (the gun-stock hit's rule, weapon_core::melee_pred 0x2E4100: the posture's
  motion +24h 4 is a jump, player_logic_stand_jump_state 0x336B87; 0 on site, 2 run, 3 sprint,
  so sprinting may swing as the gun may; read from the battery's clip set). The gun's other
  rule, not aimed (+5E8h), has no carrier's counterpart. melee_pred has no +5ECh test.
- **Who:** every carrier, bots too (their `attack_melee` presses the key in melee range).
- **Two strikes in turn** (the owner, 2026-11-15: "can we get the second animation where left
  hand doesn't move far but right one does, this is a different type of hit", "it is like pushing
  the right side of the battery and hitting with that"): the ram (both arms push the battery
  out), the shove (the left hand the pivot, the right arm drives the battery's right end
  forward), then the ram again. A battery just picked up starts with the ram. Which comes next
  is the state its machine is in (The change): simulation state every game has, nothing new in
  the world's format. A third, the slam, was tried and is parked (The animation).
- **Effect:** at the swing's end, one melee bullet as the gun-stock hit's: from the eye, the
  carrier's primary gun's (slot 7) melee data and distance, aimed over where the strike's battery
  lands (below). The same hit for every strike. The carrier keeps the battery.
- **The hitbox** (the owner, 2026-11-15: "the hitbox should be bigger to cover more than a gun
  stock"): before the bullet, 15 rays from the eye with the game's own ray test, through a fixed,
  ordered grid of 5 x 3 points on the plane square to the look 0.7 m ahead (the battery's front at
  the end of its push, 0.045 s after the strike: posed as the game mixes it, it spans 0.28..0.70 m
  ahead, 0.38 m left to 0.27 m right, 0.06..0.35 m below the look; at the strike itself, the
  push's fastest moment, it is 5 cm short of that and 7 cm lower): 0.6 m wide around the look
  (-0.3, -0.15, 0, 0.15, 0.3 m), 0.35 m from the look down to the battery's lower edge (0, -0.175,
  -0.35 m); the look first, then by distance from it, left before right. Each ray is the
  bullet's own length (the gun's melee distance, 1.7 m: 1.0 m past the battery's front on the
  look, 0.72 m at the corners), so the bullet reaches whatever its ray found. The first ray whose
  first hit is a living player's body (a wall, a lamp or anything else first blocks that ray) is
  the bullet's direction; none: the look, as the gun-stock hit. One bullet: one victim a strike,
  its walls, teams and damage the native bullet's. Every game computes the same: the eye, the look
  and the physics world of the same tick. That grid is the ram's. The shove's battery lands
  elsewhere on that plane (`battery_hit_strikes_metrics.txt`), so it has its own
  (`Strike::grid`), built and ordered alike: its right end is 0.46..0.52 m right of the look,
  past the ram's 0.3 m; its left end is the pivot at the hip; and it stays near the height it is
  carried at, 0.25..0.75 m below the look, where the ram's is 0.06..0.46. The ram's grid one
  column to the right (-0.15, 0, 0.15, 0.3, 0.45 m), its rows the look, 0.25 and 0.5 m down.
- **Damage (assumed):** the gun's melee data is blunt with an attacker, so patch 108 kills at
  once wherever it lands. A battery's own damage would need its own bullet data: the owner's
  call.
- **Kill feed** (the owner, 2026-11-15: "whether we can add killfeed battery icon"): a battery
  icon (200) in the weapon's place, no headshot mark (patch 67's melee rule); statistics count a
  melee kill (13) of the kit, as before.
- **Knockback** (the owner, 2026-11-15: "also when hitting with battery the force push back should
  be stronger, half as strong as anomalous protein and it should also be additive with it"): a
  strike's ragdoll push adds half the Force Protein's knockback, +7.5, to what the killer's kit
  sums (force-protein-knockback.md): 1 + 7.5 = 8.5 times a gun stock's push, with the protein
  1 + 15 + 7.5 = 23.5, and always thrown as a heavy hit: the body ends about half as far as the
  protein's. A plain gun stock (x1), the protein's (x16) and every round as before.
  Visual only (the ragdoll is drawn by each game alone).

## The animation

- **The owner (2026-11-15, after playing 8dd7a44df):** the put clip as a thrust "is very bad",
  "it clips", "it doesn't have a force"; a cool animation "similar to Dirty Bomb" (a quick melee
  with weight: a clear wind-up, a snappy strike, follow-through, a camera punch); "it should
  properly do that, we don't even need a game for animating": authored offline on the real
  assets, previewed before any in-game test.
- **The first-person camera** (where the keys are judged): the Item bone. computed_head_transform
  (0x2DA4B0) stores bone "Item" (0x9DE38C) times the player's transform (+40Ch) as the eye
  (+44Ch); update_camera (0x1F3480) hands +44Ch to the game camera (0x2422B0) unchanged and sets
  the near plane to 0.05 (0x9CC878) times the zoom (1 unaimed); the projection is uro_fov
  degrees vertical (patch 22; 65.0 by default and in the owner's user.cfg). Item is a child of
  Root, so the body's moves do not move the view; the look pitch does (the battery clip set's
  additive `idle_look`, frame 18 level, Item 66 deg down at 36). First person draws the body
  without the head, the hud hands for the hands: the AK's idle drawn so puts the hand and the
  sleeve where a screenshot of the game has them.
- **What the stock carry shows in first person:** nothing, looking level. The battery's nearest
  edge is 40 deg below the view (the screen's edge is 32.5): it shows only when looking 10 deg
  down or more (`carry_idle_fp.png`: 0, 20 and 35 deg). The put's clips that the first swing
  played (`hidetest`, `hidetest_electrobox`) move the hands 7-9 cm and the battery a few cm and
  nothing else, so that swing showed nothing either, looking level.
- **The owner (2026-11-15, after playing b8a7acc3e):** "battery doesn't even move in the hands,
  it is just shoulders badly moving"; then "the battery should clearly move forward with hands
  straightening out".
- **How the game plays a battery state's clips** (from the exe):
  - the take state's constructor (0x338940) keeps the six clips: the two battery clips at +0E0h,
    the four player clips at +0D0h..+0DCh (two pairs); item_and_hands_expression (0x338B20, kept
    for the swing and the recovery) builds two lexemes: the player's from a pair (0x344F60, which
    sets its bone mask +58h to 2) and the battery's from its pair (0x345070, the mask left at the
    default -1, 0x31E9F1);
  - an animation moves a bone only if its lexeme's mask and the bone's share a bit
    (computed_local_bone_transform 0x3B20F2: `test [params+58h], mask`); the skeleton's masks
    (`scavengers_01.skeleton`, each bone's `mask`): 1 Root, Spin, Spin_1, Chest, Neck, Head,
    Weapon, Item, Pistol; 2 the clavicles, shoulders, arms, forearms, hands and fingers; 4 Hip and
    the legs. So a battery state's player clip moves the arms and nothing else: the spine, the
    head, the Weapon bone and the Item camera stay the stand set's base clip's
    (`ak/.../on_site_idle`, and its locomotion);
  - the battery hangs from the carrier's Weapon bone through its own skeleton: its `offset` bone
    (compute_carried_transform 0x344E40: Weapon x Ry x offset x Ry x the carrier's transform), posed
    by the state's battery clip;
  - b8a7acc3e's clips moved the spine, the Weapon bone and the camera, all masked out: in game
    its arms took rotations solved for a turned chest over an unturned one, and the battery,
    under the base clip's Weapon bone with the carry's battery clip, stayed put. The previews'
    "before" posed so shows that: the battery 0 cm off its carry place, the hands up to 65 cm off
    its handles.
- **The owner (2026-11-15, after playing 02465a20d):** "animation is still not smooth (but
  better!)"; then "it should feel human like", "with momentum and force".
- **What was uneven** was in the clips, not in how the game plays them. 02465a20d's clips over
  the cycle carry -> swing -> recovery -> carry, every millisecond (the game's animation time;
  `preview.py`, `metrics.txt`, `battery_hit_velocity.png`):
  - they began and ended with speed (a clip's first and last Bezier slopes were one-sided
    differences): the battery went from rest to 1.4 m/s in the swing's first millisecond, from
    1.1 m/s to rest at the strike, from 0.3 m/s to rest at the recovery's end;
  - the motion stopped twice on its way, cocked (0.07-0.10 s) and held (0.20-0.31 s): go, stop,
    go, stop, go, every stretch an ease in and out on one curve for all six values;
  - the pull was as violent as the push: 3.4 m/s against 4.2 m/s;
  - the right hand left the battery by up to 21 mm between two samples in the push. Its upper
    arm's Euler y angle reaches 85 degrees there, the other two then swing 100 degrees in 10 ms,
    and one cubic per 1/60 s cannot follow that; both hands were 13 mm off for the whole hold
    (the pose was past the arms' reach);
  - of the carrier only the upper arms, the forearms and the hands moved: the shoulders
    (clavicles, mask 2 too) never.
- **How the game times the states** (from the exe, with the 0.10b source's mixer, the same
  code; not watched in game):
  - a frame draws the world's last step: `game_world_core::move` 0x2D2E40 loads the newest kept
    state at or before the frame's time when the world is not at it (0x2D3568..0x2D357A) and
    steps on to that time (tick_events 0x2D21E0); `base_player::tick` 0x2DAA60 ticks the
    animation player in it (tick_animations 0x2D8EE0, `animation_player::tick` 0x3917D0, whole
    milliseconds), and the draw does not tick a living player's again (`player::mt_draw`
    0x24751C). So the clips are sampled at each frame's time, the arms and the battery at the
    same one: both lexemes play in the carrier's animation player, the battery's driven by the
    hands' (0x338B86);
  - the battery's machine is ticked only when the carrier's animations are selected again
    (`select_animations` 0x2D9690 -> tick_active_object 0x2D7650 -> the held item's vtable +2Ch,
    victory_item_core::tick 0x301240; base_player::tick's own call, +28h, is a `ret` for a
    battery, 0x75AF0): at a change of the action keys (apply_new_input 0x2D899D) and at an
    animation event that asks for it, as a take state's clip end does (on_animation_end 0x344830
    -> 0x338EF0);
  - carry -> swing: at the melee key's input, the swing's clip from its frame 0 at that time
    and at full weight at once (a take state's hands lexeme: the instant interpolator, time
    group 2, played once and frozen at its end, 0x338B32; the carry's is linear over 0.3 s,
    group 3, cyclic, 0x338DF7; a new clip starts where another stands only inside one group and
    not from a frozen one). The carry idle, the stand set's base clip and the swing's first
    frame are one still pose: nothing pops;
  - swing -> recovery: the end event's time is the start + floor(length / scale x 1000) ms
    (n_ary_tree_time_in_ms_calculator 0x3B6C5F); the scale (0x30D750) of 6 frames in 0.2 s is
    1.0000001, the time 199 ms. At that millisecond, inside the step, the machine goes on (the
    strike), the recovery's clip starts at its frame 0, and the step is played on: no gap. One
    millisecond of the swing's clip is never shown (6 mm along the motion at the strike);
  - recovery -> carry: the same, 349 or 350 ms on; the recovery's last frame is the carry's pose;
  - a carrier's own swing starts twice in his own game: his input is played at its own time
    until the next 50 ms tick, then moved onto that tick (snap_player_inputs 0x2D4E40,
    0x2D4F0A..0x2D4F3B) and the world played again from it: up to 50 ms of the swing are shown
    and it starts over. Every key but fire has this (`match.md` 5.2); a clip can only keep
    still meanwhile. 02465a20d's battery had moved 4.8 cm 25 ms in and 12.9 cm 50 ms in.
- **Authored** (`scripts/battery-hit/swing.py`) as the game plays it: the battery moved by its
  own clip's `offset` bone, the carrier's shoulders, arms and hands (his only bones the clips
  move) following it, over the carry idle as the game mixes it. A keyed curve for each thing
  that moves (the battery's place on three axes and its turn on three, each shoulder joint
  forward and up, each elbow's lean), each with its own times, a quintic between keys (place,
  speed and acceleration continuous). A forward ram with weight, its beats those of Dirty
  Bomb's two-handed objective shove (measured offline as a reference only, with the owner's
  leave; none of its files in the repo or the content):
  - the gather (to 0.10 s): slow from rest (0.9 m/s at most; 0.4 cm in the first 25 ms, 2.2 cm
    in 50), the battery drawn 4 cm back and 4 up against the body, the shoulders up and back
    before it, the elbows folding from 112/94 to 101/87 degrees and leaning out;
  - the drive (0.10-0.20 s): one push that only gains speed (8 g), the shoulders thrown forward
    ahead of the arms, the battery's turn trailing its travel; the strike (0.2 s, the swing
    clip's end) is on the fastest frame, 6.2 m/s, three quarters of the way out;
  - the follow-through (to 0.245 s): the battery's weight carries it on to full reach (29 cm
    forward and 24 up from the carry, the place the owner called "almost"), pulling the arms
    straight (elbows 155 degrees) and the shoulders forward until they check it (24 g); each arm
    gives its shoulder way as it nears its length (`REACH`), so no hand leaves its handle;
  - the settle (to 0.30 s): it sinks 1.5 cm at 0.5 m/s, its turn still arriving;
  - the way back (to 0.55 s): one reach, slow to start and slow to land (2.7 m/s at most), the
    shoulders easing last.
  The speed stops nowhere but for the turn at full reach. No torso lean and no camera punch:
  the spine and the Item bone are not the battery clips' to move (mask 1); giving the swing's
  lexeme mask 1 too would move the eye the strike is aimed from.
- **Stored** as the game stores its own clips (`clips.Channel.bezier`: one cubic Bezier segment
  per sample pair), with the motion's own slopes at the samples (none at the swing's start and
  the recovery's end, where it rests) and, bone by bone, a sample halfway wherever the cubic
  between two would leave the motion's pose by 0.02 degrees or 0.02 mm (`swing.track`: down to
  1/480 s for the right upper arm at the end of the push). The clips keep the motion within
  0.3 mm at every millisecond.
- **The shove** (`swing.SHOVE`; asked for as a hook, then "pushing the right side of the battery
  and hitting with that"): keyed about the left hand's hold, which stays (it moves 10 cm at most,
  the right hand 38 cm). The gather draws the right end 6 degrees back; the drive turns the
  battery about the left hand, its right end forward (450 deg/s and the right hand 5.9 m/s at the
  strike, the battery's centre 3.3 m/s), to 30 degrees of turn and 8 of roll at full reach: the
  right handle 36 cm forward and 11 up; each hand turns on its handle as its forearm comes round
  (`Strike.turning`), so no wrist bends past what a wrist does. The ram's times: the strike at
  0.2 s, the carry again at 0.55 s.
  The first take (played 2026-11-16) rolled the battery 24 degrees, which lifted the right end
  32 cm on its 39 forward, into the first-person view: the owner, "the battery goes up, not
  forward (its right side which is being shoved)", then "it can go a bit upwards, but it should
  move forward". The same turn about a left hand that stays in place takes the right arm to 0.99 of
  its length (its shoulder stands 20 cm behind the left), so the pivot gives way 9 cm to the
  left (0.96, the ram's 0.97). What it costs: looking level, the shove stays under the first-person view, as the
  carry does (a corner of the battery at the lower edge at full reach, 2 % of it; the first take
  showed 42 %); looking 15 degrees down, 24 % at the strike and 42 % at full reach.
- **The slam, tried and parked** (the owner, 2026-11-15: "maybe the third one when both hands
  are raised up and then shown down (we can at least try)"; 2026-11-16, of the previews: "let's
  go w/o slam for now, since it is slower, otherwise it is good"). Its keys stay (`swing.SLAM`,
  not one of `swing.STRIKES`): a lift of 28 cm, 18 cm forward, to before the face by 0.29 s, a
  short hang, the throw down from 0.33 s, the strike at 0.40 s (4.0 m/s, the fastest, 14 cm
  down), landing 23 cm under the top with the battery tipped 22 degrees forward, the carry again
  at 0.70 s. What was found:
  - slower: the lift needs its own times, the hit 0.4 s after the press and 0.7 s a strike (the
    others' 0.2 s and 0.55 s);
  - the arms alone cannot take it over the head: the chest and the head stay (mask 1), so any
    higher the right arm runs out of reach (the carry stance holds the right shoulder 20 cm
    behind the left), the battery goes into the helmet and through the first-person near plane,
    and it fills the view (at the top 55 % of it is in view, the lower half of the screen). A
    blow from over the head needs the spine and the camera: exe code, and the eye the strike is
    aimed from would move;
  - its battery lands 0.20..0.50 m below the look at the strike, 0.26..0.60 m 0.045 s on, under
    the ram's 0.35 m: its grid was the ram's columns with rows at the look, 0.25 and 0.5 m down.

  To bring it back: `SLAM` into `swing.STRIKES` (its clips, `battery_hit_slam*`, and the edit
  list's states 7 and 8), `Strike::Slam` with its times, grid and states 8 to 10 (its carry,
  swing and recovery) in `vostok::battery`, `battery_hit` and srp-validate. The branch had it
  whole at 5f706d8c0.
- **Eight clips**, four a strike, written in the engine's format with its own spline basis
  (`clips.py`): the carrier's `<name>` (its last frame the strike: 6 frames) and
  `<name>_recover` (10.5 frames) with the carry idle's bone names, the battery's
  `<name>_electrobox` and `<name>_recover_electrobox` with its own, `<name>` being `battery_hit`
  (the ram) and `battery_hit_shove` (`swing.STRIKES`); shipped by the edit list
  `battery-hit-1.json` (op `files_from_repo`, pinned by sha256, `make_clips.py --check`) as
  `states[3]` to `states[6]`, a swing and a recovery a strike (user_animations and animations),
  which the cook loads.
- **Checked offline** (`preview.py`, the renders in `private/renders/battery-hit/`: the films
  before beside after in first and third person, `battery_hit_velocity.png`,
  `battery_hit_joints.png`, `metrics.txt`), posed as the game mixes (the arms from the state's
  clip, the rest from the base clip, the battery by its own clip under the Weapon bone), every
  millisecond of the cycle: the largest change of speed in one millisecond is 0.23 m/s (was
  1.38); the hands stay on the handles within 0.2 mm (was 20.8); in first person the battery is
  on screen from 0.17 to 0.43 s (29 % at the strike, 72 % at full reach, both hands); nothing
  crosses the near plane in view and no torso, neck, head or helmet vertex is inside the
  battery. `engine_bake.py` runs the game's baker (0x3B3250) and evaluate_frame (0x3B3180)
  under Unicorn on the clips: within 1e-6 of `clips.py` on every track. The shove the same way
  (`preview.py <assets> <out> strikes`: `battery_hit_strikes*`, the ram beside it, and
  `battery_hit_shove_*.png`; the first take beside it, `battery_hit_shove_before_after*`): the
  largest change of speed in one millisecond 0.12 m/s, the hands within 0.2 mm, nothing through
  the near plane (every 1/240 s) or the body.
- **Seen by all:** the swing and the recovery are states of the battery's machine; the
  player's animation tree is simulation state, drawn for every view; first person draws the
  same body. The battery follows the Weapon bone each tick (0x344E40).
- **Sound:** the strike is the gun-stock bullet (melee data, material 74 `melee_blunt`), so a
  body hit plays stock `material/body/melee/melee_body_hits` (a 0.2 s thud), metal and wood
  their melee pairs; concrete, ground, stone and glass are silent, as for the gun-stock hit.
  Heavier candidates (STALKER SoC thuds, a port) are in `private/renders/battery-hit/sounds/`
  for the owner to pick; none wired.
- **Not seen in game yet:** the forward shove (its first take was played); the states' timing
  above is read from the exe, not watched; locomotion under the swing, the look pitch.

## Timing

- Either strike: press to impact 0.2 s (`battery_hit::TIMES`, the clip's 6 frames; the
  take state scales its clip to it), the strike on the swing's end, the battery at its fastest
  and three quarters of the way out (it is at full reach 0.045 s later); the recovery 0.35 s,
  then the next strike's carry: 0.55 s a strike. The gun-stock strike's clip (`ak/.../kick`) is
  6 frames too.

- The strike is when the swing's clip ends (the state's readiness, set by the animation
  player's end event, as the take's): 199 ms after the swing's start, in the same step of the
  world in every game. The machine goes on in that very millisecond (the animation above), so a
  state's time need not be whole ticks.

## Edge cases

- **Dropping or switching mid-swing:** the swing ends first (the state goes on only once its
  clip ended), then the put; the strike still lands (PROVEN in the check).
- **Put away mid-swing** (a delivery, a drop, a death: `victory_item_core::deactivate` 0x300F40
  calls the current state's finalize, 0x300FA5): the swing's finalize strikes only when its clip
  ended and its carrier lives (PROVEN in the check, the game's own deactivate). So a battery put
  away on the tick its thrust ended still strikes; a dead carrier never does.
- **Death mid-swing:** the swing answers "may be put away" as the carry does (its vtable +20h:
  yes once the carrier is dead, 0x338E90's rule; the take's own answers never, 0x397C20), so a
  dead carrier's battery is let go as from the carry (PROVEN in the check through
  is_ready_to_be_deactivated 0x301270). A new pickup starts at the take (initialize 0x300BF6).
- **Crouch, prone, sprint:** one clip for all (the config's 4 player clips are one), as the
  stock battery states.
- **A trap on the look:** a strike is the gun-stock bullet, blunt, so it disarms a trap as a
  gun-stock hit does (patch 151, trap-stock-hit.md). Not played.
- **Late join, resync, replays:** the strikes' states are the machine's fourth to eighth
  (indices 3 to 7), saved and
  loaded by the game's own fsm serialize (0x38CC90: the index, then the state's fields, a
  take's layout, a carry's for the shove's carry; PROVEN in the check: saved in the ram's swing,
  the shove's swing and the shove's carry, loaded by another game into its own state of that
  index, every field the same). The strike that comes next is that index and nothing else, so
  a loaded game goes on in the saver's turn. `srp-validate` reads 1 and 5 as
  `BatteryState::Carry { next }`, 3 and 6 as `Swing { strike }`, 4 and 7 as
  `Recover { strike }` (`BatteryStrike`). A bullet
  in flight is loaded with its gun from the shooter's slot (0x32BDA1..0x32BDAE), deactivated
  there too: the substitutions below need nothing the loading game lacks. Replays recorded
  before the release are refused (`simulation_changes.tsv` `next`).

## The change (exe patch 143, Rust `battery_hit`)

- `battery.logic` (0x2FA7E9, the cook callback after `initialize_logic`): for each strike
  (`vostok::battery::Strike`: Ram, Shove) a swing and a recovery, take states (0x338940)
  over the config's clips of `states[3 + 2i]` and the next, which the cook loaded (its
  queries_result, the callback's argument: state i's six clips at results 2 + 6i, the managed
  resources at +128h, 2E0h apart, as 0x2FA52D reads the take's), with the strikes' times
  (`battery_hit::TIMES`); and, for the shove, a carry state of its own before them
  (the game's constructor 0x338BE0 over the carry's clips, `states[1]`: EAX the battery, the
  room of 0E0h bytes and the clips on the stack, as initialize_logic 0x301969 makes the
  machine's). All appended to the machine's list: take, carry, put; the ram's swing and
  recovery (3, 4); the shove's carry, swing and recovery (5, 6, 7).
  Transitions: a carry -> its swing (a function-pointer functor, our function: the carrier
  alive, melee down, not in the air; after carry -> put), a swing -> put and its recovery, a
  recovery -> put and the next strike's carry, the shove's the machine's own carry again (byte
  copies of carry -> put's and take -> carry's functors), each tried once its clip ended. Every
  state is made before the machine is written. Results without states 3 to 6 (a game without
  the content, or with the ram's alone) are the patch's crash.
- **Which strike plays** is the carry the machine stands in: the carry after a take is the
  game's own, the ram's; each recovery ends in the next strike's carry. No counter, no field of
  the patch's: the alternation is the machine's current state, which the game's own fsm runs in
  every game, saves as its index and loads. Two carries that play the same clips are what it
  costs (a state a strike could not tell the next one from the carry alone). A strike's kind is
  told where it lands by its swing's place in the list (`BatteryState::swing`).
- A swing's and a recovery's identity, slot by slot: kind 3, the carry's (read by is_carrying 0x3018C8 and the clip
  set's key 0x3012EA: the carrier's body animates and counts as carrying through it); its
  vtable a copy of the take's (readiness +18h: its clip ended; expression +24h: the take's, the
  thrust starts at once, the put's blends in over 0.3 s, 0x339107; is_user_dead +28h shared by
  all three), with finalize (+0Ch: the strike, the swing's only) and may-be-put-away (+20h: the
  carry's rule) ours.
- A battery whose machine is not the three states, or no memory, or a carrier who cannot
  strike: a crash (`expect`), not a skip.
- The strike: `emit_bullet` with the arguments the gun-stock hit gives it (0x33AB50), EDI the
  gun (slot 7, else 10), the time the carrier's, its velocity along the aim. Nothing of the gun
  is written.
- **The aim** (PROVEN, static and in the check): each ray is bullet_physics_world::ray_test
  0x75E20 (engine_ray_test; abi.tsv) as check_collision makes the bullet's own (0x32D5EE: group
  10h, mask 48h, the hittable level and the hit models; unflipped). Its requester (the bullet in
  check_collision) is read only by a hit model's delegate (needsCollision 0x75BD0 -> 0x205E0 ->
  base_player::recompute_damage_collision_bones 0x2DA5B0), which reads only its [+8]+4, the
  shooter's place, and skips that player's body (0x2DA5E3): the strike's requester is a stand-in
  whose +8 is the carrier's bullet owner (+110h), so the bullet is made once, along the aim. A
  living player's body is what patch 103 and vostok::bullet call one: a bt_animated_rigid_body
  (vtable 0x9E2AF4) of material actor (+38h, 10); check_collision itself tells a hittable from the
  level by the object's game object (+0Ch, cast to the hit target, 0x32D705). A carried battery
  has no body in the physics world (picked_up_by_player 0x300D0E -> 0x300690 -> 0x301DB0 takes it
  out), so the rays through the battery's own place meet nothing of it. The aim sees the bodies
  where this tick's physics left them; the bullet's own flight (patch 103) meets a moving one where
  it stood when crossed, so a fast target at a ray's edge may be missed after all, as by the
  gun-stock hit.
- **The kill feed's icon.** The kill event's weapon icon (its node's +2) is the bullet's data's
  +50h (the gun's melee data: 48), else the damage source's +224h (generate_killed_event
  0x2DA35A..0x2DA382); the hud's handler (0x23B5F0) alone reads it, as on_player_killed's
  `object_icon`. A lohook at `kill_feed.weapon_icon` (0x2DA3CB, just before the event's killer
  and icon are written, EDX the icon, EBP the bullet; next to patch 67's 0x2DA3D1) makes it 200 for
  a strike's kill: a bullet whose data is its gun's melee data (the function's own melee test,
  0x2DA2BC..0x2DA2D4) and whose gun's player holds a battery at the kill. Not a copy of the melee
  data with its own icon: the game tells melee by the data's address (the bullet's constructor
  0x32BB59, the melee flag above, the damage functions), which a copy would break. Edge (a melee
  bullet's flight): a gun-stock hit landing after its shooter took a battery shows the battery; a
  strike landing after its carrier put the battery away the gun stock. The hud movie's
  `CombatLogItem` shows icon 200 as the bitmap `CombatLogBatteryIcon`: the battery's model in
  three-quarter view with its two cell wells, and the gun-stock icon's impact burst, in the weapon
  icons' style (`scripts/kill-feed-icons/make_battery_icon.py`, `kill-feed-icons.md`). No
  simulation change (the hud alone reads the icon).
- **The ragdoll's knockback.** The same rule (`vostok::battery::struck_by_battery`, one copy)
  tells a strike where the ragdoll records a living player's hit, its bullet's ray
  (`bullet.ray_traced`, `BulletRay::melee`): a blunt bullet is `Blow::BatteryStrike` by it,
  else `Blow::GunStock`; a round none (the hit's record, `shot::Hit::blow: Option<Blow>`). At the death
  `shot::knocked_back(push, blow, kit)` pushes 1 + kit + the blow's own knockback times the hit
  (`BATTERY_STRIKE_KNOCKBACK = FORCE_PROTEIN_KNOCKBACK / 2`; the crate's test ties
  `FORCE_PROTEIN_KNOCKBACK` to modification 1330's value in `modifications.tsv`, so a retune of
  the protein fails it until the constant follows). Read only, drawn by each game alone: no
  `next` row. Edge: as the icon's, a strike landing after its carrier put the battery away
  pushes as a gun stock. A strike is always a heavy hit (`Hit::heavy`: a push of `HEAVY_HIT`
  or more, or a battery strike; both places that tell heaviness ask it): alone its 68 N s would
  be capped as a bullet's (1.44 m on, not lifted). So the body ends 1.95 / 1.82 m on (front /
  side), 0.05 m up: about half the protein's 3.70 / 4.11 (a plain gun stock's 0.69 / 0.98);
  with the protein 6.18 m, 0.41 m up. Renders: `private/renders/battery-hit/knockback_{front,side}.{gif,png}`.
- **The bullet and the deactivated gun.** A carrier's guns are deactivated (deactivate
  0x2E2EE0: user +8 0, its clip set's posture +278h -> +20h 0). The bullet keeps its gun (+0Ch,
  the gun's item, 0x32BB12) and reads that user and posture through it all its life:
  - the stance at the constructor (`weapon_core::player_stance` 0x2E43A0, the gun's vtable
    +7Ch: the user's alive +33Ch, the posture's kind);
  - the melee damage, `weapon::damage` 0x2E1CF0 and `weapon::min_damage` 0x2E1C80, melee branch
    (the user's kit +115h, profile +6B8h): the constructor 0x32BB38, update_bullet_position at
    every flight step 0x32F09A and 0x32F0B6, the hit dispatch 0x32DA5A, the pierce or ricochet
    0x32E35C (the five callers).
  Four filtered lohooks at those reads (`gun.stance_user` 0x2E43A3, `gun.stance_posture`
  0x2E43B9, `gun.melee_damage_user` 0x2E1D00, `gun.melee_falloff_user` 0x2E1C99), their entries
  calling the handler only when the read gave 0 (never natively: no gun shoots while not held),
  put in the register what a held gun would give: the gun's player (its item's inventory +250h,
  the inventory's holder +174h, the player's +108h interface: 0x1F17A0 in both player classes'
  vtables, so every game has it, a late joiner's and a replay's too) and that player's held
  battery's posture (+1ACh -> +20h, the clip set states of the gun's own kind). No write, so no
  lifetime and nothing in a saved world. Should the gun have no player or its player no
  battery's posture, the handler panics with the patch's name (never in a match: kits are built
  with their inventory). At the two damage sites (after `fld [eax+8]`, after `fld1`) one x87
  slot is in use: their handlers do no x87 work, and must not.
  Not used: the gun's +594h (only set_user writes it; a
  joiner's never-held gun has 0); a lend of the user (weapon_core::serialize writes its in-hand
  part only when +8 is set, 0x2E3232: a lend alive at a save would misalign every loading game).

### The cook's requests

The battery's cook (process_loading_victory_item_core 0x2FA050) gathers its resource requests
in a buffer_vector on its stack with room for 20 (esp+8Ch..12Ch, set at 0x2FA05D and 0x2FA071;
8 bytes each): the item's user animations, the skeleton, then for each `states[]` entry its 2
animations and 4 user animations. The stock three states fill it exactly; a fourth state's
first push asserts "buffer overflow" (push_back 0x5FCB0, the break at 0x5FD0D) while a match
loads. Patch 143 grows the function's frame by 200h (`sub esp,244h` -> 444h at 0x2FA056) and
moves the buffer into that new top of it (esp+250h..450h after the three pushes): room for 64
requests, ten states. No instruction of the function reads its stack at or above the old
frame's top (its highest local is +248h; its arguments go by ebp), none of the three holds an
address (no relocation), and the query (query_resources 0xE5B60) and its result (0x101C9A:
0x2E0 bytes a result, sized by the count) take any count.

## The in-game tests

- **2026-11-14, build 56dfcf96d:** the owner pressed melee while carrying: a crash at 0x2E43A3
  reading 0x33C. The dump: the gun 0x7C932B48 (vtable 0xA04174) with user +8 0 and no posture;
  0x32BAE2 (the constructor's call of +7Ch), 0x3228A4 (emit_bullet), 0xA4373BD (the strike).
  The first check had stubbed emit_bullet whole.
- **2026-11-15, build 30f8ea538** (the gun lent its user and posture for the emit only): a
  crash at 0x2E1D00 reading 0x115. The dump: the bullet 0x02AC323C (+0Ch 0x7C944990 the gun's
  item, +40h the gun's melee data, +8 its shooter), the gun 0x7C944980 with user 0 again;
  0x32F09F (update_bullet_position after 0x2E1CF0), 0x32C2AC (bullet::tick), 0x32271A
  (tick_bullets), 0x321E77 (bullet_manager::tick). That check stubbed the live-list add, so
  bullet::tick never ran; and the spec's "the flight reads no user" scan looked for direct
  reads only, not for calls of 0x2E1CF0 / 0x2E1C80.
- Both are the check's controls now, in the original exe.
- **2026-11-15, build adf67b9c6 with its content** (patch_60 1a4aea2f57ba): a crash ~2 s after
  the match began loading, before any world: BREAKPOINT at 0x5FD0D, from 0x2FA20C (the cook's
  first push of state 3's animations). The dump: the request buffer's header (ESI 0x69DFF760)
  begin 0x69DFF76C, end = room 0x69DFF80C, 20 requests (the cook's requests above). The
  "cook ... failed" texture warnings before it are in runs without patch_60 too.

## Checked

- `check_battery_hit_026g.py` (Unicorn, the patched exe beside the original):
  - initialize_logic and the site make the eight states (each strike's swing and recovery over
    the cook's clips of its two states, scaled to their times; the shove's carry, a carry state
    of the game's over the carry's clips) and their transitions, the shove's recovery back to
    the first carry; the vtable copy has exactly slots +0Ch and +20h ours;
    results without the content's states, or with the ram's alone: the patch's crash, the
    machine untouched;
  - the key held: the ram, the shove and the ram again, each from its own carry, one
    strike at its swing's end;
  - fsm::tick from the carry takes the swing only for the melee key and a living carrier, the
    put first on a switch;
  - in the swing nothing until the animation player's end event (0x344830) of its own clip,
    another clip's end changes nothing; then the recovery (or the put) and one strike, made by the
    game's own emit_bullet and bullet constructor: the gun's item its source, the melee data,
    damage 1.5, its stance the one the original gives a held gun in that posture; the gun's
    bytes unchanged; the recovery's clip end: the carry, no second strike;
  - the bullet's flight with the game's own bullet::tick 0x32C180: a tick meeting nothing (every
    step reading the gun's melee damage), and one meeting a body (check_collision, the step to
    the point, try_pierce hitting the target once with the carrier as shooter and the gun's item
    as source, reading the damage again at 0x32DA5A); the gun never written;
  - put away mid-swing through the game's deactivate: the strike only with the clip ended and
    the carrier alive; a dead carrier's swing may be put away, a living one's not;
  - saved in the ram's swing, the shove's swing and the shove's carry and loaded by another
    game: its state of that index current, every field the same;
  - no gun in either slot (never in a match): the patch's own crash;
  - the melee key while the carrier's posture is a jump: the carry stays (sprint, run: the
    swing); a strike whose gun has no player: the substitution's named panic;
  - controls in the original exe: the constructor faults at 0x2E43A3, the flight at 0x2E1D00.
  - the aim: 15 ray tests from the eye as check_collision's (the melee distance, group 10h, mask
    48h, unflipped, the requester's shooter the carrier's); a body off the look within the
    battery's width (0.22..0.4 m right) or below the look at its height: the bullet along the
    first ray to it, one hit (the game's bullet::tick); a body just past its width (0.36 m) or
    under its lower edge (0.4 m down): along the look, no hit; a wall in front of the body: no
    hit; a body 9 rays meet: one bullet, one hit; the shove hits that body past the ram's width
    (its column 0.45 m right) and one under the ram's edge (its row 0.5 m down), and not one
    0.2..0.4 m left.
  Stubbed: the allocator, a clip's time scale, the animation event channels, the battery's
  transform calls, the bullet pool's room and live-list add, the physics world's ray test (a scene
  of boxes on the battery's front) and bounds, the struck target.
- the kill feed's icon (`check_battery_hit_026g.py`, the site run in both exes): a strike's kill
  event gets 200 (the original 48); a gun-stock hit's (the gun held), a round's (a battery held) and
  a bulletless one keep the icon generate_killed_event took. The hud movie
  (`scripts/kill-feed-icons/check-combat-log/check.sh`, Ruffle): icon 200 shows
  `CombatLogBatteryIcon` 25 x 19 in the weapon's place, the clip hidden; every other icon as before.
- `cargo test` and Clippy of the patches crate (each strike's grid: 5 x 3, the look first, by
  distance, left before right) and srp-validate (states 0 to 7 read as each strike's carry,
  swing and recovery, 8 refused).
- The knockback: `cargo test -p ragdoll` (a strike 8.5 times, with the protein 23.5, a NaN or
  negative kit counting nothing; every strike a heavy hit, a plain gun stock and the heaviest
  bullet not; thrown 0.4 to 0.6 of the protein's distance and lifted, with the protein farther
  and higher than the protein alone; the protein's +15 is modifications.tsv's), Clippy, `check_ragdoll_026g.py` and `check_battery_hit_026g.py` with
  the relinked arena. Not played.
- The clips: `make_clips.py --check` (the committed files are the keys'); the offline previews
  (no near-plane crossing, no body inside the battery, every frame); the edit list's 16 pins
  and that each of its four states names clips it adds. `release/build-content` not run for the
  shove (the owner was playing the content then).
- `check_world_roundtrip_026g.py`: its three worlds pass; its three coverage failures (0x252B48,
  0x310224, 0x33A51B, other patches' sites) fail the same on origin/main.
- Not proven: the pierce or ricochet at 0x32E35C (the same 0x2E1C80, its lohook, not run);
  other hittables' hit (+24h) with a strike's source item (lamps, a battery's body): not
  traced; the victim's hit_impl and the kill and hit events read the source item's slot and
  dictionary entry, never its gun's user (static, 0x2D9F97, 0x2DA1EA, 0x2DA3F7).
- The content loads (headless, input-free: a private Xorg dummy display, a replay of a
  recorded 0.26g battery match, level_01_2 mode 2, rev56 `1112-1832_09c170a5`, with patch_60):
  - the exe without the room (904d686bfada): the same BREAKPOINT at 0x5FD0D, 23 s in;
  - with it (db4672efa86b): the cook passes, the world loads, its 33 players are made and the
    countdown runs; at the first full resync (10.6 s into the match) a crash in evaluate_frame
    reading an animation channel. That is the old recording, not this content: the same five
    states over the stock clips (no new animation resource) play the replay through 8 resyncs
    and 150 s, and the stock hidetest clip's own bytes under the new clips' names crash as
    ours do. The players' animation state in a world refers to the animation registry
    (animations_registry::register_animation 0x2EF130), which new clip names change: a
    revision's replays play only in its own build (`simulation_changes.tsv` `next`);
  - the authored clips through the game's own baker (bake_time_channel 0x3B3250, the buffer
    sized by baked_animation_size 0x3B50D0) and evaluate_frame (0x3B3180) under Unicorn: all
    1260 tracks, 41 times each, within 1e-6 of `clips.py`'s evaluation (the previews').
- Played by the owner 2026-11-16 (70d28a2e6): both strikes in turn, the shove's first take. Not
  played: the forward shove and its lower grid.

## Release

- Exe: the patch; `simulation_changes.tsv` `next`.
- World format: battery states 3 to 7 (srp-validate, and so patch 87's check in every game
  and the server's).
- Content: `battery-hit-1.json` (the eight clips, the config's states 3 to 6). The exe needs
  it: without it every battery crashes the game at its cook (by design: it would play
  otherwise).
- Content: the hud movie (`CombatLogBatteryIcon`); without it a strike's kill shows the weapon
  clip's last frame (as an icon past the clip with no bitmap).

## Open questions for the owner

- The look: the two strikes (the previews; the owner, 2026-11-16: "otherwise it is good"). The
  slam is parked (The animation): back later with the body and the camera moving (exe code)?
- Kill at once (patch 108) or a battery's own damage?
- Speed: impact at 0.2 s, 0.55 s a strike; a cooldown?
- A sound: the stock body thud plays already; a heavier one (the SoC candidates) or a swing
  whoosh (the rifles' `melee_rifle`)?
- The kill feed's battery icon: the hud's own small battery glyph doubled
  (`private/renders/battery-hit/kill_feed_before_after.png`); a bigger or drawn one?
