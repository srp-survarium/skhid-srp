//! An animation the registry has no id for (patch 95,
//! docs/notes/animation-ids.md).
//!
//! A player's animation tree holds its clips by id: base_player's
//! animation_id_resolver (0x2D7B90) asks the global animations registry
//! (0x2247D00) for the id of a clip pair through its lookup (0x2D6590), a
//! map keyed by the two resources. A pair nobody registered (the world's
//! load registers the slot 7 and 10 weapons of each kit player,
//! player::register_animations 0x2D8100, and the match rules' items; a late
//! join's install the newcomer's kit players)
//! gets 0FFFFh, which the tree keeps; drawing the held weapon then reads the
//! ids' vector at 0FFFFh (0x1F18B0, unchecked) and the game crashes.
//! `animations.id_missing` is at that 0FFFFh: what the lookup was asked for,
//! read once here.

use core::ffi::CStr;

use super::context::Context;
use super::hook::Thread;
use super::ui::c_string;
use super::world::read;
use crate::raw;

/// The longest resource name read (the clips' are about 110 bytes).
const NAME_LIMIT: usize = 160;

/// A clip resource of a pair.
#[derive(Clone, Copy, Debug)]
pub struct Clip<'t> {
    /// 0 for a pair without this resource (a clip that never loaded).
    pub address: u32,
    /// Its name in the resource name registry; None when it has no entry.
    pub name: Option<&'t CStr>,
}

impl Clip<'_> {
    const NONE: Clip<'static> = Clip { address: 0, name: None };
}

/// The registry's state at the miss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registry {
    /// Not yet numbered by actualize (0x2EF280): a lookup before the load's
    /// end.
    Unnumbered,
    Numbered { ids: u32 },
}

/// The lookup's miss: the pair asked for, the registry, and who asked.
pub struct MissingId<'t> {
    pub first: Clip<'t>,
    /// None when the pair is one resource twice (most pairs are).
    pub second: Option<Clip<'t>>,
    pub registry: Option<Registry>,
    /// The resolver's return address (into the boost invoker that called
    /// it); None without a frame.
    pub caller: Option<u32>,
    /// The six stack words above the resolver's arguments, where the
    /// invoker's own return address lies (it keeps no EBP frame): as they
    /// are, for the reader of the log to pick the code addresses from.
    pub above: Option<[u32; 6]>,
    pub thread: &'t Thread,
}

impl<'t> MissingId<'t> {
    /// The miss at `animations.id_missing`; None without the key.
    pub fn at_site(ctx: &'t Context<'_>) -> Option<MissingId<'t>> {
        use raw::site::animations_id_missing as site;
        // SAFETY: the context is this site's: EDI the lookup's key (the
        // resolver's local pair of resource pointers, each holding a
        // reference), ESP+10h its registry argument, EBP the resolver's frame
        // (push ebp / mov ebp,esp at 0x2D7B90); the resources and their name
        // entries live while the key references them.
        unsafe {
            let key = site::key(ctx)?;
            let first = key.first().map_or(Clip::NONE, |first| clip(first));
            let second = key.second().map_or(Clip::NONE, |second| clip(second));
            let second = (second.address != first.address).then_some(second);
            let registry = site::stack(ctx).and_then(|stack| stack.registry()).map(|registry| {
                if read(registry.actualized()) == 0 {
                    Registry::Unnumbered
                } else {
                    let (begin, end) = (read(registry.ids_begin()), read(registry.ids_end()));
                    Registry::Numbered { ids: end.wrapping_sub(begin) / 4 }
                }
            });
            let resolver = site::resolver(ctx);
            let caller = resolver.map(|frame| read(frame.return_address()));
            let above = resolver.map(|frame| {
                [
                    read(frame.above_0()),
                    read(frame.above_1()),
                    read(frame.above_2()),
                    read(frame.above_3()),
                    read(frame.above_4()),
                    read(frame.above_5()),
                ]
            });
            Some(MissingId { first, second, registry, caller, above, thread: ctx.thread() })
        }
    }
}

/// # Safety
/// `resource` is a live resource.
unsafe fn clip<'t>(resource: raw::ManagedResource) -> Clip<'t> {
    // SAFETY: the caller's; a name entry's name is a C string while the
    // resource is registered by name.
    let name = unsafe { resource.name_entry().and_then(|entry| c_string(entry.name(), NAME_LIMIT)) };
    Clip { address: resource.address(), name }
}
