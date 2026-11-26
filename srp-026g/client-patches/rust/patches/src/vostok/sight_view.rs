//! Patch 161's view of patch 206's second sights, the game's side
//! (`docs/notes/second-sights.md`, "The view (0.26g)"): a first-person gun's
//! eye record at `hands_ik.aim_alignment`, the world camera's zoom and near
//! plane at `sights.zoom`, the scope view byte at `sights.scope_view`, the
//! weapon's own projection at `weapon_view.camera`, and the reticle's draw
//! (vostok::attachments::draw's choice); the second sights' factors, the
//! dictionary's `srp_sight_views` ([`load_views`]).
//!
//! Every value is read here once, typed; `attachments::sights` decides.

use second_sight::{GameTime, View};
use weapon_modules::{DictId, Placement};

use super::attachments::{self, Kept, WeaponId};
use super::config::{ConfigError, ConfigValue};
use super::context::Context;
use super::log::{Piece, line};
use super::world::{read, write_floats};
use super::{Lock, Thread};
use crate::attachments::sights::Factors;
use crate::raw;

/// A weapon's two clocks at a frame: its user's simulation time (the last
/// tick's, +324h) and the game's frame time (game +5854h, ms: the last
/// frame's, game::tick 0x1ECC47).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clocks {
    pub simulation: GameTime,
    pub frame: u32,
}

/// A client weapon at one of the sites, with its user (a player's weapon
/// slot's, or none).
pub struct SightedGun<'c> {
    weapon: raw::Weapon,
    thread: &'c Thread,
}

impl<'c> SightedGun<'c> {
    /// `hands_ik.aim_alignment`: the first-person gun about to be aligned to
    /// its eye (patch 172's site; this hook runs before its).
    pub fn at_eye(ctx: &'c Context<'_>) -> Option<SightedGun<'c>> {
        // SAFETY: process_hands_ik's frame holds the user's held weapon
        // (kept at 0x295A09, tested at 0x295A0F), alive while mt_draw's bone
        // pass runs on this thread.
        let weapon = unsafe { raw::site::hands_ik_aim_alignment::frame(ctx)?.weapon()? };
        Some(SightedGun { weapon, thread: ctx.thread() })
    }

    /// `sights.scope_view`: the local player's weapon whose scope view the
    /// tick decides.
    pub fn at_scope_view(ctx: &'c Context<'_>) -> Option<SightedGun<'c>> {
        // SAFETY: the site's ESI, the client weapon its tick runs for.
        let weapon = unsafe { raw::site::sights_scope_view::weapon(ctx)? };
        Some(SightedGun { weapon, thread: ctx.thread() })
    }

    /// `weapon_view.camera`: the gun update_camera holds in first person.
    pub fn at_view_camera(ctx: &'c Context<'_>) -> Option<SightedGun<'c>> {
        // SAFETY: the site's EAX, the held item the cast (vtable+70h,
        // 0x1F34C5) took for a client weapon, read as one around the site.
        let weapon = unsafe { raw::site::weapon_view_camera::weapon(ctx)? };
        Some(SightedGun { weapon, thread: ctx.thread() })
    }

    /// `attachments.draw`: the weapon weapon::draw draws.
    pub fn at_draw(ctx: &'c Context<'_>) -> Option<SightedGun<'c>> {
        // SAFETY: the site's EBX, the weapon draw draws, alive for the draw.
        let weapon = unsafe { raw::site::attachments_draw::weapon(ctx)? };
        Some(SightedGun { weapon, thread: ctx.thread() })
    }

    pub fn id(&self) -> WeaponId {
        WeaponId::of(self.weapon)
    }

    /// What patch 161 keeps of it: the gun and what is fitted.
    pub fn kept(&self) -> Option<Kept> {
        attachments::kept(self.thread, self.id())
    }

    /// Its user's two clocks; None with no user.
    pub fn clocks(&self) -> Option<Clocks> {
        // SAFETY: the live weapon's user (0 once deactivated) and its time;
        // its world (+180Ch) and that world's game, alive with the weapon.
        unsafe {
            let simulation = GameTime(read(self.weapon.user()?.time()));
            let frame = read(self.weapon.world()?.game()?.time());
            Some(Clocks { simulation, frame })
        }
    }

    /// Its sights at `now`: None for a weapon of one sight or in no weapon
    /// slot of its user (patch 206's table).
    pub fn view_at(&self, now: GameTime) -> Option<View> {
        // SAFETY: a live weapon of the site's, on its thread.
        unsafe { super::second_sight::sight_at(self.thread, self.weapon, now) }
    }

    /// Its sights at its last tick's time (+5A4h, set at the start of
    /// weapon_core::tick, which patch 206's `sights.tick` ends a switch in):
    /// what the simulation has at that tick.
    pub fn view_at_tick(&self) -> Option<View> {
        // SAFETY: the live weapon's own field.
        let now = GameTime(unsafe { read(self.weapon.last_tick()) });
        self.view_at(now)
    }

    /// Its aim's progress (+5FCh), 0 at the hip to 1 aimed.
    pub fn progress(&self) -> f32 {
        // SAFETY: the live weapon's own float.
        unsafe { read(self.weapon.aim_progress()) }
    }

    /// Its optic's change_scope_factor (+1808h, +110h): the aim's progress
    /// from which the tick turns the scope view on; None without an optic.
    pub fn change_scope_factor(&self) -> Option<f32> {
        // SAFETY: the live weapon's optic (its own, alive with it) and float.
        unsafe { self.weapon.optics().map(|optic| read(optic.change_scope_factor())) }
    }

    /// The eye: calculate_aim_alignment, called after the site, moves the
    /// gun until this record's frame is on the camera. `at` is a placement
    /// of the gun patch 161 keeps this weapon for ([`Self::kept`]), whose
    /// bones its model has (checked when the records were written at load).
    pub fn set_eye(&mut self, at: &Placement) {
        // SAFETY: the aim record +7A8h, inside the live weapon, read by
        // calculate_aim_alignment alone, on this thread after the site; its
        // bone one of the model's (the kept gun's).
        unsafe {
            let record = self.weapon.aim_locator();
            write_floats(record.offset(), at.offset.matrix());
            record.bone().write(at.bone.0);
        }
    }

    /// The scope view (+181Eh) off: weapon::draw draws the optic's idle
    /// model, the body and the hands (0x244F00, 0x244F8B, 0x244FBF). The
    /// stock rule writes it at this site's tick, the local player's alone.
    pub fn clear_scope_view(&mut self) {
        // SAFETY: the live weapon's own byte, which the tick writes here.
        unsafe { self.weapon.scope_view().write(0) }
    }
}

/// The camera update_camera zooms for a gun in first person (`sights.zoom`).
pub struct GunZoom<'c> {
    gun: SightedGun<'c>,
    camera: raw::PlayerInputHandler,
}

impl<'c> GunZoom<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<GunZoom<'c>> {
        // SAFETY: the site's EBX, update_camera's player (0x1F348A), its
        // camera handler (+F470h, tested at 0x1F3495) and its held item,
        // which the cast at 0x1F34C5 took for a client weapon on this path
        // alone (0x1F34D2..0x1F3566 read it as one).
        unsafe {
            let player = raw::site::sights_zoom::player(ctx)?;
            let camera = player.camera_handler()?;
            let weapon = player.held_item()?.weapon();
            Some(GunZoom { gun: SightedGun { weapon, thread: ctx.thread() }, camera })
        }
    }

    pub fn gun(&self) -> &SightedGun<'c> {
        &self.gun
    }

    /// The factors update_camera took: its optic's (+1808h: fov_factor
    /// +114h, near_plane_factor +118h), else the weapon's own (+5B4h,
    /// +5B8h).
    pub fn main_factors(&self) -> Factors {
        // SAFETY: the live weapon's optic (its own, alive with it) and floats.
        unsafe {
            match self.gun.weapon.optics() {
                Some(optic) => Factors::new(read(optic.fov_factor()), read(optic.near_plane_factor())),
                None => Factors::new(read(self.gun.weapon.own_fov_factor()), read(self.gun.weapon.own_near_plane_factor())),
            }
        }
    }

    /// The weapon's own factors (with no optic).
    pub fn own_factors(&self) -> Factors {
        // SAFETY: as main_factors.
        unsafe { Factors::new(read(self.gun.weapon.own_fov_factor()), read(self.gun.weapon.own_near_plane_factor())) }
    }

    /// The camera's zoom (+94h) and its near plane (+8Ch) as the game's
    /// unit 0.05 times `near` (update_camera's product, 0x1F355E).
    pub fn set(&mut self, zoom: f32, near: f32) {
        // SAFETY: the live camera handler's two floats, which update_camera
        // wrote just before the site and the view reads after it; the
        // image's constant, read only.
        unsafe {
            let unit = read(raw::image::near_plane_unit().value());
            self.camera.fov_factor().write(zoom);
            self.camera.near_plane().write(near * unit);
        }
    }
}

/// Second sights' rows of the dictionary at most (two ship).
const ROWS: usize = 16;

/// The dictionary's `srp_sight_views`: an optic's factors as a second sight,
/// by its dict id.
struct Views {
    rows: [Option<(DictId, Factors)>; ROWS],
}

impl Views {
    const NONE: Views = Views { rows: [None; ROWS] };

    fn get(&self, sight: DictId) -> Option<Factors> {
        self.rows.iter().flatten().find(|(id, _)| *id == sight).map(|(_, factors)| *factors)
    }
}

static VIEWS: Lock<Views> = Lock::new(Views::NONE);

/// Why a row of `srp_sight_views` is left out.
#[derive(Clone, Copy)]
enum BadRow {
    /// No table, or `sight` no dict id, or `fov` no float2.
    Shape,
    /// A factor outside (0, 1].
    Factor,
    /// An optic an earlier row gives.
    Twice,
    /// More rows than there is room for.
    Full,
}

fn row(value: Result<ConfigValue<'_>, ConfigError>) -> Result<(DictId, Factors), BadRow> {
    let row = value.and_then(ConfigValue::table).map_err(|_| BadRow::Shape)?;
    let sight = row.required(c"sight").and_then(ConfigValue::integer).map_err(|_| BadRow::Shape)?;
    let sight = u16::try_from(sight).ok().and_then(DictId::new).ok_or(BadRow::Shape)?;
    let Ok(ConfigValue::Pair([fov, near])) = row.required(c"fov") else { return Err(BadRow::Shape) };
    Ok((sight, Factors::parse(fov, near).ok_or(BadRow::Factor)?))
}

fn refused(index: usize, why: BadRow) {
    let text = match why {
        BadRow::Shape => "sight views: a row is not {sight, fov}, row ",
        BadRow::Factor => "sight views: a row's factor is outside (0, 1], row ",
        BadRow::Twice => "sight views: a row's optic has a row before it, row ",
        BadRow::Full => "sight views: no room for the row ",
    };
    line(&[Piece::Text(text), Piece::Dec(index as u32)]);
}

/// The rows of the dictionary at `attachments.dictionary_configs`; None for
/// no dictionary or one without them.
fn views(ctx: &Context<'_>) -> Option<Views> {
    // SAFETY: the site's ESP, on_configs_loaded's entry: [esp+4] its
    // queries_result, alive while the callback runs on this thread; the
    // results' count, which the callback reads too (0x2C8AF5); the first,
    // the static dictionary, a binary config (class 1Fh) whose root its
    // loader relocated; the views of it do not outlive this call.
    let root = unsafe {
        let results = raw::site::attachments_dictionary_configs::frame(ctx)?.results()?;
        if read(results.count()) == 0 {
            return None;
        }
        let root = results.results().binary_config()?.root()?;
        ConfigValue::read(root).and_then(ConfigValue::table).ok()?
    };
    let Ok(Some(views)) = root.get(c"srp_sight_views") else { return None };
    let Ok(entries) = views.table().map(|views| views.entries()) else {
        line(&[Piece::Text("sight views: srp_sight_views is not a table of rows")]);
        return None;
    };
    let mut out = Views::NONE;
    for (index, value) in entries.iter().enumerate() {
        let made = row(value).and_then(|(sight, factors)| match out.get(sight) {
            Some(_) => Err(BadRow::Twice),
            None => Ok((sight, factors)),
        });
        let placed = made.and_then(|made| {
            let free = out.rows.iter_mut().find(|row| row.is_none()).ok_or(BadRow::Full)?;
            *free = Some(made);
            Ok(())
        });
        if let Err(why) = placed {
            refused(index, why);
        }
    }
    Some(out)
}

/// `attachments.dictionary_configs`: the views of the dictionary being made,
/// in place of the last one's (a game without them: none).
pub fn load_views(ctx: &Context<'_>) {
    let views = views(ctx).unwrap_or(Views::NONE);
    VIEWS.with(ctx.thread(), |kept| *kept = views);
}

/// The factors of the optic `sight` as a second sight, when the dictionary
/// gives them.
pub fn factors(thread: &Thread, sight: DictId) -> Option<Factors> {
    VIEWS.with(thread, |views| views.get(sight))
}
