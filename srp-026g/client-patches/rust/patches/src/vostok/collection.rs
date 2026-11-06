//! Fixed-capacity owned storage for patches without a heap allocator.
//! Vacant slots contain no value; only this type can change the occupied prefix.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapacityExceeded;

pub struct BoundedVec<T, const N: usize> { slots: [Option<T>; N], len: usize }

impl<T, const N: usize> Default for BoundedVec<T, N> {
    fn default() -> Self { Self::new() }
}
impl<T, const N: usize> BoundedVec<T, N> {
    pub const fn new() -> Self { Self { slots: [const { None }; N], len: 0 } }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn push(&mut self, value: T) -> Result<usize, CapacityExceeded> {
        if self.len == N { return Err(CapacityExceeded); }
        let index = self.len;
        self.slots[index] = Some(value);
        self.len += 1;
        Ok(index)
    }
    pub fn get(&self, index: usize) -> Option<&T> { self.slots.get(index)?.as_ref() }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> { self.slots.get_mut(index)?.as_mut() }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.slots[..self.len].iter().map(|slot| slot.as_ref().expect("occupied prefix"))
    }
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut T> {
        self.slots[..self.len].iter_mut().map(|slot| slot.as_mut().expect("occupied prefix"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_push_preserves_owners_and_values() {
        use std::{cell::Cell, rc::Rc};
        struct Tracked(Rc<Cell<usize>>);
        impl Drop for Tracked { fn drop(&mut self) { self.0.set(self.0.get() + 1); } }
        let drops = Rc::new(Cell::new(0));
        let mut entries = BoundedVec::<_, 2>::new();
        for i in 0..2 { assert_eq!(entries.push(Tracked(drops.clone())), Ok(i)); }
        assert_eq!(entries.push(Tracked(drops.clone())), Err(CapacityExceeded));
        assert_eq!(drops.get(), 1);
        assert_eq!(entries.iter().len(), 2);
        assert!(entries.get(2).is_none());
        drop(entries);
        assert_eq!(drops.get(), 3);
    }
}
