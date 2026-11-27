//! Byte-exact regression vectors for the local-match writers. The vectors hold only our inputs (cases, test humans,
//! rosters) and, for the produced bytes, their length and FNV-1a 64 hash (the bytes carry game data: they are built
//! from the locally extracted tables, `SRP_TEST_DATA`). The tutorial header is our own wire header and stays as bytes.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use game_data::GameData;
use local_match::{HumanLoadout, HumanProgress, Roster, RosterBot, TrainingParams};
use serde_json::Value;

fn gd() -> &'static GameData {
    static G: OnceLock<GameData> = OnceLock::new();
    G.get_or_init(|| game_data::testing::tables())
}

/// The data without weapon default modules, kit modules and module modifier pairs.
fn gd_no_default_modules() -> &'static GameData {
    static G: OnceLock<GameData> = OnceLock::new();
    G.get_or_init(|| {
        let mut g = game_data::testing::tables();
        g.modules.clear_default_modules();
        g.modules.clear_fixed_mods();
        g.matches.clear_kit_modules();
        g
    })
}

fn u(v: &Value) -> u64 {
    v.as_u64().unwrap_or_else(|| panic!("not a u64: {v}"))
}

fn human(v: &Value) -> HumanLoadout {
    let skills: BTreeMap<String, u32> = v["skills"].as_object().unwrap().iter().map(|(k, x)| (k.clone(), u(x) as u32)).collect();
    HumanLoadout {
        name: v["name"].as_str().unwrap().into(),
        clan: v["clan"].as_str().unwrap().into(),
        account_id: v["account_id"].as_array().unwrap().iter().map(|x| u(x) as u8).collect::<Vec<_>>().try_into().unwrap(),
        items: v["items"].as_array().unwrap().iter().map(|i| (u(&i[0]) as u32, u(&i[1]) as u16, u(&i[2]) as u32)).collect(),
        slots: v["slots"].as_object().unwrap().iter().map(|(k, x)| (k.parse().unwrap(), u(x) as u32)).collect(),
        slot_rounds: v["slot_rounds"].as_object().unwrap().iter().map(|(k, x)| (k.parse().unwrap(), u(x) as u32)).collect(),
        skills,
        upgrades: v["upgrades"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, l)| (k.parse().unwrap(), l.as_array().unwrap().iter().map(|p| (u(&p[0]) as u16, u(&p[1]) as u16)).collect()))
            .collect(),
        attach: v["attach"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (u(&e[0]) as u32, e[1].as_array().unwrap().iter().map(|p| (p[0].as_str().unwrap().to_string(), u(&p[1]) as u32)).collect()))
            .collect(),
        removed_defaults: BTreeMap::new(),
        bound: v["bound"].as_object().unwrap().iter().map(|(k, x)| (k.parse().unwrap(), u(x) as u16)).collect(),
        extra_modifiers: BTreeMap::new(),
        decals: BTreeMap::new(),
        games_played: 0,
        profiles: vec![],
        face: 0,
        premium: false,
    }
}

fn roster(v: &Value) -> Roster {
    Roster {
        match_id: u(&v["match_id"]) as u32,
        map_id: u(&v["map_id"]) as u32,
        mode_id: u(&v["mode_id"]) as u32,
        human_name: v["human_name"].as_str().unwrap().into(),
        bots: v["bots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| RosterBot {
                k: u(&b["k"]) as u32,
                team: u(&b["team"]) as u8,
                ai: u(&b["ai"]) as u32,
                first: u(&b["first"]) as u32,
                second: u(&b["second"]) as u32,
                name: b["name"].as_str().unwrap().into(),
            })
            .collect(),
    }
}

/// FNV-1a 64 of the bytes, as 16 hex digits (the vectors' `*_fnv` fields).
fn fnv(b: &[u8]) -> String {
    format!("{:016x}", b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x0000_0100_0000_01b3)))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn training_options_equal_the_vectors() {
    game_data::require_tables!();
    let v: Value = serde_json::from_str(include_str!("vectors/options.json")).unwrap();
    let mut grew = 0;
    for (n, c) in v["training"].as_array().unwrap().iter().enumerate() {
        let k = &c["case"];
        let mut p = TrainingParams::new(u(&k["map"]) as u32, u(&k["mode"]) as u32, u(&k["weather"]) as u32, u(&k["match_id"]) as u32);
        p.n_players = k.get("n_players").map(|x| u(x) as usize);
        if let Some(l) = k.get("bot_level") {
            p.bot_level = u(l) as u32;
        }
        let (opts, ros) = local_match::training_options(gd(), &p, &human(&c["human"])).unwrap();
        assert_eq!((opts.len() as u64, fnv(&opts)), (u(&c["options_len"]), c["options_fnv"].as_str().unwrap().to_string()), "case {n}: options bytes");
        assert_eq!(ros, roster(&c["roster"]), "case {n} roster");
        // with the default modules every bot / human weapon that has some carries them: the blob grows
        let (bare, _) = local_match::training_options(gd_no_default_modules(), &p, &human(&c["human"])).unwrap();
        assert!(opts.len() >= bare.len(), "case {n}: default modules only add bytes");
        grew += usize::from(opts.len() > bare.len());
    }
    assert!(grew > 0, "some case has a weapon with default modules");
}

/// The tutorial: the four-kit human and the nine script bots (options.rs
/// `tutorial_has_four_kit_profiles_and_the_script_bots`); here the header and the level strings.
#[test]
fn tutorial_options_header() {
    game_data::require_tables!();
    let v: Value = serde_json::from_str(include_str!("vectors/options.json")).unwrap();
    for (n, c) in v["tutorial"].as_array().unwrap().iter().enumerate() {
        let opts = local_match::tutorial_options(gd_no_default_modules(), &human(&c["human"]), u(&c["match_id"]) as u32).unwrap();
        // header (16 bytes, n_players at 5), then the project and map strings
        let strings = 16 + 1 + "tutorial".len() + 1 + "tutorial/default".len();
        assert_eq!(opts[5], 10, "tutorial {n}: players");
        assert_eq!(hex(&opts[..strings]), c["header"].as_str().unwrap(), "tutorial {n}");
    }
}

#[test]
fn pack_5b_roundtrip() {
    let opts: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let p = local_match::pack_5b(2, &opts);
    assert_eq!(p[0], 2);
    let n = u32::from_le_bytes(p[1..5].try_into().unwrap()) as usize;
    assert_eq!(n, p.len() - 5);
    assert_eq!(protocol::lzma2::decompress(&p[5..]).unwrap(), opts);
}

#[test]
fn results_equal_the_vectors() {
    game_data::require_tables!();
    let v: Value = serde_json::from_str(include_str!("vectors/results.json")).unwrap();
    for (n, c) in v.as_array().unwrap().iter().enumerate() {
        let ros = roster(&c["roster"]);
        let ev: Vec<u16> = c["events"].as_array().unwrap().iter().map(|x| u(x) as u16).collect();
        let prog = HumanProgress { xp: u(&c["xp"]) as u32, faction_points: u(&c["fac_points"]) as u32, faction: u(&c["faction"]) as u8, first_game: false };
        let (msg, rw) = local_match::results(gd(), &ros, &ev, &prog, ros.match_id).unwrap();
        assert_eq!((msg.len() as u64, fnv(&msg)), (u(&c["msg_len"]), c["msg_fnv"].as_str().unwrap().to_string()), "results {n}: message bytes");
        // the header carries the u8 team scores and the match level
        let (t0, t1) = (u(&c["t0"]) as u32, u(&c["t1"]) as u32);
        assert_eq!((msg[13], msg[14]), (t0.min(255) as u8, t1.min(255) as u8), "results {n}: scores");
        assert_eq!(msg[34], game_data::rewards::LOCAL_MATCH_LEVEL as u8, "results {n}: match level");
        // the bots mode awards nothing (client hint st_match_settings_bots_hint)
        assert_eq!((rw.exp, rw.rep, rw.silver), (0, 0, 0), "rewards {n}");
        assert_eq!((rw.team0_score as u64, rw.team1_score as u64), (t0 as u64, t1 as u64), "scores {n}");
        let names = rw.people.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join("\n");
        assert_eq!(fnv(names.as_bytes()), c["names_fnv"].as_str().unwrap(), "results {n}: names");
        for (p, w) in rw.people.iter().zip(c["people"].as_array().unwrap()) {
            assert_eq!((p.kills as u64, p.deaths as u64, p.score as u64, p.team as u64, p.pid), (u(&w["kills"]), u(&w["deaths"]), u(&w["score"]), u(&w["team"]), u(&w["pid"])));
        }
        assert_eq!(rw.people.len(), c["people"].as_array().unwrap().len());
    }
}
