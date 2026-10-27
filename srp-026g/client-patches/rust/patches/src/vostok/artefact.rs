//! The artefacts patch 54 changes, as the game has them:
//! - `artefacts.spring_loader`: a Spring's loader, ESI its config (the
//!   Spring's +340h: character modifier nodes {prev, next, index, value}),
//!   EBX the config's data;
//! - `artefacts.spring_link` / `spring_unlink`: the link (EAX) and unlink
//!   ([esp+4]) of a Spring's active nodes; a node goes into the holder's
//!   list of its index (the holder's profile +6B8h, its kit +115h x 704h,
//!   +204h + index x 14h: {count, head, tail, ?, sorted});
//! - `artefacts.lifebone`: Lifebone's enable_active, EDI the Lifebone,
//!   [esp+14h] the time; its holder's damage model's pools;
//! - `artefacts.action`: artefact_base::action at its start, ECX the
//!   artefact, [esp+4] pressed, [esp+8] the time; the inventory's artefact
//!   slots 13..18;
//! - `artefacts.stagger`: the player's stagger, ESI the player, XMM0 and
//!   [esp+14h] the recent hits' stopping power summed;
//! - `artefacts.onyx_hit`: Onyx's hit callback after its hit type test,
//!   ECX the Onyx;
//! - `artefacts.onyx_hint`: Onyx's client update at its activation hint, EBX
//!   the Onyx's client part (its +F20h), [esp] the hint's time on screen.
//!
//! What those registers and fields hold is said here, once.

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;

use super::config::ConfigKey;
use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, read, vtable, write};
use crate::raw;

/// The inventory's artefact slots.
const SLOTS: core::ops::RangeInclusive<usize> = 13..=18;
/// An artefact's states: its passive, its active.
pub const PASSIVE: u32 = 3;
pub const ACTIVE: u32 = 4;

/// A Spring's config at `artefacts.spring_loader`.
pub struct SpringLoad<'t> {
    config: Object,
    data: Object,
    thread: PhantomData<&'t Thread>,
}

/// A section of an artefact's config.
#[derive(Clone, Copy)]
pub enum Section {
    Active,
    Passive,
}

impl<'t> SpringLoad<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<SpringLoad<'t>> {
        Some(SpringLoad { config: Object::at(ctx.reg(Reg::Esi))?, data: Object::at(ctx.reg(Reg::Ebx))?,
                          thread: PhantomData })
    }

    /// data/<section>/<key> as an f32's bits (an integer made a float, as
    /// the loader does), through the config's own lookup (operator[]).
    pub fn number(&self, section: Section, key: &'static CStr) -> u32 {
        let section = match section {
            Section::Active => ConfigKey::exe(raw::image::ACTIVE_KEY),
            Section::Passive => ConfigKey::exe(raw::image::PASSIVE_KEY),
        };
        // SAFETY: the loader's data (a config node), the lookups as the
        // loader makes its own (no test first: the release's configs have
        // the keys); a value node's dword and type word.
        unsafe {
            let node = raw::engine_cfg_child(self.data.ptr().cast(), section.raw());
            let value = raw::engine_cfg_child(node, ConfigKey::ours(key).raw()).cast_mut().cast::<c_void>();
            let bits = read(raw::binary_config_value::value(value));
            if read(raw::binary_config_value::kind(value)) == 2 { bits } else { (bits as i32 as f32).to_bits() }
        }
    }

    /// The passive's second node made modifier `index` with `value`.
    pub fn set_passive_second(&self, index: u32, value: u32) {
        // SAFETY: the config's fields.
        unsafe {
            write(raw::spring_config::passive_weight_index(self.config.ptr()), index);
            write(raw::spring_config::passive_weight_value(self.config.ptr()).cast(), value);
        }
    }

    /// One of patch 54's two active nodes made {0, 0, index, value}.
    pub fn set_node(&self, node: SpringNode, index: u32, value: u32) {
        let at = match node {
            // SAFETY: the config's (the Spring's) field addresses.
            SpringNode::Aim => unsafe { raw::spring_config::aim_node(self.config.ptr()) },
            SpringNode::Stopping => unsafe { raw::spring_config::stop_node(self.config.ptr()) },
        };
        // SAFETY: the node's four dwords (the Spring has room: its size grown).
        unsafe { write_node(at.cast(), [0, 0, index, value]) }
    }
}

/// Patch 54's two active nodes of a Spring.
#[derive(Clone, Copy)]
pub enum SpringNode {
    Aim,
    Stopping,
}

/// # Safety
/// `at` is a modifier node's four dwords.
unsafe fn write_node(at: *mut c_void, words: [u32; 4]) {
    // SAFETY: the caller's.
    unsafe {
        write(raw::modifier_node::prev(at), words[0]);
        write(raw::modifier_node::next(at), words[1]);
        write(raw::modifier_node::index(at), words[2]);
        write(raw::modifier_node::value(at).cast(), words[3]);
    }
}

/// An artefact.
#[derive(Clone, Copy)]
pub struct Artefact<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Artefact<'t> {
    fn new(this: Object) -> Artefact<'t> {
        Artefact { this, thread: PhantomData }
    }

    /// The artefact a site's register names.
    pub fn in_reg(ctx: &'t Context<'_>, reg: Reg) -> Option<Artefact<'t>> {
        Object::at(ctx.reg(reg)).map(Artefact::new)
    }

    /// The artefact a site's stack slot names.
    pub fn on_stack(ctx: &'t Context<'_>, offset: u32) -> Option<Artefact<'t>> {
        Object::at(ctx.stack(offset)).map(Artefact::new)
    }

    pub fn same(&self, other: &Artefact<'_>) -> bool {
        self.this == other.this
    }

    /// Of the same class (its vtable).
    pub fn same_class(&self, other: &Artefact<'_>) -> bool {
        // SAFETY: live artefacts' vtables, compared.
        unsafe { vtable(self.this) == vtable(other.this) }
    }

    fn field(&self, at: unsafe fn(*mut c_void) -> *mut u32) -> u32 {
        // SAFETY: an artefact's field.
        unsafe { read(at(self.this.ptr())) }
    }

    fn set_field(&self, at: unsafe fn(*mut c_void) -> *mut u32, value: u32) {
        // SAFETY: as field.
        unsafe { write(at(self.this.ptr()), value) }
    }

    pub fn state(&self) -> u32 {
        self.field(raw::artefact::state)
    }

    /// Its kind (1: one its key fires).
    pub fn kind(&self) -> u32 {
        self.field(raw::artefact::kind)
    }

    /// Ready: in its passive with no cooldown.
    pub fn ready(&self) -> bool {
        self.state() == PASSIVE && self.field(raw::artefact::cooldown) == 0
    }

    pub fn activated(&self) -> u32 {
        self.field(raw::artefact::activated)
    }

    /// Activated as artefact_base::action activates one (0x31331C..):
    /// its enable_active, its effect, the time, its active state.
    pub fn activate(&self, time: u32) {
        // SAFETY: the artefact's own enable_active (vtable+80h) and the
        // game's activation effect, as action calls them.
        unsafe {
            raw::engine_enable_active(self.this.ptr(), time);
            raw::engine_artefact_effect(self.this.ptr(), time);
        }
        self.set_field(raw::artefact::activated, time);
        self.set_field(raw::artefact::state, ACTIVE);
    }

    /// An Onyx (its enable_active is artefact_onyx_core's).
    pub fn is_onyx(&self) -> bool {
        // SAFETY: the vtable's enable_active slot (+80h).
        let slot = unsafe { read(raw::artefact_vtable::enable_active(vtable(self.this) as usize as *mut c_void)) };
        slot == raw::image::ONYX_ENABLE_ACTIVE
    }

    /// An Onyx's active time made longer by `ms`.
    pub fn add_onyx_time(&self, ms: u32) {
        let left = self.field(raw::artefact::onyx_time_left);
        self.set_field(raw::artefact::onyx_time_left, left.wrapping_add(ms));
    }

    /// The inventory it is in.
    pub fn inventory(&self) -> Option<Inventory<'t>> {
        // SAFETY: the artefact's inventory or 0.
        unsafe { pointer(raw::artefact::inventory(self.this.ptr())) }.map(|this| Inventory { this, thread: PhantomData })
    }

    /// A Spring's patch 54 node's address.
    pub fn spring_node(&self, node: SpringNode) -> Node<'t> {
        // SAFETY: the Spring's field addresses (its size grown for them).
        let at = unsafe {
            match node {
                SpringNode::Aim => raw::artefact::spring_aim_node(self.this.ptr()),
                SpringNode::Stopping => raw::artefact::spring_stop_node(self.this.ptr()),
            }
        };
        Node { at: at.cast(), thread: PhantomData }
    }
}

/// An inventory.
#[derive(Clone, Copy)]
pub struct Inventory<'t> {
    this: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Inventory<'t> {
    /// The artefacts of its artefact slots (13..18), each slot's item or
    /// none, in order.
    pub fn artefacts(&self) -> impl Iterator<Item = Option<Artefact<'t>>> + 't {
        let this = self.this;
        SLOTS.map(move |slot| {
            // SAFETY: the inventory's slots, each an item or 0.
            unsafe { pointer(raw::inventory::slots(this.ptr()).add(slot)) }.map(Artefact::new)
        })
    }

    fn holder(&self) -> Option<*mut c_void> {
        // SAFETY: the inventory's holder interface (the player's +108h).
        unsafe { pointer(raw::inventory::holder(self.this.ptr())) }.map(|holder| holder.ptr())
    }

    /// The holder's kit's modifier lists (its profile, its kit).
    pub fn lists(&self) -> Option<ModifierKit<'t>> {
        let holder = self.holder()?;
        // SAFETY: the interface's own get_player (vtable+0Ch); the player's
        // profile and kit, the kit's block.
        unsafe {
            let player = Object::at(raw::engine_holder_player(holder) as usize as u32)?;
            ModifierKit::of(player)
        }
    }

    /// The holder's damage model's pools (its owner's +108h).
    pub fn pools(&self) -> Option<Pools<'t>> {
        let holder = self.holder()?;
        // SAFETY: the interface's own slot (&the damage model), the damage
        // model's parts field's address.
        unsafe {
            let model = pointer(raw::engine_damage_model_ptr(holder).cast())?;
            Some(Pools { at: raw::damage_model::parts(model.ptr()).cast(), thread: PhantomData })
        }
    }
}

/// A kit's modifiers block of a player's profile.
#[derive(Clone, Copy)]
pub struct ModifierKit<'t> {
    kit: *mut c_void,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ModifierKit<'t> {
    /// # Safety
    /// `player` is a live player.
    unsafe fn of(player: Object) -> Option<ModifierKit<'t>> {
        // SAFETY: the caller's; the profile pointer and the kit byte.
        unsafe {
            let profile = pointer(raw::base_player::modifiers_profile(player.ptr()))?;
            let kit = usize::from(read(raw::base_player::kit(player.ptr())));
            let stride = raw::modifiers_kit::next(profile.ptr()) as usize - profile.ptr() as usize;
            Some(ModifierKit { kit: profile.ptr().wrapping_byte_add(stride * kit), thread: PhantomData })
        }
    }

    /// The kit of `player` (the stagger's ESI).
    pub fn of_player(ctx: &'t Context<'_>) -> Option<ModifierKit<'t>> {
        // SAFETY: ESI at the stagger's site is the player.
        unsafe { ModifierKit::of(Object::at(ctx.reg(Reg::Esi))?) }
    }

    fn list(&self, index: u32) -> *mut c_void {
        // SAFETY: the kit's lists, 14h each.
        unsafe {
            let first = raw::modifiers_kit::lists(self.kit).cast::<c_void>();
            let stride = raw::modifier_list::next(first) as usize - first as usize;
            first.wrapping_byte_add(stride * index as usize)
        }
    }

    /// `node` last in the list of its index, as 0x311BC0 links its own: the
    /// count up, the tail's next, the list unsorted when the tail's value
    /// is above the node's.
    pub fn link(&self, node: &Node<'_>) {
        // SAFETY: a node of ours (a Spring's) and the kit's list of its
        // index, written field by field as the game's link writes them.
        unsafe {
            let list = self.list(read(raw::modifier_node::index(node.at)));
            let count = raw::modifier_list::count(list);
            write(count, read(count).wrapping_add(1));
            let tail = read(raw::modifier_list::tail(list));
            write(raw::modifier_node::next(node.at), 0);
            write(raw::modifier_node::prev(node.at), tail);
            if read(raw::modifier_list::head(list)) == 0 {
                write(raw::modifier_list::head(list), node.at as usize as u32);
            } else {
                write(raw::modifier_node::next(tail as usize as *mut c_void), node.at as usize as u32);
            }
            write(raw::modifier_list::tail(list), node.at as usize as u32);
            if tail != 0 {
                let before = read(raw::modifier_node::value(tail as usize as *mut c_void));
                if before > read(raw::modifier_node::value(node.at)) {
                    write(raw::modifier_list::sorted(list), 0);
                }
            }
        }
    }

    /// `node` out of the list of its index (the game's own unlink).
    pub fn unlink(&self, node: &Node<'_>) {
        // SAFETY: the game's unlink (ESI the list, EAX the node).
        unsafe {
            let list = self.list(read(raw::modifier_node::index(node.at)));
            raw::engine_modifier_unlink(list, node.at);
        }
    }

    /// The modifier `index` applied to `value` with weight `weight`
    /// (apply_modifier).
    pub fn apply(&self, index: u32, value: f32, weight: f32) -> f32 {
        // SAFETY: the game's getter on the kit's 68 modifiers.
        unsafe { raw::engine_apply_modifier(raw::modifiers_kit::modifiers(self.kit).cast(), index, value, weight) }
    }
}

/// A modifier node of ours (a Spring's patch 54 nodes).
pub struct Node<'t> {
    at: *mut c_void,
    thread: PhantomData<&'t Thread>,
}

/// A damage model's pools.
pub struct Pools<'t> {
    at: *mut c_void,
    thread: PhantomData<&'t Thread>,
}

impl Pools<'_> {
    /// The pool `name` filled to its maximum, as a medkit heals one
    /// (increase_health with the difference); nothing when the model has
    /// no such pool or it is full.
    pub fn fill(&self, name: &'static CStr, time: u32) {
        let name = name.as_ptr();
        // SAFETY: the game's pool lookup on a damage model's pools, the
        // pool's two floats, the game's increase_health.
        unsafe {
            let Some(pool) = Object::at(raw::engine_find_pool(self.at, (&raw const name).cast()) as usize as u32)
            else {
                return;
            };
            let missing = read(raw::body_part::health_max(pool.ptr())) - read(raw::body_part::health(pool.ptr()));
            if missing > 0.0 {
                raw::engine_increase_health(pool.ptr(), time, missing);
            }
        }
    }
}

/// The Onyx at `artefacts.onyx_hint` (EBX its client part).
pub fn hint_onyx<'t>(ctx: &'t Context<'_>) -> Option<Artefact<'t>> {
    let part = Object::at(ctx.reg(Reg::Ebx))?;
    // SAFETY: the Onyx's address by its client part's place in it.
    Object::at(unsafe { raw::onyx_client::onyx(part.ptr()) } as usize as u32).map(Artefact::new)
}

/// The hint's time on screen: the Onyx's active time left, ms.
pub fn hint_time(onyx: &Artefact<'_>) -> u32 {
    onyx.field(raw::artefact::onyx_time_left)
}

const _: () = assert!(size_of::<*mut c_void>() == 4);
