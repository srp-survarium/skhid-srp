# Battery last taker survives a world load

Pending fix, 2026-11-03. The native statistics battery record's byte +FD
identifies the player who last took that battery from a container. Event 26
(stolen and brought home) checks it against the delivering player, but the
native snapshot writer and reader omit it. A late joiner therefore can make
a different decision from a game that witnessed the taking. A resync can
also leave the client's stale value in place. See match.md §28 for the
original analysis and snapshot_rules.md §3.2 for the record layout.

The October 27–November 3 audit found 96 same-tick snapshot comparisons in
51 matches in which event 26 alone differs: replacing only those counters
reproduces the complete majority statistics bytes. Current revision-43
examples are 12f49e8b at 273250 ms (place 1, kit 2), 443cfdbd at 803100 ms
(place 11, kit 2), and 47959e2c at 419700 and 421050 ms (place 7, kit 2).
This identifies the observed difference; the unsaved byte cannot be read
back out of those old snapshots. It does not classify every resync in those
matches or prove the exact earlier taking event in each recording.

The fix extends patch 92's existing `battery.save` and `battery.load`
boundary. Native `on_event` remains the owner of +FD; there is no parallel
live taker table. Saving reads the byte alongside the life, and loading
restores it alongside the life. The per-record extension is:

```
u8 progress[2]
u8 held_count | 0x80
u8 last_taker              0..19, or FF for nobody
held_count × (u8 cell, u8 steps)
```

Legacy held counts are at most 60, so bit 7 unambiguously marks the new
byte. The server keeps `NotRecorded` distinct from `Saved(None)` and
re-encodes either representation exactly. Client legacy loads use nobody
instead of inheriting a stale local value. That is diagnostic compatibility,
not a reconstruction of missing history: the simulation-change gate must
exclude recordings from before the release. New and old clients must not
play together; the release's revision gate already enforces this.

The regression in `check_battery_progress_026g.py` runs the real native
battery serializer/deserializer and the native event-26 decision
(334C53–334C8C); only its statistics callback endpoint is a fixture. Its
negative control removes just the two codec hooks: one client counts event
26, another loads the same world and does not. With the hooks installed,
fresh and stale clients agree, loading nobody clears a later taker,
rewinding and loading forward restores the decision, and delivering to the
owner's team still counts no steal. The same check verifies exact record
restoration, legacy progress, multiple records and the reader's end.

Rust tests cover server legacy/new byte-preserving round trips, taker 19
outside the current active roster, every truncation, invalid takers/counts,
and the client's typed decoding. The change adds one byte per battery (ten
in shipped modes 2/3). Mode 4 has no battery records. The existing native
writer grows its buffer; the statistics section remains far below its u16
size limit (ten extended records add ten bytes).

The native constructor and reset behavior for `first_finder` are unchanged;
the separate cross-match quest-credit issue in match.md §28 is not a
snapshot disagreement and is outside this fix.
