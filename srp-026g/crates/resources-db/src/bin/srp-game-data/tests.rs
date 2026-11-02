// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

fn args(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn parsed(text: &str) -> Result<(PathBuf, Vec<PatchFile>, Option<PathBuf>), String> {
    archives(&args(text)).map(|archives| (archives.db, archives.patches, archives.out))
}

fn refused(text: &str) -> String {
    parsed(text).expect_err(text)
}

/// A game folder of its own under the system's temporary one, removed
/// after.
struct Game(PathBuf);

impl Game {
    fn new(name: &str, files: &[&str]) -> Self {
        let path =
            std::env::temp_dir().join(format!("srp-game-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        for file in files {
            std::fs::write(path.join(file), b"x").unwrap();
        }
        Self(path)
    }
}

impl Drop for Game {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn db_and_patches_are_named_with_their_urls() {
    let (db, patches, out) = parsed(
        "--db r/resources.db --patch a/resources.db.patch_2=http://h/x=y \
         --patch b/resources.db.patch_1 --patch-url http://h/base/ --out o",
    )
    .unwrap();
    assert_eq!(db, PathBuf::from("r/resources.db"));
    assert_eq!(out, Some(PathBuf::from("o")));
    let given: Vec<(&str, String)> = patches
        .iter()
        .map(|patch| (patch.path.to_str().unwrap(), patch.urls.join(" ")))
        .collect();
    // Split at the first `=`; the base URL for a patch without one; the
    // order as given (build puts them in the game's).
    assert_eq!(
        given,
        [
            ("a/resources.db.patch_2", "http://h/x=y".to_owned()),
            (
                "b/resources.db.patch_1",
                "http://h/base/resources.db.patch_1".to_owned()
            ),
        ]
    );
    let (_, patches, out) = parsed("--db resources.db").unwrap();
    assert!(patches.is_empty());
    assert_eq!(out, None);
}

/// Mirrors: `<file>=<URL> <URL>...` as one argument, the URLs one space
/// apart, in their order.
#[test]
fn a_patch_takes_its_urls_one_space_apart() {
    let options: Vec<String> = [
        "--db",
        "resources.db",
        "--patch",
        "p/resources.db.patch_12=http://h/p12 https://drive.usercontent.google.com/download?id=a&export=download&confirm=t",
        "--patch",
        "p/resources.db.patch_14=http://h/p14",
    ]
    .map(str::to_owned)
    .to_vec();
    let patches = archives(&options).unwrap().patches;
    assert_eq!(
        patches[0].urls,
        [
            "http://h/p12",
            "https://drive.usercontent.google.com/download?id=a&export=download&confirm=t"
        ]
    );
    assert_eq!(patches[1].urls, ["http://h/p14"]);
    // Two spaces, a space at an end: an empty URL, which build refuses.
    for bad in ["http://h/a  http://h/b", "http://h/a ", " http://h/a"] {
        let options: Vec<String> = ["--db", "resources.db", "--patch"]
            .map(str::to_owned)
            .into_iter()
            .chain([format!("p/resources.db.patch_12={bad}")])
            .collect();
        let patches = archives(&options).unwrap().patches;
        assert!(
            resources_db::server::check_urls(&patches[0].urls)
                .unwrap_err()
                .contains("\"\" is not a URL"),
            "{bad:?}"
        );
    }
}

/// The game's archive: the row of the release's list as one argument.
#[test]
fn the_games_archive_is_a_row_of_the_list() {
    let sha = "ab".repeat(32);
    let options: Vec<String> = [
        "--db".into(),
        "resources.db".into(),
        "--game".into(),
        format!("survarium-026g.7z {sha} 2906544103 http://h/g https://m.example/g"),
    ]
    .to_vec();
    let game = archives(&options).unwrap().game.unwrap();
    assert_eq!(
        (
            game.input.name.as_str(),
            game.input.size,
            game.urls.join(" ")
        ),
        (
            "survarium-026g.7z",
            2_906_544_103,
            "http://h/g https://m.example/g".into()
        )
    );
    assert_eq!(archives(&args("--db resources.db")).unwrap().game, None);
    for bad in [
        format!("survarium-026g.7z {sha} 2906544103"),
        format!("survarium-026g.7z {sha} big http://h/g"),
    ] {
        let options = ["--db".into(), "resources.db".into(), "--game".into(), bad];
        assert!(archives(&options).unwrap_err().starts_with("--game: "));
    }
    let twice = ["--game", "g", "--game", "g", "--db", "resources.db"].map(str::to_owned);
    assert_eq!(archives(&twice).unwrap_err(), "--game is given twice");
}

#[test]
fn a_game_folder_gives_its_db_and_patches() {
    let game = Game::new(
        "dir",
        &[
            "resources.db",
            "resources.db.patch_3",
            "resources.db.patch_1",
            "resources.db.patch_03",
        ],
    );
    let dir = game.0.display().to_string();
    let (db, patches, _) = parsed(&format!("--dir {dir} --patch-url http://h/p")).unwrap();
    assert_eq!(db, game.0.join("resources.db"));
    let urls: Vec<&[String]> = patches.iter().map(|patch| patch.urls.as_slice()).collect();
    assert_eq!(
        urls,
        [
            ["http://h/p/resources.db.patch_1".to_owned()],
            ["http://h/p/resources.db.patch_3".to_owned()]
        ]
    );
    // Every patch of the folder needs a URL.
    assert!(refused(&format!("--dir {dir}")).contains("no URL"));
    // A folder without resources.db.
    let empty = Game::new("empty", &[]);
    assert!(
        refused(&format!(
            "--dir {} --patch-url http://h/p",
            empty.0.display()
        ))
        .contains("no such file")
    );
}

#[test]
fn options_that_do_not_say_one_set_of_archives_are_refused() {
    for (options, why) in [
        ("", "either --dir"),
        ("--patch-url http://h/p", "either --dir"),
        ("--dir d --db resources.db", "either --dir"),
        (
            "--dir d --patch resources.db.patch_1=http://h/x",
            "either --dir",
        ),
        ("--db a --db b", "given twice"),
        ("--dir a --dir b", "given twice"),
        (
            "--db a --patch-url http://h/ --patch-url http://h/",
            "given twice",
        ),
        ("--db a --out o --out p", "given twice"),
        ("--db", "needs a value"),
        ("--db a --patch", "needs a value"),
        ("--db a --other b", "unknown option --other"),
        ("--db a b", "unknown option b"),
        ("--db a --patch resources.db.patch_1", "no URL"),
    ] {
        let message = refused(options);
        assert!(message.contains(why), "{options:?}: {message}");
    }
}

/// `<layers>`: the patches in the game's order whatever their order given,
/// the options a command takes, the other words as paths.
#[test]
fn layers_are_the_db_and_the_patches_in_ascending_n() {
    let (archives, rest) = layered(
        &args("--db r/resources.db --patch b/resources.db.patch_14 x/y --patch a/resources.db.patch_2 --dir d"),
        &["--dir"],
    )
    .unwrap();
    assert_eq!(
        archives,
        [
            PathBuf::from("r/resources.db"),
            PathBuf::from("a/resources.db.patch_2"),
            PathBuf::from("b/resources.db.patch_14"),
        ]
    );
    assert_eq!(rest.named.get("--dir").map(String::as_str), Some("d"));
    assert_eq!(rest.paths, ["x/y"]);
    for (options, why) in [
        (
            "--patch resources.db.patch_1",
            "--db <resources.db> is needed",
        ),
        (
            "--db r --patch p/patch_1",
            "the game mounts a patch only by the name",
        ),
        (
            "--db r --patch a/resources.db.patch_3 --patch b/resources.db.patch_3",
            "two patches of one number",
        ),
        ("--db r --out o", "--out: not an option here"),
        (
            "--db r --dir a --dir b",
            "--dir: not an option here, or given twice",
        ),
        ("--db r --dir", "--dir needs a value"),
    ] {
        let message = layered(&args(options), &["--dir"]).expect_err(options);
        assert!(message.contains(why), "{options}: {message}");
    }
}

/// `get` and `compare` see a later patch's file over an earlier one's.
#[test]
fn get_and_compare_see_the_files_as_the_game_mounts_them() {
    let game = Game::new("layers", &[]);
    let archive = |name: &str, files: &[(&str, &str)]| {
        let dir = game.0.join(format!("{name}.files"));
        for (path, text) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let (bytes, _) = resources_db::pack::pack(&dir).unwrap();
        std::fs::write(game.0.join(name), bytes).unwrap();
        game.0.join(name).display().to_string()
    };
    let db = archive("resources.db", &[("a/x", "stock x"), ("a/y", "stock y")]);
    let patch = archive("resources.db.patch_5", &[("a/x", "patched x")]);
    let layers = format!("--db {db} --patch {patch}");
    let out = game.0.join("got");
    get(&args(&format!("{layers} --out {} a/x a/y", out.display()))).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("a/x")).unwrap(),
        "patched x"
    );
    assert_eq!(std::fs::read_to_string(out.join("a/y")).unwrap(), "stock y");
    let refused = get(&args(&format!("{layers} --out {} a/x", out.display()))).unwrap_err();
    assert!(refused.contains("never overwritten"), "{refused}");
    let refused = get(&args(&format!(
        "{layers} --out {}/other a/z",
        game.0.display()
    )))
    .unwrap_err();
    assert!(
        refused.contains("a/z: no archive has this file"),
        "{refused}"
    );

    let made = game.0.join("made");
    for (path, text) in [("a/x", "patched x"), ("a/y", "new y"), ("b/z", "z")] {
        std::fs::create_dir_all(made.join(path).parent().unwrap()).unwrap();
        std::fs::write(made.join(path), text).unwrap();
    }
    let lines = compare(&args(&format!("{layers} --dir {}", made.display()))).unwrap();
    assert_eq!(
        lines,
        [
            "same a/x resources.db.patch_5",
            "differs a/y resources.db",
            "new b/z"
        ]
    );
}
