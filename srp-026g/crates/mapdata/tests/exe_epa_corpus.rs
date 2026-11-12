//! The port of the exe's penetration-depth solver against the exe's own recorded calls. Runs only when
//! `SRP_EXE_EPA_CORPUS` names corpus files, `;` between them (and skips with a note otherwise): the records hold the
//! game's own shapes, so the corpus stays out of the repository.
//!
//! ```text
//! SRP_EXE_EPA_CORPUS=<a.epa;b.epa> cargo test -p mapdata --release --test exe_epa_corpus -- --nocapture
//! ```
//!
//! A file is the 8 bytes `SRPEPA1\n` and records of 0x300 bytes, little-endian, one call of
//! `btGjkEpaPenetrationDepthSolver::calcPenDepth` 0xB7EE0 taken in Unicorn from the 0.26g exe
//! (`docs/notes/exactness_rig.md`): shape A's bytes (+0: type +4, scaling +10h, implicit dims +20h, margin +30h, up
//! axis +40h), shape B's (+60h: the same; a triangle's corners at its +50h, +60h, +70h), the detector's input
//! (transform A +E0h, B +120h, the largest distance squared +160h), its margins (+168h, +16Ch), `m_ignoreMargin`
//! (+170h) and `m_catchDegeneracies` (+171h), where the record comes from (+174h: 6 is `calcPenDepth` called alone),
//! the transforms the exe handed the solver (+180h, +1C0h), the solver's answer (AL +200h, `v` +210h, the witnesses
//! +220h and +230h, the statuses of its GJK +240h, EPA +244h (-1: not run) and distance GJK +248h (-1), the EPA's
//! rounds +24Ch and the GJK's +2B0h), and the detector after the call (`m_cachedSeparatingAxis` +250h,
//! `m_cachedSeparatingDistance` +260h, `m_lastUsedMethod` +264h, `m_curIter` +268h, `m_degenerateSimplex` +26Ch, the
//! point collector +270h: normal +10h, point +20h, distance +30h, `m_hasResult` +34h).
use mapdata::controller::exe_epa::{self, B, Convex};
use mapdata::controller::exe_gjk::{
    Axis, CastShape, ClosestPointInput, Detector, Margins, Outcome, Penetration, Transform, Triangle, Vec3, WorldShape, get_closest_points,
};

const MAGIC: &[u8] = b"SRPEPA1\n";
const RECORD: usize = 0x300;
/// 0xA2D990: `BT_LARGE_FLOAT`, the empty point collector's distance.
const BT_LARGE_FLOAT: f32 = f32::from_bits(0x5D5E_0B6B);

fn u32_at(r: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]])
}

fn f32_at(r: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(r, at))
}

fn vec3_at(r: &[u8], at: usize) -> Vec3 {
    [f32_at(r, at), f32_at(r, at + 4), f32_at(r, at + 8)]
}

fn transform_at(r: &[u8], at: usize) -> Transform {
    Transform { basis: [vec3_at(r, at), vec3_at(r, at + 0x10), vec3_at(r, at + 0x20)], origin: vec3_at(r, at + 0x30) }
}

fn bits(v: Vec3) -> [u32; 3] {
    v.map(f32::to_bits)
}

/// A shape's own `getMargin`: +30h, or a sphere's radius (+20h) times its scaling (+10h).
fn own_margin(s: &[u8]) -> f32 {
    if u32_at(s, 4) == 8 { f32_at(s, 0x20) * f32_at(s, 0x10) } else { f32_at(s, 0x30) }
}

fn cast_shape(s: &[u8], index: usize) -> CastShape {
    match (u32_at(s, 4), Axis::from_index(u32_at(s, 0x40) as i32)) {
        (8, _) => CastShape::Sphere,
        (10, Some(up)) => CastShape::capsule(vec3_at(s, 0x20), vec3_at(s, 0x10), f32_at(s, 0x30), up),
        (13, Some(up)) => CastShape::cylinder(vec3_at(s, 0x20), up),
        (kind, _) => panic!("record {index}: a cast shape of type {kind}"),
    }
}

fn world_shape(s: &[u8], index: usize) -> WorldShape {
    match (u32_at(s, 4), Axis::from_index(u32_at(s, 0x40) as i32)) {
        (1, _) => WorldShape::Triangle(Triangle([vec3_at(s, 0x50), vec3_at(s, 0x60), vec3_at(s, 0x70)])),
        (0, _) => WorldShape::Box { half_extents: vec3_at(s, 0x20) },
        (8, _) => WorldShape::Sphere,
        (10, Some(up)) => WorldShape::capsule(vec3_at(s, 0x20), vec3_at(s, 0x10), f32_at(s, 0x30), up),
        (13, Some(up)) => WorldShape::cylinder(vec3_at(s, 0x20), up),
        (kind, _) => panic!("record {index}: an other shape of type {kind}"),
    }
}

#[test]
fn the_solver_gives_the_exes_bits_on_every_recorded_call() {
    let Ok(paths) = std::env::var("SRP_EXE_EPA_CORPUS") else {
        eprintln!("SRP_EXE_EPA_CORPUS is not set: the recorded calls of the exe's penetration solver are not compared");
        return;
    };
    let (mut solver_calls, mut detector_calls) = (0usize, 0usize);
    for path in paths.split(';').filter(|p| !p.is_empty()) {
        let data = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        let body = data.strip_prefix(MAGIC).unwrap_or_else(|| panic!("{path}: not a corpus of the solver's calls"));
        assert_eq!(body.len() % RECORD, 0, "{path}: {} bytes", body.len());
        for (index, r) in body.chunks(RECORD).enumerate() {
            let (sa, sb) = (&r[0..0x60], &r[0x60..0xE0]);
            let (shape_a, shape_b) = (cast_shape(sa, index), world_shape(sb, index));
            let (a, b) = (Convex { shape: &shape_a, margin: own_margin(sa) }, B(&shape_b));
            let b = Convex { shape: &b, margin: own_margin(sb) };
            // `calcPenDepth` on the transforms the exe handed it.
            let (ta, tb) = (transform_at(r, 0x180), transform_at(r, 0x1C0));
            let pen = exe_epa::calc_pen_depth(Convex { shape: a.shape, margin: a.margin }, &ta, Convex { shape: b.shape, margin: b.margin }, &tb);
            assert_eq!(pen.results.penetration_gjk as i32, u32_at(r, 0x240) as i32, "{path} record {index}: the GJK's status");
            assert_eq!(pen.results.epa.map_or(-1, |s| s as i32), u32_at(r, 0x244) as i32, "{path} record {index}: the EPA's status");
            assert_eq!(pen.results.distance_gjk.map_or(-1, |s| s as i32), u32_at(r, 0x248) as i32, "{path} record {index}: the distance GJK's status");
            assert_eq!(pen.results.epa_rounds, u32_at(r, 0x24C), "{path} record {index}: the EPA's rounds");
            assert_eq!(pen.results.gjk_rounds, u32_at(r, 0x2B0), "{path} record {index}: the GJK's rounds");
            assert_eq!(u32::from(pen.valid), u32_at(r, 0x200) & 0xFF, "{path} record {index}: AL");
            match pen.written {
                Some((v, witness_a, witness_b)) => {
                    assert_eq!(bits(v), bits(vec3_at(r, 0x210)), "{path} record {index}: v");
                    assert_eq!(bits(witness_a), bits(vec3_at(r, 0x220)), "{path} record {index}: the witness on A");
                    assert_eq!(bits(witness_b), bits(vec3_at(r, 0x230)), "{path} record {index}: the witness on B");
                }
                None => assert_eq!(vec3_at(r, 0x210), [0.0; 3], "{path} record {index}: the exe wrote v"),
            }
            solver_calls += 1;
            // `calcPenDepth` called alone: no detector around it.
            if u32_at(r, 0x174) == 6 {
                continue;
            }
            // The whole detector call: up to its penetration branch, then the branch with the solver.
            let input = ClosestPointInput {
                transform_a: transform_at(r, 0xE0),
                transform_b: transform_at(r, 0x120),
                maximum_distance_squared: f32_at(r, 0x160),
            };
            let margins = if r[0x170] != 0 { Margins::Ignored } else { Margins::Applied { a: f32_at(r, 0x168), b: f32_at(r, 0x16C) } };
            let detector = Detector { margins, penetration: Penetration::Solver { catch_degeneracies: r[0x171] != 0 } };
            let answer = get_closest_points(&detector, &shape_a, &shape_b, &input);
            let Outcome::NeedsPenetration(start) = answer.outcome else {
                panic!("{path} record {index}: the port's detector does not go to the solver: {:?}", answer.outcome);
            };
            assert_eq!(bits(start.local_a.origin), bits(ta.origin), "{path} record {index}: the solver's transform A");
            assert_eq!(bits(start.local_b.origin), bits(tb.origin), "{path} record {index}: the solver's transform B");
            let margin_b = match margins {
                Margins::Applied { b, .. } => b,
                Margins::Ignored => 0.0,
            };
            let tail = exe_epa::detector_tail(
                a,
                &start.local_a,
                b,
                &start.local_b,
                start.gjk,
                start.margin_sum,
                margin_b,
                start.position_offset,
                input.maximum_distance_squared,
            );
            let collector = &r[0x270..0x2B0];
            // The collector keeps a point closer than its 1e18 (0xB88BB).
            match tail.contact.filter(|c| BT_LARGE_FLOAT > c.2) {
                Some((normal, point, distance)) => {
                    assert_ne!(collector[0x34], 0, "{path} record {index}: the exe reported no contact");
                    assert_eq!(bits(normal), bits(vec3_at(collector, 0x10)), "{path} record {index}: the normal");
                    assert_eq!(bits(point), bits(vec3_at(collector, 0x20)), "{path} record {index}: the point");
                    assert_eq!(distance.to_bits(), u32_at(collector, 0x30), "{path} record {index}: the distance");
                }
                None => assert_eq!(collector[0x34], 0, "{path} record {index}: the exe reported a contact"),
            }
            let state = answer.state;
            assert_eq!(tail.method.map_or(state.last_used_method.code(), |m| m as i32), u32_at(r, 0x264) as i32, "{path} record {index}: m_lastUsedMethod");
            assert_eq!(state.cur_iter, u32_at(r, 0x268), "{path} record {index}: m_curIter");
            assert_eq!(state.degenerate_simplex.code(), u32_at(r, 0x26C) as i32, "{path} record {index}: m_degenerateSimplex");
            assert_eq!(bits(tail.cached_axis), bits(vec3_at(r, 0x250)), "{path} record {index}: m_cachedSeparatingAxis");
            assert_eq!(
                tail.cached_distance.unwrap_or(state.cached_separating_distance).to_bits(),
                u32_at(r, 0x260),
                "{path} record {index}: m_cachedSeparatingDistance"
            );
            detector_calls += 1;
        }
    }
    eprintln!("{solver_calls} recorded calls of calcPenDepth give the exe's bits; {detector_calls} of them with the detector's whole call");
}
