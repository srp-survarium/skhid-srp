//! Pickup targets attached to native input nodes, not to packet arrival.
//!
//! This is an input journal, not a saved simulation state. Rolling the world
//! back must retain it, just as native rollback retains its input lists. The
//! native owner supplies verified node transitions and clears it on teardown
//! or replay seek. Opaque node keys must be retired before native pool reuse.
use crate::state::{DropId, Place};
use core::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeKey(NonZeroU32);
impl NodeKey {
    pub fn new(identity: u32) -> Option<Self> {
        NonZeroU32::new(identity).map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Time(pub u32);

/// The two portions of a native input node. A server-moved input becomes a
/// corrected portion; moving the original portion must leave that one behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Original,
    Corrected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combine {
    Replace,
    Merge,
}

/// A completed native mutation, not a requested mutation. Refused native
/// placements produce no transition. Moving can remove the original portion
/// even when destination placement is refused. One transition updates ownership
/// and time together; there are no independent setters for those fields.
pub enum Change<'a> {
    /// Local coalescing and tick snapping move the complete native node,
    /// preserving both portions and adding no new target.
    Retime {
        node: NodeKey,
        picker: Place,
        time: Time,
    },
    Accept {
        node: NodeKey,
        picker: Place,
        time: Time,
        part: Part,
        combine: Combine,
        target: Option<DropId>,
    },
    MoveOriginal {
        from: NodeKey,
        picker: Place,
        /// None means native extraction succeeded but placement was refused.
        destination: Option<(NodeKey, Time)>,
    },
    /// Native F1 discards the whole node, including both portions.
    Retire(NodeKey),
    /// Network placement never retimes an existing node: an identity observed
    /// at a different time has been recycled. Keep only this generation.
    RetireStale {
        node: NodeKey,
        time: Time,
    },
    /// Pool maintenance: keep only the native nodes still owned by this place.
    Retain {
        picker: Place,
        nodes: &'a [NodeKey],
    },
    Clear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Full,
    /// A recycled key was not retired before being assigned to another player.
    NodeOwnerChanged,
}

/// One native outgoing input can encode exactly one original pickup target.
/// Corrected targets belong to received moves and must not be retransmitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmbiguousOriginal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    node: NodeKey,
    picker: Place,
    time: Time,
    part: Part,
    target: DropId,
}

/// Capacity is chosen by the native owner from the input-pool bounds. Full
/// is explicit and atomic: never silently evict a still-replayable request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputHistory<const N: usize> {
    entries: [Option<Entry>; N],
}

impl<const N: usize> Default for InputHistory<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> InputHistory<N> {
    pub const fn new() -> Self {
        Self { entries: [None; N] }
    }

    /// A native owner reserves room before calling an operation that can
    /// accept one new target. Retirement/reconciliation can only free room.
    pub fn has_spare_capacity(&self) -> bool {
        self.entries.iter().any(Option::is_none)
    }

    pub fn original_target(
        &self,
        node: NodeKey,
        picker: Place,
        time: Time,
    ) -> Result<Option<DropId>, AmbiguousOriginal> {
        let mut target = None;
        for entry in self.entries.iter().flatten().filter(|entry| {
            entry.node == node
                && entry.picker == picker
                && entry.time == time
                && entry.part == Part::Original
        }) {
            if target.is_some_and(|target| target != entry.target) {
                return Err(AmbiguousOriginal);
            }
            target = Some(entry.target);
        }
        Ok(target)
    }

    fn check_owner(&self, node: NodeKey, picker: Place) -> Result<(), Error> {
        if self
            .entries
            .iter()
            .flatten()
            .any(|e| e.node == node && e.picker != picker)
        {
            Err(Error::NodeOwnerChanged)
        } else {
            Ok(())
        }
    }

    pub fn apply(&mut self, change: Change<'_>) -> Result<(), Error> {
        match change {
            Change::RetireStale { node, time } => {
                for entry in &mut self.entries {
                    if entry
                        .as_ref()
                        .is_some_and(|e| e.node == node && e.time != time)
                    {
                        *entry = None;
                    }
                }
            }
            Change::Retime { node, picker, time } => {
                self.check_owner(node, picker)?;
                for entry in self.entries.iter_mut().flatten() {
                    if entry.node == node {
                        entry.time = time;
                    }
                }
            }
            Change::Accept {
                node,
                picker,
                time,
                part,
                combine,
                target,
            } => {
                self.check_owner(node, picker)?;
                let replaced =
                    |e: &Entry| combine == Combine::Replace && e.node == node && e.part == part;
                let already = target.is_some_and(|target| {
                    self.entries.iter().flatten().any(|e| {
                        e.node == node && e.part == part && e.target == target && !replaced(e)
                    })
                });
                // Reserve before mutation. A replacing input may use the space
                // its old portion frees, even when the journal is otherwise full.
                let free = if target.is_some() && !already {
                    Some(
                        self.entries
                            .iter()
                            .position(|e| e.as_ref().is_none_or(replaced))
                            .ok_or(Error::Full)?,
                    )
                } else {
                    None
                };
                for entry in &mut self.entries {
                    if let Some(e) = entry {
                        if replaced(e) {
                            *entry = None;
                        } else if e.node == node {
                            e.time = time;
                        }
                    }
                }
                if let (Some(at), Some(target)) = (free, target) {
                    self.entries[at] = Some(Entry {
                        node,
                        picker,
                        time,
                        part,
                        target,
                    });
                }
            }
            Change::MoveOriginal {
                from,
                picker,
                destination,
            } => {
                self.check_owner(from, picker)?;
                if let Some((to, _)) = destination {
                    self.check_owner(to, picker)?;
                }
                // Native move extracts the original portion BEFORE placement.
                // A refused destination consumes that portion too. Corrected
                // targets at the source survive either outcome. No allocation
                // or spare journal capacity is needed for this transition.
                for entry in &mut self.entries {
                    if let Some(e) = entry {
                        if e.node == from && e.part == Part::Original {
                            *entry = destination.map(|(to, time)| Entry {
                                node: to,
                                time,
                                part: Part::Corrected,
                                ..*e
                            });
                        } else if let Some((to, time)) = destination
                            && e.node == to
                            && e.time != time
                        {
                            // Network placement never pulls a node's time.
                            // This address belonged to an evicted generation.
                            // If source==destination, only its extracted own
                            // portion survives; corrected entries were evicted.
                            *entry = None;
                        }
                    }
                }
            }
            Change::Retire(node) => {
                self.entries
                    .iter_mut()
                    .filter(|e| e.is_some_and(|e| e.node == node))
                    .for_each(|e| *e = None);
            }
            Change::Retain { picker, nodes } => {
                self.entries
                    .iter_mut()
                    .filter(|e| e.is_some_and(|e| e.picker == picker && !nodes.contains(&e.node)))
                    .for_each(|e| *e = None);
            }
            Change::Clear => self.entries.fill(None),
        }
        Ok(())
    }

    /// Discrete events for this exact input time. Calling this again after a
    /// world rollback yields the same events. Within a time, picker then donor
    /// life order is independent of packet arrival, node addresses and merges.
    /// The same drop in both portions is yielded once.
    pub fn at(&self, time: Time) -> impl Iterator<Item = (Place, DropId)> + '_ {
        let mut after = None;
        core::iter::from_fn(move || {
            let next = self
                .entries
                .iter()
                .flatten()
                .filter(|e| e.time == time)
                .filter_map(|e| {
                    let key = (e.picker.get(), e.target.0.place.get(), e.target.0.spawned);
                    after
                        .is_none_or(|after| key > after)
                        .then_some((key, e.picker, e.target))
                })
                .min_by_key(|(key, _, _)| *key)?;
            after = Some(next.0);
            Some((next.1, next.2))
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::state::Life;

    #[test]
    fn outgoing_target_uses_exact_original_generation_and_refuses_ambiguity() {
        let node = NodeKey::new(1).unwrap();
        let picker = Place::parse(0).unwrap();
        let target = |spawned| {
            DropId(Life {
                place: Place::parse(1).unwrap(),
                spawned,
            })
        };
        let mut history = InputHistory::<3>::new();
        for (part, spawned) in [(Part::Original, 50), (Part::Corrected, 60)] {
            history
                .apply(Change::Accept {
                    node,
                    picker,
                    time: Time(800),
                    part,
                    combine: Combine::Merge,
                    target: Some(target(spawned)),
                })
                .unwrap();
        }
        let before = history.clone();
        for _ in 0..2 {
            assert_eq!(
                history.original_target(node, picker, Time(800)),
                Ok(Some(target(50)))
            );
        }
        assert_eq!(history, before);
        assert_eq!(history.original_target(node, picker, Time(850)), Ok(None));
        assert_eq!(
            history.original_target(node, Place::parse(1).unwrap(), Time(800)),
            Ok(None)
        );
        assert_eq!(
            history.original_target(NodeKey::new(2).unwrap(), picker, Time(800)),
            Ok(None)
        );
        history
            .apply(Change::Accept {
                node,
                picker,
                time: Time(800),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(target(70)),
            })
            .unwrap();
        assert_eq!(
            history.original_target(node, picker, Time(800)),
            Err(AmbiguousOriginal)
        );
    }

    #[test]
    fn recycled_network_node_retires_both_old_portions_but_not_other_nodes() {
        let picker = Place::parse(0).unwrap();
        let target = DropId(Life {
            place: Place::parse(1).unwrap(),
            spawned: 50,
        });
        let recycled = NodeKey::new(1).unwrap();
        let survivor = NodeKey::new(2).unwrap();
        let mut history = InputHistory::<3>::new();
        for (node, part) in [
            (recycled, Part::Original),
            (recycled, Part::Corrected),
            (survivor, Part::Original),
        ] {
            history
                .apply(Change::Accept {
                    node,
                    picker,
                    time: Time(800),
                    part,
                    combine: Combine::Merge,
                    target: Some(target),
                })
                .unwrap();
        }
        let unchanged = history.clone();
        history
            .apply(Change::RetireStale {
                node: recycled,
                time: Time(800),
            })
            .unwrap();
        assert_eq!(history, unchanged);
        history
            .apply(Change::RetireStale {
                node: recycled,
                time: Time(900),
            })
            .unwrap();
        assert_eq!(
            history.at(Time(800)).collect::<std::vec::Vec<_>>(),
            [(picker, target)]
        );
        history.apply(Change::Retire(survivor)).unwrap();
        assert_eq!(history.at(Time(800)).count(), 0);
        history
            .apply(Change::Accept {
                node: recycled,
                picker,
                time: Time(900),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(target),
            })
            .unwrap();
        assert_eq!(
            history.at(Time(900)).collect::<std::vec::Vec<_>>(),
            [(picker, target)]
        );
    }
    use std::vec::Vec;

    fn node(id: u32) -> NodeKey {
        NodeKey::new(id).unwrap()
    }
    fn place(id: u8) -> Place {
        Place::parse(id).unwrap()
    }
    fn drop(id: u32) -> DropId {
        DropId(Life {
            place: place(3),
            spawned: id,
        })
    }
    fn events<const N: usize>(journal: &InputHistory<N>, time: u32) -> Vec<(Place, DropId)> {
        journal.at(Time(time)).collect()
    }
    fn accept<const N: usize>(
        journal: &mut InputHistory<N>,
        id: u32,
        time: u32,
        part: Part,
        target: Option<DropId>,
    ) {
        journal
            .apply(Change::Accept {
                node: node(id),
                picker: place(0),
                time: Time(time),
                part,
                combine: Combine::Merge,
                target,
            })
            .unwrap();
    }

    #[test]
    fn local_pull_merge_and_world_rollback_keep_all_requests_at_effective_time() {
        let mut journal = InputHistory::<8>::new();
        accept(&mut journal, 1, 901, Part::Original, Some(drop(100)));
        // Original executable fixture: node901 is pulled to912, its action
        // mask is merged. An ordinary input must not remove its pickup.
        accept(&mut journal, 1, 912, Part::Original, None);
        assert!(events(&journal, 901).is_empty());
        assert_eq!(events(&journal, 912), [(place(0), drop(100))]);
        accept(&mut journal, 1, 923, Part::Original, Some(drop(200)));
        let replay = events(&journal, 923);
        assert_eq!(replay, [(place(0), drop(100)), (place(0), drop(200))]);
        // A rollback changes simulation state, not the native input journal.
        assert_eq!(events(&journal, 923), replay);
        assert!(events(&journal, 924).is_empty());
    }

    #[test]
    fn native_tick_snap_moves_both_portions_without_capacity_or_new_targets() {
        let mut journal = InputHistory::<2>::new();
        accept(&mut journal, 1, 912, Part::Original, Some(drop(100)));
        accept(&mut journal, 1, 912, Part::Corrected, Some(drop(200)));
        let before = journal.clone();
        assert_eq!(
            journal.apply(Change::Retime {
                node: node(1),
                picker: place(1),
                time: Time(950),
            }),
            Err(Error::NodeOwnerChanged)
        );
        assert_eq!(journal, before);
        journal
            .apply(Change::Retime {
                node: node(1),
                picker: place(0),
                time: Time(950),
            })
            .unwrap();
        assert!(events(&journal, 912).is_empty());
        assert_eq!(
            events(&journal, 950),
            [(place(0), drop(100)), (place(0), drop(200))]
        );
        // Repeated observations of the settled timestamp cannot add events.
        journal
            .apply(Change::Retime {
                node: node(1),
                picker: place(0),
                time: Time(950),
            })
            .unwrap();
        assert_eq!(events(&journal, 950).len(), 2);
        // A following server move still extracts only the original portion.
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: Some((node(2), Time(1000))),
            })
            .unwrap();
        assert_eq!(events(&journal, 950), [(place(0), drop(200))]);
        assert_eq!(events(&journal, 1000), [(place(0), drop(100))]);
    }

    #[test]
    fn move_takes_only_original_portion_and_discard_takes_both() {
        let mut journal = InputHistory::<8>::new();
        accept(&mut journal, 1, 800, Part::Original, Some(drop(100)));
        accept(&mut journal, 1, 800, Part::Corrected, Some(drop(200)));
        accept(&mut journal, 2, 850, Part::Original, Some(drop(300)));
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: Some((node(2), Time(850))),
            })
            .unwrap();
        assert_eq!(events(&journal, 800), [(place(0), drop(200))]);
        assert_eq!(
            events(&journal, 850),
            [(place(0), drop(100)), (place(0), drop(300))]
        );
        journal
            .apply(Change::MoveOriginal {
                from: node(2),
                picker: place(0),
                destination: Some((node(3), Time(900))),
            })
            .unwrap();
        assert_eq!(events(&journal, 850), [(place(0), drop(100))]);
        assert_eq!(events(&journal, 900), [(place(0), drop(300))]);
        journal.apply(Change::Retire(node(1))).unwrap();
        journal.apply(Change::Retire(node(2))).unwrap();
        assert!(events(&journal, 800).is_empty());
        assert!(events(&journal, 850).is_empty());
        assert_eq!(events(&journal, 900), [(place(0), drop(300))]);
    }

    #[test]
    fn replacement_capacity_reuse_and_reset_are_atomic() {
        let mut journal = InputHistory::<1>::new();
        accept(&mut journal, 1, 100, Part::Original, Some(drop(100)));
        let before = journal.clone();
        assert_eq!(
            journal.apply(Change::Accept {
                node: node(1),
                picker: place(0),
                time: Time(200),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(drop(200))
            }),
            Err(Error::Full)
        );
        assert_eq!(journal, before);
        journal
            .apply(Change::Accept {
                node: node(1),
                picker: place(0),
                time: Time(200),
                part: Part::Original,
                combine: Combine::Replace,
                target: Some(drop(200)),
            })
            .unwrap();
        assert!(events(&journal, 100).is_empty());
        assert_eq!(events(&journal, 200), [(place(0), drop(200))]);
        let before = journal.clone();
        assert_eq!(
            journal.apply(Change::Accept {
                node: node(1),
                picker: place(1),
                time: Time(300),
                part: Part::Original,
                combine: Combine::Replace,
                target: None
            }),
            Err(Error::NodeOwnerChanged)
        );
        assert_eq!(journal, before);
        // Recycle retires the prior generation even at the same address/time.
        journal.apply(Change::Retire(node(1))).unwrap();
        journal
            .apply(Change::Accept {
                node: node(1),
                picker: place(1),
                time: Time(200),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(drop(300)),
            })
            .unwrap();
        assert_eq!(events(&journal, 200), [(place(1), drop(300))]);
        journal.apply(Change::Clear).unwrap();
        assert!(events(&journal, 200).is_empty());
    }

    #[test]
    fn refused_move_consumes_original_but_keeps_corrected_and_self_move_is_stable() {
        let mut journal = InputHistory::<2>::new();
        accept(&mut journal, 1, 800, Part::Original, Some(drop(100)));
        accept(&mut journal, 1, 800, Part::Corrected, Some(drop(200)));
        let before = journal.clone();
        // Ownership failure is atomic even for extraction without placement.
        assert_eq!(
            journal.apply(Change::MoveOriginal {
                from: node(1),
                picker: place(1),
                destination: None,
            }),
            Err(Error::NodeOwnerChanged)
        );
        assert_eq!(journal, before);
        // Native source extraction succeeds, then destination is refused.
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: None,
            })
            .unwrap();
        assert_eq!(events(&journal, 800), [(place(0), drop(200))]);

        let mut journal = before;
        // A source may also be the destination, even with a full journal.
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: Some((node(1), Time(800))),
            })
            .unwrap();
        assert_eq!(
            events(&journal, 800),
            [(place(0), drop(100)), (place(0), drop(200))]
        );
        // It now has only corrected portions: another extraction changes none.
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: None,
            })
            .unwrap();
        assert_eq!(
            events(&journal, 800),
            [(place(0), drop(100)), (place(0), drop(200))]
        );
    }

    #[test]
    fn move_retires_a_recycled_destination_without_losing_the_extracted_source() {
        let mut journal = InputHistory::<4>::new();
        accept(&mut journal, 1, 800, Part::Original, Some(drop(100)));
        accept(&mut journal, 1, 800, Part::Corrected, Some(drop(200)));
        accept(&mut journal, 2, 600, Part::Original, Some(drop(300)));
        accept(&mut journal, 2, 600, Part::Corrected, Some(drop(400)));
        journal
            .apply(Change::MoveOriginal {
                from: node(1),
                picker: place(0),
                destination: Some((node(2), Time(900))),
            })
            .unwrap();
        assert!(events(&journal, 600).is_empty());
        assert_eq!(events(&journal, 800), [(place(0), drop(200))]);
        assert_eq!(events(&journal, 900), [(place(0), drop(100))]);

        // The pool can reuse the source itself. Its corrected portion must
        // not reappear at the new time merely because the pointer is equal.
        accept(&mut journal, 2, 900, Part::Original, Some(drop(500)));
        journal
            .apply(Change::MoveOriginal {
                from: node(2),
                picker: place(0),
                destination: Some((node(2), Time(950))),
            })
            .unwrap();
        assert!(events(&journal, 900).is_empty());
        assert_eq!(events(&journal, 950), [(place(0), drop(500))]);
    }

    #[test]
    fn merged_targets_are_not_lost_and_order_does_not_depend_on_arrival() {
        let mut a = InputHistory::<12>::new();
        let mut b = InputHistory::<12>::new();
        for order in [[3, 1, 2], [2, 1, 3]] {
            let journal = if order[0] == 3 { &mut a } else { &mut b };
            for id in order {
                accept(journal, 1, 1000, Part::Corrected, Some(drop(id)));
            }
            accept(journal, 1, 1000, Part::Original, Some(drop(1)));
        }
        assert_eq!(events(&a, 1000), events(&b, 1000));
        assert_eq!(
            events(&a, 1000),
            [
                (place(0), drop(1)),
                (place(0), drop(2)),
                (place(0), drop(3))
            ]
        );
        a.apply(Change::Retain {
            picker: place(1),
            nodes: &[],
        })
        .unwrap();
        assert_eq!(events(&a, 1000).len(), 3);
        a.apply(Change::Retain {
            picker: place(0),
            nodes: &[],
        })
        .unwrap();
        assert!(events(&a, 1000).is_empty());
    }
}
