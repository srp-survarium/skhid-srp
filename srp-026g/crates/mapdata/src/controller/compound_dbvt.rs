//! [L36] The order the game's compound shape offers its children to a sweep: Bullet 2.79's `btDbvt` as the 0.26g exe builds a
//! compound's child tree, and the order `btDbvt::collideTV` 0xA58F0 walks it in (exactness `grid/rig/s7-20261114/L36-tie-order.md`
//! §3; the reference port `dbvt_port.py`, checked against the exe's own insert and update in Unicorn: 213 of 213 Vector
//! Laboratory and 204 of 204 Chemical Plant trees, structure and volume bits, 31,350 children).
//!
//! - `create_compound_shape` 0x7A460: `addChildShape` 0x8A3E0 per primitive child in order (`btDbvt::insert` 0xAA690 with the
//!   child's `getAabb` under its transform, before the compound's scaling), then always `setLocalScaling` 0x8AD90, which per
//!   child in order scales it and updates its leaf (`updateChildTransform` 0x8A620 -> `btDbvt::update` 0xAA710: `removeleaf`
//!   0xAA330, then `insertleaf` 0xAA180 from the root, `m_lkhd` -1); the library's mesh is added after (inserted only).
//! - Select and Merge are SSE over four lanes (0xAA1BD..0xAA20E: `(x + z) + (y + w)` of the absolute sum differences,
//!   `cmpless`; `minps` / `maxps`), the w lanes of `getAabb`'s results 0; Contain and NotEqual take three lanes.
//! - `collideTV` pops the last node and pushes `childs[0]` then `childs[1]`: `childs[1]` first. Every child whose box meets
//!   the query comes in the order of this walk over the whole tree.

type V3 = [f32; 3];
type V4 = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Volume {
    pub(super) mi: V4,
    pub(super) mx: V4,
}

impl Volume {
    /// `btDbvtVolume::FromMM` of a `getAabb` result (its w lanes 0).
    pub(super) fn of(lo: V3, hi: V3) -> Self {
        Self { mi: [lo[0], lo[1], lo[2], 0.0], mx: [hi[0], hi[1], hi[2], 0.0] }
    }
}

/// Select's lane sum: `|(a.mi + a.mx) - (o.mi + o.mx)|` per lane, then `(x + z) + (y + w)`.
fn proximity(o: &Volume, a: &Volume) -> f32 {
    let d: V4 = [0, 1, 2, 3].map(|k| ((a.mi[k] + a.mx[k]) - (o.mi[k] + o.mx[k])).abs());
    (d[0] + d[2]) + (d[1] + d[3])
}

/// 1 when `b` is nearer `o` than `a` or as near (`cmpless`: b <= a), else 0.
fn select(o: &Volume, a: &Volume, b: &Volume) -> usize {
    usize::from(proximity(o, b) <= proximity(o, a))
}

fn merge(a: &Volume, b: &Volume) -> Volume {
    let min = |x: f32, y: f32| if x < y { x } else { y };
    let max = |x: f32, y: f32| if x > y { x } else { y };
    Volume { mi: [0, 1, 2, 3].map(|k| min(a.mi[k], b.mi[k])), mx: [0, 1, 2, 3].map(|k| max(a.mx[k], b.mx[k])) }
}

pub(super) fn contains(outer: &Volume, inner: &Volume) -> bool {
    (0..3).all(|k| outer.mi[k] <= inner.mi[k]) && (0..3).all(|k| inner.mx[k] <= outer.mx[k])
}

#[derive(Clone, Copy, Debug)]
struct Node {
    volume: Volume,
    parent: Option<usize>,
    /// An inner node's two children; none for a leaf.
    childs: Option<[usize; 2]>,
    /// A leaf's child index.
    data: u32,
}

#[derive(Debug, Default)]
pub(super) struct Dbvt {
    nodes: Vec<Node>,
    root: Option<usize>,
}

impl Dbvt {
    fn new_node(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// `btDbvt::insert` 0xAA690: a new leaf, inserted from the root.
    pub(super) fn insert(&mut self, volume: Volume, data: u32) -> usize {
        let leaf = self.new_node(Node { volume, parent: None, childs: None, data });
        self.insert_leaf(leaf);
        leaf
    }

    /// `insertleaf` 0xAA180 from the root.
    fn insert_leaf(&mut self, leaf: usize) {
        let Some(mut root) = self.root else {
            self.root = Some(leaf);
            self.nodes[leaf].parent = None;
            return;
        };
        while let Some([c0, c1]) = self.nodes[root].childs {
            root = if select(&self.nodes[leaf].volume, &self.nodes[c0].volume, &self.nodes[c1].volume) == 1 { c1 } else { c0 };
        }
        let prev = self.nodes[root].parent;
        let volume = merge(&self.nodes[leaf].volume, &self.nodes[root].volume);
        let node = self.new_node(Node { volume, parent: prev, childs: Some([root, leaf]), data: 0 });
        self.nodes[root].parent = Some(node);
        self.nodes[leaf].parent = Some(node);
        match prev {
            Some(p) => {
                let childs = self.nodes[p].childs.as_mut().expect("a parent is an inner node");
                if childs[1] == root {
                    childs[1] = node;
                } else {
                    childs[0] = node;
                }
                let (mut node, mut prev) = (node, p);
                loop {
                    if contains(&self.nodes[prev].volume, &self.nodes[node].volume) {
                        break;
                    }
                    let [c0, c1] = self.nodes[prev].childs.expect("an inner node");
                    self.nodes[prev].volume = merge(&self.nodes[c0].volume, &self.nodes[c1].volume);
                    node = prev;
                    match self.nodes[node].parent {
                        Some(up) => prev = up,
                        None => break,
                    }
                }
            }
            None => self.root = Some(node),
        }
    }

    /// `removeleaf` 0xAA330 (the parent node goes; which node the tree's free slot caches does not change the tree).
    fn remove_leaf(&mut self, leaf: usize) {
        if self.root == Some(leaf) {
            self.root = None;
            return;
        }
        let parent = self.nodes[leaf].parent.expect("a leaf under the root has a parent");
        let [c0, c1] = self.nodes[parent].childs.expect("an inner node");
        let sibling = if c1 == leaf { c0 } else { c1 };
        match self.nodes[parent].parent {
            Some(prev) => {
                let childs = self.nodes[prev].childs.as_mut().expect("an inner node");
                if childs[1] == parent {
                    childs[1] = sibling;
                } else {
                    childs[0] = sibling;
                }
                self.nodes[sibling].parent = Some(prev);
                let mut at = Some(prev);
                while let Some(p) = at {
                    let before = self.nodes[p].volume;
                    let [a, b] = self.nodes[p].childs.expect("an inner node");
                    let after = merge(&self.nodes[a].volume, &self.nodes[b].volume);
                    self.nodes[p].volume = after;
                    // NotEqual over three lanes.
                    if (0..3).any(|k| before.mi[k] != after.mi[k] || before.mx[k] != after.mx[k]) {
                        at = self.nodes[p].parent;
                    } else {
                        break;
                    }
                }
            }
            None => {
                self.root = Some(sibling);
                self.nodes[sibling].parent = None;
            }
        }
        self.nodes[leaf].parent = None;
    }

    /// A node's volume.
    pub(super) fn volume(&self, node: usize) -> Volume {
        self.nodes[node].volume
    }

    /// `btDbvt::update` 0xAA710: the leaf out, its volume set, in again from the root.
    pub(super) fn update(&mut self, leaf: usize, volume: Volume) {
        self.remove_leaf(leaf);
        self.nodes[leaf].volume = volume;
        self.insert_leaf(leaf);
    }

    /// `collideTV`'s order of the leaves: `childs[1]` before `childs[0]`.
    pub(super) fn offering_order(&self) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.root.into_iter().collect();
        while let Some(n) = stack.pop() {
            match self.nodes[n].childs {
                None => out.push(self.nodes[n].data),
                Some([c0, c1]) => {
                    stack.push(c0);
                    stack.push(c1);
                }
            }
        }
        out
    }
}

/// How one child comes into the tree, by its `getAabb` boxes ([`crate::bullet::Added`]).
#[derive(Clone, Copy, Debug)]
pub(super) enum ChildBox {
    /// Inserted with the first box (before the compound's scaling), then updated with the second.
    BeforeScaling { inserted: (V3, V3), scaled: (V3, V3) },
    /// Inserted with this box after the scaling.
    AfterScaling((V3, V3)),
}

/// Each child's place in the order the compound's tree offers its children (`rank[i]` for child `i`), from the children in
/// the compound's order: the primitives inserted, then all updated, in order, then the ones added after the scaling inserted.
pub(super) fn offering_ranks(children: &[ChildBox]) -> Vec<u32> {
    let mut tree = Dbvt::default();
    let mut leaves = vec![None; children.len()];
    for (i, child) in children.iter().enumerate() {
        if let ChildBox::BeforeScaling { inserted: (lo, hi), .. } = child {
            leaves[i] = Some(tree.insert(Volume::of(*lo, *hi), i as u32));
        }
    }
    for (i, child) in children.iter().enumerate() {
        if let (ChildBox::BeforeScaling { scaled: (lo, hi), .. }, Some(leaf)) = (child, leaves[i]) {
            tree.update(leaf, Volume::of(*lo, *hi));
        }
    }
    for (i, child) in children.iter().enumerate() {
        if let ChildBox::AfterScaling((lo, hi)) = child {
            tree.insert(Volume::of(*lo, *hi), i as u32);
        }
    }
    let mut rank = vec![0u32; children.len()];
    for (place, child) in tree.offering_order().into_iter().enumerate() {
        rank[child as usize] = place as u32;
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(x: f32) -> (V3, V3) {
        ([x, 0.0, 0.0], [x + 1.0, 1.0, 1.0])
    }

    /// Two children: the second is inserted as the root's `childs[1]` and the walk offers it first.
    #[test]
    fn the_walk_offers_childs_1_first() {
        let rank = offering_ranks(&[
            ChildBox::BeforeScaling { inserted: unit(0.0), scaled: unit(0.0) },
            ChildBox::BeforeScaling { inserted: unit(5.0), scaled: unit(5.0) },
        ]);
        assert_eq!(rank.len(), 2);
        assert_ne!(rank[0], rank[1]);
    }

    /// Every child gets a place, each once.
    #[test]
    fn the_places_are_a_permutation() {
        let children: Vec<ChildBox> = (0..9)
            .map(|i| ChildBox::BeforeScaling { inserted: unit(i as f32 * 1.5), scaled: unit(((i * 7) % 9) as f32) })
            .chain(std::iter::once(ChildBox::AfterScaling(unit(3.0))))
            .collect();
        let mut rank = offering_ranks(&children);
        rank.sort_unstable();
        assert_eq!(rank, (0..10).collect::<Vec<u32>>());
    }
}
