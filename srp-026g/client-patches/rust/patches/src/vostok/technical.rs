//! The technical tree's native sites (patch 180, technical-skills.md): a kit's packed skills
//! (modifier 47, parsed by the `technical-skills` crate) and what each site reads of the game.
use technical_skills::{Clock, SecondWind, Skills, TankTick};

use super::config::{ConfigError, ConfigValue};
use super::context::{Context, Reg};
use super::world::read;
use crate::raw;

/// The packed technical skills of a kit's modifiers; none when the value is not valid.
///
/// # Safety
/// `modifiers` is a live kit's.
unsafe fn parse(modifiers: raw::PlayerParamsModifiersContainer) -> Skills {
    // SAFETY: the caller's live kit owns its modifier block.
    Skills::parse(unsafe { read(modifiers.technical_ranks()) }).unwrap_or_default()
}

/// The packed technical skills of `player`'s current kit (the match profile's).
pub(super) fn skills(player: raw::Player) -> Skills {
    // SAFETY: a hook's player owns its profile and current kit.
    super::medical::kit_modifiers(player).map(|m| unsafe { parse(m) }).unwrap_or_default()
}

/// A kit's armour slots (head .. back, `slots_restrictions` 0..6): an oxygen tank's place.
const WORN: core::ops::RangeInclusive<usize> = 0..=6;
/// A kit slot with no item.
const NO_ITEM: u16 = 0xFFFF;
/// `data.type` of an oxygen tank (the inventory cook's switch 0x31F660: 1, `oxygen_tank`).
const OXYGEN_TANK: u32 = 1;

/// An item config's `data.type`, none without one.
///
/// # Safety
/// `entry` is a live dictionary entry; its config lives with the dictionary.
unsafe fn item_type(entry: raw::ItemEntry) -> Option<u32> {
    let kind = || -> Result<u32, ConfigError> {
        // SAFETY: the entry's config holder and root table live with the dictionary.
        let root = unsafe { entry.config().and_then(|c| c.root()) }.ok_or(ConfigError::Missing)?;
        // SAFETY: the dictionary keeps the item configs loaded while the lobby runs.
        let data = unsafe { ConfigValue::read(root) }?.table()?.required(c"data")?.table()?;
        data.required(c"type")?.integer()
    };
    kind().ok()
}

/// Which lobby kit parameter an apply_kit_parameter computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KitParameter {
    /// 15, `pocket_slots`.
    Pockets,
    /// 17, `pocket_slot_size`.
    PocketSize,
}

impl KitParameter {
    fn parse(index: u32) -> Option<Self> {
        match index {
            15 => Some(Self::Pockets),
            17 => Some(Self::PocketSize),
            _ => None,
        }
    }
}

/// The lobby's kit parameter at apply_kit_parameter's epilogue (`medical.pocket_count`, the medical tree's clamp after it): which
/// one, the kit's technical skills and whether one of its worn items is an oxygen tank.
pub struct KitParameterSite {
    pub parameter: KitParameter,
    pub skills: Skills,
    pub oxygen_tank: bool,
}

/// The site's view; none for a parameter the tank skills do not touch.
pub fn kit_parameter(ctx: &Context<'_>) -> Option<KitParameterSite> {
    use raw::site::medical_pocket_count as site;
    // SAFETY: the epilogue's registers are the function's: EBX the kit record inside EBP's
    // profile, ESI the parameter, ESP its frame with the dictionary argument; all live for the
    // call, on the lobby's thread.
    unsafe {
        let parameter = KitParameter::parse(site::parameter(ctx))?;
        let (kit, profile, frame) = (site::kit(ctx)?, site::profile(ctx)?, site::frame(ctx)?);
        let dictionary = frame.dictionary();
        let slots = kit.slots().slots();
        let oxygen_tank = WORN.into_iter().any(|slot| {
            let index = read(slots.at(slot).item());
            if index == NO_ITEM {
                return false;
            }
            let dict = read(profile.items().at(usize::from(index)).dict());
            raw::engine_item_by_id(dictionary, u32::from(dict))
                .and_then(|entry| item_type(entry))
                .is_some_and(|kind| kind == OXYGEN_TANK)
        });
        Some(KitParameterSite { parameter, skills: parse(kit.modifiers()), oxygen_tank })
    }
}

/// Adds `amount` to the parameter the site computed (XMM0).
pub fn add_to_kit_parameter(ctx: &mut Context<'_>, amount: f32) {
    let value = ctx.xmm_f32(0);
    ctx.set_xmm_f32(0, value + amount);
}

/// `player`'s oxygen tank: the first item of its inventory whose vtable is the tank's (the back
/// slot holds it; a backpack there is armour and fails the test).
///
/// # Safety
/// `player` is alive for the hook; its inventory owns the 23 slots' array and their items.
unsafe fn tank_of(player: raw::Player) -> Option<raw::OxygenTank> {
    unsafe {
        let inventory = player.inventory()?;
        let first = inventory.slots();
        let count = inventory.slots_end().index_from(first).unwrap_or(0);
        (0..count).find_map(|slot| {
            let item = first.at(slot).item()?;
            (read(item.vtable()) == raw::image::OXYGEN_TANK_VTABLE).then(|| item.oxygen_tank())
        })
    }
}

/// Whether `player`'s oxygen tank is switched on now (its saved m_active).
///
/// # Safety
/// As [`tank_of`].
unsafe fn tank_on(player: raw::Player) -> bool {
    // SAFETY: the caller's live player's tank.
    unsafe { tank_of(player).is_some_and(|tank| read(tank.active()) != 0) }
}

/// The player holding `tank` (item -> inventory -> holder -> player, as the tank's tick asks).
///
/// # Safety
/// `tank` is a live item of a hook.
unsafe fn holder(tank: raw::OxygenTank) -> Option<raw::Player> {
    unsafe { Some(tank.item().inventory()?.holder()?.player()) }
}

/// Steady Breath (403) at the aimed sway's stores (`technical.breath_vibration`): while the
/// weapon's user has its oxygen tank on, both sway offsets x [`Skills::tank_sway`].
pub fn breath_vibration(ctx: &mut Context<'_>) {
    use raw::site::technical_breath_vibration as site;
    // SAFETY: the calculator's tick runs for a live weapon of a living user (weapon_core::tick
    // calls it only then); its user owns its kit and inventory.
    let factor = unsafe {
        let Some(player) = site::calculator(ctx).and_then(|c| c.user()) else { return };
        if !tank_on(player) {
            return;
        }
        skills(player).tank_sway()
    };
    if factor == 1.0 {
        return;
    }
    let scale = |bits: u32| (f32::from_bits(bits) * factor).to_bits();
    let (h, v) = (ctx.reg(Reg::Eax), ctx.reg(Reg::Ecx));
    ctx.set_reg(Reg::Eax, scale(h));
    ctx.set_reg(Reg::Ecx, scale(v));
}

/// Steady Breath (403) at a shot's dispersion (`technical.dispersion`): while the shooter's
/// oxygen tank is on, the dispersion x [`Skills::tank_sway`], as its sway.
pub fn dispersion(ctx: &mut Context<'_>) {
    use raw::site::technical_dispersion as site;
    // SAFETY: instant_fire's weapon, whose user it has just read (0x2E243D).
    let factor = unsafe {
        let Some(player) = site::weapon(ctx).and_then(|w| w.user()) else { return };
        if !tank_on(player) {
            return;
        }
        skills(player).tank_sway()
    };
    if factor != 1.0 {
        let dispersion = ctx.xmm_f32(1);
        ctx.set_xmm_f32(1, dispersion * factor);
    }
}

/// Deep Breaths (431) at the sprint's stamina cost (`medical.sprint`, after Numb Legs): while the
/// player's oxygen tank is on, the cost a second x [`Skills::tank_sprint_cost`].
pub fn sprint_cost(ctx: &mut Context<'_>) {
    use raw::site::medical_sprint as site;
    // SAFETY: the stamina is embedded in its live player (base_player +6C0h).
    let factor = unsafe {
        let Some(stamina) = site::stamina(ctx) else { return };
        let player = stamina.player();
        if !tank_on(player) {
            return;
        }
        skills(player).tank_sprint_cost()
    };
    if factor != 1.0 {
        let cost = ctx.xmm_f32(0);
        ctx.set_xmm_f32(0, cost * factor);
    }
}


/// Second Wind (442) at the tank's tick after the air is spent (`technical.tank_spent`): a tick
/// that takes the tank past another 30 s of use ([`TankTick::second_wind_mark`]) starts its boost,
/// or gives a running one its 20 s again, and fills its holder's stamina.
pub fn tank_spent(ctx: &Context<'_>) {
    use raw::site::technical_tank_spent as site;
    // SAFETY: the tick's live tank and its living holder (the dead path jumps past the site);
    // the holder's stamina is embedded in it.
    unsafe {
        let Some(tank) = site::tank(ctx) else { return };
        let tick = TankTick::parse(read(tank.capacity()), site::left(ctx), site::spent(ctx));
        if !tick.is_some_and(TankTick::second_wind_mark) {
            return;
        }
        let Some(player) = holder(tank) else { return };
        if !skills(player).second_wind() {
            return;
        }
        let stamina = player.stamina();
        *stamina.current() = read(stamina.maximum());
        let now = Clock(read(stamina.clock()));
        *tank.second_wind_until() = SecondWind::start(now).raw();
        *player.refresh_selection() = 1;
    }
}

/// Second Wind's end, each simulation tick of each player (`battery.carrier_tick`): a boost whose
/// end the player's stamina clock reached is over, and the movement selected again.
pub fn second_wind_tick(ctx: &Context<'_>) {
    use raw::site::battery_carrier_tick as site;
    // SAFETY: base_player::tick's live player, its inventory's tank and its embedded stamina.
    unsafe {
        let Some(player) = site::player(ctx) else { return };
        let Some(tank) = tank_of(player) else { return };
        let wind = SecondWind::parse(read(tank.second_wind_until()));
        let next = wind.at(Clock(read(player.stamina().clock())));
        if next != wind {
            *tank.second_wind_until() = next.raw();
            *player.refresh_selection() = 1;
        }
    }
}

/// Second Wind's speed at the movement speed's end (`medical.movement`, after the medical tree).
pub fn second_wind_movement(ctx: &mut Context<'_>) {
    use raw::site::medical_movement as site;
    // SAFETY: the movement epilogue's live player and its inventory's tank.
    let factor = unsafe {
        let Some(player) = site::player(ctx) else { return };
        tank_of(player).map_or(1.0, |tank| SecondWind::parse(read(tank.second_wind_until())).speed())
    };
    if factor != 1.0 {
        let speed = ctx.xmm_f32(0);
        ctx.set_xmm_f32(0, speed * factor);
    }
}

/// The tank's saved boost: none when made (the allocator leaves its tail as it was) and after a
/// refill (a respawn's), so a boost never outlives its life.
pub fn tank_reset(tank: Option<raw::OxygenTank>) {
    // SAFETY: the ctor's or the refill's live tank; the word is our own padding.
    if let Some(tank) = tank {
        unsafe { *tank.second_wind_until() = SecondWind::Off.raw() };
    }
}

/// The ctor's epilogue (`technical.tank_construct`).
pub fn tank_construct(ctx: &Context<'_>) {
    // SAFETY: the site's EBP is the constructed tank.
    tank_reset(unsafe { raw::site::technical_tank_construct::tank(ctx) });
}

/// A refill (`technical.tank_refill`, `technical.tank_refill_2`).
pub fn tank_refill(ctx: &Context<'_>, second: bool) {
    // SAFETY: each site's ECX is the refilled tank.
    let tank = unsafe {
        match second {
            false => raw::site::technical_tank_refill::tank(ctx),
            true => raw::site::technical_tank_refill_2::tank(ctx),
        }
    };
    tank_reset(tank);
}

/// The tank's saved state after m_amount_ms (`technical.tank_save`): the boost's word.
pub fn tank_save(ctx: &Context<'_>) {
    use raw::site::technical_tank_save as site;
    // SAFETY: serialize's live tank and writer.
    unsafe {
        let (Some(tank), Some(writer)) = (site::tank(ctx), site::writer(ctx)) else { return };
        super::carry_runtime::write_bytes(writer, &read(tank.second_wind_until()).to_le_bytes());
    }
}

/// The tank's saved state after m_amount_ms (`technical.tank_load`): the boost's word, checked
/// against the stream's end; a stream without it leaves no boost.
pub fn tank_load(ctx: &Context<'_>) {
    use raw::site::technical_tank_load as site;
    // SAFETY: deserialize's live tank and reader.
    unsafe {
        let (Some(tank), Some(reader)) = (site::tank(ctx), site::reader(ctx)) else { return };
        let word = super::carry_runtime::read_bytes::<4>(reader).map_or(0, u32::from_le_bytes);
        *tank.second_wind_until() = SecondWind::parse(word).raw();
    }
}

/// Dexterous Hands (401) after the take animation's time scale is stored (`technical.battery_take`):
/// the taker's animation plays [`Skills::battery_take_speed`] times as fast, so the carry state
/// starts sooner. Every game builds it from the same state, battery and taker.
pub fn battery_take(ctx: &Context<'_>) {
    use raw::site::technical_battery_take as site;
    // SAFETY: the take state's live battery and its user; the frame's scale is the stored f32 the
    // next call reads.
    unsafe {
        let (Some(state), Some(frame)) = (site::state(ctx), site::frame(ctx)) else { return };
        let Some(player) = state.item().and_then(|battery| battery.user()) else { return };
        let speed = skills(player).battery_take_speed();
        if speed != 1.0 {
            *frame.scale() = read(frame.scale()) * speed;
        }
    }
}

/// Dexterous Hands (401) at an ammo crate's use (`technical.resupply_time`): EAX, the ms passed
/// since the use began, counted as the user's rank speeds them ([`Skills::crate_ms`]); the use
/// ends by them and the hud's bar divides them.
pub fn resupply_time(ctx: &mut Context<'_>) {
    use raw::site::technical_resupply_time as site;
    // SAFETY: update_use's live user, the player at the crate.
    let skills = unsafe {
        let Some(player) = site::player(ctx) else { return };
        skills(player)
    };
    let elapsed = ctx.reg(Reg::Eax);
    ctx.set_reg(Reg::Eax, skills.crate_ms(elapsed));
}

/// Sapper (411) at the defuse's time (`technical.defuse_time`): EAX, the disarming's ms, as the
/// disarmer's rank shortens it ([`Skills::disarm_ms`]); the hud's bar divides by the same.
pub fn defuse_time(ctx: &mut Context<'_>) {
    use raw::site::technical_defuse_time as site;
    // SAFETY: use_execute's live user data and its player, the disarmer.
    let skills = unsafe {
        let Some(player) = site::user(ctx).and_then(|user| user.player()) else { return };
        skills(player)
    };
    let ms = ctx.reg(Reg::Eax);
    ctx.set_reg(Reg::Eax, skills.disarm_ms(ms));
}

/// A trap as Rearm reads it: its kind and its phase, from its saved fields, read once.
enum TrapPhase {
    /// An explosive trap: the native delayed action is its explosion.
    Explosive,
    /// A plain trap, disarmed (by hand or by a shot).
    Disarmed,
    /// A plain trap in another state.
    Other,
}

/// The phase of a live `trap`.
///
/// # Safety
/// `trap` is a hook's live trap.
unsafe fn trap_phase(trap: raw::BoobyTrapCore) -> TrapPhase {
    // SAFETY: the trap's own fields.
    unsafe {
        match (read(trap.explosive()) != 0, read(trap.state())) {
            (true, _) => TrapPhase::Explosive,
            (false, 3) => TrapPhase::Disarmed,
            (false, _) => TrapPhase::Other,
        }
    }
}

/// The player whose pocket a trap is of.
///
/// # Safety
/// `trap` is a hook's live trap; its set is its owner's pocket item.
unsafe fn trap_owner(trap: raw::BoobyTrapCore) -> Option<raw::Player> {
    unsafe { Some(trap.set()?.item().inventory()?.holder()?.player()) }
}

/// A trap's state words (`booby_trap_core::switch_to_state`'s states).
const DISARMED: u32 = 3;
const ARMED: u32 = 1;
/// `m_action_time_in_ms`'s "never".
const NEVER: u32 = u32::MAX;

/// Rearm (433) where a bullet springs a trap (`technical.trap_shot`): a plain trap whose owner has
/// the skill is disarmed as natively, but stays (no expiry) and keeps its reopening time in its
/// saved action time. True when the hook did the hit (the site's `shot` exit).
pub fn trap_shot(ctx: &Context<'_>) -> bool {
    use raw::site::technical_trap_shot as site;
    // SAFETY: hit's live trap, its owner's pocket and player; the frame's first argument.
    unsafe {
        let (Some(trap), Some(frame)) = (site::trap(ctx), site::frame(ctx)) else { return false };
        if matches!(trap_phase(trap), TrapPhase::Explosive) {
            return false;
        }
        let Some(delay) = trap_owner(trap).and_then(|owner| skills(owner).rearm_ms()) else { return false };
        raw::engine_trap_switch_state(Some(trap), DISARMED);
        if matches!(trap_phase(trap), TrapPhase::Disarmed) {
            *trap.state_timer() = 0;
            *trap.action_time() = read(frame.now()).wrapping_add(delay);
        }
        true
    }
}

/// Rearm's reopening, where a trap's action time is reached (`technical.trap_reopen`): a plain
/// trap never runs the native delayed action (only an explosive one sets an action time
/// natively); a disarmed one is armed again, its sensor, use and hittable back. True when the hook
/// took the trap (the site's `reopened` exit).
pub fn trap_reopen(ctx: &Context<'_>) -> bool {
    use raw::site::technical_trap_reopen as site;
    // SAFETY: tick's live trap; the two engine calls are the trap's own, as insert makes them.
    unsafe {
        let Some(trap) = site::trap(ctx) else { return false };
        match trap_phase(trap) {
            TrapPhase::Explosive => false,
            TrapPhase::Disarmed => {
                *trap.action_time() = NEVER;
                raw::engine_trap_switch_state(Some(trap), ARMED);
                raw::engine_trap_insert_collision(Some(trap));
                true
            }
            TrapPhase::Other => {
                *trap.action_time() = NEVER;
                true
            }
        }
    }
}

/// `item` as a grenade stack, none for another item (a trap set answers no grenade set).
///
/// # Safety
/// `item` is a live item.
pub(super) unsafe fn grenade_set(item: raw::Item) -> Option<raw::GrenadeSet> {
    // SAFETY: an item's own vtable word; the cast only after it is a grenade set's.
    unsafe {
        let table = read(item.vtable());
        (table == raw::image::GRENADE_SET_VTABLE || table == raw::image::GRENADE_SET_CORE_VTABLE)
            .then(|| item.grenade_set())
    }
}

/// A hit's type: an explosion's (splash_damage_type, `HitType::Explosive`).
const EXPLOSIVE: u32 = 3;
/// A place byte's "none".
const NO_PLACE: u8 = 0xFF;

/// Blast Drill (404) where a hit's damage is final before armour (`medical.hit`, after the
/// medical tree's Brothers in Arms): a grenade's blast thrown by the victim itself or by a teammate
/// does [`Skills::friendly_blast`] of its damage to a victim with the skill. Enemies' grenades and
/// traps are left as they are. The thrower is the hit's attacker place (+44h, the place the game
/// credits the kill to).
pub fn blast_drill(ctx: &mut Context<'_>) {
    use raw::site::medical_hit as site;
    // SAFETY: hit_body_part's live model and its owner, the hit and its item, the frame's amount;
    // the roster lookup holds its player for the hook.
    unsafe {
        let (Some(model), Some(hit), Some(frame)) = (site::model(ctx), site::hit(ctx), site::frame(ctx)) else { return };
        if read(hit.kind()) != EXPLOSIVE || !hit.item().is_some_and(|item| grenade_set(item).is_some()) {
            return;
        }
        let Some(victim) = model.owner() else { return };
        let factor = skills(victim).friendly_blast();
        let attacker = read(hit.attacker());
        if factor == 1.0 || attacker == NO_PLACE {
            return;
        }
        let friendly = attacker == read(victim.place()) || {
            let player = super::medical::Player::new(ctx.thread(), victim);
            player.at_place(ctx.thread(), attacker).is_some_and(|thrower| player.ally(thrower.get()))
        };
        if friendly {
            *frame.amount() = read(frame.amount()) * factor;
        }
    }
}

/// Bear Trap (444) where a hit's wound is stored (`medical.wound`): a leg caught by a trap whose
/// owner has the skill is left with no health, whatever its armour and its maximum (the native
/// thresholds then break it: the stock trap leaves a leg of more than its damage unbroken).
pub fn bear_trap(ctx: &mut Context<'_>) {
    use raw::site::medical_wound as site;
    // SAFETY: hit_by_type's live pool and hit; the hit's item and its inventory's holder live
    // for the hook; the pool's name is a native terminated string.
    let broken = unsafe {
        let (Some(pool), Some(hit)) = (site::pool(ctx), site::hit(ctx)) else { return };
        let Some(set) = hit.item().and_then(|item| trap_set(item)) else { return };
        let name = pool.name() as *const u8;
        let leg = [c"left_leg", c"right_leg"]
            .iter()
            .any(|leg| super::medical::name_equal(name, leg.as_ptr().cast()));
        leg && set
            .item()
            .inventory()
            .and_then(|inventory| inventory.holder())
            .is_some_and(|holder| skills(holder.player()).bear_trap())
    };
    if broken {
        ctx.set_xmm_f32(0, 0.0);
    }
}

/// Scavenger (443) at a kill (`player.killed_event`): a kill by the killer's own grenade (the kill's
/// source is the killer's stack) gives its stack one unit back, for an enemy only. The objects go
/// by the count (pull_pin arms grenades[count] after taking the unit), so the unit comes back only
/// when grenades[count] is free: never armed or spent and out of the world, or the very grenade
/// exploding at the kill's time. Otherwise (another of its grenades still live) no unit.
pub fn scavenger(ctx: &Context<'_>) {
    use raw::site::player_killed_event as site;
    // SAFETY: generate_killed_event's live victim and frame; the source item and its inventory,
    // the killer from the roster (held for the hook); the stack's own objects.
    unsafe {
        let (Some(victim), Some(frame)) = (site::victim(ctx), site::frame(ctx)) else { return };
        let killer_place = read(frame.killer());
        if killer_place == NO_PLACE || killer_place == read(victim.place()) {
            return;
        }
        let Some(set) = frame.source().and_then(|item| grenade_set(item)) else { return };
        let Some(inventory) = set.item().inventory() else { return };
        let Some(holder) = inventory.holder() else { return };
        let thrower = holder.player();
        let victim_view = super::medical::Player::new(ctx.thread(), victim);
        let Some(killer) = victim_view.at_place(ctx.thread(), killer_place) else { return };
        if killer.get().get() != thrower || victim_view.ally(killer.get()) || !skills(thrower).scavenger() {
            return;
        }
        let Some(first) = set.grenades() else { return };
        let objects = set.grenades_end().and_then(|end| end.index_from(first)).unwrap_or(0);
        let count = read(set.amount());
        if usize::from(count) >= objects {
            return;
        }
        let Some(next) = first.at(usize::from(count)).grenade() else { return };
        let explode_time = read(next.explode_time());
        let free = (read(next.physics()) == 0 && explode_time == u32::MAX) || explode_time == read(frame.time());
        if free {
            *set.amount() = count + 1;
            *inventory.weight_dirty() = 1;
        }
    }
}

/// `item` as a trap stack, none for another item.
///
/// # Safety
/// `item` is a live item.
unsafe fn trap_set(item: raw::Item) -> Option<raw::BoobyTrapSet> {
    // SAFETY: an item's own vtable word; the cast only after it is a trap set's.
    unsafe {
        let table = read(item.vtable());
        (table == raw::image::TRAP_SET_VTABLE || table == raw::image::TRAP_SET_CORE_VTABLE).then(|| item.trap_set())
    }
}

/// A trap stack's trap objects (one a unit of the kit slot's count).
///
/// # Safety
/// `set` is a live trap stack.
unsafe fn trap_objects(set: raw::BoobyTrapSet) -> u16 {
    // SAFETY: the stack's own vector words.
    let bytes = unsafe { read(set.traps_end()).wrapping_sub(read(set.traps())) };
    u16::try_from(bytes / 4).unwrap_or(0)
}

/// Trophy Trap (423) at a finished defuse (`technical.trap_defused`): a disarmer with the skill gets
/// one trap into its own trap stack, if it has room (its spare, or a unit it placed), from its own
/// trap or an enemy's, never an ally's; the trap goes off the map at once, as one whose set has no
/// disarmed life (defuse_completed's own remove). The site is after defuse_completed: a trap that
/// goes off at the defuse (the owner's modifier 46) never reaches it.
pub fn trophy_trap(ctx: &Context<'_>) {
    use raw::site::technical_trap_defused as site;
    // SAFETY: use_execute's live user data and disarmer, the trap and its set, the set's inventory
    // and owner; the disarmer's inventory and its items.
    unsafe {
        let (Some(user), Some(set), Some(usable)) = (site::user(ctx), site::set(ctx), site::usable(ctx)) else {
            return;
        };
        let Some(disarmer) = user.player() else { return };
        if !skills(disarmer).trophy_trap() {
            return;
        }
        let Some(owner) = set.item().inventory().and_then(|inventory| inventory.holder()).map(|holder| holder.player())
        else {
            return;
        };
        let (disarmer_view, owner_view) =
            (super::medical::Player::new(ctx.thread(), disarmer), super::medical::Player::new(ctx.thread(), owner));
        if disarmer != owner && disarmer_view.ally(owner_view) {
            return;
        }
        let Some(inventory) = disarmer.inventory() else { return };
        let first = inventory.slots();
        let count = inventory.slots_end().index_from(first).unwrap_or(0);
        let Some(own) = (0..count).find_map(|slot| first.at(slot).item().and_then(|item| trap_set(item))) else { return };
        let amount = read(own.amount());
        if amount < trap_objects(own) {
            *own.amount() = amount + 1;
            *inventory.weight_dirty() = 1;
            // As defuse_completed's own remove: no state timer left, which would remove the object
            // again once a placing took it.
            let trap = usable.trap();
            *trap.state_timer() = 0;
            raw::engine_trap_remove(Some(trap));
        }
    }
}
