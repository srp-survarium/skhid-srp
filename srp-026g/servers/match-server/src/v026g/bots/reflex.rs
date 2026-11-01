//! The combat reflex: a bot's second track (`player_bots_research.md` B1
//! point 2, Left 4 Dead's "Main" and "Legs"). The play decides where to be
//! and why; the reflex, at every think, picks a target, aims and fires,
//! turns to a sound, and may take the legs for a moment to survive or to
//! punish (research B1: it "may override movement to survive").
//!
//! The fairness rules hold by type: the reflex aims and fires only at a
//! `know::SeenTrack`, so never at what no sense gave, and never before the
//! noticing delay (but for its own last target, noticed already and back in
//! sight a moment after he was lost: `CombatTuning::reacquire_keep`); the
//! first shot waits a reaction delay more
//! (`AimTuning::reaction_ms`). Its aim is a human's: it starts off by
//! `AimTuning::error_deg` × 3 and settles toward its error over `settle_s`,
//! wanders slowly about the target as it tracks him (`wander_deg`); the
//! error never goes to 0 (a floor drawn every tick: no perfect recoil or
//! spread). Play 23 changes only the numbers that are aim or reaction
//! (`error_deg`, `wander_deg`, `settle_s`, `notice`, `reaction_ms`),
//! through `skill`, and the pull against the kick
//! (`CombatTuning::recoil_pull_low` to `recoil_pull`); every other number of
//! [`CombatTuning`] is the same at every skill.
//!
//! How it fires and stands, from the best players of the recorded matches
//! (`player_bots_behaviour.md` §5, `player_bots_weapons.md`): each weapon
//! by its kind, read from its config ([`super::guns::Handling`],
//! [`FireTuning`]): an automatic weapon in bursts whose length falls with
//! the distance (a rifle held to 35 m and mostly tapped past 60 m, a
//! submachine gun longer), a marksman rifle, pistol or semi-automatic
//! shotgun pressed at its cycle and later at range, a bolt action pressed
//! once, then the bolt and the sights brought back on him, never held;
//! down the sights at a seen target (always from 10 m on, always with a
//! bolt action); standing to aim it steps side to side, and a hit turns the
//! step (for some targets only with a bolt or a marksman rifle, a bolt
//! action still for its shot); crouched for some targets, by kind.
//!
//! The kick: every weapon's shots lift the view itself, and the bullet
//! with it ([`super::recoil`], as the client does it). The reflex keeps
//! its weapon's kick ([`Reflex::weapon_tick`], [`Reflex::sent`]): the view
//! it aims from is the base pitch and the kick, and while it aims at a
//! target its turn takes back the rise the kick will give the view that
//! tick (`recoil_pull`), as a player pulls the mouse down; the skill's
//! error and wander stay on top.
//!
//! What it does besides aiming and firing (`player_bots_behaviour.md` §3):
//! - **play 5, punish a reload:** an enemy seen within `punish_m` or heard
//!   within `punish_heard_m` reloading or switching weapons during a duel:
//!   the legs go at him for `punish_ms`;
//! - **play 6, turn to a sound:** a shot heard, nothing seen: look there;
//!   a step or a reload heard near from outside the view: a glance there
//!   ([`CombatTuning::glance_heard_m`]); a teammate's callout near
//!   (off by default, [`CombatTuning::glance_told_m`]);
//! - **hit by a shooter it did not sense** (off by default): the HUD's hit
//!   marker gives the direction (no distance, no shooter): it takes cover
//!   from that direction, looks there a moment and calls it out to its
//!   team ([`CombatTuning::hit_bearing`]);
//! - **outranged:** a short weapon against a seen sniper beyond its band
//!   does not step in at him; it may break sight and wait instead (off by
//!   default, [`CombatTuning::outranged_cover`]);
//! - **play 7, break sight and re-peek:** hit while in a duel: to a spot
//!   near by hidden from the enemy, wait 1-2 s, come back at a spot that
//!   sees him from another angle, never the one it last fought from;
//! - **play 10's other half:** between seen targets, a wounded one counts
//!   nearer (`wounded_bonus_m`);
//! - **worth ([`super::worth`]):** a seen carrier (his hands hold the battery), above all a thief of the
//!   team's battery, counts nearer still and takes the target from a plainer one at once; a wounded one is
//!   read from what the team saw land (every pool, pain included, mended as the game mends), not from his
//!   true body pool;
//! - **finish the hurt:** a bot that fired at a hurt enemy who broke sight within 30 m goes on his last
//!   place for a few seconds, two bots at most ([`Reflex::finish`]);
//! - **flank glance:** holding a target, a teammate's callout of an enemy outside the view is looked at
//!   for a moment ([`Reflex::flank_glance`]);
//! - **play 11, weapon matchup:** the distance band its weapon wants
//!   against the enemy's ([`preferred_band`]), stepped into while it duels;
//! - **play 19, spot:** a newly noticed enemy is spotted (bit 28, key down
//!   once, 3 s apart: the stock game's cooldown, kept as the bots' own
//!   pace since client patch 38 has none) once it is in the crosshair;
//! - **play 20, grenade before the peek:** rarely, at an enemy last seen
//!   holding an angle or at a place an enemy was sensed staying at, with
//!   grenades left (two a life), the grenade key held until the throw. The
//!   throw is the client's (`arc.rs`): aimed so that it comes down at the
//!   place, traced on the bullet mesh before the press and again at it, and
//!   not made where it lands in the thrower's own blast or a teammate's;
//!   the look is held, and the legs stand, until the grenade has left the
//!   hand;
//! - **play 21, trade (its reflex half):** the partner died within
//!   `trade_ms`: look at that spot, aiming down the sights.

use mapdata::Vec3;
use serde::{Deserialize, Serialize};
use survarium::v026g::weapons::Optic;

use super::aim_settle::{self, Settle, SettleGuns};
use super::arc;
use super::guns::{
    Arms, AutoClass, Gun, Handling, Hands, LoopEnd, MG_CATEGORY, ModCounts, PISTOL_CATEGORY,
    RIFLE_CATEGORY, SHOTGUN_CATEGORY, SMG_CATEGORY, SNIPER_CATEGORY, WeaponClass,
};
use super::kin::{forward, wrap, yaw_of};
use super::pitch::{Pitch, PitchScale};
use super::aimpoint::PartsLook;
use super::know::{LookOutcome, Memory, Miss, SeenTrack, Source, TeamKnowledge};
use super::plays::heal::Wish as HealWish;
use super::recoil::{Cadence, Recoil};
use super::rng::Rng;
use super::signals::mix;
use super::hazards::ARRIVE_MPS;
use super::route_filter::RouteFilter;
use super::tactics::{SpotQuery, Tactics};
use super::tuning::{AimTuning, SenseTuning};
use super::view::{CHEST, MatchView, Place, PlaceView, SoundKind, still_eye};
use super::worth::{Known, Worth, WorthStats, WorthTuning};
use crate::v026g::drive::key;

/// A weapon's kind as the matchup (play 11) reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponKind {
    Pistol,
    Smg,
    Shotgun,
    Rifle,
    Sniper,
}

impl WeaponKind {
    /// A server bot's weapon 1 by its role (`survarium::v026g::loadouts::
    /// BotRole::name`): the Uzi, the AK-74U, the TOZ-34, the Mosin.
    pub fn of_role(role: &str) -> Option<Self> {
        Some(match role {
            "flanker" => Self::Smg,
            "support" => Self::Rifle,
            "rusher" => Self::Shotgun,
            "camper" => Self::Sniper,
            _ => return None,
        })
    }

    /// A weapon item's kind from its dictionary category and its
    /// `item_params` kind (`wpn_*`): category 13 is the sniper category
    /// and 14 the shotguns' (`loadouts.rs` tests); else by the kind (an
    /// SVD is `wpn_aslt` in the sniper category, so the category goes
    /// first). A kind this code does not know (a patch's new `wpn_*`
    /// type) takes its category's: [`Self::of_category`], so a new weapon
    /// has a band and is not one without a kind.
    pub fn of_item(kind: &str, category: u8) -> Option<Self> {
        Self::by_type(kind, category).or_else(|| Self::of_category(category))
    }

    /// The kind by the category and the `wpn_*` types this code knows; none
    /// for a type it does not.
    fn by_type(kind: &str, category: u8) -> Option<Self> {
        Some(match (category, kind) {
            (SNIPER_CATEGORY, _) => Self::Sniper,
            (SHOTGUN_CATEGORY, _) => Self::Shotgun,
            (_, "wpn_dbrl" | "wpn_stgn" | "wpn_asgn") => Self::Shotgun,
            (_, "wpn_bolt") => Self::Sniper,
            (_, "wpn_smg") => Self::Smg,
            (_, "wpn_pstl" | "wpn_rvlr" | "wpn_apst") => Self::Pistol,
            (_, "wpn_aslt") => Self::Rifle,
            _ => return None,
        })
    }

    /// The kind of a weapon of class `class`: the matchup's five kinds. A
    /// machine gun takes the rifles' band; the marksman and bolt classes the
    /// snipers'.
    pub fn of_class(class: WeaponClass) -> Self {
        match class {
            WeaponClass::Pistol => Self::Pistol,
            WeaponClass::Smg => Self::Smg,
            WeaponClass::Shotgun => Self::Shotgun,
            WeaponClass::Rifle | WeaponClass::MachineGun => Self::Rifle,
            WeaponClass::Marksman | WeaponClass::Bolt => Self::Sniper,
        }
    }

    /// The kind of this weapon came from its category, because its type is
    /// none this code knows ([`Self::of_item`]'s fallback): 0 of the 83 weapons
    /// of the live data. The count of these is the fallback's activity.
    pub fn by_fallback(kind: &str, category: u8) -> bool {
        Self::by_type(kind, category).is_none() && Self::of_category(category).is_some()
    }

    /// The kind a weapon of the dictionary category `category` has when
    /// its `wpn_*` type is none this code knows: the kind most of the
    /// category's weapons have (the 83 weapons of the live data: 8 rifles,
    /// 13 snipers, 14 shotguns, 15 SMGs, 16 machine guns taking a rifle's
    /// band, 17 pistols). `None` for a category that holds no weapons.
    pub fn of_category(category: u8) -> Option<Self> {
        Some(match category {
            RIFLE_CATEGORY | MG_CATEGORY => Self::Rifle,
            SNIPER_CATEGORY => Self::Sniper,
            SHOTGUN_CATEGORY => Self::Shotgun,
            SMG_CATEGORY => Self::Smg,
            PISTOL_CATEGORY => Self::Pistol,
            _ => return None,
        })
    }
}

/// Whether the aim's error angles scale with the zoom of the sights, as a player's hand does (the client turns the view by
/// the sensitivity times the camera's zoom, 0x240220: PROVEN, `client_patch.md` 531): a bench setting to size the effect,
/// off in the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoomError {
    /// The error angles are the skill's whatever the sights (today).
    #[default]
    Off,
    /// Times the zoom of a scope (`fov_factor`, 0.25) while aimed in, ramped by the aim progress; iron sights unchanged.
    Scopes,
    /// As [`Self::Scopes`], and the iron sights' zoom (`1 / aim_zoom_factor`) too.
    All,
}

impl ZoomError {
    /// The factor on the error angles with the sights `optic` at aim progress `progress` (0 not aimed, 1 aimed in): the
    /// camera's zoom `1 + (target - 1) x progress`.
    pub fn factor(self, optic: Optic, progress: f32) -> f32 {
        let target = match (self, optic) {
            (Self::Off, _) | (Self::Scopes, Optic::Iron { .. }) => 1.0,
            (Self::Scopes | Self::All, Optic::Scope { zoom }) | (Self::All, Optic::Iron { zoom }) => zoom,
        };
        1.0 + (target - 1.0) * progress.clamp(0.0, 1.0)
    }
}

/// [close] How a roll that has only close places is paced (`CombatTuning::grenade_close_pace`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosePace {
    /// The wait for the next roll, ms.
    pub roll_ms: u32,
    /// The chance a roll says yes.
    pub chance: f32,
}

/// The reflex's numbers besides the aim (`AimTuning::combat`). None of
/// them is mixed by skill but the pull against the kick (`recoil_pull_low`
/// to `recoil_pull`): play 23 changes only aim and reaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CombatTuning {
    /// Play 10: between seen targets, one at no health counts this much
    /// nearer (half health, half of it), metres.
    pub wounded_bonus_m: f32,
    /// A duel lasts this long after the last target or hit, ms.
    pub duel_ms: u32,
    /// Play 5: a reload or switch seen within `punish_m`, or heard within
    /// `punish_heard_m` (the sense's reload range × the hearing cheat),
    /// at most `punish_window_ms` old, sends the legs at the enemy for
    /// `punish_ms`, with this chance.
    pub punish_m: f32,
    pub punish_heard_m: f32,
    pub punish_window_ms: u32,
    pub punish_ms: u32,
    pub punish_chance: f32,
    /// Play 7: a health drop of at least this since the last think is a
    /// hit.
    pub hit_drop: f32,
    /// A hiding spot this far from the bot, metres.
    pub hide_m: (f32, f32),
    /// Waited there, ms (drawn).
    pub hide_wait_ms: (u32, u32),
    /// Not there by this, ms: given up.
    pub hide_give_up_ms: u32,
    /// The re-peek spot this far from the hiding spot, metres.
    pub peek_m: (f32, f32),
    /// Another angle than the last peek's: its bearing from the enemy
    /// differs by `repeek_min_deg`, or it lies `repeek_min_m` from it.
    pub repeek_min_deg: f32,
    pub repeek_min_m: f32,
    /// Going to it at most this long, ms.
    pub peek_ms: u32,
    /// Play 11: the distance band each weapon wants, metres. The SMG's, the
    /// rifle's, the sniper's and the shotgun's from where the top players
    /// fight with them (matches of 6+ players, 2026-10-16..22: SMG 17/26/38
    /// m, rifle 21/33/45, bolt action 45/65/85, shotgun 12/19/27, the
    /// quartiles; `docs/notes/player_bots_weapons.md`).
    pub band_pistol: (f32, f32),
    pub band_smg: (f32, f32),
    pub band_shotgun: (f32, f32),
    pub band_rifle: (f32, f32),
    pub band_sniper: (f32, f32),
    /// Keep an enemy shotgun at least this far; close on an enemy sniper
    /// to at most this, metres.
    pub keep_from_shotgun_m: f32,
    pub close_on_sniper_m: f32,
    /// Out of the band by more than this, it steps `band_step_m` back in
    /// (only with `band_legs`).
    pub band_slack_m: f32,
    pub band_step_m: f32,
    /// The range step is taken across the line to him by this angle, degrees (none: along it, as it was): a step in or back
    /// out of the band that goes to one side of the line, longer by 1 / cos so that the way in or out is the same, and
    /// the side the other way at every new step (the bench's bots crossed the shooter's line at 1.07 m/s while a range
    /// step ran, the live humans' aimed-in 1.95: `gun_shots` by_driver, register #24). The step the side's floor refuses
    /// goes to the other side, else along the line.
    pub band_cross_deg: Option<f32>,
    /// Aimed in and firing at a target in range, it stands on its feet and steps to the side only (no forward or back from its
    /// route or a range step), for `stand_strafe_hold_ms` after the last tick the fight was on; then the play's way goes on
    /// ([`Gait`]). The live players' side-key shots are 69 % pure side steps, the bench bots' 18 % (`gun_shots` aimed_run:
    /// 82 % of theirs walk forward or back as well, and a diagonal crosses the line at 0.7 of its ground speed), register #24.
    pub stand_strafe: bool,
    pub stand_strafe_hold_ms: u32,
    pub band_legs: bool,
    /// Play 19: the bots' own spacing of spots, ms (the stock game's 3 s
    /// cooldown; client patch 38 has none); the same enemy is spotted
    /// again only after `spot_again_ms`; spotted once the view is within
    /// `spot_within_deg` of him; with this chance per new enemy.
    pub spot_cooldown_ms: u32,
    pub spot_again_ms: u32,
    pub spot_within_deg: f32,
    pub spot_chance: f32,
    /// Play 20: grenades in the kit per life (`loadouts::BOT_GRENADE`).
    pub grenades: u8,
    /// A throw is weighed this often while nothing is seen, and made with
    /// this chance.
    pub grenade_roll_ms: u32,
    pub grenade_chance: f32,
    /// At a place this far, metres. The least is the landing's floor too:
    /// the RGD-5's blast reaches 10 m (`splash_radius`, PROVEN) and the
    /// thrower is to stand 2 m outside it. The most is under the longest
    /// throw ([`arc::max_range`], about 30 m from the eye). The throw is
    /// aimed at the place ([`arc::pitch_for`]), and not made where no pitch
    /// lands it.
    pub grenade_m: (f32, f32),
    /// The key held this long once the view is on the throw, ms (drawn).
    pub grenade_hold_ms: (u32, u32),
    pub grenade_within_deg: f32,
    /// Not with a teammate this near the blast, metres: the blast may be
    /// anywhere from the first contact to [`arc::ROLL_P80_M`] along the
    /// grenade's heading, or at its rest point ([`arc::Landing`]), where it
    /// was the place the throw was at. GUESS: the RGD-5's blast
    /// (`splash_radius` 10 m, damage 1.3 to 0.05 of a body's health from its
    /// middle to its edge) falls under half a body's health at 6.4 m if it
    /// falls evenly.
    pub grenade_friend_m: f32,
    /// [play 20] The teammates are checked against the blast at the press
    /// and every tick of the hold too, where their walk takes them
    /// (`grenade_lead_s`), not only at the decision where they stand; a
    /// teammate in it then: the throw given up, or the key let go before
    /// the throw starts (the client throws only once its weapon reaches the
    /// throw state, 0x2D8E05: `player_bots.md`). Match 0x60aeff4f: 2 of the
    /// 5 team kills were grenades.
    pub grenade_recheck: bool,
    /// How far ahead a teammate's velocity is followed, s: to the blast,
    /// 4 s after the press (`ttl_ms`, PROVEN: 80 of 82 recorded blasts at
    /// the press + 4000 ms), where the bots know a teammate by his place
    /// and velocity alone (a bot's route is not shared). The whole line
    /// counts, as he may stop on it. GUESS: a straight walk of 4 s is a
    /// cautious reading (it was 1.5 s, when the blast was thought to follow
    /// the let-go by 4 s).
    pub grenade_lead_s: f32,
    /// [play 20] The throw is traced on the bullet mesh ([`arc`]), aimed to
    /// come down at the place, and not made when its first contact is
    /// within `grenade_land_m` of the thrower or a wall near him
    /// (`grenade_wall_m`), off the place by over `grenade_off_m`, or when a
    /// teammate may be in its blast. Off: the aim is the old flat line
    /// raised by `1.0 + 0.12 d` (a pitch of 2.6 to 5.7 degrees, a first
    /// contact 10.7 to 12.5 m out whatever the distance) and the only check
    /// is a teammate near the place. The look is held and the legs stand
    /// from the press to the launch either way.
    pub grenade_trace: bool,
    /// The grenade leaves the hand this long after the key goes down, ms:
    /// the throw animation's event (0x310680 -> 0x33A770), not the key. The
    /// view that counts is the one then. LIKELY: a fit to 40 recorded
    /// throws, +-200 ms (`arc::LAUNCH_DELAY_S`).
    pub grenade_launch_ms: u32,
    /// The look is held, and the legs stand, this long after the launch,
    /// ms. GUESS: the fit's error and a tick more.
    pub grenade_look_after_ms: u32,
    /// The first contact is at least this far from the thrower, metres
    /// (his chest; or the rest point). The blast's 10 m (PROVEN) and 2 m
    /// more (GUESS: the hand's place, the guessed 0.3 m under the eye, and
    /// the view's error are metres of range).
    pub grenade_land_m: f32,
    /// A first contact on a steep wall ([`arc::Contact::is_wall`]) beyond
    /// `grenade_land_m` and nearer than this to the thrower: not thrown,
    /// metres (a wall or a ceiling within `grenade_land_m` is the
    /// thrower's blast already). GUESS: the grenade falls at a wall's foot
    /// (restitution 0, PROVEN), it does not come back as in YaPB, which
    /// refuses a steep wall hit outright; kept as a margin for the model's
    /// hand and edge errors. In the smoke it decided none of 19 refused
    /// places (17 were the thrower's blast, 1 the place, 1 a floor);
    /// `grenade_wall_m=0` turns it off for a paired bench.
    pub grenade_wall_m: f32,
    /// The first contact farther than this from the place the throw is at:
    /// not thrown, metres (the arc met something on its way). GUESS: the
    /// blast's lethal reach is 2.4 m and its 10 m splash is wide, the
    /// roll's median is 2 m.
    pub grenade_off_m: f32,
    /// [A187 R31] ... and, when set, the first contact's height more than this from the place's: not thrown, metres (the arc
    /// came down on another storey or a ledge; `grenade_off_m` is on the ground plane only). A place is a body's feet or a spot's
    /// floor; a floor is 3 m up. Of 151 smoke blasts 42 (28 %) went off more than 2.5 m above the place they were thrown at
    /// (p75 +2.9 m, p90 +4.9 m). None: not asked (the throw as it was).
    ///
    /// Bench (smoke preset, 10 a side, 16 maps, seeds 1-8, the team with 2.5 m against the team without, sides swapped, 128 pairs, built on
    /// the fixed tally of #859): the switch holds back the throws that would land on another storey (82 against 35 "lands off the place"
    /// in 64 pairs, 414 throws against 476) and the team takes 20 % less `damage other` (144.1 against 180.6, z -3.0), but the outcome is
    /// not moved: score +0.6 % (z +0.4), kills +0.9 %, deaths -1.0 %, and the by-side table disagrees (side A +11.8, side B -10.0). OFF.
    pub grenade_off_y_m: Option<f32>,
    /// A teammate this near the line the grenade may run on, the first
    /// contact to [`arc::ROLL_WORST_M`] along its heading, and so on it:
    /// not thrown, metres. One in five blasts lay 8 m or more beyond the
    /// first contact, up to 16 m on the flat model (LIKELY: the recorded
    /// blasts), which `grenade_friend_m` around the P80 end does not reach;
    /// GUESS: a body's width and the grenade's.
    pub grenade_lane_m: f32,
    /// How many places a roll weighs, nearest first, before it gives the
    /// throw up: the first that passes every check is thrown at, none that
    /// fails is loosened. One arc trace a weighing. GUESS: the places are
    /// few (a held angle and a camp or two) and the nearest is often
    /// behind a wall or a ceiling near the thrower (18 of 31 in a smoke);
    /// YaPB tries up to 12 predicted places for the same reason. 1: the
    /// nearest only.
    pub grenade_places: u32,
    /// [G1] The places of a roll include where an enemy was last sensed by
    /// the bot's or its team's own senses (sight, hearing, a callout) when
    /// it is not in sight now: a target just lost, a sound. Off: only held
    /// angles and camps, as before. Why (`grenades-record.md` 2.3, PROVEN
    /// counts of 8,090 human frag throws): humans threw 54 % of them with
    /// no enemy in sight in the 5 s before, and 73 % of those that landed
    /// within 10 m of an enemy at an enemy not seen in 5 s; the rate was
    /// flat at 1.0-1.3 a minute with an enemy within 8-30 m. The sources of
    /// the numbers below: a human figure (H), a client number (C), GUESS
    /// (G, to be swept by the bench).
    pub grenade_tracks: bool,
    /// ... sensed no longer ago than this, ms: 4 s. H, LIKELY: of the
    /// aimed human throws whose aimed-at enemy had been seen in the
    /// previous 5 s, 90 % saw him within 3 s (record 9); Half-Life 2's
    /// Combine soldiers flush up to 3 s. Swept: 3 s and 6 s arms.
    pub grenade_track_ms: u32,
    /// ... no nearer than this, ms: 500. G: `Memory::seen` takes 400 ms, so
    /// a sensed place younger than that is a sight, not a place. Not
    /// swept: it only separates a target from a place by a tick or two.
    pub grenade_track_gap_ms: u32,
    /// ... with an error of at most this, metres: 8. G: inside the blast's
    /// 10 m (C, PROVEN); a track's error grows 0.5 m a step once it fades
    /// (`Memory::update`). Swept: 5 m and 12 m arms (the bench's
    /// `places` and refusals by source say what it admits).
    pub grenade_track_error_m: f32,
    /// ... within 3 m of a held angle or a camp is that place already:
    /// `camp_m`, the camp's own radius (C: the existing 3 m).
    ///
    /// The chance of a roll whose nearest place is such a track
    /// (`grenade_chance` 0.15 for a held angle or a camp): 0.08. H,
    /// LIKELY: humans threw 1.0-1.3 grenades a minute with an enemy within
    /// 8-30 m, 0.067-0.087 a 4 s roll with a place always at hand; kept at
    /// the top because a bot has a place only part of the time. Swept:
    /// 0.04 and 0.15 arms.
    pub grenade_track_chance: f32,
    /// [A170] A grenade at a held threat spot (`threat::HeldLine`): a bot with a frag, `grenade_m` from a held spot and
    /// outside its lanes, throws at the spot while the team sensed its shooter on it in the last
    /// `threat::SENSED_MS` (the 5 s rule: team knowledge only, never his live position), with this chance at a roll. The
    /// throw is weighed as any other (the thrower's blast, the mate rule, the arc) and the team's claim keeps the whole
    /// team from emptying its frags on one camper (`grenade_claim_m` / `grenade_claim_ms`). No push after the blast: it
    /// would go into his lanes. On at 1.0 (A170: 114 frags in 56 camper matches, 32 % of the blasts within 10 m of the sensed shooter,
    /// no own-side harm, no change in the match). `None`: no throw at a spot.
    pub grenade_at_held: Option<f32>,
    /// [A170] Of the held-spot rule (only with `grenade_at_held`): a throw at a spot is given up, before the key and in the hold,
    /// when the team's picture no longer holds the spot (its shooter died, a teammate cleared it, the sense of him ran out): the
    /// kill feed and the team's own knowledge, as the roll used. The frag is kept. Off: a throw goes on at a spot the team has dropped
    /// (34-38 of 220 held-spot frags left at a shooter dead 0.2-5.7 s before).
    pub grenade_held_recheck: bool,
    /// [A170] Of the held-spot rule (only with `grenade_at_held`): a held spot is a place out to the frag's real reach from where the
    /// thrower stands (`arc::Grenade::max_range` from the eye over the spot's ground: the game data's throw speed and fuse,
    /// gravity 10; about 30 m for the RGD-5) instead of `grenade_m`'s 27 m. The arc search still refuses what no arc reaches.
    pub grenade_held_reach: bool,
    /// [G2] A place whose flat arc is refused (the thrower's blast, a
    /// steep wall near him, off the place) is tried with other arcs: other
    /// pitches at the same yaw (a lob over what is in the way, 20-29 m
    /// out only: C, PROVEN, the pitch is clamped at 57 degrees), then other
    /// yaws up to 30 degrees, coarse to fine, at most
    /// `grenade_search_traces` traces a place; the first that passes every
    /// check is the throw, its landing within `grenade_search_off_m` of the
    /// place. Off: the flat arc only. H, LIKELY: at a real target 12-27 m
    /// away (3,392 human throws with an enemy there) the flat arc passed
    /// 18 % of the places, a pitch search 45 % of the rest, a pitch and
    /// yaw search 75 % (`grenades-record.md` 4). The search grid: pitches
    /// from -10 degrees (H: the human launch pitch's P10 is -4.6), steps
    /// of 3 degrees at the yaw of the place, 6 degrees at the others; yaws
    /// 6, 12, 20, 30 degrees either way (G: coarse enough for the trace
    /// cap; the first arc found is the one nearest the flat pitch, so the
    /// grid only decides what is findable, not what is preferred: not
    /// swept).
    pub grenade_search: bool,
    /// ... the landing within this of the place, metres: 5. G: the blast
    /// is linear, 0.68 of a body's health at 5 m, 0.30 at 8 m, lethal
    /// within 2.4 m (C, PROVEN); the flat arc's own limit is `grenade_off_m`
    /// 8 (the existing one). The recordings have no per-throw target, so
    /// there is no human figure. Swept: 8 m and 3 m arms.
    pub grenade_search_off_m: f32,
    /// ... at most this many traces a place: 64. G, a CPU cap: the full
    /// pitch x yaw scan took ~10 ms (release, the analysis probe); 64 is
    /// ~3 ms. Swept: 32 and 128 arms, with ms/s and poll tails paired.
    pub grenade_search_traces: u32,
    /// [G1] The team's claim on throw spots: a roll skips a place within
    /// `grenade_claim_m` of a place a teammate decided to throw at
    /// (`TeamKnowledge::claim_throw`) in the last `grenade_claim_ms`; the
    /// throw's own volley is not held against it. Its own switch, so the
    /// bench shows same-spot throws (`grenade_team_same_spot`) with and
    /// without it. The owner: do not make several bots throw at the same
    /// spots. H, LIKELY (229 matches, `grenades-record.md` 9): human
    /// same-team throws land within 8 m of a teammate's landing in the next
    /// 5 s in 1.7 % of throws (2.9 % in 10 s), twice the level by chance in
    /// the same match: they neither avoid nor seek it; the claim keeps the
    /// bots at 0, under the humans'.
    pub grenade_claim: bool,
    /// A roll for a grenade is made only if the kit holds a frag now (`Throws::frags_left` of the live list): the throw key
    /// throws from the first pocket holding any grenade (0x2FE8B0, PROVEN), so a bot of a role that carries none (the trap
    /// roles) rolled throws that do nothing, in a stance of `grenade_hold_ms` with the view turned off the fight, and with a
    /// smoke in the kit would have thrown the smoke at a frag target. A kit not read (the bench) goes by `grenades` as
    /// before. Off: the roll goes by `grenades` alone.
    pub grenade_kit_cap: bool,
    /// ... 8 m: C, PROVEN: the blast's damage at 8 m is 0.3 of a body's
    /// health (linear to 10 m). G for the exact value; swept: 5 m and 12 m.
    pub grenade_claim_m: f32,
    /// ... 6 s: C, PROVEN: the fuse is 4 s from the press; G: 2 s for the
    /// flush that follows. Swept: 4 s and 10 s.
    pub grenade_claim_ms: u32,
    /// [G2] At the press and in the hold the trace is made again from where
    /// the thrower stands now; when the chosen arc fails then for what is in
    /// the way (the thrower moved since the roll: 451 of the 540 throws given
    /// up in look 1 of the combined arm were that) the arc is searched again
    /// for the same place, and the view turns to it, instead of the throw
    /// being given up. Off: given up, as before. Swept: this switch.
    pub grenade_repick: bool,
    /// After a throw the legs go at its place from the blast for 2 s
    /// (`grenade_push_after_blast`) also when the place came from a track
    /// (G1: where an enemy was last sensed), as for a held angle or a camp.
    /// The push was built for an enemy that stays. Off: no push after a
    /// throw at a track's place. On (as before): look 1 of the combined arm
    /// had reversals +3.2 % (z 2.3), in 10v10 only; the push is the
    /// candidate cause (`grenades-record.md` 13). Swept: this switch.
    pub grenade_push_tracks: bool,
    /// [C20 volley] After a throw is let go, a second grenade at the same
    /// place while the first is still in the air or on the ground: from
    /// `grenade_volley_ms.0` after the let-go until `.1` after it or the
    /// first one's blast, whichever is first, a roll every
    /// `grenade_volley_every_ms` at `grenade_volley_chance`, with the
    /// first throw's own checks (no target in view, a clear blast, the
    /// thrower's own blast floor), and the grenades of the kit left
    /// (`grenades`). Root cause of the few second throws (57 of 1,269 in
    /// the G1 + G2 + claim arm, look 1): nothing in the roll knows that a
    /// throw was just made, so a second one is decided as a first, after
    /// `grenade_roll_ms`, a chance of 0.15 and a place that passes again.
    /// Humans threw a second within 6 s in 71 % of the lives that threw two
    /// (the gap's P50 is 3.0 s; `grenades-record.md` 15, `c20-design.md`
    /// 1). Off: as before. Swept: this switch, and the chance (0.2, 0.35).
    pub grenade_volley: bool,
    /// ... the chance of a volley roll: GUESS, a share of the human second
    /// throw (about half of the lives that threw two did within 3 s, less
    /// the repeated pulls that are not throws).
    pub grenade_volley_chance: f32,
    /// ... the window after the let-go, ms: the second press can follow the
    /// first about 1.5 s after it (the launch animation is 1.1 s, LIKELY;
    /// the data's P10 gap is 1.6 s), and 6 s covers the lives that threw
    /// two within 6 s.
    pub grenade_volley_ms: (u32, u32),
    /// ... a roll this often, ms.
    pub grenade_volley_every_ms: u32,
    /// [A187 #10] The second grenade by evidence (only with `grenade_volley`): a roll in the window throws only when an enemy has
    /// been sensed since the first was let go within `grenade_volley_near_m` of its place (a track of his, not in sight now, no
    /// older than `grenade_volley_age_ms`), and at his place (a lost or heard one), not at the first's; with none the roll is no
    /// throw and the frag is kept. The window runs to `grenade_volley_ms.1` after the let-go, past the first blast. Root cause of
    /// the volley adding nothing (A181 #10, 4 seeds, 16 maps): it threw at the first's place 3.4 s later whatever had happened
    /// there; the humans' second throw (5,101 lives with a throw, 2,054 pairs within 6 s) kills 14.7 per 100 when an enemy is within
    /// 10 m of the blast and 4.5 when none is, 18.9 against 3.8 at the gaps of 3-4 s. Off: the volley as it was.
    ///
    /// Bench (A187 #10, smoke preset, 10 a side, 16 maps; OFF is the verdict, so both switches stay off): the enemy of the bench
    /// is a bot that dodges (it moves a median 6.1-6.9 m between the two blasts, the humans' enemy 1.9 m) and the second grenade
    /// lands on empty ground. By side (the team with the switch against the team without, sides swapped, 64 pairs): targets with
    /// the avoidance on, score z +0.2 (D +0.74, SE 3.23); targets without it, a human stand-in, z +0.4 (D +1.17, SE 2.73): not
    /// settled either way. Six seeds, avoidance off for both: enemy kills per 100 throws 11.6 without the volley, 9.5 with it
    /// (the same place), 8.5 with the evidence; the thrower killed 0.2 / 0.0 / 0.4 and mates 1.3 / 0.8 / 0.8 per 100 throws. Against
    /// a still hidden camper (64 map-runs an arm): 0 camper deaths at blasts in all three arms, second grenades near him 1 / 2 / 0;
    /// the limit there is the first grenade's place.
    pub grenade_volley_track: bool,
    /// ... the evidence is within this of the first grenade's place, metres (the blast's reach).
    pub grenade_volley_near_m: f32,
    /// ... and is no older than this, ms.
    pub grenade_volley_age_ms: u32,
    /// [C20 G3] The thrower's own blast floor (`grenade_land_m`, 12 m) is
    /// lowered to this for a place whose first contact is nearer than the
    /// floor and not nearer than this, when the thrower can walk out of the
    /// blast after the launch: the way out the avoidance (`grenade_avoid`)
    /// takes for any grenade it knows of, [`Tactics::step_out`], to a floor
    /// `AvoidTuning::margin_m` past the blast's radius within the walk
    /// that is left (`ARRIVE_MPS` for the fuse less the launch and the
    /// slower of the look held and the told reaction). Needs
    /// `grenade_avoid` on (no retreat is known otherwise): with it off the
    /// floor stays. At `grenade_land_m` or above: off. Humans threw 60 % of
    /// their grenades inside 12 m of the thrower, and the thrower died at
    /// the blast in 0.83 of 100 of them (`grenades-record.md` 2.4); the
    /// bots' own floor refused 1,738 of about 2,500 weighed places. Swept:
    /// 9 m and 6 m.
    pub grenade_near_m: f32,
    /// [close] A grenade at a place under the floor's range, 4-12 m, that an enemy not in sight was sensed at (a lost or heard
    /// track, a camp, a held spot), with no hp lost by a teammate or the thrower, by construction: the arc lands not nearer the
    /// thrower than `grenade_close_m.0`; the thrower has a way out (the avoidance's own walk, never toward the landing) to a
    /// floor `grenade_close_margin_m` past the blast's radius from the rest point and from any point of the roll's worst case;
    /// every teammate was seen within `grenade_close_fresh_ms` and, predicted along his velocity for `grenade_lead_s`, is
    /// that far (radius and the margin) from the rest point and the roll's worst case, with no credit for a wall. The places
    /// of `grenade_m` are thrown at first. Needs `grenade_avoid` on (no way out is known otherwise). Humans threw 14.5 % of
    /// their grenades at an enemy under 12 m (9.3 enemy kills, 1.0 teammates and 1.2 throwers killed per 100); off.
    pub grenade_close: bool,
    /// ... the place's ground distance from the thrower, m: from, to.
    pub grenade_close_m: (f32, f32),
    /// ... past the blast's radius, m (the way out's floor, a teammate).
    pub grenade_close_margin_m: f32,
    /// ... a teammate's position is no older than this, ms: an older one refuses the throw.
    pub grenade_close_fresh_ms: u32,
    /// ... a roll that has only close places waits this long for the next, ms, and says yes at this chance (none: the roll's own
    /// `grenade_roll_ms` and chance). An enemy 4-12 m off out of sight is not where the track says 4 s later, and a "no" of the
    /// chance costs the whole lockout.
    pub grenade_close_pace: Option<ClosePace>,
    /// The view not on the throw by this, ms: given up.
    pub grenade_give_up_ms: u32,
    /// After a throw, the legs go at the place from/until this, ms.
    pub grenade_push_ms: (u32, u32),
    /// The legs' push after a throw counts from the blast (the grenade's
    /// `ttl_ms` after the press: the fuse starts with the press, not the
    /// let-go, PROVEN), not from the let-go: the RGD-5 goes off 4 s after
    /// the press (`rgd-5.options`), and a push from 1.5 s walked the
    /// thrower to within 3 m of it before it did.
    pub grenade_push_after_blast: bool,
    /// ... and this long after the blast, ms. GUESS: the blast's own
    /// moment and a tick.
    pub grenade_push_after_ms: u32,
    /// An enemy seen crouched, aiming and slower than this (m/s) holds an
    /// angle; remembered this long, ms.
    pub held_speed: f32,
    pub held_memory_ms: u32,
    /// An enemy sensed this long within `camp_m` (plus the sense's error)
    /// of one place camps there, ms.
    pub camp_ms: u32,
    pub camp_m: f32,
    /// Play 21: the partner's death spot is looked at this long, ms.
    pub trade_ms: u32,
    /// ... if it lies within this, metres (farther, the bot could do
    /// nothing there; a shot heard, play 6, comes first).
    pub trade_m: f32,
    /// A tactical reload: no enemy seen for `reload_idle_ms` and at least
    /// this share of the magazine fired (one round at least). The reload
    /// of an empty weapon needs no number: its rounds are counted
    /// ([`super::guns`]), and it comes at the first moment with no seen
    /// target or out of sight (breaking sight first when a target is
    /// seen); its time is the weapon's. The top players reload soon after
    /// a burst (41 % of their SMG bursts within 3 s, a median 300 ms after;
    /// 1.6 reloads a minute against 1.0 of the others).
    pub reload_idle_ms: u32,
    pub reload_some_share: f32,
    /// A round-by-round reload (the SPAS-12, the pumps, the bolt rifles that
    /// load by the round) is loaded with the sights down: the client ends
    /// the loop at the first round-end with them or the trigger held and a
    /// round in the weapon (0x339EE0, 0x339EA0; PROVEN in the exe), and the
    /// reflex's own AIM did that to 38 % of the SPAS-12's reloads (PROVEN by
    /// counts, eight matches). Off: the sights go as the branches ask.
    pub reload_aim_control: bool,
    /// ... except that with a target seen or a threat on, the sights can be
    /// pressed to end the loop at this many rounds put in (the rounds that
    /// load are counted by the client's own clock, [`super::guns::Load`]):
    /// the weapon is ready `finish` after that round (2.4 s for one, 2.75 s
    /// for two, against 5.0 s for eight), the aim-in running in that clip.
    /// 0: always the whole load, **the default**: the design's 3
    /// (`fighting-record.md` 3.1) failed the smoke on the bench's real kits
    /// (tier 9-10, 64 matches, 120 s): the shotgun bots reloaded 40 % more
    /// often (3 rounds a load instead of 4-8) and reloads in an enemy's line
    /// rose 16.6 % (z 4.6); with 0 they fell 1.9 % (z -0.7). The players do
    /// not cut either: 1 of 99 reloads held AIM (0 of 15 in matches of 12 or
    /// more) and they load fully (6.3 rounds at the next world). Left as a
    /// row for a bench variant (2-4), GUESS.
    pub reload_rounds_in_view: u32,
    /// A weapon that fires one round a press: the trigger held this long,
    /// ms, and never past half its shot (so it is up before the fire state
    /// is over, [`super::guns`]); the next press after the weapon's cycle
    /// and `press_gap_ms` more.
    pub press_ms: u32,
    pub press_gap_ms: u32,
    /// The kick ([`super::recoil`]): while it aims at a target, this share
    /// of the view's rise its weapon's kick will give it this tick (or of
    /// the fall as the kick comes back) is taken back by the tick's turn,
    /// as the best players pull the mouse down against it; any weapon.
    /// 1: the view stays where the turn takes it; 0: the kick lifts it.
    /// The one number here mixed by the skill (the owner, 2026-10-23):
    /// `recoil_pull` at skill 1, `recoil_pull_low` at skill 0, so the best
    /// bots take back twice what the weakest do.
    pub recoil_pull: f32,
    pub recoil_pull_low: f32,
    /// Aiming down the sights at a seen target: always from `ads_near_m`
    /// on, nearer with this chance (drawn per target); with a bolt action
    /// always.
    pub ads_near_m: f32,
    pub ads_near_chance: f32,
    /// Audit issue 8: no shot until the sights are fully up (the weapon's
    /// `aim_time` since the aim was pressed, and `aim_in_margin_ms` for the
    /// input to reach the game), unless the hip cone at his distance keeps
    /// the shots on him; and the aim pressed whenever it does not, near
    /// or not. In 0.26g a shot spreads by the weapon's base alone once the
    /// aim-in is complete, moving or not, and from the hip (or during the
    /// aim-in) by the stance and movement too: 10.3 degrees strafing with
    /// an Uzi against 0.3 aimed (`player_bots_weapons.md` §9). The hip
    /// cone "keeps the shots on him" when `hip_cone_share` of its half
    /// (0.58: four shots of five fall within it) is within `body_half_m`
    /// of him at his distance. GUESS: `body_half_m` half a torso's width;
    /// `aim_in_margin_ms` one tick. On the real-map bench with the game's
    /// spread (`SRP_BENCH_SPREAD`), 8 seeds, with the rest of audit issue
    /// 4's change, the hit rate rose from 14.8 / 16.6 / 17.5 % to 20.8 /
    /// 23.6 / 26.3 % at tiers 4-5 / 9-10 / 13-15, with a third fewer shots
    /// and the kills the same within the noise
    /// (`player_bots_behaviour.md` §2.5).
    pub aim_in_first: bool,
    pub aim_in_margin_ms: u32,
    pub hip_cone_share: f32,
    /// Fire-gates PR B: the aim held (the key and its time, so the game's
    /// aim-in goes on) through a threat's look and while the target is
    /// lost for at most `aim_carry_ms`, where the bot let go of it and the
    /// aim-in started again. PR A's gate tally: the aim-in under way was
    /// the first gate of a quarter of the on-view ticks with no shot.
    /// GUESS: `aim_carry_ms` the measure's "lost a moment" ([`LOST_MS`]).
    /// On: one-sided on the bench (team 1 alone, 16 seeds, batt and tdm,
    /// tiers 4-5 to 13-15) its K/D went up 0.18-0.29 in every cell.
    pub aim_carry: bool,
    pub aim_carry_ms: u32,
    /// Fire-gates PR B: a target taken again within `aim_carry_ms` of
    /// losing him keeps what it had of him (the error settled so far, the
    /// reaction already waited, the stance drawn for him), where it was
    /// taken as a fresh one (the error three times the skill's again, a
    /// new reaction). PR A's measure: the reaction again and the aim-in
    /// again at a target lost a moment held 1.5 % and 16 % of the on-view
    /// ticks with no shot. Off: one-sided on the bench its K/D moved
    /// 0.02-0.04, within the noise, and it is the one exception to the
    /// noticing wait ([`Memory::in_sight`]).
    pub reacquire_keep: bool,
    /// Fire-gates PR C: the trigger's range by the class of the weapon in
    /// hand ([`Self::range_floor_m`]), where it was `AimTuning::range_m`
    /// for all but the bolt actions (target choice keeps that); never past
    /// today's ceiling (`range_m`, or `fire.bolt_range_m` for a bolt
    /// action), so it only shortens. Not from the configs: where a
    /// weapon's damage is down to its floor (`min_damage_distance`, 30-50 m
    /// but 100 for the bolts, the SVD and the SKS) is under the players' use
    /// in every class, and its aimed-in cone keeps hitting past 200 m
    /// (`player_bots_weapons.md`, "Bots: the fire range by weapon").
    /// On: with the floors at the all-runs p95 (84-105 m) the flag changed
    /// only the bolt, while 11.6 % of the bots' fire ticks fell past the
    /// players' hit-run p95 of their class, at 0.023 hits a tick against
    /// 0.061 within. A bot hit in the last `hit_shot_ms` answers a shooter
    /// past its range out to the ceiling (`Reflex::hit_at`).
    pub range_by_weapon: bool,
    /// The players' range by weapon class, metres: no bot shoots farther
    /// than the players land hits. MEASURED: the 95th percentile of the
    /// distance of the trigger runs that hit one enemy, of the 2 best
    /// players (by kills, then K/D) of each of the 402 players' matches
    /// with 12 or more people (`player_bots_weapons.md`, "Bots: the fire
    /// range by weapon", the hit-run column): rifles 65 m, SMGs 58,
    /// shotguns 49, the automatic marksman rifles 74, the bolt actions
    /// 105. The machine guns (17 runs) and the pistols (48) are too few for
    /// one: none ([`NO_FLOOR_M`], the ceiling). The shotgun's bench
    /// variant, 35 m (full damage to 10 m, 0.6 of it flat from 30 m):
    /// `SRP_BENCH_COMBAT=range_floor_shotgun_m=35`.
    pub range_floor_rifle_m: f32,
    pub range_floor_smg_m: f32,
    pub range_floor_shotgun_m: f32,
    pub range_floor_pistol_m: f32,
    pub range_floor_mg_m: f32,
    pub range_floor_marksman_m: f32,
    pub range_floor_bolt_m: f32,
    pub body_half_m: f32,
    /// Audit issue 8: fire only once the view is within `fire_body_share`
    /// of his body's half width (`body_half_m`) at his distance of where
    /// it means to aim, as a player waits for the crosshair to arrive, at
    /// least `fire_within_min_deg` and at most the aim's
    /// `fire_within_deg`; 0: off (the flat `fire_within_deg`). Off: at 3 it
    /// raised the hit rate 1.6 points alone but with `aim_in_first` cost
    /// 11.5 kills a seed on the bench. Why (A228, by side against the tip,
    /// tier 9-10, 8 seeds: wins -24 %, hits dealt -5 %): the error's floor
    /// is drawn anew every tick and the view carries the last tick's draw,
    /// so a bound under the floor's tick-to-tick spread refuses shots by
    /// the draw, not by where they go (the shots it let go hit no better
    /// at any view offset); it cut 15 % of the shots in bursts and 25 % of
    /// those at 40-65 m.
    pub fire_body_share: f32,
    pub fire_within_min_deg: f32,
    /// Standing to aim at a seen target, the legs step side to side: runs
    /// of `strafe_run_ms` one way then the other, a still moment of
    /// `strafe_still_ms` between with `strafe_still_chance`; a hit turns
    /// the step at once. With a bolt action or a marksman rifle only for
    /// some targets (`fire.strafe_bolt`, `fire.strafe_marksman`), and a
    /// bolt action stands still for its shot (`fire.bolt_still_ms`).
    pub strafe_run_ms: (u32, u32),
    pub strafe_still_chance: f32,
    pub strafe_still_ms: (u32, u32),
    /// The side step also while it walks (not runs) to or from its target:
    /// a forward or back walk weaves to the side (the top players move
    /// 84.5 % of the time while finishing a kill, the others 66.6 %;
    /// players strafe 41 % of the time).
    pub strafe_walking: bool,
    /// The side step and the weave are tested over the lip band too
    /// (`Tactics::clear_step_for`): not ending in it, and the move tested is
    /// the one the keys make. OFF: falls and pockets only, the pure side
    /// step, as it was.
    pub clear_step_sees_lip: bool,
    /// With `clear_step_sees_lip`: a side step of the reflex's own that is
    /// refused is taken to the other side if that is clear (the bot keeps
    /// strafing, away from the edge); both refused, it stands. OFF: it
    /// stands.
    pub clear_step_slides_along: bool,
    /// With a bolt-action weapon, it crouches to fire at a target with
    /// this chance (drawn per target); other weapons: `fire.crouch_*`.
    pub sniper_crouch_chance: f32,
    /// How each kind of weapon is fired ([`FireTuning`]).
    pub fire: FireTuning,
    /// What a seen enemy is worth shooting first beyond his distance, and the push on a hurt one who broke
    /// sight ([`super::worth`]).
    pub worth: super::worth::WorthTuning,
    /// The target kept while seen gives way to another seen enemy nearer
    /// than `switch_share` of its distance, or nearer than
    /// `switch_near_m` and `switch_margin_m` nearer than it (match
    /// 0x5402a730: a bot aimed at a bot 67 m away with the human 6 m from
    /// it). Audit issue 4 widened them from 0.5 and 15 m (the owner at
    /// 22-28 m from a bot aimed 44-46 m away): with `switch_between_thinks`
    /// and `SenseTuning::step_m`, contacts where a farther enemy was aimed
    /// at fell from 12.75 % to 10.99 % (−1.76 ± 0.76 a seed) on the bench,
    /// tiers 9-10; alone they cost kills (−6.4 a seed). GUESSES.
    pub switch_share: f32,
    pub switch_near_m: f32,
    pub switch_margin_m: f32,
    /// Audit issue 4 (the owner's matches of 2026-10-23: a bot aimed at bots
    /// 44-46 m away with the owner in its sight at 22-28 m; a farther enemy
    /// aimed at meanwhile in 2-14 % of contacts): the rules but the much
    /// nearer one wait `switch_hold_ms` on a target (no flicking between
    /// two); a seen enemy facing the bot (his view within
    /// `switch_facing_deg` of it) and `switch_facing_margin_m` nearer takes
    /// the target from one not facing it (0: off). Both off: on the
    /// real-map bench neither changed a measure by 2 standard errors
    /// (300 ms and 20 degrees tried, `player_bots_behaviour.md` §2.5).
    pub switch_hold_ms: u32,
    pub switch_facing_deg: f32,
    pub switch_facing_margin_m: f32,
    /// A switch is weighed at every tick, not only at a think (an enemy
    /// noticed between two thinks who presses more is taken at once, as one
    /// with no target is). Bench, tiers 9-10: without it contacts never
    /// engaged rose 0.62 ± 0.28 points a seed.
    pub switch_between_thinks: bool,
    /// The aim at a seen target is where he will be at the tick decided:
    /// his part where the view has him ([`PlaceView::known_at`]: a human
    /// reckoned to the clock, a bot to its last decided tick) led along his
    /// speed to that tick, which is up to [`super::encode::LEAD_TICKS`]
    /// past the clock. Before, the aim was where the view had him: in the
    /// owner's recorded match 0x31632b5a (2026-10-24) a bot aimed at a human
    /// crossing at 2.5 m/s or more was 0.73 m off him and 0.50 m off where he
    /// was 100 ms before, and a live human's ping makes that more. Bench
    /// (`player_bots_behaviour.md` §2.12): with the pings, +1.1 points of hit
    /// rate alone.
    pub lead_target: bool,
    /// The lead also covers the bullet's flight (`flight.rs`): the aim is at the place he reaches by the time the
    /// ammunition gets there, along his velocity as the bot's picture has it, and the drop on the way is aimed over.
    /// Before, the aim was where he is: the game's bullet flies (`bullet::tick` 0x32C180) and a target
    /// strafing at 2.6 m/s is half a torso off at 23-28 m for the slow ammunition and at 65-78 m for the rifles'
    /// (`player_bots_weapons.md`, "The bullet's flight"). Needs `lead_target`; no reaction, aim error or skill changes.
    ///
    /// Off, and why (bench 2026-10-31, TDM 10v10, flown bullets, head to head by side, 12 pairs a tier, `shot_bands`): the lead fires as
    /// designed (a strafer's shot line at 40-65 m trails him by 0.07 m instead of 0.21) but the far hit rate does not rise: 20-65 m pooled
    /// over tiers 4-5, 9-10 and 13-15, 10.05 % on against 10.59 % off (2650/26382 against 2768/26136 shots, z -2.1; 40-65 m alone 5.5/6.0,
    /// 6.9/7.1 and 8.0/8.6 %, none significant). The lines miss by far more than the lead moves them: at 40-65 m only 8-9 % of the shots
    /// pass within 0.25 m across and 0.45 m high of his chest and 36 % pass over 1 m across (the aim error: the skill dial). The pooled
    /// dip itself shows no cause in the counters (GUESS: strafers that turn back inside a flight are led the wrong way).
    pub lead_flight: bool,
    /// The aim is also raised by the bullet's drop on the way to him (`flight.rs`), whether or not he is led ([`Self::lead_flight`]).
    ///
    /// Off, and why: it adds the drop to a line the bots' recoil handling already leaves 0.13-0.35 m above the chest at 20-80 m (the
    /// kick the pull does not take back, by skill: `recoil_pull`), which the drop used to bring back onto the torso by accident. Where
    /// the climb is large it costs hits (tier 9-10, 35-50 m: 6.2 % on against 9.1 % off, z -4.2); where it is small it is neutral (tier
    /// 13-15, lateral lead and drop together, 40-65 m: 8.1 against 8.5 %, 20-40 m 13.9 against 14.3 %, kills +1.6 %, z +0.3).
    pub lead_drop: bool,
    /// [recoil, register #28] The bot's mirror of its weapon's kick counts the shots the client's weapon fires, not every tick the fire key
    /// is down: the client starts a shot only from the idle state (state 6) with a round in the weapon, and the state of the last shot
    /// (its fire clip, and a bolt's chamber) and a reload keep it from idle (`weapon_core::fire_pred` 0x2E3CB0 reads the fire bit of the
    /// input, `[weapon+0x5EC] == 6`, a chambered or magazine round, and no transition pending). The mirror fed the raw key counted shots
    /// the client refused (a shot inside the last one's clip, a press during a reload; 2240 of 17769 ticks with a shot on either side,
    /// tier 9-10) and ran its cadence on while the client's restarted, so the pull took back a kick that was not there or missed one that
    /// was: the shot line of a later shot of a run stood +0.10 degrees above where the bot thought (`kick_gap`). Off: the raw key.
    pub mirror_gated: bool,
    /// [smoke review S1] A target with no part showing and no part seen (a new target held by the sight's grace,
    /// or one lost in smoke before any part was seen) is looked at where the memory has him (`track.at()`), not
    /// at his true chest: knowledge by looking comes only through the eyes (`smoke.rs`). Off: his chest, the server's
    /// true place of an enemy the bot has not seen (`MatchView::place`), so its view pre-aims at him: a cheat (A147,
    /// the owner's rule "smarter, never cheaters"). ON by default, and `all_off.json` keeps it on too (A234: no cheat
    /// comes back in the all-off arm).
    pub aim_last_known: bool,
    /// The lead's most, ms (a view older than that is not led on).
    pub lead_most_ms: u32,
    /// The aim is taken from the eye where its own keys put it at the tick
    /// decided (its last decided tick's feet led along its speed), not
    /// where that tick left it: the shot leaves from there.
    pub lead_own: bool,
    /// The eye is where the game's bullet starts (`view::eye_height`: 1.46 m
    /// standing, 1.04 / 1.14 m crouched still / moving, with the view's pitch;
    /// the stand finder's ranges; a look again when the eye has moved; the
    /// stand-in's shots from the world's positions). Off: the old 1.6 / 0.95 m,
    /// the old ranges and the old shot order, to bench the fix against. It
    /// is the process's eye (both teams of a one-sided run).
    pub live_eye: bool,
    /// The turn onto a seen target also follows his bearing's turn rate
    /// (his speed against the bot's, across the line), so a steady crossing
    /// target is not trailed by the turn's ease (the turn's time constant,
    /// `settle_s` / 2, of the bearing's rate: 0.05 s of it at skill 0.6).
    pub turn_feed: bool,
    /// The error angles times the sights' zoom ([`ZoomError`]): the scopes' (A182 #29; `all_off.json` keeps it off).
    pub zoom_error: ZoomError,
    /// The wander settling with time on target and the shot waiting for the error to fit the body ([`Settle`]), on the guns with a
    /// scope (`settle_guns`; A182 #29, `all_off.json` keeps it off). The
    /// policy's number is `settle_k` (the error to expect against this times the body's angular half width at his distance:
    /// GUESS 1.0) and its cap `settle_max_wait_ms` after the target was taken (GUESS 5 s, the humans' aim-held q75 of a
    /// bolt action at 40-80 m).
    pub settle: Settle,
    /// Which guns it applies to (the scoped ones).
    pub settle_guns: SettleGuns,
    pub settle_k: f32,
    pub settle_max_wait_ms: u32,
    /// The turn to a threat, even with a target: hit, at the nearest enemy
    /// it sensed firing in the last `threat_shot_ms` (seen, or heard: where
    /// it heard him); not hit, at a shot heard within `threat_heard_m`
    /// more than `threat_off_deg` off its view and nearer than its target.
    /// It looks there `threat_ms`, and not again for `threat_again_ms`
    /// after (unless hit); an enemy seen within `threat_match_m` (and the
    /// place's error) of the threat is the shooter and its target. Only
    /// what a player gets: its own sight and hearing (no damage record of
    /// the world names the shooter).
    pub threat_shot_ms: u32,
    pub threat_heard_m: f32,
    pub threat_off_deg: f32,
    pub threat_ms: u32,
    pub threat_again_ms: u32,
    pub threat_match_m: f32,
    /// Play 6 (no target): a heard shot is looked at `sound_ms`, and no
    /// other before `sound_again_ms` more.
    pub sound_ms: u32,
    pub sound_again_ms: u32,
    /// A glance at a fresh heard track of any sound (a sprint, a jump, a
    /// reload, a use: match 0x15aa5b99, a bot aimed at a bot 80 m away
    /// with the owner sprinting up 4-11 m behind it) within
    /// `glance_heard_m`, from outside the view, nearer than
    /// `glance_heard_share` of the target's distance (any, with no
    /// target): a threat not made by a hit, looked at `glance_ms`.
    pub glance_heard_m: f32,
    pub glance_heard_share: f32,
    /// ... and any heard within this whatever the target's distance, as a
    /// player checks a noise at arm's length (audit issue 4: match 0x4da3b51d
    /// at 190-199 s, two bots 1-4 m apart for 8 s, neither hurt). Off: 6 m
    /// changed no measure on the bench (the steps heard, `SenseTuning::step_m`,
    /// did that case's work).
    pub glance_close_m: f32,
    /// No target: a teammate's callout (a told track) at most
    /// `glance_told_fresh_ms` old (from the sense that made it) within
    /// `glance_told_m`, from outside the view, is glanced at for
    /// `glance_ms`, at most once every `glance_told_every_ms`. OFF (0) by
    /// default: the bots-only bench lost 1.5-4 deliveries a seed (of 124)
    /// with it on, at 20-30 m and every 4-8 s.
    pub glance_told_m: f32,
    pub glance_told_fresh_ms: u32,
    pub glance_told_every_ms: u32,
    pub glance_ms: u32,
    /// Hit with no shooter sensed: the direction the HUD's hit marker
    /// shows (0.26g has one; direction only), drawn within
    /// `hit_bearing_error_deg` of the true one. The world we read names no
    /// shooter of a hit, so the shooter is the enemy who fired in the last
    /// `hit_shot_ms` with his view on the bot (within `hit_aim_deg`); only
    /// the direction to him is used, never his place or distance. OFF by
    /// default: the bots-only bench (no shooter past 80 m in it) lost 2-4
    /// deliveries a seed (of 124) with it on, kills +2-4; its worth
    /// against a far human sniper is not measured. `hit_shot_ms` is also how
    /// long after a hit the trigger's range is the ceiling, not the weapon
    /// class's ([`Self::range_by_weapon`]).
    pub hit_bearing: bool,
    /// ... also with a target seen (else it keeps on its target).
    pub hit_bearing_with_target: bool,
    pub hit_bearing_error_deg: f32,
    pub hit_shot_ms: u32,
    pub hit_aim_deg: f32,
    /// A seen enemy within this of the marker's direction is the shooter.
    pub hit_match_deg: f32,
    /// With `hit_cover`, it takes cover from the marker's direction: a
    /// spot whose rays from the standing eyes along it (and the marker's
    /// error either side) meet the world within `hit_cover_m`, metres.
    pub hit_cover: bool,
    pub hit_cover_m: f32,
    /// `hit_cover`'s cover is what stops the bullet by the game's rule ([`Tactics::shot_reaches`], the bot's `cover.pierce`):
    /// a spot behind a crate or a car, which the shooter's bullets go through, is not cover. Off, or with `cover.pierce` off:
    /// any surface of the bullet mesh within `hit_cover_m` is cover, as it was.
    pub hit_cover_reach: bool,
    /// Fire through cover (P4b; the owner, 2026-10-28: "yes, but capped"): a bot that was firing at an enemy goes on for this
    /// long after the last time it saw a part of him (the sight stamped it), at the place he was, when the bullet's line
    /// there keeps at least `fire_through_share` of its damage (the game's rule, `cover.pierce`; it is the middle of the
    /// kits' pierce, the bot's own gun is not read), no mate in the line, one burst (one shot, a gun that fires singly)
    /// in `fire_through_gap_ms`. Human-available: he was seen this moment ago and a human sprays the crate he went behind.
    /// 0: off, a bot holds fire on a target it does not see, as it did.
    pub fire_through_ms: u32,
    pub fire_through_share: f32,
    pub fire_through_gap_ms: u32,
    /// `band_hold_on_carrier` reads what the team saw: the target is a carrier when a bot of the team saw him carrying this recently
    /// (`TeamKnowledge::saw_carrying`, stamped with each sighting), not when the world says so now. 0: the world's, as it was.
    pub band_carrier_seen_ms: u32,
    /// The look along the marker's direction is at a point this far,
    /// metres (a fixed one: no distance is known).
    pub hit_look_m: f32,
    /// A teammate's "shot from there" callout made within this of the bot,
    /// metres, is glanced along once (no target), while
    /// `glance_told_fresh_ms` old.
    pub hit_told_m: f32,
    /// A hide with no re-peek (from a marker's direction, or outranged)
    /// waits this long there, ms (drawn).
    pub still_wait_ms: (u32, u32),
    /// Outranged: a target seen with a sniper weapon beyond its own
    /// weapon's band (and the slack), a short weapon (the Uzi, the TOZ-34,
    /// the AK-74U, the pistol) does not step in at him; with
    /// `outranged_cover`, seen facing it (his view within
    /// `outranged_aimed_deg` of it), it breaks sight from him, at most once
    /// every `outranged_again_ms`. OFF-failed twice: #124 (2026-10-26, kills lost) and again on the tip with the weapons
    /// known, v5 mapdata, the kits (2026-10-30, 96 pairs by side): wins 68 / 95, score -4.1 % (z -2.1), hits dealt -6.3 %
    /// (z -3.8); with the cone at 25 degrees wins 65 / 87, score -3.1 %, delivered -8.4 %. It does what it says (ticks in
    /// the open in a sniper's sight -47 to -70 %, deaths to snipers -8 to -12 %) but the bot out of the fight is out of
    /// its shots: the 40-60 deaths saved cost more than that in hits and in carries.
    pub outranged_hide: bool,
    pub outranged_cover: bool,
    pub outranged_aimed_deg: f32,
    pub outranged_again_ms: u32,
    /// A glance (`Override::glance`) gives way to a play that runs
    /// somewhere as well as to a use or a carry.
    pub glance_run_yields: bool,
    /// Play 11 holds its ground at a target seen carrying a battery (he
    /// cannot shoot): no step back out of the band (match 0x15aa5b99: a
    /// Mosin bot backed off a carrier at 9-23 m, and he walked home).
    pub band_hold_on_carrier: bool,
}

/// A row of an automatic weapon's bursts: up to `up_to_m` (the first row
/// whose distance the target is within), a tap (`FireTuning::tap_ms`) with
/// `tap_chance`, else a hold drawn in `hold_ms`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BurstRow {
    pub up_to_m: f32,
    pub tap_chance: f32,
    pub hold_ms: (u32, u32),
}

/// A row of a one-round weapon's wait after its cycle: up to `up_to_m`,
/// drawn in `extra_ms`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitRow {
    pub up_to_m: f32,
    pub extra_ms: (u32, u32),
}

/// A row of the best players' pause between two bursts at the same target (register C2): from `from_m` on, the quartiles of the
/// pause, ms (`q1`, median, `q3`; `player_bots_weapons.md` and `fact-fire-human-rule.md` §1, rifle and SMG in the owner's bands).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PauseRow {
    pub from_m: f32,
    pub q: [u32; 3],
}

/// The best players' pause between two bursts (register C2, `FireTuning::burst_pause`): drawn through their quartiles for the
/// target's distance, inverse CDF linear between the floor (0), `q1` (.25), the median (.5), `q3` (.75) and the top (1).
/// `Default` and [`Self::players`] are the players' numbers, so `{"burst_pause": {}}` turns it on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BurstPause {
    pub floor_ms: u32,
    pub top_ms: u32,
    pub smg: Vec<PauseRow>,
    pub rifle: Vec<PauseRow>,
}

impl BurstPause {
    /// The best players' pause: 20-40 m is their 15-30 m row, 40+ their 30+ m row (their bands end at 5 / 15 / 30 m).
    pub fn players() -> Self {
        Self {
            floor_ms: 100,
            top_ms: 700,
            smg: vec![PauseRow { from_m: 20.0, q: [100, 250, 500] }, PauseRow { from_m: 40.0, q: [100, 200, 450] }],
            rifle: vec![PauseRow { from_m: 20.0, q: [100, 200, 462] }, PauseRow { from_m: 40.0, q: [100, 200, 350] }],
        }
    }
}

impl Default for BurstPause {
    fn default() -> Self {
        Self::players()
    }
}

/// How the bots fire each kind of weapon ([`super::guns::Handling`], read
/// from the weapon's config), from the best players of the 631 recorded
/// matches of 6+ players of 2026-10-16..22 (`player_bots_weapons.md` §3,
/// §4: the trigger runs that hurt one enemy, his distance from the
/// uploaded worlds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FireTuning {
    /// Submachine guns' bursts by distance: held long to 35 m (top
    /// players' holds 600-800 ms median, taps 9-21 %), more taps at 35-60
    /// m (36 %) and most beyond (65 %): the matches after weapon balance 1
    /// (revision 21, 2026-10-18), which cut the SR-2M's and the PP-2000's
    /// rate of fire; before it they held to 60 m.
    pub smg: Vec<BurstRow>,
    /// Rifles' and machine guns' bursts: held to 35 m (700 ms, taps 15-19
    /// %), then a third taps at 35-60 m and most beyond 60 m (36 %, 61 %;
    /// median 500 and 150 ms).
    pub rifle: Vec<BurstRow>,
    /// The pause after a burst is drawn from the best players' quartiles for the target's distance (inverse CDF through the floor,
    /// `q1`, the median, `q3` and the top) instead of `aim.pause_ms`'s flat draw, at the distances the rows cover (the bots' far
    /// pause: median 450-500 ms from the shot, the best players' 200 ms from the end of the trigger; C2). `None` (the default):
    /// the flat draw; `Some` is on, the players' numbers being [`BurstPause::players`]. On by default (#902: far shots +10 % z +4.3, far
    /// hits +9.6 % z +2.8 in the by-side bench, no cost).
    pub burst_pause: Option<BurstPause>,
    /// A tap's hold, ms.
    pub tap_ms: (u32, u32),
    /// A one-round weapon without a bolt (marksman rifles, pistols,
    /// semi-automatic shotguns): pressed again this long after its cycle,
    /// by distance (top players: +30 ms within 20 m, +100 at 20-35, +200
    /// at 35-60, +500 beyond, medians).
    pub semi_wait: Vec<WaitRow>,
    /// A bolt action: after the shot and the bolt, the sights brought
    /// back on him before the next press, ms (top players: 0.6 / 1.1 /
    /// 1.9 s past the config's cycle; nobody presses within 0.4 s of it).
    pub bolt_reaim_ms: (u32, u32),
    /// A bolt action that side-steps between shots stands still this long
    /// before its next press, ms (a choice: the press is aimed).
    pub bolt_still_ms: u32,
    /// Two barrels: the second press this long after the cycle, ms (top
    /// players: 300-350 ms press to press against a 250 ms cycle).
    pub barrels_wait_ms: (u32, u32),
    /// A bolt action fires out to this, metres (top players hit at 52 /
    /// 80 / 96 m with the SV-98); any other weapon to `AimTuning::range_m`.
    pub bolt_range_m: f32,
    /// Crouched to fire at a target, drawn per target (share of the top
    /// players' fire ticks crouched): automatic weapons 16 %, machine guns
    /// (category 16) 36 %, marksman rifles (one round a press, category 8
    /// or 13) 32 %, pistols and shotguns 10-18 %.
    pub crouch_auto: f32,
    pub crouch_machine_gun: f32,
    pub crouch_marksman: f32,
    pub crouch_other: f32,
    /// Side-steps while it fires at a target, drawn per target, with a bolt
    /// action or a marksman rifle (top players: a side key on 51 % of their
    /// fire ticks with either; 75-84 % with automatic weapons, which always
    /// side-step).
    pub strafe_bolt: f32,
    pub strafe_marksman: f32,
    /// **Switch (default on).** Which class a weapon is, for everything that
    /// goes by class: the range floor, the bursts' table, the crouch and
    /// side-step shares, the distance band against another weapon. Off: the
    /// dictionary category's, and a server bot's kind by its role. On: the
    /// game data's ([`WeaponClass::of`]: the category's, except an SMG type in
    /// the rifle category, so the AK-74U takes the SMG's bands and tables), and
    /// a server bot's kind follows the weapon it holds.
    pub weapon_class_by_data: bool,
}

impl Default for FireTuning {
    fn default() -> Self {
        let burst = |up_to_m, tap_chance, hold_ms| BurstRow {
            up_to_m,
            tap_chance,
            hold_ms,
        };
        let wait = |up_to_m, extra_ms| WaitRow { up_to_m, extra_ms };
        Self {
            smg: vec![
                burst(20.0, 0.11, (400, 1_150)),
                burst(35.0, 0.2, (450, 1_350)),
                burst(60.0, 0.36, (450, 1_500)),
                burst(f32::MAX, 0.65, (300, 900)),
            ],
            rifle: vec![
                burst(20.0, 0.16, (400, 1_200)),
                burst(35.0, 0.19, (400, 1_200)),
                burst(60.0, 0.36, (300, 1_100)),
                burst(f32::MAX, 0.61, (250, 650)),
            ],
            burst_pause: Some(BurstPause::players()),
            tap_ms: (100, 200),
            semi_wait: vec![
                wait(20.0, (0, 150)),
                wait(35.0, (30, 210)),
                wait(60.0, (110, 500)),
                wait(f32::MAX, (280, 1_000)),
            ],
            bolt_reaim_ms: (600, 1_700),
            bolt_still_ms: 300,
            barrels_wait_ms: (50, 300),
            bolt_range_m: 120.0,
            crouch_auto: 0.16,
            crouch_machine_gun: 0.35,
            crouch_marksman: 0.3,
            crouch_other: 0.15,
            strafe_bolt: 0.5,
            strafe_marksman: 0.5,
            weapon_class_by_data: true,
        }
    }
}

/// A marksman rifle: one round a press, of the rifles' or the sniper
/// rifles' category (the SVD, the SKS, the VSK-94's one-round copies); with
/// `by_data`, one of the rifle or marksman class.
fn marksman(gun: &Gun, by_data: bool) -> bool {
    gun.handling == Handling::Semi
        && if by_data {
            matches!(gun.class, WeaponClass::Rifle | WeaponClass::Marksman)
        } else {
            matches!(gun.category, RIFLE_CATEGORY | SNIPER_CATEGORY)
        }
}

impl FireTuning {
    /// The class of `gun` the bots go by ([`Self::weapon_class_by_data`]).
    pub fn class_of(&self, gun: &Gun) -> WeaponClass {
        gun.class_for(self.weapon_class_by_data)
    }

    /// The bursts' table `gun` takes: the data class' with the switch on, else
    /// the one its handling carries (`legacy`, by its category).
    pub fn auto_class(&self, gun: &Gun, legacy: AutoClass) -> AutoClass {
        if self.weapon_class_by_data {
            gun.class.auto_class()
        } else {
            legacy
        }
    }

    /// An automatic weapon's next burst at `distance` metres, ms: a tap or
    /// a hold.
    pub fn burst_ms(
        &self,
        class: AutoClass,
        distance: f32,
        fallback: (u32, u32),
        rng: &mut Rng,
    ) -> u32 {
        let rows = match class {
            AutoClass::Smg => &self.smg,
            AutoClass::Rifle => &self.rifle,
        };
        let draw =
            |range: (u32, u32), rng: &mut Rng| rng.range(range.0 as f32, range.1 as f32) as u32;
        match rows
            .iter()
            .find(|row| distance <= row.up_to_m)
            .or(rows.last())
        {
            Some(row) if rng.unit() < row.tap_chance => draw(self.tap_ms, rng),
            Some(row) => draw(row.hold_ms, rng),
            None => draw(fallback, rng),
        }
    }

    /// The pause after an automatic weapon's burst at `distance` metres, ms, from the best players' quartiles (`burst_pause`), or none
    /// (off, or a distance no row covers: the flat `aim.pause_ms`). Inverse CDF, linear between the floor (0), `q1` (.25), the median
    /// (.5), `q3` (.75) and the top (1).
    pub fn pause_ms(&self, class: AutoClass, distance: f32, rng: &mut Rng) -> Option<u32> {
        let pause = self.burst_pause.as_ref()?;
        let rows = match class {
            AutoClass::Smg => &pause.smg,
            AutoClass::Rifle => &pause.rifle,
        };
        let row = rows.iter().rev().find(|row| distance >= row.from_m)?;
        let at = [
            (0.0, pause.floor_ms as f32),
            (0.25, row.q[0] as f32),
            (0.5, row.q[1] as f32),
            (0.75, row.q[2] as f32),
            (1.0, pause.top_ms as f32),
        ];
        let u = rng.unit();
        let hi = at.iter().position(|&(p, _)| u < p).unwrap_or(at.len() - 1).max(1);
        let ((p0, v0), (p1, v1)) = (at[hi - 1], at[hi]);
        Some((v0 + (v1 - v0) * (u - p0) / (p1 - p0)).max(pause.floor_ms as f32) as u32)
    }

    /// A one-round weapon's wait after its cycle before the next press at
    /// `distance` metres, ms.
    pub fn wait_ms(&self, gun: &Gun, distance: f32, rng: &mut Rng) -> u32 {
        let draw =
            |range: (u32, u32), rng: &mut Rng| rng.range(range.0 as f32, range.1 as f32) as u32;
        match gun.handling {
            Handling::Bolt => draw(self.bolt_reaim_ms, rng),
            Handling::Barrels => draw(self.barrels_wait_ms, rng),
            Handling::Semi => self
                .semi_wait
                .iter()
                .find(|row| distance <= row.up_to_m)
                .or(self.semi_wait.last())
                .map_or(0, |row| draw(row.extra_ms, rng)),
            Handling::Burst(_) => 0,
        }
    }

    /// The chance it crouches to fire at a target with `gun`
    /// (`sniper` for a bolt action).
    pub fn crouch_chance(&self, gun: &Gun, sniper: f32) -> f32 {
        match gun.handling {
            Handling::Bolt => sniper,
            Handling::Burst(_) if self.class_of(gun) == WeaponClass::MachineGun => {
                self.crouch_machine_gun
            }
            Handling::Burst(_) => self.crouch_auto,
            _ if marksman(gun, self.weapon_class_by_data) => self.crouch_marksman,
            _ => self.crouch_other,
        }
    }

    /// The chance it side-steps while it fires at a target with `gun`.
    pub fn strafe_chance(&self, gun: &Gun) -> f32 {
        match gun.handling {
            Handling::Bolt => self.strafe_bolt,
            _ if marksman(gun, self.weapon_class_by_data) => self.strafe_marksman,
            _ => 1.0,
        }
    }
}

impl Default for CombatTuning {
    fn default() -> Self {
        Self {
            wounded_bonus_m: 20.0,
            duel_ms: 3_000,
            punish_m: 25.0,
            punish_heard_m: 12.5,
            punish_window_ms: 1_500,
            punish_ms: 1_500,
            punish_chance: 0.8,
            hit_drop: 0.03,
            hide_m: (2.0, 10.0),
            hide_wait_ms: (1_000, 2_000),
            hide_give_up_ms: 4_000,
            peek_m: (2.0, 8.0),
            repeek_min_deg: 15.0,
            repeek_min_m: 4.0,
            peek_ms: 4_000,
            band_pistol: (0.0, 20.0),
            band_smg: (10.0, 35.0),
            band_shotgun: (5.0, 25.0),
            band_rifle: (15.0, 50.0),
            band_sniper: (40.0, 95.0),
            keep_from_shotgun_m: 15.0,
            close_on_sniper_m: 30.0,
            band_slack_m: 3.0,
            band_step_m: 6.0,
            band_cross_deg: None,
            stand_strafe: false,
            stand_strafe_hold_ms: 400,
            band_legs: true,
            spot_cooldown_ms: 3_000,
            spot_again_ms: 15_000,
            spot_within_deg: 8.0,
            spot_chance: 1.0,
            grenades: 2,
            grenade_roll_ms: 4_000,
            // The habit of the live players (A159, "Measure, then match"): the bots with a frag throw 42 per 100 carrying
            // lives, the players 54.5; with the old 0.15 / 4.0 s / 12 m / 18 m they threw 8 (bench, v5, 56 pairs).
            grenade_chance: 0.6,
            grenade_m: (12.0, 27.0),
            grenade_hold_ms: (500, 800),
            grenade_within_deg: 6.0,
            grenade_friend_m: 5.0,
            grenade_recheck: true,
            grenade_lead_s: 2.5,
            grenade_trace: true,
            grenade_launch_ms: 1_100,
            grenade_look_after_ms: 300,
            grenade_land_m: 10.0,
            grenade_wall_m: 10.0,
            grenade_off_m: 8.0,
            grenade_off_y_m: None,
            grenade_lane_m: 2.0,
            grenade_places: 3,
            grenade_tracks: true,
            grenade_track_ms: 4_000,
            grenade_track_gap_ms: 500,
            grenade_track_error_m: 8.0,
            grenade_track_chance: 0.45,
            grenade_at_held: Some(1.0),
            grenade_held_recheck: true,
            grenade_held_reach: true,
            grenade_search: true,
            grenade_search_off_m: 5.0,
            grenade_search_traces: 64,
            grenade_claim: true,
            grenade_kit_cap: true,
            grenade_claim_m: 8.0,
            grenade_claim_ms: 6_000,
            grenade_repick: true,
            grenade_push_tracks: false,
            grenade_volley: false,
            grenade_volley_chance: 0.2,
            grenade_volley_ms: (1_600, 6_000),
            grenade_volley_every_ms: 500,
            grenade_volley_track: false,
            grenade_volley_near_m: 10.0,
            grenade_volley_age_ms: 2_000,
            grenade_near_m: 12.0,
            grenade_close: false,
            grenade_close_m: (4.0, 12.0),
            grenade_close_margin_m: 2.0,
            grenade_close_fresh_ms: 1_500,
            grenade_close_pace: None,
            grenade_give_up_ms: 1_500,
            grenade_push_ms: (1_500, 3_500),
            grenade_push_after_blast: true,
            grenade_push_after_ms: 300,
            held_speed: 0.3,
            held_memory_ms: 5_000,
            camp_ms: 8_000,
            camp_m: 3.0,
            trade_ms: 3_000,
            trade_m: 40.0,
            reload_idle_ms: 400,
            reload_some_share: 0.4,
            reload_aim_control: true,
            reload_rounds_in_view: 0,
            press_ms: 100,
            press_gap_ms: 50,
            recoil_pull: 1.0,
            recoil_pull_low: 0.5,
            ads_near_m: 10.0,
            ads_near_chance: 0.85,
            aim_in_first: true,
            aim_in_margin_ms: 50,
            hip_cone_share: 0.58,
            aim_carry: true,
            aim_carry_ms: LOST_MS,
            reacquire_keep: false,
            range_by_weapon: true,
            range_floor_rifle_m: 65.0,
            range_floor_smg_m: 58.0,
            range_floor_shotgun_m: 49.0,
            range_floor_pistol_m: NO_FLOOR_M,
            range_floor_mg_m: NO_FLOOR_M,
            range_floor_marksman_m: 74.0,
            range_floor_bolt_m: 105.0,
            body_half_m: 0.25,
            fire_body_share: 0.0,
            fire_within_min_deg: 0.5,
            strafe_run_ms: (250, 700),
            strafe_still_chance: 0.5,
            strafe_still_ms: (150, 450),
            strafe_walking: true,
            clear_step_sees_lip: false,
            clear_step_slides_along: false,
            sniper_crouch_chance: 0.25,
            fire: FireTuning::default(),
            worth: super::worth::WorthTuning::default(),
            switch_share: 0.65,
            switch_near_m: 25.0,
            switch_margin_m: 5.0,
            switch_hold_ms: 0,
            switch_facing_deg: 0.0,
            switch_facing_margin_m: 10.0,
            switch_between_thinks: true,
            lead_target: true,
            lead_flight: false,
            lead_drop: false,
            mirror_gated: true,
            aim_last_known: true,
            lead_most_ms: 300,
            lead_own: true,
            live_eye: true,
            turn_feed: true,
            zoom_error: ZoomError::Scopes,
            settle: Settle::WanderAndPolicy,
            settle_guns: SettleGuns::Scoped,
            settle_k: 1.0,
            settle_max_wait_ms: 5_000,
            threat_shot_ms: 1_000,
            threat_heard_m: 25.0,
            // Half the sense's view (`SenseTuning::view_deg`).
            threat_off_deg: 45.0,
            threat_ms: 1_000,
            threat_again_ms: 1_500,
            threat_match_m: 6.0,
            sound_ms: 1_000,
            sound_again_ms: 1_500,
            glance_heard_m: 10.0,
            glance_heard_share: 0.5,
            glance_close_m: 0.0,
            glance_told_m: 0.0,
            glance_told_fresh_ms: 3_000,
            glance_told_every_ms: 4_000,
            glance_ms: 1_000,
            hit_bearing: false,
            hit_bearing_with_target: false,
            hit_bearing_error_deg: 8.0,
            hit_shot_ms: 3_000,
            hit_aim_deg: 8.0,
            hit_match_deg: 15.0,
            hit_cover: true,
            hit_cover_m: 30.0,
            hit_cover_reach: true,
            fire_through_ms: 750,
            fire_through_share: 0.8,
            fire_through_gap_ms: 1_000,
            band_carrier_seen_ms: 500,
            hit_look_m: 30.0,
            hit_told_m: 40.0,
            still_wait_ms: (2_000, 4_000),
            outranged_hide: true,
            outranged_cover: false,
            outranged_aimed_deg: 180.0,
            outranged_again_ms: 6_000,
            band_hold_on_carrier: true,
            glance_run_yields: false,
        }
    }
}

impl CombatTuning {
    /// [PR C] The players' floor of a weapon class ([`Gun::class`]).
    pub fn range_floor_m(&self, class: &str) -> f32 {
        match class {
            "smg" => self.range_floor_smg_m,
            "shotgun" => self.range_floor_shotgun_m,
            "pistol" => self.range_floor_pistol_m,
            "mg" => self.range_floor_mg_m,
            "marksman" => self.range_floor_marksman_m,
            "bolt" => self.range_floor_bolt_m,
            _ => self.range_floor_rifle_m,
        }
    }

    /// [PR C] The trigger's range with `gun` in hand, under the ceiling
    /// `ceiling_m` ([`Self::range_by_weapon`]).
    pub fn fire_range_m(&self, gun: &Gun, ceiling_m: f32) -> f32 {
        if !self.range_by_weapon {
            return ceiling_m;
        }
        self.range_floor_m(self.fire.class_of(gun).name()).min(ceiling_m)
    }

    pub fn band(&self, kind: WeaponKind) -> (f32, f32) {
        match kind {
            WeaponKind::Pistol => self.band_pistol,
            WeaponKind::Smg => self.band_smg,
            WeaponKind::Shotgun => self.band_shotgun,
            WeaponKind::Rifle => self.band_rifle,
            WeaponKind::Sniper => self.band_sniper,
        }
    }
}

/// [PR C] A class with no players' range ([`CombatTuning::range_floor_m`]):
/// past every ceiling, so the ceiling holds.
pub const NO_FLOOR_M: f32 = 1_000.0;

/// Play 11: the distance band to an enemy, metres, by the bot's weapon
/// (`own`) and his (`enemy`): its own weapon's band, then an enemy shotgun
/// kept at `keep_from_shotgun_m` (by any weapon but a shotgun) and an enemy
/// sniper closed on to `close_on_sniper_m`, unless that leaves less than
/// 5 m of band (its own weapon wins). No band without its own weapon's kind.
pub fn preferred_band(
    own: Option<WeaponKind>,
    enemy: Option<WeaponKind>,
    tuning: &CombatTuning,
) -> Option<(f32, f32)> {
    let band = tuning.band(own?);
    let adjusted = match enemy {
        Some(WeaponKind::Shotgun) if own != Some(WeaponKind::Shotgun) => {
            (band.0.max(tuning.keep_from_shotgun_m), band.1)
        }
        Some(WeaponKind::Sniper) => (band.0, band.1.min(tuning.close_on_sniper_m)),
        _ => band,
    };
    Some(if adjusted.1 - adjusted.0 >= 5.0 {
        adjusted
    } else {
        band
    })
}

/// What the reflex reads at a think besides the bot and its memory.
#[derive(Clone, Copy)]
pub struct Situation<'a> {
    pub view: &'a MatchView,
    /// The map's spots (play 7); none before the map is ready.
    pub tactics: Option<&'a Tactics>,
    /// The team's danger spots (play 21).
    pub team: Option<&'a TeamKnowledge>,
    /// The bot's map side (spots off their spawn area).
    pub side: Option<i32>,
    /// Its trade partner (play 21).
    pub partner: Option<Place>,
    /// Its own weapon 1's kind (play 11).
    pub weapon: Option<WeaponKind>,
    /// Every place's weapon 1 kind, by place, where known (play 11).
    pub weapons: &'a [Option<WeaponKind>],
    /// [G3] The way out of a grenade's blast the bot will take, when the
    /// avoidance is on (`grenade_avoid`); none: no throw is made inside the
    /// floor.
    pub retreat: Option<Retreat>,
    /// What the bot's danger tests of cover go by (`Tuning::cover.rule()`): the hide spot is out of the enemy's
    /// fire, not only out of his sight.
    pub cover: Option<super::tuning::CoverRule>,
}

impl<'a> Situation<'a> {
    /// The view alone: no map, no team, no weapons.
    pub fn of(view: &'a MatchView) -> Self {
        Self {
            view,
            tactics: None,
            team: None,
            side: None,
            partner: None,
            weapon: None,
            weapons: &[],
            retreat: None,
            cover: None,
        }
    }
}

/// Why the reflex takes the legs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegsWhy {
    /// Play 5: at a reloading enemy.
    Punish,
    /// Play 7: out of his sight.
    BreakSight,
    /// Play 7: back from another angle.
    RePeek,
    /// Play 11: into the weapon's band.
    Range,
    /// Play 20: in after the grenade.
    AfterGrenade,
    /// Play 20: standing still from the press to the launch, for the throw
    /// leaves from where the thrower is then.
    Throwing,
    /// A smoke throw: standing still from the turn to the launch, as [`Self::Throwing`]; with `smoke.throw_keeps_route`
    /// the route stays for the walk on after it.
    ThrowingSmoke,
    /// [A208 `smoke.wait_for_cloud`] Holding short of a crossing until a cloud hides it (`smoke::CloudWait`): the
    /// route stays for the walk on.
    WaitingForCloud,
    /// A hurt enemy who broke sight: on his last place while he cannot mend
    /// ([`Reflex::finish`]); the push is this state, the enemy is its own.
    Finish(Place),
    /// Aimed in and firing: standing on its feet with the side step only ([`Gait::StandStrafe`]).
    Stand,
}

/// Why the side step is or is not held in an aimed fight, in ticks (the `gun_shots` topic's `strafe`, register #24); nothing
/// decides by them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StrafeStats {
    /// Ticks aimed in on a target.
    pub ticks: u32,
    /// The side step is on a run (a side key held by it).
    pub running: u32,
    /// Between two runs (`strafe_still_chance`) with the step free to go.
    pub stopped: u32,
    /// This target was drawn not to be strafed at (`FireTuning::strafe_chance`: a bolt or marksman rifle).
    pub not_for_target: u32,
    /// The bolt action stands still for its shot.
    pub bolt_holds: u32,
    /// The legs are the reflex's own or run, and `strafe_walking` does not weave there.
    pub legs_no_weave: u32,
    /// The step away from a teammate in the line of fire.
    pub friend_away: u32,
    /// The gait was [`Gait::StandStrafe`].
    pub stand: u32,
}

/// How the legs go while the bot fights (`CombatTuning::stand_strafe`). One state, not a flag: what the legs do in a fight is
/// either the play's way or the stand-and-strafe, and the transition is total over the tick's one fact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Gait {
    /// The play's route, a range step, a weave on a walk.
    #[default]
    Route,
    /// On its feet with the side step only, until this tick of the game's clock unless the fight is still on.
    StandStrafe { until: u32 },
}

impl Gait {
    /// The gait after a tick in which the fight was on (aimed in, a round in, the target in range, the play's word to fire) or
    /// not: stands and strafes for `hold_ms` after the last tick it was on.
    fn next(self, fight_on: bool, now: u32, hold_ms: u32) -> Self {
        match (self, fight_on) {
            (_, true) => Self::StandStrafe { until: now + hold_ms },
            (Self::StandStrafe { until }, false) if now < until => self,
            (_, false) => Self::Route,
        }
    }
}

/// The legs as the reflex wants them this tick, instead of the play's go,
/// pace and stance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legs {
    pub at: Vec3,
    pub within_m: f32,
    /// Sprint where the way allows (firing still stops the sprint).
    pub run: bool,
    pub crouch: bool,
    pub why: LegsWhy,
}

/// What the reflex wants of the motor this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Override {
    /// Look at this point (the aim, with its error), instead of the play's
    /// look.
    pub look_at: Option<Vec3>,
    /// The yaw and pitch error added to the aim, radians.
    pub error: (f32, f32),
    /// Added to the tick's pitch turn as it is: the predicted rise of the
    /// view by the kick taken back (`recoil_pull`), radians.
    pub pull: f32,
    /// Hold fire this tick.
    pub fire: bool,
    /// The turn's time constant while aiming, seconds.
    pub turn_time_s: Option<f32>,
    /// Keys the reflex presses or holds this tick, OR'd into the decision:
    /// `SPOT` for one tick, `GRENADE` each tick until the throw starts,
    /// `AIM` while it watches a partner's death spot.
    pub keys: u32,
    /// The legs instead of the play's (`None`: the play's).
    pub legs: Option<Legs>,
    /// Play 11: the distance band to the target it prefers, metres (a hint
    /// for the plays; the reflex steps into it itself with `legs`).
    pub band_m: Option<(f32, f32)>,
    /// The side step (`key::LEFT` or `key::RIGHT`, or 0) the legs take
    /// while the bot stands to aim at a seen target; the caller presses it
    /// only where the ground that side is clear.
    pub strafe: u32,
    /// `strafe` is the step away from a teammate in the line of fire
    /// (`friend.away`), not the reflex's own run: the caller never turns it
    /// to the other side (that would step into him).
    pub strafe_away: bool,
    /// `look_at` is a glance at a heard shot (play 6, or a threat not
    /// made by a hit), not an aim: the caller keeps the play's look when
    /// the play uses something or carries (a glance there broke the use
    /// and slowed the carrier: the bench lost a third of its deliveries
    /// once play 6 ran again).
    pub glance: bool,
    /// The look's turn rate, yaw and pitch, radians a second, while it aims
    /// at a seen target (`turn_feed`): the motor's turn follows it.
    pub track: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Push {
    toward: Vec3,
    from: u32,
    until: u32,
    why: LegsWhy,
}

/// What [`Reflex::break_sight`] came to (the heal's cover walk counts the reasons it did not walk).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sought {
    /// A spot was found and the bot hides there.
    Began,
    /// Already hiding.
    Hiding,
    /// A push at an enemy's reload is on.
    Pushing,
    /// Not in a duel (no target and no hit within `duel_ms`).
    NotDuel,
    NoTactics,
    /// A carrier, or `hit_cover` off.
    Declined,
    /// No enemy it fights to hide from.
    NoEnemy,
    /// The ring of the search holds no spot hidden from him.
    NoSpot,
}

/// Why a heal press did or did not owe a cover walk, for the bench's counts (nothing decides by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealWhy {
    Walked,
    /// The think came more than `HEAL_WALK_LATE_MS` after the press.
    Late,
    Carrier,
    Throwing,
    /// The item's delay allows a walk shorter than the search's near edge.
    ShortReach,
    /// A known enemy within the wish's `enemy_m`.
    EnemyNear,
    Hiding,
    Pushing,
    NotDuel,
    NoTactics,
    NoEnemy,
    NoSpot,
}

/// The note of a heal press's walk: when the think made it, why, the nearest known enemy and the cover found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealNote {
    pub at: u32,
    pub why: HealWhy,
    /// The nearest known enemy (any track), m.
    pub enemy_m: Option<f32>,
    /// The distance to the spot walked to, m.
    pub cover_m: Option<f32>,
}

/// Play 7's steps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Cover {
    #[default]
    Open,
    Hiding {
        at: Vec3,
        enemy: Vec3,
        started: u32,
        arrived: Option<u32>,
        wait_ms: u32,
        /// Back at another angle after the wait (else only the wait).
        repeek: bool,
    },
    Peeking {
        at: Vec3,
        until: u32,
    },
}

/// [play 20] The bots' grenade's fuse (`loadouts::BOT_GRENADE`'s
/// `ttl_ms`, from the throw to the blast), ms; none without its config,
/// and then no throw.
fn grenade_fuse_ms() -> Option<u32> {
    survarium::v026g::weapons::grenade_named(survarium::v026g::loadouts::BOT_GRENADE.name)
        .map(|g| g.ttl_ms)
}

/// [play 20] A teammate of `me` within `grenade_friend_m` of `at`, now or
/// on his way there: along his velocity for `grenade_lead_s`.
fn friend_in_blast(me: &PlaceView, view: &MatchView, at: Vec3, c: &CombatTuning) -> Option<Place> {
    view.team(me.team)
        .filter(|p| p.place != me.place)
        .find(|p| {
            let walk = Vec3::new(p.velocity[0], 0.0, p.velocity[1]) * c.grenade_lead_s;
            nearest_xz(p.feet, p.feet + walk, at) < c.grenade_friend_m
        })
        .map(|p| p.place)
}

/// The ground distance from `at` to the segment `a`-`b`.
fn nearest_xz(a: Vec3, b: Vec3, at: Vec3) -> f32 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let length = dx * dx + dz * dz;
    let s = if length > 0.0 {
        (((at.x - a.x) * dx + (at.z - a.z) * dz) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Vec3::new(a.x + s * dx, at.y, a.z + s * dz).distance_xz(at)
}

/// A grenade being thrown (play 20).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Throw {
    at: Vec3,
    started: u32,
    hold_ms: u32,
    pressed: Option<u32>,
    /// The aim's error, radians (yaw, pitch): drawn once, so the view that
    /// was weighed is the view held.
    error: (f32, f32),
    /// The key let go: the throw counted, the push set. The look is still
    /// held until the grenade has left the hand.
    released: bool,
    /// The pitch the view was last turned to for the place: held after the
    /// let-go when the thrower's own height has moved the place out of
    /// reach (the grenade leaves at the launch whatever the bot does).
    pitch: Option<f32>,
    /// The arc weighed and held: the flat one, or the search's.
    pick: ArcPick,
    /// Where its place came from.
    source: PlaceSource,
    /// What decided it: a roll, or the second grenade of a volley.
    kind: ThrowKind,
    /// [G3] Decided under the lowered floor (`grenade_near_m`): where the
    /// thrower stood when the way out of the blast was found. The lowered
    /// floor holds only while he stays within [`NEAR_STAY_M`] of it; farther
    /// off the throw is weighed by the floor of `grenade_land_m`, which a
    /// near place does not pass.
    near: Option<Vec3>,
    /// [close] Decided as a close throw (`grenade_close`): where the thrower stood when the way out of the blast was found.
    /// The close rule (`close_floor`) holds while he stays within [`NEAR_STAY_M`] of it; farther off the throw is weighed by
    /// the floor of `grenade_land_m`, which a close place does not pass.
    close: Option<Vec3>,
    /// [A170] The held spot (`threat::HeldLine::id`) a [`PlaceSource::Spot`] throw is at: the throw is given up when the team's
    /// picture drops it (`grenade_held_recheck`).
    spot: Option<u32>,
}

/// [smoke] A smoke being thrown with its quick-use key (`smoke::SmokePlan`): from the decision, the view turned to the
/// plan's, the key held, let go, and the look kept until the grenade has left the hand.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SmokeThrowing {
    plan: super::smoke::SmokePlan,
    /// The quick-use key of the kit's smoke pocket at the decision (`live_grenades::Pockets::smoke_key`).
    key: u32,
    started: u32,
    phase: SmokePhase,
    /// [A208] The hold after the launch, if any.
    wait: Option<super::smoke::CloudWait>,
}

/// [A208 `smoke.wait_for_cloud`] A hold for a cloud under way: since when (the press for its own smoke), its latest end,
/// and its legs.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CloudHolding {
    wait: super::smoke::CloudWait,
    since: u32,
    until: u32,
    legs: HoldLegs,
}

/// [A208] Where a hold's legs are: walking the route on to the hold point, or standing at it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum HoldLegs {
    Walking,
    Standing(Vec3),
}

/// [A208] A hold walking to its point stands once this near it, metres.
const HOLD_AT_M: f32 = 1.5;
/// [A208] Two crossings this near are the same one, metres (`crossing_of`'s 2 m steps and a step's slack).
const HOLD_SAME_CROSSING_M: f32 = 3.0;

/// [A208, the bench's] A thrown smoke's crossing, watched until the bot comes to it or this time passes.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CrossingWatch {
    for_use: super::smoke::SmokeUse,
    line: super::smoke::SmokeLine,
    until: u32,
}

/// [A208, the bench's] A thrown smoke's crossing is watched this long from the launch, ms.
#[cfg(test)]
const CROSSING_WATCH_MS: u32 = 15_000;
/// [A208, the bench's] The bot is at its crossing within this, metres (the throw classes' 2.5 m).
#[cfg(test)]
const CROSSING_AT_M: f32 = 2.5;

/// Where a smoke throw is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmokePhase {
    /// The view turning to the plan's.
    Turning,
    /// The key down since this time.
    Held(u32),
    /// Let go at this time: the look kept until the launch.
    Released(u32),
}

/// What decided a throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThrowKind {
    Roll,
    /// [C20] The second grenade at the place of the first.
    Volley,
}

/// [G3] A near throw keeps its lowered floor while the thrower is within
/// this of where he stood when the way out was found, metres: the way out
/// is from that floor.
const NEAR_STAY_M: f32 = 1.5;

/// [C20] The window for a second grenade at the place of a throw let go.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Volley {
    at: Vec3,
    source: PlaceSource,
    /// When the first was let go: evidence is a track sensed after it (`grenade_volley_track`).
    since: u32,
    /// The next roll not before, and the window's end (the first's blast or
    /// `grenade_volley_ms.1`), ms.
    next: u32,
    until: u32,
}

/// [A187 #10] The place of a second grenade by evidence (`grenade_volley_track`): the freshest track of an enemy the senses gave
/// since the first grenade was let go, not in sight now, within `grenade_volley_near_m` of the first's place, no older than
/// `grenade_volley_age_ms` and in the places' range; none if there is no such track or a teammate stands in its blast's reach.
fn volley_evidence(look: &RollView, volley: &Volley, c: &CombatTuning) -> Option<(Vec3, PlaceSource)> {
    let RollView { me, sit, memory, now } = *look;
    memory
        .tracks()
        .iter()
        .filter_map(|track| {
            let (at, sensed) = track.own?;
            let fits = sensed >= volley.since
                && now.saturating_sub(sensed) <= c.grenade_volley_age_ms
                && track.error_m <= c.grenade_track_error_m
                && at.distance_xz(volley.at) <= c.grenade_volley_near_m
                && (c.grenade_m.0..=c.grenade_m.1).contains(&at.distance_xz(me.feet))
                && friend_in_blast(me, sit.view, at, c).is_none();
            fits.then(|| {
                let source = if track.sight_since.is_some() || track.source == Source::Seen { PlaceSource::Lost } else { PlaceSource::Heard };
                (sensed, (at, source))
            })
        })
        .max_by_key(|(sensed, _)| *sensed)
        .map(|(_, place)| place)
}

/// [G3] What the thrower's way out of his own blast is judged by: the
/// avoidance's numbers (`AvoidTuning`). `Situation::retreat` is set only
/// when `grenade_avoid` is on; the thrower is the first to be told of his
/// grenade (`Knowledge::Told`), so it is the walk he will make.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Retreat {
    /// Past the blast's radius, metres (`AvoidTuning::margin_m`).
    pub margin_m: f32,
    /// The slowest told reaction, ms (`AvoidTuning::told_ms.1`).
    pub told_ms: u32,
}

/// [play 20] What a grenade roll looks at: the bot, its situation (the
/// match view, the team's claims), its memory of the enemies, the time.
#[derive(Clone, Copy)]
struct RollView<'a, 'b> {
    me: &'a PlaceView,
    sit: &'a Situation<'b>,
    memory: &'a Memory,
    now: u32,
}

/// [play 20] Why a throw is not made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refuse {
    /// No pitch lands it at the place.
    Reach,
    /// The arc meets no ground before the blast.
    Nowhere,
    /// The first contact is on a steep wall beyond the thrower's blast and
    /// nearer than `grenade_wall_m`.
    Wall,
    /// The first contact (a floor, a wall, a ceiling) is within the
    /// thrower's blast.
    Thrower,
    /// The first contact is far from the place.
    Off,
    /// A teammate may be in the blast.
    Mate(Place),
}

impl Refuse {
    /// The note's reason ("holds a grenade back at (x, y, z): <reason>").
    fn why(self) -> String {
        match self {
            Self::Reach => "out of reach".to_owned(),
            Self::Nowhere => "lands nowhere before the blast".to_owned(),
            Self::Wall => "hits a wall near the thrower".to_owned(),
            Self::Thrower => "lands in the thrower's blast".to_owned(),
            Self::Off => "lands off the place".to_owned(),
            Self::Mate(mate) => format!("teammate {mate}"),
        }
    }

    /// The reason's place in [`GrenadeStats::refused`].
    fn index(self) -> usize {
        match self {
            Self::Thrower => 0,
            Self::Wall => 1,
            Self::Mate(_) => 2,
            Self::Reach => 3,
            Self::Nowhere => 4,
            Self::Off => 5,
        }
    }
}

/// [measure] Where a throw's place came from: an enemy seen holding an
/// angle (`Reflex::held`) or one sensed staying at a spot (`Reflex::camps`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaceSource {
    Held,
    Camp,
    /// [G1] Where the bot's own senses last gave an enemy it saw (now out
    /// of sight).
    Lost,
    /// [G1] Where it heard one, or a teammate called one.
    Heard,
    /// A held threat spot (`grenade_at_held`): where the team sensed the shooter that killed its bots, lately.
    Spot,
}

impl PlaceSource {
    fn index(self) -> usize {
        match self {
            Self::Held => 0,
            Self::Camp => 1,
            Self::Lost => 2,
            Self::Heard => 3,
            Self::Spot => 4,
        }
    }

    /// A place from a track, not an angle held or a camp.
    fn is_track(self) -> bool {
        matches!(self, Self::Lost | Self::Heard)
    }
}

/// [G2] The arc a throw at a place takes: the flat one to it, another pitch
/// at the yaw of the place (a lob, or an arc under what is in the way), or
/// another yaw (an offset of the view from the bearing to the place) with a
/// pitch: what the search found.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ArcPick {
    Flat,
    Pitch { pitch: f32 },
    Yawed { dyaw: f32, pitch: f32 },
}

impl ArcPick {
    /// The yaw offset of the view from the bearing to the place.
    fn dyaw(self) -> f32 {
        match self {
            Self::Flat | Self::Pitch { .. } => 0.0,
            Self::Yawed { dyaw, .. } => dyaw,
        }
    }

    /// The kind's place in [`GrenadeStats::passed`]: 0 the flat arc, 1 another
    /// pitch at the yaw of the place, 2 another yaw.
    fn kind(self) -> usize {
        match self {
            Self::Flat => 0,
            Self::Pitch { .. } => 1,
            Self::Yawed { .. } => 2,
        }
    }
}

/// [measure] The kinds of place, the reasons a throw is refused
/// ([`Refuse`]) and the kinds of arc that passed, as the lengths of
/// [`GrenadeStats`]'s tables. The sources: held, camp, lost, heard
/// ([`PlaceSource`], the fifth a held threat spot); the arc kinds ([`ArcPick::kind`]): flat, another
/// pitch, another yaw.
pub const GRENADE_SOURCES: usize = 5;
pub const GRENADE_REASONS: usize = 6;
pub const GRENADE_ARCS: usize = 3;

/// [measure] What the grenade logic of one reflex (one life) did, counted
/// as it ran: nothing here decides anything. The bench's tally reads it
/// ([`Reflex::grenade_stats`]) to say where the throws are lost: no place,
/// the chance, a refused arc. Tables by place source ([`PlaceSource`]:
/// held, camp, lost, heard), refusal reason ([`Refuse::index`]: thrower's
/// blast, steep wall, teammate, out of reach, lands nowhere, off the place)
/// and arc kind (flat, another pitch, another yaw).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GrenadeStats {
    /// Which reflex counted this (one is made for each life, [`Reflex::armed`]): the readings of the same reflex are told from
    /// those of the next life by it. The reflex's age ([`Self::thinks`]) cannot tell them: a throw under way changes the counts
    /// (the release, a give-up, a timeout) while no think reaches the roll, so a reading with the same age and other counts was
    /// taken for a new life's and its whole counts were added again.
    pub born: u32,
    /// Thinks that reached the roll: the reflex's age.
    pub thinks: u32,
    /// Rolls with at least one place in range (what starts the lockout of
    /// `grenade_roll_ms`), and those the chance sent away.
    pub rolls: u32,
    pub chance_no: u32,
    /// The places of those rolls, by source (a place is counted at each
    /// roll it is in).
    pub places: [u32; GRENADE_SOURCES],
    /// Places dropped from a roll for a teammate's claim.
    pub claimed: u32,
    /// The places weighed (up to `grenade_places` a roll, nearest first), by
    /// source; of them those refused by reason, and those that passed by arc.
    pub weighed: [u32; GRENADE_SOURCES],
    pub refused: [[u32; GRENADE_REASONS]; GRENADE_SOURCES],
    pub passed: [[u32; GRENADE_ARCS]; GRENADE_SOURCES],
    /// Throws given up at the press or in the hold, by reason.
    pub given_up: [u32; GRENADE_REASONS],
    /// Throws let go as the 1st, 2nd and 3rd or later of the life.
    pub nth: [u32; 3],
    /// Throws dropped because the view was not on them within
    /// `grenade_give_up_ms`: by whether a target was in view then (0 none,
    /// 1 one) and the arc kind.
    pub timeouts: [[u32; GRENADE_ARCS]; 2],
    /// Traces at the press or in the hold that failed for what is in the way
    /// and were searched again (`grenade_repick`): 0 an arc found, 1 none.
    pub rechecks: [u32; 2],
    /// Throws let go, by the source of their place.
    pub thrown_from: [u32; GRENADE_SOURCES],
    /// Pushes after a throw left out (`grenade_push_tracks` off).
    pub pushes_skipped: u32,
    /// [C20] The volley (`grenade_volley`): windows opened by a throw let
    /// go, rolls made in them, those the chance said no to, second grenades
    /// let go, rolls with evidence of an enemy by the first's place and rolls with none (`grenade_volley_track`).
    pub volley: [u32; 6],
    /// [G3] The lowered floor (`grenade_near_m`): places refused by the
    /// floor alone that were weighed again under it, those given a way out
    /// of the blast (the throw decided), and throws let go from them.
    pub near: [u32; 3],
    /// [close] `grenade_close`: rolls with a close place and no throw made at a far one; places weighed; refused by the arc, the
    /// landing or a teammate's blast; refused for a teammate not seen lately; refused for no way out of the blast; throws
    /// decided; throws let go.
    pub close: [u32; 7],
    /// [close] The funnel of a think with the switch on, for why the throw is rare: thinks with a held spot, camp or enemy track
    /// within the close band by position alone; of those, stopped before the roll by a throw under way, a target in sight, all
    /// the life's grenades thrown, the lockout of the last roll; no frag in the kit; no retreat or map for the check; nothing
    /// left after the track window and the claims; the chance said no; a far place was thrown at instead; reached the close
    /// throw. Then tracks in the band, per track and think, left out for being younger than `grenade_track_gap_ms`, older than
    /// `grenade_track_ms`, or with an error over `grenade_track_error_m`; then the close weighings refused, by [`Refuse::index`].
    pub close_gate: [u32; 20],
    /// [A170] The held spots of a roll (`grenade_at_held`), thinks that reached the roll with: a held spot of the team, one
    /// whose shooter the team sensed on it lately, one of those in the throw range, one the thrower's feet are not in the lanes
    /// of, one with no mate in its blast and no other place at it, one no mate claimed, and, of those, thinks of a bot with a
    /// frag in its kit. Each is counted once a think, for the furthest a spot got.
    pub held_funnel: [u32; 7],
    /// [A170] Thinks that did not reach the roll while the team had a sensed held spot, by what stopped them: a throw under
    /// way, a target in sight, all the life's grenades thrown, the lockout of the last roll.
    pub held_busy: [u32; 4],
    /// [A170] Throws at a held spot given up because the team's picture dropped the spot (`grenade_held_recheck`): before the key
    /// was pressed, in the hold.
    pub held_dropped: [u32; 2],
}

impl GrenadeStats {
    /// [measure] The frags this life let go at a held threat spot (`grenade_at_held`).
    pub fn thrown_at_spots(&self) -> u32 {
        self.thrown_from[PlaceSource::Spot.index()]
    }

    /// The change from `before`, the last reading of the same reflex, and
    /// whether this reading is a new life's: the whole of `self` then (another
    /// reflex counted it, or there was no reading before).
    pub fn since(&self, before: Option<&Self>) -> (Self, bool) {
        let Some(before) = before.filter(|b| b.born == self.born && self.thinks >= b.thinks) else {
            return (*self, true);
        };
        fn sub<const N: usize>(a: [u32; N], b: [u32; N]) -> [u32; N] {
            std::array::from_fn(|i| a[i].saturating_sub(b[i]))
        }
        let change = Self {
            born: self.born,
            thinks: self.thinks.saturating_sub(before.thinks),
            rolls: self.rolls.saturating_sub(before.rolls),
            chance_no: self.chance_no.saturating_sub(before.chance_no),
            claimed: self.claimed.saturating_sub(before.claimed),
            places: sub(self.places, before.places),
            weighed: sub(self.weighed, before.weighed),
            refused: std::array::from_fn(|i| sub(self.refused[i], before.refused[i])),
            passed: std::array::from_fn(|i| sub(self.passed[i], before.passed[i])),
            given_up: sub(self.given_up, before.given_up),
            nth: sub(self.nth, before.nth),
            timeouts: std::array::from_fn(|i| sub(self.timeouts[i], before.timeouts[i])),
            rechecks: sub(self.rechecks, before.rechecks),
            thrown_from: sub(self.thrown_from, before.thrown_from),
            pushes_skipped: self.pushes_skipped.saturating_sub(before.pushes_skipped),
            volley: sub(self.volley, before.volley),
            near: sub(self.near, before.near),
            close: sub(self.close, before.close),
            close_gate: sub(self.close_gate, before.close_gate),
            held_funnel: sub(self.held_funnel, before.held_funnel),
            held_busy: sub(self.held_busy, before.held_busy),
            held_dropped: sub(self.held_dropped, before.held_dropped),
        };
        (change, false)
    }

    /// `other`'s counts added to these.
    pub fn add(&mut self, other: &Self) {
        fn plus<const N: usize>(a: &mut [u32; N], b: &[u32; N]) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
        self.thinks += other.thinks;
        self.rolls += other.rolls;
        self.chance_no += other.chance_no;
        self.claimed += other.claimed;
        plus(&mut self.places, &other.places);
        plus(&mut self.weighed, &other.weighed);
        for (a, b) in self.refused.iter_mut().zip(&other.refused) {
            plus(a, b);
        }
        for (a, b) in self.passed.iter_mut().zip(&other.passed) {
            plus(a, b);
        }
        plus(&mut self.given_up, &other.given_up);
        plus(&mut self.nth, &other.nth);
        for (a, b) in self.timeouts.iter_mut().zip(&other.timeouts) {
            plus(a, b);
        }
        plus(&mut self.rechecks, &other.rechecks);
        plus(&mut self.thrown_from, &other.thrown_from);
        self.pushes_skipped += other.pushes_skipped;
        plus(&mut self.volley, &other.volley);
        plus(&mut self.near, &other.near);
        plus(&mut self.close, &other.close);
        plus(&mut self.close_gate, &other.close_gate);
        plus(&mut self.held_funnel, &other.held_funnel);
        plus(&mut self.held_busy, &other.held_busy);
        plus(&mut self.held_dropped, &other.held_dropped);
    }
}

/// [play 20] The yaw and pitch of the view that throws at `at` from the eye
/// of `me`: the pitch whose flight comes down at the place
/// ([`arc::pitch_for`], the flat arc, from the hand over the ground at the
/// place), none when no pitch does. Without `grenade_trace`, the old flat
/// line: the place raised by `1.0 + 0.12 d`.
fn throw_view(me: &PlaceView, at: Vec3, c: &CombatTuning) -> Option<(f32, f32)> {
    let eye = me.eye();
    let d = eye.distance_xz(at);
    let yaw = yaw_of([at.x - eye.x, at.z - eye.z]);
    let pitch = if c.grenade_trace {
        arc::pitch_for(d, arc::hand_of(eye).y - at.y)?
    } else {
        (at.y + 1.0 + 0.12 * d - eye.y).atan2(d)
    };
    Some((yaw, pitch))
}

/// The yaw from `me`'s eye to `at`.
fn bearing_to(me: &PlaceView, at: Vec3) -> f32 {
    let eye = me.eye();
    yaw_of([at.x - eye.x, at.z - eye.z])
}

/// [G2] The view of `pick` at `at`: the flat arc's ([`throw_view`]), or the
/// search's pitch with its yaw offset from the bearing to the place.
fn throw_view_of(
    me: &PlaceView,
    at: Vec3,
    c: &CombatTuning,
    pick: ArcPick,
) -> Option<(f32, f32)> {
    match pick {
        ArcPick::Flat => throw_view(me, at, c),
        ArcPick::Pitch { pitch } => Some((bearing_to(me, at), pitch)),
        ArcPick::Yawed { dyaw, pitch } => Some((bearing_to(me, at) + dyaw, pitch)),
    }
}

/// [play 20] The point on the ground line to `at` that the view of `pitch`
/// from `eye` looks at, the yaw turned `dyaw` from the bearing to `at`
/// ([`ArcPick::dyaw`], 0 for the flat arc): the look the driver turns to.
fn throw_point(eye: Vec3, at: Vec3, pitch: f32, dyaw: f32) -> Vec3 {
    let d = eye.distance_xz(at);
    if dyaw == 0.0 {
        return Vec3::new(at.x, eye.y + pitch.tan() * d, at.z);
    }
    let [x, z] = forward(yaw_of([at.x - eye.x, at.z - eye.z]) + dyaw);
    Vec3::new(eye.x + x * d, eye.y + pitch.tan() * d, eye.z + z * d)
}

/// [play 20] `f` with the ground a grenade meets: the bullet mesh; a level
/// plane at `level` for a reflex never told a map.
fn over_ground<R>(
    tree: Option<&mapdata::RayTree>,
    level: f32,
    f: impl FnOnce(&dyn arc::Ground) -> R,
) -> R {
    match tree {
        Some(tree) => f(&arc::Mesh(tree)),
        None => f(&arc::Floor { y: level }),
    }
}

/// [play 20] A teammate of `me` who may be in the blast of `landing`: by
/// his chest now and along his velocity for `grenade_lead_s` (none with
/// `grenade_recheck` off), within `grenade_friend_m` of where the grenade
/// may go off, or within `grenade_lane_m` of the line a runaway grenade
/// takes.
fn mate_in_blast(
    me: &PlaceView,
    view: &MatchView,
    landing: &arc::Landing,
    c: &CombatTuning,
) -> Option<Place> {
    /// The teammate's line is looked at every this many seconds.
    const SAMPLE_S: f32 = 0.5;
    let lead = if c.grenade_recheck {
        c.grenade_lead_s
    } else {
        0.0
    };
    let steps = (lead / SAMPLE_S).ceil().max(1.0) as u32;
    view.team(me.team)
        .filter(|p| p.place != me.place)
        .find(|p| {
            let walk = Vec3::new(p.velocity[0], 0.0, p.velocity[1]) * lead;
            (0..=steps).any(|k| {
                let at = p.chest() + walk * (k as f32 / steps as f32);
                landing.blast_distance(at, arc::ROLL_P80_M) < c.grenade_friend_m
                    || landing.blast_distance(at, arc::ROLL_WORST_M) < c.grenade_lane_m
            })
        })
        .map(|p| p.place)
}

/// [play 20] Whether `me` may throw at `at` with an aim `error` off the
/// view that lands it there: the arc of that view from the hand, traced on
/// `ground` ([`arc::Flight::landing`]), must come down; not within
/// `grenade_land_m` of him (whatever it hits there), not on a steep wall
/// nearer than `grenade_wall_m`, not far from the place; and, with
/// `mates`, with no teammate in the blast. Without
/// `grenade_trace`, only the teammate near the place.
fn weigh_throw(
    me: &PlaceView,
    view: &MatchView,
    at: Vec3,
    error: (f32, f32),
    c: &CombatTuning,
    mates: bool,
    ground: &dyn arc::Ground,
) -> Result<(), Refuse> {
    if !c.grenade_trace {
        if !mates {
            return Ok(());
        }
        let near = if c.grenade_recheck {
            friend_in_blast(me, view, at, c)
        } else {
            view.team(me.team)
                .find(|p| p.place != me.place && p.feet.distance_xz(at) < c.grenade_friend_m)
                .map(|p| p.place)
        };
        return near.map_or(Ok(()), |mate| Err(Refuse::Mate(mate)));
    }
    let view_yp = throw_view(me, at, c).ok_or(Refuse::Reach)?;
    weigh_view(me, view, at, view_yp, error, c, mates, ground, c.grenade_off_m)
}

/// [play 20] The checks of [`weigh_throw`] for the view `(yaw, pitch)` of a
/// throw at `at`, the first contact to be within `off_m` of the place.
#[allow(clippy::too_many_arguments)]
fn weigh_view(
    me: &PlaceView,
    view: &MatchView,
    at: Vec3,
    (yaw, pitch): (f32, f32),
    error: (f32, f32),
    c: &CombatTuning,
    mates: bool,
    ground: &dyn arc::Ground,
    off_m: f32,
) -> Result<(), Refuse> {
    let landing = arc::Flight::thrown(me.eye(), yaw + error.0, pitch + error.1)
        .landing(ground)
        .ok_or(Refuse::Nowhere)?;
    let thrower = landing.thrower_distance(me.chest());
    // The thrower's own blast first: a wall or a ceiling within it is that
    // (17 of the 19 places a smoke refused for "a steep wall" were).
    if thrower < c.grenade_land_m {
        return Err(Refuse::Thrower);
    }
    if landing.contact.is_wall() && thrower < c.grenade_wall_m {
        return Err(Refuse::Wall);
    }
    if landing.contact.point.distance_xz(at) > off_m
        || c.grenade_off_y_m.is_some_and(|off_y| (landing.contact.point.y - at.y).abs() > off_y)
    {
        return Err(Refuse::Off);
    }
    let mate = mates.then(|| mate_in_blast(me, view, &landing, c));
    match mate.flatten() {
        Some(mate) => Err(Refuse::Mate(mate)),
        None => Ok(()),
    }
}

/// [G2] Where the flat arc to `at` is refused for the thrower's blast, a
/// steep wall near him or a landing off the place, other arcs: pitches from
/// the bots' limit down to up, nearest the flat arc's first (a lob over what
/// is in the way, an arc under it), at the yaw of the place and then turned
/// by 6, 12, 20 and 30 degrees either way (coarser pitches), at most
/// `grenade_search_traces` traces. The first whose first contact is within
/// `grenade_search_off_m` of the place and which passes every check of
/// [`weigh_view`]. A pitch is clamped at [`arc::PITCH_LIMIT`]: a lob that
/// clears cover lands 22-29 m out and no nearer.
fn search_arc(
    me: &PlaceView,
    view: &MatchView,
    at: Vec3,
    error: (f32, f32),
    c: &CombatTuning,
    ground: &dyn arc::Ground,
) -> Option<ArcPick> {
    const DYAWS_DEG: [f32; 9] = [0.0, 6.0, -6.0, 12.0, -12.0, 20.0, -20.0, 30.0, -30.0];
    const LOW_DEG: f32 = -10.0;
    let eye = me.eye();
    let bearing = yaw_of([at.x - eye.x, at.z - eye.z]);
    let flat = arc::pitch_for(eye.distance_xz(at), arc::hand_of(eye).y - at.y).unwrap_or(0.0);
    let mut traces = 0;
    for dyaw_deg in DYAWS_DEG {
        let step = if dyaw_deg == 0.0 { 3.0 } else { 6.0 };
        let top = arc::PITCH_LIMIT.to_degrees();
        let mut pitches: Vec<f32> = (0..)
            .map(|k| LOW_DEG + step * k as f32)
            .take_while(|p| *p <= top)
            .collect();
        pitches.sort_by(|a, b| {
            (a.to_radians() - flat)
                .abs()
                .total_cmp(&(b.to_radians() - flat).abs())
        });
        for pitch_deg in pitches {
            if traces >= c.grenade_search_traces {
                return None;
            }
            traces += 1;
            let dyaw = dyaw_deg.to_radians();
            let pitch = pitch_deg.to_radians();
            let pick = if dyaw_deg == 0.0 {
                ArcPick::Pitch { pitch }
            } else {
                ArcPick::Yawed { dyaw, pitch }
            };
            if weigh_view(
                me,
                view,
                at,
                (bearing + dyaw, pitch),
                error,
                c,
                true,
                ground,
                c.grenade_search_off_m,
            )
            .is_ok()
            {
                return Some(pick);
            }
        }
    }
    None
}

/// [play 20] The arc for one place: the flat arc if it passes every check;
/// with `grenade_search` the others ([`search_arc`]) where the flat arc is
/// refused for what is in the way (not for a teammate, nor a place out of
/// reach), else why it was refused.
fn weigh_place(
    me: &PlaceView,
    view: &MatchView,
    at: Vec3,
    error: (f32, f32),
    c: &CombatTuning,
    ground: &dyn arc::Ground,
) -> Result<ArcPick, Refuse> {
    match weigh_throw(me, view, at, error, c, true, ground) {
        Ok(()) => Ok(ArcPick::Flat),
        Err(why @ (Refuse::Thrower | Refuse::Wall | Refuse::Off))
            if c.grenade_search && c.grenade_trace =>
        {
            search_arc(me, view, at, error, c, ground).ok_or(why)
        }
        Err(why) => Err(why),
    }
}

/// [play 20] The place to throw at: the first of `places` (nearest first,
/// at most `grenade_places` of them) whose arc [`weigh_place`] finds, every
/// check as for one place. When none does, the nearest place and why it was
/// refused. One ground for all the places: the mesh; for a reflex never told
/// a map, a level plane at the nearest place's height.
#[cfg(test)]
fn pick_place(
    me: &PlaceView,
    view: &MatchView,
    places: &[Vec3],
    error: (f32, f32),
    c: &CombatTuning,
    ground: &dyn arc::Ground,
) -> Result<Vec3, (Vec3, Refuse)> {
    pick_place_noting(me, view, places, error, c, ground, |_, _| {}).map(|(at, _)| at)
}

/// [`pick_place`], with the arc, telling `weighed` each place it weighs (its
/// index in `places`) and the verdict, for the bench's counts.
fn pick_place_noting(
    me: &PlaceView,
    view: &MatchView,
    places: &[Vec3],
    error: (f32, f32),
    c: &CombatTuning,
    ground: &dyn arc::Ground,
    mut weighed: impl FnMut(usize, Result<ArcPick, Refuse>),
) -> Result<(Vec3, ArcPick), (Vec3, Refuse)> {
    let mut first = None;
    for (i, &at) in places.iter().take(c.grenade_places.max(1) as usize).enumerate() {
        let verdict = weigh_place(me, view, at, error, c, ground);
        weighed(i, verdict);
        match verdict {
            Ok(pick) => return Ok((at, pick)),
            Err(why) => {
                first.get_or_insert((at, why));
            }
        }
    }
    Err(first.expect("a place was weighed"))
}

/// [G3] Whether the thrower, once he has let the grenade go, can walk out of
/// its blast at `rest`: the avoidance's own walk ([`Tactics::step_out`], the
/// floors he may stand on), to a floor whose chest is `margin_m` past the
/// blast's radius, within what the fuse leaves. The legs are held until
/// `grenade_look_after_ms` after the launch and the told reaction takes up
/// to `told_ms` of the same time; the walk is at [`ARRIVE_MPS`], the pace the
/// avoidance reckons with. From a floor hemmed in (a pocket, a ledge) no
/// floor is in reach: no.
fn can_retreat(
    me: &PlaceView,
    tactics: &Tactics,
    side: Option<i32>,
    retreat: Retreat,
    rest: Vec3,
    now: u32,
    c: &CombatTuning,
) -> bool {
    let Some(fuse) = grenade_fuse_ms() else {
        return false;
    };
    let free_ms = c.grenade_launch_ms + c.grenade_look_after_ms.max(retreat.told_ms);
    let reach_m = ARRIVE_MPS * fuse.saturating_sub(free_ms) as f32 / 1000.0;
    let Some(from) = tactics.world.grid.locate(me.feet, 1.5) else {
        return false;
    };
    let radius = arc::Grenade::bots().splash_m + retreat.margin_m;
    tactics
        .step_out(from, rest, radius, reach_m, now, side, false, &RouteFilter::open())
        .is_some_and(|out| (out + Vec3::new(0.0, CHEST, 0.0) - rest).length() >= radius)
}

/// [G3] `c` with the thrower's blast floor lowered to `grenade_near_m`, and
/// the margin on a steep wall (`grenade_wall_m`, a guard beyond the floor)
/// no farther than that: the way out of the blast is what keeps the thrower
/// safe, for a floor or a wall alike.
fn near_floor(c: &CombatTuning) -> CombatTuning {
    CombatTuning {
        grenade_land_m: c.grenade_near_m,
        grenade_wall_m: c.grenade_wall_m.min(c.grenade_near_m),
        ..c.clone()
    }
}

/// [G3] Of the places the floor refused alone (`by_floor`, indices into
/// `spots`), the first that passes every check under the lowered floor
/// `grenade_near_m` and whose blast the thrower can walk out of
/// ([`can_retreat`]). None with the avoidance off, no map, or the lowered
/// floor not below the floor.
fn near_place(
    look: &RollView,
    c: &CombatTuning,
    spots: &[Vec3],
    by_floor: &[usize],
    error: (f32, f32),
    ground: &dyn arc::Ground,
    stats: &mut GrenadeStats,
) -> Option<(Vec3, ArcPick)> {
    let RollView { me, sit, now, .. } = *look;
    let (retreat, tactics) = (sit.retreat?, sit.tactics?);
    if c.grenade_near_m >= c.grenade_land_m {
        return None;
    }
    let lowered = near_floor(c);
    for &i in by_floor {
        let at = spots[i];
        stats.near[0] += 1;
        let Ok(pick) = weigh_place(me, sit.view, at, error, &lowered, ground) else {
            continue;
        };
        let landing = throw_view_of(me, at, &lowered, pick).and_then(|(yaw, pitch)| {
            arc::Flight::thrown(me.eye(), yaw + error.0, pitch + error.1).landing(ground)
        });
        if landing.is_some_and(|landing| {
            can_retreat(me, tactics, sit.side, retreat, landing.rest, now, c)
        }) {
            stats.near[1] += 1;
            return Some((at, pick));
        }
    }
    None
}

/// [close] A teammate's body is fixed no longer ago than `grenade_close_fresh_ms`, or the throw is refused for him: a mate
/// whose place is not known is one whose blast cannot be ruled out.
fn close_fresh(me: &PlaceView, view: &MatchView, c: &CombatTuning) -> Result<(), Refuse> {
    match view
        .team(me.team)
        .filter(|p| p.place != me.place)
        .find(|p| view.clock.saturating_sub(p.seen_at) > c.grenade_close_fresh_ms)
    {
        Some(mate) => Err(Refuse::Mate(mate.place)),
        None => Ok(()),
    }
}

/// [close] The lead of a teammate's walk in a close throw's check, s: to the blast and a little past it.
const CLOSE_LEAD_S: f32 = 3.0;

/// [close] `c` for a throw at a close place: no floor but the nearest landing (`grenade_close_m.0`, the walk out of the blast
/// replaces it), and a teammate looked for as far as the radius and the margin from the rest point and from any point of the
/// roll's worst case ([`mate_in_blast`], no credit for a wall), along his walk for up to [`CLOSE_LEAD_S`].
fn close_floor(c: &CombatTuning) -> CombatTuning {
    let clear = arc::Grenade::bots().splash_m + c.grenade_close_margin_m;
    CombatTuning {
        grenade_land_m: c.grenade_close_m.0,
        grenade_wall_m: c.grenade_wall_m.min(c.grenade_close_m.0),
        grenade_friend_m: c.grenade_friend_m.max(clear),
        grenade_lane_m: c.grenade_lane_m.max(clear),
        grenade_lead_s: c.grenade_lead_s.max(CLOSE_LEAD_S),
        grenade_recheck: true,
        ..c.clone()
    }
}

/// [close] Whether the thrower can walk out of the blast wherever it goes off: from the first contact, at the rest point,
/// at [`arc::ROLL_P80_M`] and at [`arc::ROLL_WORST_M`] along the heading. Each is [`can_retreat`]'s walk (the floors he may
/// stand on, within what the fuse leaves, at [`ARRIVE_MPS`]) to a floor whose chest is past the radius, by more than the
/// avoidance's margin (`grenade_close_margin_m`), and the way never leads nearer the blast than he stands
/// ([`Tactics::step_out_where`]): no walk toward the landing and back out.
fn close_way_out(
    me: &PlaceView,
    tactics: &Tactics,
    side: Option<i32>,
    retreat: Retreat,
    landing: &arc::Landing,
    now: u32,
    c: &CombatTuning,
) -> bool {
    let Some(fuse) = grenade_fuse_ms() else {
        return false;
    };
    let free_ms = c.grenade_launch_ms + c.grenade_look_after_ms.max(retreat.told_ms);
    let reach_m = ARRIVE_MPS * fuse.saturating_sub(free_ms) as f32 / 1000.0;
    let Some(from) = tactics.world.grid.locate(me.feet, 1.5) else {
        return false;
    };
    let radius = arc::Grenade::bots().splash_m + retreat.margin_m.max(c.grenade_close_margin_m);
    let open = RouteFilter::open();
    let from_contact = |m: f32| landing.contact.point + landing.heading * m;
    [landing.contact.point, landing.rest, from_contact(arc::ROLL_P80_M), from_contact(arc::ROLL_WORST_M)]
        .into_iter()
        .all(|blast| {
            tactics
                .step_out_where(from, blast, radius, reach_m, now, side, false, &open, true)
                .is_some_and(|out| (out + Vec3::new(0.0, CHEST, 0.0) - blast).length() >= radius)
        })
}

/// An enemy sensed staying at one place.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Camp {
    who: Place,
    at: Vec3,
    since: u32,
    last: u32,
}

/// A threat to turn to: a hit's likely shooter, or a near shot heard from
/// outside the view.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Threat {
    /// Where it was sensed, and that place's error, metres.
    at: Vec3,
    error_m: f32,
    until: u32,
    /// Made by a hit (play 7 hides from it).
    hit: bool,
    /// Only a direction (the HUD's hit marker): `at` is a point along it
    /// at `hit_look_m`, not where the shooter is.
    bearing: Option<f32>,
}

/// [friends] A shot fired or held near a teammate is traced at most this
/// often, ms ...
pub const FRIEND_NOTE_MS: u32 = 100;
/// ... when the teammate nearest to blocking it was within this of the
/// margin, metres ([`super::friends::Near::room`]).
pub const FRIEND_NOTE_ROOM_M: f32 = 1.0;

/// [measure] The fire gates: why a tick fired no shot. A tick's
/// [`Gates::bits`] has a bit for each gate that failed, the outer ones
/// (the bot's: getting unstuck, the play, a jump held, a use) first, then
/// the reflex's in the order its fire step tests them. Nothing decides by
/// them; the bench's gate tally and the trace read them.
pub mod gate {
    /// Getting unstuck had the keys: the reflex did not run.
    pub const UNSTICK: u32 = 1 << 0;
    /// The play allows no fire (a carrier, a put-in).
    pub const PLAY: u32 = 1 << 1;
    /// A jump held (`JUMP_HOLD_MS`), by the jump's source.
    pub const JUMP_ROUTE: u32 = 1 << 2;
    pub const JUMP_HOP: u32 = 1 << 3;
    pub const JUMP_UNSTICK: u32 = 1 << 4;
    pub const JUMP_OTHER: u32 = 1 << 5;
    /// A use in reach had the view: the reflex's shot given back.
    pub const USING: u32 = 1 << 6;
    /// A grenade throw had the look and the hands.
    pub const THROW: u32 = 1 << 7;
    /// A threat's look (a hit's shooter, a near shot).
    pub const THREAT: u32 = 1 << 8;
    /// No seen target.
    pub const NO_TARGET: u32 = 1 << 9;
    /// The view farther off than `within_deg`.
    pub const WITHIN: u32 = 1 << 10;
    /// The reaction after noticing (`first_shot_at`).
    pub const REACTION: u32 = 1 << 11;
    /// A reload, or the switch to the pistol, under way.
    pub const RELOAD: u32 = 1 << 12;
    pub const SWITCH: u32 = 1 << 13;
    /// No round in the weapon.
    pub const ROUNDS: u32 = 1 << 14;
    /// Farther than the weapon's range.
    pub const RANGE: u32 = 1 << 15;
    /// The hip cone does not fit and the sights are not up: running (no
    /// aim while it runs), or the aim-in under way.
    pub const RUNNING: u32 = 1 << 16;
    pub const AIMING_IN: u32 = 1 << 17;
    /// A teammate in the line of fire.
    pub const FRIEND: u32 = 1 << 18;
    /// `clear_shot`: no part of him shows; the view's ray hits the world;
    /// the aim's ray does.
    pub const HIDDEN: u32 = 1 << 19;
    pub const VIEW_RAY: u32 = 1 << 20;
    pub const AIM_RAY: u32 = 1 << 21;
    /// Every gate passed, an automatic weapon between its bursts.
    pub const PAUSE: u32 = 1 << 22;
    /// Every gate passed, the trigger did not take it (a weapon of a round a
    /// press between its presses).
    pub const TRIGGER: u32 = 1 << 23;
    /// The tick cut a burst short (a gate failed before `burst_until`).
    pub const CUT: u32 = 1 << 24;
    /// [measure] The restarts, in place of the gate they split:
    /// `NO_TARGET` with its last target lost within [`super::LOST_MS`]
    /// ([`super::Gates::target`] is him); `REACTION` at a target taken again
    /// within that of losing it; `AIMING_IN` at a target whose sights were
    /// up before.
    pub const LOST: u32 = 1 << 25;
    pub const REACTION_AGAIN: u32 = 1 << 26;
    pub const AIMING_AGAIN: u32 = 1 << 27;
    /// The shot waits for the aim to settle ([`super::Settle::WanderAndPolicy`]).
    pub const SETTLING: u32 = 1 << 28;

    /// The gates' names, in their order.
    pub const NAMES: [(u32, &str); 29] = [
        (UNSTICK, "unstick"),
        (PLAY, "play"),
        (JUMP_ROUTE, "jump_route"),
        (JUMP_HOP, "jump_hop"),
        (JUMP_UNSTICK, "jump_unstick"),
        (JUMP_OTHER, "jump_other"),
        (USING, "using"),
        (THROW, "throw"),
        (THREAT, "threat"),
        (NO_TARGET, "no_target"),
        (LOST, "lost"),
        (WITHIN, "within"),
        (REACTION, "reaction"),
        (REACTION_AGAIN, "reaction_again"),
        (RELOAD, "reload"),
        (SWITCH, "switch"),
        (ROUNDS, "rounds"),
        (RANGE, "range"),
        (RUNNING, "running"),
        (AIMING_IN, "aiming_in"),
        (AIMING_AGAIN, "aiming_again"),
        (SETTLING, "settling"),
        (FRIEND, "friend"),
        (HIDDEN, "hidden"),
        (VIEW_RAY, "view_ray"),
        (AIM_RAY, "aim_ray"),
        (PAUSE, "pause"),
        (TRIGGER, "trigger"),
        (CUT, "cut"),
    ];

    /// The names of `bits`, in order.
    pub fn names(bits: u32) -> Vec<&'static str> {
        NAMES
            .iter()
            .filter(|(bit, _)| bits & bit != 0)
            .map(|(_, name)| *name)
            .collect()
    }
}

/// [measure] One tick's fire gates ([`gate`]): the tick, the gates that
/// failed, and the target the reflex aimed at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gates {
    pub at: u32,
    pub bits: u32,
    pub target: Option<Place>,
}

/// [measure] Why a reload started ([`Reflex::reload_note`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadWhy {
    /// The magazine out with a target seen: out of sight first, or where
    /// it stands.
    EmptyInView,
    /// The magazine out, no target seen.
    Empty,
    /// Before the magazine is out ([`CombatTuning::reload_idle_ms`]).
    Tactical,
}

/// [measure] The last reload: when it started and is done, why, and the
/// rounds left of the magazine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReloadNote {
    pub at: u32,
    pub until: u32,
    pub why: ReloadWhy,
    pub left: u32,
    pub magazine: u32,
}

/// [measure] A target lost this long ago, ms, counts as lost a moment
/// ([`gate::LOST`], [`gate::REACTION_AGAIN`]).
pub const LOST_MS: u32 = 1000;

/// [measure] A bot's own numbers at a shot (A228, the fire gate): when (ms), its target's distance (m), how far its view was from
/// where it meant to aim (the gate's test, `off_by`: his chest moved by its aim error), the gate's bound that tick, and its aim
/// error's size (the larger of its two angles), degrees. What the shot's view was off his chest is the bench's; these split it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FireAim {
    pub at: u32,
    pub distance: f32,
    pub off_deg: f32,
    pub within_deg: f32,
    pub error_deg: f32,
}

/// Fire at a target this lately is still going, ms: a spray at the smoke he went into may go on
/// (`SmokeTuning::spray_ms`). GUESS: longer than the pause between two bursts of the best players (`aim.pause_ms`).
const SPRAY_FIRING_MS: u32 = 400;

/// [measure] A gate trace note per bot and target at most this often, ms.
pub const GATE_NOTE_MS: u32 = 1000;
/// [measure] ... once its view has been this long on a seen target, within
/// `fire_within_deg` + [`GATE_NOTE_SLACK_DEG`], with no shot, ms.
pub const GATE_NOTE_AFTER_MS: u32 = 300;
pub const GATE_NOTE_SLACK_DEG: f32 = 2.0;

/// A target lost to its sight ending (not to his death): in which ping
/// band of the target ([`super::polls::ping_band`]), and whether a burst
/// was cut by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lapse {
    pub band: usize,
    pub cut: bool,
}

/// A heal cover walk counts as on for this long after it begins, ms (the
/// bench's situation bit).
const HEAL_WALKING_MS: u32 = 1_600;

/// A heal press owes its cover walk only if the think comes within this, ms
/// (a late walk would not use the item's delay).
const HEAL_WALK_LATE_MS: u32 = 600;

/// [measure] What fire through cover did in one life of a reflex (the bench's owner counts add the lives up; nothing decides by
/// them): ticks the trigger was pressed at a target not in sight, bursts begun (one per [`CombatTuning::fire_through_gap_ms`] at
/// most), and the chances refused by what: the damage share under the bar, a mate in the line, the gap since the last burst.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThroughStats {
    pub ticks: u32,
    pub bursts: u32,
    pub refused_share: u32,
    pub refused_friend: u32,
    pub refused_gap: u32,
}

impl ThroughStats {
    pub fn total(&self) -> u32 {
        self.ticks + self.bursts + self.refused_share + self.refused_friend + self.refused_gap
    }

    pub fn add(&mut self, other: &Self) {
        self.ticks += other.ticks;
        self.bursts += other.bursts;
        self.refused_share += other.refused_share;
        self.refused_friend += other.refused_friend;
        self.refused_gap += other.refused_gap;
    }
}

/// [measure] What the outranged rule did in one life of a reflex (the bench's owner counts add the lives up; nothing decides by
/// them), in reflex ticks: the target seen was a sniper beyond the weapon's band (`sightings`), and he was facing it
/// (`facing`: the cover rule's case); of them, ticks with the bot in the open (`exposed`); breaks of sight begun (`hides`);
/// ticks it kept its way instead because it carried or was a thief (`task_kept`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutrangedStats {
    pub sightings: u32,
    pub facing: u32,
    pub exposed: u32,
    pub hides: u32,
    pub task_kept: u32,
}

impl OutrangedStats {
    pub fn total(&self) -> u32 {
        self.sightings + self.facing + self.exposed + self.hides + self.task_kept
    }

    pub fn add(&mut self, other: &Self) {
        self.sightings += other.sightings;
        self.facing += other.facing;
        self.exposed += other.exposed;
        self.hides += other.hides;
        self.task_kept += other.task_kept;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Reflex {
    target: Option<Place>,
    /// When it took the target.
    since: u32,
    /// The error drawn for this target, radians (yaw, pitch).
    error: (f32, f32),
    burst_until: u32,
    pause_until: u32,
    /// A heard shot to turn to, and until when.
    sound: Option<(Vec3, u32)>,
    /// A threat looked at even with a target, and when the next may come.
    threat: Option<Threat>,
    next_threat: u32,
    /// Play 6: the next turn to a heard shot not before this.
    next_sound: u32,
    /// A glance with no target (a callout), and until when; the next not
    /// before `next_told`; the team's "shot from there" callouts up to
    /// this time were weighed.
    glance: Option<(Vec3, u32)>,
    next_told: u32,
    told_until: u32,
    /// Hit with no shooter sensed: where it stood and the marker's
    /// direction, for its team's callout ([`Self::take_hit_note`]).
    hit_note: Option<(Vec3, f32)>,
    /// [threat.concede, item 15] Its last hit from a shooter it sensed ([`Self::last_hit`]).
    last_hit: Option<super::concede::Hit>,
    /// Outranged by the target (play 11's band); the next hide from it
    /// not before `next_outranged`.
    outranged: bool,
    next_outranged: u32,
    /// No shot at the target before this: noticed + the reaction delay.
    first_shot_at: u32,
    /// The last tick it fired, and at whom: a spray into smoke is only fire already going ([`SPRAY_FIRING_MS`]).
    last_fire: Option<(Place, u32)>,
    /// [measure] Its own numbers at its last shot ([`FireAim`], A228).
    fire_aim: Option<FireAim>,
    /// The aim's last fresh look found its target behind smoke; the aim sprays at the last point seen now (each
    /// a note when it begins); the notes not yet taken.
    smoke_lost: bool,
    spraying: bool,
    smoke_notes: Vec<String>,
    /// [smoke] The quick-use key that throws a smoke from the kit now, where the pockets are known; the smoke throw
    /// under way.
    smoke_key: Option<u32>,
    smoke_throw: Option<SmokeThrowing>,
    /// [A208] A hold for a cloud under way.
    cloud_hold: Option<CloudHolding>,
    /// [A208] The crossing of the last hold that ended and when: a mate's cloud holds it there no second time.
    hold_ended: Option<(Vec3, u32)>,
    /// [A208, the bench's] The last thrown smoke's crossing.
    #[cfg(test)]
    crossing_watch: Option<CrossingWatch>,
    /// [smoke] The lane smokes and the sniper smokes begun this life (`smoke.lanes_a_life`, `smoke.snipers_a_life`; the
    /// reflex is made anew each life).
    lane_smokes: u32,
    sniper_smokes: u32,
    /// [smoke] The sniper that outranges its weapon now (play 11's matchup, whatever `outranged_cover`): who, where,
    /// and the match clock it was seen at. Read by `smoke.anti_sniper`; no decision of the reflex reads it.
    outranged_by: Option<(Place, Vec3, u32)>,
    /// The life it counts for (`PlaceView::spawned`).
    life: Option<u32>,
    /// Its health at the last think.
    health: Option<f32>,
    /// The last think that saw its health drop ([`CombatTuning::hit_drop`]):
    /// for `hit_shot_ms` after it the trigger's range is the ceiling, not
    /// the weapon class's ([`CombatTuning::range_by_weapon`]), so a bot hit
    /// from beyond its range may answer.
    hit_at: Option<u32>,
    duel_until: u32,
    /// Reload and switch sounds up to this time were weighed.
    punished_until: u32,
    push: Option<Push>,
    cover: Cover,
    /// The spot it last fought from (play 7: not again next).
    last_peek: Option<Vec3>,
    band: Option<(f32, f32)>,
    /// A step into the band.
    range_step: Option<Vec3>,
    /// How the legs go in a fight (`stand_strafe`).
    gait: Gait,
    strafe_stats: StrafeStats,
    /// Which side of the line the last range step went to (`band_cross_deg`): the next new step takes the other.
    range_right: bool,
    spot_pending: Option<Place>,
    last_spot: Option<u32>,
    spotted: Vec<(Place, u32)>,
    throw: Option<Throw>,
    /// The heal habit ([`super::plays::heal::wish`]): what `drive` asked for
    /// this tick, taken by [`Self::tick`], which presses the key.
    heal_wish: Option<HealWish>,
    /// The cover walk owed to the press just made, and when it was made:
    /// begun at the next think (the item's delay is the walking time).
    heal_walk: Option<(HealWish, u32)>,
    /// When the last heal walk began (for the bench's situations).
    heal_walk_at: Option<u32>,
    /// Why the last heal press's walk was or was not made.
    heal_note: Option<HealNote>,
    /// The bench's counts: heal presses made, cover walks begun.
    pub heal_presses: u32,
    pub heal_walks: u32,
    thrown: u8,
    next_grenade_roll: u32,
    /// The fragmentation grenades the kit holds now, where known (`Throws::frags_left`, set each poll); none where not.
    frags_left: Option<u16>,
    /// Rolls for a grenade refused because the kit held no frag (`grenade_kit_cap`), this life.
    refused_no_frag: u32,
    /// [measure] What the grenade logic did this life ([`GrenadeStats`]).
    grenade_stats: GrenadeStats,
    /// A grenade decided on at this place, for the team's claim
    /// ([`Reflex::take_throw_claim`]).
    throw_claim: Option<Vec3>,
    /// [A170] The shooters of the held spots the last roll offered as places (the note of a throw at one names its shooter).
    spot_shooters: Vec<(Vec3, Place, u32)>,
    /// The enemy and the time it last fired at him: the team's hit marker on him
    /// ([`Reflex::fired_at`]).
    last_fired: Option<(Place, u32)>,
    /// What the team's knowledge made of each enemy at the last think ([`Known`]).
    known: Vec<Known>,
    /// [measure] What the worth, the push and the flank glance did this life, and the bot notes owed
    /// ([`Reflex::take_worth_notes`]).
    worth_stats: WorthStats,
    worth_notes: Vec<String>,
    /// How far the weapon in hand reaches (the upper end of its band and the slack), as the last think had it: the reach of an
    /// objective actor's worth ([`WorthTuning::objective_in_band`]); none: no weapon known.
    reach_m: Option<f32>,
    /// The bullet the bot's cover tests go by, as the last think had it (`Situation::cover`): fire through cover asks the
    /// share of a shot's damage that reaches a place.
    cover_rule: Option<super::tuning::CoverRule>,
    /// When the last burst of fire through cover began ([`CombatTuning::fire_through_gap_ms`] between two); none yet this life.
    through_at: Option<u32>,
    /// [measure] What fire through cover did this life.
    through_stats: ThroughStats,
    /// [measure] What the outranged rule saw and did this life.
    outranged_stats: OutrangedStats,
    /// The enemy who hit it last (the threat's shooter it took) and until when it is kept as the target whatever the
    /// worth of the others (`worth.hurter_keep_ms`).
    hurter: Option<(Place, u32)>,
    /// The time of the last bullet that hit it, a limb's too (the world's `m_last_hit_time_ms`): `hit_at` sees only a fall of the
    /// body's health.
    bullet_hit_at: Option<u32>,
    /// Not before this, a flank glance ([`Reflex::flank_glance`]); the threat looks have their own pace.
    next_flank_glance: u32,
    /// [C20] The window for a second grenade at the place of the last one
    /// let go (`grenade_volley`).
    volley: Option<Volley>,
    /// When the last grenade was let go.
    last_throw: Option<u32>,
    /// [play 20] Throws made and given up since the last think, for the
    /// bot's notes ([`Reflex::take_throw_notes`]).
    throw_notes: Vec<String>,
    /// Enemies seen holding an angle: who, where, when.
    held: Vec<(Place, Vec3, u32)>,
    camps: Vec<Camp>,
    /// The partner's death spot to watch, until when.
    trade: Option<(Vec3, u32)>,
    /// What it carries (its role's; `None`: [`Arms::fallback`]), kept
    /// from life to life; and this life's rounds, from the first tick or
    /// think of the life.
    arms: Option<&'static Arms>,
    /// The rounds of weapon 1 at this life's spawn: what the kit has left
    /// (a place's ammo lasts the match, a spawn does not refill it);
    /// `None`: the arms' whole count.
    carried: Option<u32>,
    hands: Option<Hands>,
    /// An automatic weapon: the trigger held this long without a break,
    /// ms (its shots, [`super::guns::Gun::shots_held`]).
    held_ms: u32,
    /// A one-round-a-press weapon: the present press is held until this;
    /// the next press not before `next_press`.
    press_until: u32,
    next_press: u32,
    /// The rounds the trigger counted and when ([`Reflex::hold_fire`]).
    fired: (u32, u32),
    /// The last tick and the time since the one before, ms (the fire's
    /// time).
    last_tick: Option<u32>,
    tick_ms: u32,
    /// [friends] The teammate it held fire for at its last tick.
    held_for: Option<super::friends::Held>,
    /// [friends] The match traces: a shot fired or held near a teammate is
    /// noted ([`Reflex::take_friend_note`]) ...
    pub tracing: bool,
    /// ... at most every [`FRIEND_NOTE_MS`], the last at this.
    friend_noted: u32,
    friend_note: Option<serde_json::Value>,
    /// [measure] This tick's fire gates ([`Reflex::gates`]).
    gates: Gates,
    /// [measure] The last reload ([`Reflex::reload_note`]).
    reload_note: Option<ReloadNote>,
    /// [measure] What the weapon mods did to the reloads of this life.
    mod_counts: ModCounts,
    /// [measure] The bench's gate tally: every gate is tested each tick,
    /// `clear_shot`'s rays too, though an earlier one failed (no decision
    /// changes; only the rays cost).
    pub gate_tally: bool,
    /// [measure] The source of the jump held ([`Reflex::jumped`]), a
    /// [`gate`] bit; what keeps the weapon busy, likewise.
    jump_by: u32,
    busy_why: u32,
    /// [measure] With `tracing`: since when the view is on the target with
    /// no shot; the last gate note, its target and time.
    on_since: Option<(Place, u32)>,
    gate_noted: Option<(Place, u32)>,
    gate_note: Option<serde_json::Value>,
    /// [measure] The target whose sights were last up; the last target
    /// lost and when; whether the present one was taken again within
    /// [`LOST_MS`] of losing it.
    aimed_target: Option<Place>,
    lost: Option<(Place, u32)>,
    reacquired: bool,
    /// The lost target's take: since, the error, the first shot's time
    /// (`CombatTuning::reacquire_keep`).
    lost_take: (u32, (f32, f32), u32),
    /// The target was taken back before he was noticed again.
    retaken: bool,
    /// An enemy last seen (a seen track) at.
    last_seen: u32,
    /// Empty with a target seen and no cover found: reload where it is.
    reload_here: bool,
    /// No shot before this (a reload or the switch to the pistol under
    /// way).
    busy_until: u32,
    /// The trigger or the sights were in the keys of the last tick: what the
    /// client's reload loop meets at its round-ends
    /// ([`Self::count_rounds`]).
    held_keys: bool,
    /// [measure] The end of the last round-by-round reload's loop
    /// ([`Reflex::loop_end`]).
    loop_end: Option<LoopEnd>,
    /// The aim's slow wander about the target, radians (yaw, pitch).
    wander: (f32, f32),
    /// Drawn per target: it aims down the sights nearer than `ads_near_m`;
    /// it crouches to fire (`FireTuning::crouch_chance`); it side-steps
    /// while it fires (`FireTuning::strafe_chance`).
    ads_near: bool,
    /// Since when its aim is pressed (`CombatTuning::aim_in_first`).
    aim_since: Option<u32>,
    crouch_fire: bool,
    strafe_fire: bool,
    /// The side step: the key held (0: still) and until when.
    strafe: u32,
    strafe_until: u32,
    /// The last way it stepped, to step the other way next.
    strafe_last: u32,
    /// [aimpoint] The target's parts that show, the view's line
    /// ([`Self::look`]).
    sight: super::aimpoint::Sight,
    /// The last tick's lapse of the target's sight, if it was one
    /// ([`Self::take_lapse`]).
    lapse: Option<Lapse>,
    /// [recoil] The kick of the weapon in hand, when its trigger fires,
    /// and how much the kick moved the view at this tick's weapon update.
    recoil: Recoil,
    cadence: Cadence,
    kick_step: f32,
    /// The kick the view had at the start of this tick's weapon update (what [`Self::weapon_tick`] returned).
    kick_before: f32,
    /// The mirror counts the shots the weapon can fire ([`CombatTuning::mirror_gated`], set each tick by [`Self::gate_mirror`]), and the
    /// earliest the client's weapon can fire again, ms: the last counted shot's fire clip, and the bolt after it.
    gated: bool,
    ready_at: f32,
    /// [bench] The mirror's kick after the tick's turn and before its shots (pitch units), and the shots it counted in the tick:
    /// what the stand-in's own kick is read against.
    #[cfg(test)]
    seen: (f32, u8),
    /// The pitch unit's scale as the last tick's tuning has it (`kin.pitch_scale`): the kick is in the game's unit, the aim in elevations.
    scale: PitchScale,
}

/// The bearing of `to` from `from`, radians.
fn bearing(from: Vec3, to: Vec3) -> f32 {
    yaw_of([to.x - from.x, to.z - from.z])
}

/// How far the view of `me` is off looking at `point` (plus `error`),
/// radians.
fn off_by(me: &PlaceView, point: Vec3, error: (f32, f32)) -> f32 {
    let eye = me.eye();
    let (dx, dz) = (point.x - eye.x, point.z - eye.z);
    let want_yaw = yaw_of([dx, dz]) + error.0;
    let want_pitch = (point.y - eye.y).atan2(dx.hypot(dz)) + error.1;
    wrap(want_yaw - me.yaw)
        .abs()
        .max((want_pitch - me.pitch).abs())
}

/// Play 7: whether a peek from `spot` is another angle on `enemy` than the
/// last one, from `last`.
fn another_angle(enemy: Vec3, last: Vec3, spot: Vec3, c: &CombatTuning) -> bool {
    wrap(bearing(enemy, spot) - bearing(enemy, last)).abs() >= c.repeek_min_deg.to_radians()
        || spot.distance_xz(last) >= c.repeek_min_m
}

/// `from` moved `m` metres on the ground toward `to` (away if negative).
/// [`step_toward`] turned `angle` radians off the line to one side (`right`: the bot's right when facing `to`), `m` still the
/// way along the line (so the step is `m / cos(angle)` long).
fn step_across(from: Vec3, to: Vec3, m: f32, angle: f32, right: bool) -> Vec3 {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let d = dx.hypot(dz).max(1e-3);
    let (ux, uz) = (dx / d, dz / d);
    // The left normal of the line is (-uz, ux) in x/z with z forward; the right one is its opposite.
    let side = if right { -1.0 } else { 1.0 };
    let (nx, nz) = (-uz * side, ux * side);
    let (along, across) = (m, m * angle.tan());
    Vec3::new(from.x + ux * along + nx * across, from.y, from.z + uz * along + nz * across)
}

fn step_toward(from: Vec3, to: Vec3, m: f32) -> Vec3 {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let d = dx.hypot(dz).max(1e-3);
    Vec3::new(from.x + dx / d * m, from.y, from.z + dz / d * m)
}

/// The direction the HUD's hit marker shows a bot `me` just hit, from the
/// shots of the last `hit_shot_ms`: the enemy whose view is on the bot
/// (within `hit_aim_deg`, or the bot's width) is the shooter, the nearest
/// to his line first. Only the direction from the bot to him comes out,
/// with the marker's error drawn in.
fn hit_bearing(
    me: &PlaceView,
    view: &MatchView,
    now: u32,
    c: &CombatTuning,
    rng: &mut Rng,
) -> Option<f32> {
    let mut best: Option<(f32, f32)> = None;
    for sound in &view.sounds {
        if sound.kind != SoundKind::Shot || sound.time + c.hit_shot_ms < now {
            continue;
        }
        let Some(shooter) = view.place(sound.from).filter(|p| p.team != me.team) else {
            continue;
        };
        let d = sound.at.distance_xz(me.feet);
        if d < 1.0 {
            continue;
        }
        let off = wrap(shooter.yaw - bearing(sound.at, me.feet)).abs();
        let within = c.hit_aim_deg.to_radians().max(1.5f32.atan2(d));
        if off <= within && best.is_none_or(|(o, _)| off < o) {
            best = Some((off, bearing(me.feet, sound.at)));
        }
    }
    let error = c.hit_bearing_error_deg.to_radians();
    best.map(|(_, b)| wrap(b + rng.range(-error, error)))
}

/// Audit issue 8: whether `gun`'s cone from the hip (the weapon's base and
/// the player's stance and movement value times its hip multiplier,
/// `survarium::v026g::weapons`) keeps the shots on a body `distance` away:
/// `hip_cone_share` of its half within `body_half_m` of him. Moving as the
/// game reads it, from the keys (forward xor back, left xor right), or
/// `strafing` (it side-steps to fire).
fn hip_cone_fits(me: &PlaceView, gun: &Gun, strafing: bool, distance: f32, c: &CombatTuning) -> bool {
    use survarium::v026g::weapons::{Stance, character_dispersion};
    let keys = me.keys;
    let fb = keys & (key::FORWARD | key::BACK);
    let lr = keys & (key::LEFT | key::RIGHT);
    let stance = Stance {
        crouched: keys & key::CROUCH != 0,
        moving: strafing
            || fb == key::FORWARD
            || fb == key::BACK
            || lr == key::LEFT
            || lr == key::RIGHT,
        sprinting: keys & key::SPRINT != 0,
        jumping: keys & key::JUMP != 0,
        aimed: false,
    };
    let d = &gun.dispersion;
    let cone = d.cone_deg(character_dispersion().of(stance, d), 0.0, false);
    (cone * 0.5 * c.hip_cone_share).to_radians().tan() * distance <= c.body_half_m
}

/// Whether the seen enemy `other` takes the target from the seen `kept`
/// (audit issue 4: a bot aimed 44-46 m away with the owner in its sight at
/// 22-28 m): much nearer (`switch_share` of its distance) at once; after
/// `switch_hold_ms` on the target, also nearer than `switch_near_m` and
/// `switch_margin_m` nearer, or facing the bot (his view within
/// `switch_facing_deg` of it, as a player sees which way a man looks) and
/// `switch_facing_margin_m` nearer while the kept one does not face it.
fn presses_more(
    me: &PlaceView,
    view: Option<&MatchView>,
    worth: Option<&Worth>,
    kept: &SeenTrack,
    other: &SeenTrack,
    held_ms: u32,
    c: &CombatTuning,
) -> bool {
    let (far, near) = (
        kept.at().distance_xz(me.feet),
        other.at().distance_xz(me.feet),
    );
    if near < far * c.switch_share {
        return true;
    }
    if held_ms < c.switch_hold_ms {
        return false;
    }
    // Worth more by the margin (an objective actor who cannot shoot back, a thief of the team's battery): after the
    // hold, not in the middle of a duel.
    if let Some(worth) = worth
        && worth.tuning.switch_m > 0.0
        && worth.of(other.who()) - worth.of(kept.who()) >= worth.tuning.switch_m
        && worth.pays(other.who(), kept.who(), near, far)
    {
        return true;
    }
    if near < c.switch_near_m && near + c.switch_margin_m <= far {
        return true;
    }
    let faces = |who: Place, at: Vec3| {
        c.switch_facing_deg > 0.0
            && view.and_then(|v| v.place(who)).is_some_and(|p| {
                wrap(p.yaw - bearing(at, me.feet)).abs() <= c.switch_facing_deg.to_radians()
            })
    };
    near + c.switch_facing_margin_m <= far
        && faces(other.who(), other.at())
        && !faces(kept.who(), kept.at())
}

/// A point `m` metres from `from` along the yaw `bearing`.
fn along(from: Vec3, bearing: f32, m: f32) -> Vec3 {
    let [fx, fz] = forward(bearing);
    Vec3::new(from.x + fx * m, from.y, from.z + fz * m)
}

impl Reflex {
    /// A reflex for a bot carrying `arms` (`None`: [`Arms::fallback`]).
    pub fn armed(arms: Option<&'static Arms>) -> Self {
        static BORN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        Self {
            arms,
            grenade_stats: GrenadeStats {
                born: BORN.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                ..GrenadeStats::default()
            },
            ..Self::default()
        }
    }

    /// A reflex for a bot carrying `arms` with `carried` rounds of weapon
    /// 1 left in its kit.
    pub fn armed_carrying(arms: Option<&'static Arms>, carried: u32) -> Self {
        Self {
            carried: Some(carried),
            ..Self::armed(arms)
        }
    }

    pub fn arms(&self) -> Option<&'static Arms> {
        self.arms
    }

    /// Where play 7 (the hide from an outranging enemy) is: "open", "hiding" (walking to the spot or waiting at it) or
    /// "peeking" (back at another angle). For the bench's tallies.
    pub fn cover_state(&self) -> &'static str {
        match self.cover {
            Cover::Open => "open",
            Cover::Hiding { .. } => "hiding",
            Cover::Peeking { .. } => "peeking",
        }
    }

    /// The rounds of weapon 1 its kit has left, in it and to load.
    pub fn left(&self) -> u32 {
        match &self.hands {
            Some(hands) => hands.primary.rounds + hands.primary.spare,
            None => self
                .carried
                .unwrap_or(self.arms.unwrap_or_else(Arms::fallback).primary.1),
        }
    }

    /// A resupply zone's top-up: `rounds` more of weapon 1's ammo in the
    /// stack (the magazine is not touched; the reload habit loads it).
    pub fn top_up(&mut self, rounds: u32) {
        match &mut self.hands {
            Some(hands) => hands.primary.spare += rounds,
            None => self.carried = Some(self.left() + rounds),
        }
    }

    /// Nothing to fire: weapon 1 has no round in it nor to load, and no
    /// pistol with one (the kit's ammo lasts the match: dry until a spawn
    /// with another kit).
    /// [threat.concede, item 15] Its last hit from a shooter it sensed: where he was, where it stood, when (none in a new life:
    /// the reflex is new each life).
    pub fn last_hit(&self) -> Option<super::concede::Hit> {
        self.last_hit
    }

    pub fn dry(&self) -> bool {
        match &self.hands {
            Some(hands) => hands.primary.dry() && hands.pistol.is_none_or(|p| p.dry()),
            None => {
                self.left() == 0
                    && self
                        .arms
                        .unwrap_or_else(Arms::fallback)
                        .pistol
                        .is_none_or(|(_, rounds)| rounds == 0)
            }
        }
    }

    /// The weapon in hand (weapon 1 before the life's first use).
    pub fn held_gun(&self) -> &'static Gun {
        match &self.hands {
            Some(hands) => hands.held().gun,
            None => self.arms.unwrap_or_else(Arms::fallback).primary.0,
        }
    }

    /// This life's weapons, from its arms at the first use.
    pub fn hands(&mut self) -> &mut Hands {
        let arms = self.arms.unwrap_or_else(Arms::fallback);
        let carried = self.carried.unwrap_or(arms.primary.1);
        self.hands.get_or_insert_with(|| {
            Hands::of(&Arms {
                primary: (arms.primary.0, carried),
                ..*arms
            })
        })
    }

    pub fn target(&self) -> Option<Place> {
        self.target
    }

    /// [friends] The teammate it held fire for at its last tick, if any.
    pub fn held_for(&self) -> Option<super::friends::Held> {
        self.held_for
    }

    /// [friends] The trace line of the last tick's shot, fired or held,
    /// with the teammate nearest to the line as the bot had him
    /// ([`super::friends::closest`]), if he was within
    /// [`FRIEND_NOTE_ROOM_M`] of blocking it.
    pub fn take_friend_note(&mut self) -> Option<serde_json::Value> {
        self.friend_note.take()
    }

    /// [measure] The fire gates of the last tick it ran or was kept from
    /// running ([`gate`]).
    pub fn gates(&self) -> Gates {
        self.gates
    }

    /// [measure] What the weapon mods did to this life's reloads.
    pub fn mod_counts(&self) -> ModCounts {
        self.mod_counts
    }

    /// [measure] The last reload started, if any.
    pub fn reload_note(&self) -> Option<ReloadNote> {
        self.reload_note
    }

    /// [measure] Its own numbers at its last shot, if any (A228).
    pub fn fire_aim(&self) -> Option<FireAim> {
        self.fire_aim
    }

    /// [measure] The end of the last round-by-round reload's loop, if any: the
    /// rounds the client put in, of those it was to.
    pub fn loop_end(&self) -> Option<LoopEnd> {
        self.loop_end
    }

    /// [measure] The gate trace note made since the last call (with
    /// `tracing`), if any.
    pub fn take_gate_note(&mut self) -> Option<serde_json::Value> {
        self.gate_note.take()
    }

    /// Its last target `who`, lost a moment ago, taken back as it had him
    /// (`CombatTuning::reacquire_keep`): the error settled, the reaction
    /// waited; seen in sight before he is noticed again.
    fn retake(&mut self, who: Place) {
        self.reacquired = true;
        self.retaken = true;
        self.target = Some(who);
        (self.since, self.error, self.first_shot_at) = self.lost_take;
        self.sound = None;
    }

    /// The target as seen: noticed ([`Memory::seen`]); taken back before
    /// he is noticed again ([`Self::retake`]), in sight
    /// ([`Memory::in_sight`]).
    fn seen_target(
        &self,
        memory: &Memory,
        target: Place,
        now: u32,
        sight_now: u32,
    ) -> Option<SeenTrack> {
        memory.seen(target, now, sight_now).or_else(|| {
            self.retaken
                .then(|| memory.in_sight(target, now, sight_now))
                .flatten()
        })
    }

    /// The target is no more (none seen): [measure] noted as lost at `now`.
    fn lose_target(&mut self, now: u32) {
        self.retaken = false;
        if let Some(who) = self.target.take() {
            self.lost = Some((who, now));
            self.lost_take = (self.since, self.error, self.first_shot_at);
        }
    }

    /// [measure] The gates of the tick at `now`: those noted already this
    /// tick are kept, an earlier tick's dropped.
    fn gate_tick(&mut self, now: u32) -> &mut Gates {
        if self.gates.at != now {
            self.gates = Gates {
                at: now,
                ..Gates::default()
            };
        }
        &mut self.gates
    }

    /// [measure] The bot's own gates before its tick: the play allows no
    /// fire; a jump is held (by the source [`Self::jumped`] noted).
    /// The heal habit's word for this tick (`None`: nothing to press).
    pub fn set_heal(&mut self, wish: Option<HealWish>) {
        self.heal_wish = wish;
    }

    /// A heal cover walk begun within the last 1.6 s of the last tick (the
    /// bench's situation bit).
    pub fn heal_walking(&self) -> bool {
        matches!((self.heal_walk_at, self.last_tick), (Some(at), Some(now)) if now < at + HEAL_WALKING_MS)
    }

    pub fn outer_gates(&mut self, now: u32, play_off: bool, jump_held: bool) {
        let jump = self.jump_gate();
        let g = self.gate_tick(now);
        if play_off {
            g.bits |= gate::PLAY;
        }
        if jump_held {
            g.bits |= jump;
        }
    }

    /// [measure] Getting unstuck had the keys at `now`: no tick ran.
    pub fn skipped_tick(&mut self, now: u32) {
        self.gate_tick(now).bits |= gate::UNSTICK;
    }

    /// [measure] A use in reach took the view at `now`: the shot given back.
    pub fn gate_using(&mut self, now: u32) {
        self.gate_tick(now).bits |= gate::USING;
    }

    /// [measure] A jump pressed (`by`: "route", "hop", "unstick") starts a
    /// hold: its source, for the hold's ticks.
    pub fn jumped(&mut self, by: &str) {
        self.jump_by = match by {
            "route" => gate::JUMP_ROUTE,
            "hop" => gate::JUMP_HOP,
            "unstick" => gate::JUMP_UNSTICK,
            _ => gate::JUMP_OTHER,
        };
    }

    /// [measure] The source of the jump held, as a gate.
    fn jump_gate(&self) -> u32 {
        if self.jump_by == 0 {
            gate::JUMP_OTHER
        } else {
            self.jump_by
        }
    }

    /// [measure] The jump held took this tick's trigger at `now`.
    pub fn gate_jump(&mut self, now: u32) {
        let jump = self.jump_gate();
        self.gate_tick(now).bits |= jump;
    }

    /// [play 20] The throws let go since the last call ("throws a grenade
    /// at (x, y, z), blast at <ms>") and those given up for a teammate in
    /// the blast ("holds a grenade back at (x, y, z): teammate <place>").
    pub fn take_throw_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.throw_notes)
    }

    /// [measure] How long ago, ms, the last grenade was let go.
    pub fn thrown_ago(&self, now: u32) -> Option<u32> {
        self.last_throw.map(|at| now.saturating_sub(at))
    }

    /// [measure] Whether the legs are in the push after a grenade at `now`.
    pub fn after_grenade(&self, now: u32) -> bool {
        self.push
            .is_some_and(|p| p.why == LegsWhy::AfterGrenade && p.from <= now && now < p.until)
    }

    /// Whether the last shot of this bot at `victim` left within `window_ms` before a bullet hit on him made at the
    /// world's time `hit_ms` (the bot decides up to the encoder's lead ahead of the world, so a shot that far past it
    /// still counts): the hit marker of its own bullet. When it did, the time of the shot and the share of a body's
    /// pool the nominal damage of the weapon it fires is ([`Gun::bullet_damage`]).
    pub fn shot_landed(&self, victim: Place, hit_ms: u32, window_ms: u32) -> Option<(u32, f32)> {
        let lead = (super::encode::LEAD_TICKS + 1) * survarium::v026g::match_msgs::TICK_MS;
        self.last_fired
            .filter(|&(who, at)| who == victim && at <= hit_ms.saturating_add(lead) && hit_ms.saturating_sub(at) <= window_ms)
            .map(|(_, at)| (at, self.held_gun().bullet_damage.clamp(0.0, 1.0)))
    }

    /// [measure] What fire through cover did this life.
    pub fn through_stats(&self) -> ThroughStats {
        self.through_stats
    }

    /// [measure] What the outranged rule did this life.
    pub fn outranged_stats(&self) -> OutrangedStats {
        self.outranged_stats
    }

    pub fn worth_stats(&self) -> WorthStats {
        self.worth_stats
    }

    /// The notes of the decisions the worth made since the last call (a target taken for what he is
    /// worth, a flank glance): one per decision.
    pub fn take_worth_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.worth_notes)
    }

    /// TEST ONLY: this bot fired at `victim` at `at`.
    #[cfg(test)]
    pub(crate) fn fired_on(&mut self, victim: Place, at: u32) {
        self.last_fired = Some((victim, at));
    }

    /// The world's word on the last bullet that hit this bot (none: no hit in its life so far).
    pub fn bullet_hit(&mut self, last_hit_ms: Option<u32>) {
        self.bullet_hit_at = last_hit_ms;
    }

    /// The hurt enemy it pushes on now and until when: the team's cap counts this bot while it does
    /// (`Wounds::push_claim`), and not after (`Wounds::push_release`).
    pub fn pushing(&self) -> Option<(Place, u32)> {
        match self.push {
            Some(Push { why: LegsWhy::Finish(victim), until, .. }) => Some((victim, until)),
            _ => None,
        }
    }

    /// The place a grenade was decided on since the last call: the team's
    /// claim on it (`TeamKnowledge::claim_throw`).
    pub fn take_throw_claim(&mut self) -> Option<Vec3> {
        self.throw_claim.take()
    }

    /// [measure] What the grenade logic has done this life, counted as it
    /// ran; nothing decides by it.
    pub fn grenade_stats(&self) -> GrenadeStats {
        self.grenade_stats
    }

    /// The kit's frags now, as the last poll told (none: the kit was not read).
    pub fn frags_left(&self) -> Option<u16> {
        self.frags_left
    }

    /// A test's cover rule, as a think's `Situation::cover` gives it.
    #[cfg(test)]
    pub fn set_cover_rule(&mut self, rule: Option<super::tuning::CoverRule>) {
        self.cover_rule = rule;
    }

    /// The kit's frags now, for the grenade logic ([`CombatTuning::grenade_kit_cap`]).
    pub fn set_frags_left(&mut self, frags: Option<u16>) {
        self.frags_left = frags;
    }

    /// Rolls for a grenade refused this life because the kit held no frag.
    pub fn refused_no_frag(&self) -> u32 {
        self.refused_no_frag
    }

    /// Grenades thrown this life.
    pub fn grenades_thrown(&self) -> u8 {
        self.thrown
    }

    /// Hit with no shooter sensed at the last think: where it stood and
    /// the direction its HUD's hit marker showed, once (its team's
    /// "shot from there" callout).
    pub fn take_hit_note(&mut self) -> Option<(Vec3, f32)> {
        self.hit_note.take()
    }

    /// At a think, with the view alone: the target and the turn to a sound
    /// (the first form; [`Self::think_with`] does everything).
    pub fn think(
        &mut self,
        me: &PlaceView,
        memory: &Memory,
        now: u32,
        aim: &AimTuning,
        skill: f32,
        rng: &mut Rng,
    ) {
        self.new_life(me);
        self.choose(me, None, memory, now, aim, skill, rng);
    }

    /// At a think: the target, and plays 5, 7, 11, 19, 20 and 21.
    #[allow(clippy::too_many_arguments)]
    pub fn think_with(
        &mut self,
        me: &PlaceView,
        sit: &Situation,
        memory: &Memory,
        now: u32,
        aim: &AimTuning,
        skill: f32,
        rng: &mut Rng,
    ) {
        let c = &aim.combat;
        self.new_life(me);
        let hit = self
            .health
            .is_some_and(|before| me.health <= before - c.hit_drop);
        self.health = Some(me.health);
        if hit {
            self.hit_at = Some(now);
        }
        self.known = Known::of_enemies(sit.view, sit.team, (me.team, me.place), now, c.worth.told_ms);
        self.cover_rule = sit.cover;
        self.reach_m = c.worth.objective_in_band.then(|| {
            let own = if self.hands().pistol_out { Some(WeaponKind::Pistol) } else { sit.weapon };
            own.map(|kind| c.band(kind).1 + c.band_slack_m)
        }).flatten();
        self.threats(me, sit.view, memory, now, hit, c, rng);
        self.choose(me, Some(sit.view), memory, now, aim, skill, rng);
        self.glances(me, sit, memory, now, c);
        if self.target.is_some() || hit {
            self.duel_until = now + c.duel_ms;
        }
        self.trade_watch(sit, now, c);
        self.watch_enemies(sit.view, memory, now, c);
        // Dry (nothing to fire): no push at an enemy's reload.
        if !self.dry() {
            self.punish(me, sit.view, memory, now, c, rng);
        }
        if !memory.all_seen(now, sit.view.clock).is_empty() {
            self.last_seen = now;
        }
        self.finish(me, sit, memory, now, c);
        if hit {
            // Shot at: the side step turns the other way at once.
            if self.target.is_some() {
                self.step_aside(now, c, rng);
            }
            self.break_sight(me, sit, memory, now, true, true, c.hide_m, c, rng);
        }
        if let Some((wish, at)) = self.heal_walk.take() {
            self.heal_cover(wish, at, me, sit, memory, now, c, rng);
        }
        // The magazine out with a target seen: out of sight to reload, else
        // reload where it stands. (Out of ammo too, it takes the pistol
        // where it stands: [`Self::reload`].)
        let empty = self.hands().held().needs_reload();
        if empty && self.target.is_some() && !self.reload_here {
            self.break_sight(me, sit, memory, now, false, true, c.hide_m, c, rng);
            self.reload_here = !matches!(self.cover, Cover::Hiding { .. });
        }
        self.cover_steps(me, sit, now, c, rng);
        self.matchup(me, sit, memory, now, c);
        // Outranged: out of his sight, and wait (no re-peek); a carrier
        // keeps its way home.
        if self.outranged && sit.view.objective().is_actor(me) {
            self.outranged_stats.task_kept += 1;
        }
        if self.outranged
            && now >= self.next_outranged
            && !sit.view.objective().is_actor(me)
            && !matches!(self.cover, Cover::Hiding { .. })
        {
            self.next_outranged = now + c.outranged_again_ms;
            self.outranged_stats.hides += 1;
            self.break_sight(me, sit, memory, now, false, false, c.hide_m, c, rng);
        }
        let look = RollView {
            me,
            sit,
            memory,
            now,
        };
        self.grenade_roll(look, aim, skill, rng);
    }

    /// [aimpoint] Before each [`Self::tick`]: which parts of the target
    /// show from the bot's eyes on `world`'s bullet mesh, and his sight told
    /// to `memory` ([`Self::sight_tick`]). A reflex never told this (no
    /// map) aims at the chest and holds no shot for the world.
    #[cfg(test)]
    pub fn look(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        world: &std::sync::Arc<mapdata::World>,
        memory: &mut Memory,
        sense: &SenseTuning,
        now: u32,
    ) {
        let off = super::tuning::SmokeTuning {
            sight: false,
            ..Default::default()
        };
        self.look_through(me, view, world, memory, sense, &off, now);
    }

    /// [`Self::look`] through the match's smoke as the team's `smoke` tuning
    /// reads it ([`super::smoke::Clouds::veil`]): parts behind a cloud do not
    /// show, and a target just lost in one may be sprayed ([`super::aimpoint::Aim::Spray`],
    /// continuing fire only).
    #[allow(clippy::too_many_arguments)]
    pub fn look_through(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        world: &std::sync::Arc<mapdata::World>,
        memory: &mut Memory,
        sense: &SenseTuning,
        smoke: &super::tuning::SmokeTuning,
        now: u32,
    ) {
        self.new_life(me);
        memory.set_sight_target(self.target);
        let looked = self.sight.update(me, self.target, view, world, smoke, now);
        // One note when the aim's look first finds its target behind smoke.
        let smoked = looked.is_some_and(|look| look.fresh && look.look == PartsLook::Smoke);
        if smoked && !self.smoke_lost {
            if let Some(who) = self.target {
                self.smoke_notes.push(format!("lost player {who} to smoke (the aim's look)"));
            }
            #[cfg(test)]
            super::smoke::tally::add(me.team, super::smoke::tally::Event::AimLost);
        }
        if looked.is_some_and(|look| look.fresh) {
            self.smoke_lost = smoked;
        }
        if sense.sight_refresh {
            self.sight_tick(me, view, memory, sense, looked);
        }
    }

    /// The smoke notes of the last ticks (a target lost to smoke, a spray begun and ended), for the bot's log.
    pub fn take_smoke_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.smoke_notes)
    }

    /// A smoke note of the senses, kept with the reflex's until its think takes them.
    pub fn note_smoke(&mut self, text: String) {
        self.smoke_notes.push(text);
    }

    /// [smoke] The quick-use key that throws a smoke from the kit now ([`super::view::Tracker::smoke_key`]).
    pub fn set_smoke_key(&mut self, key: Option<u32>) {
        self.smoke_key = key;
    }

    /// [smoke] The sniper that outranges it now, seen: who, where, and when (the match clock).
    pub fn outranged_by(&self) -> Option<(Place, Vec3, u32)> {
        self.outranged_by
    }

    /// [smoke] Whether the kit has a smoke to throw now, and no throw is under way.
    pub fn can_smoke(&self) -> bool {
        self.smoke_key.is_some() && self.smoke_throw.is_none() && self.throw.is_none() && self.cloud_hold.is_none()
    }

    /// [A208 `smoke.wait_for_cloud`] Whether it holds for a cloud now.
    pub fn holds_for_cloud(&self) -> bool {
        self.cloud_hold.is_some()
    }

    /// [A208 `smoke.wait_for_cloud`] A hold for a mate's cloud from now (`smoke::WaitsFor::Mates`), if nothing has the
    /// legs: no throw, no hold, no target. Whether it was begun.
    pub fn hold_for_cloud(&mut self, wait: super::smoke::CloudWait, me: &PlaceView, now: u32) -> bool {
        if self.cloud_hold.is_some() || self.smoke_throw.is_some() || self.throw.is_some() || self.target.is_some() {
            return false;
        }
        // One hold a crossing: one ended there (a hit, an enemy close, the cap) is not begun again for the same cloud.
        if self
            .hold_ended
            .is_some_and(|(crossing, ended)| crossing.distance_xz(wait.line.crossing) <= HOLD_SAME_CROSSING_M && now < ended + wait.cap_ms)
        {
            return false;
        }
        self.begin_hold(wait, me, now);
        true
    }

    /// [A208] A hold begun, counted from `since`.
    fn begin_hold(&mut self, wait: super::smoke::CloudWait, me: &PlaceView, since: u32) {
        let legs = if me.feet.distance_xz(wait.hold) <= HOLD_AT_M { HoldLegs::Standing(me.feet) } else { HoldLegs::Walking };
        self.cloud_hold = Some(CloudHolding { wait, since, until: since.saturating_add(wait.cap_ms), legs });
        #[cfg(test)]
        super::smoke::tally::wait(me.team, wait.waits_for, super::smoke::tally::Wait::Begun, 1);
    }

    /// [smoke] A smoke thrown at `plan` from now, if the kit has one and the hands are free: no throw under way, no
    /// target, no reload or weapon switch. Whether it was begun.
    /// With `wait` (`smoke::CloudWait::of`, `smoke.wait_for_cloud`), after the launch it holds short of the plan's
    /// crossing until the cloud hides the line.
    pub fn start_smoke(&mut self, plan: super::smoke::SmokePlan, now: u32, wait: Option<super::smoke::CloudWait>) -> bool {
        let Some(key) = self.smoke_key.filter(|_| self.can_smoke()) else {
            return false;
        };
        // A smoke waits for no target, but the one against the sniper it is outranged by: the matchup that names him
        // (`outranged_by`) holds him as the target (h2h-sniper-vs-off-20261030T0040: 66 plans, none begun without this).
        let fighting = self.target.is_some() && plan.for_use != super::smoke::SmokeUse::AntiSniper;
        if fighting || now < self.busy_until {
            return false;
        }
        self.smoke_throw = Some(SmokeThrowing {
            plan,
            key,
            started: now,
            phase: SmokePhase::Turning,
            wait,
        });
        match plan.for_use {
            super::smoke::SmokeUse::CrossLane => self.lane_smokes += 1,
            super::smoke::SmokeUse::AntiSniper => self.sniper_smokes += 1,
            _ => {}
        }
        true
    }

    /// [smoke] The lane smokes begun this life.
    pub fn lane_smokes(&self) -> u32 {
        self.lane_smokes
    }

    /// [smoke] The sniper smokes begun this life.
    pub fn sniper_smokes(&self) -> u32 {
        self.sniper_smokes
    }

    /// [smoke] A smoke throw's tick ([`SmokeThrowing`]): the bot stands, turns to the plan's view, holds the key the
    /// frag's hold (`grenade_hold_ms`, its middle: no draw) once within `grenade_within_deg`, lets go, and keeps the
    /// look `grenade_launch_ms` + `grenade_look_after_ms` from the press. Given up after `grenade_give_up_ms` without
    /// a press. `None`: no smoke throw this tick.
    fn smoke_throwing(&mut self, me: &PlaceView, now: u32, c: &CombatTuning, settle_s: f32) -> Option<Override> {
        let mut t = self.smoke_throw?;
        let look = me.eye() + arc::view_forward(t.plan.aim.yaw, t.plan.aim.pitch) * 10.0;
        let hold_ms = (c.grenade_hold_ms.0 + c.grenade_hold_ms.1) / 2;
        let mut keys = 0;
        match t.phase {
            SmokePhase::Turning if off_by(me, look, (0.0, 0.0)) <= c.grenade_within_deg.to_radians() => {
                t.phase = SmokePhase::Held(now);
                keys = t.key;
            }
            // Picked up a battery before the press: the client ignores the quick key in a carrier's hands.
            SmokePhase::Turning if me.carrying.is_some() => {
                self.smoke_throw = None;
                self.smoke_notes.push(format!("gave up a smoke for {:?}: carrying", t.plan.for_use));
                #[cfg(test)]
                super::smoke::tally::thrown(me.team, t.plan.for_use, false);
                return None;
            }
            SmokePhase::Turning if now >= t.started + c.grenade_give_up_ms => {
                self.smoke_throw = None;
                self.smoke_notes.push(format!("gave up a smoke for {:?}: the view did not come round", t.plan.for_use));
                #[cfg(test)]
                super::smoke::tally::thrown(me.team, t.plan.for_use, false);
                return None;
            }
            SmokePhase::Turning => {}
            SmokePhase::Held(pressed) if now < pressed + hold_ms => keys = t.key,
            SmokePhase::Held(pressed) => {
                t.phase = SmokePhase::Released(pressed);
                let at = t.plan.at;
                self.smoke_notes.push(format!(
                    "throws a smoke for {:?} at ({:.1}, {:.1}, {:.1}), covering {:.1} m of its line",
                    t.plan.for_use, at.x, at.y, at.z, t.plan.covers_m
                ));
                #[cfg(test)]
                {
                    super::smoke::tally::thrown(me.team, t.plan.for_use, true);
                    super::smoke::tally::threw(me.place, now);
                    if let Some(w) = self.crossing_watch.take() {
                        super::smoke::tally::wait(me.team, super::smoke::WaitsFor::Own(w.for_use), super::smoke::tally::Wait::CrossedNever, 1);
                    }
                    self.crossing_watch =
                        Some(CrossingWatch { for_use: t.plan.for_use, line: t.plan.line, until: now + CROSSING_WATCH_MS });
                }
            }
            SmokePhase::Released(pressed) if now >= pressed + c.grenade_launch_ms + c.grenade_look_after_ms => {
                self.smoke_throw = None;
                // [A208] Its cloud first: the hold, counted from the press.
                if let Some(wait) = t.wait {
                    self.begin_hold(wait, me, pressed);
                }
                return None;
            }
            SmokePhase::Released(_) => {}
        }
        self.smoke_throw = Some(t);
        Some(Override {
            look_at: Some(look),
            turn_time_s: Some(settle_s * 0.5),
            keys,
            legs: Some(Legs {
                at: me.feet,
                within_m: 1.0,
                run: false,
                crouch: me.crouched(),
                why: LegsWhy::ThrowingSmoke,
            }),
            band_m: self.band,
            ..Override::default()
        })
    }

    /// The target's sight at this tick, told to the memory
    /// ([`Memory::looked`]). Range, view (at the yaw `me` has now) and life
    /// are checked at every tick; whether some part shows is the aim's
    /// look, every `LOOK_AGAIN_MS`, whose rays are cast anyway: so a
    /// visible target costs no ray more, and a look not due is untested,
    /// the sight keeping its state. No random draw, no team call-out
    /// (that is a think's).
    fn sight_tick(
        &self,
        me: &PlaceView,
        view: &MatchView,
        memory: &mut Memory,
        sense: &SenseTuning,
        looked: Option<super::aimpoint::TargetLook>,
    ) {
        let Some(target) = self.target else {
            return;
        };
        let Some(since) = memory
            .tracks()
            .iter()
            .find(|t| t.who == target)
            .and_then(|t| t.sight_since)
        else {
            return;
        };
        let outcome = match view.place(target) {
            None => LookOutcome::Failed(Miss::Dead),
            Some(enemy) => match super::sense::gate(me, enemy, since, sense) {
                Err(miss) => LookOutcome::Failed(miss),
                Ok(()) => match looked {
                    Some(look) if look.fresh => match look.look {
                        PartsLook::Shows => {
                            #[cfg(test)]
                            super::smoke::tally::record_seen(me, enemy, view.clock);
                            LookOutcome::Seen
                        }
                        PartsLook::Smoke => LookOutcome::Failed(Miss::Smoke),
                        PartsLook::Hidden => LookOutcome::Failed(Miss::Blocked),
                    },
                    _ => LookOutcome::Untested,
                },
            },
        };
        let at = view.place(target).map_or(Vec3::ZERO, |p| p.feet);
        memory.looked(target, at, outcome, view.clock);
    }

    /// [measure] The lapse of the last tick, if its target's sight ended.
    pub fn take_lapse(&mut self) -> Option<Lapse> {
        self.lapse.take()
    }

    /// [recoil] At each tick, before [`Self::tick`]: the weapon's update to
    /// `now` (the client's comes before the tick's turn and shots). Returns
    /// the kick as the last tick left it, pitch units: the view the bot
    /// aims from is its base pitch and that; this tick's rise is taken
    /// back by [`Override::pull`].
    pub fn weapon_tick(&mut self, me: &PlaceView, now: u32, scale: PitchScale) -> f32 {
        self.scale = scale;
        self.new_life(me);
        self.in_hand();
        let before = self.recoil.y();
        self.recoil.update(now);
        self.kick_step = self.recoil.y() - before;
        self.kick_before = before;
        before
    }

    /// [recoil] The kick the view had when the tick's decision was made (pitch units): the one
    /// [`Self::weapon_tick`] returned.
    pub fn kick_before(&self) -> f32 {
        self.kick_before
    }

    /// [recoil] The weapon's kick now, pitch units.
    pub fn kick(&self) -> f32 {
        self.recoil.y()
    }

    /// [recoil] The tick's input as sent (`pitch`: its pitch turn as the
    /// halves say it): returns the part of a downward turn the kick takes
    /// (the base pitch moves by `pitch` and that); then the tick's shots by
    /// its `keys`.
    pub fn sent(&mut self, now: u32, keys: u32, pitch: f32) -> f32 {
        self.in_hand();
        self.recoil.update(now);
        let taken = self.recoil.turn(pitch);
        let auto = self.hands().held().gun.auto;
        let interval_ms = self.recoil.kick().interval_s() * 1000.0;
        let gun = self.hands().held().gun;
        let cycle_ms = gun.shot_ms() + gun.chamber_ms as f32;
        // The client starts a shot from the idle state only (`fire_pred` 0x2E3CB0): not in the last shot's clip, not in a reload or a switch.
        let (gated, ready_at) = (self.gated, &mut self.ready_at);
        let can_fire = !gated || (now >= self.busy_until && *ready_at <= now as f32 + 1e-3);
        let recoil = &mut self.recoil;
        let fire = keys & key::FIRE != 0 && can_fire;
        #[cfg(test)]
        let mut shots = 0u8;
        #[cfg(test)]
        {
            self.seen.0 = recoil.y();
        }
        self.cadence.tick(now, fire, auto, interval_ms, |at| {
            if gated {
                if *ready_at > at + 1e-3 {
                    return;
                }
                *ready_at = at + cycle_ms;
            }
            recoil.mean_shot(at);
            #[cfg(test)]
            {
                shots = shots.saturating_add(1);
            }
        });
        #[cfg(test)]
        {
            self.seen.1 = shots;
        }
        taken
    }

    /// [recoil] Whether the mirror counts the shots the weapon can fire ([`CombatTuning::mirror_gated`]), for the next [`Self::sent`].
    pub fn gate_mirror(&mut self, on: bool) {
        self.gated = on;
    }

    /// [bench] The mirror's kick after the last tick's turn, before its shots, and the shots it counted ([`Self::sent`]).
    #[cfg(test)]
    pub(crate) fn seen(&self) -> (f32, u8) {
        self.seen
    }

    /// The kick of the weapon in hand: another weapon taken (the pistol)
    /// starts with none (0x2E2C21). The model is the held item's, so a weapon
    /// with no kick of its own keeps its stand-in kick between ticks (A224:
    /// rebuilt at every tick, it never pulled against the kick).
    fn in_hand(&mut self) {
        let item = self.hands().held().gun.item;
        if self.recoil.held() != item {
            self.recoil = Recoil::in_hand(item);
            self.cadence = Cadence::default();
            self.ready_at = 0.0;
        }
    }

    /// A new life: the per-life counts start again; what it carries stays,
    /// and the rounds its kit has left.
    fn new_life(&mut self, me: &PlaceView) {
        if self.life != Some(me.spawned) {
            // [smoke] A smoke throw under way ends with the life: counted, so every throw begun has its exit.
            #[cfg(test)]
            if let Some(t) = self.smoke_throw {
                super::smoke::tally::died(me.team, t.plan.for_use);
            }
            #[cfg(test)]
            if let Some(h) = self.cloud_hold {
                super::smoke::tally::wait(me.team, h.wait.waits_for, super::smoke::tally::Wait::Died, 1);
            }
            #[cfg(test)]
            if let Some(w) = self.crossing_watch {
                super::smoke::tally::wait(me.team, super::smoke::WaitsFor::Own(w.for_use), super::smoke::tally::Wait::CrossedNever, 1);
            }
            if self.life.is_some() {
                *self = Self::armed_carrying(self.arms, self.left());
            }
            self.life = Some(me.spawned);
        }
    }

    /// [A208 `smoke.wait_for_cloud`] A hold for a cloud: walking its route on (no override: the drive walks it) to the
    /// hold point, then standing there looking at the threat's eye, until the clouds hide the line, a target that is
    /// not the threat, a hit since the hold began, an enemy its own senses put within `close_m` lately, or the cap.
    /// `None`: no hold, or walking to it.
    fn cloud_holding(&mut self, me: &PlaceView, view: &MatchView, memory: &Memory, now: u32, settle_s: f32) -> Option<Override> {
        use super::smoke::HoldEnd;
        let mut h = self.cloud_hold?;
        let w = h.wait;
        let hit = |at: Option<u32>| at.is_some_and(|at| at >= h.since);
        let close = || {
            memory.tracks().iter().any(|track| {
                track.own.is_some_and(|(at, time)| now.saturating_sub(time) <= w.close_sensed_ms && at.distance_xz(me.feet) <= w.close_m)
            })
        };
        let end = if w.hidden(&view.smoke, now) {
            Some(HoldEnd::Hidden)
        } else if self.target.is_some_and(|target| Some(target) != w.threat) {
            Some(HoldEnd::Fight)
        } else if hit(self.hit_at) || hit(self.bullet_hit_at) {
            Some(HoldEnd::Hit)
        } else if close() {
            Some(HoldEnd::Close)
        } else if now >= h.until {
            Some(HoldEnd::Cap)
        } else {
            None
        };
        if let Some(why) = end {
            self.cloud_hold = None;
            self.hold_ended = Some((w.line.crossing, now));
            self.smoke_notes.push(format!("walks on after holding for the cloud ({:?}): {why:?}", w.waits_for));
            #[cfg(test)]
            {
                use super::smoke::tally::{self, Wait};
                tally::wait(me.team, w.waits_for, Wait::ended(why), 1);
                tally::wait(me.team, w.waits_for, Wait::HeldMs, u64::from(now.saturating_sub(h.since)));
            }
            return None;
        }
        // At the hold point, or nearer the crossing than it is (the route bent round it): it stands there.
        if h.legs == HoldLegs::Walking
            && (me.feet.distance_xz(w.hold) <= HOLD_AT_M || me.feet.distance_xz(w.line.crossing) <= w.hold.distance_xz(w.line.crossing))
        {
            h.legs = HoldLegs::Standing(me.feet);
        }
        self.cloud_hold = Some(h);
        let HoldLegs::Standing(at) = h.legs else {
            return None;
        };
        Some(Override {
            look_at: Some(w.line.eye),
            turn_time_s: Some(settle_s),
            legs: Some(Legs { at, within_m: 1.0, run: false, crouch: me.crouched(), why: LegsWhy::WaitingForCloud }),
            band_m: self.band,
            ..Override::default()
        })
    }

    /// [A208, the bench's] The last thrown smoke's crossing: reached (within [`CROSSING_AT_M`]) with its line hidden from
    /// the client's eyes (`smoke::Forming::Client`, the default thinness) or open, or not reached in time.
    #[cfg(test)]
    fn watch_crossing(&mut self, me: &PlaceView, view: &MatchView, now: u32) {
        use super::smoke::tally::Wait;
        let Some(w) = self.crossing_watch else {
            return;
        };
        let what = if me.feet.distance_xz(w.line.crossing) <= CROSSING_AT_M {
            let veil = super::smoke::Veil::Smoke {
                clouds: view.smoke.all(),
                at: super::smoke::MatchMs(now),
                thin_m: super::tuning::SmokeTuning::default().thin_m.get(),
                forming: super::smoke::Forming::Client,
            };
            if veil.hides(w.line.eye, w.line.target) { Wait::CrossedHidden } else { Wait::CrossedOpen }
        } else if now >= w.until {
            Wait::CrossedNever
        } else {
            return;
        };
        self.crossing_watch = None;
        super::smoke::tally::wait(me.team, super::smoke::WaitsFor::Own(w.for_use), what, 1);
    }

    /// The target among the seen enemies: the present one if still seen,
    /// else the nearest in range, a wounded one counting nearer (with the
    /// view); or a sound to turn to.
    #[allow(clippy::too_many_arguments)]
    fn choose(
        &mut self,
        me: &PlaceView,
        view: Option<&MatchView>,
        memory: &Memory,
        now: u32,
        aim: &AimTuning,
        skill: f32,
        rng: &mut Rng,
    ) {
        let c = &aim.combat;
        // Hit a moment ago: the fight is with whoever is hitting the bot, and the worth's own terms steer nothing in it (an
        // objective actor neither frees the target nor outscores the others: `worth.hurter_keep_ms`, 0: the tip's one think).
        // The wounded enemy's bonus is the tip's and stays.
        let hit_lately = c.worth.hurter_keep_ms > 0
            && self
                .hit_at
                .into_iter()
                .chain(self.bullet_hit_at)
                .any(|hit| now < hit.saturating_add(c.worth.hurter_keep_ms));
        let in_a_fight = WorthTuning { objective_actor_m: 0.0, objective_threat_m: 0.0, switch_m: 0.0, ..c.worth.clone() };
        let worth = view.map(|view| Worth {
            view,
            known: &self.known,
            tuning: if hit_lately { &in_a_fight } else { &c.worth },
            hurt_m: c.wounded_bonus_m,
            reach: self.reach_m.map(|reach| (me.feet, reach)),
        });
        let sight_now = view.map_or(now, |v| v.clock);
        let mut seen = memory.all_seen(now, sight_now);
        // A target taken back before he is noticed again stays one.
        if let Some(track) = self
            .target
            .and_then(|target| self.seen_target(memory, target, now, sight_now))
            .filter(|t| !seen.iter().any(|s| s.who() == t.who()))
        {
            seen.push(track);
        }
        let in_range = |t: &&SeenTrack| t.at().distance_xz(me.feet) <= aim.range_m;
        let d = |t: &SeenTrack| t.at().distance_xz(me.feet);
        // The present target, unless another seen enemy presses more
        // ([`Self::presses_more`]).
        let held_ms = now.saturating_sub(self.since);
        let keep = self
            .target
            .and_then(|target| seen.iter().filter(in_range).find(|t| t.who() == target))
            .filter(|kept| {
                !seen.iter().filter(in_range).any(|t| {
                    t.who() != kept.who()
                        && presses_more(me, view, worth.as_ref(), kept, t, held_ms, c)
                })
            });
        // A threat's shooter seen: him before any other.
        // A marker's direction: a seen enemy near that direction, the
        // nearest first.
        let shooter = self.threat.filter(|t| now < t.until).and_then(|threat| {
            if let Some(toward) = threat.bearing {
                let off = |t: &SeenTrack| wrap(bearing(me.feet, t.at()) - toward).abs();
                return seen
                    .iter()
                    .filter(in_range)
                    .filter(|t| off(t) <= c.hit_match_deg.to_radians())
                    .min_by(|a, b| d(a).total_cmp(&d(b)));
            }
            let off = |t: &SeenTrack| t.at().distance_xz(threat.at);
            seen.iter()
                .filter(in_range)
                .filter(|t| off(t) <= c.threat_match_m + threat.error_m)
                .min_by(|a, b| off(a).total_cmp(&off(b)))
        });
        if shooter.is_some() {
            self.threat = None;
        }
        // Nearer counts, and so does what he is worth (a carrier, a thief of the team's
        // battery, a hurt enemy: [`Worth::of`]).
        let score = |t: &SeenTrack| {
            t.at().distance_xz(me.feet)
                - worth.as_ref().map_or(0.0, |w| w.of(t.who()) - w.gap_of(t.who()))
        };
        // The enemy who hit it stays its target a while: the worth neither frees him nor outscores him.
        if let Some(shooter) = shooter
            && c.worth.hurter_keep_ms > 0
        {
            self.hurter = Some((shooter.who(), now + c.worth.hurter_keep_ms));
        }
        let hurter = self
            .hurter
            .filter(|&(_, until)| now < until)
            .and_then(|(who, _)| seen.iter().filter(in_range).find(|t| t.who() == who));
        let choice = shooter.or(hurter).or(keep).or_else(|| {
            seen.iter()
                .filter(in_range)
                .min_by(|a, b| score(a).total_cmp(&score(b)))
        });
        match choice {
            Some(track)
                if Some(track.who()) != self.target
                    && c.reacquire_keep
                    && self.target.is_none()
                    && self
                        .lost
                        .is_some_and(|(who, at)| who == track.who() && now <= at + c.aim_carry_ms) =>
            {
                // Taken again a moment after he was lost: as it had him.
                self.retake(track.who());
                self.retaken = false;
            }
            Some(track) if Some(track.who()) != self.target => {
                // Taken for what he is worth, not for being the nearest: counted and noted once.
                let nearest = seen
                    .iter()
                    .filter(in_range)
                    .min_by(|a, b| d(a).total_cmp(&d(b)));
                if let (Some(w), Some(near), None) = (worth.as_ref(), nearest, shooter)
                    && near.who() != track.who()
                    && let Some(why) = w.why(track.who())
                {
                    self.worth_stats.count(why, self.target.is_some());
                    self.worth_notes.push(format!(
                        "targets {} ({}) over the nearer {}: worth {:.0} m, {:.0} m away against {:.0} m",
                        track.who(),
                        why.name(),
                        near.who(),
                        w.of(track.who()),
                        d(track),
                        d(near),
                    ));
                }
                if let Some(w) = worth.as_ref()
                    && seen.iter().filter(in_range).any(|t| w.out_of_reach(t.who()))
                {
                    self.worth_stats.out_of_reach += 1;
                }
                self.retaken = false;
                self.reacquired = self
                    .lost
                    .is_some_and(|(who, at)| who == track.who() && now <= at + LOST_MS);
                self.target = Some(track.who());
                self.since = now;
                // A fresh target: the aim starts well off and settles.
                let error = mix(aim.error_deg, skill).to_radians() * 3.0;
                self.error = (rng.range(-error, error), rng.range(-error, error) * 0.5);
                // The first shot a reaction after noticing.
                let t = track.track();
                let noticed = t.sight_since.unwrap_or(now) + t.notice_ms;
                let reaction = mix(aim.reaction_ms, skill).max(0.0) * rng.range(0.85, 1.15);
                self.first_shot_at = noticed.max(self.since.min(noticed)) + reaction as u32;
                self.sound = None;
                let c = &aim.combat;
                self.ads_near = rng.unit() < c.ads_near_chance;
                let gun = self.hands().held().gun;
                self.crouch_fire = rng.unit() < c.fire.crouch_chance(gun, c.sniper_crouch_chance);
                // (No draw for a sure one: the bots' random sequence stays
                // as it was for an automatic weapon.)
                let strafe = c.fire.strafe_chance(gun);
                self.strafe_fire = strafe >= 1.0 || rng.unit() < strafe;
                // Play 19: a new enemy is spotted once in the crosshair.
                let lately = self
                    .spotted
                    .iter()
                    .any(|(who, at)| *who == track.who() && *at + c.spot_again_ms > now);
                self.spot_pending = (!lately && rng.unit() < c.spot_chance).then_some(track.who());
            }
            Some(_) => {}
            None => {
                self.lose_target(now);
                // Play 6: a shot heard lately, nothing seen: turn to it.
                // A shot (`player_bots_behaviour.md` §3 play 6), not a
                // step or a reload: one of the sounds of him it heard was
                // a shot (with no view, any sound).
                let fired = |who: Place| {
                    view.is_none_or(|view| {
                        view.sounds.iter().any(|sound| {
                            sound.from == who
                                && sound.kind == SoundKind::Shot
                                && sound.time + 1_500 >= now
                        })
                    })
                };
                let heard = memory
                    .tracks()
                    .iter()
                    .filter(|t| t.source == Source::Heard && t.time + 1_500 >= now && fired(t.who))
                    .min_by(|a, b| {
                        a.at.distance_xz(me.feet)
                            .total_cmp(&b.at.distance_xz(me.feet))
                    });
                // A look of `sound_ms`, again after `sound_again_ms`: heard
                // tracks now last (no callout wipes them), and a bot
                // turning at every think to every shot stopped doing
                // anything else.
                if let Some(track) = heard
                    && now >= self.next_sound
                {
                    self.sound = Some((track.at, now + c.sound_ms));
                    self.next_sound = now + c.sound_ms + c.sound_again_ms;
                }
            }
        }
    }

    /// The threat to turn to at this think (`CombatTuning::threat_*`): hit,
    /// the nearest enemy it sensed firing lately unless that is the target
    /// it sees; not hit, a shot heard near from outside its view and nearer
    /// than its target. By its own senses only: a shot sound of an enemy
    /// it sees now, or whose place it heard (a heard track as new as the
    /// shot window).
    #[allow(clippy::too_many_arguments)]
    fn threats(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        memory: &Memory,
        now: u32,
        hit: bool,
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        if self.threat.is_some_and(|t| now >= t.until) {
            self.threat = None;
        }
        if !hit && (self.threat.is_some() || now < self.next_threat) {
            return;
        }
        // (who, where, the place's error, seen now)
        let mut shooters: Vec<(Place, Vec3, f32, bool)> = Vec::new();
        for sound in &view.sounds {
            if sound.kind != SoundKind::Shot
                || sound.time + c.threat_shot_ms < now
                || shooters.iter().any(|s| s.0 == sound.from)
            {
                continue;
            }
            let known = match memory.seen(sound.from, now, view.clock) {
                Some(seen) => Some((seen.at(), 0.0, true)),
                None => memory
                    .tracks()
                    .iter()
                    .find(|t| {
                        t.who == sound.from
                            && t.source == Source::Heard
                            && t.time + c.threat_shot_ms >= now
                    })
                    .map(|t| (t.at, t.error_m, false)),
            };
            if let Some((at, error_m, seen)) = known {
                shooters.push((sound.from, at, error_m, seen));
            }
        }
        let d = |at: Vec3| at.distance_xz(me.feet);
        let target_at = self
            .target
            .and_then(|target| memory.seen(target, now, view.clock))
            .map(|t| t.at());
        let pick = if hit {
            shooters
                .iter()
                .min_by(|a, b| d(a.1).total_cmp(&d(b.1)))
                .filter(|s| !(Some(s.0) == self.target && s.3))
        } else {
            shooters
                .iter()
                .filter(|s| {
                    !s.3 && d(s.1) <= c.threat_heard_m
                        && !super::sense::in_view(me.feet, me.yaw, s.1, c.threat_off_deg * 2.0)
                        && target_at.is_none_or(|at| d(s.1) < d(at))
                })
                .min_by(|a, b| d(a.1).total_cmp(&d(b.1)))
        };
        if let Some(&(_, at, error_m, _)) = pick {
            if hit {
                self.last_hit = Some(super::concede::Hit { shooter_at: at, from: me.feet, at: now });
                super::concede::tally(me.team, super::concede::Tally::LoneHit);
            }
            self.threat = Some(Threat {
                at,
                error_m,
                until: now + c.threat_ms,
                hit,
                bearing: None,
            });
            self.next_threat = now + c.threat_ms + c.threat_again_ms;
            return;
        }
        // Hit, and no shooter sensed at all: the HUD's hit marker gives
        // the direction alone.
        if hit {
            if shooters.is_empty()
                && c.hit_bearing
                && (c.hit_bearing_with_target || target_at.is_none())
                && let Some(toward) = hit_bearing(me, view, now, c, rng)
            {
                self.threat = Some(Threat {
                    at: along(me.feet, toward, c.hit_look_m),
                    error_m: 0.0,
                    until: now + c.threat_ms,
                    hit: true,
                    bearing: Some(toward),
                });
                self.next_threat = now + c.threat_ms + c.threat_again_ms;
                self.hit_note = Some((me.feet, toward));
            }
            return;
        }
        // Not hit: a step or a reload heard near from outside the view.
        let heard = memory
            .tracks()
            .iter()
            .filter(|t| {
                t.source == Source::Heard
                    && t.time + c.threat_shot_ms >= now
                    && d(t.at) <= c.glance_heard_m
                    && !super::sense::in_view(me.feet, me.yaw, t.at, c.threat_off_deg * 2.0)
                    && (d(t.at) <= c.glance_close_m
                        || target_at.is_none_or(|at| d(t.at) < d(at) * c.glance_heard_share))
            })
            .min_by(|a, b| d(a.at).total_cmp(&d(b.at)));
        if let Some(track) = heard {
            self.threat = Some(Threat {
                at: track.at,
                error_m: track.error_m,
                until: now + c.glance_ms,
                hit: false,
                bearing: None,
            });
            self.next_threat = now + c.glance_ms + c.threat_again_ms;
            return;
        }
        self.flank_glance(me, memory, now, target_at, c);
    }

    /// How long the trigger stays up from `now`, ms: between bursts the rest of the pause (none in a burst), for a weapon
    /// pressed a round at a time the time to the next press.
    fn trigger_free_ms(&mut self, now: u32) -> u32 {
        let handling = self.hands().held().gun.handling;
        match handling {
            Handling::Burst(_) if now < self.burst_until => 0,
            Handling::Burst(_) => self.pause_until.saturating_sub(now),
            _ => self.next_press.saturating_sub(now),
        }
    }

    /// R4: a target held, and a teammate's callout says an enemy is near on a side the view does not
    /// cover: a moment's look there. A player holding a man in his sights hears his mate shout; the tip
    /// looked at a callout only with no target (and a step heard has its own look above, with its own
    /// distances, which this leaves as they are). Not in a close duel
    /// (`flank_glance_min_m`) nor within a second and a half of a hit (the hit's own look is the answer).
    fn flank_glance(
        &mut self,
        me: &PlaceView,
        memory: &Memory,
        now: u32,
        target_at: Option<Vec3>,
        c: &CombatTuning,
    ) {
        let w = &c.worth;
        let d = |at: Vec3| at.distance_xz(me.feet);
        let Some(target_at) = target_at.filter(|_| w.flank_glance_m > 0.0) else {
            return;
        };
        if now < self.next_flank_glance || d(target_at) <= w.flank_glance_min_m || self.hit_at.is_some_and(|hit| now < hit + 1_500) {
            return;
        }
        let flank = memory
            .tracks()
            .iter()
            .filter(|t| {
                Some(t.who) != self.target
                    && matches!(t.source, Source::Told { .. })
                    && t.time + c.glance_told_fresh_ms >= now
                    && d(t.at) <= w.flank_glance_m
                    && !super::sense::in_view(me.feet, me.yaw, t.at, c.threat_off_deg * 2.0)
            })
            .min_by(|a, b| d(a.at).total_cmp(&d(b.at)));
        // The look lasts the pause of the trigger, so it costs no shot.
        let free_ms = self.trigger_free_ms(now);
        if w.flank_glance_min_pause_ms > 0 && free_ms < w.flank_glance_min_pause_ms {
            return;
        }
        let look_ms = if w.flank_glance_min_pause_ms > 0 { w.flank_glance_ms.min(free_ms) } else { w.flank_glance_ms };
        if let Some(track) = flank {
            super::fairness::read(me.team, super::fairness::Site::Glance, track, now);
            self.worth_stats.glances += 1;
            self.worth_notes.push(format!(
                "glances at the callout of {} at ({:.0}, {:.0}), {:.0} m away, holding a target {:.0} m away",
                track.who,
                track.at.x,
                track.at.z,
                d(track.at),
                d(target_at),
            ));
            self.threat = Some(Threat {
                at: track.at,
                error_m: track.error_m,
                until: now + look_ms,
                hit: false,
                bearing: None,
            });
            self.next_flank_glance = now + look_ms + w.flank_glance_again_ms;
        }
    }

    /// No target: a glance at a teammate's callout near (a told track, or
    /// a "shot from there" along its direction), outside the view.
    fn glances(
        &mut self,
        me: &PlaceView,
        sit: &Situation,
        memory: &Memory,
        now: u32,
        c: &CombatTuning,
    ) {
        if self.glance.is_some_and(|(_, until)| now >= until) {
            self.glance = None;
        }
        if self.target.is_some()
            || self.glance.is_some()
            || self.threat.is_some_and(|t| now < t.until)
            || self.sound.is_some_and(|(_, until)| now < until)
            || now < self.next_told
        {
            return;
        }
        let d = |at: Vec3| at.distance_xz(me.feet);
        let outside =
            |at: Vec3| !super::sense::in_view(me.feet, me.yaw, at, c.threat_off_deg * 2.0);
        // A teammate hit from a direction near by: along it, once.
        let weighed = self.told_until;
        let shot_from = sit
            .team
            .into_iter()
            .flat_map(|team| team.shots_from.iter())
            .filter(|s| {
                s.by != me.place
                    && s.time <= now
                    && s.time > weighed
                    && s.time + c.glance_told_fresh_ms >= now
                    && d(s.at) <= c.hit_told_m
            })
            .max_by_key(|s| s.time);
        if let Some(shot) = shot_from {
            self.told_until = shot.time;
            let at = along(shot.at, shot.bearing, c.hit_look_m);
            if outside(at) {
                self.glance = Some((at, now + c.glance_ms));
                self.next_told = now + c.glance_told_every_ms;
                return;
            }
        }
        let told = memory
            .tracks()
            .iter()
            .filter(|t| {
                matches!(t.source, Source::Told { .. })
                    && t.time + c.glance_told_fresh_ms >= now
                    && d(t.at) <= c.glance_told_m
                    && outside(t.at)
            })
            .min_by(|a, b| d(a.at).total_cmp(&d(b.at)));
        if let Some(track) = told {
            self.glance = Some((track.at, now + c.glance_ms));
            self.next_told = now + c.glance_told_every_ms;
        }
    }

    /// [smoke] A glance at `at` (a cloud's edge, on its floor) until `until`, if nothing else holds the look: no
    /// target nor one lost within [`LOST_MS`] (the pre-aim at where he went), no glance, trade watch, threat or sound
    /// under way. Whether it was taken.
    pub fn watch_smoke(&mut self, at: Vec3, now: u32, until: u32) -> bool {
        let busy = self.target.is_some()
            || self.lost.is_some_and(|(_, lost)| now <= lost + LOST_MS)
            || self.glance.is_some_and(|(_, end)| now < end)
            || self.trade.is_some_and(|(_, end)| now < end)
            || self.threat.is_some_and(|t| now < t.until)
            || self.sound.is_some_and(|(_, end)| now < end);
        if !busy {
            self.glance = Some((at, until));
        }
        !busy
    }

    /// Play 21: the partner died at a spot lately: watch it.
    fn trade_watch(&mut self, sit: &Situation, now: u32, c: &CombatTuning) {
        if self.trade.is_some_and(|(_, until)| now >= until) {
            self.trade = None;
        }
        let (Some(team), Some(partner)) = (sit.team, sit.partner) else {
            return;
        };
        if let Some(spot) = team
            .danger_within(now, c.trade_ms)
            .filter(|spot| spot.victim == partner)
            .max_by_key(|spot| spot.time)
        {
            self.trade = Some((spot.at, spot.time + c.trade_ms));
        }
    }

    /// What the senses showed of the enemies' habits: who holds an angle
    /// (seen crouched, aiming, still) and who stays at one place.
    fn watch_enemies(&mut self, view: &MatchView, memory: &Memory, now: u32, c: &CombatTuning) {
        for track in memory.all_seen(now, view.clock) {
            let Some(body) = view.place(track.who()) else {
                continue;
            };
            if body.crouched() && body.keys & key::AIM != 0 && body.speed() <= c.held_speed {
                self.held.retain(|(who, _, _)| *who != track.who());
                self.held.push((track.who(), track.at(), now));
            }
        }
        self.held.retain(|(_, _, at)| *at + c.held_memory_ms >= now);
        for track in memory.tracks() {
            if !matches!(track.source, Source::Seen | Source::Heard) || track.time + 500 < now {
                continue;
            }
            match self.camps.iter_mut().find(|camp| camp.who == track.who) {
                Some(camp) if camp.at.distance_xz(track.at) <= c.camp_m + track.error_m => {
                    camp.last = camp.last.max(track.time);
                }
                Some(camp) => {
                    *camp = Camp {
                        who: track.who,
                        at: track.at,
                        since: track.time,
                        last: track.time,
                    };
                }
                None => self.camps.push(Camp {
                    who: track.who,
                    at: track.at,
                    since: track.time,
                    last: track.time,
                }),
            }
        }
        self.camps
            .retain(|camp| camp.last + c.held_memory_ms >= now);
    }

    /// Play 5: a reload or switch sensed in a duel: at him.
    fn punish(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        memory: &Memory,
        now: u32,
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        let weighed = self.punished_until;
        for sound in &view.sounds {
            if !matches!(sound.kind, SoundKind::Reload | SoundKind::Switch) || sound.time <= weighed
            {
                continue;
            }
            self.punished_until = self.punished_until.max(sound.time);
            if sound.time + c.punish_window_ms < now || now >= self.duel_until {
                continue;
            }
            let Some(enemy) = view.place(sound.from) else {
                continue;
            };
            if enemy.team == me.team || !enemy.alive {
                continue;
            }
            let Some(track) = memory.tracks().iter().find(|t| t.who == sound.from) else {
                continue;
            };
            let d = sound.at.distance_xz(me.feet);
            let seen = memory.seen(sound.from, now, view.clock).is_some() && d <= c.punish_m;
            let heard = track.time + 300 >= sound.time && d <= c.punish_heard_m;
            if (seen || heard) && rng.unit() < c.punish_chance {
                self.push = Some(Push {
                    toward: track.at,
                    from: now,
                    until: now + c.punish_ms,
                    why: LegsWhy::Punish,
                });
                // Out of cover at once: the window is now.
                self.cover = Cover::Open;
            }
        }
    }

    /// R2: a hurt enemy this bot fired at has broken sight within `push_m`: it goes on his last place
    /// while his pain cannot have mended (`push_ms` from the hit, the pain refills from 8 s), one bot
    /// of the team or two (`push_bots`, the team's claim), the others hold their angles. By what the team
    /// saw land ([`super::worth::Wounds`]), never the enemy's pools. Not a carrier, not a bot that sees
    /// an enemy, not one hiding or already pushing; a seen enemy ends it (the fight is the reflex's).
    fn finish(&mut self, me: &PlaceView, sit: &Situation, memory: &Memory, now: u32, c: &CombatTuning) {
        let w = &c.worth;
        // The push is the legs' state ([`LegsWhy::Finish`]): it ends with its time, or with a target.
        if let Some((victim, until)) = self.pushing()
            && (now >= until || self.target.is_some())
        {
            if now >= until {
                self.worth_stats.push_expired += 1;
            } else if self.target == Some(victim) {
                self.worth_stats.push_seen_again += 1;
            } else {
                self.worth_stats.push_other_target += 1;
            }
            self.push = None;
        }
        if w.push_m <= 0.0
            || self.push.is_some_and(|p| now < p.until)
            || self.target.is_some()
            || !matches!(self.cover, Cover::Open)
            || super::objective_actor::is_actor(me)
            || self.dry()
        {
            return;
        }
        let (Some(team), Some((victim, fired))) = (sit.team, self.last_fired) else {
            return;
        };
        let Some(enemy) = sit.view.place(victim).filter(|p| p.alive && p.team != me.team) else {
            return;
        };
        let Some(last_hit) = team.wounds.last_hit(victim, enemy.spawned, now, me.place, w.told_ms) else {
            return;
        };
        if now >= last_hit.saturating_add(w.push_ms)
            || fired.abs_diff(last_hit) > w.credit_ms
            || team.wounds.hurt(victim, enemy.spawned, now, me.place, w.told_ms) < w.push_hurt
            || !team.wounds.may_push(victim, me.place, now, w.push_bots)
            || !memory.all_seen(now, sit.view.clock).is_empty()
        {
            return;
        }
        // By the team's own senses (seen, heard, told; never a cheat's place) within the fairness bound.
        let Some(track) = memory.tracks().iter().find(|t| {
            t.who == victim
                && t.source.own()
                && now.saturating_sub(t.time) <= super::worth::SENSED_WITHIN_MS
        }) else {
            return;
        };
        super::fairness::read(me.team, super::fairness::Site::Push, track, now);
        let at = track.own.map_or(track.at, |(at, _)| at);
        if at.distance_xz(me.feet) > w.push_m {
            return;
        }
        let until = last_hit + w.push_ms;
        self.push = Some(Push {
            toward: at,
            from: now,
            until,
            why: LegsWhy::Finish(victim),
        });
        self.worth_stats.pushes += 1;
        self.worth_notes.push(format!("pushes on hurt enemy {victim}"));
    }

    /// Play 7: hit in a duel: to a spot near by hidden from the enemy
    /// (`hit`: from the hit's threat, if it has one), then back at another
    /// angle if `repeek`. Hit from a marker's direction alone: to a spot
    /// with cover that way, and only the wait.
    #[allow(clippy::too_many_arguments)]
    fn break_sight(
        &mut self,
        me: &PlaceView,
        sit: &Situation,
        memory: &Memory,
        now: u32,
        hit: bool,
        repeek: bool,
        hide_m: (f32, f32),
        c: &CombatTuning,
        rng: &mut Rng,
    ) -> Sought {
        if matches!(self.cover, Cover::Hiding { .. }) {
            return Sought::Hiding;
        }
        if self.push.is_some_and(|p| now < p.until) {
            return Sought::Pushing;
        }
        if now >= self.duel_until {
            return Sought::NotDuel;
        }
        let Some(tactics) = sit.tactics else {
            return Sought::NoTactics;
        };
        let still_wait =
            |rng: &mut Rng| rng.range(c.still_wait_ms.0 as f32, c.still_wait_ms.1 as f32) as u32;
        if let Some(threat) = self.threat.filter(|t| hit && t.hit && now < t.until)
            && let Some(toward) = threat.bearing
        {
            // A carrier keeps its way home.
            if sit.view.objective().is_actor(me) || !c.hit_cover {
                return Sought::Declined;
            }
            let spread = c.hit_bearing_error_deg.to_radians();
            let covered = |at: Vec3| {
                let eye = still_eye(at, false);
                [-spread, 0.0, spread].iter().all(|k| {
                    let [fx, fz] = forward(toward + k);
                    let dir = Vec3::new(fx, 0.0, fz);
                    // Cover is what stops the shooter's bullet on that bearing: the bullet from `hit_cover_m` out along it
                    // does not keep the share over the rule's by the time it reaches the eye.
                    match sit.cover.filter(|_| c.hit_cover_reach) {
                        Some(rule) => !tactics.shot_reaches(Some(rule), eye + dir * c.hit_cover_m, eye),
                        None => super::rays::ray_cast(&tactics.world.sight, eye, dir, c.hit_cover_m).is_some(),
                    }
                })
            };
            let query = SpotQuery {
                side: sit.side,
                samples: 48,
                ..SpotQuery::ring(me.feet, hide_m.0, hide_m.1)
            };
            // Every floor sampled, nearest first (no rays in `spots`
            // without a sight test: the cover rays are these).
            let spot = tactics
                .spots(&query, now, query.samples, rng)
                .into_iter()
                .find(|spot| covered(spot.at));
            let Some(spot) = spot else {
                return Sought::NoSpot;
            };
            self.cover = Cover::Hiding {
                at: spot.at,
                enemy: threat.at,
                started: now,
                arrived: None,
                wait_ms: still_wait(rng),
                repeek: false,
            };
            return Sought::Began;
        }
        // The enemy it fights: hit, the threat that shot it (not the
        // target it sees); else the target, else the last one seen.
        let threat = self
            .threat
            .filter(|t| hit && t.hit && now < t.until)
            .map(|t| t.at);
        let enemy = threat
            .or_else(|| {
                self.target
                    .and_then(|target| memory.seen(target, now, sit.view.clock))
                    .map(|t| t.at())
            })
            .or_else(|| {
                memory
                    .tracks()
                    .iter()
                    .filter(|t| t.source == Source::Seen)
                    .max_by_key(|t| t.time)
                    .map(|t| t.at)
            });
        let Some(enemy) = enemy else {
            return Sought::NoEnemy;
        };
        let query = SpotQuery {
            out_of_fire_of: Some(enemy),
            cover: sit.cover,
            side: sit.side,
            samples: 48,
            ..SpotQuery::ring(me.feet, hide_m.0, hide_m.1)
        };
        if let Some(spot) = tactics.spots(&query, now, 1, rng).first() {
            self.last_peek = Some(me.feet);
            let wait_ms = if repeek {
                rng.range(c.hide_wait_ms.0 as f32, c.hide_wait_ms.1 as f32) as u32
            } else {
                still_wait(rng)
            };
            self.cover = Cover::Hiding {
                at: spot.at,
                enemy,
                started: now,
                arrived: None,
                wait_ms,
                repeek,
            };
            return Sought::Began;
        }
        Sought::NoSpot
    }

    /// Play 7 on: there, waited, then back at another angle.
    fn cover_steps(
        &mut self,
        me: &PlaceView,
        sit: &Situation,
        now: u32,
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        match self.cover {
            Cover::Open => {}
            Cover::Hiding {
                at,
                enemy,
                started,
                arrived,
                wait_ms,
                repeek,
            } => match arrived {
                None if me.feet.distance_xz(at) <= 1.2 => {
                    self.cover = Cover::Hiding {
                        at,
                        enemy,
                        started,
                        arrived: Some(now),
                        wait_ms,
                        repeek,
                    };
                }
                None if now >= started + c.hide_give_up_ms => self.cover = Cover::Open,
                None => {}
                Some(arrived) if now >= arrived + wait_ms => {
                    self.cover = Cover::Open;
                    // Dry: nothing to peek for; only the wait asked.
                    if self.dry() || !repeek {
                        return;
                    }
                    let Some(tactics) = sit.tactics else {
                        return;
                    };
                    let query = SpotQuery {
                        sight_to: Some(enemy),
                        side: sit.side,
                        samples: 48,
                        ..SpotQuery::ring(at, c.peek_m.0, c.peek_m.1)
                    };
                    let last = self.last_peek;
                    let spot = tactics.spots(&query, now, 8, rng).into_iter().find(|spot| {
                        last.is_none_or(|last| another_angle(enemy, last, spot.at, c))
                    });
                    if let Some(spot) = spot {
                        self.last_peek = Some(spot.at);
                        self.cover = Cover::Peeking {
                            at: spot.at,
                            until: now + c.peek_ms,
                        };
                    }
                }
                Some(_) => {}
            },
            Cover::Peeking { at, until } => {
                if now >= until || me.feet.distance_xz(at) <= 1.0 {
                    self.cover = Cover::Open;
                }
            }
        }
    }

    /// Play 11: the band against the target, and a step into it.
    fn matchup(
        &mut self,
        me: &PlaceView,
        sit: &Situation,
        memory: &Memory,
        now: u32,
        c: &CombatTuning,
    ) {
        self.band = None;
        let stepping = self.range_step.is_some();
        self.range_step = None;
        self.outranged = false;
        self.outranged_by = None;
        let Some(track) = self
            .target
            .and_then(|target| memory.seen(target, now, sit.view.clock))
        else {
            return;
        };
        let enemy = sit.weapons.get(track.who()).copied().flatten();
        // With the pistol out, the pistol's band.
        let own = if self.hands().pistol_out {
            Some(WeaponKind::Pistol)
        } else {
            sit.weapon
        };
        let Some(band) = preferred_band(own, enemy, c) else {
            return;
        };
        self.band = Some(band);
        // Dry: no step at him.
        if !c.band_legs || self.dry() {
            return;
        }
        let d = track.at().distance_xz(me.feet);
        // Outranged by a sniper: no step in at him; seen facing it, out of
        // his sight.
        if let Some(own) = own
            && c.outranged_hide
            && own != WeaponKind::Sniper
            && enemy == Some(WeaponKind::Sniper)
            && d > c.band(own).1 + c.band_slack_m
        {
            let facing = sit.view.place(track.who()).is_some_and(|him| {
                wrap(him.yaw - bearing(track.at(), me.feet)).abs()
                    <= c.outranged_aimed_deg.to_radians()
            });
            self.outranged = c.outranged_cover && facing;
            self.outranged_stats.sightings += 1;
            self.outranged_stats.facing += u32::from(facing);
            self.outranged_stats.exposed += u32::from(!matches!(self.cover, Cover::Hiding { .. }));
            self.outranged_by = Some((track.who(), track.at(), sit.view.clock));
            return;
        }
        // A carrier cannot shoot: no step back from him. What he carries is what the team saw of him (a human sees the
        // battery in his hands as long as he sees him), asked of the objective helper; with `band_carrier_seen_ms` 0, the
        // world's now.
        let carrier = c.band_hold_on_carrier
            && if c.band_carrier_seen_ms == 0 {
                sit.view.place(track.who()).is_some_and(super::objective_actor::is_actor)
            } else {
                sit.team.is_some_and(|team| team.seen_carrying(track.who(), now, c.band_carrier_seen_ms))
            };
        let along = if d < band.0 - c.band_slack_m {
            (!carrier).then_some(-c.band_step_m)
        } else if d > band.1 + c.band_slack_m {
            Some(c.band_step_m)
        } else {
            None
        };
        let fine = |at: &Vec3| sit.tactics.is_some_and(|tactics| tactics.fine_goal(*at, now, sit.side));
        // A step goes across the line when `band_cross_deg` says so, to the side after the last one's, else the other, else along it.
        let right_first = if stepping { self.range_right } else { !self.range_right };
        let step = along.map(|m| {
            if let Some(deg) = c.band_cross_deg {
                for right in [right_first, !right_first] {
                    let at = step_across(me.feet, track.at(), m, deg.to_radians(), right);
                    if fine(&at) {
                        self.range_right = right;
                        return at;
                    }
                }
            }
            step_toward(me.feet, track.at(), m)
        });
        self.range_step = step.filter(fine);
    }

    /// Play 20: now and then, with nothing seen, a grenade at the nearest
    /// place an enemy was seen holding an angle or sensed staying at. The
    /// throw is weighed ([`weigh_throw`]) once the chance has said yes; one
    /// that fails is not made, and the next place is weighed
    /// ([`pick_place`]), up to `grenade_places` of them, nearest first.
    fn grenade_roll(&mut self, look: RollView, aim: &AimTuning, skill: f32, rng: &mut Rng) {
        let RollView {
            me,
            sit,
            memory,
            now,
        } = look;
        let (view, team) = (sit.view, sit.team);
        let c = &aim.combat;
        self.grenade_stats.thinks += 1;
        let no_frag = c.grenade_kit_cap && self.frags_left == Some(0);
        // [C20] The window of a throw let go: a second grenade at its place.
        if let Some(volley) = self.volley {
            if !c.grenade_volley || self.thrown >= c.grenades || now >= volley.until || no_frag {
                // A second grenade the kit cannot give is a refused roll, counted once as the window closes.
                self.refused_no_frag += u32::from(no_frag && c.grenade_volley && self.thrown < c.grenades && now < volley.until);
                self.volley = None;
            } else if self.throw.is_none() && self.target.is_none() && now >= volley.next {
                self.volley_roll(&look, volley, aim, skill, rng);
                return;
            }
        }
        // [A170] A throw at a held spot the team has dropped since the roll is given up while the key is not yet let go.
        if c.grenade_held_recheck
            && let Some(throw) = self.throw
            && let Some(id) = throw.spot
            && !throw.released
            && !team.is_some_and(|team| team.threat.held.iter().any(|line| line.id == id))
        {
            self.grenade_stats.held_dropped[usize::from(throw.pressed.is_some())] += 1;
            self.throw = None;
            self.throw_notes.push(format!(
                "holds a grenade back at ({:.1}, {:.1}, {:.1}): the held spot was dropped",
                throw.at.x, throw.at.y, throw.at.z
            ));
        }
        // [close] A place within the close band by position alone, for the funnel of why the close throw is rare.
        let in_band = |at: Vec3| (c.grenade_close_m.0..=c.grenade_close_m.1).contains(&at.distance_xz(me.feet));
        let present = c.grenade_close
            && (self.held.iter().any(|(_, at, _)| in_band(*at))
                || self.camps.iter().any(|camp| camp.last >= camp.since + c.camp_ms && in_band(camp.at))
                || (c.grenade_tracks && memory.tracks().iter().any(|track| track.own.is_some_and(|(at, _)| in_band(at)))));
        self.grenade_stats.close_gate[0] += u32::from(present);
        let busy = if self.throw.is_some() {
            Some(0)
        } else if self.target.is_some() {
            Some(1)
        } else if self.thrown >= c.grenades {
            Some(2)
        } else if now < self.next_grenade_roll {
            Some(3)
        } else {
            None
        };
        if let Some(reason) = busy {
            self.grenade_stats.close_gate[1 + reason] += u32::from(present);
            // [measure] A sensed held spot that this think could not roll for.
            if c.grenade_at_held.is_some()
                && team.is_some_and(|team| {
                    team.threat
                        .held
                        .iter()
                        .any(|line| line.seen.is_some_and(|sensed| now.saturating_sub(sensed) <= super::threat::SENSED_MS))
                })
            {
                self.grenade_stats.held_busy[reason] += 1;
            }
            return;
        }
        let held = self
            .held
            .iter()
            .map(|(_, at, _)| (*at, PlaceSource::Held));
        let camps = self
            .camps
            .iter()
            .filter(|camp| camp.last >= camp.since + c.camp_ms)
            .map(|camp| (camp.at, PlaceSource::Camp));
        // A teammate near the place or on his way there: no throw to weigh
        // (the blast is weighed, once the chance has said yes).
        let friend_near = |at: Vec3| {
            if c.grenade_recheck {
                return friend_in_blast(me, view, at, c).is_some();
            }
            view.team(me.team)
                .any(|p| p.place != me.place && p.feet.distance_xz(at) < c.grenade_friend_m)
        };
        // [G1] The places a teammate decided to throw at lately.
        let claims: Vec<Vec3> = team
            .filter(|_| c.grenade_claim && c.grenade_claim_m > 0.0)
            .map(|team| {
                team.throws_of_others(me.place, now, c.grenade_claim_ms)
                    .map(|claim| claim.at)
                    .collect()
            })
            .unwrap_or_default();
        let claimed = |at: Vec3| claims.iter().any(|k| k.distance_xz(at) < c.grenade_claim_m);
        let mut dropped = 0;
        let sensed: Vec<(Vec3, PlaceSource)> = held.chain(camps).collect();
        // [close] The places at close range, for a roll no far place is thrown at: the same sources.
        let near_range = c.grenade_close && c.grenade_trace && sit.retreat.is_some() && sit.tactics.is_some();
        let in_close = |at: Vec3| near_range && (c.grenade_close_m.0..=c.grenade_close_m.1).contains(&at.distance_xz(me.feet));
        let mut close: Vec<(Vec3, PlaceSource)> = sensed
            .iter()
            .copied()
            .filter(|(at, _)| in_close(*at) && !claimed(*at))
            .collect();
        let mut places: Vec<(Vec3, PlaceSource)> = sensed
            .into_iter()
            .filter(|(at, _)| {
                if !((c.grenade_m.0..=c.grenade_m.1).contains(&at.distance_xz(me.feet))
                    && !friend_near(*at))
                {
                    return false;
                }
                let hit = claimed(*at);
                dropped += u32::from(hit);
                !hit
            })
            .collect();
        if c.grenade_tracks {
            // [G1] Where the senses last gave an enemy that is not in sight
            // now, if it is no older than `grenade_track_ms`.
            for track in memory.tracks() {
                let Some((at, sensed)) = track.own else {
                    continue;
                };
                let age = now.saturating_sub(sensed);
                if present && in_band(at) {
                    let gate = &mut self.grenade_stats.close_gate;
                    gate[11] += u32::from(age < c.grenade_track_gap_ms);
                    gate[12] += u32::from(age > c.grenade_track_ms);
                    gate[13] += u32::from(track.error_m > c.grenade_track_error_m);
                }
                if in_close(at)
                    && age >= c.grenade_track_gap_ms
                    && age <= c.grenade_track_ms
                    && track.error_m <= c.grenade_track_error_m
                    && !close.iter().any(|(near, _)| near.distance_xz(at) < c.camp_m)
                    && !claimed(at)
                {
                    let source = if track.sight_since.is_some() || track.source == Source::Seen {
                        PlaceSource::Lost
                    } else {
                        PlaceSource::Heard
                    };
                    close.push((at, source));
                }
                if age < c.grenade_track_gap_ms
                    || age > c.grenade_track_ms
                    || track.error_m > c.grenade_track_error_m
                    || !(c.grenade_m.0..=c.grenade_m.1).contains(&at.distance_xz(me.feet))
                    || places.iter().any(|(held, _)| held.distance_xz(at) < c.camp_m)
                    || friend_near(at)
                {
                    continue;
                }
                if claimed(at) {
                    dropped += 1;
                    continue;
                }
                let source = if track.sight_since.is_some() || track.source == Source::Seen {
                    PlaceSource::Lost
                } else {
                    PlaceSource::Heard
                };
                places.push((at, source));
            }
        }
        // [A170] The held spots whose shooter the team sensed on them lately: the camper the team cannot see.
        self.spot_shooters.clear();
        // [measure] How far a held spot of the team got in this think (0 none).
        let mut furthest = 0;
        if c.grenade_at_held.is_some()
            && let Some(team) = team
        {
            for line in &team.threat.held {
                furthest = furthest.max(1);
                let on_it = line
                    .seen
                    .is_some_and(|sensed| now.saturating_sub(sensed) <= super::threat::SENSED_MS);
                if !on_it {
                    continue;
                }
                furthest = furthest.max(2);
                let reach = if c.grenade_held_reach {
                    // The frag's real reach from the thrower's eye over the spot's ground, not the 27 m of the other places.
                    arc::Grenade::bots().max_range((me.eye().y - line.at.y).max(0.0)).unwrap_or(c.grenade_m.1).max(c.grenade_m.1)
                } else {
                    c.grenade_m.1
                };
                if !(c.grenade_m.0..=reach).contains(&line.at.distance_xz(me.feet)) {
                    continue;
                }
                furthest = furthest.max(3);
                if line.covers(me.feet) {
                    continue;
                }
                furthest = furthest.max(4);
                if places.iter().any(|(at, _)| at.distance_xz(line.at) < c.camp_m) || friend_near(line.at) {
                    continue;
                }
                furthest = furthest.max(5);
                if claimed(line.at) {
                    self.grenade_stats.claimed += 1;
                    continue;
                }
                furthest = 6;
                places.push((line.at, PlaceSource::Spot));
                self.spot_shooters.push((line.at, line.shooter, line.id));
            }
            for stage in 0..furthest {
                self.grenade_stats.held_funnel[stage] += 1;
            }
        }
        self.grenade_stats.claimed += dropped;
        if places.is_empty() && close.is_empty() {
            self.grenade_stats.close_gate[6] += u32::from(present);
            self.grenade_stats.close_gate[10] += u32::from(present && !near_range);
            return;
        }
        if no_frag {
            self.grenade_stats.close_gate[5] += u32::from(present);
            // A roll with a place in range that the kit cannot throw: refused (counted once), and the next one waits as a
            // roll would have.
            self.refused_no_frag += 1;
            self.next_grenade_roll = now + c.grenade_roll_ms;
            return;
        }
        self.grenade_stats.held_funnel[6] += u32::from(furthest == 6);
        // A held spot first, then the nearest.
        let order = |a: &(Vec3, PlaceSource), b: &(Vec3, PlaceSource)| {
            (a.1 != PlaceSource::Spot)
                .cmp(&(b.1 != PlaceSource::Spot))
                .then(a.0.distance_xz(me.feet).total_cmp(&b.0.distance_xz(me.feet)))
        };
        places.sort_by(order);
        close.sort_by(order);
        // [close] A roll with close places only is paced by `grenade_close_pace` when it is set.
        let pace = c.grenade_close_pace.filter(|_| places.is_empty());
        self.next_grenade_roll = now + pace.map_or(c.grenade_roll_ms, |pace| pace.roll_ms);
        self.grenade_stats.rolls += 1;
        for (_, source) in &places {
            self.grenade_stats.places[source.index()] += 1;
        }
        let Some(&(_, lead)) = places.first().or(close.first()) else {
            return;
        };
        let chance = match pace {
            Some(pace) => pace.chance,
            None => match (lead, c.grenade_at_held) {
                (PlaceSource::Spot, Some(chance)) => chance,
                (source, _) if source.is_track() => c.grenade_track_chance,
                _ => c.grenade_chance,
            },
        };
        if rng.unit() >= chance {
            self.grenade_stats.chance_no += 1;
            self.grenade_stats.close_gate[7] += u32::from(!close.is_empty());
            return;
        }
        let hold_ms = rng.range(c.grenade_hold_ms.0 as f32, c.grenade_hold_ms.1 as f32) as u32;
        let floor = mix(aim.error_deg, skill).to_radians();
        let error = (floor * (rng.unit() - 0.5), floor * 0.5 * (rng.unit() - 0.5));
        if !places.is_empty() {
            self.start_throw(&look, c, &places, error, hold_ms, ThrowKind::Roll);
        }
        // [close] No far place was thrown at: the close ones.
        self.grenade_stats.close_gate[8] += u32::from(self.throw.is_some() && !close.is_empty());
        self.grenade_stats.close_gate[9] += u32::from(self.throw.is_none() && !close.is_empty());
        if self.throw.is_none() && !close.is_empty() {
            self.start_close(&look, c, &close, error, hold_ms);
        }
    }

    /// [close] The first of `places` (at close range) whose throw passes every check of a close throw ([`close_floor`]: the
    /// arc, the landing not nearer than `grenade_close_m.0`, every teammate seen lately and out of reach of the blast), and
    /// whose blast the thrower can walk out of wherever it goes off ([`close_way_out`]), begun as a throw. A refusal makes no
    /// note: the roll comes again.
    fn start_close(
        &mut self,
        look: &RollView,
        c: &CombatTuning,
        places: &[(Vec3, PlaceSource)],
        error: (f32, f32),
        hold_ms: u32,
    ) {
        let RollView { me, sit, now, .. } = *look;
        let (Some(retreat), Some(tactics)) = (sit.retreat, sit.tactics) else {
            return;
        };
        let view = sit.view;
        let lowered = close_floor(c);
        let mesh = self.sight.ground();
        let stats = &mut self.grenade_stats;
        let mut notes: Vec<String> = Vec::new();
        stats.close[0] += 1;
        let picked = over_ground(mesh.as_deref(), places[0].0.y, |ground| {
            for &(at, source) in places.iter().take(c.grenade_places.max(1) as usize) {
                stats.close[1] += 1;
                if close_fresh(me, view, &lowered).is_err() {
                    stats.close[3] += 1;
                    return None;
                }
                let pick = match weigh_place(me, view, at, error, &lowered, ground) {
                    Ok(pick) => pick,
                    Err(refuse) => {
                        stats.close[2] += 1;
                        stats.close_gate[14 + refuse.index()] += 1;
                        continue;
                    }
                };
                let landing = throw_view_of(me, at, &lowered, pick).and_then(|(yaw, pitch)| {
                    arc::Flight::thrown(me.eye(), yaw + error.0, pitch + error.1).landing(ground)
                });
                if !landing.is_some_and(|landing| close_way_out(me, tactics, sit.side, retreat, &landing, now, &lowered)) {
                    stats.close[4] += 1;
                    continue;
                }
                stats.close[5] += 1;
                // The bench reads the landing, and where the walk out of the blast leads, from this note.
                if let Some(landing) = landing {
                    let out = tactics.world.grid.locate(me.feet, 1.5).and_then(|from| {
                        tactics.step_out_where(from, landing.rest, arc::Grenade::bots().splash_m + retreat.margin_m, 20.0, now, sit.side, false, &RouteFilter::open(), true)
                    });
                    notes.push(format!(
                        "decides a close grenade at ({:.1}, {:.1}, {:.1}): contact ({:.1}, {:.1}, {:.1}), rest ({:.1}, {:.1}, {:.1}), heading ({:.2}, {:.2}), way out {}",
                        at.x, at.y, at.z, landing.contact.point.x, landing.contact.point.y, landing.contact.point.z,
                        landing.rest.x, landing.rest.y, landing.rest.z, landing.heading.x, landing.heading.z,
                        out.map_or("none".to_owned(), |o| format!("({:.1}, {:.1}, {:.1})", o.x, o.y, o.z)),
                    ));
                }
                return Some((at, source, pick));
            }
            None
        });
        self.throw_notes.append(&mut notes);
        if let Some((at, source, pick)) = picked {
            self.throw_claim = Some(at);
            self.throw = Some(Throw {
                at,
                started: now,
                hold_ms,
                pressed: None,
                error,
                released: false,
                pitch: None,
                pick,
                source,
                kind: ThrowKind::Roll,
                near: None,
                close: Some(me.feet),
                spot: None,
            });
        }
    }

    /// [C20] The window of a throw let go (`grenade_volley`): when it is
    /// due, with nothing seen and no throw under way, a roll at the chance
    /// for a second grenade at the first one's place, weighed as any throw
    /// (a teammate in its blast, the thrower's own floor, the arc). A
    /// refusal makes no note: the roll comes again.
    fn volley_roll(&mut self, look: &RollView, volley: Volley, aim: &AimTuning, skill: f32, rng: &mut Rng) {
        let c = &aim.combat;
        self.volley = Some(Volley {
            next: look.now + c.grenade_volley_every_ms,
            ..volley
        });
        self.grenade_stats.volley[1] += 1;
        // [A187 #10] By evidence: the place is where an enemy was sensed by the first's place since it was let go.
        let place = if c.grenade_volley_track {
            match volley_evidence(look, &volley, c) {
                Some(place) => {
                    self.grenade_stats.volley[4] += 1;
                    place
                }
                None => {
                    self.grenade_stats.volley[5] += 1;
                    return;
                }
            }
        } else {
            (volley.at, volley.source)
        };
        if rng.unit() >= c.grenade_volley_chance {
            self.grenade_stats.volley[2] += 1;
            return;
        }
        let hold_ms = rng.range(c.grenade_hold_ms.0 as f32, c.grenade_hold_ms.1 as f32) as u32;
        let floor = mix(aim.error_deg, skill).to_radians();
        let error = (floor * (rng.unit() - 0.5), floor * 0.5 * (rng.unit() - 0.5));
        // The lowered floor is for a throw from rest: a volley is made while the first grenade is in
        // the air, and the way out of its blast is the first throw's.
        let sit = Situation {
            retreat: None,
            ..*look.sit
        };
        let look = RollView { sit: &sit, ..*look };
        self.start_throw(&look, c, &[place], error, hold_ms, ThrowKind::Volley);
    }

    /// Play 20: the first of `places` (nearest first) whose throw passes
    /// every check, begun as a throw, else the nearest place's note. With
    /// the avoidance on (`Situation::retreat`) and `grenade_near_m` below
    /// the floor, the places refused by the floor alone are weighed again
    /// under the lowered one, for a thrower with a way out of the blast
    /// ([`can_retreat`]).
    fn start_throw(
        &mut self,
        look: &RollView,
        c: &CombatTuning,
        places: &[(Vec3, PlaceSource)],
        error: (f32, f32),
        hold_ms: u32,
        kind: ThrowKind,
    ) {
        let RollView { me, sit, now, .. } = *look;
        let view = sit.view;
        let mesh = self.sight.ground();
        let spots: Vec<Vec3> = places.iter().map(|(at, _)| *at).collect();
        let stats = &mut self.grenade_stats;
        let mut by_floor: Vec<usize> = Vec::new();
        let picked = over_ground(mesh.as_deref(), spots[0].y, |ground| {
            let first = pick_place_noting(me, view, &spots, error, c, ground, |i, verdict| {
                let source = places[i].1.index();
                stats.weighed[source] += 1;
                match verdict {
                    Ok(pick) => stats.passed[source][pick.kind()] += 1,
                    Err(why) => {
                        stats.refused[source][why.index()] += 1;
                        if why == Refuse::Thrower {
                            by_floor.push(i);
                        }
                    }
                }
            });
            match first {
                Ok((at, pick)) => Ok((at, pick, None)),
                Err(refused) => near_place(look, c, &spots, &by_floor, error, ground, stats)
                    .map(|(at, pick)| (at, pick, Some(me.feet)))
                    .ok_or(refused),
            }
        });
        match picked {
            Ok((at, pick, near)) => {
                self.throw_claim = Some(at);
                let source = places
                    .iter()
                    .find(|(place, _)| *place == at)
                    .map_or(PlaceSource::Held, |(_, source)| *source);
                self.throw = Some(Throw {
                    at,
                    started: now,
                    hold_ms,
                    pressed: None,
                    error,
                    released: false,
                    pitch: None,
                    pick,
                    source,
                    kind,
                    near,
                    close: None,
                    spot: self
                        .spot_shooters
                        .iter()
                        .find(|(spot, ..)| *spot == at)
                        .filter(|_| source == PlaceSource::Spot)
                        .map(|(.., id)| *id),
                });
            }
            Err((at, why)) if kind == ThrowKind::Roll => self.hold_back(at, why),
            Err(_) => {}
        }
    }

    /// Where this bot's own senses had the enemy `who` staying (for at least `camp_ms`), if they had.
    pub fn camp_of(&self, who: Place, camp_ms: u32) -> Option<Vec3> {
        self.camps
            .iter()
            .find(|camp| camp.who == who && camp.last >= camp.since + camp_ms)
            .map(|camp| camp.at)
    }

    /// Play 20: the view is held for a grenade: from the key's press to the
    /// launch and `grenade_look_after_ms` more (the grenade leaves along the
    /// view then). The play's use of something waits for it
    /// ([`super::drive::Gates::using`]).
    pub fn view_held(&self) -> bool {
        self.throw.is_some_and(|throw| throw.pressed.is_some())
    }

    /// [G2] The arc for `throw`'s place searched again from where the
    /// thrower stands now, when the chosen arc failed for what is in the way
    /// (`why`) and `grenade_repick` is on: the new arc, or none (counted).
    fn repick(
        &mut self,
        mesh: Option<&mapdata::RayTree>,
        throw: &Throw,
        me: &PlaceView,
        view: &MatchView,
        c: &CombatTuning,
        why: Refuse,
    ) -> Option<ArcPick> {
        if !c.grenade_repick
            || !c.grenade_search
            || !matches!(why, Refuse::Thrower | Refuse::Wall | Refuse::Off)
        {
            return None;
        }
        let pick = over_ground(mesh, throw.at.y, |ground| {
            weigh_place(me, view, throw.at, throw.error, c, ground)
        })
        .ok();
        self.grenade_stats.rechecks[usize::from(pick.is_none())] += 1;
        pick
    }

    /// [measure] A throw under way given up at the press or in the hold:
    /// counted by reason, then as [`Self::hold_back`].
    fn give_up(&mut self, at: Vec3, why: Refuse) {
        self.grenade_stats.given_up[why.index()] += 1;
        self.hold_back(at, why);
    }

    /// Play 20: a throw not made, or given up: the note ("holds a grenade
    /// back at (x, y, z): <reason>").
    fn hold_back(&mut self, at: Vec3, why: Refuse) {
        self.throw = None;
        self.throw_notes.push(format!(
            "holds a grenade back at ({:.1}, {:.1}, {:.1}): {}",
            at.x,
            at.y,
            at.z,
            why.why()
        ));
    }

    /// Play 20: one tick of a throw under way, which has the look and the
    /// hands: turn to the view that lands the grenade at the place; once
    /// it is on it, weigh the throw again and press; hold the key
    /// `hold_ms` (weighing it every tick, the key let go on a refusal: the
    /// client throws only once its weapon reaches the throw state); let go
    /// of it, the throw counted; and keep the view, with the legs standing
    /// where the grenade leaves from, until it has left the hand
    /// (`grenade_launch_ms` after the press) and `grenade_look_after_ms`
    /// more. `None`: the throw is over, given up or not made, and the tick
    /// goes on without it.
    fn throwing(
        &mut self,
        mut throw: Throw,
        me: &PlaceView,
        view: &MatchView,
        now: u32,
        c: &CombatTuning,
        settle_s: f32,
    ) -> Option<Override> {
        self.held_ms = 0;
        self.gates.bits |= gate::THROW;
        let Some(fuse) = grenade_fuse_ms() else {
            self.throw = None;
            return None;
        };
        // [G3] A near throw is weighed by the lowered floor while the thrower stands where the way
        // out of the blast was found.
        let lowered;
        let c = match (throw.near, throw.close) {
            (Some(from), _) if me.feet.distance_xz(from) <= NEAR_STAY_M => {
                lowered = near_floor(c);
                &lowered
            }
            (_, Some(from)) if me.feet.distance_xz(from) <= NEAR_STAY_M => {
                lowered = close_floor(c);
                &lowered
            }
            _ => c,
        };
        // The pitch for the place from where the thrower stands now. Once the
        // key is let go the grenade leaves at the launch whatever he does:
        // the last pitch is held, not the look given back (a thrower who
        // stepped down in the hold, the place out of reach from there,
        // launched at the pitch of the walk after it: 1.1 m short in a
        // smoke).
        let pitch = match (throw_view_of(me, throw.at, c, throw.pick), throw.pitch) {
            (Some((_, pitch)), _) => pitch,
            (None, Some(last)) if throw.released => last,
            (None, _) => {
                self.give_up(throw.at, Refuse::Reach);
                return None;
            }
        };
        throw.pitch = Some(pitch);
        let mesh = self.sight.ground();
        let weigh = |mates: bool, pick: ArcPick| {
            // A close throw weighs the teammates every time, and refuses for one not seen lately.
            let mates = mates || throw.close.is_some();
            let seen = if throw.close.is_some() { close_fresh(me, view, c) } else { Ok(()) };
            seen.and_then(|()| over_ground(mesh.as_deref(), throw.at.y, |ground| match pick {
                ArcPick::Flat => weigh_throw(me, view, throw.at, throw.error, c, mates, ground),
                ArcPick::Pitch { .. } | ArcPick::Yawed { .. } => throw_view_of(me, throw.at, c, pick)
                    .ok_or(Refuse::Reach)
                    .and_then(|v| {
                        weigh_view(
                            me,
                            view,
                            throw.at,
                            v,
                            throw.error,
                            c,
                            mates,
                            ground,
                            c.grenade_search_off_m,
                        )
                    }),
            }))
        };
        let eye = me.eye();
        let mut keys = 0;
        match throw.pressed {
            None if off_by(
                me,
                throw_point(eye, throw.at, pitch, throw.pick.dyaw()),
                throw.error,
            )
                <= c.grenade_within_deg.to_radians() =>
            {
                // The trace again at the press, from where it stands now.
                if let Err(why) = weigh(c.grenade_recheck, throw.pick) {
                    // The arc found again for the place, the view to turn to it first.
                    let Some(pick) = self.repick(mesh.as_deref(), &throw, me, view, c, why) else {
                        self.give_up(throw.at, why);
                        return None;
                    };
                    throw.pick = pick;
                } else {
                    throw.pressed = Some(now);
                    keys = key::GRENADE;
                }
            }
            None if now >= throw.started + c.grenade_give_up_ms => {
                self.grenade_stats.timeouts[usize::from(self.target.is_some())][throw.pick.kind()] += 1;
                self.throw = None;
                return None;
            }
            None => {}
            Some(pressed) => {
                if now < pressed + throw.hold_ms {
                    // A teammate in the blast, or about to walk into it:
                    // the key let go before the throw starts.
                    if let Err(why) = weigh(c.grenade_recheck, throw.pick) {
                        let Some(pick) = self.repick(mesh.as_deref(), &throw, me, view, c, why) else {
                            self.give_up(throw.at, why);
                            return None;
                        };
                        throw.pick = pick;
                    }
                    keys = key::GRENADE;
                } else if !throw.released {
                    // Let go: thrown. The fuse runs from the press. Then
                    // in, after it goes off.
                    throw.released = true;
                    self.thrown += 1;
                    self.grenade_stats.nth[(self.thrown as usize - 1).min(2)] += 1;
                    self.grenade_stats.thrown_from[throw.source.index()] += 1;
                    self.last_throw = Some(now);
                    let blast = pressed + fuse;
                    // [C20] A second grenade at this place, while this one lies or flies.
                    self.volley = (c.grenade_volley && self.thrown < c.grenades).then(|| Volley {
                        at: throw.at,
                        source: throw.source,
                        since: now,
                        next: now + c.grenade_volley_ms.0,
                        // By evidence the window runs past the first's blast: the enemy who stays or comes back is the point.
                        until: if c.grenade_volley_track { now + c.grenade_volley_ms.1 } else { (now + c.grenade_volley_ms.1).min(blast) },
                    });
                    if self.volley.is_some() {
                        self.grenade_stats.volley[0] += 1;
                    }
                    if throw.kind == ThrowKind::Volley {
                        self.grenade_stats.volley[3] += 1;
                    }
                    if throw.near.is_some() {
                        self.grenade_stats.near[2] += 1;
                    }
                    if throw.close.is_some() {
                        self.grenade_stats.close[6] += 1;
                        // The bench's own-side damage is counted by the throws it knows to be close.
                        self.throw_notes.push(format!("throws a close grenade, blast at {blast}"));
                    }
                    let from = if c.grenade_push_after_blast {
                        blast + c.grenade_push_after_ms
                    } else {
                        now + c.grenade_push_ms.0
                    };
                    if (throw.source.is_track() && !c.grenade_push_tracks) || throw.source == PlaceSource::Spot {
                        self.grenade_stats.pushes_skipped += 1;
                    } else {
                        self.push = Some(Push {
                            toward: throw.at,
                            from,
                            until: from + c.grenade_push_ms.1.saturating_sub(c.grenade_push_ms.0),
                            why: LegsWhy::AfterGrenade,
                        });
                    }
                    self.throw_notes.push(format!(
                        "throws a grenade at ({:.1}, {:.1}, {:.1}), blast at {blast}{}",
                        throw.at.x,
                        throw.at.y,
                        throw.at.z,
                        match self.spot_shooters.iter().find(|(at, ..)| *at == throw.at) {
                            Some((_, shooter, _)) if throw.source == PlaceSource::Spot => format!(" (at a held spot of shooter {shooter})"),
                            _ => String::new(),
                        }
                    ));
                }
                if now >= pressed + c.grenade_launch_ms + c.grenade_look_after_ms && throw.released
                {
                    self.throw = None;
                    return None;
                }
            }
        }
        self.throw = Some(throw);
        Some(Override {
            look_at: Some(throw_point(eye, throw.at, pitch, throw.pick.dyaw())),
            error: throw.error,
            turn_time_s: Some(settle_s * 0.5),
            keys,
            legs: throw.pressed.map(|_| Legs {
                at: me.feet,
                within_m: 1.0,
                run: false,
                crouch: me.crouched(),
                why: LegsWhy::Throwing,
            }),
            band_m: self.band,
            ..Override::default()
        })
    }

    /// The reload habit: `key::RELOAD` (or `key::WEAPON_2`) for this tick,
    /// or 0. With the magazine out (its rounds counted), at the first
    /// moment with no target seen or out of sight (or where it stands when
    /// no cover was found); tactically after `reload_idle_ms` with nothing
    /// seen and `reload_some_share` of the magazine fired. Nothing to load
    /// (the kit's ammo is a life's), weapon 1 dry: the pistol, at once and
    /// where it stands (a switch is quicker than any way to cover). No shot
    /// until the weapon's reload or the switch is done. The client would
    /// also reload by itself at a press on an empty weapon (its reload
    /// predicate 0x2E3EF0 takes the trigger with nothing in it); the count
    /// keeps the bot from needing that.
    fn reload(&mut self, seen: bool, now: u32, c: &CombatTuning) -> u32 {
        if now < self.busy_until {
            return 0;
        }
        let hiding = matches!(self.cover, Cover::Hiding { .. });
        let last_seen = self.last_seen;
        let reload_here = self.reload_here;
        let hands = self.hands();
        if let Some(ms) = hands.switch_ms() {
            hands.pistol_out = true;
            self.busy_until = now + ms;
            self.busy_why = gate::SWITCH;
            self.held_ms = 0;
            self.press_until = 0;
            self.next_press = 0;
            self.reload_here = false;
            return key::WEAPON_2;
        }
        let load = hands.held_mut();
        let fired = load.gun.magazine - load.rounds;
        let some = ((load.gun.magazine as f32 * c.reload_some_share).ceil() as u32).max(1);
        let tactical = !seen && now >= last_seen + c.reload_idle_ms && fired >= some;
        if load.loadable() == 0
            || !(load.rounds == 0 && (!seen || hiding || reload_here) || tactical)
        {
            return 0;
        }
        let (left, magazine) = (load.rounds, load.gun.magazine);
        let ms = load.reload(now);
        let gun = load.gun;
        self.mod_counts.reload(gun);
        self.reload_note = Some(ReloadNote {
            at: now,
            until: now + ms,
            why: match (left, seen) {
                (0, true) => ReloadWhy::EmptyInView,
                (0, false) => ReloadWhy::Empty,
                _ => ReloadWhy::Tactical,
            },
            left,
            magazine,
        });
        self.busy_until = now + ms;
        self.busy_why = gate::RELOAD;
        self.held_ms = 0;
        self.press_until = 0;
        self.reload_here = false;
        // A reload resets the kick (0x310E52; when in it: not traced).
        self.recoil.reset();
        key::RELOAD
    }

    /// The trigger this tick when the reflex `wants` to fire, and the rounds
    /// that leave: an automatic weapon fires while held, a shot at every
    /// pass of its fire clip; any other is pressed for `press_ms` (at most
    /// half its shot) and pressed again only after its cycle,
    /// `press_gap_ms` and the wait its kind takes at `distance`
    /// ([`FireTuning::wait_ms`]: a bolt action's sights brought back on
    /// him, a marksman rifle's slower presses at range), one round a press
    /// ([`super::guns`] on why).
    #[allow(clippy::too_many_arguments)]
    fn trigger(
        &mut self,
        wants: bool,
        allowed: bool,
        now: u32,
        c: &CombatTuning,
        distance: f32,
        rng: &mut Rng,
    ) -> bool {
        let held = *self.hands().held();
        let gun = held.gun;
        // A press under way is held to its end while the play allows fire
        // (a tick's tap might meet a weapon not ready to take it); its
        // round is counted already.
        if !gun.auto && allowed && now < self.press_until {
            return true;
        }
        if !wants || held.rounds == 0 {
            self.held_ms = 0;
            return false;
        }
        let shots = if gun.auto {
            let before = gun.shots_held(self.held_ms);
            self.held_ms += self.tick_ms;
            gun.shots_held(self.held_ms) - before
        } else if now < self.next_press {
            return false;
        } else {
            let hold = c.press_ms.min(gun.shot_ms() as u32 / 2).max(1);
            self.press_until = now + hold;
            self.next_press =
                now + gun.cycle_ms() + c.press_gap_ms + c.fire.wait_ms(gun, distance, rng);
            1
        };
        let load = self.hands().held_mut();
        load.rounds = load.rounds.saturating_sub(shots);
        self.fired = (now, shots);
        if shots > 0 {
            self.last_fired = self.target.map(|target| (target, now));
        }
        true
    }

    /// The trigger let go at `now` though this tick's [`Reflex::tick`]
    /// fired (a jump: the weapon takes none while it fires,
    /// `weapon_core::can_jump` 0x2E3A70): the rounds it counted this tick
    /// are given back, a burst held starts again, and a weapon of a round a
    /// press may press again.
    pub fn hold_fire(&mut self, now: u32) {
        self.held_ms = 0;
        self.press_until = self.press_until.min(now);
        let (at, shots) = std::mem::take(&mut self.fired);
        if at == now && shots > 0 {
            let gun = self.hands().held().gun;
            let load = self.hands().held_mut();
            load.rounds = (load.rounds + shots).min(gun.magazine);
            if !gun.auto {
                self.next_press = now;
            }
        }
    }

    /// A run of the side step the other way than the last.
    fn step_aside(&mut self, now: u32, c: &CombatTuning, rng: &mut Rng) {
        let next = match self.strafe_last {
            key::LEFT => key::RIGHT,
            key::RIGHT => key::LEFT,
            _ if rng.unit() < 0.5 => key::LEFT,
            _ => key::RIGHT,
        };
        self.strafe = next;
        self.strafe_last = next;
        self.strafe_until =
            now + rng.range(c.strafe_run_ms.0 as f32, c.strafe_run_ms.1 as f32) as u32;
    }

    /// The side step this tick while it stands to aim: runs one way then
    /// the other, now and then a still moment between.
    fn side_step(&mut self, now: u32, c: &CombatTuning, rng: &mut Rng) -> u32 {
        if now >= self.strafe_until {
            if self.strafe != 0 && rng.unit() < c.strafe_still_chance {
                self.strafe = 0;
                self.strafe_until =
                    now + rng.range(c.strafe_still_ms.0 as f32, c.strafe_still_ms.1 as f32) as u32;
            } else {
                self.step_aside(now, c, rng);
            }
        }
        self.strafe
    }

    /// The side step's reasons so far ([`StrafeStats`]).
    pub fn strafe_stats(&self) -> StrafeStats {
        self.strafe_stats
    }

    /// The legs the reflex takes this tick, if any.
    fn legs(&mut self, now: u32) -> Option<Legs> {
        if self.push.is_some_and(|p| now >= p.until) {
            self.push = None;
        }
        match self.cover {
            Cover::Hiding { at, .. } => {
                return Some(Legs {
                    at,
                    within_m: 0.8,
                    run: true,
                    crouch: false,
                    why: LegsWhy::BreakSight,
                });
            }
            Cover::Peeking { at, .. } => {
                return Some(Legs {
                    at,
                    within_m: 0.8,
                    run: false,
                    crouch: false,
                    why: LegsWhy::RePeek,
                });
            }
            Cover::Open => {}
        }
        if let Some(push) = self.push.filter(|p| now >= p.from) {
            return Some(Legs {
                at: push.toward,
                within_m: 3.0,
                run: false,
                crouch: false,
                why: push.why,
            });
        }
        let at = self.range_step.filter(|_| self.target.is_some())?;
        Some(Legs {
            at,
            within_m: 1.0,
            run: false,
            crouch: false,
            why: LegsWhy::Range,
        })
    }

    /// One tick: where to look, whether to fire, the keys and the legs.
    /// `fire_allowed`: the play's word (a carrier cannot shoot). The sights
    /// are let down while a round-by-round reload loads
    /// ([`Self::keep_loop_going`]).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        memory: &Memory,
        now: u32,
        fire_allowed: bool,
        aim: &AimTuning,
        skill: f32,
        rng: &mut Rng,
    ) -> Override {
        let mut over = self.combat_tick(me, view, memory, now, fire_allowed, aim, skill, rng);
        self.keep_loop_going(&mut over, now, &aim.combat);
        self.heal_press(&mut over, me, now);
        over
    }

    /// The heal habit: the item's key goes out this tick, over whatever the
    /// bot does (a use changes no hand, no speed and holds no fire: PROVEN for
    /// the medkit, `medkit::action` 0x33D700, `inventory::action` 0x2FCAA0;
    /// LIKELY for the bandages and the antitoxin, the same kind-1 path). Not
    /// for a carrier, whose hands the battery has (`process_quick_slots`
    /// 0x2D8DA0), nor in a grenade throw. The cover walk it owes begins at
    /// the next think ([`Self::heal_cover`]).
    fn heal_press(&mut self, over: &mut Override, me: &PlaceView, now: u32) {
        let Some(wish) = self.heal_wish.take() else {
            return;
        };
        if me.carrying.is_some() || self.throw.is_some() {
            return;
        }
        over.keys |= wish.key;
        self.heal_presses += 1;
        self.heal_walk = Some((wish, now));
    }

    /// The walk owed to a heal press: in a duel, hidden from the enemy it
    /// fights, within the item's delay of walking (the reflex's own cover
    /// search, [`Self::break_sight`]); not with a known enemy within the
    /// wish's `enemy_m`, not a carrier, not while it already hides, and
    /// gone if the think came too late to use the delay.
    #[allow(clippy::too_many_arguments)]
    fn heal_cover(
        &mut self,
        wish: HealWish,
        pressed_at: u32,
        me: &PlaceView,
        sit: &Situation,
        memory: &Memory,
        now: u32,
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        let enemy_m = memory
            .tracks()
            .iter()
            .map(|t| t.at.distance_xz(me.feet))
            .min_by(f32::total_cmp);
        let mut note = HealNote {
            at: now,
            why: HealWhy::Walked,
            enemy_m,
            cover_m: None,
        };
        let refused = if now > pressed_at + HEAL_WALK_LATE_MS {
            Some(HealWhy::Late)
        } else if me.carrying.is_some() {
            Some(HealWhy::Carrier)
        } else if self.throw.is_some() {
            Some(HealWhy::Throwing)
        } else if wish.reach_m < c.hide_m.0 {
            Some(HealWhy::ShortReach)
        } else if enemy_m.is_some_and(|m| m <= wish.enemy_m) {
            Some(HealWhy::EnemyNear)
        } else {
            None
        };
        if let Some(why) = refused {
            note.why = why;
            self.heal_note = Some(note);
            return;
        }
        let sought =
            self.break_sight(me, sit, memory, now, false, false, (c.hide_m.0, wish.reach_m), c, rng);
        note.why = match sought {
            Sought::Began => HealWhy::Walked,
            Sought::Hiding => HealWhy::Hiding,
            Sought::Pushing => HealWhy::Pushing,
            Sought::NotDuel => HealWhy::NotDuel,
            Sought::NoTactics => HealWhy::NoTactics,
            Sought::Declined => HealWhy::Carrier,
            Sought::NoEnemy => HealWhy::NoEnemy,
            Sought::NoSpot => HealWhy::NoSpot,
        };
        if let (Sought::Began, Cover::Hiding { at, .. }) = (sought, self.cover) {
            note.cover_m = Some(at.distance_xz(me.feet));
            self.heal_walks += 1;
            self.heal_walk_at = Some(now);
        }
        self.heal_note = Some(note);
    }

    /// The last heal press's walk note (the bench reads it; nothing decides by it).
    pub fn heal_note(&self) -> Option<HealNote> {
        self.heal_note
    }

    /// The rounds the client has put in by `now` ([`super::guns::Load::advance`]), under
    /// the keys of the last tick. A loop that ended before the magazine was
    /// full frees the weapon at its own time, not the whole load's.
    fn count_rounds(&mut self, now: u32) {
        let held = self.held_keys;
        let Some(end) = self.hands().held_mut().advance(now, held) else {
            return;
        };
        self.loop_end = Some(end);
        let gun = self.hands().held().gun;
        self.mod_counts.loop_end(gun, &end);
        if self.busy_why == gate::RELOAD && end.ready < self.busy_until {
            self.busy_until = end.ready.max(now);
            if let Some(note) = self.reload_note.as_mut() {
                note.until = self.busy_until;
            }
        }
    }

    /// A round-by-round reload's loop goes on to its finish clip at the first
    /// round-end with the sights or the trigger held and a round in the
    /// weapon (the shotgun sub-FSM's predicates 0x339EE0 and 0x339EA0). Four
    /// branches of [`Self::combat_tick`] ask for the sights (a threat's look, a
    /// target just lost, the partner's death spot, a seen target) and none
    /// let them go, so 38 % of the SPAS-12's reloads in the recordings loaded
    /// one round and none reached six. While the loop runs the sights are
    /// down, and their aim-in counts afresh from its end; the finish clip is
    /// long enough for it. With a target seen or a threat on, the sights go
    /// down on purpose from the middle of the span before the round-end that
    /// is to end the loop ([`CombatTuning::reload_rounds_in_view`],
    /// [`super::guns::Load::cut_from`]). A magazine's reload takes no key but a weapon
    /// switch, and is left alone.
    fn keep_loop_going(&mut self, over: &mut Override, now: u32, c: &CombatTuning) {
        let in_view = self.last_seen == now || self.threat.is_some_and(|t| now < t.until);
        // A grenade throw under way has the hands (play 20, `Self::throwing`):
        // its keys are the grenade key alone, and the sights are not pressed
        // into them for a loop's early cut.
        let cut = self.throw.is_none()
            && in_view
            && c.reload_rounds_in_view > 0
            && self
                .hands()
                .held()
                .cut_from(c.reload_rounds_in_view)
                .is_some_and(|from| now >= from);
        if c.reload_aim_control && self.hands().held().loading() {
            if cut {
                over.keys |= key::AIM;
                self.aim_since = Some(self.aim_since.unwrap_or(now));
            } else {
                over.keys &= !key::AIM;
                self.aim_since = None;
            }
        }
        self.held_keys = over.keys & key::AIM != 0 || over.fire;
    }

    /// [`Self::tick`] with the sights as the branches below ask for them.
    #[allow(clippy::too_many_arguments)]
    fn combat_tick(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        memory: &Memory,
        now: u32,
        fire_allowed: bool,
        aim: &AimTuning,
        skill: f32,
        rng: &mut Rng,
    ) -> Override {
        let c = &aim.combat;
        self.new_life(me);
        self.count_rounds(now);
        // Ticks skipped (getting unstuck has the keys) had the trigger up.
        if self
            .last_tick
            .is_none_or(|last| now.saturating_sub(last) > 100)
        {
            self.held_ms = 0;
        }
        self.tick_ms = self
            .last_tick
            .map_or(50, |last| now.saturating_sub(last).clamp(1, 100));
        self.last_tick = Some(now);
        self.held_for = None;
        self.gate_tick(now);
        // The aim-in counts from the first tick of an unbroken press of the
        // aim (the input reaches the game within `aim_in_margin_ms`).
        let aim_since = self.aim_since.take();
        let legs = self.legs(now);
        let settle_s = mix(aim.settle_s, skill).max(0.05);
        let floor = mix(aim.error_deg, skill).to_radians();
        // Play 20: a throw under way has the look and the hands.
        if let Some(throw) = self.throw
            && let Some(over) = self.throwing(throw, me, view, now, c, settle_s)
        {
            return over;
        }
        // [smoke] A smoke throw under way has them next.
        if let Some(over) = self.smoke_throwing(me, now, c, settle_s) {
            return over;
        }
        // [A208] A hold for a cloud has them next.
        if let Some(over) = self.cloud_holding(me, view, memory, now, settle_s) {
            return over;
        }
        #[cfg(test)]
        self.watch_crossing(me, view, now);
        // An enemy noticed since the last think (the noticing is its
        // track's own time, `know::Memory::seen`) is taken as the target at
        // once: the first shot waits its reaction, not the next think too
        // (the owner, 2026-10-23: a sighting to the first shot of ~400-450
        // ms at skill 1). Only a seen, noticed enemy in range, as a think
        // takes one.
        // Its own last target back in sight a moment after he was lost,
        // noticed already: taken back as it had him, before he is noticed
        // again (`reacquire_keep`).
        if c.reacquire_keep
            && self.target.is_none()
            && let Some((who, _)) = self.lost.filter(|(_, at)| now <= at + c.aim_carry_ms)
            && memory
                .in_sight(who, now, view.clock)
                .is_some_and(|t| t.at().distance_xz(me.feet) <= aim.range_m)
        {
            self.retake(who);
        }
        if self.target.is_none()
            && memory
                .all_seen(now, view.clock)
                .iter()
                .any(|t| t.at().distance_xz(me.feet) <= aim.range_m)
        {
            self.choose(me, Some(view), memory, now, aim, skill, rng);
        } else if c.switch_between_thinks
            && let Some(kept) = self
                .target
                .and_then(|target| memory.seen(target, now, view.clock))
        {
            // Likewise an enemy noticed since the last think who presses
            // more than the target (a man stepping out beside it): him at
            // once, not a think later.
            let held_ms = now.saturating_sub(self.since);
            let seen = memory.all_seen(now, view.clock);
            let worth = Worth {
                view,
                known: &self.known,
                tuning: &c.worth,
                hurt_m: c.wounded_bonus_m,
                reach: self.reach_m.map(|reach| (me.feet, reach)),
            };
            if seen.iter().any(|t| {
                t.who() != kept.who()
                    && t.at().distance_xz(me.feet) <= aim.range_m
                    && presses_more(me, Some(view), Some(&worth), &kept, t, held_ms, c)
            }) {
                self.choose(me, Some(view), memory, now, aim, skill, rng);
            }
        }
        let seen = self
            .target
            .and_then(|target| self.seen_target(memory, target, now, view.clock));
        if seen.is_some() {
            self.last_seen = now;
        }
        let reload = self.reload(seen.is_some(), now, c);
        // A threat (a hit's shooter, a near shot from outside the view): a
        // moment's look, even from a target (`Self::threats`).
        if let Some(threat) = self.threat.filter(|t| now < t.until) {
            self.gates.bits |= gate::THREAT;
            self.gates.target = self.target;
            self.on_since = None;
            self.strafe = 0;
            self.strafe_until = 0;
            // No new shot; a one-round press under way is held to its end
            // (its round is counted). The aim held through the look
            // (`aim_carry`).
            let fire = self.trigger(false, fire_allowed, now, c, 0.0, rng);
            let mut keys = reload;
            if c.aim_carry && aim_since.is_some() {
                self.aim_since = aim_since;
                keys |= key::AIM;
            }
            return Override {
                look_at: Some(threat.at + Vec3::new(0.0, 1.4, 0.0)),
                fire,
                keys,
                legs,
                band_m: self.band,
                glance: !threat.hit || threat.bearing.is_some(),
                ..Override::default()
            };
        }
        let Some(track) = seen else {
            self.lapse = self
                .target
                .and_then(|target| view.place(target))
                .filter(|enemy| enemy.alive && enemy.present)
                .map(|enemy| Lapse {
                    band: super::polls::ping_band(enemy),
                    cut: self.held_ms > 0,
                });
            self.lose_target(now);
            match self.lost.filter(|(_, at)| now <= at + LOST_MS) {
                Some((who, _)) => {
                    self.gates.bits |= gate::LOST;
                    self.gates.target = Some(who);
                }
                None => self.gates.bits |= gate::NO_TARGET,
            }
            self.on_since = None;
            // The aim held while the target is lost a moment (`aim_carry`).
            let carry = c.aim_carry
                && aim_since.is_some()
                && self.lost.is_some_and(|(_, at)| now <= at + c.aim_carry_ms);
            if carry {
                self.aim_since = aim_since;
            }
            let reload = reload | if carry { key::AIM } else { 0 };
            self.held_ms = 0;
            self.strafe = 0;
            self.strafe_until = 0;
            if let Some(over) = self.fire_through((me, view), now, fire_allowed, aim, (legs, reload, carry), rng) {
                return over;
            }
            // Play 21: the partner's death spot within `trade_m`, weapon
            // up, unless a shot was heard lately (play 6 first: the killer
            // is there, not at the body); else play 6.
            let shot = self.sound.is_some_and(|(_, until)| now < until);
            if let Some((at, _)) = self
                .trade
                .filter(|(at, until)| now < *until && !shot && at.distance_xz(me.feet) <= c.trade_m)
            {
                self.aim_since = Some(aim_since.unwrap_or(now));
                return Override {
                    look_at: Some(at + Vec3::new(0.0, 1.3, 0.0)),
                    keys: key::AIM | reload,
                    legs,
                    ..Override::default()
                };
            }
            return match self.sound {
                Some((at, until)) if now < until => Override {
                    look_at: Some(at + Vec3::new(0.0, 1.4, 0.0)),
                    keys: reload,
                    legs,
                    glance: true,
                    ..Override::default()
                },
                _ => match self.glance.filter(|(_, until)| now < *until) {
                    // A callout's place: a glance.
                    Some((at, _)) => Override {
                        look_at: Some(at + Vec3::new(0.0, 1.4, 0.0)),
                        keys: reload,
                        legs,
                        glance: true,
                        ..Override::default()
                    },
                    None => Override {
                        // [aimpoint] Where the last target was last seen.
                        look_at: self.sight.pre_aim(now),
                        keys: reload,
                        legs,
                        ..Override::default()
                    },
                },
            };
        };
        // The seen body where it is now (a seen enemy is tracked by eye).
        let body = view.place(track.who()).filter(|p| p.alive);
        let known = track.at() + Vec3::new(0.0, 1.3, 0.0);
        let point = body.map_or(known, PlaceView::chest);
        // [aimpoint] The part of him that shows (none: where he was last
        // seen, else where the memory has him); a shot only clear of the
        // world (the last test below). A spray into the smoke he just went
        // into: only fire already going at him.
        let aimed = self
            .sight
            .aim(track.who(), body, point, if c.aim_last_known { known } else { point });
        let continuing = self
            .last_fire
            .is_some_and(|(who, at)| who == track.who() && now <= at + SPRAY_FIRING_MS);
        let aimed = aimed.sprayed(continuing);
        // A spray begun or ended: one note each.
        let spraying = matches!(aimed, super::aimpoint::Aim::Spray(_));
        if spraying != self.spraying {
            let what = if spraying { "begins" } else { "ends" };
            self.smoke_notes
                .push(format!("spray at player {}'s last seen point {what}", track.who()));
            self.spraying = spraying;
        }
        // Where he will be at the tick decided (`lead_target`): only a body seen.
        let led = body.filter(|_| aimed.sees_body() && c.lead_target);
        // And where the bullet will find him (`lead_flight`): the held weapon's ammunition, from the eye.
        let flight = self.hands().held().gun.flight.filter(|_| c.lead_flight || c.lead_drop);
        let point = led.map_or(aimed.point(), |body| {
            let at = aimed.point() + lead(body, now, c.lead_most_ms);
            let ahead = flight
                .and_then(|flight| flight.ahead(me.eye(), at, body.velocity))
                .unwrap_or_default();
            // The parts of it the switches ask for: along his velocity, and up by the drop.
            at + Vec3::new(
                if c.lead_flight { ahead.x } else { 0.0 },
                if c.lead_drop { ahead.y } else { 0.0 },
                if c.lead_flight { ahead.z } else { 0.0 },
            )
        });
        // The error settles from its start toward the skill's, never to 0,
        // and wanders slowly about him as a player's tracking does.
        let t = now.saturating_sub(self.since) as f32 / 1000.0;
        let fade = (-t / settle_s).exp();
        let settle = c.settle_guns.settle(c.settle, self.hands().held().gun.optic);
        let wander_deg = aim_settle::wander_deg(
            settle,
            mix(aim.wander_deg, skill),
            mix(aim.settled_wander_deg, skill),
            mix(aim.settle_tau_s, skill),
            t,
        );
        self.wander_step(aim, wander_deg, rng);
        let gun = self.hands().held().gun;
        // The error angles go with the zoom of the sights (`zoom_error`): the aim progress is the time since the aim key went
        // down over the sights' aim time.
        let zoom = c.zoom_error.factor(
            gun.optic,
            aim_since.map_or(0.0, |since| now.saturating_sub(since) as f32 / gun.aim_ms.max(1) as f32),
        );
        let error = (
            (self.error.0 * fade + self.wander.0 + floor * (rng.unit() - 0.5)) * zoom,
            (self.error.1 * fade + self.wander.1 + floor * 0.5 * (rng.unit() - 0.5)) * zoom,
        );
        // What a player knows of his own hand: the error to expect, not this tick's draw (`aim_settle`).
        let expected_deg = aim_settle::error_rms_deg(mix(aim.error_deg, skill), fade, wander_deg, zoom);
        // [recoil] The kick's rise of the view this tick, taken back by the
        // turn (any weapon); what is not taken back lifts the view and the
        // shot with it. The share taken back grows with the skill.
        let pull = -mix((c.recoil_pull_low, c.recoil_pull), skill) * self.kick_step;
        // The kick is in the game's pitch unit; the aim below reckons in elevations (`pitch.rs`).
        let lift = Pitch::of_elevation(me.pitch, self.scale).rise(self.kick_step + pull, self.scale);
        let off = off_by(me, point, error);
        // Play 19: spot him once he is in the crosshair, the cooldown kept.
        let mut keys = reload;
        if self.spot_pending == Some(track.who())
            && off <= c.spot_within_deg.to_radians()
            && self
                .last_spot
                .is_none_or(|last| now >= last + c.spot_cooldown_ms)
        {
            keys |= key::SPOT;
            self.last_spot = Some(now);
            self.spotted.retain(|(who, _)| *who != track.who());
            self.spotted.push((track.who(), now));
            self.spot_pending = None;
        }
        // Fire when the view is near the aim, a reaction after noticing,
        // with a round in the weapon: an automatic weapon in bursts, any
        // other at every cycle of its own (players press again as soon as
        // the weapon takes it); the weapon's trigger turns that into
        // presses and counts the rounds.
        let distance = point.distance_xz(me.feet);
        let within_deg = if c.fire_body_share > 0.0 {
            (c.fire_body_share * c.body_half_m)
                .atan2(distance.max(0.5))
                .to_degrees()
                .clamp(c.fire_within_min_deg, aim.fire_within_deg)
        } else {
            aim.fire_within_deg
        };
        let within = off <= within_deg.to_radians();
        let bolt = gun.handling == Handling::Bolt;
        // Down the sights at him, unless it runs for cover (a bolt action
        // always, else from `ads_near_m` or as drawn, and with
        // `aim_in_first` whenever the hip cone would not keep the shots
        // on him); no shot before the sights are up unless it would.
        let running = legs.is_some_and(|l| l.run);
        let hip_fits = !c.aim_in_first || hip_cone_fits(me, gun, self.strafe_fire, distance, c);
        let ads = !running && (bolt || distance >= c.ads_near_m || self.ads_near || !hip_fits);
        self.aim_since = ads.then(|| aim_since.unwrap_or(now));
        let aimed_in = self
            .aim_since
            .is_some_and(|since| now >= since + gun.aim_ms + c.aim_in_margin_ms);
        let ceiling_m = if bolt {
            c.fire.bolt_range_m.max(aim.range_m)
        } else {
            aim.range_m
        };
        // Hit lately: whoever shot it is answered out to the ceiling.
        let range_m = if self.hit_at.is_some_and(|at| now < at + c.hit_shot_ms) {
            ceiling_m
        } else {
            c.fire_range_m(gun, ceiling_m)
        };
        // [friends] A teammate in the line of fire: no shot, a step aside.
        let lines = Self::fire_lines(me, view, (point, error, lift), now, &aim.friends, self.scale);
        let friend = super::friends::blocking(
            me,
            view,
            &lines.0,
            distance,
            aim.friends.spread(gun, super::friends::Sights::of(aimed_in, me.keys, self.strafe_fire)),
            lines.1,
            &aim.friends,
        );
        self.held_for = friend.map(|f| super::friends::Held {
            who: f.who,
            ready: fire_allowed
                && within
                && self.hands().held().rounds > 0
                && distance <= range_m
                && (aimed_in || hip_fits),
            running,
        });
        // The shot waits for the aim to settle ([`aim_settle`]): a burst under way is held to its end, the first shot of one, and every shot of
        // another weapon, waits.
        let settling = now >= self.burst_until
            && aim_settle::waits(
                settle,
                expected_deg,
                distance,
                c.body_half_m,
                c.settle_k,
                now.saturating_sub(self.since),
                c.settle_max_wait_ms,
            );
        // [#24] The fight is on: aimed in on a target in range with a round in and the play's word to fire.
        let fight_on = c.stand_strafe
            && fire_allowed
            && aimed_in
            && distance <= range_m
            && self.hands().held().rounds > 0;
        self.gait = self.gait.next(fight_on, now, c.stand_strafe_hold_ms);
        // [measure] Each gate that fails ([`gate`]); the play's word and the
        // jump hold are the bot's ([`Self::outer_gates`]).
        let mut failed = 0;
        for (fails, bit) in [
            (!within, gate::WITHIN),
            (
                now < self.first_shot_at,
                if self.reacquired { gate::REACTION_AGAIN } else { gate::REACTION },
            ),
            (now < self.busy_until, self.busy_why),
            (self.hands().held().rounds == 0, gate::ROUNDS),
            (distance > range_m, gate::RANGE),
            (!(aimed_in || hip_fits) && running, gate::RUNNING),
            (
                !(aimed_in || hip_fits) && !running,
                if self.aimed_target == Some(track.who()) {
                    gate::AIMING_AGAIN
                } else {
                    gate::AIMING_IN
                },
            ),
            (settling, gate::SETTLING),
            (friend.is_some(), gate::FRIEND),
        ] {
            if fails {
                failed |= bit;
            }
        }
        if aimed_in {
            self.aimed_target = Some(track.who());
        }
        let open = fire_allowed && failed == 0;
        // The shot leaves along the view (kick and all): the wall test is on
        // the view now and the one it turns to, each with the kick's lift
        // not taken back. Tested only when the rest let the shot go, unless
        // the gate tally wants every gate.
        let blocked = if open || self.gate_tally {
            self.shot_blocked(me, &aimed, error, lift)
        } else {
            None
        };
        if let Some(blocked) = blocked {
            failed |= blocked_gate(blocked);
        }
        let mut wants = false;
        let burst = matches!(gun.handling, Handling::Burst(_));
        if open && blocked.is_none() {
            if self.shot_wanted(gun, now, distance, aim, rng) {
                wants = true;
            } else {
                failed |= gate::PAUSE;
            }
        } else if burst && now < self.burst_until && self.held_ms > 0 {
            failed |= gate::CUT;
        }
        let fire = self.trigger(wants, fire_allowed, now, c, distance, rng);
        if wants && !fire {
            failed |= gate::TRIGGER;
        }
        if fire {
            self.last_fire = Some((track.who(), now));
            self.fire_aim = Some(FireAim {
                at: now,
                distance,
                off_deg: off.to_degrees(),
                within_deg,
                error_deg: error.0.abs().max(error.1.abs()).to_degrees(),
            });
            #[cfg(test)]
            if spraying {
                super::smoke::tally::add(me.team, super::smoke::tally::Event::Spray);
                super::smoke::tally::sprayed(me.place, now);
            }
        }
        self.gates.bits |= failed;
        self.gates.target = Some(track.who());
        if self.tracing {
            self.note_gates(
                me,
                (track.who(), distance, off, within_deg),
                (aimed_in, hip_fits, running, ads, range_m),
                (&aimed, error, lift, blocked),
                fire,
                now,
                aim,
            );
        }
        if self.tracing
            && (fire || self.held_for.is_some_and(|h| h.ready))
            && now >= self.friend_noted + FRIEND_NOTE_MS
        {
            self.note_friend(
                me,
                view,
                &lines,
                (track.who(), distance),
                (gun, super::friends::Sights::of(aimed_in, me.keys, self.strafe_fire)),
                fire,
                now,
                &aim.friends,
            );
        }
        // Down the sights as above; crouched as drawn for its kind; the
        // side step while it stands to aim, for every target with an
        // automatic weapon, for some with a bolt or a marksman rifle, also
        // while it walks (`strafe_walking`, not while it runs); a bolt
        // action stands still for its aimed shot.
        if ads {
            keys |= key::AIM;
        }
        if self.crouch_fire && !running {
            keys |= key::CROUCH;
        }
        let mut strafe = 0;
        // [#24] In a fight the legs stand and the side step goes on; a hide, a push or a throw keeps its legs.
        let stand = matches!(self.gait, Gait::StandStrafe { .. }) && legs.is_none_or(|l| l.why == LegsWhy::Range && !l.run);
        let legs = if stand {
            Some(Legs { at: me.feet, within_m: 1.0, run: false, crouch: me.crouched(), why: LegsWhy::Stand })
        } else {
            legs
        };
        // A bolt action stands still for its aimed shot, and any gun with the view on him stands still while the shot waits for the aim to settle.
        let aiming_the_shot = (bolt && (fire || now + c.fire.bolt_still_ms >= self.next_press)) || (settling && within);
        if !self.strafe_fire || aiming_the_shot {
            self.strafe = 0;
        } else if stand || legs.is_none() || (c.strafe_walking && !running) {
            strafe = self.side_step(now, c, rng);
        }
        if let Some(friend) = friend.filter(|_| !running) {
            strafe = friend.away;
        }
        let strafe_away = friend.is_some_and(|friend| !running && friend.away != 0);
        if aimed_in {
            let can_step = stand || legs.is_none() || (c.strafe_walking && !running);
            let st = &mut self.strafe_stats;
            st.ticks += 1;
            st.stand += u32::from(stand);
            if friend.is_some() && !running {
                st.friend_away += 1;
            } else if !self.strafe_fire {
                st.not_for_target += 1;
            } else if aiming_the_shot {
                st.bolt_holds += 1;
            } else if !can_step {
                st.legs_no_weave += 1;
            } else if strafe != 0 {
                st.running += 1;
            } else {
                st.stopped += 1;
            }
        }
        Override {
            look_at: Some(point),
            error,
            pull,
            fire,
            turn_time_s: Some(settle_s * 0.5),
            keys,
            legs,
            band_m: self.band,
            strafe,
            strafe_away,
            glance: false,
            track: body
                .filter(|_| aimed.sees_body() && c.turn_feed)
                .map(|body| bearing_rate(me, point, body.velocity)),
        }
    }

    /// Whether the trigger is to be pressed for a shot that may go: an automatic weapon in bursts (a burst begun is held to its
    /// end; a new one after its pause), any other weapon at every cycle of its own. False: in the pause between two bursts.
    fn shot_wanted(&mut self, gun: &Gun, now: u32, distance: f32, aim: &AimTuning, rng: &mut Rng) -> bool {
        let c = &aim.combat;
        match gun.handling {
            Handling::Burst(_) if now < self.burst_until => true,
            // A burst as long as the best players hold this kind of
            // weapon at this distance (a tap, more often far).
            Handling::Burst(class) if now >= self.pause_until => {
                let class = c.fire.auto_class(gun, class);
                let hold = c.fire.burst_ms(class, distance, aim.burst_ms, rng);
                self.burst_until = now + hold;
                let pause = c
                    .fire
                    .pause_ms(class, distance, rng)
                    .unwrap_or_else(|| rng.range(aim.pause_ms.0 as f32, aim.pause_ms.1 as f32) as u32);
                self.pause_until = self.burst_until + pause;
                true
            }
            Handling::Burst(_) => false,
            _ => true,
        }
    }

    /// Fire through cover (P4b), with the target lost: a bot that was firing at an enemy goes on, for
    /// [`CombatTuning::fire_through_ms`] after the last part of him it saw, at the place that part was, while the line there
    /// keeps [`CombatTuning::fire_through_share`] of a bullet's damage by the game's rule (the pierce of the bot's cover
    /// tests), no mate stands in it and the burst before was [`CombatTuning::fire_through_gap_ms`] ago. Fire already going
    /// only: the last shot at him is within the same time, and an automatic weapon's burst under way is held to its end, no
    /// new one (it would start a pause that delays the next real shot). Nothing else is read of the hidden enemy: his place is the one
    /// the bot saw, not where he is.
    fn fire_through(
        &mut self,
        (me, view): (&PlaceView, &MatchView),
        now: u32,
        fire_allowed: bool,
        aim: &AimTuning,
        (legs, reload, carry): (Option<Legs>, u32, bool),
        rng: &mut Rng,
    ) -> Option<Override> {
        let c = &aim.combat;
        let rule = self.cover_rule.filter(|_| fire_allowed && c.fire_through_ms > 0 && !self.dry())?;
        let (who, feet, _) = self.sight.last_place(now, c.fire_through_ms)?;
        // Low, at his belly standing (a crouched head): the place a body that ducked behind a crate is, which the part he showed
        // last, over the crate, is not.
        let at = super::aimpoint::Part::Belly.point_of(feet, false, me.eye());
        // Fire already going at him.
        self.last_fire.filter(|&(shot_who, shot_at)| shot_who == who && now <= shot_at + c.fire_through_ms)?;
        // He is hidden: one that is seen is the seen branch's.
        let gun = self.hands().held().gun;
        let distance = at.distance_xz(me.feet);
        let ceiling_m = if gun.handling == Handling::Bolt { c.fire.bolt_range_m.max(aim.range_m) } else { aim.range_m };
        if distance > c.fire_range_m(gun, ceiling_m) || self.hands().held().rounds == 0 {
            return None;
        }
        // The view is on the place and the sights as they were for fire already going.
        let off = off_by(me, at, (0.0, 0.0));
        let aimed_in = self.aim_since.is_some_and(|since| now >= since + gun.aim_ms + c.aim_in_margin_ms);
        if off > aim.fire_within_deg.to_radians() || !(aimed_in || hip_cone_fits(me, gun, self.strafe_fire, distance, c)) {
            return None;
        }
        let keys = reload | if carry { key::AIM } else { 0 };
        let lines = Self::fire_lines(me, view, (at, (0.0, 0.0), 0.0), now, &aim.friends, self.scale);
        let friend = super::friends::blocking(
            me,
            view,
            &lines.0,
            distance,
            aim.friends.spread(gun, super::friends::Sights::of(aimed_in, me.keys, self.strafe_fire)),
            lines.1,
            &aim.friends,
        );
        let share = self.sight.damage_share(me.eye(), at, rule, c.fire_through_share)?;
        if share < c.fire_through_share {
            self.through_stats.refused_share += 1;
            return None;
        }
        if friend.is_some() {
            self.through_stats.refused_friend += 1;
            return None;
        }
        // One burst a gap (any other weapon: one shot): a burst under way goes to its end, a new one waits.
        let burst_gun = matches!(gun.handling, Handling::Burst(_));
        let gap_over = self.through_at.is_none_or(|began| now >= began + c.fire_through_gap_ms);
        if !(burst_gun && now < self.burst_until) && !gap_over {
            self.through_stats.refused_gap += 1;
            return None;
        }
        // A burst of an automatic weapon begun here leaves the pause after it as it was: the pause belongs to fire at a target
        // seen, and would delay the next real shot (the first bench, -2.9 % shots at a target).
        let (began_burst, pause_before) = (burst_gun && self.burst_until <= now, self.pause_until);
        if !self.shot_wanted(gun, now, distance, aim, rng) {
            return None;
        }
        if began_burst {
            self.pause_until = pause_before;
        }
        let fire = self.trigger(true, fire_allowed, now, c, distance, rng);
        if !fire {
            return None;
        }
        if gap_over {
            self.through_at = Some(now);
            self.through_stats.bursts += 1;
        }
        self.through_stats.ticks += 1;
        self.last_fire = Some((who, now));
        Some(Override {
            look_at: Some(at),
            fire,
            keys,
            legs,
            band_m: self.band,
            ..Override::default()
        })
    }

    /// [aimpoint] [`super::aimpoint::Sight::blocked`] with the view (and
    /// the view it turns to) `lift` higher: what keeps the shot from going
    /// clear, if anything.
    fn shot_blocked(
        &self,
        me: &PlaceView,
        aimed: &super::aimpoint::Aim,
        error: (f32, f32),
        lift: f32,
    ) -> Option<super::aimpoint::Blocked> {
        if lift == 0.0 {
            return self.sight.blocked(me, aimed, error);
        }
        let (lowest, highest) = self.scale.limits();
        let lifted = PlaceView {
            pitch: (me.pitch + lift).clamp(lowest, highest),
            ..me.clone()
        };
        self.sight
            .blocked(&lifted, aimed, (error.0, error.1 + lift))
    }

    /// [measure] With `tracing`, a gate note per target at most every
    /// [`GATE_NOTE_MS`]: when the view has been on a seen target (within
    /// its `within_deg` + [`GATE_NOTE_SLACK_DEG`]) for
    /// [`GATE_NOTE_AFTER_MS`] with no shot, or when a gate cut a burst
    /// short. It has every gate's value, not only the first that failed.
    #[allow(clippy::too_many_arguments)]
    fn note_gates(
        &mut self,
        me: &PlaceView,
        (target, distance, off, within_deg): (Place, f32, f32, f32),
        (aimed_in, hip_fits, running, ads, range_m): (bool, bool, bool, bool, f32),
        (aimed, error, lift, blocked): (&super::aimpoint::Aim, (f32, f32), f32, Option<super::aimpoint::Blocked>),
        fired: bool,
        now: u32,
        aim: &AimTuning,
    ) {
        let near = off <= (within_deg + GATE_NOTE_SLACK_DEG).to_radians();
        if fired || !near {
            self.on_since = None;
        } else if self.on_since.is_none_or(|(who, _)| who != target) {
            self.on_since = Some((target, now));
        }
        let bits = self.gates.bits;
        let held = self
            .on_since
            .is_some_and(|(_, since)| now >= since + GATE_NOTE_AFTER_MS);
        let due = self
            .gate_noted
            .is_none_or(|(who, at)| who != target || now >= at + GATE_NOTE_MS);
        if !(due && (held || bits & gate::CUT != 0)) {
            return;
        }
        self.gate_noted = Some((target, now));
        let c = &aim.combat;
        let round = |v: f32| (v * 1000.0).round() / 1000.0;
        let gun = self.hands().held().gun;
        let rounds = self.hands().held().rounds;
        let (ray, ray_m) = match blocked {
            None => (None, None),
            Some(super::aimpoint::Blocked::Hidden) => (Some("hidden"), None),
            Some(super::aimpoint::Blocked::View(m)) => (Some("view"), Some(round(m))),
            Some(super::aimpoint::Blocked::Aim(m)) => (Some("aim"), Some(round(m))),
        };
        self.gate_note = Some(serde_json::json!({
            "t": now, "bot": me.place, "gates": gate::names(bits),
            "why": if bits & gate::CUT != 0 { "cut" } else { "no_shot" },
            "target": target, "distance": round(distance), "range_m": range_m,
            "off_deg": round(off.to_degrees()), "within_deg": round(within_deg),
            "on_ms": self.on_since.map(|(_, since)| now - since),
            "first_shot_in": self.first_shot_at as i64 - now as i64,
            "busy_in": self.busy_until as i64 - now as i64,
            "busy": gate::names(self.busy_why),
            "rounds": rounds,
            "ads": ads, "running": running,
            "aim_ms": self.aim_since.map(|since| now - since),
            "aim_needs_ms": gun.aim_ms + c.aim_in_margin_ms, "aimed_in": aimed_in,
            "hip_fits": hip_fits,
            "pause_in": self.pause_until as i64 - now as i64,
            "burst_in": self.burst_until as i64 - now as i64,
            "friend": self.held_for.map(|h| h.who),
            "shows": aimed.may_fire(), "spray": matches!(aimed, super::aimpoint::Aim::Spray(_)), "looked_ms": self.sight.looked_ms(now),
            "ray": ray, "ray_m": ray_m, "lift": round(lift),
            "error": [round(error.0), round(error.1)],
        }));
    }

    /// [friends] The lines a shot may take (the view now and the view it
    /// turns to, `point` with `error`, each `lift` higher as
    /// [`Self::clear_shot`]'s), and how far ahead of the view's clock
    /// teammates are led for the tick decided, `now`, seconds
    /// ([`super::friends::FriendlyFireTuning::lead`]). `me` and `point` are
    /// what the gun is pointed with: the eye led to the tick
    /// (`CombatTuning::lead_own`) and the aim point led along the target
    /// (`lead_target`), not his body.
    fn fire_lines(
        me: &PlaceView,
        view: &MatchView,
        (point, error, lift): (Vec3, (f32, f32), f32),
        now: u32,
        t: &super::friends::FriendlyFireTuning,
        scale: PitchScale,
    ) -> ([Vec3; 2], f32) {
        use super::friends::along;
        let to = point - me.eye();
        let (lowest, highest) = scale.limits();
        let dirs = [
            along(me.yaw, (me.pitch + lift).clamp(lowest, highest)),
            along(
                yaw_of([to.x, to.z]) + error.0,
                to.y.atan2(to.x.hypot(to.z)) + error.1 + lift,
            ),
        ];
        // At most the encoder's lead past the clock's tick.
        let most = (super::encode::LEAD_TICKS + 1) * survarium::v026g::match_msgs::TICK_MS;
        let ahead_s = if t.lead {
            (now.saturating_sub(view.clock).min(most) + t.lead_extra_ms) as f32 / 1000.0
        } else {
            0.0
        };
        (dirs, ahead_s)
    }

    /// [friends] The trace line of a shot `fired` or held at `target` (and
    /// its distance), with the teammate nearest to blocking it as the bot
    /// had him, if he was within [`FRIEND_NOTE_ROOM_M`]: where the bot had
    /// itself and him, how near the line and its margin, how old his
    /// world was.
    #[allow(clippy::too_many_arguments)]
    fn note_friend(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        (dirs, ahead_s): &([Vec3; 2], f32),
        (target, distance): (Place, f32),
        (gun, sights): (&Gun, super::friends::Sights),
        fired: bool,
        now: u32,
        t: &super::friends::FriendlyFireTuning,
    ) {
        let Some(near) =
            super::friends::closest(me, view, dirs, distance, t.spread(gun, sights), *ahead_s, t)
                .filter(|near| near.room() <= FRIEND_NOTE_ROOM_M)
        else {
            return;
        };
        let him = view.place(near.who);
        let round = |v: f32| (v * 1000.0).round() / 1000.0;
        let at = |v: Vec3| [round(v.x), round(v.y), round(v.z)];
        self.friend_noted = now;
        self.friend_note = Some(serde_json::json!({
            "t": now, "bot": me.place, "friend": if fired { "fired" } else { "held" },
            "clock": view.clock, "target": target, "distance": round(distance),
            "pos": at(me.feet), "yaw": round(me.yaw), "pitch": round(me.pitch),
            "keys": me.keys, "lines": [at(dirs[0]), at(dirs[1])],
            "mate": near.who, "mate_pos": at(near.feet), "mate_bot": him.map(|h| h.bot),
            "mate_velocity": him.map(|h| [round(h.velocity[0]), round(h.velocity[1])]),
            "mate_keys": him.map(|h| h.keys), "mate_seen_at": him.map(|h| h.seen_at),
            "along": round(near.s), "gap": near.gap.map(round), "margin": round(near.margin),
            "touch": near.touch, "held_for": self.held_for.map(|h| h.who),
        }));
    }

    /// The aim's wander, one tick on: a mean-reverting walk whose spread is
    /// `spread_deg` (the skill's `wander_deg`, settled with the time on target
    /// when [`Settle`] says; the pitch's half) and which comes back in about
    /// `wander_s`.
    fn wander_step(&mut self, aim: &AimTuning, spread_deg: f32, rng: &mut Rng) {
        let spread = spread_deg.max(0.0).to_radians();
        let a = (self.tick_ms as f32 / 1000.0 / aim.wander_s.max(0.01)).min(1.0);
        // Uniform shocks (a spread of 1/sqrt(12)) scaled to keep the walk's
        // spread at `spread`.
        let shock = spread * (2.0 * a - a * a).sqrt() * 12f32.sqrt();
        self.wander.0 += -self.wander.0 * a + shock * (rng.unit() - 0.5);
        self.wander.1 += -self.wander.1 * a + 0.5 * shock * (rng.unit() - 0.5);
    }
}

/// [measure] The gate of what kept a shot from going clear.
fn blocked_gate(blocked: super::aimpoint::Blocked) -> u32 {
    match blocked {
        super::aimpoint::Blocked::Hidden => gate::HIDDEN,
        super::aimpoint::Blocked::View(_) => gate::VIEW_RAY,
        super::aimpoint::Blocked::Aim(_) => gate::AIM_RAY,
    }
}

/// How far `body` goes along its speed from the time the view has it for
/// ([`PlaceView::known_at`]) to the tick `now`, at most `most_ms` (and back
/// at most two ticks, for a bot decided past `now`).
fn lead(body: &PlaceView, now: u32, most_ms: u32) -> Vec3 {
    let back = (super::encode::LEAD_TICKS * survarium::v026g::match_msgs::TICK_MS) as i64;
    let ms = (i64::from(now) - i64::from(body.known_at)).clamp(-back, i64::from(most_ms));
    let s = ms as f32 / 1000.0;
    Vec3::new(body.velocity[0] * s, 0.0, body.velocity[1] * s)
}

/// The turn rate, yaw and pitch, radians a second, of the line from
/// `me`'s eye to `point` as `velocity` (the target's, on the ground) and
/// `me`'s own move them: what a turn follows to stay on a steady target.
fn bearing_rate(me: &PlaceView, point: Vec3, velocity: [f32; 2]) -> (f32, f32) {
    const DT: f32 = 0.05;
    let to = point - me.eye();
    let rel = [velocity[0] - me.velocity[0], velocity[1] - me.velocity[1]];
    let next = to + Vec3::new(rel[0] * DT, 0.0, rel[1] * DT);
    let angles = |v: Vec3| (yaw_of([v.x, v.z]), v.y.atan2(v.x.hypot(v.z)));
    if to.x.hypot(to.z) <= 0.3 {
        return (0.0, 0.0);
    }
    let (yaw0, pitch0) = angles(to);
    let (yaw1, pitch1) = angles(next);
    (wrap(yaw1 - yaw0) / DT, (pitch1 - pitch0) / DT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The best players' pause: off or under 20 m it is the flat draw's business; on, far, it is drawn through the quartiles.
    #[test]
    fn the_pause_is_drawn_from_the_best_players_quartiles_far_and_only_when_on() {
        let mut fire = FireTuning::default();
        let mut rng = Rng::new(7);
        fire.burst_pause = None;
        assert_eq!(fire.pause_ms(AutoClass::Rifle, 45.0, &mut rng), None, "off");
        fire.burst_pause = Some(BurstPause::players());
        let pause = fire.burst_pause.clone().unwrap();
        assert_eq!(fire.pause_ms(AutoClass::Rifle, 12.0, &mut rng), None, "near: the flat draw");
        for class in [AutoClass::Rifle, AutoClass::Smg] {
            for distance in [25.0, 60.0] {
                let mut draws: Vec<u32> = (0..4_000).map(|_| fire.pause_ms(class, distance, &mut rng).expect("far")).collect();
                draws.sort_unstable();
                let q = |p: f32| draws[(p * draws.len() as f32) as usize];
                let row = match class {
                    AutoClass::Rifle => &pause.rifle,
                    AutoClass::Smg => &pause.smg,
                }
                .iter()
                .rev()
                .find(|row| distance >= row.from_m)
                .unwrap();
                assert!(draws[0] >= pause.floor_ms && *draws.last().unwrap() <= pause.top_ms, "inside the floor and the top");
                for (p, want) in [(0.25, row.q[0]), (0.5, row.q[1]), (0.75, row.q[2])] {
                    assert!(q(p).abs_diff(want) <= 25, "{class:?} at {distance} m: the {p} quantile {} against {want}", q(p));
                }
            }
        }
    }

    /// The switch is one value: on by default with the players' numbers, an empty object in a tuning patch is the same, `null` is off.
    #[test]
    fn the_burst_pause_is_on_by_default_and_an_empty_patch_is_the_players_numbers() {
        assert_eq!(FireTuning::default().burst_pause, Some(BurstPause::players()));
        let off: FireTuning = serde_json::from_str(r#"{"burst_pause": null}"#).expect("a null burst pause");
        assert_eq!(off.burst_pause, None);
        let fire: FireTuning = serde_json::from_str(r#"{"burst_pause": {}}"#).expect("an empty burst pause");
        assert_eq!(fire.burst_pause, Some(BurstPause::players()));
    }

    /// The gait stands and strafes while the fight is on and for the hold after it, then the route again.
    #[test]
    fn the_gait_stands_for_the_fight_and_its_hold_then_goes_back_to_the_route() {
        let g = Gait::Route;
        assert_eq!(g.next(false, 1_000, 400), Gait::Route);
        let g = g.next(true, 1_000, 400);
        assert_eq!(g, Gait::StandStrafe { until: 1_400 });
        // Fight off, hold running: stays, until unchanged.
        assert_eq!(g.next(false, 1_200, 400), Gait::StandStrafe { until: 1_400 });
        // Fight on again: the hold starts anew.
        assert_eq!(g.next(true, 1_300, 400), Gait::StandStrafe { until: 1_700 });
        // Hold over: back to the route.
        assert_eq!(g.next(false, 1_400, 400), Gait::Route);
    }

    /// A range step across the line goes the same way along it as the straight one and sideways by `m tan(angle)`, to the
    /// side asked, in or back out (`band_cross_deg`).
    #[test]
    fn a_range_step_across_the_line_keeps_its_way_along_it() {
        let (from, to) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 30.0));
        let angle = 45f32.to_radians();
        for (m, along) in [(6.0, 6.0), (-6.0, -6.0)] {
            let straight = step_toward(from, to, m);
            let left = step_across(from, to, m, angle, false);
            let right = step_across(from, to, m, angle, true);
            assert!((left.z - straight.z).abs() < 1e-4 && (right.z - straight.z).abs() < 1e-4, "the way along the line is kept");
            assert!((left.z - along).abs() < 1e-4);
            assert!((left.x.abs() - 6.0).abs() < 1e-3 && (right.x.abs() - 6.0).abs() < 1e-3, "tan 45 = 1: sideways m");
            assert!(left.x * right.x < 0.0, "the two sides are opposite");
        }
        // Angle 0 is the straight step.
        assert_eq!(step_across(from, to, 6.0, 0.0, true), step_toward(from, to, 6.0));
    }

    use crate::v026g::bots::guns;
    use crate::v026g::bots::know::{DangerSpot, Percept};
    use crate::v026g::bots::view::{Mode, Sound};

    /// The combat tuning with the owner's options off (the bots as they were): what a test of an option's effect starts from.
    /// The throw numbers before the habit of the live players (A159): the geometry of the tests below is made for
    /// the 12 m floor, the 18 m wall, the 6 m mate rule and the 4 s lead.
    fn earlier_habit() -> CombatTuning {
        CombatTuning {
            grenade_chance: 0.15,
            grenade_friend_m: 6.0,
            grenade_lead_s: 4.0,
            grenade_land_m: 12.0,
            grenade_wall_m: 18.0,
            grenade_track_chance: 0.08,
            ..CombatTuning::default()
        }
    }

    fn combat_off() -> CombatTuning {
        crate::v026g::bots::tuning::Tuning::all_off().aim.combat
    }

    /// The error angles go with the zoom as the client's look step does: nothing when off, the scope's 0.25 aimed in (halfway
    /// up the aim: 0.625), iron sights only with `All`.
    #[test]
    fn the_error_angles_follow_the_zoom_of_the_sights() {
        let scope = Optic::Scope { zoom: 0.25 };
        let iron = Optic::Iron { zoom: 1.0 / 1.2 };
        assert_eq!(ZoomError::Off.factor(scope, 1.0), 1.0);
        assert_eq!(ZoomError::Scopes.factor(scope, 0.0), 1.0);
        assert!((ZoomError::Scopes.factor(scope, 0.5) - 0.625).abs() < 1e-6);
        assert!((ZoomError::Scopes.factor(scope, 1.0) - 0.25).abs() < 1e-6);
        assert_eq!(ZoomError::Scopes.factor(iron, 1.0), 1.0);
        assert!((ZoomError::All.factor(iron, 1.0) - 1.0 / 1.2).abs() < 1e-6);
        assert!((ZoomError::All.factor(scope, 2.0) - 0.25).abs() < 1e-6, "progress past 1 stays at the full zoom");
    }

    fn body(place: Place, team: u8, feet: Vec3, yaw: f32) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet,
            yaw,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    fn view_of(places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock: 0,
            mode: Mode::Batteries,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: vec![],
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Default::default(),
        }
    }

    fn percept(who: Place, at: Vec3, time: u32, source: Source) -> Percept {
        Percept {
            who,
            at,
            error_m: 0.0,
            time,
            source,
            busy: false,
        }
    }

    fn sense(memory: &mut Memory, percepts: &[Percept], now: u32) {
        memory.update(percepts, now, |_| 300, 3_000, 20_000);
    }

    /// The pitch and yaw that look at `point` from `me`, so the view is on
    /// it.
    fn facing(me: &mut PlaceView, point: Vec3) {
        let eye = me.eye();
        let (dx, dz) = (point.x - eye.x, point.z - eye.z);
        me.yaw = yaw_of([dx, dz]);
        me.pitch = (point.y - eye.y).atan2(dx.hypot(dz));
    }

    #[test]
    fn it_aims_only_at_a_noticed_enemy_and_fires_once_on_target() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), 0.0);
        let view = view_of(vec![me.clone(), enemy]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(9);
        let sight = |time| percept(1, Vec3::new(0.0, 0.0, 20.0), time, Source::Seen);
        sense(&mut memory, &[sight(1_000)], 1_000);
        reflex.think(&me, &memory, 1_000, &aim, 1.0, &mut rng);
        assert_eq!(reflex.target(), None, "not noticed yet");
        // Noticed: it aims; facing the enemy squarely, it fires once the
        // error has settled.
        sense(&mut memory, &[sight(1_300)], 1_300);
        reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
        assert_eq!(reflex.target(), Some(1));
        let mut fired = false;
        for k in 0..20 {
            let now = 1_300 + k * 50;
            sense(&mut memory, &[sight(now)], now);
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
            assert!(o.look_at.is_some());
            fired |= o.fire;
        }
        assert!(fired);
        // The play holds fire: no shot.
        let o = reflex.tick(&me, &view, &memory, 2_400, false, &aim, 1.0, &mut rng);
        assert!(!o.fire);
    }

    /// [friends] A teammate at arm's length in front holds the fire whatever
    /// his angle off the line; tracing, the held shots and the shots fired
    /// past a teammate near the line are noted with where the bot had him.
    #[test]
    fn a_shot_held_or_fired_near_a_teammate_is_noted_with_where_it_had_him() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), 0.0);
        let aim = AimTuning::default();
        let sight = |time| percept(1, Vec3::new(0.0, 0.0, 20.0), time, Source::Seen);
        // (where the teammate stands, fires, held, touching)
        for (mate, fires, touch) in [
            // 1.2 m on, 1.2 m to the side: 45 degrees off, touching.
            (Vec3::new(1.2, 0.0, 1.2), false, true),
            // 1.5 m off the line 10 m on: clear by 0.87 m.
            (Vec3::new(1.5, 0.0, 10.0), true, false),
        ] {
            let view = view_of(vec![me.clone(), enemy.clone(), body(2, 0, mate, 0.0)]);
            let mut memory = Memory::default();
            let mut reflex = Reflex {
                tracing: true,
                ..Reflex::default()
            };
            let mut rng = Rng::new(9);
            sense(&mut memory, &[sight(1_000)], 1_000);
            reflex.think(&me, &memory, 1_000, &aim, 1.0, &mut rng);
            sense(&mut memory, &[sight(1_300)], 1_300);
            reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
            let (mut fired, mut notes, mut away) = (false, Vec::new(), 0);
            for k in 0..30 {
                let now = 1_300 + k * 50;
                sense(&mut memory, &[sight(now)], now);
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                fired |= o.fire;
                // A step away from the teammate is marked (the drive never turns it to the other side).
                if o.strafe_away {
                    assert_ne!(o.strafe, 0);
                    away += 1;
                }
                notes.extend(reflex.take_friend_note());
            }
            assert_eq!(fired, fires, "a teammate at {mate:?}");
            assert_eq!(
                away > 0,
                !fires,
                "a teammate at {mate:?}: {away} steps away"
            );
            assert!(!notes.is_empty(), "a teammate at {mate:?}");
            for note in &notes {
                assert_eq!(note["mate"], 2);
                assert_eq!(note["touch"], touch);
                assert_eq!(note["friend"], if fires { "fired" } else { "held" });
                assert_eq!(note["mate_pos"][0].as_f64().unwrap() as f32, mate.x);
            }
        }
    }

    /// [measure] The gates of a tick name what held the shot: the reaction
    /// first, then a teammate at arm's length, the range, the play's word
    /// and a jump held (the bot's); the gate tally tests every gate and
    /// changes no shot; tracing, a held view on him is noted with them.
    #[test]
    fn the_fire_gates_name_what_held_the_shot_and_change_none() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let aim = AimTuning::default();
        // (enemy 20 m or 100 m on, a teammate touching in front, tally)
        let run = |far: bool, mate: bool, tally: bool| {
            let z = if far { 100.0 } else { 20.0 };
            let enemy = body(1, 1, Vec3::new(0.0, 0.0, z), 0.0);
            let mut places = vec![me.clone(), enemy];
            if mate {
                places.push(body(2, 0, Vec3::new(1.2, 0.0, 1.2), 0.0));
            }
            let view = view_of(places);
            let sight = |time| percept(1, Vec3::new(0.0, 0.0, z), time, Source::Seen);
            let mut memory = Memory::default();
            let mut reflex = Reflex {
                tracing: true,
                gate_tally: tally,
                ..Reflex::default()
            };
            let mut aim = aim.clone();
            aim.range_m = 150.0;
            let mut rng = Rng::new(9);
            sense(&mut memory, &[sight(1_000)], 1_000);
            reflex.think(&me, &memory, 1_000, &aim, 1.0, &mut rng);
            sense(&mut memory, &[sight(1_300)], 1_300);
            reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
            aim.range_m = 80.0;
            let (mut fires, mut gates, mut notes) = (Vec::new(), Vec::new(), Vec::new());
            for k in 0..30 {
                let now = 1_300 + k * 50;
                sense(&mut memory, &[sight(now)], now);
                reflex.outer_gates(now, false, false);
                fires.push(reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng).fire);
                let g = reflex.gates();
                assert_eq!((g.at, g.target), (now, Some(1)));
                gates.push(g.bits);
                notes.extend(reflex.take_gate_note());
            }
            (fires, gates, notes)
        };
        // Clear: the reaction, then shots.
        let (fires, gates, _) = run(false, false, false);
        assert_ne!(gates[0] & gate::REACTION, 0, "{:?}", gate::names(gates[0]));
        assert!(fires.iter().any(|&f| f));
        for (fired, bits) in fires.iter().zip(&gates) {
            if *fired {
                assert_eq!(bits & !(gate::PAUSE | gate::TRIGGER), 0, "{:?}", gate::names(*bits));
            }
        }
        // The gate tally changes no shot.
        assert_eq!(run(false, false, true).0, fires);
        assert_eq!(run(false, true, true).0, run(false, true, false).0);
        // A teammate at arm's length: held for him, and noted so.
        let (fires, gates, notes) = run(false, true, true);
        assert!(fires.iter().all(|&f| !f));
        assert!(gates[10..].iter().all(|bits| bits & gate::FRIEND != 0));
        let note = notes.first().expect("a held view is noted");
        assert_eq!(note["why"], "no_shot");
        assert!(note["gates"].as_array().unwrap().iter().any(|g| g == "friend"));
        assert_eq!(note["friend"], 2);
        assert!(notes.len() <= 2, "one a second: {notes:?}");
        // 100 m on, the range at 80 m: the range.
        let (fires, gates, _) = run(true, false, true);
        assert!(fires.iter().all(|&f| !f));
        assert!(gates[10..].iter().all(|bits| bits & gate::RANGE != 0));
        // The bot's own gates: the play's word; a jump held, by its source.
        let mut reflex = Reflex::default();
        reflex.outer_gates(5_000, true, false);
        assert_eq!(reflex.gates().bits, gate::PLAY);
        reflex.jumped("hop");
        reflex.outer_gates(5_050, false, true);
        assert_eq!(reflex.gates().bits, gate::JUMP_HOP);
        reflex.skipped_tick(5_100);
        reflex.gate_using(5_100);
        assert_eq!(reflex.gates().bits, gate::UNSTICK | gate::USING);
        assert_eq!(gate::names(gate::UNSTICK | gate::USING), ["unstick", "using"]);
    }

    /// The players' hit-run p95 by class, metres (`player_bots_weapons.md`),
    /// with a weapon of each: the defaults of `range_by_weapon`.
    const CLASS_RANGES: [(&str, &str, f32); 5] = [
        ("ak_74u", "rifle", 65.0),
        ("vityaz", "smg", 58.0),
        ("mp_153", "shotgun", 49.0),
        ("vss", "marksman", 74.0),
        ("mosin_nagant", "bolt", 105.0),
    ];

    /// [PR C] With `range_by_weapon` (on), the trigger's range is the
    /// players' hit-run p95 of the class, under the ceiling; without, the
    /// ceiling.
    #[test]
    fn the_fire_range_follows_the_weapon_in_hand() {
        let on = combat_off();
        assert!(on.range_by_weapon);
        let off = CombatTuning {
            range_by_weapon: false,
            ..combat_off()
        };
        let gun = |item| guns::gun(item).unwrap();
        for (item, class, range) in CLASS_RANGES {
            assert_eq!(gun(item).class(), class, "{item}");
            let ceiling = if class == "bolt" { 120.0 } else { 80.0 };
            assert_eq!(on.fire_range_m(gun(item), ceiling), range, "{item}");
            assert_eq!(off.fire_range_m(gun(item), ceiling), ceiling, "{item}");
        }
        // No players' range for the pistols: the ceiling.
        assert_eq!(on.fire_range_m(gun("tt_33"), 80.0), 80.0);
        // Under a ceiling that is lower, the ceiling.
        assert_eq!(on.fire_range_m(gun("mosin_nagant"), 100.0), 100.0);
        // The shotgun's bench variant is a row.
        let short = CombatTuning {
            range_floor_shotgun_m: 35.0,
            ..on
        };
        assert_eq!(short.fire_range_m(gun("mp_153"), 80.0), 35.0);
        assert_eq!(short.fire_range_m(gun("ak_74u"), 80.0), 65.0);
    }

    /// A bot with `item` in hand, an enemy `z` m ahead noticed, the ticks
    /// from 1.3 s on every 50 ms for `ticks` of them (the thinks every
    /// 250 ms): at tick `hit` its health drops. Per tick: whether it fired,
    /// and the gates that failed.
    fn fire_at_range(
        item: &'static str,
        z: f32,
        hit: Option<u32>,
        combat: CombatTuning,
        ticks: u32,
    ) -> Vec<(bool, u32)> {
        let arms: &'static Arms = Box::leak(Box::new(Arms {
            primary: (guns::gun(item).unwrap(), 300),
            pistol: None,
            reserve: 600,
        }));
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, z), 0.0);
        let aim = AimTuning {
            combat,
            ..AimTuning::default()
        };
        // Target choice sees farther than any range (a bolt action's 105 m).
        let choose = AimTuning {
            range_m: 150.0,
            ..aim.clone()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::armed(Some(arms));
        let mut rng = Rng::new(9);
        let mut out = Vec::new();
        for k in 0..ticks {
            let now = 1_000 + k * 50;
            if hit == Some(k) {
                me.health = 0.7;
            }
            let view = view_of(vec![me.clone(), enemy.clone()]);
            sense(&mut memory, &[percept(1, enemy.feet, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &choose, 1.0, &mut rng);
            }
            reflex.outer_gates(now, false, false);
            let fire = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng).fire;
            out.push((fire, reflex.gates().bits));
        }
        out
    }

    /// Where the bot `item` in hand looks at an enemy `z` m ahead moving `speed` m/s along x, as the view has him this tick, with the
    /// flight lead `on` or off: the last of a few ticks' aim point.
    fn aim_at_strafer(item: &'static str, z: f32, speed: f32, on: bool) -> Vec3 {
        let arms: &'static Arms = Box::leak(Box::new(Arms {
            primary: (guns::gun(item).unwrap(), 300),
            pistol: None,
            reserve: 600,
        }));
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let aim = AimTuning {
            combat: CombatTuning { lead_flight: on, lead_drop: on, ..CombatTuning::default() },
            range_m: 150.0,
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::armed(Some(arms));
        let mut rng = Rng::new(9);
        let mut at = Vec3::ZERO;
        for k in 0..12 {
            let now = 1_000 + k * 50;
            let mut enemy = body(1, 1, Vec3::new(0.0, 0.0, z), 0.0);
            enemy.velocity = [speed, 0.0];
            enemy.known_at = now;
            let view = view_of(vec![me.clone(), enemy.clone()]);
            sense(&mut memory, &[percept(1, enemy.feet, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, 1.0, &mut rng);
            }
            reflex.outer_gates(now, false, false);
            at = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng).look_at.unwrap_or(at);
        }
        at
    }

    /// `lead_flight`: the aim at a strafer goes along his velocity by the ammunition's flight time and up by its drop; the
    /// switch off leaves the aim where it was. Fast and slow ammunition differ as the notes' table says.
    #[test]
    fn the_aim_leads_a_strafer_for_the_flight_of_the_ammunition() {
        let (z, speed) = (40.0, 2.6);
        let off = aim_at_strafer("ump45", z, speed, false);
        let slow = aim_at_strafer("ump45", z, speed, true) - off;
        // .45 ACP (260 m/s): 168 ms to 40 m, a bit more to the place he reaches: 2.6 m/s x ~0.17 s, and 0.35 m of drop at 60 m
        // is 0.14 m here.
        assert!((slow.x - 0.45).abs() < 0.05, "{slow:?}");
        assert!((slow.y - 0.14).abs() < 0.03 && slow.z.abs() < 0.01, "{slow:?}");
        // The rifle's bullet (5.45x39, 800 m/s) is in the air a third of that.
        let fast = aim_at_strafer("ak_74u", z, speed, true) - aim_at_strafer("ak_74u", z, speed, false);
        assert!(fast.x > 0.1 && fast.x < slow.x / 2.0 && fast.y < slow.y / 4.0, "{fast:?} against {slow:?}");
    }

    /// X1: each class fires out to its range and not past it: a target 3 m
    /// inside is fired at, one 3 m beyond is held back by the range alone;
    /// the flag off keeps the 80 m (the bolt's 120 m) ceiling.
    #[test]
    fn each_class_stops_firing_at_its_range() {
        let on = combat_off();
        let off = CombatTuning {
            range_by_weapon: false,
            ..combat_off()
        };
        for (item, class, range) in CLASS_RANGES {
            let inside = fire_at_range(item, range - 3.0, None, on.clone(), 100);
            assert!(inside.iter().any(|t| t.0), "{class} at {} m", range - 3.0);
            let past = fire_at_range(item, range + 3.0, None, on.clone(), 100);
            assert!(past.iter().all(|t| !t.0), "{class} at {} m", range + 3.0);
            assert!(
                past[30..].iter().all(|t| t.1 & gate::RANGE != 0),
                "{class}: {:?}",
                gate::names(past[60].1)
            );
            // The same target with the flag off, where the ceiling reaches.
            if range + 3.0 <= 80.0 {
                let old = fire_at_range(item, range + 3.0, None, off.clone(), 100);
                assert!(old.iter().any(|t| t.0), "{class} off");
                assert!(old.iter().all(|t| t.1 & gate::RANGE == 0), "{class} off");
            }
        }
    }

    /// X1's switches are named by their path in the tuning (`/aim/combat/<name>`), which is how the bench's
    /// `SRP_BENCH_TUNING` patch and `SRP_BENCH_COMBAT` reach them: the names are in the tuning's JSON, and a value
    /// set there parses back and passes the limits: the flag off, and the shotgun's 35 m row.
    #[test]
    fn the_fire_range_switches_are_reached_by_their_tuning_path() {
        use crate::v026g::bots::tuning::Tuning;
        let with = |name: &str, value: serde_json::Value| {
            let mut tuning = serde_json::to_value(Tuning::default()).unwrap();
            let slot = tuning
                .pointer_mut(&format!("/aim/combat/{name}"))
                .unwrap_or_else(|| panic!("{name} is a name of the combat tuning"));
            *slot = value;
            let tuning: Tuning = serde_json::from_value(tuning).expect("the value parses");
            tuning.validate().expect("the value is within the limits");
            tuning
        };
        assert!(Tuning::default().aim.combat.range_by_weapon);
        assert!(!with("range_by_weapon", false.into()).aim.combat.range_by_weapon);
        let short = with("range_floor_shotgun_m", 35.into());
        assert_eq!(short.aim.combat.range_floor_shotgun_m, 35.0);
        assert_eq!(short.aim.combat.range_floor_rifle_m, 65.0);
    }

    /// X1: a bot hit from beyond its range answers for `hit_shot_ms`, out to
    /// the ceiling, and holds its fire again after; hit at the ceiling's
    /// edge it is no farther; the same run twice is the same run.
    #[test]
    fn a_bot_hit_from_beyond_its_range_answers_for_a_while() {
        let on = CombatTuning::default();
        // A rifle at 72 m: 7 m past its 65, 8 m inside the 80 m ceiling.
        let held = fire_at_range("ak_74u", 72.0, None, on.clone(), 160);
        assert!(held.iter().all(|t| !t.0));
        // Hit at tick 40 (3 s: tick 100): answered from then, then not.
        let hit = fire_at_range("ak_74u", 72.0, Some(40), on.clone(), 160);
        assert_eq!(hit[..40], held[..40]);
        let answered: Vec<usize> = (0..160).filter(|&k| hit[k].0).collect();
        assert!(!answered.is_empty(), "{:?}", hit.iter().map(|t| t.1).collect::<Vec<_>>());
        let window = on.hit_shot_ms / 50;
        assert!(answered.iter().all(|&k| (40..=40 + window as usize + 5).contains(&k)), "{answered:?}");
        assert!(hit[40 + window as usize + 10..].iter().all(|t| !t.0));
        // Past the ceiling too (90 m): no answer, the ceiling holds.
        let far = fire_at_range("ak_74u", 90.0, Some(40), on.clone(), 160);
        assert!(far.iter().all(|t| !t.0));
        // Deterministic.
        assert_eq!(hit, fire_at_range("ak_74u", 72.0, Some(40), on, 160));
    }

    /// [PR B] With `aim_carry`, the aim stays pressed while the target is
    /// lost a moment, and its time goes on: seen again, the sights are up
    /// at once; without it the aim is let go and starts again (and the
    /// gates name that restart).
    #[test]
    fn the_aim_is_held_through_a_target_lost_a_moment() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), 0.0);
        let view = view_of(vec![me.clone(), enemy]);
        let sight = |time| percept(1, Vec3::new(0.0, 0.0, 30.0), time, Source::Seen);
        // (carry) -> (AIM pressed while lost, the gates seen again)
        let run = |carry: bool| {
            let aim = AimTuning {
                combat: CombatTuning {
                    aim_carry: carry,
                    ads_near_chance: 1.0,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(9);
            sense(&mut memory, &[sight(1_000)], 1_000);
            reflex.think(&me, &memory, 1_000, &aim, 1.0, &mut rng);
            sense(&mut memory, &[sight(1_300)], 1_300);
            reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
            let mut now = 1_300;
            // Seen for a second: the sights come up.
            while now < 2_300 {
                sense(&mut memory, &[sight(now)], now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                now += 50;
            }
            // Out of sight for 500 ms (the track goes stale after 400).
            let mut aimed_while_lost = true;
            let lost_at = now;
            while now < lost_at + 500 {
                sense(&mut memory, &[], now);
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                if reflex.target().is_none() {
                    aimed_while_lost &= o.keys & key::AIM != 0;
                }
                now += 50;
            }
            // Seen again: the gates of the next ticks.
            let mut bits = 0;
            for _ in 0..12 {
                sense(&mut memory, &[sight(now)], now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                bits |= reflex.gates().bits;
                now += 50;
            }
            (aimed_while_lost, bits)
        };
        let (held, bits) = run(true);
        assert!(held);
        assert_eq!(bits & (gate::AIMING_AGAIN | gate::AIMING_IN), 0, "{:?}", gate::names(bits));
        let (held, bits) = run(false);
        assert!(!held);
        assert_ne!(bits & gate::AIMING_AGAIN, 0, "{:?}", gate::names(bits));
    }

    /// [PR B] With `reacquire_keep`, a target taken again a moment after
    /// he was lost keeps the reaction already waited and the error settled:
    /// no second reaction; without it a fresh reaction is waited.
    #[test]
    fn a_target_taken_again_a_moment_after_keeps_what_it_had_of_him() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 30.0), 0.0);
        let view = view_of(vec![me.clone(), enemy]);
        let sight = |time| percept(1, Vec3::new(0.0, 0.0, 30.0), time, Source::Seen);
        // (keep) -> (gates once taken again, the error then)
        let run = |keep: bool| {
            let aim = AimTuning {
                combat: CombatTuning {
                    reacquire_keep: keep,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(9);
            sense(&mut memory, &[sight(1_000)], 1_000);
            reflex.think(&me, &memory, 1_000, &aim, 1.0, &mut rng);
            sense(&mut memory, &[sight(1_300)], 1_300);
            reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
            let mut now = 1_300;
            while now < 3_300 {
                sense(&mut memory, &[sight(now)], now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                now += 50;
            }
            let settled = reflex.error;
            let lost_at = now;
            while now < lost_at + 500 {
                sense(&mut memory, &[], now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                now += 50;
            }
            assert_eq!(reflex.target(), None);
            let mut bits = 0;
            let mut again = None;
            for _ in 0..20 {
                sense(&mut memory, &[sight(now)], now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                if reflex.target() == Some(1) {
                    again.get_or_insert(reflex.error);
                    bits |= reflex.gates().bits;
                }
                now += 50;
            }
            (bits, settled, again.expect("taken again"))
        };
        let (bits, settled, again) = run(true);
        assert_eq!(bits & (gate::REACTION | gate::REACTION_AGAIN), 0, "{:?}", gate::names(bits));
        assert_eq!(again, settled);
        let (bits, _, _) = run(false);
        assert_ne!(bits & gate::REACTION_AGAIN, 0, "{:?}", gate::names(bits));
    }

    /// [PR B] `reacquire_keep` skips the noticing only for its own last
    /// target: another enemy who comes into sight then still waits his.
    #[test]
    fn another_enemy_in_sight_still_waits_his_noticing() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let first = Vec3::new(0.0, 0.0, 30.0);
        let other = Vec3::new(5.0, 0.0, 25.0);
        let mut view = view_of(vec![me.clone(), body(1, 1, first, 0.0), body(2, 1, other, 0.0)]);
        let aim = AimTuning {
            combat: CombatTuning {
                reacquire_keep: true,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(9);
        let mut now = 1_000;
        while now < 3_000 {
            sense(&mut memory, &[percept(1, first, now, Source::Seen)], now);
            if now % 250 == 0 {
                reflex.think(&me, &memory, now, &aim, 1.0, &mut rng);
            }
            view.clock = now;
            reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
            now += 50;
        }
        assert_eq!(reflex.target(), Some(1));
        // He goes out of sight; a moment later the other comes into it.
        let lost_at = now;
        while now < lost_at + 500 {
            sense(&mut memory, &[], now);
            view.clock = now;
            reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
            now += 50;
        }
        assert_eq!(reflex.target(), None);
        let came = now;
        sense(&mut memory, &[percept(2, other, now, Source::Seen)], now);
        reflex.think(&me, &memory, now, &aim, 1.0, &mut rng);
        view.clock = now;
        reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
        assert_eq!(reflex.target(), None, "not before his noticing");
        assert!(memory.seen(2, now, now).is_none());
        // Once noticed, he is taken.
        while memory.seen(2, now, now).is_none() {
            now += 50;
            sense(&mut memory, &[percept(2, other, now, Source::Seen)], now);
            assert!(now < came + 2_000);
        }
        view.clock = now;
        reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
        assert_eq!(reflex.target(), Some(2));
    }

    #[test]
    fn no_aim_and_no_shot_without_a_seen_track() {
        // Heard, told and cheated enemies all about, never seen: never a
        // shot, never an aim with an error, never a spot or a grenade.
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemies: Vec<PlaceView> = (1..4)
            .map(|p| body(p, 1, Vec3::new(p as f32 * 4.0, 0.0, 15.0), 0.0))
            .collect();
        let mut places = vec![me.clone()];
        places.extend(enemies.iter().cloned());
        let view = view_of(places);
        let aim = AimTuning {
            combat: CombatTuning {
                grenade_chance: 1.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(3);
        for k in 0..200u32 {
            let now = 1_000 + k * 50;
            let percepts = [
                percept(1, enemies[0].feet, now, Source::Heard),
                percept(2, enemies[1].feet, now, Source::Told { by: 5 }),
                percept(
                    3,
                    enemies[2].feet,
                    now,
                    Source::Cheat(crate::v026g::bots::know::CheatKind::Radar),
                ),
            ];
            sense(&mut memory, &percepts, now);
            if k % 6 == 0 {
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            assert!(!o.fire, "{o:?}");
            assert_eq!(o.keys & key::SPOT, 0);
            assert!(reflex.target().is_none());
            // An aim with an error is only a grenade's, and only at the
            // place the heard enemy was sensed staying at: never at the
            // told or the cheated one.
            if o.error != (0.0, 0.0) || o.keys & key::GRENADE != 0 {
                let at = o.look_at.unwrap();
                assert!(at.distance_xz(enemies[0].feet) < 0.5, "{o:?}");
            }
        }
    }

    #[test]
    fn no_first_shot_before_noticing_and_the_reaction() {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(0.0, 0.0, 20.0);
        let enemy = body(1, 1, at, 0.0);
        facing(&mut me, enemy.chest());
        let view = view_of(vec![me.clone(), enemy.clone()]);
        for (skill, seed) in [(0.0, 1), (0.5, 2), (1.0, 3)] {
            let aim = AimTuning {
                fire_within_deg: 180.0,
                ..AimTuning::default()
            };
            let reaction = mix(aim.reaction_ms, skill) * 0.85;
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(seed);
            let mut first = None;
            for k in 0..60u32 {
                let now = 1_000 + k * 50;
                sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
                if k % 5 == 0 {
                    reflex.think_with(
                        &me,
                        &Situation::of(&view),
                        &memory,
                        now,
                        &aim,
                        skill,
                        &mut rng,
                    );
                }
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, skill, &mut rng);
                if o.fire && first.is_none() {
                    first = Some(now);
                }
            }
            // Seen at 1 000, noticed 300 ms on, then the reaction.
            let first = first.expect("it fires in the end");
            assert!(
                first as f32 >= 1_300.0 + reaction,
                "skill {skill}: {first} before {}",
                1_300.0 + reaction
            );
        }
        // A weaker bot reacts slower.
        let aim = AimTuning::default();
        assert!(mix(aim.reaction_ms, 0.0) > mix(aim.reaction_ms, 1.0));
    }

    #[test]
    fn the_aim_error_never_settles_to_zero() {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(0.0, 0.0, 20.0);
        let enemy = body(1, 1, at, 0.0);
        facing(&mut me, enemy.chest());
        let view = view_of(vec![me.clone(), enemy]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(4);
        let mut largest: f32 = 0.0;
        for k in 0..400u32 {
            let now = 1_000 + k * 50;
            sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think(&me, &memory, now, &aim, 1.0, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
            if k > 200 {
                largest = largest.max(o.error.0.abs());
            }
        }
        assert!(largest > 0.1_f32.to_radians(), "{largest}");
    }

    /// A duel as a match gives it: an enemy comes into sight 20 m ahead at
    /// 1 s, strafing 2.5 m/s in runs of 450 ms; the bot thinks every
    /// 200-400 ms with the sense's noticing delay, and its motor turns the
    /// view. What came out: the time from sight to the first shot, ms; the
    /// view's yaw off the enemy's chest at every tick of fire, degrees; and
    /// the ticks of fire with AIM held, of all ticks of fire.
    fn duel_numbers(aim: &AimTuning, skill: f32, seed: u64) -> (Option<u32>, Vec<f32>, (u32, u32)) {
        use super::super::motor::{Feel, Motor, Pose};
        use super::super::sense::notice_ms;
        use super::super::tuning::SenseTuning;
        let senses = SenseTuning::default();
        let mut rng = Rng::new(seed);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut me = body(0, 0, Vec3::ZERO, rng.range(-0.3, 0.3));
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let appear = 1_000;
        let mut next_think = 800 + rng.range(0.0, 400.0) as u32;
        let (mut first, mut errors, mut aimed) = (None, Vec::new(), (0, 0));
        let (mut x, mut dir, mut turn_at) = (0.0f32, 1.0f32, appear + 450);
        for k in 0..160u32 {
            let now = 800 + k * 50;
            if now >= turn_at {
                dir = -dir;
                turn_at = now + 450;
            }
            if now >= appear {
                x += dir * 2.5 * 0.05;
            }
            let mut enemy = body(1, 1, Vec3::new(x, 0.0, 20.0), 0.0);
            enemy.velocity = [dir * 2.5, 0.0];
            // The view has him as he is at the tick (no lead to make).
            enemy.known_at = now;
            let view = view_of(vec![me.clone(), enemy.clone()]);
            if now >= next_think {
                next_think = now + rng.range(200.0, 400.0) as u32;
                let factor = mix(aim.notice, skill);
                let percepts: Vec<Percept> = if now >= appear {
                    vec![Percept {
                        busy: true,
                        ..percept(1, enemy.feet, now, Source::Seen)
                    }]
                } else {
                    vec![]
                };
                let feet = me.feet;
                memory.update(
                    &percepts,
                    now,
                    |p: &Percept| notice_ms(&senses, p.at.distance_xz(feet), p.busy, factor),
                    3_000,
                    20_000,
                );
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    aim,
                    skill,
                    &mut rng,
                );
            }
            let o = reflex.tick(&me, &view, &memory, now, true, aim, skill, &mut rng);
            if let Some(p) = o.look_at {
                let eye = me.eye();
                let (dx, dz) = (p.x - eye.x, p.z - eye.z);
                let want = (
                    yaw_of([dx, dz]) + o.error.0,
                    (p.y - eye.y).atan2(dx.hypot(dz)) + o.error.1,
                );
                motor.turn_time = o.turn_time_s;
                let pose = Pose {
                    feet: me.feet,
                    yaw: me.yaw,
                    pitch: me.pitch,
                    speed: 0.0,
                    ..Default::default()
                };
                let d = motor.aim_still(&pose, want);
                me.yaw = wrap(me.yaw + d.turn.0);
                me.pitch += d.turn.1;
            }
            if o.fire {
                first.get_or_insert(now - appear);
                let (eye, chest) = (me.eye(), enemy.chest());
                let bearing = yaw_of([chest.x - eye.x, chest.z - eye.z]);
                errors.push(wrap(bearing - me.yaw).abs().to_degrees());
                aimed.1 += 1;
                if o.keys & key::AIM != 0 {
                    aimed.0 += 1;
                }
            }
        }
        (first, errors, aimed)
    }

    /// How far the view trails a target crossing the line of fire at a steady `speed` m/s, `distance` m ahead: the mean over the
    /// ticks from 2 s on of the view after the tick's turn against the line to his chest, metres at him (positive: behind him),
    /// with the motor following the aim's turn rate (`feed`: `Motor::track`, as `drive.rs` sets it) or not. The aim error and
    /// the wander are taken out, the picture is exact, so what is left is the pipeline's own: the reflex's aim point, the
    /// feed, the motor's ease.
    fn steady_trail_m(speed: f32, distance: f32, feed: bool, skill: f32) -> (f32, f32) {
        use super::super::motor::{Feel, Motor, Pose};
        let aim = AimTuning { error_deg: (0.0, 0.0), wander_deg: (0.0, 0.0), ..AimTuning::default() };
        let mut rng = Rng::new(3);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let (mut trail, mut n, mut before) = (0.0f32, 0u32, 0.0f32);
        for k in 0..120u32 {
            let now = 1_000 + k * 50;
            let x = speed * (k as f32) * 0.05 - 2.0;
            let mut enemy = body(1, 1, Vec3::new(x, 0.0, distance), 0.0);
            enemy.velocity = [speed, 0.0];
            enemy.known_at = now;
            let view = view_of(vec![me.clone(), enemy.clone()]);
            sense(&mut memory, &[percept(1, enemy.feet, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, skill, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, skill, &mut rng);
            let Some(p) = o.look_at else { continue };
            let eye = me.eye();
            let (dx, dz) = (p.x - eye.x, p.z - eye.z);
            let want = (yaw_of([dx, dz]) + o.error.0, (p.y - eye.y).atan2(dx.hypot(dz)) + o.error.1);
            motor.turn_time = o.turn_time_s;
            motor.track = o.track.filter(|_| feed);
            let pose = Pose { feet: me.feet, yaw: me.yaw, pitch: me.pitch, speed: 0.0, ..Default::default() };
            let pre = me.yaw;
            let d = motor.aim_still(&pose, want);
            me.yaw = wrap(me.yaw + d.turn.0);
            me.pitch += d.turn.1;
            if k >= 20 {
                let chest = enemy.chest();
                let bearing = yaw_of([chest.x - eye.x, chest.z - eye.z]);
                let hit = |yaw: f32| wrap(yaw - bearing) * distance * speed.signum();
                trail += hit(me.yaw);
                before += hit(pre);
                n += 1;
            }
        }
        (trail / n.max(1) as f32, before / n.max(1) as f32)
    }

    /// Prints the steady strafer's trail (`cargo test -p match-server steady_strafer_trail -- --ignored --nocapture`): the view after
    /// the tick's turn, and the view before it (what a shot released ahead of the turn would go along).
    #[test]
    #[ignore]
    fn steady_strafer_trail() {
        for skill in [0.0, 1.0] {
            for distance in [20.0, 50.0] {
                for feed in [true, false] {
                    let (after, before) = steady_trail_m(2.1, distance, feed, skill);
                    println!("skill {skill} {distance} m feed {feed}: view trails by {after:+.3} m after the turn, {before:+.3} m before it");
                }
            }
        }
    }

    /// The mirror of the kick counts the shots the client's weapon fires: none while the weapon is busy (a reload, a switch) or inside the
    /// last shot's clip (`mirror_gated`), and every key tick down when it is off.
    #[test]
    fn the_mirror_counts_only_the_shots_the_weapon_can_fire() {
        let shots_at = |gated: bool, busy_until: u32, ready_at: f32, now: u32| {
            let mut reflex = Reflex::default();
            reflex.gate_mirror(gated);
            // The first tick takes the weapon in hand (and clears the clip), so the state is set after it.
            reflex.sent(now - 50, 0, 0.0);
            reflex.busy_until = busy_until;
            reflex.ready_at = ready_at;
            reflex.sent(now, key::FIRE, 0.0);
            reflex.seen().1
        };
        // Free: a shot, gated or not.
        assert_eq!(shots_at(true, 0, 0.0, 1_000), 1);
        assert_eq!(shots_at(false, 0, 0.0, 1_000), 1);
        // A reload under way (or the switch): the client starts no shot; the raw key counted one.
        assert_eq!(shots_at(true, 1_200, 0.0, 1_000), 0);
        assert_eq!(shots_at(false, 1_200, 0.0, 1_000), 1);
        // Inside the last shot's clip: none until it is over, and then one.
        assert_eq!(shots_at(true, 0, 1_100.0, 1_050), 0);
        assert_eq!(shots_at(true, 0, 1_100.0, 1_100), 1);
        assert_eq!(shots_at(false, 0, 1_100.0, 1_050), 1);
    }

    /// A counted shot closes the weapon for its fire clip and the bolt after it, as the client's does.
    #[test]
    fn a_counted_shot_keeps_the_next_until_its_cycle_is_over() {
        let mut reflex = Reflex::default();
        reflex.gate_mirror(true);
        reflex.sent(950, 0, 0.0);
        reflex.sent(1_000, key::FIRE, 0.0);
        let gun = reflex.hands().held().gun;
        assert_eq!(reflex.seen().1, 1);
        assert!((reflex.ready_at - (1_000.0 + gun.shot_ms() + gun.chamber_ms as f32)).abs() < 1e-3);
    }

    /// A standing target `distance` m ahead and a burst on him, the way the bench stand-in runs it (the bot's mirror of the kick in
    /// the reflex, the game's kick in `truth`: the middle of every range with `random` false, drawn with it), the view as `drive.rs`
    /// builds it and the turn as it sends it (elevation, converted at the view with `turn_for`, the pull a `rise`). Returns the mean
    /// height of the view at the tick's shots over the line to his chest, degrees, and over the aim point the reflex gave, and the
    /// shots counted. Positive: high.
    fn kicked_height_deg(skill: f32, distance: f32, scale: PitchScale, random: bool) -> (f32, f32, u32) {
        let aim = AimTuning { error_deg: (0.0, 0.0), wander_deg: (0.0, 0.0), ..AimTuning::default() };
        let mut rng = Rng::new(7);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, distance), 0.0);
        let view = view_of(vec![me.clone(), enemy.clone()]);
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut truth = Recoil::in_hand(reflex.hands().held().gun.item);
        let mut cadence = Cadence::default();
        let (interval_ms, auto) = (truth.kick().interval_s() * 1000.0, reflex.hands().held().gun.auto);
        let mut draw = Rng::new(11);
        let mut base = 0.0f32;
        let (mut chest_sum, mut aim_sum, mut n) = (0.0f32, 0.0f32, 0u32);
        for k in 0..160u32 {
            let now = 1_000 + k * 50;
            sense(&mut memory, &[percept(1, enemy.feet, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, skill, &mut rng);
            }
            truth.update(now);
            let truth_before = truth.y();
            let mirror = reflex.weapon_tick(&me, now, scale);
            let before = Pitch::view(base, mirror);
            me.pitch = before.elevation(scale);
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, skill, &mut rng);
            let Some(p) = o.look_at else { continue };
            let eye = me.eye();
            let (dx, dz) = (p.x - eye.x, p.z - eye.z);
            let want = (p.y - eye.y).atan2(dx.hypot(dz)) + o.error.1;
            let mut turn = want - me.pitch;
            turn += Pitch::of_elevation(me.pitch, scale).rise(o.pull, scale);
            let units = before.turn_for(scale, turn);
            // The game: the update came first (above); the turn takes the downward part off the kick; then the shots.
            let taken = truth.turn(units);
            base += units + taken;
            let keys = if o.fire { key::FIRE } else { 0 };
            if o.fire {
                let view_el = Pitch::view(base, truth.y()).elevation(scale);
                let chest = enemy.chest();
                let to_chest = (chest.y - eye.y).atan2(chest.x.hypot(chest.z - eye.z));
                let to_aim = (p.y - eye.y).atan2(dx.hypot(dz));
                chest_sum += view_el - to_chest;
                aim_sum += view_el - to_aim;
                n += 1;
            }
            reflex.sent(now, keys, units);
            cadence.tick(now, o.fire, auto, interval_ms, |at| {
                if random {
                    let top = truth.kick().top(draw.unit());
                    let side = truth.kick().side(draw.unit(), draw.unit());
                    truth.shot(at, top, side);
                } else {
                    truth.mean_shot(at);
                }
            });
            let _ = truth_before;
        }
        let n_f = n.max(1) as f32;
        ((chest_sum / n_f).to_degrees(), (aim_sum / n_f).to_degrees(), n)
    }

    /// Prints the burst's height (`cargo test -p match-server kicked_height -- --ignored --nocapture`): the register's #28 residual.
    #[test]
    #[ignore]
    fn kicked_height() {
        for scale in [PitchScale::Radian, PitchScale::Client] {
            for skill in [0.0, 0.75, 1.0] {
                for distance in [20.0, 30.0, 52.0] {
                    for random in [false, true] {
                        let (chest, aimed, n) = kicked_height_deg(skill, distance, scale, random);
                        println!("{scale:?} skill {skill} {distance} m random {random}: {chest:+.3} deg over the chest, {aimed:+.3} over the aim point ({n} shots)");
                    }
                }
            }
        }
    }

    fn median(mut v: Vec<f32>) -> f32 {
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(f32::NAN)
    }

    /// Prints the bot's duel numbers by skill (`cargo test -p match-server
    /// duel_numbers_by_skill -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn duel_numbers_by_skill() {
        let aim = AimTuning::default();
        for skill in [0.0, 0.25, 0.5, 0.6, 0.75, 0.85, 1.0] {
            let (mut firsts, mut errors) = (Vec::new(), Vec::new());
            for seed in 0..60 {
                let (first, e, _) = duel_numbers(&aim, skill, seed);
                firsts.extend(first.map(|f| f as f32));
                errors.extend(e);
            }
            let q = |v: &[f32], at: f32| {
                let mut v = v.to_vec();
                v.sort_by(f32::total_cmp);
                v.get(((v.len() as f32 * at) as usize).min(v.len().saturating_sub(1)))
                    .copied()
                    .unwrap_or(f32::NAN)
            };
            println!(
                "skill {skill}: first shot {:.0} / {:.0} / {:.0} ms (q1 / median / q3, least {:.0}, n={}), \
                 yaw off while firing {:.2} / {:.2} / {:.2} deg (n={})",
                q(&firsts, 0.25),
                median(firsts.clone()),
                q(&firsts, 0.75),
                q(&firsts, 0.0),
                firsts.len(),
                q(&errors, 0.25),
                median(errors.clone()),
                q(&errors, 0.75),
                errors.len()
            );
        }
    }

    /// The duel against the top players' numbers (`TOP-PLAYERS.md`, 10
    /// recorded matches): sight to first shot 400 / 650 / 1050 ms
    /// (quartiles), the yaw off the chest while firing about 1.0° once the
    /// replay's own error is taken out (1.5° measured). The best bot is
    /// their best quarter (the owner, 2026-10-23), still a human's: never
    /// a shot within 250 ms of the sighting, never an error of 0; a weaker
    /// one is slower and looser.
    #[test]
    fn the_duel_numbers_are_a_top_players_at_the_best_skill() {
        // The old kit: these rules are those of the kit before the owner's choices, which are the defaults now.
        let _kit = survarium::v026g::loadouts::KitSwitches::OFF.scope();
        let aim = AimTuning::default();
        let numbers = |skill: f32| {
            let (mut firsts, mut errors) = (Vec::new(), Vec::new());
            for seed in 0..60 {
                let (first, e, _) = duel_numbers(&aim, skill, seed);
                firsts.extend(first.map(|f| f as f32));
                errors.extend(e);
            }
            let least = firsts.iter().copied().fold(f32::MAX, f32::min);
            let tightest = errors.iter().copied().fold(f32::MAX, f32::min);
            (
                firsts.len(),
                median(firsts),
                median(errors),
                least,
                tightest,
            )
        };
        let (n, first, error, least, tightest) = numbers(1.0);
        assert_eq!(n, 60, "every duel ends in a shot");
        assert!((400.0..=650.0).contains(&first), "{first}");
        assert!(least >= 250.0, "a human's reaction: {least}");
        assert!((0.4..=1.0).contains(&error), "{error}");
        assert!(tightest > 0.0, "never exact");
        let (_, slow, loose, _, _) = numbers(0.0);
        assert!(slow > first && loose > error, "{slow} {loose}");
    }

    /// A bot of `role` facing an enemy `distance` ahead from 1 s for
    /// `ticks` ticks, at skill 1, with the view kept on the aim: each
    /// tick's override.
    fn facing_fight(
        role: &str,
        distance: f32,
        aim: &AimTuning,
        ticks: u32,
    ) -> Vec<(u32, Override)> {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(0.0, 0.0, distance);
        let enemy = body(1, 1, at, 0.0);
        facing(&mut me, enemy.chest());
        let view = view_of(vec![me.clone(), enemy]);
        let mut memory = Memory::default();
        let mut reflex = Reflex::armed(Arms::of_test_role(role));
        let mut rng = Rng::new(11);
        let mut out = Vec::new();
        for k in 0..ticks {
            let now = 1_000 + k * 50;
            sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, aim, 1.0, &mut rng);
            }
            out.push((
                now,
                reflex.tick(&me, &view, &memory, now, true, aim, 1.0, &mut rng),
            ));
        }
        out
    }

    /// Runs of `on` in the ticks: (start, length ms).
    fn spans(ticks: &[(u32, Override)], on: impl Fn(&Override) -> bool) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        let mut start = None;
        for (now, o) in ticks {
            match (on(o), start) {
                (true, None) => start = Some(*now),
                (false, Some(s)) => {
                    out.push((s, now - s));
                    start = None;
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn an_automatic_weapon_fires_in_the_top_players_bursts() {
        // Top players with a rifle within 20 m (6+ matches of 2026-10-16..
        // 22): holds of 400-1200 ms, taps (< 250 ms) 16 %; pauses 150 /
        // 300 / 600 ms.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        let row = aim.combat.fire.rifle[0];
        // 24 s: the AK-74U's 120 rounds, four magazines (then the pistol).
        let ticks = facing_fight("support", 18.0, &aim, 480);
        let bursts = spans(&ticks, |o| o.fire);
        assert!(bursts.len() >= 8, "{bursts:?}");
        // A burst cut by the magazine running out is shorter (one a
        // magazine); the rest are taps or holds of the row (a tick either
        // side).
        let tap = aim.combat.fire.tap_ms;
        let is_tap = |ms: u32| ms <= tap.1 + 50;
        let holds: Vec<u32> = bursts
            .iter()
            .map(|b| b.1)
            .filter(|&ms| !is_tap(ms))
            .collect();
        let short = holds.iter().filter(|&&ms| ms + 50 < row.hold_ms.0).count();
        assert!(short <= 4, "only magazines' ends: {bursts:?}");
        assert!(
            holds.iter().all(|&ms| ms <= row.hold_ms.1 + 50),
            "{bursts:?}"
        );
        assert!(
            holds.len() * 2 > bursts.len(),
            "mostly holds near: {bursts:?}"
        );
        let pauses: Vec<u32> = bursts
            .windows(2)
            .map(|w| w[1].0 - (w[0].0 + w[0].1))
            .filter(|&gap| gap < 1_500)
            .collect();
        assert!(!pauses.is_empty());
        for gap in &pauses {
            assert!(
                (aim.pause_ms.0 - 50..=aim.pause_ms.1 + 100).contains(gap),
                "{pauses:?}"
            );
        }
    }

    #[test]
    fn a_bolt_action_is_pressed_once_a_cycle_and_never_held() {
        // The best players press a bolt action once, then work the bolt and
        // bring the sights back on him: 0.6 / 1.1 / 1.9 s past the config's
        // cycle, never sooner than 0.4 s past it ("you can't just hold FIRE
        // on sniper weapons"). The bot: a press of at most `press_ms`, the
        // next after the cycle, `press_gap_ms` and `bolt_reaim_ms`. The
        // reload between is the whole load: the early cut is another test's.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            combat: CombatTuning {
                reload_rounds_in_view: 0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mosin = guns::gun("mosin_nagant").unwrap();
        let ticks = facing_fight("camper", 40.0, &aim, 400);
        let runs = spans(&ticks, |o| o.fire);
        assert!(runs.len() >= 4, "{runs:?}");
        assert!(
            runs.iter().all(|r| r.1 <= aim.combat.press_ms),
            "never held: {runs:?}"
        );
        let cycle = mosin.cycle_ms() + aim.combat.press_gap_ms;
        let reaim = aim.combat.fire.bolt_reaim_ms;
        for w in runs.windows(2) {
            let gap = w[1].0 - w[0].0;
            // After the cycle and the re-aim (a tick late at most), or a
            // reload between.
            assert!(gap >= cycle + reaim.0, "{runs:?}");
            assert!(
                gap <= cycle + reaim.1 + 50 || gap >= cycle + 3_000,
                "{runs:?}"
            );
        }
        // Down the sights at every press, and still for it: no side key
        // from `bolt_still_ms` before the press.
        for (now, o) in &ticks {
            if o.fire {
                assert!(o.keys & key::AIM != 0, "{now}");
                assert_eq!(o.strafe, 0, "{now}");
            }
        }
    }

    /// A gun whose shot waits for the aim to settle ([`Settle::WanderAndPolicy`], here on every gun) stands still while it waits, as a player
    /// steadies for a long shot; the same bot with the settling off fires at once and side-steps between its bursts.
    #[test]
    fn a_shot_waiting_for_the_aim_to_settle_is_not_a_side_step() {
        let aim = |settle| AimTuning {
            fire_within_deg: 180.0,
            // `settle_k` small: the error to expect never fits the body, so the shot waits for the cap.
            combat: CombatTuning { settle, settle_guns: SettleGuns::All, settle_k: 0.1, ..CombatTuning::default() },
            ..AimTuning::default()
        };
        let waiting = facing_fight("support", 30.0, &aim(Settle::WanderAndPolicy), 60);
        assert!(waiting.iter().all(|(_, o)| !o.fire && o.strafe == 0), "{:?}", waiting.iter().filter(|(_, o)| o.fire || o.strafe != 0).count());
        let free = facing_fight("support", 30.0, &aim(Settle::Off), 60);
        assert!(free.iter().any(|(_, o)| o.fire), "the same bot fires within 3 s with the settling off");
        assert!(free.iter().any(|(_, o)| o.strafe != 0), "and side-steps");
    }

    /// Every weapon of the game data that the tiers' kits may carry is
    /// fired by its kind: a bolt action (the SV-98, the Remington 700, the
    /// TOZ-122 of the stock data) is pressed once a cycle and never held, a
    /// marksman rifle (the SVD) pressed again later the farther he is, an
    /// automatic weapon held.
    #[test]
    fn each_weapon_is_fired_by_its_kind() {
        let c = CombatTuning::default();
        let mut rng = Rng::new(5);
        for item in ["sv_98", "rem_700", "toz_122", "mosin_nagant"] {
            let gun = guns::gun(item).unwrap();
            assert_eq!(gun.handling, Handling::Bolt, "{item}");
            for _ in 0..50 {
                let wait = c.fire.wait_ms(gun, 70.0, &mut rng);
                assert!((c.fire.bolt_reaim_ms.0..=c.fire.bolt_reaim_ms.1).contains(&wait));
            }
            assert_eq!(c.fire.strafe_chance(gun), c.fire.strafe_bolt);
        }
        let svd = guns::gun("svd").unwrap();
        let mean = |d: f32, rng: &mut Rng| {
            (0..400)
                .map(|_| c.fire.wait_ms(svd, d, rng) as f32)
                .sum::<f32>()
                / 400.0
        };
        let (near, far) = (mean(15.0, &mut rng), mean(80.0, &mut rng));
        assert!(near < 150.0 && far > 3.0 * near, "{near} {far}");
        assert_eq!(c.fire.crouch_chance(svd, 0.0), c.fire.crouch_marksman);
        let rpd = guns::gun("rpd").unwrap();
        assert_eq!(c.fire.crouch_chance(rpd, 0.0), c.fire.crouch_machine_gun);
        assert_eq!(c.fire.strafe_chance(rpd), 1.0);
    }

    /// The best players' bursts get shorter with the distance: a rifle is
    /// held to 35 m and mostly tapped beyond 60 m; a submachine gun is held
    /// longer than a rifle at every distance.
    #[test]
    fn a_burst_falls_with_the_distance() {
        let c = CombatTuning::default();
        let mut rng = Rng::new(9);
        let mean = |class, d: f32, rng: &mut Rng| {
            (0..2_000)
                .map(|_| c.fire.burst_ms(class, d, (500, 1_000), rng) as f32)
                .sum::<f32>()
                / 2_000.0
        };
        let rifle: Vec<f32> = [10.0, 30.0, 50.0, 80.0]
            .map(|d| mean(AutoClass::Rifle, d, &mut rng))
            .to_vec();
        assert!(rifle.windows(2).all(|w| w[1] <= w[0] + 20.0), "{rifle:?}");
        assert!(rifle[3] < 0.5 * rifle[0], "{rifle:?}");
        let smg: Vec<f32> = [10.0, 50.0, 80.0]
            .map(|d| mean(AutoClass::Smg, d, &mut rng))
            .to_vec();
        // About as long at 50 m as near (more taps, longer holds); much
        // shorter beyond 60 m; longer than a rifle's at range.
        assert!(smg[1] >= smg[0] - 50.0 && smg[2] < smg[1], "{smg:?}");
        assert!(smg[1] > rifle[2] && smg[2] > rifle[3], "{smg:?} {rifle:?}");
    }

    /// A bot of `role` facing an enemy `distance` ahead from 1 s for
    /// `ticks` ticks at skill 1, standing, its view turned by the motor and
    /// kicked as the client kicks it ([`super::super::recoil`]: the view is
    /// the base pitch and the kick, a downward turn taken off the kick
    /// first). The client's kick here is the bot's own model with the
    /// ranges' middles. Returns, for each tick it fired, how far the view
    /// was over the chest, degrees; and the shots.
    fn kicked_fight(role: &str, distance: f32, aim: &AimTuning, ticks: u32) -> (Vec<f32>, u32) {
        let (over, shots) = kicked_fight_with(Arms::of_test_role(role), distance, aim, ticks, 1.0);
        (over.into_iter().map(|(_, o)| o).collect(), shots)
    }

    /// [`kicked_fight`] with `arms` at `skill`; each tick of fire with the
    /// time the trigger had been held, ms.
    fn kicked_fight_with(
        arms: Option<&'static Arms>,
        distance: f32,
        aim: &AimTuning,
        ticks: u32,
        skill: f32,
    ) -> (Vec<(u32, f32)>, u32) {
        use super::super::motor::{Feel, Motor, Pose};
        let mut rng = Rng::new(11);
        let mut motor = Motor::new(Feel::default(), &mut rng);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(0.0, 0.0, distance);
        let enemy = body(1, 1, at, 0.0);
        facing(&mut me, enemy.chest());
        let chest = me.pitch;
        let view = view_of(vec![me.clone(), enemy]);
        let mut memory = Memory::default();
        let mut reflex = Reflex::armed(arms);
        // The client's side: the base pitch and the weapon's kick.
        let mut base = me.pitch;
        let mut client = Recoil::in_hand(reflex.hands().held().gun.item);
        let mut cadence = Cadence::default();
        let (mut over, mut shots, mut held) = (Vec::new(), 0, 0u32);
        for k in 0..ticks {
            let now = 1_000 + k * 50;
            sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    aim,
                    skill,
                    &mut rng,
                );
            }
            // The bot's own reckoning: its base pitch and its kick.
            let kick = reflex.weapon_tick(&me, now, PitchScale::Radian);
            let seen = PlaceView {
                pitch: base + kick,
                ..me.clone()
            };
            let o = reflex.tick(&seen, &view, &memory, now, true, aim, skill, &mut rng);
            // Before he is noticed it looks nowhere: the view stays.
            let eye = me.eye();
            let want = o.look_at.map_or(seen.pitch, |p| {
                (p.y - eye.y).atan2((p.z - eye.z).abs()) + o.error.1
            });
            motor.turn_time = o.turn_time_s;
            let pose = Pose {
                feet: me.feet,
                yaw: me.yaw,
                pitch: seen.pitch,
                speed: 0.0,
                ..Default::default()
            };
            let turn = motor.aim_still(&pose, (me.yaw, want)).turn.1 + o.pull;
            let keys = o.keys | if o.fire { key::FIRE } else { 0 };
            // The bot's books and the client's, each the same rule; the
            // client's kick is reset by a reload, and starts afresh with the
            // pistol.
            let taken = reflex.sent(now, keys, turn);
            let item = reflex.held_gun().item;
            if item != client.held() {
                client = Recoil::in_hand(item);
                cadence = Cadence::default();
            } else if keys & key::RELOAD != 0 {
                client.reset();
            }
            client.update(now);
            let client_taken = client.turn(turn);
            assert!((taken - client_taken).abs() < 1e-6);
            base += turn + client_taken;
            let auto = reflex.hands().held().gun.auto;
            let interval_ms = client.kick().interval_s() * 1000.0;
            cadence.tick(now, o.fire, auto, interval_ms, |t| {
                client.mean_shot(t);
                shots += 1;
            });
            if o.fire {
                over.push((held, (client.view(base) - chest).to_degrees()));
                held += 50;
            } else {
                held = 0;
            }
        }
        (over, shots)
    }

    /// Prints how far over the chest the view sits while the trigger is
    /// held, by the time held, per weapon and skill, with the kick taken
    /// back by the skill's pull (`cargo test -p match-server
    /// kick_left_by_skill -- --ignored --nocapture`). The top players'
    /// base pitch sat under the chest by 1.4 / 3.5 / 5.8 / 5.6° at 0-250 /
    /// 250-500 / 500-1000 / 1000+ ms of a burst while their hits rode the
    /// kick (`TOP-PLAYERS.md` §3, §8a): they took back about all of it.
    #[test]
    #[ignore]
    fn kick_left_by_skill() {
        let aim = AimTuning {
            fire_within_deg: 180.0,
            error_deg: (0.01, 0.01),
            wander_deg: (0.0, 0.0),
            ..AimTuning::default()
        };
        for item in [
            "ak_74u", "ak_74m", "akmn", "uzi", "pp2000", "vityaz", "sr2m", "rpd",
        ] {
            let Some(gun) = guns::gun(item) else { continue };
            let arms: &'static Arms = Box::leak(Box::new(Arms {
                primary: (gun, 300),
                pistol: None,
                reserve: 300,
            }));
            for skill in [0.0, 0.6, 1.0] {
                let (over, _) = kicked_fight_with(Some(arms), 20.0, &aim, 300, skill);
                let band = |a: u32, b: u32| {
                    let v: Vec<f32> = over
                        .iter()
                        .filter(|(h, _)| (a..b).contains(h))
                        .map(|(_, o)| *o)
                        .collect();
                    if v.is_empty() { f32::NAN } else { median(v) }
                };
                println!(
                    "{item} skill {skill}: view over the chest {:.2} / {:.2} / {:.2} / {:.2} deg at 0-250 / 250-500 / 500-1000 / 1000+ ms held",
                    band(0, 250),
                    band(250, 500),
                    band(500, 1_000),
                    band(1_000, 10_000)
                );
            }
        }
    }

    /// The best bots take back about twice the kick the weakest do: the
    /// view climbs off the chest in a burst at skill 0, much less at 1.
    #[test]
    fn the_pull_against_the_kick_grows_with_the_skill() {
        let aim = AimTuning {
            fire_within_deg: 180.0,
            error_deg: (0.01, 0.01),
            wander_deg: (0.0, 0.0),
            ..AimTuning::default()
        };
        let climb = |skill: f32| {
            let (over, _) = kicked_fight_with(Arms::of_test_role("support"), 20.0, &aim, 300, skill);
            let v: Vec<f32> = over
                .iter()
                .filter(|(h, _)| *h >= 250)
                .map(|(_, o)| *o)
                .collect();
            median(v)
        };
        let (weak, best) = (climb(0.0), climb(1.0));
        assert!(best.abs() < 0.1, "{best}");
        assert!(weak > 0.2 && weak > 3.0 * best.abs(), "{weak} {best}");
    }

    #[test]
    fn the_turns_take_back_the_kick_so_the_view_stays_on_the_target() {
        // No wander and next to no floor: the view is on the chest while it
        // fires, kick or no kick, for an automatic weapon, a bolt action and
        // the SMG.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            error_deg: (0.01, 0.01),
            wander_deg: (0.0, 0.0),
            ..AimTuning::default()
        };
        for (role, distance) in [("support", 20.0), ("camper", 40.0), ("flanker", 15.0)] {
            let (over, shots) = kicked_fight(role, distance, &aim, 200);
            assert!(shots >= 3, "{role}: {shots} shots");
            let worst = over.iter().fold(0f32, |m, o| m.max(o.abs()));
            assert!(worst < 0.1, "{role}: {worst}° off the chest");
        }
        // With none of the kick taken back, the view climbs off him while
        // the trigger is held (the turns follow it down a tick late).
        let lifted = AimTuning {
            combat: CombatTuning {
                recoil_pull: 0.0,
                recoil_pull_low: 0.0,
                ..CombatTuning::default()
            },
            ..aim.clone()
        };
        let (over, _) = kicked_fight("support", 20.0, &lifted, 200);
        let highest = over.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(highest > 0.8, "{highest}");
    }

    /// A weapon the kick table does not know (A224: main's derived weapons on
    /// live) kicks as the AK-74M, said once, and keeps that model from tick to
    /// tick: a shot's kick is there at the next tick and pulled back, so the
    /// view stays on the chest as with a known weapon. Rebuilt at every tick,
    /// the bot had no kick to pull against while the client's climbed.
    #[test]
    fn a_weapon_with_no_kick_keeps_its_model_between_ticks_and_pulls() {
        use super::super::recoil::{DEFAULT_KICK, weapons_without_kick};
        let known = Arms::of_test_role("support").expect("a support kit").primary.0;
        let gun: &'static Gun = Box::leak(Box::new(Gun {
            item: "no_kick_test",
            ..*known
        }));
        let arms: &'static Arms = Box::leak(Box::new(Arms {
            primary: (gun, 300),
            pistol: None,
            reserve: 300,
        }));
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let mut reflex = Reflex::armed(Some(arms));
        reflex.weapon_tick(&me, 1_000, PitchScale::Client);
        assert_eq!(reflex.recoil.held(), "no_kick_test");
        assert_eq!(reflex.recoil.kick().item, DEFAULT_KICK);
        assert!(weapons_without_kick().contains(&"no_kick_test"));
        reflex.sent(1_000, key::FIRE, 0.0);
        reflex.weapon_tick(&me, 1_050, PitchScale::Client);
        assert_eq!(reflex.recoil.held(), "no_kick_test");
        assert!(reflex.kick() > 0.0 && reflex.kick_step > 0.0, "kick {} step {}", reflex.kick(), reflex.kick_step);
        // The bot's books equal the client's at every tick (`kicked_fight_with`),
        // and the pull keeps the view on him; with no pull it climbs off.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            error_deg: (0.01, 0.01),
            wander_deg: (0.0, 0.0),
            ..AimTuning::default()
        };
        let (over, shots) = kicked_fight_with(Some(arms), 20.0, &aim, 200, 1.0);
        assert!(shots >= 3, "{shots} shots");
        let worst = over.iter().fold(0f32, |m, (_, o)| m.max(o.abs()));
        assert!(worst < 0.1, "{worst}° off the chest");
        let lifted = AimTuning {
            combat: CombatTuning {
                recoil_pull: 0.0,
                recoil_pull_low: 0.0,
                ..CombatTuning::default()
            },
            ..aim
        };
        let (over, _) = kicked_fight_with(Some(arms), 20.0, &lifted, 200, 1.0);
        let highest = over.iter().map(|(_, o)| *o).fold(f32::NEG_INFINITY, f32::max);
        assert!(highest > 0.8, "{highest}");
    }

    /// The trigger let go for a jump in a tick that fired: that tick's
    /// rounds are given back, an automatic's burst starts again, and a
    /// weapon of a round a press may press again at once.
    #[test]
    fn a_shot_let_go_for_a_jump_is_given_back() {
        let c = CombatTuning::default();
        let mut rng = Rng::new(5);
        let mut auto = Reflex::armed(Arms::of_test_role("support"));
        auto.tick_ms = 50;
        let mut given_back = false;
        for k in 0..20 {
            let now = 1_000 + 50 * k;
            let before = auto.hands().held().rounds;
            assert!(auto.trigger(true, true, now, &c, 20.0, &mut rng));
            if auto.hands().held().rounds < before {
                auto.hold_fire(now);
                assert_eq!(auto.hands().held().rounds, before);
                assert_eq!(auto.held_ms, 0);
                given_back = true;
                break;
            }
        }
        assert!(given_back);
        let mut bolt = Reflex::armed(Arms::of_test_role("camper"));
        let full = bolt.hands().held().rounds;
        assert!(bolt.trigger(true, true, 1_000, &c, 50.0, &mut rng));
        assert_eq!(bolt.hands().held().rounds, full - 1);
        bolt.hold_fire(1_000);
        assert_eq!(bolt.hands().held().rounds, full);
        assert!(bolt.next_press <= 1_050 && bolt.press_until <= 1_000);
        // A tick later, nothing more to give back.
        bolt.hold_fire(1_050);
        assert_eq!(bolt.hands().held().rounds, full);
    }

    /// Walking into its weapon's band (the reflex's legs, a walk: an Uzi at
    /// a rifle 46 m off), it still steps side to side (`strafe_walking`);
    /// with that off, only standing.
    #[test]
    fn it_steps_side_to_side_while_it_walks_into_its_band() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let walk = |aim: &AimTuning| {
            let me = body(0, 0, Vec3::new(24.0, 0.0, 30.0), 0.0);
            let enemy_at = Vec3::new(70.0, 0.0, 30.0);
            let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
            let weapons = [Some(WeaponKind::Smg), Some(WeaponKind::Rifle)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                weapon: Some(WeaponKind::Smg),
                weapons: &weapons,
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::armed(Arms::of_test_role("flanker"));
            let mut rng = Rng::new(64);
            let mut out = Vec::new();
            for k in 0..120 {
                let now = 1_000 + k * 50;
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                if k % 5 == 0 {
                    reflex.think_with(&me, &sit, &memory, now, aim, 1.0, &mut rng);
                }
                out.push(reflex.tick(&me, &view, &memory, now, true, aim, 1.0, &mut rng));
            }
            out
        };
        let aim = AimTuning::default();
        let ticks = walk(&aim);
        let walking: Vec<&Override> = ticks
            .iter()
            .filter(|o| o.legs.is_some_and(|l| !l.run && l.why == LegsWhy::Range))
            .collect();
        assert!(walking.len() > 50, "{}", walking.len());
        let stepping = walking.iter().filter(|o| o.strafe != 0).count();
        assert!(stepping * 2 > walking.len(), "{stepping} of {}", walking.len());
        let still = AimTuning {
            combat: CombatTuning {
                strafe_walking: false,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let ticks = walk(&still);
        assert!(ticks.iter().any(|o| o.legs.is_some()));
        assert!(
            ticks
                .iter()
                .filter(|o| o.legs.is_some())
                .all(|o| o.strafe == 0)
        );
    }

    #[test]
    fn it_aims_down_the_sights_and_steps_side_to_side_at_a_seen_target() {
        let aim = AimTuning::default();
        let ticks = facing_fight("support", 20.0, &aim, 200);
        let fighting: Vec<&Override> = ticks
            .iter()
            .map(|(_, o)| o)
            .filter(|o| o.look_at.is_some())
            .collect();
        assert!(fighting.len() > 150);
        // From 10 m on, always down the sights.
        assert!(fighting.iter().all(|o| o.keys & key::AIM != 0));
        // Side to side: both ways, runs in the tuning's range, a still
        // moment now and then; never both keys.
        let c = &aim.combat;
        let left = spans(&ticks, |o| o.strafe == key::LEFT);
        let right = spans(&ticks, |o| o.strafe == key::RIGHT);
        assert!(left.len() >= 3 && right.len() >= 3, "{left:?} {right:?}");
        for (_, ms) in left
            .iter()
            .chain(&right)
            .filter(|(start, _)| *start > 1_200)
        {
            assert!(
                *ms >= c.strafe_run_ms.0 - 50 && *ms <= c.strafe_run_ms.1 + 50,
                "{left:?} {right:?}"
            );
        }
        let moving = fighting.iter().filter(|o| o.strafe != 0).count() as f32;
        let share = moving / fighting.len() as f32;
        // Top players: a side key on 78 % of their automatic fire.
        assert!((0.6..=0.95).contains(&share), "{share}");
        // Near, down the sights only with the per-target draw (and, with
        // `aim_in_first`, where the hip cone would not keep the shots on
        // him: a strafing support weapon's does not at 6 m).
        let never = AimTuning {
            combat: CombatTuning {
                ads_near_chance: 0.0,
                aim_in_first: false,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let near = facing_fight("support", 6.0, &never, 60);
        assert!(near.iter().all(|(_, o)| o.keys & key::AIM == 0));
        // A bolt action: no side step; crouched to fire when drawn.
        let crouching = AimTuning {
            combat: CombatTuning {
                sniper_crouch_chance: 1.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let sniper = facing_fight("camper", 40.0, &crouching, 100);
        assert!(sniper.iter().all(|(_, o)| o.strafe == 0));
        assert!(
            sniper
                .iter()
                .filter(|(_, o)| o.look_at.is_some())
                .all(|(_, o)| o.keys & (key::CROUCH | key::AIM) == key::CROUCH | key::AIM)
        );
    }

    #[test]
    fn a_hit_turns_the_side_step_at_once() {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(0.0, 0.0, 20.0);
        let enemy = body(1, 1, at, 0.0);
        facing(&mut me, enemy.chest());
        let aim = AimTuning {
            combat: CombatTuning {
                strafe_run_ms: (5_000, 5_000),
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(12);
        let mut steps = Vec::new();
        for k in 0..60u32 {
            let now = 1_000 + k * 50;
            if k == 40 {
                me.health = 0.7;
            }
            let view = view_of(vec![me.clone(), enemy.clone()]);
            sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    1.0,
                    &mut rng,
                );
            }
            steps.push(
                reflex
                    .tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng)
                    .strafe,
            );
        }
        // One long run until the hit, the other way after it.
        let before = steps[39];
        assert_ne!(before, 0);
        assert!(steps[20..40].iter().all(|&s| s == before));
        assert!(
            steps[40..].iter().all(|&s| s != before && s != 0),
            "{steps:?}"
        );
    }

    #[test]
    fn a_wounded_enemy_is_preferred_among_the_seen() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let near = body(1, 1, Vec3::new(0.0, 0.0, 15.0), 0.0);
        let mut far = body(2, 1, Vec3::new(3.0, 0.0, 25.0), 0.0);
        far.health = 0.2;
        let view = view_of(vec![me.clone(), near.clone(), far.clone()]);
        // The tip's rule: the true body pool (the default reads what the team saw land).
        let mut aim = AimTuning::default();
        aim.combat.worth.hurt_from = crate::v026g::bots::worth::HurtFrom::Body;
        let mut memory = Memory::default();
        let mut rng = Rng::new(5);
        for now in [1_000, 1_400] {
            let seen = [
                percept(1, near.feet, now, Source::Seen),
                percept(2, far.feet, now, Source::Seen),
            ];
            sense(&mut memory, &seen, now);
        }
        let mut reflex = Reflex::default();
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_400,
            &aim,
            0.5,
            &mut rng,
        );
        assert_eq!(reflex.target(), Some(2));
        // Without the view's health (the first form), the nearest.
        let mut reflex = Reflex::default();
        reflex.think(&me, &memory, 1_400, &aim, 0.5, &mut rng);
        assert_eq!(reflex.target(), Some(1));
    }

    #[test]
    fn spot_is_pressed_once_per_new_enemy_and_keeps_the_cooldown() {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let a = body(1, 1, Vec3::new(0.0, 0.0, 20.0), 0.0);
        let b = body(2, 1, Vec3::new(1.0, 0.0, 20.0), 0.0);
        facing(&mut me, a.chest());
        let mut view = view_of(vec![me.clone(), a.clone(), b.clone()]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(6);
        let mut spots = Vec::new();
        for k in 0..200u32 {
            let now = 1_000 + k * 50;
            // A at first; from 1.5 s only B is in sight (A gone behind
            // something), so B is a new enemy within A's cooldown; A back
            // at 6 s.
            let mut seen = Vec::new();
            if !(1_500..6_000).contains(&now) {
                seen.push(percept(1, a.feet, now, Source::Seen));
            }
            if now >= 1_500 {
                seen.push(percept(2, b.feet, now, Source::Seen));
            }
            sense(&mut memory, &seen, now);
            view.clock = now;
            if k % 5 == 0 {
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            let target = reflex.target();
            if let Some(t) = target {
                let chest = view.place(t).unwrap().chest();
                facing(&mut me, chest);
            }
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            if o.keys & key::SPOT != 0 {
                // The tick may take a target it noticed since the think.
                spots.push((now, reflex.target().or(target)));
            }
        }
        // A once, B once (after A's cooldown), A not again within
        // `spot_again_ms`.
        assert_eq!(spots.len(), 2, "{spots:?}");
        assert_eq!(spots[0].1, Some(1));
        assert_eq!(spots[1].1, Some(2));
        assert!(spots[1].0 >= spots[0].0 + aim.combat.spot_cooldown_ms);
    }

    #[test]
    fn a_reload_heard_in_a_duel_sends_the_legs_at_him() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy_at = Vec3::new(0.0, 0.0, 15.0);
        let enemy = body(1, 1, enemy_at, 0.0);
        let mut view = view_of(vec![me.clone(), enemy]);
        let aim = AimTuning {
            combat: CombatTuning {
                punish_chance: 1.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(7);
        for now in [1_000, 1_300] {
            sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        assert_eq!(reflex.target(), Some(1));
        let o = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
        assert!(o.legs.is_none());
        // He reloads: within the window the legs go at him.
        view.sounds.push(Sound {
            from: 1,
            kind: SoundKind::Reload,
            at: enemy_at,
            time: 1_400,
            reach_m: None,
        });
        sense(
            &mut memory,
            &[percept(1, enemy_at, 1_500, Source::Seen)],
            1_500,
        );
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_500,
            &aim,
            0.5,
            &mut rng,
        );
        let o = reflex.tick(&me, &view, &memory, 1_550, true, &aim, 0.5, &mut rng);
        let legs = o.legs.expect("pushes");
        assert_eq!(legs.why, LegsWhy::Punish);
        assert!(legs.at.distance_xz(enemy_at) < 0.1);
        // Over after `punish_ms`, and the same sound is not weighed again.
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_700,
            &aim,
            0.5,
            &mut rng,
        );
        let o = reflex.tick(&me, &view, &memory, 3_100, true, &aim, 0.5, &mut rng);
        assert!(o.legs.is_none(), "{o:?}");
        // A reload out of the duel and out of hearing: nothing.
        let mut reflex = Reflex::default();
        let far = Vec3::new(0.0, 0.0, 40.0);
        let mut memory = Memory::default();
        sense(&mut memory, &[percept(1, far, 9_000, Source::Heard)], 9_000);
        view.sounds = vec![Sound {
            from: 1,
            kind: SoundKind::Reload,
            at: far,
            time: 9_000,
            reach_m: None,
        }];
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            9_100,
            &aim,
            0.5,
            &mut rng,
        );
        assert!(
            reflex
                .tick(&me, &view, &memory, 9_150, true, &aim, 0.5, &mut rng)
                .legs
                .is_none()
        );
    }

    #[test]
    fn hit_in_a_duel_it_breaks_sight_waits_and_repeeks_from_another_angle() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        // The field: a wall across x = 40 from z = 0 to 25. The enemy east
        // of it, the bot west, past the wall's end.
        let enemy_at = Vec3::new(55.0, 0.0, 30.0);
        let mut me = body(0, 0, Vec3::new(32.0, 0.0, 28.0), 0.0);
        let enemy = body(1, 1, enemy_at, 0.0);
        let view = view_of(vec![me.clone(), enemy]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(8);
        let sit = Situation {
            tactics: Some(&tactics),
            side: Some(0),
            ..Situation::of(&view)
        };
        for now in [1_000, 1_300] {
            sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
            reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
        }
        assert_eq!(reflex.target(), Some(1));
        // Hit.
        me.health = 0.7;
        sense(
            &mut memory,
            &[percept(1, enemy_at, 1_600, Source::Seen)],
            1_600,
        );
        reflex.think_with(&me, &sit, &memory, 1_600, &aim, 0.5, &mut rng);
        let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
        let legs = o.legs.expect("breaks sight");
        assert_eq!(legs.why, LegsWhy::BreakSight);
        let hide = legs.at;
        let eye = |p: Vec3| still_eye(p, false);
        assert!(!tactics.sees(eye(enemy_at), eye(hide)), "{hide:?}");
        let d = hide.distance_xz(Vec3::new(32.0, 0.0, 28.0));
        assert!((2.0..=10.0).contains(&d), "{d}");
        // There: it waits 1-2 s, then comes back at another angle.
        me.feet = hide;
        let mut now = 2_000;
        reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
        let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
        assert_eq!(o.legs.map(|l| l.why), Some(LegsWhy::BreakSight), "waits");
        let mut peek = None;
        while now < 5_000 {
            now += 250;
            reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            if let Some(legs) = o.legs.filter(|l| l.why == LegsWhy::RePeek) {
                peek = Some((now, legs.at));
                break;
            }
        }
        let (when, at) = peek.expect("re-peeks");
        assert!((3_000..=4_250).contains(&when), "{when}");
        assert!(another_angle(
            enemy_at,
            Vec3::new(32.0, 0.0, 28.0),
            at,
            &aim.combat
        ));
        assert!(tactics.sees(
            at + Vec3::new(0.0, 0.95, 0.0),
            enemy_at + Vec3::new(0.0, 1.0, 0.0)
        ));
    }

    #[test]
    fn a_grenade_goes_only_at_a_sensed_place_and_two_a_life() {
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let camper_at = Vec3::new(0.0, 0.0, 18.0);
        let mut camper = body(1, 1, camper_at, 0.0);
        camper.keys = key::CROUCH | key::AIM;
        // Another enemy holding an angle the bot never sensed.
        let mut unseen = body(2, 1, Vec3::new(-12.0, 0.0, 12.0), 0.0);
        unseen.keys = key::CROUCH | key::AIM;
        let view = view_of(vec![me.clone(), camper, unseen]);
        let aim = AimTuning {
            combat: CombatTuning {
                grenade_chance: 1.0,
                grenade_roll_ms: 500,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(10);
        // Seen holding an angle, then gone from sight (no target).
        for now in [1_000, 1_300] {
            sense(
                &mut memory,
                &[percept(1, camper_at, now, Source::Seen)],
                now,
            );
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        let mut held_ticks = 0;
        let mut throws = Vec::new();
        let mut now = 1_800;
        while now < 12_000 {
            sense(&mut memory, &[], now);
            if now % 250 == 0 {
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            let before = reflex.grenades_thrown();
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            if let Some(look) = o
                .look_at
                .filter(|_| o.keys & key::GRENADE != 0 || o.error != (0.0, 0.0))
            {
                // Only ever at the place it saw him.
                assert!(look.distance_xz(camper_at) < 0.5, "{look:?}");
                assert!(!o.fire);
                facing(&mut me, look);
            }
            if o.keys & key::GRENADE != 0 {
                held_ticks += 1;
            }
            if reflex.grenades_thrown() > before {
                throws.push(now);
            }
            now += 50;
        }
        // Two throws (the kit's two), each with the key held 0.5-0.8 s.
        assert_eq!(throws.len(), 2, "{throws:?}");
        assert!((20..=34).contains(&held_ticks), "{held_ticks}");
        // A new life: two again.
        me.spawned = 50_000;
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            now,
            &aim,
            0.5,
            &mut rng,
        );
        assert_eq!(reflex.grenades_thrown(), 0);
    }

    /// A bot that saw a camper 18 m off and lost him, through 12 s: the
    /// times a grenade was let go, and the counts.
    fn volley_run(combat: CombatTuning) -> (Vec<u32>, GrenadeStats) {
        let (throws, stats, _) = kit_run(combat, None);
        (throws, stats)
    }

    /// [`volley_run`] with the kit's frags now known to the reflex (`set_frags_left`), and the rolls it refused for them.
    fn kit_run(combat: CombatTuning, frags: Option<u16>) -> (Vec<u32>, GrenadeStats, u32) {
        kit_run_heard(combat, frags, None)
    }

    /// [`kit_run`] with the camper heard 2 m off his place every 250 ms from `heard_after` (an enemy sensed by the first
    /// grenade's place once it is let go).
    fn kit_run_heard(combat: CombatTuning, frags: Option<u16>, heard_after: Option<u32>) -> (Vec<u32>, GrenadeStats, u32) {
        let camper_at = Vec3::new(0.0, 0.0, 18.0);
        let mut camper = body(1, 1, camper_at, 0.0);
        camper.keys = key::CROUCH | key::AIM;
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone(), camper]);
        let aim = AimTuning {
            combat,
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        reflex.set_frags_left(frags);
        let mut rng = Rng::new(10);
        for now in [1_000, 1_300] {
            sense(&mut memory, &[percept(1, camper_at, now, Source::Seen)], now);
            reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, 0.5, &mut rng);
        }
        let mut throws = Vec::new();
        let mut now = 1_800;
        while now < 12_000 {
            let heard = if now % 250 == 0 && heard_after.is_some_and(|t| now >= t) {
                vec![percept(1, camper_at + Vec3::new(2.0, 0.0, 0.0), now, Source::Heard)]
            } else {
                Vec::new()
            };
            sense(&mut memory, &heard, now);
            if now % 250 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, 0.5, &mut rng);
            }
            let before = reflex.grenades_thrown();
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            if let Some(look) = o
                .look_at
                .filter(|_| o.keys & key::GRENADE != 0 || o.error != (0.0, 0.0))
            {
                facing(&mut me, look);
            }
            if reflex.grenades_thrown() > before {
                throws.push(now);
            }
            now += 50;
        }
        (throws, reflex.grenade_stats(), reflex.refused_no_frag())
    }

    /// A bot whose kit holds no frag (a trap role's) rolls no grenade, and counts the roll it refused; with the cap off it throws
    /// one that does nothing; a kit not read (none) and a kit with a frag throw as before.
    #[test]
    fn a_roll_is_refused_when_the_kit_holds_no_frag_and_only_with_the_cap_on() {
        let combat = |cap| CombatTuning {
            grenade_chance: 1.0,
            grenade_roll_ms: 60_000,
            grenade_kit_cap: cap,
            ..CombatTuning::default()
        };
        let (throws, _, refused) = kit_run(combat(true), Some(0));
        assert_eq!((throws.len(), refused), (0, 1), "no frag: no throw, one refused roll");
        let (throws, _, refused) = kit_run(combat(false), Some(0));
        assert_eq!((throws.len(), refused), (1, 0), "the cap off: the roll goes by `grenades` alone");
        let (throws, _, refused) = kit_run(combat(true), None);
        assert_eq!((throws.len(), refused), (1, 0), "a kit not read goes by `grenades`");
        let (throws, _, refused) = kit_run(combat(true), Some(2));
        assert_eq!((throws.len(), refused), (1, 0), "a kit with frags throws");
    }

    /// [C20] With the roll's lockout out of the way, a second grenade
    /// comes only with the volley: after the first is let go (not before
    /// `grenade_volley_ms.0`), at its place, before its blast.
    #[test]
    fn a_second_grenade_follows_the_first_at_its_place_only_with_the_volley() {
        let combat = |volley| CombatTuning {
            grenade_chance: 1.0,
            grenade_roll_ms: 60_000,
            grenade_volley: volley,
            grenade_volley_chance: 1.0,
            ..CombatTuning::default()
        };
        let (throws, stats) = volley_run(combat(false));
        assert_eq!(throws.len(), 1, "{throws:?}");
        assert_eq!(stats.volley, [0; 6], "{stats:?}");
        let (throws, stats) = volley_run(combat(true));
        assert_eq!(throws.len(), 2, "{throws:?}");
        let gap = throws[1] - throws[0];
        assert!((1_600..=6_000).contains(&gap), "{gap}");
        assert_eq!(stats.nth, [1, 1, 0], "{stats:?}");
        assert_eq!((stats.volley[0], stats.volley[3]), (1, 1), "{stats:?}");
        assert!(stats.volley[1] >= 1, "{stats:?}");
        // The chance can say no: no second grenade, the rolls counted.
        let (throws, stats) = volley_run(CombatTuning {
            grenade_volley_chance: 0.0,
            ..combat(true)
        });
        assert_eq!(throws.len(), 1, "{throws:?}");
        assert_eq!((stats.volley[0], stats.volley[3]), (1, 0), "{stats:?}");
        assert_eq!(stats.volley[1], stats.volley[2], "{stats:?}");
        assert!(stats.volley[1] >= 1, "{stats:?}");
    }

    /// [A187 #10] The second grenade by evidence: with `grenade_volley_track` it is thrown only when an enemy is sensed by the first's
    /// place once the first is let go, and then at where he is sensed; with no one sensed the rolls are counted as without evidence
    /// and the frag is kept; with the switch off the volley is as it was.
    #[test]
    fn the_second_grenade_follows_an_enemy_sensed_by_the_first_place_when_asked_to() {
        let combat = |track| CombatTuning {
            grenade_chance: 1.0,
            grenade_roll_ms: 60_000,
            grenade_volley: true,
            grenade_volley_chance: 1.0,
            grenade_volley_track: track,
            ..CombatTuning::default()
        };
        // Nobody sensed after the first: no second, rolls without evidence.
        let (throws, stats, _) = kit_run_heard(combat(true), None, None);
        assert_eq!(throws.len(), 1, "{throws:?}");
        assert_eq!(stats.volley[3], 0, "{stats:?}");
        assert!(stats.volley[5] >= 1 && stats.volley[4] == 0, "{stats:?}");
        // An enemy heard 2 m from the first's place from 4 s on (the first is let go about 2.5 s in): a second, rolls with evidence.
        let (throws, stats, _) = kit_run_heard(combat(true), None, Some(4_000));
        assert_eq!(throws.len(), 2, "{throws:?} {stats:?}");
        assert_eq!((stats.volley[3], stats.nth), (1, [1, 1, 0]), "{stats:?}");
        assert!(stats.volley[4] >= 1, "{stats:?}");
        // The switch off: the volley as it was, heard or not.
        let (throws, stats, _) = kit_run_heard(combat(false), None, None);
        assert_eq!(throws.len(), 2, "{throws:?}");
        assert_eq!(stats.volley[4] + stats.volley[5], 0, "{stats:?}");
    }

    /// [C20] The window ends with the kit's grenades (`grenades`), and with
    /// the first one's blast: no second after it has gone off.
    #[test]
    fn the_volley_window_closes_at_the_blast_and_with_the_last_grenade() {
        let combat = CombatTuning {
            grenade_chance: 1.0,
            grenade_roll_ms: 60_000,
            grenade_volley: true,
            grenade_volley_chance: 1.0,
            // The window is open longer than the fuse: the blast is what closes it.
            grenade_volley_ms: (5_000, 60_000),
            ..CombatTuning::default()
        };
        let (throws, stats) = volley_run(combat.clone());
        // First released at about 2.9 s, its blast about 6.2 s: a roll is due only at 7.9 s.
        assert_eq!(throws.len(), 1, "{throws:?}");
        assert_eq!(stats.volley[1], 0, "{stats:?}");
        let (throws, stats) = volley_run(CombatTuning {
            grenades: 1,
            grenade_volley_ms: (1_600, 6_000),
            ..combat
        });
        assert_eq!(throws.len(), 1, "{throws:?}");
        assert_eq!(stats.volley, [0; 6], "the kit had no second grenade: {stats:?}");
    }

    /// [G3] A place whose arc comes down inside the thrower's blast floor is
    /// thrown at under `grenade_near_m` when the avoidance is on and he can
    /// walk out of the blast; and only while he stays where he was.
    #[test]
    fn a_place_inside_the_floor_is_thrown_at_when_the_thrower_can_walk_out_of_the_blast() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let place = Vec3::new(20.0, 0.0, 19.0);
        let me = body(0, 0, Vec3::new(20.0, 0.0, 10.0), 0.0);
        let view = view_of(vec![me.clone()]);
        let memory = Memory::default();
        let near = CombatTuning {
            grenade_near_m: 6.0,
            ..combat_off()
        };
        let retreat = Retreat {
            margin_m: 0.5,
            told_ms: 400,
        };
        let start = |combat: &CombatTuning, retreat: Option<Retreat>| {
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                retreat,
                ..Situation::of(&view)
            };
            let look = RollView {
                me: &me,
                sit: &sit,
                memory: &memory,
                now: 5_000,
            };
            let mut reflex = Reflex::default();
            reflex.start_throw(&look, combat, &[(place, PlaceSource::Held)], (0.0, 0.0), 600, ThrowKind::Roll);
            reflex
        };
        // The floor as it was: the place is refused for the thrower's blast.
        let reflex = start(&combat_off(), Some(retreat));
        assert!(reflex.throw.is_none());
        assert_eq!((reflex.grenade_stats().refused[0][0], reflex.grenade_stats().near), (1, [0; 3]));
        // Lowered, with no avoidance (no retreat known): the same.
        let reflex = start(&near, None);
        assert!(reflex.throw.is_none());
        assert_eq!(reflex.grenade_stats().near, [0; 3]);
        // Lowered, with the avoidance: thrown at, from where he stands.
        let reflex = start(&near, Some(retreat));
        let throw = reflex.throw.expect("a way out of the blast was found");
        assert_eq!(throw.near, Some(me.feet));
        assert_eq!(reflex.grenade_stats().near, [1, 1, 0]);
        // Through to the let-go: the throw is made where he stands, and given up when he walked off.
        let go = |shift: f32| {
            let mut reflex = start(&near, Some(retreat));
            let mut me = me.clone();
            me.feet.x += shift;
            let aim = AimTuning {
                combat: near.clone(),
                ..AimTuning::default()
            };
            let mut rng = Rng::new(3);
            for k in 0..100 {
                let o = reflex.tick(&me, &view, &memory, 5_000 + k * 50, true, &aim, 0.5, &mut rng);
                // The throw has the look: the view goes to it (no aim error is drawn here).
                if let Some(look) = o.look_at {
                    facing(&mut me, look);
                }
            }
            (reflex.grenades_thrown(), reflex.grenade_stats())
        };
        let (thrown, stats) = go(0.0);
        assert_eq!((thrown, stats.near[2]), (1, 1), "{stats:?}");
        let (thrown, stats) = go(3.0);
        assert_eq!((thrown, stats.near[2], stats.given_up[0]), (0, 0, 1), "{stats:?}");
    }

    /// [G3] The way out is the avoidance's walk: a floor past the blast's radius within what the
    /// fuse leaves after the launch, the look held and the told reaction.
    #[test]
    fn the_way_out_of_the_blast_needs_the_walk_the_fuse_leaves() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let me = body(0, 0, Vec3::new(20.0, 0.0, 10.0), 0.0);
        let rest = Vec3::new(20.0, 0.0, 13.0);
        let c = CombatTuning::default();
        let can = |told_ms: u32| {
            can_retreat(&me, &tactics, Some(0), Retreat { margin_m: 0.5, told_ms }, rest, 5_000, &c)
        };
        // 10.5 m from the rest point is 7.5 m of walking from here: 2.5 s at 5 m/s leaves 12.5 m.
        assert!(can(400));
        // A reaction that takes up the fuse leaves none.
        assert!(!can(3_000));
        // A thrower already outside the radius needs no walk.
        let far = Vec3::new(20.0, 0.0, 30.0);
        assert!(can_retreat(&me, &tactics, Some(0), Retreat { margin_m: 0.5, told_ms: 3_800 }, far, 5_000, &c));
    }

    /// [close] A place at 8 m is thrown at as a close throw only when the thrower has a way out of the blast wherever it goes
    /// off, and every teammate was seen lately and is the radius and the margin (12 m) from the rest point and the roll's
    /// worst case, along his walk too; a refusal is counted by what refused it.
    #[test]
    fn a_close_place_is_thrown_at_only_with_a_way_out_and_every_teammate_seen_and_far() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let place = Vec3::new(20.0, 0.0, 18.0);
        let me = body(0, 0, Vec3::new(20.0, 0.0, 10.0), 0.0);
        let close = CombatTuning {
            grenade_close: true,
            ..combat_off()
        };
        let retreat = Retreat {
            margin_m: 0.5,
            told_ms: 400,
        };
        // (the mate's place, velocity and when his body was fixed, the view's clock, the retreat) -> the reflex after the roll
        let start = |mate: Option<(Vec3, [f32; 2], u32)>, clock: u32, retreat: Option<Retreat>| {
            let mut places = vec![me.clone()];
            if let Some((feet, velocity, seen_at)) = mate {
                let mut mate = body(1, 0, feet, 0.0);
                mate.velocity = velocity;
                mate.seen_at = seen_at;
                places.push(mate);
            }
            let view = MatchView { clock, ..view_of(places) };
            let memory = Memory::default();
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                retreat,
                ..Situation::of(&view)
            };
            let look = RollView {
                me: &me,
                sit: &sit,
                memory: &memory,
                now: 5_000,
            };
            let mut reflex = Reflex::default();
            reflex.start_close(&look, &close, &[(place, PlaceSource::Lost)], (0.0, 0.0), 600);
            reflex
        };
        // No teammate: thrown at, from where he stands, counted as decided.
        let reflex = start(None, 5_000, Some(retreat));
        let throw = reflex.throw.expect("a way out of every blast was found");
        assert_eq!(throw.close, Some(me.feet));
        assert_eq!(reflex.grenade_stats().close, [1, 1, 0, 0, 0, 1, 0]);
        // No avoidance, no way out known: no throw.
        assert!(start(None, 5_000, None).throw.is_none());
        // A mate 11 m from the place (across, so off the roll's lane) is refused for; at 13 m he is not.
        let refused = start(Some((Vec3::new(31.0, 0.0, 18.0), [0.0, 0.0], 5_000)), 5_000, Some(retreat));
        assert!(refused.throw.is_none());
        assert_eq!(refused.grenade_stats().close, [1, 1, 1, 0, 0, 0, 0]);
        assert!(start(Some((Vec3::new(33.0, 0.0, 18.0), [0.0, 0.0], 5_000)), 5_000, Some(retreat)).throw.is_some());
        // The same mate at 13 m walking in (5 m/s for the 3 s of the lead): refused.
        assert!(start(Some((Vec3::new(33.0, 0.0, 18.0), [-5.0, 0.0], 5_000)), 5_000, Some(retreat)).throw.is_none());
        // A mate 13 m off and fixed 2 s ago (the limit is 1.5 s): his place is not known, refused for him.
        let stale = start(Some((Vec3::new(33.0, 0.0, 18.0), [0.0, 0.0], 3_000)), 5_000, Some(retreat));
        assert!(stale.throw.is_none());
        assert_eq!(stale.grenade_stats().close, [1, 1, 0, 1, 0, 0, 0]);
        // A reaction that takes up the fuse leaves no walk.
        let slow = start(None, 5_000, Some(Retreat { margin_m: 0.5, told_ms: 3_500 }));
        assert!(slow.throw.is_none());
        assert_eq!(slow.grenade_stats().close, [1, 1, 0, 0, 1, 0, 0]);
    }

    /// [close] A roll with only a close place waits `grenade_close_pace.roll_ms` for the next and says yes at its chance; with the
    /// pace unset it waits `grenade_roll_ms`.
    #[test]
    fn a_roll_with_only_a_close_place_is_paced_by_the_close_pace() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let me = body(0, 0, Vec3::new(20.0, 0.0, 10.0), 0.0);
        let retreat = Retreat { margin_m: 0.5, told_ms: 400 };
        let roll = |pace: Option<ClosePace>| {
            let combat = CombatTuning { grenade_close: true, grenade_trace: true, grenade_close_pace: pace, ..combat_off() };
            let aim = AimTuning { combat, ..crate::v026g::bots::tuning::Tuning::all_off().aim };
            let view = view_of(vec![me.clone()]);
            let memory = Memory::default();
            let sit = Situation { tactics: Some(&tactics), side: Some(0), retreat: Some(retreat), ..Situation::of(&view) };
            let look = RollView { me: &me, sit: &sit, memory: &memory, now: 5_000 };
            let mut reflex = Reflex::default();
            reflex.held.push((9, Vec3::new(20.0, 0.0, 18.0), 4_000));
            reflex.grenade_roll(look, &aim, 1.0, &mut Rng::new(1));
            reflex
        };
        let paced = roll(Some(ClosePace { roll_ms: 1_000, chance: 1.0 }));
        assert_eq!(paced.next_grenade_roll, 6_000);
        assert_eq!(paced.grenade_stats().close[0], 1, "the roll reached the close throw at its chance");
        let own = roll(None);
        assert_eq!(own.next_grenade_roll, 5_000 + combat_off().grenade_roll_ms);
    }

    /// [close] The throw is made where the way out was found, and given up when he walked off it: the floor of a close
    /// place is not the floor of `grenade_land_m`.
    #[test]
    fn a_close_throw_is_let_go_from_where_the_way_out_was_found() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let place = Vec3::new(20.0, 0.0, 18.0);
        let me = body(0, 0, Vec3::new(20.0, 0.0, 10.0), 0.0);
        let view = view_of(vec![me.clone()]);
        let memory = Memory::default();
        let close = CombatTuning {
            grenade_close: true,
            ..combat_off()
        };
        let retreat = Retreat {
            margin_m: 0.5,
            told_ms: 400,
        };
        let go = |shift: f32| {
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                retreat: Some(retreat),
                ..Situation::of(&view)
            };
            let look = RollView {
                me: &me,
                sit: &sit,
                memory: &memory,
                now: 5_000,
            };
            let mut reflex = Reflex::default();
            reflex.start_close(&look, &close, &[(place, PlaceSource::Lost)], (0.0, 0.0), 600);
            assert!(reflex.throw.is_some());
            let mut me = me.clone();
            me.feet.x += shift;
            let aim = AimTuning {
                combat: close.clone(),
                ..AimTuning::default()
            };
            let mut rng = Rng::new(3);
            for k in 0..100 {
                let o = reflex.tick(&me, &view, &memory, 5_000 + k * 50, true, &aim, 0.5, &mut rng);
                if let Some(look) = o.look_at {
                    facing(&mut me, look);
                }
            }
            (reflex.grenades_thrown(), reflex.grenade_stats(), reflex.take_throw_notes())
        };
        let (thrown, stats, notes) = go(0.0);
        assert_eq!((thrown, stats.close[6]), (1, 1), "{stats:?}");
        assert!(notes.iter().any(|n| n.starts_with("throws a close grenade, blast at ")), "{notes:?}");
        let (thrown, stats, _) = go(3.0);
        assert_eq!((thrown, stats.close[6], stats.given_up[0]), (0, 0, 1), "{stats:?}");
    }

    /// [close] The switch is off by default, and a roll never makes a close throw with it off.
    #[test]
    fn close_throws_are_off_by_default() {
        assert!(!CombatTuning::default().grenade_close);
    }

    /// [close] The close counts are summed over lives and read as a change since the last reading.
    #[test]
    fn the_close_counts_are_added_and_read_as_a_change() {
        let one = GrenadeStats {
            close: [1, 2, 3, 4, 5, 6, 7],
            ..GrenadeStats::default()
        };
        let mut sum = GrenadeStats::default();
        sum.add(&one);
        sum.add(&one);
        assert_eq!(sum.close, [2, 4, 6, 8, 10, 12, 14]);
        assert_eq!(sum.since(Some(&one)).0.close, [1, 2, 3, 4, 5, 6, 7]);
    }

    /// Match 0x60aeff4f: 2 of the 5 team kills were the bots' own
    /// grenades. A teammate who walks at the place once the key is down
    /// (clear of it at the decision) has the key let go and the throw given
    /// up; with the re-check off the grenade goes. A throw let go sends the
    /// legs in only once it has gone off (the RGD-5's `ttl_ms`).
    #[test]
    fn a_grenade_is_held_back_from_a_teammate_walking_into_its_blast() {
        let camper_at = Vec3::new(0.0, 0.0, 18.0);
        let mut camper = body(1, 1, camper_at, 0.0);
        camper.keys = key::CROUCH | key::AIM;
        let fuse = grenade_fuse_ms().expect("the RGD-5's config");
        assert_eq!(fuse, 4_000);
        // (walks in, recheck) -> (throws, held back, the legs' push from)
        let run = |walks_in: bool, recheck: bool| {
            let mut me = body(0, 0, Vec3::ZERO, 0.0);
            let aim = AimTuning {
                combat: CombatTuning {
                    grenade_chance: 1.0,
                    grenade_roll_ms: 500,
                    grenade_recheck: recheck,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            // A teammate standing 11 m from the place, beside it.
            let still = body(3, 0, Vec3::new(11.0, 0.0, 18.0), 0.0);
            // ... then 9 m off, walking at it at 3 m/s.
            let mut walking = body(3, 0, Vec3::new(9.0, 0.0, 18.0), 0.0);
            walking.velocity = [-3.0, 0.0];
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(10);
            let mut mate = still.clone();
            let view = |me: &PlaceView, mate: &PlaceView| {
                view_of(vec![me.clone(), camper.clone(), mate.clone()])
            };
            for now in [1_000, 1_300] {
                sense(
                    &mut memory,
                    &[percept(1, camper_at, now, Source::Seen)],
                    now,
                );
                reflex.think_with(
                    &me,
                    &Situation::of(&view(&me, &mate)),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            let (mut throws, mut held_back, mut pushed) = (Vec::new(), 0, None);
            let mut first_press = None;
            let mut now = 1_800;
            while now < 14_000 {
                sense(&mut memory, &[], now);
                let view = view(&me, &mate);
                if now % 250 == 0 {
                    reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, 0.5, &mut rng);
                }
                let before = reflex.grenades_thrown();
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                if let Some(look) = o.look_at.filter(|_| o.keys & key::GRENADE != 0) {
                    facing(&mut me, look);
                    first_press.get_or_insert(now);
                    if walks_in {
                        mate = walking.clone();
                    }
                } else if let Some(look) = o.look_at.filter(|_| o.error != (0.0, 0.0)) {
                    facing(&mut me, look);
                }
                if reflex.grenades_thrown() > before {
                    throws.push(now);
                }
                if pushed.is_none()
                    && o.legs.is_some_and(|l| l.why == LegsWhy::AfterGrenade)
                {
                    pushed = Some(now);
                }
                held_back += reflex
                    .take_throw_notes()
                    .iter()
                    .filter(|n| n.starts_with("holds a grenade back") && n.ends_with("teammate 3"))
                    .count();
                now += 50;
            }
            (throws, held_back, pushed, first_press)
        };
        let (throws, held_back, ..) = run(true, true);
        assert!(throws.is_empty(), "{throws:?}");
        assert!(held_back >= 1);
        let (throws, held_back, ..) = run(true, false);
        assert_eq!(throws.len(), 2, "{throws:?}");
        assert_eq!(held_back, 0);
        // Nobody near: thrown, and in only after the blast (the second
        // throw's push replaces the first's: the bounds are the first
        // press's).
        let (throws, held_back, pushed, press) = run(false, true);
        assert_eq!((throws.len(), held_back), (2, 0), "{throws:?}");
        let pushed = pushed.expect("the legs go in after the throw");
        let press = press.expect("the key goes down");
        assert!(throws[0] > press);
        assert!(pushed >= press + fuse, "{pushed} after a press at {press}");
    }

    /// What the roll, the weighing and the throw counted ([`GrenadeStats`]):
    /// a camper 20 m ahead on level ground is a held place at every roll, the
    /// chance is 1, so every roll weighs it and it passes; each throw is
    /// counted as the life's 1st, 2nd.
    #[test]
    fn the_grenade_counts_follow_the_roll_the_weighing_and_the_throw() {
        let run = throw_run(20.0, &[], CombatTuning::default(), None, 12_000);
        let s = run.stats;
        assert!(s.thinks > 0 && s.rolls > 0, "{s:?}");
        assert_eq!(s.chance_no, 0);
        assert_eq!(s.places, [s.rolls, 0, 0, 0, 0], "one held place a roll, no camp, no track");
        assert_eq!(s.weighed, [s.rolls, 0, 0, 0, 0]);
        let refused: u32 = s.refused[0].iter().sum();
        assert_eq!(s.passed[0][0] + refused, s.weighed[0], "{s:?}");
        assert_eq!(s.nth.iter().sum::<u32>() as usize, run.thrown.len(), "{s:?}");
        assert_eq!(s.nth[0], 1, "{s:?}");
        assert!(run.thrown_ago.is_some_and(|ago| ago < 12_000));
    }

    /// [G2] A wall 8 m ahead and 3 m high: the flat arc to a place 21 m
    /// ahead (18 degrees, 2.7 m over the wall) meets it inside the thrower's
    /// blast; with the search a lob over it is found, another pitch at the
    /// same yaw (21-25 degrees: a lob over cover is a 20 m throw, the pitch
    /// is clamped at 57 degrees); one trace is not enough to find it.
    #[test]
    fn the_search_finds_a_lob_over_a_wall_the_flat_arc_meets() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let wall = |from: Vec3, dir: Vec3, max_t: f32| {
            if dir.z > 0.0 && from.z < 8.0 {
                let t = (8.0 - from.z) / dir.z;
                if t <= max_t && (from.x + dir.x * t).abs() < 8.0 && from.y + dir.y * t < 3.0 {
                    return Some(arc::Hit {
                        t,
                        normal: Vec3::new(0.0, 0.0, -1.0),
                    });
                }
            }
            arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t)
        };
        let place = Vec3::new(0.0, 0.0, 21.0);
        let off = combat_off();
        assert!(!off.grenade_search);
        assert_eq!(
            weigh_place(&me, &view, place, (0.0, 0.0), &off, &wall),
            Err(Refuse::Thrower)
        );
        let on = CombatTuning {
            grenade_search: true,
            ..combat_off()
        };
        let Ok(ArcPick::Pitch { pitch }) = weigh_place(&me, &view, place, (0.0, 0.0), &on, &wall)
        else {
            panic!("a lob over the wall is found");
        };
        assert!((19.0..32.0).contains(&pitch.to_degrees()), "{}", pitch.to_degrees());
        let capped = CombatTuning {
            grenade_search_traces: 1,
            ..on
        };
        assert_eq!(
            weigh_place(&me, &view, place, (0.0, 0.0), &capped, &wall),
            Err(Refuse::Thrower)
        );
    }

    /// [G2] A wall across the yaw of the place but not beside it: another
    /// yaw is found; a teammate in the way is no case for the search.
    #[test]
    fn the_search_turns_the_yaw_when_the_pitch_will_not_do() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        // A wall 6 m ahead, 20 m high, between x = -1 and 1; the place is 20 m out.
        let wall = |from: Vec3, dir: Vec3, max_t: f32| {
            if dir.z > 0.0 && from.z < 6.0 {
                let t = (6.0 - from.z) / dir.z;
                if t <= max_t && (from.x + dir.x * t).abs() < 1.0 && from.y + dir.y * t < 20.0 {
                    return Some(arc::Hit {
                        t,
                        normal: Vec3::new(0.0, 0.0, -1.0),
                    });
                }
            }
            arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t)
        };
        let place = Vec3::new(1.0, 0.0, 20.0);
        let on = CombatTuning {
            grenade_search: true,
            grenade_search_off_m: 8.0,
            ..combat_off()
        };
        let Ok(pick @ ArcPick::Yawed { dyaw, .. }) =
            weigh_place(&me, &view, place, (0.0, 0.0), &on, &wall)
        else {
            panic!("another yaw is found");
        };
        assert_ne!(dyaw, 0.0);
        assert_eq!(pick.kind(), 2);
        // Off: refused, as before.
        assert_eq!(
            weigh_place(&me, &view, place, (0.0, 0.0), &combat_off(), &wall),
            Err(Refuse::Thrower)
        );
    }

    /// [G1] An enemy seen a moment ago and gone, holding no angle and no
    /// camp, is a place with `grenade_tracks` (counted as a lost place):
    /// thrown at. Without it, nothing is rolled.
    #[test]
    fn a_target_just_lost_is_a_place_with_grenade_tracks() {
        let at = Vec3::new(0.0, 0.0, 20.0);
        let on = CombatTuning {
            grenade_tracks: true,
            grenade_track_chance: 1.0,
            ..combat_off()
        };
        let run = throw_run_keyed(&[at], &[], on, None, 6_000, None, 0);
        assert!(!run.thrown.is_empty(), "{:?}", run.stats);
        assert_eq!(run.stats.places[0] + run.stats.places[1], 0, "{:?}", run.stats);
        assert!(run.stats.places[2] > 0, "{:?}", run.stats);
        let off = throw_run_keyed(&[at], &[], combat_off(), None, 6_000, None, 0);
        assert!(off.thrown.is_empty());
        assert_eq!(off.stats.rolls, 0);
    }

    /// [G1] A teammate's claim within `grenade_claim_m` of the place drops it
    /// from the roll (counted); the bot's own claim does not; a decision
    /// leaves a claim for the team.
    #[test]
    fn a_teammates_claim_drops_the_place_and_a_decision_makes_one() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let mut camper = body(1, 1, Vec3::new(0.0, 0.0, 20.0), 0.0);
        camper.keys = key::CROUCH | key::AIM;
        let view = view_of(vec![me.clone(), camper.clone()]);
        let aim = AimTuning {
            combat: CombatTuning {
                grenade_claim: true,
                grenade_chance: 1.0,
                grenade_roll_ms: 500,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let run = |team: &TeamKnowledge| {
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(10);
            let sit = Situation {
                team: Some(team),
                ..Situation::of(&view)
            };
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, camper.feet, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            for now in [1_800, 2_300, 2_800] {
                sense(&mut memory, &[], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            (reflex.grenade_stats(), reflex.take_throw_claim())
        };
        let mut team = TeamKnowledge::default();
        let (stats, claim) = run(&team);
        assert!(claim.is_some(), "{stats:?}");
        assert_eq!(stats.claimed, 0);
        team.claim_throw(7, Vec3::new(2.0, 0.0, 22.0), 1_500);
        let (stats, claim) = run(&team);
        assert_eq!(claim, None, "{stats:?}");
        assert!(stats.claimed > 0, "{stats:?}");
        let mut own = TeamKnowledge::default();
        own.claim_throw(0, Vec3::new(2.0, 0.0, 22.0), 1_500);
        assert!(run(&own).1.is_some(), "his own volley is not held against him");
    }

    /// [A170] A grenade at a held threat spot: a bot with a frag outside the shooter's lanes, within `grenade_m` of the spot,
    /// whose shooter the team sensed on it in the last 5 s (`SENSED_MS`), and no teammate's claim on the place.
    #[test]
    fn a_held_spot_whose_shooter_was_sensed_lately_is_thrown_at_from_outside_its_lanes() {
        use super::super::threat::{HeldLine, Picture};
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let spot = Vec3::new(0.0, 0.0, 22.0);
        let aim = |at_held: Option<f32>| AimTuning {
            combat: CombatTuning {
                grenade_claim: true,
                grenade_tracks: false,
                grenade_chance: 0.0,
                grenade_roll_ms: 500,
                grenade_at_held: at_held,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        // His victims are down the +x side: the thrower at the origin, 22 m before him, is off his lanes.
        let victims = [Vec3::new(40.0, 0.0, 22.0)];
        let team_with = |seen: Option<u32>, victims: &[Vec3]| {
            let mut team = TeamKnowledge::default();
            team.threat = Arc::new(Picture {
                held: vec![HeldLine::for_test(3, 9, spot, victims, seen)],
                jobs: Vec::new(),
                sniper_regions: Vec::new(),
                snipers: Vec::new(),
                concede: Default::default(),
            });
            team
        };
        let run = |team: &TeamKnowledge, aim: &AimTuning, times: &[u32]| {
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(10);
            let sit = Situation {
                team: Some(team),
                ..Situation::of(&view)
            };
            for &now in times {
                sense(&mut memory, &[], now);
                reflex.think_with(&me, &sit, &memory, now, aim, 0.5, &mut rng);
            }
            (reflex.grenade_stats(), reflex.take_throw_claim())
        };
        let times = [1_800, 2_300, 2_800];
        // Sensed 1.5 s ago: thrown at, from the held spot's source.
        let (stats, claim) = run(&team_with(Some(1_500), &victims), &aim(Some(1.0)), &times);
        assert!(claim.is_some() && stats.places[PlaceSource::Spot.index()] > 0, "{stats:?}");
        // The funnel counts the furthest a spot got in each think that reached the roll: all the way and a frag (the kit's
        // `frags_left` is unknown: a frag), then the roll's lockout stops the others.
        assert!(stats.held_funnel.iter().all(|&n| n == 1), "{stats:?}");
        assert!(stats.held_busy[3] > 0 || stats.held_busy[0] > 0, "{stats:?}");
        // The switch off: no place at all (the chance for the other sources is 0).
        let (stats, claim) = run(&team_with(Some(1_500), &victims), &aim(None), &times);
        assert!(claim.is_none() && stats.places == [0; GRENADE_SOURCES], "{stats:?}");
        // Sensed more than 5 s ago, or never since his death: not thrown at.
        let late = [8_000, 8_500, 9_000];
        let (stats, claim) = run(&team_with(Some(1_500), &victims), &aim(Some(1.0)), &late);
        assert!(claim.is_none() && stats.places == [0; GRENADE_SOURCES], "{stats:?}");
        assert_eq!(stats.held_funnel, [3, 0, 0, 0, 0, 0, 0], "{stats:?}");
        let (stats, _) = run(&team_with(None, &victims), &aim(Some(1.0)), &times);
        assert_eq!(stats.places, [0; GRENADE_SOURCES], "{stats:?}");
        // The thrower in one of his lanes: not thrown from there.
        let (stats, claim) = run(&team_with(Some(1_500), &[Vec3::ZERO]), &aim(Some(1.0)), &times);
        assert!(claim.is_none() && stats.places == [0; GRENADE_SOURCES], "{stats:?}");
        assert_eq!(stats.held_funnel, [3, 3, 3, 0, 0, 0, 0], "{stats:?}");
        // A teammate's claim on the place within a few seconds: left to him.
        let mut claimed = team_with(Some(1_500), &victims);
        claimed.claim_throw(7, spot, 1_500);
        let (stats, claim) = run(&claimed, &aim(Some(1.0)), &times);
        assert!(claim.is_none() && stats.claimed > 0, "{stats:?}");
        assert_eq!(stats.held_funnel[4..], [3, 0, 0], "{stats:?}");
    }

    /// [A170] A held spot is a place out to the frag's real reach (about 30 m from the eye over level ground) with
    /// `grenade_held_reach`, 27 m without; beyond the reach, none.
    #[test]
    fn a_held_spot_is_a_place_out_to_the_frags_real_reach() {
        use super::super::threat::{HeldLine, Picture};
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let reach = arc::Grenade::bots().max_range(me.eye().y).expect("a throw reaches the ground");
        assert!(reach > 29.0 && reach < 32.0, "{reach}");
        let places = |distance: f32, held_reach: bool| {
            let spot = Vec3::new(0.0, 0.0, distance);
            let mut team = TeamKnowledge::default();
            team.threat = Arc::new(Picture {
                held: vec![HeldLine::for_test(3, 9, spot, &[Vec3::new(40.0, 0.0, distance)], Some(1_500))],
                jobs: Vec::new(),
                sniper_regions: Vec::new(),
                snipers: Vec::new(),
                concede: Default::default(),
            });
            let aim = AimTuning {
                combat: CombatTuning {
                    grenade_tracks: false,
                    grenade_chance: 0.0,
                    grenade_held_reach: held_reach,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(10);
            let sit = Situation {
                team: Some(&team),
                ..Situation::of(&view)
            };
            for now in [1_800, 2_300, 2_800] {
                sense(&mut memory, &[], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            reflex.grenade_stats().places[PlaceSource::Spot.index()]
        };
        assert!(places(25.0, false) > 0 && places(25.0, true) > 0);
        assert_eq!(places(29.0, false), 0, "beyond the 27 m of the other places");
        assert!(places(29.0, true) > 0, "inside the reach");
        assert_eq!(places(reach + 2.0, true), 0, "beyond the reach");
    }

    /// [A170] A throw at a held spot the team's picture drops before the key is let go is given up, the frag kept and counted;
    /// without `grenade_held_recheck` it goes on.
    #[test]
    fn a_throw_at_a_held_spot_the_team_dropped_is_given_up() {
        use super::super::threat::{HeldLine, Picture};
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let spot = Vec3::new(0.0, 0.0, 22.0);
        let team_with = |held: bool| {
            let mut team = TeamKnowledge::default();
            team.threat = Arc::new(Picture {
                held: if held { vec![HeldLine::for_test(3, 9, spot, &[Vec3::new(40.0, 0.0, 22.0)], Some(1_500))] } else { Vec::new() },
                jobs: Vec::new(),
                sniper_regions: Vec::new(),
                snipers: Vec::new(),
                concede: Default::default(),
            });
            team
        };
        let run = |recheck: bool| {
            let aim = AimTuning {
                combat: CombatTuning {
                    grenade_tracks: false,
                    grenade_chance: 0.0,
                    grenade_held_recheck: recheck,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(10);
            let (with, without) = (team_with(true), team_with(false));
            sense(&mut memory, &[], 1_800);
            let sit = Situation {
                team: Some(&with),
                ..Situation::of(&view)
            };
            reflex.think_with(&me, &sit, &memory, 1_800, &aim, 0.5, &mut rng);
            assert_eq!(reflex.throw.map(|throw| (throw.source, throw.spot)), Some((PlaceSource::Spot, Some(3))));
            let sit = Situation {
                team: Some(&without),
                ..Situation::of(&view)
            };
            sense(&mut memory, &[], 1_850);
            reflex.think_with(&me, &sit, &memory, 1_850, &aim, 0.5, &mut rng);
            (reflex.throw.is_some(), reflex.grenade_stats().held_dropped, reflex.thrown)
        };
        assert_eq!(run(true), (false, [1, 0], 0));
        assert_eq!(run(false), (true, [0, 0], 0));
    }

    /// The arc searched again at the press: only with `grenade_repick` (and
    /// the search), only for what is in the way, counted; the flat arc if it
    /// passes from where the thrower stands now.
    #[test]
    fn a_failed_recheck_is_searched_again_only_when_asked_and_for_what_is_in_the_way() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let throw = Throw {
            at: Vec3::new(0.0, 0.0, 20.0),
            started: 0,
            hold_ms: 500,
            pressed: None,
            error: (0.0, 0.0),
            released: false,
            pitch: None,
            pick: ArcPick::Pitch { pitch: 0.5 },
            source: PlaceSource::Heard,
            kind: ThrowKind::Roll,
            near: None,
            close: None,
            spot: None,
        };
        let on = CombatTuning {
            grenade_search: true,
            grenade_repick: true,
            ..combat_off()
        };
        let mut reflex = Reflex::default();
        assert_eq!(
            reflex.repick(None, &throw, &me, &view, &on, Refuse::Thrower),
            Some(ArcPick::Flat)
        );
        assert_eq!(reflex.grenade_stats().rechecks, [1, 0]);
        assert_eq!(reflex.repick(None, &throw, &me, &view, &on, Refuse::Mate(3)), None);
        assert_eq!(reflex.repick(None, &throw, &me, &view, &combat_off(), Refuse::Thrower), None);
        assert_eq!(reflex.grenade_stats().rechecks, [1, 0], "no search, no count");
    }

    #[test]
    fn the_grenade_counts_since_a_reading_and_a_new_life() {
        let mut a = GrenadeStats {
            thinks: 10,
            rolls: 2,
            ..GrenadeStats::default()
        };
        a.places[1] = 3;
        a.nth[0] = 1;
        let mut b = a;
        b.thinks = 14;
        b.rolls = 3;
        b.places[1] = 5;
        b.refused[1][0] = 1;
        b.volley[1] = 2;
        b.near[0] = 1;
        let (change, fresh) = b.since(Some(&a));
        assert!(!fresh);
        assert_eq!((change.thinks, change.rolls, change.places[1]), (4, 1, 2));
        assert_eq!((change.refused[1][0], change.nth[0]), (1, 0));
        assert_eq!((change.volley[1], change.near[0]), (2, 1));
        // A younger reflex is a new life's: its whole counts; so is the first reading.
        let young = GrenadeStats {
            born: 7,
            thinks: 3,
            rolls: 1,
            ..GrenadeStats::default()
        };
        assert_eq!(young.since(Some(&b)), (young, true));
        assert_eq!(young.since(None), (young, true));
        // The same reflex whose counts changed while no think reached the roll (a throw under way: the release, a give-up) is
        // the same life: only the change is added, nothing counted twice.
        let mut held = b;
        held.nth[0] += 1;
        held.thrown_from[3] += 1;
        let (later, new_life) = held.since(Some(&b));
        assert!(!new_life);
        assert_eq!((later.thinks, later.rolls, later.places[1], later.nth[0], later.thrown_from[3]), (0, 0, 0, 1, 1));
        // Every reflex made for a life has its own number.
        assert_ne!(Reflex::armed(None).grenade_stats().born, Reflex::armed(None).grenade_stats().born);
        // Added up, the changes make the last reading.
        let mut sum = GrenadeStats::default();
        sum.add(&a);
        sum.add(&change);
        assert_eq!(sum, b);
    }

    /// A bot at the origin facing +z, throwing at a camper `distance` m
    /// ahead (a level ground at 0 for a reflex never told a map, else
    /// `world`'s mesh), with `mates` about; its view turns to the look at
    /// once. The ticks from 1.8 s to `until`, and the notes made.
    struct ThrowRun {
        ticks: Vec<(u32, Override)>,
        notes: Vec<String>,
        /// The same with the tick each was made at.
        timed: Vec<(u32, String)>,
        /// Whether the view was held for the grenade after each tick.
        held: Vec<(u32, bool)>,
        thrown: Vec<u32>,
        /// The reflex's grenade counts at the end.
        stats: GrenadeStats,
        /// Ms from the last throw to the end of the run.
        thrown_ago: Option<u32>,
    }

    fn throw_run(
        distance: f32,
        mates: &[PlaceView],
        combat: CombatTuning,
        world: Option<&Arc<mapdata::World>>,
        until: u32,
    ) -> ThrowRun {
        throw_run_at(
            &[Vec3::new(0.0, 0.0, distance)],
            mates,
            combat,
            world,
            until,
        )
    }

    /// The same with an enemy holding an angle at each of `campers`.
    fn throw_run_at(
        campers: &[Vec3],
        mates: &[PlaceView],
        combat: CombatTuning,
        world: Option<&Arc<mapdata::World>>,
        until: u32,
    ) -> ThrowRun {
        throw_run_stepping(campers, mates, combat, world, until, None)
    }

    /// The same, the thrower stepping down `m` metres `ms` after the press.
    fn throw_run_stepping(
        campers: &[Vec3],
        mates: &[PlaceView],
        combat: CombatTuning,
        world: Option<&Arc<mapdata::World>>,
        until: u32,
        step_down: Option<(u32, f32)>,
    ) -> ThrowRun {
        throw_run_keyed(
            campers,
            mates,
            combat,
            world,
            until,
            step_down,
            key::CROUCH | key::AIM,
        )
    }

    /// The same, the enemies holding `camper_keys` (crouched and aiming
    /// hold an angle; none holds nothing).
    fn throw_run_keyed(
        campers: &[Vec3],
        mates: &[PlaceView],
        combat: CombatTuning,
        world: Option<&Arc<mapdata::World>>,
        until: u32,
        step_down: Option<(u32, f32)>,
        camper_keys: u32,
    ) -> ThrowRun {
        let mut pressed_at: Option<u32> = None;
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let campers: Vec<PlaceView> = campers
            .iter()
            .enumerate()
            .map(|(i, at)| {
                let mut camper = body(i as Place + 1, 1, *at, 0.0);
                camper.keys = camper_keys;
                camper
            })
            .collect();
        let aim = AimTuning {
            combat: CombatTuning {
                grenade_chance: 1.0,
                grenade_roll_ms: 500,
                ..combat
            },
            ..AimTuning::default()
        };
        let view = |me: &PlaceView| {
            let mut places = vec![me.clone()];
            places.extend(campers.iter().cloned());
            places.extend_from_slice(mates);
            view_of(places)
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(10);
        for now in [1_000, 1_300] {
            let seen: Vec<_> = campers
                .iter()
                .enumerate()
                .map(|(i, c)| percept(i as Place + 1, c.feet, now, Source::Seen))
                .collect();
            sense(&mut memory, &seen, now);
            reflex.think_with(&me, &Situation::of(&view(&me)), &memory, now, &aim, 0.5, &mut rng);
        }
        let mut run = ThrowRun {
            ticks: Vec::new(),
            notes: Vec::new(),
            timed: Vec::new(),
            held: Vec::new(),
            thrown: Vec::new(),
            stats: GrenadeStats::default(),
            thrown_ago: None,
        };
        let mut now = 1_800;
        while now < until {
            if let (Some((ms, m)), Some(pressed)) = (step_down, pressed_at)
                && now >= pressed + ms
            {
                me.feet.y = -m;
            }
            sense(&mut memory, &[], now);
            let view = view(&me);
            if now % 250 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, &aim, 0.5, &mut rng);
            }
            if let Some(world) = world {
                reflex.look(&me, &view, world, &mut memory, &SenseTuning::default(), now);
            }
            let before = reflex.grenades_thrown();
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
            if o.keys & key::GRENADE != 0 {
                pressed_at.get_or_insert(now);
            }
            if let Some(look) = o.look_at {
                facing(&mut me, look);
            }
            if reflex.grenades_thrown() > before {
                run.thrown.push(now);
            }
            for note in reflex.take_throw_notes() {
                run.timed.push((now, note.clone()));
                run.notes.push(note);
            }
            run.held.push((now, reflex.view_held()));
            run.ticks.push((now, o));
            now += 50;
        }
        run.stats = reflex.grenade_stats();
        run.thrown_ago = reflex.thrown_ago(until);
        run
    }

    /// The first tick with the grenade key down.
    fn pressed_at(run: &ThrowRun) -> Option<(u32, Override)> {
        run.ticks
            .iter()
            .copied()
            .find(|(_, o)| o.keys & key::GRENADE != 0)
    }

    /// Where a throw of the look `look` (plus `error`) from a standing
    /// thrower at the origin first touches `ground`.
    fn landing_of(look: Vec3, error: (f32, f32), ground: &impl arc::Ground) -> arc::Landing {
        let eye = still_eye(Vec3::ZERO, false);
        let yaw = yaw_of([look.x - eye.x, look.z - eye.z]) + error.0;
        let pitch = (look.y - eye.y).atan2(look.distance_xz(eye)) + error.1;
        arc::Flight::thrown(eye, yaw, pitch)
            .landing(ground)
            .expect("it comes down")
    }

    /// A map of a floor at 0, 200 m a side, and `walls`.
    fn floor_and(walls: &[[Vec3; 3]]) -> Arc<mapdata::World> {
        let (a, b) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 0.0, -100.0));
        let (c, d) = (Vec3::new(100.0, 0.0, 100.0), Vec3::new(-100.0, 0.0, 100.0));
        let mut triangles = vec![[a, b, c], [a, c, d]];
        triangles.extend_from_slice(walls);
        Arc::new(mapdata::World {
            map: mapdata::MapData::default(),
            grid: Arc::new(mapdata::WalkGrid::build(
                &[],
                [0.0, 0.0, 1.0, 1.0],
                0.25,
                mapdata::Body::default(),
            )),
            sight: Arc::new(mapdata::RayTree::from_corners(triangles)),
        })
    }

    /// A wall across the way in the plane z, 10 m high.
    fn wall_at(z: f32) -> [[Vec3; 3]; 2] {
        let (a, b) = (Vec3::new(-100.0, 0.0, z), Vec3::new(100.0, 0.0, z));
        let (c, d) = (Vec3::new(100.0, 10.0, z), Vec3::new(-100.0, 10.0, z));
        [[a, b, c], [a, c, d]]
    }

    /// X9: the bots aimed a flat line (the place raised by `1.0 + 0.12 d`,
    /// 2.6 to 5.7 degrees) while the client throws along the view at 17 m/s:
    /// the first contact was 10.7 to 12.5 m out whatever the distance. The
    /// pitch now is the one whose flight comes down at the place.
    #[test]
    fn a_grenade_is_aimed_to_come_down_at_the_place() {
        for distance in [13.0, 20.0, 25.0, 27.0] {
            let run = throw_run(distance, &[], CombatTuning::default(), None, 6_000);
            let (_, press) = pressed_at(&run).expect("thrown");
            let place = Vec3::new(0.0, 0.0, distance);
            let landing = landing_of(press.look_at.unwrap(), (0.0, 0.0), &arc::Floor { y: 0.0 });
            let miss = landing.contact.point.distance_xz(place);
            assert!(miss < 1.0, "{distance} m: first contact {miss} m off");
            // With the error the skill draws: still within the blast's
            // lethal reach.
            let landing = landing_of(press.look_at.unwrap(), press.error, &arc::Floor { y: 0.0 });
            let miss = landing.contact.point.distance_xz(place);
            assert!(miss < 2.4, "{distance} m with the error: {miss} m off");
        }
        // The old line: 12 m out, 8 m short of a place at 20.
        let old = CombatTuning {
            grenade_trace: false,
            ..CombatTuning::default()
        };
        let run = throw_run(20.0, &[], old, None, 6_000);
        let (_, press) = pressed_at(&run).expect("thrown");
        let landing = landing_of(press.look_at.unwrap(), (0.0, 0.0), &arc::Floor { y: 0.0 });
        assert!(
            (11.0..13.0).contains(&landing.contact.point.z),
            "{:?}",
            landing.contact.point
        );
    }

    /// The launch is about 1.1 s after the press, and the view then is the
    /// one that counts: the bots let go of the key at 0.5 to 0.8 s and
    /// turned back, so the view was level by the launch.
    #[test]
    fn the_look_and_the_legs_are_held_until_the_grenade_has_left_the_hand() {
        let c = CombatTuning::default();
        let run = throw_run(20.0, &[], c.clone(), None, 8_000);
        let (press, first) = pressed_at(&run).expect("thrown");
        let look = first.look_at.unwrap();
        let end = press + c.grenade_launch_ms + c.grenade_look_after_ms;
        let mut down = 0;
        for (now, o) in run
            .ticks
            .iter()
            .filter(|(now, _)| (press..end).contains(now))
        {
            let held = o.look_at.unwrap_or_else(|| panic!("no look at {now}"));
            assert!((held - look).length() < 1e-3, "{now}: {held:?} for {look:?}");
            assert!(!o.fire, "{now}");
            assert!(
                o.legs.is_some_and(|l| l.why == LegsWhy::Throwing),
                "{now}: {:?}",
                o.legs
            );
            down += u32::from(o.keys & key::GRENADE != 0);
        }
        // The key is down for the hold only (0.5 to 0.8 s), the look for the
        // 1.4 s.
        assert!((10..=16).contains(&down), "{down} ticks down");
        let (_, after) = run.ticks.iter().find(|(now, _)| *now == end).unwrap();
        assert!(after.look_at.is_none() && after.legs.is_none(), "{after:?}");
        // The throw is counted at the let-go, the fuse from the press.
        assert!(run.thrown[0] >= press + c.grenade_hold_ms.0 && run.thrown[0] < end);
        let note = run
            .notes
            .iter()
            .find(|n| n.starts_with("throws a grenade"))
            .expect("noted");
        assert!(note.ends_with(&format!("blast at {}", press + 4_000)), "{note}");
    }

    /// The push after a throw counts from the blast, the press plus the fuse.
    #[test]
    fn the_push_after_a_throw_starts_after_the_blast() {
        // One grenade: a second throw's push would replace the first's.
        let c = CombatTuning {
            grenades: 1,
            ..CombatTuning::default()
        };
        let run = throw_run(20.0, &[], c.clone(), None, 9_000);
        let (press, _) = pressed_at(&run).expect("thrown");
        let pushed = run
            .ticks
            .iter()
            .find(|(_, o)| o.legs.is_some_and(|l| l.why == LegsWhy::AfterGrenade))
            .expect("the legs go in")
            .0;
        let from = press + 4_000 + c.grenade_push_after_ms;
        assert!((from..from + 150).contains(&pushed), "{pushed} for {press}");
    }

    /// A mate beyond the place, on the grenade's line, 8 m past a landing 25
    /// m out: more than the old 6 m from the place, but in the P80 reach of
    /// the roll.
    #[test]
    fn a_mate_on_the_blast_segment_refuses_the_throw() {
        let c = earlier_habit();
        let mate_at = |x: f32, z: f32| body(3, 0, Vec3::new(x, 0.0, z), 0.0);
        // Alone: thrown.
        let alone = throw_run(25.0, &[], c.clone(), None, 6_000);
        assert!(!alone.thrown.is_empty());
        // 8 m past the place on the line, standing: in the blast's reach.
        let past = throw_run(25.0, &[mate_at(0.0, 33.0)], c.clone(), None, 6_000);
        assert!(past.thrown.is_empty(), "{:?}", past.thrown);
        assert!(
            past.notes
                .iter()
                .any(|n| n.starts_with("holds a grenade back") && n.ends_with("teammate 3")),
            "{:?}",
            past.notes
        );
        // 25 m past it, or 8 m to the side: clear of every end of it.
        for (x, z) in [(0.0, 50.0), (8.0, 25.0)] {
            let clear = throw_run(25.0, &[mate_at(x, z)], c.clone(), None, 6_000);
            assert!(!clear.thrown.is_empty(), "{x} {z}: {:?}", clear.notes);
        }
        // 15 m past the landing, 7 m past the P80 end: out of the 6 m round
        // it, but on the line a runaway grenade takes (16 m): refused; 4 m
        // aside he is off it.
        let lane = throw_run(25.0, &[mate_at(0.0, 40.0)], c.clone(), None, 6_000);
        assert!(lane.thrown.is_empty(), "{:?}", lane.notes);
        let beside = throw_run(25.0, &[mate_at(4.0, 40.0)], c.clone(), None, 6_000);
        assert!(!beside.thrown.is_empty(), "{:?}", beside.notes);
        // A mate walking at the landing from 15 m aside: his four seconds'
        // walk (3 m/s) ends in it.
        let mut walking = mate_at(15.0, 25.0);
        walking.velocity = [-3.0, 0.0];
        let walks = throw_run(25.0, &[walking.clone()], c.clone(), None, 6_000);
        assert!(walks.thrown.is_empty(), "{:?}", walks.notes);
        // ... which the old 1.5 s lead did not reach.
        let short = CombatTuning {
            grenade_lead_s: 1.5,
            ..c.clone()
        };
        assert!(!throw_run(25.0, &[walking], short, None, 6_000).thrown.is_empty());
    }

    #[test]
    fn fewer_grenades_are_thrown_with_a_mate_near() {
        let c = CombatTuning::default();
        let throws = |mate: bool| -> Vec<usize> {
            (13..=27)
                .map(|d| {
                    // A mate 6 m past the place, on the line: the same
                    // blast whatever the distance.
                    let mates = [body(3, 0, Vec3::new(0.0, 0.0, d as f32 + 6.0), 0.0)];
                    let mates = if mate { &mates[..] } else { &[] };
                    throw_run(d as f32, mates, c.clone(), None, 8_000).thrown.len()
                })
                .collect()
        };
        let alone = throws(false);
        let with = throws(true);
        assert!(alone.iter().all(|n| *n == 2), "{alone:?}");
        assert!(with.iter().all(|n| *n == 0), "{with:?}");
    }

    /// Walls: the grenade falls at a wall's foot, and a steep wall near the
    /// thrower is not a place to throw at (YaPB refuses a steep wall hit).
    #[test]
    fn a_throw_into_a_wall_near_the_thrower_is_not_made() {
        let c = combat_off();
        let open = floor_and(&[]);
        let run = throw_run(25.0, &[], c.clone(), Some(&open), 6_000);
        assert!(!run.thrown.is_empty(), "{:?}", run.notes);
        let (_, press) = pressed_at(&run).unwrap();
        let landing = landing_of(press.look_at.unwrap(), (0.0, 0.0), &arc::Mesh(&open.sight));
        let miss = landing.contact.point.distance_xz(Vec3::new(0.0, 0.0, 25.0));
        assert!(miss < 1.0, "{:?}", landing.contact);
        // A wall 15 m out: refused, whatever is behind it.
        let walled = floor_and(&wall_at(15.0));
        let run = throw_run(25.0, &[], c.clone(), Some(&walled), 6_000);
        assert!(run.thrown.is_empty());
        assert!(
            run.notes
                .iter()
                .any(|n| n.ends_with("hits a wall near the thrower")),
            "{:?}",
            run.notes
        );
        // A wall 23 m out, 2 m before the place: the thrower is clear of
        // it and the grenade lies at its foot.
        let far = floor_and(&wall_at(23.0));
        let run = throw_run(25.0, &[], c.clone(), Some(&far), 6_000);
        assert!(!run.thrown.is_empty(), "{:?}", run.notes);
    }

    /// The thrower steps down in the hold, after the key is let go, and the
    /// place, near the longest throw and above him, is out of reach from
    /// where he stands now: the grenade still leaves at the launch, and the
    /// look is held to it (it was given back, and the grenade came down 1.1 m
    /// from the thrower, in a smoke).
    #[test]
    fn a_throw_let_go_keeps_its_look_when_the_place_goes_out_of_reach() {
        let eye = still_eye(Vec3::ZERO, false);
        let up = 7.0;
        let d = arc::max_range(arc::hand_of(eye).y - up).unwrap() - 0.3;
        assert!(d > 12.0 && d < 27.0, "{d}");
        let place = Vec3::new(0.0, up, d);
        let c = CombatTuning::default();
        let stepped = |step| throw_run_stepping(&[place], &[], c.clone(), None, 6_000, step);
        // Level, it is thrown.
        let level = stepped(None);
        assert!(!level.thrown.is_empty(), "{:?}", level.notes);
        // A step of half a metre 900 ms after the press: no pitch lands it any more.
        let me = body(0, 0, Vec3::new(0.0, -0.5, 0.0), 0.0);
        assert_eq!(throw_view(&me, place, &c), None);
        let run = stepped(Some((900, 0.5)));
        assert!(!run.thrown.is_empty(), "{:?}", run.notes);
        let (press, _) = pressed_at(&run).unwrap();
        // Not given up between the press and the launch plus the look held
        // (later rolls, from the new height, are refused as they should be).
        let end = press + c.grenade_launch_ms + c.grenade_look_after_ms;
        assert!(
            run.timed
                .iter()
                .all(|(t, n)| !(press..end).contains(t) || !n.contains("holds a grenade back")),
            "{:?}",
            run.timed
        );
        for (now, o) in &run.ticks {
            if (press..press + c.grenade_launch_ms + c.grenade_look_after_ms).contains(now) {
                assert!(o.look_at.is_some() && o.legs.is_some(), "{now}: {o:?}");
            }
        }
    }

    /// [smoke] A smoke throw: refused with no smoke in the kit, or with a target unless it is the sniper's; begun, the
    /// bot stands and turns, holds the smoke's quick-use key the hold's middle, lets go (a note), keeps the look until
    /// the launch and the look after, and is over; given up when the view never comes round.
    #[test]
    fn a_smoke_is_thrown_with_its_quick_use_key_held_then_let_go() {
        use crate::v026g::bots::smoke::{SmokeAim, SmokeLine, SmokePlan, SmokeUse};
        let c = CombatTuning::default();
        let pitch = 0.3;
        let plan = |for_use| SmokePlan {
            at: Vec3::new(0.0, 0.0, 20.0),
            aim: SmokeAim {
                yaw: 0.0,
                pitch,
                rest: Vec3::new(0.0, 0.0, 20.0),
            },
            for_use,
            covers_m: 4.0,
            line: SmokeLine { eye: Vec3::new(40.0, 1.6, 20.0), target: Vec3::new(0.0, 1.3, 20.0), crossing: Vec3::new(0.0, 0.0, 20.0) },
        };
        let key = key::QUICK[3];
        let mut reflex = Reflex::default();
        assert!(!reflex.start_smoke(plan(SmokeUse::CrossLane), 0, None), "no smoke in the kit");
        reflex.set_smoke_key(Some(key));
        reflex.target = Some(1);
        assert!(!reflex.start_smoke(plan(SmokeUse::CrossLane), 0, None), "fighting");
        assert!(reflex.start_smoke(plan(SmokeUse::AntiSniper), 0, None), "the sniper it is outranged by, its target");
        reflex.target = None;
        assert_eq!((reflex.lane_smokes(), reflex.sniper_smokes()), (0, 1), "a sniper's smoke is not a lane's");
        assert!(!reflex.can_smoke(), "one at a time");
        // Facing the plan's view: pressed at once, held, let go, the look kept to the launch and after.
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        me.pitch = pitch;
        let hold = (c.grenade_hold_ms.0 + c.grenade_hold_ms.1) / 2;
        let end = c.grenade_launch_ms + c.grenade_look_after_ms;
        let mut held = Vec::new();
        for now in (0..end + 200).step_by(50) {
            match reflex.smoke_throwing(&me, now, &c, 0.1) {
                Some(o) => {
                    assert!(o.legs.is_some() && o.look_at.is_some(), "{now}: {o:?}");
                    held.push((now, o.keys == key));
                }
                None => assert!(now >= end, "{now}"),
            }
        }
        assert!(held.iter().all(|&(now, down)| down == (now < hold)), "{held:?}");
        assert!(reflex.take_smoke_notes().iter().any(|n| n.contains("throws a smoke for AntiSniper")));
        assert!(reflex.can_smoke(), "over");
        // Facing away: never pressed, given up.
        assert!(reflex.start_smoke(plan(SmokeUse::CrossLane), 0, None));
        assert_eq!(reflex.lane_smokes(), 1, "a lane's smoke counts for the life");
        let away = body(0, 0, Vec3::ZERO, std::f32::consts::PI);
        for now in (0..=c.grenade_give_up_ms).step_by(50) {
            let o = reflex.smoke_throwing(&away, now, &c, 0.1);
            assert!(o.is_none_or(|o| o.keys == 0), "{now}");
        }
        assert!(reflex.can_smoke());
        assert!(reflex.take_smoke_notes().iter().any(|n| n.contains("gave up a smoke for CrossLane")));
        // A battery picked up before the press: given up at once, the key never sent.
        assert!(reflex.start_smoke(plan(SmokeUse::CrossLane), 0, None));
        let mut carrier = body(0, 0, Vec3::ZERO, std::f32::consts::PI);
        carrier.carrying = Some(1);
        assert!(reflex.smoke_throwing(&carrier, 50, &c, 0.1).is_none());
        assert!(reflex.can_smoke());
        assert!(reflex.take_smoke_notes().iter().any(|n| n.contains("gave up a smoke for CrossLane: carrying")));
    }

    /// [A208 `smoke.wait_for_cloud`] After the launch the thrower walks on to its hold point (no override), stands
    /// there looking at the threat's eye, and walks on when the cloud hides the line; the threat as its target is no
    /// fight, another target is, and so are a hit and the cap. Off (no wait), it walks on at the launch.
    #[test]
    fn a_thrower_holds_short_of_its_crossing_until_its_cloud_hides_the_line() {
        use crate::v026g::bots::smoke::{Cloud, CloudWait, Clouds, MatchMs, Placed, SmokeAim, SmokeGrenade, SmokeLine, SmokePlan, SmokeUse, WaitsFor};
        use crate::v026g::bots::tuning::{CloudWaiters, SmokeTuning, WaitForCloud};
        let c = CombatTuning::default();
        let pitch = 0.3;
        let line = SmokeLine { eye: Vec3::new(40.0, 1.6, 20.0), target: Vec3::new(0.0, 1.3, 20.0), crossing: Vec3::new(0.0, 0.0, 20.0) };
        let plan = SmokePlan {
            at: Vec3::new(20.0, 0.0, 20.0),
            aim: SmokeAim { yaw: 0.0, pitch, rest: Vec3::new(20.0, 0.0, 20.0) },
            for_use: SmokeUse::CrossLane,
            covers_m: 4.0,
            line,
        };
        let on = SmokeTuning { wait_for_cloud: Some(WaitForCloud::default()), ..SmokeTuning::default() };
        let off = SmokeTuning { wait_for_cloud: None, ..SmokeTuning::default() };
        let grenade = SmokeGrenade::RDG_M;
        let hold = Vec3::new(0.0, 0.0, 16.0);
        assert_eq!(CloudWait::of(&off, &grenade, line, Some(7), WaitsFor::Own(plan.for_use), hold), None, "off");
        let wait = CloudWait::of(&on, &grenade, line, Some(7), WaitsFor::Own(plan.for_use), hold).expect("on");
        assert_eq!(CloudWait::of(&on, &grenade, line, Some(7), WaitsFor::Mates, hold), None, "mates only with crossers");
        assert_eq!(wait.cap_ms, 4_000 + 1_240 + 500, "the fuse, the client's forming and the margin");
        let launch = c.grenade_launch_ms + c.grenade_look_after_ms;
        let view = view_of(vec![]);
        let memory = Memory::default();
        let thrown = |reflex: &mut Reflex, me: &PlaceView| {
            assert!(reflex.start_smoke(plan, 0, Some(wait)));
            for now in (0..=launch).step_by(50) {
                reflex.smoke_throwing(me, now, &c, 0.1);
            }
            assert!(reflex.holds_for_cloud(), "held from the launch");
            assert!(!reflex.can_smoke(), "no second throw in a hold");
        };
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        me.pitch = pitch;
        let mut reflex = Reflex::default();
        reflex.set_smoke_key(Some(key::QUICK[3]));
        thrown(&mut reflex, &me);
        // Walking its route on: no override.
        assert!(reflex.cloud_holding(&me, &view, &memory, launch + 50, 0.1).is_none());
        assert!(reflex.holds_for_cloud());
        // At the hold point: standing, looking at the threat's eye.
        let mut there = body(0, 0, Vec3::new(0.0, 0.0, 15.2), 0.0);
        there.pitch = pitch;
        let o = reflex.cloud_holding(&there, &view, &memory, launch + 100, 0.1).expect("stands");
        assert_eq!(o.look_at, Some(line.eye));
        assert!(o.legs.is_some_and(|l| l.why == LegsWhy::WaitingForCloud && l.at == there.feet), "{o:?}");
        // The threat as its target is no fight.
        reflex.target = Some(7);
        assert!(reflex.cloud_holding(&there, &view, &memory, launch + 150, 0.1).is_some());
        // The cloud hides the line: it walks on.
        let cloud = Cloud {
            by: 0,
            team: 0,
            placed: Placed::Arc,
            centre: Vec3::new(20.0, 0.0, 20.0),
            thrown: MatchMs(0),
            pours: MatchMs(4_000),
            shape: grenade.cloud,
        };
        let mut clouded = view_of(vec![]);
        clouded.smoke = Clouds::new(vec![cloud]);
        assert!(reflex.cloud_holding(&there, &clouded, &memory, 4_000 + 300, 0.1).is_some(), "still forming");
        assert!(reflex.cloud_holding(&there, &clouded, &memory, 4_000 + 1_000, 0.1).is_some(), "the client's forming: 1.24 s");
        assert!(reflex.cloud_holding(&there, &clouded, &memory, 4_000 + 1_300, 0.1).is_none());
        assert!(!reflex.holds_for_cloud(), "over");
        assert!(reflex.take_smoke_notes().iter().any(|n| n.contains("Hidden")));
        // Another target ends it; so does a hit; so does the cap.
        for end in ["Fight", "Hit", "Cap"] {
            let mut reflex = Reflex::default();
            reflex.set_smoke_key(Some(key::QUICK[3]));
            thrown(&mut reflex, &me);
            let mut at = launch + 100;
            match end {
                "Fight" => reflex.target = Some(3),
                "Hit" => reflex.hit_at = Some(launch),
                _ => at = wait.cap_ms,
            }
            assert!(reflex.cloud_holding(&there, &view, &memory, at, 0.1).is_none(), "{end}");
            assert!(!reflex.holds_for_cloud(), "{end}");
            assert!(reflex.take_smoke_notes().iter().any(|n| n.contains(end)), "{end}");
        }
        // Off: over at the launch, no hold.
        let mut reflex = Reflex::default();
        reflex.set_smoke_key(Some(key::QUICK[3]));
        assert!(reflex.start_smoke(plan, 0, None));
        for now in (0..=launch).step_by(50) {
            reflex.smoke_throwing(&me, now, &c, 0.1);
        }
        assert!(!reflex.holds_for_cloud() && reflex.can_smoke());
        // A mate's cloud (crossers): a hold from now, capped from its begin; one ended at a crossing is not begun there
        // again for that cloud.
        let crossers = SmokeTuning { wait_for_cloud: Some(WaitForCloud { who: CloudWaiters::Crossers, ..WaitForCloud::default() }), ..SmokeTuning::default() };
        let mates = CloudWait::of(&crossers, &grenade, line, Some(7), WaitsFor::Mates, hold).expect("crossers");
        let mut reflex = Reflex::default();
        assert!(reflex.hold_for_cloud(mates, &me, 10_000));
        reflex.cloud_holding(&me, &view, &memory, 10_000 + mates.cap_ms - 50, 0.1);
        assert!(reflex.holds_for_cloud());
        reflex.cloud_holding(&me, &view, &memory, 10_000 + mates.cap_ms, 0.1);
        assert!(!reflex.holds_for_cloud(), "the cap from the hold's begin");
        assert!(!reflex.hold_for_cloud(mates, &me, 10_000 + mates.cap_ms + 50), "not twice at one crossing");
        assert!(reflex.hold_for_cloud(mates, &me, 10_000 + 2 * mates.cap_ms + 50), "a later cloud");
    }

    /// The view is held for the grenade from the key's press to the launch and
    /// the look time after it, and not before or after.
    #[test]
    fn the_view_is_held_for_the_grenade_from_the_press_to_the_launch_and_after() {
        let c = CombatTuning::default();
        let run = throw_run(20.0, &[], c.clone(), None, 6_000);
        let (press, _) = pressed_at(&run).expect("thrown");
        let end = press + c.grenade_launch_ms + c.grenade_look_after_ms;
        assert!(run.held.iter().any(|(_, held)| *held));
        // (A second throw of the life may press after the first has ended.)
        for (now, held) in run.held.iter().filter(|(now, _)| *now <= end) {
            // The held flag is read after the tick: the tick of the press sets it.
            let expect = (press..end).contains(now);
            assert_eq!(*held, expect, "{now} (press {press}, end {end})");
        }
    }

    /// A wall across the throws at x within 5 m of the thrower's line, 15 m
    /// out: a place straight ahead is behind it, one 30 degrees aside is not.
    fn narrow_wall() -> [[Vec3; 3]; 2] {
        let (a, b) = (Vec3::new(-5.0, 0.0, 15.0), Vec3::new(5.0, 0.0, 15.0));
        let (c, d) = (Vec3::new(5.0, 10.0, 15.0), Vec3::new(-5.0, 10.0, 15.0));
        [[a, b, c], [a, c, d]]
    }

    /// The nearest place is behind a wall, the next is clear: it is thrown at
    /// the next, whatever the first was refused for; with one place weighed
    /// (`grenade_places` 1) nothing is thrown, as before.
    #[test]
    fn the_next_place_is_thrown_at_when_the_nearest_is_behind_a_wall() {
        let walled = floor_and(&narrow_wall());
        let ahead = Vec3::new(0.0, 0.0, 25.0);
        let aside = Vec3::new(13.0, 0.0, 22.0);
        assert!(ahead.distance_xz(Vec3::ZERO) < aside.distance_xz(Vec3::ZERO));
        let one = CombatTuning {
            grenade_places: 1,
            ..combat_off()
        };
        let run = throw_run_at(&[ahead, aside], &[], one, Some(&walled), 6_000);
        assert!(run.thrown.is_empty(), "{:?}", run.notes);
        assert!(
            run.notes
                .iter()
                .any(|n| n.contains("(0.0, 0.0, 25.0)")
                    && n.ends_with("hits a wall near the thrower")),
            "{:?}",
            run.notes
        );
        let run = throw_run_at(
            &[ahead, aside],
            &[],
            combat_off(),
            Some(&walled),
            6_000,
        );
        assert!(!run.thrown.is_empty(), "{:?}", run.notes);
        assert!(
            run.notes
                .iter()
                .all(|n| !n.contains("holds a grenade back")),
            "{:?}",
            run.notes
        );
        let (_, press) = pressed_at(&run).unwrap();
        let landing = landing_of(
            press.look_at.unwrap(),
            (0.0, 0.0),
            &arc::Mesh(&walled.sight),
        );
        let miss = landing.contact.point.distance_xz(aside);
        assert!(miss < 1.0, "{:?}", landing.contact);
        // The same run is the same.
        let again = throw_run_at(
            &[ahead, aside],
            &[],
            combat_off(),
            Some(&walled),
            6_000,
        );
        assert_eq!(again.thrown, run.thrown);
        assert_eq!(again.notes, run.notes);
        assert_eq!(again.ticks, run.ticks);
    }

    /// The places weighed, on grounds made for it: the first that passes
    /// every check, at most `grenade_places` of them, nearest first.
    #[test]
    fn the_places_are_weighed_in_order_up_to_the_cap() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        // A wall across the whole line to the north: every place straight
        // ahead (x within 8 m) meets it 15 m out; the ones aside do not.
        let wall = |from: Vec3, dir: Vec3, max_t: f32| {
            if dir.z > 0.0 && from.z < 15.0 {
                let t = (15.0 - from.z) / dir.z;
                if t <= max_t && (from.x + dir.x * t).abs() < 8.0 {
                    return Some(arc::Hit {
                        t,
                        normal: Vec3::new(0.0, 0.0, -1.0),
                    });
                }
            }
            arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t)
        };
        let blocked = [
            Vec3::new(0.0, 0.0, 14.0 + 3.0),
            Vec3::new(1.0, 0.0, 20.0),
            Vec3::new(-2.0, 0.0, 25.0),
        ];
        let clear = Vec3::new(20.0, 0.0, 14.0);
        let c = combat_off();
        assert_eq!(c.grenade_places, 3);
        let pick = |places: &[Vec3], c: &CombatTuning| {
            pick_place(&me, &view, places, (0.0, 0.0), c, &wall)
        };
        // The second passes when the first is a wall.
        let mut places = vec![blocked[0], clear];
        assert_eq!(pick(&places, &c), Ok(clear));
        // Each place weighed is told with its verdict.
        let mut told = Vec::new();
        let _ = pick_place_noting(&me, &view, &places, (0.0, 0.0), &c, &wall, |i, v| {
            told.push((i, v));
        });
        let told: Vec<_> = told.into_iter().map(|(i, v)| (i, v.map(|_| ()))).collect();
        assert_eq!(told, vec![(0, Err(Refuse::Wall)), (1, Ok(()))]);
        // None passes: no throw, and the nearest is named with its reason.
        places = blocked.to_vec();
        assert_eq!(pick(&places, &c), Err((blocked[0], Refuse::Wall)));
        // The cap: the clear place is the fourth, the cap is three.
        places = vec![blocked[0], blocked[1], blocked[2], clear];
        assert_eq!(pick(&places, &c), Err((blocked[0], Refuse::Wall)));
        let four = CombatTuning {
            grenade_places: 4,
            ..c.clone()
        };
        assert_eq!(pick(&places, &four), Ok(clear));
        // The third is the last weighed.
        places = vec![blocked[0], blocked[1], clear, blocked[2]];
        assert_eq!(pick(&places, &c), Ok(clear));
        // 1 is the nearest only, and 0 is no less than 1.
        for cap in [0, 1] {
            let one = CombatTuning {
                grenade_places: cap,
                ..c.clone()
            };
            assert_eq!(pick(&[clear, blocked[0]], &one), Ok(clear));
            assert_eq!(
                pick(&[blocked[0], clear], &one),
                Err((blocked[0], Refuse::Wall))
            );
        }
        // A place that fails a check other than the wall's is not let through
        // either: out of reach.
        let far = Vec3::new(0.0, 0.0, 35.0);
        assert_eq!(pick(&[far], &c), Err((far, Refuse::Reach)));
        assert_eq!(pick(&[far, clear], &c), Ok(clear));
        // Determinism.
        assert_eq!(pick(&places, &c), pick(&places, &c));
    }

    /// The weighing, on grounds made for it.
    #[test]
    fn a_throw_is_weighed_on_the_ground_it_meets() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let c = earlier_habit();
        let at = Vec3::new(0.0, 0.0, 20.0);
        let weigh =
            |ground: &dyn arc::Ground| weigh_throw(&me, &view, at, (0.0, 0.0), &c, true, ground);
        assert_eq!(weigh(&arc::Floor { y: 0.0 }), Ok(()));
        // A ledge 2.5 m high from 8 m on: the grenade comes down on its
        // edge short of 12 m, in the thrower's own blast; not a wall.
        let ledge = |from: Vec3, dir: Vec3, max_t: f32| {
            let floor = arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t);
            let top = (dir.y < 0.0 && from.y >= 2.5)
                .then(|| (2.5 - from.y) / dir.y)
                .filter(|t| *t <= max_t && from.z + dir.z * t >= 8.0)
                .map(|t| arc::Hit { t, normal: Vec3::Y });
            match (floor, top) {
                (Some(a), Some(b)) => Some(if a.t < b.t { a } else { b }),
                (a, b) => a.or(b),
            }
        };
        let landing = arc::Flight::thrown(me.eye(), 0.0, throw_view(&me, at, &c).unwrap().1)
            .landing(&ledge)
            .unwrap();
        assert!(!landing.contact.is_wall());
        assert!(landing.contact.point.z < c.grenade_land_m, "{:?}", landing.contact);
        assert_eq!(weigh(&ledge), Err(Refuse::Thrower));
        // No ground at all: it comes down nowhere.
        let nothing = |_: Vec3, _: Vec3, _: f32| -> Option<arc::Hit> { None };
        assert_eq!(weigh(&nothing), Err(Refuse::Nowhere));
        // A place out of reach (35 m): no pitch.
        let far = Vec3::new(0.0, 0.0, 35.0);
        assert_eq!(
            weigh_throw(&me, &view, far, (0.0, 0.0), &c, true, &arc::Floor { y: 0.0 }),
            Err(Refuse::Reach)
        );
        // A place 2 m up (the arc comes down on the floor under it): thrown as it was; off the place with a height asked.
        let up = Vec3::new(0.0, 2.0, 20.0);
        let floor = arc::Floor { y: 0.0 };
        assert_eq!(weigh_throw(&me, &view, up, (0.0, 0.0), &c, true, &floor), Ok(()));
        let asked = CombatTuning { grenade_off_y_m: Some(1.5), ..c.clone() };
        assert_eq!(weigh_throw(&me, &view, up, (0.0, 0.0), &asked, true, &floor), Err(Refuse::Off));
        assert_eq!(weigh_throw(&me, &view, at, (0.0, 0.0), &asked, true, &floor), Ok(()));
        // A first contact on a wall 19 m out (clear of the thrower) for a
        // place at 27 m: off the place.
        let wall = |from: Vec3, dir: Vec3, max_t: f32| {
            if dir.z <= 0.0 || from.z > 19.0 {
                return arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t);
            }
            let t = (19.0 - from.z) / dir.z;
            (t <= max_t).then_some(arc::Hit {
                t,
                normal: Vec3::new(0.0, 0.0, -1.0),
            })
        };
        let place = Vec3::new(0.0, 0.0, 27.0);
        let off = weigh_throw(&me, &view, place, (0.0, 0.0), &c, true, &wall);
        assert_eq!(off, Err(Refuse::Off));
        // A wall inside the thrower's blast (8 m out, the place at 17 m) is
        // his blast, not "a wall": a wall 15 m out, with the place 2 m
        // beyond it, is the wall rule's.
        let wall_at = |z: f32| {
            move |from: Vec3, dir: Vec3, max_t: f32| {
                if dir.z <= 0.0 || from.z > z {
                    return arc::Ground::cast(&arc::Floor { y: 0.0 }, from, dir, max_t);
                }
                let t = (z - from.z) / dir.z;
                (t <= max_t).then_some(arc::Hit {
                    t,
                    normal: Vec3::new(0.0, 0.0, -1.0),
                })
            }
        };
        let near = Vec3::new(0.0, 0.0, 17.0);
        let inside = weigh_throw(&me, &view, near, (0.0, 0.0), &c, true, &wall_at(8.0));
        assert_eq!(inside, Err(Refuse::Thrower));
        let beyond = weigh_throw(&me, &view, near, (0.0, 0.0), &c, true, &wall_at(15.0));
        assert_eq!(beyond, Err(Refuse::Wall));
        // The aim's error moves the landing as the trace says: 0.7 rad of
        // yaw sends it 13 m aside.
        let big = weigh_throw(&me, &view, at, (0.7, 0.0), &c, true, &arc::Floor { y: 0.0 });
        assert_eq!(big, Err(Refuse::Off));
    }

    #[test]
    fn the_same_throw_is_the_same_every_run() {
        let run = || {
            throw_run(
                23.0,
                &[body(3, 0, Vec3::new(14.0, 0.0, 10.0), 0.0)],
                CombatTuning::default(),
                None,
                9_000,
            )
        };
        let first = run();
        let second = run();
        assert_eq!(first.ticks, second.ticks);
        assert_eq!(first.notes, second.notes);
        assert!(!first.thrown.is_empty());
    }

    #[test]
    fn a_partners_death_spot_is_watched_weapon_up_then_left() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let partner = body(3, 0, Vec3::new(8.0, 0.0, 8.0), 0.0);
        let view = view_of(vec![
            me.clone(),
            body(1, 1, Vec3::ZERO, 0.0),
            body(2, 0, Vec3::ZERO, 0.0),
            partner,
        ]);
        let mut team = TeamKnowledge::default();
        team.danger.push(DangerSpot {
            at: Vec3::new(8.0, 0.0, 8.0),
            time: 1_000,
            victim: 3,
        });
        let aim = AimTuning::default();
        let memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(12);
        let sit = Situation {
            team: Some(&team),
            partner: Some(3),
            ..Situation::of(&view)
        };
        reflex.think_with(&me, &sit, &memory, 1_200, &aim, 0.5, &mut rng);
        let o = reflex.tick(&me, &view, &memory, 1_250, true, &aim, 0.5, &mut rng);
        assert_eq!(
            o.look_at
                .map(|p| p.distance_xz(Vec3::new(8.0, 0.0, 8.0)) < 0.1),
            Some(true)
        );
        assert_ne!(o.keys & key::AIM, 0);
        assert!(!o.fire);
        // Another teammate's death is not a trade; after `trade_ms` it is
        // over.
        let o = reflex.tick(&me, &view, &memory, 4_100, true, &aim, 0.5, &mut rng);
        assert!(o.look_at.is_none() && o.keys == 0, "{o:?}");
        let sit = Situation {
            partner: Some(2),
            ..sit
        };
        let mut reflex = Reflex::default();
        reflex.think_with(&me, &sit, &memory, 1_200, &aim, 0.5, &mut rng);
        assert!(
            reflex
                .tick(&me, &view, &memory, 1_250, true, &aim, 0.5, &mut rng)
                .look_at
                .is_none()
        );
    }

    /// Ticks a duel of a bot of `role` with the enemy seen until
    /// `seen_until` and the view on him; returns per tick (time, fire,
    /// keys, legs' why).
    #[allow(clippy::too_many_arguments)]
    fn duel(
        role: &str,
        sit: &Situation,
        me: &mut PlaceView,
        enemy_at: Vec3,
        aim: &AimTuning,
        from: u32,
        until: u32,
        seen_until: u32,
    ) -> Vec<(u32, bool, u32, Option<LegsWhy>)> {
        duel_reflex(
            role,
            sit,
            me,
            enemy_at,
            aim,
            from,
            until,
            seen_until,
            |_, _, _| {},
        )
        .0
    }

    /// A duel's ticks: the time, the trigger, the keys and the legs' why.
    type Ticks = Vec<(u32, bool, u32, Option<LegsWhy>)>;

    /// [`duel`] with its reflex, and `after` run on it after every tick (its
    /// time and what it made).
    #[allow(clippy::too_many_arguments)]
    fn duel_reflex(
        role: &str,
        sit: &Situation,
        me: &mut PlaceView,
        enemy_at: Vec3,
        aim: &AimTuning,
        from: u32,
        until: u32,
        seen_until: u32,
        mut after: impl FnMut(&mut Reflex, u32, &Override),
    ) -> (Ticks, Reflex) {
        let mut memory = Memory::default();
        // The role's guns with the test's ammo: a magazine and a little
        // more (the kits carry a life's worth, too long a duel to test).
        let arms = Arms::of_test_role(role).map(|arms| {
            let spare = match arms.primary.0.item {
                "uzi" => 50,
                "mosin_nagant" => 10,
                _ => arms.primary.1,
            };
            // And the TT-33 as weapon 2, for the switch (the bots carry
            // none; a kit with one switches when weapon 1 is dry).
            &*Box::leak(Box::new(Arms {
                primary: (arms.primary.0, spare),
                pistol: crate::v026g::bots::guns::gun("tt_33").map(|gun| (gun, 50)),
                reserve: spare,
            }))
        });
        let mut reflex = Reflex::armed(arms);
        let mut rng = Rng::new(21);
        let mut out = Vec::new();
        let chest = enemy_at + Vec3::new(0.0, 1.3, 0.0);
        let mut now = from;
        while now < until {
            let seen = if now < seen_until {
                vec![percept(1, enemy_at, now, Source::Seen)]
            } else {
                vec![]
            };
            sense(&mut memory, &seen, now);
            if (now - from).is_multiple_of(250) {
                reflex.think_with(me, sit, &memory, now, aim, 0.5, &mut rng);
            }
            facing(me, chest);
            let o = reflex.tick(me, sit.view, &memory, now, true, aim, 0.5, &mut rng);
            out.push((now, o.fire, o.keys, o.legs.map(|l| l.why)));
            after(&mut reflex, now, &o);
            now += 50;
        }
        (out, reflex)
    }

    /// The trigger's runs in a duel: (first tick, ticks held), and the
    /// times of the presses of `keys`.
    fn runs(ticks: &[(u32, bool, u32, Option<LegsWhy>)]) -> Vec<(u32, u32)> {
        let mut runs: Vec<(u32, u32)> = Vec::new();
        let mut last = None;
        for &(now, fire, _, _) in ticks {
            if fire {
                match runs.last_mut() {
                    Some((_, n)) if last == Some(now - 50) => *n += 1,
                    _ => runs.push((now, 1)),
                }
                last = Some(now);
            }
        }
        runs
    }

    fn pressed(ticks: &[(u32, bool, u32, Option<LegsWhy>)], keys: u32) -> Vec<u32> {
        ticks
            .iter()
            .filter(|t| t.2 & keys != 0)
            .map(|t| t.0)
            .collect()
    }

    #[test]
    fn it_reloads_at_the_clip_counted_in_rounds_then_takes_the_pistol() {
        let enemy_at = Vec3::new(0.0, 0.0, 20.0);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        let [uzi, tt] = ["uzi", "tt_33"].map(|item| guns::gun(item).unwrap());
        // The flanker: the Uzi, 32 in it and 18 more; the TT-33, 8 and 42.
        // No map: no cover to find, so it reloads where it stands.
        let ticks = duel(
            "flanker",
            &Situation::of(&view),
            &mut me,
            enemy_at,
            &aim,
            1_000,
            40_000,
            40_000,
        );
        let reloads = pressed(&ticks, key::RELOAD);
        let switches = pressed(&ticks, key::WEAPON_2);
        assert_eq!(switches.len(), 1, "{switches:?}");
        let switch = switches[0];
        assert!(reloads.len() >= 2, "{reloads:?}");
        assert!(
            reloads[0] < switch && switch < reloads[1],
            "{reloads:?} {switch}"
        );
        // The Uzi's rounds, by its holds (a shot a 100 ms begun): the
        // magazine, then the 18 the one reload loads, then it is dry.
        let shots = |from: u32, to: u32| -> u32 {
            runs(&ticks)
                .iter()
                .filter(|(at, _)| (from..to).contains(at))
                .map(|(_, n)| uzi.shots_held(n * 50))
                .sum()
        };
        assert_eq!(shots(0, reloads[0]), 32);
        assert_eq!(shots(reloads[0], switch), 18);
        // No shot while reloading or switching.
        let quiet = |at: u32, ms: u32| {
            ticks
                .iter()
                .all(|&(now, fire, _, _)| !(fire && now >= at && now < at + ms))
        };
        assert!(quiet(reloads[0], 2_300) && quiet(switch, 800 + 500));
        // The pistol: one round a press, pressed at most half its 200 ms
        // shot, again after its cycle; reloaded after its 8.
        let presses: Vec<(u32, u32)> = runs(&ticks)
            .into_iter()
            .filter(|(at, _)| *at > switch)
            .collect();
        assert!(presses.iter().all(|(_, n)| *n * 50 <= 100), "{presses:?}");
        assert!(
            presses
                .windows(2)
                .all(|w| w[1].0 - w[0].0 >= tt.cycle_ms() + aim.combat.press_gap_ms)
        );
        let before = presses.iter().filter(|(at, _)| *at < reloads[1]).count();
        assert_eq!(before, 8, "{presses:?} {reloads:?}");
        assert!(quiet(reloads[1], 3_077), "from empty");
    }

    #[test]
    fn a_bolt_fires_once_a_press_and_reloads_round_by_round() {
        let enemy_at = Vec3::new(0.0, 0.0, 40.0);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        // The whole load of five: the early cut is another test's.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            combat: CombatTuning {
                reload_rounds_in_view: 0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mosin = guns::gun("mosin_nagant").unwrap();
        // The camper: the Mosin, 5 in it and 5 more.
        let ticks = duel(
            "camper",
            &Situation::of(&view),
            &mut me,
            enemy_at,
            &aim,
            1_000,
            40_000,
            40_000,
        );
        let reloads = pressed(&ticks, key::RELOAD);
        let switch = pressed(&ticks, key::WEAPON_2);
        assert_eq!(switch.len(), 1);
        let presses: Vec<(u32, u32)> = runs(&ticks)
            .into_iter()
            .filter(|(at, _)| *at < switch[0])
            .collect();
        // Each press shorter than the shot's 375 ms; the next after the
        // shot and the bolt.
        assert!(presses.iter().all(|(_, n)| *n * 50 <= 100), "{presses:?}");
        assert!(
            presses
                .windows(2)
                .all(|w| w[1].0 - w[0].0 >= mosin.cycle_ms() + aim.combat.press_gap_ms),
            "{presses:?}"
        );
        // Five presses, the reload of five (start, 5 x 500, finish), five
        // more, then dry: the pistol.
        let reloads: Vec<u32> = reloads.into_iter().filter(|at| *at < switch[0]).collect();
        assert_eq!(reloads.len(), 1, "{reloads:?}");
        let first = presses.iter().filter(|(at, _)| *at < reloads[0]).count();
        assert_eq!(first, 5);
        let reload_ms = 1_500 + 5 * 500 + 1_500;
        assert!(
            presses
                .iter()
                .all(|(at, _)| !(*at >= reloads[0] && *at < reloads[0] + reload_ms))
        );
        let second = presses
            .iter()
            .filter(|(at, _)| *at > reloads[0] && *at < switch[0])
            .count();
        assert_eq!(second, 5);
    }

    #[test]
    fn it_breaks_sight_before_reloading_and_reloads_tactically_when_quiet() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let enemy_at = Vec3::new(55.0, 0.0, 30.0);
        let mut me = body(0, 0, Vec3::new(32.0, 0.0, 28.0), 0.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        let sit = Situation {
            tactics: Some(&tactics),
            side: Some(0),
            ..Situation::of(&view)
        };
        // Empty with the target in sight: it heads for cover, and the
        // reload comes once it is breaking sight.
        let ticks = duel(
            "flanker", &sit, &mut me, enemy_at, &aim, 1_000, 12_000, 12_000,
        );
        let first = ticks
            .iter()
            .find(|t| t.2 & key::RELOAD != 0)
            .expect("reloads");
        assert_eq!(first.3, Some(LegsWhy::BreakSight), "{first:?}");
        // A little fire, then nothing seen: a tactical reload after
        // `reload_idle_ms`, once.
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy_at = Vec3::new(0.0, 0.0, 20.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let ticks = duel(
            "flanker",
            &Situation::of(&view),
            &mut me,
            enemy_at,
            &aim,
            1_000,
            12_000,
            4_500,
        );
        let uzi = guns::gun("uzi").unwrap();
        let fired: u32 = runs(&ticks)
            .iter()
            .map(|(_, n)| uzi.shots_held(n * 50))
            .sum();
        let some = (uzi.magazine as f32 * aim.combat.reload_some_share).ceil() as u32;
        assert!(fired >= some && fired < uzi.magazine, "{fired}");
        let reloads = pressed(&ticks, key::RELOAD);
        assert_eq!(reloads.len(), 1, "{reloads:?}");
        assert!(
            reloads[0] >= 4_500 + aim.combat.reload_idle_ms - 500,
            "{reloads:?}"
        );
    }

    /// The first reload key of `ticks` (a duel's), and the reflex of the same
    /// duel stopped `ms` after it, with the enemy seen until `seen_until`.
    /// The held-through-the-loop rule is off in tests that look at the whole
    /// load.
    fn first_reload(
        role: &str,
        ms: u32,
        held_at: Option<u32>,
        combat: CombatTuning,
        seen_until: u32,
    ) -> (Ticks, u32, Reflex) {
        let enemy_at = Vec3::new(0.0, 0.0, 20.0);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let aim = AimTuning {
            fire_within_deg: 180.0,
            combat,
            ..AimTuning::default()
        };
        let sit = Situation::of(&view);
        let ticks = duel(
            role, &sit, &mut me, enemy_at, &aim, 1_000, 30_000, seen_until,
        );
        let press = pressed(&ticks, key::RELOAD)[0];
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let (ticks, reflex) = duel_reflex(
            role,
            &sit,
            &mut me,
            enemy_at,
            &aim,
            1_000,
            press + ms,
            seen_until,
            |reflex, now, _| {
                // Something else holds the sights or the trigger at this tick.
                if Some(now) == held_at.map(|after| press + after) {
                    reflex.held_keys = true;
                }
            },
        );
        (ticks, press, reflex)
    }

    #[test]
    fn a_round_by_round_reload_is_loaded_with_the_sights_down_to_the_full_magazine() {
        // The SPAS-12: 8 in it, 32 more; 1.0 s, 8 rounds of 0.375 s, 1.0 s.
        let loop_ms = 1_000 + 8 * 375;
        let (ticks, press, mut reflex) = first_reload(
            "shotgunner",
            loop_ms + 100,
            None,
            CombatTuning {
                reload_rounds_in_view: 0,
                ..CombatTuning::default()
            },
            u32::MAX,
        );
        // It fought down the sights before the reload ...
        assert!(
            ticks.iter().any(|t| t.0 < press && t.2 & key::AIM != 0),
            "{press}"
        );
        // ... and has them down from the key to the last round, whatever the
        // branch (a seen target here) would ask for.
        assert!(
            ticks
                .iter()
                .filter(|t| t.0 >= press && t.0 < press + loop_ms)
                .all(|t| t.2 & key::AIM == 0),
            "{press}"
        );
        // The loop went to the full magazine, the ledger with it.
        let end = reflex.loop_end().expect("the loop ended");
        assert_eq!((end.at, end.loaded, end.target), (press, 8, 8));
        assert!(!end.cut());
        // Counted as a reload and a loop of a plain gun, not cut.
        let counts = reflex.mod_counts();
        assert_eq!((counts.reloads, counts.reloads_modded), (1, 0));
        assert_eq!((counts.loops, counts.loops_cut, counts.loops_modded), (1, 0, 0));
        let load = *reflex.hands().held();
        assert_eq!((load.rounds, load.spare), (8, 24));
        // The sights come back for the finish clip, and the bot waits the
        // whole load for its first shot.
        assert!(
            ticks
                .iter()
                .any(|t| t.0 >= press + loop_ms && t.2 & key::AIM != 0)
        );
        assert!(
            ticks
                .iter()
                .all(|t| !(t.1 && t.0 >= press && t.0 < press + loop_ms + 1_000))
        );
    }

    #[test]
    fn a_loop_cut_by_a_held_key_is_counted_as_the_client_ran_it() {
        // Something other than the reflex holds a key at the loop's first end
        // (0.375 s after the 1.0 s start): the client has one round and goes
        // on to its finish clip; the ledger has the one, the weapon is free
        // when that clip is over, not at the whole load's time.
        let (ticks, press, reflex) = first_reload(
            "shotgunner",
            4_000,
            Some(1_000 + 350),
            CombatTuning::default(),
            u32::MAX,
        );
        let end = reflex.loop_end().expect("the loop ended");
        assert_eq!((end.at, end.loaded, end.target), (press, 1, 8));
        assert!(end.cut());
        assert_eq!(reflex.mod_counts().loops_cut, 1);
        assert_eq!(end.ended, press + 1_000 + 375);
        // The weapon is free at the finish clip's end.
        let ready = press + 1_000 + 375 + 1_000 + guns::CLIENT_LAG_MS;
        assert_eq!(end.ready, ready);
        let fired: Vec<u32> = ticks
            .iter()
            .filter(|t| t.1 && t.0 > press)
            .map(|t| t.0)
            .collect();
        assert!(!fired.is_empty());
        assert!(
            fired[0] >= ready && fired[0] < ready + 1_000,
            "{fired:?} {ready}"
        );
        // One round in it, not eight: one shot, and the next reload press.
        let reloads = pressed(&ticks, key::RELOAD);
        assert!(reloads.len() >= 2, "{reloads:?}");
        let shots = runs(&ticks)
            .into_iter()
            .filter(|(at, _)| *at > press && *at < reloads[1])
            .count();
        assert_eq!(shots, 1, "{reloads:?} {fired:?}");
    }

    /// The design's early cut, off by default.
    fn cutting() -> CombatTuning {
        CombatTuning {
            reload_rounds_in_view: 3,
            ..CombatTuning::default()
        }
    }

    #[test]
    fn with_a_target_in_view_the_sights_end_the_loop_at_the_third_round() {
        // Seen all the while: the sights are pressed in the middle of the
        // span before the third round-end (1.0 s + 3 x 0.375 s), the client
        // goes on to its finish clip with three rounds, and the weapon is
        // free at the clip's end, not at the whole load's 5.0 s.
        let (ticks, press, mut reflex) =
            first_reload("shotgunner", 4_500, None, cutting(), u32::MAX);
        let end = reflex.loop_end().expect("the loop ended");
        assert_eq!((end.at, end.loaded, end.target), (press, 3, 8));
        assert!(end.cut());
        assert_eq!(end.ended, press + 1_000 + 3 * 375);
        let ready = end.ended + 1_000 + guns::CLIENT_LAG_MS;
        assert_eq!(end.ready, ready);
        // Down until the middle of the span, up from it.
        let from = press + 1_000 + 2 * 375 + 187;
        assert!(
            ticks
                .iter()
                .filter(|t| t.0 >= press && t.0 < from)
                .all(|t| t.2 & key::AIM == 0)
        );
        assert!(
            ticks
                .iter()
                .filter(|t| t.0 >= from && t.0 <= end.ended)
                .all(|t| t.2 & key::AIM != 0)
        );
        // The first shot comes when the weapon is free, not before.
        let fired: Vec<u32> = ticks
            .iter()
            .filter(|t| t.1 && t.0 > press)
            .map(|t| t.0)
            .collect();
        assert!(
            fired[0] >= ready && fired[0] < ready + 1_000,
            "{fired:?} {ready}"
        );
        // Three rounds in it: the shots it fires are of them.
        let shots = runs(&ticks).iter().filter(|(at, _)| *at > press).count() as u32;
        assert_eq!(reflex.hands().held().rounds + shots, 3);
    }

    #[test]
    fn with_nothing_in_view_or_the_rule_off_the_whole_load_goes_in() {
        let loop_of = |combat: CombatTuning, seen_until: u32| {
            let (_, press, reflex) = first_reload("shotgunner", 6_500, None, combat, seen_until);
            let end = reflex.loop_end().expect("the loop ended");
            assert_eq!(end.at, press);
            end
        };
        // A tactical reload with nothing seen for a while: more rounds to
        // load than the three, all of them.
        let end = loop_of(cutting(), 4_500);
        assert!(end.target > 3 && !end.cut(), "{end:?}");
        // In view, with the rule at 0 (the default: the cut failed the
        // smoke) the whole load; at 8 or more, which is the last round-end
        // anyway, likewise.
        assert_eq!(CombatTuning::default().reload_rounds_in_view, 0);
        for rule in [0, 8, 20] {
            let combat = CombatTuning {
                reload_rounds_in_view: rule,
                ..CombatTuning::default()
            };
            let end = loop_of(combat, u32::MAX);
            assert_eq!((end.loaded, end.target), (8, 8), "{rule}");
        }
    }

    #[test]
    fn with_the_aim_control_off_the_sights_cut_the_loop_to_one_round_as_before() {
        let combat = CombatTuning {
            reload_aim_control: false,
            ..CombatTuning::default()
        };
        let (ticks, press, reflex) = first_reload("shotgunner", 4_000, None, combat, u32::MAX);
        let end = reflex.loop_end().expect("the loop ended");
        assert_eq!((end.loaded, end.target), (1, 8));
        assert!(
            ticks
                .iter()
                .any(|t| t.0 >= press && t.0 < press + 1_375 && t.2 & key::AIM != 0)
        );
    }

    #[test]
    fn a_magazine_reload_keeps_the_sights_as_the_branches_ask() {
        // The AK-74U's 30: no key of a bot shortens its reload, so nothing is
        // changed in what it presses, and it has no loop.
        let (ticks, press, mut reflex) =
            first_reload("support", 3_000, None, CombatTuning::default(), u32::MAX);
        assert!(
            ticks
                .iter()
                .any(|t| t.0 >= press && t.0 < press + 3_000 && t.2 & key::AIM != 0),
            "{press}"
        );
        assert!(reflex.loop_end().is_none());
        // Counted in at once, as before.
        assert_eq!(reflex.hands().held().rounds, 30);
    }

    #[test]
    fn the_matchup_keeps_each_weapon_at_its_range() {
        let c = CombatTuning::default();
        assert_eq!(preferred_band(None, Some(WeaponKind::Rifle), &c), None);
        assert_eq!(
            preferred_band(Some(WeaponKind::Shotgun), None, &c),
            Some(c.band_shotgun)
        );
        assert_eq!(
            preferred_band(Some(WeaponKind::Rifle), Some(WeaponKind::Shotgun), &c),
            Some((c.keep_from_shotgun_m, c.band_rifle.1))
        );
        assert_eq!(
            preferred_band(Some(WeaponKind::Rifle), Some(WeaponKind::Sniper), &c),
            Some((c.band_rifle.0, c.close_on_sniper_m))
        );
        // A shotgun does not keep off another shotgun.
        assert_eq!(
            preferred_band(Some(WeaponKind::Shotgun), Some(WeaponKind::Shotgun), &c),
            Some(c.band_shotgun)
        );
        // Its own weapon wins when the enemy's leaves no band.
        let wide = CombatTuning {
            keep_from_shotgun_m: c.band_smg.1 - 2.0,
            ..CombatTuning::default()
        };
        assert_eq!(
            preferred_band(Some(WeaponKind::Smg), Some(WeaponKind::Shotgun), &wide),
            Some(wide.band_smg)
        );
        assert_eq!(WeaponKind::of_role("rusher"), Some(WeaponKind::Shotgun));
        assert_eq!(WeaponKind::of_role("camper"), Some(WeaponKind::Sniper));
        assert_eq!(
            WeaponKind::of_item("wpn_aslt", 13),
            Some(WeaponKind::Sniper)
        );
        assert_eq!(
            WeaponKind::of_item("wpn_dbrl", 14),
            Some(WeaponKind::Shotgun)
        );
        assert_eq!(WeaponKind::of_item("wpn_smg", 9), Some(WeaponKind::Smg));
        assert_eq!(WeaponKind::of_item("wpn_aslt", 8), Some(WeaponKind::Rifle));

        // A shotgun bot facing a rifle at 30 m steps in; a sniper bot at
        // 10 m steps back.
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let aim = AimTuning::default();
        for (kind, enemy_x, closer) in [
            (WeaponKind::Shotgun, 30.0, true),
            (WeaponKind::Sniper, 10.0, false),
        ] {
            let me = body(0, 0, Vec3::new(20.0, 0.0, 32.0), 0.0);
            let enemy_at = Vec3::new(20.0 + enemy_x, 0.0, 32.0);
            let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
            let weapons = [Some(kind), Some(WeaponKind::Rifle)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                weapon: Some(kind),
                weapons: &weapons,
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(13);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
            assert_eq!(o.band_m, Some(c.band(kind)));
            let legs = o.legs.expect("steps");
            assert_eq!(legs.why, LegsWhy::Range);
            let d = legs.at.distance_xz(enemy_at);
            assert_eq!(d < enemy_x, closer, "{kind:?}: {d}");
        }
    }

    fn shot(from: Place, at: Vec3, time: u32) -> Sound {
        Sound {
            from,
            kind: SoundKind::Shot,
            at,
            time,
            reach_m: None,
        }
    }

    /// Match 0x5402a730: a bot kept a bot 67 m away as its target with the
    /// human 6 m from it. A seen enemy much nearer takes the target; one
    /// only somewhat nearer does not.
    #[test]
    fn a_much_nearer_seen_enemy_takes_the_target() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let (far, middle, near) = (
            Vec3::new(0.0, 0.0, 60.0),
            Vec3::new(5.0, 0.0, 40.0),
            Vec3::new(-3.0, 0.0, 8.0),
        );
        let view = view_of(vec![
            me.clone(),
            body(1, 1, far, 0.0),
            body(2, 1, middle, 0.0),
            body(3, 1, near, 0.0),
        ]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(30);
        let mut think = |percepts: &[Percept], now: u32, reflex: &mut Reflex| {
            sense(&mut memory, percepts, now);
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        };
        for now in [1_000, 1_300] {
            think(&[percept(1, far, now, Source::Seen)], now, &mut reflex);
        }
        assert_eq!(reflex.target(), Some(1));
        // One at 40 m: not under half of 60, the target is kept.
        for now in [1_600, 1_900, 2_200] {
            let seen = [
                percept(1, far, now, Source::Seen),
                percept(2, middle, now, Source::Seen),
            ];
            think(&seen, now, &mut reflex);
        }
        assert_eq!(reflex.target(), Some(1));
        // One at 8.5 m: him, once noticed.
        for now in [2_500, 2_800, 3_100] {
            let seen = [
                percept(1, far, now, Source::Seen),
                percept(2, middle, now, Source::Seen),
                percept(3, near, now, Source::Seen),
            ];
            think(&seen, now, &mut reflex);
        }
        assert_eq!(reflex.target(), Some(3));
        // Under 15 m it needs 5 m less: 10 m against 14 m keeps it, 12 m
        // against 20 m does not.
        for (kept_at, other_at, switches) in [(14.0, 10.0, false), (20.0, 12.0, true)] {
            let kept = Vec3::new(0.0, 0.0, kept_at);
            let other = Vec3::new(-2.0, 0.0, other_at);
            let view = view_of(vec![
                me.clone(),
                body(1, 1, kept, 0.0),
                body(2, 1, other, 0.0),
            ]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(31);
            for now in [1_000, 1_300, 1_600, 1_900, 2_200] {
                let mut seen = vec![percept(1, kept, now, Source::Seen)];
                if now >= 1_600 {
                    seen.push(percept(2, other, now, Source::Seen));
                }
                sense(&mut memory, &seen, now);
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            let want = if switches { 2 } else { 1 };
            assert_eq!(reflex.target(), Some(want), "{kept_at} {other_at}");
        }
    }

    /// Match 0x5402a730: a bot fought two bots 34-36 m away with the human
    /// 19 m to its side firing at it and never turned. Hit, it turns for a
    /// moment to the nearest enemy it heard firing, the target kept; seen
    /// there, he is the target.
    #[test]
    fn hit_it_turns_to_the_nearest_shooter_it_heard_even_with_a_target() {
        let target_at = Vec3::new(0.0, 0.0, 35.0);
        // To its side: out of the view of a bot facing the target.
        let human_at = Vec3::new(-19.0, 0.0, -2.0);
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        let mut view = view_of(vec![
            me.clone(),
            body(1, 1, target_at, 0.0),
            body(2, 1, human_at, 0.0),
        ]);
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(32);
        facing(&mut me, target_at + Vec3::new(0.0, 1.3, 0.0));
        for now in [1_000, 1_300] {
            sense(
                &mut memory,
                &[percept(1, target_at, now, Source::Seen)],
                now,
            );
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        assert_eq!(reflex.target(), Some(1));
        // Both fire; the human's shot is heard (with its error), and it is
        // hit.
        view.sounds = vec![shot(2, human_at, 1_500), shot(1, target_at, 1_550)];
        let heard = Percept {
            error_m: 2.0,
            ..percept(2, human_at + Vec3::new(1.0, 0.0, 0.5), 1_500, Source::Heard)
        };
        sense(
            &mut memory,
            &[percept(1, target_at, 1_600, Source::Seen), heard],
            1_600,
        );
        me.health = 0.8;
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_600,
            &aim,
            0.5,
            &mut rng,
        );
        let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
        let look = o.look_at.expect("looks");
        assert!(look.distance_xz(human_at) < 2.0, "{look:?}");
        assert!(!o.fire);
        assert_eq!(reflex.target(), Some(1), "the target kept meanwhile");
        // Turned, it sees him: him, though 19 m is not under half of 35.
        facing(&mut me, human_at + Vec3::new(0.0, 1.3, 0.0));
        for now in [1_900, 2_250] {
            sense(&mut memory, &[percept(2, human_at, now, Source::Seen)], now);
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        assert_eq!(reflex.target(), Some(2));
        let o = reflex.tick(&me, &view, &memory, 2_300, true, &aim, 0.5, &mut rng);
        assert!(o.look_at.is_some_and(|p| p.distance_xz(human_at) < 0.5));

        // Hit with the target the nearest shooter: it stays on him.
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        facing(&mut me, target_at + Vec3::new(0.0, 1.3, 0.0));
        let far_at = Vec3::new(-30.0, 0.0, -30.0);
        let mut view = view_of(vec![
            me.clone(),
            body(1, 1, target_at, 0.0),
            body(2, 1, far_at, 0.0),
        ]);
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        for now in [1_000, 1_300] {
            sense(
                &mut memory,
                &[percept(1, target_at, now, Source::Seen)],
                now,
            );
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        view.sounds = vec![shot(2, far_at, 1_500), shot(1, target_at, 1_550)];
        sense(
            &mut memory,
            &[
                percept(1, target_at, 1_600, Source::Seen),
                percept(2, far_at, 1_500, Source::Heard),
            ],
            1_600,
        );
        me.health = 0.8;
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_600,
            &aim,
            0.5,
            &mut rng,
        );
        let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
        assert!(
            o.look_at.is_some_and(|p| p.distance_xz(target_at) < 1.0),
            "{o:?}"
        );
    }

    /// Not hit, a shot heard within 25 m from outside the view, nearer than
    /// the target, turns it for a moment; a farther one, one in the view,
    /// or one farther than the target does not.
    #[test]
    fn a_near_shot_heard_from_outside_the_view_turns_it_for_a_moment() {
        let target_at = Vec3::new(0.0, 0.0, 30.0);
        let aim = AimTuning::default();
        for (shooter_at, turns) in [
            (Vec3::new(2.0, 0.0, -12.0), true),
            (Vec3::new(2.0, 0.0, -28.0), false),
            (Vec3::new(4.0, 0.0, 15.0), false),
        ] {
            let mut me = body(0, 0, Vec3::ZERO, 0.0);
            facing(&mut me, target_at + Vec3::new(0.0, 1.3, 0.0));
            let mut view = view_of(vec![
                me.clone(),
                body(1, 1, target_at, 0.0),
                body(2, 1, shooter_at, 0.0),
            ]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(33);
            for now in [1_000, 1_300] {
                sense(
                    &mut memory,
                    &[percept(1, target_at, now, Source::Seen)],
                    now,
                );
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            view.sounds = vec![shot(2, shooter_at, 1_500)];
            sense(
                &mut memory,
                &[
                    percept(1, target_at, 1_600, Source::Seen),
                    percept(2, shooter_at, 1_500, Source::Heard),
                ],
                1_600,
            );
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                1_600,
                &aim,
                0.5,
                &mut rng,
            );
            let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
            let looks = o.look_at.is_some_and(|p| p.distance_xz(shooter_at) < 0.5);
            assert_eq!(looks, turns, "{shooter_at:?}");
            if turns {
                // A moment: back on the target after `threat_ms`.
                for now in [1_900, 2_200, 2_500, 2_700] {
                    sense(
                        &mut memory,
                        &[percept(1, target_at, now, Source::Seen)],
                        now,
                    );
                    reflex.think_with(
                        &me,
                        &Situation::of(&view),
                        &memory,
                        now,
                        &aim,
                        0.5,
                        &mut rng,
                    );
                }
                let o = reflex.tick(&me, &view, &memory, 2_750, true, &aim, 0.5, &mut rng);
                assert!(
                    o.look_at.is_some_and(|p| p.distance_xz(target_at) < 1.0),
                    "{o:?}"
                );
            }
        }
    }

    /// With no target, a shot heard lately is looked at before the
    /// partner's death spot (the killer is there, not at the body); with
    /// none, the spot is watched if it lies within `trade_m`.
    #[test]
    fn a_shot_heard_is_looked_at_before_the_partners_death_spot() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let near_spot = Vec3::new(8.0, 0.0, 8.0);
        let far_spot = Vec3::new(45.0, 0.0, 8.0);
        let shooter_at = Vec3::new(-10.0, 0.0, -6.0);
        for (spot, heard, want) in [
            (near_spot, true, Some(shooter_at)),
            (near_spot, false, Some(near_spot)),
            (far_spot, false, None),
        ] {
            let mut view = view_of(vec![
                me.clone(),
                body(1, 1, shooter_at, 0.0),
                body(3, 0, spot, 0.0),
            ]);
            let mut memory = Memory::default();
            if heard {
                view.sounds = vec![shot(1, shooter_at, 1_100)];
                sense(
                    &mut memory,
                    &[percept(1, shooter_at, 1_100, Source::Heard)],
                    1_200,
                );
            }
            let mut team = TeamKnowledge::default();
            team.danger.push(DangerSpot {
                at: spot,
                time: 1_000,
                victim: 3,
            });
            let aim = AimTuning::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(34);
            let sit = Situation {
                team: Some(&team),
                partner: Some(3),
                ..Situation::of(&view)
            };
            reflex.think_with(&me, &sit, &memory, 1_200, &aim, 0.5, &mut rng);
            let o = reflex.tick(&me, &view, &memory, 1_250, true, &aim, 0.5, &mut rng);
            match want {
                Some(want) => {
                    let look = o.look_at.expect("looks");
                    assert!(look.distance_xz(want) < 0.1, "{spot:?} {heard}: {look:?}");
                }
                None => assert!(o.look_at.is_none(), "{o:?}"),
            }
        }
    }

    /// Play 6 turns to a heard shot (not a step) for `sound_ms`, and not
    /// again before `sound_again_ms` more, however long he keeps firing.
    #[test]
    fn play_6_looks_at_a_shot_a_moment_at_a_time() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let at = Vec3::new(-12.0, 0.0, 30.0);
        let aim = AimTuning::default();
        let c = &aim.combat;
        // A step heard: no turn.
        let mut view = view_of(vec![me.clone(), body(1, 1, at, 0.0)]);
        view.sounds = vec![Sound {
            from: 1,
            kind: SoundKind::Sprint,
            at,
            time: 1_000,
            reach_m: None,
        }];
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(36);
        sense(&mut memory, &[percept(1, at, 1_000, Source::Heard)], 1_000);
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_000,
            &aim,
            0.5,
            &mut rng,
        );
        let o = reflex.tick(&me, &view, &memory, 1_050, true, &aim, 0.5, &mut rng);
        assert!(o.look_at.is_none(), "{o:?}");
        // Shots every think: looked at, then not, then again.
        let mut looked = Vec::new();
        let mut now = 2_000;
        while now < 6_000 {
            view.sounds = vec![shot(1, at, now)];
            sense(&mut memory, &[percept(1, at, now, Source::Heard)], now);
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
            let o = reflex.tick(&me, &view, &memory, now + 50, true, &aim, 0.5, &mut rng);
            looked.push((now, o.look_at.is_some_and(|p| p.distance_xz(at) < 0.1)));
            now += 250;
        }
        let first = looked.iter().find(|l| l.1).map(|l| l.0).expect("looks");
        assert_eq!(first, 2_000);
        let period = c.sound_ms + c.sound_again_ms;
        for &(t, on) in &looked {
            assert_eq!(on, (t - first) % period < c.sound_ms, "{t}: {looked:?}");
        }
    }

    /// Match 0x5402a730: bots with their kit's ammo spent kept pressing
    /// FIRE for nine lives. Dry, it never fires and takes no step at the
    /// enemy (play 11); with rounds, the same duel fires and steps.
    #[test]
    fn a_dry_bot_never_fires_and_steps_at_no_enemy() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let fallback = Arms::fallback();
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        for rounds in [0, 30] {
            let arms: &'static Arms = Box::leak(Box::new(Arms {
                primary: (fallback.primary.0, rounds),
                pistol: None,
                reserve: rounds,
            }));
            let mut me = body(0, 0, Vec3::new(20.0, 0.0, 32.0), 0.0);
            let enemy_at = Vec3::new(50.0, 0.0, 32.0);
            let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
            let weapons = [Some(WeaponKind::Shotgun), Some(WeaponKind::Rifle)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                weapon: Some(WeaponKind::Shotgun),
                weapons: &weapons,
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::armed_carrying(Some(arms), rounds);
            assert_eq!(reflex.dry(), rounds == 0);
            let mut rng = Rng::new(35);
            let (mut fired, mut stepped) = (false, false);
            let mut now = 1_000;
            while now < 5_000 {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                if now % 250 == 0 {
                    reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                }
                facing(&mut me, enemy_at + Vec3::new(0.0, 1.3, 0.0));
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                fired |= o.fire;
                stepped |= o.legs.is_some_and(|l| l.why == LegsWhy::Range);
                now += 50;
            }
            assert_eq!(reflex.target(), Some(1));
            assert_eq!((fired, stepped), (rounds > 0, rounds > 0), "{rounds}");
        }
    }

    /// Match 0x15aa5b99 at 100-107 s: a bot aimed at a bot 80 m away while
    /// the owner sprinted up 4-11 m behind it, heard, and it never turned.
    /// A step heard near from outside the view turns it for a moment (a
    /// glance: it gives way to a use or a carry), even from a far target;
    /// one farther than `glance_heard_m`, one in the view, or one not much
    /// nearer than the target does not.
    #[test]
    fn a_step_heard_near_behind_turns_it_even_from_a_far_target() {
        let aim = AimTuning::default();
        for (target_z, heard_at, turns) in [
            (80.0, Vec3::new(1.0, 0.0, -6.0), true),
            (80.0, Vec3::new(1.0, 0.0, -20.0), false),
            (80.0, Vec3::new(3.0, 0.0, 8.0), false),
            (20.0, Vec3::new(1.0, 0.0, -12.0), false),
        ] {
            let target_at = Vec3::new(0.0, 0.0, target_z);
            let mut me = body(0, 0, Vec3::ZERO, 0.0);
            facing(&mut me, target_at + Vec3::new(0.0, 1.3, 0.0));
            let view = view_of(vec![
                me.clone(),
                body(1, 1, target_at, 0.0),
                body(2, 1, heard_at, 0.0),
            ]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(41);
            for now in [1_000, 1_300] {
                sense(
                    &mut memory,
                    &[percept(1, target_at, now, Source::Seen)],
                    now,
                );
                reflex.think_with(
                    &me,
                    &Situation::of(&view),
                    &memory,
                    now,
                    &aim,
                    0.5,
                    &mut rng,
                );
            }
            assert_eq!(reflex.target(), Some(1));
            // A sprint heard (no shot).
            sense(
                &mut memory,
                &[
                    percept(1, target_at, 1_600, Source::Seen),
                    percept(2, heard_at, 1_550, Source::Heard),
                ],
                1_600,
            );
            reflex.think_with(
                &me,
                &Situation::of(&view),
                &memory,
                1_600,
                &aim,
                0.5,
                &mut rng,
            );
            let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
            let looks = o.look_at.is_some_and(|p| p.distance_xz(heard_at) < 0.5);
            assert_eq!(looks, turns, "{target_z} {heard_at:?}: {o:?}");
            if turns {
                assert!(o.glance, "a glance gives way to a use or a carry");
                assert!(!o.fire);
                // A moment: back on the target after `glance_ms`.
                for now in [1_900, 2_200, 2_500, 2_700] {
                    sense(
                        &mut memory,
                        &[percept(1, target_at, now, Source::Seen)],
                        now,
                    );
                    reflex.think_with(
                        &me,
                        &Situation::of(&view),
                        &memory,
                        now,
                        &aim,
                        0.5,
                        &mut rng,
                    );
                }
                let o = reflex.tick(&me, &view, &memory, 2_750, true, &aim, 0.5, &mut rng);
                assert!(
                    o.look_at.is_some_and(|p| p.distance_xz(target_at) < 1.0),
                    "{o:?}"
                );
            }
        }
    }

    /// Match 0x15aa5b99 at 270-277 s: a bot walked past the owner at 13 m,
    /// told of him by a teammate, and never looked. With no target, a
    /// callout near from outside the view is glanced at for `glance_ms`,
    /// again only after `glance_told_every_ms`; a far one or one in the
    /// view is not.
    #[test]
    fn with_no_target_a_callout_near_is_glanced_at_now_and_then() {
        let aim = AimTuning {
            combat: CombatTuning {
                glance_told_m: 30.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let c = &aim.combat;
        for (told_at, glances) in [
            (Vec3::new(-13.0, 0.0, -2.0), true),
            (Vec3::new(-36.0, 0.0, -2.0), false),
            (Vec3::new(1.0, 0.0, 13.0), false),
        ] {
            // Facing +z (yaw 0).
            let me = body(0, 0, Vec3::ZERO, 0.0);
            let view = view_of(vec![me.clone(), body(1, 1, told_at, 0.0)]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(42);
            let mut looks = Vec::new();
            let mut now = 1_000;
            while now <= 7_000 {
                // Told again and again (the teammate keeps seeing him).
                if now % 500 == 0 {
                    let told = percept(1, told_at, now - 200, Source::Told { by: 5 });
                    sense(&mut memory, &[told], now);
                }
                if now % 250 == 0 {
                    reflex.think_with(
                        &me,
                        &Situation::of(&view),
                        &memory,
                        now,
                        &aim,
                        0.5,
                        &mut rng,
                    );
                }
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                assert!(!o.fire);
                if o.look_at.is_some_and(|p| p.distance_xz(told_at) < 0.5) {
                    assert!(o.glance);
                    looks.push(now);
                }
                now += 50;
            }
            assert_eq!(!looks.is_empty(), glances, "{told_at:?}");
            if glances {
                // Glances of `glance_ms`, starting `glance_told_every_ms`
                // apart.
                let starts: Vec<u32> = looks
                    .iter()
                    .enumerate()
                    .filter(|(i, t)| *i == 0 || looks[i - 1] + 50 != **t)
                    .map(|(_, t)| *t)
                    .collect();
                assert!(starts.len() >= 2, "{starts:?}");
                for pair in starts.windows(2) {
                    assert!(pair[1] - pair[0] >= c.glance_told_every_ms, "{starts:?}");
                }
                let longest = looks.len() as u32 * 50 / starts.len() as u32;
                assert!(longest <= c.glance_ms + 50, "{looks:?}");
            }
        }
    }

    /// The far-shooter case behind cover the shooter's bullet goes through (a crate, a car): with `hit_cover_reach` that is not
    /// cover, so no spot behind it is taken; with the switch off any surface of the mesh is cover, as it was.
    #[test]
    fn a_wall_the_shooters_bullet_goes_through_is_no_cover_from_a_hit() {
        use mapdata::{Armor, FaceArmor, RayTree, World};
        let field = crate::v026g::bots::tactics::tests::field();
        // The field's wall across x = 40 from z = 0 to 25, as thin cover (armor 0.05: nearly all the damage passes).
        let (points, triangles) = mapdata::library::PrimitiveShape::Box { half: Vec3::new(0.3, 2.0, 12.5) }.tessellate();
        let tree = RayTree::from_armored_corners(triangles.iter().map(|t| {
            (t.map(|i| points[i as usize] + Vec3::new(40.0, 2.0, 12.5)), FaceArmor::Material(Armor::new(0.05).unwrap()))
        }));
        let tactics = Tactics::new(Arc::new(World::assemble(field.map.clone(), field.grid.clone(), tree.into())), 1.0);
        let rule = super::super::tuning::CoverRule {
            pierce: mapdata::Pierce::new(0.3125).unwrap(),
            max_share: 0.5,
            spot_cost: None,
        };
        let start = Vec3::new(38.0, 0.0, 30.0);
        let shooter_at = Vec3::new(158.0, 0.0, 30.0);
        let hides = |reach: bool| {
            let aim = AimTuning {
                combat: CombatTuning { hit_bearing: true, hit_cover_reach: reach, ..CombatTuning::default() },
                ..AimTuning::default()
            };
            (0..6)
                .filter(|&seed| {
                    let mut me = body(0, 0, start, std::f32::consts::FRAC_PI_2);
                    let mut view = view_of(vec![me.clone(), body(1, 1, shooter_at, bearing(shooter_at, start))]);
                    fn sit<'a>(view: &'a MatchView, tactics: &'a Tactics, rule: super::super::tuning::CoverRule) -> Situation<'a> {
                        Situation { tactics: Some(tactics), side: Some(0), cover: Some(rule), ..Situation::of(view) }
                    }
                    let memory = Memory::default();
                    let mut reflex = Reflex::default();
                    let mut rng = Rng::new(50 + seed);
                    reflex.think_with(&me, &sit(&view, &tactics, rule), &memory, 1_000, &aim, 0.5, &mut rng);
                    view.sounds = vec![shot(1, shooter_at, 1_300)];
                    me.health = 0.7;
                    reflex.think_with(&me, &sit(&view, &tactics, rule), &memory, 1_600, &aim, 0.5, &mut rng);
                    reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng).legs.is_some()
                })
                .count()
        };
        assert_eq!(hides(true), 0, "a wall it shoots through is no cover");
        assert!(hides(false) >= 5, "the mesh alone: any surface is cover");
    }

    /// The far-shooter case (match report section 10): hit from 120 m by
    /// a shooter it neither sees nor hears. The HUD's marker gives the
    /// direction alone: it runs to a spot with cover that way (the wall),
    /// looks that way a moment (at a fixed point, not at him), calls the
    /// direction out to its team, waits there and does not peek back.
    #[test]
    fn hit_by_an_unseen_far_shooter_it_takes_cover_from_the_markers_direction() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        // The field: a wall across x = 40 from z = 0 to 25. The bot west of
        // it past its end; the shooter 120 m east, out of hearing and sight.
        let start = Vec3::new(38.0, 0.0, 30.0);
        let shooter_at = Vec3::new(158.0, 0.0, 30.0);
        let aim = AimTuning {
            combat: CombatTuning {
                hit_bearing: true,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let c = &aim.combat;
        let truth = bearing(start, shooter_at);
        let mut hid = 0;
        for seed in 0..6 {
            let mut me = body(0, 0, start, std::f32::consts::FRAC_PI_2);
            let mut view = view_of(vec![
                me.clone(),
                body(1, 1, shooter_at, bearing(shooter_at, start)),
            ]);
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                ..Situation::of(&view)
            };
            let memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(50 + seed);
            reflex.think_with(&me, &sit, &memory, 1_000, &aim, 0.5, &mut rng);
            view.sounds = vec![shot(1, shooter_at, 1_300)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                ..Situation::of(&view)
            };
            me.health = 0.7;
            reflex.think_with(&me, &sit, &memory, 1_600, &aim, 0.5, &mut rng);
            assert_eq!(reflex.target(), None);
            // The callout: where it stood, the direction within the error.
            let (at, toward) = reflex.take_hit_note().expect("a callout");
            assert_eq!(at, start);
            let error = wrap(toward - truth).abs();
            assert!(
                error <= c.hit_bearing_error_deg.to_radians() + 1e-4,
                "{error}"
            );
            assert!(reflex.take_hit_note().is_none(), "once");
            // It looks that way at a fixed point, never at his distance.
            let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
            let look = o.look_at.expect("looks");
            assert!(
                (look.distance_xz(start) - c.hit_look_m).abs() < 0.1,
                "{look:?}"
            );
            assert!(wrap(bearing(start, look) - toward).abs() < 1e-3);
            assert!(o.glance, "a use or a carry keeps its look");
            assert!(!o.fire);
            let Some(legs) = o.legs else {
                continue;
            };
            hid += 1;
            assert_eq!(legs.why, LegsWhy::BreakSight);
            // Cover that way: the rays toward him meet the wall.
            let eye = still_eye(legs.at, false);
            let [fx, fz] = forward(truth);
            let blocked = tactics
                .world
                .sight
                .ray_cast(eye, Vec3::new(fx, 0.0, fz), c.hit_cover_m);
            assert!(blocked.is_some(), "{:?}", legs.at);
            let d = legs.at.distance_xz(start);
            assert!((c.hide_m.0..=c.hide_m.1 + 0.5).contains(&d), "{d}");
            // There: it waits, then only goes back to its play.
            me.feet = legs.at;
            let mut now = 2_000;
            while now < 9_000 {
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                assert_ne!(o.legs.map(|l| l.why), Some(LegsWhy::RePeek), "{now}");
                if now < 2_000 + c.still_wait_ms.0 {
                    assert_eq!(o.legs.map(|l| l.why), Some(LegsWhy::BreakSight), "{now}");
                }
                now += 250;
            }
        }
        assert!(hid >= 5, "{hid} of 6");

        // A carrier keeps its way home (it looks, it does not hide); a hit
        // nobody fired at it (the shooter faces away) gives no direction.
        for (carrying, facing_me) in [(Some(0), true), (None, false)] {
            let mut me = body(0, 0, start, std::f32::consts::FRAC_PI_2);
            me.carrying = carrying;
            let yaw = bearing(shooter_at, start) + if facing_me { 0.0 } else { 1.0 };
            let mut view = view_of(vec![me.clone(), body(1, 1, shooter_at, yaw)]);
            view.sounds = vec![shot(1, shooter_at, 1_300)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                ..Situation::of(&view)
            };
            let memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(60);
            reflex.think_with(&me, &sit, &memory, 1_000, &aim, 0.5, &mut rng);
            me.health = 0.7;
            reflex.think_with(&me, &sit, &memory, 1_600, &aim, 0.5, &mut rng);
            let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.5, &mut rng);
            assert!(o.legs.is_none(), "{carrying:?} {facing_me}: {o:?}");
            assert_eq!(reflex.take_hit_note().is_some(), facing_me);
        }
    }

    /// Seen afterwards near the marker's direction, the shooter is taken
    /// as the target (if in range).
    #[test]
    fn an_enemy_seen_near_the_markers_direction_is_the_shooter() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let shooter_at = Vec3::new(0.0, 0.0, 60.0);
        let near_at = Vec3::new(-20.0, 0.0, 10.0);
        let me = body(0, 0, start, 0.0);
        let mut me_hit = me.clone();
        me_hit.health = 0.7;
        let mut view = view_of(vec![
            me.clone(),
            body(1, 1, shooter_at, bearing(shooter_at, start)),
            body(2, 1, near_at, 0.0),
        ]);
        let aim = AimTuning {
            combat: CombatTuning {
                hit_bearing: true,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(61);
        reflex.think_with(
            &me,
            &Situation::of(&view),
            &memory,
            1_000,
            &aim,
            0.5,
            &mut rng,
        );
        view.sounds = vec![shot(1, shooter_at, 1_300)];
        reflex.think_with(
            &me_hit,
            &Situation::of(&view),
            &memory,
            1_600,
            &aim,
            0.5,
            &mut rng,
        );
        assert!(reflex.take_hit_note().is_some());
        // Both seen and noticed a moment later: the one that way, though
        // the other is much nearer.
        for now in [1_700, 2_000] {
            sense(
                &mut memory,
                &[
                    percept(1, shooter_at, now, Source::Seen),
                    percept(2, near_at, now, Source::Seen),
                ],
                now,
            );
            reflex.think_with(
                &me_hit,
                &Situation::of(&view),
                &memory,
                now,
                &aim,
                0.5,
                &mut rng,
            );
        }
        assert_eq!(reflex.target(), Some(1));
    }

    /// A teammate's "shot from there" callout near: with no target, a
    /// glance along its direction, once.
    #[test]
    fn a_teammates_shot_from_callout_is_glanced_along_once() {
        // Facing +z (yaw 0); the teammate 10 m west, hit from the east.
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let view = view_of(vec![me.clone()]);
        let aim = AimTuning::default();
        let c = &aim.combat;
        let toward = bearing(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
        let from = Vec3::new(-10.0, 0.0, 0.0);
        for (by, far, glances) in [(3, false, true), (0, false, false), (3, true, false)] {
            let at = if far {
                Vec3::new(-60.0, 0.0, 0.0)
            } else {
                from
            };
            let mut team = TeamKnowledge::default();
            team.shots_from.push(crate::v026g::bots::know::ShotFrom {
                by,
                at,
                bearing: toward,
                time: 1_000,
            });
            let sit = Situation {
                team: Some(&team),
                ..Situation::of(&view)
            };
            let memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(62);
            let mut looks = 0;
            let mut now = 1_000;
            while now < 4_000 {
                if now % 250 == 0 {
                    reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                }
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                if let Some(look) = o.look_at {
                    assert!(o.glance);
                    assert!(look.distance_xz(along(at, toward, c.hit_look_m)) < 0.1);
                    looks += 1;
                }
                now += 50;
            }
            assert_eq!(looks > 0, glances, "{by} {far}");
            assert!(looks as u32 * 50 <= c.glance_ms + 50, "once: {looks}");
        }
    }

    /// Report section 10 (c): an Uzi, a TOZ-34 or an AK-74U that sees a
    /// sniper beyond its own weapon's band does not step in at him; with
    /// `outranged_cover` (off by default: the bench lost kills) it breaks
    /// sight and waits (no re-peek); against another weapon it steps in as
    /// before; a carrier keeps its way.
    #[test]
    fn outranged_by_a_seen_sniper_a_short_weapon_hides_instead_of_stepping_in() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        for cover in [true, false] {
            outranged_case(&tactics, cover);
        }
    }

    fn outranged_case(tactics: &Tactics, cover: bool) {
        let aim = AimTuning {
            combat: CombatTuning {
                outranged_cover: cover,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let c = &aim.combat;
        // 46 m: past the SMG's and the shotgun's bands, in the rifle's.
        let start = Vec3::new(24.0, 0.0, 30.0);
        let enemy_at = Vec3::new(70.0, 0.0, 30.0);
        for (own, enemy, carrying, want) in [
            (
                WeaponKind::Smg,
                WeaponKind::Sniper,
                false,
                Some(LegsWhy::BreakSight),
            ),
            (
                WeaponKind::Shotgun,
                WeaponKind::Sniper,
                false,
                Some(LegsWhy::BreakSight),
            ),
            (
                WeaponKind::Smg,
                WeaponKind::Rifle,
                false,
                Some(LegsWhy::Range),
            ),
            (WeaponKind::Rifle, WeaponKind::Rifle, false, None),
            (WeaponKind::Sniper, WeaponKind::Sniper, false, None),
            (WeaponKind::Smg, WeaponKind::Sniper, true, None),
        ] {
            let mut me = body(0, 0, start, 0.0);
            me.carrying = carrying.then_some(0);
            let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
            let weapons = [Some(own), Some(enemy)];
            let sit = Situation {
                tactics: Some(tactics),
                side: Some(0),
                weapon: Some(own),
                weapons: &weapons,
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(63);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
            let why = o.legs.map(|l| l.why);
            let want = want.filter(|w| cover || *w != LegsWhy::BreakSight);
            assert_eq!(
                why, want,
                "{own:?} at {enemy:?}, carrying {carrying}, cover {cover}"
            );
            if want == Some(LegsWhy::BreakSight) {
                let eye = |p: Vec3| still_eye(p, false);
                let hide = o.legs.unwrap().at;
                assert!(!tactics.sees(eye(enemy_at), eye(hide)), "{hide:?}");
                // There it waits, still seen or not, and never re-peeks.
                me.feet = hide;
                let mut now = 1_500;
                while now < 6_000 {
                    reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                    let o = reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng);
                    assert_ne!(o.legs.map(|l| l.why), Some(LegsWhy::RePeek));
                    assert_ne!(o.legs.map(|l| l.why), Some(LegsWhy::Range));
                    now += 250;
                }
                assert!(c.outranged_again_ms >= c.still_wait_ms.1);
            }
        }
    }

    /// Match 0x15aa5b99 at 658-690 s: a Mosin bot backed off a battery
    /// carrier at 9-23 m and he walked home. A carrier cannot shoot: no
    /// step back from him; one who is not carrying is stepped back from as
    /// before. The carrier is the one the team saw carrying (`band_carrier_seen_ms`), not the world's
    /// word: a battery the world has in his hands the team has not seen is not held to, and a sighting
    /// older than the window is not either.
    #[test]
    fn a_sniper_holds_its_ground_at_a_battery_carrier() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let aim = AimTuning::default();
        let off = AimTuning {
            combat: CombatTuning { band_carrier_seen_ms: 0, ..CombatTuning::default() },
            ..AimTuning::default()
        };
        // (the world's carrying, seen carrying at, the aim, steps back)
        let cases = [
            (true, Some(1_300), &aim, false),
            (false, None, &aim, true),
            (true, None, &aim, true),
            (true, Some(1_300 - 600), &aim, true),
            (false, Some(1_300), &aim, false),
            (true, None, &off, false),
        ];
        for (carrying, saw, aim, steps) in cases {
            let me = body(0, 0, Vec3::new(20.0, 0.0, 32.0), 0.0);
            let enemy_at = Vec3::new(30.0, 0.0, 32.0);
            let mut enemy = body(1, 1, enemy_at, 0.0);
            enemy.carrying = carrying.then_some(0);
            let view = view_of(vec![me.clone(), enemy]);
            let weapons = [Some(WeaponKind::Sniper), Some(WeaponKind::Rifle)];
            let mut team = TeamKnowledge::default();
            if let Some(time) = saw {
                team.saw_carrying(1, 0, enemy_at, time);
            }
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                weapon: Some(WeaponKind::Sniper),
                weapons: &weapons,
                team: Some(&team),
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(64);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, aim, 0.5, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, 1_350, true, aim, 0.5, &mut rng);
            assert_eq!(
                o.legs.is_some_and(|l| l.why == LegsWhy::Range),
                steps,
                "world {carrying}, saw {saw:?}, window {}: {o:?}",
                aim.combat.band_carrier_seen_ms
            );
        }
    }

    /// The aim with the worth, the keep of the enemy who hit the bot and the push on ([`WorthTuning::parts_on`]): off by default.
    fn worth_on() -> AimTuning {
        let mut aim = AimTuning::default();
        aim.combat.worth = WorthTuning::parts_on();
        aim
    }

    /// The target the reflex takes after a few thinks, with `enemies` seen as (place, feet, carrying).
    fn target_among(
        enemies: &[(Place, Vec3, Option<usize>)],
        team: Option<&TeamKnowledge>,
        aim: &AimTuning,
    ) -> Option<Place> {
        reflex_among(enemies, team, aim).target()
    }

    /// The reflex after those thinks.
    fn reflex_among(
        enemies: &[(Place, Vec3, Option<usize>)],
        team: Option<&TeamKnowledge>,
        aim: &AimTuning,
    ) -> Reflex {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let mut places = vec![me.clone()];
        for &(who, at, carrying) in enemies {
            let mut enemy = body(who, 1, at, 0.0);
            enemy.carrying = carrying;
            places.push(enemy);
        }
        let view = view_of(places);
        let sit = Situation { team, ..Situation::of(&view) };
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(21);
        for now in [1_000, 1_300, 1_600] {
            let seen: Vec<_> = enemies
                .iter()
                .map(|&(who, at, _)| percept(who, at, now, Source::Seen))
                .collect();
            sense(&mut memory, &seen, now);
            reflex.think_with(&me, &sit, &memory, now, aim, 0.5, &mut rng);
        }
        reflex
    }

    /// The enemy who hit the bot stays its target for `hurter_keep_ms`: a carrier farther out takes him neither by the
    /// worth gap nor by the score. With the keep off (the tip: the shooter goes first for one think) the carrier takes the target
    /// at the next think.
    #[test]
    fn the_enemy_who_hit_the_bot_is_not_given_up_for_a_carrier() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let hurter_at = Vec3::new(0.0, 0.0, 25.0);
        let carrier_at = Vec3::new(0.0, 0.0, 30.0);
        let mut carrier = body(2, 1, carrier_at, 0.0);
        carrier.carrying = Some(1);
        let view = view_of(vec![me.clone(), body(1, 1, hurter_at, 0.0), carrier]);
        let sit = Situation::of(&view);
        let run = |keep_ms: u32| {
            let mut aim = worth_on();
            aim.combat.worth.hurter_keep_ms = keep_ms;
            let mut memory = Memory::default();
            let mut reflex = Reflex { threat: Some(Threat { at: hurter_at, error_m: 1.0, until: 5_000, hit: true, bearing: None }), ..Reflex::default() };
            let mut rng = Rng::new(25);
            let mut targets = Vec::new();
            for now in [1_000, 1_300, 1_600, 1_900, 2_200, 2_500, 2_800] {
                let seen = [percept(1, hurter_at, now, Source::Seen), percept(2, carrier_at, now, Source::Seen)];
                sense(&mut memory, &seen, now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                targets.push(reflex.target());
            }
            targets
        };
        let kept = run(3_000);
        assert!(kept.iter().flatten().all(|&t| t == 1) && kept.last() == Some(&Some(1)), "the hurter is kept: {kept:?}");
        let off = run(0);
        assert!(off.contains(&Some(1)) && off.last() == Some(&Some(2)), "the shooter first, then the carrier: {off:?}");
        // Hit a moment ago by someone it could not place: the worth steers nothing in the fight (the nearer enemy is taken),
        // and after the window the carrier is.
        let after_hit = |keep_ms: u32| {
            let mut aim = worth_on();
            aim.combat.worth.hurter_keep_ms = keep_ms;
            let mut memory = Memory::default();
            let mut reflex = Reflex { hit_at: Some(900), ..Reflex::default() };
            let mut rng = Rng::new(26);
            let mut targets = Vec::new();
            for now in [1_000, 1_300, 1_600, 1_900, 4_500, 4_800] {
                let seen = [percept(1, hurter_at, now, Source::Seen), percept(2, carrier_at, now, Source::Seen)];
                sense(&mut memory, &seen, now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
                targets.push(reflex.target());
            }
            targets
        };
        let kept = after_hit(3_000);
        assert!(kept[..4].iter().flatten().all(|&t| t == 1) && kept.last() == Some(&Some(2)), "{kept:?}");
        assert_eq!(after_hit(0).last(), Some(&Some(2)));
    }

    /// The target after a few thinks, a bot with a rifle (reach 53 m) among `enemies` seen as (place, feet, carrying).
    fn rifle_target(enemies: &[(Place, Vec3, Option<usize>)], worth: WorthTuning) -> (Option<Place>, WorthStats) {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let mut places = vec![me.clone()];
        for &(who, at, carrying) in enemies {
            let mut enemy = body(who, 1, at, 0.0);
            enemy.carrying = carrying;
            places.push(enemy);
        }
        let view = view_of(places);
        let sit = Situation { weapon: Some(WeaponKind::Rifle), ..Situation::of(&view) };
        let mut aim = AimTuning::default();
        aim.combat.worth = worth;
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(21);
        for now in [1_000, 1_300, 1_600, 1_900] {
            let seen: Vec<_> = enemies.iter().map(|&(who, at, _)| percept(who, at, now, Source::Seen)).collect();
            sense(&mut memory, &seen, now);
            reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
        }
        (reflex.target(), reflex.worth_stats())
    }

    /// A carrier past the reach of the bot's weapon is worth nothing to it: the nearer enemy is its target (a mate inside the reach
    /// takes the carrier); inside the reach he is worth what he was.
    #[test]
    fn a_carrier_past_the_reach_of_the_weapon_is_no_reason_for_a_target() {
        let (plain, far_carrier) = ((1, Vec3::new(0.0, 0.0, 45.0), None), (2, Vec3::new(0.0, 0.0, 62.0), Some(1)));
        let on = WorthTuning { objective_in_band: true, ..WorthTuning::parts_on() };
        assert_eq!(rifle_target(&[plain, far_carrier], WorthTuning::parts_on()).0, Some(2), "without the reach 62 m - 25 m is nearer than 45 m");
        let (target, stats) = rifle_target(&[plain, far_carrier], on.clone());
        assert_eq!(target, Some(1));
        assert_eq!((stats.actor, stats.out_of_reach), (0, 1));
        let inside = (2, Vec3::new(0.0, 0.0, 52.0), Some(1));
        assert_eq!(rifle_target(&[plain, inside], on).0, Some(2), "52 m is inside the 50 m band and its slack");
    }

    /// The real gap: the carrier's extra distance must be paid by the worth with the gap to spare, in a plain choice and in a switch.
    #[test]
    fn a_carrier_takes_the_target_only_by_a_real_gap_over_the_nearer_enemy() {
        let (plain, carrier) = ((1, Vec3::new(0.0, 0.0, 30.0), None), (2, Vec3::new(0.0, 0.0, 45.0), Some(1)));
        let with = |gap: Option<f32>| WorthTuning { switch_gap_m: gap, ..WorthTuning::parts_on() };
        assert_eq!(rifle_target(&[plain, carrier], with(None)).0, Some(2), "45 - 25 = 20 m beats 30 m");
        assert_eq!(rifle_target(&[plain, carrier], with(Some(5.0))).0, Some(2), "20 + 5 still beats 30");
        assert_eq!(rifle_target(&[plain, carrier], with(Some(12.0))).0, Some(1), "20 + 12 does not");
    }

    /// A target taken for his worth is counted once and noted once, with the numbers; one that is the
    /// nearest anyway is neither.
    #[test]
    fn a_target_taken_for_his_worth_is_counted_and_noted_once() {
        let aim = worth_on();
        let plain = (1, Vec3::new(0.0, 0.0, 12.0), None);
        let carrier = (2, Vec3::new(0.0, 0.0, 18.0), Some(1));
        let mut reflex = reflex_among(&[plain, carrier], None, &aim);
        let notes = reflex.take_worth_notes();
        assert_eq!(reflex.target(), Some(2));
        assert_eq!(reflex.worth_stats().actor, 1);
        assert_eq!(reflex.worth_stats().total(), 1);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("targets 2 (objective actor (carrier)) over the nearer 1"), "{notes:?}");
        let mut near = reflex_among(&[(1, Vec3::new(0.0, 0.0, 12.0), Some(1)), (2, Vec3::new(0.0, 0.0, 30.0), None)], None, &aim);
        assert_eq!(near.worth_stats().total(), 0, "the carrier is the nearest anyway");
        assert!(near.take_worth_notes().is_empty());
    }

    /// The Cologne steal (match 0x72dafb5b, 80-103 s): a carrier walked past ten bots who shot his
    /// escorts. A carrier cannot shoot back and a thief is on the HUD: both come before a nearer plain
    /// enemy, the thief before a carrier of the enemy's own battery; with the worth off, the nearest.
    #[test]
    fn a_carrier_and_a_thief_are_shot_before_a_nearer_plain_enemy() {
        let (plain, carrier, thief) = (
            (1, Vec3::new(0.0, 0.0, 12.0), None),
            (2, Vec3::new(0.0, 0.0, 18.0), Some(1)),
            (3, Vec3::new(5.0, 0.0, 24.0), Some(0)),
        );
        let on = worth_on();
        let mut off = AimTuning::default();
        off.combat.worth = crate::v026g::bots::worth::WorthTuning {
            objective_actor_m: 0.0,
            objective_threat_m: 0.0,
            switch_m: 0.0,
            ..off.combat.worth
        };
        let mut team = TeamKnowledge::default();
        team.thefts.push(crate::v026g::bots::know::Theft {
            battery: 0,
            at: Vec3::ZERO,
            time: 900,
            thief: Some(3),
        });
        assert_eq!(target_among(&[plain, carrier], None, &on), Some(2));
        assert_eq!(target_among(&[plain, carrier], None, &off), Some(1));
        assert_eq!(target_among(&[plain, carrier, thief], Some(&team), &on), Some(3));
        assert_eq!(target_among(&[plain, carrier, thief], Some(&team), &off), Some(1));
        // No theft noticed: he is a carrier like the other, and the nearer one is taken.
        team.thefts.clear();
        assert_eq!(target_among(&[plain, carrier, thief], Some(&team), &on), Some(2));
    }

    /// An enemy the team saw hit to pain (a hand and a leg: the body pool shows nothing) counts nearer;
    /// one the team saw nothing land on does not, whatever his true pools are.
    #[test]
    fn an_enemy_the_team_saw_hurt_is_taken_before_a_nearer_whole_one() {
        use crate::v026g::bots::worth::HurtFrom;
        let near = (1, Vec3::new(0.0, 0.0, 12.0), None);
        let hurt = (2, Vec3::new(0.0, 0.0, 18.0), None);
        let mut team = TeamKnowledge::default();
        team.wounds.landed(2, 0, 0.9, 0, 1_000);
        let aim = AimTuning::default();
        assert_eq!(target_among(&[near, hurt], Some(&team), &aim), Some(2));
        assert_eq!(target_among(&[near, hurt], None, &aim), Some(1), "nothing seen of him");
        // The tip read the true body pool: both whole there, the nearer.
        let mut tip = AimTuning::default();
        tip.combat.worth.hurt_from = HurtFrom::Body;
        assert_eq!(target_among(&[near, hurt], Some(&team), &tip), Some(1));
    }

    /// A hurt enemy this bot fired at breaks sight within 30 m: it goes on his last place, two bots at
    /// most; a bot that did not fire at him, a whole enemy, and a team already at the cap do not.
    #[test]
    fn a_bot_that_hit_a_hurt_enemy_goes_after_him_two_at_most() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy_at = Vec3::new(0.0, 0.0, 20.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let aim = worth_on();
        let run = |share: f32, fired: Option<(Place, u32)>, others: usize| {
            let mut team = TeamKnowledge::default();
            team.wounds.landed(1, 0, share, 0, 1_000);
            for other in 0..others {
                team.wounds.push_claim(1, 10 + other, 6_000);
            }
            let sit = Situation { team: Some(&team), ..Situation::of(&view) };
            let mut memory = Memory::default();
            sense(&mut memory, &[percept(1, enemy_at, 1_100, Source::Heard)], 1_100);
            let mut reflex = Reflex { last_fired: fired, ..Reflex::default() };
            let mut rng = Rng::new(22);
            reflex.think_with(&me, &sit, &memory, 1_500, &aim, 0.5, &mut rng);
            let legs = reflex.tick(&me, &view, &memory, 1_550, true, &aim, 0.5, &mut rng).legs;
            (reflex.pushing(), legs.map(|l| (l.why, l.at)))
        };
        let (claim, legs) = run(0.9, Some((1, 990)), 1);
        assert_eq!(claim, Some((1, 7_000)));
        assert_eq!(legs, Some((LegsWhy::Finish(1), enemy_at)));
        assert_eq!(run(0.9, Some((1, 990)), 2).0, None, "two bots push already");
        assert_eq!(run(0.9, None, 0).0, None, "it did not fire at him");
        assert_eq!(run(0.9, Some((3, 990)), 0).0, None, "it fired at another");
        assert_eq!(run(0.2, Some((1, 990)), 0).0, None, "not hurt enough");
    }

    /// Fairness: an enemy no sense of the team gave (a cheat's radar place only) is not pushed on, however hurt the team saw him
    /// get, nor is one last sensed more than 15 s ago; and a hurt enemy who is not in sight changes no choice of a target.
    #[test]
    fn the_worth_and_the_push_use_only_what_the_team_sensed() {
        use crate::v026g::bots::know::CheatKind;
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let enemy_at = Vec3::new(0.0, 0.0, 20.0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        let aim = worth_on();
        let pushes = |source: Source, sensed_at: u32| {
            let mut team = TeamKnowledge::default();
            team.wounds.landed(1, 0, 0.9, 0, 1_000);
            let sit = Situation { team: Some(&team), ..Situation::of(&view) };
            let mut memory = Memory::default();
            sense(&mut memory, &[percept(1, enemy_at, sensed_at, source)], sensed_at);
            let mut reflex = Reflex { last_fired: Some((1, 990)), ..Reflex::default() };
            let mut rng = Rng::new(24);
            reflex.think_with(&me, &sit, &memory, 1_500, &aim, 0.5, &mut rng);
            reflex.pushing().is_some()
        };
        assert!(pushes(Source::Heard, 1_100), "heard by the team: the push");
        assert!(!pushes(Source::Cheat(CheatKind::Radar), 1_100), "a cheat's place is not a sense");
        let mut team = TeamKnowledge::default();
        team.wounds.landed(1, 0, 0.9, 0, 1_000);
        // A hurt enemy nobody sees: the target among the seen ones is the nearest, with no note of a worth.
        let seen = (2, Vec3::new(0.0, 0.0, 30.0), None);
        let mut reflex = reflex_among(&[seen], Some(&team), &aim);
        assert_eq!(reflex.target(), Some(2));
        assert!(reflex.take_worth_notes().is_empty());
        assert_eq!(reflex.worth_stats().total(), 0);
    }

    /// A target held, a mate's callout of an enemy behind: a glance there; off, none.
    #[test]
    fn a_bot_holding_a_target_glances_at_a_callout_behind() {
        let me = body(0, 0, Vec3::ZERO, 0.0);
        let ahead = Vec3::new(0.0, 0.0, 25.0);
        let behind = Vec3::new(0.0, 0.0, -15.0);
        let view = view_of(vec![me.clone(), body(1, 1, ahead, 0.0), body(2, 1, behind, 0.0)]);
        let sit = Situation::of(&view);
        let run = |aim: &AimTuning| {
            let mut memory = Memory::default();
            let mut reflex = Reflex { pause_until: 100_000, next_press: 100_000, ..Reflex::default() };
            let mut rng = Rng::new(23);
            for now in [1_000, 1_300, 1_600, 1_900] {
                sense(
                    &mut memory,
                    &[
                        percept(1, ahead, now, Source::Seen),
                        percept(2, behind, now, Source::Told { by: 5 }),
                    ],
                    now,
                );
                reflex.think_with(&me, &sit, &memory, now, aim, 0.5, &mut rng);
            }
            (reflex.target(), reflex.threat.map(|t| (t.at, t.hit)))
        };
        // Off by default (no source found for it, and no effect of its own in the bench); on at 40 m.
        assert_eq!(run(&AimTuning::default()), (Some(1), None));
        let mut on = worth_on();
        on.combat.worth.flank_glance_m = 40.0;
        assert_eq!(run(&on), (Some(1), Some((behind, false))));
        // In a pause shorter than the least, none; with the least off, at any time.
        let mut short = on.clone();
        short.combat.worth.flank_glance_min_pause_ms = 200_000;
        assert_eq!(run(&short), (Some(1), None));
        short.combat.worth.flank_glance_min_pause_ms = 0;
        assert_eq!(run(&short), (Some(1), Some((behind, false))));
    }

    /// Audit issue 4's numbers, explicit (the tests hold whatever the
    /// defaults become).
    fn issue_4_aim() -> AimTuning {
        AimTuning {
            combat: CombatTuning {
                switch_share: 0.5,
                switch_near_m: 15.0,
                switch_margin_m: 5.0,
                switch_hold_ms: 300,
                switch_facing_deg: 20.0,
                switch_facing_margin_m: 10.0,
                switch_between_thinks: true,
                glance_heard_m: 10.0,
                glance_heard_share: 0.5,
                glance_close_m: 6.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        }
    }

    /// Audit issue 4 (match 0x4da3b51d at 190-199 s): an enemy steps out 4 m
    /// beside a bot holding a target 60 m away. Noticed between two thinks,
    /// he is the target at the tick he is noticed, not a think later.
    #[test]
    fn a_close_enemy_beside_a_bot_with_a_far_target_is_taken_at_once() {
        let (far, near) = (Vec3::new(0.0, 0.0, 60.0), Vec3::new(3.0, 0.0, 2.5));
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        facing(&mut me, far + Vec3::new(0.0, 1.3, 0.0));
        let view = view_of(vec![
            me.clone(),
            body(1, 1, far, 0.0),
            body(2, 1, near, 0.0),
        ]);
        for between in [true, false] {
            let mut aim = issue_4_aim();
            aim.combat.switch_between_thinks = between;
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(50);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, far, now, Source::Seen)], now);
                let sit = Situation::of(&view);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.6, &mut rng);
            }
            assert_eq!(reflex.target(), Some(1));
            // He shows at 1 400 (noticed 300 ms on); the next think would
            // be at 1 900.
            let mut taken = None;
            for now in (1_350..1_900).step_by(50) {
                let mut seen = vec![percept(1, far, now, Source::Seen)];
                if now >= 1_400 {
                    seen.push(percept(2, near, now, Source::Seen));
                }
                sense(&mut memory, &seen, now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 0.6, &mut rng);
                if taken.is_none() && reflex.target() == Some(2) {
                    taken = Some(now);
                }
            }
            if between {
                assert_eq!(taken, Some(1_700), "taken at the tick he is noticed");
            } else {
                assert_eq!(taken, None, "a think later");
            }
        }
    }

    /// Audit issue 4 (match 0x130b8388 at 129.8 s): a bot aimed at a bot
    /// 45 m away with the owner in its sight at 25 m. One facing the bot
    /// and 10 m nearer takes the target from one who does not face it,
    /// once the target was held `switch_hold_ms`; one not facing it does
    /// not.
    #[test]
    fn a_nearer_enemy_facing_the_bot_takes_the_target_from_one_who_does_not() {
        let aim = issue_4_aim();
        let (far, other) = (Vec3::new(0.0, 0.0, 45.0), Vec3::new(10.0, 0.0, 23.0));
        let mut me = body(0, 0, Vec3::ZERO, 0.0);
        facing(&mut me, far + Vec3::new(0.0, 1.3, 0.0));
        // The far one looks away (+z, yaw 0); the other at the bot or away.
        let toward_me = bearing(other, me.feet);
        for (other_yaw, switches) in [(toward_me, true), (toward_me + 1.2, false)] {
            let view = view_of(vec![
                me.clone(),
                body(1, 1, far, 0.0),
                body(2, 1, other, other_yaw),
            ]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(51);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, far, now, Source::Seen)], now);
                let sit = Situation::of(&view);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.6, &mut rng);
            }
            assert_eq!(reflex.target(), Some(1));
            for now in [1_600, 1_900, 2_200] {
                let seen = [
                    percept(1, far, now, Source::Seen),
                    percept(2, other, now, Source::Seen),
                ];
                sense(&mut memory, &seen, now);
                let sit = Situation::of(&view);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.6, &mut rng);
            }
            let want = if switches { 2 } else { 1 };
            assert_eq!(reflex.target(), Some(want), "facing the bot: {switches}");
            // The hold: just short of it, the facing one does not press.
            if switches {
                let (kept, by) = (memory.seen(1, 2_200, 2_200), memory.seen(2, 2_200, 2_200));
                let (kept, by) = (kept.unwrap(), by.unwrap());
                let c = &aim.combat;
                assert!(presses_more(&me, Some(&view), None, &kept, &by, c.switch_hold_ms, c));
                assert!(!presses_more(&me, Some(&view), None, &kept, &by, c.switch_hold_ms - 1, c));
            }
        }
    }

    /// Audit issue 4: steps heard 5 m behind a bot holding a target 9 m
    /// away (not under half its distance) turn it for a glance, as a
    /// player checks a noise at arm's length; at 7 m they do not, nor with
    /// the close glance off.
    #[test]
    fn steps_heard_within_six_metres_are_glanced_at_while_holding_a_target() {
        let target_at = Vec3::new(0.0, 0.0, 9.0);
        for (heard_at, close_m, turns) in [
            (Vec3::new(0.5, 0.0, -5.0), 6.0, true),
            (Vec3::new(0.5, 0.0, -7.0), 6.0, false),
            (Vec3::new(0.5, 0.0, -5.0), 0.0, false),
        ] {
            let mut aim = issue_4_aim();
            aim.combat.glance_close_m = close_m;
            let mut me = body(0, 0, Vec3::ZERO, 0.0);
            facing(&mut me, target_at + Vec3::new(0.0, 1.3, 0.0));
            let view = view_of(vec![
                me.clone(),
                body(1, 1, target_at, 0.0),
                body(2, 1, heard_at, 0.0),
            ]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(53);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, target_at, now, Source::Seen)], now);
                let sit = Situation::of(&view);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.6, &mut rng);
            }
            assert_eq!(reflex.target(), Some(1));
            let percepts = [
                percept(1, target_at, 1_600, Source::Seen),
                percept(2, heard_at, 1_600, Source::Heard),
            ];
            sense(&mut memory, &percepts, 1_600);
            let sit = Situation::of(&view);
            reflex.think_with(&me, &sit, &memory, 1_600, &aim, 0.6, &mut rng);
            let o = reflex.tick(&me, &view, &memory, 1_650, true, &aim, 0.6, &mut rng);
            let looks = o.look_at.is_some_and(|p| p.distance_xz(heard_at) < 0.5);
            assert_eq!(looks, turns, "{heard_at:?} {close_m}: {o:?}");
            if turns {
                assert!(o.glance && !o.fire, "{o:?}");
            }
        }
    }

    /// Audit issue 8: with `aim_in_first`, no shot at 20 m until the aim
    /// has been pressed for the weapon's `aim_time` (and the margin): a strafing Uzi's hip cone (10.3 degrees) would put most
    /// shots off him. At 3 m the hip cone keeps them on him: it fires from
    /// the hip. Without the rule it fires at 20 m before the sights are up.
    #[test]
    fn it_fires_only_once_the_sights_are_up_unless_the_hip_cone_fits() {
        for (at_m, first, hip) in [(20.0, true, false), (3.0, true, true), (20.0, false, false)] {
            let aim = AimTuning {
                combat: CombatTuning {
                    aim_in_first: first,
                    ads_near_chance: 0.0,
                    ..CombatTuning::default()
                },
                ..AimTuning::default()
            };
            let enemy_at = Vec3::new(0.0, 0.0, at_m);
            let mut me = body(0, 0, Vec3::ZERO, 0.0);
            let enemy = body(1, 1, enemy_at, 0.0);
            facing(&mut me, enemy.chest());
            let view = view_of(vec![me.clone(), enemy]);
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(11);
            let sight = |time| percept(1, enemy_at, time, Source::Seen);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[sight(now)], now);
                reflex.think(&me, &memory, now, &aim, 1.0, &mut rng);
            }
            assert_eq!(reflex.target(), Some(1));
            let aim_in = reflex.held_gun().aim_ms + aim.combat.aim_in_margin_ms;
            let (mut pressed, mut fired) = (None, None);
            for k in 0..40 {
                let now = 1_300 + k * 50;
                sense(&mut memory, &[sight(now)], now);
                let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                if o.keys & key::AIM != 0 && pressed.is_none() {
                    pressed = Some(now);
                }
                if o.fire && fired.is_none() {
                    fired = Some(now);
                }
            }
            let fired = fired.expect("it fires");
            if hip {
                assert_eq!(pressed, None, "{at_m} m: from the hip");
            } else if first {
                let pressed = pressed.expect("aimed");
                assert!(fired >= pressed + aim_in, "{at_m} m: {fired} {pressed}");
            } else {
                assert!(fired < pressed.expect("aimed") + aim_in, "{at_m} m without: {fired} {pressed:?}");
            }
        }
    }

    /// The heal habit: the field of `hit_in_a_duel_it_breaks_sight...`, the
    /// bot west of the wall at (32, 28) with a target 23 m off. Returns the
    /// reflex, the world and what the walk needs.
    fn heal_duel(enemy_at: Vec3, carrying: bool) -> (Reflex, PlaceView, MatchView, Memory, Rng) {
        let mut me = body(0, 0, Vec3::new(32.0, 0.0, 28.0), 0.0);
        me.carrying = carrying.then_some(0);
        let view = view_of(vec![me.clone(), body(1, 1, enemy_at, 0.0)]);
        (Reflex::default(), me, view, Memory::default(), Rng::new(8))
    }

    fn wish(reach_m: f32) -> HealWish {
        HealWish {
            key: key::QUICK[0],
            reach_m,
            enemy_m: 10.0,
        }
    }

    #[test]
    fn a_heal_wish_presses_the_key_once_beside_everything_else_the_bot_does() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let enemy_at = Vec3::new(55.0, 0.0, 30.0);
        let aim = AimTuning::default();
        let run = |wished: bool| {
            let (mut reflex, me, view, mut memory, mut rng) = heal_duel(enemy_at, false);
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                ..Situation::of(&view)
            };
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            let mut out = Vec::new();
            for now in [1_350, 1_400, 1_450] {
                reflex.set_heal(wished.then(|| wish(7.0)).filter(|_| now == 1_350));
                out.push(reflex.tick(&me, &view, &memory, now, true, &aim, 0.5, &mut rng));
            }
            (out, reflex.heal_presses)
        };
        let (with, presses) = run(true);
        let (without, none) = run(false);
        assert_eq!((presses, none), (1, 0));
        // The key once, in the tick asked, and nothing else differs: the fire, the sights, the legs, the other keys.
        assert_eq!(with[0].keys & key::QUICK[0], key::QUICK[0]);
        assert_eq!(with[1].keys & key::QUICK[0], 0);
        assert_eq!(with[2].keys & key::QUICK[0], 0);
        for (a, b) in with.iter().zip(&without) {
            assert_eq!(a.keys & !key::QUICK[0], b.keys, "no other key changes");
            assert_eq!((a.fire, a.legs, a.look_at), (b.fire, b.legs, b.look_at));
        }
    }

    #[test]
    fn a_carrier_presses_nothing_and_walks_nowhere() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let enemy_at = Vec3::new(55.0, 0.0, 30.0);
        let aim = AimTuning::default();
        let (mut reflex, me, view, mut memory, mut rng) = heal_duel(enemy_at, true);
        let sit = Situation {
            tactics: Some(&tactics),
            side: Some(0),
            ..Situation::of(&view)
        };
        for now in [1_000, 1_300] {
            sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
            reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
        }
        reflex.set_heal(Some(wish(7.0)));
        let o = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
        assert_eq!(o.keys & key::QUICK[0], 0, "a battery in the hands: the key does nothing");
        assert_eq!(reflex.heal_presses, 0);
        reflex.think_with(&me, &sit, &memory, 1_600, &aim, 0.5, &mut rng);
        assert_eq!(reflex.heal_walks, 0);
    }

    #[test]
    fn in_a_duel_it_walks_to_cover_within_the_items_delay_and_not_when_it_should_not() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let enemy_at = Vec3::new(55.0, 0.0, 30.0);
        let aim = AimTuning::default();
        // Press, then the next think: the reflex's own cover search, within the reach, the key not blocked by it.
        let walk = |reach_m: f32, enemy_at: Vec3, think_at: u32| {
            let (mut reflex, mut me, view, mut memory, mut rng) = heal_duel(enemy_at, false);
            me.feet = Vec3::new(32.0, 0.0, 28.0);
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                ..Situation::of(&view)
            };
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            reflex.set_heal(Some(wish(reach_m)));
            let pressed = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
            assert_eq!(pressed.keys & key::QUICK[0], key::QUICK[0]);
            sense(&mut memory, &[percept(1, enemy_at, think_at, Source::Seen)], think_at);
            reflex.think_with(&me, &sit, &memory, think_at, &aim, 0.5, &mut rng);
            let o = reflex.tick(&me, &view, &memory, think_at + 50, true, &aim, 0.5, &mut rng);
            (o.legs, reflex.heal_walks, me.feet, reflex.heal_note())
        };
        let (legs, walks, from, note) = walk(7.0, enemy_at, 1_600);
        let legs = legs.expect("walks to cover");
        assert_eq!((legs.why, walks), (LegsWhy::BreakSight, 1));
        let d = legs.at.distance_xz(from);
        assert!((2.0..=7.5).contains(&d), "{d}");
        // The note: walked, the nearest known enemy 23 m off, the spot's distance.
        let note = note.expect("a note");
        assert_eq!(note.why, HealWhy::Walked);
        assert!((note.enemy_m.unwrap() - 23.4).abs() < 0.5, "{note:?}");
        assert!((note.cover_m.unwrap() - d).abs() < 1e-3, "{note:?}");
        let eye = |p: Vec3| p + Vec3::new(0.0, 1.6, 0.0);
        assert!(!tactics.sees(eye(enemy_at), eye(legs.at)), "{:?}", legs.at);
        // A known enemy within 10 m: no walk (it fights on). The wall's west end is 15 m from him; this one 6 m.
        let near = Vec3::new(38.0, 0.0, 30.0);
        assert!(walk(7.0, near, 1_600).0.is_none_or(|l| l.why != LegsWhy::BreakSight));
        assert_eq!(walk(7.0, near, 1_600).1, 0);
        assert_eq!(walk(7.0, near, 1_600).3.map(|n| n.why), Some(HealWhy::EnemyNear));
        // A delay too short to walk to a spot 2 m off (a reach under the search's near edge), or a think too late.
        assert_eq!(walk(1.0, enemy_at, 1_600).1, 0);
        assert_eq!(walk(1.0, enemy_at, 1_600).3.map(|n| n.why), Some(HealWhy::ShortReach));
        assert_eq!(walk(7.0, enemy_at, 2_300).1, 0);
        assert_eq!(walk(7.0, enemy_at, 2_300).3.map(|n| n.why), Some(HealWhy::Late));
    }
}
