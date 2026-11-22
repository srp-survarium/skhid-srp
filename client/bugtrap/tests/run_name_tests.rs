// Host-only: the run logs' names (src/run_name.rs).
#[path = "../src/run_name.rs"]
mod run_name;
use run_name::{Kind, parse};

fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}
fn run(name: &str) -> Option<(Kind, String)> {
    parse(&wide(name)).map(|run| (run.kind, run.stamp().to_string()))
}

#[test]
fn each_kind_and_its_stamp() {
    let stamp = "2026-11-15_14-03-07";
    assert_eq!(run(&format!("srp_patches.{stamp}.log")), Some((Kind::Patches, stamp.into())));
    assert_eq!(run(&format!("updater_skhid.{stamp}.log")), Some((Kind::Updater, stamp.into())));
    assert_eq!(run(&format!("Survarium.{stamp}.log")), Some((Kind::Game, stamp.into())));
    assert_eq!(run(&format!("Survarium_tester.{stamp}.log")), Some((Kind::Game, stamp.into())));
    // A user name with dots and letters of other languages: the stamp is the last part.
    assert_eq!(run(&format!("Survarium_john.doe.Жółw.{stamp}.log")), Some((Kind::Game, stamp.into())));
    assert_eq!(run(&format!("SRP_PATCHES.{stamp}.LOG")), Some((Kind::Patches, stamp.into())));
    assert_eq!(run(&format!("srp_patches.{stamp}_2.log")), Some((Kind::Patches, format!("{stamp}_2"))));
    assert_eq!(run(&format!("srp_patches.{stamp}_99.log")), Some((Kind::Patches, format!("{stamp}_99"))));
    assert_eq!([Kind::Game, Kind::Patches, Kind::Updater].map(Kind::word), ["game", "srp_patches", "updater_skhid"]);
}

#[test]
fn names_of_no_run() {
    let stamp = "2026-11-15_14-03-07";
    for name in [
        "srp_patches.log".to_string(),
        "srp_patches.old.log".into(),
        "updater_skhid.log".into(),
        "Survarium_tester.log".into(),
        format!("srp_patches.{stamp}.txt"),
        format!("srp_patches.{stamp}_1.log"),
        format!("srp_patches.{stamp}_0.log"),
        format!("srp_patches.{stamp}_02.log"),
        format!("srp_patches.{stamp}_100.log"),
        format!("srp_patches.{stamp}_.log"),
        format!("srp_patches.{stamp}x.log"),
        "srp_patches.2026-11-15 14-03-07.log".into(),
        "srp_patches.2026-11-15_14-03-0x.log".into(),
        "srp_patches.2026-11-15_14-03.log".into(),
        format!("srp_patches_{stamp}.log"),
        format!("updater_skhid.resources.{stamp}.log"),
        format!("SurvariumX.{stamp}.log"),
        format!("Survarium_.{stamp}.log"),
        format!("launcher.{stamp}.log"),
        format!("{stamp}.log"),
        ".log".into(),
        "".into(),
    ] {
        assert_eq!(run(&name), None, "{name}");
    }
}
