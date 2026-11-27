# A collimator's reticle chosen per scope (0.69d0's reticle modification)

**Version scope:** 0.69d0 as the reference, client 0.26g and srp's lobby server as the target. The
owner (2026-11-27): "implement dot modification for scopes like 0.69 had it"; "different colors,
sizes, patterns"; "you set it on a scope, which then applies to every weapon which has this scope".
Tags: PROVEN (read in the data or code named), LIKELY, OPEN, [srp] our choice.

Status: research and a design. Nothing is built.

The owner's answers (2026-11-27):

- free for every owned collimator;
- brightness 0 to 10 is included;
- the choice is made in the lobby only, with no keys in a match;
- the optics and the patterns: "as many as possible, but we can start with" the ten collimators,
  each with its own 0.69d0 lists.

So the lists are data, kept in each scope's content (`data/srp_reticle`). The exe and the server
handle any scope that has them, and nothing names the ten ids.

An earlier branch, `feat/scope-reticles` (983949c51, 2026-11-15, not merged), kept the choice per
weapon in the scope variant's kit bits. The variants are retired since then, and the owner now wants
the choice per scope, so that branch is not the base for this work.

## What 0.69d0 had

### The data (PROVEN, `gameplay/game.options`)

Only the ten collimator modules have the keys. A magnified scope has one reticle and no choice.

- `possible_reticles`: a list of `{path}`, the reticle models. Index 0 is the default.
- `reticle_colors`: a list of float3 colours.
- `attach_params.slots_to_add { id: reticle, locator: aim }`: the reticle is a model of its own,
  drawn at the optic's `aim` locator. The piggyback sights have `reticle_2` and `reticle_3`.

| module (our scope item) | shapes, in order | colours |
|---|---|---|
| `aimpoint_compm4_collimator` (2110) | dot_04, dot_03, dot_02, dot_01 | red, green |
| `eotech_552_collimator` (2109) | eotech_552_o (ring and dot), eotech_552_xr308, dot_04..01 | red, green |
| `eotech_xps2_collimator` (2114) | the same as the 552 | red, green |
| `ekp8cobra_collimator` (2108) | dot_04..01, ekp8cobra_cross, ekp8cobra_tick, ekp8cobra_tick_with_dot | red, green |
| `ncstar_4_collimator` (2113) | dot_04..01, ncstar_4_cross, ncstar_4_bullseye, ncstar_4_starburst | red, green, blue |
| `leupold_dppro_collimator` (2112) | dot_04..01, leupold_dppro_triangle | green, red |
| `docter_red_dot_sight_collimator` (2111) | dot_04..01 | green, red |
| `hensoldt_rsas_collimator` (2116) | dot_04..01 | green, red |
| `walther_competition2_collimator` (2120) | dot_04..01 | green, red |
| `belomo_pk01_collimator` (2115) | dot_04..01 | red, green |

- The dot's four sizes are four models (`ncstar_4_dot_04` .. `_01`). The other shapes have one size
  each. So 0.69d0 kept size and shape in one list; colour was a list of its own.
- The 13 reticle models are each one quad, under
  `models/weapons/modules/collimators/<optic>/reticles/<name>_reticle.model`. They share one material,
  `material_instances/weapons/modules/collimators/collimator_reticles`, which is a forward effect.
  Its emission texture is the atlas `textures/weapons/modules/collimators/collimator_reticles/collimator_reticles`
  (the shape is the atlas's alpha), with `collimator_reticle_noise` as its transparency texture.
  `emission_color` and `autoexposure_bias` are model parameters (`is_model_parameter`).
- The module items had a rarity. In `game.options` a collimator's `parameters/rarity` lists 0, 1 and
  2 (grey, green, blue). The orange (3), gold (4) and legendary (5) rarities came from drops
  (`modules_drop_options/modules_rarity`) and chests.

### The exe (PROVEN, `private/music-research/retail-069d.exe`; rows in `tools/re-kit/v069d/functions.tsv`)

- **Load.** The collimator's module loader (0x20023cda4) reads `possible_reticles` into a vector of
  model resources and `reticle_colors` into a float3 vector at module+0x1d0/+0x1d8.
- **State.** A collimator module item keeps three bytes: +0x302 the shape, +0x303 the colour, +0x304
  the brightness. They are part of the weapon state in the world (0x2006ce5e9 reads three bytes for
  each module). The module's visual has a copy at +0x2b0..+0x2b2. The shape and colour index into
  the module's lists modulo their length. Brightness is clamped to 0..10. The item's constructor
  (0x200134e10) LIKELY sets 0, 0 and 6.
- **Input, in a match.** Five key actions (the table at 0x200aa1c20, input bits 46..50):
  `reticle_next` (PageUp), `reticle_prev` (PageDown), `reticle_next_color` (Home),
  `reticle_inc_bias` (Insert), `reticle_dec_bias` (Delete). `process_reticle_input` (0x2006cd120)
  handles them:
  - **Shape** (0x2006d8420) changes only if the module's rarity (+0x240) is 3 or more, which is
    orange or better.
  - **Colour** changes only if the rarity is 4 or more, which is gold or legendary.
  - **Brightness** (0x2006d8520) has no condition. It steps between 0 and 10.
  - Each change goes through 0x200343c10. That becomes the player's timed record
    {module dict id, shape, colour, brightness}, which 0x200344190 applies to every module of that
    dict id on the player's weapons (0x200344360 applies it from the input history). The choice
    belongs to the module type, not to the gun.
- **Draw.** `weapon::update_transforms` (0x2002b9260) compares the item's bytes with the visual's
  copy each frame. For a byte that differs it calls the apply function for the shape (0x2002bafd0),
  the colour (0x2002bb4b0) or the brightness (0x2002bb780):
  - The shape picks the reticle model `possible_reticles[shape]`, which goes into the scene at the
    reticle slot (0x2006062c0, 0x200605690).
  - The colour and brightness go to that model's material parameters through 0x2006051c0. It takes
    {float3 `reticle_colors[colour]`, float brightness, flags 1 colour | 2 brightness}, which become
    the model parameters `emission_color` and `autoexposure_bias` (LIKELY by the flags; the
    material has no other colour parameter).
- **Kept across matches.** Each apply also writes {dict id, shape, colour, brightness} into a table
  of 512 entries in the game object (0x20011c250, +0x16cf8c0). `survarium::game::load_stored_scopes_data`
  (0x20011b940, a qualified diagnostic) reads that table from the local file `scopes.data`, and
  0x20011be80 writes it back. A new module visual takes its entry by dict id (the chunk at 0x2002c4790).
  The choice was the player's own, on their machine, per module type.
- **Who sees it.** The reticle model is drawn only in the holder's first-person aimed view.

### The UI (PROVEN, `flash_movies/lobby_menu.swf`, `hud.swf`, `localization/*/localization`)

- There was no lobby control. The module's tooltip (`ModuleTooltip`) lists, for a collimator,
  `st_change_reticle` "Change reticle:" and `st_reticle_brightness` "Brightness". From orange rarity
  it adds `st_reticle_collimator` "Type", and for gold or legendary also `st_reticle_color` "Color".
- The key bindings' names are `st_next_reticle` "Next collimator reticle",
  `st_prev_reticle` "Previous collimator reticle", `st_next_reticle_color` "Change reticle color",
  `st_inc_reticle_bias` "Increase reticle brightness" and `st_dec_reticle_bias` "Decrease reticle
  brightness". The texts exist in all of 0.69d0's languages.

So in 0.69d0 the reticle was neither a modification, an upgrade slot nor a shop item. Brightness
was free. Shape and size came with an orange module, and colour with a gold or legendary one. All
three were changed in a match with keys and remembered per module type in a local file.

## How 0.26g draws a collimator's reticle now (PROVEN, `attachments-scopes.md`)

- Each collimator's aimed model (pack `weapon-modules`) has the players' emitter baked in: a dot
  0.66 mm across, or the EOTech ring. It sits at the optic's `aim` place, scaled to the lit size of
  0.69d0's `ncstar_4_dot_04`. Its material is `weapons/modules_069/collimator_glowing_dot_red` (red,
  with the fade by angle removed, pack version 8).
- Only the local player's weapon ever adds an aimed model to the scene
  (`add_aimed_rifle_scope_to_scene` 0x244B60 from `weapon::draw` 0x244E10, first person only).
  A second sight's aimed model is asked for at the cook and drawn in place of its own (patch 161,
  `attachments.draw` 0x245017).
- The renderer cannot recolour one surface of a model. Per-model parameters reach only materials
  appended to a model (`scene_renderer::set_model_material_parameter_impl` 0x24E60). So a colour is
  a material of its own, and a shape or size is a mesh of its own.

## Proposed design [srp]

### The choice

- A reticle is three independent choices, as the owner asked, each an index into the scope's own
  lists:
  - **Pattern:** 0.69d0's shapes for that optic, the dot first and each family once (the EOTech
    ring, the XR308, the cross, bullseye, starburst, tick, tick with dot, triangle).
  - **Size:** four steps, 0.69d0's four dot sizes (`dot_04` to `dot_01`, `dot_04` the default).
    The same scale applies to every pattern.
  - **Colour:** the optic's 0.69d0 colours in its order. The first one is the default, red or
    green as 0.69d0 had it.
- The default (0, 0, 0) is what each optic shows today, so nobody's view changes until they choose.
- **Brightness:** not offered (see the questions).
- **Kept** per account and per owned scope item. A scope item is one item that can be on several
  weapons, so every gun with that scope shows the choice, which matches 0.69d0 keying by dict id.

### The server

- Store it in a new table `g26_scope_reticles(account, item id, pattern, size, colour)`. The type
  is `Reticle { pattern: PatternIndex, size: SizeIndex, colour: ColourIndex }`, parsed against the
  scope's lists, which the server reads from the game data: the scope item's config gets
  `data/srp_reticle { patterns: [...], sizes: 4, colours: [...] }` from an edit list.
- A request for a scope without lists, or an index past a list, is refused, and the server sends
  the account again.
- A choice that a later release's lists no longer cover becomes the default when the account loads.
- Selling the scope removes its row.
- No world change and no `simulation_changes` row: only the holder's first-person aimed model shows
  the reticle, and a match computes the same.

### Getting the choice to the client

The weapon kit's value is full: the wide layout has 3 spare bits (52..54), and its records cannot
grow. The plan is to send the choice with the scope item itself in the account's inventory, one
code per scope item. Which inventory field can carry it without the cook or the movie misreading it
is still OPEN, to be read at the start of the build. The fallback is a small srp lobby message. The
patch keeps a table of {scope dict id, reticle} for the local account, which serves the lobby
preview and the match in the same process.

### The exe

- The reticle becomes a model of its own, as in 0.69d0, owned by the patch.
- It is in the scene exactly while the optic's aimed model is: added after
  `add_aimed_rifle_scope_to_scene`, removed with `remove_aimed_rifle_scope_from_scene`, removed in
  `on_hide`, and moved with the aimed model (`weapon+660h`).
- Its transform is translate(the optic's reticle place) × scale(size) × [weapon+660h].
- Which model: (pattern, colour) of the fitted optic's dict id (`data/srp_scope`), looked up in the
  table. The second sight's aimed model gets its own optic's reticle the same way.
- The C or Rust follows the arena rules: one state enum for none, shown or hidden, and one
  transition function.

### The content (resource-porter, `weapon-modules@10`)

- The aimed models without the baked dot.
- 0.69d0's reticle meshes and atlas, one model per pattern for each of the three colours
  (13 × 3 at most). Each colour is a material of its own, made from version 8's unfaded emitter
  with only the colour changed.
- The record's reticle place for each optic, which is the `aim` place already measured.
- Check each pattern's centre against 0.69d0's, as the dot is checked today.

### The window

- The Attachments page, with a collimator in the scope slot, shows three steppers: Pattern, Size
  and Colour, each with ◀ ▶. Changing one calls a new ExternalInterface `srp_set_reticle(scope item, code)`,
  which the patch sends to the server; the window redraws from the account the server sends.
- The texts are new `st_srp_reticle_*` keys in the 12 languages. 0.69d0's own words seed the
  English, Russian, Ukrainian, German, French, Polish, Spanish and Chinese texts.

### What it takes

- resource-porter: the reticle port and the pack version.
- Patches: the reticle model's lifecycle, the table and the call, with Unicorn checks.
- Server: the type, the table, the request, the load and tests.
- Movie: the steppers and a Ruffle check.
- Localization, an edit list, `tools/test-patch`, and an in-game check of every pattern's centre.

## The build's contract [srp]

The parts are built separately and meet only here. Names in angle brackets are settled during the
build: `<patch>` is the next free patch number and `<pack>` the next free `weapon-modules`
version, both claimed only when the PR is nearly done.

### The data: `data/srp_reticle` in a scope item's config

An edit list, `scope-reticles-1.json`, made by `scripts/scope-reticles/edit.py` from
`scripts/scope-reticles/reticles.tsv`, adds one table to each listed scope item's config. Every
scope item without the table has no choice: the server refuses a choice for it, and the exe draws
its aimed model as it is.

```text
data/srp_reticle {
    patterns: [ { name: "dot", model: "weapons/reticles_069/dot" }, ... ]   1..16, index 0 the default
    sizes:    [ 1.0, 0.75, 0.5, 0.35 ]                                       1..8 scale factors, index 0 the default
    colours:  [ { name: "red", rgb: [1.0, 0.0, 0.0] }, ... ]                1..8, index 0 the default
}
```

- `name` is ASCII `[a-z0-9_]`, at most 23 bytes. The window's text for a pattern is
  `st_srp_reticle_pattern_<name>` and for a colour `st_srp_reticle_colour_<name>`.
- `model` is a static model of pack `<pack>`. It is a quad lit by its own emission, centred at its
  local origin, with its sight line along the model's +z as the aimed model's. Its size at scale 1.0
  is 0.69d0's. Its one material is `weapons/reticles_069/reticle`, whose `constant_emissive` and
  `constant_emissive_multiplier` are model parameters.
- `sizes` are factors on the model's own size, the same for every pattern. The default list is
  0.69d0's four dot sizes as ratios to `ncstar_4_dot_04`.
- Brightness is not in the data: it is 0..10 everywhere, default 6 (0.69d0's), and the exe maps it
  to the emissive multiplier.
- Index 0 of each list must look like today's reticle: the dot, or the EOTech ring, in the optic's
  first colour.

### The server

- `g26_scope_reticles(account INTEGER, item INTEGER, pattern INTEGER, size INTEGER, colour INTEGER,
  brightness INTEGER, PRIMARY KEY (account, item))`, one row for each scope item the player has set.
- `Reticle { pattern: Index, size: Index, colour: Index, brightness: Brightness }`. Each `Index` is
  bounded by the scope's list (`survarium::v026g` reads `data/srp_reticle` from the game data into
  `ReticleLists`), and `Brightness` is 0..=10. A missing row is `Reticle::DEFAULT` (0, 0, 0, 6).
- **The request.** `46 05 <kit> <host> <upgrade> <u64 value>` with host == upgrade == a scope item of
  the account that has lists, and the value
  `pattern | size << 8 | colour << 16 | brightness << 24 | 0x52 << 56` (0x52 'R' marks it).
  - The server parses the request into the existing second-sight side or a `SetReticle`. A value
    outside the lists is refused.
  - Either way the server answers with the account, then the reticles message.
- **The answer.** An srp lobby message after every account the server sends (sign-in, every
  answer). It is `<id>` followed by `u16 count`, then `count` entries, one for each scope item the
  account owns that has lists:

```text
u32 item id, u16 dict id, u8 pattern, u8 size, u8 colour, u8 brightness,
u8 n patterns × { u8 name length, name, u8 model length, model },
u8 n sizes × f32,
u8 n colours × { u8 name length, name, f32 r, f32 g, f32 b }
```

- A choice that the current lists no longer cover is reset to the default at load time.
- Selling the scope deletes its row.

### The exe (patch `<patch>`, Rust module `reticle`)

- **The table.** The patch keeps the last reticles message, parsed once into a typed table (by item
  id and by dict id), in the arena. It lives for the process, so the lobby and the match read the
  same table.
- **The window's side.**
  - `srp_set_reticle(item, pattern, size, colour, brightness)` (an ExternalInterface call from the
    lobby movie) sends the `46 05` above through `engine_send_apply_decal`, after checking it
    against the table.
  - `setup_inventory_data` gives each scope item's AS3 object a member `srp_reticle` when the table
    has the item: `{pattern, size, colour, brightness, patterns: [name], sizes: n, colours: [name]}`.
- **The drawing.**
  - The reticle is drawn on the local player's weapon in first person. While the fitted optic's
    aimed model is in the scene, and the table has that optic's dict id (`data/srp_scope`), the
    reticle model is also in the scene.
  - Its transform is the optic's `aim` place in the aimed model's frame (the modules table's, as
    the dot is placed today), times `scale(size)`, times `[weapon+660h]`.
  - Its material parameters are `constant_emissive = rgb` and `constant_emissive_multiplier`,
    taken from the brightness.
  - The same applies to a second sight's aimed model with its own optic.
  - Models are queried when first needed and kept for the process.

### The content (pack `<pack>`)

- 0.69d0's 13 reticle meshes, recentred and scaled by the porter, under `weapons/reticles_069/`.
  Each name is 0.69d0's without the optic and size prefix: `dot`, `eotech_ring`, `eotech_xr308`,
  `ncstar_cross`, `ncstar_bullseye`, `ncstar_starburst`, `cobra_cross`, `cobra_tick`,
  `cobra_tick_dot`, `dppro_triangle`.
- The atlas and the noise texture.
- The one material.
- The ten collimators' aimed models without the baked dot.

### The movie (`srp-026g/flash/lobby_menu`)

- The Attachments page has four steppers (◀ value ▶) under the scope slot: Pattern, Size, Colour
  and Brightness. They appear when the item in the scope slot has a `srp_reticle` member.
- A stepper wraps around its list. Brightness goes from 0 to 10 and does not wrap.
- A step calls `srp_set_reticle` with all four values and waits for the account to redraw.
- The texts are `st_srp_reticle_title` ("Reticle"), `_pattern`, `_size`, `_colour`, `_brightness`,
  and one key for each pattern and colour name, in the 12 languages of
  `client-patches/localization/`.

## More optics and patterns later

Each of these is a content change once the first build is in place. Rough sizes:

- **Every pattern and colour (blue too) on every collimator.** Only the `data/srp_reticle` lists in
  the edit list change. This works because each reticle model is drawn at the optic's own reticle
  place, and the size scale is an angle that does not depend on the optic. Size: S (an hour, plus a
  look in the game).
- **The PK-01VI** (2115) is one of the ten already.
- **The second sights' dots** (DeltaPoint 2112, RSA-S 2116 on the MPR45). They take their own
  item's choice. The exe also has to draw the reticle at the second sight's place, where patch 161
  draws its aimed model. Size: S to M (exe and Unicorn).
- **The magnified scopes' own reticles in other colours** (PSO-1 old and new, ACOG TA31 and TA01,
  Elcan, aperture, Leupold 3-7 and Mk4, 1P69). Their grid is a surface of their aimed model
  (`scope_aimgrid_s`, `scope_reddot_s`). Each scope would need:
  - its grid taken out into a reticle model, ported per scope;
  - a list in its config;
  - a check that the grid's centre and scale stay as they are.

  Size: M a scope, about half a day each.
- **The guns' own default optics** (the HK416's and MSBS's EOTech, the Kriss's and P90's Walther,
  and the other patch-19-era aimed models with a baked dot). Each needs the dot taken out of its
  aimed model and a reticle place. Size: M each.

## Questions for the owner (answered above)

1. **Which optics:** only the ten collimators, as in 0.69d0? Or should the magnified scopes (PSO-1,
   ACOG, Leupold, Elcan, aperture and the others) also get a reticle colour? 0.69d0 had no choice
   for them, and new reticles for them would be new content.
2. **Cost:** 0.69d0 locked shape and size behind an orange module and colour behind a gold or
   legendary one. Skhid's scopes have no rarity. The proposal is free for every owned collimator.
   The other option is a paid unlock per scope, and then its price.
3. **Patterns on every optic, or each optic's own:** 0.69d0 gave the cross, bullseye and starburst
   only to the NcSTAR, the ticks only to the Cobra, the triangle only to the DeltaPoint, and blue
   only to the NcSTAR. The proposal keeps each optic's own list. The other option is every pattern
   and colour on every collimator.
4. **Brightness:** 0.69d0 also had brightness 0 to 10, which was free. Should it be included?
5. **Keys in a match:** 0.69d0 changed the reticle in a match with PageUp, PageDown, Home, Insert
   and Delete. The proposal is the lobby window only, as the owner described.
