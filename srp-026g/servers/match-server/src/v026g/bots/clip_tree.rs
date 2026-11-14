//! [Gap 3] The clocks of a player block's saved mixing tree: the tree's clock and each node's time group, own-object bit
//! and next events, read as the client's `anim_state_deserialize` 0x3A99C0 reads them (`world_frame::animation_validate`
//! walks the same bits for the world check; this reader keeps the few values the bots use). The times table at the `T`-bit
//! mark is read first (`decode_times_in_ms_impl` 0x3A9580; the readers take it first to last in the writer's order,
//! 0x3A9965..0x3A9981), then the tree's bits: the node count, the base tracks (two passes), the node records. The
//! expressions and the floats are not read.

/// One node of a saved mixing tree, as far as its clocks go (`mixing::animation_state` and `n_ary_tree_animation_node`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimNode {
    /// The node's time-synchronisation group (node +40h, 5 bits; 31 is none): the weapon state's lexeme group, 6 fire,
    /// 4 reload, 2 show / hide, 7 chamber, 3 idle (`get_user_hands_lexeme_impl` 0x31EBD0).
    pub time_group: Option<u8>,
    /// The node animates its own object (an item's model) rather than the body (the record's bit read into byte +5Dh).
    pub own_object: bool,
    /// The node's next animation event (state +78h..+87h, `anim_read_node_time_track` 0x3A8AF0): its type (+84h: 4 an
    /// interval's end, 8 the clip's end, 10h a backwards end, 20h a channel knot, 40h a direction change; the saved
    /// 6 bits are the type >> 1) and its time on the tree's clock (+80h).
    pub event: Option<(u8, u32)>,
    /// The node's next weight event (state +8Ch..+97h, `anim_read_node_flag_track` 0x3A8BA0): its type (100h a weight
    /// transition ends, 2 the lexeme ends: 102h the lexeme ends at weight 0, the node goes) and its time on the tree's
    /// clock (+90h).
    pub weight: Option<(u16, u32)>,
}

/// A saved mixing tree's clock and its nodes' next events ([`AnimEvents::read`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimEvents {
    /// The tree's clock as saved (`animation_player::serialize_state` 0x395A90 clones the tree with a shift of
    /// 1,000,000,000 - its clock, 0x395AAA): the world's time.
    pub clock: u32,
    pub nodes: Vec<AnimNode>,
}

impl AnimEvents {
    /// The tree of a player block's animation stream. `None`: the bytes end first, or the tree pops more times than the
    /// table holds (a stream `world_frame::animation_validate` refuses).
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let mut b = Bits { bytes, at: 0 };
        let tree = b.read(16)? as usize;
        b.at = 16usize.checked_add(tree)?;
        let mut times = times_table(&mut b)?.into_iter();
        b.at = 16;
        let clock = times.next()?;
        let count = b.read(6)? as usize;
        b.read(1)?;
        let channels = b.read(4)?.count_ones();
        for _ in 0..count {
            b.skip(4)?;
        }
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            let half = b.read(6)?;
            let event = if half != 0 {
                let at = times.next()?;
                b.skip(1 + 8 + 4)?;
                Some((u8::try_from(half << 1).ok()?, at))
            } else {
                None
            };
            let ended = b.read(1)?;
            let lexeme = b.read(1)?;
            let flags = u16::try_from(ended * 0x100 + lexeme * 2).ok()?;
            let weight = if flags != 0 { Some((flags, times.next()?)) } else { None };
            nodes.push(AnimNode { time_group: None, own_object: false, event, weight });
        }
        for node in &mut nodes {
            b.skip(4 + 1)?;
            let intervals = b.read(1)?;
            for _ in 0..=intervals {
                b.skip(10 + 1)?;
            }
            b.skip(1)?;
            channel_pick(&mut b, channels)?;
            channel_pick(&mut b, channels)?;
            node.own_object = b.read(1)? != 0;
            let group = b.read(5)?;
            node.time_group = (group != 31).then(|| u8::try_from(group).ok()).flatten();
            b.skip(2 + 1 + 3 + 3 + 2 + 1 + 1 + 2)?;
        }
        Some(Self { clock, nodes })
    }

    /// A time on the tree's clock as the game's ms, the world made at `world_ms`: `world_ms + (saved - clock)`.
    pub fn game_ms(&self, saved: u32, world_ms: u32) -> u32 {
        world_ms.wrapping_add(saved.wrapping_sub(self.clock))
    }
}

/// The bits an index below `count` takes (0x3A961E, 0x396120).
fn width(count: u32) -> usize {
    (u32::BITS.saturating_sub(count.leading_zeros())).max(1) as usize
}

/// A channel pick (`anim_pick_channel_value` 0x3A8EF0): 0, 1 or 2 bits by the channel count.
fn channel_pick(b: &mut Bits<'_>, channels: u32) -> Option<()> {
    match channels {
        0 => None,
        1 => Some(()),
        2 => b.skip(1),
        _ => b.skip(2),
    }
}

/// The times table (`decode_times_in_ms_impl` 0x3A9580), in the writer's order: a 10-bit count, a mode bit, a 32-bit
/// base; for more than one, a 5-bit width and then either each time as the base plus a read (mode 0) or a sorted table of
/// the different ones (the base, then each the one before plus a read plus 1) and an index into it for each (mode 1).
fn times_table(b: &mut Bits<'_>) -> Option<Vec<u32>> {
    let count = b.read(10)?;
    if count == 0 {
        return Some(Vec::new());
    }
    let indexed = b.read(1)? != 0;
    let base = b.read(32)?;
    if count == 1 {
        return Some(vec![base]);
    }
    let each = b.read(5)? as usize;
    if !indexed {
        return (0..count).map(|_| b.read(each).map(|read| base.wrapping_add(read))).collect();
    }
    let unique = b.read(width(count))?;
    let mut table = Vec::with_capacity(unique as usize);
    table.push(base);
    for _ in 1..unique {
        let before = *table.last()?;
        table.push(before.wrapping_add(b.read(each)?).wrapping_add(1));
    }
    (0..count).map(|_| b.read(width(unique)).and_then(|at| table.get(at as usize).copied())).collect()
}

/// Reads bits most significant first, as the animation encoder writes them (0x3A8980; `world_frame`'s reader).
struct Bits<'a> {
    bytes: &'a [u8],
    /// In bits from the start.
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, bits: usize) -> Option<u32> {
        let end = self.at.checked_add(bits)?;
        if bits > 32 || end > self.bytes.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0u64;
        for at in self.at..end {
            let byte = *self.bytes.get(at / 8)?;
            value = (value << 1) | u64::from((byte >> (7 - at % 8)) & 1);
        }
        self.at = end;
        u32::try_from(value).ok()
    }

    fn skip(&mut self, bits: usize) -> Option<()> {
        self.at = self.at.checked_add(bits)?;
        (self.at <= self.bytes.len().checked_mul(8)?).then_some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bits written most significant first, as the game's encoder does.
    struct Writer {
        bytes: Vec<u8>,
        at: usize,
    }

    impl Writer {
        fn put(&mut self, value: u32, bits: usize) {
            for i in (0..bits).rev() {
                if self.at / 8 >= self.bytes.len() {
                    self.bytes.push(0);
                }
                if (value >> i) & 1 != 0 {
                    self.bytes[self.at / 8] |= 1 << (7 - self.at % 8);
                }
                self.at += 1;
            }
        }
    }

    /// A one-channel tree of two nodes, written field by field as the client's writer orders them: the reload node (group 4)
    /// with its clip's end 1,350 ms after the clock, the idle node (group 3) with a weight event; the times table (mode 0)
    /// holds the clock and the two times in the order they are popped.
    #[test]
    fn a_saved_tree_gives_each_nodes_group_and_next_events() {
        let mut tree = Writer { bytes: Vec::new(), at: 0 };
        tree.put(2, 6); // two nodes
        tree.put(0, 1); // one matrix
        tree.put(0b0001, 4); // one channel
        for _ in 0..2 {
            tree.put(0, 4); // the base tracks' bits
        }
        // The reload node: an event of type 0Ch (saved as 6), its interval bit, channels, domain; no weight event.
        tree.put(6, 6);
        tree.put(0, 1);
        tree.put(0, 8);
        tree.put(0, 4);
        tree.put(0, 2);
        // The idle node: no animation event; a weight event 102h.
        tree.put(0, 6);
        tree.put(1, 1);
        tree.put(1, 1);
        for (own, group) in [(0, 4), (1, 3)] {
            tree.put(0, 4);
            tree.put(0, 1);
            tree.put(0, 1); // one interval
            tree.put(0, 10);
            tree.put(0, 1);
            tree.put(0, 1);
            tree.put(own, 1);
            tree.put(group, 5);
            tree.put(0, 2 + 1 + 3 + 3 + 2 + 1 + 1);
            tree.put(0, 2); // no expression
        }
        let tree_bits = tree.at;
        let mut all = Writer { bytes: Vec::new(), at: 0 };
        all.put(tree_bits as u32, 16);
        for i in 0..tree_bits {
            all.put(u32::from((tree.bytes[i / 8] >> (7 - i % 8)) & 1), 1);
        }
        // Times: 3, mode 0, base 1e9, width 11: 0, 1350, 400.
        all.put(3, 10);
        all.put(0, 1);
        all.put(1_000_000_000, 32);
        all.put(11, 5);
        for read in [0, 1_350, 400] {
            all.put(read, 11);
        }
        // Floats: none.
        all.put(0, 1);
        all.put(0, 10);
        let events = AnimEvents::read(&all.bytes).expect("a tree");
        assert_eq!(events.clock, 1_000_000_000);
        assert_eq!(
            events.nodes,
            vec![
                AnimNode { time_group: Some(4), own_object: false, event: Some((0x0C, 1_000_001_350)), weight: None },
                AnimNode { time_group: Some(3), own_object: true, event: None, weight: Some((0x102, 1_000_000_400)) },
            ]
        );
        assert_eq!(events.game_ms(1_000_001_350, 60_000), 61_350);
    }
}
