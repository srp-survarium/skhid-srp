//! Clans on the lobby link (`social.md` §3): `49 06..=23`, the clan info
//! 0x9A, clan chat (channels 3 and 9) and clan-team Play (`30`).
//!
//! The rules are the store's (`session::v026g::clans`, which names what is
//! the game's and what is srp's own); here a request becomes its answer
//! and the pushes that show the change to everyone it concerns:
//!
//! - **0x9A** is never asked for but by `49 0E` (the clan and clan-team
//!   windows opening), so every change is pushed to the members in the
//!   lobby: the whole clan (type 0) to a player whose clan it now is and
//!   when the registration or the commander changed, the members (type 2)
//!   when one joined, left, changed rank or went on- or offline, the ranks
//!   (type 3), the daily message (type 1), the clan team (type 4), and "no
//!   clan" (type 0 with id 0, after type 6 when the commander disbanded
//!   it). At sign-in a member gets types 5, 0, 1 and 4.
//! - **Invites.** `49 0F <name>` is answered `83 49 0F` with the clan's id
//!   as the token the client sends back in its long `4D` (type 7); only
//!   then the invite exists and the player is sent the important message
//!   (0xA4, type 7, whose sender id it answers `49 0A` with). A clan-team
//!   invite is the same with `49 19`, type 10 and `49 1A`.
//! - **Clan lines.** A member joining, leaving or being kicked and a new
//!   clan level are chat lines of the clan service (0xA2, sender type 9,
//!   channel 3, `k\x0Eargs`) the client words itself.
//! - **Clan-team Play.** `30 <kit>` is answered `83 30`; the client then
//!   toggles its ready state (`49 20` a soldier, `49 21` the commander).
//!   A team whose commander is ready waits for another clan's team: the
//!   two are seated in one match, each on its side
//!   (`session::Matchmaking::seat_teams`), and go to it like any match.
//!   While it waits its players are "in the queue" (0x85), so Play is
//!   their way out (`31`), which takes the team out.
//! - An account's `clan_tag` (chat, friends, the match) is its registered
//!   clan's abbreviation, kept in step here.

use std::net::SocketAddr;
use std::time::Instant;

use session::LockExt;
use session::v026g::clans::{Clan, ClanError, Departure, NameFault, RankUp, Team, role};
use session::v026g::{quests, rules};
use survarium::v026g::lobby::requests::{ClanRequest, id};
use survarium::v026g::lobby::{
    self, ClanApplicant, ClanInfo, ClanMember, ClanRank, ClanTeamMember, ClanTeamSeat, ClientState,
    NameRefusal, channel, important, sender,
};

use super::core::{Core, Who};

/// A clan team waiting for another clan's.
struct Waiting {
    clan_id: u32,
    /// Its players, who were told they are in the queue.
    accounts: Vec<u32>,
}

/// What the lobby keeps for the clans beside the store.
#[derive(Default)]
pub(super) struct ClanState {
    waiting: Vec<Waiting>,
    /// The invites the server permitted (`83 49 0F`, `83 49 19`) and the
    /// client has not sent yet: `(inviter, target, to the team)`.
    permitted: Vec<(u32, u32, bool)>,
}

/// The most permitted invites kept; past it the oldest goes.
const MAX_PERMITTED: usize = 1_000;

/// The kinds of the clan service's chat lines (0x26B940).
mod line {
    pub const JOINED: u8 = 0;
    pub const LEFT: u8 = 1;
    pub const KICKED: u8 = 2;
    pub const LEVEL: u8 = 3;
}

/// The game's text for a team action by a player in a squad (its client
/// offers none of them then).
const IN_SQUAD: &str = "st_cant_create_clan_team";

fn refusal(fault: Option<NameFault>) -> Option<NameRefusal> {
    Some(match fault? {
        NameFault::Short => NameRefusal::Short,
        NameFault::FewLetters => NameRefusal::FewLetters,
        NameFault::Taken => NameRefusal::Taken,
        NameFault::Delimiters => NameRefusal::Space,
        NameFault::WrongSymbol => NameRefusal::WrongSymbol,
    })
}

impl Core<'_> {
    fn clans(&self) -> &session::v026g::clans::Clans {
        self.accounts.clans()
    }

    fn clan_message_id(&mut self) -> u32 {
        let id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1).max(1);
        id
    }

    fn shown_name(&self, account_id: u32) -> String {
        self.name_of(account_id).unwrap_or_default()
    }

    /// The members of a clan in the lobby.
    fn clan_links(&self, clan: &Clan) -> Vec<(u32, SocketAddr)> {
        clan.accepted()
            .filter_map(|member| {
                Some((member.account_id, *self.by_account.get(&member.account_id)?))
            })
            .collect()
    }

    // ------------------------------------------------------------ messages

    /// The whole clan as `me` is shown it (0x9A type 0).
    fn clan_full(&self, clan: &Clan, me: u32) -> ClanInfo {
        let ranks = clan
            .ranks
            .iter()
            .map(|rank| ClanRank {
                name: rank.name.clone(),
                permissions: rank.permissions,
                is_default: rank.is_default,
            })
            .collect();
        if clan.registered {
            let (members, index) = self.clan_members(clan, me);
            let (level, start, end) = clan.level();
            return ClanInfo::Registered {
                clan_id: clan.id,
                name: clan.name.clone(),
                tag: clan.tag.clone(),
                ranks,
                members,
                unknown: index,
                flag: clan.leader == me,
                level,
                progress: [
                    self.clans().config().max_members.into(),
                    start,
                    end,
                    clan.experience,
                ],
            };
        }
        let index = clan
            .members
            .iter()
            .position(|member| member.account_id == me)
            .unwrap_or(0) as u16;
        ClanInfo::Registering {
            clan_id: clan.id,
            name: clan.name.clone(),
            tag: clan.tag.clone(),
            ranks,
            members: clan
                .members
                .iter()
                .map(|member| ClanApplicant {
                    account_id: member.account_id,
                    name: self.shown_name(member.account_id),
                    rank: member.rank,
                    status: member.status,
                })
                .collect(),
            unknown: index,
            flag: clan.leader == me,
            paid: clan.money >= self.clans().config().cost,
            money: clan.money,
            // The name and the abbreviation were checked when reserved.
            reg_state: 3,
        }
    }

    /// The members of a registered clan, highest rank first, and where
    /// `me` is among them (the client takes its own permissions from that
    /// member's rank).
    fn clan_members(&self, clan: &Clan, me: u32) -> (Vec<ClanMember>, u16) {
        let mut members: Vec<_> = clan.accepted().copied().collect();
        members.sort_by_key(|member| member.rank);
        let index = members
            .iter()
            .position(|member| member.account_id == me)
            .unwrap_or(0) as u16;
        let rows = members
            .iter()
            .map(|member| ClanMember {
                account_id: member.account_id,
                name: self.shown_name(member.account_id),
                rank: member.rank,
                level: self
                    .accounts
                    .get(member.account_id)
                    .map_or(1, |account| account.locked().level()),
                online: self.by_account.contains_key(&member.account_id),
            })
            .collect();
        (rows, index)
    }

    /// The clan's team as `me` is shown it (0x9A type 4): everyone listed,
    /// and its own place if it is of the team (applicants are not).
    fn clan_team_info(&self, clan_id: u32, team: Option<&Team>, me: u32) -> ClanInfo {
        let members = team.map_or(Vec::new(), |team| {
            team.members
                .iter()
                .map(|member| ClanTeamMember {
                    account_id: member.account_id,
                    name: self.shown_name(member.account_id),
                    role: member.role,
                    ready: member.ready,
                })
                .collect()
        });
        let seat = team.and_then(|team| {
            let member = team.taken().find(|member| member.account_id == me)?;
            Some(ClanTeamSeat {
                in_clan_team: true,
                i_am_commander: member.role == role::COMMANDER,
                // The player's own ready state (see `ClanTeamSeat`).
                commander_ready: member.ready,
                clan_team_id: team.id,
            })
        });
        ClanInfo::ClanTeam {
            clan_id,
            members,
            seat,
        }
    }

    fn send_clan(&mut self, to: SocketAddr, info: &ClanInfo) {
        self.send_made(to, lobby::clan_info(info));
    }

    // -------------------------------------------------------------- pushes

    /// The whole clan to each of its members in the lobby.
    fn clan_push_full(&mut self, clan: &Clan) {
        for (account_id, addr) in self.clan_links(clan) {
            let info = self.clan_full(clan, account_id);
            self.send_clan(addr, &info);
        }
    }

    /// The members to each member in the lobby but `except` (0x9A type 2;
    /// a registration has only the whole).
    fn clan_push_members(&mut self, clan: &Clan, except: Option<u32>) {
        for (account_id, addr) in self.clan_links(clan) {
            if Some(account_id) == except {
                continue;
            }
            let info = if clan.registered {
                let (members, index) = self.clan_members(clan, account_id);
                ClanInfo::Members {
                    clan_id: clan.id,
                    members,
                    unknown: index,
                    flag: clan.leader == account_id,
                }
            } else {
                self.clan_full(clan, account_id)
            };
            self.send_clan(addr, &info);
        }
    }

    /// The ranks, then the members (their ranks are indices into them).
    fn clan_push_ranks(&mut self, clan: &Clan, members_too: bool) {
        let ranks = ClanInfo::Ranks {
            clan_id: clan.id,
            ranks: clan
                .ranks
                .iter()
                .map(|rank| ClanRank {
                    name: rank.name.clone(),
                    permissions: rank.permissions,
                    is_default: rank.is_default,
                })
                .collect(),
        };
        for (_, addr) in self.clan_links(clan) {
            self.send_clan(addr, &ranks);
        }
        if members_too {
            self.clan_push_members(clan, None);
        }
    }

    /// The clan's team to each member of the clan in the lobby.
    fn clan_push_team(&mut self, clan_id: u32) {
        let Some(clan) = self.clans().clan(clan_id).filter(|clan| clan.registered) else {
            return;
        };
        let team = self.clans().team(clan_id);
        for (account_id, addr) in self.clan_links(&clan) {
            let info = self.clan_team_info(clan_id, team.as_ref(), account_id);
            self.send_clan(addr, &info);
        }
    }

    /// A line of the clan service on the clan's channel, which the client
    /// words itself from `kind` and the arguments.
    fn clan_line(&mut self, clan: &Clan, kind: u8, arguments: &[&str]) {
        let mut body = kind.to_string();
        for argument in arguments {
            body.push('\x0E');
            body.push_str(argument);
        }
        let line = lobby::chat_text(sender::CLAN, 0, "", "", channel::CLAN, &body);
        for (_, addr) in self.clan_links(clan) {
            self.send(addr, line.clone());
        }
    }

    /// The account's clan tag is `tag` from now on: kept with the account,
    /// and its client (and its squad mates') shown the account again.
    fn clan_set_tag(&mut self, account_id: u32, tag: &str) {
        let Some(account) = self.accounts.get(account_id) else {
            return;
        };
        {
            let mut account = account.locked();
            if account.clan_tag == tag {
                return;
            }
            account.clan_tag = tag.to_owned();
        }
        self.accounts.mark_dirty(account_id);
        if let Some(who) = self.who_is(account_id) {
            self.push_account(&who);
            self.squads_dirty = true;
        }
    }

    /// The player has no clan any more: no tag, and its client is told
    /// (after "your clan was disbanded" when it was).
    fn clan_none(&mut self, account_id: u32, disbanded: bool) {
        self.clan_set_tag(account_id, "");
        let Some(addr) = self.by_account.get(&account_id).copied() else {
            return;
        };
        if disbanded {
            self.send_clan(addr, &ClanInfo::Disbanded { show: true });
        }
        self.send_clan(addr, &ClanInfo::None);
    }

    // ---------------------------------------------------- sign-in and exit

    /// A player signed in: its clan, which its client never asks for
    /// before a clan window opens, the invites that waited for it, and its
    /// clan mates see it online.
    pub(super) fn clan_sign_in(&mut self, who: &Who) {
        let clan = self.clans().clan_of(who.account_id);
        let tag = clan
            .as_ref()
            .filter(|clan| clan.registered)
            .map_or("", |clan| clan.tag.as_str());
        self.clan_set_tag(who.account_id, tag);
        if let Some(clan) = &clan {
            if clan.registered {
                self.send_clan(
                    who.addr,
                    &ClanInfo::Startup {
                        clan_id: clan.id,
                        name: clan.name.clone(),
                        tag: clan.tag.clone(),
                    },
                );
            }
            let full = self.clan_full(clan, who.account_id);
            self.send_clan(who.addr, &full);
            if clan.registered {
                if !clan.daily_message.is_empty() {
                    self.send_clan(
                        who.addr,
                        &ClanInfo::DailyMessage {
                            clan_id: clan.id,
                            text: clan.daily_message.clone(),
                        },
                    );
                }
                if let Some(team) = self.clans().team(clan.id) {
                    let info = self.clan_team_info(clan.id, Some(&team), who.account_id);
                    self.send_clan(who.addr, &info);
                }
                self.clan_push_members(clan, Some(who.account_id));
            }
        }
        for invite in self.clans().invites_for(who.account_id) {
            if let Some(clan) = self.clans().clan(invite.clan_id) {
                let message =
                    self.clan_invite_message(&clan, invite.inviter, invite.team, invite.message_id);
                self.send(who.addr, message);
            }
        }
    }

    /// An invite to a clan (0xA4 type 7) or its team (type 10) as the
    /// client shows it (PROVEN, 0x226F77 and 0x226372): the sender line is
    /// `[<abbreviation>] <text>` and a clan invite's body
    /// `st_add_to_clan_request "<sender name>"`. So the inviter's name is
    /// the text, the clan's name the sender's, and the abbreviation the
    /// clan's (a registration's too). The sender id is the inviter's: the
    /// client answers with it.
    fn clan_invite_message(
        &self,
        clan: &Clan,
        inviter: u32,
        team: bool,
        message_id: u32,
    ) -> lobby::ServerMessage {
        let kind = if team {
            important::CLAN_TEAM_INVITE
        } else {
            important::CLAN_INVITE
        };
        lobby::important_message(
            sender::ACCOUNT,
            inviter,
            &clan.name,
            &clan.tag,
            kind,
            &self.shown_name(inviter),
            message_id,
        )
    }

    /// A player's link is no more: it is not ready, a team it played in
    /// does not wait, and with its session (`gone`) it leaves the clan
    /// team; its clan mates see it offline.
    pub(super) fn clan_link_gone(&mut self, account_id: u32, gone: bool) {
        let Some(clan) = self.clans().clan_of(account_id) else {
            return;
        };
        let left = gone && self.clans().leave_team(account_id).is_ok();
        if left || self.clans().member_unready(account_id).is_some() {
            self.clan_team_changed(clan.id, None);
        }
        if clan.registered {
            self.clan_push_members(&clan, Some(account_id));
        }
    }

    // ------------------------------------------------------------ requests

    /// `49 06..=23`.
    pub(super) fn clan_request(&mut self, who: &Who, request: &ClanRequest, now: Instant) {
        use ClanRequest as C;
        let me = who.account_id;
        let result: Result<(), ClanError> = match request {
            C::CheckName { name } => {
                let fault = self.clans().check_name(name, me);
                let check = lobby::permitted_clan_name_check(name, refusal(fault));
                self.send_made(who.addr, check);
                Ok(())
            }
            C::CheckTag { tag } => {
                let fault = self.clans().check_tag(tag, me);
                let check = lobby::permitted_clan_tag_check(tag, refusal(fault));
                self.send_made(who.addr, check);
                Ok(())
            }
            C::QueryInfo { what } => {
                self.clan_query(who, *what);
                Ok(())
            }
            C::ReserveName { name, tag } => self.clans().reserve(me, name, tag).map(|clan| {
                self.send(who.addr, lobby::permitted_clan_name_reserved());
                self.clan_push_full(&clan);
            }),
            C::BuyRegistration { value } => self.clan_pay(who, *value),
            C::Create => self.clans().create(me).map(|(clan, _)| {
                log::info!(
                    "{} registered the clan {} [{}] with {} members",
                    who.name,
                    clan.name,
                    clan.tag,
                    clan.members.len()
                );
                for member in &clan.members {
                    self.clan_set_tag(member.account_id, &clan.tag);
                }
                self.clan_push_full(&clan);
            }),
            C::LeaveOrDisband => self.clan_leave(who),
            C::Invite { target_name } => self.clan_invite(who, target_name),
            C::InviteSent { target_name } => {
                // The client's own word that its invite went out: answered
                // with the line it shows for one that did.
                let sent = self.account_named(target_name, me).is_some_and(|target| {
                    self.clans()
                        .invites_for(target)
                        .iter()
                        .any(|invite| invite.inviter == me && !invite.team)
                });
                if sent {
                    self.send(who.addr, lobby::permitted_clan_invite_sent(target_name));
                }
                Ok(())
            }
            C::AnswerInvite { inviter_id, answer } => {
                self.clan_answer_invite(who, *inviter_id, *answer == 0)
            }
            C::SetPermission {
                rank,
                permission,
                value,
            } => self
                .clans()
                .set_permission(me, *rank, *permission, *value != 0)
                .map(|clan| self.clan_push_ranks(&clan, false)),
            // The client waits for `83 49 12` either way: with 1 it asks
            // its player (the member would command the clan) and sends
            // `49 11 <id> 1`; with 0 the rank is up already.
            C::AskRankUp { account_id } => {
                self.clans()
                    .ask_rank_up(me, *account_id)
                    .map(|answer| match answer {
                        RankUp::Confirm => {
                            self.send(who.addr, lobby::permitted_rank_up(true));
                        }
                        RankUp::Done(clan) => {
                            self.send(who.addr, lobby::permitted_rank_up(false));
                            self.clan_push_members(&clan, None);
                        }
                    })
            }
            C::ChangeRank { account_id, up } => {
                let before = self.clans().clan_of(me).map(|clan| clan.leader);
                self.clans().change_rank(me, *account_id, *up).map(|clan| {
                    if Some(clan.leader) == before {
                        self.clan_push_members(&clan, None);
                    } else {
                        // The clan changed hands: who manages it did.
                        log::info!(
                            "{} handed the clan {} over to account {account_id}",
                            who.name,
                            clan.name
                        );
                        self.clan_push_full(&clan);
                    }
                })
            }
            C::Kick { account_id } => self.clans().kick(me, *account_id).map(|clan| {
                let kicked = self.shown_name(*account_id);
                log::info!("{} kicked {kicked} from the clan {}", who.name, clan.name);
                self.clan_none(*account_id, false);
                self.clan_push_members(&clan, None);
                self.clan_line(&clan, line::KICKED, &[&who.name, &kicked]);
                self.clan_team_changed(clan.id, Some(now));
            }),
            C::CreateRank { name } => self
                .clans()
                .create_rank(me, name)
                .map(|clan| self.clan_push_ranks(&clan, true)),
            C::RemoveRank { rank } => self
                .clans()
                .remove_rank(me, *rank)
                .map(|clan| self.clan_push_ranks(&clan, true)),
            C::SetDailyMessage { text } => self.clans().set_daily_message(me, text).map(|clan| {
                let message = ClanInfo::DailyMessage {
                    clan_id: clan.id,
                    text: clan.daily_message.clone(),
                };
                for (_, addr) in self.clan_links(&clan) {
                    self.send_clan(addr, &message);
                }
            }),

            // ---- the clan team ----
            C::CreateTeam => self
                .not_in_squad(me)
                .and_then(|()| self.clans().create_team(me))
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::AskToJoinTeam => self
                .not_in_squad(me)
                .and_then(|()| self.clans().ask_to_join_team(me))
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::InviteToTeam { target_name } => self.clan_team_invite(who, target_name),
            C::AnswerTeamInvite { inviter_id, answer } => {
                let accept = *answer == 0;
                let allowed = if accept {
                    self.not_in_squad(me)
                } else {
                    Ok(())
                };
                allowed
                    .and_then(|()| self.clans().answer_team_invite(me, *inviter_id, accept))
                    .map(|team| {
                        if let Some(team) = team {
                            self.clan_team_changed(team.clan_id, Some(now));
                        }
                    })
            }
            C::LeaveTeam => self
                .clans()
                .leave_team(me)
                .map(|(clan_id, _)| self.clan_team_changed(clan_id, Some(now))),
            // The clan-match window was closed: out of the team, if in one.
            C::MatchClosed => {
                if let Ok((clan_id, _)) = self.clans().leave_team(me) {
                    self.clan_team_changed(clan_id, Some(now));
                }
                Ok(())
            }
            C::DisbandTeam => self
                .clans()
                .disband_team(me)
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::SetTeamMemberStatus {
                member_name,
                status,
            } => self
                .team_member_named(me, member_name, "st_cant_change_status_clan_team_member")
                .and_then(|target| self.clans().set_team_status(me, target, *status))
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::OustCommander => self
                .clans()
                .oust_commander(me)
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::RemoveTeamMember { member_name } => self
                .team_member_named(me, member_name, "st_cant_remove_clan_team_member")
                .and_then(|target| self.clans().remove_team_member(me, target))
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::SetTeamCommander { member_name } => self
                .team_member_named(me, member_name, "st_cant_set_clan_team_commander")
                .and_then(|target| self.clans().set_team_commander(me, target))
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::SoldierReady { ready } => self
                .clans()
                .soldier_ready(me, *ready)
                .map(|team| self.clan_team_changed(team.clan_id, Some(now))),
            C::CommanderReady { ready } => self.clan_commander_ready(who, *ready, now),
        };
        if let Err(error) = result {
            log::info!("{}: {request:?} refused: {}", who.name, error.text_key());
            self.deny(who.addr, id::PROFILE, error.text_key());
        }
    }

    fn not_in_squad(&self, account_id: u32) -> Result<(), ClanError> {
        match self.store.squads().of(account_id) {
            Some(_) => Err(ClanError(IN_SQUAD)),
            None => Ok(()),
        }
    }

    /// The member of `me`'s clan team the client names.
    fn team_member_named(
        &self,
        me: u32,
        name: &str,
        refusal: &'static str,
    ) -> Result<u32, ClanError> {
        self.clans()
            .clan_of(me)
            .and_then(|clan| self.clans().team(clan.id))
            .and_then(|team| {
                team.members
                    .iter()
                    .map(|member| member.account_id)
                    .find(|member| self.shown_name(*member).eq_ignore_ascii_case(name))
            })
            .ok_or(ClanError(refusal))
    }

    /// `49 0E <what>`: the clan window (0) or the clan-team window (4)
    /// opened.
    fn clan_query(&mut self, who: &Who, what: u8) {
        let Some(clan) = self.clans().clan_of(who.account_id) else {
            self.send_clan(who.addr, &ClanInfo::None);
            return;
        };
        if what == 4 && clan.registered {
            let team = self.clans().team(clan.id);
            let info = self.clan_team_info(clan.id, team.as_ref(), who.account_id);
            self.send_clan(who.addr, &info);
        } else {
            let full = self.clan_full(&clan, who.account_id);
            self.send_clan(who.addr, &full);
        }
    }

    /// `49 0C <silver>`: pay into the registration, as much of it as is
    /// still due and the account has.
    fn clan_pay(&mut self, who: &Who, value: u32) -> Result<(), ClanError> {
        let me = who.account_id;
        let taken = {
            let mut account = who.account.locked();
            let taken = self.clans().payable(me, value.min(account.generic_money))?;
            account.generic_money -= taken;
            taken
        };
        let paid = self.clans().pay(me, taken);
        if paid.is_err() {
            // The registration went between the two looks: the silver back.
            let mut account = who.account.locked();
            account.generic_money = account.generic_money.saturating_add(taken);
        }
        let clan = paid?;
        log::info!(
            "{} paid {taken} silver into the registration of {} ({} so far)",
            who.name,
            clan.name,
            clan.money
        );
        self.push_account(who);
        self.clan_push_full(&clan);
        Ok(())
    }

    /// `49 0D`: leave, or (the commander, the founder) disband.
    fn clan_leave(&mut self, who: &Who) -> Result<(), ClanError> {
        match self.clans().leave_or_disband(who.account_id)? {
            Departure::Disbanded(clan) => {
                log::info!("{} disbanded the clan {}", who.name, clan.name);
                // Its team went with it.
                self.clan_team_left_queue(clan.id);
                for member in clan.accepted() {
                    let told = clan.registered && member.account_id != who.account_id;
                    self.clan_none(member.account_id, told);
                }
            }
            Departure::Left(clan) => {
                log::info!("{} left the clan {}", who.name, clan.name);
                self.clan_none(who.account_id, false);
                self.clan_push_members(&clan, None);
                if clan.registered {
                    self.clan_line(&clan, line::LEFT, &[&who.name]);
                    self.clan_team_changed(clan.id, None);
                }
            }
        }
        Ok(())
    }

    fn permit(&mut self, inviter: u32, target: u32, team: bool) {
        let permitted = &mut self.clan.permitted;
        permitted.retain(|kept| *kept != (inviter, target, team));
        if permitted.len() >= MAX_PERMITTED {
            permitted.remove(0);
        }
        permitted.push((inviter, target, team));
    }

    /// `49 0F <name>`: may the player be invited? If so the client is given
    /// what it sends as the invite (`4D`, type 7): the clan's id as its
    /// token [choice: the client only hands it back], the name, and the
    /// inviter's own name as the text (what the invite shows it as).
    fn clan_invite(&mut self, who: &Who, target_name: &str) -> Result<(), ClanError> {
        let refused = ClanError("st_cant_invite_player_to_clan");
        let target = self
            .account_named(target_name, who.account_id)
            .ok_or(refused)?;
        let clan = self.clans().may_invite(who.account_id, target)?;
        self.permit(who.account_id, target, false);
        self.send(
            who.addr,
            lobby::permitted_clan_invite(Some((clan.id, target_name, &who.name))),
        );
        Ok(())
    }

    /// `49 19 <name>`: the same for the clan team (`4D`, type 10), for a
    /// clan mate in the lobby.
    fn clan_team_invite(&mut self, who: &Who, target_name: &str) -> Result<(), ClanError> {
        let refused = ClanError("st_cant_invite_player_to_clan_team");
        let target = self
            .account_named(target_name, who.account_id)
            .filter(|target| self.by_account.contains_key(target))
            .ok_or(refused)?;
        let team = self.clans().may_invite_to_team(who.account_id, target)?;
        self.permit(who.account_id, target, true);
        self.send(
            who.addr,
            lobby::permitted_clan_team_invite(team.id, target_name, &who.name),
        );
        Ok(())
    }

    /// The long `4D` of type 7 or 10: the invite the server permitted goes
    /// to the player as an important message, whose sender id it answers
    /// with (`49 0A`, `49 1A`). One nobody permitted is dropped.
    pub(super) fn clan_important(&mut self, who: &Who, target_name: &str, kind: u8) {
        let team = kind == important::CLAN_TEAM_INVITE;
        let me = who.account_id;
        let target = self.account_named(target_name, me);
        let permitted = target.and_then(|target| {
            let index = self
                .clan
                .permitted
                .iter()
                .position(|kept| *kept == (me, target, team))?;
            self.clan.permitted.remove(index);
            Some(target)
        });
        let Some(target) = permitted else {
            log::info!(
                "{}: an invite of type {kind} for {target_name} the server did not permit; dropped",
                who.name
            );
            return;
        };
        let message_id = self.clan_message_id();
        let invited = if team {
            self.clans()
                .invite_to_team(me, target, message_id, &who.name)
                .and_then(|team| {
                    self.clans()
                        .clan(team.clan_id)
                        .ok_or(ClanError("st_cant_invite_player_to_clan_team"))
                })
        } else {
            self.clans().invite(me, target, message_id, &who.name)
        };
        match invited {
            Ok(clan) => {
                log::info!(
                    "{} invited {target_name} to its clan{}",
                    who.name,
                    if team { " team" } else { "" }
                );
                if let Some(addr) = self.by_account.get(&target).copied() {
                    let message = self.clan_invite_message(&clan, me, team, message_id);
                    self.send(addr, message);
                }
                // A registration lists the invited player as pending.
                if !team && !clan.registered {
                    self.clan_push_full(&clan);
                }
            }
            Err(error) => self.deny(who.addr, id::PROFILE, error.text_key()),
        }
    }

    /// `49 0A <inviter> <answer>`.
    fn clan_answer_invite(
        &mut self,
        who: &Who,
        inviter: u32,
        accept: bool,
    ) -> Result<(), ClanError> {
        let me = who.account_id;
        let (clan, joined) = self.clans().answer_invite(me, inviter, accept)?;
        log::info!(
            "{} {} the invite to the clan {}",
            who.name,
            if accept { "accepted" } else { "declined" },
            clan.name
        );
        if !clan.registered {
            // The founder sees the answer; who accepted sees the
            // registration.
            self.clan_push_full(&clan);
            return Ok(());
        }
        if joined {
            self.clan_set_tag(me, &clan.tag);
            let full = self.clan_full(&clan, me);
            self.send_clan(who.addr, &full);
            if !clan.daily_message.is_empty() {
                self.send_clan(
                    who.addr,
                    &ClanInfo::DailyMessage {
                        clan_id: clan.id,
                        text: clan.daily_message.clone(),
                    },
                );
            }
            if let Some(team) = self.clans().team(clan.id) {
                let info = self.clan_team_info(clan.id, Some(&team), me);
                self.send_clan(who.addr, &info);
            }
            self.clan_push_members(&clan, Some(me));
            self.clan_line(&clan, line::JOINED, &[&who.name]);
        }
        Ok(())
    }

    /// A member earned experience in a match: its clan's grows with it,
    /// and a new level is told to the members.
    pub(super) fn clan_experience(&mut self, account_id: u32, experience: u32) {
        let Some((clan, Some(level))) = self.clans().add_experience(account_id, experience) else {
            return;
        };
        log::info!("The clan {} reached level {level}", clan.name);
        self.clan_push_full(&clan);
        self.clan_line(&clan, line::LEVEL, &[&level.to_string()]);
    }

    // ---------------------------------------------------------------- chat

    /// Who a line on the clan channel (3) or the clan-team channel (9)
    /// from `sender` is for; `None` for another channel or a sender with
    /// no clan (team).
    pub(super) fn clan_chat_members(&self, sender: u32, chat_channel: u8) -> Option<Vec<u32>> {
        match chat_channel {
            channel::CLAN => self
                .clans()
                .clan_of(sender)
                .filter(|clan| clan.registered)
                .map(|clan| clan.accepted().map(|member| member.account_id).collect()),
            channel::CLAN_TEAM => self
                .clans()
                .team_of(sender)
                .map(|team| team.taken().map(|member| member.account_id).collect()),
            _ => None,
        }
    }

    // ---------------------------------------------------- clan-team Play

    /// `30 <kit>`: clan-team Play, for the team's commander and lineup:
    /// `83 30`, on which the client toggles its ready state.
    pub(super) fn clan_play(&mut self, who: &Who, kit_id: u32) {
        let unix = quests::now();
        let checked = {
            let mut account = who.account.locked();
            account.settle_active_kit(unix);
            rules::check_play(&account, kit_id, unix)
        };
        let key = match checked {
            Ok(()) => match self.clans().clan_play(who.account_id, kit_id) {
                Ok(_) => {
                    self.send(who.addr, lobby::permitted_clan_play());
                    return;
                }
                Err(error) => error.text_key(),
            },
            Err(error @ (rules::ActionError::NoWeapon | rules::ActionError::NoAmmo)) => {
                error.text_key()
            }
            Err(_) => "st_profile_check_failed",
        };
        log::info!("{}: clan Play refused: {key}", who.name);
        self.deny(who.addr, id::CLAN_PLAY, key);
    }

    /// `49 21 <ready>`: the commander is ready, and the team waits for a
    /// match; or not any more.
    fn clan_commander_ready(
        &mut self,
        who: &Who,
        ready: bool,
        now: Instant,
    ) -> Result<(), ClanError> {
        let team = self.clans().commander_ready(who.account_id, ready)?;
        // Everyone who plays must be here, with a kit to play, in no match
        // and watching none.
        let all_here = team.battle().all(|member| {
            member.kit_id != 0
                && self.who_is(member.account_id).is_some_and(|mate| {
                    self.store.matchmaking().seat(mate.session_id).is_none()
                        && !self.clan_watching(&mate)
                })
        });
        if ready && !all_here {
            let _ = self.clans().commander_ready(who.account_id, false);
            return Err(ClanError("st_cant_change_ready_state"));
        }
        self.clan_team_changed(team.clan_id, Some(now));
        Ok(())
    }

    /// `31`: a player left the queue. If its clan team waited, it does not
    /// any more.
    pub(super) fn clan_left_queue(&mut self, account_id: u32) {
        if let Some(team) = self.clans().member_unready(account_id) {
            self.clan_team_changed(team.clan_id, None);
        }
    }

    /// A clan's team changed: it joins or leaves the teams waiting for a
    /// match as its commander's ready state says, every member of the clan
    /// in the lobby is shown it, and a match is made if two teams wait.
    /// `now` is `None` when the change can only have taken it out.
    fn clan_team_changed(&mut self, clan_id: u32, now: Option<Instant>) {
        let team = self.clans().team(clan_id);
        let queued = team.as_ref().is_some_and(Team::is_queued);
        let waits = self
            .clan
            .waiting
            .iter()
            .any(|waiting| waiting.clan_id == clan_id);
        match (queued, waits, team, now) {
            (true, false, Some(team), Some(now)) => self.clan_team_waits(&team, now),
            (false, true, ..) => self.clan_team_left_queue(clan_id),
            _ => {}
        }
        self.clan_push_team(clan_id);
        if let Some(now) = now {
            self.clan_matches(now);
        }
    }

    /// The team waits for a match: its players are in the queue.
    fn clan_team_waits(&mut self, team: &Team, now: Instant) {
        let mut accounts = Vec::new();
        for member in team.battle() {
            let Some(who) = self.who_is(member.account_id) else {
                continue;
            };
            if let Some(account) = self.store.account(who.account_id) {
                account.locked().play = Some(session::Player {
                    session_id: who.session_id,
                    account_id: who.account_id,
                    profile_id: member.kit_id,
                });
            }
            let order_id = self.next_order_id;
            // 0xFFFFFFFF is the client's "not queued".
            self.next_order_id = (self.next_order_id + 1) % 0x7FFF_FFFF + 1;
            if let Some(player) = self.player_mut(who.addr) {
                player.queued_since = Some(now);
                player.order_id = order_id;
            }
            accounts.push(who.account_id);
            self.queue_status(who.addr, now);
        }
        log::info!(
            "Clan team {} of clan {} waits for a match with {} players",
            team.id,
            team.clan_id,
            accounts.len()
        );
        self.clan.waiting.push(Waiting {
            clan_id: team.clan_id,
            accounts,
        });
    }

    /// The team does not wait any more: its players who still do are back
    /// in the lobby.
    fn clan_team_left_queue(&mut self, clan_id: u32) {
        let Some(index) = self
            .clan
            .waiting
            .iter()
            .position(|waiting| waiting.clan_id == clan_id)
        else {
            return;
        };
        let waiting = self.clan.waiting.remove(index);
        for account_id in waiting.accounts {
            let Some(addr) = self.by_account.get(&account_id).copied() else {
                continue;
            };
            let waited = self
                .player(addr)
                .is_some_and(|player| player.queued_since.is_some() && player.in_match.is_none());
            if !waited {
                continue;
            }
            if let Some(player) = self.player_mut(addr) {
                player.queued_since = None;
            }
            if let Some(account) = self.store.account(account_id) {
                account.locked().play = None;
            }
            self.send(addr, lobby::client_state(&ClientState::Lobby, None));
        }
    }

    /// Whether the player watches something meanwhile: a match being
    /// played (a seat would never be sent to it: `queue_status`), or a
    /// replay session of its own. A clan match seats nobody who does
    /// (`docs/notes/queue-spectate.md`).
    fn clan_watching(&self, who: &Who) -> bool {
        self.store.matchmaking().watched(who.session_id).is_some()
            || self
                .replays
                .is_some_and(|replays| replays.watches().holds(who.account_id))
    }

    /// The players of a waiting team, if all of them are still in the
    /// lobby and watch nothing.
    fn clan_team_players(&self, clan_id: u32) -> Option<Vec<session::Player>> {
        let team = self.clans().team(clan_id).filter(Team::is_queued)?;
        let players: Vec<_> = team
            .battle()
            .map(|member| {
                let who = self
                    .who_is(member.account_id)
                    .filter(|who| !self.clan_watching(who))?;
                Some(session::Player {
                    session_id: who.session_id,
                    account_id: who.account_id,
                    profile_id: member.kit_id,
                })
            })
            .collect::<Option<_>>()?;
        self.pool_for_sessions(players.iter().map(|player| player.session_id))?;
        Some(players)
    }

    /// Two clan teams that wait play each other: one match, a side each.
    /// A team that can not play after all (a player went, watches
    /// something, or sits in a match it left) is not ready any more.
    pub(super) fn clan_matches(&mut self, now: Instant) {
        // A team that stopped waiting behind the lobby's back.
        let stale: Vec<u32> = self
            .clan
            .waiting
            .iter()
            .map(|waiting| waiting.clan_id)
            .filter(|clan_id| self.clan_team_players(*clan_id).is_none())
            .collect();
        for clan_id in stale {
            self.clans().team_unready(clan_id);
            self.clan_team_left_queue(clan_id);
            self.clan_push_team(clan_id);
        }
        while self.clan.waiting.len() >= 2 {
            let candidates: Vec<_> = self
                .clan
                .waiting
                .iter()
                .filter_map(|waiting| {
                    let players = self.clan_team_players(waiting.clan_id)?;
                    let pool =
                        self.pool_for_sessions(players.iter().map(|player| player.session_id))?;
                    Some((waiting.clan_id, pool, players))
                })
                .collect();
            let pair = candidates.iter().enumerate().find_map(|(index, first)| {
                candidates[index + 1..]
                    .iter()
                    .find(|other| other.1 == first.1)
                    .map(|second| (first, second))
            });
            let Some((first, second)) = pair else {
                break;
            };
            let clans = [first.0, second.0];
            let pool = first.1;
            let players = [&first.2, &second.2];
            let made =
                self.store
                    .matchmaking()
                    .seat_teams_in_pool([players[0], players[1]], now, pool);
            match made {
                Some(match_id) => {
                    log::info!(
                        "Clan match {match_id:#x}: clan {} ({} players) against clan {} ({})",
                        clans[0],
                        players[0].len(),
                        clans[1],
                        players[1].len()
                    );
                    self.clan
                        .waiting
                        .retain(|waiting| !clans.contains(&waiting.clan_id));
                    for clan_id in clans {
                        self.clans().team_unready(clan_id);
                        self.clan_push_team(clan_id);
                    }
                    // "Match found" and where it is, as for any match.
                    for player in players.iter().flat_map(|players| players.iter()) {
                        if let Some(addr) = self.by_account.get(&player.account_id).copied() {
                            self.queue_status(addr, now);
                        }
                    }
                }
                None => {
                    log::warn!(
                        "No clan match for clans {} and {}: a player of theirs sits in a match",
                        clans[0],
                        clans[1]
                    );
                    for clan_id in clans {
                        self.clans().team_unready(clan_id);
                        self.clan_team_left_queue(clan_id);
                        self.clan_push_team(clan_id);
                    }
                }
            }
        }
    }

    /// The clan teams waiting, for the tests.
    #[cfg(test)]
    pub(super) fn clan_teams_waiting(&self) -> Vec<u32> {
        self.clan
            .waiting
            .iter()
            .map(|waiting| waiting.clan_id)
            .collect()
    }
}
