# Anti-AFK

The owner, 2026-11-01: the simplest version first.

## The rule (the owner's)

- A player who gives no input for **3 minutes** of play is removed from the match at once.
- Input: a key or action pressed or let go, or the view turned (the mouse). Held keys and a still
  mouse are no input.
- No warning before, no client change.
- Removed as a player who leaves by its own word: its place is given up, its game is told it left
  (the match closes on it), the others see it leave.
- **Daily account penalties** (2026-11-12): kick **3** bans the account for **30 minutes**;
  kick **6** bans it for **6 hours**. These are thresholds in one UTC calendar day; the count
  resets at **UTC midnight**. The count and ban survive reconnects and server restarts.
  A ban ends the account's sessions and refuses sign-in until its expiry. Midnight resets
  the count, not an existing ban. A longer or permanent ban is never shortened.
  These two thresholds each fire once per day; no additional threshold is defined after six.
- **The chat says so** (2026-11-02: "both in general chat and in a match"): one line
  "`<name>` was kicked for being AFK" in the match's chat, to everyone still in it, and the same
  line in the general chat, to everyone who listens to it. On an ordinary kick the removed
  player receives it too (its lobby has it when it is back); at a ban threshold its sessions
  are closed instead. Once a removal; nothing for a player who leaves.
- **Its sender is "System"**, the same word in every language (2026-11-02: "it can be System
  name", "System can stay system everywhere"): `System: <name> was kicked for being AFK`.
- **Private rooms (a PIN): never.** Public rooms and Play: yes.
- **Matches against bots ("Player vs Bots"): never.** The owner, 2026-11-02: "it should also be
  fine to afk w/bots".
- Both decided once, when the match is made.
- Bots are never removed.
- **Four or fewer connected human players: never.** Count all connected human places,
  including loading, dead and resyncing players; exclude departed players and bots. Recheck
  on every poll and after each removal, so simultaneous AFK removals stop at four humans.
  Small matches pause the timers; a fifth human joining resumes them. Exempt matches
  produce no AFK removal events and do not add to anyone's daily count.

## Pauses (picks, to keep slow players in)

- The clock runs only while the player is alive in a match in play: loading, a late join still
  loading, the countdown and after the end never count.
- Dead (or waiting to respawn): paused, not reset.
- Its game not heard from for over 2 s (a hitch of its link, a resync, a slow PC that froze):
  paused.
- Alive or dead is read from the newest trusted world; with none in the last 10 s, paused.
- A game that connects again (a crash, a new start) starts from nothing.
- "With bots" read as Player vs Bots, the only matches the lobby puts the server's bots in;
  public PvP matches are watched while more than four humans are connected.

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
- The penalties: `crates/session/src/afk.rs`, called by `Settler::idle_removed`.
  The account repository saves `afk_kicks` (UTC day and count) with the ban in one
  transaction; SQLite schema migration 7 adds the table without changing existing accounts.

## Validation (2026-11-12)

- Implemented in the `review/1029` worktree. `nix develop ./srp-026g --command tools/check`
  passed tests and Clippy with `-D warnings` for `session`, `match-server`, `srp`,
  `login-server`, `lobby-server` and `web-server`.
- Session: 491 unit tests passed, including UTC midnight, both thresholds, persistence and
  migration, ending sessions, preserving stronger bans, and refusing a sign-in that raced
  an AFK ban. Match server: 638 unit tests passed (19 existing ignored), including the
  private/PvE exemptions, four-human boundary, departures, simultaneous kicks stopping at
  four, and a fifth human enabling enforcement without counting exempt time.
- The `srp` removal-callback integration test passed, as did its nine end-to-end tests.
  The check runner reports the branch's existing `docker-compose.yml` setting as not covered;
  this change did not edit that file.
- Not played with a native client, deployed or released.

### Merge review

- Integrated `main` at `bc580831c` in `8c24df992`; resolved the registry export conflict
  by retaining both `IdleRemoved` and `VoiceSeat`. No blocking review findings.
- The combined tree passed `nix develop ./srp-026g --command tools/check`: all six server
  crate checks above (500 session unit tests and 682 match-server unit tests, plus their
  integration/doc tests and Clippy) and the runner's additional `replay-files` check
  (generated engine/hooks/arena checks, native replay harness, patch tests and Clippy).
- Detection uses accepted input changes only. Periodic scripted input can reset it; held
  keys alone do not. These are deliberate limits of this input-inactivity rule.
