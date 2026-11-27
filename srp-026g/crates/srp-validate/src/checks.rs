//! Resource checks: the match server asks a player's game to hash files of
//! its archive ([`Request`], an 0xE4 of its own prefix, [`REQUEST_MAGIC`]),
//! and the game tells what it found in a client report of kind 4
//! ([`Report`], [`crate::report`]): which `resources.db.patch_<n>` it
//! mounted, and each file's size and the first bytes of its sha256, in the
//! request's order. The server compares them with its game data's
//! (`resources_db::checked`) and logs what differs.
//!
//! ```text
//! request: magic[4] u32 match_id u16 count  count x { u8 shared u8 len bytes[len] }
//!   a path is the one before's first `shared` bytes, then `bytes`; the
//!   paths in strictly ascending byte order, each 1 to MAX_PATH bytes of
//!   space to '~'
//! report kind 4, part 0: u8 0 (mounts)  u16 files  u8 count  count x u8 n
//!   the patches' n in ascending order, 1 to 255; files: the request's
//!   count, 0 in the offer (the mounts alone, before any request: the
//!   server asks only a game that offered)
//! report kind 4, part 1..: u8 1 (answers)  u16 first  u8 count  count x answer
//!   answer: u8 0 hashed: u32 size  u8 sha256[8]
//!         | u8 1 missing (no archive the game mounts has it)
//!         | u8 2 unreadable (an archive's table or bytes could not be read)
//! ```

/// Not a prefix of any 0xE4 the game reads: its own E4 has no 0xFF first
/// byte, and the executable's reply ([`crate::executable::REPLY_MAGIC`])
/// and the late join's proposal have other second bytes.
pub const REQUEST_MAGIC: [u8; 4] = [255, b'R', b'C', 1];
/// Files one request asks at most.
pub const MAX_FILES: usize = 512;
/// A path's bytes at most.
pub const MAX_PATH: usize = 255;
/// A request's bytes at most.
pub const MAX_REQUEST: usize = 16 * 1024;
const REQUEST_HEAD: usize = REQUEST_MAGIC.len() + 4 + 2;

/// A part's body at most: with the carrier's header it stays in one 0xE0
/// chunk ([`crate::report::MAX_SIZE`]).
pub const MAX_BODY: usize = 465;
const PART_HEAD: usize = 1 + 2 + 1;
/// The bytes of a hashed answer's sha256 the report tells.
pub const PREFIX: usize = 8;
const LONGEST_ANSWER: usize = 1 + 4 + PREFIX;
/// Answers in one part at most.
pub const PER_PART: usize = (MAX_BODY - PART_HEAD) / LONGEST_ANSWER;
/// Parts of one report: the mounts, then every answer the longest.
pub const MAX_PARTS: u8 = (1 + MAX_FILES.div_ceil(PER_PART)) as u8;
const _: () = assert!(PART_HEAD + 255 <= MAX_BODY);

/// A resource name can contain spaces (e.g. scavengers_02_chemical glass.material).
fn is_path_byte(byte: u8) -> bool {
    matches!(byte, b' '..=b'~')
}

/// A request as the game reads it: every path checked in [`Request::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    pub match_id: u32,
    count: u16,
    records: &'a [u8],
}

impl<'a> Request<'a> {
    /// Exactly a request: the magic, at most [`MAX_FILES`] paths, each
    /// valid and after the one before, nothing after the last.
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() > MAX_REQUEST {
            return None;
        }
        let (head, records) = bytes.split_at_checked(REQUEST_HEAD)?;
        let (magic, head) = head.split_at_checked(REQUEST_MAGIC.len())?;
        if magic != REQUEST_MAGIC {
            return None;
        }
        let (id, count) = head.split_at_checked(4)?;
        let match_id = u32::from_le_bytes(id.try_into().ok()?);
        let count = u16::from_le_bytes(count.try_into().ok()?);
        if usize::from(count) > MAX_FILES {
            return None;
        }
        let request = Request { match_id, count, records };
        let mut paths = request.paths();
        let mut seen = 0usize;
        let mut previous = [0u8; MAX_PATH];
        let mut previous_len = 0usize;
        while let Some(path) = paths.next_path() {
            let before = previous.get(..previous_len)?;
            if seen > 0 && path <= before {
                return None;
            }
            previous.get_mut(..path.len())?.copy_from_slice(path);
            previous_len = path.len();
            seen = seen.checked_add(1)?;
        }
        (paths.failed.is_none() && paths.rest.is_empty() && seen == usize::from(count))
            .then_some(request)
    }

    /// How many paths it asks.
    pub fn len(&self) -> u16 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Its paths, in order.
    pub fn paths(&self) -> Paths<'a> {
        Paths {
            rest: self.records,
            left: self.count,
            path: [0; MAX_PATH],
            len: 0,
            failed: None,
        }
    }

    /// The request of match `match_id` for `paths` into `out`, and its
    /// length; `None` when the paths are not in strictly ascending order,
    /// one is not a path ([`MAX_PATH`] bytes of space to '~'), there are more
    /// than [`MAX_FILES`], or they do not fit `out` or [`MAX_REQUEST`].
    pub fn encode<'p>(
        match_id: u32,
        paths: impl IntoIterator<Item = &'p [u8]>,
        out: &mut [u8],
    ) -> Option<usize> {
        let room = out.len().min(MAX_REQUEST);
        let out = out.get_mut(..room)?;
        let (head, mut rest) = out.split_at_mut_checked(REQUEST_HEAD)?;
        let mut count = 0u16;
        let mut used = REQUEST_HEAD;
        let mut previous: &[u8] = &[];
        for path in paths {
            if path.is_empty()
                || path.len() > MAX_PATH
                || !path.iter().all(|&byte| is_path_byte(byte))
                || (count > 0 && path <= previous)
            {
                return None;
            }
            let shared = previous.iter().zip(path).take_while(|(a, b)| a == b).count();
            let suffix = path.get(shared..)?;
            let record = 2usize.checked_add(suffix.len())?;
            let (here, after) = rest.split_at_mut_checked(record)?;
            let (lens, bytes) = here.split_at_mut_checked(2)?;
            lens.copy_from_slice(&[u8::try_from(shared).ok()?, u8::try_from(suffix.len()).ok()?]);
            bytes.copy_from_slice(suffix);
            rest = after;
            used = used.checked_add(record)?;
            count = count.checked_add(1)?;
            if usize::from(count) > MAX_FILES {
                return None;
            }
            previous = path;
        }
        let (magic, head) = head.split_at_mut_checked(REQUEST_MAGIC.len())?;
        magic.copy_from_slice(&REQUEST_MAGIC);
        let (id, len) = head.split_at_mut_checked(4)?;
        id.copy_from_slice(&match_id.to_le_bytes());
        len.copy_from_slice(&count.to_le_bytes());
        Some(used)
    }
}

/// A request's paths, one at a time: each lives in the reader until the
/// next ([`Paths::next_path`]).
pub struct Paths<'a> {
    rest: &'a [u8],
    left: u16,
    path: [u8; MAX_PATH],
    len: usize,
    /// Where it stopped on a record that is not a path.
    failed: Option<()>,
}

impl Paths<'_> {
    /// The next path; `None` after the last, or at a record that is not
    /// one (shared more than the path before, empty, too long, a byte not
    /// of a path).
    pub fn next_path(&mut self) -> Option<&[u8]> {
        if self.left == 0 || self.failed.is_some() {
            return None;
        }
        let path = self.step();
        if path.is_none() {
            self.failed = Some(());
            return None;
        }
        self.path.get(..self.len)
    }

    fn step(&mut self) -> Option<()> {
        let (lens, rest) = self.rest.split_at_checked(2)?;
        let &[shared, len] = lens else { return None };
        let (shared, len) = (usize::from(shared), usize::from(len));
        let (suffix, rest) = rest.split_at_checked(len)?;
        let total = shared.checked_add(len)?;
        if shared > self.len
            || total == 0
            || total > MAX_PATH
            || !suffix.iter().all(|&byte| is_path_byte(byte))
        {
            return None;
        }
        self.path.get_mut(shared..total)?.copy_from_slice(suffix);
        self.len = total;
        self.rest = rest;
        self.left = self.left.checked_sub(1)?;
        Some(())
    }
}

/// What a game found of one asked file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Its bytes as the game mounts them: their count and their sha256's
    /// first [`PREFIX`] bytes.
    Hashed { size: u32, sha256: [u8; PREFIX] },
    /// No archive the game mounts has it.
    Missing,
    /// An archive's table or the file's bytes could not be read.
    Unreadable,
}

impl Answer {
    fn told(&self) -> usize {
        match self {
            Answer::Hashed { .. } => LONGEST_ANSWER,
            Answer::Missing | Answer::Unreadable => 1,
        }
    }

    fn put(&self, out: &mut [u8]) -> Option<usize> {
        match self {
            Answer::Hashed { size, sha256 } => {
                let (kind, rest) = out.split_first_mut()?;
                *kind = 0;
                let (len, rest) = rest.split_at_mut_checked(4)?;
                len.copy_from_slice(&size.to_le_bytes());
                rest.get_mut(..PREFIX)?.copy_from_slice(sha256);
            }
            Answer::Missing => *out.first_mut()? = 1,
            Answer::Unreadable => *out.first_mut()? = 2,
        }
        Some(self.told())
    }

    fn parse(bytes: &[u8]) -> Option<(Answer, &[u8])> {
        let (&kind, rest) = bytes.split_first()?;
        match kind {
            0 => {
                let (size, rest) = rest.split_at_checked(4)?;
                let (sha256, rest) = rest.split_at_checked(PREFIX)?;
                let answer = Answer::Hashed {
                    size: u32::from_le_bytes(size.try_into().ok()?),
                    sha256: sha256.try_into().ok()?,
                };
                Some((answer, rest))
            }
            1 => Some((Answer::Missing, rest)),
            2 => Some((Answer::Unreadable, rest)),
            _ => None,
        }
    }
}

/// The `resources.db.patch_<n>` a game mounted: their n, ascending.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mounts {
    numbers: [u8; 255],
    count: u8,
}

impl Mounts {
    pub const NONE: Mounts = Mounts { numbers: [0; 255], count: 0 };

    /// `numbers` if they are patch numbers (1 to 255) in strictly ascending
    /// order, as the game mounts them.
    pub fn of(numbers: &[u8]) -> Option<Mounts> {
        let ascending = numbers.windows(2).all(|pair| matches!(pair, [a, b] if a < b));
        if !ascending || numbers.contains(&0) {
            return None;
        }
        let mut mounts = Mounts::NONE;
        mounts.numbers.get_mut(..numbers.len())?.copy_from_slice(numbers);
        mounts.count = u8::try_from(numbers.len()).ok()?;
        Some(mounts)
    }

    pub fn numbers(&self) -> &[u8] {
        self.numbers.get(..usize::from(self.count)).unwrap_or_default()
    }
}

/// A whole report, as the game gathers it: the mounts and one answer per
/// asked file, in the request's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    mounts: Mounts,
    answers: [Answer; MAX_FILES],
    count: u16,
}

impl Report {
    /// The offer: the mounts, no answer ([`Part::round`] 0).
    pub fn offer(mounts: Mounts) -> Report {
        Report::new(mounts)
    }

    /// An answer, its answers to add ([`Report::add`]).
    pub fn new(mounts: Mounts) -> Report {
        Report { mounts, answers: [Answer::Missing; MAX_FILES], count: 0 }
    }

    /// The answer to the next asked file; `None` past [`MAX_FILES`].
    pub fn add(&mut self, answer: Answer) -> Option<()> {
        *self.answers.get_mut(usize::from(self.count))? = answer;
        self.count = self.count.checked_add(1)?;
        Some(())
    }

    pub fn answers(&self) -> &[Answer] {
        self.answers.get(..usize::from(self.count)).unwrap_or_default()
    }

    fn answer_parts(&self) -> impl Iterator<Item = &[Answer]> {
        self.answers().chunks(PER_PART)
    }

    /// How many parts it goes in: the mounts, then the answers.
    pub fn parts(&self) -> u8 {
        let answers = self.answers().len().div_ceil(PER_PART);
        u8::try_from(answers.saturating_add(1)).unwrap_or(u8::MAX)
    }

    /// The body of part `part` (0-based) into `out`, and its length.
    pub(crate) fn put_part(&self, part: u8, out: &mut [u8]) -> Option<usize> {
        let (head, rest) = out.split_at_mut_checked(PART_HEAD)?;
        let Some(index) = part.checked_sub(1) else {
            let numbers = self.mounts.numbers();
            head.copy_from_slice(&[0, 0, 0, self.mounts.count]);
            head.get_mut(1..3)?.copy_from_slice(&self.count.to_le_bytes());
            rest.get_mut(..numbers.len())?.copy_from_slice(numbers);
            return PART_HEAD.checked_add(numbers.len());
        };
        let answers = self.answer_parts().nth(usize::from(index))?;
        let first = u16::try_from(usize::from(index).checked_mul(PER_PART)?).ok()?;
        head.copy_from_slice(&[1, 0, 0, u8::try_from(answers.len()).ok()?]);
        head.get_mut(1..3)?.copy_from_slice(&first.to_le_bytes());
        let mut used = PART_HEAD;
        let mut rest = rest;
        for answer in answers {
            let len = answer.put(rest)?;
            rest = rest.get_mut(len..)?;
            used = used.checked_add(len)?;
        }
        Some(used)
    }
}

/// A part as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// Part 0: the mounts, and how many files the request asked.
    Mounts { files: u16, mounts: Mounts },
    /// A later part: the answers to files `first` on.
    Answers { first: u16, answers: [Answer; PER_PART], count: u8 },
}

impl Part {
    /// Which of a connection's two reports it belongs to: 0 the offer
    /// (part 0 of no files), 1 the answer.
    pub fn round(&self) -> u8 {
        match self {
            Part::Mounts { files: 0, .. } => 0,
            Part::Mounts { .. } | Part::Answers { .. } => 1,
        }
    }

    /// The answers of an [`Part::Answers`]; none of the mounts.
    pub fn answers(&self) -> &[Answer] {
        match self {
            Part::Mounts { .. } => &[],
            Part::Answers { answers, count, .. } => {
                answers.get(..usize::from(*count)).unwrap_or_default()
            }
        }
    }

    /// Exactly a body: part 0 the mounts, a later one answers (at least
    /// one, at most [`PER_PART`], from where the parts before end).
    pub(crate) fn parse(bytes: &[u8], part: u8) -> Option<Part> {
        if bytes.len() > MAX_BODY {
            return None;
        }
        let (head, mut rest) = bytes.split_at_checked(PART_HEAD)?;
        let &[kind, low, high, count] = head else { return None };
        let number = u16::from_le_bytes([low, high]);
        match (kind, part.checked_sub(1)) {
            (0, None) => {
                let numbers = rest.get(..usize::from(count))?;
                let mounts = Mounts::of(numbers)?;
                (rest.len() == numbers.len() && usize::from(number) <= MAX_FILES)
                    .then_some(Part::Mounts { files: number, mounts })
            }
            (1, Some(index)) => {
                if count == 0 || usize::from(count) > PER_PART {
                    return None;
                }
                if usize::from(number) != usize::from(index).checked_mul(PER_PART)? {
                    return None;
                }
                let mut answers = [Answer::Missing; PER_PART];
                for answer in answers.iter_mut().take(usize::from(count)) {
                    (*answer, rest) = Answer::parse(rest)?;
                }
                rest.is_empty().then_some(Part::Answers { first: number, answers, count })
            }
            _ => None,
        }
    }
}

/// The length of the body that starts `body`: what the transport needs
/// ([`crate::report::carrier_len`]).
pub(crate) fn body_len(body: &[u8]) -> Option<usize> {
    let (head, mut rest) = body.split_at_checked(PART_HEAD)?;
    let &[kind, _, _, count] = head else { return None };
    let len = match kind {
        0 => usize::from(count),
        1 => {
            let mut len = 0usize;
            for _ in 0..count {
                let (answer, after) = Answer::parse(rest)?;
                len = len.checked_add(answer.told())?;
                rest = after;
            }
            len
        }
        _ => return None,
    };
    PART_HEAD.checked_add(len)
}

#[cfg(test)]
mod tests;
