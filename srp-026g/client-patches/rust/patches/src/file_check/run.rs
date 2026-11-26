//! One request's run: each asked path looked up in the archives the game
//! mounted, the last one first (a patch's file is over `resources.db`'s and
//! the lower patches', as the game's mount lays them), then each found file
//! hashed, a bounded part a step: the answer, in the request's order.
#![forbid(unsafe_code)]

use sign_in_cipher::Sha256;
use srp_validate::checks::{Answer, Mounts, PREFIX, Report, Request};

use super::table::{Found, Pages, Place, Source, PAGE};

/// An archive the game mounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archive {
    Db,
    Patch(u8),
}

/// What a run reads and the memory it works in, the game's or a test's.
pub trait Disk {
    type File: Source;
    type Bytes: AsMut<[u8]>;
    type Slots: AsMut<[Slot]>;
    fn open(&mut self, archive: Archive) -> Option<Self::File>;
    /// `len` zeroed bytes.
    fn bytes(&mut self, len: usize) -> Option<Self::Bytes>;
    /// `count` slots, each unresolved.
    fn slots(&mut self, count: usize) -> Option<Self::Slots>;
}

/// One asked file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Slot {
    /// No archive looked at so far has it.
    #[default]
    Unresolved,
    /// The last archive that has it, and where.
    At(Archive, Place),
    Answered(Answer),
}

/// What a step may do at most.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    /// Paths looked up.
    pub lookups: u32,
    /// Bytes of files read.
    pub bytes: u32,
}

/// The table cache's pages, and a file read's bytes a step.
pub const CACHE: usize = 32 * PAGE;
pub const CHUNK: usize = 64 * 1024;

enum Phase<F, B> {
    /// Looking paths up in the archive `archive` (0 the last mounted), from
    /// the path `next` on; its table open once it is.
    Resolving { archive: usize, next: u16, pages: Option<Pages<F, B>> },
    /// Hashing from file `index` on; the archive open, the file begun.
    Hashing { index: u16, open: Option<(Archive, F)>, file: Option<Hashing> },
    Whole,
}

struct Hashing {
    sha: Sha256,
    at: u64,
    left: u32,
    size: u32,
}

/// A run, of the file, memory and slot types of the [`Disk`] it reads
/// (none of the disk itself: it is given to each step).
pub struct Run<F, B, S> {
    request: B,
    len: usize,
    count: u16,
    mounts: Mounts,
    slots: S,
    chunk: B,
    /// The table cache, between archives.
    cache: Option<B>,
    phase: Phase<F, B>,
}

impl<F: Source, B: AsMut<[u8]>, S: AsMut<[Slot]>> Run<F, B, S> {
    /// The run of the request `bytes` (checked again here) for a game that
    /// mounted `mounts`; none when it is no request or the memory is not
    /// there.
    pub fn new<D: Disk<File = F, Bytes = B, Slots = S>>(disk: &mut D, bytes: &[u8], mounts: Mounts) -> Option<Self> {
        let count = Request::parse(bytes)?.len();
        let mut request = disk.bytes(bytes.len().max(1))?;
        request.as_mut().get_mut(..bytes.len())?.copy_from_slice(bytes);
        Some(Run {
            request,
            len: bytes.len(),
            count,
            mounts,
            slots: disk.slots(usize::from(count).max(1))?,
            chunk: disk.bytes(CHUNK)?,
            cache: Some(disk.bytes(CACHE)?),
            phase: Phase::Resolving { archive: 0, next: 0, pages: None },
        })
    }

    /// The archives, the last mounted first.
    fn archive(&self, index: usize) -> Option<Archive> {
        let numbers = self.mounts.numbers();
        match index.checked_sub(numbers.len()) {
            None => numbers.get(numbers.len() - 1 - index).map(|&n| Archive::Patch(n)),
            Some(0) => Some(Archive::Db),
            Some(_) => None,
        }
    }

    /// Whether the answer is whole ([`Run::report`]).
    pub fn whole(&self) -> bool {
        matches!(self.phase, Phase::Whole)
    }

    /// One step, as far as `budget` goes.
    pub fn step<D: Disk<File = F, Bytes = B, Slots = S>>(&mut self, disk: &mut D, mut budget: Budget) {
        loop {
            let phase = core::mem::replace(&mut self.phase, Phase::Whole);
            let (next, more) = match phase {
                Phase::Resolving { archive, next, pages } => self.resolve(disk, archive, next, pages, &mut budget),
                Phase::Hashing { index, open, file } => self.hash(disk, index, open, file, &mut budget),
                Phase::Whole => (Phase::Whole, false),
            };
            self.phase = next;
            if !more {
                return;
            }
        }
    }

    /// The answer: the mounts and each file's, in the request's order.
    pub fn report(&mut self) -> Report {
        let mut report = Report::new(self.mounts);
        for slot in self.slots.as_mut().iter().take(usize::from(self.count)) {
            let answer = match slot {
                Slot::Answered(answer) => *answer,
                Slot::Unresolved | Slot::At(..) => Answer::Unreadable,
            };
            report.add(answer);
        }
        report
    }

    /// Looks paths up in archive `archive`; `true` in the second when the
    /// next phase may go on in this step.
    fn resolve<D: Disk<File = F, Bytes = B, Slots = S>>(
        &mut self,
        disk: &mut D,
        archive: usize,
        next: u16,
        pages: Option<Pages<F, B>>,
        budget: &mut Budget,
    ) -> (Phase<F, B>, bool) {
        let Some(which) = self.archive(archive) else {
            // Past resources.db: what no archive has is missing.
            for slot in self.slots.as_mut().iter_mut() {
                if *slot == Slot::Unresolved {
                    *slot = Slot::Answered(Answer::Missing);
                }
            }
            return (Phase::Hashing { index: 0, open: None, file: None }, true);
        };
        let mut pages = match pages {
            Some(pages) => pages,
            None => match (disk.open(which), self.cache.take()) {
                (Some(file), Some(cache)) => match Pages::open(file, cache) {
                    Some(pages) => pages,
                    // No archive the game reads: whatever it may hold is
                    // not known.
                    None => {
                        self.cache = disk.bytes(CACHE);
                        return (self.unreadable_from(), true);
                    }
                },
                (_, cache) => {
                    self.cache = cache;
                    return (self.unreadable_from(), true);
                }
            },
        };
        let Some(request) = Request::parse(self.request.as_mut().get(..self.len).unwrap_or_default()) else {
            return (Phase::Whole, false);
        };
        let mut paths = request.paths();
        let mut index = 0u16;
        while let Some(path) = paths.next_path() {
            if index < next {
                index += 1;
                continue;
            }
            let Some(slot) = self.slots.as_mut().get_mut(usize::from(index)) else { break };
            if *slot == Slot::Unresolved {
                if budget.lookups == 0 {
                    return (Phase::Resolving { archive, next: index, pages: Some(pages) }, false);
                }
                budget.lookups -= 1;
                *slot = match pages.find(path) {
                    Found::File(place) => Slot::At(which, place),
                    Found::Nothing => Slot::Unresolved,
                    Found::Unreadable => Slot::Answered(Answer::Unreadable),
                };
            }
            index += 1;
        }
        self.cache = Some(pages.into_parts().1);
        (Phase::Resolving { archive: archive + 1, next: 0, pages: None }, true)
    }

    /// An archive that cannot be read: a file it may hold over the ones
    /// below is not known, so every file not found yet is unreadable.
    fn unreadable_from(&mut self) -> Phase<F, B> {
        for slot in self.slots.as_mut().iter_mut() {
            if *slot == Slot::Unresolved {
                *slot = Slot::Answered(Answer::Unreadable);
            }
        }
        Phase::Hashing { index: 0, open: None, file: None }
    }

    fn hash<D: Disk<File = F, Bytes = B, Slots = S>>(
        &mut self,
        disk: &mut D,
        mut index: u16,
        mut open: Option<(Archive, F)>,
        mut file: Option<Hashing>,
        budget: &mut Budget,
    ) -> (Phase<F, B>, bool) {
        while index < self.count {
            let Some(&slot) = self.slots.as_mut().get(usize::from(index)) else { break };
            let Slot::At(archive, place) = slot else {
                index += 1;
                continue;
            };
            if open.as_ref().is_none_or(|(at, _)| *at != archive) {
                open = disk.open(archive).map(|f| (archive, f));
            }
            let Some((_, source)) = open.as_mut() else {
                self.answer(index, Answer::Unreadable);
                index += 1;
                file = None;
                continue;
            };
            let mut hashing = file.take().unwrap_or(Hashing { sha: Sha256::new(), at: place.offset, left: place.size, size: place.size });
            while hashing.left > 0 {
                if budget.bytes == 0 {
                    return (Phase::Hashing { index, open, file: Some(hashing) }, false);
                }
                let chunk = self.chunk.as_mut();
                let want = (hashing.left as usize).min(chunk.len()).min(budget.bytes as usize);
                let Some(into) = chunk.get_mut(..want) else { break };
                match source.read_at(hashing.at, into) {
                    Some(n) if n == want => {
                        hashing.sha.update(into);
                        hashing.at += n as u64;
                        hashing.left -= n as u32;
                        budget.bytes -= n as u32;
                    }
                    _ => break,
                }
            }
            let answer = if hashing.left == 0 {
                let digest = hashing.sha.finish();
                let mut sha256 = [0; PREFIX];
                sha256.copy_from_slice(&digest[..PREFIX]);
                Answer::Hashed { size: hashing.size, sha256 }
            } else {
                Answer::Unreadable
            };
            self.answer(index, answer);
            index += 1;
        }
        (Phase::Whole, false)
    }

    fn answer(&mut self, index: u16, answer: Answer) {
        if let Some(slot) = self.slots.as_mut().get_mut(usize::from(index)) {
            *slot = Slot::Answered(answer);
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::collections::BTreeMap;
    use std::vec::Vec;

    use super::super::table::tests::{archive, Node};
    use super::*;

    /// Archives in memory.
    struct Game(BTreeMap<u8, Vec<u8>>, Vec<u8>);

    struct Bytes(Vec<u8>);
    impl Source for Bytes {
        fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Option<usize> {
            let rest = self.0.get(usize::try_from(offset).ok()?..)?;
            let n = rest.len().min(into.len());
            into[..n].copy_from_slice(&rest[..n]);
            Some(n)
        }
    }

    struct Buffer(Vec<u8>);
    impl AsMut<[u8]> for Buffer {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }
    struct Slots(Vec<Slot>);
    impl AsMut<[Slot]> for Slots {
        fn as_mut(&mut self) -> &mut [Slot] {
            &mut self.0
        }
    }

    impl Disk for Game {
        type File = Bytes;
        type Bytes = Buffer;
        type Slots = Slots;
        fn open(&mut self, archive: Archive) -> Option<Bytes> {
            match archive {
                Archive::Db => Some(Bytes(self.1.clone())),
                Archive::Patch(n) => self.0.get(&n).cloned().map(Bytes),
            }
        }
        fn bytes(&mut self, len: usize) -> Option<Buffer> {
            Some(Buffer(std::vec![0; len]))
        }
        fn slots(&mut self, count: usize) -> Option<Slots> {
            Some(Slots(std::vec![Slot::Unresolved; count]))
        }
    }

    fn request(paths: &[&str]) -> Vec<u8> {
        let mut out = std::vec![0; srp_validate::checks::MAX_REQUEST];
        let len = Request::encode(1, paths.iter().map(|p| p.as_bytes()), &mut out).unwrap();
        out.truncate(len);
        out
    }

    fn hashed(bytes: &[u8]) -> Answer {
        let digest = {
            let mut sha = Sha256::new();
            sha.update(bytes);
            sha.finish()
        };
        Answer::Hashed { size: bytes.len() as u32, sha256: digest[..PREFIX].try_into().unwrap() }
    }

    fn game() -> Game {
        let db = archive(&[
            Node::Folder("textures"),
            Node::File("textures/a.dds", b"db a"),
            Node::File("textures/b.dds", b"db b"),
            Node::File("startup.cfg", b"db cfg"),
        ]);
        let twelve = archive(&[Node::Folder("textures"), Node::File("textures/a.dds", b"12 a")]);
        let fourteen = archive(&[Node::File("startup.cfg", &[7u8; 200_000])]);
        Game(BTreeMap::from([(12, twelve), (14, fourteen)]), db)
    }

    fn run_all(game: &mut Game, paths: &[&str], mounts: &[u8], budget: Budget) -> (Vec<Answer>, usize) {
        let bytes = request(paths);
        let mut run = Run::new(game, &bytes, Mounts::of(mounts).unwrap()).unwrap();
        let mut steps = 0;
        while !run.whole() {
            run.step(game, budget);
            steps += 1;
            assert!(steps < 10_000, "a run that never ends");
        }
        (run.report().answers().to_vec(), steps)
    }

    const PATHS: [&str; 4] = ["none.cfg", "startup.cfg", "textures/a.dds", "textures/b.dds"];
    const ALL: Budget = Budget { lookups: u32::MAX, bytes: u32::MAX };

    #[test]
    fn each_file_is_the_last_mounted_archives() {
        let (answers, steps) = run_all(&mut game(), &PATHS, &[12, 14], ALL);
        assert_eq!(steps, 1);
        assert_eq!(
            answers,
            [Answer::Missing, hashed(&[7u8; 200_000]), hashed(b"12 a"), hashed(b"db b")]
        );
    }

    #[test]
    fn a_patch_not_mounted_is_not_read() {
        let (answers, _) = run_all(&mut game(), &PATHS, &[12], ALL);
        assert_eq!(answers[1], hashed(b"db cfg"));
    }

    #[test]
    fn a_small_budget_gives_the_same_answer_in_more_steps() {
        let (all, _) = run_all(&mut game(), &PATHS, &[12, 14], ALL);
        let (small, steps) = run_all(&mut game(), &PATHS, &[12, 14], Budget { lookups: 1, bytes: 1000 });
        assert_eq!(small, all);
        assert!(steps > 200, "{steps} steps");
    }

    #[test]
    fn an_archive_that_cannot_be_read_leaves_what_is_below_it_unknown() {
        let mut game = game();
        game.0.insert(14, b"not an archive".to_vec());
        let (answers, _) = run_all(&mut game, &PATHS, &[12, 14], ALL);
        assert_eq!(answers, [Answer::Unreadable; 4]);
        // A patch file gone since it was mounted, the same.
        let mut game = super::tests::game();
        game.0.remove(&12);
        let (answers, _) = run_all(&mut game, &PATHS, &[12, 14], ALL);
        assert_eq!(answers[1], hashed(&[7u8; 200_000]), "found above the gap");
        assert_eq!(answers[2], Answer::Unreadable);
    }

    #[test]
    fn a_file_cut_short_is_unreadable() {
        let mut game = game();
        let fourteen = game.0.get_mut(&14).unwrap();
        fourteen.truncate(fourteen.len() - 10);
        let (answers, _) = run_all(&mut game, &PATHS, &[12, 14], ALL);
        assert_eq!(answers[1], Answer::Unreadable);
        assert_eq!(answers[3], hashed(b"db b"));
    }
}

/// The players' archives against the server's data: every project's
/// checked files, resolved and hashed as the game does, are the sums the
/// server's data has (`resources_db::checked`). Opt in: SKHID_TEST_DB
/// (resources.db), SKHID_TEST_PATCHES (a folder of the release's
/// resources.db.patch_<n>), SKHID_TEST_CHECKED (the data's srp/checked).
#[cfg(test)]
mod live {
    extern crate std;
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{Read, Seek, SeekFrom};
    use std::string::String;
    use std::vec::Vec;

    use super::*;

    struct File(std::fs::File);
    impl Source for File {
        fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Option<usize> {
            self.0.seek(SeekFrom::Start(offset)).ok()?;
            let mut done = 0;
            while done < into.len() {
                match self.0.read(&mut into[done..]).ok()? {
                    0 => break,
                    n => done += n,
                }
            }
            Some(done)
        }
    }
    struct Buffer(Vec<u8>);
    impl AsMut<[u8]> for Buffer {
        fn as_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
    }
    struct Slots(Vec<Slot>);
    impl AsMut<[Slot]> for Slots {
        fn as_mut(&mut self) -> &mut [Slot] {
            &mut self.0
        }
    }
    struct Game {
        db: std::path::PathBuf,
        patches: std::path::PathBuf,
    }
    impl Disk for Game {
        type File = File;
        type Bytes = Buffer;
        type Slots = Slots;
        fn open(&mut self, archive: Archive) -> Option<File> {
            let path = match archive {
                Archive::Db => self.db.clone(),
                Archive::Patch(n) => self.patches.join(std::format!("resources.db.patch_{n}")),
            };
            std::fs::File::open(path).ok().map(File)
        }
        fn bytes(&mut self, len: usize) -> Option<Buffer> {
            Some(Buffer(std::vec![0; len]))
        }
        fn slots(&mut self, count: usize) -> Option<Slots> {
            Some(Slots(std::vec![Slot::Unresolved; count]))
        }
    }

    #[test]
    #[ignore = "reads the players' archives: SKHID_TEST_DB, SKHID_TEST_PATCHES, SKHID_TEST_CHECKED"]
    fn the_players_archives_hash_as_the_servers_data_says() {
        let var = |name: &str| std::env::var_os(name).unwrap_or_else(|| panic!("{name}"));
        let mut game = Game { db: var("SKHID_TEST_DB").into(), patches: var("SKHID_TEST_PATCHES").into() };
        let text = std::fs::read_to_string(var("SKHID_TEST_CHECKED")).unwrap();
        let mut sums: BTreeMap<String, (u32, Vec<u8>)> = BTreeMap::new();
        let mut projects: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut always = BTreeSet::new();
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            match fields[..] {
                ["file", path, size, sha256] => {
                    let bytes = (0..8).map(|i| u8::from_str_radix(&sha256[i * 2..i * 2 + 2], 16).unwrap()).collect();
                    sums.insert(path.into(), (size.parse().unwrap(), bytes));
                }
                ["project", project, path] => {
                    projects.entry(project.into()).or_default().insert(path.into());
                }
                ["always", path] => {
                    always.insert(String::from(path));
                }
                _ => panic!("{line}"),
            }
        }
        let mut mounted: Vec<u8> = std::fs::read_dir(&game.patches)
            .unwrap()
            .filter_map(|entry| {
                let name = entry.ok()?.file_name().into_string().ok()?;
                name.strip_prefix("resources.db.patch_")?.parse().ok()
            })
            .collect();
        mounted.sort();
        let mounts = Mounts::of(&mounted).unwrap();
        let (mut files, mut bytes) = (0usize, 0u64);
        for (project, own) in &projects {
            let paths: BTreeSet<&str> = own.iter().chain(&always).map(String::as_str).collect();
            let request = {
                let mut out = std::vec![0; srp_validate::checks::MAX_REQUEST];
                let len = Request::encode(1, paths.iter().map(|p| p.as_bytes()), &mut out).unwrap();
                out.truncate(len);
                out
            };
            let mut run = Run::new(&mut game, &request, mounts).unwrap();
            while !run.whole() {
                run.step(&mut game, Budget { lookups: 64, bytes: 256 * 1024 });
            }
            let report = run.report();
            for (path, answer) in paths.iter().zip(report.answers()) {
                let (size, prefix) = &sums[*path];
                let expected = Answer::Hashed { size: *size, sha256: prefix[..].try_into().unwrap() };
                assert_eq!(*answer, expected, "{project}: {path}");
                files += 1;
                bytes += u64::from(*size);
            }
        }
        std::eprintln!("{} projects, {files} files, {} MB: as the server's data", projects.len(), bytes / 1_000_000);
    }
}
