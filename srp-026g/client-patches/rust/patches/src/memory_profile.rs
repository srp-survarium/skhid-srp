//! Patch 127: a memory profiler of the engine's arenas, for the owner's own
//! game (`docs/notes/memory-profile.md`). Dormant unless the game starts
//! with `SKHID_MEMORY_PROFILE=on`, read once when the arenas are made
//! ([`Setting`]). Off, nothing is reserved, written or logged, and the
//! allocator is the game's byte for byte: its gates are written into it
//! only when the profile is on (`vostok::memory_profile::install`).
//!
//! On, every block the engine's heaps hand out or take back (dlmalloc's
//! mspace_malloc, mspace_realloc, mspace_free: each doug_lea arena's heap and
//! ptmalloc3's "global multithreaded", its mapped blocks too) is counted to the arena it lies in
//! and the place in the game that asked for it ([`CallSite`]), in fixed
//! tables of our own room (VirtualAlloc, never an engine heap):
//!
//! - [`Blocks`]: each live block's site, 2 Mi slots; full (7/8), a block
//!   is counted to its arena as untracked, and its free later as unseen;
//! - [`Sites`]: each (arena, site)'s live bytes and blocks, its peak, and
//!   all it asked, 16 Ki slots; full (7/8), a new site's blocks go to its
//!   arena's `other` count.
//!
//! A [`Moment`] writes a snapshot to `srp_memory_profile.txt` in the game's
//! user data folder (Documents\Survarium): a world in, a match closing, a
//! fixed arena full before out_of_memory_with_crash ends the game, and
//! Scroll Lock (polled by the gates, [`Keys`]). A snapshot is each arena (its heap walked: used, largest
//! free, the free blocks by size), its sites, and the address space region
//! by region; `scripts/memory-profile/analyze.py` reads it.
//!
//! The gates run on any of the game's threads: the tables are one
//! [`YieldingLock`] (its waiters give their time slice away after a few
//! spins), taken by a gate only after the game's body gave a block and
//! before it takes one back, so a block is in the tables only while it is
//! live. Nothing under the lock reaches the engine's heaps or calls the
//! engine; the file is opened outside it.
#![forbid(unsafe_code)]

use crate::vostok::files::tick_count;
use crate::vostok::memory_profile::ScrollLock;
use crate::vostok::gun_cook::process_id;
use crate::vostok::log::{line, Piece::*};
use crate::vostok::memory::{placed, process, regions_map};
use crate::vostok::memory_profile::{self as runtime, Block, CallSite, HeapCall, StartRefused};
use crate::vostok::trace::TraceFile;
use crate::vostok::{Context, Flow, InstallRefused, NoExit, Thread, YieldingLock};
use srp_validate::memory::{Usage, MAX_ARENAS};

/// The variable that switches the profiler on, and what it says.
const VARIABLE: &[u8] = b"SKHID_MEMORY_PROFILE\0";
/// The snapshots' file in the user data folder.
const FILE: &str = "srp_memory_profile.txt";
/// Live blocks the tables track at most (7/8 of the slots).
const BLOCK_SLOTS: usize = 1 << 21;
/// Call sites the tables tell apart at most (7/8 of the slots).
const SITE_SLOTS: usize = 1 << 14;
/// The text buffered before a write to the file.
const OUT_BYTES: usize = 1 << 16;
/// The address space's regions written at most.
const MAX_REGIONS: usize = 1 << 16;
/// The arenas of the engine's list, then one for blocks outside them all.
const ARENAS: usize = MAX_ARENAS + 1;
const OUTSIDE: u8 = MAX_ARENAS as u8;

/// SKHID_MEMORY_PROFILE as the game was started with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Off,
    On,
}

impl Setting {
    /// `on` or `1` (any case) is on; anything else, or nothing, off.
    fn parse(value: Option<&[u8]>) -> Setting {
        match value {
            Some(value) if value.eq_ignore_ascii_case(b"on") || value == b"1" => Setting::On,
            _ => Setting::Off,
        }
    }
}

/// When a snapshot is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Moment {
    /// A match's world is in (world_load.loaded).
    Loaded,
    /// The match is closing on the game, its world still there.
    Closing,
    /// The arena `arena` had no room for `asked` bytes: the game ends next.
    OutOfMemory { arena: u8, asked: Option<u32> },
    /// The owner pressed Scroll Lock.
    Key,
}

/// A block's arena and site, kept while it lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tracked {
    block: Block,
    site: SiteRef,
}

/// Where a block's bytes are counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SiteRef {
    /// A slot of [`Sites`].
    Slot(u16),
    /// Its arena's `other`: the site table was full.
    Other(u8),
}

/// What a site, or an arena, holds and has asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    live: u64,
    blocks: u32,
    peak: u64,
    asked: u32,
    asked_bytes: u64,
}

impl Counts {
    fn add(&mut self, bytes: u32) {
        self.live = self.live.saturating_add(u64::from(bytes));
        self.blocks = self.blocks.saturating_add(1);
        self.peak = self.peak.max(self.live);
        self.asked = self.asked.saturating_add(1);
        self.asked_bytes = self.asked_bytes.saturating_add(u64::from(bytes));
    }

    fn remove(&mut self, bytes: u32) {
        self.live = self.live.saturating_sub(u64::from(bytes));
        self.blocks = self.blocks.saturating_sub(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SiteKey {
    arena: u8,
    site: CallSite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Site {
    key: SiteKey,
    counts: Counts,
}

/// Fibonacci hashing of a word to `bits` bits.
fn hash(word: u32, bits: u32) -> usize {
    // 2^32 / phi, odd (a word of the layer check's size is spelled in halves)
    const GOLDEN: u32 = 0x9E37 << 16 | 0x79B1;
    (word.wrapping_mul(GOLDEN) >> (32 - bits)) as usize
}

fn bits_of(slots: usize) -> u32 {
    slots.trailing_zeros().clamp(1, 31)
}

/// The live blocks: open addressing, linear probes, a removal shifting
/// back what followed it (no tombstones). Its length is a power of two.
struct Blocks<'a> {
    slots: &'a mut [Option<Tracked>],
    count: usize,
}

impl<'a> Blocks<'a> {
    fn new(slots: &'a mut [Option<Tracked>]) -> Blocks<'a> {
        Blocks { slots, count: 0 }
    }

    fn home(&self, block: Block) -> usize {
        hash(block.address() >> 3, bits_of(self.slots.len()))
    }

    fn next(&self, at: usize) -> usize {
        (at + 1) & (self.slots.len() - 1)
    }

    /// Tracks `block`; false when the table is full (7/8). A block tracked
    /// already (a block the game never gave back through a gate) has its
    /// old site replaced, which is returned to be uncounted.
    fn insert(&mut self, tracked: Tracked) -> Result<Option<SiteRef>, Full> {
        let mut at = self.home(tracked.block);
        let most = self.slots.len() / 8 * 7;
        for _ in 0..self.slots.len() {
            match self.slots.get_mut(at) {
                Some(slot @ None) => {
                    if self.count >= most {
                        return Err(Full);
                    }
                    *slot = Some(tracked);
                    self.count += 1;
                    return Ok(None);
                }
                Some(Some(held)) if held.block == tracked.block => {
                    let old = held.site;
                    held.site = tracked.site;
                    return Ok(Some(old));
                }
                _ => at = self.next(at),
            }
        }
        Err(Full)
    }

    /// The site `block` was tracked with, and it is no longer.
    fn remove(&mut self, block: Block) -> Option<SiteRef> {
        let mut at = self.home(block);
        let mut found = None;
        for _ in 0..self.slots.len() {
            match self.slots.get(at).copied().flatten() {
                None => break,
                Some(held) if held.block == block => {
                    found = Some(held.site);
                    break;
                }
                Some(_) => at = self.next(at),
            }
        }
        let site = found?;
        // shift back each later block of the run that may sit in the hole
        let mut hole = at;
        let mut probe = self.next(at);
        while let Some(held) = self.slots.get(probe).copied().flatten() {
            let home = self.home(held.block);
            // `held` may move to the hole when its home is not in (hole, probe]
            let stays = if hole <= probe { hole < home && home <= probe } else { hole < home || home <= probe };
            if !stays {
                if let Some(slot) = self.slots.get_mut(hole) {
                    *slot = Some(held);
                }
                hole = probe;
            }
            probe = self.next(probe);
        }
        if let Some(slot) = self.slots.get_mut(hole) {
            *slot = None;
        }
        self.count -= 1;
        Some(site)
    }
}

/// A table that is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Full;

/// The call sites, by arena and return address: never removed.
struct Sites<'a> {
    slots: &'a mut [Option<Site>],
    count: usize,
}

impl<'a> Sites<'a> {
    fn new(slots: &'a mut [Option<Site>]) -> Sites<'a> {
        Sites { slots, count: 0 }
    }

    /// The slot of `key`, made when new; Full at 7/8.
    fn slot(&mut self, key: SiteKey) -> Result<u16, Full> {
        let len = self.slots.len();
        let mut at = hash(key.site.0 ^ u32::from(key.arena).rotate_left(27), bits_of(len));
        for _ in 0..len {
            match self.slots.get_mut(at) {
                Some(Some(site)) if site.key == key => return u16::try_from(at).map_err(|_| Full),
                Some(slot @ None) => {
                    if self.count >= len / 8 * 7 {
                        return Err(Full);
                    }
                    let index = u16::try_from(at).map_err(|_| Full)?;
                    *slot = Some(Site { key, counts: Counts::default() });
                    self.count += 1;
                    return Ok(index);
                }
                _ => at = (at + 1) & (len - 1),
            }
        }
        Err(Full)
    }

    fn counts(&mut self, index: u16) -> Option<&mut Counts> {
        self.slots.get_mut(usize::from(index))?.as_mut().map(|site| &mut site.counts)
    }
}

/// An arena's own counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ArenaCounts {
    /// Its tracked blocks, all sites.
    tracked: Counts,
    /// The sites the site table had no room for.
    other: Counts,
    /// Blocks the block table had no room for, and their bytes.
    untracked: u32,
    untracked_bytes: u64,
    /// Frees of blocks not tracked (from before the profile, or untracked).
    unseen: u32,
}

/// Where an arena lies: `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place {
    start: u32,
    end: u32,
}

/// The profile's whole state while it is on.
struct Profiler<'a> {
    blocks: Blocks<'a>,
    sites: Sites<'a>,
    places: [Option<Place>; MAX_ARENAS],
    arenas: [ArenaCounts; ARENAS],
}

impl<'a> Profiler<'a> {
    fn new(blocks: &'a mut [Option<Tracked>], sites: &'a mut [Option<Site>], places: [Option<Place>; MAX_ARENAS]) -> Profiler<'a> {
        Profiler { blocks: Blocks::new(blocks), sites: Sites::new(sites), places, arenas: [ArenaCounts::default(); ARENAS] }
    }

    /// The arena `address` lies in (the first that holds it), else OUTSIDE.
    fn arena_of(&self, address: u32) -> u8 {
        self.places
            .iter()
            .position(|place| place.is_some_and(|place| place.start <= address && address < place.end))
            .and_then(|index| u8::try_from(index).ok())
            .unwrap_or(OUTSIDE)
    }

    fn arena(&mut self, arena: u8) -> &mut ArenaCounts {
        let last = self.arenas.len() - 1;
        &mut self.arenas[usize::from(arena).min(last)]
    }

    fn allocated(&mut self, block: Block, bytes: u32, site: CallSite) {
        let arena = self.arena_of(block.address());
        let site = match self.sites.slot(SiteKey { arena, site }) {
            Ok(slot) => SiteRef::Slot(slot),
            Err(Full) => SiteRef::Other(arena),
        };
        match self.blocks.insert(Tracked { block, site }) {
            Ok(replaced) => {
                if let Some(old) = replaced {
                    self.uncount(old, arena, bytes);
                }
                self.count(site, arena, bytes);
            }
            Err(Full) => {
                let counts = self.arena(arena);
                counts.untracked = counts.untracked.saturating_add(1);
                counts.untracked_bytes = counts.untracked_bytes.saturating_add(u64::from(bytes));
            }
        }
    }

    fn freeing(&mut self, block: Block, bytes: u32) {
        let arena = self.arena_of(block.address());
        match self.blocks.remove(block) {
            Some(site) => self.uncount(site, arena, bytes),
            None => {
                let counts = self.arena(arena);
                counts.unseen = counts.unseen.saturating_add(1);
            }
        }
    }

    fn count(&mut self, site: SiteRef, arena: u8, bytes: u32) {
        match site {
            SiteRef::Slot(slot) => {
                if let Some(counts) = self.sites.counts(slot) {
                    counts.add(bytes);
                }
            }
            SiteRef::Other(_) => self.arena(arena).other.add(bytes),
        }
        self.arena(arena).tracked.add(bytes);
    }

    fn uncount(&mut self, site: SiteRef, arena: u8, bytes: u32) {
        match site {
            SiteRef::Slot(slot) => {
                if let Some(counts) = self.sites.counts(slot) {
                    counts.remove(bytes);
                }
            }
            SiteRef::Other(_) => self.arena(arena).other.remove(bytes),
        }
        self.arena(arena).tracked.remove(bytes);
    }
}

/// `value / 10` and its last digit, by shifts (the game's build links no
/// 64-bit division; Hacker's Delight's divu10).
fn tenth(value: u64) -> (u64, u8) {
    let mut q = (value >> 1) + (value >> 2);
    q += q >> 4;
    q += q >> 8;
    q += q >> 16;
    q += q >> 32;
    q >>= 3;
    let mut r = value - ((q << 3) + (q << 1));
    if r > 9 {
        q += 1;
        r -= 10;
    }
    (q, r as u8)
}

/// Text to the file, buffered; each piece a word, tab-separated.
struct Text<'b, W: FnMut(&[u8])> {
    buffer: &'b mut [u8],
    at: usize,
    /// The line has no word yet.
    fresh: bool,
    write: W,
}

impl<'b, W: FnMut(&[u8])> Text<'b, W> {
    fn new(buffer: &'b mut [u8], write: W) -> Text<'b, W> {
        Text { buffer, at: 0, fresh: true, write }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.at >= self.buffer.len() {
                self.flush();
            }
            if let Some(slot) = self.buffer.get_mut(self.at) {
                *slot = byte;
                self.at += 1;
            }
        }
    }

    /// A word of the line (a tab before all but the first).
    fn word(&mut self, text: &str) -> &mut Self {
        if !self.fresh {
            self.bytes(b"\t");
        }
        self.fresh = false;
        // a word never has a tab or a line's end in it
        for &byte in text.as_bytes() {
            self.bytes(&[if byte == b'\t' || byte == b'\n' { b' ' } else { byte }]);
        }
        self
    }

    fn dec(&mut self, value: u64) -> &mut Self {
        let mut digits = [0u8; 20];
        let mut at = digits.len();
        let mut rest = value;
        loop {
            let (quotient, digit) = tenth(rest);
            at -= 1;
            digits[at] = b'0' + digit;
            rest = quotient;
            if rest == 0 {
                break;
            }
        }
        self.word(core::str::from_utf8(&digits[at..]).unwrap_or("?"))
    }

    fn hex(&mut self, value: u32) -> &mut Self {
        let mut digits = *b"0x00000000";
        for (index, slot) in digits[2..].iter_mut().enumerate() {
            let nibble = (value >> (28 - 4 * index)) & 0xF;
            *slot = b"0123456789abcdef"[nibble as usize];
        }
        self.word(core::str::from_utf8(&digits).unwrap_or("?"))
    }

    fn end(&mut self) {
        self.bytes(b"\n");
        self.fresh = true;
    }

    fn flush(&mut self) {
        if self.at > 0 {
            (self.write)(&self.buffer[..self.at]);
            self.at = 0;
        }
    }
}

/// The free blocks of a heap by size: bucket `n` holds those of 2^n to
/// 2^(n+1) - 1 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Holes {
    count: [u32; 32],
    bytes: [u64; 32],
}

impl Holes {
    const NONE: Holes = Holes { count: [0; 32], bytes: [0; 32] };

    fn add(&mut self, bytes: u32) {
        if bytes == 0 {
            return;
        }
        let bucket = (31 - bytes.leading_zeros()) as usize;
        if let (Some(count), Some(sum)) = (self.count.get_mut(bucket), self.bytes.get_mut(bucket)) {
            *count = count.saturating_add(1);
            *sum = sum.saturating_add(u64::from(bytes));
        }
    }
}

/// The counts lines of `counts` after `head`'s words.
fn counts_words<W: FnMut(&[u8])>(text: &mut Text<'_, W>, counts: &Counts) {
    text.dec(counts.live).dec(u64::from(counts.blocks)).dec(counts.peak).dec(u64::from(counts.asked)).dec(counts.asked_bytes);
}

/// Each site's line, then each arena's `other`.
fn sites_lines<W: FnMut(&[u8])>(text: &mut Text<'_, W>, profiler: &Profiler<'_>) {
    for site in profiler.sites.slots.iter().flatten() {
        text.word("site").dec(u64::from(site.key.arena)).hex(site.key.site.0);
        counts_words(text, &site.counts);
        text.end();
    }
    for (index, counts) in profiler.arenas.iter().enumerate() {
        if counts.other.asked > 0 {
            text.word("other").dec(index as u64);
            counts_words(text, &counts.other);
            text.end();
        }
    }
}

/// How often the gates look at Scroll Lock at most (ms).
const KEY_POLL_MS: u32 = 200;

/// Scroll Lock as last seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyState {
    Up,
    Down,
}

/// The snapshot key, polled from the gates (an allocation every few
/// milliseconds in any game), at most every [`KEY_POLL_MS`].
struct Keys {
    key: Option<ScrollLock>,
    next: u32,
    state: KeyState,
}

impl Keys {
    /// The next state at `now`, the key read by `down` when a poll is due:
    /// true when it went down (a snapshot).
    fn poll(&mut self, now: u32, down: impl FnOnce(&ScrollLock) -> bool) -> bool {
        if (now.wrapping_sub(self.next) as i32) < 0 {
            return false;
        }
        self.next = now.wrapping_add(KEY_POLL_MS);
        let Some(key) = &self.key else { return false };
        let (state, pressed) = key_step(self.state, down(key));
        self.state = state;
        pressed
    }
}

/// Scroll Lock's next state from a poll that found it `down`, and whether
/// it went down (a snapshot).
fn key_step(state: KeyState, down: bool) -> (KeyState, bool) {
    match (state, down) {
        (KeyState::Up, true) => (KeyState::Down, true),
        (KeyState::Down, true) => (KeyState::Down, false),
        (_, false) => (KeyState::Up, false),
    }
}

/// The file's state.
enum Output {
    /// Not opened yet: the first snapshot opens it, outside the lock (the
    /// game's user data folder is the engine's, whose first call sets it up:
    /// not ours to make at the start).
    NotYet,
    /// A snapshot is opening it (outside the lock); another one meanwhile
    /// writes nothing.
    Opening,
    Open(TraceFile),
    /// It could not be opened: no snapshot is written.
    Failed,
}

/// The profile on: its tables, its room's place, its file.
struct On {
    profiler: Profiler<'static>,
    out: &'static mut [u8],
    output: Output,
    /// The room reserved, for the report: (base, bytes) each.
    rooms: [(u32, u32); 3],
    snapshots: u32,
    keys: Keys,
}

enum Profile {
    Off,
    On(On),
}

static PROFILE: YieldingLock<Profile> = YieldingLock::new(Profile::Off);

/// The arenas' places, in the engine's list's order.
fn places(thread: &Thread) -> [Option<Place>; MAX_ARENAS] {
    let mut places = [None; MAX_ARENAS];
    for (slot, arena) in places.iter_mut().zip(placed(thread)) {
        let start = arena.start();
        *slot = (start != 0).then(|| Place { start, end: start.saturating_add(arena.size()) });
    }
    places
}

pub fn on_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let mut value = [0u8; 16];
    if Setting::parse(runtime::environment(thread, VARIABLE, &mut value)) == Setting::Off {
        return Flow::Continue;
    }
    // the gates written only with the tables made, both or neither (vostok's start)
    let started = runtime::start(ctx, |thread| {
        let blocks = runtime::reserve::<Option<Tracked>>(thread, BLOCK_SLOTS)?;
        let sites = runtime::reserve::<Option<Site>>(thread, SITE_SLOTS)?;
        let out = runtime::reserve::<u8>(thread, OUT_BYTES)?;
        Some((blocks, sites, out))
    });
    let (blocks, sites, out) = match started {
        Ok(tables) => tables,
        Err(refused) => {
            let (why, site) = match refused {
                StartRefused::NotTheStart => ("memory profile: not at the start", None),
                StartRefused::NoRoom => ("memory profile: no room for its tables", None),
                StartRefused::Site(InstallRefused::Changed { site }) => ("memory profile: the allocator's bytes are not the game's at ", Some(site)),
                StartRefused::Site(InstallRefused::Protect { site }) => ("memory profile: its gate could not be made writable at ", Some(site)),
                StartRefused::Site(InstallRefused::TooMany) => ("memory profile: too many gates", None),
            };
            match site {
                Some(site) => line(&[Text(why), Hex(site), Text("; off, nothing written")]),
                None => line(&[Text(why), Text("; off, nothing written")]),
            }
            return Flow::Continue;
        }
    };
    let rooms = [blocks.place(), sites.place(), out.place()];
    let profiler = Profiler::new(blocks.keep(), sites.keep(), places(thread));
    let keys = Keys { key: ScrollLock::resolve(thread), next: tick_count(), state: KeyState::Up };
    PROFILE.with(thread, |profile| {
        *profile = Profile::On(On { profiler, out: out.keep(), output: Output::NotYet, rooms, snapshots: 0, keys });
    });
    line(&[
        Text("memory profile on: snapshots to "),
        Text(FILE),
        Text(" (a world in, a match closing, an arena full, Scroll Lock); its tables "),
        Dec(rooms.iter().map(|&(_, bytes)| bytes).sum::<u32>()),
        Text(" bytes at "),
        Hex(rooms[0].0),
    ]);
    Flow::Continue
}

pub fn on_allocated(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let HeapCall::Block { block, bytes, site } = runtime::allocated(ctx) {
        let pressed = PROFILE.with(ctx.thread(), |profile| match profile {
            Profile::On(on) => {
                on.profiler.allocated(block, bytes, site);
                on.keys.poll(tick_count(), ScrollLock::down)
            }
            Profile::Off => false,
        });
        if pressed {
            snapshot(ctx.thread(), Moment::Key);
        }
    }
    Flow::Continue
}

pub fn on_freeing(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let HeapCall::Block { block, bytes, .. } = runtime::freeing(ctx) {
        PROFILE.with(ctx.thread(), |profile| {
            if let Profile::On(on) = profile {
                on.profiler.freeing(block, bytes);
            }
        });
    }
    Flow::Continue
}

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    snapshot(ctx.thread(), Moment::Loaded);
    Flow::Continue
}

pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    snapshot(ctx.thread(), Moment::Closing);
    Flow::Continue
}

pub fn on_out_of_memory(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if !runtime::is_on() {
        return Flow::Continue;
    }
    if let Some(full) = runtime::out_of_memory(ctx) {
        let arena = PROFILE.with(ctx.thread(), |profile| match profile {
            Profile::On(on) => on.profiler.arena_of(full.space),
            Profile::Off => OUTSIDE,
        });
        snapshot(ctx.thread(), Moment::OutOfMemory { arena, asked: full.asked });
    }
    Flow::Continue
}

/// A snapshot at `moment`, when the profile is on.
fn snapshot(thread: &Thread, moment: Moment) {
    if !runtime::is_on() {
        return;
    }
    // the first snapshot opens the file outside the lock (the engine's user
    // data folder and CreateFileA), claiming it first so another waits for none
    let open = PROFILE.with(thread, |profile| match profile {
        Profile::On(on) if matches!(on.output, Output::NotYet) => {
            on.output = Output::Opening;
            true
        }
        _ => false,
    });
    let opened = open.then(|| TraceFile::open(thread, FILE));
    PROFILE.with(thread, |profile| {
        let Profile::On(on) = profile else { return };
        if let Some(opened) = opened {
            on.output = opened.map_or(Output::Failed, Output::Open);
            if let Output::Open(file) = &on.output {
                let mut text = Text::new(&mut *on.out, |bytes: &[u8]| file.write(thread, bytes));
                text.word("run").dec(u64::from(process_id())).word("tick").dec(u64::from(tick_count()));
                text.word("slots").dec(BLOCK_SLOTS as u64).dec(SITE_SLOTS as u64);
                text.end();
                for (base, bytes) in on.rooms {
                    text.word("room").hex(base).dec(u64::from(bytes));
                    text.end();
                }
                text.flush();
            }
        }
        let Output::Open(file) = &on.output else { return };
        on.snapshots = on.snapshots.saturating_add(1);
        let mut text = Text::new(&mut *on.out, |bytes: &[u8]| file.write(thread, bytes));
        write_snapshot(thread, &mut text, &on.profiler, on.snapshots, moment);
        text.flush();
    });
}

fn write_snapshot<W: FnMut(&[u8])>(thread: &Thread, text: &mut Text<'_, W>, profiler: &Profiler<'_>, number: u32, moment: Moment) {
    text.word("snapshot").dec(u64::from(number)).word(match moment {
        Moment::Loaded => "loaded",
        Moment::Closing => "closing",
        Moment::OutOfMemory { .. } => "out_of_memory",
        Moment::Key => "key",
    });
    text.word("tick").dec(u64::from(tick_count()));
    if let Moment::OutOfMemory { arena, asked } = moment {
        text.word("arena").dec(u64::from(arena));
        if let Some(asked) = asked {
            text.word("asked").dec(u64::from(asked));
        }
    }
    text.end();
    let whole = process(thread);
    text.word("process").word("free").dec(u64::from(whole.free)).word("largest").dec(u64::from(whole.largest_free));
    text.word("video").dec(whole.video_budget);
    text.end();
    text.word("blocks").dec(profiler.blocks.count as u64).word("sites").dec(profiler.sites.count as u64);
    text.end();
    for (index, arena) in placed(thread).enumerate().take(MAX_ARENAS) {
        let mut holes = Holes::NONE;
        let usage = arena.usage_with(|bytes| holes.add(bytes));
        text.word("arena").dec(index as u64).word(arena.name().as_str()).hex(arena.start()).dec(u64::from(arena.size()));
        match usage {
            Usage::Unknown => text.word("unknown"),
            Usage::Counted { used } => text.word("counted").dec(u64::from(used)),
            Usage::Heap { used, largest_free } => text.word("heap").dec(u64::from(used)).dec(u64::from(largest_free)),
        };
        text.end();
        if matches!(usage, Usage::Heap { .. }) {
            text.word("holes").dec(index as u64);
            for (bucket, (&count, &bytes)) in holes.count.iter().zip(&holes.bytes).enumerate() {
                if count > 0 {
                    text.dec(bucket as u64).dec(u64::from(count)).dec(bytes);
                }
            }
            text.end();
        }
    }
    for (index, counts) in profiler.arenas.iter().enumerate() {
        if counts.tracked.asked > 0 || counts.untracked > 0 || counts.unseen > 0 {
            text.word("tracked").dec(index as u64);
            counts_words(text, &counts.tracked);
            text.dec(u64::from(counts.untracked)).dec(counts.untracked_bytes).dec(u64::from(counts.unseen));
            text.end();
        }
    }
    sites_lines(text, profiler);
    for region in regions_map(thread, MAX_REGIONS) {
        text.word("region").hex(region.base).hex(region.size).hex(region.state).hex(region.protect).hex(region.kind).hex(region.allocation);
        text.end();
    }
    text.word("end").dec(u64::from(number));
    text.end();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn block(address: u32) -> Block {
        Block::at(address).unwrap()
    }

    fn tables(blocks: usize, sites: usize) -> (Vec<Option<Tracked>>, Vec<Option<Site>>) {
        (std::vec![None; blocks], std::vec![None; sites])
    }

    fn places(list: &[(u32, u32)]) -> [Option<Place>; MAX_ARENAS] {
        let mut places = [None; MAX_ARENAS];
        for (slot, &(start, size)) in places.iter_mut().zip(list) {
            *slot = Some(Place { start, end: start + size });
        }
        places
    }

    #[test]
    fn the_key_snapshots_once_a_press_and_is_polled_when_due() {
        let mut state = KeyState::Up;
        let mut presses = 0;
        for down in [false, true, true, false, true, false, false] {
            let (next, pressed) = key_step(state, down);
            state = next;
            presses += usize::from(pressed);
        }
        assert_eq!((presses, state), (2, KeyState::Up));
        // no key resolved (a ScrollLock is user32's, none in a test): polled when due, never pressed
        let mut keys = Keys { key: None, next: 5000, state: KeyState::Up };
        assert!(!keys.poll(4999, |_| true));
        assert_eq!(keys.next, 5000, "not due: nothing read");
        assert!(!keys.poll(5000, |_| true));
        assert_eq!(keys.next, 5000 + KEY_POLL_MS);
        let mut wrapping = Keys { key: None, next: 10, state: KeyState::Up };
        assert!(!wrapping.poll(u32::MAX - 5, |_| true));
        assert_eq!(wrapping.next, 10, "a tick just before the counter wraps is before 10");
    }

    #[test]
    fn the_variable_says_on_or_off() {
        assert_eq!(Setting::parse(None), Setting::Off);
        assert_eq!(Setting::parse(Some(b"on")), Setting::On);
        assert_eq!(Setting::parse(Some(b"ON")), Setting::On);
        assert_eq!(Setting::parse(Some(b"1")), Setting::On);
        for off in [&b"0"[..], b"off", b"", b"yes", b"full"] {
            assert_eq!(Setting::parse(Some(off)), Setting::Off);
        }
    }

    #[test]
    fn blocks_are_counted_to_their_arena_and_site_until_freed() {
        let (mut b, mut s) = tables(64, 16);
        let mut profiler = Profiler::new(&mut b, &mut s, places(&[(0x1000_0000, 0x100_0000), (0x2000_0000, 0x10_0000)]));
        let (here, there) = (CallSite(0x40_1234), CallSite(0x50_0000));
        profiler.allocated(block(0x1000_0010), 0x100, here);
        profiler.allocated(block(0x1000_0200), 0x40, here);
        profiler.allocated(block(0x2000_0010), 0x20, there);
        profiler.allocated(block(0x3000_0010), 0x18, there);
        assert_eq!(profiler.arenas[0].tracked.live, 0x140);
        assert_eq!(profiler.arenas[1].tracked.live, 0x20);
        assert_eq!(profiler.arenas[usize::from(OUTSIDE)].tracked.live, 0x18);
        profiler.freeing(block(0x1000_0010), 0x100);
        profiler.freeing(block(0x1000_0010), 0x100); // again: unseen, nothing uncounted twice
        profiler.freeing(block(0x1000_0400), 0x10); // never seen
        let arena = profiler.arenas[0];
        assert_eq!((arena.tracked.live, arena.tracked.peak, arena.tracked.blocks, arena.unseen), (0x40, 0x140, 1, 2));
        let site = profiler.sites.slots.iter().flatten().find(|site| site.key == SiteKey { arena: 0, site: here }).unwrap();
        assert_eq!(site.counts, Counts { live: 0x40, blocks: 1, peak: 0x140, asked: 2, asked_bytes: 0x140 });
        assert_eq!(profiler.sites.count, 3, "a site is its arena's and its address's");
    }

    #[test]
    fn a_full_block_table_counts_untracked_and_frees_them_unseen() {
        let (mut b, mut s) = tables(8, 16);
        let mut profiler = Profiler::new(&mut b, &mut s, places(&[(0x1000_0000, 0x100_0000)]));
        for n in 0..10 {
            profiler.allocated(block(0x1000_0000 + 0x100 * n), 0x10, CallSite(1));
        }
        assert_eq!(profiler.blocks.count, 7);
        assert_eq!((profiler.arenas[0].untracked, profiler.arenas[0].untracked_bytes), (3, 0x30));
        profiler.freeing(block(0x1000_0000 + 0x100 * 9), 0x10);
        assert_eq!(profiler.arenas[0].tracked.live, 0x70);
        assert_eq!(profiler.arenas[0].unseen, 1);
    }

    #[test]
    fn a_full_site_table_counts_new_sites_as_their_arenas_other() {
        let (mut b, mut s) = tables(64, 8);
        let mut profiler = Profiler::new(&mut b, &mut s, places(&[(0x1000_0000, 0x100_0000)]));
        for n in 0..10 {
            profiler.allocated(block(0x1000_0000 + 0x100 * n), 0x10, CallSite(0x40_0000 + n));
        }
        assert_eq!(profiler.sites.count, 7);
        assert_eq!(profiler.arenas[0].other.live, 0x30);
        profiler.freeing(block(0x1000_0000 + 0x100 * 9), 0x10);
        assert_eq!(profiler.arenas[0].other.live, 0x20);
        assert_eq!(profiler.arenas[0].tracked.live, 0x90);
    }

    #[test]
    fn removals_keep_every_other_block_findable() {
        // many blocks of few homes: long runs that wrap the table's end
        let (mut b, _) = tables(256, 0);
        let mut blocks = Blocks::new(&mut b);
        let mut model = std::collections::HashMap::new();
        let mut seed = 0x1234_5678u32;
        for step in 0..20_000u32 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let address = 0x1000_0000 + 8 * (seed >> 24);
            if step % 3 == 0 {
                assert_eq!(blocks.remove(block(address)), model.remove(&address), "step {step}");
            } else {
                let site = SiteRef::Slot((step % 7) as u16);
                match blocks.insert(Tracked { block: block(address), site }) {
                    Ok(old) => assert_eq!(old, model.insert(address, site)),
                    Err(Full) => assert!(model.len() >= 224 && !model.contains_key(&address)),
                }
            }
            assert_eq!(blocks.count, model.len());
        }
        for (&address, &site) in &model {
            assert_eq!(blocks.remove(block(address)), Some(site));
        }
        assert!(blocks.slots.iter().all(Option::is_none));
    }

    #[test]
    fn text_is_words_by_tabs_and_lines() {
        let mut written = Vec::new();
        let mut buffer = [0u8; 8];
        let mut text = Text::new(&mut buffer, |bytes: &[u8]| written.extend_from_slice(bytes));
        text.word("site").dec(0).hex(0x40_1234).dec(u64::MAX).word("a\tb");
        text.end();
        text.word("end").dec(7);
        text.end();
        text.flush();
        assert_eq!(std::str::from_utf8(&written).unwrap(), "site\t0\t0x00401234\t18446744073709551615\ta b\nend\t7\n");
    }

    #[test]
    fn a_tenth_by_shifts_is_a_tenth() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        for value in [0, 9, 10, 11, 99, 100, u64::from(u32::MAX), u64::MAX, u64::MAX - 1].into_iter().chain((0..100_000).map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed >> (seed % 64)
        })) {
            assert_eq!(tenth(value), (value / 10, (value % 10) as u8), "{value}");
        }
    }

    #[test]
    fn holes_go_by_powers_of_two() {
        let mut holes = Holes::NONE;
        for bytes in [0, 16, 31, 32, 0x10_0000, u32::MAX] {
            holes.add(bytes);
        }
        assert_eq!((holes.count[4], holes.bytes[4]), (2, 47));
        assert_eq!((holes.count[5], holes.count[20], holes.count[31]), (1, 1, 1));
        assert_eq!(holes.count.iter().sum::<u32>(), 5);
    }

    #[test]
    fn site_lines_carry_each_site_and_each_arenas_other() {
        let (mut b, mut s) = tables(64, 8);
        let mut profiler = Profiler::new(&mut b, &mut s, places(&[(0x1000_0000, 0x100_0000)]));
        for n in 0..8 {
            profiler.allocated(block(0x1000_0000 + 0x100 * n), 0x10, CallSite(0x40_0000 + n));
        }
        let mut written = Vec::new();
        let mut buffer = [0u8; 64];
        let mut text = Text::new(&mut buffer, |bytes: &[u8]| written.extend_from_slice(bytes));
        sites_lines(&mut text, &profiler);
        text.flush();
        let text = std::string::String::from_utf8(written).unwrap();
        assert_eq!(text.lines().filter(|l| l.starts_with("site\t0\t0x0040000")).count(), 7);
        assert!(text.lines().any(|l| l == "other\t0\t16\t1\t16\t1\t16"), "{text}");
    }
}
