//! Byte-exact datagrams and the rules one at a time.

use std::time::{Duration, Instant};

use super::client_model::ClientModel;
use super::{client, connect_lobby, connect_match, flow};
use crate::ack;
use crate::wire::{self, Part};
use crate::{Config, Direction, Event, PacketLog, SendError, State, Table, Transport};

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

/// The messages of a server datagram: `(id, x, payload length)`.
fn messages(table: Table, datagram: &[u8]) -> Vec<(u8, Option<u8>, usize)> {
    wire::parse(table, Direction::ToClient, datagram, 3)
        .unwrap()
        .messages
        .iter()
        .map(|message| (message.id, message.x, message.payload.len()))
        .collect()
}

// ------------------------------------------------ transport.md §9 and §7

#[test]
fn worked_example_a_the_keepalive() {
    // The server has received client seqs 5, 4 and 2; its seq is 9.
    let start = Instant::now();
    let mut transport = Transport::new(Table::Match, start);
    assert!(transport.receive(&[2, 0xFF, 0x00, 0xFC], start).is_empty());
    assert_eq!(transport.poll(start), [vec![0, 2, 0x00, 0xFC]]);
    for seq in 1..=8u8 {
        let at = start + ms(100) * u32::from(seq);
        assert_eq!(transport.poll(at), [vec![seq, 2, 0x00, 0xFC]]);
    }
    transport.receive(&[4, 0xFF, 0x00, 0xFC], start + ms(850));
    transport.receive(&[5, 0xFF, 0x00, 0xFC], start + ms(850));
    assert_eq!(
        transport.poll(start + ms(900)),
        [vec![0x09, 0x05, 0x50, 0xFC]]
    );
}

#[test]
fn worked_example_b_the_ports_after_the_connect() {
    // First client datagram `00 FF 00 D8 <session> 00`, then E3 {1236, 1236}.
    let (mut transport, now) = connect_match();
    transport.send(0xE3, &[0xD4, 0x04, 0xD4, 0x04]).unwrap();
    assert_eq!(
        transport.poll(now),
        [vec![0x00, 0x00, 0x00, 0xE3, 0xD4, 0x04, 0xD4, 0x04]]
    );
}

#[test]
fn worked_example_c_a_multi_datagram_with_one_tail_byte() {
    let (mut transport, now) = connect_match();
    // Three unique messages first, so the next unique id is 3.
    for n in 0..3u8 {
        transport.send(0xE6, &[n; 4]).unwrap();
    }
    assert_eq!(
        transport.poll(now),
        [vec![
            0x00, 0x00, 0x01, // seq 0, rseq 0, multi
            0xE6, 0, 0, 0, 0, 0, 0xE6, 1, 1, 1, 1, 1, 0xE6, 2, 2, 2, 2, 2,
        ]]
    );
    // Client seq 8 arrives (1..7 were lost) and acks server seq 0: the
    // window must reach back 8 to keep acking client seq 0, so one tail byte.
    transport.receive(&flow(8, 0, &[]), now);
    transport.send(0xE9, &[0x11, 0x22]).unwrap();
    transport.send(0xF6, &[0xAB; 300]).unwrap();
    let mut expected = vec![
        0x01, 0x08, 0x80, 0xE9, 0x11, 0x22, 0xF6, 0x03, 0xFF, 0x2C, 0x01,
    ];
    expected.extend([0xAB; 300]);
    // t1: b(8) in bit 6, the multi flag in bit 0.
    expected.push(0x41);
    assert_eq!(transport.poll(now), [expected]);
}

#[test]
fn the_lobby_sign_in_datagram_is_parsed_as_the_first_message() {
    // `2E <x> <u32 session> <bool>`: connect_lobby asserts the event.
    let (transport, now) = connect_lobby();
    assert_eq!(transport.state(), State::Connected);
    assert!(!transport.is_gone(now));
}

// ----------------------------------------------------------- the formats

#[test]
fn lobby_messages_carry_their_unique_or_order_id() {
    let (mut transport, now) = connect_lobby();
    transport.send(0x80, &[0x0A]).unwrap(); // unique 0
    transport.send(0x85, &[1, 2]).unwrap(); // order 0
    transport.send(0x87, &[]).unwrap(); // order 1
    transport.send(0x82, &[9]).unwrap(); // unique 1
    transport.send(0x9F, &[7, 7, 7]).unwrap(); // plain: no x
    assert_eq!(
        transport.poll(now),
        [vec![
            0x00, 0x00, 0x01, // multi
            0x80, 0, 1, 0x0A, 0x85, 0, 2, 1, 2, 0x87, 1, 0, 0x82, 1, 1, 9, 0x9F, 3, 7, 7, 7,
        ]]
    );
}

#[test]
fn a_large_ordered_message_goes_in_parts_under_one_order_id() {
    let (mut transport, now) = connect_lobby();
    transport.send(0x85, &[1]).unwrap();
    let payload: Vec<u8> = (0..1200u32).map(|i| (i * 7) as u8).collect();
    transport.send(0x89, &payload).unwrap();
    transport.send(0x8B, &[2]).unwrap();
    let datagrams = transport.poll(now);
    assert_eq!(datagrams.len(), 4);
    assert_eq!(datagrams[0], [0, 0, 0, 0x85, 0, 1]);
    assert_eq!(datagrams[1][..8], [1, 0, 0, 0xFF, 0x89, 1, 0, 3]);
    assert_eq!(datagrams[1].len(), 3 + 5 + 496);
    assert_eq!(datagrams[2][..7], [2, 0, 0, 0xFF, 0x89, 1, 1]);
    assert_eq!(datagrams[2].len(), 3 + 4 + 497);
    // The last part shares a multi datagram with the next message, so it
    // has a length there: 1200 - 496 - 497 = 207.
    assert_eq!(
        datagrams[3][..9],
        [3, 0, 1, 0xFF, 0x89, 1, 2, 207, payload[993]]
    );
    assert_eq!(datagrams[3][8 + 207..], [0x8B, 2, 1, 2]);
    let mut data = datagrams[1][8..].to_vec();
    data.extend(&datagrams[2][7..]);
    data.extend(&datagrams[3][8..8 + 207]);
    assert_eq!(data, payload);
    for datagram in &datagrams {
        ClientModel::check(Table::Lobby, 0, datagram).unwrap();
    }
}

#[test]
fn a_full_datagram_carries_fewer_tail_bytes() {
    let (mut transport, now) = connect_lobby();
    for seq in 1..=40 {
        transport.receive(&flow(seq, 0xFF, &[]), now);
    }
    // The largest message that is not split: 499 bytes with its x byte.
    transport.send(0x80, &[0x33; 499]).unwrap();
    transport.send(0x82, &[0x44; 500]).unwrap();
    let datagrams = transport.poll(now);
    assert_eq!(datagrams.len(), 3);
    // 40 client seqs to ack want 5 tail bytes; only 4 fit.
    assert_eq!(datagrams[0].len(), wire::MAX_DATAGRAM);
    assert_eq!(ack::decode(&datagrams[0]).unwrap().tail, 4);
    assert_eq!(
        messages(Table::Lobby, &datagrams[0]),
        [(0x80, Some(0), 499)]
    );
    // One byte more is two parts.
    let parsed = wire::parse(Table::Lobby, Direction::ToClient, &datagrams[1], 0).unwrap();
    assert_eq!(
        parsed.messages[0].part,
        Some(Part {
            index: 0,
            count: Some(2)
        })
    );
    assert_eq!(parsed.messages[0].payload.len(), 496);
    assert_eq!(ack::decode(&datagrams[2]).unwrap().tail, 5);
    for datagram in &datagrams {
        assert!(datagram.len() <= wire::MAX_DATAGRAM);
        ClientModel::check(Table::Lobby, 0, datagram).unwrap();
    }
}

#[test]
fn player_inputs_need_the_roster_and_two_players() {
    let (mut transport, now) = connect_match();
    let mut inputs = vec![0x10, 0b0000_0101, 0, 0b0000_1000];
    inputs.extend([0xEE; 24]);
    assert_eq!(
        transport.send(0xED, &inputs),
        Err(SendError::BadPlayerInputs(0xED))
    );
    transport.set_roster_size(20);
    transport.send(0xED, &inputs).unwrap();
    // EE has a second time byte.
    let mut precise = vec![0x10, 0x20, 0b11, 0, 0];
    precise.extend([0xDD; 16]);
    transport.send(0xEE, &precise).unwrap();
    // One player, and a mask that disagrees with the data.
    let mut one = vec![0x10, 0b1, 0, 0];
    one.extend([0; 8]);
    assert_eq!(
        transport.send(0xED, &one),
        Err(SendError::BadPlayerInputs(0xED))
    );
    assert_eq!(
        transport.send(0xED, &inputs[..20]),
        Err(SendError::BadPlayerInputs(0xED))
    );
    inputs.push(0);
    assert_eq!(
        transport.send(0xED, &inputs),
        Err(SendError::BadLength {
            id: 0xED,
            expected: 28,
            got: 29
        })
    );
    // No length bytes in the multi datagram: the client works them out.
    let datagrams = transport.poll(now);
    assert_eq!(datagrams.len(), 1);
    assert_eq!(datagrams[0].len(), 3 + 1 + 28 + 1 + 21);
    assert_eq!(
        messages(Table::Match, &datagrams[0]),
        [(0xED, None, 28), (0xEE, None, 21)]
    );
    ClientModel::check(Table::Match, 3, &datagrams[0]).unwrap();
}

// ------------------------------------------------------------- refusals

#[test]
fn send_refuses_ids_and_sizes_the_client_cannot_take() {
    let (mut lobby, now) = connect_lobby();
    assert_eq!(lobby.send(0x81, &[]), Err(SendError::Forbidden(0x81)));
    assert_eq!(lobby.send(0xA0, &[1]), Err(SendError::Forbidden(0xA0)));
    assert_eq!(lobby.send(0x86, &[]), Err(SendError::UnknownId(0x86)));
    assert_eq!(lobby.send(0xA7, &[]), Err(SendError::UnknownId(0xA7)));
    for id in (0x00..0x80).chain(0xAB..=0xFF) {
        assert_eq!(
            lobby.send(id, &[]),
            Err(SendError::UnknownId(id)),
            "{id:#04x}"
        );
    }
    // A plain message cannot be split, and a split one has at most 255 parts.
    assert_eq!(lobby.send(0x9F, &[0; 501]), Err(SendError::TooLarge(501)));
    lobby.send(0x9F, &[0; 500]).unwrap();
    let most = 496 + 254 * 497;
    let (mut other, _) = connect_lobby();
    assert_eq!(
        other.send(0x89, &vec![0; most + 1]),
        Err(SendError::TooLarge(most + 1))
    );
    other.send(0x89, &vec![0; most]).unwrap();
    assert_eq!(other.backlog(), 255);
    // Every id that is left goes out, and nothing else.
    let allowed: Vec<u8> = (0x80..=0xAA)
        .filter(|id| ![0x81, 0x86, 0xA0, 0xA7].contains(id))
        .collect();
    for id in &allowed {
        lobby.send(*id, &[*id]).unwrap();
    }
    let mut seen = Vec::new();
    for datagram in lobby.poll(now) {
        ClientModel::check(Table::Lobby, 0, &datagram).unwrap();
        seen.extend(
            messages(Table::Lobby, &datagram)
                .iter()
                .map(|message| message.0),
        );
    }
    assert!(seen.iter().all(|id| allowed.contains(id)));
    assert!(allowed.iter().all(|id| seen.contains(id)));

    let (mut game, now) = connect_match();
    for id in (0x00..0xE3).chain(0xF7..=0xFF) {
        assert_eq!(
            game.send(id, &[]),
            Err(SendError::UnknownId(id)),
            "{id:#04x}"
        );
    }
    assert_eq!(
        game.send(0xE3, &[1, 2, 3]),
        Err(SendError::BadLength {
            id: 0xE3,
            expected: 4,
            got: 3
        })
    );
    assert_eq!(
        game.send(0xF5, &[]),
        Err(SendError::BadLength {
            id: 0xF5,
            expected: 1,
            got: 0
        })
    );
    // The client's length getter fails E4 and F6 without a byte, and E8
    // with fewer than three.
    assert!(matches!(
        game.send(0xE4, &[]),
        Err(SendError::BadLength { .. })
    ));
    assert!(matches!(
        game.send(0xF6, &[]),
        Err(SendError::BadLength { .. })
    ));
    assert!(matches!(
        game.send(0xE8, &[0, 0]),
        Err(SendError::BadLength { .. })
    ));
    game.send(0xE8, &[0, 0, 0]).unwrap();
    assert_eq!(game.poll(now), [vec![0, 0, 0, 0xE8, 0, 0, 0, 0]]);
}

#[test]
fn nothing_is_sent_before_the_clients_first_datagram() {
    let now = Instant::now();
    let mut transport = Transport::new(Table::Match, now);
    transport.send(0xE3, &[0; 4]).unwrap();
    assert!(transport.poll(now).is_empty());
    assert!(transport.poll(now + ms(500)).is_empty());
    // A datagram that does not parse is no first datagram either.
    transport.receive(&[0, 0xFF, 0], now);
    transport.receive(&[0, 0xFF, 0, 0x99], now);
    assert_eq!(transport.rejected(), 2);
    assert!(transport.poll(now + ms(600)).is_empty());
    transport.receive(&flow(7, 0xFF, &[]), now);
    assert_eq!(
        transport.poll(now + ms(600)),
        [vec![0, 7, 0, 0xE3, 0, 0, 0, 0]]
    );
}

// ---------------------------------------------------------- reliability

#[test]
fn an_unacked_message_goes_again_under_a_new_seq_until_acked() {
    let (mut transport, now) = connect_match();
    let serial = transport.send(0xF3, &[1, 2, 3]).unwrap();
    let first = transport.poll(now);
    assert_eq!(first, [vec![0, 0, 0, 0xF3, 1, 2, 3]]);
    assert!(transport.poll(now + ms(49)).is_empty());
    // F3 is resent every 50 ms.
    let again = transport.poll(now + ms(50));
    assert_eq!(again, [vec![1, 0, 0, 0xF3, 1, 2, 3]]);

    // An ack of the first copy's seq is enough.
    assert!(!transport.is_delivered(serial));
    transport.receive(&flow(1, 0, &[]), now + ms(60));
    assert_eq!(transport.backlog(), 0);
    assert!(transport.is_delivered(serial));
    assert_eq!(transport.poll(now + ms(200)), [vec![2, 1, 0x40, 0xFC]]);
    assert_eq!(transport.resent(), 1);
}

#[test]
fn a_lobby_message_is_resent_after_a_second() {
    let (mut transport, now) = connect_lobby();
    transport.send(0x8B, &[5]).unwrap();
    assert_eq!(transport.poll(now), [vec![0, 0, 0, 0x8B, 0, 5]]);
    // Keep-alives in between, the message again at 1000 ms.
    let mut at = now;
    for _ in 0..3 {
        at += ms(250);
        assert_eq!(transport.poll(at).len(), 1);
        assert_eq!(transport.resent(), 0);
    }
    let again = transport.poll(now + ms(1000));
    assert_eq!(again, [vec![4, 0, 0, 0x8B, 0, 5]]);
}

#[test]
fn a_message_the_client_skipped_over_in_its_acks_goes_again_at_once() {
    let (mut transport, now) = connect_match();
    // Four reliable messages in four datagrams (seqs 0 to 3), the first lost.
    let mut sent = Vec::new();
    for n in 0..4u8 {
        transport.send(0xF4, &[n, n]).unwrap();
        sent.push(transport.poll(now).remove(0));
    }
    assert_eq!(sent[3][0], 3);
    // The client acks 3, 2 and 1, but not 0: no waiting for 50 ms.
    transport.receive(&flow(1, 3, &[1, 2]), now);
    let again = transport.poll(now);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0][3..6], [0xF4, 0, 0]);
    assert_eq!(transport.resent(), 1);
}

#[test]
fn a_message_only_one_datagram_behind_the_acks_waits_for_its_time() {
    let (mut transport, now) = connect_match();
    for n in 0..2u8 {
        transport.send(0xF4, &[n, n]).unwrap();
        transport.poll(now);
    }
    // 1 acked, 0 not: that may be reordering, so no resend yet.
    transport.receive(&flow(1, 1, &[]), now);
    assert!(transport.poll(now + ms(10)).is_empty());
    assert_eq!(transport.resent(), 0);
}

#[test]
fn a_short_lived_message_is_dropped_at_the_end_of_its_lifetime() {
    let (mut transport, now) = connect_match();
    // E9 lives 250 ms and goes every 50 ms.
    let lost = transport.send(0xE9, &[1, 2]).unwrap();
    for step in 0..5 {
        let datagrams = transport.poll(now + ms(50) * step);
        assert_eq!(datagrams.len(), 1, "send {step}");
        assert_eq!(datagrams[0][3..], [0xE9, 1, 2]);
        assert!(!transport.is_lost(lost));
    }
    assert!(transport.poll(now + ms(250)).is_empty());
    assert!(transport.is_lost(lost));
    assert!(!transport.is_delivered(lost));
    assert_eq!(transport.backlog(), 0);

    // An acked one is delivered, not lost. The snapshot E8 lives 1.75 s.
    let at = now + ms(300);
    let acked = transport.send(0xEA, &[3, 4]).unwrap();
    let snapshot = transport.send(0xE8, &[9; 40]).unwrap();
    assert_eq!(transport.poll(at).len(), 1);
    transport.receive(&flow(1, 5, &[]), at);
    assert!(transport.is_delivered(acked) && transport.is_delivered(snapshot));
    let unacked = transport.send(0xE8, &[8; 40]).unwrap();
    transport.poll(at);
    transport.poll(at + ms(1749));
    assert!(!transport.is_lost(unacked));
    transport.poll(at + ms(1750));
    assert!(transport.is_lost(unacked) && !transport.is_lost(snapshot));
}

/// How many of `datagrams` carry the message `id` (the others are
/// keep-alives, or carry something else).
fn carrying(datagrams: &[Vec<u8>], id: u8) -> usize {
    datagrams
        .iter()
        .filter(|datagram| {
            messages(Table::Match, datagram)
                .iter()
                .any(|message| message.0 == id)
        })
        .count()
}

#[test]
fn a_message_lasts_as_long_as_its_sender_says() {
    let (mut transport, now) = connect_match();
    // The snapshot E8 with four seconds instead of the table's 1.75.
    let patient = transport.send_lasting(0xE8, &[8; 40], ms(4000)).unwrap();
    assert_eq!(carrying(&transport.poll(now), 0xE8), 1);
    // Again every 500 ms, as the table has it.
    assert_eq!(carrying(&transport.poll(now + ms(499)), 0xE8), 0);
    assert_eq!(carrying(&transport.poll(now + ms(500)), 0xE8), 1);
    transport.poll(now + ms(3999));
    assert!(!transport.is_lost(patient));
    transport.poll(now + ms(4000));
    assert!(transport.is_lost(patient));

    // What the table sends once, or keeps until it is acked, stays so.
    let at = now + ms(5000);
    transport.send_lasting(0xE5, &[1; 7], ms(4000)).unwrap();
    let kept = transport.send_lasting(0xF3, &[1, 2, 3], ms(100)).unwrap();
    let first = transport.poll(at);
    assert_eq!((carrying(&first, 0xE5), carrying(&first, 0xF3)), (1, 1));
    let again = transport.poll(at + ms(1000));
    assert_eq!((carrying(&again, 0xE5), carrying(&again, 0xF3)), (0, 1));
    assert!(!transport.is_lost(kept));
}

#[test]
fn a_kept_message_goes_again_at_the_pace_its_sender_says() {
    let (mut transport, now) = connect_match();
    // A relay (EB: 250 ms of life, again every 50 ms) for a client that
    // waits for its world: kept, and again every 150 ms.
    let relay = transport.send_kept_every(0xEB, &[7; 10], ms(150)).unwrap();
    assert_eq!(carrying(&transport.poll(now), 0xEB), 1);
    assert_eq!(carrying(&transport.poll(now + ms(149)), 0xEB), 0);
    assert_eq!(carrying(&transport.poll(now + ms(150)), 0xEB), 1);
    assert_eq!(carrying(&transport.poll(now + ms(299)), 0xEB), 0);
    assert_eq!(carrying(&transport.poll(now + ms(300)), 0xEB), 1);
    // Long past a relay's own lifetime, and still not given up.
    let late = transport.poll(now + ms(1950));
    assert_eq!(carrying(&late, 0xEB), 1);
    assert!(!transport.is_lost(relay));
    transport.receive(&flow(1, late[0][0], &[]), now + ms(2000));
    assert!(transport.is_delivered(relay));
}

#[test]
fn a_message_goes_again_at_its_senders_pace_within_its_own_lifetime() {
    let (mut transport, now) = connect_match();
    // A world (E8: 1.75 s of life, again every 500 ms) again every 150 ms.
    let world = transport.send_every(0xE8, &[8; 40], ms(150)).unwrap();
    assert_eq!(carrying(&transport.poll(now), 0xE8), 1);
    assert_eq!(carrying(&transport.poll(now + ms(149)), 0xE8), 0);
    assert_eq!(carrying(&transport.poll(now + ms(150)), 0xE8), 1);
    assert_eq!(carrying(&transport.poll(now + ms(300)), 0xE8), 1);
    // Given up at the end of the table's lifetime all the same.
    transport.poll(now + ms(1749));
    assert!(!transport.is_lost(world));
    transport.poll(now + ms(1750));
    assert!(transport.is_lost(world));
}

#[test]
fn a_send_once_unique_message_can_be_kept_with_its_unique_id() {
    let (mut transport, now) = connect_match();
    // A clock correction (E7: sent once by the table) again every 150 ms
    // until acked, always with the same unique id: the client acts on one.
    let correction = transport
        .send_unique_kept(0xE7, &[0x10, 0x00], ms(150))
        .unwrap();
    let unique_ids = |datagrams: &[Vec<u8>]| -> Vec<u8> {
        datagrams
            .iter()
            .flat_map(|datagram| {
                wire::parse(Table::Match, Direction::ToClient, datagram, 3)
                    .unwrap()
                    .messages
                    .iter()
                    .filter(|message| message.id == 0xE7)
                    .filter_map(|message| message.x)
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let first = transport.poll(now);
    let x = unique_ids(&first);
    assert_eq!(x.len(), 1);
    assert!(unique_ids(&transport.poll(now + ms(149))).is_empty());
    let again = transport.poll(now + ms(150));
    assert_eq!(unique_ids(&again), x);
    assert_eq!(unique_ids(&transport.poll(now + ms(3_000))), x);
    assert!(!transport.is_lost(correction) && !transport.is_delivered(correction));
    let last = transport.poll(now + ms(3_150));
    transport.receive(&flow(1, last[0][0], &[]), now + ms(3_200));
    assert!(transport.is_delivered(correction));
    assert!(unique_ids(&transport.poll(now + ms(4_000))).is_empty());
}

/// How many of `datagrams` carry `bytes` somewhere.
fn holding(datagrams: &[Vec<u8>], bytes: &[u8]) -> usize {
    datagrams
        .iter()
        .filter(|datagram| datagram.windows(bytes.len()).any(|window| window == bytes))
        .count()
}

#[test]
fn a_deadline_ends_a_message_however_often_it_went() {
    let (mut transport, now) = connect_match();
    // A relay with 450 ms from now: again every 50 ms, past the table's
    // 250 ms, until then.
    let relay = transport.send_until(0xEB, &[7; 10], now + ms(450)).unwrap();
    for step in 0..9 {
        let datagrams = transport.poll(now + ms(50) * step);
        assert_eq!(carrying(&datagrams, 0xEB), 1, "send {step}");
    }
    assert!(!transport.is_lost(relay));
    assert_eq!(carrying(&transport.poll(now + ms(450)), 0xEB), 0);
    assert!(transport.is_lost(relay) && !transport.is_delivered(relay));

    // A near deadline never makes the life shorter than the table's 250
    // ms from the first send: five copies, as without one.
    let at = now + ms(1000);
    let near = transport.send_until(0xEB, &[8; 10], at + ms(100)).unwrap();
    for step in 0..5 {
        let datagrams = transport.poll(at + ms(60) + ms(50) * step);
        assert_eq!(carrying(&datagrams, 0xEB), 1, "send {step}");
    }
    assert!(!transport.is_lost(near));
    assert_eq!(carrying(&transport.poll(at + ms(310)), 0xEB), 0);
    assert!(transport.is_lost(near));

    // What the table keeps or sends once is not cut short.
    let at = now + ms(2000);
    let kept = transport.send_until(0xF3, &[1, 2, 3], at).unwrap();
    assert_eq!(carrying(&transport.poll(at + ms(500)), 0xF3), 1);
    assert!(!transport.is_lost(kept));
}

#[test]
fn a_message_held_back_past_its_deadline_is_lost_unsent() {
    let (mut transport, now) = connect_match();
    // 100 datagrams the client does not ack: data waits.
    for step in 0..100u8 {
        transport.send(0xF4, &[step, step]).unwrap();
        assert_eq!(transport.poll(now + ms(u64::from(step))).len(), 1);
    }
    let at = now + ms(100);
    let relay = transport.send_until(0xEB, &[2; 10], at + ms(450)).unwrap();
    // Without a deadline a short-lived message's life starts at its first
    // send: it would go out whenever the link moves again, however old.
    let old = transport.send(0xEB, &[1; 10]).unwrap();
    for wait in [0, 100, 300, 449] {
        assert_eq!(carrying(&transport.poll(at + ms(wait)), 0xEB), 0);
    }
    assert!(!transport.is_lost(relay));
    transport.poll(at + ms(450));
    assert!(transport.is_lost(relay) && !transport.is_lost(old));
    // The client acks everything: only the one without a deadline goes.
    let also: Vec<u8> = (0..99).collect();
    transport.receive(&flow(1, 99, &also), at + ms(600));
    let after = transport.poll(at + ms(600));
    assert_eq!(holding(&after, &[1; 10]), 1);
    assert_eq!(holding(&after, &[2; 10]), 0);
}

// ------------------------------------------------------- gathering relays

#[test]
fn relays_queued_within_the_gather_time_share_one_datagram() {
    let (mut transport, now) = connect_match();
    // After a quiet time a lone relay goes at once.
    transport.send(0xEB, &[1; 10]).unwrap();
    assert_eq!(holding(&transport.poll(now), &[1; 10]), 1);
    // What comes within 15 ms of it waits for more.
    transport.send(0xEB, &[2; 10]).unwrap();
    assert!(transport.poll(now + ms(5)).is_empty());
    transport.send(0xEC, &[3; 11]).unwrap();
    transport.send(0xF1, &[4]).unwrap();
    assert!(transport.poll(now + ms(10)).is_empty());
    let together = transport.poll(now + ms(15));
    assert_eq!(together.len(), 1);
    assert_eq!(
        messages(Table::Match, &together[0]),
        [(0xEB, None, 10), (0xEC, None, 11), (0xF1, None, 1)]
    );
    // Resends ride along with the next flush.
    transport.send(0xEB, &[5; 10]).unwrap();
    let next = transport.poll(now + ms(50));
    assert_eq!(next.len(), 1);
    assert_eq!(holding(&next, &[1; 10]) + holding(&next, &[5; 10]), 2);
}

#[test]
fn a_message_that_may_not_wait_takes_the_relays_along() {
    // The clock messages and the ping (E5, E6, E7) and anything kept
    // until acked go at once, and what waits goes with them.
    let messages_for = [
        (0xE5, vec![1; 7]),
        (0xE6, vec![1; 4]),
        (0xE7, vec![1; 2]),
        (0xF3, vec![1; 3]),
    ];
    for (id, payload) in messages_for {
        let (mut transport, now) = connect_match();
        transport.send(0xEB, &[1; 10]).unwrap();
        assert_eq!(transport.poll(now).len(), 1);
        transport.send(0xEB, &[2; 10]).unwrap();
        assert!(transport.poll(now + ms(2)).is_empty());
        transport.send(id, &payload).unwrap();
        let datagrams = transport.poll(now + ms(3));
        assert_eq!(datagrams.len(), 1, "{id:#04x}");
        assert_eq!(holding(&datagrams, &[2; 10]), 1, "{id:#04x}");
        assert_eq!(carrying(&datagrams, id), 1, "{id:#04x}");
    }
}

#[test]
fn relays_kept_until_acked_are_packed_as_any_relay() {
    let (mut transport, now) = connect_match();
    transport.send(0xEB, &[1; 10]).unwrap();
    assert_eq!(transport.poll(now).len(), 1);
    // Within the gather time of the last datagram, kept relays (a resync's
    // replay, the relays to a resynced game) wait to share a datagram ...
    transport.send_kept(0xEB, &[2; 10]).unwrap();
    assert!(transport.poll(now + ms(2)).is_empty());
    transport.send_kept(0xEB, &[3; 10]).unwrap();
    assert!(transport.poll(now + ms(5)).is_empty());
    // ... and go together at its end.
    let datagrams = transport.poll(now + ms(15));
    assert_eq!(datagrams.len(), 1);
    assert_eq!(holding(&datagrams, &[2; 10]), 1);
    assert_eq!(holding(&datagrams, &[3; 10]), 1);
    // A 0xE5 queued behind kept relays takes them along at once.
    transport.send_kept(0xEB, &[4; 10]).unwrap();
    transport.send(0xE5, &[1; 7]).unwrap();
    let datagrams = transport.poll(now + ms(16));
    assert_eq!(datagrams.len(), 1);
    assert_eq!(holding(&datagrams, &[4; 10]), 1);
    assert_eq!(carrying(&datagrams, 0xE5), 1);
}

#[test]
fn whether_a_message_is_delivered_is_known_without_a_walk_of_the_queue() {
    let (mut transport, now) = connect_match();
    let mut serials = Vec::new();
    for n in 0..40u8 {
        serials.push(transport.send_kept(0xEB, &[n; 10]).unwrap());
    }
    let world = transport.send(0xE8, &vec![7; 3_000]).unwrap();
    let once = transport.send(0xE6, &[1; 4]).unwrap();
    assert!(transport.outstanding_agrees());
    let mut at = now;
    let mut sent = Vec::new();
    for _ in 0..20 {
        at += ms(20);
        sent.extend(transport.poll(at));
        assert!(transport.outstanding_agrees());
    }
    assert!(!transport.is_delivered(serials[0]));
    // The client acks every other datagram it was sent, then all.
    let mut own = 0u8;
    let count = sent.len() as u8;
    for seq in (0..count).step_by(2) {
        own += 1;
        transport.receive(&flow(own, seq, &[]), at);
        assert!(transport.outstanding_agrees());
    }
    for _ in 0..40 {
        at += ms(20);
        sent.extend(transport.poll(at));
        let newest = (sent.len() - 1) as u8;
        own += 1;
        transport.receive(&flow(own, newest, &[]), at);
        assert!(transport.outstanding_agrees());
    }
    assert!(serials.iter().all(|serial| transport.is_delivered(*serial)));
    assert!(transport.is_delivered(world));
    assert!(transport.is_delivered(once));
    // A forgotten one counts as delivered at once.
    let forgotten = transport.send_kept(0xEB, &[99; 10]).unwrap();
    assert!(!transport.is_delivered(forgotten));
    transport.forget(forgotten, at);
    assert!(transport.is_delivered(forgotten));
    assert!(transport.outstanding_agrees());
}

#[test]
fn two_messages_sent_joined_go_in_one_datagram_in_their_order() {
    let (mut transport, now) = connect_match();
    transport.send(0xEB, &[1; 10]).unwrap();
    transport.poll(now);
    // 45 relays nearly fill a datagram: the pair does not fit behind them
    // and goes whole in the next one, 0xE7 ahead of 0xE5.
    for n in 0..45u8 {
        transport.send(0xEB, &[n; 10]).unwrap();
    }
    transport
        .send_joined((0xE7, &[0, 0]), (0xE5, &[1; 7]))
        .unwrap();
    let datagrams = transport.poll(now + ms(1));
    assert_eq!(datagrams.len(), 2);
    let first = messages(Table::Match, &datagrams[0]);
    assert!(first.iter().all(|(id, ..)| *id == 0xEB), "{first:?}");
    let second: Vec<u8> = messages(Table::Match, &datagrams[1])
        .iter()
        .map(|(id, ..)| *id)
        .collect();
    let at = second.iter().position(|id| *id == 0xE7).unwrap();
    assert_eq!(second[at + 1], 0xE5, "{second:?}");
    // Sent once each, as the table has them.
    assert!(transport.poll(now + ms(1_000)).iter().all(|datagram| {
        carrying(std::slice::from_ref(datagram), 0xE5) == 0
            && carrying(std::slice::from_ref(datagram), 0xE7) == 0
    }));
}

#[test]
fn a_full_datagram_of_relays_goes_without_waiting() {
    let (mut transport, now) = connect_match();
    transport.send(0xEB, &[1; 10]).unwrap();
    transport.poll(now);
    // 11 bytes each: 46 do not fit in one datagram.
    for n in 0..45u8 {
        transport.send(0xEB, &[n; 10]).unwrap();
    }
    assert!(transport.poll(now + ms(1)).is_empty());
    transport.send(0xEB, &[99; 10]).unwrap();
    let full = transport.poll(now + ms(2));
    assert_eq!(full.len(), 2);
    assert!(full[0].len() > 480);
}

#[test]
fn an_ack_is_never_held_longer_than_the_ack_delay() {
    let now = Instant::now();
    // A long gather time, to tell the two apart.
    let config = Config {
        gather: ms(100),
        ..Config::for_table(Table::Match)
    };
    let mut transport = Transport::with_config(Table::Match, config, now);
    transport.receive(&[0x00, 0xFF, 0x00, 0xD8, 1, 2, 3, 4, 0x00], now);
    transport.send(0xEB, &[1; 10]).unwrap();
    assert_eq!(transport.poll(now).len(), 1);
    transport.send(0xEB, &[2; 10]).unwrap();
    assert!(transport.poll(now + ms(10)).is_empty());
    // A client datagram: its ack goes 20 ms after the last send, on the
    // waiting relay rather than on a keep-alive.
    transport.receive(&flow(1, 0, &[]), now + ms(12));
    assert!(transport.poll(now + ms(15)).is_empty());
    let ack = transport.poll(now + ms(20));
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0][1], 1);
    assert_eq!(holding(&ack, &[2; 10]), 1);
    // With nothing waiting, a keep-alive carries it, as before.
    transport.receive(&flow(2, 1, &[]), now + ms(25));
    let alive = transport.poll(now + ms(40));
    assert_eq!(alive.len(), 1);
    assert_eq!(alive[0][3], 0xFC);
}

/// The parts of a split message in `datagrams`: `(unique id, index)`.
fn parts(datagrams: &[Vec<u8>]) -> Vec<(u8, u8)> {
    datagrams
        .iter()
        .flat_map(|datagram| {
            wire::parse(Table::Match, Direction::ToClient, datagram, 3)
                .unwrap()
                .messages
                .iter()
                .filter_map(|message| Some((message.x?, message.part?.index)))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_parts_of_a_split_message_leave_a_few_at_a_time() {
    let (mut transport, now) = connect_match();
    // A world of 14 KiB: 29 parts.
    let world: Vec<u8> = (0..14_000u32).map(|i| (i % 239) as u8).collect();
    // What was queued before it does not wait for it. What is queued
    // behind it comes after its last part, as when the parts left at once
    // (the discards right behind a world are for the game that loaded it).
    transport.send(0xEB, &[7; 10]).unwrap();
    transport.send(0xE8, &world).unwrap();
    transport.send(0xF4, &[5, 5]).unwrap();
    let first = transport.poll(now);
    assert_eq!(messages(Table::Match, &first[0]), [(0xEB, None, 10)]);
    assert_eq!(parts(&first), [(0, 0), (0, 1), (0, 2), (0, 3)]);
    assert_eq!(carrying(&first, 0xF4), 0);
    // One more every 3 ms, however often the server looks.
    assert!(parts(&transport.poll(now + ms(1))).is_empty());
    assert!(parts(&transport.poll(now + ms(2))).is_empty());
    assert_eq!(parts(&transport.poll(now + ms(3))), [(0, 4)]);
    assert!(parts(&transport.poll(now + ms(5))).is_empty());
    assert_eq!(parts(&transport.poll(now + ms(6))), [(0, 5)]);
    // A server that looks late sends what the time earned, four at most.
    assert_eq!(parts(&transport.poll(now + ms(12))), [(0, 6), (0, 7)]);
    assert_eq!(
        parts(&transport.poll(now + ms(60))),
        [(0, 8), (0, 9), (0, 10), (0, 11)]
    );
    // One that looks every 5 ms (the match server's own turn when nothing
    // comes in) loses none of the time between: 5 parts in 15 ms.
    let mut sent = 12;
    let mut at = now + ms(60);
    for expected in [1, 2, 2] {
        at += ms(5);
        let next = parts(&transport.poll(at));
        assert_eq!(next.len(), expected, "{next:?}");
        sent += expected as u8;
    }
    while sent < 29 {
        at += ms(3);
        let datagrams = transport.poll(at);
        assert_eq!(parts(&datagrams), [(0, sent)]);
        // With the last part, what was queued behind the world.
        assert_eq!(carrying(&datagrams, 0xF4), usize::from(sent == 28));
        sent += 1;
    }
    assert!(at <= now + ms(120));

    // The parts nobody acked go again after their 500 ms, at the same
    // pace; behind those nothing waits.
    transport.send(0xF1, &[9]).unwrap();
    let again = transport.poll(now + ms(700));
    assert_eq!(parts(&again), [(0, 0), (0, 1), (0, 2), (0, 3)]);
    assert_eq!(carrying(&again, 0xF1), 1);
    assert_eq!(parts(&transport.poll(now + ms(703))), [(0, 4)]);
}

#[test]
fn a_split_message_ends_whole_with_its_first_part() {
    let (mut transport, now) = connect_match();
    let world: Vec<u8> = (0..14_000u32).map(|i| (i % 239) as u8).collect();
    let serial = transport.send(0xE8, &world).unwrap();
    // Its 29 parts leave over 75 ms, so the last is 1.75 s old 75 ms
    // after the first.
    for step in 0..=25 {
        transport.poll(now + ms(3) * step);
    }
    transport.poll(now + ms(1749));
    assert!(!transport.is_lost(serial));
    assert_eq!(transport.backlog(), 29);
    // With the first part the message is lost, and no part of it goes
    // any more: the client could not join it.
    assert!(parts(&transport.poll(now + ms(1750))).is_empty());
    assert!(transport.is_lost(serial));
    assert_eq!(transport.backlog(), 0);
    assert!(parts(&transport.poll(now + ms(1800))).is_empty());
}

#[test]
fn the_lobby_sends_a_split_message_at_once() {
    let (mut transport, now) = connect_lobby();
    // 0x95, the shop: ordered, and split when it is large.
    transport.send(0x95, &[0x5A; 6_000]).unwrap();
    assert_eq!(transport.poll(now).len(), 13);
}

#[test]
fn a_send_once_message_is_never_resent() {
    let (mut transport, now) = connect_match();
    let serial = transport.send(0xE5, &[1; 7]).unwrap();
    assert!(!transport.is_delivered(serial));
    assert_eq!(transport.poll(now).len(), 1);
    assert!(transport.is_delivered(serial));
    assert_eq!(transport.backlog(), 0);
    assert_eq!(transport.poll(now + ms(2000)), [vec![1, 0, 0, 0xFC]]);
}

#[test]
fn the_time_to_an_ack_is_the_round_trip() {
    let (mut transport, now) = connect_match();
    assert_eq!(transport.take_round_trips(), None);
    // Seqs 0 and 1 go out together; the client acks 0 after 40 ms and 1
    // after 90 ms (its ack of 1 also names 0, which counts once).
    for n in 0..2u8 {
        transport.send(0xF4, &[n, n]).unwrap();
        transport.poll(now);
    }
    transport.receive(&flow(1, 0, &[]), now + ms(40));
    assert_eq!(transport.last_round_trip(), Some(40));
    transport.receive(&flow(2, 1, &[0]), now + ms(90));
    assert_eq!(transport.take_round_trips(), Some((65, 90)));
    // Taken: the next window starts empty, and a repeated ack adds none.
    transport.receive(&flow(3, 1, &[0]), now + ms(95));
    assert_eq!(transport.take_round_trips(), None);
    assert_eq!(transport.last_round_trip(), Some(90));
}

#[test]
fn data_waits_when_the_client_acks_nothing() {
    let (mut transport, now) = connect_lobby();
    // 121 parts.
    transport.send(0x89, &vec![1; 60000]).unwrap();
    // 48 at once, then one more each millisecond, up to 100 unacked.
    let mut sent = transport.poll(now);
    assert_eq!(sent.len(), 48);
    sent.extend(transport.poll(now + ms(10)));
    assert_eq!(sent.len(), 58);
    sent.extend(transport.poll(now + ms(100)));
    assert_eq!(sent.len(), 100);
    assert_eq!(sent[99][0], 99);
    // Nothing more, and keep-alives only once a second.
    assert!(transport.poll(now + ms(500)).is_empty());
    let alive = transport.poll(now + ms(1100));
    assert_eq!(alive.len(), 1);
    assert_eq!(alive[0][3], 0xFC);
    // The client acks seqs 0..=99: the rest goes.
    let also: Vec<u8> = (0..99).collect();
    transport.receive(&flow(1, 99, &also), now + ms(1100));
    let rest = transport.poll(now + ms(1100));
    assert_eq!(rest.len(), 21);
    assert_eq!(transport.resent(), 0);
}

#[test]
fn unique_ids_stay_within_the_clients_window() {
    let (mut transport, now) = connect_match();
    // E4 (unique id 0) stays unacked; 150 pings follow on the same counter.
    transport.send(0xE4, &[5; 10]).unwrap();
    for n in 0..150u8 {
        transport.send(0xE6, &[n; 4]).unwrap();
    }
    let count = |datagrams: &[Vec<u8>]| -> Vec<u8> {
        datagrams
            .iter()
            .flat_map(|datagram| messages(Table::Match, datagram))
            .filter(|message| message.0 == 0xE6)
            .map(|message| message.1.unwrap())
            .collect()
    };
    let first = transport.poll(now);
    // Unique ids 1..=99 go; 100 would be 100 past the unacked 0.
    assert_eq!(count(&first), (1..=99).collect::<Vec<u8>>());
    let newest = first.last().unwrap()[0];
    let also: Vec<u8> = (0..newest).collect();
    transport.receive(&flow(1, newest, &also), now);
    // Acked, but a late copy may still arrive: the ids wait out the guard
    // time (500 ms) before the window moves on.
    assert_eq!(count(&transport.poll(now)), []);
    assert_eq!(count(&transport.poll(now + ms(499))), []);
    let rest = transport.poll(now + ms(500));
    assert_eq!(count(&rest), (100..=150).collect::<Vec<u8>>());
}

// ------------------------------------------------------------ the acks

#[test]
fn every_datagram_acks_what_arrived() {
    let (mut transport, now) = connect_match();
    // Seq 2 arrives before 1; 1 then arrives too, and 1 again is dropped.
    transport.receive(&flow(2, 0xFF, &[]), now);
    assert_eq!(transport.poll(now), [vec![0, 2, 0x20, 0xFC]]);
    let report = client(1, 0xFF, &[], false, &[0xDF, 1, 2, 3, 4, 5]);
    assert_eq!(transport.receive(&report, now).len(), 1);
    assert!(transport.receive(&report, now).is_empty());
    assert_eq!(transport.poll(now + ms(100)), [vec![1, 2, 0x60, 0xFC]]);
}

#[test]
fn acks_follow_the_client_across_the_seq_wrap() {
    let (mut transport, now) = connect_lobby();
    transport.poll(now);
    let received = |number: u32| number == 0 || number % 7 != 3;
    let mut serials = Vec::new();
    let mut server_seq = 0u8;
    for number in 1..=600u32 {
        if !received(number) {
            continue;
        }
        let at = now + ms(30) * number;
        transport.receive(&flow(number as u8, server_seq, &[]), at);
        serials.push(transport.send(0x8B, &number.to_le_bytes()).unwrap());
        let datagrams = transport.poll(at);
        assert_eq!(datagrams.len(), 1);
        let datagram = &datagrams[0];
        server_seq = server_seq.wrapping_add(1);
        assert_eq!(datagram[0], server_seq);
        assert_eq!(datagram[1], number as u8);
        let acks = ack::decode(datagram).unwrap();
        // The client is known to have acks up to the seq before: the
        // window sent is short, and reaches every seq of the margin of 14.
        assert!(acks.tail <= 3, "tail {}", acks.tail);
        for k in 1..=number.min(14) {
            assert!(
                !received(number - k) || k < acks.count,
                "client seq {number} - {k}"
            );
        }
        for k in 1..acks.count {
            let bit = acks.bits >> (127 - k) & 1 != 0;
            let expected = number >= k && received(number - k);
            assert_eq!(bit, expected, "client seq {number} - {k}");
        }
        ClientModel::check(Table::Lobby, 0, datagram).unwrap();
    }
    assert_eq!(transport.resent(), 0);
    let delivered = serials
        .iter()
        .filter(|serial| transport.is_delivered(**serial));
    assert_eq!(delivered.count(), serials.len() - 1);
}

#[test]
fn the_ack_window_shrinks_to_what_the_client_has_not_seen_acked() {
    let (mut transport, now) = connect_lobby();
    let mut tails = Vec::new();
    for seq in 1..=60u8 {
        let at = now + ms(30) * u32::from(seq);
        // No acks of the server's datagrams at first, then all of them.
        let rseq = if seq <= 30 { 0xFF } else { seq - 2 };
        transport.receive(&flow(seq, rseq, &[]), at);
        let datagram = transport.poll(at).remove(0);
        tails.push(ack::decode(&datagram).unwrap().tail);
    }
    // 30 client seqs unconfirmed: 30 back bits, 4 tail bytes.
    assert_eq!(tails[29], 4);
    // Confirmed up to the seq before: the margin of 14 plus one, 2 bytes.
    assert_eq!(tails[59], 2);
}

// ------------------------------------------------------------ keep-alive

#[test]
fn a_client_datagram_is_acked_promptly_and_a_quiet_link_kept_alive() {
    let (mut transport, now) = connect_match();
    assert_eq!(transport.poll(now), [vec![0, 0, 0, 0xFC]]);
    assert!(transport.poll(now + ms(50)).is_empty());
    assert_eq!(transport.poll(now + ms(100)), [vec![1, 0, 0, 0xFC]]);
    // A client datagram is acked 20 ms after the last send at the latest.
    transport.receive(&flow(1, 1, &[]), now + ms(105));
    assert!(transport.poll(now + ms(110)).is_empty());
    assert_eq!(transport.poll(now + ms(120)), [vec![2, 1, 0x40, 0xFC]]);
    assert!(transport.poll(now + ms(200)).is_empty());
    assert_eq!(transport.poll(now + ms(220)).len(), 1);
}

#[test]
fn a_silent_client_is_gone_after_the_timeout() {
    let (transport, now) = connect_match();
    assert!(!transport.is_gone(now + ms(9999)));
    assert!(transport.is_gone(now + ms(10000)));
    assert!(!transport.is_gone_after(now + ms(10000), ms(60000)));
    assert!(transport.is_gone_after(now + ms(60000), ms(60000)));
}

// ------------------------------------------------------- disconnecting

#[test]
fn the_client_leaving_is_confirmed() {
    let (mut transport, now) = connect_match();
    transport.send(0xF3, &[1, 2, 3]).unwrap();
    let events = transport.receive(&client(1, 0xFF, &[], false, &[0xFA]), now);
    assert_eq!(events, [Event::Disconnected]);
    assert_eq!(transport.poll(now), [vec![0, 1, 0x40, 0xFB]]);
    // Asked again, it confirms again without a second event.
    let events = transport.receive(&client(2, 0xFF, &[], false, &[0xFA]), now);
    assert!(events.is_empty());
    assert_eq!(transport.poll(now), [vec![1, 2, 0x60, 0xFB]]);
    assert!(transport.is_gone(now));
    assert_eq!(
        transport.send(0xF3, &[1, 2, 3]),
        Err(SendError::NotConnected)
    );
    assert!(transport.poll(now + ms(500)).is_empty());
}

#[test]
fn a_disconnect_waits_for_the_confirm() {
    let (mut transport, now) = connect_match();
    transport.send(0xF3, &[1, 2, 3]).unwrap();
    transport.disconnect(now);
    assert_eq!(transport.poll(now), [vec![0, 0, 0, 0xFA]]);
    assert!(transport.poll(now + ms(50)).is_empty());
    assert_eq!(transport.poll(now + ms(100)), [vec![1, 0, 0, 0xFA]]);
    assert!(!transport.is_gone(now + ms(100)));
    let events = transport.receive(&client(1, 1, &[], false, &[0xFB]), now + ms(120));
    assert_eq!(events, [Event::Disconnected]);
    assert!(transport.is_gone(now + ms(120)));

    // Without a confirm it ends after a second.
    let (mut transport, now) = connect_match();
    transport.disconnect(now);
    assert_eq!(transport.poll(now + ms(900)).len(), 1);
    assert!(transport.poll(now + ms(1000)).is_empty());
    assert_eq!(transport.state(), State::Closed);
}

#[test]
fn a_kick_is_the_instant_disconnect_then_the_handshake() {
    let (mut transport, now) = connect_lobby();
    transport.kick(now);
    assert_eq!(transport.poll(now), [vec![0, 0, 0, 0xFD]]);
    assert_eq!(transport.poll(now + ms(100)), [vec![1, 0, 0, 0xFA]]);
    assert_eq!(transport.send(0x80, &[]), Err(SendError::NotConnected));
}

#[test]
fn the_clients_instant_disconnect_ends_the_connection() {
    let (mut transport, now) = connect_lobby();
    let events = transport.receive(&client(1, 0xFF, &[], false, &[0xFD]), now);
    assert_eq!(events, [Event::Disconnected]);
    assert!(transport.is_gone(now));
    assert!(transport.poll(now).is_empty());
}

// --------------------------------------------------- from the client

#[test]
fn messages_in_a_multi_datagram_reach_the_caller() {
    let (mut transport, now) = connect_match();
    // Fixed lengths, no length bytes: DC (9), DE (2), E1 (0).
    let mut body = vec![0xDC];
    body.extend(1..=9);
    body.extend([0xDE, 7, 8, 0xE1]);
    let events = transport.receive(&client(1, 0xFF, &[], true, &body), now);
    assert_eq!(
        events,
        [
            Event::Message {
                id: 0xDC,
                payload: (1..=9).collect()
            },
            Event::Message {
                id: 0xDE,
                payload: vec![7, 8]
            },
            Event::Message {
                id: 0xE1,
                payload: vec![]
            },
        ]
    );

    let (mut transport, now) = connect_lobby();
    let body = [0x2F, 1, 2, 0xAA, 0xBB, 0x30, 2, 0, 0xFC];
    let events = transport.receive(&client(1, 0xFF, &[], true, &body), now);
    assert_eq!(
        events,
        [
            Event::Message {
                id: 0x2F,
                payload: vec![0xAA, 0xBB]
            },
            Event::Message {
                id: 0x30,
                payload: vec![]
            },
        ]
    );
}

#[test]
fn a_datagrams_unique_messages_reach_the_caller_in_their_send_order() {
    // Live, 2026-10-21: the lobby movie moved a decal to the other slot
    // (drop_upgrade_item, then relocate_upgrade_item, in one frame); the
    // client's send queue (by size) put `46 01` (unique id 0x22) before
    // `46 02` (0x21) in one datagram, and taken in that order the decal
    // came off. Here with unique ids 2 and 1, and a keep-alive after them.
    let (mut transport, now) = connect_lobby();
    let attach = [1, 1, 0, 0, 0, 0xD4, 0, 0, 0, 0xC9, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
    let detach = [2, 1, 0, 0, 0, 0xC9, 0, 0, 0, 0xD4, 0, 0, 0];
    let mut body = vec![0x46, 2, attach.len() as u8];
    body.extend(attach);
    body.extend([0x46, 1, detach.len() as u8]);
    body.extend(detach);
    body.push(0xFC);
    let events = transport.receive(&client(1, 0xFF, &[], true, &body), now);
    assert_eq!(
        events,
        [
            Event::Message {
                id: 0x46,
                payload: detach.to_vec()
            },
            Event::Message {
                id: 0x46,
                payload: attach.to_vec()
            },
        ]
    );
}

#[test]
fn a_datagrams_unique_messages_are_put_in_send_order_across_the_ids_wrap() {
    // Unique ids are a byte: 0xFF was sent before 0x01 (the connection's
    // first message took 0x00). The order is taken the nearest way round
    // from the first message's id, so 0xFF comes first, and both are new.
    let (mut transport, now) = connect_lobby();
    let mut body = vec![0x46, 0x01, 3, 7, 7, 7];
    body.extend([0x46, 0xFF, 2, 9, 9]);
    let events = transport.receive(&client(1, 0xFF, &[], true, &body), now);
    assert_eq!(
        events,
        [
            Event::Message {
                id: 0x46,
                payload: vec![9, 9]
            },
            Event::Message {
                id: 0x46,
                payload: vec![7, 7, 7]
            },
        ]
    );
}

#[test]
fn a_resent_unique_message_is_delivered_once_and_a_plain_one_again() {
    let (mut transport, now) = connect_lobby();
    let request = [0x2F, 1, 0xAA];
    assert_eq!(
        transport
            .receive(&client(1, 0xFF, &[], false, &request), now)
            .len(),
        1
    );
    // The same unique id under new seqs, and the sign-in (unique id 0) too.
    assert!(
        transport
            .receive(&client(2, 0xFF, &[], false, &request), now)
            .is_empty()
    );
    let sign_in = [0x2E, 0, 1, 2, 3, 4, 1];
    assert!(
        transport
            .receive(&client(3, 0xFF, &[], false, &sign_in), now)
            .is_empty()
    );
    // A new unique id with the same bytes is a new message.
    let request = [0x2F, 2, 0xAA];
    assert_eq!(
        transport
            .receive(&client(4, 0xFF, &[], false, &request), now)
            .len(),
        1
    );

    // Match messages are plain: the caller dedups D8, D9, DA, DE and E0.
    let (mut transport, now) = connect_match();
    let connect = [0xD8, 1, 2, 3, 4, 0];
    assert_eq!(
        transport
            .receive(&client(1, 0xFF, &[], false, &connect), now)
            .len(),
        1
    );
    // The same datagram again is dropped by its seq.
    assert!(
        transport
            .receive(&client(1, 0xFF, &[], false, &connect), now)
            .is_empty()
    );
}

#[test]
fn a_split_client_message_arrives_whole() {
    let (mut transport, now) = connect_lobby();
    let first: Vec<u8> = (0..200).collect();
    let second: Vec<u8> = (0..100).rev().collect();
    let mut part_0 = vec![0xFF, 0x47, 1, 0, 2];
    part_0.extend(&first);
    let mut part_1 = vec![0xFF, 0x47, 1, 1];
    part_1.extend(&second);
    // Part 1 first, then again under a new seq, then part 0.
    assert!(
        transport
            .receive(&client(1, 0xFF, &[], false, &part_1), now)
            .is_empty()
    );
    assert!(
        transport
            .receive(&client(2, 0xFF, &[], false, &part_1), now)
            .is_empty()
    );
    let events = transport.receive(&client(3, 0xFF, &[], false, &part_0), now);
    let mut whole = first.clone();
    whole.extend(&second);
    assert_eq!(
        events,
        [Event::Message {
            id: 0x47,
            payload: whole
        }]
    );
    // A late copy of a part of the finished message is dropped.
    assert!(
        transport
            .receive(&client(4, 0xFF, &[], false, &part_0), now)
            .is_empty()
    );
    assert!(
        transport
            .receive(&client(5, 0xFF, &[], false, &part_1), now)
            .is_empty()
    );
}

#[test]
fn a_datagram_that_does_not_parse_is_dropped_whole_and_not_acked() {
    let (mut transport, now) = connect_match();
    // A valid DE, then an id outside the table: nothing is delivered.
    let body = [0xDE, 1, 2, 0x99];
    assert!(
        transport
            .receive(&client(1, 0xFF, &[], true, &body), now)
            .is_empty()
    );
    // DC with 8 bytes in a single datagram.
    let mut short = vec![0xDC];
    short.extend([0; 8]);
    assert!(
        transport
            .receive(&client(2, 0xFF, &[], false, &short), now)
            .is_empty()
    );
    assert_eq!(transport.rejected(), 2);
    // Only seq 0 is acked.
    assert_eq!(transport.poll(now), [vec![0, 0, 0, 0xFC]]);
}

// ------------------------------------------------------------- the log

#[test]
fn a_transport_with_a_log_writes_every_datagram() {
    let directory = std::env::temp_dir().join(format!(
        "srp-transport-log-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let log = PacketLog::new(&directory).unwrap();
    let peer = "10.1.2.3:4567".parse().unwrap();
    let file = log
        .connection(Table::Lobby, peer)
        .unwrap()
        .with_redaction(Box::new(|direction, id, _| {
            direction == Direction::ToServer && id == 0x2E
        }));
    let path = file.path().to_path_buf();
    let now = Instant::now();
    let mut transport = Transport::new(Table::Lobby, now);
    transport.set_log(file);
    transport.receive(
        &[0x00, 0xFF, 0x00, 0x2E, 0x00, 0x44, 0x33, 0x22, 0x11, 0x01],
        now,
    );
    transport.send(0x80, &[0x0A]).unwrap();
    transport.poll(now);
    transport.receive(&[1, 0, 0], now);
    drop(transport);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(" IN  10.1.2.3:4567 len=10 00 FF 00 2E 00 ** ** ** ** **\n"));
    assert!(text.contains("    msg id=2E x=0 len=5 at=5 redacted\n"));
    assert!(text.contains(" OUT 10.1.2.3:4567 len=6 00 00 00 80 00 0A\n"));
    assert!(text.contains("    msg id=80 x=0 len=1 at="));
    assert!(text.contains(" IN  10.1.2.3:4567 len=3 01 00 00\n    not parsed: EmptyMessageArea\n"));
    assert!(!text.contains("44 33"));
    std::fs::remove_dir_all(&directory).unwrap();
}
