//! Patch 207: the lobby movie's `srp_set_upgrade_value(host, upgrade, value)`
//! becomes the lobby's `46 05 <active kit> <host> <upgrade> <u64 value>`, the
//! request the decal editor's accept sends (`docs/notes/client_patch.md`
//! 3.111). The Attachments window calls it when the player puts a weapon's
//! second sight on its other side (UpgradeWindow.onSideButton, SightPlace:
//! 0 left, 1 right); the server checks the side against what the weapon
//! holds (session `items_rules::place_upgrade_with`) and sends the account
//! after it, so the movie shows the side the server holds.
//!
//! The one hook is `upgrade_value.toml`'s, at the start of the lobby movie's
//! every call (`lobby.callback`, lobby_menu::scaleform_callback 0x28C2C0):
//! this method is ours, taken whatever it holds (the menu's own dispatch
//! knows no such name); every other call goes on to the game.
//!
//! The call is parsed at the boundary: the value is a [`Side`] (anything
//! else: nothing sent, one line in the patches' log, once a run), the host
//! and the upgrade are items of the account's inventory, and the host's
//! records are a weapon kit whose second sight is that upgrade (its item, or
//! the dict id its kit names): else nothing is sent. The client's copy of
//! the host then holds the new side (the kit packed again), as the lobby will
//! after the request. Nothing a match computes.

#![forbid(unsafe_code)]

use weapon_kit::{Held, Logical, Side, Slot, Wire};

use crate::vostok::log::{self, Piece};
use crate::vostok::{Context, Flow, GameCell, LobbyCall, LobbyItem, Reg, Thread, UpgradeValueOnLobbyCallExit};

/// The lobby movie's method.
const METHOD: &[u8] = b"srp_set_upgrade_value";

/// The call's arguments, parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Request {
    host: u32,
    upgrade: u32,
    side: Side,
}

/// Why nothing was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    /// The three arguments are not whole numbers.
    Arguments,
    /// The value is no side (0 left, 1 right).
    NotASide(u32),
    /// No lobby client to send with.
    NoLobby,
    /// The host or the upgrade is not an item of the account's inventory.
    NotOwned,
    /// The host's records are no weapon kit with a second sight.
    NoSecondSight,
    /// The host's second sight is another item.
    OtherSight,
}

/// Whether the line of a value that is no side was written this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SideLine {
    Unsaid,
    Said,
}

impl SideLine {
    /// Another value that is no side: the state after it, and whether it gets the line.
    fn refused(self) -> (SideLine, bool) {
        (SideLine::Said, self == SideLine::Unsaid)
    }
}

static SIDE_LINE: GameCell<SideLine> = GameCell::new(SideLine::Unsaid);

/// The three arguments as a request.
fn request(host: Option<u32>, upgrade: Option<u32>, value: Option<u32>) -> Result<Request, Refusal> {
    let (Some(host), Some(upgrade), Some(value)) = (host, upgrade, value) else { return Err(Refusal::Arguments) };
    let side = Side::of_value(u64::from(value)).ok_or(Refusal::NotASide(value))?;
    Ok(Request { host, upgrade, side })
}

/// The host's records with its second sight, `upgrade`, at `side`: what the
/// lobby holds after the request.
fn placed(host: &LobbyItem, upgrade: &LobbyItem, side: Side) -> Result<Wire, Refusal> {
    let records = host.records.ok_or(Refusal::NoSecondSight)?;
    let mut logical = Logical::unpack(records.as_slice()).map_err(|_| Refusal::NoSecondSight)?;
    let held = logical.get(Slot::SecondSight).ok_or(Refusal::NoSecondSight)?;
    if !held.is(upgrade.id, u32::from(upgrade.dict)) {
        return Err(Refusal::OtherSight);
    }
    let moved = match held {
        Held::Item { id, dict, .. } => Held::Item { id, dict, value: side.value() },
        Held::Named(dict, _) => Held::Named(dict, side.value()),
    };
    logical.set(Slot::SecondSight, Some(moved));
    Ok(logical.pack())
}

/// The call's request sent; the client's copy given the new side.
fn send(call: &LobbyCall<'_>, request: Request) -> Result<(), Refusal> {
    let inventory = call.inventory().ok_or(Refusal::NoLobby)?;
    let host = inventory.item(request.host).ok_or(Refusal::NotOwned)?;
    let upgrade = inventory.item(request.upgrade).ok_or(Refusal::NotOwned)?;
    let records = placed(&host, &upgrade, request.side)?;
    if inventory.place_upgrade(request.host, request.upgrade, request.side.value(), &records).is_err() {
        log::line(&[Piece::Text("upgrade_value: sent, the client's copy of "), Piece::Dec(request.host),
                    Piece::Text(" left to the account")]);
    }
    Ok(())
}

fn say(thread: &Thread, refusal: Refusal, call: (Option<u32>, Option<u32>)) {
    let (host, upgrade) = (call.0.unwrap_or(0), call.1.unwrap_or(0));
    match refusal {
        Refusal::NotASide(value) => {
            let (next, line) = SIDE_LINE.get(thread).refused();
            SIDE_LINE.set(thread, next);
            if line {
                log::line(&[Piece::Text("upgrade_value: srp_set_upgrade_value("), Piece::Dec(host), Piece::Text(", "),
                            Piece::Dec(upgrade), Piece::Text(", "), Piece::Dec(value),
                            Piece::Text("): the value is no side (0 left, 1 right), nothing sent (said once)")]);
            }
        }
        other => {
            let why = match other {
                Refusal::Arguments => "the arguments are no ids and value",
                Refusal::NoLobby => "no lobby client",
                Refusal::NotOwned => "the host or the upgrade is no item of the inventory",
                Refusal::NoSecondSight => "the host holds no second sight",
                _ => "the host's second sight is another item",
            };
            log::refusal(thread, &[Piece::Text("upgrade_value: "), Piece::Dec(host), Piece::Text(" "),
                                   Piece::Dec(upgrade), Piece::Text(": "), Piece::Text(why), Piece::Text(", nothing sent")]);
        }
    }
}

/// lobby_menu::scaleform_callback's start (0x28C2C0): every call of the
/// lobby movie.
pub fn on_lobby_call(ctx: &mut Context<'_>) -> Flow<UpgradeValueOnLobbyCallExit> {
    {
        let Some(call) = LobbyCall::at(ctx) else { return Flow::Continue };
        if call.method().to_bytes() != METHOD {
            return Flow::Continue;
        }
        let ids = (call.uint_argument(0), call.uint_argument(1));
        if let Err(refusal) = request(ids.0, ids.1, call.uint_argument(2)).and_then(|request| send(&call, request)) {
            say(call.thread(), refusal, ids);
        }
    }
    // the menu's own handlers answer al 1, handled (GFx's caller drops it)
    ctx.set_reg(Reg::Eax, 1);
    Flow::Exit(UpgradeValueOnLobbyCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::{LobbyItem, Refusal, Request, SideLine, placed, request};
    use crate::vostok::Records;
    use weapon_kit::{Logical, Record, Side, Slot};

    const OPTIC: u16 = 640;
    const RED_DOT: u16 = 641;
    const SCOPE_ITEM: u32 = 70;
    const SIGHT_ITEM: u32 = 71;

    fn item(id: u32, dict: u16, records: &[Record]) -> LobbyItem {
        LobbyItem { id, dict, records: Records::of(records) }
    }

    fn weapon(logical: &Logical) -> LobbyItem {
        item(10, 300, logical.pack().as_slice())
    }

    #[test]
    fn a_value_that_is_no_side_is_said_once() {
        assert_eq!(SideLine::Unsaid.refused(), (SideLine::Said, true));
        assert_eq!(SideLine::Said.refused(), (SideLine::Said, false));
    }

    #[test]
    fn the_value_is_a_side() {
        assert_eq!(request(Some(1), Some(2), Some(0)), Ok(Request { host: 1, upgrade: 2, side: Side::Left }));
        assert_eq!(request(Some(1), Some(2), Some(1)), Ok(Request { host: 1, upgrade: 2, side: Side::Right }));
        assert_eq!(request(Some(1), Some(2), Some(2)), Err(Refusal::NotASide(2)));
        assert_eq!(request(Some(1), None, Some(1)), Err(Refusal::Arguments));
    }

    #[test]
    fn a_second_sight_alone_heads_its_kit() {
        let mut logical = Logical::new();
        logical.put_second_sight(SIGHT_ITEM, u32::from(OPTIC), Side::Left);
        let records = placed(&weapon(&logical), &item(SIGHT_ITEM, OPTIC, &[]), Side::Right).unwrap();
        let held = Logical::unpack(records.as_slice()).unwrap().get(Slot::SecondSight).unwrap();
        assert_eq!(held.value(), 1);
        assert_eq!(records.as_slice()[0].id, SIGHT_ITEM);
    }

    #[test]
    fn a_second_sight_beside_a_scope_is_named_by_its_dict() {
        let mut logical = Logical::new();
        logical.put(Slot::Scope, SCOPE_ITEM, u32::from(OPTIC));
        logical.put_second_sight(SIGHT_ITEM, u32::from(RED_DOT), Side::Right);
        let host = weapon(&logical);
        let records = placed(&host, &item(SIGHT_ITEM, RED_DOT, &[]), Side::Left).unwrap();
        let unpacked = Logical::unpack(records.as_slice()).unwrap();
        assert_eq!(unpacked.get(Slot::SecondSight).unwrap().value(), 0);
        assert_eq!(unpacked.get(Slot::Scope), Logical::unpack(host.records.unwrap().as_slice()).unwrap().get(Slot::Scope));
        assert_eq!(placed(&host, &item(SIGHT_ITEM, OPTIC, &[]), Side::Left), Err(Refusal::OtherSight));
        assert_eq!(placed(&host, &item(SCOPE_ITEM, OPTIC, &[]), Side::Left), Err(Refusal::OtherSight));
    }

    #[test]
    fn no_second_sight_no_request() {
        let mut logical = Logical::new();
        logical.put(Slot::Scope, SCOPE_ITEM, u32::from(OPTIC));
        assert_eq!(placed(&weapon(&logical), &item(SCOPE_ITEM, OPTIC, &[]), Side::Left), Err(Refusal::NoSecondSight));
        assert_eq!(placed(&item(5, 9, &[]), &item(SIGHT_ITEM, OPTIC, &[]), Side::Left), Err(Refusal::NoSecondSight));
    }
}
