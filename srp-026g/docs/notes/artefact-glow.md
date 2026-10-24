# Artefact glow (research, 2026-10-24)

How an artefact makes its carrier glow for others while its active is on. Research only:
nothing here changes the game. Addresses are VAs of the original 0.26g exe (image base 0x10000);
the names are in `tools/re-kit/v026g`. Tags as in README.md: PROVEN, LIKELY, GUESS.
Related: `artefacts.md` (patch 54), `sponge.md`, `rattle.md` (PR #369).

## What makes it

- A particle effect on the carrier's body, with lights in it. No material or shader change on
  the character, no post-effect, no HUD effect, no sound. PROVEN (data and code below).
- The item config's `data/effect` names a game effect, `gameplay/effects/effect_<name>.options`
  (the cook's `%s/effect_%s.options`, 0x2EC760). PROVEN.
- Each artefact effect is one particle effect: `effect/type` 114 (0.28d0 numbers it 117, same
  content), `particle_system`, `time_to_finish` 5, key frames 0 -> 0 and 1 -> 1, no `view` key.
  PROVEN.
- The three particle systems `particles/fx/artifacts/human_*.particle` are one design in three
  colours: one LOD ("LOD 0"), 10 emitters. PROVEN (decoded):
  - `up`, `rays`, `rays_chest` (billboards), trails `body`, `left_hend`, `right_hend`, `left_leg`,
    `right_leg`, `distortion` (disabled), `up_light`.
  - Each emitter starts its particles on the character's body: an "InitAnimatedDomain" action
    whose switches are the hit zones (FrontChest, BackSpin, LeftArm, LeftUpLeg, ...). The play
    call is given the player's animated collision object and skeleton (0.21d's
    `engine::world::play_particle_system` names both; LIKELY the same in 0.26g, below).
  - `up_light` has a Light action: a point light per particle (about 20 a second, each about
    1.7 s), radius about 1.2, intensity 0 -> 1 -> 0 over a particle's life, no shadows, the
    artefact's colour. The lights are LIKELY the engine's particle lights
    (`particle_engine::add_light` 0x14F200).

  | Artefact | data/effect | particle system | materials | light colour (RGB) |
  |---|---|---|---|---|
  | Lifebone (Larkspur) | artefact_lifebone | human_zhivokost | `*_green` | 0.10 0.77 0.00 |
  | Spring | artefact_spring | human_spring | `*_blue` | 0.00 0.40 1.00 |
  | Onyx | artefact_onyx | human_onix | `*_orange` | 1.00 0.48 0.00 |
  | Rattle (PR #369) | artefact_lifebone | human_zhivokost | `*_green` | 0.10 0.77 0.00 |
  | Sponge (PR #369) | artefact_spring | never played | - | - |

## The code path

```
key press (an input: every game runs it)
  -> artefact_base::action 0x313300       state 3, no cooldown -> state 4
       -> vtable +80h enable_active
       -> 0x313350 play_active_effect     emit, add to the holder's effect player
every tick:   effect_calculator 0x313640  finished = (state != 4)
every frame:  particle presenter visit 0x2AC400
                 new      -> play 0x2AC290 -> scene_renderer::play_particle_system 0x22660
                 running  -> update 0x2AC310
                 finished -> scene_renderer::stop_particle_system 0x22860
```

- Load (PROVEN): `artefact_core_cook::on_config_loaded` 0x2EE650 queries `data/effect` (class
  6Dh) -> `game_effect_emitter_cook::translate_query` 0x2EC760 -> the particle effect's cook
  (0x25A540, 0x25A730: `view`, default 1; `time_to_finish`; `time_mode`; `length`) ->
  `particle_game_effect_emitter` 0x29A0F0 (+13Ch the view, +138h time_to_finish). The artefact
  keeps the emitter at +290h (artefact_base 0x312FD0). Each artefact has its own emitter with
  one particle instance (the query's count defaults to 1, 0x2EC802): LIKELY.
- Start (PROVEN): `artefact_base::action` 0x313300: state 3, pressed, cooldown (+28Ch) 0 ->
  vtable +80h `enable_active` -> 0x313350 (the artefact, the time) -> +2A0h = the time, state 4.
  Patch 54's `fire_kind` calls 0x313350 for every copy it fires.
- 0x313350 (`play_active_effect`; 0.21d inlines it in `action` 0x1F4FCD) (PROVEN): with an
  emitter (+290h) and no live effect in +294h: `game_effect_emitter::emit` 0x329050 ->
  `particle_game_effect` 0x2AE1E0 (+30h the particle instance, +34h the view, +38h
  time_to_finish); kept in +294h; the holder ([+250h] the inventory, its +174h vtable+0Ch the
  player) -> `game_effect_player::add` 0x2DE090 on player+358h with (the effect, the time, 0, 2,
  the artefact's calculator at +268h).
- The calculator (PROVEN): `artefact_base::effect_calculator` 0x313640 (vtable 0xA15A68, the
  artefact at +8): {0, 0.0, finished = state != 4}. The effect has no timer of its own; it lasts
  exactly as long as state 4.
- End (PROVEN): `artefact_base::tick` 0x313200. Holder alive and state 4: `tick_impl` (+78h),
  then `active_effect_ended` (+8Ch) true -> `decrement_amount` 0x313530 (`disable_active`, the
  cooldown, state 3). Holder dead: 0x313530 for an active one, then `disable_passive`, state 5.
  Taken out of the inventory: `removed_from_inventory` 0x313490 (state 0). Nothing removes the
  effect by hand: the calculator says finished, the presenter stops the particle system (no new
  particles; the living ones fade, about 1.7 s), the effect node goes after `time_to_finish` 5 s.
- Show (PROVEN, the arguments' types LIKELY): `particle_game_effect_presenter` (vtable 0xA0D2E4)
  `visit` 0x2AC400 plays an effect only if its view (+34h) is the presenter's mode (+A8h) or 2.
  `play` 0x2AC290: `scene_renderer::play_particle_system` 0x22660 with the player's transform
  (player+118h, vtable+28h) and player+498h, player+F290h (by 0.21d's signature: the animated
  collision object and the skeleton model instance).

## How others see it

- From the world (PROVEN): the press is an input; every game runs `action`, emits the same
  effect onto the same player and presents it itself. Nothing is sent for the glow.
- Late join, rollback, replay (PROVEN in the code, not seen in game): `artefact_base::serialize`
  0x313670 writes the state and `m_last_activation_time_ms` (+2A0h) and, with an effect context,
  the effect pointer (`game_effect_ptr_wrapper::serialize` 0x2E5C90); the effect player
  (`game_effect_player::serialize` 0x2DDAD0) and the particle presenter (0x2AC720, 0x2AC870,
  which plays again what was playing) are saved too. `deserialize` 0x313760: with the context it
  links the effect again (0x2E5CF0, `game_effect_player::resolve_link` 0x2DDA80); without it, an
  artefact in state 4 plays its effect again from its activation time (0x313857). A late joiner
  and a replay see the glow of an active artefact.
- The carrier in first person (PROVEN): the effect's view is 1 (third person). The game world
  has one `first_person_game_effects_presenter` (0x2971C0; its particle presenter at +470h, mode
  0) and a row of `third_person_game_effects_presenter`s (0x297570; mode 1). The carrier never
  sees his own glow, nor its light. 0.21d had no particle presenter in first person at all.
- A spectator watching the carrier in first person: LIKELY none either (the first-person
  presenter follows the viewed player). Not checked.
- Through walls (LIKELY): the particles are drawn in the particle stage, depth-tested. The
  lights cast no shadows: they light what is within about 1.2 m of each particle, a thin wall's
  other side too.
- Distance (not proven): one LOD, no distance switch in the particle system; the emitters use
  the collision shapes, which every player has whatever his model's LOD.
  `stage_visibility::filter_and_sort_particles` 0x1B6F60 sorts by distance; no cut-off was found.

## Per artefact

- Lifebone (Larkspur): green, 3 s (its active). Spring: blue, 10 s. Onyx: orange, 3 s. Then
  about 1.7 s of fade.
- Several copies fired by one press (patch 54): each copy plays its own effect: as many particle
  systems and lights on the carrier, one colour. PROVEN (0x313350 per copy).

## The Sponge and the Rattle (PR #369)

- The Rattle glows: `data/effect` `artefact_lifebone` (`scripts/rattle/make_pack.py`: 0.20e's
  `effect_artefact_rattle` was Lifebone's particle; 0.26g has no `effect_artefact_rattle`). Its
  vtable keeps `artefact_base::action` (+2Ch 0x313300), so its active plays Lifebone's green
  glow for its 10 s: others see a Rattle's active as a Lifebone's. Right by 0.20e's data; for an
  artefact that hides its carrier the owner may want an effect of its own or none (an open
  question, not a bug). Nothing is missing: the effect and the particle are 0.26g's.
- The Sponge does not glow: `data/effect` `artefact_spring` (0.28d0's file as it is; 0.28d0 has
  no effect of its own for it, as its description key is Spring's). Its action (`sp_action`)
  never sets state 4 or calls 0x313350 (states 3 -> 6 armed -> 7 zone), and `deserialize` plays
  an effect in state 4 only. So the emitter is loaded and never played, as in 0.28d0, whose
  action 0x514580 arms it (client_patch.md 3.52). Its own visuals: `particle_on_spawn`
  pre_mochalka1, the zone's `activated_particle` mochalka_explosion and `idle_particle`
  fx/test/idle_particle, all in the pack. The pack also ships pre_mochalka2,
  pre_mochalka2_indoor and mochalka_idle, which no config names.

## To look at in game (next test round)

- Two clients: A fires a Spring. B sees blue particles on A's body and blue light near him for
  10 s, fading in about 2 s; A sees nothing on himself.
- B joins while A's Spring is active: B sees it.
- A fires a Rattle: B sees Lifebone's green.
- A fires two Springs with one press: two effects, brighter.
- How far it shows: B at 50, 100, 200 m; and behind a thin wall.
