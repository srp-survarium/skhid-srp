//! [body physics S3] The walk grid's client walker as the shared model's world (`client_body::Walker`): the game's shapes with the
//! grid body's controller settings, through `WalkGrid::kept_tick` and `WalkGrid::kept_fly_step`. The reckoning (`body::client_steps`)
//! and the bench's stand-in move their bodies through it.

use mapdata::controller::jump::{FlightStep, Flying};
use mapdata::controller::tick::{Controller, StepNotes, TickInput};
use mapdata::WalkGrid;

/// A grid with the client's walker (`WalkGrid::has_client_walker`), as the shared model's world.
pub(crate) struct GridWalker<'g>(pub(crate) &'g WalkGrid);

impl client_body::Walker for GridWalker<'_> {
    fn step(&self, c: &mut Controller, input: TickInput) -> StepNotes {
        self.0.kept_tick(c, input).expect("the shared model's world needs the grid's client walker") // [the shared model's world]
    }

    fn fly(&self, c: &mut Controller, flying: &mut Flying, heading: (f32, f32), air: (f32, f32)) -> FlightStep {
        self.0.kept_fly_step(c, flying, heading, air).expect("the shared model's world needs the grid's client walker") // [the shared model's world]
    }
}
