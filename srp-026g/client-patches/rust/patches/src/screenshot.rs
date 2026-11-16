//! Patch 152: two keys save the game's picture as a PNG
//! (docs/notes/screenshots.md).
//!
//! - `screenshot` (F11): the frame as the screen shows it;
//! - `screenshot_no_ui` (F12): the same frame before the interface is drawn
//!   over it.
//!
//! A key's press (`keyboard.events`: every poll of the keyboard, in a match
//! or out of one) asks for a picture; the next frame takes it at its moment
//! ([`step`]): before the interface's movies are drawn (`render.flash_ui`) or
//! before the frame is presented (`render_output.present`). The back buffer
//! is read back there and written to `screenshots\<date>_<time>.png` in the
//! game's folder, on the thread that draws: the frame waits for the file.
//! Each client's own pictures: nothing a match computes.

#![forbid(unsafe_code)]

mod png;

use crate::vostok::back_buffer::{Call, Channels, Output, Picture, ReadError};
use crate::vostok::clock::{self, FileTime};
use crate::vostok::files::{self, File, Path};
use crate::vostok::log::{self, Piece};
use crate::vostok::run_log::Stamp;
use crate::vostok::{Action, Context, Flow, GameCell, KeyEvents, Lock, NoExit, Thread};

/// What a picture shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shot {
    /// The frame as presented.
    Screen,
    /// The frame without the interface.
    Scene,
}

/// A frame's two moments a picture is taken at, in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Moment {
    BeforeInterface,
    BeforePresent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    /// A key asked for a picture.
    Asked(Shot),
    Frame(Moment),
}

/// The picture asked for and not taken yet, and after `event`: that, and
/// the picture to take now. A later key replaces an earlier one's; a picture
/// waits for its own moment of a frame.
fn step(wanted: Option<Shot>, event: Event) -> (Option<Shot>, Option<Shot>) {
    match (wanted, event) {
        (_, Event::Asked(shot)) => (Some(shot), None),
        (Some(Shot::Scene), Event::Frame(Moment::BeforeInterface)) => (None, Some(Shot::Scene)),
        (Some(Shot::Screen), Event::Frame(Moment::BeforePresent)) => (None, Some(Shot::Screen)),
        (wanted, Event::Frame(_)) => (wanted, None),
    }
}

static WANTED: GameCell<Option<Shot>> = GameCell::new(None);

/// The PNG's working memory (zeros: the arena's state, no file bytes).
static WORK: Lock<png::Work> = Lock::new(png::Work::new());

/// A poll's keys: a press of either key asks for its picture.
pub fn on_keys(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let ctx = &*ctx;
    let Some(events) = KeyEvents::at(ctx) else { return Flow::Continue };
    for key in events.pressed() {
        let shot = match events.game_action(key) {
            Some(Action::SCREENSHOT) => Shot::Screen,
            Some(Action::SCREENSHOT_NO_UI) => Shot::Scene,
            _ => continue,
        };
        WANTED.update(ctx.thread(), |wanted| step(wanted, Event::Asked(shot)).0);
    }
    Flow::Continue
}

/// The frame's draw before the interface's movies.
pub fn on_interface(ctx: &mut Context<'_>) -> Flow<NoExit> {
    frame(ctx, Moment::BeforeInterface);
    Flow::Continue
}

/// The frame's Present.
pub fn on_present(ctx: &mut Context<'_>) -> Flow<NoExit> {
    frame(ctx, Moment::BeforePresent);
    Flow::Continue
}

fn frame(ctx: &Context<'_>, moment: Moment) {
    let thread = ctx.thread();
    let (wanted, take) = step(WANTED.get(thread), Event::Frame(moment));
    WANTED.set(thread, wanted);
    if take.is_none() {
        return;
    }
    let output = match moment {
        Moment::BeforeInterface => Output::before_interface(ctx),
        Moment::BeforePresent => Output::before_present(ctx),
    };
    match output {
        Some(output) => save(thread, &output),
        None => log::line(&[Piece::Text("screenshot: not taken, the frame has no render output")]),
    }
}

/// The back buffer as a PNG's source.
struct Frame<'a>(&'a Picture<'a>);

impl png::Source for Frame<'_> {
    fn width(&self) -> u32 {
        self.0.width()
    }

    fn height(&self) -> u32 {
        self.0.height()
    }

    fn order(&self) -> png::Order {
        match self.0.channels() {
            Channels::Rgb => png::Order::Rgb,
            Channels::Bgr => png::Order::Bgr,
        }
    }

    fn row(&self, y: u32) -> &[u8] {
        self.0.row(y)
    }
}

impl png::Sink for File {
    fn write(&mut self, bytes: &[u8]) {
        File::write(self, bytes);
    }
}

/// The pictures' folder: `screenshots` in the game's folder, three up from
/// the working folder (`game\binaries\x86`, the exe's).
const FOLDER: &[u8] = b"..\\..\\..\\screenshots";

/// A picture's path: `<folder>\<stamp>.png`, a further one of the same
/// second `<stamp>_<n>.png`, n from 2 to 9; with its NUL.
struct Name {
    bytes: [u8; Name::ROOM],
    len: usize,
}

impl Name {
    const ROOM: usize = 64;
    const LAST: u8 = 9;

    fn of(stamp: Stamp, nth: u8) -> Name {
        let (text, suffix) = (stamp.text(), [b'_', b'0' + nth]);
        let suffix: &[u8] = if nth > 1 { &suffix } else { &[] };
        let mut name = Name { bytes: [0; Name::ROOM], len: 0 };
        for &byte in [FOLDER, b"\\", &text, suffix, b".png"].into_iter().flatten() {
            // the parts are 47 bytes at most: the last byte stays the NUL
            if let Some(slot) = name.bytes.get_mut(name.len).filter(|_| name.len + 1 < Name::ROOM) {
                *slot = byte;
                name.len += 1;
            }
        }
        name
    }

    fn path(&self) -> Option<Path<'_>> {
        Path::new(self.bytes.get(..=self.len)?)
    }

    /// The file's own name, past the folder.
    fn file(&self) -> &str {
        let name = self.bytes.get(FOLDER.len() + 1..self.len).unwrap_or(&[]);
        core::str::from_utf8(name).unwrap_or("")
    }

    /// The first name of `stamp` no file has, the file made; none when the
    /// second's nine are taken or the folder cannot be written.
    fn create(stamp: Stamp) -> Option<(Name, File)> {
        (1..=Name::LAST).map(|nth| Name::of(stamp, nth)).find_map(|name| {
            let path = name.path()?;
            if File::open(path).is_some() {
                return None;
            }
            let file = File::create_always(path)?;
            Some((name, file))
        })
    }
}

fn save(thread: &Thread, output: &Output<'_>) {
    let started = clock::milliseconds(thread);
    let picture = match output.read() {
        Ok(picture) => picture,
        Err(error) => return refused(error),
    };
    let Some(stamp) = FileTime::now(thread).local(thread).and_then(Stamp::of) else {
        return log::line(&[Piece::Text("screenshot: not saved, no local time")]);
    };
    if let Some(folder) = Path::new(b"..\\..\\..\\screenshots\0") {
        files::create_folder(folder);
    }
    let Some((name, mut file)) = Name::create(stamp) else {
        return log::line(&[Piece::Text("screenshot: not saved, no file could be made in screenshots")]);
    };
    let written = WORK.with(thread, |work| png::write(&Frame(&picture), work, &mut file));
    drop(file);
    match written {
        Ok(()) => log::line(&[
            Piece::Text("screenshot: "),
            Piece::Text(name.file()),
            Piece::Text(", "),
            Piece::Dec(picture.width()),
            Piece::Text(" x "),
            Piece::Dec(picture.height()),
            Piece::Text(", "),
            Piece::Dec(clock::milliseconds(thread).wrapping_sub(started)),
            Piece::Text(" ms"),
        ]),
        Err(refusal) => {
            if let Some(path) = name.path() {
                files::delete(path);
            }
            log::line(&[
                Piece::Text("screenshot: not saved, the picture is "),
                Piece::Text(match refusal {
                    png::Refusal::Empty => "empty",
                    png::Refusal::TooWide => "wider than 8192",
                }),
            ]);
        }
    }
}

fn refused(error: ReadError) {
    match error {
        ReadError::NoDevice => log::line(&[Piece::Text("screenshot: not taken, no render device")]),
        ReadError::Unread { format, samples, width, height } => log::line(&[
            Piece::Text("screenshot: not taken, the back buffer is not read: DXGI format "),
            Piece::Dec(format),
            Piece::Text(", "),
            Piece::Dec(samples),
            Piece::Text(" sample(s), "),
            Piece::Dec(width),
            Piece::Text(" x "),
            Piece::Dec(height),
        ]),
        ReadError::Refused(call, result) => log::line(&[
            Piece::Text("screenshot: not taken, Direct3D refused "),
            Piece::Text(match call {
                Call::GetBuffer => "GetBuffer",
                Call::CreateTexture2D => "CreateTexture2D",
                Call::Map => "Map",
            }),
            Piece::Text(": "),
            Piece::Hex(result),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::clock::LocalTime;

    #[test]
    fn a_picture_waits_for_its_moment() {
        use Event::{Asked, Frame};
        use Moment::{BeforeInterface, BeforePresent};
        // nothing asked: frames take nothing
        assert_eq!(step(None, Frame(BeforeInterface)), (None, None));
        assert_eq!(step(None, Frame(BeforePresent)), (None, None));
        // the screen's picture passes the interface's moment and is taken at the Present
        assert_eq!(step(None, Asked(Shot::Screen)), (Some(Shot::Screen), None));
        assert_eq!(step(Some(Shot::Screen), Frame(BeforeInterface)), (Some(Shot::Screen), None));
        assert_eq!(step(Some(Shot::Screen), Frame(BeforePresent)), (None, Some(Shot::Screen)));
        // the scene's is taken before the interface; asked between a frame's
        // two moments, it waits for the next frame's
        assert_eq!(step(Some(Shot::Scene), Frame(BeforeInterface)), (None, Some(Shot::Scene)));
        assert_eq!(step(Some(Shot::Scene), Frame(BeforePresent)), (Some(Shot::Scene), None));
        // the later key's picture is the one taken
        assert_eq!(step(Some(Shot::Screen), Asked(Shot::Scene)), (Some(Shot::Scene), None));
    }

    fn stamp() -> Stamp {
        Stamp::of(LocalTime { year: 2026, month: 11, day: 16, hour: 21, minute: 3, second: 55 }).unwrap()
    }

    #[test]
    fn a_name_is_the_second_and_a_number_past_the_first() {
        let first = Name::of(stamp(), 1);
        assert_eq!(first.file(), "2026-11-16_21-03-55.png");
        assert_eq!(&first.bytes[..=first.len], b"..\\..\\..\\screenshots\\2026-11-16_21-03-55.png\0");
        assert!(first.path().is_some());
        assert_eq!(Name::of(stamp(), 2).file(), "2026-11-16_21-03-55_2.png");
        assert_eq!(Name::of(stamp(), Name::LAST).file(), "2026-11-16_21-03-55_9.png");
    }
}
