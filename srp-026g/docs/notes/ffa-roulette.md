# Free for all: Roulette, everyone plays the same random equipment

The owner, 2026-11-22: "let's design ffa: randomness (come up w/better name)", "where everyone gets
the same random equipment of the same level (3 kits are different)"; of the levels: "it should be
either lvl1 to 5, lvl6 to 10, lvl11 to 15"; "high lvl rooms can get low lvl bands"; then "roulette
should also give random bonuses on equipment/weapons", "the ammo type should be highest available",
"and for shotguns it should be a mix".

A twist of the free-for-all cards (`ffa.md`, "Twists"), beside Racket and Saloon. The owner
approved the name, level 0 in the low band and every recommendation below (2026-11-22). Code:
`survarium::v026g::roulette`, `twist`, `session::v026g::rooms::Rooms::deal`.

## In short (for the owner)

- The card draws a **level band** among those the room allows (a level-15 room 1-5, 6-10 or 11-15;
  a level-10 room 1-5 or 6-10), each as likely, then **one equipment**: three kits from that band.
- **Everyone plays that same equipment**, all three kits, picked in the warm-up as any kit.
- **Each kit:** one gun in weapon 1's slot (weapon 2's empty), its best ammo (a shotgun buckshot and slugs), a full
  set of armour, medkit, grenade and more pockets; every gun and armour piece with random bonuses,
  as a dropped item gets them, every gun random attachments that fit it. The three guns are of **three different classes** (say a shotgun, a sniper
  rifle and an SMG).
- **Nothing of the players' is used or charged**: no item, no ammo, no medkit. Their skills still
  apply (more pockets, more ammo bags, Deep Pockets' one weapon).
- **Share:** 25 % of the free-for-all cards since 2026-11-27 (the owner, of FFA: Pistols, "take 5%
  from roulette and 5% from FFA normal": plain 25, Racket 30, Roulette 25, Pistols 10, the Saloons
  5 and 5; `ffa-pistols.md`); 30 % from 2026-11-22 (plain 30, Racket 30, Roulette 30, Saloon
  (Winchester) 5, Saloon 5).
- **Name on the card:** "FFA: Roulette", the same for every band (the owner, 2026-11-22, the day it
  went live with the band in the name: "roulette shouldn't show its level", "it should just be
  roulette").
- Server and content only (the lobby movie, the names in the 11 languages, the mode icons): no exe
  patch.

## The name

**Roulette**, ru **Рулетка**, uk **Рулетка** (my pick over the working name "The Deal", the owner's
choice): one word, as
Racket and Saloon; it says "random" at once, in every language nearly the same word; the band
after it says what is at stake. The working name stays the alternative: **Deal**, ru **Раздача**,
uk **Роздача** (everyone is dealt the same hand; in English "a deal" also reads as a bargain, and
«раздача» also as a giveaway).

The card's names, `st_srp_mode_roulette_low` / `_mid` / `_high`: one text for the three keys in
each language since 2026-11-22 (the table's `<band>` is no longer in the game's texts):

| language | text |
| --- | --- |
| en | FFA: Roulette `<band>` |
| uk, ru, be | FFA: Рулетка `<band>` |
| de | FFA: Roulette `<band>` |
| pl | FFA: Ruletka `<band>` |
| cs | FFA: Ruleta `<band>` |
| hu | FFA: Rulett `<band>` |
| fi | FFA: Ruletti `<band>` |
| sr | FFA: Rulet `<band>` |
| hr | Svatko za sebe: Rulet `<band>` |

(Croatian's prefix as its other twists'.)

## The band

- **Bands:** low 0-5, mid 6-10, high 11-15, by the item's `match_making_level`: the rooms' own
  measure of a kit (`loadouts::gear_level`; `item_level` differs from it on the medicines, whose
  `mm_level` the medicines' table sets). Level 0 is the low band's: the starter items (TT-33,
  PPSh-41, Mosin carbine) are low-level gear like the rest, and the low band is thin without them
  (two assault rifles). [my pick; the owner said 1 to 5]
- **Which bands a room allows:** each band whose top is at most the room's max level. A room of
  any max level (0) allows all three, as Racket takes such a room for a level-15 one
  (`racket::Gun::of_room`). A room under level 5 allows none: its card never draws Roulette (left out
  of the draw, the other shares as they are). Public rooms are 10 or 15 today (`rooms.md`, the two brackets);
  private rooms have no twists.
- **Drawn per card,** uniformly among the bands the room allows (the owner: equal unless there is
  a reason): a level-15 room's Roulette 1-5, 6-10 and 11-15 a third each, a level-10 room's 1-5
  and 6-10 half each. No reason seen to weigh them otherwise.
- **The whole equipment from that band:** every gun and armour piece of it. The pockets are the
  band's or lower (the game has no medkit above `mm_level` 3), a pistol hand's pistols too (none is above 10).
  The ammo is a gun's highest, whatever its level (the owner: "the ammo type should be highest
  available"): a kit's level may be over its band by its ammo alone, which no room checks in a
  match (the rooms' door reads the account's own kits).

## What is drawn

One **hand**: three kits, drawn once per card. Each kit's slots (`data.md`, "slots_restrictions"):

| slot | what | rule |
| --- | --- | --- |
| 7, weapon 1 | a main gun of the band | its class (assault rifle, machine gun, shotgun, sniper rifle, SMG) one of three different classes the hand draws, one a kit |
| 8 | weapon 1's ammo | its highest ammunition; a shotgun's highest buckshot (below) |
| 9 | a shotgun's slugs | its highest slug, beside its buckshot; empty for any other gun |
| 10, 11, 12, weapon 2 and its ammo | empty | one gun a kit (below) |
| 19-22 | empty | the gun's two ammo slots are 8 and 9 |
| 0-6, armour | helmet, mask, torso, back, legs, gloves, boots: one piece of the band each | a slot another drawn piece covers stays empty (`covered_slots`) |
| 13-18, pockets | a medkit, a grenade, then four other pocket items | drawn kinds, each pocket full (below) |
| artefacts | none | no profile slot; they are found in the match |
| a gun's kit slots (scope, muzzle, grip) | random attachments that fit it (below) | a sniper rifle no scope: it keeps the one its own config names (`weapon_scopes.md` §1); no camo |

- **Three classes:** the hand draws three different classes among the main-gun classes the band
  has (each class as likely: 4 in the low band, which has no machine gun; 5 in the others), then a
  gun of each class (each gun as likely). So the three main guns always differ, and so do the
  kits' roles. Pistols are no main-gun class.
- **One gun a kit** (the owner, 2026-11-22: "it shouldn't try to slot in two weapons or pistols in
  second slot"): weapon 2's slot and its ammo slots (10, 11, 12) stay empty, whatever the kit's
  carry rule. A kit with weapon 1 alone is one every carry rule allows and the game plays (Saloon's
  Colt alone leaves weapon 2 empty the same way; only a spawn with both weapon slots empty ends a
  game, `play::match_profile`). Every ammo bag is the gun's.
- **A pistol hand** (the owner, 2026-11-22: "we can have random versions where everyone starts with
  pistols", "it should still be random one", about 5 %): `roulette::PISTOL_HANDS_PERCENT` (5) hands
  in 100 are a pistol hand: each kit's one gun a pistol at random of the band's (three different
  ones while it has three), with the same bonuses, attachments and ammo rules; the 11-15 band takes
  the 6-10 pistols (none is above level 10). Drawn with the hand, the same for everyone. The card
  says Roulette as any (no name of its own); players see the pistols in the warm-up kit picker.
- **Armour,** drawn per kit, slot by slot: torso, helmet, mask, legs, gloves, boots, back. A
  piece is a candidate only if no slot it covers is filled; a slot a drawn piece covers is left
  empty. The back slot takes what the band has there: backpacks, the artefact containers, the
  oxygen cylinders, the field radio. Every slot has pieces in every band (`docs/data/equipment.tsv`:
  5 to 17 a slot and band).
- **Weight:** a kit weighs at most a player's `max_carried_weight` (30 kg, without skills), guns,
  ammo and pockets in. A kit over it draws its armour again (up to 16 times, the same draw); after
  that its heaviest pieces are left out, one by one, until it fits. (The high band's
  exoskeletons weigh 33 kg alone.)
- **Ammo** of a gun: of the ammunition the dictionary pairs with it (`compatible`) that fits its
  ammo slot and the shop sells, the highest level (the lower dict id of two alike), whatever the
  band. A shotgun (category 14) a mix: its highest buckshot (a round of more than one pellet,
  `data.buck_shot`) and its highest slug (one projectile), each in an ammo slot of its own (8 and
  9), its bags split between them, the buckshot the first and every other one; a shotgun of one
  kind (the KS-23M: buckshot only) has that alone. Its rounds: a clip for each of the kit's ammo
  bags, as Saloon's Winchester and Colt (`ffa-cowboy.md`, "Rounds"): the bags as the lobby counts
  them (`KitLayout::ammo_bags`: the player's 3, the worn pieces' `ammo_bags_modifier`, their
  bonuses' and the skills'), all the gun's; a gun with no clip its magazine alone.
- **Bonuses** (the owner, 2026-11-22: "roulette should also give random bonuses on
  equipment/weapons"): every gun and armour piece of the hand gets its modifications as a dropped
  item does (`Bonuses::roll`): a rarity from the after-match drop table (`drops::DropTable::
  gear_rarity`, `rarity_points` [0, 1, 1.6, 1.9, 2]: none half the time, one 30 %, two 15 %, three
  5 %, as the item's own `rarity` list allows), then that many modifications, each of another type
  the item takes, by drop weight (`Modifications::roll_for_rarity`, the one pool of drops and
  upgrades). Rolled once with the hand from a generator its card's draw seeds, so every player has
  the same; on the match's items as their own modifications, as an owned item's reach a match. They
  count in the kit's layout (an ammo bags or pockets bonus opens them). Nobody's inventory gets
  them, and they pay nothing (below). The weight cap counts the pieces' plain weight.
- **Pockets:** six kinds a kit, each a different item type (one pocket a type,
  `one-pocket-a-type.md`), of the pocket categories (10 medicines and boosters, 11 traps, 24
  grenades) the shop sells, at most the band's top: first a healing medkit (a medicine whose
  dictionary name ends in `medkit`: the Medikit, the military, science and Black Market ones,
  as likely), then a grenade (frag or smoke), then four of the rest (bandages, antitoxin,
  painkiller, the boosters, the trap, the other grenade) in a drawn order. The kit holds as many as
  its pockets open, each pocket full: the layout's own limit (`loadouts::KitLayout::kept`: the
  pockets and pocket size its worn pieces and skills give, the RGD-5's half rule, Additional
  Medicine). A player's base layout is one pocket of one unit, so most of a kit's pockets come
  from its drawn torso and backpack.
- **The pool:** the guns, the armour and the attachments are every item of the dictionary that fits
  the slot (or is a module of the gun), of the band, and is no golden or premium copy of another
  (`Item::like`): the owner, 2026-11-22, "the VSS, SR-3M, SOCOM 16 and the second NeoStead must be
  in". Against the shop's list (the rule before), that lets in exactly those four guns and nothing
  else, gun or armour: every other plain gear item of the live data (revision 69's quick build) is
  sold, or a premium plain item (the AK-74N, the tier-4 Scavenger suit's torso and legs), and every
  one has a name in the game's texts; none is a test or event item. The ammunition and the pocket
  items stay the shop's (sold, or premium). Cosmetics have no profile slot. Nothing is listed in
  code; an item to keep out later would take a key of its own in the game data.
- **Attachments** (the owner, 2026-11-22: "there should be random applicable attachments on
  weapons"; `attachments.md`): every gun of the hand, for each kind
  of module it takes by the game data's modules table (the scope, the muzzle device, the grip:
  `weapon_modules::Modules::fits`), none half the time (my pick), else one of the modules of that
  kind that fit it beside those drawn before it (`Modules::check`, the one fit check), each as
  likely (`Attachments::roll`). A sniper rifle gets no scope: its own stays. The modules have no
  levels (`match_making_level` 0 for all 84), so any band takes any. Drawn once with the hand;
  they reach the game as an owned module does, the gun's records (`weapon_kit::pack`, as Racket's
  sight), their item ids from 1,000,000 up (no item of the profile, as an owned module is in no
  kit). Nobody's inventory gives or gets one.
- **The three kits** differ by their main gun's class at least; armour and pockets are
  drawn for each kit on its own and may repeat.

Example, a level-10 room's Roulette 6-10 (illustration):

```
kit 1  Benelli M4 (shotgun)   Glock 17    Sokol vest, Viper helmet, ..., Watchman backpack
kit 2  SV-98 (sniper)         CZ 75       Explorer's jacket, Pathfinder helmet, ..., Token
kit 3  UMP45 (SMG)            Glock 17    Shturm vest, Scout's helmet, ..., Field radio
pockets  military medkit, RGD-5, bandages, antitoxin, ...   (as many as each kit opens)
```

## Determinism, late joiners and replays

- **Drawn once, with the card,** in the lobby's deal (`session::v026g::rooms::Rooms::deal`), by
  the same random source as the card's map and twist (`below`), right after the twist's roll: the
  band, the classes, the guns, the armour, the pockets, then the seed of the bonuses. The hand is a value the card holds
  (`twist::Armed::Roulette(Hand)`, in `twist::Played`), so the match server, the logs and the
  history read the same hand; nothing draws again.
- **Sent as the other twists' kits:** the match server arms every profile it gives (the first
  players', each late joiner's: `registry`, `Played::arm`) with that hand; every game cooks each
  life from the static info's profiles. A late joiner gets the same three kits.
- **Replays** hold the profiles the players were sent (`replay_watch.md`: chunk 1, kits and items),
  so a replay plays the hand as the match did, with no rule of its own.
- **The world format does not change:** the items are the game's own, in slots they already take;
  no exe patch saves or loads anything new. Kits of three loaded per player already happen today
  (any account with three playable kits). Still, per `AGENTS.md`, a local test match is played and
  its worlds checked (`tools/check` world-check) before the merge.
- **Simulation change row:** what a match computes for a given static info is the same, so older
  replays would not play out otherwise. The `next` row of
  `crates/session/src/v026g/simulation_changes.tsv` gets a line anyway (as Racket's level-10 gun
  did, revision 60), so the release names it; it costs nothing, `next` already has rows.

## Players' own things

- **Nothing of theirs is in the match:** every item of the profile is the hand's (the account's
  leave the profile's table), whole (`FULL_CONDITION`), its rolled bonuses its only own
  modifications, no upgrades;
  ids from 1. Every stack's store is the largest kit's count of it [`twist::STORE_LIVES`] times, so
  every life has its ammo and its full pockets (`ffa.md`, "Twists").
- **Nothing is charged:** no wear (no match wears an item since 2026-11-08), no ammo, no medkit,
  no grenade. `autobuy::used` charges nothing for a Roulette match: none of the account's stacks
  was in it (stronger than Racket's "the ammo the twist gives": here every stack is the hand's).
- **Every kit is loaded** (`KIT_USABLE`), each has a gun by construction, whatever the account's
  own three are. Kit ids and names are the account's, as the other twists keep them.
- **Skills apply** [my recommendation]: the kits' modifiers stay the player's. They are his
  character (health, stamina, reload) as in every twist; they change only the layout here: more
  pockets and bags hold more of the same items, Deep Pockets, Supply Pocket and Terminator change
  nothing of the guns: a kit has one. The alternative,
  zeroing the modifiers, would take the whole skill tree out of the match, not only the layout.
- **The rooms' doors are as now:** a player still needs a playable kit of his own, within the
  room's max level, to be in the room; the card is drawn after.
- **Pay:** as any free-for-all (score, win, rating). The items' bonuses (the pay's equipment
  block, `items_rules::kit_bonuses`) are none: the kits were not the account's. The history keeps
  no kit items for a twist's match already (`rewards::kit_items`).

## The draw share among the free-for-all cards

**Plain 30, Racket 30, Roulette 30, Saloon (Winchester) 5, Saloon 5** (the owner, 2026-11-22; before
it, plain 50, Racket 40 and the Saloons, then my pick of Roulette 10 from the plain mode). In a room that allows no band
(under level 5) Roulette is left out of the draw. On a map played with a twist only (Military
Warehouses, `ffa.md`) the plain share is out and Roulette keeps its 30 of the 70 left. The band is a second draw of
the 30 (each allowed band a third or a half of it), so no share is a fraction.

## Lobby and HUD

- **The card:** "FFA: Roulette", its mode tag and colour (one colour for the three bands, as
  the two Saloons share one). Lobby mode numbers **14, 15, 16** (low, mid, high):
  `twist::LobbyMode::Roulette(Band)`. 7 is Scarcity's, 9-11 the twists', 12 Freeze Tag, 13 Artifact
  Hunt; 14-16 are used nowhere (main, the branches and worktrees checked 2026-11-22). The room's
  banner of its match, the rooms list, the history and the site name it the same
  (`LobbyMode::name_key`).
- **The kits before the match:** the warm-up's kit selector (patch 31, `kit-change-before-start.md`)
  shows the three dealt kits, as the game cooks them from the match profile; a player switches
  among them freely until the start (20 s of countdown). Nothing new is needed for that.
- **Not in v1** [my recommendation]: the hand on the card before the vote (a tip with the three
  main guns: the card row would carry them, `C;...` in `RoomsWindow.as`, and the movie draw them),
  and a line at the match's start. The band in the name says what the vote needs; the guns are a
  surprise until the warm-up, which is the twist's point.
- The game's own screens (loading, results, HUD) say Free for all, as for every twist. The loading
  screen's icon is Roulette's own (two dice; `ffa.md`, "Mode icons"), by the room's match.

## Decisions (the owner approved each recommendation, 2026-11-22)

1. Name: **Roulette / Рулетка / Рулетка** (rec.), or Deal / Раздача / Роздача.
2. Band odds: **equal among the allowed bands** (rec., the owner's default).
3. The band in the card's name, three lobby numbers 14-16: **yes** (rec.: people vote knowing it,
   as the owner wanted for Saloon).
4. Level 0 in the low band (0-5, not 1-5): **yes** (rec.: starter guns, a thin band otherwise).
5. Skills apply: **yes** (rec.).
6. Share: 30 (the owner, after: plain 30, Racket 30, Roulette 30, the Saloons 5 each).
7. Main guns of three different classes, each class as likely: **yes** (rec.; a class with one gun,
   the mid band's RPD, then comes as often as the 9 assault rifles).
8. No pistol in the kits: one gun a kit (the owner, after); a pistol hand 5 % of the time.
9. Ammo: the gun's highest-level ammo, whatever the band (the owner, after: "the ammo type should
   be highest available"); a shotgun its highest buckshot and its highest slug ("for shotguns it
   should be a mix").
10. Attachments: random ones that fit (the owner, after: "there should be random applicable
    attachments on weapons"), each kind half the time (my pick).
11. The pay's equipment block: **none** (rec.).
12. Weight held to 30 kg a kit: **yes** (rec.).
13. The hand shown on the card before the vote: **later** (rec.).

## Implementation plan

No exe patch. Server, lobby movie, the names, the icons.

- `crates/survarium/src/v026g/roulette.rs` (new), the twist's rules, as `racket.rs` and `cowboy.rs`:
  - `Band` (`Low`, `Mid`, `High`: its levels, `allowed(max_level) -> &[Band]`), `Hand` (its band
    and three `Dealt` kits: its one gun and its ammo, armour `[Option<u16>; 7]`, pockets
    `[u16; 6]`: dict ids only, `Copy`, made only by `Hand::deal`), `Pool` (each band's candidates
    by slot and class, read once from the game data, `OnceLock` as `Arms::shared`).
  - `Hand::deal(band, below)`: the draw above, a pure function of the rolls and the data (pools in
    dict id order).
  - `arm(profile, &hand)`: the profile's items the hand's (each once, shared by the kits that hold
    it, as the lobby sends them), every kit rebuilt (`KIT_USABLE`, id, name and modifiers kept),
    rounds by `free_bags`, pockets by `KitLayout::kept`, carry rule by `CarryRule`, stores
    `STORE_LIVES`, `MatchKit::new` packs the pockets; `drop_unheld`.
  - `check(data)`: srp refuses to start on data where a band lacks three main-gun classes, a
    piece for a slot, a medkit and a grenade, or ammo for one of its guns;
    beside `racket::check` and `cowboy::check` (`game_data.rs`).
  - Tests: the same rolls give the same hand; every kit's `gear_level` within its band; three
    classes; ammo the gun fires and the slot takes; no covered slot worn; 30 kg; `check_sent`
    passes, pockets packed, under 69 items; one gun a kit whatever its skills; a bot profile armed
    alike; one band of each test data band dealt.
- `twist.rs`: `Twist::Roulette`; `Armed::Roulette(Hand)`; `LobbyMode::Roulette(Band)` (numbers
  14-16, name keys, `ALL` 14 long); `shares` and `draw` with the 10 (the draw names what to arm, the
  hand dealt after it); `gives` every stack of the hand; `arm`. The tests' shares and numbers.
- `session/src/v026g/rooms.rs` `Rooms::deal`: an open room's Roulette draws its band among
  `Band::allowed(max_level)` (none: plain) and deals the hand with the same `below`.
- `session/src/v026g/autobuy.rs` `used`: nothing for a Roulette match; `rewards.rs`: no equipment
  block for it.
- `lobby-server/src/v026g/rooms.rs`: the match's log line names the band and the three main guns,
  not the hand's `Debug`.
- `match-server` tests (`tests/free_for_all.rs`): a Roulette match sends every player, and a late
  joiner, the same armed profile.
- Lobby movie: `RoomsText.as` `MODES` rows 14-16, `RoomsLook.as` `MODE_COLOUR` 14-16; the test that
  ties them to `LobbyMode` follows. A movie edit, no new content row; the movie and the server ship
  together (an older movie shows no name for a number it does not know: `RoomsText.modeName`).
- Names: 3 keys in the 11 `client-patches/localization/*.tsv` (in key order); the content patch
  makes them the game's own texts (content edit `localization-1`), as Racket's.
- Docs: `ffa.md` "Twists" (the shares, the names, the numbers), `rooms.md` (the shares line), this
  note; the `next` line of `simulation_changes.tsv`.
- Mode icons (`ffa.md`, "Mode icons"): `SrvGameModeIcons` frames 6-10 and their bitmaps, the loading
  screen's twist icon by the room's match, content edit `ffa-icon-1` (Free for All's `ui.icon` 5).
- Release: the server and the content patch (the movie, the texts, `ffa-icon-1`) together.

## Risks

- **Thin pools:** the low band has 2 assault rifles and no machine gun, the mid band one machine
  gun (RPD), no pistol is above level 10 (a pistol hand of 11-15 takes 6-10's). The rules above handle each; `check` stops srp at start
  if live data thins further.
- **A kit that crashes every game:** a spawn with no gun ends every game of the match, an empty
  pocket before a held one too (`lobby-server` `play::match_profile`). Every kit has a main gun by
  construction and `MatchKit::new` packs the pockets; the tests hold both.
- **Odd items in the back slot and masks** (containers, oxygen cylinders, the field radio, the
  NVD mask): drawn as any piece; worth a look in the local test.
- **Memory:** fewer distinct models than a plain match (three kits for everyone instead of each
  player's own), so no new load; three loaded kits a player as today's full accounts.
- **Balance:** a level-15 room plays the low band a third of its Roulettes, as the owner wants; a
  mid band's sniper kit against shotguns on a small map may feel lopsided. That is the twist.
- **Card text width:** "Svatko za sebe: Rulet 11-15" and "FFA: Рулетка 11-15" in the card's
  13 px field: checked with the rooms harness.
- **Not seen:** whether the warm-up selector shows anything odd for kits whose names are the
  account's (a kit named "Sniper" holding a shotgun). Kept as the other twists keep them.
