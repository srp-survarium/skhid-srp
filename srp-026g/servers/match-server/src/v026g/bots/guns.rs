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

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::{BOT_AMMO_SLOTS, BotRole, BotTier, Loadout, bot_kits};
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
}

impl Gun {
    /// [PR C] The measure's class: by its category, and the sniper
    /// category split by its bolt.
    pub fn class(&self) -> &'static str {
        match (self.category, self.handling) {
            (15, _) => "smg",
            (16, _) => "mg",
            (17, _) => "pistol",
            (14, _) => "shotgun",
            (13, Handling::Bolt) => "bolt",
            (13, _) => "marksman",
            _ => "rifle",
        }
    }
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
            aim_ms: ms(w.aim_time),
            dispersion: w.dispersion,
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
