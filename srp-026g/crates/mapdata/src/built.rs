//! Built walk grids and sight trees kept on disk, so a server reads one
//! instead of building it (`docs/notes/player_bots_mapdata.md` §6): no
//! build peak (near twice the grid's memory) and none of the build's
//! seconds of a core.
//!
//! One file per grid or tree, named by the key of what it is built from
//! ([`crate::grid_key`], [`crate::sight_key`]): projects of one geometry
//! share it, and a file of another build (another key) is never found.
//! Little endian:
//!
//! ```text
//! magic "SRPGRID\0", "SRPSGHT\0" or "SRPTACT\0", u32 version (1)
//! u64 key, u64 digest (WalkGrid::digest / RayTree::digest)
//! the structure's fields and arrays (grid.rs, sight.rs: `write_body`)
//! magic "END\0"
//! ```
//!
//! Reading checks every count against the bytes left and every index
//! against its array, then the digest: a damaged or stale file is an
//! error, never a panic and never a grid that answers otherwise. The
//! caller builds instead.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::resources::Error;

pub const GRID_MAGIC: &[u8; 8] = b"SRPGRID\0";
pub const SIGHT_MAGIC: &[u8; 8] = b"SRPSGHT\0";
/// A map's tactics (the match server's `TacticsBase`).
pub const TACTICS_MAGIC: &[u8; 8] = b"SRPTACT\0";
pub const VERSION: u32 = 1;
const END: &[u8; 4] = b"END\0";
/// Bytes read at a time.
const CHUNK: usize = 1 << 16;

/// The file of a built grid: `grid-<key>.bin`.
pub fn grid_file_name(key: u64) -> String {
    format!("grid-{key:016x}.bin")
}

/// The file of a map's built tactics: `tactics-<key>.bin`.
pub fn tactics_file_name(key: u64) -> String {
    format!("tactics-{key:016x}.bin")
}

/// The file of a built sight tree: `sight-<key>.bin`.
pub fn sight_file_name(key: u64) -> String {
    format!("sight-{key:016x}.bin")
}

fn error(path: &Path, what: impl std::fmt::Display) -> Error {
    Error(format!("{}: {what}", path.display()))
}

/// Writes a file: the header, `body`, the end mark. Through a temporary
/// file renamed at the end, so a reader never sees half of one.
pub fn write(
    path: &Path,
    magic: &[u8; 8],
    key: u64,
    digest: u64,
    body: impl FnOnce(&mut Out) -> std::io::Result<()>,
) -> Result<(), Error> {
    let temporary = path.with_extension("tmp");
    let result = (|| {
        let mut out = Out(BufWriter::new(File::create(&temporary)?));
        out.bytes(magic)?;
        out.u32(VERSION)?;
        out.u64(key)?;
        out.u64(digest)?;
        body(&mut out)?;
        out.bytes(END)?;
        out.0.into_inner().map_err(|e| e.into_error())?.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    result.map_err(|e| {
        let _ = std::fs::remove_file(&temporary);
        error(path, e)
    })
}

/// Reads a file written by [`write`] for `key`: `body` reads the
/// structure, which must then give the stored digest (`digest_of`).
pub fn read<T>(
    path: &Path,
    magic: &[u8; 8],
    key: u64,
    body: impl FnOnce(&mut In) -> Result<T, String>,
    digest_of: impl FnOnce(&T) -> u64,
) -> Result<T, Error> {
    let file = File::open(path).map_err(|e| error(path, e))?;
    let left = file.metadata().map_err(|e| error(path, e))?.len();
    let mut input = In {
        reader: BufReader::with_capacity(CHUNK, file),
        left,
    };
    let header = (|| {
        if &input.array::<8>()? != magic {
            return Err("not a built map file of this kind".to_owned());
        }
        let version = input.u32()?;
        if version != VERSION {
            return Err(format!("format version {version}, not {VERSION}"));
        }
        let stored = input.u64()?;
        if stored != key {
            return Err(format!("built for key {stored:016x}, not {key:016x}"));
        }
        input.u64()
    })();
    let digest = header.map_err(|e| error(path, e))?;
    let value = body(&mut input).map_err(|e| error(path, e))?;
    let end = input.array::<4>().map_err(|e| error(path, e))?;
    if &end != END || input.left != 0 {
        return Err(error(path, "no end mark where the data ends"));
    }
    let found = digest_of(&value);
    if found != digest {
        return Err(error(
            path,
            format!("digest {found:016x}, stored {digest:016x}"),
        ));
    }
    Ok(value)
}

pub struct Out(BufWriter<File>);

impl Out {
    pub fn bytes(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.0.write_all(bytes)
    }

    pub fn u32(&mut self, v: u32) -> std::io::Result<()> {
        self.bytes(&v.to_le_bytes())
    }

    pub fn u64(&mut self, v: u64) -> std::io::Result<()> {
        self.bytes(&v.to_le_bytes())
    }

    pub fn f32(&mut self, v: f32) -> std::io::Result<()> {
        self.u32(v.to_bits())
    }

    /// A count, then each item as `N` bytes ([`In::plain`]).
    pub fn plain<const N: usize, T>(
        &mut self,
        items: &[T],
        to: impl Fn(&T) -> [u8; N],
    ) -> std::io::Result<()> {
        self.u64(items.len() as u64)?;
        items.iter().try_for_each(|item| self.bytes(&to(item)))
    }
}

pub struct In {
    reader: BufReader<File>,
    /// Bytes the file has left.
    left: u64,
}

impl In {
    /// Whether only the end mark is left: a section written only when it
    /// holds something ends here when it does not.
    pub fn ends(&self) -> bool {
        self.left == END.len() as u64
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        if self.left < N as u64 {
            return Err("the file ends early".into());
        }
        let mut bytes = [0u8; N];
        self.reader
            .read_exact(&mut bytes)
            .map_err(|e| e.to_string())?;
        self.left -= N as u64;
        Ok(bytes)
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }

    /// A list written by [`Out::plain`]: `N` bytes an item, read a chunk
    /// at a time.
    pub fn plain<const N: usize, T>(
        &mut self,
        from: impl Fn([u8; N]) -> T,
    ) -> Result<Vec<T>, String> {
        let count = self.u64()?;
        let bytes = count
            .checked_mul(N as u64)
            .filter(|&bytes| bytes <= self.left)
            .ok_or_else(|| format!("{count} items do not fit in the file"))?;
        let mut items = Vec::with_capacity(count as usize);
        let mut buffer = vec![0u8; CHUNK - CHUNK % N];
        let mut remaining = bytes as usize;
        while remaining > 0 {
            let take = remaining.min(buffer.len());
            self.reader
                .read_exact(&mut buffer[..take])
                .map_err(|e| e.to_string())?;
            let (groups, _) = buffer[..take].as_chunks::<N>();
            items.extend(groups.iter().map(|group| from(*group)));
            remaining -= take;
        }
        self.left -= bytes;
        Ok(items)
    }
}
