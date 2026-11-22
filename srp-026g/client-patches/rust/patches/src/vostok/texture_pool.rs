//! The streamed textures' pools as scene::process_streaming hands their slots to requests, for
//! patch 182 (texture_pool; client_patch.md 3.99).
//!
//! - resource_pools (0x11B090) makes one pool for each key of a fixed table: a size (top mip's
//!   width and height, its mips), BC1 or BC3, a fixed number of slots each (60 of 2048x2048
//!   BC1, 30 of BC3, 120 of 1024x1024, ...), whatever the video memory. The resource manager
//!   keeps them in an STLport map from the key to the pool (+94914h).
//! - Each batch, process_streaming (0x154020) gives every request that changes a texture's
//!   mips a free, unreserved slot of exactly the size it wants and reserves it; the load then
//!   takes that slot. A request whose size has no such slot is cancelled (0x1C89C0), so a
//!   texture that wants a full size whose slots are all taken keeps the mips it has, however
//!   few, until one of them frees: [`NoSlot`] is that request, at `streaming.no_slot`.
//!
//! The pools and requests are the render thread's: process_streaming runs on it, and the
//! async worker that makes the requests has finished with them before it.

use super::context::Context;
use super::world::read;
use crate::raw;

/// A texture's size as a pool keys it: the top mip's width and height, and its mips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub width: u32,
    pub height: u32,
    pub mips: u32,
}

impl Level {
    /// The size `steps` mips down, if the texture has that mip.
    pub fn smaller(self, steps: u32) -> Option<Level> {
        if steps >= self.mips || steps >= 32 {
            return None;
        }
        let level = Level { width: self.width >> steps, height: self.height >> steps, mips: self.mips - steps };
        (level.width > 0 && level.height > 0).then_some(level)
    }
}

/// What a request has and asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Want {
    /// Its texture's index in the scene's streamable textures.
    pub texture: u32,
    pub resident: Level,
    pub wanted: Level,
    /// The rest of the pools' key: the array size and the typeless DXGI format.
    pub array_size: u32,
    pub format: u32,
}

/// How the pool of one key stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    /// The table has no pool of that key.
    Absent,
    Slots { total: u32, reserved: u32, occupied: Occupants },
}

/// Who occupies a pool's slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occupants {
    /// Textures the scene streams (it can move them to another size).
    pub in_scene: u32,
    /// Textures the scene does not stream, baked camouflage (`user_data/`) ...
    pub baked_elsewhere: u32,
    /// ... and any other: nothing streams them, they keep their size while they live.
    pub other_elsewhere: u32,
}

/// A request at `streaming.no_slot`: no free, unreserved slot of the size it wants.
pub struct NoSlot {
    request: raw::PotentialRequest,
    texture: raw::StreamableTexture,
    /// The scene's streamable textures: the first, and how many.
    streamable: (raw::StreamableTexture, usize),
    pools: Option<raw::TexturePoolMap>,
    want: Want,
}

impl NoSlot {
    /// The request the site holds, with its texture and the pools.
    pub fn at_site(ctx: &Context<'_>) -> Option<NoSlot> {
        use raw::site::streaming_no_slot as site;
        // SAFETY: the site's ESI is the request process_streaming is at and EBP its frame,
        // whose first argument is the scene; the request's texture index is below the scene's
        // streamable textures' count (the game indexes it unchecked at 0x154165); the
        // resource manager is the render's, alive for the game. All the render thread's.
        unsafe {
            let request = site::request(ctx)?;
            let scene = site::frame(ctx)?.scene()?;
            let index = read(request.texture());
            let (first, end) = (scene.streamable()?, scene.streamable_end()?);
            let count = end.index_from(first).expect("the streamable textures' vector");
            assert!((index as usize) < count, "a streaming request's texture past the scene's");
            let texture = first.at(index as usize);
            let format = read(texture.texture().expect("a streamable texture's texture").pool_format());
            let pools = raw::image::render_resources().manager().and_then(|manager| manager.texture_pools());
            let want = Want {
                texture: index,
                resident: Level {
                    width: read(request.resident_width()),
                    height: read(request.resident_height()),
                    mips: read(request.resident_mips()),
                },
                wanted: Level {
                    width: read(request.wanted_width()),
                    height: read(request.wanted_height()),
                    mips: read(request.wanted_mips()),
                },
                array_size: read(request.array_size()),
                format,
            };
            Some(NoSlot { request, texture, streamable: (first, count), pools, want })
        }
    }

    pub fn want(&self) -> Want {
        self.want
    }

    /// The texture's path as the scene keeps it (no ".dds", no folder for a game texture).
    pub fn path(&self) -> &[u8] {
        // SAFETY: the texture's fixed_string, [begin, end), alive while it is streamed.
        unsafe {
            let (begin, end) = (self.texture.path(), self.texture.path_end());
            if begin.is_null() || end < begin {
                return &[];
            }
            core::slice::from_raw_parts(begin.cast_const(), end.offset_from(begin) as usize)
        }
    }

    /// How the pool of the texture's key at `level` stands.
    pub fn pool(&self, level: Level) -> Pool {
        let Some(pool) = self.find(level) else { return Pool::Absent };
        let (mut total, mut reserved, mut occupied) = (0, 0, Occupants::default());
        for slot in slots(pool) {
            total += 1;
            // SAFETY: a slot of a live pool; an occupied one's texture and name are its live
            // texture's (the take writes both, 0x3BD51A).
            unsafe {
                reserved += u32::from(read(slot.reserved()) != 0);
                if read(slot.occupied()) == 0 {
                    continue;
                }
                let texture = slot.texture();
                if self.streams(texture) {
                    occupied.in_scene += 1;
                } else if name_starts(slot.name(), b"user_data") {
                    occupied.baked_elsewhere += 1;
                } else {
                    occupied.other_elsewhere += 1;
                }
            }
        }
        Pool::Slots { total, reserved, occupied }
    }

    /// Whether the scene streams `texture`.
    fn streams(&self, texture: Option<raw::ResTexture>) -> bool {
        let (first, count) = self.streamable;
        // SAFETY: below the vector's count (at_site's).
        (0..count).any(|i| unsafe { first.at(i).texture() } == texture)
    }

    /// Takes a free, unreserved slot of the texture's key at `level` and makes the request ask
    /// for `level`: the slot reserved as process_streaming reserves one (0x1543C0), the
    /// request's mips and size those of `level`. False, and nothing written, when that pool
    /// has no such slot or there is no such pool.
    pub fn take(&mut self, level: Level) -> bool {
        let Some(pool) = self.find(level) else { return false };
        // SAFETY: the pool's slots and the request, the render thread's (module doc); the
        // free test is the game's own (0x154240: neither occupied nor reserved).
        unsafe {
            let Some(slot) = slots(pool).find(|slot| read(slot.occupied()) == 0 && read(slot.reserved()) == 0) else {
                return false;
            };
            slot.reserved().write_volatile(1);
            self.request.wanted_mips().write_volatile(level.mips);
            self.request.wanted_width().write_volatile(level.width);
            self.request.wanted_height().write_volatile(level.height);
        }
        self.want.wanted = level;
        true
    }

    /// The pool of the texture's key at `level` (usage 0, as every pool's).
    fn find(&self, level: Level) -> Option<raw::TexturePool> {
        // SAFETY: the map's nodes, live while the pools are (the render's, for the game).
        unsafe { find(self.pools?.root(), level, self.want.array_size, self.want.format, 0) }
    }
}

/// The node of the key in the subtree at `node`, by a walk of all of it (the map holds a few
/// dozen keys; its order is the game's comparator's, which the walk does not need).
///
/// # Safety
/// `node` is a live node of the pools' map, or none.
unsafe fn find(node: Option<raw::TexturePoolNode>, level: Level, array_size: u32, format: u32, depth: u32) -> Option<raw::TexturePool> {
    let node = node?;
    assert!(depth < 64, "the texture pools' map deeper than a red-black tree of them can be");
    // SAFETY: the caller's.
    unsafe {
        if read(node.width()) == level.width
            && read(node.height()) == level.height
            && read(node.mips()) == level.mips
            && read(node.array_size()) == array_size
            && read(node.format()) == format
            && read(node.usage()) == 0
        {
            return Some(node.pool().expect("a texture pool's node without its pool"));
        }
        find(node.left(), level, array_size, format, depth + 1).or_else(|| find(node.right(), level, array_size, format, depth + 1))
    }
}

/// Whether the C string at `name` starts with `prefix`.
///
/// # Safety
/// `name` is 0 or a live NUL-terminated string.
unsafe fn name_starts(name: *mut u8, prefix: &[u8]) -> bool {
    if name.is_null() {
        return false;
    }
    // SAFETY: the caller's; the walk stops at the string's NUL.
    prefix.iter().enumerate().all(|(i, &byte)| unsafe { name.add(i).read() } == byte)
}

/// A pool's slots in their order.
fn slots(pool: raw::TexturePool) -> impl Iterator<Item = raw::TexturePoolSlot> {
    // SAFETY: a live pool's slot vector, [begin, end).
    let (first, count) = unsafe {
        match (pool.slots(), pool.slots_end()) {
            (Some(first), Some(end)) => (Some(first), end.index_from(first).expect("a texture pool's slots")),
            (None, None) => (None, 0),
            _ => panic!("a texture pool's slots"),
        }
    };
    // SAFETY: below the vector's count.
    (0..count).map(move |i| unsafe { first.expect("counted").at(i) })
}
