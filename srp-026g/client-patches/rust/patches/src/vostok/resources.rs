//! The resource manager's queries a patch asks, and their results. A query
//! is asynchronous: `query_resources` (0xE5B60) copies the request (its
//! path into the query's own storage), the callback (a boost function made
//! by 0x176D0, bound to a word: the query's [`QueryTag`]) and the user data
//! at once, loads on its threads and calls the callback on the thread that
//! asked (the game's, for the game thread's hooks) with the results. The
//! callback is a generated entry ([`CallbackEntry`], the `callback` site
//! `resources.query_done`); its handler gets [`QueryResults`] and the tag,
//! and takes what it asked for with [`QueryResults::arrived`]. The music
//! (patch 88) asks its bus and tracks, Forest Protection's glow (patch 111)
//! its game effect.

use core::ffi::CStr;
use core::num::NonZeroU32;
use core::ptr::null;
use core::sync::atomic::{AtomicU32, Ordering};

use super::effect::KeptEffect;
use super::hook::{CallbackEntry, Thread};
use super::sound::{BusScene, Emitter};
use super::OrigStoreFirstResource;
use crate::raw;

/// The resource classes these queries ask (resources::class_id_enum).
const SOUND_SCENE_CLASS: u32 = 38;
const SINGLE_SOUND_CLASS: u32 = 256;
/// A game effect resource (game_effect_resource_cook, its constructor
/// 0x2EC6EB): a request's path is the effect's name, its config
/// gameplay/effects/effect_<name>.options (translate_query 0x2EC86C).
const GAME_EFFECT_CLASS: u32 = 109;
/// query_resources' last argument: a normal asynchronous query.
const ASYNCHRONOUS: u32 = 1;
/// A sound scene's creation params: the music bus holds one voice at a time,
/// and the one being let go (its destruction is the sound thread's).
const BUS_PROXIES: u32 = 8;
const BUS_PROPAGATORS: u32 = 16;
const BUS_RECEIVERS: u32 = 2;

/// The words of the tags that are not a track's.
const BUS_WORD: u32 = 0;
const EFFECT_WORD: u32 = 1;

/// A track query's number, never a word of another tag (0 and 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Serial(NonZeroU32);

impl Serial {
    pub const FIRST: Serial = match NonZeroU32::new(EFFECT_WORD + 1) {
        Some(n) => Serial(n),
        None => panic!("the first serial is not 0"),
    };

    /// The one after, wrapping past the largest to the first.
    pub fn next(self) -> Serial {
        self.0.checked_add(1).map_or(Serial::FIRST, Serial)
    }
}

/// What a query was asked for: the word bound to its callback, read back
/// when the callback comes (0 the music bus, 1 a game effect, else a
/// track's serial).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryTag {
    Bus,
    Effect,
    Track(Serial),
}

impl QueryTag {
    /// The callback's bound word, as the game hands it back.
    pub(super) fn from_raw(word: u32) -> QueryTag {
        match word {
            BUS_WORD => QueryTag::Bus,
            EFFECT_WORD => QueryTag::Effect,
            n => NonZeroU32::new(n).map_or(QueryTag::Bus, |n| QueryTag::Track(Serial(n))),
        }
    }

    fn word(self) -> u32 {
        match self {
            QueryTag::Bus => BUS_WORD,
            QueryTag::Effect => EFFECT_WORD,
            QueryTag::Track(serial) => serial.0.get(),
        }
    }
}

/// What came back: the bus's scene, a track's single sound or game effect
/// resources, each with a reference of its own (None for a query that
/// failed: a missing file).
pub enum Arrived {
    Bus(Option<BusScene>),
    Track(Serial, Option<Emitter>),
    /// Kept for the process (a game effect is played until the game exits).
    Effects(ArrivedEffects),
}

/// The most game effects one query asks for.
pub const EFFECTS_MOST: usize = 16;

/// The game effect resources a query asked for, in its order, each kept for
/// the process (None: that one failed, or past the query's requests).
#[derive(Clone, Copy)]
pub struct ArrivedEffects {
    kept: [Option<KeptEffect>; EFFECTS_MOST],
}

impl ArrivedEffects {
    /// The first `N`, when every one of them came (a query of `N`).
    pub fn all<const N: usize>(&self) -> Option<[KeptEffect; N]> {
        let mut out = [None; N];
        for (slot, kept) in out.iter_mut().zip(self.kept) {
            *slot = kept;
        }
        out.iter().all(Option::is_some).then(|| out.map(|kept| kept.expect("every one checked")))
    }
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

    /// The game's results, for the callbacks' own readers (vostok's).
    pub(super) fn raw(&self) -> Option<raw::QueriesResult> {
        self.results
    }

    /// The thread the callback runs on (the one that asked).
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Takes what the query `tag` names asked for: a reference of ours to
    /// its first result (a bus's or a track's query asks one resource), or
    /// to each of its results (a game effects' query).
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
                // SAFETY: a game effect resource for each class 109 request,
                // alive for the callback (KeptEffect::keep takes its own);
                // the results are 2E0h apart, as many as the requests.
                QueryTag::Effect => {
                    let mut kept = [None; EFFECTS_MOST];
                    if let Some(results) = first {
                        let count = (*results.count() as usize).min(EFFECTS_MOST);
                        for (index, slot) in kept.iter_mut().enumerate().take(count) {
                            *slot = results.results().at(index).effect().map(|effect| KeptEffect::keep(effect));
                        }
                    }
                    Arrived::Effects(ArrivedEffects { kept })
                }
            }
        }
    }
}

/// The callback queries hand the game: the patch's generated entry.
pub type QueryCallback = CallbackEntry<OrigStoreFirstResource>;

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
    query(c"sound_scene", SOUND_SCENE_CLASS, Some(params), QueryTag::Bus, callback);
}

/// Asks for a track: the single sound (class 256, as the scenes' sounds)
/// at `path` (`music/lobby_theme`: sounds/single/music/lobby_theme.*).
pub fn query_track(_: &Thread, path: &CStr, serial: Serial, callback: QueryCallback) {
    query(path, SINGLE_SOUND_CLASS, None, QueryTag::Track(serial), callback);
}

/// Asks for the game effects `names` in one query (class 109 each:
/// gameplay/effects/effect_<name>); they arrive together, in this order.
pub fn query_effects<const N: usize>(_: &Thread, names: &[&CStr; N], callback: QueryCallback) {
    const { assert!(N >= 1 && N <= EFFECTS_MOST) };
    let mut requests = [const { raw::ResourceRequestStorage::zeroed() }; N];
    let mut function = raw::QueryFunctionStorage::zeroed();
    // SAFETY: N contiguous requests {path, class} (8 bytes each, as the
    // native cooks' arrays: the artefact's three, 0x25E91A) and no user data
    // (the artefact cook passes none either), a boost function bound to the
    // generated callback and the effects' tag, and the game's allocator, all
    // copied by query_resources before it returns; no parent query.
    unsafe {
        for (request, name) in requests.iter_mut().zip(names) {
            let r = request.handle();
            r.set_path(name.as_ptr().cast_mut().cast());
            *r.class() = GAME_EFFECT_CLASS;
        }
        let f = function.handle();
        raw::engine_bind_query_callback(Some(f), callback.entry(), QueryTag::Effect.word());
        let allocator = raw::image::game_allocator().allocator();
        raw::engine_query_one(Some(requests[0].handle()), N as u32, Some(f), allocator, None, null(), ASYNCHRONOUS);
    }
}

fn query(path: &CStr, class: u32, params: Option<raw::SceneParamsVariant>, tag: QueryTag, callback: QueryCallback) {
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
        raw::engine_bind_query_callback(Some(f), callback.entry(), tag.word());
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
    fn serials_skip_the_other_words() {
        let last = Serial(NonZeroU32::MAX);
        assert_eq!(last.next(), Serial::FIRST);
        assert_eq!(QueryTag::from_raw(QueryTag::Track(Serial::FIRST).word()), QueryTag::Track(Serial::FIRST));
        assert_eq!(QueryTag::from_raw(QueryTag::Effect.word()), QueryTag::Effect);
        assert_eq!(QueryTag::from_raw(QueryTag::Bus.word()), QueryTag::Bus);
    }
}
