//! The shop's "Hide owned" box (`shop-owned-mark.md`), kept with the
//! account: srp's own wire, on the ranking page's request (`3B`) at
//! reserved starts and its reply (`91`), as the presets are (`lobby.md`
//! §2.16.6). Only srp's changed lobby movie asks; a stock game never does.
//!
//! [`SHOP_FILTER_FIRST`] asks for the account's choice, answered with
//! [`answer`]; [`SHOP_FILTER_FIRST`] + 1 + [`OwnedItems::code`] keeps a
//! click's, not answered.

use super::push::{Ranked, elo_rows};
use super::{ServerMessage, WireError};

/// The start that asks for the choice; the next two keep one.
pub const SHOP_FILTER_FIRST: u32 = 2_096_000_000;
/// The first field of the answer's name.
pub const SHOP_FILTER_VERSION: &str = "H1";

/// Whether the shop lists the items the account owns: the box unticked
/// (the default: nothing kept) or ticked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnedItems {
    #[default]
    Shown,
    Hidden,
}

impl OwnedItems {
    pub const ALL: [OwnedItems; 2] = [OwnedItems::Shown, OwnedItems::Hidden];

    /// Its number on the wire and in the account's row: 0 unticked, 1 ticked.
    pub fn code(self) -> u8 {
        match self {
            OwnedItems::Shown => 0,
            OwnedItems::Hidden => 1,
        }
    }

    /// The choice of a kept number: 1 is ticked, any other unticked.
    pub fn of_code(code: i64) -> OwnedItems {
        if code == i64::from(OwnedItems::Hidden.code()) {
            OwnedItems::Hidden
        } else {
            OwnedItems::Shown
        }
    }
}

/// A request of the box, by its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopFilterRequest {
    /// The account's choice, for the box at a sign-in.
    Ask,
    /// A click: keep this one.
    Keep(OwnedItems),
}

/// The request a reserved start is, if it is one of the box's.
pub fn shop_filter_request(start: u32) -> Option<ShopFilterRequest> {
    if start == SHOP_FILTER_FIRST {
        return Some(ShopFilterRequest::Ask);
    }
    OwnedItems::ALL
        .into_iter()
        .find(|owned| start == SHOP_FILTER_FIRST + 1 + u32::from(owned.code()))
        .map(ShopFilterRequest::Keep)
}

/// `91` at [`SHOP_FILTER_FIRST`]: one row, `H1;<code>` ([`OwnedItems::code`]).
pub fn answer(owned: OwnedItems) -> Result<ServerMessage, WireError> {
    let row = Ranked {
        name: format!("{SHOP_FILTER_VERSION};{}", owned.code()),
        rating: 0,
        rating_before: 0,
    };
    elo_rows(&[(SHOP_FILTER_FIRST, &row)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_start_is_one_request_inside_an_int_past_the_lobby_pick() {
        const {
            assert!(SHOP_FILTER_FIRST > super::super::lobby_pick::LOBBY_PICK_BASE + 4);
            assert!(SHOP_FILTER_FIRST + 2 <= i32::MAX as u32);
        }
        assert_eq!(
            shop_filter_request(SHOP_FILTER_FIRST),
            Some(ShopFilterRequest::Ask)
        );
        assert_eq!(
            shop_filter_request(SHOP_FILTER_FIRST + 1),
            Some(ShopFilterRequest::Keep(OwnedItems::Shown))
        );
        assert_eq!(
            shop_filter_request(SHOP_FILTER_FIRST + 2),
            Some(ShopFilterRequest::Keep(OwnedItems::Hidden))
        );
        for start in [SHOP_FILTER_FIRST - 1, SHOP_FILTER_FIRST + 3] {
            assert_eq!(shop_filter_request(start), None);
        }
    }

    #[test]
    fn a_kept_number_other_than_one_is_unticked() {
        for owned in OwnedItems::ALL {
            assert_eq!(OwnedItems::of_code(owned.code().into()), owned);
        }
        for code in [-1, 2, 255, i64::MAX] {
            assert_eq!(OwnedItems::of_code(code), OwnedItems::Shown);
        }
    }

    #[test]
    fn the_answer_is_one_row_at_the_first_start() {
        let mut client = super::super::client_model::Client::signed_in();
        for owned in OwnedItems::ALL {
            assert_eq!(
                client.receive(&answer(owned).unwrap()).unwrap(),
                super::super::client_model::Decoded::EloList(vec![(
                    format!("H1;{}", owned.code()),
                    SHOP_FILTER_FIRST,
                    0,
                    0
                )])
            );
        }
    }
}
