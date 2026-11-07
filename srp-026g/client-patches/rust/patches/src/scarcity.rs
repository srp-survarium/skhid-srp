//! Patch 119: Scarcity, a battery match with no ammo crates (the rules:
//! docs/notes/scarcity.md; client_patch.md section 3.72). Its starting ammo
//! is the server's: the kits of the static info are cut
//! (`survarium::v026g::scarcity`), so nothing here gives or counts rounds.
//! The hooks are `scarcity.toml`'s, each a `lohook`:
//!
//! - the match's options read (`static_info.options_read`, after the 0xE4's
//!   match_options::deserialize; `replay.options_read`, after a replay's
//!   chunk 1's): srp's block may say the variant Scarcity (tag 2, one byte
//!   2; `vostok::options::variant_in`), kept before the match's level is
//!   loaded (process_static_match_info starts that load after its read);
//! - the project's cook (`project.cook`, project_cooker_simple::
//!   create_game_objects at its entry, on whichever thread loads it): in a
//!   Scarcity match the crates are taken out of the project's client_project
//!   before anything is made of it: each `resupply_zone` with every client
//!   object of its folder (its use geometry, its decals), and every
//!   `item/quest/container_closed` model (its render visual and its
//!   collision objects: the crate's box, which no map uses for anything
//!   else). So no crate is drawn, collides, offers a use or refills; the
//!   lamps' metadata (patch 81: indexes into the render visuals) is
//!   renumbered to match.
//!
//! What a match computes: every game reads the same static info (a replay
//! its own chunk 1) and cooks the same project, so every game of a match
//! has the same world without crates.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::collection::BoundedVec;
use crate::vostok::config::{ConfigEdit, ConfigError, ConfigTable, Step, UpdateError};
use crate::vostok::{Context, Flow, Lock, MatchVariant, NoExit, OptionsRead, Thread};

/// The variant of the options last read (a match's or a replay's). A lock:
/// the options are read on the game's thread, a project may be cooked on a
/// loader's.
static VARIANT: Lock<MatchVariant> = Lock::new(MatchVariant::Plain);

/// A crate's box (render visuals and collision objects of this model).
const CRATE_MODEL: &[u8] = b"item/quest/container_closed";
/// The most entries a project's crates take out of one list.
const MOST: usize = 64;

type Taken = BoundedVec<usize, MOST>;

fn options_read(thread: &Thread, read: Option<OptionsRead>) -> Flow<NoExit> {
    let variant = read.map_or(MatchVariant::Plain, |read| read.variant());
    VARIANT.with(thread, |kept| *kept = variant);
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// project_cooker_simple::create_game_objects' entry (0x22C010).
pub fn on_project_cook(ctx: &mut Context) -> Flow<NoExit> {
    if VARIANT.with(ctx.thread(), |variant| *variant) != MatchVariant::Scarcity {
        return Flow::Continue;
    }
    if let Some(mut config) = ConfigEdit::at_project_cook(ctx) {
        // A project the game cannot read as its own lists is no map: refused
        // loudly rather than a match whose crates are half gone.
        remove_crates(&mut config).unwrap_or_else(|error| match error {
            CrateError::Config(e) => panic!("scarcity: a project list the game reads is not one: {e:?}"),
            CrateError::TooMany => panic!("scarcity: more than {MOST} crate entries in a project's list"),
            CrateError::LampOnCrate(i) => panic!("scarcity: a lamp names render visual {i}, a crate's"),
        });
    }
    Flow::Continue
}

#[derive(Debug)]
enum CrateError { Config(ConfigError), TooMany, LampOnCrate(u32) }
impl From<ConfigError> for CrateError { fn from(e: ConfigError) -> Self { Self::Config(e) } }

/// What the crates of a project are, read before anything moves.
struct Crates { objects: Taken, visuals: Taken, collisions: Taken }

fn remove_crates(config: &mut ConfigEdit<'_>) -> Result<(), CrateError> {
    let crates = {
        let root = config.root()?;
        Crates {
            objects: crate_objects(&root)?,
            visuals: models(&root, c"render_visuals")?,
            collisions: models(&root, c"collision_objects")?,
        }
    };
    config.remove(c"client_objects", |i| has(&crates.objects, i))?;
    config.remove(c"render_visuals", |i| has(&crates.visuals, i))?;
    config.remove(c"collision_objects", |i| has(&crates.collisions, i))?;
    let lamps = [Step::Key(c"srp_lamps"), Step::Key(c"lamps"), Step::Each, Step::Key(c"visuals"), Step::Each,
                 Step::Key(c"index")];
    config.update_integers(&lamps, &mut |index| renumbered(index, &crates.visuals)).map_err(|error| match error {
        UpdateError::Config(e) => CrateError::Config(e),
        UpdateError::Refused(e) => e,
    })
}

fn has(set: &Taken, index: usize) -> bool { set.iter().any(|&i| i == index) }

/// A render visual's index once the visuals `taken` are out.
fn renumbered(index: u32, taken: &Taken) -> Result<u32, CrateError> {
    if has(taken, index as usize) { return Err(CrateError::LampOnCrate(index)); }
    Ok(index - taken.iter().filter(|&&t| (t as u32) < index).count() as u32)
}

/// The folder of a `full_name` (up to its last `/`), none for a name alone.
fn folder(name: &[u8]) -> Option<&[u8]> {
    name.iter().rposition(|&b| b == b'/').map(|at| &name[..at])
}

fn text<'c>(entry: &ConfigTable<'c>, key: &CStr) -> Result<Option<&'c [u8]>, ConfigError> {
    entry.get(key)?.map(|value| value.text().map(CStr::to_bytes)).transpose()
}

/// The client objects of the crates: each `resupply_zone`, and every object
/// of a zone's folder.
fn crate_objects(root: &ConfigTable<'_>) -> Result<Taken, CrateError> {
    let Some(list) = root.get(c"client_objects")? else { return Ok(Taken::new()) };
    let entries = list.entries()?;
    let mut zones: BoundedVec<Option<&[u8]>, MOST> = BoundedVec::new();
    let mut taken = Taken::new();
    for (i, entry) in entries.iter().enumerate() {
        let entry = entry?.table()?;
        if text(&entry, c"game_object_type")? == Some(b"resupply_zone".as_slice()) {
            let name = text(&entry, c"full_name")?.unwrap_or_default();
            zones.push(folder(name)).map_err(|_| CrateError::TooMany)?;
            taken.push(i).map_err(|_| CrateError::TooMany)?;
        }
    }
    for (i, entry) in entries.iter().enumerate() {
        let entry = entry?.table()?;
        let Some(of) = text(&entry, c"full_name")?.and_then(folder) else { continue };
        if !has(&taken, i) && zones.iter().any(|zone| *zone == Some(of)) {
            taken.push(i).map_err(|_| CrateError::TooMany)?;
        }
    }
    Ok(taken)
}

/// The entries of `list` (render visuals, collision objects) of the crate's
/// model.
fn models(root: &ConfigTable<'_>, list: &CStr) -> Result<Taken, CrateError> {
    let Some(list) = root.get(list)? else { return Ok(Taken::new()) };
    let mut taken = Taken::new();
    for (i, entry) in list.entries()?.iter().enumerate() {
        let name = text(&entry?.table()?, c"lib_name")?.unwrap_or_default();
        if name.windows(CRATE_MODEL.len()).any(|w| w == CRATE_MODEL) {
            taken.push(i).map_err(|_| CrateError::TooMany)?;
        }
    }
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[usize]) -> Taken {
        let mut set = Taken::new();
        for &i in items { set.push(i).unwrap(); }
        set
    }

    #[test]
    fn a_lamp_after_a_crate_moves_down_by_the_crates_before_it() {
        let taken = set(&[3, 7]);
        assert_eq!(renumbered(0, &taken).unwrap(), 0);
        assert_eq!(renumbered(5, &taken).unwrap(), 4);
        assert_eq!(renumbered(9, &taken).unwrap(), 7);
        assert!(matches!(renumbered(7, &taken), Err(CrateError::LampOnCrate(7))));
    }

    #[test]
    fn a_folder_is_the_name_up_to_its_last_slash() {
        assert_eq!(folder(b"logic/resupply/team1/resupply_zone"), Some(b"logic/resupply/team1".as_slice()));
        assert_eq!(folder(b"zone"), None);
    }
}
