//! Patch 67: the kill feed shows a kill through a wall, by a ricochet and
//! by pain (client_patch.md section 3.55; the rules:
//! docs/notes/kill-feed-icons.md).
//!
//! A death's kill event gets the icons in its +5 (0.69d0's rule for its
//! penetration_icon and ricochet_icon, from the killing bullet, when someone
//! else killed and not by melee: a ricochet, else a pierce; pain when the
//! victim's pain pool is empty and the body and head pools are not: one hit
//! that empties pain and body or head together is a death by damage, the
//! owner's rule). All of it is the simulation's own state, the same in
//! every game; only the hud reads +5. The hud's kill log row passes them to
//! the movie as the ints penetration_icon, ricochet_icon and pain_icon.

#![forbid(unsafe_code)]

use crate::vostok::kill_feed::{CauseIcon, KillIcons, KillLogRow, KilledEvent, Shot};
use crate::vostok::{Context, Flow, NoExit, PartName};

/// `killed_event.cause_icon`: the event is written, before its queueing.
pub fn on_cause_icon(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut event) = KilledEvent::at(ctx) else {
        return Flow::Continue;
    };
    let bullet = event.bullet().filter(|_| event.cause() != CauseIcon::NoKiller && !event.melee());
    let shot = match bullet {
        Some(bullet) if bullet.ricocheted() => Shot::Ricochet,
        Some(bullet) if bullet.pierced() => Shot::ThroughWall,
        _ => Shot::Straight,
    };
    let pain = event.victim_part(PartName::Pain).is_some_and(|part| !part.has_health())
        && event.victim_part(PartName::Body).is_some_and(|part| part.has_health())
        && event.victim_part(PartName::Head).is_some_and(|part| part.has_health());
    event.set_icons(KillIcons::new(shot, pain));
    Flow::Continue
}

/// `player_killed.mastery_icon`: the row's description, after extra_icon.
pub fn on_mastery_icon(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let mut row = KillLogRow::at(ctx);
    let icons = row.icons();
    row.set_int(c"penetration_icon", u32::from(icons.wall()));
    row.set_int(c"ricochet_icon", u32::from(icons.ricochet()));
    row.set_int(c"pain_icon", u32::from(icons.pain()));
    Flow::Continue
}
