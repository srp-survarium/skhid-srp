//! Patch 113: the hud's kits and cursor (README "The hud's kits and cursor").
//!
//! - **A new match.** The hud is the game's for its whole life, and patch 31
//!   keeps the kits (and the cursor with them) shown until play, so a match
//!   that ends with them shown (left before the start, or dead at its end)
//!   leaves them shown for the next: there the kits' first showing returned
//!   at once, the movie was never told the new match's kit and kept the last
//!   match's pick on its tab. At each match's start the kits and the cursor
//!   are hidden, as the hud movie's `reset` hides its kits.
//! - **The main menu.** Esc over the kits (before the start, on the death
//!   screen) brings the main menu with its own cursor; the hud's stayed drawn
//!   under it where it was. While the menu is shown over a match the hud's
//!   cursor movie is off the screen: taken off when the menu comes, not put
//!   up when the hud shows its cursor (a death under the menu), put back
//!   when the menu closes over the match that still runs. The hud's cursor
//!   flag is the game's throughout, so the mouse goes where it went.
//!
//! The game shows and hides the movie by its flag (+2DCh), and a hide of a
//! movie the scene does not have crashes the render thread
//! (vostok/hud_input.rs). So where the movie is, whatever the flag says, is
//! [`CursorMovie`], kept here, and every hide of the game's passes
//! [`on_cursor_hidden`], which only writes the flag when the movie is off.

#![forbid(unsafe_code)]

use crate::vostok::{
    Context, Flow, GameCell, HudCursorOnCursorHiddenExit, HudCursorOnCursorShownExit, Hud, MainMenu, NoExit, Thread,
};

/// Where the hud's cursor movie is, against the game's flag (+2DCh).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorMovie {
    /// As the game keeps it: in the scene exactly while the flag is set.
    AsFlag,
    /// The main menu is shown over the match: out of the scene, whatever
    /// the flag; the menu's close over the match puts it back.
    Covered,
    /// The match ended under the menu with the flag set: out of the scene
    /// until the next hide (the next match's start at the latest), which
    /// only clears the flag.
    Left,
}

/// The hud's game world at the main menu's show or close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct World {
    /// A match runs in it (the active scene, +D4h).
    running: bool,
    /// The hud's cursor flag (+2DCh).
    cursor: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    MenuShown(World),
    MenuClosed(World),
    /// show_cursor about to put the movie in the scene (the flag 0, now 1).
    Show,
    /// show_cursor about to take it out (the flag 1, now 0).
    Hide,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Act {
    /// Nothing of ours: the game's own call, if the event has one, runs.
    None,
    /// The game's movie call skipped; its flag still written.
    Skip,
    /// The movie taken out of the scene.
    Withdraw,
    /// The movie put back in the scene.
    Restore,
}

impl CursorMovie {
    fn on(self, event: Event) -> (CursorMovie, Act) {
        use CursorMovie::{AsFlag, Covered, Left};
        match (self, event) {
            (AsFlag, Event::MenuShown(World { running: true, cursor })) => {
                (Covered, if cursor { Act::Withdraw } else { Act::None })
            }
            (Left, Event::MenuShown(World { running: true, .. })) => (Covered, Act::None),
            (state, Event::MenuShown(_)) => (state, Act::None),
            (Covered, Event::MenuClosed(World { running: true, cursor })) => {
                (AsFlag, if cursor { Act::Restore } else { Act::None })
            }
            (Covered, Event::MenuClosed(World { running: false, cursor: true })) => (Left, Act::None),
            (Covered, Event::MenuClosed(World { running: false, cursor: false })) => (AsFlag, Act::None),
            (state, Event::MenuClosed(_)) => (state, Act::None),
            (Covered, Event::Show) => (Covered, Act::Skip),
            (AsFlag | Left, Event::Show) => (AsFlag, Act::None),
            (AsFlag, Event::Hide) => (AsFlag, Act::None),
            (Covered, Event::Hide) => (Covered, Act::Skip),
            (Left, Event::Hide) => (AsFlag, Act::Skip),
        }
    }
}

static MOVIE: GameCell<CursorMovie> = GameCell::new(CursorMovie::AsFlag);

fn step(thread: &Thread, event: Event) -> Act {
    let (next, act) = MOVIE.get(thread).on(event);
    MOVIE.set(thread, next);
    act
}

pub fn on_match_start(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hud) = Hud::at_match_start(ctx) {
        hud.start_over();
    }
    Flow::Continue
}

pub fn on_cursor_shown(ctx: &mut Context<'_>) -> Flow<HudCursorOnCursorShownExit> {
    match step(ctx.thread(), Event::Show) {
        Act::Skip => Flow::Exit(HudCursorOnCursorShownExit::Kept),
        _ => Flow::Continue,
    }
}

pub fn on_cursor_hidden(ctx: &mut Context<'_>) -> Flow<HudCursorOnCursorHiddenExit> {
    match step(ctx.thread(), Event::Hide) {
        Act::Skip => Flow::Exit(HudCursorOnCursorHiddenExit::FlagOnly),
        _ => Flow::Continue,
    }
}

pub fn on_menu_show(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(cursor) = MainMenu::at_show(ctx).and_then(|menu| menu.cursor()) {
        let world = World { running: cursor.running(), cursor: cursor.shown() };
        if step(ctx.thread(), Event::MenuShown(world)) == Act::Withdraw {
            cursor.withdraw();
        }
    }
    Flow::Continue
}

pub fn on_menu_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(menu) = MainMenu::at_close(ctx)
        && menu.shown()
        && let Some(cursor) = menu.cursor()
    {
        let world = World { running: cursor.running(), cursor: cursor.shown() };
        if step(ctx.thread(), Event::MenuClosed(world)) == Act::Restore {
            cursor.restore();
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game as the patch sees it: the movie's copies in the scene's list,
    /// the flag, the menu, the match. A hide of a movie the list has not
    /// fails, as the render thread's erase does.
    #[derive(Clone, Copy, Debug)]
    struct Game {
        state: CursorMovie,
        copies: u32,
        flag: bool,
        menu: bool,
        running: bool,
    }

    impl Game {
        fn start() -> Game {
            Game { state: CursorMovie::AsFlag, copies: 0, flag: false, menu: false, running: true }
        }

        fn take_out(&mut self) {
            assert!(self.copies > 0, "a hide of a movie not in the scene: {self:?}");
            self.copies -= 1;
        }

        fn act(&mut self, event: Event) {
            let (next, act) = self.state.on(event);
            self.state = next;
            match (event, act) {
                (Event::Show, Act::None) | (Event::MenuClosed(_), Act::Restore) => self.copies += 1,
                (Event::Hide, Act::None) | (Event::MenuShown(_), Act::Withdraw) => self.take_out(),
                (Event::Show | Event::Hide, Act::Skip) | (Event::MenuShown(_) | Event::MenuClosed(_), Act::None) => {}
                (event, act) => panic!("{act:?} for {event:?}"),
            }
        }

        /// The movie is where the state says: off while not as the flag,
        /// else in the scene once exactly while the flag is set.
        fn consistent(&self) {
            let expected = match self.state {
                CursorMovie::AsFlag => u32::from(self.flag),
                CursorMovie::Covered | CursorMovie::Left => 0,
            };
            assert_eq!(self.copies, expected, "{self:?}");
        }

        /// show_cursor(shown): nothing when the flag already is so.
        fn cursor(&mut self, shown: bool) {
            if self.flag != shown {
                self.act(if shown { Event::Show } else { Event::Hide });
                self.flag = shown;
            }
            self.consistent();
        }

        fn menu(&mut self, shown: bool) {
            let world = World { running: self.running, cursor: self.flag };
            if shown {
                self.act(Event::MenuShown(world));
            } else if self.menu {
                self.act(Event::MenuClosed(world));
            }
            self.menu = shown;
            self.consistent();
        }

        /// A match's end: the world no longer the active scene.
        fn end(&mut self) {
            self.running = false;
        }

        /// initialize_match: the cursor hidden, then the match runs.
        fn match_start(&mut self) {
            self.cursor(false);
            self.running = true;
        }
    }

    #[test]
    fn esc_then_leave_with_the_cursor_shown() {
        let mut game = Game::start();
        game.cursor(true);
        game.menu(true);
        assert_eq!(game.copies, 0);
        game.end();
        game.menu(false);
        game.match_start();
        game.cursor(true);
        assert_eq!(game.copies, 1);
    }

    #[test]
    fn a_death_under_the_menu_then_the_match_end() {
        let mut game = Game::start();
        game.menu(true);
        game.cursor(true);
        assert_eq!(game.copies, 0);
        game.end();
        game.menu(false);
        game.match_start();
        assert_eq!((game.copies, game.state), (0, CursorMovie::AsFlag));
    }

    #[test]
    fn a_hide_under_the_menu() {
        for kept in [false, true] {
            let mut game = Game::start();
            game.cursor(!kept);
            game.menu(true);
            game.cursor(true);
            game.cursor(false);
            game.menu(false);
            game.cursor(true);
            assert_eq!(game.copies, 1);
        }
    }

    #[test]
    fn resume_puts_it_back() {
        let mut game = Game::start();
        game.cursor(true);
        game.menu(true);
        game.menu(true);
        assert_eq!(game.copies, 0);
        game.menu(false);
        assert_eq!(game.copies, 1);
        game.cursor(false);
    }

    /// No sequence of the game's steps hides a movie the scene has not, nor
    /// shows one twice: every path of seven steps from the start.
    #[test]
    fn no_sequence_hides_a_missing_movie() {
        type Step = fn(&mut Game);
        const STEPS: [Step; 6] =
            [|g| g.cursor(true), |g| g.cursor(false), |g| g.menu(true), |g| g.menu(false), Game::end, Game::match_start];
        fn walk(game: Game, depth: u32) {
            if depth > 0 {
                for step in STEPS {
                    let mut next = game;
                    step(&mut next);
                    walk(next, depth - 1);
                }
            }
        }
        walk(Game::start(), 7);
    }
}
