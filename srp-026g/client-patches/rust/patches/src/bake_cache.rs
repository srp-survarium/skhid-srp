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
//!   bake format (BAKE_FORMAT) and the fingerprint of the game data a bake
//!   reads (`inputs`: the patches of resources.db that have models,
//!   materials, textures, shaders or camouflage and decal items), so across
//!   releases until one of them changes; its files of a whole DDS go back
//!   into the list (CRC 0: the first load takes the CRC of what it reads,
//!   `on_check_crc`), at most SEED_FILES and SEED_BYTES of them, the rest
//!   deleted. Another format, other game data, or no stamp: the stock
//!   delete, then the stamp (game::exit's delete is a byte write:
//!   patch_026g.py BAKE_CACHE_KEEP);
//! - a texture whose bake or load is under way (its owner, the first query
//!   of its path) is not baked or loaded again: a later query of the path
//!   waits, and gets the owner's texture when it is made (`on_baked`,
//!   `on_loaded`); if the owner ends otherwise, the waiting queries are made
//!   again (`on_finish`);
//! - a texture loaded from the folder is held for the game (KEPT,
//!   KEPT_BYTES, the least used let go), and a later query of its path (a
//!   respawn builds the player's model again) gets it at once;
//! - the CRC-32 step, which a bake and a load run over the whole file (and
//!   the texture streamer over each archive mip it checks): folded 64 bytes
//!   a step with carry-less multiplies, or eight bytes a step through tables
//!   (`vostok::crc32::update`, the same value).
//!
//! Textures only: nothing a match computes. The look-up, the textures made
//! and finish_query run on the resource threads too: the tables are behind
//! a [`Lock`].

#![forbid(unsafe_code)]

mod inputs;
use inputs::Patch;

use core::sync::atomic::{AtomicU32, Ordering};

use crate::vostok::baker::{CheckCrc, Finish, Folder, List, Lookup, Made, Query, Surface, Texture, TextureKind,
                           Waiter, RECORD_TEXT};
use crate::vostok::crc32::{self as crc_step, CrcCall};
use crate::vostok::files::{self, File, Find, FindData, Path};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::memory_profile;
use crate::vostok::{BakeCacheOnCrcStepExit, BakeCacheOnFinishExit, BakeCacheOnGameStartExit, BakeCacheOnLookupExit,
                    Context, Flow, Lock, NoExit, Thread};

const PREFIX: &[u8] = b"user_data/streamed_textures/";
const STAMP_NAME: &[u8] = b"srp_baked_textures.txt";
const STAMP_FORMAT: &[u8] = b"Skhid baked textures, format ";
const STAMP_INPUTS: &[u8] = b", inputs ";
/// A stamp's bytes at most.
const STAMP: usize = 80;
const DDS_FILES: &[u8] = b"*.dds";
/// Where the game mounts its data from (its working folder binaries\x86,
/// mount_patches_026g.asm), and its patches' names there.
const GAME_DATA: &[u8] = b"..\\..\\";
const PATCHES: &[u8] = b"resources.db.patch_*";
const PATCH_NAME: &[u8] = b"resources.db.patch_";

/// What a bake makes of the same game data, as the bake code makes it. A
/// change of that code that makes other bytes bumps it, which deletes every
/// player's folder once; the test `the_bake_code_is_this_format` fails at
/// any change of the code it names until its hash is set again, so that
/// change is weighed. Format 1: revision 54's bakes (patch 121's are the
/// same bytes). Format 2: a weapon's bake without the camouflage's `_nmap`
/// (its `n` texture not baked, or of its decals alone) and without the
/// surface's own `_camo` textures (weapon_camo).
const BAKE_FORMAT: u32 = 2;

/// Half the list: the game's own bakes have the rest.
const SEED_FILES: u32 = 512;
/// What remove_unwanted_files keeps (1 GiB) less 128 MiB for this game's
/// bakes before it trims. A 20-player match's folder is 0.9-1 GB (~500
/// files); half of 1 GiB deleted half of it at every start, and that game
/// baked ~400 textures again (fps-measure r54-warm, 2026-11-09).
const SEED_BYTES: u32 = 896 << 20;
const OWNERS: usize = 64;
const WAITERS: usize = 192;
const PER_OWNER: usize = 16;
/// An owner this old is taken over by the next query of its path.
const STALE_MS: u32 = 10_000;
/// Textures held for the game, the least used let go first ...
const KEPT: usize = 256;
/// ... and at most this many bytes of their files.
const KEPT_BYTES: u32 = 256 << 20;

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

/// `value` in decimal at `out`'s start: how many digits.
fn decimal(out: &mut [u8], mut value: u32) -> usize {
    let (mut digits, mut count) = ([0u8; 10], 0);
    loop {
        digits[count] = b'0' + (value % 10) as u8;
        count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for (slot, &digit) in out.iter_mut().zip(digits[..count].iter().rev()) {
        *slot = digit;
    }
    count
}

/// The decimal number `text` is (digits only, at most nine).
fn read_decimal(text: &[u8]) -> Option<u32> {
    (!text.is_empty() && text.len() <= 9 && text.iter().all(u8::is_ascii_digit))
        .then(|| text.iter().fold(0, |n, &d| n * 10 + u32::from(d - b'0')))
}

/// The number `text` is in 16 lower-case hex digits.
fn read_hex(text: &[u8]) -> Option<u64> {
    (text.len() == 16).then_some(())?;
    text.iter().try_fold(0u64, |n, &d| {
        let digit = match d {
            b'0'..=b'9' => d - b'0',
            b'a'..=b'f' => d - b'a' + 10,
            _ => return None,
        };
        Some(n << 4 | u64::from(digit))
    })
}

/// What a folder's stamp file says its bakes were made with: "Skhid baked
/// textures, format F, inputs H\r\n", the bake format and the fingerprint of
/// the game data a bake reads (`inputs`), 16 hex digits. Any other text is
/// no stamp (revision 54's "Skhid baked textures, revision 54", before
/// formats, was format 1's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    format: u32,
    inputs: u64,
}

impl Stamp {
    fn parse(text: &[u8]) -> Option<Stamp> {
        let text = text.strip_suffix(b"\r\n")?;
        let rest = text.strip_prefix(STAMP_FORMAT)?;
        let comma = rest.iter().position(|&b| b == b',')?;
        let (format, inputs) = (read_decimal(&rest[..comma])?, read_hex(rest[comma..].strip_prefix(STAMP_INPUTS)?)?);
        Some(Stamp { format, inputs })
    }
}

/// A format stamp's text in `out`: its length.
fn made_text(out: &mut [u8; STAMP], format: u32, inputs: u64) -> usize {
    let mut n = STAMP_FORMAT.len();
    out[..n].copy_from_slice(STAMP_FORMAT);
    n += decimal(&mut out[n..], format);
    out[n..n + STAMP_INPUTS.len()].copy_from_slice(STAMP_INPUTS);
    n += STAMP_INPUTS.len();
    for shift in (0..16).rev() {
        out[n] = b"0123456789abcdef"[(inputs >> (4 * shift) & 0xF) as usize];
        n += 1;
    }
    out[n..n + 2].copy_from_slice(b"\r\n");
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

// ---- the game data a bake reads ----------------------------------------------------

/// What each patch of the game data gives the fingerprint, by its number
/// (`inputs`); a number with no patch gives nothing.
struct GameData([Patch; 256]);

impl GameData {
    /// The fingerprint of the patches.
    fn inputs(&self) -> u64 {
        inputs::fingerprint((1..=255u8).map(|n| (n, self.0[usize::from(n)])))
    }
}

/// The patch's number of a name `resources.db.patch_<n>` (n 1 to 255, as
/// the mount writes it).
fn patch_number(name: &[u8]) -> Option<u8> {
    let digits = name.strip_prefix(PATCH_NAME)?;
    let n = read_decimal(digits).filter(|_| digits[0] != b'0')?;
    u8::try_from(n).ok().filter(|&n| n != 0)
}

/// What the patch `n` gives: its file table read whole (pages of our own,
/// given back after).
fn read_patch(thread: &Thread, n: u8) -> Patch {
    let mut name = [0u8; 48];
    let at = GAME_DATA.len() + PATCH_NAME.len();
    name[..GAME_DATA.len()].copy_from_slice(GAME_DATA);
    name[GAME_DATA.len()..at].copy_from_slice(PATCH_NAME);
    decimal(&mut name[at..], u32::from(n));
    let Some(mut file) = File::open(path(&name)) else { return Patch::Unreadable };
    let mut header = [0u8; inputs::HEADER];
    let size = (file.read(&mut header) == Some(inputs::HEADER as u32)).then(|| inputs::table_size(&header)).flatten();
    let Some(mut table) = size.and_then(|size| memory_profile::reserve::<u8>(thread, size as usize)) else {
        return Patch::Unreadable;
    };
    let bytes = table.items();
    let mut got = 0;
    while got < bytes.len() {
        match file.read(&mut bytes[got..]) {
            Some(more) if more > 0 => got += more as usize,
            _ => return Patch::Unreadable,
        }
    }
    inputs::patch(bytes)
}

/// The game data's patches, each read; none when resources.db is not where
/// the game mounts it from (the walk would see no patch: nothing is known).
fn game_data(thread: &Thread) -> Option<GameData> {
    let (mut at, mut found) = ([0u8; 64], FindData::new());
    join(&mut at, GAME_DATA, b"resources.db");
    drop(Find::first(path(&at), &mut found)?);
    let mut data = GameData([Patch::Nothing; 256]);
    join(&mut at, GAME_DATA, PATCHES);
    let Some(mut find) = Find::first(path(&at), &mut found) else { return Some(data) };
    loop {
        if !found.is_folder()
            && let Some(n) = patch_number(found.name())
        {
            data.0[usize::from(n)] = read_patch(thread, n);
        }
        if !find.next(&mut found) {
            break;
        }
    }
    Some(data)
}

/// What the start does with the folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keep {
    /// Its stamp is this game's: its textures go back into the list.
    Kept,
    /// Another format, other game data, or no stamp: the stock delete, then
    /// stamped of `inputs`; not stamped when the game data is not known.
    Deleted { inputs: Option<u64> },
}

/// Whether the bakes the stamp says are what this game bakes: the same
/// format, of the same game data.
fn decide(found: Option<Stamp>, data: Option<&GameData>) -> Keep {
    let Some(data) = data else { return Keep::Deleted { inputs: None } };
    let now = data.inputs();
    match found {
        Some(Stamp { format, inputs }) if format == BAKE_FORMAT && inputs == now => Keep::Kept,
        _ => Keep::Deleted { inputs: Some(now) },
    }
}

/// The folder's stamp, none without one this module reads.
fn read_stamp(folder_path: Option<&[u8]>) -> Option<Stamp> {
    let (mut at, mut found) = ([0u8; PATH], [0u8; STAMP]);
    let base = folder(&mut at[..FOLDER], folder_path);
    if base == 0 || !join(&mut at[base..], STAMP_NAME, b"") {
        return None;
    }
    let mut file = File::open(path(&at))?;
    let got = file.read(&mut found)?;
    Stamp::parse(&found[..got as usize])
}

/// A kept folder's textures back into the list: each whole DDS, at most
/// SEED_FILES and SEED_BYTES of them; the rest deleted.
fn seed_folder(folder_path: Option<&[u8]>) {
    let (mut at, mut data) = ([0u8; PATH], FindData::new());
    let base = folder(&mut at[..FOLDER], folder_path);
    if base == 0 {
        return;
    }
    let (mut files, mut bytes) = (0u32, 0u32);
    join(&mut at[base..], DDS_FILES, b"");
    let Some(mut find) = Find::first(path(&at), &mut data) else { return };
    loop {
        if !data.is_folder() && join(&mut at[base..], data.name(), b"") {
            let file = path(&at);
            let kept = data.size().filter(|&size| {
                whole(file, size) && files < SEED_FILES && size <= SEED_BYTES.wrapping_sub(bytes)
                    && seed(data.name(), size)
            });
            match kept {
                Some(size) => {
                    files += 1;
                    bytes = bytes.wrapping_add(size);
                }
                None => {
                    files::delete(file);
                }
            }
        }
        if !find.next(&mut data) {
            break;
        }
    }
}

/// The folder's stamp written: this format's, of `inputs`.
fn write_stamp(at: &mut [u8; PATH], base: usize, inputs: u64) {
    let mut stamp = [0u8; STAMP];
    if !join(&mut at[base..], STAMP_NAME, b"") {
        return;
    }
    let n = made_text(&mut stamp, BAKE_FORMAT, inputs);
    let Some(mut file) = File::create_always(path(at)) else { return };
    file.write(&stamp[..n]);
}

/// After the stock delete: the folder made again and its stamp written (of
/// `inputs`; none when the game data is not known), but only when no .dds
/// is left in it (the delete can fail, a file held open by another game:
/// each left is deleted, and if one stays the folder is not stamped, so the
/// next start deletes it again).
fn stamp(folder_path: Option<&[u8]>, inputs: Option<u64>) {
    let (mut at, mut data) = ([0u8; PATH], FindData::new());
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
    }
    if let Some(inputs) = inputs.filter(|_| !left) {
        write_stamp(&mut at, base, inputs);
    }
}

// ---- the queries' tables ---------------------------------------------------------

/// The first query of a path, whose bake or load is under way.
struct Owner {
    query: Query,
    surface: Option<Surface>,
    /// Its texture's type (none for a value past the four: it has no
    /// waiters).
    kind: Option<TextureKind>,
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
        if kind.is_some() {
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
                match (free, kind, at.surface, at.cook) {
                    (Some(free), Some(kind), Some(surface), Some(cook))
                        if owner.kind == Some(kind) && waiting < PER_OWNER =>
                    {
                        let model = parameters.model();
                        if let Some(model) = model {
                            model.hold();
                        }
                        t.waiters[free] = Some((Waiter { query, surface, model, kind, cook }, i as u8));
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
            let (Some(texture), Some(surface), Some(kind)) = (made.texture, made.surface, made.kind) else {
                return None;
            };
            if owner.surface != Some(surface) || owner.kind != Some(kind) || !surface.holds(kind, texture) {
                return None;
            }
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
        // the first of the least used
        let mut oldest: Option<(usize, u32)> = None;
        for (at, kept) in t.kept.iter().enumerate() {
            if let Some(kept) = kept
                && oldest.is_none_or(|(_, used)| kept.used < used)
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

/// `bake.game_start`: game::game's delete of the folder, only when its
/// bakes are not what this game bakes (then stamped); else kept. Either way
/// the code goes on past the delete's call.
pub fn on_game_start(ctx: &mut Context<'_>) -> Flow<BakeCacheOnGameStartExit> {
    let Some(folder) = Folder::at(ctx) else { return Flow::Continue };
    let data = game_data(ctx.thread());
    let keep = decide(read_stamp(folder.path()), data.as_ref());
    match keep {
        Keep::Kept => seed_folder(folder.path()),
        Keep::Deleted { inputs } => {
            folder.delete();
            stamp(folder.path(), inputs);
        }
    }
    let what = match keep {
        Keep::Kept => "kept: ",
        Keep::Deleted { .. } => "deleted (another format, other game data or no stamp): ",
    };
    let (mut text, inputs) = ([0u8; STAMP], data.map(|data| data.inputs()));
    let n = inputs.map_or(0, |inputs| made_text(&mut text, BAKE_FORMAT, inputs) - 2);
    let made = core::str::from_utf8(&text[..n]).unwrap_or("");
    log::line(&[Text("bake_cache: the baked textures "), Text(what),
                Text(if inputs.is_some() { made } else { "the game data not found, not stamped" })]);
    Flow::Exit(BakeCacheOnGameStartExit::Kept)
}

/// `bake.check_crc`: a record of CRC 0 (one the start made of a kept file)
/// takes the CRC loaded; then the game's compare.
pub fn on_check_crc(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(check) = CheckCrc::at(ctx)
        && check.crc() == 0
    {
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
            // the waiter a query given a held texture at once is: no model
            // reference taken
            if let (Some(query), Some(surface), Some(kind), Some(cook)) =
                (at.query, at.surface, at.parameters.and_then(|p| p.kind()), at.cook)
            {
                Waiter { query, surface, model: None, kind, cook }.give(texture);
            }
            Flow::Exit(BakeCacheOnLookupExit::NotBaked)
        }
    }
}

/// `bake.baked`: a bake's texture made; the waiting queries get it.
pub fn on_baked(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(made) = Made::baked(ctx) {
        share(ctx.thread(), &made, How::Baked);
    }
    Flow::Continue
}

/// `bake.loaded`: a load's texture made; the waiting queries get it, the
/// texture is held for the game.
pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(made) = Made::loaded(ctx) {
        share(ctx.thread(), &made, How::Loaded);
    }
    Flow::Continue
}

/// `bake.finish`: an owner that ends with nothing shared (an error, a query
/// made again, a texture not where it was looked for): the function, then
/// each waiting query made again. Every other query: the function, at once.
pub fn on_finish(ctx: &mut Context<'_>) -> Flow<BakeCacheOnFinishExit> {
    if OWNED.load(Ordering::Relaxed) == 0 {
        return Flow::Continue;
    }
    let Some(finish) = Finish::at(ctx) else { return Flow::Continue };
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

/// `crc32.step`: the game's CRC-32 step, folded or sliced (`vostok::crc32::update`).
pub fn on_crc_step(ctx: &mut Context<'_>) -> Flow<BakeCacheOnCrcStepExit> {
    match CrcCall::at(ctx) {
        // no CRC or no bytes to read: the game's own step
        None => return Flow::Continue,
        // a count of 0 reads nothing, not even the CRC (EAX may be anything)
        Some(CrcCall::Empty) => crc_step::done_empty(ctx),
        Some(CrcCall::Bytes(step)) => {
            let crc = crc_step::update(step.crc(), step.bytes());
            step.done(ctx, crc);
        }
    }
    Flow::Exit(BakeCacheOnCrcStepExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn stamps() {
        let mut out = [0u8; STAMP];
        let n = made_text(&mut out, 1, 0x7A07_992F_99F5_AA9C);
        assert_eq!(&out[..n], b"Skhid baked textures, format 1, inputs 7a07992f99f5aa9c\r\n");
        assert_eq!(Stamp::parse(&out[..n]), Some(Stamp { format: 1, inputs: 0x7A07_992F_99F5_AA9C }));
        let n = made_text(&mut out, u32::MAX, u64::MAX);
        assert_eq!(n, 66);
        for other in [&b"Skhid baked textures, revision 54\r\n"[..], b"Skhid baked textures, format 1, inputs 7a07992f99f5aa9c",
                      b"Skhid baked textures, format 1, inputs 7a07992f99f5aa9\r\n",
                      b"Skhid baked textures, format 1, inputs 7A07992F99F5AA9C\r\n",
                      b"Skhid baked textures, format x, inputs 7a07992f99f5aa9c\r\n", b""] {
            assert_eq!(Stamp::parse(other), None, "{:?}", core::str::from_utf8(other));
        }
        assert_eq!(patch_number(b"resources.db.patch_54"), Some(54));
        for other in [&b"resources.db.patch_054"[..], b"resources.db.patch_0", b"resources.db.patch_256",
                      b"resources.db.patch_", b"resources.db.patch_5.tmp", b"resources.db"] {
            assert_eq!(patch_number(other), None);
        }
    }

    #[test]
    fn kept_while_the_format_and_the_game_data_are_this_game_s() {
        let mut data = GameData([Patch::Nothing; 256]);
        data.0[40] = Patch::Files(1);
        data.0[54] = Patch::Files(2);
        data.0[55] = Patch::Nothing;
        let now = data.inputs();
        let made = |format, inputs| Some(Stamp { format, inputs });
        let deleted = Keep::Deleted { inputs: Some(now) };
        assert_eq!(decide(made(BAKE_FORMAT, now), Some(&data)), Keep::Kept);
        assert_eq!(decide(made(BAKE_FORMAT + 1, now), Some(&data)), deleted);
        assert_eq!(decide(made(BAKE_FORMAT, now ^ 1), Some(&data)), deleted);
        assert_eq!(decide(None, Some(&data)), deleted);
        assert_eq!(decide(made(BAKE_FORMAT, now), None), Keep::Deleted { inputs: None }, "the game data not known");
    }

    /// The code that decides the bytes a bake makes: patch 121's bake, the
    /// baker's API, the weapons' camouflage and decals (patches 44 and 47,
    /// and 44's part in the baker).
    /// An exe patch of the baker's own code (0x11F000..0x120000,
    /// 0x12C000..0x133000) is such a change too.
    const BAKE_CODE: [&[u8]; 8] = [include_bytes!("bake_async.rs"), include_bytes!("bake_async.toml"),
                                   include_bytes!("vostok/bake_gpu.rs"), include_bytes!("vostok/baker.rs"),
                                   include_bytes!("../../../weapon_camo_026g.asm"),
                                   include_bytes!("../../../weapon_decals_026g.asm"),
                                   include_bytes!("weapon_camo.rs"), include_bytes!("weapon_camo.toml")];
    /// BAKE_CODE's hash (its bytes but '\r') as BAKE_FORMAT was set.
    const BAKE_CODE_HASH: u64 = 0xad19_3667_ac65_a7c4;

    #[test]
    fn the_bake_code_is_this_format() {
        let hash = BAKE_CODE.iter().fold(inputs::Fnv::new(), |hash, code| {
            code.iter().filter(|&&b| b != b'\r').fold(hash, |hash, &b| hash.bytes(&[b])).bytes(&[0])
        });
        assert_eq!(hash.get(), BAKE_CODE_HASH,
                   "the bake code changed (BAKE_CODE): if a bake makes other bytes of the same game data now, bump \
                    BAKE_FORMAT (every player's folder is deleted once); either way \
                    set BAKE_CODE_HASH to {:#018x}", hash.get());
    }
}
