//! The resource manager's reads: a read query (vostok::resources::
//! query_result) and the vfs nodes it reads. At `read_order.predicate`
//! (sites_026g.tsv: hdd_manager_sorting_predicate's start) EAX is the
//! first query and [ESP+4] the second.

use core::cmp::Ordering;
use core::ffi::c_void;
use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, pointer, read};
use crate::raw;

/// A pending read (query_result).
#[derive(Clone, Copy)]
pub struct ReadQuery<'t> {
    query: Object,
    thread: PhantomData<&'t Thread>,
}

/// The two queries the predicate is asked about.
pub fn predicate_queries<'t>(ctx: &'t Context<'_>) -> Option<(ReadQuery<'t>, ReadQuery<'t>)> {
    let first = Object::at(ctx.reg(Reg::Eax))?;
    let second = Object::at(ctx.stack(4))?;
    Some((ReadQuery { query: first, thread: PhantomData }, ReadQuery { query: second, thread: PhantomData }))
}

impl<'t> ReadQuery<'t> {
    /// The query's address (its identity when nothing else orders it).
    pub fn address(&self) -> u32 {
        self.query.id()
    }

    /// Its vfs iterator's node (None: none).
    pub fn iterator(&self) -> Option<VfsNode<'t>> {
        // SAFETY: a live query_result: its iterator node or 0.
        unsafe { pointer(raw::query_result::iterator(self.query.ptr())) }.map(VfsNode::new)
    }

    /// The node of its file: the file node, else its iterator's (`iterator`).
    pub fn file_node(&self, iterator: VfsNode<'t>) -> VfsNode<'t> {
        // SAFETY: as iterator.
        unsafe { pointer(raw::query_result::file_node(self.query.ptr())) }.map_or(iterator, VfsNode::new)
    }

    pub fn priority(&self) -> u32 {
        // SAFETY: as iterator.
        unsafe { read(raw::query_result::priority(self.query.ptr())) }
    }

    /// Its parent query's address, else its own (what orders loose reads).
    pub fn parent_or_self(&self) -> u32 {
        // SAFETY: as iterator.
        match unsafe { read(raw::query_result::parent(self.query.ptr())) } {
            0 => self.address(),
            parent => parent,
        }
    }
}

/// A vfs node.
#[derive(Clone, Copy)]
pub struct VfsNode<'t> {
    node: Object,
    thread: PhantomData<&'t Thread>,
}

/// A vfs node's in-an-archive flag.
const IN_ARCHIVE: u8 = 4;

/// The size of a buffer_string get_physical_path writes (its text pointer
/// first, then its own 104h bytes).
const PATH_SIZE: usize = 0x118;

impl<'t> VfsNode<'t> {
    fn new(node: Object) -> VfsNode<'t> {
        VfsNode { node, thread: PhantomData }
    }

    /// Whether the node is in an archive (+30h bit 2).
    pub fn in_archive(&self) -> bool {
        // SAFETY: a live vfs node: its flags.
        unsafe { read(raw::vfs_node::flags(self.node.ptr())) & IN_ARCHIVE != 0 }
    }

    /// The physical path of the iterator node (its archive's), the game's
    /// buffer_string as get_physical_path makes it.
    pub fn physical_path(&self) -> PhysicalPath {
        let mut path = PhysicalPath { bytes: [0; PATH_SIZE / 4] };
        // SAFETY: get_physical_path writes the path into our buffer_string's
        // 118h bytes (its text pointer at +0 into its own bytes).
        unsafe { raw::engine_physical_path(path.bytes.as_mut_ptr().cast(), self.node.ptr()) };
        path
    }

    /// The file's offset in its archive (get_file_offs).
    pub fn file_offset(&self) -> u64 {
        // SAFETY: a live vfs node.
        unsafe { raw::engine_file_offset(self.node.ptr()) }
    }
}

/// A physical path, as get_physical_path left it.
pub struct PhysicalPath {
    bytes: [u32; PATH_SIZE / 4],
}

impl PhysicalPath {
    /// The paths' order as strcmp_s gives it (on their text pointers, a
    /// null text empty).
    pub fn compare(&self, other: &PhysicalPath) -> Ordering {
        // SAFETY: the two texts get_physical_path wrote (their pointers at +0).
        let order = unsafe {
            raw::engine_strcmp(self.bytes[0] as usize as *const u8, other.bytes[0] as usize as *const u8)
        } as i32;
        order.cmp(&0)
    }
}

const _: () = assert!(size_of::<*mut c_void>() == 4);
