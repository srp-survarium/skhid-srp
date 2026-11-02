# Artefacts (the owner, 2026-10-22)

Client patch 54 (README section 54, client_patch.md section 3.51) and the edit list
`scripts/content-edit/artefacts-1.json`. The stock code and numbers:
`tools/re-kit/v026g` and the decoded retail configs (`gameplay/items/artefacts/*.options`).

## The idea

- Three combat artefacts (Onyx, Spring, Lifebone) that are best used together, one of each.
- Stacks of one kind stay possible and add up: a stack is a stronger burst, not a longer one.
  Onyx is the exception (2026-10-25): a stack is a longer burst, not a stronger one.
- The original developers mostly buffed them after 0.47 and nerfed Onyx; here Spring and
  Lifebone are buffed from 0.26g's numbers, Onyx nerfed (2026-10-25).

## Every artefact

- Passive from pickup to death, as before; each copy's passive adds up with the others'.
- A press of an artefact's key fires every artefact of that kind in the slots that is ready
  (its passive, its cooldown 0): one key, all of a kind, their actives added up.
- A copy on its cooldown or already active does nothing on that press; it is not fired later.
- The key of a copy that is not ready still fires the ready ones of its kind.
- Different kinds have their own keys and cooldowns: Onyx, Spring and Lifebone fire on their own.
- Every game of the match computes this the same way (the press is an input).

## Spring

- Passive: movement speed +5 % (was +2 %), aim time -5 %.
- Active (10 s, cooldown 40 s): movement speed +10 % and aim time -10 % on top of the passive
  (+15 %, -15 % in all), stamina as before (no drain, regeneration x6), and bullets do not slow
  you (their stopping power; their stamina damage as before).
- The speed applies while aiming too (aiming's own speed modifier, 20, is 0 in retail).
- `max_carried_weight_mod` (0 in retail) is no longer read: its node is the passive's aim time.

## Lifebone

- Passive: each of its seven pools (body, head, both hands, both legs, pain) regenerates
  base + 0.1/s, up to 100 %, as before.
- Regeneration starts after the pool's own 8 s without a hit (the artefact no longer adds 2 s).
- Active (3 s, cooldown 45 s): hand and leg injuries removed as before, and pain, body and head full
  at once.
- Cooldown 45 s, was 15 s (the owner, 2026-10-23): since its active is a full heal, 15 s was too
  often (Spring 40 s, Onyx 60 s). `data/cooldown_sec` 15 -> 45 in `artefacts-1.json`.
- Nothing hard-codes it: artefact_base's constructor (0x312FD0) reads `cooldown_sec` x 1000 into
  +274h, the active's end (0x3135EC) starts the cooldown from it (times the holder's modifier 38),
  and the HUD slot's fill (0x313880) is 100 - time left x 100 / it. Patch 54 only reads the time
  left (+28Ch).

## Onyx (the owner, 2026-10-25: "nerf onyx to 3 seconds 50% damage reduction and every single new
onyx gives +1 second to activation time"; then 60 %, not 50 %; then "let's buff Onyx to 75% when
active")

- Passive as before: -5 % damage and piercing, each carried Onyx's adding up (x0.95 each).
- Active: -75 % damage and piercing (`active/damage_mul` and `armor_piercing_mul` 0.1 -> 0.25) for
  3 s, cooldown 60 s as before.
- Its key fires every ready Onyx; n fired together are each active 3 s + (n - 1) s (1 Onyx 3 s, 2
  Onyx 4 s, 3 Onyx 5 s) and end and cool down together. A cooling Onyx adds no time.
- Several active Onyx reduce a hit once (-75 %): only the first active Onyx of the slots takes its
  share; the others leave the hit as it is (before: x0.1 per active Onyx, all 3 s).
- An Onyx in its passive (on its cooldown) beside an active one still gives its -5 %.
- "Activation time" read as the active's duration: 0.26g has no delay before an Onyx works (the
  press sets it active on the same tick). Owner to confirm.
- The +1 s is patch 54's (`ONYX_EXTRA_MS`), not a config key; the 3 s is `active/duration`.
- The pop-up at activation (its activation hint, the local player only) says 75 % and the real time:
  "... by 75% for 4 s." for two Onyx. It shows once a press (not once for each Onyx fired) and stays
  on screen for that time. The text's `%s` is the pop-up's time on screen in seconds, which patch
  54 makes the active's time left; the hud writes it in. (%s, not `{s}`: the game's localizer (0x1FEF90, every text set_parameterized_message shows) makes `{NAME}` the key of the action NAME (`{USE}`) and any other name nothing, and drops a `}` after a character; the
  owner's first test showed "for  s.".)

## Your own glow (the owner, 2026-10-25: "you see yourself shining (hands and body if you look
down) when you use artifact? Note that your vision should still be clear")

- The carrier sees his own active artefact's glow: on his arms and hands, and on his legs and
  body when he looks down. Everyone else sees the glow as before.
- Each kind its own colour, as in the third person: Lifebone green, Onyx orange, Spring blue.
- Vision (the owner, 2026-10-25: "use the original effects seen by other people, but with the
  head cleaned somewhat"; "we will see, a bit of that is fine"): the carrier's glow is the one
  everyone else sees, without what sits on the head, face or neck (the camera is in the head).
- It starts and ends with the active, as the third person's (no glow on the cooldown).
- Visual only: no exe change, nothing in a match, nothing sent; no `simulation_changes` row.
- How (content pack `artefact-glow@4`, `scripts/artefact-glow/make_pack.py`):
  - A particle effect is shown by the presenters of its `view` (the cook 0x25A730, 1 when
    absent): 1 the third-person presenters (the other players' view of a player), 0 the
    first-person presenter (the player the camera is in), 2 both. The stock glow has none: only
    the others saw it.
  - `effect_artefact_<kind>` becomes a composite (type 110) of the stock particle effect and a
    first-person one (`view` 0) playing `fx/artifacts/human_<kind>_fp`.
  - `human_<kind>_fp` is the stock particle system exactly (its ten emitters, sizes, rates, point
    lights, colours and haze), with Head, Face and Neck off in every emitter's animated domain.
    The pack maker compares the whole tree with the stock one and refuses any other difference (it
    proves that on a changed rate, an emitter left out and the head left on).
  - The first-person presenter plays it on the carrier's own body model (the same model and
    skeleton the others see, posed with the first-person animations; the head's parts are
    shadow-only there and parts 1 and 2 drawn as foreground, `player::set_head_visibility`
    0x1F37A0).
  - `scripts/artefact-glow/view_cone.py` measures each emitter against the middle of the
    first-person view in two nominal poses (the rifle at the hip and aimed): the strokes and rays
    on the forearms, the haze on the arms, `rays_chest` and the lights come near it when aiming;
    kept, as the owner asked.
  - The end: the effect's time calculator is the artefact's (`artefact_base::effect_calculator`
    0x313640): finished when the artefact leaves its active state (4); both children with it.
- History: version 1 (a lighter copy) and version 2 (with `up_light`'s lights at x0.35) showed
  too little in the first person; version 3, the stock glow without the head, was tested by the
  owner and approved (2026-10-25: "this version is good"). Version 4 is the same files.
- Checked: `client-patches/check_artefact_glow_026g.py` (Unicorn, the game's code): the cook,
  the presenters' constructors, the handler, the calculator, and the whole chain a match runs:
  each presenter built by its constructor, given to a player by `player::set_effect_presenter`,
  an activation sent by `game_effect_player` 0x2DE3A0 through the effect's own accept 0x2AE2E0.
- The Sponge (PR #369) has `data/effect` `artefact_spring`: it gets Spring's twin once both are
  merged. The Rattle (`effect_artefact_rattle`, `human_rattle`) needs its own composite.
- Later (the owner): the glow fades out at the active's end instead of stopping at once (board
  T82; a debug build of the stop path is on the branch `debug/artefact-glow-stop`).

## Several kinds at once (the owner, 2026-10-25: "what happens with colours, do they mix? We
should mix them just as retail does")

- 0.26g: each active artefact plays its own effect, side by side; nothing replaces another and
  there is no cap (`game_effect_player::add` 0x2DE090 appends an item to the player's list).
- The glow's sprites (rays, strokes, sparks) are additive (`blend_mode` 2: source alpha, one;
  0x1CD1D2's switch), so where kinds overlap their colours add (green and blue to cyan, all
  three towards white); the shared `light_idle` sprite is alpha-blended (mode 1), the haze a
  distortion; each kind's point lights add as lights do.
- Retail does the same: 0.27d to 0.31a2 list the same three effect and particle files (by name,
  their archives' listings); 0.46c2 and
  0.69d0 (configs read from their archives) keep one particle effect per kind
  (`effect_artefact_<kind>`: `fx/artifacts/human_<kind>`, no `view`, no combined or synergy
  effect; 0.46 adds Wood shine's own). 0.69's `human_sinergia` particle is no artefact's
  `data/effect`.
- So nothing changes in the third person; the first-person glows overlap the same way (each
  kind its own additive effect).

## Texts

- The only texts: each artefact's pickup hint (the passive) and activation hint (the active), fixed
  sentences in all language TSVs; no lobby card. `client-patches/localization/*.tsv` makes them say
  the numbers above (Onyx's stock 10 % and 95 % were wrong too; its active 75 % for `%s` s since
  2026-10-25).

## Release

- Changes what a match computes: a `next` row in `crates/session/src/v026g/simulation_changes.tsv`,
  the server asks for the release's revision.
- The exe reads `passive/aim_time_mod` and `active/aim_time_mod`: it needs the edit list's
  resources.db patch (the release's exe requires its patches, patch 49).

## Aim time at its limit (the owner's decision, 2026-10-23)

- The aim-time modifiers add up: the getter 0x2D5AE0 (index 49) gives the weapon's `aim_time` x
  max(0, 1 + the sum), floored at 0 by the game itself; scope-in 0x2E1B50 and scope-out 0x2E1BF0
  use it. Sources on real data: Spring -0.05 passive / -0.10 active each (artefact slots 13..18),
  the skill tree up to -0.10 and gloves (modifications 1080..1084) up to -0.10. A weapon's own
  aim-time roll (2070..2074, up to -0.10) is not in this sum: since weapon-rolls-1 it scales the
  weapon's `aim_time` in the item cook, so it multiplies the result (weapon-rolls.md).
- The sum reaches -100 % only with five or six Springs all active and the other sources; the
  passives alone stop at -0.30 (-0.60 with all others). At 0 the scope is instant for those
  seconds; nothing faults (the timer 0x32FE40 snaps to the end; SSE exceptions masked) and every
  game computes the same.
- The owner: "Leave it" (asked whether to floor it at 25 % or lower Spring's values): no floor.


## A pickup in the hand (the owner, 2026-11-02: "when artifact is picked up, quick animation is played, artifact is brought in hand, lingers there for a second, and then goes back and into the container")

The rules:

- When a container's search ends, the weapon goes down, the artefact comes up into the right palm,
  stays there a second, goes back down, and the weapon comes back.
- Weapon away, can't shoot: while it runs, fire, aim, reload, sprint, weapon keys, grenade, melee
  and quick use do nothing (patch 78's carry filter); nothing pressed ends it early.
- The search is shorter by the time it takes, so search and hold together take what the search took.
- Seen by everyone, in both views: the carry's pose (patch 78), the same arm for the damage bones.
- A hold animation, not a pickup one: the carry's raised-palm pose, nothing new authored.
- The artefact is in its slot from the take, as before: its passive starts at once, its key works
  as soon as the weapon is back.

The numbers:

- The show is 2 s (`carry::SHOW_MS`): weapon down 0.2 s (the carry's `LOWER_WEAPON_MS`), artefact
  up 0.3 s, held 1 s, down 0.3 s, weapon back 0.2 s (`RESTORE_MS`).
- The search: every container's `artefacts_search_time_sec` 6 -> 4 s (927 containers of the stock
  maps) and 5 -> 3 s (54), `scripts/content-edit/artefact-hold-1.json` (`set_every`, every map's
  `client_project`). The key is an int; the modifier `artefact_search_time` (static_info index 7)
  still scales the new time.

What happens when:

- Death during it: the artefact stays in its slot (no drop: it was never offered). With the
  artefact up (the gun away) no gun falls, as from a carrier's hands; while the weapon is still
  going down or already coming back, it falls as from any player's.
- A battery taken (its own carry has the hands): the hand goes down at once, as the carry's (`carry_allowed`).
- Taken with no gun in hand: no show; the artefact goes to its slot as before.
- Taken while the H carry is on (an offer, raising, lowering): no show; that carry goes on.
- The artefact gone from its slot during it: the hand goes down empty.
- Use still works (another container, a battery, a teammate's offer); a teammate cannot take the
  shown artefact (it is no offer).
- Slots full: the game puts the new artefact in place of the first slot's (0x2FC897), as before;
  the new one is shown.

How:

- Site `artefact_container.taken` (0x31C8E2, in transfer_artefact after take_artefact: ESI the
  artefact, EBX use_execute's user) -> `artefact_carry::on_container_taken`: the carry event
  `PickedUp`, the phase `Showing { token, start }` of `crates/artefact-carry` (one state with its
  own timeline, not flags on the carry's phases).
- Every game computes it (the take is in the simulation); the phase is in the carry's 32-byte
  snapshot (tag 9) with the world: late joins, resyncs and rollbacks restore it. A `next` row in
  `simulation_changes.tsv`.
- Bots: no bot searches artefact containers (servers/match-server bots); nothing changes for them.
- Checked: `cargo test -p artefact-carry` (timeline, presses, snapshots, split ticks, the cases
  above) and `check_artefact_carry_026g.py`'s `container_show` (the original transfer_artefact and
  take_artefact, free and full slots, no gun, death mid-show). Not yet seen in a game.
