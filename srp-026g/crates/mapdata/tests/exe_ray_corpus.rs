//! The exe's ray port against the exe's own recorded `btCollisionWorld::rayTestSingle` calls. Runs only when
//! `SRP_EXE_RAY_CORPUS` names a corpus of raw records (several, separated by `;`), and skips with a note otherwise:
//! the records hold the game's own shapes, so the corpus stays out of the repository.
//!
//! ```text
//! SRP_EXE_RAY_CORPUS=<corpus.raw> cargo test -p mapdata --release --test exe_ray_corpus -- --nocapture
//! ```
//!
//! A corpus is the private rig's records (0x300 bytes each, no header): kind 1 a 0xA02D0 call (at any depth: a
//! compound's children are calls of their own), kind 2 a convex cast under one, kind 3 a triangle under one, taken in
//! Unicorn while the 0.26g exe ran traced steps of the game on the game's real collision objects, and from 0xA02D0
//! called there on those objects with made rays (`docs/notes/exactness_rig.md`). Each record is parsed into typed
//! inputs and run through the port; a record passes when every output field the executable wrote is the port's bits:
//!
//! * triangle (kind 3): `reportHit` called or not, its normal and fraction, the triangle callback's `m_hitFraction`
//!   after;
//! * box / sphere / cylinder / capsule (kind 2, by shape B): AL, the loop's rounds and the CastResult's +90h..+BFh;
//! * convex call (kind 1, type under 20): its casts (forward, reversed), the box (taken or not, its bits), the
//!   callback after;
//! * mesh ray (kind 1, type 21): the ray's two ends in the mesh's frame and the starting `m_hitFraction`;
//! * compound composition: the child's world transform from its parent's and its own;
//! * whole top call (depth 0): the callback's first 0x60 bytes after, from a fresh callback and the shape's tree with
//!   the triangles and the children the executable's own trees offered, in its order.

use std::collections::BTreeMap;

use mapdata::controller::exe_gjk::{Axis, Transform, Vec3};
use mapdata::controller::exe_ray::{
    self, ClosestRayCallback, CompoundChild, ConvexShape, RayFlags, RayHit, RayShape, SubsimplexEnd, TriangleRay,
    compose_child, mesh_ray, ray_convex, ray_test_single, ray_triangle, subsimplex_cast,
};

const RECORD: usize = 0x300;

/// A record's bytes, read little-endian.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u32(&self, at: usize) -> u32 {
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.0[at..at + 4]);
        u32::from_le_bytes(b)
    }

    fn f32(&self, at: usize) -> f32 {
        f32::from_bits(self.u32(at))
    }

    fn vec3(&self, at: usize) -> Vec3 {
        [self.f32(at), self.f32(at + 4), self.f32(at + 8)]
    }

    fn words(&self, at: usize, n: usize) -> Vec<u32> {
        (0..n).map(|k| self.u32(at + 4 * k)).collect()
    }

    fn transform(&self, at: usize) -> Transform {
        Transform { basis: [self.vec3(at), self.vec3(at + 0x10), self.vec3(at + 0x20)], origin: self.vec3(at + 0x30) }
    }
}

fn vec_words(v: Vec3) -> Vec<u32> {
    vec![v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), 0]
}

fn transform_words(t: &Transform) -> Vec<u32> {
    let mut w = Vec::new();
    for row in &t.basis {
        w.extend(vec_words(*row));
    }
    w.extend(vec_words(t.origin));
    w
}

/// A shape's bytes (its first 0x80) parsed as a convex shape, by its vtable and type.
fn parse_convex(r: &Bytes, at: usize) -> Result<ConvexShape, String> {
    let vtable = r.u32(at);
    let kind = r.u32(at + 4);
    let axis = |off: usize| Axis::from_index(r.u32(at + off) as i32).ok_or_else(|| format!("up axis {}", r.u32(at + off)));
    match (vtable, kind) {
        (0x9E_4AFC, 0) => Ok(ConvexShape::Box { implicit: r.vec3(at + 0x20), margin: r.f32(at + 0x30) }),
        (0x9E_2FCC, 8) => Ok(ConvexShape::sphere(r.f32(at + 0x20), r.f32(at + 0x10))),
        (0x9E_4B9C, 13) => Ok(ConvexShape::CylinderY { implicit: r.vec3(at + 0x20), margin: r.f32(at + 0x30) }),
        (0x9E_48A4, 10) => Ok(ConvexShape::Capsule {
            implicit: r.vec3(at + 0x20),
            scaling: r.vec3(at + 0x10),
            margin: r.f32(at + 0x30),
            up: axis(0x40)?,
        }),
        _ => Err(format!("convex shape vtable {vtable:#x} type {kind} not read")),
    }
}

fn shape_name(kind: u32) -> &'static str {
    match kind {
        0 => "box",
        8 => "sphere",
        10 => "capsule",
        13 => "cylinder",
        21 => "mesh",
        31 => "compound",
        _ => "other",
    }
}

/// One record kept with where it came from.
struct Rec<'a> {
    file: usize,
    index: usize,
    b: Bytes<'a>,
}

impl Rec<'_> {
    fn label(&self) -> String {
        format!("file {} record {} (call {}, {}, case {})", self.file, self.index, self.b.u32(4),
                if self.b.u32(0x14) == 1 { "synthetic" } else { "real" }, self.b.u32(0x10))
    }
    fn source(&self) -> &'static str {
        if self.b.u32(0x14) == 1 {
            "synthetic"
        } else {
            "real"
        }
    }
}

/// A call with its casts, triangles and children (by id, in the order they entered).
struct Call<'a> {
    rec: Rec<'a>,
    casts: Vec<usize>,
    tris: Vec<usize>,
    children: Vec<u32>,
}

#[derive(Default)]
struct Tally {
    /// (branch, source) -> (passed, of)
    counts: BTreeMap<(String, &'static str), (usize, usize)>,
    failures: BTreeMap<String, Vec<String>>,
    parse_errors: Vec<String>,
}

impl Tally {
    fn add(&mut self, branch: &str, source: &'static str, ok: bool, what: impl FnOnce() -> String, show: usize) {
        let e = self.counts.entry((branch.to_string(), source)).or_default();
        e.1 += 1;
        if ok {
            e.0 += 1;
        } else {
            let list = self.failures.entry(branch.to_string()).or_default();
            if list.len() < show {
                list.push(what());
            }
        }
    }
}

fn hex(w: &[u32]) -> String {
    w.iter().map(|x| format!("{x:08x}")).collect::<Vec<_>>().join(" ")
}

/// The CastResult's +90h..+BFh (12 words) after the port's cast, from the words at entry.
fn cast_block_after(entry: &[u32], end: &SubsimplexEnd) -> Vec<u32> {
    let mut w = entry.to_vec();
    match *end {
        SubsimplexEnd::Refused(_) => {}
        SubsimplexEnd::Away { fraction, normal } => {
            w[0..4].copy_from_slice(&vec_words(normal));
            w[8] = fraction.to_bits();
        }
        SubsimplexEnd::Hit { fraction, normal, point } => {
            w[0..4].copy_from_slice(&vec_words(normal));
            w[4..8].copy_from_slice(&vec_words(point));
            w[8] = fraction.to_bits();
        }
    }
    w
}

fn al_of(end: &SubsimplexEnd) -> u8 {
    matches!(end, SubsimplexEnd::Hit { .. }) as u8
}

/// A ray callback from its first 0x60 bytes at `at` (ClosestRayResultCallback).
fn callback_of(r: &Bytes, at: usize) -> ClosestRayCallback {
    let object = r.u32(at + 8);
    let hit = if object != 0 {
        Some(RayHit { object, normal: r.vec3(at + 0x40), point: r.vec3(at + 0x50) })
    } else {
        None
    };
    ClosestRayCallback { from: r.vec3(at + 0x20), to: r.vec3(at + 0x30), closest: r.f32(at + 4), hit }
}

/// The callback's first 0x60 bytes (24 words) after the port, from the words at entry.
fn callback_words_after(entry: &[u32], cb: &ClosestRayCallback) -> Vec<u32> {
    let mut w = entry.to_vec();
    w[1] = cb.closest.to_bits();
    if let Some(hit) = cb.hit {
        w[2] = hit.object;
        w[16..20].copy_from_slice(&vec_words(hit.normal));
        w[20] = hit.point[0].to_bits();
        w[21] = hit.point[1].to_bits();
        w[22] = hit.point[2].to_bits();
    }
    w
}

/// The shape tree of a call, from its records: convex by its bytes, a mesh by its triangles, a compound by its
/// children's calls.
fn shape_tree(calls: &BTreeMap<u32, Call>, recs: &[Rec], id: u32) -> Result<RayShape, String> {
    let c = &calls[&id];
    let r = &c.rec.b;
    let kind = r.u32(0xA4);
    if kind < 20 {
        return Ok(RayShape::Convex(parse_convex(r, 0xA0)?));
    }
    match kind {
        21 => Ok(RayShape::Mesh(
            c.tris
                .iter()
                .map(|&t| {
                    let b = &recs[t].b;
                    [b.vec3(0x20), b.vec3(0x30), b.vec3(0x40)]
                })
                .collect(),
        )),
        31 => {
            let mut children = Vec::new();
            for child in &c.children {
                let cr = &calls[child].rec.b;
                if cr.u32(0x260) & 4 == 0 {
                    return Err("a compound's child without its data".into());
                }
                children.push(CompoundChild { transform: cr.transform(0x270), shape: shape_tree(calls, recs, *child)? });
            }
            Ok(RayShape::Compound(children))
        }
        _ => Err(format!("shape type {kind} not ported")),
    }
}

#[test]
fn the_recorded_ray_tests_give_the_exes_bits() {
    let Some(paths) = std::env::var_os("SRP_EXE_RAY_CORPUS") else {
        eprintln!("SRP_EXE_RAY_CORPUS is not set: the corpus test of the exe's ray port is skipped");
        return;
    };
    let paths = paths.into_string().expect("a path list in UTF-8");
    let mut data = Vec::new();
    for p in paths.split(';').filter(|p| !p.is_empty()) {
        let d = std::fs::read(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert!(d.len() % RECORD == 0, "{p}: {} bytes, not a whole number of records", d.len());
        data.push(d);
    }
    let mut tally = Tally::default();
    let show = 4;
    for (file, d) in data.iter().enumerate() {
        let recs: Vec<Rec> =
            d.chunks(RECORD).enumerate().map(|(index, c)| Rec { file, index, b: Bytes(c) }).collect();
        let mut calls: BTreeMap<u32, Call> = BTreeMap::new();
        for (i, r) in recs.iter().enumerate() {
            if r.b.u32(0) == 1 {
                calls.insert(r.b.u32(4), Call { rec: Rec { file, index: i, b: Bytes(r.b.0) }, casts: vec![], tris: vec![], children: vec![] });
            }
        }
        for (i, r) in recs.iter().enumerate() {
            let id = r.b.u32(4);
            match r.b.u32(0) {
                1 => {
                    let parent = r.b.u32(8);
                    if parent != 0 {
                        match calls.get_mut(&parent) {
                            Some(p) => p.children.push(id),
                            None => tally.parse_errors.push(format!("{}: no parent {parent}", r.label())),
                        }
                    }
                }
                2 | 3 => match calls.get_mut(&id) {
                    Some(c) => {
                        if r.b.u32(0) == 2 {
                            c.casts.push(i)
                        } else {
                            c.tris.push(i)
                        }
                    }
                    None => tally.parse_errors.push(format!("{}: no call {id}", r.label())),
                },
                k => tally.parse_errors.push(format!("{}: kind {k}", r.label())),
            }
        }
        for c in calls.values_mut() {
            c.children.sort_unstable();
        }

        // Triangles.
        for r in recs.iter().filter(|r| r.b.u32(0) == 3) {
            let b = &r.b;
            let tri = [b.vec3(0x20), b.vec3(0x30), b.vec3(0x40)];
            let entry = b.f32(0x80);
            let got = ray_triangle(b.vec3(0x60), b.vec3(0x70), RayFlags(b.u32(0x5C)), entry, &tri);
            let reported = b.u32(0x88) == 1;
            let (ok, after) = match got {
                TriangleRay::Hit { normal, distance } => (
                    reported && b.u32(0x8C) == distance.to_bits() && b.words(0x90, 4) == vec_words(normal),
                    distance,
                ),
                _ => (!reported, entry),
            };
            let ok = ok && b.u32(0x84) == after.to_bits();
            tally.add("triangle", r.source(), ok, || {
                format!("{}: port {got:?}; exe reported {reported} fraction {:08x} normal {} after {:08x}", r.label(),
                        b.u32(0x8C), hex(&b.words(0x90, 4)), b.u32(0x84))
            }, show);
        }

        // Casts.
        for r in recs.iter().filter(|r| r.b.u32(0) == 2) {
            let b = &r.b;
            let a = match parse_convex(b, 0x20) {
                Ok(a) => a,
                Err(e) => {
                    tally.parse_errors.push(format!("{}: shape A: {e}", r.label()));
                    continue;
                }
            };
            if a != ConvexShape::POINT {
                tally.parse_errors.push(format!("{}: shape A is not the point: {a:?}", r.label()));
            }
            let shape = match parse_convex(b, 0x60) {
                Ok(s) => s,
                Err(e) => {
                    tally.parse_errors.push(format!("{}: shape B: {e}", r.label()));
                    continue;
                }
            };
            let entry = b.words(0x1E0, 12);
            let allowed = b.f32(0x1E0 + 0x28);
            let got = subsimplex_cast(&a, &shape, &b.transform(0xE0), &b.transform(0x120), &b.transform(0x160),
                                      &b.transform(0x1A0), allowed);
            let want = cast_block_after(&entry, &got.end);
            let exe = b.words(0x210, 12);
            let al = b.0[0x240];
            let exit = b.0[0x241];
            let rounds = u16::from_le_bytes([b.0[0x242], b.0[0x243]]) as u32;
            let ok = al == al_of(&got.end) && exit == al_of(&got.end) && rounds == got.rounds && exe == want;
            let branch = format!("cast vs {}", shape_name(b.u32(0x64)));
            tally.add(&branch, r.source(), ok, || {
                format!("{}: port {:?} rounds {}; exe AL {al} rounds {rounds}\n      port {}\n      exe  {}",
                        r.label(), got.end, got.rounds, hex(&want), hex(&exe))
            }, show);
        }

        // Calls.
        for c in calls.values() {
            let b = &c.rec.b;
            let r = &c.rec;
            let kind = b.u32(0xA4);
            let depth = b.u32(0xC);
            let from = b.transform(0x20);
            let to = b.transform(0x60);
            let world = b.transform(0x120);
            let cb_in = b.words(0x160, 24);
            let cb_out = b.words(0x1C0, 24);

            // A compound's child's transform.
            if b.u32(0x260) & 4 != 0 {
                let parent = &calls[&b.u32(8)].rec.b;
                let got = compose_child(&parent.transform(0x120), &b.transform(0x270));
                let ok = transform_words(&got) == b.words(0x120, 16) && b.u32(0x224) == b.u32(0x268);
                tally.add("compound composition", r.source(), ok, || {
                    format!("{}: port {}\n      exe  {}", r.label(), hex(&transform_words(&got)), hex(&b.words(0x120, 16)))
                }, show);
            }

            if kind < 20 {
                let shape = match parse_convex(b, 0xA0) {
                    Ok(s) => s,
                    Err(e) => {
                        tally.parse_errors.push(format!("{}: {e}", r.label()));
                        continue;
                    }
                };
                let trace = ray_convex(&from, &to, &shape, &world, b.f32(0x164));
                let mut notes = Vec::new();
                let forward: Vec<&Rec> = c.casts.iter().map(|&i| &recs[i]).filter(|x| x.b.u32(0x18) == 0).collect();
                let reversed: Vec<&Rec> = c.casts.iter().map(|&i| &recs[i]).filter(|x| x.b.u32(0x18) == 1).collect();
                if forward.len() != 1 {
                    notes.push(format!("{} forward casts", forward.len()));
                } else if forward[0].b.0[0x240] != al_of(&trace.forward.end) {
                    notes.push("forward AL".into());
                }
                if reversed.len() != trace.reversed.is_some() as usize {
                    notes.push(format!("{} reversed casts, port {}", reversed.len(), trace.reversed.is_some()));
                } else if let (Some(rv), Some(x)) = (trace.reversed, reversed.first())
                    && x.b.0[0x240] != al_of(&rv.end)
                {
                    notes.push("reversed AL".into());
                }
                let took_box = b.u32(0x260) & 1 != 0;
                match trace.aabb {
                    Some((lo, hi)) => {
                        if !took_box {
                            notes.push("port took the box, exe not".into());
                        } else if b.words(0x240, 4) != vec_words(lo) || b.words(0x250, 4) != vec_words(hi) {
                            notes.push(format!("box: port {} {} exe {} {}", hex(&vec_words(lo)), hex(&vec_words(hi)),
                                               hex(&b.words(0x240, 4)), hex(&b.words(0x250, 4))));
                        }
                    }
                    None => {
                        if took_box {
                            notes.push("exe took the box, port not".into());
                        }
                    }
                }
                // The callback after: a top call's (the closest callback), a child's adder's fraction only.
                let mut cb = callback_of(b, 0x160);
                if let exe_ray::ConvexRay::Hit { fraction, normal } = trace.answer {
                    cb.add_single_result(b.u32(0x220), normal, fraction, exe_ray::NormalSpace::World);
                }
                if depth == 0 {
                    let want = callback_words_after(&cb_in, &cb);
                    if want != cb_out {
                        notes.push(format!("callback: port {}\n      exe  {}", hex(&want), hex(&cb_out)));
                    }
                } else if cb_out[1] != cb.closest.to_bits() {
                    notes.push(format!("adder's fraction: port {:08x} exe {:08x}", cb.closest.to_bits(), cb_out[1]));
                }
                let branch = format!("convex call: {}", shape_name(kind));
                tally.add(&branch, r.source(), notes.is_empty(), || {
                    format!("{}: port {:?}: {}", r.label(), trace.answer, notes.join("; "))
                }, show);
            } else if kind == 21 {
                if b.u32(0x260) & 2 == 0 {
                    tally.parse_errors.push(format!("{}: a mesh call without its bridge", r.label()));
                    continue;
                }
                let (lf, lt) = mesh_ray(from.origin, to.origin, &world);
                let mut ok = b.words(0x2B0 + 0x10, 4) == vec_words(lf) && b.words(0x2B0 + 0x20, 4) == vec_words(lt);
                ok = ok && b.u32(0x2B0 + 0x34) == b.u32(0x164);
                // Each triangle's starting m_hitFraction is the one before's after (the first: the callback's).
                let mut running = b.u32(0x164);
                for &t in &c.tris {
                    ok = ok && recs[t].b.u32(0x80) == running;
                    running = recs[t].b.u32(0x84);
                }
                tally.add("mesh ray in its frame", r.source(), ok, || {
                    format!("{}: port {} {}\n      exe  {} {}", r.label(), hex(&vec_words(lf)), hex(&vec_words(lt)),
                            hex(&b.words(0x2C0, 4)), hex(&b.words(0x2D0, 4)))
                }, show);
            }

            // The whole top call.
            if depth == 0 {
                let shape = match shape_tree(&calls, &recs, b.u32(4)) {
                    Ok(s) => s,
                    Err(e) => {
                        tally.parse_errors.push(format!("{}: {e}", r.label()));
                        continue;
                    }
                };
                let mut cb = callback_of(b, 0x160);
                ray_test_single(&from, &to, b.u32(0x220), &shape, &world, RayFlags(b.u32(0x170)), &mut cb);
                let want = callback_words_after(&cb_in, &cb);
                let branch = format!("whole top call: {}", shape_name(kind));
                tally.add(&branch, r.source(), want == cb_out, || {
                    format!("{}: port {}\n      exe  {}", r.label(), hex(&want), hex(&cb_out))
                }, show);
            }
        }
    }

    println!("records equal to the executable's bits, by branch (passed / of):");
    let mut all_ok = tally.parse_errors.is_empty();
    for ((branch, source), (ok, of)) in &tally.counts {
        println!("  {branch:<34} {source:<10} {ok:>7} / {of:<7}{}", if ok == of { "" } else { "  <-- differs" });
        all_ok &= ok == of;
    }
    for (branch, list) in &tally.failures {
        println!("first differing records of {branch}:");
        for l in list {
            println!("  {l}");
        }
    }
    if !tally.parse_errors.is_empty() {
        println!("records not parsed ({}):", tally.parse_errors.len());
        for e in tally.parse_errors.iter().take(10) {
            println!("  {e}");
        }
    }
    assert!(all_ok, "the port differs from the executable on recorded ray tests");
}
