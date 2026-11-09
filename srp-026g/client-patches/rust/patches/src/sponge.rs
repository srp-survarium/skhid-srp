//! Patch 59: the Sponge (artefact type 12, items_dict 758), 0.28d0's thrown
//! toxic artefact on 0.26g, thrown and exploding as retail 0.69d0's
//! (docs/notes/sponge.md; client_patch.md section 3.52).
//!
//! The rules are the sponge crate's: what a sponge is and its one
//! transition (`sponge::machine`), its record in a world (`sponge::record`,
//! which the world check parses too), where it spawns (`sponge::spots`). The
//! object and what the game does for it are `vostok::sponge`. This module is
//! the hooks (`sponge.toml`): each says which way the game goes on.
//!
//! - The artefact cook: data/type 12 is constructed, sized and queried as a
//!   sponge (its model, spawn particle and sound, and a damage zone).
//! - The throw-grenade state with a sponge in the grenade slot: its key puts
//!   it there; the state's predicate, pull, ready, throw, save and load ask
//!   the sponge instead of a grenade set.
//! - Its object's slots (Rattle's vtable with ten of ours): its action,
//!   tick, passive, inventory changes, world, serialization, destructor;
//!   its drawable's draw; its hit inflictor's.
//! - The damage zone's hits: a zone a sponge ticks names its thrower (kill
//!   credit), its inflictor and it.
//! - The owner's spots: the anomaly cook's kinds of a core, the core's
//!   links, spawn_artefact's pick.

#![forbid(unsafe_code)]

use crate::vostok::sponge::{self, Sponge, SpongeCallee, SpongeDrawable, SpongeInflictor, SpongeLoad, SpongeReader, SpongeWriter};
use crate::vostok::{
    Context, Flow, NoExit, OrigArtefactAction, OrigArtefactAdded, OrigArtefactDeserialize, OrigArtefactRemoved,
    OrigArtefactSerialize, OrigArtefactTick, OrigInflictorIsGrenade, OrigInflictorPosition, OrigNothing,
    OrigRattleDestroy, OrigRattleDraw, OrigRattleSetWorld, SpongeOnConstructExit, SpongeOnPredicateExit,
    SpongeOnPullExit, SpongeOnQueryExit, SpongeOnReadyExit, SpongeOnSaveExit, SpongeOnSizeExit, SpongeOnLoadExit,
    SpongeOnSpawnExit, SpongeOnThrowExit,
};

fn done<E>(handled: bool, exit: E) -> Flow<E> {
    if handled { Flow::Exit(exit) } else { Flow::Continue }
}

// ---- the artefact cook ---------------------------------------------------

pub fn on_construct(ctx: &mut Context<'_>) -> Flow<SpongeOnConstructExit> {
    done(sponge::construct(ctx), SpongeOnConstructExit::Done)
}

pub fn on_size(ctx: &mut Context<'_>) -> Flow<SpongeOnSizeExit> {
    done(sponge::size(ctx), SpongeOnSizeExit::Done)
}

pub fn on_query(ctx: &mut Context<'_>) -> Flow<SpongeOnQueryExit> {
    done(sponge::query(ctx), SpongeOnQueryExit::Done)
}

pub fn on_loaded(sponge: &mut SpongeCallee<'_>, results: &mut SpongeLoad) {
    sponge::loaded(sponge, results);
}

// ---- the throw-grenade state ---------------------------------------------

pub fn on_predicate(ctx: &mut Context<'_>) -> Flow<SpongeOnPredicateExit> {
    done(sponge::predicate(ctx), SpongeOnPredicateExit::On)
}

pub fn on_ready(ctx: &mut Context<'_>) -> Flow<SpongeOnReadyExit> {
    match sponge::ready(ctx) {
        Some(true) => Flow::Exit(SpongeOnReadyExit::Yes),
        Some(false) => Flow::Exit(SpongeOnReadyExit::No),
        None => Flow::Continue,
    }
}

pub fn on_throw(ctx: &mut Context<'_>) -> Flow<SpongeOnThrowExit> {
    done(sponge::throw(ctx), SpongeOnThrowExit::On)
}

pub fn on_pull(ctx: &mut Context<'_>) -> Flow<SpongeOnPullExit> {
    done(sponge::pull(ctx), SpongeOnPullExit::On)
}

pub fn on_save(ctx: &mut Context<'_>) -> Flow<SpongeOnSaveExit> {
    done(sponge::save(ctx), SpongeOnSaveExit::On)
}

pub fn on_load(ctx: &mut Context<'_>) -> Flow<SpongeOnLoadExit> {
    done(sponge::load(ctx), SpongeOnLoadExit::On)
}

// ---- the damage zone's hits ----------------------------------------------

pub fn on_zone_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sponge::zone_hit(ctx);
    Flow::Continue
}

pub fn on_zone_hit_done(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sponge::zone_hit_done(ctx);
    Flow::Continue
}

// ---- the owner's spots ---------------------------------------------------

pub fn on_artefacts(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sponge::artefacts(ctx);
    Flow::Continue
}

pub fn on_links(ctx: &mut Context<'_>) -> Flow<NoExit> {
    sponge::links(ctx);
    Flow::Continue
}

pub fn on_spawn(ctx: &mut Context<'_>) -> Flow<SpongeOnSpawnExit> {
    if sponge::spawn(ctx) { Flow::Exit(SpongeOnSpawnExit::Take) } else { Flow::Exit(SpongeOnSpawnExit::Nothing) }
}

// ---- the object's slots --------------------------------------------------

pub fn on_destroy(_: &OrigRattleDestroy, sponge: &mut Sponge<'_>, flags: u32) -> u32 {
    sponge.destroy(flags)
}

pub fn on_action(_: &OrigArtefactAction, sponge: &mut Sponge<'_>, pressed: u32, _time: u32) {
    sponge.action(pressed & 0xFF != 0);
}

pub fn on_removed(_: &OrigArtefactRemoved, sponge: &mut Sponge<'_>) {
    sponge.removed();
}

pub fn on_revived(_: &OrigArtefactAdded, sponge: &mut Sponge<'_>) {
    sponge.revived();
}

pub fn on_serialize(_: &OrigArtefactSerialize, sponge: &mut Sponge<'_>, writer: &mut SpongeWriter, dumper: u32) {
    sponge.serialize(writer, dumper);
}

pub fn on_deserialize(_: &OrigArtefactDeserialize, sponge: &mut Sponge<'_>, reader: &mut SpongeReader, a: u32, b: u32) {
    sponge.deserialize(reader, a, b);
}

pub fn on_set_world(_: &OrigRattleSetWorld, sponge: &mut Sponge<'_>, world: u32) {
    sponge.set_world(world);
}

pub fn on_tick(_: &OrigArtefactTick, sponge: &mut Sponge<'_>, dt: u32, time: u32) {
    sponge.tick(dt, time);
}

pub fn on_enable_passive(_: &OrigNothing, sponge: &mut Sponge<'_>) {
    sponge.enable_passive();
}

pub fn on_disable_passive(_: &OrigNothing, sponge: &mut Sponge<'_>) {
    sponge.disable_passive();
}

pub fn on_draw(_: &OrigRattleDraw, drawable: &mut SpongeDrawable<'_>) {
    drawable.draw();
}

/// 0.28's hit inflictor: not a grenade (the splash's and the zone's hits
/// are the Sponge's own).
pub fn on_is_grenade(_: &OrigInflictorIsGrenade, _: &mut SpongeInflictor) -> u32 {
    0
}

pub fn on_position(_: &OrigInflictorPosition, inflictor: &mut SpongeInflictor) -> u32 {
    inflictor.position()
}

/// A contact of its body, through the physics world's boost invoker.
pub fn on_contact(sponge: &mut SpongeCallee<'_>, _point: u32) {
    if let Some(sponge) = sponge.sponge() {
        sponge.contact();
    }
}
