//! The resource manager's queries a patch asks, and their results. A query
//! is asynchronous: `query_resources` (0xE5B60) copies the request (its
//! path into the query's own storage), the callback (a boost function made
//! by 0x176D0, bound to a word: the query's [`QueryTag`]) and the user data
//! at once, loads on its threads and calls the callback on the thread that
//! asked (the game's, for the game thread's hooks) with the results. The
//! callback is a generated entry ([`CallbackEntry`], the `callback` site
//! `resources.query_done`); its handler gets [`QueryResults`] and the tag,
//! and takes what it asked for with [`QueryResults::arrived`].

use core::ffi::CStr;
use core::num::NonZeroU32;
use core::ptr::null;
use core::sync::atomic::{AtomicU32, Ordering};

use super::hook::{CallbackEntry, Thread};
use super::sound::{BusScene, Emitter};
use super::config::{ConfigError, ConfigValue};
use super::{OrigStoreFirstConfig, OrigStoreFirstResource};
use crate::raw;

/// The resource classes these queries ask (resources::class_id_enum).
const SOUND_SCENE_CLASS: u32 = 38;
const SINGLE_SOUND_CLASS: u32 = 256;
const BINARY_CONFIG_CLASS: u32 = 0x1F;
/// query_resources' last argument: a normal asynchronous query.
const ASYNCHRONOUS: u32 = 1;
/// A sound scene's creation params: the music bus holds one voice at a time,
/// and the one being let go (its destruction is the sound thread's).
const BUS_PROXIES: u32 = 8;
const BUS_PROPAGATORS: u32 = 16;
const BUS_RECEIVERS: u32 = 2;

/// A track query's number, never 0 (0 is the bus's tag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Serial(NonZeroU32);

impl Serial {
    pub const FIRST: Serial = Serial(NonZeroU32::MIN);

    /// The one after, wrapping past 0.
    pub fn next(self) -> Serial {
        Serial(self.0.checked_add(1).unwrap_or(NonZeroU32::MIN))
    }
}

/// What a query was asked for: the word bound to its callback, read back
/// when the callback comes (0 the music bus, else a track's serial).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryTag {
    Bus,
    Track(Serial),
}

impl QueryTag {
    /// The callback's bound word, as the game hands it back.
    pub(super) fn from_raw(word: u32) -> QueryTag {
        NonZeroU32::new(word).map_or(QueryTag::Bus, |n| QueryTag::Track(Serial(n)))
    }

    fn word(self) -> u32 {
        match self {
            QueryTag::Bus => 0,
            QueryTag::Track(serial) => serial.0.get(),
        }
    }
}

/// What came back: the bus's scene or a track's single sound, each with a
/// reference of its own (None for a query that failed: a missing file).
pub enum Arrived {
    Bus(Option<BusScene>),
    Track(Serial, Option<Emitter>),
}

/// The results a query's callback is called with, for its call only.
pub struct QueryResults<'t> {
    results: Option<raw::QueriesResult>,
    thread: &'t Thread,
}

impl<'t> QueryResults<'t> {
    /// # Safety
    /// `results` is the queries_result the game called the callback with,
    /// alive for the callback, on its thread.
    pub(super) unsafe fn from_raw(thread: &'t Thread, results: Option<raw::QueriesResult>) -> QueryResults<'t> {
        QueryResults { results, thread }
    }

    /// The thread the callback runs on (the one that asked).
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Takes what the query `tag` names asked for: a reference of ours to
    /// its first result (the queries made here ask one resource).
    pub fn arrived(&self, tag: QueryTag) -> Arrived {
        // SAFETY: the results are alive for the callback (from_raw); a
        // query of ours has one request, and the tag says which class it
        // asked, so the first result is that kind's: a sound scene for the
        // bus, a single sound (a sound emitter) for a track. A failed query
        // leaves it 0. The count is atomic (lock xadd), as the game's own
        // takers do (0x14758, 0x220B55).
        unsafe {
            let first = self.results.filter(|results| *results.count() >= 1);
            match tag {
                QueryTag::Bus => Arrived::Bus(first.and_then(|r| r.scene()).map(|scene| {
                    AtomicU32::from_ptr(scene.references()).fetch_add(1, Ordering::AcqRel);
                    BusScene::of(scene)
                })),
                QueryTag::Track(serial) => Arrived::Track(serial, first.and_then(|r| r.emitter()).map(|emitter| {
                    AtomicU32::from_ptr(emitter.references()).fetch_add(1, Ordering::AcqRel);
                    // SAFETY: the reference just taken is the Emitter's.
                    Emitter::owning(emitter)
                })),
            }
        }
    }
}

/// The callback queries hand the game: the patch's generated entry.
pub type QueryCallback = CallbackEntry<OrigStoreFirstResource>;

/// What a binary config's query was asked for: the word bound to its
/// callback, chosen by the patch that asked (srp_data: a table's index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigTag(pub u32);

impl ConfigTag {
    /// The callback's bound word, as the game hands it back.
    pub(super) fn from_raw(word: u32) -> ConfigTag {
        ConfigTag(word)
    }
}

/// A binary config query's result, for its callback only.
pub struct ConfigQuery<'t> {
    results: Option<raw::QueriesResult>,
    thread: &'t Thread,
}

impl<'t> ConfigQuery<'t> {
    /// # Safety
    /// `results` is the queries_result the game called the callback with,
    /// alive for the callback, on its thread.
    pub(super) unsafe fn from_raw(thread: &'t Thread, results: Option<raw::QueriesResult>) -> ConfigQuery<'t> {
        ConfigQuery { results, thread }
    }

    /// The thread the callback runs on (the one that asked).
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// The config's root, alive while the callback runs (the result holds
    /// it, and lets it go after): None when the query failed (a missing file).
    pub fn root(&self) -> Option<Result<ConfigValue<'t>, ConfigError>> {
        // SAFETY: the results are alive for the callback (from_raw); a config
        // query has one request, a binary config's (query_config), so the
        // first result is a binary config, 0 when it failed; its loader
        // relocated its records, as on_render_scenes_ready reads the lobby's
        // (0x22108B, 0x2210CE).
        unsafe {
            let config = self.results.filter(|results| *results.count() >= 1)?.config()?;
            Some(ConfigValue::read(config.root()?))
        }
    }
}

/// The callback a binary config's query hands the game: the patch's generated entry.
pub type ConfigCallback = CallbackEntry<OrigStoreFirstConfig>;

/// Asks for the binary config at `path` (`resources/gameplay/...`, as the
/// lobby asks default.player: lobby_menu::query_resources 0x21E5BE), `tag`
/// bound to its callback.
pub fn query_config(_: &Thread, path: &CStr, tag: ConfigTag, callback: ConfigCallback) {
    query(path, BINARY_CONFIG_CLASS, None, tag.0, callback);
}

/// Asks for the music bus: a sound scene of its own (class 38, as the
/// lobby's, made by sound_scene_cook from the creation params in the
/// request's user data).
pub fn query_bus(_: &Thread, callback: QueryCallback) {
    let mut variant = raw::SceneParamsVariantStorage::zeroed();
    let params = variant.handle();
    // SAFETY: our room for a variant<32> of sound_scene_creation_params, as
    // the lobby fills its own (lobby_menu::query_resources 0x21E4D5..0x21E559):
    // the helper, the three counts, its own address and the type's id; the
    // query copies it (its helper's copy) before it returns.
    unsafe {
        *params.helper() = raw::image::SCENE_PARAMS_HELPER;
        *params.proxies() = BUS_PROXIES;
        *params.propagators() = BUS_PROPAGATORS;
        *params.receivers() = BUS_RECEIVERS;
        params.set_itself(Some(params));
        *params.type_id() = raw::engine_scene_params_type();
    }
    query(c"sound_scene", SOUND_SCENE_CLASS, Some(params), QueryTag::Bus.word(), callback);
}

/// Asks for a track: the single sound (class 256, as the scenes' sounds)
/// at `path` (`music/lobby_theme`: sounds/single/music/lobby_theme.*).
pub fn query_track(_: &Thread, path: &CStr, serial: Serial, callback: QueryCallback) {
    query(path, SINGLE_SOUND_CLASS, None, QueryTag::Track(serial).word(), callback);
}

/// One request, `word` bound to the callback (`QueryTag`, `ConfigTag`).
fn query<F>(path: &CStr, class: u32, params: Option<raw::SceneParamsVariant>, word: u32, callback: CallbackEntry<F>) {
    let mut request = raw::ResourceRequestStorage::zeroed();
    let mut function = raw::QueryFunctionStorage::zeroed();
    let mut user = raw::QueryUserDataStorage::zeroed();
    // SAFETY: one request {path, class} and its user data (a variant or
    // none), a boost function bound to the generated callback and the tag,
    // and the game's allocator, all copied by query_resources before it
    // returns (the path into the query's own storage); no parent query.
    unsafe {
        let r = request.handle();
        r.set_path(path.as_ptr().cast_mut().cast());
        *r.class() = class;
        let f = function.handle();
        raw::engine_bind_query_callback(Some(f), callback.entry(), word);
        let u = user.handle();
        u.set_variant(params);
        let allocator = raw::image::game_allocator().allocator();
        raw::engine_query_one(Some(r), 1, Some(f), allocator, Some(u), null(), ASYNCHRONOUS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_read_back_as_bound() {
        assert_eq!(QueryTag::from_raw(QueryTag::Bus.word()), QueryTag::Bus);
        let serial = Serial::FIRST.next().next();
        assert_eq!(QueryTag::from_raw(QueryTag::Track(serial).word()), QueryTag::Track(serial));
    }

    #[test]
    fn serials_skip_zero() {
        let last = Serial(NonZeroU32::MAX);
        assert_eq!(last.next(), Serial::FIRST);
    }
}
