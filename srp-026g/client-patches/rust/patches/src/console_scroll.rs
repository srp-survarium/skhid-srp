//! Patch 159: the console's output scrolls (docs/notes/console-scroll.md).
//!
//! The console shows the game's log, every line of it since the game
//! started, through the engine's scroll view, which the stock console never
//! moves from its end. The mouse wheel moves the view three lines a notch,
//! Page Up and Page Down a page (the lines it shows, less one), End (with
//! Ctrl or without) and Enter go back to the newest line, and so does
//! opening the console. Scrolled up, the view stays where it is as new
//! lines come in (the tick's own behaviour off the end); scrolled down to
//! the end, it follows them again. The console takes these keys and the
//! wheel as it takes every other (it answers 1 before the game's handlers):
//! the game gets none of them while it is open.

#![forbid(unsafe_code)]

use crate::vostok::console_view::{self, ConsoleKey, ConsoleView, ConsoleWheel, Place, Shown};
use crate::vostok::{Context, Dik, Flow, KeyKind, NoExit};

/// A move the reader asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scroll {
    /// Lines towards the oldest (negative: towards the newest).
    Lines(i32),
    /// Pages towards the oldest (negative: towards the newest).
    Pages(i32),
    Newest,
}

/// The wheel's turn a line takes: three lines a notch of 120.
const TURN_PER_LINE: i32 = 40;

/// A key the open console takes, before it handles it as it does.
pub fn on_key(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(ConsoleKey { view, key, kind }) = ConsoleKey::at(ctx)
        && let Some(scroll) = key_scroll(key, kind)
    {
        apply(view, scroll);
    }
    Flow::Continue
}

/// A mouse move the open console takes.
pub fn on_wheel(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(ConsoleWheel { view, turn }) = ConsoleWheel::at(ctx)
        && let Some(scroll) = wheel_scroll(turn)
    {
        apply(view, scroll);
    }
    Flow::Continue
}

/// The console opening: its newest lines.
pub fn on_activate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(view) = console_view::opening(ctx) {
        apply(view, Scroll::Newest);
    }
    Flow::Continue
}

fn apply(mut view: ConsoleView<'_>, scroll: Scroll) {
    if let Some(place) = place(view.shown(), scroll) {
        view.show(place);
    }
}

fn key_scroll(key: Dik, kind: KeyKind) -> Option<Scroll> {
    if kind != KeyKind::Down {
        return None;
    }
    match key {
        Dik::PAGE_UP => Some(Scroll::Pages(1)),
        Dik::PAGE_DOWN => Some(Scroll::Pages(-1)),
        Dik::END | Dik::RETURN => Some(Scroll::Newest),
        _ => None,
    }
}

fn wheel_scroll(turn: i32) -> Option<Scroll> {
    match turn / TURN_PER_LINE {
        0 if turn == 0 => None,
        0 => Some(Scroll::Lines(turn.signum())),
        lines => Some(Scroll::Lines(lines)),
    }
}

/// Where `scroll` takes a view standing at `shown` (none: not filled yet,
/// nothing to scroll). Down to the end it follows the new lines again.
fn place(shown: Option<Shown>, scroll: Scroll) -> Option<Place> {
    let lines = match scroll {
        Scroll::Newest => return Some(Place::Newest),
        Scroll::Lines(lines) => lines,
        Scroll::Pages(pages) => pages.saturating_mul(page(shown?)),
    };
    let shown = shown?;
    let top = (shown.top - lines as f32 * shown.line).max(0.0);
    Some(if top >= shown.end { Place::Newest } else { Place::At(top) })
}

/// A page: the lines shown less one, so one line stays in sight.
fn page(shown: Shown) -> i32 {
    i32::try_from(shown.lines).unwrap_or(i32::MAX).saturating_sub(1).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 17 lines of 20 shown (a 354-pixel view), the log 1000 lines long:
    /// at the end, as the tick's follow leaves the pad (N x 20 - 350).
    fn at_end() -> Shown {
        Shown { top: 19_650.0, end: 19_650.0, line: 20.0, lines: 17 }
    }

    #[test]
    fn keys_and_their_kinds() {
        assert_eq!(key_scroll(Dik::PAGE_UP, KeyKind::Down), Some(Scroll::Pages(1)));
        assert_eq!(key_scroll(Dik::PAGE_DOWN, KeyKind::Down), Some(Scroll::Pages(-1)));
        assert_eq!(key_scroll(Dik::END, KeyKind::Down), Some(Scroll::Newest));
        assert_eq!(key_scroll(Dik::RETURN, KeyKind::Down), Some(Scroll::Newest));
        assert_eq!(key_scroll(Dik::PAGE_UP, KeyKind::Held), None, "held: every frame, not a repeat");
        assert_eq!(key_scroll(Dik::PAGE_UP, KeyKind::Up), None);
        assert_eq!(key_scroll(Dik::LEFT, KeyKind::Down), None);
    }

    #[test]
    fn three_lines_a_notch_and_a_small_turn_one() {
        assert_eq!(wheel_scroll(120), Some(Scroll::Lines(3)));
        assert_eq!(wheel_scroll(-240), Some(Scroll::Lines(-6)));
        assert_eq!(wheel_scroll(15), Some(Scroll::Lines(1)));
        assert_eq!(wheel_scroll(-1), Some(Scroll::Lines(-1)));
        assert_eq!(wheel_scroll(0), None);
        assert_eq!(wheel_scroll(i32::MIN), Some(Scroll::Lines(i32::MIN / 40)));
    }

    #[test]
    fn up_holds_down_to_the_end_follows() {
        assert_eq!(place(Some(at_end()), Scroll::Lines(3)), Some(Place::At(19_590.0)));
        assert_eq!(place(Some(at_end()), Scroll::Pages(1)), Some(Place::At(19_330.0)), "16 lines");
        assert_eq!(place(Some(at_end()), Scroll::Lines(-3)), Some(Place::Newest));
        let up = Shown { top: 19_590.0, ..at_end() };
        assert_eq!(place(Some(up), Scroll::Lines(-2)), Some(Place::At(19_630.0)));
        assert_eq!(place(Some(up), Scroll::Lines(-3)), Some(Place::Newest), "the end reached");
        assert_eq!(place(Some(up), Scroll::Pages(-1)), Some(Place::Newest));
    }

    #[test]
    fn the_top_is_the_first_line() {
        let near = Shown { top: 40.0, ..at_end() };
        assert_eq!(place(Some(near), Scroll::Pages(1)), Some(Place::At(0.0)));
        assert_eq!(place(Some(Shown { top: 0.0, ..at_end() }), Scroll::Lines(3)), Some(Place::At(0.0)));
        assert_eq!(place(Some(at_end()), Scroll::Pages(i32::MAX)), Some(Place::At(0.0)));
    }

    #[test]
    fn a_log_the_view_holds_does_not_scroll() {
        let short = Shown { top: 0.0, end: 0.0, line: 20.0, lines: 17 };
        assert_eq!(place(Some(short), Scroll::Lines(3)), Some(Place::Newest));
        assert_eq!(place(Some(short), Scroll::Pages(-1)), Some(Place::Newest));
    }

    #[test]
    fn an_unfilled_view_only_goes_to_its_end() {
        assert_eq!(place(None, Scroll::Lines(3)), None);
        assert_eq!(place(None, Scroll::Pages(1)), None);
        assert_eq!(place(None, Scroll::Newest), Some(Place::Newest));
    }

    #[test]
    fn a_page_keeps_a_line_in_sight() {
        assert_eq!(page(at_end()), 16);
        assert_eq!(page(Shown { lines: 1, ..at_end() }), 1);
        assert_eq!(page(Shown { lines: 0, ..at_end() }), 1);
    }
}
