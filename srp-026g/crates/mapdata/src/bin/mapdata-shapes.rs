//! Writes the game's own Bullet shapes of a `.mapdata` file (version 6, `mapdata::bullet`) as text, every value
//! as its `f32` bits, for laying them against the game's memory (`scripts/collision-world/bw_shapes.py`):
//!
//! ```text
//! mapdata-shapes <file.mapdata> <out.txt>
//! ```
//!
//! One line a record, fields split by a space; a transform is 12 bits words (3 basis rows, then the origin):
//!
//! ```text
//! object <placement> <library> <cgroup> <cmask> <transform>
//!   empty
//!   mesh <scaling x3> <first face> <end face>
//!   convex <type> <up axis or -> <scaling x3> <dims x3> <margin> <walker> <first face> <end face>
//!   compound <scaling x3> <children>
//!     child <transform>, then its mesh or convex line
//! ```
//!
//! The output is the game's content: keep it out of the repository.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mapdata::MapData;
use mapdata::bullet::{Convex, Mesh, Part, Shape};
use mapdata::controller::exe_gjk::Transform;

#[derive(Parser)]
#[command(
    about = "The game's Bullet shapes of a 0.26g map data file, as bits, for the collision-world check"
)]
struct Cli {
    /// The `.mapdata` file (version 6).
    input: PathBuf,
    /// The text file to write.
    output: PathBuf,
}

fn bits(out: &mut String, values: &[f32]) {
    for v in values {
        let _ = write!(out, " {:08x}", v.to_bits());
    }
}

fn transform(out: &mut String, t: &Transform) {
    for row in &t.basis {
        bits(out, row);
    }
    bits(out, &t.origin);
}

fn mesh(out: &mut String, m: &Mesh) {
    out.push_str("mesh");
    bits(out, &m.scaling);
    let _ = writeln!(out, " {} {}", m.faces.start, m.faces.end);
}

fn convex(out: &mut String, c: &Convex) {
    let up = c
        .kind
        .up()
        .map_or("-".to_string(), |up| up.index().to_string());
    let _ = write!(out, "convex {} {up}", c.kind.bullet_type());
    bits(out, &c.scaling);
    bits(out, &c.dims);
    bits(out, &[c.margin]);
    let _ = writeln!(out, " {} {} {}", c.walker as u8, c.faces.start, c.faces.end);
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let map = match MapData::read(&cli.input) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(world) = &map.bullet else {
        eprintln!(
            "{}: no game shapes (a file older than version 6): dump the map again",
            cli.input.display()
        );
        return ExitCode::FAILURE;
    };
    let mut out = String::new();
    for (index, placement) in map.placements.iter().enumerate() {
        let _ = write!(
            out,
            "object {index} {} {} {}",
            placement.library, placement.cgroup, placement.cmask
        );
        transform(&mut out, world.object(index));
        out.push('\n');
        match world.shape(placement.library) {
            Shape::Empty => out.push_str("empty\n"),
            Shape::Mesh(m) => mesh(&mut out, m),
            Shape::Convex(c) => convex(&mut out, c),
            Shape::Compound { scaling, children } => {
                out.push_str("compound");
                bits(&mut out, scaling);
                let _ = writeln!(out, " {}", children.len());
                for child in children {
                    out.push_str("child");
                    transform(&mut out, &child.transform);
                    out.push('\n');
                    match &child.part {
                        Part::Mesh(m) => mesh(&mut out, m),
                        Part::Convex(c) => convex(&mut out, c),
                    }
                }
            }
        }
    }
    if let Err(error) = std::fs::write(&cli.output, out) {
        eprintln!("{}: {error}", cli.output.display());
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
