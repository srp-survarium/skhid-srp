//! The bots' weapons as the client has them: rounds a load, time between
//! shots, how the trigger works, how long a reload takes; and the rounds a
//! bot carries in a life. The reflex counts the rounds it fires from these
//! and reloads when the magazine is out ([`super::reflex`]), instead of
//! guessing "about a clip of fire" by time.
//!
//! Where the numbers come from: **every weapon of the game data**, read from
//! its own config when first used (`survarium::v026g::weapons`; tags as in
//! the notes: PROVEN read in the data or the code, LIKELY read in 0.100b's
//! source and not traced in 0.26g's exe), so any weapon a kit may carry (a
//! tier's, a patch's) is fired by its own numbers:
//! - `parameters`: `magazine_capacity`, `rounds_per_minute`,
//!   `fire_queue_types`, `reload_time`, `reload_start_time`,
//!   `reload_end_time`, `chamber_a_round_time`, `show_time`, `hide_time`;
//!   and its `states`: which reload state it has (303 a magazine, 306 round
//!   by round, 311 two barrels, 314 another clip from empty) and whether it
//!   has a `chamber_a_round` state (304). PROVEN.
//! - How those make times: a shot is `60 / rounds_per_minute` s (every
//!   `shoot` clip has its one `shoot` mark at frame 0, so a shot leaves as
//!   the fire state starts; PROVEN for the five weapons whose clips were
//!   read); a magazine reload is `reload_time` (the AK-74U's clip, 95 f, is
//!   that exactly); round by round, `reload_start_time`, `reload_time /
//!   magazine_capacity` a round and `reload_end_time` (the Mosin's clips,
//!   50, 20 a round and 41 f, give 1.67 + 0.5 + 1.37 s against the
//!   config's 1.5 + 0.5 + 1.5: within 0.2 s); an empty reload of state 314
//!   and both barrels of state 311 take the TT-33's and the TOZ-34's share
//!   of their clips ([`EMPTY_RELOAD_SHARE`], [`BARRELS_RELOAD_SHARE`]).
//! - The bolt: `chamber_a_round_time`. LIKELY: the card's rate of fire is
//!   `60 / (60 / rpm + chamber_a_round_time)` floored for all eleven
//!   weapons that have one (the Mosin 27, the SV-98 21, the Remington 700
//!   32); 0.100b played the chamber clip at its own length (the Mosin's
//!   65 f, 2.17 s, 0.37 s longer). The players never press a bolt again
//!   sooner than 0.4 s after the config's cycle (`player_bots_weapons.md`
//!   §4), and the bots wait at least `CombatTuning::fire`'s
//!   `bolt_reaim_ms` after it, more than either, so a press never falls in
//!   the bolt whichever time is right.
//!
//! The reload loop of a weapon that loads round by round, read in 0.26g's exe
//! (PROVEN; `Loading`): the shotgun's sub-FSM (`weapon_core_shotgun_reload_state`,
//! built at 0x339900) plays a start clip, then a one-round clip in a loop whose
//! every end puts a round into the weapon (0x30EF80, `reload_one_round` 0x2E1A00:
//! one from the ammo into the magazine). At each of those ends it goes on to its
//! finish clip when the magazine is full or the ammo out (0x339DE0), or when a
//! round is in the weapon and the trigger (0x339EA0) or the sights (0x339EE0,
//! the AIM bit of the input) are held. So a bot that holds its sights through the
//! loop has one round, not eight, at the first end: the ledger counts the rounds
//! the client has put in at each end ([`Load::advance`]), not the load it meant
//! ([`Load::reload`]).
//!
//! The trigger, read in 0.26g's exe (PROVEN): the fire state
//! (`weapon_core_fire_state_base`, vtable 0xA14DAC, fields named by its
//! serializer 0x30EAB0) keeps `m_bullets_in_queue`, set at its start
//! (0x30E910) to the fire mode's `fire_queue_types` entry (-1 = the whole
//! magazine), at most the rounds in the weapon. Its tick (0x30EA00) refills
//! the queue only on a press after a release (`m_can_reset_fire_queue`,
//! set while the trigger is up with the queue spent, for a weapon without
//! a chamber state). The state is over (0x30EA50, its `+0x12D`) when the
//! queue is spent, or for an automatic queue when the trigger is up; then
//! it goes to idle (predicate 0x2E3B00 in the transitions built at
//! 0x2DF780), or to chambering for a weapon with a chamber state and a
//! round left (0x2E3C80), whatever the trigger. The fire mode (`+0x5E6`) is
//! 0 from the constructor (0x2DF4A0), the first entry, and changes only
//! with the next-mode key with the trigger up (0x2E0CA0), which bots never
//! press. So with the first entries: the Uzi and the AK-74U are automatic
//! (-1), the TOZ-34 fires one barrel (1), the Mosin and the TT-33 one round
//! (1). Not traced: whether idle goes back to fire while the trigger is
//! still held (its predicate 0x2E3CB0 wants only the trigger and a round)
//! when the weapon ticks there. The reflex therefore lets the trigger go
//! before a single shot's fire state is over and presses again only after
//! the weapon's [`Gun::cycle_ms`], so each press is one shot whatever that
//! answer is.

use survarium::v026g::bot_skills::{bot_modifiers, item_added, wire_mods};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::{
    BOT_AMMO_SLOTS, BotEquipment, BotMask, BotRole, BotTier, Loadout, RusherKit, RusherKits, bot_kits,
    upgrades_on,
};
use survarium::v026g::skills::{MODIFIERS, modifier_index};
use survarium::v026g::weapons::Dispersion;

/// How a weapon reloads, ms (see the module's notes for where each comes
/// from).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reload {
    /// A magazine (state 303): the same time whatever is left.
    Magazine { ms: u32 },
    /// A pistol (state 314): `ms` with a round left, `empty_ms` from empty.
    Pistol { ms: u32, empty_ms: u32 },
    /// Two barrels (state 311): `one_ms` with a shell left, `two_ms` from
    /// empty.
    Barrels { one_ms: u32, two_ms: u32 },
    /// Round by round (state 306): `start_ms`, then `round_ms` a round,
    /// then `finish_ms`.
    Rounds {
        start_ms: u32,
        round_ms: u32,
        finish_ms: u32,
    },
}

/// One weapon's facts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    /// The item's name (`items_dict`).
    pub item: &'static str,
    /// Rounds a full load (`magazine_capacity`).
    pub magazine: u32,
    /// `rounds_per_minute`: one pass of the fire clip, one shot, is
    /// `60 000 / rpm` ms.
    pub rpm: u32,
    /// Its first fire mode is automatic (`fire_queue_types[0] == -1`): a
    /// shot at every pass while the trigger is held; else one shot a press.
    pub auto: bool,
    /// After a shot with a round left, the bolt is worked this long (a
    /// `chamber_a_round` state), ms; 0: none.
    pub chamber_ms: u32,
    pub reload: Reload,
    /// Taken out and put away, ms (`show_time`, `hide_time`; that these are
    /// the states' times is LIKELY: 0.100b timed them by the clips).
    pub show_ms: u32,
    pub hide_ms: u32,
    /// How the bots fire it ([`Handling`]).
    pub handling: Handling,
    /// The dictionary category (8 rifles, 13 sniper rifles, 14 shotguns,
    /// 15 submachine guns, 16 machine guns, 17 pistols).
    pub category: u8,
    /// `aim_time`: the sights up, ms. Until then a shot spreads as from
    /// the hip (`dispersion`).
    pub aim_ms: u32,
    /// How wide its shots spread (`survarium::v026g::weapons::Dispersion`).
    pub dispersion: Dispersion,
    /// What the game data says it is ([`WeaponClass::of`]): its `wpn_*` type,
    /// the dictionary category only for a type nobody knows. The class the
    /// bots go by with `FireTuning::weapon_class_by_data` on.
    pub class: WeaponClass,
    /// What its dictionary category says it is ([`WeaponClass::by_category`]):
    /// the class the bots went by before the data's, and still go by with
    /// the switch off.
    pub by_category: WeaponClass,
}

impl Gun {
    /// [PR C] The measure's class, by its category (the sniper category split
    /// by its bolt): [`Gun::by_category`]'s name.
    pub fn class(&self) -> &'static str {
        self.by_category.name()
    }

    /// The class the bots go by: the game data's with `by_data`, else the
    /// category's.
    pub fn class_for(&self, by_data: bool) -> WeaponClass {
        if by_data { self.class } else { self.by_category }
    }
}

/// What kind of weapon a gun is, for the bots' choices that go by kind: the
/// range it is fired to, how long its bursts are held, how often its owner
/// crouches or side-steps, the distance band against another weapon.
///
/// One rule decides it ([`WeaponClass::of`]): **a weapon is of its dictionary
/// (shop) category's class, except that a submachine-gun type (`wpn_smg`) in
/// the rifle category is an SMG.** Where a weapon's `parameters.type` and its
/// category disagree, the category wins (an MP7, a PPSh or an automatic pistol
/// of the SMG category stay SMGs); the one disagreement the type decides is an
/// SMG type in the rifle category: of the 83 weapons of the live data (rev30
/// and rev34) that is the one case in which the type names a narrower family
/// than a rifle category, and the weapons it covers are of no other
/// disagreement (the ignored test
/// `on_the_live_data_the_weapons_where_type_and_category_disagree` lists every
/// disagreeing weapon with its result). A type nobody knows takes its
/// category's class like any other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponClass {
    Pistol,
    Smg,
    Rifle,
    MachineGun,
    Shotgun,
    /// One round a press, of the sniper category: the SVD.
    Marksman,
    /// A bolt or a pump (a `chamber_a_round` state).
    Bolt,
}

impl WeaponClass {
    /// The class of the weapon `w`: its category's, except an SMG type in the
    /// rifle category.
    pub fn of(w: &survarium::v026g::weapons::WeaponParams) -> Self {
        if w.kind == SMG_TYPE && w.category == RIFLE_CATEGORY {
            Self::Smg
        } else {
            Self::by_category(w.category, bolt_action(w))
        }
    }

    /// The class the dictionary category gives a weapon whose bolt action is
    /// `bolt`: what the bots went by before the game data's.
    pub fn by_category(category: u8, bolt: bool) -> Self {
        match category {
            SMG_CATEGORY => Self::Smg,
            MG_CATEGORY => Self::MachineGun,
            PISTOL_CATEGORY => Self::Pistol,
            SHOTGUN_CATEGORY => Self::Shotgun,
            SNIPER_CATEGORY if bolt => Self::Bolt,
            SNIPER_CATEGORY => Self::Marksman,
            _ => Self::Rifle,
        }
    }

    /// Its name in the measure's tables.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pistol => "pistol",
            Self::Smg => "smg",
            Self::Rifle => "rifle",
            Self::MachineGun => "mg",
            Self::Shotgun => "shotgun",
            Self::Marksman => "marksman",
            Self::Bolt => "bolt",
        }
    }

    /// The bursts' table an automatic weapon of this class takes.
    pub fn auto_class(self) -> AutoClass {
        match self {
            Self::Smg => AutoClass::Smg,
            _ => AutoClass::Rifle,
        }
    }
}

/// [measure] What the game data's weapon class ([`WeaponClass::of`]) changes
/// for the bots that hold a weapon: how many hold one, how many are of
/// another class than their category's, and of those how many change each
/// choice that goes by class (the bursts' table, the range floor), and how
/// many a distance band (the kind, `reflex::WeaponKind`). 0 on every weapon
/// of the 0.26g data but the ones that name another family than their
/// category: the AK-74U and its like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClassCounts {
    pub bots: u32,
    /// Of another class than their category's.
    pub differ: u32,
    /// ... with another bursts' table.
    pub burst: u32,
    /// ... under another range floor (`CombatTuning::range_floor_m`: floors by
    /// the class' name).
    pub floor: u32,
    /// Bots whose kind in the distance band differs from the one the switch off
    /// gives them.
    pub band: u32,
}

impl ClassCounts {
    /// A bot holding `gun`, whose band changes or not.
    pub fn add(&mut self, gun: &Gun, band_changes: bool) {
        self.bots += 1;
        self.band += u32::from(band_changes);
        if gun.class == gun.by_category {
            return;
        }
        self.differ += 1;
        self.burst += u32::from(gun.class.auto_class() != gun.by_category.auto_class());
        self.floor += u32::from(gun.class.name() != gun.by_category.name());
    }
}

/// The submachine guns' `parameters.type`.
pub const SMG_TYPE: &str = "wpn_smg";

/// The weapon is worked by a bolt or a pump: one round a press, then a
/// `chamber_a_round` state ([`Handling::Bolt`]).
fn bolt_action(w: &survarium::v026g::weapons::WeaponParams) -> bool {
    !w.automatic() && w.has_chamber && ms(w.chamber_a_round_time) > 0
}

/// The rows of the bursts' table an automatic weapon takes
/// (`player_bots_weapons.md` §3): the players hold a submachine gun longer
/// than a rifle or a machine gun at every distance, and tap either past 35 m.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoClass {
    /// Category 15.
    Smg,
    /// Any other automatic weapon: rifles, machine guns, the automatic
    /// ones of the sniper category.
    Rifle,
}

/// How a weapon is fired, from its config: its first fire mode and states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handling {
    /// Automatic (`fire_queue_types[0] == -1`): bursts held, their length
    /// by the distance.
    Burst(AutoClass),
    /// One round a press, nothing to work between: pressed again at its
    /// cycle, a little later at range (marksman rifles, pistols, the
    /// semi-automatic shotguns).
    Semi,
    /// A bolt or a pump (a `chamber_a_round` state): one press, then the
    /// bolt, the sights brought back on him, the next press. Never held.
    Bolt,
    /// Two barrels: a press each, then the reload.
    Barrels,
}

impl Gun {
    /// One shot's fire clip, ms.
    pub fn shot_ms(&self) -> f32 {
        60_000.0 / self.rpm as f32
    }

    /// From one press to the next press that fires again, ms: the fire
    /// clip, and the bolt after it.
    pub fn cycle_ms(&self) -> u32 {
        self.shot_ms().ceil() as u32 + self.chamber_ms
    }

    /// The shots of an automatic weapon's trigger held `held_ms`: one as
    /// the hold starts, one at every pass of the fire clip begun before it
    /// ends (a pass begun runs out after the release, its shot at its
    /// start already gone).
    pub fn shots_held(&self, held_ms: u32) -> u32 {
        (held_ms as f32 / self.shot_ms()).ceil() as u32
    }

    /// A reload with `left` rounds in the weapon that loads `loading`, ms.
    pub fn reload_ms(&self, left: u32, loading: u32) -> u32 {
        match self.reload {
            Reload::Magazine { ms } => ms,
            Reload::Pistol { ms, empty_ms } => {
                if left == 0 {
                    empty_ms
                } else {
                    ms
                }
            }
            // The clip goes by what is in it, not what is loaded (0.100b
            // `double_barreled_weapon_core_reload_state`: `mag != 1`).
            Reload::Barrels { one_ms, two_ms } => {
                if left == 0 {
                    two_ms
                } else {
                    one_ms
                }
            }
            Reload::Rounds {
                start_ms,
                round_ms,
                finish_ms,
            } => start_ms + round_ms * loading + finish_ms,
        }
    }
}

/// The empty reload's time over the other's, for a weapon whose reload
/// has its own clip from empty (state 314): the TT-33's two clips, 125 f
/// over 65 f (PROVEN for the TT-33; LIKELY of the same order for the
/// others, whose clips the server's data does not hold; the longer time
/// only makes a bot wait a little more).
pub const EMPTY_RELOAD_SHARE: f32 = 125.0 / 65.0;

/// Both barrels' reload over one barrel's (state 311): the TOZ-34's clips,
/// 110 f over 80 f (PROVEN for the TOZ-34; LIKELY for the others).
pub const BARRELS_RELOAD_SHARE: f32 = 110.0 / 80.0;

/// Seconds to ms.
fn ms(seconds: f32) -> u32 {
    (seconds * 1000.0).round().max(0.0) as u32
}

impl Gun {
    /// A weapon of the game data (`survarium::v026g::weapons`).
    pub fn of(w: &survarium::v026g::weapons::WeaponParams) -> Self {
        use survarium::v026g::weapons::{
            STATE_RELOAD_BARRELS, STATE_RELOAD_EMPTY, STATE_RELOAD_ROUNDS,
        };
        let reload_ms = ms(w.reload_time);
        let reload = match w.reload_state {
            STATE_RELOAD_ROUNDS => Reload::Rounds {
                start_ms: ms(w.reload_start_time),
                round_ms: reload_ms / w.magazine_capacity.max(1),
                finish_ms: ms(w.reload_end_time),
            },
            STATE_RELOAD_BARRELS => Reload::Barrels {
                one_ms: reload_ms,
                two_ms: ms(w.reload_time * BARRELS_RELOAD_SHARE),
            },
            STATE_RELOAD_EMPTY => Reload::Pistol {
                ms: reload_ms,
                empty_ms: ms(w.reload_time * EMPTY_RELOAD_SHARE),
            },
            _ => Reload::Magazine { ms: reload_ms },
        };
        let auto = w.automatic();
        let chamber_ms = if w.has_chamber {
            ms(w.chamber_a_round_time)
        } else {
            0
        };
        let handling = if auto {
            Handling::Burst(WeaponClass::by_category(w.category, false).auto_class())
        } else if chamber_ms > 0 {
            Handling::Bolt
        } else if matches!(reload, Reload::Barrels { .. }) {
            Handling::Barrels
        } else {
            Handling::Semi
        };
        Self {
            item: w.name,
            magazine: w.magazine_capacity,
            rpm: w.rounds_per_minute,
            auto,
            chamber_ms,
            reload,
            show_ms: ms(w.show_time),
            hide_ms: ms(w.hide_time),
            handling,
            category: w.category,
            aim_ms: ms(w.aim_time),
            dispersion: w.dispersion,
            class: WeaponClass::of(w),
            by_category: WeaponClass::by_category(w.category, chamber_ms > 0 && !auto),
        }
    }
}

/// What the character gives a weapon's times besides its own mods: the sums of
/// the player modifiers the client reads for it (the kit's skills and the worn
/// items' default and upgrade modifications; getter 0x2D5AE0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Factors {
    /// `reload_speed_modifier` (13: skill 108, gloves 1140..1144): a reload
    /// takes `reload_time / (1 + this)` [LIKELY: the speed given to the reload
    /// state's animation, 0x30F61D; the form is not traced to the state's
    /// clock].
    pub reload_speed: f32,
    /// `aim_time_modifier` (49: skill 101, gloves 1080..1084): the sights
    /// take `aim_time * max(0, 1 + this)` [PROVEN, 0x2E1B9A / 0x2E1C3B].
    pub aim_time: f32,
}

impl Factors {
    /// The factors of the kit `kit` of a bot whose skills give `modifiers`:
    /// those plus what the kit's items add.
    pub fn of_kit(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> Self {
        let index = |name| modifier_index(name).expect("a modifier of the 68");
        let worn = |parameter: &str| -> f32 {
            kit.items
                .iter()
                .map(|item| item_added(item.dict_id, &wire_mods(item.mods), parameter))
                .sum()
        };
        Self {
            reload_speed: modifiers[index("reload_speed")] + worn("reload_speed_modifier"),
            aim_time: modifiers[index("aim_time")] + worn("aim_time_modifier"),
        }
    }
}

impl Gun {
    /// The weapon `w` of a kit whose weapon carries `mods` and whose character
    /// gives `factors`: the weapon's own modifications make its times
    /// (`WeaponParams::with_mods`), then the reload speed shortens the reload
    /// (the loop's round, for a weapon loaded round by round: its start and
    /// finish clips are other keys) and the aim time the sights'. With neither
    /// it is [`Gun::of`].
    pub fn of_kit(
        w: &survarium::v026g::weapons::WeaponParams,
        mods: &[(u32, bool)],
        factors: Factors,
    ) -> Self {
        let mut w = w.with_mods(mods);
        w.reload_time /= (1.0 + factors.reload_speed).max(MIN_RELOAD_SPEED);
        w.aim_time *= (1.0 + factors.aim_time).max(0.0);
        Self::of(&w)
    }
}

impl Gun {
    /// Its numbers are not its weapon's base config's: a mod or a factor of
    /// the character ([`gun_with`]) made them.
    pub fn is_modded(&self) -> bool {
        gun(self.item).is_some_and(|base| base != self)
    }
}

/// [measure] What the weapon mods did to a bot's reloads: how many reloads
/// and round-by-round loops there were, how many of them with a [`Gun::is_modded`]
/// weapon, and how many loops the sights or the trigger cut short
/// ([`LoopEnd::cut`]). Only the game's own count of the rounds could tell the
/// ledger wrong; the server has none, so these show whether the modded guns
/// are cut more often than the plain ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModCounts {
    pub reloads: u32,
    pub reloads_modded: u32,
    pub loops: u32,
    pub loops_modded: u32,
    pub loops_cut: u32,
    pub loops_cut_modded: u32,
}

impl ModCounts {
    pub fn add(&mut self, other: &Self) {
        self.reloads += other.reloads;
        self.reloads_modded += other.reloads_modded;
        self.loops += other.loops;
        self.loops_modded += other.loops_modded;
        self.loops_cut += other.loops_cut;
        self.loops_cut_modded += other.loops_cut_modded;
    }

    /// A reload of `gun` began.
    pub fn reload(&mut self, gun: &Gun) {
        self.reloads += 1;
        self.reloads_modded += u32::from(gun.is_modded());
    }

    /// A round-by-round loop of `gun` ended as `end`.
    pub fn loop_end(&mut self, gun: &Gun, end: &LoopEnd) {
        let modded = u32::from(gun.is_modded());
        let cut = u32::from(end.cut());
        self.loops += 1;
        self.loops_modded += modded;
        self.loops_cut += cut;
        self.loops_cut_modded += cut * modded;
    }
}

/// The least a human's reload time is of the weapon's base: the weapon's
/// -15 % (2044) over the most reload speed a player sums, +27 % (skill 108 at
/// 0.12 and gloves 1144 at 0.15; `fact-weapon-mods.md` §3.2).
pub const HUMAN_MIN_RELOAD_RATIO: f32 = 0.85 / 1.27;
/// The least a human's aim time is of the weapon's base: skill 101 (-10 %)
/// and gloves 1084 (-10 %).
pub const HUMAN_MIN_AIM_RATIO: f32 = 0.80;

/// [measure] The bots that now hold a modded weapon, by what is modded, and
/// those whose weapon is modded past what a player can have
/// ([`HUMAN_MIN_RELOAD_RATIO`], [`HUMAN_MIN_AIM_RATIO`]): the harm, which the
/// owner's table keeps at 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeldMods {
    /// Bots holding a [`Gun::is_modded`] weapon.
    pub bots: u32,
    /// ... of them with a shorter reload, a shorter aim time, another rate of
    /// fire, another bolt time.
    pub reload: u32,
    pub aim: u32,
    pub rate: u32,
    pub chamber: u32,
    /// ... with a reload or an aim time shorter than a player can have.
    pub out_of_range: u32,
}

impl HeldMods {
    /// A bot holding `held`.
    pub fn add(&mut self, held: &Gun) {
        let Some(base) = gun(held.item).filter(|base| *base != held) else {
            return;
        };
        let ratio = |now: u32, then: u32| if then == 0 { 1.0 } else { now as f32 / then as f32 };
        let reload = ratio(held.reload_ms(0, held.magazine), base.reload_ms(0, base.magazine));
        let aim = ratio(held.aim_ms, base.aim_ms);
        self.bots += 1;
        self.reload += u32::from(held.reload != base.reload);
        self.aim += u32::from(held.aim_ms != base.aim_ms);
        self.rate += u32::from(held.rpm != base.rpm);
        self.chamber += u32::from(held.chamber_ms != base.chamber_ms);
        self.out_of_range +=
            u32::from(reload < HUMAN_MIN_RELOAD_RATIO - 0.005 || aim < HUMAN_MIN_AIM_RATIO - 0.005);
    }
}

/// What a bot banks of its [`ModCounts`] when its reflex is made anew at a life
/// (`BotFeatures`); the live reflex has the rest.
#[derive(Clone, Copy, Debug, Default)]
pub struct ModBank(pub ModCounts);

impl super::features::Feature for ModBank {}

/// The least a reload's speed factor is, so that a pile of modifiers never
/// divides by 0.
const MIN_RELOAD_SPEED: f32 = 0.1;

/// The weapon categories of the dictionary (`categories_dict`), named once.
/// The rifles'.
pub const RIFLE_CATEGORY: u8 = 8;
/// The sniper rifles' (bolt and marksman).
pub const SNIPER_CATEGORY: u8 = 13;
/// The shotguns'.
pub const SHOTGUN_CATEGORY: u8 = 14;
/// The submachine guns'.
pub const SMG_CATEGORY: u8 = 15;
/// The machine guns'.
pub const MG_CATEGORY: u8 = 16;
/// The pistols' (revolvers, automatic pistols).
pub const PISTOL_CATEGORY: u8 = 17;

/// The guns of the weapon table `weapons`, as the bots fire them. The
/// process's own is [`guns`]; a table made of other game data (a patched
/// copy, in a test) is read the same way.
pub fn guns_of(weapons: &[survarium::v026g::weapons::WeaponParams]) -> Vec<Gun> {
    weapons.iter().map(Gun::of).collect()
}

/// Every weapon of the game data, as the bots fire it.
pub fn guns() -> &'static [Gun] {
    static ALL: std::sync::OnceLock<Vec<Gun>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| guns_of(survarium::v026g::weapons::all()))
}

/// The facts of the weapon item `item` among `guns`.
pub fn gun_in<'a>(guns: &'a [Gun], item: &str) -> Option<&'a Gun> {
    guns.iter().find(|gun| gun.item == item)
}

/// The facts of the weapon item `item`.
pub fn gun(item: &str) -> Option<&'static Gun> {
    gun_in(guns(), item)
}

/// The weapon table the bots' guns are made from: the guns of the rows
/// and the rows themselves. The process's is [`Table::process`]; a test makes
/// one of a patched copy of the game data ([`Table::of`]).
#[derive(Clone, Copy, Debug)]
pub struct Table {
    pub guns: &'static [Gun],
    pub weapons: &'static [survarium::v026g::weapons::WeaponParams],
}

impl Table {
    /// The process's own: every weapon of the game data.
    pub fn process() -> Self {
        Self {
            guns: guns(),
            weapons: survarium::v026g::weapons::all(),
        }
    }

    /// The table of the weapon rows `weapons` (kept for good: a `Gun` is a
    /// `&'static`).
    pub fn of(weapons: Vec<survarium::v026g::weapons::WeaponParams>) -> Self {
        let weapons: &'static [_] = Box::leak(weapons.into_boxed_slice());
        Self {
            guns: Box::leak(guns_of(weapons).into_boxed_slice()),
            weapons,
        }
    }
}

/// The facts of the weapon `item` as a kit with the weapon's `mods` and a
/// character with `factors` has it ([`Gun::of_kit`]): the table's own `Gun`
/// when there is nothing to change, else one made once for the combination and
/// kept for good, so that a `Gun` stays a `&'static`.
pub fn gun_with(item: &str, mods: &[u32], factors: Factors) -> Option<&'static Gun> {
    gun_with_in(Table::process(), item, mods, factors)
}

/// [`gun_with`] over the table `table`. The combination is kept with the
/// table it was made from, so a patched table never takes a gun made from
/// another's numbers.
pub fn gun_with_in(
    table: Table,
    item: &str,
    mods: &[u32],
    factors: Factors,
) -> Option<&'static Gun> {
    let base = gun_in(table.guns, item)?;
    if mods.is_empty() && factors == Factors::default() {
        return Some(base);
    }
    type Key = (usize, &'static str, Vec<u32>, [u32; 2]);
    static MADE: std::sync::Mutex<Vec<(Key, &'static Gun)>> = std::sync::Mutex::new(Vec::new());
    let key: Key = (
        table.guns.as_ptr() as usize,
        base.item,
        mods.to_vec(),
        [factors.reload_speed.to_bits(), factors.aim_time.to_bits()],
    );
    let mut made = MADE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, gun)) = made.iter().find(|(made, _)| *made == key) {
        return Some(gun);
    }
    let weapon = table.weapons.iter().find(|w| w.name == item)?;
    let gun: &'static Gun = Box::leak(Box::new(Gun::of_kit(weapon, &wire_mods(mods), factors)));
    made.push((key, gun));
    Some(gun)
}

/// How far the client's clock may be behind the bot's, ms: a key reaches the
/// client at the tick it carries and the client reads its input a frame before
/// (`player_bots.md` §0), so a loop end the bot's count puts here can come that
/// much later in the client.
pub const CLIENT_LAG_MS: u32 = 100;

/// A round-by-round reload under way in the client ([`Load::reload`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loading {
    /// The tick of the reload key.
    at: u32,
    /// The rounds the loop is to put in: the room in the magazine, or the
    /// ammo there is.
    target: u32,
    /// The rounds it has put in.
    loaded: u32,
}

/// A round-by-round reload whose loop has ended ([`Load::advance`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopEnd {
    /// The tick of the reload key.
    pub at: u32,
    /// The rounds the client put in, of the `target` it was to.
    pub loaded: u32,
    pub target: u32,
    /// The loop's last end, and when the weapon can fire after its finish
    /// clip (allowing for [`CLIENT_LAG_MS`]).
    pub ended: u32,
    pub ready: u32,
}

impl LoopEnd {
    /// The trigger or the sights ended the loop before the magazine was full
    /// or the ammo out.
    pub fn cut(&self) -> bool {
        self.loaded < self.target
    }
}

/// A weapon in a life: its rounds in it and the rest of its ammo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub gun: &'static Gun,
    pub rounds: u32,
    pub spare: u32,
    /// Its loop, while a round-by-round reload runs.
    loading: Option<Loading>,
}

impl Load {
    /// At a spawn with `carried` rounds of its ammo: the weapon loaded from
    /// them (LIKELY: 0.100b fills the magazine from the ammo at the first
    /// activation, `load_ammo`, and a spawn gives the kit afresh).
    pub fn of(gun: &'static Gun, carried: u32) -> Self {
        let rounds = carried.min(gun.magazine);
        Self {
            gun,
            rounds,
            spare: carried - rounds,
            loading: None,
        }
    }

    /// Out with ammo left to load.
    pub fn needs_reload(&self) -> bool {
        self.rounds == 0 && self.spare > 0
    }

    /// Nothing in it and nothing to load.
    pub fn dry(&self) -> bool {
        self.rounds == 0 && self.spare == 0
    }

    /// Rounds a reload loads now (the client moves
    /// `min(ammo, capacity - in it)`, 0.100b `load_magazine`).
    pub fn loadable(&self) -> u32 {
        (self.gun.magazine - self.rounds).min(self.spare)
    }

    /// A reload begun at `now`: its time, ms. A magazine's, a pistol's or
    /// two barrels' rounds are counted in at once (the client moves them at
    /// the clip's end, and the shots wait for the time anyway); a
    /// round-by-round reload's as the client puts them in ([`Self::advance`]),
    /// so the time is that of the whole load, which the loop may end before.
    pub fn reload(&mut self, now: u32) -> u32 {
        let loading = self.loadable();
        let ms = self.gun.reload_ms(self.rounds, loading);
        if matches!(self.gun.reload, Reload::Rounds { .. }) {
            self.loading = (loading > 0).then_some(Loading {
                at: now,
                target: loading,
                loaded: 0,
            });
        } else {
            self.rounds += loading;
            self.spare -= loading;
        }
        ms
    }

    /// A round-by-round reload's loop is running, so the client takes the
    /// sights or the trigger held at a round's end as the wish to stop.
    pub fn loading(&self) -> bool {
        self.loading.is_some()
    }

    /// When the sights or the trigger held end the loop at its `n`-th round
    /// (or at the next one, when `n` have been put in already): the middle of
    /// the span before that round-end, so a key pressed then is held at it
    /// whatever the tick or the client's lag. `None`: no loop, or that round
    /// is the last, where the loop ends anyway.
    pub fn cut_from(&self, n: u32) -> Option<u32> {
        let Reload::Rounds {
            start_ms, round_ms, ..
        } = self.gun.reload
        else {
            return None;
        };
        let loading = self.loading?;
        let round = n.max(loading.loaded + 1);
        (round < loading.target)
            .then(|| loading.at + start_ms + round_ms * (round - 1) + round_ms / 2)
    }

    /// The loop's round-ends up to `now`: each puts a round into the weapon
    /// (the ledger's `rounds` up, `spare` down), and ends the loop when the
    /// magazine is full or the ammo out, or when `held` (the trigger or the
    /// sights were down in the keys sent since the last call). Returns the end
    /// of the loop when it came.
    pub fn advance(&mut self, now: u32, held: bool) -> Option<LoopEnd> {
        let Reload::Rounds {
            start_ms,
            round_ms,
            finish_ms,
        } = self.gun.reload
        else {
            return None;
        };
        let mut loading = self.loading?;
        loop {
            let end = loading.at + start_ms + round_ms * (loading.loaded + 1);
            if end > now {
                self.loading = Some(loading);
                return None;
            }
            loading.loaded += 1;
            self.rounds = (self.rounds + 1).min(self.gun.magazine);
            self.spare = self.spare.saturating_sub(1);
            if loading.loaded >= loading.target || held {
                self.loading = None;
                return Some(LoopEnd {
                    at: loading.at,
                    loaded: loading.loaded,
                    target: loading.target,
                    ended: end,
                    ready: end + finish_ms + CLIENT_LAG_MS,
                });
            }
        }
    }
}

/// What a bot carries: weapon 1 with a life's ammo, and the pistol with
/// its; and the rounds of weapon 1's ammo the kit has for the match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arms {
    /// Weapon 1 and the rounds a life starts with (its ammo slots').
    pub primary: (&'static Gun, u32),
    pub pistol: Option<(&'static Gun, u32)>,
    /// The rounds of weapon 1's ammo the kit has for the match, the life's
    /// among them: the profile's storage, which the client refills the
    /// slots from at each spawn (`loadouts::BOT_AMMO_LIVES`).
    pub reserve: u32,
}

/// A kit's [`Arms`]: its weapon 1, the rounds of its ammo slots
/// (`BOT_AMMO_SLOTS`) and their storage. With the bots' upgrades on
/// ([`upgrades_on`]) the weapon is the one the client fires: its own mods,
/// the reload speed and aim time of the bot's skills (`modifiers`) and worn
/// items; off, the weapon's base config as ever.
fn arms_of(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> Option<Arms> {
    arms_of_in(kit, modifiers, upgrades_on())
}

/// [`arms_of`] with the upgrades `on` or off, over the process's weapons.
fn arms_of_in(kit: &Loadout, modifiers: &[f32; MODIFIERS], on: bool) -> Option<Arms> {
    arms_in(Table::process(), Dictionary::shared(), kit, modifiers, on)
}

/// [`arms_of_in`] over the weapon table `table` and the dictionary
/// `dictionary` that names its items: the process's kits use the process's
/// ([`Table::process`], the shared dictionary); a test hands it the table of a
/// patched copy of the game data, and the mods and factors then apply to
/// that copy's numbers ([`gun_with_in`]).
pub fn arms_in(
    table: Table,
    dictionary: &Dictionary,
    kit: &Loadout,
    modifiers: &[f32; MODIFIERS],
    on: bool,
) -> Option<Arms> {
    let slot = kit.slot(7)?;
    let weapon = dictionary.item(slot.dict_id)?;
    let primary = if on {
        gun_with_in(table, &weapon.name, slot.mods, Factors::of_kit(kit, modifiers))?
    } else {
        gun_in(table.guns, &weapon.name)?
    };
    let ammo = kit
        .items
        .iter()
        .filter(|item| BOT_AMMO_SLOTS.contains(&item.slot));
    let rounds = ammo.clone().map(|item| item.count).sum();
    let reserve = ammo.map(|item| item.stored.max(item.count)).sum();
    Some(Arms {
        primary: (primary, rounds),
        pistol: None,
        reserve,
    })
}

/// What a bot has of weapon 1's ammo, and what a resupply zone's top-up could
/// bring it to (`inventory::resupply` 0x2FDC10: each ammo slot to its count,
/// the magazine and the chamber counted in, `min(count - held, storage)`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ammo {
    /// The rounds in the magazine and the stack.
    pub have: u32,
    /// The kit's count for a life ([`Arms::primary`]'s rounds).
    pub whole: u32,
    /// The rounds its kit has for the match beyond `have`: what the
    /// profile's storage could give.
    pub storage: u32,
}

impl Ammo {
    /// What a top-up would add now.
    pub fn missing(&self) -> u32 {
        self.whole.saturating_sub(self.have).min(self.storage)
    }

    /// The share of the kit's count it holds, `[0, 1]` (1 with no kit).
    pub fn share(&self) -> f32 {
        if self.whole == 0 {
            1.0
        } else {
            (self.have as f32 / self.whole as f32).min(1.0)
        }
    }
}

type ArmsTable = Vec<((BotTier, RusherKit, BotRole, usize), Option<Arms>)>;

impl Arms {
    /// The `k`-th kit (0 on) of a bot of `role` with `equipment`
    /// (`loadouts::bot_kits`). No pistol: the bots carry none. `None` for
    /// a kit that is not there or a weapon not in the game data ([`guns`]).
    pub fn of_kit(equipment: BotEquipment, role: BotRole, k: usize) -> Option<&'static Arms> {
        static ALL: std::sync::OnceLock<ArmsTable> = std::sync::OnceLock::new();
        let rusher = equipment.rusher.worn_in(equipment.tier);
        ALL.get_or_init(|| {
            let mut table = Vec::new();
            for tier in BotTier::ALL {
                // The rusher kits the game data can make: the others are
                // the old table (`RusherKit::worn_in`).
                for rusher in tier.rusher_kits().iter().copied() {
                    if rusher.worn_in(tier) != rusher {
                        continue;
                    }
                    for role in BotRole::ALL {
                        let equipment = BotEquipment { tier, rusher, mask: BotMask::Table };
                        let kits = bot_kits(equipment, role);
                        let modifiers = bot_modifiers(&kits);
                        for (k, kit) in kits.into_iter().enumerate() {
                            table.push(((tier, rusher, role, k), arms_of(kit, &modifiers)));
                        }
                    }
                }
            }
            table
        })
        .iter()
        .find(|(key, _)| *key == (equipment.tier, rusher, role, k))
        .and_then(|(_, arms)| arms.as_ref())
    }

    /// Every kit of a bot of `role` with `equipment`, in its order.
    pub fn kits_of(equipment: BotEquipment, role: BotRole) -> Vec<&'static Arms> {
        (0..role.kit_roles().len())
            .filter_map(|k| Self::of_kit(equipment, role, k))
            .collect()
    }

    /// A role's (`BotRole::name`, the personality of that name) first kit
    /// in the lowest tier.
    pub fn of_role(role: &str) -> Option<&'static Arms> {
        let role = BotRole::ALL.into_iter().find(|each| each.name() == role)?;
        Self::of_kit(
            BotEquipment::of(BotTier::Low, RusherKits::default()),
            role,
            0,
        )
    }

    /// For the tests of how a kind of weapon is fired, reloaded and switched
    /// from: the first bots' weapons by role (the Uzi's magazine, the
    /// AK-74U's, the TOZ-34's two barrels, the Mosin's clip of 5 loaded
    /// round by round, the SPAS-12's of 8), with a life's rounds as their
    /// kits had them (the most their bags take) and
    /// `loadouts::BOT_AMMO_LIVES` lives of storage.
    #[cfg(test)]
    pub fn of_test_role(role: &str) -> Option<&'static Arms> {
        let (item, rounds) = match role {
            "flanker" => ("uzi", 200),
            "support" => ("ak_74u", 90),
            "rusher" => ("toz_34", 60),
            "camper" => ("mosin_nagant", 30),
            "shotgunner" => ("spas_12", 40),
            _ => return None,
        };
        Some(Box::leak(Box::new(Arms {
            primary: (gun(item)?, rounds),
            pistol: None,
            reserve: rounds * survarium::v026g::loadouts::BOT_AMMO_LIVES,
        })))
    }

    /// For a bot whose personality is no role (a tuning's own): the
    /// support's, the middle of the four.
    pub fn fallback() -> &'static Arms {
        Self::of_role(BotRole::Support.name()).expect("the support's weapon is in the game data")
    }
}

/// A life's weapons and which is in hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hands {
    pub primary: Load,
    pub pistol: Option<Load>,
    /// The pistol is in hand.
    pub pistol_out: bool,
}

impl Hands {
    /// At a spawn: weapon 1 in hand (LIKELY: a spawn takes weapon 1).
    pub fn of(arms: &Arms) -> Self {
        Self {
            primary: Load::of(arms.primary.0, arms.primary.1),
            pistol: arms.pistol.map(|(gun, carried)| Load::of(gun, carried)),
            pistol_out: false,
        }
    }

    pub fn held(&self) -> &Load {
        match (self.pistol_out, &self.pistol) {
            (true, Some(pistol)) => pistol,
            _ => &self.primary,
        }
    }

    pub fn held_mut(&mut self) -> &mut Load {
        match (self.pistol_out, &mut self.pistol) {
            (true, Some(pistol)) => pistol,
            _ => &mut self.primary,
        }
    }

    /// Weapon 1 is dry and the pistol has rounds: time to take it, ms
    /// (weapon 1 put away, the pistol taken out); `None` otherwise.
    pub fn switch_ms(&self) -> Option<u32> {
        let pistol = self.pistol.filter(|p| !p.dry())?;
        (!self.pistol_out && self.primary.dry())
            .then_some(self.primary.gun.hide_ms + pistol.gun.show_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::dictionary::Dictionary;
    use survarium::v026g::item_params::item_params;

    #[test]
    fn the_magazines_are_the_games_and_every_bot_weapon_is_known() {
        let dictionary = Dictionary::shared();
        for gun in guns() {
            let item = dictionary.item_named(gun.item).unwrap();
            let params = item_params(item.dict_id).unwrap();
            assert_eq!(gun.magazine, params.magazine_capacity, "{}", gun.item);
        }
        every_kit_of_every_tier_is_known();
        for role in BotRole::ALL {
            let arms = Arms::of_role(role.name()).unwrap();
            assert_eq!(arms.primary.0.item, BotTier::Low.gear().of(role).weapon);
        }
        assert!(Arms::of_role("steady").is_none());
        assert_eq!(Arms::fallback().primary.0.item, "ak_74u");
    }

    /// Every kit of every tier and each rusher kit the game data can make:
    /// its weapon is known, a life's rounds are the kit's slot, its storage
    /// the lives' worth.
    fn every_kit_of_every_tier_is_known() {
        for tier in BotTier::ALL {
            let made = tier
                .rusher_kits()
                .iter()
                .copied()
                .filter(|&kit| kit.worn_in(tier) == kit);
            for rusher in made {
                let equipment = BotEquipment { tier, rusher, mask: BotMask::Table };
                for role in BotRole::ALL {
                    let kits = Arms::kits_of(equipment, role);
                    assert_eq!(kits.len(), 3, "{tier} {} {role}", rusher.name());
                    for (arms, kit_role) in kits.iter().zip(role.kit_roles()) {
                        assert_eq!(
                            arms.primary.0.item,
                            tier.gear().of_kit(kit_role, rusher).weapon
                        );
                        assert!(arms.pistol.is_none(), "the bots carry no weapon 2");
                        assert!(
                            arms.primary.1 >= arms.primary.0.magazine,
                            "{tier} {} {role}",
                            rusher.name()
                        );
                        assert_eq!(
                            arms.reserve,
                            arms.primary.1 * survarium::v026g::loadouts::BOT_AMMO_LIVES
                        );
                    }
                }
            }
        }
    }

    /// On the live game data (`SRP_GAME_DATA`, the server's file: the repo's
    /// test data lacks the 0.29a3 weapons): every rusher kit of both tiers
    /// has its arms, the SMGs with a life of 150 or 200 rounds. Run alone:
    /// `SRP_GAME_DATA=<server-game-data.db> cargo test -p match-server --lib
    /// -- --ignored on_the_live_data_every_rusher_kit_has_its_arms`
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn on_the_live_data_every_rusher_kit_has_its_arms() {
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
        every_kit_of_every_tier_is_known();
        for (tier, kit, item, rounds, magazine) in [
            (BotTier::Mid, RusherKit::Smg, "sr2m", 150, 20),
            (BotTier::Mid, RusherKit::Alt, "ump45", 150, 25),
            (BotTier::High, RusherKit::Smg, "fn_p90", 200, 50),
            (BotTier::High, RusherKit::Alt, "kriss_vector", 200, 30),
        ] {
            assert_eq!(kit.worn_in(tier), kit, "{tier} {}", kit.name());
            let rusher = BotEquipment { tier, rusher: kit, mask: BotMask::Table };
            let arms = Arms::of_kit(rusher, BotRole::Rusher, 0).expect("the arms");
            assert_eq!(arms.primary.0.item, item);
            assert_eq!(arms.primary.0.magazine, magazine, "{item}");
            assert_eq!(arms.primary.1, rounds, "{item}");
            // The flanker's third kit is the same gun.
            let third = Arms::of_kit(rusher, BotRole::Flanker, 2).expect("the arms");
            assert_eq!(third.primary.0.item, item);
        }
    }

    #[test]
    fn times_from_the_configs() {
        let [uzi, ak, toz, mosin, tt] =
            ["uzi", "ak_74u", "toz_34", "mosin_nagant", "tt_33"].map(|item| gun(item).unwrap());
        assert_eq!((uzi.shot_ms(), uzi.cycle_ms()), (100.0, 100));
        assert_eq!(ak.cycle_ms(), 93);
        assert_eq!((toz.cycle_ms(), tt.cycle_ms()), (250, 200));
        // The bolt: 375 ms of the shot and the config's 1.8 s of
        // chambering.
        assert_eq!(mosin.cycle_ms(), 375 + 1_800);
        assert_eq!(ak.reload_ms(5, 25), 3_167);
        assert_eq!((tt.reload_ms(3, 5), tt.reload_ms(0, 8)), (1_600, 3_077));
        assert_eq!((toz.reload_ms(1, 1), toz.reload_ms(0, 2)), (2_500, 3_438));
        assert_eq!(toz.reload_ms(0, 1), 3_438);
        assert_eq!(mosin.reload_ms(0, 5), 1_500 + 2_500 + 1_500);
        assert_eq!(mosin.reload_ms(3, 2), 1_500 + 1_000 + 1_500);
        // How each is fired.
        assert_eq!(uzi.handling, Handling::Burst(AutoClass::Smg));
        assert_eq!(ak.handling, Handling::Burst(AutoClass::Rifle));
        assert_eq!(toz.handling, Handling::Barrels);
        assert_eq!(mosin.handling, Handling::Bolt);
        assert_eq!(tt.handling, Handling::Semi);
        let [svd, rpd, sv98, mp153] =
            ["svd", "rpd", "sv_98", "mp_153"].map(|item| gun(item).unwrap());
        assert_eq!(svd.handling, Handling::Semi);
        assert_eq!(rpd.handling, Handling::Burst(AutoClass::Rifle));
        assert_eq!(sv98.handling, Handling::Bolt);
        assert_eq!(sv98.cycle_ms(), 273 + 2_500);
        assert_eq!(mp153.handling, Handling::Semi);
        // Every weapon of the data has its handling, and a bolt is never
        // automatic.
        for gun in guns() {
            assert_eq!(
                gun.auto,
                matches!(gun.handling, Handling::Burst(_)),
                "{}",
                gun.item
            );
            assert!(gun.magazine > 0 && gun.rpm > 0, "{}", gun.item);
        }
        // An automatic hold: a shot at its start, one per pass begun.
        assert_eq!(ak.shots_held(50), 1);
        assert_eq!(ak.shots_held(100), 2);
        assert_eq!(ak.shots_held(600), 7);
        assert_eq!(uzi.shots_held(300), 3);
        // A tier weapon from the data: the PP-2000's 750 a minute, 80 ms a
        // shot.
        assert_eq!(gun("pp2000").unwrap().cycle_ms(), 80);
    }

    /// A role's arms with the TT-33 and a clip of its ammo as weapon 2:
    /// the switch is kept for kits that have one.
    fn with_pistol(role: &str) -> &'static Arms {
        let arms = Arms::of_test_role(role).unwrap();
        Box::leak(Box::new(Arms {
            primary: (arms.primary.0, 30),
            pistol: gun("tt_33").map(|gun| (gun, 50)),
            reserve: 30,
        }))
    }

    #[test]
    fn a_life_loads_from_its_ammo_and_reloads_what_is_left() {
        // The support: a life's 90 rounds (three bags of 30), 30 in the
        // magazine, 60 to load.
        let support = Hands::of(Arms::of_test_role("support").unwrap());
        assert_eq!((support.primary.rounds, support.primary.spare), (30, 60));
        // Dry with no weapon 2: no switch, ever.
        let mut bare = Hands::of(Arms::of_test_role("camper").unwrap());
        let mosin = bare.held_mut();
        (mosin.rounds, mosin.spare) = (0, 0);
        assert!(bare.held().dry());
        assert_eq!(bare.switch_ms(), None);
        // The camper with a pistol: 30 rounds, 5 in, 25 to load; down to
        // its last 5.
        let mut camper = Hands::of(with_pistol("camper"));
        let mosin = camper.held_mut();
        assert_eq!((mosin.rounds, mosin.spare), (5, 25));
        mosin.spare = 5;
        mosin.rounds = 2;
        // Round by round: the rounds are the client's at each end of its loop.
        assert_eq!(mosin.reload(0), 1_500 + 3 * 500 + 1_500);
        assert_eq!((mosin.rounds, mosin.spare), (2, 5));
        assert!(mosin.advance(1_500 + 3 * 500, false).is_some());
        assert_eq!((mosin.rounds, mosin.spare), (5, 2));
        mosin.rounds = 0;
        assert!(mosin.needs_reload());
        assert_eq!(mosin.reload(10_000), 1_500 + 2 * 500 + 1_500);
        assert!(mosin.advance(10_000 + 1_500 + 2 * 500, false).is_some());
        assert_eq!((mosin.rounds, mosin.spare), (2, 0));
        mosin.rounds = 0;
        assert!(mosin.dry());
        // Dry: the pistol, put away and taken out.
        assert_eq!(camper.switch_ms(), Some(800 + 500));
        camper.pistol_out = true;
        assert_eq!(camper.held().gun.item, "tt_33");
        assert_eq!((camper.held().rounds, camper.held().spare), (8, 42));
        assert_eq!(camper.switch_ms(), None);
    }

    #[test]
    fn a_mod_reaches_the_guns_numbers() {
        let none = Factors::default();
        let ak = gun("ak_74u").unwrap();
        assert_eq!(ak.reload, Reload::Magazine { ms: 3167 });
        // 2044: -15 %, 3167 ms to 2692.
        let fast = gun_with("ak_74u", &[2044], none).unwrap();
        assert_eq!(fast.reload, Reload::Magazine { ms: 2692 });
        assert_eq!(Gun { reload: ak.reload, ..*fast }, *ak);
        assert!(fast.is_modded() && !ak.is_modded());
        // The same combination is the same `Gun`; nothing to change is the table's.
        assert!(std::ptr::eq(fast, gun_with("ak_74u", &[2044], none).unwrap()));
        assert!(std::ptr::eq(gun_with("ak_74u", &[], none).unwrap(), ak));
        // Gloves with +15 % reload speed (1144) on top: 2692 / 1.15.
        let gloves = Factors { reload_speed: 0.15, aim_time: 0.0 };
        assert_eq!(gun_with("ak_74u", &[2044], gloves).unwrap().reload, Reload::Magazine { ms: 2341 });
        // The aim time follows its factor (skill 101: -10 %).
        let aim = Factors { reload_speed: 0.0, aim_time: -0.10 };
        let aimed = gun_with("ak_74u", &[], aim).unwrap();
        assert_eq!(aimed.aim_ms, (ak.aim_ms as f32 * 0.9).round() as u32);
        assert_eq!(Gun { aim_ms: ak.aim_ms, ..*aimed }, *ak);
        // Inert types leave every field.
        for inert in [2074, 2084] {
            assert_eq!(*gun_with("ak_74u", &[inert], none).unwrap(), *ak, "{inert}");
        }
        // The rate of fire and the bolt's chamber follow their types.
        let rate = gun_with("ak_74u", &[2034], none).unwrap();
        assert!(rate.rpm > ak.rpm && rate.shot_ms() < ak.shot_ms());
        let mosin = gun("mosin_nagant").unwrap();
        assert!(gun_with("mosin_nagant", &[2054], none).unwrap().chamber_ms < mosin.chamber_ms);
    }

    /// A mod on a weapon loaded round by round shortens the loop's round
    /// only, and the ledger counts the first round in at the start plus the
    /// shorter round.
    #[test]
    fn a_reload_mod_on_a_shotgun_moves_the_ledger() {
        let plain = gun("spas_12").unwrap();
        let fast = gun_with("spas_12", &[2042], Factors::default()).unwrap();
        let (Reload::Rounds { start_ms, round_ms, finish_ms }, Reload::Rounds { start_ms: s, round_ms: r, finish_ms: f }) =
            (plain.reload, fast.reload)
        else {
            panic!("round by round");
        };
        assert_eq!((start_ms, finish_ms, round_ms), (1_000, 1_000, 375));
        assert_eq!((s, f), (1_000, 1_000));
        assert_eq!(r, 2_700 / 8);
        let mut load = Load {
            rounds: 0,
            spare: 30,
            ..Load::of(fast, 0)
        };
        assert_eq!(load.reload(0), 1_000 + 8 * r + 1_000);
        assert_eq!(load.advance(1_000 + r - 1, false), None);
        assert_eq!((load.rounds, load.spare), (0, 30));
        assert_eq!(load.advance(1_000 + r, false), None);
        assert_eq!((load.rounds, load.spare), (1, 29));
    }

    /// The kits' weapons with the upgrades on carry their mods, the gloves'
    /// reload speed and the bots' aim skill; off, they are the table's.
    #[test]
    fn the_upgrades_switch_decides_whether_a_kit_has_modded_guns() {
        use survarium::v026g::bot_skills::bot_modifiers;
        use survarium::v026g::loadouts::bot_kits_in;
        for tier in [BotTier::Mid, BotTier::High] {
            for role in BotRole::ALL {
                let equipment = BotEquipment::of(tier, RusherKits::default());
                let kits = bot_kits_in(equipment, role, false, true);
                let refs: Vec<&Loadout> = kits.iter().collect();
                let modifiers = bot_modifiers(&refs);
                for kit in &kits {
                    let weapon = Dictionary::shared().item(kit.slot(7).unwrap().dict_id).unwrap();
                    let base = gun(&weapon.name).unwrap();
                    let at = format!("{tier} {role} {}", weapon.name);
                    let off = arms_of_in(kit, &modifiers, false).unwrap();
                    assert!(std::ptr::eq(off.primary.0, base), "{at}");
                    let on = arms_of_in(kit, &modifiers, true).unwrap().primary.0;
                    // Skill 101, five levels: the sights up 10 % sooner.
                    assert_eq!(on.aim_ms, (base.aim_ms as f32 * 0.9).round() as u32, "{at}");
                    let light = matches!(role, BotRole::Rusher | BotRole::Flanker);
                    // The gloves of the light roles at 9-10 (`edge_gloves_2`) carry
                    // +8 % reload speed by default (1141); at 13-15 the upgrade's 10 %.
                    let (weapon_mod, gloves) = match (tier, light) {
                        (BotTier::Mid, true) => (0.10, 0.08),
                        (BotTier::High, true) => (0.12, 0.10),
                        _ => (0.0, 0.0),
                    };
                    let w = survarium::v026g::weapons::weapon_named(&weapon.name).unwrap();
                    let time = |w: &survarium::v026g::weapons::WeaponParams| {
                        Gun::of(w).reload_ms(0, w.magazine_capacity)
                    };
                    let want = survarium::v026g::weapons::WeaponParams {
                        reload_time: w.reload_time * (1.0 - weapon_mod) / (1.0 + gloves),
                        ..*w
                    };
                    assert_eq!(on.reload_ms(0, w.magazine_capacity), time(&want), "{at}");
                    // The weight upgrade of the heavy roles changes no gun number
                    // but the sights.
                    if !light {
                        assert_eq!(Gun { aim_ms: base.aim_ms, ..*on }, *base, "{at}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_counters_tell_modded_reloads_and_cut_loops_apart() {
        let plain = gun("spas_12").unwrap();
        let fast = gun_with("spas_12", &[2042], Factors::default()).unwrap();
        let mut counts = ModCounts::default();
        counts.reload(plain);
        counts.reload(fast);
        let end = |loaded| LoopEnd {
            at: 0,
            loaded,
            target: 8,
            ended: 0,
            ready: 0,
        };
        counts.loop_end(plain, &end(8));
        counts.loop_end(fast, &end(3));
        let mut sum = ModCounts::default();
        sum.add(&counts);
        sum.add(&counts);
        assert_eq!(
            sum,
            ModCounts {
                reloads: 4,
                reloads_modded: 2,
                loops: 4,
                loops_modded: 2,
                loops_cut: 2,
                loops_cut_modded: 2,
            }
        );
    }

    #[test]
    fn the_held_mods_count_what_is_modded_and_what_is_past_a_players_reach() {
        let none = Factors::default();
        let mut held = HeldMods::default();
        held.add(gun("ak_74u").unwrap());
        assert_eq!(held, HeldMods::default());
        // The owner's 13-15 rusher: weapon -12 %, gloves +10 %, skill 101 -10 %.
        let owners = Factors { reload_speed: 0.10, aim_time: -0.10 };
        let mut held = HeldMods::default();
        held.add(gun_with("ak_74u", &[2043], owners).unwrap());
        held.add(gun_with("ak_74u", &[2061], none).unwrap());
        assert_eq!(
            held,
            HeldMods { bots: 1, reload: 1, aim: 1, rate: 0, chamber: 0, out_of_range: 0 }
        );
        // A rate of fire and a bolt are counted by what they change.
        held.add(gun_with("ak_74u", &[2034], none).unwrap());
        held.add(gun_with("mosin_nagant", &[2054], none).unwrap());
        assert_eq!((held.bots, held.rate, held.chamber), (3, 1, 1));
        // A reload past a player's most (-15 % and +27 % speed): 0.5 of the base.
        let absurd = Factors { reload_speed: 1.0, aim_time: 0.0 };
        let mut held = HeldMods::default();
        held.add(gun_with("ak_74u", &[], absurd).unwrap());
        assert_eq!((held.reload, held.out_of_range), (1, 1));
        // The most a player has is not past it.
        let most = Factors { reload_speed: 0.27, aim_time: -0.20 };
        let mut held = HeldMods::default();
        held.add(gun_with("ak_74u", &[2044], most).unwrap());
        assert_eq!(held.out_of_range, 0);
    }

    /// The SPAS-12 with `rounds` in it and `spare` to load.
    fn spas(rounds: u32, spare: u32) -> Load {
        Load {
            rounds,
            spare,
            ..Load::of(gun("spas_12").unwrap(), 0)
        }
    }

    #[test]
    fn a_round_by_round_reload_is_counted_at_each_end_of_the_clients_loop() {
        // The SPAS-12: a 1.0 s start, 0.375 s a round, a 1.0 s finish.
        let mut load = spas(0, 30);
        assert_eq!(load.reload(2_000), 1_000 + 8 * 375 + 1_000);
        // Nothing is in the weapon before the first end of the loop ...
        assert!(load.loading());
        assert_eq!(load.advance(2_000 + 1_000 + 374, false), None);
        assert_eq!((load.rounds, load.spare), (0, 30));
        // ... and then a round at each end; the eighth ends the loop.
        for k in 1..8 {
            assert_eq!(load.advance(2_000 + 1_000 + 375 * k, false), None);
            assert_eq!((load.rounds, load.spare), (k, 30 - k));
        }
        let end = load.advance(2_000 + 1_000 + 375 * 8 + 10, false).unwrap();
        assert_eq!((load.rounds, load.spare, load.loading()), (8, 22, false));
        assert_eq!(
            end,
            LoopEnd {
                at: 2_000,
                loaded: 8,
                target: 8,
                ended: 6_000,
                // The finish clip, and the client's lag.
                ready: 6_000 + 1_000 + CLIENT_LAG_MS,
            }
        );
        assert!(!end.cut());
        // A loop over counts nothing more.
        assert_eq!(load.advance(60_000, true), None);
        assert_eq!((load.rounds, load.spare), (8, 22));
    }

    #[test]
    fn one_shell_followed_by_the_sights_released_is_one_round_and_then_the_load_is_full() {
        let mut load = spas(0, 30);
        let ms = load.reload(0);
        // The sights held at the first end of the loop with a round in the
        // weapon: the client goes on to its finish clip with one round, and
        // the ledger has the one, not the eight the load meant.
        assert_eq!(
            load.advance(1_000 + 375, true).map(|end| end.cut()),
            Some(true)
        );
        assert_eq!((load.rounds, load.spare, load.loading()), (1, 29, false));
        assert!(ms > 1_000 + 375 + 1_000);
        // The sights let go for the next reload: the loop runs to the full
        // magazine, seven rounds more.
        let ms = load.reload(10_000);
        assert_eq!(ms, 1_000 + 7 * 375 + 1_000);
        let mut rounds = Vec::new();
        for k in 1..=7 {
            let end = load.advance(10_000 + 1_000 + 375 * k, false);
            rounds.push(load.rounds);
            assert_eq!(end.is_some(), k == 7);
        }
        assert_eq!(rounds, [2, 3, 4, 5, 6, 7, 8]);
        assert_eq!((load.rounds, load.spare), (8, 22));
    }

    #[test]
    fn the_ledger_has_the_rounds_for_any_load_of_one_to_eight() {
        for room in 1..=8u32 {
            let mut load = spas(8 - room, 100);
            let ms = load.reload(500);
            assert_eq!(ms, 1_000 + 375 * room + 1_000, "{room}");
            // Held at the end of the k-th round: k rounds, the rest left.
            for cut in 1..=room {
                let mut load = load;
                for k in 1..cut {
                    assert_eq!(load.advance(500 + 1_000 + 375 * k, false), None);
                }
                let end = load.advance(500 + 1_000 + 375 * cut, true).unwrap();
                assert_eq!(end.loaded, cut, "{room} {cut}");
                assert_eq!(end.cut(), cut < room);
                assert_eq!((load.rounds, load.spare), (8 - room + cut, 100 - cut));
                assert_eq!(end.ready, end.ended + 1_000 + CLIENT_LAG_MS);
            }
            // Never held: the room is filled, and no more.
            let end = load.advance(500 + 1_000 + 375 * room, false).unwrap();
            assert_eq!((end.loaded, end.target), (room, room));
            assert_eq!((load.rounds, load.spare), (8, 100 - room));
        }
        // Less ammo than room: the ammo is the load, and it ends the loop.
        let mut load = spas(2, 3);
        assert_eq!(load.reload(0), 1_000 + 3 * 375 + 1_000);
        let end = load.advance(1_000 + 3 * 375, false).unwrap();
        assert_eq!((end.loaded, end.target, end.cut()), (3, 3, false));
        assert_eq!((load.rounds, load.spare), (5, 0));
        // Nothing to load: no loop, nothing counted.
        let mut load = spas(8, 20);
        load.reload(0);
        assert!(!load.loading());
        assert_eq!(load.advance(10_000, true), None);
        assert_eq!((load.rounds, load.spare), (8, 20));
    }

    #[test]
    fn the_sights_cut_a_loop_from_the_middle_before_the_round_they_are_to_end_it_at() {
        let mut load = spas(0, 30);
        assert_eq!(load.cut_from(3), None);
        load.reload(2_000);
        // The third round ends at 1.0 + 3 x 0.375 s: from the middle of
        // the 0.375 s before it.
        assert_eq!(load.cut_from(3), Some(2_000 + 1_000 + 2 * 375 + 187));
        // Held from there, the loop ends at that round with three rounds.
        let from = load.cut_from(3).unwrap();
        assert_eq!(load.advance(from, false), None);
        assert_eq!(
            load.advance(2_000 + 1_000 + 3 * 375, true)
                .map(|e| e.loaded),
            Some(3)
        );
        // The last round ends the loop anyway: nothing to cut. Nor with a
        // load of three or less asked to stop at three.
        let mut load = spas(0, 30);
        load.reload(0);
        assert_eq!(load.cut_from(8), None);
        assert_eq!(load.cut_from(9), None);
        let mut load = spas(6, 30);
        load.reload(0);
        assert_eq!(load.cut_from(3), None);
        // Three put in already: the next round-end, not one past.
        let mut load = spas(0, 30);
        load.reload(0);
        for k in 1..=3 {
            load.advance(1_000 + 375 * k, false);
        }
        assert_eq!(load.cut_from(3), Some(1_000 + 3 * 375 + 187));
        // A magazine has no loop.
        let mut ak = Load::of(gun("ak_74u").unwrap(), 90);
        ak.rounds = 0;
        ak.reload(0);
        assert_eq!(ak.cut_from(1), None);
    }

    #[test]
    fn a_magazine_or_a_pistols_clip_is_counted_in_at_once_as_before() {
        let mut ak = Load::of(gun("ak_74u").unwrap(), 90);
        ak.rounds = 5;
        ak.spare = 60;
        assert_eq!(ak.reload(0), 3_167);
        assert_eq!((ak.rounds, ak.spare, ak.loading()), (30, 35, false));
        assert_eq!(ak.advance(10_000, true), None);
        assert_eq!((ak.rounds, ak.spare), (30, 35));
        let mut tt = Load::of(gun("tt_33").unwrap(), 50);
        tt.rounds = 0;
        tt.spare = 42;
        assert_eq!(tt.reload(0), 3_077);
        assert_eq!((tt.rounds, tt.spare, tt.loading()), (8, 34, false));
        let mut toz = Load::of(gun("toz_34").unwrap(), 20);
        toz.rounds = 0;
        toz.spare = 20;
        assert_eq!(toz.reload(0), 3_438);
        assert_eq!((toz.rounds, toz.spare, toz.loading()), (2, 18, false));
    }

    #[test]
    fn the_guns_that_load_round_by_round_are_the_datas() {
        // By the reload state of the config, not by the kind of gun: the
        // semi-automatic shotguns and the pump and lever ones are in it, the
        // two-barrel guns are not.
        for item in ["spas_12", "mp_153", "benelli_m4", "rem_870", "mosin_nagant"] {
            assert!(
                matches!(gun(item).unwrap().reload, Reload::Rounds { .. }),
                "{item}"
            );
        }
        assert!(matches!(
            gun("toz_34").unwrap().reload,
            Reload::Barrels { .. }
        ));
        let spas = gun("spas_12").unwrap();
        assert_eq!(
            spas.reload,
            Reload::Rounds {
                start_ms: 1_000,
                round_ms: 375,
                finish_ms: 1_000
            }
        );
    }
}
