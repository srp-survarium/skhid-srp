//! Patch 192's boundary: the two finds' operands read once into what the guards decide on.
use super::Context;
use super::world::read;
use crate::raw;

/// A player's game_object part as the log line says it.
#[derive(Clone, Copy)]
pub struct PlayerSeen {
    pub place: u8,
    pub kit: u8,
    pub time: u32,
    /// The player's own count of the zones he is in (+2E4h).
    pub zones: u8,
    /// How many subscribers his list holds.
    pub subscribers: u32,
}

/// The zone side of a subscriber: its sensor's vtable (0 when the subscriber is no zone's) and the
/// zone's inside list's size.
#[derive(Clone, Copy)]
pub struct ZoneSeen {
    pub vtable: u32,
    pub inside: u32,
}

#[derive(Clone, Copy)]
pub struct MissingSubscriber {
    pub player: PlayerSeen,
    pub subscriber: u32,
    pub zone: ZoneSeen,
}

/// What unsubscribe_from_actions' find came back with.
pub enum UnsubscribeFind {
    Found,
    Missing(MissingSubscriber),
}

#[derive(Clone, Copy)]
pub struct MissingInside {
    pub listener: u32,
    pub inside: u32,
    pub owner_vtable: u32,
    /// The left object's owner when it is a player's part.
    pub player: Option<PlayerSeen>,
}

/// Whether on_objects_left's object is in the zone's inside list.
pub enum LeftFind {
    Found,
    Missing(MissingInside),
}

/// # Safety
/// `part` is a live player's game_object part.
unsafe fn player_seen(part: raw::PlayerGameObject) -> PlayerSeen {
    unsafe {
        let (begin, end) = (read(part.subscribers()), read(part.subscribers_end()));
        PlayerSeen {
            place: read(part.place()),
            kit: read(part.kit()),
            time: read(part.time()),
            zones: read(part.zones()),
            subscribers: end.wrapping_sub(begin) / 4,
        }
    }
}

const ZONE_SENSOR_VTABLES: [u32; 2] = [raw::image::CLIENT_ZONE_SENSOR_VTABLE, raw::image::CORE_ZONE_SENSOR_VTABLE];

impl UnsubscribeFind {
    pub fn at(ctx: &Context<'_>) -> UnsubscribeFind {
        use raw::site::zones_unsubscribe_find as site;
        // SAFETY: the site's registers: the find's result and the vector's end (values), the
        // player's live part (EBX) and the argument's slot (EDI) on the simulation's thread; the
        // subscriber is read as a zone's only when its sensor's vtable is one.
        unsafe {
            let (found, end) = (site::found(ctx), site::end(ctx));
            if found != end {
                return UnsubscribeFind::Found;
            }
            let part = site::object(ctx).expect("unsubscribe_from_actions' player (EBX)");
            let slot = site::slot(ctx).expect("unsubscribe_from_actions' argument (EDI)");
            let subscriber = slot.subscriber();
            let zone = subscriber
                .filter(|s| ZONE_SENSOR_VTABLES.contains(&read(s.listener_vtable())))
                .map_or(ZoneSeen { vtable: 0, inside: 0 }, |s| ZoneSeen {
                    vtable: read(s.listener_vtable()),
                    inside: read(s.inside_end()).wrapping_sub(read(s.inside())) / 4,
                });
            UnsubscribeFind::Missing(MissingSubscriber {
                player: player_seen(part),
                subscriber: subscriber.map_or(0, |s| s.address()),
                zone,
            })
        }
    }
}

impl LeftFind {
    pub fn at(ctx: &Context<'_>) -> LeftFind {
        use raw::site::zones_left_find as site;
        // SAFETY: the site's registers on the simulation's thread: the zone's live sensor (EDI),
        // the left object's holder (EBP, the copy on_objects_left keeps) and its owner (ESI); the
        // inside list's entries are the zone's, each a holder or 0. The find (0x308A40) compares
        // the objects the holders name.
        unsafe {
            let listener = site::listener(ctx).expect("on_objects_left's sensor (EDI)");
            let holder = site::holder(ctx).expect("on_objects_left's holder (EBP)");
            let owner = site::object(ctx).expect("on_objects_left's object (ESI)");
            let object = read(holder.object());
            let (first, end) = (listener.inside(), listener.inside_end());
            let count = match (first, end) {
                (Some(first), Some(end)) => end.index_from(first).unwrap_or(0),
                _ => 0,
            };
            let found = first.is_some_and(|first| {
                (0..count).any(|i| first.at(i).holder().is_some_and(|h| read(h.object()) == object))
            });
            if found {
                return LeftFind::Found;
            }
            let vtable = read(owner.vtable());
            LeftFind::Missing(MissingInside {
                listener: listener.address(),
                inside: count as u32,
                owner_vtable: vtable,
                player: (vtable == raw::image::PLAYER_GAME_OBJECT_VTABLE)
                    .then(|| player_seen(owner.player())),
            })
        }
    }
}
