//! The late join's lifecycle over a model of the game: ownership,
//! cancellation, retries and cleanup (before the Rust port
//! `test_late_join_runtime_026g.c`, each of its cases here, asserting what
//! it asserted). The model counts what the game would: allocations and
//! frees of the proposals' storage, the queries, the world's references
//! (a query holds one until its callback is done), installs and answers.

use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use super::super::profile::{CheckedProfile, Dictionary, ItemRecord, Modification};
use crate::faces::{Face, Name};
use super::super::runtime::*;
use super::*;

struct NoItems;

impl Dictionary for NoItems {
    fn item(&self, _: u16) -> Option<ItemRecord> {
        None
    }
    fn has_modification(&self, _: u32, _: Modification) -> bool {
        false
    }
}

/// A profile of three kits, kits 0 and 2 usable (the C test's decode gave
/// kit 1 a 2), no items; `name` its player's.
fn body(name: u8) -> Vec<u8> {
    let mut w = vec![0, 1, name, 0];
    w.extend(10u32.to_le_bytes());
    w.extend([3, 0]);
    for usable in [0u8, 2, 0] {
        w.push(usable);
        w.extend(1u32.to_le_bytes());
        w.push(0);
        w.extend([0; 4 * 5]);
    }
    w
}

const SEED: u32 = 0x7001;
const CLIENT: ClientKey = ClientKey(0x100);
const WORLD: WorldKey = WorldKey(0x200);
const OTHER_WORLD: WorldKey = WorldKey(0x300);
const DICTIONARY: DictionaryKey = DictionaryKey(0x400);
const A: PlayerKey = PlayerKey(0x1000);
const B: PlayerKey = PlayerKey(0x2000);
const COMPLETED: [Option<PlayerKey>; KITS] = [Some(A), None, Some(B)];

/// The E4: the C test's packet(), place 2, squad 4.
fn packet(serial: u8, body: &[u8]) -> Vec<u8> {
    let mut p = vec![0xFF, b'L', b'J', 1];
    p.extend(SEED.to_le_bytes());
    p.extend([serial, 0, 0, 0, 2, 4]);
    p.extend(body);
    p
}

#[derive(Default)]
struct Counts {
    allocations: AtomicU32,
    frees: AtomicU32,
    queries: AtomicU32,
    retains: AtomicU32,
    releases: AtomicU32,
    installs: AtomicU32,
    acks: AtomicU32,
}

struct Body {
    bytes: Vec<u8>,
    counts: Arc<Counts>,
}

impl Drop for Body {
    fn drop(&mut self) {
        self.counts.frees.fetch_add(1, SeqCst);
    }
}

#[derive(Clone)]
struct Store(Arc<Body>);

impl Storage for Store {
    fn profile(&self) -> ProfileKey {
        ProfileKey(Arc::as_ptr(&self.0) as usize as u32)
    }
    fn same_body(&self, body: &[u8]) -> bool {
        self.0.bytes == body
    }
}

struct Locked(Mutex<Runtime<Store>>);

impl Shared<Store> for Locked {
    fn with<T>(&self, f: impl FnOnce(&mut Runtime<Store>) -> T) -> T {
        f(&mut self.0.lock().unwrap())
    }
}

#[derive(Clone, Copy, PartialEq)]
enum QueryMode {
    Later,
    /// The query's callback comes at once, with every player.
    AtOnce,
    /// The match retires and the callback comes with none, at once.
    RetiredAtOnce,
}

struct Model<'a> {
    shared: &'a Locked,
    counts: Arc<Counts>,
    allocation_fails: bool,
    install_fails: bool,
    query_mode: QueryMode,
    /// The query in flight: its storage (the reference it holds).
    query: Option<Store>,
    last: Option<Ack>,
    /// The world's release destroys these players (of that profile).
    destroy_on_release: Option<ProfileKey>,
    /// The faces patch 123 was told: (place, name, face), and the storages
    /// made when each was told (the face comes before the profile is stored).
    faces: Vec<(u8, Name, Face, u32)>,
}

impl Game for Model<'_> {
    type Storage = Store;
    type Player = PlayerKey;

    fn player(&self, player: PlayerKey) -> PlayerKey {
        player
    }

    fn store(&mut self, profile: &CheckedProfile<'_>) -> Option<Store> {
        if self.allocation_fails {
            return None;
        }
        self.counts.allocations.fetch_add(1, SeqCst);
        Some(Store(Arc::new(Body { bytes: profile.body().to_vec(), counts: self.counts.clone() })))
    }

    fn check<'b>(&self, dictionary: DictionaryKey, body: &'b [u8]) -> Option<CheckedProfile<'b>> {
        assert_eq!(dictionary, DICTIONARY);
        CheckedProfile::parse(body, &NoItems)
    }

    fn learn_face(&mut self, place: Place, name: Name, face: Face) {
        self.faces.push((place.get(), name, face, self.count(|c| &c.allocations)));
    }

    fn query(&mut self, storage: Store, usable: KitMask, matched: MatchKey, place: Place) {
        self.counts.queries.fetch_add(1, SeqCst);
        self.counts.retains.fetch_add(1, SeqCst);
        assert_eq!(usable.bits(), 0b101);
        assert_eq!(matched.client, CLIENT);
        assert!(place.get() > 0);
        self.query = Some(storage);
        match self.query_mode {
            QueryMode::Later => {}
            QueryMode::AtOnce => self.complete(COMPLETED),
            QueryMode::RetiredAtOnce => {
                drop(deactivate(self.shared));
                self.complete([None; KITS]);
            }
        }
    }

    fn install(&mut self, storage: &Store, players: KitPlayers<PlayerKey>, place: Place, _: Squad,
               matched: MatchKey, _: DictionaryKey) -> bool {
        self.counts.installs.fetch_add(1, SeqCst);
        assert!(self.shared.with(|rt| rt.roster() == Some(Roster(place.get())) && rt.is_current(storage.profile())));
        assert_eq!(players.mask().bits(), 0b101);
        assert_eq!(players.each().collect::<Vec<_>>(), [(0, A), (2, B)]);
        assert_eq!(matched.world, self.shared.with(|_| WORLD));
        !self.install_fails
    }

    fn ack(&mut self, client: ClientKey, ack: Ack) {
        assert_eq!(client, CLIENT);
        assert_eq!(ack.seed, Seed(SEED));
        assert!((1..20).contains(&ack.place.get()));
        self.counts.acks.fetch_add(1, SeqCst);
        self.last = Some(ack);
    }
}

impl<'a> Model<'a> {
    fn new(shared: &'a Locked) -> Model<'a> {
        let model = Model { shared, counts: Arc::new(Counts::default()), allocation_fails: false, install_fails: false,
                            query_mode: QueryMode::Later, query: None, last: None, destroy_on_release: None,
                            faces: vec![] };
        drop(ready(shared, MatchKey { client: CLIENT, world: WORLD, seed: Seed(SEED) }, DICTIONARY, Roster(2)));
        model
    }

    fn count(&self, of: impl Fn(&Counts) -> &AtomicU32) -> u32 {
        of(&self.counts).load(SeqCst)
    }

    fn prepare(&mut self, bytes: &[u8]) {
        let shared = self.shared;
        prepare(shared, self, CLIENT, bytes);
    }

    /// The query's callback, as late_join.rs's on_query_done: complete,
    /// the dead freed, the query's storage let go, its world released last.
    fn complete(&mut self, players: [Option<PlayerKey>; KITS]) {
        let storage = self.query.take().expect("a query in flight");
        let shared = self.shared;
        drop(complete(shared, self, &storage, players));
        drop(storage);
        self.counts.releases.fetch_add(1, SeqCst);
        if let Some(profile) = self.destroy_on_release.take() {
            drop(destroyed(shared, A, profile));
            drop(destroyed(shared, B, profile));
        }
    }

    fn answer(&self) -> (u32, Answer) {
        let ack = self.last.expect("an answer");
        (ack.serial.get(), ack.answer)
    }

    fn query_profile(&self) -> ProfileKey {
        self.query.as_ref().expect("a query").profile()
    }

    /// The C test's reset(): what was allocated was freed, every reference
    /// taken let go.
    fn balanced(&self) {
        assert_eq!(self.count(|c| &c.allocations), self.count(|c| &c.frees));
        assert_eq!(self.count(|c| &c.retains), self.count(|c| &c.releases));
    }
}

fn locked() -> Locked {
    Locked(Mutex::new(Runtime::new()))
}

#[test]
fn successful_load() {
    let shared = locked();
    let mut m = Model::new(&shared);
    let profile = body(0x42);
    m.prepare(&packet(1, &profile));
    assert_eq!((m.count(|c| &c.queries), m.count(|c| &c.retains), m.count(|c| &c.acks)), (1, 1, 0));
    assert!(m.query.as_ref().unwrap().same_body(&profile));
    m.prepare(&packet(1, &profile));
    assert_eq!((m.count(|c| &c.queries), m.answer().1), (1, Answer::Busy));
    m.prepare(&packet(1, &body(0x43)));
    assert_eq!((m.count(|c| &c.queries), m.answer().1), (1, Answer::Failed));
    assert!(m.query.as_ref().unwrap().same_body(&profile));
    // a squad of its own is another proposal
    let mut other_squad = packet(1, &profile);
    other_squad[13] = 5;
    m.prepare(&other_squad);
    assert_eq!(m.answer().1, Answer::Failed);
    m.prepare(&packet(3, &profile));
    // an older serial does not replace the newest for the callback's answer
    m.prepare(&packet(2, &profile));
    let query = m.query_profile();
    m.complete(COMPLETED);
    assert_eq!((m.count(|c| &c.installs), m.count(|c| &c.releases)), (1, 1));
    assert_eq!(shared.with(|rt| rt.roster()), Some(Roster(3)));
    assert_eq!(m.answer(), (3, Answer::Installed));
    m.prepare(&packet(2, &profile));
    assert_eq!((m.answer(), m.count(|c| &c.queries)), ((2, Answer::Installed), 1));
    drop(deactivate(&shared));
    assert_eq!(m.count(|c| &c.frees), 0);
    drop(destroyed(&shared, A, query));
    assert_eq!(m.count(|c| &c.frees), 0);
    drop(destroyed(&shared, B, query));
    assert_eq!((m.count(|c| &c.frees), shared.with(|rt| rt.kept())), (1, 0));
    m.balanced();
}

#[test]
fn cancelled_load() {
    let shared = locked();
    let mut m = Model::new(&shared);
    m.prepare(&packet(1, &body(0x42)));
    let query = m.query_profile();
    drop(ready(&shared, MatchKey { client: CLIENT, world: OTHER_WORLD, seed: Seed(0x7002) }, DICTIONARY,
               Roster(2)));
    assert!(!shared.with(|rt| rt.is_current(query)));
    assert_eq!((m.count(|c| &c.frees), m.count(|c| &c.releases)), (0, 0));
    m.complete(COMPLETED);
    assert_eq!((m.count(|c| &c.installs), m.count(|c| &c.acks), m.count(|c| &c.releases)), (0, 0, 1));
    assert_eq!(shared.with(|rt| rt.roster()), Some(Roster(2)));
    drop(destroyed(&shared, B, query));
    assert_eq!(m.count(|c| &c.frees), 0);
    drop(destroyed(&shared, A, query));
    assert_eq!(m.count(|c| &c.frees), 1);
    drop(deactivate(&shared));
    m.balanced();
}

#[test]
fn partial_failure() {
    let shared = locked();
    let mut m = Model::new(&shared);
    let profile = body(0x42);
    m.prepare(&packet(1, &profile));
    let query = m.query_profile();
    m.complete([Some(A), None, None]);
    assert_eq!((m.count(|c| &c.installs), m.answer().1), (0, Answer::Failed));
    assert_eq!(shared.with(|rt| rt.roster()), Some(Roster(2)));
    drop(destroyed(&shared, A, query));
    // a failed proposal is kept, immutable, until its match retires
    assert_eq!(m.count(|c| &c.frees), 0);
    m.prepare(&packet(1, &profile));
    assert_eq!((m.count(|c| &c.queries), m.answer().1), (1, Answer::Failed));
    drop(deactivate(&shared));
    assert_eq!(m.count(|c| &c.frees), 1);
    m.balanced();
}

#[test]
fn rejected_install() {
    let shared = locked();
    let mut m = Model::new(&shared);
    m.install_fails = true;
    m.prepare(&packet(1, &body(0x42)));
    let query = m.query_profile();
    m.complete(COMPLETED);
    assert_eq!((m.count(|c| &c.installs), m.answer().1), (1, Answer::Failed));
    assert_eq!(shared.with(|rt| rt.roster()), Some(Roster(2)));
    drop(destroyed(&shared, A, query));
    drop(destroyed(&shared, B, query));
    drop(deactivate(&shared));
    assert_eq!(m.count(|c| &c.frees), 1);
    m.balanced();
}

#[test]
fn bad_requests() {
    let shared = locked();
    let mut m = Model::new(&shared);
    let good = packet(1, &body(0x42));
    for n in 0..HEADER {
        m.prepare(&good[..n]);
    }
    assert_eq!((m.count(|c| &c.queries), m.count(|c| &c.acks), m.count(|c| &c.allocations)), (0, 0, 0));
    // another match's seed
    let mut other = good.clone();
    other[4] = 2;
    m.prepare(&other);
    assert_eq!(m.count(|c| &c.acks), 0);
    // not the roster's next place
    let mut place = good.clone();
    place[12] = 3;
    m.prepare(&place);
    assert_eq!((m.answer().1, m.count(|c| &c.allocations)), (Answer::Failed, 0));
    // a profile the game cannot read: never stored
    let mut bad = packet(1, &body(0x42));
    bad[HEADER + 8] = 4;
    m.prepare(&bad);
    assert_eq!((m.answer().1, m.count(|c| &c.allocations), m.count(|c| &c.retains)), (Answer::Failed, 0, 0));
    m.allocation_fails = true;
    m.prepare(&good);
    assert_eq!((m.answer().1, m.count(|c| &c.queries)), (Answer::Busy, 0));
    drop(deactivate(&shared));
    m.balanced();
}

#[test]
fn immediate_callbacks() {
    let profile = body(0x42);
    {
        let shared = locked();
        let mut m = Model::new(&shared);
        m.query_mode = QueryMode::AtOnce;
        m.prepare(&packet(1, &profile));
        assert_eq!((m.count(|c| &c.installs), m.answer().1), (1, Answer::Installed));
        drop(deactivate(&shared));
        // kept for its players; freed once they are destroyed
        assert_eq!(shared.with(|rt| rt.kept()), 1);
        m.balanced_after_players(&shared);
    }
    {
        let shared = locked();
        let mut m = Model::new(&shared);
        m.query_mode = QueryMode::RetiredAtOnce;
        m.prepare(&packet(1, &profile));
        assert_eq!((m.count(|c| &c.frees), m.count(|c| &c.releases)), (1, 1));
        assert_eq!((m.count(|c| &c.acks), m.count(|c| &c.installs)), (0, 0));
        m.balanced();
    }
    {
        let shared = locked();
        let mut m = Model::new(&shared);
        m.prepare(&packet(1, &profile));
        m.destroy_on_release = Some(m.query_profile());
        drop(deactivate(&shared));
        m.complete(COMPLETED);
        assert_eq!((m.count(|c| &c.frees), m.count(|c| &c.releases), m.count(|c| &c.installs)), (1, 1, 0));
        m.balanced();
    }
}

impl Model<'_> {
    /// Destroys the players of the one proposal kept, then checks it was freed.
    fn balanced_after_players(&self, shared: &Locked) {
        let profiles: Vec<ProfileKey> = shared.with(|rt| rt.profiles());
        assert_eq!(profiles.len(), 1);
        drop(destroyed(shared, A, profiles[0]));
        drop(destroyed(shared, B, profiles[0]));
        assert_eq!(self.count(|c| &c.frees), 1);
        self.balanced();
    }
}

#[test]
fn concurrent_destruction() {
    for _ in 0..50 {
        let shared = locked();
        let mut m = Model::new(&shared);
        m.prepare(&packet(1, &body(0x42)));
        let query = m.query_profile();
        m.complete(COMPLETED);
        std::thread::scope(|scope| {
            let first = scope.spawn(|| drop(destroyed(&shared, A, query)));
            let second = scope.spawn(|| drop(destroyed(&shared, B, query)));
            drop(deactivate(&shared));
            first.join().unwrap();
            second.join().unwrap();
        });
        assert_eq!((m.count(|c| &c.frees), shared.with(|rt| rt.kept())), (1, 0));
        m.balanced();
    }
}

#[test]
fn a_newcomers_face_is_learnt_before_its_profile_is_stored() {
    let shared = locked();
    let mut m = Model::new(&shared);
    let name = Name::of_bytes(b"B").unwrap();
    let faced = [&body(b'B')[..], b"SRPX\x03\x01\x05"].concat();
    m.prepare(&packet(1, &faced));
    // told before the storage was made; the query's storage keeps the whole
    // body, the block too
    assert_eq!(m.faces, [(2, name, Face::of_byte(5), 0)]);
    assert_eq!((m.count(|c| &c.allocations), m.count(|c| &c.queries)), (1, 1));
    assert!(m.query.as_ref().unwrap().same_body(&faced));
    // its retransmission is the same proposal (busy), told nothing again;
    // the profile without its block is another (failed)
    m.prepare(&packet(1, &faced));
    m.prepare(&packet(1, &body(b'B')));
    assert_eq!((m.count(|c| &c.queries), m.answer().1, m.faces.len()), (1, Answer::Failed, 1));
    m.complete(COMPLETED);
    assert_eq!(m.answer(), (1, Answer::Installed));
    drop(deactivate(&shared));
    m.balanced_after_players(&shared);
}

#[test]
fn a_proposal_without_a_block_is_face_0_and_one_with_another_is_refused() {
    let shared = locked();
    let mut m = Model::new(&shared);
    m.prepare(&packet(1, &body(b'C')));
    assert_eq!(m.faces, [(2, Name::of_bytes(b"C").unwrap(), Face::FIRST, 0)]);
    assert_eq!(m.count(|c| &c.queries), 1);
    m.complete(COMPLETED);
    assert_eq!(m.answer(), (1, Answer::Installed));
    drop(deactivate(&shared));
    m.balanced_after_players(&shared);
    for block in [&b"SRPX\x03\x02\x01\x01"[..], b"SRPX\x03\x01", b"SRPY\x03\x01\x01", b"\x00"] {
        let shared = locked();
        let mut m = Model::new(&shared);
        m.prepare(&packet(1, &[&body(b'D')[..], block].concat()));
        assert_eq!((m.answer().1, m.count(|c| &c.allocations), m.faces.len()), (Answer::Failed, 0, 0), "{block:?}");
        drop(deactivate(&shared));
        m.balanced();
    }
    // a name no face is known by: nothing told, the newcomer taken
    let shared = locked();
    let mut m = Model::new(&shared);
    m.prepare(&packet(1, &body(0)));
    assert_eq!((m.faces.len(), m.count(|c| &c.queries)), (0, 1));
    m.complete(COMPLETED);
    assert_eq!(m.answer(), (1, Answer::Installed));
    drop(deactivate(&shared));
    m.balanced_after_players(&shared);
}
