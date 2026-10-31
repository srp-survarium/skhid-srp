# A bullet stops a drug

The owner (2026-10-31): "Patch the game so that if a bullet hits a person while (use/active), the
healing stops." Exe patch 84 (`client-patches/rust/patches/src/drug_interrupt.rs`).

- **A drug** is an item of `gameplay/items/drugs/`: the medkit (use 2 s, active 3 s), bandages
  (1 s, 1 s), antitoxin (1 s, 10 s), painkiller (1 s, 10 s). After the press it is in its use time
  (`activation_delay_sec`, nothing yet), then its active time (`activity_time_sec`, healing each
  tick).
- **Hit in the use time:** the healing never starts. Hand and leg injuries stay (they go at the
  first active tick).
- **Hit in the active time:** it stops at once. What it healed so far stays, injuries it removed
  stay removed.
- **The unit stays used.** It was taken at the press, as before. Another drug can be used at once.
- **A bullet** is a hit of type `ranged`: every round of every gun (no 0.26g ammo sets
  `damage_type`, so all are ranged). Melee hits (blunt), grenades and the trap (explosive,
  slash), falls (blunt) and anomalies (fire, radiation, toxic, shock) do not stop a drug.
- **Only a bullet that took health.** A bullet fully stopped by armour does nothing in the game
  (hit_impl stops there: no slow, no hit report); it stops no drug either. One that took any
  health does. Spawn protection: no hit, nothing stopped.
- **Whoever fired it.** A teammate's bullet too: 0.26g deals team damage, and a hit that takes
  health is a hit. A player's own bullets do not hit him: a bullet never hits its
  `m_ignorable_object`, a player (try_pierce 0x32D9DD), LIKELY its shooter. The suicide key's
  hit is ranged too, and kills.
- **Only a drug that heals.** The painkiller heals nothing (its only effect, for its 10 s, is 10 %
  less pain from bullets), so a bullet does not stop it (the owner, 2026-10-31: the painkiller is
  not stopped). The antitoxin heals the infection pool:
  stopped, and its toxic protection ends with it.
- **Every drug in use** of the hit player is stopped (a medkit and bandages both, if both run).
- **Red for a second** (the owner, 2026-10-31: "it can show red for a second"): the hit player's
  quick slot of a stopped drug shows its icon red for one second of game time, then as the game
  ends a drug (the last unit's icon stays for that second, then goes). A drug whose time runs
  out, a death and the HUD's clean-up show no red. Display only: nothing the match computes.
- **Every client the same:** decided in the match simulation (the hit as the game applies it),
  so every game, a late joiner (the drug's state is in the snapshot) and a replay agree. The match
  simulation changes (`simulation_changes.tsv` `next`): a replay recorded before it is refused.
- **The server** keeps no drug state (it relays inputs; the bots' heal play only presses the key
  and waits), so it needs nothing.

## How (exe patch 84)

- Site `player.hit_applied` (0x2D9CB1, a lohook of 6 bytes): `base_player::hit_impl` (0x2D9B90)
  after `damage_model::hit_body_part` (0x2E5080) said 1, which it says when the damage left by
  armour and protectors is above 0.0. EBX the victim, ESI the `hit_info` (+1Ch the hit type,
  +3Ch the time).
- Hit type 0 (`ranged`, `survarium::hit_type` 0x304370: ranged, blunt, slash, explosive, toxic,
  radiation, fire, shock) only.
- Each of the victim's 23 inventory slots holding a drug (vtable 0xA1C79C) in use (+2B0h) with
  influences (+288h, the pools it heals) gets `medkit::set_active(0, drug, the hit's time)`
  (0x33D390, `engine_medkit_set_active`): the game's own end of a drug, the one medkit::tick
  calls when the active time is spent or the holder died. It clears the use, unlinks the drug
  from the world's tick list, ends its drug_activity modifier only if the active time began, tells
  the listeners, zeroes the healed-so-far counters and unlinks its damage protectors.
- With 256 bullets or more in flight the game ticks bullets on several threads, hits and all;
  the stops take a lock, since every drug shares the world's tick list.
- Check: `client-patches/check_drug_interrupt_026g.py` (Unicorn: the hook in the patched exe,
  and the game's own set_active(0) in the use time, the active time and between them).

## The red second (display only)

- set_active(0) leaves the use timers (+2ACh, +2A8h) as they were; a drug whose time ran out has
  both at 0 (medkit::tick ends it so). So a drug ended with time left, its holder alive, was
  stopped by a bullet: the other caller with time left is the dead holder's tick. No new state
  in the game tells it.
- Patch 77's hook (`consumable_activity`, at each quick-slot update of the local player) reads
  that and keeps, per quick slot, only what it saw last: idle, a drug in use (its address, an
  identity never read through), or stopped (and the local player's simulation time, player
  +324h, when it first saw it so). Seen in use, then stopped: it sends phase 3 to the hud for
  1000 ms of that clock, then 0. Stopped but never seen in use (a previous life's drug, a late
  join's, another item in the slot): 0. In use again (a rollback took the hit back): in use.
  The clock run back by a rollback counts as no time.
- The hud's QuickSlots takes phase 3 as it takes the others: the exhausted stack kept, the
  slot's own icon tinted red (green and blue x0.3) where phase 1 dims it. No ActionScript timer.
- Checks: `check_consumable_activity_026g.py` (the hook's phases over the game's clock: in the use
  time, the active time, the last unit, a natural end, a death and the respawn after it, a
  rollback, another drug), `scripts/consumable-activity/check.sh` (the movie in Ruffle: red,
  not red, the last unit, the clean-up), the patch's unit tests.
