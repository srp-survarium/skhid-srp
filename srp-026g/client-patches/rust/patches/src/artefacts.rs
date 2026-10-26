//! Patch 54: the artefacts (client_patch.md section 3.51; the rules:
//! docs/notes/artefacts.md): Spring's aim time and stopping power,
//! Lifebone's heal, one press fires every artefact of its kind, Onyx fired
//! together 1 s longer each and reducing a hit once. Their numbers are in
//! the artefacts' configs (release/content.tsv artefacts-1.json); this is
//! what the stock code could not read or do.
//!
//! - A Spring's passive second node becomes aim_time (49) with
//!   `passive/aim_time_mod`; two nodes after the object's own fields (its
//!   sizes grown, patch_026g.py ARTEFACTS_SIZES) are the active's: aim_time
//!   with `active/aim_time_mod` and list 42 with
//!   `active/stopping_power_mod`, linked and unlinked with the active's
//!   own three. List 42 (artefact_synergy, which no code of 0.26g reads)
//!   is the stagger's: a hit's slow is times max(0, 1 + its sum).
//! - Lifebone's active fills the pain, body and head pools to their
//!   maximum, as a medkit heals a pool.
//! - A press activates the one pressed and every other ready artefact of
//!   its class and kind 1 in the artefact slots, each as action activates
//!   one; each Onyx fired with others adds 1 s to every fired one's active
//!   time, so n fired together end together after 3 s + (n - 1) s.
//! - Of n active Onyx only the first of the slots takes its share of a hit;
//!   and only the first of the slots fired by the same press shows the
//!   activation hint, for the active's time left.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::artefact::{self, ACTIVE, Artefact, PASSIVE, Section, SpringLoad, SpringNode};
use crate::vostok::{ArtefactsOnActionExit, ArtefactsOnOnyxHintExit, ArtefactsOnOnyxHitExit, Context, Flow, NoExit,
                    Reg};

/// The aim_time modifier's index (49), and list 42 (the stagger's).
const AIM_TIME: u32 = 0x31;
const STOPPING: u32 = 0x2A;
/// Each Onyx fired with others: 1 s more for all of them.
const ONYX_EXTRA_MS: u32 = 1000;
const AIM_KEY: &CStr = c"aim_time_mod";
const STOPPING_KEY: &CStr = c"stopping_power_mod";
/// The pools Lifebone's active fills.
const HEALS: [&CStr; 3] = [c"pain", c"body", c"head"];

/// `artefacts.spring_loader`: the passive's aim time, the active's two
/// nodes.
pub fn on_spring_loader(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(load) = SpringLoad::at(ctx) else { return Flow::Continue };
    let passive = load.number(Section::Passive, AIM_KEY);
    load.set_passive_second(AIM_TIME, passive);
    let aim = load.number(Section::Active, AIM_KEY);
    load.set_node(SpringNode::Aim, AIM_TIME, aim);
    let stopping = load.number(Section::Active, STOPPING_KEY);
    load.set_node(SpringNode::Stopping, STOPPING, stopping);
    Flow::Continue
}

/// `artefacts.spring_link`: the active's two nodes go in first.
pub fn on_spring_link(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(spring) = Artefact::in_reg(ctx, Reg::Eax)
        && let Some(lists) = spring.inventory().and_then(|inventory| inventory.lists())
    {
        lists.link(&spring.spring_node(SpringNode::Aim));
        lists.link(&spring.spring_node(SpringNode::Stopping));
    }
    Flow::Continue
}

/// `artefacts.spring_unlink`: the active's two nodes out first.
pub fn on_spring_unlink(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(spring) = Artefact::on_stack(ctx, 4)
        && let Some(lists) = spring.inventory().and_then(|inventory| inventory.lists())
    {
        lists.unlink(&spring.spring_node(SpringNode::Aim));
        lists.unlink(&spring.spring_node(SpringNode::Stopping));
    }
    Flow::Continue
}

/// `artefacts.lifebone`: each pool of HEALS filled to its maximum.
pub fn on_lifebone(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let time = ctx.stack(0x14);
    if let Some(lifebone) = Artefact::in_reg(ctx, Reg::Edi)
        && let Some(pools) = lifebone.inventory().and_then(|inventory| inventory.pools())
    {
        for name in HEALS {
            pools.fill(name, time);
        }
    }
    Flow::Continue
}

/// Another artefact of the pressed one's class and kind 1.
fn kin(slot: &Option<Artefact<'_>>, pressed: &Artefact<'_>) -> bool {
    slot.as_ref().is_some_and(|other| !other.same(pressed) && other.same_class(pressed) && other.kind() == 1)
}

/// Activated if it is ready, `extra` ms more on an Onyx's active time.
fn fire(artefact: &Artefact<'_>, time: u32, extra: u32) {
    if artefact.ready() {
        artefact.activate(time);
        if extra != 0 {
            artefact.add_onyx_time(extra);
        }
    }
}

/// `artefacts.action`: a press fires every ready artefact of its kind; a
/// key-up is the original's.
pub fn on_action(ctx: &mut Context<'_>) -> Flow<ArtefactsOnActionExit> {
    if ctx.stack_u8(4) == 0 {
        return Flow::Continue;
    }
    let time = ctx.stack(8);
    let Some(pressed) = Artefact::in_reg(ctx, Reg::Ecx) else { return Flow::Continue };
    let inventory = pressed.inventory();
    let mut ready = u32::from(pressed.ready());
    if let Some(inventory) = &inventory {
        ready += inventory.artefacts().filter(|slot| kin(slot, &pressed) && slot.is_some_and(|a| a.ready())).count() as u32;
    }
    let extra = if ready as i32 - 1 > 0 && pressed.is_onyx() { (ready - 1) * ONYX_EXTRA_MS } else { 0 };
    fire(&pressed, time, extra);
    if let Some(inventory) = &inventory {
        for slot in inventory.artefacts() {
            if kin(&slot, &pressed)
                && let Some(other) = slot
            {
                fire(&other, time, extra);
            }
        }
    }
    Flow::Exit(ArtefactsOnActionExit::Fired)
}

/// `artefacts.stagger`: the hits' stopping power times max(0, 1 + list 42).
pub fn on_stagger(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let sum = ctx.xmm_f32(0);
    let Some(kit) = artefact::ModifierKit::of_player(ctx) else { return Flow::Continue };
    let slowed = kit.apply(STOPPING, sum, 1.0);
    ctx.set_stack(0x14, slowed.to_bits());
    ctx.set_xmm_f32(0, slowed);
    Flow::Continue
}

/// `artefacts.onyx_hit`: an active Onyx after the first active one of the
/// slots leaves the hit as it is.
pub fn on_onyx_hit(ctx: &mut Context<'_>) -> Flow<ArtefactsOnOnyxHitExit> {
    let Some(onyx) = Artefact::in_reg(ctx, Reg::Ecx) else { return Flow::Continue };
    if onyx.state() == PASSIVE {
        return Flow::Continue;
    }
    let Some(inventory) = onyx.inventory() else { return Flow::Continue };
    for other in inventory.artefacts().flatten() {
        if other.same(&onyx) {
            break; // the first active one
        }
        if other.same_class(&onyx) && other.state() == ACTIVE {
            return Flow::Exit(ArtefactsOnOnyxHitExit::Unchanged);
        }
    }
    Flow::Continue
}

/// `artefacts.onyx_hint`: the hint for the active's time left, once a press.
pub fn on_onyx_hint(ctx: &mut Context<'_>) -> Flow<ArtefactsOnOnyxHintExit> {
    let shown = {
        let Some(onyx) = artefact::hint_onyx(ctx) else { return Flow::Continue };
        let first = onyx.inventory().is_none_or(|inventory| {
            for other in inventory.artefacts().flatten() {
                if other.same(&onyx) {
                    return true;
                }
                if other.same_class(&onyx) && other.state() == ACTIVE && other.activated() == onyx.activated() {
                    return false; // one before it, fired by the same press
                }
            }
            true
        });
        first.then(|| artefact::hint_time(&onyx))
    };
    match shown {
        Some(time) => {
            ctx.set_stack(0, time);
            Flow::Continue
        }
        None => Flow::Exit(ArtefactsOnOnyxHintExit::Skip),
    }
}
