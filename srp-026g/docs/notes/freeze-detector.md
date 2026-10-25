# Freeze detector (patch 71)

The rules the code and the tests follow. Code: `client-patches/freeze_026g.asm`,
`client-patches/freeze_026g.c` and `FREEZE_*` in `client-patches/patch_026g.py` (README patch 71,
`client_patch.md` section 3.58); the check: `client-patches/check_freeze_026g.py`.

- **The owner, 2026-10-25:** "we can patch it to detect freezes and stuff": players report short
  freezes at ~100 fps; the camouflage/decal bakes are one cause (patch 70); the detector tells us
  the rest, on the owner's and players' machines. Then: "freezes happen when many people are
  shooting": the line shows shots, hits, decals, impacts, sounds and the ticks simulated again.
- **A frame:** from the end of one swap chain present to the end of the next, on the main
  (render) thread, by QueryPerformanceCounter.
- **A long frame:** at least `srp_freeze_ms` and at least three times the median of the last 64
  frames (the median made again every 64 frames). `srp_freeze_ms`: a console variable, 40 by
  default, 0 to 100000, kept in `user.cfg`; 0 logs no frame (the summary still comes).
- **Its line:** one line of the game's log (channel `game`, info) per long frame:
  `srp freeze: <ms> ms in <state> (limit <ms> ms): ...`, then what each counter grew by in that
  frame alone, the simulation first:
  - logic ticks and their time; the present's time; the simulation's move (count, time);
  - ticks simulated, those of them simulated again after a rollback ("resimulated": a tick to a
    time not past the latest ticked to), rollbacks (world history loads);
  - bullets fired ("shots"), those fired again inside a resimulated tick ("replayed"), bullet
    hits, bullet decals and impact effects asked for, sounds started, kill events and kill feed
    rows (patch 67's two hooks run once in each);
  - match messages handled, datagrams received;
  - camouflage/decal bakes and loads of saved bakes (count, time), synchronous resource loads
    (count, time), resources a cook made, the resources' callbacks (count, time);
  - particle systems started, ActionScript invokes, the movies' advance (count, time) and
    drawing (time).
  - Times overlap where one runs inside another (a bake inside the callbacks; a move inside the
    logic tick); counts are exact, times to the microsecond (the time stamp counter, set against
    QueryPerformanceCounter after the first second; before that, 0).
- **The state:** menu (login), lobby, loading (the game's own loading flag), waiting, countdown,
  play (the time rule's state the camera keeps), replay (the world shown with another client).
  Read on the logic thread after each logic tick.
- **A match:** from the first frame out of the lobby (loading or the world) to the first frame
  back in it. At its end, when it had frames in the world, the summary:
  - the world's time and frames, and p50, p95, p99 and the longest of them (0.1 ms steps to
    100 ms, 1 ms steps to 1 s);
  - the long frames, by state (loading, waiting, countdown, play, replay);
  - the three longest frames in the world, each with its counters, longest first.
- **How many lines:** at most 100 long-frame lines a match (20 of them while loading) and 100 in
  the lobby between matches, then one line that says more were not logged; the summary counts
  them all.
- **Cost:** a counter is one `lock inc` at a function's start; a timed function two `rdtsc`;
  one QueryPerformanceCounter a frame; a line only for a long frame.
- **What a match computes:** nothing changes; the game reads none of it. A game with the patch
  plays with any game of the same release.
- **Getting the data:** the log only, for now (`Survarium_<user>.log`, the lines start with
  `srp freeze`). Sending the summary to the server is not built.
