//! Why a message is refused, as numbers; the server's text from them.

use core::fmt;

use crate::world::PLAYER_STATE_CAP;

/// The part of a world a fault is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The mask, the player blocks and the six sections' lengths.
    Frame,
    /// The block of the player at this place.
    Player(u8),
    Bullets,
    Statistics,
    Objects,
    Physics,
    Rules,
    Effects,
    /// A match message, by its id (its payload after the id).
    Message(u8),
    /// 0xE4's match options, unpacked.
    Options,
}

impl Part {
    /// The six sections, in body order.
    pub const SECTIONS: [Part; 6] = [
        Part::Bullets,
        Part::Statistics,
        Part::Objects,
        Part::Physics,
        Part::Rules,
        Part::Effects,
    ];

    /// A short, fixed name of the part, without the place or id a
    /// [`Display`](fmt::Display) interpolates. It is the static table the
    /// game's payload logs on a refusal (`client_world_check.md` section 6):
    /// a `&'static str` so the payload holds no writable state, each with a
    /// trailing NUL so the payload can hand it to the native logger as a C
    /// string. The part and the offset, with the match server's own copy of
    /// the world (`SRP_SNAPSHOT_DIR`), are enough to find the fault.
    pub const fn name(self) -> &'static str {
        match self {
            Part::Frame => "frame\0",
            Part::Player(_) => "player\0",
            Part::Bullets => "bullets\0",
            Part::Statistics => "statistics\0",
            Part::Objects => "game objects\0",
            Part::Physics => "physics\0",
            Part::Rules => "rules\0",
            Part::Effects => "effects\0",
            Part::Message(_) => "message\0",
            Part::Options => "static info\0",
        }
    }
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Frame => f.write_str("frame"),
            Part::Player(place) => write!(f, "player {place}"),
            Part::Bullets => f.write_str("bullets"),
            Part::Statistics => f.write_str("statistics"),
            Part::Objects => f.write_str("game objects"),
            Part::Physics => f.write_str("physics"),
            Part::Rules => f.write_str("rules"),
            Part::Effects => f.write_str("effects"),
            Part::Message(id) => write!(f, "message 0x{id:02X}"),
            Part::Options => f.write_str("static info"),
        }
    }
}

/// What is wrong. The text of each is the server's warning
/// (world_validation.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    // The frame.
    /// The mask and the player blocks do not walk from the front.
    NoWalk,
    /// No six sections end at the body's end after the blocks.
    NoSections,
    /// The mask names a player at `place`, `places` being the roster's
    /// (at most 20).
    PlaceOutside {
        place: usize,
        places: usize,
    },
    /// A player state over [`PLAYER_STATE_CAP`].
    StateOverCap {
        len: usize,
    },
    /// A section over its cap.
    SectionOverCap {
        len: usize,
        cap: usize,
    },

    // A player block's head.
    /// No profiles known: a kit byte of 3 or more.
    KitByteOver,
    /// No profiles known: a state of under 2 bytes.
    StateUnder2,
    NoProfile,
    /// The head does not read for a profile of `items` items: the state
    /// (of `len` bytes) is shorter, or a bool is no bool.
    NoHead {
        items: usize,
        len: usize,
    },
    /// `current_index` names no kit of the place that has an object.
    CurrentKit(u8),
    /// `target_index`, the same.
    NextKit(u8),
    /// `m_need_to_select_animations` is no bool.
    SelectAnimations,
    /// The look pitch or the rotation is no number.
    LookNotNumber,

    // The bullets.
    /// The header and the records do not end at the section's end;
    /// `bullets` is the header's count (0 when there is no header).
    BulletsUnread {
        len: usize,
        bullets: u32,
    },
    BulletOwner {
        bullet: usize,
        owner: u8,
    },
    BulletSecond {
        bullet: usize,
        second: u8,
        owner: u8,
    },
    BulletSlot {
        bullet: usize,
        slot: u8,
    },
    /// The owner's current kit has nothing in the slot.
    BulletNoWeapon {
        bullet: usize,
        slot: u8,
        kit: u8,
    },
    BulletAmmo {
        bullet: usize,
        ammo: u8,
        count: u8,
    },
    BulletNotNumber {
        bullet: usize,
    },

    // The statistics.
    StatisticsEmpty,
    StatisticsUnread {
        len: usize,
        places: usize,
        batteries: usize,
    },
    DamageNotNumber {
        place: usize,
    },
    LastHitPast {
        place: usize,
        enemy: u8,
        places: usize,
    },

    // The rules.
    RulesUnread {
        len: usize,
        time: u32,
        places: usize,
    },
    NoRespawnAndModeRule,
    RespawnPoints {
        found: usize,
        map: usize,
    },
    RespawnTimePast {
        place: usize,
        places: usize,
    },
    NoTdmRule,
    TdmMaskPast {
        places: usize,
    },
    NoGatherRule,
    NoResearchRule,
    ResearchMaskPast {
        places: usize,
    },
    Batteries {
        found: usize,
        map: usize,
    },
    BatteryNotNumber {
        battery: usize,
    },
    ContainerUserPast {
        container: usize,
        user: u8,
        places: usize,
    },
    ContainerBatteryPast {
        container: usize,
        battery: u8,
        batteries: usize,
    },

    // The game objects.
    /// The map's `objects` game objects do not read to the section's exact
    /// end.
    ObjectsUnread {
        len: usize,
        objects: usize,
    },
    /// A usable object or container names a user past the roster.
    ObjectsUserPast {
        object: usize,
        user: u8,
        places: usize,
    },
    /// An artefact container names an artefact past the game's.
    ObjectsArtefactPast {
        object: usize,
        artefact: u8,
        artefacts: usize,
    },
    /// A victory container holds more batteries than the map has.
    ObjectsBatteriesPast {
        object: usize,
        count: u32,
        batteries: usize,
    },
    /// A victory container names a battery past the map's.
    ObjectsBatteryPast {
        object: usize,
        battery: u8,
        batteries: usize,
    },
    /// An anomaly names a state past its own.
    ObjectsStatePast {
        object: usize,
        state: u8,
        states: usize,
    },

    // The physics.
    PhysicsBytes {
        len: usize,
    },

    // The effects (a template).
    /// Neither 20 bytes (no effect) nor 87 (one).
    EffectsLength {
        len: usize,
    },
    /// The count at offset 8 is not the length's.
    EffectsCount {
        count: u32,
        len: usize,
    },
    /// A byte the template fixes: its offset in the section, what it is.
    EffectsByte {
        at: usize,
        what: &'static str,
        found: u8,
        wanted: u8,
    },
    /// The emitter's id (offset 58: the opening u32, 75: it less 1).
    EffectsEmitterId {
        at: usize,
        found: u32,
        wanted: u32,
    },

    // The player state's tail (after the head).
    /// The tail did not read to the inventory header: it ran short, or no
    /// item count M fit (`at` is where it stopped, in the state).
    TailUnread {
        at: usize,
    },
    /// A player-effect or hit-effect count too large to fit.
    TailCount {
        what: &'static str,
        count: u32,
    },
    /// A player-effect record is not the template's (a raw root emitter).
    TailEffect {
        at: usize,
    },
    /// A hit-effect record is not the template's (a bad bool or a raw root
    /// emitter).
    TailHitEffect {
        at: usize,
    },
    /// An f32 in the tail that is no number.
    TailNotNumber {
        at: usize,
    },
    /// An artefact-slot id past the artefacts table.
    TailArtefact {
        index: u8,
        bound: usize,
    },
    /// The artefact-slot count is over the 23 slots.
    TailArtefactCount {
        count: u8,
    },
    /// The artefact slots begin plus the count run past the 23 slots.
    TailSlotsBegin {
        begin: u8,
        count: u8,
    },
    /// A stamina bool that is not 0x00 or 0xFF.
    TailStaminaBool,
    /// An inventory bool that is not 0x00 or 0xFF.
    TailInventoryBool,
    /// An inventory item's owner (`player_id`, `profile_id`) names no player
    /// object in the match: the owner lookup `[core+0x18458]+(profile_id+
    /// player_id*12)*4` is past the table or a null entry (0x2FFF72).
    TailItemOwner {
        player_id: u8,
        profile_id: u8,
    },
    /// A weapon's `m_current_ammo_id` is past its ammo list of `count`
    /// (0x2E350A).
    TailWeaponAmmo {
        ammo: u8,
        count: u8,
    },

    // A match message.
    /// 0xE2, or an id outside 0xE3..0xF6.
    UnknownMessage,
    /// 0xED / 0xEE: srp never sends them.
    NeverSent,
    /// A late join proposal (0xE4 `FF 'L' 'J' 1`) of serial 0 or place 0.
    LateJoinProposal,
    MessageLength {
        len: usize,
        wanted: usize,
    },
    MessageShort {
        len: usize,
        least: usize,
    },
    /// A place or player byte past the 20-place table.
    PlacePast {
        place: u8,
    },
    /// An input's rotation half (dx or dy) that is no number under 1024.
    RotationNotSane {
        half: u16,
    },
    /// An input's actions with bit 29 or 30, which no stock client sets.
    UnusedActions {
        actions: u32,
    },
    /// 0xE8's compressed flag: srp sends 0 or 1.
    SnapshotFlag {
        flag: u8,
    },

    // 0xE4's options.
    OptionsTooLarge {
        len: usize,
    },
    OptionsShort,
    OptionsText {
        place: usize,
        len: u8,
    },
    OptionsKits {
        place: usize,
        kits: u8,
    },
    OptionsItems {
        place: usize,
        items: u8,
    },
    OptionsSlotMask {
        place: usize,
        mask: u32,
    },
    /// A kit's slot names an item past the table, or an empty one.
    OptionsSlotItem {
        place: usize,
        index: u16,
    },
    OptionsModifierMask {
        place: usize,
    },
    OptionsNotNumber {
        place: usize,
    },
    OptionsDictId {
        place: usize,
        dict_id: u16,
    },
    OptionsModifications {
        place: usize,
        count: u8,
    },
    OptionsUpgrades {
        place: usize,
        count: u8,
    },
    OptionsBool {
        place: usize,
        byte: u8,
    },
    OptionsRoster {
        roster: u8,
    },
    OptionsMap {
        map: u8,
    },
    OptionsTeam {
        place: usize,
        team: u8,
    },
    OptionsNoKit {
        place: usize,
    },
    /// A place past the roster with kits or items.
    OptionsPastRoster {
        place: usize,
    },
    OptionsWeight {
        event: usize,
    },
    OptionsTrailing {
        len: usize,
    },

    // A transport body's LZMA2 stream (0xE8's state, 0xE4's options),
    // bounded before the native decoder takes it ([`crate::transport`]).
    /// The chunk stream ends inside a chunk header or body.
    Lzma2Truncated,
    /// A control byte that is no chunk (not 0x00, 0x01, 0x02 or >= 0x80).
    Lzma2Control {
        control: u8,
    },
    /// The first chunk does not reset the dictionary (not 0x01, nor a
    /// compressed chunk with the dictionary-reset bits).
    Lzma2NoReset {
        control: u8,
    },
    /// Bytes follow the 0x00 end marker.
    Lzma2Trailing,
    /// The declared output is over the client's buffer.
    Lzma2TooLarge {
        len: usize,
        cap: usize,
    },
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Reason::NoWalk => f.write_str("the mask and player blocks do not walk from the front"),
            Reason::NoSections => {
                f.write_str("no six sections ending at the body's end after the player blocks")
            }
            Reason::PlaceOutside { place, places } => {
                write!(f, "a player at place {place}, past the {places} places")
            }
            Reason::StateOverCap { len } => {
                write!(f, "a state of {len:#x} bytes, over {PLAYER_STATE_CAP:#x}")
            }
            Reason::SectionOverCap { len, cap } => write!(f, "{len:#x} bytes, over {cap:#x}"),
            Reason::KitByteOver => f.write_str("a kit byte of 3 or more"),
            Reason::StateUnder2 => f.write_str("a state of under 2 bytes"),
            Reason::NoProfile => f.write_str("a place with no profile in the match"),
            Reason::NoHead { items, len } => write!(
                f,
                "no head for a profile of {items} items (state of {len} bytes, or a bool not \
                 0x00/0xFF)"
            ),
            Reason::CurrentKit(kit) => write!(f, "current kit {kit} is no usable kit of the place"),
            Reason::NextKit(kit) => write!(f, "next kit {kit} is no usable kit of the place"),
            Reason::SelectAnimations => {
                f.write_str("m_need_to_select_animations is not 0x00 or 0xFF")
            }
            Reason::LookNotNumber => f.write_str("a look pitch or rotation that is no number"),
            Reason::StatisticsEmpty => f.write_str("empty (no statistics object)"),
            Reason::StatisticsUnread {
                len,
                places,
                batteries,
            } => write!(
                f,
                "{len} bytes do not read as {places} players and {batteries} batteries to the end"
            ),
            Reason::DamageNotNumber { place } => {
                write!(f, "player {place}'s damage or bonus is no number")
            }
            Reason::LastHitPast {
                place,
                enemy,
                places,
            } => write!(
                f,
                "player {place}'s last hit enemy {enemy} is past the {places} places"
            ),
            Reason::RulesUnread { len, time, places } => write!(
                f,
                "{len} bytes do not read as a timelimit of {time} ms, kd stats of {places} places \
                 and the two flags"
            ),
            Reason::NoRespawnAndModeRule => {
                f.write_str("no respawn rule and mode rule between the timelimit and the kd stats")
            }
            Reason::RespawnPoints { found, map } => {
                write!(f, "{found} respawn points, the map has {map}")
            }
            Reason::RespawnTimePast { place, places } => write!(
                f,
                "a respawn time for place {place}, past the {places} places"
            ),
            Reason::NoTdmRule => f.write_str("no tdm rule"),
            Reason::TdmMaskPast { places } => {
                write!(f, "a tdm mask names a place past the {places} places")
            }
            Reason::NoGatherRule => f.write_str("no gather rule"),
            Reason::NoResearchRule => f.write_str("no research rule"),
            Reason::ResearchMaskPast { places } => {
                write!(f, "a research mask names a place past the {places} places")
            }
            Reason::Batteries { found, map } => write!(f, "{found} batteries, the map has {map}"),
            Reason::BatteryNotNumber { battery } => {
                write!(f, "battery {battery}'s position is no number")
            }
            Reason::ContainerUserPast {
                container,
                user,
                places,
            } => write!(
                f,
                "container {container}'s user {user} is past the {places} places"
            ),
            Reason::ContainerBatteryPast {
                container,
                battery,
                batteries,
            } => write!(
                f,
                "container {container} holds battery {battery}, past the {batteries} batteries"
            ),
            Reason::ObjectsUnread { len, objects } => write!(
                f,
                "{len} bytes do not read as the map's {objects} game objects to the end"
            ),
            Reason::ObjectsUserPast {
                object,
                user,
                places,
            } => write!(
                f,
                "object {object} names user {user}, past the {places} places"
            ),
            Reason::ObjectsArtefactPast {
                object,
                artefact,
                artefacts,
            } => write!(
                f,
                "object {object} names artefact {artefact}, past the {artefacts} the game has"
            ),
            Reason::ObjectsBatteriesPast {
                object,
                count,
                batteries,
            } => write!(
                f,
                "object {object} holds {count} batteries, the map has {batteries}"
            ),
            Reason::ObjectsBatteryPast {
                object,
                battery,
                batteries,
            } => write!(
                f,
                "object {object} holds battery {battery}, past the map's {batteries}"
            ),
            Reason::ObjectsStatePast {
                object,
                state,
                states,
            } => write!(
                f,
                "object {object} names state {state}, past the anomaly's {states}"
            ),
            Reason::PhysicsBytes { len } => write!(f, "{len} bytes where a game writes none"),
            Reason::BulletsUnread { len, bullets } => write!(
                f,
                "{len} bytes do not read as the header and {bullets} bullets to the end"
            ),
            Reason::BulletOwner { bullet, owner } => write!(
                f,
                "bullet {bullet}'s owner {owner} has no player block in the world"
            ),
            Reason::BulletSecond {
                bullet,
                second,
                owner,
            } => write!(
                f,
                "bullet {bullet}'s second id {second} is not its owner {owner}"
            ),
            Reason::BulletSlot { bullet, slot } => {
                write!(f, "bullet {bullet}'s weapon slot {slot} is not 7 or 10")
            }
            Reason::BulletNoWeapon { bullet, slot, kit } => write!(
                f,
                "bullet {bullet}'s owner has nothing in slot {slot} of its kit {kit}"
            ),
            Reason::BulletAmmo {
                bullet,
                ammo,
                count,
            } => write!(
                f,
                "bullet {bullet}'s ammo {ammo} is past the weapon's {count}"
            ),
            Reason::BulletNotNumber { bullet } => {
                write!(f, "bullet {bullet} has a float that is no number")
            }
            Reason::EffectsLength { len } => {
                write!(f, "{len} bytes, not the 20 of no effect or the 87 of one")
            }
            Reason::EffectsCount { count, len } => {
                write!(f, "{count} effects in {len} bytes")
            }
            Reason::EffectsByte {
                at,
                what,
                found,
                wanted,
            } => write!(
                f,
                "byte {at} ({what}) is {found:#04x}, the template's {wanted:#04x}"
            ),
            Reason::EffectsEmitterId { at, found, wanted } => write!(
                f,
                "the emitter id at {at} is {found:#x}, the opening u32 makes it {wanted:#x}"
            ),
            Reason::TailUnread { at } => {
                write!(
                    f,
                    "the state does not read to the inventory header (stopped at {at})"
                )
            }
            Reason::TailCount { what, count } => write!(f, "{count} {what}, too many to fit"),
            Reason::TailEffect { at } => {
                write!(
                    f,
                    "a player effect at {at} is not the template's (a raw root emitter)"
                )
            }
            Reason::TailHitEffect { at } => {
                write!(f, "a hit effect at {at} is not the template's")
            }
            Reason::TailNotNumber { at } => write!(f, "an f32 at {at} that is no number"),
            Reason::TailArtefact { index, bound } => {
                write!(f, "an artefact-slot id {index}, past the {bound} artefacts")
            }
            Reason::TailArtefactCount { count } => {
                write!(f, "an artefact-slot count {count}, over the 23 slots")
            }
            Reason::TailSlotsBegin { begin, count } => {
                write!(
                    f,
                    "artefact slots {begin}..{begin}+{count}, past the 23 slots"
                )
            }
            Reason::TailStaminaBool => f.write_str("a stamina bool that is not 0x00 or 0xFF"),
            Reason::TailInventoryBool => f.write_str("an inventory bool that is not 0x00 or 0xFF"),
            Reason::TailItemOwner {
                player_id,
                profile_id,
            } => write!(
                f,
                "an inventory item owned by place {player_id} kit {profile_id}, no player object in the match"
            ),
            Reason::TailWeaponAmmo { ammo, count } => {
                write!(
                    f,
                    "a weapon's current ammo id {ammo}, past its {count} ammo"
                )
            }
            Reason::UnknownMessage => f.write_str("no message a client takes"),
            Reason::NeverSent => f.write_str("a message srp never sends"),
            Reason::LateJoinProposal => {
                f.write_str("a late join proposal of serial 0 or of the first place")
            }
            Reason::MessageLength { len, wanted } => write!(f, "{len} bytes, not {wanted}"),
            Reason::MessageShort { len, least } => write!(f, "{len} bytes, under {least}"),
            Reason::PlacePast { place } => write!(f, "place {place}, past the 20 places"),
            Reason::RotationNotSane { half } => write!(
                f,
                "a rotation half {half:#06x} that is no number under 1024"
            ),
            Reason::UnusedActions { actions } => write!(
                f,
                "actions {actions:#010x} with bit 29 or 30, which no client sets"
            ),
            Reason::SnapshotFlag { flag } => {
                write!(f, "a compressed flag of {flag:#04x}, not 0 or 1")
            }
            Reason::OptionsTooLarge { len } => {
                write!(f, "{len} bytes, over the client's 0x578a0")
            }
            Reason::OptionsShort => f.write_str("the options end early"),
            Reason::OptionsText { place, len } => write!(
                f,
                "place {place} has a text of {len} bytes over its buffer, or with a NUL"
            ),
            Reason::OptionsKits { place, kits } => {
                write!(f, "place {place} has {kits} kits, over 3")
            }
            Reason::OptionsItems { place, items } => {
                write!(f, "place {place} has {items} items, over 69")
            }
            Reason::OptionsSlotMask { place, mask } => write!(
                f,
                "place {place} has a kit slot mask {mask:#x}, past slot 22"
            ),
            Reason::OptionsSlotItem { place, index } => write!(
                f,
                "place {place} has a kit slot of item {index}, past its items or empty"
            ),
            Reason::OptionsModifierMask { place } => {
                write!(f, "place {place} has a kit modifier past the 68")
            }
            Reason::OptionsNotNumber { place } => {
                write!(f, "place {place} has a kit modifier that is no number")
            }
            Reason::OptionsDictId { place, dict_id } => write!(
                f,
                "place {place} has item {dict_id}, which the dictionary does not have"
            ),
            Reason::OptionsModifications { place, count } => write!(
                f,
                "place {place} has an item with {count} modifications, over 8"
            ),
            Reason::OptionsUpgrades { place, count } => {
                write!(f, "place {place} has an item with {count} upgrades, over 3")
            }
            Reason::OptionsBool { place, byte } => {
                write!(f, "place {place} has a modification state of {byte:#04x}")
            }
            Reason::OptionsRoster { roster } => {
                write!(f, "a roster of {roster}, not 1 to 20")
            }
            Reason::OptionsMap { map } => {
                write!(f, "map {map}, which the client does not have")
            }
            Reason::OptionsTeam { place, team } => {
                write!(f, "place {place} has team {team}, not 0 or 1")
            }
            Reason::OptionsNoKit { place } => {
                write!(f, "place {place}, in the roster, has no kit")
            }
            Reason::OptionsPastRoster { place } => {
                write!(f, "place {place}, past the roster, has kits or items")
            }
            Reason::OptionsWeight { event } => {
                write!(f, "score weight {event} is no number")
            }
            Reason::OptionsTrailing { len } => write!(f, "{len} bytes after the options"),
            Reason::Lzma2Truncated => f.write_str("an LZMA2 stream cut inside a chunk"),
            Reason::Lzma2Control { control } => {
                write!(f, "an LZMA2 control byte {control:#04x} that is no chunk")
            }
            Reason::Lzma2NoReset { control } => write!(
                f,
                "a first LZMA2 chunk {control:#04x} that does not reset the dictionary"
            ),
            Reason::Lzma2Trailing => f.write_str("bytes after the LZMA2 end marker"),
            Reason::Lzma2TooLarge { len, cap } => {
                write!(
                    f,
                    "an LZMA2 stream decoding to {len:#x} bytes, over {cap:#x}"
                )
            }
        }
    }
}

/// Why a message goes to nobody: the part, where in the message (the
/// start of the part when the fault is the part as a whole), and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub part: Part,
    pub offset: usize,
    pub reason: Reason,
}

impl Refusal {
    pub const fn new(part: Part, offset: usize, reason: Reason) -> Self {
        Self {
            part,
            offset,
            reason,
        }
    }
}

/// `<part> at <offset>: <reason>`, the offset in hex.
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:#x}: {}", self.part, self.offset, self.reason)
    }
}
