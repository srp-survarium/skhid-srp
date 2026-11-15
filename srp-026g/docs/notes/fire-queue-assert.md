# Empty shots: `m_bullets_in_queue > 0` (2026-11-15)

The owner's FFA Saloon match 0x30ffba2c (rev59, everyone a Colt Python) logged 85 engine
asserts `m_bullets_in_queue > 0` in `weapon_core_fire_state_base::on_shot_event` (0x30EBF0,
`weapon_core_fire_state_base.cpp:85`), 61 of them in 2.4 s. Each costs about 25 ms of logging on
the thread that ticks the weapons; none crashes, no bullet leaves.

## What happens

Read in the exe and measured in replays of that match's recording with a probe exe (rev59 plus
log lines at on_shot_event's queue test 0x30EBFD, the fire state's initialize 0x30E910, finalize
0x33AA40, execute 0x30EA00 and on_animation_end_impl 0x30EA50):

1. A semi-automatic fire state (state 305, `fire_queue_types` 1) starts with one round queued and
   shoots at its item clip's `shoot` mark. Its tick (execute 0x30EA00) refills the queue
   (reset_fire_queue 0x30ECC0) on a press after a release, while it is still playing.
2. With a refilled queue its clip's end does not end it (on_animation_end_impl: over when the
   queue is spent, or for an automatic mode with the trigger up). It goes on.
3. From then on every input change (the weapon's rebuild of its animations on a key change)
   starts the item clip again from frame 0, until a whole clip plays with no change. Measured:
   the empty shot comes exactly 50 ms after the key change (place 3 riad5 at 136 269 and
   136 400 ms: shots at 136 319 and 136 450).
4. The Colt Python (`wpn_magnum`) is the only live gun whose `shoot` mark is not at frame 0: at
   frame 2 of 20, 50 ms at 120 rounds a minute. Each restart crosses it again with the queue
   spent: the assert, and its bang (the fire state's second sound, `sound_events` played in turn,
   weapon_sound_effect::on_sound_event 0x297C50) with no bullet. A mark at frame 0 is not fired by
   a restart, so the other guns never show the assert.

Replay counts (the same recording, de-duplicated by fire state and callback time): rev59 as it is,
1636 shots and 27 empty; with the Python's `shoot` mark alone at frame 0 (a local test patch), 1526
shots and 1 empty (at the switch between hip and aimed fire, which keeps the spent queue). The
engine log of the first: 31 asserts, the second: 1. The shots differ because the bullets leave
50 ms earlier and the match plays out otherwise from there.

The other big Saloon match of rev59 (0x263a71ab, 16 players) replayed the same way: 1727 shots
and 4 empty (11 asserts logged). Its fast clicking was no rarer (236 presses under 450 ms after the
one before, against 377); the storm of 0x30ffba2c was one player's revolver held in its fire state
for 2.3 s at the match's end (22 of its 27).

## The fix: patch 148, the empty shot without its report

The owner, 2026-11-15: "weapon should not change". Moving the Python's mark to frame 0 (tried in
the replay above) changes when its bullet leaves, so the clip stays. `EMPTY_SHOT_QUIET`
(patch_026g.py) makes the assert's own skip (0x30EC1F, `je 0x30ECB2`, taken when its counter
0xAE2DF4 runs out) always taken: on_shot_event returns 0 for a spent queue at once, as it did
after the report. Nothing else changes: the shot of a queued round, the queue, the clip's
restarts, and the bang a restart plays (its `sound_events` mark is crossed too).
`check_empty_shot_026g.py` runs the function in the original and the patched exe.

## Not fixed: a refilled semi-automatic fire state hangs

Step 3 holds for every semi-automatic gun: while the player keeps changing keys after such a
refill, the fire state does not end, and its weapon neither fires the refilled round nor reloads.
Seen at the end of the replay (place with the empty Python, match 434-437 s): 2.3 s in the fire
state with the magazine empty, its reload presses (0x40) ignored, over only once its keys stayed
still for a clip's length. A fix changes what a click during the cycle does (fire at the cycle's
end, or nothing), which is the owner's call.
