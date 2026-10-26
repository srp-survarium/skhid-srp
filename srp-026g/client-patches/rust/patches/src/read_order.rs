//! Patch 33: the order of the archive reads is a strict weak ordering
//! (client_patch.md section 3.31).
//!
//! The resource manager sorts its pending reads with an unguarded
//! std::sort, which needs its predicate to be a strict weak ordering. The
//! original was none when a batch held reads of two archives. Here,
//! lexicographically: a read with no iterator first (those by address);
//! the higher priority first; a read from an archive before one that is
//! not; reads from archives by the archive's physical path, then the file
//! offset; other reads by their parent query (or themselves).

#![forbid(unsafe_code)]

use core::cmp::Ordering;

use crate::vostok::resources::{ReadQuery, predicate_queries};
use crate::vostok::{Context, Flow, ReadOrderOnPredicateExit, Reg};

/// Whether `a` is read before `b`.
fn before(a: &ReadQuery<'_>, b: &ReadQuery<'_>) -> bool {
    let (a_iterator, b_iterator) = match (a.iterator(), b.iterator()) {
        (None, Some(_)) => return true,
        (None, None) => return a.address() < b.address(),
        (Some(_), None) => return false,
        (Some(x), Some(y)) => (x, y),
    };
    match b.priority().cmp(&a.priority()) {
        Ordering::Less => return true,
        Ordering::Greater => return false,
        Ordering::Equal => {}
    }
    let (a_file, b_file) = (a.file_node(a_iterator), b.file_node(b_iterator));
    match (a_file.in_archive(), b_file.in_archive()) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => a.parent_or_self() < b.parent_or_self(),
        (true, true) => match a_iterator.physical_path().compare(&b_iterator.physical_path()) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => a_file.file_offset() < b_file.file_offset(),
        },
    }
}

/// `read_order.predicate`: the whole predicate, AL (EAX 0 or 1) its answer,
/// returning for the function (`ret 4`).
pub fn on_predicate(ctx: &mut Context<'_>) -> Flow<ReadOrderOnPredicateExit> {
    let answer = match predicate_queries(ctx) {
        Some((a, b)) => before(&a, &b),
        None => return Flow::Continue,
    };
    ctx.set_reg(Reg::Eax, u32::from(answer));
    Flow::Exit(ReadOrderOnPredicateExit::Done)
}
