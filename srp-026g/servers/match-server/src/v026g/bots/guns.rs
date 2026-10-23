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

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::{BOT_AMMO_SLOTS, BotRole, BotTier, Loadout, bot_kits};

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
            Handling::Burst(if w.category == SMG_CATEGORY {
                AutoClass::Smg
            } else {
                AutoClass::Rifle
            })
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
        }
    }
}

/// The submachine guns' category (`categories_dict`).
pub const SMG_CATEGORY: u8 = 15;

/// Every weapon of the game data, as the bots fire it.
pub fn guns() -> &'static [Gun] {
    static ALL: std::sync::OnceLock<Vec<Gun>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        survarium::v026g::weapons::all()
            .iter()
            .map(Gun::of)
            .collect()
    })
}

/// The facts of the weapon item `item`.
pub fn gun(item: &str) -> Option<&'static Gun> {
    guns().iter().find(|gun| gun.item == item)
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
/// (`BOT_AMMO_SLOTS`) and their storage.
fn arms_of(kit: &Loadout) -> Option<Arms> {
    let weapon = Dictionary::shared().item(kit.slot(7)?.dict_id)?;
    let ammo = kit
        .items
        .iter()
        .filter(|item| BOT_AMMO_SLOTS.contains(&item.slot));
    let rounds = ammo.clone().map(|item| item.count).sum();
    let reserve = ammo.map(|item| item.stored.max(item.count)).sum();
    Some(Arms {
        primary: (gun(&weapon.name)?, rounds),
        pistol: None,
        reserve,
    })
}

type ArmsTable = Vec<((BotTier, BotRole, usize), Option<Arms>)>;

impl Arms {
    /// The `k`-th kit (0 on) of a bot of `role` in `tier`
    /// (`loadouts::bot_kits`). No pistol: the bots carry none. `None` for
    /// a kit that is not there or a weapon not in the game data ([`guns`]).
    pub fn of_kit(tier: BotTier, role: BotRole, k: usize) -> Option<&'static Arms> {
        static ALL: std::sync::OnceLock<ArmsTable> = std::sync::OnceLock::new();
        ALL.get_or_init(|| {
            let mut table = Vec::new();
            for tier in BotTier::ALL {
                for role in BotRole::ALL {
                    for (k, kit) in bot_kits(tier, role).into_iter().enumerate() {
                        table.push(((tier, role, k), arms_of(kit)));
                    }
                }
            }
            table
        })
        .iter()
        .find(|(key, _)| *key == (tier, role, k))
        .and_then(|(_, arms)| arms.as_ref())
    }

    /// Every kit of a bot of `role` in `tier`, in its order.
    pub fn kits_of(tier: BotTier, role: BotRole) -> Vec<&'static Arms> {
        (0..role.kit_roles().len())
            .filter_map(|k| Self::of_kit(tier, role, k))
            .collect()
    }

    /// A role's (`BotRole::name`, the personality of that name) first kit
    /// in the lowest tier.
    pub fn of_role(role: &str) -> Option<&'static Arms> {
        let role = BotRole::ALL.into_iter().find(|each| each.name() == role)?;
        Self::of_kit(BotTier::Low, role, 0)
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
        // Every kit of every tier: its weapon is known, a life's rounds
        // are the kit's slot, its storage the lives' worth.
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let kits = Arms::kits_of(tier, role);
                assert_eq!(kits.len(), 3, "{tier} {role}");
                for (arms, kit_role) in kits.iter().zip(role.kit_roles()) {
                    assert_eq!(arms.primary.0.item, tier.gear().of(kit_role).weapon);
                    assert!(arms.pistol.is_none(), "the bots carry no weapon 2");
                    assert!(arms.primary.1 >= arms.primary.0.magazine, "{tier} {role}");
                    assert_eq!(
                        arms.reserve,
                        arms.primary.1 * survarium::v026g::loadouts::BOT_AMMO_LIVES
                    );
                }
            }
        }
        for role in BotRole::ALL {
            let arms = Arms::of_role(role.name()).unwrap();
            assert_eq!(arms.primary.0.item, BotTier::Low.gear().of(role).weapon);
        }
        assert!(Arms::of_role("steady").is_none());
        assert_eq!(Arms::fallback().primary.0.item, "ak_74u");
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
        let arms = Arms::of_role(role).unwrap();
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
        let support = Hands::of(Arms::of_role("support").unwrap());
        assert_eq!((support.primary.rounds, support.primary.spare), (30, 60));
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
        assert_eq!(mosin.reload(), 1_500 + 3 * 500 + 1_500);
        assert_eq!((mosin.rounds, mosin.spare), (5, 2));
        mosin.rounds = 0;
        assert!(mosin.needs_reload());
        assert_eq!(mosin.reload(), 1_500 + 2 * 500 + 1_500);
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
