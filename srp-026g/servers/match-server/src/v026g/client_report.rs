//! What players' games tell about themselves (client patch 126 on,
//! `srp_validate::report`, `docs/notes/memory-report.md`): the one owner of
//! a carrier once the game has taken it. Each kind has its own log target
//! and so its own file (`servers/srp/src/logger.rs`): `memory` for the
//! memory reports, read by `release/moderate memory`. Only logged: no ban,
//! nothing a player sees, nothing in the db.

use srp_validate::memory::{Entry, Usage};
use srp_validate::report::{Body, Carrier};

/// The log target of the memory reports (`logs/memory.log`).
pub const MEMORY: &str = "memory";

/// Logs one part of a report, of the player `name` (account `account`)
/// in match `match_id`.
pub fn log(carrier: &Carrier, name: &str, account: u32, match_id: u32) {
    match carrier.body {
        Body::Memory(_) => log::info!(target: MEMORY, "{}", memory_line(carrier, name, account, match_id)),
    }
}

/// A memory part's line: `<name> (account <id>), match <id>, <moment>,
/// part <n>/<parts>: process free <bytes> largest <bytes> video <bytes> |
/// arena "<name>" size <bytes> used <bytes> largest <bytes> | arena "..."
/// size <bytes> counted <bytes> | arena "..." size <bytes> unknown`,
/// bytes in decimal.
fn memory_line(carrier: &Carrier, name: &str, account: u32, match_id: u32) -> String {
    let Body::Memory(part) = carrier.body;
    let entries: Vec<String> = part
        .entries()
        .iter()
        .map(|entry| match entry {
            Entry::Process(process) => format!(
                "process free {} largest {} video {}",
                process.free, process.largest_free, process.video_budget
            ),
            Entry::Arena(arena) => {
                let usage = match arena.usage {
                    Usage::Unknown => "unknown".to_string(),
                    Usage::Counted { used } => format!("counted {used}"),
                    Usage::Heap { used, largest_free } => format!("used {used} largest {largest_free}"),
                };
                format!("arena {:?} size {} {usage}", arena.name.as_str(), arena.size)
            }
        })
        .collect();
    format!(
        "{name} (account {account}), match {match_id:#x}, {}, part {}/{}: {}",
        part.moment.word(),
        u16::from(carrier.part) + 1,
        carrier.parts,
        entries.join(" | ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use srp_validate::memory::{Arena, Moment, Name, Process, Report};

    #[test]
    fn a_memory_part_is_one_line() {
        let mut report = Report::new(Moment::Loaded, Process { free: 300, largest_free: 200, video_budget: 1 << 32 });
        report.add(Arena { name: Name::of_c(b"render"), size: 1000, usage: Usage::Heap { used: 900, largest_free: 40 } });
        report.add(Arena { name: Name::of_c(b"global multithreaded"), size: 800, usage: Usage::Counted { used: 10 } });
        report.add(Arena { name: Name::of_c(b"process heap"), size: 0, usage: Usage::Unknown });
        let report = srp_validate::report::Report::Memory(report);
        let (bytes, len) = report.part(0x8000_0001, 0).unwrap();
        let carrier = Carrier::parse(&bytes[..len]).unwrap();
        assert_eq!(
            memory_line(&carrier, "Ivan", 7, 0x8000_0001),
            "Ivan (account 7), match 0x80000001, loaded, part 1/1: process free 300 largest 200 video 4294967296 \
             | arena \"render\" size 1000 used 900 largest 40 | arena \"global multithreaded\" size 800 counted 10 \
             | arena \"process heap\" size 0 unknown"
        );
    }
}
