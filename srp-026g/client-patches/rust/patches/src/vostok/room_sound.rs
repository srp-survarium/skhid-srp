//! The sites of patch 155, the room acoustics (docs/notes/room-acoustics.md):
//!
//! - [`ItemRead`]: `composite_sound.item_read`, a composite's item as its
//!   cook reads it: its `sound_environment` tag, and the byte of the item
//!   where its room is kept (padding the game copies and never reads);
//! - [`ItemPlay`]: `composite_sound.item_play`, an item about to play: its
//!   room, and where the sound is heard from (a hud sound at the listener,
//!   any other at its proxy);
//! - [`Listening`]: `game_world.listener`, every frame of a game world: the
//!   listener's position and the local player's feet while he is alive.

use core::ffi::CStr;

use super::config::ConfigValue;
use super::context::Context;
use super::world::{floats, read, GameWorld};
use crate::raw;

/// A proxy's sound type that is not placed: heard the same wherever the
/// listener is (calculate_3d_pan 0x57D88: 3 hud).
const HUD: u32 = 3;

/// A composite's item as its cook reads it.
pub struct ItemRead<'t> {
    tag: Option<&'t CStr>,
    frame: raw::CompositeItemFrame,
}

impl<'t> ItemRead<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<ItemRead<'t>> {
        use raw::site::composite_sound_item_read as site;
        // SAFETY: the site's EBX is the item's config table, a node of the
        // composite's config the cook holds loaded while it runs; ESP its
        // frame, on the cook's thread.
        unsafe {
            let frame = site::frame(ctx)?;
            let tag = site::item(ctx)
                .and_then(|node| ConfigValue::read(node).ok())
                .and_then(|value| value.table().ok())
                .and_then(|table| table.get(c"sound_environment").ok().flatten())
                .and_then(|value| value.text().ok());
            Some(ItemRead { tag, frame })
        }
    }

    /// Its `sound_environment`, if it has one as a text.
    pub fn tag(&self) -> Option<&'t CStr> {
        self.tag
    }

    /// Keeps `room` with the item (0: none). Written at every item, so no
    /// item keeps a byte of the stack.
    pub fn set_room(&self, room: u8) {
        // SAFETY: the byte after the item's `enabled` in the cook's frame,
        // copied with it into the composite's item (0x63F30).
        unsafe { self.frame.room().write_volatile(room) }
    }
}

/// Where a sound is heard from, for its room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heard {
    /// A hud sound (first person, not placed): in the listener's room.
    AtListener,
    /// A placed sound: at its source.
    AtSource([f32; 3]),
}

/// A composite's item about to play.
pub struct ItemPlay {
    room: u8,
    heard: Heard,
}

impl ItemPlay {
    pub fn at(ctx: &Context<'_>) -> Option<ItemPlay> {
        use raw::site::composite_sound_item_play as site;
        // SAFETY: the site's EDX is the composite's items, EBP the byte
        // offset of one of them (a multiple of its size, below the end: the
        // loop's bound, 0x63EAF); ESP the emit's frame with the positions it
        // computed and its proxy, alive while it runs.
        unsafe {
            let frame = site::frame(ctx)?;
            let offset = site::offset(ctx) as usize;
            let item = site::items(ctx)?.at(offset / raw::CompositeItem::SIZE);
            let proxy = frame.proxy()?;
            let heard = if read(proxy.sound_type()) == HUD {
                Heard::AtListener
            } else {
                Heard::AtSource(floats(frame.emitter()))
            };
            Some(ItemPlay { room: read(item.room()), heard })
        }
    }

    /// Its room tag (0: none).
    pub fn room(&self) -> u8 {
        self.room
    }

    pub fn heard(&self) -> Heard {
        self.heard
    }
}

/// A game world's frame, as it gives its scene the listener.
pub struct Listening {
    pub listener: [f32; 3],
    /// The local player's feet, while he is in the match and alive.
    pub alive_player: Option<[f32; 3]>,
}

impl Listening {
    pub fn at(ctx: &Context<'_>) -> Option<Listening> {
        // SAFETY: the site's ESI is the game world, ticking on the game's
        // thread; its listener's three floats are its own.
        let world = unsafe { raw::site::game_world_listener::world(ctx) }?;
        let listener = unsafe { floats(world.listener()) };
        let alive_player = GameWorld::of(world, ctx.thread())
            .network_client()
            .and_then(|client| client.local_player())
            .filter(|player| player.inserted() && player.alive())
            .map(|player| player.feet());
        Some(Listening { listener, alive_player })
    }
}
