//! Patch 121: the contact manifolds of the bodies that move (the batteries,
//! the grenades) go with every world, so every game steps them from the same
//! contacts after any load (`docs/notes/physics_contacts.md`).
//!
//! Bullet keeps a touching pair's contact points from step to step (a
//! persistent manifold) and no world carried them, so a game that loaded a
//! world (a rollback, a resync, a joiner, a replay's seek) stepped a landing
//! or settling battery from contacts of another moment than a game that
//! stepped straight on, and their worlds parted by millimetres (match
//! 0x54417103, 135.45 s: seven CRCs among fifteen games, one battery's
//! transform the only difference).
//!
//! - A world's save (`physics_contacts.saved`): every manifold with points of
//!   a pair with a moving body and one manifold, with its bodies' keys, which
//!   of them the near callback passes first and its points, in key order
//!   (srp_validate::world::Contacts), and the records a load left waiting.
//! - A world's load (`keeper.world_loaded`, order 2): those manifolds of the
//!   game emptied, the world's records waiting.
//! - The step puts each record into its pair's algorithm, as the game that
//!   saved the world had it: before a pair's processCollision
//!   (`near_process`) into its manifold when the order the near callback
//!   passes the bodies in and the manifold's are the record's (else it is
//!   spent: that game made its algorithm again then, empty); in a manifold a
//!   processCollision makes (`new_manifold`, btConvexConvexAlgorithm's);
//!   after the first collision detection (`detected`) a pair the near
//!   callback did not take (asleep) gets its algorithm in the record's order.
//! - Every solveGroup takes its manifolds in key order (`solve`): the
//!   dispatcher's array is in the order the game made its manifolds, which a
//!   load does not keep.
//!
//! The C++ simulation `client-patches/contacts_sim.cpp` (Bullet 2.79 of the
//! 0.10b source, the game's world, step, near callback and battery) steps a
//! game straight, its twin with rollbacks and a joiner from its world: every
//! one bit for bit with the fix (600 drops), stock 599 of 600 apart.

#![forbid(unsafe_code)]

use crate::vostok::log::{Piece::Dec, Piece::Text, line};
use crate::vostok::physics_contacts::{
    self as physics, Body, Dispatcher, DispatcherId, Manifold, Pair, PointBytes, World,
};
use crate::vostok::{Context, Flow, GameCell, Lock, NoExit};
use srp_validate::world::{BodyKey, ContactHead, Contacts, MAX_CONTACTS, POINT_BYTES, PointCount};

/// A manifold as a world holds it.
#[derive(Clone, Copy)]
struct Record {
    head: ContactHead,
    points: [PointBytes; 4],
}

impl Record {
    fn points(&self) -> &[PointBytes] {
        self.points.get(..self.head.points.get()).unwrap_or_default()
    }
}

/// A loaded record: waiting for its pair's manifold, or put in (or spent).
#[derive(Clone, Copy)]
enum Slot {
    Waiting(Record),
    Spent,
}

/// Where the step is since the load.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// No collision detection yet: the pairs of a world just loaded may not
    /// overlap yet (their boxes are the game's of before).
    Unstepped,
    /// After the first.
    Detected,
}

/// The records of the last world loaded into a physics world, until each
/// is put in.
#[derive(Clone, Copy)]
enum Pending {
    None,
    Loaded { dispatcher: DispatcherId, step: Step, len: usize, slots: [Slot; MAX_CONTACTS] },
}

static PENDING: Lock<Pending> = Lock::new(Pending::None);

/// The pair whose processCollision runs (its bodies' addresses), from
/// `near_process` to the next near callback's entry: a manifold made for
/// these bodies in between is that processCollision's.
static PROCESSING: GameCell<Option<(u32, u32)>> = GameCell::new(None);

/// Whether a manifold is this patch's to save and put back: a pair with a
/// moving body, the one manifold of its bodies (a compound's children share
/// the pair: theirs stay the game's).
fn managed(dispatcher: Dispatcher, manifold: Manifold) -> Option<(Body, Body)> {
    let (a, b) = manifold.bodies()?;
    if !(a.moving() || b.moving()) {
        return None;
    }
    (dispatcher.manifolds().filter(|m| m.joins(a, b)).count() == 1).then_some((a, b))
}

fn record_of(world: World, manifold: Manifold, a: Body, b: Body) -> Option<Record> {
    let points = PointCount::new(manifold.points())?;
    let first = world.pair(a, b).and_then(Pair::first);
    let mut out = [[0u8; POINT_BYTES]; 4];
    for (i, point) in out.iter_mut().take(points.get()).enumerate() {
        *point = manifold.read_point(i);
    }
    Some(Record {
        head: ContactHead { body0: a.key(), body1: b.key(), body0_first: first.is_none_or(|f| f == a), points },
        points: out,
    })
}

/// A body of the world by its key.
fn body_of(world: World, key: BodyKey) -> Option<Body> {
    world.bodies().find(|body| body.key() == key)
}

/// `game_world_core::serialize` after the physics world's own (nothing): the
/// physics section.
pub fn on_saved(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(saving) = physics::Saving::at(ctx) else { return Flow::Continue };
    let world = saving.world;
    let dispatcher = world.dispatcher();
    let mut records = [None::<Record>; MAX_CONTACTS];
    let mut len = 0;
    let mut over = 0u32;
    let mut push = |record: Record| match records.get_mut(len) {
        Some(slot) => {
            *slot = Some(record);
            len += 1;
        }
        None => over += 1,
    };
    for manifold in dispatcher.manifolds() {
        if let Some((a, b)) = managed(dispatcher, manifold)
            && let Some(record) = record_of(world, manifold, a, b)
        {
            push(record);
        }
    }
    // the records a load left waiting are the world's too, while their
    // bodies are where the world has them; a body gone or moved spends its
    // record (the game that saved it stepped the pair then)
    PENDING.with(ctx.thread(), |pending| {
        let Pending::Loaded { dispatcher: of, step, len: n, slots } = pending else { return };
        if *of != dispatcher.id() {
            return;
        }
        for slot in slots.iter_mut().take(*n) {
            if let Slot::Waiting(record) = *slot {
                if body_of(world, record.head.body0).is_some() && body_of(world, record.head.body1).is_some() {
                    push(record);
                } else if *step == Step::Detected {
                    *slot = Slot::Spent;
                }
            }
        }
        if *step == Step::Detected && slots.iter().take(*n).all(|s| matches!(s, Slot::Spent)) {
            *pending = Pending::None;
        }
    });
    let records = records.get_mut(..len).unwrap_or_default();
    records.sort_unstable_by_key(|r| r.map(|r| r.head.order()));
    // one record a pair: a live manifold before a waiting record of it
    let mut kept = [None::<Record>; MAX_CONTACTS];
    let mut count = 0;
    for record in records.iter().flatten() {
        let fresh = count == 0
            || kept.get(count - 1).copied().flatten().is_none_or(|last| last.head.order() != record.head.order());
        if fresh && let Some(slot) = kept.get_mut(count) {
            *slot = Some(*record);
            count += 1;
        }
    }
    if over > 0 {
        line(&[Text("physics_contacts: "), Dec(over), Text(" manifolds over the world's 32 not saved")]);
    }
    let kept = kept.get(..count).unwrap_or_default();
    Contacts::encode(RecordBytes { records: kept.iter() }, |bytes| saving.write(bytes));
    Flow::Continue
}

/// Each record as its head and its points' bytes (contiguous in the record).
struct RecordBytes<'r> {
    records: core::slice::Iter<'r, Option<Record>>,
}

impl<'r> Iterator for RecordBytes<'r> {
    type Item = (ContactHead, &'r [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        let record = self.records.next()?.as_ref()?;
        let bytes = record.points.as_flattened();
        Some((record.head, bytes.get(..record.head.points.get() * POINT_BYTES).unwrap_or(bytes)))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.records.size_hint()
    }
}

impl ExactSizeIterator for RecordBytes<'_> {}

/// The end of a world's load: the game's manifolds of moving bodies emptied,
/// the world's records waiting for the step.
pub fn on_world_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(loaded) = physics::Loaded::at(ctx) else { return Flow::Continue };
    let world = loaded.world;
    let dispatcher = world.dispatcher();
    let mut slots = [Slot::Spent; MAX_CONTACTS];
    let mut len = 0;
    match Contacts::parse(loaded.bytes, 0) {
        Ok(contacts) => {
            for contact in contacts.iter() {
                let Some(slot) = slots.get_mut(len) else { break };
                let mut points = [[0u8; POINT_BYTES]; 4];
                for (i, point) in points.iter_mut().enumerate().take(contact.head.points.get()) {
                    if let Some(bytes) = contact.point(i) {
                        point.copy_from_slice(bytes);
                    }
                }
                *slot = Slot::Waiting(Record { head: contact.head, points });
                len += 1;
            }
        }
        Err(_) => line(&[Text("physics_contacts: a world's contacts do not read; none put back")]),
    }
    for manifold in dispatcher.manifolds() {
        if managed(dispatcher, manifold).is_some() {
            manifold.clear();
        }
    }
    let thread = ctx.thread();
    PROCESSING.set(thread, None);
    PENDING.with(thread, |pending| {
        *pending = Pending::Loaded { dispatcher: dispatcher.id(), step: Step::Unstepped, len, slots };
    });
    Flow::Continue
}

/// Runs `f` on the waiting records of `dispatcher`'s physics world.
fn waiting(ctx: &Context<'_>, dispatcher: Dispatcher, f: impl FnOnce(&mut [Slot], &mut Step)) {
    PENDING.with(ctx.thread(), |pending| {
        if let Pending::Loaded { dispatcher: of, step, len, slots } = pending
            && *of == dispatcher.id()
        {
            f(slots.get_mut(..*len).unwrap_or_default(), step);
        }
    });
}

/// The near callback's entry: no pair's processCollision runs.
pub fn on_near_entry(ctx: &mut Context<'_>) -> Flow<NoExit> {
    PROCESSING.set(ctx.thread(), None);
    Flow::Continue
}

/// A record's two keys are these bodies', in either order.
fn names(record: &Record, a: BodyKey, b: BodyKey) -> bool {
    (record.head.body0 == a && record.head.body1 == b) || (record.head.body0 == b && record.head.body1 == a)
}

/// Before a pair's processCollision: its record into its manifold.
pub fn on_near_process(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some((dispatcher, pair)) = physics::near_process(ctx) else { return Flow::Continue };
    let Some((a, b)) = pair.bodies() else { return Flow::Continue };
    let thread = ctx.thread();
    let mut processing = false;
    waiting(ctx, dispatcher, |slots, _| {
        processing = true;
        if !(a.moving() || b.moving()) {
            return;
        }
        let (ka, kb) = (a.key(), b.key());
        for slot in slots.iter_mut() {
            let Slot::Waiting(record) = *slot else { continue };
            if !names(&record, ka, kb) {
                continue;
            }
            let mut ours = dispatcher.manifolds().filter(|m| m.joins(a, b));
            let (Some(manifold), None) = (ours.next(), ours.next()) else {
                // none yet: the processCollision makes it (new_manifold); or
                // a compound's, which are the game's
                if dispatcher.manifolds().filter(|m| m.joins(a, b)).count() > 1 {
                    *slot = Slot::Spent;
                }
                continue;
            };
            *slot = Slot::Spent;
            put_in(manifold, &record, pair.first().map(Body::key));
        }
    });
    if processing {
        PROCESSING.set(thread, Some((a.id(), b.id())));
    }
    Flow::Continue
}

/// The record into the manifold when its order is the record's and the near
/// callback passes the record's body first.
fn put_in(manifold: Manifold, record: &Record, first: Option<BodyKey>) {
    let Some((m0, _)) = manifold.bodies() else { return };
    let first0 = first.map(|first| first == record.head.body0);
    if m0.key() == record.head.body0 && first0 == Some(record.head.body0_first) {
        manifold.fill(record.points());
    }
}

/// A manifold getNewManifold made: in a processCollision of its pair (a
/// btConvexConvexAlgorithm's own), its record.
pub fn on_new_manifold(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some((dispatcher, manifold)) = physics::new_manifold(ctx) else { return Flow::Continue };
    let Some((a, b)) = manifold.bodies() else { return Flow::Continue };
    let thread = ctx.thread();
    let Some(processing) = PROCESSING.get(thread) else { return Flow::Continue };
    let (pa, pb) = (a.id(), b.id());
    if processing != (pa, pb) && processing != (pb, pa) {
        return Flow::Continue;
    }
    let (ka, kb) = (a.key(), b.key());
    waiting(ctx, dispatcher, |slots, _| {
        for slot in slots.iter_mut() {
            let Slot::Waiting(record) = *slot else { continue };
            if record.head.body0 == ka && record.head.body1 == kb {
                *slot = Slot::Spent;
                // made in the call's order: its body 0 the one passed first
                if record.head.body0_first {
                    manifold.fill(record.points());
                }
                return;
            }
        }
    });
    Flow::Continue
}

/// After the first collision detection since a load: a record whose pair
/// the near callback did not take (asleep) into its algorithm, of the
/// record's order (made again when it is of the other).
pub fn on_detected(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(world) = physics::detected(ctx) else { return Flow::Continue };
    let dispatcher = world.dispatcher();
    let thread = ctx.thread();
    PROCESSING.set(thread, None);
    waiting(ctx, dispatcher, |slots, step| {
        if *step != Step::Unstepped {
            return;
        }
        *step = Step::Detected;
        for slot in slots.iter_mut() {
            let Slot::Waiting(record) = *slot else { continue };
            *slot = materialize(world, &record);
        }
    });
    Flow::Continue
}

fn materialize(world: World, record: &Record) -> Slot {
    let dispatcher = world.dispatcher();
    let (Some(a), Some(b)) = (body_of(world, record.head.body0), body_of(world, record.head.body1)) else {
        return Slot::Spent; // a body gone: its pair is gone in the game that saved it too
    };
    let Some(pair) = world.pair(a, b) else { return Slot::Spent };
    let first = |pair: Pair| pair.first().map(|f| f == a);
    let count = dispatcher.manifolds().filter(|m| m.joins(a, b)).count();
    if count > 1 {
        return Slot::Spent;
    }
    if pair.has_algorithm() {
        if first(pair) == Some(record.head.body0_first) {
            match dispatcher.manifolds().find(|m| m.joins(a, b)) {
                Some(manifold) if manifold.bodies().is_some_and(|(m0, _)| m0 == a) => {
                    manifold.fill(record.points());
                    return Slot::Spent;
                }
                Some(_) => {}
                None => return Slot::Waiting(*record), // made in its first processCollision
            }
        }
        world.clean(pair);
    }
    let (one, two) = if record.head.body0_first { (a, b) } else { (b, a) };
    world.make(pair, one, two);
    match dispatcher.manifolds().find(|m| m.joins(a, b)) {
        Some(manifold) => {
            if manifold.bodies().is_some_and(|(m0, _)| m0 == a) {
                manifold.fill(record.points());
            }
            Slot::Spent
        }
        None => Slot::Waiting(*record),
    }
}

/// solveGroup's entry: its manifolds in their bodies' key order.
pub fn on_solve(ctx: &mut Context<'_>) -> Flow<NoExit> {
    physics::sort_solve(ctx, |manifold: Manifold| manifold.bodies().map(|(a, b)| (a.key(), b.key())));
    Flow::Continue
}
