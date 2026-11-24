//! SQLite schema (PRAGMA user_version = 6). Version 1 -> 2 adds the `skills` table (`SCHEMA_V2_UP`), 2 -> 3 the
//! achievements table (`SCHEMA_V3_UP`, progression), 3 -> 4 the ban columns and the clan tables (`SCHEMA_V4_UP`, social),
//! 4 -> 5 the item uid renumbering and the presets tables (`SCHEMA_V5_UP`, items), 5 -> 6 the premium expiry table
//! (`SCHEMA_V6_UP`, store).

pub const VERSION: i64 = 23;

/// Login sessions (sessions.rs): the `u32` of the login server's `0B`, redeemed by the lobby's `2E`, kept so a server
/// restart does not lose them (the client re-sends the same session on its own lobby reconnect).
/// `last_used` is the unix second of the grant, of the last `2E` or of the last time the lobby saw the account connected.
/// Runtime state, not game data: not part of the versioned schema, made (IF NOT EXISTS) on every open of any version.
pub const LOGIN_SESSIONS: &str = "
CREATE TABLE IF NOT EXISTS login_sessions (
    session     INTEGER PRIMARY KEY,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    last_used   INTEGER NOT NULL
);
";

/// The SteamID64 a Steam-mode client claimed at its last successful sign-in (steam.rs). Information only: the client
/// sends it unverified, it never grants anything. `steam_id` is the u64 stored bit for bit as INTEGER; `last_seen` the
/// unix second of that sign-in. Not part of the versioned schema: made (IF NOT EXISTS) on every open of any version,
/// so an existing database file gets it without a version step.
pub const STEAM_IDS: &str = "
CREATE TABLE IF NOT EXISTS steam_ids (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    steam_id    INTEGER NOT NULL,
    last_seen   INTEGER NOT NULL
);
";

/// v22 -> v23: finished matches of the account (match options record +0x2D8; the client gives a player below
/// `damage_protector` 3 matches less toxic / radiation / shock damage, 0x2003EA53D).
pub const SCHEMA_V23_UP: &str = "ALTER TABLE profiles ADD COLUMN matches INTEGER NOT NULL DEFAULT 0;";

/// v21 -> v22 (clan registration deposits): the silver deposited on a reservation so far
/// (the client shows it as the registration progress, 9A sub 0 registration form). A reservation marked paid before
/// v22 paid the whole cost at once.
pub const SCHEMA_V22_UP: &str = "
ALTER TABLE clan_reservations ADD COLUMN collected INTEGER NOT NULL DEFAULT 0;
UPDATE clan_reservations SET collected = 50000 WHERE paid = 1 AND collected = 0;
";

/// The data part of [`SCHEMA_V22_UP`] (the migration of an existing file adds the column only when it is missing).
pub const SCHEMA_V22_UP_DATA: &str = "UPDATE clan_reservations SET collected = 50000 WHERE paid = 1 AND collected = 0;";

/// v20 -> v21 (clan rank names): `clan_ranks.is_default` now means "standard rank" (wire
/// `default`: the client shows its own text `clan_rank_<name>`); the three old default rows become the client's names
/// commander / assistant / soldier (standard). Custom ranks are untouched. Idempotent (matches only the old names).
pub const SCHEMA_V21_UP: &str = "
UPDATE clan_ranks SET name = 'commander', is_default = 1 WHERE idx = 0 AND name = 'Commander';
UPDATE clan_ranks SET name = 'assistant', is_default = 1 WHERE idx = 1 AND name = 'Officer';
UPDATE clan_ranks SET name = 'soldier', is_default = 1 WHERE idx = 2 AND name = 'Member';
";

/// v19 -> v20 (the Winner's Contract, docs/notes/store.md "Store offers"): the unix second the running contract was bought
/// and the bit mask of its days whose first win was paid (bit 0 = day 1). One row per account that ever bought one; a
/// contract older than its 3 days is over (the row stays until the next purchase). Idempotent (IF NOT EXISTS).
pub const SCHEMA_V20_UP: &str = "
CREATE TABLE IF NOT EXISTS contracts (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    started     INTEGER NOT NULL,
    won_mask    INTEGER NOT NULL
);
";

/// v18 -> v19 (leaver penalty, docs/notes/online.md): the UTC day of the last counted leave, the leaves counted that day
/// and the end of the queue block (unix seconds). Idempotent (IF NOT EXISTS).
pub const SCHEMA_V19_UP: &str = "
CREATE TABLE IF NOT EXISTS leave_penalty (
    account_id     INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    day            INTEGER NOT NULL,
    count          INTEGER NOT NULL,
    blocked_until  INTEGER NOT NULL
);
";

/// v16 -> v17 (event currencies, docs/notes/events.md). One row per account and event currency that is not zero; `slot` is
/// 0 / 1 / 2 = `global_event_currency` / `2` / `3` = item dict 951 / 1138 / 1139 = the 8B fields `event1` / `event2` /
/// `event3`. Idempotent (IF NOT EXISTS).
pub const SCHEMA_V17_UP: &str = "
CREATE TABLE IF NOT EXISTS event_wallet (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slot        INTEGER NOT NULL,
    amount      INTEGER NOT NULL,
    PRIMARY KEY (account_id, slot)
) WITHOUT ROWID;
";

/// v15 -> v16 (complaints / compliments, `48 06`). One row per comment
/// a player filed about another; written idempotent (IF NOT EXISTS). No foreign keys: the history stays when an
/// account goes.
pub const SCHEMA_V16_UP: &str = "
CREATE TABLE IF NOT EXISTS user_comments (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    at             INTEGER NOT NULL,
    from_account   INTEGER NOT NULL,
    target_account INTEGER NOT NULL,
    kind           INTEGER NOT NULL,
    a              INTEGER NOT NULL,
    b              INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS user_comments_from ON user_comments (from_account, kind, at);
CREATE INDEX IF NOT EXISTS user_comments_target ON user_comments (target_account);
";

/// v10 -> v11 (onboarding): the client's tutorial-gate flags (S->C 9D `F`: bit 7 nickname chosen, bit 6 faction
/// chosen, bit 5 tutorial complete; docs/notes/onboarding.md) and the character face byte (87 account body +0x2F6)
/// per account. Accounts that exist before v11 already play in the full lobby (the earlier reference implementation sent
/// F = 0xE2 to everybody) and kept their login name as the nickname: they get F = 0xE2 and nick = login name, so a NULL nick
/// means "no nickname chosen yet" from now on.
pub const SCHEMA_V11_UP: &str = "
ALTER TABLE profiles ADD COLUMN onboarding INTEGER NOT NULL DEFAULT 0;
ALTER TABLE profiles ADD COLUMN face INTEGER NOT NULL DEFAULT 0;
UPDATE profiles SET onboarding = 226;
UPDATE accounts SET nick = name WHERE nick IS NULL;
";

/// v1 -> v2: learned skills per account (skill id -> points).
pub const SCHEMA_V2_UP: &str = "
CREATE TABLE skills (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    skill_id    INTEGER NOT NULL,
    points      INTEGER NOT NULL,
    PRIMARY KEY (account_id, skill_id)
) WITHOUT ROWID;
";

/// v2 -> v3: achievement progress per account (value = the number the client shows, run = the current streak of a
/// streak achievement).
pub const SCHEMA_V3_UP: &str = "
CREATE TABLE achievements (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    ach_id      INTEGER NOT NULL,
    value       INTEGER NOT NULL,
    run         INTEGER NOT NULL,
    PRIMARY KEY (account_id, ach_id)
) WITHOUT ROWID;
";

/// v3 -> v4 (social): sign-in bans / suspensions on the account and the clan model.
///
/// * `ban_kind` 0 none, 2 banned, 3 suspended (the `0C` refusal codes); `ban_reason` 0..15 (`st_ban_reason_*`);
///   `ban_until` unix seconds, 0 = permanent.
/// * `clan_ranks.perms`: bit 4 invite, bit 3 kick, bit 2 change rank, bit 1 daily message, bit 0 battle commander
///   (the order the client unpacks the mask in, 0x2800A3..); rank 0 is the commander.
/// * One clan per account (`clan_members.account_id` is the key).
pub const SCHEMA_V4_UP: &str = "
ALTER TABLE accounts ADD COLUMN ban_kind   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE accounts ADD COLUMN ban_reason INTEGER NOT NULL DEFAULT 0;
ALTER TABLE accounts ADD COLUMN ban_until  INTEGER NOT NULL DEFAULT 0;

CREATE TABLE clans (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    abbr           TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at     INTEGER NOT NULL,
    daily_message  TEXT NOT NULL DEFAULT '',
    xp             INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE clan_ranks (
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    idx         INTEGER NOT NULL,
    name        TEXT NOT NULL,
    perms       INTEGER NOT NULL,
    is_default  INTEGER NOT NULL,
    PRIMARY KEY (clan_id, idx)
) WITHOUT ROWID;

CREATE TABLE clan_members (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    rank        INTEGER NOT NULL,
    joined_at   INTEGER NOT NULL
);
CREATE INDEX clan_members_clan ON clan_members(clan_id);

CREATE TABLE clan_invites (
    clan_id     INTEGER NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    inviter_id  INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (clan_id, account_id)
) WITHOUT ROWID;

-- a clan name + abbreviation reserved by an account before the clan is created (48 07); paid = registration paid
CREATE TABLE clan_reservations (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    abbr        TEXT NOT NULL,
    paid        INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
";

/// v4 -> v5 (items), part 1: item uids start at 100001 (`UID_BASE`). An item uid that is a valid dictionary id is read by the lobby
/// movie as that dictionary item in ammo and pocket slots (the pocket crash). Every account whose
/// smallest uid is below 100001 gets all of its uids shifted by 100000 in every table that stores one (items, slots,
/// item_mods, attachments weapon and module, bound, profiles.next_uid). Two steps through negative values so that a
/// shifted uid never meets an unshifted primary key; accounts without items only get next_uid raised to the base.
///
/// Part 2 of the same step: equipment presets (`58` requests, `BB` push). A preset remembers which item sits in which
/// slot (only occupied slots are stored) and the rounds / units placed there. `id` is the per-account preset id the
/// client sends back; `pos` keeps the list order.
pub const SCHEMA_V5_UP: &str = "
CREATE TEMP TABLE uid_lift AS SELECT account_id FROM items GROUP BY account_id HAVING MIN(uid) < 100001;
UPDATE items SET uid = -(uid + 100000) WHERE account_id IN (SELECT account_id FROM uid_lift);
UPDATE items SET uid = -uid WHERE uid < 0;
UPDATE slots SET uid = uid + 100000 WHERE uid IS NOT NULL AND account_id IN (SELECT account_id FROM uid_lift);
UPDATE item_mods SET uid = -(uid + 100000) WHERE account_id IN (SELECT account_id FROM uid_lift);
UPDATE item_mods SET uid = -uid WHERE uid < 0;
UPDATE attachments SET weapon_uid = -(weapon_uid + 100000), module_uid = module_uid + 100000
    WHERE account_id IN (SELECT account_id FROM uid_lift);
UPDATE attachments SET weapon_uid = -weapon_uid WHERE weapon_uid < 0;
UPDATE bound SET module_uid = -(module_uid + 100000) WHERE account_id IN (SELECT account_id FROM uid_lift);
UPDATE bound SET module_uid = -module_uid WHERE module_uid < 0;
UPDATE profiles SET next_uid = next_uid + 100000 WHERE account_id IN (SELECT account_id FROM uid_lift);
UPDATE profiles SET next_uid = 100001 WHERE next_uid < 100001;
DROP TABLE uid_lift;
CREATE TABLE presets (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    id          INTEGER NOT NULL,
    pos         INTEGER NOT NULL,
    name        TEXT NOT NULL,
    PRIMARY KEY (account_id, id)
) WITHOUT ROWID;

CREATE TABLE preset_slots (
    account_id  INTEGER NOT NULL,
    preset_id   INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    uid         INTEGER NOT NULL,
    rounds      INTEGER,
    PRIMARY KEY (account_id, preset_id, slot),
    FOREIGN KEY (account_id, preset_id) REFERENCES presets(account_id, id) ON DELETE CASCADE
) WITHOUT ROWID;
";

/// v5 -> v6 (store): premium account expiry per account (unix seconds; a row exists only while it is set).
pub const SCHEMA_V6_UP: &str = "
CREATE TABLE premium (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    until       INTEGER NOT NULL
);
";

/// v8 -> v9 (store): boosters per account. `enabled` = the player switched the booster on (45 0C); `until` = unix
/// seconds a time booster (the 24 h vouchers) runs, 0 for the per-match boosters; `autobuy` = 48 29.
pub const SCHEMA_V9_UP: &str = "
-- the in-game nickname; NULL = the login name (the rename voucher changes only this)
ALTER TABLE accounts ADD COLUMN nick TEXT COLLATE NOCASE;

CREATE TABLE boosters (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    dict_id     INTEGER NOT NULL,
    enabled     INTEGER NOT NULL,
    until       INTEGER NOT NULL,
    autobuy     INTEGER NOT NULL,
    PRIMARY KEY (account_id, dict_id)
) WITHOUT ROWID;
";

/// v9 -> v10 (ranked leagues): the position in the running season with the league token balance (`league`: one row per
/// account that ever played ranked or earned tokens; `league` 0 = no ranked match in `season` yet) and the finished
/// seasons (`league_history`; `notified` = the season-final message was sent).
pub const SCHEMA_V10_UP: &str = "
CREATE TABLE league (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    season      INTEGER NOT NULL,
    league      INTEGER NOT NULL,
    points      INTEGER NOT NULL,
    max_league  INTEGER NOT NULL,
    matches     INTEGER NOT NULL,
    tokens      INTEGER NOT NULL
);
CREATE INDEX league_ladder ON league(season, league DESC, points DESC);

CREATE TABLE league_history (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    season      INTEGER NOT NULL,
    league      INTEGER NOT NULL,
    max_league  INTEGER NOT NULL,
    tokens      INTEGER NOT NULL,
    closed_at   INTEGER NOT NULL,
    notified    INTEGER NOT NULL,
    PRIMARY KEY (account_id, season)
) WITHOUT ROWID;
";

/// v11 -> v12 (progression: decals and one-time rewards). Written to be idempotent (IF NOT EXISTS) so a merge that
/// renumbers the steps stays safe.
///
/// * `item_decals`: camouflage / decal entries of an armour piece (`slot` = the u8 of 45 01 / 02 / 04, `decal_uid` = the
///   inventory item that is attached, `dict_id` = its dictionary id, `x_bits` = the client's f64 as raw bits).
/// * `claims`: one-time rewards an account already got (`tutorial_completed`, `game_world_tutorial_completed`,
///   `played_first_game`).
pub const SCHEMA_V12_UP: &str = "
CREATE TABLE IF NOT EXISTS item_decals (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    item_uid    INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    decal_uid   INTEGER NOT NULL,
    dict_id     INTEGER NOT NULL,
    x_bits      INTEGER NOT NULL,
    PRIMARY KEY (account_id, item_uid, slot)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS claims (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    claimed_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, name)
) WITHOUT ROWID;
";

/// v13 -> v14 (PvE missions, docs/notes/pve.md section 6). Idempotent (IF NOT EXISTS) so a merge that renumbers the steps
/// stays safe.
///
/// * `pve_results`: one row per finished run (`match_id` unique, so a match end reported twice stores once); `squad_key` =
///   the sorted account ids of the squad (the ladder keeps the best run per key).
/// * `pve_result_players`: the squad of a run with the numbers of the `99` screen.
/// * `pve_rewards`: the chests of a run per player (closed chest dict, content tier / difficulty, opened, rolled item,
///   reroll count, paid) so that `5F 05` / `5F 06` are idempotent.
/// * `pve_daily_wins`: wins per account and day (days since the epoch): the free daily keys.
pub const SCHEMA_V14_UP: &str = "
CREATE TABLE IF NOT EXISTS pve_results (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    match_id      INTEGER NOT NULL UNIQUE,
    mission       INTEGER NOT NULL,
    difficulty    INTEGER NOT NULL,
    success       INTEGER NOT NULL,
    time_secs     INTEGER NOT NULL,
    chests_found  INTEGER NOT NULL,
    chests_total  INTEGER NOT NULL,
    started_at    INTEGER NOT NULL,
    squad_key     TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS pve_results_ladder ON pve_results (mission, success, squad_key);

CREATE TABLE IF NOT EXISTS pve_result_players (
    result_id   INTEGER NOT NULL REFERENCES pve_results(id) ON DELETE CASCADE,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slot        INTEGER NOT NULL,
    score       INTEGER NOT NULL,
    kills       INTEGER NOT NULL,
    deaths      INTEGER NOT NULL,
    sort        INTEGER NOT NULL,
    premium     INTEGER NOT NULL,
    PRIMARY KEY (result_id, account_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS pve_result_players_account ON pve_result_players (account_id);

CREATE TABLE IF NOT EXISTS pve_rewards (
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    match_id       INTEGER NOT NULL,
    idx            INTEGER NOT NULL,
    chest_dict     INTEGER NOT NULL,
    tier           INTEGER NOT NULL,
    difficulty     INTEGER NOT NULL,
    opened         INTEGER NOT NULL DEFAULT 0,
    reward_dict    INTEGER NOT NULL DEFAULT 0,
    reward_amount  INTEGER NOT NULL DEFAULT 0,
    rerolls        INTEGER NOT NULL DEFAULT 0,
    paid           INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, match_id, idx)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS pve_daily_wins (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    day         INTEGER NOT NULL,
    wins        INTEGER NOT NULL,
    PRIMARY KEY (account_id, day)
) WITHOUT ROWID;
";

/// v6 -> v7 (quests): the daily quest set of an account and its slots.
pub const SCHEMA_V7_UP: &str = "
CREATE TABLE daily_quests (
    account_id    INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    issued        INTEGER NOT NULL,
    expires       INTEGER NOT NULL,
    free_used     INTEGER NOT NULL,
    chest_dict    INTEGER NOT NULL,
    chest_amount  INTEGER NOT NULL,
    next_uid      INTEGER NOT NULL,
    next_msg      INTEGER NOT NULL
);

CREATE TABLE daily_quest_slots (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slot        INTEGER NOT NULL,
    uid         INTEGER NOT NULL,
    dict_id     INTEGER NOT NULL,
    progress    INTEGER NOT NULL,
    done        INTEGER NOT NULL,
    PRIMARY KEY (account_id, slot)
) WITHOUT ROWID;
";

/// v7 -> v8 (periodic features): the treasure hunt (item search roulette), the faction battle reward progress and the
/// daily login calendar of an account. `hunt.period` = unix second the weekly period started at; `hunt_entries.pos` is
/// the item index of the page; `battle_rows` keeps per faction the battle (start) its numbers belong to.
pub const SCHEMA_V8_UP: &str = "
CREATE TABLE hunt (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    period      INTEGER NOT NULL,
    level       INTEGER NOT NULL,
    page        INTEGER NOT NULL,
    free_used   INTEGER NOT NULL
);

CREATE TABLE hunt_entries (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    pos         INTEGER NOT NULL,
    dict_id     INTEGER NOT NULL,
    found       INTEGER NOT NULL,
    PRIMARY KEY (account_id, pos)
) WITHOUT ROWID;

CREATE TABLE battle_rows (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    faction     INTEGER NOT NULL,
    start       INTEGER NOT NULL,
    reputation  INTEGER NOT NULL,
    received    INTEGER NOT NULL,
    PRIMARY KEY (account_id, faction)
) WITHOUT ROWID;

CREATE TABLE login_calendar (
    account_id     INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    last_day       INTEGER NOT NULL,
    day            INTEGER NOT NULL,
    received_mask  INTEGER NOT NULL
);
";

/// Version 1 tables (a fresh database runs this and then `SCHEMA_V2_UP`).
pub const SCHEMA_V1: &str = "
CREATE TABLE accounts (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
    pass_hash   TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

-- one profile per account; money is u32 on the wire
CREATE TABLE profiles (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    silver      INTEGER NOT NULL,
    gold        INTEGER NOT NULL,
    parts       INTEGER NOT NULL,
    xp          INTEGER NOT NULL,
    faction     INTEGER NOT NULL,
    version     INTEGER NOT NULL,
    next_uid    INTEGER NOT NULL
);

-- pos keeps the list order (the 87 slot table indexes the inventory list)
CREATE TABLE items (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    uid         INTEGER NOT NULL,
    pos         INTEGER NOT NULL,
    dict_id     INTEGER NOT NULL,
    count       INTEGER NOT NULL,
    PRIMARY KEY (account_id, uid)
) WITHOUT ROWID;

-- uid and rounds may each be NULL (rounds are kept per slot even when the slot is empty)
CREATE TABLE slots (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    slot        INTEGER NOT NULL,
    uid         INTEGER,
    rounds      INTEGER,
    PRIMARY KEY (account_id, slot)
) WITHOUT ROWID;

CREATE TABLE item_mods (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    uid         INTEGER NOT NULL,
    pos         INTEGER NOT NULL,
    grp         INTEGER NOT NULL,
    grade       INTEGER NOT NULL,
    PRIMARY KEY (account_id, uid, pos)
) WITHOUT ROWID;

CREATE TABLE attachments (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    weapon_uid  INTEGER NOT NULL,
    slot_id     TEXT NOT NULL,
    module_uid  INTEGER NOT NULL,
    PRIMARY KEY (account_id, weapon_uid, slot_id)
) WITHOUT ROWID;

CREATE TABLE bound (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    module_uid  INTEGER NOT NULL,
    dict_id     INTEGER NOT NULL,
    PRIMARY KEY (account_id, module_uid)
) WITHOUT ROWID;

CREATE TABLE faction_points (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    faction     INTEGER NOT NULL,
    points      INTEGER NOT NULL,
    PRIMARY KEY (account_id, faction)
) WITHOUT ROWID;

-- one row per side: state 1 friends, 2 request sent by account_id, 3 request received by account_id
CREATE TABLE friends (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    other_id    INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    state       INTEGER NOT NULL CHECK (state IN (1, 2, 3)),
    since       INTEGER NOT NULL,
    PRIMARY KEY (account_id, other_id),
    CHECK (account_id <> other_id)
) WITHOUT ROWID;

CREATE TABLE ignores (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    ignored_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    PRIMARY KEY (account_id, ignored_id),
    CHECK (account_id <> ignored_id)
) WITHOUT ROWID;
";

/// v12 -> v13 (hero of the day, bought specifics). Idempotent (IF NOT EXISTS).
/// * `hero_results`: one row per finished ranked match (`grp` = competitive group index, `won` 0 / 1).
/// * `hero_titles`: the top three of each (group, nomination) of a recalculation slot; `since` = slot start, `until` =
///   `since` + the operating period; `value` = score / kill streak / win streak.
/// * `hero_meta`: key `slot` = the last calculated slot.
/// * `specifics_opened`: pocket consumables opened with gold (`46 09`), the bits of the `91` mask.
pub const SCHEMA_V13_UP: &str = "
CREATE TABLE IF NOT EXISTS hero_results (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    at          INTEGER NOT NULL,
    grp         INTEGER NOT NULL,
    score       INTEGER NOT NULL,
    killstreak  INTEGER NOT NULL,
    won         INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS hero_results_at ON hero_results (at);

CREATE TABLE IF NOT EXISTS hero_titles (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    grp         INTEGER NOT NULL,
    nomination  INTEGER NOT NULL,
    place       INTEGER NOT NULL,
    value       INTEGER NOT NULL,
    since       INTEGER NOT NULL,
    until       INTEGER NOT NULL,
    PRIMARY KEY (grp, nomination, since, place)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS hero_titles_account ON hero_titles (account_id);

CREATE TABLE IF NOT EXISTS hero_meta (
    key    TEXT PRIMARY KEY,
    value  INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS specifics_opened (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    dict_id     INTEGER NOT NULL,
    opened_at   INTEGER NOT NULL,
    PRIMARY KEY (account_id, dict_id)
) WITHOUT ROWID;
";

/// v14 -> v15 (equipment profiles and weapon skins). Idempotent (IF NOT EXISTS).
///
/// * `equip_state`: the current profile index (0..2) of an account.
/// * `equip_profiles`: per profile index its name ("" = default), enabled flag and owned flag; written only for accounts
///   whose table differs from the default (profile 0 owned, 1 and 2 not).
/// * `equip_profile_slots`: the equipment of the profiles that are not current (the current one is in `slots`).
/// * `skin_unlocks`: dict ids of the weapon skins (premium variants) the account unlocked.
pub const SCHEMA_V15_UP: &str = "
CREATE TABLE IF NOT EXISTS equip_state (
    account_id  INTEGER PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    current     INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS equip_profiles (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    idx         INTEGER NOT NULL,
    name        TEXT NOT NULL,
    enabled     INTEGER NOT NULL,
    owned       INTEGER NOT NULL,
    PRIMARY KEY (account_id, idx)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS equip_profile_slots (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    idx         INTEGER NOT NULL,
    slot        INTEGER NOT NULL,
    uid         INTEGER,
    rounds      INTEGER,
    PRIMARY KEY (account_id, idx, slot)
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS skin_unlocks (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    skin_dict   INTEGER NOT NULL,
    PRIMARY KEY (account_id, skin_dict)
) WITHOUT ROWID;
";

/// v17 -> v18 (weapon skins keep the real item). Idempotent (IF NOT EXISTS).
/// * `item_skin_base`: a weapon row that wears a skin (its `items.dict_id` is the skin) -> the dict id of the weapon itself.
pub const SCHEMA_V18_UP: &str = "
CREATE TABLE IF NOT EXISTS item_skin_base (
    account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    uid         INTEGER NOT NULL,
    base_dict   INTEGER NOT NULL,
    PRIMARY KEY (account_id, uid)
) WITHOUT ROWID;
";
