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
//! - **F10** takes back the last mark of the map: its line leaves the file.
//!
//! Each mark of the map is shown where it is ([`MARKERS`]): a looping
//! particle system of the game's own at the player's feet (at the camera
//! for a mark flown with the free-fly camera), green spores for a spawn, a
//! bonfire for no spawn. The marks of the file come back at the first frame
//! of a match on their map, so marking goes on over matches and runs.
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
//! every other build the three handlers below are the whole patch, each
//! doing nothing, and the arena the lock describes has no byte of the tool
//! (the self-test's test_spawn_marks). The crate's tests compile it too.

#![forbid(unsafe_code)]

use crate::vostok::resources::{QueryResults, QueryTag};
use crate::vostok::{Context, Flow, NoExit, SpawnMarksOnKeyExit};

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

/// game_world::tick after the simulation's move (`game_world.after_move`),
/// every frame of a match: the markers follow the map. Without the tool,
/// nothing.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    #[cfg(any(srp_spawn_marks, test))]
    tool::on_frame(ctx);
    #[cfg(not(any(srp_spawn_marks, test)))]
    let _ = ctx;
    Flow::Continue
}

/// The markers' particles came: the callback only the tool's queries are
/// given. Without the tool, nothing asks it.
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    #[cfg(any(srp_spawn_marks, test))]
    tool::on_loaded(results, tag);
    #[cfg(not(any(srp_spawn_marks, test)))]
    let _ = (results, tag);
}

#[cfg(any(srp_spawn_marks, test))]
mod tool {
    use core::ffi::CStr;

    use crate::console_where::{Line, MOST, pitch_degrees, unit, yaw_degrees};
    use crate::vostok::clock::{FileTime, LocalTime};
    use crate::vostok::collection::BoundedVec;
    use crate::vostok::files::{self, File, Find, FindData, Path};
    use crate::vostok::free_fly::{self, Screen, ScreenView, ViewCamera};
    use crate::vostok::log::{self, Piece};
    use crate::vostok::particle::{Particle, WorldScene};
    use crate::vostok::resources::{self, Arrived, EFFECTS_MOST, QueryResults, QueryTag};
    use crate::vostok::world::{GameWorld, LoadedMap, loaded_map};
    use crate::vostok::{Context, Dik, GameCell, GameWorldKey, KeyKind, Lock, Thread};

    /// The marks' file, in the game's working folder (srp_patches.log's).
    const FILE: &[u8] = b"srp_spawn_marks.tsv\0";
    /// The file F10 writes whole, then puts in the marks' file's place.
    const NEW_FILE: &[u8] = b"srp_spawn_marks.tsv.new\0";

    /// The file's first line: a column each, tab-separated, as [`row`] writes
    /// them.
    const HEADER: &str =
        "n\ttime\tmap\tmap_id\tkind\tcamera\tx\ty\tz\tyaw\tpitch\tdx\tdy\tdz\tplayer_x\tplayer_y\tplayer_z\n";

    /// How long the hud shows a press's message, ms.
    const SHOWN_MS: u32 = 2000;

    /// The most of the file read at once (some 2000 marks): past it, its
    /// marks are not shown and F10 takes none back.
    const TEXT_MOST: usize = 256 * 1024;

    /// The most markers a map shows (the file's first ones).
    const MARKERS_MOST: usize = 256;

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

        fn of_name(name: &[u8]) -> Option<Kind> {
            match name {
                b"spawn" => Some(Kind::Spawn),
                b"no_spawn" => Some(Kind::NoSpawn),
                _ => None,
            }
        }

        /// What its hud message says.
        fn said(self) -> &'static str {
            match self {
                Kind::Spawn => "spawn here",
                Kind::NoSpawn => "NO spawn near here",
            }
        }

        /// The particle system its marker plays, a looping one of the
        /// game's own (no content pack): green spores rising from the
        /// ground (the spore anomaly's, particles/fx/level_4_anomaly/
        /// spori.particle) for a spawn; a bonfire (a map's own,
        /// particles/fx/lights/bonfire.particle) for no spawn.
        fn particle(self) -> &'static CStr {
            match self {
                Kind::Spawn => c"fx/level_4_anomaly/spori",
                Kind::NoSpawn => c"fx/lights/bonfire",
            }
        }
    }

    /// What a key asks of the tool.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Asked {
        /// The other camera: the free-fly one, or the player's again.
        Camera,
        Mark(Kind),
        /// The last mark of the map taken back.
        TakeBack,
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
            Dik::F10 => Some(Asked::TakeBack),
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

    /// Where a line's marker is: its kind and place. The place is the
    /// player's feet for a mark from the player's camera (the spot on the
    /// floor), the camera's place for one from the free-fly camera or with
    /// no player.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Spot {
        kind: Kind,
        at: [f32; 3],
    }

    /// A line of the file read: its map's name (the third column) and its
    /// marker's spot. None for the header, a line cut short and one whose
    /// kind, camera or place is not one [`row`] writes.
    fn parse(line: &[u8]) -> Option<(&[u8], Spot)> {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let mut columns = [&line[..0]; 17];
        let mut count = 0;
        for column in line.split(|&b| b == b'\t') {
            *columns.get_mut(count)? = column;
            count += 1;
        }
        if count != columns.len() {
            return None;
        }
        let kind = Kind::of_name(columns[4])?;
        let place = |at: usize| Some([number(columns[at])?, number(columns[at + 1])?, number(columns[at + 2])?]);
        let at = match (columns[5], place(14)) {
            (b"player", Some(feet)) => feet,
            (b"player" | b"free", _) => place(6)?,
            _ => return None,
        };
        Some((columns[2], Spot { kind, at }))
    }

    /// A number as [`Line::fixed`] writes it (`-51.900`): none for any other
    /// text (`-`, `nan`, `big`).
    fn number(text: &[u8]) -> Option<f32> {
        let (negative, digits) = match text.split_first()? {
            (b'-', rest) => (true, rest),
            _ => (false, text),
        };
        let (whole, fraction) = match digits.iter().position(|&b| b == b'.') {
            Some(dot) => (&digits[..dot], &digits[dot + 1..]),
            None => (digits, &digits[..0]),
        };
        let decimal = |part: &[u8]| part.iter().all(u8::is_ascii_digit);
        if whole.is_empty() || whole.len() > 7 || fraction.len() > 6 || !decimal(whole) || !decimal(fraction) {
            return None;
        }
        let value = |part: &[u8]| part.iter().fold(0u32, |n, &d| n * 10 + u32::from(d - b'0'));
        let scale = 10u32.pow(fraction.len() as u32);
        let magnitude = value(whole) as f32 + value(fraction) as f32 / scale as f32;
        Some(if negative { -magnitude } else { magnitude })
    }

    /// The file's lines: each with its byte range in `text` (its newline
    /// in it).
    fn lines(text: &[u8]) -> impl Iterator<Item = (usize, &[u8])> {
        let mut at = 0;
        core::iter::from_fn(move || {
            if at >= text.len() {
                return None;
            }
            let start = at;
            at = text[start..].iter().position(|&b| b == b'\n').map_or(text.len(), |end| start + end + 1);
            Some((start, &text[start..at]))
        })
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

    /// Why the file's text is not there.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Unread {
        /// No such file (no mark yet), or one that could not be opened.
        Absent,
        /// Past [`TEXT_MOST`].
        TooBig,
        /// ReadFile failed.
        Failed,
    }

    /// The file's text, read whole into `room`.
    fn read(room: &mut [u8]) -> Result<&[u8], Unread> {
        let mut file = Path::new(FILE).and_then(File::open).ok_or(Unread::Absent)?;
        let mut len = 0;
        loop {
            if len == room.len() {
                return Err(Unread::TooBig);
            }
            match file.read(&mut room[len..]) {
                Some(0) => return Ok(&room[..len]),
                Some(got) => len += got as usize,
                None => return Err(Unread::Failed),
            }
        }
    }

    /// Room for the file's text (the game's thread's: every hook of the
    /// tool runs on it).
    static TEXT: Lock<[u8; TEXT_MOST]> = Lock::new([0; TEXT_MOST]);

    /// A marker's particle.
    enum Shown {
        /// None yet: a query brings one.
        Waiting,
        /// Come; the next frame plays it.
        Come(Particle),
        /// Playing in the game world's scene.
        Playing(Particle),
        /// Its query brought none (the particle system did not load).
        Missing,
    }

    /// A mark of the map, shown where it is.
    struct Marker {
        spot: Spot,
        shown: Shown,
    }

    impl Marker {
        fn waiting(spot: Spot) -> Marker {
            Marker { spot, shown: Shown::Waiting }
        }

        /// Out of the scene, when it plays there (its particle is let go with
        /// the marker).
        fn take_out(&self, scene: Option<&WorldScene<'_>>) {
            if let (Shown::Playing(particle), Some(scene)) = (&self.shown, scene) {
                scene.remove(particle);
            }
        }
    }

    type List = BoundedVec<Marker, MARKERS_MOST>;

    /// The markers shown, for the map of their marks.
    enum Markers {
        /// None yet: no match's frame since the game started.
        Unshown,
        Of { map: LoadedMap, list: List },
    }

    /// The particles a query is out for: its first `spawn` are a spawn's,
    /// the next `no_spawn` no spawn's.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Query {
        spawn: usize,
        no_spawn: usize,
    }

    impl Query {
        /// The particles' kinds, in the query's order.
        fn kinds(self) -> impl Iterator<Item = Kind> {
            core::iter::repeat_n(Kind::Spawn, self.spawn).chain(core::iter::repeat_n(Kind::NoSpawn, self.no_spawn))
        }
    }

    struct State {
        markers: Markers,
        /// The query out, none while none is.
        query: Option<Query>,
    }

    /// The tool's markers (the game's thread's, as TEXT).
    static MARKERS: Lock<State> = Lock::new(State { markers: Markers::Unshown, query: None });

    impl State {
        /// The list of the markers of `map`, when they are the ones shown.
        fn list_of(&mut self, map: &[u8]) -> Option<&mut List> {
            match &mut self.markers {
                Markers::Of { map: shown, list } if shown.name().as_bytes() == map => Some(list),
                _ => None,
            }
        }

        /// The markers of `map` shown: the others taken out, the file's
        /// marks of `map` waiting for their particles. Nothing when they are
        /// shown already.
        fn show(&mut self, thread: &Thread, map: LoadedMap, scene: &WorldScene<'_>) {
            if matches!(&self.markers, Markers::Of { map: shown, .. } if *shown == map) {
                return;
            }
            if let Markers::Of { list, .. } = core::mem::replace(&mut self.markers, Markers::Unshown) {
                list.iter().for_each(|marker| marker.take_out(Some(scene)));
            }
            let mut list = List::new();
            let read = TEXT.with(thread, |room| match read(room) {
                Ok(text) => {
                    let mut left = 0u32;
                    for (_, line) in lines(text) {
                        if let Some((of, spot)) = parse(line)
                            && of == map.name().as_bytes()
                            && list.push(Marker::waiting(spot)).is_err()
                        {
                            left += 1;
                        }
                    }
                    Ok(left)
                }
                Err(why) => Err(why),
            });
            let count = list.len() as u32;
            match read {
                Ok(0) | Err(Unread::Absent) => log::line(&[
                    Piece::Text("spawn_marks: "),
                    Piece::Text(map.name()),
                    Piece::Text(": "),
                    Piece::Dec(count),
                    Piece::Text(" marks of srp_spawn_marks.tsv shown"),
                ]),
                Ok(left) => log::line(&[
                    Piece::Text("spawn_marks: "),
                    Piece::Text(map.name()),
                    Piece::Text(": "),
                    Piece::Dec(count),
                    Piece::Text(" marks of srp_spawn_marks.tsv shown, "),
                    Piece::Dec(left),
                    Piece::Text(" more not (the most a map shows)"),
                ]),
                Err(Unread::TooBig) => log::line(&[Piece::Text(
                    "spawn_marks: srp_spawn_marks.tsv is over 256 KiB: none of its marks shown (begin a new file)",
                )]),
                Err(Unread::Failed) => {
                    log::line(&[Piece::Text("spawn_marks: srp_spawn_marks.tsv could not be read: none of its marks shown")])
                }
            }
            self.markers = Markers::Of { map, list };
        }

        /// The markers whose particles came played, at their spots; the
        /// query for those still waiting, when none is out (the caller asks
        /// it, this lock let go: a query's callback may come inside the ask).
        fn present(&mut self, scene: &WorldScene<'_>) -> Option<Query> {
            let Markers::Of { list, .. } = &mut self.markers else { return None };
            let mut waiting = Query { spawn: 0, no_spawn: 0 };
            for marker in list.iter_mut() {
                marker.shown = match core::mem::replace(&mut marker.shown, Shown::Missing) {
                    Shown::Come(particle) => {
                        scene.play(&particle, marker.spot.at);
                        Shown::Playing(particle)
                    }
                    Shown::Waiting => {
                        match marker.spot.kind {
                            Kind::Spawn => waiting.spawn += 1,
                            Kind::NoSpawn => waiting.no_spawn += 1,
                        }
                        Shown::Waiting
                    }
                    other => other,
                };
            }
            if self.query.is_some() || waiting.spawn + waiting.no_spawn == 0 {
                return None;
            }
            let spawn = waiting.spawn.min(EFFECTS_MOST);
            self.query = Some(Query { spawn, no_spawn: waiting.no_spawn.min(EFFECTS_MOST - spawn) });
            self.query
        }

        /// A query's particles: each to the first marker of its kind still
        /// waiting (none left: let go).
        fn came(&mut self, particles: [Option<Particle>; EFFECTS_MOST]) {
            let Some(query) = self.query.take() else { return };
            let mut missing = None;
            for (kind, particle) in query.kinds().zip(particles) {
                let waiting = match &mut self.markers {
                    Markers::Of { list, .. } => {
                        list.iter_mut().find(|m| m.spot.kind == kind && matches!(m.shown, Shown::Waiting))
                    }
                    Markers::Unshown => None,
                };
                if particle.is_none() {
                    missing = Some(kind);
                }
                if let Some(marker) = waiting {
                    marker.shown = particle.map_or(Shown::Missing, Shown::Come);
                }
            }
            if let Some(kind) = missing {
                log::line(&[
                    Piece::Text("spawn_marks: the particle system "),
                    Piece::Text(kind.particle().to_str().unwrap_or("?")),
                    Piece::Text(" did not load: a marker of it is not shown"),
                ]);
            }
        }

        /// The last marker of `spot` of the map taken out of the scene and
        /// let go (none when the map is not the one shown, or the file had a
        /// line no marker is of).
        fn take_back(&mut self, map: &[u8], spot: Spot, scene: Option<&WorldScene<'_>>) {
            let Some(list) = self.list_of(map) else { return };
            let last = list.iter().rposition(|marker| marker.spot == spot);
            if let Some(marker) = last.and_then(|last| list.remove(last)) {
                marker.take_out(scene);
            }
        }
    }

    pub(super) fn on_frame(ctx: &mut Context<'_>) {
        let thread = ctx.thread();
        let Some(world) = GameWorld::at_after_move(ctx) else { return };
        let (Some(map), Some(scene)) = (loaded_map(thread), WorldScene::of(&world)) else { return };
        let query = MARKERS.with(thread, |state| {
            state.show(thread, map, &scene);
            state.present(&scene)
        });
        if let Some(query) = query {
            let mut names = [c""; EFFECTS_MOST];
            let mut count = 0;
            for (slot, kind) in names.iter_mut().zip(query.kinds()) {
                *slot = kind.particle();
                count += 1;
            }
            resources::query_particles(thread, &names[..count], crate::vostok::spawn_marks_on_loaded_entry());
        }
    }

    pub(super) fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
        let thread = results.thread();
        match results.arrived(tag) {
            Arrived::Particles(particles) => MARKERS.with(thread, |state| state.came(particles)),
            // only the tool's particle queries are given this callback
            Arrived::Bus(_) | Arrived::Track(..) | Arrived::Effects(_) => {}
        }
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

    /// F8, F9: a mark of where the screen's camera is, and its marker.
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
        if let Some((map, spot)) = parse(line.text().as_bytes()) {
            // a map not shown yet: the next frame shows the file's marks, this one's too
            let full = MARKERS.with(thread, |state| {
                state.list_of(map).is_some_and(|list| list.push(Marker::waiting(spot)).is_err())
            });
            if full {
                log::line(&[Piece::Text("spawn_marks: the mark is in the file, not shown (the most a map shows)")]);
            }
        }
        let mut text = Line::new();
        text.push("mark ");
        text.number(mark.n);
        text.push(": ");
        text.push(kind.said());
        say(key, &text);
    }

    /// Why F10 took nothing back.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kept {
        /// The file has no mark of the map.
        Nothing,
        /// The file could not be read whole (past 256 KiB, or ReadFile failed).
        Unread,
        /// The file without the line could not be written or put in its place.
        Unwritten,
    }

    /// The file without its last mark of `map`: that mark's spot.
    fn remove_last(room: &mut [u8], map: &[u8]) -> Result<Spot, Kept> {
        let text = match read(room) {
            Ok(text) => text,
            Err(Unread::Absent) => return Err(Kept::Nothing),
            Err(Unread::TooBig | Unread::Failed) => return Err(Kept::Unread),
        };
        let (start, line, spot) = lines(text)
            .filter_map(|(start, line)| parse(line).filter(|(of, _)| *of == map).map(|(_, spot)| (start, line, spot)))
            .last()
            .ok_or(Kept::Nothing)?;
        let (before, after) = (&text[..start], &text[start + line.len()..]);
        let (Some(new), Some(path)) = (Path::new(NEW_FILE), Path::new(FILE)) else { return Err(Kept::Unwritten) };
        let written = File::create_always(new).is_some_and(|mut file| file.write_all(before) && file.write_all(after));
        if !(written && files::replace(new, path)) {
            files::delete(new);
            return Err(Kept::Unwritten);
        }
        Ok(spot)
    }

    /// F10: the last mark of the map taken back, its line and its marker.
    fn take_back(key: &GameWorldKey<'_, '_>) {
        let thread = key.thread();
        let Some(map) = loaded_map(thread) else {
            return told(key, "NOTHING TAKEN BACK: no map loaded");
        };
        let name = map.name().as_bytes();
        match TEXT.with(thread, |room| remove_last(room, name)) {
            Ok(spot) => {
                let scene = key.world().and_then(|world| WorldScene::of(&world));
                let left = MARKERS.with(thread, |state| {
                    state.take_back(name, spot, scene.as_ref());
                    state.list_of(name).map_or(0, |list| list.len() as u32)
                });
                let mut text = Line::new();
                text.push("took back the last mark: ");
                text.push(spot.kind.said());
                text.push(" (");
                text.number(left);
                text.push(" shown)");
                say(key, &text);
            }
            Err(Kept::Nothing) => told(key, "NOTHING TAKEN BACK: no mark of this map in srp_spawn_marks.tsv"),
            Err(Kept::Unread) => told(key, "NOTHING TAKEN BACK: srp_spawn_marks.tsv could not be read whole"),
            Err(Kept::Unwritten) => told(key, "NOTHING TAKEN BACK: srp_spawn_marks.tsv could not be written"),
        }
    }

    /// Whether the key was one of the tool's presses: acted on, and taken.
    pub(super) fn on_key(ctx: &mut Context<'_>) -> bool {
        let Some(key) = GameWorldKey::at_start(ctx) else { return false };
        match asked(key.key(), key.kind()) {
            Some(Asked::Camera) => camera(&key),
            Some(Asked::Mark(kind)) => mark(&key, kind),
            Some(Asked::TakeBack) => take_back(&key),
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
            assert_eq!(asked(Dik::F10, KeyKind::Down), Some(Asked::TakeBack));
            for key in [Dik::F7, Dik::F8, Dik::F9, Dik::F10] {
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

        #[test]
        fn a_line_reads_back_as_its_marker() {
            // a walked mark: the player's feet
            let walked = Mark { view: ScreenView { camera: ViewCamera::Player, ..sample().view }, ..sample() };
            let line = text_of(&walked);
            let (map, spot) = parse(line.as_bytes()).unwrap();
            assert_eq!(map, b"v021d_level_03/day_2");
            assert_eq!(spot, Spot { kind: Kind::Spawn, at: [-48.12, 5.414, 10.002] });
            // a flown one: the camera's place
            let (_, spot) = parse(text_of(&sample()).as_bytes()).unwrap();
            assert_eq!(spot.at, [-51.9, 7.114, 12.914]);
            // no player: the camera's place, for either camera
            let bare = Mark { player: None, kind: Kind::NoSpawn, ..walked };
            assert_eq!(parse(text_of(&bare).as_bytes()).unwrap().1, Spot { kind: Kind::NoSpawn, at: [-51.9, 7.114, 12.914] });
            // a line of the owner's first run, read as the file has it
            let first = b"2\t11-14 01:41\tlevel_02/day_8\t49\tno_spawn\tplayer\t-28.573\t17.811\t136.787\t146.6\t-25.5\t0.497\t-0.431\t-0.754\t-27.578\t15.765\t135.822\r\n";
            assert_eq!(parse(first), Some((&b"level_02/day_8"[..], Spot { kind: Kind::NoSpawn, at: [-27.578, 15.765, 135.822] })));
        }

        #[test]
        fn a_line_not_of_a_mark_reads_as_none() {
            assert_eq!(parse(HEADER.as_bytes()), None);
            assert_eq!(parse(b""), None);
            let line = text_of(&sample());
            assert_eq!(parse(&line.as_bytes()[..line.len() / 2]), None, "cut short");
            assert_eq!(parse(line.replace("\tspawn\t", "\tmaybe\t").as_bytes()), None, "another kind");
            assert_eq!(parse(line.replace("\tfree\t", "\tdrone\t").as_bytes()), None, "another camera");
            let flown_nowhere = line.replace("-51.900", "big");
            assert_eq!(parse(flown_nowhere.as_bytes()), None, "a flown mark with no place");
            assert_eq!(parse(format!("{}\textra\n", line.trim_end()).as_bytes()), None, "a column too many");
        }

        #[test]
        fn numbers_read_as_fixed_writes_them() {
            for (text, value) in [("-51.900", -51.9), ("7.114", 7.114), ("0.000", 0.0), ("123456.790", 123_456.79), ("12", 12.0)] {
                assert_eq!(number(text.as_bytes()), Some(value), "{text}");
            }
            for text in ["-", "", "nan", "inf", "-inf", "big", "1.2.3", "1e5", "+1.0", ".5", "12345678.0"] {
                assert_eq!(number(text.as_bytes()), None, "{text}");
            }
        }

        #[test]
        fn lines_keep_their_ranges() {
            let text = b"a\nbb\n\nccc";
            let got: Vec<_> = lines(text).collect();
            assert_eq!(got, [(0, &b"a\n"[..]), (2, &b"bb\n"[..]), (5, &b"\n"[..]), (6, &b"ccc"[..])]);
            assert_eq!(lines(b"").count(), 0);
        }

        #[test]
        fn a_query_is_its_kinds_in_order() {
            let query = Query { spawn: 2, no_spawn: 1 };
            assert_eq!(query.kinds().collect::<Vec<_>>(), [Kind::Spawn, Kind::Spawn, Kind::NoSpawn]);
            assert_eq!(Query { spawn: 0, no_spawn: 0 }.kinds().count(), 0);
        }
    }
}
