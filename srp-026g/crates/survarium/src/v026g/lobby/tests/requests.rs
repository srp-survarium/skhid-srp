//! Every client→server message, from bytes laid out by hand
//! (`lobby_requests.md` §1..§3, `social.md` §1.5 and §3.3).

use super::super::requests::*;
use super::h;

/// Parses `id body`.
#[track_caller]
fn read(text: &str) -> ClientMessage {
    let bytes = h(text);
    parse(bytes[0], &bytes[1..]).unwrap_or_else(|error| panic!("{text}: {error}"))
}

use ClientMessage as M;

#[test]
fn sign_in_and_play() {
    assert_eq!(
        read("2E 78563412 FF"),
        M::SignIn {
            session_id: 0x12345678,
            need_shop_items: true
        }
    );
    assert_eq!(
        read("2E 01000000 00"),
        M::SignIn {
            session_id: 1,
            need_shop_items: false
        }
    );
    assert_eq!(read("2F 02000000"), M::Play { kit_id: 2 });
    assert_eq!(read("30 02000000"), M::ClanPlay { kit_id: 2 });
    assert_eq!(
        read("31 07000000 00"),
        M::Leave {
            order_id: 7,
            left_match: false
        }
    );
    assert_eq!(
        read("31 FFFFFFFF FF"),
        M::Leave {
            order_id: u32::MAX,
            left_match: true
        }
    );
    assert_eq!(read("32 00 00"), M::AfterSignIn { flag: false });
    assert_eq!(read("32 00 FF"), M::AfterSignIn { flag: true });
}

#[test]
fn queries_skip_their_leading_zero() {
    assert_eq!(
        read("34 00 05000000 09000000"),
        M::QuerySquadMember {
            account_id: 5,
            kit_id: 9
        }
    );
    assert_eq!(read("37 00"), M::QuerySkills);
    assert_eq!(read("39 00"), M::QueryReputations);
    assert_eq!(
        read("3B 00 14000000 0A"),
        M::QueryRanking {
            start: 20,
            count: 10
        }
    );
    assert_eq!(read("3C 00"), M::QueryQuests);
    assert_eq!(
        read("3D 00 05000000 23010000"),
        M::QueryPlayerDetails {
            player_id: 5,
            match_id: 0x123
        }
    );
    assert_eq!(read("3E 00"), M::QueryServices);
    assert_eq!(read("40 00"), M::QueryUnlockedFactions);
    assert_eq!(
        read("41 00 23010000"),
        M::QueryMatchStats { match_id: 0x123 }
    );
    assert_eq!(read("42 00 07000000"), M::QueryDrop { drop_id: 7 });
    assert_eq!(read("43 00"), M::QueryDailyBonus);
    // No leading zero: the byte is the language.
    assert_eq!(read("45 02"), M::QueryVideoLink { language: 2 });
}

#[test]
fn inventory_actions() {
    // `46 00`: kit, `u8 n`, n x 16 bytes.
    assert_eq!(
        read(
            "46 00 01000000 02
             09000000 1200 64 07 01000000 00000000
             0A000000 3200 08 64 32000000 96000000"
        ),
        M::MoveItems {
            kit_id: 1,
            moves: vec![
                Relocation {
                    item_id: 9,
                    dict_id: 18,
                    source_slot: INVENTORY_SLOT,
                    target_slot: 7,
                    count: 1,
                    count_in_inventory: 0,
                },
                Relocation {
                    item_id: 10,
                    dict_id: 50,
                    source_slot: 8,
                    target_slot: INVENTORY_SLOT,
                    count: 50,
                    count_in_inventory: 150,
                },
            ],
        }
    );
    assert_eq!(
        read("46 00 01000000 00"),
        M::MoveItems {
            kit_id: 1,
            moves: Vec::new()
        }
    );
    // `46 01`: the 8 zero bytes are part of it.
    assert_eq!(
        read("46 01 01000000 02000000 03000000 04 0000000000000000"),
        M::AttachUpgrade {
            kit_id: 1,
            a0: 2,
            a1: 3,
            a2: 4
        }
    );
    assert_eq!(
        read("46 02 01000000 05000000 06000000"),
        M::DetachUpgrade {
            kit_id: 1,
            upgrade_item_id: 5,
            host_item_id: 6
        }
    );
    // A stock client's `46 02` names no host (patch 30 adds it): host 0,
    // the first item holding the upgrade. Cut inside the upgrade: short.
    assert_eq!(
        read("46 02 01000000 05000000"),
        M::DetachUpgrade {
            kit_id: 1,
            upgrade_item_id: 5,
            host_item_id: 0
        }
    );
    let short = h("46 02 01000000 0500");
    assert_eq!(
        parse(short[0], &short[1..]),
        Err(ParseError::Truncated { id: 0x46 })
    );
    assert_eq!(
        read("46 03 05000000 23010000"),
        M::UseDropCard { a0: 5, a1: 0x123 }
    );
    assert_eq!(read("46 04 05000000"), M::UseItem { item_id: 5 });
    assert_eq!(
        read("46 05 01000000 05000000 02000000 0807060504030201"),
        M::ApplyDecal {
            kit_id: 1,
            item_id: 5,
            a: 2,
            value: 0x0102030405060708
        }
    );
    assert_eq!(read("46 06 05000000"), M::OpenReroll { item_id: 5 });
    assert_eq!(
        read("46 07 05000000 1A040000 FF"),
        M::RerollModifier {
            item_id: 5,
            modification_id: 1050,
            flag: true
        }
    );
    assert_eq!(
        read("46 08 05000000 1A040000 1B040000"),
        M::ChooseModifier {
            item_id: 5,
            old_modification_id: 1050,
            new_modification_id: 1051
        }
    );
    assert_eq!(
        read("46 09 05000000 06000000 00"),
        M::Craft {
            a0: 5,
            a1: 6,
            flag: false
        }
    );
    assert_eq!(read("46 0A"), M::TutorialWeaponChosen);
}

#[test]
fn shop_actions() {
    // `u16 dict, u32 count, u8 faction, u8 00, u8 target slot, u32 kit`.
    assert_eq!(
        read("47 00 3200 03000000 00 00 64 01000000"),
        M::Buy {
            dict_id: 50,
            count: 3,
            faction: 0,
            target_slot: INVENTORY_SLOT,
            kit_id: 1
        }
    );
    assert_eq!(
        read("47 01 0A000000 02000000"),
        M::Sell {
            item_id: 10,
            amount: 2
        }
    );
    assert_eq!(
        read("47 02 07000000 00000000 00000000"),
        M::BuyOffer {
            offer_id: 7,
            x: 0,
            factor: 0.0
        }
    );
    // The kit's id only for kind 2.
    assert_eq!(
        read("47 03 09000000 00"),
        M::Repair {
            item_id: 9,
            payment: RepairPayment::Silver
        }
    );
    assert_eq!(
        read("47 03 09000000 01"),
        M::Repair {
            item_id: 9,
            payment: RepairPayment::Gold
        }
    );
    assert_eq!(
        read("47 03 09000000 02 0C000000"),
        M::Repair {
            item_id: 9,
            payment: RepairPayment::Kit { kit_item_id: 12 }
        }
    );
    assert_eq!(read("47 04 64000000"), M::ExchangePremium { amount: 100 });
    assert_eq!(read("47 04 FFFFFF3F"), M::ExchangePremium { amount: 0x3FFF_FFFF });
    assert_eq!(read("47 04 05000040"), M::BuyPremium { gold: 5 });
    assert_eq!(read("47 04 00000040"), M::BuyPremium { gold: 0 });
    assert_eq!(
        read("47 05 01000000 0000003F 02000000 03000000 04000000 02"),
        M::AcceptPremiumWithBonuses {
            a0: 1,
            a1: 0.5,
            a2: 2,
            a3: 3,
            a4: 4,
            faction: 2
        }
    );
    assert_eq!(
        read("47 06 01000000 0000803E"),
        M::AcceptPremiumWithDiscount { a0: 1, a1: 0.25 }
    );
}

#[test]
fn skills() {
    // `u8 n`, n x {u16 skill, u8 points}: no kit id, no perks.
    assert_eq!(
        read("48 00 02 6500 02 D100 01"),
        M::SetSkills {
            skills: vec![(101, 2), (209, 1)]
        }
    );
    assert_eq!(read("48 00 00"), M::SetSkills { skills: Vec::new() });
    assert_eq!(
        read("48 01 01 00"),
        M::ResetSkills {
            branch: 1,
            payment: 0
        }
    );
}

#[test]
fn kit_and_account_actions() {
    assert_eq!(
        read("49 00 02000000 'Sniper'"),
        M::RenameKit {
            kit_id: 2,
            name: "Sniper".into()
        }
    );
    assert_eq!(
        read("49 01 01"),
        M::SetAutobuy {
            on: true,
            repair: false
        }
    );
    assert_eq!(
        read("49 01 00"),
        M::SetAutobuy {
            on: false,
            repair: false
        }
    );
    // The lobby's number: 2 is auto-repair alone, 3 both.
    assert_eq!(
        read("49 01 02"),
        M::SetAutobuy {
            on: false,
            repair: true
        }
    );
    assert_eq!(
        read("49 01 03"),
        M::SetAutobuy {
            on: true,
            repair: true
        }
    );
    assert_eq!(read("49 02 02000000"), M::SetActiveKit { kit_id: 2 });
    assert_eq!(
        read("49 03 'Alice'"),
        M::CheckNickname {
            nickname: "Alice".into()
        }
    );
    assert_eq!(
        read("49 04 'Alice' 4D000000"),
        M::ChangeNickname {
            nickname: "Alice".into(),
            ticket_item_id: 77
        }
    );
    assert_eq!(
        read("49 05 'Bob' 01 02"),
        M::CommentAboutUser {
            nickname: "Bob".into(),
            a: 1,
            b: 2
        }
    );
    assert_eq!(read("49 24 00"), M::HideVideoLink);
}

#[test]
fn clan_requests() {
    use ClanRequest as C;
    let clan = |text: &str| match read(text) {
        M::Clan(request) => request,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        clan("49 06 'Wolves' 'WLF'"),
        C::ReserveName {
            name: "Wolves".into(),
            tag: "WLF".into()
        }
    );
    assert_eq!(clan("49 07"), C::Create);
    assert_eq!(
        clan("49 08 'Wolves'"),
        C::CheckName {
            name: "Wolves".into()
        }
    );
    assert_eq!(clan("49 09 'WLF'"), C::CheckTag { tag: "WLF".into() });
    assert_eq!(
        clan("49 0A 07000000 01"),
        C::AnswerInvite {
            inviter_id: 7,
            answer: 1
        }
    );
    assert_eq!(
        clan("49 0B 'Bob'"),
        C::InviteSent {
            target_name: "Bob".into()
        }
    );
    assert_eq!(clan("49 0C 05000000"), C::BuyRegistration { value: 5 });
    assert_eq!(clan("49 0D"), C::LeaveOrDisband);
    assert_eq!(clan("49 0E 04"), C::QueryInfo { what: 4 });
    assert_eq!(
        clan("49 0F 'Bob'"),
        C::Invite {
            target_name: "Bob".into()
        }
    );
    assert_eq!(
        clan("49 10 01 01 02"),
        C::SetPermission {
            rank: 1,
            permission: 2,
            value: 1
        }
    );
    assert_eq!(
        clan("49 11 07000000 01"),
        C::ChangeRank {
            account_id: 7,
            up: true
        }
    );
    assert_eq!(clan("49 12 07000000"), C::AskRankUp { account_id: 7 });
    assert_eq!(clan("49 13 07000000"), C::Kick { account_id: 7 });
    assert_eq!(
        clan("49 14 'Chief'"),
        C::CreateRank {
            name: "Chief".into()
        }
    );
    assert_eq!(clan("49 15 03"), C::RemoveRank { rank: 3 });
    // `u16 count` and UTF-16LE units: two Cyrillic letters.
    assert_eq!(
        clan("49 16 0200 3E04 3A04"),
        C::SetDailyMessage {
            text: "\u{43E}\u{43A}".into()
        }
    );
    assert_eq!(clan("49 17"), C::CreateTeam);
    assert_eq!(clan("49 18"), C::AskToJoinTeam);
    assert_eq!(
        clan("49 19 'Bob'"),
        C::InviteToTeam {
            target_name: "Bob".into()
        }
    );
    assert_eq!(
        clan("49 1A 07000000 00"),
        C::AnswerTeamInvite {
            inviter_id: 7,
            answer: 0
        }
    );
    assert_eq!(clan("49 1B"), C::LeaveTeam);
    assert_eq!(clan("49 1C"), C::DisbandTeam);
    assert_eq!(
        clan("49 1D 'Bob' 02"),
        C::SetTeamMemberStatus {
            member_name: "Bob".into(),
            status: 2
        }
    );
    assert_eq!(clan("49 1E"), C::OustCommander);
    assert_eq!(
        clan("49 1F 'Bob'"),
        C::RemoveTeamMember {
            member_name: "Bob".into()
        }
    );
    assert_eq!(clan("49 20 FF"), C::SoldierReady { ready: true });
    assert_eq!(clan("49 21 01"), C::CommanderReady { ready: true });
    assert_eq!(
        clan("49 22 'Bob'"),
        C::SetTeamCommander {
            member_name: "Bob".into()
        }
    );
    assert_eq!(clan("49 23"), C::MatchClosed);
}

#[test]
fn squad_and_quest_actions() {
    assert_eq!(read("4A 01"), M::DisbandSquad);
    assert_eq!(
        read("4A 02 'Bob'"),
        M::AddSquadMember { name: "Bob".into() }
    );
    assert_eq!(read("4A 03 07000000"), M::DropSquadMember { account_id: 7 });
    assert_eq!(read("4A 04"), M::LeaveSquad);
    assert_eq!(read("4B 00 03"), M::ChooseQuestFaction { faction: 3 });
    assert_eq!(read("4B 01 02"), M::TutorialStep { step: 2 });
    assert_eq!(read("4B 02"), M::SkipTutorial);
}

#[test]
fn chat_messages() {
    // `u32 channel id, string receiver, u8 channel, string body`.
    assert_eq!(
        read("4C 01000000 00 01 'hello'"),
        M::ChatText {
            channel_id: 1,
            receiver: String::new(),
            channel: 1,
            body: "hello".into()
        }
    );
    assert_eq!(
        read("4C 00000000 'Bob' 04 'psst'"),
        M::ChatText {
            channel_id: 0,
            receiver: "Bob".into(),
            channel: 4,
            body: "psst".into()
        }
    );
    // `u32 0, string target, u8 type, string text`.
    assert_eq!(
        read("4D 00000000 'Bob' 02 'invite'"),
        M::Important {
            target_name: "Bob".into(),
            kind: 2,
            text: "invite".into(),
            extra: None
        }
    );
    assert_eq!(
        read("4D 00000000 'Bob' 00 00"),
        M::Important {
            target_name: "Bob".into(),
            kind: 0,
            text: String::new(),
            extra: None
        }
    );
    // The long form of a clan invite ends with a u32.
    assert_eq!(
        read("4D 00000000 'Bob' 07 'Wolves' 05000000"),
        M::Important {
            target_name: "Bob".into(),
            kind: 7,
            text: "Wolves".into(),
            extra: Some(5)
        }
    );
    use FriendshipAction as F;
    assert_eq!(
        read("4E 02 07000000"),
        M::Friendship(F::RemoveFriend { account_id: 7 })
    );
    assert_eq!(
        read("4E 03 07000000"),
        M::Friendship(F::Ignore { account_id: 7 })
    );
    assert_eq!(
        read("4E 04 'Troll'"),
        M::Friendship(F::IgnoreByName {
            name: "Troll".into()
        })
    );
    assert_eq!(
        read("4E 05 07000000"),
        M::Friendship(F::StopIgnoring { account_id: 7 })
    );
    assert_eq!(
        read("4E 06 'Bo'"),
        M::Friendship(F::FindPlayers { name: "Bo".into() })
    );
    // 10 x u32 by channel number, then the language.
    assert_eq!(
        read(
            "4F 00000000 01000000 FFFFFFFF 00000000 01000000
                00000000 00000000 00000000 55000000 00000000 02"
        ),
        M::Subscribe {
            channel_ids: [0, 1, u32::MAX, 0, 1, 0, 0, 0, 0x55, 0],
            language: 2
        }
    );
    assert_eq!(
        read("50 01100000 01"),
        M::ImportantRead {
            message_id: 0x1001,
            answer: 1
        }
    );
}

#[test]
fn what_is_not_a_client_message_is_refused() {
    // Ids the client never makes, and ids of the other direction.
    for id in id::NEVER_SENT
        .into_iter()
        .chain([0x00, 0x2D, 0x51, 0x80, 0xFF])
    {
        assert_eq!(parse(id, &[0]), Err(ParseError::UnknownId(id)), "{id:#04X}");
    }
    // Every other id in the client's range is read.
    for id in 0x2E..=0x50_u8 {
        if !id::NEVER_SENT.contains(&id) {
            assert!(
                !matches!(parse(id, &[]), Err(ParseError::UnknownId(_))),
                "{id:#04X}"
            );
        }
    }
    assert_eq!(
        parse(0x46, &h("0B")),
        Err(ParseError::UnknownSubType {
            id: 0x46,
            sub: 0x0B
        })
    );
    assert_eq!(
        parse(0x47, &h("07")),
        Err(ParseError::UnknownSubType { id: 0x47, sub: 7 })
    );
    assert_eq!(
        parse(0x49, &h("25")),
        Err(ParseError::UnknownSubType {
            id: 0x49,
            sub: 0x25
        })
    );
    assert_eq!(
        parse(0x4A, &h("00")),
        Err(ParseError::UnknownSubType { id: 0x4A, sub: 0 })
    );
    // Friendship actions 0, 1, 7, 8 and 9 are never sent.
    for sub in [0, 1, 7, 8, 9] {
        assert_eq!(
            parse(0x4E, &[sub, 0, 0, 0, 0]),
            Err(ParseError::UnknownSubType { id: 0x4E, sub })
        );
    }
}

#[test]
fn a_cut_or_overlong_message_is_reported() {
    assert_eq!(
        parse(0x2E, &h("01000000")),
        Err(ParseError::Truncated { id: 0x2E })
    );
    assert_eq!(parse(0x2E, &[]), Err(ParseError::Truncated { id: 0x2E }));
    // A string whose length byte runs past the body.
    assert_eq!(
        parse(0x4A, &h("02 05 \"Bo\"")),
        Err(ParseError::Truncated { id: 0x4A })
    );
    // A move list shorter than its count.
    assert_eq!(
        parse(
            0x46,
            &h("00 01000000 02 09000000 1200 64 07 01000000 00000000")
        ),
        Err(ParseError::Truncated { id: 0x46 })
    );
    // Bytes after a whole message: reported, with the message.
    match parse(0x2F, &h("02000000 AA BB")) {
        Err(ParseError::Trailing { message, extra }) => {
            assert_eq!(*message, M::Play { kit_id: 2 });
            assert_eq!(extra, 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn text_that_is_no_utf8_is_still_read() {
    assert_eq!(
        read("4A 02 03 41 FF 42"),
        M::AddSquadMember {
            name: "A\u{FFFD}B".into()
        }
    );
}
