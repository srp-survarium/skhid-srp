use super::*;
use crate::grid::tests::two_rooms;

/// A folder of its own under the system's temporary folder.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("srp-memo-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn modes_parse() {
    assert_eq!(Mode::parse("off"), Ok(Mode::Off));
    assert_eq!(Mode::parse(""), Ok(Mode::Off));
    assert_eq!(Mode::parse("use"), Ok(Mode::Use));
    assert_eq!(Mode::parse("write-only"), Ok(Mode::WriteOnly));
    assert_eq!(Mode::parse("verify=0.02"), Ok(Mode::Verify { per_million: 20_000 }));
    assert_eq!(Mode::parse("verify=1"), Ok(Mode::Verify { per_million: 1_000_000 }));
    assert!(Mode::parse("verify=2").is_err());
    assert!(Mode::parse("on").is_err());
}

#[test]
fn a_segment_reads_back_and_a_damaged_one_is_left_out_whole() {
    let dir = scratch("segments");
    std::fs::create_dir_all(&dir).unwrap();
    let records = vec![(1u128, vec![1, 2, 3]), (u128::from_le_bytes(*b"END\0END\0END\0END\0"), vec![]), (7, vec![9; 40])];
    let path = dir.join("s.seg");
    let bytes = segment_bytes("s", 5, &records);
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read_segment(&path, "s", 5).unwrap(), records);
    assert!(read_segment(&path, "t", 5).is_err(), "another site's");
    assert!(read_segment(&path, "s", 6).is_err(), "another result type's");
    for at in (0..bytes.len()).step_by(7).chain([bytes.len() - 1]) {
        let mut damaged = bytes.clone();
        damaged[at] ^= 0x41;
        std::fs::write(&path, &damaged).unwrap();
        assert!(read_segment(&path, "s", 5).is_err(), "byte {at}");
    }
    for cut in [bytes.len() / 2, bytes.len() - 1] {
        std::fs::write(&path, &bytes[..cut]).unwrap();
        assert!(read_segment(&path, "s", 5).is_err(), "cut at {cut}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stable_ids_name_each_floor_both_ways() {
    let grid = two_rooms();
    let ids = StableIds::of(&grid).expect("one floor a height in a cell");
    for a in (0..grid.len() as NodeId).step_by(13) {
        assert_eq!(ids.node(ids.id(a)), Some(a));
    }
    assert_eq!(ids.node(StableId { cell: u32::MAX, height: 0 }), None);
}

#[test]
fn a_window_is_the_sum_of_its_cells_and_sees_only_them() {
    let (nx, nz) = (9, 7);
    let state = |changed: Option<usize>| move |cell: usize| if Some(cell) == changed { 99 } else { (cell as u128 * 31) % 5 };
    let base = Windows::new(nx, nz, state(None));
    let moved = Windows::new(nx, nz, state(Some(3 * nx + 4)));
    // A box over the changed cell sees it; one beside it does not.
    assert_ne!(base.of_box(3, 2, 5, 4), moved.of_box(3, 2, 5, 4));
    assert_eq!(base.of_box(5, 0, 8, 6), moved.of_box(5, 0, 8, 6));
    assert_eq!(base.around(0, 2), moved.around(0, 2));
    // The prefix sums give a box's sum of the cells' digests.
    let cell_digest = |cell: usize| {
        let s = state(None)(cell);
        if s == 0 {
            0u128
        } else {
            let mut d = Digest128::default();
            d.u64(cell as u64);
            d.u128(s);
            d.finish()
        }
    };
    let mut sum = 0u128;
    for k in 1..=4 {
        for i in 2..=6 {
            sum = sum.wrapping_add(cell_digest(k * nx + i));
        }
    }
    let mut d = Digest128::default();
    for v in [2u64, 1, 6, 4] {
        d.u64(v);
    }
    d.u128(sum);
    assert_eq!(base.of_box(2, 1, 6, 4), d.finish());
    // Clamped to the grid; outside it, nothing.
    assert_eq!(base.of_box(-5, -5, 100, 100), base.of_box(0, 0, 8, 6));
    assert_eq!(base.of_box(20, 20, 30, 30), 0);
}

/// The test job's result: the floor `step` floors on, if the grid has it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Next(Option<NodeId>);

impl MemoCodec for Next {
    const TAG: u32 = 0x7e57;
    fn encode(&self, out: &mut Codec<'_>) {
        match self.0 {
            None => out.put_u8(0),
            Some(n) => {
                out.put_u8(1);
                out.put_node(n);
            }
        }
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        match input.u8()? {
            0 => Some(Next(None)),
            1 => Some(Next(Some(input.node()?))),
            _ => None,
        }
    }
}

crate::memo_ctx! {
    struct Ctx { step: u32, floors: u32 }
}

fn job(ctx: &Ctx, &a: &NodeId) -> Next {
    let b = a + ctx.step;
    Next((b < ctx.floors).then_some(b))
}

/// The item's stable id: enough for this job, whose answer depends on the item and the context only.
fn key(_: &Ctx, ids: &StableIds, &a: &NodeId) -> u128 {
    let mut d = Digest128::default();
    ids.id(a).feed(&mut d);
    d.finish()
}

const SITE: Site = Site { id: "test.next", source: 1 };

fn run(memo: &mut Memo, grid: &WalkGrid, ctx: &Ctx) -> Vec<Next> {
    let items: Vec<NodeId> = (0..grid.len() as NodeId).collect();
    memo.run(SITE, grid, ctx, &items, key, job)
}

#[test]
fn a_warm_memo_gives_the_cold_build_s_results() {
    let grid = two_rooms();
    let dir = scratch("warm");
    let ctx = Ctx { step: 3, floors: grid.len() as u32 };
    let reference: Vec<Next> = (0..grid.len() as NodeId).map(|a| job(&ctx, &a)).collect();
    let n = grid.len();

    let mut cold = Memo::new(Mode::Use, dir.clone(), 42);
    assert_eq!(run(&mut cold, &grid, &ctx), reference);
    assert_eq!((cold.stats[0].hits, cold.stats[0].misses, cold.stats[0].written), (0, n, n));

    let mut warm = Memo::new(Mode::Use, dir.clone(), 42);
    assert_eq!(run(&mut warm, &grid, &ctx), reference);
    assert_eq!((warm.stats[0].hits, warm.stats[0].misses, warm.stats[0].written), (n, 0, 0));
    assert!(warm.table().contains("site test.next"), "{}", warm.table());

    let mut verify = Memo::new(Mode::Verify { per_million: 1_000_000 }, dir.clone(), 42);
    assert_eq!(run(&mut verify, &grid, &ctx), reference);
    assert_eq!((verify.stats[0].hits, verify.stats[0].verified), (n, n));

    // Another context, other global inputs: other keys, all misses.
    let mut other = Memo::new(Mode::Use, dir.clone(), 42);
    let ctx5 = Ctx { step: 5, floors: grid.len() as u32 };
    run(&mut other, &grid, &ctx5);
    assert_eq!(other.stats[0].misses, n);
    let mut global = Memo::new(Mode::Use, dir.clone(), 43);
    run(&mut global, &grid, &ctx);
    assert_eq!(global.stats[0].misses, n);

    // Write-only runs everything; off is `parallel` itself and writes nothing.
    let mut write_only = Memo::new(Mode::WriteOnly, dir.clone(), 42);
    assert_eq!(run(&mut write_only, &grid, &ctx), reference);
    assert_eq!(write_only.stats[0].misses, n);
    let mut off = Memo::new(Mode::Off, scratch("off"), 42);
    assert_eq!(run(&mut off, &grid, &ctx), reference);
    assert!(!scratch("off").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_stored_floor_the_grid_does_not_have_is_an_error_and_a_miss() {
    // Under-keyed on purpose: the key leaves out where the floors are, so a grid with fewer floors finds results naming floors
    // it does not have.
    let grid = two_rooms();
    let dir = scratch("errors");
    let ctx = Ctx { step: 0, floors: u32::MAX };
    let items: Vec<NodeId> = (0..grid.len() as NodeId).collect();
    fn by_index(_: &Ctx, _: &StableIds, &a: &NodeId) -> u128 {
        u128::from(a)
    }
    let mut memo = Memo::new(Mode::Use, dir.clone(), 1);
    memo.run(SITE, &grid, &ctx, &items, by_index, job);
    // The same indices on a grid whose floors are elsewhere (moved up by a centimetre).
    let mut moved = two_rooms();
    moved.height.iter_mut().for_each(|h| *h += 0.01);
    let mut memo = Memo::new(Mode::Use, dir.clone(), 1);
    let out = memo.run(SITE, &moved, &ctx, &items, by_index, job);
    assert_eq!(out, items.iter().map(|&a| Next(Some(a))).collect::<Vec<_>>(), "the misses ran");
    assert_eq!((memo.stats[0].hits, memo.stats[0].errors), (0, items.len()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[should_panic(expected = "VERIFY FAILED at site test.next")]
fn verify_stops_at_a_stored_result_the_job_does_not_give() {
    let grid = two_rooms();
    let dir = scratch("verify");
    let items: Vec<NodeId> = (0..grid.len() as NodeId).collect();
    // A key that leaves out the context: a context change finds the old results.
    fn no_ctx(_: &Ctx, ids: &StableIds, &a: &NodeId) -> u128 {
        let mut d = Digest128::default();
        ids.id(a).feed(&mut d);
        d.finish()
    }
    let ctx = Ctx { step: 1, floors: grid.len() as u32 };
    Memo::new(Mode::Use, dir.clone(), 1).run(SITE, &grid, &ctx, &items, no_ctx, job);
    // The same keys, another answer: a job reading an input its key misses is what verify catches.
    fn other_job(_: &Ctx, &a: &NodeId) -> Next {
        Next(Some(a))
    }
    Memo::new(Mode::Verify { per_million: 1_000_000 }, dir, 1).run(SITE, &grid, &ctx, &items, no_ctx, other_job);
}

#[test]
fn a_context_field_is_in_the_key() {
    let a = Ctx { step: 1, floors: 2 };
    let b = Ctx { step: 2, floors: 1 };
    let digest = |c: &Ctx| {
        let mut d = Digest128::default();
        MemoCtx::feed(c, &mut d);
        d.finish()
    };
    assert_ne!(digest(&a), digest(&b));
    assert_eq!(digest(&a), digest(&Ctx { step: 1, floors: 2 }));
}

#[test]
fn every_body_field_is_in_a_memo_key() {
    let digest = |b: &crate::Body| {
        let mut d = Digest128::default();
        b.feed(&mut d);
        d.finish()
    };
    let base = crate::Body::default();
    let mut cap = base;
    cap.fastest_walk_cap *= 1.1;
    assert_ne!(digest(&base), digest(&cap), "the game data's fastest walk cap");
    let mut steps = base;
    steps.jump_air_steps = !steps.jump_air_steps;
    assert_ne!(digest(&base), digest(&steps));
    assert_eq!(digest(&base), digest(&crate::Body::default()));
}
