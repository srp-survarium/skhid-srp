//! The match's batteries (the client's `victory_item`s, the battery rule's
//! items) as patch 76 reads them: which lie loose and where the simulation
//! has them, the two matrices their draw gives the renderer, and a pickup
//! command's entry into the game's own use.
//!
//! Written: the draw's own stack copies of a battery's transform
//! ([`Draw::pose`]), and, for a pickup command, what the game's Use press
//! writes itself (the player's use data and the usable's users:
//! [`Battery::take`]). Nothing else.

use core::marker::PhantomData;

use weapon_pickup::BATTERIES;

use super::world::{BodyId, GameWorld, Player, WorldKey, floats, read, write_floats};
use super::{Context, Thread};
use crate::raw;

/// A battery's identity while its match lasts (its address as a value,
/// never read).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryId(u32);

/// A battery lying loose in the world, as the simulation has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loose {
    pub id: BatteryId,
    /// Its body in the physics world (what a level ray meets of it).
    pub body: Option<BodyId>,
    /// Its model's transform (get_transform's: the matrix its draw gives).
    pub transform: [f32; 16],
}

pub struct Battery<'t> {
    this: raw::VictoryItem,
    thread: PhantomData<&'t Thread>,
}

/// A loose battery's facts: carried by nobody, in no container, in the
/// world as a body.
///
/// # Safety
/// `item` is a live battery, on the game's thread.
unsafe fn loose(item: raw::VictoryItem) -> Option<Loose> {
    // SAFETY: the caller's.
    unsafe {
        if item.user().is_some() || read(item.container()) != 0 || read(item.physical()) == 0 {
            return None;
        }
        let mut transform = [0f32; 16];
        raw::engine_usable_transform(Some(item.usable()), transform.as_mut_ptr());
        let finite = transform.iter().all(|f| f.is_finite());
        finite.then_some(Loose { id: BatteryId(item.address()), body: item.body().map(|b| BodyId(b.address())), transform })
    }
}

/// The battery `battery` of the world `core`: the rule's item at that index,
/// if it is the client's battery.
fn at<'t>(core: raw::CoreWorld, battery: weapon_pickup::Battery) -> Option<Battery<'t>> {
    // SAFETY: a live world's core; the statistics' rule owns its vector, and
    // the index is under its count before the slot is read. Only the
    // client battery's own vtable gives the battery's view.
    unsafe {
        let rule = core.statistics()?.shared().victory_items().rule()?;
        let first = rule.items_begin()?;
        let count = rule.items_end()?.index_from(first)?;
        let index = usize::from(battery.get());
        if index >= count {
            return None;
        }
        let item = first.at(index).item()?;
        (read(item.vtable()) == raw::image::VICTORY_ITEM_VTABLE)
            .then(|| Battery { this: item.victory_item(), thread: PhantomData })
    }
}

impl<'t> Battery<'t> {
    /// The battery of `player`'s world.
    pub fn of(player: &Player<'t>, battery: weapon_pickup::Battery) -> Option<Battery<'t>> {
        // SAFETY: the live player's world.
        at(unsafe { player.this.world_core() }?, battery)
    }

    /// It as it lies loose, None while it is carried, in a container or out
    /// of the world.
    pub fn loose(&self) -> Option<Loose> {
        // SAFETY: a live battery, on the game's thread.
        unsafe { loose(self.this) }
    }

    /// Whether the game lets `player` start taking it now: what its own Use
    /// press asks (the player alive, using nothing, its can_use: no battery
    /// carried, no broken limb, its held item ready), and what the engine
    /// leaves to the scan that found it: the battery loose, and nobody
    /// taking it yet (use_initialize refuses no second user).
    pub fn free_for(&self, player: &Player<'_>) -> bool {
        if !player.alive() || !player.inserted() || self.loose().is_none() {
            return false;
        }
        // SAFETY: the live player's use data and held item (can_use calls
        // the held item's vtable: there must be one), the battery's usable.
        unsafe {
            let user = player.this.use_user();
            player.this.held_item().is_some()
                && read(user.object()) == 0
                && read(self.this.usable().users()) == 0
                && raw::engine_can_use(Some(self.this.usable()), Some(user)) != 0
        }
    }

    /// `player` starts taking it, as the game's Use press does
    /// (apply_new_input 0x2D891D..0x2D898F): the use's point and the eye's
    /// offset saved in the player's use data, then the usable's use
    /// (use_initialize 0x3007E0). The point is the player's own eye in the
    /// simulation, the same in every world, so use_tick (0x300830) holds the
    /// player within 2 m of where it pressed. The caller has asked
    /// [`Battery::free_for`] in this hook.
    pub fn take(&self, player: &Player<'_>) {
        let eye = player.eye_transform();
        let place = player.transform();
        // SAFETY: where an input is applied, on the game's thread, the
        // writes apply_new_input makes itself there.
        unsafe {
            let user = player.this.use_user();
            write_floats(user.eye_offset(), [eye[12] - place[12], eye[13] - place[13], eye[14] - place[14]]);
            write_floats(user.point(), [eye[12], eye[13], eye[14]]);
            raw::engine_battery_use(Some(self.this.usable()), Some(user), read(player.this.time()));
        }
    }
}

/// The world's batteries at a step.
pub struct Batteries {
    pub world: WorldKey,
    /// Each battery of the rule, loose or not.
    pub loose: [Option<Loose>; BATTERIES as usize],
}

/// The batteries of the world ticked at `game_world.after_move` (with no
/// network client too: a replay's).
pub fn at_after_move(world: &GameWorld<'_>) -> Option<Batteries> {
    // SAFETY: the site's game world, its match and core, alive for the hook.
    let core = unsafe { world.this.pvp_match()?.core() };
    let loose = core::array::from_fn(|index| {
        let battery = weapon_pickup::Battery::new(index as u8)?;
        at(core, battery)?.loose()
    });
    Some(Batteries { world: WorldKey::of(core), loose })
}

/// Which of a battery's two models a draw is of.
#[derive(Clone, Copy)]
pub enum Pass {
    Primary,
    Secondary,
}

/// A loose battery's draw, before its matrices go to the renderer.
pub struct Draw<'t> {
    pub battery: BatteryId,
    frame: raw::BatteryModelFrame,
    thread: PhantomData<&'t Thread>,
}

/// The draw at `battery.draw_primary` or `battery.draw_secondary`, of a
/// loose battery.
pub fn draw<'t>(ctx: &'t Context<'_>, pass: Pass) -> Option<Draw<'t>> {
    // SAFETY: the context is that site's: its battery in the register
    // sites_026g.tsv names, its two matrices on the stack.
    unsafe {
        let (item, frame) = match pass {
            Pass::Primary => (raw::site::battery_draw_primary::item(ctx)?, raw::site::battery_draw_primary::frame(ctx)?),
            Pass::Secondary => {
                (raw::site::battery_draw_secondary::item(ctx)?, raw::site::battery_draw_secondary::frame(ctx)?)
            }
        };
        let loose = loose(item)?;
        Some(Draw { battery: loose.id, frame, thread: PhantomData })
    }
}

impl Draw<'_> {
    /// Draws the model at `matrix` instead.
    pub fn pose(&mut self, matrix: [f32; 16]) {
        // SAFETY: the draw's own stack copies, which update_model (0x22280)
        // copies next and nothing else reads.
        unsafe {
            write_floats(self.frame.first().value(), matrix);
            write_floats(self.frame.second().value(), matrix);
        }
    }

    /// The transform the game draws it at.
    pub fn transform(&self) -> [f32; 16] {
        // SAFETY: as pose.
        unsafe { floats(self.frame.first().value()) }
    }
}
