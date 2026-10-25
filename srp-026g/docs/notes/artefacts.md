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
onyx gives +1 second to activation time"; then 60 %, not 50 %)

- Passive as before: -5 % damage and piercing, each carried Onyx's adding up (x0.95 each).
- Active: -60 % damage and piercing (`active/damage_mul` and `armor_piercing_mul` 0.1 -> 0.4) for
  3 s, cooldown 60 s as before.
- Its key fires every ready Onyx; n fired together are each active 3 s + (n - 1) s (1 Onyx 3 s, 2
  Onyx 4 s, 3 Onyx 5 s) and end and cool down together. A cooling Onyx adds no time.
- Several active Onyx reduce a hit once (-60 %): only the first active Onyx of the slots takes its
  share; the others leave the hit as it is (before: x0.1 per active Onyx, all 3 s).
- An Onyx in its passive (on its cooldown) beside an active one still gives its -5 %.
- "Activation time" read as the active's duration: 0.26g has no delay before an Onyx works (the
  press sets it active on the same tick). Owner to confirm.
- The +1 s is patch 54's (`ONYX_EXTRA_MS`), not a config key; the 3 s is `active/duration`.
- The pop-up at activation (its activation hint, the local player only) says 60 % and the real time:
  "... by 60% for 4 s." for two Onyx. It shows once a press (not once for each Onyx fired) and stays
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
- How (content pack `artefact-glow@3`, `scripts/artefact-glow/make_pack.py`):
  - A particle effect is shown by the presenters of its `view` (the cook 0x25A730, 1 when
    absent): 1 the third-person presenters (the other players' view of a player), 0 the
    first-person presenter (the player the camera is in), 2 both. The stock glow has none: only
    the others saw it.
  - `effect_artefact_<kind>` becomes a composite (type 110) of the stock particle effect and a
    first-person one (`view` 0) playing `fx/artifacts/human_<kind>_fp`.
  - The first-person presenter plays it on the carrier's own body model (the same model and
    skeleton the others see, posed with the first-person animations; the head's parts are
    shadow-only there and parts 1 and 2 (LIKELY the torso and arms) drawn as foreground,
    `player::set_head_visibility` 0x1F37A0). The foreground is drawn after the world with a depth
    range of its own (`renderer::foreground_begin` 0x13E010), so a particle on the arms, drawn in
    the world pass, is hidden behind them: only what stands out of the arms' outline, what lights
    them, and the legs when one looks down can show (LIKELY: read from the code, not seen).
  - Version 1 (tested by the owner, 2026-10-25: the carrier saw no glow at all) was a lighter
    copy (no `up_light`, rays half as big and half as many, strokes 0.6 wide): little of it stood
    out of the arms. The presenter path was right (the check below runs it whole).
  - Version 2 (the owner: a tint on the weapon from the dimmed point lights; no particles on the
    arms, hands or legs; the gloves not lit) put `up_light` back with its lights at x0.35.
  - Version 3: the first-person particle system is the stock one value for value (emitters, sizes,
    rates, point lights, colours, the haze), with Head, Face and Neck off in every emitter's
    animated domain. `up_light`'s sprites are invisible by their material (`light_idle`:
    transparency and emissive 0): it is the point lights. `scripts/artefact-glow/view_cone.py`
    measures each emitter against the middle of the first-person view in two nominal poses (the
    rifle at the hip and aimed): the strokes and rays on the forearms, the haze on the arms,
    `rays_chest` and the lights come near it when aiming; kept, as the owner asked, to be judged
    in the game.
  - Not yet done: drawing the twin's particles with the foreground arms (no particle stage of
    0.26g draws in the foreground; 0.21d's symbols name only models' foreground stages:
    `stage_forward::execute_foreground`, `stage_lights::execute_foreground`,
    `stage_foreground::execute`), and the glow's fade at the active's end.
  - The end: the effect's time calculator is the artefact's (`artefact_base::effect_calculator`
    0x313640): finished when the artefact leaves its active state (4); both children with it.
- Checked: `client-patches/check_artefact_glow_026g.py` (Unicorn, the game's code): the cook,
  the presenters' constructors, the handler, the calculator, and the whole chain a match runs:
  each presenter built by its constructor, given to a player by `player::set_effect_presenter`,
  an activation sent by `game_effect_player` 0x2DE3A0 through the effect's own accept 0x2AE2E0.
  Version 2 not run in a game yet.
- The Sponge and the Rattle (PR #369) get theirs the same way once their effects are on main.

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
  sentences in the six localization tables; no lobby card. `artefact-texts-1.json` makes them say
  the numbers above (Onyx's stock 10 % and 95 % were wrong too; its active 60 % for `%s` s since
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
  the skill tree up to -0.10, gloves (modifications 1080..1084) and weapons (2070..2074) up to
  -0.10 each.
- The sum reaches -100 % only with five or six Springs all active and the other sources; the
  passives alone stop at -0.30 (-0.60 with all others). At 0 the scope is instant for those
  seconds; nothing faults (the timer 0x32FE40 snaps to the end; SSE exceptions masked) and every
  game computes the same.
- The owner: "Leave it" (asked whether to floor it at 25 % or lower Spring's values): no floor.

