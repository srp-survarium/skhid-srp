//! The machine against a fake engine: its voices counted, its bus stepped
//! as the sound thread steps the real one. The rules are music.md's.

use std::cell::Cell;
use std::rc::Rc;
use std::vec::Vec;

use super::*;

/// A sound or a voice that counts how many of its kind are alive.
struct Counted {
    alive: Rc<Cell<i32>>,
}

impl Counted {
    fn new(alive: &Rc<Cell<i32>>) -> Counted {
        alive.set(alive.get() + 1);
        Counted { alive: alive.clone() }
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.alive.set(self.alive.get() - 1);
    }
}

struct Fake {
    now: u64,
    serial: Serial,
    loads: Vec<(Serial, Track)>,
    sounds: Rc<Cell<i32>>,
    voices: Rc<Cell<i32>>,
    loaded: Vec<(Serial, Track)>,
    bus: Bus<()>,
    /// The most voices ever alive at once.
    most: i32,
    /// A voice was heard (its bus above silent) while another was alive.
    overlapped: bool,
}

impl Fake {
    fn new() -> Fake {
        let mut bus = Bus::new();
        bus.scene = Some(());
        Fake {
            now: 1000, serial: Serial::FIRST, loads: Vec::new(), sounds: Rc::default(), voices: Rc::default(),
            loaded: Vec::new(), bus, most: 0, overlapped: false,
        }
    }

    fn sound(&self) -> Counted {
        Counted::new(&self.sounds)
    }
}

impl Engine for Fake {
    type Emitter = Counted;
    type Voice = Counted;

    fn now(&self) -> Millis {
        Millis(self.now)
    }

    fn load(&mut self, track: &Track) -> Serial {
        self.serial = self.serial.next();
        self.loads.push((self.serial, *track));
        self.serial
    }

    fn play(&mut self, _: &Counted) -> Option<Counted> {
        // the bus must be silent when a voice starts, and no other voice alive
        if self.voices.get() != 0 || !self.bus_volume_is_silent_after_commands() {
            self.overlapped = true;
        }
        let voice = Counted::new(&self.voices);
        self.most = self.most.max(self.voices.get());
        Some(voice)
    }

    fn fade(&mut self, to: Volume) {
        self.bus.fade(to);
    }

    fn cut(&mut self) {
        self.bus.cut();
    }

    fn silent(&self) -> bool {
        self.bus.silent()
    }
}

impl Fake {
    /// The bus as the sound thread will make it at its next tick, before
    /// any time passes: silent when a cut is pending or it is silent.
    fn bus_volume_is_silent_after_commands(&self) -> bool {
        self.bus.cuts != self.bus.seen_cuts || self.bus.volume == Volume::SILENT
    }
}

/// The machine and its fake engine, driven as the hooks drive it.
struct Rig {
    music: Option<Music<Counted, Counted>>,
    fake: Fake,
}

impl Rig {
    fn new() -> Rig {
        Rig { music: Some(Music::Silent), fake: Fake::new() }
    }

    fn tell(&mut self, event: Event<Counted>) {
        let music = self.music.take().expect("a state");
        self.music = Some(music.step(event, &mut self.fake));
        self.fake.most = self.fake.most.max(self.fake.voices.get());
        assert!(self.fake.voices.get() <= 1, "two voices at once");
        assert_eq!(self.music.as_ref().unwrap().voices() as i32, self.fake.voices.get());
    }

    fn ask(&mut self, track: Option<Track>) {
        self.tell(Event::Ask(track.map(Want::new)));
    }

    /// `ms` go by: the sound thread ticks every 10 ms, the game every 16.
    fn wait(&mut self, ms: u64) {
        let end = self.fake.now + ms;
        while self.fake.now < end {
            self.fake.now += 1;
            if self.fake.now.is_multiple_of(10) {
                self.fake.bus.tick(10);
            }
            if self.fake.now.is_multiple_of(16) {
                self.tell(Event::Frame);
            }
        }
    }

    /// The pending load of `track` arrives (with its sound, or none).
    fn arrive(&mut self, found: bool) {
        let (serial, track) = self.fake.loads.pop().expect("a load asked");
        self.fake.loaded.push((serial, track));
        let sound = found.then(|| self.fake.sound());
        self.tell(Event::Arrived(serial, sound));
    }

    fn playing(&self) -> Option<Track> {
        match self.music.as_ref().unwrap() {
            Music::Sounding { now, .. } | Music::FadingOut { now, .. } => Some(now.track),
            _ => None,
        }
    }

    fn volume(&self) -> f32 {
        self.fake.bus.volume.get()
    }
}

fn map(name: &CStr) -> Track {
    Track::Map(TrackPath::new(name).unwrap())
}

#[test]
fn login_then_lobby_fades_out_before_the_lobby_starts() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Main));
    rig.arrive(true);
    rig.wait(2100);
    assert_eq!(rig.playing(), Some(Track::Main));
    assert_eq!(rig.volume(), 1.0, "faded in fully within 2 s");
    rig.ask(Some(Track::Lobby));
    assert!(rig.fake.loads.is_empty(), "the lobby theme is not asked while the main one fades");
    rig.wait(1000);
    assert!(rig.volume() > 0.4 && rig.volume() < 0.6, "half way out after 1 s");
    rig.wait(1100);
    assert_eq!(rig.playing(), None, "the main theme gone once silent");
    assert_eq!(rig.fake.loads.last().map(|l| l.1), Some(Track::Lobby));
    rig.arrive(true);
    assert_eq!(rig.playing(), Some(Track::Lobby));
    assert!(!rig.fake.overlapped && rig.fake.most == 1);
}

#[test]
fn the_map_theme_plays_fifteen_seconds_past_the_announcement() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Lobby));
    rig.arrive(true);
    rig.wait(3000);
    let level = map(c"music/level_03_loading");
    rig.ask(Some(level));
    rig.wait(2100);
    rig.arrive(true);
    rig.wait(30_000);
    assert_eq!(rig.playing(), Some(level), "the loading and the countdown do not count");
    rig.tell(Event::Announced);
    rig.wait(14_900);
    assert_eq!(rig.playing(), Some(level));
    assert_eq!(rig.volume(), 1.0, "full until 15 s");
    rig.wait(1100);
    assert!(rig.volume() < 0.6, "fading out after 15 s");
    rig.wait(1100);
    assert_eq!(rig.playing(), None, "silent 2 s later");
    rig.tell(Event::Announced);
    assert!(rig.fake.loads.is_empty());
}

#[test]
fn a_quick_return_fades_the_same_track_back_in() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Lobby));
    rig.arrive(true);
    rig.wait(2100);
    rig.ask(Some(map(c"music/level_01_loading")));
    rig.wait(500);
    rig.ask(Some(Track::Lobby));
    assert_eq!(rig.playing(), Some(Track::Lobby));
    assert!(matches!(rig.music, Some(Music::Sounding { .. })));
    rig.wait(2100);
    assert_eq!(rig.volume(), 1.0, "back in from where it was, never restarted");
    assert_eq!(rig.fake.voices.get(), 1);
}

#[test]
fn another_track_during_a_fade_out_stops_the_fading_one_at_once() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Main));
    rig.arrive(true);
    rig.wait(2100);
    rig.ask(Some(Track::Lobby));
    rig.wait(500);
    rig.ask(Some(map(c"music/level_02_loading")));
    assert_eq!(rig.playing(), None, "stopped at once");
    assert_eq!(rig.fake.voices.get(), 0);
    rig.arrive(true);
    rig.wait(50);
    assert!(rig.volume() < 0.05, "the new one fades in from silence");
    assert!(!rig.fake.overlapped);
}

#[test]
fn a_missing_track_is_silence_and_a_late_one_is_let_go() {
    let mut rig = Rig::new();
    rig.ask(Some(map(c"music/none")));
    rig.arrive(false);
    assert!(matches!(rig.music, Some(Music::Silent)));
    rig.ask(Some(Track::Main));
    rig.ask(Some(Track::Lobby));
    let lobby = rig.fake.loads.pop().unwrap();
    let main = rig.fake.loads.pop().unwrap();
    let late = rig.fake.sound();
    rig.tell(Event::Arrived(main.0, Some(late)));
    assert_eq!(rig.fake.sounds.get(), 0, "the main theme, no longer wanted, let go");
    assert_eq!(rig.playing(), None);
    rig.tell(Event::Arrived(lobby.0, Some(rig.fake.sound())));
    assert_eq!(rig.playing(), Some(Track::Lobby));
}

#[test]
fn a_mode_without_a_theme_fades_out_to_silence() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Lobby));
    rig.arrive(true);
    rig.wait(2100);
    rig.ask(None);
    rig.wait(2100);
    assert!(matches!(rig.music, Some(Music::Silent)));
    assert!(rig.fake.loads.is_empty());
}

#[test]
fn the_unload_stops_everything() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Lobby));
    rig.arrive(true);
    rig.tell(Event::Unload);
    assert!(matches!(rig.music, Some(Music::Silent)));
    assert_eq!((rig.fake.voices.get(), rig.fake.sounds.get()), (0, 0));
}

#[test]
fn an_announcement_before_the_theme_loaded_still_counts() {
    let mut rig = Rig::new();
    rig.ask(Some(map(c"music/level_05_loading")));
    rig.tell(Event::Announced);
    rig.wait(16_000);
    // its 15 s went by while it loaded: it never starts
    rig.arrive(true);
    assert!(matches!(rig.music, Some(Music::Silent)));
    assert_eq!(rig.fake.sounds.get(), 0);
}

#[test]
fn the_same_track_asked_again_plays_on() {
    let mut rig = Rig::new();
    rig.ask(Some(Track::Lobby));
    rig.arrive(true);
    rig.wait(500);
    rig.ask(Some(Track::Lobby));
    assert!(rig.fake.loads.is_empty());
    assert!(matches!(rig.music, Some(Music::Sounding { .. })));
}

#[test]
fn fast_random_screens_never_give_two_voices() {
    let mut rig = Rig::new();
    let tracks = [Some(Track::Main), Some(Track::Lobby), Some(map(c"music/level_01_loading")),
                  Some(map(c"music/level_07_loading")), None];
    let mut seed = 0x2545_u32.wrapping_mul(0xf491);
    for _ in 0..20_000 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        match seed % 7 {
            0 | 1 => rig.ask(tracks[(seed >> 8) as usize % tracks.len()]),
            2 => rig.tell(Event::Announced),
            3 if !rig.fake.loads.is_empty() => rig.arrive(seed & 0x100 != 0),
            4 => rig.tell(Event::Unload),
            _ => rig.wait(u64::from(seed >> 20) % 900),
        }
        assert!(rig.fake.voices.get() <= 1);
    }
    assert!(!rig.fake.overlapped, "a voice started while another was heard");
    assert_eq!(rig.fake.most, 1);
}

#[test]
fn the_bus_steps_at_a_fade_a_two_seconds() {
    let mut bus = Bus::new();
    bus.scene = Some(());
    bus.fade(Volume::FULL);
    assert!(!bus.silent() || bus.volume == Volume::SILENT);
    for _ in 0..100 {
        bus.tick(10);
    }
    assert!((bus.volume.get() - 0.5).abs() < 1e-4);
    bus.cut();
    assert!(!bus.silent(), "a cut not yet seen");
    bus.tick(0);
    assert!(bus.silent());
    bus.tick(2000);
    assert_eq!(bus.volume, Volume::FULL);
    bus.fade(Volume::SILENT);
    bus.tick(4000);
    assert!(bus.silent());
}
