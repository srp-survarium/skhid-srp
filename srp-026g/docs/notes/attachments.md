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
