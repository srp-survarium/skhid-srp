//! What players' games tell about themselves (client patch 126 on,
//! `srp_validate::report`, `docs/notes/memory-report.md`): the one owner of
//! a carrier once the game has taken it. Each kind has its own log target
//! and so its own file (`servers/srp/src/logger.rs`): `memory` for the
//! memory reports, read by `release/moderate memory`. Only logged: no ban,
//! nothing a player sees, nothing in the db.

use srp_validate::memory::{Census, Client, Entry, Renderer, Usage};
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
/// part <n>/<parts>: ` and its entries split by ` | `, bytes in decimal:
/// - `process free <bytes> largest <bytes> video <bytes>`;
/// - `arena "<name>" size <bytes> used <bytes> largest <bytes>`, or
///   `... counted <bytes>`, or `... unknown`;
/// - `census image <reserved>/<committed> in <allocations>, mapped ...,
///   private ...`;
/// - `client <windows|wine>, d3d11 <direct3d|dxvk> (<dxvk folder|system
///   folder|elsewhere>)` or `d3d11 unbound`, `, baked kept <count> of
///   <bytes>, listed <count> of <bytes>`;
/// - `allocation 0x<base> <image|mapped|private> "<file>" size <bytes>
///   committed <bytes>` (no file: no name).
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
            Entry::Census(census) => census_words(census),
            Entry::Client(client) => client_words(client),
            Entry::Allocation(allocation) => {
                let owner = allocation.owner();
                let file = owner.name().map(|name| format!(" {:?}", name.as_str())).unwrap_or_default();
                format!(
                    "allocation {:#x} {}{file} size {} committed {}",
                    allocation.base(),
                    owner.kind().word(),
                    allocation.size(),
                    allocation.committed()
                )
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

fn census_words(census: &Census) -> String {
    let each = [("image", census.image), ("mapped", census.mapped), ("private", census.private)]
        .map(|(kind, footprint)| format!("{kind} {}/{} in {}", footprint.reserved(), footprint.committed(), footprint.allocations()));
    format!("census {}", each.join(", "))
}

fn client_words(client: &Client) -> String {
    let d3d11 = match client.renderer {
        Renderer::Unbound => "unbound".to_string(),
        Renderer::Direct3d(folder) => format!("direct3d ({})", folder.word()),
        Renderer::Dxvk(folder) => format!("dxvk ({})", folder.word()),
    };
    let baked = client.textures;
    format!(
        "client {}, d3d11 {d3d11}, baked kept {} of {}, listed {} of {}",
        client.system.word(),
        baked.kept,
        baked.kept_bytes,
        baked.listed,
        baked.listed_bytes
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use srp_validate::memory::{Arena, Folder, Moment, Name, Process, Region, Report, System, Textures, Walk};

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

    #[test]
    fn the_census_the_client_and_the_largest_allocations_are_words() {
        let mut walk = Walk::new([(0x1000_0000, 0x100_0000)]);
        for (state, kind, base, size) in [
            (0x1000, 0x100_0000, 0x1_0000, 0x90_0000),
            (0x1_0000, 0, 0, 0x10_0000),
            (0x1000, 0x2_0000, 0x1000_0000, 0x100_0000),
            (0x1000, 0x4_0000, 0x2000_0000, 0x1000),
            (0x2000, 0x4_0000, 0x2000_0000, 0x3F_F000),
            (0x2000, 0x2_0000, 0x3000_0000, 0x10_0000),
        ] {
            walk.region(Region::of(state, kind, base, size));
        }
        let walked = walk.end();
        let mut report = Report::new(Moment::Ended, walked.process(1 << 30));
        report.add_census(walked.census());
        report.add_client(Client {
            system: System::Windows,
            renderer: Renderer::Direct3d(Folder::System),
            textures: Textures { kept: 12, kept_bytes: 5_000_000, listed: 40, listed_bytes: 20_000_000 },
        });
        let mut names = [Name::of_file(b"C:\\Game\\survarium_skhid.exe"), Name::of_file(b"\\Device\\HarddiskVolume3\\x\\resources.db"), None].into_iter();
        walked.largest().for_each(|found| report.add_allocation(found.named(names.next().flatten())));
        let report = srp_validate::report::Report::Memory(report);
        let (bytes, len) = report.part(5, 0).unwrap();
        let carrier = Carrier::parse(&bytes[..len]).unwrap();
        assert_eq!(
            memory_line(&carrier, "Ivan", 7, 5),
            "Ivan (account 7), match 0x5, ended, part 1/1: process free 1048576 largest 1048576 video 1073741824 \
             | census image 9437184/9437184 in 1, mapped 4194304/4096 in 1, private 17825792/16777216 in 2 \
             | client windows, d3d11 direct3d (system folder), baked kept 12 of 5000000, listed 40 of 20000000 \
             | allocation 0x10000 image \"survarium_skhid.exe\" size 9437184 committed 9437184 \
             | allocation 0x20000000 mapped \"resources.db\" size 4194304 committed 4096 \
             | allocation 0x30000000 private size 1048576 committed 0"
        );
    }
}
