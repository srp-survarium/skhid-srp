//! Check every weather entry through the server's actual playable-map parser.
use std::path::PathBuf;
use survarium::v026g::{
    dictionary::MapId,
    game_data,
    maps::{GameMode, MatchMap},
};

fn main() {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("server-game-data.db"));
    game_data::init_from(&path).expect("server must accept the composed data");
    let cases: [(u8, u8, [u16; 4]); 5] = [
        (1, 29, [262, 263, 264, 265]),
        (2, 25, [1, 2, 3, 4]),
        (3, 10, [170, 171, 172, 173]),
        (5, 26, [5, 6, 7, 8]),
        (6, 11, [179, 180, 253, 254]),
    ];
    for (source_level, level, ids) in cases {
        for (index, id) in ids.into_iter().enumerate() {
            let map = MatchMap::of(MapId::new(id))
                .expect("weather map must be playable")
                .map();
            let weather = if index < 2 { "rain" } else { "evening" };
            let (mode, rules) = if index % 2 == 0 {
                (2, GameMode::Batteries)
            } else {
                (4, GameMode::TeamDeathmatch)
            };
            let project = format!("v021d_level_{source_level:02}/{weather}_{mode}");
            assert_eq!(map.id.get(), id);
            assert_eq!(map.level, level);
            assert_eq!(map.project, project);
            assert_eq!(map.mode, rules);
            println!("{id}: {project}, level {level}, playable without map-ID truncation");
        }
    }
    println!("All 20 weather entries resolve through MatchMap::of(MapId) in composed server data");
}
