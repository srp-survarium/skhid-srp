use super::*;

#[test]
fn the_test_archive_is_0_26g_with_the_test_patches() {
    let data = shared();
    assert_eq!(data.origin, TEST_ORIGIN);
    // 0.26g's 508, the Bandit balaclava's config of patch 15, the English
    // and Ukrainian localizations, and srp's own configs (SRP_TABLES).
    assert_eq!(data.len(), 511 + SRP_TABLES.len());
    assert!(data.file("localization/ukrainian/localization").is_some());
    assert!(
        data.file("gameplay/items/armour/mask/bandit_026.options")
            .is_some()
    );
    assert!(data.file("gameplay/missing").is_none());
    // The projects are there, their server objects; level_08 at night is
    // not.
    assert!(data.has_project("level_01/day_2"));
    assert!(!data.has_project("level_08/night_2"));
    // It names what it was made of: 0.26g's archive and patches 14, 15
    // and 16, in the game's order, each with the URL the players fetch it
    // from.
    let provenance = &data.provenance;
    assert_eq!(provenance.db.name, "resources.db");
    assert_eq!(provenance.db.size, 6_285_533_906);
    let offered: Vec<(String, &str, &[String])> = provenance.offered().collect();
    let keys: Vec<&str> = offered.iter().map(|(key, _, _)| key.as_str()).collect();
    assert_eq!(keys, ["patch_14", "patch_15", "patch_16"]);
    assert_eq!(
        offered[2].1,
        "add87569ed39cfe693ac02e7f4a7aaaea89e003ce7715be0cb972a99c036d093"
    );
    assert_eq!(
        offered[0].2,
        ["http://203.0.113.5/patches/resources.db.patch_14"]
    );
    assert!(
        data.made_of()
            .starts_with("resources.db c74078be702d + patch_14 7535d0d0c287")
    );
    assert!(super::super::tables::check(data).is_ok());
}

struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("srp-game-data-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An archive of `files`, as the game's.
fn archive_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let files: BTreeMap<String, Vec<u8>> = files
        .iter()
        .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
        .collect();
    resources_db::write::write(&files).unwrap()
}

#[test]
fn srp_needs_readable_server_game_data() {
    // SAFETY: no other test of this crate reads this variable.
    unsafe {
        std::env::remove_var(GAME_DATA_ENV);
    }
    let message = from_env().unwrap_err().to_string();
    assert!(message.contains(GAME_DATA_ENV), "{message}");
    assert!(message.contains("srp-game-data build"), "{message}");

    let folder = Folder::new("open");
    let data = folder.0.join("server-game-data.db");
    // No such file.
    let message = GameData::open(&data).unwrap_err().to_string();
    assert!(message.contains(&data.display().to_string()), "{message}");
    assert!(message.contains("srp-game-data build"), "{message}");

    // The test data, as srp reads it.
    std::fs::write(&data, TEST_ARCHIVE).unwrap();
    let opened = GameData::open(&data).unwrap();
    // The test data has srp's own configs over the archive's (SRP_TABLES).
    let mut files = opened.files.clone();
    files.extend(
        SRP_TABLES
            .iter()
            .map(|(path, table)| ((*path).to_owned(), key_value_config(table))),
    );
    assert_eq!(files, shared().files);
    assert_eq!(opened.projects, shared().projects);
    assert_eq!(opened.provenance, shared().provenance);
    // SAFETY: as above.
    unsafe {
        std::env::set_var(GAME_DATA_ENV, &data);
    }
    let from_env = from_env();
    unsafe {
        std::env::remove_var(GAME_DATA_ENV);
    }
    assert_eq!(from_env.unwrap().provenance, shared().provenance);

    // Not an archive at all, or cut short.
    for bytes in [
        &b"little-endian"[..],
        &TEST_ARCHIVE[..TEST_ARCHIVE.len() / 2],
    ] {
        std::fs::write(&data, bytes).unwrap();
        let message = GameData::open(&data).unwrap_err().to_string();
        assert!(message.contains(&data.display().to_string()), "{message}");
    }
    // A byte of a file changed: its table's CRC-32 says so.
    let archive = Archive::new(Cursor::new(TEST_ARCHIVE)).unwrap();
    let (_, entry) = archive
        .files_under("gameplay/items/weapons/")
        .find(|(_, entry)| entry.size > 0)
        .unwrap();
    let mut damaged = TEST_ARCHIVE.to_vec();
    damaged[entry.offset as usize] ^= 0xFF;
    std::fs::write(&data, &damaged).unwrap();
    let message = GameData::open(&data).unwrap_err().to_string();
    assert!(message.contains("not those its table says"), "{message}");
    // An archive of the game's that is not the server's data: no
    // srp/provenance, or one srp-game-data does not write.
    std::fs::write(
        &data,
        archive_of(&[("gameplay/db_static_dictionaries", b"x")]),
    )
    .unwrap();
    let message = GameData::open(&data).unwrap_err().to_string();
    assert!(message.contains("srp/provenance"), "{message}");
    std::fs::write(
        &data,
        archive_of(&[
            ("gameplay/db_static_dictionaries", b"x"),
            ("srp/provenance", b"skhid server game data 1\nplan none\n"),
        ]),
    )
    .unwrap();
    let message = GameData::open(&data).unwrap_err().to_string();
    assert!(message.contains(resources_db::server::FORMAT), "{message}");
}

#[test]
fn data_whose_tables_do_not_read_is_refused() {
    // Server data of the right form, but its dictionary is not one: the
    // tables' check stops srp at start.
    let provenance = format!(
        "{}\ndb resources.db 1 {}\n",
        resources_db::server::FORMAT,
        "0".repeat(64)
    );
    let bytes = archive_of(&[
        ("gameplay/db_static_dictionaries", b"not a config"),
        ("srp/provenance", provenance.as_bytes()),
    ]);
    let mut archive = Archive::new(Cursor::new(bytes)).unwrap();
    let data = GameData::from_archive(&mut archive, "broken".into()).unwrap();
    assert!(data.provenance.patches.is_empty());
    assert_eq!(data.provenance.offered().count(), 0);
    let message = super::super::tables::check(&data).unwrap_err().to_string();
    assert!(message.contains("db_static_dictionaries"), "{message}");
}

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name}")))
}

/// The test data is `srp-game-data build` of 0.26g's archive
/// (`SRP_TEST_RESOURCES_DB`) with the test patches (`SRP_TEST_PATCHES`,
/// separated by spaces) at `http://203.0.113.5/patches`, byte for byte; and
/// 0.26g's own archive gives the test data's tables without the patches'
/// item, the Bandit balaclava (1810).
#[test]
#[ignore = "needs 0.26g's resources.db and the test patches"]
fn the_test_data_is_0_26g_with_the_test_patches() {
    use super::super::dictionary::{self, Dictionary};
    use super::super::{item_params, maps, modifications};
    use resources_db::server::{PatchFile, build, url_in};
    let db = env_path("SRP_TEST_RESOURCES_DB");
    let patches: Vec<PatchFile> = std::env::var("SRP_TEST_PATCHES")
        .expect("SRP_TEST_PATCHES")
        .split_whitespace()
        .map(|path| {
            let path = PathBuf::from(path);
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            PatchFile {
                urls: vec![url_in("http://203.0.113.5/patches", &name)],
                path,
            }
        })
        .collect();
    let built = build(&db, &patches, None).unwrap();
    assert!(
        built.bytes == TEST_ARCHIVE,
        "the test data is not that of 0.26g with these patches"
    );

    let stock = build(&db, &[], None).unwrap();
    let mut archive = Archive::new(Cursor::new(stock.bytes)).unwrap();
    let stock = GameData::from_archive(&mut archive, "0.26g".into()).unwrap();
    super::super::tables::check(&stock).unwrap();
    let patched = shared();
    let changed = [
        dictionary::PATH,
        "gameplay/items/armour/mask/bandit_026.options",
        "gameplay/items/weapons/wpn_svd.options",
        // Patch 15 has its texts.
        "localization/english/localization",
        "localization/ukrainian/localization",
    ];
    assert_eq!(stock.len() + 1, patched.len());
    for (path, bytes) in &patched.files {
        if !changed.contains(&path.as_str()) {
            assert_eq!(stock.file(path), Some(bytes.as_slice()), "{path}");
        }
    }
    assert_eq!(stock.projects, patched.projects);
    let new = |id: u16| id == 1810;
    let stock_dictionary = Dictionary::parse(stock.file(dictionary::PATH).unwrap()).unwrap();
    let mut items = Dictionary::shared().items.clone();
    items.retain(|item| !new(item.dict_id));
    assert_eq!(stock_dictionary.items, items);
    let svd = Dictionary::shared().item_named("svd").unwrap().dict_id;
    let mut params = item_params::all().to_vec();
    params.retain(|row| !new(row.dict_id) && row.dict_id != svd);
    let mut stock_params = item_params::derive(&stock, &stock_dictionary).unwrap();
    stock_params.retain(|row| row.dict_id != svd);
    assert_eq!(stock_params, params);
    let mut mods = modifications::all_item_mods().to_vec();
    mods.retain(|row| !new(row.dict_id));
    assert_eq!(
        modifications::derive_item_mods(&stock, &stock_dictionary).unwrap(),
        mods
    );
    assert_eq!(
        maps::derive(&stock, &stock_dictionary).unwrap(),
        maps::all()
    );
}
