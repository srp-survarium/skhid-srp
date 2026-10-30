// SPDX-License-Identifier: GPL-3.0-or-later
use std::io::Cursor;

use super::*;
use crate::overlay::tests::archive_of;

/// A folder of its own under the system's temporary one, emptied first and
/// removed after.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("resources-db-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const URL: &str = "http://203.0.113.5/files";

fn patch(path: &Path) -> PatchFile {
    PatchFile {
        path: path.to_owned(),
        urls: vec![url_in(URL, path.file_name().unwrap().to_str().unwrap())],
    }
}

/// An archive of `list`, its files given as bytes.
fn archive_of_bytes(list: &[(&str, &[u8])]) -> Vec<u8> {
    let files: BTreeMap<String, Vec<u8>> = list
        .iter()
        .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
        .collect();
    crate::write::write(&files).unwrap()
}

/// A `client_project` of `respawn_points` points, with a world srp does
/// not read beside its `server_objects`; and what the data keeps of it.
fn project(respawn_points: usize) -> (Vec<u8>, Vec<u8>) {
    use binary_config_parser::tree::ConfigValue;
    let point = ConfigValue::Named(vec![
        ("team".into(), ConfigValue::Integer(1)),
        ("position".into(), ConfigValue::Float3([1.0, 2.0, 3.0])),
    ]);
    let server_objects = (
        SERVER_OBJECTS.to_owned(),
        ConfigValue::Named(vec![(
            "respawn_points".into(),
            ConfigValue::Indexed(vec![point; respawn_points]),
        )]),
    );
    let world = (
        "collision_objects".to_owned(),
        ConfigValue::Indexed(vec![ConfigValue::String("a wall".into()); 50]),
    );
    let whole = ConfigValue::Named(vec![world, server_objects.clone()]);
    let kept = ConfigValue::Named(vec![server_objects]);
    (whole.to_bytes().unwrap(), kept.to_bytes().unwrap())
}

/// A project of no `server_objects` (`lobby_scene`), and what is kept of
/// it: a config of nothing.
fn bare_project() -> (Vec<u8>, Vec<u8>) {
    use binary_config_parser::tree::ConfigValue;
    let whole = ConfigValue::Named(vec![("camera".into(), ConfigValue::Integer(1))]);
    let kept = ConfigValue::Named(Vec::new());
    (whole.to_bytes().unwrap(), kept.to_bytes().unwrap())
}

/// `resources.db` and patches 2 and 14 over it: an item changed by both,
/// one added by each, a folder that only a patch has, a project in the db
/// and one in patch 14, the English texts, files the server does not read.
fn game(folder: &Folder) -> (PathBuf, PathBuf, PathBuf) {
    let db = folder.file(
        RESOURCES_DB,
        &archive_of_bytes(&[
            ("gameplay/db_static_dictionaries", b"dictionary 0"),
            ("gameplay/items/item.options", b"item 0"),
            ("gameplay/items/kept.options", b"kept 0"),
            ("projects/level_01/day/client_project", &project(3).0),
            ("projects/level_01/day/level.bin", b"not read"),
            ("localization/english/localization", b"english"),
            ("localization/german/localization", b"not read"),
            ("textures/a.dds", b"not read"),
        ]),
    );
    let two = folder.file(
        "resources.db.patch_2",
        &archive_of(&[
            ("gameplay/items/item.options", "item 2"),
            ("gameplay/items/two.options", "two"),
            ("menus/main.xml", "not read"),
        ]),
    );
    let fourteen = folder.file(
        "resources.db.patch_14",
        &archive_of_bytes(&[
            ("gameplay/items/item.options", b"item 14"),
            ("gameplay/items/armour/mask/new.options", b"fourteen"),
            ("projects/level_02/day/client_project", &bare_project().0),
        ]),
    );
    (db, two, fourteen)
}

fn all_files(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = Archive::new(Cursor::new(bytes.to_vec())).unwrap();
    let paths: Vec<String> = archive
        .files_under("")
        .map(|(path, _)| path.to_owned())
        .collect();
    paths
        .into_iter()
        .map(|path| {
            let bytes = archive.read(&path).unwrap().unwrap();
            (path, bytes)
        })
        .collect()
}

#[test]
fn a_patchs_number_is_its_name_as_the_game_writes_it() {
    assert_eq!(patch_number("resources.db.patch_1"), Some(1));
    assert_eq!(patch_number("resources.db.patch_14"), Some(14));
    assert_eq!(patch_number("resources.db.patch_255"), Some(255));
    for other in [
        "resources.db.patch_0",
        "resources.db.patch_01",
        "resources.db.patch_256",
        "resources.db.patch_+1",
        "resources.db.patch_",
        "resources.db.patch_1.db",
        "resources.db.patch_1 ",
        "resources.db",
        "Resources.db.patch_1",
    ] {
        assert_eq!(patch_number(other), None, "{other}");
    }
}

#[test]
fn a_game_folder_gives_its_patches_in_the_games_order_with_gaps() {
    let folder = Folder::new("game-folder");
    let empty = archive_of(&[("gameplay/x", "x")]);
    for name in [
        RESOURCES_DB,
        "resources.db.patch_14",
        "resources.db.patch_2",
        "resources.db.patch_255",
        "resources.db.patch_016",
        "resources.db.patch_256",
        "resources.db.patch_0",
        "resources.db.patch_1.db",
    ] {
        folder.file(name, &empty);
    }
    std::fs::create_dir(folder.0.join("resources.db.patch_3")).unwrap();
    let (db, patches) = archives_in(&folder.0).unwrap();
    assert_eq!(db, folder.0.join(RESOURCES_DB));
    let names: Vec<&str> = patches
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "resources.db.patch_2",
            "resources.db.patch_14",
            "resources.db.patch_255"
        ]
    );
    std::fs::remove_file(&db).unwrap();
    assert!(archives_in(&folder.0).is_err(), "no resources.db");
}

#[test]
fn the_server_data_is_the_files_srp_reads_of_the_patches_over_the_db() {
    let folder = Folder::new("build");
    let (db, two, fourteen) = game(&folder);
    // Given out of order: laid in the game's order all the same.
    let built = build(&db, &[patch(&fourteen), patch(&two)]).unwrap();
    let files = all_files(&built.bytes);
    let mut expected: BTreeMap<String, Vec<u8>> = [
        ("gameplay/db_static_dictionaries", "dictionary 0"),
        ("gameplay/items/armour/mask/new.options", "fourteen"),
        ("gameplay/items/item.options", "item 14"),
        ("gameplay/items/kept.options", "kept 0"),
        ("gameplay/items/two.options", "two"),
        ("localization/english/localization", "english"),
    ]
    .into_iter()
    .map(|(path, bytes)| (path.to_owned(), bytes.as_bytes().to_vec()))
    .collect();
    // The projects, cut to their server objects.
    expected.insert("projects/level_01/day/client_project".into(), project(3).1);
    expected.insert(
        "projects/level_02/day/client_project".into(),
        bare_project().1,
    );
    expected.insert(PROVENANCE.into(), built.provenance.text().into_bytes());
    // No clips in this game: the list is there, and empty.
    expected.insert(CLIP_LENGTHS.into(), ClipLengths::default().text().into_bytes());
    assert_eq!(files, expected);
    assert_eq!((built.gameplay, built.projects), (5, 2));
    assert_eq!((built.clips, built.clips_unread), (0, 0));
    assert_eq!(
        built.given,
        [
            Given {
                files: 5,
                replaced: 0
            },
            Given {
                files: 2,
                replaced: 1
            },
            Given {
                files: 3,
                replaced: 1
            },
        ]
    );

    let provenance = &built.provenance;
    assert_eq!(provenance.db, Input::of(&db).unwrap());
    assert_eq!(provenance.db.name, RESOURCES_DB);
    assert_eq!(provenance.db.size, std::fs::metadata(&db).unwrap().len());
    let numbers: Vec<u32> = provenance.patches.iter().map(|p| p.number).collect();
    assert_eq!(numbers, [2, 14]);
    assert_eq!(provenance.patches[1].input, Input::of(&fourteen).unwrap());
    assert_eq!(
        provenance.patches[1].urls,
        ["http://203.0.113.5/files/resources.db.patch_14"]
    );
    let text = provenance.text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], FORMAT);
    assert_eq!(
        lines[1],
        format!(
            "db resources.db {} {}",
            provenance.db.size, provenance.db.sha256
        )
    );
    assert!(lines[2].starts_with("patch resources.db.patch_2 "));
    assert!(lines[3].ends_with(" http://203.0.113.5/files/resources.db.patch_14"));
    assert_eq!(lines.len(), 4);

    // What `info` reads back, every file checked.
    let data = read(&mut Archive::new(Cursor::new(built.bytes.clone())).unwrap()).unwrap();
    assert_eq!(data.provenance, *provenance);
    assert_eq!((data.gameplay, data.projects), (5, 2));
    // What srp offers the players' updater.
    let offered: Vec<(String, &str, &[String])> = data.provenance.offered().collect();
    assert_eq!(
        offered,
        [
            (
                "patch_2".to_owned(),
                provenance.patches[0].input.sha256.as_str(),
                &["http://203.0.113.5/files/resources.db.patch_2".to_owned()][..]
            ),
            (
                "patch_14".to_owned(),
                provenance.patches[1].input.sha256.as_str(),
                &["http://203.0.113.5/files/resources.db.patch_14".to_owned()][..]
            ),
        ]
    );
}

#[test]
fn the_clip_lengths_are_those_of_the_clips_seen_with_the_patches_over_the_db() {
    use crate::clips::tests::clip;
    let folder = Folder::new("clips");
    let long = clip(&[0.0, 0.0, 0.0, 0.0, 10.0, 40.0, 125.0, 125.0]);
    let short = clip(&[0.0, 0.0, 0.0, 0.0, 10.0, 40.0, 65.0, 65.0]);
    let db = folder.file(
        RESOURCES_DB,
        &archive_of_bytes(&[
            ("gameplay/items/item.options", b"item 0"),
            ("animations/single/weapons/pm/item/stand/3rd_view/reload.b-spline", &short),
            ("animations/single/weapons/pm/item/stand/3rd_view/reload_empty.b-spline", &short),
            ("animations/single/weapons/cut.b-spline", &long[..long.len() - 3]),
            ("animations/single/weapons/not_a_clip.txt", b"not read"),
        ]),
    );
    let two = folder.file(
        "resources.db.patch_2",
        &archive_of_bytes(&[(
            "animations/single/weapons/pm/item/stand/3rd_view/reload_empty.b-spline",
            &long,
        )]),
    );
    let built = build(&db, &[patch(&two)]).unwrap();
    // The patch's clip is the one the game plays; the cut one has no length.
    assert_eq!((built.clips, built.clips_unread), (2, 1));
    let files = all_files(&built.bytes);
    let text = String::from_utf8(files[CLIP_LENGTHS].clone()).unwrap();
    let clips = ClipLengths::parse(&text).unwrap();
    let reload = "resources/animations/single/weapons/pm/item/stand/3rd_view/reload";
    assert_eq!(clips.get(reload), Some(65.0));
    assert_eq!(clips.get(&format!("{reload}_empty")), Some(125.0));
    assert_eq!(clips.get("resources/animations/single/weapons/cut"), None);
    assert_eq!(clips.text(), text);
    // What `info` reads back still checks every file.
    read(&mut Archive::new(Cursor::new(built.bytes.clone())).unwrap()).unwrap();
}

#[test]
fn a_clip_length_list_reads_back_and_a_wrong_one_is_refused() {
    let mut clips = ClipLengths::default();
    clips.insert("resources/animations/a/reload".into(), 65.0);
    clips.insert("resources/animations/a/reload_empty".into(), 125.5);
    assert_eq!(ClipLengths::parse(&clips.text()).unwrap(), clips);
    assert_eq!(
        ClipLengths::name_of("animations/a/reload.b-spline").as_deref(),
        Some("resources/animations/a/reload")
    );
    assert_eq!(ClipLengths::name_of("animations/a/reload.txt"), None);
    for (bad, why) in [
        ("", "starts with"),
        ("clip lengths\n", "starts with"),
        ("skhid clip lengths 1\nno tab\n", "name<TAB>frames"),
        ("skhid clip lengths 1\na\tx\n", "not a length"),
        ("skhid clip lengths 1\na\t-1\n", "not a length"),
        ("skhid clip lengths 1\na\tinf\n", "not a length"),
        ("skhid clip lengths 1\na\t1\na\t2\n", "twice"),
    ] {
        let message = ClipLengths::parse(bad).unwrap_err().to_string();
        assert!(message.contains(why), "{bad:?}: {message}");
    }
}

#[test]
fn the_same_inputs_give_the_same_bytes_and_other_inputs_other_bytes() {
    let folder = Folder::new("determinism");
    let (db, two, fourteen) = game(&folder);
    let first = build(&db, &[patch(&two), patch(&fourteen)]).unwrap();
    let again = build(&db, &[patch(&fourteen), patch(&two)]).unwrap();
    assert_eq!(first.bytes, again.bytes);
    // Another URL: the same files, another provenance.
    let mut moved = patch(&two);
    moved.urls = vec!["https://patches.example/resources.db.patch_2".into()];
    let other = build(&db, &[moved, patch(&fourteen)]).unwrap();
    assert_ne!(other.bytes, first.bytes);
    assert_eq!(all_files(&other.bytes).len(), all_files(&first.bytes).len());
    // A patch of other bytes, even of no file the server reads.
    let alone = build(&db, &[patch(&fourteen)]).unwrap();
    std::fs::write(
        &two,
        archive_of(&[
            ("menus/main.xml", "other"),
            ("gameplay/items/kept.options", "kept 0"),
        ]),
    )
    .unwrap();
    let changed = build(&db, &[patch(&two), patch(&fourteen)]).unwrap();
    assert_ne!(changed.provenance, first.provenance);
    assert_ne!(
        changed.provenance.patches[0].input.sha256,
        first.provenance.patches[0].input.sha256
    );
    // Its files are those of 14 alone over the db, but the data names it.
    let without = |bytes: &[u8]| {
        let mut files = all_files(bytes);
        files.remove(PROVENANCE);
        files
    };
    assert_eq!(without(&changed.bytes), without(&alone.bytes));
    assert_ne!(changed.bytes, alone.bytes);
    // No patch at all.
    let bare = build(&db, &[]).unwrap();
    assert!(bare.provenance.patches.is_empty());
    assert_eq!(bare.gameplay, 3);
}

#[test]
fn a_project_that_is_not_a_config_is_refused() {
    let folder = Folder::new("bad-project");
    let (db, _, _) = game(&folder);
    for bytes in [&b""[..], b"project bytes", &project(2).0[..30]] {
        let patch = folder.file(
            "resources.db.patch_3",
            &archive_of_bytes(&[("projects/level_03/day/client_project", bytes)]),
        );
        let message = build(&db, &[self::patch(&patch)]).unwrap_err().to_string();
        assert!(
            message.contains("resources.db.patch_3: projects/level_03/day/client_project:"),
            "{message}"
        );
    }
    // What is kept of a project reads as its server objects.
    let kept = server_objects_of(&project(4).0).unwrap();
    let kept = binary_config_parser::ConfigValue::parse(&kept).unwrap();
    let points = kept.get(SERVER_OBJECTS).unwrap().get("respawn_points");
    assert_eq!(points.unwrap().children().len(), 4);
    assert!(kept.get("collision_objects").is_none());
}

#[test]
fn what_the_game_would_not_mount_or_the_players_could_not_fetch_is_refused() {
    let folder = Folder::new("refused");
    let (db, two, fourteen) = game(&folder);
    let renamed = folder.file("patch_15.db", &std::fs::read(&two).unwrap());
    let message = build(&db, &[patch(&renamed)]).unwrap_err().to_string();
    assert!(message.contains("resources.db.patch_<n>"), "{message}");

    let other = Folder::new("refused-other");
    let again = other.file("resources.db.patch_2", &std::fs::read(&two).unwrap());
    let message = build(&db, &[patch(&two), patch(&again)])
        .unwrap_err()
        .to_string();
    assert!(message.contains("two patches of one number"), "{message}");

    for url in [
        "",
        "ftp://x/y",
        "http://",
        "http://x/a b",
        "http://x/\"",
        "/files/x",
    ] {
        let bad = PatchFile {
            path: fourteen.clone(),
            urls: vec![url.into()],
        };
        assert!(build(&db, &[bad]).is_err(), "{url:?}");
    }

    // A file unlike its table, named with its archive.
    let mut damaged = std::fs::read(&fourteen).unwrap();
    let at = damaged.windows(8).position(|w| w == b"fourteen").unwrap();
    damaged[at] = b'F';
    std::fs::write(&fourteen, damaged).unwrap();
    let message = build(&db, &[patch(&fourteen)]).unwrap_err().to_string();
    assert!(message.contains("resources.db.patch_14"), "{message}");
    assert!(
        message.contains("gameplay/items/armour/mask/new.options"),
        "{message}"
    );

    // A file over a folder.
    let clash = folder.file(
        "resources.db.patch_20",
        &archive_of(&[("gameplay/items", "x")]),
    );
    assert!(build(&db, &[patch(&clash)]).is_err());

    // No gameplay/.
    let none = folder.file("resources.db.patch_21", &archive_of(&[("x/y", "x")]));
    assert!(
        build(&none, &[])
            .unwrap_err()
            .to_string()
            .contains("no gameplay/")
    );
}

#[test]
fn a_provenance_reads_back_and_a_wrong_one_is_refused() {
    let sha = "ab".repeat(32);
    let text = format!(
        "{FORMAT}\ndb resources.db 10 {sha}\n\
         patch resources.db.patch_3 5 {sha} http://h/p3\n\
         patch resources.db.patch_14 6 {sha} https://h/p?x=14\n"
    );
    let provenance = Provenance::parse(&text).unwrap();
    assert_eq!(provenance.text(), text);
    assert_eq!(provenance.patches[1].key(), "patch_14");
    assert_eq!(provenance.patches[1].urls, ["https://h/p?x=14"]);
    for wrong in [
        String::new(),
        "skhid server game data 1\nplan none\n".into(),
        // Data 2: its projects are empty.
        format!("skhid server game data 2\ndb resources.db 10 {sha}\n"),
        format!("{FORMAT}\n"),
        format!("{FORMAT}\ndb resources.db 10 {}\n", "AB".repeat(32)),
        format!("{FORMAT}\ndb resources.db ten {sha}\n"),
        // A size as no build writes it.
        format!("{FORMAT}\ndb resources.db +10 {sha}\n"),
        format!("{FORMAT}\ndb resources.db 010 {sha}\n"),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 +5 {sha} http://h/p\n"
        ),
        format!("{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha}\n"),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_03 5 {sha} http://h/p\n"
        ),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_4 5 {sha} http://h/a\n\
             patch resources.db.patch_3 5 {sha} http://h/b\n"
        ),
        format!("{FORMAT}\ndb resources.db 10 {sha}\nextra\n"),
    ] {
        assert!(Provenance::parse(&wrong).is_err(), "{wrong:?}");
    }
    // An archive that is not server data.
    let plain = archive_of(&[("gameplay/x", "x")]);
    assert!(read(&mut Archive::new(Cursor::new(plain)).unwrap()).is_err());
}

/// A patch with mirrors: its URLs, in their order, in the provenance and
/// in what is offered; one URL is written as before mirrors.
#[test]
fn a_patch_may_have_mirrors_in_its_order() {
    let folder = Folder::new("mirrors");
    let (db, two, fourteen) = game(&folder);
    let mut mirrored = patch(&fourteen);
    mirrored.urls.push(
        "https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t".into(),
    );
    mirrored
        .urls
        .push("http://198.51.100.7:8090/resources.db.patch_14".into());
    let built = build(&db, &[patch(&two), mirrored.clone()]).unwrap();
    assert_eq!(built.provenance.patches[1].urls, mirrored.urls);
    let text = built.provenance.text();
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[2].ends_with(" http://203.0.113.5/files/resources.db.patch_2"));
    assert!(
        lines[3].ends_with(
            " http://203.0.113.5/files/resources.db.patch_14 \
             https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t \
             http://198.51.100.7:8090/resources.db.patch_14"
        ),
        "{}",
        lines[3]
    );
    let data = read(&mut Archive::new(Cursor::new(built.bytes.clone())).unwrap()).unwrap();
    assert_eq!(data.provenance, built.provenance);
    let offered: Vec<(String, &str, &[String])> = data.provenance.offered().collect();
    assert_eq!(offered[1].2, mirrored.urls.as_slice());
    // Mirrors change the provenance, not the files.
    let plain = build(&db, &[patch(&two), patch(&fourteen)]).unwrap();
    let without = |bytes: &[u8]| {
        let mut files = all_files(bytes);
        files.remove(PROVENANCE);
        files
    };
    assert_eq!(without(&built.bytes), without(&plain.bytes));
    assert_ne!(built.bytes, plain.bytes);
    // Up to eight.
    let mut eight = patch(&fourteen);
    eight.urls = (1..=8)
        .map(|n| format!("http://m{n}.example/p14"))
        .collect();
    assert_eq!(
        build(&db, &[eight.clone()]).unwrap().provenance.patches[0].urls,
        eight.urls
    );
    eight.urls.push("http://m9.example/p14".into());
    let message = build(&db, &[eight]).unwrap_err().to_string();
    assert!(
        message.contains("9 URLs; the players' updater takes at most 8"),
        "{message}"
    );
}

#[test]
fn a_patch_needs_a_url_and_none_twice() {
    let folder = Folder::new("mirrors-refused");
    let (db, _, fourteen) = game(&folder);
    for (urls, why) in [
        (vec![], "no URL"),
        (
            vec!["http://h/p14".to_owned(), "http://h/p14".to_owned()],
            "http://h/p14 is given twice (URL 2 of 2)",
        ),
        (
            vec!["http://h/p14".to_owned(), "ftp://h/p14".to_owned()],
            "\"ftp://h/p14\" is not a URL the players' updater takes",
        ),
    ] {
        let bad = PatchFile {
            path: fourteen.clone(),
            urls,
        };
        let message = build(&db, &[bad]).unwrap_err().to_string();
        assert!(message.contains(why), "{message}");
    }
}

#[test]
fn a_provenance_reads_its_mirrors_and_refuses_wrong_ones() {
    let sha = "ab".repeat(32);
    let text = format!(
        "{FORMAT}\ndb resources.db 10 {sha}\n\
         patch resources.db.patch_3 5 {sha} http://h/p3 https://m/p3?x=1\n"
    );
    let provenance = Provenance::parse(&text).unwrap();
    assert_eq!(
        provenance.patches[0].urls,
        ["http://h/p3", "https://m/p3?x=1"]
    );
    assert_eq!(provenance.text(), text);
    let nine: Vec<String> = (1..=9).map(|n| format!("http://m{n}/p")).collect();
    for wrong in [
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} http://h/p http://h/p\n"
        ),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} http://h/p ftp://h/p\n"
        ),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} http://h/p  http://m/p\n"
        ),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} http://h/p \n"
        ),
        format!(
            "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} {}\n",
            nine.join(" ")
        ),
    ] {
        assert!(Provenance::parse(&wrong).is_err(), "{wrong:?}");
    }
    let eight = format!(
        "{FORMAT}\ndb resources.db 10 {sha}\npatch resources.db.patch_3 5 {sha} {}\n",
        nine[..8].join(" ")
    );
    assert_eq!(
        Provenance::parse(&eight).unwrap().patches[0].urls,
        nine[..8]
    );
}

#[test]
fn a_base_url_gets_the_file_name() {
    assert_eq!(
        url_in("http://203.0.113.5", "resources.db.patch_14"),
        "http://203.0.113.5/resources.db.patch_14"
    );
    assert_eq!(
        url_in("http://203.0.113.5/p/", "resources.db.patch_14"),
        "http://203.0.113.5/p/resources.db.patch_14"
    );
    assert!(is_url("http://203.0.113.5/p/resources.db.patch_14"));
}

/// The cases of the players' updater (client/updater/logic/src/url.rs,
/// `patch`): each URL, with whether it takes it.
fn updater_patch_urls() -> Vec<(String, bool)> {
    let cases: Vec<(String, bool)> =
        include_str!("../../../../../client/updater/logic/tests/data/patch_urls.txt")
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (verdict, url) = line.split_once(' ').unwrap_or((line, ""));
                let taken = match verdict {
                    "ok" => true,
                    "bad" => false,
                    _ => panic!("patch_urls.txt: {line:?}"),
                };
                (expand(url), taken)
            })
            .collect();
    assert!(cases.iter().filter(|case| case.1).count() >= 10);
    assert!(cases.iter().filter(|case| !case.1).count() >= 40);
    cases
}

/// A URL of `patch_urls.txt` with its `{SP}`, ..., `{a*N}` written out.
fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('{') {
        out.push_str(&rest[..at]);
        let end = at + rest[at..].find('}').expect(text);
        match &rest[at + 1..end] {
            "SP" => out.push(' '),
            "TAB" => out.push('\t'),
            "CR" => out.push('\r'),
            "LF" => out.push('\n'),
            "NUL" => out.push('\0'),
            "DEL" => out.push('\x7f'),
            token => {
                let n: usize = token
                    .strip_prefix("a*")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or_else(|| panic!("patch_urls.txt: {text:?}"));
                out.extend(std::iter::repeat_n('a', n));
            }
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[test]
fn a_url_is_taken_when_the_players_updater_takes_it() {
    for (url, taken) in updater_patch_urls() {
        assert_eq!(is_url(&url), taken, "{url:?}");
    }
}

/// A patch whose URL the updater would refuse fails the build, before any
/// archive is read.
#[test]
fn a_build_refuses_a_url_the_updater_refuses() {
    let folder = Folder::new("bad-url");
    let db = folder.file(RESOURCES_DB, &archive_of(&[("gameplay/x", "x")]));
    let patch = folder.file("resources.db.patch_14", &archive_of(&[("gameplay/x", "y")]));
    for (url, taken) in updater_patch_urls() {
        let patches = [PatchFile {
            path: patch.clone(),
            urls: vec![url.clone()],
        }];
        match build(&db, &patches) {
            Ok(built) => {
                assert!(taken, "{url:?}");
                assert_eq!(built.provenance.patches[0].urls, [url]);
            }
            Err(error) => {
                assert!(!taken, "{url:?}: {error}");
                assert!(error.to_string().contains("not a URL"), "{error}");
            }
        }
    }
}

/// The server data of 0.26g's archive with patches over it:
/// `SRP_TEST_RESOURCES_DB=<…/game/resources.db>`, `SRP_TEST_PATCHES` the
/// patches' files, by spaces (`resources.db.patch_14 _15 _16`: the menus,
/// the Bandit balaclava, the SVD's damage), and `SRP_TEST_PATCH_URL` their
/// folder on the patches server.
#[test]
#[ignore = "needs 0.26g's resources.db and its patches"]
fn the_server_data_of_0_26g_with_its_patches() {
    let var = |name: &str| std::env::var(name).expect(name);
    let db = PathBuf::from(var("SRP_TEST_RESOURCES_DB"));
    let base = var("SRP_TEST_PATCH_URL");
    let patches: Vec<PatchFile> = var("SRP_TEST_PATCHES")
        .split_whitespace()
        .map(|path| {
            let path = PathBuf::from(path);
            let url = url_in(&base, path.file_name().unwrap().to_str().unwrap());
            PatchFile {
                path,
                urls: vec![url],
            }
        })
        .collect();
    let started = std::time::Instant::now();
    let built = build(&db, &patches).unwrap();
    eprintln!(
        "{} bytes in {:?}, {} gameplay files, {} projects, {:?}",
        built.bytes.len(),
        started.elapsed(),
        built.gameplay,
        built.projects,
        built.given
    );
    eprint!("{}", built.provenance.text());
    assert_eq!(built.provenance.db.size, 6_285_533_906);
    assert!(built.gameplay >= 508 && built.projects > 70);
    // Each archive's files srp reads, one over the other in the game's
    // order, read again here without the overlay.
    let mut expected = BTreeMap::new();
    let mut ordered: Vec<&Path> = patches.iter().map(|patch| patch.path.as_path()).collect();
    ordered.sort_by_key(|path| patch_number(path.file_name().unwrap().to_str().unwrap()));
    for path in std::iter::once(db.as_path()).chain(ordered) {
        let mut archive = Archive::new(File::open(path).unwrap()).unwrap();
        let read: Vec<String> = archive
            .files_under("")
            .filter(|(path, _)| is_read(path))
            .map(|(path, _)| path.to_owned())
            .collect();
        for file in read {
            let bytes = archive.read(&file).unwrap().unwrap();
            let kept = if file.starts_with(PROJECTS) {
                // Its server objects as the game has them.
                let kept = server_objects_of(&bytes).unwrap();
                let kept = binary_config_parser::ConfigValue::parse(&kept).unwrap();
                let whole = binary_config_parser::ConfigValue::parse(&bytes).unwrap();
                assert_eq!(
                    kept.get(SERVER_OBJECTS),
                    whole.get(SERVER_OBJECTS),
                    "{file}"
                );
                server_objects_of(&bytes).unwrap()
            } else {
                bytes
            };
            expected.insert(file, kept);
        }
    }
    let mut files = all_files(&built.bytes);
    assert_eq!(
        files.remove(PROVENANCE).unwrap(),
        built.provenance.text().into_bytes()
    );
    assert_eq!(files, expected);
    assert_eq!(build(&db, &patches).unwrap().bytes, built.bytes);
    if let Ok(out) = std::env::var("SRP_TEST_GAME_DATA_OUT") {
        std::fs::write(out, &built.bytes).unwrap();
    }
}
