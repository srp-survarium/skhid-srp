//! The spawn marks: a tool of a local build, for picking spawn places by eye
//! in the game (docs/notes/client_patch.md section 3.82). The owner flies or
//! walks through a map of a live match and presses a key at each place; each
//! press is a line of `srp_spawn_marks.tsv` beside `srp_patches.log`, which
//! the offline spawn tools read.
//!
//! - **F7** the screen goes to the engine's free-fly camera, and back to the
//!   player's (a live match only; a replay has its own F2 and F1).
//! - **F8** marks "a spawn here", **F9** "no spawn near here": where the
//!   screen's camera is and looks, whichever camera that is.
//!
//! Each press says what it did in the hud's centre message and in
//! `srp_patches.log`; a mark's message comes only after its line is in the
//! file. A press the tool acts on is taken: no later handler is told the key
//! (the input world's keyboard goes through its handlers by pointers it read
//! before the first, 0x3B9B2A, and F7 changes that list).
//!
//! **Never in a release.** A free camera in a live match is a cheat. All of
//! the tool is compiled only with `cfg(srp_spawn_marks)`, which
//! `patch_026g.py --spawn-marks` alone passes (link_026g.py's RUST_CFGS); in
//! every other build `on_key` below is the whole patch, and the arena the
//! lock describes has no byte of the tool (the self-test's
//! test_spawn_marks). The crate's tests compile it too.

#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, SpawnMarksOnKeyExit};

/// game_world::on_keyboard_action's start (`tab_cursor.key`, after the
/// replay's keys): every key the game world gets. Without the tool, nothing.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<SpawnMarksOnKeyExit> {
    #[cfg(any(srp_spawn_marks, test))]
    if tool::on_key(ctx) {
        return Flow::Exit(SpawnMarksOnKeyExit::Taken);
    }
    #[cfg(not(any(srp_spawn_marks, test)))]
    let _ = ctx;
    Flow::Continue
}

#[cfg(any(srp_spawn_marks, test))]
mod tool {
    use crate::console_where::{Line, MOST, pitch_degrees, unit, yaw_degrees};
    use crate::vostok::clock::{FileTime, LocalTime};
    use crate::vostok::files::{File, Find, FindData, Path};
    use crate::vostok::free_fly::{self, Screen, ScreenView, ViewCamera};
    use crate::vostok::log::{self, Piece};
    use crate::vostok::world::{LoadedMap, loaded_map};
    use crate::vostok::{Context, Dik, GameCell, GameWorldKey, KeyKind};

    /// The marks' file, in the game's working folder (srp_patches.log's).
    const FILE: &[u8] = b"srp_spawn_marks.tsv\0";

    /// The file's first line: a column each, tab-separated, as [`row`] writes
    /// them.
    const HEADER: &str =
        "n\ttime\tmap\tmap_id\tkind\tcamera\tx\ty\tz\tyaw\tpitch\tdx\tdy\tdz\tplayer_x\tplayer_y\tplayer_z\n";

    /// How long the hud shows a press's message, ms.
    const SHOWN_MS: u32 = 2000;

    /// What a mark says of its place.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        /// A spawn here.
        Spawn,
        /// No spawn near here (an existing one that should go).
        NoSpawn,
    }

    impl Kind {
        /// Its name in the file.
        fn name(self) -> &'static str {
            match self {
                Kind::Spawn => "spawn",
                Kind::NoSpawn => "no_spawn",
            }
        }
    }

    /// What a key asks of the tool.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Asked {
        /// The other camera: the free-fly one, or the player's again.
        Camera,
        Mark(Kind),
    }

    /// What a key event asks: one going down only (a held key marks once).
    fn asked(key: Dik, kind: KeyKind) -> Option<Asked> {
        if kind != KeyKind::Down {
            return None;
        }
        match key {
            Dik::F7 => Some(Asked::Camera),
            Dik::F8 => Some(Asked::Mark(Kind::Spawn)),
            Dik::F9 => Some(Asked::Mark(Kind::NoSpawn)),
            _ => None,
        }
    }

    /// A mark: one line of the file.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Mark {
        /// Its number among this run's marks, from 1 (a new run of the game
        /// starts again at 1).
        n: u32,
        /// The machine's local time, when it could be told.
        time: Option<LocalTime>,
        map: Option<LoadedMap>,
        kind: Kind,
        view: ScreenView,
        /// The local player's place (its feet), when the match has one.
        player: Option<[f32; 3]>,
    }

    /// `value` in two digits.
    fn two(line: &mut Line, value: u8) {
        line.byte(b'0' + value / 10 % 10);
        line.byte(b'0' + value % 10);
    }

    /// The mark's line: [`HEADER`]'s columns. `time` the local `MM-DD HH:MM`;
    /// `map` the project as content edits name it and `map_id` its maps_dict
    /// id; `camera` `free` or `player`; `x y z` the camera's place (metres,
    /// the map's coordinates); `yaw` the look's heading in degrees from +Z
    /// towards +X (0 to 360) and `pitch` its angle above the horizontal, as
    /// `where` says them; `dx dy dz` the look as a unit vector; `player_*`
    /// the local player's feet. `-` for what is not known.
    fn row(mark: &Mark, line: &mut Line) {
        line.number(mark.n);
        line.byte(b'\t');
        match mark.time {
            Some(time) => {
                two(line, time.month);
                line.byte(b'-');
                two(line, time.day);
                line.byte(b' ');
                two(line, time.hour);
                line.byte(b':');
                two(line, time.minute);
            }
            None => line.byte(b'-'),
        }
        line.byte(b'\t');
        match mark.map {
            Some(map) => {
                line.push(map.name());
                line.byte(b'\t');
                match map.id {
                    Some(id) => line.number(u32::from(id)),
                    None => line.byte(b'-'),
                }
            }
            None => line.push("-\t-"),
        }
        line.byte(b'\t');
        line.push(mark.kind.name());
        line.byte(b'\t');
        line.push(match mark.view.camera {
            ViewCamera::Free => "free",
            ViewCamera::Player => "player",
        });
        for value in mark.view.place {
            line.byte(b'\t');
            line.fixed(value, 3);
        }
        match unit(mark.view.forward) {
            Some(forward) => {
                line.byte(b'\t');
                line.fixed(yaw_degrees(forward), 1);
                line.byte(b'\t');
                line.fixed(pitch_degrees(forward), 1);
                for value in forward {
                    line.byte(b'\t');
                    line.fixed(value, 3);
                }
            }
            None => line.push("\t-\t-\t-\t-\t-"),
        }
        match mark.player {
            Some(place) => {
                for value in place {
                    line.byte(b'\t');
                    line.fixed(value, 3);
                }
            }
            None => line.push("\t-\t-\t-"),
        }
        line.byte(b'\n');
    }

    /// Whether the line has all of its text (a full one may have been cut).
    fn whole(line: &Line) -> bool {
        line.text().len() < MOST
    }

    /// The line added at the file's end, the header before it in a file that
    /// is new or empty: whether all of it was written.
    fn append(line: &Line) -> bool {
        let Some(path) = Path::new(FILE) else { return false };
        let mut found = FindData::new();
        let fresh = match Find::first(path, &mut found) {
            Some(_) => found.size() == Some(0),
            None => true,
        };
        let Some(mut file) = File::append(path) else { return false };
        (!fresh || file.write_all(HEADER.as_bytes())) && file.write_all(line.text().as_bytes())
    }

    /// This run's marks so far.
    static MARKS: GameCell<u32> = GameCell::new(0);

    /// What a press did, told in the hud's centre message (when the hud has
    /// its movie) and in srp_patches.log.
    fn say(key: &GameWorldKey<'_, '_>, text: &Line) {
        log::line(&[Piece::Text("spawn_marks: "), Piece::Text(text.text())]);
        if let Some(movie) = key.hud().movie() {
            movie.show_message(text.c_str(), SHOWN_MS);
        }
    }

    fn told(key: &GameWorldKey<'_, '_>, text: &str) {
        let mut line = Line::new();
        line.push(text);
        say(key, &line);
    }

    /// F7: the other camera.
    fn camera(key: &GameWorldKey<'_, '_>) {
        match Screen::of_live(key) {
            Some(Screen::Player(camera)) => {
                camera.fly();
                told(key, "free camera (F7: back to the player)");
            }
            Some(Screen::FreeFly(flight)) => {
                flight.land();
                told(key, "player camera");
            }
            Some(Screen::Unbound) => told(key, "no player's camera to leave or come back to yet"),
            None => told(key, "a replay: F2 is its free view, F1 follows a player"),
        }
    }

    /// F8, F9: a mark of where the screen's camera is.
    fn mark(key: &GameWorldKey<'_, '_>, kind: Kind) {
        let Some(view) = free_fly::view(key) else {
            return told(key, "NO MARK: no camera is on");
        };
        let thread = key.thread();
        let mark = Mark {
            n: MARKS.get(thread) + 1,
            time: FileTime::now(thread).local(thread),
            map: loaded_map(thread),
            kind,
            view,
            player: free_fly::player_place(key),
        };
        let mut line = Line::new();
        row(&mark, &mut line);
        if !(whole(&line) && append(&line)) {
            return told(key, "NO MARK: srp_spawn_marks.tsv could not be written");
        }
        MARKS.set(thread, mark.n);
        let mut text = Line::new();
        text.push("mark ");
        text.number(mark.n);
        text.push(match kind {
            Kind::Spawn => ": spawn here",
            Kind::NoSpawn => ": NO spawn near here",
        });
        say(key, &text);
    }

    /// Whether the key was one of the tool's presses: acted on, and taken.
    pub(super) fn on_key(ctx: &mut Context<'_>) -> bool {
        let Some(key) = GameWorldKey::at_start(ctx) else { return false };
        match asked(key.key(), key.kind()) {
            Some(Asked::Camera) => camera(&key),
            Some(Asked::Mark(kind)) => mark(&key, kind),
            None => return false,
        }
        key.take();
        true
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn text_of(mark: &Mark) -> String {
            let mut line = Line::new();
            row(mark, &mut line);
            assert!(whole(&line));
            line.text().to_string()
        }

        fn sample() -> Mark {
            Mark {
                n: 3,
                time: Some(LocalTime { month: 11, day: 4, hour: 9, minute: 5 }),
                map: Some(LoadedMap::new(b"v021d_level_03/day_2", Some(61))),
                kind: Kind::Spawn,
                view: ScreenView {
                    place: [-51.9, 7.114, 12.914],
                    forward: [0.0, 0.0, -2.0],
                    camera: ViewCamera::Free,
                },
                player: Some([-48.12, 5.414, 10.002]),
            }
        }

        #[test]
        fn a_key_asks_once_when_it_goes_down() {
            assert_eq!(asked(Dik::F7, KeyKind::Down), Some(Asked::Camera));
            assert_eq!(asked(Dik::F8, KeyKind::Down), Some(Asked::Mark(Kind::Spawn)));
            assert_eq!(asked(Dik::F9, KeyKind::Down), Some(Asked::Mark(Kind::NoSpawn)));
            for key in [Dik::F7, Dik::F8, Dik::F9] {
                assert_eq!(asked(key, KeyKind::Held), None);
                assert_eq!(asked(key, KeyKind::Up), None);
            }
            assert_eq!(asked(Dik::F6, KeyKind::Down), None);
            assert_eq!(asked(Dik::TAB, KeyKind::Down), None);
        }

        #[test]
        fn a_mark_is_a_line_of_the_headers_columns() {
            let line = text_of(&sample());
            assert_eq!(
                line,
                "3\t11-04 09:05\tv021d_level_03/day_2\t61\tspawn\tfree\t-51.900\t7.114\t12.914\t180.0\t0.0\t0.000\t0.000\t-1.000\t-48.120\t5.414\t10.002\n"
            );
            assert_eq!(line.matches('\t').count(), HEADER.matches('\t').count());
        }

        #[test]
        fn what_is_not_known_is_a_dash_in_its_column() {
            let bare = Mark {
                n: 1,
                time: None,
                map: None,
                kind: Kind::NoSpawn,
                view: ScreenView { place: [1.0, 2.0, 3.0], forward: [0.0; 3], camera: ViewCamera::Player },
                player: None,
            };
            let line = text_of(&bare);
            assert_eq!(line, "1\t-\t-\t-\tno_spawn\tplayer\t1.000\t2.000\t3.000\t-\t-\t-\t-\t-\t-\t-\t-\n");
            assert_eq!(line.matches('\t').count(), HEADER.matches('\t').count());
            let unnumbered = Mark { map: Some(LoadedMap::new(b"level_01/day_2", None)), ..bare };
            assert!(text_of(&unnumbered).starts_with("1\t-\tlevel_01/day_2\t-\tno_spawn\t"));
        }

        #[test]
        fn the_longest_mark_is_whole() {
            let far = [-123_456.79, -123_456.79, -123_456.79];
            let longest = Mark {
                n: u32::MAX,
                map: Some(LoadedMap::new(&[b'x'; 300], Some(255))),
                kind: Kind::NoSpawn,
                view: ScreenView { place: far, forward: [-0.577, -0.577, -0.577], camera: ViewCamera::Player },
                player: Some(far),
                ..sample()
            };
            let line = text_of(&longest);
            assert!(line.ends_with('\n') && line.len() < MOST, "{} bytes", line.len());
            let mut cut = Line::new();
            cut.push(&"x".repeat(MOST + 1));
            assert!(!whole(&cut));
        }
    }
}
