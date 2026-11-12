# Player reports and praise

The lobby accepts `49 05 <nickname> <kind> <reason>` and stores it in
`g26_player_comments` in `srp.db`. It used to reject every submission with
`st_error_during_storing_user_comment` (shown as “Database error.”).

The existing Flash sources establish the meaning of both bytes:
`CommentKind.as`, `ClaimReason.as`, `Compliment.as`, and the menu's
`comment_about_user` calls. No client or content change is required.

| Kind | Reason | Choice |
|---|---|---|
| 1 | 1 | Report: bot or cheater |
| 1 | 2 | Report: swearing |
| 1 | 3 | Report: unsporting behavior |
| 1 | 4 | Report: account trading |
| 2 | 1 | Praise: friendly player |
| 2 | 2 | Praise: good player |
| 2 | 3 | Praise: born leader |

The protocol parser converts these pairs into `PlayerComment`; other pairs
are refused. The reserved `FE` kind remains the existing chat-mute command
and is parsed separately. The authenticated session supplies the sender. The
target is resolved by displayed nickname, ignoring ASCII case, including offline
accounts. Missing targets and self-comments are refused with
`st_player_not_found`.

Each row preserves both account IDs, both displayed names at submission, the
server's Unix timestamp, and the kind/reason. A unique index makes an identical
choice by the same sender about the same target on the same UTC day idempotent,
including after a restart or rename. This is an SRP policy, not a recovered
original-server rule. Different reasons, senders and days remain separate.

Only a successful repository write is acknowledged with `83 49 05`; the stock
client reads no body for event 5 (`lobby_requests.md` §4). A failed write, busy
account writer, a SQLite lock held by another connection, or disabled persistence
(`SRP_DB=off`) still produces the storage error. The lobby does not wait through
SQLite's normal five-second lock timeout; the repository's timeout is restored
after the write attempt. Nothing is silently queued and acknowledged before it
is stored.

Feedback does not automatically ban players or change ratings, rewards or
match simulation. Nothing is sent to the target. There is no new moderation
page; operators can review the stored rows directly, for example:

```sql
SELECT seq, datetime(at, 'unixepoch') AS submitted_utc,
       sender_id, sender_name, target_id, target_name, kind, reason
FROM g26_player_comments
ORDER BY seq DESC
LIMIT 100;
```

The additive schema migration 15 follows all existing migrations and leaves
accounts and prior audit history intact.
Feedback rows are independent of account replacement and deletion, as the admin
audit trail is. Tests cover report/praise byte pairs, the separate mute command,
all seven choices through lobby transport, missing/self targets, failed writes
and retries, daily deduplication, database contention, upgrade, rename and
reopen. Actual client play and deployment require separate validation.

Review validation on current main is recorded in the pull request.
