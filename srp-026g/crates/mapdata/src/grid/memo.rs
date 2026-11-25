//! [incremental grids] The job memo: each heavy `parallel` site of the grid build keeps its jobs' results on disk, keyed by
//! everything a job reads, so a rebuild runs only the jobs whose inputs changed and takes the rest from the memo. Its output is
//! then the full build's: a job given the same inputs gives the same answer (`controlled::parallel`).
//!
//! A job's key ([`Memo::run`]): the site's id and source digest ([`Site`]), the memo's global inputs (the world, the body: what
//! every site reads whole), the site's context ([`MemoCtx`]: each field hashed whole), and the item's own part, which the site
//! gives: the item in stable form ([`StableIds`]: a floor named by its cell and height, never by its index, which the build's
//! erosion and packing renumber) and the digest of the grid state the job may read around it ([`Windows`]: a box of cells).
//!
//! Three guards against a key that misses an input:
//! - the job is a function pointer, `fn(&C, &T) -> R`, taking only its context and its item: it captures nothing the key does
//!   not see. A context is made by [`memo_ctx!`], which hashes every field it declares;
//! - [`Mode::Verify`] runs again a share of the hits and stops the build at the first result that differs ([`Memo::run`]);
//! - a stored result naming a floor the grid does not have is a miss and an error ([`MemoCodec::decode`]).
//!
//! Storage: under the memo's folder, one segment file per site and build that had misses, `<site>.<stamp>.seg`: the header
//! (magic `SRPMEMO` and a zero byte, version, the site's id, the result's tag), records (a 1 byte, key u128, length u32,
//! bytes), the end (a 0 byte, magic `END` and a zero byte, the count, the digest of the records). Written to a name of its own and renamed in; a segment whose end does not check is
//! left out whole, never half read. Nothing here deletes a file.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{NodeId, WalkGrid};
use crate::digest::Digest128;

const MAGIC: &[u8; 8] = b"SRPMEMO\0";
const END: &[u8; 4] = b"END\0";
/// The segment format's version: a reader leaves out a segment of another.
pub const VERSION: u32 = 1;

/// How a build uses the memo (`SRP_GRID_MEMO`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// No memo: the build as it was (the reference).
    Off,
    /// Hits are taken, misses run and stored.
    Use,
    /// As [`Mode::Use`], and of the hits `per_million` in a million (chosen by their keys) run again and compared: a difference
    /// stops the build. A million is a full build that also checks every stored result.
    Verify { per_million: u32 },
    /// Every job runs and is stored (a new memo's first build).
    WriteOnly,
}

impl Mode {
    /// `off`, `use`, `verify=<share 0..1>`, `write-only`.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "" | "off" => Ok(Mode::Off),
            "use" => Ok(Mode::Use),
            "write-only" => Ok(Mode::WriteOnly),
            other => {
                let share = other
                    .strip_prefix("verify=")
                    .and_then(|p| p.parse::<f64>().ok())
                    .filter(|p| (0.0..=1.0).contains(p))
                    .ok_or_else(|| format!("SRP_GRID_MEMO={other}: not off, use, verify=<0..1> or write-only"))?;
                Ok(Mode::Verify { per_million: (share * 1e6).round() as u32 })
            }
        }
    }
}

/// A floor named by what it is, not by its index: its cell and its height's bits. A column's floors have distinct heights
/// (its spans are merged), which [`StableIds::of`] checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StableId {
    pub cell: u32,
    pub height: u32,
}

/// The floors of a grid as it is at a site: index to [`StableId`] and back.
pub struct StableIds {
    ids: Vec<StableId>,
    nodes: HashMap<StableId, NodeId>,
}

impl StableIds {
    /// The grid's floors' ids; none when two floors of a cell share a height (then no floor has a stable name, and the memo is
    /// off for the site).
    pub fn of(grid: &WalkGrid) -> Option<Self> {
        let ids: Vec<StableId> =
            (0..grid.height.len()).map(|a| StableId { cell: grid.cell_of[a], height: grid.height[a].to_bits() }).collect();
        let mut nodes = HashMap::with_capacity(ids.len());
        for (a, &id) in ids.iter().enumerate() {
            if nodes.insert(id, a as NodeId).is_some() {
                return None;
            }
        }
        Some(Self { ids, nodes })
    }

    pub fn id(&self, node: NodeId) -> StableId {
        self.ids[node as usize]
    }

    pub fn node(&self, id: StableId) -> Option<NodeId> {
        self.nodes.get(&id).copied()
    }
}

/// What a context field gives its key: everything it holds that a job may read.
pub trait MemoInput {
    fn feed(&self, digest: &mut Digest128);
}

macro_rules! plain_input {
    ($($t:ty => |$v:ident, $d:ident| $body:expr;)*) => {
        $(impl MemoInput for $t {
            fn feed(&self, $d: &mut Digest128) {
                let $v = *self;
                $body
            }
        })*
    };
}

plain_input! {
    u8 => |v, d| d.u32(u32::from(v));
    u32 => |v, d| d.u32(v);
    u64 => |v, d| d.u64(v);
    u128 => |v, d| d.u128(v);
    usize => |v, d| d.u64(v as u64);
    bool => |v, d| d.u32(u32::from(v));
    f32 => |v, d| d.u32(v.to_bits());
    i32 => |v, d| d.u32(v as u32);
    i64 => |v, d| d.u64(v as u64);
}

impl<T: MemoInput> MemoInput for [T] {
    fn feed(&self, digest: &mut Digest128) {
        digest.u64(self.len() as u64);
        self.iter().for_each(|v| v.feed(digest));
    }
}

impl<T: MemoInput> MemoInput for Vec<T> {
    fn feed(&self, digest: &mut Digest128) {
        self.as_slice().feed(digest);
    }
}

impl<T: MemoInput> MemoInput for Option<T> {
    fn feed(&self, digest: &mut Digest128) {
        match self {
            None => digest.u32(0),
            Some(v) => {
                digest.u32(1);
                v.feed(digest);
            }
        }
    }
}

impl<T: MemoInput + ?Sized> MemoInput for &T {
    fn feed(&self, digest: &mut Digest128) {
        (**self).feed(digest);
    }
}

impl<A: MemoInput, B: MemoInput> MemoInput for (A, B) {
    fn feed(&self, digest: &mut Digest128) {
        self.0.feed(digest);
        self.1.feed(digest);
    }
}

/// [incremental grids PR 5] The flights' walk caps ([`crate::controller::jump::Kits`]): both, whole.
impl MemoInput for crate::controller::jump::Kits {
    fn feed(&self, digest: &mut Digest128) {
        digest.u32(self.body.to_bits());
        digest.u32(self.fastest.to_bits());
    }
}

impl MemoInput for StableId {
    fn feed(&self, digest: &mut Digest128) {
        digest.u32(self.cell);
        digest.u32(self.height);
    }
}

/// [incremental grids] Every field of the body, by name: written as a pattern with no `..`, so a field added to [`Body`] does
/// not compile until it is in the memo's keys (the grid key, [`crate::grid_key_with_links`], covers the switches and the values
/// it was made for; a memo key covers them all, the game data's [`Body::fastest_walk_cap`] too).
impl MemoInput for super::Body {
    fn feed(&self, digest: &mut Digest128) {
        let super::Body {
            step,
            stand_height,
            crouch_height,
            radius,
            min_ground_normal_y,
            max_drop,
            max_jump,
            drop_flight_mps,
            erosion,
            walk_off_lips,
            landing_carry,
            keep_last_resorts,
            crouch_only_floors,
            capsule_steps,
            drop_room,
            slip_landings,
            run_ups_after_erosion,
            steep_walk_base,
            pocket_exits_walled,
            press_window,
            exits_in_reach,
            gap_no_vaults,
            ramp_faces,
            client_controller,
            drop_room_walk_faces,
            narrow_doors,
            drop_check,
            walk_ceiling_stalls,
            rim_jumps,
            fastest_walk_cap,
            wall_held_tops,
            drop_lands,
            gap_flights,
            rest_floors,
            held_floors,
            walk_face_stands,
            wall_give_back,
            exe_collision,
            walk_start_clear,
            crack_bridges,
            start_witness,
            give_back_in_cell,
            fall_stops_on_solids,
            walk_every_link,
            jump_air_steps,
        } = *self;
        digest.str(b"step");
        step.feed(digest);
        digest.str(b"stand_height");
        stand_height.feed(digest);
        digest.str(b"crouch_height");
        crouch_height.feed(digest);
        digest.str(b"radius");
        radius.feed(digest);
        digest.str(b"min_ground_normal_y");
        min_ground_normal_y.feed(digest);
        digest.str(b"max_drop");
        max_drop.feed(digest);
        digest.str(b"max_jump");
        max_jump.feed(digest);
        digest.str(b"drop_flight_mps");
        drop_flight_mps.feed(digest);
        digest.str(b"erosion");
        digest.str(format!("{erosion:?}").as_bytes());
        digest.str(b"walk_off_lips");
        walk_off_lips.feed(digest);
        digest.str(b"landing_carry");
        landing_carry.feed(digest);
        digest.str(b"keep_last_resorts");
        keep_last_resorts.feed(digest);
        digest.str(b"crouch_only_floors");
        crouch_only_floors.feed(digest);
        digest.str(b"capsule_steps");
        capsule_steps.feed(digest);
        digest.str(b"drop_room");
        drop_room.feed(digest);
        digest.str(b"slip_landings");
        slip_landings.feed(digest);
        digest.str(b"run_ups_after_erosion");
        run_ups_after_erosion.feed(digest);
        digest.str(b"steep_walk_base");
        steep_walk_base.feed(digest);
        digest.str(b"pocket_exits_walled");
        pocket_exits_walled.feed(digest);
        digest.str(b"press_window");
        press_window.feed(digest);
        digest.str(b"exits_in_reach");
        exits_in_reach.feed(digest);
        digest.str(b"gap_no_vaults");
        gap_no_vaults.feed(digest);
        digest.str(b"ramp_faces");
        ramp_faces.feed(digest);
        digest.str(b"client_controller");
        client_controller.feed(digest);
        digest.str(b"drop_room_walk_faces");
        drop_room_walk_faces.feed(digest);
        digest.str(b"narrow_doors");
        narrow_doors.feed(digest);
        digest.str(b"drop_check");
        drop_check.feed(digest);
        digest.str(b"walk_ceiling_stalls");
        walk_ceiling_stalls.feed(digest);
        digest.str(b"rim_jumps");
        rim_jumps.feed(digest);
        digest.str(b"fastest_walk_cap");
        fastest_walk_cap.feed(digest);
        digest.str(b"wall_held_tops");
        wall_held_tops.feed(digest);
        digest.str(b"drop_lands");
        drop_lands.feed(digest);
        digest.str(b"gap_flights");
        gap_flights.feed(digest);
        digest.str(b"rest_floors");
        rest_floors.feed(digest);
        digest.str(b"held_floors");
        held_floors.feed(digest);
        digest.str(b"walk_face_stands");
        walk_face_stands.feed(digest);
        digest.str(b"wall_give_back");
        wall_give_back.feed(digest);
        digest.str(b"exe_collision");
        exe_collision.feed(digest);
        digest.str(b"walk_start_clear");
        walk_start_clear.feed(digest);
        digest.str(b"crack_bridges");
        crack_bridges.feed(digest);
        digest.str(b"start_witness");
        start_witness.feed(digest);
        digest.str(b"give_back_in_cell");
        give_back_in_cell.feed(digest);
        digest.str(b"fall_stops_on_solids");
        fall_stops_on_solids.feed(digest);
        digest.str(b"walk_every_link");
        walk_every_link.feed(digest);
        digest.str(b"jump_air_steps");
        jump_air_steps.feed(digest);
    }
}

/// A site's context: what its jobs read besides their item, each field hashed whole into every key of the site. Made by
/// [`memo_ctx!`] (which lists every field in the digest), so a field added later is in the key with no line to forget.
pub trait MemoCtx: Sync {
    fn feed(&self, digest: &mut Digest128);
}

/// A context struct and its [`MemoCtx`]: `memo_ctx! { pub struct RimCtx<'a> { kits: &'a Kits, leads: u32 } }`. Every field's
/// type implements [`MemoInput`] (it does not compile otherwise). The grid state a job reads near its item is not a field: it is
/// the item's window ([`Windows`]), in the key the site's `key` function gives.
///
/// [incremental grids PR 5] A second block, `through { grid: &'a WalkGrid, world: &'a Collision }`, declares the fields a job
/// reads only through what the key already holds, never hashed whole: the grid (read in the item's window), the world, the
/// body and the tick's settings (the memo's global inputs, [`Memo::new`]). Each such field is named in the struct's
/// declaration, so a reader sees what the key leaves to the window and the global part.
#[macro_export]
macro_rules! memo_ctx {
    ($(#[$meta:meta])* $vis:vis struct $name:ident $(<$lt:lifetime>)? { $($fvis:vis $field:ident : $ty:ty),* $(,)? }) => {
        $crate::memo_ctx! { $(#[$meta])* $vis struct $name $(<$lt>)? { $($fvis $field: $ty),* } through {} }
    };
    ($(#[$meta:meta])* $vis:vis struct $name:ident $(<$lt:lifetime>)? { $($fvis:vis $field:ident : $ty:ty),* $(,)? }
        through { $($tvis:vis $through:ident : $tty:ty),* $(,)? }) => {
        $(#[$meta])*
        $vis struct $name $(<$lt>)? { $($fvis $field: $ty,)* $($tvis $through: $tty,)* }
        impl $(<$lt>)? $crate::grid::memo::MemoCtx for $name $(<$lt>)? {
            fn feed(&self, digest: &mut $crate::digest::Digest128) {
                $(
                    digest.str(stringify!($field).as_bytes());
                    $crate::grid::memo::MemoInput::feed(&self.$field, digest);
                )*
                // The fields read through the window and the global inputs: their names only.
                $(digest.str(stringify!($through).as_bytes());)*
            }
        }
    };
}

/// A job's result as the memo stores it. A floor in it is stored as its [`StableId`] ([`Codec::node`]) and read back as the
/// grid's index of that floor at the site; one the grid does not have makes the read fail (`None`): a miss, counted as an error.
pub trait MemoCodec: Sized {
    /// The result type's tag in a segment's header: a segment of another type is left out.
    const TAG: u32;
    fn encode(&self, out: &mut Codec<'_>);
    fn decode(input: &mut Codec<'_>) -> Option<Self>;
}

/// The bytes of one result being written or read, with the site's floors for the indices in it.
pub struct Codec<'a> {
    ids: &'a StableIds,
    bytes: Vec<u8>,
    at: usize,
}

impl<'a> Codec<'a> {
    pub(in crate::grid) fn writer(ids: &'a StableIds) -> Self {
        Self { ids, bytes: Vec::new(), at: 0 }
    }

    pub(in crate::grid) fn reader(ids: &'a StableIds, bytes: &[u8]) -> Self {
        Self { ids, bytes: bytes.to_vec(), at: 0 }
    }

    pub fn put_u8(&mut self, v: u8) {
        self.bytes.push(v);
    }

    pub fn put_u32(&mut self, v: u32) {
        self.bytes.extend(v.to_le_bytes());
    }

    pub fn put_f32(&mut self, v: f32) {
        self.put_u32(v.to_bits());
    }

    /// A floor, as its stable id.
    pub fn put_node(&mut self, node: NodeId) {
        let id = self.ids.id(node);
        self.put_u32(id.cell);
        self.put_u32(id.height);
    }

    pub fn u8(&mut self) -> Option<u8> {
        let v = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(v)
    }

    pub fn u32(&mut self) -> Option<u32> {
        let bytes = self.bytes.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    pub fn f32(&mut self) -> Option<f32> {
        self.u32().map(f32::from_bits)
    }

    /// A floor stored by [`Self::put_node`], as this grid's index; none when the grid has no such floor.
    pub fn node(&mut self) -> Option<NodeId> {
        let cell = self.u32()?;
        let height = self.u32()?;
        self.ids.node(StableId { cell, height })
    }

    /// [incremental grids PR 5] The bytes written (a site's tests).
    #[cfg(test)]
    pub(in crate::grid) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub(in crate::grid) fn done(&self) -> bool {
        self.at == self.bytes.len()
    }
}

impl MemoCodec for bool {
    const TAG: u32 = 1;
    fn encode(&self, out: &mut Codec<'_>) {
        out.put_u8(u8::from(*self));
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        match input.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
}

impl<T: MemoCodec> MemoCodec for Option<T> {
    const TAG: u32 = 0x100 ^ T::TAG.rotate_left(8);
    fn encode(&self, out: &mut Codec<'_>) {
        match self {
            None => out.put_u8(0),
            Some(v) => {
                out.put_u8(1);
                v.encode(out);
            }
        }
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        match input.u8()? {
            0 => Some(None),
            1 => Some(Some(T::decode(input)?)),
            _ => None,
        }
    }
}

impl<T: MemoCodec> MemoCodec for Vec<T> {
    const TAG: u32 = 0x200 ^ T::TAG.rotate_left(8);
    fn encode(&self, out: &mut Codec<'_>) {
        out.put_u32(self.len() as u32);
        self.iter().for_each(|v| v.encode(out));
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        let n = input.u32()? as usize;
        (0..n).map(|_| T::decode(input)).collect()
    }
}

/// The grid state around each cell, summed over boxes in O(1): each cell's state digest ([`Windows::new`]'s `state`, with the
/// cell's index in it, so a state moved to another cell digests otherwise), added with wrapping into 2-D prefix sums.
pub struct Windows {
    nx: usize,
    nz: usize,
    /// `(nx + 1) * (nz + 1)` sums: `sums[(k + 1) * (nx + 1) + i + 1]` is the sum over cells `0..=i`, `0..=k`.
    sums: Vec<u128>,
}

impl Windows {
    /// `state(cell)`: the digest of what a job may read of the grid in that cell (its floors, their links, side tables), each
    /// floor by its stable id. A cell with nothing to read gives 0.
    pub fn new(nx: usize, nz: usize, state: impl Fn(usize) -> u128) -> Self {
        let w = nx + 1;
        let mut sums = vec![0u128; w * (nz + 1)];
        for k in 0..nz {
            let mut row = 0u128;
            for i in 0..nx {
                let cell = k * nx + i;
                let s = state(cell);
                let h = if s == 0 {
                    0
                } else {
                    let mut d = Digest128::default();
                    d.u64(cell as u64);
                    d.u128(s);
                    d.finish()
                };
                row = row.wrapping_add(h);
                sums[(k + 1) * w + i + 1] = sums[k * w + i + 1].wrapping_add(row);
            }
        }
        Self { nx, nz, sums }
    }

    /// The digest of the cells `i0..=i1`, `k0..=k1`, clamped to the grid (empty: 0), with the box itself in it.
    pub fn of_box(&self, i0: i64, k0: i64, i1: i64, k1: i64) -> u128 {
        let clamp = |v: i64, n: usize| v.clamp(0, n as i64 - 1) as usize;
        if self.nx == 0 || self.nz == 0 || i1 < 0 || k1 < 0 || i0 >= self.nx as i64 || k0 >= self.nz as i64 || i0 > i1 || k0 > k1 {
            return 0;
        }
        let (i0, i1, k0, k1) = (clamp(i0, self.nx), clamp(i1, self.nx), clamp(k0, self.nz), clamp(k1, self.nz));
        let w = self.nx + 1;
        let at = |i: usize, k: usize| self.sums[k * w + i];
        let sum = at(i1 + 1, k1 + 1).wrapping_sub(at(i0, k1 + 1)).wrapping_sub(at(i1 + 1, k0)).wrapping_add(at(i0, k0));
        let mut d = Digest128::default();
        for v in [i0, k0, i1, k1] {
            d.u64(v as u64);
        }
        d.u128(sum);
        d.finish()
    }

    /// [incremental grids PR 5] The windows of the floors themselves: per cell, its floors in order (their count, and each one's
    /// height, the lowest point of its `walk` faces and its ceiling, as bits): what [`WalkGrid::floor_under`],
    /// [`WalkGrid::can_stand`], [`WalkGrid::own_top`] and [`WalkGrid::position`] read. Links and side tables are not in it: a
    /// site whose job reads them puts them in its item or in windows of its own.
    pub fn of_floors(grid: &WalkGrid) -> Self {
        Self::new(grid.nx, grid.nz, |cell| {
            let floors = grid.start[cell] as usize..grid.start[cell + 1] as usize;
            if floors.is_empty() {
                return 0;
            }
            let mut d = Digest128::default();
            d.u64(floors.len() as u64);
            for f in floors {
                d.u32(grid.height[f].to_bits());
                d.u32(grid.walk_low.get(f).copied().unwrap_or(grid.height[f]).to_bits());
                d.u32(grid.ceiling[f].to_bits());
            }
            d.finish()
        })
    }

    /// The box of `radius` cells around a cell index.
    pub fn around(&self, cell: usize, radius: i64) -> u128 {
        let (i, k) = ((cell % self.nx.max(1)) as i64, (cell / self.nx.max(1)) as i64);
        self.of_box(i - radius, k - radius, i + radius, k + radius)
    }
}

/// A site: its id (fixed, one a call: `"rim.line"`) and the digest of its job's source (`mapdata::source`).
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub id: &'static str,
    pub source: u128,
}

/// One site's counts in a build, for the log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteStats {
    pub site: &'static str,
    pub items: usize,
    pub hits: usize,
    pub misses: usize,
    /// Hits run again and found equal ([`Mode::Verify`]).
    pub verified: usize,
    /// Stored results that named a floor the grid does not have, or did not read back: run as misses.
    pub errors: usize,
    /// Results written (a new segment), and its bytes.
    pub written: usize,
    pub bytes: u64,
    /// Entries the site's segments held, and the segments left out (bad end, another version or type).
    pub loaded: usize,
    pub segments_refused: usize,
    /// Off for this site: no stable floor names ([`StableIds::of`]).
    pub off: bool,
}

/// A build's memo: its mode, its folder, the inputs every site reads whole, and each site's counts.
pub struct Memo {
    mode: Mode,
    dir: PathBuf,
    global: u128,
    pub stats: Vec<SiteStats>,
}

impl Memo {
    /// The memo of the map whose segments are in `dir` (made when missing), `global` the digest of what every job reads whole
    /// (the world, the body, the tick's settings).
    pub fn new(mode: Mode, dir: PathBuf, global: u128) -> Self {
        Self { mode, dir, global, stats: Vec::new() }
    }

    /// No memo: every site runs `parallel` itself ([`Mode::Off`]), a build as it was.
    pub fn off() -> Self {
        Self::new(Mode::Off, PathBuf::new(), 0)
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// `job` over `items` on the machine's threads ([`super::controlled::parallel`]), each result taken from the memo where its key
    /// is there. `key` gives an item's own part of its key (the item in stable form and its window). With [`Mode::Off`], or a
    /// grid with no stable floor names, `parallel` itself.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn run<C, T, R>(
        &mut self,
        site: Site,
        grid: &WalkGrid,
        ctx: &C,
        items: &[T],
        key: fn(&C, &StableIds, &T) -> u128,
        job: fn(&C, &T) -> R,
    ) -> Vec<R>
    where
        C: MemoCtx,
        T: Sync,
        R: MemoCodec + Send + Sync + Default + Clone,
    {
        let mut stats = SiteStats { site: site.id, items: items.len(), ..SiteStats::default() };
        let ids = match (self.mode, StableIds::of(grid)) {
            (Mode::Off, _) => None,
            (_, None) => {
                stats.off = true;
                None
            }
            (_, Some(ids)) => Some(ids),
        };
        let Some(ids) = ids else {
            stats.misses = items.len();
            self.stats.push(stats);
            // [srchash oracle] The site's jobs, for the coverage check of its digest (`crate::coverage`).
            return crate::coverage::window(site.id, || super::controlled::parallel(items, |item| job(ctx, item)));
        };
        // The site's part of every key.
        let mut head = Digest128::default();
        head.str(site.id.as_bytes());
        head.u128(site.source);
        head.u128(self.global);
        ctx.feed(&mut head);
        let head = head.finish();
        let keys: Vec<u128> = super::controlled::parallel(items, |item| {
            let mut d = Digest128::default();
            d.u128(head);
            d.u128(key(ctx, &ids, item));
            d.finish()
        });
        let stored = if self.mode == Mode::WriteOnly { HashMap::new() } else { self.load(site.id, R::TAG, &mut stats) };
        // Each item: its stored result, or none (a miss).
        let found: Vec<Option<R>> = super::controlled::parallel(&(0..items.len()).collect::<Vec<_>>(), |&at| {
            let bytes = stored.get(&keys[at])?;
            let mut input = Codec::reader(&ids, bytes);
            R::decode(&mut input).filter(|_| input.done())
        });
        let stored_count: Vec<bool> = (0..items.len()).map(|at| stored.contains_key(&keys[at])).collect();
        drop(stored);
        let verify = |key: u128| match self.mode {
            Mode::Verify { per_million } => (key % 1_000_000) < u128::from(per_million),
            _ => false,
        };
        let todo: Vec<usize> = (0..items.len()).filter(|&at| found[at].is_none() || verify(keys[at])).collect();
        let ran: Vec<R> = super::controlled::parallel(&todo, |&at| job(ctx, &items[at]));
        let mut out: Vec<R> = Vec::with_capacity(items.len());
        let mut fresh: Vec<(u128, Vec<u8>)> = Vec::new();
        let mut ran = todo.iter().copied().zip(ran).peekable();
        for (at, hit) in found.into_iter().enumerate() {
            let run = ran.next_if(|(r, _)| *r == at).map(|(_, r)| r);
            let encode = |r: &R| {
                let mut w = Codec::writer(&ids);
                r.encode(&mut w);
                w.bytes
            };
            match (hit, run) {
                (Some(hit), Some(run)) => {
                    let (a, b) = (encode(&hit), encode(&run));
                    assert!(
                        a == b,
                        "memo: VERIFY FAILED at site {}: item {at}, key {:032x}: the stored result is not the job's (stored {} bytes, \
                         ran {} bytes)",
                        site.id,
                        keys[at],
                        a.len(),
                        b.len()
                    );
                    stats.hits += 1;
                    stats.verified += 1;
                    out.push(run);
                }
                (Some(hit), None) => {
                    stats.hits += 1;
                    out.push(hit);
                }
                (None, Some(run)) => {
                    stats.misses += 1;
                    if stored_count[at] {
                        stats.errors += 1;
                    }
                    fresh.push((keys[at], encode(&run)));
                    out.push(run);
                }
                (None, None) => unreachable!("a miss is run"),
            }
        }
        if !fresh.is_empty() {
            match self.write(site.id, R::TAG, &fresh) {
                Ok(bytes) => {
                    stats.written = fresh.len();
                    stats.bytes = bytes;
                }
                Err(why) => {
                    eprintln!("memo: ERROR site {}: a segment not written ({why}); this build ran its misses", site.id);
                }
            }
        }
        if stats.errors > 0 {
            eprintln!(
                "memo: ERROR site {}: {} stored results named a floor this grid does not have or did not read back (a key that \
                 misses an input?)",
                site.id, stats.errors
            );
        }
        self.stats.push(stats);
        out
    }

    /// The entries of the site's segments in the folder (a segment that does not check is left out whole).
    fn load(&self, site: &str, tag: u32, stats: &mut SiteStats) -> HashMap<u128, Vec<u8>> {
        let mut entries = HashMap::new();
        let Ok(dir) = std::fs::read_dir(&self.dir) else { return entries };
        let prefix = format!("{site}.");
        let mut names: Vec<String> = dir
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|n| n.starts_with(&prefix) && n.ends_with(".seg"))
            .collect();
        names.sort();
        for name in names {
            match read_segment(&self.dir.join(&name), site, tag) {
                Ok(records) => {
                    stats.loaded += records.len();
                    entries.extend(records);
                }
                Err(why) => {
                    stats.segments_refused += 1;
                    eprintln!("memo: segment {name} left out ({why})");
                }
            }
        }
        entries
    }

    /// Writes a new segment of `records`; its bytes.
    fn write(&self, site: &str, tag: u32, records: &[(u128, Vec<u8>)]) -> Result<u64, String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let name = format!("{site}.{nanos:x}-{}.seg", std::process::id());
        let path = self.dir.join(&name);
        let part = self.dir.join(format!(".part-{name}"));
        let bytes = segment_bytes(site, tag, records);
        std::fs::File::create(&part)
            .and_then(|mut f| f.write_all(&bytes).and_then(|()| f.sync_all()))
            .and_then(|()| std::fs::rename(&part, &path))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(bytes.len() as u64)
    }

    /// The counts as a table for the log, one line a site.
    pub fn table(&self) -> String {
        let mut out = String::new();
        for s in &self.stats {
            let _ = writeln!(
                out,
                "memo {:?} site {:<24} items {:>9} hits {:>9} misses {:>9} verified {:>7} errors {} written {} ({} bytes) loaded {} \
                 segments refused {}{}",
                self.mode,
                s.site,
                s.items,
                s.hits,
                s.misses,
                s.verified,
                s.errors,
                s.written,
                s.bytes,
                s.loaded,
                s.segments_refused,
                if s.off { " OFF (no stable floor names)" } else { "" }
            );
        }
        out
    }
}

fn segment_bytes(site: &str, tag: u32, records: &[(u128, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(MAGIC);
    out.extend(VERSION.to_le_bytes());
    out.extend((site.len() as u32).to_le_bytes());
    out.extend(site.as_bytes());
    out.extend(tag.to_le_bytes());
    let mut digest = Digest128::default();
    for (key, bytes) in records {
        out.push(1);
        out.extend(key.to_le_bytes());
        out.extend((bytes.len() as u32).to_le_bytes());
        out.extend(bytes);
        digest.u128(*key);
        digest.str(bytes);
    }
    out.push(0);
    out.extend(END);
    out.extend((records.len() as u64).to_le_bytes());
    out.extend(digest.finish().to_le_bytes());
    out
}

fn read_segment(path: &Path, site: &str, tag: u32) -> Result<Vec<(u128, Vec<u8>)>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| e.to_string())?;
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let piece = bytes.get(at..at + n).ok_or("the segment ends early")?;
        at += n;
        Ok(piece)
    };
    let u32_of = |b: &[u8]| u32::from_le_bytes(b.try_into().expect("4 bytes"));
    if take(8)? != MAGIC {
        return Err("not a memo segment".into());
    }
    let version = u32_of(take(4)?);
    if version != VERSION {
        return Err(format!("version {version}, not {VERSION}"));
    }
    let len = u32_of(take(4)?) as usize;
    if take(len)? != site.as_bytes() {
        return Err("another site's".into());
    }
    let found = u32_of(take(4)?);
    if found != tag {
        return Err(format!("result type {found:x}, not {tag:x}"));
    }
    let mut records = Vec::new();
    let mut digest = Digest128::default();
    loop {
        match take(1)? {
            [1] => {}
            [0] => break,
            _ => return Err("no record or end mark where one is".into()),
        }
        let key = u128::from_le_bytes(take(16)?.try_into().expect("16 bytes"));
        let n = u32_of(take(4)?) as usize;
        let value = take(n)?.to_vec();
        digest.u128(key);
        digest.str(&value);
        records.push((key, value));
    }
    if take(4)? != END {
        return Err("no end mark".into());
    }
    let count = u64::from_le_bytes(take(8)?.try_into().expect("8 bytes"));
    let stored = u128::from_le_bytes(take(16)?.try_into().expect("16 bytes"));
    if count != records.len() as u64 || stored != digest.finish() {
        return Err("its end does not check".into());
    }
    if at != bytes.len() {
        return Err("bytes after its end".into());
    }
    Ok(records)
}

#[cfg(test)]
mod tests;
