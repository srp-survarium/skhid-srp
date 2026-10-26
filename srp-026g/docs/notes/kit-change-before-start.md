# A kit chosen before the match starts (patch 31)

The rules the code and the tests follow. Code: the Rust patch `kit_change`
(`client-patches/rust/patches/src/kit_change.rs`; `kit_change_026g.asm` until the Rust port),
README patch 31, `client_patch.md` section 3.29; the server: `TimelimitRule::before_play`,
`PlayerResult::kit_changes`, `autobuy::used`, the bots' `pick_kit`.

- **When:** from the player's spawn on entering the match until the match starts (the time
  rule leaves waiting/countdown). A choice for a tick is played at the next tick; a choice for
  the last tick before play switches in the tick that begins play, before its change. Never
  after.
- **Who:** a player in the match from its start, alive, who has done nothing yet: no shot, hit,
  kill or other event, no damage from another player. Several switches allowed until then.
- **Effect:** respawn in the same place, same facing (the look up or down is level, as at any
  spawn), what is left of the spawn invulnerability kept (none left: not invulnerable, the
  new kit's own earlier protection never kept), with the chosen kit; no death, no timer, no
  ticket; supplies from the account as at any spawn. Only the match's own places (the core's
  list, never the buffer past its end).
- **State:** after a switch the player stays in the pre-start state (warmup camera, movement
  lock) taken from the match's current state, the one the network client set the camera for
  (+1F1C60h); the start then releases them like everyone.
- **Ordering:** switches key off the server-ordered kit choice (0xF3), by the tick it is for.
  In each tick, the same on every client and in every replay of it: the respawn rule (tick
  list 0, key from the core's counter) with the switch and the tick's spawns, then the places
  (list 0, the late join's keys (place+1) << 16), which take the tick's kit choices, then the
  time rule (list 1). So a choice is played by the respawn rule at the next tick, and a switch
  is applied before the start transition or not at all.
- **HUD:** the kit selector shows at spawn before the start and stays open after a pick, the
  cursor where it was (another pick switches again); it hides at the start. It is shown for a
  live player exactly while the camera is the pre-start one.
- **Camera while the selector is open (before the start), as the lobby's character view**
  (the owner, 2026-10-18):
  - plain mouse moves move the cursor only; a left click (press and release with no more
    than 8 units of movement) is the selector's, as before;
  - a drag turns the camera while the button is held: the left button once it moved more
    than that, the lobby camera's buttons (right, 5th, 7th) at once; the cursor stays where
    the drag began (it is off the screen during the drag, so a drag that began on a kit picks
    nothing) and is back there at the release;
  - the wheel moves the camera nearer (forward) or further, 0.12 a notch as the lobby's;
  - the camera keeps the lobby camera's limits: distance 1.5 to 4.6, pitch within 60 degrees
    up or down (so it starts at 1.5, the stock warmup's 1.0 being nearer than the lobby's);
  - on the death screen: patch 46's free look (`death-camera.md`), the wheel nothing;
  - a button pressed while the cursor is shown (a drag, a pick on the death screen) and still
    held when the cursor goes (the start, the respawn) does nothing, held or let go: no
    shot and no aim without a press of its own (the owner, 2026-10-19: "players will shoot
    immediately once match starts"; "when mouse is released one shot still happens").
- **Death screen (in play):** the selector also stays open after a pick, the cursor where it
  was, and can be clicked again until the respawn; each pick is the next life's kit, the last
  one wins; it hides at the respawn as before. A pick for the respawn's own tick is the life
  after (the respawn rule ticks before the places), on every client alike. No switch while
  alive in play.
- **Late joiners:** join only after the start; no selector, no switch, their spawn untouched;
  they start with their first kit (the lowest kit the match loaded, as the slot's bind takes
  it; the same on `main`).
- **Bots:** pick a kit only after the start.
- **Results/supplies:** each kit choice before play counts as a life for supplies used;
  per-kit counters unaffected (a switch needs counters all zero).
- **Release:** changes the simulation: the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv` says so; the release that ships it makes
  that row its revision, its server's minimum patch revision, and from it on replays recorded by
  older games are refused (`session::v026g::replays`).
