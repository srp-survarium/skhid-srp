//! How much memory a client's game has left (client patch 126,
//! `docs/notes/memory-report.md`): the game is a 32-bit process, carved
//! at its start into fixed arenas and what is left for the video driver,
//! and an arena that is full ends the game. A [`Report`] is a snapshot of
//! each arena the engine registered and of the address space, taken at a
//! [`Moment`] of a match; the server only logs it. Where the address space
//! outside the arenas goes is its census ([`Census`], every allocation by
//! its type), its [`TOP`] largest allocations outside the arenas
//! ([`Allocation`], a file's name with each image and mapping), and what
//! the game runs with ([`Client`]: Windows or Wine, the d3d11.dll in use,
//! the baked textures held), all read in one walk ([`Walk`]).
//!
//! The body of a [`crate::report`] carrier of kind memory:
//!
//! ```text
//! u8 moment  u8 count  count x { u8 kind  body[45] }
//!   kind 0 process: u32 free  u32 largest_free  u32 video_low  u32 video_high  zeros[29]
//!   kind 1 arena:   name[32] (printable ASCII, zero-padded)  u32 size  u8 usage  u32 used  u32 largest_free
//!     usage 0 unknown (used and largest_free zero), 1 counted (largest_free zero), 2 heap
//!   kind 2 census:  3 x { u32 reserved  u32 committed  u32 allocations } (image, mapped, private)  zeros[9]
//!   kind 3 allocation: u32 base  u8 type  u32 size  u32 committed  name[32]
//!     type 0 image, 1 mapped, 2 private; name: its file's (as an arena's), zeros for none (always, private)
//!   kind 4 client:  u8 system  u8 renderer  u32 kept  u32 kept_bytes  u32 listed  u32 listed_bytes  zeros[27]
//!     system 0 Windows, 1 Wine; renderer 0 unbound, else 1 + 3 x (0 Direct3D, 1 DXVK) + folder
//!     (0 the game's dxvk, 1 the system's, 2 another)
//! ```
//!
//! The process is the report's first entry, and only there; the others
//! follow in any order (the game's: the arenas, the census, the client, the
//! allocations largest first).

/// An arena's name's bytes; a longer one is cut.
pub const NAME: usize = 32;
const BODY: usize = 45;
const RECORD: usize = 1 + BODY;
const HEAD: usize = 2;
/// Records in one part: a part's body, with the carrier's header, stays in
/// one 0xE0 chunk ([`crate::report::MAX_SIZE`]).
pub const PER_PART: usize = 10;
/// The arenas the engine's list holds at most (a fixed_vector of 32 in
/// 0.10b's memory.cpp; a full list asserts, register_allocator 0xE21C0).
pub const MAX_ARENAS: usize = 32;
/// The allocations outside the arenas a report tells: the largest.
pub const TOP: usize = 20;
/// Entries of one report: the process, the arenas, the census, the client
/// and the allocations.
pub const MAX_ENTRIES: usize = 1 + MAX_ARENAS + 2 + TOP;
/// Parts of one report.
pub const MAX_PARTS: u8 = MAX_ENTRIES.div_ceil(PER_PART) as u8;
/// A part's body at most.
pub const MAX_BODY: usize = HEAD + PER_PART * RECORD;

/// When in a match the snapshot was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Moment {
    /// The connection's first world is in: the level, its bakes and the
    /// players' textures are loaded.
    Loaded,
    /// The match is closing on the game (close_current_match), its world
    /// still there: the match's own peak.
    Ended,
}

impl Moment {
    pub fn word(self) -> &'static str {
        match self {
            Moment::Loaded => "loaded",
            Moment::Ended => "ended",
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Moment::Loaded => 0,
            Moment::Ended => 1,
        }
    }

    fn of(byte: u8) -> Option<Moment> {
        match byte {
            0 => Some(Moment::Loaded),
            1 => Some(Moment::Ended),
            _ => None,
        }
    }
}

/// The process as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Process {
    /// Free address space in all (`VirtualQuery`'s MEM_FREE regions).
    pub free: u32,
    /// The largest free region: the most one allocation outside the
    /// arenas (the video driver's) can still get.
    pub largest_free: u32,
    /// The video memory budget the engine computed at its start (its
    /// "video memory stats for engine").
    pub video_budget: u64,
}

/// What is known of an arena's bytes in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Usage {
    /// Its allocator is of a kind not read, its memory was not all
    /// readable, or its heap changed under the walk (another thread's).
    Unknown,
    /// Bytes taken, as the allocator counts them; it has no free block to
    /// tell (the managed resources' defragment, the multithreaded arena's
    /// pages are taken from a free list).
    Counted { used: u32 },
    /// A dlmalloc heap walked chunk by chunk: the bytes of its chunks in
    /// use (what the engine's "out of memory in arena" dump calls used)
    /// and its largest free block, the never-used top included: the
    /// largest allocation that still fits.
    Heap { used: u32, largest_free: u32 },
}

/// An arena the engine registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arena {
    pub name: Name,
    /// Its bytes, as registered.
    pub size: u32,
    pub usage: Usage,
}

/// An arena's name: 1 to [`NAME`] bytes of printable ASCII.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    bytes: [u8; NAME],
    len: u8,
}

impl Name {
    /// The name of a C string's bytes (up to its NUL, cut at [`NAME`]);
    /// a byte that is no printable ASCII becomes `?`, no byte at all `?`.
    pub fn of_c(text: &[u8]) -> Name {
        let mut name = Name { bytes: [0; NAME], len: 0 };
        for (slot, &byte) in name.bytes.iter_mut().zip(text.iter().take_while(|&&byte| byte != 0)) {
            *slot = if byte.is_ascii_graphic() || byte == b' ' { byte } else { b'?' };
            name.len = name.len.saturating_add(1);
        }
        if name.len == 0 {
            name.bytes = [0; NAME];
            if let Some(first) = name.bytes.first_mut() {
                *first = b'?';
            }
            name.len = 1;
        }
        name
    }

    /// A file's name of its path's bytes (up to its NUL): what follows its
    /// last `\` or `/`, as [`Name::of_c`] keeps it; none for a path that
    /// ends in a separator or is empty.
    pub fn of_file(path: &[u8]) -> Option<Name> {
        let path = path.split(|&byte| byte == 0).next().unwrap_or_default();
        let file = path.rsplit(|&byte| byte == b'\\' || byte == b'/').next().unwrap_or_default();
        (!file.is_empty()).then(|| Name::of_c(file))
    }

    pub fn as_str(&self) -> &str {
        self.bytes.get(..usize::from(self.len)).and_then(|bytes| core::str::from_utf8(bytes).ok()).unwrap_or_default()
    }

    fn parse(bytes: &[u8]) -> Option<Name> {
        let bytes: [u8; NAME] = bytes.try_into().ok()?;
        let len = bytes.iter().position(|&byte| byte == 0).unwrap_or(NAME);
        let (text, rest) = bytes.split_at(len);
        if text.is_empty() || !text.iter().all(|&byte| byte.is_ascii_graphic() || byte == b' ') || rest.iter().any(|&byte| byte != 0) {
            return None;
        }
        Some(Name { bytes, len: len as u8 })
    }
}

impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// What an allocation of the address space is (`VirtualQuery`'s `Type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    /// MEM_IMAGE: a module (the exe, a DLL) mapped from its file.
    Image,
    /// MEM_MAPPED: a view of a section (a file mapping, shared memory).
    Mapped,
    /// MEM_PRIVATE: VirtualAlloc's (the arenas, heaps, the drivers' own).
    Private,
}

impl Type {
    /// `VirtualQuery`'s `Type` word: MEM_IMAGE 1000000h, MEM_MAPPED
    /// 40000h, MEM_PRIVATE 20000h.
    pub fn of(word: u32) -> Option<Type> {
        match word {
            0x100_0000 => Some(Type::Image),
            0x4_0000 => Some(Type::Mapped),
            0x2_0000 => Some(Type::Private),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Type::Image => "image",
            Type::Mapped => "mapped",
            Type::Private => "private",
        }
    }

    fn byte(self) -> u8 {
        match self {
            Type::Image => 0,
            Type::Mapped => 1,
            Type::Private => 2,
        }
    }

    fn of_byte(byte: u8) -> Option<Type> {
        match byte {
            0 => Some(Type::Image),
            1 => Some(Type::Mapped),
            2 => Some(Type::Private),
            _ => None,
        }
    }
}

/// The address space one [`Type`] holds: every allocation of it, the
/// arenas' too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Footprint {
    reserved: u32,
    committed: u32,
    allocations: u32,
}

impl Footprint {
    /// Its bytes taken from the address space, committed or only reserved.
    pub fn reserved(self) -> u32 {
        self.reserved
    }

    /// Of those, the committed ones (backed by memory or the page file).
    pub fn committed(self) -> u32 {
        self.committed
    }

    /// Its allocations (reservations, each from its own base).
    pub fn allocations(self) -> u32 {
        self.allocations
    }

    fn parse(reserved: u32, committed: u32, allocations: u32) -> Option<Footprint> {
        (committed <= reserved && (reserved == 0) == (allocations == 0)).then_some(Footprint { reserved, committed, allocations })
    }
}

/// The address space taken, by [`Type`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Census {
    pub image: Footprint,
    pub mapped: Footprint,
    pub private: Footprint,
}

impl Census {
    fn of(&mut self, kind: Type) -> &mut Footprint {
        match kind {
            Type::Image => &mut self.image,
            Type::Mapped => &mut self.mapped,
            Type::Private => &mut self.private,
        }
    }
}

/// Who an allocation is, as far as the game can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Owner {
    /// A module: its file's name (`GetModuleFileNameA`), when found.
    Image(Option<Name>),
    /// A view of a section: its file's name (`GetMappedFileNameA`), none
    /// for one of no file (shared memory, the drivers').
    Mapped(Option<Name>),
    /// VirtualAlloc's: no name.
    Private,
}

impl Owner {
    pub fn kind(self) -> Type {
        match self {
            Owner::Image(_) => Type::Image,
            Owner::Mapped(_) => Type::Mapped,
            Owner::Private => Type::Private,
        }
    }

    pub fn name(self) -> Option<Name> {
        match self {
            Owner::Image(name) | Owner::Mapped(name) => name,
            Owner::Private => None,
        }
    }
}

/// An allocation outside the arenas, among the largest ([`TOP`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Allocation {
    base: u32,
    size: u32,
    committed: u32,
    owner: Owner,
}

impl Allocation {
    /// Its first byte (`VirtualQuery`'s AllocationBase).
    pub fn base(self) -> u32 {
        self.base
    }

    /// Its bytes taken from the address space, committed or only reserved.
    pub fn size(self) -> u32 {
        self.size
    }

    /// Of those, the committed ones.
    pub fn committed(self) -> u32 {
        self.committed
    }

    pub fn owner(self) -> Owner {
        self.owner
    }
}

/// The system the game runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum System {
    Windows,
    /// ntdll exports `wine_get_version`.
    Wine,
}

impl System {
    pub fn word(self) -> &'static str {
        match self {
            System::Windows => "windows",
            System::Wine => "wine",
        }
    }
}

/// The folder a DLL was loaded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Folder {
    /// A folder named `dxvk`: the game's own DXVK (patch 83, the Settings'
    /// Renderer on Vulkan/DXVK).
    Dxvk,
    /// `system32` or `syswow64`: the system's (on Wine, its prefix's, where
    /// play_skhid.sh installs DXVK).
    System,
    /// Any other, or none told (a player's own beside the exe).
    Elsewhere,
}

impl Folder {
    /// The folder of a module's path (`GetModuleFileNameA`'s, up to its
    /// NUL), by its last folder's name, in any case.
    pub fn of_path(path: &[u8]) -> Folder {
        let path = path.split(|&byte| byte == 0).next().unwrap_or_default();
        let mut parts = path.rsplit(|&byte| byte == b'\\' || byte == b'/');
        let (_file, folder) = (parts.next(), parts.next().unwrap_or_default());
        if folder.eq_ignore_ascii_case(b"dxvk") {
            Folder::Dxvk
        } else if folder.eq_ignore_ascii_case(b"system32") || folder.eq_ignore_ascii_case(b"syswow64") {
            Folder::System
        } else {
            Folder::Elsewhere
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Folder::Dxvk => "dxvk folder",
            Folder::System => "system folder",
            Folder::Elsewhere => "elsewhere",
        }
    }

    fn byte(self) -> u8 {
        match self {
            Folder::Dxvk => 0,
            Folder::System => 1,
            Folder::Elsewhere => 2,
        }
    }

    fn of_byte(byte: u8) -> Option<Folder> {
        match byte {
            0 => Some(Folder::Dxvk),
            1 => Some(Folder::System),
            2 => Some(Folder::Elsewhere),
            _ => None,
        }
    }
}

/// The Direct3D 11 the game draws with: the d3d11.dll its delayed import
/// is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Renderer {
    /// None bound yet (the game has not created its device).
    Unbound,
    /// A d3d11.dll with the layered-device exports
    /// (`D3D11CoreRegisterLayers`): Windows' own, or Wine's over wined3d.
    Direct3d(Folder),
    /// One without them: DXVK's (3.1.1 exports four functions, none of
    /// those).
    Dxvk(Folder),
}

impl Renderer {
    fn byte(self) -> u8 {
        match self {
            Renderer::Unbound => 0,
            Renderer::Direct3d(folder) => folder.byte().saturating_add(1),
            Renderer::Dxvk(folder) => folder.byte().saturating_add(4),
        }
    }

    fn of_byte(byte: u8) -> Option<Renderer> {
        match byte {
            0 => Some(Renderer::Unbound),
            1..=3 => Folder::of_byte(byte.checked_sub(1)?).map(Renderer::Direct3d),
            4..=6 => Folder::of_byte(byte.checked_sub(4)?).map(Renderer::Dxvk),
            _ => None,
        }
    }
}

/// The baked camouflage/decal textures (patch 70).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Textures {
    /// Textures loaded from the folder the cache holds for the game (at
    /// most 256), and their files' bytes (at most 256 MiB).
    pub kept: u32,
    pub kept_bytes: u32,
    /// The baker's list of saved textures (its records, at most 1024), and
    /// their files' bytes: what this game baked or found in the folder.
    pub listed: u32,
    pub listed_bytes: u32,
}

/// What the game runs with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Client {
    pub system: System,
    pub renderer: Renderer,
    pub textures: Textures,
}

/// One entry of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry {
    Process(Process),
    Arena(Arena),
    Census(Census),
    Allocation(Allocation),
    Client(Client),
}

/// A record's body written field by field; what does not fit is left out.
struct Out<'a>(core::slice::IterMut<'a, u8>);

impl Out<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        for (&byte, slot) in bytes.iter().zip(self.0.by_ref()) {
            *slot = byte;
        }
    }

    fn word(&mut self, word: u32) {
        self.bytes(&word.to_le_bytes());
    }
}

/// A record's body read field by field.
struct In<'a>(&'a [u8]);

impl<'a> In<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.0.split_at_checked(count)?;
        self.0 = rest;
        Some(taken)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1)?.first().copied()
    }

    fn word(&mut self) -> Option<u32> {
        word(self.take(4)?)
    }

    /// The rest is zeros.
    fn zeros(&self) -> bool {
        self.0.iter().all(|&byte| byte == 0)
    }
}

fn words(out: &mut [u8], words: &[u32]) {
    for (field, word) in out.chunks_exact_mut(4).zip(words) {
        field.copy_from_slice(&word.to_le_bytes());
    }
}

fn word(bytes: &[u8]) -> Option<u32> {
    bytes.try_into().ok().map(u32::from_le_bytes)
}

impl Entry {
    fn put(&self, record: &mut [u8]) {
        let Some((kind, body)) = record.split_first_mut() else { return };
        match self {
            Entry::Process(process) => {
                *kind = 0;
                words(body, &[process.free, process.largest_free, process.video_budget as u32, (process.video_budget >> 32) as u32]);
            }
            Entry::Arena(arena) => {
                *kind = 1;
                let (name, rest) = body.split_at_mut(NAME);
                name.copy_from_slice(&arena.name.bytes);
                let (size, rest) = rest.split_at_mut(4);
                words(size, &[arena.size]);
                let (usage, rest) = rest.split_at_mut(1);
                let (tag, figures) = match arena.usage {
                    Usage::Unknown => (0, [0, 0]),
                    Usage::Counted { used } => (1, [used, 0]),
                    Usage::Heap { used, largest_free } => (2, [used, largest_free]),
                };
                usage.copy_from_slice(&[tag]);
                words(rest, &figures);
            }
            Entry::Census(census) => {
                *kind = 2;
                let mut out = Out(body.iter_mut());
                for footprint in [census.image, census.mapped, census.private] {
                    [footprint.reserved, footprint.committed, footprint.allocations].into_iter().for_each(|w| out.word(w));
                }
            }
            Entry::Allocation(allocation) => {
                *kind = 3;
                let mut out = Out(body.iter_mut());
                out.word(allocation.base);
                out.bytes(&[allocation.owner.kind().byte()]);
                out.word(allocation.size);
                out.word(allocation.committed);
                out.bytes(&allocation.owner.name().map_or([0; NAME], |name| name.bytes));
            }
            Entry::Client(client) => {
                *kind = 4;
                let mut out = Out(body.iter_mut());
                let system = match client.system {
                    System::Windows => 0,
                    System::Wine => 1,
                };
                out.bytes(&[system, client.renderer.byte()]);
                let textures = client.textures;
                [textures.kept, textures.kept_bytes, textures.listed, textures.listed_bytes].into_iter().for_each(|w| out.word(w));
            }
        }
    }

    fn parse(record: &[u8]) -> Option<Entry> {
        let (&kind, body) = record.split_first()?;
        match kind {
            0 => {
                let (figures, zeros) = body.split_at_checked(16)?;
                let mut each = figures.chunks_exact(4).map(word);
                let (Some(Some(free)), Some(Some(largest_free)), Some(Some(low)), Some(Some(high))) = (each.next(), each.next(), each.next(), each.next()) else {
                    return None;
                };
                if zeros.iter().any(|&byte| byte != 0) || largest_free > free {
                    return None;
                }
                Some(Entry::Process(Process { free, largest_free, video_budget: u64::from(high) << 32 | u64::from(low) }))
            }
            1 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (size, rest) = rest.split_at_checked(4)?;
                let (&tag, figures) = rest.split_first()?;
                let (used, largest) = figures.split_at_checked(4)?;
                let (size, used, largest) = (word(size)?, word(used)?, word(largest)?);
                let usage = match (tag, used, largest) {
                    (0, 0, 0) => Usage::Unknown,
                    (1, used, 0) => Usage::Counted { used },
                    (2, used, largest_free) => Usage::Heap { used, largest_free },
                    _ => return None,
                };
                Some(Entry::Arena(Arena { name: Name::parse(name)?, size, usage }))
            }
            2 => {
                let mut body = In(body);
                let mut footprint = || Footprint::parse(body.word()?, body.word()?, body.word()?);
                let census = Census { image: footprint()?, mapped: footprint()?, private: footprint()? };
                body.zeros().then_some(Entry::Census(census))
            }
            3 => {
                let mut body = In(body);
                let (base, kind, size, committed) = (body.word()?, Type::of_byte(body.byte()?)?, body.word()?, body.word()?);
                let name = body.take(NAME)?;
                let name = if name.iter().all(|&byte| byte == 0) { None } else { Some(Name::parse(name)?) };
                let owner = match (kind, name) {
                    (Type::Image, name) => Owner::Image(name),
                    (Type::Mapped, name) => Owner::Mapped(name),
                    (Type::Private, None) => Owner::Private,
                    (Type::Private, Some(_)) => return None,
                };
                (committed <= size && size != 0 && body.zeros()).then_some(Entry::Allocation(Allocation { base, size, committed, owner }))
            }
            4 => {
                let mut body = In(body);
                let system = match body.byte()? {
                    0 => System::Windows,
                    1 => System::Wine,
                    _ => return None,
                };
                let renderer = Renderer::of_byte(body.byte()?)?;
                let textures = Textures { kept: body.word()?, kept_bytes: body.word()?, listed: body.word()?, listed_bytes: body.word()? };
                body.zeros().then_some(Entry::Client(Client { system, renderer, textures }))
            }
            _ => None,
        }
    }
}

const NOTHING: Entry = Entry::Process(Process { free: 0, largest_free: 0, video_budget: 0 });

/// A whole report, as the client gathers it: the process, then the
/// arenas in the engine's order, the census, the client and the largest
/// allocations.
pub struct Report {
    moment: Moment,
    entries: [Entry; MAX_ENTRIES],
    told: usize,
}

impl Report {
    pub fn new(moment: Moment, process: Process) -> Report {
        let mut entries = [NOTHING; MAX_ENTRIES];
        if let Some(first) = entries.first_mut() {
            *first = Entry::Process(process);
        }
        Report { moment, entries, told: 1 }
    }

    pub fn moment(&self) -> Moment {
        self.moment
    }

    /// The next arena; past [`MAX_ARENAS`] (more than the engine's list
    /// holds) it is not kept.
    pub fn add(&mut self, arena: Arena) {
        let arenas = self.entries().iter().filter(|entry| matches!(entry, Entry::Arena(_))).count();
        if arenas < MAX_ARENAS {
            self.push(Entry::Arena(arena));
        }
    }

    /// The address space's census.
    pub fn add_census(&mut self, census: Census) {
        self.push(Entry::Census(census));
    }

    /// What the game runs with.
    pub fn add_client(&mut self, client: Client) {
        self.push(Entry::Client(client));
    }

    /// The next of the largest allocations; past [`TOP`] it is not kept.
    pub fn add_allocation(&mut self, allocation: Allocation) {
        let told = self.entries().iter().filter(|entry| matches!(entry, Entry::Allocation(_))).count();
        if told < TOP {
            self.push(Entry::Allocation(allocation));
        }
    }

    fn push(&mut self, entry: Entry) {
        if let Some(place) = self.entries.get_mut(self.told) {
            *place = entry;
            self.told = self.told.saturating_add(1);
        }
    }

    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..self.told).unwrap_or_default()
    }

    /// How many parts it goes in.
    pub fn parts(&self) -> u8 {
        self.told.div_ceil(PER_PART) as u8
    }

    /// The body of part `part` (0-based) into `out`, and its length.
    pub(crate) fn put_part(&self, part: u8, out: &mut [u8]) -> Option<usize> {
        if part >= self.parts() {
            return None;
        }
        let start = usize::from(part).checked_mul(PER_PART)?;
        let entries = self.entries.get(start..self.told.min(start.checked_add(PER_PART)?))?;
        let (head, records) = out.split_at_mut_checked(HEAD)?;
        head.copy_from_slice(&[self.moment.byte(), entries.len() as u8]);
        let mut records = records.chunks_exact_mut(RECORD);
        for entry in entries {
            entry.put(records.next()?);
        }
        HEAD.checked_add(entries.len().checked_mul(RECORD)?)
    }
}

/// The bytes of the body that starts `bytes` (more may follow it), from
/// its count alone: 1 to [`PER_PART`] records after the moment and the
/// count. The rest of it is checked by [`Part::parse`].
pub(crate) fn body_len(bytes: &[u8]) -> Option<usize> {
    let &count = bytes.get(1)?;
    if count == 0 || usize::from(count) > PER_PART {
        return None;
    }
    HEAD.checked_add(usize::from(count).checked_mul(RECORD)?)
}

/// One part as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub moment: Moment,
    entries: [Entry; PER_PART],
    count: u8,
}

impl Part {
    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..usize::from(self.count)).unwrap_or_default()
    }

    /// Exactly a body: a moment, `count` records (full ones only in a part
    /// before the last, `full`), each valid; the process only first in the
    /// report (`first`), the other kinds after it.
    pub(crate) fn parse(bytes: &[u8], first: bool, full: bool) -> Option<Part> {
        let (head, records) = bytes.split_at_checked(HEAD)?;
        let &[moment, count] = head else { return None };
        let moment = Moment::of(moment)?;
        if count == 0 || usize::from(count) > PER_PART || (full && usize::from(count) != PER_PART) {
            return None;
        }
        if records.len() != usize::from(count).checked_mul(RECORD)? {
            return None;
        }
        let mut entries = [NOTHING; PER_PART];
        for (at, (record, entry)) in records.chunks_exact(RECORD).zip(&mut entries).enumerate() {
            *entry = Entry::parse(record)?;
            if matches!(entry, Entry::Process(_)) != (first && at == 0) {
                return None;
            }
        }
        Some(Part { moment, entries, count })
    }
}

/// A region of the address space, as `VirtualQuery` tells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    /// MEM_FREE.
    Free { size: u32 },
    /// MEM_RESERVE or MEM_COMMIT: part of the allocation that starts at
    /// `allocation`.
    Taken { allocation: u32, size: u32, committed: bool, kind: Type },
}

impl Region {
    /// A region of `VirtualQuery`'s words: its `State`, `Type`,
    /// `AllocationBase` and `RegionSize`; none for a state or a type it
    /// does not have.
    pub fn of(state: u32, kind: u32, allocation: u32, size: u32) -> Option<Region> {
        const COMMIT: u32 = 0x1000;
        const RESERVE: u32 = 0x2000;
        const FREE: u32 = 0x1_0000;
        match state {
            FREE => Some(Region::Free { size }),
            COMMIT | RESERVE => Some(Region::Taken { allocation, size, committed: state == COMMIT, kind: Type::of(kind)? }),
            _ => None,
        }
    }
}

/// An allocation the walk found, not named yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    base: u32,
    size: u32,
    committed: u32,
    kind: Type,
}

impl Found {
    pub fn base(self) -> u32 {
        self.base
    }

    pub fn kind(self) -> Type {
        self.kind
    }

    /// The allocation with its file's `name` (an image's, a mapping's; a
    /// private one has none, and is given none).
    pub fn named(self, name: Option<Name>) -> Allocation {
        let owner = match self.kind {
            Type::Image => Owner::Image(name),
            Type::Mapped => Owner::Mapped(name),
            Type::Private => Owner::Private,
        };
        Allocation { base: self.base, size: self.size, committed: self.committed, owner }
    }
}

/// The address space walked region by region from 0 (`VirtualQuery`), in
/// one pass: the free bytes and the largest free region, the [`Census`],
/// and the [`TOP`] largest allocations whose base is in none of the
/// arenas given (those the report tells already).
pub struct Walk {
    free: u32,
    largest_free: u32,
    census: Census,
    current: Option<Found>,
    /// The largest first.
    top: [Option<Found>; TOP],
    /// The arenas' spans, `[start, end)`.
    arenas: [Option<(u32, u32)>; MAX_ARENAS],
}

impl Walk {
    /// A walk whose largest allocations leave out the `arenas` (start,
    /// size; one of no bytes, the process heap's, leaves nothing out).
    pub fn new(arenas: impl IntoIterator<Item = (u32, u32)>) -> Walk {
        let mut spans = [None; MAX_ARENAS];
        for (slot, (start, size)) in spans.iter_mut().zip(arenas.into_iter().filter(|&(start, size)| start != 0 && size != 0)) {
            *slot = Some((start, start.saturating_add(size)));
        }
        Walk { free: 0, largest_free: 0, census: Census::default(), current: None, top: [None; TOP], arenas: spans }
    }

    /// The next region, in address order (none: one of no known state or
    /// type, counted nowhere).
    pub fn region(&mut self, region: Option<Region>) {
        match region {
            Some(Region::Taken { allocation, size, committed, kind }) => {
                let committed = if committed { size } else { 0 };
                match self.current.as_mut().filter(|found| found.base == allocation && found.kind == kind) {
                    Some(found) => {
                        found.size = found.size.saturating_add(size);
                        found.committed = found.committed.saturating_add(committed);
                    }
                    None => {
                        self.close();
                        self.current = Some(Found { base: allocation, size, committed, kind });
                        let footprint = self.census.of(kind);
                        footprint.allocations = footprint.allocations.saturating_add(1);
                    }
                }
                let footprint = self.census.of(kind);
                footprint.reserved = footprint.reserved.saturating_add(size);
                footprint.committed = footprint.committed.saturating_add(committed);
            }
            Some(Region::Free { size }) => {
                self.close();
                self.free = self.free.saturating_add(size);
                self.largest_free = self.largest_free.max(size);
            }
            None => self.close(),
        }
    }

    /// The allocation walked so far kept among the largest, unless it is
    /// an arena's.
    fn close(&mut self) {
        let Some(found) = self.current.take() else { return };
        if self.arenas.iter().flatten().any(|&(start, end)| (start..end).contains(&found.base)) {
            return;
        }
        let mut carry = Some(found);
        for slot in self.top.iter_mut() {
            match (*slot, carry) {
                (_, None) => break,
                (None, new) => {
                    *slot = new;
                    carry = None;
                }
                (Some(held), Some(new)) if new.size > held.size => {
                    *slot = Some(new);
                    carry = Some(held);
                }
                _ => {}
            }
        }
    }

    /// The walk's end.
    pub fn end(mut self) -> Walked {
        self.close();
        Walked { free: self.free, largest_free: self.largest_free, census: self.census, top: self.top }
    }
}

/// A walk of the whole address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walked {
    free: u32,
    largest_free: u32,
    census: Census,
    top: [Option<Found>; TOP],
}

impl Walked {
    pub fn process(&self, video_budget: u64) -> Process {
        Process { free: self.free, largest_free: self.largest_free, video_budget }
    }

    pub fn census(&self) -> Census {
        self.census
    }

    /// The largest allocations outside the arenas, the largest first.
    pub fn largest(&self) -> impl Iterator<Item = Found> + '_ {
        self.top.iter().flatten().copied()
    }
}

/// A region (`VirtualQuery`'s `State` and `Protect`) whose bytes can be
/// read: committed, readable, no guard page.
pub fn readable(state: u32, protect: u32) -> bool {
    const COMMIT: u32 = 0x1000;
    const GUARD: u32 = 0x100;
    state == COMMIT && protect & GUARD == 0 && matches!(protect & 0xFF, 0x02 | 0x04 | 0x08 | 0x20 | 0x40 | 0x80)
}

/// A dlmalloc 2.8 mspace's state (`malloc_state`, client_patch.md section
/// 3.76) as the walk needs it. An arena's mspace is made with its base
/// (`create_mspace_with_base`): one segment, the whole arena, its
/// footprint (and max_footprint, so no high-water mark) the arena's size
/// from the start, its never-used tail the top chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mspace {
    pub top: u32,
    pub top_size: u32,
    pub footprint: u32,
    pub segment: u32,
    pub segment_size: u32,
    /// The next segment (0: none).
    pub next_segment: u32,
}

impl Mspace {
    /// Chunks walked at most (1 Mi: a few milliseconds).
    pub const MAX_CHUNKS: u32 = 1 << 20;

    /// Its usage, as the engine's `allocated_size` (internal_mallinfo
    /// 0xFF200, less the mspace's own chunk) counts it, walked from its
    /// segment's first chunk to the top: `head` reads a chunk's head word
    /// (the word at its address + 4; `None` outside the arena). Another
    /// thread may use the heap meanwhile (the game's arenas have no lock):
    /// a walk that does not end at the top, the fence or the segment's end
    /// on a chunk's boundary, or meets a chunk too small to be one, is
    /// `None`. A walk that ends cleanly may still be a little stale: the
    /// state (top, topsize) is read before the walk, and another thread can
    /// move the top or split a chunk meanwhile.
    pub fn usage(self, head: impl FnMut(u32) -> Option<u32>) -> Option<Usage> {
        self.walk(head, |_| ())
    }

    /// [`Mspace::usage`], each free block's bytes told to `free` on the
    /// way (the top first, then each free chunk in address order; patch
    /// 127's holes). A walk that is `None` may have told some: the caller
    /// keeps them only with a figure.
    pub fn walk(self, mut head: impl FnMut(u32) -> Option<u32>, mut free_block: impl FnMut(u32)) -> Option<Usage> {
        const TOP_FOOT: u32 = 0x28;
        const STATE_CHUNK: u32 = 0x1E0;
        const FENCEPOST: u32 = 7;
        const IN_USE: u32 = 2;
        const MIN_CHUNK: u32 = 16;
        if self.next_segment != 0 {
            return None;
        }
        let end = self.segment.checked_add(self.segment_size)?;
        let mut chunk = self.segment.checked_add(self.segment.wrapping_neg() & 7)?;
        let mut free = self.top_size.checked_add(TOP_FOOT)?;
        let mut largest_free = self.top_size;
        free_block(self.top_size);
        let mut chunks = 0u32;
        while chunk < end && chunk != self.top {
            let word = head(chunk)?;
            if word == FENCEPOST {
                break;
            }
            let size = word & !7;
            if size < MIN_CHUNK {
                return None;
            }
            if word & IN_USE == 0 {
                free = free.checked_add(size)?;
                largest_free = largest_free.max(size);
                free_block(size);
            }
            chunk = chunk.checked_add(size)?;
            chunks = chunks.checked_add(1).filter(|&n| n <= Mspace::MAX_CHUNKS)?;
        }
        if chunk > end {
            return None;
        }
        let used = self.footprint.checked_sub(free)?.checked_sub(STATE_CHUNK)?;
        Some(Usage::Heap { used, largest_free })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn arena(name: &str, n: u32) -> Arena {
        let usage = match n % 3 {
            0 => Usage::Unknown,
            1 => Usage::Counted { used: n * 1000 },
            _ => Usage::Heap { used: n * 1000, largest_free: n * 7 },
        };
        Arena { name: Name::of_c(name.as_bytes()), size: 0x100_0000 + n, usage }
    }

    fn process() -> Process {
        Process { free: 0x2000_0000, largest_free: 0x0800_0000, video_budget: 0x1_6E2C_D108 }
    }

    #[test]
    fn names_are_printable_ascii_cut_and_never_empty() {
        assert_eq!(Name::of_c(b"render\0junk").as_str(), "render");
        assert_eq!(Name::of_c(b"resources (unmanaged) allocator").as_str(), "resources (unmanaged) allocator");
        assert_eq!(Name::of_c(b"resources (unmanaged) allocator and more").as_str().len(), NAME);
        assert_eq!(Name::of_c(b"a\x07b\xd0").as_str(), "a?b?");
        assert_eq!(Name::of_c(b"").as_str(), "?");
        assert_eq!(Name::of_c(b"\0render").as_str(), "?");
    }

    #[test]
    fn the_address_space_sums_free_regions() {
        let mut walk = Walk::new([]);
        for (state, size) in [(0x1000, 0x1000_0000), (0x1_0000, 0x0100_0000), (0x2000, 0x10), (0x1_0000, 0x0300_0000), (0x1_0000, 0x10_0000)] {
            walk.region(Region::of(state, 0x2_0000, 0x1000, size));
        }
        assert_eq!(walk.end().process(5), Process { free: 0x0410_0000, largest_free: 0x0300_0000, video_budget: 5 });
        assert!(readable(0x1000, 0x04) && readable(0x1000, 0x20) && readable(0x1000, 0x40));
        assert!(!readable(0x1000, 0x01) && !readable(0x1000, 0x104) && !readable(0x2000, 0x04) && !readable(0x1000, 0x10));
    }

    const COMMIT: u32 = 0x1000;
    const RESERVE: u32 = 0x2000;
    const FREE: u32 = 0x1_0000;
    const IMAGE: u32 = 0x100_0000;
    const MAPPED: u32 = 0x4_0000;
    const PRIVATE: u32 = 0x2_0000;

    #[test]
    fn a_walk_takes_the_census_and_the_largest_allocations_outside_the_arenas() {
        // (base, state, type, allocation base, size), in address order
        let space = [
            (0x0000_0000, FREE, 0, 0, 0x1_0000),
            (0x0001_0000, COMMIT, IMAGE, 0x1_0000, 0x1000),         // the exe: header,
            (0x0001_1000, COMMIT, IMAGE, 0x1_0000, 0x8F_0000),      // its sections
            (0x0090_1000, RESERVE, PRIVATE, 0x90_1000, 0x10_0000),  // an arena (reserved part)
            (0x00A0_1000, COMMIT, PRIVATE, 0x90_1000, 0x1000_0000), // ... its committed part
            (0x10A0_1000, COMMIT, MAPPED, 0x10A0_1000, 0x40_0000),  // a mapping
            (0x10E0_1000, RESERVE, MAPPED, 0x10A0_1000, 0x40_0000), // ... the rest of its view
            (0x1120_1000, COMMIT, PRIVATE, 0x1120_1000, 0x2_0000),  // a driver's, right after it
            (0x1122_1000, 0x8000, PRIVATE, 0x1122_1000, 0x1000),    // a state there is none of
            (0x1122_2000, COMMIT, 0x7, 0x1122_2000, 0x1000),        // a type there is none of
            (0x1122_3000, FREE, 0, 0, 0x100_0000),
            (0x1222_3000, RESERVE, PRIVATE, 0x1222_3000, 0x80_0000),
            (0x12A2_3000, COMMIT, IMAGE, 0x12A2_3000, 0x20_0000),   // d3d11.dll
        ];
        let mut walk = Walk::new([(0x90_1000, 0x1010_0000), (0, 0)]);
        for (_, state, kind, allocation, size) in space {
            walk.region(Region::of(state, kind, allocation, size));
        }
        let walked = walk.end();
        assert_eq!(walked.process(0), Process { free: 0x101_0000, largest_free: 0x100_0000, video_budget: 0 });
        let census = walked.census();
        assert_eq!((census.image.reserved(), census.image.committed(), census.image.allocations()), (0xAF_1000, 0xAF_1000, 2));
        assert_eq!((census.mapped.reserved(), census.mapped.committed(), census.mapped.allocations()), (0x80_0000, 0x40_0000, 1));
        assert_eq!(
            (census.private.reserved(), census.private.committed(), census.private.allocations()),
            (0x1010_0000 + 0x2_0000 + 0x80_0000, 0x1000_0000 + 0x2_0000, 3)
        );
        // the arena left out; the largest first
        let largest: Vec<(u32, u32, u32, Type)> = walked.largest().map(|found| {
            let allocation = found.named(None);
            (allocation.base(), allocation.size(), allocation.committed(), allocation.owner().kind())
        }).collect();
        assert_eq!(
            largest,
            [
                (0x1_0000, 0x8F_1000, 0x8F_1000, Type::Image),
                (0x10A0_1000, 0x80_0000, 0x40_0000, Type::Mapped), // closed first: an equal stays behind it
                (0x1222_3000, 0x80_0000, 0, Type::Private),
                (0x12A2_3000, 0x20_0000, 0x20_0000, Type::Image),
                (0x1120_1000, 0x2_0000, 0x2_0000, Type::Private),
            ]
        );
        // a private one is given no name
        let name = Name::of_file(b"x.dll");
        assert_eq!(walked.largest().last().map(|found| found.named(name).owner()), Some(Owner::Private));
    }

    #[test]
    fn a_walk_keeps_the_top_largest_of_many() {
        let mut walk = Walk::new([]);
        // 3 * TOP allocations of sizes in no order, each with a free gap
        let sizes: Vec<u32> = (0..3 * TOP as u32).map(|n| 0x1_0000 * (1 + (n * 37) % 61)).collect();
        let mut at = 0x1_0000u32;
        for &size in &sizes {
            walk.region(Region::of(COMMIT, PRIVATE, at, size));
            walk.region(Region::of(FREE, 0, 0, 0x1_0000));
            at += size + 0x1_0000;
        }
        let mut expected = sizes.clone();
        expected.sort_unstable_by(|a, b| b.cmp(a));
        expected.truncate(TOP);
        let told: Vec<u32> = walk.end().largest().map(|found| found.named(None).size()).collect();
        assert_eq!(told, expected);
    }

    #[test]
    fn file_names_and_folders() {
        assert_eq!(Name::of_file(b"C:\\Windows\\SysWOW64\\d3d11.dll\0junk"), Some(Name::of_c(b"d3d11.dll")));
        assert_eq!(Name::of_file(b"\\Device\\HarddiskVolume3\\Games\\resources.db"), Some(Name::of_c(b"resources.db")));
        assert_eq!(Name::of_file(b"plain.dll"), Some(Name::of_c(b"plain.dll")));
        assert_eq!(Name::of_file(b"C:\\folder\\"), None);
        assert_eq!(Name::of_file(b""), None);
        assert_eq!(Name::of_file(b"\0C:\\a.dll"), None);
        assert_eq!(Folder::of_path(b"C:\\Game\\game\\binaries\\x86\\dxvk\\d3d11.dll"), Folder::Dxvk);
        assert_eq!(Folder::of_path(b"C:\\Game\\game\\binaries\\x86\\DXVK\\d3d11.dll\0"), Folder::Dxvk);
        assert_eq!(Folder::of_path(b"C:\\Windows\\SysWOW64\\d3d11.dll"), Folder::System);
        assert_eq!(Folder::of_path(b"C:\\windows\\system32\\d3d11.dll"), Folder::System);
        assert_eq!(Folder::of_path(b"C:\\Game\\game\\binaries\\x86\\d3d11.dll"), Folder::Elsewhere);
        assert_eq!(Folder::of_path(b"d3d11.dll"), Folder::Elsewhere);
        assert_eq!(Folder::of_path(b""), Folder::Elsewhere);
    }

    #[test]
    fn every_renderer_has_its_byte() {
        let all = [Renderer::Unbound, Renderer::Direct3d(Folder::Dxvk), Renderer::Direct3d(Folder::System), Renderer::Direct3d(Folder::Elsewhere),
                   Renderer::Dxvk(Folder::Dxvk), Renderer::Dxvk(Folder::System), Renderer::Dxvk(Folder::Elsewhere)];
        for (byte, renderer) in all.into_iter().enumerate() {
            assert_eq!(renderer.byte(), byte as u8);
            assert_eq!(Renderer::of_byte(byte as u8), Some(renderer));
        }
        assert_eq!(Renderer::of_byte(7), None);
    }

    /// An arena as create_mspace_with_base leaves it at `base`, with
    /// chunks of `sizes` (in use when odd in the list's order) and the top.
    fn heap(base: u32, capacity: u32, sizes: &[(u32, bool)]) -> (Mspace, Vec<(u32, u32)>) {
        let mut heads = std::vec![(base, 0x1E3)];
        let mut at = base + 0x1E0;
        for &(size, used) in sizes {
            heads.push((at, size | 1 | if used { 2 } else { 0 }));
            at += size;
        }
        let top_size = base + capacity - at - 0x28;
        heads.push((at, top_size | 1));
        let mspace = Mspace { top: at, top_size, footprint: capacity, segment: base, segment_size: capacity, next_segment: 0 };
        (mspace, heads)
    }

    #[test]
    fn a_heap_walk_counts_as_the_engine_and_finds_the_largest_free_block() {
        let (mspace, heads) = heap(0x1000_0000, 0x10_0000, &[(0x100, true), (0x2_0000, false), (0x40, true), (0x18, false)]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        // the engine's used: the in-use chunks (the mspace's own chunk not)
        assert_eq!(mspace.usage(read), Some(Usage::Heap { used: 0x140, largest_free: mspace.top_size }));
        let (full, heads) = heap(0x1000_0000, 0x10_0000, &[(0xF_0000, true), (0x8000, false), (0x10, true)]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        assert_eq!(full.usage(read), Some(Usage::Heap { used: 0xF_0010, largest_free: 0x8000 }));
        // a fresh arena: nothing used, the top all of it
        let (fresh, heads) = heap(0x1000_0000, 0x10_0000, &[]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        assert_eq!(fresh.usage(read), Some(Usage::Heap { used: 0, largest_free: 0x10_0000 - 0x1E0 - 0x28 }));
    }

    #[test]
    fn a_walk_tells_each_free_block() {
        let (mspace, heads) = heap(0x1000_0000, 0x10_0000, &[(0x100, true), (0x2_0000, false), (0x40, true), (0x18, false)]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        let mut told = Vec::new();
        assert_eq!(mspace.walk(read, |bytes| told.push(bytes)), mspace.usage(read));
        assert_eq!(told, [mspace.top_size, 0x2_0000, 0x18]);
    }

    #[test]
    fn a_heap_changed_under_the_walk_is_no_figure() {
        let (mspace, heads) = heap(0x1000_0000, 0x10_0000, &[(0x100, true), (0x2_0000, false), (0x40, true)]);
        let torn = |bad: u32| { let heads = heads.clone(); move |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(chunk, head)| if chunk == 0x1000_0000 + 0x1E0 + 0x100 { bad } else { head }) };
        // a head read as zero (a freed chunk's data), a size past the arena, a size off a boundary
        assert_eq!(mspace.usage(torn(0)), None);
        assert_eq!(mspace.usage(torn(0x8000_0000)), None);
        assert_eq!(mspace.usage(torn(0x2_0008)), None);
        assert_eq!(mspace.usage(|_| None), None);
        assert_eq!(Mspace { next_segment: 1, ..mspace }.usage(|_| Some(3)), None);
        // a walk that never ends at the top is cut
        let endless = Mspace { top: 1, segment_size: 0xFFFF_0000, segment: 0, top_size: 0, footprint: 0xFFFF_0000, next_segment: 0 };
        assert_eq!(endless.usage(|_| Some(16 | 3)), None);
    }

    fn census() -> Census {
        let mut walk = Walk::new([]);
        for (state, kind, allocation, size) in [(0x1000, 0x100_0000, 0x1_0000, 0x90_0000), (0x2000, 0x4_0000, 0x100_0000, 0x10_0000),
                                                (0x1000, 0x2_0000, 0x200_0000, 0x2000), (0x2000, 0x2_0000, 0x200_0000, 0x1_E000)] {
            walk.region(Region::of(state, kind, allocation, size));
        }
        walk.end().census()
    }

    fn client() -> Client {
        Client {
            system: System::Wine,
            renderer: Renderer::Dxvk(Folder::System),
            textures: Textures { kept: 41, kept_bytes: 0x0123_4567, listed: 300, listed_bytes: 0x1234_5678 },
        }
    }

    fn allocation(n: u32) -> Allocation {
        let kind = Type::of_byte((n % 3) as u8).unwrap();
        let found = Found { base: 0x1000_0000 + (n << 16), size: 0x10_0000 - n, committed: 0x1000 * n, kind };
        found.named(if n.is_multiple_of(2) { Name::of_file(std::format!("C:\\dir\\file {n}.dll").as_bytes()) } else { None })
    }

    #[test]
    fn a_report_goes_in_parts_and_back() {
        for (arenas, allocations) in [(0, 0), (1, 1), (9, 0), (10, 3), (16, TOP), (17, TOP + 2), (MAX_ARENAS, TOP), (MAX_ARENAS + 3, TOP + 5)] {
            let mut report = Report::new(Moment::Ended, process());
            let all: Vec<Arena> = (0..arenas as u32).map(|n| arena(&std::format!("arena {n}"), n)).collect();
            all.iter().for_each(|&a| report.add(a));
            report.add_census(census());
            report.add_client(client());
            let largest: Vec<Allocation> = (0..allocations as u32).map(allocation).collect();
            largest.iter().for_each(|&a| report.add_allocation(a));
            assert!(report.parts() <= MAX_PARTS);
            let mut heard = Vec::new();
            for part in 0..report.parts() {
                let mut out = [0; MAX_BODY];
                let len = report.put_part(part, &mut out).unwrap();
                let full = part + 1 < report.parts();
                let read = Part::parse(&out[..len], part == 0, full).unwrap();
                assert_eq!(read.moment, Moment::Ended);
                heard.extend_from_slice(read.entries());
                assert_eq!(Part::parse(&out[..len], part != 0, full), None, "the process is first, and only there");
                assert_eq!(Part::parse(&out[..len - 1], part == 0, full), None);
            }
            assert_eq!(report.put_part(report.parts(), &mut [0; MAX_BODY]), None);
            let mut told = std::vec![Entry::Process(process())];
            told.extend(all.iter().take(MAX_ARENAS).map(|&a| Entry::Arena(a)));
            told.extend([Entry::Census(census()), Entry::Client(client())]);
            told.extend(largest.iter().take(TOP).map(|&a| Entry::Allocation(a)));
            assert_eq!(heard, told);
            assert_eq!(report.entries(), &told[..]);
        }
    }

    #[test]
    fn a_body_is_exact() {
        let mut report = Report::new(Moment::Loaded, process());
        (0..3).for_each(|n| report.add(arena("render", n)));
        let mut out = [0; MAX_BODY];
        let len = report.put_part(0, &mut out).unwrap();
        let body = &out[..len];
        let bad = |at: usize, value: u8| {
            let mut bytes = body.to_vec();
            bytes[at] = value;
            Part::parse(&bytes, true, false)
        };
        assert!(Part::parse(body, true, false).is_some());
        assert_eq!(bad(0, 2), None, "a moment");
        assert_eq!(bad(1, 0), None, "no entries");
        assert_eq!(bad(1, 5), None, "a count");
        assert_eq!(Part::parse(body, true, true), None, "a part before the last is full");
        let process_at = HEAD;
        assert_eq!(bad(process_at, 9), None, "a kind");
        assert_eq!(bad(process_at + 1 + 16, 1), None, "the process's zeros");
        assert_eq!(bad(process_at + 1 + 7, 0x30), None, "a largest free region over the free total");
        let unknown_at = HEAD + RECORD;
        let counted_at = HEAD + 2 * RECORD;
        assert_eq!(bad(unknown_at + 1, 0), None, "an empty name");
        assert_eq!(bad(unknown_at + 1 + 3, 0x80), None, "a name that is no ASCII");
        assert_eq!(bad(unknown_at + 1 + 10, b'x'), None, "bytes after the name's end");
        assert_eq!(bad(unknown_at + 1 + NAME + 4, 3), None, "a usage");
        assert_eq!(bad(unknown_at + 1 + NAME + 5, 1), None, "an unknown usage's figures");
        assert_eq!(bad(counted_at + 1 + NAME + 9, 1), None, "a count's largest free block");
    }

    #[test]
    fn the_new_kinds_are_exact() {
        let mut report = Report::new(Moment::Loaded, process());
        report.add_census(census());
        report.add_client(client());
        report.add_allocation(allocation(0));
        report.add_allocation(allocation(1));
        report.add_allocation(allocation(2));
        let mut out = [0; MAX_BODY];
        let len = report.put_part(0, &mut out).unwrap();
        let body = &out[..len];
        let bad = |at: usize, value: u8| {
            let mut bytes = body.to_vec();
            bytes[at] = value;
            Part::parse(&bytes, true, false)
        };
        let read = Part::parse(body, true, false).unwrap();
        assert_eq!(read.entries()[1], Entry::Census(census()));
        assert_eq!(read.entries()[2], Entry::Client(client()));
        assert_eq!(read.entries()[3], Entry::Allocation(allocation(0)));
        assert_eq!(allocation(0).owner(), Owner::Image(Name::of_file(b"file 0.dll")));
        assert_eq!(allocation(1).owner(), Owner::Mapped(None));
        assert_eq!(allocation(2).owner(), Owner::Private);
        let (census_at, client_at) = (HEAD + RECORD, HEAD + 2 * RECORD);
        let (image_at, mapped_at, private_at) = (HEAD + 3 * RECORD, HEAD + 4 * RECORD, HEAD + 5 * RECORD);
        // census: committed over reserved; bytes in no allocation; bytes after it
        assert_eq!(bad(census_at + 1 + 4 + 3, 0xFF), None, "image committed over reserved");
        assert_eq!(bad(census_at + 1 + 12 + 8, 0), None, "mapped bytes in no allocation");
        assert_eq!(bad(census_at + 1 + 36, 1), None, "the census's zeros");
        // client: a system, a renderer, its zeros
        assert_eq!(bad(client_at + 1, 2), None, "a system");
        assert_eq!(bad(client_at + 2, 7), None, "a renderer");
        assert!(bad(client_at + 2, 0).is_some(), "unbound");
        assert_eq!(bad(client_at + 1 + 18, 1), None, "the client's zeros");
        // allocations: a type, committed over size, no size, a name
        assert_eq!(bad(image_at + 1 + 4, 3), None, "a type");
        assert_eq!(bad(mapped_at + 1 + 9 + 3, 0xFF), None, "committed over size");
        let mut sizeless = body.to_vec();
        sizeless[image_at + 1 + 5..image_at + 1 + 13].fill(0);
        assert_eq!(Part::parse(&sizeless, true, false), None, "no size");
        assert_eq!(bad(private_at + 1 + 13, b'a'), None, "a private allocation's name");
        assert!(bad(mapped_at + 1 + 13, b'a').is_some(), "a mapping's name");
        assert_eq!(bad(image_at + 1 + 13 + 2, 0x7F), None, "a name that is no ASCII");
    }
}
