//! The client's clock, steered by the client itself (`netcode_port_069.md`
//! 5.3, phase 3; 0.69's `netsync_send_window_in_ms` rule, 0x2001AF210 and
//! the windowed minimum 0x2001B1AC0).
//!
//! Every server update says how early the client's own updates came
//! ([`crate::wire::Arrival`]: its update's tick less the match clock when it
//! arrived; negative: late). The client wants its updates to arrive
//! [`SEND_WINDOW_MS`] before their tick on the match clock: then the server
//! never waits for it and never moves its inputs. So it runs ahead of the
//! match clock by its one-way latency and the window, whatever its ping.
//!
//! The machine keeps a window of the latest arrivals; its **least** one (the
//! worst arrival, as 0.69) decides. Below the window by more than the dead
//! band, or far above it, the clock is corrected: by the game's own gradual
//! correction (`floating_timer::set_offset` 0x1EE9E0, what 0xE7 did from
//! the server), or set outright past [`JUMP_MS`]. While a correction runs
//! the arrivals are of a clock still on its way: they are not looked at,
//! and the window starts anew when it has run.

/// How early the client's updates should come, at the least (0.69's
/// default `netsync_send_window_in_ms`, 2, and a frame's slack).
pub const SEND_WINDOW_MS: i32 = 5;

/// A worst arrival this close to the window is left as it is.
pub const DEAD_BAND_MS: i32 = 10;

/// A clock this far off is set outright, not corrected over time.
pub const JUMP_MS: i32 = 250;

/// The arrivals in the window: about two seconds of server updates at 50 ms.
pub const WINDOW: usize = 40;

/// The arrivals the window needs before it decides.
pub const MIN_SAMPLES: usize = 10;

/// A correction from far above the window (running needlessly far ahead)
/// is made only for this much more than the dead band: a clock that is a
/// little early costs nothing, one that is late costs moved inputs.
pub const EARLY_SLACK_MS: i32 = 40;

/// What the client does to its clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Nothing.
    Keep,
    /// The game's gradual correction by so many ms (positive: forward).
    Shift { ms: i32 },
    /// Set the clock so many ms forward (or back) at once.
    Jump { ms: i32 },
}

/// The machine's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Collecting arrivals.
    Steady,
    /// A correction runs until the client's clock (ms) is at `until`.
    Correcting { until: u32 },
}

/// The client's steering: its state and the window of arrivals.
#[derive(Clone, Copy, Debug)]
pub struct Steering {
    state: Clock,
    window: [i16; WINDOW],
    len: usize,
    next: usize,
}

impl Default for Steering {
    fn default() -> Self {
        Self::new()
    }
}

impl Steering {
    pub const fn new() -> Steering {
        Steering {
            state: Clock::Steady,
            window: [0; WINDOW],
            len: 0,
            next: 0,
        }
    }

    pub fn state(&self) -> Clock {
        self.state
    }

    /// The least arrival in the window, if it has any.
    pub fn worst(&self) -> Option<i16> {
        self.window[..self.len].iter().copied().min()
    }

    fn forget(&mut self) {
        self.len = 0;
        self.next = 0;
    }

    /// A server update came, its arrival `arrival`, the client's clock at
    /// `now` (ms). `factor_permille`: the share of real time, per mille,
    /// the game's gradual correction makes up (`match_msgs::clock::factor`
    /// of its patch: 600 with patch 15). Total over every event.
    pub fn sample(&mut self, arrival: Option<i16>, now: u32, factor_permille: u32) -> Order {
        if let Clock::Correcting { until } = self.state {
            // Wrap-safe: `now` has reached `until`.
            if now.wrapping_sub(until) as i32 >= 0 {
                self.state = Clock::Steady;
                self.forget();
            } else {
                return Order::Keep;
            }
        }
        let Some(arrival) = arrival else {
            return Order::Keep;
        };
        self.window[self.next] = arrival;
        self.next = (self.next + 1) % WINDOW;
        self.len = (self.len + 1).min(WINDOW);
        if self.len < MIN_SAMPLES {
            return Order::Keep;
        }
        let worst = i32::from(self.worst().unwrap_or(arrival));
        // Forward when it comes too late; back only when far too early.
        let ms = SEND_WINDOW_MS - worst;
        let order = if ms.abs() >= JUMP_MS {
            Order::Jump { ms }
        } else if !(-(DEAD_BAND_MS + EARLY_SLACK_MS)..=DEAD_BAND_MS).contains(&ms) {
            Order::Shift { ms }
        } else {
            return Order::Keep;
        };
        // How long the game takes for it, from now on its own clock.
        let takes = match order {
            Order::Shift { ms } => ms.unsigned_abs() * 1000 / factor_permille.max(10),
            _ => 0,
        };
        // And the updates of the old clock still on their way: a window.
        self.state = Clock::Correcting {
            until: now
                .wrapping_add(takes)
                .wrapping_add(WINDOW as u32 * 50 / 4),
        };
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// A client whose updates arrive `offset + jitter` ms early, its clock
    /// steered by its orders, over `ticks` updates: the arrivals at the end.
    fn steer(offset: i32, jitter: &[i32], ticks: u32) -> (i32, Vec<Order>) {
        let mut steering = Steering::new();
        let mut early = offset;
        let mut orders = Vec::new();
        // The clock takes a shift over time: what is still to come.
        let mut owed = 0f32;
        let factor: f32 = 0.6;
        for n in 0..ticks {
            let now = n * 50;
            let step = (50.0 * factor).min(owed.abs()).copysign(owed);
            owed -= step;
            early += step.round() as i32;
            let arrival = early + jitter[n as usize % jitter.len()];
            match steering.sample(Some(arrival as i16), now, 600) {
                Order::Keep => {}
                order @ Order::Shift { ms } => {
                    orders.push(order);
                    owed += ms as f32;
                }
                order @ Order::Jump { ms } => {
                    orders.push(order);
                    early += ms;
                }
            }
        }
        (early, orders)
    }

    #[test]
    fn a_late_clock_is_brought_ahead_until_its_worst_update_is_in_time() {
        // 80 ms late, 0..30 ms of jitter.
        let (early, orders) = steer(-80, &[0, 10, 30, 5, 20], 400);
        assert!(!orders.is_empty());
        assert!(
            (SEND_WINDOW_MS..=SEND_WINDOW_MS + DEAD_BAND_MS).contains(&early),
            "worst at {early}: {orders:?}"
        );
    }

    #[test]
    fn a_far_off_clock_jumps_once_and_a_steady_one_is_left_alone() {
        let (early, orders) = steer(-600, &[0], 200);
        assert_eq!(orders[0], Order::Jump { ms: 605 });
        assert_eq!(early, SEND_WINDOW_MS);
        let (_, orders) = steer(10, &[0, 3, 6], 400);
        assert_eq!(orders, []);
    }

    #[test]
    fn a_clock_a_little_early_stays_and_one_far_too_early_comes_back() {
        let (_, orders) = steer(40, &[0], 400);
        assert_eq!(orders, []);
        let (early, orders) = steer(120, &[0], 400);
        assert!(matches!(orders[0], Order::Shift { ms } if ms < 0));
        assert!(early <= SEND_WINDOW_MS + DEAD_BAND_MS + EARLY_SLACK_MS);
    }

    #[test]
    fn it_never_oscillates_on_a_jittery_line() {
        // 0..60 ms of jitter: the worst decides, the orders stop.
        let (_, orders) = steer(-30, &[0, 60, 15, 45, 30, 5, 55], 2_000);
        assert!(orders.len() <= 3, "{orders:?}");
    }

    #[test]
    fn an_update_without_an_arrival_says_nothing_and_a_running_correction_waits() {
        let mut steering = Steering::new();
        for n in 0..MIN_SAMPLES as u32 {
            assert_eq!(steering.sample(None, n * 50, 600), Order::Keep);
        }
        assert_eq!(steering.worst(), None);
        for n in 0..MIN_SAMPLES as u32 - 1 {
            assert_eq!(steering.sample(Some(-100), n * 50, 600), Order::Keep);
        }
        assert_eq!(
            steering.sample(Some(-100), 500, 600),
            Order::Shift { ms: 105 }
        );
        assert!(matches!(steering.state(), Clock::Correcting { .. }));
        assert_eq!(steering.sample(Some(-100), 550, 600), Order::Keep);
    }
}
