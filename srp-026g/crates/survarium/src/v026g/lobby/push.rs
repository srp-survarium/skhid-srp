//! The lobby's core server→client messages: sign-in, the account, the
//! inventory, the economy, and the answers to the client's actions
//! (`lobby.md` §2, `lobby_requests.md` §4).

use std::num::NonZeroU16;

use super::super::dictionary::Dictionary;
use super::super::lzma2;
use super::super::modifications::Modifications;
use super::super::prices::PriceItem;
use super::super::skills::{MODIFIERS, write_modifier_block};
use super::{
    FIRST_CLIENT_ID, LAST_CLIENT_ID, ServerId, ServerMessage, WireError, Writer, at_most, limits,
};

/// The condition of an item that is not worn at all: the client shows
/// `round(v * 0.0001)`, so 1 000 000 is 100 (`lobby.md` §2.6; the client
/// itself writes this value on a repair and a purchase).
pub const FULL_CONDITION: u32 = 1_000_000;
/// One point of shown condition.
pub const CONDITION_UNIT: u32 = 10_000;

/// A kit the player may use: the lobby works its hint out from the slots,
/// and a match loads a player for it (`static_info.md` §3.1). PROVEN.
pub const KIT_READY: u8 = 0;
/// A kit only a premium account may use: the lobby shows it with the hint
/// `st_premium_profile_hint` ("Available with premium account") and skips
/// its own checks of the kit (`ProfileSelector.setupProfiles`,
/// `lobby_menu.swf`; exe 0x22201B). PROVEN. It is also the constructor's
/// default. srp sends it for no kit: every kit is every account's
/// (`session::v026g::Account::kit_usable`, since 2026-10-17).
pub const KIT_PREMIUM: u8 = 2;

/// `inventory_item_descr::modification`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Modification {
    pub id: u32,
    /// Clear: a modification, which counts towards the item's rank. Set: a
    /// defect, which the lobby shows as damage to repair. The client looks
    /// the id up in the table of that kind and crashes if it is not there
    /// (`lobby.md` §2.6; `super::super::modifications`).
    pub state: bool,
}

/// `survarium::upgrade`: `u32 id, u32, u64, u8 param` (`lobby.md` §2.6):
/// a camouflage or decal on the item. `id` is the instance id of the
/// cosmetic, which stays in the inventory; `a` its dict id (the client
/// fills it so when it attaches one, 0x275671); `value` what the decal
/// editor sets (`46 05`); `param` the slot, 0 and 1 for decals and 3 for
/// the camouflage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Upgrade {
    pub id: u32,
    pub a: u32,
    pub value: u64,
    pub param: u8,
}

/// `inventory_item_descr` (`lobby.md` §2.6): one item instance.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ItemDescr {
    /// The instance id: what the client finds an item by.
    pub item_id: u32,
    pub dict_id: u16,
    /// A stack's count, or an item's condition × 10 000.
    pub condition_or_stack: u32,
    /// Only on the wire for an item that is no stack (and only in the
    /// lobby's form of the descriptor). Meaning LIKELY from the UI's names
    /// (`repair_time`, `reroll_time`); srp sends 0.
    pub repair_time: u32,
    pub reroll_time: u32,
    pub modifications: Vec<Modification>,
    pub upgrades: Vec<Upgrade>,
}

impl ItemDescr {
    /// An item with no times, modifications or upgrades.
    pub fn plain(item_id: u32, dict_id: u16, condition_or_stack: u32) -> Self {
        Self {
            item_id,
            dict_id,
            condition_or_stack,
            repair_time: 0,
            reroll_time: 0,
            modifications: Vec::new(),
            upgrades: Vec::new(),
        }
    }
}

/// Which form of the descriptor a message carries (the reader's last
/// argument, 0x1F1350).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DescrMode {
    /// Mode 0: every lobby reader but the results' dropped item.
    Lobby,
    /// Mode 1: no `repair_time`.
    Brief,
}

/// Writes a descriptor whose `dict_id` the dictionary has.
pub(crate) fn write_item(
    writer: &mut Writer,
    item: &ItemDescr,
    mode: DescrMode,
) -> Result<(), WireError> {
    let entry = Dictionary::shared()
        .item(item.dict_id)
        .ok_or(WireError::UnknownItem(item.dict_id))?;
    writer.u32(item.item_id).u16(item.dict_id);
    write_item_tail(writer, item, entry.is_stack, mode)
}

/// What follows `item_id` and `dict_id` (0x1F13D0).
pub(crate) fn write_item_tail(
    writer: &mut Writer,
    item: &ItemDescr,
    is_stack: bool,
    mode: DescrMode,
) -> Result<(), WireError> {
    at_most(
        "item modifications",
        item.modifications.len(),
        limits::MODIFICATIONS,
    )?;
    at_most("item upgrades", item.upgrades.len(), limits::UPGRADES)?;
    writer.u32(item.condition_or_stack);
    if !is_stack {
        if mode == DescrMode::Lobby {
            writer.u32(item.repair_time);
        }
        writer.u32(item.reroll_time);
    }
    writer.u8(item.modifications.len() as u8);
    let table = Modifications::shared();
    for modification in &item.modifications {
        if !table.is_known(modification.id, modification.state) {
            return Err(WireError::UnknownModification {
                id: modification.id,
                state: modification.state,
            });
        }
        writer.u32(modification.id).bool(modification.state);
    }
    writer.u8(item.upgrades.len() as u8);
    for upgrade in &item.upgrades {
        writer
            .u32(upgrade.id)
            .u32(upgrade.a)
            .u64(upgrade.value)
            .u8(upgrade.param);
    }
    Ok(())
}

/// What a kit slot holds: an index into the account's item table (not an
/// item id) and how many of it the kit takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KitSlot {
    pub item_index: u16,
    /// The lobby shows it as the slot's count for a stack; a match skips a
    /// slot whose count is 0 and clamps it to what the item table's entry
    /// holds (`static_info.md` §3.1). 1 for an item that is no stack.
    pub count: u16,
}

/// One kit (the client's "profile") of an account body.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kit {
    /// [`KIT_READY`] for a kit the player may use.
    pub state: u8,
    /// Non-zero, distinct among the account's kits.
    pub id: u32,
    /// Empty shows the client's own default name for the kit's index.
    pub name: String,
    pub slots: [Option<KitSlot>; limits::KIT_SLOTS],
    /// The 68 player modifiers; a 0 is left out of the wire.
    #[serde(with = "modifiers_serde")]
    pub modifiers: [f32; MODIFIERS],
}

/// serde for a kit's modifiers: a list of exactly [`MODIFIERS`] numbers
/// (serde's own arrays end at 32 elements).
mod modifiers_serde {
    use super::MODIFIERS;

    pub fn serialize<S: serde::Serializer>(
        modifiers: &[f32; MODIFIERS],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&modifiers[..], serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[f32; MODIFIERS], D::Error> {
        let list: Vec<f32> = serde::Deserialize::deserialize(deserializer)?;
        list.try_into().map_err(|list: Vec<f32>| {
            serde::de::Error::invalid_length(list.len(), &"a kit's 68 modifiers")
        })
    }
}

impl Kit {
    pub fn empty(id: u32) -> Self {
        Self {
            state: KIT_READY,
            id,
            name: String::new(),
            slots: [None; limits::KIT_SLOTS],
            modifiers: [0.0; MODIFIERS],
        }
    }
}

/// The account body (`deserialize` 0x1F1070 and 0x1F1120; `lobby.md` §2.5):
/// what 0x87, 0x88 and 0x93 share with the match's static info.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AccountBody {
    /// `game_team_id` (PROVEN in a match; the lobby's use is not known).
    pub team: u8,
    pub name: String,
    /// The clan abbreviation, at most 14 bytes.
    pub clan_tag: String,
    /// New-player protection: the damage factor is `max(0, 1 - 0.1 * v)`,
    /// so 10 or more is none (`static_info.md` §3). PROVEN.
    pub protection: u32,
    pub kits: Vec<Kit>,
    /// The item table the kits' slots index: at most 69.
    pub items: Vec<ItemDescr>,
}

/// No new-player protection.
pub const NO_PROTECTION: u32 = 10;

pub(crate) fn write_account_body(writer: &mut Writer, body: &AccountBody) -> Result<(), WireError> {
    at_most("kits", body.kits.len(), limits::KITS)?;
    at_most("account items", body.items.len(), limits::ACCOUNT_ITEMS)?;
    writer
        .u8(body.team)
        .str(&body.name, limits::NAME)
        .str(&body.clan_tag, limits::ACCOUNT_CLAN_TAG)
        .u32(body.protection)
        .u8(body.kits.len() as u8)
        .u8(body.items.len() as u8);
    for kit in &body.kits {
        writer
            .u8(kit.state)
            .u32(kit.id)
            .str(&kit.name, limits::NAME);
        let mask = kit
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.is_some())
            .fold(0_u32, |mask, (index, _)| mask | 1 << index);
        writer.u32(mask);
        for slot in kit.slots.iter().flatten() {
            if usize::from(slot.item_index) >= body.items.len() {
                return Err(WireError::BadItemIndex {
                    index: slot.item_index,
                    items: body.items.len(),
                });
            }
            writer.u16(slot.item_index).u16(slot.count);
        }
        let mut block = Vec::new();
        write_modifier_block(&kit.modifiers, &mut block);
        writer.bytes(&block);
    }
    for item in &body.items {
        write_item(writer, item, DescrMode::Lobby)?;
    }
    Ok(())
}

/// What 0x87 carries: the player's own account.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnAccount {
    pub body: AccountBody,
    /// One for each kit, in order (LIKELY a revision: 0.21d's P+0x9DC).
    pub revisions: Vec<u32>,
    /// The id of the kit the player plays; the client selects the kit with
    /// this id.
    pub active_kit: u32,
    /// The two boxes of the kit window: one byte on the wire, bit 0
    /// autobuy and bit 1 auto-repair (the client hands the byte to
    /// `root.set_autobuy` as a number, 0x227760; `LobbyMenu.set_autobuy`
    /// ticks autobuy for 1 and 3, auto-repair for 2 and 3. PROVEN).
    pub autobuy: bool,
    pub auto_repair: bool,
    /// The account's level: also the skill points it has unlocked.
    pub level: u8,
    pub exp_total: u32,
    pub exp_level_start: u32,
    pub exp_next_level: u32,
}

/// `80`: the lobby has signed the client in (`lobby.md` §2.1). The client
/// only echoes `flag` in its `32 00 <flag>`; send state 0 and no flag.
pub fn signed_in(client_state: u8, flag: bool) -> ServerMessage {
    Writer::new()
        .u8(client_state)
        .bool(flag)
        .done(ServerId::SignedIn)
}

/// `82`: go to the match server (`lobby.md` §2.2). `team` is 0 or 1.
/// `match_session` is what the client then sends the match server as its
/// session (0xD8, 0xD9); its login session is not used there.
pub fn connect_to_match(
    host: &str,
    port: u16,
    match_id: u32,
    team: u8,
    match_session: u32,
) -> ServerMessage {
    Writer::new()
        .str(host, limits::HOST)
        .u16(port)
        .u32(match_id)
        .u8(team)
        .u32(match_session)
        .done(ServerId::ConnectToMatch)
}

fn permitted_writer(op: u8) -> Result<Writer, WireError> {
    if !(FIRST_CLIENT_ID..=LAST_CLIENT_ID).contains(&op) {
        return Err(WireError::BadValue("an op that is no client id"));
    }
    let mut writer = Writer::new();
    writer.u8(op);
    Ok(writer)
}

fn permitted_event(op: u8, event: u8) -> Writer {
    let mut writer = Writer::new();
    writer.u8(op).u8(event);
    writer
}

/// `83 <op>` with no body. The client acts on it only for 0x30 (clan play:
/// it sends `49 20`/`49 21`) and 0x4B (it asks for its quests and services
/// again); for any other op it reads nothing (`lobby_requests.md` §4.1).
pub fn permitted(op: u8) -> Result<ServerMessage, WireError> {
    Ok(permitted_writer(op)?.done(ServerId::Permitted))
}

/// `83 4B`: the quest faction was changed; the client then sends `3C 00`
/// and `3E 00`.
pub fn permitted_quest_faction() -> ServerMessage {
    let mut writer = Writer::new();
    writer
        .u8(super::requests::id::QUEST)
        .done(ServerId::Permitted)
}

/// `84 <op> 00 <text>`: refused (`lobby_requests.md` §4.2). `text` is a
/// localization key (the client localizes it, then shows it in a message
/// box for 0x2F, 0x30 and 0x46..=0x49; `st_not_logged_id` for 0x2F/0x30
/// signs the client out; a denial of 0x4A is silent).
pub fn denied(op: u8, text: &str) -> ServerMessage {
    Writer::new()
        .u8(op)
        .u8(0)
        .str(text, limits::TEXT)
        .done(ServerId::Denied)
}

/// `client_state_enum` and what 0x85 carries for each (`lobby.md` §2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientState {
    /// In the lobby: the only state in which Play is enabled.
    Lobby,
    InQueue {
        order_id: u32,
        /// 0 shows "match found".
        place: u32,
        queue_total: u32,
        current_wait_seconds: u32,
        average_wait_seconds: u32,
        /// The UI shows `current_wait / search_step` as the search range:
        /// 0 is a divide by zero in the client, hence the type.
        search_step: NonZeroU16,
        /// Entry `i` is shown as `{level: i, value}`.
        search_properties: [u16; limits::SEARCH_PROPERTIES],
    },
    InMatch {
        order_id: u32,
        match_id: u32,
        team: u8,
    },
}

/// `85`: the client's state, and an optional status text (read only if
/// bytes remain; at most 127).
pub fn client_state(state: &ClientState, status_text: Option<&str>) -> ServerMessage {
    let mut writer = Writer::new();
    match *state {
        ClientState::Lobby => {
            writer.u8(0);
        }
        ClientState::InQueue {
            order_id,
            place,
            queue_total,
            current_wait_seconds,
            average_wait_seconds,
            search_step,
            search_properties,
        } => {
            writer
                .u8(1)
                .u32(order_id)
                .u32(place)
                .u32(queue_total)
                .u32(current_wait_seconds)
                .u32(average_wait_seconds)
                .u16(search_step.get());
            for property in search_properties {
                writer.u16(property);
            }
        }
        ClientState::InMatch {
            order_id,
            match_id,
            team,
        } => {
            writer.u8(2).u32(order_id).u32(match_id).u8(team);
        }
    }
    if let Some(text) = status_text {
        writer.str(text, limits::STATUS_TEXT);
    }
    writer.done(ServerId::ClientState)
}

/// `87`: the player's own account (`lobby.md` §2.7). **Every item of its
/// table must already be in the client's inventory (0x89), or the client
/// crashes**: send [`inventory`] first, after every change.
pub fn own_account(account: &OwnAccount) -> Result<ServerMessage, WireError> {
    if account.revisions.len() != account.body.kits.len() {
        return Err(WireError::BadValue("one revision for each kit"));
    }
    let mut writer = Writer::new();
    write_account_body(&mut writer, &account.body)?;
    for &revision in &account.revisions {
        writer.u32(revision);
    }
    writer
        .u32(account.active_kit)
        .u8(auto_state(account.autobuy, account.auto_repair))
        .u8(account.level)
        .u32(account.exp_total)
        .u32(account.exp_level_start)
        .u32(account.exp_next_level);
    Ok(writer.done(ServerId::Account))
}

/// `88`: a squad mate's account, the answer to `34 00 <account> <kit>`
/// (`lobby.md` §2.8).
pub fn squad_member_account(body: &AccountBody, revision: u32) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    write_account_body(&mut writer, body)?;
    writer.u32(revision);
    Ok(writer.done(ServerId::SquadMemberAccount))
}

/// What the server paid from the player's money after a match, the tail
/// of `89` (`lobby.md` §2.9; client 0x26EF40): three lists of `(silver,
/// dict id)`, shown on the results screen as "Ammunition", "Special
/// equipment" and "Repair cost" (`VictoryScreen.setSpendMoneyDetailed`).
/// All empty unless the server really took that money.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpentMoney {
    pub autobuy_ammo: Vec<(u32, u16)>,
    pub autobuy_consumables: Vec<(u32, u16)>,
    pub repair: Vec<(u32, u16)>,
}

/// One list of [`SpentMoney`]: what it is, its rows, and the most the
/// client's buffer holds.
pub type SpentList<'a> = (&'static str, &'a [(u32, u16)], usize);

impl SpentMoney {
    /// The lists in the order the client reads them, each with the size of
    /// the client's buffer for it.
    pub fn lists(&self) -> [SpentList<'_>; 3] {
        [
            (
                "autobuy ammo rows",
                &self.autobuy_ammo,
                limits::SPENT_AUTOBUY_AMMO,
            ),
            (
                "autobuy consumable rows",
                &self.autobuy_consumables,
                limits::SPENT_AUTOBUY_CONSUMABLES,
            ),
            ("repair rows", &self.repair, limits::SPENT_REPAIR),
        ]
    }
}

/// `89`: everything the account owns, with nothing spent after a match
/// (`lobby.md` §2.9). The message ends with the three empty lists of
/// [`SpentMoney`]: `00 00 00`.
pub fn inventory(items: &[ItemDescr]) -> Result<ServerMessage, WireError> {
    inventory_with_spent(items, &SpentMoney::default())
}

/// `89` with the money the server spent for the player after a match: the
/// client reads `u32 n`, n items, then three times `u8 count`, count ×
/// {`u32 silver`, `u16 dict_id`} (PROVEN: 0x270534 → 0x26EF40).
pub fn inventory_with_spent(
    items: &[ItemDescr],
    spent: &SpentMoney,
) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    writer.u32(items.len() as u32);
    for item in items {
        write_item(&mut writer, item, DescrMode::Lobby)?;
    }
    for (what, rows, most) in spent.lists() {
        at_most(what, rows.len(), most)?;
        writer.u8(rows.len() as u8);
        for &(value, dict_id) in rows {
            writer.u32(value).u16(dict_id);
        }
    }
    Ok(writer.done(ServerId::Inventory))
}

/// `8A`: the price list (`lobby.md` §2.10): `u16 n`, `u32 packed_size` and
/// a raw LZMA2 stream that unpacks to exactly `n × 8` bytes. With no
/// records the client reads nothing after `n`.
pub fn prices(records: &[PriceItem]) -> Result<ServerMessage, WireError> {
    at_most("price records", records.len(), u16::MAX.into())?;
    let mut writer = Writer::new();
    writer.u16(records.len() as u16);
    if !records.is_empty() {
        let unpacked: Vec<u8> = records
            .iter()
            .flat_map(|record| record.to_bytes())
            .collect();
        let packed = lzma2::stored(&unpacked);
        writer.u32(packed.len() as u32).bytes(&packed);
    }
    Ok(writer.done(ServerId::Prices))
}

/// `8B`: the account's silver and gold (`lobby.md` §2.11). The client never
/// asks: push it after every change.
pub fn money(generic: u32, premium: u32) -> ServerMessage {
    Writer::new()
        .u32(generic)
        .u32(premium)
        .done(ServerId::Money)
}

/// `8C`: the account's level, experience and skills as `(skill id,
/// points)` (`lobby.md` §2.12). Also the only answer to `48` and `37`.
pub fn level_and_skills(
    level: u8,
    exp_total: u32,
    exp_level_start: u32,
    exp_next_level: u32,
    skills: &[(u16, u8)],
) -> Result<ServerMessage, WireError> {
    at_most("skills", skills.len(), u8::MAX.into())?;
    let mut writer = Writer::new();
    writer
        .u8(level)
        .u32(exp_total)
        .u32(exp_level_start)
        .u32(exp_next_level)
        .u8(skills.len() as u8);
    for &(skill_id, points) in skills {
        writer.u16(skill_id).u8(points);
    }
    Ok(writer.done(ServerId::Skills))
}

/// The six prices of 0x8D (`lobby.md` §2.13).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServicePrices {
    /// `root.set_reroll_cost(a, b)`: the skills reset. That the pair is
    /// (silver, gold) is LIKELY (the reset dialog has a box for each).
    pub reroll: u32,
    pub reroll_premium: u32,
    /// `set_profile_cost`: another kit.
    pub profile: u32,
    /// `set_change_name_cost`.
    pub change_name: u32,
    /// `set_premium_exchange_rate`: silver for one gold.
    pub premium_exchange_rate: u32,
    /// `set_clan_creation_cost`.
    pub clan_creation: u32,
}

/// `8D`.
pub fn service_prices(prices: &ServicePrices) -> ServerMessage {
    Writer::new()
        .u32(prices.reroll)
        .u32(prices.reroll_premium)
        .u32(prices.profile)
        .u32(prices.change_name)
        .u32(prices.premium_exchange_rate)
        .u32(prices.clan_creation)
        .done(ServerId::ServicePrices)
}

/// `8E`: `(faction, points)` for each faction, then the faction whose
/// quests the player takes (`lobby.md` §2.14). 0 for that makes the client
/// ask the player to choose one. Each record is `u32 faction, u16, u16
/// points` (the points at +6 as a u16: PROVEN 0x225AB0; the u16 at +4 is
/// not read there).
pub fn reputations(
    reputations: &[(u8, u16)],
    quest_active_faction: u8,
) -> Result<ServerMessage, WireError> {
    // 0xFF is the client's "never received".
    at_most("reputations", reputations.len(), 0xFE)?;
    let mut writer = Writer::new();
    writer.u8(reputations.len() as u8);
    for &(faction, points) in reputations {
        writer.u32(faction.into()).u16(0).u16(points);
    }
    writer.u8(quest_active_faction);
    Ok(writer.done(ServerId::Reputations))
}

/// `8F`: how many players the ranking has, and the account's nickname
/// (`lobby.md` §2.15).
pub fn base_info(ranked_players: u32, nickname: &str) -> ServerMessage {
    Writer::new()
        .u32(ranked_players)
        .str(nickname, limits::NAME)
        .done(ServerId::BaseInfo)
}

/// `90`: the account's own row of the ranking: its place (from 1), rating
/// and the rating before its last match (`lobby.md` §2.16; layout as
/// 0.21d's `36 0B`).
pub fn player_elo(place: u32, rating: u16, rating_before: u16) -> ServerMessage {
    Writer::new()
        .u32(place)
        .u16(rating)
        .u16(rating_before)
        .done(ServerId::PlayerElo)
}

/// A row of the ranking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ranked {
    pub name: String,
    pub rating: u16,
    pub rating_before: u16,
}

/// `91`: the ranking from place `start + 1` on, the answer to
/// `3B 00 <start> <count>`: each row `{name, place, rating, change}`
/// (layout as 0.21d's `36 0C`).
pub fn elo_list(start: u32, rows: &[Ranked]) -> Result<ServerMessage, WireError> {
    let placed: Vec<(u32, &Ranked)> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (start + index as u32 + 1, row))
        .collect();
    elo_rows(&placed)
}

/// `91` with each row's place given.
pub(super) fn elo_rows(rows: &[(u32, &Ranked)]) -> Result<ServerMessage, WireError> {
    at_most("ranking rows", rows.len(), u8::MAX.into())?;
    let mut writer = Writer::new();
    writer.u8(rows.len() as u8);
    for (place, row) in rows {
        let change = i32::from(row.rating) - i32::from(row.rating_before);
        writer
            .str(&row.name, limits::NAME)
            .u32(*place)
            .u16(row.rating)
            .u16(change.clamp(i16::MIN.into(), i16::MAX.into()) as i16 as u16);
    }
    Ok(writer.done(ServerId::EloList))
}

/// The rows of a [`ranking_flush`]: all one `91` holds.
pub const RANKING_FLUSH_ROWS: usize = u8::MAX as usize;
/// The most rows the ranking page asks for at once
/// (`PlayerRanking.request_players`).
pub const RANKING_PAGE_ROWS: usize = 100;

/// A `91` nobody asked for, for a client that has just signed in to the
/// lobby: it empties the ranking page's list of the rows it waits for.
/// `top` is the ranking from place 1; `None` when it is empty.
///
/// The page gives each row of a `91` to the next row index it asked for
/// (`PlayerRanking.fillPlayers`: `_requestList.shift()`), whatever place
/// the row names, and nothing else ever takes an index off that list. A
/// `3B` that was never answered (srp was started again, or the client made
/// its lobby connection anew) leaves its indexes there, and every page
/// after it is filled shifted by that many rows until the game is started
/// again (`client_rules::RankingPage`; seen live 2026-10-12). The client
/// reads any number of rows (0x271EB0: a vector, no limit but the u8) and
/// hands them to the page with no condition (0x2271B0,
/// `LobbyMenu.ranking_set_data`).
///
/// The rows are the top of the ranking as a first page's answer would give
/// them, then the first row again up to [`RANKING_FLUSH_ROWS`]: a lost
/// first-page request is answered rightly by them, rows past the list's
/// end all go to index 0 (`shift()` of an empty list is `undefined`, 0 as
/// an `int`), where the last one, the first row, is the right one. Other
/// indexes may hold a wrong row until the page is opened again, which
/// empties its cache (`onActivate`).
pub fn ranking_flush(top: &[Ranked]) -> Option<Result<ServerMessage, WireError>> {
    let first = top.first()?;
    let mut rows: Vec<(u32, &Ranked)> = top
        .iter()
        .take(RANKING_PAGE_ROWS)
        .enumerate()
        .map(|(index, row)| (index as u32 + 1, row))
        .collect();
    rows.resize(RANKING_FLUSH_ROWS, (1, first));
    Some(elo_rows(&rows))
}

/// Every `start` of a `3B` from here on is srp's own and never a ranking
/// page. The stock page asks only for indexes below the count of ranked
/// players it was told (`PlayerRanking.request_players`), so a stock game
/// never sends one. Only "Player vs Bots" ([`PVB_STATUS_START`],
/// [`PVB_CHOOSE_BASE`]), the rooms (`rooms::ROOMS_FIRST` to
/// `rooms::ROOMS_LAST`) and the replay poll ([`REPLAY_START`]) are
/// answered; every other one gets no answer (the "matches now" list and
/// its JOIN, at 2 000 000 000 and from 2 100 000 000, went with the queue:
/// the owner's of 2026-10-17).
pub const RESERVED_START: u32 = 2_000_000_000;
/// The `start` of a `3B` that polls for the asker's "Watch replay" answer
/// ([`replay_answer`]).
pub const REPLAY_START: u32 = 2_200_000_000;
/// The `player` of a `3D 00 <player> <match>` (`victory_screen_request_
/// player_data`) that asks for the match's replay, from the History of
/// srp's changed lobby movie: `0xFFFF_FF01` as the movie's `uint`. No
/// account id is that high (they are below 2^31), so no row of a results
/// screen asks for it.
pub const REPLAY_ASK: u32 = 0xFFFF_FF01;
/// The `player` of a `3D 00 <player> <place>` that asks for the equipment
/// of the player at that place of the ranking (1 the first), as
/// [`REPLAY_ASK`] asks for a replay: "Show player's information" in the
/// right-click menu of a row of the Rankings window of srp's changed lobby
/// movie (since 2026-10-20). Answered with `93` as a results screen's row is
/// ([`player_details`]), from the account as it is now. A results screen's
/// rows are numbered from 1 by their place in the match: none is this high.
pub const GEAR_BY_PLACE: u32 = 0xFFFF_FF03;
/// The `player` of a `3D 00 <player> <account>` that asks for the
/// equipment of that account, as [`GEAR_BY_PLACE`] asks by a ranking place:
/// a right-click on a member in the Rooms list's side panel (the rooms'
/// member rows give the account, `lobby.md` §2.16.3; since 2026-10-20).
/// Answered only for a member of a room now (one the list shows, its
/// members shown to anyone), so account ids cannot be walked.
pub const GEAR_BY_ACCOUNT: u32 = 0xFFFF_FF02;
/// The `player` of a `3D 00 <player> <place + 20 × check>` that asks for the
/// equipment of the member at that place (0 to 19) of the asker's own room,
/// a right-click on it in the room's window; `check` is the room state's
/// `serial` modulo 495, as a kick's (`rooms::member_gear`), and one from an
/// older list (a member came or went since) gets no answer (since
/// 2026-10-20).
pub const GEAR_IN_ROOM: u32 = 0xFFFF_FF04;
/// The first field of the row of [`replay_answer`].
pub const REPLAY_ANSWER_VERSION: &str = "R1";

/// The `start` of a `3B` that asks whether the asker may play against the
/// server's bots now, and on what ("Player vs Bots"; srp's changed lobby
/// movie asks when its match settings panel opens). Answered with
/// [`pvb_status`]. **srp's own** ([`RESERVED_START`]).
pub const PVB_STATUS_START: u32 = 2_050_000_000;
/// The `start` of a `3B` that chooses a match against the server's bots:
/// `PVB_CHOOSE_BASE + size × 1000 + map_id` ([`pvb_choice`]), answered at
/// the same place with [`pvb_choose_ok`] or [`pvb_choose_no`].
pub const PVB_CHOOSE_BASE: u32 = 2_051_000_000;
/// The last start of a choice: `PVB_CHOOSE_BASE + 999 999`.
pub const PVB_CHOOSE_LAST: u32 = PVB_CHOOSE_BASE + 999_999;
/// The first field of [`pvb_status`]'s header row.
pub const PVB_STATUS_VERSION: &str = "B1";
/// The first field of the row of a choice's answer.
pub const PVB_CHOOSE_VERSION: &str = "B2";
/// The most maps one [`pvb_status`] lists.
pub const PVB_MAP_ROWS: usize = 31;

/// Whether the asker may start a match against the server's bots now, and
/// if not why: the `code` of [`pvb_status`]'s header and of
/// [`pvb_choose_no`]. The numbers are the contract's
/// (`private/pvb-mode/CONTRACT-PVB.md`); the movie holds a text for each
/// in every language and takes a number it does not know as "not now".
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PvbCode {
    Allowed = 0,
    /// The asker's equipment level is over the most allowed: the row's
    /// `gear` and `max` say both.
    GearTooHigh = 1,
    /// In a squad with others.
    InSquad = 2,
    /// Waiting for a match (a clan team's).
    Searching = 3,
    /// In a match, sent to one, or with a place in one.
    InMatch = 4,
    /// The asker's game is too old for this.
    Outdated = 5,
    /// The server does not offer the mode now.
    Off = 6,
    /// As many matches against bots run as the server allows at once: the
    /// row's `running` and `cap` say both.
    AtLimit = 7,
    /// A planned maintenance is 30 minutes away or less.
    Maintenance = 8,
    /// The asker is in its clan's team (clan-team Play).
    ClanTeam = 9,
    /// A choice of a size or a map the server does not offer (only in
    /// [`pvb_choose_no`]).
    UnknownChoice = 10,
}

/// The numbers a refusal is shown with: the asker's equipment level and
/// the most allowed (code 1), the matches against bots running and the
/// most at once (code 7). Sent with every answer, whatever its code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PvbNumbers {
    pub gear: u8,
    pub max_gear: u8,
    /// Running, and held for the players who chose one and have not
    /// pressed Play yet.
    pub running: u32,
    pub max_matches: u32,
}

/// The header of [`pvb_status`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PvbStatus {
    pub code: PvbCode,
    pub numbers: PvbNumbers,
    /// The team sizes offered, each side's players.
    pub sizes: Vec<u8>,
    pub default_size: u8,
    pub default_map: u8,
}

/// A map of [`pvb_status`]: one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PvbMap {
    pub map_id: u8,
    /// `maps::MapMode`'s level and mode.
    pub level: u8,
    pub mode: u8,
    /// How many more matches against bots may start on it now; 0: none.
    pub free: u32,
}

/// `91` as the answer to `3B 00 <PVB_STATUS_START> <count>`. **srp's own**,
/// as [`matches_now`]: ranking rows with rating and change 0.
///
/// - row 0, the header, always there: `place = PVB_STATUS_START`, `name =
///   "B1;<code>;<gear>;<max gear>;<sizes>;<default size>;<default
///   map>;<running>;<max matches>"`, `sizes` joined by `.` (`6.8.10`);
/// - row `i` from 1, a map: `place = PVB_STATUS_START + i`, `name =
///   "<map_id>;<level>;<mode>;<free>"`.
///
/// At most `count` rows (the header always), and [`PVB_MAP_ROWS`] maps.
/// The widest header is 47 bytes, a map 18: within [`limits::NAME`].
pub fn pvb_status(
    status: &PvbStatus,
    maps: &[PvbMap],
    count: u8,
) -> Result<ServerMessage, WireError> {
    at_most("bot maps", maps.len(), PVB_MAP_ROWS)?;
    let row = |name: String| Ranked {
        name,
        rating: 0,
        rating_before: 0,
    };
    let sizes: Vec<String> = status.sizes.iter().map(u8::to_string).collect();
    let numbers = status.numbers;
    let mut rows = vec![row(format!(
        "{PVB_STATUS_VERSION};{};{};{};{};{};{};{};{}",
        status.code as u8,
        numbers.gear,
        numbers.max_gear,
        sizes.join("."),
        status.default_size,
        status.default_map,
        numbers.running,
        numbers.max_matches
    ))];
    let most = usize::from(count).saturating_sub(1);
    rows.extend(maps.iter().take(most).map(|map| {
        row(format!(
            "{};{};{};{}",
            map.map_id, map.level, map.mode, map.free
        ))
    }));
    let placed: Vec<(u32, &Ranked)> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (PVB_STATUS_START + index as u32, row))
        .collect();
    elo_rows(&placed)
}

/// What a choice's start asks for: `(size, map_id)`; `None` for a start
/// that is no choice (outside `PVB_CHOOSE_BASE..=PVB_CHOOSE_LAST`, a size
/// over 255 or a map id over 255). Whether the server offers it is the
/// lobby's to say.
pub fn pvb_choice(start: u32) -> Option<(u8, u8)> {
    if !(PVB_CHOOSE_BASE..=PVB_CHOOSE_LAST).contains(&start) {
        return None;
    }
    let offset = start - PVB_CHOOSE_BASE;
    let size = u8::try_from(offset / 1000).ok()?;
    let map_id = u8::try_from(offset % 1000).ok()?;
    Some((size, map_id))
}

/// The start a choice of `size` and `map_id` is asked with.
pub fn pvb_choice_start(size: u8, map_id: u8) -> u32 {
    PVB_CHOOSE_BASE + u32::from(size) * 1000 + u32::from(map_id)
}

/// `91` as the answer to a choice that was granted: one row at its start,
/// `"B2;ok;<size>;<map_id>"`. A place is held for the asker for a few
/// seconds; the movie then presses Play for it.
pub fn pvb_choose_ok(size: u8, map_id: u8) -> Result<ServerMessage, WireError> {
    let row = Ranked {
        name: format!("{PVB_CHOOSE_VERSION};ok;{size};{map_id}"),
        rating: 0,
        rating_before: 0,
    };
    elo_rows(&[(pvb_choice_start(size, map_id), &row)])
}

/// `91` as the answer to a choice that was refused: one row at the start
/// it was asked with, `"B2;no;<code>;<gear>;<max gear>;<running>;<max
/// matches>"` (the numbers as [`pvb_status`] has them). At most 41 bytes.
pub fn pvb_choose_no(
    start: u32,
    code: PvbCode,
    numbers: PvbNumbers,
) -> Result<ServerMessage, WireError> {
    let row = Ranked {
        name: format!(
            "{PVB_CHOOSE_VERSION};no;{};{};{};{};{}",
            code as u8, numbers.gear, numbers.max_gear, numbers.running, numbers.max_matches
        ),
        rating: 0,
        rating_before: 0,
    };
    elo_rows(&[(start, &row)])
}

/// `91` as the answer to a replay poll (`3B 00 <REPLAY_START> <count>`):
/// one row, `name = "R1;<status>;<match id 8 lower hex>;<n>"`,
/// `place = REPLAY_START`, rating and change 0 (`CONTRACT-R1`). `status`
/// is the word of the asker's latest ask (`session::v026g::replays::
/// Status::word`), the match the one it asked for (0 before any), the last
/// field the replay file's size when it is ready, its place in the
/// conversion line while preparing (1 being made now, 0 not known), else
/// 0. The movie's
/// `PlayerRanking.fillPlayers` routes an `R1;` header to the History, as
/// `M2;` and `J1;` go to the matches' window. **srp's own**, as safe for
/// the client as [`join_answer`]: 45 bytes at the most (the longest word is
/// 12 letters, a u64 20 digits), the place a u32 the movie tells apart from
/// a ranking row.
pub fn replay_answer(status: &str, match_id: u32, size: u64) -> Result<ServerMessage, WireError> {
    let row = Ranked {
        name: format!("{REPLAY_ANSWER_VERSION};{status};{match_id:08x};{size}"),
        rating: 0,
        rating_before: 0,
    };
    elo_rows(&[(REPLAY_START, &row)])
}

/// One 20-byte `quest_instance` of 0x92.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestRecord {
    pub instance_id: u32,
    pub quest_id: u32,
    /// Games counted towards it.
    pub games: u32,
    pub minutes_left: u32,
    /// The byte at +0x10: 1 is a quest the player has, anything else a
    /// proposal (`cmp byte [q+0x10], 1` at 0x28DDD0+; PROVEN).
    pub active: bool,
}

/// `92`: the account's quests (`lobby.md` §2.16). The field meanings are
/// 0.21d's (LIKELY unchanged; the record size and the byte at +0x10 are
/// PROVEN).
pub fn quests(records: &[QuestRecord]) -> ServerMessage {
    let mut writer = Writer::new();
    writer.u32(records.len() as u32);
    for record in records {
        writer
            .u32(record.instance_id)
            .u32(record.quest_id)
            .u32(record.games)
            .u32(record.minutes_left)
            .bool(record.active)
            .u8(0)
            .u8(0)
            .u8(0);
    }
    writer.done(ServerId::Quests)
}

/// `94`: the account's services (`lobby.md` §2.17). Only the premium
/// account (type 0, its hours left) is read; `None` sends none.
pub fn services(premium_hours: Option<u32>) -> ServerMessage {
    let mut writer = Writer::new();
    match premium_hours {
        Some(hours) => {
            writer.u8(1).u8(0).u8(0).u32(hours).u32(0);
        }
        None => {
            writer.u8(0);
        }
    }
    writer.done(ServerId::Services)
}

/// An item of a shop record: 0x24 bytes (`premium.md` §1; the offsets are
/// PROVEN). The client also reads a u32 at +0x04 and one at +0x1C for a
/// premium entry's discount label (0x228280); srp sends no such item, so
/// they are not modelled and go out as 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShopRecordItem {
    /// +0x00.
    pub dict_id: u32,
    /// +0x0C.
    pub amount: u32,
    /// +0x14 and +0x18: used in the discount computation.
    pub a: u32,
    pub b: u32,
    /// +0x20: 0 is an item of the inventory, which a bundle lists as its
    /// content.
    pub kind: u32,
}

/// A record of the premium shop: 0xD0 bytes (`lobby.md` §2.18).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShopRecord {
    /// +0x00: what `47 02` names.
    pub id: u32,
    /// +0x04: the bundle of `shop_bundles_dict` whose name, description
    /// and icon the offer shows (0x2CAC10).
    pub ui_key: u32,
    /// +0x0C: 0 a premium store entry, else a bundle.
    pub kind: u32,
    /// +0x10: the value or cost.
    pub value: u32,
    /// +0x14 (count) and +0x18 (the items): at most 5.
    pub items: Vec<ShopRecordItem>,
    /// +0xCC.
    pub money_type: u32,
}

impl ShopRecord {
    pub const SIZE: usize = 0xD0;

    fn to_bytes(&self) -> Result<[u8; Self::SIZE], WireError> {
        at_most(
            "shop record items",
            self.items.len(),
            limits::SHOP_RECORD_ITEMS,
        )?;
        let mut bytes = [0; Self::SIZE];
        let mut put = |offset: usize, value: u32| {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(0x00, self.id);
        put(0x04, self.ui_key);
        put(0x0C, self.kind);
        put(0x10, self.value);
        put(0x14, self.items.len() as u32);
        for (index, item) in self.items.iter().enumerate() {
            let at = 0x18 + index * 0x24;
            put(at, item.dict_id);
            put(at + 0x0C, item.amount);
            put(at + 0x14, item.a);
            put(at + 0x18, item.b);
            put(at + 0x20, item.kind);
        }
        put(0xCC, self.money_type);
        Ok(bytes)
    }
}

/// `95`: the premium shop (`lobby.md` §2.18): `u32 n`, `u32 packed_size`
/// and raw LZMA2 that unpacks to `n × 0xD0` bytes. The client always reads
/// the packed size and decodes, so an empty shop carries the stream of no
/// bytes (its end marker): `95 00000000 01000000 00`. That a real server
/// packed nothing the same way is LIKELY (it is what the LZMA2 encoder
/// writes for empty input).
pub fn shop(records: &[ShopRecord]) -> Result<ServerMessage, WireError> {
    let mut unpacked = Vec::with_capacity(records.len() * ShopRecord::SIZE);
    for record in records {
        unpacked.extend_from_slice(&record.to_bytes()?);
    }
    let packed = lzma2::stored(&unpacked);
    let mut writer = Writer::new();
    writer
        .u32(records.len() as u32)
        .u32(packed.len() as u32)
        .bytes(&packed);
    Ok(writer.done(ServerId::Shop))
}

/// `96`: bit `f - 1` unlocks faction `f` (`lobby.md` §2.19).
pub fn unlocked_factions(mask: u8) -> ServerMessage {
    Writer::new().u8(mask).done(ServerId::UnlockedFactions)
}

/// `9D`: the tutorial (`lobby.md` §2.21). `show` runs the tutorial; `a` is
/// a bit set and `b` a reward state (GUESS). After it the client sends
/// `45 <language>` if it has no video link yet.
pub fn tutorial(show: bool, a: u32, b: u32) -> ServerMessage {
    Writer::new()
        .bool(show)
        .u32(a)
        .u32(b)
        .done(ServerId::Tutorial)
}

/// `9F`: players and games online and the minutes to maintenance
/// (`social.md` §4). Under 60 minutes the client shows a warning, at 30 or
/// less Play is locked (and it starts locked: this message is needed), and
/// 0 shows the maintenance notice.
pub fn online_stats(
    players_online: u32,
    games_online: u32,
    minutes_to_maintenance: u32,
) -> ServerMessage {
    Writer::new()
        .u32(players_online)
        .u32(games_online)
        .u32(minutes_to_maintenance)
        .done(ServerId::OnlineStats)
}

/// No maintenance planned: a day, well over the 60 minutes below which the
/// client warns.
pub const MINUTES_TO_MAINTENANCE: u32 = 1440;

// ---- 83 46: inventory actions (`process_succeeded_inventory_action`) ----

const INVENTORY: u8 = super::requests::id::INVENTORY;
const SHOP: u8 = super::requests::id::SHOP;
const PROFILE: u8 = super::requests::id::PROFILE;

/// `83 46 01`: an upgrade (a camouflage or decal, by its item id) went
/// into `slot` of the host; an upgrade that was in the slot comes off.
/// `detach_first` first takes the upgrade off the host itself, where it
/// was in another slot (0x275585; patch 30 of `client-patches`: the stock
/// client took it off the first item of its inventory holding it). An
/// upgrade may be on other items too: they keep it. `value` is the
/// upgrade's u64 (a decal's placement).
pub fn permitted_upgrade_attached(
    host_item_id: u32,
    upgrade_item_id: u32,
    slot: u8,
    value: u64,
    detach_first: bool,
) -> ServerMessage {
    permitted_event(INVENTORY, 1)
        .u32(host_item_id)
        .u32(upgrade_item_id)
        .u8(slot)
        .u64(value)
        .bool(detach_first)
        .done(ServerId::Permitted)
}

/// `83 46 02 <upgrade> <host>`: the upgrade with this item id came off
/// the host; any other item wearing it keeps it. The host is patch 30's
/// (`client-patches`, 0x275756): the stock client reads the upgrade alone
/// and takes it off the first item of its inventory holding it.
pub fn permitted_upgrade_detached(upgrade_item_id: u32, host_item_id: u32) -> ServerMessage {
    permitted_event(INVENTORY, 2)
        .u32(upgrade_item_id)
        .u32(host_item_id)
        .done(ServerId::Permitted)
}

/// `83 46 04`: one of a usable item was used; the client takes one off its
/// count and drops the item at 0.
pub fn permitted_item_used(item_id: u32) -> ServerMessage {
    permitted_event(INVENTORY, 4)
        .u32(item_id)
        .done(ServerId::Permitted)
}

/// `83 46 06`: the reroll window of an item: what the next change costs
/// in silver (`a`) and in gold (`b`), and, once one of its modifications
/// was changed, `more`: that modification's id (the only one the window
/// lets be changed again, `was_rerolled_before`, 0x29FD13) and the item's
/// `reroll_time` in seconds (PROVEN, 0x2757CC -> 0x29F430: `regular_cost`,
/// `premium_cost`, `reroll_time`). The item must be in the client's
/// inventory.
pub fn permitted_reroll_window(
    item_id: u32,
    a: u32,
    b: u32,
    more: Option<(u32, u32)>,
) -> ServerMessage {
    let mut writer = permitted_event(INVENTORY, 6);
    writer.u32(item_id).u32(a).u32(b).bool(more.is_some());
    if let Some((c, d)) = more {
        writer.u32(c).u32(d);
    }
    writer.done(ServerId::Permitted)
}

/// `83 46 07`: the three modifications a reroll offers, and the item's new
/// `reroll_time`. Each must be a modification the client has (0x29F680
/// looks them up).
pub fn permitted_reroll_result(item_id: u32, choices: [u32; 3], reroll_time: u32) -> ServerMessage {
    let mut writer = permitted_event(INVENTORY, 7);
    writer.u32(item_id);
    for choice in choices {
        writer.u32(choice);
    }
    writer.u32(reroll_time).done(ServerId::Permitted)
}

/// `83 46 08`: a modification was replaced. `rest` is what the window
/// shows next: the cost in silver, the cost in gold and the item's
/// `reroll_time` (0x2759F0 -> 0x29F430, with the new modification as the
/// one changed before).
pub fn permitted_modifier_chosen(
    item_id: u32,
    old_modification: u32,
    new_modification: u32,
    rest: [u32; 3],
) -> ServerMessage {
    let mut writer = permitted_event(INVENTORY, 8);
    writer
        .u32(item_id)
        .u32(old_modification)
        .u32(new_modification);
    for value in rest {
        writer.u32(value);
    }
    writer.done(ServerId::Permitted)
}

/// What a craft did to the item (`83 46 09`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crafted {
    /// Mode 0: nothing changed; `consumed_item_id` is taken from the
    /// inventory.
    Nothing { consumed_item_id: u32 },
    /// Mode 1: the modification was added.
    Added {
        consumed_item_id: u32,
        modification: u32,
    },
    /// Another non-zero mode but 0xFE: the modification was removed.
    Removed {
        mode: u8,
        consumed_item_id: u32,
        modification: u32,
    },
    /// Mode 0xFE: reported to the UI as -2 (`st_cannot_craft_item`);
    /// nothing is consumed and nothing more is read (0x275A31).
    Failed,
}

/// `83 46 09`: the result of a craft. The item's `dict_id` must be one the
/// client knows (it is looked up with no check).
pub fn permitted_crafted(item_id: u32, crafted: Crafted) -> Result<ServerMessage, WireError> {
    let mut writer = permitted_event(INVENTORY, 9);
    match crafted {
        Crafted::Nothing { consumed_item_id } => {
            writer.u8(0).u32(item_id).u32(consumed_item_id);
        }
        Crafted::Added {
            consumed_item_id,
            modification,
        } => {
            writer
                .u8(1)
                .u32(item_id)
                .u32(consumed_item_id)
                .u32(modification);
        }
        Crafted::Removed {
            mode,
            consumed_item_id,
            modification,
        } => {
            if matches!(mode, 0 | 1 | 0xFE) {
                return Err(WireError::BadValue("a remove mode that means another"));
            }
            writer
                .u8(mode)
                .u32(item_id)
                .u32(consumed_item_id)
                .u32(modification);
        }
        Crafted::Failed => {
            writer.u8(0xFE).u32(item_id);
        }
    }
    Ok(writer.done(ServerId::Permitted))
}

// ---- 83 47: the shop (`process_shop_action`) ----

/// One entry of a purchase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bought {
    pub dict_id: u16,
    pub item_id: u32,
    /// For a stack: the count added to the item with this id if the client
    /// has it, else the count of a new item. For anything else the client
    /// replaces it with a full condition.
    pub value: u32,
}

/// `83 47 00`: bought. Each `dict_id` must be one the client knows (NULL
/// dereference otherwise). The client asks for nothing after it: push the
/// money (0x8B), and the inventory and account if they changed otherwise.
pub fn permitted_bought(items: &[Bought]) -> Result<ServerMessage, WireError> {
    at_most("bought items", items.len(), u8::MAX.into())?;
    let dictionary = Dictionary::shared();
    let mut writer = permitted_event(SHOP, 0);
    writer.u8(items.len() as u8);
    for item in items {
        if dictionary.item(item.dict_id).is_none() {
            return Err(WireError::UnknownItem(item.dict_id));
        }
        writer.u16(item.dict_id).u32(item.item_id).u32(item.value);
    }
    Ok(writer.done(ServerId::Permitted))
}

/// `83 47 01`: sold. The client takes `amount` off a stack (dropping it at
/// 0) or drops the item.
pub fn permitted_sold(item_id: u32, amount: u32) -> ServerMessage {
    permitted_event(SHOP, 1)
        .u32(item_id)
        .u32(amount)
        .done(ServerId::Permitted)
}

/// `83 47 02`: a shop offer was bought. The client reads and does nothing:
/// push what changed.
pub fn permitted_offer_bought() -> ServerMessage {
    permitted_event(SHOP, 2).done(ServerId::Permitted)
}

/// How a repair was paid (`83 47 03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repaired {
    /// The item's `repair_time` becomes this non-zero value.
    Timed { repair_time: u32 },
    /// At once, with nothing taken from the inventory.
    AtOnce,
    /// At once with a repair kit: `used` of the kit item are taken.
    WithKit { kit_item_id: u32, used: u8 },
}

/// `83 47 03`: repaired. The client sets the item's condition to full and
/// its `repair_time`; **only for a `repair_time` of 0** it reads a kit item
/// id, and for a kit it has, how many were used.
pub fn permitted_repaired(item_id: u32, repaired: Repaired) -> Result<ServerMessage, WireError> {
    let mut writer = permitted_event(SHOP, 3);
    writer.u32(item_id);
    match repaired {
        Repaired::Timed { repair_time } => {
            if repair_time == 0 {
                return Err(WireError::BadValue("a timed repair with no time"));
            }
            writer.u32(repair_time);
        }
        Repaired::AtOnce => {
            writer.u32(0).u32(0);
        }
        Repaired::WithKit { kit_item_id, used } => {
            if kit_item_id == 0 {
                return Err(WireError::BadValue("a repair kit with no id"));
            }
            writer.u32(0).u32(kit_item_id).u8(used);
        }
    }
    Ok(writer.done(ServerId::Permitted))
}

/// `83 47 04` (gold exchanged) and `83 47 05` (shows an important message
/// of type 7): no body is read.
pub fn permitted_shop_event(event: u8) -> Result<ServerMessage, WireError> {
    if !matches!(event, 4 | 5) {
        return Err(WireError::BadValue("a shop event with a body of its own"));
    }
    Ok(permitted_event(SHOP, event).done(ServerId::Permitted))
}

// ---- 83 49: kits and the account (`process_profiles_action`) ----

/// `83 49 01`: the account's autobuy box.
pub fn permitted_autobuy(on: bool) -> ServerMessage {
    permitted_auto_state(on, false)
}

/// The lobby's number for the autobuy and auto-repair boxes: bit 0 and
/// bit 1 (`LobbyMenu.set_autobuy`).
pub(crate) fn auto_state(autobuy: bool, auto_repair: bool) -> u8 {
    u8::from(autobuy) | u8::from(auto_repair) << 1
}

/// `83 49 01`: the account's autobuy and auto-repair boxes, one byte the
/// client stores and shows (0x275CC5 → `root.set_autobuy`).
pub fn permitted_auto_state(autobuy: bool, auto_repair: bool) -> ServerMessage {
    permitted_event(PROFILE, 1)
        .u8(auto_state(autobuy, auto_repair))
        .done(ServerId::Permitted)
}

/// `83 49 02`: the active kit. The client stores the id and selects the kit
/// with it.
pub fn permitted_active_kit(kit_id: u32) -> ServerMessage {
    permitted_event(PROFILE, 2)
        .u32(kit_id)
        .done(ServerId::Permitted)
}

/// Why a name is refused: the client's own texts (`83 49 03/08/09`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameRefusal {
    /// `st_short_clan_string`.
    Short = 0,
    /// `st_few_alpha_clan_string`.
    FewLetters = 1,
    /// `st_already_exist_clan_string`.
    Taken = 2,
    /// `st_space_symbol_clan_string`.
    Space = 3,
    /// `st_wrong_symbol_clan_string`.
    WrongSymbol = 4,
    /// `st_string_pattern_error`: only for a nickname (`83 49 03`).
    Pattern = 5,
}

pub(crate) fn write_name_check(writer: &mut Writer, name: &str, refusal: Option<NameRefusal>) {
    writer.str(name, limits::NAME).bool(refusal.is_none());
    if let Some(refusal) = refusal {
        writer.u8(refusal as u8);
    }
}

/// `83 49 03`: the answer to a nickname check (`49 03`).
pub fn permitted_nickname_check(nickname: &str, refusal: Option<NameRefusal>) -> ServerMessage {
    let mut writer = permitted_event(PROFILE, 3);
    write_name_check(&mut writer, nickname, refusal);
    writer.done(ServerId::Permitted)
}

/// `83 49 04`: the nickname was changed; one of the ticket item is taken
/// (0 for none).
pub fn permitted_nickname_changed(ticket_item_id: u32) -> ServerMessage {
    permitted_event(PROFILE, 4)
        .u32(ticket_item_id)
        .done(ServerId::Permitted)
}
