//! 0xE4's per-tick trailer (`survarium::v026g::match_msgs::static_info::
//! ticks_trailer`): after the LZMA2 stream, the match's tick in ms, the
//! protocol's version and the magic `SRPT`. A server sends it only to a
//! game that announced the protocol (its 0xD8 byte's bit 1).

use sync_core::{Grid, TickLen};

/// The trailer's bytes.
pub const LEN: usize = 6;
const MAGIC: [u8; 4] = *b"SRPT";

/// The grid a static info's payload ends with, if it has the trailer (of
/// this protocol's version, and a tick the game runs).
pub fn parse(payload: &[u8]) -> Option<Grid> {
    let at = payload.len().checked_sub(LEN)?;
    let trailer = payload.get(at..)?;
    if trailer.get(2..)? != MAGIC || trailer[1] != sync_core::wire::VERSION {
        return None;
    }
    TickLen::from_ms(trailer[0].into()).map(Grid::of)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trailer_of_the_server_and_nothing_else() {
        let stock = [1u8, 1, 0, 0, 0x55, 0];
        assert_eq!(parse(&stock), None);
        let mut ticks = stock.to_vec();
        ticks.extend([20, sync_core::wire::VERSION, b'S', b'R', b'P', b'T']);
        assert_eq!(parse(&ticks), Some(Grid::of(TickLen::Ms20)));
        ticks[6] = 40;
        assert_eq!(parse(&ticks), None);
        ticks[6] = 50;
        ticks[7] = 9;
        assert_eq!(parse(&ticks), None);
        assert_eq!(parse(b"SRPT"), None);
    }
}
