//! `A9`: srp's message of an account's scope reticles (`reticle-choice.md`, "The build's
//! contract"). 0.26g's id A9 is the premium discount, which srp never makes (there is no premium
//! account); its transport class is "reliable, unique and ordered" (`transport.md` §4.1), so it is
//! taken in order with the inventory and the account. The patched client takes it at its
//! dispatcher before the jump table (`lobby_client::on_message_received` 0x274A39, patch
//! `reticle`), and keeps it for the process: the lobby's window and the match's drawing read it.
//!
//! It goes **before every inventory** (`89`): the inventory's arrival makes the lobby movie's
//! items (setup_inventory_data 0x288520), and the patch gives each scope item with lists its
//! `srp_reticle` member there from the table it holds.
//!
//! ```text
//! u8 n lists × {
//!     u16 dict id,
//!     u8 n patterns × { u8 len, name, u8 len, model },
//!     u8 n sizes × f32,
//!     u8 n colours × { u8 len, name, f32 r, f32 g, f32 b },
//! }
//! u16 n items × { u32 item id, u16 dict id, u8 pattern, u8 size, u8 colour, u8 brightness }
//! ```
//!
//! The lists are those of every scope type with lists (a gun picked up in a match may hold a scope
//! the account lacks: its aimed view draws that scope's default), each once; every item names one
//! of them and indexes it.

use super::super::reticles::{
    MAX_SCOPES, MODEL_LIMIT, NAME_LIMIT, Reticle, ReticleLists,
};
use super::{ServerId, ServerMessage, WireError, Writer, at_most};

/// The most items the client's table holds.
pub const MAX_ITEMS: usize = 128;

/// One scope item of the account and its reticle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScopeReticle {
    pub item_id: u32,
    pub dict_id: u16,
    pub reticle: Reticle,
}

/// `A9` of `items`, with the lists of their dict ids (`lists`, each dict id once). Each item's
/// dict id must be one of `lists`, its reticle held by those lists.
pub fn scope_reticles(
    lists: &[(u16, &ReticleLists)],
    items: &[ScopeReticle],
) -> Result<ServerMessage, WireError> {
    at_most("scope reticle lists", lists.len(), MAX_SCOPES)?;
    at_most("scope reticle items", items.len(), MAX_ITEMS)?;
    let mut writer = Writer::new();
    writer.u8(lists.len() as u8);
    for (index, &(dict_id, list)) in lists.iter().enumerate() {
        if lists[..index].iter().any(|&(earlier, _)| earlier == dict_id) {
            return Err(WireError::BadValue("a scope's reticle lists twice"));
        }
        writer.u16(dict_id);
        writer.u8(list.patterns().len() as u8);
        for pattern in list.patterns() {
            writer.str(pattern.name.as_str(), NAME_LIMIT);
            writer.str(pattern.model.as_str(), MODEL_LIMIT);
        }
        writer.u8(list.sizes().len() as u8);
        for size in list.sizes() {
            writer.f32(size.get());
        }
        writer.u8(list.colours().len() as u8);
        for colour in list.colours() {
            writer.str(colour.name.as_str(), NAME_LIMIT);
            for channel in colour.rgb {
                writer.f32(channel);
            }
        }
    }
    writer.u16(items.len() as u16);
    for item in items {
        let held = lists
            .iter()
            .find(|&&(dict_id, _)| dict_id == item.dict_id)
            .is_some_and(|&(_, list)| list.holds(item.reticle));
        if !held {
            return Err(WireError::BadValue(
                "a scope item's reticle not held by its lists in the message",
            ));
        }
        let numbers = item.reticle.numbers();
        writer
            .u32(item.item_id)
            .u16(item.dict_id)
            .u8(numbers.pattern)
            .u8(numbers.size)
            .u8(numbers.colour)
            .u8(numbers.brightness);
    }
    Ok(writer.done(ServerId::ScopeReticles))
}

#[cfg(test)]
mod tests {
    use super::super::super::reticles::fixtures::lists;
    use super::super::super::reticles::{Numbers, Reticle};
    use super::super::client_model::{Client, Decoded};
    use super::super::signed_in;
    use super::*;

    /// A scope item of the test data's dictionary (0.26g's Leupold, category 22).
    const SCOPE: u16 = 69;

    fn signed_in_client() -> Client {
        let mut client = Client::new();
        client.receive(&signed_in(0, false)).unwrap();
        client
    }

    #[test]
    fn the_message_goes_out_byte_for_byte_and_reads_back() {
        let list = lists(&["dot", "ncstar_cross"], &["red", "green"]);
        let chosen = list
            .reticle(Numbers { pattern: 1, size: 2, colour: 1, brightness: 9 })
            .unwrap();
        let items = [
            ScopeReticle { item_id: 7, dict_id: SCOPE, reticle: chosen },
            ScopeReticle { item_id: 9, dict_id: SCOPE, reticle: Reticle::DEFAULT },
        ];
        let message = scope_reticles(&[(SCOPE, &list)], &items).unwrap();
        assert_eq!(message.id_byte(), 0xA9);
        let body = message.body();
        assert_eq!(&body[..4], &[1, SCOPE as u8, 0, 2]);
        assert_eq!(body[4], 3);
        assert_eq!(&body[5..8], b"dot");
        assert_eq!(body[8] as usize, "weapons/reticles_069/dot".len());
        let tail = &body[body.len() - 2 - 2 * 10..];
        assert_eq!(tail[..2], [2, 0]);
        assert_eq!(tail[2..12], [7, 0, 0, 0, SCOPE as u8, 0, 1, 2, 1, 9]);
        assert_eq!(tail[12..], [9, 0, 0, 0, SCOPE as u8, 0, 0, 0, 0, 6]);
        let Decoded::ScopeReticles { lists: read, items: read_items } =
            signed_in_client().receive(&message).unwrap()
        else {
            panic!("not A9");
        };
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].patterns[1], ("ncstar_cross".into(), "weapons/reticles_069/ncstar_cross".into()));
        assert_eq!(read[0].sizes, vec![1.0, 0.75, 0.5, 0.35]);
        assert_eq!(read[0].colours[1], ("green".into(), [0.0, 1.0, 0.0]));
        assert_eq!(read_items[0], (7, SCOPE, chosen.numbers()));
        // None at all: two bytes and a count.
        let empty = scope_reticles(&[], &[]).unwrap();
        assert_eq!(empty.body(), &[0, 0, 0]);
        assert!(signed_in_client().receive(&empty).is_ok());
    }

    #[test]
    fn an_item_its_lists_do_not_hold_is_refused() {
        let long = lists(&["dot", "ncstar_cross"], &["red", "green"]);
        let short = lists(&["dot"], &["red"]);
        let chosen = long
            .reticle(Numbers { pattern: 1, size: 0, colour: 0, brightness: 6 })
            .unwrap();
        let item = ScopeReticle { item_id: 7, dict_id: SCOPE, reticle: chosen };
        assert!(scope_reticles(&[(SCOPE, &short)], &[item]).is_err());
        assert!(scope_reticles(&[], &[item]).is_err());
        assert!(scope_reticles(&[(SCOPE, &long), (SCOPE, &long)], &[]).is_err());
        let many = vec![item; MAX_ITEMS + 1];
        assert!(scope_reticles(&[(SCOPE, &long)], &many).is_err());
    }
}
