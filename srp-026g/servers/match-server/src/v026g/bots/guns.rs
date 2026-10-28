//! The bots' weapons as the client has them: rounds a load, time between
//! shots, how the trigger works, how long a reload takes; and the rounds a
//! bot carries in a life. The reflex counts the rounds it fires from these
//! and reloads when the magazine is out ([`super::reflex`]), instead of
//! guessing "about a clip of fire" by time.
//!
//! Where the numbers come from (0.26g's `resources.db`; tags as in the
//! notes: PROVEN read in the data or the code, LIKELY read in 0.100b's
//! source and not traced in 0.26g's exe):
//! - `gameplay/items/weapons/wpn_<name>.options`, `parameters`:
//!   `magazine_capacity`, `rounds_per_minute`, `reload_time`,
//!   `fire_queue_types`, `show_time`, `hide_time`; and its `states`: which
//!   reload state the weapon has (303 a magazine, 306 round by round with
//!   start, one-round and finish substates, 311 two barrels, 314 a
//!   pistol's) and whether it has a `chamber_a_round` state (304). PROVEN.
//!   `magazine_capacity` is also in `survarium::v026g::item_params`, which
//!   is generated from the same files; a test holds the two together.
//! - The item clips of those states
//!   (`animations/single/weapons/<model>/item/stand/3rd_view/*.b-spline`,
//!   30 frames a second, the length as `combat/pose.rs` of srp-0100b reads
//!   it): every `shoot` clip has its one `shoot` mark at frame 0, so a
//!   shot leaves as the fire state starts (PROVEN); AK-74U reload 95 f,
//!   Uzi reload 80 f, TOZ-34 one barrel 80 f and two barrels 110 f, TT-33
//!   reload 65 f and from empty 125 f, Mosin `reload_start` 50 f,
//!   `reload_1` 20 f, `reload_end` 41 f and `chamber_a_round` 65 f. PROVEN.
//! - How those make times (0.100b's source,
//!   `srp-0100b/docs/notes/weapon_logic_timing.md` §3, §6; LIKELY the same
//!   in 0.26g): a fire clip is scaled so one pass is
//!   `60 / rounds_per_minute` s; a magazine, pistol or barrel reload is
//!   scaled by its first clip to `reload_time`, so the other clip of a
//!   pistol or a double barrel takes its length in proportion; round by
//!   round, start and finish play at their own length and the one-round
//!   loop takes `reload_time / magazine_capacity` a round; a chamber state
//!   plays at its own length. The AK-74U's reload clip, 95 f = 3.167 s,
//!   is its `reload_time` exactly. The lobby's rate of fire for the Mosin,
//!   27 a minute, is 60 s over its 2.167 s of chambering (what the lobby
//!   computes is not read).
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

use survarium::v026g::item_params::item_params;
use survarium::v026g::loadouts::bot_gear::{WeaponFacts, weapon_facts};
use survarium::v026g::loadouts::{self, BOT_AMMO_SLOTS, BotRole, Loadout, POCKET_SLOTS, SlotItem};

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

/// `frames` at 30 a second scaled from `base` frames to `base_ms`, ms.
const fn scaled(frames: u32, base: u32, base_ms: u32) -> u32 {
    (frames * base_ms + base / 2) / base
}

/// `frames` at 30 a second, ms.
const fn frames_ms(frames: u32) -> u32 {
    (frames * 1000 + 15) / 30
}

/// The five weapons the server's bots carry
/// (`survarium::v026g::loadouts`): each role's weapon 1 and the TT-33.
pub const GUNS: [Gun; 5] = [
    // `wpn_uzi`: 32 rounds, 600 a minute, fire modes [-1, 1], reload 2.3 s
    // (its clip 80 f scaled to it).
    Gun {
        item: "uzi",
        magazine: 32,
        rpm: 600,
        auto: true,
        chamber_ms: 0,
        reload: Reload::Magazine { ms: 2_300 },
        show_ms: 800,
        hide_ms: 800,
    },
    // `wpn_ak_74u`: 30 rounds, 650 a minute, fire modes [-1, 1], reload
    // 3.167 s (its clip, 95 f, is that long).
    Gun {
        item: "ak_74u",
        magazine: 30,
        rpm: 650,
        auto: true,
        chamber_ms: 0,
        reload: Reload::Magazine { ms: 3_167 },
        show_ms: 800,
        hide_ms: 800,
    },
    // `wpn_toz_34`: 2 shells, 240 a minute, fire modes [1, 2] (one barrel,
    // both), reload 2.5 s for the first barrel's clip (80 f); both barrels'
    // clip is 110 f.
    Gun {
        item: "toz_34",
        magazine: 2,
        rpm: 240,
        auto: false,
        chamber_ms: 0,
        reload: Reload::Barrels {
            one_ms: 2_500,
            two_ms: scaled(110, 80, 2_500),
        },
        show_ms: 800,
        hide_ms: 800,
    },
    // `wpn_mosin_nagant`: 5 rounds, 160 a minute, fire modes [1], a bolt
    // (`chamber_a_round`, 65 f), reload round by round: `reload_start`
    // 50 f, `reload_1` looped at 2.5 s / 5 a round, `reload_end` 41 f.
    Gun {
        item: "mosin_nagant",
        magazine: 5,
        rpm: 160,
        auto: false,
        chamber_ms: frames_ms(65),
        reload: Reload::Rounds {
            start_ms: frames_ms(50),
            round_ms: 2_500 / 5,
            finish_ms: frames_ms(41),
        },
        show_ms: 800,
        hide_ms: 800,
    },
    // `wpn_tt_33`: 8 rounds, 300 a minute, fire modes [1], reload 1.6 s
    // for its first clip (`reload`, 65 f); `reload_empty` is 125 f.
    Gun {
        item: "tt_33",
        magazine: 8,
        rpm: 300,
        auto: false,
        chamber_ms: 0,
        reload: Reload::Pistol {
            ms: 1_600,
            empty_ms: scaled(125, 65, 1_600),
        },
        show_ms: 500,
        hide_ms: 800,
    },
];

/// The facts of the weapon item `item`: [`GUNS`], else derived from its
/// config ([`derived_guns`]).
pub fn gun(item: &str) -> Option<&'static Gun> {
    GUNS.iter()
        .chain(derived_guns())
        .find(|gun| gun.item == item)
}

/// Every other weapon of the game data a bot may carry: its weapon of a
/// role's category from a player's kit (`session::v026g::bot_kits`), with
/// the facts its config gives (`loadouts::bot_gear::weapon_facts`, the
/// numbers of the module's notes), and for what the clips would give, the
/// clips of [`GUNS`]'s [GUESS: the clips are not in the server's data]: a
/// bolt or a pump is worked as the Mosin's (65 f), a round-by-round reload
/// starts and ends as the Mosin's (50 f, 41 f), two barrels from empty as
/// the TOZ-34's (110 f for its 80), and a pistol's reload from empty is a
/// quarter longer (the TT-33's is near twice: 125 f for 65).
pub fn derived_guns() -> &'static [Gun] {
    static DERIVED: std::sync::OnceLock<Vec<Gun>> = std::sync::OnceLock::new();
    DERIVED.get_or_init(|| {
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        dictionary
            .items
            .iter()
            .filter(|item| BotRole::of_weapon(item.dict_id).is_some())
            .filter(|item| GUNS.iter().all(|gun| gun.item != item.name))
            .filter_map(|item| Some(derived(&item.name, weapon_facts(item.dict_id)?)))
            .collect()
    })
}

/// A weapon's facts from its config's ([`derived_guns`]).
fn derived(item: &'static str, facts: WeaponFacts) -> Gun {
    let ms = |s: f32| (s * 1000.0).round() as u32;
    let reload_ms = ms(facts.reload_s);
    let magazine = facts.magazine.max(1);
    let reload = match facts.reload_state {
        306 => Reload::Rounds {
            start_ms: frames_ms(50),
            round_ms: reload_ms / magazine,
            finish_ms: frames_ms(41),
        },
        311 => Reload::Barrels {
            one_ms: reload_ms,
            two_ms: scaled(110, 80, reload_ms),
        },
        314 => Reload::Pistol {
            ms: reload_ms,
            empty_ms: reload_ms * 5 / 4,
        },
        _ => Reload::Magazine { ms: reload_ms },
    };
    Gun {
        item,
        magazine,
        rpm: (facts.rounds_per_minute.round() as u32).max(1),
        auto: facts.first_queue == -1,
        chamber_ms: if facts.chamber { frames_ms(65) } else { 0 },
        reload,
        show_ms: ms(facts.show_s),
        hide_ms: ms(facts.hide_s),
    }
}

/// A weapon in a life: its rounds in it and the rest of its ammo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub gun: &'static Gun,
    pub rounds: u32,
    pub spare: u32,
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

    /// A reload begun: its time, ms; the rounds counted in at once (the
    /// shots wait for the time anyway).
    pub fn reload(&mut self) -> u32 {
        let loading = self.loadable();
        let ms = self.gun.reload_ms(self.rounds, loading);
        self.rounds += loading;
        self.spare -= loading;
        ms
    }
}

/// What a bot carries: weapon 1 with its ammo, the pistol with its, and
/// what its pockets hold for its plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arms {
    pub primary: (&'static Gun, u32),
    pub pistol: Option<(&'static Gun, u32)>,
    /// Units of the drug in the first pocket (quick use 1: a medkit, or
    /// bandages in a low kit): what the heal play may use in a life, at
    /// most its tuning's `medkits`.
    pub medkits: u8,
    /// Units of the first frag grenade in the pockets: what the grenade
    /// play may throw in a life, at most its tuning's `grenades`.
    pub grenades: u8,
}

impl Arms {
    /// A bot's kit's: weapon 1 (slot 7) and every round of its ammo slots
    /// (`BOT_AMMO_SLOTS`) that fit it, a life's ammo, and its pockets'
    /// medkits and grenades. No pistol: the bots carry none. None: no
    /// weapon 1, or one [`gun`] does not know.
    pub fn of_kit(kit: &Loadout) -> Option<Arms> {
        let dictionary = survarium::v026g::dictionary::Dictionary::shared();
        let weapon = kit.slot(7)?;
        let rounds = BOT_AMMO_SLOTS
            .iter()
            .filter_map(|&slot| kit.slot(slot))
            .filter(|ammo| dictionary.compatible(weapon.dict_id, ammo.dict_id))
            .map(|ammo| ammo.count)
            .sum();
        let kind = |dict_id: u16| item_params(dict_id).map_or("", |params| params.kind);
        let units = |item: &SlotItem| item.count.min(u32::from(u8::MAX)) as u8;
        let medkits = kit
            .slot(POCKET_SLOTS[0])
            .filter(|item| kind(item.dict_id) == "drugs")
            .map_or(0, units);
        let grenades = POCKET_SLOTS
            .iter()
            .filter_map(|&slot| kit.slot(slot))
            .find(|item| kind(item.dict_id) == "grenade")
            .map_or(0, units);
        Some(Arms {
            primary: (gun(&dictionary.item(weapon.dict_id)?.name)?, rounds),
            pistol: None,
            medkits,
            grenades,
        })
    }

    /// A role's fixed kit's (`BotRole::name`, the personality of that
    /// name; `loadouts::bot_kit`): what a bot starts with, until the kits
    /// of its profile are worn (`Bots::wear`).
    pub fn of_role(role: &str) -> Option<&'static Arms> {
        static ALL: std::sync::OnceLock<Vec<(&'static str, Arms)>> = std::sync::OnceLock::new();
        ALL.get_or_init(|| {
            BotRole::ALL
                .into_iter()
                .filter_map(|role| Some((role.name(), Arms::of_kit(loadouts::bot_kit(role))?)))
                .collect()
        })
        .iter()
        .find(|(name, _)| *name == role)
        .map(|(_, arms)| arms)
    }

    /// For a bot whose personality is no role (a tuning's own): the
    /// support's, the middle of the four.
    pub fn fallback() -> &'static Arms {
        Self::of_role(BotRole::Support.name()).expect("the support's weapon is in GUNS")
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
        for gun in &GUNS {
            let item = dictionary.item_named(gun.item).unwrap();
            let params = item_params(item.dict_id).unwrap();
            assert_eq!(gun.magazine, params.magazine_capacity, "{}", gun.item);
        }
        for role in BotRole::ALL {
            let arms = Arms::of_role(role.name()).unwrap();
            assert_eq!(arms.primary.0.item, role.gear()[0].name);
            assert!(arms.pistol.is_none(), "the bots carry no weapon 2");
            // The fixed kits' pockets: two medkits, two grenades.
            assert_eq!((arms.medkits, arms.grenades), (2, 2), "{role}");
        }
        assert!(Arms::of_role("steady").is_none());
        assert_eq!(Arms::fallback().primary.0.item, "ak_74u");
        // Every weapon of a role is known, its magazine the game's; the
        // five of GUNS are theirs, not derived.
        for item in &dictionary.items {
            if BotRole::of_weapon(item.dict_id).is_none() {
                continue;
            }
            let gun = gun(&item.name).unwrap_or_else(|| panic!("{}", item.name));
            let params = item_params(item.dict_id).unwrap();
            assert_eq!(gun.magazine, params.magazine_capacity, "{}", item.name);
        }
        assert!(
            derived_guns()
                .iter()
                .all(|gun| GUNS.iter().all(|own| own.item != gun.item))
        );
        // A derived weapon of a known kind: the PPSh's magazine reload.
        let ppsh = gun("ppsh").unwrap();
        assert_eq!((ppsh.magazine, ppsh.rpm, ppsh.auto), (35, 1000, true));
        assert_eq!(ppsh.reload, Reload::Magazine { ms: 3_700 });
    }

    /// A bot's kit's arms: its weapon 1, every round of its ammo that fits
    /// it, its first pocket's drug and its grenades.
    #[test]
    fn a_kits_arms_are_its_weapon_its_rounds_and_its_pockets() {
        use survarium::v026g::loadouts::bot_gear::bot_kit_of;
        let dictionary = Dictionary::shared();
        let dict = |name: &str| dictionary.item_named(name).unwrap().dict_id;
        let kit = bot_kit_of(
            &[(3, dict("bm_back_1")), (4, dict("bm_legs_5"))],
            dict("svd"),
            dict("ammo_7.62x54_7n1"),
            BotRole::Camper,
        )
        .unwrap();
        let arms = Arms::of_kit(&kit).unwrap();
        assert_eq!(arms.primary.0.item, "svd");
        assert_eq!(arms.primary.1, 3 * BotRole::Camper.rounds_a_slot());
        assert!(arms.primary.1 >= arms.primary.0.magazine);
        assert_eq!((arms.medkits, arms.grenades), (2, 2));
        // Bare: one pocket of one unit, the medkit; no grenade.
        let bare = bot_kit_of(&[], dict("svd"), dict("ammo_7.62x54_7n1"), BotRole::Camper).unwrap();
        let arms = Arms::of_kit(&bare).unwrap();
        assert_eq!((arms.medkits, arms.grenades), (1, 0));
        // A known pistol has its arms (the TT-33 is of GUNS); no weapon 1,
        // none.
        let pistol = Loadout {
            items: vec![SlotItem {
                slot: 7,
                dict_id: dict("tt_33"),
                count: 1,
            }],
        };
        assert_eq!(
            Arms::of_kit(&pistol).map(|arms| arms.primary.0.item),
            Some("tt_33")
        );
        assert!(Arms::of_kit(&Loadout { items: Vec::new() }).is_none());
    }

    #[test]
    fn times_from_the_clips() {
        let [uzi, ak, toz, mosin, tt] = &GUNS;
        assert_eq!((uzi.shot_ms(), uzi.cycle_ms()), (100.0, 100));
        assert_eq!(ak.cycle_ms(), 93);
        assert_eq!((toz.cycle_ms(), tt.cycle_ms()), (250, 200));
        // The bolt: 375 ms of the shot and 2 167 of chambering.
        assert_eq!(mosin.cycle_ms(), 375 + 2_167);
        assert_eq!(ak.reload_ms(5, 25), 3_167);
        assert_eq!((tt.reload_ms(3, 5), tt.reload_ms(0, 8)), (1_600, 3_077));
        assert_eq!((toz.reload_ms(1, 1), toz.reload_ms(0, 2)), (2_500, 3_438));
        assert_eq!(toz.reload_ms(0, 1), 3_438);
        assert_eq!(mosin.reload_ms(0, 5), 1_667 + 2_500 + 1_367);
        assert_eq!(mosin.reload_ms(3, 2), 1_667 + 1_000 + 1_367);
        // An automatic hold: a shot at its start, one per pass begun.
        assert_eq!(ak.shots_held(50), 1);
        assert_eq!(ak.shots_held(100), 2);
        assert_eq!(ak.shots_held(600), 7);
        assert_eq!(uzi.shots_held(300), 3);
    }

    /// A role's arms with the TT-33 and a clip of its ammo as weapon 2:
    /// the switch is kept for kits that have one.
    fn with_pistol(role: &str) -> &'static Arms {
        let arms = Arms::of_role(role).unwrap();
        Box::leak(Box::new(Arms {
            primary: (arms.primary.0, 30),
            pistol: gun("tt_33").map(|gun| (gun, 50)),
            ..*arms
        }))
    }

    #[test]
    fn a_life_loads_from_its_ammo_and_reloads_what_is_left() {
        // The support: 600 rounds (three slots of 200), 30 in the
        // magazine, 570 to load.
        let support = Hands::of(Arms::of_role("support").unwrap());
        assert_eq!((support.primary.rounds, support.primary.spare), (30, 570));
        // Dry with no weapon 2: no switch, ever.
        let mut bare = Hands::of(Arms::of_role("camper").unwrap());
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
        assert_eq!(mosin.reload(), 1_667 + 3 * 500 + 1_367);
        assert_eq!((mosin.rounds, mosin.spare), (5, 2));
        mosin.rounds = 0;
        assert!(mosin.needs_reload());
        assert_eq!(mosin.reload(), 1_667 + 2 * 500 + 1_367);
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
}
