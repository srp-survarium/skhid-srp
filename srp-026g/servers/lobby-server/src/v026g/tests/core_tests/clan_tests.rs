//! Clans through the lobby's core (`crate::v026g::clans`), with the scripted
//! clients of `core_tests`: every push goes through the strict client
//! model, and the messages a clan adds are compared byte for byte.

use std::time::Duration;

use session::v026g::clans::{Config, permission, role, status};
use survarium::v026g::lobby::client_model::{Decoded, Permitted};
use survarium::v026g::lobby::{
    ClanApplicant, ClanInfo, ClanMember, ClanTeamSeat, ClientState, NameRefusal, channel,
    important, sender,
};

use super::super::client::request;
use super::super::{denial, find, sent_to_match, signed_in};
use super::{World, queue_state};

// ------------------------------------------------------------ the requests

fn string(out: &mut Vec<u8>, text: &str) {
    out.push(text.len() as u8);
    out.extend_from_slice(text.as_bytes());
}

/// `49 <sub> <tail>`.
fn clan(sub: u8, tail: &[u8]) -> (u8, Vec<u8>) {
    let mut body = vec![sub];
    body.extend_from_slice(tail);
    (0x49, body)
}

/// `49 <sub> <string> <tail>`.
fn named(sub: u8, name: &str, tail: &[u8]) -> (u8, Vec<u8>) {
    let mut body = vec![sub];
    string(&mut body, name);
    body.extend_from_slice(tail);
    (0x49, body)
}

/// `49 <sub> <u32 account> <tail>`.
fn about(sub: u8, account_id: u32, tail: &[u8]) -> (u8, Vec<u8>) {
    let mut body = vec![sub];
    body.extend(account_id.to_le_bytes());
    body.extend_from_slice(tail);
    (0x49, body)
}

/// `49 06 <name> <abbreviation>`.
fn reserve(name: &str, tag: &str) -> (u8, Vec<u8>) {
    let mut body = vec![6];
    string(&mut body, name);
    string(&mut body, tag);
    (0x49, body)
}

/// `49 0C <silver>`.
fn pay(silver: u32) -> (u8, Vec<u8>) {
    clan(0x0C, &silver.to_le_bytes())
}

/// `49 16`: `u16 count` and UTF-16LE units.
fn daily_message(text: &str) -> (u8, Vec<u8>) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut body = vec![0x16];
    body.extend((units.len() as u16).to_le_bytes());
    body.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    (0x49, body)
}

/// The long `4D`: an invite with the token of `83 49 0F` / `83 49 19`.
fn invite_message(target: &str, kind: u8, text: &str, token: u32) -> (u8, Vec<u8>) {
    let mut body = 0_u32.to_le_bytes().to_vec();
    string(&mut body, target);
    body.push(kind);
    string(&mut body, text);
    body.extend(token.to_le_bytes());
    (0x4D, body)
}

/// `30 <kit>`.
fn clan_play(kit_id: u32) -> (u8, Vec<u8>) {
    (0x30, kit_id.to_le_bytes().to_vec())
}

// ------------------------------------------------------------ what came

/// Every clan info (0x9A), in order.
fn infos(decoded: &[Decoded]) -> Vec<ClanInfo> {
    decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::ClanInfo(info) => Some(info.clone()),
            _ => None,
        })
        .collect()
}

/// The bodies of the clan service's chat lines.
fn clan_lines(decoded: &[Decoded]) -> Vec<String> {
    decoded
        .iter()
        .filter_map(|message| match message {
            Decoded::ChatText {
                sender_type,
                channel: 3,
                body,
                sender_clan_tag: None,
                ..
            } if *sender_type == sender::CLAN => Some(body.clone()),
            _ => None,
        })
        .collect()
}

type TeamShown = (Vec<(u32, u8, bool)>, Option<ClanTeamSeat>);

/// The clan's team as the client was last shown it: `(account, role,
/// ready)` of everyone listed, and the client's own place.
fn team_shown(decoded: &[Decoded]) -> Option<TeamShown> {
    infos(decoded)
        .into_iter()
        .rev()
        .find_map(|info| match info {
            ClanInfo::ClanTeam { members, seat, .. } => Some((
                members
                    .iter()
                    .map(|member| (member.account_id, member.role, member.ready))
                    .collect(),
                seat,
            )),
            _ => None,
        })
}

/// The members a client was last shown: `(account, rank, online)`.
fn members_shown(decoded: &[Decoded]) -> Option<Vec<(u32, u8, bool)>> {
    let rows = |members: &[ClanMember]| {
        members
            .iter()
            .map(|member| (member.account_id, member.rank, member.online))
            .collect()
    };
    infos(decoded)
        .into_iter()
        .rev()
        .find_map(|info| match info {
            ClanInfo::Members { members, .. } | ClanInfo::Registered { members, .. } => {
                Some(rows(&members))
            }
            _ => None,
        })
}

/// The clan rules these tests were written with, said outright: a clan
/// for 1,000 silver (a new account's 2,000 pays it) and 100 members.
/// srp's defaults are other numbers ([`Config::default`]).
fn test_config() -> Config {
    Config {
        cost: 1_000,
        players_required: 1,
        team_min: 1,
        team_max: 10,
        max_members: 100,
    }
}

impl World {
    /// A lobby with the clan rules given, whatever the environment
    /// says.
    fn with_clans(config: Config) -> Self {
        let world = World::new();
        world.accounts.clans().set_config(config);
        world
    }

    fn id(&self, index: usize) -> u32 {
        self.ids[index].0
    }

    /// The last message with this id the lobby pushed to a client, as it
    /// goes on the wire without the transport's byte: `id body`.
    fn pushed(&self, index: usize, id: u8) -> Vec<u8> {
        let link = &self.core.links[&self.clients[index].0];
        let message = link
            .pushed
            .iter()
            .rev()
            .find(|message| message.id_byte() == id)
            .expect("a message with that id");
        let mut bytes = vec![id];
        bytes.extend_from_slice(message.body());
        bytes
    }

    /// An invite to the clan as the client makes it: ask (`49 0F`), send
    /// what the server permitted (the long `4D`), say it went (`49 0B`).
    /// Returns the id of the important message the target got, and what
    /// the inviter was sent meanwhile.
    fn invite(&mut self, inviter: usize, target: usize, target_name: &str) -> (u32, Vec<Decoded>) {
        self.take(inviter);
        self.ask(inviter, named(0x0F, target_name, &[]));
        let decoded = self.take(inviter);
        let (token, name, text) = find(&decoded, |message| match message {
            Decoded::Permitted(Permitted::ClanInvite(Some(invite))) => Some(invite.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no invite permitted: {decoded:?}"));
        assert_eq!(name, target_name);
        self.ask(
            inviter,
            invite_message(&name, important::CLAN_INVITE, &text, token),
        );
        self.ask(inviter, named(0x0B, &name, &[]));
        let decoded = self.take(inviter);
        assert!(
            decoded.iter().any(|message| matches!(
                message,
                Decoded::Permitted(Permitted::ClanInviteSent { target_name }) if *target_name == name
            )),
            "{decoded:?}"
        );
        let inviter_id = self.id(inviter);
        let message_id = find(&self.clients[target].1.decoded, |message| match message {
            Decoded::Important {
                kind: important::CLAN_INVITE,
                sender_id,
                message_id,
                duplicate: false,
                ..
            } if *sender_id == inviter_id => Some(*message_id),
            _ => None,
        })
        .expect("the invite reached its player");
        (message_id, decoded)
    }

    /// The target's answer: `49 0A <inviter> <answer>`, then `50`.
    fn answer_invite(&mut self, target: usize, inviter: usize, message_id: u32, accept: bool) {
        let answer = u8::from(!accept);
        let inviter_id = self.id(inviter);
        self.ask(target, about(0x0A, inviter_id, &[answer]));
        assert!(self.client(target).model.answer_important(message_id));
        self.ask(target, request::answer(message_id, answer));
    }

    /// A registered clan: reserved by `founder`, joined by `members`, paid
    /// and registered. Returns its id; what the clients were sent is taken.
    fn register(
        &mut self,
        name: &str,
        tag: &str,
        founder: usize,
        members: &[(usize, &str)],
    ) -> u32 {
        self.ask(founder, reserve(name, tag));
        for &(member, member_name) in members {
            let (message_id, _) = self.invite(founder, member, member_name);
            self.answer_invite(member, founder, message_id, true);
        }
        let cost = self.accounts.clans().config().cost;
        self.ask(founder, pay(cost));
        self.ask(founder, clan(7, &[]));
        let clan = self
            .accounts
            .clans()
            .clan_of(self.id(founder))
            .expect("the founder's clan");
        assert!(clan.registered, "{clan:?}");
        assert_eq!(clan.members.len(), members.len() + 1);
        self.take(founder);
        for &(member, _) in members {
            self.take(member);
        }
        clan.id
    }

    /// An invite to the clan team as the client makes it (`49 19`, the
    /// long `4D` of type 10), accepted (`49 1A`, `50`).
    fn join_team(&mut self, commander: usize, target: usize, target_name: &str) {
        self.take(commander);
        self.ask(commander, named(0x19, target_name, &[]));
        let decoded = self.take(commander);
        let (token, name, text) = find(&decoded, |message| match message {
            Decoded::Permitted(Permitted::ClanTeamInvite {
                extra,
                target_name,
                text,
            }) => Some((*extra, target_name.clone(), text.clone())),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no team invite permitted: {decoded:?}"));
        self.ask(
            commander,
            invite_message(&name, important::CLAN_TEAM_INVITE, &text, token),
        );
        let commander_id = self.id(commander);
        let message_id = find(&self.clients[target].1.decoded, |message| match message {
            Decoded::Important {
                kind: important::CLAN_TEAM_INVITE,
                sender_id,
                message_id,
                ..
            } if *sender_id == commander_id => Some(*message_id),
            _ => None,
        })
        .expect("the team invite reached its player");
        self.ask(target, about(0x1A, commander_id, &[0]));
        assert!(self.client(target).model.answer_important(message_id));
        self.ask(target, request::answer(message_id, 0));
    }

    /// Clan Play as the client does it: `30`, and on `83 30` the ready
    /// toggle (`49 21` the commander's, `49 20` a soldier's).
    fn clan_ready(&mut self, index: usize, commander: bool, ready: bool) {
        self.ask(index, clan_play(1));
        let decoded = self.take(index);
        assert!(
            decoded
                .iter()
                .any(|message| matches!(message, Decoded::Permitted(Permitted::ClanPlay))),
            "{decoded:?}"
        );
        let toggle = if commander {
            clan(0x21, &[u8::from(ready)])
        } else {
            clan(0x20, &[if ready { 0xFF } else { 0 }])
        };
        self.ask(index, toggle);
    }
}

fn hex(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
        } else if c == '\'' {
            // A string with its length byte.
            let end = rest[1..].find('\'').expect("a closing quote") + 1;
            bytes.push((end - 1) as u8);
            bytes.extend_from_slice(&rest.as_bytes()[1..end]);
            rest = &rest[end + 1..];
        } else {
            bytes.push(u8::from_str_radix(&rest[..2], 16).expect("two hex digits"));
            rest = &rest[2..];
        }
    }
    bytes
}

// ---------------------------------------------------------------- tests

#[test]
fn a_clan_is_reserved_filled_paid_and_registered() {
    let mut world = World::with_clans(Config {
        players_required: 2,
        cost: 1_500,
        ..test_config()
    });
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let (alice_id, bob_id, eve_id) = (world.id(alice), world.id(bob), world.id(eve));
    // A player of no clan is sent no clan at sign-in: its client starts
    // with none. The rules' numbers (0x9C) are the environment's.
    assert!(!world.client(alice).ids.contains(&0x9A));
    let shared = Config::shared();
    assert_eq!(
        world.pushed(alice, 0x9C),
        [
            0x9C,
            shared.players_required,
            shared.team_min,
            shared.team_max,
            0
        ]
    );
    for index in [alice, bob, eve] {
        world.take(index);
    }

    // The window asks about the name and the abbreviation as they are
    // typed.
    world.ask(alice, request::profile_text(8, "Stalkers"));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::Permitted(Permitted::ClanNameCheck { name, refusal: None })] if name == "Stalkers"
    ));
    world.ask(alice, request::profile_text(9, "ST"));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::Permitted(Permitted::ClanTagCheck {
            refusal: Some(NameRefusal::Short),
            ..
        })]
    ));
    world.ask(alice, request::profile_text(9, "S T K"));
    assert!(matches!(
        &world.take(alice)[..],
        [Decoded::Permitted(Permitted::ClanTagCheck {
            refusal: Some(NameRefusal::Space),
            ..
        })]
    ));

    // Reserved: `83 49 06`, and the registration as its founder sees it.
    world.ask(alice, reserve("Stalkers", "STK"));
    let decoded = world.take(alice);
    assert_eq!(world.pushed(alice, 0x83), hex("83 49 06"));
    assert!(matches!(
        decoded[0],
        Decoded::Permitted(Permitted::ClanNameReserved)
    ));
    let [
        ClanInfo::Registering {
            clan_id,
            name,
            tag,
            ranks,
            members,
            flag: true,
            paid: false,
            money: 0,
            reg_state: 3,
            ..
        },
    ] = &infos(&decoded)[..]
    else {
        panic!("{decoded:?}");
    };
    let clan_id = *clan_id;
    assert_eq!((name.as_str(), tag.as_str()), ("Stalkers", "STK"));
    assert_eq!(ranks.len(), 4);
    assert_eq!(
        members,
        &[ClanApplicant {
            account_id: alice_id,
            name: "Alice".into(),
            rank: 0,
            status: status::ACCEPTED,
        }]
    );
    // Its name is taken from now on, however it is written.
    world.ask(bob, request::profile_text(8, "stalkers"));
    assert!(matches!(
        &world.take(bob)[..],
        [Decoded::Permitted(Permitted::ClanNameCheck {
            refusal: Some(NameRefusal::Taken),
            ..
        })]
    ));
    // No tag before the clan is registered, and no registering yet.
    assert_eq!(world.account(alice).clan_tag, "");
    world.ask(alice, clan(7, &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_create_clan")
    );

    // Bob and Eve are invited: the founder lists them as pending.
    let (bob_invite, decoded) = world.invite(alice, bob, "Bob");
    assert_eq!(
        world.pushed(alice, 0x83),
        hex("83 49 0B 'Bob'"),
        "the answer to the client's `49 0B`"
    );
    let Some(ClanInfo::Registering { members, .. }) = infos(&decoded).pop() else {
        panic!("{decoded:?}");
    };
    assert_eq!(members[1].account_id, bob_id);
    assert_eq!(members[1].status, status::PENDING);
    // The invite as the client words it: the sender line is
    // `[abbreviation] text`, the body `invites you to clan "sender name"`;
    // the sender's id is the inviter's, which the answer names.
    let mut expected = hex("A4 05");
    expected.extend(alice_id.to_le_bytes());
    expected.extend(hex("'Stalkers' 'STK' 07 'Alice'"));
    expected.extend(bob_invite.to_le_bytes());
    assert_eq!(world.pushed(bob, 0xA4), expected);
    let (eve_invite, _) = world.invite(alice, eve, "Eve");
    world.take(bob);
    world.take(eve);

    // Bob accepts and sees the registration as an invited player; Eve
    // declines and sees nothing.
    world.answer_invite(bob, alice, bob_invite, true);
    world.answer_invite(eve, alice, eve_invite, false);
    let statuses = |decoded: &[Decoded]| match infos(decoded).pop() {
        Some(ClanInfo::Registering { members, flag, .. }) => (
            flag,
            members
                .iter()
                .map(|member| member.status)
                .collect::<Vec<u8>>(),
        ),
        other => panic!("{other:?}"),
    };
    assert_eq!(statuses(&world.take(bob)), (false, vec![0, 0, 1]));
    assert_eq!(statuses(&world.take(alice)), (true, vec![0, 0, 1]));
    assert!(infos(&world.take(eve)).is_empty());
    assert_eq!(world.accounts.clans().clan_of(eve_id), None);

    // Paying: the founder some, the invited player the rest and no more.
    let silver = world.account(alice).generic_money;
    world.ask(alice, pay(1_000));
    assert_eq!(world.account(alice).generic_money, silver - 1_000);
    let decoded = world.take(alice);
    assert!(matches!(
        infos(&decoded).pop(),
        Some(ClanInfo::Registering {
            money: 1_000,
            paid: false,
            ..
        })
    ));
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Money { .. }))
    );
    world.ask(bob, pay(5_000));
    assert_eq!(world.account(bob).generic_money, silver - 500);
    for index in [alice, bob] {
        assert!(matches!(
            infos(&world.take(index)).pop(),
            Some(ClanInfo::Registering {
                money: 1_500,
                paid: true,
                ..
            })
        ));
    }
    world.ask(bob, pay(1));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_buy_clan_registration")
    );

    // Registered by its founder alone.
    world.ask(bob, clan(7, &[]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_create_clan")
    );
    world.ask(alice, clan(7, &[]));
    let decoded = world.take(alice);
    let Some(ClanInfo::Registered {
        members,
        unknown: 0,
        flag: true,
        level: 1,
        progress,
        ..
    }) = infos(&decoded).pop()
    else {
        panic!("{decoded:?}");
    };
    assert_eq!(progress, [100, 0, 10_000, 0]);
    assert_eq!(
        members
            .iter()
            .map(|member| (member.account_id, member.rank, member.online))
            .collect::<Vec<_>>(),
        [(alice_id, 0, true), (bob_id, 3, true)]
    );
    // Bob is the second row, and manages nothing.
    assert!(matches!(
        infos(&world.take(bob)).pop(),
        Some(ClanInfo::Registered {
            unknown: 1,
            flag: false,
            ..
        })
    ));
    // The members carry the abbreviation; their clients were sent their
    // accounts with it.
    assert_eq!(world.account(alice).clan_tag, "STK");
    assert_eq!(world.account(bob).clan_tag, "STK");
    assert_eq!(world.account(eve).clan_tag, "");
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::Account(account) if account.body.clan_tag == "STK"
    )));

    // The whole clan, byte for byte, as its founder got it.
    let level = world.account(alice).level();
    let mut expected = hex("9A 00");
    expected.extend(clan_id.to_le_bytes());
    expected.extend(hex("'Stalkers' 'STK' 01
         04 'commander' 1F 01  'assistant' 1F 01  'warlord' 11 01  'soldier' 00 01
         02000000"));
    expected.extend(alice_id.to_le_bytes());
    expected.extend(hex("'Alice' 00"));
    expected.extend([level, 1]);
    expected.extend(bob_id.to_le_bytes());
    expected.extend(hex("'Bob' 03"));
    expected.extend([level, 1]);
    // The founder's row, it commands; level 1, 100 members, 0..10000, 0.
    expected.extend(hex("0000 01  01 64000000 00000000 10270000 00000000"));
    assert_eq!(world.pushed(alice, 0x9A), expected);
    world.assert_clean();
}

#[test]
fn members_are_invited_ranked_and_kicked_and_the_clan_is_disbanded() {
    let mut world = World::with_clans(test_config());
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let dave = world.connect("Dave");
    let (alice_id, bob_id, eve_id, dave_id) = (
        world.id(alice),
        world.id(bob),
        world.id(eve),
        world.id(dave),
    );
    let clan_id = world.register("Stalkers", "STK", alice, &[(bob, "Bob"), (eve, "Eve")]);
    let all = [alice, bob, eve, dave];
    world.take(dave);

    // A soldier invites nobody; a member of a clan is not invited.
    world.ask(bob, named(0x0F, "Dave", &[]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_invite_player_to_clan")
    );
    world.ask(alice, named(0x0F, "Bob", &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_player_is_already_in_clan")
    );
    world.ask(alice, named(0x0F, "Nobody", &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_invite_player_to_clan")
    );
    // An invite the server did not permit goes nowhere.
    world.ask(
        bob,
        invite_message("Dave", important::CLAN_INVITE, "Stalkers", clan_id),
    );
    assert!(world.take(dave).is_empty());

    // The commander invites Dave, who joins: the clan for him, the
    // members and a line of the clan service for the others.
    let (dave_invite, _) = world.invite(alice, dave, "Dave");
    // (`83 49 0F`: the clan's id is the token.)
    world.answer_invite(dave, alice, dave_invite, true);
    let decoded = world.take(dave);
    assert!(matches!(
        infos(&decoded).first(),
        Some(ClanInfo::Registered {
            unknown: 3,
            flag: false,
            ..
        })
    ));
    assert_eq!(world.account(dave).clan_tag, "STK");
    for index in [alice, bob, eve] {
        let decoded = world.take(index);
        assert!(
            matches!(
                infos(&decoded)[..],
                [ClanInfo::Members { clan_id: id, ref members, .. }] if id == clan_id && members.len() == 4
            ),
            "{decoded:?}"
        );
        assert_eq!(clan_lines(&decoded), ["0\u{e}Dave"]);
    }
    assert_eq!(
        world.pushed(bob, 0xA2),
        hex("A2 09 00000000 '' 03 '0\u{e}Dave'"),
        "a clan line carries no clan tag: its sender is no player"
    );
    world.take(dave);

    // Rank up: done at once (`83 49 12 00`) while it is not to the top.
    world.ask(alice, about(0x12, bob_id, &[]));
    let decoded = world.take(alice);
    assert!(
        decoded
            .iter()
            .any(|message| matches!(message, Decoded::Permitted(Permitted::RankUp(false))))
    );
    assert_eq!(
        members_shown(&decoded).unwrap(),
        // Highest rank first.
        [
            (alice_id, 0, true),
            (bob_id, 2, true),
            (eve_id, 3, true),
            (dave_id, 3, true)
        ]
    );
    // Bob (a sergeant: invite, battle commander) sees his new rights from
    // his own row.
    assert!(matches!(
        infos(&world.take(bob))[..],
        [ClanInfo::Members { unknown: 1, .. }]
    ));
    world.take(eve);
    world.take(dave);

    // A permission: `49 10 <rank> <value> <permission>`; the ranks go to
    // everyone.
    world.ask(alice, clan(0x10, &[3, 1, 1]));
    for index in all {
        let decoded = world.take(index);
        let [ClanInfo::Ranks { ranks, .. }] = &infos(&decoded)[..] else {
            panic!("{decoded:?}");
        };
        assert_eq!(ranks[3].permissions, permission::KICK);
    }
    world.ask(alice, clan(0x10, &[3, 0, 1]));
    assert_eq!(world.pushed(eve, 0x9A)[..7], hex("9A 03 01000000 04")[..]);
    world.take(eve);
    // A new rank above the lowest, and gone again: ranks, then members.
    world.ask(alice, named(0x14, "Scouts", &[]));
    let decoded = world.take(eve);
    let [
        ClanInfo::Ranks { ranks, .. },
        ClanInfo::Members { members, .. },
    ] = &infos(&decoded)[..]
    else {
        panic!("{decoded:?}");
    };
    assert_eq!(ranks.len(), 5);
    assert_eq!(
        (ranks[3].name.as_str(), ranks[3].is_default),
        ("Scouts", false)
    );
    assert_eq!(members[2].rank, 4);
    world.ask(alice, clan(0x15, &[3]));
    assert!(matches!(
        &infos(&world.take(eve))[..],
        [ClanInfo::Ranks { ranks, .. }, ClanInfo::Members { .. }] if ranks.len() == 4
    ));
    world.ask(alice, clan(0x15, &[3]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_remove_clan_rank")
    );

    // A soldier may do none of it: each refusal is its own text.
    #[rustfmt::skip]
    let refused = [
        (named(0x14, "Mine", &[]), "st_cant_create_clan_rank"),
        (clan(0x15, &[1]), "st_cant_remove_clan_rank"),
        (clan(0x10, &[3, 1, 0]), "st_cant_change_permission"),
        (about(0x13, dave_id, &[]), "st_cant_kick_member"),
        (about(0x12, dave_id, &[]), "st_cant_rank_up_clan_member"),
        (about(0x11, bob_id, &[0]), "st_cant_change_rank_clan_member"),
        (daily_message("mine"), "st_cant_set_daily_message"),
        (clan(7, &[]), "st_cant_create_clan"),
        (reserve("Others", "OTH"), "st_player_is_already_in_clan"),
    ];
    world.take(eve);
    for (request, text) in refused {
        world.ask(eve, request.clone());
        let decoded = world.take(eve);
        assert_eq!(
            denial(&decoded, 0x49).as_deref(),
            Some(text),
            "{request:02X?}"
        );
    }
    for index in all {
        world.take(index);
    }

    // The daily message goes to every member.
    world.ask(alice, daily_message("Сбор в девять"));
    for index in all {
        assert!(matches!(
            &infos(&world.take(index))[..],
            [ClanInfo::DailyMessage { text, .. }] if text == "Сбор в девять"
        ));
    }
    let mut expected = hex("9A 01");
    expected.extend(clan_id.to_le_bytes());
    expected.extend(hex("0D00"));
    expected.extend("Сбор в девять".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(world.pushed(dave, 0x9A), expected);

    // Down again; then up to the top, which the commander must confirm:
    // the clan changes hands and everyone gets it whole.
    world.ask(alice, about(0x11, bob_id, &[0]));
    assert_eq!(
        members_shown(&world.take(dave)).unwrap()[1],
        (bob_id, 3, true)
    );
    for _ in 0..2 {
        world.ask(alice, about(0x12, bob_id, &[]));
    }
    world.take(alice);
    world.ask(alice, about(0x12, bob_id, &[]));
    let decoded = world.take(alice);
    assert!(matches!(
        decoded[..],
        [Decoded::Permitted(Permitted::RankUp(true))]
    ));
    assert_eq!(world.pushed(alice, 0x83), hex("83 49 12 01"));
    world.ask(alice, about(0x11, bob_id, &[0xFF]));
    assert!(matches!(
        infos(&world.take(alice)).pop(),
        Some(ClanInfo::Registered {
            flag: false,
            unknown: 1,
            ..
        })
    ));
    assert!(matches!(
        infos(&world.take(bob)).pop(),
        Some(ClanInfo::Registered {
            flag: true,
            unknown: 0,
            ..
        })
    ));
    assert_eq!(world.accounts.clans().clan(clan_id).unwrap().leader, bob_id);
    world.take(eve);
    world.take(dave);

    // A kick: no clan for Dave; the others see who kicked whom.
    world.ask(bob, about(0x13, bob_id, &[]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_kick_himself")
    );
    world.ask(bob, about(0x13, dave_id, &[]));
    let decoded = world.take(dave);
    assert_eq!(infos(&decoded), [ClanInfo::None]);
    assert!(clan_lines(&decoded).is_empty());
    assert_eq!(world.account(dave).clan_tag, "");
    assert_eq!(world.pushed(dave, 0x9A), hex("9A 00 00000000"));
    for index in [alice, bob, eve] {
        let decoded = world.take(index);
        assert_eq!(members_shown(&decoded).unwrap().len(), 3);
        assert_eq!(clan_lines(&decoded), ["2\u{e}Bob\u{e}Dave"]);
    }

    // Eve leaves.
    world.ask(eve, clan(0x0D, &[]));
    assert_eq!(infos(&world.take(eve)), [ClanInfo::None]);
    assert_eq!(world.account(eve).clan_tag, "");
    for index in [alice, bob] {
        let decoded = world.take(index);
        assert_eq!(members_shown(&decoded).unwrap().len(), 2);
        assert_eq!(clan_lines(&decoded), ["1\u{e}Eve"]);
    }
    // Who has no clan can leave none.
    world.ask(eve, clan(0x0D, &[]));
    assert_eq!(
        denial(&world.take(eve), 0x49).as_deref(),
        Some("st_cant_cancel_clan_registration")
    );

    // The commander's `49 0D` disbands it: the others are told so.
    world.ask(bob, clan(0x0D, &[]));
    assert_eq!(infos(&world.take(bob)), [ClanInfo::None]);
    assert_eq!(
        infos(&world.take(alice)),
        [ClanInfo::Disbanded { show: true }, ClanInfo::None]
    );
    assert!(world.accounts.clans().is_empty());
    assert_eq!(world.account(alice).clan_tag, "");
    assert_eq!(world.account(bob).clan_tag, "");
    // The window of a player with no clan asks and is told so.
    world.ask(alice, clan(0x0E, &[0]));
    assert_eq!(infos(&world.take(alice)), [ClanInfo::None]);
    world.assert_clean();
}

#[test]
fn clan_chat_reaches_the_clan_and_team_chat_the_team() {
    let mut world = World::with_clans(test_config());
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let alice_id = world.id(alice);
    let clan_id = world.register("Stalkers", "STK", alice, &[(bob, "Bob")]);
    for index in [alice, bob, eve] {
        world.ask(index, request::subscribe());
        world.take(index);
    }
    let lines = |decoded: &[Decoded]| -> Vec<(u8, String, Option<String>)> {
        decoded
            .iter()
            .filter_map(|message| match message {
                Decoded::ChatText {
                    sender_type: sender::ACCOUNT,
                    channel,
                    body,
                    sender_clan_tag,
                    ..
                } => Some((*channel, body.clone(), sender_clan_tag.clone())),
                _ => None,
            })
            .collect()
    };

    // The clan channel: the clan's members, under the clan's tag.
    world.ask(
        alice,
        request::chat(clan_id, "", channel::CLAN, "to the clan"),
    );
    assert_eq!(
        lines(&world.take(bob)),
        [(
            channel::CLAN,
            "to the clan".to_owned(),
            Some("STK".to_owned())
        )]
    );
    assert!(world.take(eve).is_empty());
    let mut expected = hex("A2 05");
    expected.extend(alice_id.to_le_bytes());
    expected.extend(hex("'Alice' 'STK' 03 'to the clan'"));
    assert_eq!(world.pushed(bob, 0xA2), expected);
    // A player of no clan talks to nobody there, whatever id it sends.
    world.ask(eve, request::chat(clan_id, "", channel::CLAN, "let me in"));
    assert!(world.take(alice).is_empty() && world.take(bob).is_empty());
    // Everywhere else the tag shows too.
    world.ask(alice, request::chat(2, "", channel::GENERAL, "hello"));
    assert_eq!(
        lines(&world.take(eve)),
        [(channel::GENERAL, "hello".to_owned(), Some("STK".to_owned()))]
    );
    world.take(bob);

    // The clan-team channel: nobody without a team, then the team.
    world.ask(alice, request::chat(1, "", channel::CLAN_TEAM, "team?"));
    assert!(world.take(bob).is_empty());
    world.ask(alice, clan(0x17, &[]));
    world.take(bob);
    world.ask(alice, request::chat(1, "", channel::CLAN_TEAM, "team?"));
    assert!(lines(&world.take(bob)).is_empty());
    world.join_team(alice, bob, "Bob");
    world.take(alice);
    world.take(bob);
    world.ask(bob, request::chat(1, "", channel::CLAN_TEAM, "here"));
    assert_eq!(
        lines(&world.take(alice)),
        [(
            channel::CLAN_TEAM,
            "here".to_owned(),
            Some("STK".to_owned())
        )]
    );
    assert!(world.take(eve).is_empty());
    world.assert_clean();
}

#[test]
fn a_member_who_signs_in_is_sent_its_clan_and_its_mates_see_it_online() {
    let mut world = World::with_clans(test_config());
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let (alice_id, bob_id) = (world.id(alice), world.id(bob));
    let clan_id = world.register("Stalkers", "STK", alice, &[(bob, "Bob")]);
    world.ask(alice, daily_message("Rally at nine"));
    world.ask(alice, clan(0x17, &[]));
    world.join_team(alice, bob, "Bob");
    world.take(alice);

    // Bob's game goes: his mates see him offline, and he is out of the
    // clan team (a lobby thing) but not of the clan.
    world.client(bob).leave();
    world.settle();
    let decoded = world.take(alice);
    assert_eq!(
        members_shown(&decoded).unwrap(),
        [(alice_id, 0, true), (bob_id, 3, false)]
    );
    assert_eq!(
        team_shown(&decoded).unwrap().0,
        [(alice_id, role::COMMANDER, false)]
    );
    // An invite for Eve, who is away when it is sent.
    world.client(eve).leave();
    world.settle();
    world.ask(alice, named(0x0F, "Eve", &[]));
    world.ask(
        alice,
        invite_message("Eve", important::CLAN_INVITE, "Stalkers", clan_id),
    );
    world.take(alice);

    // Bob signs in again: the clan's name, the whole clan, the daily
    // message and the team, in that order; Alice sees him online.
    let bob = world.connect_session(signed_in(world.store, "Bob"));
    let decoded = world.take(bob);
    let infos = infos(&decoded);
    assert!(
        matches!(
            &infos[..],
            [
                ClanInfo::Startup { name, tag, .. },
                ClanInfo::Registered { unknown: 1, flag: false, .. },
                ClanInfo::DailyMessage { text, .. },
                ClanInfo::ClanTeam { seat: None, .. },
            ] if name == "Stalkers" && tag == "STK" && text == "Rally at nine"
        ),
        "{infos:?}"
    );
    let mut expected = hex("9A 05");
    expected.extend(clan_id.to_le_bytes());
    expected.extend(hex("01 'Stalkers' 'STK'"));
    let startup = {
        let link = &world.core.links[&world.clients[bob].0];
        link.pushed
            .iter()
            .find(|message| message.id_byte() == 0x9A)
            .map(|message| [&[0x9A][..], message.body()].concat())
            .unwrap()
    };
    assert_eq!(startup, expected);
    assert_eq!(
        members_shown(&world.take(alice)).unwrap(),
        [(alice_id, 0, true), (bob_id, 3, true)]
    );

    // Eve signs in: the invite waited for her, and she can take it.
    let eve = world.connect_session(signed_in(world.store, "Eve"));
    let decoded = world.take(eve);
    let message_id = find(&decoded, |message| match message {
        Decoded::Important {
            kind: important::CLAN_INVITE,
            sender_id,
            sender_name,
            sender_clan_tag,
            text,
            message_id,
            ..
        } if *sender_id == alice_id
            && sender_name == "Stalkers"
            && sender_clan_tag == "STK"
            && text == "Alice" =>
        {
            Some(*message_id)
        }
        _ => None,
    })
    .expect("the invite that waited");
    world.answer_invite(eve, alice, message_id, true);
    assert_eq!(world.account(eve).clan_tag, "STK");
    assert_eq!(members_shown(&world.take(alice)).unwrap().len(), 3);
    world.assert_clean();
}

#[test]
fn the_clan_team_window_does_what_it_offers() {
    let mut world = World::with_clans(Config {
        team_max: 2,
        ..test_config()
    });
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let dave = world.connect("Dave");
    let (alice_id, bob_id, eve_id) = (world.id(alice), world.id(bob), world.id(eve));
    let clan_id = world.register("Stalkers", "STK", alice, &[(bob, "Bob"), (eve, "Eve")]);
    world.take(dave);

    // The window opens (`49 0E 04`): no team yet.
    world.ask(bob, clan(0x0E, &[4]));
    assert_eq!(team_shown(&world.take(bob)), Some((Vec::new(), None)));
    assert_eq!(world.pushed(bob, 0x9A), hex("9A 04 01000000 00 00"));
    // A soldier has no battle command; nobody applies to a team that is
    // not; a player of no clan has nothing to do with any of it.
    #[rustfmt::skip]
    let refused = [
        (bob, clan(0x17, &[]), "st_cant_create_clan_team"),
        (bob, clan(0x18, &[]), "st_cant_request_to_join_clan_team"),
        (bob, clan(0x1B, &[]), "st_cant_leave_clan_team"),
        (bob, clan(0x1C, &[]), "st_cant_disband_clan_team"),
        (bob, clan(0x1E, &[]), "st_cant_oust_commander"),
        (bob, named(0x19, "Eve", &[]), "st_cant_invite_player_to_clan_team"),
        (bob, named(0x1D, "Eve", &[1]), "st_cant_change_status_clan_team_member"),
        (bob, named(0x1F, "Eve", &[]), "st_cant_remove_clan_team_member"),
        (bob, named(0x22, "Eve", &[]), "st_cant_set_clan_team_commander"),
        (bob, clan(0x20, &[0xFF]), "st_cant_change_ready_state"),
        (bob, about(0x1A, 1, &[0]), "st_cant_add_clan_team_member"),
        (dave, clan(0x17, &[]), "st_cant_create_clan_team"),
    ];
    for (index, request, text) in refused {
        world.ask(index, request.clone());
        let decoded = world.take(index);
        assert_eq!(
            denial(&decoded, 0x49).as_deref(),
            Some(text),
            "{request:02X?}"
        );
    }
    // Closing the window with no team is nothing to refuse.
    world.ask(bob, clan(0x23, &[]));
    assert!(world.take(bob).is_empty());

    // The commander makes the team: every member of the clan sees it, and
    // the commander its own place in it.
    world.ask(alice, clan(0x17, &[]));
    let (members, seat) = team_shown(&world.take(alice)).unwrap();
    assert_eq!(members, [(alice_id, role::COMMANDER, false)]);
    let team_id = seat.unwrap().clan_team_id;
    assert_eq!(
        seat,
        Some(ClanTeamSeat {
            in_clan_team: true,
            i_am_commander: true,
            commander_ready: false,
            clan_team_id: team_id,
        })
    );
    let mut expected = hex("9A 04");
    expected.extend(clan_id.to_le_bytes());
    expected.push(1);
    expected.extend(alice_id.to_le_bytes());
    expected.extend(hex("'Alice' 00 00  01 01 01 00"));
    expected.extend(team_id.to_le_bytes());
    assert_eq!(world.pushed(alice, 0x9A), expected);
    assert_eq!(team_shown(&world.take(bob)).unwrap().1, None);
    assert!(team_shown(&world.take(dave)).is_none());
    world.ask(alice, clan(0x17, &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_team_already_has_commander")
    );

    // Bob applies: listed under "applications", of the team not yet.
    world.ask(bob, clan(0x18, &[]));
    let (members, seat) = team_shown(&world.take(bob)).unwrap();
    assert_eq!(members[1], (bob_id, role::APPLICANT, false));
    assert_eq!(seat, None);
    world.ask(bob, clan(0x18, &[]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_already_in_clan_team")
    );
    // The commander takes him into the lineup.
    world.ask(alice, named(0x1D, "Bob", &[role::SOLDIER]));
    let (members, seat) = team_shown(&world.take(bob)).unwrap();
    assert_eq!(members[1], (bob_id, role::SOLDIER, false));
    assert_eq!(
        seat,
        Some(ClanTeamSeat {
            in_clan_team: true,
            i_am_commander: false,
            commander_ready: false,
            clan_team_id: team_id,
        })
    );
    // Eve is invited (`83 49 19`, the long `4D`, `49 1A`): the lineup of
    // two is full, so to the reserve.
    world.ask(alice, named(0x19, "Bob", &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_already_has_status_clan_team")
    );
    world.ask(alice, named(0x19, "Dave", &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_invite_player_to_clan_team")
    );
    world.join_team(alice, eve, "Eve");
    let mut expected = hex("83 49 19");
    expected.extend(team_id.to_le_bytes());
    expected.extend(hex("'Eve' 'Alice'"));
    assert_eq!(world.pushed(alice, 0x83), expected);
    let (members, _) = team_shown(&world.take(eve)).unwrap();
    assert_eq!(
        members,
        [
            (alice_id, role::COMMANDER, false),
            (bob_id, role::SOLDIER, false),
            (eve_id, role::RESERVE, false)
        ]
    );
    world.ask(alice, named(0x1D, "Eve", &[role::SOLDIER]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_clan_team_is_full")
    );
    // The reserve does not play.
    world.ask(eve, clan_play(1));
    assert_eq!(
        denial(&world.take(eve), 0x30).as_deref(),
        Some("st_must_be_in_clan_team")
    );

    // Bob to the reserve, Eve to the lineup; the list stays grouped.
    world.ask(alice, named(0x1D, "Bob", &[role::RESERVE]));
    world.ask(alice, named(0x1D, "Eve", &[role::SOLDIER]));
    let (members, _) = team_shown(&world.take(bob)).unwrap();
    assert_eq!(
        members,
        [
            (alice_id, role::COMMANDER, false),
            (eve_id, role::SOLDIER, false),
            (bob_id, role::RESERVE, false)
        ]
    );
    // The clan's commander names Eve the team's; then she is ousted and
    // the command is free for who may command.
    world.ask(alice, named(0x22, "Eve", &[]));
    let (members, seat) = team_shown(&world.take(eve)).unwrap();
    assert_eq!(members[0], (eve_id, role::COMMANDER, false));
    assert_eq!(members[1], (alice_id, role::SOLDIER, false));
    assert!(seat.unwrap().i_am_commander);
    world.ask(bob, clan(0x1E, &[]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_oust_commander")
    );
    world.ask(alice, clan(0x1E, &[]));
    let (members, seat) = team_shown(&world.take(eve)).unwrap();
    assert!(members.iter().all(|member| member.1 != role::COMMANDER));
    assert!(!seat.unwrap().i_am_commander);
    world.ask(alice, clan(0x17, &[]));
    let (members, _) = team_shown(&world.take(alice)).unwrap();
    assert_eq!(members[0], (alice_id, role::COMMANDER, false));
    assert_eq!(members.len(), 3);

    // Removed, left, disbanded.
    world.ask(alice, named(0x1F, "Bob", &[]));
    let (members, seat) = team_shown(&world.take(bob)).unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(seat, None);
    world.ask(eve, clan(0x1B, &[]));
    assert_eq!(team_shown(&world.take(eve)).unwrap().0.len(), 1);
    world.ask(eve, clan(0x1C, &[]));
    assert_eq!(
        denial(&world.take(eve), 0x49).as_deref(),
        Some("st_cant_disband_clan_team")
    );
    world.ask(alice, clan(0x1C, &[]));
    for index in [alice, bob, eve] {
        assert_eq!(team_shown(&world.take(index)), Some((Vec::new(), None)));
    }
    assert_eq!(world.accounts.clans().team(clan_id), None);

    // A player in a squad stays out of clan teams.
    super::form_squad(&mut world, alice, bob);
    world.take(alice);
    world.ask(alice, clan(0x17, &[]));
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_create_clan_team")
    );
    world.assert_clean();
}

#[test]
fn two_clan_teams_meet_in_a_match_and_one_alone_waits() {
    let mut world = World::with_clans(test_config());
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let eve = world.connect("Eve");
    let (alice_id, bob_id, eve_id) = (world.id(alice), world.id(bob), world.id(eve));
    let stalkers = world.register("Stalkers", "STK", alice, &[(bob, "Bob")]);
    let bandits = world.register("Bandits", "BND", eve, &[]);
    world.ask(alice, clan(0x17, &[]));
    world.join_team(alice, bob, "Bob");
    for index in [alice, bob, eve] {
        world.take(index);
    }

    // Clan Play without a clan team is refused with the game's text.
    world.ask(eve, clan_play(1));
    assert_eq!(
        denial(&world.take(eve), 0x30).as_deref(),
        Some("st_must_be_in_clan_team")
    );
    // The commander is not ready before its soldier.
    world.clan_ready(alice, true, true);
    assert_eq!(
        denial(&world.take(alice), 0x49).as_deref(),
        Some("st_cant_change_ready_state")
    );
    assert_eq!(world.pushed(alice, 0x83), hex("83 30"));
    world.take(bob);

    // The soldier's Play makes him ready: his own flag, and everyone's
    // list. No queue yet.
    world.clan_ready(bob, false, true);
    let decoded = world.take(bob);
    let (members, seat) = team_shown(&decoded).unwrap();
    assert_eq!(
        members,
        [
            (alice_id, role::COMMANDER, false),
            (bob_id, role::SOLDIER, true)
        ]
    );
    let seat = seat.unwrap();
    assert!(seat.commander_ready && !seat.i_am_commander);
    assert_eq!(queue_state(&decoded), None);
    let (_, seat) = team_shown(&world.take(alice)).unwrap();
    assert!(!seat.unwrap().commander_ready);
    // And not ready again, by the same Play.
    world.clan_ready(bob, false, false);
    assert!(!team_shown(&world.take(bob)).unwrap().0[1].2);
    world.clan_ready(bob, false, true);

    // The commander's Play: the team waits, its players are in the queue.
    world.clan_ready(alice, true, true);
    for index in [alice, bob] {
        let decoded = world.take(index);
        assert!(
            matches!(queue_state(&decoded), Some(ClientState::InQueue { .. })),
            "{decoded:?}"
        );
        let (members, _) = team_shown(&decoded).unwrap();
        assert!(members.iter().all(|member| member.2));
    }
    assert_eq!(world.core.clan_teams_waiting(), [stalkers]);
    // A soldier's ready toggle is off while the commander is ready.
    world.ask(bob, clan(0x20, &[0]));
    assert_eq!(
        denial(&world.take(bob), 0x49).as_deref(),
        Some("st_cant_change_ready_state")
    );
    // One team alone waits, however long: a match needs another clan's.
    world.advance(Duration::from_secs(120));
    assert!(world.host.made.lock().unwrap().is_empty());
    assert!(matches!(
        queue_state(&world.take(alice)),
        Some(ClientState::InQueue { .. })
    ));
    world.take(bob);

    // A player of it leaves the queue (`31`): the team is out, and the
    // others are back in the lobby.
    world.ask(bob, request::leave(1, false));
    assert_eq!(queue_state(&world.take(bob)), Some(ClientState::Lobby));
    let decoded = world.take(alice);
    assert_eq!(queue_state(&decoded), Some(ClientState::Lobby));
    let (members, _) = team_shown(&decoded).unwrap();
    assert!(members.iter().all(|member| !member.2));
    assert!(world.core.clan_teams_waiting().is_empty());

    // Ready again; then the other clan's team of one is, and the two meet.
    world.clan_ready(bob, false, true);
    world.clan_ready(alice, true, true);
    world.ask(eve, clan(0x17, &[]));
    for index in [alice, bob, eve] {
        world.take(index);
    }
    world.clan_ready(eve, true, true);
    world.advance(Duration::from_secs(1));
    let sent: Vec<(u32, u8)> = [alice, bob, eve]
        .into_iter()
        .map(|index| {
            let decoded = world.take(index);
            // Nobody is ready any more: the team is shown as it left.
            let (members, _) = team_shown(&decoded).unwrap();
            assert!(members.iter().all(|member| !member.2));
            sent_to_match(&decoded).unwrap_or_else(|| panic!("no match: {decoded:?}"))
        })
        .collect();
    let match_id = sent[0].0;
    // One match, a clan a side.
    assert_eq!(sent, [(match_id, 0), (match_id, 0), (match_id, 1)]);
    assert!(world.core.clan_teams_waiting().is_empty());
    {
        let made = world.host.made.lock().unwrap();
        assert_eq!(made.len(), 1);
        let players: Vec<(u32, u8, &str)> = made[0]
            .1
            .players
            .iter()
            .map(|player| {
                (
                    player.account_id,
                    player.profile.team,
                    player.profile.clan_tag.as_str(),
                )
            })
            .collect();
        // The match shows each player's clan.
        assert_eq!(
            players,
            [(alice_id, 0, "STK"), (bob_id, 0, "STK"), (eve_id, 1, "BND")]
        );
    }

    // The match ends: its players are back, and what they earned is
    // their clans' too.
    world.end_match(match_id, 0);
    world.settle();
    let clans = world.accounts.clans();
    let report = world.results.find(match_id).expect("the match's results");
    let earned = |account_id: u32| report.player(account_id).unwrap().reward.experience;
    assert!(earned(alice_id) > 0 && earned(eve_id) > 0);
    assert_eq!(
        clans.clan(stalkers).unwrap().experience,
        earned(alice_id) + earned(bob_id)
    );
    assert_eq!(clans.clan(bandits).unwrap().experience, earned(eve_id));
    // And the teams can play again.
    for index in [alice, bob, eve] {
        world.take(index);
    }
    world.clan_ready(eve, true, true);
    assert_eq!(world.core.clan_teams_waiting(), [bandits]);
    // A commander whose game goes takes its team out of the queue.
    world.client(eve).leave();
    world.settle();
    assert!(world.core.clan_teams_waiting().is_empty());
    assert_eq!(clans.team(bandits), None);
    world.assert_clean();
}

/// A clan match seats nobody who watches something meanwhile: its seat would
/// never be sent (`docs/notes/queue-spectate.md`). A player with a replay
/// session on keeps its commander from being ready; one who watches a match
/// being played while its team waits takes the team out of the queue, and the
/// other clan's team waits alone.
#[test]
fn a_clan_team_with_a_player_watching_is_not_ready() {
    use session::v026g::replay_watch::WatchSettings;
    use session::v026g::replays::Replays;
    use session::{LiveDelay, WatchKind, Watcher};

    use super::live_tests::{a_match_in_play, ask_to_watch};

    const REPLAY: u32 = 0x1F07_C8FB;
    let mut world = World::with_clans(test_config());
    let (ticket, [alice, _]) = a_match_in_play(&mut world);
    let (match_id, _) = world
        .store
        .matchmaking()
        .seat(world.ids[alice].1)
        .expect("a match being played");
    let carol = world.connect("Carol");
    let eve = world.connect("Eve");
    let stalkers = world.register("Stalkers", "STK", carol, &[]);
    let bandits = world.register("Bandits", "BND", eve, &[]);
    for index in [carol, eve] {
        world.ask(index, clan(0x17, &[]));
        world.take(index);
    }
    let desk: &'static Replays = Box::leak(Box::new(
        Replays::new(None).with_watch(WatchSettings::default()),
    ));
    world.core.replays = Some(desk);
    let (account, session) = world.ids[carol];

    // A replay session of hers on (its recording not read yet).
    assert!(ask_to_watch(&mut world, carol, REPLAY).starts_with("R1;preparing;"));
    world.clan_ready(carol, true, true);
    assert_eq!(
        denial(&world.take(carol), 0x49).as_deref(),
        Some("st_cant_change_ready_state")
    );
    assert!(world.core.clan_teams_waiting().is_empty());
    // Over: her team waits.
    assert!(desk.watches().withdraw(account, REPLAY));
    world.clan_ready(carol, true, true);
    assert_eq!(world.core.clan_teams_waiting(), [stalkers]);

    // She watches the match being played (the lobby's own asks refuse a
    // waiting player; matchmaking's word is what counts).
    let live = Watcher {
        match_id,
        ticket,
        account_id: account,
        kind: WatchKind::Live {
            delay: LiveDelay::DEFAULT,
            view: None,
        },
    };
    assert!(world.store.matchmaking().watch(session, live, world.now));
    world.clan_ready(eve, true, true);
    assert_eq!(world.core.clan_teams_waiting(), [bandits]);
    world.advance(Duration::from_secs(1));
    assert_eq!(world.store.matchmaking().seat(session), None);
    assert_eq!(
        world.host.made.lock().unwrap().len(),
        1,
        "only the match being played"
    );
    assert_eq!(world.core.clan_teams_waiting(), [bandits]);
}

/// A clan match (a Play with no room and no bots: the lobby draws its map)
/// is of two teams, so its map is never a free-for-all's, nor Blackout's,
/// however many of them the match pool has: from a pool of them and one Team
/// Deathmatch map it is that map; from a pool of them alone, a map of two
/// teams all the same.
#[test]
fn a_clan_match_never_draws_a_free_for_all() {
    use crate::v026g::Settings;
    use survarium::v026g::maps::{self, GameMode};

    let of = |mode| -> Vec<u8> { maps::playable_in(mode).map(|map| map.id).collect() };
    let free_for_alls = of(GameMode::FreeForAll);
    // The test data's: one for each Team Deathmatch map.
    assert_eq!(free_for_alls.len(), 24);
    let default = Settings::local("127.0.0.1:0").maps;
    assert!(
        free_for_alls.iter().all(|id| default.contains(id)),
        "the default pool has them"
    );
    let others = [free_for_alls, of(GameMode::Blackout)].concat();
    for (pool, only) in [
        ([others.clone(), vec![117]].concat(), Some(117)),
        (others.clone(), None),
    ] {
        let mut settings = Settings::local("127.0.0.1:0");
        settings.maps = pool;
        let mut world = World::with_settings(settings);
        world.accounts.clans().set_config(test_config());
        let alice = world.connect("Alice");
        let eve = world.connect("Eve");
        world.register("Stalkers", "STK", alice, &[]);
        world.register("Bandits", "BND", eve, &[]);
        for index in [alice, eve] {
            world.ask(index, clan(0x17, &[]));
            world.take(index);
        }
        world.clan_ready(alice, true, true);
        world.clan_ready(eve, true, true);
        world.advance(Duration::from_secs(1));
        let (match_id, _) = sent_to_match(&world.take(alice)).expect("a match");
        let made = world.host.made.lock().unwrap();
        assert_eq!(made.len(), 1);
        let (id, setup) = &made[0];
        assert_eq!(*id, match_id);
        let map = setup.played.map();
        assert!(
            !matches!(map.mode, GameMode::FreeForAll | GameMode::Blackout),
            "map {} is {:?}",
            map.id,
            map.mode
        );
        if let Some(only) = only {
            assert_eq!(map.id, only);
        }
        let teams: Vec<u8> = setup.players.iter().map(|player| player.profile.team).collect();
        assert_eq!(teams, [0, 1], "a clan a side");
    }
}
