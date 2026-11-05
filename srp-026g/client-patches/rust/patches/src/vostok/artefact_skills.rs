//! The artefact tree's native sites (patch 109, artefact-skills.md): the kit's packed skills
//! (modifier 40) of the player a site's object belongs to, and Resonance's and Cascade's aura:
//! what a recipient's living teammates' artefacts give it where the effect is used, from the
//! world as the game has it at that moment; nothing kept.
use super::{
    Context, Reg, Thread,
    medical::{OwnedPlayer, Player, kit_modifiers, name_equal},
    types::{GameTime, Place, Tick},
};
use crate::{
    artefact_skills::{
        HitKind, Kind, LifeboneRegeneration, MOVEMENT_LIST, OnyxProtection, Phase, Regeneration, Room, SPRING_LISTS,
        Skills, State,
    },
    raw,
};

/// The packed skills of `player`'s current kit; none when the payload is not valid.
fn skills(player: raw::Player) -> Skills {
    kit_modifiers(player)
        // SAFETY: the generated modifier field belongs to the live current kit.
        .and_then(|m| Skills::parse(unsafe { *m.artefact_ranks() }))
        .unwrap_or_default()
}

/// Resistance: XMM0 the part's summed armour for the hit's type, before the piercing's
/// subtraction (body_part_parameters::hit_by_type 0x32A083).
pub fn isolation(ctx: &mut Context<'_>) {
    use raw::site::artefact_isolation as site;
    // SAFETY: hit_by_type's pool, its model and the model's player live for the hit; EDI is
    // the hit's type there.
    let (kind, player) = unsafe {
        let kind = HitKind::parse(site::kind(ctx));
        let player = site::pool(ctx).and_then(|pool| pool.model()).and_then(|model| model.owner());
        (kind, player)
    };
    let (Some(kind), Some(player)) = (kind, player) else {
        return;
    };
    let armour = skills(player).armour(ctx.xmm_f32(0), kind);
    ctx.set_xmm_f32(0, armour);
}

/// An artefact's kind by its virtual table's enable_active (+80h).
fn kind_of(artefact: raw::Artefact) -> Option<Kind> {
    // SAFETY: a carried artefact's own virtual table.
    let enable_active = unsafe { *artefact.vtable()?.enable_active() };
    match enable_active {
        raw::image::SPRING_ACTIVE => Some(Kind::Spring),
        raw::image::LIFEBONE_ACTIVE => Some(Kind::Lifebone),
        raw::image::ONYX_ACTIVE => Some(Kind::Onyx),
        _ => None,
    }
}

/// An artefact in a player's containers, of a kind the tree reads.
#[derive(Clone, Copy)]
struct Carried {
    at: raw::Artefact,
    kind: Kind,
    state: State,
}

/// The artefacts of `player`'s containers, in their order (the Rattle and empty containers
/// left out).
fn carried(player: raw::Player) -> impl Iterator<Item = Carried> {
    // SAFETY: a hook's live player owns its inventory and the container array it points at
    // (`artefact_slot_count` entries).
    let slots = unsafe {
        player
            .inventory()
            .and_then(|inventory| Some((inventory.artefact_slots_begin()?, usize::from(*inventory.artefact_slot_count()))))
    };
    let (first, count) = match slots {
        Some((first, count)) => (Some(first), count),
        None => (None, 0),
    };
    (0..count).filter_map(move |index| {
        // SAFETY: a container below the count; each item there is an artefact.
        let at = unsafe { first?.at(index).item()?.artefact() };
        // SAFETY: the artefact's own state.
        let state = State::parse(unsafe { *at.state() });
        Some(Carried { at, kind: kind_of(at)?, state })
    })
}

/// Whether one of `player`'s artefact containers holds an active Onyx.
fn active_onyx(player: raw::Player) -> bool {
    carried(player).any(|copy| copy.kind == Kind::Onyx && copy.state == State::Active)
}

/// Onyx II: the landing of a player with the skill and an active Onyx does no damage.
pub fn lands_unhurt(ctx: &Context<'_>) -> bool {
    // SAFETY: the landing's ECX is its live player.
    let Some(player) = (unsafe { raw::site::artefact_landing::player(ctx) }) else {
        return false;
    };
    skills(player).onyx() && active_onyx(player)
}

/// Larkspur II: a Larkspur's actual activation (never a restore) fills its holder's stamina.
pub fn larkspur(ctx: &Context<'_>) {
    use raw::site::medical_artefact as site;
    // SAFETY: the actual activation owns this item, its inventory and holder; the holder's
    // stamina is embedded in it.
    unsafe {
        let Some(item) = site::item(ctx) else {
            return;
        };
        if !item.vtable().is_some_and(|table| *table.enable_active() == raw::image::LIFEBONE_ACTIVE) {
            return;
        }
        let Some(holder) = item.item().inventory().and_then(|inventory| inventory.holder()) else {
            return;
        };
        let player = holder.player();
        if skills(player).larkspur() {
            let stamina = player.stamina();
            *stamina.current() = *stamina.maximum();
        }
    }
}

/// Spring II at a Spring's passive link (patch 54's entry): its two reload nodes' values from
/// its holder's skills, before the passive's goes in.
pub fn spring_reload(ctx: &Context<'_>) {
    use raw::site::artefact_spring_reload as site;
    // SAFETY: the linking Spring, its inventory and holder are live for the link; the two
    // nodes are its own (patch 54 grew the object to 3F8h).
    unsafe {
        let Some(spring) = site::frame(ctx).and_then(|frame| frame.spring()) else {
            return;
        };
        let Some(holder) = spring.item().inventory().and_then(|inventory| inventory.holder()) else {
            return;
        };
        let (passive, active) = skills(holder.player()).spring_reload();
        *spring.passive_reload() = passive;
        *spring.active_reload() = active;
    }
}

/// The places a world has at most (core_active_player refuses past its places' vector).
const PLACES: u8 = 20;

/// A world's active roster, one reference held on each player for the hook.
struct Roster<'t> {
    players: [Option<OwnedPlayer<'t>>; PLACES as usize],
}

impl<'t> Roster<'t> {
    fn of(thread: &'t Thread, core: Option<raw::CoreWorld>) -> Self {
        Self { players: core::array::from_fn(|place| Player::in_core(thread, core, place as u8)) }
    }

    /// Its players in place order.
    fn iter(&self) -> impl Iterator<Item = Player<'t>> + '_ {
        self.players.iter().flatten().map(OwnedPlayer::get)
    }
}

/// The match's world core: the engine world's game's game world's match (a live or a replay
/// match alike; never the network client's own field, which a replay client keeps elsewhere,
/// ragdoll.md section 16), none in the lobby (the world's match 0) or before the engine runs.
fn match_core() -> Option<raw::CoreWorld> {
    // SAFETY: the image's engine world slot, the engine world, its game and the game's embedded
    // world are live while the game runs, the match while the world holds it; each pointer is 0
    // when absent.
    unsafe { Some(raw::image::engine_world().world()?.game()?.world().pvp_match()?.core()) }
}

/// The shared copies a recipient takes of one kind and phase, in the order they are offered, as
/// many as its room.
#[derive(Clone, Copy)]
struct Shared {
    room: Room,
    copies: [Option<raw::Artefact>; Room::MOST as usize],
}

impl Shared {
    fn new(room: Room) -> Self {
        Self { room, copies: [None; Room::MOST as usize] }
    }

    /// Takes `copy` if there is room for it.
    fn offer(&mut self, copy: raw::Artefact) {
        if self.room.take()
            && let Some(free) = self.copies.iter_mut().find(|copy| copy.is_none())
        {
            *free = Some(copy);
        }
    }

    fn iter(&self) -> impl Iterator<Item = raw::Artefact> + '_ {
        self.copies.iter().flatten().copied()
    }
}

/// What `recipient` gets of `kind` from its teammates: their copies' passives (Resonance, 8 m)
/// and actives (Cascade, 5 m). A carrier is another living player of its team in the roster
/// whose skill reaches it; the copies come in place order, each carrier's in container order,
/// as many of each phase as the recipient's room after its own carried copies of the kind. A
/// dead recipient gets none.
fn shared(roster: &Roster<'_>, recipient: Player<'_>, kind: Kind) -> (Shared, Shared) {
    if !recipient.active() {
        let none = Shared::new(Room::NONE);
        return (none, none);
    }
    let own = carried(recipient.get()).filter(|copy| copy.kind == kind && copy.state != State::Other).count();
    let (mut passives, mut actives) = (Shared::new(Room::after(own)), Shared::new(Room::after(own)));
    for carrier in roster.iter() {
        if !carrier.active() || !carrier.ally(recipient) {
            continue;
        }
        let skills = skills(carrier.get());
        let distance = carrier.distance_squared(recipient);
        let reaches = |phase| skills.reach(phase).is_some_and(|reach| distance <= reach);
        let (passive, active) = (reaches(Phase::Passive), reaches(Phase::Active));
        if !passive && !active {
            continue;
        }
        for copy in carried(carrier.get()).filter(|copy| copy.kind == kind) {
            if passive && kind.gives(Phase::Passive, copy.state) {
                passives.offer(copy.at);
            }
            if active && kind.gives(Phase::Active, copy.state) {
                actives.offer(copy.at);
            }
        }
    }
    (passives, actives)
}

/// A shared Spring's nodes of the kit list `index` for `phase`, summed: its passive's +340h,
/// +350h and +3D8h, its active's +360h, +370h, +380h, +3B8h, +3C8h and +3E8h (each its index).
fn spring_nodes(spring: raw::Artefact, phase: Phase, index: u32) -> f32 {
    // SAFETY: a carried Spring (its vtable checked), its config's nodes and patch 54's.
    unsafe {
        let spring = spring.spring();
        let (first, added) = (spring.first_node(), spring.added_node());
        let passive = [first.at(0), first.at(1), added.at(2)];
        let active = [first.at(2), first.at(3), first.at(4), added.at(0), added.at(1), added.at(3)];
        let nodes: &[raw::ModifierNode] = match phase {
            Phase::Passive => &passive,
            Phase::Active => &active,
        };
        nodes.iter().filter(|node| *node.index() == index).fold(0.0, |sum, node| sum + *node.value())
    }
}

/// What the shared Springs' nodes add to the kit list `index` of `modifiers`, when the kit is
/// the current kit of a player of the match (none in the lobby, or for another kit).
fn shared_springs(thread: &Thread, modifiers: raw::PlayerParamsModifiersContainer, index: u32) -> Option<f32> {
    let roster = Roster::of(thread, Some(match_core()?));
    let recipient = roster.iter().find(|player| kit_modifiers(player.get()) == Some(modifiers))?;
    let (passives, actives) = shared(&roster, recipient, Kind::Spring);
    let passive = passives.iter().fold(0.0, |sum, spring| sum + spring_nodes(spring, Phase::Passive, index));
    Some(actives.iter().fold(passive, |sum, spring| sum + spring_nodes(spring, Phase::Active, index)))
}

/// Resonance and Cascade at a kit modifier's own float (XMM0): the shared Springs' nodes of its
/// list.
fn add_shared_springs(ctx: &mut Context<'_>, modifiers: Option<raw::PlayerParamsModifiersContainer>, index: u32) {
    if !SPRING_LISTS.contains(&index) {
        return;
    }
    let Some(sum) = modifiers.and_then(|modifiers| shared_springs(ctx.thread(), modifiers, index)) else {
        return;
    };
    if sum != 0.0 {
        ctx.set_xmm_f32(0, ctx.xmm_f32(0) + sum);
    }
}

/// A player at its tick before its animations (base_player::tick, battery.carrier_tick
/// 0x2DAAD3, EBX; tick_animations 0x2D8EE0 is called right after): its place, the tick of its
/// time (+324h) and what the shared Springs add to its movement list now. Only for the hook.
pub struct MovementTick<'c> {
    player: Player<'c>,
    place: Place,
    tick: Tick,
    shared: f32,
}

impl<'c> MovementTick<'c> {
    /// The tick's player, none outside a match (or a player at no place of it).
    pub fn at(ctx: &'c Context<'_>) -> Option<Self> {
        // SAFETY: base_player::tick's EBX is its live player, its place and time its own.
        let (at, place, time) = unsafe {
            let at = raw::site::battery_carrier_tick::player(ctx)?;
            (at, *at.place(), *at.time())
        };
        let place = Place::new(u32::from(place))?;
        let shared = kit_modifiers(at).and_then(|modifiers| shared_springs(ctx.thread(), modifiers, MOVEMENT_LIST))?;
        Some(Self { player: Player::new(ctx.thread(), at), place, tick: GameTime(time).tick(), shared })
    }

    pub fn place(&self) -> Place {
        self.place
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn shared(&self) -> f32 {
        self.shared
    }

    /// Its animations selected again this tick (m_need_to_select_animations, +33Eh, a simulation
    /// field): movement_speed (0x2DBB50) is read only by select_animations (0x2D9690), which
    /// tick_animations runs only when it is set.
    pub fn reselect(self) {
        self.player.refresh();
    }
}

/// apply_modifier (0x2D5AE6): EAX the kit's modifiers, ESI the index.
pub fn shared_modifier(ctx: &mut Context<'_>) {
    use raw::site::artefact_shared_modifier as site;
    // SAFETY: the getter's own registers.
    let (modifiers, index) = unsafe { (site::modifiers(ctx), site::index(ctx)) };
    add_shared_springs(ctx, modifiers, index);
}

/// apply_modifiers' loop (0x2D5C18): EBX the kit's modifiers, EAX the index.
pub fn shared_modifiers(ctx: &mut Context<'_>) {
    use raw::site::artefact_shared_modifiers as site;
    // SAFETY: the getter's own registers.
    let (modifiers, index) = unsafe { (site::modifiers(ctx), site::index(ctx)) };
    add_shared_springs(ctx, modifiers, index);
}

/// What a shared Onyx's `phase` does to a hit of `kind` on the part named `part`: none when it
/// protects against another type or not that part (its config's parts, +0C90h).
fn onyx_protection(onyx: raw::Artefact, phase: Phase, kind: u32, part: *const u8) -> Option<OnyxProtection> {
    // SAFETY: a carried Onyx (its vtable checked), its config's numbers and parts; the part's
    // name is the live pool's C string.
    unsafe {
        let onyx = onyx.onyx();
        if *onyx.hit_kind() != kind {
            return None;
        }
        let (first, end) = (onyx.parts()?, onyx.parts_end()?);
        if !(0..end.index_from(first).unwrap_or(0)).any(|at| name_equal(first.at(at).name(), part)) {
            return None;
        }
        Some(match phase {
            Phase::Passive => OnyxProtection {
                damage_add: *onyx.passive_damage_add(),
                damage_mul: *onyx.passive_damage_mul(),
                piercing_add: *onyx.passive_piercing_add(),
                piercing_mul: *onyx.passive_piercing_mul(),
                absorb: None,
            },
            Phase::Active => OnyxProtection {
                damage_add: *onyx.active_damage_add(),
                damage_mul: *onyx.active_damage_mul(),
                piercing_add: *onyx.active_piercing_add(),
                piercing_mul: *onyx.active_piercing_mul(),
                absorb: Some(*onyx.absorb()),
            },
        })
    }
}

/// Resonance and Cascade before a part's protectors (hit_by_type 0x32A020): each shared Onyx's
/// passive, then one shared active when none of the recipient's own Onyx is active (patch 54:
/// active Onyx reduce a hit once), as the Onyx's own protect does.
pub fn shared_protection(ctx: &mut Context<'_>) {
    use raw::site::artefact_shared_protection as site;
    // SAFETY: hit_by_type's pool, its model's player, the hit and the two floats it changes
    // live for the hit.
    unsafe {
        let (Some(pool), Some(hit), Some(damage), Some(frame)) =
            (site::pool(ctx), site::hit(ctx), site::damage(ctx), site::frame(ctx))
        else {
            return;
        };
        let (Some(piercing), Some(recipient)) = (frame.piercing(), pool.model().and_then(|model| model.owner())) else {
            return;
        };
        let roster = Roster::of(ctx.thread(), recipient.world_core());
        let (passives, actives) = shared(&roster, Player::new(ctx.thread(), recipient), Kind::Onyx);
        let (kind, part) = (*hit.kind(), pool.name() as *const u8);
        let own_active = active_onyx(recipient);
        let protections = passives
            .iter()
            .filter_map(|onyx| onyx_protection(onyx, Phase::Passive, kind, part))
            .chain(
                actives
                    .iter()
                    .filter(|_| !own_active)
                    .take(1)
                    .filter_map(|onyx| onyx_protection(onyx, Phase::Active, kind, part)),
            );
        let mut result = None;
        for protection in protections {
            let (d, p) = result.unwrap_or((*damage.value(), *piercing.value()));
            result = Some(protection.protect(d, p));
        }
        if let Some((d, p)) = result {
            *damage.value() = d;
            *piercing.value() = p;
        }
    }
}

/// A shared Lifebone's record for the part named `part` (its passive's, +354h).
fn lifebone_regeneration(lifebone: raw::Artefact, part: *const u8) -> Option<LifeboneRegeneration> {
    // SAFETY: a carried Lifebone (its vtable checked) and its config's records; the part's
    // name is the live pool's C string.
    unsafe {
        let lifebone = lifebone.lifebone();
        let (first, end) = (lifebone.regeneration()?, lifebone.regeneration_end()?);
        let record = (0..end.index_from(first).unwrap_or(0)).map(|at| first.at(at)).find(|record| name_equal(record.name(), part))?;
        Some(LifeboneRegeneration {
            rate_add: *record.rate_add(),
            rate_mul: *record.rate_mul(),
            delay_add: *record.delay_add(),
            delay_mul: *record.delay_mul(),
            limit_add: *record.limit_add(),
            limit_mul: *record.limit_mul(),
        })
    }
}

/// Resonance at a part's regeneration tick (body_part::regenerate 0x32A53F, after its own
/// modifiers, before patch 94's): each shared Lifebone's record for the part, as its own
/// regeneration modifier does. EAX the delay, the frame's rate and limit.
pub fn shared_regeneration(ctx: &mut Context<'_>) {
    use raw::site::medical_regenerate as site;
    let delay = ctx.reg(Reg::Eax);
    // SAFETY: body_part::regenerate's pool, its model's player and the frame's locals live for
    // the tick.
    let delay = unsafe {
        let (Some(pool), Some(frame)) = (site::pool(ctx), site::frame(ctx)) else {
            return;
        };
        let Some(recipient) = pool.model().and_then(|model| model.owner()) else {
            return;
        };
        let roster = Roster::of(ctx.thread(), recipient.world_core());
        let (passives, _) = shared(&roster, Player::new(ctx.thread(), recipient), Kind::Lifebone);
        let part = pool.name() as *const u8;
        let mut tick = None;
        for record in passives.iter().filter_map(|lifebone| lifebone_regeneration(lifebone, part)) {
            let native = Regeneration { rate: *frame.rate(), delay_ms: delay, limit: *frame.limit() };
            tick = Some(record.apply(tick.unwrap_or(native)));
        }
        let Some(tick) = tick else {
            return;
        };
        *frame.rate() = tick.rate;
        *frame.limit() = tick.limit;
        tick.delay_ms
    };
    ctx.set_reg(Reg::Eax, delay);
}
