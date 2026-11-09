// SPDX-License-Identifier: GPL-3.0-or-later
//! `srp-game-data` run as the release scripts run it: what it writes, what it
//! refuses, and its exit status.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use resources_db::write::write;

/// A folder of its own under the system's temporary one, removed after.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("srp-game-data-run-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).display().to_string()
    }

    /// The archive of `files` at `name`.
    fn archive(&self, name: &str, files: &[(&str, &str)]) -> String {
        let files: BTreeMap<String, Vec<u8>> = files
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.as_bytes().to_vec()))
            .collect();
        std::fs::write(self.0.join(name), write(&files).unwrap()).unwrap();
        self.path(name)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_srp-game-data"))
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Every file in `folder`.
fn listing(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// The patches' base URL.
const BASE: &str = "http://203.0.113.5/patches";

/// A project of nothing: a binary config of an empty table (the root value,
/// its children at 24, none; `objects_of` keeps it as it is).
const PROJECT: &str = "\x18\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x03\0\0\0";

/// A game folder: resources.db and resources.db.patch_1.
fn game(folder: &Folder) -> (String, String) {
    let db = folder.archive(
        "resources.db",
        &[
            ("gameplay/a", "db a"),
            ("gameplay/b", "db b"),
            ("projects/p/client_project", PROJECT),
            ("models/m", "not read"),
        ],
    );
    let patch = folder.archive("resources.db.patch_1", &[("gameplay/a", "patch a")]);
    (db, patch)
}

#[test]
fn check_passes_on_what_build_wrote_and_fails_on_anything_else() {
    let folder = Folder::new("check");
    let (db, patch) = game(&folder);
    let data = folder.path("server-game-data.db");
    let archives = ["--db", &db, "--patch", &patch, "--patch-url", BASE];
    let built = run(&[&["build"][..], &archives, &["--out", &data]].concat());
    assert!(built.status.success(), "{}", stderr(&built));

    let check = |data: &str, archives: &[&str]| run(&[&["check", data][..], archives].concat());
    let same = check(&data, &archives);
    assert!(same.status.success(), "{}", stderr(&same));
    assert!(String::from_utf8_lossy(&same.stdout).contains("the bytes these archives give"));

    // Another URL: other provenance.
    let other_url = check(
        &data,
        &[
            "--db",
            &db,
            "--patch",
            &patch,
            "--patch-url",
            "http://203.0.113.6/p",
        ],
    );
    assert_eq!(other_url.status.code(), Some(1));
    assert!(stderr(&other_url).contains("made of something else"));

    // The same provenance, other files: made by hand, or damaged.
    let built_bytes = std::fs::read(&data).unwrap();
    let mut archive = resources_db::Archive::new(std::io::Cursor::new(built_bytes)).unwrap();
    let mut files = BTreeMap::new();
    let paths: Vec<String> = archive
        .files_under("")
        .map(|(path, _)| path.to_owned())
        .collect();
    for path in paths {
        let bytes = archive.read(&path).unwrap().unwrap();
        files.insert(path, bytes);
    }
    files.insert("gameplay/b".into(), b"other b".to_vec());
    let forged = folder.path("forged.db");
    std::fs::write(&forged, write(&files).unwrap()).unwrap();
    let other_files = check(&forged, &archives);
    assert_eq!(other_files.status.code(), Some(1));
    assert!(
        stderr(&other_files).contains("not the bytes they give now"),
        "{}",
        stderr(&other_files)
    );

    // Not server data at all, and no file.
    let other = check(&db, &archives);
    assert_eq!(other.status.code(), Some(1));
    let missing = check(&folder.path("missing.db"), &archives);
    assert_eq!(missing.status.code(), Some(1));
    // check writes nothing.
    assert!(
        check(&data, &[&archives[..], &["--out", "x"]].concat())
            .status
            .code()
            == Some(1)
    );
    assert_eq!(
        listing(&folder.0),
        [
            "forged.db",
            "resources.db",
            "resources.db.patch_1",
            "server-game-data.db"
        ]
    );
}

#[test]
fn build_and_pack_never_overwrite_and_a_failure_writes_nothing() {
    let folder = Folder::new("overwrite");
    let (db, patch) = game(&folder);
    let data = folder.path("server-game-data.db");
    std::fs::write(&data, b"there").unwrap();
    let archives = ["--db", &db, "--patch", &patch, "--patch-url", BASE];
    let built = run(&[&["build"][..], &archives, &["--out", &data]].concat());
    assert_eq!(built.status.code(), Some(1));
    assert!(
        stderr(&built).contains("never overwritten"),
        "{}",
        stderr(&built)
    );
    assert_eq!(std::fs::read(&data).unwrap(), b"there");

    // A build that fails writes nothing.
    let out = folder.path("new.db");
    let failed = run(&["build", "--db", &data, "--out", &out]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(
        stderr(&failed).contains("not a readable resources.db"),
        "{}",
        stderr(&failed)
    );

    let source = Folder::new("overwrite-source");
    std::fs::create_dir_all(source.0.join("gameplay")).unwrap();
    std::fs::write(source.0.join("gameplay/x"), b"x").unwrap();
    let dir = source.0.display().to_string();
    let packed = run(&["pack", "--dir", &dir, "--out", &patch]);
    assert_eq!(packed.status.code(), Some(1));
    assert!(stderr(&packed).contains("never overwritten"));

    let patch_2 = folder.path("resources.db.patch_2");
    let packed = run(&["pack", "--out", &patch_2, "--dir", &dir]);
    assert!(packed.status.success(), "{}", stderr(&packed));
    assert_eq!(
        listing(&folder.0),
        [
            "resources.db",
            "resources.db.patch_1",
            "resources.db.patch_2",
            "server-game-data.db"
        ]
    );
}

#[test]
fn pack_writes_only_a_name_the_game_mounts() {
    let source = Folder::new("names-source");
    std::fs::write(source.0.join("x"), b"x").unwrap();
    let dir = source.0.display().to_string();
    let out = Folder::new("names");
    for name in [
        "patch.db",
        "resources.db.patch_0",
        "resources.db.patch_01",
        "resources.db.patch_256",
        "resources.db.patch_1.db",
    ] {
        let packed = run(&["pack", "--dir", &dir, "--out", &out.path(name)]);
        assert_eq!(packed.status.code(), Some(1), "{name}");
        assert!(stderr(&packed).contains("the game mounts an archive only by the name"));
    }
    assert!(listing(&out.0).is_empty());
    for name in ["resources.db.patch_255", "resources.db"] {
        let packed = run(&["pack", "--dir", &dir, "--out", &out.path(name)]);
        assert!(packed.status.success(), "{name}: {}", stderr(&packed));
    }
}

/// A patch with mirrors, as release/build-server-db gives it (the url
/// column of patches.tsv): `info` and `check` name its URLs in their order,
/// and other mirrors, or another order, are another provenance.
#[test]
fn mirrors_are_given_one_space_apart_and_shown_by_info_and_check() {
    let folder = Folder::new("mirrors");
    let (db, patch) = game(&folder);
    let data = folder.path("server-game-data.db");
    let urls = "http://203.0.113.5/patches/resources.db.patch_1 \
                https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t";
    let given = format!("{patch}={urls}");
    let archives = ["--db", &db, "--patch", &given];
    let built = run(&[&["build"][..], &archives, &["--out", &data]].concat());
    assert!(built.status.success(), "{}", stderr(&built));
    let line = format!(" urls {urls}\n");
    assert!(String::from_utf8_lossy(&built.stdout).contains(&line));

    let info = run(&["info", &data]);
    assert!(info.status.success(), "{}", stderr(&info));
    let info = String::from_utf8_lossy(&info.stdout).into_owned();
    assert!(
        info.contains("  resources.db.patch_1 ") && info.contains(&line),
        "{info}"
    );

    let check = |given: &str| run(&["check", &data, "--db", &db, "--patch", given]);
    let same = check(&given);
    assert!(same.status.success(), "{}", stderr(&same));
    assert!(String::from_utf8_lossy(&same.stdout).contains(&line));
    let reversed: Vec<&str> = urls.split(' ').rev().collect();
    for other in [
        format!("{patch}={}", reversed.join(" ")),
        format!("{patch}={}", urls.split(' ').next().unwrap()),
        format!("{patch}={urls} http://198.51.100.7/p1"),
    ] {
        let found = check(&other);
        assert_eq!(found.status.code(), Some(1), "{other}");
        assert!(stderr(&found).contains("made of something else"), "{other}");
    }

    // A URL twice, nine of them: refused, nothing written.
    let out = folder.path("refused.db");
    let twice = format!("{patch}=http://h/p http://h/p");
    let refused = run(&["build", "--db", &db, "--patch", &twice, "--out", &out]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        stderr(&refused).contains("is given twice"),
        "{}",
        stderr(&refused)
    );
    let nine: Vec<String> = (1..=9).map(|n| format!("http://m{n}/p")).collect();
    let nine = format!("{patch}={}", nine.join(" "));
    let refused = run(&["build", "--db", &db, "--patch", &nine, "--out", &out]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        stderr(&refused).contains("at most 8"),
        "{}",
        stderr(&refused)
    );
    assert!(!Path::new(&out).exists());
}
