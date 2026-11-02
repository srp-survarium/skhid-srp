//! Native batteries. Spatial selection is local; a replicated command
//! enters the ordinary use lifecycle by id, with a canonical use point.
use super::{Context, Thread};
use super::world::{BodyId, Player, WorldKey, read, write_floats};
use crate::raw;
use weapon_pickup::battery::{Id, COUNT};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Object(u32);
#[derive(Clone, Copy)]
pub struct Free { pub object: Object, pub body: Option<BodyId>, pub transform: [f32; 16] }
pub struct Frame { pub world: WorldKey, pub time: weapon_pickup::Ms, pub items: [Option<Free>; COUNT] }

pub struct Battery<'t> { this: raw::VictoryItem, _thread: &'t Thread }
impl<'t> Battery<'t> {
    fn from_item(item: raw::Item, thread: &'t Thread) -> Option<Self> {
        // SAFETY: item belongs to the live rule; only the exact client
        // battery vtable allows the client-specific typed view.
        unsafe { (read(item.vtable()) == raw::image::VICTORY_ITEM_VTABLE)
            .then(|| Self { this: item.victory_item(), _thread: thread }) }
    }
    pub fn free(&self) -> Option<Free> {
        // SAFETY: battery borrowed from its live world's rule for this hook.
        unsafe {
            if self.this.user().is_some() || read(self.this.container()) != 0 || read(self.this.physical()) == 0 {
                return None;
            }
            let mut transform = [0.0; 16];
            raw::engine_battery_transform(Some(self.this), transform.as_mut_ptr());
            transform.iter().all(|f| f.is_finite()).then_some(Free { object: Object(self.this.address()), body: self.this.body().map(|b| BodyId(b.address())), transform })
        }
    }
    pub fn can_take(&self, player: &Player<'_>) -> bool {
        if !player.alive() || !player.inserted() || self.free().is_none() { return false; }
        // SAFETY: the live player owns its native use-data and inventory;
        // the engine's own can_use checks carrying and action restrictions.
        unsafe {
            player.this.held_item().is_some() && read(player.this.battery_use_data().object()) == 0
                && read(self.this.usable().users()) == 0
                && raw::engine_battery_can_use(Some(self.this.usable()), Some(player.this.battery_use_data())) != 0
        }
    }
    pub fn take(&self, player: &Player<'_>) {
        if !self.can_take(player) { return; }
        let eye = player.eye_transform();
        let transform = player.transform();
        // SAFETY: simulation input application on the game thread, exactly
        // where native apply_new_input fills this same record and calls use.
        // The point is canonical, independent of this client's loose model.
        unsafe {
            let user = player.this.battery_use_data();
            write_floats(user.eye_offset(), [eye[12]-transform[12], eye[13]-transform[13], eye[14]-transform[14]]);
            write_floats(user.point(), [eye[12], eye[13], eye[14]]);
            raw::engine_battery_use(Some(self.this.usable()), Some(user), read(player.this.time()));
        }
    }
}

fn at<'t>(core: raw::CoreWorld, id: Id, thread: &'t Thread) -> Option<Battery<'t>> {
    // SAFETY: core is borrowed from a live player/world. The statistics'
    // rule owns its vector; index is bounded before the slot is read.
    unsafe {
        let rule = core.statistics()?.shared().victory_items().rule()?;
        let first = rule.items_begin()?;
        let count = rule.items_end()?.index_from(first)?.min(COUNT);
        if id.index() >= count { return None; }
        Battery::from_item(first.at(id.index()).item()?, thread)
    }
}

pub fn for_player<'t>(player: &Player<'t>, id: Id) -> Option<Battery<'t>> {
    // SAFETY: player's live simulation core.
    at(unsafe { player.this.world_core() }?, id, player.thread)
}

pub fn frame(ctx: &Context<'_>) -> Option<Frame> {
    // SAFETY: after_move site owns this game world's core. This works
    // without a network client too (replays).
    let core = unsafe { raw::site::game_world_after_move::world(ctx)?.pvp_match()?.core() };
    Some(Frame { world: WorldKey::of(core), time: weapon_pickup::Ms(unsafe { read(core.time()) }),
        items: core::array::from_fn(|i| at(core, Id::new(i)?, ctx.thread())?.free()) })
}

#[derive(Clone, Copy)]
pub enum Pass { Primary, Secondary }

pub fn draw<'t>(ctx: &'t Context<'_>, pass: Pass) -> Option<Draw<'t>> {
    // SAFETY: these two sites carry the live battery and its stack-owned
    // matrix pair in the registers named by sites_026g.tsv.
    unsafe {
        let (item, frame) = match pass {
            Pass::Primary => (raw::site::battery_draw_primary::item(ctx)?, raw::site::battery_draw_primary::frame(ctx)?),
            Pass::Secondary => (raw::site::battery_draw_secondary::item(ctx)?, raw::site::battery_draw_secondary::frame(ctx)?),
        };
        if item.user().is_some() || read(item.container()) != 0 || read(item.physical()) == 0 { return None; }
        Some(Draw { item: Object(item.address()), frame, _thread: core::marker::PhantomData })
    }
}

pub struct Draw<'t> { pub item: Object, frame: raw::BatteryModelFrame, _thread: core::marker::PhantomData<&'t Thread> }
impl Draw<'_> {
    pub fn pose(&mut self, matrix: [f32; 16]) {
        // SAFETY: stack matrices borrowed for this draw; engine_update_model
        // copies them into the render command. No simulation field is written.
        unsafe {
            write_floats(self.frame.first().value(), matrix);
            write_floats(self.frame.second().value(), matrix);
        }
    }
}

pub fn native_target_is_battery(player: &Player<'_>) -> bool {
    // SAFETY: ordinary read-only local use scan, with its optional point
    // omitted, followed by exact type discrimination of the returned object.
    unsafe { raw::engine_detect_usable(Some(player.this), core::ptr::null_mut())
        .is_some_and(|usable| read(usable.vtable()) == raw::image::BATTERY_USABLE_VTABLE) }
}
