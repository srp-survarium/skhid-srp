//! Patch 180: the technical tree's skills the exe computes (docs/notes/technical-skills.md).
//! Every skill travels packed in kit modifier 47 (`spot_enemy_traps`), parsed once by the
//! `technical-skills` crate, which srp shares.
#![forbid(unsafe_code)]

use crate::vostok::{
    Context, Flow, NoExit, TechnicalSkillsOnTrapReopenExit, TechnicalSkillsOnTrapShotExit,
    technical::{self, KitParameter},
};

/// Tank Pocket (413) or Tank Depth (414) at the lobby's kit parameter: a kit with an oxygen tank
/// among its worn items gets a pocket or +1 pocket size. Every kit pocket count of the game is the
/// lobby's (the match takes the kit's pockets as the lobby filled them).
pub fn on_kit_parameter(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(site) = technical::kit_parameter(ctx) else {
        return Flow::Continue;
    };
    if !site.oxygen_tank {
        return Flow::Continue;
    }
    let (pockets, size) = site.skills.tank_harness().pockets();
    let added = match site.parameter {
        KitParameter::Pockets => pockets,
        KitParameter::PocketSize => size,
    };
    if added > 0 {
        technical::add_to_kit_parameter(ctx, f32::from(added));
    }
    Flow::Continue
}

/// Steady Breath (403): the aimed sway while the oxygen tank is on.
pub fn on_breath_vibration(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::breath_vibration(ctx);
    Flow::Continue
}

/// Deep Breaths (431): the stamina's regeneration while the oxygen tank is on.
pub fn on_dispersion(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::dispersion(ctx);
    Flow::Continue
}

pub fn on_sprint_cost(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::sprint_cost(ctx);
    Flow::Continue
}


/// Oxygen Rush (442): the tank spent its air for this tick.
pub fn on_tank_spent(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_spent(ctx);
    Flow::Continue
}

/// Oxygen Rush's end, every simulation tick of every player.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::oxygen_rush_tick(ctx);
    Flow::Continue
}

/// Oxygen Rush's speed.
pub fn on_movement(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::oxygen_rush_movement(ctx);
    Flow::Continue
}

/// Oxygen Rush's saved word: none in a new tank and after a refill; saved and loaded with the tank.
pub fn on_tank_construct(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_construct(ctx);
    Flow::Continue
}
pub fn on_tank_refill(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_refill(ctx, false);
    Flow::Continue
}
pub fn on_tank_refill_2(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_refill(ctx, true);
    Flow::Continue
}
pub fn on_tank_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_save(ctx);
    Flow::Continue
}
pub fn on_tank_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::tank_load(ctx);
    Flow::Continue
}

/// Dexterous Hands (401): the take animation of a skilled taker plays faster.
pub fn on_battery_take(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::battery_take(ctx);
    Flow::Continue
}

/// Dexterous Hands (401): a skilled player's time at an ammo crate counts faster.
pub fn on_resupply_time(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::resupply_time(ctx);
    Flow::Continue
}

/// Sapper (411): the disarming's time.
pub fn on_defuse_time(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::defuse_time(ctx);
    Flow::Continue
}

/// Rearm (433): a shot trap of a skilled owner stays disarmed and reopens after 20 s.
pub fn on_trap_shot(ctx: &mut Context<'_>) -> Flow<TechnicalSkillsOnTrapShotExit> {
    match technical::trap_shot(ctx) {
        true => Flow::Exit(TechnicalSkillsOnTrapShotExit::Shot),
        false => Flow::Continue,
    }
}

/// Rearm's reopening.
pub fn on_trap_reopen(ctx: &mut Context<'_>) -> Flow<TechnicalSkillsOnTrapReopenExit> {
    match technical::trap_reopen(ctx) {
        true => Flow::Exit(TechnicalSkillsOnTrapReopenExit::Reopened),
        false => Flow::Continue,
    }
}

/// Blast Drill (404): less damage from one's own and teammates' grenades.
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::blast_drill(ctx);
    Flow::Continue
}

/// Bear Trap (444): a leg its owner's trap catches breaks.
pub fn on_wound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::bear_trap(ctx);
    Flow::Continue
}

/// Scavenger (443): a grenade kill gives the grenade back.
pub fn on_killed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::scavenger(ctx);
    Flow::Continue
}

/// Trophy Trap (423): a disarmed own or enemy trap into the disarmer's trap pocket.
pub fn on_trap_defused(ctx: &mut Context<'_>) -> Flow<NoExit> {
    technical::trophy_trap(ctx);
    Flow::Continue
}
