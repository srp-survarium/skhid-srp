//! Patch 180: Dense Smoke (402) and Toxic Smoke (441), the thrower's smoke cloud
//! (docs/notes/technical-skills.md, "Dense Smoke and Toxic Smoke"). The cloud's numbers are the
//! `technical-skills` crate's (`smoke`), which the match server's bots share.
#![forbid(unsafe_code)]

use crate::vostok::{
    Context, Flow, NoExit, TechnicalSmokeOnAnomalyZonesExit, TechnicalSmokeOnCookQueryExit, technical_smoke,
};

/// The client grenade's allocation, with the tail the two skills keep on it.
pub fn on_grenade_size(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::grenade_size(ctx);
    Flow::Skip
}

/// A new grenade: no cloud, no green instance.
pub fn on_grenade_construct(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::grenade_construct(ctx);
    Flow::Continue
}

/// A grenade's end: its holds ended, its green instance released.
pub fn on_grenade_destroy(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::grenade_destroy(ctx);
    Flow::Continue
}

/// The cook's query: a smoke grenade also asks the green copy of its particle.
pub fn on_cook_query(ctx: &mut Context<'_>) -> Flow<TechnicalSmokeOnCookQueryExit> {
    match technical_smoke::cook_query(ctx) {
        true => Flow::Exit(TechnicalSmokeOnCookQueryExit::Done),
        false => Flow::Continue,
    }
}

/// The cook's results: the green instance kept.
pub fn on_cook_ready(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::cook_ready(ctx);
    Flow::Continue
}

/// Toxic Smoke: a blast keeps or clears the grenade's toxic cloud (the simulation).
pub fn on_explode(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::explode(ctx);
    Flow::Continue
}

/// Toxic Smoke's hits, each player's tick.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::toxic_tick(ctx);
    Flow::Continue
}

/// Forest Protection: a toxic cloud counts as a damage zone for a hit with an owner.
pub fn on_anomaly_zones(ctx: &mut Context<'_>) -> Flow<TechnicalSmokeOnAnomalyZonesExit> {
    match technical_smoke::anomaly_zones(ctx) {
        true => Flow::Exit(TechnicalSmokeOnAnomalyZonesExit::Inside),
        false => Flow::Continue,
    }
}

/// The cloud saved with its grenade: the state's bit, the bytes, and their load.
pub fn on_save_state(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::save_state(ctx);
    Flow::Continue
}
pub fn on_save_cloud(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::save_cloud(ctx);
    Flow::Continue
}
pub fn on_load_cloud(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::load_cloud(ctx);
    Flow::Continue
}

/// The blast's particle: green for Toxic Smoke, held for Dense Smoke.
pub fn on_play(ctx: &mut Context<'_>) -> Flow<NoExit> {
    match technical_smoke::play(ctx) {
        true => Flow::Skip,
        false => Flow::Continue,
    }
}

/// Dense Smoke's hold, each tick of a particle instance (the particle world's thread).
pub fn on_particle_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical_smoke::particle_tick(ctx);
    Flow::Continue
}
