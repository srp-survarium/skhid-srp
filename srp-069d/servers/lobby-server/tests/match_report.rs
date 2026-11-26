//! Real `53` flushes of training matches (logs of the 0.69d client, bytes after the message id; no names in them) through
//! the decoder and `match_report::local_report`. See docs/notes/match-stats.md.

use lobby_server::match_report::{local_report, LocalReport};
use protocol::c2s::{Event53, Request};

fn flush(hex: &str) -> Vec<Event53> {
    let b: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
    match Request::decode(0x53, &b).unwrap() {
        Request::Events(e) => e,
        o => panic!("{o:?}"),
    }
}

/// (flush, won, finished)
const REAL: &[(&str, bool, bool)] = &[
    // login flush (events 1, 2): no match
    ("02000000010075da0000000000000000020000000000000000000000", false, false),
    // the player left early: event 2 only, time = client uptime in seconds (0x353 = 851)
    ("01000000020047000000000053030000", false, false),
    // a played bots match he won: event 5 (value 1), then event 2 (lobby loaded)
    ("0200000005001f000100000026080000020047000000000027080000", true, true),
    ("0200000005001b0001000000e70600000200470000000000e8060000", true, true),
    // the second flush one second after our 97: the client's own 97 handler echoes 5 / 6 (0x18B2E5); it is not a match
    ("010000000500000001000000e9060000", true, true),
    // a loss marker (the echo of a 97 with the other team as winner)
    ("01000000060000000100000053030000", false, true),
];

#[test]
fn real_flushes_report_only_the_outcome() {
    for (hex, won, finished) in REAL {
        let r = local_report(&flush(hex));
        assert_eq!((r.won, r.finished), (*won, *finished), "{hex}");
    }
}

#[test]
fn no_real_flush_carries_anything_but_markers() {
    // every row of every real flush is event 1, 2, 5 or 6 with value 0 / 1: no kills, deaths, score
    for (hex, _, _) in REAL {
        for e in flush(hex) {
            assert!(matches!(e.id, 1 | 2 | 5 | 6), "{e:?}");
            assert!(e.value <= 1, "{e:?}");
        }
    }
}

#[test]
fn single_player_ids_and_long_flushes() {
    let mk = |id| Event53 { id, x: 0, value: 1, time: 0 };
    assert_eq!(local_report(&[mk(0x3D)]), LocalReport { won: true, finished: true, ids: vec![0x3D] });
    let lost = local_report(&[mk(0x3E)]);
    assert_eq!((lost.won, lost.finished), (false, true));
    // a flush holds up to 50 rows: the marker in row 48 is still seen
    let mut v: Vec<_> = (0..47).map(|_| mk(2)).collect();
    v.push(mk(5));
    assert!(local_report(&v).won);
    assert_eq!(local_report(&[]), LocalReport::default());
}
