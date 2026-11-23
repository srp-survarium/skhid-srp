//! Patch 192: a damage zone and the players it holds out of step no longer move memory.
//!
//! The stock game erases without testing its finds, in two places (2026-11-22, a crash of every
//! game of an Artifact Hunt match: a 39 MB memmove):
//! - `base_player::unsubscribe_from_actions` (0x2DBE90): a subscriber not in the player's list
//!   makes a memmove of -4 bytes (0x2DBEBA) and a vector end before its begin;
//! - `damage_zone_core::on_objects_left` (0x319780): an object not in the zone's inside list pops
//!   the list's last entry, another object's.
//!
//! Both skip the erase of what is not there, say so in the patches' log (the player's place and
//! kit, the zone, the time and both lists' sizes) and the match plays on (the owner, 2026-11-22:
//! "noone did anything wrong"). Nothing is erased that was there, so a match that would not have
//! crashed computes the same.
#![forbid(unsafe_code)]

use crate::vostok::log::{Piece::{Dec, Hex, Text}, line};
use crate::vostok::zone_guard::{LeftFind, UnsubscribeFind};
use crate::vostok::{Context, Flow, ZoneGuardsOnLeftFindExit, ZoneGuardsOnUnsubscribeFindExit};

pub fn on_unsubscribe_find(ctx: &mut Context<'_>) -> Flow<ZoneGuardsOnUnsubscribeFindExit> {
    match UnsubscribeFind::at(ctx) {
        UnsubscribeFind::Found => Flow::Continue,
        UnsubscribeFind::Missing(m) => {
            line(&[Text("zones: unsubscribe of a zone the player is not subscribed to, skipped: place "),
                   Dec(m.player.place.into()), Text(" kit "), Dec(m.player.kit.into()), Text(" at "),
                   Dec(m.player.time), Text(" ms, zones "), Dec(m.player.zones.into()), Text(", subscribers "),
                   Dec(m.player.subscribers), Text("; subscriber "), Hex(m.subscriber), Text(", its zone's sensor "),
                   Hex(m.zone.vtable), Text(" inside "), Dec(m.zone.inside)]);
            Flow::Exit(ZoneGuardsOnUnsubscribeFindExit::Missing)
        }
    }
}

pub fn on_left_find(ctx: &mut Context<'_>) -> Flow<ZoneGuardsOnLeftFindExit> {
    match LeftFind::at(ctx) {
        LeftFind::Found => Flow::Continue,
        LeftFind::Missing(m) => {
            match m.player {
                Some(p) => line(&[Text("zones: a left player not in the zone's inside list, its erase skipped: place "),
                                  Dec(p.place.into()), Text(" kit "), Dec(p.kit.into()), Text(" at "), Dec(p.time),
                                  Text(" ms, zones "), Dec(p.zones.into()), Text(", subscribers "), Dec(p.subscribers),
                                  Text("; sensor "), Hex(m.listener), Text(" inside "), Dec(m.inside)]),
                None => line(&[Text("zones: a left object not in the zone's inside list, its erase skipped: owner "),
                               Hex(m.owner_vtable), Text("; sensor "), Hex(m.listener), Text(" inside "), Dec(m.inside)]),
            }
            Flow::Exit(ZoneGuardsOnLeftFindExit::Missing)
        }
    }
}
