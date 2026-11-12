//! What players' games tell about themselves (client patches 125 and 126,
//! `srp_validate::report`, `docs/notes/memory-report.md`,
//! `docs/notes/module-report.md`): the one owner of a carrier once the game
//! has taken it. Each kind has its own log target and so its own file
//! (`servers/srp/src/logger.rs`): `memory` for the memory reports, read by
//! `release/moderate memory`, `modules` for the module reports, read by
//! `release/moderate modules`. Only logged: nothing a player sees, nothing
//! in the db.

use srp_validate::memory::{self, Entry, Usage};
use srp_validate::modules;
use srp_validate::report::{Body, Carrier};

/// The log target of the memory reports (`logs/memory.log`).
pub const MEMORY: &str = "memory";
/// The log target of the module reports (`logs/modules.log`).
pub const MODULES: &str = "modules";

/// Logs one part of a report, of the player `name` (account `account`)
/// in match `match_id`.
pub fn log(carrier: &Carrier, name: &str, account: u32, match_id: u32) {
    match carrier.body {
        Body::Memory(part) => log::info!(target: MEMORY, "{}", memory_line(carrier, &part, name, account, match_id)),
        Body::Modules(part) => log::info!(target: MODULES, "{}", modules_line(carrier, &part, name, account, match_id)),
    }
}

/// A memory part's line: `<name> (account <id>), match <id>, <moment>,
/// part <n>/<parts>: process free <bytes> largest <bytes> video <bytes> |
/// arena "<name>" size <bytes> used <bytes> largest <bytes> | arena "..."
/// size <bytes> counted <bytes> | arena "..." size <bytes> unknown`,
/// bytes in decimal.
fn memory_line(carrier: &Carrier, part: &memory::Part, name: &str, account: u32, match_id: u32) -> String {
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

/// A module part's line: `<name> (account <id>), match <id>, <moment>,
/// part <n>/<parts> of <seen> entries: <entry> | <entry> ...`, an entry one of
/// - `<place> "<name>" <image size> pe <stamp>/<checksum>` (a module),
/// - `code <memory> <protect> <exec size>/<size> start <fingerprint> pe ...`,
/// - `load <place> "<name>" <image size> pe ... at <seconds> listed|unlisted`,
/// - `over "<exe name>"|? class "<class>"|? <extended styles> <width>x<height> from <place>|windows|?`
///   (another process's always-on-top window over the game's, and where its exe is),
/// - `over unknown: <why>`,
/// - `exe <place> "<name>" sha256 <hex> size <bytes>` (the file of such a window's exe, read
///   through), or `... too big` / `... unread` where it was not,
///
/// sizes and addresses hex, seconds decimal, `-` for what is not there; a
/// place is `game`, `temp` or `other`, then ` in "<folders>"` where it has
/// any (below the game's or the temporary folder; an other one's from its
/// drive, a user's name `*`).
fn modules_line(carrier: &Carrier, part: &modules::Part, name: &str, account: u32, match_id: u32) -> String {
    use srp_validate::modules::{Entry, File, Home, Name, PeMarks, Place};
    let place = |place: Place| match place.dir.as_str() {
        "" => place.folder.word().to_string(),
        dir => format!("{} in {dir:?}", place.folder.word()),
    };
    let pe = |pe: Option<PeMarks>| pe.map_or_else(|| "-".to_string(), |pe| format!("{:08x}/{:x}", pe.stamp, pe.checksum));
    let entries: Vec<String> = part
        .entries()
        .iter()
        .map(|entry| match entry {
            Entry::Module(module) => {
                format!("{} {:?} {:x} pe {}", place(module.place), module.name, module.image_size, pe(module.pe))
            }
            Entry::Code(code) => format!(
                "code {} {} {:x}/{:x} start {} pe {}",
                code.memory.word(),
                code.protect.word(),
                code.exec_size,
                code.size,
                code.start.map_or_else(|| "-".to_string(), |start| format!("{start:08x}")),
                pe(code.pe)
            ),
            Entry::Load(load) => format!(
                "load {} {:?} {:x} pe {} at {} {}",
                place(load.place),
                load.name,
                load.image_size,
                pe(load.pe),
                load.at,
                if load.listed { "listed" } else { "unlisted" }
            ),
            Entry::Over(over) => {
                let quoted = |name: Option<Name>| name.map_or_else(|| "?".to_string(), |name| format!("{name:?}"));
                let home = match over.home {
                    Home::Unknown => "?".to_string(),
                    Home::Windows => "windows".to_string(),
                    Home::At(at) => place(at),
                };
                format!(
                    "over {} class {} {:x} {}x{} from {home}",
                    quoted(over.name),
                    quoted(over.class),
                    over.style,
                    over.width,
                    over.height
                )
            }
            Entry::Unseen(why) => format!("over unknown: {}", why.word()),
            Entry::Exe(exe) => {
                let file = match exe.file {
                    File::Hashed(identity) => format!("sha256 {identity} size {}", identity.size()),
                    File::TooBig => "too big".to_string(),
                    File::Unread => "unread".to_string(),
                };
                format!("exe {} {:?} {file}", place(exe.place), exe.name)
            }
        })
        .collect();
    format!(
        "{name} (account {account}), match {match_id:#x}, {}, part {}/{} of {} entries: {}",
        part.moment,
        u16::from(carrier.part) + 1,
        carrier.parts,
        part.seen,
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
        let Body::Memory(part) = carrier.body else { panic!("{carrier:?}") };
        assert_eq!(
            memory_line(&carrier, &part, "Ivan", 7, 0x8000_0001),
            "Ivan (account 7), match 0x80000001, loaded, part 1/1: process free 300 largest 200 video 4294967296 \
             | arena \"render\" size 1000 used 900 largest 40 | arena \"global multithreaded\" size 800 counted 10 \
             | arena \"process heap\" size 0 unknown"
        );
    }

    #[test]
    fn a_module_part_is_one_line() {
        use srp_validate::executable::Identity;
        use srp_validate::modules::{
            Entry, Exe, File, Folder, Home, Load, Module, Moment, Name, Over, PeMarks, Place, Report, Unseen,
        };
        let wide = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
        let downloads = Place::of(&wide("C:\\Users\\Ivan\\Downloads\\extra.dll"), &[], &[], &[]).unwrap();
        let shell = Some(PeMarks { stamp: 0x4b4b_4321, checksum: 0 });
        let name = |text: &str| Name::of_path(&text.encode_utf16().collect::<Vec<u16>>()).unwrap();
        let mut report = Report::new(Moment::Ended);
        for entry in [
            Entry::Over(Over { name: Some(name("overlay.exe")), class: Some(name("Overlay")), style: 0x8_00a8, width: 1280, height: 720, home: Home::At(downloads) }),
            Entry::Over(Over { name: Some(name("explorer.exe")), class: None, style: 0x88, width: 1280, height: 40, home: Home::Windows }),
            Entry::Unseen(Unseen::NoOrder),
            Entry::Exe(Exe { name: name("overlay.exe"), place: downloads, file: File::Hashed(Identity::new(4321, [0xab; 32]).unwrap()) }),
            Entry::Exe(Exe { name: name("big.exe"), place: Place::bare(Folder::Game), file: File::TooBig }),
            Entry::Module(Module { name: name("extra.dll"), place: downloads, image_size: 0x1_2000, pe: None }),
            Entry::Load(Load { name: name("added.dll"), place: Place::bare(Folder::Temp), image_size: 0x7000, pe: shell, at: 311, listed: false }),
        ] {
            report.add(entry);
        }
        let report = srp_validate::report::Report::Modules(report);
        let (bytes, len) = report.part(0x8000_0001, 0).unwrap();
        let carrier = Carrier::parse(&bytes[..len]).unwrap();
        let Body::Modules(part) = carrier.body else { panic!("{carrier:?}") };
        assert_eq!(
            modules_line(&carrier, &part, "Ivan", 7, 0x8000_0001),
            "Ivan (account 7), match 0x80000001, ended, part 1/1 of 7 entries: \
             over \"overlay.exe\" class \"Overlay\" 800a8 1280x720 from other in \"C:\\\\Users\\\\*\\\\Downloads\" \
             | over \"explorer.exe\" class ? 88 1280x40 from windows \
             | over unknown: game window not in the order \
             | exe other in \"C:\\\\Users\\\\*\\\\Downloads\" \"overlay.exe\" sha256 abababababababababababababababababababababababababababababababab size 4321 \
             | exe game \"big.exe\" too big \
             | other in \"C:\\\\Users\\\\*\\\\Downloads\" \"extra.dll\" 12000 pe - \
             | load temp \"added.dll\" 7000 pe 4b4b4321/0 at 311 unlisted"
        );
    }

    #[test]
    fn a_minute_part_names_its_minute() {
        use srp_validate::modules::{Entry, Minute, Moment, Report, Unseen};
        let line = |entries: &[Entry]| {
            let mut report = Report::new(Moment::Minute(Minute::FIRST.next().unwrap()));
            entries.iter().for_each(|&entry| report.add(entry));
            let (bytes, len) = srp_validate::report::Report::Modules(report).part(0x8000_0001, 0).unwrap();
            let carrier = Carrier::parse(&bytes[..len]).unwrap();
            let Body::Modules(part) = carrier.body else { panic!("{carrier:?}") };
            modules_line(&carrier, &part, "Ivan", 7, 0x8000_0001)
        };
        assert_eq!(
            line(&[Entry::Unseen(Unseen::NoOrder)]),
            "Ivan (account 7), match 0x80000001, minute 2, part 1/1 of 1 entries: over unknown: game window not in the order"
        );
        assert_eq!(line(&[]), "Ivan (account 7), match 0x80000001, minute 2, part 1/1 of 0 entries: ");
    }
}
