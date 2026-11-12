//! The exe's cast port against the exe's own recorded `calcTimeOfImpact` calls. Runs only when
//! `SRP_EXE_TOI_CORPUS` names a corpus of raw records (and skips with a note otherwise): the records hold the game's
//! own shapes, so the corpus stays out of the repository.
//!
//! ```text
//! SRP_EXE_TOI_CORPUS=<corpus.raw> cargo test -p mapdata --release --test exe_toi_corpus -- --nocapture
//! ```
//!
//! A record is 0x400 bytes, little-endian, one call of `btContinuousConvexCollision::calcTimeOfImpact` 0xB9320 taken in
//! Unicorn while the 0.26g exe ran traced steps of the game on the game's real collision objects
//! (`docs/notes/exactness_rig.md`): shape A's bytes (+0: type +4, scaling +10h, implicit dims +20h, margin +30h, up
//! axis +40h), shape B's (+60h: the same fields; a triangle's corners at its +50h, +60h, +70h), the transforms fromA
//! (+E0h), toA (+120h), fromB (+160h), toB (+1A0h) (basis rows +0 / +10h / +20h, origin +30h), the cast result at entry
//! (+1E0h: the allowed penetration at its +B8h), the result at exit from its +90h on (+2A0h: normal, +2B0h: hit point,
//! +2C0h: fraction), AL at exit (+2D0h), the count of penetration-solver calls inside (+2D2h), which `ret` it left by
//! (+2D3h: 0xB982C, 0xB9843, 0xB985A), the return address (+2D4h: 0xB8772 is `processTriangle`), and for
//! `processTriangle` the callback's fraction at entry (+3D0h), whether it reported (+3D4h) and the report's normal
//! (+3D8h), point (+3E4h) and fraction (+3F0h).
use mapdata::controller::exe_cast::{Cast, CastEnd, Toi, cast_triangle, time_of_impact};
use mapdata::controller::exe_gjk::{Axis, CastShape, Transform, Triangle, WorldShape};

const RECORD: usize = 0x400;
const B: usize = 0x060;
const EXITS: [u32; 3] = [0xB982C, 0xB9843, 0xB985A];

fn u32_at(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

fn f32_at(r: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(r, at))
}

fn vec3_at(r: &[u8], at: usize) -> [f32; 3] {
    [f32_at(r, at), f32_at(r, at + 4), f32_at(r, at + 8)]
}

fn bits_at(r: &[u8], at: usize) -> [u32; 3] {
    [u32_at(r, at), u32_at(r, at + 4), u32_at(r, at + 8)]
}

fn bits(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

fn transform_at(r: &[u8], at: usize) -> Transform {
    Transform { basis: [vec3_at(r, at), vec3_at(r, at + 0x10), vec3_at(r, at + 0x20)], origin: vec3_at(r, at + 0x30) }
}

fn axis_at(r: &[u8], at: usize, index: usize) -> Axis {
    Axis::from_index(u32_at(r, at) as i32).unwrap_or_else(|| panic!("record {index}: an up axis of {}", u32_at(r, at)))
}

#[test]
fn every_recorded_cast_gives_the_exes_bits() {
    let Some(path) = std::env::var_os("SRP_EXE_TOI_CORPUS") else {
        eprintln!("SRP_EXE_TOI_CORPUS is not set: the corpus check is skipped");
        return;
    };
    let data = std::fs::read(&path).expect("the corpus file");
    assert!(data.len() % RECORD == 0 && !data.is_empty(), "{} bytes is not a whole number of records", data.len());
    let (mut compared, mut penetration, mut reports, mut rotated) = (0usize, 0usize, 0usize, 0usize);
    for (index, r) in data.chunks(RECORD).enumerate() {
        // Each shape with its own `getMargin` (a sphere's is its radius times its scaling).
        let (shape, margin) = match u32_at(r, 0x04) {
            8 => (CastShape::Sphere, f32_at(r, 0x20) * f32_at(r, 0x10)),
            10 => (CastShape::capsule(vec3_at(r, 0x20), vec3_at(r, 0x10), f32_at(r, 0x30), axis_at(r, 0x40, index)), f32_at(r, 0x30)),
            13 => (CastShape::cylinder(vec3_at(r, 0x20), axis_at(r, 0x40, index)), f32_at(r, 0x30)),
            kind => panic!("record {index}: a cast shape of type {kind}"),
        };
        let (other, margin_b) = match u32_at(r, B + 0x04) {
            1 => (WorldShape::Triangle(Triangle([vec3_at(r, B + 0x50), vec3_at(r, B + 0x60), vec3_at(r, B + 0x70)])), f32_at(r, B + 0x30)),
            0 => (WorldShape::Box { half_extents: vec3_at(r, B + 0x20) }, f32_at(r, B + 0x30)),
            8 => (WorldShape::Sphere, f32_at(r, B + 0x20) * f32_at(r, B + 0x10)),
            13 => (WorldShape::cylinder(vec3_at(r, B + 0x20), axis_at(r, B + 0x40, index)), f32_at(r, B + 0x30)),
            kind => panic!("record {index}: an other shape of type {kind}"),
        };
        let identity = transform_at(r, 0xE0).basis == [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(identity && transform_at(r, 0x120).basis == transform_at(r, 0xE0).basis, "record {index}: shape A is rotated");
        assert_eq!(r[0x160..0x1A0], r[0x1A0..0x1E0], "record {index}: shape B moves");
        let (from, to) = (vec3_at(r, 0x110), vec3_at(r, 0x150));
        let cast = Cast { shape, margin, from, by: [to[0] - from[0], to[1] - from[1], to[2] - from[2]] };
        let transform_b = transform_at(r, 0x160);
        let allowed = f32_at(r, 0x1E0 + 0xB8);
        let toi = time_of_impact(&cast, &other, margin_b, &transform_b, allowed);
        // The exe went to its penetration solver inside this call: the port must stop there too, and nowhere else.
        if r[0x2D2] != 0 {
            assert!(matches!(toi, Toi::NeedsPenetration { .. }), "record {index}: {toi:?}");
            penetration += 1;
            continue;
        }
        compared += 1;
        rotated += usize::from(transform_b.basis != [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let exit = EXITS[usize::from(r[0x2D3])];
        match toi {
            Toi::Hit { fraction, normal, point } => {
                assert_eq!((r[0x2D0], exit), (1, 0xB982C), "record {index}: the exe missed");
                assert_eq!(fraction.to_bits(), u32_at(r, 0x2C0), "record {index}: the fraction");
                assert_eq!(bits(normal), bits_at(r, 0x2A0), "record {index}: the normal");
                assert_eq!(bits(point), bits_at(r, 0x2B0), "record {index}: the hit point");
            }
            Toi::Miss(why) => assert_eq!((r[0x2D0], exit), (0, why.exit()), "record {index}: {why:?}"),
            Toi::NeedsPenetration { .. } => panic!("record {index}: the exe made no penetration call"),
        }
        // `processTriangle`'s report on top of the cast.
        if let (0xB8772, WorldShape::Triangle(triangle)) = (u32_at(r, 0x2D4), other) {
            let end = cast_triangle(&cast, &triangle, margin_b, &transform_b, allowed, f32_at(r, 0x3D0));
            match end {
                CastEnd::Hit { fraction, normal, point } => {
                    assert_ne!(u32_at(r, 0x3D4), 0, "record {index}: the exe reported nothing");
                    assert_eq!(fraction.to_bits(), u32_at(r, 0x3F0), "record {index}: the report's fraction");
                    assert_eq!(bits(normal), bits_at(r, 0x3D8), "record {index}: the report's normal");
                    assert_eq!(bits(point), bits_at(r, 0x3E4), "record {index}: the report's point");
                }
                CastEnd::Miss => assert_eq!(u32_at(r, 0x3D4), 0, "record {index}: the exe reported a hit"),
                CastEnd::NeedsPenetration { .. } => panic!("record {index}: the exe made no penetration call"),
            }
            reports += 1;
        }
    }
    eprintln!(
        "{compared} recorded casts give the exe's bits ({rotated} against a rotated shape, {reports} with processTriangle's report); \
         {penetration} go on to the penetration solver in both"
    );
}
