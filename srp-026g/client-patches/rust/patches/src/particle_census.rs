//! Patch 138: a census of live particles by effect name, for the owner's
//! own game. Dormant unless the game starts with `SKHID_PARTICLE_CENSUS=1`,
//! read once when the arenas are made ([`Setting`]). Off, the particle code
//! is the game's byte for byte: its two hooks are written into it only when
//! the census is on (`vostok::particle::start`).
//!
//! On, it only reads, and writes lines to `srp_patches.log`:
//!
//! - a particle system instance the cook finishes names its template by the
//!   path its query asked for, and counts as one instance made of that name
//!   ([`Census::created`]): every played effect that is cooked anew shows as
//!   a creation;
//! - about once a second, at the start of a particle world's tick, the
//!   world's ticked instances are counted by name: instances, their current
//!   lod's emitter instances, those emitters' live particles, and the same
//!   again for instances the game hid ([`Census::tick`]);
//! - every ten seconds a report: the totals, then the names holding the most
//!   particles, then the names made most often ([`Report`]). Counts are
//!   means over the window's samples, creations a rate per second.
//!
//! The names live in a fixed table ([`NAMES`]); an instance whose template
//! was never named (made before the table had room, or its name pushed out)
//! counts as `?`. One world is counted at a time ([`Window`]): another
//! world ticking meanwhile (a lobby scene's) is skipped and said so.
#![forbid(unsafe_code)]

use crate::vostok::files::tick_count;
use crate::vostok::log::{parts, Piece, Piece::*};
use crate::vostok::memory_profile::environment;
use crate::vostok::particle::{self, Created, Instance, Shown, StartRefused, TemplateId, World, WorldId};
use crate::vostok::{Context, Flow, InstallRefused, Lock, NoExit};

/// The variable that switches the census on.
const VARIABLE: &[u8] = b"SKHID_PARTICLE_CENSUS\0";
/// The effect names told apart at most.
const NAMES: usize = 256;
/// A sample of the world this often, a report this often.
const SAMPLE_MS: u32 = 1000;
const REPORT_MS: u32 = 10_000;
/// The counted world not ticking for this long: another one takes its place.
const WORLD_GONE_MS: u32 = 3000;
/// The names a report lists by particles held, and by instances made.
const TOP: usize = 12;
const TOP_MADE: usize = 5;

/// SKHID_PARTICLE_CENSUS as the game was started with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Off,
    On,
}

impl Setting {
    /// `1` or `on` (any case) is on; anything else, or nothing, off.
    fn parse(value: Option<&[u8]>) -> Setting {
        match value {
            Some(value) if value == b"1" || value.eq_ignore_ascii_case(b"on") => Setting::On,
            _ => Setting::Off,
        }
    }
}

/// GetTickCount's milliseconds: a moment, compared only by [`Tick::since`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tick(u32);

impl Tick {
    /// The milliseconds from `earlier` to `self` (the counter wraps).
    fn since(self, earlier: Tick) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }
}

/// An effect's name: its resource path's printable ASCII, the last
/// [`Name::MOST`] bytes of a longer one (a path's end tells effects apart).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Name {
    bytes: [u8; Name::MOST],
    len: u8,
}

impl Name {
    const MOST: usize = 63;

    fn from_path(path: &[u8]) -> Name {
        let tail = &path[path.len().saturating_sub(Name::MOST)..];
        let mut bytes = [0u8; Name::MOST];
        for (to, &from) in bytes.iter_mut().zip(tail) {
            *to = if from.is_ascii_graphic() { from } else { b'_' };
        }
        Name { bytes, len: tail.len() as u8 }
    }

    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap_or("?")
    }
}

/// Instances, their emitter instances and those emitters' live particles:
/// sums over a window's samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Held {
    systems: u32,
    emitters: u32,
    particles: u32,
}

impl Held {
    const NONE: Held = Held { systems: 0, emitters: 0, particles: 0 };

    fn add(&mut self, instance: &Instance) {
        self.systems = self.systems.saturating_add(1);
        self.emitters = self.emitters.saturating_add(instance.emitters);
        self.particles = self.particles.saturating_add(instance.particles);
    }

    fn sum(&mut self, other: &Held) {
        self.systems = self.systems.saturating_add(other.systems);
        self.emitters = self.emitters.saturating_add(other.emitters);
        self.particles = self.particles.saturating_add(other.particles);
    }
}

/// What a window counted for one name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts {
    /// Every ticked instance, the hidden ones too.
    all: Held,
    /// Those the game hid.
    hidden: Held,
    /// Instances the cook made, and the emitter instances made with them.
    made: u32,
    made_emitters: u32,
}

impl Counts {
    const NONE: Counts = Counts { all: Held::NONE, hidden: Held::NONE, made: 0, made_emitters: 0 };

    fn add(&mut self, instance: &Instance) {
        self.all.add(instance);
        if instance.shown == Shown::Hidden {
            self.hidden.add(instance);
        }
    }

    fn sum(&mut self, other: &Counts) {
        self.all.sum(&other.all);
        self.hidden.sum(&other.hidden);
        self.made = self.made.saturating_add(other.made);
        self.made_emitters = self.made_emitters.saturating_add(other.made_emitters);
    }
}

/// A name and the template that carries it now (None: the template's
/// address went to another name since).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    name: Name,
    template: Option<TemplateId>,
    counts: Counts,
}

/// The world being counted and its window so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window {
    world: WorldId,
    started: Tick,
    /// The world's last tick, and its last sample.
    seen: Tick,
    sampled: Tick,
    samples: u32,
    /// The world's own count of its particles at the last sample, the most a
    /// sample saw, and its cap.
    particles: u32,
    peak: u32,
    cap: u32,
    /// Ticks of other worlds skipped.
    others: u32,
}

impl Window {
    fn open(world: WorldId, now: Tick) -> Window {
        Window { world, started: now, seen: now, sampled: now, samples: 0, particles: 0, peak: 0, cap: 0, others: 0 }
    }
}

/// Whose a report's row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Who {
    Named(Name),
    /// Instances whose template has no name in the table.
    Unnamed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    who: Who,
    counts: Counts,
}

/// A window, closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Report {
    world: WorldId,
    samples: u32,
    ms: u32,
    total: Counts,
    particles: u32,
    peak: u32,
    cap: u32,
    others: u32,
    names: u32,
    /// By particles held, the most first; by instances made, the most first.
    top: [Option<Row>; TOP],
    made: [Option<Row>; TOP_MADE],
}

struct Census {
    slots: [Option<Slot>; NAMES],
    unnamed: Counts,
    /// The slot a new name takes when none is free.
    evict: usize,
    window: Option<Window>,
}

impl Census {
    const fn new() -> Census {
        Census { slots: [None; NAMES], unnamed: Counts::NONE, evict: 0, window: None }
    }

    /// The cook made an instance of `template`, asked for as `path`, with
    /// `emitters` emitter instances.
    fn created(&mut self, template: TemplateId, path: &[u8], emitters: u32) {
        let name = Name::from_path(path);
        // the template's address may have been another name's before
        for slot in self.slots.iter_mut().flatten() {
            if slot.template == Some(template) && slot.name != name {
                slot.template = None;
            }
        }
        let index = match self.slots.iter().position(|slot| slot.is_some_and(|slot| slot.name == name)) {
            Some(index) => index,
            None => {
                let index = self.slots.iter().position(Option::is_none).unwrap_or_else(|| {
                    self.evict = (self.evict + 1) % NAMES;
                    self.evict
                });
                self.slots[index] = Some(Slot { name, template: None, counts: Counts::NONE });
                index
            }
        };
        if let Some(slot) = &mut self.slots[index] {
            slot.template = Some(template);
            slot.counts.made = slot.counts.made.saturating_add(1);
            slot.counts.made_emitters = slot.counts.made_emitters.saturating_add(emitters);
        }
    }

    /// The cook made an instance the site could not name.
    fn created_unnamed(&mut self) {
        self.unnamed.made = self.unnamed.made.saturating_add(1);
    }

    fn count(&mut self, instance: &Instance) {
        let slot = instance.template.and_then(|template| self.slots.iter_mut().flatten().find(|slot| slot.template == Some(template)));
        match slot {
            Some(slot) => slot.counts.add(instance),
            None => self.unnamed.add(instance),
        }
    }

    fn clear_counts(&mut self) {
        self.slots.iter_mut().flatten().for_each(|slot| slot.counts = Counts::NONE);
        self.unnamed = Counts::NONE;
    }

    /// `world` starts a tick at `now`, holding `particles` of `cap`; `walk`
    /// hands each of its ticked instances to its argument, and is called
    /// only when a sample is due. The report when the window is over.
    fn tick(&mut self, now: Tick, world: WorldId, particles: u32, cap: u32, walk: impl FnOnce(&mut dyn FnMut(Instance))) -> Option<Report> {
        let mut window = match self.window {
            Some(window) if window.world == world => Window { seen: now, ..window },
            Some(window) if now.since(window.seen) < WORLD_GONE_MS => {
                self.window = Some(Window { others: window.others.saturating_add(1), ..window });
                return None;
            }
            Some(_) => {
                self.clear_counts();
                Window::open(world, now)
            }
            None => Window::open(world, now),
        };
        if window.samples == 0 || now.since(window.sampled) >= SAMPLE_MS {
            walk(&mut |instance| self.count(&instance));
            window = Window {
                sampled: now,
                samples: window.samples + 1,
                particles,
                peak: window.peak.max(particles),
                cap,
                ..window
            };
        }
        let ms = now.since(window.started);
        if ms < REPORT_MS {
            self.window = Some(window);
            return None;
        }
        let report = self.report(&window, ms);
        self.clear_counts();
        self.window = Some(Window::open(world, now));
        Some(report)
    }

    fn row(&self, index: usize) -> Option<Row> {
        match self.slots.get(index) {
            Some(slot) => slot.map(|slot| Row { who: Who::Named(slot.name), counts: slot.counts }),
            None => Some(Row { who: Who::Unnamed, counts: self.unnamed }),
        }
    }

    /// The `N` rows with the largest `key`, the largest first; none of key 0.
    fn top<const N: usize>(&self, key: impl Fn(&Counts) -> u32) -> [Option<Row>; N] {
        let mut taken = [false; NAMES + 1];
        let mut out = [None; N];
        for place in &mut out {
            let best = (0..=NAMES)
                .filter(|&index| !taken[index])
                .filter_map(|index| self.row(index).map(|row| (index, row)))
                .filter(|(_, row)| key(&row.counts) > 0)
                .max_by_key(|(index, row)| (key(&row.counts), core::cmp::Reverse(*index)));
            let Some((index, row)) = best else { break };
            taken[index] = true;
            *place = Some(row);
        }
        out
    }

    fn report(&self, window: &Window, ms: u32) -> Report {
        let mut total = self.unnamed;
        self.slots.iter().flatten().for_each(|slot| total.sum(&slot.counts));
        Report {
            world: window.world,
            samples: window.samples,
            ms,
            total,
            particles: window.particles,
            peak: window.peak,
            cap: window.cap,
            others: window.others,
            names: self.slots.iter().flatten().count() as u32,
            top: self.top(|counts| counts.all.particles),
            made: self.top(|counts| counts.made),
        }
    }
}

/// `value` in tenths, as `12.3`.
fn tenths<'a>(value: u32) -> [Piece<'a>; 3] {
    [Dec(value / 10), Text("."), Dec(value % 10)]
}

impl Report {
    /// A sum over the samples as a mean, in tenths.
    fn mean(&self, sum: u32) -> u32 {
        sum.saturating_mul(10) / self.samples.max(1)
    }

    /// A count over the window as a rate per second, in tenths.
    fn rate(&self, count: u32) -> u32 {
        count.saturating_mul(10_000) / self.ms.max(1)
    }

    fn write(&self) {
        let mean = |sum| tenths(self.mean(sum));
        let rate = |count| tenths(self.rate(count));
        let total = &self.total;
        parts(&[
            &[Text("particle census: world "), Hex(self.world.address()), Text(" "), Dec(self.samples), Text(" samples in ")],
            &tenths(self.ms / 100),
            &[Text(" s; systems ")], &mean(total.all.systems), &[Text(" (hidden ")], &mean(total.hidden.systems),
            &[Text(") emitters ")], &mean(total.all.emitters), &[Text(" (hidden ")], &mean(total.hidden.emitters),
            &[Text(") particles ")], &mean(total.all.particles), &[Text(" (hidden ")], &mean(total.hidden.particles),
            &[Text("); world particles "), Dec(self.particles), Text(" peak "), Dec(self.peak), Text(" of "), Dec(self.cap)],
            &[Text("; made ")], &rate(total.made), &[Text(" systems/s ")], &rate(total.made_emitters),
            &[Text(" emitters/s; "), Dec(self.names), Text(" names; other worlds' ticks skipped "), Dec(self.others)],
        ]);
        for (place, row) in self.top.iter().flatten().enumerate() {
            let counts = &row.counts;
            parts(&[
                &[Text("particle census: "), Dec(place as u32 + 1), Text(". "), Text(row.who.text()), Text(": particles ")],
                &mean(counts.all.particles), &[Text(" (hidden ")], &mean(counts.hidden.particles),
                &[Text(") emitters ")], &mean(counts.all.emitters), &[Text(" systems ")], &mean(counts.all.systems),
                &[Text(" made ")], &rate(counts.made), &[Text("/s")],
            ]);
        }
        for (place, row) in self.made.iter().flatten().enumerate() {
            let counts = &row.counts;
            parts(&[
                &[Text("particle census: made "), Dec(place as u32 + 1), Text(". "), Text(row.who.text()), Text(": ")],
                &rate(counts.made), &[Text(" systems/s ")], &rate(counts.made_emitters),
                &[Text(" emitters/s, particles ")], &mean(counts.all.particles),
            ]);
        }
    }
}

impl Who {
    fn text(&self) -> &str {
        match self {
            Who::Named(name) => name.text(),
            Who::Unnamed => "?",
        }
    }
}

/// The game's threads reach it: a world's tick on the thread that ticks it,
/// the cook's last step on the thread its query ends on.
static CENSUS: Lock<Census> = Lock::new(Census::new());

pub fn on_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let mut value = [0u8; 8];
    if Setting::parse(environment(ctx.thread(), VARIABLE, &mut value)) == Setting::Off {
        return Flow::Continue;
    }
    match particle::start(ctx) {
        Ok(()) => parts(&[&[Text("particle census on: a report every 10 s of a particle world's ticks")]]),
        Err(StartRefused::NotTheStart) => parts(&[&[Text("particle census: not at the start; off, nothing written")]]),
        Err(StartRefused::Site(InstallRefused::Changed { site })) => {
            parts(&[&[Text("particle census: the particle code's bytes are not the game's at "), Hex(site), Text("; off, nothing written")]])
        }
        Err(StartRefused::Site(InstallRefused::Protect { site })) => {
            parts(&[&[Text("particle census: no write access at "), Hex(site), Text("; off, nothing written")]])
        }
        Err(StartRefused::Site(InstallRefused::TooMany)) => parts(&[&[Text("particle census: too many sites; off, nothing written")]]),
    }
    Flow::Continue
}

pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let report = World::at_tick(ctx).and_then(|world| {
        let id = world.id()?;
        let now = Tick(tick_count());
        CENSUS.with(ctx.thread(), |census| census.tick(now, id, world.particles(), world.max_particles(), |each| world.instances(each)))
    });
    if let Some(report) = report {
        report.write();
    }
    Flow::Continue
}

pub fn on_created(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let created = Created::at_site(ctx);
    CENSUS.with(ctx.thread(), |census| match created {
        Some(created) => census.created(created.template, created.path, created.emitters),
        None => census.created_unnamed(),
    });
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(template: u32, shown: Shown, emitters: u32, particles: u32) -> Instance {
        Instance { template: Some(TemplateId::at(template)), shown, emitters, particles }
    }

    fn tick(census: &mut Census, ms: u32, world: u32, instances: &[Instance]) -> Option<Report> {
        let particles = instances.iter().map(|instance| instance.particles).sum();
        census.tick(Tick(ms), WorldId::at(world), particles, 50_000, |each| instances.iter().for_each(|&instance| each(instance)))
    }

    fn names(rows: &[Option<Row>]) -> Vec<&str> {
        rows.iter().flatten().map(|row| row.who.text()).collect()
    }

    #[test]
    fn only_one_or_on_switches_it_on() {
        for on in [&b"1"[..], b"on", b"ON", b"On"] {
            assert_eq!(Setting::parse(Some(on)), Setting::On);
        }
        for off in [None, Some(&b"0"[..]), Some(b"off"), Some(b""), Some(b"yes"), Some(b"11")] {
            assert_eq!(Setting::parse(off), Setting::Off);
        }
    }

    #[test]
    fn a_name_is_the_paths_printable_end() {
        assert_eq!(Name::from_path(b"fx/artifacts/onyx").text(), "fx/artifacts/onyx");
        assert_eq!(Name::from_path(b"a b\x01\xFFc").text(), "a_b__c");
        assert_eq!(Name::from_path(b"").text(), "");
        let long: Vec<u8> = (0..100u8).map(|n| b'a' + n % 26).collect();
        let name = Name::from_path(&long);
        assert_eq!(name.text().as_bytes(), &long[100 - Name::MOST..]);
    }

    #[test]
    fn a_tick_count_that_wraps_still_measures() {
        assert_eq!(Tick(5).since(Tick(u32::MAX - 4)), 10);
    }

    #[test]
    fn a_window_reports_means_by_name_and_the_unnamed() {
        let mut census = Census::new();
        census.created(TemplateId::at(0x100), b"fx/glow", 3);
        census.created(TemplateId::at(0x200), b"fx/muzzle", 2);
        census.created(TemplateId::at(0x200), b"fx/muzzle", 2);
        let world = [
            instance(0x100, Shown::Shown, 3, 300),
            instance(0x100, Shown::Hidden, 3, 100),
            instance(0x200, Shown::Shown, 2, 20),
            instance(0x900, Shown::Shown, 1, 7),
            Instance { template: None, shown: Shown::Hidden, emitters: 0, particles: 0 },
        ];
        // a sample at the first tick, then one a second; ticks between them are not samples
        assert_eq!(tick(&mut census, 1000, 0x5000, &world), None);
        assert_eq!(tick(&mut census, 1016, 0x5000, &world), None);
        for second in 1..10 {
            assert_eq!(tick(&mut census, 1000 + second * 1000, 0x5000, &world[..3]), None);
        }
        let report = tick(&mut census, 11_000, 0x5000, &world[..3]).expect("the window is over");
        assert_eq!((report.samples, report.ms, report.names, report.others), (11, 10_000, 2, 0));
        assert_eq!((report.particles, report.peak, report.cap), (420, 427, 50_000));
        assert_eq!(names(&report.top), ["fx/glow", "fx/muzzle", "?"]);
        let glow = report.top[0].expect("a row").counts;
        assert_eq!(glow.all, Held { systems: 22, emitters: 66, particles: 4400 });
        assert_eq!(glow.hidden, Held { systems: 11, emitters: 33, particles: 1100 });
        assert_eq!((report.mean(glow.all.particles), report.mean(glow.hidden.systems)), (4000, 10));
        let unnamed = report.top[2].expect("a row").counts;
        assert_eq!((unnamed.all, unnamed.hidden.systems), (Held { systems: 2, emitters: 1, particles: 7 }, 1));
        assert_eq!(names(&report.made), ["fx/muzzle", "fx/glow"]);
        assert_eq!((report.total.made, report.total.made_emitters, report.rate(report.total.made)), (3, 7, 3));
        assert_eq!(report.total.all.systems, 3 * 11 + 2);
        // the next window starts empty, its names kept
        for second in 1..10 {
            assert_eq!(tick(&mut census, 11_000 + second * 1000, 0x5000, &world[2..3]), None);
        }
        let next = tick(&mut census, 21_000, 0x5000, &world[2..3]).expect("the next window");
        assert_eq!((names(&next.top), next.total.made, next.samples), (vec!["fx/muzzle"], 0, 10));
    }

    #[test]
    fn a_reused_template_address_takes_the_new_name() {
        let mut census = Census::new();
        census.created(TemplateId::at(0x100), b"fx/old", 1);
        census.created(TemplateId::at(0x100), b"fx/new", 1);
        // the old name's template reloaded elsewhere
        census.created(TemplateId::at(0x300), b"fx/old", 1);
        let world = [instance(0x100, Shown::Shown, 1, 5), instance(0x300, Shown::Shown, 1, 9)];
        assert_eq!(tick(&mut census, 0, 0x5000, &world), None);
        let report = tick(&mut census, 10_000, 0x5000, &world).expect("a report");
        assert_eq!(names(&report.top), ["fx/old", "fx/new"]);
        assert_eq!(report.names, 2);
        assert_eq!(report.top[0].expect("a row").counts.made, 2);
    }

    #[test]
    fn a_full_table_pushes_a_name_out() {
        let mut census = Census::new();
        for n in 0..NAMES as u32 + 1 {
            census.created(TemplateId::at(0x1000 + n * 0x10), format!("fx/{n}").as_bytes(), 1);
        }
        assert_eq!(census.slots.iter().flatten().count(), NAMES);
        let last = format!("fx/{NAMES}");
        assert!(census.slots.iter().flatten().any(|slot| slot.name.text() == last));
        // the pushed-out name's instances are the unnamed
        let gone = census.slots.iter().flatten().all(|slot| slot.name.text() != "fx/1");
        assert!(gone);
        assert_eq!(tick(&mut census, 0, 0x5000, &[instance(0x1010, Shown::Shown, 1, 4)]), None);
        assert_eq!(census.unnamed.all.particles, 4);
    }

    #[test]
    fn another_world_is_skipped_while_the_first_ticks_and_taken_when_it_stops() {
        let mut census = Census::new();
        census.created(TemplateId::at(0x100), b"fx/glow", 1);
        let world = [instance(0x100, Shown::Shown, 1, 5)];
        assert_eq!(tick(&mut census, 0, 0x5000, &world), None);
        assert_eq!(tick(&mut census, 10, 0x6000, &world), None);
        assert_eq!(tick(&mut census, 2000, 0x5000, &world), None);
        assert_eq!(tick(&mut census, 2010, 0x6000, &world), None);
        let report = tick(&mut census, 10_000, 0x5000, &world).expect("the first world's report");
        assert_eq!((report.world, report.others, report.samples), (WorldId::at(0x5000), 2, 3));
        // the first world gone for three seconds: the second is counted from then
        assert_eq!(tick(&mut census, 12_999, 0x6000, &world), None);
        assert_eq!(census.window.map(|window| window.world), Some(WorldId::at(0x5000)));
        assert_eq!(tick(&mut census, 13_000, 0x6000, &world), None);
        assert_eq!(census.window.map(|window| (window.world, window.samples)), Some((WorldId::at(0x6000), 1)));
    }

    #[test]
    fn an_unnamed_creation_is_counted() {
        let mut census = Census::new();
        census.created_unnamed();
        assert_eq!(tick(&mut census, 0, 0x5000, &[]), None);
        let report = tick(&mut census, 10_000, 0x5000, &[]).expect("a report");
        assert_eq!((report.total.made, names(&report.made)), (1, vec!["?"]));
        assert_eq!(names(&report.top), Vec::<&str>::new());
    }
}
