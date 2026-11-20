# The gun's kick on a shot, as 0.45a6 has it

The owner (2026-11-18): how guns behave on a shot, as build 0.45a6 shows it. Pack `shot-kick-045`
(`scripts/shot-kick-045/make_pack.sh`, `gen.py`); the exe half is patch 165 (`aim-pose.md`).

- **What.** A gun's item fire clip (`<gun>/item/stand/3rd_view/shoot`, `states/fire/animations[0]`)
  moves its body bone `offset`: the kick, and what a scope shows of it. 16 stock guns' clips take
  0.45a6's bone curves (9A-91, A-545, AK-12, ASh-12, Kiparis, Mosin, PP-2000, Remington 870,
  SOCOM 16, SR-3M, SV-98, SVD, TOZ-122, UMP-45, Val, Winchester 1894, and the weapons sharing them),
  and the last-round clips (`animations[1]`) of the Kiparis, SOCOM 16 and SVD. 0.45a6 kicks sooner
  (the SVD's peak at 10 % of the shot against 20 %, the A-545's at 7 % against 60 %); its SV-98
  kicks 27 mm, ours not at all.
- **The rule.** The live clip's header, bone table and events; between them 0.45a6's nine curves a
  bone, as they are. Where its clip is longer (9A-91 15 frames against 10, A-545 15 against 5) every
  knot is times ours / its: a pass of the fire clip is one shot in both builds whatever its frames.
- **Held back: the rigs differ.** `gen.py` refuses a clip unless both builds pose every bone alike
  at frame 0 (0.1 mm, 0.1 degrees; the 19 shipped are within 0.06 mm, 0.08 degrees). Twelve guns
  fail: 0.45a6's `offset` rests 201 mm from ours on the PM, TT and APS, 171 mm on the Magnum, 136 mm
  on the Uzi, 74 mm on the Vepr, 65 mm on the SKS; the pistols' hammers 51 to 90 degrees; hand
  points 14 to 55 mm on seven guns (the AK's 45 mm, its top round 33 mm); the RPD's 0.8 mm. Their
  curves need the models' bind poses: an open choice. The Remington 700 keeps ours (0.45a6 holds
  its `offset` still); the VSSK's clip is 0.45a6's already; SR-2M and TOZ-34: no clip in 0.45a6.
- **View only.** The fire state's speed is its first hands clip's length times the fire rate
  (`(end - start) / 30 x rounds_per_minute / 60`, 0x30E5C0 from the constructor 0x30E677, +138h,
  the hands lexeme's; the item lexeme runs in its time-sync group), and the hands clips stay. The
  shot is the `shoot` event (0x30E9C6): events and lengths are the live bytes. A bullet starts from
  the player's own `Item` bone (player+44Ch); the gun's locators are read only when drawing
  (`calculate_locator` 0x243BC0). No server code reads a clip. No row in `simulation_changes.tsv`.
- **Checked.** `gen.py` reads each clip back (the live bytes around the tracks, the live length,
  0.45a6's controls on its knots); `vostok-port pack check` against the base; the 19 clips sampled
  against 0.45a6's with a second reader (equal to 1e-6). Not seen in a game.
