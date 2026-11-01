# Anti-AFK

The owner, 2026-11-01: the simplest version first.

## The rule (the owner's)

- A player who gives no input for **3 minutes** of play is removed from the match at once.
- Input: a key or action pressed or let go, or the view turned (the mouse). Held keys and a still
  mouse are no input.
- No warning, no message, no client change.
- Removed as a player who leaves by its own word: its place is given up, its game is told it left
  (the match closes on it), the others see it leave. No other punishment, no ban.
- **Private rooms (a PIN): never.** Public rooms, Play and bot matches: yes.
- Bots are never removed.

## Pauses (picks, to keep slow players in)

- The clock runs only while the player is alive in a match in play: loading, a late join still
  loading, the countdown and after the end never count.
- Dead (or waiting to respawn): paused, not reset.
- Its game not heard from for over 2 s (a hitch of its link, a resync, a slow PC that froze):
  paused.
- Alive or dead is read from the newest trusted world; with none in the last 10 s, paused.
- A game that connects again (a crash, a new start) starts from nothing.

## The setting

- `SRP_ANTI_AFK`: seconds, default 180, at least 60; `off` turns it off.

## Parked

- A smarter stage against AFK scripts: points, damage and objective progress as the signals,
  and an on-screen check (a code typed in chat) before the removal. Parked: no server text is
  sure to be seen in a match (Hide chat in match, F11, Blackout).

## Where

- `servers/match-server/src/v026g/afk.rs`: the rule (one state a player, one transition), the
  setting.
- `servers/match-server/src/v026g/game/afk_watch.rs`: what each player is doing at a poll, the
  removal.
- Not run in a game.
