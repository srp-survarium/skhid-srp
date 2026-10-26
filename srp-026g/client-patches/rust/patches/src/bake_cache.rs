//! Patch 70: the camouflage/decal baker keeps its textures, and bakes or
//! loads a texture once while it is asked for again (client_patch.md
//! section 3.57; the rules: docs/notes/baked-textures.md).
//!
//! The baker (vostok::baker) bakes an item's camouflage and decals into a
//! texture of each type and saves it as user_data/streamed_textures/
//! <name>.dds; its list of saved textures lets a later query load the file
//! instead. Stock, the game deletes the whole folder when it starts and when
//! it exits, so every game bakes again every item it sees; and a texture is
//! listed only when its bake is done, so the player objects of one player
//! that ask for the same texture together bake it once each. Here:
//! - the game's start: the folder is kept while its stamp file says this
//!   revision; its files of a whole DDS go back into the list (CRC 0: the
//!   first load takes the CRC of what it reads, `on_check_crc`), at most
//!   SEED_FILES and SEED_BYTES of them, the rest deleted. Another revision,
//!   or no stamp: the stock delete, then the stamp (game::exit's delete is
//!   a byte write: patch_026g.py BAKE_CACHE_KEEP);
//! - a texture whose bake or load is under way (its owner, the first query
//!   of its path) is not baked or loaded again: a later query of the path
//!   waits, and gets the owner's texture when it is made (`on_baked`,
//!   `on_loaded`); if the owner ends otherwise, the waiting queries are made
//!   again (`on_finish`);
//! - a texture loaded from the folder is held for the game (KEPT,
//!   KEPT_BYTES, the least used let go), and a later query of its path (a
//!   respawn builds the player's model again) gets it at once;
//! - the CRC-32 step, which a bake and a load run over the whole file,
//!   eight bytes a step (the same value).
//!
//! Textures only: nothing a match computes. The look-up, the textures made
//! and finish_query run on the resource threads too: the tables are behind
//! a [`Lock`].

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU32, Ordering};

use crate::vostok::baker::{self, CheckCrc, Cook, Finish, Folder, List, Lookup, Made, Parameters, Query, Surface,
                           Texture, Waiter, RECORD_TEXT};
use crate::vostok::crc32::CrcStep;
use crate::vostok::files::{self, File, Find, FindData, Path};
use crate::vostok::{BakeCacheOnCrcStepExit, BakeCacheOnFinishExit, BakeCacheOnLookupExit, Context, Flow, Lock, NoExit,
                    Thread};

const PREFIX: &[u8] = b"user_data/streamed_textures/";
const STAMP_NAME: &[u8] = b"srp_baked_textures.txt";
const STAMP_TEXT: &[u8] = b"Skhid baked textures, revision ";
const DDS_FILES: &[u8] = b"*.dds";

/// Half the list: the game's own bakes have the rest.
const SEED_FILES: u32 = 512;
/// Half of what remove_unwanted_files keeps (1 GiB).
const SEED_BYTES: u32 = 512 << 20;
const OWNERS: usize = 64;
const WAITERS: usize = 192;
const PER_OWNER: usize = 16;
/// An owner this old is taken over by the next query of its path.
const STALE_MS: u32 = 10_000;
/// Textures held for the game, the least used let go first ...
const KEPT: usize = 256;
/// ... and at most this many bytes of their files.
const KEPT_BYTES: u32 = 256 << 20;
/// A texture's types: d, n, f, s.
const KINDS: u32 = 4;

/// A path buffer: the folder's path, a separator, a name.
const PATH: usize = 300;
/// The folder's path takes at most this much of it (with its separator).
const FOLDER: usize = PATH - 32;

// ---- strings ----------------------------------------------------------------

/// A C string's bytes in a buffer: up to its NUL.
fn text(buffer: &[u8]) -> &[u8] {
    &buffer[..buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len())]
}

/// `out` = a then b, NUL-ended; false (what fitted written, no NUL) when
/// that and its NUL do not fit in `out`.
fn join(out: &mut [u8], a: &[u8], b: &[u8]) -> bool {
    let mut n = 0;
    for &byte in a.iter().chain(b) {
        if n + 1 >= out.len() {
            return false;
        }
        out[n] = byte;
        n += 1;
    }
    out[n] = 0;
    true
}

/// The folder's path with a separator after it, NUL-ended, in `out`: its
/// length, 0 when it has none or does not fit.
fn folder(out: &mut [u8], path: Option<&[u8]>) -> usize {
    let Some(path) = path else { return 0 };
    let mut n = path.len();
    if n + 2 > out.len() {
        return 0;
    }
    out[..n].copy_from_slice(path);
    if n != 0 && out[n - 1] != b'\\' && out[n - 1] != b'/' {
        out[n] = b'\\';
        n += 1;
    }
    out[n] = 0;
    n
}

/// "Skhid baked textures, revision N\r\n", NUL-ended: its length.
fn stamp_text(out: &mut [u8; 64]) -> usize {
    let mut n = STAMP_TEXT.len();
    out[..n].copy_from_slice(STAMP_TEXT);
    let mut revision = baker::revision();
    let (mut digits, mut count) = ([0u8; 3], 0);
    loop {
        digits[count] = b'0' + revision % 10;
        count += 1;
        revision /= 10;
        if revision == 0 {
            break;
        }
    }
    while count > 0 {
        count -= 1;
        out[n] = digits[count];
        n += 1;
    }
    out[n] = b'\r';
    out[n + 1] = b'\n';
    out[n + 2] = 0;
    n + 2
}

/// The path in a buffer this module NUL-ended.
fn path(buffer: &[u8]) -> Path<'_> {
    Path::new(buffer).unwrap_or(Path::EMPTY)
}

// ---- the folder ---------------------------------------------------------------

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// A whole DDS the baker writes: "DDS ", a header of 124, DXT1 or DXT5, and
/// the file 128 bytes of header and the data the header's linear size says
/// (a file cut short by a game that stopped while it wrote is not).
fn whole(path: Path, size: u32) -> bool {
    let Some(mut file) = File::open(path) else { return false };
    let mut header = [0u8; 128];
    let got = file.read(&mut header);
    drop(file);
    if got != Some(128) {
        return false;
    }
    let (width, height, linear, fourcc) = (word(&header, 16), word(&header, 12), word(&header, 20), word(&header, 84));
    word(&header, 0) == u32::from_le_bytes(*b"DDS ") && word(&header, 4) == 124
        && (fourcc == u32::from_le_bytes(*b"DXT1") || fourcc == u32::from_le_bytes(*b"DXT5"))
        && (4..=4096).contains(&width) && (4..=4096).contains(&height) && linear != 0
        && size == linear.wrapping_add(128)
}

/// A record of a file kept from an earlier game, at the list's end.
fn seed(name: &[u8], size: u32) -> bool {
    let Some(mut record) = List::free() else { return false };
    if !join(record.text(), PREFIX, name) {
        return false;
    }
    record.push(size);
    true
}

/// The game's start, before the stock delete: true to delete the folder (no
/// stamp, or another revision's); else the folder's textures go back into
/// the list.
fn open(folder_path: Option<&[u8]>) -> bool {
    let (mut at, mut expected, mut found, mut data) = ([0u8; PATH], [0u8; 64], [0u8; 64], FindData::new());
    let base = folder(&mut at[..FOLDER], folder_path);
    if base == 0 || !join(&mut at[base..], STAMP_NAME, b"") {
        return true;
    }
    let want = stamp_text(&mut expected);
    let Some(mut file) = File::open(path(&at)) else { return true };
    let got = file.read(&mut found[..63]);
    drop(file);
    let Some(got) = got.filter(|&got| got as usize == want) else { return true };
    found[got as usize] = 0;
    if text(&found) != text(&expected) {
        return true;
    }

    let (mut files, mut bytes) = (0u32, 0u32);
    at[base] = 0;
    join(&mut at[base..], DDS_FILES, b"");
    let Some(mut find) = Find::first(path(&at), &mut data) else { return false };
    loop {
        if !data.is_folder() && join(&mut at[base..], data.name(), b"") {
            let (size, file) = (data.size(), path(&at));
            if data.size_high() == 0 && whole(file, size) && files < SEED_FILES
                && size <= SEED_BYTES.wrapping_sub(bytes) && seed(data.name(), size) {
                files += 1;
                bytes = bytes.wrapping_add(size);
            } else {
                files::delete(file);
            }
        }
        if !find.next(&mut data) {
            break;
        }
    }
    drop(find);
    false
}

/// After the stock delete: the folder made again and its stamp written, but
/// only when no .dds is left in it (the delete can fail, a file held open by
/// another game: each left is deleted, and if one stays the folder is not
/// stamped, so the next start deletes it again).
fn stamp(folder_path: Option<&[u8]>) {
    let (mut at, mut stamp, mut data) = ([0u8; PATH], [0u8; 64], FindData::new());
    let base = folder(&mut at[..FOLDER], folder_path);
    if base == 0 {
        return;
    }
    let mut left = false;
    at[base - 1] = 0;
    files::create_folder(path(&at));
    at[base - 1] = b'\\';
    join(&mut at[base..], DDS_FILES, b"");
    if let Some(mut find) = Find::first(path(&at), &mut data) {
        loop {
            if !data.is_folder() && (!join(&mut at[base..], data.name(), b"") || !files::delete(path(&at))) {
                left = true;
            }
            if !find.next(&mut data) {
                break;
            }
        }
        drop(find);
    }
    if left || !join(&mut at[base..], STAMP_NAME, b"") {
        return;
    }
    let n = stamp_text(&mut stamp);
    let Some(mut file) = File::create_always(path(&at)) else { return };
    file.write(&stamp[..n]);
    drop(file);
}

// ---- the queries' tables ---------------------------------------------------------

/// The first query of a path, whose bake or load is under way.
struct Owner {
    query: Query,
    surface: Surface,
    kind: u32,
    /// GetTickCount at its look-up.
    since: u32,
    path: [u8; RECORD_TEXT],
}

/// A loaded texture held for the game: a respawn's query gets it.
struct Kept {
    texture: Texture,
    bytes: u32,
    used: u32,
    path: [u8; RECORD_TEXT],
}

struct Tables {
    owners: [Option<Owner>; OWNERS],
    kept: [Option<Kept>; KEPT],
    kept_bytes: u32,
    kept_clock: u32,
    /// A waiting query and its owner's index.
    waiters: [Option<(Waiter, u8)>; WAITERS],
}

static TABLES: Lock<Tables> = Lock::new(Tables {
    owners: [const { None }; OWNERS],
    kept: [const { None }; KEPT],
    kept_bytes: 0,
    kept_clock: 0,
    waiters: [const { None }; WAITERS],
});
/// The owners in the table: finish_query's quick test, read without the lock.
static OWNED: AtomicU32 = AtomicU32::new(0);

/// What translate_query does with a query (`lookup`).
enum Looked {
    /// It bakes or loads (the path's owner when the table has room).
    Go,
    /// A query of the same path is under way: it waits (a reference to its
    /// model instance kept).
    Waits,
    /// A texture of the path is held: the query gets it at once.
    Held(Texture),
}

/// translate_query's look-up. An owner listed for STALE_MS (one that never
/// ended: none is known) is taken over by this query, its waiters with it.
fn lookup(thread: &Thread, at: &Lookup) -> Looked {
    let (Some(path), Some(query), Some(parameters)) = (at.path, at.query, at.parameters) else { return Looked::Go };
    let (kind, now) = (parameters.kind(), files::tick_count());
    TABLES.with(thread, |t| {
        if kind < KINDS {
            for kept in t.kept.iter_mut().flatten() {
                if text(&kept.path) == path {
                    t.kept_clock = t.kept_clock.wrapping_add(1);
                    kept.used = t.kept_clock;
                    return Looked::Held(kept.texture);
                }
            }
        }
        let owner = t.owners.iter().position(|o| o.as_ref().is_some_and(|o| text(&o.path) == path));
        match owner {
            Some(i) => {
                let owner = t.owners[i].as_mut().expect("the owner just found");
                if now.wrapping_sub(owner.since) > STALE_MS {
                    owner.query = query;
                    owner.surface = at.surface;
                    owner.kind = kind;
                    owner.since = now;
                    return Looked::Go;
                }
                let waiting = t.waiters.iter().flatten().filter(|(_, of)| usize::from(*of) == i).count();
                let free = t.waiters.iter().position(Option::is_none);
                match free {
                    Some(free) if owner.kind == kind && kind < KINDS && waiting < PER_OWNER => {
                        let model = parameters.model();
                        if let Some(model) = model {
                            model.hold();
                        }
                        t.waiters[free] = Some((Waiter { query, surface: at.surface, model, kind, cook: at.cook },
                                                i as u8));
                        Looked::Waits
                    }
                    _ => Looked::Go,
                }
            }
            None => {
                if let Some(slot) = t.owners.iter_mut().find(|o| o.is_none()) {
                    let mut text = [0u8; RECORD_TEXT];
                    if join(&mut text, path, b"") {
                        *slot = Some(Owner { query, surface: at.surface, kind, since: now, path: text });
                        OWNED.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Looked::Go
            }
        }
    })
}

/// How a texture was made: a bake (its texture not held), or a load (held
/// for the game besides).
#[derive(Clone, Copy, PartialEq, Eq)]
enum How {
    Baked,
    Loaded,
}

/// The end of an owner's query: its waiting queries into `out` and their
/// count, the owner out of the table; none when the query owns nothing. A
/// texture made (`made`): none also, the owner kept, unless the texture is
/// in the owner's surface's slot of its type, as the bake and the load put
/// it. A loaded texture is held for the game (`keep`).
fn take(thread: &Thread, query: Option<Query>, made: Option<(How, &Made)>,
        out: &mut [Option<Waiter>; PER_OWNER]) -> Option<usize> {
    let (mut gone, mut went) = ([None; KEPT], 0);
    let count = TABLES.with(thread, |t| {
        let i = query.and_then(|query| t.owners.iter().rposition(|o| o.as_ref().is_some_and(|o| o.query == query)))?;
        let owner = t.owners[i].as_ref().expect("the owner just found");
        if let Some((_, made)) = made {
            let held = made.texture.filter(|&texture| {
                made.surface == owner.surface && made.kind == owner.kind && made.kind < KINDS
                    && made.surface.holds(made.kind, texture)
            });
            held?;
        }
        let mut count = 0;
        for slot in t.waiters.iter_mut() {
            if let Some((waiter, of)) = *slot
                && usize::from(of) == i
                && count < PER_OWNER
            {
                out[count] = Some(waiter);
                count += 1;
                *slot = None;
            }
        }
        if let Some((How::Loaded, made)) = made
            && let Some(texture) = made.texture
        {
            went = keep(t, i, texture, &mut gone);
        }
        t.owners[i] = None;
        OWNED.fetch_sub(1, Ordering::Relaxed);
        Some(count)
    });
    for texture in gone[..went].iter().flatten() {
        texture.release();
    }
    count
}

/// A loaded texture (the owner `i`'s) held for the game: one reference
/// more, the least used let go past KEPT or KEPT_BYTES (into `gone`,
/// released by the caller with the lock let go). How many went.
fn keep(t: &mut Tables, i: usize, texture: Texture, gone: &mut [Option<Texture>; KEPT]) -> usize {
    let Some(owner) = t.owners[i].as_ref() else { return 0 };
    let path = text(&owner.path);
    let bytes = List::size_of(path);
    if bytes == 0 || bytes > KEPT_BYTES || t.kept.iter().flatten().any(|kept| text(&kept.path) == path) {
        return 0;
    }
    let mut count = 0;
    loop {
        let free = t.kept.iter().position(Option::is_none);
        let mut oldest: Option<(usize, u32)> = None;
        for (at, kept) in t.kept.iter().enumerate() {
            if let Some(kept) = kept
                && oldest.is_none_or(|(_, used)| kept.used <= used)
            {
                oldest = Some((at, kept.used));
            }
        }
        if let Some(free) = free
            && t.kept_bytes.wrapping_add(bytes) <= KEPT_BYTES
        {
            let mut held = [0u8; RECORD_TEXT];
            if !join(&mut held, path, b"") {
                return count;
            }
            texture.hold();
            t.kept_clock = t.kept_clock.wrapping_add(1);
            t.kept[free] = Some(Kept { texture, bytes, used: t.kept_clock, path: held });
            t.kept_bytes = t.kept_bytes.wrapping_add(bytes);
            return count;
        }
        let Some((oldest, _)) = oldest else { return count };
        let Some(let_go) = t.kept[oldest].take() else { return count };
        gone[count] = Some(let_go.texture);
        count += 1;
        t.kept_bytes = t.kept_bytes.wrapping_sub(let_go.bytes);
    }
}

/// The texture made: the waiting queries of the path the query owns get it,
/// when it is in the owner's surface's slot.
fn share(thread: &Thread, made: &Made, how: How) {
    let mut out = [None; PER_OWNER];
    let (Some(count), Some(texture)) = (take(thread, made.query, Some((how, made)), &mut out), made.texture) else {
        return;
    };
    for waiter in out[..count].iter().flatten() {
        waiter.give(texture);
    }
}

// ---- the hooks -------------------------------------------------------------------

/// `bake.game_start`: game::game's delete of the folder, only for another
/// revision's (then stamped); this revision's kept.
pub fn on_game_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let folder = Folder::at(ctx);
    if open(folder.path()) {
        folder.delete();
        stamp(folder.path());
    }
    Flow::Skip
}

/// `bake.check_crc`: a record of CRC 0 (one the start made of a kept file)
/// takes the CRC loaded; then the game's compare.
pub fn on_check_crc(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let check = CheckCrc::at(ctx);
    if check.crc() == 0 {
        check.set_crc(check.loaded);
    }
    Flow::Continue
}

/// `bake.lookup`: a path under way leaves translate_query by its end with
/// the query not finished (it waits); a held texture is given to the query
/// at once, and it leaves the same way; else the game's look-up.
pub fn on_lookup(ctx: &mut Context<'_>) -> Flow<BakeCacheOnLookupExit> {
    let at = Lookup::at(ctx);
    match lookup(ctx.thread(), &at) {
        Looked::Go => Flow::Continue,
        Looked::Waits => Flow::Exit(BakeCacheOnLookupExit::NotBaked),
        Looked::Held(texture) => {
            if let (Some(query), Some(parameters)) = (at.query, at.parameters) {
                held_for(query, at.surface, parameters, at.cook).give(texture);
            }
            Flow::Exit(BakeCacheOnLookupExit::NotBaked)
        }
    }
}

/// The waiter a query given a held texture at once is: no model reference
/// taken.
fn held_for(query: Query, surface: Surface, parameters: Parameters, cook: Cook) -> Waiter {
    Waiter { query, surface, model: None, kind: parameters.kind(), cook }
}

/// `bake.baked`: a bake's texture made; the waiting queries get it.
pub fn on_baked(ctx: &mut Context<'_>) -> Flow<NoExit> {
    share(ctx.thread(), &Made::baked(ctx), How::Baked);
    Made::name_allocation(ctx);
    Flow::Skip
}

/// `bake.loaded`: a load's texture made; the waiting queries get it, the
/// texture is held for the game.
pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    share(ctx.thread(), &Made::loaded(ctx), How::Loaded);
    Made::name_allocation(ctx);
    Flow::Skip
}

/// `bake.finish`: an owner that ends with nothing shared (an error, a query
/// made again, a texture not where it was looked for): the function, then
/// each waiting query made again. Every other query: the function, at once.
pub fn on_finish(ctx: &mut Context<'_>) -> Flow<BakeCacheOnFinishExit> {
    if OWNED.load(Ordering::Relaxed) == 0 {
        return Flow::Continue;
    }
    let finish = Finish::at(ctx);
    let mut out = [None; PER_OWNER];
    let Some(count) = take(ctx.thread(), finish.query, None, &mut out).filter(|&count| count > 0) else {
        return Flow::Continue;
    };
    finish.call();
    for waiter in out[..count].iter().flatten() {
        waiter.retranslate();
    }
    Flow::Exit(BakeCacheOnFinishExit::Done)
}

// ---- the CRC-32 step --------------------------------------------------------------

/// The polynomial 04C11DB7h reflected (the game's table at 0x225FD68).
const POLYNOMIAL: u32 = 0xEDB8 << 16 | 0x8320;

/// The game's table (T0) and the seven a step of eight bytes needs: Tk[i]
/// the CRC of i followed by k zero bytes.
static SLICES: [[u32; 256]; 8] = slices();

const fn slices() -> [[u32; 256]; 8] {
    let mut out = [[0u32; 256]; 8];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ POLYNOMIAL } else { c >> 1 };
            k += 1;
        }
        out[0][i] = c;
        i += 1;
    }
    let mut t = 1;
    while t < 8 {
        let mut i = 0;
        while i < 256 {
            out[t][i] = (out[t - 1][i] >> 8) ^ out[0][(out[t - 1][i] & 0xFF) as usize];
            i += 1;
        }
        t += 1;
    }
    out
}

/// The CRC-32 step over `bytes` from `crc`: eight bytes a step through the
/// eight tables, the rest a byte a step through T0 (the game's), the same
/// value as the game's byte a step for every input.
fn crc32(mut crc: u32, bytes: &[u8]) -> u32 {
    let t = &SLICES;
    let mut eights = bytes.chunks_exact(8);
    for b in &mut eights {
        let low = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) ^ crc;
        let high = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        crc = t[7][(low & 0xFF) as usize] ^ t[6][(low >> 8 & 0xFF) as usize] ^ t[5][(low >> 16 & 0xFF) as usize]
            ^ t[4][(low >> 24) as usize] ^ t[3][(high & 0xFF) as usize] ^ t[2][(high >> 8 & 0xFF) as usize]
            ^ t[1][(high >> 16 & 0xFF) as usize] ^ t[0][(high >> 24) as usize];
    }
    for &b in eights.remainder() {
        crc = t[0][((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc
}

/// `crc32.step`: the game's CRC-32 step, eight bytes a step.
pub fn on_crc_step(ctx: &mut Context<'_>) -> Flow<BakeCacheOnCrcStepExit> {
    let step = CrcStep::at(ctx);
    // a count of 0 reads nothing, not even the CRC (EAX may be anything)
    let crc = match step.bytes() {
        [] => 0,
        bytes => crc32(step.crc(), bytes),
    };
    step.done(ctx, crc);
    Flow::Exit(BakeCacheOnCrcStepExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's step, a byte at a time (0x42050).
    fn bytewise(mut crc: u32, bytes: &[u8]) -> u32 {
        for &b in bytes {
            crc = SLICES[0][((crc ^ u32::from(b)) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc
    }

    #[test]
    fn eight_a_step_is_the_byte_step() {
        assert_eq!(SLICES[0][1], 0x7707_3096);
        assert_eq!(!crc32(!0, b"123456789"), 0xCBF4_3926);
        let bytes: [u8; 300] = core::array::from_fn(|i| (i * 7 + 3) as u8);
        for (start, len) in [(0, 0), (0, 1), (1, 7), (2, 8), (3, 9), (0, 64), (5, 255), (1, 299)] {
            let crc = 0x1234_5678 ^ len as u32;
            assert_eq!(crc32(crc, &bytes[start..start + len]), bytewise(crc, &bytes[start..start + len]));
        }
    }

    #[test]
    fn strings() {
        let mut out = [0xAAu8; 8];
        assert!(join(&mut out, b"abc", b"de"));
        assert_eq!(text(&out), b"abcde");
        assert!(!join(&mut out, b"abcd", b"efgh"));
        assert_eq!(&out[..7], b"abcdefg");
        let mut at = [0u8; 10];
        assert_eq!(folder(&mut at, Some(b"c:\\x")), 5);
        assert_eq!(text(&at), b"c:\\x\\");
        assert_eq!(folder(&mut at, Some(b"c:/x/")), 5);
        assert_eq!(folder(&mut at, Some(b"")), 0);
        assert_eq!(folder(&mut at, Some(b"123456789")), 0);
        assert_eq!(folder(&mut at, None), 0);
    }
}
