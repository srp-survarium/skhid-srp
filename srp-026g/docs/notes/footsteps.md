# Footsteps lost to re-simulation (patch 101)

The players' B20 (backlog 2026-10-21): "When walking, the sound randomly disappears sometimes".
Later builds: 0.51c5 "Fixed a bug which caused faulty player footstep sound playback (in some
situations). Now the sound is consistent." The code below is 0.27d0's, the nearest later build we
have; the rule it removed is the one this patch relaxes.

## What 0.26g does (PROVEN, the original exe)

- A step is an animation event of the body's `player_sound_events` channel
  (crouch_steps.md): `player_equipment_sound_effect::on_player_sound_event` (0x2A6470) is called
  with the event; domain 0 and 1 are a step of one foot or the other (`on_foot_step` 0x2A6520 with
  the foot's position, effect +0h / +18h), 2..4 three rustles.
- Before the switch (0x2A6475..0x2A6493): if the player has a world core (+120h), an event whose
  time (+10h) is **at or before the core's `maximum_time` (+18FC4h)** is dropped. `maximum_time` is
  the newest tick simulated once (set to a history item's time after its move, 0x2D0FCB; 50 by the
  clear, 0x2D0776). The same "first simulation only" test is at nine other places (0x23EA20..
  0x23F634, the time rule 0x267AD2, 0x29618C, 0x297BDC).
- The body animates in the simulation, so a re-simulated tick fires its steps again, and the test
  keeps them from playing twice. A game re-simulates whenever another player's input reaches it
  after its tick (every relayed input that is late: match.md section 5.1) and at a resync. A step
  that **only the re-simulation has** (the prediction kept the other player standing or on his old
  course; the real input moved his gait) falls at a played time and is never heard. So another
  player's steps drop out when he starts, stops or changes pace under lag, and come back once
  his ticks are first simulated again.

## What 0.27d0 does (PROVEN, the BCJ-decoded exe)

- `player_equipment_sound_effect` is a `drawable_object` (RTTI 0xB15F1C, vtable 0xA364A4): its
  event handler (0x29A980) no longer plays anything and has no time test. A step stores the
  event's time per foot (+54h / +58h) and the movement state (+5Ch / +60h); the rustles set flags.
- Its per-frame update (vtable slot 0, 0x29AFA0 -> 0x29AEF0) plays a foot's step when its stored
  time is newer than the one at the last update (+68h / +6Ch, 0x29AF5B / 0x29AF74), then keeps the
  times. A step a re-simulation makes plays (a frame later); one it only repeats does not.
- None of the ten `maximum_time` tests is left in 0.27d0 (its `maximum_time` is +1906Ch; no compare
  with it). 0.69d records the step times the same way (on_player_sound_event 0x2002A2AA0 stores
  them at +7B8h..+7C8h).

## The patch

One lohook at the test (`player_sound.step_guard`, 0x2A6485, 14 bytes), Rust `footsteps`:

- a rustle, another object's event, feet not read: the game's own test, unchanged;
- a step past `maximum_time`: plays, as in the original;
- a step at or before it (a re-simulation): plays when the same foot of the same effect played
  nothing in the 250 ms before it; otherwise it is the step already played, its time moved by the
  correction, and is dropped. A foot steps once a cycle (600 ms at the quickest, the sprint
  clips), so 250 ms keeps every real step and drops the copies.
- The newest played time per foot is kept for 128 effects (three player objects a player); a
  new effect takes a free slot, else the oldest. An effect not remembered plays its step.
- Never fewer steps than the original: every step the original plays still plays.

Nothing a match computes: a step is a sound, a decal and particles, and differs between games
already (first-person set for oneself). No simulation_changes.tsv row.

## In-game check

Two clients, one with 150-250 ms of added latency (or a real far player). From the other client,
watch and listen to him: walk/stop/walk in short bursts and change between walking and sprinting
near the listener, on concrete. Before the patch some first steps after each change are silent;
after it every step is heard, none doubled. The listener's own steps are unchanged.
