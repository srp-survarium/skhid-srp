//! A ban as srp tells it to the banned player's game: its reason, one of
//! srp's own (each a dictionary text in every language,
//! `st_srp_ban_reason_<name>`), and when it ends. srp's login server sends it
//! after a ban's code (the game's own reason, which a game without patch 116
//! shows); patch 116 reads it and shows it on the sign-in screen instead
//! (client-patches/README.md, "A ban's reason and end on the sign-in screen").
//!
//! The answer's tail, after the code: the reason's id, then for a ban that
//! ends its seconds left (u32 LE). A ban for good is the id alone.

#![no_std]

use core::ffi::CStr;

/// The reasons a ban is told by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanReason {
    /// None of the others; the default.
    Administration,
    Cheating,
    /// Farming is fine while few players are online; pressing the players
    /// who don't farm (in chat, or by killing them) is not.
    FarmPressure,
    Teamkilling,
    Insults,
    BadLanguage,
    Flood,
    Advertising,
    AccountTrading,
    Threats,
    Nickname,
    Racism,
    Politics,
    FalseInformation,
}

impl BanReason {
    /// Every reason, by its id (its place here): the wire's, never reordered.
    pub const ALL: [BanReason; 14] = [
        BanReason::Administration,
        BanReason::Cheating,
        BanReason::FarmPressure,
        BanReason::Teamkilling,
        BanReason::Insults,
        BanReason::BadLanguage,
        BanReason::Flood,
        BanReason::Advertising,
        BanReason::AccountTrading,
        BanReason::Threats,
        BanReason::Nickname,
        BanReason::Racism,
        BanReason::Politics,
        BanReason::FalseInformation,
    ];

    pub fn id(self) -> u8 {
        Self::ALL.iter().position(|reason| *reason == self).unwrap_or(0) as u8
    }

    pub fn from_id(id: u8) -> Option<BanReason> {
        Self::ALL.get(usize::from(id)).copied()
    }

    /// Its text's dictionary key (client-patches/localization/*.tsv).
    pub fn key(self) -> &'static CStr {
        match self {
            BanReason::Administration => c"st_srp_ban_reason_administration",
            BanReason::Cheating => c"st_srp_ban_reason_cheating",
            BanReason::FarmPressure => c"st_srp_ban_reason_farm_pressure",
            BanReason::Teamkilling => c"st_srp_ban_reason_teamkilling",
            BanReason::Insults => c"st_srp_ban_reason_insults",
            BanReason::BadLanguage => c"st_srp_ban_reason_bad_language",
            BanReason::Flood => c"st_srp_ban_reason_flood",
            BanReason::Advertising => c"st_srp_ban_reason_advertising",
            BanReason::AccountTrading => c"st_srp_ban_reason_account_trading",
            BanReason::Threats => c"st_srp_ban_reason_threats",
            BanReason::Nickname => c"st_srp_ban_reason_nickname",
            BanReason::Racism => c"st_srp_ban_reason_racism",
            BanReason::Politics => c"st_srp_ban_reason_politics",
            BanReason::FalseInformation => c"st_srp_ban_reason_false_information",
        }
    }
}

/// When a ban ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BanEnd {
    Never,
    /// This many seconds from the answer.
    In(u32),
}

/// What follows a ban's code in srp's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BanTold {
    pub reason: BanReason,
    pub end: BanEnd,
}

impl BanTold {
    /// Its bytes: the reason's id, then the seconds left of a ban that ends.
    pub fn encode(self) -> ([u8; 5], usize) {
        let mut bytes = [self.reason.id(), 0, 0, 0, 0];
        match self.end {
            BanEnd::Never => (bytes, 1),
            BanEnd::In(seconds) => {
                bytes[1..].copy_from_slice(&seconds.to_le_bytes());
                (bytes, 5)
            }
        }
    }

    /// The bytes after the code; None for none (a server that tells
    /// nothing), a reason this game does not know, or any other length.
    pub fn decode(bytes: &[u8]) -> Option<BanTold> {
        let (reason, end) = match *bytes {
            [id] => (id, BanEnd::Never),
            [id, a, b, c, d] => (id, BanEnd::In(u32::from_le_bytes([a, b, c, d]))),
            _ => return None,
        };
        Some(BanTold { reason: BanReason::from_id(reason)?, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_keys_are_one_each() {
        for (id, reason) in BanReason::ALL.iter().enumerate() {
            assert_eq!(reason.id() as usize, id);
            assert_eq!(BanReason::from_id(id as u8), Some(*reason));
            assert!(BanReason::ALL.iter().filter(|other| other.key() == reason.key()).count() == 1);
        }
        assert_eq!(BanReason::from_id(14), None);
    }

    #[test]
    fn a_ban_told_round_trips() {
        for told in [
            BanTold { reason: BanReason::FarmPressure, end: BanEnd::In(1800) },
            BanTold { reason: BanReason::Cheating, end: BanEnd::Never },
            BanTold { reason: BanReason::Administration, end: BanEnd::In(u32::MAX) },
        ] {
            let (bytes, len) = told.encode();
            assert_eq!(BanTold::decode(&bytes[..len]), Some(told));
        }
        assert_eq!(BanTold::encode(BanTold { reason: BanReason::FarmPressure, end: BanEnd::In(1800) }).0, [2, 0x08, 0x07, 0, 0]);
        // No tail, an unknown reason, a cut tail.
        assert_eq!(BanTold::decode(&[]), None);
        assert_eq!(BanTold::decode(&[99]), None);
        assert_eq!(BanTold::decode(&[2, 8, 7]), None);
    }
}
