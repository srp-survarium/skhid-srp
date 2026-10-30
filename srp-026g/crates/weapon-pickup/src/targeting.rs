//! Client-only selection against a weapon model's oriented bounding box.
//! These values never enter simulation, snapshots, hashes or network packets.
//! The adapter supplies the actual draw transform, after cosmetic ragdoll
//! posing, and a camera segment ending at the allowed interaction distance.

use crate::state::{DropId, World};

/// Finite camera-to-reach segment. Its endpoint defines the reach; an infinite
/// ray cannot accidentally select a weapon across the map.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    start: [f64; 3],
    end: [f64; 3],
}

impl Segment {
    pub fn parse(start: [f32; 3], end: [f32; 3]) -> Option<Self> {
        (start.iter().chain(&end).all(|v| v.is_finite()) && start != end).then_some(Self {
            start: start.map(f64::from),
            end: end.map(f64::from),
        })
    }
}

/// A model-local box and the inverse of its exact affine draw transform.
/// Rotation, reflection, nonuniform scale and shear are retained. A world-axis
/// enclosing box would make the empty space beside a tilted rifle clickable.
#[derive(Clone, Copy, Debug)]
pub struct WeaponBox {
    min: [f64; 3],
    max: [f64; 3],
    inverse: [[f64; 3]; 3],
    translation: [f64; 3],
}

impl WeaponBox {
    /// `model_bounds` is already in the renderer's local coordinate system.
    /// `draw` is Vostok's row-vector 4x4 matrix, translation at indices 12..15.
    /// Invalid boxes and singular/projective matrices cannot become targets.
    pub fn parse(model_bounds: [f32; 6], draw: [f32; 16]) -> Option<Self> {
        if !model_bounds.iter().chain(&draw).all(|v| v.is_finite())
            || (0..3).any(|i| model_bounds[i] > model_bounds[i + 3])
            || [draw[3], draw[7], draw[11], draw[15]] != [0.0, 0.0, 0.0, 1.0]
        {
            return None;
        }
        // Work in f64 so even extreme finite f32 input cannot overflow the
        // determinant or the intermediate coordinate subtraction.
        let a = [draw[0], draw[1], draw[2]].map(f64::from);
        let b = [draw[4], draw[5], draw[6]].map(f64::from);
        let c = [draw[8], draw[9], draw[10]].map(f64::from);
        let cross = |u: [f64; 3], v: [f64; 3]| {
            [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ]
        };
        let columns = [cross(b, c), cross(c, a), cross(a, b)];
        let determinant = dot(a, columns[0]);
        if determinant == 0.0 {
            return None;
        }
        Some(Self {
            min: [model_bounds[0], model_bounds[1], model_bounds[2]].map(f64::from),
            max: [model_bounds[3], model_bounds[4], model_bounds[5]].map(f64::from),
            inverse: columns.map(|column| column.map(|v| v / determinant)),
            translation: [draw[12], draw[13], draw[14]].map(f64::from),
        })
    }

    /// First intersection as a fraction of the finite segment, including its
    /// endpoints. Starting inside the box produces zero. Parallel rays outside
    /// a slab are rejected without a division by zero or an epsilon-sized halo.
    pub fn intersection(&self, segment: Segment) -> Option<f64> {
        let local = |point: [f64; 3]| {
            let delta = core::array::from_fn(|i| point[i] - self.translation[i]);
            self.inverse.map(|column| dot(delta, column))
        };
        let start = local(segment.start);
        let end = local(segment.end);
        let mut enter = 0.0_f64;
        let mut leave = 1.0_f64;
        for i in 0..3 {
            let direction = end[i] - start[i];
            if direction == 0.0 {
                if start[i] < self.min[i] || start[i] > self.max[i] {
                    return None;
                }
                continue;
            }
            let a = (self.min[i] - start[i]) / direction;
            let b = (self.max[i] - start[i]) / direction;
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
            if enter > leave {
                return None;
            }
        }
        Some(enter)
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Copied draw data, never an engine pointer. A pose alone grants no pickup:
/// selection also requires this precise life to have an available drop.
#[derive(Clone, Copy, Debug)]
pub struct RenderedDrop {
    pub id: DropId,
    pub bounds: WeaponBox,
}

/// One match's presentation cache. A new frame retires every previous pose;
/// only completion of all player draw tasks makes its observations selectable.
/// Missing/culled/nonweapon draws therefore cannot leave clickable old boxes.
pub struct Frames {
    state: FrameState,
}

enum FrameState {
    Drawing([Option<RenderedDrop>; crate::state::MAX_PLACES]),
    Published([Option<RenderedDrop>; crate::state::MAX_PLACES]),
}

pub enum FrameEvent {
    Begin,
    Draw(RenderedDrop),
    Complete,
}

impl Frames {
    pub const fn new() -> Self {
        Self {
            state: FrameState::Published([None; crate::state::MAX_PLACES]),
        }
    }

    /// The draw-task join must precede Complete. Calls are serialized by the
    /// native match owner's lock; no partially written matrix is published.
    pub fn transition(&mut self, event: FrameEvent) {
        match event {
            FrameEvent::Begin => {
                self.state = FrameState::Drawing([None; crate::state::MAX_PLACES]);
            }
            FrameEvent::Draw(view) => {
                if let FrameState::Drawing(views) = &mut self.state {
                    views[view.id.0.place.index()] = Some(view);
                }
            }
            FrameEvent::Complete => {
                if let FrameState::Drawing(views) = &self.state {
                    self.state = FrameState::Published(*views);
                }
            }
        }
    }

    pub fn select(&self, world: &World, segment: Segment) -> Option<DropId> {
        match &self.state {
            FrameState::Drawing(_) => None,
            FrameState::Published(views) => select(world, segment, views.iter().flatten().copied()),
        }
    }
}

/// Select only an available drop, nearest first. Equal-depth ties use the
/// roster place so parallel player drawing cannot change the selected target.
/// The caller supplies the current frame's views; stale frame retention is
/// the presentation owner's responsibility, not deterministic simulation.
fn select(
    world: &World,
    segment: Segment,
    views: impl IntoIterator<Item = RenderedDrop>,
) -> Option<DropId> {
    let mut nearest: Option<(f64, DropId)> = None;
    for view in views {
        if world.drop(view.id).is_none() {
            continue;
        }
        let Some(depth) = view.bounds.intersection(segment) else {
            continue;
        };
        if nearest.is_none_or(|(old_depth, old_id)| {
            depth < old_depth
                || (depth == old_depth && view.id.0.place.get() < old_id.0.place.get())
        }) {
            nearest = Some((depth, view.id));
        }
    }
    nearest.map(|(_, id)| id)
}

#[cfg(test)]
mod tests;
