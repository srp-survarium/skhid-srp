//! The artefact tree's native sites (patch 109, artefact-skills.md): the kit's packed skills
//! (modifier 40) of the player a site's object belongs to, and Resonance's and Cascade's aura:
//! what a recipient's living teammates' artefacts give it where the effect is used, from the
//! world as the game has it at that moment; nothing kept.
use super::{
    Context, Lock, Reg, Thread,
    medical::{Player, kit_modifiers, name_equal},
    types::{GameTime, Place, Tick},
};
use srp_validate::{Places as Reached, world::ArtefactWear};
use crate::{
    artefact_skills::{
        Copies, HitKind, Kind, LifeboneRegeneration, MOVEMENT_LIST, OnyxProtection, Phase, Regeneration, Room, SPRING_LISTS,
        Skills, State,
    },
    raw,
};

/// The packed skills of `player`'s current kit; none when the payload is not valid.
pub(super) fn skills(player: raw::Player) -> Skills {
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
pub(super) fn kind_of(artefact: raw::Artefact) -> Option<Kind> {
    // SAFETY: a carried artefact's own virtual table.
    let enable_active = unsafe { *artefact.vtable()?.enable_active() };
    match enable_active {
        raw::image::SPRING_ACTIVE => Some(Kind::Spring),
        raw::image::LIFEBONE_ACTIVE => Some(Kind::Lifebone),
        raw::image::ONYX_ACTIVE => Some(Kind::Onyx),
        _ => None,
    }
}

/// A copy at its activation (artefact_base::action's hook, after its enable_active): its kind and
/// its active's milliseconds as that enable_active set them; none for a kind Cascade does not
/// share (the Rattle).
pub(super) fn activated(artefact: raw::Artefact) -> Option<(Kind, u32)> {
    let kind = kind_of(artefact)?;
    // SAFETY: the activating copy of that kind (its vtable checked), its own field.
    let ms = unsafe {
        match kind {
            Kind::Spring => *artefact.spring().active_left(),
            Kind::Lifebone => *artefact.lifebone().active_left(),
            Kind::Onyx => *artefact.onyx().active_left(),
        }
    };
    Some((kind, ms))
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

/// One of a player's own copies of a kind and whether it counts for it.
#[derive(Clone, Copy)]
struct Own {
    copy: Carried,
    counts: bool,
}

/// `player`'s own copies of `kind` in their passive or active, in container order, each with
/// whether it counts for it ([`Room::counts_own`]: the first 4; one past them gives nothing). The
/// one count of own copies: the sharing's room and what each carrier shares, the leg quarters, the
/// cap where each passive is applied and the press's.
fn own(player: raw::Player, kind: Kind) -> impl Iterator<Item = Own> {
    carried(player)
        .filter(move |copy| copy.kind == kind && copy.state != State::Other)
        .enumerate()
        .map(|(index, copy)| Own { copy, counts: Room::counts_own(index) })
}

/// The player whose inventory holds `artefact`; none for one in no inventory.
fn holder(artefact: raw::Artefact) -> Option<raw::Player> {
    // SAFETY: a live artefact's inventory and its holder, whose interface is its base_player's
    // (+108h).
    unsafe { artefact.item().inventory().and_then(|inventory| inventory.holder()).map(|holder| holder.player()) }
}

/// Whether `artefact` is one of its holder's own copies past the 4 of its kind that count: it
/// then gives nothing, its passive nor its active. The holder is its inventory's (an Onyx's protector and a
/// Lifebone's regeneration subscriber are in their holder's parts only while it carries them);
/// an artefact in no inventory, or in no passive nor active, counts as the game has it.
pub(super) fn uncounted(artefact: raw::Artefact) -> bool {
    let (Some(player), Some(kind)) = (holder(artefact), kind_of(artefact)) else {
        return false;
    };
    own(player, kind).any(|own| own.copy.at == artefact && !own.counts)
}

/// Onyx's protect (0x312BF0, ECX the Onyx): whether it is in its passive and past the 4 Onyx that
/// count for its holder, leaving the hit as it is.
pub fn onyx_uncounted(ctx: &Context<'_>) -> bool {
    // SAFETY: the protect's ECX is its live Onyx.
    let Some(onyx) = (unsafe { raw::site::artefact_own_onyx::onyx(ctx) }) else {
        return false;
    };
    // SAFETY: the Onyx's own state.
    State::parse(unsafe { *onyx.state() }) == State::Passive && uncounted(onyx)
}

/// A Lifebone's regeneration modifier (0x312590, ECX the Lifebone): whether it is past the 4
/// Lifebones that count for its holder, leaving the part's tick as it is.
pub fn lifebone_uncounted(ctx: &Context<'_>) -> bool {
    // SAFETY: the subscriber's invoker calls it on its live Lifebone.
    unsafe {
        use raw::site::artefact_own_lifebone as site;
        let Some(item) = site::lifebone(ctx) else { return false };
        if uncounted(item) { return true; }
        let power = wear(item).power();
        if power == 1.0 { return false; }
        let Some(args) = site::frame(ctx) else { return false };
        let (Some(tick), Some(record)) = (args.tick(), args.record()) else { return false };
        let scaled = regeneration_values(record).at_power(Regeneration {
            rate: *tick.rate(), delay_ms: *tick.delay(), limit: *tick.limit()
        }, power);
        *tick.rate() = scaled.rate;
        *tick.delay() = scaled.delay_ms;
        *tick.limit() = scaled.limit;
        true
    }
}

/// Patch 54's press (artefacts_026g.asm `ready`) for a copy it would fire: one past the 4 of its
/// kind that count for its holder does not fire (its fires 0), so it stays ready and its active
/// adds nothing (Spring's active nodes, Lifebone's heal, Onyx's second more for each fired).
pub fn fire(ctx: &Context<'_>) {
    use raw::site::artefact_fire as site;
    // SAFETY: the press's frame: a live copy in its holder's containers, its fires.
    unsafe {
        let Some(frame) = site::frame(ctx) else {
            return;
        };
        if frame.artefact().is_some_and(uncounted) {
            *frame.fires() = 0;
        }
    }
}

/// Whether one of `player`'s artefact containers holds an active Onyx.
fn active_onyx(player: raw::Player) -> bool {
    carried(player).any(|copy| copy.kind == Kind::Onyx && copy.state == State::Active)
}

/// Onyx II: the landing of a player does no damage while an Onyx of a holder with the skill is
/// active for it: one of its own, or one a teammate's Cascade gives it (the carrier's skill goes
/// with the copy, as Spring II's reload nodes do).
fn landing_power(thread: &Thread, player: raw::Player) -> f32 {
    let own = if skills(player).onyx() {
        carried(player).find(|copy| copy.kind == Kind::Onyx && copy.state == State::Active)
    } else { None };
    if let Some(own) = own { return wear(own.at).firing().power(); }
    // SAFETY: the landing player's live world.
    let roster = Roster::of(thread, unsafe { player.world_core() });
    let (_, actives) = shared(roster, Player::new(thread, player), Kind::Onyx);
    actives.iter().find(|&onyx| holder(onyx).is_some_and(|carrier| skills(carrier).onyx()))
        .map_or(0.0, |onyx| wear(onyx).firing().power())
}

pub fn lands_unhurt(ctx: &Context<'_>) -> bool {
    // SAFETY: the landing's ECX is its live player.
    unsafe { raw::site::artefact_landing::player(ctx) }.is_some_and(|player| landing_power(ctx.thread(), player) == 1.0)
}

/// Larkspur II: a Larkspur's actual activation (never a restore) fills its holder's stamina, and
/// that of each teammate the activation reached (Cascade's, Medic's share just before this: the
/// places it healed).
pub fn larkspur(ctx: &Context<'_>) {
    use raw::site::medical_artefact as site;
    // SAFETY: the actual activation owns this item.
    let artefact = unsafe {
        let Some(item) = site::item(ctx) else {
            return;
        };
        item.item().artefact()
    };
    if kind_of(artefact) != Some(Kind::Lifebone) {
        return;
    }
    let Some(player) = holder(artefact).filter(|&player| skills(player).larkspur()) else {
        return;
    };
    let reached = reached(ctx.thread(), artefact);
    // SAFETY: the live holder's world core (0 when it has none).
    let roster = Roster::of(ctx.thread(), unsafe { player.world_core() });
    let teammates = roster.iter().map(Player::get).filter(|&teammate| {
        // SAFETY: a roster player's own place.
        srp_validate::Place::new(unsafe { *teammate.place() }).is_some_and(|place| reached.contains(place))
    });
    for filled in core::iter::once(player).chain(teammates) {
        // SAFETY: a live player's embedded stamina.
        unsafe {
            let stamina = filled.stamina();
            // A worn copy gives its power's share of the whole bar, never past full.
            let full = *stamina.maximum();
            *stamina.current() = (*stamina.current() + full * wear(artefact).firing().power()).min(full);
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

/// A world's places as core_active_player (0x2D0AB0) reads them: the places' vector and the bit of
/// each place in the world.
#[derive(Clone, Copy)]
struct Places {
    first: raw::CorePlace,
    count: usize,
    active: u32,
}

/// A world's active roster: the players core_active_player gives, read where they lie and
/// borrowed for the hook. The aura is read at every modifier getter's call, so it takes no
/// reference (core_active_player adds one, and its release is an atomic each): the hook runs no
/// game code, so no player it reads is released while it runs.
#[derive(Clone, Copy)]
struct Roster<'t> {
    thread: &'t Thread,
    /// None without a world core (a lobby's player).
    places: Option<Places>,
}

impl<'t> Roster<'t> {
    fn of(thread: &'t Thread, core: Option<raw::CoreWorld>) -> Self {
        // SAFETY: a live world core's places vector and the bits of its places in the world.
        let places = core.and_then(|core| unsafe {
            let (first, end) = (core.places()?, core.places_end()?);
            Some(Places { first, count: end.index_from(first)?, active: *core.active_places() })
        });
        Self { thread, places }
    }

    /// Its players in place order, each as core_active_player gives the place's (places 0..20 as
    /// the roster asked them one by one): none past the places' vector, for a place not in the
    /// world (its bit clear: a departed one), or without a player of its kit.
    fn iter(self) -> impl Iterator<Item = Player<'t>> {
        let (first, count, active) = self.places.map_or((None, 0, 0), |places| (Some(places.first), places.count, places.active));
        (0..count.min(usize::from(PLACES))).filter(move |&place| active & (1 << place) != 0).filter_map(move |place| {
            // SAFETY: a place below the vector's end: its kit and that kit's player, the array
            // core_active_player indexes by it.
            let at = unsafe {
                let entry = first?.at(place);
                entry.kits().at(usize::from(*entry.kit())).player()?
            };
            Some(Player::new(self.thread, at))
        })
    }
}

/// The match's world core: the engine world's game's game world's match (a live or a replay
/// match alike; never the network client's own field, which a replay client keeps elsewhere,
/// ragdoll.md section 16), none in the lobby (the world's match 0) or before the engine runs.
pub(in crate::vostok) fn match_core() -> Option<raw::CoreWorld> {
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

    /// Whether it still takes a copy.
    fn open(&self) -> bool {
        self.room.open()
    }

    fn iter(&self) -> impl Iterator<Item = raw::Artefact> + '_ {
        self.copies.iter().flatten().copied()
    }
}

/// A copy of a world, by its registry id (an id is another copy in another match's world).
#[derive(Clone, Copy, PartialEq, Eq)]
struct CopyKey {
    world: u32,
    item: u32,
}

/// The places one activation of a copy reached.
#[derive(Clone, Copy)]
struct Activation {
    key: CopyKey,
    reached: Reached,
}

/// Each copy's last activation, by its registry id (`artefact.id`, below 255): the places of the
/// teammates Cascade gave its active to (the owner, 2026-11-11: "they should get full effect"),
/// read only while the copy is active (state 4). Every activation of a kind the tree shares sets
/// it (none reached without Cascade); a world's save writes it with each active copy and its load
/// reads it back (`carry.ground_save`, `carry.ground_load`), so a rollback and a late joiner have the
/// activation's own.
static ACTIVATIONS: Lock<[Option<Activation>; 255]> = Lock::new([None; 255]);

/// `artefact`'s registry slot and key; none without an id or a world.
fn copy_key(artefact: raw::Artefact) -> Option<(usize, CopyKey)> {
    // SAFETY: a live artefact's own id and its registered world.
    unsafe {
        let id = *artefact.id();
        let world = artefact.item().world()?;
        (id != u8::MAX).then_some((usize::from(id), CopyKey { world: world.address(), item: artefact.address() }))
    }
}

fn set_reached(thread: &Thread, artefact: raw::Artefact, reached: Option<Reached>) {
    let Some((slot, key)) = copy_key(artefact) else {
        return;
    };
    ACTIVATIONS.with(thread, |all| all[slot] = reached.map(|reached| Activation { key, reached }));
}

/// The places `artefact`'s activation reached (none for a copy no activation of this world set).
fn reached(thread: &Thread, artefact: raw::Artefact) -> Reached {
    let Some((slot, key)) = copy_key(artefact) else {
        return Reached::EMPTY;
    };
    ACTIVATIONS.with(thread, |all| all[slot].filter(|activation| activation.key == key).map_or(Reached::EMPTY, |a| a.reached))
}

/// A copy at its activation (Medic's path, `medical::share_active`): the places its active is
/// given to until it ends.
pub(super) fn activate(thread: &Thread, artefact: raw::Artefact, reached: Reached) {
    set_reached(thread, artefact, Some(reached));
}

/// The bytes an active copy saves (artefact_base::serialize after its cooldown, 0x313735): the
/// places its activation reached, a u32 of their bits ([`Reached`]); none in another state.
pub fn save(ctx: &Context<'_>) {
    use raw::site::carry_ground_save as site;
    // SAFETY: artefact_base::serialize's live artefact and writer.
    let (Some(item), Some(writer)) = (unsafe { site::item(ctx) }, unsafe { site::writer(ctx) }) else {
        return;
    };
    // SAFETY: the artefact's own state.
    if State::parse(unsafe { *item.state() }) == State::Active {
        super::carry_runtime::write_bytes(writer, &reached(ctx.thread(), item).bits().to_le_bytes());
    }
}

/// As [`save`] at artefact_base::deserialize (0x3137EC, its state just read): an active copy's
/// places read back; another state has none.
pub fn load(ctx: &Context<'_>) {
    use raw::site::carry_ground_load as site;
    // SAFETY: artefact_base::deserialize's live artefact.
    let Some(item) = (unsafe { site::item(ctx) }) else {
        return;
    };
    // SAFETY: the state deserialize just read.
    let reached = if State::parse(unsafe { *item.state() }) == State::Active {
        // SAFETY: artefact_base::deserialize's live reader.
        let bytes = unsafe { site::reader(ctx) }.and_then(super::carry_runtime::read_bytes::<4>);
        // The world check (patch 87) refused a world with a place past the 20 before its load.
        Some(bytes.and_then(|bytes| Reached::new(u32::from_le_bytes(bytes))).unwrap_or(Reached::EMPTY))
    } else {
        None
    };
    set_reached(ctx.thread(), item, reached);
}

/// What `recipient` gets of `kind` from its teammates: their copies' passives (Resonance, within
/// 9 m now) and actives (Cascade: each active copy whose activation reached it, within 6 m then,
/// to the active's end). A carrier is another living player of its team in the roster; the copies
/// come in place order, each carrier's that count for it ([`own`]) in container order, as many of
/// each phase as the recipient's room after its own copies of the kind that count (passives and
/// actives alike: 4 less them). A dead recipient gets none.
///
/// Read at every getter's call, so it stops as soon as the rest cannot change it: with no room
/// left (4 own copies), at a carrier without the skills (before its distance), and once both
/// rooms are full.
fn shared(roster: Roster<'_>, recipient: Player<'_>, kind: Kind) -> (Shared, Shared) {
    let none = Shared::new(Room::NONE);
    // SAFETY: the live recipient's own place.
    let place = srp_validate::Place::new(unsafe { *recipient.get().place() });
    let Some(place) = place.filter(|_| recipient.active()) else {
        return (none, none);
    };
    let counted = own(recipient.get(), kind).filter(|own| own.counts).count();
    let (mut passives, mut actives) = (Shared::new(Room::after(counted)), Shared::new(Room::after(counted)));
    let ally = recipient.allies();
    for carrier in roster.iter() {
        if !passives.open() && !actives.open() {
            break;
        }
        if !carrier.active() || !ally(carrier) {
            continue;
        }
        let skills = skills(carrier.get());
        let (resonance, cascade) = (skills.reach(Phase::Passive), skills.reach(Phase::Active).is_some());
        if resonance.is_none() && !cascade {
            continue;
        }
        let passive = resonance.is_some_and(|reach| carrier.distance_squared(recipient) <= reach);
        for copy in own(carrier.get(), kind).filter(|own| own.counts).map(|own| own.copy) {
            if passive && kind.gives(Phase::Passive, copy.state) {
                passives.offer(copy.at);
            }
            if cascade && kind.gives(Phase::Active, copy.state) && reached(roster.thread, copy.at).contains(place) {
                actives.offer(copy.at);
            }
        }
    }
    (passives, actives)
}

impl super::world::Player<'_> {
    /// Its passive copies of each kind that count for it (the leg quarters, patch 111): its own
    /// copies that count and give their passive and the passives Resonance shares with it, counted
    /// with Resonance's own sharing (`shared`), in its world's roster as it is now; none when it
    /// is not in that roster or is dead.
    pub fn passive_copies(&self) -> Copies {
        // SAFETY: the live player's world core (0 when it has none).
        let core = unsafe { self.this.world_core() };
        let roster = Roster::of(self.thread, core);
        let Some(recipient) = roster.iter().find(|player| player.get() == self.this).filter(|player| player.active())
        else {
            return Copies::default();
        };
        Copies::of(|kind| {
            let own = own(self.this, kind).filter(|own| own.counts && kind.gives(Phase::Passive, own.copy.state)).count();
            let (passives, _) = shared(roster, recipient, kind);
            (own, passives.iter().count())
        })
    }
}

/// A shared Spring's nodes of the kit list `index` for `phase`, summed: its passive's +340h,
/// +350h and +3D8h, its active's +360h, +370h, +380h, +3B8h, +3C8h and +3E8h (each its index).
fn spring_nodes(spring: raw::Artefact, phase: Phase, index: u32) -> f32 {
    let strength = phase_wear(spring, phase).power();
    let (passive, active) = spring_phase_nodes(spring);
    let nodes: &[raw::ModifierNode] = match phase {
        Phase::Passive => &passive,
        Phase::Active => &active,
    };
    // SAFETY: the Spring's own nodes.
    nodes.iter().filter(|node| unsafe { *node.index() } == index).fold(0.0, |sum, node| sum + spring_value(index, unsafe { *node.value() }, strength))
}

/// A carried Spring's nodes: its passive's (+340h, +350h, +3D8h) and its active's (+360h, +370h,
/// +380h, +3B8h, +3C8h, +3E8h).
fn spring_phase_nodes(spring: raw::Artefact) -> ([raw::ModifierNode; 3], [raw::ModifierNode; 6]) {
    // SAFETY: a carried Spring (its vtable checked), its config's nodes and patch 54's.
    unsafe {
        let spring = spring.spring();
        let (first, added) = (spring.first_node(), spring.added_node());
        ([first.at(0), first.at(1), added.at(2)], [first.at(2), first.at(3), first.at(4), added.at(0), added.at(1), added.at(3)])
    }
}

/// A kit list's nodes in its order.
fn list_nodes(list: raw::ModifierList) -> impl Iterator<Item = raw::ModifierNode> {
    // SAFETY: a live kit's list and its linked nodes.
    core::iter::successors(unsafe { list.first() }, |node| unsafe { node.next() })
}

/// The artefact containers a player has at most (quick slots 13..19).
const CONTAINERS: usize = 7;

fn phase_wear(item: raw::Artefact, phase: Phase) -> ArtefactWear {
    match phase {
        Phase::Passive => wear(item),
        Phase::Active => wear(item).firing(),
    }
}

fn spring_value(index: u32, value: f32, power: f32) -> f32 {
    // Spending is floored at zero by the game. A worn copy gives a fraction
    // of the original complete saving, even though the config specifies -5.
    if power == 1.0 { value } else if index == 19 { value.max(-1.0) * power } else { value * power }
}

struct SpringNodes {
    passive: [raw::ModifierNode; 3],
    active: [raw::ModifierNode; 6],
    power: [f32; 2],
}

impl SpringNodes {
    fn changed(own: Own) -> Option<Self> {
        let wear = wear(own.copy.at);
        if own.counts && wear == ArtefactWear::FRESH { return None; }
        let (passive, active) = spring_phase_nodes(own.copy.at);
        let power = if own.counts { [wear.power(), wear.firing().power()] } else { [0.0; 2] };
        Some(Self { passive, active, power })
    }

    fn strength(&self, node: raw::ModifierNode) -> Option<f32> {
        if self.passive.contains(&node) { Some(self.power[0]) }
        else if self.active.contains(&node) { Some(self.power[1]) }
        else { None }
    }
}

/// What the getters sum the kit list `list` (of the Spring list `index`) to without its player's
/// own Springs past the 4 that count (all their nodes left out), in the list's order as the game
/// sums it (value + sum): none when the game's sum is the rule's, so the game sums it. An empty
/// list has nothing to leave out; outside a match, for a kit that is no player's current one, or
/// when every own Spring of its player counts, nothing is left out.
fn own_springs(thread: &Thread, list: raw::ModifierList, index: u32) -> Option<f32> {
    list_nodes(list).next()?;
    let roster = Roster::of(thread, Some(match_core()?));
    let player = roster.iter().find(|player| {
        // SAFETY: a kit's lists, one for each of its 68 indices (the Spring lists among them).
        kit_modifiers(player.get()).is_some_and(|modifiers| unsafe { modifiers.lists().at(index as usize) } == list)
    })?;
    let mut changed = own(player.get(), Kind::Spring).filter_map(SpringNodes::changed);
    let first = changed.next()?;
    let mut springs = [const { None }; CONTAINERS];
    springs[0] = Some(first);
    for (slot, spring) in springs[1..].iter_mut().zip(changed) { *slot = Some(spring); }
    Some(list_nodes(list).fold(0.0, |sum, node| {
        // SAFETY: the list owns this live modifier node.
        let value = unsafe { *node.value() };
        let power = springs.iter().flatten().find_map(|spring| spring.strength(node)).unwrap_or(1.0);
        spring_value(index, value, power) + sum
    }))
}

/// apply_modifier before its list's sum (0x2D5B0C, EDI the list of the index ESI): the sum
/// without the own Springs past the 4 that count, none when nothing is left out.
pub fn own_modifier(ctx: &mut Context<'_>) -> bool {
    use raw::site::artefact_own_modifier as site;
    // SAFETY: the getter's own registers.
    let (list, index) = unsafe { (site::list(ctx), site::index(ctx)) };
    if !SPRING_LISTS.contains(&index) {
        return false;
    }
    let Some(sum) = list.and_then(|list| own_springs(ctx.thread(), list, index)) else {
        return false;
    };
    // The loop's end as the game's: EAX its last node's next (0), XMM1 the sum, XMM2 the 0.
    ctx.set_reg(Reg::Eax, 0);
    ctx.set_xmm_f32(1, sum);
    ctx.set_xmm_f32(2, 0.0);
    true
}

/// apply_modifiers' loop before one index's sum (0x2D5C46, EBX the kit's modifiers, EDI the list
/// of one of their indices): the same, XMM2 the sum. The index is the one whose list EDI is
/// (EAX is no longer it on the path through the sort 0x2D59C0); a list of no Spring index is left
/// to the game before anything else is read.
pub fn own_modifiers(ctx: &mut Context<'_>) -> bool {
    use raw::site::artefact_own_modifiers as site;
    // SAFETY: the getter's own registers: the live kit's modifiers and one of their lists.
    let (Some(modifiers), Some(list)) = (unsafe { (site::modifiers(ctx), site::list(ctx)) }) else {
        return false;
    };
    // SAFETY: a kit's lists, one for each of its 68 indices (the Spring lists among them).
    let Some(&index) = SPRING_LISTS.iter().find(|&&index| unsafe { modifiers.lists().at(index as usize) } == list) else {
        return false;
    };
    let Some(sum) = own_springs(ctx.thread(), list, index) else {
        return false;
    };
    ctx.set_xmm_f32(2, sum);
    true
}

/// What the shared Springs' nodes add to the kit list `index` of `modifiers`, when the kit is
/// the current kit of a player of the match (none in the lobby, or for another kit).
fn shared_springs(thread: &Thread, modifiers: raw::PlayerParamsModifiersContainer, index: u32) -> Option<f32> {
    let roster = Roster::of(thread, Some(match_core()?));
    let recipient = roster.iter().find(|player| kit_modifiers(player.get()) == Some(modifiers))?;
    let (passives, actives) = shared(roster, recipient, Kind::Spring);
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
        Some(onyx_values(onyx, phase))
    }
}

fn onyx_values(onyx: raw::SkillOnyx, phase: Phase) -> OnyxProtection {
    // SAFETY: both callers hold a checked Onyx object at its protect callback.
    unsafe {
        match phase {
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
        }
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
        let (passives, actives) = shared(roster, Player::new(ctx.thread(), recipient), Kind::Onyx);
        let (kind, part) = (*hit.kind(), pool.name() as *const u8);
        let own_active = active_onyx(recipient);
        let protections = passives
            .iter()
            .filter_map(|onyx| onyx_protection(onyx, Phase::Passive, kind, part).map(|p| (p, phase_wear(onyx, Phase::Passive).power())))
            .chain(
                actives
                    .iter()
                    .filter(|_| !own_active)
                    .take(1)
                    .filter_map(|onyx| onyx_protection(onyx, Phase::Active, kind, part).map(|p| (p, phase_wear(onyx, Phase::Active).power()))),
            );
        let mut result = None;
        for (protection, power) in protections {
            let (d, p) = result.unwrap_or((*damage.value(), *piercing.value()));
            result = Some(protection.at_power(power, d, p));
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
        Some(regeneration_values(record))
    }
}

fn regeneration_values(record: raw::LifeboneRegeneration) -> LifeboneRegeneration {
    // SAFETY: the Lifebone owns this live regeneration record.
    unsafe {
        LifeboneRegeneration {
            rate_add: *record.rate_add(),
            rate_mul: *record.rate_mul(),
            delay_add: *record.delay_add(),
            delay_mul: *record.delay_mul(),
            limit_add: *record.limit_add(),
            limit_mul: *record.limit_mul(),
        }
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
        let (passives, _) = shared(roster, Player::new(ctx.thread(), recipient), Kind::Lifebone);
        let part = pool.name() as *const u8;
        let mut tick = None;
        for (record, power) in passives.iter().filter_map(|item| lifebone_regeneration(item, part).map(|r| (r, wear(item).power()))) {
            let native = Regeneration { rate: *frame.rate(), delay_ms: delay, limit: *frame.limit() };
            tick = Some(record.at_power(tick.unwrap_or(native), power));
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


pub(super) fn wear(item: raw::Artefact) -> ArtefactWear {
    // SAFETY: the live artefact base owns the initialized use count.
    ArtefactWear::new(unsafe { *item.uses_spent() })
}

fn set_wear(item: raw::Artefact, wear: ArtefactWear) {
    // SAFETY: each caller holds the live artefact in its simulation or load hook.
    unsafe { *item.uses_spent() = wear.uses() };
}

pub fn wear_init(ctx: &mut Context<'_>) {
    // SAFETY: the site holds the live artefact base.
    if let Some(item) = unsafe { raw::site::artefact_wear_init::item(ctx) } {
        set_wear(item, ArtefactWear::FRESH);
    }
}

pub fn wear_spawn(ctx: &mut Context<'_>) {
    // SAFETY: the site holds the live artefact base.
    if let Some(item) = unsafe { raw::site::artefact_wear_spawn::item(ctx) } {
        set_wear(item, ArtefactWear::FRESH);
    }
}

pub fn wear_save(ctx: &mut Context<'_>) {
    use raw::site::artefact_wear_save as site;
    // SAFETY: the serializer holds this live artefact and the writer for its state.
    let (Some(item), Some(writer)) = (unsafe { site::item(ctx) }, unsafe { site::writer(ctx) }) else { return };
    let wear = wear(item);
    let state = ctx.reg(Reg::Eax) as u8;
    if let Some(byte) = wear.inline(state) {
        ctx.set_reg8(Reg::Eax, byte);
    } else {
        let [lo, hi] = wear.uses().to_le_bytes();
        super::carry_runtime::write_bytes(writer, &[ArtefactWear::EXTENDED, lo, hi]);
        // The native writer appends the ordinary state byte after the extension.
    }
}

pub fn wear_load(ctx: &mut Context<'_>) {
    use raw::site::artefact_wear_load as site;
    // SAFETY: the deserializer holds the live artefact and its reader just after the first byte.
    let Some(item) = (unsafe { site::item(ctx) }) else { return };
    let byte = ctx.reg(Reg::Eax) as u8;
    let (state, wear) = if byte == ArtefactWear::EXTENDED {
        let Some([lo, hi, state]) = (unsafe { site::reader(ctx) }).and_then(super::carry_runtime::read_bytes) else { return };
        (state, ArtefactWear::new(u16::from_le_bytes([lo, hi])))
    } else { ArtefactWear::from_inline(byte) };
    set_wear(item, wear);
    ctx.set_reg(Reg::Eax, u32::from(state));
}

pub fn wear_use(ctx: &mut Context<'_>) {
    // SAFETY: the site holds the live artefact base.
    if let Some(item) = unsafe { raw::site::artefact_wear_use::item(ctx) }
        && kind_of(item).is_some() {
        set_wear(item, wear(item).after_use());
    }
}

pub fn onyx_passive(ctx: &Context<'_>) -> bool {
    use raw::site::artefact_onyx_passive as site;
    // SAFETY: protect owns the item and hit arguments at this site.
    unsafe { apply_onyx(site::item(ctx), site::frame(ctx), Phase::Passive) }
}

pub fn onyx_active(ctx: &Context<'_>) -> bool {
    use raw::site::artefact_onyx_active as site;
    // SAFETY: protect owns the item and hit arguments at this site.
    unsafe { apply_onyx(site::item(ctx), site::frame(ctx), Phase::Active) }
}

fn apply_onyx(item: Option<raw::Artefact>, frame: Option<raw::OnyxHitArgs>, phase: Phase) -> bool {
    let (Some(item), Some(frame)) = (item, frame) else { return false };
    let power = phase_wear(item, phase).power();
    if power == 1.0 { return false; }
    // SAFETY: the caller holds the live Onyx and the two floats of its protect callback.
    unsafe {
        let (Some(d), Some(p)) = (frame.damage(), frame.piercing()) else { return false };
        let values = onyx_values(item.onyx(), phase);
        (*d.value(), *p.value()) = values.at_power(power, *d.value(), *p.value());
    }
    true
}

pub fn heal_amount(ctx: &mut Context<'_>) {
    // SAFETY: patch 54 keeps its Lifebone in EDI throughout the heal loop.
    if let Some(item) = unsafe { raw::site::artefact_heal_amount::item(ctx) } {
        ctx.set_xmm_f32(0, ctx.xmm_f32(0) * wear(item).firing().power());
    }
}

pub(super) fn worn_resource(thread: &Thread, item: raw::Artefact) -> Option<raw::MedicalEffectResource> {
    crate::forest_glow::worn_glow(thread, kind_of(item)?, wear(item)).map(|kept| kept.resource())
}

pub fn wear_glow(ctx: &mut Context<'_>) -> bool {
    // SAFETY: play_effect holds its live artefact in EBX. The selected resource
    // is retained for the process and is passed to emit in ESI.
    if let Some(item) = unsafe { raw::site::artefact_wear_glow::item(ctx) }
        && let Some(resource) = worn_resource(ctx.thread(), item) {
        ctx.set_reg(Reg::Esi, resource.address());
        return true;
    }
    false
}

pub fn fall(ctx: &mut Context<'_>) {
    // SAFETY: the native landing owns its player in EDI.
    if let Some(player) = unsafe { raw::site::artefact_fall::player(ctx) } {
        ctx.set_xmm_f32(0, ctx.xmm_f32(0) * (1.0 - landing_power(ctx.thread(), player)));
    }
}

pub fn fatal_fall(ctx: &mut Context<'_>) {
    // SAFETY: the native landing owns its player in EDI.
    if let Some(player) = unsafe { raw::site::artefact_fatal_fall::player(ctx) } {
        ctx.set_xmm_f32(0, ctx.xmm_f32(0) * (1.0 - landing_power(ctx.thread(), player)));
    }
}
