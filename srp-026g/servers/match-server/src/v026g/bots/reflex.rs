//! The combat reflex: a bot's second track (`player_bots_research.md` B1
//! point 2, Left 4 Dead's "Main" and "Legs"). The play decides where to be
//! and why; the reflex, at every think, picks a target, aims and fires,
//! turns to a sound, and may take the legs for a moment to survive or to
//! punish (research B1: it "may override movement to survive").
//!
//! The fairness rules hold by type: the reflex aims and fires only at a
//! `know::SeenTrack`, so never at what no sense gave, and never before the
//! noticing delay; the first shot waits a reaction delay more
//! (`AimTuning::reaction_ms`). Its aim is a human's: it starts off by
//! `AimTuning::error_deg` × 3 and settles toward its error over `settle_s`,
//! wanders slowly about the target as it tracks him (`wander_deg`); the
//! error never goes to 0 (a floor drawn every tick: no perfect recoil or
//! spread). Play 23 changes only the numbers that are aim or reaction
//! (`error_deg`, `wander_deg`, `settle_s`, `notice`, `reaction_ms`),
//! through `skill`; every number of [`CombatTuning`] is the same at every
//! skill.
//!
//! How it fires and stands, from the best players of the recorded matches
//! (`player_bots_behaviour.md` §5): an automatic weapon in bursts of
//! 0.5-1 s, a one-round weapon at every cycle of its own; down the sights
//! at a seen target (always from 10 m on); standing to aim it steps side
//! to side, and a hit turns the step; with a bolt action it stands (or
//! crouches) instead.
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
//! - **play 11, weapon matchup:** the distance band its weapon wants
//!   against the enemy's ([`preferred_band`]), stepped into while it duels;
//! - **play 19, spot:** a newly noticed enemy is spotted (bit 28, key down
//!   once, 3 s apart: the stock game's cooldown, kept as the bots' own
//!   pace since client patch 38 has none) once it is in the crosshair;
//! - **play 20, grenade before the peek:** rarely, at an enemy last seen
//!   holding an angle or at a place an enemy was sensed staying at, with
//!   grenades left (two a life), the grenade key held until the throw;
//! - **play 21, trade (its reflex half):** the partner died within
//!   `trade_ms`: look at that spot, aiming down the sights.

use mapdata::Vec3;
use serde::{Deserialize, Serialize};

use super::guns::{Arms, Gun, Hands};
use super::kin::{forward, wrap, yaw_of};
use super::know::{Memory, SeenTrack, Source, TeamKnowledge};
use super::recoil::{Cadence, Recoil, kick_of};
use super::rng::Rng;
use super::signals::mix;
use super::tactics::{SpotQuery, Tactics};
use super::tuning::AimTuning;
use super::view::{EYE_STANDING, MatchView, Place, PlaceView, SoundKind};
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
    /// first).
    pub fn of_item(kind: &str, category: u8) -> Option<Self> {
        Some(match (category, kind) {
            (13, _) => Self::Sniper,
            (14, _) => Self::Shotgun,
            (_, "wpn_dbrl" | "wpn_stgn" | "wpn_asgn") => Self::Shotgun,
            (_, "wpn_bolt") => Self::Sniper,
            (_, "wpn_smg") => Self::Smg,
            (_, "wpn_pstl" | "wpn_rvlr" | "wpn_apst") => Self::Pistol,
            (_, "wpn_aslt") => Self::Rifle,
            _ => return None,
        })
    }
}

/// The reflex's numbers besides the aim (`AimTuning::combat`). None of
/// them is mixed by skill: play 23 changes only aim and reaction.
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
    /// Play 11: the distance band each weapon wants, metres.
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
    /// At a place this far, metres.
    pub grenade_m: (f32, f32),
    /// The key held this long once the view is on the throw, ms (drawn).
    pub grenade_hold_ms: (u32, u32),
    pub grenade_within_deg: f32,
    /// Not with a teammate this near the place, metres.
    pub grenade_friend_m: f32,
    /// The view not on the throw by this, ms: given up.
    pub grenade_give_up_ms: u32,
    /// After a throw, the legs go at the place from/until this, ms.
    pub grenade_push_ms: (u32, u32),
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
    /// seen); its time is the weapon's.
    pub reload_idle_ms: u32,
    pub reload_some_share: f32,
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
    pub recoil_pull: f32,
    /// Aiming down the sights at a seen target: always from `ads_near_m`
    /// on, nearer with this chance (drawn per target).
    pub ads_near_m: f32,
    pub ads_near_chance: f32,
    /// Standing to aim at a seen target, the legs step side to side: runs
    /// of `strafe_run_ms` one way then the other, a still moment of
    /// `strafe_still_ms` between with `strafe_still_chance`; a hit turns
    /// the step at once. Never with a bolt-action weapon.
    pub strafe_run_ms: (u32, u32),
    pub strafe_still_chance: f32,
    pub strafe_still_ms: (u32, u32),
    /// With a bolt-action weapon, it crouches to fire at a target with
    /// this chance (drawn per target).
    pub sniper_crouch_chance: f32,
    /// The target kept while seen gives way to another seen enemy nearer
    /// than `switch_share` of its distance, or nearer than
    /// `switch_near_m` and `switch_margin_m` nearer than it (match
    /// 0x5402a730: a bot aimed at a bot 67 m away with the human 6 m from
    /// it).
    pub switch_share: f32,
    pub switch_near_m: f32,
    pub switch_margin_m: f32,
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
    /// against a far human sniper is not measured.
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
    /// every `outranged_again_ms`.
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
            band_smg: (5.0, 25.0),
            band_shotgun: (0.0, 12.0),
            band_rifle: (10.0, 45.0),
            band_sniper: (30.0, 80.0),
            keep_from_shotgun_m: 15.0,
            close_on_sniper_m: 30.0,
            band_slack_m: 3.0,
            band_step_m: 6.0,
            band_legs: true,
            spot_cooldown_ms: 3_000,
            spot_again_ms: 15_000,
            spot_within_deg: 8.0,
            spot_chance: 1.0,
            grenades: 2,
            grenade_roll_ms: 4_000,
            grenade_chance: 0.15,
            grenade_m: (8.0, 30.0),
            grenade_hold_ms: (500, 800),
            grenade_within_deg: 6.0,
            grenade_friend_m: 6.0,
            grenade_give_up_ms: 1_500,
            grenade_push_ms: (1_500, 3_500),
            held_speed: 0.3,
            held_memory_ms: 5_000,
            camp_ms: 8_000,
            camp_m: 3.0,
            trade_ms: 3_000,
            trade_m: 40.0,
            reload_idle_ms: 1_000,
            reload_some_share: 0.3,
            press_ms: 100,
            press_gap_ms: 50,
            recoil_pull: 1.0,
            ads_near_m: 10.0,
            ads_near_chance: 0.85,
            strafe_run_ms: (250, 700),
            strafe_still_chance: 0.5,
            strafe_still_ms: (150, 450),
            sniper_crouch_chance: 0.3,
            switch_share: 0.5,
            switch_near_m: 15.0,
            switch_margin_m: 5.0,
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

/// Play 11: the distance band to an enemy, metres, by the bot's weapon
/// (`own`) and his (`enemy`): its own weapon's band, then an enemy shotgun
/// kept at `keep_from_shotgun_m` and an enemy sniper closed on to
/// `close_on_sniper_m`, unless that leaves less than 5 m of band (its own
/// weapon wins). No band without its own weapon's kind.
pub fn preferred_band(
    own: Option<WeaponKind>,
    enemy: Option<WeaponKind>,
    tuning: &CombatTuning,
) -> Option<(f32, f32)> {
    let band = tuning.band(own?);
    let adjusted = match enemy {
        Some(WeaponKind::Shotgun) => (band.0.max(tuning.keep_from_shotgun_m), band.1),
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
    /// `look_at` is a glance at a heard shot (play 6, or a threat not
    /// made by a hit), not an aim: the caller keeps the play's look when
    /// the play uses something or carries (a glance there broke the use
    /// and slowed the carrier: the bench lost a third of its deliveries
    /// once play 6 ran again).
    pub glance: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Push {
    toward: Vec3,
    from: u32,
    until: u32,
    why: LegsWhy,
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

/// A grenade being thrown (play 20).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Throw {
    at: Vec3,
    started: u32,
    hold_ms: u32,
    pressed: Option<u32>,
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
    /// Outranged by the target (play 11's band); the next hide from it
    /// not before `next_outranged`.
    outranged: bool,
    next_outranged: u32,
    /// No shot at the target before this: noticed + the reaction delay.
    first_shot_at: u32,
    /// The life it counts for (`PlaceView::spawned`).
    life: Option<u32>,
    /// Its health at the last think.
    health: Option<f32>,
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
    spot_pending: Option<Place>,
    last_spot: Option<u32>,
    spotted: Vec<(Place, u32)>,
    throw: Option<Throw>,
    thrown: u8,
    next_grenade_roll: u32,
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
    /// The last tick and the time since the one before, ms (the fire's
    /// time).
    last_tick: Option<u32>,
    tick_ms: u32,
    /// An enemy last seen (a seen track) at.
    last_seen: u32,
    /// Empty with a target seen and no cover found: reload where it is.
    reload_here: bool,
    /// No shot before this (a reload or the switch to the pistol under
    /// way).
    busy_until: u32,
    /// The aim's slow wander about the target, radians (yaw, pitch).
    wander: (f32, f32),
    /// Drawn per target: it aims down the sights nearer than `ads_near_m`;
    /// it crouches to fire (a bolt-action weapon).
    ads_near: bool,
    crouch_fire: bool,
    /// The side step: the key held (0: still) and until when.
    strafe: u32,
    strafe_until: u32,
    /// The last way it stepped, to step the other way next.
    strafe_last: u32,
    /// [aimpoint] The target's parts that show, the view's line
    /// ([`Self::look`]).
    sight: super::aimpoint::Sight,
    /// [recoil] The kick of the weapon in hand, when its trigger fires,
    /// and how much the kick moved the view at this tick's weapon update.
    recoil: Recoil,
    cadence: Cadence,
    kick_step: f32,
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

/// A point `m` metres from `from` along the yaw `bearing`.
fn along(from: Vec3, bearing: f32, m: f32) -> Vec3 {
    let [fx, fz] = forward(bearing);
    Vec3::new(from.x + fx * m, from.y, from.z + fz * m)
}

impl Reflex {
    /// A reflex for a bot carrying `arms` (`None`: [`Arms::fallback`]).
    pub fn armed(arms: Option<&'static Arms>) -> Self {
        Self {
            arms,
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

    /// The rounds of weapon 1 its kit has left, in it and to load.
    pub fn left(&self) -> u32 {
        match &self.hands {
            Some(hands) => hands.primary.rounds + hands.primary.spare,
            None => self
                .carried
                .unwrap_or(self.arms.unwrap_or_else(Arms::fallback).primary.1),
        }
    }

    /// Nothing to fire: weapon 1 has no round in it nor to load, and no
    /// pistol with one (the kit's ammo lasts the match: dry until a spawn
    /// with another kit).
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
        if !memory.all_seen(now).is_empty() {
            self.last_seen = now;
        }
        if hit {
            // Shot at: the side step turns the other way at once.
            if self.target.is_some() {
                self.step_aside(now, c, rng);
            }
            self.break_sight(me, sit, memory, now, true, true, c, rng);
        }
        // The magazine out with a target seen: out of sight to reload, else
        // reload where it stands. (Out of ammo too, it takes the pistol
        // where it stands: [`Self::reload`].)
        let empty = self.hands().held().needs_reload();
        if empty && self.target.is_some() && !self.reload_here {
            self.break_sight(me, sit, memory, now, false, true, c, rng);
            self.reload_here = !matches!(self.cover, Cover::Hiding { .. });
        }
        self.cover_steps(me, sit, now, c, rng);
        self.matchup(me, sit, memory, now, c);
        // Outranged: out of his sight, and wait (no re-peek); a carrier
        // keeps its way home.
        if self.outranged
            && now >= self.next_outranged
            && me.carrying.is_none()
            && !matches!(self.cover, Cover::Hiding { .. })
        {
            self.next_outranged = now + c.outranged_again_ms;
            self.break_sight(me, sit, memory, now, false, false, c, rng);
        }
        self.grenade_roll(me, sit.view, now, c, rng);
    }

    /// [aimpoint] Before each [`Self::tick`]: which parts of the target
    /// show from the bot's eyes on `world`'s bullet mesh. A reflex never
    /// told this (no map) aims at the chest and holds no shot for the
    /// world.
    pub fn look(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        world: &std::sync::Arc<mapdata::World>,
        now: u32,
    ) {
        self.new_life(me);
        self.sight.update(me, self.target, view, world, now);
    }

    /// [recoil] At each tick, before [`Self::tick`]: the weapon's update to
    /// `now` (the client's comes before the tick's turn and shots). Returns
    /// the kick as the last tick left it, pitch units: the view the bot
    /// aims from is its base pitch and that; this tick's rise is taken
    /// back by [`Override::pull`].
    pub fn weapon_tick(&mut self, me: &PlaceView, now: u32) -> f32 {
        self.new_life(me);
        self.in_hand();
        let before = self.recoil.y();
        self.recoil.update(now);
        self.kick_step = self.recoil.y() - before;
        before
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
        let recoil = &mut self.recoil;
        let fire = keys & key::FIRE != 0;
        self.cadence
            .tick(now, fire, auto, interval_ms, |at| recoil.mean_shot(at));
        taken
    }

    /// The kick of the weapon in hand: another weapon taken (the pistol)
    /// starts with none (0x2E2C21).
    fn in_hand(&mut self) {
        let item = self.hands().held().gun.item;
        if self.recoil.kick().item != item {
            self.recoil = Recoil::new(kick_of(item));
            self.cadence = Cadence::default();
        }
    }

    /// A new life: the per-life counts start again; what it carries stays,
    /// and the rounds its kit has left.
    fn new_life(&mut self, me: &PlaceView) {
        if self.life != Some(me.spawned) {
            if self.life.is_some() {
                *self = Self::armed_carrying(self.arms, self.left());
            }
            self.life = Some(me.spawned);
        }
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
        let seen = memory.all_seen(now);
        let in_range = |t: &&SeenTrack| t.at().distance_xz(me.feet) <= aim.range_m;
        let d = |t: &SeenTrack| t.at().distance_xz(me.feet);
        // The present target, unless another seen enemy is much nearer.
        let keep = self
            .target
            .and_then(|target| seen.iter().filter(in_range).find(|t| t.who() == target))
            .filter(|kept| {
                let far = d(kept);
                !seen.iter().filter(in_range).any(|t| {
                    let near = d(t);
                    t.who() != kept.who()
                        && (near < far * c.switch_share
                            || (near < c.switch_near_m && near + c.switch_margin_m <= far))
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
        let bonus = view.map_or(0.0, |_| aim.combat.wounded_bonus_m);
        let score = |t: &SeenTrack| {
            let health = view
                .and_then(|v| v.place(t.who()))
                .map_or(1.0, |p| p.health.clamp(0.0, 1.0));
            t.at().distance_xz(me.feet) - bonus * (1.0 - health)
        };
        let choice = shooter.or(keep).or_else(|| {
            seen.iter()
                .filter(in_range)
                .min_by(|a, b| score(a).total_cmp(&score(b)))
        });
        match choice {
            Some(track) if Some(track.who()) != self.target => {
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
                self.crouch_fire = rng.unit() < c.sniper_crouch_chance;
                // Play 19: a new enemy is spotted once in the crosshair.
                let lately = self
                    .spotted
                    .iter()
                    .any(|(who, at)| *who == track.who() && *at + c.spot_again_ms > now);
                self.spot_pending = (!lately && rng.unit() < c.spot_chance).then_some(track.who());
            }
            Some(_) => {}
            None => {
                self.target = None;
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
            let known = match memory.seen(sound.from, now) {
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
            .and_then(|target| memory.seen(target, now))
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
                    && target_at.is_none_or(|at| d(t.at) < d(at) * c.glance_heard_share)
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
        for track in memory.all_seen(now) {
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
            let seen = memory.seen(sound.from, now).is_some() && d <= c.punish_m;
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
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        if matches!(self.cover, Cover::Hiding { .. })
            || self.push.is_some_and(|p| now < p.until)
            || now >= self.duel_until
        {
            return;
        }
        let Some(tactics) = sit.tactics else {
            return;
        };
        let still_wait =
            |rng: &mut Rng| rng.range(c.still_wait_ms.0 as f32, c.still_wait_ms.1 as f32) as u32;
        if let Some(threat) = self.threat.filter(|t| hit && t.hit && now < t.until)
            && let Some(toward) = threat.bearing
        {
            // A carrier keeps its way home.
            if me.carrying.is_some() || !c.hit_cover {
                return;
            }
            let spread = c.hit_bearing_error_deg.to_radians();
            let covered = |at: Vec3| {
                let eye = at + Vec3::new(0.0, EYE_STANDING, 0.0);
                [-spread, 0.0, spread].iter().all(|k| {
                    let [fx, fz] = forward(toward + k);
                    tactics
                        .world
                        .sight
                        .ray_cast(eye, Vec3::new(fx, 0.0, fz), c.hit_cover_m)
                        .is_some()
                })
            };
            let query = SpotQuery {
                side: sit.side,
                samples: 48,
                ..SpotQuery::ring(me.feet, c.hide_m.0, c.hide_m.1)
            };
            // Every floor sampled, nearest first (no rays in `spots`
            // without a sight test: the cover rays are these).
            let spot = tactics
                .spots(&query, now, query.samples, rng)
                .into_iter()
                .find(|spot| covered(spot.at));
            if let Some(spot) = spot {
                self.cover = Cover::Hiding {
                    at: spot.at,
                    enemy: threat.at,
                    started: now,
                    arrived: None,
                    wait_ms: still_wait(rng),
                    repeek: false,
                };
            }
            return;
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
                    .and_then(|target| memory.seen(target, now))
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
            return;
        };
        let query = SpotQuery {
            hidden_from: Some(enemy),
            side: sit.side,
            samples: 48,
            ..SpotQuery::ring(me.feet, c.hide_m.0, c.hide_m.1)
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
        }
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
        self.range_step = None;
        self.outranged = false;
        let Some(track) = self.target.and_then(|target| memory.seen(target, now)) else {
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
            return;
        }
        // A carrier cannot shoot: no step back from him.
        let carrier = c.band_hold_on_carrier
            && sit
                .view
                .place(track.who())
                .is_some_and(|p| p.carrying.is_some());
        let step = if d < band.0 - c.band_slack_m {
            (!carrier).then(|| step_toward(me.feet, track.at(), -c.band_step_m))
        } else if d > band.1 + c.band_slack_m {
            Some(step_toward(me.feet, track.at(), c.band_step_m))
        } else {
            None
        };
        self.range_step = step.filter(|at| {
            sit.tactics
                .is_some_and(|tactics| tactics.fine_goal(*at, now, sit.side))
        });
    }

    /// Play 20: now and then, with nothing seen, a grenade at a place an
    /// enemy was seen holding an angle or sensed staying at.
    fn grenade_roll(
        &mut self,
        me: &PlaceView,
        view: &MatchView,
        now: u32,
        c: &CombatTuning,
        rng: &mut Rng,
    ) {
        if self.throw.is_some()
            || self.target.is_some()
            || self.thrown >= c.grenades
            || now < self.next_grenade_roll
        {
            return;
        }
        let held = self.held.iter().map(|(_, at, _)| *at);
        let camps = self
            .camps
            .iter()
            .filter(|camp| camp.last >= camp.since + c.camp_ms)
            .map(|camp| camp.at);
        let friend_near = |at: Vec3| {
            view.team(me.team)
                .any(|p| p.place != me.place && p.feet.distance_xz(at) < c.grenade_friend_m)
        };
        let at = held
            .chain(camps)
            .filter(|at| {
                let d = at.distance_xz(me.feet);
                (c.grenade_m.0..=c.grenade_m.1).contains(&d) && !friend_near(*at)
            })
            .min_by(|a, b| a.distance_xz(me.feet).total_cmp(&b.distance_xz(me.feet)));
        let Some(at) = at else {
            return;
        };
        self.next_grenade_roll = now + c.grenade_roll_ms;
        if rng.unit() < c.grenade_chance {
            self.throw = Some(Throw {
                at,
                started: now,
                hold_ms: rng.range(c.grenade_hold_ms.0 as f32, c.grenade_hold_ms.1 as f32) as u32,
                pressed: None,
            });
        }
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
        let ms = load.reload();
        self.busy_until = now + ms;
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
    /// half its shot) and pressed again only after its cycle and
    /// `press_gap_ms`, one round a press ([`super::guns`] on why).
    fn trigger(&mut self, wants: bool, allowed: bool, now: u32, c: &CombatTuning) -> bool {
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
            self.next_press = now + gun.cycle_ms() + c.press_gap_ms;
            1
        };
        let load = self.hands().held_mut();
        load.rounds = load.rounds.saturating_sub(shots);
        true
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
    /// `fire_allowed`: the play's word (a carrier cannot shoot).
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
        let c = &aim.combat;
        self.new_life(me);
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
        let legs = self.legs(now);
        let settle_s = mix(aim.settle_s, skill).max(0.05);
        let floor = mix(aim.error_deg, skill).to_radians();
        // Play 20: a throw under way has the look and the hands.
        if let Some(mut throw) = self.throw {
            self.held_ms = 0;
            let d = throw.at.distance_xz(me.feet);
            let point = throw.at + Vec3::new(0.0, 1.0 + 0.12 * d, 0.0);
            let error = (floor * (rng.unit() - 0.5), floor * 0.5 * (rng.unit() - 0.5));
            let mut keys = 0;
            match throw.pressed {
                None if off_by(me, point, error) <= c.grenade_within_deg.to_radians() => {
                    throw.pressed = Some(now);
                    keys = key::GRENADE;
                    self.throw = Some(throw);
                }
                None if now >= throw.started + c.grenade_give_up_ms => self.throw = None,
                None => {}
                Some(pressed) if now < pressed + throw.hold_ms => keys = key::GRENADE,
                Some(_) => {
                    // Let go: thrown. Then in, after it goes off.
                    self.throw = None;
                    self.thrown += 1;
                    self.push = Some(Push {
                        toward: throw.at,
                        from: now + c.grenade_push_ms.0,
                        until: now + c.grenade_push_ms.1,
                        why: LegsWhy::AfterGrenade,
                    });
                }
            }
            return Override {
                look_at: Some(point),
                error,
                pull: 0.0,
                fire: false,
                turn_time_s: Some(settle_s * 0.5),
                keys,
                legs: None,
                band_m: self.band,
                strafe: 0,
                glance: false,
            };
        }
        let seen = self.target.and_then(|target| memory.seen(target, now));
        if seen.is_some() {
            self.last_seen = now;
        }
        let reload = self.reload(seen.is_some(), now, c);
        // A threat (a hit's shooter, a near shot from outside the view): a
        // moment's look, even from a target (`Self::threats`).
        if let Some(threat) = self.threat.filter(|t| now < t.until) {
            self.strafe = 0;
            self.strafe_until = 0;
            // No new shot; a one-round press under way is held to its end
            // (its round is counted).
            let fire = self.trigger(false, fire_allowed, now, c);
            return Override {
                look_at: Some(threat.at + Vec3::new(0.0, 1.4, 0.0)),
                fire,
                keys: reload,
                legs,
                band_m: self.band,
                glance: !threat.hit || threat.bearing.is_some(),
                ..Override::default()
            };
        }
        let Some(track) = seen else {
            self.target = None;
            self.held_ms = 0;
            self.strafe = 0;
            self.strafe_until = 0;
            // Play 21: the partner's death spot within `trade_m`, weapon
            // up, unless a shot was heard lately (play 6 first: the killer
            // is there, not at the body); else play 6.
            let shot = self.sound.is_some_and(|(_, until)| now < until);
            if let Some((at, _)) = self
                .trade
                .filter(|(at, until)| now < *until && !shot && at.distance_xz(me.feet) <= c.trade_m)
            {
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
        let point = body.map_or(track.at() + Vec3::new(0.0, 1.3, 0.0), PlaceView::chest);
        // [aimpoint] The part of him that shows (none: where he was last
        // seen); a shot only clear of the world (the last test below).
        let aimed = self.sight.aim(track.who(), body, point);
        let point = aimed.point;
        // The error settles from its start toward the skill's, never to 0,
        // and wanders slowly about him as a player's tracking does.
        let t = now.saturating_sub(self.since) as f32 / 1000.0;
        let fade = (-t / settle_s).exp();
        self.wander_step(aim, skill, rng);
        let gun = self.hands().held().gun;
        let error = (
            self.error.0 * fade + self.wander.0 + floor * (rng.unit() - 0.5),
            self.error.1 * fade + self.wander.1 + floor * 0.5 * (rng.unit() - 0.5),
        );
        // [recoil] The kick's rise of the view this tick, taken back by the
        // turn (any weapon); what is not taken back lifts the view and the
        // shot with it.
        let pull = -c.recoil_pull * self.kick_step;
        let lift = self.kick_step + pull;
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
        let within = off <= aim.fire_within_deg.to_radians();
        let mut wants = false;
        if fire_allowed
            && within
            && now >= self.first_shot_at
            && now >= self.busy_until
            && self.hands().held().rounds > 0
            && distance <= aim.range_m
            // The shot leaves along the view (kick and all): the wall test is
            // on the view now and the one it turns to, each with the kick's
            // lift not taken back.
            && self.clear_shot(me, &aimed, error, lift)
        {
            if !gun.auto || now < self.burst_until {
                wants = true;
            } else if now >= self.pause_until {
                self.burst_until =
                    now + rng.range(aim.burst_ms.0 as f32, aim.burst_ms.1 as f32) as u32;
                self.pause_until = self.burst_until
                    + rng.range(aim.pause_ms.0 as f32, aim.pause_ms.1 as f32) as u32;
                wants = true;
            }
        }
        let fire = self.trigger(wants, fire_allowed, now, c);
        // Down the sights at him, unless it runs for cover; with a bolt
        // action crouched (drawn per target), with anything else the side
        // step while it stands to aim.
        let running = legs.is_some_and(|l| l.run);
        if !running && (distance >= c.ads_near_m || self.ads_near) {
            keys |= key::AIM;
        }
        let mut strafe = 0;
        if !gun.auto && gun.chamber_ms > 0 {
            self.strafe = 0;
            if self.crouch_fire && !running {
                keys |= key::CROUCH;
            }
        } else if legs.is_none() {
            strafe = self.side_step(now, c, rng);
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
            glance: false,
        }
    }

    /// [aimpoint] [`super::aimpoint::Sight::clear_shot`] with the view (and
    /// the view it turns to) `lift` higher.
    fn clear_shot(
        &self,
        me: &PlaceView,
        aimed: &super::aimpoint::Aim,
        error: (f32, f32),
        lift: f32,
    ) -> bool {
        if lift == 0.0 {
            return self.sight.clear_shot(me, aimed, error);
        }
        let lifted = PlaceView {
            pitch: (me.pitch + lift).clamp(-1.0, 1.0),
            ..me.clone()
        };
        self.sight
            .clear_shot(&lifted, aimed, (error.0, error.1 + lift))
    }

    /// The aim's wander, one tick on: a mean-reverting walk whose spread is
    /// the skill's `wander_deg` (the pitch's half) and which comes back in
    /// about `wander_s`.
    fn wander_step(&mut self, aim: &AimTuning, skill: f32, rng: &mut Rng) {
        let spread = mix(aim.wander_deg, skill).max(0.0).to_radians();
        let a = (self.tick_ms as f32 / 1000.0 / aim.wander_s.max(0.01)).min(1.0);
        // Uniform shocks (a spread of 1/sqrt(12)) scaled to keep the walk's
        // spread at `spread`.
        let shock = spread * (2.0 * a - a * a).sqrt() * 12f32.sqrt();
        self.wander.0 += -self.wander.0 * a + shock * (rng.unit() - 0.5);
        self.wander.1 += -self.wander.1 * a + 0.5 * shock * (rng.unit() - 0.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns;
    use crate::v026g::bots::know::{DangerSpot, Percept};
    use crate::v026g::bots::view::{Mode, Sound};

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
        }
    }

    fn view_of(places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock: 0,
            mode: Mode::TeamDeathmatch,
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
        for skill in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let (mut firsts, mut errors) = (Vec::new(), Vec::new());
            for seed in 0..60 {
                let (first, e, _) = duel_numbers(&aim, skill, seed);
                firsts.extend(first.map(|f| f as f32));
                errors.extend(e);
            }
            println!(
                "skill {skill}: first shot {:.0} ms (n={}), yaw off while firing {:.2} deg (n={})",
                median(firsts.clone()),
                firsts.len(),
                median(errors.clone()),
                errors.len()
            );
        }
    }

    /// The duel against the top players' numbers (`TOP-PLAYERS.md`, 10
    /// recorded matches): sight to first shot 400 / 650 / 1050 ms
    /// (quartiles), the yaw off the chest while firing about 1.0° once the
    /// replay's own error is taken out (1.5° measured). The best bot is a
    /// top player's match, not better; a weaker one is slower and looser.
    #[test]
    fn the_duel_numbers_are_a_top_players_at_the_best_skill() {
        let aim = AimTuning::default();
        let numbers = |skill: f32| {
            let (mut firsts, mut errors) = (Vec::new(), Vec::new());
            for seed in 0..60 {
                let (first, e, _) = duel_numbers(&aim, skill, seed);
                firsts.extend(first.map(|f| f as f32));
                errors.extend(e);
            }
            (firsts.len(), median(firsts), median(errors))
        };
        let (n, first, error) = numbers(1.0);
        assert_eq!(n, 60, "every duel ends in a shot");
        assert!((400.0..=1_050.0).contains(&first), "{first}");
        assert!((0.6..=1.5).contains(&error), "{error}");
        let (_, slow, loose) = numbers(0.0);
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
        let mut reflex = Reflex::armed(Arms::of_role(role));
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
        // Top players: bursts 500 / 750 / 1100 ms, pauses 200 / 400 / 750 ms
        // (everyone else: 150 / 350 / 800 and 100 / 200 / 450).
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        // 24 s: the AK-74U's 120 rounds, four magazines (then the pistol).
        let ticks = facing_fight("support", 20.0, &aim, 480);
        let bursts = spans(&ticks, |o| o.fire);
        assert!(bursts.len() >= 8, "{bursts:?}");
        // A burst cut by the magazine running out is shorter (one a
        // magazine); the rest lie in the tuning's range (a tick either side).
        let whole: Vec<u32> = bursts
            .iter()
            .map(|b| b.1)
            .filter(|&ms| ms + 50 >= aim.burst_ms.0)
            .collect();
        assert!(whole.len() + 4 >= bursts.len(), "{bursts:?}");
        for ms in &whole {
            assert!(
                (aim.burst_ms.0 - 50..=aim.burst_ms.1 + 50).contains(ms),
                "{bursts:?}"
            );
        }
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
    fn a_bolt_action_is_pressed_once_a_cycle_with_no_burst_pause() {
        // Players press a one-round weapon again as soon as it takes it
        // (SKS 200 ms apart at its 200 ms cycle, the TT-33 250 at 200): the
        // bot presses at the cycle and `press_gap_ms`, whatever the bursts.
        let aim = AimTuning {
            fire_within_deg: 180.0,
            ..AimTuning::default()
        };
        let mosin = guns::gun("mosin_nagant").unwrap();
        let ticks = facing_fight("camper", 40.0, &aim, 300);
        let presses: Vec<u32> = spans(&ticks, |o| o.fire).iter().map(|r| r.0).collect();
        assert!(presses.len() >= 4, "{presses:?}");
        let cycle = mosin.cycle_ms() + aim.combat.press_gap_ms;
        for w in presses.windows(2) {
            let gap = w[1] - w[0];
            // The next tick after the cycle, or a reload between.
            assert!(gap >= cycle, "{presses:?}");
            assert!(gap <= cycle + 50 || gap >= cycle + 1_000, "{presses:?}");
        }
    }

    /// A bot of `role` facing an enemy `distance` ahead from 1 s for
    /// `ticks` ticks at skill 1, standing, its view turned by the motor and
    /// kicked as the client kicks it ([`super::super::recoil`]: the view is
    /// the base pitch and the kick, a downward turn taken off the kick
    /// first). The client's kick here is the bot's own model with the
    /// ranges' middles. Returns, for each tick it fired, how far the view
    /// was over the chest, degrees; and the shots.
    fn kicked_fight(role: &str, distance: f32, aim: &AimTuning, ticks: u32) -> (Vec<f32>, u32) {
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
        let mut reflex = Reflex::armed(Arms::of_role(role));
        // The client's side: the base pitch and the weapon's kick.
        let mut base = me.pitch;
        let mut client = Recoil::new(kick_of(reflex.hands().held().gun.item));
        let mut cadence = Cadence::default();
        let (mut over, mut shots) = (Vec::new(), 0);
        for k in 0..ticks {
            let now = 1_000 + k * 50;
            sense(&mut memory, &[percept(1, at, now, Source::Seen)], now);
            if k % 5 == 0 {
                reflex.think_with(&me, &Situation::of(&view), &memory, now, aim, 1.0, &mut rng);
            }
            // The bot's own reckoning: its base pitch and its kick.
            let kick = reflex.weapon_tick(&me, now);
            let seen = PlaceView {
                pitch: base + kick,
                ..me.clone()
            };
            let o = reflex.tick(&seen, &view, &memory, now, true, aim, 1.0, &mut rng);
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
            };
            let turn = motor.aim_still(&pose, (me.yaw, want)).turn.1 + o.pull;
            let keys = o.keys | if o.fire { key::FIRE } else { 0 };
            // The bot's books and the client's, each the same rule; the
            // client's kick is reset by a reload, and starts afresh with the
            // pistol.
            let taken = reflex.sent(now, keys, turn);
            let item = reflex.held_gun().item;
            if item != client.kick().item {
                client = Recoil::new(kick_of(item));
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
                over.push((client.view(base) - chest).to_degrees());
            }
        }
        (over, shots)
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
                ..CombatTuning::default()
            },
            ..aim.clone()
        };
        let (over, _) = kicked_fight("support", 20.0, &lifted, 200);
        let highest = over.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(highest > 0.8, "{highest}");
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
        // Near, down the sights only with the per-target draw.
        let never = AimTuning {
            combat: CombatTuning {
                ads_near_chance: 0.0,
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
        let aim = AimTuning::default();
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
                spots.push((now, target));
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
        let eye = |p: Vec3| p + Vec3::new(0.0, 1.6, 0.0);
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
        let mut memory = Memory::default();
        // The role's guns with the test's ammo: a magazine and a little
        // more (the kits carry a life's worth, too long a duel to test).
        let arms = Arms::of_role(role).map(|arms| {
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
            now += 50;
        }
        out
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
        let [uzi, .., tt] = &guns::GUNS;
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
        let aim = AimTuning {
            fire_within_deg: 180.0,
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
        let reload_ms = 1_667 + 5 * 500 + 1_367;
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
        // Its own weapon wins when the enemy's leaves no band.
        assert_eq!(
            preferred_band(Some(WeaponKind::Shotgun), Some(WeaponKind::Shotgun), &c),
            Some(c.band_shotgun)
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
            let eye = legs.at + Vec3::new(0.0, EYE_STANDING, 0.0);
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
        let start = Vec3::new(34.0, 0.0, 30.0);
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
                let eye = |p: Vec3| p + Vec3::new(0.0, 1.6, 0.0);
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
    /// before.
    #[test]
    fn a_sniper_holds_its_ground_at_a_battery_carrier() {
        let tactics = Tactics::new(crate::v026g::bots::tactics::tests::field(), 1.0);
        let aim = AimTuning::default();
        for (carrying, steps) in [(true, false), (false, true)] {
            let me = body(0, 0, Vec3::new(20.0, 0.0, 32.0), 0.0);
            let enemy_at = Vec3::new(30.0, 0.0, 32.0);
            let mut enemy = body(1, 1, enemy_at, 0.0);
            enemy.carrying = carrying.then_some(0);
            let view = view_of(vec![me.clone(), enemy]);
            let weapons = [Some(WeaponKind::Sniper), Some(WeaponKind::Rifle)];
            let sit = Situation {
                tactics: Some(&tactics),
                side: Some(0),
                weapon: Some(WeaponKind::Sniper),
                weapons: &weapons,
                ..Situation::of(&view)
            };
            let mut memory = Memory::default();
            let mut reflex = Reflex::default();
            let mut rng = Rng::new(64);
            for now in [1_000, 1_300] {
                sense(&mut memory, &[percept(1, enemy_at, now, Source::Seen)], now);
                reflex.think_with(&me, &sit, &memory, now, &aim, 0.5, &mut rng);
            }
            let o = reflex.tick(&me, &view, &memory, 1_350, true, &aim, 0.5, &mut rng);
            assert_eq!(
                o.legs.is_some_and(|l| l.why == LegsWhy::Range),
                steps,
                "carrying {carrying}: {o:?}"
            );
        }
    }
}
