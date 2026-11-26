//! What patch 206 (`second_sights`) reads and writes of the game: the
//! second sight's press in an applied input, a weapon's tick, its
//! deactivation, its making and its end, the reads of its aim by the shot's
//! dispersion, and the player's state's save and load (the rules:
//! docs/notes/second-sights.md, "The simulation (0.26g)").
//!
//! A weapon's sights are its player's, by the weapon slot (7 or 10) that
//! holds it: a [`Holder`]. Every value is read here once, typed; the patch
//! keeps the states and decides.

use second_sight::{GameTime, Sights, View, WeaponSlot};
use srp_validate::world::DictId;

use super::carry_runtime::{self, Key};
use super::config::ConfigValue;
use super::context::{Context, Flag, Reg};
use super::hook::Thread;
use super::world::{Player, read};
use super::ActionMask;
use crate::raw;

/// A weapon by its address: compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponId(u32);

#[cfg(test)]
impl WeaponId {
    pub const fn numbered(number: u32) -> WeaponId {
        WeaponId(number)
    }
}

/// A weapon in one of a player's weapon slots: whose sights they are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Holder {
    /// The player's life (its place and kit, its object, its spawn).
    pub player: Key,
    pub slot: WeaponSlot,
    /// The slot's gun (its item's dict id): a state is of the gun it was
    /// made for.
    pub gun: Option<DictId>,
    pub weapon: WeaponId,
}

/// The dict id of `item`, if it has a dictionary entry.
///
/// # Safety
/// `item` is a live inventory item on the hook's thread.
unsafe fn gun_of(item: raw::Item) -> Option<DictId> {
    // SAFETY: the caller's; an item's dictionary entry (+25Ch) and its id.
    unsafe { DictId::new(read(item.entry()?.dict_id())) }
}

/// The items of `player`'s two weapon slots.
///
/// # Safety
/// `player` is a live player on the hook's thread.
unsafe fn weapon_slots(player: raw::Player) -> [(WeaponSlot, Option<raw::Item>); 2] {
    // SAFETY: the caller's: its inventory's 23 slots, each an item or 0.
    let item = |slot: WeaponSlot| unsafe {
        player.inventory().and_then(|inventory| inventory.slots().at(usize::from(slot.slot())).item())
    };
    [WeaponSlot::First, WeaponSlot::Second].map(|slot| (slot, item(slot)))
}

/// `weapon` as `player`'s, when one of its weapon slots holds it (a slot
/// holds a weapon's item, the weapon + 10h).
///
/// # Safety
/// `player` and `weapon` are live on the hook's thread (the weapon only
/// compared until a slot is found to hold it).
unsafe fn holder_of(thread: &Thread, player: raw::Player, weapon: raw::Weapon) -> Option<Holder> {
    let key = Player { this: player, thread }.carry_key()?;
    // SAFETY: the caller's; the weapon's item is its address + 10h.
    let item = unsafe { weapon.item() };
    // SAFETY: as above.
    let (slot, held) = unsafe { weapon_slots(player) }.into_iter().find(|(_, held)| *held == Some(item))?;
    // SAFETY: a slot's live item.
    let gun = held.and_then(|held| unsafe { gun_of(held) });
    Some(Holder { player: key, slot, gun, weapon: WeaponId(weapon.address()) })
}

/// `weapon` as its user's, with its last tick's time (+5A4h): the time
/// its reads within a tick are at.
///
/// # Safety
/// `weapon` is a live weapon on the hook's thread.
unsafe fn holder(thread: &Thread, weapon: raw::Weapon) -> Option<(Holder, GameTime)> {
    // SAFETY: the caller's: its user (0 once deactivated), its tick's time.
    unsafe {
        let user = weapon.user()?;
        Some((holder_of(thread, user, weapon)?, GameTime(read(weapon.last_tick()))))
    }
}

/// The second sight's press in the input `sights.press` applies: its bit
/// set now and clear in the actions before. The weapon is what the player
/// holds, when one of its weapon slots does; its time the input's.
pub fn pressed(ctx: &Context<'_>) -> Option<(Holder, GameTime)> {
    use raw::site::sights_press as site;
    // SAFETY: the site's typed registers: EDI the player apply_new_input
    // runs for, ESP its frame (the actions before at +24h), EBP its frame
    // (the time at +8); all alive while it runs on the game's thread.
    unsafe {
        let player = site::player(ctx)?;
        let before = read(site::before(ctx)?.before_actions());
        let now = read(player.actions());
        let time = GameTime(read(site::time(ctx)?.time()));
        let bit = ActionMask::SCOPE_NEXT.bits();
        if before & bit != 0 || now & bit == 0 {
            return None;
        }
        let weapon = player.held_item()?.weapon();
        Some((holder_of(ctx.thread(), player, weapon)?, time))
    }
}

/// The weapon `sights.tick` ticks, and the tick's time.
pub fn ticked(ctx: &Context<'_>) -> Option<(Holder, GameTime)> {
    use raw::site::sights_tick as site;
    // SAFETY: the site's EBP is the weapon weapon_core::tick runs for, EBX
    // its time.
    unsafe {
        let weapon = site::weapon(ctx)?;
        Some((holder_of(ctx.thread(), weapon.user()?, weapon)?, GameTime(site::time(ctx))))
    }
}

/// The weapon `sights.deactivate` puts away (its user still set).
pub fn deactivated(ctx: &Context<'_>) -> Option<Holder> {
    // SAFETY: the site's ECX is the weapon at weapon_core::deactivate's entry.
    unsafe {
        let weapon = raw::site::sights_deactivate::weapon(ctx)?;
        holder_of(ctx.thread(), weapon.user()?, weapon)
    }
}

/// What a weapon config's `data` says of a second sight: two sights with a
/// nonzero `srp_second_sight` (the optic's dict id) and an
/// `srp_second_side` of 0 or 1 (the weapon kit's, patch 161's reading),
/// else one.
///
/// # Safety
/// `root` is a live weapon config's root.
unsafe fn sights_of(root: raw::EngineCfg) -> Sights {
    // SAFETY: the caller's.
    let Some(data) = unsafe { ConfigValue::read(root) }.ok().and_then(|root| root.table().ok())
        .and_then(|root| root.get(c"data").ok().flatten()).and_then(|data| data.table().ok()) else {
        return Sights::One;
    };
    let integer = |key| data.get(key).ok().flatten().and_then(|value| value.integer().ok());
    match (integer(c"srp_second_sight"), integer(c"srp_second_side")) {
        (Some(optic), Some(0 | 1)) if optic != 0 => Sights::Two,
        _ => Sights::One,
    }
}

/// The weapon `sights.made` makes, and what its config gives it.
pub fn made(ctx: &Context<'_>) -> Option<(WeaponId, Sights)> {
    // SAFETY: the site's ESP is weapon_core::weapon_core's entry frame: the
    // weapon's address (identity) and its config's root, the cook's, alive
    // for the constructor.
    unsafe {
        let frame = raw::site::sights_made::frame(ctx)?;
        let weapon = WeaponId(read(frame.weapon()));
        Some((weapon, frame.config().map_or(Sights::One, |root| sights_of(root))))
    }
}

/// The weapon `sights.destroyed` destroys.
pub fn destroyed(ctx: &Context<'_>) -> Option<WeaponId> {
    // SAFETY: ~weapon_core's entry frame: the weapon's address, compared.
    unsafe { Some(WeaponId(read(raw::site::sights_destroyed::frame(ctx)?.weapon()))) }
}

/// A read of a weapon's aim by the shot's dispersion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AimRead {
    /// get_dispersed_bullet_direction's aim progress (`sights.bullet_aim`).
    Bullet,
    /// dispersion_calculator::tick's m_aimed (`sights.dispersion_aimed`).
    Dispersion,
    /// get_skill_influence's m_aimed (`sights.skill_aimed`).
    Skill,
    /// get_target_koef's m_aimed (`sights.target_aimed`).
    Target,
}

/// The weapon whose aim `read` reads, and its time.
pub fn aiming(ctx: &Context<'_>, read: AimRead) -> Option<(Holder, GameTime)> {
    // SAFETY: each site's typed register is the weapon the dispersion
    // calculator (+3F8h, its [+0] the weapon) or the shot reads.
    unsafe {
        let weapon = match read {
            AimRead::Bullet => raw::site::sights_bullet_aim::weapon(ctx),
            AimRead::Dispersion => raw::site::sights_dispersion_aimed::weapon(ctx),
            AimRead::Skill => raw::site::sights_skill_aimed::weapon(ctx),
            AimRead::Target => raw::site::sights_target_aimed::weapon(ctx),
        }?;
        holder(ctx.thread(), weapon)
    }
}

/// The aim progress the shot read (`sights.bullet_aim`'s XMM1).
pub fn bullet_aim(ctx: &Context<'_>) -> f32 {
    ctx.xmm_f32(1)
}

/// The shot takes `aim` for the aim progress it compares with 1.0.
pub fn set_bullet_aim(ctx: &mut Context<'_>, aim: f32) {
    ctx.set_xmm_f32(1, aim);
}

/// The m_aimed `read` reads is taken as not aimed, for a `Flow::Skip` of
/// the read: the test's flags or the loaded byte as a 0 makes them.
pub fn take_as_unaimed(ctx: &mut Context<'_>, read: AimRead) {
    match read {
        AimRead::Bullet => {}
        // cmp byte [eax+5E8h],0 of a 0: the je takes the hip's multiplier
        AimRead::Dispersion => ctx.set_flag(Flag::Zf, true),
        // mov cl,[ecx+5E8h] / mov dl,[eax+5E8h] of a 0
        AimRead::Skill => ctx.set_reg8(Reg::Ecx, 0),
        AimRead::Target => ctx.set_reg8(Reg::Edx, 0),
    }
}

/// A player's state being saved at its end (`carry.save`, after patch 78's
/// snapshot): whose, and the guns of its weapon slots.
pub struct Saving {
    pub player: Key,
    pub guns: [Option<DictId>; 2],
}

/// The guns of `player`'s weapon slots, by [`WeaponSlot`] (first, second).
///
/// # Safety
/// `player` is live on the hook's thread.
unsafe fn guns(player: raw::Player) -> [Option<DictId>; 2] {
    // SAFETY: the caller's; each slot's live item.
    unsafe { weapon_slots(player) }.map(|(_, item)| item.and_then(|item| unsafe { gun_of(item) }))
}

pub fn saving(ctx: &Context<'_>) -> Option<Saving> {
    // SAFETY: base_player::serialize's epilogue: EBX its live player.
    let player = unsafe { raw::site::carry_save::player(ctx) }?;
    let key = Player { this: player, thread: ctx.thread() }.carry_key()?;
    // SAFETY: as above.
    Some(Saving { player: key, guns: unsafe { guns(player) } })
}

/// Appends `bytes` to the state being saved (the game's byte writer, as
/// the carry snapshot's).
pub fn write(ctx: &Context<'_>, bytes: &[u8]) {
    // SAFETY: base_player::serialize's epilogue: EDI its live writer.
    if let Some(writer) = unsafe { raw::site::carry_save::writer(ctx) } {
        carry_runtime::write_bytes(writer, bytes);
    }
}

/// A player's state loaded to its end (`carry.load`, after patch 78's
/// snapshot): whose, the guns of its weapon slots (the inventory is
/// loaded), and the record that ends it, `None` for a state without one.
pub struct Loading {
    pub player: Key,
    pub guns: [Option<DictId>; 2],
    pub record: Option<second_sight::Record>,
}

pub fn loading(ctx: &Context<'_>) -> Option<Loading> {
    // SAFETY: base_player::deserialize after its last native field: EBX
    // its live player, ESI its reader over the state's bytes alone (the
    // history item's copy, snapshot_rules.md 7.1).
    let (player, reader) = unsafe { (raw::site::carry_load::player(ctx)?, raw::site::carry_load::reader(ctx)?) };
    let key = Player { this: player, thread: ctx.thread() }.carry_key()?;
    // The state's checked bytes (patch 87) end with the record or with the
    // carry; the record is read whole or not at all.
    let record = carry_runtime::read_bytes::<{ second_sight::Record::BYTES }>(reader)
        .and_then(|bytes| second_sight::Record::decode(&bytes));
    // SAFETY: as above.
    Some(Loading { player: key, guns: unsafe { guns(player) }, record })
}

/// What the view shows of `weapon`'s sights at its user's simulation time
/// (+324h, a tick's): `None` for a weapon with no second sight or in no
/// weapon slot of a player (patch 161's view reads it each frame).
///
/// # Safety
/// `weapon` is a live weapon on the caller's thread (a draw's or a hook's).
// the view reads sight_at (a frame's time, a tick's): no reader at the user's time yet
#[allow(dead_code)]
pub(in crate::vostok) unsafe fn sight(thread: &Thread, weapon: raw::Weapon) -> Option<View> {
    // SAFETY: the caller's: its user and the user's time.
    let now = unsafe { GameTime(read(weapon.user()?.time())) };
    // SAFETY: the caller's.
    unsafe { sight_at(thread, weapon, now) }
}

/// The same at `now` (a frame's time between two ticks, for a smooth
/// blend: the switch's s is a line in time).
///
/// # Safety
/// As [`sight`].
pub(in crate::vostok) unsafe fn sight_at(thread: &Thread, weapon: raw::Weapon, now: GameTime) -> Option<View> {
    // SAFETY: the caller's: its user.
    unsafe { crate::second_sights::view(thread, &holder_of(thread, weapon.user()?, weapon)?, now) }
}
