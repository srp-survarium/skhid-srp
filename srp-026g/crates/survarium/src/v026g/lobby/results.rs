//! The results screen, drops, the daily bonus and the craft tables
//! (`results.md`).

use super::super::dictionary::Dictionary;
use super::push::{
    AccountBody, DescrMode, ItemDescr, write_account_body, write_item, write_item_tail,
};
use super::{ServerId, ServerMessage, WireError, Writer, at_most, limits};

/// The clan tag beside a title's name: a 15-byte buffer.
const TITLE_TAG: usize = 14;

/// A title of the results screen: its holder.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TitleHolder {
    pub name: String,
    pub clan_tag: String,
}

/// A row of the results screen (`player_result_brief`, `results.md` §1.2).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResultRow {
    pub name: String,
    pub clan_tag: String,
    /// The player's id: the recipient's own row is the first whose id is
    /// that of its full result.
    pub player_id: u32,
    pub score: u16,
    pub kills: u16,
    pub deaths: u16,
    /// 0 or 1: anything else corrupts the client's stack.
    pub team: u8,
    pub level: u16,
}

/// `match_stats` (`results.md` §1.1).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MatchStats {
    pub match_id: u32,
    /// Read and never used by the client.
    pub unknown: u32,
    pub winner_team: u8,
    pub team_scores: [u8; 2],
    /// Shown as `team1_param`/`team2_param`; meaning GUESS (a mode's own
    /// score).
    pub team_params: [u32; 2],
    pub game_mode: u8,
    pub best_player: TitleHolder,
    pub best_player_score: u16,
    pub best_shooter: TitleHolder,
    pub shooter_accuracy_percent: u8,
    pub shooter_shots: u16,
    pub best_support: TitleHolder,
    pub support_value: u16,
    pub best_explorer: TitleHolder,
    pub explorer_artefacts: u16,
    pub explorer_items: u16,
    /// At most 20.
    pub rows: Vec<ResultRow>,
}

/// One line of the score breakdown (`results.md` §1.3).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreComponent {
    /// A localization key: the client localizes it.
    pub name_key: String,
    pub show_amount: bool,
    /// For each of the account's kit columns: `(amount, money, experience,
    /// reputation)`. A column whose amount is 0 carries nothing else on the
    /// wire, so its other values are dropped.
    pub columns: [(f32, u32, i16, i16); 3],
}

/// A bonus block (`results.md` §1.4): for each kit column `(money,
/// reputation, experience)` and three bytes (GUESS: percents).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BonusBlock {
    pub columns: [BonusColumn; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BonusColumn {
    pub money: u32,
    pub reputation: u16,
    pub experience: u16,
    pub bonus_money: u8,
    pub bonus_reputation: u8,
    pub bonus_experience: u8,
}

/// `player_result_full` (`results.md` §1.3): the recipient's own result.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FullResult {
    pub player_id: u32,
    pub components: [ScoreComponent; 3],
    pub daily: BonusBlock,
    pub premium_items: BonusBlock,
    // The premium account's block, read and not shown, follows: always
    // empty (there is no premium account, the owner's of 2026-11-09).
    pub elo_before: u16,
    pub elo_after: u16,
    pub quest_money: u32,
    pub quest_reputation: u16,
    /// The faction the reputation is with.
    pub faction: u8,
    /// Seconds played with each kit column.
    pub play_seconds: [u16; 3],
    /// The item the match dropped, if any.
    pub dropped: Option<ItemDescr>,
    pub drop_rarity: u8,
    /// The drop chance's parts, as fractions: base, from the result, from
    /// premium, and the penalty.
    pub drop_chances: [f32; 4],
}

impl FullResult {
    /// A result with nothing in it, for 0x93 (the client discards it).
    pub fn empty(player_id: u32) -> Self {
        Self {
            player_id,
            components: Default::default(),
            daily: BonusBlock::default(),
            premium_items: BonusBlock::default(),
            elo_before: 0,
            elo_after: 0,
            quest_money: 0,
            quest_reputation: 0,
            faction: 0,
            play_seconds: [0; 3],
            dropped: None,
            drop_rarity: 0,
            drop_chances: [0.0; 4],
        }
    }
}

fn write_title(writer: &mut Writer, holder: &TitleHolder) {
    writer
        .str(&holder.name, limits::NAME)
        .str(&holder.clan_tag, TITLE_TAG);
}

fn write_match_stats(writer: &mut Writer, stats: &MatchStats) -> Result<(), WireError> {
    at_most("result rows", stats.rows.len(), limits::RESULT_ROWS)?;
    writer
        .u32(stats.match_id)
        .u32(stats.unknown)
        .u8(stats.winner_team)
        .u8(stats.team_scores[0])
        .u8(stats.team_scores[1])
        .u32(stats.team_params[0])
        .u32(stats.team_params[1])
        .u8(stats.game_mode);
    write_title(writer, &stats.best_player);
    writer.u16(stats.best_player_score);
    write_title(writer, &stats.best_shooter);
    writer
        .u8(stats.shooter_accuracy_percent)
        .u16(stats.shooter_shots);
    write_title(writer, &stats.best_support);
    writer.u16(stats.support_value);
    write_title(writer, &stats.best_explorer);
    writer
        .u16(stats.explorer_artefacts)
        .u16(stats.explorer_items);
    writer.u8(stats.rows.len() as u8);
    for row in &stats.rows {
        if row.team > 1 {
            return Err(WireError::BadValue("a result row's team past 1"));
        }
        writer
            .str(&row.name, limits::NAME)
            .str(&row.clan_tag, limits::CLAN_TAG)
            .u32(row.player_id)
            .u16(row.score)
            .u16(row.kills)
            .u16(row.deaths)
            .u8(row.team)
            .u16(row.level);
    }
    Ok(())
}

fn write_bonus(writer: &mut Writer, block: &BonusBlock) {
    for column in &block.columns {
        writer
            .u32(column.money)
            .u16(column.reputation)
            .u16(column.experience)
            .u8(column.bonus_money)
            .u8(column.bonus_reputation)
            .u8(column.bonus_experience);
    }
}

fn write_full(writer: &mut Writer, full: &FullResult) -> Result<(), WireError> {
    writer.u32(full.player_id);
    for component in &full.components {
        writer
            .str(&component.name_key, limits::NAME)
            .bool(component.show_amount);
        for &(amount, money, experience, reputation) in &component.columns {
            if amount.is_nan() {
                return Err(WireError::BadValue("a score amount that is no number"));
            }
            writer.f32(amount);
            // The reader skips the rest of a column whose amount is 0.0 or
            // -0.0.
            if amount != 0.0 {
                writer
                    .u32(money)
                    .u16(experience as u16)
                    .u16(reputation as u16);
            }
        }
    }
    write_bonus(writer, &full.daily);
    write_bonus(writer, &full.premium_items);
    // The premium account's, empty.
    write_bonus(writer, &BonusBlock::default());
    writer
        .u16(full.elo_before)
        .u16(full.elo_after)
        .u32(full.quest_money)
        .u16(full.quest_reputation)
        .u8(full.faction);
    for seconds in full.play_seconds {
        writer.u16(seconds);
    }
    // The list the UI never reads; a 5th entry would smash the client's
    // stack (`results.md` §6.1): always none.
    writer.u8(0);
    match &full.dropped {
        Some(item) => write_item(writer, item, DescrMode::Brief)?,
        // No drop: an empty descriptor (dict id 0 ends it).
        None => {
            writer.u32(0).u16(0);
        }
    }
    writer.u8(full.drop_rarity);
    for chance in full.drop_chances {
        writer.f32(chance);
    }
    Ok(())
}

/// `97`: a match's results with the recipient's own full result
/// (`results.md` §1). It opens the results screen by itself; afterwards the
/// client sends `3C 00`.
pub fn match_stats(stats: &MatchStats, own: &FullResult) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    write_match_stats(&mut writer, stats)?;
    write_full(&mut writer, own)?;
    Ok(writer.done(ServerId::MatchStats))
}

/// `93`: the player clicked on the results screen, the answer to
/// `3D 00 <player> <match>` (`results.md` §2): a full result the client
/// discards, then that player's account body, whose kits it shows.
pub fn player_details(
    discarded: &FullResult,
    player: &AccountBody,
) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    write_full(&mut writer, discarded)?;
    write_account_body(&mut writer, player)?;
    Ok(writer.done(ServerId::PlayerDetails))
}

/// `98`: an item dropped from a drop card (`results.md` §3). The client
/// adds the item to its inventory (or its count to the item it has with
/// that id) and sets the card's count. The item's `dict_id` must be known
/// and not 0: the client dereferences its dictionary entry with no check.
pub fn drop(
    item: &ItemDescr,
    card_item_id: u32,
    card_count: u32,
    rarity: u8,
) -> Result<ServerMessage, WireError> {
    let mut writer = Writer::new();
    write_item(&mut writer, item, DescrMode::Lobby)?;
    writer.u32(card_item_id).u32(card_count).u8(rarity);
    Ok(writer.done(ServerId::Drop))
}

/// `99`: the daily bonus (`results.md` §4.1): the progress of each day so
/// far (at most 7; their number is the current day), the reward item, and
/// whether it was taken. `show_window` opens the reward window.
pub fn daily_bonus(
    progress: &[u8],
    reward: &ItemDescr,
    completed: bool,
    show_window: bool,
) -> Result<ServerMessage, WireError> {
    at_most("daily bonus days", progress.len(), limits::BONUS_DAYS)?;
    let entry = Dictionary::shared()
        .item(reward.dict_id)
        .ok_or(WireError::UnknownItem(reward.dict_id))?;
    let mut writer = Writer::new();
    writer.u8(progress.len() as u8).bytes(progress);
    // This descriptor names `is_stack` on the wire, and names it FIRST
    // (PROVEN 0x2773CE: `mov dl,[eax]`, then the u32 at +1, then the u16
    // at +5). The other order makes the client take a stack for an item
    // with two more u32, so it reads 8 bytes past the message's end and
    // takes its counts from whatever lies there: the crash of 2026-10-12
    // (`results.md` §4.1).
    writer
        .bool(entry.is_stack)
        .u32(reward.item_id)
        .u16(reward.dict_id);
    write_item_tail(&mut writer, reward, entry.is_stack, DescrMode::Lobby)?;
    writer.bool(completed).bool(show_window);
    Ok(writer.done(ServerId::DailyBonus))
}

/// `9B`: the craft tables (`results.md` §4.2): 12 chances (3 rows of 4)
/// and 16 prices in silver and in gold. Fixed 112 bytes.
pub fn craft_tables(
    chances: &[f32; 12],
    prices_general: &[u16; 16],
    prices_premium: &[u16; 16],
) -> ServerMessage {
    let mut writer = Writer::new();
    for &chance in chances {
        writer.f32(chance);
    }
    for &price in prices_general.iter().chain(prices_premium) {
        writer.u16(price);
    }
    writer.done(ServerId::CraftTables)
}
