# The view fading on the death screen

The owner (2026-10-26): "when you die, your view should start shrinking like you are dying and eyes
close, speed based on the timer". After the first videos: blood and interface stay; less cartoonish;
of three variants, the third without its lids: "keep the 3 before lids close, so it will be not a
complete darkening before restart". Code: the hud movie,
`flash/hud/src/survarium/controls/DeathEyes.as` (placed and fed by `HUD.as`); check:
`scripts/death-eyes/check.sh`. No exe change.

- **When:** while the hud shows the respawn countdown (`root.set_respawn_time`): from the death to
  the respawn. Nothing without a countdown (alive, before the start, a respawn with no delay).
- **Effect:** the view slowly loses brightness and colour (a dark grey wash, up to 60 %) under a
  soft vignette that narrows, slightly darker toward the top, with three faint dims like a slowing
  heartbeat (at 22 %, 48 % and 74 % of the countdown). No lids, no fade to black.
- **At the end:** the view at its darkest (the wash and the vignette at full), the scene still
  faintly visible (0.40 of it in the middle of the screen); then clear at once at the respawn.
- **Speed:** the countdown's own: a 5 s respawn fades in 5 s, a 10 s one in 10 s; the same
  at the same share (shown in the middle: 25 %: 0.89, 50 %: 0.71, 75 %: 0.52, end: 0.40).
- **How the time is known:** the exe sends `"<prefix> MM:SS"` (the localized
  `st_respawn_time_pref` and `%s %02d:%02d`, 0x236C30) only when the seconds change, the seconds
  being the respawn rule's time left rounded up (`fill_match_hud_state` 0x316C20: ceil of
  (respawn time - now) / 1000), and `""` at 0. So a new value N comes when exactly N s are left:
  the movie takes the end as now + N s at each change, the start at the first. A time that is not
  whole seconds (a radio: 8 s, other modifiers) is right from the second value on.
- **A countdown that stops** (the world stopped, the match's end): the fading waits 1.1 s past the
  last change and goes no further until the next one.
- **Clear at once:** at the respawn (`""`), a rollback that undoes the death (the countdown goes,
  `""`), the hud's reset. No animation.
- **Over and under:** on the stage below the hud's root, above the screen effects (the blood stays
  under it, the owner: the order does not matter): the kits, the countdown, the kill feed and the
  rest of the hud stay over it, readable; it never takes a click (kits clickable, keys 1 to 3 as
  before).
- **F11 (hide the interface):** it still fades: a screen effect like the blood, not the interface
  (hide-ui-key.md).
- **Blackout:** it still fades (an immersion effect; with the countdown hidden it is the only sign
  of the respawn coming).
- **Death camera:** whatever camera the exe draws (patch 46's turn from the corpse's eyes, a
  ragdoll camera) is under it, unchanged.
- **Spectating and replays:** the countdown is the hud's own place's (`fill_match_hud_state`
  reads the hud's place): your own death, or the replay's recorded player's; another player's
  death never fades the view.
- **No blur or real desaturation:** the 3D view is drawn by the engine behind the Flash movie; the
  hud can only draw over it (the grey wash stands in for the loss of colour).
- **Lockstep:** the hud only; nothing a match computes, no row in `simulation_changes.tsv`.
- **Checked:** `scripts/death-eyes/check.sh` (Ruffle, the hud movie's own class and kit panel):
  the curve at 0, 25, 50, 75, 100 % of a 5 s and a 10 s countdown, going down but for its dims,
  deepest at the end and not dark there, clear at the respawn; a time that is not whole seconds, a
  rollback, a stopped countdown, the texts, a real click on a kit over it; `--video` records it
  over a 5 s countdown (`--blood` with the hud's blood). **Not run in a game.**
- **Release:** the hud movie (its `movie` row of `release/content.tsv`), a content patch.
