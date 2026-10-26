# The eyes closing on the death screen

The owner (2026-10-26): "when you die, your view should start shrinking like you are dying and eyes
close, speed based on the timer". Code: the hud movie, `flash/hud/src/survarium/controls/DeathEyes.as`
(placed and fed by `HUD.as`); check: `scripts/death-eyes/check.sh`. No exe change.

- **When:** while the hud shows the respawn countdown (`root.set_respawn_time`): from the death to
  the respawn. Nothing without a countdown (alive, before the start, a respawn with no delay).
- **Effect:** a vignette that narrows, the screen darkening and two soft lids (top and bottom,
  curved, corners lower) closing over the 3D view. Slow at first, like fading consciousness
  (closing = share^1.8), two slow blinks near the end (at 62 % and 86 % of the countdown), shut
  when the countdown ends.
- **Speed:** the countdown's own: a 5 s respawn closes in 5 s, a 10 s one in 10 s; the same
  openness at the same share (25 %: 0.92, 50 %: 0.71, 75 %: 0.40 open).
- **How the time is known:** the exe sends `"<prefix> MM:SS"` (the localized
  `st_respawn_time_pref` and `%s %02d:%02d`, 0x236C30) only when the seconds change, the seconds
  being the respawn rule's time left rounded up (`fill_match_hud_state` 0x316C20: ceil of
  (respawn time - now) / 1000), and `""` at 0. So a new value N comes when exactly N s are left:
  the movie takes the end as now + N s at each change, the start at the first. A time that is not
  whole seconds (a radio: 8 s, other modifiers) is right from the second value on.
- **A countdown that stops** (the world stopped, the match's end): the eyes wait 1.1 s past the
  last change and close no further until the next one.
- **Open at once:** at the respawn (`""`), a rollback that undoes the death (the countdown goes,
  `""`), the hud's reset. No opening animation.
- **Over and under:** on the stage below the hud's root, above the screen effects (blood, the
  grenade's flash): the kits, the countdown, the kill feed and the rest of the hud stay readable
  over the closing eyes; the eyes never take a click (kits clickable, keys 1 to 3 as before).
- **F11 (hide the interface):** the eyes still close: they are a screen effect like the blood,
  not the interface (hide-ui-key.md).
- **Blackout:** the eyes still close (an immersion effect; with the countdown hidden it is the
  only sign of the respawn coming).
- **Death camera:** whatever camera the exe draws (patch 46's turn from the corpse's eyes, a
  ragdoll camera) is under the eyes, unchanged.
- **Spectating and replays:** the countdown is the hud's own place's (`fill_match_hud_state`
  reads the hud's place): your own death, or the replay's recorded player's; another player's
  death never closes them.
- **No blur or desaturation:** the 3D view is drawn by the engine behind the Flash movie; the hud
  can only draw over it.
- **Lockstep:** the hud only; nothing a match computes, no row in `simulation_changes.tsv`.
- **Checked:** `scripts/death-eyes/check.sh` (Ruffle, the hud movie's own class and kit panel):
  the openness at 0, 25, 50, 75, 100 % of a 5 s and a 10 s countdown, open at the respawn, a
  time that is not whole seconds, a rollback, a stopped countdown, the texts, a real click on a
  kit over the closing eyes; `--video` records the closing over a 5 s countdown. **Not run in a
  game.**
- **Release:** the hud movie (its `movie` row of `release/content.tsv`), a content patch.
