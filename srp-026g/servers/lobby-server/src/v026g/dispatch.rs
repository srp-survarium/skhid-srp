//! Every request of the client and its answer (`lobby_requests.md`).
//!
//! The client asks for almost nothing: an action is answered with its
//! `83` where the client has a handler for one, and always with the pushes
//! of what it changed (the inventory, the account and the money: the
//! client never asks for them). A refusal is `84 <op> 00 <key>` with the
//! game's own text; the client waits for the answer of an inventory or
//! shop request before it shows the result, so none of them goes
//! unanswered.
//!
//! What srp does not have is refused with the game's text for that
//! failure. Clans are `clans`, items' changes `items`, the premium shop,
//! the daily bonus, the tutorial and a nickname change `shop`.

use std::net::SocketAddr;
use std::time::Instant;

use session::LockExt;
use session::v026g::{quests, rewards, rules, tutorial, wire};
use survarium::v026g::lobby::requests::{ClientMessage, id};
use survarium::v026g::lobby::{self, NameRefusal, Ranked, important};

use super::core::{Core, Who};

/// The texts of the refusals the rules do not name themselves.
mod text {
    pub const DROP_CARD: &str = "st_cannot_use_drop_card";
    pub const RENAME: &str = "st_change_name_error";
    pub const COMMENT: &str = "st_error_during_storing_user_comment";
}

/// What is wrong with a name a client asks about (`49 03`, `49 08`,
/// `49 09`), in the client's own reasons. [choice] The original server's
/// rules are not known; these are the ones srp can keep: three characters
/// or more, no blanks, nothing that does not print, and within the 63
/// bytes the client's buffers hold.
pub(super) fn name_refusal(name: &str, nickname: bool) -> Option<NameRefusal> {
    if name.chars().count() < 3 {
        return Some(NameRefusal::Short);
    }
    if name.chars().any(char::is_whitespace) {
        return Some(NameRefusal::Space);
    }
    if name
        .chars()
        .any(|c| c.is_control() || c == char::REPLACEMENT_CHARACTER)
    {
        return Some(NameRefusal::WrongSymbol);
    }
    if name.len() > lobby::limits::NAME {
        // A clan check has no pattern reason.
        return Some(if nickname {
            NameRefusal::Pattern
        } else {
            NameRefusal::WrongSymbol
        });
    }
    None
}

impl Core<'_> {
    /// One request of a signed-in client.
    pub(super) fn handle(&mut self, addr: SocketAddr, message: ClientMessage, now: Instant) {
        use ClientMessage as M;
        let Some(who) = self.who(addr) else {
            return;
        };
        let who = &who;
        log::debug!("{}: {message:?}", who.name);
        let unix = quests::now();
        match message {
            // The transport delivers a sign-in once; one on a signed-in
            // link is not the client's.
            M::SignIn { .. } => log::warn!("{} signed in twice on one link; ignored", who.name),
            // The echo of 0x80's flag: nothing is asked.
            M::AfterSignIn { .. } => {}

            M::Play { kit_id } => self.play(who, kit_id, now),
            M::ClanPlay { kit_id } => self.clan_play(who, kit_id),
            M::Leave { left_match, .. } => self.leave(who, left_match),

            // ---- queries ----
            M::QuerySquadMember { account_id, .. } => self.squad_member(who, account_id),
            M::QuerySkills => {
                let skills = wire::level_and_skills(&who.account.locked());
                self.send_made(addr, skills);
            }
            M::QueryReputations => {
                let reputations = wire::reputations(&who.account.locked());
                self.send_made(addr, reputations);
            }
            // A start no ranking page asks for is srp's own request, and
            // none of the ranking's.
            M::QueryRanking { start, count } if start >= lobby::RESERVED_START => {
                self.reserved_ranking(who, start, count, now);
            }
            M::QueryRanking { start, count } => self.ranking(who, start, count),
            M::QueryQuests => {
                let (list, notices) = {
                    let mut account = who.account.locked();
                    let notices = quests::refresh(&mut account, unix, &mut self.rng);
                    (wire::quests(&account, unix), notices)
                };
                self.accounts.mark_dirty(who.account_id);
                self.send(addr, list);
                self.notify(who, &notices);
            }
            M::QueryServices => {
                let services = wire::services(&who.account.locked(), unix);
                self.send(addr, services);
            }
            M::QueryUnlockedFactions => {
                let mask = wire::unlocked_factions(&who.account.locked());
                self.send(addr, mask);
            }
            // `41 00 <match>`: the results screen again, for a player of
            // a match still kept.
            M::QueryMatchStats { match_id } => {
                let screen = self
                    .results
                    .find(match_id)
                    .and_then(|report| report.screen_for(who.account_id));
                match screen {
                    Some(screen) => {
                        // Its screen is open again: the report stays for
                        // the rows it may click (`3D`).
                        self.results.shown(who.account_id, match_id);
                        self.send_made(addr, screen);
                    }
                    // Not reached with a stock client: it asks only after a
                    // `#mstats [N]` chat line (0x225810), which srp never
                    // sends, and has nothing to show for "no such match".
                    None => log::info!(
                        "{} asked for the results of match {match_id:#x}: none kept for it",
                        who.name
                    ),
                }
            }
            // `3D 00 <REPLAY_ASK> <match>`: the History's "Watch replay"
            // (`replay`); no row of a results screen has that player.
            M::QueryPlayerDetails {
                player_id: lobby::REPLAY_ASK,
                match_id,
            } => self.replay_ask(who, match_id, now),
            // `3D 00 <GEAR_BY_PLACE> <place>`: the kits of a row of the
            // Rankings window (`gear`), any player's.
            M::QueryPlayerDetails {
                player_id: lobby::GEAR_BY_PLACE,
                match_id: place,
            } => self.ranked_gear(who, place),
            // `3D 00 <GEAR_BY_ACCOUNT> <account>`: the kits of a member of a
            // room the list shows (`rooms`); nobody else's.
            M::QueryPlayerDetails {
                player_id: lobby::GEAR_BY_ACCOUNT,
                match_id: account_id,
            } => self.listed_member_gear(who, account_id),
            // `3D 00 <GEAR_IN_ROOM> <place + 20 × check>`: the kits of a
            // member of the asker's room (`rooms`).
            M::QueryPlayerDetails {
                player_id: lobby::GEAR_IN_ROOM,
                match_id,
            } => self.room_member_gear(who, match_id),
            // `3D 00 <player> <match>`: the kits of a row of that screen,
            // for a player of the match.
            M::QueryPlayerDetails {
                player_id,
                match_id,
            } => {
                let details = self
                    .results
                    .find(match_id)
                    .filter(|report| report.player(who.account_id).is_some())
                    .and_then(|report| report.player_details(player_id));
                match details {
                    Some(details) => self.send_made(addr, details),
                    // Not from an open results screen: the report of a
                    // screen a client may have open is kept
                    // (`Results`, `SHOWN_FOR_SECONDS`), and the client has
                    // no "nothing to show" answer (0x93 is always a
                    // player's kits, 0x271A30).
                    None => log::info!(
                        "{} asked for player {player_id} of match {match_id:#x}: not kept",
                        who.name
                    ),
                }
            }
            // `42 00 <u32>`: not answered, on purpose (`results.md` §5.3).
            // The client's one sender of it (0x270000) is called only from
            // `lobby_menu::on_additional_drop_arrived` (0x2259D9), which
            // runs only for a chat line `[value][type]` of the statistics
            // service on channel 4 (0x26B7F4..0x26B84B). srp never sends
            // that line: its drops come in the results (`97`), the drop
            // card's answer (`98`) and the daily bonus (`99`). The sender
            // keeps nothing and nothing waits for an answer [PROVEN]; what
            // the original server answered is not in the client [`98` is
            // LIKELY: the one other message that carries a drop].
            M::QueryDrop { drop_id } => {
                log::info!(
                    "{} asked for drop {drop_id}, which srp never announced: not answered",
                    who.name
                );
            }
            // `43 00`: the daily bonus (`shop`).
            M::QueryDailyBonus => self.daily_bonus(who),
            // No video: the link stays hidden.
            M::QueryVideoLink { language } => {
                if let Some(player) = self.player_mut(addr) {
                    player.language = Some(language);
                }
                self.send(addr, lobby::video_link(false, ""));
            }

            // ---- the inventory (0x46) ----
            M::MoveItems { kit_id, moves } => {
                let moved = rules::move_items(&mut who.account.locked(), kit_id, &moves, unix);
                if let Err(error) = moved {
                    log::info!(
                        "{}: move refused: {error}; kit {kit_id}, asked: {}",
                        who.name,
                        moves
                            .iter()
                            .map(|m| format!(
                                "item {} (dict {}) slot {} -> {}, {} of {}",
                                m.item_id,
                                m.dict_id,
                                m.source_slot,
                                m.target_slot,
                                m.count,
                                m.count_in_inventory
                            ))
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                    self.deny(addr, id::INVENTORY, error.text_key());
                }
                // The client has no confirmation for a move: the inventory
                // and the account are it; after a refusal they put its
                // window right again.
                self.push_account(who);
                self.squads_dirty = true;
            }
            // Cosmetics, modifications and upgrades: `items`. The fields'
            // meanings are the client's (`lobby_requests.md`): `a0` the
            // item, `a1` the cosmetic, `a2` its slot.
            M::AttachUpgrade { a0, a1, a2, .. } => self.attach_upgrade(who, a0, a1, a2),
            M::DetachUpgrade {
                upgrade_item_id,
                host_item_id,
                ..
            } => self.detach_upgrade(who, host_item_id, upgrade_item_id),
            M::ApplyDecal {
                item_id, a, value, ..
            } => self.place_upgrade(who, item_id, a, value),
            M::UseDropCard { a0, a1 } => self.use_drop_card(who, a0, a1),
            M::UseItem { item_id } => self.use_item(who, item_id),
            M::OpenReroll { item_id } => self.open_reroll(who, item_id),
            // The flag: paid in gold.
            M::RerollModifier {
                item_id,
                modification_id,
                flag,
            } => self.reroll_modifier(who, item_id, modification_id, flag),
            // The "new modification" is the place of the choice among the
            // three offered.
            M::ChooseModifier {
                item_id,
                old_modification_id,
                new_modification_id,
            } => self.choose_modifier(who, item_id, old_modification_id, new_modification_id),
            // The item, its donor, and whether gold pays.
            M::Craft { a0, a1, flag } => self.craft(who, a0, a1, flag),
            // No client runs the tutorial (`session::v026g::tutorial`): its
            // steps are not answered.
            M::TutorialWeaponChosen | M::TutorialStep { .. } | M::SkipTutorial => {
                log::info!("{}: a tutorial step, but there is no tutorial", who.name);
            }

            // ---- the shop (0x47) ----
            M::Buy {
                dict_id,
                count,
                faction,
                target_slot,
                kit_id,
            } => {
                // A premium item is paid in gold (`shop`).
                if session::v026g::premium::sells(dict_id) {
                    self.buy_premium_item(who, dict_id, count, target_slot, kit_id);
                    return;
                }
                let bought = rules::buy(
                    &mut who.account.locked(),
                    dict_id,
                    count,
                    faction,
                    target_slot,
                    kit_id,
                    unix,
                );
                match bought {
                    Ok(bought) => {
                        self.send_made(addr, lobby::permitted_bought(&bought));
                        self.push_account(who);
                    }
                    Err(error) => {
                        log::info!("{}: purchase refused: {error}", who.name);
                        self.deny(addr, id::SHOP, error.text_key());
                    }
                }
            }
            M::Sell { item_id, amount } => {
                let sold = rules::sell(&mut who.account.locked(), item_id, amount);
                match sold {
                    Ok(sold) => {
                        self.send(addr, lobby::permitted_sold(sold.item_id, sold.amount));
                        self.push_account(who);
                        self.squads_dirty = true;
                    }
                    Err(error) => {
                        log::info!("{}: sale refused: {error}", who.name);
                        self.deny(addr, id::SHOP, error.text_key());
                    }
                }
            }
            M::Repair { item_id, payment } => self.repair(who, item_id, payment),
            M::ExchangePremium { amount } => {
                let exchanged = rules::exchange_premium(&mut who.account.locked(), amount);
                self.exchanged(who, exchanged);
            }
            M::BuyPremium { gold } => {
                let bought = rules::buy_premium(&mut who.account.locked(), gold);
                self.exchanged(who, bought);
            }
            // The premium shop (`shop`). `47 02` with a match id is the
            // results screen's "buy premium": the match's premium half is
            // given with it.
            M::BuyOffer { offer_id, x, .. } => self.buy_offer(who, offer_id, x),
            M::AcceptPremiumWithBonuses { a0, a1, .. } => {
                self.accept_premium_with_bonuses(who, a0, a1);
            }
            M::AcceptPremiumWithDiscount { a0, a1 } => {
                self.accept_premium_with_discount(who, a0, a1);
            }

            // ---- skills (0x48): the client has no `83 48`; 0x8C is the
            // answer, and the kits carry the skills' modifiers ----
            M::SetSkills { skills } => {
                let set = rules::set_skills(&mut who.account.locked(), &skills);
                self.skills_answer(who, set.map(drop));
            }
            M::ResetSkills { branch, payment } => {
                let reset = rules::reset_skills(&mut who.account.locked(), branch, payment);
                self.skills_answer(who, reset.map(drop));
            }

            // ---- kits and the account (0x49) ----
            M::RenameKit { kit_id, name } => {
                let renamed = rules::rename_kit(&mut who.account.locked(), kit_id, &name);
                if renamed.is_err() {
                    self.deny(addr, id::PROFILE, text::RENAME);
                }
                self.push_account(who);
            }
            // The two boxes of the kit window: autobuy and auto-repair
            // (`session::v026g::autobuy`).
            M::SetAutobuy { on, repair } => {
                {
                    let mut account = who.account.locked();
                    rules::set_autobuy(&mut account, on);
                    account.extras.auto_repair = repair;
                }
                self.accounts.mark_dirty(who.account_id);
                self.send(addr, lobby::permitted_auto_state(on, repair));
            }
            M::SetActiveKit { kit_id } => {
                let set = rules::set_active_kit(&mut who.account.locked(), kit_id, unix);
                match set {
                    Ok(()) => {
                        self.accounts.mark_dirty(who.account_id);
                        self.send(addr, lobby::permitted_active_kit(kit_id));
                        // The squad mates see the kit played.
                        self.squads_dirty = true;
                    }
                    Err(error) => {
                        self.deny(addr, id::PROFILE, error.text_key());
                        // The kit the account plays, as it is.
                        self.push_account(who);
                    }
                }
            }
            M::CheckNickname { nickname } => {
                let refusal = self.nickname_refusal(&nickname, who.account_id);
                self.send(addr, lobby::permitted_nickname_check(&nickname, refusal));
            }
            M::ChangeNickname {
                nickname,
                ticket_item_id,
            } => self.change_nickname(who, &nickname, ticket_item_id),
            // Not kept, and the player is told so.
            M::CommentAboutUser { .. } => self.deny(addr, id::PROFILE, text::COMMENT),
            M::HideVideoLink => {}
            M::Clan(request) => self.clan_request(who, &request, now),

            // ---- squads (0x4A) ----
            M::DisbandSquad
            | M::AddSquadMember { .. }
            | M::DropSquadMember { .. }
            | M::LeaveSquad => self.squad_action(who, &message),

            // ---- quests and the tutorial (0x4B) ----
            M::ChooseQuestFaction { faction } => {
                let chosen = {
                    let mut account = who.account.locked();
                    quests::choose_faction(&mut account, faction).map(|()| {
                        let notices = quests::refresh(&mut account, unix, &mut self.rng);
                        (wire::reputations(&account), notices)
                    })
                };
                match chosen {
                    Ok((reputations, notices)) => {
                        self.accounts.mark_dirty(who.account_id);
                        // The faction byte travels in 0x8E; then `83 4B`
                        // makes the client ask for its quests and services.
                        self.send_made(addr, reputations);
                        self.send(addr, lobby::permitted_quest_faction());
                        self.notify(who, &notices);
                    }
                    // The client shows nothing for a denied 0x4B.
                    Err(error) => {
                        log::info!("{}: faction {faction} refused: {error}", who.name);
                        self.deny(addr, id::QUEST, error.text_key());
                    }
                }
            }

            // ---- chat, friends (0x4C..=0x50) ----
            M::ChatText {
                receiver,
                channel,
                body,
                ..
            } => self.chat(who, &receiver, channel, &body, now),
            // A clan or clan-team invite the server permitted (`clans`).
            M::Important {
                target_name,
                kind: kind @ (important::CLAN_INVITE | important::CLAN_TEAM_INVITE),
                ..
            } => self.clan_important(who, &target_name, kind),
            M::Important {
                target_name,
                kind,
                text,
                ..
            } => self.important(who, &target_name, kind, &text, now),
            M::Friendship(action) => self.friendship(who, &action, now),
            M::Subscribe {
                channel_ids,
                language,
            } => {
                if let Some(player) = self.player_mut(addr) {
                    player.channel_ids = channel_ids;
                    player.language = Some(language);
                }
            }
            M::ImportantRead { message_id, answer } => self.important_read(who, message_id, answer),
        }
    }

    /// The answer to a skills request: a refusal's text, then the skills as
    /// they are, and once changed the account, whose kits carry the skills'
    /// modifiers (and the money a reset cost).
    fn skills_answer(&mut self, who: &Who, result: Result<(), rules::ActionError>) {
        let changed = result.is_ok();
        if let Err(error) = result {
            log::info!("{}: skills refused: {error}", who.name);
            self.deny(who.addr, id::SKILLS, error.text_key());
        }
        let skills = wire::level_and_skills(&who.account.locked());
        self.send_made(who.addr, skills);
        if changed {
            self.push_account(who);
            self.squads_dirty = true;
        }
    }

    /// The answer to `47 04`, either way: `83 47 04`, then the account with
    /// its new silver and gold; or the game's text for the refusal.
    fn exchanged(&mut self, who: &Who, result: Result<u32, rules::ActionError>) {
        match result {
            Ok(_) => {
                self.send_made(who.addr, lobby::permitted_shop_event(4));
                self.push_account(who);
            }
            Err(error) => {
                log::info!("{}: exchange refused: {error}", who.name);
                self.deny(who.addr, id::SHOP, error.text_key());
            }
        }
    }

    /// `46 03 <a> <b>`: a drop card used from the results screen's stash
    /// map: `98` with what it gave, then the inventory, the account and the
    /// money (a used-up card and silver or gold are not in `98`). `a` is
    /// the match and `b` the card's item id [PROVEN: the screen calls
    /// `use_drop_card(match id, voucher id)`, and 0x288D00 writes the two
    /// in that order; `results.md` §3]. Only an item that is a drop card
    /// of the account is taken, and the other order is still tried after.
    fn use_drop_card(&mut self, who: &Who, a: u32, b: u32) {
        let used = [(b, a), (a, b)].into_iter().find_map(|(card, match_id)| {
            let level = self
                .results
                .find(match_id)
                .map_or(0, |report| report.match_level);
            rewards::use_drop_card(&mut who.account.locked(), card, level, &mut self.rng)
        });
        match used {
            Some(Ok(used)) => {
                log::info!(
                    "{} used a drop card: {} x dict {} ({} left)",
                    who.name,
                    used.dropped.count,
                    used.dropped.dict_id,
                    used.cards_left
                );
                self.send(who.addr, used.message);
                self.push_account(who);
            }
            Some(Err(error)) => {
                // The card is spent and the item given: the pushes say so.
                log::error!("{}'s drop could not be sent: {error}", who.name);
                self.push_account(who);
            }
            None => {
                log::info!("{}: no drop card {a} / {b} to use", who.name);
                self.deny(who.addr, id::INVENTORY, text::DROP_CARD);
            }
        }
    }

    /// `3B 00 <start> <count>`: the ranking rows the page asks for. The
    /// page opens with `start` 0: the count and the account's own row were
    /// sent at sign-in, and players who signed up or moved since have
    /// changed them.
    fn ranking(&mut self, who: &Who, start: u32, count: u8) {
        // With no account locked: the ranking locks each in turn.
        let ranking = self.accounts.ranking();
        let from = (start as usize).min(ranking.len());
        let to = (from + usize::from(count)).min(ranking.len());
        if start == 0 {
            self.send(who.addr, lobby::base_info(ranking.len() as u32, &who.name));
            // The client hands the count to the page only in its `9D`
            // handler (0x274D72..0x274D8E, the one caller of 0x227700), and
            // the page shows no row past the count it has: a player who
            // came after this one signed in pushed its last rows out. `9D`
            // with `show` off stores the same flags and runs no tutorial
            // (0x27136E).
            let recount = tutorial::message(false);
            self.send(who.addr, recount);
            if let Some(player) = self.player_mut(who.addr) {
                player.ranked_sent = ranking.len() as u32;
            }
            if let Some(place) = ranking.iter().position(|row| row.0 == who.account_id) {
                let row = &ranking[place];
                self.send(who.addr, lobby::player_elo(place as u32 + 1, row.2, row.3));
                // The own place under the list: a right-click on it is the
                // asker, however the ranking moves.
                self.ranked_shown(who, place as u32, vec![who.account_id]);
            }
        }
        let rows: Vec<Ranked> = ranking[from..to]
            .iter()
            .map(|row| Ranked {
                name: row.1.clone(),
                rating: row.2,
                rating_before: row.3,
            })
            .collect();
        // Who each row of the page is: a right-click on it asks for that
        // account's kits by its place (`gear`), whatever moved since.
        let shown = ranking[from..to].iter().map(|row| row.0).collect();
        self.ranked_shown(who, from as u32, shown);
        self.send_made(who.addr, lobby::elo_list(from as u32, &rows));
    }

    /// `3B 00 <start> <count>` with a start of [`lobby::RESERVED_START`] or
    /// more: srp's own requests, which only its changed lobby movie makes
    /// (the stock ranking page asks for no index past the count of ranked
    /// players). [`lobby::PVB_STATUS_START`] and
    /// a start from [`lobby::PVB_CHOOSE_BASE`] to
    /// [`lobby::PVB_CHOOSE_LAST`] with the status and the choice of a
    /// match against bots (`pvb`), a rooms' start (`lobby::rooms::ROOMS_FIRST`
    /// to `ROOMS_LAST`) with a room request (`rooms`),
    /// [`lobby::REPLAY_START`] with the asker's
    /// own "Watch replay" answer (`replay`); any other gets no answer.
    /// Nothing of the ranking is read, sent or kept for them.
    fn reserved_ranking(&mut self, who: &Who, start: u32, count: u8, now: Instant) {
        if let Some(request) = lobby::rooms::room_request(start, count) {
            self.room_request(who, request, now);
            return;
        }
        if start == lobby::PVB_STATUS_START {
            self.pvb_status(who, count, now);
            return;
        }
        if (lobby::PVB_CHOOSE_BASE..=lobby::PVB_CHOOSE_LAST).contains(&start) {
            self.pvb_choose(who, start, now);
            return;
        }
        if start == lobby::REPLAY_START {
            self.replay_poll(who, now);
            return;
        }
        log::debug!(
            "{} asked for the reserved ranking start {start}: not answered",
            who.name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_checked_by_the_clients_reasons() {
        assert_eq!(name_refusal("ab", true), Some(NameRefusal::Short));
        assert_eq!(name_refusal("a b c", true), Some(NameRefusal::Space));
        assert_eq!(
            name_refusal("abc\u{7}", true),
            Some(NameRefusal::WrongSymbol)
        );
        assert_eq!(
            name_refusal(&"x".repeat(64), true),
            Some(NameRefusal::Pattern)
        );
        // A clan check has no pattern reason.
        assert_eq!(
            name_refusal(&"x".repeat(64), false),
            Some(NameRefusal::WrongSymbol)
        );
        assert_eq!(name_refusal("Сталкер", true), None);
        assert_eq!(name_refusal("abc", false), None);
    }
}
