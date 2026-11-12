//! Writes the libraries and placements of a `.mapdata` file in a flat form, for laying the export against
//! the game's own collision world (`scripts/collision-world/bw_diff.py`, `docs/notes/collision_world.md`):
//!
//! ```text
//! mapdata-flat <file.mapdata> <out.mdf>
//! ```
//!
//! Little endian:
//!
//! ```text
//! magic "SRPMDF1\n", u32 name_len, project name
//! u32 libraries; each: u32 name_len, name, u32 primitives, u32 vertices, 3 f32 each,
//!                      u32 faces, 3 u32 each, then one u8 walker behaviour per face
//! u32 placements; each: u32 library, 3 f32 position, 3 f32 rotation (angles), u16 cgroup, u16 cmask,
//!                       9 f32: where the placement's own `Transform` sends the local x, y and z axes
//! ```
//!
//! The output is the game's content: keep it out of the repository.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mapdata::{MapData, Vec3};

#[derive(Parser)]
#[command(about = "Libraries and placements of a 0.26g map data file, flat, for the collision-world diff")]
struct Cli {
    /// The `.mapdata` file.
    input: PathBuf,
    /// The flat file to write.
    output: PathBuf,
}

/// The file's bytes, written field by field.
#[derive(Default)]
struct Flat(Vec<u8>);

impl Flat {
    fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }

    /// A count: the file's lists are far under `u32::MAX`, and one that is not is a broken file.
    fn count(&mut self, count: usize) {
        self.u32(u32::try_from(count).expect("a list of a map data file fits a u32"));
    }

    fn vec3(&mut self, v: Vec3) {
        for f in [v.x, v.y, v.z] {
            self.0.extend_from_slice(&f.to_le_bytes());
        }
    }

    fn text(&mut self, text: &str) {
        self.count(text.len());
        self.0.extend_from_slice(text.as_bytes());
    }
}

fn flatten(map: &MapData) -> Vec<u8> {
    let mut out = Flat::default();
    out.0.extend_from_slice(b"SRPMDF1\n");
    out.text(&map.project);
    out.count(map.libraries.len());
    for library in &map.libraries {
        out.text(&library.name);
        out.u32(library.primitives);
        out.count(library.vertices.len());
        for &vertex in &library.vertices {
            out.vec3(vertex);
        }
        out.count(library.faces.len());
        for face in &library.faces {
            for corner in face.corners {
                out.u32(corner);
            }
        }
        out.0.extend(library.faces.iter().map(|face| face.surface.walker as u8));
    }
    out.count(map.placements.len());
    for placement in &map.placements {
        out.u32(placement.library);
        out.vec3(placement.position);
        out.vec3(placement.rotation);
        out.0.extend_from_slice(&placement.cgroup.to_le_bytes());
        out.0.extend_from_slice(&placement.cmask.to_le_bytes());
        let rotation = placement.transform().rotation;
        for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            out.vec3(rotation.apply(axis));
        }
    }
    out.0
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let map = match MapData::read(&cli.input) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("{}: {error:?}", cli.input.display());
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = std::fs::write(&cli.output, flatten(&map)) {
        eprintln!("{}: {error}", cli.output.display());
        return ExitCode::FAILURE;
    }
    println!(
        "{}: {} libraries, {} placements -> {}",
        map.project,
        map.libraries.len(),
        map.placements.len(),
        cli.output.display()
    );
    ExitCode::SUCCESS
}
