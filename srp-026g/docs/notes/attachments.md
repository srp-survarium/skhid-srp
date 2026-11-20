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
(`tests/sample-v2.srpm`) is what the cases encode to and parses; every prefix and every single
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

1. **The table** at `resources/gameplay/weapon_modules.srpm` in the pack (a file of the game data
   as it is, asked for as raw data) and in the server game data: `weapon_modules::encode`'s bytes,
   version 2.
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

What the client does with them: `resolve` at the weapon cook (once for the requests, once after the
model is loaded); the bare body asked for in place of `object/model`; each part's model asked for
in the cook's own query and drawn at its placement; the barrel, scope and aim records and the left
hand written.

### The client's runtime (patch 161, `rust/patches/src/attachments.rs`)

Everything it changes is data the game fills once from the weapon's model and reads each frame; the
patch writes it again after the game did. No per-frame hook.

| Site | When | What is written |
|---|---|---|
| `attachments.dictionary_requests` 0x2C8963 | the items dictionary cook's query of its five configs | the query asks for a sixth resource, the modules table |
| `attachments.dictionary_configs` 0x2C8AA0 | that query's callback, its entry | the table's bytes copied, parsed and kept |
| `attachments.ready` 0x24ACF0 | the weapon cook's callback, its entry | nothing: the config's four integers are read and kept for the next site |
| `attachments.loaded` 0x24B0E1 | after `load_weapon` | the scope's record +744h (the optic's frame); the aim record +7A8h (the eye); the left hand's finger sets (idle and lobby); the weapon is remembered |
| `attachments.hands` 0x244649 | `weapon::activate`, after `set_weapon_locators` | the left hand's records, idle and lobby, first and third view (0, 4, 3, 7 of +1A20h) |
| `attachments.fingers_view` 0x298664 | the game's refill of the fingers at a change of view (the call wrapped) | the finger sets again |
| `attachments.shells` 0x2443EE | the shell effect's read of the scope's frame | nothing: the effect gets the frame the scope had |
| `attachments.release` 0x24B190 | the weapon is freed | the weapon is forgotten |

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
- **The eye is one switch**, `attachments::EYE`:
  - `EyeAnchor::Optic` (set): the aim record is the optic's own eye on the optic's bone. The reticle
    is at the screen's centre at every moment of every clip; what a clip does to that bone while
    aiming, the shot's kick, is not seen.
  - `EyeAnchor::GunIdle`: the record hangs on the bone of the gun's own `aim`, at the offset that
    puts the optic's eye there in the gun's aimed idle pose (the table's pose). The optic is centred
    while the gun is in that pose and a clip that moves the optic's bone against it shows, as over
    iron sights. Both run against the engine's formulas in the crate's tests.
  Which clips move the `offset` bone while aiming was not surveyed; the "scope shot animations from
  0.45" work decides the value.
- **The fingers are one switch**, `attachments::FINGER_JOINTS`. The game copies a finger's matrix
  whole, and its translation is the joint's place in the hand's skeleton. `FingerJoints::Gun` (set)
  keeps the translations the game made of the gun's own finger locators (0.26g's hand) and takes
  each finger's turn from the grip; `FingerJoints::Grip` takes 0.69d's whole. 0.69d's finger
  locators were not compared with 0.26g's hand; `Gun` cannot stretch a finger either way.

### What is not done

1. **The table's way into the game** is written, not linked or run: "The table's way in" below.
2. **Drawing the parts** (P1, with the fire point P2a and the shapes P3): patch 90's request, scene
   and release code for N parts. `Loadout::Bare::parts` is what it draws.
3. **The arena's link.** `arena_026g.lock.json` is not written again (`python link_026g.py
   --write`), so nothing that builds the exe passes on this branch yet.
4. **Unicorn**, `check_attachments_026g.py`, to write with item 1 (a table has to be in the game's
   memory for any case): the cook's callback from its entry over a made-up config, weapon and model
   with the scope and aim records compared with `resolve`'s; `weapon::draw` to +660h and
   `calculate_aim_alignment` on the written records, for the centred optic in both eye settings;
   `weapon::activate` and `correct_fingers` at a change of view with the hand records and finger
   sets compared; `play_weapon_shell_pfx` with the frame the effect gets; a weapon without `srp_gun`
   and one whose fit is refused, byte for byte the original's; the release.
5. **A written bone against the game's own model.** A placement's bone is below the table's bone
   count for the gun; the game indexes its bones unchecked. The model's own count is to be read
   where the records are written, and a table that names more refused.
6. The six guns whose left hand does not rest on the idle locator need their clip event before a
   grip moves the hand (stage 0).

## The three lanes (briefs, kept current)

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
- 0.69d's finger joints against 0.26g's hand, to settle `attachments::FINGER_JOINTS`.
- The idle-target clip event for the UMP45.
- Whether the lobby's poses use a grip's idle set (they do now).
- The twelve guns with `double_handed` 0 release the hand in sprint and jump: unchanged.
- A clamp on a suppressor only where the suppressor is the gun's stock part and adds the clamp's
  slot.
