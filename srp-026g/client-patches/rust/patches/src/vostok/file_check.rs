//! The game's side of patch 215 (`crate::file_check`): the archives it
//! mounted read through kernel32 (`files`), where the mount found them
//! (`..\..\resources.db`, then each `resources.db.patch_<n>` there, n 1 to
//! 255: mount_patches_026g.asm), its memory pages of its own while a run
//! lasts (`memory_profile::reserve`), its reports on the match connection
//! (`packet_recovery`). One match connection at a time: its offer when its
//! first world is in, its run when the server's request comes, a bounded
//! step each frame, its answer told once whole; all of it gone at the
//! match's close.
use super::files::{File, Find, FindData, Path};
use super::log::{line, Piece::*};
use super::memory_profile::{reserve, Room};
use super::packet_recovery::Connection;
use super::{Context, Lock, Thread, ThreadBound};
use crate::file_check::run::{Archive, Budget, Disk, Run, Slot};
use crate::file_check::table::Source;
use srp_validate::checks::{Answer, Mounts, Report as Checks, Request};
use srp_validate::report::Report;

const GAME_DATA: &[u8] = b"..\\..\\";
const DB: &[u8] = b"resources.db";
const PATCH: &[u8] = b"resources.db.patch_";
/// A frame's part of a run: a slow disk reads 64 KiB in a few ms.
const BUDGET: Budget = Budget { lookups: 32, bytes: 64 * 1024 };

pub struct ArchiveFile(File);

impl Source for ArchiveFile {
    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Option<usize> {
        self.0.read_at(offset, into)
    }
}

/// The game's archives and its memory, for one call.
struct Game<'t>(&'t Thread);

impl Disk for Game<'_> {
    type File = ArchiveFile;
    type Bytes = Room<u8>;
    type Slots = Room<Slot>;

    fn open(&mut self, archive: Archive) -> Option<ArchiveFile> {
        let mut name = [0u8; 48];
        let mut len = put(&mut name, 0, GAME_DATA)?;
        len = match archive {
            Archive::Db => put(&mut name, len, DB)?,
            Archive::Patch(n) => {
                let len = put(&mut name, len, PATCH)?;
                decimal(&mut name, len, n)?
            }
        };
        // The NUL after the name: the buffer is zeroed and longer.
        name.get(len)?;
        File::open(Path::new(&name)?).map(ArchiveFile)
    }

    fn bytes(&mut self, len: usize) -> Option<Room<u8>> {
        reserve(self.0, len)
    }

    fn slots(&mut self, count: usize) -> Option<Room<Slot>> {
        reserve(self.0, count)
    }
}

/// `bytes` into `out` at `at`: where they end.
fn put(out: &mut [u8], at: usize, bytes: &[u8]) -> Option<usize> {
    let end = at.checked_add(bytes.len())?;
    out.get_mut(at..end)?.copy_from_slice(bytes);
    Some(end)
}

/// `n` in decimal, as the mount writes a patch's number (`%u`).
fn decimal(out: &mut [u8], at: usize, n: u8) -> Option<usize> {
    let digits = [n / 100, n / 10 % 10, n % 10];
    let first = digits.iter().position(|&d| d != 0).unwrap_or(2);
    let mut end = at;
    for &digit in digits.get(first..)? {
        end = put(out, end, &[b'0' + digit])?;
    }
    Some(end)
}

/// The number of a name `resources.db.patch_<n>` as the mount names one.
fn number(name: &[u8]) -> Option<u8> {
    let digits = name.strip_prefix(PATCH)?;
    if digits.is_empty() || digits.len() > 3 || digits.first() == Some(&b'0') || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let n = digits.iter().fold(0u32, |n, &d| n * 10 + u32::from(d - b'0'));
    u8::try_from(n).ok().filter(|&n| n != 0)
}

/// The patches there, as the mount found them at the game's start: each
/// `resources.db.patch_<n>` of the game folder (patch 49 stops a game that
/// misses one its release needs).
fn mounts() -> Mounts {
    let mut pattern = [0u8; 48];
    let Some(end) = put(&mut pattern, 0, GAME_DATA).and_then(|at| put(&mut pattern, at, PATCH)).and_then(|at| put(&mut pattern, at, b"*")) else {
        return Mounts::NONE;
    };
    let mut there = [false; 256];
    let mut found = FindData::new();
    if pattern.get(end).is_some()
        && let Some(path) = Path::new(&pattern)
        && let Some(mut find) = Find::first(path, &mut found)
    {
        loop {
            if !found.is_folder()
                && let Some(n) = number(found.name())
                && let Some(slot) = there.get_mut(usize::from(n))
            {
                *slot = true;
            }
            if !find.next(&mut found) {
                break;
            }
        }
    }
    let mut numbers = [0u8; 255];
    let mut count = 0;
    for n in 1..=255u8 {
        if there[usize::from(n)]
            && let Some(slot) = numbers.get_mut(count)
        {
            *slot = n;
            count += 1;
        }
    }
    numbers.get(..count).and_then(Mounts::of).unwrap_or(Mounts::NONE)
}

type GameRun = Run<ArchiveFile, Room<u8>, Room<Slot>>;

enum Stage {
    /// The offer is to go (the pool had no room for it yet).
    Offering(Mounts),
    /// Offered: the server's request is waited for.
    Offered(Mounts),
    Running(GameRun),
    /// The run's memory could not be had: each of `count` files unreadable.
    Unmet { mounts: Mounts, count: u16 },
    Told,
}

struct Active {
    key: (usize, u32),
    stage: ThreadBound<Stage>,
}

static ACTIVE: Lock<Option<Active>> = Lock::new(None);

/// The connection's first world is in: its offer.
pub fn loaded(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_loaded(ctx) else { return };
    let thread = ctx.thread();
    ACTIVE.with(thread, |active| {
        // A resync's world: the same connection, offered already.
        if active.as_ref().is_some_and(|a| a.key == connection.key()) {
            return;
        }
        let mut stage = Stage::Offering(mounts());
        advance(thread, &connection, &mut stage);
        *active = Some(Active { key: connection.key(), stage: ThreadBound::new(thread, stage) });
    });
}

/// Each frame of the connection's match: its next step.
pub fn frame(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_game_tick(ctx) else { return };
    let thread = ctx.thread();
    ACTIVE.with(thread, |active| {
        if let Some(active) = active.as_mut().filter(|a| a.key == connection.key())
            && let Some(stage) = active.stage.get_mut(thread)
        {
            advance(thread, &connection, stage);
        }
    });
}

/// The match closes on the game: nothing of it is kept.
pub fn close(ctx: &Context<'_>) {
    let Some(connection) = Connection::at_close(ctx) else { return };
    ACTIVE.with(ctx.thread(), |active| {
        if active.as_ref().is_some_and(|a| a.key == connection.key()) {
            *active = None;
        }
    });
}

/// The server's request `bytes` for the connection `key`: its run, for a
/// connection that offered and was not asked yet.
pub fn request(thread: &Thread, key: (usize, u32), bytes: &[u8]) {
    let Some(request) = Request::parse(bytes) else { return };
    if request.match_id != key.1 {
        return;
    }
    ACTIVE.with(thread, |active| {
        let Some(active) = active.as_mut().filter(|a| a.key == key) else { return };
        let Some(stage) = active.stage.get_mut(thread) else { return };
        let Stage::Offered(mounts) = *stage else { return };
        *stage = match Run::new(&mut Game(thread), bytes, mounts) {
            Some(run) => Stage::Running(run),
            None => Stage::Unmet { mounts, count: request.len() },
        };
    });
}

fn advance(thread: &Thread, connection: &Connection<'_>, stage: &mut Stage) {
    match stage {
        Stage::Offering(mounts) => {
            let mounts = *mounts;
            if connection.tell(&Report::Checks(Checks::offer(mounts))) {
                line(&[Text("files, match "), Hex(connection.match_id()), Text(": "), Dec(mounts.numbers().len() as u32), Text(" patches")]);
                *stage = Stage::Offered(mounts);
            }
        }
        Stage::Running(run) if !run.whole() => run.step(&mut Game(thread), BUDGET),
        Stage::Running(run) => {
            let report = run.report();
            tell(connection, report, stage);
        }
        Stage::Unmet { mounts, count } => {
            let mut report = Checks::new(*mounts);
            for _ in 0..*count {
                report.add(Answer::Unreadable);
            }
            tell(connection, report, stage);
        }
        Stage::Offered(_) | Stage::Told => {}
    }
}

/// The answer, once whole; its run's memory given back once it went.
fn tell(connection: &Connection<'_>, report: Checks, stage: &mut Stage) {
    let count = report.answers().len() as u32;
    if connection.tell(&Report::Checks(report)) {
        line(&[Text("files, match "), Hex(connection.match_id()), Text(": "), Dec(count), Text(" told the server")]);
        *stage = Stage::Told;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_is_named_as_the_mount_names_it() {
        for n in [1u8, 9, 10, 73, 100, 255] {
            let mut name = [0u8; 48];
            let len = put(&mut name, 0, PATCH).unwrap();
            let len = decimal(&mut name, len, n).unwrap();
            assert_eq!(number(&name[..len]), Some(n));
        }
        for bad in [&b"resources.db.patch_0"[..], b"resources.db.patch_073", b"resources.db.patch_256", b"resources.db.patch_", b"resources.db.patch_7a", b"resources.db"] {
            assert_eq!(number(bad), None);
        }
    }
}
