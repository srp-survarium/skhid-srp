# 0.26g progression and economy: what the sources of the time say

Research of 2026-10-12 (web and the local data dumps; no code changed). Scope: the
server-side numbers the 0.26g client does not carry. Each row says which version and date the
source describes. srp is client 0.26g, build 2777, live 16 to 22 January 2015 (0.26a 12 Dec
2014, 0.26i 23 Jan 2015, 0.27a 2 Mar 2015).

Confidence labels: **official** (Vostok Games patch note, news, FAQ or knowledge base),
**wiki of the time** (a survarium.fandom.com revision dated while 0.25 to 0.26 was live),
**later** (describes 0.27 or newer), **screenshot** (the owner's), **data** (the 0.26g client
files), **inferred** (my arithmetic on the above).

## Summary first (interim report, 2026-10-12)

Tags here: **PROVEN** = stated by an official text or by a wiki revision written while 0.26
was live, and not contradicted by the client data; **LIKELY** = derived by arithmetic from
such a source, or stated for a neighbouring version with nothing taking it back; **GUESS** =
my choice among several that fit. `W:<id>` is
`https://survarium.fandom.com/api.php?action=query&prop=revisions&revids=<id>&rvprop=content&format=json`;
`PN:<v>` is `https://survarium.fandom.com/wiki/<v>` (the official patch note as transcribed);
`A:<timestamp>/<url>` is `https://web.archive.org/web/<timestamp>/<url>`.

### (1) The numbers found

| Number | Source, its date, the version it describes | Tag |
|---|---|---|
| Level table: 500 for the first level, +250 each next | W:9456, 2015-02-10, 0.26i | PROVEN |
| Highest level 100 | A:20220526223259/https://support.survarium.com/ru/index.php?/en/Knowledgebase/Article/View/268/49/povyshenie-urovnja-personazha (written 2014-12-15, edited after 0.27); W:9456 lists rewards to 100 | PROVEN |
| Level rewards: 100 gold at 1 and every fifth level, silver 500 (2 to 4) rising 250 each five levels to 5,000, 250 gold at 100; rewards introduced in 0.26g | W:9456; PN:0.26g, 2015-01-16 | PROVEN |
| Score: kill 10, assist 5, stolen battery 35 | W:9456, 0.26i | PROVEN (the screenshot's score fits) |
| Premium +50 % experience, silver, reputation | A:20150224091649/http://survarium.com/ru/page/faq, 0.26i | PROVEN |
| First three wins of a day: double experience and reputation; seven such days give the reward; a missed day does not reset | PN:0.24a (2014-08-07), PN:0.24c (2014-08-21); removed in PN:0.27a | PROVEN |
| Seventh-day reward 50 gold | W:9979, 2015-02-22, 0.26i | PROVEN |
| Reputation goes to the one chosen faction, changeable at will | PN:0.25a (2014-10-10); A:20150206043734/http://survarium.com/ru/news/gruppirovki-razvitie-igroka (2015-01-13) | PROVEN |
| Three quests at once from the faction | PN:0.22a (2014-05-30), unchanged to 0.27a | PROVEN |
| Quest reward depends on the level of the match | PN:0.23h (2014-07-17) | PROVEN (the multiplier: LIKELY `match_levels_dict`) |
| Weapon prices = the client's `base_cost` (AKS-74U 12,920, TOZ-34 2,100 ...) | W:9794, 2015-02-18, 0.26i | PROVEN |
| Owner's table: Lancaster 2,100 and AKS-74U 12,930 belong to no version | W:9794 and the page histories of `MP-27M`, `AKS-74U` | LIKELY (two slips) |
| Ammo price is for a pack of `clip_size` rounds | W:10031, 2015-02-27 | PROVEN |
| Sell-back 25 %, full repair 10 % of the price | client data; PN:0.27a (2015-03-02) changes them to 15 % and 20 % | PROVEN |
| Rank of every item by faction | W:8170, W:8172, W:8175, W:8176 (2015-01-25), W:9794 | PROVEN; exceptions to srp's rule in §3 |
| Ammo needs a faction rank in 0.26 | PN:0.26c (2014-12-17), PN:0.27a | PROVEN |
| 9A-91, VSK-94 sold by the Fringe Settlers | PN:0.25a; W:8175 | PROVEN |
| PPSh-41, Mosin, TT-33 are owned by every new player | W:9600 (2015-02-14), W:10011 (2015-02-24) | PROVEN |
| Bundle prices 1,500 / 2,000 / 2,000 / 300 / 700 / 5,000 / 5,000 gold | W:7866, 2015-01-22, 0.26h | PROVEN |
| 1000 gold = $10 | A:20140209060032/https://shop.survarium.com/en/ | PROVEN |
| Premium 90 days = $25 = 2,500 gold; a stash map 100 gold | A:20150119223217/http://survarium.com/ru/news/akciya-zimniy-uikend (Jan 2015) | LIKELY (arithmetic on an official line) |
| Drop chance: 40 % base, +2.5 % a battery home, +25 % premium | W:9456, 0.26i; base equals the client's `generic_drop` | PROVEN (base), LIKELY (the two bonuses: the screenshot's 90 % does not fit) |
| Dropped gear arrives damaged with a defect | PN:0.24a | LIKELY (two versions earlier, never taken back) |
| Craft chances 50/100, 25/50, 12/25 | W:9975, 2015-02-22; equals `craft.options` | PROVEN |
| Clan registration needs 5 players | client text `st_clan_creation_info_description` | PROVEN |
| Clan price 250,000 silver, 15 members | `https://survarium.fandom.com/ru/wiki/Клан`, 2015-07-29, 0.29 to 0.30 | GUESS for 0.26 (later version) |
| 8 a side, 15 minutes; matchmaking by the highest-level kit; least-played map | W:9147 (2015-02-08); A:20141120183926/http://survarium.com/ru/news/prevyu-obnovleniya-026; PN:0.25a | PROVEN |
| Rating starts near 100 | the screenshot (118 at account level 3); client `elo_min: 100` | LIKELY |

### (2) The fitted formula, by results-screen row

**Superseded on 2026-10-12** by a second screen of the owner's (SHOT2): a kill is 25 / 3 / 1
(with a lesser kind of 20 / 2 / 1, cause unknown), every cell is rounded down, and the
activity row is the line through both screens. What the code pays is in `results.md` §10;
the table below is the first fit, kept for the record.

Before premium (+50 % of these) and the daily bonus (+100 % of experience and reputation).
`k` are the three multipliers of `match_levels_dict` for the match's level; the screenshot's
match is level 1 (`money_k` 1.47), which is what makes its money rows whole numbers.

| Row | Money | Experience | Reputation | Fit | Tag |
|---|---|---|---|---|---|
| Enemies killed | 22 × kills × k | 2.4 × kills × k | 1 × kills × k | 550 / 41 / 17 against 551 / 41 / 17 | LIKELY (reputation exact; 22 and 2.4 are one match's averages, a kill's worth varied) |
| Help ally | 10 × events × k | 1 × events × k | 1 × events × k | 58 / 4 / 4, exact | LIKELY |
| Match activity | (100 finishing + 200 win + 0.5 × other score) × k | (40 finishing + 100 win + 0.1 × other score) × k | 0.15 × other score × k | 465 / 143 / 5 against 463 / 143 / 5 | GUESS (totals fit; the split is mine) |

### (3) Open questions for the owner

1. The activity row's split between finishing, winning and events (a lost match's screen
   would settle it), and what makes one kill worth more than another.
2. Premium prices other than 90 days; the gold to silver rate; the skill reset price; the
   gold price of a nickname change; a new account's starting silver and gold.
3. The 0.26 clan price, clan size by clan level, clan levels and clan battle rules.
4. What a change of faction did to running quests in 0.26, and how often quests were given.
5. The condition and defect a dropped item arrives with; the team's premium drop bonus; the
   TDM drop formula; why the screenshot shows 90 %.
6. New-player protection; the least players to start a match; the matchmaking level range.
7. Whether to sell decals at faction ranks as the wiki lists them. (Camouflages: the
   factions' own are sold at no rank since 2026-10-17, the owner's choice.)
8. The medkit's price: 50 on the wiki, 20 in the client data.

## Sources and how to read them again

| Tag | What | How to fetch |
|---|---|---|
| PN | Patch notes 0.22a to 0.29a as transcribed on the English wiki (each page links the official news; the 0.25a and 0.26a pages name `survarium.com/en/news/survarium-update-025a` / `-026a`) | `https://survarium.fandom.com/wiki/0.26a` etc.; API `api.php?action=query&prop=revisions&titles=<page>&rvprop=timestamp\|ids\|content` |
| W-date | A wiki revision of that date | `api.php?action=query&prop=revisions&revids=<id>&rvprop=content` on `survarium.fandom.com` |
| N | Official Russian news, Wayback Machine | `https://web.archive.org/web/<timestamp>/http://survarium.com/ru/news/<slug>` |
| KB | Official knowledge base `support.survarium.com` (only 2022 snapshots exist; each article prints its date of writing, later edits are possible) | `https://web.archive.org/web/2022052622id_/https://support.survarium.com/ru/index.php?/en/Knowledgebase/Article/View/<id>/...` |
| SHOT | The owner's results-screen screenshot from a video "of that version" (owner's video screenshot, version as stated by owner). I did not see the image: the only file in the session's `images\` folder is `1.png`, the owner's price table. The numbers are the coordinator's reading of it. | |
| TBL | The owner's price table, `images\1.png` | |

WebFetch cannot open `web.archive.org` or `fandom.com` (blocked, HTTP 402); `curl` to stdout
reads both. Nothing was saved from the web.

Wiki revisions used: `Survarium` 9456 (2015-02-10), `Weapons` 9794 (2015-02-18), `Scavengers`
8170, `Black Market` 8172, `Fringe Settlers` 8175, `Renaissance Army` 8176 (all 2015-01-25),
`Bundles` 7866 (2015-01-22), `Upgrades` 9975 (2015-02-22), `Items` 9996 (2015-02-22),
`Ammunition` 10031 (2015-02-27), `Shop` 9979 (2015-02-22), `Teamplay` 9147 (2015-02-08),
`Skills` 8303 (2015-01-27). All are from 0.26g to 0.26i, before 0.27a.

## 1. Levels and experience

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Experience for a level | 500 for the first, 250 more for each next ("1: 500, 2: 750, 3: 1000 ...", listed to 100) | 0.26i, W-2015-02-10 (`Survarium` 9456, "Required Experience") | wiki | wiki of the time | the same (`account.rs` `level_at`, `v021d::results::experience_for_level`) | keep |
| The bar restarts at each level | "the experience is reset. It should again gather experience from zero" | same | wiki | wiki of the time | `level_at` returns start and next | keep |
| Same table after 0.27a | 1000 for the first, 250 more each ("второго уровня ... ещё 1250 ... для пятого – уже 2000") | KB 268, written 2014-12-15, text edited after 0.27 (it names point capture) | official | later | - | not applied |
| Highest level | 100 ("Максимально возможный уровень персонажа – сотый"); the wiki's reward table of Feb 2015 also ends at 100 | KB 268; W-2015-02-10 | official + wiki | official (the wiki row dates it to 0.26) | `account.rs` `MAX_LEVEL = 50` | **100** |
| Skill points | one a level | W-2015-02-10; KB 268 | wiki, official | official | level = points | keep |
| Level rewards exist from 0.26g | "Added rewards for reaching certain character levels (all players will get the reward according to their current level)" | PN 0.26g, 16 Jan 2015 | official | official | paid (`notices.rs`, `v021d::results::level_reward`) | keep |
| Level reward amounts | 100 gold at level 1 and every fifth; silver 500 at 2 to 4, 750 at 6 to 9, +250 each five levels, 5000 at 91 to 99; 250 gold at 100 | W-2015-02-10 | wiki | wiki of the time | the same but no 250 at 100 (the level could not be reached) | add 250 gold at 100 with the new cap |
| Score for a kill / assist / stolen battery | 10 / 5 / 35 ("Find a battery and take it to your base": not filled in) | W-2015-02-10 "Activity Rewards" | wiki | wiki of the time | `static_info.rs` `SCORE_WEIGHTS`: 100 / 50 / 100 (ten times the scale) | kill 10, assist 5, steal 35 (see §2 for the check against SHOT) |
| Experience for a kill, earlier | "reduced from 10 to 3"; money "from 50 to 15 rubles" | PN 0.22a, 30 May 2014 | official | official, older version | - | context only |
| Kill money and reputation raised | "Increased the amount of money and reputation earned for killing opponents" | PN 0.24b, 9 Aug 2014 | official | official, no number | - | - |
| Experience depends on | "убийства противников, помощи союзникам, поиска батарей" | KB 268 | official | official, no number | - | - |

## 2. Match rewards: the one results screen we have

SHOT (player roxius2, account level 3, premium, Scavengers "Rookie 230 / 320", victory 5:4,
8 against 8, score 223, 17 kills, 9 deaths, rating 118 (+12), "Item drop chance 90%"):

| Row | Money | Experience | Reputation |
|---|---|---|---|
| Enemies killed (x18) | 551 | 41 | 17 |
| Help ally (4) | 58 | 4 | 4 |
| Match Activity Rewards | 463 | 143 | 5 |
| Daily bonus | - | 188 (+100%) | 26 (+100%) |
| Premium account +50% | 536 | 94 | 13 |
| Total | 1608 | 470 | 65 |

What it settles (inferred, exact):

- base = the three rows = 1072 / 188 / 26; the daily bonus is +100 % of the base experience
  and reputation and nothing on money; premium is +50 % of the base of all three, not of the
  daily bonus; total = base + daily + premium. srp's rules agree with this (`bonus.rs`,
  `rewards.rs`), only the base amounts differ.
- "Rookie 230 / 320" = Scavengers level 1 between the data's 100 and 420. The thresholds of
  `factions_dict` are the real ones.
- A rating of 118 on a level-3 account: the rating starts near 100 (the data's
  `elo_modifiers` begin at `elo_min: 100`), not at srp's 1000 (`v021d::rating::START`).

The fit (inferred):

- A level-3 player in Rookie gear plays match level 1, where `match_levels_dict` has
  `money_k 1.47`, `experience_k 1`, `reputation_k 1`. Dividing the money by 1.47 gives whole
  numbers in all three rows: help 58 → 40 (39.5 to 40.1), kills 551 → 375 (374.8 to 375.5),
  activity 463 → 315 (463.05 exactly). With `money_k` 2.0 or 2.56 (levels 2, 3) the help row
  is not a multiple of 4. So `match_levels_dict` is the reward multiplier by match level, and
  the row is rounded after multiplying.
- **Help ally**: 4 events = 40 silver, 4 experience, 4 reputation: **10 silver, 1 experience,
  1 reputation each**, exact.
- **Enemies killed**: 375 silver, 41 experience, 17 reputation over "x18". The scoreboard has
  17 kills; the row counts events 0 and 4 (`victory_screen.options`), so the 18th is LIKELY a
  team kill that paid nothing. Per kill: reputation exactly 1; silver 22.06; experience 2.41.
  No single whole pair fits: 17 × (22, 2.4) gives 550 / 41 / 17 against 551 / 41 / 17 (silver
  −0.2 %). "20 silver and 2 experience a kill, and 7 kills with +5 silver and +1 experience"
  fits exactly, and so does "13 kills of 20 / 2 and 5 of 23 / 3 over 18", so a kill's worth
  varied with something the row does not show (the victim's level or rating, GUESS).
- **Match activity**: 315 silver, 143 experience, 5 reputation for: a win 5:4, the whole
  match played, and about 33 points of other score (223 − 17 × 10 − 4 × 5, with the wiki's
  weights; this difference being small and positive is the check that the wiki's 10 and 5
  are the 0.26 score weights). One screen cannot split it into win, participation and events.
- The drop chance of 90 %: the wiki's formula of the same weeks (§7) gives 40 + 5 × 2.5 + 25 =
  77.5 % for a premium player with 5 batteries home. 90 needs 5 % a battery, or 12.5 more from
  something else. Not resolved.

Against srp today (`rewards.rs`: experience = score / 10 + 10 + 20, × `SRP_EXPERIENCE_RATE` 3;
money = 1.5 × experience before the rate; reputation = experience): about 157 experience,
78 money, 157 reputation for this match, against 188 / 1072 / 26. Money is about 14 times too
low, reputation 6 times too high.

**Recommended formula**, by the screen's three rows, all before premium and the daily bonus,
each row rounded after its multiplier:

| Row | Money | Experience | Reputation | Fit to SHOT | Status |
|---|---|---|---|---|---|
| Enemies killed | 22 × kills × `money_k` | 2.4 × kills × `experience_k` | 1 × kills × `reputation_k` | 550 / 41 / 17 against 551 / 41 / 17 | the reputation is exact; 22 and 2.4 are averages of one match |
| Help ally | 10 × events × `money_k` | 1 × events × `experience_k` | 1 × events × `reputation_k` | 58 / 4 / 4, exact (59 if rounded to nearest; the screen floors) | exact for assists; cure and rescue assumed the same |
| Match activity | (100 for finishing + 200 for a win + 0.5 × other score) × `money_k` | (40 for finishing + 100 for a win + 0.1 × other score) × `experience_k` | (0.15 × other score) × `reputation_k` | 316.5 → 465 / 143 / 5 against 463 / 143 / 5 | **a guess that reproduces one winning match**: the split between finishing, winning and the events is mine. A lost match's screen is needed to fix it |

"Other score" is the score less 10 a kill and 5 an assist. With these, `SRP_EXPERIENCE_RATE`
should default to 1: the screen's numbers are already the real pace (about 190 experience a
won match against 500 for the first level). No other results screen of 0.25 to 0.27 was found
(searched in Russian and English for the row names).

### A second results screen (owner, 2026-10-12): replaces the fit above

SHOT2: Russian client, LIKELY 0.27 (mode selector, an "equipment bonus" row, no daily bonus
row). Level 49, premium, win 8:2, score 289, 19 kills, 9 deaths.

| Row | Money | Experience | Reputation |
|---|---|---|---|
| Enemies killed (19) | 4830 | 96 | 32 |
| Help ally (8) | 813 | 13 | 13 |
| Match activity | 5064 | 270 | 11 |
| Premium +50 % | 5353 | 189 | 28 |
| Total | 9784 | 568 | 84 |
| Expenses | −6276 (repair −3409, ammo −1927, special −940) | | |

- The rows are `match_levels_dict` level 15 (`money_k` 10.17, the other two 1.7), floored
  after multiplying: help 8 × 10 × 10.17 = 813, 8 × 1.7 = 13. PROVEN: the multiplier model and
  help = 10 / 1 / 1.
- A kill is **25 silver, 3 experience, 1 reputation**: 19 × 25 × 10.17 = 4830, 19 × 3 × 1.7 =
  96, 19 × 1.7 = 32. PROVEN. SHOT's 375 / 41 over 17 kills is exactly 7 kills of 25 / 3 and 10
  of 20 / 2: a lesser kind of kill exists, cause unknown.
- Activity before the multiplier: 498 / 159 / 7 at other score 59, against SHOT's 315 / 143 / 5
  at 33. The line through both: money 83 + 7.04 s, experience 122.7 + 0.615 s, reputation
  2.46 + 0.077 s. GUESS (two wins; what a lost match pays is still unknown).
- The money total is after expenses: 4830 + 813 + 5064 + 5353 − 6276 = 9784.

Other match-reward facts:

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Premium | +50 % experience, income, reputation ("опыт, доход и репутация будут накапливаться быстрее на 50%") | FAQ page, snapshot 2015-02-24 (`/ru/page/faq`); KB 170 | official | official | +50 % (`rewards.rs`) | keep |
| First three wins of a day | "the first three victories in a day will give you twice the experience and reputation" | PN 0.24a, 7 Aug 2014; removed in 0.27a ("Bonus for achieving first three wins in a day removed") | official | official | the same (`bonus.rs` `WINS_A_DAY = 3`) | keep |
| The seven days | "the reward for three wins in a series of seven days is not reset if you missed a day or were not able to win three times" | PN 0.24c, 21 Aug 2014 | official | official | a missed day starts a new run; a day counts when the account is seen (`bonus.rs`) | **a day counts when its three wins are made; a missed day does not reset; the reward comes with the seventh such day** |
| The seventh day's reward | "may not only be premium currency, but also rare decals, camo, premium account" (0.24c); "they will receive 50 gold for that week" (wiki) | PN 0.24c; W-2015-02-22 `Shop` | official / wiki | wiki of the time for 0.26 | 50 gold (the data's `daily_bonus_drop`) | keep |
| Reward depends on the final score | "размер награды за бой зависит не только от ваших личных показателей, но и от того, с каким счетом закончится бой" | `/ru/page/komandnaya-igra-pvp`, snapshot 2015-02-24 | official | official, no number | win bonus only | - |
| Costs taken on the screen | repair, ammunition, special equipment ("на починку каких предметов и на пополнение каких патронов тратятся средства") | N `prevyu-obnovleniya-026`, 14 Nov 2014; SHOT "Consumption" | official | official | autobuy and auto-repair (`autobuy.rs`) | keep |
| New-player protection | nothing found | - | - | - | - | - |

## 3. Reputation and the shop's ranks

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Which faction gets it | "Reputation for actions in battle is now added to the faction with which a player cooperates"; "Одновременно игрок может улучшать репутацию только с одной группировкой, но ничто не мешает ему менять активную группировку сколько угодно раз" | PN 0.25a; N `gruppirovki-razvitie-igroka`, 13 Jan 2015 | official | official | the chosen faction (`quests.rs`, `rewards.rs`) | keep |
| Where most of it comes from | "Основной вклад в прокачку репутации вносит выполнение заданий от группировок" | same news | official | official | reputation = experience, far more than quests give | the §2 formula (26 a won match against 100+ a quest) |
| Thresholds | Scavengers level 1 = 100, level 2 = 420 | SHOT | screenshot | matches the data | `factions_dict` | keep |
| Renaissance and Edge open after | "only ... if they achieve maximum reputation with one of the basic factions" | W-2015-01-26 `Factions` (text from 2013) | wiki | weak | `rules.rs` `faction_unlocked` | keep as is |
| Rank of each item | the four faction pages' "Equipment for sale" lists and the `Weapons` table's rank column | 0.26i, W-2015-01-25 and W-2015-02-18 | wiki | wiki of the time | `prices.rs` `seller_level`: item level (Scavengers, Black Market), item level − 5 (Renaissance, Edge) | keep the rule, with the exceptions below |
| The lists against the rule | every weapon and armour piece agrees except the rows below | same | inferred | - | - | - |
| Chemical protection set and the Amber respirator | rank 2 "Determined" (the data's item level is 3; 0.27a then "moved to equipment tier 3 and now requires Experienced") | W-2015-01-25; PN 0.27a | wiki + official | wiki of the time | 3 | **2** for `scav_torso_2`, `scav_legs_2`, `scav_gloves_2`, `scav_boots_2`, `scav_resp_1` |
| Fort 17, APS | rank 2 (Lance Corporal, Huntsman); the editor's note: "Moved Fort 17 and 9x18 PBM to correct rank" | W-2015-01-24/25 | wiki | wiki of the time | 1 (item level 6) | **2** |
| "Marodeur" container (`bm_back_1`) | Black Market rank 4; the data gives it faction 1 and item level 3 | W-2015-01-25 | wiki | wiki of the time | Scavengers, 3 | **Black Market, 4** |
| 9A-91 and VSK-94 | sold by the Fringe Settlers at ranks 8 and 9 ("Fringe Settlers ... Added new weapons: H&K UMP45, 9A-91, VSK-94"); both configs say faction 3 | PN 0.25a; W-2015-01-25; W-2015-02-18 | official + wiki | official | Renaissance (the config's faction) | **seller Edge** for dict ids 303 and 305 |
| Ammo needs a rank | "7.62x54 7N1 is on sale at the Renaissance Army store on the 7th level of reputation (Captain)"; 0.27a: "Purchase of ammunition no longer requires reputation" | PN 0.26c; PN 0.27a | official | official | every ammo under seller 0, rank 0 (`prices.rs`) | **sell ammo by the factions at a rank**, table below; since 2026-10-13 by the same factions **at no rank** (the owner's choice, as 0.27a: `prices.rs` `AMMO_NEEDS_RANK`) |
| Consumables and grenades by faction | bandages Scavengers 1; medkit, antitoxin, RGD-5 Scavengers 3; trap Black Market 1; anesthetic, RGD-5 Black Market 3; RDG-M smoke Renaissance 1 and Edge 1; boosters at the "Independent Traders" | W-2015-01-25; W-2015-02-22 `Items` | wiki | wiki of the time | all under seller 0 | as listed; boosters stay under seller 0 |
| Cosmetics at ranks | camouflages and decals in the lists ("Quilted fabric", "Scavengers rookie" at rank 1, "Seeker" 3, "Ghost" and "Scavengers veteran" 5 ...) | W-2015-01-25 | wiki | wiki of the time | since 2026-10-17 the factions' own camouflages (`stk_camo`) by the faction of their folder at **no rank** (the owner's choice; `data.md`, "Settled in step 4"); decals not sold | the decals' names need matching to dict ids |
| Notice at a new rank | "In the notification window when reaching a faction level we have added a list of unlocked items" | PN 0.26g | official | official | `#ru` notice | nothing to do (the client builds it) |

Ammo by seller and rank (W-2015-01-25; where a faction is not named the ammo was not on its
list):

| Ammo | Scavengers | Black Market | Renaissance | Edge |
|---|---|---|---|---|
| 12x70 buckshot | 1 | 1 | | |
| 12x70 slug | 3 | 3 | | |
| 9x18 MP | 2 | | | |
| 9x19 Parabellum | | 2 | | |
| 7.62x51 | 3 | | | |
| 7.62x51 M59 | 5 | | | |
| .30-30, .357 | | 3 | | |
| 5.45x39 7N6M | | 5 | | |
| 7.62x39 | | | 1 | 1 |
| 9x18 PBM | | | 2 | 2 |
| 9x21 SP-10 | | | 3 | |
| .45 ACP | | | | 3 |
| 7.62x39 7N23 | | | 4 | 4 |
| 7.62x39 RPD | | | 4 | |
| 7.62x54 7N1 | | | 7 (official) | 4 |
| 9x19 7N31 | | | | 4 |
| 5.45x39 7N10 | | | 5 | 5 |
| 9x39 PAB-9 | | | 8 | 8 |
| 9x39 SP-5, SP-6, 5.45x39 7N22 | | | 9 | 9 (no 7N22) |
| 6.5x39 Grendel | | | 10 | |
| 12.7x55, 12.7x54 | | | | 10 |

7.62x25 and 7.62x54R cost nothing; 9x19 7N21 is on no list.

## 4. Faction quests in 0.26

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| One faction at a time | "Now the player can only accept tasks from one faction" | PN 0.22a | official | official | yes | keep |
| How many at once | "Number of tasks simultaneously given by a faction increased to 3" | PN 0.22a; unchanged until 0.27a | official | official | `quests.rs` `QUESTS_AT_ONCE = 3` | keep (now sourced) |
| Reward by match level | "The quest reward now depends on the level of match, in which the quest was completed" | PN 0.23h, 17 Jul 2014 | official | official | the quest's data award, unscaled | **multiply the quest's money by `money_k` and its reputation by `reputation_k` of the match it was done in** (the multiplier table is LIKELY the same one) |
| Doubled in bonus matches | "Fixed bug in calculation of awards for quests completed in "x2" matches" | PN 0.25a | official | official | not doubled | double a quest's reputation (and experience) in a first-three-wins match |
| Gear needed | 0.27b: "The minimum level required for a mission increased by 1. So now must use equipment of the current or previous level" | PN 0.27b | official | later | - | not applied |
| Three a day, bronze / silver / gold; what a change of faction does | "Группировкой будет выдаваться только три ежедневных задания, один раз в сутки"; unfinished quests are replaced on a change | N `gruppirovki-razvitie-igroka`, N `prevyu-obnovleniya-027`; PN 0.27a | official | later (0.27) | drop the old faction's quests unpaid | not applied; what 0.26 did on a change is not stated anywhere |
| Loyalty | removed in 0.24a | PN 0.24a | official | official | none | keep |
| Time limits, how often new ones come | nothing beyond the data's `ttl_minutes` 1440 and cooldown 120 | - | - | - | data | - |
| What a goal counts | the client's own counter of each quest type (bring = a battery found and carried alone, event 24; steal = event 26; K/D with no death = the kills); the battery goals are listed only in the battery modes | 0.26g | client code: `refresh_quest_ui_data` 0x2A2FD0, `set_player_quests` 0x2A35E0 | PROVEN | followed since 2026-10-14 (the table in `crates/survarium/src/v026g/quests.rs`); that a goal needs the battle won, and "at least" for K/D, are from the objective texts | keep |

## 5. Shop

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Weapon prices | the wiki's `Weapons` table equals `base_cost` for every weapon sold (TOZ-34 2,100, TOZ-66 1,900, AKS-74U 12,920, AKM 20,900, Remington 700 21,420, AK-74M 47,500, VSSK 165,000 ...) | 0.26i, W-2015-02-18 | wiki | wiki of the time | `base_cost` (`prices.rs`) | keep: `base_cost` is the 0.26 price |
| TBL against the data | 18 of 20 rows equal `base_cost`. AKS-74U 12,930: the wiki and the data both say 12,920 for 0.26 (16,000 on 7 Mar 2015, 60,000 in 0.28), so 12,930 is a slip. MP-27M Lancaster 2,100: the data says 2,500; the wiki never gives it a silver price in 0.26 ("*", "0 rubles, 700 gold", from 0.27 "0 (Drop)", 5,250 in April 2015); 2,100 is the TOZ-34's price two rows above | 0.26 | TBL, wiki | inferred | - | the table is the 0.26 list with two slips; no version had those two numbers |
| Lancaster, Vityaz, VSS, SR-3M, AK-74N | not sold for silver: bundles only ("Independent traders" in PN 0.26a); from 0.27 also drops | W-2015-02-18 | wiki | wiki of the time | not listed | keep |
| SOCOM 16 | "доступная только в качестве награды за бой" | N `prevyu-obnovleniya-026`; PN 0.26a | official | official | not listed, can drop | keep |
| Ammo price and pack | the wiki's cost is for a pack of `clip_size` rounds (.357: 20 for 50, 9x18: 50 for 20, .45: 50 for 65) | W-2015-02-27 `Ammunition` | wiki | wiki of the time | per clip | keep |
| Medkit | 50 on the wiki, 20 in the data; every other consumable agrees (bandages 10, anesthetic 50, antitoxin 50, RGD-5 30, RDG-M 50, trap 20, boosters 1,500) | W-2015-02-22 `Items` | wiki | one disagreement | 20 | keep 20 (one wiki cell against the data) |
| Selling | a quarter; 0.27a: "Equipment sale cost in the shop is reduced to 15%" | data (client `cost / 4`); PN 0.27a | official | official | a quarter | keep |
| Selling a modified item | +25 % / +50 % / +100 % for 1 / 2 / 3 modifiers; starter and drop-only items sell for nothing | KB 317, 5 Mar 2015 | official | later | plain quarter | not applied |
| Repair | 10 % of the price for a full repair; 0.27a: "Repair cost increased to 20% of the base value" | data `repair_cost_ratio 0.1`; PN 0.27a | official | official | 0.1 | keep |
| The free weapons | "one of the starting weapons available to all players when they first enter the world, along with the TT-33 and Mosin carbine"; KB: "стартовые предметы, которые даются при первом заходе в игру (они имеют нулевой уровень)" | W-2015-02-14 `PPSH-41`, W-2015-02-24 `Mosin carbine`; KB 317 | wiki + official | wiki of the time | a new account has the TT-33 and a clip (`loadouts.rs` `STARTER`) | **a new account owns the PPSh-41, the Mosin, the TT-33 and their free ammo** |
| Breakage by shots | 0.26: by shots and matches; 0.27c: "Removed the dependency of the breakdown of weapons from the number of shots" | PN 0.27c | official | official for 0.26 by contrast | `degrade_per_use` and `degrade_per_match` | keep |
| Bought ammo is a store, not a per-match charge | 0.27a ended it: "There is no concept of "storage" ... The cost of emptied cartridges is automatically removed after a battle" | PN 0.27a | official | official for 0.26 by contrast | stacks used up, autobuy | keep |

## 6. Premium and gold

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Gold's worth | 1000 gold = $10 ("1000 Golden Rubles (equivalent of $10 USD)") | shop.survarium.com, snapshot 2014-02-09 | official | official | - | - |
| Bundle prices | AK-74N 1,500; Bandit 2,000; Survivor 2,000; Lancaster 300; Vityaz 700; SR-3M Vikhr 5,000; VSS Vintorez 5,000 gold | 0.26g to 0.26i, W-2015-01-22 `Bundles` | wiki | wiki of the time | `shop_offers.rs`: 1,200 / 1,500 / 1,500 / 500 / 1,000 / 2,500 / 2,500 | **the wiki's seven prices** |
| Bundle contents | as srp has them; the wiki also gives each premium weapon's fixed modifiers (+10 % silver, one weapon modifier, +25 % reliability) | same | wiki | wiki of the time | the `*_premium` copies | keep |
| Three months of premium | $25 ("Премиум-аккаунт сроком на три месяца предоставляется со скидкой 40%! Сэкономьте 10$!") = 2,500 gold | N `akciya-zimniy-uikend`, Jan 2015 | official | inferred from an official line | `SRP_PREMIUM_PRICES` 7,500 for 90 days (this row said 3,000, which is the 30 days' price; corrected 2026-10-12) | **2,500** |
| Stash map (drop card) | 10 for $10 before the discount ("набор, из 10 карт схрона со скидкой 25% вы экономите 2,5$") = 100 gold each | same | official | inferred | `base_cost` 100 | keep (confirms `base_cost` of vouchers is gold) |
| Other premium durations | "от 24 часов до 1 года"; no prices found | KB 170 | official | - | 250 / 600 / 1,200 / 3,000 / 7,500 / 13,500 / 24,000 | not found; only the 90-day price is sourced |
| Gold to silver | exists since 0.23a ("The possibility to convert premium currency into game rubles added"); the rate is nowhere | PN 0.23a; KB 244 | official | no number | `rules.rs` `PREMIUM_EXCHANGE_RATE = 100` | not found |
| Skill reset price | not found | - | - | - | 100 silver or 1 gold | not found |
| Nickname change | by a "Бланк для смены имени" or for gold; no price | KB 163 | official | no number | 250 gold (the voucher's `base_cost`) | keep |
| Third kit | "Added the ability to purchase premium from the locked (3rd) equipment profile" | PN 0.26g | official | official | every account's, premium or not, since 2026-10-17 (`account.rs` `kit_usable`) | the owner's choice over the source |
| Premium compensation | 12 hours given for an outage | N `kompensaciya-premiuma`, Nov 2014 | official | - | - | - |
| Promo code of the time | "1 неделю премиума и 5000 внутриигровой валюты" | itc.ua, 25 Jan 2015 | press | - | - | - |
| Starting silver and gold of a new account | not found | - | - | - | 2,000 and 1,000 | not found |

## 7. Drops, modifiers, craft

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Base chance | 40 % ("Your drop rate has a base of 40%"); the data's `generic_drop` gives category 0 a weight of 60 in 100 | W-2015-02-10; data | wiki + data | wiki of the time | 40 % (`drops.rs`, category 0 = nothing) | keep; the reading of category 0 is now confirmed |
| Match result | "+2.5%" for each battery in the base at the end | W-2015-02-10 | wiki | wiki of the time | 0 (the screen's second part is sent as 0) | **add 2.5 % a battery of the own team, shown as the "match result" part**; SHOT's 90 % does not fit it exactly (§2) |
| Premium | "+25%" | W-2015-02-10; PN 0.25a: "the result of a match and premium account increase the chance of finding items" | wiki + official | wiki of the time | 0 | **+25 % for a premium account, in the screen's premium part** (the client's label test at 12.5 separates the owner's 25 from a smaller team bonus) |
| Premium for the team | 0.23a: "by 50% for its owner and by 20% for his teammates" (then for quality) | PN 0.23a | official | older; the 0.26 team number is unknown | 0 | not applied |
| No drop exists | "Now there's a chance of a zero drop after a match" | PN 0.25a | official | official | yes | keep |
| What drops | weapons and equipment +50 %; "Removed ammunition and special equipment from the list of items dropped after a match"; gold, silver and a one-day premium ticket since 0.23a; stash map: "Only weapons, items, camouflage and decals" | PN 0.25a, 0.23a | official | official | the data's tables (which already have no ammo group) | keep |
| The screen shows the real chance | "match results window display a real chance of finding items" | PN 0.25b, 0.26c | official | official | chance sent | keep |
| Dropped gear is damaged | "Now all the items found after a match will be in poor condition with critical malfunctions" | PN 0.24a; no later note takes it back (0.24b to 0.26i read) | official | official, two versions earlier | full condition, no defects (`rewards.rs`, `items_rules.rs`) | **dropped weapons and armour arrive worn and with a defect**; the condition value is not given anywhere (suggest under the 0.5 defect threshold) |
| Higher levels drop less | "Reduced the probability of finding items after a match the level of which is higher than your current level of battle" | PN 0.24a | official | official | the level table | keep |
| Rarity | white none, blue 1, green 2, gold 3 modifiers | PN 0.23e; W-2015-02-22 `Upgrades` | official | official | rarity n = n modifiers | keep |
| Craft chances | same rarity 50 % / 100 % (silver / gold), failure 25 % / 0; one below 25 / 50, failure 25 / 0; two below 12 / 25, failure 67 / 25 | W-2015-02-22 | wiki | equals `craft.options` | data | keep |
| Craft needs an identical donor; price by level | "upgrade an item using an identical item as a donor"; "Prices for equipment upgrade now depends on the level of equipment" | PN 0.25a, 0.26a | official | official | same item as donor, `craft_price_*` by level | keep (the LIKELY in `items_rules.rs` is now sourced) |
| Reroll | one modifier only, three offered, silver once in 6 hours, gold any time; "С каждой последующей попыткой ... стоимость операции возрастает" | W-2015-02-22; KB 243 (later) | wiki + official | wiki of the time; the rising price is later | as described, price × 1.1 a change | keep |
| Default modifiers | cannot be changed or doubled | W-2015-02-22 | wiki | wiki of the time | - | check `items_rules.rs` refuses a reroll of a default modification |
| TDM drop chance | "Revised the formula for calculating the chance of finding an item during Team Deathmatch" | PN 0.27a | official | later | - | not applied; the 0.26 TDM formula is unknown (the data's `game_mode_params` 0.6 / 0.2 is likely it) |

## 8. Clans

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Introduced | "Player clans were implemented ... The number of players in the clan depends on the level of clan ... clan battle, where the selection of opponents depends on clan rating" | PN 0.25a | official | official | levels exist, one size (`clans.rs`) | size by level: numbers not found |
| Players to register | 5 ("enlist at least 5 players") | data, `st_clan_creation_info_description` | data | official | `players_required: 1` (a test default) | **5** on the live server (`SRP_CLAN_PLAYERS_REQUIRED=5`) |
| Price | "250 тысяч серебра ... все кто примут ... смогут помочь вам собрать" | Russian wiki `Клан`, 29 Jul 2015 (0.29 to 0.30) | wiki | later | 1,000 | not applied (the only number found; the preview of Nov 2014 says only "определенное количество серебра") |
| Most members | 15 | same | wiki | later | 100 | not applied |
| Default ranks | commander (all five rights), deputy (all but kick), sergeant (change rank, field commander), soldier (none) | same | wiki | later | commander, assistant (all), warlord (invite, field commander), soldier | not applied |
| The commander adds ranks and hands over by "promote" | as srp does | KB 220, 19 Nov 2014 | official | official | yes | keep |
| Clan battle mode | random mode ("В клановых боях режим будет выбираться, как и прежде, случайным образом") | N `prevyu-obnovleniya-027` | official | official | - | keep random |
| Region | clans tied to a region from 0.27c | press, Mar 2015 | later | - | - | not applied |

## 9. Matchmaking, modes, maps

| Fact | Value | Version / date | Source | Confidence | srp now | Recommended |
|---|---|---|---|---|---|---|
| Team size, match time | "two teams of up to 8 players", "rounds last up to 15 minutes"; SHOT is 8 against 8 | W-2015-02-08 `Teamplay`; data `match_time 900` | wiki + data | wiki of the time | 8 a side (`SRP_MATCH_TEAM_SIZE`), 900 s | done |
| Modes | batteries scattered, batteries from charging stations ("both variations ... available on all the maps", 0.25a), Team Deathmatch from 0.26a | PN 0.25a, 0.26a | official | official | modes 2, 3, 4 | keep |
| TDM | 100 tickets, 150 on Tarakanovsky Fort and Cologne Bridge (0.26c); base spawns (0.26d) | PN 0.26c, 0.26d | official | equals the data | 125 tickets on every played map (`modes.tsv`; the owner, 2026-11-13) | done |
| Mode choice | none in 0.26 (added in 0.27a) | PN 0.27a | official | official | - | random mode |
| Map choice | "the history of players' matches is taken into account now. The system selects the less played map" | PN 0.25a | official | official | the map the match's players played least (`g26_map_plays`; `matchmaking.md` §5) | done |
| Level of a player | by gear; "если вы создадите профили с разным уровнем экипировки, то противники будут искаться по максимальному уровню экипировки"; ammo level counts (0.23g); ELO modifies it (0.22a) | N `prevyu-obnovleniya-026`; PN 0.23g, 0.22a | official | official, and the client computes it the same way (0x2CA9B0; `matchmaking.md` §1) | the client's own level; the match's level is the mean of its players' | done |
| Range | "Reduced starting level range in matchmaking" (0.25b); wider for high gear levels (0.25a) | PN | official | no number; the client shows a range that starts at the level and the next (data) and grows by one at each end every "search step" the server sends (0x225655) | that range, the step being srp's (`SRP_MATCH_SEARCH_STEP_SECONDS`, 4) | srp's choice |
| Least players to start | nothing found; the data has `wait_player_percent 0.8` | data | - | - | - | - |
| Maps | eight; Radar back in rotation in 0.25c | PN 0.25c | official | equals the data | - | keep |

## Apply list (official or contemporary sources for 0.25 to 0.27 only), most certain first

1. `account.rs` `MAX_LEVEL` 50 → 100, and 250 gold at level 100.
2. Sell the 9A-91 and VSK-94 at the Fringe Settlers (ranks 8 and 9), not Renaissance.
3. Ammo, consumables and grenades are sold by the factions at ranks (tables in §3), not by
   seller 0 at rank 0. The 7N1 at Renaissance rank 7 is the one row with an official line.
4. The seven-day bonus: a day counts when its three wins are made, and a missed day does not
   reset the run.
5. Quest rewards scale with the level of the match they were completed in, and double in a
   first-three-wins match.
6. Bundle prices: 1,500 / 2,000 / 2,000 / 300 / 700 / 5,000 / 5,000 gold (AK-74N, Bandit,
   Survivor, Lancaster, Vityaz, Vikhr, Vintorez).
7. Premium for 90 days: 2,500 gold.
8. A new account owns the PPSh-41, the Mosin and the TT-33 with their free ammo.
9. Rank exceptions: Scavengers' chemical set and Amber respirator 2; Fort 17 and APS 2; the
   Marodeur container at the Black Market, rank 4.
10. Score weights: kill 10, assist 5, stolen battery 35.
11. Match rewards by the screen's rows (§2): a kill 22 silver, 2.4 experience, 1 reputation;
    a help event 10 / 1 / 1; all times `match_levels_dict`; activity row as proposed. Kills
    fit SHOT within 0.2 %, help exactly, activity within 0.5 % but its split is a guess.
    `SRP_EXPERIENCE_RATE` default 1. Rating starts at 100.
12. Drop chance: 40 % base, +2.5 % a battery home, +25 % premium, each in its own part of the
    results screen.
13. Dropped weapons and armour arrive worn with a defect (official for 0.24a, never taken
    back).
14. Clan registration needs 5 players on the live server.
15. Matchmaking: 8 a side; level from the highest-level kit; the least-played map.

**Applied 2026-10-12 (shop side):** 2, 3, 6, 7, 8, 9 and 14 (14 in
`deploy/env-026g.example`; the code's default stays 1). For 3, ammo keeps its sellers but
needs no rank since 2026-10-13 (the owner's choice; drugs, the trap and grenades keep
theirs); the 9x19 7N21 is on no list
and stays with seller 0; "7.62x39 RPD" is taken to be `ammo_7.62x39_m` (LIKELY: the one
7.62x39 with a clip of 100) and "12.7x54" both `ammo_12.7x54_pt` and `ammo_12.7x54_vps`.
For 8, the first kit still wears only the TT-33 and its clip; the Mosin, its clip and the
PPSh-41 are in the inventory. The table is `prices.rs` `EXCEPTIONS`. The clan price and
size became 50,000 silver and 30 members by the owner's choice (not from a source).

## Not applied: only later-version evidence

- Experience table 1000 + 250 a level (after 0.27a).
- Clan price 250,000 silver, 15 members, the deputy / sergeant rights (mid 2015).
- Selling at 15 %, repair at 20 %, +25 / 50 / 100 % for modifiers when selling (0.27).
- Three daily quests with bronze, silver and gold, and what a change of faction does to them
  (0.27a).
- Quest gear of the current or previous level (0.27b).
- Team premium bonuses (10 % in 0.31; 20 % to quality in 0.23a).
- The 0.28 rank tables and prices (AKS-74U 60,000, Lancaster 5,250).

## Not found

- Premium prices for a day, 3 days, a week, a month, 6 months, a year (only 90 days = $25).
- The gold to silver rate, the skill reset price, the gold price of a nickname change.
- A new account's starting silver and gold.
- The 0.26 clan price, clan sizes by level, clan experience and levels, clan battle rules.
- How the activity row splits between finishing, winning and events; why a kill's worth
  varies; a results screen of a lost match; reputation and experience for battery events.
- New-player protection rules; the least players to start a match; the matchmaking range.
- What changing faction did to running quests in 0.26; how often quests were handed out.
- The condition and defect of a dropped item; the team's premium drop bonus; the TDM drop
  formula; why SHOT shows 90 %.
- The official forum's patch topics (login wall in every snapshot), VK posts and video
  descriptions: not reached.
