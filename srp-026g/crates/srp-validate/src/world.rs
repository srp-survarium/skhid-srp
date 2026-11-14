//! World context, player heads, rules and statistics. Runtime enforcement is separate.
mod bullets;
mod effects;
mod frame;
mod objects;
pub use bullets::{Bullet, BulletFault, Bullets, Lamps, WeaponSlot, bullets};
pub use effects::{
    ActivationStatus, Effect, EffectFault, Effects, PlaybackType, SerializationMode, effects,
};
pub use objects::{
    ActiveCore, AnomalyArtefacts, AnomalyState, ArtefactBase, ArtefactBases, ArtefactContainerCore,
    ArtefactContainerCores, ArtefactId, ArtefactState, BoobyTrapCore, DamageZoneCore,
    DamageZoneCores, GameObject, GameObjectIter, GameObjects, GenericAnomalyCore, GrenadeCore, Kit,
    Motion, ObjectFault, Owner, PlacedTrap, ResupplyZoneCore, Thrown, TrapState,
    UsableObject, User, ZoneGroup, ZoneGroups, ZoneMask,
};
pub use objects::{Anomaly, Artefact, Artefacts, Group, Held, Map, Objects, State, objects};
mod inventory;
pub use inventory::{
    Affect, AffectKind, AfterItems, Ammo, Body, BodyPart, Chamber, Controller, DamageModel, Gun,
    GunState, Inventory, Item, ItemBody, ItemOwner, ItemState, ItemsAndRest, Left, List,
    Measure, Medkit, MedkitUse, OxygenTank, PartHit, RecentHit, SlotId, Usable,
};
mod player_tail;
pub use player_tail::{
    ArtefactSlots, BatteryAnimation, BatteryMotion, BatteryState, CarriedBattery, Head, HitEffect,
    HitEmitter, MAX_HIT_EFFECTS, MAX_PLAYER_EFFECTS,
    InventoryHeader, PlayerState, Stamina, player_head, player_tail, player_tail_kept,
};
mod weapon;
pub use weapon::{
    AmmoSwitch, AnimationWait, Breath, BreathState, Fire, HIT_BODY_PARTS, HitBodyPart, InHand, Jump,
    JumpStep, JumpType, JumpWait, Portable, Recoil, ReloadStep, SelectorState, Shot,
    SprintAnimation, StateData, ThrowStep,
    Transition, WeaponState,
};
mod rules;
mod statistics;
use crate::{KITS, Part, Reason, Refusal, mode};
pub use frame::{Block, Frame, PLAYER_STATE_CAP, Section, frame};
pub use rules::{
    Battery, Container, ContainerId, FreeBattery, Gather, KillsDeaths, ModeRule, Phase, PlaceList,
    Research, ResearchUnit, Respawn, Rules, Tdm, Timed, Timelimit, rules,
};
pub use statistics::{
    BatteryStatus, ByPlace, EVENTS, Events, LastTaker, OwnerTeam, PlayerStatistics, Statistics, Tail,
    Winner, statistics,
};

/// Native weapon and ammo slot ordering.
pub const PLAYER_FIELDS_AT: usize = 2 + weapon_pickup::Record::BYTES;

pub const WEAPON_SLOTS: [u8; 2] = [7, 10];
pub const AMMO_SLOTS: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

/// What a world is checked against: the match it is of. The server fills
/// it from its match, the game from its own objects
/// (client_world_check.md section 7).
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    /// The world's time: its timelimit rule's `m_current_time_in_ms` is
    /// this.
    pub time: u32,
    /// The roster places the world's statistics and rules are for.
    pub places: usize,
    pub game_mode: u8,
    /// Modes 2 and 3: the map's `victory_items_count`.
    pub batteries: usize,
    /// The map's respawn points for the mode, when known.
    pub respawn_points: Option<usize>,
    /// By place, when known: the kits a place has player objects for and
    /// the item count of its state's head. Without them the kit bytes are
    /// checked against the table's three entries only.
    pub profiles: Option<&'a [Profile]>,
    /// The size of the inventory's artefacts table
    /// (`[[inventory+0x108]+0x18940]`), which a player state's artefact-slot
    /// ids index with no null check (0x2FE655). The game fills it from its
    /// live world; the server leaves it `None`, and the game's own check
    /// (which has the table) bounds it then, like the effects section's
    /// emitter index. The active-objects and carried-item bytes need no such
    /// field: their table is a fixed 255 entries and the reader null-checks
    /// each slot, so no u8 is a crash. See
    /// the player-tail checker.
    pub artefacts: Option<usize>,
    /// The objects of the world's game-objects section, in its order, when
    /// known: the places' grenades and traps, then the map's objects. Without
    /// it the section is not checked (a guessed match).
    pub objects: Option<&'a Objects<'a>>,
    /// The match's kits as the game makes their objects, when known: with
    /// it a player's state is read to its exact end ([`Inventory`]);
    /// without it the walk stops at the inventory header.
    pub inventory: Option<&'a Inventory<'a>>,
}

/// What a world's player block is checked against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    /// Bit k: kit k has a player object, which the game makes for the kits
    /// the match sent as usable (`KIT_USABLE`, 0x321820).
    pub usable_kits: u8,
    /// M, the items of the place's profile: the head has a u32 for each.
    pub items: usize,
    /// By kit, its two weapon slots ([`WEAPON_SLOTS`]): what a bullet of
    /// the place fired from that slot may name.
    pub weapons: [[Weapon; 2]; KITS],
}

/// What a kit has in a weapon slot, as the game's inventory has it
/// (`inventory_cook::on_subresources_loaded` 0x2C75B0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Weapon {
    /// The slot holds an item: the kit's slot has an item index (0xFFFF is
    /// none; the count is not looked at, 0x2C762E..0x2C7639).
    pub present: bool,
    /// The length of its ammo list: the kit's ammo slots ([`AMMO_SLOTS`])
    /// that hold an item (an index and a count over 0, 0x2C7739..0x2C77BA)
    /// compatible with the weapon (`items_compatibility`, either order,
    /// 0x2C78ED..0x2C7921), in slot order. At most 8.
    pub ammo: u8,
    /// Bit i: the ammo of list entry i has `buck_shot` over 1; bit `ammo`:
    /// the weapon's own `melee` params do (a bullet whose ammo index is the
    /// list's length is a melee hit, 0x2E2750).
    pub buckshot: u16,
}

impl Weapon {
    /// Whether a bullet of ammo index `ammo` has its pellet byte.
    pub fn buckshot(&self, ammo: u8) -> bool {
        self.buckshot.checked_shr(u32::from(ammo)).unwrap_or(0) & 1 != 0
    }
}

impl Profile {
    /// Whether `kit` is a kit of the place with a player object.
    pub fn usable(&self, kit: u8) -> bool {
        usize::from(kit) < KITS
            && self.usable_kits.checked_shr(u32::from(kit)).unwrap_or(0) & 1 != 0
    }
}

/// The battery records the statistics and the gather rule hold: the
/// map's batteries in modes 2 and 3, none in Team Deathmatch or Research.
fn battery_records(context: &Context) -> usize {
    if mode::WITH_BATTERIES.contains(&context.game_mode) {
        context.batteries
    } else {
        0
    }
}

/// The physics section is empty. The world's physics object has two
/// virtuals for it: slot +0x40, which `game_world_core::serialize`
/// 0x2D2570 calls with the section's writer (0x2D26C7..0x2D26D9), and slot
/// +0x44, which world apply 0x2D2770 calls with its reader
/// (0x2D2BEE..0x2D2C18), both on `[core+0x18FA4]` (set by the core's
/// constructor 0x2CFE90 from an argument, 0x2D00AC). In the vtable of
/// `vostok::physics::bullet_physics_world` (0x9E2EBC, written by its
/// constructor 0x74950), the only physics world 0.26g has, they sit
/// between `move` and `ray_test` and are `ret 4` (0x75AF0) and `ret 8`
/// (0x15A4B0): the game writes nothing there and reads nothing. PROVEN for
/// the vtable; that the core's object is that class is LIKELY (the only
/// implementation of the interface). All 2319 kept worlds have 0 bytes.
/// So a game that loads a world never reads its physics bytes, and a real
/// one never writes any: any byte there is not a game's.
pub fn physics(section: &Section) -> Result<(), Refusal> {
    if section.bytes.is_empty() {
        return Ok(());
    }
    Err(Refusal::new(
        Part::Physics,
        section.at,
        Reason::PhysicsBytes {
            len: section.bytes.len(),
        },
    ))
}

/// Validate all supported semantic sections after parsing the frame once.
/// Optional context bounds checks that need a live map or player profile.
pub fn validate_frame(frame: &Frame<'_>, context: &Context<'_>) -> Result<(), Refusal> {
    for block in frame.blocks() {
        player_head(&block, context)?;
        player_tail(&block, context)?;
    }
    // [`validate_kept`] repeats these sections: a new one goes in both.
    bullets(frame, context)?;
    statistics(&frame.statistics(), context)?;
    objects(&frame.objects(), context)?;
    rules(&frame.rules(), context)?;
    physics(&frame.physics())?;
    effects(&frame.effects())
}

/// Parse a body and validate its supported semantic sections.
pub fn validate(body: &[u8], context: &Context<'_>) -> Result<(), Refusal> {
    validate_frame(&frame(body, context.places)?, context)
}

/// [`validate`] keeping the players' states it read ([`player_tail_kept`]):
/// `keep` is handed each place's state as the walk read it, in block order.
/// A refusal may come after some places were handed: the caller keeps them
/// only once the whole world passed. The server's (it has the profiles and
/// the kits); the game's check is [`validate`].
pub fn validate_kept<'a>(
    body: &'a [u8],
    context: &Context<'_>,
    keep: &mut dyn for<'k> FnMut(u8, &PlayerState<'a>, Option<&ItemsAndRest<'a, 'k>>),
) -> Result<(), Refusal> {
    let frame = frame(body, context.places)?;
    for block in frame.blocks() {
        player_head(&block, context)?;
        player_tail_kept(&block, context, keep)?;
    }
    // [`validate_frame`]'s sections, repeated so that the game's path stays as it is (the exe's arena links it).
    bullets(&frame, context)?;
    statistics(&frame.statistics(), context)?;
    objects(&frame.objects(), context)?;
    rules(&frame.rules(), context)?;
    physics(&frame.physics())?;
    effects(&frame.effects())
}
