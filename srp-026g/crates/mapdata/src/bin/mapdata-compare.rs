//! [incremental grids] Compares two built map files (grids, sight trees: `crate::built`) byte for byte:
//!
//! ```text
//! mapdata-compare <a.bin> <b.bin>
//! ```
//!
//! Prints each file's header (kind, version, key, digest) and size, then `SAME` (exit 0) or `DIFFERENT` with the first offset
//! where they differ, the bytes around it in both, and how many bytes differ (exit 1). An unreadable file: exit 2.

use std::path::Path;
use std::process::ExitCode;

/// The header: magic (8), version (u32), key (u64), digest (u64).
const HEADER: usize = 28;
/// Bytes shown each side of the first difference.
const AROUND: usize = 16;

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let [a, b] = paths.as_slice() else {
        eprintln!("usage: mapdata-compare <a.bin> <b.bin>");
        return ExitCode::from(2);
    };
    let read = |path: &str| std::fs::read(path).map_err(|e| format!("{path}: {e}"));
    let (a_bytes, b_bytes) = match (read(a), read(b)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    for (path, bytes) in [(a, &a_bytes), (b, &b_bytes)] {
        println!("{}: {}", Path::new(path).display(), header(bytes));
    }
    match compare(&a_bytes, &b_bytes) {
        Comparison::Same => {
            println!("SAME: {} bytes", a_bytes.len());
            ExitCode::SUCCESS
        }
        Comparison::Different { first, differing } => {
            println!(
                "DIFFERENT: first at byte {first} ({}), {differing} bytes differ in the {} both have, sizes {} and {}",
                if first < HEADER { "in the header" } else { "in the body" },
                a_bytes.len().min(b_bytes.len()),
                a_bytes.len(),
                b_bytes.len()
            );
            let from = first.saturating_sub(AROUND);
            for (name, bytes) in [("a", &a_bytes), ("b", &b_bytes)] {
                let to = (first + AROUND).min(bytes.len());
                println!("  {name} [{from}..{to}]: {}", hex(&bytes[from.min(to)..to]));
            }
            ExitCode::from(1)
        }
    }
}

#[derive(Debug, PartialEq)]
enum Comparison {
    Same,
    /// The first differing offset (or the shorter file's end), and how many bytes differ: in the common length, plus the
    /// longer file's extra bytes.
    Different { first: usize, differing: usize },
}

fn compare(a: &[u8], b: &[u8]) -> Comparison {
    let common = a.len().min(b.len());
    let mismatched = a[..common].iter().zip(&b[..common]).filter(|(x, y)| x != y).count();
    let differing = mismatched + a.len().max(b.len()) - common;
    if differing == 0 {
        return Comparison::Same;
    }
    let first = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(common);
    Comparison::Different { first, differing }
}

fn header(bytes: &[u8]) -> String {
    if bytes.len() < HEADER {
        return format!("{} bytes, shorter than a header", bytes.len());
    }
    let magic = String::from_utf8_lossy(&bytes[..8]).trim_end_matches('\0').to_owned();
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
    format!("{magic} version {} key {:016x} digest {:016x}, {} bytes", u32_at(8), u64_at(12), u64_at(20), bytes.len())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_first_difference_and_counts_them() {
        assert_eq!(compare(b"abcdef", b"abcdef"), Comparison::Same);
        assert_eq!(compare(b"abcdef", b"abXdeY"), Comparison::Different { first: 2, differing: 2 });
        assert_eq!(compare(b"abc", b"abcde"), Comparison::Different { first: 3, differing: 2 });
        assert_eq!(compare(b"", b""), Comparison::Same);
    }

    #[test]
    fn reads_a_header() {
        let mut bytes = b"SRPGRID\0".to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(0xabu64.to_le_bytes());
        bytes.extend(0xcdu64.to_le_bytes());
        assert_eq!(header(&bytes), "SRPGRID version 1 key 00000000000000ab digest 00000000000000cd, 28 bytes");
        assert_eq!(header(b"x"), "1 bytes, shorter than a header");
    }
}
