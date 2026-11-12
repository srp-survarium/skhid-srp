//! The exe's GJK port against the exe's own recorded calls. Runs only when `SRP_EXE_GJK_CORPUS` names a corpus of
//! raw records (and skips with a note otherwise): the records hold the game's own triangles, so the corpus stays out
//! of the repository.
//!
//! ```text
//! SRP_EXE_GJK_CORPUS=<corpus.raw> cargo test -p mapdata --release --test exe_gjk_corpus -- --nocapture
//! ```
//!
//! A record is 0x200 bytes, little-endian, one call of `btGjkPairDetector::getClosestPointsNonVirtual` 0xD6CD0 taken
//! in Unicorn while the 0.26g exe ran traced steps of the game (`docs/notes/exactness_rig.md`): the cast shape's
//! bytes (+0: type +4, scaling +10h, implicit dims +20h, margin +30h, up axis +40h), the other shape's (+60h: a
//! triangle's corners at its +50h, +60h, +70h; a box's, a sphere's or a cylinder's fields as the cast shape's), the input (+E0h: two transforms, the largest distance squared at +80h), the
//! point collector after the call (+170h: normal +10h, point +20h, distance +30h, has-result +34h), the detector's
//! margins (+1B0h, +1B4h), `m_ignoreMargin` (+1B8h), the count of penetration-solver calls (+1B9h),
//! `m_catchDegeneracies` (+1BCh), and after the call `m_lastUsedMethod`, `m_curIter`, `m_degenerateSimplex`
//! (+1C0h), `m_cachedSeparatingAxis` (+1D8h), `m_cachedSeparatingDistance` (+1E8h), and whether it has a penetration
//! solver (+1ECh).
use mapdata::controller::exe_gjk::{
    Axis, CastShape, ClosestPointInput, Detector, Margins, Outcome, Penetration, Transform, Triangle, WorldShape, get_closest_points,
};

const RECORD: usize = 0x200;

fn u32_at(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

fn f32_at(r: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(r, at))
}

fn vec3_at(r: &[u8], at: usize) -> [f32; 3] {
    [f32_at(r, at), f32_at(r, at + 4), f32_at(r, at + 8)]
}

fn transform_at(r: &[u8], at: usize) -> Transform {
    Transform { basis: [vec3_at(r, at), vec3_at(r, at + 0x10), vec3_at(r, at + 0x20)], origin: vec3_at(r, at + 0x30) }
}

fn bits(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

#[test]
fn every_recorded_call_gives_the_exes_bits() {
    let Some(path) = std::env::var_os("SRP_EXE_GJK_CORPUS") else {
        eprintln!("SRP_EXE_GJK_CORPUS is not set: the corpus check is skipped");
        return;
    };
    let data = std::fs::read(&path).expect("the corpus file");
    assert!(data.len() % RECORD == 0 && !data.is_empty(), "{} bytes is not a whole number of records", data.len());
    let (mut compared, mut penetration) = (0usize, 0usize);
    for (index, r) in data.chunks(RECORD).enumerate() {
        let up = Axis::from_index(u32_at(r, 0x40) as i32);
        let shape = match (u32_at(r, 0x04), up) {
            (8, _) => CastShape::Sphere,
            (10, Some(up)) => CastShape::capsule(vec3_at(r, 0x20), vec3_at(r, 0x10), f32_at(r, 0x30), up),
            (13, Some(up)) => CastShape::cylinder(vec3_at(r, 0x20), up),
            (kind, _) => panic!("record {index}: a cast shape of type {kind}"),
        };
        let other = match (u32_at(r, 0x64), Axis::from_index(u32_at(r, 0xA0) as i32)) {
            (1, _) => WorldShape::Triangle(Triangle([vec3_at(r, 0xB0), vec3_at(r, 0xC0), vec3_at(r, 0xD0)])),
            (0, _) => WorldShape::Box { half_extents: vec3_at(r, 0x80) },
            (8, _) => WorldShape::Sphere,
            (13, Some(up)) => WorldShape::cylinder(vec3_at(r, 0x80), up),
            (kind, _) => panic!("record {index}: an other shape of type {kind}"),
        };
        let detector = Detector {
            margins: if r[0x1B8] != 0 { Margins::Ignored } else { Margins::Applied { a: f32_at(r, 0x1B0), b: f32_at(r, 0x1B4) } },
            penetration: if u32_at(r, 0x1EC) != 0 {
                Penetration::Solver { catch_degeneracies: u32_at(r, 0x1BC) != 0 }
            } else {
                Penetration::NoSolver
            },
        };
        let input = ClosestPointInput {
            transform_a: transform_at(r, 0xE0),
            transform_b: transform_at(r, 0x120),
            maximum_distance_squared: f32_at(r, 0x160),
        };
        let answer = get_closest_points(&detector, &shape, &other, &input);
        // The exe went on to its penetration solver: the port must stop there too, and nowhere else.
        if r[0x1B9] != 0 {
            assert!(matches!(answer.outcome, Outcome::NeedsPenetration(_)), "record {index}: {:?}", answer.outcome);
            penetration += 1;
            continue;
        }
        compared += 1;
        match answer.outcome {
            Outcome::Contact { normal_on_b, point_on_b, distance } => {
                assert_eq!(r[0x1A4], 1, "record {index}: the exe reported no contact");
                assert_eq!(bits(normal_on_b), bits(vec3_at(r, 0x180)), "record {index}: the normal");
                assert_eq!(bits(point_on_b), bits(vec3_at(r, 0x190)), "record {index}: the point");
                assert_eq!(distance.to_bits(), u32_at(r, 0x1A0), "record {index}: the distance");
            }
            Outcome::NoContact => assert_eq!(r[0x1A4], 0, "record {index}: the exe reported a contact"),
            Outcome::NeedsPenetration(_) => panic!("record {index}: the exe made no penetration call"),
        }
        let state = answer.state;
        assert_eq!(state.last_used_method.code(), u32_at(r, 0x1C0) as i32, "record {index}: m_lastUsedMethod");
        assert_eq!(state.cur_iter, u32_at(r, 0x1C4), "record {index}: m_curIter");
        assert_eq!(state.degenerate_simplex.code(), u32_at(r, 0x1C8) as i32, "record {index}: m_degenerateSimplex");
        assert_eq!(bits(state.cached_separating_axis), bits(vec3_at(r, 0x1D8)), "record {index}: m_cachedSeparatingAxis");
        assert_eq!(state.cached_separating_distance.to_bits(), u32_at(r, 0x1E8), "record {index}: m_cachedSeparatingDistance");
    }
    eprintln!("{compared} recorded calls give the exe's bits; {penetration} go on to the penetration solver in both");
}
