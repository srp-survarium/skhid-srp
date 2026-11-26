//! [incremental grids] Prints `mapdata`'s source digests of a source tree, as a build of it bakes them (`mapdata::source`):
//!
//! ```text
//! mapdata-srchash <srp-026g folder>
//! ```
//!
//! `SRC_ALL`, `SRC_GRID`, `SRC_SIGHT` (32 hex digits each), the 16-digit names the bench's grid cache gives its files, then the
//! report (each root's items by file). The `--cfg` rustflags are read from the environment as a build reads them.
//! `--why <root> <name>`, `--wide <root>`, `--diff <other srp-026g folder>` (why a key moved) and `--dropped` (what the scoped
//! resolver leaves out of each key) print reports instead.

use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(workspace) = std::env::args().nth(1) else {
        eprintln!("usage: mapdata-srchash <srp-026g folder>");
        return ExitCode::from(2);
    };
    // `--why <root> <name>`: how the root reaches the name; `--wide <root>`: the root's widest plain names; `--diff <other tree>`:
    // per digest, the reached items whose text differs; `--dropped`: per digest, what the scoped resolver leaves out of the
    // legacy one's closure.
    let args: Vec<String> = std::env::args().collect();
    if let [_, _, flag] = args.as_slice()
        && (flag == "--dropped" || flag == "--spans")
    {
        let report = if flag == "--dropped" {
            mapdata_srchash::dropped_report(Path::new(&workspace))
        } else {
            mapdata_srchash::spans_report(Path::new(&workspace))
        };
        return match report {
            Ok(report) => {
                print!("{report}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if let [_, _, flag, root, rest @ ..] = args.as_slice() {
        let source = match mapdata_srchash::mapdata_source(Path::new(&workspace)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        };
        match (flag.as_str(), rest) {
            ("--why", [target]) => match source.why(&[root], target) {
                Ok(Some(chain)) => chain.iter().for_each(|l| println!("{l}")),
                Ok(None) => println!("{root} does not reach {target}"),
                Err(e) => eprintln!("error: {e}"),
            },
            ("--diff", []) => match mapdata_srchash::mapdata_source(Path::new(root.as_str())) {
                Ok(other) => match mapdata_srchash::diff_report(&source, &other) {
                    Ok(report) => print!("{report}"),
                    Err(e) => eprintln!("error: {e}"),
                },
                Err(e) => eprintln!("error: {e}"),
            },
            ("--wide", []) => match source.closure(&[root]) {
                Ok(reached) => {
                    for (name, units, uses) in source.widest_names(&reached, 40) {
                        println!("{name}: names {units} items, used by {uses}");
                    }
                }
                Err(e) => eprintln!("error: {e}"),
            },
            _ => eprintln!("usage: mapdata-srchash <srp-026g folder> [--why <root> <name> | --wide <root> | --diff <other srp-026g folder> | --dropped | --spans]"),
        }
        return ExitCode::SUCCESS;
    }
    let encoded = std::env::var("CARGO_ENCODED_RUSTFLAGS").ok();
    let plain = std::env::var("RUSTFLAGS").ok();
    let cfgs = mapdata_srchash::rustflags_cfgs(encoded.as_deref(), plain.as_deref());
    match mapdata_srchash::mapdata_digests(Path::new(&workspace), &cfgs) {
        Ok(d) => {
            println!("SRC_ALL {:032x}", d.all);
            println!("SRC_GRID {:032x}", d.grid);
            println!("SRC_SIGHT {:032x}", d.sight);
            for (id, digest) in &d.sites {
                println!("SITE {id} {digest:032x}");
            }
            println!("cache names: bench-grid-{:016x}-*, bench-sight-{:016x}-*", (d.grid >> 64) as u64, (d.sight >> 64) as u64);
            print!("{}", d.report);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
