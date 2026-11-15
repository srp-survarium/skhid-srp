//! [Gap 3] The clocks of a player block's saved mixing tree: the tree's clock and each node's time group, own-object bit,
//! next events and time-scale operand, read as the client's `anim_state_deserialize` 0x3A99C0 reads them
//! (`world_frame::animation_validate` walks the same bits for the world check; this reader keeps the few values the bots
//! use). The times table at the `T`-bit mark is read first (`decode_times_in_ms_impl` 0x3A9580), then the floats table
//! after it (`decode_floats_impl` 0x3A9720); the readers take each first to last in the writer's order (0x3A9965..0x3A9981,
//! 0x3A98C7..0x3A98F5). Then the tree's bits: the node count, the base tracks (two passes), the object's matrix (12
//! floats, `anim_init_node_matrices` 0x3A8D60), the node records, and each node's operand expressions
//! (`anim_build_node` 0x3A9250, pre-order). Read the same way on 456,702 of 456,702 blocks of 60 recordings: every table
//! popped to its end, the tree's bits to their end (settle-tree 0).

/// A node's next animation event (state +78h..+87h, `anim_read_node_time_track` 0x3A8AF0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NextEvent {
    /// Its type (+84h: 4 an interval's end, 8 the clip's end, 10h a backwards end, 20h a channel knot, 40h a direction
    /// change; the saved 6 bits are the type >> 1).
    pub kind: u8,
    /// Its time on the tree's clock (+80h).
    pub at: u32,
    /// Its time in the clip, seconds (the iterator's interval time, the float the reader pops after the time).
    pub clip_s: f32,
}

/// A time-scale operand (`anim_read_expression` 0x3A90C0 op 0, the node 0xA23F2C): the clip runs at `scale` from
/// `clip_s` seconds into it at `start` on the tree's clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeScale {
    /// +8: clip seconds a second.
    pub scale: f32,
    /// +0Ch: the clip's time at `start`, seconds.
    pub clip_s: f32,
    /// +10h: on the tree's clock.
    pub start: u32,
}

impl TimeScale {
    /// When the clip reaches `clip_s` seconds, on the tree's clock: `start + floor(f32(f32(f32(clip_s - self.clip_s) /
    /// scale) x 1000))`, the next interval end from the base as the client computes it (0x3B6C5F; settle-tree L39, the
    /// fire clip's 8,685 of 8,685 passes). `None`: a scale not above 0, or a time before the base's.
    pub fn reaches(&self, clip_s: f32) -> Option<u32> {
        if !(self.scale > 0.0 && clip_s >= self.clip_s) {
            return None;
        }
        let ms = ((clip_s - self.clip_s) / self.scale * 1000.0).floor();
        (ms < 4_294_967_296.0).then(|| self.start.wrapping_add(ms as u32))
    }
}

/// One node of a saved mixing tree, as far as its clocks go (`mixing::animation_state` and `n_ary_tree_animation_node`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimNode {
    /// The node's time-synchronisation group (node +40h, 5 bits; 31 is none): the weapon state's lexeme group, 6 fire,
    /// 4 reload, 2 show / hide, 7 chamber, 3 idle (`get_user_hands_lexeme_impl` 0x31EBD0).
    pub time_group: Option<u8>,
    /// The node animates its own object (an item's model) rather than the body (the record's bit read into byte +5Dh).
    pub own_object: bool,
    pub event: Option<NextEvent>,
    /// The node's next weight event (state +8Ch..+97h, `anim_read_node_flag_track` 0x3A8BA0): its type (100h a weight
    /// transition ends, 2 the lexeme ends: 102h the lexeme ends at weight 0, the node goes) and its time on the tree's
    /// clock (+90h).
    pub weight: Option<(u16, u32)>,
    /// The node's operands when they are one time-scale node (`None`: none, more than one, or another kind).
    pub time_scale: Option<TimeScale>,
}

/// A saved mixing tree's clock and its nodes' next events ([`AnimEvents::read`]).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimEvents {
    /// The tree's clock as saved (`animation_player::serialize_state` 0x395A90 clones the tree with a shift of
    /// 1,000,000,000 - its clock, 0x395AAA): the world's time.
    pub clock: u32,
    pub nodes: Vec<AnimNode>,
}

/// The most operand expressions a tree is read for (a node's operands are a handful; a stream that asks for more is
/// refused).
const EXPRESSIONS_MOST: u32 = 4_096;

impl AnimEvents {
    /// The tree of a player block's animation stream. `None`: the bytes end first, a table runs out, the tree's bits do
    /// not end at the mark, or the operands go over [`EXPRESSIONS_MOST`] (a stream `world_frame::animation_validate`
    /// refuses).
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let mut b = Bits { bytes, at: 0 };
        let tree = b.read(16)? as usize;
        let end = 16usize.checked_add(tree)?;
        b.at = end;
        let times = times_table(&mut b)?;
        let floats = floats_table(&mut b)?;
        let mut p = Pops { times: times.into_iter(), floats: floats.into_iter() };
        b.at = 16;
        let clock = p.time()?;
        let count = b.read(6)? as usize;
        b.read(1)?;
        let channels = b.read(4)?.count_ones();
        // `anim_read_node_base_tracks` 0x3A8C10: every node's base members (two bits, its interval time and weight, two
        // bits), then each one's two iterators.
        for _ in 0..count {
            b.skip(2)?;
            p.float()?;
            p.float()?;
            b.skip(2)?;
        }
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            let half = b.read(6)?;
            let event = if half != 0 {
                let at = p.time()?;
                b.skip(1)?;
                let clip_s = p.float()?;
                b.skip(8 + 4)?;
                Some(NextEvent { kind: u8::try_from(half << 1).ok()?, at, clip_s })
            } else {
                None
            };
            let ended = b.read(1)?;
            let lexeme = b.read(1)?;
            let flags = u16::try_from(ended * 0x100 + lexeme * 2).ok()?;
            let weight = if flags != 0 { Some((flags, p.time()?)) } else { None };
            nodes.push(AnimNode { time_group: None, own_object: false, event, weight, time_scale: None });
        }
        for _ in 0..12 {
            p.float()?;
        }
        let mut roots = Vec::with_capacity(count);
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
            b.skip(2 + 1 + 3 + 3 + 2 + 1 + 1)?;
            roots.push(b.read(2)?);
        }
        let mut budget = EXPRESSIONS_MOST;
        for (node, roots) in nodes.iter_mut().zip(roots) {
            let mut operands = Vec::new();
            for _ in 0..roots {
                operands.push(expression(&mut b, &mut p, channels, &mut budget)?);
            }
            node.time_scale = match operands.as_slice() {
                [only] => *only,
                _ => None,
            };
        }
        (b.at == end).then_some(Self { clock, nodes })
    }

    /// A time on the tree's clock as the game's ms, the world made at `world_ms`: `world_ms + (saved - clock)`.
    pub fn game_ms(&self, saved: u32, world_ms: u32) -> u32 {
        world_ms.wrapping_add(saved.wrapping_sub(self.clock))
    }
}

/// One operand expression, pre-order (`anim_read_expression` 0x3A90C0): 2 bits of op and a channel pick; op 0 a time-scale
/// node (its scale and clip time from the floats, its start from the times), op 1 a weight node (a float), op 2 a weight
/// transition and op 3 a time-scale transition (a time, then their two operands). `Some(Some(_))`: a time-scale node.
fn expression(b: &mut Bits<'_>, p: &mut Pops, channels: u32, budget: &mut u32) -> Option<Option<TimeScale>> {
    *budget = budget.checked_sub(1)?;
    let op = b.read(2)?;
    channel_pick(b, channels)?;
    match op {
        0 => {
            let scale = p.float()?;
            let clip_s = p.float()?;
            let start = p.time()?;
            Some(Some(TimeScale { scale, clip_s, start }))
        }
        1 => {
            p.float()?;
            Some(None)
        }
        _ => {
            p.time()?;
            expression(b, p, channels, budget)?;
            expression(b, p, channels, budget)?;
            Some(None)
        }
    }
}

/// The two tables' values, taken first to last.
struct Pops {
    times: std::vec::IntoIter<u32>,
    floats: std::vec::IntoIter<u32>,
}

impl Pops {
    fn time(&mut self) -> Option<u32> {
        self.times.next()
    }

    fn float(&mut self) -> Option<f32> {
        self.floats.next().map(f32::from_bits)
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

/// The floats table (`decode_floats_impl` 0x3A9720), in the writer's order: a mode bit and a 10-bit count; then either
/// each float's 32 bits (mode 0) or the count of the different ones, each 32 bits, and an index into them for each
/// (mode 1).
fn floats_table(b: &mut Bits<'_>) -> Option<Vec<u32>> {
    let indexed = b.read(1)? != 0;
    let count = b.read(10)?;
    if !indexed {
        return (0..count).map(|_| b.read(32)).collect();
    }
    let unique = b.read(width(count))?;
    let table = (0..unique).map(|_| b.read(32)).collect::<Option<Vec<u32>>>()?;
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

    /// A one-channel tree of two nodes, written field by field as the client's writer orders them: the reload node (group
    /// 4) with a channel knot (20h) 944 ms after the clock at 6.866667 s into its clip and one time-scale operand (1.7857145
    /// clip seconds a second from 4.644644 s, 300 ms before the clock: a world of a Vepr-12 reload in the sample), the idle
    /// node (group 3, its own object's) with a weight event 102h and no operand. The times table (mode 0) holds the clock,
    /// the knot, the weight event and the operand's start in the order they are popped; the floats table (mode 0) the two
    /// base floats of each node, the knot's clip time, the matrix's 12, the operand's two.
    fn vepr_tree(knot: u32) -> Vec<u8> {
        let mut tree = Writer { bytes: Vec::new(), at: 0 };
        tree.put(2, 6); // two nodes
        tree.put(0, 1); // one matrix
        tree.put(0b0001, 4); // one channel
        for _ in 0..2 {
            tree.put(0, 4); // the base members' bits
        }
        // The reload node: an event of type `knot` (saved as half of it), its interval bit, channels, domain; no weight event.
        tree.put(knot >> 1, 6);
        tree.put(0, 1);
        tree.put(0, 8);
        tree.put(0, 4);
        tree.put(0, 2);
        // The idle node: no animation event; a weight event 102h.
        tree.put(0, 6);
        tree.put(1, 1);
        tree.put(1, 1);
        for (own, group, roots) in [(0, 4, 1), (1, 3, 0)] {
            tree.put(0, 4);
            tree.put(0, 1);
            tree.put(0, 1); // one interval
            tree.put(0, 10);
            tree.put(0, 1);
            tree.put(0, 1);
            tree.put(own, 1);
            tree.put(group, 5);
            tree.put(0, 2 + 1 + 3 + 3 + 2 + 1 + 1);
            tree.put(roots, 2);
        }
        // The reload node's operand: op 0 (the channel pick takes no bits with one channel).
        tree.put(0, 2);
        let tree_bits = tree.at;
        let mut all = Writer { bytes: Vec::new(), at: 0 };
        all.put(tree_bits as u32, 16);
        for i in 0..tree_bits {
            all.put(u32::from((tree.bytes[i / 8] >> (7 - i % 8)) & 1), 1);
        }
        // Times: 4, mode 0, base 999,999,700, width 11: the clock 1e9, the knot 1e9 + 944, the weight event 1e9 + 400, the
        // operand's start 1e9 - 300.
        all.put(4, 10);
        all.put(0, 1);
        all.put(999_999_700, 32);
        all.put(11, 5);
        for read in [300, 1_244, 700, 0] {
            all.put(read, 11);
        }
        // Floats: mode 0, 19.
        let floats = [[5.180358, 1.0, 0.0, 1.0, 6.866667].as_slice(), &[0.0; 12], &[1.7857145, 4.644644]].concat();
        all.put(0, 1);
        all.put(floats.len() as u32, 10);
        for float in floats {
            all.put(f32::to_bits(float), 32);
        }
        all.bytes
    }

    #[test]
    fn a_saved_tree_gives_each_nodes_group_next_events_and_time_scale() {
        let events = AnimEvents::read(&vepr_tree(0x20)).expect("a tree");
        assert_eq!(events.clock, 1_000_000_000);
        let scale = TimeScale { scale: 1.7857145, clip_s: 4.644644, start: 999_999_700 };
        assert_eq!(
            events.nodes,
            vec![
                AnimNode {
                    time_group: Some(4),
                    own_object: false,
                    event: Some(NextEvent { kind: 0x20, at: 1_000_000_944, clip_s: 6.866667 }),
                    weight: None,
                    time_scale: Some(scale),
                },
                AnimNode {
                    time_group: Some(3),
                    own_object: true,
                    event: None,
                    weight: Some((0x102, 1_000_000_400)),
                    time_scale: None,
                },
            ]
        );
        assert_eq!(events.game_ms(1_000_000_944, 48_600), 49_544);
        // The knot itself is where the operand reaches its clip time: 944 ms after the clock.
        assert_eq!(scale.reaches(6.866667), Some(1_000_000_944));
        // The Vepr-12's empty reload clip, 209 frames: 6.9666677 s, reached 1,000 ms after the clock (the sample's next
        // world, 1,050 ms later, shows the magazine filled).
        assert_eq!(scale.reaches(209.0 * 0.033333335), Some(1_000_001_000));
        // No time before the base, no scale.
        assert_eq!(scale.reaches(4.0), None);
        assert_eq!(TimeScale { scale: 0.0, ..scale }.reaches(6.0), None);
    }

    /// A stream whose tables run out, or whose tree's bits end elsewhere than the mark, is no tree.
    #[test]
    fn a_stream_that_runs_out_is_no_tree() {
        let bytes = vepr_tree(0x20);
        assert!(AnimEvents::read(&bytes[..bytes.len() - 8]).is_none());
        let mut longer = bytes.clone();
        longer[0] = 0;
        longer[1] = longer[1].wrapping_add(1);
        assert!(AnimEvents::read(&longer).is_none());
    }
}
