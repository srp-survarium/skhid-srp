use std::vec::Vec;

use super::*;
use crate::report::{self, Body, Carrier};

fn encode(paths: &[&str]) -> Option<Vec<u8>> {
    let mut out = [0u8; MAX_REQUEST];
    let len = Request::encode(7, paths.iter().map(|path| path.as_bytes()), &mut out)?;
    Some(out[..len].to_vec())
}

fn paths_of(request: &Request) -> Vec<Vec<u8>> {
    let mut paths = request.paths();
    let mut all = Vec::new();
    while let Some(path) = paths.next_path() {
        all.push(path.to_vec());
    }
    all
}

const PATHS: [&str; 4] = [
    "material_instances/flora/grass_l01.material",
    "projects/level_01/day/client_project",
    "projects/level_01/vegetation/green.veg",
    "startup.cfg",
];

#[test]
fn a_request_reads_back_its_paths_shared_heads_once() {
    let bytes = encode(&PATHS).unwrap();
    let plain: usize = PATHS.iter().map(|path| 2 + path.len()).sum();
    assert!(bytes.len() < REQUEST_HEAD + plain, "front coded");
    let request = Request::parse(&bytes).unwrap();
    assert_eq!(request.match_id, 7);
    assert_eq!(request.len(), 4);
    let read: Vec<Vec<u8>> = PATHS.iter().map(|path| path.as_bytes().to_vec()).collect();
    assert_eq!(paths_of(&request), read);
    let empty = encode(&[]).unwrap();
    assert!(Request::parse(&empty).unwrap().is_empty());
}

#[test]
fn paths_out_of_order_or_unlike_a_path_are_not_encoded() {
    assert!(encode(&["b", "a"]).is_none());
    assert!(encode(&["a", "a"]).is_none());
    assert!(encode(&["has space"]).is_none());
    assert!(encode(&[""]).is_none());
    let long = "x".repeat(MAX_PATH + 1);
    assert!(encode(&[long.as_str()]).is_none());
    let max = "x".repeat(MAX_PATH);
    assert!(encode(&[max.as_str()]).is_some());
    let many: Vec<std::string::String> = (0..=MAX_FILES).map(|n| std::format!("f{n:04}")).collect();
    let many: Vec<&str> = many.iter().map(std::string::String::as_str).collect();
    assert!(encode(&many).is_none());
    assert!(encode(&many[..MAX_FILES]).is_some());
}

#[test]
fn a_request_unlike_the_format_is_refused() {
    let good = encode(&PATHS).unwrap();
    let mut bad = Vec::new();
    // Another magic, a count unlike the records, a byte after the last.
    let mut other = good.clone();
    other[3] = 2;
    bad.push(other);
    let mut fewer = good.clone();
    fewer[8] = 3;
    bad.push(fewer);
    let mut more = good.clone();
    more[8] = 5;
    bad.push(more);
    let mut after = good.clone();
    after.push(0);
    bad.push(after);
    // A record that shares more than the path before has.
    let mut shared = encode(&["ab", "abc"]).unwrap();
    shared[REQUEST_HEAD + 4] = 3;
    bad.push(shared);
    // Paths not ascending, read as given.
    let mut swapped = Vec::from(&good[..REQUEST_HEAD]);
    swapped[8] = 2;
    swapped.extend([0, 1, b'b', 0, 1, b'a']);
    bad.push(swapped);
    // A path byte that is no path's.
    let mut space = encode(&["ab"]).unwrap();
    *space.last_mut().unwrap() = b' ';
    bad.push(space);
    // Cut anywhere.
    for len in 0..good.len() {
        bad.push(good[..len].to_vec());
    }
    for bytes in bad {
        assert!(Request::parse(&bytes).is_none(), "{bytes:?}");
    }
}

fn hashed(n: u32) -> Answer {
    Answer::Hashed { size: n, sha256: [n as u8; PREFIX] }
}

fn carriers(report: &Report) -> Vec<Carrier> {
    let report = report::Report::Checks(report.clone());
    (0..report.parts())
        .map(|part| {
            let (bytes, len) = report.part(9, part).unwrap();
            assert_eq!(report::carrier_len(&bytes[..len]), Some(len));
            Carrier::parse(&bytes[..len]).unwrap()
        })
        .collect()
}

#[test]
fn a_report_reads_back_its_mounts_and_every_answer_in_order() {
    let mounts = Mounts::of(&[12, 14, 73]).unwrap();
    let mut report = Report::new(mounts);
    let answers: Vec<Answer> = (0..MAX_FILES as u32)
        .map(|n| match n % 5 {
            3 => Answer::Missing,
            4 => Answer::Unreadable,
            _ => hashed(n),
        })
        .collect();
    for answer in &answers {
        report.add(*answer).unwrap();
    }
    assert!(report.add(Answer::Missing).is_none(), "no more than MAX_FILES");
    let carriers = carriers(&report);
    assert_eq!(carriers.len(), usize::from(MAX_PARTS));
    let Body::Checks(Part::Mounts { files, mounts: read }) = carriers[0].body else { panic!() };
    assert_eq!((files, read.numbers()), (MAX_FILES as u16, &[12, 14, 73][..]));
    let mut read = Vec::new();
    for (index, carrier) in carriers.iter().enumerate().skip(1) {
        let Body::Checks(part) = carrier.body else { panic!() };
        let Part::Answers { first, .. } = part else { panic!() };
        assert_eq!(usize::from(first), (index - 1) * PER_PART);
        assert_eq!(carrier.key(), (3, 1, index as u8));
        read.extend_from_slice(part.answers());
    }
    assert_eq!(read, answers);
}

#[test]
fn a_report_of_nothing_asked_is_its_mounts_alone() {
    let carriers = carriers(&Report::new(Mounts::NONE));
    assert_eq!(carriers.len(), 1);
    assert_eq!(
        carriers[0].body,
        Body::Checks(Part::Mounts { files: 0, mounts: Mounts::NONE })
    );
}

#[test]
fn mounts_are_patch_numbers_ascending() {
    assert!(Mounts::of(&[]).is_some());
    assert!(Mounts::of(&[1, 255]).is_some());
    assert!(Mounts::of(&[0, 1]).is_none());
    assert!(Mounts::of(&[14, 12]).is_none());
    assert!(Mounts::of(&[12, 12]).is_none());
    let all: Vec<u8> = (1..=255).collect();
    let mounts = Mounts::of(&all).unwrap();
    let mut report = Report::new(mounts);
    report.add(hashed(1)).unwrap();
    let carriers = carriers(&report);
    let Body::Checks(Part::Mounts { mounts: read, .. }) = carriers[0].body else { panic!() };
    assert_eq!(read.numbers(), &all[..]);
}

#[test]
fn a_part_unlike_the_format_is_refused() {
    let mut report = Report::new(Mounts::of(&[12]).unwrap());
    for n in 0..40 {
        report.add(hashed(n)).unwrap();
    }
    let report = report::Report::Checks(report);
    let (part0, len0) = report.part(9, 0).unwrap();
    let (part1, len1) = report.part(9, 1).unwrap();
    let good = [part0[..len0].to_vec(), part1[..len1].to_vec()];
    let mut bad = Vec::new();
    // Mounts as a later part, answers as part 0.
    let mut moved = good[0].clone();
    moved[13] = 1;
    bad.push(moved);
    let mut moved = good[1].clone();
    moved[13] = 0;
    bad.push(moved);
    // Answers of a part from where the parts before do not end.
    let mut first = good[1].clone();
    first[16] = 1;
    bad.push(first);
    // An answer of no kind, a mount 0.
    let mut kind = good[1].clone();
    kind[19] = 7;
    bad.push(kind);
    let mut zero = good[0].clone();
    *zero.last_mut().unwrap() = 0;
    bad.push(zero);
    // A byte after the body.
    let mut after = good[1].clone();
    after.push(0);
    bad.push(after);
    for bytes in bad {
        assert!(Carrier::parse(&bytes).is_none(), "{bytes:?}");
    }
}

#[test]
fn the_offer_and_the_answer_are_told_apart() {
    let mounts = Mounts::of(&[12, 14]).unwrap();
    let offer = carriers(&Report::offer(mounts));
    assert_eq!(offer.len(), 1);
    assert_eq!(offer[0].key(), (3, 0, 0));
    let mut answer = Report::new(mounts);
    answer.add(Answer::Missing).unwrap();
    let answer = carriers(&answer);
    assert_eq!(answer.iter().map(Carrier::key).collect::<Vec<_>>(), [(3, 1, 0), (3, 1, 1)]);
}
