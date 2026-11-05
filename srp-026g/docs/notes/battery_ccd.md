# Batteries under the ground (patch 106)

- **Reports:** the owner (2026-11-05): "batteries sometimes break when you die with
  them in your hands". Discord #general-ru, 2026-11-04 21:07, arto4221 (after a
  grenade report): "Иди ещё ящики под текстуру проваливаются" (the boxes, the
  batteries, fall under the textures too). No map, mode or video; nothing in the
  #bugreports forum.
- **Stock 0.26g, the same fault as the grenades' (grenade_ccd.md):** the battery
  (`victory_item_core`, its constructor 0x3000A0) is a box of half extents 0.9 x its
  config's collision mesh scale (`gameplay/victory_items/default.options`
  `collision_geometries[0]/meshes[0]/scale` 0.3 0.12 0.16, so 0.27 x 0.108 x 0.144 m;
  create_primitive_shape type 1 at 0x30037C), mass 10, friction 1, restitution 0,
  sleeping thresholds 2 and 4 (0x357100's info). One 0.01 (0xA2D774, loaded at
  0x300468) is both its motion threshold (+104h, 0x30047D) and its swept sphere
  (+100h, 0x300485). PROVEN (code).
- **The drop:** at the carrier's death `victory_item_core::drop` (0x300E50, vtable
  +B4h) places it where the hands held it (+204h with the config's offset +284h,
  0 0.12 0) and, now or after the carrier's step (player +33Ch; 0x2D9570), 0x300DC0
  puts it into the world (0x301DB0, `activate_physics` 0x301CB0: group A18h, mask
  418h, the level's collision bullets hit; zero velocity) and gives it the impulse of
  its +290h/+29Ch, which `deactivate` (0x300F40) zeroes: it falls from rest. The same
  body comes back at every drop. No reset of a battery that fell through was found
  in victory_item_core: it is gone for the match (LIKELY the "breaks": no one can
  reach it).
- **Why it sinks:** a fall of 1 m reaches 4.5 m/s, 7.5 cm a 1/60 s step; above the
  0.01 threshold every hit is swept, and vostok's integrateTransforms moves the box to
  where the 1 cm sphere at its centre touches: the box 9.8 cm or more into the
  surface, its speed capped at 0.6 m/s still into it; the one-sided mesh then often
  pushes it out under.
- **Simulated** (`client-patches/battery_ccd_sim.cpp`, the grenade simulation's world
  and step, the battery's body; 1000 drops a case from rest at the carrier's hands
  (0.9..1.3 m, crouched 0.5..0.8 m), random yaw and +-34 degree tilt):

  | case | stock (0.01) | patch 106 (0.08) |
  |---|---|---|
  | flat floor, 4 m triangles | 2.0 % under | 0 |
  | rough ground (+-0.3 m), 1 m triangles | 0.2 % | 0 |
  | a crouched carrier, rough ground | 0 | 0 |
  | a 25 degree slope | 3.1 % | 0 |
  | from a ledge, 3 m | 15.2 % | 0 |

  Two more seeds: stock 1.9-2.1 %, 0.2-0.3 %, 2.4-2.5 %, 12.8-13.1 %; 0.08 none of
  15000 in all. Smaller spheres: 0.05 loses 0.2 to 0.8 %, 0.06 0.1 to 0.3 % (two of
  three seeds), 0.07 none in one seed. A bigger threshold is worse: 0.08 / 0.08
  loses 2-3 % on floors and 12 % off a ledge (fewer hits are swept and the box
  tunnels by itself), so the threshold stays 0.01.
- **The fix:** the threshold keeps its 0.01; the swept sphere is 0.08, 2.8 cm inside
  the box's thinnest half (0.108). The two stores at 0x30047D (16 bytes) become
  `add eax,100h / movss [eax+4],xmm0 / mov dword [eax],0x3DA3D70A`: no .rdata
  operand fits one store and not the other; eax is the btRigidBody there and the
  constructor's result only after `mov eax,ebx` (0x30048E), and no flag is read
  before `ret`. No address in the bytes, no relocation.
- **What else changes:** the box stops on the surface instead of 10 cm into it. It
  slides about as far (simulated, from the drop to rest: flat 0.14 m, stock 0.16;
  rough 0.33, stock 0.34; crouched 0.21, the same; slope 2.05, stock 2.31; off a
  ledge 0.32, stock 0.44) and lies a little more tilted after a long fall (19
  degrees off a ledge, stock 13).
- **Other paths:** carried, the battery has no body in the world (`deactivate`); a
  drop, a put into a container (`put_item`) and patch 92's distance points read its
  position as before; patch 78's carry is the artefacts' (their drop is analytic, no
  physics body). Not looked at: a world load or resync while it is carried (its
  saved state has no CCD values; the body is the constructor's).
- **Simulation:** a match plays out otherwise: the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv`.
- **Checked:** selftest check 106 (`test_battery_ccd`) and the late join's site
  checks; the simulation. **Not run in a game.**
- **In game:** a battery mode (Protective device, mode 2) on a map with raised
  floors (the Chemical plant's walkways, a building's upper floor): carry a battery,
  stand on a floor over open space or at a ledge, die (suicide key) with it in your
  hands; again on open ground and on a slope, ten times each. Stock, some drop
  through and the battery is never seen again (its marker under the map, no one can
  take it); with patch 106 every one lies on the floor and can be taken.
