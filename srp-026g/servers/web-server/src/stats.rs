//! Public aggregate counts. This reads state without advancing matchmaking.
use session::SessionStore;

use crate::{ContentType, Game, LiveKind, LiveMatch, Response};

pub(super) fn response(store: &SessionStore, game: &Game) -> Response {
    let online = store.players_present();
    let registered = store.registered_accounts();
    let matches = game.matches.active_matches();
    let rooms = store.room_counts();
    let live = game.matches.live_matches();
    let last_match_ends_in = last_ends_in(live.iter());
    let normal = || live.iter().filter(|each| each.kind == LiveKind::Normal);
    let normal_matches = normal().count();
    let last_normal_match_ends_in = last_ends_in(normal());
    Response {
        status: "200 OK",
        content_type: ContentType::Json,
        // Only integers enter the JSON; no player or account data is exposed.
        // Fields are only added, at the end: the launcher and older tooling
        // read the ones before.
        body: format!(
            "{{\"online\":{online},\"registered\":{registered},\"matches\":{matches},\"rooms\":{},\
             \"in_rooms\":{},\"queued\":{},\"last_match_ends_in\":{last_match_ends_in},\
             \"normal_matches\":{normal_matches},\
             \"last_normal_match_ends_in\":{last_normal_match_ends_in}}}",
            rooms.rooms, rooms.in_rooms, rooms.queued
        ),
    }
}

/// Whole seconds, rounded up, until the last of the matches ends at the
/// latest ([`LiveMatch::ends_within`]); 0 when none runs. A release plans
/// its maintenance from it (`release/release.sh`): over every match
/// (`last_match_ends_in`), or over the normal ones alone
/// (`last_normal_match_ends_in`, [`LiveKind::Normal`]) with
/// `--end-bot-and-private-matches`.
fn last_ends_in<'a>(live: impl Iterator<Item = &'a LiveMatch>) -> u128 {
    live.map(LiveMatch::ends_within)
        .max()
        .map_or(0, |most| most.as_millis().div_ceil(1000))
}
