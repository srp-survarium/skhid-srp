//! The player blocks of one saved Team Deathmatch snapshot: for each place
//! the item counts with which its state head reads plausibly, and what the
//! head says (`m_is_alive`). `tdm_blocks <file> <map id> <time> <players>`.
use survarium::v026g::maps;
use survarium::v026g::match_msgs::{players, snapshot};

fn main() {
    let mut args = std::env::args().skip(1);
    let file = args.next().expect("file");
    let map = maps::map_mode(args.next().unwrap().parse().unwrap()).expect("map");
    let time: u32 = args.next().unwrap().parse().unwrap();
    let count: usize = args.next().unwrap().parse().unwrap();
    let body = std::fs::read(file).unwrap();
    let decoded = snapshot::decode(time, &body, count, map.mode.rules(), 0).expect("decode");
    let tdm = decoded
        .rules
        .as_ref()
        .and_then(|rules| rules.tdm)
        .expect("tdm");
    println!(
        "tickets {:?} alive mask {:020b}",
        tdm.tickets, tdm.alive_players_mask
    );
    for block in players::blocks(&body).expect("blocks") {
        let reads: Vec<(usize, bool, bool)> = (0..400)
            .filter_map(|items| {
                let head = players::PlayerState::read(block.state, items)?;
                let sane = head.look_pitch.is_finite()
                    && head.look_pitch.abs() < 4.0
                    && head.rotation_y.is_finite()
                    && head.rotation_y.abs() < 8.0
                    && head.effects < 40;
                sane.then_some((items, head.alive, head.has_to_die))
            })
            .collect();
        let verdicts: std::collections::BTreeSet<(bool, bool)> =
            reads.iter().map(|read| (read.1, read.2)).collect();
        println!(
            "place {:2} state {:5} bytes  candidates {:3}  verdicts {:?}",
            block.place,
            block.state.len(),
            reads.len(),
            verdicts
        );
    }
}
