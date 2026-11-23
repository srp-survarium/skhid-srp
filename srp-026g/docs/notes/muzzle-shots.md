# Shots from the muzzle (patch 200)

The owner, 2026-11-23: "Bullets leave the barrel, not the eye. Full simulation of where the weapon
looks, there shots go, with simulation with recoil being the next thing on top." And, the same
day: "the rule is being closest to bodycam as possible, which means full direction gun
simulation". Where a choice comes up, it is Bodycam's: the gun's own direction. The proof is
`client_patch.md`; the aimed view that goes with it is `aim-pose.md` and patch 197.

## The rules

- **Where a bullet starts:** the gun's fire point (the model's `barrel_point`, the weapon's
  +6E0h), on the gun as the simulation poses it. Fully aimed that is 5 to 10 cm under the eye's
  line and 30 to 110 cm ahead of the eye.
- **Where it goes:** the way the gun looks: along the barrel (the fire point's own axis),
  wherever the hands hold the gun. From the hip, fully aimed (the gun laid by the aim's alignment),
  while the sight comes up, and out of a sprint or any other pose alike. Nothing trues it to the
  sights and nothing falls back to the eye (the owner, asked: "Always follow the barrel", "Raw
  barrel axis").
- **The spread** is the game's, around that way, and the kick and the camera's own motions turn
  the eye and the hands as they did: the gun's pose is made from both.
- **A wall between the eye and the muzzle:** the shot leaves the eye instead (along the same
  way), so it meets that wall. No bullet starts beyond what the barrel pokes through.
- **Who:** every player, in every game, computed in the simulation from the simulation's own
  state (the animation tree, the weapon's records, the aim's progress). A bot's shots are a
  player's.
- **What does not change:** the recoil's numbers, the spread's, the damage, the bullets' flight,
  the world's format (a bullet's start is a place, as before).

## What the barrel's own axis means

- **Fully aimed** the fire point's axis is within 0.6 degrees of the sights' line on the guns
  measured (AK-74M 0.23, PM 0.58, the others 0 to 0.2): the models' own. A bullet follows the
  barrel, so on those guns it lands that much off the sights (the PM: 30 cm at 30 m).
- **A gun not brought up** shoots where it points: a sprinting player's gun is 54 to 58 degrees
  left and down of the view (measured below), so a shot fired as the hands come out of a sprint
  goes low and left until the gun is up.

## What follows from it

- **From the hip the centre of the screen is not the bullet.** The HUD's crosshair is put where
  the own player's gun looks: the place its shot's line first meets the level (150 m along it
  otherwise), through the frame's camera (the hud movie's `srp_crosshair_at`). Drawing only. The
  movie's side is run in Ruffle (`scripts/muzzle-crosshair`, `tools/check muzzle-crosshair`): a
  place in parts of the screen is that pixel at 1920x1080 and 1280x720, and 0.5, 0.5 is where the
  movie's own layout had the crosshair.
- **The gun is posed at the shot's own time.** The game's copy was the eye of the step before (up
  to 50 ms old), which in a burst carried the hands clip's camera turn of the pass before (the
  H&K 416, the M4A1: up to 0.67 degrees). Those bullets now go where the gun points then.
- **Bots** aim their view. From 10 m on they fire down the sights (`bots/reflex.rs`,
  `ads_near_m`), where a shot's way is the eye's line as before. Nearer (15 % of their near
  targets) or running for cover they fire from the hip: there a bot turns its view by the gun's
  measured line and muzzle place (`hip_line_deg`, `hip_muzzle_m`, `hip_turn`), so that the gun's
  line is on its aim.
- **Replays** of games before the revision that ships this play out otherwise: a row in
  `simulation_changes.tsv`.
- **Peeking:** a bullet no longer clears a ledge the eye sees over and the barrel does not.

## Measured on real matches' memory

Three full-memory dumps of the owner's game inside matches (2026-11-20 and 2026-11-22, 33 held
guns) and two in the lobby. The game's own `compute_bone_matrix` was run under Unicorn on each
dump's memory for every held gun's bones, and patch 200's rig made of them
(`private/scope-shot-045/muzzle-shots/dump_pose.py`).

- **The bone function as the patch calls it gives the game's own values.** Bone Item times the
  player's transform is the eye the game kept (+44Ch) to 0.0 mm and 0.000 degrees, for every
  player of a match.
- **The hip pose is what the game draws.** For the players drawn in the first person the rig's
  fire point is the one `weapon::draw` kept (the weapon's +620h): 0.0 cm apart in the lobby, 0.7
  and 1.8 cm in a match (a frame's motion between the two).
- **A gun held ready from the hip** (eleven guns): its barrel is 0.5 to 2.4 degrees off the eye's
  line, on average 1.0 right and 0.3 down; its muzzle 10 to 15 cm right of the eye, 8 to 16 cm
  under it and 44 to 92 cm ahead. So from the hip a bullet lands right of the screen's centre:
  about 13 cm, plus 1.7 cm a metre. The HUD's crosshair shows it.
- **A gun not held ready points elsewhere:** a sprinting player's 54 to 58 degrees left and down,
  some states' 73 degrees up, a dead player's 125. A shot fired as the hands come out of such a
  pose goes where the gun points then. **Not known:** how often that is (a trigger pressed in a
  sprint fires at once, by the bots' model of the cadence); `SKHID_SHOT_LOG` in a game tells.

### The built exe's shot on those matches

The same dumps, the built exe's own code and arena laid over the dump's memory
(`private/scope-shot-045/muzzle-shots/dump_shot.py`): `instant_fire` run under Unicorn from its copy
of the eye to its call of `get_dispersed_bullet_direction`, for every held gun, beside the original
exe's code on the same memory. The hook, the Rust, the engine's `compute_bone_matrix` and the
level's own `ray_test` on the match's collision all run for real; only GetCurrentThreadId is stood
in.

- **Twenty-five held guns** of two matches (held ready from the hip, half raised, fully aimed; a
  sprint's pose; other states; the dead): twenty-four leave the muzzle along the barrel, within
  0.05 mm of the place and 0.01 degrees of the way this note's arithmetic gives from the same
  bones, each after one ray of the level's collision from the eye to the muzzle (33 to 156 cm)
  that met nothing.
- **One** (a dead player's gun, its muzzle 127 cm from the eye) had the level in that ray's way:
  its shot leaves the eye, the same way. The wall's rule on a real level.
- The stack at the call is the original's in every case; no run failed.

## A recorded match played back with it (2026-11-23)

The owner's go for a headless replay. A match recorded on revision 71 (seven players) played back
for five minutes by the built exe (revision 72's tree with patch 200, the barrel's axis always) on
a display of its own, `SKHID_SHOT_LOG` on (`private/scope-shot-045/muzzle-shots/replay_shots.py`).
The recording is from before the patch, so its match plays out otherwise from the first shot on;
its players still fire. 768 shots, every player's, no panic, assert or crash; the game ran until it
was stopped.

| the sight | shots | the way against the game's old copy of the eye (degrees) | the muzzle from that eye (mm) |
|---|---|---|---|
| fully raised | 710 | median 0.27, nine in ten under 0.80, the most 1.86 | 0 right (-16 to 15), 91 under, 733 ahead |
| coming up | 46 | median 0.62, the most 4.72 | 42 right, 96 under, 755 ahead |
| from the hip | 12 | median 1.39, the most 7.20 | 119 right, 91 under, 735 ahead |

- Three shots of the 768 left more than 3 degrees off the old eye's line; none anywhere near a
  sprint's 54. Nobody in this match fired with the gun still down.
- The numbers agree with the dumps': from the hip the muzzle 12 cm right and 9 under, the barrel
  1.4 degrees off (the bots' `hip_line_deg`, `hip_muzzle_m`).
- Unseen still: the crosshair on a screen (no picture was taken), two games of one match.

### Two games' logs of that match, side by side

The same recording played back by two games, one held to 60 frames a second and one to 27, each
shot logged with whose it is, the simulation's time, the game's own copy of the eye and its
replacement, all to the bit (`SKHID_SHOT_LOG`'s second line).

- **The same input gives the same shot.** 146 shots have the same player, time and eye bits in
  both logs: all 146 have the same replacement, bit for bit. No shot of one player at one time
  has two eyes.
- **The playback itself is not the same twice.** Four of the seven players fire at the same times
  in both (25 shots, none apart). The two who fire most, and one shot of a third, fire at other
  times in the two games (place 1: 69 shots at the same time, 53 and 41 apart; place 7: 38, 34 and
  19): when a held trigger's shots fall is the replay's own stepping, by its frames. That is the
  game's, with or without the patch, and why two playbacks' logs differ in order. It is not a
  test of two games of one live match, whose steps are the match's own.

## Recoil, on top (the owner: "simulation with recoil being the next thing")

The game's recoil is already a state of the weapon in the simulation: the kick (the recoil
calculator, the weapon's +468h; its top kick at +478h) is added to the player's look pitch, a shot
raises it, a downward turn of the mouse is taken off it first and a controller returns it
(`bots/recoil.rs` has the proof). The look pitch with the kick poses the eye, the hands and the gun
together, so in the stock game the kick is a turn of the screen.

With the bullet following the gun, the kick is the gun's motion and the eye need not take all of
it. Patch 197 draws it so: of the lift the kick gives the eye the view takes 40 % (`KICK_SHOWN`, a
first value: the owner, asked, "we will iterate on this"), from the hip too; the gun stays on the
simulation's aim, so it rises over the screen as the kick grows and comes back as the player pulls
it down. Nothing the simulation computes changes for it: the kick, its numbers and where the
bullets go are the game's. Not while the scope's own view is on (its picture is the screen).

Not done: the side kick (about 0.1 degree a shot: it scrubs the `recoil_horiz` clip, whose turn
on the screen is not traced) stays a turn of the view.

## The gun turns first, the view follows (the owner's first play, 2026-11-23)

The owner played the build (the barrel's axis always, the kick's share): "almost good, one thing
to build on top, weapons don't have momentum when turn, like they do in similar games", "it is
always bobling right in front of you", "make sure turn momentum looks good", "bodycam should have
perfected the technique".

Bodycam's technique is in the STALKER bodycam fork as a reproduction of the game's own camera
motion (`bodycam_reference/camera.cpp`, `CameraMotion::Update`): the mouse turns a control; the
control is smoothed (25 a second); the arms, which carry the gun, follow it quickly (20 a second)
and the camera slowly, at 2 a second while the turn is under 125 degrees a second up to 16 over
250; the view rolls against a turn. So on a slow turn the gun swings far ahead of a view that
drifts after it, and on a fast one the view keeps up.

The owner's second play, of the first build of it: "almost good camera is bobbling though", "the
weapon also throws the camera it seems", "which makes it a bit painful". That build fed the follow
the eye's whole motion; the steps' bob and the breath then moved the view and the gun out of step,
and the kick went through the lag and came back late. Since then the control is the input's aim
alone (the player's yaw, +490h, and the eye's pitch of its look pitch, +48Ch): the kick's share and
what the clips add to the eye move the view as they did before the follow.

Patch 197 draws exactly that relation and nothing else (`camera_trail.rs`): the gun stays on the
simulation's aim, so the mouse moves it at once and a bullet goes where it points; the view is
drawn as far from the gun as the reference's camera is from its arms. It is the view's own state,
a frame's: nothing a match computes, no step of the simulation, no lag of the gun behind the mouse
(the reference's arms are 90 ms of turn behind the control; a gun that late would be a late shot,
and a state of the simulation every game would have to agree on). The view is held at most 10
degrees across and 5 up off the gun from the hip, 3 and 2 fully aimed (the hands are the game's
first-person rig, not re-posed for a view far off the gun); the roll is the reference's 8 degrees
at 375 degrees a second of turn, at the fork's share of 0.43.

## A shot in a sprint (patch 201)

Asked in the same play: "you should also be able to shoot while sprinting, it is just that weapon
is not looking straight", "so shots will land where muzzle looks". The second is the rule as
built. The first was the game's own, in three places of the simulation (read in the exe;
`private/scope-shot-045/muzzle-shots/sprint-fire/FINDINGS.txt` has the listings):

- **A trigger ends the sprint.** `player_input::is_sprinting` is inlined three times (the speed
  and the stamina, 0x2FC207; the posture machine, 0x2FBCAA; a sprint's jump, 0x2F1231): SPRINT
  and FORWARD, and none of BACK, FIRE and CROUCH (`test eax,122h`).
- **The weapon waits for the sprint's clip.** There is no sprint state of the weapon: a sprinting
  player's is idle. Its fire predicate (0x2E3CB0) refuses while the weapon's +5ECh (the sprint
  clip to wait for: 0, 2 or 4 once the sprint posture selects its clips, 6 again when the last one
  has ended out of a sprint) is not 6.
- **A fire state ends any sprint.** `weapon_core::can_sprint` (0x2E3A10) takes the idle state, and
  a melee while +5ECh is not 6.

Patch 201 (`sprint_fire.rs`), as a melee already is: the three key tests refuse BACK and CROUCH
only; the fire predicate goes on past its wait while the user's posture is the sprint (after a
sprint it waits as before); `can_sprint` takes the fire state while a sprint's clip is selected,
so a fire begun in a sprint keeps it and one begun outside starts none. A match plays out
otherwise: a row in `simulation_changes.tsv`. `check_sprint_fire_026g.py` (`tools/check
sprint-fire`) runs the five tests in both exes.

Played by the owner the same day (355 shots logged, no crash): 25 shots left a gun 54 to 56
degrees off the eye's line, its muzzle 40 to 65 cm left of the eye and 47 to 57 cm under it: the
sprint's pose. So the hands stay in the sprint's clips while the weapon fires, and the shot goes
where the muzzle points, as asked; the fire state's own hands clip does not bring the gun up (the
reading of the exe expected it might).

Not done:
- Aim, a reload and a grenade still wait for the sprint's end; a jump is refused while the fire
  state runs; a bolt or pump gun's chambering ends the sprint.
- The bots let SPRINT go as they fire (`bots/mod.rs`), so they never do it.
- No world of a game with it was checked (`world-check`, `world-drift`): nothing new is written,
  but a sprint posture with a fire state is a pair no recorded world has.

## The view on the mouse, the gun behind the turn (the owner's third play)

Of the follow fed by the input's aim alone: "still a bit heavy boobling of camera", "the camera is
being flung w/weapon", "uncomfortably". A view that comes after the mouse, as Bodycam's camera
does, is not wanted here at any strength. So the view is the eye again, turn for turn (with the
kick's share), and the weight is the gun's: patch 197 draws the gun held from the hip behind a
turn, by half the lag of the reference's arms (the input's aim smoothed at 25 a second, the arms
after it at 20: at 60 degrees a second 1.7 degrees), at most 5 degrees across and 3.5 up; less the
further the sight is raised, none fully aimed, so the sights stay true. It swings the first
person's Weapon bone about the eye before the hands' IK (`hands_ik.solve`, 0x295B00).

What it costs: while the gun trails it is drawn where a shot does not go. The shot is the aim's,
which the HUD's crosshair shows. A gun that trailed in the simulation would need a state of the
simulation that every game agrees on at every step (the turns of the last tenth of a second), and
the game's own re-simulation of a step would have to rewind it: not attempted.
