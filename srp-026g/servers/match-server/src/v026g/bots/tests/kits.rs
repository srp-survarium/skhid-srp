use super::*;

/// A kit's ammo is its storage for the match: each life starts with the
/// slot's rounds while the storage has them, only the rounds fired are
/// gone, and when the present kit has less than a magazine left the bot
/// picks, once, the kit with the most rounds for its next life.
#[test]
pub(crate) fn a_bot_spent_of_its_first_kit_picks_its_second_for_the_next_life() {
    use survarium::v026g::loadouts::BOT_AMMO_LIVES;
    let mut bots = Bots::new(
        0x417,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 1],
        &[true, true],
        &[Some("flanker"), Some("rusher")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    let items: Vec<&str> = bot
        .kits
        .iter()
        .map(|(arms, _)| arms.primary.0.item)
        .collect();
    assert_eq!(items, ["vityaz", "ak_74u", "mp_153"]);
    // Each kit full: a life's rounds (the bags' most) times the lives.
    let life: Vec<u32> = bot.kits.iter().map(|(arms, _)| arms.primary.1).collect();
    assert_eq!(life, [200, 120, 60]);
    let left: Vec<u32> = bot.kits.iter().map(|&(_, left)| left).collect();
    assert_eq!(
        left,
        life.iter().map(|n| n * BOT_AMMO_LIVES).collect::<Vec<_>>()
    );
    assert_eq!(bot.reflex.left(), 200);
    // Full: no choice. A life that fires 160 of its 200: the kit has
    // 1040 left, and it keeps the kit.
    assert_eq!(bot.pick_kit(), None);
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (32, 8);
    assert_eq!(bot.pick_kit(), None);
    // The next life in the same kit starts full again from its storage.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.kits[0].1, bot.reflex.left()), (0, 1040, 200));
    // Its storage down to 210, and this life fires 190 of them: 20 left,
    // under a magazine. The fullest other kit for the next life (the
    // AK-74U's 720 before the MP-153's 360), chosen once.
    bot.kits[0].1 = 210;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (10, 0);
    assert_eq!(bot.pick_kit(), Some((1, 20, 720)));
    assert_eq!(bot.pick_kit(), None);
    // Its next life is in kit 2 with a life's 120 of the AK-74U; kit 1
    // keeps its 20.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!(bot.kit, 1);
    assert_eq!(bot.reflex.held_gun().item, "ak_74u");
    assert_eq!(bot.reflex.left(), 120);
    assert_eq!(bot.kits[0].1, 20);
    // The rusher's kits: the MP-153, the Vityaz, the AK-74U.
    let rusher = bots.bots[1].as_ref().unwrap();
    let items: Vec<&str> = rusher
        .kits
        .iter()
        .map(|(arms, _)| arms.primary.0.item)
        .collect();
    assert_eq!(items, ["mp_153", "vityaz", "ak_74u"]);
}

/// Three kits, the first empty: the bot picks the fullest of the others,
/// whichever its place.
#[test]
pub(crate) fn a_bot_with_its_first_kit_empty_picks_the_fullest() {
    let mut bots = Bots::new(
        0x418,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0],
        &[true],
        &[Some("flanker")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    assert_eq!(bot.kits.len(), 3);
    // This life's 200 were the kit's last, all fired.
    bot.kits[0].1 = 200;
    bot.kits[1].1 = 100;
    bot.kits[2].1 = 300;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (0, 0);
    assert!(bot.reflex.dry());
    assert_eq!(bot.pick_kit(), Some((2, 0, 300)));
    assert_eq!(bot.pick_kit(), None);
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.reflex.held_gun().item), (2, "mp_153"));
    assert_eq!(bot.reflex.left(), 60);
    assert_eq!(bot.kits[0].1, 0);
}

/// Every kit under a magazine: another only when it has more rounds than
/// the present one, chosen once, and the choice does not go back and
/// forth while the present kit runs down.
#[test]
pub(crate) fn every_kit_low_the_bot_picks_the_fullest_once() {
    let mut bots = Bots::new(
        0x419,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0],
        &[true],
        &[Some("flanker")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    // The Vityaz's storage 200, this life fired 180: 20 left (under a
    // magazine). The AK-74U's kit 10 (of 30), the MP-153's 1.
    bot.kits[0].1 = 200;
    bot.kits[1].1 = 10;
    bot.kits[2].1 = 1;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (20, 0);
    // Nothing fuller than the present kit: it stays.
    assert_eq!(bot.pick_kit(), None);
    // Down to 5: the AK-74U's 10, once.
    bot.reflex.hands().primary.rounds = 5;
    assert_eq!(bot.pick_kit(), Some((1, 5, 10)));
    for rounds in [4, 2, 0] {
        bot.reflex.hands().primary.rounds = rounds;
        assert_eq!(bot.pick_kit(), None, "{rounds}");
    }
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.reflex.left()), (1, 10));
    // In the AK-74U's kit with its 10: the Vityaz's has nothing, the
    // MP-153's 1. No kit with more: it stays, and never goes back to the empty one.
    assert_eq!(bot.pick_kit(), None);
    bot.reflex.hands().primary.rounds = 0;
    bot.reflex.hands().primary.spare = 0;
    assert_eq!(bot.pick_kit(), Some((2, 0, 1)));
    assert_eq!(bot.pick_kit(), None);
}

/// A bot's kits are its tier's (`Bots::with_tiers`, read from its
/// profile): three kits, each with a life's rounds of its own ammo.
#[test]
pub(crate) fn a_bot_of_a_higher_tier_carries_that_tiers_kits() {
    use survarium::v026g::loadouts::{BotRole, BotTier};
    let bots = Bots::new(
        0x41A,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 1, 1],
        &[true, true, true],
        &[Some("support"), Some("camper"), None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    )
    .with_tiers(&[Some(BotTier::High), Some(BotTier::Mid), Some(BotTier::High)]);
    let items = |place: usize| -> Vec<&str> {
        bots.bots[place]
            .as_ref()
            .unwrap()
            .kits
            .iter()
            .map(|(arms, _)| arms.primary.0.item)
            .collect()
    };
    assert_eq!(items(0), ["ak_12", "sr3m", "vsk_94"]);
    assert_eq!(items(1), ["sv_98", "akmn", "pp2000"]);
    // A bot of no role keeps none.
    assert!(items(2).is_empty());
    let support = bots.bots[0].as_ref().unwrap();
    assert_eq!(support.reflex.held_gun().item, "ak_12");
    assert_eq!(support.reflex.left(), support.kits[0].0.primary.1);
    assert_eq!(support.role, Some(BotRole::Support));
}

/// A resupply zone's top-up moves rounds from the kit's storage into the
/// stack: what the bot holds rises to the life's count (or what the storage
/// has), what the kit has for the match is unchanged, and the next life
/// starts from the same storage.
#[test]
pub(crate) fn a_top_up_moves_the_kits_storage_into_the_stack_and_nowhere_else() {
    let mut bots = Bots::new(
        0x418,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0],
        &[true],
        &[Some("flanker")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    assert_eq!(bot.ammo().missing(), 0, "a full kit has nothing to refill");
    // A life that fired 160 of its 200: 40 in the bags and the magazine.
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (32, 8);
    let kit_total = bot.kit_left(0);
    assert_eq!(kit_total, 1_200 - 160);
    let ammo = bot.ammo();
    assert_eq!((ammo.have, ammo.whole), (40, 200));
    assert_eq!(ammo.missing(), 160);
    assert_eq!(bot.top_up(), 160);
    assert_eq!(bot.reflex.left(), 200);
    assert_eq!(bot.reflex.hands().primary.rounds, 32, "the magazine is not touched");
    assert_eq!(bot.kit_left(0), kit_total, "the kit's rounds only moved");
    assert_eq!(bot.ammo().missing(), 0);
    // The storage short: only what it has. The kit's last 70 rounds, 10 in
    // hand: 60 more, not the 190 the life's count would allow.
    bot.kits[0].1 = 70;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (0, 10);
    bot.life_carried = 80;
    assert_eq!(bot.kit_left(0), 70 - (80 - 10));
    let ammo = bot.ammo();
    assert_eq!(ammo.storage, 0);
    assert_eq!(bot.top_up(), 0, "nothing in the storage to give");
}
