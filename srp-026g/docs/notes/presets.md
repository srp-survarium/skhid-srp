# Kit presets

The owner, 2026-10-23: "we need to build presets (5), it is a drop down menu in Character, the
idea is to people have presets for different equipment levels and allow them to join low level
lobbies (they will see a popup that they can join with a lower level preset)"; "preset is a
snapshot of 3 kit contents"; items "just like normal kits are"; on join "Apply then join", "and
then they join with a new one". Then, the same day: "no need for set or apply, just choose preset
applies it, and adding equipment to it saves things". Code: `session::v026g::presets` (the rules),
`rules::fill_kit`, `persist/presets.rs`, `lobby-server` `presets.rs` and `rooms.rs`; the lobby:
`LocalPlayerProfile` (the panel's title), `RoomsWindow` (the popup). Wire: `lobby.md` §2.16.3 and
§2.16.4.

- **Five presets an account, one active,** kept with it in srp.db (`g26_preset_slots`,
  `g26_preset_active`).
- **A preset** is the contents of the three kits (Profile 1, 2, 3: every slot, weapons, ammo,
  armour, pockets): the owned item in each slot, and for a stack its units. Not the kits' names,
  nor which kit is played [choice].
- **The active preset is the kits.** It is kept nowhere else, so every change of a kit (a move,
  the ammo window, a purchase dropped onto a slot, a sale or craft that takes an item, a match's
  ammo, a repair of the kits at load) is the active preset's, with nothing to save and no path
  that could miss it [choice: no copy kept in step]. The other four are snapshots, as they were
  left.
- **Items, as kits:** a slot names an item the account owns. Kits share one inventory: an item may
  be in each of the three kits (one slot of a kit at most). Presets follow it: the same item may be
  in several presets. A preset reserves, copies or moves nothing.
- **Choosing a preset** in the Character panel's title (the only control): the kits stay with the preset left as
  its snapshot, then the chosen one is put on the three kits and becomes active. One server
  action, all or nothing: each kit emptied and filled through the lobby's kit rules
  (`rules::fill_kit`, the moves' own `relocate`): the item fits the slot, one item one slot of a
  kit, a covering item takes off what it covers, one pocket an item type.
  - An item no longer owned (sold, crafted away, used up): its slot is just empty, and nothing
    is said (the owner's, 2026-10-23: "I got a popup when item was sold, which I shouldn't", "the
    slot should just be empty"). The server's log line still counts the slots.
  - A stack: the units kept, at most what is owned now. A stack used up and bought again is a new
    item: the stack of that type owned now is taken [choice].
  - A type in two pockets (data from before the rule): the first pocket keeps it, the other stays
    empty.
  - An item that no longer fits its slot (the game's data changed since; the repair at load takes
    such an item off a kit, not off a snapshot): its slot stays empty; the preset can still be
    chosen.
  - The kit played: when it can not play after the choice (no weapon, too little ammo) and another
    kit can, that one is played, the kit whose level is the preset's, so the room's gear check and
    the kit its match takes agree (review of #314).
  - Never buys (the ammo window's moves buy what is missing; choosing does not), never makes an
    item or money, never takes anything the account does not own.
  - Refused (nothing changes) while the player is in a match or on its way to one.
  - **An empty preset chosen starts empty** (the owner's, 2026-10-23: "empty presets should start
    empty and not copy items from previous preset"): the three kits are emptied. A kit names
    owned items, it holds none, so every item stays in the inventory as it was (stacks whole):
    nothing is made, sold or lost; the preset left keeps its kits as its snapshot, and choosing
    it again puts them back exactly. The empty one chosen again changes nothing.
  - With the kits empty (or the kit a match would take unable to play, `rules::kits_can_play`),
    the owner's of 2026-10-23 ("cannot play pop up is bad, the button should just be gray"):
    - **Play is greyed** (no press, no popup) and lights up as soon as the kits can play (a
      switch, an equip): the movie reads the reasons in the presets list (the table below), the
      server's own rule, so the two can not disagree. Leave (in a room or its queue) never greys.
      The greyed button's tip lists the reasons.
    - **Join** in the rooms list (and Queue), and a friend's "Join room", are greyed (the friend's
      item left out of the menu) unless the join popup can offer another loadout that can play
      (one at most the room's max level; any max level, a friend's room). The popup then says
      "Your loadout can't play. Loadout N is level L." with "Exit" / "Switch and enter" (offer
      kind 3); the active loadout is never offered.
    - The server refuses all of them at the door (code 15: Play with or without an open room,
      Join, a PIN's join, a queue): no room made or joined. The movie shows nothing of code 15.
      Making a room by hand is not refused (the member check warns in the room).
    - The game's own Play (`2F`, not the rooms' path) keeps its stock refusal ("no weapon").
    - An empty preset has no level and is never offered.
  - A preset left with nothing in its kits is empty.
- **Why Play or Join is greyed: the reasons** (the owner's, 2026-10-23: "multiple reasons might get
  added in the future and it is better to have all of them documented"). The presets list carries
  them as bits (`<blocks>`, `lobby.md` 2.16.4), worked out by the server's own functions, the same
  that refuse at the door, so the movie's greying and the server can not disagree. A greyed Play,
  Join, Queue or New room shows every reason that holds in its tip (the game's tooltip, as
  Autobuy's), a line each, with no full stop at a line's end (the game's own texts trimmed
  too) and the text's left margin as wide as its right (`ReasonsTip`: the renderer's own margin
  setting; the owner's, 2026-10-23: "no need for dot in the end and a bit more padding from the
  left"). The kit's reasons are `rules::PlayBlock` (`rules::play_blocks` of the
  kit a match would take, `rules::kits_play_blocks`; `check_play` says the first); the maintenance
  rule is the lobby's (`Core::entry_blocks`, `BLOCK_MAINTENANCE`).

  | bit | reason | when | greys | server's door | text (en / uk / ru / de / pl) |
  | --- | --- | --- | --- | --- | --- |
  | 1 | kit locked | the account may not use the kit a match would take (`kit_usable`; today only an account without kits) | Play; Join, Queue unless another loadout can play | code 15 | This profile is locked / Цей профіль закритий / Этот профиль закрыт / Dieses Profil ist gesperrt / Ten profil jest zablokowany |
  | 2 | covered slot | an item is worn in a slot another worn item covers (data from before the rule) | as 1 | code 15 | Two items share a slot / Два предмети в одному слоті / Два предмета в одном слоте / Zwei Gegenstände in einem Platz / Dwa przedmioty w jednym miejscu |
  | 4 | two pockets | one item type in two pockets (data from before the rule) | as 1 | code 15 | One item type in two pockets / Один тип предмета у двох кишенях / Один тип предмета в двух карманах / Ein Gegenstandstyp in zwei Taschen / Ten sam przedmiot w dwóch kieszeniach |
  | 8 | no ammo | a weapon has less ammo in the kit (or owned) than its category's minimum | as 1 | code 15 | the game's `st_profile_check_failed_weapon_ammo`; without it: Too little ammo for a weapon / Замало набоїв до зброї / Мало патронов к оружию / Zu wenig Munition für eine Waffe / Za mało amunicji do broni |
  | 16 | no weapon | no weapon in the kit (an empty loadout) | as 1 | code 15 | the game's `st_profile_check_failed_weapon`; without it: No weapon / Немає зброї / Нет оружия / Keine Waffe / Brak broni |
  | 32 | maintenance | a maintenance is planned within the rooms' window (`maintenance_is_near`, `MAINTENANCE_WINDOW`, 30 min: the minutes the clients are told, at which the game turns its own Play off) | Play, Join, Queue, New room | code 9 (no room made, no entry) | Server maintenance soon: in N min / Скоро технічні роботи: за N хв / Скоро технические работы: через N мин / Bald Wartung: in N Min / Wkrótce przerwa techniczna: za N min |

  - Checked: the server at every room entry (`room_enter_fit`: Play with or without an open room,
    Join, a PIN's join, a queue; making a room: maintenance only), and for the list
    (`send_presets`); the movie greys by the list (`LocalPlayerProfile.mayPlay`, `mayEnter`,
    `mayCreate`; `StatusPanel.showRoomsPlay`, `RoomsList`, `RoomsCreate.onNew`,
    `FriendListRender`). The movie never lights a Play the game keeps off.
  - A Join and a full room's Queue ask the popup first: a friend's "Join room" (whose room's
    max level the movie does not know) is greyed by any reason, not lit for a loadout the popup
    could offer. "Switch room" is not offered while a reason holds (`room_switch`
    asks the door's `room_enter_fit`). A queued player whose kit can not play when a place
    frees keeps its place, passed over, and the next goes in (`next_ready_in_queue`).
  - Until the first list the reasons are not known: Play, Join, Queue and New room start greyed,
    with no tip (the owner's, 2026-10-23: "the game starts with Play red, and you can quickly
    click on it before it is blocked"). The list comes within about 2 s (it is asked again every
    2 s); with none 10 s after the game's first word on Play (the lobby live; the asks made while
    the movie loads are lost) the buttons light and the server's doors alone decide. Play is
    greyed from the panel's start, before the game's first word on it. A Play that opened the
    rooms window and is refused for the kit (code 15) within 6 s closes it again; a poll's plain
    answer before Play's own does not end that, a room, another code, the window closed, or a
    Join or New room in it do. The empty list's hint says "press Play" only while Play is really
    lit (`noneNoPlay`, `noneQuiet` when New room is greyed too).
  - N, the minutes, come with the list (`<minutes>`); while a maintenance is planned the movie asks
    for the list every minute, so N and the rule's start follow.
  - **A new reason** gets a bit (`PlayBlock` or a lobby bit next to `BLOCK_MAINTENANCE`), a row
    here, a test that it comes out for its case and that the list carries it, and its texts in the
    movie's `REASONS` (`LocalPlayerProfile`).
- **Existing accounts:** preset 1 active (their kits as they are), presets 2 to 5 empty.
- **A preset's level:** the rooms' measure (`matchmaking::gear_level`, the refusal "gear over the
  max level"): the highest kit level of the three kits that can play (all three, not only the
  active one). The active preset's is the kits' own; another's is that of the kits it would give
  now (sold items gone). A preset none of whose kits can play (no weapon, too little ammo) has no
  level and is never offered.
- **Players see "Loadout"** (the owner's, 2026-10-23), never "preset" (code, logs, docs and
  tables keep "preset"). The movie's words, en / uk / ru / de / pl:

  | word | en | uk | ru | de | pl |
  | --- | --- | --- | --- | --- | --- |
  | the name | Loadout 2 | Комплект 2 | Комплект 2 | Loadout 2 | Zestaw 2 |
  | empty | empty | порожній | пустой | leer | pusty |
  | can't play | can't play | не для бою | не для боя | nicht spielbar | nie do gry |
  | refused | The loadout could not be chosen. | Не вдалося обрати комплект. | Не удалось выбрать комплект. | Das Loadout ließ sich nicht wählen. | Nie udało się wybrać zestawu. |
  | sale, one | Used in Loadout 2. Sell anyway? | Використовується в комплекті 2. Продати? | Используется в комплекте 2. Продать? | Wird in Loadout 2 genutzt. Trotzdem verkaufen? | Używany w zestawie 2. Sprzedać? |
  | sale, more | Used in Loadout 2 and Loadout 4. Sell anyway? | Використовується в комплектах 2 і 4. Продати? | Используется в комплектах 2 и 4. Продать? | Wird in Loadout 2 und 4 genutzt. Trotzdem verkaufen? | Używany w zestawach 2 i 4. Sprzedać? |

  The level's word is the kit tabs' own (`clan_players_level_label`, "Lvl."). The join popup's
  lines (`st_srp_rooms_offer_higher`, `_offer_fits`, `_code_14`) say "Loadout" too. The movie's fonts
  (CalibriBold, Calibri Bold) have every glyph of them.
- **A sale asks first** (the owner's, 2026-10-23: "when selling an item which is equipped in any
  kit of any preset I should get a warning"): selling an item a preset uses, the active one's
  kits or another's (an item it would put on now), the game's own sell dialog shows one more line
  under the item's name, "Used in Loadout 2 and Loadout 4. Sell anyway?"; Sell sells, Cancel
  sells nothing. An item in no preset sells as before. The list (`lobby.md` 2.16.4) carries the
  items the presets use; the server sells as ever (the question is the client's).
- **The inventory marks what the loadouts use** (the owner's, 2026-10-23): an owned item (stacks
  too) that a loadout uses has a small badge with those loadouts' roman numerals at its tile's
  bottom-left corner, above its name: "II", "I IV", a run of three or more joined ("I-V",
  "II-IV V"). Every loadout that uses it, the active one too [choice: the badge says where an
  item is, as the sale's question does; the game's own equipped mark still tells the kits worn].
  The data is the sale's (the list's U rows); the inventory is drawn again when they change (a
  list after a move, a choice, a sale). The badge: the stack count's font and size (CalibriBold
  14), the equipped green, on a dark backing so it reads over any icon; it covers neither the
  level (top-left) nor the count (bottom-right) (`ItemRendererBase.updateLoadoutBadge`,
  `LocalPlayerProfile.loadoutMark`). The Upgrade window's item and donor slots show it too, at the
  same corner of the same tile: their items leave the list while they are there (craft-menu.md,
  "Items in the slots"), and the badge went with them (owner, rev 31) (`ItemRendererFull`'s
  `showLoadouts`, set by `MessageCraftItem`). The upgraded result, the paper doll and the other
  windows' tiles have none.
- **Character panel:** its title is the switcher (the owner's, 2026-10-23: the dropdown left of
  the title was ugly). The title names the active preset in the title's own font and colour,
  centred, with a caret after it: "Loadout 2 · Lvl. 15" (the level's word is the kit tabs'
  own). A press anywhere on the title opens the five under it, the title bar's width: "Loadout 1"
  left and "Lvl. 15", "empty" or "can't play" right. The look (the owner's, 2026-10-23: "bigger
  font", "cleaner, nicer design"): the title 24 px (the panel's stock 22), its field the bar's
  width less the caret's room, so every language's longest title fits one line; rows 36 px with
  19 px text (the stock option rows': 24 and 14) on the game's dropdown backdrop, a line between
  rows; the active row as the rooms list's selected room (gold-tinted, a gold bar at its left,
  white text); hover and press lighten a row and the title alike (`LocalPlayerProfile`'s
  `ROW_H`). A press on a row chooses it (one press to open, one to choose); a
  press outside, Esc or a window opening closes the list with no change (a transient window:
  `transient-windows.md`). No step arrows: each step would choose a
  preset, so going from 2 to 4 would put 3 on the way. Until the server's first list the title
  is "Character"; while the kits are locked (a search, a match) it takes no press.
  - The movie asks for the list at its setup, on every account push, and every 2 s until the
    first list comes: an ask sent while the movie loads is lost (the game has not installed its
    handler yet: "ExternalInterface.call - handler is not installed" in the game's log), and with
    it the title stayed "Character" (the first in-game test, 2026-10-23). A test session is
    checked with `scripts/presets-check/check_presets_session.py <runtime/packets> --since <start>`:
    every lobby client must get the list within 10 s and ask for it before any choice.
  A choice refused says so in the game's message box ("The loadout could not be chosen.").
- **Joining a room** (the owner's, 2026-10-23: "If the person joins a high level room with low
  level preset, they get a warning which will switch them to the biggest one: enter anyway, switch
  and enter | for low level ones it is exit or switch and enter"; "we choose the preset which has
  the highest allowed level"):
  - The preset offered is the saved one with the highest gear level the room allows (at most the
    room's max). Ties are broken by the lower preset number. One loadout is offered, with "Exit" /
    "Switch and enter", even when two tie: no button a loadout (the owner's, 2026-10-23: "one
    offer keeps the popup simple").
  - A room's gear range is its max level alone (the room's "level" is its members' match level,
    no cap). A room of any max level (0) offers nothing [choice]; nor does a private room (a
    PIN), which has no max level (the owner's of 2026-10-26).
  - **Room allows more than the kits:** gear at most the max, and a preset higher than the gear
    fits: "Enter anyway" (the kits as they are) or "Switch and enter". None higher: joins, no popup.
  - **Gear over the max:** a preset fits: "Exit" (no join) or "Switch and enter". None fits: the
    refusal as before (code 12).
  - "Switch and enter": that preset chosen (active, its kits put on), then the join (a room
    full by then: its queue, as its Queue).
  - Where: Join in the list (a friend's "Join room" goes through it), the Queue of a full room,
    and Play (Quick join), for the room Play would take now. "Switch and enter" from Play
    chooses, then Play picks again by the new gear [choice]. Not "Switch room" (it already takes
    the gear), nor making a room.
  - **Queue of a full room** (the owner's of 2026-11-01, the room's Queue pressed with no offer
    on rev 40): the offer of a Join of that room, the same popup and text. "Room allows more":
    its first button is "Queue" (the queue with the kits as they are); "gear over the max" and
    "can't play" keep "Exit". "Switch and enter" chooses the loadout, then queues (a place kept
    if already queued there), or joins if a place has freed.
  - **Play that would queue** (the owner's of 2026-10-30: "I want exactly the same
    recommendation when I join a room with lower/higher gear"): no room of its bracket takes
    it now and Play would wait in a queue. Play then offers what a Join of the other bracket's
    room with a place would (the room Play would take with that gear), when the loadout offered
    is of that bracket:
    - gear 11-15, a level-10 room with a place, a saved loadout of level 10 or less: Join's
      "gear over the max" offer;
    - gear 10 or less, a level-15 room with a place, a saved loadout of level 11-15: Join's
      "room allows more" offer.
    - The same popup and text as that Join. Its first button is "Queue": Play as before, into
      the queue. "Switch and enter" chooses the loadout; Play then takes that room by the new
      gear.
    - Nothing is offered when the Join would offer nothing, or only a loadout of the player's
      own bracket (a gear-5 player whose best loadout is level 9, say). Then Play queues as
      before: Play does not put a level-10 player into a level-15 room as it is. Nothing is
      offered when Play makes its bracket's first room either.
  - The server decides the popup (the movie asks before joining); it shows over the Rooms window
    (Play opens it), the view under it inactive until a button. "Switch and enter" is one server
    action: choose, then the join; a choice refused joins nothing (code 14).
- **Gone with the owner's change of the same day:** the Save and Apply buttons, saving into a
  preset by hand, an "apply" that left no preset active, and their requests on the wire.
- **Texts:** short; srp's dictionary texts (`st_srp_loadout_*`, `st_srp_rooms_*`,
  `client-patches/localization`).
- **Wire:** srp's `3B` ranking starts and `91` rows, as the rooms; no exe change.
- **Release:** server and lobby movie together.
