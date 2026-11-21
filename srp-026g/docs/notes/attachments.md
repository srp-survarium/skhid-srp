# Weapon attachments: one system for scopes, muzzle devices and grips (design and stage 0)

**Version scope:** client 0.26g (build 2777; VAs are image VAs, base 0x10000), 0.69d0 as the
reference for content. Tags: PROVEN (read at the address or in the data named), LIKELY, OPEN.
Nothing here is built yet: this is the plan and the research that sizes it. The transport is the
weapon kit (`weapon-kit.md`, patch 68): slot 3 a scope, 4 a muzzle device, 5 a grip.

## The ask

The owner, 2026-11-20: every grip and every muzzle device 0.69d has, except suppressors, on every
gun 0.69d fits them to; and the scopes moved onto the same system, "start almost anew and learn our
0.69 lessons". Stats that need new exe readers are simplified for now.

## Why not as before

A scope pair today is three baked copies (`weapon_scopes.md`): a variant item, the optic's models
moved by a translation, and a copy of the gun's body with its `aim` locator moved. Players' data
holds 1,672 gun bodies, 1,462 of them made for scope variants, and each misalignment was fixed pair
by pair. The earlier muzzle work drew one model at a locator at run time but cut the built-in brake
out of our meshes by a tolerance rule that failed on some guns.

## The model: 0.69d's own

In 0.69d a gun is a base mesh with slots (`slots[] {id, locator}`), and every other part is a module
in a slot, the gun's own brake, forend and sights too (`default_modules`). A module's rules are a
small vocabulary (counted over its 416 modules): it adds slots, moves a point of the gun to its own
locator (`fire_point`, `aim`), shows another slot's part in its alternative shape or closes it,
hides slots while aiming, sets the shot effect, and a grip carries 16 hand locators.
`module_to_item_links` says which gun takes which module.

```text
enum Kind { Scope, Muzzle, Grip }            // what a player fits: kit slots 3, 4, 5
enum Role { Fitted(Kind), Carrier, Stock }   // Carrier: a rail bracket, clamp, tactical forend or
                                             // barrel a fit needs; Stock: the gun's own part
fn resolve(gun, fitted) -> Result<Loadout, Refusal>
struct Loadout { body: Whole | Bare, parts, eye, fire_point, left_hand, shot_effect }
```

One `no_std` crate holds the tables' types and `resolve`, linked by the porter (to build, check and
render), the servers (to accept an attach) and the client patch (to draw). Carriers come with the
fit. A gun has two bodies: `whole` (its stock parts in the mesh, drawn when nothing is fitted) and
`bare` (drawn with the stock parts as separate models once anything is fitted). The foundation is
every gun on 0.69d0's own model, whose slots the modules are made for.

## Stage 0: what the client does, and where a patch writes (PROVEN unless tagged)

### The gun's three points

- `weapon::load_weapon` 0x243A30 (only caller: the weapon cook, 0x24B0DC) reads three locators of
  the gun's model by name, once: `barrel_point` to weapon+6E0h, `scope_point` to +744h (the
  barrel's when missing), `aim` to +7A8h (the scope's when missing). A record is 64h bytes: name
  (20h), the offset matrix +20h, the bone index +60h.
- Each record has one reader a frame, `weapon::calculate_locator` 0x243BC0: out = extra x
  rotation_y(pi) x offset x bones[bone] x world (run in Unicorn on the stock exe, 60 cases). The
  half turn is a static built inside it.
- **The scope's frame:** `weapon::draw` 0x244E10 evaluates +744h into weapon+660h and places the
  scope's idle or aimed model there. One path for the first view, the third view and the lobby.
- **The eye:** `weapon::calculate_aim_alignment` 0x244D40 (one caller, 0x295AAD, first view while
  aiming) evaluates +7A8h with the `aim_offset` matrix +6A0h; the gun is moved until that frame
  sits on the camera bone. The scope item has no eye of its own (`rifle_scope` holds two models and
  four numbers).
- **So both scope overrides are data written once.** After `load_weapon` returns (0x24B0E1, eight
  relocatable bytes) a patch writes the +744h record (the slot's frame, a chain composed
  statically) and the +7A8h record (the optic's own eye through the same frame). The optic then
  stands at one fixed matrix of the eye whatever the gun's bones do (Unicorn, 40 records x 4
  poses): centred by construction.
- To mind: the shell-ejection effect plays at the scope's frame +660h (0x2443EE), so shells follow
  a moved scope frame unless that site takes the old one; an eye on the optic's bone no longer
  shows that bone's motion while aiming (LIKELY); `on_culled_player_draw` 0x245250 places things at
  the player's root without the locators; with `hide_weapon_on_aim` the gun's model leaves the
  scene (0x244C08) and separately drawn parts must follow it.

### The left hand

- The hand IK and the finger corrector each keep their own copy of the gun's hand locators.
  `hand_to_weapon_ik_solver::set_weapon_locators` 0x392AE0 (from `weapon::activate`, 0x244644)
  fills eight records a hand (idle, reload, reload pivot, lobby; first view, then third) and
  evaluates one on its gun bone each frame (0x392E30).
  `fingers_to_weapon_corrector::set_weapon_model` 0x390620 (from `load_weapon` 0x243BB6, and again
  at every change of view, 0x298664) keeps 15 matrices a set, each finger's locator relative to its
  parent's.
- A clip's `left_hand_ik` event picks the target: v/2 = 0 the hand's own place, 1 the gun's
  `left_hand_cont` bone, 2..5 the idle, reload, pivot and lobby locators; an odd value is the short
  blend (100 ms; even 300 ms).
- **A grip is then two caches overwritten** for the left hand: the two idle records and finger set
  0, after those three calls. First and third view run the same code.
- From the clips of all 103 guns: 97 hold the left hand on the idle locator whenever the gun is
  simply held. Six rest on `left_hand_cont` in some case (UMP45, Rem 870, TOZ-66, Magnum, SV-98,
  PPSh) and need a clip event before a grip can move the hand. Reload, show and hide leave the idle
  locator by design. The lobby asks for the lobby set. The twelve guns with `double_handed` 0
  release the left hand in sprint and jump.
- OPEN: the corrector writes the finger matrices whole, translation included; 0.69d's finger
  locators against 0.26g's hand are not yet compared.

### Shot effects

- 0.69d's attach effects (`fx/weapons/attach/*`, eleven in scope) have 0.26g's particle tree: every
  entity type and property they use exists here, and the loader has no version check. They are
  stored in a 16-byte record layout that 0.26g's loader (24-byte records, `parse_emitter` 0x363C10)
  misreads, so each needs a re-encode. An action type the client lacks (`type_index` 14, once,
  disabled in the donor) is skipped without an assert (`construct_action` 0x364F10).
- They name 17 materials and 15 textures 0.26g lacks. 0.69d0's effect materials use renamed
  parameters with no known mapping; 0.46c2 has the same effects and materials in 0.26g's own
  vocabulary, so the materials come from there, rebuilt on live templates of the same shader
  switches. Two light intensities differ between the builds (3.0 in 0.69d0, 0.18 and 0.16 in
  0.46c2; the stock AK effect peaks at 0.86). Not seen in a game.
- A weapon has one `particles/shoot` for every viewer; a device's effect is a set of that key.

### Stat keys a weapon's modification can reach

- Recoil (the six kick bounds) and `parameters/aim_time`: `weapon-rolls.md`.
- `parameters/stopping_power`: read by `weapon_damage_params` 0x332330 from the same config, used
  only by `base_player::hit_impl` (stopping_power x the round's `k_power`, which is 1 on every round).
- Pellet spread: per pellet 0.5 x the weapon's `dispersion/base_dispersion` x the round's
  `buck_dispersion` x a normal draw (`get_dispersed_buckshot_direction` 0x2E2120). A weapon's own
  `melee/data/buck_dispersion` is its melee attack's. So a weapon modification changes pellet
  spread only through `base_dispersion`, which also scales every shot's static dispersion.

## Stats for now

0.69d's bought grade (7) by one rule onto keys the weapon reads: a main effect on a key 0.26g has
keeps its value; aimed, hip and first-three-shots recoil become plain recoil at a share (0.75,
0.25, 0.25); a penalty on a key 0.26g lacks is dropped; a device whose main effect has no key takes
recoil -2 %. Chokes and duckbills can take `base_dispersion` instead. The values are a table of the
content pack. Later, each with its own reader: aimed and hip recoil, first-three-shots recoil (a
new field in the recoil snapshot), aimed movement speed, the reveal radius, pellet spread per axis.

## Stages

0. this research; 1. the guns on 0.69d0's models with the two bodies; 2. the crate, the porter's
`port-modules` and the client runtime; then three lanes: scopes and their migration, muzzle devices
and effects, grips and carriers.

## Stage 2, the core: the crate, the table, the client's runtime

Built without a content build or a game: the shared crate with its tests, the table's format, and the
client's writes for the primitives stage 0 proved. Not linked into the exe yet, not run in Unicorn or
a game ("What is not done" below).

### The crate `weapon-modules` (`srp-026g/crates/weapon-modules`)

`no_std`, no allocation, no `unsafe`, no read that can panic; the same code for the porter, srp's
servers and the game's arena.

- `SlotId`: 0.69d0's 94 slot ids, a closed enum generated from its `game.options`
  (`scripts/weapon-modules/slot_ids.py`). A slot's number is the table's byte: kept for ever, new ids
  appended.
- `Kind { Scope, Muzzle, Grip }`, `Role { Fitted(Kind), Carrier, Stock }`, `Point { Eye, FirePoint }`.
- `Table::parse(bytes)` reads and checks everything once; `Gun`, `Module` and `Attach` are views that
  check nothing again. A module's rules in one slot (`Attach`, 0.69d's `attach_params`): the slots it
  adds, the points it moves, the slots it alters, the slots it closes, its shot effect; a grip has
  sixteen hand locators.
- `resolve(gun, Wanted { scope, muzzle, grip }) -> Result<Loadout, Refusal>`.
  `Loadout::Whole` when nothing is fitted; else `Loadout::Bare`: the parts to draw on the bare body
  with their shape and placement, the optic's frame, the eye, the fire point, the left hand, the shot
  effect.
- `Placement` is a locator record's content (an offset on a bone of the gun), so what `resolve` gives
  is written into the game's records as it is. A part on a part composes statically:
  child = locator x half turn x parent's offset, on the parent's bone (`Placement::child`, the one
  place that says how a part's own locators stand on it).

**The rules of `resolve`** (0.69d's, with the design's decisions):

1. The gun's own parts stand in their slots. Each fitted module goes in the first slot it names that
   the gun with its own parts has; else in the first it names that the gun has carriers for, which
   come with it. Each kind is placed against the gun with nothing fitted: where a scope sits never
   depends on the grip.
2. A module or a carrier takes the place of the gun's own part in its slot (a device the brake's, a
   tactical forend the forend's). Two kinds that need one carrier share it (P90: the ACM forend
   brings both the grip's rail and a thread).
3. A close removes the gun's own part in that slot and nothing else. PROVEN in 0.69d0's data for the
   parts in scope: an optic closes every scope slot, its own and the one its rail is in, and still
   stands on that rail; the parts its closes do remove are guns' own sights and scopes in another
   scope slot (L85A2, SV-98). 0.69d's own code was not read.
4. An altered slot's part shows its alternative shape, when it has one (a sight folds).
5. The eye, the fire point and the shot effect are the last part's that sets them, a fitted module's
   over a carrier's over the gun's own. The gun's own parts set them too (the HK416's rear sight is
   its eye in 0.69d).
6. Refusals: the gun takes no such module of that kind; it has none of the module's slots and no
   carrier; the slot is another fitted module's or its carrier's (P90: a device of the barrel's own
   thread with a grip, whose forend sits on that thread); the part it stood on was displaced.

Seen in the data and not carried over: `slots_to_hide_while_aiming` is empty on every module;
overrides of `charging_handle` (5 modules) and the second aim points (`aim_2`..`aim_5`) are out, as
0.26g has one zoom and one eye a scope. A module whose rules differ on some gun (0.69d's
`exceptions`: a premium's own brake model, the bracket's attach on the RPD and PKP) is a row of its
own under the same dict id.

**Tests** (`cargo test -p weapon-modules`, 17): the guns, modules, slots, rules and dict ids are
0.69d0's own (rows of the design's fit table), the locators made up. A bare gun; the AK-74M's brake
replaced by a DTK-3; a grip on the FN FNC through its bracket; a grip on the AK-74M through the
tactical forend in the forend's place; an EOTech on the HK416 folding both sights and taking the eye
from the rear sight; a rail optic bringing the dovetail rail and not moving when a grip brings a
forend with a rail of its own; the Victory Day AK-74M taking the AK-74M's fits with its own brake's
model; the P90's shared carrier and its refused pair; a choke, a grip as a scope and a gun's own
brake refused. The eye against the engine's two formulas. The format: the checked-in table
(`tests/sample-v3.srpm`) is what the cases encode to and parses; every prefix and every single
changed byte of it is refused or parsed without a panic; nineteen broken rules each refused by name.

### The table

One file of game data, written by the porter (`weapon_modules::encode`, the crate's `encode`
feature, which parses what it wrote) and read by the game and the servers (`Table::parse`). Its
layout is `table.rs`'s header comment: `SRPM`, a version, fourteen sections of fixed-size records.
A gun: its dict id, its model's bone count and aim bone, its slots (a bone and an offset each), its
aimed idle pose for the bones they hang on, its own parts, its carriers (the slot they bring, each
with the slot it goes in), its fits, the gun whose fits it takes (a premium or camouflage item's
base gun), and its bare body's model. A module: dict id, role, model, alternative and aimed model,
its attaches, a grip's hand.

The format's versions (a table of another version is refused whole):

| Version | Change |
|---|---|
| 1 | the first layout (`feat/attachments-core`); never built into a pack |
| 2 | a gun record ends with a text, its bare body's model (34 bytes, was 28) |

Every locator of the table is written for one rule: a model drawn at a frame F has its locator L at
half turn x L x F, the engine's own rule for a gun's locators (`calculate_locator`). The porter
converts what its models hold to that rule and its renders check it against 0.69d (stage 0's open
point on 0.69d's placement convention lands there, and in `Placement::child` alone).

A weapon's config names what it is and has, four integers under `data`: `srp_gun` (the gun's dict
id) and `srp_scope`, `srp_muzzle`, `srp_grip` (a fitted module's dict id; absent or 0 for none). The
guns' rows and the fitted items' modifications that set them are content.

### What the core needs of the content

The content side (the porter's `port-modules` and the remodel, sub-branch `feat/attachments-content`)
and the client's runtime meet in these, and nowhere else:

1. **The table**: the archive's file `gameplay/weapon_modules.srpm` in the pack and in the server
   game data, `weapon_modules::encode`'s bytes, version 2. The game asks for it as
   `resources/gameplay/weapon_modules.srpm` (raw data): `resources/` is the request's prefix, as for
   the dictionary itself (asked for as `resources/gameplay/db_static_dictionaries`, 0xA0E8CC; the
   archive's `gameplay/db_static_dictionaries`).
2. **A gun record for every weapon item** that takes anything, premium and camouflage items too
   (their own record, `fits-of` their base gun). `bones` is the bone count of the item's model.
3. **Two bodies a gun.** The weapon config's `object/model` stays the whole body. The table's
   `bare` text is the bare body's path in the same form (a skeleton model's request path, what
   `object/model` holds). The bare body has the whole body's skeleton, bone for bone, and its
   locators (`barrel_point`, `scope_point`, `aim`, the hand and finger locators): the game reads
   them from whichever body it draws.
4. **A part's model** (`Module::model`, its alternative shape) is a static model's request path
   (what a scope item's `idle_model` holds), its origin the frame of the slot it goes in. A scope's
   idle and aimed models stay in its item's config (`rifle_scope`), one pair an optic, with the same
   origin; the native scope object draws them.
5. **Four integers under `data` of the weapon's config** as the weapon cook sees it (after the
   item's modifications): `srp_gun` on every gun of the table, and `srp_scope`, `srp_muzzle`,
   `srp_grip` set by the fitted item's modification to its dict id. `addons/rifle_scope_dict_id`
   stays what names the scope item to the game.
6. **The shot effect** is content: a device's modification sets `particles/shoot`. The client does
   not read the table's effect.
7. **First, one gun**: a table and a quick pack with the AK-74M alone (its bare body, its own
   brake, forend and sights as parts, one brake, one flash hider, one grip with its tactical
   forend, one scope with its rail), to run the core in a game before the rest is built.

**Agreed with the content side, 2026-11-20** (its conventions; nothing in the crate or the client
changes for them):

- The bare body's path is `<object/model>_bare` by the porter's rule
  (`weapons/remodel_069/<item>/<item>_bare`). The table still names it: the client has no rule of
  its own.
- A gun's slot is 0.69d0's locator as it is. A module's own slot, eye and fire-point locators are
  plain in its model's space and written as half turn x L, so `Placement::child` gives 0.69d0's
  place (seen on the Midwest forend). A grip's hand locators are in a gun rig's convention and
  written L x half turn, so the wrist is L x the grip's offset and the fingers are plain
  finger x parent^-1. LIKELY until the renders show it.
- A stock part's or a carrier's row carries 0.69d0's own dict id; only a fit's row carries ours.
  Nothing looks a stock part or a carrier up by its dict id (`Gun::fit` reads the gun's fits
  alone), so the two ranges may overlap.
- A rail mount's own `aim` is not written: an optic's eye and a stock part's are.
- **A stock part's row sets neither the eye nor the fire point.** Both bodies keep the live
  model's `aim` and `barrel_point`, so with nothing fitted, or only a grip, the iron sights and the
  flash are where they are today: the client writes the aim's and the barrel's records only when
  the loadout moves them, and leaves the model's own otherwise. The eye comes from a fitted optic
  alone, the fire point from a fitted device or a carrier (the P90's forend). The crate allows a
  stock part to set them (0.69d0's HK416 takes its eye from its rear sight, the crate's test); the
  content does not use it, and the client needs nothing else.
- **A gun's built-in suppressor needs no new rule, and is measured, not listed.** Where 0.69d0
  links a suppressor to the gun and it lies on the live mesh (median gap 1.5 mm at most), it is
  the gun's own part in its muzzle slot (a stock row whose attach adds the bracket's slot), and
  the bracket is the grip's carrier in that slot. `resolve` seats a carrier on a slot a stock part
  brings, and with no muzzle fit nothing takes the suppressor's place
  (`a_grip_hangs_on_a_built_in_suppressor_through_its_bracket`). Measured over the 102 guns: only
  the VSK-94 and its premium item qualify (a 0.69d0 default there already). The 9A-91's linked
  suppressor does not lie on the live mesh, and the OTs-14 and the SR-3M carry a brake in that
  slot and show no suppressor today, so eight clamp grips stay refused (9A-91 three, OTs-14
  three, SR-3M two), with that reason: a row would put a suppressor on a gun that has none.
- **A gun with a default optic.** The game draws that optic itself (the scope item
  `addons/rifle_scope_dict_id` names) at the body's `scope_point`, with nothing fitted and with
  only a device or a grip fitted alike, since no scope's frame is written then. So a default optic
  is never a stock part of the table (it would be drawn twice) and both bodies keep `scope_point`
  and `aim` for it. A stock sight's row of such a gun has the folded shape as its main model. A
  part in a slot an optic takes has no row and is in neither body: what stands there with the
  default optic on is that optic's own models'. How a fitted scope takes its place is the scopes
  lane's.
- **An optic's lens and reticle surfaces** take the players' `weapons/loc2` material of the same
  name where there is one, and are left out where there is none (the ACOG TA31's light pipe); no
  dot is baked yet.
- **Left out by the content build, each with its reason in the build's record:** the
  `short_picatinnyrail` that brings `picatinny_grip` (its locator is missing in 0.69d0's model);
  and LIKELY, not seen in a picture: the PKP's and the RPD's rail bracket is read as an overlay by
  its index.
- **The content side's state, 2026-11-20:** all 102 guns built (pack `weapon-modules@1`, 214 module
  rows, 6,933 comparisons with 0.69d0 through `Table::parse` and `resolve`, none off by 0.5 mm).
  That check proves the arithmetic, not the conventions: the game's pictures are the evidence for
  those, and nobody has judged a grip's hand yet.
- **To mind in the scopes lane:** the game applies the weapon's own `aim_offset` (its config's,
  the matrix at weapon +6A0h) to whatever the aim record holds
  (`calculate_aim_alignment` 0x244D5B). A fitted optic's eye is therefore seen through the gun's
  iron-sight eye relief unless the scope's modification sets `aim_offset` for it.
- **The first AK-74M run needs no device items.** A local edit that sets `data/srp_gun` and
  `data/srp_muzzle` or `data/srp_grip` on the gun's config is all the client reads for a device or
  a grip (with the fit's row in the table under that dict id). A scope needs one thing more: the
  scope item `addons/rifle_scope_dict_id` names must have its idle and aimed models with the
  optic's slot as their origin (a new pair of models, or a local edit that points an existing
  scope item at them), and `data/srp_scope` its dict id.

**The first real table through the client's code** (the content side's AK-74M quick pack,
24,803 bytes, version 2; Unicorn, the game's own cook beside patch 161, 2026-11-20):

- It parses in the arena. For a brake (2635), a flash hider (2631), a grip (2661), the PSO-1
  (2101), the Aimpoint (2110) and three together, the cook asks for `ak_74m_bare` in place of the
  model and for the right parts; every path it asks for is a model of the pack.
- The barrel's record moves only with a device (to z -0.6533 for the DTK-3, -0.7032 for the flash
  hider, on the `offset` bone); the scope's and the aim's only with an optic.
- **For the content side to look at:** the PSO-1's eye stands 11.6 mm to the side of its own
  frame (the aim record's x +0.0116 against the scope record's x -0.0003, both in the gun's
  terms), where the Aimpoint's is on its frame's axis (-0.0003 and -0.0003). It may be 0.69d0's own
  (the PSO-1 sits off the bore on its side rail); a mirrored x would look the same in these
  numbers. The first aimed picture in a game will show which.

What the client does with them: `resolve` at the weapon cook (once for the requests, once after the
model is loaded); the bare body asked for in place of `object/model`; each part's model asked for
in the cook's own query and drawn at its placement; the barrel, scope and aim records and the left
hand written.

### The client's runtime (patch 161, `rust/patches/src/attachments.rs`)

The scope's frame, the eye, the fire point and the left hand are data the game fills once from the
weapon's model and reads each frame; the patch writes them again after the game did. The parts are
models of their own: asked for by the weapon cook's own query, and put in the scene at the end of
`weapon::draw`, the one site that runs each frame.

| Site | When | What it does |
|---|---|---|
| `attachments.dictionary_requests` 0x2C8963 | the items dictionary cook's query of its five configs | the query asks for a sixth resource, the modules table |
| `attachments.dictionary_configs` 0x2C8AA0 | that query's callback, its entry | the table's bytes copied, parsed and kept |
| `attachments.scope_dict` 0x24AA15 | the weapon cook, before it looks for `addons/rifle_scope_dict_id` | a config that names a scope by `data/srp_scope` asks for that scope item's config (class 4Dh, the first request), whatever `addons` says |
| `attachments.scope_result` 0x24AD13 | the cook's callback, before it asks the config again whether there is a scope | the same config: the first result is the scope's |
| `attachments.request_room` 0x24AAB4 | the weapon cook sizes its requests | a place more for each part |
| `attachments.asked` 0x24AC30 | the weapon made, its requests pushed, none read yet | the weapon's model request names the bare body; each part's model is asked for after the game's own; the weapon is remembered with where its parts' results will be |
| `attachments.ready` 0x24ACF0 | the cook's callback, its entry | each part's model taken from the results (a reference of ours) and kept with its placement |
| `attachments.loaded` 0x24B0E1 | after `load_weapon` | the barrel's record +6E0h (the fire point), the scope's +744h (the optic's frame), the aim's +7A8h (the eye); the left hand's finger sets (idle and lobby) |
| `attachments.draw` 0x245017 | `weapon::draw`, after the weapon's model and the aimed scope | each part added or updated at the frame `calculate_locator` gives its record, drawn as the weapon's model is; taken out while that model is out |
| `attachments.culled` 0x245353, `attachments.hidden` 0x244017 | a culled player's weapon; `on_hide` | the parts out of the scene |
| `attachments.hands` 0x244649 | `weapon::activate`, after `set_weapon_locators` | the left hand's records, idle and lobby, first and third view (0, 4, 3, 7 of +1A20h) |
| `attachments.fingers_view` 0x298664 | the game's refill of the fingers at a change of view (the call wrapped) | the finger sets again |
| `attachments.shells` 0x2443EE | the shell effect's read of the scope's frame | nothing: the effect gets the frame the scope had |
| `attachments.release` 0x24B190 | the weapon is freed | the parts out of the scene, their models given back, the weapon forgotten |

- **One registry, a state a weapon.** A weapon with something fitted is known by its address from
  `attachments.asked` to `attachments.release`: asked (where its parts' results will be), then held
  (the models, each with the locator record its frame is worked out from). The parts are memory of
  the game's allocator. 256 weapons at once; a weapon past that, a cook without room or a part's
  model that did not load leaves the weapon as the game made it (or as its bare body alone) with a
  line in the log, once a kind.
- **`resolve` runs at each site that needs it** (the two of the cook, the callback's two, the
  hands, a change of view): the same table and the same four integers give the same answer, so no
  site keeps another's.
- **The bare body** is asked for in place of `object/model` (the one class 16 request of the cook).
  A weapon with nothing fitted keeps its whole body and is not in the registry.
- **The fire point is a record too**: `weapon::draw` evaluates the barrel's record into +620h each
  frame, where the shot's effects play, so the device's fire point is written once (patch 90 moved
  the flash at each draw).
- **Drawn as the weapon's model is.** Each draw copies the two bytes that put the gun in the
  first-person pass and make it movable (`weapon::set_foreground` 0x243D40,
  `set_movable_static` 0x243CB0) to each part before it is added or updated.
- **A written bone is bounded by the game's own model**: where the records are written, the
  model's bone count (its skeleton render model's inverse binds) must be the table's gun's or more;
  else the weapon is forgotten with a line.
- **The arena's size and its limit.** Patch 161 adds 49,152 bytes to the arena (1,517,148 in
  all), which was 96.5 % of `ARENA_LIMIT` (0x180000), with the three lanes still to come. The limit
  is our own number, a stop for a runaway block: `.srpx` is the image's last section, nothing
  follows it in the file's layout, the link and the self-test take their bound from the constant,
  and nothing outside the build knows it. It is 0x300000 from this branch on. What a larger arena
  really costs is its own bytes of the game's address space at the image's end (the engine's
  arenas and the video driver take theirs from what is free there): 49 KB for this patch.
- **The table's way in.** One file of the game data, `resources/gameplay/weapon_modules.srpm`, asked
  for as raw data (class 3: PROVEN, the game's own `startup.cfg` at `game::query_base_resources`
  0x1EC182 and the collision cook's three files at 0x828FB). It rides the items dictionary cook's
  query (`translate_query` 0x2C88D0: five binary configs, one `query_resources`): the query is
  handed six requests, and its callback `on_configs_loaded` 0x2C8AA0, which reads its five results by
  their place and never their count (PROVEN, 0x2C8AF5..0x2C8BEC), finds the table as the sixth. Its
  bytes are read as the game reads a raw file (a pinned pointer, 0x3DD990 / 0x3C1020), copied into
  memory of the game's allocator that is never freed, and parsed once.
  - Why not the weapon cook's own query, as first planned: the parts' models are requests of that
    same query, and which models a weapon needs is what `resolve` says from the table. The first
    weapon cooked would have had no table to ask them by.
  - Order: a weapon's config is named by an entry of the dictionary (the inventory cook looks each
    record's dict id up in it, 0x2F18E0; the lobby's preview takes the entry's config), so the
    dictionary's callback has run before any weapon cook. LIKELY until a game's log shows it.
  - A game data without the file, or with one that does not parse (a line in the log says which
    record), has no table: every weapon stays as the game made it, and the dictionary is made as
    ever.
- **The scope's dict id.** The game's scope object (zoom, the optic's two models, the gun hidden
  while aimed) is made of the item `addons/rifle_scope_dict_id` names: the cook looks that id up in
  the items dictionary and asks for the entry's config (0x24AA1C..0x24AA75), and its callback asks
  the weapon's config the same two questions again before it takes its first result as the scope
  (0x24AD1B..0x24AD41). One modification cannot write that key on every gun: a set (type 0) is
  skipped where the gun has no such node, an add (type 3) meets the duplicate-name assert where it
  has one (the guns with an optic of their own: SVD, VSS, VSK-94, SV-98, VSSK, Remington 700,
  HK416, MSBS...), and the cook has no value that means none. So both places take `data/srp_scope`
  first: when it names an item of the dictionary, that item is the scope and the game's own block is
  skipped (the cook's site sets the path and the count as 0x24AA67..0x24AA75 do; the callback's goes
  on at 0x24AD47 with the two slots the skipped instructions set). Else the game's own, untouched:
  `addons/rifle_scope_dict_id` is the gun's default optic and nothing else. A `srp_scope` the
  dictionary lacks is no scope (the game's own lookup would assert). `data/srp_scope` is read
  through `data/srp_gun` as the parts are: a config without `srp_gun` has no fitted scope.
- **A gun that lies alone** (srp's dropped gun, a dead body's): `vostok::pickup::place_lying` places
  the body in its bind pose; each part is placed with it at its record's frame through that pose
  (`attachments::lie`), and taken out with it. Written and compiled; not seen in a game yet.
- **0x24B0E1 has one owner.** The parked muzzle patch (90) used it; the new runtime takes it alone.
  `load_weapon`'s own finger fill (0x243BB6) is inside it, so this site covers that fill too.
- **Every weapon passes it.** PROVEN by a scan of every direct call of the image: `weapon::weapon`
  0x243640 is called only by `weapon_cook::allocate_weapon` (0x24A7C1), that only by
  `on_weapon_config_loaded` (0x24AC2B), `load_weapon` only at 0x24B0DC. srp's own copies (a pickup's
  gun, the ragdoll's) are cooked through an inventory, which makes its weapon by the same cook
  (`weapon_pickup.md`, "The instance"). LIKELY, not read: that the copy's item keeps its upgrades, so
  its config names the same modules.
- **Shells.** The shell effect plays at the scope's frame +660h. When the optic's frame and the
  gun's own `scope_point` hang on one bone, the old frame is one fixed matrix times the new one
  (half turn x old offset x inverse(new offset) x half turn), worked out once; the effect is handed
  that. On different bones the shells go with the scope.
- **The lobby.** A grip has one set of hand locators (idle). The lobby's clips ask for the lobby
  set, which gets the same values.
- **The eye is a switch**, and the fingers were one: "The two switches" below.

### What is done, and what is not

Done, 2026-11-20:

- **The arena is linked with patch 161** (`arena_026g.lock.json`).
- **`check_attachments_026g.py` passes** (`tools/check attachments`; 39 cases in the build and
  the original): the table's way in; the cook's requests; the callback's records and fingers; the
  parts in the scene; the left hand; the shells; the release; a weapon without any of it. What it
  compares with is the weapon-modules crate's own `resolve`, pinned in
  `tests/sample-v3.loadouts.tsv` by the crate's test. Its objects are made up and its table is the
  crate's sample: it proves the patch's own code beside the game's, not the content.

- **One gun runs in a game** (2026-11-21; a headless client on a local server, the content side's
  AK-74M quick pack, and a local edit that sets `data/srp_gun` 3 and the fit on the gun's config;
  beside it main's own exe on the same content as the look with nothing fitted):
  - **A brake**, the Zenit DTK-3 (2635): in the lobby the device with its three ports and crown
    where main's exe shows the stock brake; in the first view the gun idle and aimed over its iron
    sights as ever (the device is behind the front sight there).
  - **A flash hider and a grip on a swapped forend**, 2631 and the PTK grip 2661: in the lobby the
    long plain flash hider, the railed tactical forend in place of the stock one and the left hand
    on the grip under it; in the first view the railed forend and the left hand lower, under the
    gun.
  - **A scope**, the PSO-1 (2101, the shop scope item pointed at the optic's own two models), with
    the DTK-3 and the grip: on the gun in the lobby and in the first view; aimed, the scope's
    picture with its chevron's tip on the screen's centre (within a pixel at 1280x720, at rest).
    The eye's 11.6 mm to the side of the optic's frame is 0.69d0's own, not a mirrored axis.
  - Every game's log has the table's line and none of a part not asked for, not loaded or a model
    with fewer bones. The table was there for every weapon's cook of these runs.
  - One of the five sessions left its match in the countdown: the client stood still for twelve
    seconds and timed out while the server's other player's ping reached 8.5 s in the same seconds
    (the machine was running four heavy jobs). The same build and content stayed in its match the
    next time. Read as the machine's, not proven.

  - **A shot** (the same three parts; `private/renders/attachments-core/ak74m-complete-shot/`):
    from the hip the shot's smoke rises at the muzzle's end, ahead of the front sight; aimed, the
    scope's picture lifts with the shot and comes back to the centre within a few frames (the kick
    is seen under a scope, `EyeAnchor::GunIdle`). No frame of the 60 a second caught the flash
    itself, so its place against the device's fire point is not judged; the shells were not looked
    for. The death view of that run looks along the ground and does not show the body's gun.

Done since (2026-11-21):

- **The servers' one fit check** and slots 4 and 5 (`survarium::v026g::weapon_modules`; an attach
  of a muzzle device or a grip; the shop's rule): "The contract" below.
- **The scope's dict id from `data/srp_scope`** (two sites), **parts on a lying gun**, **the
  fingers' one path**: written, compiled, the crates' tests pass; the Unicorn cases of the scope
  sites are written. Linked and checked: see the PR's state.

Not done:

1. **Seen by one viewer only.** The third view is the lobby's character (the same `weapon::draw`);
   no second client has looked at a fitted gun in a match. The flash's place at a device's fire
   point and the shells' place under a moved scope are unseen. A grip's fingers were seen from
   across the lobby, not judged (the grips lane takes that picture).
2. The order "the table before any weapon" held in every run; it stays LIKELY as a rule.
3. **A dropped gun and a dead body's gun** draw their parts by code no game has shown yet.
4. **The lookup by address** scans the registry at each weapon's draw; to be measured in a match
   before it is called done.
5. The six guns whose left hand does not rest on the idle locator need their clip event before a
   grip moves the hand (stage 0).
6. **The Attachments window's muzzle and grip slots: written, compiled, not seen** (2026-11-21).
   Two more slots of the scope slot's own symbol (`SrvModifierRendererSk`, made by the window),
   stacked under the scope on the Attachments page in armour's three decal places, only the kinds
   the weapon takes. What goes on a weapon is the items dictionary's `items_compatibility` pairs of
   all three kinds (`UpgradeWindow.attachmentPairs`); a pair's kind is the shop's scope range or
   `survarium.data.ModuleKinds`, the table `ids.py --movie` writes from `ids.tsv` (check
   `module-kinds`). A muzzle device and a grip that do not go together are refused by the server
   with their own text (`st_srp_attach_unfit`), and the slot shows what the weapon holds again at
   the account's update. An account's load checks each record's slot, not yet the pair the weapon
   holds.
7. **`plane_slot`: table version 3, started.** In 0.69d0's `game.options` every collimator, every
   folding or rear iron sight and the dovetail, pistol and G3 rail mounts override the eye as
   `slots_to_override { id: aim, locator: aim, plane_slot: body_aim }` (34 keys on `aim`, 5 on
   piggyback sights that are out of scope); no magnified scope does. The scopes lane read the rule
   in 0.69d0's exe (PROVEN, static and 400 Unicorn cases; `attachments-scopes.md`,
   `get_slot_transform` 0x2006CF320): the slot's frame M keeps its axes, and its origin moves along
   its own k (the line of sight) to the plane through the plane slot's origin normal to its k:
   `s = (P.c - M.c)·P.k / M.k·P.k`, `M.c += s·M.k` when `s >= -2.0` m; a line along the plane or a
   plane over 2 m behind leaves the locator whole. `aim_offset` is added after, along the same k.
   So a collimator gives the line of sight and the gun keeps its own eye relief. Today's table
   (version 2) writes such a module's `aim` whole: the eye is nearer the optic than in 0.69d0, the
   reticle's centre the same.
   - Decided (the coordinator): one value on an attach's eye that `resolve` applies with the gun's
     own eye, not per-gun rows.
   - On the branch `feat/attachments-table-v3` (one commit over c81ada3df, NOT merged): version 3
     of the table (a gun record has its own eye, `body_aim`, 84 bytes; a point of kind 2 is an eye
     at the gun's plane), `AttachEye::Own | InGunPlane`, `Gun::body_aim`, `resolve`'s
     `in_gun_plane` in the aimed idle pose, the sample at version 3; the crate's 20 tests pass.
   - Left for it: compile `survarium` and the patch crate against it; `check_attachments_026g.py`
     (it reads `sample-v3.*` on that branch); a test of the two cases that leave the eye whole;
     the porter writing `body_aim` and the 34 `InGunPlane` eyes; then link, check, a server and an
     exe published TOGETHER with the first version 3 pack (a version 2 table is refused by a
     version 3 game and server).
8. **The left hand on a turned grip** (the grips lane's finding, the coordinator's decision,
   2026-11-21; written in `Hand::new` the same day, the crate's
   `a_turned_grip_keeps_the_hand_in_the_guns_axes`, the pinned loadouts and the Unicorn check
   with it; exe `e17ace688`; not seen in a game): laid over each grip's mesh in all 24 orientations, the table's
   convention for the hand locators is the best on twelve of the thirteen grips (finger joints 12
   to 24 mm from the surface), which settles it by data. The thirteenth, the PKP's own grip (2668),
   is 139 mm off: its is the only grip fit of 318 whose frame is turned on the gun (a quarter turn:
   the PKP's `grip_picatinnyrail_point` is on the gun's side), and the hand lies on that grip
   (19.9 mm) when placed at the grip's POSITION in the GUN'S axes. Decided: `Hand::new` takes the
   grip's position and not its turn (wrist = locator x half turn x translation(the grip's
   offset's translation), on the grip's bone); the fingers are untouched; it moves one fit and the
   other 317 by under a degree. From 0.69d0's data alone, not its code: the PKP in a game is the
   proof. Pictures: `private/renders/attachments-grips/computed/`. It changes the crate's pinned
   loadouts (`sample-*.loadouts.tsv`) and so the Unicorn check's expectations.

## The two switches

**The eye, `attachments::EYE`: `EyeAnchor::GunIdle` (set), settled 2026-11-20.**

- `EyeAnchor::Optic`: the aim record is the optic's own eye on the optic's bone. The reticle is at
  the screen's centre at every moment of every clip; what a clip does to that bone while aiming,
  the shot's kick, is never seen.
- `EyeAnchor::GunIdle`: the record hangs on the bone of the gun's own `aim`, at the offset that
  puts the optic's eye there in the gun's aimed idle pose (the table's pose). The optic is centred
  while the gun is at rest, and a clip that moves the optic's bone against the aim's shows.
- Patch 165 (#2294, `aim-pose.md`) makes the kick seen in sights and scopes by its own rule: on an
  aimed frame at rest the aim record leaves its bone for the gun's model at that pose. It reads
  whatever record is there, so it runs after either setting.
- The evidence (`cargo test -p weapon-modules`,
  `the_idle_pose_anchor_shows_the_kick_and_agrees_with_patch_165`: both settings through the
  engine's two formulas, alone and under the patch's rule, on a gun aimed from the root and on one
  aimed from the kicked bone):

| | A gun whose own `aim` is on the root (every stock gun) | A gun whose `aim` is on the kicked bone (the ten ported ones) |
|---|---|---|
| `Optic` alone | centred, no kick | centred, no kick |
| `GunIdle` alone | centred at rest, the kick seen | centred, no kick (as over its iron sights today) |
| either, under patch 165 | centred at rest, the kick seen: one eye for both settings, and the picture `GunIdle` gives alone | centred at rest, the kick seen: one eye for both settings |

- So `GunIdle` shows the kick with or without #2294, and the two agree: no order of merging hides
  it. What it costs: at rest the optic is centred as far as the table's pose is the game's idle
  pose (the optic's bone against the aim's bone); the porter reads that pose from the gun's idle
  clip and the scopes lane checks the centre with the camera oracle for every fit.
- The two patches write one record, the weapon's aim locator (+7A8h): this one once, when the
  weapon is cooked; patch 165 on aimed frames at rest, from what it finds there. No Unicorn case
  runs them together yet (#2294 is not in this branch).

**The fingers: the grip's own matrices, whole. Settled 2026-11-21; the switch is gone.**

- The game copies a finger's matrix whole, and its translation is the joint's place in the hand's
  skeleton. The question was whose joints a hand on a grip keeps: the gun's own (0.26g's hand) with
  the grip's turns only, or the grip's.
- The grips lane measured it: the finger joints of all thirteen grips' locators are one set, and
  that set is 0.26g's own hand skeleton (the largest gap to its bind pose is 3.4 micrometres); the
  guns' own finger locators are the same joints rounded to a millimetre. So there is nothing to
  choose: `Hand::fingers()` gives the grip's matrices and `FingerJoints` is removed. A game picture
  of a hand on a grip, up close, is the grips lane's.

## The contract between the core and the lanes

Three lanes work on sub-branches of `feat/attachments` (`feat/attachments-scopes`,
`feat/attachments-muzzles`, `feat/attachments-grips`); the lead merges them. This section is what
they share. A lane that needs something here changed asks the lead; it does not edit a core-only
file.

**Ids and slots**

| Thing | Value |
|---|---|
| Kit slot of a scope, a muzzle device, a grip | 3, 4, 5 (`weapon-kit`; `weapon_modules::slot_of`) |
| Scope items | dict ids 2101-2120, the shop's own (ownership does not change) |
| Muzzle devices and grips | dict ids 2601-2671 (`scripts/weapon-modules/ids.tsv`, append-only) |
| Not items: 2652-2655 | the VSSK's four barrels, its suppressor in other shapes: out by the owner's rule (no silencers). Their ids stay taken (never reused) and no item is made for them, so nothing can hold one; their fits leave the table with the porter's suppressor rule at the next pack version |
| Not items: 2667, 2669, 2670 | the OTs-14's, the SR-3M premium's and the SR-3M's own foregrips: the guns' own parts, always on (decided 2026-11-21); reserved, unused |
| A fitted item's modification | id 994000 + the item's dict id, all three kinds |
| The items' icons | `LobbyMenu_AmmoIcons`: columns 5, 6, 7 (rows 1-22) the muzzle lane's, column 8 the grips lane's |
| A stock part's or a carrier's row in the table | 0.69d0's own dict id; never an item of ours |
| A slot's id | `weapon_modules::SlotId`, 0.69d0's 94 names; its number is the table's byte |

**What makes a weapon fitted** (all of it content; the client reads the weapon's config as its cook
sees it, after the item's modifications)

- A gun's config: `data/srp_gun` = its own dict id (the edit list `weapon-modules-1.json`), and
  `object/model` its whole remodel body (`remodel-069-1.json`).
- A fitted item's upgrade record holds the item's **own** dict id in its kind's slot (no variant
  ids any more), and its modification puts that id on the weapon the same way for all three kinds:
  one **type 3 add, path `[data]`, value `[srp_<kind>, <dict id>]`** (`srp_scope`, `srp_muzzle`,
  `srp_grip`). Its stats are type 2 rows. So:
  - a scope: `data/srp_scope` and nothing else. The weapon cook makes the game's scope object
    (zoom, the two models, hiding the gun) of that item (patch 161's two scope sites);
    `addons/rifle_scope_dict_id` is the gun's own default optic only and no modification writes it;
  - a muzzle device: `data/srp_muzzle`; its shot effect `particles/shoot`; its stats;
  - a grip: `data/srp_grip`; its stats.
- A scope item's config keeps `data/idle_model` and `data/aimed_model`: the optic's own two models
  of the pack, their origin the optic's slot.
- Nothing else is read: the table says the rest.
- **A gun's config has room for it.** The client rebuilds an upgraded item's config in a stack
  arena its allocator never bounds (`create_binary_config_buffer` 0x358AA0: 24 bytes an entry, 12
  more a table or list, one entry more for each type 3 add). It was 16 KB, which the largest gun
  configs fill with a camouflage and decals (the server's check: 565 entries on the plain config,
  `tables::MAX_CONFIG_ENTRIES`; the Remington 700 premium and the KS-23M are at 565 with `srp_gun`,
  the SV-98 564, the Remington 700 563, the Neosteads 562 and 560, the Mosin 560). Patch 161 makes
  it 64 KB (`ITEM_CONFIG_ARENAS`), so the check stays as it is and a kit's adds have 48 KB on top.
  A game without that patch must not be given a fitted gun of those six (one supported client
  version: the release ships both).

**The servers' one fit check** (`survarium::v026g::weapon_modules`, core-only)

```text
Modules::shared()                         the game data's table, read once (none: nothing fits)
  .check(weapon, Fitted { scope, muzzle, grip }) -> Result<(), Unfit>
  .kind(weapon, module) -> Option<Kind>   the kind a module is fitted as on that weapon
  .fits(weapon) -> Vec<(module, Kind)>    what the weapon takes, for a list
  .is_module(item) -> bool                some gun takes it: the shop sells it (prices.rs)
weapon_modules::fixtures::modules(&[(weapon, module, Kind)])   a made-up table, for tests
slot_of(Kind) -> u8, kind_of_slot(u8) -> Option<Kind>
Unfit::NoGun | Unfit::Refused(Refusal)    Refusal is resolve's own: NoFit, NoSlot, Taken, Unseated
```

- `check` runs the same `resolve` the game runs, over the whole of what the weapon would hold, so a
  pair that does not go together (the P90's own-thread device with a grip) is refused whichever is
  put on second.
- Every fit check calls it: an attach, an account's load, a shop's list. No lane keeps a table or a
  rule of its own.
- A muzzle device or a grip goes in slot 4 or 5 by `kind` (`modifications::upgrade_slots_with`),
  and an attach checks the whole weapon (`items_rules::attach_upgrade_with`).
- The shop: a category-22 item with a price is sold by seller 0, at no rank, when it is a scope
  item of a pair or `is_module` (`prices::sellers_of_with`). A device or grip item needs its
  dictionary row with a `base_cost` and a fit in the table; nothing else.

**What a lane runs its games on** (built once by the core; a lane builds no server and no exe)

- `private/local-clients-attachments/server-bin/<commit>/`: `srp`, `srp-bot`, `certs/` and a
  README with the commit, of `feat/attachments`' head; again when the attach path or the cook's
  sites change.
- `private/local-clients-attachments/exe/<commit>/`: the game's exe of the branch, the same way.
- A lane's test packs are `weapon-modules-quick@N`; the full `weapon-modules@N` is the core's.

**What is settled between the lanes and the core**

- The `next` row of `simulation_changes.tsv` is one: each lane hands its words to the lead, who
  writes the row at the merge (grips: "a fitted grip changes the weapon's recoil kick bounds and
  aim time").
- For the lobby's text: 66 muzzle x grip pairs are refused by `resolve` (the P90's two own-thread
  devices with its six grips; on the VSK-94 and its premium nine devices with three grips).
- A machine fact for every lane (the scopes lane, 2026-11-21): the system's `magick`
  (`/run/current-system/sw/bin/magick`) is a bubblewrap wrapper that binds only its working folder,
  so the porter run from its checkout cannot open a texture it just wrote in a work folder
  elsewhere, on the SSD or the HDD alike. Run the porter with its dev shell's ImageMagick on PATH
  (`nix develop -c ...` in the porter's checkout); then the work folder may be on the HDD.
- The eight clamp fits stay refused: in 0.69d0 the clamp sits on a slot only a fitted suppressor
  brings. The 9A-91 takes no grip.
- The SR-3M's, its premium's and the OTs-14's own foregrips stay the guns' own parts, always on,
  as today (decided 2026-11-21): no item, no stat. The remodel bakes them into both the whole and
  the bare body, and both keep the live hand locators (the next pack version).
- The muzzle lane's item table stands as proposed, the two Zenit DTK-4 in as brakes, the duckbills'
  spread -15 %; the grips lane's as proposed, 6,000 silver each.
- 46 released (weapon, scope) pairs are not fits of 0.69d0's (19 Docter pairs, the 18 of
  `sr3m_premium`, 3 on `wpn_mp7_silencer_special`, 5 singles; the Mosin's PU 3.5x was only a
  missing id row and is a plain fit now): the scopes lane carries them as legacy per-gun rows in
  the porter (`modules.rs`, its own functions).
- `sr3m_premium` has no muzzle fit in the table of pack 1 where `sr3m` has 13 (its `fits_like`):
  the cause is the porter's: a premium takes its base gun's fits only when its own links give
  none, and 0.69d0 links this one to its own camouflage foregrip. Fixed with the next pack version.
- **The next pack version** (`weapon-modules@2`; the pack rebuilt over patch 68 with the same
  porter is byte for byte pack 1, so the row stays `@1` until then) takes: `sr3m_premium`'s fits;
  the VSSK barrels out of the fits; the PU 3.5x's row; the three own foregrips baked into both
  bodies; the remodel bodies' camouflage masks (below); the `items_compatibility` rows of every
  device and grip item with its guns (what the lobby's window lists by, as for scopes); the scopes
  lane's legacy pairs. Being built on the branch `feat/attachments-pack2` and the porter's
  `feat/attachments-069d`. Table version 3 (`plane_slot`, "Not done" 7) comes with the version
  after it, or with this one if it is ready first.
- **The remodel bodies' camouflage masks** are made inside the attachments pack, by the mask maker
  main has today over `scripts/remodel-069/camo.tsv`: every gun gets a remodel body, so without them
  every owned camouflage would change with this PR. The one-rule masks of #2314 (parked) remake them
  when that lands, as every other gun's. Open until the next pack version is built.

**Who owns which file**

| Files | Owner |
|---|---|
| `crates/weapon-modules/**` (the format, `resolve`) | core |
| `client-patches/rust/patches/src/attachments.rs`, `attachments.toml`, `vostok/attachments.rs`; the `attachments.*` rows of `sites_026g.tsv`, `fields_026g.tsv`, `engine_026g.tsv`; the generated files; `arena_026g.lock.json`; `check_attachments_026g.py` | core |
| `crates/survarium/src/v026g/weapon_modules.rs` | core |
| `crates/survarium/src/v026g/modifications.rs`, `crates/session/src/v026g/items_rules.rs` (where an upgrade's slot and an attach are decided) | core for slots 4 and 5; then the scopes lane alone, for the scope's branch |
| `flash/lobby_menu/**` (the Attachments window) | core |
| `scripts/weapon-modules/ids.tsv`, `make_pack.sh`, `edits.py`, `review.py`; `scripts/remodel-069/**`; the pack's rows of `release/content.tsv` | core (the content pipeline) |
| this note | core; a lane keeps its own `docs/notes/attachments-<lane>.md` |
| `crates/survarium/src/v026g/scopes.rs`, `scripts/scope-fits/**`, the migration of variant ids, the scope items' edit lists | scopes |
| `crates/survarium/src/v026g/prices.rs` (who sells a module item: `sellers_of_with`) | core |
| the device items (their configs, stats, texts, icons, prices), the shot effects, its words for the `next` row | muzzles |
| the grip items (the same), the clip events of the six guns, the hand's checks | grips |
| `client-patches/localization/*.tsv`, `release/content.tsv` (a lane's own new rows) | each lane adds its rows; the lead settles them at the merge |

## The three lanes (briefs, kept current)

Not made by the content side's first pass, and so each lane's own: the device and grip items
(dictionary rows, configs whose modification sets `srp_muzzle` / `srp_grip`, stats, texts, icons,
prices); the shot effects' particles; the scope items on the new origin; the camouflage masks of
the remodel bodies (the core's, with the next pack version: "What is settled" above); `release/build-content --full` with the pack; clippy on the porter.

They start when the core and the content run on one gun (the AK-74M: a brake, a flash hider, a grip
on a swapped forend, a scope; both views and the lobby), each a sub-branch of `feat/attachments`.
For all three: the porter converts every locator to the table's rule (`Placement::child`, the one
place to change if 0.69d's placement differs); no value is typed per gun; a left-out row has a
reason the build checks again.

**Scopes and their migration**

- Optic rows with one eye per attach; a piggyback attach (the Docter on the Elcan) is dropped with
  its reason.
- The rail carriers per gun.
- A gun whose default scope or sight stands in a scope slot (SV-98, L85A2) has it as a stock part
  an optic displaces or closes: decide how today's built-in scopes map onto that.
- The legacy pairs (pairs we released that 0.69d does not have) are per-gun module rows under the
  scope's dict id.
- `parameters/aim_offset` stays the eye-relief knob.
- Servers: `resolve` in place of the `srp_scope_fit` pairs; an account's load rewrites a variant
  dict id to the scope's own.
- `attachments::EYE` with #2294 (patch 165): see "The two switches".
- Centring checked with the camera oracle on the written records.

**Muzzle devices and effects**

- A row is one thread slot, a fire point and an effect.
- Carriers: the P90's ACM forend, the Neostead's and the Glock's barrels.
- Refused by rule: a device of the P90's own thread with a grip (the grip's forend takes that
  thread). The lobby says so.
- The effects re-encoded with 0.46c2's materials (`widen_config` of PR #420).
- The stats: one `next` row in `simulation_changes.tsv`.

**Grips and carriers**

- 13 tactical forends, 3 brackets, 1 clamp.
- The hand locators in the table's rule, checked against 0.69d's renders.
- 0.69d's finger joints against 0.26g's hand: measured, they are one skeleton ("The two
  switches"); a picture of a hand on a grip in a game is left.
- The idle-target clip event for the UMP45.
- Whether the lobby's poses use a grip's idle set (they do now).
- The twelve guns with `double_handed` 0 release the hand in sprint and jump: unchanged.
- A clamp on a suppressor only where the suppressor is the gun's stock part and adds the clamp's
  slot (no gun of ours: the eight clamp fits are refused).
