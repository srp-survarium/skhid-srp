//! Match-scoped injected input counts. The worker observes desktop events,
//! the game thread alone owns the connection, clock and report cadence.
#![forbid(unsafe_code)]
use crate::vostok::files::tick_count;
use crate::vostok::packet_recovery::Connection;
use crate::vostok::{Context, Flow, GameCell, NoExit, Thread, input_report as watch};
use srp_validate::input_report::{Counts, Summary, When};
use srp_validate::modules::Minute;
use srp_validate::report::Report;

type Key = (usize, u32);

#[derive(Clone, Copy)]
enum State {
    Idle,
    Playing {
        key: Key,
        at: u32,
        next: Option<Minute>,
        sent: Counts,
    },
    Ended(Key),
}

#[derive(Clone, Copy)]
enum Event {
    Load(Key),
    Frame(Option<Key>),
    Close(Key),
}

trait Monitor {
    fn start(&self);
    fn sample(&self) -> Option<Counts>;
    fn stop(&self) -> Option<Counts>;
    fn send(&self, report: Summary) -> bool;
}

impl State {
    fn step(self, event: Event, now: u32, monitor: &impl Monitor) -> Self {
        match (self, event) {
            (Self::Playing { key, .. } | Self::Ended(key), Event::Load(same)) if key == same => self,
            (_, Event::Load(key)) => {
                monitor.start();
                Self::Playing {
                    key,
                    at: now,
                    next: Some(Minute::FIRST),
                    sent: Counts::default(),
                }
            }
            (Self::Playing { key, .. }, Event::Close(closing)) if key == closing => {
                if let Some(counts) = monitor.stop() {
                    monitor.send(Summary {
                        moment: When::Ended,
                        counts,
                    });
                }
                Self::Ended(key)
            }
            (Self::Playing { key, .. }, Event::Frame(current)) if current != Some(key) => {
                monitor.stop();
                Self::Idle
            }
            (
                Self::Playing {
                    key,
                    at,
                    next: Some(minute),
                    mut sent,
                },
                Event::Frame(_),
            ) if now.wrapping_sub(at) >= 60_000 => {
                if let Some(counts) = monitor.sample()
                    && counts != sent
                    && monitor.send(Summary {
                        moment: When::Minute(minute),
                        counts,
                    })
                {
                    sent = counts;
                }
                Self::Playing {
                    key,
                    at: now,
                    next: minute.next(),
                    sent,
                }
            }
            _ => self,
        }
    }
}

struct Match<'a> {
    thread: &'a Thread,
    connection: Option<Connection<'a>>,
}

impl Monitor for Match<'_> {
    fn start(&self) {
        watch::start(self.thread);
    }
    fn sample(&self) -> Option<Counts> {
        watch::sample(self.thread)
    }
    fn stop(&self) -> Option<Counts> {
        watch::stop(self.thread)
    }
    fn send(&self, report: Summary) -> bool {
        self.connection.as_ref().is_some_and(|c| c.tell(&Report::Input(report)))
    }
}

static STATE: GameCell<State> = GameCell::new(State::Idle);

impl Match<'_> {
    fn apply(&self, event: Event) {
        let state = STATE.get(self.thread);
        STATE.set(self.thread, state.step(event, tick_count(), self));
    }
}

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(c) = Connection::at_loaded(ctx) {
        let key = c.key();
        Match {
            thread: ctx.thread(),
            connection: Some(c),
        }
        .apply(Event::Load(key));
    }
    Flow::Continue
}

pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if matches!(STATE.get(ctx.thread()), State::Playing { .. }) {
        let connection = Connection::at_game_tick(ctx);
        let key = connection.as_ref().map(Connection::key);
        Match {
            thread: ctx.thread(),
            connection,
        }
        .apply(Event::Frame(key));
    }
    Flow::Continue
}

pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(c) = Connection::at_close(ctx) {
        let key = c.key();
        Match {
            thread: ctx.thread(),
            connection: Some(c),
        }
        .apply(Event::Close(key));
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::{Cell, RefCell};

    #[derive(Default)]
    struct Probe {
        counts: Cell<Option<Counts>>,
        starts: Cell<usize>,
        room: Cell<bool>,
        reports: RefCell<std::vec::Vec<Summary>>,
    }

    impl Monitor for Probe {
        fn start(&self) {
            self.starts.set(self.starts.get() + 1);
            self.counts.set(Some(Counts::default()));
        }
        fn sample(&self) -> Option<Counts> {
            self.counts.get()
        }
        fn stop(&self) -> Option<Counts> {
            self.counts.take()
        }
        fn send(&self, report: Summary) -> bool {
            if self.room.get() {
                self.reports.borrow_mut().push(report);
            }
            self.room.get()
        }
    }

    #[test]
    fn resync_silence_pool_pressure_and_close() {
        let probe = Probe::default();
        let key = (17, 23);
        let mut state = State::Idle;
        let mut step = |event, elapsed| {
            state = state.step(event, u32::MAX.wrapping_add(elapsed), &probe);
        };
        step(Event::Load(key), 0);
        step(Event::Load(key), 100); // resync
        step(Event::Frame(Some(key)), 60_000);
        let counts = Counts {
            key_down: 3,
            button_down: 5,
            mouse_move: 7,
        };
        probe.counts.set(Some(counts));
        step(Event::Frame(Some(key)), 120_000); // send queue full
        probe.room.set(true);
        step(Event::Frame(Some(key)), 180_000);
        step(Event::Frame(Some(key)), 240_000); // unchanged
        step(Event::Close((17, 24)), 250_000);
        step(Event::Close(key), 251_000);
        step(Event::Close(key), 252_000);
        assert_eq!(probe.starts.get(), 1);
        assert_eq!(probe.counts.get(), None);
        assert_eq!(
            *probe.reports.borrow(),
            [
                Summary {
                    moment: When::Minute(Minute::FIRST.next().unwrap().next().unwrap()),
                    counts
                },
                Summary {
                    moment: When::Ended,
                    counts
                }
            ]
        );
    }

    #[test]
    fn lost_connection_and_unavailable_hooks_do_not_report_zero() {
        let probe = Probe::default();
        probe.room.set(true);
        let a = (3, 4);
        let b = (3, 5);
        let state = State::Idle.step(Event::Load(a), 0, &probe);
        let state = state.step(Event::Frame(None), 1, &probe);
        assert_eq!(probe.sample(), None);
        let state = state.step(Event::Load(b), 2, &probe);
        probe.counts.set(None);
        state.step(Event::Close(b), 3, &probe);
        assert!(probe.reports.borrow().is_empty());
    }
}
