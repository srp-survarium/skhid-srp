#!/usr/bin/env bash
# usage: ammo-top-up.sh [--apply] <srp.db>
#
# Once, at the release after revision 58 (the owner, 2026-11-13: "we added new
# type of ammo", "and didn't add 1000 bullets for everyone"): every 0.26g
# account owns at least the 1000 rounds of every ammo a new account is given
# (`tutorial::signup_grant`, scripts/content-edit/new-account.tsv `rounds`).
# An account made before an ammo was added never got it: on the pre-rev58
# copy, the five newest (23mm, 7.62x51 of 20, the two belts, the SCAR-H's AP)
# were missing on about 590 of 799 accounts.
#
# The rule, per account and ammo: fewer than 1000 rounds owned -> topped up to
# 1000, onto the stack it owns, else as a new stack in its storage (the next
# item id); never any taken. Kits, presets and the rest are left as they are.
# Run again, it adds nothing: every account has its 1000 by then.
#
# Without --apply, a dry run: reads the file only (any copy, srp running or
# not) and prints what --apply would give, per ammo and in total. With --apply,
# on the live srp.db: srp stopped (as the release's other one-off scripts:
# right after the swap, srp's start copy of srp.db made, `docker compose stop
# srp26`, run as root on the data volume's srp.db, `docker compose start
# srp26`). It first copies the file to
# backups/srp-<unix>-before-ammo-top-up.db beside it (with the file's owner
# and mode), then changes everything in one transaction.
#
# The ammo: every stack under gameplay/items/weapons/ammo/ of the released
# data of revision 58, 39 of them (the list srp-026g/crates/session/tests/
# starter_kit.rs checks on that data). Ammo added after it needs its row here
# before the script runs again.
set -euo pipefail

apply=0
if [[ ${1-} == --apply ]]; then
  apply=1
  shift
fi
db=${1:?usage: ammo-top-up.sh [--apply] <srp.db>}
[[ -f $db ]] || { echo "no file $db" >&2; exit 1; }
command -v sqlite3 >/dev/null || { echo "sqlite3 is needed" >&2; exit 1; }

# What each account is short of each ammo (temporary tables only), and the
# report.
plan="
CREATE TEMP TABLE ammo (dict_id INTEGER PRIMARY KEY, name TEXT NOT NULL);
INSERT INTO ammo VALUES
  (7, 'ammo_5.45x39_7n6m'), (20, 'ammo_.357m'), (22, 'ammo_12mm_buck'),
  (50, 'ammo_7.62x25'), (51, 'ammo_7.62x51'), (52, 'ammo_7.62x51_ap'),
  (53, 'ammo_9x19p_para'), (70, 'ammo_9x18_makarov'), (71, 'ammo_9x19_7n21'),
  (72, 'ammo_12mm_slug'), (81, 'ammo_7.62x39'), (91, 'ammo_.30-30'),
  (103, 'ammo_9x18_pbm'), (104, 'ammo_7.62x54r_old'), (147, 'ammo_7.62x39_m'),
  (148, 'ammo_7.62x54_7n1'), (217, 'ammo_5.45x39_7n10'), (218, 'ammo_7.62x39_7n23'),
  (300, 'ammo_pab9'), (301, 'ammo_sp5'), (302, 'ammo_sp6'), (322, 'ammo_9x21'),
  (323, 'ammo_45acp'), (331, 'ammo_5.45x39_7n22'), (368, 'ammo_9x19p_7n31'),
  (369, 'ammo_6.5x39'), (370, 'ammo_12.7x55'), (371, 'ammo_12.7x54_pt'),
  (372, 'ammo_12.7x54_vps'), (2201, 'ammo_5.56x45'), (2202, 'ammo_4.6x30'),
  (2203, 'ammo_5.7x28'), (2204, 'ammo_7.62x54_7n13'), (2205, 'ammo_12mm_buck2'),
  (2253, 'ammo_23mm_r'), (2254, 'ammo_7.62x51_20'), (2265, 'ammo_5.56x45_belt'),
  (2266, 'ammo_7.62x51_scar_ap'), (2275, 'ammo_7.62x51_belt');
CREATE TEMP TABLE short AS
  SELECT account_id, dict_id, name, 1000 - owned AS rounds, stack_seq FROM (
    SELECT a.account_id, m.dict_id, m.name,
           COALESCE((SELECT SUM(i.condition_or_stack) FROM g26_items i
                     WHERE i.account_id = a.account_id AND i.dict_id = m.dict_id), 0) AS owned,
           (SELECT MIN(i.seq) FROM g26_items i
             WHERE i.account_id = a.account_id AND i.dict_id = m.dict_id) AS stack_seq
      FROM g26_accounts a CROSS JOIN ammo m)
  WHERE owned < 1000;
SELECT dict_id, name, COUNT(*) || ' accounts', SUM(rounds) || ' rounds'
  FROM short GROUP BY dict_id ORDER BY dict_id;
SELECT COUNT(DISTINCT account_id) || ' of ' || (SELECT COUNT(*) FROM g26_accounts)
       || ' accounts topped up, ' || COALESCE(SUM(rounds), 0) || ' rounds given, '
       || (SELECT COUNT(*) FROM short WHERE stack_seq IS NULL) || ' new stacks'
  FROM short;
"

# The change: a stack owned grows, an ammo not owned is a new stack after the
# account's items, with the next item ids.
change="
UPDATE g26_items SET condition_or_stack = condition_or_stack + (
    SELECT s.rounds FROM short s
     WHERE s.account_id = g26_items.account_id AND s.dict_id = g26_items.dict_id)
 WHERE EXISTS (SELECT 1 FROM short s
                WHERE s.account_id = g26_items.account_id AND s.dict_id = g26_items.dict_id
                  AND s.stack_seq = g26_items.seq);
CREATE TEMP TABLE new_stack AS
  SELECT s.account_id, s.dict_id, s.rounds,
         MAX(a.next_item_id, (SELECT COALESCE(MAX(i.item_id), 0) + 1 FROM g26_items i
                               WHERE i.account_id = s.account_id))
           + ROW_NUMBER() OVER (PARTITION BY s.account_id ORDER BY s.dict_id) - 1 AS item_id,
         (SELECT COALESCE(MAX(i.seq), -1) + 1 FROM g26_items i WHERE i.account_id = s.account_id)
           + ROW_NUMBER() OVER (PARTITION BY s.account_id ORDER BY s.dict_id) - 1 AS seq
    FROM short s JOIN g26_accounts a ON a.account_id = s.account_id
   WHERE s.stack_seq IS NULL;
INSERT INTO g26_items (account_id, item_id, seq, dict_id, condition_or_stack, repair_time,
                       reroll_time, modifications, upgrades)
  SELECT account_id, item_id, seq, dict_id, rounds, 0, 0, '', '' FROM new_stack;
UPDATE g26_accounts SET next_item_id = (
    SELECT MAX(n.item_id) + 1 FROM new_stack n WHERE n.account_id = g26_accounts.account_id)
 WHERE account_id IN (SELECT account_id FROM new_stack);
"

if ((!apply)); then
  echo "dry run (--apply changes the file):"
  sqlite3 -bail -readonly -separator $'\t' "$db" "$plan"
  exit 0
fi

dir=$(dirname "$db")
mkdir -p "$dir/backups"
copy="$dir/backups/srp-$(date +%s)-before-ammo-top-up.db"
# .backup, not VACUUM INTO: that builds the indexes again, and the nickname
# index's collation is srp's own (session::nickname_key), unknown to sqlite3.
sqlite3 -bail "$db" ".backup '$copy'"
chown --reference="$db" "$copy" 2>/dev/null || true
chmod --reference="$db" "$copy" 2>/dev/null || true
echo "copied $db to $copy"
sqlite3 -bail -separator $'\t' "$db" "BEGIN IMMEDIATE; $plan $change COMMIT;"
echo "applied"
