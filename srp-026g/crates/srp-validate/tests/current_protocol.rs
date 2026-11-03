//! Production encoders are an independent compatibility oracle for the guard.
use srp_validate::messages::{self, Dictionary as GuardDictionary};
use survarium::v026g::{dictionary::Dictionary, maps, match_msgs::static_info as info};

struct Data(&'static Dictionary);
impl GuardDictionary for Data {
    fn item(&self, id: u16) -> Option<bool> {
        self.0.item(id).map(|item| item.is_stack)
    }
    fn map(&self, id: u8) -> bool {
        maps::map_mode(id).is_some_and(|map| map.is_playable())
    }
}

fn options(blackout: bool) -> info::MatchOptions {
    let mut kit = info::MatchKit::empty(1, "kit");
    kit.slots[7] = Some((0, 1));
    info::MatchOptions {
        players: vec![info::MatchProfile {
            team: 0,
            name: "alice".into(),
            clan_tag: String::new(),
            protection: info::NO_PROTECTION,
            kits: vec![kit],
            items: vec![info::MatchItem {
                instance_id: 1,
                dict_id: Dictionary::shared().item_named("tt_33").unwrap().dict_id,
                condition_or_stack: 100,
                ..info::MatchItem::default()
            }],
            squad: 0,
        }],
        // Blackout is its maps' mode (#1243): the options carry its SRPX entry.
        map: if blackout {
            maps::playable()
                .find(|map| map.mode == maps::GameMode::Blackout)
                .and_then(|map| maps::MatchMap::of(map.id))
                .expect("a Blackout map")
        } else {
            maps::MatchMap::of(61).unwrap()
        },
        match_id: 0x1234_5678,
        match_level: 3,
        score_weights: info::SCORE_WEIGHTS,
    }
}

#[test]
fn current_server_options_and_blackout_are_accepted() {
    let data = Data(Dictionary::shared());
    for blackout in [false, true] {
        let bytes = info::options_bytes(&options(blackout), data.0).unwrap();
        assert_eq!(messages::options(&bytes, &data), Ok(1));
        let mut payload = vec![0];
        payload.extend(survarium::v026g::lzma2::stored(&bytes));
        assert_eq!(messages::check(messages::id::STATIC_INFO, &payload), Ok(()));
        // Every truncation of the mandatory native options is rejected.
        let native_len = bytes.len() - if blackout { 7 } else { 0 };
        for len in 0..native_len {
            assert!(
                messages::options(&bytes[..len], &data).is_err(),
                "prefix {len}"
            );
        }
        if blackout {
            for suffix in [
                &b"SRPX\x02\x01\x00"[..],
                b"SRPX\x09\x00",
                b"SRPY\x02\x01\x01",
            ] {
                let mut bad = bytes[..native_len].to_vec();
                bad.extend(suffix);
                assert!(messages::options(&bad, &data).is_err());
            }
        }
    }
}

#[test]
fn shipped_command_marker_is_allowed_but_unused_bit_and_nan_are_not() {
    for id in [messages::id::INPUT_RELAY, messages::id::INPUT_RELAY_PRECISE] {
        let mut payload = vec![0; messages::fixed_length(id).unwrap()];
        let actions = payload.len() - 4;
        payload[actions..].copy_from_slice(&((1u32 << 29) | (3 << 14)).to_le_bytes());
        assert_eq!(messages::check(id, &payload), Ok(()));
        payload[actions..].copy_from_slice(&(1u32 << 30).to_le_bytes());
        assert!(messages::check(id, &payload).is_err());
        payload[actions..].fill(0);
        payload[actions - 4..actions - 2].copy_from_slice(&0x7c00u16.to_le_bytes());
        assert!(messages::check(id, &payload).is_err());
    }
}
