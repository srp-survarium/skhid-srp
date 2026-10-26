# Battery drop at the carrier's death

The owner's rule and what it changes. Exe patch 80, the Rust patch
`battery_drop` (`client-patches/rust/patches/src/battery_drop.rs`, its sites
`battery_drop.toml`); proof `client-patches/check_battery_drop_026g.py`
(Unicorn) and self-test check 80. The findings behind it are in the re-kit
rows of this change (0x2D7650, 0x300DC0, 0x300F40, 0x301270, 0x344A70,
0x344B30, 0x344C30, 0x357480).

- **Decision (the owner, 2026-10-26):** option B of the design study. A
  carried battery drops in the match simulation at the moment its carrier
  dies, from the hand, with the hand's velocity. It falls for real and is
  picked up exactly where everyone sees it.
- **Before (0.26g):** a dead carrier's battery stayed "in the hand" until the
  item switch found it ready to be deactivated. The death's hands
  expression made it ready about the death clip's end later (1 to 2 s,
  LIKELY). Then it appeared at the invisible clip's hand, up to 2.5 m from
  the drawn body, and fell again.
- **When:** on the death's tick. on_player_death makes the wanted item none;
  the item switch (tick_active_object 0x2D7650) asks the battery
  `is_ready_to_be_deactivated` (0x301270). The patch answers yes at once for
  a dead carrier (its user +8 with +33Ch 0); otherwise the game's own answer.
- **Where:** at the hand's last tracked pose. The carried states' execute
  (0x344B30) now tracks the hand every tick (the je at 0x344B89 is nops):
  the simulation's own "Weapon" bone (get_bone_transform 0x2DA540, the
  player's animation player +750h, not the drawn bones) through
  compute_carried_transform 0x344E40, and the velocities by finite
  difference over the tick.
- **How fast:** finalize (0x344C30) gives the battery the hand's linear and
  angular velocity; release_body (0x300DC0) applies a central impulse of
  10 x the linear velocity (the body's mass is 10, LIKELY: the battery
  leaves at the hand's speed, a running carrier's included) and a torque
  impulse of the angular velocity.
- **Unchanged:**
  - a living carrier's drop: deactivate zeroes the velocities and finalize
    gives none for a living user (its own test, the state's vt+28h), so the
    impulses stay zero;
  - a put into a container: the put state tracked the hand before (its
    vt+28h is always 1); the same bytes as the original;
  - the pickup: the game's own, from the eye at the battery's body.
- **Edge cases:**
  - death during the take animation: the take state is left at once too
    (its leave stops the take animation, then finalize);
  - a suicide (K), a fall, a blast: any death, the same rule;
  - killed on the tick right after the take began: the take state starts
    tracking from the battery's own place on the floor (initialize
    0x344A70), so that one tick's "velocity" is the battery's move from the
    floor to the hand over one tick, and the battery is thrown off fast. A
    one-tick window, deterministic; not capped (open: the owner's call).
- **Who computes it:** every client, in the lockstep simulation. The
  tracked fields (state +2Ch, +6Ch, +ACh, +B8h, +0C4h) are serialized with
  the world (0x344CB0 / 0x344D70), so a rollback, a late join and a replay
  re-simulate the same drop; the check runs a tick from a state the game's
  deserialize loaded and gets the same bytes. They now change every tick
  while a living player carries (they did only after the death): the world
  bytes differ from an unpatched game's.
- **Server and bots:** srp simulates no battery (it reads a lying battery's
  position from the uploaded worlds, velocities skipped, and nothing of a
  carried state); bots only press use. No server change. srp-validate
  (not on main) checks a lying battery's position is a number; a dropped
  battery's velocity is not checked, and a carried state is not read.
- **The ragdoll (patch 76):** a body holding nothing gets no skeleton from
  the game, so a carrier's dying body may never be drawn for its ragdoll's
  start. A living carrier's last draw is kept (8 at most, 0.5 s), and a
  dying body the game no longer poses starts from one capture with the
  velocity it had (`a_carrier_whose_battery_drops_at_the_death_starts_from_its_last_draw_alive`).
  The body then falls with or beside the battery, and rests on it if on top
  (the rays read the batteries; the battery is never moved by a body).
- **Release:** a simulation change: the `next` row of
  `crates/session/src/v026g/simulation_changes.tsv` (older replays refused,
  the release's revision the minimum). Every player needs the release's exe.
- **Not run in a game** until the owner plays it.
