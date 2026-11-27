// Host-only: which crash files a folder keeps (src/crash_files.rs).
#[path = "../src/crash_files.rs"]
mod crash_files;
use crash_files::{Folder, KEEP, Part, parse};

const ME: u32 = 4242;

fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}
/// The `i`-th crash's name of process `process`, written at minute `i`.
fn crash(i: u32, process: u32, extension: &str) -> String {
    format!("SRP-crash-20261103-{:02}{:02}00-123-p{process}-t{}-0{extension}", i / 60, i % 60, process + 1)
}
/// The names that go, sorted.
fn going(files: &[(String, u64)]) -> Vec<String> {
    let mut folder = Box::new(Folder::new());
    for (name, written) in files {
        folder.add(&wide(name), *written);
    }
    let mut out: Vec<String> = folder.going(ME).map(|found| found.file.to_string()).collect();
    out.sort();
    let n = out.len();
    out.dedup();
    assert_eq!(n, out.len(), "a file chosen twice");
    out
}
/// Crashes `0..n` of other processes, each a ZIP and its report, crash i written at i.
fn crashes(n: u32) -> Vec<(String, u64)> {
    (0..n)
        .flat_map(|i| [(crash(i, 100 + i, ".zip"), u64::from(i) * 10), (crash(i, 100 + i, ".txt"), u64::from(i) * 10 + 1)])
        .collect()
}

#[test]
fn names_of_ours_only() {
    let name = "SRP-crash-20261103-142530-007-p1234-t5678-0.zip";
    let file = parse(&wide(name)).unwrap();
    assert_eq!((file.id.process, file.part), (1234, Part::Zip));
    assert_eq!(file.to_string(), name);
    for (extension, part) in [(".dmp", Part::Dump), (".txt", Part::Report), (".zip", Part::Zip), (".zip.part", Part::PartialZip)] {
        let name = format!("SRP-crash-20261103-142530-007-p1-t2-13{extension}");
        let file = parse(&wide(&name)).unwrap();
        assert_eq!(file.part, part);
        assert_eq!(file.to_string(), name, "the name back exactly");
    }
    for name in [
        "SRP-crash-20261103-142530-007-p1234-t5678-0",            // an unpacked ZIP's folder
        "SRP-crash-20261103-142530-007-p1234-t5678-0.ZIP",        // not how it writes them
        "srp-crash-20261103-142530-007-p1234-t5678-0.zip",
        "SRP-crash-20261103-142530-007-p1234-t5678-0 (1).zip",    // a browser's copy
        "SRP-crash-20261103-142530-007-p1234-t5678-0.zip.bak",
        "SRP-crash-20261103-142530-007-p1234-t5678-0.zip.part.txt",
        "SRP-crash-20261103-142530-07-p1234-t5678-0.zip",
        "SRP-crash-2026110-142530-007-p1234-t5678-0.zip",
        "SRP-crash-20261103-142530-007-p01234-t5678-0.zip",       // a leading zero
        "SRP-crash-20261103-142530-007-p1234-t5678-00.zip",
        "SRP-crash-20261103-142530-007-p4294967296-t5678-0.zip",  // past u32
        "SRP-crash-20261103-142530-007-p1234-5678-0.zip",
        "SRP-crash-20261102-1.txt",
        "SRP-crash-old.zip",
        "Copy of SRP-crash-20261103-142530-007-p1234-t5678-0.zip",
        "SRP-crash-２0261103-142530-007-p1234-t5678-0.zip",       // a full-width digit
    ] {
        assert!(parse(&wide(name)).is_none(), "{name}");
    }
}

#[test]
fn ten_or_fewer_stay() {
    for n in 0..=KEEP as u32 {
        assert!(going(&crashes(n)).is_empty(), "{n}");
    }
}

#[test]
fn the_oldest_go_whole() {
    let files = crashes(13);
    let mut expect: Vec<String> = files[..6].iter().map(|(name, _)| name.clone()).collect();
    expect.sort();
    assert_eq!(going(&files), expect, "crashes 0, 1 and 2, both files of each");
}

#[test]
fn a_crash_is_dated_by_its_newest_file() {
    // Crash 0's report written last of all: it is the newest crash.
    let mut files = crashes(11);
    files[1].1 = 1000;
    assert_eq!(going(&files), vec![crash(1, 101, ".txt"), crash(1, 101, ".zip")]);
}

#[test]
fn this_process_never() {
    // Twelve old crashes of this process (a snapshot each, say) and eleven
    // of others: only the others' oldest goes, and this one's do not count.
    let mut files = crashes(11);
    for i in 0..12 {
        files.push((format!("SRP-crash-20261101-0000{i:02}-000-p{ME}-t1-{i}.zip"), i as u64));
    }
    files.push((format!("SRP-crash-20261101-000000-000-p{ME}-t1-99.zip.part"), 0));
    assert_eq!(going(&files), vec![crash(0, 100, ".txt"), crash(0, 100, ".zip")]);
}

#[test]
fn a_part_left_over_goes() {
    // A crash zipped when the game died: its .dmp and .part stay beside the
    // report. The .part goes; the crash is kept, counted once.
    let mut files = crashes(9);
    files.push((crash(50, 7, ".dmp"), 500));
    files.push((crash(50, 7, ".txt"), 501));
    files.push((crash(50, 7, ".zip.part"), 9999));
    assert_eq!(going(&files), vec![crash(50, 7, ".zip.part")]);
    // A .part alone dates nothing: with eleven other crashes the oldest goes.
    files.extend(crashes(11).into_iter().skip(18));
    assert_eq!(going(&files).len(), 1 + 2 * 2, "{:?}", going(&files));
}

#[test]
fn similar_names_are_not_counted() {
    let mut files = crashes(10);
    for name in ["SRP-crash-20261103-000000-000-p1-t2-0 (1).zip", "SRP-crash-old.zip", "SRP-crash-20261103-000000-000-p1-t2-0"] {
        files.push((name.into(), 0));
    }
    assert!(going(&files).is_empty());
}

#[test]
fn mixed_kinds() {
    // ZIPs, a raw dump kept when zipping failed, a full dump with its
    // report, a report alone (no dump): five crashes each kind, the oldest
    // of each 2-file crash going whole.
    let mut files = Vec::new();
    let mut at = 0;
    for (i, kinds) in [&[".zip", ".txt"][..], &[".dmp", ".txt"], &[".txt"], &[".dmp"]].iter().cycle().take(14).enumerate() {
        for extension in kinds.iter() {
            files.push((crash(i as u32, 200 + i as u32, extension), at));
            at += 1;
        }
    }
    let mut expect: Vec<String> = files.iter().map(|(n, _)| n.clone()).filter(|n| n.contains("-p200-") || n.contains("-p201-") || n.contains("-p202-") || n.contains("-p203-")).collect();
    expect.sort();
    assert_eq!(going(&files), expect);
}

#[test]
fn two_folders_counted_apart() {
    // Eight crashes beside the game and eight in the temporary folder: each
    // folder is listed and decided alone, so nothing goes from either.
    let beside = crashes(8);
    let temp: Vec<_> = (20..28).map(|i| (crash(i, 100 + i, ".txt"), u64::from(i))).collect();
    assert!(going(&beside).is_empty() && going(&temp).is_empty());
    // Together they would be sixteen: the point of one Folder per folder.
    assert!(!going(&[beside, temp].concat()).is_empty());
}

#[test]
fn bounded() {
    // More files than a folder takes: only those seen are decided, and with
    // more than ten newer seen, the oldest seen still go.
    let files = crashes(400);
    let gone = going(&files);
    assert!(!gone.is_empty() && gone.len() <= crash_files::MOST);
    assert!(gone.iter().all(|name| !name.contains("-p399-")));
}
