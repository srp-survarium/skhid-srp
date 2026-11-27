//! Client to server requests (payloads after `id x`, the sub byte included).
//! `Request::decode(id, payload)` parses, `Request::encode()` gives `(id, payload)` again (used by tests and by
//! scripted clients). Strings are the client's `u8` / `FF u16` strings. Ids or subs not modelled stay
//! [`Request::Other`] with the raw payload.
//!
//! Tags: PROVEN = builder read in the client at the quoted address and/or the bytes seen in client logs;
//! LIKELY = layout from the builder only, trigger or meaning inferred.

use crate::ids::c2s as id;
use crate::items::ItemRecord;
use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// One entry of `45 00` (16 bytes on the wire). PROVEN (client logs):
/// `u32 item uid, u16 dict id, u8 from slot, u8 to slot, u32 count / rounds, u32 x`.
/// Slot 100 = inventory, 0..22 = profile slots, 200 = "bought straight from the shop" (pocket popup).
/// An ammo move may name the item by its dict id (uid == dict).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipEntry {
    pub item: u32,
    pub dict: u16,
    pub from: u8,
    pub to: u8,
    pub count: u32,
    pub x: u32,
}

/// `45 xx` inventory requests (builders 0x200187160 etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum Inventory45 {
    /// 00 `apply_profile_items`: `u32 profile id, u8 n, n x EquipEntry, u32 counter, u32 counter`. PROVEN. Sets the
    /// client's pending flag 7A5758, which ANY server message clears (answer with 83/87 or any push).
    Equip { profile: u32, entries: Vec<EquipEntry>, c1: u32, c2: u32 },
    /// 01 `relocate_upgrade_item`: `u32 item (the armour piece), u32 id (uid of the decal / camo item), u8 slot, u64 x`
    /// (the `f64` bits). Wire order PROVEN by the builder 0x2001875D0; meaning from the reply reader 0x200190238.
    RelocateUpgrade { a: u32, b: u32, c: u8, d: u64 },
    /// 02 `drop_upgrade_item`: `u32 item, u32 id, u8 slot` (builder 0x2002CABA0, reply reader 0x200190422).
    DropUpgrade { a: u32, b: u32, c: u8 },
    /// 03 `item_use`: `u32 item id`.
    Use { item: u32 },
    /// 04 decal editor save: `u32 item, u32 id, u64 x (f64 bits), u8 slot`. The client has already set `x` on the entry
    /// of `item` with that `id` and slot (PROVEN 0x2001878B4..0x20018794E); no reply is read.
    Decal { a: u32, b: u32, c: u64, d: u8 },
    /// 05 `reroll_item_modifier`: `u32 item, u16 group, u16 grade, u8 pay (0xFF gold, 0 spare parts)`
    Reroll { item: u32, group: u16, grade: u16, pay: u8 },
    /// 06 `modifier_choosed`: `u32 item, u16 old group, u16 old grade, u32 row, u32 gold`. `row` = the list row the player
    /// clicked (`modifiersList.selectedIndex`), `gold` = the price the client showed for a "premium" row, else 0 (PROVEN:
    /// AS `RerollItem.doReroll`, writer 0x20028D87C..0x20028D9DF; the old pair is the item's modifier at the slot).
    RerollPick { item: u32, old_group: u16, old_grade: u16, row: u32, gold: u32 },
    /// 07 `craft_item` (workshop upgrade): `u32 item, u8 pay (0xFF gold, 0 spare parts)`. PROVEN (client logs).
    Craft { item: u32, pay: u8 },
    /// 08 `craft_item_choose_modifier`: `u32 item, u32 index` into the sorted offer. PROVEN.
    CraftPick { item: u32, index: u32 },
    /// 0C / 0D / 0E boosters: `u16`, `u16`, `u16 u16` (answers ignored by the client).
    BoosterEnable { dict: u16 },
    BoosterDisable { dict: u16 },
    BoosterReplace { old: u16, new: u16 },
    /// 10 module upgrade: `u32 x4, u8 pay` (loop of 4 elements, LIKELY).
    ModuleUpgrade { items: [u32; 4], pay: u8 },
    /// 11 `change_weapon_skin`: `u32 item (weapon uid), u16 skin (dict id of the skin item)`. PROVEN: handler 0x20028D530 builds `45 11 u32 u16` from the two Flash arguments (docs/notes/weapon-skins.md).
    Skin { item: u32, skin: u16 },
    /// 12 `preview_weapon_skin`: `u32 item, u16 skin`. PROVEN: handler 0x20028E360 (log "process_preview_weapon_skin %d %d").
    SkinPreview { item: u32, skin: u16 },
}

/// `46 xx` shop requests.
#[derive(Debug, Clone, PartialEq)]
pub enum Shop46 {
    /// 00 `buy_ok_clicked`: `u16 dict id, u32 count, u8 target slot (100 = inventory), u32 active profile id, u8 bool`.
    /// PROVEN (0x200187A40).
    Buy { dict: u16, count: u32, slot: u8, profile: u32, flag: u8 },
    /// 01 `sell_ok_clicked`: `u32 item id, u32 amount`. PROVEN.
    Sell { item: u32, amount: u32 },
    /// 02 `accept_premium` (0x24B890) / `accept_premium_victory_screen` (0x290E20): `u32 offer id, u32 match id (0 from the
    /// lobby), f32 0.0` (builder 0x200187E90, PROVEN; `b` holds the float's bits).
    BuyOffer { offer: u32, a: u32, b: u32 },
    /// 03 `exchange_premium_money`: `u32 amount` (gold). No answer; silver += amount * rate, then push 8B.
    Exchange { amount: u32 },
    /// 06 `open_chest`: `u32 item id`.
    OpenChest { item: u32 },
    /// 07 `start_item_search`: `u8`.
    ItemSearch { a: u8 },
    /// 08 `favorite_weapon_discount_clicked`: `u32, u8`.
    FavoriteDiscount { a: u32, b: u8 },
    /// 09 `buy_usage_of_specific`: `u16`.
    BuySpecific { dict: u16 },
}

/// `48 xx` account / clan requests (PROVEN builders, meaning per trigger LIKELY).
#[derive(Debug, Clone, PartialEq)]
pub enum Account48 {
    ProfileRename { profile: u32, name: String },
    EnableProfile { profile: u32, enable: u8 },
    ProfileChanged { profile: u32 },
    BuyPreset { a: u8 },
    /// 04 `request_nickname_validation`: `str nickname, u32 3`. PROVEN (client logs).
    NicknameCheck { name: String, x: u32 },
    /// 05 `change_player_nickname`: `str nickname, u8, u32`.
    NicknameChange { name: String, a: u8, b: u32 },
    /// 06 `comment_about_user`: `u64 player, u8 kind, u8, u32`.
    Comment { player: u64, kind: u8, a: u8, b: u32 },
    ReserveClanName { name: String, abbr: String },
    CreateClan,
    ClanNameCheck { name: String },
    ClanAbbrCheck { abbr: String },
    ImportantMessageRead { id: u64, answer: u8 },
    /// 0D: `u32` silver deposited on the registration (the Flash stepper value, 0x200280FC0).
    BuyClanRegistration { amount: u32 },
    /// 0E: the confirmed leave / disband / cancel-registration dialog (0xD7 / 0xDA; 0x2001E9093), no payload.
    LeaveClan,
    /// 0F: `u8 0`, sent by `clan_window_opened` / `clan_members_tab_opened` (0x200281F10 -> 0x20018CEE0); no dialog.
    ClanWindowOpened { a: u8 },
    /// 14: `u64 member`, the confirmed kick (dialog 0xDB, 0x2001E9277).
    Kick { member: u64 },
    /// 0C: `u64 player`, sent after the `83 48 10` invite reply (0x2001928B5): the invitation went out; the client prints
    /// "Player <name> successfully invited to clan." from the `83 48 0C u64, str128 name` reply (0x20019294A).
    ClanInviteSent { player: u64 },
    ClanInvite { player: u64 },
    ClanPermission { rank: u8, value: u8, permission: u8 },
    /// 12: `u64 member, u8 to_commander`: 0 = rank down (0x20028195F); 1 = the CONFIRMED hand-over of command after the
    /// `83 48 13 01` question (0x2001E8F69).
    RankDown { member: u64, a: u8 },
    RankUp { member: u64 },
    CreateRank { name: String },
    RemoveRank { rank: u8 },
    DailyMessage { text: String },
    ClanData,
    /// 28 `action_item_exchanged`: `u16, u16`. Sets pending flag 7A5759: needs `83 48` / `84 48`.
    ActionItemExchanged { a: u16, b: u16 },
    BoosterAutobuy { dict: u16, on: u8 },
    HeroOfTheDay { a: u32, b: u8 },
    DayHeroData { a: u8, b: u32 },
}

/// `49 xx` squad requests.
#[derive(Debug, Clone, PartialEq)]
pub enum Squad49 {
    Disband,
    /// 02: sent by the INVENTORY inviter's client after a kind 3 message arrives: `u64 joiner id`.
    Accepted { player: u64 },
    Kick { player: u64 },
    Leave,
    /// 05 own member state in a PvE squad (builder 0x20018DAF0): `u8 state, u8 0`. `state` = the new value of the sender's
    /// own `state` byte of the AE record: bit 0 ready (Play pressed by a member), bits 1..3 "I pay for squad slot
    /// bit - 1", bits 4 / 5 the commander locked slot 0 / 1 (docs/notes/pve.md 1.1).
    PveState { state: u8, zero: u8 },
}

/// `4A xx` quests / factions.
#[derive(Debug, Clone, PartialEq)]
pub enum Quest4A {
    /// 00: `u8 faction` (1..4), the active-faction change; the answer is 8E. PROVEN (client logs).
    SetFaction { faction: u8 },
    Sub01 { a: u8 },
    SkipTutorial,
    DailyReroll { quest: u32 },
    ChestOpened { a: u8 },
    FactionBattleRewards { a: u8 },
}

/// `4B xx` league window.
#[derive(Debug, Clone, PartialEq)]
pub enum League4B {
    Opened,
    Ack,
    /// `4B 04 u64`: the public id of the player whose season history the window shows (PROVEN: the builder 0x20027D330
    /// takes it from the Flash argument with `flash_value::GetUInt64`; `ask_league_history` passes the viewer's own
    /// `PlayerInfo.publicAccountId` on the player tab, the selected friend's / clan member's on the other two).
    History { public_id: u64 },
    TopPlayers,
}

/// `4E xx` friend list actions.
#[derive(Debug, Clone, PartialEq)]
pub enum Friends4E {
    Remove { id: u64 },
    AddIgnore { id: u64 },
    RemoveIgnored { id: u64 },
    /// 04 `find_players`: `str query`.
    Find { query: String },
}

/// One tracked event of request 53. PROVEN (client logs): `u16 id, u16, u32 value, u32 time`.
/// Ids from push B8; 5 / 6 and 0x3D / 0x3E tell a won / lost match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event53 {
    pub id: u16,
    pub x: u16,
    pub value: u32,
    pub time: u32,
}

/// `55 xx` achievements.
#[derive(Debug, Clone, PartialEq)]
pub enum Achievements55 {
    /// 00 (after B9 / 83 55): `u32`.
    Ack { a: u32 },
    /// 04 `achievements_window_opened`.
    WindowOpened,
}

/// `5B xx` play / lobby load. PROVEN (client logs).
#[derive(Debug, Clone, PartialEq)]
pub enum Play5B {
    /// 01 play pressed: `u32 map, u32 mode, u32 weather` (13 bytes with the sub).
    Play { map: u32, mode: u32, weather: u32 },
    /// 02 lobby-load handshake (first per connection) or tutorial start (later ones): no fields.
    Load,
    /// 03 `goto_shooting_range`: `item record (the weapon to test, with its modules), u8`. PROVEN by client logs (the
    /// 22-byte form is a record with empty lists plus one trailing byte; a longer form carries one module). Trailing byte meaning unknown (0 in logs).
    ShootingRange { weapon: ItemRecord, tail: u8 },
}

/// `5F xx` PvE (docs/notes/pve.md section 1.1). PROVEN builders: 00 0x2001EBF40, 01 / 02 0x2001EB730, 03 0x2001EC8E0,
/// 04 0x2001973E0, 05 0x2001974E0, 06 0x200197460.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pve5F {
    /// 00 `set_pve_match_settings`: `u32 map id, u32 difficulty`. Squad commander only.
    Settings { map: u32, difficulty: u32 },
    /// 01 (squad commander) / 02 (solo, squad count 0) `pve_match_start_clicked`: `u32 map id, u32 difficulty`.
    Start { solo: bool, map: u32, difficulty: u32 },
    /// 03 `ask_pve_top_100` (the client asks at most once per 60 s).
    Top100,
    /// 04 `close_pve_stats_clicked`.
    CloseStats,
    /// 05 `pve_unlock_rewards`.
    UnlockRewards,
    /// 06 `pve_reroll_rewards`.
    RerollRewards,
}

/// Weapon module action of `60 01`. PROVEN.
/// `u8 type, u32 f24, u32 f20 module uid, u16 f28 module dict, [type 0 or 2: str slot], u16 f2A cost, u8 f2D,
/// u8 f2C bound`. Types: 0 attach, 1 detach (f24 1 = destroy), 3 buy with gold, 4 buy with spare parts, 6 bind to this
/// weapon (f2C = action id of the buy that created the module), 7 rebind to this weapon (f24 = old binding), 2 and 5
/// logged only (PROVEN 0x200282E90, 0x200284F00).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleAction {
    pub kind: u8,
    pub f24: u32,
    pub module_uid: u32,
    pub module_dict: u16,
    /// Present for types 0 and 2.
    pub slot: Option<String>,
    pub cost: u16,
    pub f2d: u8,
    pub bound: u8,
}

/// `60 xx` weapon workshop.
#[derive(Debug, Clone, PartialEq)]
pub enum Workshop60 {
    /// 00 `on_weapon_upgrade_opened`: `u16 dict`. Answered with push B3.
    Opened { dict: u16 },
    /// 01 Save: `u32 weapon uid, u16 weapon dict, u32 n, n x ModuleAction`. Answered with `83 45 0F`, 8B, `83 60 01`.
    Save { weapon: u32, weapon_dict: u16, actions: Vec<ModuleAction> },
}

/// Every modelled request.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// 2E sign-in: `u32 session, u8, u8` (`78563412 ff ff`). PROVEN.
    SignIn { session: u32, a: u8, b: u8 },
    /// 2F play / queue: `u32 global (0x200AA2134), u8 arg, u32 mode, u8 bool` (LIKELY, builder 0x200185E60).
    Queue { global: u32, arg: u8, mode: u32, flag: u8 },
    /// 31 cancel: `u32 order id`.
    QueueCancel { order: u32 },
    /// 32 (after the 80): `u8, u8 bool`. PROVEN (`0200`).
    Info32 { a: u8, b: u8 },
    /// 33 squad member data: `u8, u64 member id, u32 profile id`.
    MemberProfile { a: u8, member: u64, profile: u32 },
    /// 37 / 39 / 3A / 34 / 35 "generic id 00": one byte. 37 also acks 97 (`u8`).
    Generic { id: u8, a: u8 },
    /// 38 `victory_screen_request_player_data`: `u8 0, u64, u32`.
    VictoryData { a: u8, player: u64, b: u32 },
    Inventory(Inventory45),
    Shop(Shop46),
    /// 47 00 `apply_skills`: `u8 n, n x {u16 skill id, u8 points}`.
    Skills { skills: Vec<(u16, u8)> },
    /// 47 01 `reroll_ok_clicked` (reset learned skills): `u8 branch (0 = all, LIKELY), u8 payment (0 silver, 1 gold)`.
    /// PROVEN wire order (0x20028FEB0 stores sub 1, branch, payment at 0xB0/0xB8/0xC0 before the send); the meaning of the
    /// bytes comes from MessageAskReroll.as (`branch`, `money_type` of the two cost check boxes).
    SkillReset { branch: u8, payment: u8 },
    Account(Account48),
    Squad(Squad49),
    Quest(Quest4A),
    League(League4B),
    /// 4C chat: `u8 channel, u32 target id, str text`. PROVEN (client logs: `01 00000000 05 "rerer"`).
    /// `target`: channel 4 (private) = u64 public account id; other channels = u32 context id (0x200264519).
    Chat { channel: u8, target: u64, text: String },
    /// 4D: `u64 id, u8 kind, str text`; kind 0 add friend (accept invite), 2 squad invite, 3 accept squad invite,
    /// 0x17 PvE squad invite, 0x18 accept PvE squad invite.
    People { id: u64, kind: u8, text: String },
    Friends(Friends4E),
    /// 4F messaging sign-in: `u32 a, k x u32, u8, u8`. PROVEN by the logs in two sizes: 30 bytes at sign-in
    /// (`ffffffff`, k = 6, `02 ff`) and 18 bytes later (`000000e0`, k = 3, `02 00`, sent again when the squad id
    /// changes, squad id at +0xB8 of the client struct, LIKELY). `k` is taken from the length.
    MessagingSignIn { a: u32, b: Vec<u32>, c: u8, d: u8 },
    /// 50 system info blob (541 bytes in the logs), opaque.
    SystemInfo { blob: Vec<u8> },
    /// 51 read important message: `u32 message id, u8`.
    ReadMessage { id: u32, a: u8 },
    /// 53 event tracker flush: `u32 n, n x Event53`. PROVEN (logs).
    Events(Vec<Event53>),
    Achievements(Achievements55),
    /// 56 ping results of the 9E game-server list (PROVEN, builder 0x195150): `u8 n`, n x {`u8 server id`, `u16 ping ms`};
    /// the client pings the servers one after the other and sends the whole list after the last.
    Retry56 { pings: Vec<(u8, u16)> },
    /// 57 `server_list_updated` (PROVEN, builder 0x195560): `u32 selected-server mask` (bit = server id; the client's
    /// saved choice), `u32 flags` (`[0xAA2120]`, 0x10 forced when bit 4 is set).
    ServerList { a: u32, b: u32 },
    /// 5C sub 0 `get_actions_data` (builder 0x200196130, PROVEN): `u8 0, u32 tab, u8 language, u8 steam` (7 bytes in the
    /// client logs). `a` is the sub byte (0).
    ActionsData { a: u8, b: u32, c: u8, d: u8 },
    /// 5C sub 1 `buy_action` (builder 0x200196CC0, PROVEN): `u8 1, u32 data id, u8 mode` (mode 1 = gold; the other
    /// payment kind is not sent, the client only drops its cached tabs).
    ActionBuy { id: u32, mode: u8 },
    /// 5E `daily_reward_collect`: `u32`.
    DailyReward { a: u32 },
    Play(Play5B),
    Pve(Pve5F),
    Workshop(Workshop60),
    /// 58 equipment presets (builder 0x2001EAB40, one callback per op). PROVEN layouts.
    Presets(Presets58),
    /// 5A item search / roulette (0x200196F80, 0x2001EA8B0). PROVEN layouts, meaning LIKELY.
    Roulette(Roulette5A),
    /// Anything else, payload as received.
    Other { id: u8, payload: Vec<u8> },
}

/// `58 <op> ...` equipment presets. Sender 0x2001EAB40 (`process_default_stored_preset_action`, log strings
/// "preset create %s" / "preset remove %d" / "preset rename %d %s" / "preset save %d" / "preset load %d", the op byte
/// is the first payload byte). PROVEN: layouts and which op is which (the log string sits in the same branch as the
/// op's writes). `profile` is the id of the active profile record (profile record + 0x29F8), `preset` the id the
/// `BB` push gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presets58 {
    /// Op 0: `str name, u32 profile`: store the profile's current equipment as a new preset.
    Create { name: String, profile: u32 },
    /// Op 1: `u32 preset`.
    Remove { preset: u32 },
    /// Op 2: `u32 preset, str name`.
    Rename { preset: u32, name: String },
    /// Op 3: `u32 profile, u32 preset`: equip the preset on the profile.
    Load { profile: u32, preset: u32 },
    /// Op 4: `u32 profile, u32 preset`: overwrite the preset with the profile's current equipment.
    Save { profile: u32, preset: u32 },
}

/// `5A <sub> ...`: sub 2 `u8 flag, [flag == 0: u32 hunt]` (builder 0x200196F80, PROVEN: flag = 0xFF or 0, and with 0 the
/// `a` field of the last `AA` hunt state follows; `item_search_opened` = the 0 form LIKELY, `last_item_search_ok_clicked`
/// = 0xFF; the faction level-up window also sends the 0 form, 0x2001C3478), sub 3 `u8, u8` (`change_roulette_clicked`,
/// 0x2001EA8B0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Roulette5A {
    Sub2 { a: u8, hunt: Option<u32> },
    Sub3 { a: u8, b: u8 },
}

fn str_cap(r: &mut Reader) -> Result<String> {
    r.str()
}

impl Request {
    /// Parse the payload of request `mid` (bytes after `id x`). Strict: the whole payload must be consumed.
    pub fn decode(mid: u8, p: &[u8]) -> Result<Request> {
        let mut r = Reader::new(p);
        let req = Self::decode_inner(mid, &mut r, p)?;
        r.finish()?;
        Ok(req)
    }

    fn decode_inner(mid: u8, r: &mut Reader, _p: &[u8]) -> Result<Request> {
        Ok(match mid {
            id::SIGN_IN => Request::SignIn { session: r.u32()?, a: r.u8()?, b: r.u8()? },
            id::QUEUE => Request::Queue { global: r.u32()?, arg: r.u8()?, mode: r.u32()?, flag: r.u8()? },
            id::QUEUE_CANCEL => Request::QueueCancel { order: r.u32()? },
            id::INFO_32 => Request::Info32 { a: r.u8()?, b: r.u8()? },
            id::MEMBER_PROFILE => Request::MemberProfile { a: r.u8()?, member: r.u64()?, profile: r.u32()? },
            0x34 | 0x35 | 0x37 | 0x39 | 0x3A => Request::Generic { id: mid, a: r.u8()? },
            id::VICTORY_DATA => Request::VictoryData { a: r.u8()?, player: r.u64()?, b: r.u32()? },
            id::INVENTORY => Request::Inventory(Self::inv(r)?),
            id::SHOP => Request::Shop(Self::shop(r)?),
            id::SKILLS => {
                match r.u8()? {
                    0 => {
                        let n = r.u8()? as usize;
                        Request::Skills { skills: (0..n).map(|_| Ok((r.u16()?, r.u8()?))).collect::<Result<_>>()? }
                    }
                    1 => Request::SkillReset { branch: r.u8()?, payment: r.u8()? },
                    _ => return Err(Error::Unsupported("47 sub other than 0 or 1")),
                }
            }
            id::ACCOUNT => Request::Account(Self::account(r)?),
            id::SQUAD => Request::Squad(match r.u8()? {
                1 => Squad49::Disband,
                2 => Squad49::Accepted { player: r.u64()? },
                3 => Squad49::Kick { player: r.u64()? },
                4 => Squad49::Leave,
                5 => Squad49::PveState { state: r.u8()?, zero: r.u8()? },
                _ => return Err(Error::Unsupported("49 sub")),
            }),
            id::QUESTS => Request::Quest(match r.u8()? {
                0 => Quest4A::SetFaction { faction: r.u8()? },
                1 => Quest4A::Sub01 { a: r.u8()? },
                2 => Quest4A::SkipTutorial,
                3 => Quest4A::DailyReroll { quest: r.u32()? },
                4 => Quest4A::ChestOpened { a: r.u8()? },
                5 => Quest4A::FactionBattleRewards { a: r.u8()? },
                _ => return Err(Error::Unsupported("4A sub")),
            }),
            id::LEAGUE => Request::League(match r.u8()? {
                1 => League4B::Opened,
                2 => League4B::Ack,
                4 => League4B::History { public_id: r.u64()? },
                5 => League4B::TopPlayers,
                _ => return Err(Error::Unsupported("4B sub")),
            }),
            id::CHAT => {
                let channel = r.u8()?;
                let target = if channel == 4 { r.u64()? } else { r.u32()? as u64 };
                Request::Chat { channel, target, text: str_cap(r)? }
            }
            id::PEOPLE => Request::People { id: r.u64()?, kind: r.u8()?, text: str_cap(r)? },
            id::FRIENDS => Request::Friends(match r.u8()? {
                1 => Friends4E::Remove { id: r.u64()? },
                2 => Friends4E::AddIgnore { id: r.u64()? },
                3 => Friends4E::RemoveIgnored { id: r.u64()? },
                4 => Friends4E::Find { query: str_cap(r)? },
                _ => return Err(Error::Unsupported("4E sub")),
            }),
            id::MESSAGING_SIGN_IN => {
                let a = r.u32()?;
                #[allow(clippy::manual_is_multiple_of)]
                let bad_len = r.remaining() < 2 || (r.remaining() - 2) % 4 != 0;
                if bad_len {
                    return Err(Error::Invalid("4F: length is not 4 + 4k + 2"));
                }
                let k = (r.remaining() - 2) / 4;
                let b = (0..k).map(|_| r.u32()).collect::<Result<Vec<_>>>()?;
                Request::MessagingSignIn { a, b, c: r.u8()?, d: r.u8()? }
            }
            id::SYSTEM_INFO => Request::SystemInfo { blob: r.rest().to_vec() },
            id::READ_MESSAGE => Request::ReadMessage { id: r.u32()?, a: r.u8()? },
            id::EVENTS => {
                let n = r.u32()? as usize;
                r.check_count(n, 12)?;
                Request::Events(
                    (0..n)
                        .map(|_| Ok(Event53 { id: r.u16()?, x: r.u16()?, value: r.u32()?, time: r.u32()? }))
                        .collect::<Result<_>>()?,
                )
            }
            id::ACHIEVEMENTS => Request::Achievements(match r.u8()? {
                0 => Achievements55::Ack { a: r.u32()? },
                4 => Achievements55::WindowOpened,
                _ => return Err(Error::Unsupported("55 sub")),
            }),
            0x56 => {
                let n = r.u8()? as usize;
                r.check_count(n, 3)?;
                Request::Retry56 { pings: (0..n).map(|_| Ok((r.u8()?, r.u16()?))).collect::<Result<_>>()? }
            }
            0x57 => Request::ServerList { a: r.u32()?, b: r.u32()? },
            0x58 => Request::Presets(match r.u8()? {
                0 => Presets58::Create { name: str_cap(r)?, profile: r.u32()? },
                1 => Presets58::Remove { preset: r.u32()? },
                2 => Presets58::Rename { preset: r.u32()?, name: str_cap(r)? },
                3 => Presets58::Load { profile: r.u32()?, preset: r.u32()? },
                4 => Presets58::Save { profile: r.u32()?, preset: r.u32()? },
                _ => return Err(Error::Unsupported("58 op")),
            }),
            0x5A => Request::Roulette(match r.u8()? {
                2 => {
                    let a = r.u8()?;
                    // the u32 follows the 0 form (a short 0 form is still taken, as before)
                    let hunt = if a == 0 && r.remaining() >= 4 { Some(r.u32()?) } else { None };
                    Roulette5A::Sub2 { a, hunt }
                }
                3 => Roulette5A::Sub3 { a: r.u8()?, b: r.u8()? },
                _ => return Err(Error::Unsupported("5A sub")),
            }),
            0x5C => match r.u8()? {
                1 => Request::ActionBuy { id: r.u32()?, mode: r.u8()? },
                a => Request::ActionsData { a, b: r.u32()?, c: r.u8()?, d: r.u8()? },
            },
            0x5E => Request::DailyReward { a: r.u32()? },
            id::PLAY => Request::Play(match r.u8()? {
                1 => Play5B::Play { map: r.u32()?, mode: r.u32()?, weather: r.u32()? },
                2 => Play5B::Load,
                3 => Play5B::ShootingRange { weapon: ItemRecord::decode(r)?, tail: r.u8()? },
                _ => return Err(Error::Unsupported("5B sub")),
            }),
            id::PVE => Request::Pve(match r.u8()? {
                0 => Pve5F::Settings { map: r.u32()?, difficulty: r.u32()? },
                sub @ (1 | 2) => Pve5F::Start { solo: sub == 2, map: r.u32()?, difficulty: r.u32()? },
                3 => Pve5F::Top100,
                4 => Pve5F::CloseStats,
                5 => Pve5F::UnlockRewards,
                6 => Pve5F::RerollRewards,
                _ => return Err(Error::Unsupported("5F sub")),
            }),
            id::WORKSHOP => Request::Workshop(match r.u8()? {
                0 => Workshop60::Opened { dict: r.u16()? },
                1 => {
                    let weapon = r.u32()?;
                    let weapon_dict = r.u16()?;
                    let n = r.u32()? as usize;
                    if n > 64 {
                        return Err(Error::Limit("60 01 actions (64)"));
                    }
                    let mut actions = Vec::new();
                    for _ in 0..n {
                        let kind = r.u8()?;
                        let (f24, module_uid, module_dict) = (r.u32()?, r.u32()?, r.u16()?);
                        let slot = if kind == 0 || kind == 2 { Some(r.str()?) } else { None };
                        actions.push(ModuleAction {
                            kind,
                            f24,
                            module_uid,
                            module_dict,
                            slot,
                            cost: r.u16()?,
                            f2d: r.u8()?,
                            bound: r.u8()?,
                        });
                    }
                    Workshop60::Save { weapon, weapon_dict, actions }
                }
                _ => return Err(Error::Unsupported("60 sub")),
            }),
            _ => Request::Other { id: mid, payload: r.rest().to_vec() },
        })
    }

    fn inv(r: &mut Reader) -> Result<Inventory45> {
        Ok(match r.u8()? {
            0 => {
                let profile = r.u32()?;
                let n = r.u8()? as usize;
                let mut entries = Vec::new();
                for _ in 0..n {
                    entries.push(EquipEntry { item: r.u32()?, dict: r.u16()?, from: r.u8()?, to: r.u8()?, count: r.u32()?, x: r.u32()? });
                }
                Inventory45::Equip { profile, entries, c1: r.u32()?, c2: r.u32()? }
            }
            1 => Inventory45::RelocateUpgrade { a: r.u32()?, b: r.u32()?, c: r.u8()?, d: r.u64()? },
            2 => Inventory45::DropUpgrade { a: r.u32()?, b: r.u32()?, c: r.u8()? },
            3 => Inventory45::Use { item: r.u32()? },
            4 => Inventory45::Decal { a: r.u32()?, b: r.u32()?, c: r.u64()?, d: r.u8()? },
            5 => Inventory45::Reroll { item: r.u32()?, group: r.u16()?, grade: r.u16()?, pay: r.u8()? },
            6 => Inventory45::RerollPick { item: r.u32()?, old_group: r.u16()?, old_grade: r.u16()?, row: r.u32()?, gold: r.u32()? },
            7 => Inventory45::Craft { item: r.u32()?, pay: r.u8()? },
            8 => Inventory45::CraftPick { item: r.u32()?, index: r.u32()? },
            0x0C => Inventory45::BoosterEnable { dict: r.u16()? },
            0x0D => Inventory45::BoosterDisable { dict: r.u16()? },
            0x0E => Inventory45::BoosterReplace { old: r.u16()?, new: r.u16()? },
            0x10 => {
                let items = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                Inventory45::ModuleUpgrade { items, pay: r.u8()? }
            }
            0x11 => Inventory45::Skin { item: r.u32()?, skin: r.u16()? },
            0x12 => Inventory45::SkinPreview { item: r.u32()?, skin: r.u16()? },
            _ => return Err(Error::Unsupported("45 sub")),
        })
    }

    fn shop(r: &mut Reader) -> Result<Shop46> {
        Ok(match r.u8()? {
            0 => Shop46::Buy { dict: r.u16()?, count: r.u32()?, slot: r.u8()?, profile: r.u32()?, flag: r.u8()? },
            1 => Shop46::Sell { item: r.u32()?, amount: r.u32()? },
            2 => Shop46::BuyOffer { offer: r.u32()?, a: r.u32()?, b: r.u32()? },
            3 => Shop46::Exchange { amount: r.u32()? },
            6 => Shop46::OpenChest { item: r.u32()? },
            7 => Shop46::ItemSearch { a: r.u8()? },
            8 => Shop46::FavoriteDiscount { a: r.u32()?, b: r.u8()? },
            9 => Shop46::BuySpecific { dict: r.u16()? },
            _ => return Err(Error::Unsupported("46 sub")),
        })
    }

    fn account(r: &mut Reader) -> Result<Account48> {
        Ok(match r.u8()? {
            0 => Account48::ProfileRename { profile: r.u32()?, name: r.str()? },
            1 => Account48::EnableProfile { profile: r.u32()?, enable: r.u8()? },
            2 => Account48::ProfileChanged { profile: r.u32()? },
            3 => Account48::BuyPreset { a: r.u8()? },
            4 => Account48::NicknameCheck { name: r.str()?, x: r.u32()? },
            5 => Account48::NicknameChange { name: r.str()?, a: r.u8()?, b: r.u32()? },
            6 => Account48::Comment { player: r.u64()?, kind: r.u8()?, a: r.u8()?, b: r.u32()? },
            7 => Account48::ReserveClanName { name: r.str()?, abbr: r.str()? },
            8 => Account48::CreateClan,
            9 => Account48::ClanNameCheck { name: r.str()? },
            0x0A => Account48::ClanAbbrCheck { abbr: r.str()? },
            0x0B => Account48::ImportantMessageRead { id: r.u64()?, answer: r.u8()? },
            0x0C => Account48::ClanInviteSent { player: r.u64()? },
            0x0D => Account48::BuyClanRegistration { amount: r.u32()? },
            0x0E => Account48::LeaveClan,
            0x0F => Account48::ClanWindowOpened { a: r.u8()? },
            0x10 => Account48::ClanInvite { player: r.u64()? },
            0x11 => Account48::ClanPermission { rank: r.u8()?, value: r.u8()?, permission: r.u8()? },
            0x12 => Account48::RankDown { member: r.u64()?, a: r.u8()? },
            0x13 => Account48::RankUp { member: r.u64()? },
            0x14 => Account48::Kick { member: r.u64()? },
            0x15 => Account48::CreateRank { name: r.str()? },
            0x16 => Account48::RemoveRank { rank: r.u8()? },
            0x17 => Account48::DailyMessage { text: r.str()? },
            0x26 => Account48::ClanData,
            0x28 => Account48::ActionItemExchanged { a: r.u16()?, b: r.u16()? },
            0x29 => Account48::BoosterAutobuy { dict: r.u16()?, on: r.u8()? },
            0x2B => Account48::HeroOfTheDay { a: r.u32()?, b: r.u8()? },
            0x2C => Account48::DayHeroData { a: r.u8()?, b: r.u32()? },
            _ => return Err(Error::Unsupported("48 sub")),
        })
    }

    /// `(id, payload after id x)`.
    pub fn encode(&self) -> Result<(u8, Vec<u8>)> {
        let mut w = Writer::new();
        let mid = self.encode_into(&mut w);
        Ok((mid, w.finish()?))
    }

    fn encode_into(&self, w: &mut Writer) -> u8 {
        match self {
            Request::SignIn { session, a, b } => {
                w.u32(*session).u8(*a).u8(*b);
                id::SIGN_IN
            }
            Request::Queue { global, arg, mode, flag } => {
                w.u32(*global).u8(*arg).u32(*mode).u8(*flag);
                id::QUEUE
            }
            Request::QueueCancel { order } => {
                w.u32(*order);
                id::QUEUE_CANCEL
            }
            Request::Info32 { a, b } => {
                w.u8(*a).u8(*b);
                id::INFO_32
            }
            Request::MemberProfile { a, member, profile } => {
                w.u8(*a).u64(*member).u32(*profile);
                id::MEMBER_PROFILE
            }
            Request::Generic { id: i, a } => {
                w.u8(*a);
                *i
            }
            Request::VictoryData { a, player, b } => {
                w.u8(*a).u64(*player).u32(*b);
                id::VICTORY_DATA
            }
            Request::Inventory(i) => {
                Self::enc_inv(i, w);
                id::INVENTORY
            }
            Request::Shop(s) => {
                Self::enc_shop(s, w);
                id::SHOP
            }
            Request::Skills { skills } => {
                w.u8(0).count_u8(skills.len(), "skills");
                for &(i, p) in skills {
                    w.u16(i).u8(p);
                }
                id::SKILLS
            }
            Request::SkillReset { branch, payment } => {
                w.u8(1).u8(*branch).u8(*payment);
                id::SKILLS
            }
            Request::Account(a) => {
                Self::enc_account(a, w);
                id::ACCOUNT
            }
            Request::Squad(s) => {
                match s {
                    Squad49::Disband => w.u8(1),
                    Squad49::Accepted { player } => w.u8(2).u64(*player),
                    Squad49::Kick { player } => w.u8(3).u64(*player),
                    Squad49::Leave => w.u8(4),
                    Squad49::PveState { state, zero } => w.u8(5).u8(*state).u8(*zero),
                };
                id::SQUAD
            }
            Request::Quest(q) => {
                match q {
                    Quest4A::SetFaction { faction } => w.u8(0).u8(*faction),
                    Quest4A::Sub01 { a } => w.u8(1).u8(*a),
                    Quest4A::SkipTutorial => w.u8(2),
                    Quest4A::DailyReroll { quest } => w.u8(3).u32(*quest),
                    Quest4A::ChestOpened { a } => w.u8(4).u8(*a),
                    Quest4A::FactionBattleRewards { a } => w.u8(5).u8(*a),
                };
                id::QUESTS
            }
            Request::League(l) => {
                match l {
                    League4B::Opened => w.u8(1),
                    League4B::Ack => w.u8(2),
                    League4B::History { public_id } => w.u8(4).u64(*public_id),
                    League4B::TopPlayers => w.u8(5),
                };
                id::LEAGUE
            }
            Request::Chat { channel, target, text } => {
                w.u8(*channel);
                if *channel == 4 {
                    w.u64(*target);
                } else {
                    w.u32(*target as u32);
                }
                w.str_max(text, 255);
                id::CHAT
            }
            Request::People { id: i, kind, text } => {
                w.u64(*i).u8(*kind).str_max(text, 255);
                id::PEOPLE
            }
            Request::Friends(f) => {
                match f {
                    Friends4E::Remove { id } => w.u8(1).u64(*id),
                    Friends4E::AddIgnore { id } => w.u8(2).u64(*id),
                    Friends4E::RemoveIgnored { id } => w.u8(3).u64(*id),
                    Friends4E::Find { query } => w.u8(4).str(query),
                };
                id::FRIENDS
            }
            Request::MessagingSignIn { a, b, c, d } => {
                w.u32(*a);
                for v in b {
                    w.u32(*v);
                }
                w.u8(*c).u8(*d);
                id::MESSAGING_SIGN_IN
            }
            Request::SystemInfo { blob } => {
                w.bytes(blob);
                id::SYSTEM_INFO
            }
            Request::ReadMessage { id: i, a } => {
                w.u32(*i).u8(*a);
                id::READ_MESSAGE
            }
            Request::Events(ev) => {
                w.count_u32(ev.len());
                for e in ev {
                    w.u16(e.id).u16(e.x).u32(e.value).u32(e.time);
                }
                id::EVENTS
            }
            Request::Achievements(a) => {
                match a {
                    Achievements55::Ack { a } => w.u8(0).u32(*a),
                    Achievements55::WindowOpened => w.u8(4),
                };
                id::ACHIEVEMENTS
            }
            Request::Presets(p) => {
                match p {
                    Presets58::Create { name, profile } => w.u8(0).str(name).u32(*profile),
                    Presets58::Remove { preset } => w.u8(1).u32(*preset),
                    Presets58::Rename { preset, name } => w.u8(2).u32(*preset).str(name),
                    Presets58::Load { profile, preset } => w.u8(3).u32(*profile).u32(*preset),
                    Presets58::Save { profile, preset } => w.u8(4).u32(*profile).u32(*preset),
                };
                0x58
            }
            Request::Roulette(p) => {
                match p {
                    Roulette5A::Sub2 { a, hunt } => {
                        w.u8(2).u8(*a);
                        if let Some(h) = hunt {
                            w.u32(*h);
                        }
                        &mut *w
                    }
                    Roulette5A::Sub3 { a, b } => w.u8(3).u8(*a).u8(*b),
                };
                0x5A
            }
            Request::Retry56 { pings } => {
                w.count_u8(pings.len(), "56 pings");
                for (id, ms) in pings {
                    w.u8(*id).u16(*ms);
                }
                0x56
            }
            Request::ServerList { a, b } => {
                w.u32(*a).u32(*b);
                0x57
            }
            Request::ActionsData { a, b, c, d } => {
                w.u8(*a).u32(*b).u8(*c).u8(*d);
                0x5C
            }
            Request::ActionBuy { id, mode } => {
                w.u8(1).u32(*id).u8(*mode);
                0x5C
            }
            Request::DailyReward { a } => {
                w.u32(*a);
                0x5E
            }
            Request::Play(p) => {
                match p {
                    Play5B::Play { map, mode, weather } => w.u8(1).u32(*map).u32(*mode).u32(*weather),
                    Play5B::Load => w.u8(2),
                    Play5B::ShootingRange { weapon, tail } => {
                        w.u8(3);
                        weapon.encode(w);
                        w.u8(*tail)
                    }
                };
                id::PLAY
            }
            Request::Pve(p) => {
                match p {
                    Pve5F::Settings { map, difficulty } => w.u8(0).u32(*map).u32(*difficulty),
                    Pve5F::Start { solo, map, difficulty } => w.u8(if *solo { 2 } else { 1 }).u32(*map).u32(*difficulty),
                    Pve5F::Top100 => w.u8(3),
                    Pve5F::CloseStats => w.u8(4),
                    Pve5F::UnlockRewards => w.u8(5),
                    Pve5F::RerollRewards => w.u8(6),
                };
                id::PVE
            }
            Request::Workshop(k) => {
                match k {
                    Workshop60::Opened { dict } => {
                        w.u8(0).u16(*dict);
                    }
                    Workshop60::Save { weapon, weapon_dict, actions } => {
                        w.u8(1).u32(*weapon).u16(*weapon_dict).count_u32(actions.len());
                        for a in actions {
                            w.u8(a.kind).u32(a.f24).u32(a.module_uid).u16(a.module_dict);
                            if a.kind == 0 || a.kind == 2 {
                                w.str(a.slot.as_deref().unwrap_or(""));
                            }
                            w.u16(a.cost).u8(a.f2d).u8(a.bound);
                        }
                    }
                }
                id::WORKSHOP
            }
            Request::Other { id: i, payload } => {
                w.bytes(payload);
                *i
            }
        }
    }

    fn enc_inv(i: &Inventory45, w: &mut Writer) {
        match i {
            Inventory45::Equip { profile, entries, c1, c2 } => {
                w.u8(0).u32(*profile).count_u8(entries.len(), "equip entries");
                for e in entries {
                    w.u32(e.item).u16(e.dict).u8(e.from).u8(e.to).u32(e.count).u32(e.x);
                }
                w.u32(*c1).u32(*c2);
            }
            Inventory45::RelocateUpgrade { a, b, c, d } => {
                w.u8(1).u32(*a).u32(*b).u8(*c).u64(*d);
            }
            Inventory45::DropUpgrade { a, b, c } => {
                w.u8(2).u32(*a).u32(*b).u8(*c);
            }
            Inventory45::Use { item } => {
                w.u8(3).u32(*item);
            }
            Inventory45::Decal { a, b, c, d } => {
                w.u8(4).u32(*a).u32(*b).u64(*c).u8(*d);
            }
            Inventory45::Reroll { item, group, grade, pay } => {
                w.u8(5).u32(*item).u16(*group).u16(*grade).u8(*pay);
            }
            Inventory45::RerollPick { item, old_group, old_grade, row, gold } => {
                w.u8(6).u32(*item).u16(*old_group).u16(*old_grade).u32(*row).u32(*gold);
            }
            Inventory45::Craft { item, pay } => {
                w.u8(7).u32(*item).u8(*pay);
            }
            Inventory45::CraftPick { item, index } => {
                w.u8(8).u32(*item).u32(*index);
            }
            Inventory45::BoosterEnable { dict } => {
                w.u8(0x0C).u16(*dict);
            }
            Inventory45::BoosterDisable { dict } => {
                w.u8(0x0D).u16(*dict);
            }
            Inventory45::BoosterReplace { old, new } => {
                w.u8(0x0E).u16(*old).u16(*new);
            }
            Inventory45::ModuleUpgrade { items, pay } => {
                w.u8(0x10);
                for v in items {
                    w.u32(*v);
                }
                w.u8(*pay);
            }
            Inventory45::Skin { item, skin } => {
                w.u8(0x11).u32(*item).u16(*skin);
            }
            Inventory45::SkinPreview { item, skin } => {
                w.u8(0x12).u32(*item).u16(*skin);
            }
        }
    }

    fn enc_shop(s: &Shop46, w: &mut Writer) {
        match s {
            Shop46::Buy { dict, count, slot, profile, flag } => {
                w.u8(0).u16(*dict).u32(*count).u8(*slot).u32(*profile).u8(*flag);
            }
            Shop46::Sell { item, amount } => {
                w.u8(1).u32(*item).u32(*amount);
            }
            Shop46::BuyOffer { offer, a, b } => {
                w.u8(2).u32(*offer).u32(*a).u32(*b);
            }
            Shop46::Exchange { amount } => {
                w.u8(3).u32(*amount);
            }
            Shop46::OpenChest { item } => {
                w.u8(6).u32(*item);
            }
            Shop46::ItemSearch { a } => {
                w.u8(7).u8(*a);
            }
            Shop46::FavoriteDiscount { a, b } => {
                w.u8(8).u32(*a).u8(*b);
            }
            Shop46::BuySpecific { dict } => {
                w.u8(9).u16(*dict);
            }
        }
    }

    fn enc_account(a: &Account48, w: &mut Writer) {
        match a {
            Account48::ProfileRename { profile, name } => {
                w.u8(0).u32(*profile).str(name);
            }
            Account48::EnableProfile { profile, enable } => {
                w.u8(1).u32(*profile).u8(*enable);
            }
            Account48::ProfileChanged { profile } => {
                w.u8(2).u32(*profile);
            }
            Account48::BuyPreset { a } => {
                w.u8(3).u8(*a);
            }
            Account48::NicknameCheck { name, x } => {
                w.u8(4).str(name).u32(*x);
            }
            Account48::NicknameChange { name, a, b } => {
                w.u8(5).str(name).u8(*a).u32(*b);
            }
            Account48::Comment { player, kind, a, b } => {
                w.u8(6).u64(*player).u8(*kind).u8(*a).u32(*b);
            }
            Account48::ReserveClanName { name, abbr } => {
                w.u8(7).str(name).str(abbr);
            }
            Account48::CreateClan => {
                w.u8(8);
            }
            Account48::ClanNameCheck { name } => {
                w.u8(9).str(name);
            }
            Account48::ClanAbbrCheck { abbr } => {
                w.u8(0x0A).str(abbr);
            }
            Account48::ImportantMessageRead { id, answer } => {
                w.u8(0x0B).u64(*id).u8(*answer);
            }
            Account48::BuyClanRegistration { amount } => {
                w.u8(0x0D).u32(*amount);
            }
            Account48::LeaveClan => {
                w.u8(0x0E);
            }
            Account48::ClanInviteSent { player } => {
                w.u8(0x0C).u64(*player);
            }
            Account48::ClanWindowOpened { a } => {
                w.u8(0x0F).u8(*a);
            }
            Account48::Kick { member } => {
                w.u8(0x14).u64(*member);
            }
            Account48::ClanInvite { player } => {
                w.u8(0x10).u64(*player);
            }
            Account48::ClanPermission { rank, value, permission } => {
                w.u8(0x11).u8(*rank).u8(*value).u8(*permission);
            }
            Account48::RankDown { member, a } => {
                w.u8(0x12).u64(*member).u8(*a);
            }
            Account48::RankUp { member } => {
                w.u8(0x13).u64(*member);
            }
            Account48::CreateRank { name } => {
                w.u8(0x15).str(name);
            }
            Account48::RemoveRank { rank } => {
                w.u8(0x16).u8(*rank);
            }
            Account48::DailyMessage { text } => {
                w.u8(0x17).str(text);
            }
            Account48::ClanData => {
                w.u8(0x26);
            }
            Account48::ActionItemExchanged { a, b } => {
                w.u8(0x28).u16(*a).u16(*b);
            }
            Account48::BoosterAutobuy { dict, on } => {
                w.u8(0x29).u16(*dict).u8(*on);
            }
            Account48::HeroOfTheDay { a, b } => {
                w.u8(0x2B).u32(*a).u8(*b);
            }
            Account48::DayHeroData { a, b } => {
                w.u8(0x2C).u8(*a).u32(*b);
            }
        }
    }
}
