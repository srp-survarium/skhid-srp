//! A death's kill event and the hud's kill log row.
//!
//! base_player::generate_killed_event (0x2DA170) takes a node of the
//! world's events (0x2D43B0 zeroes its +0..+0Bh), writes the victim (+0),
//! the killer (+1), the weapon's icon (+2) and the cause icon (+4), then
//! queues it (0x2D0A60). At `killed_event.cause_icon` (sites_026g.tsv,
//! before the cause icon's store) EAX is the node, BL the cause icon, ESI
//! the victim, EBP the killing bullet or 0, and the frame's [esp+14h] says
//! the bullet's damage is its weapon's melee.
//!
//! When the event plays, the hud's handler (0x23B5F0) gives
//! game_world_ui::on_player_killed (0x237C70) the node's bytes, the word at
//! +4 fifth (KILL_FEED_ARGUMENT: the cause icon and, in its second byte, the
//! node's +5). At `player_killed.mastery_icon` (after extra_icon) the frame
//! holds a GFx value it reuses ([esp+14h]), the row's description
//! ([esp+2Ch]) and that second byte ([esp+105h]).
//! What those registers and slots hold is said here, once.

use core::ffi::CStr;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::damage::{BodyPart, DamageModel, PartName};
use super::gfx::{self, Value};
use super::hook::Thread;
use super::object::{Object, read, write};
use crate::raw;

/// The cause icon of a kill event (+4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CauseIcon {
    /// A kill by someone else, not a headshot.
    Kill,
    Headshot,
    /// No killer, or the victim himself.
    NoKiller,
    /// A value the game does not write here.
    Other(u8),
}

impl CauseIcon {
    fn parse(raw: u8) -> CauseIcon {
        match raw {
            0 => CauseIcon::Kill,
            1 => CauseIcon::Headshot,
            3 => CauseIcon::NoKiller,
            other => CauseIcon::Other(other),
        }
    }
}

/// How the killing bullet came to the victim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shot {
    /// Straight, or no bullet to tell.
    Straight,
    /// Through something (a wall, another player).
    ThroughWall,
    /// After a ricochet, with or without a pierce.
    Ricochet,
}

/// The kill feed's icons of a kill: the event's +5, the hud's row's flags
/// (1 a wall, 2 pain, 4 a ricochet; the hud movie reads each alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KillIcons(u8);

impl KillIcons {
    const WALL: u8 = 1;
    const PAIN: u8 = 2;
    const RICOCHET: u8 = 4;

    pub fn new(shot: Shot, pain: bool) -> KillIcons {
        let shot = match shot {
            Shot::Straight => 0,
            Shot::ThroughWall => Self::WALL,
            Shot::Ricochet => Self::RICOCHET,
        };
        KillIcons(shot | if pain { Self::PAIN } else { 0 })
    }

    pub fn wall(self) -> bool {
        self.0 & Self::WALL != 0
    }

    pub fn ricochet(self) -> bool {
        self.0 & Self::RICOCHET != 0
    }

    pub fn pain(self) -> bool {
        self.0 & Self::PAIN != 0
    }
}

/// The kill event `killed_event.cause_icon` looks at.
pub struct KilledEvent<'t> {
    node: Object,
    cause: CauseIcon,
    victim: Option<Object>,
    bullet: Option<Object>,
    melee: bool,
    thread: PhantomData<&'t Thread>,
}

impl<'t> KilledEvent<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<KilledEvent<'t>> {
        let node = Object::at(ctx.reg(Reg::Eax))?;
        // SAFETY: the site's frame (its [esp+14h], a byte generate_killed_event
        // keeps there).
        let melee = unsafe { read(raw::killed_event_frame::melee(ctx.site_esp() as usize as *mut _)) } != 0;
        Some(KilledEvent {
            node,
            cause: CauseIcon::parse(ctx.reg(Reg::Ebx) as u8),
            victim: Object::at(ctx.reg(Reg::Esi)),
            bullet: Object::at(ctx.reg(Reg::Ebp)),
            melee,
            thread: PhantomData,
        })
    }

    pub fn cause(&self) -> CauseIcon {
        self.cause
    }

    /// The bullet's damage is its weapon's melee.
    pub fn melee(&self) -> bool {
        self.melee
    }

    /// The bullet that killed, or none (a grenade, a fall, ..).
    pub fn bullet(&self) -> Option<Bullet<'t>> {
        self.bullet.map(|bullet| Bullet { bullet, thread: PhantomData })
    }

    /// The victim's part, or none (no damage model, no such part).
    pub fn victim_part(&self, name: PartName) -> Option<BodyPart<'t>> {
        // SAFETY: ESI at the site, the victim, a live base_player.
        let model = unsafe { DamageModel::of(self.victim?) }?;
        model.part(name)
    }

    /// Writes the event's kill feed icons (+5).
    pub fn set_icons(&mut self, icons: KillIcons) {
        // SAFETY: the node the site writes (+4 the cause icon); its +5 is
        // ours, zeroed by its allocation and read by the hud alone.
        unsafe { write(raw::killed_event::flags(self.node.ptr()), icons.0) }
    }
}

/// The killing bullet.
pub struct Bullet<'t> {
    bullet: Object,
    thread: PhantomData<&'t Thread>,
}

impl Bullet<'_> {
    /// It ricocheted before this hit (its reflections).
    pub fn ricocheted(&self) -> bool {
        // SAFETY: EBP at the site, a live bullet.
        unsafe { read(raw::bullet::reflections(self.bullet.ptr())) != 0 }
    }

    /// It went through something (its trajectory's segments past the first).
    pub fn pierced(&self) -> bool {
        // SAFETY: as ricocheted.
        unsafe { read(raw::bullet::segments(self.bullet.ptr())) != 0 }
    }
}

/// The kill log row `player_killed.mastery_icon` is filling.
pub struct KillLogRow<'t> {
    value: *mut Value,
    description: *const Value,
    icons: KillIcons,
    thread: PhantomData<&'t Thread>,
}

impl<'t> KillLogRow<'t> {
    pub fn at(ctx: &'t Context<'_>) -> KillLogRow<'t> {
        let frame = ctx.site_esp() as usize as *mut core::ffi::c_void;
        // SAFETY: on_player_killed's frame at the site: its GFx value, its
        // description and its fifth argument's second byte.
        unsafe {
            KillLogRow {
                value: raw::player_killed_frame::value(frame).cast(),
                description: raw::player_killed_frame::description(frame).cast(),
                icons: KillIcons(read(raw::player_killed_frame::icons(frame))),
                thread: PhantomData,
            }
        }
    }

    /// The kill's icons, as the event's +5 gave them.
    pub fn icons(&self) -> KillIcons {
        self.icons
    }

    /// The description's member `name` set to an int, as the game sets
    /// extra_icon (its GFx value reused).
    pub fn set_int(&mut self, name: &CStr, int: u32) {
        // SAFETY: the frame's GFx value and the row's description, the
        // game's at the site; SetMember takes the value's address.
        unsafe {
            gfx::set_int(self.value, int);
            gfx::set_member(self.description, name, self.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_the_movies_bits() {
        assert_eq!(KillIcons::new(Shot::Straight, false).0, 0);
        assert_eq!(KillIcons::new(Shot::ThroughWall, false).0, 1);
        assert_eq!(KillIcons::new(Shot::Ricochet, true).0, 6);
        let all = KillIcons(7);
        assert!(all.wall() && all.ricochet() && all.pain());
        assert_eq!(CauseIcon::parse(3), CauseIcon::NoKiller);
        assert_eq!(CauseIcon::parse(9), CauseIcon::Other(9));
    }
}
