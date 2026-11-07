# Anti-AFK

The owner, 2026-11-01: the simplest version first.

## The rule (the owner's)

- A player who gives no input for **3 minutes** of play is removed from the match at once.
- Input: a key or action pressed or let go, or the view turned (the mouse). Held keys and a still
  mouse are no input.
- No warning before, no client change.
- Removed as a player who leaves by its own word: its place is given up, its game is told it left
  (the match closes on it), the others see it leave. No other punishment, no ban.
- **The chat says so** (2026-11-02: "both in general chat and in a match"): one line
  "`<name>` was kicked for being AFK" in the match's chat, to everyone still in it, and the same
  line in the general chat, to everyone who listens to it, the removed player too (its lobby has
  it when it is back). Once a removal; nothing for a player who leaves.
- **Its sender is "System"**, the same word in every language (2026-11-02: "it can be System
  name", "System can stay system everywhere"): `System: <name> was kicked for being AFK`.
- **Private rooms (a PIN): never.** Public rooms and Play: yes.
- **Matches against bots ("Player vs Bots"): never.** The owner, 2026-11-02: "it should also be
  fine to afk w/bots".
- Both decided once, when the match is made.
- Bots are never removed.

## Pauses (picks, to keep slow players in)

- The clock runs only while the player is alive in a match in play: loading, a late join still
  loading, the countdown and after the end never count.
- Dead (or waiting to respawn): paused, not reset.
- Its game not heard from for over 2 s (a hitch of its link, a resync, a slow PC that froze):
  paused.
- Alive or dead is read from the newest trusted world; with none in the last 10 s, paused.
- A game that connects again (a crash, a new start) starts from nothing.
- "With bots" read as Player vs Bots, the only matches the lobby puts the server's bots in;
  every PvP match stays watched.

## The chat line (picks)

- A server's line: an account line (sender type 5) of id 0, named "System", no clan tag: the
  type the client reads a name for and draws as a player's line, id 0 in nobody's ignore list.
- The text in each receiver's language, by the language its game gave the lobby (English, Russian,
  Ukrainian, German, Polish; English otherwise), as the replay refusals are:
  - en: `<name> was kicked for being AFK`
  - ru: `<name> исключён из матча за AFK`
  - uk: `<name> виключений з матчу за AFK`
  - de: `<name> wurde wegen AFK aus dem Match entfernt`
  - pl: `<name> został wyrzucony z meczu za AFK`
- The name is the character's, as the match shows it.
- A player with "Hide chat in match" on does not see the match's line (patch 58 hides channel 5);
  the general line is the lobby's.

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
- The chat: the match server's `ResultsSink::idle_removed`, srp's `Settler`, the lobby's
  `Inbox::idle_removed` and `Core::idle_removed` (`servers/lobby-server/src/v026g/social.rs`).
- Not run in a game.
